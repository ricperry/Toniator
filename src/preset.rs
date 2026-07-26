//! Versioned, scoped `.tntr` treatment presets.
//!
//! The wire format deliberately stores semantic pipeline/channel identifiers,
//! never GTK positions or the renderer's legacy mapping facade.  Renderer
//! `value_mode`/`single_channel` remain internal compatibility projections.

use crate::artwork_pipeline::{
    ArtworkPipelineSettings, ChannelAssignment, LegacyCompatibilityAssignment, OutputChannelId,
};
use crate::model::{
    ClosedShapePath, Document, RenderVariant, Settings, WebCurveChannel, WebCurveSettings,
    WebShapeChannel, WebShapeSettings, normalize_crosshatch_render,
    normalize_render_variant_canvas,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The only treatment-preset format accepted by current Toniator builds.
pub const CURRENT_PRESET_VERSION: u32 = 4;

const FORMAT: &str = "toniator-preset";
const KIND_NATIVE: &str = "treatment.native_basic.v1";
const KIND_SHAPES: &str = "treatment.web_shape.v1";
const KIND_CURVES: &str = "treatment.web_curve.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PresetScope {
    Pipeline,
    Treatment,
    Channel,
    CompleteWorkflow,
}

impl PresetScope {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pipeline => "Pipeline",
            Self::Treatment => "Treatment",
            Self::Channel => "Current Channel",
            Self::CompleteWorkflow => "Complete Workflow",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTreatment {
    pub scope: PresetScope,
    pub artwork_pipeline: Option<ArtworkPipelineSettings>,
    treatment: Option<TreatmentSection>,
    channels: Option<ChannelSection>,
    pub canvas_normalized: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PresetHeader {
    format: String,
    version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentPresetV4 {
    format: String,
    version: u32,
    name: String,
    scope: PresetScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pipeline: Option<ArtworkPipelineSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    treatment: Option<TreatmentSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    channel: Option<ChannelSection>,
}

/// Treatment-global values only. Per-channel maps are intentionally separated
/// into `ChannelSection`, so treatment imports do not repaint channels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TreatmentSection {
    kind: String,
    settings: Settings,
    render: Value,
}

/// Semantic channel records are sorted before serialization.  A channel-scope
/// preset has exactly one record; complete workflow has every active-model
/// record.  The map never uses renderer slot labels such as `c` or `r`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChannelSection {
    treatment_kind: String,
    channels: BTreeMap<String, Value>,
}

pub fn parse_treatment(bytes: &[u8], source_dimensions: (u32, u32)) -> Result<ParsedTreatment> {
    let mut raw: Value =
        serde_json::from_slice(bytes).context("Could not read this treatment preset")?;
    let header: PresetHeader =
        serde_json::from_value(raw.clone()).context("Could not read this treatment preset")?;
    ensure!(
        header.format == FORMAT,
        "This is not a Toniator treatment preset"
    );
    ensure!(
        header.version == CURRENT_PRESET_VERSION,
        "This preset was created with an unsupported pre-release Toniator format."
    );
    let omitted_active_channel = inject_active_channel_for_current_parse(&mut raw)?;
    validate_current_v4_nested_fields(&raw)?;
    let mut preset: CurrentPresetV4 =
        serde_json::from_value(raw).context("Could not read this current treatment preset")?;
    ensure!(
        !preset.name.trim().is_empty(),
        "Preset name cannot be empty"
    );
    validate_scope(&preset)?;
    if let Some(pipeline) = &mut preset.pipeline {
        if omitted_active_channel && matches!(pipeline.assignment, ChannelAssignment::ActiveChannel)
        {
            pipeline.active_channel = None;
            let mut representative = pipeline.clone();
            representative.active_channel = Some(pipeline.output_model.default_channel());
            representative.validate().map_err(anyhow::Error::msg)?;
        } else {
            pipeline.validate().map_err(anyhow::Error::msg)?;
        }
    }

    let mut canvas_normalized = false;
    if let Some(treatment) = &mut preset.treatment {
        let mut render = render_from_treatment(treatment, None)?;
        canvas_normalized =
            normalize_render_variant_canvas(&mut render, source_dimensions.0, source_dimensions.1);
        // Re-split the validated normalized form; this guarantees that no
        // legacy projection or channel map can leak into the wire state.
        *treatment = treatment_from_render(treatment.settings, &render)?;
    }
    if let Some(channels) = &preset.channel {
        validate_channel_section(channels, preset.scope)?;
    }

    Ok(ParsedTreatment {
        scope: preset.scope,
        artwork_pipeline: preset.pipeline,
        treatment: preset.treatment,
        channels: preset.channel,
        canvas_normalized,
    })
}

/// `ArtworkPipelineSettings` correctly rejects an ActiveChannel assignment
/// without a destination.  A v4 preset is allowed to omit that destination to
/// request transition restoration, so insert a temporary valid representative
/// solely for serde and restore `None` immediately after parsing.
fn inject_active_channel_for_current_parse(raw: &mut Value) -> Result<bool> {
    let Some(pipeline) = raw.get_mut("pipeline").and_then(Value::as_object_mut) else {
        return Ok(false);
    };
    if pipeline.contains_key("active_channel")
        || pipeline.get("assignment").and_then(Value::as_str) != Some("assignment.active_channel")
    {
        return Ok(false);
    }
    let output = match pipeline.get("output_model").and_then(Value::as_str) {
        Some("output.cmyk_print") => OutputChannelId::CmykCyan,
        Some("output.rgb_screen") => OutputChannelId::RgbRed,
        _ => return Ok(false),
    };
    pipeline.insert(
        "active_channel".into(),
        Value::String(output.stable_id().into()),
    );
    Ok(true)
}

/// Build a complete, validated document without touching the live editor.
/// `DocumentEditor::replace_with_preset_candidate` commits this candidate as
/// one undoable edit only after every declared section succeeds.
impl ParsedTreatment {
    pub fn candidate_for(&self, document: &Document) -> Result<Document> {
        let mut candidate = document.clone();
        // A complete Crosshatch preset changes both pipeline and treatment.
        // First use the normal output transition (which preserves paired mode
        // caches), then install the treatment while the outgoing snapshot still
        // has a valid ordinary pipeline.  Only then install the final semantic
        // pipeline and its compatibility projection.
        if self.treatment.is_some()
            && let Some(pipeline) = &self.artwork_pipeline
            && candidate.artwork_pipeline.output_model != pipeline.output_model
        {
            candidate.switch_output_mode(pipeline.output_model.to_legacy());
        }
        if let Some(treatment) = &self.treatment {
            let render = render_from_treatment(treatment, Some(&candidate.render))?;
            candidate.apply_preset_treatment(render, Some(treatment.settings))?;
        }
        if let Some(pipeline) = &self.artwork_pipeline {
            candidate.apply_preset_pipeline_unchecked(pipeline.clone())?;
        }
        if let Some(channels) = &self.channels {
            apply_channel_section(&mut candidate, channels)?;
        }
        normalize_crosshatch_render(&mut candidate.render);
        candidate.sync_legacy_projection()?;
        candidate.validate()?;
        Ok(candidate)
    }
}

pub fn document_treatment_preset_bytes(name: &str, document: &Document) -> Result<Vec<u8>> {
    document_preset_bytes(name, document, PresetScope::Treatment)
}

pub fn document_preset_bytes(
    name: &str,
    document: &Document,
    scope: PresetScope,
) -> Result<Vec<u8>> {
    let name = name.trim();
    ensure!(!name.is_empty(), "Preset name cannot be empty");
    let mut canonical = document.clone();
    canonical.canonicalize_pipeline_facades()?;
    normalize_crosshatch_render(&mut canonical.render);
    canonical.validate()?;

    let treatment = match scope {
        PresetScope::Treatment | PresetScope::CompleteWorkflow => Some(treatment_from_render(
            canonical.settings,
            &canonical.render,
        )?),
        _ => None,
    };
    let pipeline = match scope {
        PresetScope::Pipeline | PresetScope::CompleteWorkflow => {
            Some(canonical.artwork_pipeline.clone())
        }
        _ => None,
    };
    let channel = match scope {
        PresetScope::Channel => {
            ensure!(
                !is_crosshatch_pipeline(Some(&canonical.artwork_pipeline)),
                "Crosshatch does not support Current Channel preset scope"
            );
            Some(channel_from_document(&canonical, false)?)
        }
        PresetScope::CompleteWorkflow => Some(channel_from_document(&canonical, true)?),
        _ => None,
    };
    let preset = CurrentPresetV4 {
        format: FORMAT.to_owned(),
        version: CURRENT_PRESET_VERSION,
        name: name.to_owned(),
        scope,
        pipeline,
        treatment,
        channel,
    };
    let mut bytes = serde_json::to_vec_pretty(&preset)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_scope(preset: &CurrentPresetV4) -> Result<()> {
    let expected = match preset.scope {
        PresetScope::Pipeline => (true, false, false),
        PresetScope::Treatment => (false, true, false),
        PresetScope::Channel => (false, false, true),
        PresetScope::CompleteWorkflow => (true, true, true),
    };
    ensure!(
        (
            preset.pipeline.is_some(),
            preset.treatment.is_some(),
            preset.channel.is_some()
        ) == expected,
        "Preset sections contradict its declared scope"
    );
    if is_crosshatch_pipeline(preset.pipeline.as_ref()) {
        ensure!(
            preset.scope == PresetScope::CompleteWorkflow,
            "Crosshatch presets require Complete Workflow scope"
        );
        ensure!(
            preset
                .treatment
                .as_ref()
                .is_some_and(|treatment| treatment.kind == KIND_CURVES),
            "Crosshatch presets require a Curves treatment"
        );
        let Some(channel) = preset.channel.as_ref() else {
            unreachable!("complete workflow scope requires a channel section")
        };
        ensure!(
            channel.channels.len() == OutputChannelId::CMYK.len()
                && OutputChannelId::CMYK
                    .iter()
                    .all(|id| channel.channels.contains_key(id.stable_id())),
            "Crosshatch presets require CMYK compatibility channel records"
        );
    }
    Ok(())
}

fn is_crosshatch_pipeline(pipeline: Option<&ArtworkPipelineSettings>) -> bool {
    pipeline.is_some_and(|pipeline| {
        matches!(
            pipeline.assignment,
            ChannelAssignment::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1
            )
        )
    })
}

/// Current v4 parsing is intentionally stricter than the project DTOs.  The
/// shared model structs keep backward-compatible serde defaults for v6 project
/// data, while this schema walk rejects unknown preset fields before serde can
/// discard them.  Templates are produced from the current native types so the
/// format stays aligned with supported geometry without making project loading
/// stricter as a side effect.
fn validate_current_v4_nested_fields(raw: &Value) -> Result<()> {
    let object = raw
        .as_object()
        .context("Current preset must be a JSON object")?;
    if let Some(pipeline) = object.get("pipeline") {
        let mut schema = serde_json::to_value(ArtworkPipelineSettings::default())?;
        schema
            .as_object_mut()
            .expect("pipeline serialization is an object")
            .insert(
                "active_channel".into(),
                Value::String("channel.cmyk.cyan".into()),
            );
        validate_known_fields(pipeline, &schema, "pipeline")?;
    }
    if let Some(treatment) = object.get("treatment") {
        let treatment = treatment
            .as_object()
            .context("Treatment section must be an object")?;
        let settings = treatment
            .get("settings")
            .context("Treatment section is missing settings")?;
        validate_known_fields(
            settings,
            &serde_json::to_value(Settings::default())?,
            "treatment.settings",
        )?;
        let kind = treatment
            .get("kind")
            .and_then(Value::as_str)
            .context("Treatment section is missing kind")?;
        let render = treatment
            .get("render")
            .context("Treatment section is missing render")?;
        match kind {
            KIND_NATIVE => {}
            KIND_SHAPES => {
                validate_known_fields(render, &shape_treatment_schema()?, "treatment.render")?
            }
            KIND_CURVES => {
                validate_known_fields(render, &curve_treatment_schema()?, "treatment.render")?
            }
            _ => {}
        }
    }
    if let Some(channel) = object.get("channel") {
        let channel = channel
            .as_object()
            .context("Channel section must be an object")?;
        let kind = channel
            .get("treatment_kind")
            .and_then(Value::as_str)
            .context("Channel section is missing treatment kind")?;
        let values = channel
            .get("channels")
            .and_then(Value::as_object)
            .context("Channel section is missing channel records")?;
        let schema = match kind {
            KIND_SHAPES => shape_channel_schema()?,
            KIND_CURVES => serde_json::to_value(WebCurveChannel::default())?,
            _ => return Ok(()),
        };
        for (id, value) in values {
            validate_known_fields(value, &schema, &format!("channel.channels.{id}"))?;
        }
    }
    Ok(())
}

fn shape_treatment_schema() -> Result<Value> {
    let mut settings = WebShapeSettings::default();
    settings.custom_shape_path = Some(ClosedShapePath::from_polygon(&settings.custom_nodes));
    let mut schema = serde_json::to_value(settings)?;
    let object = schema
        .as_object_mut()
        .expect("Shapes settings serialization is an object");
    object.remove("channels");
    object.remove("value_mode");
    object.remove("single_channel");
    Ok(schema)
}

fn curve_treatment_schema() -> Result<Value> {
    let mut schema = serde_json::to_value(WebCurveSettings::default())?;
    let object = schema
        .as_object_mut()
        .expect("Curves settings serialization is an object");
    object.remove("channels");
    object.remove("value_mode");
    object.remove("single_channel");
    Ok(schema)
}

fn shape_channel_schema() -> Result<Value> {
    let channel = WebShapeChannel {
        custom_shape_path: Some(ClosedShapePath::from_polygon(
            &crate::model::default_shape_nodes(),
        )),
        ..Default::default()
    };
    Ok(serde_json::to_value(channel)?)
}

fn validate_known_fields(value: &Value, schema: &Value, path: &str) -> Result<()> {
    match (value, schema) {
        (Value::Object(values), Value::Object(schema)) => {
            for (key, value) in values {
                let Some(field_schema) = schema.get(key) else {
                    anyhow::bail!("Unknown field in current preset: {path}.{key}");
                };
                validate_known_fields(value, field_schema, &format!("{path}.{key}"))?;
            }
        }
        (Value::Array(values), Value::Array(schema)) => {
            if let Some(item_schema) = schema.first() {
                for (index, value) in values.iter().enumerate() {
                    validate_known_fields(value, item_schema, &format!("{path}[{index}]"))?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn treatment_kind(render: &RenderVariant) -> &'static str {
    match render {
        RenderVariant::NativeBasicV1 => KIND_NATIVE,
        RenderVariant::WebShapeV1 { .. } => KIND_SHAPES,
        RenderVariant::WebCurveV1 { .. } => KIND_CURVES,
    }
}

fn treatment_from_render(settings: Settings, render: &RenderVariant) -> Result<TreatmentSection> {
    let kind = treatment_kind(render).to_owned();
    let mut body = match render {
        RenderVariant::NativeBasicV1 => Value::Null,
        RenderVariant::WebShapeV1 { settings } => serde_json::to_value(settings)?,
        RenderVariant::WebCurveV1 { settings } => serde_json::to_value(settings)?,
    };
    if let Value::Object(object) = &mut body {
        object.remove("channels");
        // These fields are rebuilt from `Document.artwork_pipeline` and must
        // never become preset authority again.
        object.remove("value_mode");
        object.remove("single_channel");
    }
    Ok(TreatmentSection {
        kind,
        settings,
        render: body,
    })
}

fn render_from_treatment(
    treatment: &TreatmentSection,
    existing: Option<&RenderVariant>,
) -> Result<RenderVariant> {
    match treatment.kind.as_str() {
        KIND_NATIVE => {
            ensure!(
                treatment.render.is_null(),
                "Native treatment must not contain render geometry"
            );
            Ok(RenderVariant::NativeBasicV1)
        }
        KIND_SHAPES => {
            let channels = match existing {
                Some(RenderVariant::WebShapeV1 { settings }) => {
                    serde_json::to_value(&settings.channels)?
                }
                _ => serde_json::to_value(&WebShapeSettings::default().channels)?,
            };
            let settings = shape_settings_from_body(&treatment.render, channels)?;
            Ok(RenderVariant::WebShapeV1 {
                settings: Box::new(settings),
            })
        }
        KIND_CURVES => {
            let channels = match existing {
                Some(RenderVariant::WebCurveV1 { settings }) => {
                    serde_json::to_value(&settings.channels)?
                }
                _ => serde_json::to_value(&WebCurveSettings::default().channels)?,
            };
            let settings = curve_settings_from_body(&treatment.render, channels)?;
            Ok(RenderVariant::WebCurveV1 {
                settings: Box::new(settings),
            })
        }
        _ => anyhow::bail!("Unsupported treatment kind: {}", treatment.kind),
    }
}

fn body_object(body: &Value) -> Result<Map<String, Value>> {
    let object = body
        .as_object()
        .cloned()
        .context("Treatment render geometry must be an object")?;
    ensure!(
        !object.contains_key("channels")
            && !object.contains_key("value_mode")
            && !object.contains_key("single_channel"),
        "Treatment render contains renderer compatibility data"
    );
    Ok(object)
}

fn shape_settings_from_body(body: &Value, channels: Value) -> Result<WebShapeSettings> {
    let mut object = body_object(body)?;
    object.insert("channels".into(), channels);
    object.insert(
        "value_mode".into(),
        serde_json::to_value(crate::model::ValueMode::Cmyk)?,
    );
    object.insert(
        "single_channel".into(),
        serde_json::to_value(crate::model::Ink::Black)?,
    );
    serde_json::from_value(Value::Object(object)).context("Invalid Shapes treatment geometry")
}

fn curve_settings_from_body(body: &Value, channels: Value) -> Result<WebCurveSettings> {
    let mut object = body_object(body)?;
    object.insert("channels".into(), channels);
    object.insert(
        "value_mode".into(),
        serde_json::to_value(crate::model::ValueMode::Cmyk)?,
    );
    object.insert(
        "single_channel".into(),
        serde_json::to_value(crate::model::Ink::Black)?,
    );
    serde_json::from_value(Value::Object(object)).context("Invalid Curves treatment geometry")
}

fn channel_from_document(document: &Document, all_channels: bool) -> Result<ChannelSection> {
    let kind = treatment_kind(&document.render).to_owned();
    let selected: Vec<_> = if is_crosshatch_pipeline(Some(&document.artwork_pipeline)) {
        OutputChannelId::CMYK.to_vec()
    } else if all_channels {
        document.artwork_pipeline.output_model.channels().to_vec()
    } else {
        vec![
            document
                .artwork_pipeline
                .active_channel
                .unwrap_or_else(|| document.artwork_pipeline.output_model.default_channel()),
        ]
    };
    let mut channels = BTreeMap::new();
    for channel in selected {
        channels.insert(
            channel.stable_id().to_owned(),
            channel_value(&document.render, channel)?,
        );
    }
    Ok(ChannelSection {
        treatment_kind: kind,
        channels,
    })
}

fn channel_value(render: &RenderVariant, channel: OutputChannelId) -> Result<Value> {
    let ink = channel.to_legacy_ink();
    match render {
        RenderVariant::WebShapeV1 { settings } => {
            Ok(serde_json::to_value(settings.channels.get(ink))?)
        }
        RenderVariant::WebCurveV1 { settings } => {
            Ok(serde_json::to_value(settings.channels.get(ink))?)
        }
        RenderVariant::NativeBasicV1 => {
            anyhow::bail!("Native Basic does not have channel-specific treatment state")
        }
    }
}

fn validate_channel_section(section: &ChannelSection, scope: PresetScope) -> Result<()> {
    ensure!(
        matches!(section.treatment_kind.as_str(), KIND_SHAPES | KIND_CURVES),
        "Unsupported channel treatment kind: {}",
        section.treatment_kind
    );
    ensure!(
        !section.channels.is_empty(),
        "Channel preset has no channel records"
    );
    if scope == PresetScope::Channel {
        ensure!(
            section.channels.len() == 1,
            "Channel presets must contain exactly one channel record"
        );
    }
    for (id, value) in &section.channels {
        let _: OutputChannelId = id.parse().map_err(anyhow::Error::msg)?;
        match section.treatment_kind.as_str() {
            KIND_SHAPES => {
                let _: WebShapeChannel = serde_json::from_value(value.clone())
                    .context("Invalid Shapes channel state")?;
            }
            KIND_CURVES => {
                let _: WebCurveChannel = serde_json::from_value(value.clone())
                    .context("Invalid Curves channel state")?;
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}

fn apply_channel_section(document: &mut Document, section: &ChannelSection) -> Result<()> {
    ensure!(
        treatment_kind(&document.render) == section.treatment_kind,
        "Channel preset treatment kind does not match the current treatment"
    );
    for (id, value) in &section.channels {
        let channel: OutputChannelId = id.parse().map_err(anyhow::Error::msg)?;
        ensure!(
            channel.belongs_to(document.artwork_pipeline.output_model)
                || (is_crosshatch_pipeline(Some(&document.artwork_pipeline))
                    && channel.belongs_to(crate::artwork_pipeline::OutputModel::CmykPrint)),
            "Channel preset is incompatible with the current output model"
        );
        let ink = channel.to_legacy_ink();
        match &mut document.render {
            RenderVariant::WebShapeV1 { settings } => {
                *settings.channels.get_mut(ink) = serde_json::from_value(value.clone())
                    .context("Invalid Shapes channel state")?;
            }
            RenderVariant::WebCurveV1 { settings } => {
                *settings.channels.get_mut(ink) = serde_json::from_value(value.clone())
                    .context("Invalid Curves channel state")?;
            }
            RenderVariant::NativeBasicV1 => {
                anyhow::bail!("Native Basic does not have channel-specific treatment state")
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artwork_pipeline::{
        ArtworkSource, AutomaticSeparationStrategy, ChannelAssignment, LegacyBrightnessKind,
        LegacyCompatibilityAssignment, OutputModel, SourceAlphaPolicy,
    };
    use crate::model::{DocumentEditor, Ink, SourceArtwork};

    fn document() -> Document {
        Document::new(SourceArtwork {
            name: "preset-validation".into(),
            media_type: "application/octet-stream".into(),
            bytes: std::sync::Arc::from([1]),
        })
    }

    fn pipeline(output_model: OutputModel) -> ArtworkPipelineSettings {
        ArtworkPipelineSettings {
            source: ArtworkSource::FullColor,
            alpha_policy: SourceAlphaPolicy::Preserve,
            output_model,
            assignment: ChannelAssignment::automatic(match output_model {
                OutputModel::CmykPrint => AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1,
                OutputModel::RgbScreen => AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1,
            }),
            active_channel: None,
        }
    }

    #[test]
    fn every_scope_round_trips_and_only_changes_declared_state() {
        let mut source = document();
        source.artwork_pipeline = pipeline(OutputModel::RgbScreen);
        source.sync_legacy_projection().unwrap();
        let original = document();
        for scope in [
            PresetScope::Pipeline,
            PresetScope::Treatment,
            PresetScope::Channel,
            PresetScope::CompleteWorkflow,
        ] {
            let parsed = parse_treatment(
                &document_preset_bytes("Scope", &source, scope).unwrap(),
                (900, 620),
            )
            .unwrap();
            let receiving = if scope == PresetScope::Channel {
                source.clone()
            } else {
                original.clone()
            };
            let candidate = parsed.candidate_for(&receiving).unwrap();
            match scope {
                PresetScope::Pipeline => assert_pipeline_matches_except_omitted_active(
                    &candidate.artwork_pipeline,
                    &source.artwork_pipeline,
                ),
                PresetScope::Treatment => {
                    assert_eq!(candidate.artwork_pipeline, original.artwork_pipeline)
                }
                PresetScope::Channel => {
                    assert_eq!(candidate.artwork_pipeline, receiving.artwork_pipeline)
                }
                PresetScope::CompleteWorkflow => assert_pipeline_matches_except_omitted_active(
                    &candidate.artwork_pipeline,
                    &source.artwork_pipeline,
                ),
            }
        }
    }

    fn assert_pipeline_matches_except_omitted_active(
        actual: &ArtworkPipelineSettings,
        serialized: &ArtworkPipelineSettings,
    ) {
        assert_eq!(actual.source, serialized.source);
        assert_eq!(actual.alpha_policy, serialized.alpha_policy);
        assert_eq!(actual.output_model, serialized.output_model);
        assert_eq!(actual.assignment, serialized.assignment);
        if serialized.active_channel.is_some() {
            assert_eq!(actual.active_channel, serialized.active_channel);
        }
    }

    #[test]
    fn candidate_commit_is_one_undo_redo_and_parse_failure_is_inert() {
        let source = document();
        let bytes = document_preset_bytes("One", &source, PresetScope::CompleteWorkflow).unwrap();
        let parsed = parse_treatment(&bytes, (900, 620)).unwrap();
        let mut target = document();
        target.render = RenderVariant::WebCurveV1 {
            settings: Box::new(WebCurveSettings::default()),
        };
        let mut editor = DocumentEditor::new(target.clone());
        let candidate = parsed.candidate_for(editor.document()).unwrap();
        assert!(editor.replace_with_preset_candidate(candidate));
        assert!(editor.undo());
        assert_eq!(editor.document(), &target);
        assert!(editor.redo());

        let before = editor.document().clone();
        assert!(
            parse_treatment(br#"{\"format\":\"toniator-preset\",\"version\":3}"#, (1, 1)).is_err()
        );
        assert_eq!(editor.document(), &before);
    }

    #[test]
    fn semantic_cmyk_and_rgb_channel_records_are_portable() {
        let mut source = document();
        let RenderVariant::WebShapeV1 { settings } = &mut source.render else {
            unreachable!()
        };
        settings.channels.c.color = "#222222".into();
        let parsed = parse_treatment(
            &document_preset_bytes("Black", &source, PresetScope::Channel).unwrap(),
            (900, 620),
        )
        .unwrap();
        let candidate = parsed.candidate_for(&document()).unwrap();
        let RenderVariant::WebShapeV1 { settings } = candidate.render else {
            unreachable!()
        };
        assert_eq!(settings.channels.c.color, "#222222");

        source
            .apply_preset_pipeline(pipeline(OutputModel::RgbScreen))
            .unwrap();
        let RenderVariant::WebShapeV1 { settings } = &mut source.render else {
            unreachable!()
        };
        settings.channels.r.opacity = 0.37;
        let parsed = parse_treatment(
            &document_preset_bytes("Blue", &source, PresetScope::Channel).unwrap(),
            (900, 620),
        )
        .unwrap();
        let mut target = document();
        target
            .apply_preset_pipeline(pipeline(OutputModel::RgbScreen))
            .unwrap();
        let candidate = parsed.candidate_for(&target).unwrap();
        let RenderVariant::WebShapeV1 { settings } = candidate.render else {
            unreachable!()
        };
        assert_eq!(settings.channels.r.opacity, 0.37);
    }

    #[test]
    fn malformed_unknown_and_incompatible_channel_presets_reject() {
        let bytes = document_preset_bytes("Known", &document(), PresetScope::Channel).unwrap();
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        value["channel"]["channels"] = serde_json::json!({"channel.unknown": {}});
        assert!(parse_treatment(&serde_json::to_vec(&value).unwrap(), (1, 1)).is_err());

        let mut cmyk = document();
        cmyk.apply_preset_pipeline(pipeline(OutputModel::CmykPrint))
            .unwrap();
        let rgb = document_preset_bytes("RGB", &document_with_rgb(), PresetScope::Channel).unwrap();
        let parsed = parse_treatment(&rgb, (1, 1)).unwrap();
        assert!(parsed.candidate_for(&cmyk).is_err());
    }

    #[test]
    fn current_v4_rejects_unknown_nested_treatment_and_channel_fields() {
        let mut shapes = document();
        let RenderVariant::WebShapeV1 { settings } = &mut shapes.render else {
            unreachable!()
        };
        settings.custom_shape_path = Some(ClosedShapePath::from_polygon(&settings.custom_nodes));
        let shapes = document_preset_bytes("Shapes", &shapes, PresetScope::Treatment).unwrap();
        let mut unknown_settings: Value = serde_json::from_slice(&shapes).unwrap();
        unknown_settings["treatment"]["settings"]["unknown"] = serde_json::json!(1);
        assert!(parse_treatment(&serde_json::to_vec(&unknown_settings).unwrap(), (1, 1)).is_err());

        let mut unknown_custom_path: Value = serde_json::from_slice(&shapes).unwrap();
        unknown_custom_path["treatment"]["render"]["custom_shape_path"]["anchors"][0]["point"]["unknown"] =
            serde_json::json!(1);
        assert!(
            parse_treatment(&serde_json::to_vec(&unknown_custom_path).unwrap(), (1, 1)).is_err()
        );

        let curves = document_preset_bytes(
            "Curves",
            &Document {
                render: RenderVariant::WebCurveV1 {
                    settings: Box::new(WebCurveSettings::default()),
                },
                ..document()
            },
            PresetScope::Treatment,
        )
        .unwrap();
        let mut unknown_shared_path: Value = serde_json::from_slice(&curves).unwrap();
        unknown_shared_path["treatment"]["render"]["shared_path"]["start"]["unknown"] =
            serde_json::json!(1);
        assert!(
            parse_treatment(&serde_json::to_vec(&unknown_shared_path).unwrap(), (1, 1)).is_err()
        );

        let channel = document_preset_bytes("Channel", &document(), PresetScope::Channel).unwrap();
        let mut unknown_channel: Value = serde_json::from_slice(&channel).unwrap();
        unknown_channel["channel"]["channels"]["channel.cmyk.cyan"]["unknown"] =
            serde_json::json!(1);
        assert!(parse_treatment(&serde_json::to_vec(&unknown_channel).unwrap(), (1, 1)).is_err());
    }

    #[test]
    fn treatment_only_preserves_same_kind_channel_state_and_resets_cross_kind_channels() {
        let mut shape_source = document();
        let RenderVariant::WebShapeV1 { settings } = &mut shape_source.render else {
            unreachable!()
        };
        settings.grid_scale = 71.0;
        let shape_treatment = parse_treatment(
            &document_preset_bytes("Shape", &shape_source, PresetScope::Treatment).unwrap(),
            (900, 620),
        )
        .unwrap();
        let mut shape_target = document();
        let RenderVariant::WebShapeV1 { settings } = &mut shape_target.render else {
            unreachable!()
        };
        settings.channels.c.color = "#123456".into();
        let shape_candidate = shape_treatment.candidate_for(&shape_target).unwrap();
        let RenderVariant::WebShapeV1 { settings } = shape_candidate.render else {
            unreachable!()
        };
        assert_eq!(settings.grid_scale, 71.0);
        assert_eq!(settings.channels.c.color, "#123456");

        let mut curve_source = document();
        curve_source.render = RenderVariant::WebCurveV1 {
            settings: Box::new(WebCurveSettings {
                long_edge_cells: 73.0,
                ..Default::default()
            }),
        };
        let curve_treatment = parse_treatment(
            &document_preset_bytes("Curve", &curve_source, PresetScope::Treatment).unwrap(),
            (900, 620),
        )
        .unwrap();
        let mut curve_target = curve_source.clone();
        let RenderVariant::WebCurveV1 { settings } = &mut curve_target.render else {
            unreachable!()
        };
        settings.channels.c.curve_scale = 77.0;
        let curve_candidate = curve_treatment.candidate_for(&curve_target).unwrap();
        let RenderVariant::WebCurveV1 { settings } = curve_candidate.render else {
            unreachable!()
        };
        assert_eq!(settings.long_edge_cells, 73.0);
        assert_eq!(settings.channels.c.curve_scale, 77.0);

        let RenderVariant::WebCurveV1 { settings } = &mut curve_source.render else {
            unreachable!()
        };
        settings.channels.c.color = "#123456".into();
        let cross_kind = parse_treatment(
            &document_preset_bytes("Curve", &curve_source, PresetScope::Treatment).unwrap(),
            (900, 620),
        )
        .unwrap()
        .candidate_for(&document())
        .unwrap();
        let RenderVariant::WebCurveV1 { settings } = cross_kind.render else {
            unreachable!()
        };
        assert_eq!(
            settings.channels.c.color,
            WebCurveSettings::default().channels.c.color,
            "cross-kind Treatment imports intentionally reset target channel maps to current defaults"
        );
    }

    #[test]
    fn pipeline_without_active_channel_uses_the_output_transition_rule() {
        let source =
            document_preset_bytes("RGB pipeline", &document_with_rgb(), PresetScope::Pipeline)
                .unwrap();
        let mut value: Value = serde_json::from_slice(&source).unwrap();
        value["pipeline"]
            .as_object_mut()
            .unwrap()
            .remove("active_channel");
        let parsed = parse_treatment(&serde_json::to_vec(&value).unwrap(), (1, 1)).unwrap();
        let candidate = parsed.candidate_for(&document()).unwrap();
        assert_eq!(
            candidate.artwork_pipeline.output_model,
            OutputModel::RgbScreen
        );
        assert_eq!(
            candidate.artwork_pipeline.active_channel,
            Some(OutputChannelId::RgbRed)
        );
    }

    #[test]
    fn omitted_active_channel_preserves_or_transitions_semantic_destinations() {
        let omitted_active = |pipeline: ArtworkPipelineSettings| {
            let mut source = document();
            source.apply_preset_pipeline(pipeline).unwrap();
            let mut value: Value = serde_json::from_slice(
                &document_preset_bytes("Pipeline", &source, PresetScope::Pipeline).unwrap(),
            )
            .unwrap();
            value["pipeline"]
                .as_object_mut()
                .unwrap()
                .remove("active_channel");
            parse_treatment(&serde_json::to_vec(&value).unwrap(), (1, 1)).unwrap()
        };
        let scalar = |output_model, active_channel| ArtworkPipelineSettings {
            source: ArtworkSource::Value,
            alpha_policy: SourceAlphaPolicy::Preserve,
            output_model,
            assignment: ChannelAssignment::ActiveChannel,
            active_channel: Some(active_channel),
        };

        let cmyk_to_rgb = omitted_active(scalar(OutputModel::RgbScreen, OutputChannelId::RgbRed));
        let mut cmyk_target = document();
        cmyk_target
            .apply_preset_pipeline(scalar(OutputModel::CmykPrint, OutputChannelId::CmykBlack))
            .unwrap();
        assert_eq!(
            cmyk_to_rgb
                .candidate_for(&cmyk_target)
                .unwrap()
                .artwork_pipeline
                .active_channel,
            Some(OutputChannelId::RgbRed),
            "CMYK Black has no RGB slot and follows the existing RGB default transition"
        );

        let rgb_to_cmyk = omitted_active(scalar(OutputModel::CmykPrint, OutputChannelId::CmykCyan));
        let mut rgb_target = document();
        rgb_target
            .apply_preset_pipeline(scalar(OutputModel::RgbScreen, OutputChannelId::RgbBlue))
            .unwrap();
        assert_eq!(
            rgb_to_cmyk
                .candidate_for(&rgb_target)
                .unwrap()
                .artwork_pipeline
                .active_channel,
            Some(OutputChannelId::CmykYellow),
            "RGB Blue retains its semantic legacy slot when CMYK is available"
        );

        let same_model = omitted_active(scalar(OutputModel::CmykPrint, OutputChannelId::CmykCyan));
        assert_eq!(
            same_model
                .candidate_for(&cmyk_target)
                .unwrap()
                .artwork_pipeline
                .active_channel,
            Some(OutputChannelId::CmykBlack),
            "same-model omitted active channel preserves the receiving active destination"
        );
    }

    #[test]
    fn treatment_bytes_are_deterministic_and_do_not_mutate_save_source() {
        let source = document();
        let before = source.clone();
        let first = document_preset_bytes("Stable", &source, PresetScope::Treatment).unwrap();
        let second = document_preset_bytes("Stable", &source, PresetScope::Treatment).unwrap();
        assert_eq!(first, second);
        assert_eq!(source, before);
        let value: Value = serde_json::from_slice(&first).unwrap();
        assert!(value["treatment"]["render"].get("channels").is_none());
        assert!(value["treatment"]["render"].get("value_mode").is_none());
    }

    #[test]
    fn every_runtime_bundled_preset_is_current_and_applicable() {
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
            assert_eq!(parsed.scope, PresetScope::CompleteWorkflow);
            parsed
                .candidate_for(&document())
                .unwrap_or_else(|error| panic!("{name} did not apply: {error:#}"));
        }
    }

    #[test]
    fn complete_crosshatch_keeps_its_curves_transition_representation() {
        let mut source = document();
        source.artwork_pipeline = ArtworkPipelineSettings {
            source: ArtworkSource::LegacyBrightness(LegacyBrightnessKind::EncodedRec709InvertedV1),
            alpha_policy: SourceAlphaPolicy::LegacyCurrentV1,
            output_model: OutputModel::RgbScreen,
            assignment: ChannelAssignment::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1,
            ),
            active_channel: None,
        };
        let mut curves = WebCurveSettings::default();
        curves.configure_crosshatch();
        curves.channels.k.opacity = 0.51;
        source.render = RenderVariant::WebCurveV1 {
            settings: Box::new(curves),
        };
        source.sync_legacy_projection().unwrap();
        let parsed = parse_treatment(
            &document_preset_bytes("Crosshatch", &source, PresetScope::CompleteWorkflow).unwrap(),
            (900, 620),
        )
        .unwrap();
        let candidate = parsed.candidate_for(&document()).unwrap();
        let RenderVariant::WebCurveV1 { settings } = &candidate.render else {
            unreachable!()
        };
        assert_eq!(settings.channels.k.opacity, 0.51);
        assert_eq!(candidate.artwork_pipeline, source.artwork_pipeline);

        assert!(document_preset_bytes("Crosshatch", &source, PresetScope::Channel).is_err());
        let mut pipeline_only: Value = serde_json::from_slice(
            &document_preset_bytes("Crosshatch", &source, PresetScope::CompleteWorkflow).unwrap(),
        )
        .unwrap();
        pipeline_only["scope"] = serde_json::json!("pipeline");
        pipeline_only.as_object_mut().unwrap().remove("treatment");
        pipeline_only.as_object_mut().unwrap().remove("channel");
        assert!(parse_treatment(&serde_json::to_vec(&pipeline_only).unwrap(), (900, 620)).is_err());
    }

    fn document_with_rgb() -> Document {
        let mut document = document();
        document
            .apply_preset_pipeline(pipeline(OutputModel::RgbScreen))
            .unwrap();
        let RenderVariant::WebShapeV1 { settings } = &mut document.render else {
            unreachable!()
        };
        settings.channels.get_mut(Ink::Blue).enabled = true;
        document
    }
}
