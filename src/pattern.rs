//! Stable pattern vocabulary and canonical runtime output algebra.
//!
//! The output algebra is deliberately *not* a universal geometry container.
//! Existing marks and curve outlines retain their mature concrete types while
//! filled regions and shared boundary networks have independent semantic
//! representations. See `docs/TON-010_STAGE_3_CANONICAL_OUTPUT.md` for the
//! coordinate, ordering, clipping, polarity, and bounded-work contract.

use crate::curve_render::CurveGeometry;
use crate::render::MarkSet;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

/// Stable identifier for a built-in pattern.
///
/// Pattern identifiers are serialized as dotted strings so interface labels
/// can change without affecting saved pattern state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum PatternId {
    #[serde(rename = "compat.shapes.v1")]
    CompatibilityShapesV1,
    #[serde(rename = "compat.curves.v1")]
    CompatibilityCurvesV1,
    #[serde(rename = "weighted-voronoi.v1")]
    WeightedVoronoiV1,
}

impl PatternId {
    pub const COMPATIBILITY_SHAPES_V1: Self = Self::CompatibilityShapesV1;
    pub const COMPATIBILITY_CURVES_V1: Self = Self::CompatibilityCurvesV1;
    pub const WEIGHTED_VORONOI_V1: Self = Self::WeightedVoronoiV1;

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompatibilityShapesV1 => "compat.shapes.v1",
            Self::CompatibilityCurvesV1 => "compat.curves.v1",
            Self::WeightedVoronoiV1 => "weighted-voronoi.v1",
        }
    }
}

impl fmt::Display for PatternId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for PatternId {
    type Err = PatternIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !is_valid_dotted_id(value) {
            return Err(PatternIdError::InvalidFormat(value.to_owned()));
        }

        match value {
            "compat.shapes.v1" => Ok(Self::CompatibilityShapesV1),
            "compat.curves.v1" => Ok(Self::CompatibilityCurvesV1),
            "weighted-voronoi.v1" => Ok(Self::WeightedVoronoiV1),
            _ => Err(PatternIdError::Unknown(value.to_owned())),
        }
    }
}

/// Checks the durable identifier syntax before registry lookup.
pub fn is_valid_dotted_id(value: &str) -> bool {
    value.split('.').count() >= 2
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternIdError {
    InvalidFormat(String),
    Unknown(String),
}

impl fmt::Display for PatternIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(value) => write!(formatter, "invalid pattern identifier {value:?}"),
            Self::Unknown(value) => write!(formatter, "unknown pattern identifier {value:?}"),
        }
    }
}

impl std::error::Error for PatternIdError {}

/// Broad organization for future pattern discovery and filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatternFamily {
    StructuredFields,
    StochasticDistributions,
    ParametricPaths,
    ConstructivePatterns,
}

/// Canonical geometry category emitted by a pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatternOutputKind {
    Marks,
    Paths,
    Regions,
    Networks,
}

/// The existing inspector surface selected by a registered pattern.
///
/// This is presentation metadata only. `PatternDocumentState` remains the
/// authority for persisted selection and parameters; `RenderVariant` is its
/// derived execution adapter during the compatibility phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternInspectorPanel {
    Shapes,
    Curves,
    WeightedVoronoi,
}

impl PatternInspectorPanel {
    pub const fn stack_name(self) -> &'static str {
        match self {
            Self::Shapes => "web",
            Self::Curves => "curve",
            Self::WeightedVoronoi => "weighted-voronoi",
        }
    }
}

/// The one current authoring scope for compatibility-pattern controls.
///
/// `channel_scope` is the only UI which selects this scope. In particular,
/// schema metadata must not introduce a second web or curve target selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternParameterScope {
    TreatmentScope,
    PatternScope,
}

/// Conditions under which an existing compatibility control is applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternParameterVisibility {
    Always,
    PolygonMark,
    UserDefinedMark,
    MotifLayout,
}

/// A schema descriptor for an existing compatibility-pattern control.
///
/// The `control_id` refers to the stable Blueprint widget ID. It allows the
/// current inspector to consume labels, help, scope, and visibility metadata
/// without replacing the mature Shapes and Curves editors in this stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatternParameterDescriptor {
    pub key: &'static str,
    pub control_id: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    pub scope: PatternParameterScope,
    pub visibility: PatternParameterVisibility,
}

/// Stable selector and inspector metadata for a registered pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatternSelectorMetadata {
    pub label: &'static str,
    pub help: &'static str,
    pub inspector_panel: PatternInspectorPanel,
}

/// Legacy behavior declared by a compatibility registry entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LegacyPatternCompatibility {
    ShapesV1,
    CurvesV1,
}

/// Compatibility constraints for an entry retained from the pre-registry
/// renderer. These are declarations only; they do not route rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternCompatibility {
    Legacy {
        legacy_render_variant: LegacyPatternCompatibility,
    },
    CanonicalRegions,
}

/// Persistable identity and version contract for one registered pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatternMetadata {
    pub id: PatternId,
    pub family: PatternFamily,
    pub output_kind: PatternOutputKind,
    pub parameter_schema_version: u32,
    pub generator_version: u32,
    pub compatibility: PatternCompatibility,
    pub selector: PatternSelectorMetadata,
    pub parameters: &'static [PatternParameterDescriptor],
}

impl PatternMetadata {
    /// Finds the schema descriptor bound to one stable inspector control ID.
    pub fn parameter_for_control(
        &self,
        control_id: &str,
    ) -> Option<&'static PatternParameterDescriptor> {
        self.parameters
            .iter()
            .find(|descriptor| descriptor.control_id == control_id)
    }
}

const SHAPES_PARAMETERS: [PatternParameterDescriptor; 13] = [
    PatternParameterDescriptor {
        key: "shared-mark",
        control_id: "web_shared",
        label: "Share Mark Shape",
        help: "Edit one mark shape for every ink or channel in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "mark",
        control_id: "web_shape",
        label: "Mark",
        help: "Choose the mark geometry used by the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "polygon-sides",
        control_id: "web_polygon_sides",
        label: "Polygon Sides (3–6)",
        help: "Choose the number of sides for the current polygon mark.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::PolygonMark,
    },
    PatternParameterDescriptor {
        key: "user-defined-mark",
        control_id: "web_edit_shape",
        label: "Edit User-Defined Mark…",
        help: "Edit the custom mark used by the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::UserDefinedMark,
    },
    PatternParameterDescriptor {
        key: "visible-channels",
        control_id: "web_visible_row",
        label: "Visible Inks",
        help: "Show or hide generated marks for each output ink or channel.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "ink-color",
        control_id: "web_color",
        label: "Ink Color",
        help: "Set the color for one selected output ink or channel.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "coverage",
        control_id: "web_coverage_scale",
        label: "Mark Coverage",
        help: "Change the coverage of marks in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "screen-angle",
        control_id: "web_angle_scale",
        label: "Screen Angle",
        help: "Rotate the mark screen in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "mark-angle",
        control_id: "web_mark_angle_scale",
        label: "Mark Angle",
        help: "Rotate each mark in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "mark-width",
        control_id: "web_width_scale",
        label: "Mark Width",
        help: "Change mark width in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "mark-height",
        control_id: "web_height_scale",
        label: "Mark Height",
        help: "Change mark height in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "light-tone-cutoff",
        control_id: "web_threshold_scale",
        label: "Light-Tone Cutoff",
        help: "Hide light marks in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "sampling-detail",
        control_id: "web_detail_scale",
        label: "Sampling Detail",
        help: "Change source sampling detail in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
];

const CURVES_PARAMETERS: [PatternParameterDescriptor; 15] = [
    PatternParameterDescriptor {
        key: "layout",
        control_id: "curve_layout",
        label: "Layout",
        help: "Choose continuous lines or a repeated motif.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "line-shape",
        control_id: "curve_editor",
        label: "Line Shape",
        help: "Edit the curve path used by the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "shared-line-shape",
        control_id: "curve_shared",
        label: "Share Line Shape",
        help: "Edit one line shape for every ink or channel in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "visible-channels",
        control_id: "curve_visible_row",
        label: "Visible Inks",
        help: "Show or hide generated lines for each output ink or channel.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "ink-color",
        control_id: "curve_color",
        label: "Ink Color",
        help: "Set the color for one selected output ink or channel.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "line-weight",
        control_id: "curve_weight_scale",
        label: "Line Weight",
        help: "Change the thickness of every curve line.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "line-spacing",
        control_id: "curve_spacing_scale",
        label: "Line Spacing",
        help: "Change the distance between curve lines.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "line-coverage",
        control_id: "curve_coverage_scale",
        label: "Line Coverage",
        help: "Change line coverage in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "screen-angle",
        control_id: "curve_angle_scale",
        label: "Screen Angle",
        help: "Rotate the line screen in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "position-x",
        control_id: "curve_position_x_scale",
        label: "Position X",
        help: "Move the line screen across the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "position-y",
        control_id: "curve_position_y_scale",
        label: "Position Y",
        help: "Move the line screen vertically in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "ink-opacity",
        control_id: "curve_opacity_scale",
        label: "Ink Opacity",
        help: "Change opacity in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "light-tone-cutoff",
        control_id: "curve_threshold_scale",
        label: "Light-Tone Cutoff",
        help: "Hide light lines in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "sampling-detail",
        control_id: "curve_detail_scale",
        label: "Sampling Detail",
        help: "Change source sampling detail in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "motif-arrangement",
        control_id: "motif_controls",
        label: "Motif Arrangement",
        help: "Arrange the repeated motif in the current treatment scope.",
        scope: PatternParameterScope::TreatmentScope,
        visibility: PatternParameterVisibility::MotifLayout,
    },
];

const WEIGHTED_VORONOI_PARAMETERS: [PatternParameterDescriptor; 8] = [
    PatternParameterDescriptor {
        key: "enabled-channels",
        control_id: "weighted_voronoi_visible",
        label: "Enabled Channels",
        help: "Include or omit each semantic output channel from the generated regions.",
        scope: PatternParameterScope::PatternScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "cell-count",
        control_id: "weighted_voronoi_cell_count",
        label: "Cell Count",
        help: "Exact bounded cells per enabled semantic channel.",
        scope: PatternParameterScope::PatternScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "arrangement",
        control_id: "weighted_voronoi_arrangement",
        label: "Arrangement",
        help: "Share candidates across channels or use channel-specific candidates.",
        scope: PatternParameterScope::PatternScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "placement",
        control_id: "weighted_voronoi_placement",
        label: "Placement",
        help: "Use uniform or source-weighted site placement.",
        scope: PatternParameterScope::PatternScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "density-strength",
        control_id: "weighted_voronoi_density_strength",
        label: "Density Strength",
        help: "Controls how strongly source values bias weighted site placement.",
        scope: PatternParameterScope::PatternScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "response-strength",
        control_id: "weighted_voronoi_response_strength",
        label: "Interior Response",
        help: "Controls how strongly source values inset each cell interior.",
        scope: PatternParameterScope::PatternScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "boundary-gap",
        control_id: "weighted_voronoi_boundary_gap",
        label: "Boundary Gap",
        help: "Sets the artboard-safe gap between a cell interior and its boundary region.",
        scope: PatternParameterScope::PatternScope,
        visibility: PatternParameterVisibility::Always,
    },
    PatternParameterDescriptor {
        key: "seed",
        control_id: "weighted_voronoi_seed",
        label: "Seed",
        help: "Fixed deterministic distribution seed.",
        scope: PatternParameterScope::PatternScope,
        visibility: PatternParameterVisibility::Always,
    },
];

const BUILTIN_PATTERN_METADATA: [PatternMetadata; 3] = [
    PatternMetadata {
        id: PatternId::CompatibilityShapesV1,
        family: PatternFamily::StructuredFields,
        output_kind: PatternOutputKind::Marks,
        parameter_schema_version: 1,
        generator_version: 1,
        compatibility: PatternCompatibility::Legacy {
            legacy_render_variant: LegacyPatternCompatibility::ShapesV1,
        },
        selector: PatternSelectorMetadata {
            label: "Shapes",
            help: "Use the existing mark-based Shapes treatment.",
            inspector_panel: PatternInspectorPanel::Shapes,
        },
        parameters: &SHAPES_PARAMETERS,
    },
    PatternMetadata {
        id: PatternId::CompatibilityCurvesV1,
        family: PatternFamily::ParametricPaths,
        output_kind: PatternOutputKind::Paths,
        parameter_schema_version: 1,
        generator_version: 1,
        compatibility: PatternCompatibility::Legacy {
            legacy_render_variant: LegacyPatternCompatibility::CurvesV1,
        },
        selector: PatternSelectorMetadata {
            label: "Curves",
            help: "Use the existing path-based Curves treatment.",
            inspector_panel: PatternInspectorPanel::Curves,
        },
        parameters: &CURVES_PARAMETERS,
    },
    PatternMetadata {
        id: PatternId::WeightedVoronoiV1,
        family: PatternFamily::StochasticDistributions,
        output_kind: PatternOutputKind::Regions,
        parameter_schema_version: 1,
        // v2 is the framework-restart algorithm. v1 is deliberately rejected.
        generator_version: 2,
        compatibility: PatternCompatibility::CanonicalRegions,
        selector: PatternSelectorMetadata {
            label: "Weighted Voronoi",
            help: "Generate deterministic source-responsive cell regions.",
            inspector_panel: PatternInspectorPanel::WeightedVoronoi,
        },
        parameters: &WEIGHTED_VORONOI_PARAMETERS,
    },
];

/// Immutable lookup surface for built-in patterns.
#[derive(Debug)]
pub struct PatternRegistry {
    entries: &'static [PatternMetadata],
}

impl PatternRegistry {
    pub const fn new(entries: &'static [PatternMetadata]) -> Self {
        Self { entries }
    }

    pub const fn entries(&self) -> &'static [PatternMetadata] {
        self.entries
    }

    pub fn get(&self, id: PatternId) -> Option<&'static PatternMetadata> {
        self.entries.iter().find(|metadata| metadata.id == id)
    }

    /// Finds a registered schema descriptor by its stable inspector control
    /// ID, so UI bindings do not need to infer a pattern from legacy render
    /// adapter state.
    pub fn parameter_for_control(
        &self,
        id: PatternId,
        control_id: &str,
    ) -> Option<&'static PatternParameterDescriptor> {
        self.get(id)?.parameter_for_control(control_id)
    }

    pub fn validate(&self) -> Result<(), PatternRegistryError> {
        let mut ids = HashSet::with_capacity(self.entries.len());
        for metadata in self.entries {
            if !ids.insert(metadata.id) {
                return Err(PatternRegistryError::DuplicateId(metadata.id));
            }
            if metadata.selector.label.is_empty() || metadata.selector.help.is_empty() {
                return Err(PatternRegistryError::InvalidSelector(metadata.id));
            }
            let mut keys = HashSet::with_capacity(metadata.parameters.len());
            let mut controls = HashSet::with_capacity(metadata.parameters.len());
            for parameter in metadata.parameters {
                if parameter.key.is_empty()
                    || parameter.control_id.is_empty()
                    || parameter.label.is_empty()
                    || parameter.help.is_empty()
                {
                    return Err(PatternRegistryError::InvalidParameter {
                        id: metadata.id,
                        key: parameter.key,
                    });
                }
                if !keys.insert(parameter.key) {
                    return Err(PatternRegistryError::DuplicateParameterKey {
                        id: metadata.id,
                        key: parameter.key,
                    });
                }
                if !controls.insert(parameter.control_id) {
                    return Err(PatternRegistryError::DuplicateParameterControl {
                        id: metadata.id,
                        control_id: parameter.control_id,
                    });
                }
            }
        }
        Ok(())
    }
}

/// The complete Stage 1 registry. It contains only existing renderer adapters.
pub static PATTERN_REGISTRY: PatternRegistry = PatternRegistry::new(&BUILTIN_PATTERN_METADATA);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternRegistryError {
    DuplicateId(PatternId),
    InvalidSelector(PatternId),
    InvalidParameter {
        id: PatternId,
        key: &'static str,
    },
    DuplicateParameterKey {
        id: PatternId,
        key: &'static str,
    },
    DuplicateParameterControl {
        id: PatternId,
        control_id: &'static str,
    },
}

impl fmt::Display for PatternRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(formatter, "duplicate pattern identifier {id}"),
            Self::InvalidSelector(id) => {
                write!(formatter, "pattern {id} has invalid selector metadata")
            }
            Self::InvalidParameter { id, key } => write!(
                formatter,
                "pattern {id} has invalid parameter metadata for {key:?}"
            ),
            Self::DuplicateParameterKey { id, key } => {
                write!(formatter, "pattern {id} duplicates parameter key {key:?}")
            }
            Self::DuplicateParameterControl { id, control_id } => write!(
                formatter,
                "pattern {id} duplicates parameter control {control_id:?}"
            ),
        }
    }
}

impl std::error::Error for PatternRegistryError {}

/// A versioned, serializable parameter object for one registered pattern.
///
/// Stage 1 intentionally accepts opaque parameter members. A later schema
/// owner can add per-pattern member validation without changing this envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionedPatternParameters {
    pub pattern_id: PatternId,
    pub schema_version: u32,
    pub generator_version: u32,
    #[serde(default)]
    pub values: serde_json::Map<String, serde_json::Value>,
}

impl VersionedPatternParameters {
    pub fn validate(&self) -> Result<&'static PatternMetadata, PatternParameterError> {
        let metadata = PATTERN_REGISTRY
            .get(self.pattern_id)
            .ok_or(PatternParameterError::UnknownPattern(self.pattern_id))?;

        if self.schema_version != metadata.parameter_schema_version {
            return Err(PatternParameterError::UnsupportedSchemaVersion {
                id: self.pattern_id,
                received: self.schema_version,
                supported: metadata.parameter_schema_version,
            });
        }
        if self.generator_version != metadata.generator_version {
            return Err(PatternParameterError::UnsupportedGeneratorVersion {
                id: self.pattern_id,
                received: self.generator_version,
                supported: metadata.generator_version,
            });
        }
        Ok(metadata)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternParameterError {
    UnknownPattern(PatternId),
    UnsupportedSchemaVersion {
        id: PatternId,
        received: u32,
        supported: u32,
    },
    UnsupportedGeneratorVersion {
        id: PatternId,
        received: u32,
        supported: u32,
    },
}

impl fmt::Display for PatternParameterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPattern(id) => write!(formatter, "pattern {id} is not registered"),
            Self::UnsupportedSchemaVersion {
                id,
                received,
                supported,
            } => write!(
                formatter,
                "pattern {id} does not support parameter schema version {received} (supports {supported})"
            ),
            Self::UnsupportedGeneratorVersion {
                id,
                received,
                supported,
            } => write!(
                formatter,
                "pattern {id} does not support generator version {received} (supports {supported})"
            ),
        }
    }
}

impl std::error::Error for PatternParameterError {}

/// Existing discrete-mark geometry at the registry boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkPatternOutput {
    pub geometry: MarkSet,
}

/// Existing continuous-path geometry at the registry boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct PathPatternOutput {
    pub geometry: CurveGeometry,
}

/// Artboard coordinates: `(0, 0)` is the top-left pixel corner and positive
/// Y points down. All canonical output is expressed in this space before its
/// consumer scales it to a preview or export surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArtboardSpace {
    pub width: u32,
    pub height: u32,
}

impl ArtboardSpace {
    pub fn validate(self) -> Result<(), CanonicalOutputError> {
        if self.width == 0 || self.height == 0 {
            return Err(CanonicalOutputError::InvalidArtboard);
        }
        Ok(())
    }
}

/// A stable semantic identifier; numeric values are local to one canonical
/// output and are never a persisted pattern-instance authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct RegionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct NetworkNodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct NetworkEdgeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct CanonicalLayerId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalPoint {
    pub x: f32,
    pub y: f32,
}

/// Row-major affine matrix applied in artboard coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffineTransform {
    pub xx: f32,
    pub yx: f32,
    pub xy: f32,
    pub yy: f32,
    pub dx: f32,
    pub dy: f32,
}

impl AffineTransform {
    pub const IDENTITY: Self = Self {
        xx: 1.0,
        yx: 0.0,
        xy: 0.0,
        yy: 1.0,
        dx: 0.0,
        dy: 0.0,
    };

    pub fn apply(self, point: CanonicalPoint) -> CanonicalPoint {
        CanonicalPoint {
            x: self.xx * point.x + self.xy * point.y + self.dx,
            y: self.yx * point.x + self.yy * point.y + self.dy,
        }
    }

    fn validate(self) -> bool {
        [self.xx, self.yx, self.xy, self.yy, self.dx, self.dy]
            .into_iter()
            .all(f32::is_finite)
            && (self.xx * self.yy - self.xy * self.yx).abs() > f32::EPSILON
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingWinding {
    /// Positive signed area in the declared top-left, Y-down coordinate space.
    Clockwise,
    /// Negative signed area in the declared top-left, Y-down coordinate space.
    CounterClockwise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryPolarity {
    Positive,
    /// Removes alpha from already-composed canonical geometry. It is never
    /// rendered as a background-coloured mark or stroke.
    Subtractive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalBlendMode {
    Multiply,
    Screen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

/// Stable channel/layer identity and presentation for region and network
/// geometry. `order` is the deterministic compositing order, then `id`.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalLayer {
    pub id: CanonicalLayerId,
    /// Optional stable output channel; `None` is valid for non-ink
    /// constructive layers while the layer ID still identifies the compositing
    /// target.
    pub channel: Option<crate::artwork_pipeline::OutputChannelId>,
    pub label: String,
    pub order: u32,
    pub color: CanonicalColor,
    pub opacity: f32,
    pub blend_mode: CanonicalBlendMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PolygonRing {
    pub vertices: Vec<CanonicalPoint>,
    pub winding: RingWinding,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilledRegion {
    pub id: RegionId,
    pub layer_id: CanonicalLayerId,
    pub order: u32,
    pub rings: Vec<PolygonRing>,
    pub fill_rule: FillRule,
    pub polarity: GeometryPolarity,
    pub transform: AffineTransform,
}

/// Semantically filled polygonal cells/regions. The artboard is an explicit
/// clip boundary for every consumer; out-of-bounds geometry is valid input.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionPatternOutput {
    pub artboard: ArtboardSpace,
    pub layers: Vec<CanonicalLayer>,
    pub regions: Vec<FilledRegion>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkNode {
    pub id: NetworkNodeId,
    pub point: CanonicalPoint,
}

/// A shared edge references stable node IDs so adjacent cells can share one
/// topology edge instead of independently approximating coincident strokes.
#[derive(Debug, Clone, PartialEq)]
pub struct SharedBoundaryEdge {
    pub id: NetworkEdgeId,
    pub layer_id: CanonicalLayerId,
    pub order: u32,
    pub start: NetworkNodeId,
    pub end: NetworkNodeId,
    pub width: f32,
    pub polarity: GeometryPolarity,
}

/// Semantically connected boundary topology. Positive edges are strokes;
/// subtractive edges are destination-out masks with the same geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkPatternOutput {
    pub artboard: ArtboardSpace,
    pub layers: Vec<CanonicalLayer>,
    pub nodes: Vec<NetworkNode>,
    pub edges: Vec<SharedBoundaryEdge>,
    pub transform: AffineTransform,
}

/// A typed composition for patterns whose semantics intentionally include
/// both filled regions and a shared boundary/network overlay. Components keep
/// their own region/network algebra; this is only the deterministic ordering
/// boundary between those semantic families.
#[derive(Debug, Clone, PartialEq)]
pub struct CompositePatternOutput {
    pub artboard: ArtboardSpace,
    pub regions: Option<RegionPatternOutput>,
    pub network: Option<NetworkPatternOutput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalOutputLimits {
    pub max_layers: usize,
    pub max_regions: usize,
    pub max_vertices: usize,
    pub max_nodes: usize,
    pub max_edges: usize,
}

impl Default for CanonicalOutputLimits {
    fn default() -> Self {
        Self {
            max_layers: 256,
            max_regions: 100_000,
            max_vertices: 1_000_000,
            max_nodes: 1_000_000,
            max_edges: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalOutputError {
    InvalidArtboard,
    NonFiniteGeometry,
    InvalidTransform,
    InvalidOpacity,
    InvalidRing,
    InvalidWinding,
    DuplicateIdentity,
    MissingLayer(CanonicalLayerId),
    MissingNode(NetworkNodeId),
    InvalidEdge,
    EmptyComposite,
    MismatchedArtboard,
    LimitExceeded(&'static str),
}

impl fmt::Display for CanonicalOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArtboard => formatter.write_str("canonical artboard must be positive"),
            Self::NonFiniteGeometry => formatter.write_str("canonical geometry must be finite"),
            Self::InvalidTransform => formatter.write_str("canonical transform must be invertible"),
            Self::InvalidOpacity => {
                formatter.write_str("canonical layer opacity must be between zero and one")
            }
            Self::InvalidRing => {
                formatter.write_str("canonical polygon rings need at least three vertices")
            }
            Self::InvalidWinding => {
                formatter.write_str("canonical ring winding contradicts its vertices")
            }
            Self::DuplicateIdentity => {
                formatter.write_str("canonical identity must be unique within its output")
            }
            Self::MissingLayer(id) => write!(
                formatter,
                "canonical geometry references missing layer {}",
                id.0
            ),
            Self::MissingNode(id) => {
                write!(formatter, "canonical edge references missing node {}", id.0)
            }
            Self::InvalidEdge => formatter
                .write_str("canonical edge must have positive finite width and distinct endpoints"),
            Self::EmptyComposite => formatter
                .write_str("canonical composite must contain a region or network component"),
            Self::MismatchedArtboard => {
                formatter.write_str("canonical composite components must share its artboard")
            }
            Self::LimitExceeded(kind) => {
                write!(formatter, "canonical output exceeds bounded {kind} limit")
            }
        }
    }
}

impl std::error::Error for CanonicalOutputError {}

/// Canonical pattern output remains intentionally split by geometry kind.
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalPatternOutput {
    Marks(MarkPatternOutput),
    Paths(PathPatternOutput),
    Regions(RegionPatternOutput),
    Network(NetworkPatternOutput),
    Composite(CompositePatternOutput),
}

impl CanonicalPatternOutput {
    pub fn artboard(&self) -> ArtboardSpace {
        match self {
            Self::Marks(output) => ArtboardSpace {
                width: output.geometry.width,
                height: output.geometry.height,
            },
            Self::Paths(output) => ArtboardSpace {
                width: output.geometry.width,
                height: output.geometry.height,
            },
            Self::Regions(output) => output.artboard,
            Self::Network(output) => output.artboard,
            Self::Composite(output) => output.artboard,
        }
    }

    pub fn validate(&self) -> Result<(), CanonicalOutputError> {
        self.validate_with_limits(CanonicalOutputLimits::default())
    }

    pub fn validate_with_limits(
        &self,
        limits: CanonicalOutputLimits,
    ) -> Result<(), CanonicalOutputError> {
        match self {
            Self::Marks(output) => ArtboardSpace {
                width: output.geometry.width,
                height: output.geometry.height,
            }
            .validate(),
            Self::Paths(output) => ArtboardSpace {
                width: output.geometry.width,
                height: output.geometry.height,
            }
            .validate(),
            Self::Regions(output) => validate_regions(output, limits),
            Self::Network(output) => validate_network(output, limits),
            Self::Composite(output) => validate_composite(output, limits),
        }
    }
}

fn validate_composite(
    output: &CompositePatternOutput,
    limits: CanonicalOutputLimits,
) -> Result<(), CanonicalOutputError> {
    output.artboard.validate()?;
    if output.regions.is_none() && output.network.is_none() {
        return Err(CanonicalOutputError::EmptyComposite);
    }
    if let Some(regions) = &output.regions {
        if regions.artboard != output.artboard {
            return Err(CanonicalOutputError::MismatchedArtboard);
        }
        validate_regions(regions, limits)?;
    }
    if let Some(network) = &output.network {
        if network.artboard != output.artboard {
            return Err(CanonicalOutputError::MismatchedArtboard);
        }
        validate_network(network, limits)?;
    }
    Ok(())
}

fn validate_layers(
    layers: &[CanonicalLayer],
    limits: CanonicalOutputLimits,
) -> Result<(), CanonicalOutputError> {
    if layers.len() > limits.max_layers {
        return Err(CanonicalOutputError::LimitExceeded("layer"));
    }
    let mut ids = HashSet::new();
    for layer in layers {
        if !ids.insert(layer.id) {
            return Err(CanonicalOutputError::DuplicateIdentity);
        }
        if !layer.opacity.is_finite() || !(0.0..=1.0).contains(&layer.opacity) {
            return Err(CanonicalOutputError::InvalidOpacity);
        }
    }
    Ok(())
}

fn validate_regions(
    output: &RegionPatternOutput,
    limits: CanonicalOutputLimits,
) -> Result<(), CanonicalOutputError> {
    output.artboard.validate()?;
    validate_layers(&output.layers, limits)?;
    if output.regions.len() > limits.max_regions {
        return Err(CanonicalOutputError::LimitExceeded("region"));
    }
    let layers: HashSet<_> = output.layers.iter().map(|layer| layer.id).collect();
    let mut region_ids = HashSet::new();
    let mut vertices = 0usize;
    for region in &output.regions {
        if !region_ids.insert(region.id) {
            return Err(CanonicalOutputError::DuplicateIdentity);
        }
        if !layers.contains(&region.layer_id) {
            return Err(CanonicalOutputError::MissingLayer(region.layer_id));
        }
        if !region.transform.validate() {
            return Err(CanonicalOutputError::InvalidTransform);
        }
        for ring in &region.rings {
            vertices = vertices.saturating_add(ring.vertices.len());
            if vertices > limits.max_vertices {
                return Err(CanonicalOutputError::LimitExceeded("vertex"));
            }
            if ring.vertices.len() < 3 {
                return Err(CanonicalOutputError::InvalidRing);
            }
            let mut area = 0.0f32;
            for (current, next) in ring
                .vertices
                .iter()
                .zip(ring.vertices.iter().cycle().skip(1))
                .take(ring.vertices.len())
            {
                if !current.x.is_finite() || !current.y.is_finite() {
                    return Err(CanonicalOutputError::NonFiniteGeometry);
                }
                area += current.x * next.y - next.x * current.y;
            }
            if area.abs() <= f32::EPSILON
                || (area > 0.0) != matches!(ring.winding, RingWinding::Clockwise)
            {
                return Err(CanonicalOutputError::InvalidWinding);
            }
        }
    }
    Ok(())
}

fn validate_network(
    output: &NetworkPatternOutput,
    limits: CanonicalOutputLimits,
) -> Result<(), CanonicalOutputError> {
    output.artboard.validate()?;
    validate_layers(&output.layers, limits)?;
    if output.nodes.len() > limits.max_nodes {
        return Err(CanonicalOutputError::LimitExceeded("node"));
    }
    if output.edges.len() > limits.max_edges {
        return Err(CanonicalOutputError::LimitExceeded("edge"));
    }
    if !output.transform.validate() {
        return Err(CanonicalOutputError::InvalidTransform);
    }
    let mut nodes = HashSet::new();
    for node in &output.nodes {
        if !nodes.insert(node.id) {
            return Err(CanonicalOutputError::DuplicateIdentity);
        }
        if !node.point.x.is_finite() || !node.point.y.is_finite() {
            return Err(CanonicalOutputError::NonFiniteGeometry);
        }
    }
    let layers: HashSet<_> = output.layers.iter().map(|layer| layer.id).collect();
    let mut edges = HashSet::new();
    for edge in &output.edges {
        if !edges.insert(edge.id) {
            return Err(CanonicalOutputError::DuplicateIdentity);
        }
        if !layers.contains(&edge.layer_id) {
            return Err(CanonicalOutputError::MissingLayer(edge.layer_id));
        }
        if !nodes.contains(&edge.start) {
            return Err(CanonicalOutputError::MissingNode(edge.start));
        }
        if !nodes.contains(&edge.end) {
            return Err(CanonicalOutputError::MissingNode(edge.end));
        }
        if edge.start == edge.end || !edge.width.is_finite() || edge.width <= 0.0 {
            return Err(CanonicalOutputError::InvalidEdge);
        }
    }
    Ok(())
}

/// Failure to adapt legacy renderer geometry through the declared registry
/// compatibility entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyPatternAdapterError {
    UnregisteredPattern(PatternId),
    NonLegacyPattern(PatternId),
    LegacyRenderMismatch {
        id: PatternId,
        expected: LegacyPatternCompatibility,
        actual: LegacyPatternCompatibility,
    },
    OutputKindMismatch {
        id: PatternId,
        expected: PatternOutputKind,
        actual: PatternOutputKind,
    },
}

impl fmt::Display for LegacyPatternAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnregisteredPattern(id) => write!(formatter, "pattern {id} is not registered"),
            Self::NonLegacyPattern(id) => write!(formatter, "pattern {id} has no legacy adapter"),
            Self::LegacyRenderMismatch {
                id,
                expected,
                actual,
            } => write!(
                formatter,
                "pattern {id} declares legacy render {actual:?}, not {expected:?}"
            ),
            Self::OutputKindMismatch {
                id,
                expected,
                actual,
            } => write!(
                formatter,
                "pattern {id} declares output {actual:?}, not {expected:?}"
            ),
        }
    }
}

impl std::error::Error for LegacyPatternAdapterError {}

fn legacy_adapter_metadata(
    id: PatternId,
    expected_legacy_render: LegacyPatternCompatibility,
    expected_output_kind: PatternOutputKind,
) -> Result<&'static PatternMetadata, LegacyPatternAdapterError> {
    let metadata = PATTERN_REGISTRY
        .get(id)
        .ok_or(LegacyPatternAdapterError::UnregisteredPattern(id))?;
    let PatternCompatibility::Legacy {
        legacy_render_variant: actual_legacy_render,
    } = metadata.compatibility
    else {
        return Err(LegacyPatternAdapterError::NonLegacyPattern(id));
    };
    if actual_legacy_render != expected_legacy_render {
        return Err(LegacyPatternAdapterError::LegacyRenderMismatch {
            id,
            expected: expected_legacy_render,
            actual: actual_legacy_render,
        });
    }
    if metadata.output_kind != expected_output_kind {
        return Err(LegacyPatternAdapterError::OutputKindMismatch {
            id,
            expected: expected_output_kind,
            actual: metadata.output_kind,
        });
    }
    Ok(metadata)
}

/// Adapts existing Shapes mark geometry into the registry output boundary.
/// This validates the entry's declared legacy renderer and output kind; it
/// does not route preview or export through the registry.
pub fn adapt_legacy_shapes(
    id: PatternId,
    geometry: MarkSet,
) -> Result<CanonicalPatternOutput, LegacyPatternAdapterError> {
    legacy_adapter_metadata(
        id,
        LegacyPatternCompatibility::ShapesV1,
        PatternOutputKind::Marks,
    )?;
    Ok(CanonicalPatternOutput::Marks(MarkPatternOutput {
        geometry,
    }))
}

/// Adapts existing Curves path geometry into the registry output boundary.
/// This validates the entry's declared legacy renderer and output kind; it
/// does not route preview or export through the registry.
pub fn adapt_legacy_curves(
    id: PatternId,
    geometry: CurveGeometry,
) -> Result<CanonicalPatternOutput, LegacyPatternAdapterError> {
    legacy_adapter_metadata(
        id,
        LegacyPatternCompatibility::CurvesV1,
        PatternOutputKind::Paths,
    )?;
    Ok(CanonicalPatternOutput::Paths(PathPatternOutput {
        geometry,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parameters(id: PatternId) -> VersionedPatternParameters {
        VersionedPatternParameters {
            pattern_id: id,
            schema_version: 1,
            generator_version: 1,
            values: serde_json::Map::new(),
        }
    }

    #[test]
    fn stable_ids_round_trip_through_strings_and_serde() {
        for id in [
            PatternId::COMPATIBILITY_SHAPES_V1,
            PatternId::COMPATIBILITY_CURVES_V1,
        ] {
            assert_eq!(id.to_string().parse::<PatternId>(), Ok(id));
            let serialized = serde_json::to_string(&id).unwrap();
            assert_eq!(serde_json::from_str::<PatternId>(&serialized).unwrap(), id);
        }
        assert!(matches!(
            "compat.shapes".parse::<PatternId>(),
            Err(PatternIdError::Unknown(_))
        ));
        assert!(matches!(
            "Compat.shapes.v1".parse::<PatternId>(),
            Err(PatternIdError::InvalidFormat(_))
        ));
    }

    #[test]
    fn registry_is_unique_and_supports_stable_lookup() {
        PATTERN_REGISTRY.validate().unwrap();
        let shapes = PATTERN_REGISTRY
            .get(PatternId::COMPATIBILITY_SHAPES_V1)
            .unwrap();
        assert_eq!(shapes.output_kind, PatternOutputKind::Marks);
        assert_eq!(
            PATTERN_REGISTRY
                .get(PatternId::COMPATIBILITY_CURVES_V1)
                .unwrap()
                .output_kind,
            PatternOutputKind::Paths
        );
    }

    #[test]
    fn registry_exposes_shapes_and_curves_control_descriptors() {
        let shapes = PATTERN_REGISTRY
            .get(PatternId::COMPATIBILITY_SHAPES_V1)
            .unwrap();
        assert_eq!(shapes.selector.label, "Shapes");
        assert_eq!(
            shapes
                .parameter_for_control("web_shape")
                .map(|descriptor| descriptor.key),
            Some("mark")
        );

        let curves = PATTERN_REGISTRY
            .get(PatternId::COMPATIBILITY_CURVES_V1)
            .unwrap();
        assert_eq!(curves.selector.label, "Curves");
        assert_eq!(
            PATTERN_REGISTRY
                .parameter_for_control(PatternId::COMPATIBILITY_CURVES_V1, "curve_layout")
                .map(|descriptor| descriptor.key),
            Some("layout")
        );
        assert_eq!(
            PATTERN_REGISTRY
                .parameter_for_control(PatternId::COMPATIBILITY_SHAPES_V1, "curve_layout"),
            None
        );
    }

    #[test]
    fn family_and_output_kind_serialize_stably() {
        assert_eq!(
            serde_json::to_string(&PatternFamily::StructuredFields).unwrap(),
            "\"structured-fields\""
        );
        assert_eq!(
            serde_json::to_string(&PatternFamily::StochasticDistributions).unwrap(),
            "\"stochastic-distributions\""
        );
        assert_eq!(
            serde_json::to_string(&PatternFamily::ParametricPaths).unwrap(),
            "\"parametric-paths\""
        );
        assert_eq!(
            serde_json::to_string(&PatternFamily::ConstructivePatterns).unwrap(),
            "\"constructive-patterns\""
        );
        assert_eq!(
            serde_json::to_string(&PatternOutputKind::Marks).unwrap(),
            "\"marks\""
        );
        assert_eq!(
            serde_json::to_string(&PatternOutputKind::Paths).unwrap(),
            "\"paths\""
        );
        assert_eq!(
            serde_json::to_string(&PatternOutputKind::Regions).unwrap(),
            "\"regions\""
        );
        assert_eq!(
            serde_json::to_string(&PatternOutputKind::Networks).unwrap(),
            "\"networks\""
        );
    }

    #[test]
    fn parameters_reject_unsupported_versions() {
        let mut payload = parameters(PatternId::COMPATIBILITY_SHAPES_V1);
        payload.schema_version = 2;
        assert!(matches!(
            payload.validate(),
            Err(PatternParameterError::UnsupportedSchemaVersion { .. })
        ));

        payload.schema_version = 1;
        payload.generator_version = 2;
        assert!(matches!(
            payload.validate(),
            Err(PatternParameterError::UnsupportedGeneratorVersion { .. })
        ));
    }

    #[test]
    fn parameter_payload_rejects_unknown_pattern_ids() {
        let payload = r#"{
            "pattern_id": "compat.new-pattern.v1",
            "schema_version": 1,
            "generator_version": 1,
            "values": {}
        }"#;
        assert!(serde_json::from_str::<VersionedPatternParameters>(payload).is_err());
    }

    #[test]
    fn mark_and_path_outputs_remain_separate() {
        let marks = CanonicalPatternOutput::Marks(MarkPatternOutput {
            geometry: MarkSet {
                width: 1,
                height: 1,
                marks: Vec::new(),
                layers: Vec::new(),
            },
        });
        let paths = CanonicalPatternOutput::Paths(PathPatternOutput {
            geometry: CurveGeometry {
                width: 1,
                height: 1,
                layers: Vec::new(),
            },
        });

        assert!(matches!(marks, CanonicalPatternOutput::Marks(_)));
        assert!(matches!(paths, CanonicalPatternOutput::Paths(_)));
    }

    #[test]
    fn legacy_adapters_preserve_declared_geometry_and_metadata() {
        let marks = MarkSet {
            width: 2,
            height: 3,
            marks: Vec::new(),
            layers: Vec::new(),
        };
        let shapes = PATTERN_REGISTRY
            .get(PatternId::COMPATIBILITY_SHAPES_V1)
            .unwrap();
        assert_eq!(shapes.output_kind, PatternOutputKind::Marks);
        assert!(matches!(
            shapes.compatibility,
            PatternCompatibility::Legacy {
                legacy_render_variant: LegacyPatternCompatibility::ShapesV1
            }
        ));
        assert!(matches!(
            adapt_legacy_shapes(PatternId::COMPATIBILITY_SHAPES_V1, marks.clone()).unwrap(),
            CanonicalPatternOutput::Marks(MarkPatternOutput { geometry }) if geometry == marks
        ));

        let curves = CurveGeometry {
            width: 5,
            height: 7,
            layers: Vec::new(),
        };
        let curve_metadata = PATTERN_REGISTRY
            .get(PatternId::COMPATIBILITY_CURVES_V1)
            .unwrap();
        assert_eq!(curve_metadata.output_kind, PatternOutputKind::Paths);
        assert!(matches!(
            curve_metadata.compatibility,
            PatternCompatibility::Legacy {
                legacy_render_variant: LegacyPatternCompatibility::CurvesV1
            }
        ));
        assert!(matches!(
            adapt_legacy_curves(PatternId::COMPATIBILITY_CURVES_V1, curves.clone()).unwrap(),
            CanonicalPatternOutput::Paths(PathPatternOutput { geometry }) if geometry == curves
        ));
    }

    #[test]
    fn legacy_adapters_reject_mismatched_registry_entries() {
        let marks = MarkSet {
            width: 1,
            height: 1,
            marks: Vec::new(),
            layers: Vec::new(),
        };
        assert!(matches!(
            adapt_legacy_shapes(PatternId::COMPATIBILITY_CURVES_V1, marks),
            Err(LegacyPatternAdapterError::LegacyRenderMismatch { .. })
        ));

        let curves = CurveGeometry {
            width: 1,
            height: 1,
            layers: Vec::new(),
        };
        assert!(matches!(
            adapt_legacy_curves(PatternId::COMPATIBILITY_SHAPES_V1, curves),
            Err(LegacyPatternAdapterError::LegacyRenderMismatch { .. })
        ));
    }

    fn layer() -> CanonicalLayer {
        CanonicalLayer {
            id: CanonicalLayerId(7),
            channel: None,
            label: "Fixture Ink".into(),
            order: 0,
            color: CanonicalColor {
                red: 220,
                green: 30,
                blue: 40,
            },
            opacity: 0.5,
            blend_mode: CanonicalBlendMode::Multiply,
        }
    }

    fn clockwise(points: &[(f32, f32)]) -> PolygonRing {
        PolygonRing {
            vertices: points
                .iter()
                .map(|&(x, y)| CanonicalPoint { x, y })
                .collect(),
            winding: RingWinding::Clockwise,
        }
    }

    fn counter_clockwise(points: &[(f32, f32)]) -> PolygonRing {
        PolygonRing {
            vertices: points
                .iter()
                .map(|&(x, y)| CanonicalPoint { x, y })
                .collect(),
            winding: RingWinding::CounterClockwise,
        }
    }

    fn region_fixture() -> CanonicalPatternOutput {
        CanonicalPatternOutput::Regions(RegionPatternOutput {
            artboard: ArtboardSpace {
                width: 12,
                height: 12,
            },
            layers: vec![layer()],
            regions: vec![
                FilledRegion {
                    id: RegionId(1),
                    layer_id: CanonicalLayerId(7),
                    order: 1,
                    rings: vec![
                        clockwise(&[(-2.0, -2.0), (12.0, -2.0), (12.0, 12.0), (-2.0, 12.0)]),
                        counter_clockwise(&[(3.0, 3.0), (3.0, 7.0), (7.0, 7.0), (7.0, 3.0)]),
                    ],
                    fill_rule: FillRule::EvenOdd,
                    polarity: GeometryPolarity::Positive,
                    transform: AffineTransform::IDENTITY,
                },
                FilledRegion {
                    id: RegionId(2),
                    layer_id: CanonicalLayerId(7),
                    order: 2,
                    rings: vec![clockwise(&[
                        (8.0, 1.0),
                        (11.0, 1.0),
                        (11.0, 5.0),
                        (8.0, 5.0),
                    ])],
                    fill_rule: FillRule::NonZero,
                    polarity: GeometryPolarity::Subtractive,
                    transform: AffineTransform::IDENTITY,
                },
            ],
        })
    }

    #[test]
    fn regions_preserve_holes_clipping_opacity_and_subtractive_masks() {
        let output = region_fixture();
        output.validate().unwrap();
        let image = crate::render::render_canonical_pattern_output_cancellable(
            &output,
            12,
            12,
            false,
            None,
            &crate::CancellationToken::new(),
        )
        .unwrap();
        // The outer ring is clipped to the artboard, while the EvenOdd hole
        // and later destination-out region remove alpha rather than paint.
        assert_eq!(image.get_pixel(0, 0)[3], 128);
        assert_eq!(image.get_pixel(5, 5)[3], 0);
        assert_eq!(image.get_pixel(9, 2)[3], 0);
        assert_eq!(image.get_pixel(1, 10)[3], 128);
    }

    #[test]
    fn transforms_and_stable_order_are_deterministic() {
        let mut output = region_fixture();
        let CanonicalPatternOutput::Regions(regions) = &mut output else {
            unreachable!()
        };
        regions.regions = vec![FilledRegion {
            id: RegionId(9),
            layer_id: CanonicalLayerId(7),
            order: 3,
            rings: vec![clockwise(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)])],
            fill_rule: FillRule::NonZero,
            polarity: GeometryPolarity::Positive,
            transform: AffineTransform {
                dx: 5.0,
                dy: 4.0,
                ..AffineTransform::IDENTITY
            },
        }];
        let before = crate::render::render_canonical_pattern_output_cancellable(
            &output,
            12,
            12,
            false,
            None,
            &crate::CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(before.get_pixel(6, 5)[3], 128);
        let CanonicalPatternOutput::Regions(regions) = &mut output else {
            unreachable!()
        };
        regions.layers.reverse();
        let after = crate::render::render_canonical_pattern_output_cancellable(
            &output,
            12,
            12,
            false,
            None,
            &crate::CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn network_keeps_shared_node_edge_topology_and_bounded_validation() {
        let output = CanonicalPatternOutput::Network(NetworkPatternOutput {
            artboard: ArtboardSpace {
                width: 10,
                height: 10,
            },
            layers: vec![layer()],
            nodes: vec![
                NetworkNode {
                    id: NetworkNodeId(1),
                    point: CanonicalPoint { x: 1.0, y: 1.0 },
                },
                NetworkNode {
                    id: NetworkNodeId(2),
                    point: CanonicalPoint { x: 5.0, y: 5.0 },
                },
                NetworkNode {
                    id: NetworkNodeId(3),
                    point: CanonicalPoint { x: 9.0, y: 1.0 },
                },
            ],
            edges: vec![
                SharedBoundaryEdge {
                    id: NetworkEdgeId(1),
                    layer_id: CanonicalLayerId(7),
                    order: 0,
                    start: NetworkNodeId(1),
                    end: NetworkNodeId(2),
                    width: 1.0,
                    polarity: GeometryPolarity::Positive,
                },
                SharedBoundaryEdge {
                    id: NetworkEdgeId(2),
                    layer_id: CanonicalLayerId(7),
                    order: 1,
                    start: NetworkNodeId(2),
                    end: NetworkNodeId(3),
                    width: 1.0,
                    polarity: GeometryPolarity::Positive,
                },
            ],
            transform: AffineTransform::IDENTITY,
        });
        output.validate().unwrap();
        assert!(matches!(
            output.validate_with_limits(CanonicalOutputLimits {
                max_edges: 1,
                ..CanonicalOutputLimits::default()
            }),
            Err(CanonicalOutputError::LimitExceeded("edge"))
        ));
    }

    #[test]
    fn canonical_raster_checks_cancellation_before_allocating() {
        let token = crate::CancellationToken::new();
        assert!(token.cancel());
        assert!(
            crate::render::render_canonical_pattern_output_cancellable(
                &region_fixture(),
                12,
                12,
                false,
                None,
                &token,
            )
            .is_err()
        );
    }
}
