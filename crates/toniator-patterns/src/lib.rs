#![forbid(unsafe_code)]

//! Deterministic straight-guide family evaluation.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fmt,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use rayon::prelude::*;
use serde::Serialize;
use toniator_domain::{
    ArtworkWeightResponse, AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft,
    AuthoredStructureId, AuthoredStructureKind, CanvasSpec, ChannelId, ChannelPaint,
    ConnectionProgram, CurveRepetition, CurveWinding, DensityMetric2D, Document, DocumentCommand,
    DocumentHistory, DocumentSessionError, EffectivePatternOutputSettings, GuideDimension,
    GuideDimensionDraft, GuideDimensionId, GuidePrototype, GuideRepetition, MarkOrientation,
    MarkOrientationDraft, MarkPrototype, MazeProgram, OffsetCleanup, ParametricCurve,
    PatternCapabilityFlag, PatternCapabilityScope, PatternDefinition, PatternDefinitionRecipe,
    PatternFamily, PatternGeometryResponse, PatternMechanism, PatternMechanismId,
    PatternModulation, PatternOutputLayerId, PatternOutputRealization, PatternStructureRecipe,
    PresetMetadata, PresetRecord, RandomSiteCharacter, RegionResizeAlgorithm, RegionSourceIntent,
    ResolvedDensityMetric2D, SiteDensityModulation, SiteExclusionPolicy, SiteUseFilter,
    SourceMapping, SourceReference, SpiralCurve, SpiralShape, StraightGuideDimension,
    pattern_output_evaluation_order,
};
pub use toniator_geometry::{
    AffineTransform2D, Bounds, CONNECTION_PATH_CONTRACT_ID, CONNECTION_TRAIL_CONTRACT_ID,
    CanonicalCircleMark, CanonicalFillRule, CanonicalMark, CanonicalPathMark,
    CanonicalRegionLimits, CanonicalRegionProposal, CanonicalRegionSet, CanonicalRegionSourceGroup,
    CanonicalRegionSourceId, CanonicalStroke, CanonicalStrokeSourceId, ConnectionPathId,
    ConnectionPathLimits, ConnectionPathSet, CubicBezierSegment, CurveError, CurvePath,
    CurveSegment, FamilySite, FamilySiteError, FamilySiteId, FamilySiteProvenance, FamilySiteSet,
    GUIDE_FACE_CONTRACT_ID, GuideFaceLimits, GuideFaceRequest, GuideInstanceId,
    GuideIntersectionProvenance, IntersectionSite, LineSegment, MAZE_WALL_CONTRACT_ID,
    MazeGuideAxis, MazeLimits, MazeProgramResult, MazeWallPathId, NominalCellBasis,
    PATH_OFFSET_ALGORITHM_CONTRACT_ID, PathClosure, PathLocation, PathOffsetCleanup,
    PathOffsetEndpointPolicy, PathOffsetLimits, PathOffsetRequest, PathOffsetResult, Point2,
    REGION_TREATMENT_CONTRACT_ID, RegionReference, RegionTreatment, RegionTreatmentError,
    RegionTreatmentLimits, RegionTreatmentProvenance, RegionTreatmentRequest,
    RegionTreatmentResult, SITE_ADJACENCY_CONTRACT_ID, SiteAdjacencyError, SiteAdjacencyGraph,
    SiteAdjacencyLimits, SiteAdjacencyPolicy, SiteId, SiteScope, StraightGuide,
    StrokeProfileSample, StructuralPathInstance, StructuralPathInstanceId,
    StructuralPathLocationProvenance, StructuralPathSet, StructuralPathSourceId,
    VORONOI_REGION_CONTRACT_ID, VariableWidthOutlineLimits, VariableWidthPathSample, Vector2,
    VoronoiRegionDiagnostics, VoronoiRegionLimits, VoronoiRegionRequest,
    advance_planar_constant_gap_frontier_cancellable, build_canonical_regions_cancellable,
    build_connection_paths_cancellable, build_guide_faces_cancellable,
    build_maze_walls_from_sites_cancellable, build_site_adjacency_cancellable,
    build_variable_width_outline_cancellable, build_voronoi_regions_cancellable,
    connection_program_contract_id, insert_solved_crossing_nodes_cancellable,
    offset_path_cancellable, projection_range, resolve_guide_prototype,
    stabilize_planar_constant_gap_path, treat_region_requests_cancellable,
    treat_regions_cancellable, voronoi_region_references,
};
use toniator_sampling::{
    RegionSamplingDiagnostics, RegionSamplingLimits, RegionSourceSample, SampledSourcePaint,
    SamplingError, SourceComponent, SourceField, SourceMappingComponent, SourcePlacement,
    sample_region_area_average_batch_with_diagnostics,
    sample_region_area_average_batch_with_diagnostics_and_progress, sample_region_reference,
};

/// The finite antialiasing envelope included in every Stage 3 generation plan.
pub const ANTIALIAS_MARGIN: f64 = 1.0;

/// Stable IDs for the two fixed rectangular straight-guide dimensions.
pub const FIRST_DIMENSION_ID: GuideDimensionId = GuideDimensionId(1);
pub const SECOND_DIMENSION_ID: GuideDimensionId = GuideDimensionId(2);

/// The stable version of the built-in registry ordering and metadata contract.
pub const BUNDLED_PRESET_REGISTRY_VERSION: u32 = 3;

/// Versioned cache-identity contract for typed filled-region realization.
///
/// The digest encodes the complete ordered realization inputs without retaining debug-rendered
/// canonical IDs, whose Guide Face boundary provenance can otherwise grow with every region.
const REGION_REALIZER_FINGERPRINT_CONTRACT_ID: &str =
    "toniator-stage-21a-region-realizer-v2-typed-streaming";

/// Immutable, deterministic preset registry owned by the pattern/schema layer.
#[derive(Clone, Debug, PartialEq)]
pub struct PresetRegistry {
    version: u32,
    entries: Vec<PresetRecord>,
    catalog_entries: Vec<PresetCatalogEntry>,
}

/// Builds one catalog mark recipe with the approved normalized response range.
fn mark_recipe(structure: PatternStructureRecipe) -> PatternDefinitionRecipe {
    let mut recipe = PatternDefinitionRecipe::marks(structure);
    recipe.output_settings[0].response =
        PatternGeometryResponse::Marks(toniator_domain::MarkGeometryResponse {
            minimum_fill: 0.25,
            maximum_fill: 0.85,
        });
    recipe
}

/// Builds one catalog connected recipe with the approved normalized response range.
fn path_recipe(structure: PatternStructureRecipe) -> PatternDefinitionRecipe {
    let mut recipe = PatternDefinitionRecipe::connected(structure);
    recipe.output_settings[0].response =
        PatternGeometryResponse::Connected(toniator_domain::ConnectedGeometryResponse {
            minimum_thickness: 0.15,
            maximum_thickness: 0.65,
            bias: 0.0,
        });
    recipe
}

/// Builds a catalog Voronoi recipe with an explicit existing region response.
fn voronoi_recipe(
    structure: PatternStructureRecipe,
    response: toniator_domain::RegionGeometryResponse,
) -> PatternDefinitionRecipe {
    let mut recipe = PatternDefinitionRecipe::regions(structure);
    recipe.output_settings[0].response = PatternGeometryResponse::Regions(response);
    recipe
}

/// Builds a catalog Guide Faces recipe with an explicit existing region response.
fn guide_face_recipe(
    structure: PatternStructureRecipe,
    dimensions: Vec<usize>,
    response: toniator_domain::RegionGeometryResponse,
) -> PatternDefinitionRecipe {
    let mut recipe = PatternDefinitionRecipe::guide_faces(structure, dimensions);
    recipe.output_settings[0].response = PatternGeometryResponse::Regions(response);
    recipe
}

/// Builds the normalized finite four-edge diamond used by the triagrid mark card.
fn diamond_shape() -> AuthoredStructureDraft {
    let points = [
        AuthoredPoint2 { x: 0.0, y: -1.0 },
        AuthoredPoint2 { x: 1.0, y: 0.0 },
        AuthoredPoint2 { x: 0.0, y: 1.0 },
        AuthoredPoint2 { x: -1.0, y: 0.0 },
    ];
    AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        (0..points.len())
            .map(|index| AuthoredCurveSegment::Line {
                start: points[index],
                end: points[(index + 1) % points.len()],
            })
            .collect(),
    )
    .expect("fixed normalized diamond is finite and closed")
}

/// Builds the accepted asymmetric open three-line Curve Motif payload.
fn curve_motif_shape() -> AuthoredStructureDraft {
    let points = [
        AuthoredPoint2 { x: 0.0, y: 0.0 },
        AuthoredPoint2 { x: 0.32, y: 0.27 },
        AuthoredPoint2 { x: 0.7, y: -0.18 },
        AuthoredPoint2 { x: 1.0, y: 0.0 },
    ];
    AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        points
            .windows(2)
            .map(|points| AuthoredCurveSegment::Line {
                start: points[0],
                end: points[1],
            })
            .collect(),
    )
    .expect("fixed asymmetric Curve Motif is a valid open path")
}

/// Gallery-only preset metadata that remains outside persistence and evaluation identity.
#[derive(Clone, Debug, PartialEq)]
pub struct PresetCatalogEntry {
    pub preset: PresetRecord,
    pub required_features: Vec<PatternCapabilityFlag>,
}

/// The storage authority of one layered preset record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetOrigin {
    /// One immutable recipe bundled with the application.
    BuiltIn,
    /// One current-version recipe loaded from the active personal library.
    Personal,
}

/// One validated preset record together with its immutable catalog origin.
#[derive(Clone, Debug, PartialEq)]
pub struct LayeredPresetEntry {
    pub origin: PresetOrigin,
    pub preset: PresetRecord,
}

/// A deterministic combined built-in and personal preset catalog.
#[derive(Clone, Debug, PartialEq)]
pub struct LayeredPresetCatalog {
    entries: Vec<LayeredPresetEntry>,
    warnings: Vec<LayeredPresetCatalogWarning>,
}

/// One isolated personal-preset conflict that keeps immutable bundled authority usable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayeredPresetCatalogWarning {
    pub id: String,
    pub message: String,
}

/// Stable validation failure for combining built-in and personal preset records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayeredPresetCatalogError {
    message: String,
}

impl LayeredPresetCatalogError {
    /// Returns the stable combined-catalog validation message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for LayeredPresetCatalogError {
    /// Formats the stable combined-catalog validation message.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for LayeredPresetCatalogError {}

impl LayeredPresetCatalog {
    /// Combines immutable built-ins with validated personal preset-v4 records.
    ///
    /// # Errors
    ///
    /// Returns a stable error when a built-in authority is invalid, a personal ID is not
    /// canonical, a personal record is invalid, or personal records collide with one another.
    /// A personal name that collides with an immutable built-in is instead isolated as a warning.
    pub fn new(
        built_ins: &PresetRegistry,
        mut personal: Vec<PresetRecord>,
    ) -> Result<Self, LayeredPresetCatalogError> {
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for record in built_ins.entries() {
            toniator_domain::validate_preset_record(record).map_err(|error| {
                LayeredPresetCatalogError {
                    message: format!("bundled preset authority is invalid: {error}"),
                }
            })?;
            if !ids.insert(record.metadata.id.clone())
                || !names.insert(record.metadata.name.to_lowercase())
            {
                return Err(LayeredPresetCatalogError {
                    message: "bundled preset authority has duplicate IDs or names".into(),
                });
            }
        }
        for record in &personal {
            if !toniator_domain::is_personal_library_id(&record.metadata.id) {
                return Err(LayeredPresetCatalogError {
                    message:
                        "personal preset IDs must use the canonical user-<lowercase UUID> form"
                            .into(),
                });
            }
            toniator_domain::validate_preset_record(record).map_err(|error| {
                LayeredPresetCatalogError {
                    message: error.to_string(),
                }
            })?;
        }
        personal.sort_by(|left, right| {
            left.metadata
                .name
                .to_lowercase()
                .cmp(&right.metadata.name.to_lowercase())
                .then_with(|| left.metadata.id.cmp(&right.metadata.id))
        });
        let mut entries = Vec::with_capacity(built_ins.entries().len() + personal.len());
        entries.extend(
            built_ins
                .entries()
                .iter()
                .cloned()
                .map(|preset| LayeredPresetEntry {
                    origin: PresetOrigin::BuiltIn,
                    preset,
                }),
        );
        let mut warnings = Vec::new();
        for record in personal {
            if ids.contains(&record.metadata.id) {
                return Err(LayeredPresetCatalogError {
                    message: "preset IDs must be unique across the layered catalog".into(),
                });
            }
            let normalized_name = record.metadata.name.to_lowercase();
            if names.contains(&normalized_name) {
                if built_ins
                    .entries()
                    .iter()
                    .any(|built_in| built_in.metadata.name.to_lowercase() == normalized_name)
                {
                    warnings.push(LayeredPresetCatalogWarning {
                        id: record.metadata.id,
                        message: "personal preset name conflicts with an immutable built-in".into(),
                    });
                    continue;
                }
                return Err(LayeredPresetCatalogError {
                    message:
                        "personal preset names must be case-insensitively unique across the layered catalog"
                            .into(),
                });
            }
            ids.insert(record.metadata.id.clone());
            names.insert(normalized_name);
            entries.push(LayeredPresetEntry {
                origin: PresetOrigin::Personal,
                preset: record,
            });
        }
        Ok(Self { entries, warnings })
    }

    /// Returns records in deterministic built-in-first and personal-name order.
    pub fn entries(&self) -> &[LayeredPresetEntry] {
        &self.entries
    }

    /// Returns isolated nonfatal personal-preset conflicts in deterministic personal-name order.
    pub fn warnings(&self) -> &[LayeredPresetCatalogWarning] {
        &self.warnings
    }

    /// Finds one record solely by its stable identifier.
    pub fn find(&self, id: &str) -> Option<&LayeredPresetEntry> {
        self.entries
            .iter()
            .find(|entry| entry.preset.metadata.id == id)
    }

    /// Reconstructs one ID-free recipe without display-name behavior.
    pub fn reconstruct(&self, id: &str) -> Option<PatternDefinitionRecipe> {
        self.find(id).map(|entry| entry.preset.recipe.clone())
    }

    /// Applies one layered record to a selected channel through the authoritative history boundary.
    ///
    /// # Errors
    ///
    /// Returns the ordinary document-session failure, including a stable unknown-ID validation
    /// error, without mutating history for failed lookup or validation.
    pub fn apply_to_selected(
        &self,
        history: &mut DocumentHistory,
        channel_id: ChannelId,
        id: &str,
    ) -> Result<toniator_domain::CommandResult, DocumentSessionError> {
        let recipe = self.recipe_for(id)?;
        let base_definition = history
            .document()
            .pattern_definition_for(channel_id)
            .cloned()
            .ok_or_else(|| unknown_channel_error(channel_id))?;
        history.apply(
            &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base: history.document().pattern_settings().clone(),
                channel_id,
                base_definition,
                recipe,
            },
        )
    }

    /// Applies one layered record as the document base through the authoritative history boundary.
    ///
    /// # Errors
    ///
    /// Returns the ordinary document-session failure, including a stable unknown-ID validation
    /// error, without mutating history for failed lookup or validation.
    pub fn apply_to_document_base(
        &self,
        history: &mut DocumentHistory,
        id: &str,
    ) -> Result<toniator_domain::CommandResult, DocumentSessionError> {
        let recipe = self.recipe_for(id)?;
        let base = history.document().pattern_settings().clone();
        let base_definition = history
            .document()
            .pattern_definition_bundles()
            .iter()
            .find(|definition| definition.id == base.definition_id)
            .map(|bundle| bundle.definition.clone())
            .ok_or_else(missing_document_base_definition_error)?;
        history.apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe,
        })
    }

    /// Resolves one recipe or returns the stable unknown-ID session error.
    fn recipe_for(&self, id: &str) -> Result<PatternDefinitionRecipe, DocumentSessionError> {
        self.reconstruct(id).ok_or_else(|| {
            DocumentSessionError::Validation(toniator_domain::ValidationError::new(
                "preset.id",
                "preset ID is not present in this catalog",
            ))
        })
    }
}

/// Builds the stable selected-channel lookup failure without relying on document invariants.
fn unknown_channel_error(_channel_id: ChannelId) -> DocumentSessionError {
    DocumentSessionError::Validation(toniator_domain::ValidationError::new(
        "preset.channel_id",
        "channel is not present in this document",
    ))
}

/// Builds the stable missing-base-definition failure without panicking on corrupted history state.
fn missing_document_base_definition_error() -> DocumentSessionError {
    DocumentSessionError::Validation(toniator_domain::ValidationError::new(
        "preset.document_base.definition_id",
        "document base definition is not present in this document",
    ))
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
        let catalog_entries = entries
            .iter()
            .cloned()
            .map(|preset| PresetCatalogEntry {
                preset,
                required_features: Vec::new(),
            })
            .collect();
        Ok(Self {
            version,
            entries,
            catalog_entries,
        })
    }

    /// Returns the versioned built-in pure-schema registry in stable ID order.
    pub fn bundled() -> Self {
        let coverage = || toniator_domain::CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        };
        let metadata = |id: &str, name: &str, category: &str, description: &str| PresetMetadata {
            id: id.into(),
            name: name.into(),
            category: category.into(),
            description: description.into(),
            thumbnail: None,
        };
        let grid = |name: &str, angles: &[f64], along: bool| {
            PatternStructureRecipe::GeneralizedStraightGuides {
                name: name.into(),
                coverage: coverage(),
                dimensions: angles
                    .iter()
                    .map(|angle| GuideDimensionDraft {
                        baseline_angle_degrees: *angle,
                        phase: 0.0,
                        spacing_multiplier: 1.0,
                    })
                    .collect(),
                product: if along {
                    toniator_domain::GeneralizedSiteProductDraft::AlongGuides {
                        dimension_indices: vec![0],
                        interval_multiplier: 1.0,
                        phase: 0.0,
                    }
                } else {
                    toniator_domain::GeneralizedSiteProductDraft::Intersections {
                        dimension_indices: (0..angles.len()).collect(),
                        merge_epsilon: 1e-9,
                    }
                },
                orientation: MarkOrientationDraft::Fixed,
            }
        };
        let tangent_grid = |name: &str, angles: &[f64]| {
            let mut recipe = grid(name, angles, false);
            let PatternStructureRecipe::GeneralizedStraightGuides { orientation, .. } = &mut recipe
            else {
                unreachable!("grid helper always returns generalized straight guides")
            };
            *orientation = MarkOrientationDraft::GuideTangent { dimension_index: 0 };
            recipe
        };
        let random = |name: &str, character: RandomSiteCharacter, seed: u32| {
            PatternStructureRecipe::RandomSites {
                name: name.into(),
                coverage: coverage(),
                character,
                seed,
                density_modulation: SiteDensityModulation::Uniform,
                exclusion: SiteExclusionPolicy::None,
                maximum_attempts: 16_000_000,
                maximum_neighbor_checks: 16_000_000,
            }
        };
        let adjacency = toniator_domain::ConnectionAdjacencyIntent {
            maximum_degree: 6,
            maximum_distance: 48.0,
        };
        let mut registry = Self::new(
            BUNDLED_PRESET_REGISTRY_VERSION,
            vec![
                PresetRecord {
                    metadata: metadata(
                        "clustered-dispersion-random-links",
                        "Clustered Connections",
                        "Connections",
                        "Clusters of marks joined by short, repeatable random connections.",
                    ),
                    recipe: path_recipe(PatternStructureRecipe::ConnectionPaths {
                        definition: Box::new(random(
                            "Clustered connections",
                            RandomSiteCharacter::Clustered {
                                cluster_density: 1.0,
                                cluster_spread: 16.0,
                                cluster_strength: 0.5,
                            },
                            43,
                        )),
                        program: ConnectionProgram::RandomLinks {
                            adjacency: toniator_domain::ConnectionAdjacencyIntent {
                                maximum_degree: 3,
                                maximum_distance: 48.0,
                            },
                            minimum_degree: 0,
                            seed: 43,
                        },
                        style: Default::default(),
                    }),
                },
                PresetRecord {
                    metadata: metadata(
                        "curve-motif-rows",
                        "Curve Motif",
                        "Guides",
                        "A custom curve repeated end to end across evenly spaced rows.",
                    ),
                    recipe: PatternDefinitionRecipe::connected(
                        PatternStructureRecipe::AuthoredResources {
                            resources: vec![curve_motif_shape()],
                            definition: Box::new(PatternStructureRecipe::CurveMotifPaths {
                            definition: Box::new(
                                PatternStructureRecipe::GeneralizedStraightGuides {
                                    name: "Curve motif rows".into(),
                                    coverage: coverage(),
                                    dimensions: vec![GuideDimensionDraft {
                                        baseline_angle_degrees: 0.0,
                                        phase: 0.125,
                                        spacing_multiplier: 1.0,
                                    }],
                                    product: toniator_domain::GeneralizedSiteProductDraft::AlongGuides {
                                        dimension_indices: vec![0],
                                        interval_multiplier: 1.0,
                                        phase: 0.25,
                                    },
                                    orientation: MarkOrientationDraft::GuideTangent {
                                        dimension_index: 0,
                                    },
                                },
                            ),
                            resource_index: 0,
                            style: Default::default(),
                            mirror_alternate_rows: true,
                            alternate_row_phase: Some(0.25),
                            }),
                        },
                    ),
                },
                PresetRecord {
                    metadata: metadata(
                        "even-random-circles",
                        "Even Dispersion Marks",
                        "Dispersion",
                        "Evenly spaced circles in a repeatable random arrangement.",
                    ),
                    recipe: mark_recipe(random(
                        "Even random circles",
                        RandomSiteCharacter::Even {
                            minimum_center_distance: 8.0,
                        },
                        19,
                    )),
                },
                PresetRecord {
                    metadata: metadata(
                        "grid-voronoi-scale",
                        "Grid Voronoi",
                        "Regions",
                        "A rectangular grid divided into cells whose size follows the artwork tone.",
                    ),
                    recipe: voronoi_recipe(
                        grid("Grid Voronoi", &[0.0, 90.0], false),
                        toniator_domain::RegionGeometryResponse {
                            algorithm: RegionResizeAlgorithm::Scale,
                            sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
                            minimum_fill: 0.0,
                            maximum_fill: 1.0,
                        },
                    ),
                },
                PresetRecord {
                    metadata: metadata(
                        "one-guide-lines",
                        "One Guide Lines",
                        "Guides",
                        "Evenly spaced straight lines in one direction.",
                    ),
                    recipe: path_recipe(PatternStructureRecipe::OrderedOutputs {
                        definition: Box::new(grid("One guide lines", &[0.0], true)),
                        outputs: vec![
                            toniator_domain::PatternOutputRealizationRecipe::StructuralPaths {
                                style: Default::default(),
                            },
                        ],
                    }),
                },
                PresetRecord {
                    metadata: metadata(
                        "residual-sites-along-guide",
                        "Connected and Residual Sites",
                        "Composites",
                        "Nearby sites are joined first, with circles drawn at the unused sites.",
                    ),
                    recipe: composite_connection_marks(
                        grid("Residual guide sites", &[0.0], true),
                        ConnectionProgram::NearestLinks {
                            adjacency: toniator_domain::ConnectionAdjacencyIntent {
                                maximum_degree: 3,
                                maximum_distance: 48.0,
                            },
                        },
                        true,
                    ),
                },
                PresetRecord {
                    metadata: metadata(
                        "round-spiral-line",
                        "Round Spiral Line",
                        "Parametric",
                        "One round spiral line that expands to cover the artwork.",
                    ),
                    recipe: path_recipe(PatternStructureRecipe::ParametricCurve {
                        name: "Round spiral line".into(),
                        coverage: coverage(),
                        curve: ParametricCurve::Spiral(SpiralCurve {
                            shape: SpiralShape::Round,
                            turns: 5.0,
                            radial_spacing: 16.0,
                            phase_degrees: 0.0,
                            winding: CurveWinding::Clockwise,
                        }),
                        spiral_coverage: toniator_domain::SpiralCoveragePolicy::CoverCanvas,
                        repetition: CurveRepetition::Single,
                        sites: None,
                    }),
                },
                PresetRecord {
                    metadata: metadata(
                        "round-spiral-marks",
                        "Round Spiral Marks",
                        "Parametric",
                        "Circles spaced evenly along a round spiral.",
                    ),
                    recipe: mark_recipe(PatternStructureRecipe::ParametricCurve {
                        name: "Round spiral marks".into(),
                        coverage: coverage(),
                        curve: ParametricCurve::Spiral(SpiralCurve {
                            shape: SpiralShape::Round,
                            turns: 5.0,
                            radial_spacing: 16.0,
                            phase_degrees: 0.0,
                            winding: CurveWinding::Clockwise,
                        }),
                        spiral_coverage: toniator_domain::SpiralCoveragePolicy::CoverCanvas,
                        repetition: CurveRepetition::Single,
                        sites: Some(toniator_domain::ParametricCurveSiteDraft {
                            interval: 16.0,
                            phase: 0.0,
                        }),
                    }),
                },
                PresetRecord {
                    metadata: metadata(
                        "source-weighted-dispersion-voronoi",
                        "Source-Weighted Voronoi",
                        "Regions",
                        "Voronoi cells gather according to the light and dark areas of the artwork.",
                    ),
                    recipe: voronoi_recipe(
                        PatternStructureRecipe::RandomSites {
                            name: "Source weighted Voronoi".into(),
                            coverage: coverage(),
                            character: RandomSiteCharacter::RawUniform,
                            seed: 23,
                            density_modulation: SiteDensityModulation::ArtworkWeighted {
                                mapping: toniator_domain::SourceMapping::canonical(
                                    toniator_domain::SourceMappingComponent::Luminance,
                                ),
                                strength: 0.75,
                                response: ArtworkWeightResponse::Linear,
                            },
                            exclusion: SiteExclusionPolicy::None,
                            maximum_attempts: 16_000_000,
                            maximum_neighbor_checks: 16_000_000,
                        },
                        toniator_domain::RegionGeometryResponse {
                            algorithm: RegionResizeAlgorithm::Scale,
                            sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
                            minimum_fill: 0.0,
                            maximum_fill: 1.0,
                        },
                    ),
                },
                PresetRecord {
                    metadata: metadata(
                        "square-spiral-marks",
                        "Square Spiral Marks",
                        "Parametric",
                        "Circles spaced evenly along a square spiral.",
                    ),
                    recipe: mark_recipe(PatternStructureRecipe::ParametricCurve {
                        name: "Square spiral marks".into(),
                        coverage: coverage(),
                        curve: ParametricCurve::Spiral(SpiralCurve {
                            shape: SpiralShape::Square,
                            turns: 5.0,
                            radial_spacing: 16.0,
                            phase_degrees: 0.0,
                            winding: CurveWinding::Clockwise,
                        }),
                        spiral_coverage: toniator_domain::SpiralCoveragePolicy::CoverCanvas,
                        repetition: CurveRepetition::Single,
                        sites: Some(toniator_domain::ParametricCurveSiteDraft {
                            interval: 16.0,
                            phase: 0.0,
                        }),
                    }),
                },
                PresetRecord {
                    metadata: metadata(
                        "straight-grid-circles",
                        "Straight Grid Circles",
                        "Marks",
                        "Circles placed where horizontal and vertical guides cross.",
                    ),
                    recipe: mark_recipe(tangent_grid("Straight grid circles", &[0.0, 90.0])),
                },
                PresetRecord {
                    metadata: metadata(
                        "three-guide-cells-scale",
                        "Three-Guide Cells",
                        "Regions",
                        "Triangular cells formed by three evenly spaced guide directions.",
                    ),
                    recipe: guide_face_recipe(
                        grid("Three guide cells", &[0.0, 60.0, 120.0], false),
                        vec![0, 1, 2],
                        toniator_domain::RegionGeometryResponse {
                            algorithm: RegionResizeAlgorithm::Scale,
                            sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
                            minimum_fill: 0.65,
                            maximum_fill: 0.90,
                        },
                    ),
                },
                PresetRecord {
                    metadata: metadata(
                        "three-guide-maze",
                        "Three-Guide Maze",
                        "Connections",
                        "A maze built on a triangular guide grid.",
                    ),
                    recipe: path_recipe(PatternStructureRecipe::MazeWalls {
                        definition: Box::new(grid("Three guide maze", &[0.0, 60.0, 120.0], false)),
                        program: toniator_domain::MazeProgram {
                            algorithm: toniator_domain::GridMazeAlgorithm::RecursiveBacktracker,
                            seed: 31,
                        },
                        style: Default::default(),
                    }),
                },
                PresetRecord {
                    metadata: metadata(
                        "triagrid-custom-shape-marks",
                        "Triagrid Diamond Marks",
                        "Marks",
                        "Diamond marks placed where three guide directions cross.",
                    ),
                    recipe: mark_recipe(PatternStructureRecipe::AuthoredResources {
                        resources: vec![diamond_shape()],
                        definition: Box::new(PatternStructureRecipe::AuthoredClosedShapeMarks {
                            definition: Box::new(grid(
                                "Triagrid diamond marks",
                                &[0.0, 60.0, 120.0],
                                false,
                            )),
                            resource_index: 0,
                        }),
                    }),
                },
                PresetRecord {
                    metadata: metadata(
                        "triagrid-spanning-tree",
                        "Triagrid Spanning Tree",
                        "Connections",
                        "A branching tree of connections built on a triangular guide grid.",
                    ),
                    recipe: path_recipe(PatternStructureRecipe::ConnectionPaths {
                        definition: Box::new(grid(
                            "Triagrid spanning tree",
                            &[0.0, 60.0, 120.0],
                            false,
                        )),
                        program: ConnectionProgram::GridSpanningTree {
                            adjacency,
                            algorithm: toniator_domain::GridSpanningTreeAlgorithm::RandomizedPrim,
                            seed: 47,
                        },
                        style: Default::default(),
                    }),
                },
                PresetRecord {
                    metadata: metadata(
                        "two-guide-cells-uniform-offset",
                        "Two-Guide Cells",
                        "Regions",
                        "Rectangular cells formed by two perpendicular guide directions.",
                    ),
                    recipe: guide_face_recipe(
                        grid("Two guide cells", &[0.0, 90.0], false),
                        vec![0, 1],
                        toniator_domain::RegionGeometryResponse {
                            algorithm: RegionResizeAlgorithm::UniformOffset,
                            sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                            minimum_fill: 0.0,
                            maximum_fill: 1.0,
                        },
                    ),
                },
                PresetRecord {
                    metadata: metadata(
                        "two-guide-maze",
                        "Two-Guide Maze",
                        "Connections",
                        "A maze built on a rectangular guide grid.",
                    ),
                    recipe: path_recipe(PatternStructureRecipe::MazeWalls {
                        definition: Box::new(grid("Two guide maze", &[0.0, 90.0], false)),
                        program: toniator_domain::MazeProgram {
                            algorithm: toniator_domain::GridMazeAlgorithm::RecursiveBacktracker,
                            seed: 29,
                        },
                        style: Default::default(),
                    }),
                },
            ],
        )
        .expect("bundled preset literals satisfy registry validation");
        let requirement_document = Document::new_default_document(
            CanvasSpec {
                width: 120.0,
                height: 80.0,
            },
            SourceReference::Unassigned,
        )
        .expect("fixed catalog requirement document validates");
        registry.catalog_entries = registry
            .entries
            .iter()
            .cloned()
            .map(|preset| {
                let base = requirement_document.pattern_settings().clone();
                let base_definition = requirement_document.pattern_definition_bundles()[0]
                    .definition
                    .clone();
                let (candidate, _) = requirement_document
                    .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                        base,
                        base_definition,
                        recipe: preset.recipe.clone(),
                    })
                    .expect("validated bundled recipe materializes for its requirements");
                let required_features = candidate
                    .pattern_capabilities(PatternCapabilityScope::DocumentBase)
                    .expect("materialized bundled recipe projects requirements")
                    .features;
                PresetCatalogEntry {
                    required_features,
                    preset,
                }
            })
            .collect();
        registry
    }

    /// Returns this registry format version without exposing mutable state.
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns records in their validated stable order.
    pub fn entries(&self) -> &[PresetRecord] {
        &self.entries
    }

    /// Returns nonserialized catalog records and their gallery-only feature requirements.
    pub fn catalog_entries(&self) -> &[PresetCatalogEntry] {
        &self.catalog_entries
    }

    /// Returns catalog entries whose cloned reconstruction projects every requested feature.
    ///
    /// Failure to reconstruct is treated as unavailable so callers never need
    /// a frontend diagnostic inventory for unsupported cards.
    pub fn available_for(
        &self,
        document: &Document,
        scope: PatternCapabilityScope,
    ) -> Vec<&PresetCatalogEntry> {
        self.catalog_entries
            .iter()
            .filter(|entry| {
                let command = match scope {
                    PatternCapabilityScope::DocumentBase => {
                        let base = document.pattern_settings().clone();
                        let Some(base_definition) = document
                            .pattern_definition_bundles()
                            .iter()
                            .find(|bundle| bundle.definition.id == base.definition_id)
                            .map(|bundle| bundle.definition.clone())
                        else {
                            return false;
                        };
                        DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                            base,
                            base_definition,
                            recipe: entry.preset.recipe.clone(),
                        }
                    }
                    PatternCapabilityScope::Channel(channel_id) => {
                        let Some(base_definition) =
                            document.pattern_definition_for(channel_id).cloned()
                        else {
                            return false;
                        };
                        DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                            base: document.pattern_settings().clone(),
                            channel_id,
                            base_definition,
                            recipe: entry.preset.recipe.clone(),
                        }
                    }
                };
                document
                    .apply_command(&command)
                    .ok()
                    .and_then(|(candidate, _)| candidate.pattern_capabilities(scope).ok())
                    .is_some_and(|projection| projection.supports_all(&entry.required_features))
            })
            .collect()
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
            .pattern_definition_bundles()
            .iter()
            .find(|definition| definition.id == base.definition_id)
            .expect("validated document base definition")
            .definition
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
            .pattern_definition_bundles()
            .iter()
            .find(|definition| definition.id == definition_id)
            .ok_or_else(|| {
                DocumentSessionError::Validation(toniator_domain::ValidationError::new(
                    "pattern_definitions.id",
                    "shared preset replacement targets a missing definition",
                ))
            })?
            .definition
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

/// Builds the approved connection-plus-residual-marks painter-order composite.
fn composite_connection_marks(
    definition: PatternStructureRecipe,
    program: ConnectionProgram,
    residual: bool,
) -> PatternDefinitionRecipe {
    PatternDefinitionRecipe {
        structure: PatternStructureRecipe::OrderedOutputs {
            definition: Box::new(definition),
            outputs: vec![
                toniator_domain::PatternOutputRealizationRecipe::ConnectionPaths {
                    program,
                    style: Default::default(),
                },
                toniator_domain::PatternOutputRealizationRecipe::Marks,
            ],
        },
        output_settings: vec![
            toniator_domain::PatternOutputSettingsRecipe {
                source_filter: toniator_domain::SiteUseFilterRecipe::All,
                response: PatternGeometryResponse::Connected(
                    toniator_domain::ConnectedGeometryResponse {
                        minimum_thickness: 0.15,
                        maximum_thickness: 0.65,
                        bias: 0.0,
                    },
                ),
            },
            toniator_domain::PatternOutputSettingsRecipe {
                source_filter: if residual {
                    toniator_domain::SiteUseFilterRecipe::SitesUnusedBy { output_index: 0 }
                } else {
                    toniator_domain::SiteUseFilterRecipe::SitesUsedBy { output_index: 0 }
                },
                response: PatternGeometryResponse::Marks(toniator_domain::MarkGeometryResponse {
                    minimum_fill: 0.25,
                    maximum_fill: 0.85,
                }),
            },
        ],
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
    /// Authored dependency intent applied to the complete guard-inclusive family.
    pub source_filter: SiteUseFilter,
    pub consumes: StructuralProductCapability,
    /// The mutually exclusive output authority; guide paths never masquerade as marks.
    pub payload: OutputCapabilityPayload,
}

/// Typed realization payload retained in cache/provenance identity.
#[derive(Clone, Debug, PartialEq)]
pub enum OutputCapabilityPayload {
    Marks {
        prototype: MarkPrototype,
        orientation: MarkOrientation,
    },
    GuidePaths {
        guide_mechanism_id: PatternMechanismId,
        style: toniator_domain::PathStrokeStyle,
    },
    /// Curve Motif carries only document-authoritative recipe fields until row paths are derived.
    CurveMotifPaths {
        site_mechanism_id: PatternMechanismId,
        structure_id: AuthoredStructureId,
        style: toniator_domain::PathStrokeStyle,
        mirror_alternate_rows: bool,
        alternate_row_phase: Option<f64>,
    },
    /// Connection selection remains authored while graph/path products are derived at evaluation time.
    ConnectionPaths {
        site_mechanism_id: PatternMechanismId,
        program: ConnectionProgram,
        style: toniator_domain::PathStrokeStyle,
    },
    /// Conventional wall-maze selection remains authored while its arrangement and retained walls are derived.
    MazeWalls {
        site_mechanism_id: PatternMechanismId,
        program: MazeProgram,
        style: toniator_domain::PathStrokeStyle,
    },
    /// Fixed canonical regions retain their typed source rather than inferring it from sites.
    Regions { source: RegionSourceIntent },
}

impl Eq for OutputCapabilityPayload {}

impl OutputCapability {
    /// Returns mark authority only when this output is a mark layer.
    pub fn marks(&self) -> Option<(&MarkPrototype, &MarkOrientation)> {
        match &self.payload {
            OutputCapabilityPayload::Marks {
                prototype,
                orientation,
            } => Some((prototype, orientation)),
            OutputCapabilityPayload::GuidePaths { .. } => None,
            OutputCapabilityPayload::CurveMotifPaths { .. } => None,
            OutputCapabilityPayload::ConnectionPaths { .. } => None,
            OutputCapabilityPayload::MazeWalls { .. } => None,
            OutputCapabilityPayload::Regions { .. } => None,
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
            OutputCapabilityPayload::CurveMotifPaths { .. } => None,
            OutputCapabilityPayload::ConnectionPaths { .. } => None,
            OutputCapabilityPayload::MazeWalls { .. } => None,
            OutputCapabilityPayload::Regions { .. } => None,
        }
    }

    /// Returns connection authority only when this output consumes a site product as paths.
    pub fn connection_paths(
        &self,
    ) -> Option<(
        PatternMechanismId,
        &ConnectionProgram,
        toniator_domain::PathStrokeStyle,
    )> {
        match &self.payload {
            OutputCapabilityPayload::ConnectionPaths {
                site_mechanism_id,
                program,
                style,
            } => Some((*site_mechanism_id, program, *style)),
            _ => None,
        }
    }

    /// Returns wall-maze authority only when this output consumes typed straight intersections.
    pub fn maze_walls(
        &self,
    ) -> Option<(
        PatternMechanismId,
        &MazeProgram,
        toniator_domain::PathStrokeStyle,
    )> {
        match &self.payload {
            OutputCapabilityPayload::MazeWalls {
                site_mechanism_id,
                program,
                style,
            } => Some((*site_mechanism_id, program, *style)),
            _ => None,
        }
    }

    /// Returns fixed-region source authority only when this output is a region layer.
    pub fn regions(&self) -> Option<&RegionSourceIntent> {
        match &self.payload {
            OutputCapabilityPayload::Regions { source } => Some(source),
            _ => None,
        }
    }
}

/// Chains an open motif between adjacent Along Guides sites in deterministic guide-instance order.
///
/// Rows are grouped by their stable guide provenance, not by site ordinal. Odd negative guide
/// indices use Euclidean parity. The returned paths retain the shared endpoint exactly once per
/// join and remain open so the ordinary canonical stroke realizer owns sampling and outlines.
///
/// # Errors
///
/// Returns a stable motif diagnostic when sites lack consecutive Along Guides provenance, the
/// motif endpoint basis is degenerate, a row cannot form finite transformed segments, or a
/// cancellation request arrives before output publication.
///
/// # Panics
///
/// Panics only if the caller's synchronized progress callback poisons the internal reporting lock.
pub fn chain_curve_motif_rows_cancellable(
    sites: &FamilySiteSet,
    motif: &CurvePath,
    mirror_alternate_rows: bool,
    alternate_row_phase: Option<f64>,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<CurvePath>, PatternPipelineError> {
    chain_curve_motif_rows_with_progress_cancellable(
        sites,
        motif,
        mirror_alternate_rows,
        alternate_row_phase,
        is_cancelled,
        &|_, _| {},
    )
}

/// Chains motif rows while reporting completed stable guide rows in output order.
///
/// Straight and authored-curve guide sites retain their complete guide/component identity,
/// so disconnected paths never acquire an invented joining motif. Each copy uses the existing
/// adjacent-site chord mapping; curved guides do not introduce a separate deformation model.
/// The callback observes only completed rows and never changes row geometry,
/// Rayon scheduling, or the ordered result. It may be called from worker
/// threads and therefore must be thread-safe.
///
/// # Errors
///
/// Returns invalid motif, cadence, provenance, allocation, or cancellation diagnostics before
/// publishing any partial row collection.
///
/// # Panics
///
/// Panics only if the caller's synchronized progress callback poisons the internal reporting lock.
fn chain_curve_motif_rows_with_progress_cancellable(
    sites: &FamilySiteSet,
    motif: &CurvePath,
    mirror_alternate_rows: bool,
    alternate_row_phase: Option<f64>,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<Vec<CurvePath>, PatternPipelineError> {
    let start = motif.start();
    let end = motif.end();
    let source_axis = Vector2::new(end.x - start.x, end.y - start.y);
    let source_length_squared = source_axis.dot(source_axis);
    if !source_length_squared.is_finite() || source_length_squared <= 0.0 {
        return Err(PatternPipelineError::new(
            "curve_motif.endpoints",
            "Curve Motif requires distinct finite path endpoints",
        ));
    }
    if alternate_row_phase.is_some_and(|value| !(value.is_finite() && 0.0 < value && value < 1.0)) {
        return Err(PatternPipelineError::new(
            "curve_motif.alternate_row_phase",
            "Curve Motif alternate row phase must be finite and strictly between zero and one",
        ));
    }
    let mut rows = BTreeMap::<(GuideInstanceId, bool), Vec<(i64, Point2)>>::new();
    for site in sites.sites() {
        let row_site = match site.provenance {
            FamilySiteProvenance::AlongGuide {
                guide_id, sequence, ..
            } => Some((guide_id, false, sequence)),
            FamilySiteProvenance::CurveAlongGuide {
                location, sequence, ..
            } => location
                .path
                .guide_instance()
                .map(|guide_id| (guide_id, true, sequence)),
            _ => None,
        };
        let (guide_id, curved, sequence) = row_site.ok_or_else(|| {
            PatternPipelineError::new(
                "curve_motif.sites",
                "Curve Motif requires sites placed along a guide",
            )
        })?;
        rows.entry((guide_id, curved))
            .or_default()
            .push((sequence, site.position));
    }
    let mut row_specs = Vec::new();
    for ((guide_id, curved), mut row) in rows {
        row.sort_by_key(|(sequence, _)| *sequence);
        let mut run: Vec<(i64, Point2)> = Vec::new();
        for site in row {
            if let Some(previous) = run.last()
                && site.0.checked_sub(previous.0) != Some(1)
            {
                // A curved path may leave the generation envelope and return. Omitted cadence
                // positions split that path into runs; joining across the gap invents geometry.
                if !curved || site.0 == previous.0 {
                    return Err(PatternPipelineError::new(
                        "curve_motif.sites",
                        "Curve Motif guide sequences must be unique; straight rows must be consecutive",
                    ));
                }
                row_specs.push((guide_id, std::mem::take(&mut run)));
            }
            run.push(site);
        }
        row_specs.push((guide_id, run));
    }
    let motif_segment_count = motif.segments().len();
    let mut total_work = 0_usize;
    for (_, row) in &row_specs {
        let copies = row.len().saturating_sub(1);
        let segment_work = copies.checked_mul(motif_segment_count).ok_or_else(|| {
            PatternPipelineError::new(
                "curve_motif.allocation",
                "Curve Motif segment work count overflows addressable memory",
            )
        })?;
        total_work = total_work
            .checked_add(segment_work)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| {
                PatternPipelineError::new(
                    "curve_motif.allocation",
                    "Curve Motif progress work count overflows addressable memory",
                )
            })?;
    }
    let completed_work = AtomicUsize::new(0);
    let reported_work = AtomicUsize::new(0);
    let progress_lock = Mutex::new(());
    let report_completed_work = |increment: usize| {
        let completed = completed_work.fetch_add(increment, Ordering::AcqRel) + increment;
        let _guard = progress_lock
            .lock()
            .expect("Curve Motif progress lock poisoned");
        loop {
            let previous = reported_work.load(Ordering::Acquire);
            if completed <= previous {
                return;
            }
            if reported_work
                .compare_exchange(previous, completed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                progress(completed, total_work);
                return;
            }
        }
    };
    row_specs
        .par_iter()
        .map(|(guide_id, row)| {
            if is_cancelled() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "evaluation was cancelled",
                ));
            }
            let odd = guide_id.index.rem_euclid(2) == 1;
            let mirror = odd && mirror_alternate_rows;
            let phase = if odd {
                alternate_row_phase.unwrap_or(0.0)
            } else {
                0.0
            };
            let row_axis = row
                .windows(2)
                .next()
                .map(|pair| Vector2::new(pair[1].1.x - pair[0].1.x, pair[1].1.y - pair[0].1.y))
                .unwrap_or(Vector2::new(0.0, 0.0));
            let row_translate = row_axis.scale(phase);
            let mut segments = Vec::new();
            let copy_count = row.len().saturating_sub(1);
            let segment_capacity =
                copy_count
                    .checked_mul(motif.segments().len())
                    .ok_or_else(|| {
                        PatternPipelineError::new(
                            "curve_motif.allocation",
                            "Curve Motif segment count overflows addressable memory",
                        )
                    })?;
            reserve_stage20m(
                &mut segments,
                segment_capacity,
                "curve_motif.allocation",
                "Curve Motif segment allocation failed",
            )?;
            for pair in row.windows(2) {
                if is_cancelled() {
                    return Err(PatternPipelineError::new(
                        "evaluation.cancelled",
                        "evaluation was cancelled",
                    ));
                }
                let destination_axis =
                    Vector2::new(pair[1].1.x - pair[0].1.x, pair[1].1.y - pair[0].1.y);
                if !destination_axis.x.is_finite()
                    || !destination_axis.y.is_finite()
                    || destination_axis.dot(destination_axis) <= 0.0
                {
                    return Err(PatternPipelineError::new(
                        "curve_motif.cadence",
                        "Curve Motif adjacent sites must remain distinct and finite",
                    ));
                }
                let mapped_start = segments.last().map(CurveSegment::end).unwrap_or_else(|| {
                    Point2::new(pair[0].1.x + row_translate.x, pair[0].1.y + row_translate.y)
                });
                let mapped_end =
                    Point2::new(pair[1].1.x + row_translate.x, pair[1].1.y + row_translate.y);
                let map = |point: Point2| -> Result<Point2, PatternPipelineError> {
                    if point == start {
                        return Ok(mapped_start);
                    }
                    if point == end {
                        return Ok(mapped_end);
                    }
                    let relative = Vector2::new(point.x - start.x, point.y - start.y);
                    let along = relative.dot(source_axis) / source_length_squared;
                    let across = relative.dot(source_axis.perpendicular()) / source_length_squared;
                    let signed_across = if mirror { -across } else { across };
                    let normal = destination_axis.perpendicular();
                    let result = Point2::new(
                        pair[0].1.x
                            + row_translate.x
                            + destination_axis.x * along
                            + normal.x * signed_across,
                        pair[0].1.y
                            + row_translate.y
                            + destination_axis.y * along
                            + normal.y * signed_across,
                    );
                    result.is_finite().then_some(result).ok_or_else(|| {
                        PatternPipelineError::new(
                            "curve_motif.numeric",
                            "Curve Motif mapping must remain finite",
                        )
                    })
                };
                for segment in motif.segments() {
                    let transformed = match segment {
                        CurveSegment::Line(line) => CurveSegment::Line(
                            LineSegment::new(map(line.start())?, map(line.end())?).map_err(
                                |_| {
                                    PatternPipelineError::new(
                                        "curve_motif.numeric",
                                        "Curve Motif mapping must remain finite",
                                    )
                                },
                            )?,
                        ),
                        CurveSegment::CubicBezier(cubic) => CurveSegment::CubicBezier(
                            CubicBezierSegment::new(
                                map(cubic.start())?,
                                map(cubic.control_1())?,
                                map(cubic.control_2())?,
                                map(cubic.end())?,
                            )
                            .map_err(|_| {
                                PatternPipelineError::new(
                                    "curve_motif.numeric",
                                    "Curve Motif mapping must remain finite",
                                )
                            })?,
                        ),
                    };
                    segments.push(transformed);
                    report_completed_work(1);
                }
            }
            if segments.is_empty() {
                report_completed_work(1);
                return Ok(None);
            }
            let path = CurvePath::new(segments, PathClosure::Open).map_err(|_| {
                PatternPipelineError::new(
                    "curve_motif.continuity",
                    "Curve Motif rows must chain exact consecutive endpoints",
                )
            })?;
            report_completed_work(1);
            Ok(Some(path))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|rows| {
            let mut paths = Vec::new();
            reserve_stage20m(
                &mut paths,
                rows.len(),
                "curve_motif.allocation",
                "Curve Motif row allocation failed",
            )?;
            for row in rows.into_iter().flatten() {
                paths.push(row);
            }
            Ok(paths)
        })?
}

/// Realizes chained Curve Motif rows through the existing source-sampled variable-width outline machinery.
///
/// This path owns only motif centerline construction. It reuses the same profile sampling, round
/// outline, and canonical-stroke types as guide paths, so renderers retain no motif-specific code.
///
/// # Errors
///
/// Returns motif chaining, source sampling, cancellation, allocation, or outline diagnostics
/// without returning partially realized rows.
#[allow(clippy::too_many_arguments)]
pub fn realize_curve_motif_canonical_strokes_cancellable(
    family_fingerprint: &str,
    sites: &FamilySiteSet,
    motif: &CurvePath,
    structure_id: AuthoredStructureId,
    style: toniator_domain::PathStrokeStyle,
    mirror_alternate_rows: bool,
    alternate_row_phase: Option<f64>,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<CanonicalStrokeRealization, PatternPipelineError> {
    let rows = chain_curve_motif_rows_with_progress_cancellable(
        sites,
        motif,
        mirror_alternate_rows,
        alternate_row_phase,
        is_cancelled,
        progress,
    )?;
    let mut strokes = Vec::new();
    reserve_stage20m(
        &mut strokes,
        rows.len(),
        "curve_motif.allocation",
        "Curve Motif stroke allocation failed",
    )?;
    let mut profile_samples = 0_usize;
    let mut outline_segments = 0_usize;
    let mut identity = Fnv1a64State::new();
    identity.write(family_fingerprint.bytes());
    identity.write(CANONICAL_STROKE_OUTLINE_CONTRACT_ID.bytes());
    identity.write(structure_id.0.to_le_bytes());
    identity.write(u8::from(mirror_alternate_rows).to_le_bytes());
    identity.write(alternate_row_phase.unwrap_or(-1.0).to_bits().to_le_bytes());
    identity.write(response.minimum_thickness.to_bits().to_le_bytes());
    identity.write(response.maximum_thickness.to_bits().to_le_bytes());
    identity.write(response.bias.to_bits().to_le_bytes());
    for (row_ordinal, path) in rows.into_iter().enumerate() {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        let first_copy_end = path
            .segments()
            .get(motif.segments().len().saturating_sub(1))
            .ok_or(PatternPipelineError::new(
                "curve_motif.cadence",
                "Curve Motif row must retain one complete adjacent-site copy",
            ))?
            .end();
        let start = path.start();
        let nominal_basis = (first_copy_end.x - start.x).hypot(first_copy_end.y - start.y);
        if !nominal_basis.is_finite() || nominal_basis <= 0.0 {
            return Err(PatternPipelineError::new(
                "curve_motif.cadence",
                "Curve Motif row interval must be finite and positive",
            ));
        }
        let path_id = StructuralPathInstanceId::guide_dimension(
            GuideDimensionId(structure_id.0),
            i64::try_from(row_ordinal).map_err(|_| {
                PatternPipelineError::new(
                    "curve_motif.identity",
                    "Curve Motif row index exceeds structural identity bounds",
                )
            })?,
            0,
        );
        let pixel_footprint = (canvas.width / f64::from(source.identity().width))
            .min(canvas.height / f64::from(source.identity().height));
        let profile_sample_interval = nominal_basis.max(pixel_footprint);
        let mut profile = Vec::new();
        for (segment_index, segment) in path.segments().iter().enumerate() {
            let start_sample = stroke_sample(
                segment,
                segment_index,
                0.0,
                source,
                canvas,
                mapping,
                response,
                nominal_basis,
            )?;
            let end_sample = stroke_sample(
                segment,
                segment_index,
                1.0,
                source,
                canvas,
                mapping,
                response,
                nominal_basis,
            )?;
            if profile.last().is_none_or(|previous: &StrokeProfileSample| {
                previous.location != start_sample.location
            }) {
                if profile_samples >= max_profile_samples {
                    return Err(PatternPipelineError::new(
                        "curve_motif.profile_limit",
                        "Curve Motif profile exceeds the sample limit",
                    ));
                }
                profile.push(start_sample);
                profile_samples += 1;
            }
            append_adaptive_stroke_interval(
                segment,
                segment_index,
                0.0,
                start_sample,
                1.0,
                end_sample,
                0,
                profile_sample_interval,
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
            &path,
            &profile
                .iter()
                .map(|sample| VariableWidthPathSample {
                    location: sample.location,
                    width: sample.width,
                })
                .collect::<Vec<_>>(),
            style,
            response.bias,
            1.0 / 8.0,
            VariableWidthOutlineLimits::new(
                max_outline_segments.saturating_sub(outline_segments).max(1),
            )
            .map_err(|_| {
                PatternPipelineError::new(
                    "curve_motif.outline_limit",
                    "Curve Motif outline limit must be nonzero",
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
                "curve_motif.outline_limit",
                "Curve Motif outline exceeds the segment limit",
            ))?;
        if outline_segments > max_outline_segments {
            return Err(PatternPipelineError::new(
                "curve_motif.outline_limit",
                "Curve Motif outline exceeds the segment limit",
            ));
        }
        for sample in &profile {
            identity.write(sample.center.x.to_bits().to_le_bytes());
            identity.write(sample.center.y.to_bits().to_le_bytes());
            identity.write(sample.normalized_thickness.to_bits().to_le_bytes());
        }
        strokes.push(
            CanonicalStroke::new(
                path_id,
                Some(structure_id),
                path,
                nominal_basis,
                style,
                profile,
                outline,
            )
            .map_err(|_| {
                PatternPipelineError::new(
                    "curve_motif.stroke",
                    "Curve Motif canonical stroke geometry must remain finite",
                )
            })?,
        );
    }
    Ok(CanonicalStrokeRealization {
        family_fingerprint: family_fingerprint.to_owned(),
        realization_fingerprint: identity.finish(),
        source_identity: source.identity().clone(),
        response,
        strokes,
    })
}

/// Typed family/modulation/output plan. Modulation has no variants in the
/// accepted schema, but remains an explicit stage instead of a hidden no-op in
/// family or renderer code.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternPipelinePlan {
    pub family: FamilyCapability,
    pub modulation: PatternModulation,
    /// Authored painter order, retained independently from dependency evaluation.
    pub ordered_outputs: Vec<OutputCapability>,
    /// Deterministic dependency order, with stable output IDs breaking ready-node ties.
    pub evaluation_order: Vec<PatternOutputLayerId>,
}

/// Canonical derived membership published by one completed output realization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteUsageSet {
    site_mechanism_id: Option<PatternMechanismId>,
    members: Vec<FamilySiteId>,
    fingerprint: String,
}

impl SiteUsageSet {
    /// Canonicalizes one site-backed usage set into sorted unique stable family-site identities.
    ///
    /// # Errors
    ///
    /// Returns a stable mechanism diagnostic when a site-backed set uses a zero mechanism ID or
    /// contains an identity owned by another mechanism.
    pub fn new(
        site_mechanism_id: PatternMechanismId,
        mut members: Vec<FamilySiteId>,
    ) -> Result<Self, PatternPipelineError> {
        if site_mechanism_id.0 == 0
            || members
                .iter()
                .any(|member| member.mechanism_id != site_mechanism_id)
        {
            return Err(PatternPipelineError::new(
                "realization.site_usage.mechanism",
                "site usage members must share one exact nonzero site mechanism",
            ));
        }
        members.sort_unstable();
        members.dedup();
        Ok(Self::from_canonical(Some(site_mechanism_id), members))
    }

    /// Publishes the canonical empty usage identity for a structural non-site output.
    pub fn empty_non_site() -> Self {
        Self::from_canonical(None, Vec::new())
    }

    /// Builds one fingerprint from already canonical membership without exposing mutable state.
    fn from_canonical(
        site_mechanism_id: Option<PatternMechanismId>,
        members: Vec<FamilySiteId>,
    ) -> Self {
        let mut bytes = b"toniator-stage-20r-site-usage-v1".to_vec();
        bytes.extend(site_mechanism_id.map_or(0, |id| id.0).to_le_bytes());
        bytes.extend(
            u64::try_from(members.len())
                .expect("usize fits u64")
                .to_le_bytes(),
        );
        for member in &members {
            bytes.extend(member.mechanism_id.0.to_le_bytes());
            bytes.extend(
                u64::try_from(member.ordinal)
                    .expect("usize fits u64")
                    .to_le_bytes(),
            );
        }
        Self {
            site_mechanism_id,
            members,
            fingerprint: format!("toniator-stage-20r-site-usage-v1:{}", fnv1a64(bytes)),
        }
    }

    /// Returns the exact site mechanism identity, or `None` for structural non-site output.
    pub const fn site_mechanism_id(&self) -> Option<PatternMechanismId> {
        self.site_mechanism_id
    }

    /// Returns sorted unique stable members.
    pub fn members(&self) -> &[FamilySiteId] {
        &self.members
    }

    /// Returns the stable cache and dependency fingerprint for this membership set.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
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

/// One complete derived connection evaluation paired with its guard-inclusive family envelope.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionPathEvaluation {
    pub family: TypedFamilyOutput,
    pub graph: SiteAdjacencyGraph,
    pub paths: ConnectionPathSet,
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

    /// Applies one validated site-use filter to the complete guard-inclusive family.
    ///
    /// The returned view retains family order, stable site IDs, base-family identity, structural
    /// paths, and diagnostics. It never clips to the canvas or mutates the cached complete family.
    ///
    /// # Errors
    ///
    /// Returns stable missing-usage or mechanism diagnostics for malformed dependency input.
    pub fn filtered_for_output(
        &self,
        source_filter: SiteUseFilter,
        referenced_usage: Option<&SiteUsageSet>,
    ) -> Result<Self, PatternPipelineError> {
        if source_filter == SiteUseFilter::All {
            return Ok(self.clone());
        }
        let usage = referenced_usage.ok_or(PatternPipelineError::new(
            "realization.site_filter.reference",
            "dependent site filter requires completed referenced usage",
        ))?;
        if usage.site_mechanism_id() != Some(self.sites.product_mechanism_id()) {
            return Err(PatternPipelineError::new(
                "realization.site_filter.mechanism",
                "dependent usage must use the complete family's exact site mechanism",
            ));
        }
        let used = usage.members().iter().copied().collect::<BTreeSet<_>>();
        let members = match source_filter {
            SiteUseFilter::All => unreachable!("handled above"),
            SiteUseFilter::SitesUsedBy { .. } => used,
            SiteUseFilter::SitesUnusedBy { .. } => self
                .sites
                .iter()
                .map(|site| site.id)
                .filter(|id| !used.contains(id))
                .collect(),
        };
        Ok(Self {
            family: self.family.clone(),
            sites: self.sites.filtered(&members),
            diagnostics: self.diagnostics.clone(),
            structure: self.structure.clone(),
        })
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

/// One independently addressed output realization unit.
///
/// The family remains shared across outputs, but this record binds exactly one
/// structural capability to its matching domain-resolved response. Stage 20R orchestration
/// evaluates units by dependency order and aggregates them by authored painter order.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedOutputRealization<T> {
    pub output_layer_id: PatternOutputLayerId,
    pub capability: OutputCapability,
    pub effective_setting: EffectivePatternOutputSettings,
    pub realization: TypedRealization<T>,
}

impl PatternPipelinePlan {
    /// Resolves one structural output capability by its stable output-layer ID.
    pub fn output_capability(
        &self,
        output_layer_id: PatternOutputLayerId,
    ) -> Option<&OutputCapability> {
        self.ordered_outputs
            .iter()
            .find(|capability| capability.layer_id == output_layer_id)
    }
}

/// Validates one explicit capability and effective setting before output realization.
///
/// The document remains responsible for resolving inheritance and typed response arithmetic.
/// This boundary only proves that the resolved setting addresses exactly one ordered capability
/// and has the response kind the structural output consumes.
///
/// # Errors
///
/// Returns a stable output-order, output-ID, or response-kind diagnostic without realizing
/// source samples or publishing geometry.
pub fn validate_output_realization_binding(
    plan: &PatternPipelinePlan,
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
) -> Result<(), PatternPipelineError> {
    if plan.output_capability(capability.layer_id) != Some(capability) {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.capability",
            "realization capability is not a member of the ordered pipeline plan",
        ));
    }
    if capability.layer_id != setting.output_layer_id {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.setting",
            "effective output setting must address the realized output layer",
        ));
    }
    match (&capability.payload, &setting.response) {
        (OutputCapabilityPayload::Marks { .. }, PatternGeometryResponse::Marks(_))
        | (OutputCapabilityPayload::GuidePaths { .. }, PatternGeometryResponse::Connected(_))
        | (
            OutputCapabilityPayload::CurveMotifPaths { .. },
            PatternGeometryResponse::Connected(_),
        )
        | (
            OutputCapabilityPayload::ConnectionPaths { .. },
            PatternGeometryResponse::Connected(_),
        )
        | (OutputCapabilityPayload::MazeWalls { .. }, PatternGeometryResponse::Connected(_)) => {
            Ok(())
        }
        (OutputCapabilityPayload::Regions { .. }, PatternGeometryResponse::Regions(_)) => Ok(()),
        _ => Err(PatternPipelineError::new(
            "pattern.output_layers.setting",
            "effective output response kind is incompatible with its structural capability",
        )),
    }
}

/// Complete region realization emitted by the sole typed region-output authority.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedRegionOutputRealization {
    /// Stores treated canonical fill geometry, which may be empty after collapse or alpha suppression.
    pub regions: CanonicalRegionSet,
    /// Stores sampled paint in the exact canonical treated-region order when requested.
    pub paints: Option<Vec<SampledSourcePaint>>,
    /// Stores the cacheable realization identity; diagnostics and limits remain excluded.
    pub fingerprint: String,
    /// Stores local bounded-work facts without admitting them into geometry identity.
    pub diagnostics: RegionOutputRealizationDiagnostics,
}

/// Local typed region-realization facts retained with a cache unit but excluded from its fingerprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionOutputRealizationDiagnostics {
    /// Counts untreated bases sampled exactly once during this realization.
    pub sampled_bases: usize,
    /// Counts retained treated canonical fill components.
    pub retained_regions: usize,
}

/// Diagnostic-only timing and workload facts from one completed typed region realization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionOutputPerformanceMetrics {
    /// Measures complete untreated-region source sampling.
    pub sampling_duration: Duration,
    /// Measures positive region resize and canonical treatment work.
    pub treatment_duration: Duration,
    /// Counts sampled untreated bases in stable producer order.
    pub sampled_bases: usize,
    /// Counts AreaAverage flattened boundary chords, or zero for ReferencePoint sampling.
    pub flattened_segments: usize,
    /// Counts AreaAverage source-cell intersections, or zero for ReferencePoint sampling.
    pub cell_intersections: usize,
    /// Counts retained positive treated canonical components.
    pub retained_regions: usize,
    /// Reports the shared Rayon worker count available to indexed region work.
    pub worker_count: usize,
}

/// Records one already-completed typed region realization for engine test evidence only.
///
/// This record exists solely behind the opt-in `test-evidence` feature. It is not serialized,
/// cached, included in a public production build, or used by capability projection; it observes
/// the same sampling and treatment invocation that produced `TypedRegionOutputRealization`.
#[cfg(feature = "test-evidence")]
#[derive(Clone, Debug, PartialEq)]
pub struct RegionEvaluationEvidence {
    /// Preserves the complete untreated canonical geometry from the actual producer invocation.
    pub untreated_regions: CanonicalRegionSet,
    /// Preserves untreated canonical IDs in their accepted producer order.
    pub untreated_region_ids: Vec<toniator_geometry::CanonicalRegionId>,
    /// Preserves the producer-owned reference table in the same accepted order.
    pub references: Vec<RegionReference>,
    /// Records the response-selected sampling strategy.
    pub sampling: toniator_domain::RegionSamplingStrategy,
    /// Preserves exactly one source sample per untreated base when sampling is required.
    pub samples: Vec<RegionSourceSample>,
    /// Preserves the resolved per-base treatment request, including alpha suppression omissions.
    pub treatments: Vec<RegionTreatmentRequest>,
    /// Preserves treated canonical geometry in the normal output order for validation records only.
    pub treated_regions: CanonicalRegionSet,
    /// Preserves treated-to-base ownership and deterministic component ordinals in output order.
    pub provenance: Vec<RegionTreatmentProvenance>,
    /// Records the accepted untreated canonical fingerprint.
    pub untreated_fingerprint: String,
    /// Records the treated canonical fingerprint, including a valid empty result.
    pub treated_fingerprint: String,
    /// Records the normal realization fingerprint returned to the engine.
    pub realization_fingerprint: String,
    /// Records ordinary local diagnostics without adding them to the realization identity.
    pub diagnostics: RegionOutputRealizationDiagnostics,
}

/// Stable failure from the typed region output boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionOutputRealizationError {
    path: &'static str,
    message: &'static str,
}

impl RegionOutputRealizationError {
    /// Returns the stable region-output failure path.
    pub const fn path(&self) -> &'static str {
        self.path
    }

    /// Returns the stable region-output failure message.
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for RegionOutputRealizationError {
    /// Formats a stable typed-region failure without exposing partial geometry or paint.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for RegionOutputRealizationError {}

/// Realizes exactly one accepted untreated region output with one typed effective response.
///
/// Every path samples each complete untreated base exactly once, maps that scalar linearly into
/// the effective normalized fill range, omits zero fill and exact-zero-alpha sampled bases before
/// geometry, and aligns sampled paint through retained positive-region provenance. Renderers
/// receive only closed canonical rings and never construct topology.
///
/// # Errors
///
/// Returns a stable binding, identity, sampling, treatment, allocation, or cancellation failure
/// without publishing a partial sample table, paint table, or treated region candidate.
#[allow(clippy::too_many_arguments)]
pub fn realize_region_output_cancellable(
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    untreated: &CanonicalRegionSet,
    references: &[RegionReference],
    source: Option<&SourceField>,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    paint: &ChannelPaint,
    sampling_limits: RegionSamplingLimits,
    treatment_limits: RegionTreatmentLimits,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<TypedRegionOutputRealization, RegionOutputRealizationError> {
    realize_region_output_cancellable_impl(
        capability,
        setting,
        untreated,
        references,
        source,
        canvas,
        mapping,
        paint,
        sampling_limits,
        treatment_limits,
        cancelled,
        None,
        None,
        #[cfg(feature = "test-evidence")]
        None,
    )
}

/// Realizes one typed Region output with observational sampling progress.
///
/// The callback reports completed sampling work only and cannot publish partial regions, alter
/// identity, or replace cancellation and cache authority.
///
/// # Errors
///
/// Returns the same stable failures as [`realize_region_output_cancellable`] atomically.
#[allow(clippy::too_many_arguments)]
pub fn realize_region_output_with_progress_cancellable(
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    untreated: &CanonicalRegionSet,
    references: &[RegionReference],
    source: Option<&SourceField>,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    paint: &ChannelPaint,
    sampling_limits: RegionSamplingLimits,
    treatment_limits: RegionTreatmentLimits,
    cancelled: &(dyn Fn() -> bool + Sync),
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<TypedRegionOutputRealization, RegionOutputRealizationError> {
    realize_region_output_cancellable_impl(
        capability,
        setting,
        untreated,
        references,
        source,
        canvas,
        mapping,
        paint,
        sampling_limits,
        treatment_limits,
        cancelled,
        Some(progress),
        None,
        #[cfg(feature = "test-evidence")]
        None,
    )
}

/// Realizes one typed region output and reports coarse production boundary metrics.
///
/// This calls the same authority as [`realize_region_output_cancellable`]. Timings and workload
/// counters remain diagnostic-only and are returned only with a complete realization.
///
/// # Errors
///
/// Returns the normal binding, identity, sampling, treatment, allocation, or cancellation failure
/// without returning partial geometry or performance data.
#[allow(clippy::too_many_arguments)]
pub fn realize_region_output_profiled_cancellable(
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    untreated: &CanonicalRegionSet,
    references: &[RegionReference],
    source: Option<&SourceField>,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    paint: &ChannelPaint,
    sampling_limits: RegionSamplingLimits,
    treatment_limits: RegionTreatmentLimits,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<
    (TypedRegionOutputRealization, RegionOutputPerformanceMetrics),
    RegionOutputRealizationError,
> {
    let mut performance = RegionOutputPerformanceMetrics {
        worker_count: rayon::current_num_threads(),
        ..RegionOutputPerformanceMetrics::default()
    };
    let realization = realize_region_output_cancellable_impl(
        capability,
        setting,
        untreated,
        references,
        source,
        canvas,
        mapping,
        paint,
        sampling_limits,
        treatment_limits,
        cancelled,
        None,
        Some(&mut performance),
        #[cfg(feature = "test-evidence")]
        None,
    )?;
    Ok((realization, performance))
}

/// Realizes one Region output while retaining a test-only record from that exact invocation.
///
/// The opt-in feature is used only by engine validation tests. It never repeats sampling or
/// treatment and is absent from ordinary production pattern builds.
///
/// # Errors
///
/// Returns the normal realization error and never returns an evidence record for a failed call.
#[cfg(feature = "test-evidence")]
#[allow(clippy::too_many_arguments)]
pub fn realize_region_output_with_evidence_cancellable(
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    untreated: &CanonicalRegionSet,
    references: &[RegionReference],
    source: Option<&SourceField>,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    paint: &ChannelPaint,
    sampling_limits: RegionSamplingLimits,
    treatment_limits: RegionTreatmentLimits,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(TypedRegionOutputRealization, RegionEvaluationEvidence), RegionOutputRealizationError>
{
    let mut evidence = None;
    let realization = realize_region_output_cancellable_impl(
        capability,
        setting,
        untreated,
        references,
        source,
        canvas,
        mapping,
        paint,
        sampling_limits,
        treatment_limits,
        cancelled,
        None,
        None,
        Some(&mut evidence),
    )?;
    let evidence = evidence.expect("successful test evidence realization records one snapshot");
    Ok((realization, evidence))
}

/// Implements the sole Region output realization and optionally records test-only evidence.
///
/// The optional sink is feature-gated from production builds and is filled only after all source
/// sampling, treatment, paint alignment, and fingerprint construction complete successfully.
///
/// # Errors
///
/// Returns normal stable realization failures without filling the optional test evidence sink.
#[allow(clippy::too_many_arguments)]
fn realize_region_output_cancellable_impl(
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    untreated: &CanonicalRegionSet,
    references: &[RegionReference],
    source: Option<&SourceField>,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    paint: &ChannelPaint,
    sampling_limits: RegionSamplingLimits,
    treatment_limits: RegionTreatmentLimits,
    cancelled: &(dyn Fn() -> bool + Sync),
    progress: Option<&(dyn Fn(usize, usize) + Sync)>,
    mut performance: Option<&mut RegionOutputPerformanceMetrics>,
    #[cfg(feature = "test-evidence")] evidence: Option<&mut Option<RegionEvaluationEvidence>>,
) -> Result<TypedRegionOutputRealization, RegionOutputRealizationError> {
    validate_region_output_binding(capability, setting)?;
    poll_region_realizer(cancelled)?;
    let toniator_domain::PatternGeometryResponse::Regions(response) = &setting.response else {
        return Err(region_realization_error(
            "region.treatment.identity.response",
            "region output requires a region response",
        ));
    };
    let references_by_id = region_reference_table(untreated, references)?;
    let source = source.ok_or(region_realization_error(
        "sampling.region.source",
        "region sampling requires a decoded source field",
    ))?;
    let sampling_started = Instant::now();
    let report_sampling_progress = |completed: usize, total: usize| {
        if let Some(progress) = progress {
            progress(completed.saturating_mul(800) / total.max(1), 1_000);
        }
    };
    let (samples, sampling_diagnostics) = sample_untreated_regions(
        source,
        untreated,
        &references_by_id,
        canvas,
        mapping,
        response,
        sampling_limits,
        cancelled,
        progress.map(|_| &report_sampling_progress as &(dyn Fn(usize, usize) + Sync)),
    )?;
    if let Some(progress) = progress {
        progress(800, 1_000);
    }
    if let Some(performance) = performance.as_deref_mut() {
        performance.sampling_duration = sampling_started.elapsed();
        performance.sampled_bases = samples.len();
        performance.flattened_segments = sampling_diagnostics.flattened_segments;
        performance.cell_intersections = sampling_diagnostics.cell_intersections;
    }
    let mut requests = Vec::new();
    requests
        .try_reserve(untreated.regions().len())
        .map_err(|_| {
            region_realization_error(
                "region.treatment.allocation.requests",
                "region treatment request allocation failed",
            )
        })?;
    for (region, sample) in untreated.regions().iter().zip(&samples) {
        poll_region_realizer(cancelled)?;
        let treatment = if matches!(paint, ChannelPaint::SampledSource) && sample.paint.is_none() {
            None
        } else {
            Some(interpolated_region_treatment(response, sample.response)?)
        };
        requests.push(RegionTreatmentRequest {
            base_region_id: region.id.clone(),
            reference: references_by_id.get(&region.id).copied(),
            treatment,
        });
    }
    if let Some(progress) = progress {
        progress(850, 1_000);
    }
    let treatment_started = Instant::now();
    let treated = treat_region_requests_cancellable(
        capability.layer_id,
        untreated,
        &requests,
        treatment_limits,
        cancelled,
    )
    .map_err(|error| region_realization_error(error.path(), error.message()))?;
    if let Some(progress) = progress {
        progress(1_000, 1_000);
    }
    if let Some(performance) = performance {
        performance.treatment_duration = treatment_started.elapsed();
        performance.retained_regions = treated.regions.regions().len();
    }
    let paints = matches!(paint, ChannelPaint::SampledSource)
        .then(|| align_treated_region_paints(&treated, untreated, &samples))
        .transpose()?;
    let fingerprint = region_realization_fingerprint(
        untreated,
        &references_by_id,
        &samples,
        &treated,
        paints.as_deref(),
    );
    let diagnostics = RegionOutputRealizationDiagnostics {
        sampled_bases: samples.len(),
        retained_regions: treated.regions.regions().len(),
    };
    #[cfg(feature = "test-evidence")]
    record_region_evaluation_evidence(
        evidence,
        untreated,
        references,
        response,
        &samples,
        &requests,
        &treated,
        &fingerprint,
        diagnostics,
    );
    Ok(TypedRegionOutputRealization {
        diagnostics,
        regions: treated.regions,
        paints,
        fingerprint,
    })
}

/// Publishes a test-only snapshot only after the ordinary region realization has fully succeeded.
#[cfg(feature = "test-evidence")]
#[allow(clippy::too_many_arguments)]
fn record_region_evaluation_evidence(
    sink: Option<&mut Option<RegionEvaluationEvidence>>,
    untreated: &CanonicalRegionSet,
    references: &[RegionReference],
    response: &toniator_domain::RegionGeometryResponse,
    samples: &[RegionSourceSample],
    treatments: &[RegionTreatmentRequest],
    treated: &RegionTreatmentResult,
    realization_fingerprint: &str,
    diagnostics: RegionOutputRealizationDiagnostics,
) {
    let Some(sink) = sink else {
        return;
    };
    *sink = Some(RegionEvaluationEvidence {
        untreated_regions: untreated.clone(),
        untreated_region_ids: untreated
            .regions()
            .iter()
            .map(|region| region.id.clone())
            .collect(),
        references: references.to_vec(),
        sampling: response.sampling,
        samples: samples.to_vec(),
        treatments: treatments.to_vec(),
        treated_regions: treated.regions.clone(),
        provenance: treated.provenance.clone(),
        untreated_fingerprint: untreated.fingerprint().to_owned(),
        treated_fingerprint: treated.regions.fingerprint().to_owned(),
        realization_fingerprint: realization_fingerprint.to_owned(),
        diagnostics,
    });
}

/// Validates a direct capability/setting pair without admitting an unrelated pipeline plan.
///
/// # Errors
///
/// Returns only stable output-ID or response-kind diagnostics before sampling or geometry work.
fn validate_region_output_binding(
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
) -> Result<(), RegionOutputRealizationError> {
    if capability.layer_id != setting.output_layer_id {
        return Err(region_realization_error(
            "pattern.output_layers.setting",
            "effective output setting must address the realized output layer",
        ));
    }
    matches!(
        (&capability.payload, &setting.response),
        (
            OutputCapabilityPayload::Regions { .. },
            PatternGeometryResponse::Regions(_)
        )
    )
    .then_some(())
    .ok_or(region_realization_error(
        "pattern.output_layers.setting",
        "typed region realizer requires a Region capability and response",
    ))
}

/// Builds a complete producer-reference table keyed one-to-one with untreated canonical IDs.
///
/// # Errors
///
/// Returns identity failures before any source sampling can create a misaligned table.
fn region_reference_table(
    untreated: &CanonicalRegionSet,
    references: &[RegionReference],
) -> Result<BTreeMap<toniator_geometry::CanonicalRegionId, Point2>, RegionOutputRealizationError> {
    if references.len() != untreated.regions().len() {
        return Err(region_realization_error(
            "region.treatment.identity.reference",
            "every untreated region requires one producer reference",
        ));
    }
    let mut table = BTreeMap::new();
    for reference in references {
        if !reference.point.is_finite()
            || !untreated
                .regions()
                .iter()
                .any(|region| region.id == reference.region_id)
            || table
                .insert(reference.region_id.clone(), reference.point)
                .is_some()
        {
            return Err(region_realization_error(
                "region.treatment.identity.reference",
                "producer references must be finite and keyed one-to-one with untreated regions",
            ));
        }
    }
    Ok(table)
}

/// Samples every complete untreated base once under the response-selected sampling strategy.
///
/// # Errors
///
/// Propagates stable source sampling, allocation, and cancellation diagnostics atomically.
#[allow(clippy::too_many_arguments)]
fn sample_untreated_regions(
    source: &SourceField,
    untreated: &CanonicalRegionSet,
    references: &BTreeMap<toniator_geometry::CanonicalRegionId, Point2>,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: &toniator_domain::RegionGeometryResponse,
    limits: RegionSamplingLimits,
    cancelled: &(dyn Fn() -> bool + Sync),
    progress: Option<&(dyn Fn(usize, usize) + Sync)>,
) -> Result<(Vec<RegionSourceSample>, RegionSamplingDiagnostics), RegionOutputRealizationError> {
    match response.sampling {
        toniator_domain::RegionSamplingStrategy::ReferencePoint => {
            let completed = AtomicUsize::new(0);
            let total = untreated.regions().len().max(1);
            let samples = untreated
                .regions()
                .par_iter()
                .map(|region| {
                    poll_region_realizer(cancelled)?;
                    let sample =
                        sample_region_reference(source, references[&region.id], canvas, mapping)
                            .map_err(|error| {
                                region_realization_error(error.path(), error.message())
                            })?;
                    let count = completed.fetch_add(1, Ordering::Relaxed).saturating_add(1);
                    if let Some(progress) = progress {
                        progress(count.min(total), total);
                    }
                    Ok(sample)
                })
                .collect::<Vec<_>>()
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;
            Ok((samples, RegionSamplingDiagnostics::default()))
        }
        toniator_domain::RegionSamplingStrategy::AreaAverage => {
            let rings: Vec<_> = untreated
                .regions()
                .iter()
                .map(|region| region.ring.clone())
                .collect();
            if let Some(progress) = progress {
                sample_region_area_average_batch_with_diagnostics_and_progress(
                    source, &rings, canvas, mapping, limits, cancelled, progress,
                )
            } else {
                sample_region_area_average_batch_with_diagnostics(
                    source, &rings, canvas, mapping, limits, cancelled,
                )
            }
            .map_err(|error| region_realization_error(error.path(), error.message()))
        }
    }
}

/// Interpolates one sampled scalar linearly into the typed normalized fill range.
///
/// # Errors
///
/// Returns only a stable response-kind error when a non-region setting reaches this boundary.
fn interpolated_region_treatment(
    response: &toniator_domain::RegionGeometryResponse,
    sample: f64,
) -> Result<RegionTreatment, RegionOutputRealizationError> {
    if !sample.is_finite() {
        return Err(region_realization_error(
            "region.resize.response.sample",
            "region source response must remain finite",
        ));
    }
    Ok(RegionTreatment {
        algorithm: response.algorithm,
        fill: response.minimum_fill + sample * (response.maximum_fill - response.minimum_fill),
    })
}

/// Aligns sampled base paint with every retained treated component through explicit geometry provenance.
///
/// # Errors
///
/// Returns an identity failure if geometry omission or provenance would create a partial paint table.
fn align_treated_region_paints(
    treated: &RegionTreatmentResult,
    untreated: &CanonicalRegionSet,
    samples: &[RegionSourceSample],
) -> Result<Vec<SampledSourcePaint>, RegionOutputRealizationError> {
    let paints_by_base: BTreeMap<_, _> = untreated
        .regions()
        .iter()
        .zip(samples)
        .filter_map(|(region, sample)| sample.paint.map(|paint| (region.id.clone(), paint)))
        .collect();
    treated
        .provenance
        .iter()
        .map(|item| {
            paints_by_base
                .get(&item.base_region_id)
                .copied()
                .ok_or(region_realization_error(
                    "region.treatment.identity.paint",
                    "every retained treated region requires positive-alpha base paint",
                ))
        })
        .collect()
}

/// Builds the fixed-width cacheable region-realization identity from ordered base inputs and final products.
///
/// Every typed field, canonical-ID variant, and variable-length payload is written in its
/// authoritative order. The digest deliberately does not retain debug text for each region, so
/// Guide Face boundary provenance cannot multiply a cached fingerprint's allocation footprint.
fn region_realization_fingerprint(
    untreated: &CanonicalRegionSet,
    references: &BTreeMap<toniator_geometry::CanonicalRegionId, Point2>,
    samples: &[RegionSourceSample],
    treated: &RegionTreatmentResult,
    paints: Option<&[SampledSourcePaint]>,
) -> String {
    let mut fingerprint = Fnv1a64State::new();
    append_region_realizer_text(&mut fingerprint, REGION_REALIZER_FINGERPRINT_CONTRACT_ID);
    append_region_realizer_text(&mut fingerprint, untreated.fingerprint());
    fingerprint.write(
        u64::try_from(untreated.regions().len())
            .expect("region count fits u64")
            .to_le_bytes(),
    );
    for (base, sample) in untreated.regions().iter().zip(samples) {
        let reference = references[&base.id];
        append_canonical_region_id_identity(&mut fingerprint, &base.id);
        fingerprint.write(reference.x.to_bits().to_le_bytes());
        fingerprint.write(reference.y.to_bits().to_le_bytes());
        fingerprint.write(sample.response.to_bits().to_le_bytes());
        if let Some(paint) = sample.paint {
            fingerprint.write([1]);
            append_sampled_source_paint_identity(&mut fingerprint, paint);
        } else {
            fingerprint.write([0]);
        }
    }
    append_region_realizer_text(&mut fingerprint, treated.regions.fingerprint());
    if let Some(paints) = paints {
        fingerprint.write([1]);
        fingerprint.write(
            u64::try_from(paints.len())
                .expect("sampled paint count fits u64")
                .to_le_bytes(),
        );
        for paint in paints {
            append_sampled_source_paint_identity(&mut fingerprint, *paint);
        }
    } else {
        fingerprint.write([0]);
    }
    fingerprint.finish()
}

/// Appends one length-delimited textual sub-identity to the typed region-realizer digest.
fn append_region_realizer_text(fingerprint: &mut Fnv1a64State, value: &str) {
    fingerprint.write(
        u64::try_from(value.len())
            .expect("region identity text length fits u64")
            .to_le_bytes(),
    );
    fingerprint.write(value.bytes());
}

/// Appends one fully typed canonical-region identity with explicit source variant and payload length.
fn append_canonical_region_id_identity(
    fingerprint: &mut Fnv1a64State,
    region_id: &toniator_geometry::CanonicalRegionId,
) {
    fingerprint.write(region_id.output_layer_id.0.to_le_bytes());
    match &region_id.source_id {
        CanonicalRegionSourceId::SiteOwners(owners) => {
            fingerprint.write([1]);
            fingerprint.write(
                u64::try_from(owners.len())
                    .expect("canonical site-owner count fits u64")
                    .to_le_bytes(),
            );
            for owner in owners {
                fingerprint.write(owner.mechanism_id.0.to_le_bytes());
                fingerprint.write(
                    u64::try_from(owner.ordinal)
                        .expect("family-site ordinal fits u64")
                        .to_le_bytes(),
                );
            }
        }
        CanonicalRegionSourceId::GuideBoundary(boundary) => {
            fingerprint.write([2]);
            fingerprint.write(
                u64::try_from(boundary.len())
                    .expect("canonical Guide Face boundary count fits u64")
                    .to_le_bytes(),
            );
            for location in boundary {
                append_region_realizer_path_location_identity(fingerprint, location);
            }
        }
    }
    fingerprint.write(region_id.component_ordinal.to_le_bytes());
}

/// Appends one complete Guide Face structural location without allocating a debug representation.
fn append_region_realizer_path_location_identity(
    fingerprint: &mut Fnv1a64State,
    location: &StructuralPathLocationProvenance,
) {
    match location.path.source {
        StructuralPathSourceId::GuideDimension(id) => {
            fingerprint.write([1]);
            fingerprint.write(id.0.to_le_bytes());
        }
        StructuralPathSourceId::ParametricCurve(id) => {
            fingerprint.write([2]);
            fingerprint.write(id.0.to_le_bytes());
        }
    }
    fingerprint.write(location.path.repetition_index.to_le_bytes());
    fingerprint.write(location.path.component_ordinal.to_le_bytes());
    fingerprint.write(
        u64::try_from(location.segment_index)
            .expect("structural path segment index fits u64")
            .to_le_bytes(),
    );
    fingerprint.write(location.parameter_bits.to_le_bytes());
}

/// Appends one sampled source color in exact component-bit order.
fn append_sampled_source_paint_identity(fingerprint: &mut Fnv1a64State, paint: SampledSourcePaint) {
    fingerprint.write(paint.red.to_bits().to_le_bytes());
    fingerprint.write(paint.green.to_bits().to_le_bytes());
    fingerprint.write(paint.blue.to_bits().to_le_bytes());
    fingerprint.write(paint.alpha.to_bits().to_le_bytes());
}

/// Polls cancellation at the typed output boundary before allocating or publishing a candidate.
///
/// # Errors
///
/// Returns exactly `evaluation.cancelled` when the caller invalidates this realization.
fn poll_region_realizer(cancelled: &dyn Fn() -> bool) -> Result<(), RegionOutputRealizationError> {
    (!cancelled()).then_some(()).ok_or(region_realization_error(
        "evaluation.cancelled",
        "evaluation cancelled",
    ))
}

/// Constructs one stable region-output error without exposing internal partial state.
fn region_realization_error(
    path: &'static str,
    message: &'static str,
) -> RegionOutputRealizationError {
    RegionOutputRealizationError { path, message }
}

#[cfg(test)]
mod stage20q_region_realizer_tests {
    use super::*;

    /// Builds a single typed Region capability for direct realizer authority tests.
    fn stage20q_capability() -> OutputCapability {
        OutputCapability {
            layer_id: PatternOutputLayerId(81),
            source_filter: SiteUseFilter::All,
            consumes: StructuralProductCapability::RandomSites,
            payload: OutputCapabilityPayload::Regions {
                source: RegionSourceIntent::VoronoiSites {
                    site_mechanism_id: PatternMechanismId(7),
                },
            },
        }
    }

    /// Builds the matching effective unit-fill response without document-level inheritance setup.
    fn stage20q_full_setting(
        output_layer_id: PatternOutputLayerId,
    ) -> EffectivePatternOutputSettings {
        EffectivePatternOutputSettings {
            output_layer_id,
            response: PatternGeometryResponse::Regions(toniator_domain::RegionGeometryResponse {
                algorithm: RegionResizeAlgorithm::Scale,
                sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                minimum_fill: 1.0,
                maximum_fill: 1.0,
            }),
        }
    }

    /// Builds one accepted untreated triangle and its producer reference.
    fn stage20q_untreated() -> (CanonicalRegionSet, Vec<RegionReference>) {
        let regions = toniator_geometry::build_canonical_regions_cancellable(
            toniator_geometry::CanonicalRegionProposal {
                output_layer_id: PatternOutputLayerId(81),
                source_groups: vec![toniator_geometry::CanonicalRegionSourceGroup {
                    source_id: toniator_geometry::CanonicalRegionSourceId::SiteOwners(vec![
                        toniator_geometry::FamilySiteId {
                            mechanism_id: PatternMechanismId(7),
                            ordinal: 0,
                        },
                    ]),
                    components: vec![
                        CurvePath::polyline(
                            vec![
                                Point2::new(0.0, 0.0),
                                Point2::new(2.0, 0.0),
                                Point2::new(0.0, 2.0),
                            ],
                            PathClosure::Closed,
                        )
                        .unwrap(),
                    ],
                }],
            },
            toniator_geometry::CanonicalRegionLimits::default(),
            || false,
        )
        .unwrap()
        .0;
        let references = vec![RegionReference {
            region_id: regions.regions()[0].id.clone(),
            point: Point2::new(0.0, 0.0),
        }];
        (regions, references)
    }

    /// Builds two ordered canonical bases and matching direct-realizer identity inputs.
    fn stage21a_fingerprint_inputs() -> (
        CanonicalRegionSet,
        BTreeMap<toniator_geometry::CanonicalRegionId, Point2>,
        Vec<RegionSourceSample>,
        RegionTreatmentResult,
    ) {
        let untreated = toniator_geometry::build_canonical_regions(
            toniator_geometry::CanonicalRegionProposal {
                output_layer_id: PatternOutputLayerId(81),
                source_groups: vec![toniator_geometry::CanonicalRegionSourceGroup {
                    source_id: toniator_geometry::CanonicalRegionSourceId::SiteOwners(vec![
                        toniator_geometry::FamilySiteId {
                            mechanism_id: PatternMechanismId(7),
                            ordinal: 10,
                        },
                    ]),
                    components: vec![
                        CurvePath::polyline(
                            vec![
                                Point2::new(0.0, 0.0),
                                Point2::new(2.0, 0.0),
                                Point2::new(0.0, 2.0),
                            ],
                            PathClosure::Closed,
                        )
                        .expect("first fingerprint triangle closes"),
                        CurvePath::polyline(
                            vec![
                                Point2::new(4.0, 0.0),
                                Point2::new(6.0, 0.0),
                                Point2::new(4.0, 2.0),
                            ],
                            PathClosure::Closed,
                        )
                        .expect("second fingerprint triangle closes"),
                    ],
                }],
            },
        )
        .expect("two fingerprint bases canonicalize");
        let references = untreated
            .regions()
            .iter()
            .enumerate()
            .map(|(ordinal, region)| (region.id.clone(), Point2::new(ordinal as f64, 0.0)))
            .collect();
        let samples = vec![
            RegionSourceSample {
                response: 0.125,
                paint: None,
            },
            RegionSourceSample {
                response: 0.875,
                paint: None,
            },
        ];
        let treated = RegionTreatmentResult {
            regions: untreated.clone(),
            provenance: Vec::new(),
        };
        (untreated, references, samples, treated)
    }

    /// Proves the typed digest is fixed-width, deterministic, and sensitive to authoritative order.
    #[test]
    fn stage21a_region_realizer_fingerprint_is_fixed_width_and_order_sensitive() {
        let (untreated, references, samples, treated) = stage21a_fingerprint_inputs();
        let first =
            region_realization_fingerprint(&untreated, &references, &samples, &treated, None);
        let repeated =
            region_realization_fingerprint(&untreated, &references, &samples, &treated, None);
        let reversed_samples = samples.iter().copied().rev().collect::<Vec<_>>();
        let reordered = region_realization_fingerprint(
            &untreated,
            &references,
            &reversed_samples,
            &treated,
            None,
        );
        assert_eq!(first, repeated);
        assert_eq!(first.len(), "fnv1a64:".len() + 16);
        assert_ne!(first, reordered);
    }

    /// Proves canonical-region identity encoding distinguishes variants and length-delimited payloads.
    #[test]
    fn stage21a_region_realizer_fingerprint_distinguishes_canonical_id_variants_and_lengths() {
        let output = PatternOutputLayerId(81);
        let site_owner = toniator_geometry::FamilySiteId {
            mechanism_id: PatternMechanismId(7),
            ordinal: 10,
        };
        let guide_location = StructuralPathLocationProvenance {
            path: StructuralPathInstanceId::guide_dimension(GuideDimensionId(9), -2, 3),
            segment_index: 4,
            parameter_bits: 0.25_f64.to_bits(),
        };
        let ids = [
            toniator_geometry::CanonicalRegionId {
                output_layer_id: output,
                source_id: CanonicalRegionSourceId::SiteOwners(vec![site_owner]),
                component_ordinal: 0,
            },
            toniator_geometry::CanonicalRegionId {
                output_layer_id: output,
                source_id: CanonicalRegionSourceId::SiteOwners(vec![site_owner, site_owner]),
                component_ordinal: 0,
            },
            toniator_geometry::CanonicalRegionId {
                output_layer_id: output,
                source_id: CanonicalRegionSourceId::GuideBoundary(vec![guide_location]),
                component_ordinal: 0,
            },
            toniator_geometry::CanonicalRegionId {
                output_layer_id: output,
                source_id: CanonicalRegionSourceId::GuideBoundary(vec![
                    guide_location,
                    guide_location,
                ]),
                component_ordinal: 0,
            },
        ];
        let fingerprints = ids.map(|id| {
            let mut state = Fnv1a64State::new();
            append_canonical_region_id_identity(&mut state, &id);
            state.finish()
        });
        assert_ne!(fingerprints[0], fingerprints[1]);
        assert_ne!(fingerprints[0], fingerprints[2]);
        assert_ne!(fingerprints[2], fingerprints[3]);
    }

    /// Verifies normalized unit fill still requires its source-sampling authority.
    #[test]
    fn stage20q_unit_fill_rejects_missing_source() {
        let capability = stage20q_capability();
        let setting = stage20q_full_setting(capability.layer_id);
        let (untreated, references) = stage20q_untreated();
        let result = realize_region_output_cancellable(
            &capability,
            &setting,
            &untreated,
            &references,
            None,
            &CanvasSpec {
                width: 2.0,
                height: 2.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Alpha),
            &ChannelPaint::Solid(toniator_domain::ColorValue {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            }),
            RegionSamplingLimits::default(),
            RegionTreatmentLimits::default(),
            &|| false,
        )
        .expect_err("normalized fill requires source sampling");
        assert_eq!(result.path(), "sampling.region.source");
    }

    /// Verifies an effective response targeting a different output is rejected before sampling.
    #[test]
    fn stage20q_region_realizer_rejects_output_binding_mismatch() {
        let capability = stage20q_capability();
        let (untreated, references) = stage20q_untreated();
        let error = realize_region_output_cancellable(
            &capability,
            &stage20q_full_setting(PatternOutputLayerId(82)),
            &untreated,
            &references,
            None,
            &CanvasSpec {
                width: 2.0,
                height: 2.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Alpha),
            &ChannelPaint::Solid(toniator_domain::ColorValue {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            }),
            RegionSamplingLimits::default(),
            RegionTreatmentLimits::default(),
            &|| false,
        )
        .unwrap_err();
        assert_eq!(error.path(), "pattern.output_layers.setting");
    }

    /// Verifies canonical components from shared-source bases select paint by base provenance,
    /// never by their common source identity or newly assigned treated ordinal.
    #[test]
    fn stage20q_shared_source_treated_components_do_not_leak_sampled_paint() {
        let untreated = toniator_geometry::build_canonical_regions(
            toniator_geometry::CanonicalRegionProposal {
                output_layer_id: PatternOutputLayerId(81),
                source_groups: vec![toniator_geometry::CanonicalRegionSourceGroup {
                    source_id: toniator_geometry::CanonicalRegionSourceId::SiteOwners(vec![
                        toniator_geometry::FamilySiteId {
                            mechanism_id: PatternMechanismId(7),
                            ordinal: 9,
                        },
                    ]),
                    components: vec![
                        CurvePath::polyline(
                            vec![
                                Point2::new(0.0, 0.0),
                                Point2::new(2.0, 0.0),
                                Point2::new(0.0, 2.0),
                            ],
                            PathClosure::Closed,
                        )
                        .unwrap(),
                        CurvePath::polyline(
                            vec![
                                Point2::new(8.0, 0.0),
                                Point2::new(10.0, 0.0),
                                Point2::new(8.0, 2.0),
                            ],
                            PathClosure::Closed,
                        )
                        .unwrap(),
                    ],
                }],
            },
        )
        .unwrap();
        let treated = treat_region_requests_cancellable(
            PatternOutputLayerId(81),
            &untreated,
            &[
                RegionTreatmentRequest {
                    base_region_id: untreated.regions()[0].id.clone(),
                    reference: Some(Point2::new(0.0, 0.0)),
                    treatment: Some(RegionTreatment {
                        algorithm: RegionResizeAlgorithm::Scale,
                        fill: 0.5,
                    }),
                },
                RegionTreatmentRequest {
                    base_region_id: untreated.regions()[1].id.clone(),
                    reference: None,
                    treatment: Some(RegionTreatment {
                        algorithm: RegionResizeAlgorithm::UniformOffset,
                        fill: 1.0,
                    }),
                },
            ],
            RegionTreatmentLimits::default(),
            || false,
        )
        .unwrap();
        let paints = align_treated_region_paints(
            &treated,
            &untreated,
            &[
                RegionSourceSample {
                    response: 0.0,
                    paint: Some(SampledSourcePaint {
                        red: 1.0,
                        green: 0.0,
                        blue: 0.0,
                        alpha: 1.0,
                    }),
                },
                RegionSourceSample {
                    response: 1.0,
                    paint: Some(SampledSourcePaint {
                        red: 0.0,
                        green: 0.0,
                        blue: 1.0,
                        alpha: 1.0,
                    }),
                },
            ],
        )
        .unwrap();
        assert_eq!(paints.len(), 2);
        assert!(
            paints
                .iter()
                .any(|paint| paint.red == 1.0 && paint.blue == 0.0)
        );
        assert!(
            paints
                .iter()
                .any(|paint| paint.red == 0.0 && paint.blue == 1.0)
        );
    }
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
        match &output.realization {
            PatternOutputRealization::CircularMarks {
                site_mechanism_id: source_id,
            } if *source_id == site_mechanism_id => ordered_outputs.push(OutputCapability {
                layer_id: output.id,
                source_filter: output.source_filter,
                consumes: product,
                payload: OutputCapabilityPayload::Marks {
                    prototype: MarkPrototype::Circle,
                    orientation: MarkOrientation::Fixed,
                },
            }),
            PatternOutputRealization::MarkPrototype {
                site_mechanism_id: source_id,
                prototype,
                orientation,
            } if *source_id == site_mechanism_id => ordered_outputs.push(OutputCapability {
                layer_id: output.id,
                source_filter: output.source_filter,
                consumes: product,
                payload: OutputCapabilityPayload::Marks {
                    prototype: prototype.clone(),
                    orientation: orientation.clone(),
                },
            }),
            PatternOutputRealization::GuidePaths {
                guide_mechanism_id: source_id,
                style,
            } if *source_id == guide_mechanism_id => ordered_outputs.push(OutputCapability {
                layer_id: output.id,
                source_filter: output.source_filter,
                consumes: product,
                payload: OutputCapabilityPayload::GuidePaths {
                    guide_mechanism_id: *source_id,
                    style: *style,
                },
            }),
            PatternOutputRealization::CurveMotifPaths {
                site_mechanism_id: source_id,
                structure_id,
                style,
                mirror_alternate_rows,
                alternate_row_phase,
            } if *source_id == site_mechanism_id
                && product == StructuralProductCapability::AlongGuideSites =>
            {
                ordered_outputs.push(OutputCapability {
                    layer_id: output.id,
                    source_filter: output.source_filter,
                    consumes: product,
                    payload: OutputCapabilityPayload::CurveMotifPaths {
                        site_mechanism_id: *source_id,
                        structure_id: *structure_id,
                        style: *style,
                        mirror_alternate_rows: *mirror_alternate_rows,
                        alternate_row_phase: *alternate_row_phase,
                    },
                })
            }
            PatternOutputRealization::ConnectionPaths {
                site_mechanism_id: source_id,
                program,
                style,
            } if *source_id == site_mechanism_id => ordered_outputs.push(OutputCapability {
                layer_id: output.id,
                source_filter: output.source_filter,
                consumes: product,
                payload: OutputCapabilityPayload::ConnectionPaths {
                    site_mechanism_id: *source_id,
                    program: program.clone(),
                    style: *style,
                },
            }),
            PatternOutputRealization::MazeWalls {
                site_mechanism_id: source_id,
                program,
                style,
            } if *source_id == site_mechanism_id
                && product == StructuralProductCapability::GuideIntersections =>
            {
                ordered_outputs.push(OutputCapability {
                    layer_id: output.id,
                    source_filter: output.source_filter,
                    consumes: product,
                    payload: OutputCapabilityPayload::MazeWalls {
                        site_mechanism_id: *source_id,
                        program: program.clone(),
                        style: *style,
                    },
                })
            }
            PatternOutputRealization::Regions { source }
                if matches!(source, RegionSourceIntent::VoronoiSites { site_mechanism_id: source_id } if *source_id == site_mechanism_id)
                    && matches!(
                        product,
                        StructuralProductCapability::GuideIntersections
                            | StructuralProductCapability::AlongGuideSites
                            | StructuralProductCapability::RandomSites
                    ) =>
            {
                ordered_outputs.push(OutputCapability {
                    layer_id: output.id,
                    source_filter: output.source_filter,
                    consumes: product,
                    payload: OutputCapabilityPayload::Regions {
                        source: source.clone(),
                    },
                });
            }
            PatternOutputRealization::Regions {
                source:
                    RegionSourceIntent::GuideFaces {
                        guide_mechanism_id: source_id,
                        dimensions,
                    },
            } if *source_id == guide_mechanism_id => {
                ordered_outputs.push(OutputCapability {
                    layer_id: output.id,
                    source_filter: output.source_filter,
                    consumes: product,
                    payload: OutputCapabilityPayload::Regions {
                        source: RegionSourceIntent::GuideFaces {
                            guide_mechanism_id: *source_id,
                            dimensions: dimensions.clone(),
                        },
                    },
                });
            }
            _ => {
                return Err(PatternPipelineError::new(
                    "pattern.output_layers.capability",
                    "output layer cannot consume the declared structural product",
                ));
            }
        }
    }
    let evaluation_order = pattern_output_evaluation_order(definition)
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
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
        evaluation_order,
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
    let ordered_outputs = definition
        .output_layers
        .iter()
        .map(|output| match &output.realization {
            PatternOutputRealization::ParametricPaths {
                curve_mechanism_id: source,
                style,
            } if product == StructuralProductCapability::ParametricPaths
            && *source == curve_mechanism_id =>
        {
            Ok(OutputCapability {
                layer_id: output.id,
                source_filter: output.source_filter,
                consumes: product,
                payload: OutputCapabilityPayload::GuidePaths {
                    guide_mechanism_id: *source,
                    style: *style,
                },
            })
        }
        PatternOutputRealization::CircularMarks { site_mechanism_id }
            if Some(*site_mechanism_id) == declared_site_mechanism_id => Ok(OutputCapability {
            layer_id: output.id,
            source_filter: output.source_filter,
            consumes: product,
            payload: OutputCapabilityPayload::Marks {
                prototype: MarkPrototype::Circle,
                orientation: MarkOrientation::Fixed,
            },
        }),
        PatternOutputRealization::MarkPrototype {
                site_mechanism_id,
                prototype,
                orientation: MarkOrientation::Fixed,
            } if Some(*site_mechanism_id) == declared_site_mechanism_id => Ok(OutputCapability {
            layer_id: output.id,
            source_filter: output.source_filter,
            consumes: product,
            payload: OutputCapabilityPayload::Marks {
                prototype: prototype.clone(),
                orientation: MarkOrientation::Fixed,
            },
        }),
        PatternOutputRealization::ConnectionPaths {
                site_mechanism_id,
                program,
                style,
            } if Some(*site_mechanism_id) == declared_site_mechanism_id => Ok(OutputCapability {
            layer_id: output.id,
            source_filter: output.source_filter,
            consumes: product,
            payload: OutputCapabilityPayload::ConnectionPaths {
                site_mechanism_id: *site_mechanism_id,
                program: program.clone(),
                style: *style,
            },
        }),
        PatternOutputRealization::Regions { source }
            if matches!(source, RegionSourceIntent::VoronoiSites { site_mechanism_id } if Some(*site_mechanism_id) == declared_site_mechanism_id)
                && product == StructuralProductCapability::AlongGuideSites =>
        {
            Ok(OutputCapability {
                layer_id: output.id,
                source_filter: output.source_filter,
                consumes: product,
                payload: OutputCapabilityPayload::Regions {
                    source: source.clone(),
                },
            })
        }
        _ => Err(PatternPipelineError::new(
                "pattern.output_layers.capability",
                "parametric output cannot consume the declared structural product",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let evaluation_order = pattern_output_evaluation_order(definition)
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
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
        ordered_outputs,
        evaluation_order,
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
    if let Some((source_id, selected)) = definition.output_layers.iter().find_map(|output| {
        let PatternOutputRealization::Regions {
            source:
                RegionSourceIntent::GuideFaces {
                    guide_mechanism_id,
                    dimensions,
                },
            ..
        } = &output.realization
        else {
            return None;
        };
        Some((*guide_mechanism_id, dimensions))
    }) && (source_id != guide_id
        || !(2..=3).contains(&selected.len())
        || selected.iter().any(|selected_id| {
            !generic_dimensions.iter().any(|dimension| {
                dimension.id == *selected_id
                    && matches!(dimension.prototype, GuidePrototype::AuthoredOpenPath { .. })
            })
        }))
    {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.guide_faces",
            "Guide Faces requires two or three selected authored open guide paths",
        ));
    }
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

/// Resolves the fixed random mechanism chain and its typed mark or connection output capability.
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
    let ordered_outputs = definition
        .output_layers
        .iter()
        .map(|output| match &output.realization {
            PatternOutputRealization::MarkPrototype {
                site_mechanism_id,
                prototype,
                orientation: MarkOrientation::Fixed,
            } if *site_mechanism_id == site_product_id => Ok(OutputCapability {
                layer_id: output.id,
                source_filter: output.source_filter,
                consumes: StructuralProductCapability::RandomSites,
                payload: OutputCapabilityPayload::Marks {
                    prototype: prototype.clone(),
                    orientation: MarkOrientation::Fixed,
                },
            }),
            PatternOutputRealization::ConnectionPaths {
                site_mechanism_id,
                program,
                style,
            } if *site_mechanism_id == site_product_id => Ok(OutputCapability {
                layer_id: output.id,
                source_filter: output.source_filter,
                consumes: StructuralProductCapability::RandomSites,
                payload: OutputCapabilityPayload::ConnectionPaths {
                    site_mechanism_id: *site_mechanism_id,
                    program: program.clone(),
                    style: *style,
                },
            }),
            PatternOutputRealization::Regions {
                source: RegionSourceIntent::VoronoiSites { site_mechanism_id },
            } if *site_mechanism_id == site_product_id => Ok(OutputCapability {
                layer_id: output.id,
                source_filter: output.source_filter,
                consumes: StructuralProductCapability::RandomSites,
                payload: OutputCapabilityPayload::Regions {
                    source: RegionSourceIntent::VoronoiSites {
                        site_mechanism_id: site_product_id,
                    },
                },
            }),
            _ => Err(PatternPipelineError::new(
                "pattern.output_layers.capability",
                "random-site products require fixed marks, connections, or Voronoi regions",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let product = StructuralProductCapability::RandomSites;
    let evaluation_order = pattern_output_evaluation_order(definition)
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
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
        ordered_outputs,
        evaluation_order,
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
    evaluate_typed_family_product_with_source_progress_cancellable(
        family,
        request,
        source,
        is_cancelled,
        &|_, _| {},
    )
}

/// Evaluates a family while reporting completed units within expensive site-generation phases.
///
/// Progress is non-authoritative, excluded from family identity, and may be
/// called from evaluator work without retaining the callback. Existing callers
/// that do not need progress use [`evaluate_typed_family_product_with_source_cancellable`].
///
/// # Errors
///
/// Returns the same resolver, coverage, geometry, limit, or cancellation
/// diagnostics as the non-observed entry point, without publishing partial
/// family output.
pub fn evaluate_typed_family_product_with_source_progress_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    source: Option<&SourceField>,
    is_cancelled: &dyn Fn() -> bool,
    report_progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<TypedFamilyOutput, PatternPipelineError> {
    if family.parametric_curve.is_some() {
        return evaluate_parametric_curve_with_progress_cancellable(
            family,
            request,
            is_cancelled,
            report_progress,
        );
    }
    if family.generic_guides.is_some() {
        return evaluate_generic_curve_guides_with_progress_cancellable(
            family,
            request,
            is_cancelled,
            report_progress,
        );
    }
    if family.product == StructuralProductCapability::RandomSites {
        let output = evaluate_random_sites_with_progress_cancellable(
            family,
            request,
            source,
            is_cancelled,
            report_progress,
        )?;
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
        let output = evaluate_generalized_straight_guides_with_progress_cancellable(
            family,
            &StraightGuideInspectRequest {
                canvas: request.canvas.clone(),
                density: request.density,
                rotation_degrees: request.rotation_degrees,
                translation_x: request.translation_x,
                translation_y: request.translation_y,
                guard_steps: request.guard_steps,
                support_radius: request.support_radius,
                max_family_candidates: request.max_family_candidates,
            },
            is_cancelled,
            report_progress,
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

/// Maps completed local work into one fixed per-mille portion of family progress.
///
/// Zero totals remain at the portion start; callers publish their phase end only
/// after the corresponding private result is complete.
fn report_family_progress_portion(
    report_progress: &(dyn Fn(usize, usize) + Sync),
    start_per_mille: usize,
    weight_per_mille: usize,
    completed: usize,
    total: usize,
) {
    let local = if total == 0 {
        0
    } else {
        (weight_per_mille as u128 * completed.min(total) as u128 / total as u128) as usize
    };
    report_progress(start_per_mille.saturating_add(local).min(1_000), 1_000);
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

/// Evaluates one authored connection output through capability validation, full support coverage, graph construction, and path selection.
///
/// # Errors
///
/// Returns a stable program, capability, coverage, adjacency, connection, or cancellation diagnostic
/// without exposing partial derived products.
#[allow(clippy::too_many_arguments)] // This public boundary keeps the existing family/coverage/limit authorities explicit.
pub fn evaluate_typed_connection_paths_with_source_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    source: Option<&SourceField>,
    output_layer_id: PatternOutputLayerId,
    program: &ConnectionProgram,
    adjacency_limits: SiteAdjacencyLimits,
    connection_limits: ConnectionPathLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<ConnectionPathEvaluation, PatternPipelineError> {
    program
        .validate()
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
    validate_connection_product(family.product, program)?;
    let adjacency = program.adjacency();
    let policy = SiteAdjacencyPolicy::MutualNearest {
        maximum_degree: adjacency.maximum_degree as usize,
        maximum_distance: adjacency.maximum_distance,
    };
    let evaluated = evaluate_typed_site_adjacency_with_source_cancellable(
        family,
        request,
        source,
        policy,
        adjacency_limits,
        is_cancelled,
    )?;
    let paths = build_connection_paths_cancellable(
        output_layer_id,
        &evaluated.graph,
        program,
        connection_limits,
        is_cancelled,
    )
    .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
    Ok(ConnectionPathEvaluation {
        family: evaluated.family,
        graph: evaluated.graph,
        paths,
    })
}

/// Evaluates one typed straight-guide intersection family into conventional retained maze walls.
///
/// The family envelope includes one complete outer cell ring for coverage, while geometry retains
/// only actual sites inside or on the document canvas before deriving walls and cells. The canvas
/// never manufactures arrangement edges or faces.
///
/// # Errors
///
/// Returns a stable typed-product, coverage, family, arrangement, limit, or cancellation error
/// without exposing a partial maze result.
pub fn evaluate_typed_maze_walls_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    output_layer_id: PatternOutputLayerId,
    program: &MazeProgram,
    limits: MazeLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<MazeProgramResult, PatternPipelineError> {
    if request.guard_steps == 0 {
        return Err(PatternPipelineError::new(
            "maze.coverage.guard_steps",
            "wall mazes require at least one request coverage guard step",
        ));
    }
    if family.product != StructuralProductCapability::GuideIntersections
        || family.generic_guides.is_some()
        || family.dimensions.len() < 2
        || family.dimensions.len() > 3
    {
        return Err(PatternPipelineError::new(
            "maze.family.product",
            "wall mazes require two or three typed straight-guide intersection dimensions",
        ));
    }
    program
        .validate()
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
    if !request.support_radius.is_finite() {
        return Err(PatternPipelineError::new(
            "maze.coverage.support",
            "maze outer-cell-ring support must remain finite",
        ));
    }
    // The generalized family evaluator already expands the base support by its active
    // guard-step spacing.  Supplying the base request here therefore retains one complete
    // outer cell ring without applying that guard expansion a second time.
    let evaluated = evaluate_typed_family_product_cancellable(family, request, is_cancelled)?;
    evaluate_typed_maze_walls_from_family_cancellable(
        &evaluated,
        request,
        output_layer_id,
        program,
        limits,
        is_cancelled,
    )
}

/// Builds a maze from an accepted family after retaining only its inclusive document-canvas sites.
///
/// # Errors
///
/// Returns a stable support, typed-product, arrangement, limit, or cancellation error atomically.
pub fn evaluate_typed_maze_walls_from_family_cancellable(
    family: &TypedFamilyOutput,
    request: &GridInspectRequest,
    output_layer_id: PatternOutputLayerId,
    program: &MazeProgram,
    limits: MazeLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<MazeProgramResult, PatternPipelineError> {
    if is_cancelled() {
        return Err(PatternPipelineError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ));
    }
    if request.guard_steps == 0 {
        return Err(PatternPipelineError::new(
            "maze.coverage.guard_steps",
            "wall mazes require at least one request coverage guard step",
        ));
    }
    if family.guard_steps() == 0 || family.guard_steps() != request.guard_steps {
        return Err(PatternPipelineError::new(
            "maze.coverage.family_guard_steps",
            "accepted maze family guard policy must exactly match the requested nonzero guard",
        ));
    }
    if family.family.product != StructuralProductCapability::GuideIntersections
        || family.family.generic_guides.is_some()
        || !(2..=3).contains(&family.family.dimensions.len())
    {
        return Err(PatternPipelineError::new(
            "maze.family.product",
            "wall mazes require two or three typed straight-guide intersection dimensions",
        ));
    }
    let exact_support = request.support_radius;
    if !exact_support.is_finite() || family.planned_support_radius() + 1e-12 < exact_support {
        return Err(PatternPipelineError::new(
            "maze.coverage.support",
            "accepted family envelope does not cover the requested maze support",
        ));
    }
    let canvas = Bounds::new(
        Point2::new(0.0, 0.0),
        Point2::new(request.canvas.width, request.canvas.height),
    )
    .ok_or(PatternPipelineError::new(
        "maze.coverage.support",
        "maze canvas must remain finite",
    ))?;
    let mut site_count = 0_usize;
    for site in family.site_set().iter() {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        if canvas.contains(site.position) {
            site_count = site_count.checked_add(1).ok_or(PatternPipelineError::new(
                "maze.allocation",
                "exact maze site selection overflows its allocation size",
            ))?;
        }
    }
    let mut sites = Vec::new();
    reserve_stage20m(
        &mut sites,
        site_count,
        "maze.allocation",
        "exact maze site selection allocation failed",
    )?;
    for site in family.site_set().iter() {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        if canvas.contains(site.position) {
            sites.push(site.clone());
        }
    }
    if sites.is_empty() {
        return Err(PatternPipelineError::new(
            "maze.coverage.support",
            "requested maze support contains no family sites",
        ));
    }
    let identity = generalized_fingerprint(
        &family.family,
        &StraightGuideInspectRequest {
            canvas: request.canvas.clone(),
            density: request.density,
            rotation_degrees: request.rotation_degrees,
            translation_x: request.translation_x,
            translation_y: request.translation_y,
            guard_steps: request.guard_steps,
            support_radius: exact_support,
            max_family_candidates: request.max_family_candidates,
        },
    );
    let mut guide_axes = Vec::new();
    reserve_stage20m(
        &mut guide_axes,
        family.straight_guides().len(),
        "maze.allocation",
        "maze guide-axis allocation failed",
    )?;
    for guide in family.straight_guides() {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        guide_axes.push(
            MazeGuideAxis::new(guide.id, guide.tangent)
                .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?,
        );
    }
    build_maze_walls_from_sites_cancellable(
        output_layer_id,
        &identity,
        canvas,
        &guide_axes,
        &sites,
        program,
        limits,
        is_cancelled,
    )
    .map_err(|error| PatternPipelineError::new(error.path(), error.message()))
}

/// Reserves bounded material Stage 20M vectors through one stable allocation diagnostic mapping.
///
/// # Errors
///
/// Returns the supplied stable allocation diagnostic when `Vec` cannot reserve the requested
/// bounded material capacity. Standard `BTreeMap`/`BTreeSet` insertion remains infallible in the
/// Rust standard library and is deliberately not replaced here.
fn reserve_stage20m<T>(
    values: &mut Vec<T>,
    additional: usize,
    path: &'static str,
    message: &'static str,
) -> Result<(), PatternPipelineError> {
    values
        .try_reserve(additional)
        .map_err(|_| PatternPipelineError::new(path, message))
}

/// Enforces the typed site-product eligibility contract before family or graph allocation.
fn validate_connection_product(
    product: StructuralProductCapability,
    program: &ConnectionProgram,
) -> Result<(), PatternPipelineError> {
    if product == StructuralProductCapability::ParametricPaths {
        return Err(PatternPipelineError::new(
            "connection.family.product",
            "raw parametric-path products do not publish sites for connections",
        ));
    }
    let broadly_eligible = matches!(
        product,
        StructuralProductCapability::GuideIntersections
            | StructuralProductCapability::AlongGuideSites
            | StructuralProductCapability::RandomSites
    );
    if !broadly_eligible {
        return Err(PatternPipelineError::new(
            "connection.family.product",
            "resolved family product does not publish eligible connection sites",
        ));
    }
    if matches!(program, ConnectionProgram::GridSpanningTree { .. })
        && product != StructuralProductCapability::GuideIntersections
    {
        return Err(PatternPipelineError::new(
            "connection.family.product",
            "maze and spanning-tree programs require guide-intersection sites",
        ));
    }
    Ok(())
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
fn evaluate_parametric_curve_with_progress_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    is_cancelled: &dyn Fn() -> bool,
    report_progress: &(dyn Fn(usize, usize) + Sync),
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
    report_progress(50, 1_000);
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
    report_progress(100, 1_000);
    let nested_progress = |completed, total| {
        report_family_progress_portion(report_progress, 100, 900, completed, total);
    };
    let mut output = evaluate_generic_curve_guides_with_progress_cancellable(
        &reusable,
        request,
        is_cancelled,
        &nested_progress,
    )?;
    output.family = family.clone();
    report_progress(1_000, 1_000);
    Ok(output)
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
    let tolerance = PathOffsetLimits::default().tolerance;
    let mut inspected_normal = false;
    for segment_index in 0..source.segments().len() {
        for parameter in [0.0, 0.5, 1.0] {
            let normal = match source.unit_normal_at(PathLocation::new(segment_index, parameter)?) {
                Ok(normal) => normal,
                Err(error) if error.path() == "curve.path.tangent.stationary" => continue,
                Err(error) => return Err(error.into()),
            };
            inspected_normal = true;
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
    Ok(inspected_normal)
}

/// Reports whether every open Constant-gap endpoint inside the generation domain is a shared node.
///
/// Exact crossing authority copies one solved coordinate into every incident segment, so bitwise
/// coordinate identity is required instead of a renderer-scale proximity test. Boundary endpoints
/// belong to final clipping and are intentionally excluded. Signed zero is canonicalized because
/// it denotes the same exact Cartesian coordinate.
fn constant_gap_interior_endpoints_are_paired(
    paths: &[StructuralPathInstance],
    domain: Bounds,
) -> bool {
    let coordinate_bits = |value: f64| if value == 0.0 { 0 } else { value.to_bits() };
    let point_key = |point: Point2| (coordinate_bits(point.x), coordinate_bits(point.y));
    let mut node_counts = BTreeMap::<(u64, u64), usize>::new();
    for segment in paths.iter().flat_map(|path| path.path.segments()) {
        for point in [segment.start(), segment.end()] {
            *node_counts.entry(point_key(point)).or_default() += 1;
        }
    }
    paths
        .iter()
        .flat_map(|path| [path.path.start(), path.path.end()])
        .filter(|endpoint| {
            endpoint.x > domain.min.x
                && endpoint.x < domain.max.x
                && endpoint.y > domain.min.y
                && endpoint.y < domain.max.y
        })
        .all(|endpoint| node_counts.get(&point_key(endpoint)).copied().unwrap_or(0) >= 2)
}

/// Returns one curve's conservative projection range on an exact finite unit axis.
///
/// The path is rotated so the requested axis becomes the local x-axis, then canonical analytic
/// curve bounds own the extrema calculation. This avoids projecting an axis-aligned bounds box,
/// whose empty corners can under-count required bilateral Constant-gap ranks.
///
/// # Errors
///
/// Returns finite-transform, curve-bounds, or numeric-overflow diagnostics without publishing a
/// partial range.
fn curve_projection_range(
    path: &CurvePath,
    axis: Vector2,
) -> Result<(f64, f64), PatternPipelineError> {
    let axis_length = axis.x.hypot(axis.y);
    if !axis_length.is_finite() || (axis_length - 1.0).abs() > 1.0e-9 {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.proof",
            "normal-offset projection axis must remain finite and normalized",
        ));
    }
    let transform = AffineTransform2D::rotate_about_then_translate(
        Point2::new(0.0, 0.0),
        -axis.y.atan2(axis.x).to_degrees(),
        Vector2::new(0.0, 0.0),
    )
    .ok_or(PatternPipelineError::new(
        "coverage.curved_guides.proof",
        "normal-offset projection transform could not be represented",
    ))?;
    let bounds = path.transformed(transform)?.bounds()?;
    Ok((bounds.min.x, bounds.max.x))
}

/// Resolves the authored frame's stable transverse axis from its fixed left/right endpoints.
///
/// Guide-editor endpoint authority makes the terminal chord the repetition-frame baseline even
/// when the interior curve reverses. Its left-hand perpendicular therefore remains stable across
/// rotations and topology events.
///
/// # Errors
///
/// Returns a stable coverage diagnostic when the terminal chord is non-finite or collapsed.
fn authored_guide_transverse_axis(path: &CurvePath) -> Result<Vector2, PatternPipelineError> {
    let delta = Vector2::new(path.end().x - path.start().x, path.end().y - path.start().y);
    let length = delta.x.hypot(delta.y);
    if !length.is_finite() || length <= 1.0e-12 {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.normal_offset",
            "authored Constant-gap guide requires distinct left and right endpoints",
        ));
    }
    Ok(Vector2::new(-delta.y / length, delta.x / length))
}

/// Computes bilateral authored Constant-gap ranks in the stable guide-authoring frame.
///
/// The calculation uses exact curve projection extrema rather than changing interior segment
/// normals. Interior normals may reverse at cusps; treating those reversals as family axes can
/// falsely claim one side is covered after a single rank. The caller's guard is applied after the
/// two signed rank bounds are rounded outward. A symmetric canvas-center-to-corner rank floor plus
/// two clipping ranks prevents an early projected-edge proof from leaving a diagonal canvas wedge
/// uncovered.
///
/// # Errors
///
/// Returns stable spacing, endpoint, transform, bounds, or integer-overflow diagnostics.
fn authored_normal_offset_rank_bounds(
    source: &CurvePath,
    domain: Bounds,
    canvas_center_to_corner: f64,
    spacing: f64,
    guard: i64,
) -> Result<(i64, i64), PatternPipelineError> {
    if !spacing.is_finite()
        || spacing <= 0.0
        || !canvas_center_to_corner.is_finite()
        || canvas_center_to_corner < 0.0
    {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.normal_offset",
            "normal-offset spacing and canvas extent must remain finite and positive",
        ));
    }
    let axis = authored_guide_transverse_axis(source)?;
    let (source_minimum, source_maximum) = curve_projection_range(source, axis)?;
    let (domain_minimum, domain_maximum) = domain
        .corners()
        .into_iter()
        .map(|point| point.dot(axis))
        .fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(minimum, maximum), projection| (minimum.min(projection), maximum.max(projection)),
        );
    let first_raw = ((domain_minimum - source_minimum) / spacing).floor();
    let last_raw = ((domain_maximum - source_maximum) / spacing).ceil();
    if !first_raw.is_finite()
        || !last_raw.is_finite()
        || first_raw < i64::MIN as f64
        || last_raw > i64::MAX as f64
    {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.numeric_overflow",
            "normal-offset rank arithmetic overflowed",
        ));
    }
    let projected_first = (first_raw as i64)
        .checked_sub(guard)
        .ok_or(PatternPipelineError::new(
            "coverage.curved_guides.numeric_overflow",
            "normal-offset rank arithmetic overflowed",
        ))?
        .min(0);
    let projected_last = (last_raw as i64)
        .checked_add(guard)
        .ok_or(PatternPipelineError::new(
            "coverage.curved_guides.numeric_overflow",
            "normal-offset rank arithmetic overflowed",
        ))?
        .max(0);
    const DIAGONAL_CLIPPING_RANKS: i64 = 2;
    let diagonal_raw = (canvas_center_to_corner / spacing).ceil();
    if !diagonal_raw.is_finite() || diagonal_raw > i64::MAX as f64 {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.numeric_overflow",
            "normal-offset diagonal rank arithmetic overflowed",
        ));
    }
    let diagonal_rank = (diagonal_raw as i64)
        .checked_add(DIAGONAL_CLIPPING_RANKS)
        .ok_or(PatternPipelineError::new(
            "coverage.curved_guides.numeric_overflow",
            "normal-offset diagonal rank arithmetic overflowed",
        ))?;
    let negative_diagonal_rank = diagonal_rank
        .checked_neg()
        .ok_or(PatternPipelineError::new(
            "coverage.curved_guides.numeric_overflow",
            "normal-offset diagonal rank arithmetic overflowed",
        ))?;
    Ok((
        projected_first.min(negative_diagonal_rank),
        projected_last.max(diagonal_rank),
    ))
}

/// Proves one authored frontier reaches the requested transverse edge of the generation domain.
///
/// This uses the same fixed endpoint-owned transverse axis as authored rank planning, so local
/// tangent reversals cannot make the negative side appear covered by a positive lobe.
///
/// # Errors
///
/// Returns endpoint, transform, or curve-bounds diagnostics without accepting partial coverage.
fn authored_normal_offset_components_bracket_domain(
    paths: &[toniator_geometry::OffsetPathComponent],
    source: &CurvePath,
    domain: Bounds,
    signed_side: f64,
) -> Result<bool, PatternPipelineError> {
    if paths.is_empty() {
        return Ok(false);
    }
    let axis = authored_guide_transverse_axis(source)?;
    let (domain_minimum, domain_maximum) = domain
        .corners()
        .into_iter()
        .map(|point| point.dot(axis))
        .fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(minimum, maximum), projection| (minimum.min(projection), maximum.max(projection)),
        );
    let frontier = authored_normal_offset_frontier_projection(paths, source, signed_side)?
        .expect("nonempty authored offset components have a frontier");
    let tolerance = PathOffsetLimits::default().tolerance;
    Ok(if signed_side > 0.0 {
        frontier + tolerance >= domain_maximum
    } else {
        frontier - tolerance <= domain_minimum
    })
}

/// Returns the farthest retained component projection on one signed authored-guide side.
///
/// The endpoint-owned transverse axis remains stable across ranks. This makes a cusp-induced
/// frontier reversal observable without allowing another lobe to substitute for progress on the
/// requested side.
///
/// # Errors
///
/// Returns endpoint, transform, or curve-bounds diagnostics without accepting a non-finite
/// component projection.
fn authored_normal_offset_frontier_projection(
    paths: &[toniator_geometry::OffsetPathComponent],
    source: &CurvePath,
    signed_side: f64,
) -> Result<Option<f64>, PatternPipelineError> {
    if paths.is_empty() {
        return Ok(None);
    }
    let axis = authored_guide_transverse_axis(source)?;
    let mut path_minimum = f64::INFINITY;
    let mut path_maximum = f64::NEG_INFINITY;
    for component in paths {
        let (minimum, maximum) = curve_projection_range(&component.path, axis)?;
        path_minimum = path_minimum.min(minimum);
        path_maximum = path_maximum.max(maximum);
    }
    Ok(Some(if signed_side > 0.0 {
        path_maximum
    } else {
        path_minimum
    }))
}

/// Advances one authored Constant-gap side by exactly one preset spacing rank.
///
/// Open exterior components from the preceding rank seed the next rank. Closed reversal loops
/// created while solving a self-crossing are construction-only topology: Dissolve crossings keeps
/// their exact shared intersection on the exterior continuation, but does not publish or advance
/// the enclosed loop. Each child still runs the geometry-owned segment offset, exact crossing
/// solve, and relink pipeline before receiving a deterministic component ordinal.
///
/// # Errors
///
/// Returns cancellation, geometry, component-limit, or integer-conversion diagnostics without
/// exposing a partially advanced rank.
fn advance_constant_gap_frontier(
    previous: &[toniator_geometry::OffsetPathComponent],
    signed_spacing: f64,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<toniator_geometry::OffsetPathComponent>, PatternPipelineError> {
    let limits = PathOffsetLimits::default();
    let mut next = Vec::new();
    for component in previous {
        if component.path.closure() != PathClosure::Open {
            continue;
        }
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        let result = advance_planar_constant_gap_frontier_cancellable(
            &component.path,
            &component.planar_switch_nodes,
            signed_spacing,
            limits,
            is_cancelled,
        )?;
        if let PathOffsetResult::Paths(children) = result {
            next.extend(
                children
                    .into_iter()
                    .filter(|child| child.path.closure() == PathClosure::Open),
            );
        }
        if next.len() > limits.maximum_components {
            return Err(PatternPipelineError::new(
                "coverage.curved_guides.instance_limit",
                "constant-gap frontier exceeds the configured component limit",
            ));
        }
    }
    for (ordinal, component) in next.iter_mut().enumerate() {
        component.component_ordinal = u32::try_from(ordinal).map_err(|_| {
            PatternPipelineError::new(
                "coverage.curved_guides.instance_limit",
                "constant-gap component ordinal exceeds its identity range",
            )
        })?;
    }
    Ok(next)
}

/// Bounds the farthest offset index that can still intersect the generation domain.
///
/// Authored curve points stay inside `source`, while tangential endpoint extensions stay inside
/// `domain`. The union of those bounds therefore contains every construction-source point. Any
/// normal-offset point inside `domain` must be no farther from that source point than the maximum
/// corner-to-corner distance used here. The fitting tolerance is included before conversion to a
/// repetition index.
///
/// # Errors
///
/// Returns a stable coverage diagnostic when spacing is not positive or the finite geometric bound
/// cannot be represented as an `i64` repetition index.
fn normal_offset_absolute_index_limit(
    source: Bounds,
    domain: Bounds,
    spacing: f64,
) -> Result<i64, PatternPipelineError> {
    if !spacing.is_finite() || spacing <= 0.0 {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.normal_offset",
            "normal-offset spacing must remain finite and positive",
        ));
    }
    let source_envelope = Bounds::new(
        Point2::new(
            source.min.x.min(domain.min.x),
            source.min.y.min(domain.min.y),
        ),
        Point2::new(
            source.max.x.max(domain.max.x),
            source.max.y.max(domain.max.y),
        ),
    )
    .ok_or(PatternPipelineError::new(
        "coverage.curved_guides.numeric_overflow",
        "normal-offset coverage arithmetic overflowed",
    ))?;
    let domain_corners = domain.corners();
    let maximum_distance = source_envelope
        .corners()
        .into_iter()
        .flat_map(|source_point| {
            domain_corners.into_iter().map(move |domain_point| {
                (domain_point.x - source_point.x).hypot(domain_point.y - source_point.y)
            })
        })
        .fold(0.0_f64, f64::max);
    let raw = ((maximum_distance + PathOffsetLimits::default().tolerance) / spacing).ceil();
    if !raw.is_finite() || raw < 0.0 || raw > i64::MAX as f64 {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.numeric_overflow",
            "normal-offset coverage arithmetic overflowed",
        ));
    }
    Ok(raw as i64)
}

/// Uniformly scales one centered authored guide through the local generation span.
///
/// Guide-editor endpoints are fixed on opposite sides of a centered authoring frame. Rotation can
/// require a longer finite guide than that frame supplies, so this applies one shape-preserving
/// scale before any Single, Stacked, or Constant-gap repetition. Repetition pitch remains separate
/// domain intent and is never derived from the scaled path. Final canvas clipping remains a
/// downstream consumer operation.
///
/// # Errors
///
/// Returns stable finite-geometry or coverage diagnostics when the authored terminal span cannot
/// bracket the centered generation domain or its scale cannot be represented.
fn scale_authored_guide_to_generation_span(
    path: &CurvePath,
    domain: Bounds,
) -> Result<CurvePath, PatternPipelineError> {
    let terminal_delta = Vector2::new(path.end().x - path.start().x, path.end().y - path.start().y);
    let terminal_length = terminal_delta.x.hypot(terminal_delta.y);
    if !terminal_length.is_finite() || terminal_length <= 1.0e-12 {
        return Ok(path.clone());
    }
    let tangent = terminal_delta.scale(terminal_length.recip());
    let terminal_minimum = path.start().dot(tangent).min(path.end().dot(tangent));
    let terminal_maximum = path.start().dot(tangent).max(path.end().dot(tangent));
    let (domain_minimum, domain_maximum) = domain
        .corners()
        .into_iter()
        .map(|point| point.dot(tangent))
        .fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(minimum, maximum), projection| (minimum.min(projection), maximum.max(projection)),
        );
    if !terminal_minimum.is_finite()
        || !terminal_maximum.is_finite()
        || !domain_minimum.is_finite()
        || !domain_maximum.is_finite()
    {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.proof",
            "authored guide terminals must bracket the centered generation span",
        ));
    }
    if terminal_minimum >= 0.0 || terminal_maximum <= 0.0 {
        return Ok(path.clone());
    }
    let scale = 1.0_f64
        .max((domain_minimum / terminal_minimum).max(1.0))
        .max((domain_maximum / terminal_maximum).max(1.0));
    let transform = AffineTransform2D::scale_then_rotate_about_then_translate(
        Point2::new(0.0, 0.0),
        scale,
        scale,
        0.0,
        Vector2::new(0.0, 0.0),
    )
    .ok_or(PatternPipelineError::new(
        "coverage.curved_guides.proof",
        "curved-guide coverage scale must remain finite and positive",
    ))?;
    path.transformed(transform).map_err(Into::into)
}

/// Translates one authored segment for a tiled guide while forcing exact path continuity.
///
/// The first anchor supplied by the caller is the previous derived segment's exact endpoint; all
/// remaining construction points receive the same finite tile translation. This avoids a
/// floating-point seam without smoothing, reshaping, or changing line/cubic construction kind.
///
/// # Errors
///
/// Returns canonical finite-coordinate diagnostics when the translated construction overflows.
fn translated_tiled_segment(
    segment: CurveSegment,
    start: Point2,
    translation: Vector2,
) -> Result<CurveSegment, PatternPipelineError> {
    let translate = |point: Point2| {
        let translated = Point2::new(point.x + translation.x, point.y + translation.y);
        translated
            .is_finite()
            .then_some(translated)
            .ok_or(PatternPipelineError::new(
                "coverage.curved_guides.numeric_overflow",
                "curved-guide tiling arithmetic overflowed",
            ))
    };
    match segment {
        CurveSegment::Line(line) => Ok(CurveSegment::Line(LineSegment::new(
            start,
            translate(line.end())?,
        )?)),
        CurveSegment::CubicBezier(cubic) => Ok(CurveSegment::CubicBezier(CubicBezierSegment::new(
            start,
            translate(cubic.control_1())?,
            translate(cubic.control_2())?,
            translate(cubic.end())?,
        )?)),
    }
}

/// Tiles one authored open guide end-to-end beyond every planned normal-offset endpoint.
///
/// Constant-gap copies remain independent guide instances. The authored curve is repeated only
/// along its start-to-end vector. Repeated cubic seams retain the next tile's handle length while
/// aligning its outgoing direction with the preceding tile's incoming tangent, preventing a finite
/// tile boundary from becoming a false Constant-gap cusp. Adjacent offsets are never wrapped into a
/// fingerprint-style serpentine path. The caller has already applied the one-time uniform coverage
/// scale, so tiling cannot change repetition pitch.
///
/// # Errors
///
/// Returns stable finite-coverage, segment-limit, or canonical path diagnostics before exposing a
/// partial tiled centerline.
fn tile_authored_guide_end_to_end(
    path: &CurvePath,
    domain: Bounds,
    longitudinal_margin: f64,
) -> Result<CurvePath, PatternPipelineError> {
    let terminal_delta = Vector2::new(path.end().x - path.start().x, path.end().y - path.start().y);
    let terminal_span = terminal_delta.x.hypot(terminal_delta.y);
    if !terminal_span.is_finite()
        || terminal_span <= 1.0e-12
        || !longitudinal_margin.is_finite()
        || longitudinal_margin < 0.0
    {
        return Ok(path.clone());
    }
    let tangent = terminal_delta.scale(terminal_span.recip());
    let start_projection = path.start().dot(tangent);
    let end_projection = path.end().dot(tangent);
    let (domain_minimum, domain_maximum) = domain
        .corners()
        .into_iter()
        .map(|point| point.dot(tangent))
        .fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(minimum, maximum), projection| (minimum.min(projection), maximum.max(projection)),
        );
    let first_raw =
        ((domain_minimum - longitudinal_margin - start_projection) / terminal_span).floor();
    let last_raw = ((domain_maximum + longitudinal_margin - end_projection) / terminal_span).ceil();
    if !first_raw.is_finite()
        || !last_raw.is_finite()
        || first_raw < i64::MIN as f64
        || last_raw > i64::MAX as f64
    {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.numeric_overflow",
            "curved-guide tiling arithmetic overflowed",
        ));
    }
    let first = (first_raw as i64).min(0);
    let last = (last_raw as i64).max(0);
    let tile_count = last
        .checked_sub(first)
        .and_then(|count| count.checked_add(1))
        .ok_or(PatternPipelineError::new(
            "coverage.curved_guides.numeric_overflow",
            "curved-guide tiling arithmetic overflowed",
        ))?;
    let segment_count = usize::try_from(tile_count)
        .ok()
        .and_then(|count| count.checked_mul(path.segments().len()))
        .filter(|count| *count <= 4_096)
        .ok_or(PatternPipelineError::new(
            "coverage.curved_guides.instance_limit",
            "tiled curved-guide segment count exceeds the configured path limit",
        ))?;
    let mut segments = Vec::<CurveSegment>::with_capacity(segment_count);
    for tile_index in first..=last {
        let amount = tile_index as f64;
        let translation = terminal_delta.scale(amount);
        for (segment_index, segment) in path.segments().iter().enumerate() {
            let start = segments.last().map_or_else(
                || {
                    let point = segment.start();
                    Point2::new(point.x + translation.x, point.y + translation.y)
                },
                CurveSegment::end,
            );
            let mut tiled = translated_tiled_segment(*segment, start, translation)?;
            if segment_index == 0
                && let (Some(previous), CurveSegment::CubicBezier(cubic)) =
                    (segments.last().copied(), tiled)
            {
                let incoming = previous.limiting_unit_tangent_at(1.0)?;
                let handle_length = (cubic.control_1().x - cubic.start().x)
                    .hypot(cubic.control_1().y - cubic.start().y);
                let control_1 = Point2::new(
                    cubic.start().x + incoming.x * handle_length,
                    cubic.start().y + incoming.y * handle_length,
                );
                tiled = CurveSegment::CubicBezier(CubicBezierSegment::new(
                    cubic.start(),
                    control_1,
                    cubic.control_2(),
                    cubic.end(),
                )?);
            }
            segments.push(tiled);
        }
    }
    CurvePath::new(segments, PathClosure::Open).map_err(Into::into)
}

/// Resolves the artist-facing uniform Pattern size from canonical density authority.
///
/// Density aspect is deliberately excluded: the geometric mean recovers the persisted scalar
/// density, while the source-size-normalized default provides the same size-one reference used by
/// the main-window control. Callers apply this scale to authored absolute curve distances; ordinary
/// density-derived spacing continues to consume the resolved directional metric directly.
///
/// # Errors
///
/// Returns the domain's finite-positive density diagnostics when the resolved layout cannot be
/// represented, or a stable guide-spacing diagnostic when their ratio is invalid.
fn resolved_pattern_size_scale(
    canvas: &CanvasSpec,
    density: &ResolvedDensityMetric2D,
) -> Result<f64, PatternPipelineError> {
    let current = DensityMetric2D::from_resolved(canvas, density)
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?
        .density;
    let default = DensityMetric2D::default_for_canvas(canvas)
        .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?
        .density;
    let scale = default / current;
    (scale.is_finite() && scale > 0.0)
        .then_some(scale)
        .ok_or(PatternPipelineError::new(
            "pattern.family.guide_spacing",
            "pattern size must resolve to a finite positive curve scale",
        ))
}

/// Evaluates resolved finite guide paths using the authoritative local-grid placement policy.
///
/// Authored grid prototypes use the centered local origin, while the parametric structural-source
/// adapter retains its already-established document placement. Cancellation is observed before
/// allocation and during finite-path expansion.
///
/// # Errors
///
/// Returns bounded coverage, numeric, path, or cancellation errors without publishing a partial
/// typed family.
fn evaluate_generic_curve_guides_with_progress_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    is_cancelled: &dyn Fn() -> bool,
    report_progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<TypedFamilyOutput, PatternPipelineError> {
    #[derive(Clone, Copy)]
    struct NormalOffsetCoveragePlan {
        first_required: i64,
        last_required: i64,
        first_probe_limit: i64,
        last_probe_limit: i64,
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
    let pattern_size_scale = resolved_pattern_size_scale(&request.canvas, &request.density)?;
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
            maximum_spacing = maximum_spacing.max(spacing * pattern_size_scale);
        }
    }
    let margin =
        request.support_radius + ANTIALIAS_MARGIN + request.guard_steps as f64 * maximum_spacing;
    let document_domain = canvas.expanded(margin).ok_or(PatternPipelineError::new(
        "coverage.curved_guides.proof",
        "curved-guide coverage could not prove a complete generation envelope",
    ))?;
    let channel_transform = if generic.structural_source.is_some() {
        // Parametric curves are explicitly centered by their adapter before this shared
        // finite-path evaluator runs, so retain that adapter's established channel transform.
        AffineTransform2D::rotate_about_then_translate(
            Point2::new(request.canvas.width / 2.0, request.canvas.height / 2.0),
            request.rotation_degrees,
            Vector2::new(request.translation_x, request.translation_y),
        )
    } else {
        grid_prototype_local_to_document_transform(
            &request.canvas,
            request.rotation_degrees,
            Vector2::new(request.translation_x, request.translation_y),
        )
    }
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
    let mut requires_solved_crossing_nodes = false;
    let dimension_total = generic.dimensions.len().max(1);
    for (dimension_index, (dimension, (source_structure_id, prototype))) in generic
        .dimensions
        .iter()
        .zip(&generic.resolved_paths)
        .enumerate()
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
        let unscaled_base = prototype.transformed(baseline)?;
        let base = if matches!(dimension.prototype, GuidePrototype::AuthoredOpenPath { .. }) {
            scale_authored_guide_to_generation_span(&unscaled_base, local_domain)?
        } else {
            unscaled_base
        };
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
            GuideRepetition::NormalOffset { spacing, .. } => {
                let spacing = spacing * pattern_size_scale;
                // Constant-gap construction is always bilateral. Visible left/right weighting is
                // output response bias and must not delete guide topology or family sites.
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
                let first = first.min(0);
                let last = last.max(0);
                let (first, last) =
                    if matches!(dimension.prototype, GuidePrototype::AuthoredOpenPath { .. }) {
                        authored_normal_offset_rank_bounds(
                            &base,
                            local_domain,
                            0.5 * request.canvas.width.hypot(request.canvas.height),
                            spacing,
                            i64::from(request.guard_steps),
                        )?
                    } else {
                        (first, last)
                    };
                let (first_probe_limit, last_probe_limit) =
                    if matches!(dimension.prototype, GuidePrototype::AuthoredOpenPath { .. }) {
                        let fallback_ranks = i64::from(request.guard_steps).max(2) + 1;
                        let symmetric_extent = first
                            .saturating_abs()
                            .max(last.saturating_abs())
                            .checked_add(fallback_ranks)
                            .ok_or(PatternPipelineError::new(
                                "coverage.curved_guides.numeric_overflow",
                                "normal-offset coverage arithmetic overflowed",
                            ))?;
                        (
                            symmetric_extent
                                .checked_neg()
                                .ok_or(PatternPipelineError::new(
                                    "coverage.curved_guides.numeric_overflow",
                                    "normal-offset coverage arithmetic overflowed",
                                ))?,
                            symmetric_extent,
                        )
                    } else {
                        let absolute_index_limit =
                            normal_offset_absolute_index_limit(path_bounds, local_domain, spacing)?;
                        (
                            first.min(absolute_index_limit.checked_neg().ok_or(
                                PatternPipelineError::new(
                                    "coverage.curved_guides.numeric_overflow",
                                    "normal-offset coverage arithmetic overflowed",
                                ),
                            )?),
                            last.max(absolute_index_limit),
                        )
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
                        first_probe_limit,
                        last_probe_limit,
                    }),
                )
            }
        };
        let uses_tiled_authored_normal_offset = normal_offset_coverage.is_some()
            && matches!(dimension.prototype, GuidePrototype::AuthoredOpenPath { .. });
        // Iterated authored Constant-gap ranks already solve and relink their crossings before the
        // next frontier is constructed. Re-running an all-rank arrangement here would discard that
        // bounded rank ownership and charge every retained closed event component against one
        // quadratic cleanup budget. Direct one-shot offset families still require the collection
        // arrangement below.
        requires_solved_crossing_nodes |=
            normal_offset_coverage.is_some() && !uses_tiled_authored_normal_offset;
        let evaluation_base = if uses_tiled_authored_normal_offset {
            let coverage = normal_offset_coverage.expect("tiled normal offset owns coverage");
            let maximum_offset_rank = coverage
                .first_probe_limit
                .saturating_abs()
                .max(coverage.last_probe_limit.saturating_abs());
            let maximum_offset_travel = maximum_offset_rank as f64 * spacing;
            let longitudinal_margin = (local_domain.max.x - local_domain.min.x)
                .hypot(local_domain.max.y - local_domain.min.y)
                + maximum_offset_travel;
            tile_authored_guide_end_to_end(&base, local_domain, longitudinal_margin)?
        } else {
            base.clone()
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
                        OffsetCleanup::DissolveCrossings => PathOffsetCleanup::PlanarConstantGap,
                    };
                    match offset_path_cancellable(
                        PathOffsetRequest {
                            path: &evaluation_base,
                            signed_distance: index as f64 * spacing,
                            endpoint_policy: if uses_tiled_authored_normal_offset {
                                PathOffsetEndpointPolicy::Preserve
                            } else {
                                PathOffsetEndpointPolicy::TangentialExtension {
                                    bounds: local_domain,
                                }
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
                    let path = evaluation_base.transformed(local)?;
                    Ok(vec![toniator_geometry::OffsetPathComponent {
                        component_ordinal: 0,
                        source_start: PathLocation::new(0, 0.0)?,
                        source_end: PathLocation::new(evaluation_base.segments().len() - 1, 1.0)?,
                        path,
                        planar_switch_nodes: Vec::new(),
                    }])
                }
            }
        };
        let mut attempts = 0_usize;
        let mut paths_by_index =
            BTreeMap::<i64, Vec<toniator_geometry::OffsetPathComponent>>::new();
        let mut evaluation_indices = indices;
        if normal_offset_coverage.is_some() && !uses_tiled_authored_normal_offset {
            evaluation_indices.sort_by(|left, right| {
                left.unsigned_abs()
                    .cmp(&right.unsigned_abs())
                    .then_with(|| left.cmp(right))
            });
        }
        let planned_attempts = evaluation_indices.len().max(1);
        let report_attempt = |completed| {
            let dimension_start = 400 * dimension_index / dimension_total;
            let dimension_end = 400 * (dimension_index + 1) / dimension_total;
            report_family_progress_portion(
                report_progress,
                dimension_start,
                dimension_end - dimension_start,
                completed,
                planned_attempts,
            );
        };
        if uses_tiled_authored_normal_offset {
            let coverage = normal_offset_coverage.expect("tiled normal offset owns coverage");
            let frontier_tolerance = PathOffsetLimits::default().tolerance;
            let frontier_retracted =
                |previous: Option<f64>, advanced: Option<f64>, signed_side: f64| match (
                    previous, advanced,
                ) {
                    (Some(_), None) => true,
                    (Some(previous), Some(advanced)) if signed_side > 0.0 => {
                        advanced <= previous + frontier_tolerance
                    }
                    (Some(previous), Some(advanced)) => advanced >= previous - frontier_tolerance,
                    _ => false,
                };
            paths_by_index.insert(
                0,
                vec![toniator_geometry::OffsetPathComponent {
                    component_ordinal: 0,
                    source_start: PathLocation::new(0, 0.0)?,
                    source_end: PathLocation::new(evaluation_base.segments().len() - 1, 1.0)?,
                    path: evaluation_base.clone(),
                    planar_switch_nodes: Vec::new(),
                }],
            );
            attempts = 1;
            report_attempt(attempts);
            let mut positive_bracketed = coverage.last_required <= 0;
            let mut index = 1_i64;
            while !positive_bracketed && index <= coverage.last_probe_limit {
                let previous =
                    paths_by_index
                        .get(&(index - 1))
                        .ok_or(PatternPipelineError::new(
                            "coverage.curved_guides.normal_offset",
                            "constant-gap frontier lost its preceding positive rank",
                        ))?;
                let previous_frontier =
                    authored_normal_offset_frontier_projection(previous, &evaluation_base, 1.0)?;
                let mut paths = advance_constant_gap_frontier(previous, spacing, is_cancelled)?;
                let advanced_frontier =
                    authored_normal_offset_frontier_projection(&paths, &evaluation_base, 1.0)?;
                if frontier_retracted(previous_frontier, advanced_frontier, 1.0) {
                    // A cusp can turn the relinked exterior back toward the source. Restart this
                    // rank from the original tiled runway at its absolute distance so coverage
                    // continues on the same signed side without scaling or re-spacing the stack.
                    paths = evaluate_index(index, &[])?
                        .into_iter()
                        .filter(|component| component.path.closure() == PathClosure::Open)
                        .collect();
                }
                let reaches_edge = !paths.is_empty()
                    && authored_normal_offset_components_bracket_domain(
                        &paths,
                        &evaluation_base,
                        local_domain,
                        1.0,
                    )?;
                positive_bracketed = index >= coverage.last_required && reaches_edge;
                paths_by_index.insert(index, paths);
                attempts += 1;
                report_attempt(attempts);
                index = index.checked_add(1).ok_or(PatternPipelineError::new(
                    "coverage.curved_guides.numeric_overflow",
                    "normal-offset rank arithmetic overflowed",
                ))?;
            }
            if !positive_bracketed {
                return Err(PatternPipelineError::new(
                    "coverage.curved_guides.normal_offset",
                    "constant-gap positive frontier did not bracket the generation domain",
                ));
            }
            let mut negative_bracketed = coverage.first_required >= 0;
            let mut index = -1_i64;
            while !negative_bracketed && index >= coverage.first_probe_limit {
                let previous =
                    paths_by_index
                        .get(&(index + 1))
                        .ok_or(PatternPipelineError::new(
                            "coverage.curved_guides.normal_offset",
                            "constant-gap frontier lost its preceding negative rank",
                        ))?;
                let previous_frontier =
                    authored_normal_offset_frontier_projection(previous, &evaluation_base, -1.0)?;
                let mut paths = advance_constant_gap_frontier(previous, -spacing, is_cancelled)?;
                let advanced_frontier =
                    authored_normal_offset_frontier_projection(&paths, &evaluation_base, -1.0)?;
                if frontier_retracted(previous_frontier, advanced_frontier, -1.0) {
                    // See the positive-side reset above. Absolute source distance restores the
                    // signed frontier after a cusp instead of advancing a folded-back remnant.
                    paths = evaluate_index(index, &[])?
                        .into_iter()
                        .filter(|component| component.path.closure() == PathClosure::Open)
                        .collect();
                }
                let reaches_edge = !paths.is_empty()
                    && authored_normal_offset_components_bracket_domain(
                        &paths,
                        &evaluation_base,
                        local_domain,
                        -1.0,
                    )?;
                negative_bracketed = index <= coverage.first_required && reaches_edge;
                paths_by_index.insert(index, paths);
                attempts += 1;
                report_attempt(attempts);
                index = index.checked_sub(1).ok_or(PatternPipelineError::new(
                    "coverage.curved_guides.numeric_overflow",
                    "normal-offset rank arithmetic overflowed",
                ))?;
            }
            if !negative_bracketed {
                return Err(PatternPipelineError::new(
                    "coverage.curved_guides.normal_offset",
                    "constant-gap negative frontier did not bracket the generation domain",
                ));
            }
        } else {
            for index in evaluation_indices {
                attempts += 1;
                let paths = if index == 0 && normal_offset_coverage.is_some() {
                    vec![toniator_geometry::OffsetPathComponent {
                        component_ordinal: 0,
                        source_start: PathLocation::new(0, 0.0)?,
                        source_end: PathLocation::new(evaluation_base.segments().len() - 1, 1.0)?,
                        path: evaluation_base.clone(),
                        planar_switch_nodes: Vec::new(),
                    }]
                } else {
                    evaluate_index(index, &[])?
                };
                paths_by_index.insert(index, paths);
                report_attempt(attempts);
            }
        }
        if let Some(coverage) = normal_offset_coverage {
            if paths_by_index.get(&0).is_none_or(Vec::is_empty) {
                return Err(PatternPipelineError::new(
                    "coverage.curved_guides.normal_offset",
                    "normal-offset source guide collapsed before coverage could be proved",
                ));
            }
            if !uses_tiled_authored_normal_offset && coverage.last_required > 0 {
                let mut probe = coverage.last_required;
                while paths_by_index
                    .get(&probe)
                    .is_some_and(|paths| !paths.is_empty())
                    && !normal_offset_components_bracket_domain(
                        paths_by_index.get(&probe).map_or(&[], Vec::as_slice),
                        &evaluation_base,
                        local_domain,
                        1.0,
                    )?
                    && probe < coverage.last_probe_limit
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
                    let paths = evaluate_index(probe, &[])?;
                    paths_by_index.insert(probe, paths);
                }
            }
            if !uses_tiled_authored_normal_offset && coverage.first_required < 0 {
                let mut probe = coverage.first_required;
                while paths_by_index
                    .get(&probe)
                    .is_some_and(|paths| !paths.is_empty())
                    && !normal_offset_components_bracket_domain(
                        paths_by_index.get(&probe).map_or(&[], Vec::as_slice),
                        &evaluation_base,
                        local_domain,
                        -1.0,
                    )?
                    && probe > coverage.first_probe_limit
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
                    let paths = evaluate_index(probe, &[])?;
                    paths_by_index.insert(probe, paths);
                }
            }
        }
        let mut this_dimension = Vec::new();
        let published_dimension_start = guides.len();
        for (index, paths) in paths_by_index {
            let basis = if spacing > 0.0 {
                spacing
            } else {
                generic.single_nominal_spacing.unwrap_or(
                    (request.canvas.width / request.density.across_x)
                        .max(request.canvas.height / request.density.across_y),
                )
            };
            let mut published_component_ordinal = 0_u32;
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
                for fragment in instance.path.clip_to_bounds(document_domain)? {
                    let fragment = if uses_tiled_authored_normal_offset {
                        stabilize_planar_constant_gap_path(&fragment, PathOffsetLimits::default())?
                    } else {
                        fragment
                    };
                    let mut published = instance.clone();
                    published.id.component_ordinal = published_component_ordinal;
                    published.path = fragment;
                    guide_nominal_bases.insert(published.id, basis);
                    guides.push(published);
                    published_component_ordinal = published_component_ordinal
                        .checked_add(1)
                        .ok_or(PatternPipelineError::new(
                            "coverage.curved_guides.instance_limit",
                            "curved-guide component count exceeds the configured family limit",
                        ))?;
                    if guides.len() > request.max_family_candidates {
                        return Err(PatternPipelineError::new(
                            "coverage.curved_guides.instance_limit",
                            "curved-guide instance count exceeds the configured family limit",
                        ));
                    }
                }
            }
        }
        if uses_tiled_authored_normal_offset
            && !constant_gap_interior_endpoints_are_paired(
                &guides[published_dimension_start..],
                document_domain,
            )
        {
            return Err(PatternPipelineError::new(
                "coverage.curved_guides.normal_offset",
                "constant-gap cleanup left an unpaired endpoint inside the generation domain",
            ));
        }
        grouped.push(this_dimension);
        report_progress(400 * (dimension_index + 1) / dimension_total, 1_000);
    }
    if requires_solved_crossing_nodes {
        let source_paths = guides
            .iter()
            .map(|instance| instance.path.clone())
            .collect::<Vec<_>>();
        let planarized = insert_solved_crossing_nodes_cancellable(
            &source_paths,
            PathOffsetLimits::default(),
            is_cancelled,
        )?;
        for (instance, path) in guides.iter_mut().zip(planarized) {
            instance.path = path;
        }
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
            let guide_pairs = preflight_curve_intersection_work(
                &grouped,
                &selected,
                request.max_family_candidates,
            )?;
            let site_progress = |completed, total| {
                report_family_progress_portion(report_progress, 400, 600, completed, total);
            };
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
                guide_pairs,
                &site_progress,
            )?
        }
        StructuralProductCapability::AlongGuideSites => {
            let multiplier = family.along_interval_multiplier.unwrap_or(1.0);
            let predicted_sites = preflight_curve_along_work(
                &grouped,
                &selected,
                multiplier,
                generic.absolute_site_interval,
                &request.canvas,
                &request.density,
                request.max_family_candidates,
                is_cancelled,
            )?;
            let site_progress = |completed, total| {
                report_family_progress_portion(report_progress, 400, 600, completed, total);
            };
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
                predicted_sites,
                &site_progress,
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
    report_progress(1_000, 1_000);
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
    density: &ResolvedDensityMetric2D,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, PatternPipelineError> {
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
    Ok(predicted.max(1))
}

/// Bounds selected guide pairs, segment products, and merge candidates before curve allocation.
fn preflight_curve_intersection_work(
    grouped: &[Vec<StructuralPathInstance>],
    selected: &[usize],
    limit: usize,
) -> Result<usize, PatternPipelineError> {
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
    Ok(guide_pairs.max(1))
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
    density: &ResolvedDensityMetric2D,
    product_id: PatternMechanismId,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
    guide_pair_total: usize,
    report_progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<Vec<FamilySite>, PatternPipelineError> {
    let mut raw = Vec::<(
        Point2,
        Vec<StructuralPathLocationProvenance>,
        NominalCellBasis,
    )>::new();
    let mut completed_guide_pairs = 0_usize;
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
                            .limiting_unit_tangent_at(contact.first_location())
                            .map_err(|_| {
                                PatternPipelineError::new(
                                    "pattern.family.curved_guides.tangent",
                                    "curved intersections require nonstationary tangents",
                                )
                            })?;
                        let second_tangent = second
                            .path
                            .limiting_unit_tangent_at(contact.second_location())
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
                    completed_guide_pairs = completed_guide_pairs.saturating_add(1);
                    report_family_progress_portion(
                        report_progress,
                        0,
                        500,
                        completed_guide_pairs,
                        guide_pair_total,
                    );
                }
            }
        }
    }
    let mut output: Vec<FamilySite> = Vec::new();
    let raw_total = raw.len().max(1);
    for (raw_index, (point, mut contributors, basis)) in raw.into_iter().enumerate() {
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
            report_family_progress_portion(report_progress, 500, 500, raw_index + 1, raw_total);
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
        report_family_progress_portion(report_progress, 500, 500, raw_index + 1, raw_total);
    }
    report_progress(1_000, 1_000);
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
    density: &ResolvedDensityMetric2D,
    canvas: Bounds,
    generation_domain: Bounds,
    product_id: PatternMechanismId,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
    predicted_sites: usize,
    report_progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<Vec<FamilySite>, PatternPipelineError> {
    let mut output = Vec::new();
    let mut completed_samples = 0_usize;
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
            let start_tangent = guide.path.limiting_unit_tangent_at(start).map_err(|_| {
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
                completed_samples = completed_samples.saturating_add(1);
                if completed_samples == 1 || completed_samples.is_multiple_of(64) {
                    report_progress(completed_samples.min(predicted_sites), predicted_sites);
                }
                let location = measure.location_at_length(local_position)?;
                let tangent = guide.path.limiting_unit_tangent_at(location).map_err(|_| {
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
    report_progress(predicted_sites, predicted_sites);
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
            &ResolvedDensityMetric2D {
                across_x: 4.0,
                across_y: 2.0,
            },
            Bounds::new(Point2::new(0.0, 0.0), Point2::new(40.0, 20.0)).expect("finite canvas"),
            Bounds::new(Point2::new(0.0, 0.0), Point2::new(40.0, 20.0)).expect("finite domain"),
            PatternMechanismId(702),
            32,
            &|| false,
            32,
            &|_, _| {},
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
    density: &ResolvedDensityMetric2D,
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

/// Bounds every possible Stage 20E1 nominal-cell diagonal before family allocation,
/// including generic AlongGuide tangent intervals and resolved transverse spacing.
///
/// # Errors
///
/// Returns a stable pipeline error when validated density or repetition inputs cannot produce a
/// finite positive conservative bound.
pub fn maximum_nominal_cell_diameter(
    family: &FamilyCapability,
    canvas: &CanvasSpec,
    density: &ResolvedDensityMetric2D,
) -> Result<f64, PatternPipelineError> {
    let pattern_size_scale = resolved_pattern_size_scale(canvas, density)?;
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
            GuideRepetition::NormalOffset { spacing, .. } => spacing * pattern_size_scale,
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
                GuideRepetition::NormalOffset { spacing, .. } => {
                    Ok(maximum.max(spacing * pattern_size_scale))
                }
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
                // A generic AlongGuide site owns one tangent interval selected
                // from document density plus its guide's resolved transverse
                // basis. NormalOffset may make that transverse spacing larger
                // than density, so the positive axis sum conservatively bounds
                // every realized two-axis nominal-cell diameter before support
                // allocation without changing any family identity.
                maximum_directional_spacing * along_multiplier
                    + generic_spacing_bound()?.max(maximum_directional_spacing)
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
    density: &ResolvedDensityMetric2D,
) -> Result<f64, PatternPipelineError> {
    let pattern_size_scale = resolved_pattern_size_scale(canvas, density)?;
    if let Some(parametric) = &family.parametric_curve {
        let spacing = match (&parametric.curve, &parametric.repetition) {
            (ParametricCurve::Spiral(_), GuideRepetition::NormalOffset { spacing, .. }) => {
                *spacing * pattern_size_scale
            }
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
                GuideRepetition::NormalOffset { spacing, .. } => spacing * pattern_size_scale,
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

/// Hashes complete resolved generic-guide intent and layout inputs under its placement contract.
///
/// Parametric structural sources keep their Stage 20D v1 identity because their adapter retains
/// its historic document transform. Authored grid prototypes use the centered-local v2 identity.
fn generic_curve_fingerprint(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    generic: &GenericGuideCapability,
) -> String {
    let prefix = if generic.structural_source.is_some() {
        b"toniator-stage-20d-guide-family-v1|arc-policy-fixed-90-degree-cubic-v1".as_slice()
    } else {
        b"toniator-stage-20d-guide-family-v2-centered-local-origin|arc-policy-fixed-90-degree-cubic-v1"
            .as_slice()
    };
    let mut bytes = prefix.to_vec();
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
            GuideRepetition::NormalOffset { spacing, cleanup } => {
                bytes.push(3);
                bytes.extend(spacing.to_bits().to_le_bytes());
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

struct RandomSiteEvaluation {
    family_fingerprint: String,
    generation_domain: Bounds,
    sites: Vec<FamilySite>,
    diagnostics: RandomSiteDiagnostics,
}

struct SpatialIndex {
    cell_size: f64,
    cells: HashMap<(i64, i64), Vec<usize>>,
    neighbor_work: usize,
}

impl SpatialIndex {
    /// Creates an exclusion lookup without an application-authored work ceiling.
    ///
    /// # Errors
    ///
    /// Returns the stable spatial-index diagnostic when cell size is not
    /// finite-positive; allocation, arithmetic, and cancellation remain checked
    /// during use.
    fn new(cell_size: f64) -> Result<Self, PatternPipelineError> {
        if !cell_size.is_finite() || cell_size <= 0.0 {
            return Err(PatternPipelineError::new(
                "coverage.random_sites.spatial_index",
                "exclusion cell size must be positive and finite",
            ));
        }
        Ok(Self {
            cell_size,
            cells: HashMap::new(),
            neighbor_work: 0,
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
    /// Returns the first accepted-site ordinal within the configured exclusion distance.
    ///
    /// The spatial index stores only ordinals, while positions remain in the
    /// final `FamilySite` population. This avoids a second accepted-point
    /// buffer and preserves deterministic ordinal/provenance identity.
    ///
    /// # Errors
    ///
    /// Returns canonical cancellation, coordinate, or neighbor-work failures
    /// without changing the index or accepted-site population.
    fn find_conflict(
        &mut self,
        point: Point2,
        accepted: &[FamilySite],
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
                        if point_distance(point, accepted[index].position) < distance {
                            return Ok(Some(index));
                        }
                    }
                }
            }
        }
        Ok(None)
    }
    /// Inserts one accepted-site ordinal through fallible cell and bucket growth.
    ///
    /// # Errors
    ///
    /// Returns stable coordinate or allocation diagnostics without appending
    /// the ordinal when its cell cannot be represented or reserved.
    fn insert(&mut self, point: Point2, index: usize) -> Result<(), PatternPipelineError> {
        let key = self.cell(point)?;
        if !self.cells.contains_key(&key) {
            self.cells.try_reserve(1).map_err(|_| {
                PatternPipelineError::new(
                    "coverage.random_sites.allocation",
                    "random-site spatial index could not reserve another cell",
                )
            })?;
        }
        let indices = self.cells.entry(key).or_default();
        indices.try_reserve(1).map_err(|_| {
            PatternPipelineError::new(
                "coverage.random_sites.allocation",
                "random-site spatial index cell could not reserve another site",
            )
        })?;
        indices.push(index);
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

/// Evaluates random sites until deterministic geometry-derived convergence and
/// publishes completed candidate work without imposing an app-authored count
/// ceiling.
///
/// # Errors
///
/// Returns stable validation, source, cancellation, arithmetic, allocation, or
/// explicit caller-limit diagnostics without publishing a partial family.
fn evaluate_random_sites_with_progress_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    source: Option<&SourceField>,
    is_cancelled: &dyn Fn() -> bool,
    report_progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<RandomSiteEvaluation, PatternPipelineError> {
    if is_cancelled() {
        return Err(PatternPipelineError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ));
    }
    validate_straight_request(&StraightGuideInspectRequest {
        canvas: request.canvas.clone(),
        density: request.density,
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
        .max(1.0);
    if !requested.is_finite() || requested > usize::MAX as f64 {
        return Err(PatternPipelineError::new(
            "channel.pattern.layout.density",
            "random-site padded count is not machine-representable",
        ));
    }
    let requested = requested as usize;
    if requested > request.max_family_candidates {
        return Err(PatternPipelineError::new(
            "coverage.candidate_limit",
            "random-site requested count exceeds the configured candidate limit",
        ));
    }
    let candidate_budget = request.max_family_candidates;
    if candidate_budget == 0 {
        return Err(PatternPipelineError::new(
            "coverage.random_sites.attempts",
            "random-site attempt budget must be nonzero",
        ));
    }
    let exclusion_distance = required_exclusion_distance(random, request.support_radius);
    let settled_rejection_span = random_settled_rejection_span(random, local, requested)?;
    let parent_count = cluster_parent_count(&random.character, requested);
    let estimated_work = parent_count
        .saturating_add(requested)
        .saturating_add(settled_rejection_span)
        .max(1);
    let mut prng = StablePrng::new(random.seed);
    let parents = cluster_parents(
        &random.character,
        local,
        parent_count,
        &mut prng,
        is_cancelled,
        report_progress,
        estimated_work,
    )?;
    let nominal_cell_basis = density_cell_basis(&request.canvas, &request.density)?;
    let mut sites = Vec::new();
    let mut spatial_index = (exclusion_distance > 0.0)
        .then(|| SpatialIndex::new(exclusion_distance))
        .transpose()?;
    let mut rejected_by_density = 0;
    let mut rejected_by_exclusion = 0;
    let mut rejected_outside_envelope = 0;
    let mut candidates_considered = 0;
    let mut consecutive_rejections = 0_usize;
    for candidate_ordinal in 0..candidate_budget {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        if sites.len() == requested {
            break;
        }
        candidates_considered = candidate_ordinal + 1;
        if candidates_considered == 1 || candidates_considered.is_multiple_of(64) {
            report_progress(
                parent_count
                    .saturating_add(candidates_considered)
                    .min(estimated_work),
                estimated_work,
            );
        }
        let local_point = random_candidate(&random.character, local, &parents, &mut prng);
        let point = transform.apply_point(local_point);
        if !padded.contains(point) {
            rejected_outside_envelope += 1;
            consecutive_rejections =
                consecutive_rejections
                    .checked_add(1)
                    .ok_or(PatternPipelineError::new(
                        "coverage.random_sites.attempts",
                        "random-site convergence counter overflowed",
                    ))?;
            if consecutive_rejections >= settled_rejection_span {
                break;
            }
            continue;
        }
        let density_weight = if !canvas.contains(point) {
            // Guard sites are coverage topology, not visible source-weighted placement. Sampling
            // a clamped zero-valued canvas edge here can otherwise remove the entire exterior
            // hull and leave an ordinary Voronoi output unbounded inside the canvas.
            1.0
        } else {
            match (&random.density_modulation, weighted_source) {
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
                        .map_err(|error| {
                            PatternPipelineError::new(error.path(), error.message())
                        })?;
                    let shaped = artwork_weight_response(sampled, response);
                    (1.0 - strength) + strength * shaped
                }
                _ => unreachable!("weighted source requirement is checked above"),
            }
        };
        if prng.unit() > density_weight {
            rejected_by_density += 1;
            consecutive_rejections =
                consecutive_rejections
                    .checked_add(1)
                    .ok_or(PatternPipelineError::new(
                        "coverage.random_sites.attempts",
                        "random-site convergence counter overflowed",
                    ))?;
            if consecutive_rejections >= settled_rejection_span {
                break;
            }
            continue;
        }
        // `Option::transpose` here would retain the fact that an index exists
        // as `Some(None)`.  That is not a collision: only an actual accepted
        // ordinal rejects the candidate.
        let conflict = match spatial_index.as_mut() {
            Some(index) => index.find_conflict(point, &sites, exclusion_distance, is_cancelled)?,
            None => None,
        };
        if conflict.is_some() {
            rejected_by_exclusion += 1;
            consecutive_rejections =
                consecutive_rejections
                    .checked_add(1)
                    .ok_or(PatternPipelineError::new(
                        "coverage.random_sites.attempts",
                        "random-site convergence counter overflowed",
                    ))?;
            if consecutive_rejections >= settled_rejection_span {
                break;
            }
            // Rejected candidates are aggregated rather than retained.  The
            // accepted record remains bounded and identifies its own ordinal.
            continue;
        }
        let scope = if canvas.contains(point) {
            SiteScope::Canvas
        } else {
            SiteScope::Guard
        };
        let accepted_ordinal = sites.len();
        try_push_family_value(
            &mut sites,
            FamilySite {
                id: FamilySiteId {
                    mechanism_id: family.provenance.mechanism_ids[3],
                    ordinal: accepted_ordinal,
                },
                position: point,
                nominal_cell_basis,
                scope,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal,
                    accepted_ordinal,
                    exclusion_neighbor_ordinal: None,
                },
            },
            "coverage.random_sites.allocation",
            "random-site family output could not reserve another site",
        )?;
        consecutive_rejections = 0;
        if let Some(index) = &mut spatial_index {
            index.insert(point, accepted_ordinal)?;
        }
    }
    // These working populations are not part of the published family result;
    // release them before diagnostics and fingerprint assembly retain only the
    // completed `FamilySite` values.
    drop(spatial_index);
    drop(parents);
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
    report_progress(estimated_work, estimated_work);
    Ok(RandomSiteEvaluation {
        family_fingerprint: random_family_fingerprint(family, request, weighted_source),
        generation_domain: padded,
        sites,
        diagnostics,
    })
}

/// Derives a finite convergence sweep for random placement that stops making progress.
///
/// The baseline span is the requested padded-domain population. Even dispersion
/// expands it to the number of exclusion-sized cells covering the complete
/// local domain. The rule therefore scales with canvas, zoom, and authored
/// spacing rather than imposing an application-authored geometry or work
/// ceiling. Reaching it publishes the best deterministic placement attained,
/// including physically saturated or zero-weight source cases.
///
/// # Errors
///
/// Returns a stable coverage diagnostic if the finite validated bounds cannot
/// be represented as a machine-sized cell count.
fn random_settled_rejection_span(
    random: &RandomSiteCapability,
    local: Bounds,
    requested: usize,
) -> Result<usize, PatternPipelineError> {
    let RandomSiteCharacter::Even {
        minimum_center_distance,
    } = random.character
    else {
        return Ok(requested.max(1));
    };
    let columns = ((local.max.x - local.min.x) / minimum_center_distance)
        .ceil()
        .max(1.0);
    let rows = ((local.max.y - local.min.y) / minimum_center_distance)
        .ceil()
        .max(1.0);
    if columns > usize::MAX as f64 || rows > usize::MAX as f64 {
        return Err(PatternPipelineError::new(
            "coverage.random_sites.spatial_index",
            "even-dispersion convergence sweep is not representable",
        ));
    }
    let span = (columns as usize)
        .checked_mul(rows as usize)
        .ok_or(PatternPipelineError::new(
            "coverage.random_sites.spatial_index",
            "even-dispersion convergence sweep overflowed",
        ))?;
    Ok(span.max(requested).max(1))
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

/// Pushes one family value only after a fallible incremental reservation.
///
/// # Errors
///
/// Returns the caller-selected stable allocation diagnostic without mutating
/// the vector when its next element cannot be reserved.
fn try_push_family_value<T>(
    values: &mut Vec<T>,
    value: T,
    path: &'static str,
    message: &'static str,
) -> Result<(), PatternPipelineError> {
    values
        .try_reserve(1)
        .map_err(|_| PatternPipelineError::new(path, message))?;
    values.push(value);
    Ok(())
}

/// Resolves an exclusion policy against the evaluator-supplied active maximum mark support.
fn exclusion_distance(policy: &SiteExclusionPolicy, support: f64) -> f64 {
    match policy {
        SiteExclusionPolicy::None => 0.0,
        SiteExclusionPolicy::MinimumCenterDistance { minimum } => *minimum,
        SiteExclusionPolicy::VisibleMarkMargin { margin } => support * 2.0 + margin,
    }
}

/// Resolves the deterministic clustered-parent population without allocating it.
fn cluster_parent_count(character: &RandomSiteCharacter, requested: usize) -> usize {
    let RandomSiteCharacter::Clustered {
        cluster_density, ..
    } = character
    else {
        return 0;
    };
    ((requested as f64 * cluster_density).round() as usize).clamp(1, requested.max(1))
}

/// Builds clustered parents incrementally with cancellation and family progress.
///
/// # Errors
///
/// Returns cancellation or a stable allocation diagnostic before returning a
/// partial parent population.
#[allow(clippy::too_many_arguments)]
fn cluster_parents(
    character: &RandomSiteCharacter,
    bounds: Bounds,
    count: usize,
    prng: &mut StablePrng,
    is_cancelled: &dyn Fn() -> bool,
    report_progress: &(dyn Fn(usize, usize) + Sync),
    estimated_work: usize,
) -> Result<Vec<Point2>, PatternPipelineError> {
    if !matches!(character, RandomSiteCharacter::Clustered { .. }) {
        return Ok(Vec::new());
    }
    let mut parents = Vec::new();
    for index in 0..count {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        try_push_family_value(
            &mut parents,
            Point2::new(
                bounds.min.x + prng.unit() * (bounds.max.x - bounds.min.x),
                bounds.min.y + prng.unit() * (bounds.max.y - bounds.min.y),
            ),
            "coverage.random_sites.allocation",
            "clustered random sites could not reserve another parent",
        )?;
        let completed = index + 1;
        if completed == 1 || completed.is_multiple_of(64) || completed == count {
            report_progress(completed.min(estimated_work), estimated_work);
        }
    }
    Ok(parents)
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
        SiteExclusionPolicy::VisibleMarkMargin { margin } => {
            bytes.push(3);
            bytes.extend(margin.to_bits().to_le_bytes());
        }
    }
    bytes.extend(request.canvas.width.to_bits().to_le_bytes());
    bytes.extend(request.canvas.height.to_bits().to_le_bytes());
    bytes.extend(request.density.across_x.to_bits().to_le_bytes());
    bytes.extend(request.density.across_y.to_bits().to_le_bytes());
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
    realize_typed_mapped_outputs_cancellable(
        family,
        plan,
        source,
        canvas,
        mapping,
        response,
        &|| false,
    )
}

/// Realizes typed mapped marks with indexed CPU work and cooperative cancellation.
///
/// # Errors
///
/// Returns cancellation or the existing typed provenance, sampling, response, and geometry
/// diagnostics without exposing a partial output.
pub fn realize_typed_mapped_outputs_cancellable(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: MarkResponse,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<TypedRealization<MappedCircularMarkRealization>, PatternPipelineError> {
    let provenance = realization_provenance(family, plan)?;
    let compatibility = legacy_grid_sites_for_circular_marks(family)?;
    realize_mapped_circular_marks_cancellable(
        &compatibility,
        source,
        canvas,
        mapping,
        response,
        is_cancelled,
    )
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
    realize_typed_source_color_outputs_cancellable(
        family,
        plan,
        source,
        canvas,
        mapping,
        response,
        &|| false,
    )
}

/// Realizes typed sampled-color marks with indexed CPU work and cancellation.
///
/// # Errors
///
/// Returns cancellation or the existing typed provenance, sampling, response, and geometry
/// diagnostics without exposing a partial output.
pub fn realize_typed_source_color_outputs_cancellable(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: MarkResponse,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<TypedRealization<SourceColorCircularMarkRealization>, PatternPipelineError> {
    let provenance = realization_provenance(family, plan)?;
    let compatibility = legacy_grid_sites_for_circular_marks(family)?;
    realize_source_color_circular_marks_cancellable(
        &compatibility,
        source,
        canvas,
        mapping,
        response,
        is_cancelled,
    )
    .map(|mut output| {
        output.realization_fingerprint =
            orientation_identity(&output.realization_fingerprint, family, &provenance);
        TypedRealization { provenance, output }
    })
    .map_err(|error| PatternPipelineError::new(error.path(), error.message()))
}

/// Realizes one explicit mark output into a capability-addressed mapped unit.
///
/// # Errors
///
/// Returns binding, response-kind, source, or canonical-mark diagnostics without exposing a
/// partially realized output unit.
#[allow(clippy::too_many_arguments)]
pub fn realize_typed_mapped_output(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    shape_rotation_degrees: f64,
) -> Result<TypedOutputRealization<MappedCircularMarkRealization>, PatternPipelineError> {
    realize_typed_mapped_output_cancellable(
        family,
        plan,
        capability,
        setting,
        source,
        canvas,
        mapping,
        shape_rotation_degrees,
        &|| false,
    )
}

/// Realizes one explicit mapped mark output with cooperative worker cancellation.
///
/// # Errors
///
/// Returns cancellation or the existing binding, response, sampling, and geometry diagnostic
/// without exposing a partial output unit.
#[allow(clippy::too_many_arguments)]
pub fn realize_typed_mapped_output_cancellable(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    shape_rotation_degrees: f64,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<TypedOutputRealization<MappedCircularMarkRealization>, PatternPipelineError> {
    validate_output_realization_binding(plan, capability, setting)?;
    let PatternGeometryResponse::Marks(response) = &setting.response else {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.setting",
            "mapped mark realization requires a mark response",
        ));
    };
    let realization = realize_typed_mapped_outputs_cancellable(
        family,
        plan,
        source,
        canvas,
        mapping,
        MarkResponse {
            minimum_fill: response.minimum_fill,
            maximum_fill: response.maximum_fill,
            rotation_offset_degrees: shape_rotation_degrees,
        },
        is_cancelled,
    )?;
    Ok(TypedOutputRealization {
        output_layer_id: capability.layer_id,
        capability: capability.clone(),
        effective_setting: setting.clone(),
        realization,
    })
}

/// Realizes one explicit sampled-paint mark output into a capability-addressed unit.
///
/// # Errors
///
/// Returns the first explicit-binding or source/mark realization diagnostic without partial output.
#[allow(clippy::too_many_arguments)]
pub fn realize_typed_source_color_output(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    shape_rotation_degrees: f64,
) -> Result<TypedOutputRealization<SourceColorCircularMarkRealization>, PatternPipelineError> {
    realize_typed_source_color_output_cancellable(
        family,
        plan,
        capability,
        setting,
        source,
        canvas,
        mapping,
        shape_rotation_degrees,
        &|| false,
    )
}

/// Realizes one explicit sampled-color mark output with cooperative worker cancellation.
///
/// # Errors
///
/// Returns cancellation or the existing binding, response, sampling, and geometry diagnostic
/// without exposing a partial output unit.
#[allow(clippy::too_many_arguments)]
pub fn realize_typed_source_color_output_cancellable(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    shape_rotation_degrees: f64,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<TypedOutputRealization<SourceColorCircularMarkRealization>, PatternPipelineError> {
    validate_output_realization_binding(plan, capability, setting)?;
    let PatternGeometryResponse::Marks(response) = &setting.response else {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.setting",
            "sampled mark realization requires a mark response",
        ));
    };
    let realization = realize_typed_source_color_outputs_cancellable(
        family,
        plan,
        source,
        canvas,
        mapping,
        MarkResponse {
            minimum_fill: response.minimum_fill,
            maximum_fill: response.maximum_fill,
            rotation_offset_degrees: shape_rotation_degrees,
        },
        is_cancelled,
    )?;
    Ok(TypedOutputRealization {
        output_layer_id: capability.layer_id,
        capability: capability.clone(),
        effective_setting: setting.clone(),
        realization,
    })
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
    is_cancelled: &(dyn Fn() -> bool + Sync),
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
    let parallel_results = family
        .site_set()
        .sites()
        .par_iter()
        .map(|site| {
            if is_cancelled() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "realization was cancelled",
                ));
            }
            let orientation = site_orientation_degrees(
                family,
                site,
                orientation,
                request.response.rotation_offset_degrees,
            )?;
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
                        .map_err(|error| {
                            PatternPipelineError::new(error.path(), error.message())
                        })?,
                    None,
                )
            };
            let radius = if sampled_paint.is_some_and(|(_, suppressed)| suppressed) {
                0.0
            } else {
                radius_from_ink_with_diameter(
                    ink,
                    request.response,
                    site.nominal_cell_basis.diameter(),
                )
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
            Ok((mark, sampled_paint.map(|(paint, _)| paint)))
        })
        .collect::<Vec<_>>();
    let completed = parallel_results
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let mut marks = Vec::with_capacity(completed.len());
    let mut paints = request
        .sampled_paint
        .then(|| Vec::with_capacity(completed.len()));
    for (mark, paint) in completed {
        marks.push(mark);
        if let Some(paint) = paint {
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

/// Realizes one explicit canonical-mark output while retaining its output capability and setting.
///
/// # Errors
///
/// Returns explicit-binding, mark response, cancellation, or canonical geometry diagnostics
/// without exposing a partial output unit.
#[allow(clippy::too_many_arguments)]
pub fn realize_typed_canonical_mark_output_cancellable(
    document: &Document,
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    sampled_paint: bool,
    shape_rotation_degrees: f64,
    max_transformed_curve_segment_instances: usize,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<TypedOutputRealization<CanonicalMarkRealization>, PatternPipelineError> {
    validate_output_realization_binding(plan, capability, setting)?;
    let PatternGeometryResponse::Marks(response) = &setting.response else {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.setting",
            "canonical mark realization requires a mark response",
        ));
    };
    let realization = realize_typed_canonical_marks_cancellable(
        document,
        family,
        plan,
        source,
        canvas,
        CanonicalMarkRequest {
            mapping,
            sampled_paint,
            response: MarkResponse {
                minimum_fill: response.minimum_fill,
                maximum_fill: response.maximum_fill,
                rotation_offset_degrees: shape_rotation_degrees,
            },
            max_transformed_curve_segment_instances,
        },
        is_cancelled,
    )?;
    Ok(TypedOutputRealization {
        output_layer_id: capability.layer_id,
        capability: capability.clone(),
        effective_setting: setting.clone(),
        realization,
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
            let structural_input = if output.marks().is_some()
                || output.connection_paths().is_some()
                || output.maze_walls().is_some()
            {
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
    pub density: ResolvedDensityMetric2D,
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
    /// The local phase projected through the placed center and translation, normalized only for reporting.
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
    pub density: ResolvedDensityMetric2D,
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

/// Evaluates a selected generalized straight-guide product with bounded cancellation points.
///
/// Local guide coordinates map through the shared centered-origin transform and remain independent
/// of source, modulation, and output presentation.
///
/// # Errors
///
/// Returns validation, bounded coverage, numeric, candidate-limit, or cancellation errors before
/// exposing a partial structural output.
pub fn evaluate_generalized_straight_guides_cancellable(
    family: &FamilyCapability,
    request: &StraightGuideInspectRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<GeneralizedStraightGuideOutput, GridError> {
    evaluate_generalized_straight_guides_with_progress_cancellable(
        family,
        request,
        is_cancelled,
        &|_, _| {},
    )
}

/// Evaluates a generalized straight-guide product with deterministic work progress.
///
/// Progress covers pairwise intersection generation and coincident-site merge
/// work, is excluded from family identity, and never exposes partial geometry.
///
/// # Errors
///
/// Returns validation, coverage, numeric, explicit-limit, or cancellation
/// diagnostics before exposing a structural output.
pub fn evaluate_generalized_straight_guides_with_progress_cancellable(
    family: &FamilyCapability,
    request: &StraightGuideInspectRequest,
    is_cancelled: &dyn Fn() -> bool,
    report_progress: &(dyn Fn(usize, usize) + Sync),
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
    let (layout_density, aspect_scale_x, aspect_scale_y) =
        aspect_normalized_straight_layout(&request.canvas, request.density)?;
    let transform = AffineTransform2D::scale_then_rotate_about_then_translate(
        Point2::new(0.0, 0.0),
        aspect_scale_x,
        aspect_scale_y,
        request.rotation_degrees,
        Vector2::new(
            request.canvas.width * 0.5 + request.translation_x,
            request.canvas.height * 0.5 + request.translation_y,
        ),
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
        let spacing = directional_spacing(&request.canvas, &layout_density, normal)?
            * dimension.repetition.spacing_multiplier;
        let plan = DimensionPlan::new(dimension.id, normal, spacing, dimension.phase);
        let item = plan.coverage(domain, transform)?;
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
    let extension = margin / aspect_scale_x.min(aspect_scale_y);
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
            GeneralizedWorkObserver {
                is_cancelled,
                report_progress,
            },
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

/// Borrows cancellation and progress observations for one generalized-family operation.
struct GeneralizedWorkObserver<'a> {
    is_cancelled: &'a dyn Fn() -> bool,
    report_progress: &'a (dyn Fn(usize, usize) + Sync),
}

/// Produces bounded pairwise guide intersections and merges every epsilon-coincident contributor set.
///
/// # Errors
///
/// Returns stable coverage or cancellation diagnostics before returning an incomplete site list.
fn generalized_intersections(
    grouped: &[Vec<StraightGuide>],
    selected: &[usize],
    epsilon: f64,
    canvas: Bounds,
    margin: f64,
    limit: usize,
    observer: GeneralizedWorkObserver<'_>,
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
    let pair_work_total =
        selected
            .iter()
            .enumerate()
            .try_fold(0_usize, |total, (offset, &left)| {
                selected[offset + 1..]
                    .iter()
                    .try_fold(total, |total, &right| {
                        grouped[left]
                            .len()
                            .checked_mul(grouped[right].len())
                            .and_then(|pairs| total.checked_add(pairs))
                            .ok_or(GridError::new(
                                "coverage.intersections.pairwise_limit",
                                "pairwise intersection progress count overflowed",
                            ))
                    })
            })?;
    let estimated_work = pair_work_total.saturating_mul(2).max(1);
    let mut inspected_pairs = 0_usize;
    let mut raw: Vec<(Point2, Vec<GuideInstanceId>)> = Vec::new();
    for (left_offset, &left) in selected.iter().enumerate() {
        for &right in &selected[left_offset + 1..] {
            if (observer.is_cancelled)() {
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
                if (observer.is_cancelled)() {
                    return Err(GridError::new(
                        "evaluation.cancelled",
                        "evaluation was cancelled",
                    ));
                }
                for guide_b in &grouped[right] {
                    inspected_pairs = inspected_pairs.checked_add(1).ok_or(GridError::new(
                        "coverage.intersections.pairwise_limit",
                        "pairwise intersection progress count overflowed",
                    ))?;
                    if inspected_pairs == 1 || inspected_pairs.is_multiple_of(64) {
                        (observer.report_progress)(
                            inspected_pairs.min(estimated_work),
                            estimated_work,
                        );
                    }
                    if let Some(point) = line_intersection(guide_a, guide_b)
                        .map(|point| snap_intersection_to_canvas_boundary(point, canvas))
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
    let mut merge_buckets: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    let mut merge_comparisons = 0_usize;
    let raw_count = raw.len();
    for (raw_index, (point, contributors)) in raw.into_iter().enumerate() {
        if (observer.is_cancelled)() {
            return Err(GridError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        let completed = pair_work_total.saturating_add(raw_index + 1);
        if raw_index == 0 || completed.is_multiple_of(64) || raw_index + 1 == raw_count {
            (observer.report_progress)(completed.min(estimated_work), estimated_work);
        }
        let mut existing_index = None;
        let bucket = if epsilon == 0.0 {
            None
        } else {
            Some((point.y / epsilon).floor() as i64)
        };
        let bucket_start = bucket.map_or(0, |key| key.saturating_sub(1));
        let bucket_end = bucket.map_or(0, |key| key.saturating_add(1));
        for key in bucket_start..=bucket_end {
            let candidate_indices = if epsilon == 0.0 {
                sites.last().map(|_| vec![sites.len() - 1])
            } else {
                merge_buckets.get(&key).cloned()
            };
            for index in candidate_indices.into_iter().flatten().rev() {
                if (observer.is_cancelled)() {
                    return Err(GridError::new(
                        "evaluation.cancelled",
                        "evaluation was cancelled",
                    ));
                }
                if point.x - sites[index].position.x > epsilon {
                    break;
                }
                merge_comparisons = merge_comparisons.checked_add(1).ok_or(GridError::new(
                    "coverage.intersections.merge_limit",
                    "coincident merge work overflowed",
                ))?;
                if merge_comparisons > limit {
                    return Err(GridError::new(
                        "coverage.intersections.merge_limit",
                        "coincident merge work exceeds the configured limit",
                    ));
                }
                if (point.y - sites[index].position.y).abs() <= epsilon
                    && distance(sites[index].position, point) <= epsilon
                    && existing_index.is_none_or(|existing| index > existing)
                {
                    existing_index = Some(index);
                }
            }
        }
        if let Some(index) = existing_index {
            let existing = &mut sites[index];
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
        let site_index = sites.len();
        sites.push(GeneralizedSite {
            sequence: sites.len(),
            position: point,
            scope,
            provenance: GeneralizedSiteProvenance::Intersection { contributors },
        });
        if let Some(bucket) = bucket {
            merge_buckets.entry(bucket).or_default().push(site_index);
        }
    }
    (observer.report_progress)(estimated_work, estimated_work);
    Ok(sites)
}

/// Canonicalizes only numerical intersection noise that lies immediately beside a canvas edge.
///
/// Generalized-guide phases stay authored in their existing coordinate frame. This helper changes
/// neither that phase nor a genuine guard-row coordinate: it clamps a finite coordinate only when
/// it is within a scale-aware floating-point tolerance of the corresponding canvas minimum or
/// maximum, before coverage, scope, merging, provenance, and identity are published.
fn snap_intersection_to_canvas_boundary(point: Point2, canvas: Bounds) -> Point2 {
    let coordinate_scale = canvas
        .min
        .x
        .abs()
        .max(canvas.min.y.abs())
        .max(canvas.max.x.abs())
        .max(canvas.max.y.abs())
        .max(1.0);
    let tolerance = 64.0 * f64::EPSILON * coordinate_scale;
    Point2::new(
        snap_coordinate_to_canvas_boundary(point.x, canvas.min.x, canvas.max.x, tolerance),
        snap_coordinate_to_canvas_boundary(point.y, canvas.min.y, canvas.max.y, tolerance),
    )
}

/// Clamps one finite coordinate only when roundoff places it beside an exact canvas boundary.
fn snap_coordinate_to_canvas_boundary(
    value: f64,
    minimum: f64,
    maximum: f64,
    tolerance: f64,
) -> f64 {
    if (value - minimum).abs() <= tolerance {
        minimum
    } else if (value - maximum).abs() <= tolerance {
        maximum
    } else {
        value
    }
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
        density: request.density,
        rotation_degrees: request.rotation_degrees,
        translation_x: request.translation_x,
        translation_y: request.translation_y,
        guard_steps: request.guard_steps,
        support_radius: request.support_radius,
        max_family_candidates: request.max_family_candidates,
    })
}

/// Hashes generalized straight-guide intent under the centered-local placement contract.
fn generalized_fingerprint(
    family: &FamilyCapability,
    request: &StraightGuideInspectRequest,
) -> String {
    let mut bytes = b"toniator-stage-21a-straight-guide-family-v3-aspect-affine-layout".to_vec();
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
    pub bias: f64,
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
pub const CANONICAL_STROKE_OUTLINE_CONTRACT_ID: &str = "toniator-stage-21a-filled-outline-v4";
/// Bounds adaptive samples per evaluation request before profile allocation.
pub const MAX_STROKE_PROFILE_SAMPLES: usize = 262_144;
/// Bounds derived filled-outline segments per evaluation request.
pub const MAX_STROKE_OUTLINE_SEGMENTS: usize = 524_288;
/// Caps adaptive centerline/width subdivision deterministically.
pub const MAX_STROKE_SUBDIVISION_DEPTH: u8 = 48;
const STROKE_CENTERLINE_TOLERANCE: f64 = 1.0 / 64.0;
/// Bounds the permitted normalized source-response interpolation error for one profile interval.
///
/// This is an internal canonical-outline sampling tolerance, not artist-facing motif sizing or
/// a renderer setting. It prevents a sharp source transition from becoming a long, thin tapered
/// outline wedge between two otherwise widely spaced cadence samples.
const STROKE_RESPONSE_TOLERANCE: f64 = 1.0 / 64.0;

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
        let profile_sample_interval = nominal_basis.max(pixel_footprint);
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
                profile_sample_interval,
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
            response.bias,
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
        .map_err(|error| {
            if error.path() == "curve.outline.segment_limit" {
                PatternPipelineError::new(
                    "connection.stroke.outline_limit",
                    "connection stroke outline exceeds the segment limit",
                )
            } else {
                PatternPipelineError::new(error.path(), error.message())
            }
        })?;
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
    bytes.extend(response.bias.to_bits().to_le_bytes());
    for stroke in &strokes {
        match &stroke.source_id {
            toniator_geometry::CanonicalStrokeSourceId::Structural(id) => {
                append_structural_path_instance_identity(&mut bytes, *id);
            }
            toniator_geometry::CanonicalStrokeSourceId::Connection(id) => {
                bytes.push(0x43);
                bytes.extend(id.output_layer_id.0.to_le_bytes());
                bytes.extend(id.component_minimum.mechanism_id.0.to_le_bytes());
                bytes.extend((id.component_minimum.ordinal as u64).to_le_bytes());
                bytes.extend(id.component_ordinal.to_le_bytes());
                bytes.extend(id.first_endpoint.mechanism_id.0.to_le_bytes());
                bytes.extend((id.first_endpoint.ordinal as u64).to_le_bytes());
                bytes.extend(id.last_endpoint.mechanism_id.0.to_le_bytes());
                bytes.extend((id.last_endpoint.ordinal as u64).to_le_bytes());
                bytes.extend(id.ordinal.to_le_bytes());
            }
            toniator_geometry::CanonicalStrokeSourceId::Maze(id) => {
                bytes.push(0x4d);
                bytes.extend(id.output_layer_id.0.to_le_bytes());
                bytes.extend(id.wall.first.0.to_le_bytes());
                bytes.extend(id.wall.second.0.to_le_bytes());
            }
        }
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

/// Realizes one explicit guide-path output into a capability-addressed canonical-stroke unit.
///
/// # Errors
///
/// Returns binding, response-kind, cancellation, sampling, or outline diagnostics without partial output.
#[allow(clippy::too_many_arguments)]
pub fn realize_typed_canonical_stroke_output_cancellable(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedOutputRealization<CanonicalStrokeRealization>, PatternPipelineError> {
    validate_output_realization_binding(plan, capability, setting)?;
    let PatternGeometryResponse::Connected(response) = &setting.response else {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.setting",
            "guide-path realization requires a connected response",
        ));
    };
    let realization = realize_typed_canonical_strokes_cancellable(
        family,
        plan,
        source,
        canvas,
        mapping,
        StrokeResponse {
            minimum_thickness: response.minimum_thickness,
            maximum_thickness: response.maximum_thickness,
            bias: response.bias,
        },
        1.0,
        max_profile_samples,
        max_outline_segments,
        is_cancelled,
    )?;
    Ok(TypedOutputRealization {
        output_layer_id: capability.layer_id,
        capability: capability.clone(),
        effective_setting: setting.clone(),
        realization,
    })
}

/// Realizes selected connection paths as canonical round strokes without exposing adjacency to renderers.
///
/// # Errors
///
/// Returns stable sampling, geometry, configured-limit, or cancellation diagnostics before any
/// connection stroke collection is published.
#[allow(clippy::too_many_arguments)] // The existing canonical stroke boundary keeps source, canvas, response, and limits explicit.
pub fn realize_connection_canonical_strokes_cancellable(
    paths: &ConnectionPathSet,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    style: toniator_domain::PathStrokeStyle,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CanonicalStrokeRealization, PatternPipelineError> {
    realize_connection_canonical_strokes_from_paths(
        paths.fingerprint().to_owned(),
        paths.paths.len(),
        paths.paths.iter().map(|connection| {
            (
                connection.id.clone(),
                connection.path.clone(),
                connection.nominal_basis,
            )
        }),
        source,
        canvas,
        mapping,
        response,
        style,
        max_profile_samples,
        max_outline_segments,
        is_cancelled,
    )
}

/// Consumes selected connection paths while realizing their canonical round strokes.
///
/// This output-only boundary releases graph-selection diagnostics and moves each centerline into
/// its stroke. It is intended for orchestration that has already derived any required usage facts;
/// the borrowed variant remains available when callers must retain the complete path result.
///
/// # Errors
///
/// Returns stable sampling, geometry, allocation, or cancellation diagnostics without publishing
/// a partial realization.
#[allow(clippy::too_many_arguments)]
pub fn realize_owned_connection_canonical_strokes_cancellable(
    mut paths: ConnectionPathSet,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    style: toniator_domain::PathStrokeStyle,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CanonicalStrokeRealization, PatternPipelineError> {
    let family_fingerprint = paths.fingerprint().to_owned();
    let connections = std::mem::take(&mut paths.paths);
    let path_count = connections.len();
    drop(paths);
    realize_connection_canonical_strokes_from_paths(
        family_fingerprint,
        path_count,
        connections
            .into_iter()
            .map(|connection| (connection.id, connection.path, connection.nominal_basis)),
        source,
        canvas,
        mapping,
        response,
        style,
        max_profile_samples,
        max_outline_segments,
        is_cancelled,
    )
}

/// Realizes owned connection centerlines and streams their fingerprint without a byte mirror.
///
/// # Errors
///
/// Returns stable sampling, geometry, allocation, configured-limit, or cancellation diagnostics.
#[allow(clippy::too_many_arguments)]
fn realize_connection_canonical_strokes_from_paths<I>(
    family_fingerprint: String,
    path_count: usize,
    connections: I,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    style: toniator_domain::PathStrokeStyle,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CanonicalStrokeRealization, PatternPipelineError>
where
    I: IntoIterator<Item = (ConnectionPathId, CurvePath, f64)>,
{
    let mut strokes = Vec::new();
    reserve_stage20m(
        &mut strokes,
        path_count,
        "connection.allocation",
        "connection canonical-stroke allocation failed",
    )?;
    let mut identity = Fnv1a64State::new();
    identity.write(family_fingerprint.bytes());
    identity.write(CANONICAL_STROKE_OUTLINE_CONTRACT_ID.bytes());
    identity.write(response.minimum_thickness.to_bits().to_le_bytes());
    identity.write(response.maximum_thickness.to_bits().to_le_bytes());
    identity.write(response.bias.to_bits().to_le_bytes());
    let mut profile_samples = 0_usize;
    let mut outline_segments = 0_usize;
    for (connection_id, path, nominal_basis) in connections {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        let mut profile = Vec::new();
        let profile_capacity =
            path.segments()
                .len()
                .checked_mul(2)
                .ok_or(PatternPipelineError::new(
                    "connection.allocation",
                    "connection stroke-profile allocation size overflows",
                ))?;
        reserve_stage20m(
            &mut profile,
            profile_capacity,
            "connection.allocation",
            "connection stroke-profile allocation failed",
        )?;
        for (segment_index, segment) in path.segments().iter().enumerate() {
            for parameter in [0.0, 1.0] {
                if is_cancelled() {
                    return Err(PatternPipelineError::new(
                        "evaluation.cancelled",
                        "evaluation was cancelled",
                    ));
                }
                let sample = stroke_sample(
                    segment,
                    segment_index,
                    parameter,
                    source,
                    canvas,
                    mapping,
                    response,
                    nominal_basis,
                )?;
                // Preserve both segment-local locations at a shared graph vertex. The outline
                // authority consumes that same-center tangent discontinuity to construct its
                // round join; only duplicate locations, not duplicate centers, are redundant.
                if profile.last().is_none_or(|previous: &StrokeProfileSample| {
                    previous.location != sample.location
                }) {
                    profile_samples =
                        profile_samples
                            .checked_add(1)
                            .ok_or(PatternPipelineError::new(
                                "connection.stroke.profile_limit",
                                "connection stroke profile exceeds the sample limit",
                            ))?;
                    if profile_samples > max_profile_samples {
                        return Err(PatternPipelineError::new(
                            "connection.stroke.profile_limit",
                            "connection stroke profile exceeds the sample limit",
                        ));
                    }
                    profile.push(sample);
                }
            }
        }
        let mut outline_input = Vec::new();
        reserve_stage20m(
            &mut outline_input,
            profile.len(),
            "connection.allocation",
            "connection outline-input allocation failed",
        )?;
        for sample in &profile {
            if is_cancelled() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "evaluation was cancelled",
                ));
            }
            outline_input.push(VariableWidthPathSample {
                location: sample.location,
                width: sample.width,
            });
        }
        let outline = build_variable_width_outline_cancellable(
            &path,
            &outline_input,
            style,
            response.bias,
            1.0 / 8.0,
            VariableWidthOutlineLimits::new(
                max_outline_segments.saturating_sub(outline_segments).max(1),
            )
            .map_err(|_| {
                PatternPipelineError::new(
                    "connection.stroke.outline_limit",
                    "configured connection stroke outline limit must be nonzero",
                )
            })?,
            is_cancelled,
        )
        .map_err(|error| {
            if error.path() == "curve.outline.segment_limit" {
                PatternPipelineError::new(
                    "connection.stroke.outline_limit",
                    "connection stroke outline exceeds the segment limit",
                )
            } else {
                PatternPipelineError::new(error.path(), error.message())
            }
        })?;
        outline_segments = outline_segments
            .checked_add(
                outline
                    .contours
                    .iter()
                    .map(|contour| contour.segments.len())
                    .sum::<usize>(),
            )
            .ok_or(PatternPipelineError::new(
                "connection.stroke.outline_limit",
                "connection stroke outline exceeds the segment limit",
            ))?;
        if outline_segments > max_outline_segments {
            return Err(PatternPipelineError::new(
                "connection.stroke.outline_limit",
                "connection stroke outline exceeds the segment limit",
            ));
        }
        identity.write(connection_id.output_layer_id.0.to_le_bytes());
        identity.write(connection_id.component_minimum.mechanism_id.0.to_le_bytes());
        identity.write((connection_id.component_minimum.ordinal as u64).to_le_bytes());
        identity.write(connection_id.component_ordinal.to_le_bytes());
        identity.write(connection_id.first_endpoint.mechanism_id.0.to_le_bytes());
        identity.write((connection_id.first_endpoint.ordinal as u64).to_le_bytes());
        identity.write(connection_id.last_endpoint.mechanism_id.0.to_le_bytes());
        identity.write((connection_id.last_endpoint.ordinal as u64).to_le_bytes());
        identity.write(connection_id.ordinal.to_le_bytes());
        for sample in &profile {
            identity.write(sample.center.x.to_bits().to_le_bytes());
            identity.write(sample.center.y.to_bits().to_le_bytes());
            identity.write(sample.normalized_thickness.to_bits().to_le_bytes());
        }
        strokes.push(
            CanonicalStroke::new_connection(
                connection_id,
                path,
                nominal_basis,
                style,
                profile,
                outline,
            )
            .map_err(|_| {
                PatternPipelineError::new(
                    "connection.stroke.geometry",
                    "connection canonical stroke geometry must remain finite",
                )
            })?,
        );
    }
    Ok(CanonicalStrokeRealization {
        family_fingerprint,
        realization_fingerprint: identity.finish(),
        source_identity: source.identity().clone(),
        response,
        strokes,
    })
}

/// Realizes retained conventional maze walls as canonical round strokes without exposing faces or passages.
///
/// # Errors
///
/// Returns stable sampling, outline-limit, geometry, or cancellation errors before publishing any
/// partial wall-stroke collection.
#[allow(clippy::too_many_arguments)] // The canonical stroke boundary keeps all resource policy explicit.
pub fn realize_maze_canonical_strokes_cancellable(
    maze: &MazeProgramResult,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    style: toniator_domain::PathStrokeStyle,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CanonicalStrokeRealization, PatternPipelineError> {
    realize_maze_canonical_strokes_from_paths(
        maze.fingerprint().to_owned(),
        maze.wall_paths.len(),
        maze.wall_paths
            .iter()
            .map(|wall| (wall.id, wall.path.clone(), wall.nominal_basis)),
        source,
        canvas,
        mapping,
        response,
        style,
        max_profile_samples,
        max_outline_segments,
        is_cancelled,
    )
}

/// Consumes a completed maze while realizing its retained wall paths as canonical strokes.
///
/// The caller must derive usage or diagnostics before this boundary. Consuming the maze releases
/// its arrangement, dual graph, and solution storage before variable-width outlines are built.
///
/// # Errors
///
/// Returns stable sampling, outline, geometry, allocation, or cancellation diagnostics without
/// publishing a partial realization.
#[allow(clippy::too_many_arguments)]
pub fn realize_owned_maze_canonical_strokes_cancellable(
    mut maze: MazeProgramResult,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    style: toniator_domain::PathStrokeStyle,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CanonicalStrokeRealization, PatternPipelineError> {
    let family_fingerprint = maze.fingerprint().to_owned();
    let walls = std::mem::take(&mut maze.wall_paths);
    let wall_count = walls.len();
    drop(maze);
    realize_maze_canonical_strokes_from_paths(
        family_fingerprint,
        wall_count,
        walls
            .into_iter()
            .map(|wall| (wall.id, wall.path, wall.nominal_basis)),
        source,
        canvas,
        mapping,
        response,
        style,
        max_profile_samples,
        max_outline_segments,
        is_cancelled,
    )
}

/// Realizes owned maze wall centerlines and streams their fingerprint without a byte mirror.
///
/// # Errors
///
/// Returns stable sampling, outline, geometry, allocation, configured-limit, or cancellation
/// diagnostics.
#[allow(clippy::too_many_arguments)]
fn realize_maze_canonical_strokes_from_paths<I>(
    family_fingerprint: String,
    wall_count: usize,
    walls: I,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    style: toniator_domain::PathStrokeStyle,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CanonicalStrokeRealization, PatternPipelineError>
where
    I: IntoIterator<Item = (MazeWallPathId, CurvePath, f64)>,
{
    let mut strokes = Vec::new();
    reserve_stage20m(
        &mut strokes,
        wall_count,
        "maze.allocation",
        "maze canonical-stroke allocation failed",
    )?;
    let mut identity = Fnv1a64State::new();
    identity.write(family_fingerprint.bytes());
    identity.write(CANONICAL_STROKE_OUTLINE_CONTRACT_ID.bytes());
    identity.write(response.minimum_thickness.to_bits().to_le_bytes());
    identity.write(response.maximum_thickness.to_bits().to_le_bytes());
    identity.write(response.bias.to_bits().to_le_bytes());
    let mut profile_samples = 0_usize;
    let mut outline_segments = 0_usize;
    for (wall_id, path, nominal_basis) in walls {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        let mut profile = Vec::new();
        let profile_capacity =
            path.segments()
                .len()
                .checked_mul(2)
                .ok_or(PatternPipelineError::new(
                    "maze.allocation",
                    "maze stroke-profile allocation size overflows",
                ))?;
        reserve_stage20m(
            &mut profile,
            profile_capacity,
            "maze.allocation",
            "maze stroke-profile allocation failed",
        )?;
        for (segment_index, segment) in path.segments().iter().enumerate() {
            for parameter in [0.0, 1.0] {
                if is_cancelled() {
                    return Err(PatternPipelineError::new(
                        "evaluation.cancelled",
                        "evaluation was cancelled",
                    ));
                }
                let sample = stroke_sample(
                    segment,
                    segment_index,
                    parameter,
                    source,
                    canvas,
                    mapping,
                    response,
                    nominal_basis,
                )?;
                if profile.last().is_none_or(|previous: &StrokeProfileSample| {
                    previous.location != sample.location
                }) {
                    profile_samples =
                        profile_samples
                            .checked_add(1)
                            .ok_or(PatternPipelineError::new(
                                "maze.stroke.profile_limit",
                                "maze stroke profile exceeds the sample limit",
                            ))?;
                    if profile_samples > max_profile_samples {
                        return Err(PatternPipelineError::new(
                            "maze.stroke.profile_limit",
                            "maze stroke profile exceeds the sample limit",
                        ));
                    }
                    profile.push(sample);
                }
            }
        }
        let mut outline_input = Vec::new();
        reserve_stage20m(
            &mut outline_input,
            profile.len(),
            "maze.allocation",
            "maze outline-input allocation failed",
        )?;
        for sample in &profile {
            if is_cancelled() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "evaluation was cancelled",
                ));
            }
            outline_input.push(VariableWidthPathSample {
                location: sample.location,
                width: sample.width,
            });
        }
        let outline = build_variable_width_outline_cancellable(
            &path,
            &outline_input,
            style,
            response.bias,
            1.0 / 8.0,
            VariableWidthOutlineLimits::new(
                max_outline_segments.saturating_sub(outline_segments).max(1),
            )
            .map_err(|_| {
                PatternPipelineError::new(
                    "maze.stroke.outline_limit",
                    "configured maze stroke outline limit must be nonzero",
                )
            })?,
            is_cancelled,
        )
        .map_err(|error| {
            if error.path() == "curve.outline.segment_limit" {
                PatternPipelineError::new(
                    "maze.stroke.outline_limit",
                    "maze stroke outline exceeds the segment limit",
                )
            } else {
                PatternPipelineError::new(error.path(), error.message())
            }
        })?;
        outline_segments = outline_segments
            .checked_add(
                outline
                    .contours
                    .iter()
                    .map(|contour| contour.segments.len())
                    .sum::<usize>(),
            )
            .ok_or(PatternPipelineError::new(
                "maze.stroke.outline_limit",
                "maze stroke outline exceeds the segment limit",
            ))?;
        if outline_segments > max_outline_segments {
            return Err(PatternPipelineError::new(
                "maze.stroke.outline_limit",
                "maze stroke outline exceeds the segment limit",
            ));
        }
        identity.write(wall_id.output_layer_id.0.to_le_bytes());
        identity.write(wall_id.wall.first.0.to_le_bytes());
        identity.write(wall_id.wall.second.0.to_le_bytes());
        for sample in &profile {
            identity.write(sample.center.x.to_bits().to_le_bytes());
            identity.write(sample.center.y.to_bits().to_le_bytes());
            identity.write(sample.normalized_thickness.to_bits().to_le_bytes());
        }
        strokes.push(
            CanonicalStroke::new_maze(wall_id, path, nominal_basis, style, profile, outline)
                .map_err(|_| {
                    PatternPipelineError::new(
                        "maze.stroke.geometry",
                        "maze canonical stroke geometry must remain finite",
                    )
                })?,
        );
    }
    Ok(CanonicalStrokeRealization {
        family_fingerprint,
        realization_fingerprint: identity.finish(),
        source_identity: source.identity().clone(),
        response,
        strokes,
    })
}

/// Binds canonical maze-wall strokes to the validated typed family and output plan before rendering.
///
/// # Errors
///
/// Returns a plan-provenance, stroke, limit, or cancellation error without publishing partial output.
#[allow(clippy::too_many_arguments)]
pub fn realize_typed_maze_canonical_strokes_cancellable(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    maze: &MazeProgramResult,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    style: toniator_domain::PathStrokeStyle,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedRealization<CanonicalStrokeRealization>, PatternPipelineError> {
    Ok(TypedRealization {
        provenance: realization_provenance(family, plan)?,
        output: realize_maze_canonical_strokes_cancellable(
            maze,
            source,
            canvas,
            mapping,
            response,
            style,
            max_profile_samples,
            max_outline_segments,
            is_cancelled,
        )?,
    })
}

/// Realizes one explicit maze-wall output while retaining its capability and effective setting.
///
/// # Errors
///
/// Returns explicit-binding, connected-response, cancellation, sampling, or outline diagnostics
/// without exposing a partial output unit.
#[allow(clippy::too_many_arguments)]
pub fn realize_typed_maze_canonical_stroke_output_cancellable(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    maze: &MazeProgramResult,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedOutputRealization<CanonicalStrokeRealization>, PatternPipelineError> {
    validate_output_realization_binding(plan, capability, setting)?;
    let PatternGeometryResponse::Connected(response) = &setting.response else {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.setting",
            "maze realization requires a connected response",
        ));
    };
    let Some((_site_mechanism, _program, style)) = capability.maze_walls() else {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.capability",
            "maze realization requires a maze-wall capability",
        ));
    };
    let realization = realize_typed_maze_canonical_strokes_cancellable(
        family,
        plan,
        maze,
        source,
        canvas,
        mapping,
        StrokeResponse {
            minimum_thickness: response.minimum_thickness,
            maximum_thickness: response.maximum_thickness,
            bias: response.bias,
        },
        style,
        max_profile_samples,
        max_outline_segments,
        is_cancelled,
    )?;
    Ok(TypedOutputRealization {
        output_layer_id: capability.layer_id,
        capability: capability.clone(),
        effective_setting: setting.clone(),
        realization,
    })
}

/// Binds connection strokes to the validated typed family/plan provenance before renderer consumption.
///
/// # Errors
///
/// Returns a stable plan-provenance or canonical-connection diagnostic without publishing partial strokes.
#[allow(clippy::too_many_arguments)]
pub fn realize_typed_connection_canonical_strokes_cancellable(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    paths: &ConnectionPathSet,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    style: toniator_domain::PathStrokeStyle,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedRealization<CanonicalStrokeRealization>, PatternPipelineError> {
    Ok(TypedRealization {
        provenance: realization_provenance(family, plan)?,
        output: realize_connection_canonical_strokes_cancellable(
            paths,
            source,
            canvas,
            mapping,
            response,
            style,
            max_profile_samples,
            max_outline_segments,
            is_cancelled,
        )?,
    })
}

/// Realizes one explicit connection-path output while retaining its capability and effective setting.
///
/// # Errors
///
/// Returns binding, response, cancellation, source, or canonical-outline diagnostics without partial output.
#[allow(clippy::too_many_arguments)]
pub fn realize_typed_connection_canonical_stroke_output_cancellable(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    capability: &OutputCapability,
    setting: &EffectivePatternOutputSettings,
    paths: &ConnectionPathSet,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<TypedOutputRealization<CanonicalStrokeRealization>, PatternPipelineError> {
    validate_output_realization_binding(plan, capability, setting)?;
    let PatternGeometryResponse::Connected(response) = &setting.response else {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.setting",
            "connection realization requires a connected response",
        ));
    };
    let Some((_site_mechanism, _program, style)) = capability.connection_paths() else {
        return Err(PatternPipelineError::new(
            "pattern.output_layers.capability",
            "connection realization requires a connection-path capability",
        ));
    };
    let realization = realize_typed_connection_canonical_strokes_cancellable(
        family,
        plan,
        paths,
        source,
        canvas,
        mapping,
        StrokeResponse {
            minimum_thickness: response.minimum_thickness,
            maximum_thickness: response.maximum_thickness,
            bias: response.bias,
        },
        style,
        max_profile_samples,
        max_outline_segments,
        is_cancelled,
    )?;
    Ok(TypedOutputRealization {
        output_layer_id: capability.layer_id,
        capability: capability.clone(),
        effective_setting: setting.clone(),
        realization,
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

/// Appends the right endpoint once centerline shape and pattern-scale response spacing are resolved.
///
/// Source response is sampled at nominal pattern-spacing or source-pixel-footprint resolution
/// unless its midpoint departs materially from linear interpolation. That bounded extra
/// refinement preserves an abrupt source-driven zero-width break instead of manufacturing a
/// long thin taper; curved centerlines retain their independent geometric flatness refinement.
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
    profile_sample_interval: f64,
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
    let response_error = (middle.normalized_thickness
        - (left.normalized_thickness + right.normalized_thickness) * 0.5)
        .abs();
    let interval = ((right.center.x - left.center.x).powi(2)
        + (right.center.y - left.center.y).powi(2))
    .sqrt();
    let refine = centerline_error > STROKE_CENTERLINE_TOLERANCE
        || response_error > STROKE_RESPONSE_TOLERANCE
        || interval > profile_sample_interval;
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
            profile_sample_interval,
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
            profile_sample_interval,
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
    realize_circular_marks_cancellable(
        family,
        source,
        canvas,
        placement,
        component,
        response,
        &|| false,
    )
}

/// Realizes legacy circular marks through indexed CPU work with cooperative cancellation.
///
/// Stable family order is restored before fingerprint construction, and no partial mark vector is
/// returned when any worker observes cancellation or a realization failure.
///
/// # Errors
///
/// Returns cancellation or the same stable sampling, response, support, and geometry failures as
/// [`realize_circular_marks`].
pub fn realize_circular_marks_cancellable(
    family: &GridFamilyOutput,
    source: &SourceField,
    canvas: &CanvasSpec,
    placement: SourcePlacement,
    component: SourceComponent,
    response: MarkResponse,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<CircularMarkRealization, RealizationError> {
    validate_response(response, family.support_radius)?;
    if !family.has_only_finite_geometry() {
        return Err(RealizationError::new(
            "realization.family",
            "family geometry must be finite",
        ));
    }
    let results = family
        .sites
        .par_iter()
        .map(|site| {
            if is_cancelled() {
                return Err(RealizationError::new(
                    "evaluation.cancelled",
                    "realization was cancelled",
                ));
            }
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
            Ok(mark)
        })
        .collect::<Vec<_>>();
    let marks = results.into_iter().collect::<Result<Vec<_>, _>>()?;
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
    realize_mapped_circular_marks_cancellable(family, source, canvas, mapping, response, &|| false)
}

/// Realizes mapped circles in stable indexed parallel order with cooperative cancellation.
///
/// # Errors
///
/// Returns cancellation or the ordinary mapped sampling, response, support, and geometry failure
/// without publishing a partial realization.
pub fn realize_mapped_circular_marks_cancellable(
    family: &GridFamilyOutput,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: MarkResponse,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<MappedCircularMarkRealization, RealizationError> {
    validate_response(response, family.support_radius)?;
    if !family.has_only_finite_geometry() {
        return Err(RealizationError::new(
            "realization.family",
            "family geometry must be finite",
        ));
    }
    let results = family
        .sites
        .par_iter()
        .map(|site| {
            if is_cancelled() {
                return Err(RealizationError::new(
                    "evaluation.cancelled",
                    "realization was cancelled",
                ));
            }
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
            Ok(mark)
        })
        .collect::<Vec<_>>();
    let marks = results.into_iter().collect::<Result<Vec<_>, _>>()?;
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
    realize_source_color_circular_marks_cancellable(
        family,
        source,
        canvas,
        mapping,
        response,
        &|| false,
    )
}

/// Realizes source-colored circles in stable indexed parallel order with cancellation.
///
/// Exact-zero-alpha sites retain their existing omission semantics, and the sequential ordered
/// collection keeps paint/mark correspondence and fingerprints scheduling-independent.
///
/// # Errors
///
/// Returns cancellation or the ordinary sampled-color, response, support, and geometry failure
/// without publishing a partial realization.
pub fn realize_source_color_circular_marks_cancellable(
    family: &GridFamilyOutput,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: MarkResponse,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<SourceColorCircularMarkRealization, RealizationError> {
    validate_response(response, family.support_radius)?;
    if !family.has_only_finite_geometry() {
        return Err(RealizationError::new(
            "realization.family",
            "family geometry must be finite",
        ));
    }
    let results = family
        .sites
        .par_iter()
        .map(|site| {
            if is_cancelled() {
                return Err(RealizationError::new(
                    "evaluation.cancelled",
                    "realization was cancelled",
                ));
            }
            let sample = source.sample_source_color(site.position, canvas, mapping)?;
            let Some(paint) = sample.paint else {
                return Ok(None);
            };
            let radius = radius_from_ink_with_diameter(
                sample.response,
                response,
                site.nominal_cell_diameter,
            )?;
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
            Ok(Some(SourceColorCircleMark { mark, paint }))
        })
        .collect::<Vec<_>>();
    let marks: Vec<SourceColorCircleMark> = results
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
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

/// Computes the legacy circular-realization identity from every decoder-owned source discriminator.
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
    let format = source_format_identity_code(source.identity().format);
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

/// Builds the stable prefix shared by mapped-mark realization identities.
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
    bytes.push(source_format_identity_code(source.identity().format));
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
    bytes.push(source_format_identity_code(identity.format));
    append_identity_text(bytes, identity.content_hash.as_str());
    append_identity_text(bytes, identity.decoded_pixel_hash.as_str());
    bytes.extend(identity.width.to_le_bytes());
    bytes.extend(identity.height.to_le_bytes());
}

/// Assigns every persisted decoder format a stable distinct byte for derived identity hashing.
///
/// The code is not a file-format contract; the decoder contract ID and decoded pixel hash remain
/// the source authority. Adding a format must assign a new byte rather than aliasing an existing
/// format and accidentally reusing a realization cache key.
const fn source_format_identity_code(format: toniator_sampling::SourceFormat) -> u8 {
    match format {
        toniator_sampling::SourceFormat::Png => 1,
        toniator_sampling::SourceFormat::Svg => 2,
        toniator_sampling::SourceFormat::Jpeg => 3,
        toniator_sampling::SourceFormat::Webp => 4,
        toniator_sampling::SourceFormat::Bmp => 5,
        toniator_sampling::SourceFormat::Tiff => 6,
        toniator_sampling::SourceFormat::OpenExr => 7,
        toniator_sampling::SourceFormat::Avif => 8,
    }
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
        OutputCapabilityPayload::CurveMotifPaths {
            site_mechanism_id,
            structure_id,
            style,
            mirror_alternate_rows,
            alternate_row_phase,
        } => {
            bytes.push(7);
            bytes.extend(site_mechanism_id.0.to_le_bytes());
            bytes.extend(structure_id.0.to_le_bytes());
            bytes.push(u8::from(*mirror_alternate_rows));
            bytes.extend(alternate_row_phase.unwrap_or(-1.0).to_bits().to_le_bytes());
            bytes.push(match style.join {
                toniator_domain::StrokeJoin::Round => 1,
            });
            bytes.push(match style.cap {
                toniator_domain::StrokeCap::Round => 1,
            });
        }
        OutputCapabilityPayload::Regions { source } => match source {
            RegionSourceIntent::VoronoiSites { site_mechanism_id } => {
                bytes.push(5);
                bytes.extend(site_mechanism_id.0.to_le_bytes());
                append_identity_text(bytes, toniator_geometry::VORONOI_REGION_CONTRACT_ID);
            }
            RegionSourceIntent::GuideFaces {
                guide_mechanism_id,
                dimensions,
            } => {
                bytes.push(6);
                bytes.extend(guide_mechanism_id.0.to_le_bytes());
                for dimension in dimensions {
                    bytes.extend(dimension.0.to_le_bytes());
                }
                append_identity_text(bytes, toniator_geometry::GUIDE_FACE_CONTRACT_ID);
            }
        },
        OutputCapabilityPayload::MazeWalls {
            site_mechanism_id,
            program,
            style,
        } => {
            bytes.push(4);
            bytes.extend(site_mechanism_id.0.to_le_bytes());
            bytes.extend(program.seed.to_le_bytes());
            bytes.push(match program.algorithm {
                toniator_domain::GridMazeAlgorithm::RecursiveBacktracker => 1,
            });
            bytes.push(match style.join {
                toniator_domain::StrokeJoin::Round => 1,
            });
            bytes.push(match style.cap {
                toniator_domain::StrokeCap::Round => 1,
            });
        }
        OutputCapabilityPayload::ConnectionPaths {
            site_mechanism_id,
            program,
            style,
        } => {
            bytes.push(3);
            bytes.extend(site_mechanism_id.0.to_le_bytes());
            append_connection_program_identity(bytes, program);
            bytes.push(match style.join {
                toniator_domain::StrokeJoin::Round => 1,
            });
            bytes.push(match style.cap {
                toniator_domain::StrokeCap::Round => 1,
            });
        }
    }
}

/// Appends complete authored connection intent so program and seed changes cannot reuse a realization identity.
fn append_connection_program_identity(bytes: &mut Vec<u8>, program: &ConnectionProgram) {
    // Geometry owns the selection-algorithm discriminator. This stays in the connection-only
    // realization identity; the family structural fingerprint has no output/program bytes.
    append_identity_text(bytes, connection_program_contract_id(program));
    let adjacency = program.adjacency();
    bytes.extend(adjacency.maximum_degree.to_le_bytes());
    bytes.extend(adjacency.maximum_distance.to_bits().to_le_bytes());
    match program {
        ConnectionProgram::NearestLinks { .. } => bytes.push(1),
        ConnectionProgram::RandomLinks {
            minimum_degree,
            seed,
            ..
        } => {
            bytes.push(2);
            bytes.extend(minimum_degree.to_le_bytes());
            bytes.extend(seed.to_le_bytes());
        }
        ConnectionProgram::GridSpanningTree { seed, .. } => {
            bytes.push(3);
            bytes.extend(seed.to_le_bytes());
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

/// Incremental FNV-1a state used when identity inputs are already retained as geometry.
struct Fnv1a64State(u64);

impl Fnv1a64State {
    /// Creates the canonical 64-bit FNV-1a offset-basis state.
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325_u64)
    }

    /// Extends the identity with bytes in their authoritative authored order.
    fn write(&mut self, bytes: impl IntoIterator<Item = u8>) {
        for byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    /// Formats the completed state with the existing stable fingerprint prefix.
    fn finish(self) -> String {
        format!("fnv1a64:{:016x}", self.0)
    }
}

/// Hashes one finite byte sequence with the stable 64-bit FNV-1a contract.
fn fnv1a64(bytes: impl IntoIterator<Item = u8>) -> String {
    let mut state = Fnv1a64State::new();
    state.write(bytes);
    state.finish()
}

#[cfg(test)]
mod stage20m_allocation_tests {
    use super::*;

    /// Exercises Stage 20M's fallible vector reservation mapping without a process-wide allocator.
    #[test]
    fn impossible_reservation_reports_the_requested_stage20m_allocation_path() {
        let mut values = Vec::<u8>::new();
        assert_eq!(
            reserve_stage20m(
                &mut values,
                usize::MAX,
                "maze.allocation",
                "test allocation failure",
            )
            .expect_err("an impossible capacity reservation fails deterministically")
            .path(),
            "maze.allocation"
        );
        assert_eq!(
            reserve_stage20m(
                &mut values,
                usize::MAX,
                "connection.allocation",
                "test allocation failure",
            )
            .expect_err("connection reservation reports its own stable path")
            .path(),
            "connection.allocation"
        );
    }
}

#[cfg(test)]
mod random_prng_contract_tests {
    use super::*;
    use std::cell::Cell;
    use toniator_domain::{CoveragePolicy, PatternDefinitionId};

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

    /// Proves populated-cell cancellation remains per-index without a recipe work ceiling.
    #[test]
    fn populated_spatial_cell_cancels_at_the_per_index_boundary() {
        let mut index = SpatialIndex::new(10.0).unwrap();
        let accepted = vec![FamilySite {
            id: FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal: 0,
            },
            position: Point2::new(1.0, 1.0),
            nominal_cell_basis: NominalCellBasis::new(
                Vector2::new(1.0, 0.0),
                Vector2::new(0.0, 1.0),
            )
            .expect("finite spatial-index test basis"),
            scope: SiteScope::Canvas,
            provenance: FamilySiteProvenance::Random {
                candidate_ordinal: 0,
                accepted_ordinal: 0,
                exclusion_neighbor_ordinal: None,
            },
        }];
        index.insert(accepted[0].position, 0).unwrap();
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

    /// Proves visible-mark exclusion derives center spacing from active maximum support plus margin.
    #[test]
    fn visible_mark_margin_changes_random_site_separation_with_support() {
        let definition = PatternDefinition::random_sites(
            PatternDefinitionId(501),
            "visible mark margin",
            PatternMechanismId(502),
            PatternMechanismId(503),
            PatternMechanismId(504),
            PatternMechanismId(505),
            PatternOutputLayerId(506),
            RandomSiteCharacter::RawUniform,
            7,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::VisibleMarkMargin { margin: 1.0 },
            10_000,
            1_000_000,
            CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        let plan = resolve_pattern_pipeline(&definition).expect("random pipeline resolves");
        let evaluate = |support_radius| {
            evaluate_random_sites_with_progress_cancellable(
                &plan.family,
                &GridInspectRequest {
                    canvas: CanvasSpec {
                        width: 100.0,
                        height: 100.0,
                    },
                    density: ResolvedDensityMetric2D {
                        across_x: 5.0,
                        across_y: 5.0,
                    },
                    rotation_degrees: 0.0,
                    translation_x: 0.0,
                    translation_y: 0.0,
                    guard_steps: 1,
                    support_radius,
                    max_family_candidates: 100_000,
                },
                None,
                &|| false,
                &|_, _| {},
            )
            .expect("bounded random sites evaluate")
        };
        let small = evaluate(1.0);
        let large = evaluate(4.0);
        assert_eq!(
            required_exclusion_distance(&plan.family.random.clone().unwrap(), 1.0),
            3.0
        );
        assert_eq!(
            required_exclusion_distance(&plan.family.random.clone().unwrap(), 4.0),
            9.0
        );
        for (evaluation, minimum) in [(&small, 3.0), (&large, 9.0)] {
            for (index, site) in evaluation.sites.iter().enumerate() {
                assert!(evaluation.sites[index + 1..].iter().all(|other| {
                    (site.position.x - other.position.x).hypot(site.position.y - other.position.y)
                        + 1.0e-12
                        >= minimum
                }));
            }
        }
        assert_ne!(small.family_fingerprint, large.family_fingerprint);
        assert!(large.diagnostics.rejected_by_exclusion > small.diagnostics.rejected_by_exclusion);
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
            density: ResolvedDensityMetric2D {
                across_x: 9.0,
                across_y: 6.0,
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

    /// Realizes a cornered connection trail while retaining segment-local round-join samples.
    ///
    /// # Panics
    ///
    /// Panics when the local finite graph fixture no longer satisfies the public adjacency or
    /// connection-program contracts.
    #[test]
    fn connection_corner_profiles_preserve_round_join_locations_without_stationarity() {
        let mechanism = PatternMechanismId(991);
        let sites = FamilySiteSet::new(
            "connection-corner-profile".into(),
            mechanism,
            [
                Point2::new(0.0, 0.0),
                Point2::new(10.0, 0.0),
                Point2::new(10.0, 10.0),
            ]
            .into_iter()
            .enumerate()
            .map(|(ordinal, position)| FamilySite {
                id: FamilySiteId {
                    mechanism_id: mechanism,
                    ordinal,
                },
                position,
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(10.0, 0.0),
                    Vector2::new(0.0, 10.0),
                )
                .expect("finite local basis"),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: ordinal,
                    accepted_ordinal: ordinal,
                    exclusion_neighbor_ordinal: None,
                },
            })
            .collect(),
        )
        .expect("finite connection sites");
        let adjacency = toniator_domain::ConnectionAdjacencyIntent {
            maximum_degree: 2,
            maximum_distance: 20.0,
        };
        let graph = build_site_adjacency_cancellable(
            &sites,
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: adjacency.maximum_degree as usize,
                maximum_distance: adjacency.maximum_distance,
            },
            SiteAdjacencyLimits::default(),
            &|| false,
        )
        .expect("corner adjacency");
        let paths = build_connection_paths_cancellable(
            PatternOutputLayerId(992),
            &graph,
            &ConnectionProgram::NearestLinks { adjacency },
            ConnectionPathLimits::default(),
            &|| false,
        )
        .expect("corner connection paths");
        assert!(
            paths
                .paths
                .iter()
                .any(|path| path.path.segments().len() > 1)
        );
        let source = source_from_rgba(
            11,
            (0_u8..11)
                .flat_map(|index| {
                    let value = index.saturating_mul(24);
                    [value, value, value, 255]
                })
                .collect(),
        );
        let realized = realize_connection_canonical_strokes_cancellable(
            &paths,
            &source,
            &CanvasSpec {
                width: 10.0,
                height: 10.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Luminance),
            StrokeResponse {
                minimum_thickness: 0.1,
                maximum_thickness: 0.6,
                bias: 0.0,
            },
            toniator_domain::PathStrokeStyle::default(),
            MAX_STROKE_PROFILE_SAMPLES,
            MAX_STROKE_OUTLINE_SEGMENTS,
            &|| false,
        )
        .expect("corner connection outline");
        assert!(
            realized.strokes.iter().any(|stroke| {
                stroke.profile.windows(2).any(|pair| {
                    pair[0].center == pair[1].center && pair[0].location != pair[1].location
                })
            }),
            "connection profiles retain segment-local samples for a round join"
        );
        let source_driven = realize_connection_canonical_strokes_cancellable(
            &paths,
            &source,
            &CanvasSpec {
                width: 10.0,
                height: 10.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Luminance),
            StrokeResponse {
                minimum_thickness: 0.0,
                maximum_thickness: 0.6,
                bias: 0.0,
            },
            toniator_domain::PathStrokeStyle::default(),
            MAX_STROKE_PROFILE_SAMPLES,
            MAX_STROKE_OUTLINE_SEGMENTS,
            &|| false,
        )
        .expect("nonzero source preserves source-driven contours at zero minimum");
        assert!(
            source_driven
                .strokes
                .iter()
                .any(|stroke| !stroke.outline.contours.is_empty()),
            "zero is a lower interpolation bound, not a global stroke visibility toggle"
        );
        let black_source = source_from_rgba(11, [0, 0, 0, 255].repeat(11));
        let zero_response = realize_connection_canonical_strokes_cancellable(
            &paths,
            &black_source,
            &CanvasSpec {
                width: 10.0,
                height: 10.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Luminance),
            StrokeResponse {
                minimum_thickness: 0.0,
                maximum_thickness: 0.6,
                bias: 0.0,
            },
            toniator_domain::PathStrokeStyle::default(),
            MAX_STROKE_PROFILE_SAMPLES,
            MAX_STROKE_OUTLINE_SEGMENTS,
            &|| false,
        )
        .expect("all-zero source keeps the valid empty contour result");
        assert!(
            zero_response
                .strokes
                .iter()
                .all(|stroke| stroke.outline.contours.is_empty()),
            "all-zero source response produces no visible stroke contours"
        );
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

    /// Proves mapped per-site workers preserve stable output and observe cancellation atomically.
    #[test]
    fn mapped_parallel_realization_matches_one_worker_and_cancels() {
        let positions = (0..128)
            .map(|value| f64::from(value) / 127.0)
            .collect::<Vec<_>>();
        let family = sites_at_positions(&positions);
        let source = source_from_rgba(2, vec![255, 0, 0, 255, 0, 0, 0, 255]);
        let canvas = CanvasSpec {
            width: 1.0,
            height: 1.0,
        };
        let mapping = SourceMapping::canonical(SourceMappingComponent::Red);
        let response = MarkResponse {
            minimum_fill: 0.2,
            maximum_fill: 0.9,
            rotation_offset_degrees: 0.0,
        };
        let run = || {
            realize_mapped_circular_marks_cancellable(
                &family,
                &source,
                &canvas,
                mapping,
                response,
                &|| false,
            )
            .expect("mapped realization completes")
        };
        let one = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("one-worker pool builds")
            .install(run);
        let many = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .expect("four-worker pool builds")
            .install(run);
        assert_eq!(one, many);
        let error = realize_mapped_circular_marks_cancellable(
            &family,
            &source,
            &canvas,
            mapping,
            response,
            &|| true,
        )
        .expect_err("cancelled per-site work publishes nothing");
        assert_eq!(error.path(), "evaluation.cancelled");
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

/// Evaluates the cancellation-aware legacy straight-grid structural planner.
///
/// It uses the shared centered local origin and retains the exact accepted output when the probe
/// never cancels.
///
/// # Errors
///
/// Returns validation, bounded coverage, candidate-limit, numeric, or cancellation errors without
/// publishing a partial family.
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
    let transform = grid_prototype_local_to_document_transform(
        &request.canvas,
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
        dimensions[0].coverage(generation_domain, transform)?,
        dimensions[1].coverage(generation_domain, transform)?,
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

    /// Computes bounded local index coverage and the final document-space line phase.
    ///
    /// # Errors
    ///
    /// Returns `coverage` when the finite local generation domain cannot be projected.
    fn coverage(
        self,
        domain: Bounds,
        transform: AffineTransform2D,
    ) -> Result<GuideCoverage, GridError> {
        let (minimum, maximum) = projection_range(domain.corners(), self.normal)
            .ok_or(GridError::new("coverage", "could not project local domain"))?;
        let first_index = checked_index(((minimum - self.phase) / self.spacing).floor())?;
        let last_index = checked_index(((maximum - self.phase) / self.spacing).ceil())?;
        let transformed_tangent = transform.apply_vector(self.tangent);
        let tangent_length = transformed_tangent.x.hypot(transformed_tangent.y);
        if !tangent_length.is_finite() || tangent_length <= 0.0 {
            return Err(GridError::new(
                "channel.pattern.layout",
                "transformed guide tangent must remain finite and nonzero",
            ));
        }
        // Pure rotation preserves a unit tangent exactly by authority even though `sin`/`cos`
        // arithmetic can land a few ulps from one. Stabilizing only that machine-epsilon window
        // retains exact authored spacing and cache identity without masking a real aspect scale.
        let tangent_length = if (tangent_length - 1.0).abs() <= 8.0 * f64::EPSILON {
            1.0
        } else {
            tangent_length
        };
        let document_normal = Vector2::new(
            transformed_tangent.y / tangent_length,
            -transformed_tangent.x / tangent_length,
        );
        let placed_origin = transform.apply_point(Point2::new(0.0, 0.0));
        let placed_phase = placed_origin.dot(document_normal);
        let physical_spacing = self.spacing / tangent_length;
        if !physical_spacing.is_finite() || physical_spacing <= 0.0 {
            return Err(GridError::new(
                "channel.pattern.layout",
                "transformed guide spacing must remain finite and positive",
            ));
        }
        Ok(GuideCoverage {
            dimension_id: self.id.0,
            spacing: physical_spacing,
            normalized_phase: (self.phase + placed_phase).rem_euclid(physical_spacing),
            first_index,
            last_index,
        })
    }

    /// Emits finite presentation segments for this locally indexed guide plan.
    ///
    /// The returned anchors, tangent, and normal are document-space layout values. The pattern
    /// aspect transform may stretch the local lattice, but the tangent and normal remain unit
    /// directions so downstream arc-length and nominal-basis authorities retain physical units.
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
                let transformed_tangent = transform.apply_vector(self.tangent);
                let tangent_length = transformed_tangent.x.hypot(transformed_tangent.y);
                let tangent = transformed_tangent.scale(tangent_length.recip());
                StraightGuide {
                    id: GuideInstanceId::new(self.id, index),
                    normal: Vector2::new(tangent.y, -tangent.x),
                    tangent,
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

/// Converts resolved density authority into an aspect-neutral guide lattice and its layout scale.
///
/// Generalized straight guides first construct their complete topology from the same density-only
/// lattice at every pattern aspect, then apply this positive area-preserving affine transform.
/// That preserves coincident multiway intersections for fixed-direction Triagrid while still
/// stretching all site and baseline positions by the requested pattern aspect.
///
/// # Errors
///
/// Returns the existing finite-positive density diagnostics if a caller bypasses document
/// validation with an invalid resolved layout; no noninvertible layout transform is returned.
fn aspect_normalized_straight_layout(
    canvas: &CanvasSpec,
    resolved: ResolvedDensityMetric2D,
) -> Result<(ResolvedDensityMetric2D, f64, f64), GridError> {
    let metric = DensityMetric2D::from_resolved(canvas, &resolved)
        .map_err(|error| GridError::new(error.path(), error.message()))?;
    let normalized = DensityMetric2D {
        density: metric.density,
        aspect: 1.0,
    }
    .resolve(canvas)
    .map_err(|error| GridError::new(error.path(), error.message()))?;
    let horizontal_scale = metric.aspect.sqrt();
    let vertical_scale = horizontal_scale.recip();
    if !horizontal_scale.is_finite()
        || !vertical_scale.is_finite()
        || horizontal_scale <= 0.0
        || vertical_scale <= 0.0
    {
        return Err(GridError::new(
            "channel.pattern.layout.density_aspect",
            "pattern aspect must resolve to a finite positive layout scale",
        ));
    }
    Ok((normalized, horizontal_scale, vertical_scale))
}

/// Resolves the guide spacing from the documented directional-frequency metric.
pub fn directional_spacing(
    canvas: &CanvasSpec,
    density: &ResolvedDensityMetric2D,
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

/// Places local grid-prototype coordinates at the document canvas center before authored translation.
///
/// Legacy/generalized straight grids and generic-guide prototypes share this authority: local `(0, 0)` maps to
/// the geometric canvas center, rotation occurs about that local origin, and translation then
/// moves the placed result in document axes. The helper returns `None` only when the finite
/// canvas/translation combination cannot produce a finite affine transform.
fn grid_prototype_local_to_document_transform(
    canvas: &CanvasSpec,
    rotation_degrees: f64,
    translation: Vector2,
) -> Option<AffineTransform2D> {
    let placed_origin = Point2::new(
        canvas.width * 0.5 + translation.x,
        canvas.height * 0.5 + translation.y,
    );
    placed_origin.is_finite().then_some(())?;
    AffineTransform2D::rotate_about_then_translate(
        Point2::new(0.0, 0.0),
        rotation_degrees,
        Vector2::new(placed_origin.x, placed_origin.y),
    )
}

/// Hashes legacy straight-grid intent under the centered-local placement contract.
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
    for byte in b"toniator-stage-3-straight-grid-v3-centered-local-origin"
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

    /// Proves authored Constant-gap planning retains the canvas half-diagonal plus two ranks.
    ///
    /// # Panics
    ///
    /// Panics when a smaller projected transverse bound can shorten either mandatory bilateral
    /// rank range for the reported 900-by-620 canvas witness.
    #[test]
    fn authored_normal_offset_rank_plan_has_diagonal_plus_two_floor() {
        let source = CurvePath::line(Point2::new(-450.0, 0.0), Point2::new(450.0, 0.0))
            .expect("fixed authored source validates");
        let domain = Bounds::new(Point2::new(-465.0, -330.0), Point2::new(465.0, 330.0))
            .expect("fixed centered domain validates");
        let center_to_corner = 0.5 * 900.0_f64.hypot(620.0);
        let (first, last) =
            authored_normal_offset_rank_bounds(&source, domain, center_to_corner, 16.128, 1)
                .expect("finite authored Constant-gap range resolves");
        assert_eq!((first, last), (-36, 36));
    }

    /// Proves normal-offset coverage probing has a geometry-owned finite distance sentinel.
    ///
    /// # Panics
    ///
    /// Panics when the bound omits the domain diagonal, fails to include an authored source that
    /// begins outside the domain, or no longer rounds outward to a complete repetition index.
    #[test]
    fn normal_offset_absolute_limit_contains_authored_and_extension_sources() {
        let domain = Bounds::new(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0)).unwrap();
        let inside = Bounds::new(Point2::new(2.0, 4.0), Point2::new(8.0, 6.0)).unwrap();
        assert_eq!(
            normal_offset_absolute_index_limit(inside, domain, 2.0).unwrap(),
            8,
            "the endpoint-extension domain contributes its complete diagonal"
        );
        let outside = Bounds::new(Point2::new(-20.0, 4.0), Point2::new(-10.0, 6.0)).unwrap();
        assert_eq!(
            normal_offset_absolute_index_limit(outside, domain, 2.0).unwrap(),
            16,
            "authored geometry outside the generation domain expands the proof envelope"
        );
    }

    /// Proves chained cubic runway owns exact nodes and tangent-continuous derived seams.
    ///
    /// # Panics
    ///
    /// Panics when end-to-end guide tiling changes the next handle length, leaves a floating-point
    /// node gap, or introduces a seam tangent discontinuity before Constant-gap repetition.
    #[test]
    fn authored_cubic_runway_chains_with_exact_g1_seams() {
        let cubic = CubicBezierSegment::new(
            Point2::new(-10.0, 0.0),
            Point2::new(-6.0, -5.0),
            Point2::new(4.0, 7.0),
            Point2::new(10.0, 0.0),
        )
        .expect("finite authored cubic");
        let source = CurvePath::new(vec![CurveSegment::CubicBezier(cubic)], PathClosure::Open)
            .expect("one-cubic authored guide");
        let domain = Bounds::new(Point2::new(-45.0, -20.0), Point2::new(45.0, 20.0))
            .expect("finite runway domain");
        let tiled = tile_authored_guide_end_to_end(&source, domain, 10.0)
            .expect("authored runway tiles within its bound");
        assert!(tiled.segments().len() > 2);
        let authored_handle_length =
            (cubic.control_1().x - cubic.start().x).hypot(cubic.control_1().y - cubic.start().y);
        for seam in tiled.segments().windows(2) {
            assert_eq!(seam[0].end(), seam[1].start());
            let incoming = seam[0]
                .limiting_unit_tangent_at(1.0)
                .expect("incoming tiled tangent is finite");
            let outgoing = seam[1]
                .limiting_unit_tangent_at(0.0)
                .expect("outgoing tiled tangent is finite");
            assert!((incoming.dot(outgoing) - 1.0).abs() <= 1.0e-12);
            let CurveSegment::CubicBezier(next) = seam[1] else {
                panic!("cubic source produces cubic runway tiles");
            };
            let next_handle_length =
                (next.control_1().x - next.start().x).hypot(next.control_1().y - next.start().y);
            assert!((next_handle_length - authored_handle_length).abs() <= 1.0e-12);
        }
    }

    /// Proves one-time longitudinal scaling cannot change Stacked directional pitch.
    ///
    /// # Panics
    ///
    /// Panics when the authored guide does not scale to the generation span, directional density
    /// does not resolve the expected pitch, or any next-rank construction point receives a
    /// displacement other than that single preset-owned pitch vector.
    #[test]
    fn authored_curve_scales_once_without_scaling_stack_pitch() {
        let cubic = CubicBezierSegment::new(
            Point2::new(-60.0, 0.0),
            Point2::new(-20.0, -24.0),
            Point2::new(20.0, 16.0),
            Point2::new(60.0, 0.0),
        )
        .expect("finite authored stack curve");
        let source = CurvePath::new(vec![CurveSegment::CubicBezier(cubic)], PathClosure::Open)
            .expect("one-cubic authored stack guide");
        let domain = Bounds::new(Point2::new(-160.0, -60.0), Point2::new(160.0, 60.0))
            .expect("finite centered generation domain");
        let scaled =
            scale_authored_guide_to_generation_span(&source, domain).expect("guide scales once");
        assert!(
            (scaled.end().x - scaled.start().x).hypot(scaled.end().y - scaled.start().y) > 120.0
        );
        let canvas = CanvasSpec {
            width: 320.0,
            height: 120.0,
        };
        let density = ResolvedDensityMetric2D {
            across_x: 40.0,
            across_y: 15.0,
        };
        let unit = Vector2::new(0.0, 1.0);
        let pitch = directional_spacing(&canvas, &density, unit)
            .expect("directional stack spacing resolves");
        assert!((pitch - 8.0).abs() <= 1.0e-12);
        let transform = AffineTransform2D::rotate_about_then_translate(
            Point2::new(0.0, 0.0),
            0.0,
            unit.scale(pitch),
        )
        .expect("finite stack translation");
        let next = scaled
            .transformed(transform)
            .expect("next stack rank translates");
        let CurveSegment::CubicBezier(current) = scaled.segments()[0] else {
            panic!("scaled cubic retains its construction kind");
        };
        let CurveSegment::CubicBezier(next) = next.segments()[0] else {
            panic!("translated cubic retains its construction kind");
        };
        for (current, next) in [
            (current.start(), next.start()),
            (current.control_1(), next.control_1()),
            (current.control_2(), next.control_2()),
            (current.end(), next.end()),
        ] {
            assert!((next.x - current.x).abs() <= 1.0e-12);
            assert!((next.y - current.y - pitch).abs() <= 1.0e-12);
        }
    }

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
                planar_switch_nodes: Vec::new(),
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

    /// Places zero-index legacy grid lines at the canvas center before rotation and translation.
    ///
    /// # Panics
    ///
    /// Panics when the centered local-origin transform, zero-index intersection, or reported
    /// document-space phase diverges from the shared straight-grid authority.
    #[test]
    fn legacy_grid_local_origin_maps_to_center_then_document_translation() {
        let mut request = GridInspectRequest {
            canvas: CanvasSpec {
                width: 137.0,
                height: 83.0,
            },
            density: ResolvedDensityMetric2D {
                across_x: 13.7,
                across_y: 8.3,
            },
            rotation_degrees: 47.0,
            translation_x: 0.0,
            translation_y: 0.0,
            guard_steps: 1,
            support_radius: 2.0,
            max_family_candidates: 20_000,
        };
        let center = Point2::new(request.canvas.width * 0.5, request.canvas.height * 0.5);
        let centered = evaluate_straight_grid(&request).expect("centered legacy grid evaluates");
        for dimension_id in [FIRST_DIMENSION_ID, SECOND_DIMENSION_ID] {
            let guide = centered
                .guides
                .iter()
                .find(|guide| guide.id == GuideInstanceId::new(dimension_id, 0))
                .expect("zero-index guide remains covered");
            assert_eq!(guide.anchor, center);
        }
        assert!(centered.sites.iter().any(|site| site.id.first_index == 0
            && site.id.second_index == 0
            && site.position == center));

        request.translation_x = 7.25;
        request.translation_y = -11.5;
        let output = evaluate_straight_grid(&request).expect("translated legacy grid evaluates");
        let origin = Point2::new(
            center.x + request.translation_x,
            center.y + request.translation_y,
        );
        for dimension_id in [FIRST_DIMENSION_ID, SECOND_DIMENSION_ID] {
            let guide = output
                .guides
                .iter()
                .find(|guide| guide.id == GuideInstanceId::new(dimension_id, 0))
                .expect("zero-index guide remains covered");
            assert_eq!(guide.anchor, origin);
            let coverage = output
                .coverage
                .iter()
                .find(|coverage| coverage.dimension_id == dimension_id.0)
                .expect("zero-index guide has coverage");
            let expected_phase = origin.dot(guide.normal).rem_euclid(coverage.spacing);
            assert!((coverage.normalized_phase - expected_phase).abs() < 1.0e-12);
        }
        let site = output
            .sites
            .iter()
            .find(|site| site.id.first_index == 0 && site.id.second_index == 0)
            .expect("zero-index intersection remains covered");
        assert_eq!(site.position, origin);
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
            density: ResolvedDensityMetric2D {
                across_x: 9.0,
                across_y: 6.0,
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
            "fnv1a64:5793408be43e029e:nominal-cell-basis:fnv1a64:2d456031fa9b767c"
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

    /// Proves structural family evaluation remains shared when ordered compatible outputs are plural.
    #[test]
    fn compatible_plural_outputs_share_one_family_product() {
        let mut definition = definition();
        let mut second = definition.output_layers[0].clone();
        second.id = PatternOutputLayerId(14);
        definition.output_layers.push(second);
        let plan = resolve_pattern_pipeline(&definition).expect("plural outputs resolve");
        assert_eq!(plan.ordered_outputs.len(), 2);
        assert_eq!(
            plan.evaluation_order,
            vec![PatternOutputLayerId(13), PatternOutputLayerId(14)]
        );
        let family = evaluate_typed_family(&definition, &request()).expect("one family evaluates");
        assert!(!family.site_set().is_empty());
    }

    /// Proves used, unused, and empty dependency semantics preserve complete-family order and IDs.
    #[test]
    fn site_use_filters_project_complete_family_membership_without_renumbering() {
        let family = evaluate_typed_family(&definition(), &request()).expect("family evaluates");
        let mechanism_id = family.site_set().product_mechanism_id();
        let selected = family
            .site_set()
            .sites()
            .iter()
            .step_by(2)
            .map(|site| site.id)
            .collect::<Vec<_>>();
        let usage = SiteUsageSet::new(mechanism_id, selected.clone()).expect("usage validates");
        let used = family
            .filtered_for_output(
                SiteUseFilter::SitesUsedBy {
                    output_layer_id: PatternOutputLayerId(99),
                },
                Some(&usage),
            )
            .expect("used filter projects");
        assert_eq!(
            used.site_set()
                .sites()
                .iter()
                .map(|site| site.id)
                .collect::<Vec<_>>(),
            selected
        );
        let unused = family
            .filtered_for_output(
                SiteUseFilter::SitesUnusedBy {
                    output_layer_id: PatternOutputLayerId(99),
                },
                Some(&usage),
            )
            .expect("unused filter projects");
        assert_eq!(
            used.site_set().len() + unused.site_set().len(),
            family.site_set().len()
        );
        assert_eq!(used.family_fingerprint(), family.family_fingerprint());
        assert_eq!(unused.family_fingerprint(), family.family_fingerprint());
        let empty = SiteUsageSet::new(mechanism_id, Vec::new()).expect("empty usage validates");
        assert!(
            family
                .filtered_for_output(
                    SiteUseFilter::SitesUsedBy {
                        output_layer_id: PatternOutputLayerId(99),
                    },
                    Some(&empty),
                )
                .expect("empty used filter is valid")
                .site_set()
                .is_empty()
        );
        assert_eq!(
            family
                .filtered_for_output(
                    SiteUseFilter::SitesUnusedBy {
                        output_layer_id: PatternOutputLayerId(99),
                    },
                    Some(&empty),
                )
                .expect("empty complement is valid")
                .site_set()
                .len(),
            family.site_set().len()
        );
    }

    /// Proves family construction polls cancellation before publishing a partial product.
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
        CoveragePolicy, DocumentSession, GeneralizedSiteProduct, MarkOrientation,
        PatternDefinitionId, PatternMechanismId, PatternOutputLayerId, StraightGuideRepetition,
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
            density: ResolvedDensityMetric2D {
                across_x: 12.0,
                across_y: 8.0,
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

    /// Places generalized zero-index guides and their shared intersection at the centered local origin.
    ///
    /// # Panics
    ///
    /// Panics when generalized straight-guide placement diverges from the shared legacy transform.
    #[test]
    fn generalized_grid_local_origin_maps_to_center_then_document_translation() {
        let definition = intersections(
            vec![dimension(1, 0.0), dimension(2, 90.0)],
            vec![GuideDimensionId(1), GuideDimensionId(2)],
        );
        let plan = resolve_pattern_pipeline(&definition).expect("generalized plan resolves");
        let mut request = request();
        request.canvas = CanvasSpec {
            width: 137.0,
            height: 83.0,
        };
        request.density = ResolvedDensityMetric2D {
            across_x: 13.7,
            across_y: 8.3,
        };
        request.rotation_degrees = 47.0;
        request.translation_x = 0.0;
        request.translation_y = 0.0;
        let centered =
            evaluate_generalized_straight_guides_cancellable(&plan.family, &request, &|| false)
                .expect("centered generalized grid evaluates");
        let center = Point2::new(request.canvas.width * 0.5, request.canvas.height * 0.5);
        for dimension_id in [GuideDimensionId(1), GuideDimensionId(2)] {
            let guide = centered
                .guides
                .iter()
                .find(|guide| guide.id == GuideInstanceId::new(dimension_id, 0))
                .expect("zero-index generalized guide remains covered");
            assert_eq!(guide.anchor, center);
        }
        request.translation_x = 7.25;
        request.translation_y = -11.5;
        let output =
            evaluate_generalized_straight_guides_cancellable(&plan.family, &request, &|| false)
                .expect("translated generalized grid evaluates");
        let origin = Point2::new(
            center.x + request.translation_x,
            center.y + request.translation_y,
        );
        for dimension_id in [GuideDimensionId(1), GuideDimensionId(2)] {
            let guide = output
                .guides
                .iter()
                .find(|guide| guide.id == GuideInstanceId::new(dimension_id, 0))
                .expect("zero-index generalized guide remains covered");
            assert_eq!(guide.anchor, origin);
        }
        assert!(output.sites.iter().any(|site| {
            (site.position.x - origin.x).abs() < 1.0e-12
                && (site.position.y - origin.y).abs() < 1.0e-12
        }));
    }

    /// Proves phase-zero 0/60/120 production guides share one centered document origin and spacing.
    ///
    /// # Panics
    ///
    /// Panics when any generalized guide family anchors independently, changes its phase-zero
    /// origin, or resolves unequal physical spacing under an equal document-space density.
    #[test]
    fn phase_zero_triangular_guides_share_the_centered_document_origin() {
        let definition = intersections(
            vec![dimension(1, 0.0), dimension(2, 60.0), dimension(3, 120.0)],
            vec![
                GuideDimensionId(1),
                GuideDimensionId(2),
                GuideDimensionId(3),
            ],
        );
        let plan = resolve_pattern_pipeline(&definition).expect("triangular plan resolves");
        let mut request = request();
        request.canvas = CanvasSpec {
            width: 900.0,
            height: 620.0,
        };
        request.density = ResolvedDensityMetric2D {
            across_x: 5.0,
            across_y: 5.0 * request.canvas.height / request.canvas.width,
        };
        request.rotation_degrees = 0.0;
        request.translation_x = 0.0;
        request.translation_y = 0.0;
        let output =
            evaluate_generalized_straight_guides_cancellable(&plan.family, &request, &|| false)
                .expect("triangular family evaluates");
        let center = Point2::new(request.canvas.width * 0.5, request.canvas.height * 0.5);
        for dimension_id in [
            GuideDimensionId(1),
            GuideDimensionId(2),
            GuideDimensionId(3),
        ] {
            let guide = output
                .guides
                .iter()
                .find(|guide| guide.id == GuideInstanceId::new(dimension_id, 0))
                .expect("zero-index triangular guide remains covered");
            assert!((guide.anchor.x - center.x).hypot(guide.anchor.y - center.y) <= 1.0e-10);
        }
        assert!(
            output
                .coverage
                .windows(2)
                .all(|pair| (pair[0].spacing - pair[1].spacing).abs() <= 1.0e-10)
        );
        assert!(output.sites.iter().any(|site| {
            (site.position.x - center.x).abs() <= 1.0e-10
                && (site.position.y - center.y).abs() <= 1.0e-10
                && matches!(
                    &site.provenance,
                    GeneralizedSiteProvenance::Intersection { contributors }
                        if contributors.len() == 3
                )
        }));
    }

    /// Proves pattern aspect affinely stretches fixed-direction Triagrid without multiplying its topology.
    ///
    /// The finite canvas edge may add or remove one guard row, so this witnesses a narrow count
    /// envelope rather than an impossible exact equality. It specifically rejects the former
    /// independent-direction-spacing behavior that broke three-way concurrence and produced a
    /// roughly threefold site expansion.
    #[test]
    fn triagrid_pattern_aspect_preserves_multiway_topology_and_site_count_scale() {
        let definition = intersections(
            vec![dimension(1, 0.0), dimension(2, 60.0), dimension(3, 120.0)],
            vec![
                GuideDimensionId(1),
                GuideDimensionId(2),
                GuideDimensionId(3),
            ],
        );
        let plan = resolve_pattern_pipeline(&definition).expect("triagrid plan resolves");
        let canvas = CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        };
        let mut counts = Vec::new();
        for aspect in [1.0, 2.0, 0.5] {
            let density = DensityMetric2D {
                density: 100.0,
                aspect,
            }
            .resolve(&canvas)
            .expect("finite pattern aspect resolves");
            let output = evaluate_generalized_straight_guides_cancellable(
                &plan.family,
                &StraightGuideInspectRequest {
                    canvas: canvas.clone(),
                    density,
                    rotation_degrees: 17.0,
                    translation_x: 0.0,
                    translation_y: 0.0,
                    guard_steps: 2,
                    support_radius: 4.5,
                    max_family_candidates: usize::MAX,
                },
                &|| false,
            )
            .expect("aspect-stretched Triagrid evaluates");
            assert!(output.sites.iter().any(|site| matches!(
                &site.provenance,
                GeneralizedSiteProvenance::Intersection { contributors } if contributors.len() == 3
            )));
            counts.push(output.sites.len());
        }
        let baseline = counts[0];
        for (aspect, count) in [2.0, 0.5].into_iter().zip(counts.into_iter().skip(1)) {
            assert!(
                count.abs_diff(baseline) * 100 <= baseline * 6,
                "aspect {aspect} changed Triagrid site count from {baseline} to {count}",
            );
        }
    }

    /// Proves zoomed and rotated fixed-direction triangular guides remain pairwise discrete.
    ///
    /// The Guide Faces consumer requires distinct straight-guide paths to intersect only at
    /// discrete crossings. This reproduces the Stage 21A combined zoom/rotation case before any
    /// canonical region construction, so a duplicate or numerically coincident guide reports its
    /// stable dimension/index identity at the producer boundary.
    #[test]
    fn zoomed_rotated_triangular_guides_have_no_positive_overlap() {
        let definition = intersections(
            vec![dimension(1, 0.0), dimension(2, 60.0), dimension(3, 120.0)],
            vec![
                GuideDimensionId(1),
                GuideDimensionId(2),
                GuideDimensionId(3),
            ],
        );
        let plan = resolve_pattern_pipeline(&definition).expect("triangular plan resolves");
        let canvas = CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        };
        let output = evaluate_generalized_straight_guides_cancellable(
            &plan.family,
            &StraightGuideInspectRequest {
                canvas: canvas.clone(),
                density: DensityMetric2D {
                    density: 125.0,
                    aspect: 1.0,
                }
                .resolve(&canvas)
                .expect("zoomed density resolves"),
                rotation_degrees: 17.0,
                translation_x: 0.0,
                translation_y: 0.0,
                guard_steps: 2,
                support_radius: 12.692,
                max_family_candidates: usize::MAX,
            },
            &|| false,
        )
        .expect("zoomed rotated triangular family evaluates");
        for (first_index, first) in output.guides.iter().enumerate() {
            let first_path = CurvePath::line(first.start, first.end)
                .expect("validated first guide rebuilds a line path");
            for second in output.guides.iter().skip(first_index + 1) {
                let second_path = CurvePath::line(second.start, second.end)
                    .expect("validated second guide rebuilds a line path");
                first_path
                    .intersections(&second_path)
                    .unwrap_or_else(|error| {
                        panic!(
                            "guide {:?} {:?}->{:?} against {:?} {:?}->{:?}: {error}",
                            first.id, first.start, first.end, second.id, second.start, second.end,
                        )
                    });
            }
        }
    }

    /// Proves the bundled authored-guide recipe emits no coincident path interval after zoom and rotation.
    ///
    /// This crosses the document-owned guide-resource adapter used by the production engine. Every
    /// selected structural path pair must remain a set of discrete intersections suitable for Guide
    /// Faces; the assertion reports stable path identity and endpoints if that adapter duplicates an
    /// interval.
    #[test]
    fn zoomed_rotated_three_guide_preset_paths_have_no_positive_overlap() {
        let canvas = CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        };
        let document = Document::new_default_document(canvas.clone(), SourceReference::Unassigned)
            .expect("default document is valid");
        let mut history = DocumentHistory::new(
            DocumentSession::new(document).expect("default document starts a session"),
        );
        PresetRegistry::bundled()
            .apply_to_document_base(&mut history, "three-guide-cells-scale")
            .expect("three-guide preset applies to the document base");
        let definition = history
            .document()
            .pattern_definition_for(ChannelId(1))
            .expect("RGB channel resolves the bundled definition");
        let plan = resolve_document_pattern_pipeline(history.document(), definition)
            .expect("document-owned guide resources resolve");
        let density = DensityMetric2D {
            density: 125.0,
            aspect: 1.0,
        }
        .resolve(&canvas)
        .expect("zoomed density resolves");
        let support_radius = maximum_emitted_guide_spacing(&plan.family, &canvas, &density)
            .expect("guide spacing resolves")
            + definition.coverage.additional_margin;
        let family = evaluate_typed_family_product_cancellable(
            &plan.family,
            &GridInspectRequest {
                canvas,
                density,
                rotation_degrees: 17.0,
                translation_x: 0.0,
                translation_y: 0.0,
                guard_steps: definition.coverage.guard_steps,
                support_radius,
                max_family_candidates: usize::MAX,
            },
            &|| false,
        )
        .expect("bundled zoomed and rotated family evaluates");
        let paths = family
            .structural_path_set()
            .expect("Guide Faces family publishes structural paths")
            .paths();
        for (first_index, first) in paths.iter().enumerate() {
            for first_segment in 0..first.path.segments().len() {
                for second_segment in (first_segment + 2)..first.path.segments().len() {
                    first.path.segments()[first_segment]
                        .intersections(&first.path.segments()[second_segment])
                        .unwrap_or_else(|error| {
                            panic!(
                                "path {:?} segments {first_segment}/{second_segment} {:?}: {error}",
                                first.id,
                                first.path.segments(),
                            )
                        });
                }
            }
            for second in paths.iter().skip(first_index + 1) {
                first
                    .path
                    .intersections(&second.path)
                    .unwrap_or_else(|error| {
                        panic!(
                            "path {:?} {:?} against {:?} {:?}: {error}",
                            first.id,
                            first.path.segments(),
                            second.id,
                            second.path.segments(),
                        )
                    });
            }
        }
        let output = &plan.ordered_outputs[0];
        let RegionSourceIntent::GuideFaces {
            guide_mechanism_id,
            dimensions,
        } = output.regions().expect("bundled output is regional")
        else {
            panic!("bundled output must use Guide Faces")
        };
        build_guide_faces_cancellable(
            GuideFaceRequest {
                output_layer_id: output.layer_id,
                guide_mechanism_id: *guide_mechanism_id,
                dimensions: dimensions.clone(),
                paths: family
                    .structural_path_set()
                    .expect("Guide Faces family publishes structural paths")
                    .clone(),
                canvas: Bounds::new(Point2::new(0.0, 0.0), Point2::new(1024.0, 1024.0))
                    .expect("canvas bounds are finite"),
            },
            GuideFaceLimits::default(),
            || false,
        )
        .expect("zoomed rotated bundled paths build canonical Guide Faces");
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
        let PatternOutputRealization::MarkPrototype {
            site_mechanism_id, ..
        } = &mut changed_mechanism_ids.output_layers[0].realization
        else {
            unreachable!()
        };
        *site_mechanism_id = PatternMechanismId(72);
        assert_ne!(baseline, fingerprint(&changed_mechanism_ids, &request()));
        let mut changed_density = request();
        changed_density.density.across_x *= 2.0;
        assert_ne!(baseline, fingerprint(&definition, &changed_density));
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
        let PatternOutputRealization::MarkPrototype { orientation, .. } =
            &mut fixed.output_layers[0].realization
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
        different_layer.output_layers[0].id = PatternOutputLayerId(99);
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
