//! Strict, declarative `.tnpattern` format-v1 definitions.
//!
//! This module describes recipes but deliberately does not execute them. Native
//! operation registration is the only executable boundary: definitions cannot
//! contain scripts, plugins, expressions, commands, or native code.

use crate::{
    artwork_pipeline::OutputChannelId,
    cancel::CancellationToken,
    curves_native::{
        CurvesDeformedPaths, CurvesModulatedPaths, CurvesMotif, CurvesPlacement, CurvesSamples,
    },
    pattern::{
        ArtboardSpace, CanonicalPatternOutput, PatternFamily, PatternId, PatternOutputKind,
        RegionPatternOutput,
    },
    shapes_native::{
        ShapesLattice, ShapesMappedValues, ShapesSamples, ShapesSelectedPrimitive,
        ShapesTransformedMarks,
    },
    site_distribution::{DistributionField, OrderedPoint, SiteDistribution},
    voronoi_geometry::VoronoiDiagram,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

pub const TNPATTERN_FORMAT_VERSION: u32 = 1;
pub const TNPATTERN_RECIPE_VERSION: u32 = 1;
pub const TNPATTERN_INSTANCE_FORMAT_VERSION: u32 = 1;
pub const MAX_PATTERN_NODES: usize = 64;
pub const MAX_PATTERN_EDGES: usize = 128;
pub const MAX_PATTERN_PARAMETERS: usize = 64;
pub const MAX_PATTERN_ASSETS: usize = 16;
pub const MAX_EMBEDDED_SVG_BYTES: usize = 262_144;
/// The aggregate bound is explicit even though it is also the product of the
/// per-asset and asset-count limits.
pub const MAX_TOTAL_EMBEDDED_SVG_BYTES: usize = MAX_PATTERN_ASSETS * MAX_EMBEDDED_SVG_BYTES;
pub const MAX_TEXT_PARAMETER_BYTES: usize = 4_096;
pub const MAX_PATTERN_INSTANCE_OUTPUT_CHANNELS: usize = 7;
pub const MAX_PATTERN_INSTANCE_VALUES: usize =
    MAX_PATTERN_PARAMETERS * (MAX_PATTERN_INSTANCE_OUTPUT_CHANNELS + 1);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternDefinition {
    pub format_version: u32,
    pub recipe_version: u32,
    pub id: PatternId,
    pub display: PatternDisplayMetadata,
    pub family: PatternFamily,
    pub outputs: Vec<PatternOutputKind>,
    pub parameters: Vec<PatternParameterDefinition>,
    pub quick_controls: Vec<QuickControlDefinition>,
    pub layout: AuthoringLayout,
    pub recipe: RecipeGraph,
    pub assets: Vec<EmbeddedSvgAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternDisplayMetadata {
    pub name: String,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefinitionParameterScope {
    Pattern,
    OutputChannel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecipeValueType {
    Number,
    Integer,
    Boolean,
    Text,
    Choice,
    SvgAsset,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum LiteralValue {
    Number(f64),
    Integer(u64),
    Boolean(bool),
    Text(String),
    Choice(String),
    SvgAsset(String),
}

impl LiteralValue {
    fn value_type(&self) -> RecipeValueType {
        match self {
            Self::Number(_) => RecipeValueType::Number,
            Self::Integer(_) => RecipeValueType::Integer,
            Self::Boolean(_) => RecipeValueType::Boolean,
            Self::Text(_) => RecipeValueType::Text,
            Self::Choice(_) => RecipeValueType::Choice,
            Self::SvgAsset(_) => RecipeValueType::SvgAsset,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternParameterDefinition {
    /// A stable, machine-facing v1 key. It is never inferred from the label.
    pub key: String,
    /// Creator-facing text; this remains independent from the stable key.
    pub label: String,
    pub help: String,
    pub scope: DefinitionParameterScope,
    pub value_type: RecipeValueType,
    pub default: LiteralValue,
    pub constraints: PatternParameterConstraints,
    pub choices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[serde(deny_unknown_fields)]
pub enum PatternParameterConstraints {
    Number {
        minimum: f64,
        maximum: f64,
        step: f64,
    },
    Integer {
        minimum: u64,
        maximum: u64,
        step: u64,
    },
    Boolean,
    Text {
        max_length: usize,
    },
    Choice,
    SvgAsset,
}

impl PatternParameterConstraints {
    fn value_type(&self) -> RecipeValueType {
        match self {
            Self::Number { .. } => RecipeValueType::Number,
            Self::Integer { .. } => RecipeValueType::Integer,
            Self::Boolean => RecipeValueType::Boolean,
            Self::Text { .. } => RecipeValueType::Text,
            Self::Choice => RecipeValueType::Choice,
            Self::SvgAsset => RecipeValueType::SvgAsset,
        }
    }
}

/// A single named parameter value. Lists, rather than maps, preserve duplicate
/// input so validation can reject it instead of accepting a last-write-wins
/// JSON object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternInstanceValue {
    pub key: String,
    pub value: LiteralValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputChannelParameterValues {
    pub channel: String,
    pub values: Vec<PatternInstanceValue>,
}

/// A separate, scoped v1 instance payload. It carries values only; selecting
/// per-channel artwork inputs remains outside this contract until TON-011.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternInstanceParameters {
    pub format_version: u32,
    pub pattern_id: PatternId,
    pub pattern_values: Vec<PatternInstanceValue>,
    pub output_channel_values: Vec<OutputChannelParameterValues>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuickControlKind {
    Slider,
    Toggle,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuickControlDefinition {
    pub id: String,
    pub parameter: String,
    pub scope: DefinitionParameterScope,
    pub kind: QuickControlKind,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringLayout {
    pub sections: Vec<AuthoringSection>,
    pub node_positions: BTreeMap<String, GraphPosition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringSection {
    pub id: String,
    pub label: String,
    pub parameters: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphPosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeGraph {
    pub nodes: Vec<RecipeNode>,
    pub edges: Vec<RecipeEdge>,
    pub output: PortReference,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeNode {
    pub id: String,
    pub operation: OperationReference,
    pub parameters: BTreeMap<String, RecipeArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationReference {
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", content = "value", rename_all = "kebab-case")]
pub enum RecipeArgument {
    Literal(LiteralValue),
    Parameter(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeEdge {
    pub from: PortReference,
    pub to: PortReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortReference {
    pub node: String,
    pub port: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddedSvgAsset {
    pub digest: String,
    pub svg: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipePortType {
    /// A finite deterministic lattice of candidate mark positions.
    Lattice,
    Placement,
    Samples,
    MappedField,
    /// Shapes-specific sampled values, retaining their lattice provenance.
    ShapesSamples,
    /// Shapes-specific threshold/size response values.
    ShapesMappedValues,
    /// Curves-specific lattice placement, including its grid provenance.
    CurvePlacement,
    /// Curves-specific sampled source response.
    CurveSamples,
    /// An editable curve motif/path selection.
    CurveMotif,
    /// Repeated and transformed curve paths before width modulation.
    CurveDeformedPaths,
    /// Curve paths carrying source-derived width response.
    CurveModulatedPaths,
    /// One resolved primitive or editable custom mark definition.
    MarkPrimitive,
    /// Final transformed mark instances before canonical wrapping.
    TransformedMarks,
    /// Future native site-deformation stages may use this truthful point-set
    /// port. Boundary-derived polygons must use `BoundaryDerivedRegionCells`.
    DeformedSites,
    VoronoiDiagram,
    BoundaryDerivedRegionCells,
    CanonicalGeometry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationPortDescriptor {
    pub name: &'static str,
    pub kind: RecipePortType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationParameterDescriptor {
    pub name: &'static str,
    pub value_type: RecipeValueType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisteredOperationDescriptor {
    pub id: &'static str,
    pub version: u32,
    pub inputs: &'static [OperationPortDescriptor],
    pub output: OperationPortDescriptor,
    pub parameters: &'static [OperationParameterDescriptor],
    /// Canonical output kinds emitted by this operation. It is internal native
    /// metadata, not a new `.tnpattern` field.
    pub canonical_output_kinds: &'static [PatternOutputKind],
}

const NO_PORTS: [OperationPortDescriptor; 0] = [];
const NO_PARAMETERS: [OperationParameterDescriptor; 0] = [];
const NO_CANONICAL_OUTPUT_KINDS: [PatternOutputKind; 0] = [];
const REGION_OUTPUT_KIND: [PatternOutputKind; 1] = [PatternOutputKind::Regions];
const MARK_OUTPUT_KIND: [PatternOutputKind; 1] = [PatternOutputKind::Marks];
const NETWORK_OUTPUT_KIND: [PatternOutputKind; 1] = [PatternOutputKind::Networks];
const RESPONSE_FIELD_INPUT: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
    name: "samples",
    kind: RecipePortType::Samples,
}];
const SITE_DISTRIBUTION_INPUT: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
    name: "response-field",
    kind: RecipePortType::MappedField,
}];
const VORONOI_INPUT: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
    name: "sites",
    kind: RecipePortType::Placement,
}];
const RESPONSE_INSET_INPUTS: [OperationPortDescriptor; 2] = [
    OperationPortDescriptor {
        name: "diagram",
        kind: RecipePortType::VoronoiDiagram,
    },
    OperationPortDescriptor {
        name: "response-field",
        kind: RecipePortType::MappedField,
    },
];
const REGION_EMIT_INPUT: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
    name: "response-insets",
    kind: RecipePortType::BoundaryDerivedRegionCells,
}];
const SITE_DISTRIBUTION_PARAMETERS: [OperationParameterDescriptor; 6] = [
    OperationParameterDescriptor {
        name: "cell-count",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "seed",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "arrangement",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "placement",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "density-polarity",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "density-strength",
        value_type: RecipeValueType::Number,
    },
];
const RESPONSE_INSET_PARAMETERS: [OperationParameterDescriptor; 3] = [
    OperationParameterDescriptor {
        name: "response-strength",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "minimum-cell-scale",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "boundary-gap",
        value_type: RecipeValueType::Number,
    },
];
const REGION_EMIT_PARAMETERS: [OperationParameterDescriptor; 1] = [OperationParameterDescriptor {
    name: "enabled",
    value_type: RecipeValueType::Boolean,
}];
const SHAPES_LATTICE_PARAMETERS: [OperationParameterDescriptor; 9] = [
    OperationParameterDescriptor {
        name: "output-width",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "output-height",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "long-edge-cells",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "resolution-scale",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-rotation",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-pivot-x",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-pivot-y",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "offset-x",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "offset-y",
        value_type: RecipeValueType::Number,
    },
];
// The editor's extended lattice operation is deliberately versioned instead
// of changing the bundled v1 operation contract. User-authored recipes can
// opt into deterministic axis spacing, curved grid offsets, random sampling,
// and jitter while existing bundled Shapes definitions remain immutable.
const SHAPES_EDITOR_LATTICE_PARAMETERS_V2: [OperationParameterDescriptor; 25] = [
    OperationParameterDescriptor {
        name: "output-width",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "output-height",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "long-edge-cells",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "resolution-scale",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-rotation",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-pivot-x",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-pivot-y",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "offset-x",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "offset-y",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "x-grid-spacing",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "y-grid-spacing",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "x-grid-mode",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "y-grid-mode",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "x-grid-curve",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "y-grid-curve",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "curve-function",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "placement-strategy",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "curve-spacing",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "random-dispersion",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "point-definition",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "jitter-factor",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "point-sampler",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "channel-seed",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "channel-weight-influence",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "random-size-response",
        value_type: RecipeValueType::Number,
    },
];
// Version 1 predates the editor's random-placement size-response binding.
// Keep it executable for already-embedded custom recipes; new editor drafts
// use v2 and resolve the channel-scoped value.
const SHAPES_EDITOR_LATTICE_PARAMETERS_V1: [OperationParameterDescriptor; 24] = [
    OperationParameterDescriptor {
        name: "output-width",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "output-height",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "long-edge-cells",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "resolution-scale",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-rotation",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-pivot-x",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-pivot-y",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "offset-x",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "offset-y",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "x-grid-spacing",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "y-grid-spacing",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "x-grid-mode",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "y-grid-mode",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "x-grid-curve",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "y-grid-curve",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "curve-function",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "placement-strategy",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "curve-spacing",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "random-dispersion",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "point-definition",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "jitter-factor",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "point-sampler",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "channel-seed",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "channel-weight-influence",
        value_type: RecipeValueType::Number,
    },
];
const SHAPES_LATTICE_INPUT: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
    name: "lattice",
    kind: RecipePortType::Lattice,
}];
const SHAPES_SAMPLES_INPUT: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
    name: "samples",
    kind: RecipePortType::ShapesSamples,
}];
const SHAPES_MARK_MAP_PARAMETERS: [OperationParameterDescriptor; 4] = [
    OperationParameterDescriptor {
        name: "min-mark",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "max-mark",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "threshold",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "max-size",
        value_type: RecipeValueType::Number,
    },
];
const SHAPES_PRIMITIVE_PARAMETERS: [OperationParameterDescriptor; 7] = [
    OperationParameterDescriptor {
        name: "use-shared-mark",
        value_type: RecipeValueType::Boolean,
    },
    OperationParameterDescriptor {
        name: "shared-shape",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "polygon-sides",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "global-custom-motif",
        value_type: RecipeValueType::SvgAsset,
    },
    OperationParameterDescriptor {
        name: "shape",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "channel-polygon-sides",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "channel-custom-motif",
        value_type: RecipeValueType::SvgAsset,
    },
];
const SHAPES_TRANSFORM_INPUTS: [OperationPortDescriptor; 2] = [
    OperationPortDescriptor {
        name: "lattice",
        kind: RecipePortType::Lattice,
    },
    OperationPortDescriptor {
        name: "primitive",
        kind: RecipePortType::MarkPrimitive,
    },
];
const SHAPES_TRANSFORM_PARAMETERS: [OperationParameterDescriptor; 5] = [
    OperationParameterDescriptor {
        name: "rotation",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "scale",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "width-scale",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "height-scale",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-scale",
        value_type: RecipeValueType::Number,
    },
];
const SHAPES_EMIT_PARAMETERS: [OperationParameterDescriptor; 3] = [
    OperationParameterDescriptor {
        name: "enabled",
        value_type: RecipeValueType::Boolean,
    },
    OperationParameterDescriptor {
        name: "color",
        value_type: RecipeValueType::Text,
    },
    OperationParameterDescriptor {
        name: "opacity",
        value_type: RecipeValueType::Number,
    },
];
const SHAPES_NETWORK_EMIT_PARAMETERS: [OperationParameterDescriptor; 4] = [
    OperationParameterDescriptor {
        name: "enabled",
        value_type: RecipeValueType::Boolean,
    },
    OperationParameterDescriptor {
        name: "color",
        value_type: RecipeValueType::Text,
    },
    OperationParameterDescriptor {
        name: "opacity",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "connection-mode",
        value_type: RecipeValueType::Choice,
    },
];
const SHAPES_PRIMITIVE_INPUT: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
    name: "mapped-values",
    kind: RecipePortType::ShapesMappedValues,
}];
const SHAPES_EMIT_INPUT: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
    name: "marks",
    kind: RecipePortType::TransformedMarks,
}];
const CURVE_PLACEMENT_INPUT: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
    name: "placement",
    kind: RecipePortType::CurvePlacement,
}];
const CURVE_DEFORM_INPUTS: [OperationPortDescriptor; 2] = [
    OperationPortDescriptor {
        name: "placement",
        kind: RecipePortType::CurvePlacement,
    },
    OperationPortDescriptor {
        name: "motif",
        kind: RecipePortType::CurveMotif,
    },
];
const CURVE_MODULATE_INPUTS: [OperationPortDescriptor; 2] = [
    OperationPortDescriptor {
        name: "paths",
        kind: RecipePortType::CurveDeformedPaths,
    },
    OperationPortDescriptor {
        name: "samples",
        kind: RecipePortType::CurveSamples,
    },
];
const CURVE_EMIT_INPUT: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
    name: "paths",
    kind: RecipePortType::CurveModulatedPaths,
}];
const CURVE_PLACEMENT_PARAMETERS: [OperationParameterDescriptor; 9] = [
    OperationParameterDescriptor {
        name: "output-width",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "output-height",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "long-edge-cells",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "resolution-scale",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-rotation",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-pivot-x",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "grid-pivot-y",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "offset-x",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "offset-y",
        value_type: RecipeValueType::Number,
    },
];
const CURVE_MOTIF_PARAMETERS: [OperationParameterDescriptor; 7] = [
    OperationParameterDescriptor {
        name: "use-shared-curve",
        value_type: RecipeValueType::Boolean,
    },
    OperationParameterDescriptor {
        name: "shared-path",
        value_type: RecipeValueType::SvgAsset,
    },
    OperationParameterDescriptor {
        name: "shared-close-ends",
        value_type: RecipeValueType::Boolean,
    },
    OperationParameterDescriptor {
        name: "shared-smooth-join",
        value_type: RecipeValueType::Boolean,
    },
    OperationParameterDescriptor {
        name: "path",
        value_type: RecipeValueType::SvgAsset,
    },
    OperationParameterDescriptor {
        name: "close-ends",
        value_type: RecipeValueType::Boolean,
    },
    OperationParameterDescriptor {
        name: "smooth-join",
        value_type: RecipeValueType::Boolean,
    },
];
const CURVE_DEFORM_PARAMETERS: [OperationParameterDescriptor; 18] = [
    OperationParameterDescriptor {
        name: "layout",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "curve-scale",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "motif-coverage",
        value_type: RecipeValueType::Choice,
    },
    OperationParameterDescriptor {
        name: "motif-bleed",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "tile-count",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "tile-angle",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "tile-offset",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "stack-count",
        value_type: RecipeValueType::Integer,
    },
    OperationParameterDescriptor {
        name: "stack-spacing",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "stack-angle",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "stack-offset",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "alternate-stack-offset",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "alternate-tile-transform",
        value_type: RecipeValueType::Choice,
    },
    // Automatic motif coverage calls `max_curve_width` before width modulation
    // emits points, so these response values are also deformation inputs.
    OperationParameterDescriptor {
        name: "min-mark",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "max-mark",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "max-size",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "scale",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "output-quality",
        value_type: RecipeValueType::Number,
    },
];
const CURVE_MODULATE_PARAMETERS: [OperationParameterDescriptor; 6] = [
    OperationParameterDescriptor {
        name: "min-mark",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "max-mark",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "threshold",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "max-size",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "scale",
        value_type: RecipeValueType::Number,
    },
    OperationParameterDescriptor {
        name: "output-quality",
        value_type: RecipeValueType::Number,
    },
];
const CURVE_EMIT_PARAMETERS: [OperationParameterDescriptor; 3] = [
    OperationParameterDescriptor {
        name: "enabled",
        value_type: RecipeValueType::Boolean,
    },
    OperationParameterDescriptor {
        name: "color",
        value_type: RecipeValueType::Text,
    },
    OperationParameterDescriptor {
        name: "opacity",
        value_type: RecipeValueType::Number,
    },
];
const PATH_OUTPUT_KIND: [PatternOutputKind; 1] = [PatternOutputKind::Paths];

pub static REGISTERED_OPERATIONS: OperationRegistry = OperationRegistry::new(&[
    RegisteredOperationDescriptor {
        id: "shapes.lattice-placement",
        version: 1,
        inputs: &NO_PORTS,
        output: OperationPortDescriptor {
            name: "lattice",
            kind: RecipePortType::Lattice,
        },
        parameters: &SHAPES_LATTICE_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "shapes.lattice-placement-editor",
        version: 1,
        inputs: &NO_PORTS,
        output: OperationPortDescriptor {
            name: "lattice",
            kind: RecipePortType::Lattice,
        },
        parameters: &SHAPES_EDITOR_LATTICE_PARAMETERS_V1,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "shapes.lattice-placement-editor",
        version: 2,
        inputs: &NO_PORTS,
        output: OperationPortDescriptor {
            name: "lattice",
            kind: RecipePortType::Lattice,
        },
        parameters: &SHAPES_EDITOR_LATTICE_PARAMETERS_V2,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "shapes.source-sample",
        version: 1,
        inputs: &SHAPES_LATTICE_INPUT,
        output: OperationPortDescriptor {
            name: "samples",
            kind: RecipePortType::ShapesSamples,
        },
        parameters: &NO_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "shapes.mark-map",
        version: 1,
        inputs: &SHAPES_SAMPLES_INPUT,
        output: OperationPortDescriptor {
            name: "mapped-values",
            kind: RecipePortType::ShapesMappedValues,
        },
        parameters: &SHAPES_MARK_MAP_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "shapes.primitive-selection",
        version: 1,
        inputs: &SHAPES_PRIMITIVE_INPUT,
        output: OperationPortDescriptor {
            name: "primitive",
            kind: RecipePortType::MarkPrimitive,
        },
        parameters: &SHAPES_PRIMITIVE_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "shapes.transforms",
        version: 1,
        inputs: &SHAPES_TRANSFORM_INPUTS,
        output: OperationPortDescriptor {
            name: "marks",
            kind: RecipePortType::TransformedMarks,
        },
        parameters: &SHAPES_TRANSFORM_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "shapes.emit-marks",
        version: 1,
        inputs: &SHAPES_EMIT_INPUT,
        output: OperationPortDescriptor {
            name: "geometry",
            kind: RecipePortType::CanonicalGeometry,
        },
        parameters: &SHAPES_EMIT_PARAMETERS,
        canonical_output_kinds: &MARK_OUTPUT_KIND,
    },
    RegisteredOperationDescriptor {
        id: "shapes.emit-network",
        version: 1,
        inputs: &SHAPES_EMIT_INPUT,
        output: OperationPortDescriptor {
            name: "geometry",
            kind: RecipePortType::CanonicalGeometry,
        },
        parameters: &SHAPES_NETWORK_EMIT_PARAMETERS,
        canonical_output_kinds: &NETWORK_OUTPUT_KIND,
    },
    RegisteredOperationDescriptor {
        id: "curves.placement",
        version: 1,
        inputs: &NO_PORTS,
        output: OperationPortDescriptor {
            name: "placement",
            kind: RecipePortType::CurvePlacement,
        },
        parameters: &CURVE_PLACEMENT_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "curves.source-sample",
        version: 1,
        inputs: &CURVE_PLACEMENT_INPUT,
        output: OperationPortDescriptor {
            name: "samples",
            kind: RecipePortType::CurveSamples,
        },
        parameters: &NO_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "curves.motif-selection",
        version: 1,
        inputs: &NO_PORTS,
        output: OperationPortDescriptor {
            name: "motif",
            kind: RecipePortType::CurveMotif,
        },
        parameters: &CURVE_MOTIF_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "curves.deformation",
        version: 1,
        inputs: &CURVE_DEFORM_INPUTS,
        output: OperationPortDescriptor {
            name: "paths",
            kind: RecipePortType::CurveDeformedPaths,
        },
        parameters: &CURVE_DEFORM_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "curves.width-modulation",
        version: 1,
        inputs: &CURVE_MODULATE_INPUTS,
        output: OperationPortDescriptor {
            name: "paths",
            kind: RecipePortType::CurveModulatedPaths,
        },
        parameters: &CURVE_MODULATE_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "curves.emit-paths",
        version: 1,
        inputs: &CURVE_EMIT_INPUT,
        output: OperationPortDescriptor {
            name: "geometry",
            kind: RecipePortType::CanonicalGeometry,
        },
        parameters: &CURVE_EMIT_PARAMETERS,
        canonical_output_kinds: &PATH_OUTPUT_KIND,
    },
    RegisteredOperationDescriptor {
        id: "weighted-voronoi.source-sample",
        version: 1,
        inputs: &NO_PORTS,
        output: OperationPortDescriptor {
            name: "samples",
            kind: RecipePortType::Samples,
        },
        parameters: &NO_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "weighted-voronoi.response-map",
        version: 1,
        inputs: &RESPONSE_FIELD_INPUT,
        output: OperationPortDescriptor {
            name: "response-field",
            kind: RecipePortType::MappedField,
        },
        parameters: &NO_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "weighted-voronoi.site-distribution",
        version: 1,
        inputs: &SITE_DISTRIBUTION_INPUT,
        output: OperationPortDescriptor {
            name: "sites",
            kind: RecipePortType::Placement,
        },
        parameters: &SITE_DISTRIBUTION_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "weighted-voronoi.construct-voronoi",
        version: 1,
        inputs: &VORONOI_INPUT,
        output: OperationPortDescriptor {
            name: "diagram",
            kind: RecipePortType::VoronoiDiagram,
        },
        parameters: &NO_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "weighted-voronoi.response-inset",
        version: 1,
        inputs: &RESPONSE_INSET_INPUTS,
        output: OperationPortDescriptor {
            name: "response-insets",
            kind: RecipePortType::BoundaryDerivedRegionCells,
        },
        parameters: &RESPONSE_INSET_PARAMETERS,
        canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
    },
    RegisteredOperationDescriptor {
        id: "weighted-voronoi.emit-regions",
        version: 1,
        inputs: &REGION_EMIT_INPUT,
        output: OperationPortDescriptor {
            name: "geometry",
            kind: RecipePortType::CanonicalGeometry,
        },
        parameters: &REGION_EMIT_PARAMETERS,
        canonical_output_kinds: &REGION_OUTPUT_KIND,
    },
]);

#[derive(Debug)]
pub struct OperationRegistry {
    entries: &'static [RegisteredOperationDescriptor],
}

impl OperationRegistry {
    pub const fn new(entries: &'static [RegisteredOperationDescriptor]) -> Self {
        Self { entries }
    }

    pub const fn entries(&self) -> &'static [RegisteredOperationDescriptor] {
        self.entries
    }

    pub fn get(&self, id: &str, version: u32) -> Option<&'static RegisteredOperationDescriptor> {
        self.entries
            .iter()
            .find(|descriptor| descriptor.id == id && descriptor.version == version)
    }

    fn operation_exists(&self, id: &str) -> bool {
        self.entries.iter().any(|descriptor| descriptor.id == id)
    }
}

/// Typed native values flowing only between registered Rust operations. The
/// recipe file can name ports but cannot construct, serialize, or execute any
/// of these values.
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeVoronoiDiagram {
    /// Geometry is produced exclusively by the neutral Voronoi authority.
    pub diagram: VoronoiDiagram,
    /// The exact sites used to construct `diagram`, retained only for the
    /// subsequent response-inset stage. The neutral geometry type intentionally
    /// does not own caller placement provenance.
    pub sites: Vec<OrderedPoint>,
}

/// Typed native values flowing only between registered Rust operations. The
/// recipe file can name ports but cannot construct, serialize, or execute any
/// of these values.
#[derive(Debug, Clone, PartialEq)]
pub enum RecipeRuntimeValue {
    Placement(SiteDistribution),
    Samples(DistributionField),
    MappedField(DistributionField),
    DeformedSites(SiteDistribution),
    VoronoiDiagram(RecipeVoronoiDiagram),
    /// Final boundary-derived inset cell polygons before canonical wrapping.
    /// This reuses the existing region geometry authority rather than copying
    /// polygon data into a second recipe-specific model.
    BoundaryDerivedRegionCells(RegionPatternOutput),
    ShapesLattice(ShapesLattice),
    ShapesSamples(ShapesSamples),
    ShapesMappedValues(ShapesMappedValues),
    ShapesPrimitive(ShapesSelectedPrimitive),
    ShapesTransformedMarks(ShapesTransformedMarks),
    CurvesPlacement(CurvesPlacement),
    CurvesSamples(CurvesSamples),
    CurvesMotif(CurvesMotif),
    CurvesDeformedPaths(CurvesDeformedPaths),
    CurvesModulatedPaths(CurvesModulatedPaths),
    CanonicalOutput(CanonicalPatternOutput),
}

impl RecipeRuntimeValue {
    pub const fn port_type(&self) -> RecipePortType {
        match self {
            Self::Placement(_) => RecipePortType::Placement,
            Self::Samples(_) => RecipePortType::Samples,
            Self::MappedField(_) => RecipePortType::MappedField,
            Self::DeformedSites(_) => RecipePortType::DeformedSites,
            Self::VoronoiDiagram(_) => RecipePortType::VoronoiDiagram,
            Self::BoundaryDerivedRegionCells(_) => RecipePortType::BoundaryDerivedRegionCells,
            Self::ShapesLattice(_) => RecipePortType::Lattice,
            Self::ShapesSamples(_) => RecipePortType::ShapesSamples,
            Self::ShapesMappedValues(_) => RecipePortType::ShapesMappedValues,
            Self::ShapesPrimitive(_) => RecipePortType::MarkPrimitive,
            Self::ShapesTransformedMarks(_) => RecipePortType::TransformedMarks,
            Self::CurvesPlacement(_) => RecipePortType::CurvePlacement,
            Self::CurvesSamples(_) => RecipePortType::CurveSamples,
            Self::CurvesMotif(_) => RecipePortType::CurveMotif,
            Self::CurvesDeformedPaths(_) => RecipePortType::CurveDeformedPaths,
            Self::CurvesModulatedPaths(_) => RecipePortType::CurveModulatedPaths,
            Self::CanonicalOutput(_) => RecipePortType::CanonicalGeometry,
        }
    }
}

/// Non-persisted native inputs available to a recipe operation. The source
/// field is intentionally optional because uniform placement never consumes
/// it; TON-011 owns per-channel source selection.
pub struct RecipeExecutionContext<'a> {
    pub artboard: ArtboardSpace,
    pub output_channel: Option<OutputChannelId>,
    /// Optional generic field-request authority. A source-sampling operation
    /// may request its already-declared lattice dimensions through this
    /// boundary, so orchestration never duplicates pattern placement math.
    pub source_field_provider: Option<&'a dyn RecipeSourceFieldProvider>,
    pub source_field: Option<&'a DistributionField>,
    /// Transient source generation provenance supplied by the caller. Recipe
    /// instances remain value-only and never persist this derived metadata.
    pub source_generation: u64,
    /// Transient resolved-field generation provenance supplied by the caller.
    pub resolved_field_generation: u64,
    /// Stable semantic-channel position in the caller's resolved field set.
    /// It preserves canonical region identity across per-channel execution.
    pub semantic_channel_index: u32,
    /// Stable visible-layer position after disabled channels are omitted.
    pub enabled_layer_index: u32,
    /// Definition-owned embedded assets. `PatternDefinition::execute_recipe`
    /// replaces this caller-provided value with its validated immutable asset
    /// list before an operation runs.
    pub definition_assets: &'a [EmbeddedSvgAsset],
    pub cancellation: &'a CancellationToken,
}

pub type RecipeOperationInputs<'a> = BTreeMap<&'static str, &'a RecipeRuntimeValue>;
pub type RecipeOperationParameters<'a> = BTreeMap<&'static str, &'a LiteralValue>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRecipeOperationError(String);

impl NativeRecipeOperationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for NativeRecipeOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for NativeRecipeOperationError {}

/// Supplies semantic source values for a recipe-declared lattice request.
/// Implementations belong to the artwork pipeline/orchestration boundary; a
/// recipe cannot persist or select a provider.
pub trait RecipeSourceFieldProvider {
    fn resolve_source_field(
        &self,
        channel: OutputChannelId,
        columns: u32,
        rows: u32,
        cancellation: &CancellationToken,
    ) -> Result<DistributionField, NativeRecipeOperationError>;
}

pub type NativeRecipeOperation =
    for<'context, 'values> fn(
        &RecipeExecutionContext<'context>,
        &RecipeOperationInputs<'values>,
        &RecipeOperationParameters<'values>,
    ) -> Result<RecipeRuntimeValue, NativeRecipeOperationError>;

/// Registry-trusted preflight for semantic constraints that are stricter than
/// the data-only definition schema. The registry chooses it; recipes cannot.
pub type NativeRecipePreflight = for<'context> fn(
    &PatternDefinition,
    &PatternInstanceParameters,
    &RecipeExecutionContext<'context>,
) -> Result<(), NativeRecipeOperationError>;

#[derive(Clone, Copy)]
pub struct RegisteredNativeRecipeOperation {
    pub id: &'static str,
    pub version: u32,
    pub execute: NativeRecipeOperation,
}

/// Static, bounded pairing of declarative descriptors and native Rust
/// implementations. It deliberately has no plugin, script, or dynamic loader
/// facility.
pub struct NativeRecipeOperationRegistry<'a> {
    descriptors: &'static [RegisteredOperationDescriptor],
    implementations: &'a [RegisteredNativeRecipeOperation],
    preflight: Option<NativeRecipePreflight>,
}

impl<'a> NativeRecipeOperationRegistry<'a> {
    pub const fn new(
        descriptors: &'static [RegisteredOperationDescriptor],
        implementations: &'a [RegisteredNativeRecipeOperation],
    ) -> Self {
        Self {
            descriptors,
            implementations,
            preflight: None,
        }
    }

    pub const fn with_preflight(
        descriptors: &'static [RegisteredOperationDescriptor],
        implementations: &'a [RegisteredNativeRecipeOperation],
        preflight: NativeRecipePreflight,
    ) -> Self {
        Self {
            descriptors,
            implementations,
            preflight: Some(preflight),
        }
    }

    pub const fn descriptors(&self) -> OperationRegistry {
        OperationRegistry::new(self.descriptors)
    }

    pub fn get(&self, id: &str, version: u32) -> Option<&RegisteredNativeRecipeOperation> {
        self.implementations
            .iter()
            .find(|operation| operation.id == id && operation.version == version)
    }

    fn validate(&self) -> Result<(), RecipeExecutionError> {
        let mut implementations = HashSet::new();
        for implementation in self.implementations {
            if !implementations.insert((implementation.id, implementation.version)) {
                return Err(RecipeExecutionError::new(
                    "native operation registry has a duplicate implementation",
                ));
            }
            if !self.descriptors.iter().any(|descriptor| {
                descriptor.id == implementation.id && descriptor.version == implementation.version
            }) {
                return Err(RecipeExecutionError::new(format!(
                    "native operation `{}` version {} has no matching descriptor",
                    implementation.id, implementation.version
                )));
            }
        }
        Ok(())
    }

    fn preflight(
        &self,
        definition: &PatternDefinition,
        instance: &PatternInstanceParameters,
        context: &RecipeExecutionContext<'_>,
    ) -> Result<(), RecipeExecutionError> {
        self.preflight
            .map(|preflight| preflight(definition, instance, context))
            .transpose()
            .map(|_| ())
            .map_err(|error| {
                RecipeExecutionError::new(format!("native recipe preflight failed: {error}"))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternDefinitionError(String);

impl PatternDefinitionError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for PatternDefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PatternDefinitionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternInstanceParametersError(String);

impl PatternInstanceParametersError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for PatternInstanceParametersError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PatternInstanceParametersError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeExecutionError(String);

impl RecipeExecutionError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for RecipeExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RecipeExecutionError {}

impl PatternDefinition {
    pub fn validate(&self) -> Result<(), PatternDefinitionError> {
        self.validate_with_registry(&REGISTERED_OPERATIONS)
    }

    pub fn validate_with_registry(
        &self,
        registry: &OperationRegistry,
    ) -> Result<(), PatternDefinitionError> {
        if self.format_version != TNPATTERN_FORMAT_VERSION {
            return Err(PatternDefinitionError::new(
                "unsupported .tnpattern format version",
            ));
        }
        if self.recipe_version != TNPATTERN_RECIPE_VERSION {
            return Err(PatternDefinitionError::new("unsupported recipe version"));
        }
        if self.display.name.trim().is_empty() || self.display.summary.trim().is_empty() {
            return Err(PatternDefinitionError::new(
                "pattern display metadata is incomplete",
            ));
        }
        if self.outputs.is_empty()
            || self.outputs.iter().collect::<HashSet<_>>().len() != self.outputs.len()
        {
            return Err(PatternDefinitionError::new(
                "pattern output capabilities must be non-empty and unique",
            ));
        }
        if self.parameters.len() > MAX_PATTERN_PARAMETERS || self.assets.len() > MAX_PATTERN_ASSETS
        {
            return Err(PatternDefinitionError::new(
                "pattern definition exceeds resource limits",
            ));
        }
        let asset_digests = self.validate_assets()?;
        let parameters = self.validate_parameters(&asset_digests)?;
        self.validate_layout(&parameters)?;
        self.validate_recipe(&parameters, registry)
    }

    fn validate_parameters(
        &self,
        asset_digests: &HashSet<&str>,
    ) -> Result<HashMap<&str, &PatternParameterDefinition>, PatternDefinitionError> {
        let mut parameters = HashMap::new();
        for parameter in &self.parameters {
            if !is_safe_name(&parameter.key)
                || parameters
                    .insert(parameter.key.as_str(), parameter)
                    .is_some()
            {
                return Err(PatternDefinitionError::new(
                    "pattern parameter keys must be unique stable names",
                ));
            }
            if parameter.label.trim().is_empty()
                || parameter.label.len() > 256
                || parameter.help.trim().is_empty()
                || parameter.help.len() > 1_024
                || parameter.constraints.value_type() != parameter.value_type
            {
                return Err(PatternDefinitionError::new(
                    "pattern parameter creator metadata or constraints are invalid",
                ));
            }
            if !parameter_constraints_are_valid(parameter) {
                return Err(PatternDefinitionError::new(
                    "pattern parameter constraints are invalid",
                ));
            }
            if validate_parameter_value(parameter, &parameter.default, asset_digests).is_err() {
                return Err(PatternDefinitionError::new(format!(
                    "pattern parameter `{}` default has an invalid type, value, or asset reference",
                    parameter.key
                )));
            }
            if parameter.value_type == RecipeValueType::Choice
                && (parameter.choices.is_empty()
                    || parameter.choices.iter().any(|choice| !is_safe_name(choice))
                    || parameter.choices.iter().collect::<HashSet<_>>().len()
                        != parameter.choices.len()
                    || !parameter.choices.contains(match &parameter.default {
                        LiteralValue::Choice(value) => value,
                        _ => unreachable!(),
                    }))
            {
                return Err(PatternDefinitionError::new(
                    "choice parameter default is not declared",
                ));
            }
            if parameter.value_type != RecipeValueType::Choice && !parameter.choices.is_empty() {
                return Err(PatternDefinitionError::new(
                    "only choice parameters may declare choices",
                ));
            }
        }
        let mut controls = HashSet::new();
        for control in &self.quick_controls {
            let Some(parameter) = parameters.get(control.parameter.as_str()) else {
                return Err(PatternDefinitionError::new(
                    "quick control references an unknown parameter",
                ));
            };
            if !is_safe_name(&control.id)
                || control.label.trim().is_empty()
                || !controls.insert(&control.id)
                || control.scope != parameter.scope
            {
                return Err(PatternDefinitionError::new(
                    "quick controls must be unique and match parameter scope",
                ));
            }
            let valid_kind = matches!(
                (control.kind, parameter.value_type),
                (
                    QuickControlKind::Slider,
                    RecipeValueType::Number | RecipeValueType::Integer
                ) | (QuickControlKind::Toggle, RecipeValueType::Boolean)
                    | (QuickControlKind::Choice, RecipeValueType::Choice)
            );
            if !valid_kind {
                return Err(PatternDefinitionError::new(
                    "quick control kind is incompatible with its parameter",
                ));
            }
        }
        Ok(parameters)
    }

    /// Builds a complete current-v1 instance from this definition's defaults.
    /// Callers must supply the active output channels; no historical payload is
    /// ever defaulted or migrated by this method.
    pub fn default_instance_parameters(
        &self,
        channels: impl IntoIterator<Item = OutputChannelId>,
    ) -> Result<PatternInstanceParameters, PatternInstanceParametersError> {
        self.validate()
            .map_err(|error| PatternInstanceParametersError::new(error.to_string()))?;
        let pattern_values = self
            .parameters
            .iter()
            .filter(|parameter| parameter.scope == DefinitionParameterScope::Pattern)
            .map(|parameter| PatternInstanceValue {
                key: parameter.key.clone(),
                value: parameter.default.clone(),
            })
            .collect();
        let output_parameters: Vec<_> = self
            .parameters
            .iter()
            .filter(|parameter| parameter.scope == DefinitionParameterScope::OutputChannel)
            .collect();
        let output_channel_values = if output_parameters.is_empty() {
            Vec::new()
        } else {
            channels
                .into_iter()
                .map(|channel| OutputChannelParameterValues {
                    channel: channel.stable_id().to_owned(),
                    values: output_parameters
                        .iter()
                        .map(|parameter| PatternInstanceValue {
                            key: parameter.key.clone(),
                            value: parameter.default.clone(),
                        })
                        .collect(),
                })
                .collect()
        };
        let instance = PatternInstanceParameters {
            format_version: TNPATTERN_INSTANCE_FORMAT_VERSION,
            pattern_id: self.id.clone(),
            pattern_values,
            output_channel_values,
        };
        self.validate_instance_parameters(&instance)?;
        Ok(instance)
    }

    /// Validates a complete instance before any recipe execution boundary.
    pub fn validate_instance_parameters(
        &self,
        instance: &PatternInstanceParameters,
    ) -> Result<(), PatternInstanceParametersError> {
        self.validate_instance_parameters_with_registry(instance, &REGISTERED_OPERATIONS)
    }

    pub fn validate_instance_parameters_with_registry(
        &self,
        instance: &PatternInstanceParameters,
        registry: &OperationRegistry,
    ) -> Result<(), PatternInstanceParametersError> {
        self.validate_with_registry(registry)
            .map_err(|error| PatternInstanceParametersError::new(error.to_string()))?;
        if instance.format_version != TNPATTERN_INSTANCE_FORMAT_VERSION {
            return Err(PatternInstanceParametersError::new(
                "unsupported .tnpattern instance format version",
            ));
        }
        if instance.pattern_id != self.id {
            return Err(PatternInstanceParametersError::new(
                "instance pattern ID does not match its definition",
            ));
        }
        if instance.output_channel_values.len() > MAX_PATTERN_INSTANCE_OUTPUT_CHANNELS {
            return Err(PatternInstanceParametersError::new(
                "instance exceeds output-channel resource limit",
            ));
        }
        let total_values = instance
            .output_channel_values
            .iter()
            .try_fold(instance.pattern_values.len(), |total, channel| {
                total.checked_add(channel.values.len())
            })
            .ok_or_else(|| {
                PatternInstanceParametersError::new("instance exceeds value resource limit")
            })?;
        if total_values > MAX_PATTERN_INSTANCE_VALUES {
            return Err(PatternInstanceParametersError::new(
                "instance exceeds value resource limit",
            ));
        }

        let asset_digests = self
            .assets
            .iter()
            .map(|asset| asset.digest.as_str())
            .collect::<HashSet<_>>();
        let parameters = self
            .validate_parameters(&asset_digests)
            .map_err(|error| PatternInstanceParametersError::new(error.to_string()))?;
        validate_instance_values(
            &instance.pattern_values,
            DefinitionParameterScope::Pattern,
            None,
            &parameters,
            &asset_digests,
        )?;

        let has_output_parameters = self
            .parameters
            .iter()
            .any(|parameter| parameter.scope == DefinitionParameterScope::OutputChannel);
        if has_output_parameters && instance.output_channel_values.is_empty() {
            return Err(PatternInstanceParametersError::new(
                "instance is missing output-channel parameter values",
            ));
        }
        if !has_output_parameters && !instance.output_channel_values.is_empty() {
            return Err(PatternInstanceParametersError::new(
                "instance supplies output-channel values but the definition has none",
            ));
        }
        let mut channels = HashSet::new();
        for channel_values in &instance.output_channel_values {
            if channel_values.channel.parse::<OutputChannelId>().is_err() {
                return Err(PatternInstanceParametersError::new(format!(
                    "instance references unknown output channel `{}`",
                    channel_values.channel
                )));
            }
            if !channels.insert(channel_values.channel.as_str()) {
                return Err(PatternInstanceParametersError::new(format!(
                    "instance has duplicate output channel `{}`",
                    channel_values.channel
                )));
            }
            validate_instance_values(
                &channel_values.values,
                DefinitionParameterScope::OutputChannel,
                Some(channel_values.channel.as_str()),
                &parameters,
                &asset_digests,
            )?;
        }
        Ok(())
    }

    /// Executes this validated data-only recipe through explicitly registered,
    /// bounded Rust operations. No definition can supply executable code.
    pub fn execute_recipe(
        &self,
        instance: &PatternInstanceParameters,
        context: &RecipeExecutionContext<'_>,
        operations: &NativeRecipeOperationRegistry<'_>,
    ) -> Result<CanonicalPatternOutput, RecipeExecutionError> {
        context
            .cancellation
            .checkpoint()
            .map_err(|_| RecipeExecutionError::new("recipe execution cancelled before start"))?;
        context.artboard.validate().map_err(|error| {
            RecipeExecutionError::new(format!("invalid execution artboard: {error}"))
        })?;
        operations.validate()?;
        let descriptors = operations.descriptors();
        self.validate_with_registry(&descriptors).map_err(|error| {
            RecipeExecutionError::new(format!("invalid recipe definition: {error}"))
        })?;
        self.validate_instance_parameters_with_registry(instance, &descriptors)
            .map_err(|error| {
                RecipeExecutionError::new(format!("invalid recipe instance: {error}"))
            })?;
        operations.preflight(self, instance, context)?;
        let resolved_parameters =
            self.resolve_instance_parameters(instance, context.output_channel)?;
        let operation_context = RecipeExecutionContext {
            artboard: context.artboard,
            output_channel: context.output_channel,
            source_field_provider: context.source_field_provider,
            source_field: context.source_field,
            source_generation: context.source_generation,
            resolved_field_generation: context.resolved_field_generation,
            semantic_channel_index: context.semantic_channel_index,
            enabled_layer_index: context.enabled_layer_index,
            definition_assets: &self.assets,
            cancellation: context.cancellation,
        };
        let nodes = self
            .recipe
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<BTreeMap<_, _>>();
        let mut incoming = BTreeMap::<&str, Vec<&RecipeEdge>>::new();
        let mut successors = BTreeMap::<&str, Vec<&str>>::new();
        let mut indegrees = nodes
            .keys()
            .map(|node| (*node, 0usize))
            .collect::<BTreeMap<_, _>>();
        for edge in &self.recipe.edges {
            incoming
                .entry(edge.to.node.as_str())
                .or_default()
                .push(edge);
            successors
                .entry(edge.from.node.as_str())
                .or_default()
                .push(edge.to.node.as_str());
            *indegrees
                .get_mut(edge.to.node.as_str())
                .expect("definition validation guarantees recipe nodes") += 1;
        }
        let mut ready = indegrees
            .iter()
            .filter_map(|(node, degree)| (*degree == 0).then_some(*node))
            .collect::<std::collections::BTreeSet<_>>();
        let mut values = BTreeMap::<&str, RecipeRuntimeValue>::new();
        while let Some(node_id) = ready.pop_first() {
            context.cancellation.checkpoint().map_err(|_| {
                RecipeExecutionError::new(format!(
                    "recipe execution cancelled before node `{node_id}`"
                ))
            })?;
            let node = nodes[node_id];
            let descriptor = descriptors
                .get(&node.operation.id, node.operation.version)
                .expect("definition validation guarantees operation descriptor");
            let implementation = operations
                .get(&node.operation.id, node.operation.version)
                .ok_or_else(|| {
                    RecipeExecutionError::new(format!(
                        "node `{node_id}` operation `{}` version {} has no native implementation",
                        node.operation.id, node.operation.version
                    ))
                })?;
            let inputs = runtime_inputs_for_node(node, descriptor, &incoming, &values)?;
            let parameters = runtime_parameters_for_node(node, descriptor, &resolved_parameters)?;
            let value = (implementation.execute)(&operation_context, &inputs, &parameters)
                .map_err(|error| {
                    RecipeExecutionError::new(format!(
                        "node `{node_id}` operation `{}` version {} failed: {error}",
                        node.operation.id, node.operation.version
                    ))
                })?;
            if value.port_type() != descriptor.output.kind {
                return Err(RecipeExecutionError::new(format!(
                    "node `{node_id}` operation `{}` returned {:?}, expected {:?}",
                    node.operation.id,
                    value.port_type(),
                    descriptor.output.kind
                )));
            }
            values.insert(node_id, value);
            for successor in successors.get(node_id).into_iter().flatten() {
                let degree = indegrees
                    .get_mut(successor)
                    .expect("definition validation guarantees recipe nodes");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(successor);
                }
            }
        }
        context
            .cancellation
            .checkpoint()
            .map_err(|_| RecipeExecutionError::new("recipe execution cancelled before output"))?;
        let output = values
            .remove(self.recipe.output.node.as_str())
            .ok_or_else(|| RecipeExecutionError::new("recipe execution produced no output"))?;
        let RecipeRuntimeValue::CanonicalOutput(output) = output else {
            return Err(RecipeExecutionError::new(
                "recipe execution output is not canonical geometry",
            ));
        };
        output.validate().map_err(|error| {
            RecipeExecutionError::new(format!(
                "recipe execution returned invalid canonical output: {error}"
            ))
        })?;
        if output.artboard() != context.artboard {
            return Err(RecipeExecutionError::new(
                "recipe execution canonical output artboard does not match its context",
            ));
        }
        let actual = canonical_output_capabilities(&output);
        let output_descriptor = descriptors
            .get(
                &nodes[self.recipe.output.node.as_str()].operation.id,
                nodes[self.recipe.output.node.as_str()].operation.version,
            )
            .expect("definition validation guarantees output descriptor");
        if !same_output_capabilities(&actual, output_descriptor.canonical_output_kinds)
            || !same_output_capabilities(&actual, &self.outputs)
        {
            return Err(RecipeExecutionError::new(
                "recipe execution canonical output does not match declared output capabilities",
            ));
        }
        Ok(output)
    }

    fn resolve_instance_parameters<'a>(
        &'a self,
        instance: &'a PatternInstanceParameters,
        channel: Option<OutputChannelId>,
    ) -> Result<HashMap<&'a str, &'a LiteralValue>, RecipeExecutionError> {
        let mut values = instance
            .pattern_values
            .iter()
            .map(|value| (value.key.as_str(), &value.value))
            .collect::<HashMap<_, _>>();
        if self
            .parameters
            .iter()
            .any(|parameter| parameter.scope == DefinitionParameterScope::OutputChannel)
        {
            let channel = channel.ok_or_else(|| {
                RecipeExecutionError::new(
                    "recipe has output-channel parameters but execution context selects no channel",
                )
            })?;
            let channel_values = instance
                .output_channel_values
                .iter()
                .find(|values| values.channel == channel.stable_id())
                .ok_or_else(|| {
                    RecipeExecutionError::new(format!(
                        "recipe instance has no values for selected output channel `{}`",
                        channel.stable_id()
                    ))
                })?;
            values.extend(
                channel_values
                    .values
                    .iter()
                    .map(|value| (value.key.as_str(), &value.value)),
            );
        }
        Ok(values)
    }

    fn validate_layout(
        &self,
        parameters: &HashMap<&str, &PatternParameterDefinition>,
    ) -> Result<(), PatternDefinitionError> {
        let mut section_ids = HashSet::new();
        let mut listed = HashSet::new();
        for section in &self.layout.sections {
            if !is_safe_name(&section.id)
                || section.label.trim().is_empty()
                || !section_ids.insert(&section.id)
            {
                return Err(PatternDefinitionError::new(
                    "authoring sections must have unique stable IDs",
                ));
            }
            for parameter in &section.parameters {
                if !parameters.contains_key(parameter.as_str()) || !listed.insert(parameter) {
                    return Err(PatternDefinitionError::new(
                        "authoring layout has an invalid parameter reference",
                    ));
                }
            }
        }
        for (node, position) in &self.layout.node_positions {
            if !is_safe_name(node) || !position.x.is_finite() || !position.y.is_finite() {
                return Err(PatternDefinitionError::new(
                    "authoring graph position is invalid",
                ));
            }
        }
        Ok(())
    }

    fn validate_assets(&self) -> Result<HashSet<&str>, PatternDefinitionError> {
        let mut digests = HashSet::new();
        let mut total_svg_bytes = 0usize;
        for asset in &self.assets {
            if !is_sha256_digest(&asset.digest) || !digests.insert(asset.digest.as_str()) {
                return Err(PatternDefinitionError::new(
                    "embedded SVG asset has an invalid or duplicate digest",
                ));
            }
            total_svg_bytes = total_svg_bytes
                .checked_add(asset.svg.len())
                .ok_or_else(|| {
                    PatternDefinitionError::new("embedded SVG assets exceed total byte limit")
                })?;
            if asset.svg.len() > MAX_EMBEDDED_SVG_BYTES
                || total_svg_bytes > MAX_TOTAL_EMBEDDED_SVG_BYTES
            {
                return Err(PatternDefinitionError::new(
                    "embedded SVG assets exceed byte limits",
                ));
            }
            if !is_safe_svg(&asset.svg)
                || usvg::Tree::from_str(&asset.svg, &usvg::Options::default()).is_err()
            {
                return Err(PatternDefinitionError::new(
                    "embedded SVG asset contains unsafe content or references",
                ));
            }
            if asset.digest != sha256_digest(&asset.svg) {
                return Err(PatternDefinitionError::new(
                    "embedded SVG asset digest does not match its exact UTF-8 bytes",
                ));
            }
        }
        Ok(digests)
    }

    fn validate_recipe(
        &self,
        parameters: &HashMap<&str, &PatternParameterDefinition>,
        registry: &OperationRegistry,
    ) -> Result<(), PatternDefinitionError> {
        if self.recipe.nodes.is_empty()
            || self.recipe.nodes.len() > MAX_PATTERN_NODES
            || self.recipe.edges.len() > MAX_PATTERN_EDGES
        {
            return Err(PatternDefinitionError::new(
                "recipe graph violates node or edge limits",
            ));
        }
        let asset_digests: HashSet<_> = self
            .assets
            .iter()
            .map(|asset| asset.digest.as_str())
            .collect();
        let mut nodes = HashMap::new();
        let mut descriptors = HashMap::new();
        for node in &self.recipe.nodes {
            if !is_safe_name(&node.id) || nodes.insert(node.id.as_str(), node).is_some() {
                return Err(PatternDefinitionError::new(
                    "recipe node IDs must be unique stable names",
                ));
            }
            let descriptor = registry
                .get(&node.operation.id, node.operation.version)
                .ok_or_else(|| {
                    if registry.operation_exists(&node.operation.id) {
                        PatternDefinitionError::new(
                            "recipe references an unsupported operation version",
                        )
                    } else {
                        PatternDefinitionError::new("recipe references an unknown operation")
                    }
                })?;
            validate_operation_arguments(node, descriptor, parameters, &asset_digests)?;
            descriptors.insert(node.id.as_str(), descriptor);
        }
        let output_node = nodes.get(self.recipe.output.node.as_str()).ok_or_else(|| {
            PatternDefinitionError::new("recipe output references a missing node")
        })?;
        let output_descriptor = descriptors[output_node.id.as_str()];
        if self.recipe.output.port != output_descriptor.output.name
            || output_descriptor.output.kind != RecipePortType::CanonicalGeometry
        {
            return Err(PatternDefinitionError::new(
                "recipe output must be canonical geometry",
            ));
        }
        if !same_output_capabilities(&self.outputs, output_descriptor.canonical_output_kinds) {
            return Err(PatternDefinitionError::new(
                "pattern output capabilities do not match the canonical output operation",
            ));
        }

        let mut incoming = HashSet::new();
        let mut reverse: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut forward: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.recipe.edges {
            let from = nodes
                .get(edge.from.node.as_str())
                .ok_or_else(|| PatternDefinitionError::new("recipe edge source node is missing"))?;
            let to = nodes.get(edge.to.node.as_str()).ok_or_else(|| {
                PatternDefinitionError::new("recipe edge destination node is missing")
            })?;
            let from_descriptor = descriptors[from.id.as_str()];
            let to_descriptor = descriptors[to.id.as_str()];
            if edge.from.port != from_descriptor.output.name {
                return Err(PatternDefinitionError::new(
                    "recipe edge source is not an output port",
                ));
            }
            let Some(input) = to_descriptor
                .inputs
                .iter()
                .find(|input| input.name == edge.to.port)
            else {
                return Err(PatternDefinitionError::new(
                    "recipe edge destination is not an input port",
                ));
            };
            if input.kind != from_descriptor.output.kind {
                return Err(PatternDefinitionError::new(format!(
                    "recipe edge `{}.{}` {:?} is incompatible with `{}.{}` {:?}",
                    edge.from.node,
                    edge.from.port,
                    from_descriptor.output.kind,
                    edge.to.node,
                    edge.to.port,
                    input.kind,
                )));
            }
            if !incoming.insert((edge.to.node.as_str(), edge.to.port.as_str())) {
                return Err(PatternDefinitionError::new(
                    "recipe edges have duplicate destination ports",
                ));
            }
            reverse
                .entry(to.id.as_str())
                .or_default()
                .push(from.id.as_str());
            forward
                .entry(from.id.as_str())
                .or_default()
                .push(to.id.as_str());
        }
        for node in &self.recipe.nodes {
            let descriptor = descriptors[node.id.as_str()];
            for input in descriptor.inputs {
                if !incoming.contains(&(node.id.as_str(), input.name)) {
                    return Err(PatternDefinitionError::new(
                        "recipe node has a missing required input",
                    ));
                }
            }
        }
        if graph_has_cycle(
            &forward,
            self.recipe.nodes.iter().map(|node| node.id.as_str()),
        ) {
            return Err(PatternDefinitionError::new("recipe graph contains a cycle"));
        }
        let mut reachable = HashSet::new();
        collect_reverse(self.recipe.output.node.as_str(), &reverse, &mut reachable);
        if reachable.len() != self.recipe.nodes.len() {
            return Err(PatternDefinitionError::new(
                "recipe graph contains nodes unreachable from its output",
            ));
        }
        Ok(())
    }
}

fn validate_operation_arguments(
    node: &RecipeNode,
    descriptor: &RegisteredOperationDescriptor,
    parameters: &HashMap<&str, &PatternParameterDefinition>,
    asset_digests: &HashSet<&str>,
) -> Result<(), PatternDefinitionError> {
    if node.parameters.len() != descriptor.parameters.len() {
        return Err(PatternDefinitionError::new(
            "recipe operation has missing or extra parameters",
        ));
    }
    for expected in descriptor.parameters {
        let argument = node
            .parameters
            .get(expected.name)
            .ok_or_else(|| PatternDefinitionError::new("recipe operation parameter is missing"))?;
        match argument {
            RecipeArgument::Literal(value)
                if value.value_type() == expected.value_type && literal_is_safe(value) =>
            {
                if let LiteralValue::SvgAsset(digest) = value
                    && !asset_digests.contains(digest.as_str())
                {
                    return Err(PatternDefinitionError::new(
                        "recipe references an unknown SVG asset digest",
                    ));
                }
            }
            RecipeArgument::Parameter(key) => {
                let parameter = parameters.get(key.as_str()).ok_or_else(|| {
                    PatternDefinitionError::new("recipe references an unknown parameter")
                })?;
                if parameter.value_type != expected.value_type {
                    return Err(PatternDefinitionError::new(
                        "recipe parameter reference has an incompatible type",
                    ));
                }
            }
            _ => {
                return Err(PatternDefinitionError::new(
                    "recipe operation parameter has an invalid value",
                ));
            }
        }
    }
    Ok(())
}

fn same_output_capabilities(declared: &[PatternOutputKind], native: &[PatternOutputKind]) -> bool {
    declared.len() == native.len()
        && declared.iter().all(|kind| native.contains(kind))
        && native.iter().all(|kind| declared.contains(kind))
}

fn runtime_inputs_for_node<'a>(
    node: &RecipeNode,
    descriptor: &RegisteredOperationDescriptor,
    incoming: &BTreeMap<&str, Vec<&RecipeEdge>>,
    values: &'a BTreeMap<&str, RecipeRuntimeValue>,
) -> Result<RecipeOperationInputs<'a>, RecipeExecutionError> {
    let mut inputs = BTreeMap::new();
    for expected in descriptor.inputs {
        let edge = incoming
            .get(node.id.as_str())
            .into_iter()
            .flatten()
            .find(|edge| edge.to.port == expected.name)
            .expect("definition validation guarantees required input edges");
        let value = values.get(edge.from.node.as_str()).ok_or_else(|| {
            RecipeExecutionError::new(format!(
                "node `{}` input `{}` is unavailable from `{}`",
                node.id, expected.name, edge.from.node
            ))
        })?;
        if value.port_type() != expected.kind {
            return Err(RecipeExecutionError::new(format!(
                "node `{}` input `{}` has runtime type {:?}, expected {:?}",
                node.id,
                expected.name,
                value.port_type(),
                expected.kind
            )));
        }
        inputs.insert(expected.name, value);
    }
    Ok(inputs)
}

fn runtime_parameters_for_node<'a>(
    node: &'a RecipeNode,
    descriptor: &RegisteredOperationDescriptor,
    resolved: &HashMap<&str, &'a LiteralValue>,
) -> Result<RecipeOperationParameters<'a>, RecipeExecutionError> {
    let mut parameters = BTreeMap::new();
    for expected in descriptor.parameters {
        let argument = node
            .parameters
            .get(expected.name)
            .expect("definition validation guarantees operation parameters");
        let value = match argument {
            RecipeArgument::Literal(value) => value,
            RecipeArgument::Parameter(key) => {
                resolved.get(key.as_str()).copied().ok_or_else(|| {
                    RecipeExecutionError::new(format!(
                        "node `{}` parameter `{}` is unresolved",
                        node.id, key
                    ))
                })?
            }
        };
        if value.value_type() != expected.value_type {
            return Err(RecipeExecutionError::new(format!(
                "node `{}` parameter `{}` has runtime type {:?}, expected {:?}",
                node.id,
                expected.name,
                value.value_type(),
                expected.value_type
            )));
        }
        parameters.insert(expected.name, value);
    }
    Ok(parameters)
}

fn canonical_output_capabilities(output: &CanonicalPatternOutput) -> Vec<PatternOutputKind> {
    match output {
        CanonicalPatternOutput::Marks(_) => vec![PatternOutputKind::Marks],
        CanonicalPatternOutput::Paths(_) => vec![PatternOutputKind::Paths],
        CanonicalPatternOutput::Regions(_) => vec![PatternOutputKind::Regions],
        CanonicalPatternOutput::Network(_) => vec![PatternOutputKind::Networks],
        CanonicalPatternOutput::Composite(output) => {
            let mut capabilities = Vec::new();
            if output.regions.is_some() {
                capabilities.push(PatternOutputKind::Regions);
            }
            if output.network.is_some() {
                capabilities.push(PatternOutputKind::Networks);
            }
            capabilities
        }
    }
}

fn parameter_constraints_are_valid(parameter: &PatternParameterDefinition) -> bool {
    match &parameter.constraints {
        PatternParameterConstraints::Number {
            minimum,
            maximum,
            step,
        } => {
            minimum.is_finite()
                && maximum.is_finite()
                && step.is_finite()
                && minimum <= maximum
                && *step > 0.0
        }
        PatternParameterConstraints::Integer {
            minimum,
            maximum,
            step,
        } => minimum <= maximum && *step > 0,
        PatternParameterConstraints::Boolean | PatternParameterConstraints::Choice => true,
        PatternParameterConstraints::Text { max_length } => *max_length <= MAX_TEXT_PARAMETER_BYTES,
        PatternParameterConstraints::SvgAsset => true,
    }
}

fn validate_parameter_value(
    parameter: &PatternParameterDefinition,
    value: &LiteralValue,
    asset_digests: &HashSet<&str>,
) -> Result<(), &'static str> {
    if value.value_type() != parameter.value_type || !literal_is_safe(value) {
        return Err("has an incompatible type or unsafe value");
    }
    match (&parameter.constraints, value) {
        (
            PatternParameterConstraints::Number {
                minimum,
                maximum,
                step,
            },
            LiteralValue::Number(value),
        ) if *value >= *minimum
            && *value <= *maximum
            && is_on_number_step(*value, *minimum, *step) =>
        {
            Ok(())
        }
        (
            PatternParameterConstraints::Integer {
                minimum,
                maximum,
                step,
            },
            LiteralValue::Integer(value),
        ) if *value >= *minimum && *value <= *maximum && (*value - *minimum) % *step == 0 => Ok(()),
        (PatternParameterConstraints::Boolean, LiteralValue::Boolean(_)) => Ok(()),
        (PatternParameterConstraints::Text { max_length }, LiteralValue::Text(value))
            if value.len() <= *max_length =>
        {
            Ok(())
        }
        (PatternParameterConstraints::Choice, LiteralValue::Choice(value))
            if parameter.choices.iter().any(|choice| choice == value) =>
        {
            Ok(())
        }
        (PatternParameterConstraints::SvgAsset, LiteralValue::SvgAsset(digest))
            if asset_digests.contains(digest.as_str()) =>
        {
            Ok(())
        }
        _ => Err("violates its declared constraints"),
    }
}

fn is_on_number_step(value: f64, minimum: f64, step: f64) -> bool {
    let quotient = (value - minimum) / step;
    // A definition may truthfully span the complete finite f64 domain for a
    // continuous model value. At that magnitude a UI-oriented step cannot be
    // represented without overflow; the bounds and finite-value checks remain
    // authoritative, so do not reject the value solely for that overflow.
    if !quotient.is_finite() {
        return true;
    }
    let nearest = quotient.round();
    (quotient - nearest).abs() <= 1e-9 * quotient.abs().max(1.0)
}

fn validate_instance_values(
    values: &[PatternInstanceValue],
    actual_scope: DefinitionParameterScope,
    channel: Option<&str>,
    parameters: &HashMap<&str, &PatternParameterDefinition>,
    asset_digests: &HashSet<&str>,
) -> Result<(), PatternInstanceParametersError> {
    let mut values_by_key = HashSet::new();
    for entry in values {
        let location = channel
            .map(|channel| format!(" for output channel `{channel}`"))
            .unwrap_or_default();
        let parameter = parameters.get(entry.key.as_str()).ok_or_else(|| {
            PatternInstanceParametersError::new(format!(
                "instance has unknown parameter `{}`{location}",
                entry.key
            ))
        })?;
        if parameter.scope != actual_scope {
            return Err(PatternInstanceParametersError::new(format!(
                "instance parameter `{}` has a scope mismatch{location}",
                entry.key
            )));
        }
        if !values_by_key.insert(entry.key.as_str()) {
            return Err(PatternInstanceParametersError::new(format!(
                "instance has duplicate parameter `{}`{location}",
                entry.key
            )));
        }
        validate_parameter_value(parameter, &entry.value, asset_digests).map_err(|reason| {
            PatternInstanceParametersError::new(format!(
                "instance parameter `{}` {reason}{location}",
                entry.key
            ))
        })?;
    }
    for parameter in parameters.values() {
        if parameter.scope == actual_scope && !values_by_key.contains(parameter.key.as_str()) {
            let location = channel
                .map(|channel| format!(" for output channel `{channel}`"))
                .unwrap_or_default();
            return Err(PatternInstanceParametersError::new(format!(
                "instance is missing parameter `{}`{location}",
                parameter.key
            )));
        }
    }
    Ok(())
}

fn collect_reverse<'a>(
    node: &'a str,
    reverse: &HashMap<&'a str, Vec<&'a str>>,
    reached: &mut HashSet<&'a str>,
) {
    if reached.insert(node)
        && let Some(inputs) = reverse.get(node)
    {
        for input in inputs {
            collect_reverse(input, reverse, reached);
        }
    }
}

fn graph_has_cycle<'a>(
    forward: &HashMap<&'a str, Vec<&'a str>>,
    mut nodes: impl Iterator<Item = &'a str>,
) -> bool {
    fn visit<'a>(
        node: &'a str,
        forward: &HashMap<&'a str, Vec<&'a str>>,
        active: &mut HashSet<&'a str>,
        done: &mut HashSet<&'a str>,
    ) -> bool {
        if !active.insert(node) {
            return true;
        }
        if let Some(next) = forward.get(node) {
            for target in next {
                if !done.contains(target) && visit(target, forward, active, done) {
                    return true;
                }
            }
        }
        active.remove(node);
        done.insert(node);
        false
    }
    let mut active = HashSet::new();
    let mut done = HashSet::new();
    nodes.any(|node| !done.contains(node) && visit(node, forward, &mut active, &mut done))
}

fn is_safe_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn literal_is_safe(value: &LiteralValue) -> bool {
    match value {
        LiteralValue::Number(value) => value.is_finite(),
        LiteralValue::Text(value) => value.len() <= MAX_TEXT_PARAMETER_BYTES,
        LiteralValue::Choice(value) => !value.is_empty() && value.len() <= 256,
        LiteralValue::SvgAsset(value) => is_sha256_digest(value),
        _ => true,
    }
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_digest(svg: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(svg.as_bytes()))
}

fn is_safe_svg(svg: &str) -> bool {
    let lowered = svg
        .to_ascii_lowercase()
        .replace("http://www.w3.org/2000/svg", "");
    lowered.contains("<svg")
        && !lowered.contains("<script")
        && !lowered.contains("<foreignobject")
        && !lowered.contains("javascript:")
        && !lowered.contains("data:")
        && !lowered.contains("http:")
        && !lowered.contains("https:")
        && !lowered.contains("file:")
        && !lowered.contains("//")
        && !lowered.contains(" onload=")
        && !lowered.contains(" onclick=")
        && !lowered.contains(" onerror=")
        && has_only_fragment_references(&lowered, "href=")
        && has_only_fragment_references(&lowered, "url(")
}

fn has_only_fragment_references(svg: &str, marker: &str) -> bool {
    let mut remaining = svg;
    while let Some(index) = remaining.find(marker) {
        let value = remaining[index + marker.len()..].trim_start();
        let value = value
            .strip_prefix('"')
            .or_else(|| value.strip_prefix('\''))
            .unwrap_or(value);
        if !value.starts_with('#') {
            return false;
        }
        remaining = &value[1..];
    }
    true
}

/// Parses one UTF-8 `.tnpattern` v1 file and validates every declarative boundary.
pub fn parse_tnpattern(bytes: &[u8]) -> Result<PatternDefinition, PatternDefinitionError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| PatternDefinitionError::new(".tnpattern must be UTF-8 JSON"))?;
    let definition: PatternDefinition = serde_json::from_str(text).map_err(|error| {
        PatternDefinitionError::new(format!("invalid .tnpattern JSON: {error}"))
    })?;
    definition.validate()?;
    Ok(definition)
}

/// Serializes one valid v1 definition using ordered structs and maps, producing stable JSON bytes.
pub fn serialize_tnpattern(
    definition: &PatternDefinition,
) -> Result<Vec<u8>, PatternDefinitionError> {
    definition.validate()?;
    serde_json::to_vec(definition).map_err(|error| {
        PatternDefinitionError::new(format!("cannot serialize .tnpattern: {error}"))
    })
}

/// Parses and fully validates a distinct `.tnpattern` instance payload.
pub fn parse_tnpattern_instance_parameters(
    definition: &PatternDefinition,
    bytes: &[u8],
) -> Result<PatternInstanceParameters, PatternInstanceParametersError> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        PatternInstanceParametersError::new(".tnpattern instance must be UTF-8 JSON")
    })?;
    let instance: PatternInstanceParameters = serde_json::from_str(text).map_err(|error| {
        PatternInstanceParametersError::new(format!("invalid .tnpattern instance JSON: {error}"))
    })?;
    definition.validate_instance_parameters(&instance)?;
    Ok(instance)
}

/// Serializes a validated v1 instance using a canonical entry ordering.
pub fn serialize_tnpattern_instance_parameters(
    definition: &PatternDefinition,
    instance: &PatternInstanceParameters,
) -> Result<Vec<u8>, PatternInstanceParametersError> {
    definition.validate_instance_parameters(instance)?;
    let mut canonical = instance.clone();
    canonical
        .pattern_values
        .sort_by(|left, right| left.key.cmp(&right.key));
    canonical
        .output_channel_values
        .sort_by(|left, right| left.channel.cmp(&right.channel));
    for channel in &mut canonical.output_channel_values {
        channel
            .values
            .sort_by(|left, right| left.key.cmp(&right.key));
    }
    serde_json::to_vec(&canonical).map_err(|error| {
        PatternInstanceParametersError::new(format!(
            "cannot serialize .tnpattern instance: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pattern::{MarkPatternOutput, RegionPatternOutput},
        render::MarkSet,
    };
    use std::sync::{Mutex, OnceLock};

    fn definition() -> PatternDefinition {
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><path d=\"M0 0\"/></svg>".to_owned();
        let mut arrangement = parameter(
            "arrangement",
            DefinitionParameterScope::OutputChannel,
            RecipeValueType::Choice,
            LiteralValue::Choice("shared".into()),
            PatternParameterConstraints::Choice,
        );
        arrangement.choices = vec!["shared".into(), "independent".into()];
        let mut placement = parameter(
            "placement",
            DefinitionParameterScope::OutputChannel,
            RecipeValueType::Choice,
            LiteralValue::Choice("source-weighted".into()),
            PatternParameterConstraints::Choice,
        );
        placement.choices = vec!["source-weighted".into(), "uniform".into()];
        let mut density_polarity = parameter(
            "density-polarity",
            DefinitionParameterScope::OutputChannel,
            RecipeValueType::Choice,
            LiteralValue::Choice("darker-more-dense".into()),
            PatternParameterConstraints::Choice,
        );
        density_polarity.choices = vec!["darker-more-dense".into(), "lighter-more-dense".into()];
        let parameters = vec![
            parameter(
                "enabled",
                DefinitionParameterScope::OutputChannel,
                RecipeValueType::Boolean,
                LiteralValue::Boolean(true),
                PatternParameterConstraints::Boolean,
            ),
            arrangement,
            parameter(
                "cell-count",
                DefinitionParameterScope::OutputChannel,
                RecipeValueType::Integer,
                LiteralValue::Integer(256),
                PatternParameterConstraints::Integer {
                    minimum: 2,
                    maximum: 8_192,
                    step: 1,
                },
            ),
            parameter(
                "seed",
                DefinitionParameterScope::OutputChannel,
                RecipeValueType::Integer,
                LiteralValue::Integer(0),
                PatternParameterConstraints::Integer {
                    minimum: 0,
                    maximum: u64::MAX,
                    step: 1,
                },
            ),
            parameter(
                "boundary-gap",
                DefinitionParameterScope::OutputChannel,
                RecipeValueType::Number,
                LiteralValue::Number(1.0),
                PatternParameterConstraints::Number {
                    minimum: 0.0,
                    maximum: 64.0,
                    step: 0.25,
                },
            ),
            placement,
            density_polarity,
            parameter(
                "density-strength",
                DefinitionParameterScope::OutputChannel,
                RecipeValueType::Number,
                LiteralValue::Number(1.0),
                PatternParameterConstraints::Number {
                    minimum: 0.001,
                    maximum: 16.0,
                    step: 0.001,
                },
            ),
            parameter(
                "response-strength",
                DefinitionParameterScope::OutputChannel,
                RecipeValueType::Number,
                LiteralValue::Number(1.0),
                PatternParameterConstraints::Number {
                    minimum: 0.0,
                    maximum: 16.0,
                    step: 0.05,
                },
            ),
            parameter(
                "minimum-cell-scale",
                DefinitionParameterScope::OutputChannel,
                RecipeValueType::Number,
                LiteralValue::Number(0.0),
                PatternParameterConstraints::Number {
                    minimum: 0.0,
                    maximum: 1.0,
                    step: 0.01,
                },
            ),
        ];
        let node =
            |id: &str, operation: &str, parameters: BTreeMap<String, RecipeArgument>| RecipeNode {
                id: id.into(),
                operation: OperationReference {
                    id: operation.into(),
                    version: 1,
                },
                parameters,
            };
        let site_parameters = BTreeMap::from([
            (
                "cell-count".into(),
                RecipeArgument::Parameter("cell-count".into()),
            ),
            ("seed".into(), RecipeArgument::Parameter("seed".into())),
            (
                "arrangement".into(),
                RecipeArgument::Parameter("arrangement".into()),
            ),
            (
                "placement".into(),
                RecipeArgument::Parameter("placement".into()),
            ),
            (
                "density-polarity".into(),
                RecipeArgument::Parameter("density-polarity".into()),
            ),
            (
                "density-strength".into(),
                RecipeArgument::Parameter("density-strength".into()),
            ),
        ]);
        let inset_parameters = BTreeMap::from([
            (
                "response-strength".into(),
                RecipeArgument::Parameter("response-strength".into()),
            ),
            (
                "minimum-cell-scale".into(),
                RecipeArgument::Parameter("minimum-cell-scale".into()),
            ),
            (
                "boundary-gap".into(),
                RecipeArgument::Parameter("boundary-gap".into()),
            ),
        ]);
        let emit_parameters = BTreeMap::from([(
            "enabled".into(),
            RecipeArgument::Parameter("enabled".into()),
        )]);
        PatternDefinition {
            format_version: 1,
            recipe_version: 1,
            id: PatternId::new("custom.v1").unwrap(),
            display: PatternDisplayMetadata {
                name: "Custom".into(),
                summary: "A test recipe".into(),
            },
            family: PatternFamily::StochasticDistributions,
            outputs: vec![PatternOutputKind::Regions],
            parameters,
            quick_controls: vec![QuickControlDefinition {
                id: "cell-count".into(),
                parameter: "cell-count".into(),
                scope: DefinitionParameterScope::OutputChannel,
                kind: QuickControlKind::Slider,
                label: "Cell Count".into(),
            }],
            layout: AuthoringLayout {
                sections: vec![AuthoringSection {
                    id: "weighted-voronoi".into(),
                    label: "Weighted Voronoi".into(),
                    parameters: vec![
                        "enabled".into(),
                        "arrangement".into(),
                        "cell-count".into(),
                        "seed".into(),
                        "boundary-gap".into(),
                        "placement".into(),
                        "density-polarity".into(),
                        "density-strength".into(),
                        "response-strength".into(),
                        "minimum-cell-scale".into(),
                    ],
                }],
                node_positions: BTreeMap::new(),
            },
            recipe: RecipeGraph {
                nodes: vec![
                    node("sample", "weighted-voronoi.source-sample", BTreeMap::new()),
                    node("map", "weighted-voronoi.response-map", BTreeMap::new()),
                    node(
                        "sites",
                        "weighted-voronoi.site-distribution",
                        site_parameters,
                    ),
                    node(
                        "voronoi",
                        "weighted-voronoi.construct-voronoi",
                        BTreeMap::new(),
                    ),
                    node("inset", "weighted-voronoi.response-inset", inset_parameters),
                    node("emit", "weighted-voronoi.emit-regions", emit_parameters),
                ],
                edges: vec![
                    edge("sample", "samples", "map", "samples"),
                    edge("map", "response-field", "sites", "response-field"),
                    edge("sites", "sites", "voronoi", "sites"),
                    edge("voronoi", "diagram", "inset", "diagram"),
                    edge("map", "response-field", "inset", "response-field"),
                    edge("inset", "response-insets", "emit", "response-insets"),
                ],
                output: port("emit", "geometry"),
            },
            assets: vec![EmbeddedSvgAsset {
                digest: sha256_digest(&svg),
                svg,
            }],
        }
    }
    fn port(node: &str, port: &str) -> PortReference {
        PortReference {
            node: node.into(),
            port: port.into(),
        }
    }
    fn edge(from_node: &str, from_port: &str, to_node: &str, to_port: &str) -> RecipeEdge {
        RecipeEdge {
            from: port(from_node, from_port),
            to: port(to_node, to_port),
        }
    }

    #[test]
    fn parse_and_serialize_are_deterministic_and_strict() {
        let definition = definition();
        let first = serialize_tnpattern(&definition).unwrap();
        let parsed = parse_tnpattern(&first).unwrap();
        assert_eq!(first, serialize_tnpattern(&parsed).unwrap());
        assert!(
            parse_tnpattern(br#"{"format_version":1,"recipe_version":1,"unknown":true}"#).is_err()
        );
        assert!(parse_tnpattern(&[0xff]).is_err());
    }

    #[test]
    fn ids_ports_operations_and_graph_safety_are_validated() {
        let mut invalid = definition();
        invalid.recipe.edges[0].to.port = "response-field".into();
        assert!(invalid.validate().is_err());
        let mut cyclic = definition();
        static LOOP_PORTS: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
            name: "value",
            kind: RecipePortType::Placement,
        }];
        static EMIT_PORTS: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
            name: "value",
            kind: RecipePortType::Placement,
        }];
        static CYCLE_OPERATIONS: [RegisteredOperationDescriptor; 2] = [
            RegisteredOperationDescriptor {
                id: "test.loop",
                version: 1,
                inputs: &LOOP_PORTS,
                output: OperationPortDescriptor {
                    name: "value",
                    kind: RecipePortType::Placement,
                },
                parameters: &NO_PARAMETERS,
                canonical_output_kinds: &NO_CANONICAL_OUTPUT_KINDS,
            },
            RegisteredOperationDescriptor {
                id: "test.emit",
                version: 1,
                inputs: &EMIT_PORTS,
                output: OperationPortDescriptor {
                    name: "geometry",
                    kind: RecipePortType::CanonicalGeometry,
                },
                parameters: &NO_PARAMETERS,
                canonical_output_kinds: &REGION_OUTPUT_KIND,
            },
        ];
        cyclic.recipe.nodes = vec![
            RecipeNode {
                id: "loop-a".into(),
                operation: OperationReference {
                    id: "test.loop".into(),
                    version: 1,
                },
                parameters: BTreeMap::new(),
            },
            RecipeNode {
                id: "loop-b".into(),
                operation: OperationReference {
                    id: "test.loop".into(),
                    version: 1,
                },
                parameters: BTreeMap::new(),
            },
            RecipeNode {
                id: "emit".into(),
                operation: OperationReference {
                    id: "test.emit".into(),
                    version: 1,
                },
                parameters: BTreeMap::new(),
            },
        ];
        cyclic.recipe.edges = vec![
            edge("loop-a", "value", "loop-b", "value"),
            edge("loop-b", "value", "loop-a", "value"),
            edge("loop-a", "value", "emit", "value"),
        ];
        cyclic.recipe.output = port("emit", "geometry");
        assert!(
            cyclic
                .validate_with_registry(&OperationRegistry::new(&CYCLE_OPERATIONS))
                .unwrap_err()
                .to_string()
                .contains("cycle")
        );
        let mut version = definition();
        version.recipe.nodes[0].operation.version = 2;
        assert!(
            version
                .validate()
                .unwrap_err()
                .to_string()
                .contains("version")
        );
        let mut orphan = definition();
        orphan.recipe.nodes.push(RecipeNode {
            id: "orphan".into(),
            operation: OperationReference {
                id: "weighted-voronoi.source-sample".into(),
                version: 1,
            },
            parameters: BTreeMap::new(),
        });
        assert!(
            orphan
                .validate()
                .unwrap_err()
                .to_string()
                .contains("unreachable")
        );
    }

    #[test]
    fn parameters_controls_limits_and_assets_are_validated() {
        let mut invalid = definition();
        invalid.quick_controls[0].scope = DefinitionParameterScope::Pattern;
        assert!(
            invalid
                .validate()
                .unwrap_err()
                .to_string()
                .contains("scope")
        );
        let mut asset = definition();
        asset.assets[0].svg = "<svg><script/></svg>".into();
        assert!(asset.validate().unwrap_err().to_string().contains("unsafe"));
        let mut external_reference = definition();
        external_reference.assets[0].svg = "<svg><use href=\"motif.svg\"/></svg>".into();
        assert!(external_reference.validate().is_err());
        let mut digest = definition();
        digest.assets[0].digest = "sha256:not-a-digest".into();
        assert!(
            digest
                .validate()
                .unwrap_err()
                .to_string()
                .contains("digest")
        );
        let mut uppercase = definition();
        uppercase.assets[0].digest = format!(
            "sha256:{}",
            uppercase.assets[0].digest[7..].to_ascii_uppercase()
        );
        assert!(
            uppercase
                .validate()
                .unwrap_err()
                .to_string()
                .contains("digest")
        );
        let mut tampered = definition();
        tampered.assets[0].svg.push(' ');
        assert!(
            tampered
                .validate()
                .unwrap_err()
                .to_string()
                .contains("does not match")
        );
        let mut limited = definition();
        limited.assets = vec![limited.assets[0].clone(); MAX_PATTERN_ASSETS + 1];
        assert!(
            limited
                .validate()
                .unwrap_err()
                .to_string()
                .contains("limits")
        );
    }

    fn parameter(
        key: &str,
        scope: DefinitionParameterScope,
        value_type: RecipeValueType,
        default: LiteralValue,
        constraints: PatternParameterConstraints,
    ) -> PatternParameterDefinition {
        PatternParameterDefinition {
            key: key.into(),
            label: key.replace('-', " "),
            help: format!("Configure {key}."),
            scope,
            value_type,
            default,
            constraints,
            choices: vec![],
        }
    }

    fn instance_definition() -> PatternDefinition {
        let mut definition = definition();
        let mut mode = parameter(
            "mode",
            DefinitionParameterScope::Pattern,
            RecipeValueType::Choice,
            LiteralValue::Choice("balanced".into()),
            PatternParameterConstraints::Choice,
        );
        mode.choices = vec!["balanced".into(), "dense".into()];
        definition.parameters.extend([
            parameter(
                "max-seed",
                DefinitionParameterScope::Pattern,
                RecipeValueType::Integer,
                LiteralValue::Integer(u64::MAX),
                PatternParameterConstraints::Integer {
                    minimum: 0,
                    maximum: u64::MAX,
                    step: 1,
                },
            ),
            mode,
            parameter(
                "note",
                DefinitionParameterScope::Pattern,
                RecipeValueType::Text,
                LiteralValue::Text("".into()),
                PatternParameterConstraints::Text { max_length: 32 },
            ),
            parameter(
                "motif",
                DefinitionParameterScope::Pattern,
                RecipeValueType::SvgAsset,
                LiteralValue::SvgAsset(definition.assets[0].digest.clone()),
                PatternParameterConstraints::SvgAsset,
            ),
            parameter(
                "channel-weight",
                DefinitionParameterScope::OutputChannel,
                RecipeValueType::Number,
                LiteralValue::Number(0.5),
                PatternParameterConstraints::Number {
                    minimum: 0.0,
                    maximum: 1.0,
                    step: 0.25,
                },
            ),
        ]);
        definition
    }

    #[test]
    fn parameter_constraints_are_strict_and_creator_ready() {
        let mut invalid = definition();
        invalid.parameters[7].constraints = PatternParameterConstraints::Number {
            minimum: 1.0,
            maximum: 32.0,
            step: 0.0,
        };
        assert!(
            invalid
                .validate()
                .unwrap_err()
                .to_string()
                .contains("constraints")
        );

        let mut non_finite = definition();
        non_finite.parameters[7].constraints = PatternParameterConstraints::Number {
            minimum: 1.0,
            maximum: f64::INFINITY,
            step: 1.0,
        };
        assert!(non_finite.validate().is_err());

        let mut choice = instance_definition();
        choice
            .parameters
            .iter_mut()
            .find(|parameter| parameter.key == "mode")
            .unwrap()
            .choices
            .push("balanced".into());
        assert!(
            choice
                .validate()
                .unwrap_err()
                .to_string()
                .contains("choice")
        );

        let mut text = instance_definition();
        text.parameters
            .iter_mut()
            .find(|parameter| parameter.key == "note")
            .unwrap()
            .constraints = PatternParameterConstraints::Text {
            max_length: MAX_TEXT_PARAMETER_BYTES + 1,
        };
        assert!(
            text.validate()
                .unwrap_err()
                .to_string()
                .contains("constraints")
        );

        let mut asset = instance_definition();
        asset
            .parameters
            .iter_mut()
            .find(|parameter| parameter.key == "motif")
            .unwrap()
            .default = LiteralValue::SvgAsset(format!("sha256:{}", "0".repeat(64)));
        assert!(
            asset
                .validate()
                .unwrap_err()
                .to_string()
                .contains("asset reference")
        );
        assert!(
            serde_json::from_str::<PatternParameterConstraints>(
                r#"{"kind":"number","minimum":0.0,"maximum":1.0,"step":0.1,"unknown":true}"#
            )
            .is_err()
        );
    }

    #[test]
    fn instance_parameters_are_complete_scoped_and_deterministic() {
        let definition = instance_definition();
        let instance = definition
            .default_instance_parameters([OutputChannelId::RgbBlue, OutputChannelId::CmykCyan])
            .unwrap();
        assert_eq!(
            instance
                .pattern_values
                .iter()
                .find(|value| value.key == "max-seed")
                .unwrap()
                .value,
            LiteralValue::Integer(u64::MAX)
        );
        let first = serialize_tnpattern_instance_parameters(&definition, &instance).unwrap();
        let parsed = parse_tnpattern_instance_parameters(&definition, &first).unwrap();
        assert_eq!(
            first,
            serialize_tnpattern_instance_parameters(&definition, &parsed).unwrap()
        );

        let mut out_of_step = instance.clone();
        out_of_step.output_channel_values[0]
            .values
            .iter_mut()
            .find(|value| value.key == "channel-weight")
            .unwrap()
            .value = LiteralValue::Number(8.1);
        assert!(
            definition
                .validate_instance_parameters(&out_of_step)
                .unwrap_err()
                .to_string()
                .contains("constraints")
        );

        let mut non_finite = instance.clone();
        non_finite.output_channel_values[0]
            .values
            .iter_mut()
            .find(|value| value.key == "channel-weight")
            .unwrap()
            .value = LiteralValue::Number(f64::NAN);
        assert!(
            definition
                .validate_instance_parameters(&non_finite)
                .unwrap_err()
                .to_string()
                .contains("unsafe value")
        );

        let mut missing = instance.clone();
        missing.pattern_values.retain(|value| value.key != "mode");
        assert!(
            definition
                .validate_instance_parameters(&missing)
                .unwrap_err()
                .to_string()
                .contains("missing parameter `mode`")
        );

        let mut unknown = instance.clone();
        unknown.pattern_values[0].key = "unknown".into();
        assert!(
            definition
                .validate_instance_parameters(&unknown)
                .unwrap_err()
                .to_string()
                .contains("unknown parameter")
        );

        let mut scope_mismatch = instance.clone();
        scope_mismatch.output_channel_values[0]
            .values
            .push(PatternInstanceValue {
                key: "mode".into(),
                value: LiteralValue::Choice("balanced".into()),
            });
        assert!(
            definition
                .validate_instance_parameters(&scope_mismatch)
                .unwrap_err()
                .to_string()
                .contains("scope mismatch")
        );

        let mut missing_channel_value = instance.clone();
        missing_channel_value.output_channel_values[0]
            .values
            .retain(|value| value.key != "channel-weight");
        assert!(
            definition
                .validate_instance_parameters(&missing_channel_value)
                .unwrap_err()
                .to_string()
                .contains("missing parameter `channel-weight`")
        );

        let mut duplicate = instance.clone();
        duplicate
            .pattern_values
            .push(duplicate.pattern_values[0].clone());
        assert!(
            definition
                .validate_instance_parameters(&duplicate)
                .unwrap_err()
                .to_string()
                .contains("duplicate parameter")
        );

        let mut duplicate_channel = instance.clone();
        duplicate_channel
            .output_channel_values
            .push(duplicate_channel.output_channel_values[0].clone());
        assert!(
            definition
                .validate_instance_parameters(&duplicate_channel)
                .unwrap_err()
                .to_string()
                .contains("duplicate output channel")
        );

        let mut too_many_channels = instance.clone();
        too_many_channels.output_channel_values = vec![
            too_many_channels.output_channel_values[0]
                .clone();
            MAX_PATTERN_INSTANCE_OUTPUT_CHANNELS + 1
        ];
        assert!(
            definition
                .validate_instance_parameters(&too_many_channels)
                .unwrap_err()
                .to_string()
                .contains("resource limit")
        );

        let mut too_many_values = instance.clone();
        let repeated = too_many_values.pattern_values[0].clone();
        too_many_values.pattern_values = vec![repeated; MAX_PATTERN_INSTANCE_VALUES + 1];
        assert!(
            definition
                .validate_instance_parameters(&too_many_values)
                .unwrap_err()
                .to_string()
                .contains("resource limit")
        );

        let mut unknown_channel = instance.clone();
        unknown_channel.output_channel_values[0].channel = "channel.none".into();
        assert!(
            definition
                .validate_instance_parameters(&unknown_channel)
                .unwrap_err()
                .to_string()
                .contains("unknown output channel")
        );

        let mut unknown_asset = instance.clone();
        unknown_asset
            .pattern_values
            .iter_mut()
            .find(|value| value.key == "motif")
            .unwrap()
            .value = LiteralValue::SvgAsset(format!("sha256:{}", "0".repeat(64)));
        assert!(
            definition
                .validate_instance_parameters(&unknown_asset)
                .unwrap_err()
                .to_string()
                .contains("constraints")
        );

        let text = String::from_utf8(first).unwrap();
        let unknown_field = format!("{},\"unknown\":true}}", &text[..text.len() - 1]);
        assert!(
            parse_tnpattern_instance_parameters(&definition, unknown_field.as_bytes()).is_err()
        );
    }

    static EXECUTION_TRACE: OnceLock<Mutex<Vec<&'static str>>> = OnceLock::new();
    static EXEC_INPUTS: [OperationPortDescriptor; 2] = [
        OperationPortDescriptor {
            name: "left",
            kind: RecipePortType::CanonicalGeometry,
        },
        OperationPortDescriptor {
            name: "right",
            kind: RecipePortType::CanonicalGeometry,
        },
    ];
    static EXEC_PARAMETERS: [OperationParameterDescriptor; 3] = [
        OperationParameterDescriptor {
            name: "max-seed",
            value_type: RecipeValueType::Integer,
        },
        OperationParameterDescriptor {
            name: "channel-weight",
            value_type: RecipeValueType::Number,
        },
        OperationParameterDescriptor {
            name: "literal-weight",
            value_type: RecipeValueType::Number,
        },
    ];
    static EXECUTION_DESCRIPTORS: [RegisteredOperationDescriptor; 3] = [
        RegisteredOperationDescriptor {
            id: "test.alpha",
            version: 1,
            inputs: &NO_PORTS,
            output: OperationPortDescriptor {
                name: "geometry",
                kind: RecipePortType::CanonicalGeometry,
            },
            parameters: &NO_PARAMETERS,
            canonical_output_kinds: &REGION_OUTPUT_KIND,
        },
        RegisteredOperationDescriptor {
            id: "test.zeta",
            version: 1,
            inputs: &NO_PORTS,
            output: OperationPortDescriptor {
                name: "geometry",
                kind: RecipePortType::CanonicalGeometry,
            },
            parameters: &NO_PARAMETERS,
            canonical_output_kinds: &REGION_OUTPUT_KIND,
        },
        RegisteredOperationDescriptor {
            id: "test.emit",
            version: 1,
            inputs: &EXEC_INPUTS,
            output: OperationPortDescriptor {
                name: "geometry",
                kind: RecipePortType::CanonicalGeometry,
            },
            parameters: &EXEC_PARAMETERS,
            canonical_output_kinds: &REGION_OUTPUT_KIND,
        },
    ];

    fn trace() -> &'static Mutex<Vec<&'static str>> {
        EXECUTION_TRACE.get_or_init(|| Mutex::new(Vec::new()))
    }

    fn region_output(context: &RecipeExecutionContext<'_>) -> RecipeRuntimeValue {
        RecipeRuntimeValue::CanonicalOutput(CanonicalPatternOutput::Regions(RegionPatternOutput {
            artboard: context.artboard,
            layers: vec![],
            regions: vec![],
        }))
    }

    fn mark_output(context: &RecipeExecutionContext<'_>) -> RecipeRuntimeValue {
        RecipeRuntimeValue::CanonicalOutput(CanonicalPatternOutput::Marks(MarkPatternOutput {
            geometry: MarkSet {
                width: context.artboard.width,
                height: context.artboard.height,
                marks: vec![],
                layers: vec![],
            },
        }))
    }

    fn alpha_operation(
        context: &RecipeExecutionContext<'_>,
        _: &RecipeOperationInputs<'_>,
        _: &RecipeOperationParameters<'_>,
    ) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
        trace().lock().unwrap().push("alpha");
        Ok(region_output(context))
    }

    fn zeta_operation(
        context: &RecipeExecutionContext<'_>,
        _: &RecipeOperationInputs<'_>,
        _: &RecipeOperationParameters<'_>,
    ) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
        trace().lock().unwrap().push("zeta");
        Ok(region_output(context))
    }

    fn silent_region_operation(
        context: &RecipeExecutionContext<'_>,
        _: &RecipeOperationInputs<'_>,
        _: &RecipeOperationParameters<'_>,
    ) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
        Ok(region_output(context))
    }

    fn emit_operation(
        context: &RecipeExecutionContext<'_>,
        inputs: &RecipeOperationInputs<'_>,
        parameters: &RecipeOperationParameters<'_>,
    ) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
        if inputs.len() != 2
            || !matches!(
                parameters.get("max-seed"),
                Some(LiteralValue::Integer(u64::MAX))
            )
            || !matches!(
                parameters.get("channel-weight"),
                Some(LiteralValue::Number(0.5))
            )
            || !matches!(
                parameters.get("literal-weight"),
                Some(LiteralValue::Number(0.25))
            )
        {
            return Err(NativeRecipeOperationError::new(
                "test operation received unexpected bound values",
            ));
        }
        trace().lock().unwrap().push("emit");
        Ok(region_output(context))
    }

    fn failing_operation(
        _: &RecipeExecutionContext<'_>,
        _: &RecipeOperationInputs<'_>,
        _: &RecipeOperationParameters<'_>,
    ) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
        Err(NativeRecipeOperationError::new(
            "intentional native failure",
        ))
    }

    fn cancelling_operation(
        context: &RecipeExecutionContext<'_>,
        _: &RecipeOperationInputs<'_>,
        _: &RecipeOperationParameters<'_>,
    ) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
        assert!(context.cancellation.cancel());
        Ok(region_output(context))
    }

    fn wrong_port_operation(
        _: &RecipeExecutionContext<'_>,
        _: &RecipeOperationInputs<'_>,
        _: &RecipeOperationParameters<'_>,
    ) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
        Ok(RecipeRuntimeValue::MappedField(
            DistributionField::new(1, 1, vec![1.0]).unwrap(),
        ))
    }

    fn wrong_canonical_output_operation(
        context: &RecipeExecutionContext<'_>,
        _: &RecipeOperationInputs<'_>,
        _: &RecipeOperationParameters<'_>,
    ) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
        Ok(mark_output(context))
    }

    static EXECUTION_IMPLEMENTATIONS: [RegisteredNativeRecipeOperation; 3] = [
        RegisteredNativeRecipeOperation {
            id: "test.alpha",
            version: 1,
            execute: alpha_operation,
        },
        RegisteredNativeRecipeOperation {
            id: "test.zeta",
            version: 1,
            execute: zeta_operation,
        },
        RegisteredNativeRecipeOperation {
            id: "test.emit",
            version: 1,
            execute: emit_operation,
        },
    ];

    fn execution_definition() -> PatternDefinition {
        let mut definition = instance_definition();
        let node = |id: &str, operation: &str, parameters| RecipeNode {
            id: id.into(),
            operation: OperationReference {
                id: operation.into(),
                version: 1,
            },
            parameters,
        };
        definition.recipe = RecipeGraph {
            // Deliberately reverse lexical source order: execution must still
            // visit alpha before zeta.
            nodes: vec![
                node("zeta", "test.zeta", BTreeMap::new()),
                node("alpha", "test.alpha", BTreeMap::new()),
                node(
                    "emit",
                    "test.emit",
                    BTreeMap::from([
                        (
                            "max-seed".into(),
                            RecipeArgument::Parameter("max-seed".into()),
                        ),
                        (
                            "channel-weight".into(),
                            RecipeArgument::Parameter("channel-weight".into()),
                        ),
                        (
                            "literal-weight".into(),
                            RecipeArgument::Literal(LiteralValue::Number(0.25)),
                        ),
                    ]),
                ),
            ],
            edges: vec![
                edge("zeta", "geometry", "emit", "right"),
                edge("alpha", "geometry", "emit", "left"),
            ],
            output: port("emit", "geometry"),
        };
        definition
    }

    fn execution_context<'a>(token: &'a CancellationToken) -> RecipeExecutionContext<'a> {
        RecipeExecutionContext {
            artboard: ArtboardSpace {
                width: 16,
                height: 12,
            },
            output_channel: Some(OutputChannelId::CmykCyan),
            source_field_provider: None,
            source_field: None,
            source_generation: 0,
            resolved_field_generation: 0,
            semantic_channel_index: 0,
            enabled_layer_index: 0,
            definition_assets: &[],
            cancellation: token,
        }
    }

    #[test]
    fn native_recipe_execution_is_deterministic_typed_and_cancellable() {
        assert_eq!(
            RecipeRuntimeValue::BoundaryDerivedRegionCells(RegionPatternOutput {
                artboard: ArtboardSpace {
                    width: 1,
                    height: 1,
                },
                layers: vec![],
                regions: vec![],
            })
            .port_type(),
            RecipePortType::BoundaryDerivedRegionCells
        );
        let definition = execution_definition();
        let instance = instance_definition()
            .default_instance_parameters([OutputChannelId::CmykCyan])
            .unwrap();
        let registry =
            NativeRecipeOperationRegistry::new(&EXECUTION_DESCRIPTORS, &EXECUTION_IMPLEMENTATIONS);
        trace().lock().unwrap().clear();
        let token = CancellationToken::new();
        let output = definition
            .execute_recipe(&instance, &execution_context(&token), &registry)
            .unwrap();
        assert!(matches!(output, CanonicalPatternOutput::Regions(_)));
        assert_eq!(*trace().lock().unwrap(), vec!["alpha", "zeta", "emit"]);

        let missing_channel_instance = instance_definition()
            .default_instance_parameters([OutputChannelId::RgbRed])
            .unwrap();
        assert!(
            definition
                .execute_recipe(
                    &missing_channel_instance,
                    &execution_context(&CancellationToken::new()),
                    &registry,
                )
                .unwrap_err()
                .to_string()
                .contains("selected output channel")
        );

        let no_implementation = NativeRecipeOperationRegistry::new(&EXECUTION_DESCRIPTORS, &[]);
        assert!(
            definition
                .execute_recipe(
                    &instance,
                    &execution_context(&CancellationToken::new()),
                    &no_implementation,
                )
                .unwrap_err()
                .to_string()
                .contains("no native implementation")
        );

        let mut unsupported_version = definition.clone();
        unsupported_version.recipe.nodes[0].operation.version = 2;
        assert!(
            unsupported_version
                .execute_recipe(
                    &instance,
                    &execution_context(&CancellationToken::new()),
                    &registry,
                )
                .unwrap_err()
                .to_string()
                .contains("unsupported operation version")
        );

        let before_start = CancellationToken::new();
        assert!(before_start.cancel());
        assert!(
            definition
                .execute_recipe(&instance, &execution_context(&before_start), &registry)
                .unwrap_err()
                .to_string()
                .contains("before start")
        );
    }

    #[test]
    fn native_recipe_execution_contextualizes_operation_and_runtime_failures() {
        let definition = execution_definition();
        let instance = instance_definition()
            .default_instance_parameters([OutputChannelId::CmykCyan])
            .unwrap();
        let failure_implementations = [
            RegisteredNativeRecipeOperation {
                id: "test.alpha",
                version: 1,
                execute: failing_operation,
            },
            EXECUTION_IMPLEMENTATIONS[1],
            EXECUTION_IMPLEMENTATIONS[2],
        ];
        let failure_registry =
            NativeRecipeOperationRegistry::new(&EXECUTION_DESCRIPTORS, &failure_implementations);
        assert!(definition
            .execute_recipe(
                &instance,
                &execution_context(&CancellationToken::new()),
                &failure_registry,
            )
            .unwrap_err()
            .to_string()
            .contains("node `alpha` operation `test.alpha` version 1 failed: intentional native failure"));

        let wrong_port_implementations = [
            RegisteredNativeRecipeOperation {
                id: "test.alpha",
                version: 1,
                execute: wrong_port_operation,
            },
            EXECUTION_IMPLEMENTATIONS[1],
            EXECUTION_IMPLEMENTATIONS[2],
        ];
        let wrong_port_registry =
            NativeRecipeOperationRegistry::new(&EXECUTION_DESCRIPTORS, &wrong_port_implementations);
        assert!(
            definition
                .execute_recipe(
                    &instance,
                    &execution_context(&CancellationToken::new()),
                    &wrong_port_registry,
                )
                .unwrap_err()
                .to_string()
                .contains("returned MappedField, expected CanonicalGeometry")
        );

        let wrong_output_implementations = [
            RegisteredNativeRecipeOperation {
                id: "test.alpha",
                version: 1,
                execute: silent_region_operation,
            },
            RegisteredNativeRecipeOperation {
                id: "test.zeta",
                version: 1,
                execute: silent_region_operation,
            },
            RegisteredNativeRecipeOperation {
                id: "test.emit",
                version: 1,
                execute: wrong_canonical_output_operation,
            },
        ];
        let wrong_output_registry = NativeRecipeOperationRegistry::new(
            &EXECUTION_DESCRIPTORS,
            &wrong_output_implementations,
        );
        assert!(
            definition
                .execute_recipe(
                    &instance,
                    &execution_context(&CancellationToken::new()),
                    &wrong_output_registry,
                )
                .unwrap_err()
                .to_string()
                .contains("does not match declared output capabilities")
        );

        let cancelling_implementations = [
            RegisteredNativeRecipeOperation {
                id: "test.alpha",
                version: 1,
                execute: cancelling_operation,
            },
            EXECUTION_IMPLEMENTATIONS[1],
            EXECUTION_IMPLEMENTATIONS[2],
        ];
        let cancelling_registry =
            NativeRecipeOperationRegistry::new(&EXECUTION_DESCRIPTORS, &cancelling_implementations);
        assert!(
            definition
                .execute_recipe(
                    &instance,
                    &execution_context(&CancellationToken::new()),
                    &cancelling_registry,
                )
                .unwrap_err()
                .to_string()
                .contains("cancelled before node `zeta`")
        );
    }
}
