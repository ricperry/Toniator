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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorComponent {
    Red,
    Green,
    Blue,
    Alpha,
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarkGeometryFieldEdit {
    MinimumSize(f64),
    MaximumSize(f64),
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyMappingFieldEdit {
    Component(SourceComponent),
    Placement(SourcePlacement),
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModeledMappingFieldEdit {
    Component(SourceMappingComponent),
    Placement(SourcePlacement),
    Inverted(bool),
    Gain(f64),
    Bias(f64),
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
    /// A deterministic, site-only family.  Its ordered mechanisms keep the
    /// base process, density field, collision policy, and published site
    /// product independently addressable without inventing named patterns.
    RandomSites {
        base_site_process_id: PatternMechanismId,
        density_modulation_id: PatternMechanismId,
        exclusion_id: PatternMechanismId,
        site_product_id: PatternMechanismId,
    },
}

/// The distinct structural character of an unmodulated candidate process.
#[derive(Clone, Debug, PartialEq)]
pub enum RandomSiteCharacter {
    /// Independent uniform candidates; close neighbors are permitted unless
    /// the separately declared exclusion mechanism rejects them.
    RawUniform,
    /// Sequential dart throwing with a declared center-distance construction.
    /// This is deliberately not an alias named "BlueNoise" or "Poisson".
    Even { minimum_center_distance: f64 },
    /// Parent-centered Gaussian islands mixed with a uniform background.
    Clustered {
        cluster_density: f64,
        cluster_spread: f64,
        cluster_strength: f64,
    },
}

/// Density enters site placement before collision acceptance.  Artwork
/// weighting is an explicit structural dependency, not mark modulation.
#[derive(Clone, Debug, PartialEq)]
pub enum SiteDensityModulation {
    Uniform,
    ArtworkWeighted {
        mapping: SourceMapping,
        strength: f64,
        response: ArtworkWeightResponse,
    },
}

/// Typed response applied at the decoder-owned artwork density field boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtworkWeightResponse {
    Linear,
    /// A fixed IEEE-arithmetic curve: `x * x * (3 - 2 * x)` for clamped x.
    Smoothstep,
}

/// Size policy used by a visible-mark exclusion constraint.  The current
/// maximum-support policy is conservative: every realized circle is bounded
/// by the family support radius, so the stated separation survives realization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisibleMarkSizingPolicy {
    MaximumSupportRadius,
}

/// Collision behavior remains independent from candidate character and
/// density modulation.
#[derive(Clone, Debug, PartialEq)]
pub enum SiteExclusionPolicy {
    None,
    MinimumCenterDistance {
        minimum: f64,
    },
    VisibleMarkMargin {
        margin: f64,
        sizing: VisibleMarkSizingPolicy,
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
    RandomSiteProcess {
        id: PatternMechanismId,
        character: RandomSiteCharacter,
        seed: u32,
    },
    SiteDensityModulation {
        id: PatternMechanismId,
        base_site_process_id: PatternMechanismId,
        modulation: SiteDensityModulation,
    },
    SiteExclusion {
        id: PatternMechanismId,
        density_modulation_id: PatternMechanismId,
        policy: SiteExclusionPolicy,
    },
    /// The published, reproducible site collection. Work limits are
    /// definition-owned: attempts bound candidate generation and neighbor
    /// checks bound deterministic spatial-index exclusion work.
    RandomSiteProduct {
        id: PatternMechanismId,
        exclusion_id: PatternMechanismId,
        maximum_attempts: u32,
        maximum_neighbor_checks: u32,
    },
}

impl PatternMechanism {
    pub const fn id(&self) -> PatternMechanismId {
        match self {
            Self::StraightGuides { id }
            | Self::GuideIntersections { id, .. }
            | Self::StraightGuideDimensions { id, .. }
            | Self::SelectedGuideIntersections { id, .. }
            | Self::AlongGuideSites { id, .. }
            | Self::RandomSiteProcess { id, .. }
            | Self::SiteDensityModulation { id, .. }
            | Self::SiteExclusion { id, .. }
            | Self::RandomSiteProduct { id, .. } => *id,
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

    /// Constructs the bounded Stage 16B random-site mechanism chain.  The
    /// caller supplies document-wide IDs explicitly; document validation owns
    /// collision and order checks.
    #[allow(clippy::too_many_arguments)]
    pub fn random_sites(
        id: PatternDefinitionId,
        name: impl Into<String>,
        base_id: PatternMechanismId,
        modulation_id: PatternMechanismId,
        exclusion_id: PatternMechanismId,
        site_id: PatternMechanismId,
        output_id: PatternOutputLayerId,
        character: RandomSiteCharacter,
        seed: u32,
        density_modulation: SiteDensityModulation,
        exclusion: SiteExclusionPolicy,
        maximum_attempts: u32,
        maximum_neighbor_checks: u32,
        coverage: CoveragePolicy,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            family: PatternFamily::RandomSites {
                base_site_process_id: base_id,
                density_modulation_id: modulation_id,
                exclusion_id,
                site_product_id: site_id,
            },
            mechanisms: vec![
                PatternMechanism::RandomSiteProcess {
                    id: base_id,
                    character,
                    seed,
                },
                PatternMechanism::SiteDensityModulation {
                    id: modulation_id,
                    base_site_process_id: base_id,
                    modulation: density_modulation,
                },
                PatternMechanism::SiteExclusion {
                    id: exclusion_id,
                    density_modulation_id: modulation_id,
                    policy: exclusion,
                },
                PatternMechanism::RandomSiteProduct {
                    id: site_id,
                    exclusion_id,
                    maximum_attempts,
                    maximum_neighbor_checks,
                },
            ],
            output_layers: vec![PatternOutputLayer::MarkPrototype {
                id: output_id,
                site_mechanism_id: site_id,
                prototype: MarkPrototype::Circle,
                orientation: MarkOrientation::Fixed,
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

    /// Returns deterministic schema-derived descriptors from the exhaustive
    /// field contract. Only the three state-dependent capability predicates
    /// supply runtime context; no static metadata is repeated here.
    pub fn property_descriptors(&self) -> Vec<PropertyDescriptor> {
        let mut descriptors = vec![descriptor_from_contract(
            PropertyFieldId::SourceReference,
            PropertyTarget::Document,
        )];
        for channel_id in self.channel_ids() {
            let target = PropertyTarget::Channel(channel_id);
            for field in [
                PropertyFieldId::DensityAcrossX,
                PropertyFieldId::DensityAcrossY,
                PropertyFieldId::DensityAspectLocked,
                PropertyFieldId::RotationDegrees,
                PropertyFieldId::TranslationX,
                PropertyFieldId::TranslationY,
                PropertyFieldId::MarkMinimumSize,
                PropertyFieldId::MarkMaximumSize,
                PropertyFieldId::Opacity,
                PropertyFieldId::Visibility,
                PropertyFieldId::DefinitionSelection,
            ] {
                descriptors.push(descriptor_from_contract(field, target));
            }
            if let Some(model) = self.channel_model() {
                for field in [
                    PropertyFieldId::ModeledMappingComponent,
                    PropertyFieldId::ModeledMappingPlacement,
                    PropertyFieldId::ModeledMappingInverted,
                    PropertyFieldId::ModeledMappingGain,
                    PropertyFieldId::ModeledMappingBias,
                ] {
                    descriptors.push(descriptor_from_contract(field, target));
                }
                let context = if model == HalftoneChannelModel::SourceColorAlpha {
                    DescriptorRuntimeContext::Paint {
                        choices: SAMPLED_PAINT_CHOICES,
                        dependency: PropertyDependency::SampledPaint,
                    }
                } else {
                    DescriptorRuntimeContext::Paint {
                        choices: SOLID_PAINT_CHOICES,
                        dependency: PropertyDependency::SolidPaint,
                    }
                };
                descriptors.push(descriptor_with_runtime_context(
                    PropertyFieldId::Paint,
                    target,
                    context,
                ));
                if model != HalftoneChannelModel::SourceColorAlpha {
                    for field in [
                        PropertyFieldId::ColorRed,
                        PropertyFieldId::ColorGreen,
                        PropertyFieldId::ColorBlue,
                        PropertyFieldId::ColorAlpha,
                    ] {
                        descriptors.push(descriptor_from_contract(field, target));
                    }
                }
            } else {
                for field in [
                    PropertyFieldId::LegacyMappingComponent,
                    PropertyFieldId::LegacyMappingPlacement,
                    PropertyFieldId::ColorRed,
                    PropertyFieldId::ColorGreen,
                    PropertyFieldId::ColorBlue,
                    PropertyFieldId::ColorAlpha,
                ] {
                    descriptors.push(descriptor_from_contract(field, target));
                }
            }
        }
        for definition in &self.pattern_definitions {
            let definition_target = PropertyTarget::Definition(definition.id);
            for field in [
                PropertyFieldId::CoverageGuardSteps,
                PropertyFieldId::CoverageMaximumSupportRadius,
            ] {
                descriptors.push(descriptor_from_contract(field, definition_target));
            }
            for mechanism in &definition.mechanisms {
                let target = PropertyTarget::Mechanism(definition.id, mechanism.id());
                match mechanism {
                    PatternMechanism::StraightGuideDimensions { dimensions, .. } => {
                        for dimension in dimensions {
                            let target = PropertyTarget::GuideDimension(
                                definition.id,
                                mechanism.id(),
                                dimension.id,
                            );
                            for field in [
                                PropertyFieldId::GuideBaselineAngle,
                                PropertyFieldId::GuidePhase,
                                PropertyFieldId::GuideSpacingMultiplier,
                            ] {
                                descriptors.push(descriptor_from_contract(field, target));
                            }
                        }
                    }
                    PatternMechanism::SelectedGuideIntersections { .. } => {
                        for field in [
                            PropertyFieldId::IntersectionDimensions,
                            PropertyFieldId::IntersectionMergeEpsilon,
                        ] {
                            descriptors.push(descriptor_from_contract(field, target));
                        }
                    }
                    PatternMechanism::AlongGuideSites { .. } => {
                        for field in [
                            PropertyFieldId::AlongGuideDimensions,
                            PropertyFieldId::AlongGuideIntervalMultiplier,
                            PropertyFieldId::AlongGuidePhase,
                        ] {
                            descriptors.push(descriptor_from_contract(field, target));
                        }
                    }
                    PatternMechanism::RandomSiteProcess { character, .. } => {
                        for field in [
                            PropertyFieldId::RandomCharacter,
                            PropertyFieldId::RandomSeed,
                        ] {
                            descriptors.push(descriptor_from_contract(field, target));
                        }
                        match character {
                            RandomSiteCharacter::RawUniform => {}
                            RandomSiteCharacter::Even { .. } => {
                                descriptors.push(descriptor_from_contract(
                                    PropertyFieldId::RandomEvenMinimumCenterDistance,
                                    target,
                                ))
                            }
                            RandomSiteCharacter::Clustered { .. } => {
                                for field in [
                                    PropertyFieldId::RandomClusterDensity,
                                    PropertyFieldId::RandomClusterSpread,
                                    PropertyFieldId::RandomClusterStrength,
                                ] {
                                    descriptors.push(descriptor_from_contract(field, target));
                                }
                            }
                        }
                    }
                    PatternMechanism::SiteDensityModulation { modulation, .. } => {
                        descriptors.push(descriptor_with_runtime_context(
                            PropertyFieldId::RandomDensityModulation,
                            target,
                            DescriptorRuntimeContext::DensityModulation {
                                dependency: if matches!(
                                    modulation,
                                    SiteDensityModulation::ArtworkWeighted { .. }
                                ) {
                                    PropertyDependency::ArtworkWeightedDensity
                                } else {
                                    PropertyDependency::RandomProcess
                                },
                            },
                        ));
                        if matches!(modulation, SiteDensityModulation::ArtworkWeighted { .. }) {
                            for field in [
                                PropertyFieldId::ArtworkWeightMappingComponent,
                                PropertyFieldId::ArtworkWeightMappingPlacement,
                                PropertyFieldId::ArtworkWeightMappingInverted,
                                PropertyFieldId::ArtworkWeightMappingGain,
                                PropertyFieldId::ArtworkWeightMappingBias,
                                PropertyFieldId::ArtworkWeightStrength,
                                PropertyFieldId::ArtworkWeightResponse,
                            ] {
                                descriptors.push(descriptor_from_contract(field, target));
                            }
                        }
                    }
                    PatternMechanism::SiteExclusion { policy, .. } => {
                        let visible =
                            matches!(policy, SiteExclusionPolicy::VisibleMarkMargin { .. });
                        descriptors.push(descriptor_with_runtime_context(
                            PropertyFieldId::RandomExclusion,
                            target,
                            DescriptorRuntimeContext::Exclusion {
                                dependency: if visible { PropertyDependency::VisibleMarkExclusion } else { PropertyDependency::RandomProcess },
                                support: if visible { StructuralSupportConstraint::VisibleMarkMarginUsesMaximumSupportRadius } else { StructuralSupportConstraint::None },
                            },
                        ));
                        match policy {
                            SiteExclusionPolicy::None => {}
                            SiteExclusionPolicy::MinimumCenterDistance { .. } => {
                                descriptors.push(descriptor_from_contract(
                                    PropertyFieldId::ExclusionMinimumCenterDistance,
                                    target,
                                ))
                            }
                            SiteExclusionPolicy::VisibleMarkMargin { .. } => {
                                for field in [
                                    PropertyFieldId::VisibleMarkMargin,
                                    PropertyFieldId::VisibleMarkSizingPolicy,
                                ] {
                                    descriptors.push(descriptor_from_contract(field, target));
                                }
                            }
                        }
                    }
                    PatternMechanism::RandomSiteProduct { .. } => {
                        for field in [
                            PropertyFieldId::RandomMaximumAttempts,
                            PropertyFieldId::RandomMaximumNeighborChecks,
                        ] {
                            descriptors.push(descriptor_from_contract(field, target));
                        }
                    }
                    PatternMechanism::StraightGuides { .. }
                    | PatternMechanism::GuideIntersections { .. } => {}
                }
            }
            for layer in &definition.output_layers {
                let target = PropertyTarget::OutputLayer(definition.id, layer.id());
                descriptors.push(descriptor_from_contract(
                    PropertyFieldId::OutputSiteProduct,
                    target,
                ));
                if let PatternOutputLayer::MarkPrototype { orientation, .. } = layer {
                    for field in [
                        PropertyFieldId::OutputPrototype,
                        PropertyFieldId::OutputOrientation,
                    ] {
                        descriptors.push(descriptor_from_contract(field, target));
                    }
                    if matches!(
                        orientation,
                        MarkOrientation::GuideTangent { .. } | MarkOrientation::GuideNormal { .. }
                    ) {
                        descriptors.push(descriptor_from_contract(
                            PropertyFieldId::OutputOrientationDimension,
                            target,
                        ));
                    }
                }
            }
        }
        descriptors
    }

    /// Returns the immutable current value for every active descriptor.  This
    /// deliberately mirrors `property_descriptors` rather than extending a
    /// descriptor with mutable document data: descriptor metadata and current
    /// authority remain two independently readable surfaces.
    pub fn property_values(&self) -> Vec<PropertyCurrentValue> {
        self.property_descriptors()
            .into_iter()
            .map(|descriptor| PropertyCurrentValue {
                value: self.property_value_for(&descriptor),
                descriptor,
            })
            .collect()
    }

    /// Begins a deliberate compound-variant transition from an active selector
    /// descriptor. The resulting draft has explicit domain-owned initial
    /// payload values and must be finalized before it becomes an existing
    /// `PatternDefinitionEdit`.
    pub fn variant_transition_draft(
        &self,
        selector: &PropertyDescriptor,
        choice: PropertyEnumChoice,
    ) -> Result<VariantTransitionDraft, ValidationError> {
        if !self.property_descriptors().contains(selector) {
            return Err(ValidationError::new(
                "transition_draft.selector",
                "transition selector is inactive or stale",
            ));
        }
        let base_choice = self
            .property_values()
            .into_iter()
            .find(|value| value.descriptor == *selector)
            .and_then(|value| match value.value {
                PropertyCurrentValueKind::EnumChoice(choice) => Some(choice),
                _ => None,
            })
            .ok_or_else(|| {
                ValidationError::new(
                    "transition_draft.selector",
                    "transition selector does not have an enum value",
                )
            })?;
        if !selector.choices.contains(&choice) {
            return Err(ValidationError::new(
                "transition_draft.choice",
                "transition choice is not supported by this selector",
            ));
        }
        let fields = transition_fields_for(self, selector, base_choice, choice)?;
        let definition_id = transition_definition_id(selector.target).ok_or_else(|| {
            ValidationError::new(
                "transition_draft.selector",
                "transition selector has no structural definition target",
            )
        })?;
        let base_definition = self.definition(definition_id).cloned().ok_or_else(|| {
            ValidationError::new(
                "transition_draft.target",
                "transition definition is missing",
            )
        })?;
        Ok(VariantTransitionDraft {
            selector: selector.clone(),
            base_choice,
            choice,
            fields,
            base_definition,
        })
    }

    fn property_value_for(&self, descriptor: &PropertyDescriptor) -> PropertyCurrentValueKind {
        let channel = |id| {
            self.channel(id)
                .map(ChannelPropertyState::Legacy)
                .or_else(|| self.modeled_channel(id).map(ChannelPropertyState::Modeled))
        };
        match descriptor.target {
            PropertyTarget::Document => match descriptor.field {
                PropertyFieldId::SourceReference => PropertyCurrentValueKind::Reference(
                    PropertyReferenceValue::Source(self.source.clone()),
                ),
                _ => unreachable!("only document descriptor is source reference"),
            },
            PropertyTarget::Channel(channel_id) => {
                let channel = channel(channel_id).expect("active descriptor targets channel");
                match descriptor.field {
                    PropertyFieldId::DensityAcrossX => {
                        PropertyCurrentValueKind::FiniteF64(channel.layout().density.across_x)
                    }
                    PropertyFieldId::DensityAcrossY => {
                        PropertyCurrentValueKind::FiniteF64(channel.layout().density.across_y)
                    }
                    PropertyFieldId::DensityAspectLocked => {
                        PropertyCurrentValueKind::Boolean(channel.layout().density.aspect_locked)
                    }
                    PropertyFieldId::RotationDegrees => {
                        PropertyCurrentValueKind::FiniteF64(channel.layout().rotation_degrees)
                    }
                    PropertyFieldId::TranslationX => {
                        PropertyCurrentValueKind::FiniteF64(channel.layout().translation_x)
                    }
                    PropertyFieldId::TranslationY => {
                        PropertyCurrentValueKind::FiniteF64(channel.layout().translation_y)
                    }
                    PropertyFieldId::MarkMinimumSize => {
                        PropertyCurrentValueKind::FiniteF64(channel.mark().minimum_size)
                    }
                    PropertyFieldId::MarkMaximumSize => {
                        PropertyCurrentValueKind::FiniteF64(channel.mark().maximum_size)
                    }
                    PropertyFieldId::Opacity => {
                        PropertyCurrentValueKind::FiniteF64(channel.opacity())
                    }
                    PropertyFieldId::Visibility => {
                        PropertyCurrentValueKind::Boolean(channel.visible())
                    }
                    PropertyFieldId::DefinitionSelection => PropertyCurrentValueKind::Reference(
                        PropertyReferenceValue::Definition(channel.definition_id()),
                    ),
                    PropertyFieldId::ColorRed => {
                        PropertyCurrentValueKind::FiniteF64(channel.color().red)
                    }
                    PropertyFieldId::ColorGreen => {
                        PropertyCurrentValueKind::FiniteF64(channel.color().green)
                    }
                    PropertyFieldId::ColorBlue => {
                        PropertyCurrentValueKind::FiniteF64(channel.color().blue)
                    }
                    PropertyFieldId::ColorAlpha => {
                        PropertyCurrentValueKind::FiniteF64(channel.color().alpha)
                    }
                    PropertyFieldId::LegacyMappingComponent => {
                        PropertyCurrentValueKind::EnumChoice(
                            PropertyEnumChoice::SourceMappingComponent(
                                match channel
                                    .legacy_mapping()
                                    .expect("legacy descriptor")
                                    .component
                                {
                                    SourceComponent::Luminance => SourceMappingComponent::Luminance,
                                    SourceComponent::Alpha => SourceMappingComponent::Alpha,
                                },
                            ),
                        )
                    }
                    PropertyFieldId::LegacyMappingPlacement => {
                        PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::SourcePlacement(
                            channel
                                .legacy_mapping()
                                .expect("legacy descriptor")
                                .placement,
                        ))
                    }
                    PropertyFieldId::ModeledMappingComponent => {
                        PropertyCurrentValueKind::EnumChoice(
                            PropertyEnumChoice::SourceMappingComponent(
                                channel.mapping().expect("modeled descriptor").component,
                            ),
                        )
                    }
                    PropertyFieldId::ModeledMappingPlacement => {
                        PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::SourcePlacement(
                            channel.mapping().expect("modeled descriptor").placement,
                        ))
                    }
                    PropertyFieldId::ModeledMappingInverted => PropertyCurrentValueKind::Boolean(
                        channel.mapping().expect("modeled descriptor").inverted,
                    ),
                    PropertyFieldId::ModeledMappingGain => PropertyCurrentValueKind::FiniteF64(
                        channel.mapping().expect("modeled descriptor").gain,
                    ),
                    PropertyFieldId::ModeledMappingBias => PropertyCurrentValueKind::FiniteF64(
                        channel.mapping().expect("modeled descriptor").bias,
                    ),
                    PropertyFieldId::Paint => {
                        PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::Paint(
                            match channel.paint().expect("modeled descriptor") {
                                ChannelPaint::Solid(_) => PaintKind::Solid,
                                ChannelPaint::SampledSource => PaintKind::SampledSource,
                            },
                        ))
                    }
                    _ => unreachable!("channel descriptor field"),
                }
            }
            PropertyTarget::Definition(definition_id) => {
                let definition = self
                    .pattern_definitions
                    .iter()
                    .find(|definition| definition.id == definition_id)
                    .expect("active definition descriptor");
                match descriptor.field {
                    PropertyFieldId::CoverageGuardSteps => {
                        PropertyCurrentValueKind::U32(definition.coverage.guard_steps)
                    }
                    PropertyFieldId::CoverageMaximumSupportRadius => {
                        PropertyCurrentValueKind::FiniteF64(
                            definition.coverage.maximum_support_radius,
                        )
                    }
                    _ => unreachable!("definition descriptor field"),
                }
            }
            PropertyTarget::GuideDimension(definition_id, mechanism_id, dimension_id) => {
                let dimension = self
                    .pattern_definitions
                    .iter()
                    .find(|definition| definition.id == definition_id)
                    .and_then(|definition| {
                        definition
                            .mechanisms
                            .iter()
                            .find(|mechanism| mechanism.id() == mechanism_id)
                    })
                    .and_then(|mechanism| match mechanism {
                        PatternMechanism::StraightGuideDimensions { dimensions, .. } => dimensions
                            .iter()
                            .find(|dimension| dimension.id == dimension_id),
                        _ => None,
                    })
                    .expect("active guide dimension descriptor");
                match descriptor.field {
                    PropertyFieldId::GuideBaselineAngle => {
                        PropertyCurrentValueKind::FiniteF64(dimension.baseline_angle_degrees)
                    }
                    PropertyFieldId::GuidePhase => {
                        PropertyCurrentValueKind::FiniteF64(dimension.phase)
                    }
                    PropertyFieldId::GuideSpacingMultiplier => {
                        PropertyCurrentValueKind::FiniteF64(dimension.repetition.spacing_multiplier)
                    }
                    _ => unreachable!("guide descriptor field"),
                }
            }
            PropertyTarget::Mechanism(definition_id, mechanism_id) => {
                let mechanism = self
                    .pattern_definitions
                    .iter()
                    .find(|definition| definition.id == definition_id)
                    .and_then(|definition| {
                        definition
                            .mechanisms
                            .iter()
                            .find(|mechanism| mechanism.id() == mechanism_id)
                    })
                    .expect("active mechanism descriptor");
                property_value_for_mechanism(descriptor.field, mechanism)
            }
            PropertyTarget::OutputLayer(definition_id, output_layer_id) => {
                let layer = self
                    .pattern_definitions
                    .iter()
                    .find(|definition| definition.id == definition_id)
                    .and_then(|definition| {
                        definition
                            .output_layers
                            .iter()
                            .find(|layer| layer.id() == output_layer_id)
                    })
                    .expect("active output descriptor");
                match (descriptor.field, layer) {
                    (
                        PropertyFieldId::OutputSiteProduct,
                        PatternOutputLayer::CircularMarks {
                            site_mechanism_id, ..
                        }
                        | PatternOutputLayer::MarkPrototype {
                            site_mechanism_id, ..
                        },
                    ) => PropertyCurrentValueKind::Reference(PropertyReferenceValue::Mechanism(
                        *site_mechanism_id,
                    )),
                    (
                        PropertyFieldId::OutputPrototype,
                        PatternOutputLayer::MarkPrototype { prototype, .. },
                    ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::MarkPrototype(
                        match prototype {
                            MarkPrototype::Circle => MarkPrototypeKind::Circle,
                        },
                    )),
                    (
                        PropertyFieldId::OutputOrientation,
                        PatternOutputLayer::MarkPrototype { orientation, .. },
                    ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::MarkOrientation(
                        match orientation {
                            MarkOrientation::Fixed => MarkOrientationKind::Fixed,
                            MarkOrientation::GuideTangent { .. } => {
                                MarkOrientationKind::GuideTangent
                            }
                            MarkOrientation::GuideNormal { .. } => MarkOrientationKind::GuideNormal,
                        },
                    )),
                    (
                        PropertyFieldId::OutputOrientationDimension,
                        PatternOutputLayer::MarkPrototype {
                            orientation:
                                MarkOrientation::GuideTangent { dimension_id }
                                | MarkOrientation::GuideNormal { dimension_id },
                            ..
                        },
                    ) => PropertyCurrentValueKind::Reference(
                        PropertyReferenceValue::GuideDimension(*dimension_id),
                    ),
                    _ => unreachable!("output descriptor field"),
                }
            }
        }
    }

    /// Mechanical bidirectional completeness gate for the active schema.
    pub fn validate_property_descriptors(&self) -> Result<(), ValidationError> {
        let descriptors = self.property_descriptors();
        let mut seen = HashSet::new();
        for descriptor in &descriptors {
            if !seen.insert((descriptor.field, descriptor.target)) {
                return Err(ValidationError::new(
                    "capabilities.descriptors",
                    "duplicate descriptor field target",
                ));
            }
            let contract = property_field_contract(descriptor.field);
            if descriptor.command_kind() != contract.command_kind
                || descriptor.value_kind != contract.value_kind
                || descriptor.bounds != contract.bounds
                || descriptor.unit != contract.unit
                || descriptor.invalidation != contract.invalidation
                || descriptor.copy_on_edit_escalates_to_family
                    != contract.copy_on_edit_escalates_to_family
                || descriptor.reference_constraint != contract.reference_constraint
                || descriptor.choice_policy != contract.choice_policy
            {
                return Err(ValidationError::new(
                    "capabilities.descriptors",
                    "descriptor diverges from its field contract",
                ));
            }
            if contract.choice_policy == PropertyChoicePolicy::Static
                && descriptor.choices != contract.choices
            {
                return Err(ValidationError::new(
                    "capabilities.descriptors",
                    "descriptor choices diverge from its field contract",
                ));
            }
            if !matches!(
                contract.applicability,
                PropertyApplicability::CurrentExclusion
            ) && descriptor.structural_support != contract.structural_support
            {
                return Err(ValidationError::new(
                    "capabilities.descriptors",
                    "descriptor structural support diverges from its field contract",
                ));
            }
            if dependency_for_contract(contract.applicability, descriptor.dependency)
                != descriptor.dependency
            {
                return Err(ValidationError::new(
                    "capabilities.descriptors",
                    "descriptor dependency diverges from its field contract",
                ));
            }
        }
        if descriptors != self.property_descriptors() {
            return Err(ValidationError::new(
                "capabilities.descriptors",
                "descriptor order is nondeterministic",
            ));
        }
        Ok(())
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

    fn channel_layout(&self, channel_id: ChannelId) -> Option<&ChannelPatternLayout> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .map(|channel| &channel.layout),
            ChannelConfiguration::Topology { topology, .. } => topology
                .channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .map(|channel| &channel.layout),
        }
    }
    fn channel_mark_response(&self, channel_id: ChannelId) -> Option<&MarkGeometryResponse> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .map(|channel| &channel.mark_geometry_response),
            ChannelConfiguration::Topology { topology, .. } => topology
                .channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .map(|channel| &channel.mark_geometry_response),
        }
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
        if let PatternFamily::RandomSites { .. } = source.family {
            let id = self.allocate_definition_id()?;
            let base_id = self.allocate_mechanism_id()?;
            let modulation_id = PatternMechanismId(base_id.0.checked_add(1).ok_or_else(|| {
                ValidationError::new(
                    "pattern_definitions.mechanisms.id",
                    "document mechanism ID space is exhausted",
                )
            })?);
            let exclusion_id =
                PatternMechanismId(modulation_id.0.checked_add(1).ok_or_else(|| {
                    ValidationError::new(
                        "pattern_definitions.mechanisms.id",
                        "document mechanism ID space is exhausted",
                    )
                })?);
            let site_id = PatternMechanismId(exclusion_id.0.checked_add(1).ok_or_else(|| {
                ValidationError::new(
                    "pattern_definitions.mechanisms.id",
                    "document mechanism ID space is exhausted",
                )
            })?);
            let output_id = self.allocate_output_layer_id()?;
            let [
                PatternMechanism::RandomSiteProcess {
                    character, seed, ..
                },
                PatternMechanism::SiteDensityModulation { modulation, .. },
                PatternMechanism::SiteExclusion { policy, .. },
                PatternMechanism::RandomSiteProduct {
                    maximum_attempts,
                    maximum_neighbor_checks,
                    ..
                },
            ] = source.mechanisms.as_slice()
            else {
                return Err(ValidationError::new(
                    "pattern_definitions.family",
                    "random definition has an incompatible mechanism chain",
                ));
            };
            return Ok(PatternDefinition::random_sites(
                id,
                source.name.clone(),
                base_id,
                modulation_id,
                exclusion_id,
                site_id,
                output_id,
                character.clone(),
                *seed,
                modulation.clone(),
                policy.clone(),
                *maximum_attempts,
                *maximum_neighbor_checks,
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
        if candidate == *self {
            return Err(ValidationError::new(
                "command",
                "command is a semantic no-op",
            ));
        }
        let mut result = command.result_for_transition(self, &candidate);
        if let DocumentCommand::EditSharedPatternDefinition { definition_id, .. } = command {
            result.affected_channels = self.linked_channels(*definition_id);
        }
        if let DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id,
            base_definition,
            edit,
        } = command
            && self.linked_channels(base_definition.id).len() > 1
            && definition_edit_invalidation(edit) == InvalidationLevel::Realization
        {
            // Copy-on-edit changes definition and internal stable IDs, so its
            // family key necessarily changes even for an output-only field.
            result.affected_channels = vec![*channel_id];
            result.invalidation = InvalidationLevel::Family;
        } else if matches!(
            command,
            DocumentCommand::EditSelectedChannelPatternDefinition { .. }
        ) || matches!(command, DocumentCommand::EditSharedPatternDefinition { .. })
        {
            let edit = match command {
                DocumentCommand::EditSelectedChannelPatternDefinition { edit, .. }
                | DocumentCommand::EditSharedPatternDefinition { edit, .. } => edit,
                _ => unreachable!(),
            };
            result.invalidation = definition_edit_invalidation(edit);
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
        PatternDefinitionEdit::SetCoverageGuardSteps { guard_steps } => {
            definition.coverage.guard_steps = *guard_steps
        }
        PatternDefinitionEdit::SetCoverageMaximumSupportRadius {
            maximum_support_radius,
        } => definition.coverage.maximum_support_radius = *maximum_support_radius,
        PatternDefinitionEdit::SetGuideBaselineAngle {
            mechanism_id,
            dimension_id,
            baseline_angle_degrees,
        } => {
            if let Some(PatternMechanism::StraightGuideDimensions { dimensions, .. }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
                && let Some(existing) = dimensions
                    .iter_mut()
                    .find(|value| value.id == *dimension_id)
            {
                existing.baseline_angle_degrees = *baseline_angle_degrees;
            }
        }
        PatternDefinitionEdit::SetGuidePhase {
            mechanism_id,
            dimension_id,
            phase,
        } => {
            if let Some(PatternMechanism::StraightGuideDimensions { dimensions, .. }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
                && let Some(existing) = dimensions
                    .iter_mut()
                    .find(|value| value.id == *dimension_id)
            {
                existing.phase = *phase;
            }
        }
        PatternDefinitionEdit::SetGuideSpacingMultiplier {
            mechanism_id,
            dimension_id,
            spacing_multiplier,
        } => {
            if let Some(PatternMechanism::StraightGuideDimensions { dimensions, .. }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
                && let Some(existing) = dimensions
                    .iter_mut()
                    .find(|value| value.id == *dimension_id)
            {
                existing.repetition.spacing_multiplier = *spacing_multiplier;
            }
        }
        PatternDefinitionEdit::SetIntersectionDimensions {
            mechanism_id,
            dimensions,
        } => {
            if let Some(PatternMechanism::SelectedGuideIntersections {
                dimensions: current,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = dimensions.clone();
            }
        }
        PatternDefinitionEdit::SetIntersectionMergeEpsilon {
            mechanism_id,
            merge_epsilon,
        } => {
            if let Some(PatternMechanism::SelectedGuideIntersections {
                merge_epsilon: current,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *merge_epsilon;
            }
        }
        PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id,
            dimensions,
        } => {
            if let Some(PatternMechanism::AlongGuideSites {
                dimensions: current,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = dimensions.clone();
            }
        }
        PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
            mechanism_id,
            interval_multiplier,
        } => {
            if let Some(PatternMechanism::AlongGuideSites {
                interval_multiplier: current,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *interval_multiplier;
            }
        }
        PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id,
            phase,
        } => {
            if let Some(PatternMechanism::AlongGuideSites { phase: current, .. }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *phase;
            }
        }
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id,
            character,
        } => {
            if let Some(PatternMechanism::RandomSiteProcess {
                character: current, ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = character.clone();
            }
        }
        PatternDefinitionEdit::SetRandomSeed { mechanism_id, seed } => {
            if let Some(PatternMechanism::RandomSiteProcess { seed: current, .. }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *seed;
            }
        }
        PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id,
            minimum_center_distance,
        } => {
            if let Some(PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Even {
                        minimum_center_distance: current,
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *minimum_center_distance;
            }
        }
        PatternDefinitionEdit::SetRandomClusterDensity {
            mechanism_id,
            cluster_density,
        } => {
            if let Some(PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Clustered {
                        cluster_density: current,
                        ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *cluster_density;
            }
        }
        PatternDefinitionEdit::SetRandomClusterSpread {
            mechanism_id,
            cluster_spread,
        } => {
            if let Some(PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Clustered {
                        cluster_spread: current,
                        ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *cluster_spread;
            }
        }
        PatternDefinitionEdit::SetRandomClusterStrength {
            mechanism_id,
            cluster_strength,
        } => {
            if let Some(PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Clustered {
                        cluster_strength: current,
                        ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *cluster_strength;
            }
        }
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id,
            modulation,
        } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation: current,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = modulation.clone();
            }
        }
        PatternDefinitionEdit::SetArtworkWeightMappingComponent {
            mechanism_id,
            component,
        } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        mapping: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                current.component = *component;
            }
        }
        PatternDefinitionEdit::SetArtworkWeightMappingPlacement {
            mechanism_id,
            placement,
        } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        mapping: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                current.placement = *placement;
            }
        }
        PatternDefinitionEdit::SetArtworkWeightMappingInverted {
            mechanism_id,
            inverted,
        } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        mapping: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                current.inverted = *inverted;
            }
        }
        PatternDefinitionEdit::SetArtworkWeightMappingGain { mechanism_id, gain } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        mapping: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                current.gain = *gain;
            }
        }
        PatternDefinitionEdit::SetArtworkWeightMappingBias { mechanism_id, bias } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        mapping: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                current.bias = *bias;
            }
        }
        PatternDefinitionEdit::SetArtworkWeightStrength {
            mechanism_id,
            strength,
        } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        strength: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *strength;
            }
        }
        PatternDefinitionEdit::SetArtworkWeightResponse {
            mechanism_id,
            response,
        } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        response: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *response;
            }
        }
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id,
            policy,
        } => {
            if let Some(PatternMechanism::SiteExclusion {
                policy: current, ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = policy.clone();
            }
        }
        PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id,
            minimum_center_distance,
        } => {
            if let Some(PatternMechanism::SiteExclusion {
                policy: SiteExclusionPolicy::MinimumCenterDistance { minimum: current },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *minimum_center_distance;
            }
        }
        PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id,
            margin,
        } => {
            if let Some(PatternMechanism::SiteExclusion {
                policy:
                    SiteExclusionPolicy::VisibleMarkMargin {
                        margin: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *margin;
            }
        }
        PatternDefinitionEdit::SetVisibleMarkSizingPolicy {
            mechanism_id,
            sizing,
        } => {
            if let Some(PatternMechanism::SiteExclusion {
                policy:
                    SiteExclusionPolicy::VisibleMarkMargin {
                        sizing: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *sizing;
            }
        }
        PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id,
            maximum_attempts,
        } => {
            if let Some(PatternMechanism::RandomSiteProduct {
                maximum_attempts: current_attempts,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current_attempts = *maximum_attempts;
            }
        }
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id,
            maximum_neighbor_checks,
        } => {
            if let Some(PatternMechanism::RandomSiteProduct {
                maximum_neighbor_checks: current_checks,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current_checks = *maximum_neighbor_checks;
            }
        }
        PatternDefinitionEdit::SetOutputSiteProduct {
            output_layer_id,
            site_mechanism_id,
        } => {
            if let Some(
                PatternOutputLayer::CircularMarks {
                    site_mechanism_id: current,
                    ..
                }
                | PatternOutputLayer::MarkPrototype {
                    site_mechanism_id: current,
                    ..
                },
            ) = definition
                .output_layers
                .iter_mut()
                .find(|layer| layer.id() == *output_layer_id)
            {
                *current = *site_mechanism_id;
            }
        }
        PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id,
            prototype,
        } => {
            if let Some(PatternOutputLayer::MarkPrototype {
                prototype: current, ..
            }) = definition
                .output_layers
                .iter_mut()
                .find(|layer| layer.id() == *output_layer_id)
            {
                *current = prototype.clone();
            }
        }
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id,
            orientation,
        } => {
            if let Some(PatternOutputLayer::MarkPrototype {
                orientation: current,
                ..
            }) = definition
                .output_layers
                .iter_mut()
                .find(|layer| layer.id() == *output_layer_id)
            {
                *current = orientation.clone();
            }
        }
        PatternDefinitionEdit::SetOutputOrientationDimension {
            output_layer_id,
            dimension_id,
        } => {
            if let Some(PatternOutputLayer::MarkPrototype { orientation, .. }) = definition
                .output_layers
                .iter_mut()
                .find(|layer| layer.id() == *output_layer_id)
            {
                match orientation {
                    MarkOrientation::GuideTangent {
                        dimension_id: current,
                    }
                    | MarkOrientation::GuideNormal {
                        dimension_id: current,
                    } => *current = *dimension_id,
                    MarkOrientation::Fixed => {}
                }
            }
        }
    }
}

fn remap_definition_edit_for_duplicate(
    source: &PatternDefinition,
    duplicate: &PatternDefinition,
    edit: &PatternDefinitionEdit,
) -> PatternDefinitionEdit {
    let mechanism = |id| remap_mechanism_id(source, duplicate, id);
    let dimension = |id| remap_dimension_id(source, duplicate, id);
    match edit {
        PatternDefinitionEdit::SetCoverageGuardSteps { guard_steps } => {
            PatternDefinitionEdit::SetCoverageGuardSteps {
                guard_steps: *guard_steps,
            }
        }
        PatternDefinitionEdit::SetCoverageMaximumSupportRadius {
            maximum_support_radius,
        } => PatternDefinitionEdit::SetCoverageMaximumSupportRadius {
            maximum_support_radius: *maximum_support_radius,
        },
        PatternDefinitionEdit::SetGuideBaselineAngle {
            mechanism_id,
            dimension_id,
            baseline_angle_degrees,
        } => PatternDefinitionEdit::SetGuideBaselineAngle {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            baseline_angle_degrees: *baseline_angle_degrees,
        },
        PatternDefinitionEdit::SetGuidePhase {
            mechanism_id,
            dimension_id,
            phase,
        } => PatternDefinitionEdit::SetGuidePhase {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            phase: *phase,
        },
        PatternDefinitionEdit::SetGuideSpacingMultiplier {
            mechanism_id,
            dimension_id,
            spacing_multiplier,
        } => PatternDefinitionEdit::SetGuideSpacingMultiplier {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            spacing_multiplier: *spacing_multiplier,
        },
        PatternDefinitionEdit::SetIntersectionDimensions {
            mechanism_id,
            dimensions,
        } => PatternDefinitionEdit::SetIntersectionDimensions {
            mechanism_id: mechanism(*mechanism_id),
            dimensions: dimensions.iter().copied().map(dimension).collect(),
        },
        PatternDefinitionEdit::SetIntersectionMergeEpsilon {
            mechanism_id,
            merge_epsilon,
        } => PatternDefinitionEdit::SetIntersectionMergeEpsilon {
            mechanism_id: mechanism(*mechanism_id),
            merge_epsilon: *merge_epsilon,
        },
        PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id,
            dimensions,
        } => PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id: mechanism(*mechanism_id),
            dimensions: dimensions.iter().copied().map(dimension).collect(),
        },
        PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
            mechanism_id,
            interval_multiplier,
        } => PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
            mechanism_id: mechanism(*mechanism_id),
            interval_multiplier: *interval_multiplier,
        },
        PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id,
            phase,
        } => PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id: mechanism(*mechanism_id),
            phase: *phase,
        },
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id,
            character,
        } => PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: mechanism(*mechanism_id),
            character: character.clone(),
        },
        PatternDefinitionEdit::SetRandomSeed { mechanism_id, seed } => {
            PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: mechanism(*mechanism_id),
                seed: *seed,
            }
        }
        PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id,
            minimum_center_distance,
        } => PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id: mechanism(*mechanism_id),
            minimum_center_distance: *minimum_center_distance,
        },
        PatternDefinitionEdit::SetRandomClusterDensity {
            mechanism_id,
            cluster_density,
        } => PatternDefinitionEdit::SetRandomClusterDensity {
            mechanism_id: mechanism(*mechanism_id),
            cluster_density: *cluster_density,
        },
        PatternDefinitionEdit::SetRandomClusterSpread {
            mechanism_id,
            cluster_spread,
        } => PatternDefinitionEdit::SetRandomClusterSpread {
            mechanism_id: mechanism(*mechanism_id),
            cluster_spread: *cluster_spread,
        },
        PatternDefinitionEdit::SetRandomClusterStrength {
            mechanism_id,
            cluster_strength,
        } => PatternDefinitionEdit::SetRandomClusterStrength {
            mechanism_id: mechanism(*mechanism_id),
            cluster_strength: *cluster_strength,
        },
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id,
            modulation,
        } => PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id: mechanism(*mechanism_id),
            modulation: modulation.clone(),
        },
        PatternDefinitionEdit::SetArtworkWeightMappingComponent {
            mechanism_id,
            component,
        } => PatternDefinitionEdit::SetArtworkWeightMappingComponent {
            mechanism_id: mechanism(*mechanism_id),
            component: *component,
        },
        PatternDefinitionEdit::SetArtworkWeightMappingPlacement {
            mechanism_id,
            placement,
        } => PatternDefinitionEdit::SetArtworkWeightMappingPlacement {
            mechanism_id: mechanism(*mechanism_id),
            placement: *placement,
        },
        PatternDefinitionEdit::SetArtworkWeightMappingInverted {
            mechanism_id,
            inverted,
        } => PatternDefinitionEdit::SetArtworkWeightMappingInverted {
            mechanism_id: mechanism(*mechanism_id),
            inverted: *inverted,
        },
        PatternDefinitionEdit::SetArtworkWeightMappingGain { mechanism_id, gain } => {
            PatternDefinitionEdit::SetArtworkWeightMappingGain {
                mechanism_id: mechanism(*mechanism_id),
                gain: *gain,
            }
        }
        PatternDefinitionEdit::SetArtworkWeightMappingBias { mechanism_id, bias } => {
            PatternDefinitionEdit::SetArtworkWeightMappingBias {
                mechanism_id: mechanism(*mechanism_id),
                bias: *bias,
            }
        }
        PatternDefinitionEdit::SetArtworkWeightStrength {
            mechanism_id,
            strength,
        } => PatternDefinitionEdit::SetArtworkWeightStrength {
            mechanism_id: mechanism(*mechanism_id),
            strength: *strength,
        },
        PatternDefinitionEdit::SetArtworkWeightResponse {
            mechanism_id,
            response,
        } => PatternDefinitionEdit::SetArtworkWeightResponse {
            mechanism_id: mechanism(*mechanism_id),
            response: *response,
        },
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id,
            policy,
        } => PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: mechanism(*mechanism_id),
            policy: policy.clone(),
        },
        PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id,
            minimum_center_distance,
        } => PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id: mechanism(*mechanism_id),
            minimum_center_distance: *minimum_center_distance,
        },
        PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id,
            margin,
        } => PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id: mechanism(*mechanism_id),
            margin: *margin,
        },
        PatternDefinitionEdit::SetVisibleMarkSizingPolicy {
            mechanism_id,
            sizing,
        } => PatternDefinitionEdit::SetVisibleMarkSizingPolicy {
            mechanism_id: mechanism(*mechanism_id),
            sizing: *sizing,
        },
        PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id,
            maximum_attempts,
        } => PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id: mechanism(*mechanism_id),
            maximum_attempts: *maximum_attempts,
        },
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id,
            maximum_neighbor_checks,
        } => PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id: mechanism(*mechanism_id),
            maximum_neighbor_checks: *maximum_neighbor_checks,
        },
        PatternDefinitionEdit::SetOutputSiteProduct {
            output_layer_id,
            site_mechanism_id,
        } => PatternDefinitionEdit::SetOutputSiteProduct {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            site_mechanism_id: mechanism(*site_mechanism_id),
        },
        PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id,
            prototype,
        } => PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            prototype: prototype.clone(),
        },
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id,
            orientation,
        } => PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            orientation: remap_orientation(source, duplicate, orientation),
        },
        PatternDefinitionEdit::SetOutputOrientationDimension {
            output_layer_id,
            dimension_id,
        } => PatternDefinitionEdit::SetOutputOrientationDimension {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            dimension_id: dimension(*dimension_id),
        },
    }
}

fn remap_mechanism_id(
    source: &PatternDefinition,
    duplicate: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> PatternMechanismId {
    let index = source
        .mechanisms
        .iter()
        .position(|mechanism| mechanism.id() == mechanism_id)
        .expect("validated edit references a source mechanism");
    duplicate.mechanisms[index].id()
}

fn remap_dimension_id(
    source: &PatternDefinition,
    duplicate: &PatternDefinition,
    dimension_id: GuideDimensionId,
) -> GuideDimensionId {
    for (source_mechanism, duplicate_mechanism) in
        source.mechanisms.iter().zip(&duplicate.mechanisms)
    {
        let (
            PatternMechanism::StraightGuideDimensions {
                dimensions: source_dimensions,
                ..
            },
            PatternMechanism::StraightGuideDimensions {
                dimensions: duplicate_dimensions,
                ..
            },
        ) = (source_mechanism, duplicate_mechanism)
        else {
            continue;
        };
        if let Some(index) = source_dimensions
            .iter()
            .position(|dimension| dimension.id == dimension_id)
        {
            return duplicate_dimensions[index].id;
        }
    }
    panic!("validated edit references a source guide dimension")
}

fn remap_output_layer_id(
    source: &PatternDefinition,
    duplicate: &PatternDefinition,
    output_layer_id: PatternOutputLayerId,
) -> PatternOutputLayerId {
    let index = source
        .output_layers
        .iter()
        .position(|layer| layer.id() == output_layer_id)
        .expect("validated edit references a source output layer");
    duplicate.output_layers[index].id()
}

fn remap_orientation(
    source: &PatternDefinition,
    duplicate: &PatternDefinition,
    orientation: &MarkOrientation,
) -> MarkOrientation {
    match orientation {
        MarkOrientation::Fixed => MarkOrientation::Fixed,
        MarkOrientation::GuideTangent { dimension_id } => MarkOrientation::GuideTangent {
            dimension_id: remap_dimension_id(source, duplicate, *dimension_id),
        },
        MarkOrientation::GuideNormal { dimension_id } => MarkOrientation::GuideNormal {
            dimension_id: remap_dimension_id(source, duplicate, *dimension_id),
        },
    }
}

fn validate_definition_edit(
    definition: &PatternDefinition,
    edit: &PatternDefinitionEdit,
) -> Result<(), ValidationError> {
    validate_property_field_projection(edit.field_projection())?;
    match edit {
        PatternDefinitionEdit::SetCoverageGuardSteps { .. } => Ok(()),
        PatternDefinitionEdit::SetCoverageMaximumSupportRadius {
            maximum_support_radius,
        } => validate_nonnegative_finite(
            *maximum_support_radius,
            "pattern_definitions.coverage.maximum_support_radius",
        ),
        PatternDefinitionEdit::SetGuideBaselineAngle {
            mechanism_id,
            dimension_id,
            baseline_angle_degrees,
        } => {
            validate_guide_dimension_target(definition, *mechanism_id, *dimension_id)?;
            validate_finite(
                *baseline_angle_degrees,
                "pattern_definitions.mechanisms.dimensions.baseline_angle_degrees",
            )
        }
        PatternDefinitionEdit::SetGuidePhase {
            mechanism_id,
            dimension_id,
            phase,
        } => {
            validate_guide_dimension_target(definition, *mechanism_id, *dimension_id)?;
            validate_finite(*phase, "pattern_definitions.mechanisms.dimensions.phase")
        }
        PatternDefinitionEdit::SetGuideSpacingMultiplier {
            mechanism_id,
            dimension_id,
            spacing_multiplier,
        } => {
            validate_guide_dimension_target(definition, *mechanism_id, *dimension_id)?;
            validate_positive_finite(
                *spacing_multiplier,
                "pattern_definitions.mechanisms.dimensions.repetition.spacing_multiplier",
            )
        }
        PatternDefinitionEdit::SetIntersectionDimensions {
            mechanism_id,
            dimensions,
        } => {
            let available = validate_selected_intersection_target(definition, *mechanism_id)?;
            validate_selection(
                dimensions,
                available,
                2,
                "pattern_definitions.mechanisms.intersections.dimensions",
            )
        }
        PatternDefinitionEdit::SetIntersectionMergeEpsilon {
            mechanism_id,
            merge_epsilon,
        } => {
            validate_selected_intersection_target(definition, *mechanism_id)?;
            validate_nonnegative_finite(
                *merge_epsilon,
                "pattern_definitions.mechanisms.intersections.merge_epsilon",
            )
        }
        PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id,
            dimensions,
        } => {
            let available = validate_along_guide_target(definition, *mechanism_id)?;
            validate_selection(
                dimensions,
                available,
                1,
                "pattern_definitions.mechanisms.along_guides.dimensions",
            )
        }
        PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
            mechanism_id,
            interval_multiplier,
        } => {
            validate_along_guide_target(definition, *mechanism_id)?;
            validate_positive_finite(
                *interval_multiplier,
                "pattern_definitions.mechanisms.along_guides.interval_multiplier",
            )
        }
        PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id,
            phase,
        } => {
            validate_along_guide_target(definition, *mechanism_id)?;
            validate_finite(*phase, "pattern_definitions.mechanisms.along_guides.phase")
        }
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id,
            character,
        } => {
            validate_random_process_target(definition, *mechanism_id)?;
            validate_random_character(character)
        }
        PatternDefinitionEdit::SetRandomSeed { mechanism_id, .. } => {
            validate_random_process_target(definition, *mechanism_id).map(|_| ())
        }
        PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id,
            minimum_center_distance,
        } => match validate_random_process_target(definition, *mechanism_id)? {
            RandomSiteCharacter::Even { .. } => validate_positive_finite(
                *minimum_center_distance,
                "pattern_definitions.mechanisms.random_sites.minimum_center_distance",
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.mechanisms.random_sites.minimum_center_distance",
                "field is inactive for the current random character",
            )),
        },
        PatternDefinitionEdit::SetRandomClusterDensity {
            mechanism_id,
            cluster_density,
        } => match validate_random_process_target(definition, *mechanism_id)? {
            RandomSiteCharacter::Clustered { .. } => validate_positive_finite(
                *cluster_density,
                "pattern_definitions.mechanisms.random_sites.cluster_density",
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.mechanisms.random_sites.cluster_density",
                "field is inactive for the current random character",
            )),
        },
        PatternDefinitionEdit::SetRandomClusterSpread {
            mechanism_id,
            cluster_spread,
        } => match validate_random_process_target(definition, *mechanism_id)? {
            RandomSiteCharacter::Clustered { .. } => validate_positive_finite(
                *cluster_spread,
                "pattern_definitions.mechanisms.random_sites.cluster_spread",
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.mechanisms.random_sites.cluster_spread",
                "field is inactive for the current random character",
            )),
        },
        PatternDefinitionEdit::SetRandomClusterStrength {
            mechanism_id,
            cluster_strength,
        } => match validate_random_process_target(definition, *mechanism_id)? {
            RandomSiteCharacter::Clustered { .. } => validate_unit_component(
                *cluster_strength,
                "pattern_definitions.mechanisms.random_sites.cluster_strength",
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.mechanisms.random_sites.cluster_strength",
                "field is inactive for the current random character",
            )),
        },
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id,
            modulation,
        } => {
            validate_density_modulation_target(definition, *mechanism_id)?;
            validate_site_density_modulation(modulation)
        }
        PatternDefinitionEdit::SetArtworkWeightMappingComponent { mechanism_id, .. }
        | PatternDefinitionEdit::SetArtworkWeightMappingPlacement { mechanism_id, .. }
        | PatternDefinitionEdit::SetArtworkWeightMappingInverted { mechanism_id, .. } => {
            validate_artwork_weighted_target(definition, *mechanism_id).map(|_| ())
        }
        PatternDefinitionEdit::SetArtworkWeightMappingGain { mechanism_id, gain } => {
            validate_artwork_weighted_target(definition, *mechanism_id)?;
            validate_nonnegative_finite(
                *gain,
                "pattern_definitions.mechanisms.site_density.mapping.gain",
            )
        }
        PatternDefinitionEdit::SetArtworkWeightMappingBias { mechanism_id, bias } => {
            validate_artwork_weighted_target(definition, *mechanism_id)?;
            validate_finite(
                *bias,
                "pattern_definitions.mechanisms.site_density.mapping.bias",
            )
        }
        PatternDefinitionEdit::SetArtworkWeightStrength {
            mechanism_id,
            strength,
        } => {
            validate_artwork_weighted_target(definition, *mechanism_id)?;
            validate_unit_component(
                *strength,
                "pattern_definitions.mechanisms.site_density.strength",
            )
        }
        PatternDefinitionEdit::SetArtworkWeightResponse { mechanism_id, .. } => {
            validate_artwork_weighted_target(definition, *mechanism_id).map(|_| ())
        }
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id,
            policy,
        } => {
            validate_exclusion_target(definition, *mechanism_id)?;
            validate_site_exclusion(policy)
        }
        PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id,
            minimum_center_distance,
        } => match validate_exclusion_target(definition, *mechanism_id)? {
            SiteExclusionPolicy::MinimumCenterDistance { .. } => validate_positive_finite(
                *minimum_center_distance,
                "pattern_definitions.mechanisms.site_exclusion.minimum",
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.mechanisms.site_exclusion.minimum",
                "field is inactive for the current exclusion policy",
            )),
        },
        PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id,
            margin,
        } => match validate_exclusion_target(definition, *mechanism_id)? {
            SiteExclusionPolicy::VisibleMarkMargin { .. } => validate_nonnegative_finite(
                *margin,
                "pattern_definitions.mechanisms.site_exclusion.margin",
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.mechanisms.site_exclusion.margin",
                "field is inactive for the current exclusion policy",
            )),
        },
        PatternDefinitionEdit::SetVisibleMarkSizingPolicy { mechanism_id, .. } => {
            match validate_exclusion_target(definition, *mechanism_id)? {
                SiteExclusionPolicy::VisibleMarkMargin { .. } => Ok(()),
                _ => Err(ValidationError::new(
                    "pattern_definitions.mechanisms.site_exclusion.sizing",
                    "field is inactive for the current exclusion policy",
                )),
            }
        }
        PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id,
            maximum_attempts,
        } => {
            validate_random_product_target(definition, *mechanism_id)?;
            if *maximum_attempts > 0 {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "pattern_definitions.mechanisms.random_sites.maximum_attempts",
                    "random product work limits must be nonzero",
                ))
            }
        }
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id,
            maximum_neighbor_checks,
        } => {
            validate_random_product_target(definition, *mechanism_id)?;
            if *maximum_neighbor_checks > 0 {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "pattern_definitions.mechanisms.random_sites.maximum_neighbor_checks",
                    "random product work limits must be nonzero",
                ))
            }
        }
        PatternDefinitionEdit::SetOutputSiteProduct {
            output_layer_id,
            site_mechanism_id,
        } => {
            validate_output_layer_target(definition, *output_layer_id)?;
            if definition
                .mechanisms
                .iter()
                .any(|mechanism| mechanism.id() == *site_mechanism_id)
            {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "pattern_definitions.output_layers.site_mechanism_id",
                    "command references a missing site mechanism",
                ))
            }
        }
        PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id,
            prototype,
        } => {
            validate_mark_prototype_output_target(definition, *output_layer_id)?;
            if matches!(prototype, MarkPrototype::Circle) {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "pattern_definitions.output_layers.prototype",
                    "unsupported mark prototype",
                ))
            }
        }
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id,
            orientation,
        } => {
            validate_mark_prototype_output_target(definition, *output_layer_id)?;
            validate_output_orientation(definition, orientation)
        }
        PatternDefinitionEdit::SetOutputOrientationDimension {
            output_layer_id,
            dimension_id,
        } => match validate_mark_prototype_output_target(definition, *output_layer_id)? {
            PatternOutputLayer::MarkPrototype {
                orientation:
                    MarkOrientation::GuideTangent { .. } | MarkOrientation::GuideNormal { .. },
                ..
            } => validate_output_orientation(
                definition,
                &MarkOrientation::GuideTangent {
                    dimension_id: *dimension_id,
                },
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.output_layers.orientation.dimension_id",
                "field is inactive for fixed output orientation",
            )),
        },
    }
}

fn validate_property_field_projection(
    projection: PropertyCommandFieldProjection,
) -> Result<(), ValidationError> {
    let contract = property_field_contract(projection.field);
    let kind_matches = matches!(
        (contract.value_kind, projection.value),
        (
            PropertyValueKind::FiniteF64,
            PropertyFieldValue::FiniteF64(_)
        ) | (PropertyValueKind::U32, PropertyFieldValue::U32(_))
            | (PropertyValueKind::Boolean, PropertyFieldValue::Boolean(_))
            | (
                PropertyValueKind::StableIdReference,
                PropertyFieldValue::StableIdReference
            )
            | (
                PropertyValueKind::StableIdReference,
                PropertyFieldValue::StableIdReferenceCollection(_)
            )
            | (
                PropertyValueKind::EnumChoice,
                PropertyFieldValue::EnumChoice(_)
            )
    );
    if !kind_matches {
        return Err(ValidationError::new(
            "command.field",
            "command value does not match its field contract",
        ));
    }
    let value = match projection.value {
        PropertyFieldValue::FiniteF64(value) => {
            if !value.is_finite() {
                return Err(ValidationError::new(
                    "command.field",
                    "value must be finite",
                ));
            }
            value
        }
        PropertyFieldValue::U32(value) => value as f64,
        PropertyFieldValue::EnumChoice(choice) => {
            if !contract.choices.is_empty() && !contract.choices.contains(&choice) {
                return Err(ValidationError::new(
                    "command.field",
                    "value is not a legal field enum choice",
                ));
            }
            return Ok(());
        }
        PropertyFieldValue::StableIdReferenceCollection(count) => {
            let PropertyReferenceConstraint::OrderedUniqueCollection {
                minimum_items,
                maximum_items,
            } = contract.reference_constraint
            else {
                return Err(ValidationError::new(
                    "command.field",
                    "field does not accept a reference collection",
                ));
            };
            if count < minimum_items as usize || count > maximum_items as usize {
                return Err(ValidationError::new(
                    "command.field",
                    "reference collection violates field cardinality",
                ));
            }
            return Ok(());
        }
        PropertyFieldValue::Boolean(_) | PropertyFieldValue::StableIdReference => return Ok(()),
    };
    if let Some(bounds) = contract.bounds {
        if let Some(minimum) = bounds.minimum
            && (value < minimum || (!bounds.minimum_inclusive && value == minimum))
        {
            return Err(ValidationError::new(
                "command.field",
                "value is below field minimum",
            ));
        }
        if let Some(maximum) = bounds.maximum
            && (value > maximum || (!bounds.maximum_inclusive && value == maximum))
        {
            return Err(ValidationError::new(
                "command.field",
                "value is above field maximum",
            ));
        }
    }
    Ok(())
}

fn validate_guide_dimension_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
    dimension_id: GuideDimensionId,
) -> Result<(), ValidationError> {
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::StraightGuideDimensions { dimensions, .. })
            if dimensions
                .iter()
                .any(|dimension| dimension.id == dimension_id) =>
        {
            Ok(())
        }
        Some(PatternMechanism::StraightGuideDimensions { .. }) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.dimensions.id",
            "command targets a missing guide dimension",
        )),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.dimensions",
            "command targets an incompatible guide mechanism",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing guide mechanism",
        )),
    }
}

fn straight_guide_root_dimensions(
    definition: &PatternDefinition,
) -> Result<&[StraightGuideDimension], ValidationError> {
    match definition.mechanisms.first() {
        Some(PatternMechanism::StraightGuideDimensions { dimensions, .. }) => Ok(dimensions),
        _ => Err(ValidationError::new(
            "pattern_definitions.family",
            "definition has no straight guide root",
        )),
    }
}

fn validate_selected_intersection_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<&[StraightGuideDimension], ValidationError> {
    let available = straight_guide_root_dimensions(definition)?;
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::SelectedGuideIntersections { .. }) => Ok(available),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets an incompatible intersection mechanism",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing intersection mechanism",
        )),
    }
}

fn validate_along_guide_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<&[StraightGuideDimension], ValidationError> {
    let available = straight_guide_root_dimensions(definition)?;
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::AlongGuideSites { .. }) => Ok(available),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets an incompatible along-guide mechanism",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing along-guide mechanism",
        )),
    }
}

fn validate_random_process_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<&RandomSiteCharacter, ValidationError> {
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::RandomSiteProcess { character, .. }) => Ok(character),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets an incompatible random process",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing random process",
        )),
    }
}

fn validate_density_modulation_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<&SiteDensityModulation, ValidationError> {
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::SiteDensityModulation { modulation, .. }) => Ok(modulation),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets an incompatible density modulation",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing density modulation",
        )),
    }
}

fn validate_artwork_weighted_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<&SourceMapping, ValidationError> {
    match validate_density_modulation_target(definition, mechanism_id)? {
        SiteDensityModulation::ArtworkWeighted { mapping, .. } => Ok(mapping),
        SiteDensityModulation::Uniform => Err(ValidationError::new(
            "pattern_definitions.mechanisms.site_density",
            "field is inactive for uniform density modulation",
        )),
    }
}

fn validate_exclusion_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<&SiteExclusionPolicy, ValidationError> {
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::SiteExclusion { policy, .. }) => Ok(policy),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets an incompatible exclusion mechanism",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing exclusion mechanism",
        )),
    }
}

fn validate_random_product_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<(), ValidationError> {
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::RandomSiteProduct { .. }) => Ok(()),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets an incompatible random product",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing random product",
        )),
    }
}

fn validate_output_layer_target(
    definition: &PatternDefinition,
    output_layer_id: PatternOutputLayerId,
) -> Result<&PatternOutputLayer, ValidationError> {
    definition
        .output_layers
        .iter()
        .find(|layer| layer.id() == output_layer_id)
        .ok_or_else(|| {
            ValidationError::new(
                "pattern_definitions.output_layers.id",
                "command targets a missing output layer",
            )
        })
}

fn validate_mark_prototype_output_target(
    definition: &PatternDefinition,
    output_layer_id: PatternOutputLayerId,
) -> Result<&PatternOutputLayer, ValidationError> {
    match validate_output_layer_target(definition, output_layer_id)? {
        layer @ PatternOutputLayer::MarkPrototype { .. } => Ok(layer),
        PatternOutputLayer::CircularMarks { .. } => Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "command targets an output without a mark-prototype configuration",
        )),
    }
}

fn validate_output_orientation(
    definition: &PatternDefinition,
    orientation: &MarkOrientation,
) -> Result<(), ValidationError> {
    match orientation {
        MarkOrientation::Fixed => Ok(()),
        MarkOrientation::GuideTangent { dimension_id }
        | MarkOrientation::GuideNormal { dimension_id }
            if definition.mechanisms.iter().any(|mechanism| {
                matches!(mechanism, PatternMechanism::StraightGuideDimensions { dimensions, .. } if dimensions.iter().any(|dimension| dimension.id == *dimension_id))
            }) =>
        {
            Ok(())
        }
        _ => Err(ValidationError::new(
            "pattern_definitions.output_layers.orientation",
            "orientation is incompatible with definition dimensions",
        )),
    }
}

fn definition_edit_invalidation(edit: &PatternDefinitionEdit) -> InvalidationLevel {
    property_field_contract(edit.field_projection().field).invalidation
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
    if let PatternFamily::RandomSites {
        base_site_process_id,
        density_modulation_id,
        exclusion_id,
        site_product_id,
    } = definition.family
    {
        return validate_random_site_definition(
            definition,
            base_site_process_id,
            density_modulation_id,
            exclusion_id,
            site_product_id,
        );
    }
    let PatternFamily::GuideIntersections {
        guide_mechanism_id,
        site_mechanism_id: root_site_id,
    } = definition.family
    else {
        unreachable!("all pattern families are handled above");
    };
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

fn validate_random_site_definition(
    definition: &PatternDefinition,
    base_id: PatternMechanismId,
    modulation_id: PatternMechanismId,
    exclusion_id: PatternMechanismId,
    site_id: PatternMechanismId,
) -> Result<(), ValidationError> {
    let [
        PatternMechanism::RandomSiteProcess {
            id: declared_base,
            character,
            ..
        },
        PatternMechanism::SiteDensityModulation {
            id: declared_modulation,
            base_site_process_id,
            modulation,
        },
        PatternMechanism::SiteExclusion {
            id: declared_exclusion,
            density_modulation_id,
            policy,
        },
        PatternMechanism::RandomSiteProduct {
            id: declared_site,
            exclusion_id: product_exclusion_id,
            maximum_attempts,
            maximum_neighbor_checks,
        },
    ] = definition.mechanisms.as_slice()
    else {
        return Err(ValidationError::new(
            "pattern_definitions.family",
            "random-site family requires base, modulation, exclusion, and site product in stored order",
        ));
    };
    if *declared_base != base_id
        || *declared_modulation != modulation_id
        || *declared_exclusion != exclusion_id
        || *declared_site != site_id
        || *base_site_process_id != base_id
        || *density_modulation_id != modulation_id
        || *product_exclusion_id != exclusion_id
    {
        return Err(ValidationError::new(
            "pattern_definitions.family",
            "random-site family root must reference its ordered mechanism chain",
        ));
    }
    validate_random_character(character)?;
    validate_site_density_modulation(modulation)?;
    validate_site_exclusion(policy)?;
    if *maximum_attempts == 0 {
        return Err(ValidationError::new(
            "pattern_definitions.mechanisms.random_sites.maximum_attempts",
            "random-site maximum attempts must be nonzero",
        ));
    }
    if *maximum_neighbor_checks == 0 {
        return Err(ValidationError::new(
            "pattern_definitions.mechanisms.random_sites.maximum_neighbor_checks",
            "random-site maximum neighbor checks must be nonzero",
        ));
    }
    let [
        PatternOutputLayer::MarkPrototype {
            site_mechanism_id,
            prototype: MarkPrototype::Circle,
            orientation: MarkOrientation::Fixed,
            ..
        },
    ] = definition.output_layers.as_slice()
    else {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "random-site products require exactly one fixed circle mark layer",
        ));
    };
    if *site_mechanism_id != site_id {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers.site_mechanism_id",
            "output layer must consume the declared random-site product",
        ));
    }
    Ok(())
}

fn validate_random_character(character: &RandomSiteCharacter) -> Result<(), ValidationError> {
    match character {
        RandomSiteCharacter::RawUniform => Ok(()),
        RandomSiteCharacter::Even {
            minimum_center_distance,
        } => validate_positive_finite(
            *minimum_center_distance,
            "pattern_definitions.mechanisms.random_sites.minimum_center_distance",
        ),
        RandomSiteCharacter::Clustered {
            cluster_density,
            cluster_spread,
            cluster_strength,
        } => {
            validate_positive_finite(
                *cluster_density,
                "pattern_definitions.mechanisms.random_sites.cluster_density",
            )?;
            validate_positive_finite(
                *cluster_spread,
                "pattern_definitions.mechanisms.random_sites.cluster_spread",
            )?;
            validate_unit_component(
                *cluster_strength,
                "pattern_definitions.mechanisms.random_sites.cluster_strength",
            )
        }
    }
}

fn validate_site_density_modulation(
    modulation: &SiteDensityModulation,
) -> Result<(), ValidationError> {
    match modulation {
        SiteDensityModulation::Uniform => Ok(()),
        SiteDensityModulation::ArtworkWeighted {
            mapping,
            strength,
            response,
        } => {
            validate_source_mapping(*mapping)?;
            validate_unit_component(
                *strength,
                "pattern_definitions.mechanisms.site_density.strength",
            )?;
            match response {
                ArtworkWeightResponse::Linear | ArtworkWeightResponse::Smoothstep => Ok(()),
            }
        }
    }
}

fn validate_site_exclusion(policy: &SiteExclusionPolicy) -> Result<(), ValidationError> {
    match policy {
        SiteExclusionPolicy::None => Ok(()),
        SiteExclusionPolicy::MinimumCenterDistance { minimum } => validate_positive_finite(
            *minimum,
            "pattern_definitions.mechanisms.site_exclusion.minimum",
        ),
        SiteExclusionPolicy::VisibleMarkMargin { margin, .. } => validate_nonnegative_finite(
            *margin,
            "pattern_definitions.mechanisms.site_exclusion.margin",
        ),
    }
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

fn derive_density_axis(
    canvas: &CanvasSpec,
    authoritative_value: f64,
    edited_axis: DensityEditedAxis,
) -> Result<f64, ValidationError> {
    validate_positive_finite(authoritative_value, "channel.pattern.layout.density")?;
    validate_positive_finite(canvas.width, "canvas.width")?;
    validate_positive_finite(canvas.height, "canvas.height")?;
    let result = match edited_axis {
        DensityEditedAxis::AcrossX => authoritative_value * canvas.height / canvas.width,
        DensityEditedAxis::AcrossY => authoritative_value * canvas.width / canvas.height,
    };
    validate_positive_finite(result, "channel.pattern.layout.density.derived_axis")?;
    Ok(result)
}

fn set_density_axis(
    density: &mut DensityMetric2D,
    canvas: &CanvasSpec,
    edited_axis: DensityEditedAxis,
    value: f64,
) -> Result<(), ValidationError> {
    let paired = if density.aspect_locked {
        Some(derive_density_axis(canvas, value, edited_axis)?)
    } else {
        None
    };
    match edited_axis {
        DensityEditedAxis::AcrossX => {
            density.across_x = value;
            if let Some(value) = paired {
                density.across_y = value;
            }
        }
        DensityEditedAxis::AcrossY => {
            density.across_y = value;
            if let Some(value) = paired {
                density.across_x = value;
            }
        }
    }
    Ok(())
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

fn set_color_component(color: &mut ColorValue, component: ColorComponent, value: f64) {
    match component {
        ColorComponent::Red => color.red = value,
        ColorComponent::Green => color.green = value,
        ColorComponent::Blue => color.blue = value,
        ColorComponent::Alpha => color.alpha = value,
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

/// The explicit density control which supplied a replacement value.  Keeping
/// this intent in the command (rather than inferring it from two values) is
/// what makes aspect-locked updates deterministic and replayable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DensityEditedAxis {
    AcrossX,
    AcrossY,
}

/// The explicit translation coordinate supplied by a channel-frame edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslationEditedAxis {
    X,
    Y,
}

/// A compact, schema-only capability vocabulary.  It deliberately contains
/// neither current values nor UI policy: callers read those from `Document`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropertyFieldId {
    SourceReference,
    DensityAcrossX,
    DensityAcrossY,
    DensityAspectLocked,
    RotationDegrees,
    TranslationX,
    TranslationY,
    MarkMinimumSize,
    MarkMaximumSize,
    LegacyMappingComponent,
    LegacyMappingPlacement,
    ModeledMappingComponent,
    ModeledMappingPlacement,
    ModeledMappingInverted,
    ModeledMappingGain,
    ModeledMappingBias,
    Paint,
    ColorRed,
    ColorGreen,
    ColorBlue,
    ColorAlpha,
    Opacity,
    Visibility,
    DefinitionSelection,
    CoverageGuardSteps,
    CoverageMaximumSupportRadius,
    GuideBaselineAngle,
    GuidePhase,
    GuideSpacingMultiplier,
    IntersectionDimensions,
    IntersectionMergeEpsilon,
    AlongGuideDimensions,
    AlongGuideIntervalMultiplier,
    AlongGuidePhase,
    RandomCharacter,
    RandomEvenMinimumCenterDistance,
    RandomClusterDensity,
    RandomClusterSpread,
    RandomClusterStrength,
    RandomSeed,
    RandomDensityModulation,
    ArtworkWeightMappingComponent,
    ArtworkWeightMappingPlacement,
    ArtworkWeightMappingInverted,
    ArtworkWeightMappingGain,
    ArtworkWeightMappingBias,
    ArtworkWeightStrength,
    ArtworkWeightResponse,
    RandomExclusion,
    ExclusionMinimumCenterDistance,
    VisibleMarkMargin,
    VisibleMarkSizingPolicy,
    RandomMaximumAttempts,
    RandomMaximumNeighborChecks,
    OutputSiteProduct,
    OutputPrototype,
    OutputOrientation,
    OutputOrientationDimension,
}

/// The authoritative descriptor order.  Keeping this list beside the field
/// enum makes missing or duplicate command-facing schema fields testable
/// without deriving any ordering from frontend state or allocation order.
pub const PROPERTY_FIELD_IDS: &[PropertyFieldId] = &[
    PropertyFieldId::SourceReference,
    PropertyFieldId::DensityAcrossX,
    PropertyFieldId::DensityAcrossY,
    PropertyFieldId::DensityAspectLocked,
    PropertyFieldId::RotationDegrees,
    PropertyFieldId::TranslationX,
    PropertyFieldId::TranslationY,
    PropertyFieldId::MarkMinimumSize,
    PropertyFieldId::MarkMaximumSize,
    PropertyFieldId::LegacyMappingComponent,
    PropertyFieldId::LegacyMappingPlacement,
    PropertyFieldId::ModeledMappingComponent,
    PropertyFieldId::ModeledMappingPlacement,
    PropertyFieldId::ModeledMappingInverted,
    PropertyFieldId::ModeledMappingGain,
    PropertyFieldId::ModeledMappingBias,
    PropertyFieldId::Paint,
    PropertyFieldId::ColorRed,
    PropertyFieldId::ColorGreen,
    PropertyFieldId::ColorBlue,
    PropertyFieldId::ColorAlpha,
    PropertyFieldId::Opacity,
    PropertyFieldId::Visibility,
    PropertyFieldId::DefinitionSelection,
    PropertyFieldId::CoverageGuardSteps,
    PropertyFieldId::CoverageMaximumSupportRadius,
    PropertyFieldId::GuideBaselineAngle,
    PropertyFieldId::GuidePhase,
    PropertyFieldId::GuideSpacingMultiplier,
    PropertyFieldId::IntersectionDimensions,
    PropertyFieldId::IntersectionMergeEpsilon,
    PropertyFieldId::AlongGuideDimensions,
    PropertyFieldId::AlongGuideIntervalMultiplier,
    PropertyFieldId::AlongGuidePhase,
    PropertyFieldId::RandomCharacter,
    PropertyFieldId::RandomEvenMinimumCenterDistance,
    PropertyFieldId::RandomClusterDensity,
    PropertyFieldId::RandomClusterSpread,
    PropertyFieldId::RandomClusterStrength,
    PropertyFieldId::RandomSeed,
    PropertyFieldId::RandomDensityModulation,
    PropertyFieldId::ArtworkWeightMappingComponent,
    PropertyFieldId::ArtworkWeightMappingPlacement,
    PropertyFieldId::ArtworkWeightMappingInverted,
    PropertyFieldId::ArtworkWeightMappingGain,
    PropertyFieldId::ArtworkWeightMappingBias,
    PropertyFieldId::ArtworkWeightStrength,
    PropertyFieldId::ArtworkWeightResponse,
    PropertyFieldId::RandomExclusion,
    PropertyFieldId::ExclusionMinimumCenterDistance,
    PropertyFieldId::VisibleMarkMargin,
    PropertyFieldId::VisibleMarkSizingPolicy,
    PropertyFieldId::RandomMaximumAttempts,
    PropertyFieldId::RandomMaximumNeighborChecks,
    PropertyFieldId::OutputSiteProduct,
    PropertyFieldId::OutputPrototype,
    PropertyFieldId::OutputOrientation,
    PropertyFieldId::OutputOrientationDimension,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropertyTarget {
    Document,
    Channel(ChannelId),
    Definition(PatternDefinitionId),
    Mechanism(PatternDefinitionId, PatternMechanismId),
    OutputLayer(PatternDefinitionId, PatternOutputLayerId),
    GuideDimension(PatternDefinitionId, PatternMechanismId, GuideDimensionId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyValueKind {
    FiniteF64,
    U32,
    Boolean,
    StableIdReference,
    EnumChoice,
}

/// Stable finite discriminants for descriptor enum fields. Payload values are
/// addressed by their own descriptors and never encoded as display strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyEnumChoice {
    SourceMappingComponent(SourceMappingComponent),
    SourcePlacement(SourcePlacement),
    Paint(PaintKind),
    RandomCharacter(RandomCharacterKind),
    DensityModulation(DensityModulationKind),
    ArtworkWeightResponse(ArtworkWeightResponse),
    Exclusion(ExclusionKind),
    VisibleMarkSizingPolicy(VisibleMarkSizingPolicy),
    MarkPrototype(MarkPrototypeKind),
    MarkOrientation(MarkOrientationKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintKind {
    Solid,
    SampledSource,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RandomCharacterKind {
    RawUniform,
    Even,
    Clustered,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DensityModulationKind {
    Uniform,
    ArtworkWeighted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExclusionKind {
    None,
    MinimumCenterDistance,
    VisibleMarkMargin,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkPrototypeKind {
    Circle,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkOrientationKind {
    Fixed,
    GuideTangent,
    GuideNormal,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropertyBounds {
    pub minimum: Option<f64>,
    pub minimum_inclusive: bool,
    pub maximum: Option<f64>,
    pub maximum_inclusive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyUnit {
    None,
    Density,
    Degrees,
    Phase,
    DocumentDistance,
    NormalizedComponent,
    Count,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyDependency {
    Always,
    ModeledChannel,
    SolidPaint,
    SampledPaint,
    StraightGuideDimension,
    IntersectionProduct,
    AlongGuideProduct,
    RandomProcess,
    EvenRandomProcess,
    ClusteredRandomProcess,
    ArtworkWeightedDensity,
    MinimumCenterExclusion,
    VisibleMarkExclusion,
    MarkPrototypeOutput,
    GuidedOutputOrientation,
}

/// The schema predicate that decides whether an otherwise stable field is
/// active.  This is capability metadata, not a stored value or UI policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyApplicability {
    Always,
    ModeledChannel,
    SolidPaint,
    StraightGuideDimension,
    IntersectionProduct,
    AlongGuideProduct,
    RandomProcess,
    EvenRandomProcess,
    ClusteredRandomProcess,
    ArtworkWeightedDensity,
    MinimumCenterExclusion,
    VisibleMarkExclusion,
    MarkPrototypeOutput,
    GuidedOutputOrientation,
    CurrentPaint,
    CurrentDensityModulation,
    CurrentExclusion,
}

/// Schema constraint explanation only; this never calculates geometry or
/// captures a persisted value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructuralSupportConstraint {
    None,
    /// This field declares the definition-wide maximum support envelope that
    /// mark response and conservative visible-mark exclusion must respect.
    DefinesMaximumMarkSupportRadius,
    MarkResponseMustFitDefinitionMaximumSupport,
    VisibleMarkMarginUsesMaximumSupportRadius,
}

/// Validation shape for stable references. Collection fields carry their
/// cardinality here; exact ID existence, canonical stored order, and
/// duplicate rejection remain document-relative typed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyReferenceConstraint {
    NotReference,
    Singular,
    OrderedUniqueCollection {
        minimum_items: u8,
        maximum_items: u8,
    },
}

/// Choice resolution is explicit when legal choices depend on the active
/// channel model/role; all other enum fields use `PropertyFieldContract::choices`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyChoicePolicy {
    Static,
    ModelRolePaint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyCommandKind {
    SetSourceReference,
    SetDensityAxis,
    SetDensityAspectLock,
    SetRotation,
    SetTranslationAxis,
    SetMarkGeometryField,
    SetLegacyMappingField,
    SetModeledMappingField,
    SetPaint,
    SetColorComponent,
    SetOpacity,
    SetVisibility,
    RetargetDefinition,
    SetGuideBaselineAngle,
    SetGuidePhase,
    SetGuideSpacingMultiplier,
    SetIntersectionDimensions,
    SetIntersectionMergeEpsilon,
    SetAlongGuideDimensions,
    SetAlongGuideIntervalMultiplier,
    SetAlongGuidePhase,
    SetRandomCharacter,
    SetRandomSeed,
    SetRandomEvenMinimumCenterDistance,
    SetRandomClusterDensity,
    SetRandomClusterSpread,
    SetRandomClusterStrength,
    SetDensityModulationVariant,
    SetArtworkWeightMappingComponent,
    SetArtworkWeightMappingPlacement,
    SetArtworkWeightMappingInverted,
    SetArtworkWeightMappingGain,
    SetArtworkWeightMappingBias,
    SetArtworkWeightStrength,
    SetArtworkWeightResponse,
    SetExclusionVariant,
    SetExclusionMinimumCenterDistance,
    SetVisibleMarkMargin,
    SetVisibleMarkSizingPolicy,
    SetRandomMaximumAttempts,
    SetRandomMaximumNeighborChecks,
    SetOutputSiteProduct,
    SetOutputMarkPrototype,
    SetOutputOrientation,
    SetOutputOrientationDimension,
    SetCoverageGuardSteps,
    SetCoverageMaximumSupportRadius,
}

/// A deterministic read-only descriptor derived from the validated current
/// schema. `choices` uses stable discriminants, never localized labels.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyDescriptor {
    pub field: PropertyFieldId,
    pub target: PropertyTarget,
    pub value_kind: PropertyValueKind,
    pub choices: &'static [PropertyEnumChoice],
    pub bounds: Option<PropertyBounds>,
    pub unit: PropertyUnit,
    pub dependency: PropertyDependency,
    pub invalidation: InvalidationLevel,
    pub copy_on_edit_escalates_to_family: bool,
    pub structural_support: StructuralSupportConstraint,
    pub reference_constraint: PropertyReferenceConstraint,
    pub choice_policy: PropertyChoicePolicy,
}

/// An immutable typed value paired with one active descriptor.  This is a
/// read boundary for frontends and tooling, not a second document shape.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyCurrentValue {
    pub descriptor: PropertyDescriptor,
    pub value: PropertyCurrentValueKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PropertyCurrentValueKind {
    FiniteF64(f64),
    U32(u32),
    Boolean(bool),
    EnumChoice(PropertyEnumChoice),
    Reference(PropertyReferenceValue),
    ReferenceCollection(Vec<PropertyReferenceValue>),
}

/// Stable identifiers remain typed at the read boundary; presentation code
/// may format them, but cannot turn them into positional aliases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertyReferenceValue {
    Source(SourceReference),
    Definition(PatternDefinitionId),
    Mechanism(PatternMechanismId),
    GuideDimension(GuideDimensionId),
}

/// A non-persisted, immutable proposal for the complete payload needed when a
/// selector changes to a payload-bearing alternative.  It is intentionally
/// separate from active property descriptors: these fields are transition
/// inputs, not current document capabilities.
#[derive(Clone, Debug, PartialEq)]
pub struct VariantTransitionDraft {
    selector: PropertyDescriptor,
    /// Private immutable structural base. Like command bases, this is
    /// lifecycle-only stale detection and never document/persistence state.
    base_definition: PatternDefinition,
    base_choice: PropertyEnumChoice,
    choice: PropertyEnumChoice,
    fields: Vec<VariantTransitionField>,
}

impl VariantTransitionDraft {
    pub fn selector(&self) -> &PropertyDescriptor {
        &self.selector
    }

    pub fn choice(&self) -> PropertyEnumChoice {
        self.choice
    }

    pub fn fields(&self) -> &[VariantTransitionField] {
        &self.fields
    }

    /// Returns a new draft after validating the complete update set.  The
    /// update list is intentionally keyed by stable field and target, never
    /// list index; duplicate, missing, or foreign updates are rejected.
    pub fn with_updates(
        &self,
        updates: &[VariantTransitionFieldUpdate],
    ) -> Result<Self, ValidationError> {
        let mut next = self.clone();
        let mut seen = HashSet::new();
        for update in updates {
            let key = (update.field, update.target);
            if !seen.insert(key) {
                return Err(ValidationError::new(
                    "transition_draft.update",
                    "transition draft contains a duplicate field update",
                ));
            }
            let field = next
                .fields
                .iter_mut()
                .find(|field| field.field == update.field && field.target == update.target)
                .ok_or_else(|| {
                    ValidationError::new(
                        "transition_draft.update",
                        "transition draft update targets an absent field",
                    )
                })?;
            validate_transition_field_value(field, &update.value)?;
            field.value = update.value.clone();
        }
        Ok(next)
    }

    /// Validates this transient draft against the current immutable document
    /// and returns the existing typed edit.  It never mutates a document or
    /// history owner.
    pub fn finalize(&self, document: &Document) -> Result<PatternDefinitionEdit, ValidationError> {
        validate_transition_draft_shape(self, document)?;
        let edit = transition_draft_edit(self)?;
        let definition_id =
            transition_definition_id(self.selector.target).expect("validated selector target");
        let definition = document.definition(definition_id).ok_or_else(|| {
            ValidationError::new(
                "transition_draft.target",
                "transition definition is missing",
            )
        })?;
        validate_definition_edit(definition, &edit)?;
        let mut candidate = definition.clone();
        apply_definition_edit(&mut candidate, &edit);
        if &candidate == definition {
            return Err(ValidationError::new(
                "transition_draft.confirm",
                "transition draft is a semantic no-op",
            ));
        }
        validate_definition(&candidate)?;
        Ok(edit)
    }
}

/// One complete typed transition payload field. `contract` is copied from the
/// immutable schema field contract, never from a frontend policy or document value.
#[derive(Clone, Debug, PartialEq)]
pub struct VariantTransitionField {
    pub field: PropertyFieldId,
    pub target: PropertyTarget,
    pub contract: PropertyFieldContract,
    pub value: VariantTransitionValue,
    pub reference_choices: Vec<PropertyReferenceValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VariantTransitionValue {
    FiniteF64(f64),
    U32(u32),
    Boolean(bool),
    EnumChoice(PropertyEnumChoice),
    /// `None` is an explicit incomplete required reference, used only where
    /// transition policy cannot choose a stable identity on the user's behalf.
    StableReference(Option<PropertyReferenceValue>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct VariantTransitionFieldUpdate {
    pub field: PropertyFieldId,
    pub target: PropertyTarget,
    pub value: VariantTransitionValue,
}

enum ChannelPropertyState<'a> {
    Legacy(&'a ChannelState),
    Modeled(&'a ModeledChannelState),
}

impl ChannelPropertyState<'_> {
    fn layout(&self) -> &ChannelPatternLayout {
        match self {
            Self::Legacy(channel) => &channel.layout,
            Self::Modeled(channel) => &channel.layout,
        }
    }
    fn mark(&self) -> &MarkGeometryResponse {
        match self {
            Self::Legacy(channel) => &channel.mark_geometry_response,
            Self::Modeled(channel) => &channel.mark_geometry_response,
        }
    }
    fn definition_id(&self) -> PatternDefinitionId {
        match self {
            Self::Legacy(channel) => channel.pattern_definition_id,
            Self::Modeled(channel) => channel.pattern_definition_id,
        }
    }
    fn opacity(&self) -> f64 {
        match self {
            Self::Legacy(channel) => channel.appearance.opacity,
            Self::Modeled(channel) => channel.opacity,
        }
    }
    fn visible(&self) -> bool {
        match self {
            Self::Legacy(channel) => channel.appearance.visible,
            Self::Modeled(channel) => channel.visible,
        }
    }
    fn color(&self) -> &ColorValue {
        match self {
            Self::Legacy(channel) => &channel.appearance.color,
            Self::Modeled(channel) => match &channel.paint {
                ChannelPaint::Solid(color) => color,
                ChannelPaint::SampledSource => {
                    unreachable!("sampled paint has no color descriptors")
                }
            },
        }
    }
    fn legacy_mapping(&self) -> Option<ChannelSourceMapping> {
        match self {
            Self::Legacy(channel) => Some(channel.source_mapping),
            Self::Modeled(_) => None,
        }
    }
    fn mapping(&self) -> Option<SourceMapping> {
        match self {
            Self::Legacy(_) => None,
            Self::Modeled(channel) => Some(channel.mapping),
        }
    }
    fn paint(&self) -> Option<&ChannelPaint> {
        match self {
            Self::Legacy(_) => None,
            Self::Modeled(channel) => Some(&channel.paint),
        }
    }
}

fn property_value_for_mechanism(
    field: PropertyFieldId,
    mechanism: &PatternMechanism,
) -> PropertyCurrentValueKind {
    match (field, mechanism) {
        (
            PropertyFieldId::IntersectionDimensions,
            PatternMechanism::SelectedGuideIntersections { dimensions, .. },
        ) => PropertyCurrentValueKind::ReferenceCollection(
            dimensions
                .iter()
                .copied()
                .map(PropertyReferenceValue::GuideDimension)
                .collect(),
        ),
        (
            PropertyFieldId::IntersectionMergeEpsilon,
            PatternMechanism::SelectedGuideIntersections { merge_epsilon, .. },
        ) => PropertyCurrentValueKind::FiniteF64(*merge_epsilon),
        (
            PropertyFieldId::AlongGuideDimensions,
            PatternMechanism::AlongGuideSites { dimensions, .. },
        ) => PropertyCurrentValueKind::ReferenceCollection(
            dimensions
                .iter()
                .copied()
                .map(PropertyReferenceValue::GuideDimension)
                .collect(),
        ),
        (
            PropertyFieldId::AlongGuideIntervalMultiplier,
            PatternMechanism::AlongGuideSites {
                interval_multiplier,
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*interval_multiplier),
        (PropertyFieldId::AlongGuidePhase, PatternMechanism::AlongGuideSites { phase, .. }) => {
            PropertyCurrentValueKind::FiniteF64(*phase)
        }
        (
            PropertyFieldId::RandomCharacter,
            PatternMechanism::RandomSiteProcess { character, .. },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::RandomCharacter(
            match character {
                RandomSiteCharacter::RawUniform => RandomCharacterKind::RawUniform,
                RandomSiteCharacter::Even { .. } => RandomCharacterKind::Even,
                RandomSiteCharacter::Clustered { .. } => RandomCharacterKind::Clustered,
            },
        )),
        (PropertyFieldId::RandomSeed, PatternMechanism::RandomSiteProcess { seed, .. }) => {
            PropertyCurrentValueKind::U32(*seed)
        }
        (
            PropertyFieldId::RandomEvenMinimumCenterDistance,
            PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Even {
                        minimum_center_distance,
                    },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*minimum_center_distance),
        (
            PropertyFieldId::RandomClusterDensity,
            PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Clustered {
                        cluster_density, ..
                    },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*cluster_density),
        (
            PropertyFieldId::RandomClusterSpread,
            PatternMechanism::RandomSiteProcess {
                character: RandomSiteCharacter::Clustered { cluster_spread, .. },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*cluster_spread),
        (
            PropertyFieldId::RandomClusterStrength,
            PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Clustered {
                        cluster_strength, ..
                    },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*cluster_strength),
        (
            PropertyFieldId::RandomDensityModulation,
            PatternMechanism::SiteDensityModulation { modulation, .. },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::DensityModulation(
            match modulation {
                SiteDensityModulation::Uniform => DensityModulationKind::Uniform,
                SiteDensityModulation::ArtworkWeighted { .. } => {
                    DensityModulationKind::ArtworkWeighted
                }
            },
        )),
        (
            PropertyFieldId::ArtworkWeightMappingComponent,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { mapping, .. },
                ..
            },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::SourceMappingComponent(
            mapping.component,
        )),
        (
            PropertyFieldId::ArtworkWeightMappingPlacement,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { mapping, .. },
                ..
            },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::SourcePlacement(
            mapping.placement,
        )),
        (
            PropertyFieldId::ArtworkWeightMappingInverted,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { mapping, .. },
                ..
            },
        ) => PropertyCurrentValueKind::Boolean(mapping.inverted),
        (
            PropertyFieldId::ArtworkWeightMappingGain,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { mapping, .. },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(mapping.gain),
        (
            PropertyFieldId::ArtworkWeightMappingBias,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { mapping, .. },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(mapping.bias),
        (
            PropertyFieldId::ArtworkWeightStrength,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { strength, .. },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*strength),
        (
            PropertyFieldId::ArtworkWeightResponse,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { response, .. },
                ..
            },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::ArtworkWeightResponse(
            *response,
        )),
        (PropertyFieldId::RandomExclusion, PatternMechanism::SiteExclusion { policy, .. }) => {
            PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::Exclusion(match policy {
                SiteExclusionPolicy::None => ExclusionKind::None,
                SiteExclusionPolicy::MinimumCenterDistance { .. } => {
                    ExclusionKind::MinimumCenterDistance
                }
                SiteExclusionPolicy::VisibleMarkMargin { .. } => ExclusionKind::VisibleMarkMargin,
            }))
        }
        (
            PropertyFieldId::ExclusionMinimumCenterDistance,
            PatternMechanism::SiteExclusion {
                policy: SiteExclusionPolicy::MinimumCenterDistance { minimum },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*minimum),
        (
            PropertyFieldId::VisibleMarkMargin,
            PatternMechanism::SiteExclusion {
                policy: SiteExclusionPolicy::VisibleMarkMargin { margin, .. },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*margin),
        (
            PropertyFieldId::VisibleMarkSizingPolicy,
            PatternMechanism::SiteExclusion {
                policy: SiteExclusionPolicy::VisibleMarkMargin { sizing, .. },
                ..
            },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::VisibleMarkSizingPolicy(
            *sizing,
        )),
        (
            PropertyFieldId::RandomMaximumAttempts,
            PatternMechanism::RandomSiteProduct {
                maximum_attempts, ..
            },
        ) => PropertyCurrentValueKind::U32(*maximum_attempts),
        (
            PropertyFieldId::RandomMaximumNeighborChecks,
            PatternMechanism::RandomSiteProduct {
                maximum_neighbor_checks,
                ..
            },
        ) => PropertyCurrentValueKind::U32(*maximum_neighbor_checks),
        _ => unreachable!("active mechanism descriptor field"),
    }
}

fn transition_definition_id(target: PropertyTarget) -> Option<PatternDefinitionId> {
    match target {
        PropertyTarget::Mechanism(definition_id, _)
        | PropertyTarget::OutputLayer(definition_id, _) => Some(definition_id),
        _ => None,
    }
}

fn transition_field(
    field: PropertyFieldId,
    target: PropertyTarget,
    value: VariantTransitionValue,
    reference_choices: Vec<PropertyReferenceValue>,
) -> VariantTransitionField {
    VariantTransitionField {
        field,
        target,
        contract: property_field_contract(field),
        value,
        reference_choices,
    }
}

fn transition_fields_for(
    document: &Document,
    selector: &PropertyDescriptor,
    base_choice: PropertyEnumChoice,
    choice: PropertyEnumChoice,
) -> Result<Vec<VariantTransitionField>, ValidationError> {
    match (selector.field, selector.target, base_choice, choice) {
        (
            PropertyFieldId::RandomCharacter,
            PropertyTarget::Mechanism(definition_id, mechanism_id),
            PropertyEnumChoice::RandomCharacter(base),
            PropertyEnumChoice::RandomCharacter(choice),
        ) => random_transition_fields(document, definition_id, mechanism_id, base, choice),
        (
            PropertyFieldId::RandomDensityModulation,
            PropertyTarget::Mechanism(definition_id, mechanism_id),
            PropertyEnumChoice::DensityModulation(base),
            PropertyEnumChoice::DensityModulation(choice),
        ) => modulation_transition_fields(document, definition_id, mechanism_id, base, choice),
        (
            PropertyFieldId::RandomExclusion,
            PropertyTarget::Mechanism(definition_id, mechanism_id),
            PropertyEnumChoice::Exclusion(base),
            PropertyEnumChoice::Exclusion(choice),
        ) => exclusion_transition_fields(document, definition_id, mechanism_id, base, choice),
        (
            PropertyFieldId::OutputOrientation,
            PropertyTarget::OutputLayer(definition_id, output_layer_id),
            PropertyEnumChoice::MarkOrientation(base),
            PropertyEnumChoice::MarkOrientation(choice),
        ) => orientation_transition_fields(document, definition_id, output_layer_id, base, choice),
        _ => Err(ValidationError::new(
            "transition_draft.selector",
            "selector target or choice does not support compound transition drafts",
        )),
    }
}

fn transition_mechanism(
    document: &Document,
    definition_id: PatternDefinitionId,
    mechanism_id: PatternMechanismId,
) -> Result<&PatternMechanism, ValidationError> {
    document
        .definition(definition_id)
        .and_then(|definition| {
            definition
                .mechanisms
                .iter()
                .find(|mechanism| mechanism.id() == mechanism_id)
        })
        .ok_or_else(|| {
            ValidationError::new("transition_draft.target", "transition mechanism is missing")
        })
}

fn random_transition_fields(
    document: &Document,
    definition_id: PatternDefinitionId,
    mechanism_id: PatternMechanismId,
    base: RandomCharacterKind,
    choice: RandomCharacterKind,
) -> Result<Vec<VariantTransitionField>, ValidationError> {
    let mechanism = transition_mechanism(document, definition_id, mechanism_id)?;
    let PatternMechanism::RandomSiteProcess { character, .. } = mechanism else {
        return Err(ValidationError::new(
            "transition_draft.target",
            "selector is not a random process",
        ));
    };
    let target = PropertyTarget::Mechanism(definition_id, mechanism_id);
    match choice {
        RandomCharacterKind::RawUniform => Ok(Vec::new()),
        RandomCharacterKind::Even => {
            let value = if base == choice {
                match character {
                    RandomSiteCharacter::Even {
                        minimum_center_distance,
                    } => *minimum_center_distance,
                    _ => unreachable!("base selector is current"),
                }
            } else {
                1.0
            };
            Ok(vec![transition_field(
                PropertyFieldId::RandomEvenMinimumCenterDistance,
                target,
                VariantTransitionValue::FiniteF64(value),
                Vec::new(),
            )])
        }
        RandomCharacterKind::Clustered => {
            let (density, spread, strength) = if base == choice {
                match character {
                    RandomSiteCharacter::Clustered {
                        cluster_density,
                        cluster_spread,
                        cluster_strength,
                    } => (*cluster_density, *cluster_spread, *cluster_strength),
                    _ => unreachable!("base selector is current"),
                }
            } else {
                (1.0, 1.0, 1.0)
            };
            Ok(vec![
                transition_field(
                    PropertyFieldId::RandomClusterDensity,
                    target,
                    VariantTransitionValue::FiniteF64(density),
                    Vec::new(),
                ),
                transition_field(
                    PropertyFieldId::RandomClusterSpread,
                    target,
                    VariantTransitionValue::FiniteF64(spread),
                    Vec::new(),
                ),
                transition_field(
                    PropertyFieldId::RandomClusterStrength,
                    target,
                    VariantTransitionValue::FiniteF64(strength),
                    Vec::new(),
                ),
            ])
        }
    }
}

fn modulation_transition_fields(
    document: &Document,
    definition_id: PatternDefinitionId,
    mechanism_id: PatternMechanismId,
    base: DensityModulationKind,
    choice: DensityModulationKind,
) -> Result<Vec<VariantTransitionField>, ValidationError> {
    let mechanism = transition_mechanism(document, definition_id, mechanism_id)?;
    let PatternMechanism::SiteDensityModulation { modulation, .. } = mechanism else {
        return Err(ValidationError::new(
            "transition_draft.target",
            "selector is not density modulation",
        ));
    };
    if choice == DensityModulationKind::Uniform {
        return Ok(Vec::new());
    }
    let target = PropertyTarget::Mechanism(definition_id, mechanism_id);
    let (mapping, strength, response) = if base == choice {
        match modulation {
            SiteDensityModulation::ArtworkWeighted {
                mapping,
                strength,
                response,
            } => (*mapping, *strength, *response),
            _ => unreachable!("base selector is current"),
        }
    } else {
        (
            SourceMapping::canonical(SourceMappingComponent::Luminance),
            1.0,
            ArtworkWeightResponse::Linear,
        )
    };
    Ok(vec![
        transition_field(
            PropertyFieldId::ArtworkWeightMappingComponent,
            target,
            VariantTransitionValue::EnumChoice(PropertyEnumChoice::SourceMappingComponent(
                mapping.component,
            )),
            Vec::new(),
        ),
        transition_field(
            PropertyFieldId::ArtworkWeightMappingPlacement,
            target,
            VariantTransitionValue::EnumChoice(PropertyEnumChoice::SourcePlacement(
                mapping.placement,
            )),
            Vec::new(),
        ),
        transition_field(
            PropertyFieldId::ArtworkWeightMappingInverted,
            target,
            VariantTransitionValue::Boolean(mapping.inverted),
            Vec::new(),
        ),
        transition_field(
            PropertyFieldId::ArtworkWeightMappingGain,
            target,
            VariantTransitionValue::FiniteF64(mapping.gain),
            Vec::new(),
        ),
        transition_field(
            PropertyFieldId::ArtworkWeightMappingBias,
            target,
            VariantTransitionValue::FiniteF64(mapping.bias),
            Vec::new(),
        ),
        transition_field(
            PropertyFieldId::ArtworkWeightStrength,
            target,
            VariantTransitionValue::FiniteF64(strength),
            Vec::new(),
        ),
        transition_field(
            PropertyFieldId::ArtworkWeightResponse,
            target,
            VariantTransitionValue::EnumChoice(PropertyEnumChoice::ArtworkWeightResponse(response)),
            Vec::new(),
        ),
    ])
}

fn exclusion_transition_fields(
    document: &Document,
    definition_id: PatternDefinitionId,
    mechanism_id: PatternMechanismId,
    base: ExclusionKind,
    choice: ExclusionKind,
) -> Result<Vec<VariantTransitionField>, ValidationError> {
    let mechanism = transition_mechanism(document, definition_id, mechanism_id)?;
    let PatternMechanism::SiteExclusion { policy, .. } = mechanism else {
        return Err(ValidationError::new(
            "transition_draft.target",
            "selector is not exclusion policy",
        ));
    };
    let target = PropertyTarget::Mechanism(definition_id, mechanism_id);
    match choice {
        ExclusionKind::None => Ok(Vec::new()),
        ExclusionKind::MinimumCenterDistance => {
            let minimum = if base == choice {
                match policy {
                    SiteExclusionPolicy::MinimumCenterDistance { minimum } => *minimum,
                    _ => unreachable!("base selector is current"),
                }
            } else {
                1.0
            };
            Ok(vec![transition_field(
                PropertyFieldId::ExclusionMinimumCenterDistance,
                target,
                VariantTransitionValue::FiniteF64(minimum),
                Vec::new(),
            )])
        }
        ExclusionKind::VisibleMarkMargin => {
            let (margin, sizing) = if base == choice {
                match policy {
                    SiteExclusionPolicy::VisibleMarkMargin { margin, sizing } => (*margin, *sizing),
                    _ => unreachable!("base selector is current"),
                }
            } else {
                (0.0, VisibleMarkSizingPolicy::MaximumSupportRadius)
            };
            Ok(vec![
                transition_field(
                    PropertyFieldId::VisibleMarkMargin,
                    target,
                    VariantTransitionValue::FiniteF64(margin),
                    Vec::new(),
                ),
                transition_field(
                    PropertyFieldId::VisibleMarkSizingPolicy,
                    target,
                    VariantTransitionValue::EnumChoice(
                        PropertyEnumChoice::VisibleMarkSizingPolicy(sizing),
                    ),
                    Vec::new(),
                ),
            ])
        }
    }
}

fn orientation_transition_fields(
    document: &Document,
    definition_id: PatternDefinitionId,
    output_layer_id: PatternOutputLayerId,
    base: MarkOrientationKind,
    choice: MarkOrientationKind,
) -> Result<Vec<VariantTransitionField>, ValidationError> {
    let definition = document.definition(definition_id).ok_or_else(|| {
        ValidationError::new(
            "transition_draft.target",
            "transition definition is missing",
        )
    })?;
    let layer = definition
        .output_layers
        .iter()
        .find(|layer| layer.id() == output_layer_id)
        .ok_or_else(|| {
            ValidationError::new(
                "transition_draft.target",
                "transition output layer is missing",
            )
        })?;
    if !matches!(layer, PatternOutputLayer::MarkPrototype { .. }) {
        return Err(ValidationError::new(
            "transition_draft.target",
            "selector is not a mark-prototype output",
        ));
    }
    if choice == MarkOrientationKind::Fixed {
        return Ok(Vec::new());
    }
    let reference_choices = definition
        .mechanisms
        .iter()
        .flat_map(|mechanism| match mechanism {
            PatternMechanism::StraightGuideDimensions { dimensions, .. } => dimensions
                .iter()
                .map(|dimension| PropertyReferenceValue::GuideDimension(dimension.id))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    let existing = if base == choice {
        match layer {
            PatternOutputLayer::MarkPrototype {
                orientation:
                    MarkOrientation::GuideTangent { dimension_id }
                    | MarkOrientation::GuideNormal { dimension_id },
                ..
            } => Some(PropertyReferenceValue::GuideDimension(*dimension_id)),
            _ => unreachable!("base selector is current"),
        }
    } else {
        None
    };
    Ok(vec![transition_field(
        PropertyFieldId::OutputOrientationDimension,
        PropertyTarget::OutputLayer(definition_id, output_layer_id),
        VariantTransitionValue::StableReference(existing),
        reference_choices,
    )])
}

fn validate_transition_field_value(
    field: &VariantTransitionField,
    value: &VariantTransitionValue,
) -> Result<(), ValidationError> {
    let projection = match (field.contract.value_kind, value) {
        (PropertyValueKind::FiniteF64, VariantTransitionValue::FiniteF64(value)) => {
            PropertyFieldValue::FiniteF64(*value)
        }
        (PropertyValueKind::U32, VariantTransitionValue::U32(value)) => {
            PropertyFieldValue::U32(*value)
        }
        (PropertyValueKind::Boolean, VariantTransitionValue::Boolean(value)) => {
            PropertyFieldValue::Boolean(*value)
        }
        (PropertyValueKind::EnumChoice, VariantTransitionValue::EnumChoice(value)) => {
            if !field.contract.choices.contains(value) {
                return Err(ValidationError::new(
                    "transition_draft.value",
                    "transition enum choice is unsupported",
                ));
            }
            PropertyFieldValue::EnumChoice(*value)
        }
        (
            PropertyValueKind::StableIdReference,
            VariantTransitionValue::StableReference(Some(reference)),
        ) => {
            if !field.reference_choices.contains(reference) {
                return Err(ValidationError::new(
                    "transition_draft.reference",
                    "transition reference is missing or incompatible",
                ));
            }
            PropertyFieldValue::StableIdReference
        }
        (PropertyValueKind::StableIdReference, VariantTransitionValue::StableReference(None)) => {
            return Err(ValidationError::new(
                "transition_draft.reference",
                "transition requires an explicit stable reference",
            ));
        }
        _ => {
            return Err(ValidationError::new(
                "transition_draft.value",
                "transition value has the wrong kind",
            ));
        }
    };
    validate_property_field_projection(PropertyCommandFieldProjection {
        field: field.field,
        value: projection,
    })
}

fn validate_transition_draft_shape(
    draft: &VariantTransitionDraft,
    document: &Document,
) -> Result<(), ValidationError> {
    let definition_id = transition_definition_id(draft.selector.target).ok_or_else(|| {
        ValidationError::new(
            "transition_draft.selector",
            "transition selector has no structural definition target",
        )
    })?;
    if document.definition(definition_id) != Some(&draft.base_definition) {
        return Err(ValidationError::new(
            "transition_draft.stale",
            "transition draft definition base is stale",
        ));
    }
    let active = document.property_descriptors();
    if !active.contains(&draft.selector) {
        return Err(ValidationError::new(
            "transition_draft.selector",
            "transition selector is inactive or stale",
        ));
    }
    let current = document
        .property_values()
        .into_iter()
        .find(|value| value.descriptor == draft.selector)
        .and_then(|value| match value.value {
            PropertyCurrentValueKind::EnumChoice(choice) => Some(choice),
            _ => None,
        })
        .ok_or_else(|| {
            ValidationError::new(
                "transition_draft.selector",
                "transition selector no longer has an enum value",
            )
        })?;
    if current != draft.base_choice {
        return Err(ValidationError::new(
            "transition_draft.stale",
            "transition draft base is stale",
        ));
    }
    let expected =
        transition_fields_for(document, &draft.selector, draft.base_choice, draft.choice)?;
    if expected.len() != draft.fields.len()
        || expected
            .iter()
            .zip(&draft.fields)
            .any(|(expected, actual)| {
                expected.field != actual.field
                    || expected.target != actual.target
                    || expected.contract != actual.contract
                    || expected.reference_choices != actual.reference_choices
            })
    {
        return Err(ValidationError::new(
            "transition_draft.fields",
            "transition draft field order or shape is invalid",
        ));
    }
    for field in &draft.fields {
        validate_transition_field_value(field, &field.value)?;
    }
    Ok(())
}

fn transition_value(
    draft: &VariantTransitionDraft,
    field: PropertyFieldId,
) -> Result<&VariantTransitionValue, ValidationError> {
    draft
        .fields
        .iter()
        .find(|candidate| candidate.field == field)
        .map(|field| &field.value)
        .ok_or_else(|| {
            ValidationError::new(
                "transition_draft.fields",
                "transition draft is missing a required field",
            )
        })
}

fn transition_f64(
    draft: &VariantTransitionDraft,
    field: PropertyFieldId,
) -> Result<f64, ValidationError> {
    match transition_value(draft, field)? {
        VariantTransitionValue::FiniteF64(value) => Ok(*value),
        _ => Err(ValidationError::new(
            "transition_draft.value",
            "transition field has the wrong kind",
        )),
    }
}

fn transition_choice(
    draft: &VariantTransitionDraft,
    field: PropertyFieldId,
) -> Result<PropertyEnumChoice, ValidationError> {
    match transition_value(draft, field)? {
        VariantTransitionValue::EnumChoice(value) => Ok(*value),
        _ => Err(ValidationError::new(
            "transition_draft.value",
            "transition field has the wrong kind",
        )),
    }
}

fn transition_reference(
    draft: &VariantTransitionDraft,
    field: PropertyFieldId,
) -> Result<PropertyReferenceValue, ValidationError> {
    match transition_value(draft, field)? {
        VariantTransitionValue::StableReference(Some(value)) => Ok(value.clone()),
        _ => Err(ValidationError::new(
            "transition_draft.reference",
            "transition requires an explicit stable reference",
        )),
    }
}

fn transition_draft_edit(
    draft: &VariantTransitionDraft,
) -> Result<PatternDefinitionEdit, ValidationError> {
    match (draft.selector.field, draft.selector.target, draft.choice) {
        (
            PropertyFieldId::RandomCharacter,
            PropertyTarget::Mechanism(_, mechanism_id),
            PropertyEnumChoice::RandomCharacter(choice),
        ) => {
            let character = match choice {
                RandomCharacterKind::RawUniform => RandomSiteCharacter::RawUniform,
                RandomCharacterKind::Even => RandomSiteCharacter::Even {
                    minimum_center_distance: transition_f64(
                        draft,
                        PropertyFieldId::RandomEvenMinimumCenterDistance,
                    )?,
                },
                RandomCharacterKind::Clustered => RandomSiteCharacter::Clustered {
                    cluster_density: transition_f64(draft, PropertyFieldId::RandomClusterDensity)?,
                    cluster_spread: transition_f64(draft, PropertyFieldId::RandomClusterSpread)?,
                    cluster_strength: transition_f64(
                        draft,
                        PropertyFieldId::RandomClusterStrength,
                    )?,
                },
            };
            Ok(PatternDefinitionEdit::SetRandomCharacter {
                mechanism_id,
                character,
            })
        }
        (
            PropertyFieldId::RandomDensityModulation,
            PropertyTarget::Mechanism(_, mechanism_id),
            PropertyEnumChoice::DensityModulation(choice),
        ) => {
            let modulation = match choice {
                DensityModulationKind::Uniform => SiteDensityModulation::Uniform,
                DensityModulationKind::ArtworkWeighted => {
                    let component = match transition_choice(
                        draft,
                        PropertyFieldId::ArtworkWeightMappingComponent,
                    )? {
                        PropertyEnumChoice::SourceMappingComponent(value) => value,
                        _ => {
                            return Err(ValidationError::new(
                                "transition_draft.value",
                                "mapping component choice is invalid",
                            ));
                        }
                    };
                    let placement = match transition_choice(
                        draft,
                        PropertyFieldId::ArtworkWeightMappingPlacement,
                    )? {
                        PropertyEnumChoice::SourcePlacement(value) => value,
                        _ => {
                            return Err(ValidationError::new(
                                "transition_draft.value",
                                "mapping placement choice is invalid",
                            ));
                        }
                    };
                    let inverted = match transition_value(
                        draft,
                        PropertyFieldId::ArtworkWeightMappingInverted,
                    )? {
                        VariantTransitionValue::Boolean(value) => *value,
                        _ => {
                            return Err(ValidationError::new(
                                "transition_draft.value",
                                "mapping inverted value is invalid",
                            ));
                        }
                    };
                    let response =
                        match transition_choice(draft, PropertyFieldId::ArtworkWeightResponse)? {
                            PropertyEnumChoice::ArtworkWeightResponse(value) => value,
                            _ => {
                                return Err(ValidationError::new(
                                    "transition_draft.value",
                                    "artwork response choice is invalid",
                                ));
                            }
                        };
                    SiteDensityModulation::ArtworkWeighted {
                        mapping: SourceMapping {
                            component,
                            placement,
                            inverted,
                            gain: transition_f64(draft, PropertyFieldId::ArtworkWeightMappingGain)?,
                            bias: transition_f64(draft, PropertyFieldId::ArtworkWeightMappingBias)?,
                        },
                        strength: transition_f64(draft, PropertyFieldId::ArtworkWeightStrength)?,
                        response,
                    }
                }
            };
            Ok(PatternDefinitionEdit::SetDensityModulationVariant {
                mechanism_id,
                modulation,
            })
        }
        (
            PropertyFieldId::RandomExclusion,
            PropertyTarget::Mechanism(_, mechanism_id),
            PropertyEnumChoice::Exclusion(choice),
        ) => {
            let policy = match choice {
                ExclusionKind::None => SiteExclusionPolicy::None,
                ExclusionKind::MinimumCenterDistance => {
                    SiteExclusionPolicy::MinimumCenterDistance {
                        minimum: transition_f64(
                            draft,
                            PropertyFieldId::ExclusionMinimumCenterDistance,
                        )?,
                    }
                }
                ExclusionKind::VisibleMarkMargin => {
                    let sizing =
                        match transition_choice(draft, PropertyFieldId::VisibleMarkSizingPolicy)? {
                            PropertyEnumChoice::VisibleMarkSizingPolicy(value) => value,
                            _ => {
                                return Err(ValidationError::new(
                                    "transition_draft.value",
                                    "visible-mark sizing choice is invalid",
                                ));
                            }
                        };
                    SiteExclusionPolicy::VisibleMarkMargin {
                        margin: transition_f64(draft, PropertyFieldId::VisibleMarkMargin)?,
                        sizing,
                    }
                }
            };
            Ok(PatternDefinitionEdit::SetExclusionVariant {
                mechanism_id,
                policy,
            })
        }
        (
            PropertyFieldId::OutputOrientation,
            PropertyTarget::OutputLayer(_, output_layer_id),
            PropertyEnumChoice::MarkOrientation(choice),
        ) => {
            let orientation = match choice {
                MarkOrientationKind::Fixed => MarkOrientation::Fixed,
                MarkOrientationKind::GuideTangent => {
                    match transition_reference(draft, PropertyFieldId::OutputOrientationDimension)?
                    {
                        PropertyReferenceValue::GuideDimension(dimension_id) => {
                            MarkOrientation::GuideTangent { dimension_id }
                        }
                        _ => {
                            return Err(ValidationError::new(
                                "transition_draft.reference",
                                "orientation requires a guide dimension",
                            ));
                        }
                    }
                }
                MarkOrientationKind::GuideNormal => {
                    match transition_reference(draft, PropertyFieldId::OutputOrientationDimension)?
                    {
                        PropertyReferenceValue::GuideDimension(dimension_id) => {
                            MarkOrientation::GuideNormal { dimension_id }
                        }
                        _ => {
                            return Err(ValidationError::new(
                                "transition_draft.reference",
                                "orientation requires a guide dimension",
                            ));
                        }
                    }
                }
            };
            Ok(PatternDefinitionEdit::SetOutputOrientation {
                output_layer_id,
                orientation,
            })
        }
        _ => Err(ValidationError::new(
            "transition_draft.selector",
            "transition draft selector/choice is invalid",
        )),
    }
}

/// Exhaustive command-facing schema contract for one editable field.  It is
/// intentionally value-free; descriptors supply active-variant applicability,
/// while command validation uses the same field identity and invalidation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropertyFieldContract {
    pub field: PropertyFieldId,
    pub command_kind: PropertyCommandKind,
    pub value_kind: PropertyValueKind,
    pub choices: &'static [PropertyEnumChoice],
    pub bounds: Option<PropertyBounds>,
    pub unit: PropertyUnit,
    pub applicability: PropertyApplicability,
    pub invalidation: InvalidationLevel,
    pub copy_on_edit_escalates_to_family: bool,
    pub structural_support: StructuralSupportConstraint,
    pub reference_constraint: PropertyReferenceConstraint,
    pub choice_policy: PropertyChoicePolicy,
}

pub const fn property_field_contract(field: PropertyFieldId) -> PropertyFieldContract {
    PropertyFieldContract {
        field,
        command_kind: match field {
            PropertyFieldId::SourceReference => PropertyCommandKind::SetSourceReference,
            PropertyFieldId::DensityAcrossX | PropertyFieldId::DensityAcrossY => {
                PropertyCommandKind::SetDensityAxis
            }
            PropertyFieldId::DensityAspectLocked => PropertyCommandKind::SetDensityAspectLock,
            PropertyFieldId::RotationDegrees => PropertyCommandKind::SetRotation,
            PropertyFieldId::TranslationX | PropertyFieldId::TranslationY => {
                PropertyCommandKind::SetTranslationAxis
            }
            PropertyFieldId::MarkMinimumSize | PropertyFieldId::MarkMaximumSize => {
                PropertyCommandKind::SetMarkGeometryField
            }
            PropertyFieldId::LegacyMappingComponent | PropertyFieldId::LegacyMappingPlacement => {
                PropertyCommandKind::SetLegacyMappingField
            }
            PropertyFieldId::ModeledMappingComponent
            | PropertyFieldId::ModeledMappingPlacement
            | PropertyFieldId::ModeledMappingInverted
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ModeledMappingBias => PropertyCommandKind::SetModeledMappingField,
            PropertyFieldId::Paint => PropertyCommandKind::SetPaint,
            PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha => PropertyCommandKind::SetColorComponent,
            PropertyFieldId::Opacity => PropertyCommandKind::SetOpacity,
            PropertyFieldId::Visibility => PropertyCommandKind::SetVisibility,
            PropertyFieldId::DefinitionSelection => PropertyCommandKind::RetargetDefinition,
            PropertyFieldId::GuideBaselineAngle => PropertyCommandKind::SetGuideBaselineAngle,
            PropertyFieldId::GuidePhase => PropertyCommandKind::SetGuidePhase,
            PropertyFieldId::GuideSpacingMultiplier => {
                PropertyCommandKind::SetGuideSpacingMultiplier
            }
            PropertyFieldId::IntersectionDimensions => {
                PropertyCommandKind::SetIntersectionDimensions
            }
            PropertyFieldId::IntersectionMergeEpsilon => {
                PropertyCommandKind::SetIntersectionMergeEpsilon
            }
            PropertyFieldId::AlongGuideDimensions => PropertyCommandKind::SetAlongGuideDimensions,
            PropertyFieldId::AlongGuideIntervalMultiplier => {
                PropertyCommandKind::SetAlongGuideIntervalMultiplier
            }
            PropertyFieldId::AlongGuidePhase => PropertyCommandKind::SetAlongGuidePhase,
            PropertyFieldId::RandomCharacter => PropertyCommandKind::SetRandomCharacter,
            PropertyFieldId::RandomSeed => PropertyCommandKind::SetRandomSeed,
            PropertyFieldId::RandomEvenMinimumCenterDistance => {
                PropertyCommandKind::SetRandomEvenMinimumCenterDistance
            }
            PropertyFieldId::RandomClusterDensity => PropertyCommandKind::SetRandomClusterDensity,
            PropertyFieldId::RandomClusterSpread => PropertyCommandKind::SetRandomClusterSpread,
            PropertyFieldId::RandomClusterStrength => PropertyCommandKind::SetRandomClusterStrength,
            PropertyFieldId::RandomDensityModulation => {
                PropertyCommandKind::SetDensityModulationVariant
            }
            PropertyFieldId::ArtworkWeightMappingComponent => {
                PropertyCommandKind::SetArtworkWeightMappingComponent
            }
            PropertyFieldId::ArtworkWeightMappingPlacement => {
                PropertyCommandKind::SetArtworkWeightMappingPlacement
            }
            PropertyFieldId::ArtworkWeightMappingInverted => {
                PropertyCommandKind::SetArtworkWeightMappingInverted
            }
            PropertyFieldId::ArtworkWeightMappingGain => {
                PropertyCommandKind::SetArtworkWeightMappingGain
            }
            PropertyFieldId::ArtworkWeightMappingBias => {
                PropertyCommandKind::SetArtworkWeightMappingBias
            }
            PropertyFieldId::ArtworkWeightStrength => PropertyCommandKind::SetArtworkWeightStrength,
            PropertyFieldId::ArtworkWeightResponse => PropertyCommandKind::SetArtworkWeightResponse,
            PropertyFieldId::RandomExclusion => PropertyCommandKind::SetExclusionVariant,
            PropertyFieldId::ExclusionMinimumCenterDistance => {
                PropertyCommandKind::SetExclusionMinimumCenterDistance
            }
            PropertyFieldId::VisibleMarkMargin => PropertyCommandKind::SetVisibleMarkMargin,
            PropertyFieldId::VisibleMarkSizingPolicy => {
                PropertyCommandKind::SetVisibleMarkSizingPolicy
            }
            PropertyFieldId::RandomMaximumAttempts => PropertyCommandKind::SetRandomMaximumAttempts,
            PropertyFieldId::RandomMaximumNeighborChecks => {
                PropertyCommandKind::SetRandomMaximumNeighborChecks
            }
            PropertyFieldId::OutputSiteProduct => PropertyCommandKind::SetOutputSiteProduct,
            PropertyFieldId::OutputPrototype => PropertyCommandKind::SetOutputMarkPrototype,
            PropertyFieldId::OutputOrientation => PropertyCommandKind::SetOutputOrientation,
            PropertyFieldId::OutputOrientationDimension => {
                PropertyCommandKind::SetOutputOrientationDimension
            }
            PropertyFieldId::CoverageGuardSteps => PropertyCommandKind::SetCoverageGuardSteps,
            PropertyFieldId::CoverageMaximumSupportRadius => {
                PropertyCommandKind::SetCoverageMaximumSupportRadius
            }
        },
        value_kind: match field {
            PropertyFieldId::SourceReference
            | PropertyFieldId::DefinitionSelection
            | PropertyFieldId::IntersectionDimensions
            | PropertyFieldId::AlongGuideDimensions
            | PropertyFieldId::OutputSiteProduct
            | PropertyFieldId::OutputOrientationDimension => PropertyValueKind::StableIdReference,
            PropertyFieldId::DensityAspectLocked
            | PropertyFieldId::ModeledMappingInverted
            | PropertyFieldId::ArtworkWeightMappingInverted
            | PropertyFieldId::Visibility => PropertyValueKind::Boolean,
            PropertyFieldId::CoverageGuardSteps
            | PropertyFieldId::RandomSeed
            | PropertyFieldId::RandomMaximumAttempts
            | PropertyFieldId::RandomMaximumNeighborChecks => PropertyValueKind::U32,
            PropertyFieldId::LegacyMappingComponent
            | PropertyFieldId::LegacyMappingPlacement
            | PropertyFieldId::ModeledMappingComponent
            | PropertyFieldId::ModeledMappingPlacement
            | PropertyFieldId::Paint
            | PropertyFieldId::RandomCharacter
            | PropertyFieldId::RandomDensityModulation
            | PropertyFieldId::ArtworkWeightMappingComponent
            | PropertyFieldId::ArtworkWeightMappingPlacement
            | PropertyFieldId::ArtworkWeightResponse
            | PropertyFieldId::RandomExclusion
            | PropertyFieldId::VisibleMarkSizingPolicy
            | PropertyFieldId::OutputPrototype
            | PropertyFieldId::OutputOrientation => PropertyValueKind::EnumChoice,
            _ => PropertyValueKind::FiniteF64,
        },
        choices: match field {
            PropertyFieldId::LegacyMappingComponent => LEGACY_COMPONENT_CHOICES,
            PropertyFieldId::LegacyMappingPlacement
            | PropertyFieldId::ModeledMappingPlacement
            | PropertyFieldId::ArtworkWeightMappingPlacement => SOURCE_PLACEMENT_CHOICES,
            PropertyFieldId::ModeledMappingComponent
            | PropertyFieldId::ArtworkWeightMappingComponent => SOURCE_MAPPING_CHOICES,
            PropertyFieldId::RandomCharacter => RANDOM_CHARACTER_CHOICES,
            PropertyFieldId::RandomDensityModulation => DENSITY_MODULATION_CHOICES,
            PropertyFieldId::ArtworkWeightResponse => ARTWORK_RESPONSE_CHOICES,
            PropertyFieldId::RandomExclusion => EXCLUSION_CHOICES,
            PropertyFieldId::VisibleMarkSizingPolicy => VISIBLE_MARK_SIZING_POLICY_CHOICES,
            PropertyFieldId::OutputPrototype => MARK_PROTOTYPE_CHOICES,
            PropertyFieldId::OutputOrientation => MARK_ORIENTATION_CHOICES,
            _ => &[],
        },
        bounds: match field {
            PropertyFieldId::DensityAcrossX
            | PropertyFieldId::DensityAcrossY
            | PropertyFieldId::GuideSpacingMultiplier
            | PropertyFieldId::AlongGuideIntervalMultiplier
            | PropertyFieldId::RandomEvenMinimumCenterDistance
            | PropertyFieldId::RandomClusterDensity
            | PropertyFieldId::RandomClusterSpread
            | PropertyFieldId::ExclusionMinimumCenterDistance
            | PropertyFieldId::RandomMaximumAttempts
            | PropertyFieldId::RandomMaximumNeighborChecks => positive_bounds(),
            PropertyFieldId::MarkMinimumSize
            | PropertyFieldId::MarkMaximumSize
            | PropertyFieldId::CoverageMaximumSupportRadius
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ArtworkWeightMappingGain
            | PropertyFieldId::VisibleMarkMargin
            | PropertyFieldId::IntersectionMergeEpsilon => nonnegative_bounds(),
            PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha
            | PropertyFieldId::Opacity
            | PropertyFieldId::RandomClusterStrength
            | PropertyFieldId::ArtworkWeightStrength => unit_bounds(),
            _ => None,
        },
        unit: match field {
            PropertyFieldId::DensityAcrossX
            | PropertyFieldId::DensityAcrossY
            | PropertyFieldId::GuideSpacingMultiplier
            | PropertyFieldId::AlongGuideIntervalMultiplier
            | PropertyFieldId::RandomClusterDensity => PropertyUnit::Density,
            PropertyFieldId::RotationDegrees | PropertyFieldId::GuideBaselineAngle => {
                PropertyUnit::Degrees
            }
            PropertyFieldId::GuidePhase | PropertyFieldId::AlongGuidePhase => PropertyUnit::Phase,
            PropertyFieldId::TranslationX
            | PropertyFieldId::TranslationY
            | PropertyFieldId::MarkMinimumSize
            | PropertyFieldId::MarkMaximumSize
            | PropertyFieldId::CoverageMaximumSupportRadius
            | PropertyFieldId::IntersectionMergeEpsilon
            | PropertyFieldId::RandomEvenMinimumCenterDistance
            | PropertyFieldId::RandomClusterSpread
            | PropertyFieldId::ExclusionMinimumCenterDistance
            | PropertyFieldId::VisibleMarkMargin => PropertyUnit::DocumentDistance,
            PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha
            | PropertyFieldId::Opacity
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ModeledMappingBias
            | PropertyFieldId::RandomClusterStrength
            | PropertyFieldId::ArtworkWeightMappingGain
            | PropertyFieldId::ArtworkWeightMappingBias
            | PropertyFieldId::ArtworkWeightStrength => PropertyUnit::NormalizedComponent,
            PropertyFieldId::CoverageGuardSteps
            | PropertyFieldId::RandomSeed
            | PropertyFieldId::RandomMaximumAttempts
            | PropertyFieldId::RandomMaximumNeighborChecks => PropertyUnit::Count,
            _ => PropertyUnit::None,
        },
        applicability: match field {
            PropertyFieldId::ModeledMappingComponent
            | PropertyFieldId::ModeledMappingPlacement
            | PropertyFieldId::ModeledMappingInverted
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ModeledMappingBias => PropertyApplicability::ModeledChannel,
            PropertyFieldId::Paint => PropertyApplicability::CurrentPaint,
            PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha => PropertyApplicability::SolidPaint,
            PropertyFieldId::GuideBaselineAngle
            | PropertyFieldId::GuidePhase
            | PropertyFieldId::GuideSpacingMultiplier => {
                PropertyApplicability::StraightGuideDimension
            }
            PropertyFieldId::IntersectionDimensions | PropertyFieldId::IntersectionMergeEpsilon => {
                PropertyApplicability::IntersectionProduct
            }
            PropertyFieldId::AlongGuideDimensions
            | PropertyFieldId::AlongGuideIntervalMultiplier
            | PropertyFieldId::AlongGuidePhase => PropertyApplicability::AlongGuideProduct,
            PropertyFieldId::RandomCharacter | PropertyFieldId::RandomSeed => {
                PropertyApplicability::RandomProcess
            }
            PropertyFieldId::RandomEvenMinimumCenterDistance => {
                PropertyApplicability::EvenRandomProcess
            }
            PropertyFieldId::RandomClusterDensity
            | PropertyFieldId::RandomClusterSpread
            | PropertyFieldId::RandomClusterStrength => {
                PropertyApplicability::ClusteredRandomProcess
            }
            PropertyFieldId::RandomDensityModulation => {
                PropertyApplicability::CurrentDensityModulation
            }
            PropertyFieldId::ArtworkWeightMappingComponent
            | PropertyFieldId::ArtworkWeightMappingPlacement
            | PropertyFieldId::ArtworkWeightMappingInverted
            | PropertyFieldId::ArtworkWeightMappingGain
            | PropertyFieldId::ArtworkWeightMappingBias
            | PropertyFieldId::ArtworkWeightStrength
            | PropertyFieldId::ArtworkWeightResponse => {
                PropertyApplicability::ArtworkWeightedDensity
            }
            PropertyFieldId::RandomExclusion => PropertyApplicability::CurrentExclusion,
            PropertyFieldId::ExclusionMinimumCenterDistance => {
                PropertyApplicability::MinimumCenterExclusion
            }
            PropertyFieldId::VisibleMarkMargin | PropertyFieldId::VisibleMarkSizingPolicy => {
                PropertyApplicability::VisibleMarkExclusion
            }
            PropertyFieldId::RandomMaximumAttempts
            | PropertyFieldId::RandomMaximumNeighborChecks => PropertyApplicability::RandomProcess,
            PropertyFieldId::OutputSiteProduct
            | PropertyFieldId::OutputPrototype
            | PropertyFieldId::OutputOrientation => PropertyApplicability::MarkPrototypeOutput,
            PropertyFieldId::OutputOrientationDimension => {
                PropertyApplicability::GuidedOutputOrientation
            }
            _ => PropertyApplicability::Always,
        },
        invalidation: match field {
            PropertyFieldId::SourceReference => InvalidationLevel::Source,
            PropertyFieldId::MarkMinimumSize
            | PropertyFieldId::MarkMaximumSize
            | PropertyFieldId::LegacyMappingComponent
            | PropertyFieldId::LegacyMappingPlacement
            | PropertyFieldId::ModeledMappingComponent
            | PropertyFieldId::ModeledMappingPlacement
            | PropertyFieldId::ModeledMappingInverted
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ModeledMappingBias
            | PropertyFieldId::OutputSiteProduct
            | PropertyFieldId::OutputPrototype
            | PropertyFieldId::OutputOrientation
            | PropertyFieldId::OutputOrientationDimension => InvalidationLevel::Realization,
            PropertyFieldId::Paint
            | PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha
            | PropertyFieldId::Opacity
            | PropertyFieldId::Visibility => InvalidationLevel::Presentation,
            _ => InvalidationLevel::Family,
        },
        copy_on_edit_escalates_to_family: matches!(
            field,
            PropertyFieldId::CoverageGuardSteps
                | PropertyFieldId::CoverageMaximumSupportRadius
                | PropertyFieldId::GuideBaselineAngle
                | PropertyFieldId::GuidePhase
                | PropertyFieldId::GuideSpacingMultiplier
                | PropertyFieldId::IntersectionDimensions
                | PropertyFieldId::IntersectionMergeEpsilon
                | PropertyFieldId::AlongGuideDimensions
                | PropertyFieldId::AlongGuideIntervalMultiplier
                | PropertyFieldId::AlongGuidePhase
                | PropertyFieldId::RandomCharacter
                | PropertyFieldId::RandomEvenMinimumCenterDistance
                | PropertyFieldId::RandomClusterDensity
                | PropertyFieldId::RandomClusterSpread
                | PropertyFieldId::RandomClusterStrength
                | PropertyFieldId::RandomSeed
                | PropertyFieldId::RandomDensityModulation
                | PropertyFieldId::ArtworkWeightMappingComponent
                | PropertyFieldId::ArtworkWeightMappingPlacement
                | PropertyFieldId::ArtworkWeightMappingInverted
                | PropertyFieldId::ArtworkWeightMappingGain
                | PropertyFieldId::ArtworkWeightMappingBias
                | PropertyFieldId::ArtworkWeightStrength
                | PropertyFieldId::ArtworkWeightResponse
                | PropertyFieldId::RandomExclusion
                | PropertyFieldId::ExclusionMinimumCenterDistance
                | PropertyFieldId::VisibleMarkMargin
                | PropertyFieldId::VisibleMarkSizingPolicy
                | PropertyFieldId::RandomMaximumAttempts
                | PropertyFieldId::RandomMaximumNeighborChecks
                | PropertyFieldId::OutputSiteProduct
                | PropertyFieldId::OutputPrototype
                | PropertyFieldId::OutputOrientation
                | PropertyFieldId::OutputOrientationDimension
        ),
        structural_support: match field {
            PropertyFieldId::MarkMinimumSize | PropertyFieldId::MarkMaximumSize => {
                StructuralSupportConstraint::MarkResponseMustFitDefinitionMaximumSupport
            }
            PropertyFieldId::CoverageMaximumSupportRadius => {
                StructuralSupportConstraint::DefinesMaximumMarkSupportRadius
            }
            PropertyFieldId::RandomExclusion
            | PropertyFieldId::VisibleMarkMargin
            | PropertyFieldId::VisibleMarkSizingPolicy => {
                StructuralSupportConstraint::VisibleMarkMarginUsesMaximumSupportRadius
            }
            _ => StructuralSupportConstraint::None,
        },
        reference_constraint: match field {
            PropertyFieldId::IntersectionDimensions => {
                PropertyReferenceConstraint::OrderedUniqueCollection {
                    minimum_items: 2,
                    maximum_items: 4,
                }
            }
            PropertyFieldId::AlongGuideDimensions => {
                PropertyReferenceConstraint::OrderedUniqueCollection {
                    minimum_items: 1,
                    maximum_items: 4,
                }
            }
            PropertyFieldId::SourceReference
            | PropertyFieldId::DefinitionSelection
            | PropertyFieldId::OutputSiteProduct
            | PropertyFieldId::OutputOrientationDimension => PropertyReferenceConstraint::Singular,
            _ => PropertyReferenceConstraint::NotReference,
        },
        choice_policy: match field {
            PropertyFieldId::Paint => PropertyChoicePolicy::ModelRolePaint,
            _ => PropertyChoicePolicy::Static,
        },
    }
}

pub fn property_field_contracts() -> impl ExactSizeIterator<Item = PropertyFieldContract> {
    PROPERTY_FIELD_IDS
        .iter()
        .copied()
        .map(property_field_contract)
}

impl PropertyDescriptor {
    pub const fn command_kind(&self) -> PropertyCommandKind {
        property_field_contract(self.field).command_kind
    }
}

const SOURCE_MAPPING_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Red),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Green),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Blue),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Cyan),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Magenta),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Yellow),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Black),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Alpha),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Luminance),
];
const LEGACY_COMPONENT_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Luminance),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Alpha),
];
const SOURCE_PLACEMENT_CHOICES: &[PropertyEnumChoice] = &[PropertyEnumChoice::SourcePlacement(
    SourcePlacement::StretchToCanvas,
)];
const SOLID_PAINT_CHOICES: &[PropertyEnumChoice] = &[PropertyEnumChoice::Paint(PaintKind::Solid)];
const SAMPLED_PAINT_CHOICES: &[PropertyEnumChoice] =
    &[PropertyEnumChoice::Paint(PaintKind::SampledSource)];
const RANDOM_CHARACTER_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::RandomCharacter(RandomCharacterKind::RawUniform),
    PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
    PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Clustered),
];
const DENSITY_MODULATION_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::DensityModulation(DensityModulationKind::Uniform),
    PropertyEnumChoice::DensityModulation(DensityModulationKind::ArtworkWeighted),
];
const ARTWORK_RESPONSE_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::ArtworkWeightResponse(ArtworkWeightResponse::Linear),
    PropertyEnumChoice::ArtworkWeightResponse(ArtworkWeightResponse::Smoothstep),
];
const EXCLUSION_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::Exclusion(ExclusionKind::None),
    PropertyEnumChoice::Exclusion(ExclusionKind::MinimumCenterDistance),
    PropertyEnumChoice::Exclusion(ExclusionKind::VisibleMarkMargin),
];
const VISIBLE_MARK_SIZING_POLICY_CHOICES: &[PropertyEnumChoice] =
    &[PropertyEnumChoice::VisibleMarkSizingPolicy(
        VisibleMarkSizingPolicy::MaximumSupportRadius,
    )];
const MARK_PROTOTYPE_CHOICES: &[PropertyEnumChoice] =
    &[PropertyEnumChoice::MarkPrototype(MarkPrototypeKind::Circle)];
const MARK_ORIENTATION_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::MarkOrientation(MarkOrientationKind::Fixed),
    PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideTangent),
    PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideNormal),
];

const fn dependency_for_contract(
    applicability: PropertyApplicability,
    dynamic: PropertyDependency,
) -> PropertyDependency {
    match applicability {
        PropertyApplicability::Always => PropertyDependency::Always,
        PropertyApplicability::ModeledChannel => PropertyDependency::ModeledChannel,
        PropertyApplicability::SolidPaint => PropertyDependency::SolidPaint,
        PropertyApplicability::StraightGuideDimension => PropertyDependency::StraightGuideDimension,
        PropertyApplicability::IntersectionProduct => PropertyDependency::IntersectionProduct,
        PropertyApplicability::AlongGuideProduct => PropertyDependency::AlongGuideProduct,
        PropertyApplicability::RandomProcess => PropertyDependency::RandomProcess,
        PropertyApplicability::EvenRandomProcess => PropertyDependency::EvenRandomProcess,
        PropertyApplicability::ClusteredRandomProcess => PropertyDependency::ClusteredRandomProcess,
        PropertyApplicability::ArtworkWeightedDensity => PropertyDependency::ArtworkWeightedDensity,
        PropertyApplicability::MinimumCenterExclusion => PropertyDependency::MinimumCenterExclusion,
        PropertyApplicability::VisibleMarkExclusion => PropertyDependency::VisibleMarkExclusion,
        PropertyApplicability::MarkPrototypeOutput => PropertyDependency::MarkPrototypeOutput,
        PropertyApplicability::GuidedOutputOrientation => {
            PropertyDependency::GuidedOutputOrientation
        }
        PropertyApplicability::CurrentPaint
        | PropertyApplicability::CurrentDensityModulation
        | PropertyApplicability::CurrentExclusion => dynamic,
    }
}

#[derive(Clone, Copy)]
enum DescriptorRuntimeContext {
    Paint {
        choices: &'static [PropertyEnumChoice],
        dependency: PropertyDependency,
    },
    DensityModulation {
        dependency: PropertyDependency,
    },
    Exclusion {
        dependency: PropertyDependency,
        support: StructuralSupportConstraint,
    },
}

const fn descriptor_from_contract(
    field: PropertyFieldId,
    target: PropertyTarget,
) -> PropertyDescriptor {
    let contract = property_field_contract(field);
    PropertyDescriptor {
        field,
        target,
        value_kind: contract.value_kind,
        choices: contract.choices,
        bounds: contract.bounds,
        unit: contract.unit,
        dependency: dependency_for_contract(contract.applicability, PropertyDependency::Always),
        invalidation: contract.invalidation,
        copy_on_edit_escalates_to_family: contract.copy_on_edit_escalates_to_family,
        structural_support: contract.structural_support,
        reference_constraint: contract.reference_constraint,
        choice_policy: contract.choice_policy,
    }
}

const fn descriptor_with_runtime_context(
    field: PropertyFieldId,
    target: PropertyTarget,
    context: DescriptorRuntimeContext,
) -> PropertyDescriptor {
    let contract = property_field_contract(field);
    let (choices, dependency, structural_support) = match context {
        DescriptorRuntimeContext::Paint {
            choices,
            dependency,
        } => (choices, dependency, contract.structural_support),
        DescriptorRuntimeContext::DensityModulation { dependency } => {
            (contract.choices, dependency, contract.structural_support)
        }
        DescriptorRuntimeContext::Exclusion {
            dependency,
            support,
        } => (contract.choices, dependency, support),
    };
    PropertyDescriptor {
        field,
        target,
        value_kind: contract.value_kind,
        choices,
        bounds: contract.bounds,
        unit: contract.unit,
        dependency,
        invalidation: contract.invalidation,
        copy_on_edit_escalates_to_family: contract.copy_on_edit_escalates_to_family,
        structural_support,
        reference_constraint: contract.reference_constraint,
        choice_policy: contract.choice_policy,
    }
}

const fn positive_bounds() -> Option<PropertyBounds> {
    Some(PropertyBounds {
        minimum: Some(0.0),
        minimum_inclusive: false,
        maximum: None,
        maximum_inclusive: false,
    })
}
const fn nonnegative_bounds() -> Option<PropertyBounds> {
    Some(PropertyBounds {
        minimum: Some(0.0),
        minimum_inclusive: true,
        maximum: None,
        maximum_inclusive: false,
    })
}
const fn unit_bounds() -> Option<PropertyBounds> {
    Some(PropertyBounds {
        minimum: Some(0.0),
        minimum_inclusive: true,
        maximum: Some(1.0),
        maximum_inclusive: true,
    })
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
    SetCoverageGuardSteps {
        guard_steps: u32,
    },
    SetCoverageMaximumSupportRadius {
        maximum_support_radius: f64,
    },
    SetGuideBaselineAngle {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        baseline_angle_degrees: f64,
    },
    SetGuidePhase {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        phase: f64,
    },
    SetGuideSpacingMultiplier {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        spacing_multiplier: f64,
    },
    SetIntersectionDimensions {
        mechanism_id: PatternMechanismId,
        dimensions: Vec<GuideDimensionId>,
    },
    SetIntersectionMergeEpsilon {
        mechanism_id: PatternMechanismId,
        merge_epsilon: f64,
    },
    SetAlongGuideDimensions {
        mechanism_id: PatternMechanismId,
        dimensions: Vec<GuideDimensionId>,
    },
    SetAlongGuideIntervalMultiplier {
        mechanism_id: PatternMechanismId,
        interval_multiplier: f64,
    },
    SetAlongGuidePhase {
        mechanism_id: PatternMechanismId,
        phase: f64,
    },
    SetRandomCharacter {
        mechanism_id: PatternMechanismId,
        character: RandomSiteCharacter,
    },
    SetRandomSeed {
        mechanism_id: PatternMechanismId,
        seed: u32,
    },
    SetRandomEvenMinimumCenterDistance {
        mechanism_id: PatternMechanismId,
        minimum_center_distance: f64,
    },
    SetRandomClusterDensity {
        mechanism_id: PatternMechanismId,
        cluster_density: f64,
    },
    SetRandomClusterSpread {
        mechanism_id: PatternMechanismId,
        cluster_spread: f64,
    },
    SetRandomClusterStrength {
        mechanism_id: PatternMechanismId,
        cluster_strength: f64,
    },
    SetDensityModulationVariant {
        mechanism_id: PatternMechanismId,
        modulation: SiteDensityModulation,
    },
    SetArtworkWeightMappingComponent {
        mechanism_id: PatternMechanismId,
        component: SourceMappingComponent,
    },
    SetArtworkWeightMappingPlacement {
        mechanism_id: PatternMechanismId,
        placement: SourcePlacement,
    },
    SetArtworkWeightMappingInverted {
        mechanism_id: PatternMechanismId,
        inverted: bool,
    },
    SetArtworkWeightMappingGain {
        mechanism_id: PatternMechanismId,
        gain: f64,
    },
    SetArtworkWeightMappingBias {
        mechanism_id: PatternMechanismId,
        bias: f64,
    },
    SetArtworkWeightStrength {
        mechanism_id: PatternMechanismId,
        strength: f64,
    },
    SetArtworkWeightResponse {
        mechanism_id: PatternMechanismId,
        response: ArtworkWeightResponse,
    },
    SetExclusionVariant {
        mechanism_id: PatternMechanismId,
        policy: SiteExclusionPolicy,
    },
    SetExclusionMinimumCenterDistance {
        mechanism_id: PatternMechanismId,
        minimum_center_distance: f64,
    },
    SetVisibleMarkMargin {
        mechanism_id: PatternMechanismId,
        margin: f64,
    },
    SetVisibleMarkSizingPolicy {
        mechanism_id: PatternMechanismId,
        sizing: VisibleMarkSizingPolicy,
    },
    SetRandomMaximumAttempts {
        mechanism_id: PatternMechanismId,
        maximum_attempts: u32,
    },
    SetRandomMaximumNeighborChecks {
        mechanism_id: PatternMechanismId,
        maximum_neighbor_checks: u32,
    },
    SetOutputSiteProduct {
        output_layer_id: PatternOutputLayerId,
        site_mechanism_id: PatternMechanismId,
    },
    SetOutputMarkPrototype {
        output_layer_id: PatternOutputLayerId,
        prototype: MarkPrototype,
    },
    SetOutputOrientation {
        output_layer_id: PatternOutputLayerId,
        orientation: MarkOrientation,
    },
    SetOutputOrientationDimension {
        output_layer_id: PatternOutputLayerId,
        dimension_id: GuideDimensionId,
    },
}

/// Supported channel edits in the Stage 2 authoritative command boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum DocumentCommand {
    AddPatternDefinition {
        definition: PatternDefinitionDraft,
    },
    /// Installs a fully typed, stable-ID structural definition. This is the
    /// headless construction path for every accepted family; document-wide ID
    /// collision/order/reference validation still occurs before publication.
    AddTypedPatternDefinition {
        definition: PatternDefinition,
    },
    /// Atomically introduces a complete, fresh typed definition and retargets
    /// one selected channel. Family/topology conversion is intentionally not
    /// represented as a field edit.
    ReplaceSelectedChannelDefinitionTopology {
        channel_id: ChannelId,
        base_definition: PatternDefinition,
        definition: PatternDefinition,
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
    SetDensityAxis {
        channel_id: ChannelId,
        edited_axis: DensityEditedAxis,
        value: f64,
    },
    SetDensityAspectLock {
        channel_id: ChannelId,
        aspect_locked: bool,
    },
    SetRotation {
        channel_id: ChannelId,
        rotation_degrees: f64,
    },
    SetTranslationAxis {
        channel_id: ChannelId,
        edited_axis: TranslationEditedAxis,
        value: f64,
    },
    SetMarkGeometryField {
        channel_id: ChannelId,
        edit: MarkGeometryFieldEdit,
    },
    SetColorComponent {
        channel_id: ChannelId,
        component: ColorComponent,
        value: f64,
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
    SetLegacyMappingField {
        channel_id: ChannelId,
        edit: LegacyMappingFieldEdit,
    },
    /// Replaces the complete model and its ordered channel topology together.
    ReplaceChannelTopology {
        model: HalftoneChannelModel,
        topology: ChannelTopology,
    },
    /// Replaces the full Stage 9 mapping for one modeled channel.
    SetModeledMappingField {
        channel_id: ChannelId,
        edit: ModeledMappingFieldEdit,
    },
    /// Replaces ordinary solid paint for a modeled channel.
    SetChannelPaint {
        channel_id: ChannelId,
        paint: ChannelPaint,
    },
}

/// One typed value supplied to a descriptor-backed command leaf.  Structural
/// references and relational payloads remain typed in their command variants;
/// this projection exists only to share the scalar/choice contract with the
/// read-only descriptor surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PropertyFieldValue {
    FiniteF64(f64),
    U32(u32),
    Boolean(bool),
    StableIdReference,
    StableIdReferenceCollection(usize),
    EnumChoice(PropertyEnumChoice),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropertyCommandFieldProjection {
    pub field: PropertyFieldId,
    pub value: PropertyFieldValue,
}

/// Commands that intentionally do not address one editable property field.
/// These operations have their own typed authority and transition semantics;
/// keeping them explicit prevents new command variants from being silently
/// omitted from descriptor/command completeness checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonFieldCommandOperation {
    AddPatternDefinition,
    AddTypedPatternDefinition,
    ReplaceSelectedChannelDefinitionTopology,
    DuplicatePatternDefinition,
    RemoveUnreferencedPatternDefinition,
    ReplaceChannelTopology,
}

/// Exhaustive command classification at the descriptor boundary. A command is
/// either one or more descriptor-backed field leaves, or an explicitly typed
/// structural/topology operation. There is deliberately no implicit empty
/// fallback for future command variants.
#[derive(Clone, Debug, PartialEq)]
pub enum DocumentCommandFieldClassification {
    DescriptorBacked(Vec<PropertyCommandFieldProjection>),
    NonField(NonFieldCommandOperation),
}

impl PatternDefinitionEdit {
    pub fn field_projection(&self) -> PropertyCommandFieldProjection {
        use PatternDefinitionEdit as Edit;
        let (field, value) = match self {
            Edit::SetCoverageGuardSteps { guard_steps } => (
                PropertyFieldId::CoverageGuardSteps,
                PropertyFieldValue::U32(*guard_steps),
            ),
            Edit::SetCoverageMaximumSupportRadius {
                maximum_support_radius,
            } => (
                PropertyFieldId::CoverageMaximumSupportRadius,
                PropertyFieldValue::FiniteF64(*maximum_support_radius),
            ),
            Edit::SetGuideBaselineAngle {
                baseline_angle_degrees,
                ..
            } => (
                PropertyFieldId::GuideBaselineAngle,
                PropertyFieldValue::FiniteF64(*baseline_angle_degrees),
            ),
            Edit::SetGuidePhase { phase, .. } => (
                PropertyFieldId::GuidePhase,
                PropertyFieldValue::FiniteF64(*phase),
            ),
            Edit::SetGuideSpacingMultiplier {
                spacing_multiplier, ..
            } => (
                PropertyFieldId::GuideSpacingMultiplier,
                PropertyFieldValue::FiniteF64(*spacing_multiplier),
            ),
            Edit::SetIntersectionDimensions { dimensions, .. } => (
                PropertyFieldId::IntersectionDimensions,
                PropertyFieldValue::StableIdReferenceCollection(dimensions.len()),
            ),
            Edit::SetIntersectionMergeEpsilon { merge_epsilon, .. } => (
                PropertyFieldId::IntersectionMergeEpsilon,
                PropertyFieldValue::FiniteF64(*merge_epsilon),
            ),
            Edit::SetAlongGuideDimensions { dimensions, .. } => (
                PropertyFieldId::AlongGuideDimensions,
                PropertyFieldValue::StableIdReferenceCollection(dimensions.len()),
            ),
            Edit::SetAlongGuideIntervalMultiplier {
                interval_multiplier,
                ..
            } => (
                PropertyFieldId::AlongGuideIntervalMultiplier,
                PropertyFieldValue::FiniteF64(*interval_multiplier),
            ),
            Edit::SetAlongGuidePhase { phase, .. } => (
                PropertyFieldId::AlongGuidePhase,
                PropertyFieldValue::FiniteF64(*phase),
            ),
            Edit::SetRandomCharacter { character, .. } => (
                PropertyFieldId::RandomCharacter,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::RandomCharacter(
                    match character {
                        RandomSiteCharacter::RawUniform => RandomCharacterKind::RawUniform,
                        RandomSiteCharacter::Even { .. } => RandomCharacterKind::Even,
                        RandomSiteCharacter::Clustered { .. } => RandomCharacterKind::Clustered,
                    },
                )),
            ),
            Edit::SetRandomSeed { seed, .. } => {
                (PropertyFieldId::RandomSeed, PropertyFieldValue::U32(*seed))
            }
            Edit::SetRandomEvenMinimumCenterDistance {
                minimum_center_distance,
                ..
            } => (
                PropertyFieldId::RandomEvenMinimumCenterDistance,
                PropertyFieldValue::FiniteF64(*minimum_center_distance),
            ),
            Edit::SetRandomClusterDensity {
                cluster_density, ..
            } => (
                PropertyFieldId::RandomClusterDensity,
                PropertyFieldValue::FiniteF64(*cluster_density),
            ),
            Edit::SetRandomClusterSpread { cluster_spread, .. } => (
                PropertyFieldId::RandomClusterSpread,
                PropertyFieldValue::FiniteF64(*cluster_spread),
            ),
            Edit::SetRandomClusterStrength {
                cluster_strength, ..
            } => (
                PropertyFieldId::RandomClusterStrength,
                PropertyFieldValue::FiniteF64(*cluster_strength),
            ),
            Edit::SetDensityModulationVariant { modulation, .. } => (
                PropertyFieldId::RandomDensityModulation,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::DensityModulation(
                    match modulation {
                        SiteDensityModulation::Uniform => DensityModulationKind::Uniform,
                        SiteDensityModulation::ArtworkWeighted { .. } => {
                            DensityModulationKind::ArtworkWeighted
                        }
                    },
                )),
            ),
            Edit::SetArtworkWeightMappingComponent { component, .. } => (
                PropertyFieldId::ArtworkWeightMappingComponent,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::SourceMappingComponent(
                    *component,
                )),
            ),
            Edit::SetArtworkWeightMappingPlacement { placement, .. } => (
                PropertyFieldId::ArtworkWeightMappingPlacement,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::SourcePlacement(*placement)),
            ),
            Edit::SetArtworkWeightMappingInverted { inverted, .. } => (
                PropertyFieldId::ArtworkWeightMappingInverted,
                PropertyFieldValue::Boolean(*inverted),
            ),
            Edit::SetArtworkWeightMappingGain { gain, .. } => (
                PropertyFieldId::ArtworkWeightMappingGain,
                PropertyFieldValue::FiniteF64(*gain),
            ),
            Edit::SetArtworkWeightMappingBias { bias, .. } => (
                PropertyFieldId::ArtworkWeightMappingBias,
                PropertyFieldValue::FiniteF64(*bias),
            ),
            Edit::SetArtworkWeightStrength { strength, .. } => (
                PropertyFieldId::ArtworkWeightStrength,
                PropertyFieldValue::FiniteF64(*strength),
            ),
            Edit::SetArtworkWeightResponse { response, .. } => (
                PropertyFieldId::ArtworkWeightResponse,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::ArtworkWeightResponse(
                    *response,
                )),
            ),
            Edit::SetExclusionVariant { policy, .. } => (
                PropertyFieldId::RandomExclusion,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::Exclusion(match policy {
                    SiteExclusionPolicy::None => ExclusionKind::None,
                    SiteExclusionPolicy::MinimumCenterDistance { .. } => {
                        ExclusionKind::MinimumCenterDistance
                    }
                    SiteExclusionPolicy::VisibleMarkMargin { .. } => {
                        ExclusionKind::VisibleMarkMargin
                    }
                })),
            ),
            Edit::SetExclusionMinimumCenterDistance {
                minimum_center_distance,
                ..
            } => (
                PropertyFieldId::ExclusionMinimumCenterDistance,
                PropertyFieldValue::FiniteF64(*minimum_center_distance),
            ),
            Edit::SetVisibleMarkMargin { margin, .. } => (
                PropertyFieldId::VisibleMarkMargin,
                PropertyFieldValue::FiniteF64(*margin),
            ),
            Edit::SetVisibleMarkSizingPolicy { sizing, .. } => (
                PropertyFieldId::VisibleMarkSizingPolicy,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::VisibleMarkSizingPolicy(
                    *sizing,
                )),
            ),
            Edit::SetRandomMaximumAttempts {
                maximum_attempts, ..
            } => (
                PropertyFieldId::RandomMaximumAttempts,
                PropertyFieldValue::U32(*maximum_attempts),
            ),
            Edit::SetRandomMaximumNeighborChecks {
                maximum_neighbor_checks,
                ..
            } => (
                PropertyFieldId::RandomMaximumNeighborChecks,
                PropertyFieldValue::U32(*maximum_neighbor_checks),
            ),
            Edit::SetOutputSiteProduct { .. } => (
                PropertyFieldId::OutputSiteProduct,
                PropertyFieldValue::StableIdReference,
            ),
            Edit::SetOutputMarkPrototype { prototype, .. } => (
                PropertyFieldId::OutputPrototype,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::MarkPrototype(
                    match prototype {
                        MarkPrototype::Circle => MarkPrototypeKind::Circle,
                    },
                )),
            ),
            Edit::SetOutputOrientation { orientation, .. } => (
                PropertyFieldId::OutputOrientation,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::MarkOrientation(
                    match orientation {
                        MarkOrientation::Fixed => MarkOrientationKind::Fixed,
                        MarkOrientation::GuideTangent { .. } => MarkOrientationKind::GuideTangent,
                        MarkOrientation::GuideNormal { .. } => MarkOrientationKind::GuideNormal,
                    },
                )),
            ),
            Edit::SetOutputOrientationDimension { .. } => (
                PropertyFieldId::OutputOrientationDimension,
                PropertyFieldValue::StableIdReference,
            ),
        };
        PropertyCommandFieldProjection { field, value }
    }
}

impl DocumentCommand {
    /// Exhaustively classifies every command at the descriptor boundary.
    /// Compound coordinate commands deliberately yield one independently
    /// addressable field leaf.
    pub fn field_classification(&self) -> DocumentCommandFieldClassification {
        use DocumentCommand as Command;
        let one = |field, value| {
            DocumentCommandFieldClassification::DescriptorBacked(vec![
                PropertyCommandFieldProjection { field, value },
            ])
        };
        match self {
            Command::AddPatternDefinition { .. } => DocumentCommandFieldClassification::NonField(
                NonFieldCommandOperation::AddPatternDefinition,
            ),
            Command::AddTypedPatternDefinition { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::AddTypedPatternDefinition,
                )
            }
            Command::ReplaceSelectedChannelDefinitionTopology { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::ReplaceSelectedChannelDefinitionTopology,
                )
            }
            Command::DuplicatePatternDefinition { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::DuplicatePatternDefinition,
                )
            }
            Command::RemoveUnreferencedPatternDefinition { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::RemoveUnreferencedPatternDefinition,
                )
            }
            Command::ReplaceChannelTopology { .. } => DocumentCommandFieldClassification::NonField(
                NonFieldCommandOperation::ReplaceChannelTopology,
            ),
            Command::SetDensityAxis {
                edited_axis, value, ..
            } => one(
                match edited_axis {
                    DensityEditedAxis::AcrossX => PropertyFieldId::DensityAcrossX,
                    DensityEditedAxis::AcrossY => PropertyFieldId::DensityAcrossY,
                },
                PropertyFieldValue::FiniteF64(*value),
            ),
            Command::SetDensityAspectLock { aspect_locked, .. } => one(
                PropertyFieldId::DensityAspectLocked,
                PropertyFieldValue::Boolean(*aspect_locked),
            ),
            Command::SetRotation {
                rotation_degrees, ..
            } => one(
                PropertyFieldId::RotationDegrees,
                PropertyFieldValue::FiniteF64(*rotation_degrees),
            ),
            Command::SetTranslationAxis {
                edited_axis, value, ..
            } => one(
                match edited_axis {
                    TranslationEditedAxis::X => PropertyFieldId::TranslationX,
                    TranslationEditedAxis::Y => PropertyFieldId::TranslationY,
                },
                PropertyFieldValue::FiniteF64(*value),
            ),
            Command::SetMarkGeometryField { edit, .. } => one(
                match edit {
                    MarkGeometryFieldEdit::MinimumSize(_) => PropertyFieldId::MarkMinimumSize,
                    MarkGeometryFieldEdit::MaximumSize(_) => PropertyFieldId::MarkMaximumSize,
                },
                PropertyFieldValue::FiniteF64(match edit {
                    MarkGeometryFieldEdit::MinimumSize(value)
                    | MarkGeometryFieldEdit::MaximumSize(value) => *value,
                }),
            ),
            Command::SetColorComponent {
                component, value, ..
            } => one(
                match component {
                    ColorComponent::Red => PropertyFieldId::ColorRed,
                    ColorComponent::Green => PropertyFieldId::ColorGreen,
                    ColorComponent::Blue => PropertyFieldId::ColorBlue,
                    ColorComponent::Alpha => PropertyFieldId::ColorAlpha,
                },
                PropertyFieldValue::FiniteF64(*value),
            ),
            Command::SetOpacity { opacity, .. } => one(
                PropertyFieldId::Opacity,
                PropertyFieldValue::FiniteF64(*opacity),
            ),
            Command::SetVisibility { visible, .. } => one(
                PropertyFieldId::Visibility,
                PropertyFieldValue::Boolean(*visible),
            ),
            Command::SetSourceReference { .. } => one(
                PropertyFieldId::SourceReference,
                PropertyFieldValue::StableIdReference,
            ),
            Command::RetargetChannelPatternDefinition { .. } => one(
                PropertyFieldId::DefinitionSelection,
                PropertyFieldValue::StableIdReference,
            ),
            Command::SetLegacyMappingField { edit, .. } => one(
                match edit {
                    LegacyMappingFieldEdit::Component(_) => PropertyFieldId::LegacyMappingComponent,
                    LegacyMappingFieldEdit::Placement(_) => PropertyFieldId::LegacyMappingPlacement,
                },
                match edit {
                    LegacyMappingFieldEdit::Component(component) => PropertyFieldValue::EnumChoice(
                        PropertyEnumChoice::SourceMappingComponent(match component {
                            SourceComponent::Luminance => SourceMappingComponent::Luminance,
                            SourceComponent::Alpha => SourceMappingComponent::Alpha,
                        }),
                    ),
                    LegacyMappingFieldEdit::Placement(placement) => PropertyFieldValue::EnumChoice(
                        PropertyEnumChoice::SourcePlacement(*placement),
                    ),
                },
            ),
            Command::SetModeledMappingField { edit, .. } => one(
                match edit {
                    ModeledMappingFieldEdit::Component(_) => {
                        PropertyFieldId::ModeledMappingComponent
                    }
                    ModeledMappingFieldEdit::Placement(_) => {
                        PropertyFieldId::ModeledMappingPlacement
                    }
                    ModeledMappingFieldEdit::Inverted(_) => PropertyFieldId::ModeledMappingInverted,
                    ModeledMappingFieldEdit::Gain(_) => PropertyFieldId::ModeledMappingGain,
                    ModeledMappingFieldEdit::Bias(_) => PropertyFieldId::ModeledMappingBias,
                },
                match edit {
                    ModeledMappingFieldEdit::Component(value) => PropertyFieldValue::EnumChoice(
                        PropertyEnumChoice::SourceMappingComponent(*value),
                    ),
                    ModeledMappingFieldEdit::Placement(value) => {
                        PropertyFieldValue::EnumChoice(PropertyEnumChoice::SourcePlacement(*value))
                    }
                    ModeledMappingFieldEdit::Inverted(value) => PropertyFieldValue::Boolean(*value),
                    ModeledMappingFieldEdit::Gain(value) | ModeledMappingFieldEdit::Bias(value) => {
                        PropertyFieldValue::FiniteF64(*value)
                    }
                },
            ),
            Command::SetChannelPaint { paint, .. } => one(
                PropertyFieldId::Paint,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::Paint(match paint {
                    ChannelPaint::Solid(_) => PaintKind::Solid,
                    ChannelPaint::SampledSource => PaintKind::SampledSource,
                })),
            ),
            Command::EditSelectedChannelPatternDefinition { edit, .. }
            | Command::EditSharedPatternDefinition { edit, .. } => {
                DocumentCommandFieldClassification::DescriptorBacked(vec![edit.field_projection()])
            }
        }
    }

    /// Projects only descriptor-backed command leaves to their contract
    /// fields. Non-field structural operations intentionally have no property
    /// projections, but are exhaustively represented by `field_classification`.
    pub fn field_projections(&self) -> Vec<PropertyCommandFieldProjection> {
        match self.field_classification() {
            DocumentCommandFieldClassification::DescriptorBacked(projections) => projections,
            DocumentCommandFieldClassification::NonField(_) => Vec::new(),
        }
    }

    /// Stage 14 definition transitions require the reversible history owner.
    /// The public `DocumentSession` surface retains its pre-Stage-14 command
    /// behavior, while `DocumentHistory` uses its private transition path.
    fn requires_history(&self) -> bool {
        matches!(
            self,
            Self::AddPatternDefinition { .. }
                | Self::AddTypedPatternDefinition { .. }
                | Self::ReplaceSelectedChannelDefinitionTopology { .. }
                | Self::DuplicatePatternDefinition { .. }
                | Self::RetargetChannelPatternDefinition { .. }
                | Self::RemoveUnreferencedPatternDefinition { .. }
                | Self::EditSelectedChannelPatternDefinition { .. }
                | Self::EditSharedPatternDefinition { .. }
        )
    }

    fn channel_id(&self) -> ChannelId {
        match self {
            Self::ReplaceSelectedChannelDefinitionTopology { channel_id, .. }
            | Self::SetDensityAxis { channel_id, .. }
            | Self::SetDensityAspectLock { channel_id, .. }
            | Self::SetRotation { channel_id, .. }
            | Self::SetTranslationAxis { channel_id, .. }
            | Self::SetMarkGeometryField { channel_id, .. }
            | Self::SetColorComponent { channel_id, .. }
            | Self::SetOpacity { channel_id, .. }
            | Self::SetVisibility { channel_id, .. }
            | Self::SetLegacyMappingField { channel_id, .. }
            | Self::SetModeledMappingField { channel_id, .. }
            | Self::SetChannelPaint { channel_id, .. }
            | Self::RetargetChannelPatternDefinition { channel_id, .. }
            | Self::EditSelectedChannelPatternDefinition { channel_id, .. } => *channel_id,
            Self::SetSourceReference { .. }
            | Self::ReplaceChannelTopology { .. }
            | Self::AddPatternDefinition { .. }
            | Self::AddTypedPatternDefinition { .. }
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
                | Self::AddTypedPatternDefinition { .. }
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

        for projection in self.field_projections() {
            validate_property_field_projection(projection)?;
        }

        match self {
            Self::AddPatternDefinition { definition } => {
                document.allocate_definition_from_draft(definition)?;
                Ok(())
            }
            Self::AddTypedPatternDefinition { definition } => {
                validate_definition(definition)?;
                if document.definition(definition.id).is_some() {
                    return Err(ValidationError::new(
                        "pattern_definitions.id",
                        "definition ID already exists",
                    ));
                }
                // Candidate validation performs the document-wide collision
                // check over mechanisms, output layers, and dimensions.
                Ok(())
            }
            Self::ReplaceSelectedChannelDefinitionTopology {
                channel_id,
                base_definition,
                definition,
            } => {
                if document.pattern_definition_id_for(*channel_id) != Some(base_definition.id)
                    || document.definition(base_definition.id) != Some(base_definition)
                {
                    return Err(ValidationError::new(
                        "pattern_definitions.base",
                        "selected definition base is stale",
                    ));
                }
                if definition.id == base_definition.id
                    || document.definition(definition.id).is_some()
                {
                    return Err(ValidationError::new(
                        "pattern_definitions.id",
                        "topology replacement requires a fresh definition ID",
                    ));
                }
                validate_definition(definition)
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
                let definition = document
                    .definition(base_definition.id)
                    .expect("validated reference");
                validate_definition_edit(definition, edit)?;
                let mut edited = definition.clone();
                apply_definition_edit(&mut edited, edit);
                if &edited == definition {
                    return Err(ValidationError::new(
                        "pattern_definitions.edit",
                        "structural edit is a semantic no-op",
                    ));
                }
                validate_definition(&edited)?;
                if document.linked_channels(base_definition.id).len() > 1 {
                    // Copy-on-edit must allocate every fresh stable ID before
                    // publication. Validate the exact duplicate/remapped
                    // candidate here so exhaustion and remap failures remain
                    // ordinary atomic command errors rather than apply-time
                    // assertions.
                    let duplicate = document.duplicate_definition(definition)?;
                    let remapped_edit =
                        remap_definition_edit_for_duplicate(definition, &duplicate, edit);
                    let mut remapped = duplicate;
                    apply_definition_edit(&mut remapped, &remapped_edit);
                    validate_definition(&remapped)?;
                }
                Ok(())
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
                let mut edited = document
                    .definition(*definition_id)
                    .expect("validated reference")
                    .clone();
                validate_definition_edit(&edited, edit)?;
                apply_definition_edit(&mut edited, edit);
                if document.definition(*definition_id) == Some(&edited) {
                    return Err(ValidationError::new(
                        "pattern_definitions.edit",
                        "structural edit is a semantic no-op",
                    ));
                }
                validate_definition(&edited)
            }
            Self::SetDensityAxis {
                edited_axis, value, ..
            } => {
                validate_positive_finite(*value, "channel.pattern.layout.density")?;
                let layout = document
                    .channel_layout(self.channel_id())
                    .expect("validated channel");
                if layout.density.aspect_locked {
                    derive_density_axis(&document.canvas, *value, *edited_axis)?;
                }
                Ok(())
            }
            Self::SetDensityAspectLock { aspect_locked, .. } => {
                if *aspect_locked {
                    let layout = document
                        .channel_layout(self.channel_id())
                        .expect("validated channel");
                    derive_density_axis(
                        &document.canvas,
                        layout.density.across_x,
                        DensityEditedAxis::AcrossX,
                    )?;
                }
                Ok(())
            }
            Self::SetRotation {
                rotation_degrees, ..
            } => validate_finite(*rotation_degrees, "channel.pattern.layout.rotation_degrees"),
            Self::SetTranslationAxis { .. } => Ok(()),
            Self::SetMarkGeometryField { channel_id, edit } => {
                let pattern_definition_id = document
                    .pattern_definition_id_for(*channel_id)
                    .expect("command channel existence was checked above");
                let definition = document
                    .pattern_definitions
                    .iter()
                    .find(|definition| definition.id == pattern_definition_id)
                    .expect("document validation keeps channel definitions valid");
                let mut response = document
                    .channel_mark_response(*channel_id)
                    .expect("validated channel")
                    .clone();
                match edit {
                    MarkGeometryFieldEdit::MinimumSize(value) => response.minimum_size = *value,
                    MarkGeometryFieldEdit::MaximumSize(value) => response.maximum_size = *value,
                }
                validate_mark_response(&response, Some(definition.coverage.maximum_support_radius))
            }
            Self::SetColorComponent {
                channel_id, value, ..
            } => {
                validate_unit_component(*value, "channel.appearance.color")?;
                if let Some(channel) = document.modeled_channel(*channel_id)
                    && !matches!(channel.paint, ChannelPaint::Solid(_))
                {
                    return Err(ValidationError::new(
                        "channel.paint",
                        "sampled-source paint has no editable solid components",
                    ));
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
            Self::SetLegacyMappingField { .. } if document.channel_topology().is_some() => {
                Err(ValidationError::new(
                    "channel.source_mapping",
                    "modeled channels require a complete Stage 9 source mapping",
                ))
            }
            Self::SetLegacyMappingField { .. } => Ok(()),
            Self::ReplaceChannelTopology { model, topology } => {
                validate_topology(*model, topology, &document.pattern_definitions)
            }
            Self::SetModeledMappingField { channel_id, edit } => {
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
                let mut mapping = topology
                    .channels
                    .iter()
                    .find(|channel| channel.id == *channel_id)
                    .expect("validated channel")
                    .mapping;
                match edit {
                    ModeledMappingFieldEdit::Component(value) => mapping.component = *value,
                    ModeledMappingFieldEdit::Placement(value) => mapping.placement = *value,
                    ModeledMappingFieldEdit::Inverted(value) => mapping.inverted = *value,
                    ModeledMappingFieldEdit::Gain(value) => mapping.gain = *value,
                    ModeledMappingFieldEdit::Bias(value) => mapping.bias = *value,
                }
                validate_source_mapping(mapping)
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
            Self::AddTypedPatternDefinition { definition } => {
                document.pattern_definitions.push(definition.clone());
                return;
            }
            Self::ReplaceSelectedChannelDefinitionTopology {
                channel_id,
                definition,
                ..
            } => {
                document.pattern_definitions.push(definition.clone());
                document.retarget_channel(*channel_id, definition.id);
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
                    let remapped_edit = remap_definition_edit_for_duplicate(&source, &clone, edit);
                    apply_definition_edit(&mut clone, &remapped_edit);
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
        let canvas = document.canvas.clone();
        match &mut document.channel_configuration {
            ChannelConfiguration::Legacy(_) => {
                let channel = document
                    .legacy_channel_mut(self.channel_id())
                    .expect("validated command must target an existing legacy channel");
                match self {
                    Self::SetDensityAxis {
                        edited_axis, value, ..
                    } => {
                        set_density_axis(&mut channel.layout.density, &canvas, *edited_axis, *value)
                            .expect("validated density axis")
                    }
                    Self::SetDensityAspectLock { aspect_locked, .. } => {
                        if *aspect_locked {
                            let paired = derive_density_axis(
                                &canvas,
                                channel.layout.density.across_x,
                                DensityEditedAxis::AcrossX,
                            )
                            .expect("validated canvas");
                            channel.layout.density.across_y = paired;
                        }
                        channel.layout.density.aspect_locked = *aspect_locked;
                    }
                    Self::SetRotation {
                        rotation_degrees, ..
                    } => channel.layout.rotation_degrees = *rotation_degrees,
                    Self::SetTranslationAxis {
                        edited_axis, value, ..
                    } => match edited_axis {
                        TranslationEditedAxis::X => channel.layout.translation_x = *value,
                        TranslationEditedAxis::Y => channel.layout.translation_y = *value,
                    },
                    Self::SetMarkGeometryField { edit, .. } => match edit {
                        MarkGeometryFieldEdit::MinimumSize(value) => {
                            channel.mark_geometry_response.minimum_size = *value
                        }
                        MarkGeometryFieldEdit::MaximumSize(value) => {
                            channel.mark_geometry_response.maximum_size = *value
                        }
                    },
                    Self::SetColorComponent {
                        component, value, ..
                    } => set_color_component(&mut channel.appearance.color, *component, *value),
                    Self::SetOpacity { opacity, .. } => channel.appearance.opacity = *opacity,
                    Self::SetVisibility { visible, .. } => channel.appearance.visible = *visible,
                    Self::SetLegacyMappingField { edit, .. } => match edit {
                        LegacyMappingFieldEdit::Component(value) => {
                            channel.source_mapping.component = *value
                        }
                        LegacyMappingFieldEdit::Placement(value) => {
                            channel.source_mapping.placement = *value
                        }
                    },
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
                    Self::SetDensityAxis {
                        edited_axis, value, ..
                    } => {
                        set_density_axis(&mut channel.layout.density, &canvas, *edited_axis, *value)
                            .expect("validated density axis")
                    }
                    Self::SetDensityAspectLock { aspect_locked, .. } => {
                        if *aspect_locked {
                            let paired = derive_density_axis(
                                &canvas,
                                channel.layout.density.across_x,
                                DensityEditedAxis::AcrossX,
                            )
                            .expect("validated canvas");
                            channel.layout.density.across_y = paired;
                        }
                        channel.layout.density.aspect_locked = *aspect_locked;
                    }
                    Self::SetRotation {
                        rotation_degrees, ..
                    } => channel.layout.rotation_degrees = *rotation_degrees,
                    Self::SetTranslationAxis {
                        edited_axis, value, ..
                    } => match edited_axis {
                        TranslationEditedAxis::X => channel.layout.translation_x = *value,
                        TranslationEditedAxis::Y => channel.layout.translation_y = *value,
                    },
                    Self::SetMarkGeometryField { edit, .. } => match edit {
                        MarkGeometryFieldEdit::MinimumSize(value) => {
                            channel.mark_geometry_response.minimum_size = *value
                        }
                        MarkGeometryFieldEdit::MaximumSize(value) => {
                            channel.mark_geometry_response.maximum_size = *value
                        }
                    },
                    Self::SetColorComponent {
                        component, value, ..
                    } => {
                        let ChannelPaint::Solid(color) = &mut channel.paint else {
                            unreachable!("validated solid paint");
                        };
                        set_color_component(color, *component, *value);
                    }
                    Self::SetOpacity { opacity, .. } => channel.opacity = *opacity,
                    Self::SetVisibility { visible, .. } => channel.visible = *visible,
                    Self::SetModeledMappingField { edit, .. } => match edit {
                        ModeledMappingFieldEdit::Component(value) => {
                            channel.mapping.component = *value
                        }
                        ModeledMappingFieldEdit::Placement(value) => {
                            channel.mapping.placement = *value
                        }
                        ModeledMappingFieldEdit::Inverted(value) => {
                            channel.mapping.inverted = *value
                        }
                        ModeledMappingFieldEdit::Gain(value) => channel.mapping.gain = *value,
                        ModeledMappingFieldEdit::Bias(value) => channel.mapping.bias = *value,
                    },
                    Self::SetChannelPaint { paint, .. } => channel.paint = paint.clone(),
                    Self::SetLegacyMappingField { .. } => {
                        unreachable!("legacy mapping is rejected for modeled state")
                    }
                    _ => unreachable!("handled before configuration mutation"),
                }
            }
        }
    }

    fn result_for_transition(&self, before: &Document, after: &Document) -> CommandResult {
        let invalidation = match self.field_classification() {
            DocumentCommandFieldClassification::DescriptorBacked(projections) => {
                let projection = projections
                    .first()
                    .expect("descriptor-backed command has one field projection");
                property_field_contract(projection.field).invalidation
            }
            DocumentCommandFieldClassification::NonField(operation) => match operation {
                NonFieldCommandOperation::AddPatternDefinition
                | NonFieldCommandOperation::AddTypedPatternDefinition
                | NonFieldCommandOperation::ReplaceSelectedChannelDefinitionTopology
                | NonFieldCommandOperation::DuplicatePatternDefinition
                | NonFieldCommandOperation::RemoveUnreferencedPatternDefinition => {
                    InvalidationLevel::Family
                }
                NonFieldCommandOperation::ReplaceChannelTopology => {
                    InvalidationLevel::ChannelTopology
                }
            },
        };
        CommandResult {
            affected_channels: match self {
                Self::AddPatternDefinition { .. }
                | Self::AddTypedPatternDefinition { .. }
                | Self::DuplicatePatternDefinition { .. }
                | Self::RemoveUnreferencedPatternDefinition { .. } => Vec::new(),
                Self::RetargetChannelPatternDefinition { channel_id, .. }
                | Self::EditSelectedChannelPatternDefinition { channel_id, .. } => {
                    vec![*channel_id]
                }
                Self::EditSharedPatternDefinition { .. } => Vec::new(),
                Self::SetSourceReference { .. } => after.channel_ids(),
                Self::ReplaceChannelTopology { topology, .. } => {
                    affected_topology_channels(before.channel_ids(), topology)
                }
                _ => vec![self.channel_id()],
            },
            invalidation,
        }
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
        let (candidate, result) = self.document.apply_command(command)?;
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
