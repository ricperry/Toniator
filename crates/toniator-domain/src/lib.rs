#![forbid(unsafe_code)]

//! Authoritative, headless document concepts for Toniator.

use std::{collections::HashSet, error::Error, fmt};

use serde::Serialize;

/// A stable identifier for an authoritative document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(pub u64);

/// A stable identifier for a structural pattern definition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatternDefinitionId(pub u64);

/// A stable address for a typed structural mechanism.  These IDs are owned by
/// the document rather than by an evaluator or a UI draft.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatternMechanismId(pub u64);

/// A stable address for one ordered typed output layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatternOutputLayerId(pub u64);

/// A stable identifier for one structural guide dimension.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuideDimensionId(pub u64);

/// A finite, independently-addressable repeated straight-guide dimension.
/// `baseline_angle_degrees` describes the local-space normal; the channel
/// layout transform is applied by family planning, never after a finite set of
/// guides has been generated.
#[derive(Clone, Debug, PartialEq)]
pub struct StraightGuideDimension {
    pub id: GuideDimensionId,
    pub baseline_angle_degrees: f64,
    pub phase: f64,
    pub repetition: StraightGuideRepetition,
}

/// The deliberately small Stage 16A repetition vocabulary.  The spacing is a
/// multiplier of the resolved channel density, so authored density remains a
/// channel concern rather than becoming a second structural density system.
#[derive(Clone, Debug, PartialEq)]
pub struct StraightGuideRepetition {
    pub spacing_multiplier: f64,
}

/// Stable typed orientation for a mark prototype.  Circle rendering remains
/// visually invariant, but orientation is still part of the realization
/// contract so later compatible prototypes do not require renderer dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkOrientation {
    Fixed,
    GuideTangent { dimension_id: GuideDimensionId },
    GuideNormal { dimension_id: GuideDimensionId },
}

/// The only Stage 16A prototype.  It is intentionally explicit rather than
/// relying on a renderer-specific "circle" fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkPrototype {
    Circle,
}

/// A stable identifier for a channel owned by a document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelId(pub u64);

/// The discrete revision of an authoritative document session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

/// A stable logical identifier for source artwork. It deliberately does not
/// encode a filesystem location, source bytes, or decoded pixels.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct SourceReferenceId(String);

impl SourceReferenceId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ValidationError::new(
                "source.reference_id",
                "source reference ID must not be empty",
            ));
        }
        if value.contains('/') || value.contains('\\') {
            return Err(ValidationError::new(
                "source.reference_id",
                "source reference ID must not be a filesystem path",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Document-space canvas dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSpec {
    pub width: f64,
    pub height: f64,
}

/// The continuous density metric supplied to a channel's pattern layout.
#[derive(Clone, Debug, PartialEq)]
pub struct DensityMetric2D {
    pub across_x: f64,
    pub across_y: f64,
    pub aspect_locked: bool,
}

/// Per-channel placement controls.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelPatternLayout {
    pub density: DensityMetric2D,
    pub rotation_degrees: f64,
    pub translation_x: f64,
    pub translation_y: f64,
}

/// Canonical linear RGBA color components.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorValue {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

/// Presentation settings that do not alter canonical geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelAppearance {
    pub visible: bool,
    pub color: ColorValue,
    pub opacity: f64,
}

/// The current minimal mark-size response contract.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkGeometryResponse {
    pub minimum_size: f64,
    pub maximum_size: f64,
}

/// The source component selected by a channel's authoritative source mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceComponent {
    Luminance,
    Alpha,
}

/// The sole supported document-to-source placement contract.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourcePlacement {
    #[default]
    StretchToCanvas,
}

/// Authoritative source interpretation for one channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelSourceMapping {
    pub component: SourceComponent,
    pub placement: SourcePlacement,
}

/// The fixed set of authoritative complete-document channel arrangements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HalftoneChannelModel {
    Rgb,
    Cmyk,
    SourceColorAlpha,
}

/// One semantic role in an ordered halftone channel topology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HalftoneChannelRole {
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    Black,
    SourceColor,
}

impl HalftoneChannelModel {
    fn canonical_roles(self) -> &'static [HalftoneChannelRole] {
        match self {
            Self::Rgb => &[
                HalftoneChannelRole::Red,
                HalftoneChannelRole::Green,
                HalftoneChannelRole::Blue,
            ],
            Self::Cmyk => &[
                HalftoneChannelRole::Cyan,
                HalftoneChannelRole::Magenta,
                HalftoneChannelRole::Yellow,
                HalftoneChannelRole::Black,
            ],
            Self::SourceColorAlpha => &[HalftoneChannelRole::SourceColor],
        }
    }
}

/// A scalar component supported by the Stage 9 channel-mapping authority.
///
/// Evaluation of these source fields is deliberately deferred to Stage 9B.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMappingComponent {
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    Black,
    Alpha,
    Luminance,
}

/// Complete source-mapping state used by a modeled topology.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceMapping {
    pub component: SourceMappingComponent,
    pub placement: SourcePlacement,
    pub inverted: bool,
    pub gain: f64,
    pub bias: f64,
}

impl SourceMapping {
    pub const fn canonical(component: SourceMappingComponent) -> Self {
        Self {
            component,
            placement: SourcePlacement::StretchToCanvas,
            inverted: false,
            gain: 1.0,
            bias: 0.0,
        }
    }

    /// Applies the authoritative Stage 9 transform. Source-field evaluation is
    /// intentionally not part of this domain-only slice.
    pub fn transform(self, value: f64) -> f64 {
        let value = if self.inverted { 1.0 - value } else { value };
        (self.gain * value + self.bias).clamp(0.0, 1.0)
    }
}

/// Presentation paint for a modeled channel. Sampled source paint is a
/// validated representation only until Stage 9B realizes it.
#[derive(Clone, Debug, PartialEq)]
pub enum ChannelPaint {
    Solid(ColorValue),
    SampledSource,
}

/// The pattern/layout/mark-response input cloned by a canonical topology
/// factory. It deliberately has no role-specific geometry defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelTopologyTemplate {
    pub pattern_definition_id: PatternDefinitionId,
    pub layout: ChannelPatternLayout,
    pub mark_geometry_response: MarkGeometryResponse,
}

/// One complete modeled channel in its authoritative ordered topology.
#[derive(Clone, Debug, PartialEq)]
pub struct ModeledChannelState {
    pub role: HalftoneChannelRole,
    pub id: ChannelId,
    pub pattern_definition_id: PatternDefinitionId,
    pub layout: ChannelPatternLayout,
    pub mark_geometry_response: MarkGeometryResponse,
    pub mapping: SourceMapping,
    pub paint: ChannelPaint,
    pub visible: bool,
    pub opacity: f64,
}

/// A complete ordered channel topology supplied atomically with its model.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelTopology {
    channels: Vec<ModeledChannelState>,
}

impl ChannelTopology {
    pub fn new(channels: Vec<ModeledChannelState>) -> Self {
        Self { channels }
    }

    pub fn channels(&self) -> &[ModeledChannelState] {
        &self.channels
    }

    /// Builds the exact canonical topology for `model` by cloning one caller
    /// supplied template. Pattern-definition compatibility is checked when the
    /// topology is validated against a document.
    pub fn canonical(
        model: HalftoneChannelModel,
        template: ChannelTopologyTemplate,
    ) -> Result<Self, ValidationError> {
        validate_layout(&template.layout)?;
        validate_mark_response(&template.mark_geometry_response, None)?;

        Ok(Self::new(
            model
                .canonical_roles()
                .iter()
                .copied()
                .map(|role| ModeledChannelState::canonical(role, &template))
                .collect(),
        ))
    }
}

impl ModeledChannelState {
    fn canonical(role: HalftoneChannelRole, template: &ChannelTopologyTemplate) -> Self {
        let (id, component, paint) = match role {
            HalftoneChannelRole::Red => (
                ChannelId(1),
                SourceMappingComponent::Red,
                ChannelPaint::Solid(ColorValue::red()),
            ),
            HalftoneChannelRole::Green => (
                ChannelId(2),
                SourceMappingComponent::Green,
                ChannelPaint::Solid(ColorValue::green()),
            ),
            HalftoneChannelRole::Blue => (
                ChannelId(3),
                SourceMappingComponent::Blue,
                ChannelPaint::Solid(ColorValue::blue()),
            ),
            HalftoneChannelRole::Cyan => (
                ChannelId(4),
                SourceMappingComponent::Cyan,
                ChannelPaint::Solid(ColorValue::cyan()),
            ),
            HalftoneChannelRole::Magenta => (
                ChannelId(5),
                SourceMappingComponent::Magenta,
                ChannelPaint::Solid(ColorValue::magenta()),
            ),
            HalftoneChannelRole::Yellow => (
                ChannelId(6),
                SourceMappingComponent::Yellow,
                ChannelPaint::Solid(ColorValue::yellow()),
            ),
            HalftoneChannelRole::Black => (
                ChannelId(7),
                SourceMappingComponent::Black,
                ChannelPaint::Solid(ColorValue::black()),
            ),
            HalftoneChannelRole::SourceColor => (
                ChannelId(8),
                SourceMappingComponent::Alpha,
                ChannelPaint::SampledSource,
            ),
        };
        let mapping = SourceMapping::canonical(component);
        Self {
            role,
            id,
            pattern_definition_id: template.pattern_definition_id,
            layout: template.layout.clone(),
            mark_geometry_response: template.mark_geometry_response.clone(),
            mapping,
            paint,
            visible: true,
            opacity: 1.0,
        }
    }
}

impl ColorValue {
    fn red() -> Self {
        Self {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        }
    }
    fn green() -> Self {
        Self {
            red: 0.0,
            green: 1.0,
            blue: 0.0,
            alpha: 1.0,
        }
    }
    fn blue() -> Self {
        Self {
            red: 0.0,
            green: 0.0,
            blue: 1.0,
            alpha: 1.0,
        }
    }
    fn cyan() -> Self {
        Self {
            red: 0.0,
            green: 1.0,
            blue: 1.0,
            alpha: 1.0,
        }
    }
    fn magenta() -> Self {
        Self {
            red: 1.0,
            green: 0.0,
            blue: 1.0,
            alpha: 1.0,
        }
    }
    fn yellow() -> Self {
        Self {
            red: 1.0,
            green: 1.0,
            blue: 0.0,
            alpha: 1.0,
        }
    }
    fn black() -> Self {
        Self {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        }
    }
}

/// Exactly one structural root for a pattern definition.  The currently
/// supported root deliberately names mechanisms rather than a named artistic
/// result; later roots are data additions, not renderer branches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternFamily {
    GuideIntersections {
        guide_mechanism_id: PatternMechanismId,
        site_mechanism_id: PatternMechanismId,
    },
}

/// A typed reusable structural mechanism.  Stage 14 retains only the accepted
/// straight-guide/intersection meaning; it is intentionally not a node graph.
#[derive(Clone, Debug, PartialEq)]
pub enum PatternMechanism {
    StraightGuides {
        id: PatternMechanismId,
    },
    GuideIntersections {
        id: PatternMechanismId,
        guide_mechanism_id: PatternMechanismId,
    },
    /// Generalized ordered straight dimensions.  The legacy `StraightGuides`
    /// form remains readable/writable for exact existing-v2 bytes.
    StraightGuideDimensions {
        id: PatternMechanismId,
        dimensions: Vec<StraightGuideDimension>,
    },
    /// An explicit selected intersection product; selections are stable IDs,
    /// not positional aliases.
    SelectedGuideIntersections {
        id: PatternMechanismId,
        guide_mechanism_id: PatternMechanismId,
        dimensions: Vec<GuideDimensionId>,
        merge_epsilon: f64,
    },
    /// Regular arc-length sites along explicitly selected guides.
    AlongGuideSites {
        id: PatternMechanismId,
        guide_mechanism_id: PatternMechanismId,
        dimensions: Vec<GuideDimensionId>,
        interval_multiplier: f64,
        phase: f64,
    },
}

impl PatternMechanism {
    pub const fn id(&self) -> PatternMechanismId {
        match self {
            Self::StraightGuides { id }
            | Self::GuideIntersections { id, .. }
            | Self::StraightGuideDimensions { id, .. }
            | Self::SelectedGuideIntersections { id, .. }
            | Self::AlongGuideSites { id, .. } => *id,
        }
    }
}

/// One ordered typed output layer.  Additional layer variants are deferred;
/// their absence is a capability failure, never a legacy fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternOutputLayer {
    CircularMarks {
        id: PatternOutputLayerId,
        site_mechanism_id: PatternMechanismId,
    },
    MarkPrototype {
        id: PatternOutputLayerId,
        site_mechanism_id: PatternMechanismId,
        prototype: MarkPrototype,
        orientation: MarkOrientation,
    },
}

impl PatternOutputLayer {
    pub const fn id(&self) -> PatternOutputLayerId {
        match self {
            Self::CircularMarks { id, .. } | Self::MarkPrototype { id, .. } => *id,
        }
    }
}

/// Structural modulation is a separate typed top-level slot.  The accepted
/// v1 configuration has no modulation; later additions must remain typed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PatternModulation;

/// Structural coverage planning.  Canvas boundaries remain final-consumer
/// clipping and are never topology input.
#[derive(Clone, Debug, PartialEq)]
pub struct CoveragePolicy {
    pub guard_steps: u32,
    pub maximum_support_radius: f64,
}

/// Structural pattern authority owned by the document.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternDefinition {
    pub id: PatternDefinitionId,
    pub name: String,
    pub family: PatternFamily,
    pub mechanisms: Vec<PatternMechanism>,
    pub output_layers: Vec<PatternOutputLayer>,
    pub modulation: PatternModulation,
    pub coverage: CoveragePolicy,
}

impl PatternDefinition {
    /// Builds the sole Stage 14 adapter configuration.  It is data only: the
    /// engine asks this definition for typed capability, never for a preset or
    /// legacy name.
    pub fn supported_straight_grid(
        id: PatternDefinitionId,
        name: impl Into<String>,
        guide_id: PatternMechanismId,
        intersections_id: PatternMechanismId,
        output_id: PatternOutputLayerId,
        coverage: CoveragePolicy,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            family: PatternFamily::GuideIntersections {
                guide_mechanism_id: guide_id,
                site_mechanism_id: intersections_id,
            },
            mechanisms: vec![
                PatternMechanism::StraightGuides { id: guide_id },
                PatternMechanism::GuideIntersections {
                    id: intersections_id,
                    guide_mechanism_id: guide_id,
                },
            ],
            output_layers: vec![PatternOutputLayer::CircularMarks {
                id: output_id,
                site_mechanism_id: intersections_id,
            }],
            modulation: PatternModulation,
            coverage,
        }
    }

    /// Constructs an explicit generalized straight-guide definition without a
    /// named pattern/preset discriminator.  Selection IDs must appear in the
    /// stored dimension order and are validated with the document.
    #[allow(clippy::too_many_arguments)] // Stable IDs are intentionally explicit at the schema boundary.
    pub fn generalized_straight_guides(
        id: PatternDefinitionId,
        name: impl Into<String>,
        guide_id: PatternMechanismId,
        site_id: PatternMechanismId,
        output_id: PatternOutputLayerId,
        dimensions: Vec<StraightGuideDimension>,
        product: GeneralizedSiteProduct,
        orientation: MarkOrientation,
        coverage: CoveragePolicy,
    ) -> Self {
        let site = match product {
            GeneralizedSiteProduct::Intersections {
                dimensions,
                merge_epsilon,
            } => PatternMechanism::SelectedGuideIntersections {
                id: site_id,
                guide_mechanism_id: guide_id,
                dimensions,
                merge_epsilon,
            },
            GeneralizedSiteProduct::AlongGuides {
                dimensions,
                interval_multiplier,
                phase,
            } => PatternMechanism::AlongGuideSites {
                id: site_id,
                guide_mechanism_id: guide_id,
                dimensions,
                interval_multiplier,
                phase,
            },
        };
        Self {
            id,
            name: name.into(),
            family: PatternFamily::GuideIntersections {
                guide_mechanism_id: guide_id,
                site_mechanism_id: site_id,
            },
            mechanisms: vec![
                PatternMechanism::StraightGuideDimensions {
                    id: guide_id,
                    dimensions,
                },
                site,
            ],
            output_layers: vec![PatternOutputLayer::MarkPrototype {
                id: output_id,
                site_mechanism_id: site_id,
                prototype: MarkPrototype::Circle,
                orientation,
            }],
            modulation: PatternModulation,
            coverage,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GeneralizedSiteProduct {
    Intersections {
        dimensions: Vec<GuideDimensionId>,
        merge_epsilon: f64,
    },
    AlongGuides {
        dimensions: Vec<GuideDimensionId>,
        interval_multiplier: f64,
        phase: f64,
    },
}

/// Source state owned by the document, never a filesystem path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SourceReference {
    #[default]
    Unassigned,
    Assigned(SourceReferenceId),
}

/// Per-channel authoritative state.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelState {
    pub id: ChannelId,
    pub pattern_definition_id: PatternDefinitionId,
    pub layout: ChannelPatternLayout,
    pub appearance: ChannelAppearance,
    pub mark_geometry_response: MarkGeometryResponse,
    pub source_mapping: ChannelSourceMapping,
}

/// The sole document model. Its collections are read-only outside this crate.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    id: DocumentId,
    canvas: CanvasSpec,
    source: SourceReference,
    pattern_definitions: Vec<PatternDefinition>,
    channel_configuration: ChannelConfiguration,
}

#[derive(Clone, Debug, PartialEq)]
enum ChannelConfiguration {
    Legacy(Vec<ChannelState>),
    Topology {
        model: HalftoneChannelModel,
        topology: ChannelTopology,
    },
}

impl Document {
    /// Builds the accepted default document directly, without a
    /// command transition.  Frontends use this for new and direct-source
    /// workspaces so their `DocumentSession`/`DocumentHistory` begins at
    /// revision zero. The fixed template stays encapsulated in the headless
    /// document layer rather than requiring caller-side construction details.
    pub fn new_default_document(
        canvas: CanvasSpec,
        source: SourceReference,
    ) -> Result<Self, ValidationError> {
        let layout = ChannelPatternLayout {
            density: DensityMetric2D {
                across_x: canvas.width / 10.0,
                across_y: canvas.height / 10.0,
                aspect_locked: true,
            },
            rotation_degrees: 0.0,
            translation_x: 0.0,
            translation_y: 0.0,
        };
        let response = MarkGeometryResponse {
            minimum_size: 2.0,
            maximum_size: 9.0,
        };
        let definition = PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "Straight circular marks",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps: 2,
                maximum_support_radius: 4.5,
            },
        );
        let template = ChannelTopologyTemplate {
            pattern_definition_id: definition.id,
            layout,
            mark_geometry_response: response,
        };
        let topology = ChannelTopology::canonical(HalftoneChannelModel::Rgb, template)?;
        Self::with_source_and_topology(
            DocumentId(1),
            canvas,
            source,
            vec![definition],
            HalftoneChannelModel::Rgb,
            topology,
        )
    }

    /// Constructs and validates a document with an unassigned source reference.
    pub fn new(
        id: DocumentId,
        canvas: CanvasSpec,
        pattern_definitions: Vec<PatternDefinition>,
        channels: Vec<ChannelState>,
    ) -> Result<Self, ValidationError> {
        Self::with_source(
            id,
            canvas,
            SourceReference::Unassigned,
            pattern_definitions,
            channels,
        )
    }

    /// Constructs and validates a document with explicitly supplied source state.
    pub fn with_source(
        id: DocumentId,
        canvas: CanvasSpec,
        source: SourceReference,
        pattern_definitions: Vec<PatternDefinition>,
        channels: Vec<ChannelState>,
    ) -> Result<Self, ValidationError> {
        let document = Self {
            id,
            canvas,
            source,
            pattern_definitions,
            channel_configuration: ChannelConfiguration::Legacy(channels),
        };
        document.validate()?;
        Ok(document)
    }

    /// Constructs a complete modeled document from an explicit, already
    /// ordered topology. This is intentionally the narrow construction seam
    /// used by persistence to rebuild a validated authoritative document; it
    /// does not expose the private channel-configuration representation.
    pub fn with_source_and_topology(
        id: DocumentId,
        canvas: CanvasSpec,
        source: SourceReference,
        pattern_definitions: Vec<PatternDefinition>,
        model: HalftoneChannelModel,
        topology: ChannelTopology,
    ) -> Result<Self, ValidationError> {
        let document = Self {
            id,
            canvas,
            source,
            pattern_definitions,
            channel_configuration: ChannelConfiguration::Topology { model, topology },
        };
        document.validate()?;
        Ok(document)
    }

    pub fn id(&self) -> DocumentId {
        self.id
    }

    pub fn canvas(&self) -> &CanvasSpec {
        &self.canvas
    }

    pub fn source(&self) -> &SourceReference {
        &self.source
    }

    pub fn pattern_definitions(&self) -> &[PatternDefinition] {
        &self.pattern_definitions
    }

    /// Returns legacy single-channel state only. A modeled topology is not a
    /// legacy evaluation input and therefore is intentionally not projected.
    pub fn channels(&self) -> Option<&[ChannelState]> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => Some(channels),
            ChannelConfiguration::Topology { .. } => None,
        }
    }

    /// The installed Stage 9 model, if this document has received an atomic
    /// model/topology replacement. Existing pre-Stage-9 documents retain their
    /// accepted single-channel representation until explicitly replaced.
    pub fn channel_model(&self) -> Option<HalftoneChannelModel> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(_) => None,
            ChannelConfiguration::Topology { model, .. } => Some(*model),
        }
    }

    pub fn channel_topology(&self) -> Option<&ChannelTopology> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(_) => None,
            ChannelConfiguration::Topology { topology, .. } => Some(topology),
        }
    }

    /// Builds and validates a canonical modeled topology using definitions in
    /// this document. The factory never selects geometry settings.
    pub fn canonical_channel_topology(
        &self,
        model: HalftoneChannelModel,
        template: ChannelTopologyTemplate,
    ) -> Result<ChannelTopology, ValidationError> {
        let topology = ChannelTopology::canonical(model, template)?;
        validate_topology(model, &topology, &self.pattern_definitions)?;
        Ok(topology)
    }

    /// Returns a legacy channel only; modeled channels are available through
    /// `channel_topology` and `modeled_channel`.
    pub fn channel(&self, channel_id: ChannelId) -> Option<&ChannelState> {
        self.channels()?
            .iter()
            .find(|channel| channel.id == channel_id)
    }

    pub fn modeled_channel(&self, channel_id: ChannelId) -> Option<&ModeledChannelState> {
        self.channel_topology()?
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
    }

    fn channel_ids(&self) -> Vec<ChannelId> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => {
                channels.iter().map(|channel| channel.id).collect()
            }
            ChannelConfiguration::Topology { topology, .. } => {
                topology.channels.iter().map(|channel| channel.id).collect()
            }
        }
    }

    fn has_channel(&self, channel_id: ChannelId) -> bool {
        self.channel_ids().contains(&channel_id)
    }

    fn pattern_definition_id_for(&self, channel_id: ChannelId) -> Option<PatternDefinitionId> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .map(|channel| channel.pattern_definition_id),
            ChannelConfiguration::Topology { topology, .. } => topology
                .channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .map(|channel| channel.pattern_definition_id),
        }
    }

    fn definition(&self, id: PatternDefinitionId) -> Option<&PatternDefinition> {
        self.pattern_definitions
            .iter()
            .find(|definition| definition.id == id)
    }

    fn linked_channels(&self, definition_id: PatternDefinitionId) -> Vec<ChannelId> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => channels
                .iter()
                .filter(|channel| channel.pattern_definition_id == definition_id)
                .map(|channel| channel.id)
                .collect(),
            ChannelConfiguration::Topology { topology, .. } => topology
                .channels
                .iter()
                .filter(|channel| channel.pattern_definition_id == definition_id)
                .map(|channel| channel.id)
                .collect(),
        }
    }

    fn allocate_definition_id(&self) -> Result<PatternDefinitionId, ValidationError> {
        next_id(
            self.pattern_definitions
                .iter()
                .map(|definition| definition.id.0),
            "pattern_definitions.id",
        )
        .map(PatternDefinitionId)
    }

    fn allocate_mechanism_id(&self) -> Result<PatternMechanismId, ValidationError> {
        next_id(
            self.pattern_definitions.iter().flat_map(|definition| {
                definition
                    .mechanisms
                    .iter()
                    .map(|mechanism| mechanism.id().0)
            }),
            "pattern_definitions.mechanisms.id",
        )
        .map(PatternMechanismId)
    }

    fn allocate_output_layer_id(&self) -> Result<PatternOutputLayerId, ValidationError> {
        next_id(
            self.pattern_definitions
                .iter()
                .flat_map(|definition| definition.output_layers.iter().map(|layer| layer.id().0)),
            "pattern_definitions.output_layers.id",
        )
        .map(PatternOutputLayerId)
    }

    fn allocate_dimension_id(&self) -> Result<GuideDimensionId, ValidationError> {
        next_id(
            self.pattern_definitions
                .iter()
                .flat_map(|definition| definition.mechanisms.iter())
                .flat_map(|mechanism| match mechanism {
                    PatternMechanism::StraightGuideDimensions { dimensions, .. } => dimensions
                        .iter()
                        .map(|dimension| dimension.id.0)
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                }),
            "pattern_definitions.mechanisms.dimensions.id",
        )
        .map(GuideDimensionId)
    }

    fn allocate_definition_from_draft(
        &self,
        draft: &PatternDefinitionDraft,
    ) -> Result<PatternDefinition, ValidationError> {
        validate_definition_draft(draft)?;
        let id = self.allocate_definition_id()?;
        let guide_id = self.allocate_mechanism_id()?;
        let intersection_id = PatternMechanismId(guide_id.0.checked_add(1).ok_or_else(|| {
            ValidationError::new(
                "pattern_definitions.mechanisms.id",
                "document mechanism ID space is exhausted",
            )
        })?);
        if self
            .pattern_definitions
            .iter()
            .flat_map(|definition| definition.mechanisms.iter())
            .any(|mechanism| mechanism.id() == intersection_id)
        {
            return Err(ValidationError::new(
                "pattern_definitions.mechanisms.id",
                "document mechanism ID allocation collided",
            ));
        }
        let output_id = self.allocate_output_layer_id()?;
        Ok(PatternDefinition::supported_straight_grid(
            id,
            draft.name.clone(),
            guide_id,
            intersection_id,
            output_id,
            draft.coverage.clone(),
        ))
    }

    fn duplicate_definition(
        &self,
        source: &PatternDefinition,
    ) -> Result<PatternDefinition, ValidationError> {
        if let [
            PatternMechanism::StraightGuideDimensions { dimensions, .. },
            site,
        ] = source.mechanisms.as_slice()
        {
            let id = self.allocate_definition_id()?;
            let guide_id = self.allocate_mechanism_id()?;
            let site_id = PatternMechanismId(guide_id.0.checked_add(1).ok_or_else(|| {
                ValidationError::new(
                    "pattern_definitions.mechanisms.id",
                    "document mechanism ID space is exhausted",
                )
            })?);
            let output_id = self.allocate_output_layer_id()?;
            let mut next_dimension = self.allocate_dimension_id()?.0;
            let mut remapped = Vec::with_capacity(dimensions.len());
            for dimension in dimensions {
                let new_id = GuideDimensionId(next_dimension);
                next_dimension = next_dimension.checked_add(1).ok_or_else(|| {
                    ValidationError::new(
                        "pattern_definitions.mechanisms.dimensions.id",
                        "document dimension ID space is exhausted",
                    )
                })?;
                remapped.push((
                    dimension.id,
                    StraightGuideDimension {
                        id: new_id,
                        baseline_angle_degrees: dimension.baseline_angle_degrees,
                        phase: dimension.phase,
                        repetition: dimension.repetition.clone(),
                    },
                ));
            }
            let remap = |old: GuideDimensionId| {
                remapped
                    .iter()
                    .find(|(candidate, _)| *candidate == old)
                    .map(|(_, value)| value.id)
                    .expect("source selection is validated")
            };
            let product = match site {
                PatternMechanism::SelectedGuideIntersections {
                    dimensions,
                    merge_epsilon,
                    ..
                } => GeneralizedSiteProduct::Intersections {
                    dimensions: dimensions.iter().copied().map(remap).collect(),
                    merge_epsilon: *merge_epsilon,
                },
                PatternMechanism::AlongGuideSites {
                    dimensions,
                    interval_multiplier,
                    phase,
                    ..
                } => GeneralizedSiteProduct::AlongGuides {
                    dimensions: dimensions.iter().copied().map(remap).collect(),
                    interval_multiplier: *interval_multiplier,
                    phase: *phase,
                },
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.family",
                        "generalized definition has an incompatible site mechanism",
                    ));
                }
            };
            let orientation = match source.output_layers.as_slice() {
                [
                    PatternOutputLayer::MarkPrototype {
                        orientation: MarkOrientation::Fixed,
                        ..
                    },
                ] => MarkOrientation::Fixed,
                [
                    PatternOutputLayer::MarkPrototype {
                        orientation: MarkOrientation::GuideTangent { dimension_id },
                        ..
                    },
                ] => MarkOrientation::GuideTangent {
                    dimension_id: remap(*dimension_id),
                },
                [
                    PatternOutputLayer::MarkPrototype {
                        orientation: MarkOrientation::GuideNormal { dimension_id },
                        ..
                    },
                ] => MarkOrientation::GuideNormal {
                    dimension_id: remap(*dimension_id),
                },
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.output_layers",
                        "generalized definition has an incompatible mark prototype",
                    ));
                }
            };
            return Ok(PatternDefinition::generalized_straight_guides(
                id,
                source.name.clone(),
                guide_id,
                site_id,
                output_id,
                remapped.into_iter().map(|(_, value)| value).collect(),
                product,
                orientation,
                source.coverage.clone(),
            ));
        }
        let draft = PatternDefinitionDraft {
            name: source.name.clone(),
            coverage: source.coverage.clone(),
        };
        self.allocate_definition_from_draft(&draft)
    }

    fn retarget_channel(&mut self, channel_id: ChannelId, definition_id: PatternDefinitionId) {
        match &mut self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => {
                channels
                    .iter_mut()
                    .find(|channel| channel.id == channel_id)
                    .expect("validated channel")
                    .pattern_definition_id = definition_id
            }
            ChannelConfiguration::Topology { topology, .. } => {
                topology
                    .channels
                    .iter_mut()
                    .find(|channel| channel.id == channel_id)
                    .expect("validated channel")
                    .pattern_definition_id = definition_id
            }
        }
    }

    fn legacy_channel_mut(&mut self, channel_id: ChannelId) -> Option<&mut ChannelState> {
        match &mut self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => {
                channels.iter_mut().find(|channel| channel.id == channel_id)
            }
            ChannelConfiguration::Topology { .. } => None,
        }
    }

    /// Validates the complete persisted document contract.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_positive_finite(self.canvas.width, "canvas.width")?;
        validate_positive_finite(self.canvas.height, "canvas.height")?;

        let mut definition_ids = HashSet::new();
        let mut mechanism_ids = HashSet::new();
        let mut output_layer_ids = HashSet::new();
        let mut dimension_ids = HashSet::new();
        for definition in &self.pattern_definitions {
            if !definition_ids.insert(definition.id) {
                return Err(ValidationError::new(
                    "pattern_definitions",
                    "document-owned pattern definition IDs must be unique",
                ));
            }
            if definition.name.trim().is_empty() {
                return Err(ValidationError::new(
                    "pattern_definitions.name",
                    "pattern definition name must not be empty",
                ));
            }
            validate_definition(definition)?;
            for mechanism in &definition.mechanisms {
                if !mechanism_ids.insert(mechanism.id()) {
                    return Err(ValidationError::new(
                        "pattern_definitions.mechanisms",
                        "mechanism IDs must be unique document-wide",
                    ));
                }
                if let PatternMechanism::StraightGuideDimensions { dimensions, .. } = mechanism {
                    for dimension in dimensions {
                        if !dimension_ids.insert(dimension.id) {
                            return Err(ValidationError::new(
                                "pattern_definitions.mechanisms.dimensions",
                                "straight-guide dimension IDs must be unique document-wide",
                            ));
                        }
                    }
                }
            }
            for layer in &definition.output_layers {
                if !output_layer_ids.insert(layer.id()) {
                    return Err(ValidationError::new(
                        "pattern_definitions.output_layers",
                        "output layer IDs must be unique document-wide",
                    ));
                }
            }
        }

        match &self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => {
                let mut channel_ids = HashSet::new();
                for channel in channels {
                    if !channel_ids.insert(channel.id) {
                        return Err(ValidationError::new(
                            "channels",
                            "document-owned channel IDs must be unique",
                        ));
                    }
                    let definition = self
                        .pattern_definitions
                        .iter()
                        .find(|definition| definition.id == channel.pattern_definition_id)
                        .ok_or(ValidationError::new(
                            "channel.pattern.definition_id",
                            "channel references a missing pattern definition",
                        ))?;
                    validate_channel(channel, definition)?;
                }
            }
            ChannelConfiguration::Topology { model, topology } => {
                validate_topology(*model, topology, &self.pattern_definitions)?;
            }
        }
        Ok(())
    }

    /// Produces one fully validated document transition without mutating `self`.
    ///
    /// `toniator-engine` is responsible for atomically installing the returned
    /// candidate alongside the corresponding revision advance.
    pub fn apply_command(
        &self,
        command: &DocumentCommand,
    ) -> Result<(Self, CommandResult), ValidationError> {
        command.validate(self)?;
        let mut candidate = self.clone();
        command.apply_to_valid_document(&mut candidate);
        candidate.validate()?;
        let mut result = command.result();
        if let DocumentCommand::ReplaceChannelTopology { topology, .. } = command {
            result.affected_channels = affected_topology_channels(self.channel_ids(), topology);
        }
        if let DocumentCommand::EditSharedPatternDefinition { definition_id, .. } = command {
            result.affected_channels = self.linked_channels(*definition_id);
        }
        Ok((candidate, result))
    }
}

fn next_id(values: impl Iterator<Item = u64>, path: &'static str) -> Result<u64, ValidationError> {
    let maximum = values.max().unwrap_or(0);
    maximum
        .checked_add(1)
        .ok_or_else(|| ValidationError::new(path, "document ID space is exhausted"))
}

fn validate_definition_draft(draft: &PatternDefinitionDraft) -> Result<(), ValidationError> {
    if draft.name.trim().is_empty() {
        return Err(ValidationError::new(
            "pattern_definitions.name",
            "pattern definition name must not be empty",
        ));
    }
    validate_nonnegative_finite(
        draft.coverage.maximum_support_radius,
        "pattern_definitions.coverage.maximum_support_radius",
    )
}

fn apply_definition_edit(definition: &mut PatternDefinition, edit: &PatternDefinitionEdit) {
    match edit {
        PatternDefinitionEdit::SetCoverage { coverage } => definition.coverage = coverage.clone(),
    }
}

fn validate_definition_edit(edit: &PatternDefinitionEdit) -> Result<(), ValidationError> {
    match edit {
        PatternDefinitionEdit::SetCoverage { coverage } => validate_nonnegative_finite(
            coverage.maximum_support_radius,
            "pattern_definitions.coverage.maximum_support_radius",
        ),
    }
}

fn affected_topology_channels(
    old_channel_ids: Vec<ChannelId>,
    new_topology: &ChannelTopology,
) -> Vec<ChannelId> {
    let mut affected = old_channel_ids;
    let introduced: Vec<_> = new_topology
        .channels
        .iter()
        .map(|channel| channel.id)
        .filter(|id| !affected.contains(id))
        .collect();
    affected.extend(introduced);
    affected
}

fn validate_channel(
    channel: &ChannelState,
    definition: &PatternDefinition,
) -> Result<(), ValidationError> {
    validate_layout(&channel.layout)?;
    validate_unit_component(channel.appearance.color.red, "channel.appearance.color.red")?;
    validate_unit_component(
        channel.appearance.color.green,
        "channel.appearance.color.green",
    )?;
    validate_unit_component(
        channel.appearance.color.blue,
        "channel.appearance.color.blue",
    )?;
    validate_unit_component(
        channel.appearance.color.alpha,
        "channel.appearance.color.alpha",
    )?;
    validate_unit_component(channel.appearance.opacity, "channel.appearance.opacity")?;
    validate_mark_response(
        &channel.mark_geometry_response,
        Some(definition.coverage.maximum_support_radius),
    )
}

fn validate_definition(definition: &PatternDefinition) -> Result<(), ValidationError> {
    validate_nonnegative_finite(
        definition.coverage.maximum_support_radius,
        "pattern_definitions.coverage.maximum_support_radius",
    )?;
    let mut mechanism_ids = HashSet::new();
    for mechanism in &definition.mechanisms {
        if !mechanism_ids.insert(mechanism.id()) {
            return Err(ValidationError::new(
                "pattern_definitions.mechanisms",
                "mechanism IDs must be unique and deterministically ordered",
            ));
        }
    }
    let mut output_ids = HashSet::new();
    for layer in &definition.output_layers {
        if !output_ids.insert(layer.id()) {
            return Err(ValidationError::new(
                "pattern_definitions.output_layers",
                "output layer IDs must be unique and deterministically ordered",
            ));
        }
    }
    let PatternFamily::GuideIntersections {
        guide_mechanism_id,
        site_mechanism_id: root_site_id,
    } = definition.family;
    if let [
        PatternMechanism::StraightGuideDimensions { id, dimensions },
        site,
    ] = definition.mechanisms.as_slice()
    {
        if *id != guide_mechanism_id {
            return Err(ValidationError::new(
                "pattern_definitions.family.guide_mechanism_id",
                "family root must reference the ordered straight-guide mechanism",
            ));
        }
        validate_straight_dimensions(dimensions)?;
        validate_site_mechanism(site, *id, root_site_id, dimensions)?;
        validate_generalized_output_layers(&definition.output_layers, root_site_id, dimensions)?;
        return Ok(());
    }
    if !matches!(definition.mechanisms.first(), Some(PatternMechanism::StraightGuides { id }) if *id == guide_mechanism_id)
        || !matches!(definition.mechanisms.get(1), Some(PatternMechanism::GuideIntersections { id, guide_mechanism_id: parent }) if *id == root_site_id && *parent == guide_mechanism_id)
        || definition.mechanisms.len() != 2
    {
        return Err(ValidationError::new(
            "pattern_definitions.family",
            "family root requires ordered straight-guide and intersection mechanisms",
        ));
    }
    if !matches!(definition.output_layers.as_slice(), [PatternOutputLayer::CircularMarks { site_mechanism_id, .. }] if *site_mechanism_id == root_site_id)
    {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "ordered circular-mark output requires the family intersection mechanism",
        ));
    }
    Ok(())
}

/// Validates one standalone typed definition before it is installed in an
/// authoritative document.  Document construction additionally validates
/// document-wide definition/channel references and IDs.
pub fn validate_pattern_definition(definition: &PatternDefinition) -> Result<(), ValidationError> {
    validate_definition(definition)
}

fn validate_straight_dimensions(
    dimensions: &[StraightGuideDimension],
) -> Result<(), ValidationError> {
    if !(1..=4).contains(&dimensions.len()) {
        return Err(ValidationError::new(
            "pattern_definitions.mechanisms.dimensions",
            "straight-guide dimensions must contain one through four entries",
        ));
    }
    let mut ids = HashSet::new();
    for dimension in dimensions {
        if !ids.insert(dimension.id) {
            return Err(ValidationError::new(
                "pattern_definitions.mechanisms.dimensions",
                "straight-guide dimension IDs must be unique in stored order",
            ));
        }
        validate_finite(
            dimension.baseline_angle_degrees,
            "pattern_definitions.mechanisms.dimensions.baseline_angle_degrees",
        )?;
        validate_finite(
            dimension.phase,
            "pattern_definitions.mechanisms.dimensions.phase",
        )?;
        validate_positive_finite(
            dimension.repetition.spacing_multiplier,
            "pattern_definitions.mechanisms.dimensions.repetition.spacing_multiplier",
        )?;
        if dimension.id.0 == 0 {
            return Err(ValidationError::new(
                "pattern_definitions.mechanisms.dimensions.id",
                "straight-guide dimension IDs must be nonzero stable IDs",
            ));
        }
    }
    Ok(())
}

fn validate_selection(
    selection: &[GuideDimensionId],
    dimensions: &[StraightGuideDimension],
    minimum: usize,
    path: &'static str,
) -> Result<(), ValidationError> {
    if selection.len() < minimum {
        return Err(ValidationError::new(
            path,
            "selection has too few dimensions",
        ));
    }
    let expected: Vec<_> = dimensions.iter().map(|dimension| dimension.id).collect();
    let mut previous = None;
    for id in selection {
        let Some(position) = expected.iter().position(|candidate| candidate == id) else {
            return Err(ValidationError::new(
                path,
                "selection references a missing dimension ID",
            ));
        };
        if previous.is_some_and(|value| position <= value) {
            return Err(ValidationError::new(
                path,
                "selection must be unique and follow dimension stored order",
            ));
        }
        previous = Some(position);
    }
    Ok(())
}

fn validate_site_mechanism(
    mechanism: &PatternMechanism,
    guide_id: PatternMechanismId,
    site_id: PatternMechanismId,
    dimensions: &[StraightGuideDimension],
) -> Result<(), ValidationError> {
    match mechanism {
        PatternMechanism::SelectedGuideIntersections {
            id,
            guide_mechanism_id,
            dimensions: selection,
            merge_epsilon,
        } if *id == site_id && *guide_mechanism_id == guide_id => {
            validate_selection(
                selection,
                dimensions,
                2,
                "pattern_definitions.mechanisms.intersections.dimensions",
            )?;
            validate_nonnegative_finite(
                *merge_epsilon,
                "pattern_definitions.mechanisms.intersections.merge_epsilon",
            )
        }
        PatternMechanism::AlongGuideSites {
            id,
            guide_mechanism_id,
            dimensions: selection,
            interval_multiplier,
            phase,
        } if *id == site_id && *guide_mechanism_id == guide_id => {
            validate_selection(
                selection,
                dimensions,
                1,
                "pattern_definitions.mechanisms.along_guides.dimensions",
            )?;
            validate_positive_finite(
                *interval_multiplier,
                "pattern_definitions.mechanisms.along_guides.interval_multiplier",
            )?;
            validate_finite(*phase, "pattern_definitions.mechanisms.along_guides.phase")
        }
        _ => Err(ValidationError::new(
            "pattern_definitions.family",
            "family root requires a compatible declared straight-guide site mechanism",
        )),
    }
}

fn validate_generalized_output_layers(
    layers: &[PatternOutputLayer],
    site_id: PatternMechanismId,
    dimensions: &[StraightGuideDimension],
) -> Result<(), ValidationError> {
    let [
        PatternOutputLayer::MarkPrototype {
            site_mechanism_id,
            prototype: MarkPrototype::Circle,
            orientation,
            ..
        },
    ] = layers
    else {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "generalized straight-guide products require exactly one circle mark prototype layer",
        ));
    };
    if *site_mechanism_id != site_id {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers.site_mechanism_id",
            "output layer must consume its declared site product",
        ));
    }
    match orientation {
        MarkOrientation::Fixed => Ok(()),
        MarkOrientation::GuideTangent { dimension_id }
        | MarkOrientation::GuideNormal { dimension_id }
            if dimensions
                .iter()
                .any(|dimension| dimension.id == *dimension_id) =>
        {
            Ok(())
        }
        _ => Err(ValidationError::new(
            "pattern_definitions.output_layers.orientation",
            "orientation references a missing straight-guide dimension",
        )),
    }
}

fn validate_layout(layout: &ChannelPatternLayout) -> Result<(), ValidationError> {
    validate_positive_finite(
        layout.density.across_x,
        "channel.pattern.layout.density.across_x",
    )?;
    validate_positive_finite(
        layout.density.across_y,
        "channel.pattern.layout.density.across_y",
    )?;
    validate_finite(
        layout.rotation_degrees,
        "channel.pattern.layout.rotation_degrees",
    )?;
    validate_finite(layout.translation_x, "channel.pattern.layout.translation_x")?;
    validate_finite(layout.translation_y, "channel.pattern.layout.translation_y")
}

fn validate_mark_response(
    response: &MarkGeometryResponse,
    maximum_support_radius: Option<f64>,
) -> Result<(), ValidationError> {
    validate_nonnegative_finite(
        response.minimum_size,
        "channel.pattern.mark_geometry_response.minimum_size",
    )?;
    validate_nonnegative_finite(
        response.maximum_size,
        "channel.pattern.mark_geometry_response.maximum_size",
    )?;
    if response.minimum_size > response.maximum_size {
        return Err(ValidationError::new(
            "channel.pattern.mark_geometry_response",
            "minimum_size must not exceed maximum_size",
        ));
    }
    if let Some(maximum_support_radius) = maximum_support_radius
        && response.maximum_size / 2.0 > maximum_support_radius
    {
        return Err(ValidationError::new(
            "channel.pattern.mark_geometry_response.maximum_size",
            "maximum_size exceeds the pattern definition support capability",
        ));
    }
    Ok(())
}

fn validate_topology(
    model: HalftoneChannelModel,
    topology: &ChannelTopology,
    definitions: &[PatternDefinition],
) -> Result<(), ValidationError> {
    let required_roles = model.canonical_roles();
    if topology.channels.len() != required_roles.len() {
        return Err(ValidationError::new(
            "channel_topology.roles",
            "topology must contain every required role exactly once",
        ));
    }
    let mut ids = HashSet::new();
    for (channel, required_role) in topology.channels.iter().zip(required_roles.iter()) {
        if channel.role != *required_role {
            return Err(ValidationError::new(
                "channel_topology.roles",
                "topology roles must use the model's canonical order without extras",
            ));
        }
        if !ids.insert(channel.id) {
            return Err(ValidationError::new(
                "channel_topology.channels",
                "topology channel IDs must be unique",
            ));
        }
        let definition = definitions
            .iter()
            .find(|definition| definition.id == channel.pattern_definition_id)
            .ok_or(ValidationError::new(
                "channel.pattern.definition_id",
                "channel references a missing pattern definition",
            ))?;
        validate_layout(&channel.layout)?;
        validate_mark_response(
            &channel.mark_geometry_response,
            Some(definition.coverage.maximum_support_radius),
        )?;
        validate_unit_component(channel.opacity, "channel.appearance.opacity")?;
        validate_source_mapping(channel.mapping)?;
        validate_paint(model, channel.role, &channel.paint)?;
    }
    Ok(())
}

fn validate_source_mapping(mapping: SourceMapping) -> Result<(), ValidationError> {
    validate_nonnegative_finite(mapping.gain, "channel.source_mapping.gain")?;
    validate_finite(mapping.bias, "channel.source_mapping.bias")
}

fn validate_paint(
    model: HalftoneChannelModel,
    role: HalftoneChannelRole,
    paint: &ChannelPaint,
) -> Result<(), ValidationError> {
    match (model, role, paint) {
        (
            HalftoneChannelModel::SourceColorAlpha,
            HalftoneChannelRole::SourceColor,
            ChannelPaint::SampledSource,
        ) => Ok(()),
        (HalftoneChannelModel::Rgb | HalftoneChannelModel::Cmyk, _, ChannelPaint::Solid(color)) => {
            validate_unit_component(color.red, "channel.paint.solid.red")?;
            validate_unit_component(color.green, "channel.paint.solid.green")?;
            validate_unit_component(color.blue, "channel.paint.solid.blue")?;
            validate_unit_component(color.alpha, "channel.paint.solid.alpha")
        }
        _ => Err(ValidationError::new(
            "channel.paint",
            "paint is incompatible with the channel role and model",
        )),
    }
}

fn validate_finite(value: f64, path: &'static str) -> Result<(), ValidationError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ValidationError::new(path, "value must be finite"))
    }
}

fn validate_positive_finite(value: f64, path: &'static str) -> Result<(), ValidationError> {
    validate_finite(value, path)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(ValidationError::new(
            path,
            "value must be greater than zero",
        ))
    }
}

fn validate_nonnegative_finite(value: f64, path: &'static str) -> Result<(), ValidationError> {
    validate_finite(value, path)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(ValidationError::new(path, "value must not be negative"))
    }
}

fn validate_unit_component(value: f64, path: &'static str) -> Result<(), ValidationError> {
    validate_finite(value, path)?;
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::new(path, "value must be within 0.0..=1.0"))
    }
}

/// A schema-scoped validation failure suitable for a frontend error display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError {
    path: &'static str,
    message: &'static str,
}

impl ValidationError {
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }

    pub const fn path(&self) -> &'static str {
        self.path
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for ValidationError {}

/// The pipeline layer invalidated by a committed document command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidationLevel {
    Presentation,
    Realization,
    Family,
    Source,
    ChannelTopology,
}

/// The committed effect of a document command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandResult {
    pub affected_channels: Vec<ChannelId>,
    pub invalidation: InvalidationLevel,
}

/// A typed, ID-free definition proposal.  Stage 14 intentionally exposes only
/// the accepted mechanism composition; IDs are allocated by the document.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternDefinitionDraft {
    pub name: String,
    pub coverage: CoveragePolicy,
}

/// A typed structural edit. It has no UI/editor state and can be applied only
/// through `DocumentHistory`, which records its exact inverse.
#[derive(Clone, Debug, PartialEq)]
pub enum PatternDefinitionEdit {
    SetCoverage { coverage: CoveragePolicy },
}

/// Supported channel edits in the Stage 2 authoritative command boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum DocumentCommand {
    AddPatternDefinition {
        definition: PatternDefinitionDraft,
    },
    DuplicatePatternDefinition {
        definition_id: PatternDefinitionId,
    },
    RetargetChannelPatternDefinition {
        channel_id: ChannelId,
        definition_id: PatternDefinitionId,
    },
    RemoveUnreferencedPatternDefinition {
        definition_id: PatternDefinitionId,
    },
    /// The ordinary selected-channel structural path.  A stale base is
    /// rejected; shared definitions are cloned and only this channel retargets.
    EditSelectedChannelPatternDefinition {
        channel_id: ChannelId,
        /// Immutable editor base. This is command-only lifecycle input, never
        /// document/persistence state, and detects same-ID in-place edits.
        base_definition: PatternDefinition,
        edit: PatternDefinitionEdit,
    },
    /// The deliberate sharing path.  It keeps definition/internal IDs stable
    /// and reports every linked channel in document order.
    EditSharedPatternDefinition {
        definition_id: PatternDefinitionId,
        /// Immutable editor base; see selected-channel edit above.
        base_definition: PatternDefinition,
        edit: PatternDefinitionEdit,
    },
    SetDensity {
        channel_id: ChannelId,
        density: DensityMetric2D,
    },
    SetRotation {
        channel_id: ChannelId,
        rotation_degrees: f64,
    },
    SetTranslation {
        channel_id: ChannelId,
        translation_x: f64,
        translation_y: f64,
    },
    SetMarkGeometryResponse {
        channel_id: ChannelId,
        response: MarkGeometryResponse,
    },
    SetColor {
        channel_id: ChannelId,
        color: ColorValue,
    },
    SetOpacity {
        channel_id: ChannelId,
        opacity: f64,
    },
    SetVisibility {
        channel_id: ChannelId,
        visible: bool,
    },
    SetSourceReference {
        source: SourceReference,
    },
    SetSourceMapping {
        channel_id: ChannelId,
        mapping: ChannelSourceMapping,
    },
    /// Replaces the complete model and its ordered channel topology together.
    ReplaceChannelTopology {
        model: HalftoneChannelModel,
        topology: ChannelTopology,
    },
    /// Replaces the full Stage 9 mapping for one modeled channel.
    SetTopologySourceMapping {
        channel_id: ChannelId,
        mapping: SourceMapping,
    },
    /// Replaces ordinary solid paint for a modeled channel.
    SetChannelPaint {
        channel_id: ChannelId,
        paint: ChannelPaint,
    },
}

impl DocumentCommand {
    /// Stage 14 definition transitions require the reversible history owner.
    /// The public `DocumentSession` surface retains its pre-Stage-14 command
    /// behavior, while `DocumentHistory` uses its private transition path.
    fn requires_history(&self) -> bool {
        matches!(
            self,
            Self::AddPatternDefinition { .. }
                | Self::DuplicatePatternDefinition { .. }
                | Self::RetargetChannelPatternDefinition { .. }
                | Self::RemoveUnreferencedPatternDefinition { .. }
                | Self::EditSelectedChannelPatternDefinition { .. }
                | Self::EditSharedPatternDefinition { .. }
        )
    }

    fn channel_id(&self) -> ChannelId {
        match self {
            Self::SetDensity { channel_id, .. }
            | Self::SetRotation { channel_id, .. }
            | Self::SetTranslation { channel_id, .. }
            | Self::SetMarkGeometryResponse { channel_id, .. }
            | Self::SetColor { channel_id, .. }
            | Self::SetOpacity { channel_id, .. }
            | Self::SetVisibility { channel_id, .. }
            | Self::SetSourceMapping { channel_id, .. }
            | Self::SetTopologySourceMapping { channel_id, .. }
            | Self::SetChannelPaint { channel_id, .. }
            | Self::RetargetChannelPatternDefinition { channel_id, .. }
            | Self::EditSelectedChannelPatternDefinition { channel_id, .. } => *channel_id,
            Self::SetSourceReference { .. }
            | Self::ReplaceChannelTopology { .. }
            | Self::AddPatternDefinition { .. }
            | Self::DuplicatePatternDefinition { .. }
            | Self::RemoveUnreferencedPatternDefinition { .. }
            | Self::EditSharedPatternDefinition { .. } => ChannelId(0),
        }
    }

    fn validate(&self, document: &Document) -> Result<(), ValidationError> {
        if !matches!(
            self,
            Self::SetSourceReference { .. }
                | Self::ReplaceChannelTopology { .. }
                | Self::AddPatternDefinition { .. }
                | Self::DuplicatePatternDefinition { .. }
                | Self::RemoveUnreferencedPatternDefinition { .. }
                | Self::EditSharedPatternDefinition { .. }
        ) && !document.has_channel(self.channel_id())
        {
            return Err(ValidationError::new(
                "command.channel_id",
                "command targets a missing channel",
            ));
        }

        match self {
            Self::AddPatternDefinition { definition } => {
                document.allocate_definition_from_draft(definition)?;
                Ok(())
            }
            Self::DuplicatePatternDefinition { definition_id } => {
                let source = document
                    .definition(*definition_id)
                    .ok_or(ValidationError::new(
                        "pattern_definitions.id",
                        "definition to duplicate does not exist",
                    ))?;
                document.duplicate_definition(source)?;
                Ok(())
            }
            Self::RetargetChannelPatternDefinition {
                channel_id,
                definition_id,
            } => {
                if document.definition(*definition_id).is_none() {
                    return Err(ValidationError::new(
                        "channel.pattern.definition_id",
                        "channel retargets a missing pattern definition",
                    ));
                }
                if document.pattern_definition_id_for(*channel_id) == Some(*definition_id) {
                    return Err(ValidationError::new(
                        "channel.pattern.definition_id",
                        "definition retarget is a semantic no-op",
                    ));
                }
                Ok(())
            }
            Self::RemoveUnreferencedPatternDefinition { definition_id } => {
                if document.definition(*definition_id).is_none() {
                    return Err(ValidationError::new(
                        "pattern_definitions.id",
                        "definition to remove does not exist",
                    ));
                }
                if !document.linked_channels(*definition_id).is_empty() {
                    return Err(ValidationError::new(
                        "pattern_definitions",
                        "referenced pattern definitions cannot be removed",
                    ));
                }
                Ok(())
            }
            Self::EditSelectedChannelPatternDefinition {
                channel_id,
                base_definition,
                edit,
            } => {
                if document.pattern_definition_id_for(*channel_id) != Some(base_definition.id)
                    || document.definition(base_definition.id) != Some(base_definition)
                {
                    return Err(ValidationError::new(
                        "pattern_definitions.base",
                        "selected-channel definition base is stale",
                    ));
                }
                validate_definition_edit(edit)?;
                let definition = document
                    .definition(base_definition.id)
                    .expect("validated reference");
                let mut edited = definition.clone();
                apply_definition_edit(&mut edited, edit);
                if &edited == definition {
                    return Err(ValidationError::new(
                        "pattern_definitions.edit",
                        "structural edit is a semantic no-op",
                    ));
                }
                validate_definition(&edited)
            }
            Self::EditSharedPatternDefinition {
                definition_id,
                base_definition,
                edit,
            } => {
                if *definition_id != base_definition.id
                    || document.definition(*definition_id) != Some(base_definition)
                {
                    return Err(ValidationError::new(
                        "pattern_definitions.base",
                        "shared definition base is stale",
                    ));
                }
                validate_definition_edit(edit)?;
                let mut edited = document
                    .definition(*definition_id)
                    .expect("validated reference")
                    .clone();
                apply_definition_edit(&mut edited, edit);
                if document.definition(*definition_id) == Some(&edited) {
                    return Err(ValidationError::new(
                        "pattern_definitions.edit",
                        "structural edit is a semantic no-op",
                    ));
                }
                validate_definition(&edited)
            }
            Self::SetDensity { density, .. } => {
                validate_positive_finite(
                    density.across_x,
                    "channel.pattern.layout.density.across_x",
                )?;
                validate_positive_finite(
                    density.across_y,
                    "channel.pattern.layout.density.across_y",
                )
            }
            Self::SetRotation {
                rotation_degrees, ..
            } => validate_finite(*rotation_degrees, "channel.pattern.layout.rotation_degrees"),
            Self::SetTranslation {
                translation_x,
                translation_y,
                ..
            } => {
                validate_finite(*translation_x, "channel.pattern.layout.translation_x")?;
                validate_finite(*translation_y, "channel.pattern.layout.translation_y")
            }
            Self::SetMarkGeometryResponse {
                channel_id,
                response,
            } => {
                validate_nonnegative_finite(
                    response.minimum_size,
                    "channel.pattern.mark_geometry_response.minimum_size",
                )?;
                validate_nonnegative_finite(
                    response.maximum_size,
                    "channel.pattern.mark_geometry_response.maximum_size",
                )?;
                if response.minimum_size > response.maximum_size {
                    return Err(ValidationError::new(
                        "channel.pattern.mark_geometry_response",
                        "minimum_size must not exceed maximum_size",
                    ));
                }
                let pattern_definition_id = document
                    .pattern_definition_id_for(*channel_id)
                    .expect("command channel existence was checked above");
                let definition = document
                    .pattern_definitions
                    .iter()
                    .find(|definition| definition.id == pattern_definition_id)
                    .expect("document validation keeps channel definitions valid");
                if response.maximum_size / 2.0 > definition.coverage.maximum_support_radius {
                    return Err(ValidationError::new(
                        "channel.pattern.mark_geometry_response.maximum_size",
                        "maximum_size exceeds the pattern definition support capability",
                    ));
                }
                Ok(())
            }
            Self::SetColor { channel_id, color } => {
                validate_unit_component(color.red, "channel.appearance.color.red")?;
                validate_unit_component(color.green, "channel.appearance.color.green")?;
                validate_unit_component(color.blue, "channel.appearance.color.blue")?;
                validate_unit_component(color.alpha, "channel.appearance.color.alpha")?;
                if let Some(topology) = document.channel_topology() {
                    let channel = topology
                        .channels
                        .iter()
                        .find(|channel| channel.id == *channel_id)
                        .expect("modeled topology and document channel IDs validate together");
                    if !matches!(channel.paint, ChannelPaint::Solid(_)) {
                        return Err(ValidationError::new(
                            "channel.paint",
                            "sampled-source paint cannot be replaced by an ordinary solid color",
                        ));
                    }
                }
                Ok(())
            }
            Self::SetOpacity { opacity, .. } => {
                validate_unit_component(*opacity, "channel.appearance.opacity")
            }
            Self::SetVisibility { .. } => Ok(()),
            Self::SetSourceReference { source } => match source {
                SourceReference::Unassigned => Ok(()),
                SourceReference::Assigned(id) if id.as_str().trim().is_empty() => {
                    Err(ValidationError::new(
                        "source.reference_id",
                        "source reference ID must not be empty",
                    ))
                }
                SourceReference::Assigned(_) => Ok(()),
            },
            Self::SetSourceMapping { .. } if document.channel_topology().is_some() => {
                Err(ValidationError::new(
                    "channel.source_mapping",
                    "modeled channels require a complete Stage 9 source mapping",
                ))
            }
            Self::SetSourceMapping { .. } => Ok(()),
            Self::ReplaceChannelTopology { model, topology } => {
                validate_topology(*model, topology, &document.pattern_definitions)
            }
            Self::SetTopologySourceMapping {
                channel_id,
                mapping,
            } => {
                let topology = document.channel_topology().ok_or(ValidationError::new(
                    "channel_topology",
                    "complete source mappings require an installed channel topology",
                ))?;
                if !topology
                    .channels
                    .iter()
                    .any(|channel| channel.id == *channel_id)
                {
                    return Err(ValidationError::new(
                        "command.channel_id",
                        "command targets a missing modeled channel",
                    ));
                }
                validate_source_mapping(*mapping)
            }
            Self::SetChannelPaint { channel_id, paint } => {
                let model = document.channel_model().ok_or(ValidationError::new(
                    "channel_topology",
                    "paint changes require an installed channel topology",
                ))?;
                let topology = document
                    .channel_topology()
                    .expect("model and topology validate together");
                let channel = topology
                    .channels
                    .iter()
                    .find(|channel| channel.id == *channel_id)
                    .ok_or(ValidationError::new(
                        "command.channel_id",
                        "command targets a missing modeled channel",
                    ))?;
                validate_paint(model, channel.role, paint)
            }
        }
    }

    fn apply_to_valid_document(&self, document: &mut Document) {
        match self {
            Self::AddPatternDefinition { definition } => {
                let definition = document
                    .allocate_definition_from_draft(definition)
                    .expect("command validation allocated a definition");
                document.pattern_definitions.push(definition);
                return;
            }
            Self::DuplicatePatternDefinition { definition_id } => {
                let source = document
                    .definition(*definition_id)
                    .expect("validated definition")
                    .clone();
                let definition = document
                    .duplicate_definition(&source)
                    .expect("validated duplicate allocation");
                document.pattern_definitions.push(definition);
                return;
            }
            Self::RetargetChannelPatternDefinition {
                channel_id,
                definition_id,
            } => {
                document.retarget_channel(*channel_id, *definition_id);
                return;
            }
            Self::RemoveUnreferencedPatternDefinition { definition_id } => {
                document
                    .pattern_definitions
                    .retain(|definition| definition.id != *definition_id);
                return;
            }
            Self::EditSelectedChannelPatternDefinition {
                channel_id,
                base_definition,
                edit,
            } => {
                if document.linked_channels(base_definition.id).len() > 1 {
                    let source = document
                        .definition(base_definition.id)
                        .expect("validated definition")
                        .clone();
                    let mut clone = document
                        .duplicate_definition(&source)
                        .expect("validated clone allocation");
                    apply_definition_edit(&mut clone, edit);
                    let clone_id = clone.id;
                    document.pattern_definitions.push(clone);
                    document.retarget_channel(*channel_id, clone_id);
                } else {
                    let definition = document
                        .pattern_definitions
                        .iter_mut()
                        .find(|definition| definition.id == base_definition.id)
                        .expect("validated definition");
                    apply_definition_edit(definition, edit);
                }
                return;
            }
            Self::EditSharedPatternDefinition {
                definition_id,
                edit,
                ..
            } => {
                let definition = document
                    .pattern_definitions
                    .iter_mut()
                    .find(|definition| definition.id == *definition_id)
                    .expect("validated definition");
                apply_definition_edit(definition, edit);
                return;
            }
            _ => {}
        }
        if let Self::SetSourceReference { source } = self {
            document.source = source.clone();
            return;
        }
        if let Self::ReplaceChannelTopology { model, topology } = self {
            document.channel_configuration = ChannelConfiguration::Topology {
                model: *model,
                topology: topology.clone(),
            };
            return;
        }
        match &mut document.channel_configuration {
            ChannelConfiguration::Legacy(_) => {
                let channel = document
                    .legacy_channel_mut(self.channel_id())
                    .expect("validated command must target an existing legacy channel");
                match self {
                    Self::SetDensity { density, .. } => channel.layout.density = density.clone(),
                    Self::SetRotation {
                        rotation_degrees, ..
                    } => channel.layout.rotation_degrees = *rotation_degrees,
                    Self::SetTranslation {
                        translation_x,
                        translation_y,
                        ..
                    } => {
                        channel.layout.translation_x = *translation_x;
                        channel.layout.translation_y = *translation_y;
                    }
                    Self::SetMarkGeometryResponse { response, .. } => {
                        channel.mark_geometry_response = response.clone()
                    }
                    Self::SetColor { color, .. } => channel.appearance.color = color.clone(),
                    Self::SetOpacity { opacity, .. } => channel.appearance.opacity = *opacity,
                    Self::SetVisibility { visible, .. } => channel.appearance.visible = *visible,
                    Self::SetSourceMapping { mapping, .. } => channel.source_mapping = *mapping,
                    _ => unreachable!("modeled-only command was validated against legacy state"),
                }
            }
            ChannelConfiguration::Topology { topology, .. } => {
                let channel = topology
                    .channels
                    .iter_mut()
                    .find(|channel| channel.id == self.channel_id())
                    .expect("validated command must target an existing modeled channel");
                match self {
                    Self::SetDensity { density, .. } => channel.layout.density = density.clone(),
                    Self::SetRotation {
                        rotation_degrees, ..
                    } => channel.layout.rotation_degrees = *rotation_degrees,
                    Self::SetTranslation {
                        translation_x,
                        translation_y,
                        ..
                    } => {
                        channel.layout.translation_x = *translation_x;
                        channel.layout.translation_y = *translation_y;
                    }
                    Self::SetMarkGeometryResponse { response, .. } => {
                        channel.mark_geometry_response = response.clone()
                    }
                    Self::SetColor { color, .. } => {
                        channel.paint = ChannelPaint::Solid(color.clone())
                    }
                    Self::SetOpacity { opacity, .. } => channel.opacity = *opacity,
                    Self::SetVisibility { visible, .. } => channel.visible = *visible,
                    Self::SetTopologySourceMapping { mapping, .. } => channel.mapping = *mapping,
                    Self::SetChannelPaint { paint, .. } => channel.paint = paint.clone(),
                    Self::SetSourceMapping { .. } => {
                        unreachable!("legacy mapping is rejected for modeled state")
                    }
                    _ => unreachable!("handled before configuration mutation"),
                }
            }
        }
    }

    fn result(&self) -> CommandResult {
        let invalidation = match self {
            Self::AddPatternDefinition { .. }
            | Self::DuplicatePatternDefinition { .. }
            | Self::RemoveUnreferencedPatternDefinition { .. } => InvalidationLevel::Family,
            Self::RetargetChannelPatternDefinition { .. }
            | Self::EditSelectedChannelPatternDefinition { .. }
            | Self::EditSharedPatternDefinition { .. } => InvalidationLevel::Family,
            Self::SetDensity { .. } | Self::SetRotation { .. } | Self::SetTranslation { .. } => {
                InvalidationLevel::Family
            }
            Self::SetMarkGeometryResponse { .. } => InvalidationLevel::Realization,
            Self::SetSourceMapping { .. } | Self::SetTopologySourceMapping { .. } => {
                InvalidationLevel::Realization
            }
            Self::SetSourceReference { .. } => InvalidationLevel::Source,
            Self::SetColor { .. }
            | Self::SetOpacity { .. }
            | Self::SetVisibility { .. }
            | Self::SetChannelPaint { .. } => InvalidationLevel::Presentation,
            Self::ReplaceChannelTopology { .. } => InvalidationLevel::ChannelTopology,
        };
        CommandResult {
            affected_channels: match self {
                Self::AddPatternDefinition { .. }
                | Self::DuplicatePatternDefinition { .. }
                | Self::RemoveUnreferencedPatternDefinition { .. } => Vec::new(),
                Self::RetargetChannelPatternDefinition { channel_id, .. }
                | Self::EditSelectedChannelPatternDefinition { channel_id, .. } => {
                    vec![*channel_id]
                }
                Self::EditSharedPatternDefinition { .. } => Vec::new(),
                Self::SetSourceReference { .. } => self.channels_for_source_change(),
                Self::ReplaceChannelTopology { topology, .. } => {
                    topology.channels.iter().map(|channel| channel.id).collect()
                }
                _ => vec![self.channel_id()],
            },
            invalidation,
        }
    }
}

impl DocumentCommand {
    fn channels_for_source_change(&self) -> Vec<ChannelId> {
        // Source invalidation applies to the complete document; callers retain
        // the immutable snapshot to discover the current channel list.
        Vec::new()
    }
}

/// An immutable revision/channel token. Its fields are private so callers can
/// observe, but not manufacture, a different revision/channel pairing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationToken {
    revision: Revision,
    channel_id: ChannelId,
}

impl EvaluationToken {
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn channel_id(&self) -> ChannelId {
        self.channel_id
    }
}

/// One atomic read of the authority for a requested channel. A snapshot owns
/// both the document clone and its exact token and has no public constructor.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationSnapshot {
    document: Document,
    token: EvaluationToken,
}

impl EvaluationSnapshot {
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn token(&self) -> EvaluationToken {
        self.token
    }
}

/// An immutable revision token for complete-document evaluation. Its fields
/// are private so only a `DocumentSession` can mint a document/revision pair.
/// The token remains cheaply carryable by evaluators and schedulers.
///
/// ```compile_fail
/// use toniator_domain::{DocumentEvaluationToken, DocumentId, Revision};
///
/// let _forged = DocumentEvaluationToken {
///     document_id: DocumentId(1),
///     revision: Revision(0),
/// };
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentEvaluationToken {
    document_id: DocumentId,
    revision: Revision,
}

impl DocumentEvaluationToken {
    pub fn document_id(&self) -> DocumentId {
        self.document_id
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }
}

/// One atomic read of the complete document authority. A snapshot owns both
/// the document clone and its exact document-level token and has no public
/// constructor.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentEvaluationSnapshot {
    document: Document,
    token: DocumentEvaluationToken,
}

impl DocumentEvaluationSnapshot {
    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn token(&self) -> DocumentEvaluationToken {
        self.token
    }
}

/// The exclusive owner of mutable authoritative document state.
#[derive(Clone, Debug)]
pub struct DocumentSession {
    document: Document,
    revision: Revision,
}

impl DocumentSession {
    pub fn new(document: Document) -> Result<Self, ValidationError> {
        document.validate()?;
        Ok(Self {
            document,
            revision: Revision(0),
        })
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn snapshot(&self) -> Document {
        self.document.clone()
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn apply(
        &mut self,
        command: &DocumentCommand,
    ) -> Result<CommandResult, DocumentSessionError> {
        if command.requires_history() {
            return Err(DocumentSessionError::HistoryRequired);
        }
        self.apply_authoritative(command)
    }

    /// Applies a definition or legacy command through the only caller that
    /// records reversible definition transitions.
    fn apply_from_history(
        &mut self,
        command: &DocumentCommand,
    ) -> Result<CommandResult, DocumentSessionError> {
        self.apply_authoritative(command)
    }

    fn apply_authoritative(
        &mut self,
        command: &DocumentCommand,
    ) -> Result<CommandResult, DocumentSessionError> {
        let next_revision = self.next_revision()?;
        let (candidate, mut result) = self.document.apply_command(command)?;
        if matches!(command, DocumentCommand::SetSourceReference { .. }) {
            result.affected_channels = candidate.channel_ids();
        }
        self.document = candidate;
        self.revision = next_revision;
        Ok(result)
    }

    /// Installs an already-validated authoritative snapshot while advancing the
    /// session revision. This is deliberately private: `DocumentHistory` is
    /// the only caller and records the snapshots from successful session
    /// transitions itself.
    fn restore_history_snapshot(&mut self, document: Document) -> Result<(), DocumentSessionError> {
        let next_revision = self.next_revision()?;
        self.document = document;
        self.revision = next_revision;
        Ok(())
    }

    fn next_revision(&self) -> Result<Revision, DocumentSessionError> {
        self.revision
            .0
            .checked_add(1)
            .map(Revision)
            .ok_or(DocumentSessionError::RevisionExhausted)
    }
    pub fn evaluation_snapshot(
        &self,
        channel_id: ChannelId,
    ) -> Result<EvaluationSnapshot, ValidationError> {
        if self.document.channel_model().is_some() {
            return Err(ValidationError::new(
                "evaluation.channel_topology",
                "modeled topology evaluation is not available before Stage 9D",
            ));
        }
        if self.document.channel(channel_id).is_none() {
            return Err(ValidationError::new(
                "evaluation.channel_id",
                "evaluation targets a missing channel",
            ));
        }
        Ok(EvaluationSnapshot {
            document: self.document.clone(),
            token: EvaluationToken {
                revision: self.revision,
                channel_id,
            },
        })
    }
    pub fn evaluation_token(
        &self,
        channel_id: ChannelId,
    ) -> Result<EvaluationToken, ValidationError> {
        Ok(self.evaluation_snapshot(channel_id)?.token())
    }

    /// Atomically captures the complete authoritative document with its
    /// session-minted revision token. Unlike the retained channel diagnostic
    /// snapshot, this supports both legacy and modeled document topologies.
    pub fn document_evaluation_snapshot(&self) -> DocumentEvaluationSnapshot {
        DocumentEvaluationSnapshot {
            document: self.document.clone(),
            token: self.document_evaluation_token(),
        }
    }

    pub fn document_evaluation_token(&self) -> DocumentEvaluationToken {
        DocumentEvaluationToken {
            document_id: self.document.id,
            revision: self.revision,
        }
    }

    pub fn accepts_evaluation(&self, token: EvaluationToken) -> bool {
        token.revision == self.revision
    }

    /// Returns whether `token` was minted by this document session for its
    /// still-current document revision.
    pub fn accepts_document_evaluation(&self, token: DocumentEvaluationToken) -> bool {
        token.document_id == self.document.id && token.revision == self.revision
    }
}

/// Session-lifetime reversible authoritative document transitions.
///
/// The history owns no source bytes, evaluator state, caches, or UI state.
/// Entries retain complete validated document snapshots so future document
/// fields participate automatically without history-specific reconstruction.
#[derive(Debug)]
pub struct DocumentHistory {
    session: DocumentSession,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    before: Document,
    after: Document,
    result: CommandResult,
}

impl DocumentHistory {
    pub fn new(session: DocumentSession) -> Self {
        Self {
            session,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn session(&self) -> &DocumentSession {
        &self.session
    }

    pub fn document(&self) -> &Document {
        self.session.document()
    }

    pub fn revision(&self) -> Revision {
        self.session.revision()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn apply(
        &mut self,
        command: &DocumentCommand,
    ) -> Result<CommandResult, DocumentSessionError> {
        let before = self.session.snapshot();
        let result = self.session.apply_from_history(command)?;
        let after = self.session.snapshot();
        self.undo.push(HistoryEntry {
            before,
            after,
            result: result.clone(),
        });
        self.redo.clear();
        Ok(result)
    }

    pub fn undo(&mut self) -> Result<Option<CommandResult>, DocumentSessionError> {
        let Some(entry) = self.undo.last() else {
            return Ok(None);
        };
        self.session
            .restore_history_snapshot(entry.before.clone())?;
        let entry = self
            .undo
            .pop()
            .expect("history entry remains present after successful restoration");
        let result = entry.result.clone();
        self.redo.push(entry);
        Ok(Some(result))
    }

    pub fn redo(&mut self) -> Result<Option<CommandResult>, DocumentSessionError> {
        let Some(entry) = self.redo.last() else {
            return Ok(None);
        };
        self.session.restore_history_snapshot(entry.after.clone())?;
        let entry = self
            .redo
            .pop()
            .expect("history entry remains present after successful restoration");
        let result = entry.result.clone();
        self.undo.push(entry);
        Ok(Some(result))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentSessionError {
    Validation(ValidationError),
    RevisionExhausted,
    /// Stage 14 definition commands must retain their inverse in
    /// `DocumentHistory`; public session application is intentionally refused.
    HistoryRequired,
}
impl From<ValidationError> for DocumentSessionError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}
impl fmt::Display for DocumentSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(f),
            Self::RevisionExhausted => f.write_str("document revision is exhausted"),
            Self::HistoryRequired => {
                f.write_str("pattern definition commands require document history")
            }
        }
    }
}
impl Error for DocumentSessionError {}

#[cfg(test)]
mod history_tests {
    use super::*;

    fn history() -> DocumentHistory {
        let definition = PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "grid",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps: 0,
                maximum_support_radius: 5.0,
            },
        );
        let document = Document::new(
            DocumentId(1),
            CanvasSpec {
                width: 10.0,
                height: 10.0,
            },
            vec![definition],
            vec![ChannelState {
                id: ChannelId(1),
                pattern_definition_id: PatternDefinitionId(1),
                layout: ChannelPatternLayout {
                    density: DensityMetric2D {
                        across_x: 1.0,
                        across_y: 1.0,
                        aspect_locked: true,
                    },
                    rotation_degrees: 0.0,
                    translation_x: 0.0,
                    translation_y: 0.0,
                },
                appearance: ChannelAppearance {
                    visible: true,
                    color: ColorValue {
                        red: 0.0,
                        green: 0.0,
                        blue: 0.0,
                        alpha: 1.0,
                    },
                    opacity: 1.0,
                },
                mark_geometry_response: MarkGeometryResponse {
                    minimum_size: 0.0,
                    maximum_size: 10.0,
                },
                source_mapping: ChannelSourceMapping {
                    component: SourceComponent::Luminance,
                    placement: SourcePlacement::StretchToCanvas,
                },
            }],
        )
        .unwrap();
        DocumentHistory::new(DocumentSession::new(document).unwrap())
    }

    #[test]
    fn history_revision_exhaustion_keeps_document_and_stacks_atomic() {
        let mut history = history();
        history
            .apply(&DocumentCommand::SetVisibility {
                channel_id: ChannelId(1),
                visible: false,
            })
            .unwrap();
        let before_document = history.document().clone();
        history.session.revision = Revision(u64::MAX);

        assert_eq!(history.undo(), Err(DocumentSessionError::RevisionExhausted));
        assert_eq!(history.document(), &before_document);
        assert_eq!(history.revision(), Revision(u64::MAX));
        assert!(history.can_undo());
        assert!(!history.can_redo());

        assert_eq!(
            history.apply(&DocumentCommand::SetVisibility {
                channel_id: ChannelId(1),
                visible: true,
            }),
            Err(DocumentSessionError::RevisionExhausted)
        );
        assert_eq!(history.document(), &before_document);
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }
}
