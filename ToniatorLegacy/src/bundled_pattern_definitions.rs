//! Immutable compile-time bundled `.tnpattern` definitions.
//!
//! This module intentionally has no filesystem, XDG, import, legacy metadata,
//! execution, or renderer fallback path. Every bundled byte sequence crosses
//! the same strict parser and registry used by future imported definitions.

use crate::{
    PatternDefinition, PatternDefinitionError, PatternDefinitionRegistry,
    PatternDefinitionRegistryError, parse_tnpattern,
};
use std::fmt;

pub const WEIGHTED_VORONOI_BUNDLED_BYTES: &[u8] =
    include_bytes!("../assets/patterns/weighted-voronoi.v1.tnpattern");
pub const SHAPES_BUNDLED_BYTES: &[u8] =
    include_bytes!("../assets/patterns/compat-shapes.v1.tnpattern");
pub const CURVES_BUNDLED_BYTES: &[u8] =
    include_bytes!("../assets/patterns/compat-curves.v1.tnpattern");
pub const QUADRATIC_RADIAL_SPIRAL_BUNDLED_BYTES: &[u8] =
    include_bytes!("../assets/patterns/quadratic-radial-spiral.v1.tnpattern");
pub const WAVE_LINE_FIELD_BUNDLED_BYTES: &[u8] =
    include_bytes!("../assets/patterns/wave-line-field.v1.tnpattern");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundledPatternDefinitionError {
    Parse {
        bundle: &'static str,
        message: String,
    },
    Registry(PatternDefinitionRegistryError),
}

impl fmt::Display for BundledPatternDefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { bundle, message } => {
                write!(
                    formatter,
                    "bundled pattern definition `{bundle}` is invalid: {message}"
                )
            }
            Self::Registry(error) => write!(formatter, "bundled pattern registry failed: {error}"),
        }
    }
}

impl std::error::Error for BundledPatternDefinitionError {}

/// Parses the immutable Weighted Voronoi bytes through the strict v1 parser.
pub fn load_bundled_weighted_voronoi_definition()
-> Result<PatternDefinition, BundledPatternDefinitionError> {
    parse_tnpattern(WEIGHTED_VORONOI_BUNDLED_BYTES)
        .map_err(|error| parse_error("weighted-voronoi.v1", error))
}

/// Parses the immutable compatibility Shapes bytes through the strict v1 parser.
pub fn load_bundled_shapes_definition() -> Result<PatternDefinition, BundledPatternDefinitionError>
{
    parse_tnpattern(SHAPES_BUNDLED_BYTES).map_err(|error| parse_error("compat-shapes.v1", error))
}

/// Parses the immutable compatibility Curves bytes through the strict v1 parser.
pub fn load_bundled_curves_definition() -> Result<PatternDefinition, BundledPatternDefinitionError>
{
    parse_tnpattern(CURVES_BUNDLED_BYTES).map_err(|error| parse_error("compat-curves.v1", error))
}

/// Parses the immutable Parametric Paths Spiral bytes through the strict v1 parser.
pub fn load_bundled_quadratic_radial_spiral_definition()
-> Result<PatternDefinition, BundledPatternDefinitionError> {
    parse_tnpattern(QUADRATIC_RADIAL_SPIRAL_BUNDLED_BYTES)
        .map_err(|error| parse_error("quadratic-radial-spiral.v1", error))
}

/// Parses the immutable Structured Fields proof through the strict v1 parser.
pub fn load_bundled_wave_line_field_definition()
-> Result<PatternDefinition, BundledPatternDefinitionError> {
    parse_tnpattern(WAVE_LINE_FIELD_BUNDLED_BYTES)
        .map_err(|error| parse_error("wave-line-field.v1", error))
}

/// Builds the immutable bundled registry. There is intentionally no legacy
/// metadata fallback and no user/project/filesystem source at this boundary.
pub fn load_bundled_pattern_definition_registry()
-> Result<PatternDefinitionRegistry, BundledPatternDefinitionError> {
    let shapes = load_bundled_shapes_definition()?;
    let curves = load_bundled_curves_definition()?;
    let quadratic_radial_spiral = load_bundled_quadratic_radial_spiral_definition()?;
    let wave_line_field = load_bundled_wave_line_field_definition()?;
    let weighted = load_bundled_weighted_voronoi_definition()?;
    PatternDefinitionRegistry::build(
        [
            shapes,
            curves,
            quadratic_radial_spiral,
            wave_line_field,
            weighted,
        ],
        std::iter::empty(),
        std::iter::empty(),
    )
    .map_err(BundledPatternDefinitionError::Registry)
}

fn parse_error(
    bundle: &'static str,
    error: PatternDefinitionError,
) -> BundledPatternDefinitionError {
    BundledPatternDefinitionError::Parse {
        bundle,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CreatorParameterCategory, CreatorParameterUnit, DefinitionParameterScope, LiteralValue,
        OutputChannelId, ParameterAuthoring, PatternDefinitionFingerprint,
        PatternDefinitionRegistryError, PatternDefinitionSource, PatternFamily, PatternId,
        PatternOutputKind, PatternParameterConstraints, RecipePortType, serialize_tnpattern,
    };
    use std::collections::BTreeSet;

    #[test]
    fn bundled_weighted_voronoi_is_strict_deterministic_and_registry_backed() {
        let definition = load_bundled_weighted_voronoi_definition().unwrap();
        assert_eq!(definition.id, PatternId::WEIGHTED_VORONOI_V1);
        assert_eq!(definition.display.name, "Weighted Voronoi");
        assert_eq!(definition.family, PatternFamily::StochasticDistributions);
        assert_eq!(definition.outputs, vec![PatternOutputKind::Regions]);
        assert_eq!(definition.recipe.nodes.len(), 6);
        assert_eq!(
            definition.recipe.nodes[2].operation.id,
            "weighted-voronoi.site-distribution"
        );
        assert_eq!(
            crate::REGISTERED_OPERATIONS
                .get("shapes.source-sample", 1)
                .unwrap()
                .output
                .kind,
            RecipePortType::ShapesSamples
        );
        assert_eq!(
            crate::REGISTERED_OPERATIONS
                .get("shapes.mark-map", 1)
                .unwrap()
                .output
                .kind,
            RecipePortType::ShapesMappedValues
        );
        assert_eq!(
            definition.recipe.nodes[4].operation.id,
            "weighted-voronoi.response-inset"
        );
        assert_eq!(
            definition.recipe.nodes[5].operation.id,
            "weighted-voronoi.emit-regions"
        );
        assert_eq!(definition.recipe.output.port, "geometry");
        assert!(
            definition
                .parameters
                .iter()
                .all(|parameter| { parameter.scope == DefinitionParameterScope::OutputChannel })
        );
        assert_eq!(
            definition
                .parameters
                .iter()
                .map(|parameter| parameter.key.as_str())
                .collect::<Vec<_>>(),
            vec![
                "enabled",
                "arrangement",
                "cell-count",
                "seed",
                "boundary-gap",
                "placement",
                "density-polarity",
                "density-strength",
                "response-strength",
                "minimum-cell-scale",
            ]
        );
        let seed = definition
            .parameters
            .iter()
            .find(|parameter| parameter.key == "seed")
            .unwrap();
        assert_eq!(seed.default, LiteralValue::Integer(0));
        let ParameterAuthoring::Creator(seed_metadata) = &seed.authoring else {
            unreachable!()
        };
        assert_eq!(
            seed_metadata.category,
            CreatorParameterCategory::IntegerValue
        );
        assert_eq!(seed_metadata.unit, CreatorParameterUnit::None);
        for key in ["density-strength", "response-strength"] {
            let parameter = definition
                .parameters
                .iter()
                .find(|parameter| parameter.key == key)
                .unwrap();
            let ParameterAuthoring::Creator(metadata) = &parameter.authoring else {
                unreachable!()
            };
            assert_eq!(
                metadata.category,
                CreatorParameterCategory::ResponseExponent
            );
            assert_eq!(metadata.unit, CreatorParameterUnit::Unitless);
        }
        assert_eq!(
            definition
                .parameters
                .iter()
                .find(|parameter| parameter.key == "arrangement")
                .unwrap()
                .choices,
            vec!["shared", "independent"]
        );
        assert_eq!(definition.quick_controls.len(), 9);
        let instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        definition.validate_instance_parameters(&instance).unwrap();
        let instance = definition
            .default_instance_parameters(OutputChannelId::RGB)
            .unwrap();
        definition.validate_instance_parameters(&instance).unwrap();
        let first = serialize_tnpattern(&definition).unwrap();
        let second =
            serialize_tnpattern(&load_bundled_weighted_voronoi_definition().unwrap()).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            PatternDefinitionFingerprint::from_canonical_bytes(&first),
            PatternDefinitionFingerprint::from_canonical_bytes(&second)
        );
        let registry = load_bundled_pattern_definition_registry().unwrap();
        let resolved = registry.get(&PatternId::WEIGHTED_VORONOI_V1).unwrap();
        assert_eq!(
            resolved.authoritative_source,
            PatternDefinitionSource::Bundled
        );
        assert_eq!(registry.definitions().count(), 5);
        assert_eq!(
            resolved.definition.recipe.nodes[3].operation.id,
            "weighted-voronoi.construct-voronoi"
        );
        assert_eq!(resolved.definition.recipe.nodes[3].operation.version, 1);
        assert_eq!(
            crate::REGISTERED_OPERATIONS
                .get("weighted-voronoi.construct-voronoi", 1)
                .unwrap()
                .output
                .kind,
            RecipePortType::VoronoiDiagram
        );
        let response_inset = crate::REGISTERED_OPERATIONS
            .get("weighted-voronoi.response-inset", 1)
            .unwrap();
        assert_eq!(
            response_inset.output.kind,
            RecipePortType::BoundaryDerivedRegionCells
        );
        assert_eq!(
            crate::REGISTERED_OPERATIONS
                .get("weighted-voronoi.emit-regions", 1)
                .unwrap()
                .inputs[0]
                .kind,
            RecipePortType::BoundaryDerivedRegionCells
        );
    }

    #[test]
    fn bundled_shapes_is_strict_typed_and_registry_backed() {
        let definition = load_bundled_shapes_definition().unwrap();
        assert_eq!(definition.id, PatternId::COMPATIBILITY_SHAPES_V1);
        assert_eq!(definition.family, PatternFamily::StructuredFields);
        assert_eq!(definition.outputs, vec![PatternOutputKind::Marks]);
        assert_eq!(definition.parameters.len(), 29);
        assert_eq!(
            definition
                .parameters
                .iter()
                .filter(|parameter| parameter.scope == DefinitionParameterScope::Pattern)
                .count(),
            10
        );
        assert_eq!(
            definition
                .parameters
                .iter()
                .filter(|parameter| parameter.scope == DefinitionParameterScope::OutputChannel)
                .count(),
            19
        );
        assert_eq!(definition.recipe.nodes.len(), 6);
        assert_eq!(
            definition
                .recipe
                .nodes
                .iter()
                .map(|node| node.operation.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "shapes.lattice-placement",
                "shapes.source-sample",
                "shapes.mark-map",
                "shapes.primitive-selection",
                "shapes.transforms",
                "shapes.emit-marks",
            ]
        );
        assert!(
            !definition
                .parameters
                .iter()
                .any(|parameter| parameter.key == "crosshatch-color")
        );
        let listed_parameters = definition
            .layout
            .sections
            .iter()
            .flat_map(|section| section.parameters.iter().map(String::as_str))
            .collect::<BTreeSet<_>>();
        let declared_parameters = definition
            .parameters
            .iter()
            .filter(|parameter| matches!(parameter.authoring, ParameterAuthoring::Creator(_)))
            .map(|parameter| parameter.key.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(listed_parameters, declared_parameters);
        assert_eq!(
            definition
                .layout
                .sections
                .iter()
                .map(|section| (section.id.as_str(), section.label.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("placement", "Placement"),
                ("motif", "Motif"),
                ("modulation", "Modulation"),
                ("deformation", "Deformation"),
                ("output", "Output"),
            ]
        );
        let lattice = crate::REGISTERED_OPERATIONS
            .get("shapes.lattice-placement", 1)
            .unwrap();
        assert_eq!(
            lattice
                .parameters
                .iter()
                .map(|parameter| parameter.name)
                .collect::<Vec<_>>(),
            vec![
                "output-width",
                "output-height",
                "long-edge-cells",
                "resolution-scale",
                "grid-rotation",
                "grid-pivot-x",
                "grid-pivot-y",
                "offset-x",
                "offset-y",
            ]
        );
        let transforms = crate::REGISTERED_OPERATIONS
            .get("shapes.transforms", 1)
            .unwrap();
        assert_eq!(
            transforms
                .parameters
                .iter()
                .map(|parameter| parameter.name)
                .collect::<Vec<_>>(),
            vec![
                "rotation",
                "scale",
                "width-scale",
                "height-scale",
                "grid-scale",
            ]
        );
        let mark_map = crate::REGISTERED_OPERATIONS
            .get("shapes.mark-map", 1)
            .unwrap();
        assert_eq!(
            mark_map
                .parameters
                .iter()
                .map(|parameter| parameter.name)
                .collect::<Vec<_>>(),
            vec!["min-mark", "max-mark", "threshold", "max-size"]
        );
        let lattice_node = definition
            .recipe
            .nodes
            .iter()
            .find(|node| node.id == "lattice")
            .unwrap();
        let transform_node = definition
            .recipe
            .nodes
            .iter()
            .find(|node| node.id == "transform")
            .unwrap();
        for key in [
            "resolution-scale",
            "grid-rotation",
            "grid-pivot-x",
            "grid-pivot-y",
            "offset-x",
            "offset-y",
        ] {
            assert!(lattice_node.parameters.contains_key(key));
            assert!(!transform_node.parameters.contains_key(key));
        }
        assert_eq!(definition.assets.len(), 1);
        assert_eq!(
            definition.assets[0].digest,
            "sha256:98ab8bcac5d0b69137f20f639e2204b8543d9f0b62c3516ab87f5fa66b3a2360"
        );
        let instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        definition.validate_instance_parameters(&instance).unwrap();
        let canonical = serialize_tnpattern(&definition).unwrap();
        assert_eq!(
            PatternDefinitionFingerprint::from_canonical_bytes(&canonical),
            PatternDefinitionFingerprint::from_canonical_bytes(
                &serialize_tnpattern(&load_bundled_shapes_definition().unwrap()).unwrap()
            )
        );
        let registry = load_bundled_pattern_definition_registry().unwrap();
        assert_eq!(
            registry
                .get(&PatternId::COMPATIBILITY_SHAPES_V1)
                .unwrap()
                .authoritative_source,
            PatternDefinitionSource::Bundled
        );
        assert_eq!(
            crate::REGISTERED_OPERATIONS
                .get("shapes.transforms", 1)
                .unwrap()
                .output
                .kind,
            RecipePortType::TransformedMarks
        );
        let mut incompatible = definition.clone();
        let map = incompatible
            .recipe
            .nodes
            .iter_mut()
            .find(|node| node.id == "map")
            .unwrap();
        map.operation.id = "weighted-voronoi.response-map".into();
        map.parameters.clear();
        let error = incompatible
            .validate_with_registry(&crate::REGISTERED_OPERATIONS)
            .unwrap_err();
        assert!(
            error.to_string().contains(
                "`sample.samples` ShapesSamples is incompatible with `map.samples` Samples"
            ),
            "unexpected cross-family graph error: {error}"
        );
        let mut changed = definition.clone();
        changed.display.name = "Changed Shapes".into();
        assert!(matches!(
            PatternDefinitionRegistry::build([definition], [changed], std::iter::empty()),
            Err(PatternDefinitionRegistryError::BundledDefinitionImmutable { .. })
        ));
    }

    #[test]
    fn bundled_curves_is_strict_typed_and_registry_backed() {
        let definition = load_bundled_curves_definition().unwrap();
        assert_eq!(definition.id, PatternId::COMPATIBILITY_CURVES_V1);
        assert_eq!(definition.family, PatternFamily::StructuredFields);
        assert_eq!(definition.outputs, vec![PatternOutputKind::Paths]);
        assert_eq!(definition.parameters.len(), 38);
        assert_eq!(
            definition
                .parameters
                .iter()
                .filter(|parameter| parameter.scope == DefinitionParameterScope::Pattern)
                .count(),
            10
        );
        assert_eq!(
            definition
                .parameters
                .iter()
                .filter(|parameter| parameter.scope == DefinitionParameterScope::OutputChannel)
                .count(),
            28
        );
        assert_eq!(
            definition
                .recipe
                .nodes
                .iter()
                .map(|node| node.operation.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "curves.placement",
                "curves.source-sample",
                "curves.motif-selection",
                "curves.deformation",
                "curves.width-modulation",
                "curves.emit-paths",
            ]
        );
        let listed_parameters = definition
            .layout
            .sections
            .iter()
            .flat_map(|section| section.parameters.iter().map(String::as_str))
            .collect::<BTreeSet<_>>();
        let declared_parameters = definition
            .parameters
            .iter()
            .filter(|parameter| matches!(parameter.authoring, ParameterAuthoring::Creator(_)))
            .map(|parameter| parameter.key.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(listed_parameters, declared_parameters);
        assert_eq!(
            crate::REGISTERED_OPERATIONS
                .get("curves.placement", 1)
                .unwrap()
                .output
                .kind,
            RecipePortType::CurvePlacement
        );
        assert_eq!(
            crate::REGISTERED_OPERATIONS
                .get("curves.width-modulation", 1)
                .unwrap()
                .output
                .kind,
            RecipePortType::CurveModulatedPaths
        );
        let mut incompatible = definition.clone();
        let sample = incompatible
            .recipe
            .nodes
            .iter_mut()
            .find(|node| node.id == "sample")
            .unwrap();
        sample.operation.id = "weighted-voronoi.response-map".into();
        incompatible
            .recipe
            .edges
            .iter_mut()
            .find(|edge| edge.to.node == "sample")
            .unwrap()
            .to
            .port = "samples".into();
        let error = incompatible
            .validate_with_registry(&crate::REGISTERED_OPERATIONS)
            .unwrap_err();
        assert!(
            error.to_string().contains("CurvePlacement"),
            "unexpected cross-family graph error: {error}"
        );
        let registry = load_bundled_pattern_definition_registry().unwrap();
        assert_eq!(
            registry
                .get(&PatternId::COMPATIBILITY_CURVES_V1)
                .unwrap()
                .authoritative_source,
            PatternDefinitionSource::Bundled
        );
    }

    #[test]
    fn bundled_bytes_are_strict_and_immutable_without_legacy_fallback() {
        assert!(
            parse_tnpattern(
                br#"{"format_version":0,"recipe_version":1,"id":"weighted-voronoi.v1"}"#
            )
            .is_err()
        );
        let bundled = load_bundled_weighted_voronoi_definition().unwrap();
        let mut changed = bundled.clone();
        changed.display.name = "Changed".into();
        assert!(matches!(
            PatternDefinitionRegistry::build([bundled], [changed], std::iter::empty()),
            Err(PatternDefinitionRegistryError::BundledDefinitionImmutable { .. })
        ));
    }

    #[test]
    fn bundled_quadratic_radial_spiral_bytes_are_strict_stable_and_nonlossy() {
        let definition = load_bundled_quadratic_radial_spiral_definition().unwrap();
        assert_eq!(definition.id, PatternId::QUADRATIC_RADIAL_SPIRAL_V1);
        assert_eq!(definition.family, PatternFamily::ParametricPaths);
        assert_eq!(definition.outputs, vec![PatternOutputKind::Paths]);
        assert_eq!(definition.recipe.nodes.len(), 2);
        assert_eq!(
            definition.recipe.nodes[0].operation.id,
            "parametric-paths.quadratic-radial-spiral"
        );
        assert_eq!(
            definition.recipe.nodes[1].operation.id,
            "parametric-paths.emit-paths"
        );
        definition
            .validate_with_registry(&crate::REGISTERED_OPERATIONS)
            .unwrap();
        let canonical = serialize_tnpattern(&definition).unwrap();
        assert_eq!(
            canonical,
            serialize_tnpattern(&parse_tnpattern(&canonical).unwrap()).unwrap()
        );

        // A valid extension-position record has no current editor meaning, but
        // it is graph/schema data and must survive the generic serializer.
        let mut unfamiliar = definition.clone();
        unfamiliar.layout.node_positions.insert(
            "third-party-analysis".into(),
            crate::GraphPosition { x: -18.5, y: 42.0 },
        );
        unfamiliar
            .validate_with_registry(&crate::REGISTERED_OPERATIONS)
            .unwrap();
        let bytes = serialize_tnpattern(&unfamiliar).unwrap();
        assert_eq!(parse_tnpattern(&bytes).unwrap(), unfamiliar);
    }

    #[test]
    fn bundled_wave_line_field_is_strict_stable_and_registry_backed() {
        let definition = load_bundled_wave_line_field_definition().unwrap();
        assert_eq!(definition.id, PatternId::WAVE_LINE_FIELD_V1);
        assert_eq!(definition.family, PatternFamily::StructuredFields);
        assert_eq!(definition.outputs, vec![PatternOutputKind::Paths]);
        assert_eq!(
            definition.recipe.nodes[0].operation.id,
            "structured-fields.wave-line-field"
        );
        assert_eq!(
            definition.recipe.nodes[1].operation.id,
            "structured-fields.source-width"
        );
        assert_eq!(
            definition.recipe.nodes[2].operation.id,
            "structured-fields.emit-paths"
        );
        assert!(
            definition
                .parameters
                .iter()
                .all(|parameter| parameter.key != "seed")
        );
        definition
            .validate_with_registry(&crate::REGISTERED_OPERATIONS)
            .unwrap();
        let canonical = serialize_tnpattern(&definition).unwrap();
        assert_eq!(
            canonical,
            serialize_tnpattern(&parse_tnpattern(&canonical).unwrap()).unwrap()
        );
        let registry = load_bundled_pattern_definition_registry().unwrap();
        assert_eq!(
            registry
                .get(&PatternId::WAVE_LINE_FIELD_V1)
                .unwrap()
                .authoritative_source,
            PatternDefinitionSource::Bundled
        );

        let mut invalid_channel_values = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        invalid_channel_values.output_channel_values[0]
            .values
            .iter_mut()
            .find(|value| value.key == "line-width-min")
            .unwrap()
            .value = LiteralValue::Number(1.4);
        assert!(
            definition
                .validate_instance_parameters(&invalid_channel_values)
                .unwrap_err()
                .to_string()
                .contains("must be less than")
        );

        let mut self_referential = definition.clone();
        let minimum = self_referential
            .parameters
            .iter_mut()
            .find(|parameter| parameter.key == "line-width-min")
            .unwrap();
        minimum.constraints = PatternParameterConstraints::NumberLessThanParameter {
            minimum: 0.1,
            maximum: 10.0,
            step: 0.1,
            parameter: "line-width-min".into(),
            factor: 1.0,
        };
        assert!(
            self_referential
                .validate_with_registry(&crate::REGISTERED_OPERATIONS)
                .unwrap_err()
                .to_string()
                .contains("invalid cross-parameter")
        );
    }

    #[test]
    fn runtime_dimensions_are_internal_and_recipe_bound() {
        for definition in [
            load_bundled_shapes_definition().unwrap(),
            load_bundled_curves_definition().unwrap(),
        ] {
            let listed = definition
                .layout
                .sections
                .iter()
                .flat_map(|section| section.parameters.iter().map(String::as_str))
                .collect::<BTreeSet<_>>();
            let placement = definition
                .recipe
                .nodes
                .iter()
                .find(|node| node.id == "lattice" || node.id == "placement")
                .unwrap();
            for key in ["output-width", "output-height"] {
                let parameter = definition
                    .parameters
                    .iter()
                    .find(|parameter| parameter.key == key)
                    .unwrap();
                assert!(matches!(parameter.authoring, ParameterAuthoring::Internal));
                assert!(!listed.contains(key));
                assert!(placement.parameters.contains_key(key));
            }
            let instance = definition
                .default_instance_parameters(OutputChannelId::CMYK)
                .unwrap();
            definition.validate_instance_parameters(&instance).unwrap();
        }
    }
}
