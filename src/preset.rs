use crate::artwork_pipeline::ArtworkPipelineSettings;
use crate::model::{
    Document, RenderVariant, SourceArtwork, normalize_crosshatch_render,
    normalize_render_variant_canvas,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;

/// The only treatment preset format accepted by current Toniator builds.
const CURRENT_PRESET_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTreatment {
    pub render: RenderVariant,
    pub native_settings: Option<crate::model::Settings>,
    pub artwork_pipeline: ArtworkPipelineSettings,
    pub canvas_normalized: bool,
}

#[derive(Deserialize)]
struct PresetHeader {
    format: String,
    version: u32,
}

#[derive(Deserialize)]
struct CurrentPresetV3 {
    #[serde(default)]
    settings: Option<crate::model::Settings>,
    render: RenderVariant,
    artwork_pipeline: ArtworkPipelineSettings,
}

pub fn parse_treatment(bytes: &[u8], source_dimensions: (u32, u32)) -> Result<ParsedTreatment> {
    let header: PresetHeader =
        serde_json::from_slice(bytes).context("Could not read this treatment preset")?;
    ensure!(
        header.format == "toniator-preset",
        "This is not a Toniator treatment preset"
    );
    ensure!(
        header.version == CURRENT_PRESET_VERSION,
        "This preset was created with an unsupported pre-release Toniator format."
    );
    let preset: CurrentPresetV3 =
        serde_json::from_slice(bytes).context("Could not read this current treatment preset")?;
    preset
        .artwork_pipeline
        .validate()
        .map_err(anyhow::Error::msg)?;

    let mut render = preset.render;
    let canvas_normalized =
        normalize_render_variant_canvas(&mut render, source_dimensions.0, source_dimensions.1);

    // The semantic pipeline is authoritative. Rebuild its temporary legacy
    // facade before validating the renderer-owned settings.
    let mut document = validation_document();
    document.render = render;
    document.artwork_pipeline = preset.artwork_pipeline.clone();
    document.sync_legacy_projection()?;
    normalize_crosshatch_render(&mut document.render);
    document.validate()?;
    Ok(ParsedTreatment {
        render: document.render,
        native_settings: preset.settings.map(crate::model::Settings::sanitized),
        artwork_pipeline: preset.artwork_pipeline,
        canvas_normalized,
    })
}

pub fn document_treatment_preset_bytes(name: &str, document: &Document) -> Result<Vec<u8>> {
    let name = name.trim();
    ensure!(!name.is_empty(), "Treatment name cannot be empty");
    let mut canonical = document.clone();
    canonical.canonicalize_pipeline_facades()?;
    normalize_crosshatch_render(&mut canonical.render);
    canonical.validate()?;
    let value = serde_json::json!({
        "format": "toniator-preset",
        "version": CURRENT_PRESET_VERSION,
        "name": name,
        "settings": canonical.settings,
        "render": canonical.render,
        "artwork_pipeline": canonical.artwork_pipeline,
    });
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validation_document() -> Document {
    Document::new(SourceArtwork {
        name: "preset-validation".into(),
        media_type: "application/octet-stream".into(),
        bytes: std::sync::Arc::from([1]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artwork_pipeline::{
        ArtworkSource, ChannelAssignment, LegacyBrightnessKind, LegacyCompatibilityAssignment,
        OutputModel, SourceAlphaPolicy,
    };
    use crate::model::{ValueMode, WebShapeSettings};

    #[test]
    fn current_preset_round_trips_and_pre_release_is_rejected() {
        let document = validation_document();
        let bytes = document_treatment_preset_bytes("Current", &document).unwrap();
        let parsed = parse_treatment(&bytes, (900, 620)).unwrap();
        assert_eq!(parsed.render, document.render);
        assert_eq!(parsed.artwork_pipeline, document.artwork_pipeline);

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["version"] = serde_json::json!(2);
        let error = parse_treatment(&serde_json::to_vec(&value).unwrap(), (900, 620))
            .unwrap_err()
            .to_string();
        assert!(error.contains("unsupported pre-release"));
    }

    #[test]
    fn bundled_current_presets_parse_with_authoritative_pipeline_state() {
        for (name, bytes) in [
            (
                "Chunky Fingerprints",
                include_bytes!("../assets/presets/Chunky Fingerprints.tntr").as_slice(),
            ),
            (
                "ComicBook",
                include_bytes!("../assets/presets/ComicBook.tntr").as_slice(),
            ),
            (
                "Skinny Curve",
                include_bytes!("../assets/presets/Skinny Curve.tntr").as_slice(),
            ),
            (
                "Tiled Stacked Motif Stress Test",
                include_bytes!("../assets/presets/Tiled Stacked Motif Stress Test.tntr").as_slice(),
            ),
        ] {
            let parsed = parse_treatment(bytes, (900, 620))
                .unwrap_or_else(|error| panic!("{name} did not parse: {error:#}"));
            parsed.artwork_pipeline.validate().unwrap();
            let mut document = validation_document();
            document.render = parsed.render;
            document.artwork_pipeline = parsed.artwork_pipeline;
            document.sync_legacy_projection().unwrap();
            document.validate().unwrap();
        }
    }

    #[test]
    fn current_preset_pipeline_overrides_contradictory_renderer_facade() {
        let mut render = WebShapeSettings {
            value_mode: ValueMode::Rgb,
            ..Default::default()
        };
        render.single_channel = crate::model::Ink::Red;
        let value = serde_json::json!({
            "format": "toniator-preset",
            "version": CURRENT_PRESET_VERSION,
            "name": "Contradictory facade",
            "render": RenderVariant::WebShapeV1 { settings: Box::new(render) },
            "artwork_pipeline": ArtworkPipelineSettings::default(),
        });
        let parsed = parse_treatment(&serde_json::to_vec(&value).unwrap(), (900, 620)).unwrap();
        let RenderVariant::WebShapeV1 { settings } = parsed.render else {
            panic!("fixture changed treatment kind")
        };
        assert_eq!(settings.value_mode, ValueMode::Cmyk);
        assert_eq!(parsed.artwork_pipeline.output_model, OutputModel::CmykPrint);
    }

    #[test]
    fn current_crosshatch_preset_is_configured_as_curves_and_unknown_ids_reject() {
        let pipeline = ArtworkPipelineSettings {
            source: ArtworkSource::LegacyBrightness(LegacyBrightnessKind::EncodedRec709InvertedV1),
            alpha_policy: SourceAlphaPolicy::LegacyCurrentV1,
            output_model: OutputModel::RgbScreen,
            assignment: ChannelAssignment::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1,
            ),
            active_channel: None,
        };
        let value = serde_json::json!({
            "format": "toniator-preset",
            "version": CURRENT_PRESET_VERSION,
            "name": "Crosshatch",
            "render": RenderVariant::WebShapeV1 {
                settings: Box::new(WebShapeSettings::default())
            },
            "artwork_pipeline": pipeline,
        });
        let parsed = parse_treatment(&serde_json::to_vec(&value).unwrap(), (900, 620)).unwrap();
        assert!(matches!(parsed.render, RenderVariant::WebCurveV1 { .. }));
        assert_eq!(parsed.artwork_pipeline, pipeline);

        let mut unknown = value;
        unknown["artwork_pipeline"]["output_model"] = serde_json::json!("output.unknown");
        assert!(parse_treatment(&serde_json::to_vec(&unknown).unwrap(), (900, 620)).is_err());
    }
}
