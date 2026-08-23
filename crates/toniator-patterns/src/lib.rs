#![forbid(unsafe_code)]

//! Deterministic straight-guide family evaluation.

use std::{collections::BTreeMap, error::Error, fmt};

use serde::Serialize;
use toniator_domain::{
    ArtworkWeightResponse, AuthoredStructureId, CanvasSpec, ChannelId, DensityMetric2D, Document,
    DocumentCommand, DocumentHistory, DocumentSessionError, GuideDimension, GuideDimensionDraft,
    GuideDimensionId, GuidePrototype, GuideRepetition, MarkOrientation, MarkOrientationDraft,
    MarkPrototype, OffsetCleanup, OffsetSides, ParametricCurve, PatternDefinition,
    PatternDefinitionRecipe, PatternFamily, PatternMechanism, PatternMechanismId,
    PatternModulation, PatternOutputLayer, PatternOutputLayerId, PresetMetadata, PresetRecord,
    RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy, SourceMapping,
    StraightGuideDimension, VisibleMarkSizingPolicy,
};
pub use toniator_geometry::{
    AffineTransform2D, Bounds, CanonicalCircleMark, CanonicalFillRule, CanonicalMark,
    CanonicalPathMark, CanonicalStroke, CubicBezierSegment, CurveError, CurvePath, CurveSegment,
    FamilySite, FamilySiteError, FamilySiteId, FamilySiteProvenance, FamilySiteSet,
    GuideInstanceId, GuideIntersectionProvenance, IntersectionSite, NominalCellBasis,
    PATH_OFFSET_ALGORITHM_CONTRACT_ID, PathClosure, PathLocation, PathOffsetCleanup,
    PathOffsetEndpointPolicy, PathOffsetLimits, PathOffsetRequest, PathOffsetResult, Point2,
    SiteAdjacencyError, SiteAdjacencyGraph, SiteAdjacencyLimits, SiteAdjacencyPolicy, SiteId,
    SiteScope, StraightGuide, StrokeProfileSample, StructuralPathInstance,
    StructuralPathInstanceId, StructuralPathLocationProvenance, StructuralPathSet,
    StructuralPathSourceId, VariableWidthOutlineLimits, VariableWidthPathSample, Vector2,
    build_site_adjacency_cancellable, build_variable_width_outline_cancellable,
    offset_path_cancellable, projection_range, resolve_guide_prototype,
};
use toniator_sampling::{
    SampledSourcePaint, SamplingError, SourceComponent, SourceField, SourceMappingComponent,
    SourcePlacement,
};

/// The finite antialiasing envelope included in every Stage 3 generation plan.
pub const ANTIALIAS_MARGIN: f64 = 1.0;

/// Stable IDs for the two fixed rectangular straight-guide dimensions.
pub const FIRST_DIMENSION_ID: GuideDimensionId = GuideDimensionId(1);
pub const SECOND_DIMENSION_ID: GuideDimensionId = GuideDimensionId(2);

/// The stable version of the built-in registry ordering and metadata contract.
pub const BUNDLED_PRESET_REGISTRY_VERSION: u32 = 1;

/// Immutable, deterministic preset registry owned by the pattern/schema layer.
#[derive(Clone, Debug, PartialEq)]
pub struct PresetRegistry {
    version: u32,
    entries: Vec<PresetRecord>,
}

#[cfg(test)]
mod stage20e2_shape_work_tests {
    use std::cell::Cell;

    use super::*;
    use toniator_geometry::{LineSegment, PathClosure};

    /// Builds a finite four-segment closed path for cancellation and normalization checks.
    fn square_path() -> CurvePath {
        let points = [
            Point2::new(-1.0, -1.0),
            Point2::new(1.0, -1.0),
            Point2::new(1.0, 1.0),
            Point2::new(-1.0, 1.0),
        ];
        CurvePath::new(
            (0..4)
                .map(|index| {
                    CurveSegment::Line(
                        LineSegment::new(points[index], points[(index + 1) % 4])
                            .expect("square endpoints are finite"),
                    )
                })
                .collect(),
            PathClosure::Closed,
        )
        .expect("the square is exactly closed")
    }

    /// Fixes inclusive transformed-segment limits plus zero, overflow, and over-limit errors.
    #[test]
    fn transformed_segment_preflight_is_exact_and_checked() {
        assert_eq!(
            preflight_transformed_curve_segment_instances(4, 8, 32)
                .expect("the exact limit is inclusive"),
            32
        );
        for result in [
            preflight_transformed_curve_segment_instances(4, 8, 31),
            preflight_transformed_curve_segment_instances(4, 8, 0),
            preflight_transformed_curve_segment_instances(usize::MAX, 2, usize::MAX),
        ] {
            assert_eq!(
                result.expect_err("invalid bounded work must fail").path(),
                "realization.mark.segment_limit"
            );
        }
    }

    /// Proves both prototype scanning and per-site segment transformation poll cancellation.
    #[test]
    fn closed_shape_segment_work_cancels_without_returning_partial_geometry() {
        let path = square_path();
        let checks = Cell::new(0_u32);
        let error = path_reference_radius(&path, Point2::new(0.0, 0.0), &|| {
            let next = checks.get() + 1;
            checks.set(next);
            next >= 2
        })
        .expect_err("prototype scanning must observe cancellation");
        assert_eq!(error.path(), "evaluation.cancelled");

        checks.set(0);
        let error = transform_closed_shape(
            &path,
            Point2::new(0.0, 0.0),
            2.0,
            30.0,
            Point2::new(10.0, 20.0),
            &|| {
                let next = checks.get() + 1;
                checks.set(next);
                next >= 3
            },
        )
        .expect_err("segment transformation must observe cancellation");
        assert_eq!(error.path(), "evaluation.cancelled");
        assert_eq!(checks.get(), 3);
    }
}

/// Validation failure for pure metadata/registry structure before any document
/// command is constructed or history state can change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresetRegistryError {
    message: String,
}

/// A deliberate, non-mutating shared replacement proposal. Callers inspect
/// `affected_channels` before explicitly confirming the history transition.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedSharedPresetReplacement {
    definition_id: toniator_domain::PatternDefinitionId,
    base_definition: PatternDefinition,
    recipe: PatternDefinitionRecipe,
    affected_channels: Vec<ChannelId>,
}

impl PreparedSharedPresetReplacement {
    /// Returns every linked channel in authoritative document order before any
    /// shared definition mutation has been dispatched.
    pub fn affected_channels(&self) -> &[ChannelId] {
        &self.affected_channels
    }

    /// Confirms the previously disclosed replacement through `DocumentHistory`.
    /// The immutable base keeps stale/no-op validation and undo/redo semantics
    /// identical to every other authoritative shared transition. A changed
    /// linked-channel set is rejected before command dispatch so confirmation
    /// cannot mutate a different scope than the caller disclosed.
    pub fn confirm(
        self,
        history: &mut DocumentHistory,
    ) -> Result<toniator_domain::CommandResult, DocumentSessionError> {
        if history.document().linked_channels(self.definition_id) != self.affected_channels {
            return Err(DocumentSessionError::Validation(
                toniator_domain::ValidationError::new(
                    "preset.shared.affected_channels",
                    "shared preset replacement disclosure is stale",
                ),
            ));
        }
        history.apply(&DocumentCommand::ReplaceSharedPatternDefinitionRecipe {
            definition_id: self.definition_id,
            base_definition: self.base_definition,
            recipe: self.recipe,
        })
    }
}

impl PresetRegistryError {
    /// Returns the stable human-readable validation failure.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for PresetRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for PresetRegistryError {}

impl PresetRegistry {
    /// Builds a validated registry with caller-defined deterministic entry order.
    pub fn new(version: u32, entries: Vec<PresetRecord>) -> Result<Self, PresetRegistryError> {
        if version == 0 {
            return Err(PresetRegistryError {
                message: "preset registry version must be nonzero".into(),
            });
        }
        let mut previous = None;
        for entry in &entries {
            toniator_domain::validate_preset_record(entry).map_err(|error| {
                PresetRegistryError {
                    message: error.to_string(),
                }
            })?;
            for (path, value) in [
                ("id", entry.metadata.id.as_str()),
                ("name", entry.metadata.name.as_str()),
                ("category", entry.metadata.category.as_str()),
                ("description", entry.metadata.description.as_str()),
            ] {
                if value.trim().is_empty() || value.chars().any(char::is_control) {
                    return Err(PresetRegistryError {
                        message: format!("preset metadata {path} must be nonempty printable text"),
                    });
                }
            }
            if entry
                .metadata
                .thumbnail
                .as_deref()
                .is_some_and(|value| value.trim().is_empty() || value.chars().any(char::is_control))
            {
                return Err(PresetRegistryError {
                    message: "preset thumbnail must be printable text when present".into(),
                });
            }
            if previous.is_some_and(|id: &str| id >= entry.metadata.id.as_str()) {
                return Err(PresetRegistryError {
                    message: "preset IDs must be unique and strictly sorted".into(),
                });
            }
            previous = Some(entry.metadata.id.as_str());
        }
        Ok(Self { version, entries })
    }

    /// Returns the versioned built-in pure-schema registry in stable ID order.
    pub fn bundled() -> Self {
        Self::new(
            BUNDLED_PRESET_REGISTRY_VERSION,
            vec![
                PresetRecord {
                    metadata: PresetMetadata {
                        id: "even-random-circles".into(),
                        name: "Even Random Circles".into(),
                        category: "Random sites".into(),
                        description: "Evenly separated deterministic circular marks.".into(),
                        thumbnail: Some("builtin:even-random-circles".into()),
                    },
                    recipe: PatternDefinitionRecipe::RandomSites {
                        name: "Even random circles".into(),
                        coverage: toniator_domain::CoveragePolicy {
                            guard_steps: 2,
                            additional_margin: 0.0,
                        },
                        character: RandomSiteCharacter::Even {
                            minimum_center_distance: 8.0,
                        },
                        seed: 19,
                        density_modulation: SiteDensityModulation::Uniform,
                        exclusion: SiteExclusionPolicy::None,
                        maximum_attempts: 16_000_000,
                        maximum_neighbor_checks: 16_000_000,
                    },
                },
                PresetRecord {
                    metadata: PresetMetadata {
                        id: "straight-grid-circles".into(),
                        name: "Straight Grid Circles".into(),
                        category: "Guides".into(),
                        description: "Two rotated, offset straight guides with circular marks."
                            .into(),
                        thumbnail: Some("builtin:straight-grid-circles".into()),
                    },
                    recipe: PatternDefinitionRecipe::GeneralizedStraightGuides {
                        name: "Straight grid circles".into(),
                        coverage: toniator_domain::CoveragePolicy {
                            guard_steps: 2,
                            additional_margin: 0.0,
                        },
                        dimensions: vec![
                            GuideDimensionDraft {
                                baseline_angle_degrees: 17.0,
                                phase: 0.23,
                                spacing_multiplier: 0.82,
                            },
                            GuideDimensionDraft {
                                baseline_angle_degrees: 107.0,
                                phase: -0.31,
                                spacing_multiplier: 1.18,
                            },
                        ],
                        product: toniator_domain::GeneralizedSiteProductDraft::Intersections {
                            dimension_indices: vec![0, 1],
                            merge_epsilon: 1e-9,
                        },
                        orientation: MarkOrientationDraft::Fixed,
                    },
                },
            ],
        )
        .expect("bundled preset literals satisfy registry validation")
    }

    /// Returns this registry format version without exposing mutable state.
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns records in their validated stable order.
    pub fn entries(&self) -> &[PresetRecord] {
        &self.entries
    }

    /// Finds one entry by its stable metadata ID.
    pub fn find(&self, id: &str) -> Option<&PresetRecord> {
        self.entries
            .binary_search_by(|entry| entry.metadata.id.as_str().cmp(id))
            .ok()
            .map(|index| &self.entries[index])
    }

    /// Returns an ID-free reconstruction recipe for one preset shortcut.
    pub fn reconstruct(&self, id: &str) -> Option<PatternDefinitionRecipe> {
        self.find(id).map(|entry| entry.recipe.clone())
    }

    /// Applies a preset as a fresh document-owned definition and retargets only
    /// the selected channel through the authoritative history boundary.
    pub fn apply_to_selected(
        &self,
        history: &mut DocumentHistory,
        channel_id: ChannelId,
        id: &str,
    ) -> Result<toniator_domain::CommandResult, DocumentSessionError> {
        let recipe = self
            .find(id)
            .ok_or_else(|| {
                DocumentSessionError::Validation(toniator_domain::ValidationError::new(
                    "preset.id",
                    "preset ID is not present in this registry",
                ))
            })?
            .recipe
            .clone();
        let base_definition = history
            .document()
            .pattern_definition_for(channel_id)
            .expect("document history preserves channel references")
            .clone();
        history.apply(
            &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base: history.document().pattern_settings().clone(),
                channel_id,
                base_definition,
                recipe,
            },
        )
    }

    /// Materializes a preset as a fresh document-base definition.  The preset
    /// remains ID-free; the domain allocates the definition and validates every
    /// retained channel delta atomically.
    pub fn apply_to_document_base(
        &self,
        history: &mut DocumentHistory,
        id: &str,
    ) -> Result<toniator_domain::CommandResult, DocumentSessionError> {
        let recipe = self
            .find(id)
            .ok_or_else(|| {
                DocumentSessionError::Validation(toniator_domain::ValidationError::new(
                    "preset.id",
                    "preset ID is not present in this registry",
                ))
            })?
            .recipe
            .clone();
        let base = history.document().pattern_settings().clone();
        let base_definition = history
            .document()
            .pattern_definitions()
            .iter()
            .find(|definition| definition.id == base.definition_id)
            .expect("validated document base definition")
            .clone();
        history.apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe,
        })
    }

    /// Prepares a shared replacement without mutating the document so callers
    /// can disclose every affected channel before explicit confirmation.
    pub fn prepare_shared_replacement(
        &self,
        history: &DocumentHistory,
        definition_id: toniator_domain::PatternDefinitionId,
        id: &str,
    ) -> Result<PreparedSharedPresetReplacement, DocumentSessionError> {
        let recipe = self
            .find(id)
            .ok_or_else(|| {
                DocumentSessionError::Validation(toniator_domain::ValidationError::new(
                    "preset.id",
                    "preset ID is not present in this registry",
                ))
            })?
            .recipe
            .clone();
        let base_definition = history
            .document()
            .pattern_definitions()
            .iter()
            .find(|definition| definition.id == definition_id)
            .ok_or_else(|| {
                DocumentSessionError::Validation(toniator_domain::ValidationError::new(
                    "pattern_definitions.id",
                    "shared preset replacement targets a missing definition",
                ))
            })?
            .clone();
        let affected_channels = history.document().linked_channels(definition_id);
        Ok(PreparedSharedPresetReplacement {
            definition_id,
            base_definition,
            recipe,
            affected_channels,
        })
    }
}

/// The structural product a family makes available to later pipeline stages.
/// It is deliberately typed rather than inferred from a pattern name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructuralProductCapability {
    GuideIntersections,
    AlongGuideSites,
    RandomSites,
    /// One ordered finite parametric source consumed directly by connected realization.
    ParametricPaths,
}

/// A stable record of the typed mechanisms that produced a structural product.
/// It travels beside geometry without changing the accepted Stage 3 geometry
/// fingerprint, so current artifacts remain byte-for-byte equivalent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructuralProductProvenance {
    pub definition_id: u64,
    pub family_capability: StructuralProductCapability,
    pub mechanism_ids: Vec<PatternMechanismId>,
}

/// A reusable family contract resolved from the document's typed definition.
#[derive(Clone, Debug, PartialEq)]
pub struct FamilyCapability {
    pub product: StructuralProductCapability,
    pub provenance: StructuralProductProvenance,
    pub dimensions: Vec<StraightGuideDimension>,
    pub site_selection: Vec<GuideDimensionId>,
    pub merge_epsilon: Option<f64>,
    pub along_interval_multiplier: Option<f64>,
    pub along_phase: Option<f64>,
    pub random: Option<RandomSiteCapability>,
    /// Document-resolved generic guide capability.  Legacy definition-only plans leave this absent.
    pub generic_guides: Option<GenericGuideCapability>,
    /// Analytic intent retained until this crate converts it to a canonical finite CurvePath.
    pub parametric_curve: Option<ParametricCurveCapability>,
}

/// Resolved finite parametric source intent; it never stores generated geometry or sites.
#[derive(Clone, Debug, PartialEq)]
pub struct ParametricCurveCapability {
    pub source_id: toniator_geometry::StructuralPathSourceId,
    pub curve: ParametricCurve,
    pub repetition: GuideRepetition,
    pub site_interval: Option<f64>,
    pub site_phase: Option<f64>,
}

/// Resolved document-owned or procedural guide prototypes retained only for one family evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct GenericGuideCapability {
    pub dimensions: Vec<GuideDimension>,
    pub resolved_paths: Vec<(Option<AuthoredStructureId>, CurvePath)>,
    /// Optional neutral source used by adapters that reuse finite-path mechanics without becoming guides.
    pub structural_source: Option<StructuralPathSourceId>,
    /// Optional absolute along-path interval retained by the parametric adapter.
    pub absolute_site_interval: Option<f64>,
    /// Optional authored radial spacing used as the normal basis for a single parametric path.
    pub single_nominal_spacing: Option<f64>,
}

/// Resolved Stage 16B structural chain.  The source-dependent modulation is
/// explicit, keeping independent random/even/clustered families source-free.
#[derive(Clone, Debug, PartialEq)]
pub struct RandomSiteCapability {
    pub character: RandomSiteCharacter,
    pub seed: u32,
    pub density_modulation: SiteDensityModulation,
    pub exclusion: SiteExclusionPolicy,
    pub maximum_attempts: u32,
    /// Explicit, persisted bound for deterministic spatial-index neighbor
    /// checks. It is distinct from candidate generation work.
    pub maximum_neighbor_checks: u32,
}

/// Whether the family identity must include decoded source pixels.  Logical
/// source IDs remain solely at the decoder lookup boundary.
pub fn family_requires_decoded_source(family: &FamilyCapability) -> bool {
    matches!(
        family
            .random
            .as_ref()
            .map(|random| &random.density_modulation),
        Some(SiteDensityModulation::ArtworkWeighted { .. })
    )
}

/// A reusable ordered realization contract. A realizer can consume only the
/// declared structural product, before any source sampling or geometry output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputCapability {
    pub layer_id: PatternOutputLayerId,
    pub consumes: StructuralProductCapability,
    /// The mutually exclusive output authority; guide paths never masquerade as marks.
    pub payload: OutputCapabilityPayload,
}

/// Typed realization payload retained in cache/provenance identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputCapabilityPayload {
    Marks {
        prototype: MarkPrototype,
        orientation: MarkOrientation,
    },
    GuidePaths {
        guide_mechanism_id: PatternMechanismId,
        style: toniator_domain::PathStrokeStyle,
    },
}

impl OutputCapability {
    /// Returns mark authority only when this output is a mark layer.
    pub fn marks(&self) -> Option<(&MarkPrototype, &MarkOrientation)> {
        match &self.payload {
            OutputCapabilityPayload::Marks {
                prototype,
                orientation,
            } => Some((prototype, orientation)),
            OutputCapabilityPayload::GuidePaths { .. } => None,
        }
    }

    /// Returns guide-path authority only when this output is a connected path layer.
    pub fn guide_paths(&self) -> Option<(PatternMechanismId, toniator_domain::PathStrokeStyle)> {
        match self.payload {
            OutputCapabilityPayload::Marks { .. } => None,
            OutputCapabilityPayload::GuidePaths {
                guide_mechanism_id,
                style,
            } => Some((guide_mechanism_id, style)),
        }
    }
}

/// Typed family/modulation/output plan. Modulation has no variants in the
/// accepted schema, but remains an explicit stage instead of a hidden no-op in
/// family or renderer code.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternPipelinePlan {
    pub family: FamilyCapability,
    pub modulation: PatternModulation,
    pub ordered_outputs: Vec<OutputCapability>,
}

/// A family result flowing into modulation and ordered realization. Renderers
/// consume only the canonical marks emitted by realizers, never this value.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedFamilyOutput {
    family: FamilyCapability,
    sites: FamilySiteSet,
    diagnostics: Option<RandomSiteDiagnostics>,
    structure: TypedFamilyStructure,
}

/// One caller-requested topology result paired with the complete family envelope that produced it.
///
/// The family stays a normal cacheable product while adjacency remains caller-supplied derived data.
#[derive(Clone, Debug, PartialEq)]
pub struct SiteAdjacencyEvaluation {
    pub family: TypedFamilyOutput,
    pub graph: SiteAdjacencyGraph,
}

/// Truthful non-site metadata retained for the current circle compatibility adapter.
#[derive(Clone, Debug, PartialEq)]
struct TypedFamilyStructure {
    coverage: Vec<GuideCoverage>,
    guides: Vec<StraightGuide>,
    support_radius: f64,
    guard_steps: u32,
    antialias_margin: f64,
    generation_domain: Bounds,
    structural_path_set: Option<StructuralPathSet>,
    /// Per-guide nominal spacing retained with family output so realization never uses a global axis proxy.
    guide_nominal_bases: BTreeMap<StructuralPathInstanceId, f64>,
}

impl TypedFamilyOutput {
    /// Returns the immutable structural capability that produced this derived output.
    pub fn family(&self) -> &FamilyCapability {
        &self.family
    }

    /// Returns the existing family identity; site interchange adds no cache identity.
    pub fn family_fingerprint(&self) -> &str {
        self.sites.family_fingerprint()
    }

    /// Returns the sole truthful reusable site authority for this family result.
    pub fn site_set(&self) -> &FamilySiteSet {
        &self.sites
    }

    /// Returns bounded random-process diagnostics only for random-site products.
    pub fn random_diagnostics(&self) -> Option<&RandomSiteDiagnostics> {
        self.diagnostics.as_ref()
    }

    /// Returns Stage 20D's truthful reusable finite guide authority when this family produces it.
    pub fn structural_path_set(&self) -> Option<&StructuralPathSet> {
        self.structure.structural_path_set.as_ref()
    }

    /// Returns the family-resolved nominal basis for one emitted guide path.
    pub fn guide_nominal_basis(&self, path_id: StructuralPathInstanceId) -> Option<f64> {
        self.structure.guide_nominal_bases.get(&path_id).copied()
    }

    /// Returns retained straight-guide authority for per-site mark orientation.
    ///
    /// This exposes derived family data read-only; realization may consume it
    /// but never synthesize a guide for a site missing the requested contributor.
    pub fn straight_guides(&self) -> &[StraightGuide] {
        &self.structure.guides
    }

    /// Returns the conservative support radius used to allocate this immutable family envelope.
    pub fn planned_support_radius(&self) -> f64 {
        self.structure.support_radius
    }

    /// Returns the complete guard-step count retained by the immutable family envelope.
    pub const fn guard_steps(&self) -> u32 {
        self.structure.guard_steps
    }
}

/// Provenance that survives explicit modulation and ordered output realization.
/// It is intentionally adjacent to, rather than mixed into, canonical marks.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedRealizationProvenance {
    pub structural: StructuralProductProvenance,
    pub modulation: PatternModulation,
    pub ordered_output_layer_ids: Vec<PatternOutputLayerId>,
    pub ordered_output_prototypes: Vec<MarkPrototype>,
    pub ordered_output_orientations: Vec<MarkOrientation>,
    /// The exclusive truthful structural authority consumed by this realization.
    pub structural_input: RealizationStructuralInput,
}

/// Tagged realization input proving whether an output consumed sites or raw ordered structural paths.
#[derive(Clone, Debug, PartialEq)]
pub enum RealizationStructuralInput {
    /// Mark outputs consume only evaluator-published sites.
    Sites(FamilySiteSet),
    /// Connected path outputs consume ordered raw structural paths and family-resolved bases.
    StructuralPaths {
        paths: StructuralPathSet,
        nominal_bases: BTreeMap<StructuralPathInstanceId, f64>,
    },
}

/// A realization plus its typed provenance. Renderers consume `output` only.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedRealization<T> {
    pub provenance: TypedRealizationProvenance,
    pub output: T,
}

/// Stable typed diagnostic emitted before family output, cache publication, or
/// partial realization when a definition cannot form one compatible pipeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternPipelineError {
    path: &'static str,
    message: &'static str,
}

impl PatternPipelineError {
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

impl fmt::Display for PatternPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for PatternPipelineError {}

impl From<CurveError> for PatternPipelineError {
    /// Preserves the stable Stage 20B curve diagnostic at the typed pipeline boundary.
    fn from(error: CurveError) -> Self {
        Self::new(error.path(), error.message())
    }
}

impl From<GridError> for PatternPipelineError {
    /// Preserves existing grid/coverage diagnostics when shared spacing helpers fail.
    fn from(error: GridError) -> Self {
        Self::new(error.path(), error.message())
    }
}

/// Resolve the typed capability graph in declared order. This is the one
/// family/output compatibility boundary shared by document and diagnostic
/// evaluation; unsupported combinations fail before any source decode or
/// cache transaction can occur.
///
/// # Errors
///
/// Returns stable family, mechanism, output, or capability diagnostics.
pub fn resolve_pattern_pipeline(
    definition: &PatternDefinition,
) -> Result<PatternPipelinePlan, PatternPipelineError> {
    if definition
        .mechanisms
        .iter()
        .any(|mechanism| matches!(mechanism, PatternMechanism::GuideDimensions { .. }))
    {
        return Err(PatternPipelineError::new(
            "pattern.pipeline.guide_resources",
            "document-owned guide resources require document-aware pipeline resolution",
        ));
    }
    if matches!(definition.family, PatternFamily::RandomSites { .. }) {
        return resolve_random_site_pipeline(definition);
    }
    if matches!(definition.family, PatternFamily::ParametricCurve { .. }) {
        return resolve_parametric_curve_pipeline(definition);
    }
    let PatternFamily::GuideIntersections {
        guide_mechanism_id,
        site_mechanism_id,
    } = definition.family
    else {
        unreachable!("random-site families return through their dedicated resolver");
    };
    let (
        ordered_mechanisms,
        product,
        dimensions,
        site_selection,
        merge_epsilon,
        along_interval_multiplier,
        along_phase,
    ) = match definition.mechanisms.as_slice() {
        [
            PatternMechanism::StraightGuides { id },
            PatternMechanism::GuideIntersections {
                id: intersection_id,
                guide_mechanism_id: parent_id,
            },
        ] if *id == guide_mechanism_id
            && *intersection_id == site_mechanism_id
            && *parent_id == guide_mechanism_id =>
        {
            (
                vec![*id, *intersection_id],
                StructuralProductCapability::GuideIntersections,
                vec![
                    StraightGuideDimension {
                        id: FIRST_DIMENSION_ID,
                        baseline_angle_degrees: 0.0,
                        phase: 0.0,
                        repetition: toniator_domain::StraightGuideRepetition {
                            spacing_multiplier: 1.0,
                        },
                    },
                    StraightGuideDimension {
                        id: SECOND_DIMENSION_ID,
                        baseline_angle_degrees: 90.0,
                        phase: 0.0,
                        repetition: toniator_domain::StraightGuideRepetition {
                            spacing_multiplier: 1.0,
                        },
                    },
                ],
                vec![FIRST_DIMENSION_ID, SECOND_DIMENSION_ID],
                Some(0.0),
                None,
                None,
            )
        }
        [
            PatternMechanism::StraightGuideDimensions { id, dimensions },
            PatternMechanism::SelectedGuideIntersections {
                id: site_id,
                guide_mechanism_id: parent_id,
                dimensions: selected,
                merge_epsilon,
            },
        ] if *id == guide_mechanism_id
            && *site_id == site_mechanism_id
            && *parent_id == guide_mechanism_id =>
        {
            (
                vec![*id, *site_id],
                StructuralProductCapability::GuideIntersections,
                dimensions.clone(),
                selected.clone(),
                Some(*merge_epsilon),
                None,
                None,
            )
        }
        [
            PatternMechanism::StraightGuideDimensions { id, dimensions },
            PatternMechanism::AlongGuideSites {
                id: site_id,
                guide_mechanism_id: parent_id,
                dimensions: selected,
                interval_multiplier,
                phase,
            },
        ] if *id == guide_mechanism_id
            && *site_id == site_mechanism_id
            && *parent_id == guide_mechanism_id =>
        {
            (
                vec![*id, *site_id],
                StructuralProductCapability::AlongGuideSites,
                dimensions.clone(),
                selected.clone(),
                None,
                Some(*interval_multiplier),
                Some(*phase),
            )
        }
        _ => {
            return Err(PatternPipelineError::new(
                "pattern.family.capability",
                "typed family mechanisms cannot produce the declared structural product",
            ));
        }
    };
    let mut ordered_outputs = Vec::with_capacity(definition.output_layers.len());
    for output in &definition.output_layers {
        match output {
            PatternOutputLayer::CircularMarks {
                id,
                site_mechanism_id: source_id,
            } if *source_id == site_mechanism_id => ordered_outputs.push(OutputCapability {
                layer_id: *id,
                consumes: product,
                payload: OutputCapabilityPayload::Marks {
                    prototype: MarkPrototype::Circle,
                    orientation: MarkOrientation::Fixed,
                },
            }),
            PatternOutputLayer::MarkPrototype {
                id,
                site_mechanism_id: source_id,
                prototype,
                orientation,
            } if *source_id == site_mechanism_id => ordered_outputs.push(OutputCapability {
                layer_id: *id,
                consumes: product,
                payload: OutputCapabilityPayload::Marks {
                    prototype: prototype.clone(),
                    orientation: orientation.clone(),
                },
            }),
            PatternOutputLayer::GuidePaths {
                id,
                guide_mechanism_id: source_id,
                style,
            } if *source_id == guide_mechanism_id => ordered_outputs.push(OutputCapability {
                layer_id: *id,
                consumes: product,
                payload: OutputCapabilityPayload::GuidePaths {
                    guide_mechanism_id: *source_id,
                    style: *style,
                },
            }),
            _ => {
                return Err(PatternPipelineError::new(
                    "pattern.output_layers.capability",
                    "output layer cannot consume the declared structural product",
                ));
            }
        }
    }
    if ordered_outputs.len() != 1 {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.capability",
            "the current typed output contract requires exactly one ordered realization layer",
        ));
    }
    Ok(PatternPipelinePlan {
        family: FamilyCapability {
            product,
            provenance: StructuralProductProvenance {
                definition_id: definition.id.0,
                family_capability: product,
                mechanism_ids: ordered_mechanisms,
            },
            dimensions,
            site_selection,
            merge_epsilon,
            along_interval_multiplier,
            along_phase,
            random: None,
            generic_guides: None,
            parametric_curve: None,
        },
        modulation: definition.modulation.clone(),
        ordered_outputs,
    })
}

/// Resolves the bounded Stage 20K source/site chain without deriving geometry.
///
/// # Errors
///
/// Returns a stable source ordering or homogeneous-output diagnostic.
fn resolve_parametric_curve_pipeline(
    definition: &PatternDefinition,
) -> Result<PatternPipelinePlan, PatternPipelineError> {
    let PatternFamily::ParametricCurve {
        curve_mechanism_id,
        site_mechanism_id: declared_site_mechanism_id,
    } = definition.family
    else {
        unreachable!("parametric resolver receives only a parametric family");
    };
    let (curve, repetition, interval, phase, product, mechanism_ids) =
        match (definition.mechanisms.as_slice(), declared_site_mechanism_id) {
            (
                [
                    PatternMechanism::ParametricCurveSource {
                        id,
                        curve,
                        repetition,
                    },
                ],
                None,
            ) if *id == curve_mechanism_id => (
                curve.clone(),
                repetition.clone(),
                None,
                None,
                StructuralProductCapability::ParametricPaths,
                vec![*id],
            ),
            (
                [
                    PatternMechanism::ParametricCurveSource {
                        id,
                        curve,
                        repetition,
                    },
                    PatternMechanism::AlongParametricCurveSites {
                        id: site_id,
                        curve_mechanism_id: parent,
                        interval,
                        phase,
                    },
                ],
                Some(declared_site),
            ) if *id == curve_mechanism_id
                && *site_id == declared_site
                && *parent == curve_mechanism_id =>
            {
                (
                    curve.clone(),
                    repetition.clone(),
                    Some(*interval),
                    Some(*phase),
                    StructuralProductCapability::AlongGuideSites,
                    vec![*id, *site_id],
                )
            }
            _ => {
                return Err(PatternPipelineError::new(
                    "pattern.family.capability",
                    "typed parametric mechanisms cannot produce the declared structural product",
                ));
            }
        };
    let output = match definition.output_layers.as_slice() {
        [
            PatternOutputLayer::ParametricPaths {
                id,
                curve_mechanism_id: source,
                style,
            },
        ] if product == StructuralProductCapability::ParametricPaths
            && *source == curve_mechanism_id =>
        {
            OutputCapability {
                layer_id: *id,
                consumes: product,
                payload: OutputCapabilityPayload::GuidePaths {
                    guide_mechanism_id: *source,
                    style: *style,
                },
            }
        }
        [
            PatternOutputLayer::CircularMarks {
                id,
                site_mechanism_id,
            },
        ] if Some(*site_mechanism_id) == declared_site_mechanism_id => OutputCapability {
            layer_id: *id,
            consumes: product,
            payload: OutputCapabilityPayload::Marks {
                prototype: MarkPrototype::Circle,
                orientation: MarkOrientation::Fixed,
            },
        },
        [
            PatternOutputLayer::MarkPrototype {
                id,
                site_mechanism_id,
                prototype,
                orientation: MarkOrientation::Fixed,
            },
        ] if Some(*site_mechanism_id) == declared_site_mechanism_id => OutputCapability {
            layer_id: *id,
            consumes: product,
            payload: OutputCapabilityPayload::Marks {
                prototype: prototype.clone(),
                orientation: MarkOrientation::Fixed,
            },
        },
        _ => {
            return Err(PatternPipelineError::new(
                "pattern.output_layers.capability",
                "parametric output cannot consume the declared structural product",
            ));
        }
    };
    Ok(PatternPipelinePlan {
        family: FamilyCapability {
            product,
            provenance: StructuralProductProvenance {
                definition_id: definition.id.0,
                family_capability: product,
                mechanism_ids,
            },
            dimensions: Vec::new(),
            site_selection: Vec::new(),
            merge_epsilon: None,
            along_interval_multiplier: None,
            along_phase: phase,
            random: None,
            generic_guides: None,
            parametric_curve: Some(ParametricCurveCapability {
                source_id: toniator_geometry::StructuralPathSourceId::ParametricCurve(
                    curve_mechanism_id,
                ),
                curve,
                repetition,
                site_interval: interval,
                site_phase: phase,
            }),
        },
        modulation: definition.modulation.clone(),
        ordered_outputs: vec![output],
    })
}

/// Resolves a typed family with document-owned generic guide resources before family allocation.
///
/// # Errors
///
/// Returns the established document resource or pipeline diagnostic without consulting global state.
pub fn resolve_document_pattern_pipeline(
    document: &Document,
    definition: &PatternDefinition,
) -> Result<PatternPipelinePlan, PatternPipelineError> {
    let Some((guide_id, dimensions)) =
        definition
            .mechanisms
            .iter()
            .find_map(|mechanism| match mechanism {
                PatternMechanism::GuideDimensions { id, dimensions } => Some((*id, dimensions)),
                _ => None,
            })
    else {
        return resolve_pattern_pipeline(definition);
    };
    let mut surrogate = definition.clone();
    let generic_dimensions = dimensions.to_vec();
    let replacement = PatternMechanism::StraightGuideDimensions {
        id: guide_id,
        dimensions: generic_dimensions
            .iter()
            .map(|dimension| StraightGuideDimension {
                id: dimension.id,
                baseline_angle_degrees: dimension.baseline_angle_degrees,
                phase: dimension.phase,
                repetition: toniator_domain::StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            })
            .collect(),
    };
    let target = surrogate
        .mechanisms
        .iter_mut()
        .find(|mechanism| mechanism.id() == guide_id)
        .expect("generic root was found in cloned definition");
    *target = replacement;
    let mut plan = resolve_pattern_pipeline(&surrogate)?;
    let mut resolved_paths = Vec::with_capacity(generic_dimensions.len());
    for dimension in &generic_dimensions {
        let structure = match dimension.prototype {
            GuidePrototype::AuthoredOpenPath { structure_id } => {
                document.authored_structure(structure_id)
            }
            GuidePrototype::CircularArc { .. } => None,
        };
        let path = resolve_guide_prototype(&dimension.prototype, structure).map_err(|_| {
            PatternPipelineError::new(
                "pattern.pipeline.guide_resources",
                "document-owned guide resources require document-aware pipeline resolution",
            )
        })?;
        let source_id = match dimension.prototype {
            GuidePrototype::AuthoredOpenPath { structure_id } => Some(structure_id),
            _ => None,
        };
        resolved_paths.push((source_id, path));
    }
    plan.family.generic_guides = Some(GenericGuideCapability {
        dimensions: generic_dimensions,
        resolved_paths,
        structural_source: None,
        absolute_site_interval: None,
        single_nominal_spacing: None,
    });
    Ok(plan)
}

/// Resolves the fixed random mechanism chain and its ordinary typed mark prototype capability.
///
/// # Errors
///
/// Returns stable family-reference, ordering, or output compatibility diagnostics.
fn resolve_random_site_pipeline(
    definition: &PatternDefinition,
) -> Result<PatternPipelinePlan, PatternPipelineError> {
    let PatternFamily::RandomSites {
        base_site_process_id,
        density_modulation_id,
        exclusion_id,
        site_product_id,
    } = definition.family
    else {
        unreachable!("random resolver is selected only for random-site families");
    };
    let [
        PatternMechanism::RandomSiteProcess {
            id: base_id,
            character,
            seed,
        },
        PatternMechanism::SiteDensityModulation {
            id: modulation_id,
            base_site_process_id: parent_base_id,
            modulation,
        },
        PatternMechanism::SiteExclusion {
            id: declared_exclusion_id,
            density_modulation_id: parent_modulation_id,
            policy,
        },
        PatternMechanism::RandomSiteProduct {
            id: declared_site_id,
            exclusion_id: parent_exclusion_id,
            maximum_attempts,
            maximum_neighbor_checks,
        },
    ] = definition.mechanisms.as_slice()
    else {
        return Err(PatternPipelineError::new(
            "pattern.family.capability",
            "typed random-site mechanisms cannot produce the declared structural product",
        ));
    };
    if *base_id != base_site_process_id
        || *modulation_id != density_modulation_id
        || *declared_exclusion_id != exclusion_id
        || *declared_site_id != site_product_id
        || *parent_base_id != base_site_process_id
        || *parent_modulation_id != density_modulation_id
        || *parent_exclusion_id != exclusion_id
    {
        return Err(PatternPipelineError::new(
            "pattern.family.capability",
            "random-site mechanism references do not match the family root",
        ));
    }
    let [
        PatternOutputLayer::MarkPrototype {
            id,
            site_mechanism_id,
            prototype,
            orientation: MarkOrientation::Fixed,
        },
    ] = definition.output_layers.as_slice()
    else {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.capability",
            "random-site products require one fixed mark-prototype output",
        ));
    };
    if *site_mechanism_id != site_product_id {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.capability",
            "random-site output must consume its declared site product",
        ));
    }
    let product = StructuralProductCapability::RandomSites;
    Ok(PatternPipelinePlan {
        family: FamilyCapability {
            product,
            provenance: StructuralProductProvenance {
                definition_id: definition.id.0,
                family_capability: product,
                mechanism_ids: vec![
                    base_site_process_id,
                    density_modulation_id,
                    exclusion_id,
                    site_product_id,
                ],
            },
            dimensions: Vec::new(),
            site_selection: Vec::new(),
            merge_epsilon: None,
            along_interval_multiplier: None,
            along_phase: None,
            random: Some(RandomSiteCapability {
                character: character.clone(),
                seed: *seed,
                density_modulation: modulation.clone(),
                exclusion: policy.clone(),
                maximum_attempts: *maximum_attempts,
                maximum_neighbor_checks: *maximum_neighbor_checks,
            }),
            generic_guides: None,
            parametric_curve: None,
        },
        modulation: definition.modulation.clone(),
        ordered_outputs: vec![OutputCapability {
            layer_id: *id,
            consumes: product,
            payload: OutputCapabilityPayload::Marks {
                prototype: prototype.clone(),
                orientation: MarkOrientation::Fixed,
            },
        }],
    })
}

/// Evaluate one typed family through its resolved capability plan. The current
/// no-variant modulation is deliberately resolved between family generation
/// and realization, preserving the authoritative stage order for future data
/// additions without a name-based dispatch path.
pub fn evaluate_typed_family(
    definition: &PatternDefinition,
    request: &GridInspectRequest,
) -> Result<TypedFamilyOutput, PatternPipelineError> {
    evaluate_typed_family_cancellable(definition, request, &|| false)
}

/// Cancellation-aware typed family planning. It checks before coverage,
/// allocation, and each bounded candidate row; final-consumer clipping is not
/// involved in the structural work policy.
pub fn evaluate_typed_family_cancellable(
    definition: &PatternDefinition,
    request: &GridInspectRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedFamilyOutput, PatternPipelineError> {
    let plan = resolve_pattern_pipeline(definition)?;
    evaluate_typed_family_product_cancellable(&plan.family, request, is_cancelled)
}

/// Resolves document-owned guide resources and evaluates their typed family without global state.
///
/// # Errors
///
/// Returns the resolver, coverage, geometry, limit, or cancellation error before publishing any
/// partial guide or site result.
pub fn evaluate_document_typed_family_cancellable(
    document: &Document,
    definition: &PatternDefinition,
    request: &GridInspectRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedFamilyOutput, PatternPipelineError> {
    let plan = resolve_document_pattern_pipeline(document, definition)?;
    evaluate_typed_family_product_cancellable(&plan.family, request, is_cancelled)
}

/// Evaluates only the structural family product. Output-layer and modulation
/// contracts intentionally stay out of this cacheable result and are supplied
/// later to realization.
pub fn evaluate_typed_family_product_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedFamilyOutput, PatternPipelineError> {
    evaluate_typed_family_product_with_source_cancellable(family, request, None, is_cancelled)
}

/// Evaluates a family with the decoded source only when its declared density
/// modulation requires it.  Source-independent mechanisms deliberately use
/// the same path and identity boundary as Stage 15/16A.
pub fn evaluate_typed_family_product_with_source_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    source: Option<&SourceField>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedFamilyOutput, PatternPipelineError> {
    if family.parametric_curve.is_some() {
        return evaluate_parametric_curve_cancellable(family, request, is_cancelled);
    }
    if family.generic_guides.is_some() {
        return evaluate_generic_curve_guides_cancellable(family, request, is_cancelled);
    }
    if family.product == StructuralProductCapability::RandomSites {
        let output = evaluate_random_sites_cancellable(family, request, source, is_cancelled)?;
        let fingerprint = family_site_fingerprint(&output.family_fingerprint, &output.sites);
        let site_set = FamilySiteSet::new(
            fingerprint,
            family.provenance.mechanism_ids[3],
            output.sites,
        )
        .map_err(family_site_error)?;
        return Ok(TypedFamilyOutput {
            family: family.clone(),
            sites: site_set,
            diagnostics: Some(output.diagnostics),
            structure: TypedFamilyStructure {
                coverage: Vec::new(),
                guides: Vec::new(),
                support_radius: request.support_radius,
                guard_steps: request.guard_steps,
                antialias_margin: ANTIALIAS_MARGIN,
                generation_domain: output.generation_domain,
                structural_path_set: None,
                guide_nominal_bases: BTreeMap::new(),
            },
        });
    }
    let legacy_dimensions = family.dimensions.as_slice()
        == [
            StraightGuideDimension {
                id: FIRST_DIMENSION_ID,
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: toniator_domain::StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: SECOND_DIMENSION_ID,
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                repetition: toniator_domain::StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ]
        && family.product == StructuralProductCapability::GuideIntersections;
    if !legacy_dimensions {
        let output = evaluate_generalized_straight_guides_cancellable(
            family,
            &StraightGuideInspectRequest {
                canvas: request.canvas.clone(),
                density: request.density.clone(),
                rotation_degrees: request.rotation_degrees,
                translation_x: request.translation_x,
                translation_y: request.translation_y,
                guard_steps: request.guard_steps,
                support_radius: request.support_radius,
                max_family_candidates: request.max_family_candidates,
            },
            is_cancelled,
        )
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
        let site_set = family_sites_from_generalized(
            &output,
            family,
            request,
            family.provenance.mechanism_ids[1],
        )?;
        let generation_domain = Bounds::from_points(
            output
                .guides
                .iter()
                .flat_map(|guide| [guide.start, guide.end]),
        )
        .expect("generalized finite guides produce bounds");
        let guide_paths = output
            .guides
            .iter()
            .map(|guide| StructuralPathInstance {
                id: StructuralPathInstanceId::guide_dimension(
                    GuideDimensionId(guide.id.dimension_id),
                    guide.id.index,
                    guide.id.component_ordinal,
                ),
                source_structure_id: None,
                path: CurvePath::line(guide.start, guide.end)
                    .expect("validated generalized guide endpoints build a path"),
            })
            .collect::<Vec<_>>();
        let guide_nominal_bases = output
            .guides
            .iter()
            .filter_map(|guide| {
                output
                    .coverage
                    .iter()
                    .find(|coverage| coverage.dimension_id == guide.id.dimension_id)
                    .map(|coverage| {
                        (
                            StructuralPathInstanceId::guide_dimension(
                                GuideDimensionId(guide.id.dimension_id),
                                guide.id.index,
                                guide.id.component_ordinal,
                            ),
                            coverage.spacing,
                        )
                    })
            })
            .collect::<BTreeMap<_, _>>();
        let structural_path_set = (!guide_paths.is_empty()).then(|| {
            StructuralPathSet::new(
                output.family_fingerprint.clone(),
                family.provenance.mechanism_ids[0],
                guide_paths,
            )
            .expect("validated generalized guide output builds a path set")
        });
        return Ok(TypedFamilyOutput {
            family: family.clone(),
            sites: site_set,
            diagnostics: None,
            structure: TypedFamilyStructure {
                coverage: output.coverage,
                guides: output.guides,
                support_radius: request.support_radius,
                guard_steps: request.guard_steps,
                antialias_margin: ANTIALIAS_MARGIN,
                generation_domain,
                structural_path_set,
                guide_nominal_bases,
            },
        });
    }
    let output = evaluate_straight_grid_cancellable(request, is_cancelled)
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
    let site_set = family_sites_from_grid(&output, family.provenance.mechanism_ids[1])?;
    Ok(TypedFamilyOutput {
        family: family.clone(),
        sites: site_set,
        diagnostics: None,
        structure: TypedFamilyStructure::from_grid(&output, family.provenance.mechanism_ids[0]),
    })
}

/// Evaluates a complete topology-supporting family envelope, then derives caller-supplied site adjacency.
///
/// This boundary dispatches only on the resolved structural-product capability.  It never uses a
/// mechanism name or provenance variant, never treats canvas bounds as graph input, and never
/// stores adjacency as document intent or a family-cache product.
///
/// # Errors
///
/// Returns stable raw-path, guard, family, source, geometry, limit, or cancellation diagnostics
/// before a partial family/graph pair can escape.
pub fn evaluate_typed_site_adjacency_with_source_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    source: Option<&SourceField>,
    policy: SiteAdjacencyPolicy,
    limits: SiteAdjacencyLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<SiteAdjacencyEvaluation, PatternPipelineError> {
    if family.product == StructuralProductCapability::ParametricPaths {
        return Err(PatternPipelineError::new(
            "adjacency.family.product",
            "raw parametric-path products do not publish sites for adjacency",
        ));
    }
    if !matches!(
        family.product,
        StructuralProductCapability::GuideIntersections
            | StructuralProductCapability::AlongGuideSites
            | StructuralProductCapability::RandomSites
    ) {
        return Err(PatternPipelineError::new(
            "adjacency.family.product",
            "resolved family product does not publish eligible adjacency sites",
        ));
    }
    if request.guard_steps == 0 {
        return Err(PatternPipelineError::new(
            "adjacency.coverage.guard_steps",
            "active adjacency requires at least one guard step",
        ));
    }
    policy.validate().map_err(site_adjacency_error)?;
    let topology_support = f64::from(request.guard_steps) * policy.maximum_distance();
    let support_radius = request.support_radius + topology_support;
    if !support_radius.is_finite() {
        return Err(PatternPipelineError::new(
            "adjacency.coverage.support",
            "topology support must remain finite",
        ));
    }
    let mut complete_request = request.clone();
    complete_request.support_radius = support_radius;
    let complete_family = evaluate_typed_family_product_with_source_cancellable(
        family,
        &complete_request,
        source,
        is_cancelled,
    )?;
    let graph = build_typed_site_adjacency_cancellable(
        &complete_family,
        request.support_radius,
        policy,
        limits,
        is_cancelled,
    )?;
    Ok(SiteAdjacencyEvaluation {
        family: complete_family,
        graph,
    })
}

/// Derives topology from one already evaluated typed site product without reinterpreting its mechanism.
///
/// # Errors
///
/// Returns a stable product, guard, geometry, resource, or cancellation diagnostic without
/// publishing a partial graph. Raw parametric paths are rejected before graph allocation.
pub fn build_typed_site_adjacency_cancellable(
    family: &TypedFamilyOutput,
    base_support_radius: f64,
    policy: SiteAdjacencyPolicy,
    limits: SiteAdjacencyLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<SiteAdjacencyGraph, PatternPipelineError> {
    if family.family.product == StructuralProductCapability::ParametricPaths {
        return Err(PatternPipelineError::new(
            "adjacency.family.product",
            "raw parametric-path products do not publish sites for adjacency",
        ));
    }
    if !matches!(
        family.family.product,
        StructuralProductCapability::GuideIntersections
            | StructuralProductCapability::AlongGuideSites
            | StructuralProductCapability::RandomSites
    ) {
        return Err(PatternPipelineError::new(
            "adjacency.family.product",
            "resolved family product does not publish eligible adjacency sites",
        ));
    }
    if family.guard_steps() == 0 {
        return Err(PatternPipelineError::new(
            "adjacency.coverage.guard_steps",
            "active adjacency requires at least one guard step",
        ));
    }
    policy.validate().map_err(site_adjacency_error)?;
    if !base_support_radius.is_finite() || base_support_radius < 0.0 {
        return Err(PatternPipelineError::new(
            "adjacency.coverage.support",
            "base topology support must be finite and nonnegative",
        ));
    }
    let required_support =
        base_support_radius + f64::from(family.guard_steps()) * policy.maximum_distance();
    if !required_support.is_finite() || family.planned_support_radius() < required_support {
        return Err(PatternPipelineError::new(
            "adjacency.coverage.support",
            "family envelope does not cover required topology support",
        ));
    }
    build_site_adjacency_cancellable(family.site_set(), policy, limits, is_cancelled)
        .map_err(site_adjacency_error)
}

/// Preserves geometry-owned topology diagnostics at the typed family boundary.
fn site_adjacency_error(error: SiteAdjacencyError) -> PatternPipelineError {
    PatternPipelineError::new(error.path(), error.message())
}

/// Converts one validated analytic parametric source then reuses the accepted finite-path evaluator.
///
/// # Errors
///
/// Returns construction, cancellation, coverage, offset, or site-product errors without partial output.
fn evaluate_parametric_curve_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedFamilyOutput, PatternPipelineError> {
    let parametric = family
        .parametric_curve
        .as_ref()
        .expect("parametric branch is present");
    let local_path = toniator_geometry::construct_parametric_curve_path_cancellable(
        &parametric.curve,
        Point2::new(0.0, 0.0),
        is_cancelled,
    )?;
    let origin_at_canvas_center = AffineTransform2D::rotate_about_then_translate(
        Point2::new(0.0, 0.0),
        0.0,
        Vector2::new(request.canvas.width / 2.0, request.canvas.height / 2.0),
    )
    .ok_or(PatternPipelineError::new(
        "pattern.parametric.origin",
        "canvas center placement must remain finite",
    ))?;
    let path = local_path.transformed(origin_at_canvas_center)?;
    let dimension_id = GuideDimensionId(family.provenance.mechanism_ids[0].0);
    let dimension = GuideDimension {
        id: dimension_id,
        baseline_angle_degrees: 0.0,
        phase: 0.0,
        prototype: GuidePrototype::CircularArc {
            center: toniator_domain::AuthoredPoint2 { x: 0.0, y: 0.0 },
            radius: 1.0,
            start_angle_degrees: 0.0,
            sweep_angle_degrees: 90.0,
        },
        repetition: parametric.repetition.clone(),
    };
    let mut reusable = family.clone();
    reusable.generic_guides = Some(GenericGuideCapability {
        dimensions: vec![dimension],
        resolved_paths: vec![(None, path)],
        structural_source: Some(parametric.source_id),
        absolute_site_interval: parametric.site_interval,
        single_nominal_spacing: match &parametric.curve {
            ParametricCurve::Spiral(spiral) => Some(spiral.radial_spacing),
        },
    });
    reusable.parametric_curve = None;
    reusable.site_selection = if reusable.product == StructuralProductCapability::AlongGuideSites {
        vec![dimension_id]
    } else {
        Vec::new()
    };
    let mut output = evaluate_generic_curve_guides_cancellable(&reusable, request, is_cancelled)?;
    output.family = family.clone();
    Ok(output)
}

/// Borrows already accepted offsets between the source and one same-side candidate as barriers.
fn nearer_same_side_offset_barriers(
    paths_by_index: &BTreeMap<i64, Vec<toniator_geometry::OffsetPathComponent>>,
    candidate_index: i64,
) -> Vec<&CurvePath> {
    paths_by_index
        .iter()
        .filter(|(index, _)| {
            if candidate_index > 0 {
                **index >= 0 && **index < candidate_index
            } else if candidate_index < 0 {
                **index <= 0 && **index > candidate_index
            } else {
                false
            }
        })
        .flat_map(|(_, components)| components.iter().map(|component| &component.path))
        .collect()
}

/// Proves that surviving outer offset components span authored endpoints and the requested normal side.
///
/// # Errors
///
/// Returns canonical path, normal, bounds, or finite-location diagnostics without accepting partial coverage.
fn normal_offset_components_bracket_domain(
    paths: &[toniator_geometry::OffsetPathComponent],
    source: &CurvePath,
    domain: Bounds,
    signed_side: f64,
) -> Result<bool, PatternPipelineError> {
    if paths.is_empty() {
        return Ok(false);
    }
    let authored_start = PathLocation::new(0, 0.0)?;
    let authored_end = PathLocation::new(source.segments().len() - 1, 1.0)?;
    let tolerance = PathOffsetLimits::default().tolerance;
    if paths.first().map(|component| component.source_start) != Some(authored_start)
        || paths.last().map(|component| component.source_end) != Some(authored_end)
        || paths.iter().any(|component| {
            source_location_order(component.source_start, component.source_end)
                != std::cmp::Ordering::Less
                && component.source_start != component.source_end
        })
        || paths.windows(2).any(|pair| {
            pair[0].component_ordinal >= pair[1].component_ordinal
                || source_location_order(pair[0].source_start, pair[1].source_start)
                    != std::cmp::Ordering::Less
                || source_location_order(pair[0].source_end, pair[1].source_start)
                    == std::cmp::Ordering::Greater
        })
    {
        return Ok(false);
    }
    for segment_index in 0..source.segments().len() {
        for parameter in [0.0, 0.5, 1.0] {
            let normal = source.unit_normal_at(PathLocation::new(segment_index, parameter)?)?;
            let (domain_minimum, domain_maximum) = domain
                .corners()
                .into_iter()
                .map(|point| point.dot(normal))
                .fold(
                    (f64::INFINITY, f64::NEG_INFINITY),
                    |(minimum, maximum), projection| {
                        (minimum.min(projection), maximum.max(projection))
                    },
                );
            let mut path_minimum = f64::INFINITY;
            let mut path_maximum = f64::NEG_INFINITY;
            for component in paths {
                for point in component.path.bounds()?.corners() {
                    let projection = point.dot(normal);
                    path_minimum = path_minimum.min(projection);
                    path_maximum = path_maximum.max(projection);
                }
            }
            let bracketed = if signed_side > 0.0 {
                path_maximum + tolerance >= domain_maximum
            } else {
                path_minimum - tolerance <= domain_minimum
            };
            if !bracketed {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// Orders exact authored path locations without projecting them into a lossy global parameter.
fn source_location_order(left: PathLocation, right: PathLocation) -> std::cmp::Ordering {
    left.segment_index()
        .cmp(&right.segment_index())
        .then_with(|| left.parameter().total_cmp(&right.parameter()))
}

/// Evaluates resolved Stage 20D finite guide paths before any current-circle compatibility realization.
fn evaluate_generic_curve_guides_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedFamilyOutput, PatternPipelineError> {
    #[derive(Clone, Copy)]
    struct NormalOffsetCoveragePlan {
        first_required: i64,
        last_required: i64,
        sides: OffsetSides,
    }

    let generic = family
        .generic_guides
        .as_ref()
        .expect("generic branch has resolved guide capability");
    if is_cancelled() {
        return Err(PatternPipelineError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ));
    }
    let canvas = Bounds::new(
        Point2::new(0.0, 0.0),
        Point2::new(request.canvas.width, request.canvas.height),
    )
    .ok_or(PatternPipelineError::new(
        "coverage.curved_guides.proof",
        "curved-guide coverage could not prove a complete generation envelope",
    ))?;
    let maximum_directional_spacing = (request.canvas.width / request.density.across_x)
        .max(request.canvas.height / request.density.across_y);
    if !maximum_directional_spacing.is_finite() || maximum_directional_spacing <= 0.0 {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.proof",
            "curved-guide coverage could not prove a complete generation envelope",
        ));
    }
    let along_bound = family
        .along_interval_multiplier
        .map(|value| value * maximum_directional_spacing)
        .unwrap_or(0.0);
    let mut maximum_spacing = along_bound;
    for dimension in &generic.dimensions {
        if let GuideRepetition::TransformStack {
            direction_degrees,
            spacing_multiplier,
        } = dimension.repetition
        {
            let angle = (dimension.baseline_angle_degrees + direction_degrees).to_radians();
            let unit = Vector2::new(angle.cos(), angle.sin());
            let spacing =
                directional_spacing(&request.canvas, &request.density, unit)? * spacing_multiplier;
            maximum_spacing = maximum_spacing.max(spacing);
        }
        if let GuideRepetition::NormalOffset { spacing, .. } = dimension.repetition {
            maximum_spacing = maximum_spacing.max(spacing);
        }
    }
    let margin =
        request.support_radius + ANTIALIAS_MARGIN + request.guard_steps as f64 * maximum_spacing;
    let document_domain = canvas.expanded(margin).ok_or(PatternPipelineError::new(
        "coverage.curved_guides.proof",
        "curved-guide coverage could not prove a complete generation envelope",
    ))?;
    let channel_transform = AffineTransform2D::rotate_about_then_translate(
        Point2::new(request.canvas.width / 2.0, request.canvas.height / 2.0),
        request.rotation_degrees,
        Vector2::new(request.translation_x, request.translation_y),
    )
    .ok_or(PatternPipelineError::new(
        "coverage.curved_guides.proof",
        "curved-guide coverage could not prove a complete generation envelope",
    ))?;
    let local_domain =
        channel_transform
            .inverse_bounds(document_domain)
            .ok_or(PatternPipelineError::new(
                "coverage.curved_guides.proof",
                "curved-guide coverage could not prove a complete generation envelope",
            ))?;
    let mut guides = Vec::new();
    let mut guide_nominal_bases = BTreeMap::new();
    let mut grouped = Vec::<Vec<StructuralPathInstance>>::new();
    for (dimension, (source_structure_id, prototype)) in
        generic.dimensions.iter().zip(&generic.resolved_paths)
    {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        let baseline = AffineTransform2D::rotate_about_then_translate(
            Point2::new(0.0, 0.0),
            dimension.baseline_angle_degrees,
            Vector2::new(0.0, 0.0),
        )
        .ok_or(PatternPipelineError::new(
            "coverage.curved_guides.proof",
            "curved-guide coverage could not prove a complete generation envelope",
        ))?;
        let base = prototype.transformed(baseline)?;
        let (unit, spacing, indices, normal_offset_coverage) = match dimension.repetition {
            GuideRepetition::Single => {
                let angle = dimension.baseline_angle_degrees.to_radians();
                (Vector2::new(angle.cos(), angle.sin()), 0.0, vec![0], None)
            }
            GuideRepetition::TransformStack {
                direction_degrees,
                spacing_multiplier,
            } => {
                let angle = (dimension.baseline_angle_degrees + direction_degrees).to_radians();
                let unit = Vector2::new(angle.cos(), angle.sin());
                let spacing = directional_spacing(&request.canvas, &request.density, unit)?
                    * spacing_multiplier;
                let bounds = base.bounds()?;
                let projections = local_domain
                    .corners()
                    .into_iter()
                    .map(|point| point.dot(unit));
                let (min_domain, max_domain) = projections
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
                        (min.min(value), max.max(value))
                    });
                let path_projections = bounds.corners().into_iter().map(|point| point.dot(unit));
                let (min_path, max_path) = path_projections
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
                        (min.min(value), max.max(value))
                    });
                // Index zero remains the raw authored phase; normalizing it here would
                // preserve geometry but incorrectly renumber derived guide identities.
                let first_raw = ((min_domain - max_path - dimension.phase) / spacing).floor();
                let last_raw = ((max_domain - min_path - dimension.phase) / spacing).ceil();
                if !first_raw.is_finite()
                    || !last_raw.is_finite()
                    || first_raw < i64::MIN as f64
                    || last_raw > i64::MAX as f64
                {
                    return Err(PatternPipelineError::new(
                        "coverage.curved_guides.numeric_overflow",
                        "curved-guide coverage arithmetic overflowed",
                    ));
                }
                let guard = i64::from(request.guard_steps);
                let first =
                    (first_raw as i64)
                        .checked_sub(guard)
                        .ok_or(PatternPipelineError::new(
                            "coverage.curved_guides.numeric_overflow",
                            "curved-guide coverage arithmetic overflowed",
                        ))?;
                let last =
                    (last_raw as i64)
                        .checked_add(guard)
                        .ok_or(PatternPipelineError::new(
                            "coverage.curved_guides.numeric_overflow",
                            "curved-guide coverage arithmetic overflowed",
                        ))?;
                let count = last
                    .checked_sub(first)
                    .and_then(|value| value.checked_add(1));
                if count.is_none() || count.unwrap() as u64 > request.max_family_candidates as u64 {
                    return Err(PatternPipelineError::new(
                        "coverage.curved_guides.instance_limit",
                        "curved-guide instance count exceeds the configured family limit",
                    ));
                }
                (unit, spacing, (first..=last).collect(), None)
            }
            GuideRepetition::NormalOffset { spacing, sides, .. } => {
                let path_bounds = base.bounds()?;
                if path_bounds.min == path_bounds.max {
                    return Err(PatternPipelineError::new(
                        "coverage.curved_guides.normal_offset",
                        "normal-offset source guide collapsed before coverage could be proved",
                    ));
                }
                let mut first_raw = f64::INFINITY;
                let mut last_raw = f64::NEG_INFINITY;
                for segment_index in 0..base.segments().len() {
                    for parameter in [0.0, 0.5, 1.0] {
                        let normal =
                            base.unit_normal_at(PathLocation::new(segment_index, parameter)?)?;
                        let domain_projections = local_domain
                            .corners()
                            .into_iter()
                            .map(|point| point.dot(normal));
                        let (domain_minimum, domain_maximum) = domain_projections.fold(
                            (f64::INFINITY, f64::NEG_INFINITY),
                            |(minimum, maximum), projection| {
                                (minimum.min(projection), maximum.max(projection))
                            },
                        );
                        let path_projections = path_bounds
                            .corners()
                            .into_iter()
                            .map(|point| point.dot(normal));
                        let (path_minimum, path_maximum) = path_projections.fold(
                            (f64::INFINITY, f64::NEG_INFINITY),
                            |(minimum, maximum), projection| {
                                (minimum.min(projection), maximum.max(projection))
                            },
                        );
                        first_raw =
                            first_raw.min(((domain_minimum - path_maximum) / spacing).floor());
                        last_raw = last_raw.max(((domain_maximum - path_minimum) / spacing).ceil());
                    }
                }
                if !first_raw.is_finite()
                    || !last_raw.is_finite()
                    || first_raw < i64::MIN as f64
                    || last_raw > i64::MAX as f64
                {
                    return Err(PatternPipelineError::new(
                        "coverage.curved_guides.numeric_overflow",
                        "normal-offset coverage arithmetic overflowed",
                    ));
                }
                let guard = i64::from(request.guard_steps);
                let first =
                    (first_raw as i64)
                        .checked_sub(guard)
                        .ok_or(PatternPipelineError::new(
                            "coverage.curved_guides.numeric_overflow",
                            "normal-offset coverage arithmetic overflowed",
                        ))?;
                let last =
                    (last_raw as i64)
                        .checked_add(guard)
                        .ok_or(PatternPipelineError::new(
                            "coverage.curved_guides.numeric_overflow",
                            "normal-offset coverage arithmetic overflowed",
                        ))?;
                let (first, last) = match sides {
                    OffsetSides::Left => (0, last.max(0)),
                    OffsetSides::Right => (first.min(0), 0),
                    OffsetSides::Both => (first.min(0), last.max(0)),
                };
                let count = last
                    .checked_sub(first)
                    .and_then(|value| value.checked_add(1));
                if count.is_none() || count.unwrap() as u64 > request.max_family_candidates as u64 {
                    return Err(PatternPipelineError::new(
                        "coverage.curved_guides.instance_limit",
                        "normal-offset instance count exceeds the configured family limit",
                    ));
                }
                (
                    Vector2::new(0.0, 0.0),
                    spacing,
                    (first..=last).collect(),
                    Some(NormalOffsetCoveragePlan {
                        first_required: first,
                        last_required: last,
                        sides,
                    }),
                )
            }
        };
        let evaluate_index = |index: i64,
                              crossing_barriers: &[&CurvePath]|
         -> Result<
            Vec<toniator_geometry::OffsetPathComponent>,
            PatternPipelineError,
        > {
            if is_cancelled() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "evaluation was cancelled",
                ));
            }
            match dimension.repetition {
                GuideRepetition::NormalOffset { cleanup, .. } => {
                    let cleanup = match cleanup {
                        OffsetCleanup::DissolveCrossings => PathOffsetCleanup::DissolveCrossings,
                    };
                    match offset_path_cancellable(
                        PathOffsetRequest {
                            path: &base,
                            signed_distance: index as f64 * spacing,
                            endpoint_policy: PathOffsetEndpointPolicy::TangentialExtension {
                                bounds: local_domain,
                            },
                            cleanup,
                            crossing_barriers,
                            limits: PathOffsetLimits::default(),
                        },
                        is_cancelled,
                    )? {
                        PathOffsetResult::Paths(paths) => Ok(paths),
                        PathOffsetResult::Collapsed => Ok(Vec::new()),
                    }
                }
                _ => {
                    let offset = dimension.phase + index as f64 * spacing;
                    let local = AffineTransform2D::rotate_about_then_translate(
                        Point2::new(0.0, 0.0),
                        0.0,
                        unit.scale(offset),
                    )
                    .ok_or(PatternPipelineError::new(
                        "coverage.curved_guides.proof",
                        "curved-guide coverage could not prove a complete generation envelope",
                    ))?;
                    Ok(vec![toniator_geometry::OffsetPathComponent {
                        component_ordinal: 0,
                        source_start: PathLocation::new(0, 0.0)?,
                        source_end: PathLocation::new(base.segments().len() - 1, 1.0)?,
                        path: base.transformed(local)?,
                    }])
                }
            }
        };
        let mut attempts = 0_usize;
        let mut paths_by_index = BTreeMap::new();
        let mut evaluation_indices = indices;
        if normal_offset_coverage.is_some() {
            evaluation_indices.sort_by(|left, right| {
                left.unsigned_abs()
                    .cmp(&right.unsigned_abs())
                    .then_with(|| left.cmp(right))
            });
        }
        for index in evaluation_indices {
            attempts += 1;
            let barriers = if normal_offset_coverage.is_some() {
                nearer_same_side_offset_barriers(&paths_by_index, index)
            } else {
                Vec::new()
            };
            let paths = evaluate_index(index, &barriers)?;
            paths_by_index.insert(index, paths);
        }
        if let Some(coverage) = normal_offset_coverage {
            if paths_by_index.get(&0).is_none_or(Vec::is_empty) {
                return Err(PatternPipelineError::new(
                    "coverage.curved_guides.normal_offset",
                    "normal-offset source guide collapsed before coverage could be proved",
                ));
            }
            if matches!(coverage.sides, OffsetSides::Left | OffsetSides::Both)
                && coverage.last_required > 0
            {
                let mut probe = coverage.last_required;
                while paths_by_index
                    .get(&probe)
                    .is_some_and(|paths| !paths.is_empty())
                    && !normal_offset_components_bracket_domain(
                        paths_by_index.get(&probe).map_or(&[], Vec::as_slice),
                        &base,
                        local_domain,
                        1.0,
                    )?
                {
                    if attempts >= request.max_family_candidates {
                        return Err(PatternPipelineError::new(
                            "coverage.curved_guides.normal_offset",
                            "normal-offset left coverage could not find a surviving outer component within the configured family limit",
                        ));
                    }
                    probe = probe.checked_add(1).ok_or(PatternPipelineError::new(
                        "coverage.curved_guides.numeric_overflow",
                        "normal-offset coverage arithmetic overflowed",
                    ))?;
                    attempts += 1;
                    let barriers = nearer_same_side_offset_barriers(&paths_by_index, probe);
                    let paths = evaluate_index(probe, &barriers)?;
                    paths_by_index.insert(probe, paths);
                }
            }
            if matches!(coverage.sides, OffsetSides::Right | OffsetSides::Both)
                && coverage.first_required < 0
            {
                let mut probe = coverage.first_required;
                while paths_by_index
                    .get(&probe)
                    .is_some_and(|paths| !paths.is_empty())
                    && !normal_offset_components_bracket_domain(
                        paths_by_index.get(&probe).map_or(&[], Vec::as_slice),
                        &base,
                        local_domain,
                        -1.0,
                    )?
                {
                    if attempts >= request.max_family_candidates {
                        return Err(PatternPipelineError::new(
                            "coverage.curved_guides.normal_offset",
                            "normal-offset right coverage could not find a surviving outer component within the configured family limit",
                        ));
                    }
                    probe = probe.checked_sub(1).ok_or(PatternPipelineError::new(
                        "coverage.curved_guides.numeric_overflow",
                        "normal-offset coverage arithmetic overflowed",
                    ))?;
                    attempts += 1;
                    let barriers = nearer_same_side_offset_barriers(&paths_by_index, probe);
                    let paths = evaluate_index(probe, &barriers)?;
                    paths_by_index.insert(probe, paths);
                }
            }
        }
        let mut this_dimension = Vec::new();
        for (index, paths) in paths_by_index {
            let basis = if spacing > 0.0 {
                spacing
            } else {
                generic.single_nominal_spacing.unwrap_or(
                    (request.canvas.width / request.density.across_x)
                        .max(request.canvas.height / request.density.across_y),
                )
            };
            for component in paths {
                let instance = StructuralPathInstance {
                    id: match generic.structural_source {
                        Some(source) => StructuralPathInstanceId {
                            source,
                            repetition_index: index,
                            component_ordinal: component.component_ordinal,
                        },
                        None => StructuralPathInstanceId::guide_dimension(
                            dimension.id,
                            index,
                            component.component_ordinal,
                        ),
                    },
                    source_structure_id: *source_structure_id,
                    path: component.path.transformed(channel_transform)?,
                };
                guide_nominal_bases.insert(instance.id, basis);
                this_dimension.push(instance.clone());
                guides.push(instance);
                if guides.len() > request.max_family_candidates {
                    return Err(PatternPipelineError::new(
                        "coverage.curved_guides.instance_limit",
                        "curved-guide instance count exceeds the configured family limit",
                    ));
                }
            }
        }
        grouped.push(this_dimension);
    }
    let fingerprint = generic_curve_fingerprint(family, request, generic);
    let path_set = StructuralPathSet::new(
        fingerprint.clone(),
        family.provenance.mechanism_ids[0],
        guides,
    )
    .map_err(|_| {
        PatternPipelineError::new(
            "coverage.curved_guides.proof",
            "curved-guide coverage could not prove a complete generation envelope",
        )
    })?;
    let selected = family
        .site_selection
        .iter()
        .map(|id| {
            generic
                .dimensions
                .iter()
                .position(|dimension| dimension.id == *id)
                .ok_or(PatternPipelineError::new(
                    "pattern.family.selection",
                    "selection references a missing dimension ID",
                ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sites = match family.product {
        StructuralProductCapability::GuideIntersections => {
            preflight_curve_intersection_work(&grouped, &selected, request.max_family_candidates)?;
            curve_intersection_sites(
                &grouped,
                &selected,
                family.merge_epsilon.unwrap_or(0.0),
                canvas,
                document_domain,
                &request.canvas,
                &request.density,
                family.provenance.mechanism_ids[1],
                request.max_family_candidates,
                is_cancelled,
            )?
        }
        StructuralProductCapability::AlongGuideSites => {
            let multiplier = family.along_interval_multiplier.unwrap_or(1.0);
            preflight_curve_along_work(
                &grouped,
                &selected,
                multiplier,
                generic.absolute_site_interval,
                &request.canvas,
                &request.density,
                request.max_family_candidates,
                is_cancelled,
            )?;
            curve_along_sites(
                &grouped,
                &selected,
                multiplier,
                family.along_phase.unwrap_or(0.0),
                generic.absolute_site_interval,
                &guide_nominal_bases,
                &request.canvas,
                &request.density,
                canvas,
                document_domain,
                family.provenance.mechanism_ids[1],
                request.max_family_candidates,
                is_cancelled,
            )?
        }
        StructuralProductCapability::ParametricPaths => Vec::new(),
        StructuralProductCapability::RandomSites => unreachable!(),
    };
    let site_set = FamilySiteSet::new(
        family_site_fingerprint(&fingerprint, &sites),
        *family
            .provenance
            .mechanism_ids
            .last()
            .expect("parametric provenance has a source"),
        sites,
    )
    .map_err(family_site_error)?;
    Ok(TypedFamilyOutput {
        family: family.clone(),
        sites: site_set,
        diagnostics: None,
        structure: TypedFamilyStructure {
            coverage: Vec::new(),
            guides: Vec::new(),
            support_radius: request.support_radius,
            guard_steps: request.guard_steps,
            antialias_margin: ANTIALIAS_MARGIN,
            generation_domain: document_domain,
            structural_path_set: Some(path_set),
            guide_nominal_bases,
        },
    })
}

/// Bounds variable-tangent along-guide sampling before site output can be allocated.
///
/// # Errors
///
/// Returns the Stage 20D cancellation, numeric, or along-guide-limit diagnostic when a
/// selected finite guide cannot be measured within the declared family work limit.
#[allow(clippy::too_many_arguments)] // Keeps the evaluator's explicit authority inputs visible.
fn preflight_curve_along_work(
    grouped: &[Vec<StructuralPathInstance>],
    selected: &[usize],
    multiplier: f64,
    absolute_interval: Option<f64>,
    canvas: &CanvasSpec,
    density: &DensityMetric2D,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), PatternPipelineError> {
    let minimum_interval = absolute_interval.unwrap_or(
        (canvas.width / density.across_x).min(canvas.height / density.across_y) * multiplier,
    );
    if !minimum_interval.is_finite() || minimum_interval <= 0.0 {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.proof",
            "curved-guide coverage could not prove a complete generation envelope",
        ));
    }
    let mut predicted = 0_usize;
    for &dimension in selected {
        for guide in &grouped[dimension] {
            if cancelled() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "evaluation was cancelled",
                ));
            }
            let count = (guide.path.measure_arc_length()?.total_length() / minimum_interval)
                .ceil()
                .max(0.0) as usize;
            predicted =
                predicted
                    .checked_add(count.saturating_add(1))
                    .ok_or(PatternPipelineError::new(
                        "coverage.curved_guides.numeric_overflow",
                        "curved-guide coverage arithmetic overflowed",
                    ))?;
            if predicted > limit {
                return Err(PatternPipelineError::new(
                    "coverage.curved_guides.along_guide_limit",
                    "curved along-guide site count exceeds the configured family limit",
                ));
            }
        }
    }
    Ok(())
}

/// Bounds selected guide pairs, segment products, and merge candidates before curve allocation.
fn preflight_curve_intersection_work(
    grouped: &[Vec<StructuralPathInstance>],
    selected: &[usize],
    limit: usize,
) -> Result<(), PatternPipelineError> {
    let mut guide_pairs = 0_usize;
    let mut segment_pairs = 0_usize;
    for (offset, &left) in selected.iter().enumerate() {
        for &right in &selected[offset + 1..] {
            guide_pairs = guide_pairs
                .checked_add(
                    grouped[left]
                        .len()
                        .checked_mul(grouped[right].len())
                        .ok_or(PatternPipelineError::new(
                            "coverage.curved_guides.numeric_overflow",
                            "curved-guide coverage arithmetic overflowed",
                        ))?,
                )
                .ok_or(PatternPipelineError::new(
                    "coverage.curved_guides.numeric_overflow",
                    "curved-guide coverage arithmetic overflowed",
                ))?;
            for first in &grouped[left] {
                for second in &grouped[right] {
                    segment_pairs = segment_pairs
                        .checked_add(
                            first
                                .path
                                .segments()
                                .len()
                                .checked_mul(second.path.segments().len())
                                .ok_or(PatternPipelineError::new(
                                    "coverage.curved_guides.numeric_overflow",
                                    "curved-guide coverage arithmetic overflowed",
                                ))?,
                        )
                        .ok_or(PatternPipelineError::new(
                            "coverage.curved_guides.numeric_overflow",
                            "curved-guide coverage arithmetic overflowed",
                        ))?;
                }
            }
        }
    }
    if guide_pairs > limit || segment_pairs > limit {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.pairwise_limit",
            "curved-guide pairwise work exceeds the configured family limit",
        ));
    }
    // Two cubic Béziers have at most nine isolated contacts.  The geometry layer also
    // defends its own larger diagnostic cap, but this strict mathematical bound lets the
    // family reject impossible merge work before invoking any intersection routine.
    let merge_work = segment_pairs
        .checked_mul(9)
        .ok_or(PatternPipelineError::new(
            "coverage.curved_guides.numeric_overflow",
            "curved-guide coverage arithmetic overflowed",
        ))?;
    if merge_work > limit {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.merge_limit",
            "curved-guide merge work exceeds the configured family limit",
        ));
    }
    Ok(())
}

/// Builds bounded curve-intersection sites in selected-dimension and guide-instance order.
#[allow(clippy::too_many_arguments)] // Explicit evaluator inputs preserve the bounded headless authority.
fn curve_intersection_sites(
    grouped: &[Vec<StructuralPathInstance>],
    selected: &[usize],
    epsilon: f64,
    canvas: Bounds,
    generation_domain: Bounds,
    canvas_spec: &CanvasSpec,
    density: &DensityMetric2D,
    product_id: PatternMechanismId,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<FamilySite>, PatternPipelineError> {
    let mut raw = Vec::<(
        Point2,
        Vec<StructuralPathLocationProvenance>,
        NominalCellBasis,
    )>::new();
    for (offset, &left) in selected.iter().enumerate() {
        for &right in &selected[offset + 1..] {
            for first in &grouped[left] {
                for second in &grouped[right] {
                    if cancelled() {
                        return Err(PatternPipelineError::new(
                            "evaluation.cancelled",
                            "evaluation was cancelled",
                        ));
                    }
                    let contacts = first.path.intersections(&second.path)?;
                    if cancelled() {
                        return Err(PatternPipelineError::new(
                            "evaluation.cancelled",
                            "evaluation was cancelled",
                        ));
                    }
                    for contact in contacts {
                        let point = contact.point();
                        if site_scope(point, canvas, generation_domain).is_none() {
                            continue;
                        }
                        let first_tangent = first
                            .path
                            .unit_tangent_at(contact.first_location())
                            .map_err(|_| {
                                PatternPipelineError::new(
                                    "pattern.family.curved_guides.tangent",
                                    "curved intersections require nonstationary tangents",
                                )
                            })?;
                        let second_tangent = second
                            .path
                            .unit_tangent_at(contact.second_location())
                            .map_err(|_| {
                                PatternPipelineError::new(
                                    "pattern.family.curved_guides.tangent",
                                    "curved intersections require nonstationary tangents",
                                )
                            })?;
                        let first_spacing = directional_spacing(
                            canvas_spec,
                            density,
                            first_tangent.perpendicular(),
                        )?;
                        let second_spacing = directional_spacing(
                            canvas_spec,
                            density,
                            second_tangent.perpendicular(),
                        )?;
                        let basis = NominalCellBasis::new(
                            first_tangent.scale(second_spacing),
                            second_tangent.scale(first_spacing),
                        )
                        .map_err(family_site_error)?;
                        raw.push((
                            point,
                            vec![
                                StructuralPathLocationProvenance {
                                    path: first.id,
                                    segment_index: contact.first_location().segment_index(),
                                    parameter_bits: contact.first_location().parameter().to_bits(),
                                },
                                StructuralPathLocationProvenance {
                                    path: second.id,
                                    segment_index: contact.second_location().segment_index(),
                                    parameter_bits: contact.second_location().parameter().to_bits(),
                                },
                            ],
                            basis,
                        ));
                        if raw.len() > limit {
                            return Err(PatternPipelineError::new(
                                "coverage.curved_guides.merge_limit",
                                "curved-guide merge work exceeds the configured family limit",
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut output: Vec<FamilySite> = Vec::new();
    for (point, mut contributors, basis) in raw {
        if let Some(existing) = output
            .iter_mut()
            .find(|site| ((site.position.x - point.x).hypot(site.position.y - point.y)) <= epsilon)
        {
            if let FamilySiteProvenance::CurveGuideIntersection {
                contributors: prior,
            } = &mut existing.provenance
            {
                for contributor in contributors {
                    if !prior.contains(&contributor) {
                        prior.push(contributor);
                    }
                }
            }
            if basis
                .diameter()
                .total_cmp(&existing.nominal_cell_basis.diameter())
                .is_lt()
            {
                existing.nominal_cell_basis = basis;
            }
            continue;
        }
        contributors.dedup();
        output.push(FamilySite {
            id: FamilySiteId {
                mechanism_id: product_id,
                ordinal: output.len(),
            },
            position: point,
            nominal_cell_basis: basis,
            // Contacts outside the document coverage envelope were excluded before
            // raw accumulation, so they cannot consume merge work or become Guard sites.
            scope: site_scope(point, canvas, generation_domain).expect("filtered envelope"),
            provenance: FamilySiteProvenance::CurveGuideIntersection { contributors },
        });
    }
    Ok(output)
}

/// Samples selected finite curve paths by bounded arc length and exact tangent-derived directional spacing.
#[allow(clippy::too_many_arguments)] // Explicit evaluator inputs preserve the bounded headless authority.
fn curve_along_sites(
    grouped: &[Vec<StructuralPathInstance>],
    selected: &[usize],
    multiplier: f64,
    phase: f64,
    absolute_interval: Option<f64>,
    nominal_bases: &BTreeMap<StructuralPathInstanceId, f64>,
    canvas_spec: &CanvasSpec,
    density: &DensityMetric2D,
    canvas: Bounds,
    generation_domain: Bounds,
    product_id: PatternMechanismId,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<FamilySite>, PatternPipelineError> {
    let mut output = Vec::new();
    let mut guide_order = 0;
    for &dimension in selected {
        let mut active_repetition: Option<(StructuralPathSourceId, i64)> = None;
        let mut component_origin = 0.0_f64;
        let mut next_position = 0.0_f64;
        let mut sequence = 0_i64;
        let mut previous_end: Option<Point2> = None;
        for guide in &grouped[dimension] {
            if cancelled() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "evaluation was cancelled",
                ));
            }
            let measure = guide.path.measure_arc_length()?;
            let total = measure.total_length();
            let start = measure.location_at_length(0.0)?;
            let start_tangent = guide.path.unit_tangent_at(start).map_err(|_| {
                PatternPipelineError::new(
                    "pattern.family.curved_guides.tangent",
                    "curved along-guide sampling requires a nonstationary tangent",
                )
            })?;
            let start_interval = absolute_interval.unwrap_or(
                directional_spacing(canvas_spec, density, start_tangent.perpendicular())?
                    * multiplier,
            );
            if !start_interval.is_finite() || start_interval <= 0.0 {
                return Err(PatternPipelineError::new(
                    "coverage.curved_guides.proof",
                    "curved-guide coverage could not prove a complete generation envelope",
                ));
            }
            let repetition = (guide.id.source, guide.id.repetition_index);
            if active_repetition != Some(repetition) {
                active_repetition = Some(repetition);
                component_origin = 0.0;
                next_position = phase.rem_euclid(1.0) * start_interval;
                sequence = 0;
                previous_end = None;
            }
            let shared_join = previous_end.is_some_and(|previous| {
                (previous.x - guide.path.start().x).hypot(previous.y - guide.path.start().y)
                    <= 1.0e-9
            });
            if shared_join && (next_position - component_origin).abs() <= 1.0e-9 {
                next_position += start_interval;
                sequence += 1;
            }
            while next_position - component_origin <= total {
                if cancelled() {
                    return Err(PatternPipelineError::new(
                        "evaluation.cancelled",
                        "evaluation was cancelled",
                    ));
                }
                let local_position = (next_position - component_origin).max(0.0);
                let location = measure.location_at_length(local_position)?;
                let tangent = guide.path.unit_tangent_at(location).map_err(|_| {
                    PatternPipelineError::new(
                        "pattern.family.curved_guides.tangent",
                        "curved along-guide sampling requires a nonstationary tangent",
                    )
                })?;
                let normal = tangent.perpendicular();
                let spacing = absolute_interval
                    .unwrap_or(directional_spacing(canvas_spec, density, normal)? * multiplier);
                if !spacing.is_finite() || spacing <= 0.0 {
                    return Err(PatternPipelineError::new(
                        "coverage.curved_guides.proof",
                        "curved-guide coverage could not prove a complete generation envelope",
                    ));
                }
                let point = guide.path.point_at(location)?;
                let scope = match site_scope(point, canvas, generation_domain) {
                    Some(scope) => scope,
                    None => {
                        next_position += spacing;
                        sequence += 1;
                        continue;
                    }
                };
                output.push(FamilySite {
                    id: FamilySiteId {
                        mechanism_id: product_id,
                        ordinal: output.len(),
                    },
                    position: point,
                    nominal_cell_basis: NominalCellBasis::new(
                        tangent.scale(spacing),
                        normal.scale(
                            nominal_bases
                                .get(&guide.id)
                                .copied()
                                .unwrap_or(directional_spacing(canvas_spec, density, normal)?),
                        ),
                    )
                    .map_err(family_site_error)?,
                    scope,
                    provenance: match guide.id.source {
                        StructuralPathSourceId::ParametricCurve(_) => {
                            FamilySiteProvenance::AlongParametricCurve {
                                location: StructuralPathLocationProvenance {
                                    path: guide.id,
                                    segment_index: location.segment_index(),
                                    parameter_bits: location.parameter().to_bits(),
                                },
                                path_order: guide_order,
                                sequence,
                                absolute_arc_position_bits: next_position.to_bits(),
                                local_arc_position_bits: local_position.to_bits(),
                            }
                        }
                        StructuralPathSourceId::GuideDimension(_) => {
                            FamilySiteProvenance::CurveAlongGuide {
                                location: StructuralPathLocationProvenance {
                                    path: guide.id,
                                    segment_index: location.segment_index(),
                                    parameter_bits: location.parameter().to_bits(),
                                },
                                guide_order,
                                sequence,
                                absolute_arc_position_bits: next_position.to_bits(),
                                local_arc_position_bits: local_position.to_bits(),
                            }
                        }
                    },
                });
                if output.len() > limit {
                    return Err(PatternPipelineError::new(
                        "coverage.curved_guides.along_guide_limit",
                        "curved along-guide site count exceeds the configured family limit",
                    ));
                }
                next_position += spacing;
                sequence += 1;
            }
            let endpoint = guide.path.end();
            previous_end = Some(endpoint);
            component_origin += total;
            if grouped[dimension]
                .iter()
                .position(|candidate| candidate.id == guide.id)
                .is_some_and(|index| {
                    grouped[dimension]
                        .get(index + 1)
                        .is_none_or(|next| (next.id.source, next.id.repetition_index) != repetition)
                })
            {
                guide_order += 1;
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod parametric_component_sampling_tests {
    use super::*;

    /// Proves one normal-offset cleanup repetition carries absolute interval, phase, and sequence
    /// across adjacent parametric components while suppressing their shared join exactly once.
    #[test]
    fn parametric_cleanup_components_continue_one_absolute_sampling_sequence() {
        let source = StructuralPathSourceId::ParametricCurve(PatternMechanismId(701));
        let first_id = StructuralPathInstanceId {
            source,
            repetition_index: 0,
            component_ordinal: 0,
        };
        let second_id = StructuralPathInstanceId {
            source,
            repetition_index: 0,
            component_ordinal: 1,
        };
        let path = |start, end| {
            CurvePath::line(Point2::new(start, 0.0), Point2::new(end, 0.0))
                .expect("finite cleanup component")
        };
        let grouped = vec![vec![
            StructuralPathInstance {
                id: first_id,
                source_structure_id: None,
                path: path(0.0, 10.0),
            },
            StructuralPathInstance {
                id: second_id,
                source_structure_id: None,
                path: path(10.0, 20.0),
            },
        ]];
        let bases = BTreeMap::from([(first_id, 4.0), (second_id, 4.0)]);
        let sites = curve_along_sites(
            &grouped,
            &[0],
            1.0,
            0.0,
            Some(6.0),
            &bases,
            &CanvasSpec {
                width: 40.0,
                height: 20.0,
            },
            &DensityMetric2D {
                across_x: 4.0,
                across_y: 2.0,
                aspect_locked: false,
            },
            Bounds::new(Point2::new(0.0, 0.0), Point2::new(40.0, 20.0)).expect("finite canvas"),
            Bounds::new(Point2::new(0.0, 0.0), Point2::new(40.0, 20.0)).expect("finite domain"),
            PatternMechanismId(702),
            32,
            &|| false,
        )
        .expect("component sampler succeeds");
        let samples = sites
            .iter()
            .map(|site| match &site.provenance {
                FamilySiteProvenance::AlongParametricCurve {
                    location,
                    sequence,
                    absolute_arc_position_bits,
                    local_arc_position_bits,
                    ..
                } => (
                    location.path.component_ordinal,
                    *sequence,
                    f64::from_bits(*absolute_arc_position_bits),
                    f64::from_bits(*local_arc_position_bits),
                ),
                _ => panic!("parametric sampler retains parametric provenance"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            samples,
            vec![
                (0, 0, 0.0, 0.0),
                (0, 1, 6.0, 6.0),
                (1, 2, 12.0, 2.0),
                (1, 3, 18.0, 8.0)
            ]
        );
    }
}

/// Classifies a completed document-space site without manufacturing guard output outside coverage.
fn site_scope(point: Point2, canvas: Bounds, generation_domain: Bounds) -> Option<SiteScope> {
    canvas
        .contains(point)
        .then_some(SiteScope::Canvas)
        .or_else(|| {
            generation_domain
                .contains(point)
                .then_some(SiteScope::Guard)
        })
}

/// Derives a deterministic square-equivalent basis from the active document density.
fn density_cell_basis(
    canvas: &CanvasSpec,
    density: &DensityMetric2D,
) -> Result<NominalCellBasis, PatternPipelineError> {
    let area = (canvas.width / density.across_x) * (canvas.height / density.across_y);
    let side = area.sqrt();
    NominalCellBasis::new(Vector2::new(side, 0.0), Vector2::new(0.0, side))
        .map_err(family_site_error)
}

/// Derives an intersection cell basis from ordered contributing straight guides and their resolved spacing.
fn straight_intersection_basis(
    guides: &[StraightGuide],
    coverage: &[GuideCoverage],
    contributors: &[GuideInstanceId],
) -> Result<NominalCellBasis, PatternPipelineError> {
    let mut best: Option<(f64, NominalCellBasis)> = None;
    for (left_index, left) in contributors.iter().enumerate() {
        for right in &contributors[left_index + 1..] {
            let Some(left_guide) = guides.iter().find(|guide| guide.id == *left) else {
                continue;
            };
            let Some(right_guide) = guides.iter().find(|guide| guide.id == *right) else {
                continue;
            };
            let Some(left_spacing) = coverage
                .iter()
                .find(|value| value.dimension_id == left.dimension_id)
                .map(|value| value.spacing)
            else {
                continue;
            };
            let Some(right_spacing) = coverage
                .iter()
                .find(|value| value.dimension_id == right.dimension_id)
                .map(|value| value.spacing)
            else {
                continue;
            };
            let cross = left_guide.tangent.x.mul_add(
                right_guide.tangent.y,
                -left_guide.tangent.y * right_guide.tangent.x,
            );
            if !cross.is_finite() || cross.abs() <= 1.0e-12 {
                continue;
            }
            let Ok(basis) = NominalCellBasis::new(
                left_guide.tangent.scale(right_spacing),
                right_guide.tangent.scale(left_spacing),
            ) else {
                continue;
            };
            let candidate = basis.diameter();
            if best.is_none_or(|(diameter, _)| candidate < diameter) {
                best = Some((candidate, basis));
            }
        }
    }
    best.map(|(_, basis)| basis).ok_or_else(|| {
        PatternPipelineError::new(
            "pattern.family.nominal_cell_basis",
            "guide intersection lacks a finite nonparallel contributor basis",
        )
    })
}

/// Bounds every possible Stage 20E1 nominal-cell diagonal before family allocation.
///
/// # Errors
///
/// Returns a stable pipeline error when validated density or repetition inputs cannot produce a
/// finite positive conservative bound.
pub fn maximum_nominal_cell_diameter(
    family: &FamilyCapability,
    canvas: &CanvasSpec,
    density: &DensityMetric2D,
) -> Result<f64, PatternPipelineError> {
    if let Some(parametric) = &family.parametric_curve {
        let radial_spacing = match &parametric.curve {
            ParametricCurve::Spiral(spiral) => spiral.radial_spacing,
        };
        let repetition_basis = match parametric.repetition {
            GuideRepetition::Single => radial_spacing,
            GuideRepetition::TransformStack {
                direction_degrees,
                spacing_multiplier,
            } => {
                let angle = direction_degrees.to_radians();
                directional_spacing(canvas, density, Vector2::new(angle.cos(), angle.sin()))?
                    * spacing_multiplier
            }
            GuideRepetition::NormalOffset { spacing, .. } => spacing,
        };
        let bound = parametric
            .site_interval
            .unwrap_or(repetition_basis)
            .max(repetition_basis);
        return (bound.is_finite() && bound > 0.0).then_some(bound).ok_or(
            PatternPipelineError::new(
                "pattern.family.nominal_cell_basis",
                "parametric sources require a finite positive radial or site spacing",
            ),
        );
    }
    let spacing_x = canvas.width / density.across_x;
    let spacing_y = canvas.height / density.across_y;
    if !spacing_x.is_finite() || !spacing_y.is_finite() || spacing_x <= 0.0 || spacing_y <= 0.0 {
        return Err(PatternPipelineError::new(
            "pattern.family.nominal_cell_basis",
            "density must produce finite positive nominal-cell spacing",
        ));
    }
    let rectangular_diagonal = spacing_x.hypot(spacing_y);
    let maximum_directional_spacing = spacing_x.max(spacing_y);
    let selected_spacing_bound = |dimensions: &[StraightGuideDimension]| {
        family
            .site_selection
            .iter()
            .filter_map(|id| dimensions.iter().find(|dimension| dimension.id == *id))
            .try_fold(0.0_f64, |maximum, dimension| {
                let radians = dimension.baseline_angle_degrees.to_radians();
                let normal = Vector2::new(radians.cos(), radians.sin());
                directional_spacing(canvas, density, normal)
                    .map(|spacing| maximum.max(spacing * dimension.repetition.spacing_multiplier))
            })
    };
    let generic_spacing_bound = || {
        family
            .generic_guides
            .as_ref()
            .expect("generic branch checked above")
            .dimensions
            .iter()
            .filter(|dimension| family.site_selection.contains(&dimension.id))
            .try_fold(0.0_f64, |maximum, dimension| match dimension.repetition {
                GuideRepetition::Single => Ok(maximum),
                GuideRepetition::TransformStack {
                    direction_degrees,
                    spacing_multiplier,
                } => {
                    let angle = (dimension.baseline_angle_degrees + direction_degrees).to_radians();
                    directional_spacing(canvas, density, Vector2::new(angle.cos(), angle.sin()))
                        .map(|spacing| maximum.max(spacing * spacing_multiplier))
                }
                GuideRepetition::NormalOffset { spacing, .. } => Ok(maximum.max(spacing)),
            })
    };
    let maximum = match family.product {
        StructuralProductCapability::RandomSites => rectangular_diagonal,
        StructuralProductCapability::GuideIntersections => {
            if family.generic_guides.is_some() {
                2.0 * generic_spacing_bound()?.max(maximum_directional_spacing)
            } else {
                2.0 * selected_spacing_bound(&family.dimensions)?
            }
        }
        StructuralProductCapability::AlongGuideSites => {
            let along_multiplier = family.along_interval_multiplier.unwrap_or(1.0);
            if family.generic_guides.is_some() {
                maximum_directional_spacing * (along_multiplier + 1.0)
            } else {
                let resolved_spacing = selected_spacing_bound(&family.dimensions)?;
                resolved_spacing * (along_multiplier + 1.0)
            }
        }
        StructuralProductCapability::ParametricPaths => {
            generic_spacing_bound()?.max(maximum_directional_spacing)
        }
    };
    (maximum.is_finite() && maximum > 0.0)
        .then_some(maximum)
        .ok_or_else(|| {
            PatternPipelineError::new(
                "pattern.family.nominal_cell_basis",
                "family inputs cannot produce a finite positive nominal-cell bound",
            )
        })
}

/// Returns the largest nominal spacing among every emitted guide dimension, independent of site selection.
///
/// # Errors
///
/// Rejects nonfinite density or repetition values before a connected-path family envelope is allocated.
pub fn maximum_emitted_guide_spacing(
    family: &FamilyCapability,
    canvas: &CanvasSpec,
    density: &DensityMetric2D,
) -> Result<f64, PatternPipelineError> {
    if let Some(parametric) = &family.parametric_curve {
        let spacing = match (&parametric.curve, &parametric.repetition) {
            (ParametricCurve::Spiral(_), GuideRepetition::NormalOffset { spacing, .. }) => *spacing,
            (
                ParametricCurve::Spiral(_),
                GuideRepetition::TransformStack {
                    direction_degrees,
                    spacing_multiplier,
                },
            ) => {
                let angle = direction_degrees.to_radians();
                directional_spacing(canvas, density, Vector2::new(angle.cos(), angle.sin()))?
                    * spacing_multiplier
            }
            (ParametricCurve::Spiral(spiral), GuideRepetition::Single) => spiral.radial_spacing,
        };
        return (spacing.is_finite() && spacing > 0.0)
            .then_some(spacing)
            .ok_or(PatternPipelineError::new(
                "pattern.family.guide_spacing",
                "parametric sources require finite positive transverse spacing",
            ));
    }
    let dimensions = family
        .generic_guides
        .as_ref()
        .map(|guide| &guide.dimensions);
    let mut maximum = 0.0_f64;
    if let Some(dimensions) = dimensions {
        for dimension in dimensions {
            let spacing = match dimension.repetition {
                GuideRepetition::Single => {
                    (canvas.width / density.across_x).max(canvas.height / density.across_y)
                }
                GuideRepetition::TransformStack {
                    direction_degrees,
                    spacing_multiplier,
                } => {
                    let angle = (dimension.baseline_angle_degrees + direction_degrees).to_radians();
                    directional_spacing(canvas, density, Vector2::new(angle.cos(), angle.sin()))?
                        * spacing_multiplier
                }
                GuideRepetition::NormalOffset { spacing, .. } => spacing,
            };
            maximum = maximum.max(spacing);
        }
    } else {
        for dimension in &family.dimensions {
            let radians = dimension.baseline_angle_degrees.to_radians();
            maximum = maximum.max(
                directional_spacing(canvas, density, Vector2::new(radians.cos(), radians.sin()))?
                    * dimension.repetition.spacing_multiplier,
            );
        }
    }
    (maximum.is_finite() && maximum > 0.0)
        .then_some(maximum)
        .ok_or(PatternPipelineError::new(
            "pattern.family.guide_spacing",
            "emitted guides require a finite positive nominal spacing",
        ))
}

/// Extends a family fingerprint with every published site's exact nominal basis and diameter.
fn family_site_fingerprint(base: &str, sites: &[FamilySite]) -> String {
    let mut bytes = base.as_bytes().to_vec();
    bytes.extend(b"|nominal-cell-basis-v1");
    for site in sites {
        bytes.extend(site.id.mechanism_id.0.to_le_bytes());
        bytes.extend(
            u64::try_from(site.id.ordinal)
                .expect("usize fits u64")
                .to_le_bytes(),
        );
        bytes.extend(site.position.x.to_bits().to_le_bytes());
        bytes.extend(site.position.y.to_bits().to_le_bytes());
        bytes.extend(site.nominal_cell_basis.axis_a.x.to_bits().to_le_bytes());
        bytes.extend(site.nominal_cell_basis.axis_a.y.to_bits().to_le_bytes());
        bytes.extend(site.nominal_cell_basis.axis_b.x.to_bits().to_le_bytes());
        bytes.extend(site.nominal_cell_basis.axis_b.y.to_bits().to_le_bytes());
        bytes.extend(site.nominal_cell_basis.diameter().to_bits().to_le_bytes());
    }
    format!("{base}:nominal-cell-basis:{}", fnv1a64(bytes))
}

/// Hashes complete resolved generic guide intent and layout inputs under the fixed Stage 20D identity prefix.
fn generic_curve_fingerprint(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    generic: &GenericGuideCapability,
) -> String {
    let mut bytes =
        b"toniator-stage-20d-guide-family-v1|arc-policy-fixed-90-degree-cubic-v1".to_vec();
    bytes.extend(family.provenance.definition_id.to_le_bytes());
    for id in &family.provenance.mechanism_ids {
        bytes.extend(id.0.to_le_bytes());
    }
    bytes.push(match family.product {
        StructuralProductCapability::GuideIntersections => 1,
        StructuralProductCapability::AlongGuideSites => 2,
        StructuralProductCapability::RandomSites => 3,
        StructuralProductCapability::ParametricPaths => 4,
    });
    for id in &family.site_selection {
        bytes.extend(id.0.to_le_bytes());
    }
    bytes.extend(family.merge_epsilon.unwrap_or(0.0).to_bits().to_le_bytes());
    bytes.extend(
        family
            .along_interval_multiplier
            .unwrap_or(0.0)
            .to_bits()
            .to_le_bytes(),
    );
    bytes.extend(family.along_phase.unwrap_or(0.0).to_bits().to_le_bytes());
    for dimension in &generic.dimensions {
        bytes.extend(dimension.id.0.to_le_bytes());
        bytes.extend(dimension.baseline_angle_degrees.to_bits().to_le_bytes());
        bytes.extend(dimension.phase.to_bits().to_le_bytes());
        match &dimension.prototype {
            GuidePrototype::AuthoredOpenPath { structure_id } => {
                bytes.push(1);
                bytes.extend(structure_id.0.to_le_bytes());
            }
            GuidePrototype::CircularArc {
                center,
                radius,
                start_angle_degrees,
                sweep_angle_degrees,
            } => {
                bytes.push(2);
                for value in [
                    center.x,
                    center.y,
                    *radius,
                    *start_angle_degrees,
                    *sweep_angle_degrees,
                ] {
                    bytes.extend(value.to_bits().to_le_bytes());
                }
            }
        }
        match dimension.repetition {
            GuideRepetition::Single => bytes.push(1),
            GuideRepetition::TransformStack {
                direction_degrees,
                spacing_multiplier,
            } => {
                bytes.push(2);
                bytes.extend(direction_degrees.to_bits().to_le_bytes());
                bytes.extend(spacing_multiplier.to_bits().to_le_bytes());
            }
            GuideRepetition::NormalOffset {
                spacing,
                sides,
                cleanup,
            } => {
                bytes.push(3);
                bytes.extend(spacing.to_bits().to_le_bytes());
                bytes.push(match sides {
                    OffsetSides::Left => 1,
                    OffsetSides::Right => 2,
                    OffsetSides::Both => 3,
                });
                bytes.push(match cleanup {
                    OffsetCleanup::DissolveCrossings => 1,
                });
                let limits = PathOffsetLimits::default();
                bytes.extend(PATH_OFFSET_ALGORITHM_CONTRACT_ID.as_bytes());
                bytes.push(limits.maximum_subdivision_depth);
                for limit in [
                    limits.maximum_segments,
                    limits.maximum_components,
                    limits.maximum_cleanup_pairs,
                    limits.maximum_cusp_isolation_work,
                ] {
                    bytes.extend(
                        u64::try_from(limit)
                            .expect("path-offset fixed limit fits u64")
                            .to_le_bytes(),
                    );
                }
                bytes.extend(limits.tolerance.to_bits().to_le_bytes());
            }
        }
    }
    for (source, path) in &generic.resolved_paths {
        bytes.extend(source.map_or(0, |id| id.0).to_le_bytes());
        bytes.extend(
            u64::try_from(path.segments().len())
                .expect("usize fits u64")
                .to_le_bytes(),
        );
        for segment in path.segments() {
            match segment {
                CurveSegment::Line(line) => {
                    bytes.push(1);
                    for point in [line.start(), line.end()] {
                        bytes.extend(point.x.to_bits().to_le_bytes());
                        bytes.extend(point.y.to_bits().to_le_bytes());
                    }
                }
                CurveSegment::CubicBezier(cubic) => {
                    bytes.push(2);
                    for point in [
                        cubic.start(),
                        cubic.control_1(),
                        cubic.control_2(),
                        cubic.end(),
                    ] {
                        bytes.extend(point.x.to_bits().to_le_bytes());
                        bytes.extend(point.y.to_bits().to_le_bytes());
                    }
                }
            }
        }
    }
    for value in [
        request.canvas.width,
        request.canvas.height,
        request.density.across_x,
        request.density.across_y,
        request.rotation_degrees,
        request.translation_x,
        request.translation_y,
        request.support_radius,
    ] {
        bytes.extend(value.to_bits().to_le_bytes());
    }
    bytes.push(u8::from(request.density.aspect_locked));
    bytes.extend(request.guard_steps.to_le_bytes());
    bytes.extend(
        u64::try_from(request.max_family_candidates)
            .expect("usize fits u64")
            .to_le_bytes(),
    );
    bytes.extend(ANTIALIAS_MARGIN.to_bits().to_le_bytes());
    format!("toniator-stage-20d-guide-family-v1:{}", fnv1a64(bytes))
}

/// Converts a validated family-site failure into the typed evaluation error boundary.
fn family_site_error(error: FamilySiteError) -> PatternPipelineError {
    PatternPipelineError::new(error.path(), error.message())
}

/// Publishes existing straight intersections through the shared site authority.
fn family_sites_from_grid(
    output: &GridFamilyOutput,
    product_mechanism_id: PatternMechanismId,
) -> Result<FamilySiteSet, PatternPipelineError> {
    let sites = output
        .sites
        .iter()
        .enumerate()
        .map(|(ordinal, site)| {
            let nominal_cell_basis = straight_intersection_basis(
                &output.guides,
                &output.coverage,
                &site.provenance.contributors,
            )?;
            Ok(FamilySite {
                id: FamilySiteId {
                    mechanism_id: product_mechanism_id,
                    ordinal,
                },
                position: site.position,
                nominal_cell_basis,
                scope: site.scope,
                provenance: FamilySiteProvenance::GuideIntersection {
                    contributors: site.provenance.contributors.clone(),
                },
            })
        })
        .collect::<Result<Vec<_>, PatternPipelineError>>()?;
    FamilySiteSet::new(
        family_site_fingerprint(&output.family_fingerprint, &sites),
        product_mechanism_id,
        sites,
    )
    .map_err(family_site_error)
}

/// Publishes generalized guide products without fabricating intersection facts.
fn family_sites_from_generalized(
    output: &GeneralizedStraightGuideOutput,
    family: &FamilyCapability,
    request: &GridInspectRequest,
    product_mechanism_id: PatternMechanismId,
) -> Result<FamilySiteSet, PatternPipelineError> {
    let sites = output
        .sites
        .iter()
        .enumerate()
        .map(|(ordinal, site)| {
            let nominal_cell_basis = match &site.provenance {
                GeneralizedSiteProvenance::Intersection { contributors } => {
                    straight_intersection_basis(&output.guides, &output.coverage, contributors)?
                }
                GeneralizedSiteProvenance::AlongGuide { guide_id, .. } => {
                    let guide = output
                        .guides
                        .iter()
                        .find(|guide| guide.id == *guide_id)
                        .ok_or_else(|| {
                            PatternPipelineError::new(
                                "pattern.family.nominal_cell_basis",
                                "along-guide site lacks its resolved guide",
                            )
                        })?;
                    let transverse_spacing = output
                        .coverage
                        .iter()
                        .find(|value| value.dimension_id == guide_id.dimension_id)
                        .map(|value| value.spacing)
                        .ok_or_else(|| {
                            PatternPipelineError::new(
                                "pattern.family.nominal_cell_basis",
                                "along-guide site lacks resolved transverse spacing",
                            )
                        })?;
                    let along_interval =
                        directional_spacing(&request.canvas, &request.density, guide.normal)?
                            * family.along_interval_multiplier.unwrap_or(1.0);
                    NominalCellBasis::new(
                        guide.tangent.scale(along_interval),
                        guide.normal.scale(transverse_spacing),
                    )
                    .map_err(family_site_error)?
                }
            };
            let provenance = match &site.provenance {
                GeneralizedSiteProvenance::Intersection { contributors } => {
                    FamilySiteProvenance::GuideIntersection {
                        contributors: contributors.clone(),
                    }
                }
                GeneralizedSiteProvenance::AlongGuide {
                    guide_id,
                    guide_order,
                    sequence,
                    absolute_arc_position_bits,
                    local_arc_position_bits,
                } => FamilySiteProvenance::AlongGuide {
                    guide_id: *guide_id,
                    guide_order: *guide_order,
                    sequence: *sequence,
                    absolute_arc_position_bits: *absolute_arc_position_bits,
                    local_arc_position_bits: *local_arc_position_bits,
                },
            };
            Ok(FamilySite {
                id: FamilySiteId {
                    mechanism_id: product_mechanism_id,
                    ordinal,
                },
                position: site.position,
                nominal_cell_basis,
                scope: site.scope,
                provenance,
            })
        })
        .collect::<Result<Vec<_>, PatternPipelineError>>()?;
    FamilySiteSet::new(
        family_site_fingerprint(&output.family_fingerprint, &sites),
        product_mechanism_id,
        sites,
    )
    .map_err(family_site_error)
}

impl TypedFamilyStructure {
    /// Captures straight-grid metadata as ordered raw line paths for connected realization.
    ///
    /// # Panics
    ///
    /// Panics only if a previously validated grid guide cannot rebuild its own finite line path.
    fn from_grid(output: &GridFamilyOutput, guide_mechanism_id: PatternMechanismId) -> Self {
        let paths = output
            .guides
            .iter()
            .map(|guide| StructuralPathInstance {
                id: StructuralPathInstanceId::guide_dimension(
                    GuideDimensionId(guide.id.dimension_id),
                    guide.id.index,
                    guide.id.component_ordinal,
                ),
                source_structure_id: None,
                path: CurvePath::line(guide.start, guide.end)
                    .expect("validated straight guide endpoints build a path"),
            })
            .collect::<Vec<_>>();
        let structural_path_set = (!paths.is_empty()).then(|| {
            StructuralPathSet::new(output.family_fingerprint.clone(), guide_mechanism_id, paths)
                .expect("validated ordered straight guides build a path set")
        });
        let guide_nominal_bases = output
            .guides
            .iter()
            .filter_map(|guide| {
                output
                    .coverage
                    .iter()
                    .find(|coverage| coverage.dimension_id == guide.id.dimension_id)
                    .map(|coverage| {
                        (
                            StructuralPathInstanceId::guide_dimension(
                                GuideDimensionId(guide.id.dimension_id),
                                guide.id.index,
                                guide.id.component_ordinal,
                            ),
                            coverage.spacing,
                        )
                    })
            })
            .collect();
        Self {
            coverage: output.coverage.clone(),
            guides: output.guides.clone(),
            support_radius: output.support_radius,
            guard_steps: output.guard_steps,
            antialias_margin: output.antialias_margin,
            generation_domain: output.generation_domain,
            structural_path_set,
            guide_nominal_bases,
        }
    }
}

/// Bounded, reproducible structural diagnostics.  The collection contains no
/// unbounded candidate log: accepted-site provenance carries its candidate
/// ordinal while aggregate counts make an unsatisfiable request inspectable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RandomSiteDiagnostics {
    pub requested_sites: usize,
    pub achieved_sites: usize,
    pub candidates_considered: usize,
    pub rejected_by_density: usize,
    pub rejected_by_exclusion: usize,
    pub rejected_outside_envelope: usize,
    pub canvas_sites: usize,
    pub guard_sites: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RandomSiteProvenance {
    pub candidate_ordinal: usize,
    pub accepted_ordinal: usize,
    pub scope: SiteScope,
    pub exclusion_neighbor_ordinal: Option<usize>,
}

struct RandomSiteEvaluation {
    family_fingerprint: String,
    generation_domain: Bounds,
    sites: Vec<FamilySite>,
    diagnostics: RandomSiteDiagnostics,
}

struct SpatialIndex {
    cell_size: f64,
    cells: BTreeMap<(i64, i64), Vec<usize>>,
    neighbor_work: usize,
    neighbor_limit: usize,
}

impl SpatialIndex {
    fn new(cell_size: f64, neighbor_limit: usize) -> Result<Self, PatternPipelineError> {
        if !cell_size.is_finite() || cell_size <= 0.0 {
            return Err(PatternPipelineError::new(
                "coverage.random_sites.spatial_index",
                "exclusion cell size must be positive and finite",
            ));
        }
        Ok(Self {
            cell_size,
            cells: BTreeMap::new(),
            neighbor_work: 0,
            neighbor_limit,
        })
    }
    fn cell(&self, point: Point2) -> Result<(i64, i64), PatternPipelineError> {
        let x = (point.x / self.cell_size).floor();
        let y = (point.y / self.cell_size).floor();
        if !x.is_finite()
            || !y.is_finite()
            || x < i64::MIN as f64
            || x > i64::MAX as f64
            || y < i64::MIN as f64
            || y > i64::MAX as f64
        {
            return Err(PatternPipelineError::new(
                "coverage.random_sites.spatial_index",
                "exclusion cell coordinate is not representable",
            ));
        }
        Ok((x as i64, y as i64))
    }
    fn find_conflict(
        &mut self,
        point: Point2,
        accepted: &[(Point2, usize, SiteScope)],
        distance: f64,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<usize>, PatternPipelineError> {
        if distance == 0.0 {
            return Ok(None);
        }
        let (x, y) = self.cell(point)?;
        for dx in -1_i64..=1 {
            for dy in -1_i64..=1 {
                if is_cancelled() {
                    return Err(PatternPipelineError::new(
                        "evaluation.cancelled",
                        "evaluation was cancelled",
                    ));
                }
                let key = (
                    x.checked_add(dx).ok_or(PatternPipelineError::new(
                        "coverage.random_sites.spatial_index",
                        "exclusion neighbor coordinate overflowed",
                    ))?,
                    y.checked_add(dy).ok_or(PatternPipelineError::new(
                        "coverage.random_sites.spatial_index",
                        "exclusion neighbor coordinate overflowed",
                    ))?,
                );
                if let Some(indices) = self.cells.get(&key) {
                    self.neighbor_work = self.neighbor_work.checked_add(indices.len()).ok_or(
                        PatternPipelineError::new(
                            "coverage.random_sites.neighbor_limit",
                            "exclusion neighbor work overflowed",
                        ),
                    )?;
                    if self.neighbor_work > self.neighbor_limit {
                        return Err(PatternPipelineError::new(
                            "coverage.random_sites.neighbor_limit",
                            "exclusion neighbor work exceeds configured limit",
                        ));
                    }
                    for &index in indices {
                        // A populated cell can contain bounded-but-large work.
                        // Poll every deterministic index so cancellation cannot
                        // be delayed until the next candidate or cell.
                        if is_cancelled() {
                            return Err(PatternPipelineError::new(
                                "evaluation.cancelled",
                                "evaluation was cancelled",
                            ));
                        }
                        if point_distance(point, accepted[index].0) < distance {
                            return Ok(Some(index));
                        }
                    }
                }
            }
        }
        Ok(None)
    }
    fn insert(&mut self, point: Point2, index: usize) -> Result<(), PatternPipelineError> {
        let key = self.cell(point)?;
        self.cells.entry(key).or_default().push(index);
        Ok(())
    }
}

/// A small, fixed xorshift32 generator.  The sequence is defined entirely by
/// u32 wrapping operations and so does not inherit platform RNG behavior.
#[derive(Clone, Copy)]
struct StablePrng(u32);

impl StablePrng {
    fn new(seed: u32) -> Self {
        // xorshift32 has an all-zero absorbing state; map that single user
        // seed to a documented nonzero state without changing other seeds.
        Self(if seed == 0 { 0x6d2b_79f5 } else { seed })
    }

    fn next_u32(&mut self) -> u32 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 17;
        value ^= value << 5;
        self.0 = value;
        value
    }

    fn unit(&mut self) -> f64 {
        f64::from(self.next_u32()) / (f64::from(u32::MAX) + 1.0)
    }

    /// Fixed twelve-uniform central-limit approximation.  It uses only the
    /// specified u32 stream and basic IEEE add/subtract operations, avoiding
    /// platform libm behavior from Box-Muller.
    fn clustered_offset(&mut self) -> f64 {
        let mut sum = 0.0;
        for _ in 0..12 {
            sum += self.unit();
        }
        sum - 6.0
    }
}

/// Evaluates bounded random sites and publishes only truthful random provenance.
fn evaluate_random_sites_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    source: Option<&SourceField>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<RandomSiteEvaluation, PatternPipelineError> {
    if is_cancelled() {
        return Err(PatternPipelineError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ));
    }
    validate_straight_request(&StraightGuideInspectRequest {
        canvas: request.canvas.clone(),
        density: request.density.clone(),
        rotation_degrees: request.rotation_degrees,
        translation_x: request.translation_x,
        translation_y: request.translation_y,
        guard_steps: request.guard_steps,
        support_radius: request.support_radius,
        max_family_candidates: request.max_family_candidates,
    })
    .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
    let random = family.random.as_ref().ok_or(PatternPipelineError::new(
        "pattern.family.random_sites",
        "random-site capability requires a declared random mechanism chain",
    ))?;
    let weighted_source = match &random.density_modulation {
        SiteDensityModulation::Uniform => None,
        SiteDensityModulation::ArtworkWeighted { .. } => {
            Some(source.ok_or(PatternPipelineError::new(
                "pattern.family.random_sites.source",
                "artwork-weighted site placement requires decoded source pixels",
            ))?)
        }
    };
    let canvas = Bounds::new(
        Point2::new(0.0, 0.0),
        Point2::new(request.canvas.width, request.canvas.height),
    )
    .expect("validated canvas forms finite bounds");
    let expected_visible = checked_requested_count(request)?;
    let neighborhood = random_neighborhood(random, request.support_radius);
    let cluster_guard = match random.character {
        RandomSiteCharacter::Clustered { cluster_spread, .. } => cluster_spread * 3.0,
        _ => 0.0,
    };
    let guard = request.support_radius.max(neighborhood).max(cluster_guard)
        + ANTIALIAS_MARGIN
        + f64::from(request.guard_steps) * neighborhood.max(1.0);
    if !guard.is_finite() {
        return Err(PatternPipelineError::new(
            "pattern.family.random_sites.coverage",
            "random-site guard envelope must be finite",
        ));
    }
    let padded = canvas
        .expanded(guard)
        .expect("validated finite guard expands bounds");
    let transform = AffineTransform2D::rotate_about_then_translate(
        Point2::new(request.canvas.width / 2.0, request.canvas.height / 2.0),
        request.rotation_degrees,
        Vector2::new(request.translation_x, request.translation_y),
    )
    .ok_or(PatternPipelineError::new(
        "channel.pattern.layout",
        "transform is not finite",
    ))?;
    let local = transform
        .inverse_bounds(padded)
        .ok_or(PatternPipelineError::new(
            "pattern.family.random_sites.coverage",
            "inverse random-site coverage bounds must be finite",
        ))?;
    let visible_area = request.canvas.width * request.canvas.height;
    let padded_area = (padded.max.x - padded.min.x) * (padded.max.y - padded.min.y);
    let requested = (expected_visible as f64 * padded_area / visible_area)
        .ceil()
        .max(1.0) as usize;
    if requested > request.max_family_candidates {
        return Err(PatternPipelineError::new(
            "coverage.candidate_limit",
            "random-site requested count exceeds the configured candidate limit",
        ));
    }
    let candidate_budget = usize::try_from(random.maximum_attempts)
        .ok()
        .and_then(|attempts| attempts.checked_mul(requested))
        .ok_or(PatternPipelineError::new(
            "coverage.random_sites.attempts",
            "random-site attempt budget overflowed",
        ))?
        .min(request.max_family_candidates);
    if candidate_budget == 0 {
        return Err(PatternPipelineError::new(
            "coverage.random_sites.attempts",
            "random-site attempt budget must be nonzero",
        ));
    }
    let mut prng = StablePrng::new(random.seed);
    let parents = cluster_parents(&random.character, local, requested, &mut prng);
    let mut accepted: Vec<(Point2, usize, SiteScope)> = Vec::with_capacity(requested);
    let exclusion_distance = required_exclusion_distance(random, request.support_radius);
    let mut spatial_index = (exclusion_distance > 0.0)
        .then(|| {
            SpatialIndex::new(
                exclusion_distance,
                usize::try_from(random.maximum_neighbor_checks).expect("u32 fits usize"),
            )
        })
        .transpose()?;
    let mut provenance = Vec::with_capacity(requested);
    let mut rejected_by_density = 0;
    let mut rejected_by_exclusion = 0;
    let mut rejected_outside_envelope = 0;
    let mut candidates_considered = 0;
    for candidate_ordinal in 0..candidate_budget {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        if accepted.len() == requested {
            break;
        }
        candidates_considered = candidate_ordinal + 1;
        let local_point = random_candidate(&random.character, local, &parents, &mut prng);
        let point = transform.apply_point(local_point);
        if !padded.contains(point) {
            rejected_outside_envelope += 1;
            continue;
        }
        let density_weight = match (&random.density_modulation, weighted_source) {
            (SiteDensityModulation::Uniform, _) => 1.0,
            (
                SiteDensityModulation::ArtworkWeighted {
                    mapping,
                    strength,
                    response,
                },
                Some(field),
            ) => {
                let sampled = field
                    .sample_density_weight(point, &request.canvas, *mapping)
                    .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
                let shaped = artwork_weight_response(sampled, response);
                (1.0 - strength) + strength * shaped
            }
            _ => unreachable!("weighted source requirement is checked above"),
        };
        if prng.unit() > density_weight {
            rejected_by_density += 1;
            continue;
        }
        // `Option::transpose` here would retain the fact that an index exists
        // as `Some(None)`.  That is not a collision: only an actual accepted
        // ordinal rejects the candidate.
        let conflict = match spatial_index.as_mut() {
            Some(index) => {
                index.find_conflict(point, &accepted, exclusion_distance, is_cancelled)?
            }
            None => None,
        };
        if conflict.is_some() {
            rejected_by_exclusion += 1;
            // Rejected candidates are aggregated rather than retained.  The
            // accepted record remains bounded and identifies its own ordinal.
            continue;
        }
        let scope = if canvas.contains(point) {
            SiteScope::Canvas
        } else {
            SiteScope::Guard
        };
        let accepted_ordinal = accepted.len();
        accepted.push((point, candidate_ordinal, scope));
        if let Some(index) = &mut spatial_index {
            index.insert(point, accepted_ordinal)?;
        }
        provenance.push(RandomSiteProvenance {
            candidate_ordinal,
            accepted_ordinal,
            scope,
            exclusion_neighbor_ordinal: None,
        });
    }
    let nominal_cell_basis = density_cell_basis(&request.canvas, &request.density)?;
    let sites: Vec<_> = accepted
        .iter()
        .enumerate()
        .map(|(index, (point, _, scope))| FamilySite {
            id: FamilySiteId {
                mechanism_id: family.provenance.mechanism_ids[3],
                ordinal: index,
            },
            position: *point,
            nominal_cell_basis,
            scope: *scope,
            provenance: FamilySiteProvenance::Random {
                candidate_ordinal: provenance[index].candidate_ordinal,
                accepted_ordinal: provenance[index].accepted_ordinal,
                exclusion_neighbor_ordinal: provenance[index].exclusion_neighbor_ordinal,
            },
        })
        .collect();
    let canvas_sites = sites
        .iter()
        .filter(|site| site.scope == SiteScope::Canvas)
        .count();
    let diagnostics = RandomSiteDiagnostics {
        requested_sites: requested,
        achieved_sites: sites.len(),
        candidates_considered,
        rejected_by_density,
        rejected_by_exclusion,
        rejected_outside_envelope,
        canvas_sites,
        guard_sites: sites.len() - canvas_sites,
    };
    Ok(RandomSiteEvaluation {
        family_fingerprint: random_family_fingerprint(family, request, weighted_source),
        generation_domain: padded,
        sites,
        diagnostics,
    })
}

fn checked_requested_count(request: &GridInspectRequest) -> Result<usize, PatternPipelineError> {
    let expected = request.density.across_x * request.density.across_y;
    if !expected.is_finite() || expected <= 0.0 || expected > usize::MAX as f64 {
        return Err(PatternPipelineError::new(
            "channel.pattern.layout.density",
            "random-site requested count is not representable",
        ));
    }
    Ok(expected.round().max(1.0) as usize)
}

fn random_neighborhood(random: &RandomSiteCapability, support: f64) -> f64 {
    let character = match random.character {
        RandomSiteCharacter::Even {
            minimum_center_distance,
        } => minimum_center_distance,
        _ => 0.0,
    };
    character.max(exclusion_distance(&random.exclusion, support))
}

fn required_exclusion_distance(random: &RandomSiteCapability, support: f64) -> f64 {
    random_neighborhood(random, support)
}

fn exclusion_distance(policy: &SiteExclusionPolicy, support: f64) -> f64 {
    match policy {
        SiteExclusionPolicy::None => 0.0,
        SiteExclusionPolicy::MinimumCenterDistance { minimum } => *minimum,
        SiteExclusionPolicy::VisibleMarkMargin { margin, sizing } => match sizing {
            VisibleMarkSizingPolicy::MaximumSupportRadius => support * 2.0 + margin,
        },
    }
}

fn cluster_parents(
    character: &RandomSiteCharacter,
    bounds: Bounds,
    requested: usize,
    prng: &mut StablePrng,
) -> Vec<Point2> {
    let RandomSiteCharacter::Clustered {
        cluster_density, ..
    } = character
    else {
        return Vec::new();
    };
    let count = ((requested as f64 * cluster_density).round() as usize).clamp(1, requested.max(1));
    (0..count)
        .map(|_| {
            Point2::new(
                bounds.min.x + prng.unit() * (bounds.max.x - bounds.min.x),
                bounds.min.y + prng.unit() * (bounds.max.y - bounds.min.y),
            )
        })
        .collect()
}

fn random_candidate(
    character: &RandomSiteCharacter,
    bounds: Bounds,
    parents: &[Point2],
    prng: &mut StablePrng,
) -> Point2 {
    if let RandomSiteCharacter::Clustered {
        cluster_spread,
        cluster_strength,
        ..
    } = character
        && !parents.is_empty()
        && prng.unit() < *cluster_strength
    {
        let index = (prng.unit() * parents.len() as f64) as usize;
        let parent = parents[index.min(parents.len() - 1)];
        return Point2::new(
            (parent.x + prng.clustered_offset() * cluster_spread).clamp(bounds.min.x, bounds.max.x),
            (parent.y + prng.clustered_offset() * cluster_spread).clamp(bounds.min.y, bounds.max.y),
        );
    }
    Point2::new(
        bounds.min.x + prng.unit() * (bounds.max.x - bounds.min.x),
        bounds.min.y + prng.unit() * (bounds.max.y - bounds.min.y),
    )
}

/// Fixed-IEEE response curve for decoder-owned artwork weighting. This avoids
/// libm so the acceptance decision has no platform math-library dependency.
fn artwork_weight_response(sampled: f64, response: &ArtworkWeightResponse) -> f64 {
    match response {
        ArtworkWeightResponse::Linear => sampled,
        ArtworkWeightResponse::Smoothstep => {
            let x = sampled.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        }
    }
}

fn point_distance(first: Point2, second: Point2) -> f64 {
    let dx = first.x - second.x;
    let dy = first.y - second.y;
    (dx * dx + dy * dy).sqrt()
}

fn random_family_fingerprint(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    source: Option<&SourceField>,
) -> String {
    let random = family
        .random
        .as_ref()
        .expect("random family has random capability");
    let mut bytes = b"toniator-stage-16b-random-sites-v1".to_vec();
    bytes.extend(family.provenance.definition_id.to_le_bytes());
    for id in &family.provenance.mechanism_ids {
        bytes.extend(id.0.to_le_bytes());
    }
    bytes.extend(random.seed.to_le_bytes());
    match &random.character {
        RandomSiteCharacter::RawUniform => bytes.push(1),
        RandomSiteCharacter::Even {
            minimum_center_distance,
        } => {
            bytes.push(2);
            bytes.extend(minimum_center_distance.to_bits().to_le_bytes());
        }
        RandomSiteCharacter::Clustered {
            cluster_density,
            cluster_spread,
            cluster_strength,
        } => {
            bytes.push(3);
            bytes.extend(cluster_density.to_bits().to_le_bytes());
            bytes.extend(cluster_spread.to_bits().to_le_bytes());
            bytes.extend(cluster_strength.to_bits().to_le_bytes());
        }
    }
    match &random.density_modulation {
        SiteDensityModulation::Uniform => bytes.push(1),
        SiteDensityModulation::ArtworkWeighted {
            mapping,
            strength,
            response,
        } => {
            bytes.push(2);
            bytes.extend(strength.to_bits().to_le_bytes());
            bytes.push(mapping_component_code(mapping.component));
            bytes.push(u8::from(mapping.inverted));
            bytes.extend(mapping.gain.to_bits().to_le_bytes());
            bytes.extend(mapping.bias.to_bits().to_le_bytes());
            match response {
                ArtworkWeightResponse::Linear => bytes.push(1),
                ArtworkWeightResponse::Smoothstep => bytes.push(2),
            }
            let source = source.expect("weighted random family supplies decoded source");
            bytes.extend(source.identity().content_hash.bytes());
            bytes.extend(source.identity().decoded_pixel_hash.bytes());
            bytes.extend(source.identity().width.to_le_bytes());
            bytes.extend(source.identity().height.to_le_bytes());
        }
    }
    match &random.exclusion {
        SiteExclusionPolicy::None => bytes.push(1),
        SiteExclusionPolicy::MinimumCenterDistance { minimum } => {
            bytes.push(2);
            bytes.extend(minimum.to_bits().to_le_bytes());
        }
        SiteExclusionPolicy::VisibleMarkMargin { margin, sizing } => {
            bytes.push(3);
            bytes.extend(margin.to_bits().to_le_bytes());
            bytes.push(match sizing {
                VisibleMarkSizingPolicy::MaximumSupportRadius => 1,
            });
        }
    }
    bytes.extend(random.maximum_attempts.to_le_bytes());
    bytes.extend(random.maximum_neighbor_checks.to_le_bytes());
    bytes.extend(request.canvas.width.to_bits().to_le_bytes());
    bytes.extend(request.canvas.height.to_bits().to_le_bytes());
    bytes.extend(request.density.across_x.to_bits().to_le_bytes());
    bytes.extend(request.density.across_y.to_bits().to_le_bytes());
    bytes.push(u8::from(request.density.aspect_locked));
    bytes.extend(request.rotation_degrees.to_bits().to_le_bytes());
    bytes.extend(request.translation_x.to_bits().to_le_bytes());
    bytes.extend(request.translation_y.to_bits().to_le_bytes());
    bytes.extend(request.guard_steps.to_le_bytes());
    bytes.extend(request.support_radius.to_bits().to_le_bytes());
    bytes.extend(
        u64::try_from(request.max_family_candidates)
            .expect("usize fits u64")
            .to_le_bytes(),
    );
    fnv1a64(bytes)
}

/// Ordered scalar-field output realization through the declared typed layer.
/// The returned mark geometry is canonical; clipping remains exclusively with
/// the final renderer consumer.
pub fn realize_typed_mapped_outputs(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: MarkResponse,
) -> Result<TypedRealization<MappedCircularMarkRealization>, PatternPipelineError> {
    let provenance = realization_provenance(family, plan)?;
    let compatibility = legacy_grid_sites_for_circular_marks(family)?;
    realize_mapped_circular_marks(&compatibility, source, canvas, mapping, response)
        .map(|mut output| {
            output.realization_fingerprint =
                orientation_identity(&output.realization_fingerprint, family, &provenance);
            TypedRealization { provenance, output }
        })
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))
}

/// Ordered sampled-paint output realization through the declared typed layer.
pub fn realize_typed_source_color_outputs(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: MarkResponse,
) -> Result<TypedRealization<SourceColorCircularMarkRealization>, PatternPipelineError> {
    let provenance = realization_provenance(family, plan)?;
    let compatibility = legacy_grid_sites_for_circular_marks(family)?;
    realize_source_color_circular_marks(&compatibility, source, canvas, mapping, response)
        .map(|mut output| {
            output.realization_fingerprint =
                orientation_identity(&output.realization_fingerprint, family, &provenance);
            TypedRealization { provenance, output }
        })
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))
}

/// Retained diagnostic realization, now routed through the same typed output
/// capability boundary as authoritative document evaluation.
pub fn realize_typed_diagnostic_outputs(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    source: &SourceField,
    canvas: &CanvasSpec,
    placement: SourcePlacement,
    component: SourceComponent,
    response: MarkResponse,
) -> Result<TypedRealization<CircularMarkRealization>, PatternPipelineError> {
    let provenance = realization_provenance(family, plan)?;
    let compatibility = legacy_grid_sites_for_circular_marks(family)?;
    realize_circular_marks(
        &compatibility,
        source,
        canvas,
        placement,
        component,
        response,
    )
    .map(|mut output| {
        output.realization_fingerprint =
            orientation_identity(&output.realization_fingerprint, family, &provenance);
        TypedRealization { provenance, output }
    })
    .map_err(|error| PatternPipelineError::new(error.path(), error.message()))
}

/// Immutable request inputs for one generalized canonical mark realization.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanonicalMarkRequest {
    /// Complete source mapping that determines canonical response and solid-mark size.
    pub mapping: SourceMapping,
    /// Selects sampled-source paint while retaining `mapping` as the sole mapping authority.
    pub sampled_paint: bool,
    /// Defines normalized diameter and the channel rotation applied after requested orientation.
    pub response: MarkResponse,
    /// Bounds checked site-by-authored-segment expansion before canonical-mark allocation.
    pub max_transformed_curve_segment_instances: usize,
}

/// Realizes one ordered typed output layer into truthful canonical circle or closed-path marks.
///
/// The document resolves the closed-shape resource before this function runs; family sites,
/// source response, and final canonical geometry remain immutable inputs. The current
/// single-layer contract rejects unsupported plans before a partial output can escape.
///
/// # Errors
///
/// Returns stable provenance, resource, response, normalization, orientation, or work-limit
/// diagnostics without returning partial marks.
pub fn realize_typed_canonical_marks(
    document: &Document,
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    source: &SourceField,
    canvas: &CanvasSpec,
    request: CanonicalMarkRequest,
) -> Result<TypedRealization<CanonicalMarkRealization>, PatternPipelineError> {
    realize_typed_canonical_marks_cancellable(
        document,
        family,
        plan,
        source,
        canvas,
        request,
        &|| false,
    )
}

/// Realizes generalized canonical marks while polling the caller-owned cancellation probe.
///
/// # Errors
///
/// Returns cancellation or any stable canonical realization diagnostic without partial output.
pub fn realize_typed_canonical_marks_cancellable(
    document: &Document,
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    source: &SourceField,
    canvas: &CanvasSpec,
    request: CanonicalMarkRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedRealization<CanonicalMarkRealization>, PatternPipelineError> {
    let provenance = realization_provenance(family, plan)?;
    let [output] = plan.ordered_outputs.as_slice() else {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.capability",
            "canonical mark realization requires exactly one ordered output layer",
        ));
    };
    let (prototype, orientation) = output.marks().ok_or(PatternPipelineError::new(
        "pattern.output_layers.capability",
        "canonical mark realization requires a mark output",
    ))?;
    let prototype_path = match prototype {
        MarkPrototype::Circle => None,
        MarkPrototype::AuthoredClosedShape { structure_id } => {
            let structure =
                document
                    .authored_structure(*structure_id)
                    .ok_or(PatternPipelineError::new(
                        "pattern.output_layers.prototype.reference",
                        "authored closed-shape mark resource is missing",
                    ))?;
            let path = CurvePath::from_authored_structure(structure).map_err(|_| {
                PatternPipelineError::new(
                    "pattern.output_layers.prototype.geometry",
                    "authored closed-shape mark resource is not finite closed geometry",
                )
            })?;
            if path.closure() != PathClosure::Closed {
                return Err(PatternPipelineError::new(
                    "pattern.output_layers.prototype.kind",
                    "authored mark resource must be a closed shape",
                ));
            }
            let bounds = path.bounds().map_err(|_| {
                PatternPipelineError::new(
                    "pattern.output_layers.prototype.bounds",
                    "authored closed-shape bounds must remain finite",
                )
            })?;
            let anchor = Point2::new(
                (bounds.min.x + bounds.max.x) / 2.0,
                (bounds.min.y + bounds.max.y) / 2.0,
            );
            let radius = path_reference_radius(&path, anchor, is_cancelled)?;
            Some((path, anchor, radius, *structure_id))
        }
    };
    if let Some((path, _, _, _)) = &prototype_path {
        preflight_transformed_curve_segment_instances(
            family.site_set().len(),
            path.segments().len(),
            request.max_transformed_curve_segment_instances,
        )?;
    }
    let orientations = family
        .site_set()
        .iter()
        .map(|site| {
            if is_cancelled() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "realization was cancelled",
                ));
            }
            site_orientation_degrees(
                family,
                site,
                orientation,
                request.response.rotation_offset_degrees,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut marks = Vec::with_capacity(family.site_set().len());
    let mut paints = request
        .sampled_paint
        .then(|| Vec::with_capacity(family.site_set().len()));
    for (site, orientation) in family.site_set().iter().zip(orientations) {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "realization was cancelled",
            ));
        }
        let (ink, sampled_paint) = if request.sampled_paint {
            let sample = source
                .sample_source_color(site.position, canvas, request.mapping)
                .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
            (
                sample.response,
                Some(match sample.paint {
                    Some(paint) => (paint, false),
                    None => (
                        SampledSourcePaint {
                            red: 0.0,
                            green: 0.0,
                            blue: 0.0,
                            alpha: 1.0,
                        },
                        true,
                    ),
                }),
            )
        } else {
            (
                source
                    .sample_mapping_response(site.position, canvas, request.mapping)
                    .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?,
                None,
            )
        };
        let radius = if sampled_paint.is_some_and(|(_, suppressed)| suppressed) {
            0.0
        } else {
            radius_from_ink_with_diameter(ink, request.response, site.nominal_cell_basis.diameter())
                .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?
        };
        if radius > family.planned_support_radius() {
            return Err(PatternPipelineError::new(
                "realization.family.support_radius",
                "realized mark radius exceeds the planned family support",
            ));
        }
        let mark = match &prototype_path {
            None => CanonicalMark::Circle {
                source_site_id: site.id,
                center: site.position,
                radius,
                scope: site.scope,
                provenance: site.provenance.clone(),
                fill_rule: CanonicalFillRule::EvenOdd,
            },
            Some((path, anchor, reference_radius, _)) => {
                let scale = radius / reference_radius;
                let transformed = transform_closed_shape(
                    path,
                    *anchor,
                    scale,
                    orientation,
                    site.position,
                    is_cancelled,
                )?;
                CanonicalMark::ClosedPath(
                    CanonicalPathMark::new(
                        site.id,
                        transformed,
                        site.scope,
                        site.provenance.clone(),
                        CanonicalFillRule::EvenOdd,
                    )
                    .map_err(|_| {
                        PatternPipelineError::new(
                            "realization.mark",
                            "transformed authored closed shape must remain finite and closed",
                        )
                    })?,
                )
            }
        };
        marks.push(mark);
        if let Some((paint, _)) = sampled_paint {
            paints
                .as_mut()
                .expect("sampled paint mode initialized paint storage")
                .push(paint);
        }
    }
    let mut bytes = family.family_fingerprint().as_bytes().to_vec();
    bytes.extend(b"|stage-20e2-canonical-mark-v3");
    append_output_capability_identity(&mut bytes, output);
    match &prototype_path {
        None => bytes.push(0),
        Some((path, anchor, reference_radius, _)) => {
            bytes.push(1);
            append_curve_path_identity(&mut bytes, path);
            bytes.extend(anchor.x.to_bits().to_le_bytes());
            bytes.extend(anchor.y.to_bits().to_le_bytes());
            bytes.extend(reference_radius.to_bits().to_le_bytes());
            append_fill_rule_identity(&mut bytes, CanonicalFillRule::EvenOdd);
        }
    }
    append_source_identity(&mut bytes, source);
    append_source_mapping_identity(&mut bytes, request.mapping);
    bytes.push(u8::from(request.sampled_paint));
    bytes.extend(request.response.minimum_fill.to_bits().to_le_bytes());
    bytes.extend(request.response.maximum_fill.to_bits().to_le_bytes());
    bytes.extend(
        request
            .response
            .rotation_offset_degrees
            .to_bits()
            .to_le_bytes(),
    );
    for mark in &marks {
        append_canonical_mark_identity(&mut bytes, mark);
    }
    let realization = CanonicalMarkRealization {
        family_fingerprint: family.family_fingerprint().to_owned(),
        realization_fingerprint: fnv1a64(bytes),
        source_identity: source.identity().clone(),
        response: request.response,
        marks,
        paints,
    };
    Ok(TypedRealization {
        provenance,
        output: realization,
    })
}

/// Preflights the exact site-by-segment expansion without allocating any canonical paths.
///
/// # Errors
///
/// Returns the stable transformed-segment limit diagnostic for a zero limit, multiplication
/// overflow, or a product above the inclusive configured bound.
fn preflight_transformed_curve_segment_instances(
    site_count: usize,
    segment_count: usize,
    limit: usize,
) -> Result<usize, PatternPipelineError> {
    if limit == 0 {
        return Err(PatternPipelineError::new(
            "realization.mark.segment_limit",
            "transformed curve-segment instance limit exceeded",
        ));
    }
    let instances = site_count
        .checked_mul(segment_count)
        .ok_or(PatternPipelineError::new(
            "realization.mark.segment_limit",
            "transformed curve-segment instance count overflows",
        ))?;
    if instances > limit {
        return Err(PatternPipelineError::new(
            "realization.mark.segment_limit",
            "transformed curve-segment instance limit exceeded",
        ));
    }
    Ok(instances)
}

/// Computes the conservative exact-control-point reference radius about one bounds-center anchor.
///
/// # Errors
///
/// Returns cancellation or the stable finite, nonzero-radius diagnostic without publishing work.
fn path_reference_radius(
    path: &CurvePath,
    anchor: Point2,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<f64, PatternPipelineError> {
    let mut maximum = 0.0_f64;
    for segment in path.segments() {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "realization was cancelled",
            ));
        }
        let points = match segment {
            CurveSegment::Line(line) => vec![line.start(), line.end()],
            CurveSegment::CubicBezier(cubic) => vec![
                cubic.start(),
                cubic.control_1(),
                cubic.control_2(),
                cubic.end(),
            ],
        };
        for point in points {
            let radius = ((point.x - anchor.x).powi(2) + (point.y - anchor.y).powi(2)).sqrt();
            if !radius.is_finite() {
                return Err(PatternPipelineError::new(
                    "pattern.output_layers.prototype.radius",
                    "authored closed-shape reference radius must remain finite",
                ));
            }
            maximum = maximum.max(radius);
        }
    }
    (maximum > 0.0)
        .then_some(maximum)
        .ok_or(PatternPipelineError::new(
            "pattern.output_layers.prototype.radius",
            "authored closed-shape prototype requires a nonzero reference radius",
        ))
}

/// Scales a closed path about its canonical anchor and translates that anchor to one family site.
///
/// # Errors
///
/// Returns cancellation or stable non-finite/closure transform diagnostics without a partial path.
fn transform_closed_shape(
    path: &CurvePath,
    anchor: Point2,
    scale: f64,
    orientation_degrees: f64,
    destination: Point2,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CurvePath, PatternPipelineError> {
    if !scale.is_finite()
        || scale < 0.0
        || !orientation_degrees.is_finite()
        || !destination.is_finite()
    {
        return Err(PatternPipelineError::new(
            "realization.mark.transform",
            "closed-shape scale and destination must be finite",
        ));
    }
    let radians = orientation_degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    let map = |point: Point2| {
        let x = (point.x - anchor.x) * scale;
        let y = (point.y - anchor.y) * scale;
        Point2::new(
            destination.x + cos * x - sin * y,
            destination.y + sin * x + cos * y,
        )
    };
    let mut segments = Vec::with_capacity(path.segments().len());
    for segment in path.segments() {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "realization was cancelled",
            ));
        }
        let transformed = match segment {
            CurveSegment::Line(line) => {
                toniator_geometry::LineSegment::new(map(line.start()), map(line.end()))
                    .map(CurveSegment::Line)
            }
            CurveSegment::CubicBezier(cubic) => toniator_geometry::CubicBezierSegment::new(
                map(cubic.start()),
                map(cubic.control_1()),
                map(cubic.control_2()),
                map(cubic.end()),
            )
            .map(CurveSegment::CubicBezier),
        }
        .map_err(|_| {
            PatternPipelineError::new(
                "realization.mark.transform",
                "transformed closed-shape coordinates must remain finite",
            )
        })?;
        segments.push(transformed);
    }
    CurvePath::new(segments, PathClosure::Closed).map_err(|_| {
        PatternPipelineError::new(
            "realization.mark.transform",
            "transformed closed-shape path must retain exact closure",
        )
    })
}

/// Resolves a requested fixed/tangent/normal orientation from the exact site contributor.
///
/// # Errors
///
/// Returns stable missing-contributor, guide, location, or tangent diagnostics.
fn site_orientation_degrees(
    family: &TypedFamilyOutput,
    site: &FamilySite,
    orientation: &MarkOrientation,
    rotation_offset_degrees: f64,
) -> Result<f64, PatternPipelineError> {
    let vector = match orientation {
        MarkOrientation::Fixed => return Ok(rotation_offset_degrees),
        MarkOrientation::GuideTangent { dimension_id }
        | MarkOrientation::GuideNormal { dimension_id } => {
            let requested = dimension_id.0;
            let path_id = match &site.provenance {
                FamilySiteProvenance::GuideIntersection { contributors } => contributors
                    .iter()
                    .find(|id| id.dimension_id == requested)
                    .map(|id| {
                        StructuralPathInstanceId::guide_dimension(
                            GuideDimensionId(id.dimension_id),
                            id.index,
                            id.component_ordinal,
                        )
                    }),
                FamilySiteProvenance::AlongGuide { guide_id, .. } => {
                    (guide_id.dimension_id == requested).then_some(
                        StructuralPathInstanceId::guide_dimension(
                            GuideDimensionId(guide_id.dimension_id),
                            guide_id.index,
                            guide_id.component_ordinal,
                        ),
                    )
                }
                FamilySiteProvenance::CurveGuideIntersection { contributors } => contributors
                    .iter()
                    .find(|entry| {
                        matches!(entry.path.source, StructuralPathSourceId::GuideDimension(id) if id.0 == requested)
                    })
                    .map(|entry| entry.path),
                FamilySiteProvenance::CurveAlongGuide { location, .. } => {
                    matches!(location.path.source, StructuralPathSourceId::GuideDimension(id) if id.0 == requested)
                        .then_some(location.path)
                }
                FamilySiteProvenance::AlongParametricCurve { .. } => None,
                FamilySiteProvenance::Random { .. } => None,
            }
            .ok_or(PatternPipelineError::new(
                "realization.orientation.contributor",
                "requested orientation dimension is absent from this site provenance",
            ))?;
            if let Some(guide_id) = path_id.guide_instance() {
                if let Some(guide) = family
                    .straight_guides()
                    .iter()
                    .find(|guide| guide.id == guide_id)
                {
                    guide.tangent
                } else {
                    let location = match &site.provenance {
                        FamilySiteProvenance::CurveGuideIntersection { contributors } => {
                            contributors.iter().find(|entry| entry.path == path_id)
                        }
                        FamilySiteProvenance::CurveAlongGuide { location, .. }
                            if location.path == path_id =>
                        {
                            Some(location)
                        }
                        _ => None,
                    }
                    .ok_or(PatternPipelineError::new(
                        "realization.orientation.guide",
                        "site contributor lacks retained guide tangent location",
                    ))?;
                    let guide = family
                        .structural_path_set()
                        .and_then(|set| set.paths().iter().find(|guide| guide.id == path_id))
                        .ok_or(PatternPipelineError::new(
                            "realization.orientation.guide",
                            "site contributor lacks retained curve guide",
                        ))?;
                    guide
                        .path
                        .unit_tangent_at(
                            PathLocation::new(
                                location.segment_index,
                                f64::from_bits(location.parameter_bits),
                            )
                            .map_err(|_| {
                                PatternPipelineError::new(
                                    "realization.orientation.location",
                                    "curve contributor has invalid location",
                                )
                            })?,
                        )
                        .map_err(|_| {
                            PatternPipelineError::new(
                                "realization.orientation.tangent",
                                "curve contributor has no finite tangent",
                            )
                        })?
                }
            } else {
                return Err(PatternPipelineError::new(
                    "realization.orientation.contributor",
                    "guide orientation cannot address a parametric structural path",
                ));
            }
        }
    };
    let degrees = vector.y.atan2(vector.x).to_degrees();
    let normal = matches!(orientation, MarkOrientation::GuideNormal { .. })
        .then_some(90.0)
        .unwrap_or(0.0);
    Ok(degrees + normal + rotation_offset_degrees)
}

/// Extends retained realization identity with ordered prototype and orientation capability data.
fn orientation_identity(
    legacy_identity: &str,
    family: &TypedFamilyOutput,
    provenance: &TypedRealizationProvenance,
) -> String {
    if family.family().product == StructuralProductCapability::GuideIntersections
        && family.family().dimensions.as_slice()
            == [
                StraightGuideDimension {
                    id: FIRST_DIMENSION_ID,
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    repetition: toniator_domain::StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: SECOND_DIMENSION_ID,
                    baseline_angle_degrees: 90.0,
                    phase: 0.0,
                    repetition: toniator_domain::StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
            ]
    {
        return legacy_identity.to_owned();
    }
    let mut bytes = legacy_identity.as_bytes().to_vec();
    bytes.extend(b"toniator-stage-16a-realization-contract-v2");
    // The accepted modulation type is intentionally unit-like today.  It is
    // nevertheless tagged here so a future typed variant cannot silently
    // share a realization identity with the no-modulation contract.
    match &provenance.modulation {
        PatternModulation => bytes.push(1),
    }
    for ((layer_id, prototype), orientation) in provenance
        .ordered_output_layer_ids
        .iter()
        .zip(&provenance.ordered_output_prototypes)
        .zip(&provenance.ordered_output_orientations)
    {
        bytes.extend(layer_id.0.to_le_bytes());
        match prototype {
            MarkPrototype::Circle => bytes.push(1),
            MarkPrototype::AuthoredClosedShape { structure_id } => {
                bytes.push(2);
                bytes.extend(structure_id.0.to_le_bytes());
            }
        }
        match orientation {
            MarkOrientation::Fixed => bytes.push(1),
            MarkOrientation::GuideTangent { dimension_id } => {
                bytes.push(2);
                bytes.extend(dimension_id.0.to_le_bytes());
            }
            MarkOrientation::GuideNormal { dimension_id } => {
                bytes.push(3);
                bytes.extend(dimension_id.0.to_le_bytes());
            }
        }
    }
    fnv1a64(bytes)
}

/// Binds a compatible typed plan to truthful family sites before current realization.
///
/// # Errors
///
/// Returns stable family or ordered-output provenance mismatch diagnostics.
fn realization_provenance(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
) -> Result<TypedRealizationProvenance, PatternPipelineError> {
    if family.family() != &plan.family {
        return Err(PatternPipelineError::new(
            "pattern.family.provenance",
            "realization plan does not match the structural family product",
        ));
    }
    match plan.ordered_outputs.as_slice() {
        [output @ OutputCapability { consumes, .. }] if *consumes == family.family().product => {
            let structural_input = if output.marks().is_some() {
                RealizationStructuralInput::Sites(family.site_set().clone())
            } else if output.guide_paths().is_some() {
                let paths = family
                    .structural_path_set()
                    .ok_or(PatternPipelineError::new(
                        "pattern.output_layers.guide_paths",
                        "guide-path output requires ordered raw guide paths",
                    ))?;
                RealizationStructuralInput::StructuralPaths {
                    paths: paths.clone(),
                    nominal_bases: family.structure.guide_nominal_bases.clone(),
                }
            } else {
                return Err(PatternPipelineError::new(
                    "pattern.output_layers.capability",
                    "ordered output has no typed structural input",
                ));
            };
            Ok(TypedRealizationProvenance {
                structural: family.family().provenance.clone(),
                modulation: plan.modulation.clone(),
                ordered_output_layer_ids: plan
                    .ordered_outputs
                    .iter()
                    .map(|output| output.layer_id)
                    .collect(),
                ordered_output_prototypes: plan
                    .ordered_outputs
                    .iter()
                    .filter_map(|output| output.marks().map(|(prototype, _)| prototype.clone()))
                    .collect(),
                ordered_output_orientations: plan
                    .ordered_outputs
                    .iter()
                    .filter_map(|output| output.marks().map(|(_, orientation)| orientation.clone()))
                    .collect(),
                structural_input,
            })
        }
        _ => Err(PatternPipelineError::new(
            "pattern.output_layers.capability",
            "ordered output realization has no compatible structural product",
        )),
    }
}

/// Retains pre-Stage-20E2 circle realization only for genuine guide-originated sites.
///
/// Parametric sites must use `realize_typed_canonical_marks`, which retains their
/// path-neutral `FamilySiteId` and `AlongParametricCurve` provenance without
/// synthesizing a `GuideInstanceId`.
///
/// # Errors
///
/// Rejects parametric site provenance because the retained circle representation
/// has guide-only identity fields and cannot truthfully represent it.
fn legacy_grid_sites_for_circular_marks(
    family: &TypedFamilyOutput,
) -> Result<GridFamilyOutput, PatternPipelineError> {
    if family.site_set().iter().any(|site| {
        matches!(
            site.provenance,
            FamilySiteProvenance::AlongParametricCurve { .. }
        )
    }) {
        return Err(PatternPipelineError::new(
            "realization.legacy_circle.parametric_provenance",
            "parametric sites require the path-neutral canonical mark realization",
        ));
    }
    let sites = family
        .site_set()
        .iter()
        .map(|site| {
            let (id, contributors) = match &site.provenance {
                FamilySiteProvenance::GuideIntersection { contributors } => {
                    let first = contributors[0];
                    let second = contributors[1];
                    (
                        SiteId {
                            first_dimension_id: first.dimension_id,
                            first_index: first.index,
                            second_dimension_id: second.dimension_id,
                            second_index: second.index,
                        },
                        contributors.clone(),
                    )
                }
                FamilySiteProvenance::AlongGuide {
                    guide_id, sequence, ..
                } => (
                    SiteId {
                        first_dimension_id: guide_id.dimension_id,
                        first_index: guide_id.index,
                        second_dimension_id: guide_id.dimension_id,
                        second_index: *sequence,
                    },
                    vec![*guide_id],
                ),
                FamilySiteProvenance::CurveGuideIntersection { contributors } => {
                    let first = contributors[0]
                        .path
                        .guide_instance()
                        .expect("curve-guide provenance has a guide source");
                    let second = contributors[1]
                        .path
                        .guide_instance()
                        .expect("curve-guide provenance has a guide source");
                    (
                        SiteId {
                            first_dimension_id: first.dimension_id,
                            first_index: first.index,
                            second_dimension_id: second.dimension_id,
                            second_index: second.index,
                        },
                        contributors
                            .iter()
                            .map(|location| {
                                location
                                    .path
                                    .guide_instance()
                                    .expect("curve-guide provenance has a guide source")
                            })
                            .collect(),
                    )
                }
                FamilySiteProvenance::CurveAlongGuide {
                    location, sequence, ..
                } => (
                    SiteId {
                        first_dimension_id: location
                            .path
                            .guide_instance()
                            .expect("curve-guide provenance has a guide source")
                            .dimension_id,
                        first_index: location
                            .path
                            .guide_instance()
                            .expect("curve-guide provenance has a guide source")
                            .index,
                        second_dimension_id: location
                            .path
                            .guide_instance()
                            .expect("curve-guide provenance has a guide source")
                            .dimension_id,
                        second_index: *sequence,
                    },
                    vec![
                        location
                            .path
                            .guide_instance()
                            .expect("curve-guide provenance has a guide source"),
                    ],
                ),
                FamilySiteProvenance::AlongParametricCurve { .. } => {
                    unreachable!("parametric sites were rejected before the legacy circle seam")
                }
                FamilySiteProvenance::Random {
                    accepted_ordinal, ..
                } => {
                    let random = family
                        .family()
                        .random
                        .as_ref()
                        .expect("random site provenance requires a random family capability");
                    let process = family.family().provenance.mechanism_ids[0];
                    let product = family.site_set().product_mechanism_id();
                    let accepted = i64::try_from(*accepted_ordinal)
                        .expect("accepted random-site ordinal fits i64");
                    let seed = i64::from(random.seed);
                    (
                        SiteId {
                            first_dimension_id: product.0,
                            first_index: accepted,
                            second_dimension_id: process.0,
                            second_index: seed,
                        },
                        vec![
                            GuideInstanceId {
                                dimension_id: process.0,
                                index: seed,
                                component_ordinal: 0,
                            },
                            GuideInstanceId {
                                dimension_id: product.0,
                                index: accepted,
                                component_ordinal: 0,
                            },
                        ],
                    )
                }
            };
            Ok(IntersectionSite {
                id,
                position: site.position,
                nominal_cell_diameter: site.nominal_cell_basis.diameter(),
                scope: site.scope,
                provenance: GuideIntersectionProvenance { contributors },
            })
        })
        .collect::<Result<Vec<_>, PatternPipelineError>>()?;
    Ok(GridFamilyOutput {
        family_fingerprint: family.family_fingerprint().to_owned(),
        guard_steps: family.structure.guard_steps,
        support_radius: family.structure.support_radius,
        antialias_margin: family.structure.antialias_margin,
        generation_domain: family.structure.generation_domain,
        coverage: family.structure.coverage.clone(),
        guides: family.structure.guides.clone(),
        sites,
    })
}

/// Headless input to the two-dimension straight-grid family.
#[derive(Clone, Debug, PartialEq)]
pub struct GridInspectRequest {
    pub canvas: CanvasSpec,
    pub density: DensityMetric2D,
    pub rotation_degrees: f64,
    /// Authored document-axis translation; it is never replaced by phase.
    pub translation_x: f64,
    /// Authored document-axis translation; it is never replaced by phase.
    pub translation_y: f64,
    pub guard_steps: u32,
    pub support_radius: f64,
    /// Caller-owned upper bound for the conservative guide-range Cartesian
    /// product, checked before allocating guides or sites.
    pub max_family_candidates: usize,
}

/// A coverage result for one stable guide dimension.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct GuideCoverage {
    pub dimension_id: u64,
    pub spacing: f64,
    /// The phase is normalized only for reporting; authored translation remains input state.
    pub normalized_phase: f64,
    pub first_index: i64,
    pub last_index: i64,
}

/// Deterministic, off-canvas family output before any realization or clipping.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GridFamilyOutput {
    pub family_fingerprint: String,
    pub guard_steps: u32,
    pub support_radius: f64,
    pub antialias_margin: f64,
    pub generation_domain: Bounds,
    pub coverage: Vec<GuideCoverage>,
    pub guides: Vec<StraightGuide>,
    pub sites: Vec<IntersectionSite>,
}

/// Generic Stage 16A structural planning input.  It uses the existing shared
/// channel density and transform while the definition supplies independent
/// dimension angle/phase/repetition state.
#[derive(Clone, Debug, PartialEq)]
pub struct StraightGuideInspectRequest {
    pub canvas: CanvasSpec,
    pub density: DensityMetric2D,
    pub rotation_degrees: f64,
    pub translation_x: f64,
    pub translation_y: f64,
    pub guard_steps: u32,
    pub support_radius: f64,
    pub max_family_candidates: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum GeneralizedSiteProvenance {
    Intersection {
        contributors: Vec<GuideInstanceId>,
    },
    AlongGuide {
        guide_id: GuideInstanceId,
        guide_order: usize,
        sequence: i64,
        absolute_arc_position_bits: u64,
        local_arc_position_bits: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GeneralizedSite {
    pub sequence: usize,
    pub position: Point2,
    pub scope: SiteScope,
    pub provenance: GeneralizedSiteProvenance,
}

/// Structural products are kept separate from canonical realization.  Output
/// layers address exactly one declared product and never reconstruct guides.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GeneralizedStraightGuideOutput {
    pub family_fingerprint: String,
    pub coverage: Vec<GuideCoverage>,
    pub guides: Vec<StraightGuide>,
    pub sites: Vec<GeneralizedSite>,
}

/// Evaluate a selected generalized product with bounded cancellation points.
/// It is intentionally independent of source/modulation/output presentation.
pub fn evaluate_generalized_straight_guides_cancellable(
    family: &FamilyCapability,
    request: &StraightGuideInspectRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<GeneralizedStraightGuideOutput, GridError> {
    if is_cancelled() {
        return Err(GridError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ));
    }
    validate_straight_request(request)?;
    if family.dimensions.is_empty() || family.dimensions.len() > 4 {
        return Err(GridError::new(
            "pattern.family.dimensions",
            "straight-guide family requires one through four dimensions",
        ));
    }
    let transform = AffineTransform2D::rotate_about_then_translate(
        Point2::new(request.canvas.width / 2.0, request.canvas.height / 2.0),
        request.rotation_degrees,
        Vector2::new(request.translation_x, request.translation_y),
    )
    .ok_or(GridError::new(
        "channel.pattern.layout",
        "transform is not finite",
    ))?;
    let canvas = Bounds::new(
        Point2::new(0.0, 0.0),
        Point2::new(request.canvas.width, request.canvas.height),
    )
    .expect("validated canvas bounds");
    let maximum_spacing = family
        .dimensions
        .iter()
        .try_fold(0.0_f64, |maximum, dimension| {
            let radians = dimension.baseline_angle_degrees.to_radians();
            let normal = Vector2::new(radians.cos(), radians.sin());
            let value = directional_spacing(&request.canvas, &request.density, normal)?
                * dimension.repetition.spacing_multiplier;
            (value.is_finite() && value > 0.0)
                .then_some(maximum.max(value))
                .ok_or(GridError::new(
                    "pattern.family.dimensions.repetition",
                    "spacing multiplier must resolve to a positive finite interval",
                ))
        })?;
    let margin = request.support_radius
        + ANTIALIAS_MARGIN
        + f64::from(request.guard_steps) * maximum_spacing;
    let domain = transform
        .inverse_bounds(canvas.expanded(margin).expect("finite margin"))
        .ok_or(GridError::new(
            "coverage",
            "inverse transform produced non-finite bounds",
        ))?;
    let mut coverage = Vec::with_capacity(family.dimensions.len());
    let mut plans = Vec::with_capacity(family.dimensions.len());
    let mut guide_total = 0_usize;
    for dimension in &family.dimensions {
        if is_cancelled() {
            return Err(GridError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        if !dimension.baseline_angle_degrees.is_finite()
            || !dimension.phase.is_finite()
            || dimension.id.0 == 0
        {
            return Err(GridError::new(
                "pattern.family.dimensions",
                "dimension values must be finite with nonzero stable IDs",
            ));
        }
        let radians = dimension.baseline_angle_degrees.to_radians();
        let normal = Vector2::new(radians.cos(), radians.sin());
        let spacing = directional_spacing(&request.canvas, &request.density, normal)?
            * dimension.repetition.spacing_multiplier;
        let plan = DimensionPlan::new(dimension.id, normal, spacing, dimension.phase);
        let item = plan.coverage(
            domain,
            transform,
            &GridInspectRequest {
                canvas: request.canvas.clone(),
                density: request.density.clone(),
                rotation_degrees: request.rotation_degrees,
                translation_x: request.translation_x,
                translation_y: request.translation_y,
                guard_steps: request.guard_steps,
                support_radius: request.support_radius,
                max_family_candidates: request.max_family_candidates,
            },
        )?;
        guide_total = guide_total
            .checked_add(guide_range_count(item)?)
            .ok_or(GridError::new(
                "coverage.candidate_limit",
                "guide-range candidate count overflowed",
            ))?;
        coverage.push(item);
        plans.push(plan);
    }
    if guide_total > request.max_family_candidates {
        return Err(GridError::new(
            "coverage.candidate_limit",
            "guide-range candidate count exceeds the configured limit",
        ));
    }
    let selected: Vec<usize> = family
        .site_selection
        .iter()
        .map(|id| {
            family
                .dimensions
                .iter()
                .position(|dimension| dimension.id == *id)
                .ok_or(GridError::new(
                    "pattern.family.selection",
                    "selection references a missing dimension ID",
                ))
        })
        .collect::<Result<_, _>>()?;
    if family.product == StructuralProductCapability::GuideIntersections {
        let mut selected_pair_count = 0_usize;
        let mut selected_pair_candidates = 0_usize;
        for (offset, &left) in selected.iter().enumerate() {
            for &right in &selected[offset + 1..] {
                if is_cancelled() {
                    return Err(GridError::new(
                        "evaluation.cancelled",
                        "evaluation was cancelled",
                    ));
                }
                selected_pair_count = selected_pair_count.checked_add(1).ok_or(GridError::new(
                    "coverage.intersections.selected_pairs",
                    "selected dimension-pair count overflowed",
                ))?;
                let candidates = guide_range_count(coverage[left])?
                    .checked_mul(guide_range_count(coverage[right])?)
                    .ok_or(GridError::new(
                        "coverage.intersections.pairwise_limit",
                        "pairwise intersection count overflowed",
                    ))?;
                selected_pair_candidates =
                    selected_pair_candidates
                        .checked_add(candidates)
                        .ok_or(GridError::new(
                            "coverage.intersections.selected_pairs",
                            "selected pair count overflowed",
                        ))?;
            }
        }
        if selected_pair_count > request.max_family_candidates {
            return Err(GridError::new(
                "coverage.intersections.selected_pairs",
                "selected dimension-pair count exceeds the configured limit",
            ));
        }
        if selected_pair_candidates > request.max_family_candidates {
            return Err(GridError::new(
                "coverage.intersections.pairwise_limit",
                "pairwise intersection count exceeds the configured limit",
            ));
        }
        // Sorting and adjacent merging have separately bounded work.  Account
        // for both the raw records and the merge pass before any guide/site
        // allocation so a configured work limit is never discovered late.
        let merge_work = selected_pair_candidates
            .checked_mul(2)
            .ok_or(GridError::new(
                "coverage.intersections.merge_limit",
                "coincident merge work overflowed",
            ))?;
        if merge_work > request.max_family_candidates {
            return Err(GridError::new(
                "coverage.intersections.merge_limit",
                "coincident merge work exceeds the configured limit",
            ));
        }
    }
    let extension = margin;
    let mut guides = Vec::with_capacity(guide_total);
    let mut grouped = Vec::with_capacity(plans.len());
    for (plan, item) in plans.iter().zip(&coverage) {
        if is_cancelled() {
            return Err(GridError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        let range = plan.guides(*item, domain, transform, extension);
        grouped.push(range.clone());
        guides.extend(range);
    }
    let sites = match family.product {
        StructuralProductCapability::GuideIntersections => generalized_intersections(
            &grouped,
            &selected,
            family.merge_epsilon.unwrap_or(0.0),
            canvas,
            margin,
            request.max_family_candidates,
            is_cancelled,
        )?,
        StructuralProductCapability::AlongGuideSites => generalized_along_guides(
            &grouped,
            &selected,
            selected.iter().try_fold(0.0_f64, |sum, &index| {
                let next = sum + plans[index].spacing;
                next.is_finite().then_some(next).ok_or(GridError::new(
                    "coverage.along_guides.interval",
                    "selected guide interval overflowed",
                ))
            })? / selected.len() as f64
                * family.along_interval_multiplier.unwrap_or(1.0),
            family.along_phase.unwrap_or(0.0),
            canvas,
            margin,
            request.max_family_candidates,
            is_cancelled,
        )?,
        StructuralProductCapability::RandomSites => {
            return Err(GridError::new(
                "pattern.family.random_sites",
                "random-site products are evaluated by the random-site family evaluator",
            ));
        }
        StructuralProductCapability::ParametricPaths => {
            return Err(GridError::new(
                "pattern.family.parametric",
                "parametric paths are evaluated through their finite CurvePath adapter",
            ));
        }
    };
    Ok(GeneralizedStraightGuideOutput {
        family_fingerprint: generalized_fingerprint(family, request),
        coverage,
        guides,
        sites,
    })
}

fn generalized_intersections(
    grouped: &[Vec<StraightGuide>],
    selected: &[usize],
    epsilon: f64,
    canvas: Bounds,
    margin: f64,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<GeneralizedSite>, GridError> {
    if selected.len() < 2 {
        return Err(GridError::new(
            "pattern.family.intersections.dimensions",
            "intersection selection requires at least two dimensions",
        ));
    }
    if !epsilon.is_finite() || epsilon < 0.0 {
        return Err(GridError::new(
            "pattern.family.intersections.merge_epsilon",
            "merge epsilon must be finite and nonnegative",
        ));
    }
    let mut raw: Vec<(Point2, Vec<GuideInstanceId>)> = Vec::new();
    for (left_offset, &left) in selected.iter().enumerate() {
        for &right in &selected[left_offset + 1..] {
            if cancelled() {
                return Err(GridError::new(
                    "evaluation.cancelled",
                    "evaluation was cancelled",
                ));
            }
            let representative_left = grouped[left].first().ok_or(GridError::new(
                "coverage",
                "selected dimension produced no guides",
            ))?;
            let representative_right = grouped[right].first().ok_or(GridError::new(
                "coverage",
                "selected dimension produced no guides",
            ))?;
            let normal_cross = representative_left.normal.x.mul_add(
                representative_right.normal.y,
                -representative_left.normal.y * representative_right.normal.x,
            );
            if normal_cross.abs() <= 1e-10 {
                continue;
            }
            let pairs = grouped[left]
                .len()
                .checked_mul(grouped[right].len())
                .ok_or(GridError::new(
                    "coverage.candidate_limit",
                    "pairwise-intersection count overflowed",
                ))?;
            if raw
                .len()
                .checked_add(pairs)
                .is_none_or(|value| value > limit)
            {
                return Err(GridError::new(
                    "coverage.candidate_limit",
                    "pairwise-intersection count exceeds the configured limit",
                ));
            }
            for guide_a in &grouped[left] {
                if cancelled() {
                    return Err(GridError::new(
                        "evaluation.cancelled",
                        "evaluation was cancelled",
                    ));
                }
                for guide_b in &grouped[right] {
                    if let Some(point) = line_intersection(guide_a, guide_b)
                        && distance_to_canvas(point, canvas) <= margin
                    {
                        raw.push((point, vec![guide_a.id, guide_b.id]));
                    }
                }
            }
        }
    }
    if raw.is_empty() {
        return Err(GridError::new(
            "pattern.family.intersections",
            "selected parallel or coincident dimensions cannot produce intersections",
        ));
    }
    raw.sort_by(|a, b| {
        a.0.x
            .total_cmp(&b.0.x)
            .then(a.0.y.total_cmp(&b.0.y))
            .then(a.1.cmp(&b.1))
    });
    let mut sites: Vec<GeneralizedSite> = Vec::with_capacity(raw.len());
    for (point, contributors) in raw {
        if cancelled() {
            return Err(GridError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        if let Some(existing) = sites
            .last_mut()
            .filter(|site| distance(site.position, point) <= epsilon)
        {
            let GeneralizedSiteProvenance::Intersection {
                contributors: existing_contributors,
            } = &mut existing.provenance
            else {
                unreachable!()
            };
            existing_contributors.extend(contributors);
            existing_contributors.sort();
            existing_contributors.dedup();
            continue;
        }
        let mut contributors = contributors;
        contributors.sort();
        contributors.dedup();
        let scope = if canvas.contains(point) {
            SiteScope::Canvas
        } else {
            SiteScope::Guard
        };
        sites.push(GeneralizedSite {
            sequence: sites.len(),
            position: point,
            scope,
            provenance: GeneralizedSiteProvenance::Intersection { contributors },
        });
    }
    Ok(sites)
}

#[allow(clippy::too_many_arguments)] // Explicit structural inputs keep this product independent of render state.
fn generalized_along_guides(
    grouped: &[Vec<StraightGuide>],
    selected: &[usize],
    interval: f64,
    phase: f64,
    canvas: Bounds,
    margin: f64,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<GeneralizedSite>, GridError> {
    if selected.is_empty() {
        return Err(GridError::new(
            "pattern.family.along_guides.dimensions",
            "along-guide selection must not be empty",
        ));
    }
    if !interval.is_finite() || interval <= 0.0 || !phase.is_finite() {
        return Err(GridError::new(
            "pattern.family.along_guides",
            "arc-length interval and phase must be finite",
        ));
    }
    let mut sites = Vec::new();
    let mut guide_order = 0;
    for &dimension in selected {
        for guide in &grouped[dimension] {
            if cancelled() {
                return Err(GridError::new(
                    "evaluation.cancelled",
                    "evaluation was cancelled",
                ));
            }
            let start_arc = Vector2::new(
                guide.start.x - guide.anchor.x,
                guide.start.y - guide.anchor.y,
            )
            .dot(guide.tangent);
            let end_arc = Vector2::new(guide.end.x - guide.anchor.x, guide.end.y - guide.anchor.y)
                .dot(guide.tangent);
            let minimum = start_arc.min(end_arc);
            let maximum = start_arc.max(end_arc);
            let first_sequence = checked_index(((minimum - phase) / interval).ceil())?;
            let last_sequence = checked_index(((maximum - phase) / interval).floor())?;
            let count = last_sequence
                .checked_sub(first_sequence)
                .and_then(|span| span.checked_add(1))
                .ok_or(GridError::new(
                    "coverage.along_guides.count",
                    "along-guide sequence range overflowed",
                ))?;
            let count = usize::try_from(count).map_err(|_| {
                GridError::new(
                    "coverage.along_guides.count",
                    "along-guide sequence range overflowed",
                )
            })?;
            if sites
                .len()
                .checked_add(count)
                .is_none_or(|value| value > limit)
            {
                return Err(GridError::new(
                    "coverage.candidate_limit",
                    "along-guide site count exceeds the configured limit",
                ));
            }
            for sequence in first_sequence..=last_sequence {
                if cancelled() {
                    return Err(GridError::new(
                        "evaluation.cancelled",
                        "evaluation was cancelled",
                    ));
                }
                let absolute = phase + sequence as f64 * interval;
                // Both positions use the guide's stable anchor frame.  The
                // finite coverage segment selects which lattice members are
                // emitted; it never becomes a provenance origin.
                let local = absolute;
                let point = Point2::new(
                    guide.anchor.x + guide.tangent.x * absolute,
                    guide.anchor.y + guide.tangent.y * absolute,
                );
                if distance_to_canvas(point, canvas) > margin {
                    continue;
                }
                let scope = if canvas.contains(point) {
                    SiteScope::Canvas
                } else {
                    SiteScope::Guard
                };
                sites.push(GeneralizedSite {
                    sequence: sites.len(),
                    position: point,
                    scope,
                    provenance: GeneralizedSiteProvenance::AlongGuide {
                        guide_id: guide.id,
                        guide_order,
                        sequence,
                        absolute_arc_position_bits: absolute.to_bits(),
                        local_arc_position_bits: local.to_bits(),
                    },
                });
            }
            guide_order += 1;
        }
    }
    Ok(sites)
}

fn line_intersection(a: &StraightGuide, b: &StraightGuide) -> Option<Point2> {
    let r = Vector2::new(a.end.x - a.start.x, a.end.y - a.start.y);
    let s = Vector2::new(b.end.x - b.start.x, b.end.y - b.start.y);
    let cross = r.x.mul_add(s.y, -r.y * s.x);
    if cross.abs() <= 1e-12 {
        return None;
    }
    let delta = Vector2::new(b.start.x - a.start.x, b.start.y - a.start.y);
    let t = delta.x.mul_add(s.y, -delta.y * s.x) / cross;
    let point = Point2::new(a.start.x + t * r.x, a.start.y + t * r.y);
    point.is_finite().then_some(point)
}

fn distance(a: Point2, b: Point2) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

fn validate_straight_request(request: &StraightGuideInspectRequest) -> Result<(), GridError> {
    validate(&GridInspectRequest {
        canvas: request.canvas.clone(),
        density: request.density.clone(),
        rotation_degrees: request.rotation_degrees,
        translation_x: request.translation_x,
        translation_y: request.translation_y,
        guard_steps: request.guard_steps,
        support_radius: request.support_radius,
        max_family_candidates: request.max_family_candidates,
    })
}

fn generalized_fingerprint(
    family: &FamilyCapability,
    request: &StraightGuideInspectRequest,
) -> String {
    let mut bytes = b"toniator-stage-16a-straight-guide-family-v1".to_vec();
    bytes.extend(family.provenance.definition_id.to_le_bytes());
    for mechanism_id in &family.provenance.mechanism_ids {
        bytes.extend(mechanism_id.0.to_le_bytes());
    }
    for dimension in &family.dimensions {
        bytes.extend(dimension.id.0.to_le_bytes());
        bytes.extend(dimension.baseline_angle_degrees.to_bits().to_le_bytes());
        bytes.extend(dimension.phase.to_bits().to_le_bytes());
        bytes.extend(
            dimension
                .repetition
                .spacing_multiplier
                .to_bits()
                .to_le_bytes(),
        );
    }
    for id in &family.site_selection {
        bytes.extend(id.0.to_le_bytes());
    }
    bytes.push(match family.product {
        StructuralProductCapability::GuideIntersections => 1,
        StructuralProductCapability::AlongGuideSites => 2,
        StructuralProductCapability::RandomSites => 3,
        StructuralProductCapability::ParametricPaths => 4,
    });
    bytes.extend(family.merge_epsilon.unwrap_or(0.0).to_bits().to_le_bytes());
    bytes.extend(
        family
            .along_interval_multiplier
            .unwrap_or(0.0)
            .to_bits()
            .to_le_bytes(),
    );
    bytes.extend(family.along_phase.unwrap_or(0.0).to_bits().to_le_bytes());
    bytes.extend(request.canvas.width.to_bits().to_le_bytes());
    bytes.extend(request.canvas.height.to_bits().to_le_bytes());
    bytes.extend(request.density.across_x.to_bits().to_le_bytes());
    bytes.extend(request.density.across_y.to_bits().to_le_bytes());
    bytes.push(u8::from(request.density.aspect_locked));
    bytes.extend(request.rotation_degrees.to_bits().to_le_bytes());
    bytes.extend(request.translation_x.to_bits().to_le_bytes());
    bytes.extend(request.translation_y.to_bits().to_le_bytes());
    bytes.extend(request.guard_steps.to_le_bytes());
    bytes.extend(request.support_radius.to_bits().to_le_bytes());
    bytes.extend(
        u64::try_from(request.max_family_candidates)
            .expect("usize fits u64")
            .to_le_bytes(),
    );
    fnv1a64(bytes)
}

/// Immutable, renderer-independent circular realization of an existing family.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CircularMarkRealization {
    pub family_fingerprint: String,
    pub realization_fingerprint: String,
    pub source_identity: toniator_sampling::SourceIdentity,
    pub source_component: SourceComponent,
    pub placement: SourcePlacement,
    pub response: MarkResponse,
    pub marks: Vec<CanonicalCircleMark>,
}

/// Immutable generalized canonical mark realization for current typed output layers.
///
/// This is the Stage 20E2 realization boundary: it consumes existing family
/// sites and resolved document resources, and never allocates or reorders sites.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalMarkRealization {
    pub family_fingerprint: String,
    pub realization_fingerprint: String,
    pub source_identity: toniator_sampling::SourceIdentity,
    pub response: MarkResponse,
    pub marks: Vec<CanonicalMark>,
    pub paints: Option<Vec<SampledSourcePaint>>,
}

/// Effective normalized width response consumed only by guide-path realization.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeResponse {
    pub minimum_thickness: f64,
    pub maximum_thickness: f64,
}

/// Immutable ordered canonical strokes produced from one guide-path output layer.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalStrokeRealization {
    pub family_fingerprint: String,
    pub realization_fingerprint: String,
    pub source_identity: toniator_sampling::SourceIdentity,
    pub response: StrokeResponse,
    pub strokes: Vec<CanonicalStroke>,
}

/// Stable canonical sweep contract included in all stroke realization identity.
pub const CANONICAL_STROKE_OUTLINE_CONTRACT_ID: &str = "toniator-stage-20i-filled-outline-v3";
/// Bounds adaptive samples per evaluation request before profile allocation.
pub const MAX_STROKE_PROFILE_SAMPLES: usize = 262_144;
/// Bounds derived filled-outline segments per evaluation request.
pub const MAX_STROKE_OUTLINE_SEGMENTS: usize = 524_288;
/// Caps adaptive centerline/width subdivision deterministically.
pub const MAX_STROKE_SUBDIVISION_DEPTH: u8 = 48;
const STROKE_CENTERLINE_TOLERANCE: f64 = 1.0 / 64.0;
const STROKE_WIDTH_TOLERANCE: f64 = 1.0 / 64.0;

/// Realizes ordered guide paths into reusable finite filled outlines.
///
/// # Errors
///
/// Returns a stable capability, sampling, cancellation, or canonical-geometry
/// error without exposing a partial stroke collection.
#[allow(clippy::too_many_arguments)]
pub fn realize_typed_canonical_strokes_cancellable(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    _nominal_basis: f64,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedRealization<CanonicalStrokeRealization>, PatternPipelineError> {
    let provenance = realization_provenance(family, plan)?;
    let [output] = plan.ordered_outputs.as_slice() else {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.capability",
            "canonical stroke realization requires one output layer",
        ));
    };
    if output.guide_paths().is_none() {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.capability",
            "canonical stroke realization requires a guide-path output",
        ));
    }
    let paths = family
        .structural_path_set()
        .ok_or(PatternPipelineError::new(
            "pattern.output_layers.guide_paths",
            "guide-path output requires derived guide paths",
        ))?;
    let mut strokes = Vec::with_capacity(paths.paths().len());
    let mut profile_samples = 0_usize;
    let mut outline_segments = 0_usize;
    for guide in paths.paths() {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        let nominal_basis =
            family
                .guide_nominal_basis(guide.id)
                .ok_or(PatternPipelineError::new(
                    "realization.stroke.basis",
                    "guide path has no resolved nominal spacing basis",
                ))?;
        let pixel_footprint = (canvas.width / f64::from(source.identity().width))
            .min(canvas.height / f64::from(source.identity().height));
        let mut profile: Vec<StrokeProfileSample> =
            Vec::with_capacity(guide.path.segments().len() * 2);
        for (segment_index, segment) in guide.path.segments().iter().enumerate() {
            let start = stroke_sample(
                segment,
                segment_index,
                0.0,
                source,
                canvas,
                mapping,
                response,
                nominal_basis,
            )?;
            let end = stroke_sample(
                segment,
                segment_index,
                1.0,
                source,
                canvas,
                mapping,
                response,
                nominal_basis,
            )?;
            if profile
                .last()
                .is_some_and(|previous| previous.location != start.location)
            {
                if profile_samples >= max_profile_samples {
                    return Err(PatternPipelineError::new(
                        "realization.stroke.profile_limit",
                        "canonical stroke profile exceeds the sample limit",
                    ));
                }
                profile.push(start);
                profile_samples += 1;
            }
            append_adaptive_stroke_interval(
                segment,
                segment_index,
                0.0,
                start,
                1.0,
                end,
                0,
                pixel_footprint,
                source,
                canvas,
                mapping,
                response,
                nominal_basis,
                &mut profile,
                &mut profile_samples,
                max_profile_samples,
                is_cancelled,
            )?;
        }
        let outline = build_variable_width_outline_cancellable(
            &guide.path,
            &profile
                .iter()
                .map(|sample| VariableWidthPathSample {
                    location: sample.location,
                    width: sample.width,
                })
                .collect::<Vec<_>>(),
            output.guide_paths().expect("validated guide output").1,
            1.0 / 8.0,
            VariableWidthOutlineLimits::new(
                max_outline_segments.saturating_sub(outline_segments).max(1),
            )
            .map_err(|_| {
                PatternPipelineError::new(
                    "realization.stroke.outline_limit",
                    "configured stroke outline limit must be nonzero",
                )
            })?,
            is_cancelled,
        )
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
        outline_segments = outline_segments
            .checked_add(
                outline
                    .contours
                    .iter()
                    .map(|contour| contour.segments.len())
                    .sum::<usize>(),
            )
            .ok_or(PatternPipelineError::new(
                "realization.stroke.outline_limit",
                "canonical stroke outline exceeds the segment limit",
            ))?;
        if outline_segments > max_outline_segments {
            return Err(PatternPipelineError::new(
                "realization.stroke.outline_limit",
                "canonical stroke outline exceeds the segment limit",
            ));
        }
        strokes.push(
            CanonicalStroke::new(
                guide.id,
                guide.source_structure_id,
                guide.path.clone(),
                nominal_basis,
                output.guide_paths().expect("validated guide output").1,
                profile,
                outline,
            )
            .map_err(|_| {
                PatternPipelineError::new(
                    "realization.stroke.geometry",
                    "canonical stroke geometry must remain finite",
                )
            })?,
        );
    }
    let mut bytes = family.family_fingerprint().as_bytes().to_vec();
    bytes.extend(CANONICAL_STROKE_OUTLINE_CONTRACT_ID.as_bytes());
    bytes.extend(response.minimum_thickness.to_bits().to_le_bytes());
    bytes.extend(response.maximum_thickness.to_bits().to_le_bytes());
    for stroke in &strokes {
        append_structural_path_instance_identity(&mut bytes, stroke.source_path_id);
        for sample in &stroke.profile {
            bytes.extend(sample.center.x.to_bits().to_le_bytes());
            bytes.extend(sample.center.y.to_bits().to_le_bytes());
            bytes.extend(sample.location.segment_index().to_le_bytes());
            bytes.extend(sample.location.parameter().to_bits().to_le_bytes());
            bytes.extend(sample.normalized_thickness.to_bits().to_le_bytes());
        }
        for contour in &stroke.outline.contours {
            bytes.extend((contour.segments.len() as u64).to_le_bytes());
        }
    }
    Ok(TypedRealization {
        provenance,
        output: CanonicalStrokeRealization {
            family_fingerprint: family.family_fingerprint().to_owned(),
            realization_fingerprint: fnv1a64(bytes),
            source_identity: source.identity().clone(),
            response,
            strokes,
        },
    })
}

/// Samples one exact segment-local centerline position and its normalized round-brush width.
///
/// # Errors
///
/// Propagates bounded source sampling failures without substituting a synthetic width.
#[allow(clippy::too_many_arguments)]
fn stroke_sample(
    segment: &CurveSegment,
    segment_index: usize,
    parameter: f64,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    basis: f64,
) -> Result<StrokeProfileSample, PatternPipelineError> {
    let center = segment.point_at(parameter).map_err(|_| {
        PatternPipelineError::new(
            "realization.stroke.centerline",
            "stroke centerline must remain finite",
        )
    })?;
    let ink = source
        .sample_mapping_response(center, canvas, mapping)
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
    let thickness = response.minimum_thickness
        + ink * (response.maximum_thickness - response.minimum_thickness);
    let width = thickness * basis;
    Ok(StrokeProfileSample {
        location: PathLocation::new(segment_index, parameter).map_err(|_| {
            PatternPipelineError::new(
                "realization.stroke.location",
                "stroke location must remain normalized",
            )
        })?,
        center,
        normalized_thickness: thickness,
        width,
    })
}

/// Adaptively appends the right endpoint of one interval once its centerline and width meet Stage 20I bounds.
///
/// # Errors
///
/// Stops atomically at cancellation, depth, numeric, or profile-allocation limits.
#[allow(clippy::too_many_arguments)]
fn append_adaptive_stroke_interval(
    segment: &CurveSegment,
    segment_index: usize,
    left_t: f64,
    left: StrokeProfileSample,
    right_t: f64,
    right: StrokeProfileSample,
    depth: u8,
    pixel_footprint: f64,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    basis: f64,
    profile: &mut Vec<StrokeProfileSample>,
    profile_samples: &mut usize,
    max_profile_samples: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), PatternPipelineError> {
    if is_cancelled() {
        return Err(PatternPipelineError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ));
    }
    let middle_t = (left_t + right_t) * 0.5;
    let middle = stroke_sample(
        segment,
        segment_index,
        middle_t,
        source,
        canvas,
        mapping,
        response,
        basis,
    )?;
    let chord_middle = Point2::new(
        (left.center.x + right.center.x) * 0.5,
        (left.center.y + right.center.y) * 0.5,
    );
    let midpoint_error = ((middle.center.x - chord_middle.x).powi(2)
        + (middle.center.y - chord_middle.y).powi(2))
    .sqrt();
    let centerline_error = midpoint_error.max(cubic_subcurve_flatness(segment, left_t, right_t));
    let interval = ((right.center.x - left.center.x).powi(2)
        + (right.center.y - left.center.y).powi(2))
    .sqrt();
    let width_error = (middle.width - (left.width + right.width) * 0.5).abs();
    let refine = centerline_error > STROKE_CENTERLINE_TOLERANCE
        || interval > pixel_footprint * 0.5
        || width_error > STROKE_WIDTH_TOLERANCE;
    if refine {
        if depth >= MAX_STROKE_SUBDIVISION_DEPTH {
            return Err(PatternPipelineError::new(
                "realization.stroke.subdivision_depth",
                "stroke response exceeds the adaptive subdivision depth",
            ));
        }
        append_adaptive_stroke_interval(
            segment,
            segment_index,
            left_t,
            left,
            middle_t,
            middle,
            depth + 1,
            pixel_footprint,
            source,
            canvas,
            mapping,
            response,
            basis,
            profile,
            profile_samples,
            max_profile_samples,
            is_cancelled,
        )?;
        append_adaptive_stroke_interval(
            segment,
            segment_index,
            middle_t,
            middle,
            right_t,
            right,
            depth + 1,
            pixel_footprint,
            source,
            canvas,
            mapping,
            response,
            basis,
            profile,
            profile_samples,
            max_profile_samples,
            is_cancelled,
        )?;
    } else {
        if profile.is_empty() {
            profile.push(left);
            *profile_samples += 1;
        }
        if *profile_samples >= max_profile_samples {
            return Err(PatternPipelineError::new(
                "realization.stroke.profile_limit",
                "canonical stroke profile exceeds the sample limit",
            ));
        }
        profile.push(right);
        *profile_samples += 1;
    }
    Ok(())
}

/// Returns a conservative De Casteljau control-polygon flatness bound for one original cubic subinterval.
fn cubic_subcurve_flatness(segment: &CurveSegment, left: f64, right: f64) -> f64 {
    let CurveSegment::CubicBezier(cubic) = segment else {
        return 0.0;
    };
    let lerp =
        |a: Point2, b: Point2, t: f64| Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
    let split = |points: [Point2; 4], t: f64| {
        let a = lerp(points[0], points[1], t);
        let b = lerp(points[1], points[2], t);
        let c = lerp(points[2], points[3], t);
        let d = lerp(a, b, t);
        let e = lerp(b, c, t);
        let f = lerp(d, e, t);
        ([points[0], a, d, f], [f, e, c, points[3]])
    };
    let points = [
        cubic.start(),
        cubic.control_1(),
        cubic.control_2(),
        cubic.end(),
    ];
    let (at_right, _) = split(points, right);
    let fraction = if right > 0.0 { left / right } else { 0.0 };
    let (_, interval) = split(at_right, fraction);
    let start = interval[0];
    let end = interval[3];
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length == 0.0 {
        return interval[1..3]
            .iter()
            .map(|point| ((point.x - start.x).powi(2) + (point.y - start.y).powi(2)).sqrt())
            .fold(0.0, f64::max);
    }
    interval[1..3]
        .iter()
        .map(|point| ((dy * (point.x - start.x) - dx * (point.y - start.y)).abs()) / length)
        .fold(0.0, f64::max)
}

/// The exact nonzero upper bound for transformed authored curve-segment instances.
pub const MAX_TRANSFORMED_CURVE_SEGMENT_INSTANCES: usize = 1_048_576;

/// Immutable scalar-field realization for an explicit Stage 9 source mapping.
/// This is deliberately distinct from the retained Stage 3–8 single-channel
/// realization so its new mapping identity cannot alter accepted results.
#[derive(Clone, Debug, PartialEq)]
pub struct MappedCircularMarkRealization {
    pub family_fingerprint: String,
    pub realization_fingerprint: String,
    pub source_identity: toniator_sampling::SourceIdentity,
    pub mapping: SourceMapping,
    pub response: MarkResponse,
    pub marks: Vec<CanonicalCircleMark>,
}

impl MappedCircularMarkRealization {
    pub fn has_only_finite_marks(&self) -> bool {
        self.marks
            .iter()
            .all(|mark| mark.center.is_finite() && mark.radius.is_finite())
    }
}

/// One immutable SourceColorAlpha mark. `paint.alpha` is always one; channel
/// presentation opacity is intentionally not represented at this layer.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceColorCircleMark {
    pub mark: CanonicalCircleMark,
    pub paint: SampledSourcePaint,
}

/// SourceColorAlpha realization. Exact-zero source alpha is represented by an
/// omitted mark, rather than a transparent sampled paint.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceColorCircularMarkRealization {
    pub family_fingerprint: String,
    pub realization_fingerprint: String,
    pub source_identity: toniator_sampling::SourceIdentity,
    pub mapping: SourceMapping,
    pub response: MarkResponse,
    pub marks: Vec<SourceColorCircleMark>,
}

impl SourceColorCircularMarkRealization {
    pub fn has_only_finite_marks(&self) -> bool {
        self.marks.iter().all(|mark| {
            mark.mark.center.is_finite()
                && mark.mark.radius.is_finite()
                && mark.paint.red.is_finite()
                && mark.paint.green.is_finite()
                && mark.paint.blue.is_finite()
                && mark.paint.alpha == 1.0
        })
    }
}

impl CircularMarkRealization {
    pub fn has_only_finite_marks(&self) -> bool {
        self.marks
            .iter()
            .all(|mark| mark.center.is_finite() && mark.radius.is_finite())
    }
}

/// The bounded diameter response used to realize canonical radii.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct MarkResponse {
    pub minimum_fill: f64,
    pub maximum_fill: f64,
    pub rotation_offset_degrees: f64,
}

/// A realization-boundary failure. Family generation errors remain `GridError`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizationError {
    path: &'static str,
    message: &'static str,
}

impl RealizationError {
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

impl fmt::Display for RealizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for RealizationError {}

impl From<SamplingError> for RealizationError {
    fn from(error: SamplingError) -> Self {
        Self::new(error.path(), error.message())
    }
}

/// Realizes every supplied family site in its existing stable order.
///
/// This function deliberately receives `GridFamilyOutput` rather than a grid
/// request so a size change cannot recreate guides, sites, or provenance.
pub fn realize_circular_marks(
    family: &GridFamilyOutput,
    source: &SourceField,
    canvas: &CanvasSpec,
    placement: SourcePlacement,
    component: SourceComponent,
    response: MarkResponse,
) -> Result<CircularMarkRealization, RealizationError> {
    validate_response(response, family.support_radius)?;
    if !family.has_only_finite_geometry() {
        return Err(RealizationError::new(
            "realization.family",
            "family geometry must be finite",
        ));
    }
    let mut marks = Vec::with_capacity(family.sites.len());
    for site in &family.sites {
        let ink = source.sample_mark_ink(site.position, canvas, placement, component)?;
        if !ink.is_finite() {
            return Err(RealizationError::new(
                "realization.sample",
                "effective mark ink must be finite",
            ));
        }
        let radius = radius_from_ink_with_diameter(ink, response, site.nominal_cell_diameter)?;
        if radius > family.support_radius {
            return Err(RealizationError::new(
                "realization.family.support_radius",
                "realized radius exceeds the planned family support",
            ));
        }
        let mark = CanonicalCircleMark::new(
            site.id,
            site.position,
            radius,
            site.scope,
            site.provenance.clone(),
        )
        .ok_or(RealizationError::new(
            "realization.mark",
            "mark geometry must be finite",
        ))?;
        marks.push(mark);
    }
    let output = CircularMarkRealization {
        family_fingerprint: family.family_fingerprint.clone(),
        realization_fingerprint: realization_fingerprint(
            family, source, placement, component, response,
        ),
        source_identity: source.identity().clone(),
        source_component: component,
        placement,
        response,
        marks,
    };
    output
        .has_only_finite_marks()
        .then_some(output)
        .ok_or(RealizationError::new(
            "realization.mark",
            "realization produced non-finite marks",
        ))
}

/// Realizes an explicit Stage 9 scalar mapping while retaining the family as a
/// structural-only input. Mapping, source, and response all live in the
/// realization identity.
pub fn realize_mapped_circular_marks(
    family: &GridFamilyOutput,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: MarkResponse,
) -> Result<MappedCircularMarkRealization, RealizationError> {
    validate_response(response, family.support_radius)?;
    if !family.has_only_finite_geometry() {
        return Err(RealizationError::new(
            "realization.family",
            "family geometry must be finite",
        ));
    }
    let mut marks = Vec::with_capacity(family.sites.len());
    for site in &family.sites {
        let ink = source.sample_mapping_response(site.position, canvas, mapping)?;
        let radius = radius_from_ink_with_diameter(ink, response, site.nominal_cell_diameter)?;
        if radius > family.support_radius {
            return Err(RealizationError::new(
                "realization.family.support_radius",
                "realized radius exceeds the planned family support",
            ));
        }
        let mark = CanonicalCircleMark::new(
            site.id,
            site.position,
            radius,
            site.scope,
            site.provenance.clone(),
        )
        .ok_or(RealizationError::new(
            "realization.mark",
            "mark geometry must be finite",
        ))?;
        marks.push(mark);
    }
    let output = MappedCircularMarkRealization {
        family_fingerprint: family.family_fingerprint.clone(),
        realization_fingerprint: mapped_realization_fingerprint(
            family, source, mapping, response, &marks,
        ),
        source_identity: source.identity().clone(),
        mapping,
        response,
        marks,
    };
    output
        .has_only_finite_marks()
        .then_some(output)
        .ok_or(RealizationError::new(
            "realization.mark",
            "realization produced non-finite marks",
        ))
}

/// Realizes source-colored marks without a compositor. Associated interpolation
/// and unassociation are owned by sampling; this layer only turns the sampled
/// alpha response into size and retains an immutable per-mark paint. A source
/// sample with exactly zero alpha produces no mark even with a positive minimum
/// diameter.
pub fn realize_source_color_circular_marks(
    family: &GridFamilyOutput,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: MarkResponse,
) -> Result<SourceColorCircularMarkRealization, RealizationError> {
    validate_response(response, family.support_radius)?;
    if !family.has_only_finite_geometry() {
        return Err(RealizationError::new(
            "realization.family",
            "family geometry must be finite",
        ));
    }
    let mut marks = Vec::with_capacity(family.sites.len());
    for site in &family.sites {
        let sample = source.sample_source_color(site.position, canvas, mapping)?;
        let Some(paint) = sample.paint else {
            continue;
        };
        let radius =
            radius_from_ink_with_diameter(sample.response, response, site.nominal_cell_diameter)?;
        if radius > family.support_radius {
            return Err(RealizationError::new(
                "realization.family.support_radius",
                "realized radius exceeds the planned family support",
            ));
        }
        let mark = CanonicalCircleMark::new(
            site.id,
            site.position,
            radius,
            site.scope,
            site.provenance.clone(),
        )
        .ok_or(RealizationError::new(
            "realization.mark",
            "mark geometry must be finite",
        ))?;
        marks.push(SourceColorCircleMark { mark, paint });
    }
    let output = SourceColorCircularMarkRealization {
        family_fingerprint: family.family_fingerprint.clone(),
        realization_fingerprint: source_color_realization_fingerprint(
            family, source, mapping, response, &marks,
        ),
        source_identity: source.identity().clone(),
        mapping,
        response,
        marks,
    };
    output
        .has_only_finite_marks()
        .then_some(output)
        .ok_or(RealizationError::new(
            "realization.mark",
            "realization produced non-finite marks",
        ))
}

/// Maps an effective mark response to radius using one finite nominal cell diameter.
pub fn radius_from_ink_with_diameter(
    ink: f64,
    response: MarkResponse,
    nominal_cell_diameter: f64,
) -> Result<f64, RealizationError> {
    validate_response_basic(response)?;
    if !ink.is_finite() {
        return Err(RealizationError::new(
            "realization.ink",
            "effective mark ink must be finite",
        ));
    }
    let ink = ink.clamp(0.0, 1.0);
    if !nominal_cell_diameter.is_finite() || nominal_cell_diameter <= 0.0 {
        return Err(RealizationError::new(
            "realization.nominal_cell_diameter",
            "nominal cell diameter must be finite and positive",
        ));
    }
    Ok(
        (response.minimum_fill + ink * (response.maximum_fill - response.minimum_fill))
            * nominal_cell_diameter
            / 2.0,
    )
}

fn validate_response(
    response: MarkResponse,
    additional_margin: f64,
) -> Result<(), RealizationError> {
    validate_response_basic(response)?;
    if !additional_margin.is_finite() || additional_margin < 0.0 {
        return Err(RealizationError::new(
            "realization.family.support_radius",
            "family support capability must be finite and nonnegative",
        ));
    }
    if response.maximum_fill > 2.0 {
        return Err(RealizationError::new(
            "realization.response.maximum_fill",
            "maximum fill must not exceed 2.0",
        ));
    }
    Ok(())
}

fn validate_response_basic(response: MarkResponse) -> Result<(), RealizationError> {
    if !response.minimum_fill.is_finite()
        || !response.maximum_fill.is_finite()
        || !response.rotation_offset_degrees.is_finite()
    {
        return Err(RealizationError::new(
            "realization.response",
            "fill response and rotation offset must be finite",
        ));
    }
    if response.minimum_fill < 0.0
        || response.maximum_fill > 2.0
        || response.minimum_fill > response.maximum_fill
    {
        return Err(RealizationError::new(
            "realization.response",
            "fill values must be within 0.0..=2.0 and minimum must not exceed maximum",
        ));
    }
    Ok(())
}

fn realization_fingerprint(
    family: &GridFamilyOutput,
    source: &SourceField,
    placement: SourcePlacement,
    component: SourceComponent,
    response: MarkResponse,
) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let placement = match placement {
        SourcePlacement::StretchToCanvas => 1_u8,
    };
    let component = match component {
        SourceComponent::Luminance => 1_u8,
        SourceComponent::Alpha => 2_u8,
    };
    let format = match source.identity().format {
        toniator_sampling::SourceFormat::Png => 1_u8,
        toniator_sampling::SourceFormat::Svg => 2_u8,
    };
    for byte in b"toniator-stage-4-circular-realization-v2-alpha-associated"
        .iter()
        .copied()
        .chain(family.family_fingerprint.bytes())
        .chain(source.identity().content_hash.bytes())
        .chain(source.identity().decoded_pixel_hash.bytes())
        .chain([format, placement, component])
        .chain(source.identity().width.to_le_bytes())
        .chain(source.identity().height.to_le_bytes())
        .chain(response.minimum_fill.to_bits().to_le_bytes())
        .chain(response.maximum_fill.to_bits().to_le_bytes())
        .chain(response.rotation_offset_degrees.to_bits().to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn mapped_realization_fingerprint(
    family: &GridFamilyOutput,
    source: &SourceField,
    mapping: SourceMapping,
    response: MarkResponse,
    marks: &[CanonicalCircleMark],
) -> String {
    let mut bytes = realization_identity_prefix(
        b"toniator-stage-9b-mapped-circular-realization-v1",
        family,
        source,
        mapping,
        response,
    );
    for mark in marks {
        append_mark_identity(&mut bytes, mark);
    }
    fnv1a64(bytes)
}

fn source_color_realization_fingerprint(
    family: &GridFamilyOutput,
    source: &SourceField,
    mapping: SourceMapping,
    response: MarkResponse,
    marks: &[SourceColorCircleMark],
) -> String {
    let mut bytes = realization_identity_prefix(
        b"toniator-stage-9b-source-color-circular-realization-v1",
        family,
        source,
        mapping,
        response,
    );
    for source_color_mark in marks {
        append_mark_identity(&mut bytes, &source_color_mark.mark);
        bytes.extend(source_color_mark.paint.red.to_bits().to_le_bytes());
        bytes.extend(source_color_mark.paint.green.to_bits().to_le_bytes());
        bytes.extend(source_color_mark.paint.blue.to_bits().to_le_bytes());
        bytes.extend(source_color_mark.paint.alpha.to_bits().to_le_bytes());
    }
    fnv1a64(bytes)
}

fn realization_identity_prefix(
    contract: &[u8],
    family: &GridFamilyOutput,
    source: &SourceField,
    mapping: SourceMapping,
    response: MarkResponse,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(contract);
    bytes.extend(family.family_fingerprint.bytes());
    bytes.extend(source.identity().content_hash.bytes());
    bytes.extend(source.identity().decoded_pixel_hash.bytes());
    bytes.push(match source.identity().format {
        toniator_sampling::SourceFormat::Png => 1,
        toniator_sampling::SourceFormat::Svg => 2,
    });
    bytes.extend(source.identity().width.to_le_bytes());
    bytes.extend(source.identity().height.to_le_bytes());
    bytes.push(match mapping.placement {
        SourcePlacement::StretchToCanvas => 1,
    });
    bytes.push(mapping_component_code(mapping.component));
    bytes.push(u8::from(mapping.inverted));
    bytes.extend(mapping.gain.to_bits().to_le_bytes());
    bytes.extend(mapping.bias.to_bits().to_le_bytes());
    bytes.extend(response.minimum_fill.to_bits().to_le_bytes());
    bytes.extend(response.maximum_fill.to_bits().to_le_bytes());
    bytes.extend(response.rotation_offset_degrees.to_bits().to_le_bytes());
    bytes
}

fn mapping_component_code(component: SourceMappingComponent) -> u8 {
    match component {
        SourceMappingComponent::Red => 1,
        SourceMappingComponent::Green => 2,
        SourceMappingComponent::Blue => 3,
        SourceMappingComponent::Cyan => 4,
        SourceMappingComponent::Magenta => 5,
        SourceMappingComponent::Yellow => 6,
        SourceMappingComponent::Black => 7,
        SourceMappingComponent::Alpha => 8,
        SourceMappingComponent::Luminance => 9,
    }
}

/// Appends every source identity discriminator used to derive canonical geometry or sampled paint.
fn append_source_identity(bytes: &mut Vec<u8>, source: &SourceField) {
    let identity = source.identity();
    bytes.push(match identity.format {
        toniator_sampling::SourceFormat::Png => 1,
        toniator_sampling::SourceFormat::Svg => 2,
    });
    append_identity_text(bytes, identity.content_hash.as_str());
    append_identity_text(bytes, identity.decoded_pixel_hash.as_str());
    bytes.extend(identity.width.to_le_bytes());
    bytes.extend(identity.height.to_le_bytes());
}

/// Delimits one textual identity field so adjacent variable-length values remain unambiguous.
fn append_identity_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend(
        u64::try_from(value.len())
            .expect("string length fits u64")
            .to_le_bytes(),
    );
    bytes.extend(value.bytes());
}

/// Appends the full source-mapping contract that determines response and sampled paint.
fn append_source_mapping_identity(bytes: &mut Vec<u8>, mapping: SourceMapping) {
    bytes.push(mapping_component_code(mapping.component));
    bytes.push(match mapping.placement {
        SourcePlacement::StretchToCanvas => 1,
    });
    bytes.push(u8::from(mapping.inverted));
    bytes.extend(mapping.gain.to_bits().to_le_bytes());
    bytes.extend(mapping.bias.to_bits().to_le_bytes());
}

/// Appends the complete ordered output-layer contract before derived canonical geometry.
fn append_output_capability_identity(bytes: &mut Vec<u8>, output: &OutputCapability) {
    bytes.extend(output.layer_id.0.to_le_bytes());
    bytes.push(match output.consumes {
        StructuralProductCapability::GuideIntersections => 1,
        StructuralProductCapability::AlongGuideSites => 2,
        StructuralProductCapability::RandomSites => 3,
        StructuralProductCapability::ParametricPaths => 4,
    });
    match &output.payload {
        OutputCapabilityPayload::Marks {
            prototype,
            orientation,
        } => {
            bytes.push(1);
            match prototype {
                MarkPrototype::Circle => bytes.push(1),
                MarkPrototype::AuthoredClosedShape { structure_id } => {
                    bytes.push(2);
                    bytes.extend(structure_id.0.to_le_bytes());
                }
            }
            match orientation {
                MarkOrientation::Fixed => bytes.push(1),
                MarkOrientation::GuideTangent { dimension_id } => {
                    bytes.push(2);
                    bytes.extend(dimension_id.0.to_le_bytes());
                }
                MarkOrientation::GuideNormal { dimension_id } => {
                    bytes.push(3);
                    bytes.extend(dimension_id.0.to_le_bytes());
                }
            }
        }
        OutputCapabilityPayload::GuidePaths {
            guide_mechanism_id,
            style,
        } => {
            bytes.push(2);
            bytes.extend(guide_mechanism_id.0.to_le_bytes());
            bytes.push(match style.join {
                toniator_domain::StrokeJoin::Round => 1,
            });
            bytes.push(match style.cap {
                toniator_domain::StrokeCap::Round => 1,
            });
        }
    }
}

/// Appends one ordered generalized canonical mark, including geometry and truthful site provenance.
fn append_canonical_mark_identity(bytes: &mut Vec<u8>, mark: &CanonicalMark) {
    match mark {
        CanonicalMark::Circle {
            source_site_id,
            center,
            radius,
            scope,
            provenance,
            fill_rule,
        } => {
            bytes.push(1);
            append_family_site_identity(bytes, *source_site_id);
            append_site_scope_identity(bytes, *scope);
            append_family_site_provenance_identity(bytes, provenance);
            append_fill_rule_identity(bytes, *fill_rule);
            bytes.extend(center.x.to_bits().to_le_bytes());
            bytes.extend(center.y.to_bits().to_le_bytes());
            bytes.extend(radius.to_bits().to_le_bytes());
        }
        CanonicalMark::ClosedPath(mark) => {
            bytes.push(2);
            append_family_site_identity(bytes, mark.source_site_id);
            append_site_scope_identity(bytes, mark.scope);
            append_family_site_provenance_identity(bytes, &mark.provenance);
            append_fill_rule_identity(bytes, mark.fill_rule);
            append_curve_path_identity(bytes, &mark.path);
        }
    }
}

/// Appends the stable evaluator-emission identifier of one canonical mark.
fn append_family_site_identity(bytes: &mut Vec<u8>, site_id: FamilySiteId) {
    bytes.extend(site_id.mechanism_id.0.to_le_bytes());
    bytes.extend(
        u64::try_from(site_id.ordinal)
            .expect("usize fits u64")
            .to_le_bytes(),
    );
}

/// Appends the final-canvas scope discriminator without changing structural provenance.
fn append_site_scope_identity(bytes: &mut Vec<u8>, scope: SiteScope) {
    bytes.push(match scope {
        SiteScope::Canvas => 1,
        SiteScope::Guard => 2,
    });
}

/// Appends the complete variant and payload of the family authority that emitted a site.
fn append_family_site_provenance_identity(bytes: &mut Vec<u8>, provenance: &FamilySiteProvenance) {
    match provenance {
        FamilySiteProvenance::GuideIntersection { contributors } => {
            bytes.push(1);
            append_guide_instances_identity(bytes, contributors);
        }
        FamilySiteProvenance::AlongGuide {
            guide_id,
            guide_order,
            sequence,
            absolute_arc_position_bits,
            local_arc_position_bits,
        } => {
            bytes.push(2);
            append_guide_instance_identity(bytes, *guide_id);
            bytes.extend(
                u64::try_from(*guide_order)
                    .expect("usize fits u64")
                    .to_le_bytes(),
            );
            bytes.extend(sequence.to_le_bytes());
            bytes.extend(absolute_arc_position_bits.to_le_bytes());
            bytes.extend(local_arc_position_bits.to_le_bytes());
        }
        FamilySiteProvenance::Random {
            candidate_ordinal,
            accepted_ordinal,
            exclusion_neighbor_ordinal,
        } => {
            bytes.push(3);
            for value in [candidate_ordinal, accepted_ordinal] {
                bytes.extend(u64::try_from(*value).expect("usize fits u64").to_le_bytes());
            }
            match exclusion_neighbor_ordinal {
                Some(value) => {
                    bytes.push(1);
                    bytes.extend(u64::try_from(*value).expect("usize fits u64").to_le_bytes());
                }
                None => bytes.push(0),
            }
        }
        FamilySiteProvenance::CurveGuideIntersection { contributors } => {
            bytes.push(4);
            bytes.extend(
                u64::try_from(contributors.len())
                    .expect("usize fits u64")
                    .to_le_bytes(),
            );
            for contributor in contributors {
                append_guide_path_location_identity(bytes, contributor);
            }
        }
        FamilySiteProvenance::CurveAlongGuide {
            location,
            guide_order,
            sequence,
            absolute_arc_position_bits,
            local_arc_position_bits,
        } => {
            bytes.push(5);
            append_guide_path_location_identity(bytes, location);
            bytes.extend(
                u64::try_from(*guide_order)
                    .expect("usize fits u64")
                    .to_le_bytes(),
            );
            bytes.extend(sequence.to_le_bytes());
            bytes.extend(absolute_arc_position_bits.to_le_bytes());
            bytes.extend(local_arc_position_bits.to_le_bytes());
        }
        FamilySiteProvenance::AlongParametricCurve {
            location,
            path_order,
            sequence,
            absolute_arc_position_bits,
            local_arc_position_bits,
        } => {
            bytes.push(6);
            append_guide_path_location_identity(bytes, location);
            bytes.extend(
                u64::try_from(*path_order)
                    .expect("usize fits u64")
                    .to_le_bytes(),
            );
            bytes.extend(sequence.to_le_bytes());
            bytes.extend(absolute_arc_position_bits.to_le_bytes());
            bytes.extend(local_arc_position_bits.to_le_bytes());
        }
    }
}

/// Appends an ordered straight-guide contributor sequence with a length delimiter.
fn append_guide_instances_identity(bytes: &mut Vec<u8>, contributors: &[GuideInstanceId]) {
    bytes.extend(
        u64::try_from(contributors.len())
            .expect("usize fits u64")
            .to_le_bytes(),
    );
    for contributor in contributors {
        append_guide_instance_identity(bytes, *contributor);
    }
}

/// Appends one exact dimension/index/component guide contributor.
fn append_guide_instance_identity(bytes: &mut Vec<u8>, guide_id: GuideInstanceId) {
    bytes.extend(guide_id.dimension_id.to_le_bytes());
    bytes.extend(guide_id.index.to_le_bytes());
    bytes.extend(guide_id.component_ordinal.to_le_bytes());
}

/// Appends one exact curve-guide contributor location.
fn append_guide_path_location_identity(
    bytes: &mut Vec<u8>,
    location: &StructuralPathLocationProvenance,
) {
    append_structural_path_instance_identity(bytes, location.path);
    bytes.extend(
        u64::try_from(location.segment_index)
            .expect("usize fits u64")
            .to_le_bytes(),
    );
    bytes.extend(location.parameter_bits.to_le_bytes());
}

/// Appends one stable path-neutral source/repetition/component identity for fingerprints.
fn append_structural_path_instance_identity(bytes: &mut Vec<u8>, path: StructuralPathInstanceId) {
    match path.source {
        StructuralPathSourceId::GuideDimension(id) => {
            bytes.push(1);
            bytes.extend(id.0.to_le_bytes());
        }
        StructuralPathSourceId::ParametricCurve(id) => {
            bytes.push(2);
            bytes.extend(id.0.to_le_bytes());
        }
    }
    bytes.extend(path.repetition_index.to_le_bytes());
    bytes.extend(path.component_ordinal.to_le_bytes());
}

/// Appends explicit even-odd fill semantics rather than relying on a renderer default.
fn append_fill_rule_identity(bytes: &mut Vec<u8>, fill_rule: CanonicalFillRule) {
    bytes.push(match fill_rule {
        CanonicalFillRule::EvenOdd => 1,
        CanonicalFillRule::NonZero => 2,
    });
}

/// Appends path closure, ordered segment kinds, and every construction-point bit exactly.
fn append_curve_path_identity(bytes: &mut Vec<u8>, path: &CurvePath) {
    bytes.push(match path.closure() {
        PathClosure::Open => 1,
        PathClosure::Closed => 2,
    });
    bytes.extend(
        u64::try_from(path.segments().len())
            .expect("usize fits u64")
            .to_le_bytes(),
    );
    for segment in path.segments() {
        match segment {
            CurveSegment::Line(line) => {
                bytes.push(1);
                for point in [line.start(), line.end()] {
                    bytes.extend(point.x.to_bits().to_le_bytes());
                    bytes.extend(point.y.to_bits().to_le_bytes());
                }
            }
            CurveSegment::CubicBezier(cubic) => {
                bytes.push(2);
                for point in [
                    cubic.start(),
                    cubic.control_1(),
                    cubic.control_2(),
                    cubic.end(),
                ] {
                    bytes.extend(point.x.to_bits().to_le_bytes());
                    bytes.extend(point.y.to_bits().to_le_bytes());
                }
            }
        }
    }
}

fn append_mark_identity(bytes: &mut Vec<u8>, mark: &CanonicalCircleMark) {
    bytes.extend(mark.source_site_id.first_dimension_id.to_le_bytes());
    bytes.extend(mark.source_site_id.first_index.to_le_bytes());
    bytes.extend(mark.source_site_id.second_dimension_id.to_le_bytes());
    bytes.extend(mark.source_site_id.second_index.to_le_bytes());
    bytes.extend(mark.center.x.to_bits().to_le_bytes());
    bytes.extend(mark.center.y.to_bits().to_le_bytes());
    bytes.extend(mark.radius.to_bits().to_le_bytes());
}

fn fnv1a64(bytes: impl IntoIterator<Item = u8>) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
mod random_prng_contract_tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn xorshift32_sequence_and_zero_seed_mapping_are_fixed() {
        let mut seeded = StablePrng::new(1);
        assert_eq!(seeded.next_u32(), 270_369);
        assert_eq!(seeded.next_u32(), 67_634_689);
        assert_eq!(seeded.next_u32(), 2_647_435_461);
        let mut zero_a = StablePrng::new(0);
        let mut zero_b = StablePrng::new(0);
        assert_eq!(zero_a.next_u32(), zero_b.next_u32());
        assert_ne!(zero_a.next_u32(), seeded.next_u32());
    }

    #[test]
    fn populated_spatial_cell_cancels_at_the_per_index_boundary() {
        let mut index = SpatialIndex::new(10.0, 100).unwrap();
        let accepted = vec![(Point2::new(1.0, 1.0), 0, SiteScope::Canvas)];
        index.insert(accepted[0].0, 0).unwrap();
        let polls = Cell::new(0_u32);
        let error = index
            .find_conflict(Point2::new(1.5, 1.5), &accepted, 10.0, &|| {
                let next = polls.get() + 1;
                polls.set(next);
                // Calls 1-5 are candidate-cell polling; call 6 is the first
                // populated-cell index. Without the per-index poll this would
                // return a collision instead of cancellation.
                next >= 6
            })
            .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
        assert_eq!(polls.get(), 6);
        assert_eq!(index.neighbor_work, 1);
    }

    #[test]
    fn smoothstep_response_has_fixed_basic_ieee_semantics() {
        assert_eq!(
            artwork_weight_response(-1.0, &ArtworkWeightResponse::Smoothstep),
            0.0
        );
        assert_eq!(
            artwork_weight_response(0.0, &ArtworkWeightResponse::Smoothstep),
            0.0
        );
        assert_eq!(
            artwork_weight_response(0.25, &ArtworkWeightResponse::Smoothstep),
            0.15625
        );
        assert_eq!(
            artwork_weight_response(0.5, &ArtworkWeightResponse::Smoothstep),
            0.5
        );
        assert_eq!(
            artwork_weight_response(1.0, &ArtworkWeightResponse::Smoothstep),
            1.0
        );
        assert_eq!(
            artwork_weight_response(2.0, &ArtworkWeightResponse::Smoothstep),
            1.0
        );
    }
}

#[cfg(test)]
mod realization_tests {
    use super::*;
    use toniator_sampling::{SourceFormatHint, decode_source};

    /// Builds a current-valid guarded grid fixture whose declared support covers normalized marks.
    ///
    /// # Panics
    ///
    /// Panics when the guarded fixture violates current grid preconditions.
    fn family() -> GridFamilyOutput {
        evaluate_straight_grid(&GridInspectRequest {
            canvas: CanvasSpec {
                width: 90.0,
                height: 60.0,
            },
            density: DensityMetric2D {
                across_x: 9.0,
                across_y: 6.0,
                aspect_locked: true,
            },
            rotation_degrees: 17.0,
            translation_x: 3.25,
            translation_y: -4.5,
            guard_steps: 2,
            support_radius: 10.0,
            max_family_candidates: 1_048_576,
        })
        .unwrap()
    }

    fn field() -> SourceField {
        let bytes = std::fs::read(format!(
            "{}/../../assets/raster-sample.png",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        decode_source(&bytes, SourceFormatHint::Png).unwrap()
    }

    fn asset_field(name: &str, hint: SourceFormatHint) -> SourceField {
        let bytes = std::fs::read(format!(
            "{}/../../assets/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        decode_source(&bytes, hint).unwrap()
    }

    /// Verifies alpha association controls canonical radii without exposing hidden RGB.
    ///
    /// # Panics
    ///
    /// Panics when canonical alpha association or nominal-cell normalization changes.
    #[test]
    fn png_alpha_associated_ink_reaches_canonical_radii_without_hidden_rgb_fringes() {
        let image = image::RgbaImage::from_raw(
            8,
            1,
            vec![
                0, 0, 0, 0, // transparent black
                255, 255, 255, 0, // transparent white
                255, 0, 0, 0, // transparent saturated red
                0, 0, 0, 255, // opaque black
                255, 255, 255, 255, // opaque white
                0, 0, 0, 0, // same black RGB, alpha 0
                0, 0, 0, 128, // same black RGB, alpha about 0.5
                0, 0, 0, 255, // same black RGB, alpha 1
            ],
        )
        .unwrap();
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let source = decode_source(&bytes, SourceFormatHint::Png).unwrap();
        let mut grid = family();
        let prototype = grid.sites[0].clone();
        grid.sites = (0..8)
            .map(|x| {
                let mut site = prototype.clone();
                site.position = Point2::new(f64::from(x), 0.0);
                site
            })
            .collect();
        let canvas = CanvasSpec {
            width: 7.0,
            height: 1.0,
        };
        let response = MarkResponse {
            minimum_fill: 0.2,
            maximum_fill: 0.9,
            rotation_offset_degrees: 0.0,
        };
        let radius = |ink| {
            radius_from_ink_with_diameter(ink, response, grid.sites[0].nominal_cell_diameter)
                .expect("fixture nominal cell basis is valid")
        };
        let luminance = realize_circular_marks(
            &grid,
            &source,
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            response,
        )
        .unwrap();
        let alpha = realize_circular_marks(
            &grid,
            &source,
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Alpha,
            response,
        )
        .unwrap();
        assert_eq!(
            luminance.marks[0].radius,
            radius(0.0),
            "transparent black is the minimum mark radius"
        );
        assert!(
            [1, 2, 5]
                .into_iter()
                .all(|index| luminance.marks[index].radius == luminance.marks[0].radius),
            "all zero-alpha hidden RGB variants map to minimum radius"
        );
        assert_eq!(luminance.marks[3].radius, radius(1.0));
        assert_eq!(luminance.marks[4].radius, radius(0.0));
        let half_alpha_radius = radius(128.0 / 255.0);
        assert!((luminance.marks[6].radius - half_alpha_radius).abs() < 1e-12);
        assert_eq!(luminance.marks[7].radius, radius(1.0));
        assert!(
            alpha.marks[5].radius > alpha.marks[6].radius
                && alpha.marks[6].radius > alpha.marks[7].radius,
            "Alpha response has one decreasing alpha polarity, without squaring"
        );
        assert!((alpha.marks[6].radius - radius(127.0 / 255.0)).abs() < 1e-12);
        assert_ne!(
            luminance.realization_fingerprint,
            alpha.realization_fingerprint
        );
    }

    /// Verifies normalized responses require an explicit nominal cell diameter.
    ///
    /// # Panics
    ///
    /// Panics when response validation or nominal-cell normalization changes.
    #[test]
    fn normalized_diameter_response_requires_an_explicit_nominal_cell() {
        let response = MarkResponse {
            minimum_fill: 0.2,
            maximum_fill: 0.9,
            rotation_offset_degrees: 0.0,
        };
        assert_eq!(
            radius_from_ink_with_diameter(0.0, response, 10.0).unwrap(),
            1.0
        );
        assert_eq!(
            radius_from_ink_with_diameter(0.5, response, 10.0).unwrap(),
            2.75
        );
        assert_eq!(
            radius_from_ink_with_diameter(1.0, response, 10.0).unwrap(),
            4.5
        );
        assert!(radius_from_ink_with_diameter(f64::NAN, response, 10.0).is_err());
        assert!(
            radius_from_ink_with_diameter(
                0.5,
                MarkResponse {
                    minimum_fill: -1.0,
                    maximum_fill: 0.9,
                    rotation_offset_degrees: 0.0,
                },
                10.0,
            )
            .is_err()
        );
        assert!(
            radius_from_ink_with_diameter(
                0.5,
                MarkResponse {
                    minimum_fill: 0.5,
                    maximum_fill: 2.1,
                    rotation_offset_degrees: 0.0,
                },
                10.0,
            )
            .is_err()
        );
    }

    /// Verifies size-only responses retain every planned canvas and guard site.
    ///
    /// # Panics
    ///
    /// Panics when current realization changes the evaluated family envelope.
    #[test]
    fn size_changes_reuse_every_site_and_keep_guards_without_clipping() {
        let family = family();
        let canvas = CanvasSpec {
            width: 90.0,
            height: 60.0,
        };
        let first = realize_circular_marks(
            &family,
            &field(),
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            MarkResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.9,
                rotation_offset_degrees: 0.0,
            },
        )
        .unwrap();
        let second = realize_circular_marks(
            &family,
            &field(),
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            MarkResponse {
                minimum_fill: 0.3,
                maximum_fill: 0.8,
                rotation_offset_degrees: 0.0,
            },
        )
        .unwrap();
        assert_eq!(first.family_fingerprint, family.family_fingerprint);
        assert_eq!(first.marks.len(), family.sites.len());
        assert!(
            first
                .marks
                .iter()
                .any(|mark| mark.scope == SiteScope::Guard)
        );
        for ((mark_a, mark_b), site) in first.marks.iter().zip(&second.marks).zip(&family.sites) {
            assert_eq!(mark_a.source_site_id, site.id);
            assert_eq!(mark_a.center, site.position);
            assert_eq!(mark_a.scope, site.scope);
            assert_eq!(mark_a.provenance, site.provenance);
            assert_eq!(mark_a.source_site_id, mark_b.source_site_id);
            assert_eq!(mark_a.center, mark_b.center);
        }
        assert!(
            first
                .marks
                .iter()
                .zip(&second.marks)
                .any(|(left, right)| left.radius != right.radius)
        );
        assert_ne!(
            first.realization_fingerprint,
            second.realization_fingerprint
        );
    }

    /// Proves a current-valid normalized response cannot exceed declared family support.
    ///
    /// # Panics
    ///
    /// Panics when declared support no longer governs direct realization.
    #[test]
    fn direct_realization_rejects_a_response_beyond_declared_family_support() {
        let mut family = family();
        family.support_radius = 4.5;
        let canvas = CanvasSpec {
            width: 90.0,
            height: 60.0,
        };
        let error = realize_circular_marks(
            &family,
            &field(),
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            MarkResponse {
                minimum_fill: 0.2,
                maximum_fill: 2.0,
                rotation_offset_degrees: 0.0,
            },
        )
        .expect_err("family support is authoritative at direct realization");
        assert_eq!(error.path(), "realization.family.support_radius");

        let mut wider_family = family;
        wider_family.support_radius = 20.0;
        let wider = realize_circular_marks(
            &wider_family,
            &field(),
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            MarkResponse {
                minimum_fill: 0.5,
                maximum_fill: 2.0,
                rotation_offset_degrees: 0.0,
            },
        )
        .expect("declared support permits current normalized fills through 2.0");
        assert!(
            wider
                .marks
                .iter()
                .zip(&wider_family.sites)
                .all(|(mark, site)| {
                    let minimum_radius = 0.5 * site.nominal_cell_diameter / 2.0;
                    let maximum_radius = site.nominal_cell_diameter;
                    mark.radius >= minimum_radius && mark.radius <= maximum_radius
                })
        );
    }

    /// Verifies canonical mark output excludes presentation-only inputs from identity.
    ///
    /// # Panics
    ///
    /// Panics when presentation-independent mark content or identity changes.
    #[test]
    fn canonical_marks_and_fingerprint_are_presentation_independent() {
        let family = family();
        let canvas = CanvasSpec {
            width: 90.0,
            height: 60.0,
        };
        let left = realize_circular_marks(
            &family,
            &field(),
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            MarkResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.9,
                rotation_offset_degrees: 0.0,
            },
        )
        .unwrap();
        // Color, opacity, and visibility have no realization inputs by design.
        let right = realize_circular_marks(
            &family,
            &field(),
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            MarkResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.9,
                rotation_offset_degrees: 0.0,
            },
        )
        .unwrap();
        assert_eq!(
            serde_json::to_vec(&left.marks).unwrap(),
            serde_json::to_vec(&right.marks).unwrap()
        );
        assert_eq!(left.realization_fingerprint, right.realization_fingerprint);
        assert!(left.has_only_finite_marks());
    }

    fn source_from_rgba(width: u32, rgba: Vec<u8>) -> SourceField {
        let image = image::RgbaImage::from_raw(width, 1, rgba).unwrap();
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        decode_source(&bytes, SourceFormatHint::Png).unwrap()
    }

    fn sites_at_positions(positions: &[f64]) -> GridFamilyOutput {
        let mut output = family();
        let prototype = output.sites[0].clone();
        output.sites = positions
            .iter()
            .map(|x| {
                let mut site = prototype.clone();
                site.position = Point2::new(*x, 0.0);
                site
            })
            .collect();
        output
    }

    /// Verifies mapped realization retains family identity while validating current response input.
    ///
    /// # Panics
    ///
    /// Panics when mapped realization accepts invalid current response input.
    #[test]
    fn mapped_realization_keeps_family_structural_and_validates_direct_inputs() {
        let family = sites_at_positions(&[0.0, 1.0]);
        let source = source_from_rgba(2, vec![255, 0, 0, 128, 0, 0, 0, 255]);
        let canvas = CanvasSpec {
            width: 1.0,
            height: 1.0,
        };
        let response = MarkResponse {
            minimum_fill: 0.2,
            maximum_fill: 0.9,
            rotation_offset_degrees: 0.0,
        };
        let mapping = SourceMapping::canonical(SourceMappingComponent::Red);
        let first =
            realize_mapped_circular_marks(&family, &source, &canvas, mapping, response).unwrap();
        let second =
            realize_mapped_circular_marks(&family, &source, &canvas, mapping, response).unwrap();
        assert_eq!(first.family_fingerprint, family.family_fingerprint);
        assert_eq!(
            first.realization_fingerprint,
            second.realization_fingerprint
        );
        assert_eq!(first.marks.len(), family.sites.len());
        assert!(first.marks[0].radius > first.marks[1].radius);
        assert_eq!(
            realize_mapped_circular_marks(
                &family,
                &source,
                &canvas,
                SourceMapping {
                    gain: f64::NAN,
                    ..mapping
                },
                response,
            )
            .unwrap_err()
            .path(),
            "sampling.mapping.gain"
        );
        assert_eq!(
            realize_mapped_circular_marks(
                &family,
                &source,
                &canvas,
                mapping,
                MarkResponse {
                    minimum_fill: 0.2,
                    maximum_fill: 2.1,
                    rotation_offset_degrees: 0.0,
                },
            )
            .unwrap_err()
            .path(),
            "realization.response"
        );
    }

    /// Verifies source-colored realization omits transparent samples and keeps paint opaque.
    ///
    /// # Panics
    ///
    /// Panics when transparent suppression, opaque paint, or nominal sizing changes.
    #[test]
    fn source_color_realization_suppresses_zero_alpha_and_keeps_positive_paint_opaque() {
        let family = sites_at_positions(&[0.0, 1.0, 2.0, 3.0]);
        let source = source_from_rgba(
            4,
            vec![
                255, 0, 0, 0, // hidden red must not leak a mark
                255, 0, 0, 255, // opaque red
                0, 255, 0, 128, // partial green
                0, 0, 255, 0, // hidden blue must not leak a mark
            ],
        );
        let canvas = CanvasSpec {
            width: 3.0,
            height: 1.0,
        };
        let response = MarkResponse {
            minimum_fill: 0.2,
            maximum_fill: 0.9,
            rotation_offset_degrees: 0.0,
        };
        let mapping = SourceMapping::canonical(SourceMappingComponent::Alpha);
        let realization =
            realize_source_color_circular_marks(&family, &source, &canvas, mapping, response)
                .unwrap();
        assert_eq!(
            realization.marks.len(),
            2,
            "exact-zero alpha omits marks despite minimum size"
        );
        let opaque = &realization.marks[0];
        let partial = &realization.marks[1];
        assert_eq!(
            opaque.paint,
            SampledSourcePaint {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0
            }
        );
        assert_eq!(
            partial.paint,
            SampledSourcePaint {
                red: 0.0,
                green: 1.0,
                blue: 0.0,
                alpha: 1.0
            }
        );
        let nominal_cell_diameter = family.sites[0].nominal_cell_diameter;
        assert_eq!(
            opaque.mark.radius,
            radius_from_ink_with_diameter(1.0, response, nominal_cell_diameter).unwrap()
        );
        assert!(
            (partial.mark.radius
                - radius_from_ink_with_diameter(128.0 / 255.0, response, nominal_cell_diameter)
                    .unwrap())
            .abs()
                < 1e-12
        );
        assert!(realization.has_only_finite_marks());

        let repeated =
            realize_source_color_circular_marks(&family, &source, &canvas, mapping, response)
                .unwrap();
        assert_eq!(
            realization, repeated,
            "sampled paint is immutable realization content"
        );
        let changed_source = source_from_rgba(
            4,
            vec![255, 0, 0, 0, 255, 0, 0, 255, 0, 0, 255, 128, 0, 0, 255, 0],
        );
        let changed = realize_source_color_circular_marks(
            &family,
            &changed_source,
            &canvas,
            mapping,
            response,
        )
        .unwrap();
        assert_ne!(
            realization.realization_fingerprint,
            changed.realization_fingerprint
        );
        assert_ne!(realization.marks[1].paint, changed.marks[1].paint);
    }

    /// Verifies paint payload independently contributes to source-colored realization identity.
    ///
    /// # Panics
    ///
    /// Panics when sampled paint is omitted from source-color identity.
    #[test]
    fn sampled_paint_alone_changes_the_source_color_realization_identity() {
        let family = sites_at_positions(&[0.0]);
        let source = source_from_rgba(1, vec![255, 0, 0, 255]);
        let canvas = CanvasSpec {
            width: 1.0,
            height: 1.0,
        };
        let mapping = SourceMapping::canonical(SourceMappingComponent::Alpha);
        let response = MarkResponse {
            minimum_fill: 0.2,
            maximum_fill: 0.9,
            rotation_offset_degrees: 0.0,
        };
        let realization =
            realize_source_color_circular_marks(&family, &source, &canvas, mapping, response)
                .unwrap();
        let mut changed_paint_only = realization.marks.clone();
        changed_paint_only[0].paint.blue = 0.25;
        let original = source_color_realization_fingerprint(
            &family,
            &source,
            mapping,
            response,
            &realization.marks,
        );
        let changed = source_color_realization_fingerprint(
            &family,
            &source,
            mapping,
            response,
            &changed_paint_only,
        );
        assert_eq!(original, realization.realization_fingerprint);
        assert_ne!(
            original, changed,
            "sampled paint is an identity input on its own"
        );
    }

    /// Verifies interpolation unassociates sampled color before opaque mark construction.
    ///
    /// # Panics
    ///
    /// Panics when interpolation leaks hidden RGB or changes normalized radius.
    #[test]
    fn source_color_unassociates_after_interpolation_without_a_hidden_rgb_fringe() {
        let family = sites_at_positions(&[0.5]);
        let source = source_from_rgba(2, vec![255, 0, 0, 255, 0, 255, 0, 0]);
        let canvas = CanvasSpec {
            width: 1.0,
            height: 1.0,
        };
        let realization = realize_source_color_circular_marks(
            &family,
            &source,
            &canvas,
            SourceMapping::canonical(SourceMappingComponent::Alpha),
            MarkResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.9,
                rotation_offset_degrees: 0.0,
            },
        )
        .unwrap();
        assert_eq!(realization.marks.len(), 1);
        assert_eq!(
            realization.marks[0].paint,
            SampledSourcePaint {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0
            }
        );
        assert_eq!(
            realization.marks[0].mark.radius,
            radius_from_ink_with_diameter(
                0.5,
                MarkResponse {
                    minimum_fill: 0.2,
                    maximum_fill: 0.9,
                    rotation_offset_degrees: 0.0,
                },
                family.sites[0].nominal_cell_diameter,
            )
            .unwrap()
        );
    }

    /// Verifies inverted mapped luminance and alpha preserve current canonical mark geometry.
    ///
    /// # Panics
    ///
    /// Panics when the mapped and component realization paths diverge.
    #[test]
    fn inverted_luminance_and_alpha_mappings_retain_canonical_mark_geometry() {
        let family = sites_at_positions(&[0.0, 1.0]);
        let source = source_from_rgba(2, vec![255, 0, 0, 128, 255, 255, 255, 64]);
        let canvas = CanvasSpec {
            width: 1.0,
            height: 1.0,
        };
        let response = MarkResponse {
            minimum_fill: 0.2,
            maximum_fill: 0.9,
            rotation_offset_degrees: 0.0,
        };
        for (legacy_component, mapping_component) in [
            (
                SourceComponent::Luminance,
                SourceMappingComponent::Luminance,
            ),
            (SourceComponent::Alpha, SourceMappingComponent::Alpha),
        ] {
            let legacy = realize_circular_marks(
                &family,
                &source,
                &canvas,
                SourcePlacement::StretchToCanvas,
                legacy_component,
                response,
            )
            .unwrap();
            let legacy_repeat = realize_circular_marks(
                &family,
                &source,
                &canvas,
                SourcePlacement::StretchToCanvas,
                legacy_component,
                response,
            )
            .unwrap();
            let mapped = realize_mapped_circular_marks(
                &family,
                &source,
                &canvas,
                SourceMapping {
                    component: mapping_component,
                    placement: SourcePlacement::StretchToCanvas,
                    inverted: true,
                    gain: 1.0,
                    bias: 0.0,
                },
                response,
            )
            .unwrap();
            assert_eq!(legacy.marks, mapped.marks);
            assert_eq!(
                legacy.realization_fingerprint,
                legacy_repeat.realization_fingerprint
            );
            assert_ne!(
                legacy.realization_fingerprint,
                mapped.realization_fingerprint
            );
        }
    }

    /// Exercises both immutable baseline sources through current mapped realization APIs.
    ///
    /// # Panics
    ///
    /// Panics when either immutable source or current mapped realization contract changes.
    #[test]
    fn stage9_realizations_exercise_both_immutable_baseline_sources() {
        let family = family();
        let canvas = CanvasSpec {
            width: 90.0,
            height: 60.0,
        };
        let response = MarkResponse {
            minimum_fill: 0.2,
            maximum_fill: 0.9,
            rotation_offset_degrees: 0.0,
        };
        for (name, hint, expected_hash) in [
            (
                "raster-sample.png",
                SourceFormatHint::Png,
                "sha256:324ac232e319002a13fbcfac46538ca5d7e8ba8a127eea2eaf20e8ddb3ed2ef2",
            ),
            (
                "vector-sample.svg",
                SourceFormatHint::Svg,
                "sha256:42eb5e23111a5dbad66f2b1802a7cc06391c7ede829b99eb28aeb1ac91596e2e",
            ),
        ] {
            let source = asset_field(name, hint);
            assert_eq!(source.identity().content_hash, expected_hash);
            let mapped = realize_mapped_circular_marks(
                &family,
                &source,
                &canvas,
                SourceMapping::canonical(SourceMappingComponent::Luminance),
                response,
            )
            .unwrap();
            let source_color = realize_source_color_circular_marks(
                &family,
                &source,
                &canvas,
                SourceMapping::canonical(SourceMappingComponent::Alpha),
                response,
            )
            .unwrap();
            assert_eq!(mapped.family_fingerprint, family.family_fingerprint);
            assert!(mapped.has_only_finite_marks());
            assert!(source_color.has_only_finite_marks());
            assert!(source_color.marks.len() <= family.sites.len());
        }
    }
}

impl GridFamilyOutput {
    pub fn has_only_finite_geometry(&self) -> bool {
        self.generation_domain.min.is_finite()
            && self.generation_domain.max.is_finite()
            && self.guides.iter().all(|guide| {
                guide.normal.x.is_finite()
                    && guide.normal.y.is_finite()
                    && guide.tangent.x.is_finite()
                    && guide.tangent.y.is_finite()
                    && guide.offset.is_finite()
                    && guide.anchor.is_finite()
                    && guide.start.is_finite()
                    && guide.end.is_finite()
            })
            && self.sites.iter().all(|site| site.position.is_finite())
    }
}

/// A schema-scoped failure before geometric generation begins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridError {
    path: &'static str,
    message: &'static str,
}

impl GridError {
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

impl fmt::Display for GridError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for GridError {}

/// Evaluates two stable-ID, straight dimensions and all of their intersections.
///
/// The canvas contributes only the padded local extent. It never contributes a
/// guide, a site, or topology. Returned lines are finite presentations of
/// infinite guides and deliberately extend beyond that planned local extent.
pub fn evaluate_straight_grid(request: &GridInspectRequest) -> Result<GridFamilyOutput, GridError> {
    evaluate_straight_grid_cancellable(request, &|| false)
}

/// The cancellation-aware structural planner used by the generic evaluator.
/// It retains the exact accepted output when the probe never cancels.
pub fn evaluate_straight_grid_cancellable(
    request: &GridInspectRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<GridFamilyOutput, GridError> {
    if is_cancelled() {
        return Err(GridError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ));
    }
    validate(request)?;

    let spacing_x = directional_spacing(&request.canvas, &request.density, Vector2::new(1.0, 0.0))?;
    let spacing_y = directional_spacing(&request.canvas, &request.density, Vector2::new(0.0, 1.0))?;
    let transform = AffineTransform2D::rotate_about_then_translate(
        Point2::new(request.canvas.width / 2.0, request.canvas.height / 2.0),
        request.rotation_degrees,
        Vector2::new(request.translation_x, request.translation_y),
    )
    .ok_or(GridError::new(
        "channel.pattern.layout",
        "transform is not finite",
    ))?;

    let document_canvas = Bounds::new(
        Point2::new(0.0, 0.0),
        Point2::new(request.canvas.width, request.canvas.height),
    )
    .expect("validated canvas creates finite bounds");
    let planning_margin = request.support_radius
        + ANTIALIAS_MARGIN
        + f64::from(request.guard_steps) * spacing_x.max(spacing_y);
    let padded_document_canvas = document_canvas
        .expanded(planning_margin)
        .expect("validated finite margin expands finite canvas");
    let generation_domain =
        transform
            .inverse_bounds(padded_document_canvas)
            .ok_or(GridError::new(
                "coverage",
                "inverse transform produced non-finite bounds",
            ))?;

    let dimensions = [
        DimensionPlan::new(FIRST_DIMENSION_ID, Vector2::new(1.0, 0.0), spacing_x, 0.0),
        DimensionPlan::new(SECOND_DIMENSION_ID, Vector2::new(0.0, 1.0), spacing_y, 0.0),
    ];
    let plans = [
        dimensions[0].coverage(generation_domain, transform, request)?,
        dimensions[1].coverage(generation_domain, transform, request)?,
    ];
    if is_cancelled() {
        return Err(GridError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ));
    }
    let first_count = guide_range_count(plans[0])?;
    let second_count = guide_range_count(plans[1])?;
    let candidate_count = first_count.checked_mul(second_count).ok_or(GridError::new(
        "coverage.candidate_limit",
        "guide-range candidate count overflowed",
    ))?;
    if candidate_count > request.max_family_candidates {
        return Err(GridError::new(
            "coverage.candidate_limit",
            "guide-range candidate count exceeds the configured limit",
        ));
    }

    let extension = planning_margin;
    let mut guides = Vec::new();
    for (dimension, coverage) in dimensions.iter().zip(plans.iter()) {
        if is_cancelled() {
            return Err(GridError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        guides.extend(dimension.guides(*coverage, generation_domain, transform, extension));
    }

    let mut sites = Vec::new();
    for first_index in plans[0].first_index..=plans[0].last_index {
        if is_cancelled() {
            return Err(GridError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        for second_index in plans[1].first_index..=plans[1].last_index {
            let local = Point2::new(
                first_index as f64 * dimensions[0].spacing,
                second_index as f64 * dimensions[1].spacing,
            );
            let position = transform.apply_point(local);
            // Coverage enumeration intentionally remains conservative in
            // inverse-local space. Publish only intersections whose world-space
            // Euclidean distance to the axis-aligned canvas is within the same
            // support/AA/guard envelope used to plan that enumeration. This
            // keeps required corner coverage while excluding Cartesian-product
            // corners that cannot affect final marks or clipping.
            if distance_to_canvas(position, document_canvas) > planning_margin {
                continue;
            }
            let first = GuideInstanceId::new(FIRST_DIMENSION_ID, first_index);
            let second = GuideInstanceId::new(SECOND_DIMENSION_ID, second_index);
            sites.push(IntersectionSite {
                id: SiteId {
                    first_dimension_id: FIRST_DIMENSION_ID.0,
                    first_index,
                    second_dimension_id: SECOND_DIMENSION_ID.0,
                    second_index,
                },
                position,
                nominal_cell_diameter: spacing_x.hypot(spacing_y),
                scope: if document_canvas.contains(position) {
                    SiteScope::Canvas
                } else {
                    SiteScope::Guard
                },
                provenance: GuideIntersectionProvenance {
                    contributors: vec![first, second],
                },
            });
        }
    }

    let output = GridFamilyOutput {
        family_fingerprint: fingerprint(request, spacing_x, spacing_y),
        guard_steps: request.guard_steps,
        support_radius: request.support_radius,
        antialias_margin: ANTIALIAS_MARGIN,
        generation_domain,
        coverage: plans.to_vec(),
        guides,
        sites,
    };
    if output.has_only_finite_geometry() {
        Ok(output)
    } else {
        Err(GridError::new(
            "coverage",
            "generation produced non-finite geometry",
        ))
    }
}

#[derive(Clone, Copy, Debug)]
struct DimensionPlan {
    id: GuideDimensionId,
    normal: Vector2,
    tangent: Vector2,
    spacing: f64,
    phase: f64,
}

impl DimensionPlan {
    fn new(id: GuideDimensionId, normal: Vector2, spacing: f64, phase: f64) -> Self {
        Self {
            id,
            normal,
            tangent: normal.perpendicular(),
            spacing,
            phase,
        }
    }

    fn coverage(
        self,
        domain: Bounds,
        transform: AffineTransform2D,
        request: &GridInspectRequest,
    ) -> Result<GuideCoverage, GridError> {
        let (minimum, maximum) = projection_range(domain.corners(), self.normal)
            .ok_or(GridError::new("coverage", "could not project local domain"))?;
        let first_index = checked_index(((minimum - self.phase) / self.spacing).floor())?;
        let last_index = checked_index(((maximum - self.phase) / self.spacing).ceil())?;
        let document_normal = transform.apply_vector(self.normal);
        let translated_phase = request
            .translation_x
            .mul_add(document_normal.x, request.translation_y * document_normal.y);
        Ok(GuideCoverage {
            dimension_id: self.id.0,
            spacing: self.spacing,
            normalized_phase: (self.phase + translated_phase).rem_euclid(self.spacing),
            first_index,
            last_index,
        })
    }

    fn guides(
        self,
        coverage: GuideCoverage,
        domain: Bounds,
        transform: AffineTransform2D,
        extension: f64,
    ) -> Vec<StraightGuide> {
        let (minimum_tangent, maximum_tangent) =
            projection_range(domain.corners(), self.tangent).expect("finite domain projects");
        (coverage.first_index..=coverage.last_index)
            .map(|index| {
                let offset = index as f64 * self.spacing + self.phase;
                let start_local = point_on_line(
                    self.normal,
                    self.tangent,
                    offset,
                    minimum_tangent - extension,
                );
                let end_local = point_on_line(
                    self.normal,
                    self.tangent,
                    offset,
                    maximum_tangent + extension,
                );
                StraightGuide {
                    id: GuideInstanceId::new(self.id, index),
                    normal: transform.apply_vector(self.normal),
                    tangent: transform.apply_vector(self.tangent),
                    offset,
                    anchor: transform.apply_point(point_on_line(
                        self.normal,
                        self.tangent,
                        offset,
                        0.0,
                    )),
                    start: transform.apply_point(start_local),
                    end: transform.apply_point(end_local),
                }
            })
            .collect()
    }
}

fn point_on_line(
    normal: Vector2,
    tangent: Vector2,
    normal_offset: f64,
    tangent_offset: f64,
) -> Point2 {
    Point2::new(
        normal.x.mul_add(normal_offset, tangent.x * tangent_offset),
        normal.y.mul_add(normal_offset, tangent.y * tangent_offset),
    )
}

fn checked_index(value: f64) -> Result<i64, GridError> {
    if !value.is_finite() || value < i64::MIN as f64 || value > i64::MAX as f64 {
        return Err(GridError::new(
            "coverage",
            "guide index is outside the supported range",
        ));
    }
    Ok(value as i64)
}

fn guide_range_count(coverage: GuideCoverage) -> Result<usize, GridError> {
    let span = coverage
        .last_index
        .checked_sub(coverage.first_index)
        .and_then(|value| value.checked_add(1))
        .ok_or(GridError::new(
            "coverage.candidate_limit",
            "guide-range candidate count overflowed",
        ))?;
    usize::try_from(span).map_err(|_| {
        GridError::new(
            "coverage.candidate_limit",
            "guide-range candidate count overflowed",
        )
    })
}

fn validate(request: &GridInspectRequest) -> Result<(), GridError> {
    validate_positive(request.canvas.width, "canvas.width")?;
    validate_positive(request.canvas.height, "canvas.height")?;
    validate_positive(
        request.density.across_x,
        "channel.pattern.layout.density.across_x",
    )?;
    validate_positive(
        request.density.across_y,
        "channel.pattern.layout.density.across_y",
    )?;
    validate_finite(
        request.rotation_degrees,
        "channel.pattern.layout.rotation_degrees",
    )?;
    validate_finite(
        request.translation_x,
        "channel.pattern.layout.translation_x",
    )?;
    validate_finite(
        request.translation_y,
        "channel.pattern.layout.translation_y",
    )?;
    validate_finite(request.support_radius, "coverage.support_radius")?;
    if request.support_radius < 0.0 {
        return Err(GridError::new(
            "coverage.support_radius",
            "value must not be negative",
        ));
    }
    if request.max_family_candidates == 0 {
        return Err(GridError::new(
            "coverage.candidate_limit",
            "configured candidate limit must be nonzero",
        ));
    }
    Ok(())
}

fn validate_finite(value: f64, path: &'static str) -> Result<(), GridError> {
    value
        .is_finite()
        .then_some(())
        .ok_or(GridError::new(path, "value must be finite"))
}

fn validate_positive(value: f64, path: &'static str) -> Result<(), GridError> {
    validate_finite(value, path)?;
    (value > 0.0)
        .then_some(())
        .ok_or(GridError::new(path, "value must be greater than zero"))
}

/// Resolves the guide spacing from the documented directional-frequency metric.
pub fn directional_spacing(
    canvas: &CanvasSpec,
    density: &DensityMetric2D,
    unit_normal: Vector2,
) -> Result<f64, GridError> {
    validate_positive(canvas.width, "canvas.width")?;
    validate_positive(canvas.height, "canvas.height")?;
    validate_positive(density.across_x, "channel.pattern.layout.density.across_x")?;
    validate_positive(density.across_y, "channel.pattern.layout.density.across_y")?;
    let spacing_x = canvas.width / density.across_x;
    let spacing_y = canvas.height / density.across_y;
    let frequency = (unit_normal.x / spacing_x).hypot(unit_normal.y / spacing_y);
    validate_positive(frequency, "density.directional_frequency")?;
    Ok(frequency.recip())
}

fn distance_to_canvas(point: Point2, canvas: Bounds) -> f64 {
    let dx = if point.x < canvas.min.x {
        canvas.min.x - point.x
    } else if point.x > canvas.max.x {
        point.x - canvas.max.x
    } else {
        0.0
    };
    let dy = if point.y < canvas.min.y {
        canvas.min.y - point.y
    } else if point.y > canvas.max.y {
        point.y - canvas.max.y
    } else {
        0.0
    };
    dx.hypot(dy)
}

fn fingerprint(request: &GridInspectRequest, spacing_x: f64, spacing_y: f64) -> String {
    let values = [
        request.canvas.width,
        request.canvas.height,
        request.density.across_x,
        request.density.across_y,
        request.rotation_degrees,
        request.translation_x,
        request.translation_y,
        request.support_radius,
        spacing_x,
        spacing_y,
    ];
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in b"toniator-stage-3-straight-grid-v2-world-support-envelope"
        .iter()
        .copied()
        .chain(request.guard_steps.to_le_bytes())
        .chain(FIRST_DIMENSION_ID.0.to_le_bytes())
        .chain(SECOND_DIMENSION_ID.0.to_le_bytes())
        .chain(
            values
                .into_iter()
                .flat_map(|value| value.to_bits().to_le_bytes()),
        )
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    /// Proves outer normal-offset coverage requires authored endpoint span and geometric side bracketing.
    #[test]
    fn normal_offset_outer_components_must_bracket_the_generation_domain() {
        let source = CurvePath::line(Point2::new(2.0, 5.0), Point2::new(8.0, 5.0)).unwrap();
        let domain = Bounds::new(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0)).unwrap();
        let component =
            |ordinal, start, end, first_x, last_x, y| toniator_geometry::OffsetPathComponent {
                component_ordinal: ordinal,
                source_start: PathLocation::new(0, start).unwrap(),
                source_end: PathLocation::new(0, end).unwrap(),
                path: CurvePath::line(Point2::new(first_x, y), Point2::new(last_x, y)).unwrap(),
            };
        assert!(
            !normal_offset_components_bracket_domain(
                &[component(0, 0.4, 0.6, 0.0, 10.0, 11.0)],
                &source,
                domain,
                1.0,
            )
            .unwrap()
        );
        assert!(
            !normal_offset_components_bracket_domain(
                &[component(0, 0.0, 1.0, 0.0, 10.0, 9.0)],
                &source,
                domain,
                1.0,
            )
            .unwrap()
        );
        assert!(
            normal_offset_components_bracket_domain(
                &[component(0, 0.0, 1.0, 0.0, 10.0, 11.0)],
                &source,
                domain,
                1.0,
            )
            .unwrap()
        );
        assert!(
            normal_offset_components_bracket_domain(
                &[component(0, 0.0, 1.0, 0.0, 10.0, -1.0)],
                &source,
                domain,
                -1.0,
            )
            .unwrap()
        );
        assert!(
            normal_offset_components_bracket_domain(
                &[
                    component(0, 0.0, 0.4, 0.0, 4.0, 11.0),
                    component(1, 0.6, 1.0, 6.0, 10.0, 11.0),
                ],
                &source,
                domain,
                1.0,
            )
            .unwrap()
        );
        assert!(
            normal_offset_components_bracket_domain(
                &[
                    component(0, 0.0, 0.4, 0.0, 5.0, 11.0),
                    component(1, 0.6, 1.0, 5.0, 10.0, 11.0),
                ],
                &source,
                domain,
                1.0,
            )
            .unwrap()
        );
        assert!(
            !normal_offset_components_bracket_domain(
                &[
                    component(0, 0.0, 0.7, 0.0, 7.0, 11.0),
                    component(1, 0.6, 1.0, 6.0, 10.0, 11.0),
                ],
                &source,
                domain,
                1.0,
            )
            .unwrap(),
            "overlapping source intervals are not authoritative cusp gaps"
        );
        assert!(
            !normal_offset_components_bracket_domain(
                &[
                    component(1, 0.6, 1.0, 6.0, 10.0, 11.0),
                    component(0, 0.0, 0.4, 0.0, 4.0, 11.0),
                ],
                &source,
                domain,
                1.0,
            )
            .unwrap(),
            "source-disordered components are rejected rather than sorted"
        );
        assert!(
            !normal_offset_components_bracket_domain(
                &[component(0, 0.7, 0.3, 0.0, 10.0, 11.0)],
                &source,
                domain,
                1.0,
            )
            .unwrap(),
            "reversed source intervals are rejected"
        );
        assert!(
            !normal_offset_components_bracket_domain(
                &[
                    component(0, 0.0, 0.4, 1.0, 4.0, 9.0),
                    component(1, 0.6, 1.0, 6.0, 9.0, 9.0),
                ],
                &source,
                domain,
                1.0,
            )
            .unwrap(),
            "ordered gaps still require complete requested-side projection"
        );
    }

    #[test]
    fn candidate_range_multiplication_reports_overflow_at_the_stable_limit_path() {
        let coverage = GuideCoverage {
            dimension_id: 1,
            spacing: 1.0,
            normalized_phase: 0.0,
            first_index: i64::MIN,
            last_index: i64::MAX,
        };
        assert_eq!(
            guide_range_count(coverage)
                .expect_err("span cannot fit i64")
                .path(),
            "coverage.candidate_limit"
        );
    }

    /// Proves nominal intersection bases reject parallel and near-parallel contributor tangents.
    #[test]
    fn straight_intersection_basis_rejects_parallel_contributors() {
        let first = GuideInstanceId::new(GuideDimensionId(1), 0);
        let second = GuideInstanceId::new(GuideDimensionId(2), 0);
        let guides = vec![
            StraightGuide {
                id: first,
                normal: Vector2::new(0.0, 1.0),
                tangent: Vector2::new(1.0, 0.0),
                offset: 0.0,
                anchor: Point2::new(0.0, 0.0),
                start: Point2::new(0.0, 0.0),
                end: Point2::new(1.0, 0.0),
            },
            StraightGuide {
                id: second,
                normal: Vector2::new(0.0, 1.0),
                tangent: Vector2::new(1.0, 1.0e-13),
                offset: 1.0,
                anchor: Point2::new(0.0, 1.0),
                start: Point2::new(0.0, 1.0),
                end: Point2::new(1.0, 1.0),
            },
        ];
        let coverage = vec![
            GuideCoverage {
                dimension_id: 1,
                spacing: 1.0,
                normalized_phase: 0.0,
                first_index: 0,
                last_index: 0,
            },
            GuideCoverage {
                dimension_id: 2,
                spacing: 1.0,
                normalized_phase: 0.0,
                first_index: 0,
                last_index: 0,
            },
        ];

        let error = straight_intersection_basis(&guides, &coverage, &[first, second])
            .expect_err("near-parallel contributors do not define a nominal cell");
        assert_eq!(error.path(), "pattern.family.nominal_cell_basis");
    }
}

#[cfg(test)]
mod typed_pipeline_tests {
    use std::cell::Cell;

    use super::*;
    use toniator_domain::{
        CoveragePolicy, PatternDefinitionId, PatternMechanismId, PatternOutputLayerId,
        SourceMappingComponent,
    };
    use toniator_sampling::{SourceFormatHint, decode_source};

    fn definition() -> PatternDefinition {
        PatternDefinition::supported_straight_grid(
            PatternDefinitionId(7),
            "presentation-only-name-is-not-dispatch",
            PatternMechanismId(11),
            PatternMechanismId(12),
            PatternOutputLayerId(13),
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        )
    }

    /// Builds the typed-grid fixture with support valid for its normalized response bounds.
    fn request() -> GridInspectRequest {
        GridInspectRequest {
            canvas: CanvasSpec {
                width: 90.0,
                height: 60.0,
            },
            density: DensityMetric2D {
                across_x: 9.0,
                across_y: 6.0,
                aspect_locked: true,
            },
            rotation_degrees: 17.0,
            translation_x: 3.25,
            translation_y: -4.5,
            guard_steps: 2,
            support_radius: 10.0,
            max_family_candidates: 100_000,
        }
    }

    fn source() -> SourceField {
        let image = image::RgbaImage::from_raw(1, 1, vec![255, 0, 0, 255]).unwrap();
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        decode_source(&bytes, SourceFormatHint::Png).unwrap()
    }

    /// Verifies typed capabilities retain ordered provenance and exact current family identity.
    ///
    /// # Panics
    ///
    /// Panics when typed capability identity or provenance changes.
    #[test]
    fn typed_capability_plan_preserves_order_provenance_and_accepted_geometry() {
        let definition = definition();
        let plan = resolve_pattern_pipeline(&definition).unwrap();
        assert_eq!(plan.family.provenance.definition_id, 7);
        assert_eq!(
            plan.family.provenance.mechanism_ids,
            vec![PatternMechanismId(11), PatternMechanismId(12)]
        );
        assert_eq!(plan.ordered_outputs[0].layer_id, PatternOutputLayerId(13));

        let expected = evaluate_straight_grid(&request()).unwrap();
        let generic = evaluate_typed_family(&definition, &request()).unwrap();
        assert_eq!(
            generic.family_fingerprint(),
            "fnv1a64:392c46103de1f1ab:nominal-cell-basis:fnv1a64:ed9b878e62f6850b"
        );
        assert_eq!(generic.site_set().len(), expected.sites.len());
        assert_eq!(
            generic.site_set().sites()[0].position,
            expected.sites[0].position
        );

        let realization = realize_typed_mapped_outputs(
            &generic,
            &plan,
            &source(),
            &request().canvas,
            SourceMapping::canonical(SourceMappingComponent::Red),
            MarkResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.9,
                rotation_offset_degrees: 0.0,
            },
        )
        .unwrap();
        assert_eq!(realization.provenance.structural.definition_id, 7);
        assert_eq!(
            realization.provenance.structural.mechanism_ids,
            vec![PatternMechanismId(11), PatternMechanismId(12)]
        );
        assert_eq!(
            realization.provenance.ordered_output_layer_ids,
            vec![PatternOutputLayerId(13)]
        );
    }

    #[test]
    fn incompatible_output_is_a_stable_preflight_error_without_family_output() {
        let mut definition = definition();
        definition.output_layers.clear();
        let error = evaluate_typed_family(&definition, &request()).unwrap_err();
        assert_eq!(error.path(), "pattern.output_layers.capability");
        assert_eq!(
            error.message(),
            "the current typed output contract requires exactly one ordered realization layer"
        );
    }

    #[test]
    fn bounded_structural_planning_observes_cancellation_before_final_clipping() {
        let checks = Cell::new(0_u32);
        let error = evaluate_typed_family_cancellable(&definition(), &request(), &|| {
            let next = checks.get() + 1;
            checks.set(next);
            next >= 5
        })
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
        assert!(checks.get() >= 5);
    }
}

#[cfg(test)]
mod generalized_straight_guide_tests {
    use super::*;
    use toniator_domain::{
        CoveragePolicy, GeneralizedSiteProduct, MarkOrientation, PatternDefinitionId,
        PatternMechanismId, PatternOutputLayerId, StraightGuideRepetition,
    };
    use toniator_sampling::{SourceFormatHint, decode_source};

    fn dimension(id: u64, angle: f64) -> StraightGuideDimension {
        StraightGuideDimension {
            id: GuideDimensionId(id),
            baseline_angle_degrees: angle,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        }
    }
    /// Builds the generalized-guide fixture with support valid for its normalized response bounds.
    fn request() -> StraightGuideInspectRequest {
        StraightGuideInspectRequest {
            canvas: CanvasSpec {
                width: 120.0,
                height: 80.0,
            },
            density: DensityMetric2D {
                across_x: 12.0,
                across_y: 8.0,
                aspect_locked: true,
            },
            rotation_degrees: 17.0,
            translation_x: 3.25,
            translation_y: -4.5,
            guard_steps: 2,
            support_radius: 10.0,
            max_family_candidates: 100_000,
        }
    }
    fn intersections(
        dimensions: Vec<StraightGuideDimension>,
        selected: Vec<GuideDimensionId>,
    ) -> PatternDefinition {
        PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(7),
            "not-a-name-dispatch",
            PatternMechanismId(11),
            PatternMechanismId(12),
            PatternOutputLayerId(13),
            dimensions,
            GeneralizedSiteProduct::Intersections {
                dimensions: selected,
                merge_epsilon: 1e-9,
            },
            MarkOrientation::GuideTangent {
                dimension_id: GuideDimensionId(1),
            },
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        )
    }

    #[test]
    fn one_through_four_dimensions_validate_with_explicit_ordered_intersection_provenance() {
        for count in 1..=4 {
            let dimensions: Vec<_> = [0.0, 47.0, 90.0, 137.0]
                .into_iter()
                .take(count)
                .enumerate()
                .map(|(index, angle)| dimension((index + 1) as u64, angle))
                .collect();
            let selection: Vec<_> = dimensions
                .iter()
                .take(2.min(count))
                .map(|dimension| dimension.id)
                .collect();
            let definition = intersections(dimensions, selection);
            if count == 1 {
                assert!(toniator_domain::validate_pattern_definition(&definition).is_err());
                continue;
            }
            toniator_domain::validate_pattern_definition(&definition).unwrap();
            let plan = resolve_pattern_pipeline(&definition).unwrap();
            let output =
                evaluate_generalized_straight_guides_cancellable(&plan.family, &request(), &|| {
                    false
                })
                .unwrap();
            assert!(output.guides.windows(2).all(|pair| pair[0].id <= pair[1].id
                || pair[0].id.dimension_id != pair[1].id.dimension_id));
            assert!(output.sites.iter().all(|site| matches!(&site.provenance, GeneralizedSiteProvenance::Intersection { contributors } if contributors.len() >= 2)));
        }
    }

    #[test]
    fn multiway_intersections_merge_all_contributors_and_along_guides_keep_arc_provenance() {
        let dimensions = vec![dimension(1, 0.0), dimension(2, 90.0), dimension(3, 45.0)];
        let definition = intersections(
            dimensions.clone(),
            vec![
                GuideDimensionId(1),
                GuideDimensionId(2),
                GuideDimensionId(3),
            ],
        );
        let plan = resolve_pattern_pipeline(&definition).unwrap();
        let output =
            evaluate_generalized_straight_guides_cancellable(&plan.family, &request(), &|| false)
                .unwrap();
        assert!(output.sites.iter().any(|site| matches!(&site.provenance, GeneralizedSiteProvenance::Intersection { contributors } if contributors.len() >= 3)));
        let along = PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(8),
            "along",
            PatternMechanismId(21),
            PatternMechanismId(22),
            PatternOutputLayerId(23),
            dimensions,
            GeneralizedSiteProduct::AlongGuides {
                dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
                interval_multiplier: 0.5,
                phase: 1.0,
            },
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        );
        toniator_domain::validate_pattern_definition(&along).unwrap();
        let plan = resolve_pattern_pipeline(&along).unwrap();
        let output =
            evaluate_generalized_straight_guides_cancellable(&plan.family, &request(), &|| false)
                .unwrap();
        assert!(output.sites.iter().all(|site| matches!(
            site.provenance,
            GeneralizedSiteProvenance::AlongGuide { .. }
        )));
        assert!(
            output
                .sites
                .iter()
                .any(|site| matches!(site.scope, SiteScope::Guard))
        );
    }

    #[test]
    fn invalid_selection_parallel_products_limits_and_cancellation_are_stable() {
        let mut definition = intersections(
            vec![dimension(1, 0.0), dimension(2, 90.0)],
            vec![GuideDimensionId(2), GuideDimensionId(1)],
        );
        assert_eq!(
            toniator_domain::validate_pattern_definition(&definition)
                .unwrap_err()
                .path(),
            "pattern_definitions.mechanisms.intersections.dimensions"
        );
        definition = intersections(
            vec![dimension(1, 0.0), dimension(2, 0.0)],
            vec![GuideDimensionId(1), GuideDimensionId(2)],
        );
        let plan = resolve_pattern_pipeline(&definition).unwrap();
        assert_eq!(
            evaluate_generalized_straight_guides_cancellable(&plan.family, &request(), &|| false)
                .unwrap_err()
                .path(),
            "pattern.family.intersections"
        );
        let mut limited = request();
        limited.max_family_candidates = 1;
        assert_eq!(
            evaluate_generalized_straight_guides_cancellable(&plan.family, &limited, &|| false)
                .unwrap_err()
                .path(),
            "coverage.candidate_limit"
        );
        assert_eq!(
            evaluate_generalized_straight_guides_cancellable(&plan.family, &request(), &|| true)
                .unwrap_err()
                .path(),
            "evaluation.cancelled"
        );
    }

    #[test]
    fn anisotropic_phase_and_along_arc_anchor_are_structural_inputs() {
        let mut dimensions = vec![dimension(1, 17.0), dimension(2, 89.5)];
        dimensions[0].phase = 2.5;
        dimensions[1].phase = 23.0;
        let definition = intersections(
            dimensions.clone(),
            vec![GuideDimensionId(1), GuideDimensionId(2)],
        );
        let plan = resolve_pattern_pipeline(&definition).unwrap();
        let mut anisotropic = request();
        anisotropic.density.across_x = 24.0;
        anisotropic.density.across_y = 4.0;
        let output =
            evaluate_generalized_straight_guides_cancellable(&plan.family, &anisotropic, &|| false)
                .unwrap();
        assert_ne!(output.coverage[0].spacing, output.coverage[1].spacing);
        assert!(
            output
                .coverage
                .iter()
                .all(|coverage| coverage.normalized_phase.is_finite())
        );
        assert_eq!(
            output
                .guides
                .iter()
                .find(|guide| guide.id == GuideInstanceId::new(GuideDimensionId(1), 0))
                .unwrap()
                .offset,
            2.5
        );
        assert_eq!(
            output
                .guides
                .iter()
                .find(|guide| guide.id == GuideInstanceId::new(GuideDimensionId(2), 0))
                .unwrap()
                .offset,
            23.0
        );

        let along = PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(9),
            "arc",
            PatternMechanismId(31),
            PatternMechanismId(32),
            PatternOutputLayerId(33),
            dimensions,
            GeneralizedSiteProduct::AlongGuides {
                dimensions: vec![GuideDimensionId(1)],
                interval_multiplier: 0.5,
                phase: 1.25,
            },
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        );
        let plan = resolve_pattern_pipeline(&along).unwrap();
        let first =
            evaluate_generalized_straight_guides_cancellable(&plan.family, &anisotropic, &|| false)
                .unwrap();
        let mut resized = anisotropic.clone();
        resized.canvas.width = 240.0;
        let second =
            evaluate_generalized_straight_guides_cancellable(&plan.family, &resized, &|| false)
                .unwrap();
        let first_positions: Vec<_> = first
            .sites
            .iter()
            .filter_map(|site| match site.provenance {
                GeneralizedSiteProvenance::AlongGuide {
                    absolute_arc_position_bits,
                    local_arc_position_bits,
                    ..
                } => Some((absolute_arc_position_bits, local_arc_position_bits)),
                _ => None,
            })
            .collect();
        let second_positions: Vec<_> = second
            .sites
            .iter()
            .filter_map(|site| match site.provenance {
                GeneralizedSiteProvenance::AlongGuide {
                    absolute_arc_position_bits,
                    local_arc_position_bits,
                    ..
                } => Some((absolute_arc_position_bits, local_arc_position_bits)),
                _ => None,
            })
            .collect();
        assert!(
            first_positions
                .iter()
                .any(|value| second_positions.contains(value))
        );
    }

    #[test]
    fn dimension_count_duplicate_and_missing_ids_have_stable_validation_paths() {
        for count in [0_usize, 5] {
            let dimensions = (0..count)
                .map(|index| dimension((index + 1) as u64, index as f64 * 30.0))
                .collect();
            let definition =
                intersections(dimensions, vec![GuideDimensionId(1), GuideDimensionId(2)]);
            assert_eq!(
                toniator_domain::validate_pattern_definition(&definition)
                    .unwrap_err()
                    .path(),
                "pattern_definitions.mechanisms.dimensions"
            );
        }
        let duplicate = intersections(
            vec![dimension(1, 0.0), dimension(1, 90.0)],
            vec![GuideDimensionId(1), GuideDimensionId(1)],
        );
        assert_eq!(
            toniator_domain::validate_pattern_definition(&duplicate)
                .unwrap_err()
                .path(),
            "pattern_definitions.mechanisms.dimensions"
        );
        let missing = intersections(
            vec![dimension(1, 0.0), dimension(2, 90.0)],
            vec![GuideDimensionId(1), GuideDimensionId(9)],
        );
        assert_eq!(
            toniator_domain::validate_pattern_definition(&missing)
                .unwrap_err()
                .path(),
            "pattern_definitions.mechanisms.intersections.dimensions"
        );
        let along = PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(10),
            "one",
            PatternMechanismId(41),
            PatternMechanismId(42),
            PatternOutputLayerId(43),
            vec![dimension(1, 0.0)],
            GeneralizedSiteProduct::AlongGuides {
                dimensions: vec![GuideDimensionId(1)],
                interval_multiplier: 1.0,
                phase: 0.0,
            },
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        );
        toniator_domain::validate_pattern_definition(&along).unwrap();
    }

    /// Verifies generalized family and realization inputs contribute to their contract identities.
    ///
    /// # Panics
    ///
    /// Panics when generalized identity omits an asserted structural or realization input.
    #[test]
    fn generalized_identity_covers_family_and_realization_contract_inputs() {
        let definition = intersections(
            vec![dimension(1, 0.0), dimension(2, 67.0), dimension(3, 121.0)],
            vec![
                GuideDimensionId(1),
                GuideDimensionId(2),
                GuideDimensionId(3),
            ],
        );
        let fingerprint = |definition: &PatternDefinition,
                           request: &StraightGuideInspectRequest| {
            let plan = resolve_pattern_pipeline(definition).unwrap();
            evaluate_generalized_straight_guides_cancellable(&plan.family, request, &|| false)
                .unwrap()
                .family_fingerprint
        };
        let baseline = fingerprint(&definition, &request());

        let mut changed_mechanism_ids = definition.clone();
        changed_mechanism_ids.family = PatternFamily::GuideIntersections {
            guide_mechanism_id: PatternMechanismId(71),
            site_mechanism_id: PatternMechanismId(72),
        };
        let PatternMechanism::StraightGuideDimensions { id, .. } =
            &mut changed_mechanism_ids.mechanisms[0]
        else {
            unreachable!()
        };
        *id = PatternMechanismId(71);
        let PatternMechanism::SelectedGuideIntersections {
            id,
            guide_mechanism_id,
            ..
        } = &mut changed_mechanism_ids.mechanisms[1]
        else {
            unreachable!()
        };
        *id = PatternMechanismId(72);
        *guide_mechanism_id = PatternMechanismId(71);
        let PatternOutputLayer::MarkPrototype {
            site_mechanism_id, ..
        } = &mut changed_mechanism_ids.output_layers[0]
        else {
            unreachable!()
        };
        *site_mechanism_id = PatternMechanismId(72);
        assert_ne!(baseline, fingerprint(&changed_mechanism_ids, &request()));
        let mut aspect = request();
        aspect.density.aspect_locked = false;
        assert_ne!(baseline, fingerprint(&definition, &aspect));
        let mut changed_phase = definition.clone();
        let PatternMechanism::StraightGuideDimensions { dimensions, .. } =
            &mut changed_phase.mechanisms[0]
        else {
            unreachable!()
        };
        dimensions[0].phase = 2.5;
        assert_ne!(baseline, fingerprint(&changed_phase, &request()));
        let mut changed_selection = definition.clone();
        let PatternMechanism::SelectedGuideIntersections { dimensions, .. } =
            &mut changed_selection.mechanisms[1]
        else {
            unreachable!()
        };
        *dimensions = vec![GuideDimensionId(1), GuideDimensionId(3)];
        assert_ne!(baseline, fingerprint(&changed_selection, &request()));
        let mut changed_merge = definition.clone();
        let PatternMechanism::SelectedGuideIntersections { merge_epsilon, .. } =
            &mut changed_merge.mechanisms[1]
        else {
            unreachable!()
        };
        *merge_epsilon = 0.25;
        assert_ne!(baseline, fingerprint(&changed_merge, &request()));

        let family = evaluate_typed_family(
            &definition,
            &GridInspectRequest {
                canvas: request().canvas,
                density: request().density,
                rotation_degrees: request().rotation_degrees,
                translation_x: request().translation_x,
                translation_y: request().translation_y,
                guard_steps: request().guard_steps,
                support_radius: request().support_radius,
                max_family_candidates: request().max_family_candidates,
            },
        )
        .unwrap();
        let mut fixed = definition.clone();
        let PatternOutputLayer::MarkPrototype { orientation, .. } = &mut fixed.output_layers[0]
        else {
            unreachable!()
        };
        *orientation = MarkOrientation::Fixed;
        let image = image::RgbaImage::from_raw(1, 1, vec![255, 0, 0, 255]).unwrap();
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let source = decode_source(&bytes, SourceFormatHint::Png).unwrap();
        let grid_request = GridInspectRequest {
            canvas: request().canvas,
            density: request().density,
            rotation_degrees: request().rotation_degrees,
            translation_x: request().translation_x,
            translation_y: request().translation_y,
            guard_steps: request().guard_steps,
            support_radius: request().support_radius,
            max_family_candidates: request().max_family_candidates,
        };
        let tangent = realize_typed_mapped_outputs(
            &family,
            &resolve_pattern_pipeline(&definition).unwrap(),
            &source,
            &grid_request.canvas,
            SourceMapping::canonical(SourceMappingComponent::Red),
            MarkResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.9,
                rotation_offset_degrees: 0.0,
            },
        )
        .unwrap();
        let mut different_layer = fixed.clone();
        let PatternOutputLayer::MarkPrototype { id, .. } = &mut different_layer.output_layers[0]
        else {
            unreachable!()
        };
        *id = PatternOutputLayerId(99);
        let fixed = realize_typed_mapped_outputs(
            &family,
            &resolve_pattern_pipeline(&fixed).unwrap(),
            &source,
            &grid_request.canvas,
            SourceMapping::canonical(SourceMappingComponent::Red),
            MarkResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.9,
                rotation_offset_degrees: 0.0,
            },
        )
        .unwrap();
        let different_layer = realize_typed_mapped_outputs(
            &family,
            &resolve_pattern_pipeline(&different_layer).unwrap(),
            &source,
            &grid_request.canvas,
            SourceMapping::canonical(SourceMappingComponent::Red),
            MarkResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.9,
                rotation_offset_degrees: 0.0,
            },
        )
        .unwrap();
        assert_eq!(tangent.output.marks, fixed.output.marks);
        assert_ne!(
            tangent.output.realization_fingerprint,
            fixed.output.realization_fingerprint
        );
        assert_eq!(
            tangent.provenance.ordered_output_prototypes,
            vec![MarkPrototype::Circle]
        );
        assert_eq!(fixed.output.marks, different_layer.output.marks);
        assert_ne!(
            fixed.output.realization_fingerprint,
            different_layer.output.realization_fingerprint
        );
    }
}
