#![forbid(unsafe_code)]

//! Deterministic straight-guide family evaluation.

use std::{collections::BTreeMap, error::Error, fmt};

use serde::Serialize;
use toniator_domain::{
    ArtworkWeightResponse, CanvasSpec, ChannelId, DensityMetric2D, DocumentCommand,
    DocumentHistory, DocumentSessionError, GuideDimensionDraft, GuideDimensionId, MarkOrientation,
    MarkOrientationDraft, MarkPrototype, PatternDefinition, PatternDefinitionRecipe, PatternFamily,
    PatternMechanism, PatternMechanismId, PatternModulation, PatternOutputLayer,
    PatternOutputLayerId, PresetMetadata, PresetRecord, RandomSiteCharacter, SiteDensityModulation,
    SiteExclusionPolicy, SourceMapping, StraightGuideDimension, VisibleMarkSizingPolicy,
};
pub use toniator_geometry::{
    AffineTransform2D, Bounds, CanonicalCircleMark, GuideInstanceId, GuideIntersectionProvenance,
    IntersectionSite, Point2, SiteId, SiteScope, StraightGuide, Vector2, projection_range,
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
                            maximum_support_radius: 4.5,
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
                            maximum_support_radius: 4.5,
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
        history.apply(&DocumentCommand::ReplaceSelectedChannelDefinitionRecipe {
            channel_id,
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
    pub prototype: MarkPrototype,
    pub orientation: MarkOrientation,
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
/// consume only the canonical marks emitted by realizers, never this enum.
#[derive(Clone, Debug, PartialEq)]
pub enum TypedFamilyOutput {
    GuideIntersections {
        family: FamilyCapability,
        output: GridFamilyOutput,
    },
    GeneralizedStraightGuides {
        family: FamilyCapability,
        output: GridFamilyOutput,
        product_provenance: Vec<GeneralizedSiteProvenance>,
    },
    RandomSites {
        family: FamilyCapability,
        output: GridFamilyOutput,
        product_provenance: Vec<RandomSiteProvenance>,
        diagnostics: RandomSiteDiagnostics,
    },
}

impl TypedFamilyOutput {
    pub fn family(&self) -> &FamilyCapability {
        match self {
            Self::GuideIntersections { family, .. }
            | Self::GeneralizedStraightGuides { family, .. }
            | Self::RandomSites { family, .. } => family,
        }
    }

    pub fn family_fingerprint(&self) -> &str {
        match self {
            Self::GuideIntersections { output, .. }
            | Self::GeneralizedStraightGuides { output, .. }
            | Self::RandomSites { output, .. } => &output.family_fingerprint,
        }
    }

    pub fn grid(&self) -> &GridFamilyOutput {
        match self {
            Self::GuideIntersections { output, .. }
            | Self::GeneralizedStraightGuides { output, .. }
            | Self::RandomSites { output, .. } => output,
        }
    }
}

/// Provenance that survives explicit modulation and ordered output realization.
/// It is intentionally adjacent to, rather than mixed into, canonical marks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedRealizationProvenance {
    pub structural: StructuralProductProvenance,
    pub modulation: PatternModulation,
    pub ordered_output_layer_ids: Vec<PatternOutputLayerId>,
    pub ordered_output_prototypes: Vec<MarkPrototype>,
    pub ordered_output_orientations: Vec<MarkOrientation>,
    /// Exact generalized product provenance retained beside the canonical
    /// circle adapter; no realizer reconstructs it from finite guides.
    pub site_product_provenance: Vec<GeneralizedSiteProvenance>,
    /// Random-site provenance remains adjacent to canonical marks so render
    /// algorithms do not acquire a source-distribution branch.
    pub random_site_product_provenance: Vec<RandomSiteProvenance>,
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

/// Resolve the typed capability graph in declared order. This is the one
/// family/output compatibility boundary shared by document and diagnostic
/// evaluation; unsupported combinations fail before any source decode or
/// cache transaction can occur.
pub fn resolve_pattern_pipeline(
    definition: &PatternDefinition,
) -> Result<PatternPipelinePlan, PatternPipelineError> {
    if matches!(definition.family, PatternFamily::RandomSites { .. }) {
        return resolve_random_site_pipeline(definition);
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
                prototype: MarkPrototype::Circle,
                orientation: MarkOrientation::Fixed,
            }),
            PatternOutputLayer::MarkPrototype {
                id,
                site_mechanism_id: source_id,
                prototype: toniator_domain::MarkPrototype::Circle,
                orientation,
            } if *source_id == site_mechanism_id => ordered_outputs.push(OutputCapability {
                layer_id: *id,
                consumes: product,
                prototype: MarkPrototype::Circle,
                orientation: orientation.clone(),
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
        },
        modulation: definition.modulation.clone(),
        ordered_outputs,
    })
}

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
            prototype: MarkPrototype::Circle,
            orientation: MarkOrientation::Fixed,
        },
    ] = definition.output_layers.as_slice()
    else {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.capability",
            "random-site products require one fixed circle mark output",
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
        },
        modulation: definition.modulation.clone(),
        ordered_outputs: vec![OutputCapability {
            layer_id: *id,
            consumes: product,
            prototype: MarkPrototype::Circle,
            orientation: MarkOrientation::Fixed,
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
    if family.product == StructuralProductCapability::RandomSites {
        let output = evaluate_random_sites_cancellable(family, request, source, is_cancelled)?;
        return Ok(TypedFamilyOutput::RandomSites {
            family: family.clone(),
            product_provenance: output.provenance,
            diagnostics: output.diagnostics,
            output: output.grid,
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
        let product_provenance = output
            .sites
            .iter()
            .map(|site| site.provenance.clone())
            .collect();
        return Ok(TypedFamilyOutput::GeneralizedStraightGuides {
            family: family.clone(),
            output: generalized_as_grid_output(output, request.support_radius, request.guard_steps),
            product_provenance,
        });
    }
    let output = evaluate_straight_grid_cancellable(request, is_cancelled)
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
    Ok(TypedFamilyOutput::GuideIntersections {
        family: family.clone(),
        output,
    })
}

fn generalized_as_grid_output(
    output: GeneralizedStraightGuideOutput,
    support_radius: f64,
    guard_steps: u32,
) -> GridFamilyOutput {
    let sites = output
        .sites
        .into_iter()
        .map(|site| {
            let (first, second, contributors) = match site.provenance {
                GeneralizedSiteProvenance::Intersection { contributors } => {
                    let first = contributors
                        .first()
                        .copied()
                        .expect("intersection provenance has contributors");
                    let second = contributors.get(1).copied().unwrap_or(first);
                    (first, second, contributors)
                }
                GeneralizedSiteProvenance::AlongGuide {
                    guide_id, sequence, ..
                } => (
                    guide_id,
                    GuideInstanceId {
                        dimension_id: guide_id.dimension_id,
                        index: sequence,
                    },
                    vec![guide_id],
                ),
            };
            IntersectionSite {
                id: SiteId {
                    first_dimension_id: first.dimension_id,
                    first_index: first.index,
                    second_dimension_id: second.dimension_id,
                    second_index: second.index,
                },
                position: site.position,
                scope: site.scope,
                provenance: GuideIntersectionProvenance { contributors },
            }
        })
        .collect();
    GridFamilyOutput {
        family_fingerprint: output.family_fingerprint,
        guard_steps,
        support_radius,
        antialias_margin: ANTIALIAS_MARGIN,
        generation_domain: Bounds::from_points(
            output
                .guides
                .iter()
                .flat_map(|guide| [guide.start, guide.end]),
        )
        .expect("generalized finite guides produce bounds"),
        coverage: output.coverage,
        guides: output.guides,
        sites,
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
    grid: GridFamilyOutput,
    provenance: Vec<RandomSiteProvenance>,
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
    let sites: Vec<_> = accepted
        .iter()
        .enumerate()
        .map(|(index, (point, _, scope))| IntersectionSite {
            id: SiteId {
                first_dimension_id: family.provenance.mechanism_ids[3].0,
                first_index: i64::try_from(index).expect("candidate limit bounds site index"),
                second_dimension_id: family.provenance.mechanism_ids[0].0,
                second_index: i64::from(random.seed),
            },
            position: *point,
            scope: *scope,
            provenance: GuideIntersectionProvenance {
                contributors: vec![
                    GuideInstanceId {
                        dimension_id: family.provenance.mechanism_ids[0].0,
                        index: i64::from(random.seed),
                    },
                    GuideInstanceId {
                        dimension_id: family.provenance.mechanism_ids[3].0,
                        index: i64::try_from(index).expect("candidate limit bounds site index"),
                    },
                ],
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
        grid: GridFamilyOutput {
            family_fingerprint: random_family_fingerprint(family, request, weighted_source),
            guard_steps: request.guard_steps,
            support_radius: request.support_radius,
            antialias_margin: ANTIALIAS_MARGIN,
            generation_domain: padded,
            coverage: Vec::new(),
            guides: Vec::new(),
            sites,
        },
        provenance,
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
    realize_mapped_circular_marks(family.grid(), source, canvas, mapping, response)
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
    realize_source_color_circular_marks(family.grid(), source, canvas, mapping, response)
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
    realize_circular_marks(
        family.grid(),
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

fn orientation_identity(
    legacy_identity: &str,
    family: &TypedFamilyOutput,
    provenance: &TypedRealizationProvenance,
) -> String {
    if matches!(family, TypedFamilyOutput::GuideIntersections { .. }) {
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
        bytes.push(match prototype {
            MarkPrototype::Circle => 1,
        });
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
        [OutputCapability { consumes, .. }] if *consumes == family.family().product => {
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
                    .map(|output| output.prototype.clone())
                    .collect(),
                ordered_output_orientations: plan
                    .ordered_outputs
                    .iter()
                    .map(|output| output.orientation.clone())
                    .collect(),
                site_product_provenance: match family {
                    TypedFamilyOutput::GuideIntersections { .. } => Vec::new(),
                    TypedFamilyOutput::GeneralizedStraightGuides {
                        product_provenance, ..
                    } => product_provenance.clone(),
                    TypedFamilyOutput::RandomSites { .. } => Vec::new(),
                },
                random_site_product_provenance: match family {
                    TypedFamilyOutput::RandomSites {
                        product_provenance, ..
                    } => product_provenance.clone(),
                    _ => Vec::new(),
                },
            })
        }
        _ => Err(PatternPipelineError::new(
            "pattern.output_layers.capability",
            "ordered output realization has no compatible circular-mark layer",
        )),
    }
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
    pub minimum_size: f64,
    pub maximum_size: f64,
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
        let radius = radius_from_ink_with_support(ink, response, family.support_radius)?;
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
        let radius = radius_from_ink_with_support(ink, response, family.support_radius)?;
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
            radius_from_ink_with_support(sample.response, response, family.support_radius)?;
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

/// Maps an effective mark-ink response linearly to radius using the authored
/// diameter bounds. Source sampling owns component polarity and alpha handling.
pub fn radius_from_ink(ink: f64, response: MarkResponse) -> Result<f64, RealizationError> {
    validate_response_basic(response)?;
    if !ink.is_finite() {
        return Err(RealizationError::new(
            "realization.ink",
            "effective mark ink must be finite",
        ));
    }
    let ink = ink.clamp(0.0, 1.0);
    Ok((response.minimum_size + ink * (response.maximum_size - response.minimum_size)) / 2.0)
}

fn radius_from_ink_with_support(
    ink: f64,
    response: MarkResponse,
    maximum_support_radius: f64,
) -> Result<f64, RealizationError> {
    validate_response(response, maximum_support_radius)?;
    if !ink.is_finite() {
        return Err(RealizationError::new(
            "realization.ink",
            "effective mark ink must be finite",
        ));
    }
    let ink = ink.clamp(0.0, 1.0);
    Ok((response.minimum_size + ink * (response.maximum_size - response.minimum_size)) / 2.0)
}

fn validate_response(
    response: MarkResponse,
    maximum_support_radius: f64,
) -> Result<(), RealizationError> {
    validate_response_basic(response)?;
    if !maximum_support_radius.is_finite() || maximum_support_radius < 0.0 {
        return Err(RealizationError::new(
            "realization.family.support_radius",
            "family support capability must be finite and nonnegative",
        ));
    }
    if response.maximum_size / 2.0 > maximum_support_radius {
        return Err(RealizationError::new(
            "realization.response.maximum_size",
            "maximum diameter exceeds the family support capability",
        ));
    }
    Ok(())
}

fn validate_response_basic(response: MarkResponse) -> Result<(), RealizationError> {
    if !response.minimum_size.is_finite() || !response.maximum_size.is_finite() {
        return Err(RealizationError::new(
            "realization.response",
            "diameters must be finite",
        ));
    }
    if response.minimum_size < 0.0
        || response.maximum_size < 0.0
        || response.minimum_size > response.maximum_size
    {
        return Err(RealizationError::new(
            "realization.response",
            "diameters must be nonnegative and minimum must not exceed maximum",
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
        .chain(response.minimum_size.to_bits().to_le_bytes())
        .chain(response.maximum_size.to_bits().to_le_bytes())
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
    bytes.extend(response.minimum_size.to_bits().to_le_bytes());
    bytes.extend(response.maximum_size.to_bits().to_le_bytes());
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
            support_radius: 4.5,
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
            minimum_size: 2.0,
            maximum_size: 9.0,
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
            luminance.marks[0].radius, 1.0,
            "transparent black is the minimum mark radius"
        );
        assert!(
            [1, 2, 5]
                .into_iter()
                .all(|index| luminance.marks[index].radius == luminance.marks[0].radius),
            "all zero-alpha hidden RGB variants map to minimum radius"
        );
        assert_eq!(luminance.marks[3].radius, 4.5);
        assert_eq!(luminance.marks[4].radius, 1.0);
        let half_alpha_radius = (2.0 + (128.0 / 255.0) * 7.0) / 2.0;
        assert!((luminance.marks[6].radius - half_alpha_radius).abs() < 1e-12);
        assert_eq!(luminance.marks[7].radius, 4.5);
        assert!(
            alpha.marks[5].radius > alpha.marks[6].radius
                && alpha.marks[6].radius > alpha.marks[7].radius,
            "Alpha response has one decreasing alpha polarity, without squaring"
        );
        assert!((alpha.marks[6].radius - (2.0 + (127.0 / 255.0) * 7.0) / 2.0).abs() < 1e-12);
        assert_ne!(
            luminance.realization_fingerprint,
            alpha.realization_fingerprint
        );
    }

    #[test]
    fn diameter_response_uses_effective_ink_and_stores_radius() {
        let response = MarkResponse {
            minimum_size: 2.0,
            maximum_size: 9.0,
        };
        assert_eq!(radius_from_ink(0.0, response).unwrap(), 1.0);
        assert_eq!(radius_from_ink(0.5, response).unwrap(), 2.75);
        assert_eq!(radius_from_ink(1.0, response).unwrap(), 4.5);
        assert!(radius_from_ink(f64::NAN, response).is_err());
        assert!(
            radius_from_ink(
                0.5,
                MarkResponse {
                    minimum_size: -1.0,
                    maximum_size: 9.0
                }
            )
            .is_err()
        );
        assert!(
            radius_from_ink(
                0.5,
                MarkResponse {
                    minimum_size: 0.5,
                    maximum_size: 12.0,
                }
            )
            .is_ok()
        );
    }

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
                minimum_size: 2.0,
                maximum_size: 9.0,
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
                minimum_size: 3.0,
                maximum_size: 8.0,
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

    #[test]
    fn direct_realization_rejects_a_response_beyond_declared_family_support() {
        let family = family();
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
                minimum_size: 2.0,
                maximum_size: 9.1,
            },
        )
        .expect_err("family support is authoritative at direct realization");
        assert_eq!(error.path(), "realization.response.maximum_size");

        let mut wider_family = family;
        wider_family.support_radius = 6.0;
        let wider = realize_circular_marks(
            &wider_family,
            &field(),
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            MarkResponse {
                minimum_size: 0.5,
                maximum_size: 12.0,
            },
        )
        .expect("declared support permits diameters below 2 and above 9");
        assert!(
            wider
                .marks
                .iter()
                .all(|mark| mark.radius >= 0.25 && mark.radius <= 6.0)
        );
    }

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
                minimum_size: 2.0,
                maximum_size: 9.0,
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
                minimum_size: 2.0,
                maximum_size: 9.0,
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

    #[test]
    fn mapped_realization_keeps_family_structural_and_validates_direct_inputs() {
        let family = sites_at_positions(&[0.0, 1.0]);
        let source = source_from_rgba(2, vec![255, 0, 0, 128, 0, 0, 0, 255]);
        let canvas = CanvasSpec {
            width: 1.0,
            height: 1.0,
        };
        let response = MarkResponse {
            minimum_size: 2.0,
            maximum_size: 9.0,
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
                    minimum_size: 2.0,
                    maximum_size: 9.1
                },
            )
            .unwrap_err()
            .path(),
            "realization.response.maximum_size"
        );
    }

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
            minimum_size: 2.0,
            maximum_size: 9.0,
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
        assert_eq!(opaque.mark.radius, 4.5);
        assert!((partial.mark.radius - (2.0 + (128.0 / 255.0) * 7.0) / 2.0).abs() < 1e-12);
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
            minimum_size: 2.0,
            maximum_size: 9.0,
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
                minimum_size: 2.0,
                maximum_size: 9.0,
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
        assert_eq!(realization.marks[0].mark.radius, 2.75);
    }

    #[test]
    fn inverted_stage9_luminance_and_alpha_mappings_retain_legacy_mark_geometry() {
        let family = sites_at_positions(&[0.0, 1.0]);
        let source = source_from_rgba(2, vec![255, 0, 0, 128, 255, 255, 255, 64]);
        let canvas = CanvasSpec {
            width: 1.0,
            height: 1.0,
        };
        let response = MarkResponse {
            minimum_size: 2.0,
            maximum_size: 9.0,
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

    #[test]
    fn stage9_realizations_exercise_both_immutable_baseline_sources() {
        let family = family();
        let canvas = CanvasSpec {
            width: 90.0,
            height: 60.0,
        };
        let response = MarkResponse {
            minimum_size: 2.0,
            maximum_size: 9.0,
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
                maximum_support_radius: 4.5,
            },
        )
    }

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
            support_radius: 4.5,
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
        assert_eq!(generic.family_fingerprint(), expected.family_fingerprint);
        assert_eq!(generic.grid(), &expected);

        let realization = realize_typed_mapped_outputs(
            &generic,
            &plan,
            &source(),
            &request().canvas,
            SourceMapping::canonical(SourceMappingComponent::Red),
            MarkResponse {
                minimum_size: 2.0,
                maximum_size: 9.0,
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
            support_radius: 4.5,
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
                maximum_support_radius: 4.5,
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
                maximum_support_radius: 4.5,
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
                maximum_support_radius: 4.5,
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
                maximum_support_radius: 4.5,
            },
        );
        toniator_domain::validate_pattern_definition(&along).unwrap();
    }

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
                minimum_size: 2.0,
                maximum_size: 9.0,
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
                minimum_size: 2.0,
                maximum_size: 9.0,
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
                minimum_size: 2.0,
                maximum_size: 9.0,
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
