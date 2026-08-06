//! One-way bridge from authoritative Curves state to the immutable recipe contract.

use crate::model::parse_hex_color;
use crate::{
    Document, EmbeddedSvgAsset, LiteralValue, OutputChannelId, PatternDefinition,
    PatternInstanceParameters, PatternInstanceValue, WebCurveChannel, WebCurveSettings,
    load_bundled_curves_definition,
};
use anyhow::Result;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq)]
pub struct CurvesRecipeAdaptation {
    pub definition: PatternDefinition,
    pub instance: PatternInstanceParameters,
}

/// Adapts current Curves semantic state only. Legacy value mode, single ink,
/// Crosshatch color, and artwork-pipeline assignment are external authority;
/// `base_channel` is inspector-only. `show_background` and `tile_spacing`
/// remain retained compatibility state with no current render/export consumer,
/// so they deliberately do not enter the recipe pending later v9 cleanup.
pub fn adapt_curves_settings_to_recipe(
    settings: &WebCurveSettings,
) -> Result<CurvesRecipeAdaptation> {
    validate_settings_semantics(settings)?;
    let mut definition = load_bundled_curves_definition()?;
    let mut instance = definition.default_instance_parameters(
        OutputChannelId::CMYK
            .into_iter()
            .chain(OutputChannelId::RGB),
    )?;
    let shared = insert_curve_asset(&mut definition, &settings.shared_path)?;
    instance.pattern_values = vec![
        v(
            "output-width",
            LiteralValue::Integer(settings.output_width.into()),
        ),
        v(
            "output-height",
            LiteralValue::Integer(settings.output_height.into()),
        ),
        v(
            "long-edge-cells",
            LiteralValue::Number(settings.long_edge_cells),
        ),
        v("min-mark", LiteralValue::Number(settings.min_mark)),
        v("max-mark", LiteralValue::Number(settings.max_mark)),
        v(
            "layout",
            LiteralValue::Choice(layout_id(settings.layout).into()),
        ),
        v(
            "use-shared-curve",
            LiteralValue::Boolean(settings.use_shared_curve),
        ),
        v("shared-path", LiteralValue::SvgAsset(shared)),
        v(
            "shared-close-ends",
            LiteralValue::Boolean(settings.shared_close_ends),
        ),
        v(
            "shared-smooth-join",
            LiteralValue::Boolean(settings.shared_smooth_join),
        ),
    ];
    for output in &mut instance.output_channel_values {
        let channel = settings
            .channels
            .get(output.channel.parse::<OutputChannelId>()?.to_legacy_ink());
        output.values = channel_values(&mut definition, channel)?;
    }
    definition.validate_instance_parameters(&instance)?;
    Ok(CurvesRecipeAdaptation {
        definition,
        instance,
    })
}

fn validate_settings_semantics(settings: &WebCurveSettings) -> Result<()> {
    anyhow::ensure!(
        settings.max_mark >= settings.min_mark,
        "curve maximum width must not be less than minimum width"
    );
    Ok(())
}

pub fn adapt_document_curves_to_recipe(document: &Document) -> Result<CurvesRecipeAdaptation> {
    adapt_curves_settings_to_recipe(&document.pattern_state.curve_settings()?)
}

fn channel_values(
    definition: &mut PatternDefinition,
    channel: &WebCurveChannel,
) -> Result<Vec<PatternInstanceValue>> {
    validate_channel_semantics(channel)?;
    let path = insert_curve_asset(definition, &channel.path)?;
    Ok(vec![
        v("enabled", LiteralValue::Boolean(channel.enabled)),
        v("color", LiteralValue::Text(channel.color.clone())),
        n("grid-rotation", channel.grid_rotation),
        n("grid-pivot-x", channel.grid_pivot_x),
        n("grid-pivot-y", channel.grid_pivot_y),
        n("resolution-scale", channel.resolution_scale),
        n("offset-x", channel.offset_x),
        n("offset-y", channel.offset_y),
        v("path", LiteralValue::SvgAsset(path)),
        v("close-ends", LiteralValue::Boolean(channel.close_ends)),
        v("smooth-join", LiteralValue::Boolean(channel.smooth_join)),
        n("curve-scale", channel.curve_scale),
        v(
            "motif-coverage",
            LiteralValue::Choice(coverage_id(channel.motif_coverage).into()),
        ),
        n("motif-bleed", channel.motif_bleed),
        i("tile-count", channel.tile_count),
        n("tile-angle", channel.tile_angle),
        n("tile-offset", channel.tile_offset),
        i("stack-count", channel.stack_count),
        n("stack-spacing", channel.stack_spacing),
        n("stack-angle", channel.stack_angle),
        n("stack-offset", channel.stack_offset),
        n("alternate-stack-offset", channel.alternate_stack_offset),
        v(
            "alternate-tile-transform",
            LiteralValue::Choice(transform_id(channel.alternate_tile_transform).into()),
        ),
        n("scale", channel.scale),
        n("threshold", channel.threshold),
        n("max-size", channel.max_size),
        n("output-quality", channel.output_quality),
        n("opacity", channel.opacity),
    ])
}

fn validate_channel_semantics(channel: &WebCurveChannel) -> Result<()> {
    anyhow::ensure!(
        parse_hex_color(&channel.color).is_some(),
        "curve channel color must be a #rrggbb hexadecimal color"
    );
    anyhow::ensure!(
        channel.resolution_scale > 0.0 && channel.resolution_scale <= 100.0,
        "curve channel resolution scale must be in (0, 100]"
    );
    anyhow::ensure!(
        channel.output_quality > 0.0 && channel.output_quality <= 100.0,
        "curve channel output quality must be in (0, 100]"
    );
    Ok(())
}
fn v(key: &str, value: LiteralValue) -> PatternInstanceValue {
    PatternInstanceValue {
        key: key.into(),
        value,
    }
}
fn n(key: &str, value: f64) -> PatternInstanceValue {
    v(key, LiteralValue::Number(value))
}
fn i(key: &str, value: u32) -> PatternInstanceValue {
    v(key, LiteralValue::Integer(value.into()))
}
fn layout_id(value: crate::CurveLayout) -> &'static str {
    match value {
        crate::CurveLayout::FullWidth => "full-width",
        crate::CurveLayout::MotifPattern => "motif-pattern",
    }
}
fn coverage_id(value: crate::MotifCoverage) -> &'static str {
    match value {
        crate::MotifCoverage::Auto => "auto",
        crate::MotifCoverage::Manual => "manual",
    }
}
fn transform_id(value: crate::AlternateTileTransform) -> &'static str {
    match value {
        crate::AlternateTileTransform::None => "none",
        crate::AlternateTileTransform::Flip => "flip",
        crate::AlternateTileTransform::Rotate180 => "rotate-180",
    }
}

fn insert_curve_asset(
    definition: &mut PatternDefinition,
    path: &crate::CurvePath,
) -> Result<String> {
    anyhow::ensure!(
        (1..=64).contains(&path.segments.len())
            && path
                .points()
                .all(|point| point.x.is_finite() && point.y.is_finite()),
        "curve motif path must contain between 1 and 64 finite cubic segments"
    );
    let mut d = format!("M {} {}", path.start.x, path.start.y);
    for segment in &path.segments {
        d.push_str(&format!(
            " C {} {}, {} {}, {} {}",
            segment.control_1.x,
            segment.control_1.y,
            segment.control_2.x,
            segment.control_2.y,
            segment.end.x,
            segment.end.y
        ));
    }
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-0.5 -0.5 1 1\"><path d=\"{d}\"/></svg>"
    );
    let digest = format!("sha256:{:x}", Sha256::digest(svg.as_bytes()));
    if definition.assets.iter().all(|asset| asset.digest != digest) {
        definition.assets.push(EmbeddedSvgAsset {
            digest: digest.clone(),
            svg,
        });
    }
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DocumentEditor, PatternParameterConstraints, RecipeArgument, RenderVariant, SourceArtwork,
    };
    use std::collections::BTreeSet;
    use std::sync::Arc;

    #[test]
    fn adapter_covers_the_complete_curves_recipe_contract() {
        let adaptation = adapt_curves_settings_to_recipe(&WebCurveSettings::default()).unwrap();
        adaptation
            .definition
            .validate_instance_parameters(&adaptation.instance)
            .unwrap();
        assert_eq!(adaptation.instance.pattern_values.len(), 10);
        assert_eq!(adaptation.instance.output_channel_values.len(), 7);
        assert!(
            adaptation
                .instance
                .output_channel_values
                .iter()
                .all(|channel| channel.values.len() == 28)
        );
        let declared = adaptation
            .definition
            .parameters
            .iter()
            .map(|parameter| parameter.key.as_str())
            .collect::<BTreeSet<_>>();
        let adapted = adaptation
            .instance
            .pattern_values
            .iter()
            .map(|value| value.key.as_str())
            .chain(
                adaptation
                    .instance
                    .output_channel_values
                    .iter()
                    .flat_map(|channel| channel.values.iter().map(|value| value.key.as_str())),
            )
            .collect::<BTreeSet<_>>();
        assert_eq!(adapted, declared);
    }

    #[test]
    fn recipe_ownership_matches_retained_curves_dataflow() {
        let definition = load_bundled_curves_definition().unwrap();
        let node_parameters = |node_id: &str| {
            definition
                .recipe
                .nodes
                .iter()
                .find(|node| node.id == node_id)
                .unwrap()
                .parameters
                .iter()
                .filter_map(|(key, argument)| {
                    matches!(argument, RecipeArgument::Parameter(_)).then_some(key.as_str())
                })
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(
            node_parameters("motif"),
            BTreeSet::from([
                "use-shared-curve",
                "shared-path",
                "shared-close-ends",
                "shared-smooth-join",
                "path",
                "close-ends",
                "smooth-join",
            ])
        );
        assert_eq!(
            node_parameters("deform"),
            BTreeSet::from([
                "layout",
                "curve-scale",
                "motif-coverage",
                "motif-bleed",
                "tile-count",
                "tile-angle",
                "tile-offset",
                "stack-count",
                "stack-spacing",
                "stack-angle",
                "stack-offset",
                "alternate-stack-offset",
                "alternate-tile-transform",
                // `motif_counts` calls `max_curve_width` for automatic
                // coverage guards before modulation creates final widths.
                "min-mark",
                "max-mark",
                "max-size",
                "scale",
                "output-quality",
            ])
        );
        assert_eq!(
            node_parameters("modulate"),
            BTreeSet::from([
                "min-mark",
                "max-mark",
                "threshold",
                "max-size",
                "scale",
                "output-quality",
            ])
        );
        assert_eq!(
            definition
                .recipe
                .nodes
                .iter()
                .filter(|node| node.parameters.contains_key("output-quality"))
                .map(|node| node.id.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["deform", "modulate"])
        );
        for parameter in ["min-mark", "max-mark", "max-size", "scale"] {
            assert_eq!(
                definition
                    .recipe
                    .nodes
                    .iter()
                    .filter(|node| node.parameters.contains_key(parameter))
                    .map(|node| node.id.as_str())
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["deform", "modulate"]),
                "{parameter} must drive automatic coverage guards and width modulation",
            );
        }
        let declared = definition
            .parameters
            .iter()
            .map(|parameter| parameter.key.as_str())
            .collect::<BTreeSet<_>>();
        let consumed = definition
            .recipe
            .nodes
            .iter()
            .flat_map(|node| node.parameters.values())
            .filter_map(|argument| match argument {
                RecipeArgument::Parameter(key) => Some(key.as_str()),
                RecipeArgument::Literal(_) => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(consumed, declared);
        for no_op in ["tile-spacing", "show-background"] {
            assert!(!declared.contains(no_op));
            assert!(definition.layout.sections.iter().all(|section| {
                !section
                    .parameters
                    .iter()
                    .any(|parameter| parameter == no_op)
            }));
            assert!(
                definition
                    .recipe
                    .nodes
                    .iter()
                    .all(|node| !node.parameters.contains_key(no_op))
            );
        }
    }

    #[test]
    fn document_adapter_reads_pattern_state_not_transient_render_settings() {
        let source = SourceArtwork {
            name: "test.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from([]),
        };
        let mut editor = DocumentEditor::new(Document::new(source));
        let authoritative = WebCurveSettings {
            output_width: 1_234,
            ..WebCurveSettings::default()
        };
        assert!(editor.set_curve_settings(authoritative));
        let mut document = editor.document().clone();
        document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(WebCurveSettings::default()),
        };
        let adaptation = adapt_document_curves_to_recipe(&document).unwrap();
        assert!(adaptation.instance.pattern_values.iter().any(|value| {
            value.key == "output-width" && value.value == LiteralValue::Integer(1_234)
        }));
    }

    #[test]
    fn adapter_accepts_current_curve_boundaries_and_declares_them() {
        let mut settings = WebCurveSettings::default();
        let channel = &mut settings.channels.c;
        channel.max_size = 10_000.0;
        channel.curve_scale = 0.1;
        channel.motif_bleed = 0.0;
        channel.tile_count = 10_000;
        channel.tile_spacing = -10_000.0;
        channel.stack_count = 10_000;
        channel.stack_spacing = 10_000.0;
        channel.resolution_scale = f64::from_bits(1);
        channel.output_quality = f64::from_bits(1);
        channel.grid_rotation = f64::MAX;
        channel.tile_offset = f64::MIN;
        settings.channels.m.curve_scale = 500.0;
        settings.channels.m.motif_bleed = 100.0;
        assert!(adapt_curves_settings_to_recipe(&settings).is_ok());

        let definition = load_bundled_curves_definition().unwrap();
        for (key, minimum, maximum) in [
            ("max-size", 0.0, 10_000.0),
            ("curve-scale", 0.1, 500.0),
            ("motif-bleed", 0.0, 100.0),
            ("stack-spacing", -10_000.0, 10_000.0),
            ("resolution-scale", 0.0, 100.0),
            ("output-quality", 0.0, 100.0),
        ] {
            let parameter = definition
                .parameters
                .iter()
                .find(|parameter| parameter.key == key)
                .unwrap();
            assert_eq!(
                parameter.constraints,
                PatternParameterConstraints::Number {
                    minimum,
                    maximum,
                    step: match key {
                        "curve-scale" | "motif-bleed" | "max-size" | "stack-spacing" => {
                            0.001
                        }
                        "resolution-scale" | "output-quality" => 0.000001,
                        _ => unreachable!(),
                    },
                }
            );
        }
        for key in ["tile-count", "stack-count"] {
            let parameter = definition
                .parameters
                .iter()
                .find(|parameter| parameter.key == key)
                .unwrap();
            assert_eq!(
                parameter.constraints,
                PatternParameterConstraints::Integer {
                    minimum: 1,
                    maximum: 10_000,
                    step: 1,
                }
            );
        }
        for key in [
            "grid-rotation",
            "grid-pivot-x",
            "grid-pivot-y",
            "offset-x",
            "offset-y",
            "tile-angle",
            "tile-offset",
            "stack-angle",
            "stack-offset",
            "alternate-stack-offset",
        ] {
            let parameter = definition
                .parameters
                .iter()
                .find(|parameter| parameter.key == key)
                .unwrap();
            assert!(matches!(
                parameter.constraints,
                PatternParameterConstraints::Number {
                    minimum,
                    maximum,
                    ..
                } if minimum == f64::MIN && maximum == f64::MAX
            ));
        }
    }

    #[test]
    fn adapter_rejects_invalid_curve_paths_and_channel_colors() {
        let mut oversized = WebCurveSettings::default();
        let segment = oversized.shared_path.segments[0];
        oversized.shared_path.segments = vec![segment; 65];
        assert!(
            adapt_curves_settings_to_recipe(&oversized)
                .unwrap_err()
                .to_string()
                .contains("between 1 and 64")
        );

        let mut invalid_color = WebCurveSettings::default();
        invalid_color.channels.c.color = "#12gg56".into();
        assert!(
            adapt_curves_settings_to_recipe(&invalid_color)
                .unwrap_err()
                .to_string()
                .contains("#rrggbb")
        );

        let mut zero_quality = WebCurveSettings::default();
        zero_quality.channels.c.output_quality = 0.0;
        assert!(
            adapt_curves_settings_to_recipe(&zero_quality)
                .unwrap_err()
                .to_string()
                .contains("(0, 100]")
        );
    }
}
