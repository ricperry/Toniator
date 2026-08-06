//! One-way bridge from authoritative compatibility Shapes settings to the
//! strict data-only Shapes recipe contract. No live renderer calls this module
//! during 3D1.

use crate::{
    Document, EmbeddedSvgAsset, LiteralValue, OutputChannelId, PatternDefinition,
    PatternInstanceParameters, PatternInstanceValue, WebShape, WebShapeChannel, WebShapeSettings,
    load_bundled_shapes_definition,
};
use anyhow::Result;
use sha2::{Digest, Sha256};

/// A strict instance plus the transient derived definition required to carry
/// digest-identified editable custom motifs. The immutable bundled definition
/// is never changed; derived assets exist only in this adaptation result.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapesRecipeAdaptation {
    pub definition: PatternDefinition,
    pub instance: PatternInstanceParameters,
}

/// Adapts current authoritative Shapes state one-way. Pipeline `value_mode`
/// and `single_channel` are intentionally absent: they are artwork-pipeline
/// authority, not persisted Shapes parameters.
pub fn adapt_shapes_settings_to_recipe(
    settings: &WebShapeSettings,
) -> Result<ShapesRecipeAdaptation> {
    let mut definition = load_bundled_shapes_definition()?;
    let mut instance = definition.default_instance_parameters(
        OutputChannelId::CMYK
            .into_iter()
            .chain(OutputChannelId::RGB),
    )?;
    let global_path = settings.resolved_custom_shape_path();
    crate::model::validate_shape_path(&global_path)?;
    let global_motif = insert_path_asset(&mut definition, &global_path);
    instance.pattern_values = vec![
        value(
            "output-width",
            LiteralValue::Integer(u64::from(settings.output_width)),
        ),
        value(
            "output-height",
            LiteralValue::Integer(u64::from(settings.output_height)),
        ),
        value(
            "long-edge-cells",
            LiteralValue::Number(settings.long_edge_cells),
        ),
        value("grid-scale", LiteralValue::Number(settings.grid_scale)),
        value("min-mark", LiteralValue::Number(settings.min_mark)),
        value("max-mark", LiteralValue::Number(settings.max_mark)),
        value(
            "use-shared-mark",
            LiteralValue::Boolean(settings.use_shared_mark),
        ),
        value(
            "shared-shape",
            LiteralValue::Choice(shape_id(settings.shared_shape).into()),
        ),
        value(
            "polygon-sides",
            LiteralValue::Integer(u64::from(settings.polygon_sides)),
        ),
        value("global-custom-motif", LiteralValue::SvgAsset(global_motif)),
    ];
    for channel_values in &mut instance.output_channel_values {
        let channel = channel_values.channel.parse::<OutputChannelId>()?;
        let shape = settings.channels.get(channel.to_legacy_ink());
        let channel_path = settings.resolved_channel_shape_path(shape);
        crate::model::validate_shape_path(&channel_path)?;
        let motif = insert_path_asset(&mut definition, &channel_path);
        channel_values.values = channel_values_for(shape, motif);
    }
    definition.validate_instance_parameters(&instance)?;
    Ok(ShapesRecipeAdaptation {
        definition,
        instance,
    })
}

/// Reads persisted Shapes state without consulting the transient renderer
/// compatibility adapter.
pub fn adapt_document_shapes_to_recipe(document: &Document) -> Result<ShapesRecipeAdaptation> {
    adapt_shapes_settings_to_recipe(&document.pattern_state.shape_settings()?)
}

fn channel_values_for(channel: &WebShapeChannel, motif: String) -> Vec<PatternInstanceValue> {
    vec![
        value("enabled", LiteralValue::Boolean(channel.enabled)),
        value("color", LiteralValue::Text(channel.color.clone())),
        value("rotation", LiteralValue::Number(channel.rotation)),
        value("grid-rotation", LiteralValue::Number(channel.grid_rotation)),
        value("grid-pivot-x", LiteralValue::Number(channel.grid_pivot_x)),
        value("grid-pivot-y", LiteralValue::Number(channel.grid_pivot_y)),
        value("scale", LiteralValue::Number(channel.scale)),
        value("width-scale", LiteralValue::Number(channel.width_scale)),
        value("height-scale", LiteralValue::Number(channel.height_scale)),
        value("threshold", LiteralValue::Number(channel.threshold)),
        value("max-size", LiteralValue::Number(channel.max_size)),
        value(
            "resolution-scale",
            LiteralValue::Number(channel.resolution_scale),
        ),
        value(
            "random-size-response",
            LiteralValue::Number(channel.random_size_response),
        ),
        value("offset-x", LiteralValue::Number(channel.offset_x)),
        value("offset-y", LiteralValue::Number(channel.offset_y)),
        value("opacity", LiteralValue::Number(channel.opacity)),
        value(
            "shape",
            LiteralValue::Choice(shape_id(channel.shape).into()),
        ),
        value(
            "channel-polygon-sides",
            LiteralValue::Integer(u64::from(channel.polygon_sides)),
        ),
        value("channel-custom-motif", LiteralValue::SvgAsset(motif)),
    ]
}

fn value(key: &str, value: LiteralValue) -> PatternInstanceValue {
    PatternInstanceValue {
        key: key.into(),
        value,
    }
}

fn shape_id(shape: WebShape) -> &'static str {
    match shape {
        WebShape::Circle => "circle",
        WebShape::RegularPolygon => "regular-polygon",
        WebShape::UserDefined => "user-defined",
        WebShape::Rectangle => "rectangle",
        WebShape::Triangle => "triangle",
        WebShape::Pentagon => "pentagon",
        WebShape::Hexagon => "hexagon",
    }
}

fn insert_path_asset(
    definition: &mut PatternDefinition,
    path: &crate::model::ClosedShapePath,
) -> String {
    let svg = path_svg(path);
    let digest = format!("sha256:{:x}", Sha256::digest(svg.as_bytes()));
    if definition.assets.iter().all(|asset| asset.digest != digest) {
        definition.assets.push(EmbeddedSvgAsset {
            digest: digest.clone(),
            svg,
        });
    }
    digest
}

fn path_svg(path: &crate::model::ClosedShapePath) -> String {
    let first = path
        .anchors
        .first()
        .expect("validated Shapes path is non-empty");
    let mut d = format!("M {} {}", first.point.x, first.point.y);
    for (index, anchor) in path.anchors.iter().enumerate() {
        let next = &path.anchors[(index + 1) % path.anchors.len()];
        d.push_str(&format!(
            " C {} {}, {} {}, {} {}",
            anchor.outgoing.x,
            anchor.outgoing.y,
            next.incoming.x,
            next.incoming.y,
            next.point.x,
            next.point.y,
        ));
    }
    d.push_str(" Z");
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-0.5 -0.5 1 1\"><path d=\"{d}\"/></svg>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ClosedShapePath, ShapeAnchor, ShapePoint};

    #[test]
    fn adapter_is_deterministic_strict_and_keeps_custom_paths_as_digest_assets() {
        let path = ClosedShapePath {
            anchors: vec![
                ShapeAnchor {
                    point: ShapePoint { x: -0.4, y: -0.4 },
                    incoming: ShapePoint { x: -0.4, y: -0.4 },
                    outgoing: ShapePoint { x: 0.0, y: -0.6 },
                },
                ShapeAnchor {
                    point: ShapePoint { x: 0.5, y: -0.2 },
                    incoming: ShapePoint { x: 0.1, y: -0.5 },
                    outgoing: ShapePoint { x: 0.6, y: 0.1 },
                },
                ShapeAnchor {
                    point: ShapePoint { x: 0.0, y: 0.5 },
                    incoming: ShapePoint { x: 0.3, y: 0.3 },
                    outgoing: ShapePoint { x: -0.3, y: 0.6 },
                },
            ],
        };
        let mut settings = WebShapeSettings {
            shared_shape: WebShape::UserDefined,
            custom_shape_path: Some(path.clone()),
            ..WebShapeSettings::default()
        };
        settings.channels.r.shape = WebShape::UserDefined;
        settings.channels.r.custom_shape_path = Some(path);
        let first = adapt_shapes_settings_to_recipe(&settings).unwrap();
        let second = adapt_shapes_settings_to_recipe(&settings).unwrap();
        assert_eq!(first, second);
        first
            .definition
            .validate_instance_parameters(&first.instance)
            .unwrap();
        let global = first
            .instance
            .pattern_values
            .iter()
            .find(|value| value.key == "global-custom-motif")
            .unwrap();
        let LiteralValue::SvgAsset(digest) = &global.value else {
            panic!("custom motif must be an SVG asset")
        };
        assert!(
            first
                .definition
                .assets
                .iter()
                .any(|asset| &asset.digest == digest)
        );
        assert!(
            first
                .definition
                .assets
                .iter()
                .all(|asset| asset.svg.contains("<svg"))
        );
    }

    #[test]
    fn adapter_maps_defaults_enums_and_transforms_without_pipeline_authority() {
        let mut settings = WebShapeSettings {
            long_edge_cells: 73.5,
            use_shared_mark: false,
            shared_shape: WebShape::Hexagon,
            ..WebShapeSettings::default()
        };
        settings.channels.b.enabled = false;
        settings.channels.b.shape = WebShape::Triangle;
        settings.channels.b.rotation = 17.25;
        settings.channels.b.grid_rotation = -32.5;
        settings.channels.b.grid_pivot_x = 123.0;
        settings.channels.b.grid_pivot_y = -45.0;
        settings.channels.b.width_scale = 1.5;
        settings.channels.b.height_scale = 0.75;
        settings.channels.b.offset_x = 0.125;
        settings.channels.b.offset_y = -0.25;
        let adaptation = adapt_shapes_settings_to_recipe(&settings).unwrap();
        assert_eq!(adaptation.instance.pattern_values.len(), 10);
        let blue = adaptation
            .instance
            .output_channel_values
            .iter()
            .find(|values| values.channel == OutputChannelId::RgbBlue.stable_id())
            .unwrap();
        assert_eq!(blue.values.len(), 19);
        assert!(
            blue.values
                .contains(&value("enabled", LiteralValue::Boolean(false)))
        );
        assert!(
            blue.values
                .contains(&value("shape", LiteralValue::Choice("triangle".into())))
        );
        assert!(
            blue.values
                .contains(&value("rotation", LiteralValue::Number(17.25)))
        );
        assert!(!adaptation.instance.pattern_values.iter().any(|entry| {
            entry.key == "value-mode"
                || entry.key == "single-channel"
                || entry.key == "crosshatch-color"
        }));
    }

    #[test]
    fn adapter_rejects_malformed_resolved_global_and_channel_paths() {
        let mut malformed_global = WebShapeSettings::default().resolved_custom_shape_path();
        malformed_global.anchors[0].outgoing.x = f64::NAN;
        let settings = WebShapeSettings {
            custom_shape_path: Some(malformed_global),
            ..WebShapeSettings::default()
        };
        assert!(adapt_shapes_settings_to_recipe(&settings).is_err());

        let mut malformed_channel = WebShapeSettings::default().resolved_custom_shape_path();
        malformed_channel.anchors[1].incoming.y = f64::INFINITY;
        let mut settings = WebShapeSettings::default();
        settings.channels.r.custom_shape_path = Some(malformed_channel);
        assert!(adapt_shapes_settings_to_recipe(&settings).is_err());
    }
}
