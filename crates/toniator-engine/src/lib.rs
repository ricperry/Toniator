#![forbid(unsafe_code)]

//! The shared mutable-document boundary for headless Toniator frontends.

use std::cell::Cell;
#[cfg(test)]
use std::cell::RefCell;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

mod scheduler;

pub use scheduler::{
    ChannelDiagnosticCompletion, ChannelDiagnosticScheduler, ChannelDiagnosticTicket,
    SchedulerError,
};

use toniator_domain::{
    CanvasSpec, ChannelId, EvaluationSnapshot, EvaluationToken, PatternDefinition,
    PatternMechanism, PatternOutputLayer, PatternOutputRealization, RegionGeometryResponse,
    SourceComponent, SourcePlacement, SourceReference, SourceReferenceId,
};
use toniator_domain::{
    ChannelPaint, DocumentEvaluationSnapshot, DocumentEvaluationToken, DocumentSession,
    HalftoneChannelModel, HalftoneChannelRole, ModeledChannelState,
};
use toniator_patterns::{
    Bounds, CONNECTION_PATH_CONTRACT_ID, CONNECTION_TRAIL_CONTRACT_ID, CanonicalRegionLimits,
    CanonicalRegionSet, CurvePath, CurveSegment, FamilyCapability, GUIDE_FACE_CONTRACT_ID,
    GenericGuideCapability, GridFamilyOutput, GuideFaceLimits, GuideFaceRequest,
    MAZE_WALL_CONTRACT_ID, MazeLimits, PathOffsetLimits, PatternPipelineError,
    REGION_TREATMENT_CONTRACT_ID, RegionReference, RegionTreatmentLimits,
    SITE_ADJACENCY_CONTRACT_ID, SiteUsageSet, TypedFamilyOutput, TypedRealization,
    VORONOI_REGION_CONTRACT_ID, VoronoiRegionDiagnostics, VoronoiRegionLimits,
    VoronoiRegionRequest, build_connection_paths_cancellable, build_guide_faces_cancellable,
    build_typed_site_adjacency_cancellable, build_voronoi_regions_cancellable,
    connection_program_contract_id, evaluate_straight_grid,
    evaluate_typed_connection_paths_with_source_cancellable,
    evaluate_typed_family_product_with_source_progress_cancellable,
    evaluate_typed_maze_walls_from_family_cancellable, family_requires_decoded_source,
    maximum_emitted_guide_spacing, maximum_nominal_cell_diameter, realize_circular_marks,
    realize_region_output_with_progress_cancellable, voronoi_region_references,
};
pub use toniator_patterns::{
    CanonicalCircleMark, CanonicalMark, CanonicalMarkRealization, CanonicalStrokeRealization,
    CircularMarkRealization, ConnectionPathEvaluation, ConnectionPathLimits, ConnectionPathSet,
    MarkResponse, MazeProgramResult, Point2, RealizationError, SiteAdjacencyGraph,
    SiteAdjacencyLimits, SiteAdjacencyPolicy, SiteId, SiteScope,
};
#[cfg(test)]
use toniator_patterns::{
    RegionEvaluationEvidence, realize_region_output_with_evidence_cancellable,
};
pub use toniator_render::{
    GeometryOutput, OutputRasterTarget, PreviewRasterTarget, RasterAntialiasing, RasterBackground,
    RasterSurface, RenderError, RenderLayer, RenderScene, SceneIdentity, encode_png,
    linear_to_srgb, raster_output_identity, rasterize, rasterize_cancellable,
    rasterize_cancellable_with_progress, rasterize_output, rasterize_preview,
    rasterize_preview_cancellable, rasterize_preview_cancellable_with_progress, srgb_to_linear,
    write_svg,
};
pub use toniator_sampling::{
    DECODER_CONTRACT_ID, ReducedPreviewSource, SourceField, SourceFormat, SourceFormatHint,
    SourceIdentity, SvgTextDiagnostic, reduced_preview_png,
};
use toniator_sampling::{RegionSamplingLimits, decode_source};

pub use toniator_patterns::{GridError, GridInspectRequest};

/// The accepted mark-response ceiling reserved by family coverage so fill edits remain realization-only.
const MAXIMUM_NORMALIZED_MARK_FILL: f64 = 2.0;

/// Resolves source identity through the accepted sampling decoder.
///
/// This deliberately performs no format parsing, dimension extraction, or
/// sizing policy of its own. Frontends may use it for preflight while the
/// authoritative evaluator performs its normal decode when scheduled.
pub fn resolve_source_identity(
    bytes: &[u8],
    format_hint: SourceFormatHint,
) -> Result<SourceIdentity, EvaluationError> {
    Ok(decode_source(bytes, format_hint)
        .map_err(EvaluationError::from_sampling)?
        .identity()
        .clone())
}

/// Runs the bounded Stage 3 family evaluation through the shared headless boundary.
pub fn inspect_straight_grid(request: &GridInspectRequest) -> Result<GridFamilyOutput, GridError> {
    evaluate_straight_grid(request)
}

/// Derives a caller-supplied topology graph from an accepted family output without changing caches.
///
/// The engine forwards the immutable family result and caller resource policy to the patterns and
/// geometry authorities. It neither persists policy nor publishes adjacency into ordinary family,
/// realization, scene, raster, or scheduler cache slots.
///
/// # Errors
///
/// Returns stable family-product, topology, resource, or cancellation diagnostics and never
/// modifies accepted evaluation caches.
pub fn derive_site_adjacency_cancellable(
    family: &TypedFamilyOutput,
    base_support_radius: f64,
    policy: SiteAdjacencyPolicy,
    limits: EvaluationLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<SiteAdjacencyGraph, EvaluationError> {
    build_typed_site_adjacency_cancellable(
        family,
        base_support_radius,
        policy,
        limits.site_adjacency_limits(),
        is_cancelled,
    )
    .map_err(EvaluationError::from_pipeline)
}

/// Derives one authored connection program from a typed family without adding an adjacency or connection cache.
///
/// # Errors
///
/// Returns stable family, coverage, topology, connection, resource, or cancellation diagnostics and
/// never modifies accepted document cache state.
#[allow(clippy::too_many_arguments)]
pub fn derive_connection_paths_cancellable(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    source: Option<&SourceField>,
    output_layer_id: toniator_domain::PatternOutputLayerId,
    program: &toniator_domain::ConnectionProgram,
    limits: EvaluationLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<ConnectionPathEvaluation, EvaluationError> {
    evaluate_typed_connection_paths_with_source_cancellable(
        family,
        request,
        source,
        output_layer_id,
        program,
        limits.site_adjacency_limits(),
        limits.connection_path_limits(),
        is_cancelled,
    )
    .map_err(EvaluationError::from_pipeline)
}

/// One immutable Stage 4 request: the engine creates family output once, then
/// decodes supplied bytes and realizes canonical circles from that output.
#[derive(Clone, Debug, PartialEq)]
pub struct MarksInspectRequest<'a> {
    pub grid: GridInspectRequest,
    pub source_bytes: &'a [u8],
    pub source_format: SourceFormatHint,
    pub source_component: SourceComponent,
    pub placement: SourcePlacement,
    pub response: MarkResponse,
}

/// The shared headless source-to-family-to-realization boundary.
pub fn inspect_circular_marks(
    request: &MarksInspectRequest<'_>,
) -> Result<CircularMarkRealization, MarksInspectError> {
    let family = inspect_straight_grid(&request.grid)?;
    let source = decode_source(request.source_bytes, request.source_format)?;
    Ok(realize_from_existing_family(
        &family,
        &source,
        &request.grid.canvas,
        request.placement,
        request.source_component,
        request.response,
    )?)
}

/// Immutable source bytes resolved outside the domain. The ID and decoding
/// hint travel with the bytes so the engine can reject authority mismatches
/// before it decodes or evaluates geometry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSource {
    reference_id: SourceReferenceId,
    bytes: Arc<[u8]>,
    format: SourceFormatHint,
}

impl ResolvedSource {
    /// Retains nonempty source bytes with one supported decoder hint.
    ///
    /// # Errors
    ///
    /// Returns a stable source diagnostic for empty bytes or an unsupported
    /// format hint without decoding or changing the supplied bytes.
    pub fn new(
        reference_id: SourceReferenceId,
        bytes: impl Into<Arc<[u8]>>,
        format: SourceFormatHint,
    ) -> Result<Self, EvaluationError> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(EvaluationError::new(
                "source.bytes",
                "source must not be empty",
            ));
        }
        if matches!(format, SourceFormatHint::Unsupported) {
            return Err(EvaluationError::new(
                "source.format",
                "supported source formats are PNG, SVG, JPEG, WebP, BMP, TIFF, OpenEXR, and AVIF",
            ));
        }
        Ok(Self {
            reference_id,
            bytes,
            format,
        })
    }

    pub fn reference_id(&self) -> &SourceReferenceId {
        &self.reference_id
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn format(&self) -> SourceFormatHint {
        self.format
    }
}

/// The sole public Stage 6 evaluation entry. Its fields are private so a
/// snapshot cannot be mixed with another token or resolved source after it is
/// validated.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelDiagnosticRequest {
    snapshot: EvaluationSnapshot,
    source: ResolvedSource,
}

/// Request-wide bounds for ordered composite orchestration across every document channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompositeOutputLimits {
    pub maximum_output_units: usize,
    pub maximum_usage_memberships: usize,
    pub maximum_dependency_inspections: usize,
}

impl CompositeOutputLimits {
    /// Builds one fully enabled composite policy.
    ///
    /// # Errors
    ///
    /// Returns the stable composite-limit diagnostic when any required bound is zero.
    pub fn new(
        maximum_output_units: usize,
        maximum_usage_memberships: usize,
        maximum_dependency_inspections: usize,
    ) -> Result<Self, EvaluationError> {
        if [
            maximum_output_units,
            maximum_usage_memberships,
            maximum_dependency_inspections,
        ]
        .contains(&0)
        {
            return Err(EvaluationError::new(
                "realization.composite.limits.zero",
                "all composite output limits must be nonzero",
            ));
        }
        Ok(Self {
            maximum_output_units,
            maximum_usage_memberships,
            maximum_dependency_inspections,
        })
    }
}

impl Default for CompositeOutputLimits {
    /// Supplies the accepted Stage 20R request-wide composite bounds.
    fn default() -> Self {
        Self {
            maximum_output_units: 4_096,
            maximum_usage_memberships: 8_388_608,
            maximum_dependency_inspections: 16_777_216,
        }
    }
}

/// Immutable resource policy for one evaluation or scheduler.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvaluationLimits {
    max_family_candidates: usize,
    max_flattened_raster_edges: usize,
    max_transformed_curve_segment_instances: usize,
    max_stroke_profile_samples: usize,
    max_stroke_outline_segments: usize,
    site_adjacency: SiteAdjacencyLimits,
    connection_paths: ConnectionPathLimits,
    maze: MazeLimits,
    voronoi: VoronoiRegionLimits,
    guide_faces: GuideFaceLimits,
    region_sampling: RegionSamplingLimits,
    region_treatment: RegionTreatmentLimits,
    composite_outputs: CompositeOutputLimits,
}

impl Eq for EvaluationLimits {}

impl EvaluationLimits {
    /// Represents the absence of an application-authored creative-work ceiling.
    ///
    /// Checked arithmetic, allocation failure, cancellation, and machine
    /// addressability remain authoritative even when this sentinel is active.
    pub const UNBOUNDED_WORK_LIMIT: usize = usize::MAX;
    pub const DEFAULT_MAX_FAMILY_CANDIDATES: usize = Self::UNBOUNDED_WORK_LIMIT;

    /// Builds the complete nonzero default E2 work policy with a caller-selected family bound.
    ///
    /// # Errors
    ///
    /// Returns `coverage.candidate_limit` when family work is disabled.
    pub fn new(max_family_candidates: usize) -> Result<Self, EvaluationError> {
        if max_family_candidates == 0 {
            return Err(EvaluationError::new(
                "coverage.candidate_limit",
                "configured candidate limit must be nonzero",
            ));
        }
        Ok(Self::without_creative_work_ceilings(max_family_candidates))
    }

    /// Builds normal evaluation policy without application-authored count ceilings.
    ///
    /// Algorithmic subdivision depth and numerical tolerances retain their
    /// accepted values because they define deterministic approximation rather
    /// than limiting an artist's requested geometry. Explicit `with_*` methods
    /// remain available to tests and constrained callers.
    ///
    /// # Panics
    ///
    /// Panics only if `usize::MAX` is ever rejected as a nonzero policy by a
    /// geometry-owned constructor, which would violate those constructors'
    /// documented contracts.
    fn without_creative_work_ceilings(max_family_candidates: usize) -> Self {
        let unlimited = Self::UNBOUNDED_WORK_LIMIT;
        let default_offsets = PathOffsetLimits::default();
        Self {
            max_family_candidates,
            max_flattened_raster_edges: unlimited,
            max_transformed_curve_segment_instances: unlimited,
            max_stroke_profile_samples: unlimited,
            max_stroke_outline_segments: unlimited,
            site_adjacency: SiteAdjacencyLimits::new(unlimited, unlimited, unlimited, unlimited)
                .expect("machine work limit is nonzero"),
            connection_paths: ConnectionPathLimits::new(unlimited, unlimited, unlimited, unlimited)
                .expect("machine work limit is nonzero"),
            maze: MazeLimits::new(
                unlimited, unlimited, unlimited, unlimited, unlimited, unlimited, unlimited,
            )
            .expect("machine work limit is nonzero"),
            voronoi: VoronoiRegionLimits::new(
                unlimited, unlimited, unlimited, unlimited, unlimited,
            )
            .expect("machine work limit is nonzero"),
            guide_faces: GuideFaceLimits {
                max_source_paths: unlimited,
                max_source_segments: unlimited,
                max_intersection_contacts: unlimited,
                max_split_segments: unlimited,
                max_vertices: unlimited,
                max_half_edges: unlimited,
                max_faces: unlimited,
                max_ring_segments: unlimited,
                max_inspections: unlimited,
            },
            region_sampling: RegionSamplingLimits {
                max_cell_intersections: unlimited,
                max_flattened_segments: unlimited,
                ..RegionSamplingLimits::default()
            },
            region_treatment: RegionTreatmentLimits {
                canonical: CanonicalRegionLimits::new(unlimited, unlimited, unlimited, unlimited)
                    .expect("machine work limit is nonzero"),
                path_offset: PathOffsetLimits {
                    maximum_segments: unlimited,
                    maximum_components: unlimited,
                    maximum_cleanup_pairs: unlimited,
                    maximum_cusp_isolation_work: unlimited,
                    ..default_offsets
                },
            },
            composite_outputs: CompositeOutputLimits {
                maximum_output_units: unlimited,
                maximum_usage_memberships: unlimited,
                maximum_dependency_inspections: unlimited,
            },
        }
    }

    pub const fn max_family_candidates(self) -> usize {
        self.max_family_candidates
    }

    /// Replaces the concrete raster edge budget while retaining the existing family candidate limit.
    ///
    /// # Errors
    ///
    /// Returns a stable evaluation diagnostic when the required finite raster bound is disabled.
    pub fn with_max_flattened_raster_edges(
        mut self,
        value: usize,
    ) -> Result<Self, EvaluationError> {
        if value == 0 {
            return Err(EvaluationError::new(
                "raster.limits.flattened_edges",
                "configured flattened raster edge limit must be nonzero",
            ));
        }
        self.max_flattened_raster_edges = value;
        Ok(self)
    }

    /// Returns the request-wide upper bound shared by all adaptive canonical path rasterization.
    pub const fn max_flattened_raster_edges(self) -> usize {
        self.max_flattened_raster_edges
    }

    /// Replaces the pre-allocation transformed authored-segment instance bound.
    ///
    /// # Errors
    ///
    /// Returns the stable segment-limit diagnostic when the required bound is disabled.
    pub fn with_max_transformed_curve_segment_instances(
        mut self,
        value: usize,
    ) -> Result<Self, EvaluationError> {
        if value == 0 {
            return Err(EvaluationError::new(
                "realization.mark.segment_limit",
                "configured transformed curve-segment limit must be nonzero",
            ));
        }
        self.max_transformed_curve_segment_instances = value;
        Ok(self)
    }

    /// Returns the exact checked transformed authored-segment instance limit.
    pub const fn max_transformed_curve_segment_instances(self) -> usize {
        self.max_transformed_curve_segment_instances
    }

    /// Returns the request-wide canonical connected-stroke profile sample limit.
    pub const fn max_stroke_profile_samples(self) -> usize {
        self.max_stroke_profile_samples
    }

    /// Replaces the nonzero request-wide connected-stroke profile bound.
    ///
    /// # Errors
    ///
    /// Rejects a disabled profile budget before an evaluation can allocate partial geometry.
    pub fn with_max_stroke_profile_samples(
        mut self,
        value: usize,
    ) -> Result<Self, EvaluationError> {
        if value == 0 {
            return Err(EvaluationError::new(
                "realization.stroke.profile_limit",
                "configured stroke profile limit must be nonzero",
            ));
        }
        self.max_stroke_profile_samples = value;
        Ok(self)
    }

    /// Returns the request-wide canonical connected-stroke outline segment limit.
    pub const fn max_stroke_outline_segments(self) -> usize {
        self.max_stroke_outline_segments
    }

    /// Replaces the nonzero request-wide connected-stroke outline bound.
    ///
    /// # Errors
    ///
    /// Rejects a disabled outline-segment budget before an evaluation can allocate partial geometry.
    pub fn with_max_stroke_outline_segments(
        mut self,
        value: usize,
    ) -> Result<Self, EvaluationError> {
        if value == 0 {
            return Err(EvaluationError::new(
                "realization.stroke.outline_limit",
                "configured stroke outline limit must be nonzero",
            ));
        }
        self.max_stroke_outline_segments = value;
        Ok(self)
    }

    /// Returns the caller-applied bounded work policy for derived site adjacency.
    pub const fn site_adjacency_limits(self) -> SiteAdjacencyLimits {
        self.site_adjacency
    }

    /// Returns the bounded derived connection selection and trail policy.
    pub const fn connection_path_limits(self) -> ConnectionPathLimits {
        self.connection_paths
    }

    /// Returns the bounded arrangement, dual traversal, and retained-wall policy for maze outputs.
    pub const fn maze_limits(self) -> MazeLimits {
        self.maze
    }

    /// Returns the geometry-owned bounded policy for ordinary Voronoi region realization.
    pub const fn voronoi_region_limits(self) -> VoronoiRegionLimits {
        self.voronoi
    }

    /// Returns the geometry-owned bounded policy for guide-arrangement face realization.
    pub const fn guide_face_limits(self) -> GuideFaceLimits {
        self.guide_faces
    }

    /// Returns the request-wide sampling bounds for one typed filled-region output.
    ///
    /// These bounds remain evaluator policy rather than document intent and therefore participate
    /// in only the independently cached region realization that consumes them.
    pub const fn region_sampling_limits(self) -> RegionSamplingLimits {
        self.region_sampling
    }

    /// Returns the request-wide treatment bounds for one typed filled-region output.
    ///
    /// These bounds remain evaluator policy rather than document intent and are never persisted.
    pub const fn region_treatment_limits(self) -> RegionTreatmentLimits {
        self.region_treatment
    }

    /// Returns the request-wide output, usage, and dependency work policy.
    pub const fn composite_output_limits(self) -> CompositeOutputLimits {
        self.composite_outputs
    }

    /// Replaces the complete nonzero composite work policy.
    ///
    /// # Errors
    ///
    /// Returns the stable zero-limit diagnostic before evaluation can allocate derived output.
    pub fn with_composite_output_limits(
        mut self,
        limits: CompositeOutputLimits,
    ) -> Result<Self, EvaluationError> {
        self.composite_outputs = CompositeOutputLimits::new(
            limits.maximum_output_units,
            limits.maximum_usage_memberships,
            limits.maximum_dependency_inspections,
        )?;
        Ok(self)
    }

    /// Replaces nonzero bounded work policy for deterministic region-source sampling.
    ///
    /// # Errors
    ///
    /// Returns `sampling.region_average.limits.zero` before a cache candidate or partial sample
    /// table can be allocated when any mandatory work bound is disabled.
    pub fn with_region_sampling_limits(
        mut self,
        limits: RegionSamplingLimits,
    ) -> Result<Self, EvaluationError> {
        if limits.max_cell_intersections == 0
            || limits.max_flattened_segments == 0
            || limits.max_subdivision_depth == 0
        {
            return Err(EvaluationError::new(
                "sampling.region_average.limits.zero",
                "region sampling limits must be nonzero",
            ));
        }
        self.region_sampling = limits;
        Ok(self)
    }

    /// Replaces filled-region treatment policy without altering document-owned response intent.
    ///
    /// # Errors
    ///
    /// Returns the treatment boundary's stable limit error before evaluation if a canonical or
    /// path-offset limit is invalid; the geometry authority retains ownership of detailed checks.
    pub fn with_region_treatment_limits(
        mut self,
        limits: RegionTreatmentLimits,
    ) -> Result<Self, EvaluationError> {
        if limits.canonical.max_source_groups() == 0
            || limits.canonical.max_regions() == 0
            || limits.canonical.max_segments() == 0
            || limits.canonical.max_inspections() == 0
            || limits.path_offset.maximum_subdivision_depth == 0
            || limits.path_offset.maximum_segments == 0
            || limits.path_offset.maximum_components == 0
            || limits.path_offset.maximum_cleanup_pairs == 0
            || limits.path_offset.maximum_cusp_isolation_work == 0
            || !limits.path_offset.tolerance.is_finite()
            || limits.path_offset.tolerance <= 0.0
        {
            return Err(EvaluationError::new(
                "region.treatment.limits.zero",
                "region treatment limits must be finite and nonzero",
            ));
        }
        self.region_treatment = limits;
        Ok(self)
    }

    /// Replaces nonzero guide-face work bounds without changing document authority.
    ///
    /// # Errors
    ///
    /// Returns the geometry-owned zero-limit diagnostic before a cache key or
    /// candidate arrangement can be allocated.
    pub fn with_guide_face_limits(
        mut self,
        limits: GuideFaceLimits,
    ) -> Result<Self, EvaluationError> {
        if [
            limits.max_source_paths,
            limits.max_source_segments,
            limits.max_intersection_contacts,
            limits.max_split_segments,
            limits.max_vertices,
            limits.max_half_edges,
            limits.max_faces,
            limits.max_ring_segments,
            limits.max_inspections,
        ]
        .into_iter()
        .any(|limit| limit == 0)
        {
            return Err(EvaluationError::new(
                "region.guide_faces.limits.zero",
                "guide-face limits must be nonzero",
            ));
        }
        self.guide_faces = limits;
        Ok(self)
    }

    /// Replaces the complete nonzero ordinary-Voronoi work policy without changing document intent.
    ///
    /// # Errors
    ///
    /// Returns the geometry-owned zero-limit diagnostic before any cache key or realization builds.
    pub fn with_voronoi_region_limits(
        mut self,
        limits: VoronoiRegionLimits,
    ) -> Result<Self, EvaluationError> {
        VoronoiRegionLimits::new(
            limits.max_site_groups(),
            limits.max_topology_edges(),
            limits.max_regions(),
            limits.max_boundary_points(),
            limits.max_inspections(),
        )
        .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
        self.voronoi = limits;
        Ok(self)
    }

    /// Replaces maze work limits without changing authored document authority.
    ///
    /// # Errors
    ///
    /// Returns the geometry-owned maze limit diagnostic before any arrangement allocation.
    pub fn with_maze_limits(mut self, limits: MazeLimits) -> Result<Self, EvaluationError> {
        MazeLimits::new(
            limits.maximum_source_walls,
            limits.maximum_faces,
            limits.maximum_dual_adjacencies,
            limits.maximum_passages,
            limits.maximum_wall_trails,
            limits.maximum_retained_points,
            limits.maximum_inspections,
        )
        .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
        self.maze = limits;
        Ok(self)
    }

    /// Replaces connection work limits without changing authored document authority.
    ///
    /// # Errors
    ///
    /// Returns a stable geometry-owned diagnostic when any connection work category is disabled.
    pub fn with_connection_path_limits(
        mut self,
        limits: ConnectionPathLimits,
    ) -> Result<Self, EvaluationError> {
        ConnectionPathLimits::new(
            limits.maximum_selected_edges,
            limits.maximum_trails,
            limits.maximum_retained_path_points,
            limits.maximum_inspections,
        )
        .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
        self.connection_paths = limits;
        Ok(self)
    }

    /// Replaces the complete nonzero adjacency resource policy without changing document authority.
    ///
    /// # Errors
    ///
    /// Returns the stable geometry-owned limit diagnostic when a topology budget is disabled.
    pub fn with_site_adjacency_limits(
        mut self,
        limits: SiteAdjacencyLimits,
    ) -> Result<Self, EvaluationError> {
        SiteAdjacencyLimits::new(
            limits.maximum_nodes,
            limits.maximum_neighbor_memberships,
            limits.maximum_edges,
            limits.maximum_distance_checks,
        )
        .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
        self.site_adjacency = limits;
        Ok(self)
    }
}

impl Default for EvaluationLimits {
    /// Supplies normal policy without application-authored creative-work ceilings.
    ///
    /// Machine addressability, checked arithmetic, fallible allocation,
    /// cancellation, stale-result rejection, and deterministic algorithmic
    /// subdivision remain enforced.
    fn default() -> Self {
        Self::without_creative_work_ceilings(Self::DEFAULT_MAX_FAMILY_CANDIDATES)
    }
}

#[cfg(test)]
mod stage20e2_limit_tests {
    use super::*;

    /// Fixes monotonic family/output milestone weights independently of worker timing.
    #[test]
    fn document_progress_assigns_fixed_family_and_output_contributions() {
        assert_eq!(document_work_progress(0, 3, 0, 6), 200);
        assert_eq!(document_work_progress(1, 3, 0, 6), 283);
        assert_eq!(document_work_progress(3, 3, 3, 6), 675);
        assert_eq!(document_work_progress(3, 3, 6, 6), 900);
        assert_eq!(document_family_work_progress(0, 2, 0, 2, 0, 100), 200);
        assert_eq!(document_family_work_progress(0, 2, 0, 2, 50, 100), 262);
        assert_eq!(document_family_work_progress(1, 2, 1, 2, 50, 100), 612);
        assert_eq!(document_output_work_progress(2, 2, 0, 2, 0, 100), 450);
        assert_eq!(document_output_work_progress(2, 2, 0, 2, 50, 100), 562);
        assert_eq!(document_output_work_progress(2, 2, 1, 2, 50, 100), 787);
        assert_eq!(raster_work_progress(0, 100), 950);
        assert_eq!(raster_work_progress(50, 100), 970);
        assert_eq!(raster_work_progress(100, 100), 990);
    }

    /// Proves normal evaluation has no creative count ceiling while explicit limits remain valid.
    #[test]
    fn evaluation_defaults_disable_creative_work_ceilings() {
        let defaults = EvaluationLimits::default();
        assert_eq!(
            defaults.max_transformed_curve_segment_instances(),
            EvaluationLimits::UNBOUNDED_WORK_LIMIT
        );
        assert_eq!(
            defaults.max_flattened_raster_edges(),
            EvaluationLimits::UNBOUNDED_WORK_LIMIT
        );
        assert_eq!(
            defaults.max_family_candidates(),
            EvaluationLimits::UNBOUNDED_WORK_LIMIT
        );
        assert_eq!(
            defaults.guide_face_limits().max_inspections,
            EvaluationLimits::UNBOUNDED_WORK_LIMIT
        );
        assert_eq!(
            defaults.site_adjacency_limits().maximum_nodes,
            EvaluationLimits::UNBOUNDED_WORK_LIMIT
        );
        assert_eq!(
            defaults
                .with_max_transformed_curve_segment_instances(0)
                .unwrap_err()
                .path(),
            "realization.mark.segment_limit"
        );
        assert_eq!(
            defaults
                .with_max_flattened_raster_edges(0)
                .unwrap_err()
                .path(),
            "raster.limits.flattened_edges"
        );
    }

    /// Proves connected-stroke request limits are explicit nonzero cache-contract inputs.
    #[test]
    fn connected_stroke_limits_are_configurable_and_reject_disabled_values() {
        let limits = EvaluationLimits::default()
            .with_max_stroke_profile_samples(17)
            .expect("profile limit accepts nonzero")
            .with_max_stroke_outline_segments(23)
            .expect("outline limit accepts nonzero");
        assert_eq!(limits.max_stroke_profile_samples(), 17);
        assert_eq!(limits.max_stroke_outline_segments(), 23);
        assert_eq!(
            EvaluationLimits::default()
                .with_max_stroke_profile_samples(0)
                .expect_err("disabled profile limit rejects")
                .path(),
            "realization.stroke.profile_limit"
        );
        assert_eq!(
            EvaluationLimits::default()
                .with_max_stroke_outline_segments(0)
                .expect_err("disabled outline limit rejects")
                .path(),
            "realization.stroke.outline_limit"
        );
    }

    /// Proves Stage 20Q sampling policy is explicit evaluator state and rejects a disabled budget.
    #[test]
    fn region_sampling_limits_are_configurable_and_reject_disabled_values() {
        let configured = RegionSamplingLimits {
            max_cell_intersections: 17,
            max_flattened_segments: 19,
            max_subdivision_depth: 23,
        };
        assert_eq!(
            EvaluationLimits::default()
                .with_region_sampling_limits(configured)
                .expect("nonzero sampling limits")
                .region_sampling_limits(),
            configured
        );
        assert_eq!(
            EvaluationLimits::default()
                .with_region_sampling_limits(RegionSamplingLimits {
                    max_cell_intersections: 0,
                    ..configured
                })
                .expect_err("disabled intersection budget rejects")
                .path(),
            "sampling.region_average.limits.zero"
        );
    }

    /// Proves Stage 20Q treatment policy enters evaluation limits and rejects invalid offset work.
    #[test]
    fn region_treatment_limits_are_configurable_and_reject_invalid_offset_policy() {
        let accepted = RegionTreatmentLimits::default();
        assert_eq!(
            EvaluationLimits::default()
                .with_region_treatment_limits(accepted)
                .expect("default treatment limits")
                .region_treatment_limits(),
            accepted
        );
        let mut invalid = accepted;
        invalid.path_offset.maximum_segments = 0;
        assert_eq!(
            EvaluationLimits::default()
                .with_region_treatment_limits(invalid)
                .expect_err("disabled offset work rejects")
                .path(),
            "region.treatment.limits.zero"
        );
    }
}

#[cfg(test)]
mod stage20q_region_cache_tests {
    use super::*;

    /// Builds a finite solid paint fixture without creating a frontend-owned color authority.
    fn solid_paint() -> ChannelPaint {
        ChannelPaint::Solid(toniator_domain::ColorValue {
            red: 0.25,
            green: 0.5,
            blue: 0.75,
            alpha: 1.0,
        })
    }

    /// Proves resize envelopes use the shared normalized fill maximum for both algorithms.
    #[test]
    fn region_support_uses_outward_normalized_fill_extrema() {
        let scale = RegionGeometryResponse {
            algorithm: toniator_domain::RegionResizeAlgorithm::Scale,
            sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
            minimum_fill: 0.0,
            maximum_fill: 1.75,
        };
        let unit_offset = RegionGeometryResponse {
            algorithm: toniator_domain::RegionResizeAlgorithm::UniformOffset,
            sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
            minimum_fill: 0.0,
            maximum_fill: 1.0,
        };
        assert_eq!(
            region_treatment_outward_support(&scale, 10.0).expect("finite scale support"),
            7.5
        );
        assert_eq!(
            region_treatment_outward_support(&unit_offset, 10.0).expect("unit fill has no growth"),
            0.0
        );
    }

    /// Proves invalid support arithmetic fails before a family evaluator can allocate candidates.
    #[test]
    fn region_support_rejects_nonfinite_and_overflowing_inputs() {
        let nonfinite = RegionGeometryResponse {
            algorithm: toniator_domain::RegionResizeAlgorithm::Scale,
            sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
            minimum_fill: 0.0,
            maximum_fill: f64::INFINITY,
        };
        assert_eq!(
            region_treatment_outward_support(&nonfinite, 1.0)
                .expect_err("nonfinite fill rejects")
                .path(),
            "region.resize.coverage.maximum_fill"
        );
        assert_eq!(
            checked_region_support_add(f64::MAX, f64::MAX)
                .expect_err("overflow rejects")
                .path(),
            "region.treatment.coverage.support"
        );
    }

    /// Proves every normalized region response keeps source identity because fill is sampled.
    #[test]
    fn normalized_region_output_requires_sampling_identity_for_solid_and_sampled_paint() {
        let response = toniator_domain::PatternGeometryResponse::Regions(RegionGeometryResponse {
            algorithm: toniator_domain::RegionResizeAlgorithm::Scale,
            sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
            minimum_fill: 0.0,
            maximum_fill: 1.0,
        });
        assert!(output_sampling_required(&response, &solid_paint()));
        assert!(output_sampling_required(
            &response,
            &ChannelPaint::SampledSource
        ));
    }

    /// Proves numeric treatment and sampled-paint paths remain source-sensitive cache consumers.
    #[test]
    fn sampled_or_treated_region_output_requires_sampling_identity() {
        let scale = toniator_domain::PatternGeometryResponse::Regions(RegionGeometryResponse {
            algorithm: toniator_domain::RegionResizeAlgorithm::Scale,
            sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
            minimum_fill: 0.5,
            maximum_fill: 1.5,
        });
        assert!(output_sampling_required(&scale, &solid_paint()));
        assert!(output_sampling_required(
            &scale,
            &ChannelPaint::SampledSource
        ));
    }
}

/// Whether a successful evaluation reused an accepted derived value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheDisposition {
    Hit,
    Miss,
}

/// Immutable per-layer diagnostics for a successful evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheDiagnostics {
    pub decoded_source: CacheDisposition,
    pub family: CacheDisposition,
    pub realization: CacheDisposition,
    pub scene: CacheDisposition,
    pub raster: CacheDisposition,
}

impl ChannelDiagnosticRequest {
    pub fn new(snapshot: EvaluationSnapshot, source: ResolvedSource) -> Self {
        Self { snapshot, source }
    }

    pub(crate) fn token(&self) -> EvaluationToken {
        self.snapshot.token()
    }
}

/// Immutable result from evaluating one authoritative snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelDiagnosticResult {
    token: EvaluationToken,
    source_identity: SourceIdentity,
    scene: Arc<RenderScene>,
    raster: Arc<RasterSurface>,
}

impl ChannelDiagnosticResult {
    pub fn token(&self) -> EvaluationToken {
        self.token
    }
    pub fn source_identity(&self) -> &SourceIdentity {
        &self.source_identity
    }

    /// Returns the immutable diagnostic scene without exposing result/cache ownership.
    pub fn scene(&self) -> &RenderScene {
        self.scene.as_ref()
    }

    /// Returns the immutable diagnostic raster without exposing result/cache ownership.
    pub fn raster(&self) -> &RasterSurface {
        self.raster.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceCacheKey {
    reference_id: String,
    bytes: Arc<[u8]>,
    format: SourceFormatHint,
    decoder_contract: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
struct FamilyCacheKey {
    canvas: (u64, u64),
    density: (u64, u64),
    rotation: u64,
    translation: (u64, u64),
    guard_steps: u32,
    definition: FamilyDefinitionKey,
    required_support_radius: u64,
    max_family_candidates: usize,
    structural_source: Option<RealizationSourceIdentity>,
}

/// Reports whether an existing family key has identical structural inputs and an envelope at least as broad.
fn family_key_supports(candidate: &FamilyCacheKey, requested: &FamilyCacheKey) -> bool {
    let candidate_support = f64::from_bits(candidate.required_support_radius);
    let requested_support = f64::from_bits(requested.required_support_radius);
    let mut comparable = candidate.clone();
    comparable.required_support_radius = requested.required_support_radius;
    comparable == *requested && candidate_support >= requested_support
}

#[derive(Clone, Debug, PartialEq)]
struct FamilyDefinitionKey {
    definition_id: u64,
    family: toniator_domain::PatternFamily,
    mechanisms: Vec<PatternMechanism>,
    resolved_guide_content: Option<String>,
    path_offset_algorithm: Option<PathOffsetAlgorithmKey>,
}

#[derive(Clone, Debug, PartialEq)]
struct PathOffsetAlgorithmKey {
    contract_id: &'static str,
    maximum_subdivision_depth: u8,
    maximum_segments: usize,
    maximum_components: usize,
    maximum_cleanup_pairs: usize,
    maximum_cusp_isolation_work: usize,
    tolerance: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct RealizationContractKey {
    output_layers: Vec<PatternOutputLayer>,
    modulation: toniator_domain::PatternModulation,
}

/// The decoder-owned source identity consumed by realization. Logical source
/// lookup/reference identity stays at the decode cache boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RealizationSourceIdentity {
    format: toniator_sampling::SourceFormat,
    width: u32,
    height: u32,
    content_hash: String,
    decoded_pixel_hash: String,
}

#[derive(Clone, Debug, PartialEq)]
struct RealizationCacheKey {
    family: FamilyCacheKey,
    contract: RealizationContractKey,
    source_identity: RealizationSourceIdentity,
    canvas: (u64, u64),
    source_component: u8,
    placement: u8,
    response: (u64, u64, u64),
}

#[derive(Clone, Debug, PartialEq)]
struct SceneCacheKey {
    realization: RealizationCacheKey,
    canvas: (u64, u64),
    channel_id: u64,
    visible: bool,
    color: (u64, u64, u64, u64),
    opacity: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct RasterCacheKey {
    scene: SceneCacheKey,
    transparent_raster_contract: &'static str,
    max_flattened_raster_edges: usize,
}

const TRANSPARENT_RASTER_CONTRACT_ID: &str = "toniator-render-transparent-raster-v1";

#[derive(Clone)]
struct DerivedCacheSnapshot {
    source: Option<(SourceCacheKey, Arc<SourceField>)>,
    family: Option<(FamilyCacheKey, Arc<TypedFamilyOutput>)>,
    realization: Option<(
        RealizationCacheKey,
        Arc<TypedRealization<CircularMarkRealization>>,
    )>,
    scene: Option<(SceneCacheKey, Arc<RenderScene>)>,
    raster: Option<(RasterCacheKey, Arc<RasterSurface>)>,
}

impl DerivedCacheSnapshot {
    fn empty() -> Self {
        Self {
            source: None,
            family: None,
            realization: None,
            scene: None,
            raster: None,
        }
    }
}

/// Private, disposable, last-successful cache. Values are immutable `Arc`s and
/// are installed only by scheduler acceptance.
#[derive(Default)]
pub(crate) struct DerivedCache {
    source: Option<(SourceCacheKey, Arc<SourceField>)>,
    family: Option<(FamilyCacheKey, Arc<TypedFamilyOutput>)>,
    realization: Option<(
        RealizationCacheKey,
        Arc<TypedRealization<CircularMarkRealization>>,
    )>,
    scene: Option<(SceneCacheKey, Arc<RenderScene>)>,
    raster: Option<(RasterCacheKey, Arc<RasterSurface>)>,
}

impl DerivedCache {
    pub(crate) fn snapshot(&self) -> DerivedCacheSnapshot {
        DerivedCacheSnapshot {
            source: self.source.clone(),
            family: self.family.clone(),
            realization: self.realization.clone(),
            scene: self.scene.clone(),
            raster: self.raster.clone(),
        }
    }

    pub(crate) fn commit(&mut self, transaction: CacheTransaction) {
        if let Some(value) = transaction.source {
            self.source = Some(value);
        }
        if let Some(value) = transaction.family {
            self.family = Some(value);
        }
        if let Some(value) = transaction.realization {
            self.realization = Some(value);
        }
        if let Some(value) = transaction.scene {
            self.scene = Some(value);
        }
        if let Some(value) = transaction.raster {
            self.raster = Some(value);
        }
    }
}

#[derive(Default)]
pub(crate) struct CacheTransaction {
    source: Option<(SourceCacheKey, Arc<SourceField>)>,
    family: Option<(FamilyCacheKey, Arc<TypedFamilyOutput>)>,
    realization: Option<(
        RealizationCacheKey,
        Arc<TypedRealization<CircularMarkRealization>>,
    )>,
    scene: Option<(SceneCacheKey, Arc<RenderScene>)>,
    raster: Option<(RasterCacheKey, Arc<RasterSurface>)>,
}

pub(crate) struct CachedEvaluation {
    pub(crate) result: ChannelDiagnosticResult,
    pub(crate) diagnostics: CacheDiagnostics,
    pub(crate) transaction: CacheTransaction,
}

/// Evaluates exactly the Stage 3 -> Stage 4 -> Stage 5 chain represented by
/// the immutable snapshot. It performs no document mutation or source lookup.
pub fn evaluate_channel_diagnostic(
    request: ChannelDiagnosticRequest,
) -> Result<ChannelDiagnosticResult, EvaluationError> {
    evaluate_channel_diagnostic_with_limits(request, EvaluationLimits::default())
}

/// Evaluates one immutable request under a caller-selected family candidate
/// policy. This synchronous entry is intentionally uncached.
pub fn evaluate_channel_diagnostic_with_limits(
    request: ChannelDiagnosticRequest,
    limits: EvaluationLimits,
) -> Result<ChannelDiagnosticResult, EvaluationError> {
    match evaluate_channel_diagnostic_cached_with_cancellation(
        request,
        &NeverCancelled,
        DerivedCacheSnapshot::empty(),
        limits,
    ) {
        Ok(result) => Ok(result.result),
        Err(EvaluationRunError::Evaluation(error)) => Err(error),
        Err(EvaluationRunError::Cancelled) => unreachable!("synchronous evaluation never cancels"),
    }
}

pub(crate) fn evaluate_channel_diagnostic_cancellable_cached(
    request: ChannelDiagnosticRequest,
    cancelled: &AtomicBool,
    cache: DerivedCacheSnapshot,
    limits: EvaluationLimits,
) -> Result<CachedEvaluation, EvaluationRunError> {
    evaluate_channel_diagnostic_cached_with_cancellation(
        request,
        &AtomicCancellation(cancelled),
        cache,
        limits,
    )
}

#[cfg(test)]
pub(crate) fn evaluate_channel_diagnostic_cancellable_with_gate(
    request: ChannelDiagnosticRequest,
    cancelled: &AtomicBool,
    gate: &EvaluationStageGate,
) -> Result<ChannelDiagnosticResult, EvaluationRunError> {
    evaluate_channel_diagnostic_cached_with_cancellation(
        request,
        &ObservedCancellation { cancelled, gate },
        DerivedCacheSnapshot::empty(),
        EvaluationLimits::default(),
    )
    .map(|result| result.result)
}

trait CancellationProbe: Sync {
    fn is_cancelled(&self) -> bool;

    /// Reports one non-authoritative document-worker milestone and its current-stage completion.
    fn report_progress(
        &self,
        _stage: EvaluationProgressStage,
        _completed_per_mille: u16,
        _stage_completed_per_mille: u16,
    ) {
    }

    #[cfg(test)]
    fn observe_stage(&self, _stage: EvaluationStage, _checkpoint: EvaluationCheckpoint) {}
}

thread_local! {
    /// Remembers the last profiled invocation registered by each participating worker thread.
    static LAST_PROFILE_PARTICIPATION_ID: Cell<usize> = const { Cell::new(0) };
}

/// Supplies process-local identities for opt-in profiling invocations only.
static NEXT_PROFILE_PARTICIPATION_ID: AtomicUsize = AtomicUsize::new(1);

/// Records lightweight observed Rayon participation through existing cancellation polls.
struct ParallelParticipation {
    id: usize,
    worker_mask: AtomicU64,
    worker_registration_count: AtomicUsize,
}

impl Default for ParallelParticipation {
    /// Allocates one diagnostic-only invocation identity and empty participation set.
    fn default() -> Self {
        Self {
            id: NEXT_PROFILE_PARTICIPATION_ID.fetch_add(1, Ordering::Relaxed),
            worker_mask: AtomicU64::new(0),
            worker_registration_count: AtomicUsize::new(0),
        }
    }
}

impl ParallelParticipation {
    /// Registers the current Rayon worker once per profiled invocation.
    ///
    /// The thread-local fast path avoids an atomic operation on every fine-grained cancellation
    /// poll while leaving the wrapped cancellation decision unchanged.
    fn observe(&self) {
        LAST_PROFILE_PARTICIPATION_ID.with(|last| {
            if last.get() == self.id {
                return;
            }
            last.set(self.id);
            if let Some(index) = rayon::current_thread_index() {
                self.worker_registration_count
                    .fetch_add(1, Ordering::Relaxed);
                if index < u64::BITS as usize {
                    self.worker_mask.fetch_or(1_u64 << index, Ordering::Relaxed);
                }
            }
        });
    }

    /// Returns the number of distinct representable worker indices observed in real work.
    fn observed_worker_count(&self) -> usize {
        self.worker_mask.load(Ordering::Relaxed).count_ones() as usize
    }
}

/// Delegates cancellation unchanged while observing worker participation for opt-in profiling.
struct ProfiledCancellation<'a> {
    inner: &'a dyn CancellationProbe,
    participation: &'a ParallelParticipation,
}

impl CancellationProbe for ProfiledCancellation<'_> {
    /// Preserves the underlying cancellation decision after one diagnostic-only observation.
    fn is_cancelled(&self) -> bool {
        self.participation.observe();
        self.inner.is_cancelled()
    }

    /// Forwards document-worker progress without including it in profiling identity.
    fn report_progress(
        &self,
        stage: EvaluationProgressStage,
        completed_per_mille: u16,
        stage_completed_per_mille: u16,
    ) {
        self.inner
            .report_progress(stage, completed_per_mille, stage_completed_per_mille);
    }

    #[cfg(test)]
    /// Forwards test-only stage gates without altering scheduler ordering.
    fn observe_stage(&self, stage: EvaluationStage, checkpoint: EvaluationCheckpoint) {
        self.inner.observe_stage(stage, checkpoint);
    }
}

struct NeverCancelled;

impl CancellationProbe for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

struct AtomicCancellation<'a>(&'a AtomicBool);

impl CancellationProbe for AtomicCancellation<'_> {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Couples scheduler cancellation with ticketed monotonic progress delivery.
struct SchedulerCancellation<'a> {
    cancelled: &'a AtomicBool,
    ticket: EvaluationTicket,
    progress: &'a Sender<EvaluationProgress>,
    last_progress: &'a Mutex<Option<EvaluationProgress>>,
    #[cfg(test)]
    gate: Option<&'a EvaluationStageGate>,
}

impl CancellationProbe for SchedulerCancellation<'_> {
    /// Reads the scheduler-owned cancellation flag without changing progress.
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Publishes nondecreasing, duplicate-coalesced progress while silently tolerating shutdown.
    fn report_progress(
        &self,
        stage: EvaluationProgressStage,
        completed_per_mille: u16,
        stage_completed_per_mille: u16,
    ) {
        let completed = completed_per_mille.min(1_000);
        let stage_completed = stage_completed_per_mille.min(1_000);
        let next = EvaluationProgress {
            ticket: self.ticket,
            stage,
            completed_per_mille: completed,
            stage_completed_per_mille: stage_completed,
        };
        let mut previous = self
            .last_progress
            .lock()
            .expect("scheduler progress lock poisoned");
        if previous.is_some_and(|previous| {
            completed < previous.completed_per_mille
                || (completed == previous.completed_per_mille
                    && stage == previous.stage
                    && stage_completed <= previous.stage_completed_per_mille)
        }) {
            return;
        }
        *previous = Some(next);
        let _ = self.progress.send(next);
    }

    #[cfg(test)]
    /// Preserves deterministic test gates beside production progress reporting.
    fn observe_stage(&self, stage: EvaluationStage, checkpoint: EvaluationCheckpoint) {
        if let Some(gate) = self.gate {
            gate.wait(stage, checkpoint);
        }
    }
}

#[cfg(test)]
struct ObservedCancellation<'a> {
    cancelled: &'a AtomicBool,
    gate: &'a EvaluationStageGate,
}

#[cfg(test)]
impl CancellationProbe for ObservedCancellation<'_> {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn observe_stage(&self, stage: EvaluationStage, checkpoint: EvaluationCheckpoint) {
        self.gate.wait(stage, checkpoint);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EvaluationStage {
    #[cfg(test)]
    Channel,
    Family,
    Decode,
    Realization,
    Scene,
    Raster,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EvaluationCheckpoint {
    Before,
    After,
}

#[cfg(test)]
struct EvaluationStageGateState {
    entered: bool,
    released: bool,
}

#[cfg(test)]
pub(crate) struct EvaluationStageGate {
    stage: EvaluationStage,
    checkpoint: EvaluationCheckpoint,
    entered: std::sync::mpsc::Sender<()>,
    state: std::sync::Mutex<EvaluationStageGateState>,
    wake: std::sync::Condvar,
}

#[cfg(test)]
impl EvaluationStageGate {
    pub(crate) fn new(
        stage: EvaluationStage,
        checkpoint: EvaluationCheckpoint,
    ) -> (std::sync::Arc<Self>, std::sync::mpsc::Receiver<()>) {
        let (entered, receiver) = std::sync::mpsc::channel();
        (
            std::sync::Arc::new(Self {
                stage,
                checkpoint,
                entered,
                state: std::sync::Mutex::new(EvaluationStageGateState {
                    entered: false,
                    released: false,
                }),
                wake: std::sync::Condvar::new(),
            }),
            receiver,
        )
    }

    pub(crate) fn release(&self) {
        let mut state = self
            .state
            .lock()
            .expect("evaluation stage gate lock poisoned");
        state.released = true;
        self.wake.notify_all();
    }

    fn wait(&self, stage: EvaluationStage, checkpoint: EvaluationCheckpoint) {
        if stage != self.stage || checkpoint != self.checkpoint {
            return;
        }
        let mut state = self
            .state
            .lock()
            .expect("evaluation stage gate lock poisoned");
        if state.entered {
            return;
        }
        state.entered = true;
        let _ = self.entered.send(());
        while !state.released {
            state = self
                .wake
                .wait(state)
                .expect("evaluation stage gate lock poisoned");
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EvaluationRunError {
    Cancelled,
    Evaluation(EvaluationError),
}

impl From<EvaluationError> for EvaluationRunError {
    fn from(error: EvaluationError) -> Self {
        Self::Evaluation(error)
    }
}

fn evaluate_stage<T>(
    stage: EvaluationStage,
    cancellation: &dyn CancellationProbe,
    evaluate: impl FnOnce() -> Result<T, EvaluationError>,
) -> Result<T, EvaluationRunError> {
    #[cfg(not(test))]
    let _ = stage;
    if cancellation.is_cancelled() {
        return Err(EvaluationRunError::Cancelled);
    }
    #[cfg(test)]
    cancellation.observe_stage(stage, EvaluationCheckpoint::Before);
    if cancellation.is_cancelled() {
        return Err(EvaluationRunError::Cancelled);
    }
    let result = evaluate();
    #[cfg(test)]
    cancellation.observe_stage(stage, EvaluationCheckpoint::After);
    if cancellation.is_cancelled() {
        return Err(EvaluationRunError::Cancelled);
    }
    result.map_err(EvaluationRunError::Evaluation)
}

/// Evaluates one structural family with cancellation and optional unit progress.
///
/// # Errors
///
/// Returns cancellation or the first family-pipeline diagnostic without
/// exposing partial structural output.
fn evaluate_generic_family_stage(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    source: &SourceField,
    cancellation: &dyn CancellationProbe,
    report_progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<TypedFamilyOutput, EvaluationRunError> {
    match evaluate_stage(EvaluationStage::Family, cancellation, || {
        evaluate_typed_family_product_with_source_progress_cancellable(
            family,
            request,
            family_requires_decoded_source(family).then_some(source),
            &|| cancellation.is_cancelled(),
            report_progress,
        )
        .map_err(EvaluationError::from_pipeline)
    }) {
        Err(EvaluationRunError::Evaluation(error)) if error.path() == "evaluation.cancelled" => {
            Err(EvaluationRunError::Cancelled)
        }
        result => result,
    }
}

/// Evaluates the same ordered diagnostic path as [`evaluate_channel_diagnostic`] with accepted
/// cache reuse and caller-owned cancellation through family, realization, and raster work.
///
/// # Errors
///
/// Returns cancellation or the first stable decode, family, realization, scene, or raster failure
/// without publishing a cache transaction.
fn evaluate_channel_diagnostic_cached_with_cancellation(
    request: ChannelDiagnosticRequest,
    cancellation: &dyn CancellationProbe,
    cache: DerivedCacheSnapshot,
    limits: EvaluationLimits,
) -> Result<CachedEvaluation, EvaluationRunError> {
    let document = request.snapshot.document();
    let channel_id = request.snapshot.token().channel_id();
    let channel = document.channel(channel_id).ok_or(EvaluationError::new(
        "evaluation.channel_id",
        "evaluation targets a missing channel",
    ))?;
    let effective = document
        .effective_channel_pattern(channel_id)
        .map_err(EvaluationError::from_domain)?;
    let source_id = match document.source() {
        SourceReference::Assigned(id) => id,
        SourceReference::Unassigned => {
            return Err(EvaluationError::new(
                "evaluation.source_reference",
                "evaluation requires an assigned source reference",
            )
            .into());
        }
    };
    if source_id != request.source.reference_id() {
        return Err(EvaluationError::new(
            "evaluation.source_reference",
            "resolved source reference does not match the document snapshot",
        )
        .into());
    }
    let definition = document
        .pattern_definition_bundles()
        .iter()
        .find(|definition| definition.id == effective.definition_id)
        .map(|bundle| &bundle.definition)
        .ok_or(EvaluationError::new(
            "evaluation.pattern_definition",
            "channel references a missing pattern definition",
        ))?;
    // Capability resolution is authoritative and happens before decoding or
    // cache lookup, so an unsupported composition cannot publish a partial
    // artifact into a last-successful cache.
    let plan = toniator_patterns::resolve_document_pattern_pipeline(document, definition)
        .map_err(EvaluationError::from_pipeline)?;
    let outputs = ordered_output_bindings(&effective, &plan)?;
    let [(capability, setting)] = outputs.as_slice() else {
        return Err(EvaluationError::new(
            "evaluation.single_channel.composite",
            "diagnostic single-channel evaluation requires exactly one output; complete document evaluation owns composites",
        )
        .into());
    };
    let output = (*capability, *setting);
    let toniator_domain::PatternGeometryResponse::Marks(mark_response) = &output.1.response else {
        return Err(EvaluationError::new(
            "evaluation.stroke.single_channel",
            "guide-path output is available through complete document evaluation only",
        )
        .into());
    };
    let response = MarkResponse {
        minimum_fill: mark_response.minimum_fill,
        maximum_fill: mark_response.maximum_fill,
        rotation_offset_degrees: effective.shape_rotation_degrees,
    };
    let source_key = SourceCacheKey {
        reference_id: request.source.reference_id().as_str().to_owned(),
        bytes: Arc::clone(&request.source.bytes),
        format: request.source.format(),
        decoder_contract: DECODER_CONTRACT_ID,
    };
    // Preflight above remains authoritative. Decode deliberately occurs before
    // the family lookup so decoded-pixel identity participates downstream.
    let (source, source_disposition) =
        evaluate_stage(EvaluationStage::Decode, cancellation, || {
            match &cache.source {
                Some((key, source)) if *key == source_key => {
                    Ok((Arc::clone(source), CacheDisposition::Hit))
                }
                _ => decode_source(request.source.bytes(), request.source.format())
                    .map(|source| (Arc::new(source), CacheDisposition::Miss))
                    .map_err(EvaluationError::from_sampling),
            }
        })?;
    let family_key = FamilyCacheKey {
        canvas: canvas_key(document.canvas()),
        density: (
            effective.density.density.to_bits(),
            effective.density.aspect.to_bits(),
        ),
        rotation: effective.pattern_rotation_degrees.to_bits(),
        translation: (
            effective.translation_x.to_bits(),
            effective.translation_y.to_bits(),
        ),
        guard_steps: definition.coverage.guard_steps,
        definition: FamilyDefinitionKey {
            resolved_guide_content: plan
                .family
                .generic_guides
                .as_ref()
                .map(resolved_guide_identity),
            ..family_definition_key(definition)
        },
        required_support_radius: required_support_radius_legacy(
            document.canvas(),
            &effective,
            definition,
            &plan.family,
        )?
        .to_bits(),
        max_family_candidates: limits.max_family_candidates(),
        structural_source: family_requires_decoded_source(&plan.family)
            .then(|| realization_source_identity(source.identity())),
    };
    let grid = GridInspectRequest {
        canvas: document.canvas().clone(),
        density: effective.resolved_density,
        rotation_degrees: effective.pattern_rotation_degrees,
        translation_x: effective.translation_x,
        translation_y: effective.translation_y,
        guard_steps: definition.coverage.guard_steps,
        support_radius: required_support_radius_legacy(
            document.canvas(),
            &effective,
            definition,
            &plan.family,
        )?,
        max_family_candidates: limits.max_family_candidates(),
    };
    let (family, family_disposition) = match &cache.family {
        Some((key, family)) if family_key_supports(key, &family_key) => {
            (Arc::clone(family), CacheDisposition::Hit)
        }
        _ => (
            Arc::new(evaluate_generic_family_stage(
                &plan.family,
                &grid,
                &source,
                cancellation,
                &|_, _| {},
            )?),
            CacheDisposition::Miss,
        ),
    };
    let realization_key = RealizationCacheKey {
        family: family_key.clone(),
        contract: realization_contract_key(definition),
        source_identity: realization_source_identity(source.identity()),
        canvas: canvas_key(document.canvas()),
        source_component: source_component_key(channel.source_mapping.component),
        placement: placement_key(channel.source_mapping.placement),
        response: (
            response.minimum_fill.to_bits(),
            response.maximum_fill.to_bits(),
            response.rotation_offset_degrees.to_bits(),
        ),
    };
    let (realization, realization_disposition) = evaluate_stage(
        EvaluationStage::Realization,
        cancellation,
        || match &cache.realization {
            Some((key, realization)) if *key == realization_key => {
                Ok((Arc::clone(realization), CacheDisposition::Hit))
            }
            _ => toniator_patterns::realize_typed_diagnostic_outputs(
                &family,
                &plan,
                &source,
                document.canvas(),
                channel.source_mapping.placement,
                channel.source_mapping.component,
                response,
            )
            .map(|realization| (Arc::new(realization), CacheDisposition::Miss))
            .map_err(EvaluationError::from_pipeline),
        },
    )?;
    let scene_key = SceneCacheKey {
        realization: realization_key.clone(),
        canvas: canvas_key(document.canvas()),
        channel_id: channel_id.0,
        visible: channel.appearance.visible,
        color: color_key(&channel.appearance.color),
        opacity: channel.appearance.opacity.to_bits(),
    };
    let (scene, scene_disposition) = evaluate_stage(EvaluationStage::Scene, cancellation, || {
        match &cache.scene {
            Some((key, scene)) if *key == scene_key => {
                Ok((Arc::clone(scene), CacheDisposition::Hit))
            }
            _ => build_scene(SceneBuild {
                canvas: document.canvas().clone(),
                channel_id,
                visible: channel.appearance.visible,
                color: channel.appearance.color.clone(),
                opacity: channel.appearance.opacity,
                family_fingerprint: family.family_fingerprint().to_owned(),
                realization_fingerprint: realization.output.realization_fingerprint.clone(),
                marks: realization.output.marks.clone(),
            })
            .map(|scene| (Arc::new(scene), CacheDisposition::Miss)),
        }
    })?;
    let raster_key = RasterCacheKey {
        scene: scene_key.clone(),
        transparent_raster_contract: TRANSPARENT_RASTER_CONTRACT_ID,
        max_flattened_raster_edges: limits.max_flattened_raster_edges(),
    };
    let (raster, raster_disposition) =
        evaluate_stage(EvaluationStage::Raster, cancellation, || {
            match &cache.raster {
                Some((key, raster)) if *key == raster_key => {
                    Ok((Arc::clone(raster), CacheDisposition::Hit))
                }
                _ => rasterize_cancellable(
                    &scene,
                    RasterBackground::Transparent,
                    toniator_render::RasterizationLimits::new(limits.max_flattened_raster_edges())
                        .expect("EvaluationLimits validates raster edge bounds"),
                    &|| cancellation.is_cancelled(),
                )
                .map(|raster| (Arc::new(raster), CacheDisposition::Miss))
                .map_err(EvaluationError::from_render),
            }
        })?;
    let diagnostics = CacheDiagnostics {
        decoded_source: source_disposition,
        family: family_disposition,
        realization: realization_disposition,
        scene: scene_disposition,
        raster: raster_disposition,
    };
    let transaction = CacheTransaction {
        source: matches!(source_disposition, CacheDisposition::Miss)
            .then(|| (source_key, source.clone())),
        family: matches!(family_disposition, CacheDisposition::Miss)
            .then(|| (family_key, family.clone())),
        realization: matches!(realization_disposition, CacheDisposition::Miss)
            .then(|| (realization_key, realization.clone())),
        scene: matches!(scene_disposition, CacheDisposition::Miss)
            .then(|| (scene_key, scene.clone())),
        raster: matches!(raster_disposition, CacheDisposition::Miss)
            .then(|| (raster_key, raster.clone())),
    };
    Ok(CachedEvaluation {
        result: ChannelDiagnosticResult {
            token: request.snapshot.token(),
            source_identity: source.identity().clone(),
            // The diagnostic result and pending cache transaction share the
            // same immutable render payloads. Accessors continue to project
            // borrows, preserving the public result contract.
            scene: Arc::clone(&scene),
            raster: Arc::clone(&raster),
        },
        diagnostics,
        transaction,
    })
}

struct SceneBuild {
    canvas: CanvasSpec,
    channel_id: ChannelId,
    visible: bool,
    color: toniator_domain::ColorValue,
    opacity: f64,
    family_fingerprint: String,
    realization_fingerprint: String,
    marks: Vec<CanonicalCircleMark>,
}

fn build_scene(input: SceneBuild) -> Result<RenderScene, EvaluationError> {
    RenderScene::new(
        input.canvas,
        input.family_fingerprint,
        input.realization_fingerprint,
        vec![
            RenderLayer::new(
                input.channel_id,
                input.visible,
                input.color,
                input.opacity,
                GeometryOutput::CircularMarks(input.marks),
            )
            .map_err(EvaluationError::from_render)?,
        ],
    )
    .map_err(EvaluationError::from_render)
}

fn canvas_key(canvas: &CanvasSpec) -> (u64, u64) {
    (canvas.width.to_bits(), canvas.height.to_bits())
}

/// Captures immutable authored definition intent before resolved resource content enters a cache key.
///
/// The caller must add document-resolved authored guide content for resource-bearing
/// Stage 20D definitions; this helper alone deliberately cannot cross that cache boundary.
fn family_definition_key(value: &PatternDefinition) -> FamilyDefinitionKey {
    FamilyDefinitionKey {
        definition_id: value.id.0,
        family: value.family.clone(),
        mechanisms: value.mechanisms.clone(),
        resolved_guide_content: None,
        path_offset_algorithm: path_offset_algorithm_key(value),
    }
}

/// Captures the versioned geometry algorithm and fixed limits when a definition uses normal offsets.
fn path_offset_algorithm_key(value: &PatternDefinition) -> Option<PathOffsetAlgorithmKey> {
    let uses_normal_offsets = value.mechanisms.iter().any(|mechanism| match mechanism {
        PatternMechanism::GuideDimensions { dimensions, .. } => {
            dimensions.iter().any(|dimension| {
                matches!(
                    &dimension.repetition,
                    toniator_domain::CurveRepetition::NormalOffset { .. }
                )
            })
        }
        PatternMechanism::ParametricCurveSource { repetition, .. } => matches!(
            repetition,
            toniator_domain::CurveRepetition::NormalOffset { .. }
        ),
        _ => false,
    });
    uses_normal_offsets.then(|| {
        let limits = toniator_patterns::PathOffsetLimits::default();
        PathOffsetAlgorithmKey {
            contract_id: toniator_patterns::PATH_OFFSET_ALGORITHM_CONTRACT_ID,
            maximum_subdivision_depth: limits.maximum_subdivision_depth,
            maximum_segments: limits.maximum_segments,
            maximum_components: limits.maximum_components,
            maximum_cleanup_pairs: limits.maximum_cleanup_pairs,
            maximum_cusp_isolation_work: limits.maximum_cusp_isolation_work,
            tolerance: limits.tolerance.to_bits(),
        }
    })
}

/// Hashes resolved generic guide content in stored order for both engine family-cache paths.
fn resolved_guide_identity(value: &GenericGuideCapability) -> String {
    let mut bytes = b"toniator-stage-20d-resolved-guide-content-v1".to_vec();
    for (source, path) in &value.resolved_paths {
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
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("toniator-stage-20d-guide-family-v1:fnv1a64:{hash:016x}")
}

fn realization_source_identity(value: &SourceIdentity) -> RealizationSourceIdentity {
    RealizationSourceIdentity {
        format: value.format,
        width: value.width,
        height: value.height,
        content_hash: value.content_hash.clone(),
        decoded_pixel_hash: value.decoded_pixel_hash.clone(),
    }
}

/// Captures ordered output and modulation authority for realization-cache discrimination.
fn realization_contract_key(value: &PatternDefinition) -> RealizationContractKey {
    RealizationContractKey {
        output_layers: value.output_layers.clone(),
        modulation: value.modulation.clone(),
    }
}

/// Captures exactly one output's authored realization contract for independent caching.
///
/// A one-output definition intentionally produces the same value as the accepted aggregate helper,
/// preserving legacy `All` cache comparisons while composite painter moves stay presentation-only.
///
/// # Errors
///
/// Returns a stable missing-output diagnostic when the requested cache unit is not authored by
/// the definition.
fn realization_contract_key_for_output(
    value: &PatternDefinition,
    output_layer_id: toniator_domain::PatternOutputLayerId,
) -> Result<RealizationContractKey, EvaluationError> {
    let output = value
        .output_layers
        .iter()
        .find(|output| output.id == output_layer_id)
        .ok_or(EvaluationError::new(
            "realization.site_filter.output",
            "realization cache key targets a missing output",
        ))?;
    Ok(RealizationContractKey {
        output_layers: vec![output.clone()],
        modulation: value.modulation.clone(),
    })
}

/// Hashes resolved authored mark or Curve Motif content so a resource replacement cannot reuse stale output.
///
/// The typed definition retains a stable resource ID, but cache reuse additionally depends on the
/// resource's exact closure, segment kinds, and construction-point bits. Missing or malformed
/// resources fail before a cache candidate can be selected or a transaction can publish.
///
/// # Errors
///
/// Returns stable missing-resource or malformed-geometry diagnostics.
fn resolved_shape_content_identity(
    document: &toniator_domain::Document,
    definition: &PatternDefinition,
    output_layer_id: toniator_domain::PatternOutputLayerId,
) -> Result<String, EvaluationError> {
    let mut bytes = b"toniator-stage-21b-resolved-authored-content-v1".to_vec();
    for output in &definition.output_layers {
        if output.id != output_layer_id {
            continue;
        }
        let structure_id = match &output.realization {
            PatternOutputRealization::MarkPrototype {
                prototype: toniator_domain::MarkPrototype::AuthoredClosedShape { structure_id },
                ..
            }
            | PatternOutputRealization::CurveMotifPaths { structure_id, .. } => *structure_id,
            _ => continue,
        };
        let structure = document
            .authored_structure(structure_id)
            .ok_or(EvaluationError::new(
                "evaluation.authored_output.reference",
                "authored output resource is missing",
            ))?;
        let path = CurvePath::from_authored_structure(structure).map_err(|_| {
            EvaluationError::new(
                "evaluation.authored_output.geometry",
                "authored output resource is not valid curve geometry",
            )
        })?;
        bytes.extend(structure_id.0.to_le_bytes());
        bytes.push(match path.closure() {
            toniator_patterns::PathClosure::Open => 1,
            toniator_patterns::PathClosure::Closed => 2,
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
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Ok(format!(
        "toniator-stage-21b-authored-content-v1:fnv1a64:{hash:016x}"
    ))
}

fn source_component_key(value: SourceComponent) -> u8 {
    match value {
        SourceComponent::Luminance => 1,
        SourceComponent::Alpha => 2,
    }
}

fn placement_key(value: SourcePlacement) -> u8 {
    match value {
        SourcePlacement::StretchToCanvas => 1,
    }
}

fn color_key(color: &toniator_domain::ColorValue) -> (u64, u64, u64, u64) {
    (
        color.red.to_bits(),
        color.green.to_bits(),
        color.blue.to_bits(),
        color.alpha.to_bits(),
    )
}

/// Exposes realization from an already evaluated family so callers can prove
/// exact Stage 3 reuse while varying only the realization response.
pub fn realize_from_existing_family(
    family: &GridFamilyOutput,
    source: &SourceField,
    canvas: &toniator_domain::CanvasSpec,
    placement: SourcePlacement,
    component: SourceComponent,
    response: MarkResponse,
) -> Result<CircularMarkRealization, RealizationError> {
    realize_circular_marks(family, source, canvas, placement, component, response)
}

/// Errors crossing the source, family, and realization boundaries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarksInspectError {
    Grid(GridError),
    Sampling(toniator_sampling::SamplingError),
    Realization(RealizationError),
}

/// Stable failures at the authoritative document-evaluation boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationError {
    path: &'static str,
    message: &'static str,
}

impl EvaluationError {
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }
    pub const fn path(&self) -> &'static str {
        self.path
    }
    pub const fn message(&self) -> &'static str {
        self.message
    }
    fn from_sampling(error: toniator_sampling::SamplingError) -> Self {
        Self::new(error.path(), error.message())
    }
    /// Preserves one domain validation path and message at the evaluation boundary.
    fn from_domain(error: toniator_domain::ValidationError) -> Self {
        Self::new(error.path(), error.message())
    }
    fn from_pipeline(error: PatternPipelineError) -> Self {
        Self::new(error.path(), error.message())
    }
    fn from_render(error: toniator_render::RenderError) -> Self {
        Self::new(error.path(), error.message())
    }
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for EvaluationError {}

impl From<GridError> for MarksInspectError {
    fn from(error: GridError) -> Self {
        Self::Grid(error)
    }
}

impl From<toniator_sampling::SamplingError> for MarksInspectError {
    fn from(error: toniator_sampling::SamplingError) -> Self {
        Self::Sampling(error)
    }
}

impl From<RealizationError> for MarksInspectError {
    fn from(error: RealizationError) -> Self {
        Self::Realization(error)
    }
}

impl fmt::Display for MarksInspectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Grid(error) => error.fmt(formatter),
            Self::Sampling(error) => error.fmt(formatter),
            Self::Realization(error) => error.fmt(formatter),
        }
    }
}

impl Error for MarksInspectError {}

/// Complete-document evaluation request. The retained single-channel request
/// is explicitly diagnostic and cannot enter this authority.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationRequest {
    snapshot: DocumentEvaluationSnapshot,
    source: ResolvedSource,
    preview_target: Option<PreviewRasterTarget>,
}

impl EvaluationRequest {
    pub fn new(snapshot: DocumentEvaluationSnapshot, source: ResolvedSource) -> Self {
        Self {
            snapshot,
            source,
            preview_target: None,
        }
    }

    /// Keeps the authoritative document/scene native while deriving only the
    /// transparent preview raster at this checked output target.
    pub fn with_preview_target(
        snapshot: DocumentEvaluationSnapshot,
        source: ResolvedSource,
        preview_target: PreviewRasterTarget,
    ) -> Self {
        Self {
            snapshot,
            source,
            preview_target: Some(preview_target),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelEvaluationSummary {
    role: HalftoneChannelRole,
    channel_id: ChannelId,
    family_identity: String,
    realization_identity: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One independent output realization cache disposition within a channel.
pub struct OutputCacheDiagnostics {
    pub output_layer_id: toniator_domain::PatternOutputLayerId,
    pub realization: CacheDisposition,
    /// Geometry-owned ordinary-region work facts retained with a cached immutable output unit.
    pub voronoi: Option<VoronoiRegionDiagnostics>,
    /// Complete producer, source, sampling, and treatment facts replayed from a Region cache unit.
    pub region: Option<RegionOutputCacheDiagnostics>,
}

/// Producer diagnostics retained with one independently cached filled-region output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegionProducerCacheDiagnostics {
    /// Stores ordinary Voronoi bounded-work facts from the canonical region producer.
    Voronoi(VoronoiRegionDiagnostics),
    /// Records the analytic Guide-Face producer, whose detailed facts remain geometry-owned.
    GuideFaces,
}

/// Requested source sampling facts replayed with one filled-region output cache hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionSamplingCacheDiagnostics {
    /// Stores the response-selected strategy for normalized source-sampled fill.
    pub strategy: toniator_domain::RegionSamplingStrategy,
    /// Counts complete untreated bases sampled by the typed patterns realizer.
    pub sampled_bases: usize,
}

/// Typed treatment classification retained with bounded retained-region facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionTreatmentCacheKind {
    /// Applies a per-base affine scale.
    Scale,
    /// Applies a per-base signed normal offset to match normalized positive area.
    UniformOffset,
}

/// Treatment facts replayed from one independently cached filled-region output unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionTreatmentCacheDiagnostics {
    /// Identifies the typed treatment without storing an effective derived response table.
    pub kind: RegionTreatmentCacheKind,
    /// Counts canonical treated components retained after collapse and alpha suppression.
    pub retained_regions: usize,
}

/// Complete non-fingerprint diagnostics stored beside one immutable filled-region cache unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionOutputCacheDiagnostics {
    /// Stores the decoded source identity only when the output actually sampled it.
    pub source_identity: Option<SourceIdentity>,
    /// Stores producer bounded-work facts without deriving renderer topology.
    pub producer: RegionProducerCacheDiagnostics,
    /// Stores the response-selected sampling strategy and completed base count.
    pub sampling: RegionSamplingCacheDiagnostics,
    /// Stores treatment classification and retained canonical component count.
    pub treatment: RegionTreatmentCacheDiagnostics,
}

/// Cache dispositions for one evaluated channel and its ordered output units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelCacheDiagnostics {
    pub channel_id: ChannelId,
    pub family: CacheDisposition,
    pub realization: CacheDisposition,
    pub outputs: Vec<OutputCacheDiagnostics>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentCacheDiagnostics {
    pub aggregate: CacheDiagnostics,
    pub channels: Vec<ChannelCacheDiagnostics>,
}

/// Coarse authoritative compute boundary recorded by opt-in evaluation profiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluationPerformanceStage {
    Preflight,
    SourceDecode,
    Family,
    DependencyFilter,
    MarkRealization,
    StructuralPathRealization,
    ConnectionRealization,
    MazeRealization,
    VoronoiRealization,
    GuideFaceRealization,
    RegionSampling,
    RegionTreatment,
    Scene,
    Raster,
    Total,
}

/// Diagnostic classification of whether one timed boundary computed, replayed, or reused work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluationExecutionClass {
    Computed,
    AcceptedCacheHit,
    LocalReuse,
}

/// Stable workload category paired with one inexpensive deterministic count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluationWorkloadKind {
    SourcePixels,
    OutputLayers,
    FamilySites,
    StructuralPaths,
    RetainedSites,
    Marks,
    Strokes,
    StrokeProfileSamples,
    StrokeOutlineSegments,
    Regions,
    RegionBoundarySegments,
    AreaAverageFlattenedSegments,
    AreaAverageCellIntersections,
    RasterPixels,
    MarkOutput,
    StructuralPathOutput,
    ConnectionOutput,
    MazeOutput,
    VoronoiOutput,
    GuideFaceOutput,
}

/// One deterministic workload count attached to a performance stage record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationWorkloadMetric {
    pub kind: EvaluationWorkloadKind,
    pub count: usize,
}

/// Diagnostic-only timing for one scoped architectural compute boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationPerformanceRecord {
    pub stage: EvaluationPerformanceStage,
    pub channel_id: Option<ChannelId>,
    pub output_layer_id: Option<toniator_domain::PatternOutputLayerId>,
    pub elapsed: Duration,
    pub cache: Option<CacheDisposition>,
    pub execution: EvaluationExecutionClass,
    pub workloads: Vec<EvaluationWorkloadMetric>,
}

/// Complete opt-in timing/workload report from one authoritative evaluation invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationPerformanceMetrics {
    /// Reports configured shared Rayon pool capacity; workload/CPU evidence establishes use.
    pub configured_worker_count: usize,
    /// Counts distinct Rayon worker indices that executed cancellation-polled evaluation work.
    pub observed_worker_count: usize,
    /// Counts first-poll worker registrations, avoiding per-poll atomic diagnostic overhead.
    pub worker_registration_count: usize,
    /// Retains records in deterministic coordinator traversal order, never worker completion order.
    pub records: Vec<EvaluationPerformanceRecord>,
}

/// One complete evaluation plus cache and diagnostic-only performance facts.
pub struct ProfiledEvaluation {
    pub result: EvaluationResult,
    pub diagnostics: DocumentCacheDiagnostics,
    pub performance: EvaluationPerformanceMetrics,
}

/// Stateful diagnostic cache for cold, warm, and targeted-invalidation performance comparisons.
///
/// The wrapper exposes no derived entries or mutation authority. Successful profiled evaluations
/// publish their ordinary evaluator transaction; failures leave the previously accepted state
/// unchanged.
#[derive(Clone, Default)]
pub struct EvaluationProfileCache {
    derived: DocumentDerivedCache,
}

/// Private collector that never enters cache keys, transactions, persistence, or result identity.
struct EvaluationPerformanceBuilder {
    records: Vec<EvaluationPerformanceRecord>,
}

impl EvaluationPerformanceBuilder {
    /// Starts an empty current-run record collection.
    fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Appends one coordinator-ordered coarse record after its boundary completes.
    fn record(
        &mut self,
        stage: EvaluationPerformanceStage,
        channel_id: Option<ChannelId>,
        output_layer_id: Option<toniator_domain::PatternOutputLayerId>,
        elapsed: Duration,
        cache: Option<CacheDisposition>,
        workloads: Vec<EvaluationWorkloadMetric>,
    ) {
        let execution = match cache {
            Some(CacheDisposition::Hit) => EvaluationExecutionClass::AcceptedCacheHit,
            Some(CacheDisposition::Miss) | None => EvaluationExecutionClass::Computed,
        };
        self.record_with_execution(
            stage,
            channel_id,
            output_layer_id,
            elapsed,
            cache,
            execution,
            workloads,
        );
    }

    /// Appends one record whose execution class differs from its aggregate cache disposition.
    #[allow(clippy::too_many_arguments)]
    fn record_with_execution(
        &mut self,
        stage: EvaluationPerformanceStage,
        channel_id: Option<ChannelId>,
        output_layer_id: Option<toniator_domain::PatternOutputLayerId>,
        elapsed: Duration,
        cache: Option<CacheDisposition>,
        execution: EvaluationExecutionClass,
        workloads: Vec<EvaluationWorkloadMetric>,
    ) {
        self.records.push(EvaluationPerformanceRecord {
            stage,
            channel_id,
            output_layer_id,
            elapsed,
            cache,
            execution,
            workloads,
        });
    }

    /// Finishes the report with configured and actually observed worker participation.
    fn finish(self, participation: &ParallelParticipation) -> EvaluationPerformanceMetrics {
        EvaluationPerformanceMetrics {
            configured_worker_count: rayon::current_num_threads(),
            observed_worker_count: participation.observed_worker_count(),
            worker_registration_count: participation
                .worker_registration_count
                .load(Ordering::Relaxed),
            records: self.records,
        }
    }
}
impl ChannelEvaluationSummary {
    pub const fn role(&self) -> HalftoneChannelRole {
        self.role
    }
    pub const fn channel_id(&self) -> ChannelId {
        self.channel_id
    }
    pub fn family_identity(&self) -> &str {
        &self.family_identity
    }
    pub fn realization_identity(&self) -> &str {
        &self.realization_identity
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationResult {
    token: DocumentEvaluationToken,
    source_identity: SourceIdentity,
    channels: Vec<ChannelEvaluationSummary>,
    scene: Arc<RenderScene>,
    raster: Arc<RasterSurface>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationTicket(u64);
impl EvaluationTicket {
    /// Returns the scheduler-local monotonic ticket value.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Identifies one artist-visible complete-document evaluation phase.
///
/// The variants describe coordinator work rather than renderer internals and
/// never enter document, cache, scene, or export identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluationProgressStage {
    Preparing,
    DecodingSource,
    GeneratingGeometry,
    RealizingOutputs,
    ComposingScene,
    RasterizingPreview,
    Finalizing,
    Complete,
}

/// Carries monotonic ticketed progress from a document worker.
///
/// Per-mille completion permits deterministic fixed stage weights without
/// making floating-point values part of scheduler equality or identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationProgress {
    ticket: EvaluationTicket,
    stage: EvaluationProgressStage,
    completed_per_mille: u16,
    stage_completed_per_mille: u16,
}

impl EvaluationProgress {
    /// Returns the scheduler ticket whose worker emitted this update.
    pub const fn ticket(self) -> EvaluationTicket {
        self.ticket
    }

    /// Returns the current coarse evaluation phase.
    pub const fn stage(self) -> EvaluationProgressStage {
        self.stage
    }

    /// Returns determinate completion in the inclusive `0.0..=1.0` range.
    pub fn fraction(self) -> f64 {
        f64::from(self.completed_per_mille) / 1_000.0
    }

    /// Returns completion within the currently named stage in the inclusive `0.0..=1.0` range.
    pub fn stage_fraction(self) -> f64 {
        f64::from(self.stage_completed_per_mille) / 1_000.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EvaluationCompletion {
    Completed {
        ticket: EvaluationTicket,
        result: Box<EvaluationResult>,
        diagnostics: DocumentCacheDiagnostics,
    },
    Failed {
        ticket: EvaluationTicket,
        token: DocumentEvaluationToken,
        error: EvaluationError,
    },
}
impl EvaluationCompletion {
    pub const fn ticket(&self) -> EvaluationTicket {
        match self {
            Self::Completed { ticket, .. } | Self::Failed { ticket, .. } => *ticket,
        }
    }
    pub fn token(&self) -> DocumentEvaluationToken {
        match self {
            Self::Completed { result, .. } => result.token(),
            Self::Failed { token, .. } => *token,
        }
    }
    pub fn result(&self) -> Option<&EvaluationResult> {
        match self {
            Self::Completed { result, .. } => Some(result),
            Self::Failed { .. } => None,
        }
    }
    pub fn error(&self) -> Option<&EvaluationError> {
        match self {
            Self::Completed { .. } => None,
            Self::Failed { error, .. } => Some(error),
        }
    }
    pub fn cache_diagnostics(&self) -> Option<&DocumentCacheDiagnostics> {
        match self {
            Self::Completed { diagnostics, .. } => Some(diagnostics),
            Self::Failed { .. } => None,
        }
    }
}

/// Complete-document scheduler state. The diagnostic scheduler remains a
/// separate retained API; this cache is never writable authority.
struct DocumentJob {
    ticket: EvaluationTicket,
    request: EvaluationRequest,
    cancelled: Arc<AtomicBool>,
    cache: DocumentDerivedCache,
    #[cfg(test)]
    decode_observer: Option<Arc<AtomicUsize>>,
}
struct DocumentWorkerCompletion {
    completion: EvaluationCompletion,
    transaction: Option<DocumentCacheTransaction>,
}
struct DocumentSchedulerState {
    next_ticket: Option<u64>,
    sender: Option<Sender<DocumentJob>>,
    worker: Option<JoinHandle<()>>,
    latest_cancellation: Option<Arc<AtomicBool>>,
    latest_ticket: Option<EvaluationTicket>,
    cache: DocumentDerivedCache,
    pending: Option<(EvaluationTicket, DocumentCacheTransaction)>,
    accepted: Option<EvaluationTicket>,
    #[cfg(test)]
    decode_observer: Option<Arc<AtomicUsize>>,
}
pub struct EvaluationScheduler {
    state: Mutex<DocumentSchedulerState>,
    latest_ticket: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
    completions: Mutex<Receiver<DocumentWorkerCompletion>>,
    progress: Mutex<Receiver<EvaluationProgress>>,
    publication: Arc<Mutex<()>>,
}
impl EvaluationScheduler {
    pub fn new() -> Result<Self, SchedulerError> {
        Self::new_with_limits(EvaluationLimits::default())
    }

    pub fn new_with_limits(limits: EvaluationLimits) -> Result<Self, SchedulerError> {
        #[cfg(test)]
        return Self::new_with_limits_and_gate(limits, None, None);
        #[cfg(not(test))]
        Self::new_with_limits_and_gate(limits)
    }

    #[cfg(test)]
    pub(crate) fn new_with_test_gate(
        limits: EvaluationLimits,
        gate: Arc<EvaluationStageGate>,
    ) -> Result<Self, SchedulerError> {
        Self::new_with_limits_and_gate(limits, Some(gate), None)
    }

    #[cfg(test)]
    fn new_with_test_decode_observer(
        limits: EvaluationLimits,
        decode_observer: Arc<AtomicUsize>,
    ) -> Result<Self, SchedulerError> {
        Self::new_with_limits_and_gate(limits, None, Some(decode_observer))
    }

    /// Starts one reusable worker with bounded evaluation limits and atomic completion publication.
    ///
    /// Test-only probes observe stage/cancellation boundaries without changing production authority.
    ///
    /// # Errors
    /// Returns `WorkerSpawn` when the operating system cannot start the evaluation thread.
    fn new_with_limits_and_gate(
        limits: EvaluationLimits,
        #[cfg(test)] gate: Option<Arc<EvaluationStageGate>>,
        #[cfg(test)] decode_observer: Option<Arc<AtomicUsize>>,
    ) -> Result<Self, SchedulerError> {
        let (sender, receiver) = mpsc::channel::<DocumentJob>();
        let (completion_sender, completions) = mpsc::channel::<DocumentWorkerCompletion>();
        let (progress_sender, progress) = mpsc::channel::<EvaluationProgress>();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = Arc::clone(&shutdown);
        let publication = Arc::new(Mutex::new(()));
        let worker_publication = Arc::clone(&publication);
        #[cfg(test)]
        let worker_gate = gate;
        let worker = thread::Builder::new()
            .name("toniator-document-evaluation".into())
            .spawn(move || {
                while !worker_shutdown.load(Ordering::Acquire) {
                    let mut job = match receiver.recv() {
                        Ok(job) => job,
                        Err(_) => break,
                    };
                    while let Ok(next) = receiver.try_recv() {
                        job.cancelled.store(true, Ordering::Release);
                        job = next;
                    }
                    if job.cancelled.load(Ordering::Acquire) {
                        continue;
                    }
                    let token = job.request.snapshot.token();
                    let last_progress = Mutex::new(None);
                    let worker_cancellation = SchedulerCancellation {
                        cancelled: &job.cancelled,
                        ticket: job.ticket,
                        progress: &progress_sender,
                        last_progress: &last_progress,
                        #[cfg(test)]
                        gate: worker_gate.as_deref(),
                    };
                    #[cfg(test)]
                    let outcome = evaluate_cached_document_with_test_observer(
                        job.request,
                        limits,
                        &job.cache,
                        &worker_cancellation,
                        job.decode_observer.as_deref(),
                    );
                    #[cfg(not(test))]
                    let outcome = evaluate_cached_document(
                        job.request,
                        limits,
                        &job.cache,
                        &worker_cancellation,
                    );
                    if job.cancelled.load(Ordering::Acquire) {
                        continue;
                    }
                    let completion = match outcome {
                        Ok(value) => DocumentWorkerCompletion {
                            completion: EvaluationCompletion::Completed {
                                ticket: job.ticket,
                                result: Box::new(value.result),
                                diagnostics: value.diagnostics,
                            },
                            transaction: Some(value.transaction),
                        },
                        Err(EvaluationRunError::Evaluation(error)) => DocumentWorkerCompletion {
                            completion: EvaluationCompletion::Failed {
                                ticket: job.ticket,
                                token,
                                error,
                            },
                            transaction: None,
                        },
                        Err(EvaluationRunError::Cancelled) => continue,
                    };
                    let _publication = worker_publication
                        .lock()
                        .expect("document scheduler publication lock poisoned");
                    if !job.cancelled.load(Ordering::Acquire) {
                        worker_cancellation.report_progress(
                            EvaluationProgressStage::Complete,
                            1_000,
                            1_000,
                        );
                        let _ = completion_sender.send(completion);
                    }
                }
            })
            .map_err(|_| SchedulerError::WorkerSpawn)?;
        Ok(Self {
            state: Mutex::new(DocumentSchedulerState {
                next_ticket: Some(1),
                sender: Some(sender),
                worker: Some(worker),
                latest_cancellation: None,
                latest_ticket: None,
                cache: DocumentDerivedCache::default(),
                pending: None,
                accepted: None,
                #[cfg(test)]
                decode_observer,
            }),
            latest_ticket: Arc::new(AtomicU64::new(0)),
            shutdown,
            completions: Mutex::new(completions),
            progress: Mutex::new(progress),
            publication,
        })
    }

    pub fn submit(&self, request: EvaluationRequest) -> Result<EvaluationTicket, SchedulerError> {
        let mut state = self
            .state
            .lock()
            .expect("document scheduler state lock poisoned");
        if self.shutdown.load(Ordering::Acquire) {
            return Err(SchedulerError::WorkerUnavailable);
        }
        let value = state
            .next_ticket
            .take()
            .ok_or(SchedulerError::TicketExhausted)?;
        state.next_ticket = value.checked_add(1);
        if let Some(cancelled) = &state.latest_cancellation {
            cancelled.store(true, Ordering::Release);
        }
        state.pending = None;
        state.accepted = None;
        let ticket = EvaluationTicket(value);
        let cancelled = Arc::new(AtomicBool::new(false));
        let cache = state.cache.snapshot();
        #[cfg(test)]
        let decode_observer = state.decode_observer.clone();
        state.latest_cancellation = Some(Arc::clone(&cancelled));
        state.latest_ticket = Some(ticket);
        self.latest_ticket.store(ticket.value(), Ordering::Release);
        state
            .sender
            .as_ref()
            .ok_or(SchedulerError::WorkerUnavailable)?
            .send(DocumentJob {
                ticket,
                request,
                cancelled,
                cache,
                #[cfg(test)]
                decode_observer,
            })
            .map_err(|_| SchedulerError::WorkerUnavailable)?;
        Ok(ticket)
    }

    /// Cancels private work and releases accepted/pending caches while retaining the worker.
    ///
    /// Ticket allocation remains monotonic. A publication gate excludes cancellation-racing
    /// completed payloads while queues are drained after releasing the state mutex. Queued work
    /// becomes stale; an already running job releases its snapshot after observing cancellation.
    /// This resets derived state only and never mutates a caller's document or session.
    ///
    /// # Panics
    ///
    /// Panics if the scheduler state mutex is poisoned.
    pub fn cancel_and_clear(&self) {
        let _publication = self
            .publication
            .lock()
            .expect("document scheduler publication lock poisoned");
        let mut state = self
            .state
            .lock()
            .expect("document scheduler state lock poisoned");
        if let Some(cancelled) = state.latest_cancellation.take() {
            cancelled.store(true, Ordering::Release);
        }
        state.latest_ticket = None;
        self.latest_ticket.store(0, Ordering::Release);
        state.pending = None;
        state.accepted = None;
        state.cache = DocumentDerivedCache::default();
        drop(state);
        // Receive paths lock receiver before state. Never retain state while draining.
        let completions = self
            .completions
            .lock()
            .expect("document scheduler completion lock poisoned");
        while completions.try_recv().is_ok() {}
        let progress = self
            .progress
            .lock()
            .expect("document scheduler progress lock poisoned");
        while progress.try_recv().is_ok() {}
    }

    pub fn is_latest(&self, ticket: EvaluationTicket) -> bool {
        !self.shutdown.load(Ordering::Acquire)
            && self
                .state
                .lock()
                .expect("document scheduler state lock poisoned")
                .latest_ticket
                == Some(ticket)
    }

    #[cfg(test)]
    fn set_next_ticket_for_test(&self, next_ticket: Option<u64>) {
        self.state
            .lock()
            .expect("document scheduler state lock poisoned")
            .next_ticket = next_ticket;
    }

    /// Returns the next queued progress update for the latest ticket.
    ///
    /// Stale or cancelled ticket updates are discarded at the scheduler boundary,
    /// before a frontend can project them. Progress never commits cache state.
    ///
    /// # Errors
    ///
    /// Returns `WorkerUnavailable` if the progress channel disconnects while
    /// the scheduler remains active.
    pub fn try_receive_latest_progress(
        &self,
    ) -> Result<Option<EvaluationProgress>, SchedulerError> {
        let receiver = self
            .progress
            .lock()
            .expect("document scheduler progress lock poisoned");
        loop {
            match receiver.try_recv() {
                Ok(progress) => {
                    if !self.shutdown.load(Ordering::Acquire)
                        && self
                            .state
                            .lock()
                            .expect("document scheduler state lock poisoned")
                            .latest_ticket
                            == Some(progress.ticket())
                    {
                        return Ok(Some(progress));
                    }
                }
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Disconnected) => {
                    return if self.shutdown.load(Ordering::Acquire) {
                        Ok(None)
                    } else {
                        Err(SchedulerError::WorkerUnavailable)
                    };
                }
            }
        }
    }

    /// Drains worker completions and returns only the latest ticket's atomic candidate.
    pub fn try_receive_latest(&self) -> Result<Option<EvaluationCompletion>, SchedulerError> {
        let receiver = self
            .completions
            .lock()
            .expect("document scheduler completion lock poisoned");
        let mut current = None;
        loop {
            match receiver.try_recv() {
                Ok(worker) => {
                    let mut state = self
                        .state
                        .lock()
                        .expect("document scheduler state lock poisoned");
                    if !self.shutdown.load(Ordering::Acquire)
                        && state.latest_ticket == Some(worker.completion.ticket())
                    {
                        if let Some(transaction) = worker.transaction {
                            state.pending = Some((worker.completion.ticket(), transaction));
                        }
                        current = Some(worker.completion);
                    }
                }
                Err(TryRecvError::Empty) => return Ok(current),
                Err(TryRecvError::Disconnected) => {
                    return if self.shutdown.load(Ordering::Acquire) {
                        Ok(current)
                    } else {
                        Err(SchedulerError::WorkerUnavailable)
                    };
                }
            }
        }
    }

    pub fn accept_completion(
        &self,
        completion: &EvaluationCompletion,
        session: &DocumentSession,
    ) -> Result<bool, SchedulerError> {
        let mut state = self
            .state
            .lock()
            .expect("document scheduler state lock poisoned");
        if self.shutdown.load(Ordering::Acquire)
            || state.latest_ticket != Some(completion.ticket())
            || !session.accepts_document_evaluation(completion.token())
        {
            return Ok(false);
        }
        if state.accepted == Some(completion.ticket()) {
            return Ok(true);
        }
        if completion.result().is_some()
            && let Some((ticket, transaction)) = state.pending.take()
            && ticket == completion.ticket()
        {
            state.cache.commit(transaction);
        }
        state.accepted = Some(completion.ticket());
        Ok(true)
    }

    pub fn shutdown(mut self) -> Result<(), SchedulerError> {
        self.stop_worker()
    }

    fn stop_worker(&mut self) -> Result<(), SchedulerError> {
        let worker = {
            let mut state = self
                .state
                .lock()
                .expect("document scheduler state lock poisoned");
            self.shutdown.store(true, Ordering::Release);
            if let Some(cancelled) = &state.latest_cancellation {
                cancelled.store(true, Ordering::Release);
            }
            state.sender.take();
            state.worker.take()
        };
        if let Some(worker) = worker {
            worker.join().map_err(|_| SchedulerError::WorkerPanicked)?;
        }
        Ok(())
    }
}
impl Drop for EvaluationScheduler {
    fn drop(&mut self) {
        let _ = self.stop_worker();
    }
}
impl EvaluationResult {
    pub const fn token(&self) -> DocumentEvaluationToken {
        self.token
    }
    pub fn source_identity(&self) -> &SourceIdentity {
        &self.source_identity
    }
    pub fn channels(&self) -> &[ChannelEvaluationSummary] {
        &self.channels
    }
    pub fn scene(&self) -> &RenderScene {
        self.scene.as_ref()
    }
    pub fn raster(&self) -> &RasterSurface {
        self.raster.as_ref()
    }
}

/// Evaluates all authoritative topology channels in stable order. A failing
/// channel aborts before any scene/raster is returned.
pub fn evaluate(request: EvaluationRequest) -> Result<EvaluationResult, EvaluationError> {
    evaluate_with_limits(request, EvaluationLimits::default())
}

pub fn evaluate_with_limits(
    request: EvaluationRequest,
    limits: EvaluationLimits,
) -> Result<EvaluationResult, EvaluationError> {
    evaluate_cached_document(
        request,
        limits,
        &DocumentDerivedCache::default(),
        &NeverCancelled,
    )
    .map(|value| value.result)
    .map_err(|error| match error {
        EvaluationRunError::Evaluation(error) => error,
        EvaluationRunError::Cancelled => unreachable!("synchronous evaluation never cancels"),
    })
}

/// Evaluates one complete document while collecting diagnostic-only architectural metrics.
///
/// The call uses the ordinary evaluator with a fresh private cache, preserving the same authority,
/// deterministic outputs, cancellation semantics, and transactional candidate construction.
/// Metrics never enter the returned result identity, cache keys, persistence, or scheduler state.
///
/// # Errors
///
/// Returns the same validation, source, family, realization, scene, or raster error as
/// [`evaluate_with_limits`] without returning a partial performance report.
pub fn evaluate_profiled_with_limits(
    request: EvaluationRequest,
    limits: EvaluationLimits,
) -> Result<ProfiledEvaluation, EvaluationError> {
    evaluate_profiled_cached_with_limits(request, limits, &mut EvaluationProfileCache::default())
}

/// Profiles one evaluation against caller-retained accepted derived state.
///
/// This is a diagnostic seam for cold-to-warm and targeted-edit comparisons. It delegates all
/// semantic work to the ordinary evaluator and commits cache candidates only after complete
/// success; neither timing nor participation facts enter cache keys or product results.
///
/// # Errors
///
/// Returns the same stable evaluation error as [`evaluate_with_limits`] and leaves `cache`
/// unchanged on failure.
pub fn evaluate_profiled_cached_with_limits(
    request: EvaluationRequest,
    limits: EvaluationLimits,
    cache: &mut EvaluationProfileCache,
) -> Result<ProfiledEvaluation, EvaluationError> {
    let started = Instant::now();
    let mut performance = EvaluationPerformanceBuilder::new();
    let participation = ParallelParticipation::default();
    let cancellation = ProfiledCancellation {
        inner: &NeverCancelled,
        participation: &participation,
    };
    let evaluation = evaluate_cached_document_profiled(
        request,
        limits,
        &cache.derived,
        &cancellation,
        &mut performance,
    )
    .map_err(|error| match error {
        EvaluationRunError::Evaluation(error) => error,
        EvaluationRunError::Cancelled => {
            unreachable!("synchronous evaluation never cancels")
        }
    })?;
    let CachedDocumentEvaluation {
        result,
        diagnostics,
        transaction,
    } = evaluation;
    cache.derived.commit(transaction);
    performance.record(
        EvaluationPerformanceStage::Total,
        None,
        None,
        started.elapsed(),
        None,
        Vec::new(),
    );
    Ok(ProfiledEvaluation {
        result,
        diagnostics,
        performance: performance.finish(&participation),
    })
}

/// Routes profiled synchronous evaluation through the same implementation in all builds.
fn evaluate_cached_document_profiled(
    request: EvaluationRequest,
    limits: EvaluationLimits,
    accepted: &DocumentDerivedCache,
    cancellation: &dyn CancellationProbe,
    performance: &mut EvaluationPerformanceBuilder,
) -> Result<CachedDocumentEvaluation, EvaluationRunError> {
    #[cfg(test)]
    {
        evaluate_cached_document_impl(
            request,
            limits,
            accepted,
            cancellation,
            Some(performance),
            None,
        )
    }
    #[cfg(not(test))]
    evaluate_cached_document_impl(request, limits, accepted, cancellation, Some(performance))
}

// Holds an engine-test-only Region snapshot for the current synchronous evaluation thread. This
// storage is never serialized, cached, projected as capability data, or present in production.
#[cfg(test)]
thread_local! {
    static REGION_EVALUATION_EVIDENCE_ENABLED: Cell<bool> = const { Cell::new(false) };
    static REGION_EVALUATION_EVIDENCE: RefCell<Option<RegionEvaluationEvidence>> = const { RefCell::new(None) };
}

/// Evaluates one document while atomically collecting a Region snapshot from its actual realization.
///
/// A failing or cancelled invocation clears the thread-local slot, so callers never receive a
/// partial snapshot. Callers provide a fresh cache when they require a cache-miss observation.
#[cfg(test)]
fn evaluate_cached_document_with_region_evidence(
    request: EvaluationRequest,
    limits: EvaluationLimits,
    accepted: &DocumentDerivedCache,
    cancellation: &dyn CancellationProbe,
) -> (
    Result<CachedDocumentEvaluation, EvaluationRunError>,
    Option<RegionEvaluationEvidence>,
) {
    REGION_EVALUATION_EVIDENCE.with(|slot| *slot.borrow_mut() = None);
    REGION_EVALUATION_EVIDENCE_ENABLED.with(|enabled| enabled.set(true));
    let outcome = evaluate_cached_document(request, limits, accepted, cancellation);
    REGION_EVALUATION_EVIDENCE_ENABLED.with(|enabled| enabled.set(false));
    let evidence = outcome
        .is_ok()
        .then(|| REGION_EVALUATION_EVIDENCE.with(|slot| slot.borrow_mut().take()))
        .flatten();
    if outcome.is_err() {
        REGION_EVALUATION_EVIDENCE.with(|slot| *slot.borrow_mut() = None);
    }
    (outcome, evidence)
}

/// Returns whether the current engine test requests one Region realization observation.
#[cfg(test)]
fn region_evaluation_evidence_enabled() -> bool {
    REGION_EVALUATION_EVIDENCE_ENABLED.with(Cell::get)
}

/// Stores one completed test-only Region snapshot after the typed realizer has succeeded.
#[cfg(test)]
fn record_region_evaluation_evidence(evidence: RegionEvaluationEvidence) {
    REGION_EVALUATION_EVIDENCE.with(|slot| *slot.borrow_mut() = Some(evidence));
}

fn evaluate_cached_document(
    request: EvaluationRequest,
    limits: EvaluationLimits,
    accepted: &DocumentDerivedCache,
    cancellation: &dyn CancellationProbe,
) -> Result<CachedDocumentEvaluation, EvaluationRunError> {
    #[cfg(test)]
    {
        evaluate_cached_document_with_test_observer(request, limits, accepted, cancellation, None)
    }
    #[cfg(not(test))]
    evaluate_cached_document_impl(request, limits, accepted, cancellation, None)
}

#[cfg(test)]
fn evaluate_cached_document_with_test_observer(
    request: EvaluationRequest,
    limits: EvaluationLimits,
    accepted: &DocumentDerivedCache,
    cancellation: &dyn CancellationProbe,
    decode_observer: Option<&AtomicUsize>,
) -> Result<CachedDocumentEvaluation, EvaluationRunError> {
    evaluate_cached_document_impl(
        request,
        limits,
        accepted,
        cancellation,
        None,
        decode_observer,
    )
}

/// Computes the fixed-weight coordinator progress for completed family/output units.
///
/// Source preparation owns the first 200 per-mille, channel family generation
/// owns 250, and ordered output realization owns 450. Zero totals are treated
/// as already complete so malformed work cannot divide by zero; document
/// preflight remains responsible for rejecting invalid topology.
///
/// # Panics
///
/// Cannot panic: the fixed weights sum to at most 900, which fits in `u16`.
fn document_work_progress(
    completed_families: usize,
    total_families: usize,
    completed_outputs: usize,
    total_outputs: usize,
) -> u16 {
    let weighted = |completed: usize, total: usize, weight: u128| {
        if total == 0 {
            weight
        } else {
            weight * completed.min(total) as u128 / total as u128
        }
    };
    let value = 200_u128
        + weighted(completed_families, total_families, 250)
        + weighted(completed_outputs, total_outputs, 450);
    u16::try_from(value.min(900)).expect("document progress is at most 900 per-mille")
}

/// Resolves unit progress inside the current family into its fixed document share.
///
/// Completed output work is retained because document evaluation interleaves
/// family generation and ordered realization by channel. Invalid zero totals
/// contribute no partial-family work and cannot divide by zero.
///
/// # Panics
///
/// Cannot panic: the fixed preparation, family, and output weights sum to at
/// most 900 per-mille.
fn document_family_work_progress(
    completed_families: usize,
    total_families: usize,
    completed_outputs: usize,
    total_outputs: usize,
    completed_units: usize,
    total_units: usize,
) -> u16 {
    let output = if total_outputs == 0 {
        450_u128
    } else {
        450_u128 * completed_outputs.min(total_outputs) as u128 / total_outputs as u128
    };
    let family = if total_families == 0 {
        250_u128
    } else {
        let local_total = total_units.max(1) as u128;
        let local_completed = completed_units.min(total_units) as u128;
        let numerator = (completed_families.min(total_families) as u128)
            .saturating_mul(local_total)
            .saturating_add(local_completed);
        let denominator = (total_families as u128).saturating_mul(local_total);
        250_u128 * numerator.min(denominator) / denominator
    };
    let value = 200_u128 + family + output;
    u16::try_from(value.min(900)).expect("document progress is at most 900 per-mille")
}

/// Resolves site-worker progress inside the current output into its fixed document share.
///
/// Cancellation polls are issued at deterministic unit boundaries throughout
/// realization, including Rayon workers. The caller supplies the family-site
/// count as the determinate unit total; excess nested polls are clamped and the
/// coordinator completes the output only after atomic realization succeeds.
///
/// # Panics
///
/// Cannot panic: the fixed preparation, family, and output weights sum to at
/// most 900 per-mille.
fn document_output_work_progress(
    completed_families: usize,
    total_families: usize,
    completed_outputs: usize,
    total_outputs: usize,
    completed_units: usize,
    total_units: usize,
) -> u16 {
    let family = if total_families == 0 {
        250_u128
    } else {
        250_u128 * completed_families.min(total_families) as u128 / total_families as u128
    };
    let output = if total_outputs == 0 {
        450_u128
    } else {
        let local_total = total_units.max(1) as u128;
        let local_completed = completed_units.min(total_units) as u128;
        let numerator = (completed_outputs.min(total_outputs) as u128)
            .saturating_mul(local_total)
            .saturating_add(local_completed);
        let denominator = (total_outputs as u128).saturating_mul(local_total);
        450_u128 * numerator.min(denominator) / denominator
    };
    let value = 200_u128 + family + output;
    u16::try_from(value.min(900)).expect("document progress is at most 900 per-mille")
}

/// Resolves completed family groups plus current-family work into local stage progress.
fn family_stage_progress(
    completed_families: usize,
    total_families: usize,
    completed_units: usize,
    total_units: usize,
) -> u16 {
    grouped_stage_progress(
        completed_families,
        total_families,
        completed_units,
        total_units,
    )
}

/// Resolves completed output groups plus current-output work into local stage progress.
fn output_stage_progress(
    completed_outputs: usize,
    total_outputs: usize,
    completed_units: usize,
    total_units: usize,
) -> u16 {
    grouped_stage_progress(
        completed_outputs,
        total_outputs,
        completed_units,
        total_units,
    )
}

/// Maps grouped completed/current work into the inclusive local per-mille range.
fn grouped_stage_progress(
    completed_groups: usize,
    total_groups: usize,
    completed_units: usize,
    total_units: usize,
) -> u16 {
    if total_groups == 0 {
        return 1_000;
    }
    let local_total = total_units.max(1) as u128;
    let numerator = (completed_groups.min(total_groups) as u128)
        .saturating_mul(local_total)
        .saturating_add(completed_units.min(total_units) as u128);
    let denominator = (total_groups as u128).saturating_mul(local_total);
    let value = 1_000_u128 * numerator.min(denominator) / denominator;
    u16::try_from(value).expect("local stage progress is at most 1,000 per-mille")
}

/// Maps one stage's completed units into the inclusive local per-mille range.
fn unit_stage_progress(completed_units: usize, total_units: usize) -> u16 {
    if total_units == 0 {
        return 1_000;
    }
    let value = 1_000_u128 * completed_units.min(total_units) as u128 / total_units as u128;
    u16::try_from(value).expect("local unit progress is at most 1,000 per-mille")
}

/// Maps completed raster primitives and parallel phases into the fixed four-percent share.
///
/// Empty work begins at the raster-stage boundary and completion is finalized
/// by the coordinator's following publication stage.
fn raster_work_progress(completed_units: usize, total_units: usize) -> u16 {
    if total_units == 0 {
        return 950;
    }
    let completed = completed_units.min(total_units) as u128;
    let value = 950_u128 + 40_u128 * completed / total_units as u128;
    u16::try_from(value.min(990)).expect("raster progress is at most 990 per-mille")
}

/// Evaluates one complete modeled document into private cache candidates under request-wide limits.
///
/// # Errors
///
/// Returns cancellation or the first stable validation, source, family, realization, scene, or
/// raster diagnostic; callers receive no partial result or transaction.
fn evaluate_cached_document_impl(
    request: EvaluationRequest,
    limits: EvaluationLimits,
    accepted: &DocumentDerivedCache,
    cancellation: &dyn CancellationProbe,
    mut performance: Option<&mut EvaluationPerformanceBuilder>,
    #[cfg(test)] decode_observer: Option<&AtomicUsize>,
) -> Result<CachedDocumentEvaluation, EvaluationRunError> {
    cancellation.report_progress(EvaluationProgressStage::Preparing, 10, 0);
    let preflight_started = Instant::now();
    if cancellation.is_cancelled() {
        return Err(EvaluationRunError::Cancelled);
    }
    let document = request.snapshot.document();
    document
        .validate()
        .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
    let model = document.channel_model().ok_or(EvaluationError::new(
        "evaluation.channel_topology",
        "complete evaluation requires a modeled topology",
    ))?;
    let topology = document.channel_topology().ok_or(EvaluationError::new(
        "evaluation.channel_topology",
        "complete evaluation requires an installed topology",
    ))?;
    let source_id = match document.source() {
        SourceReference::Assigned(value) => value,
        SourceReference::Unassigned => {
            return Err(EvaluationError::new(
                "evaluation.source_reference",
                "evaluation requires an assigned source reference",
            )
            .into());
        }
    };
    if source_id != request.source.reference_id() {
        return Err(EvaluationError::new(
            "evaluation.source_reference",
            "resolved source reference does not match the document snapshot",
        )
        .into());
    }
    // Resolve once in document order before capabilities, decoding, or cache
    // lookup.  The retained modeled channel contributes only mapping, paint,
    // and presentation after this authority projection.
    let resolved = topology
        .channels()
        .iter()
        .map(|channel| {
            let effective = document
                .effective_channel_pattern(channel.id)
                .map_err(EvaluationError::from_domain)?;
            let definition = document
                .pattern_definition_bundles()
                .iter()
                .find(|value| value.id == effective.definition_id)
                .map(|bundle| &bundle.definition)
                .ok_or(EvaluationError::new(
                    "evaluation.pattern_definition",
                    "channel resolves a missing pattern definition",
                ))?;
            let plan = toniator_patterns::resolve_document_pattern_pipeline(document, definition)
                .map_err(EvaluationError::from_pipeline)?;
            Ok::<_, EvaluationRunError>((channel, effective, definition, plan))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let composite_limits = limits.composite_output_limits();
    let output_units = resolved
        .iter()
        .try_fold(0_usize, |total, (_, _, _, plan)| {
            total
                .checked_add(plan.ordered_outputs.len())
                .ok_or(EvaluationError::new(
                    "realization.composite.allocation.output_units",
                    "composite output unit count overflows",
                ))
        })?;
    if output_units > composite_limits.maximum_output_units {
        return Err(EvaluationError::new(
            "realization.composite.limits.output_units",
            "request-wide composite output unit limit exceeded",
        )
        .into());
    }
    // Complete document preflight intentionally precedes decode: a later
    // invalid topology channel cannot produce an acceptable partial result.
    for (_channel, _, _, _) in &resolved {
        if cancellation.is_cancelled() {
            return Err(EvaluationRunError::Cancelled);
        }
        #[cfg(test)]
        cancellation.observe_stage(EvaluationStage::Channel, EvaluationCheckpoint::Before);
        if cancellation.is_cancelled() {
            return Err(EvaluationRunError::Cancelled);
        }
    }
    if let Some(performance) = performance.as_deref_mut() {
        performance.record(
            EvaluationPerformanceStage::Preflight,
            None,
            None,
            preflight_started.elapsed(),
            None,
            vec![EvaluationWorkloadMetric {
                kind: EvaluationWorkloadKind::OutputLayers,
                count: output_units,
            }],
        );
    }
    cancellation.report_progress(EvaluationProgressStage::Preparing, 100, 1_000);
    cancellation.report_progress(EvaluationProgressStage::DecodingSource, 100, 0);
    let source_key = SourceCacheKey {
        reference_id: request.source.reference_id().as_str().to_owned(),
        bytes: Arc::clone(&request.source.bytes),
        format: request.source.format(),
        decoder_contract: DECODER_CONTRACT_ID,
    };
    let decode_started = Instant::now();
    let (source, source_hit) = match &accepted.decoded_source {
        Some((key, value)) if *key == source_key => (Arc::clone(value), CacheDisposition::Hit),
        _ => (
            Arc::new(evaluate_stage(
                EvaluationStage::Decode,
                cancellation,
                || {
                    #[cfg(test)]
                    if let Some(observer) = decode_observer {
                        observer.fetch_add(1, Ordering::Relaxed);
                    }
                    decode_source(request.source.bytes(), request.source.format())
                        .map_err(EvaluationError::from_sampling)
                },
            )?),
            CacheDisposition::Miss,
        ),
    };
    if let Some(performance) = performance.as_deref_mut() {
        let source_pixels = usize::try_from(source.identity().width)
            .ok()
            .and_then(|width| {
                usize::try_from(source.identity().height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .unwrap_or(usize::MAX);
        performance.record(
            EvaluationPerformanceStage::SourceDecode,
            None,
            None,
            decode_started.elapsed(),
            Some(source_hit),
            vec![EvaluationWorkloadMetric {
                kind: EvaluationWorkloadKind::SourcePixels,
                count: source_pixels,
            }],
        );
    }
    cancellation.report_progress(EvaluationProgressStage::DecodingSource, 200, 1_000);
    cancellation.report_progress(EvaluationProgressStage::GeneratingGeometry, 200, 0);
    let mut summaries = Vec::with_capacity(topology.channels().len());
    let mut layers = Vec::with_capacity(topology.channels().len());
    let mut families = Vec::with_capacity(topology.channels().len());
    let mut realizations = Vec::with_capacity(topology.channels().len());
    let mut family_dispositions = Vec::with_capacity(topology.channels().len());
    let mut realization_dispositions = Vec::with_capacity(topology.channels().len());
    let mut output_realization_dispositions = Vec::with_capacity(topology.channels().len());
    let mut remaining_transformed_curve_segment_instances =
        limits.max_transformed_curve_segment_instances();
    let mut remaining_stroke_profile_samples = limits.max_stroke_profile_samples();
    let mut remaining_stroke_outline_segments = limits.max_stroke_outline_segments();
    let mut remaining_usage_memberships = composite_limits.maximum_usage_memberships;
    let mut remaining_dependency_inspections = composite_limits.maximum_dependency_inspections;
    let total_family_channels = resolved.len();
    let mut completed_family_channels = 0_usize;
    let mut completed_output_units = 0_usize;
    for (channel, effective, definition, plan) in &resolved {
        if cancellation.is_cancelled() {
            return Err(EvaluationRunError::Cancelled);
        }
        #[cfg(test)]
        cancellation.observe_stage(EvaluationStage::Channel, EvaluationCheckpoint::After);
        if cancellation.is_cancelled() {
            return Err(EvaluationRunError::Cancelled);
        }
        let output_bindings = ordered_output_bindings(effective, plan)?;
        let key = document_family_cache_key(
            document.canvas(),
            definition,
            effective,
            limits,
            &plan.family,
            &plan.ordered_outputs,
            &source,
        )?;
        let family_started = Instant::now();
        cancellation.report_progress(
            EvaluationProgressStage::GeneratingGeometry,
            document_work_progress(
                completed_family_channels,
                total_family_channels,
                completed_output_units,
                output_units,
            ),
            family_stage_progress(completed_family_channels, total_family_channels, 0, 1),
        );
        let (family, disposition, family_execution) = match families
            .iter()
            .find(|(candidate, _, _)| document_family_key_supports(candidate, &key))
        {
            Some((_, family, origin)) => (
                Arc::clone(family),
                *origin,
                EvaluationExecutionClass::LocalReuse,
            ),
            None => match accepted
                .families
                .iter()
                .find(|(candidate, _)| document_family_key_supports(candidate, &key))
            {
                Some((_, family)) => (
                    Arc::clone(family),
                    CacheDisposition::Hit,
                    EvaluationExecutionClass::AcceptedCacheHit,
                ),
                None => (
                    Arc::new(evaluate_generic_family_stage(
                        &plan.family,
                        &GridInspectRequest {
                            canvas: document.canvas().clone(),
                            density: effective.resolved_density,
                            rotation_degrees: effective.pattern_rotation_degrees,
                            translation_x: effective.translation_x,
                            translation_y: effective.translation_y,
                            guard_steps: curve_motif_phase_guard_steps(
                                definition.coverage.guard_steps,
                                &plan.ordered_outputs,
                            )?,
                            support_radius: required_support_radius_for_outputs(
                                document.canvas(),
                                effective,
                                definition,
                                &plan.family,
                                &plan.ordered_outputs,
                            )?,
                            max_family_candidates: limits.max_family_candidates(),
                        },
                        &source,
                        cancellation,
                        &|completed, total| {
                            cancellation.report_progress(
                                EvaluationProgressStage::GeneratingGeometry,
                                document_family_work_progress(
                                    completed_family_channels,
                                    total_family_channels,
                                    completed_output_units,
                                    output_units,
                                    completed,
                                    total,
                                ),
                                family_stage_progress(
                                    completed_family_channels,
                                    total_family_channels,
                                    completed,
                                    total,
                                ),
                            );
                        },
                    )?),
                    CacheDisposition::Miss,
                    EvaluationExecutionClass::Computed,
                ),
            },
        };
        completed_family_channels = completed_family_channels.saturating_add(1);
        cancellation.report_progress(
            EvaluationProgressStage::GeneratingGeometry,
            document_work_progress(
                completed_family_channels,
                total_family_channels,
                completed_output_units,
                output_units,
            ),
            family_stage_progress(completed_family_channels, total_family_channels, 0, 1),
        );
        if let Some(performance) = performance.as_deref_mut() {
            let mut workloads = vec![EvaluationWorkloadMetric {
                kind: EvaluationWorkloadKind::FamilySites,
                count: family.site_set().len(),
            }];
            if let Some(paths) = family.structural_path_set() {
                workloads.push(EvaluationWorkloadMetric {
                    kind: EvaluationWorkloadKind::StructuralPaths,
                    count: paths.paths().len(),
                });
            }
            performance.record_with_execution(
                EvaluationPerformanceStage::Family,
                Some(channel.id),
                None,
                family_started.elapsed(),
                Some(disposition),
                family_execution,
                workloads,
            );
        }
        let mut bindings_by_id = BTreeMap::new();
        for (capability, setting) in &output_bindings {
            bindings_by_id.insert(capability.layer_id, (*capability, *setting));
        }
        let mut completed_outputs = BTreeMap::new();
        let mut completed_dispositions = BTreeMap::new();
        let mut completed_diagnostics = BTreeMap::new();
        for output_layer_id in &plan.evaluation_order {
            if cancellation.is_cancelled() {
                return Err(EvaluationRunError::Cancelled);
            }
            let (output_capability, output_setting) = bindings_by_id
                .get(output_layer_id)
                .copied()
                .ok_or(EvaluationError::new(
                    "pattern.output_layers.dependency.binding",
                    "dependency order references a missing output binding",
                ))?;
            let referenced_usage = output_capability
                .source_filter
                .referenced_output_layer_id()
                .map(|referenced_id| {
                    completed_outputs
                        .get(&referenced_id)
                        .map(|output: &Arc<DocumentOutputRealization>| &output.usage)
                        .ok_or(EvaluationError::new(
                            "realization.site_filter.reference",
                            "dependent output requires completed referenced usage",
                        ))
                })
                .transpose()?;
            let inspection_count =
                family
                    .site_set()
                    .len()
                    .checked_add(1)
                    .ok_or(EvaluationError::new(
                        "realization.composite.allocation.dependency_inspections",
                        "dependency inspection count overflows",
                    ))?;
            remaining_dependency_inspections = remaining_dependency_inspections
                .checked_sub(inspection_count)
                .ok_or(EvaluationError::new(
                    "realization.composite.limits.dependency_inspections",
                    "request-wide dependency and selection inspection limit exceeded",
                ))?;
            let filter_started = Instant::now();
            let filtered_family = filtered_family_for_output(
                family.as_ref(),
                output_capability.source_filter,
                referenced_usage,
            )?;
            if let Some(performance) = performance.as_deref_mut() {
                performance.record(
                    EvaluationPerformanceStage::DependencyFilter,
                    Some(channel.id),
                    Some(*output_layer_id),
                    filter_started.elapsed(),
                    None,
                    vec![EvaluationWorkloadMetric {
                        kind: EvaluationWorkloadKind::RetainedSites,
                        count: filtered_family.site_set().len(),
                    }],
                );
            }
            let realization_key = document_realization_cache_key(
                document,
                definition,
                channel,
                effective,
                &key,
                &family,
                &source,
                output_setting,
                referenced_usage,
                limits,
            )?;
            let realization_started = Instant::now();
            cancellation.report_progress(
                EvaluationProgressStage::RealizingOutputs,
                document_work_progress(
                    completed_family_channels,
                    total_family_channels,
                    completed_output_units,
                    output_units,
                ),
                output_stage_progress(completed_output_units, output_units, 0, 1),
            );
            let collect_performance = performance.is_some();
            let output_progress_units = AtomicUsize::new(0);
            let output_total_units = filtered_family.site_set().len().max(1);
            let report_output_progress = |completed: usize, total: usize| {
                cancellation.report_progress(
                    EvaluationProgressStage::RealizingOutputs,
                    document_output_work_progress(
                        completed_family_channels,
                        total_family_channels,
                        completed_output_units,
                        output_units,
                        completed,
                        total,
                    ),
                    output_stage_progress(completed_output_units, output_units, completed, total),
                );
            };
            let (realization, realization_disposition, region_performance) = match accepted
                .realizations
                .iter()
                .find(|(candidate, _)| *candidate == realization_key)
            {
                Some((_, realization)) => (Arc::clone(realization), CacheDisposition::Hit, None),
                None => {
                    let (realization, region_performance) = match evaluate_stage(
                        EvaluationStage::Realization,
                        cancellation,
                        || {
                            evaluate_document_output(
                                document,
                                definition,
                                channel,
                                effective,
                                &source,
                                &filtered_family,
                                plan,
                                output_capability,
                                output_setting,
                                limits.max_family_candidates(),
                                remaining_transformed_curve_segment_instances,
                                remaining_stroke_profile_samples,
                                remaining_stroke_outline_segments,
                                limits.site_adjacency_limits(),
                                limits.connection_path_limits(),
                                limits.maze_limits(),
                                limits.voronoi_region_limits(),
                                limits.guide_face_limits(),
                                limits.region_sampling_limits(),
                                limits.region_treatment_limits(),
                                collect_performance,
                                &|| {
                                    if cancellation.is_cancelled() {
                                        return true;
                                    }
                                    if output_capability.regions().is_none()
                                        && !matches!(
                                            &output_capability.payload,
                                            toniator_patterns::OutputCapabilityPayload::CurveMotifPaths { .. }
                                        )
                                        && let Ok(previous) = output_progress_units.fetch_update(
                                            Ordering::Relaxed,
                                            Ordering::Relaxed,
                                            |value| {
                                                (value < output_total_units).then_some(value + 1)
                                            },
                                        )
                                    {
                                        report_output_progress(previous + 1, output_total_units);
                                    }
                                    false
                                },
                                &report_output_progress,
                            )
                        },
                    ) {
                        Err(EvaluationRunError::Evaluation(error))
                            if error.path() == "evaluation.cancelled" =>
                        {
                            return Err(EvaluationRunError::Cancelled);
                        }
                        result => result?,
                    };
                    (
                        Arc::new(realization),
                        CacheDisposition::Miss,
                        region_performance,
                    )
                }
            };
            let realization_elapsed = realization_started.elapsed();
            if let Some(performance) = performance.as_deref_mut() {
                let workloads =
                    realization_workloads(&realization.realization, &realization.capability)?;
                performance.record(
                    output_performance_stage(&realization.capability),
                    Some(channel.id),
                    Some(*output_layer_id),
                    realization_elapsed,
                    Some(realization_disposition),
                    workloads,
                );
                if let Some(region) = &region_performance {
                    performance.record(
                        EvaluationPerformanceStage::RegionSampling,
                        Some(channel.id),
                        Some(*output_layer_id),
                        region.sampling_duration,
                        Some(realization_disposition),
                        vec![
                            EvaluationWorkloadMetric {
                                kind: EvaluationWorkloadKind::Regions,
                                count: region.sampled_bases,
                            },
                            EvaluationWorkloadMetric {
                                kind: EvaluationWorkloadKind::AreaAverageFlattenedSegments,
                                count: region.flattened_segments,
                            },
                            EvaluationWorkloadMetric {
                                kind: EvaluationWorkloadKind::AreaAverageCellIntersections,
                                count: region.cell_intersections,
                            },
                        ],
                    );
                    performance.record(
                        EvaluationPerformanceStage::RegionTreatment,
                        Some(channel.id),
                        Some(*output_layer_id),
                        region.treatment_duration,
                        Some(realization_disposition),
                        vec![EvaluationWorkloadMetric {
                            kind: EvaluationWorkloadKind::Regions,
                            count: region.retained_regions,
                        }],
                    );
                }
            }
            remaining_transformed_curve_segment_instances =
                remaining_transformed_curve_segment_instances
                    .checked_sub(transformed_curve_segment_instances(
                        &realization.realization,
                    )?)
                    .ok_or(EvaluationError::new(
                        "realization.mark.segment_limit",
                        "transformed curve-segment instance limit exceeded",
                    ))?;
            let (profile_samples, outline_segments) = stroke_work(&realization.realization)?;
            remaining_stroke_profile_samples = remaining_stroke_profile_samples
                .checked_sub(profile_samples)
                .ok_or(EvaluationError::new(
                    "realization.stroke.profile_limit",
                    "request-wide canonical stroke profile sample limit exceeded",
                ))?;
            remaining_stroke_outline_segments = remaining_stroke_outline_segments
                .checked_sub(outline_segments)
                .ok_or(EvaluationError::new(
                    "realization.stroke.outline_limit",
                    "request-wide canonical stroke outline segment limit exceeded",
                ))?;
            remaining_usage_memberships = remaining_usage_memberships
                .checked_sub(realization.usage.members().len())
                .ok_or(EvaluationError::new(
                    "realization.composite.limits.usage_memberships",
                    "request-wide site-usage membership limit exceeded",
                ))?;
            completed_diagnostics.insert(
                output_setting.output_layer_id,
                OutputCacheDiagnostics {
                    output_layer_id: output_setting.output_layer_id,
                    realization: realization_disposition,
                    voronoi: region_diagnostics(&realization.realization),
                    region: region_output_diagnostics(&realization.realization),
                },
            );
            realizations.push((realization_key, Arc::clone(&realization)));
            completed_dispositions.insert(output_setting.output_layer_id, realization_disposition);
            completed_outputs.insert(output_setting.output_layer_id, realization);
            completed_output_units = completed_output_units.saturating_add(1);
            cancellation.report_progress(
                EvaluationProgressStage::RealizingOutputs,
                document_work_progress(
                    completed_family_channels,
                    total_family_channels,
                    completed_output_units,
                    output_units,
                ),
                output_stage_progress(completed_output_units, output_units, 0, 1),
            );
        }
        let channel_outputs = output_bindings
            .iter()
            .map(|(capability, _)| {
                completed_outputs
                    .get(&capability.layer_id)
                    .cloned()
                    .expect("validated dependency order completes every painter output")
            })
            .collect::<Vec<_>>();
        let channel_output_dispositions = output_bindings
            .iter()
            .map(|(capability, _)| {
                completed_diagnostics
                    .get(&capability.layer_id)
                    .cloned()
                    .expect("completed output retains painter-order diagnostics")
            })
            .collect::<Vec<_>>();
        let realization_disposition = if channel_output_dispositions
            .iter()
            .all(|value| value.realization == CacheDisposition::Hit)
        {
            CacheDisposition::Hit
        } else {
            CacheDisposition::Miss
        };
        summaries.push(ChannelEvaluationSummary {
            role: channel.role,
            channel_id: channel.id,
            family_identity: family.family_fingerprint().to_owned(),
            realization_identity: aggregate_output_realization_identity(
                &channel_outputs
                    .iter()
                    .map(|output| {
                        (
                            output.output_layer_id,
                            document_realization_identity(&output.realization),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
        });
        layers.push(document_render_layer(channel, &channel_outputs)?);
        families.push((key, family, disposition));
        family_dispositions.push(disposition);
        realization_dispositions.push(realization_disposition);
        output_realization_dispositions.push(channel_output_dispositions);
        if cancellation.is_cancelled() {
            return Err(EvaluationRunError::Cancelled);
        }
    }
    let family_identity = aggregate_document_identity(
        "family",
        model,
        summaries
            .iter()
            .map(|value| (value.role, value.channel_id, value.family_identity.as_str())),
    );
    let realization_identity = aggregate_document_identity(
        "realization",
        model,
        summaries.iter().map(|value| {
            (
                value.role,
                value.channel_id,
                value.realization_identity.as_str(),
            )
        }),
    );
    cancellation.report_progress(EvaluationProgressStage::ComposingScene, 900, 0);
    let scene_started = Instant::now();
    let built_scene = evaluate_stage(EvaluationStage::Scene, cancellation, || {
        RenderScene::new_modeled(
            document.canvas().clone(),
            family_identity,
            realization_identity,
            model,
            layers,
        )
        .map_err(EvaluationError::from_render)
    })?;
    let scene_key = built_scene.identity().scene_fingerprint().to_owned();
    let (scene, scene_disposition) = match &accepted.scene {
        Some((key, value)) if *key == scene_key => (Arc::clone(value), CacheDisposition::Hit),
        _ => (Arc::new(built_scene), CacheDisposition::Miss),
    };
    if let Some(performance) = performance.as_deref_mut() {
        performance.record(
            EvaluationPerformanceStage::Scene,
            None,
            None,
            scene_started.elapsed(),
            Some(scene_disposition),
            vec![EvaluationWorkloadMetric {
                kind: EvaluationWorkloadKind::OutputLayers,
                count: output_units,
            }],
        );
    }
    let raster_key = match request.preview_target {
        Some(target) => format!(
            "{}:{TRANSPARENT_RASTER_CONTRACT_ID}:preview-v1:{model:?}:{}x{}:edges={}",
            scene.identity().scene_fingerprint(),
            target.width(),
            target.height(),
            limits.max_flattened_raster_edges()
        ),
        None => format!(
            "{}:{TRANSPARENT_RASTER_CONTRACT_ID}:{model:?}:edges={}",
            scene.identity().scene_fingerprint(),
            limits.max_flattened_raster_edges()
        ),
    };
    cancellation.report_progress(EvaluationProgressStage::ComposingScene, 950, 1_000);
    cancellation.report_progress(EvaluationProgressStage::RasterizingPreview, 950, 0);
    let raster_started = Instant::now();
    let (raster, raster_disposition) = match &accepted.raster {
        Some((key, value)) if *key == raster_key => (Arc::clone(value), CacheDisposition::Hit),
        _ => (
            Arc::new(evaluate_stage(
                EvaluationStage::Raster,
                cancellation,
                || {
                    match request.preview_target {
                        Some(target) => rasterize_preview_cancellable_with_progress(
                            &scene,
                            target,
                            toniator_render::RasterizationLimits::new(
                                limits.max_flattened_raster_edges(),
                            )
                            .expect("EvaluationLimits validates raster edge bounds"),
                            &|| cancellation.is_cancelled(),
                            &|completed, total| {
                                cancellation.report_progress(
                                    EvaluationProgressStage::RasterizingPreview,
                                    raster_work_progress(completed, total),
                                    unit_stage_progress(completed, total),
                                );
                            },
                        ),
                        None => rasterize_cancellable_with_progress(
                            &scene,
                            RasterBackground::Transparent,
                            toniator_render::RasterizationLimits::new(
                                limits.max_flattened_raster_edges(),
                            )
                            .expect("EvaluationLimits validates raster edge bounds"),
                            &|| cancellation.is_cancelled(),
                            &|completed, total| {
                                cancellation.report_progress(
                                    EvaluationProgressStage::RasterizingPreview,
                                    raster_work_progress(completed, total),
                                    unit_stage_progress(completed, total),
                                );
                            },
                        ),
                    }
                    .map_err(EvaluationError::from_render)
                },
            )?),
            CacheDisposition::Miss,
        ),
    };
    if let Some(performance) = performance {
        let raster_pixels = usize::try_from(raster.width())
            .ok()
            .and_then(|width| {
                usize::try_from(raster.height())
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .unwrap_or(usize::MAX);
        performance.record(
            EvaluationPerformanceStage::Raster,
            None,
            None,
            raster_started.elapsed(),
            Some(raster_disposition),
            vec![EvaluationWorkloadMetric {
                kind: EvaluationWorkloadKind::RasterPixels,
                count: raster_pixels,
            }],
        );
    }
    cancellation.report_progress(EvaluationProgressStage::RasterizingPreview, 990, 1_000);
    cancellation.report_progress(EvaluationProgressStage::Finalizing, 990, 0);
    let result = EvaluationResult {
        token: request.snapshot.token(),
        source_identity: source.identity().clone(),
        channels: summaries,
        // Evaluation results share the immutable cache values directly. The
        // public accessors still expose ordinary borrows, so callers cannot
        // observe this storage optimization or mutate cached authority.
        scene: Arc::clone(&scene),
        raster: Arc::clone(&raster),
    };
    let channels = result
        .channels
        .iter()
        .enumerate()
        .map(|(index, value)| ChannelCacheDiagnostics {
            channel_id: value.channel_id,
            family: family_dispositions[index],
            realization: realization_dispositions[index],
            outputs: output_realization_dispositions[index].clone(),
        })
        .collect();
    cancellation.report_progress(EvaluationProgressStage::Finalizing, 1_000, 1_000);
    Ok(CachedDocumentEvaluation {
        result,
        diagnostics: DocumentCacheDiagnostics {
            aggregate: CacheDiagnostics {
                decoded_source: source_hit,
                family: if family_dispositions
                    .iter()
                    .all(|value| *value == CacheDisposition::Hit)
                {
                    CacheDisposition::Hit
                } else {
                    CacheDisposition::Miss
                },
                realization: if realization_dispositions
                    .iter()
                    .all(|value| *value == CacheDisposition::Hit)
                {
                    CacheDisposition::Hit
                } else {
                    CacheDisposition::Miss
                },
                scene: scene_disposition,
                raster: raster_disposition,
            },
            channels,
        },
        transaction: DocumentCacheTransaction {
            decoded_source: (source_hit == CacheDisposition::Miss).then_some((source_key, source)),
            families: families
                .into_iter()
                .map(|(key, family, _)| (key, family))
                .collect(),
            realizations,
            scene: (scene_disposition == CacheDisposition::Miss).then_some((scene_key, scene)),
            raster: (raster_disposition == CacheDisposition::Miss).then_some((raster_key, raster)),
        },
    })
}

/// Returns geometry-owned algorithm identities only for a typed connection output.
fn connection_cache_contracts(
    definition: &PatternDefinition,
    output_layer_id: toniator_domain::PatternOutputLayerId,
    adjacency_limits: SiteAdjacencyLimits,
    connection_limits: ConnectionPathLimits,
) -> Option<ConnectionCacheContracts> {
    let output = definition
        .output_layers
        .iter()
        .find(|output| output.id() == output_layer_id)?;
    let PatternOutputRealization::ConnectionPaths { program, .. } = &output.realization else {
        return None;
    };
    Some(ConnectionCacheContracts {
        site_adjacency: SITE_ADJACENCY_CONTRACT_ID,
        path_selection: CONNECTION_PATH_CONTRACT_ID,
        trail_decomposition: CONNECTION_TRAIL_CONTRACT_ID,
        program_selection: connection_program_contract_id(program),
        adjacency_limits,
        connection_limits,
    })
}

/// Returns geometry-owned identity and bounded work inputs only for a typed maze-wall output.
fn maze_cache_contracts(
    definition: &PatternDefinition,
    output_layer_id: toniator_domain::PatternOutputLayerId,
    limits: MazeLimits,
) -> Option<MazeCacheContracts> {
    let output = definition
        .output_layers
        .iter()
        .find(|output| output.id() == output_layer_id)?;
    let PatternOutputRealization::MazeWalls { program, .. } = &output.realization else {
        return None;
    };
    Some(MazeCacheContracts {
        arrangement: MAZE_WALL_CONTRACT_ID,
        algorithm: program.algorithm,
        seed: program.seed,
        limits,
    })
}

#[derive(Clone)]
enum DocumentRealization {
    Mapped {
        geometry: Arc<GeometryOutput>,
        fingerprint: String,
    },
    SourceColor {
        geometry: Arc<GeometryOutput>,
        paints: Arc<Vec<toniator_domain::ColorValue>>,
        fingerprint: String,
    },
    Canonical {
        geometry: Arc<GeometryOutput>,
        paints: Option<Arc<Vec<toniator_domain::ColorValue>>>,
        fingerprint: String,
    },
    Strokes {
        geometry: Arc<GeometryOutput>,
        fingerprint: String,
    },
    Regions {
        geometry: Arc<GeometryOutput>,
        paints: Option<Arc<Vec<toniator_domain::ColorValue>>>,
        fingerprint: String,
        diagnostics: RegionRealizationDiagnostics,
    },
}

impl DocumentRealization {
    /// Returns the one immutable renderer-ready geometry payload retained by the output cache.
    fn geometry(&self) -> &Arc<GeometryOutput> {
        match self {
            Self::Mapped { geometry, .. }
            | Self::SourceColor { geometry, .. }
            | Self::Canonical { geometry, .. }
            | Self::Strokes { geometry, .. }
            | Self::Regions { geometry, .. } => geometry,
        }
    }

    /// Returns immutable sampled paint when the cached output owns per-primitive color.
    fn primitive_paints(&self) -> Option<&Arc<Vec<toniator_domain::ColorValue>>> {
        match self {
            Self::SourceColor { paints, .. } => Some(paints),
            Self::Canonical { paints, .. } | Self::Regions { paints, .. } => paints.as_ref(),
            Self::Mapped { .. } | Self::Strokes { .. } => None,
        }
    }
}

/// Converts sampled source paint into renderer-owned immutable linear color storage.
fn shared_sampled_paints(
    paints: Option<Vec<toniator_sampling::SampledSourcePaint>>,
) -> Option<Arc<Vec<toniator_domain::ColorValue>>> {
    paints.map(|paints| {
        Arc::new(
            paints
                .into_iter()
                .map(|paint| toniator_domain::ColorValue {
                    red: paint.red,
                    green: paint.green,
                    blue: paint.blue,
                    alpha: paint.alpha,
                })
                .collect(),
        )
    })
}

/// Moves one typed mapped-mark output into the engine's shared renderer-ready cache record.
fn cache_mapped_realization(
    value: toniator_patterns::TypedRealization<toniator_patterns::MappedCircularMarkRealization>,
) -> DocumentRealization {
    let output = value.output;
    DocumentRealization::Mapped {
        geometry: Arc::new(GeometryOutput::CircularMarks(output.marks)),
        fingerprint: output.realization_fingerprint,
    }
}

/// Splits one typed source-colored output once, retaining shared geometry and paint thereafter.
fn cache_source_color_realization(
    value: toniator_patterns::TypedRealization<
        toniator_patterns::SourceColorCircularMarkRealization,
    >,
) -> DocumentRealization {
    let output = value.output;
    let mut marks = Vec::with_capacity(output.marks.len());
    let mut paints = Vec::with_capacity(output.marks.len());
    for entry in output.marks {
        marks.push(entry.mark);
        paints.push(toniator_domain::ColorValue {
            red: entry.paint.red,
            green: entry.paint.green,
            blue: entry.paint.blue,
            alpha: entry.paint.alpha,
        });
    }
    DocumentRealization::SourceColor {
        geometry: Arc::new(GeometryOutput::CircularMarks(marks)),
        paints: Arc::new(paints),
        fingerprint: output.realization_fingerprint,
    }
}

/// Moves one generalized canonical-mark output into shared cache/scene payloads.
fn cache_canonical_mark_realization(
    value: toniator_patterns::TypedRealization<CanonicalMarkRealization>,
) -> DocumentRealization {
    let output = value.output;
    DocumentRealization::Canonical {
        geometry: Arc::new(GeometryOutput::CanonicalMarks(output.marks)),
        paints: shared_sampled_paints(output.paints),
        fingerprint: output.realization_fingerprint,
    }
}

/// Moves one canonical-stroke output into the sole cache/scene geometry allocation.
fn cache_stroke_realization(value: CanonicalStrokeRealization) -> DocumentRealization {
    DocumentRealization::Strokes {
        geometry: Arc::new(GeometryOutput::CanonicalStrokes(value.strokes)),
        fingerprint: value.realization_fingerprint,
    }
}

/// Producer/source/sampling/treatment facts retained beside canonical region geometry identity.
///
/// This private cache-unit record is cloned into public output diagnostics on every miss and hit.
/// It never contributes to a region geometry fingerprint or persistence payload.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RegionRealizationDiagnostics {
    source_identity: Option<SourceIdentity>,
    producer: RegionProducerCacheDiagnostics,
    sampling: RegionSamplingCacheDiagnostics,
    treatment: RegionTreatmentCacheDiagnostics,
}

/// One independently cached channel-output realization assembled only after its explicit binding validates.
#[derive(Clone)]
struct DocumentOutputRealization {
    output_layer_id: toniator_domain::PatternOutputLayerId,
    capability: toniator_patterns::OutputCapability,
    setting: toniator_domain::EffectivePatternOutputSettings,
    realization: DocumentRealization,
    usage: SiteUsageSet,
}

/// Pairs every effective output with its authored painter-order capability.
///
/// # Errors
///
/// Returns a stable pipeline diagnostic when effective settings cannot bind exactly to the
/// resolved authored output plan.
fn ordered_output_bindings<'a>(
    effective: &'a toniator_domain::EffectiveChannelPatternInstance,
    plan: &'a toniator_patterns::PatternPipelinePlan,
) -> Result<
    Vec<(
        &'a toniator_patterns::OutputCapability,
        &'a toniator_domain::EffectivePatternOutputSettings,
    )>,
    EvaluationError,
> {
    if effective.output_settings.len() != plan.ordered_outputs.len() {
        return Err(EvaluationError::new(
            "evaluation.output_binding",
            "effective output settings must match the complete pipeline output collection",
        ));
    }
    effective
        .output_settings
        .iter()
        .map(|setting| {
            let capability =
                plan.output_capability(setting.output_layer_id)
                    .ok_or(EvaluationError::new(
                        "evaluation.output_binding",
                        "effective output setting has no matching pipeline capability",
                    ))?;
            toniator_patterns::validate_output_realization_binding(plan, capability, setting)
                .map_err(EvaluationError::from_pipeline)?;
            Ok((capability, setting))
        })
        .collect()
}

/// Returns the complete immutable realization fingerprint for any retained or canonical variant.
fn document_realization_identity(value: &DocumentRealization) -> &str {
    match value {
        DocumentRealization::Mapped { fingerprint, .. }
        | DocumentRealization::SourceColor { fingerprint, .. }
        | DocumentRealization::Canonical { fingerprint, .. }
        | DocumentRealization::Strokes { fingerprint, .. }
        | DocumentRealization::Regions { fingerprint, .. } => fingerprint,
    }
}

/// Projects immutable geometry diagnostics from a completed ordinary-region output cache unit.
fn region_diagnostics(value: &DocumentRealization) -> Option<VoronoiRegionDiagnostics> {
    match value {
        DocumentRealization::Regions {
            diagnostics:
                RegionRealizationDiagnostics {
                    producer: RegionProducerCacheDiagnostics::Voronoi(diagnostics),
                    ..
                },
            ..
        } => Some(*diagnostics),
        _ => None,
    }
}

/// Clones complete Region output diagnostics from a cache unit without altering geometry identity.
fn region_output_diagnostics(value: &DocumentRealization) -> Option<RegionOutputCacheDiagnostics> {
    let DocumentRealization::Regions { diagnostics, .. } = value else {
        return None;
    };
    Some(RegionOutputCacheDiagnostics {
        source_identity: diagnostics.source_identity.clone(),
        producer: diagnostics.producer.clone(),
        sampling: diagnostics.sampling,
        treatment: diagnostics.treatment,
    })
}

/// Builds cache-unit diagnostics from the completed typed region realizer and one producer result.
///
/// Source identity is retained only when an actual base sample occurred. The response-selected
/// strategy and treatment kind remain diagnostic facts, never a replacement effective model.
fn completed_region_diagnostics(
    response: &RegionGeometryResponse,
    source: &SourceField,
    realization: toniator_patterns::RegionOutputRealizationDiagnostics,
    producer: RegionProducerCacheDiagnostics,
) -> RegionRealizationDiagnostics {
    let kind = match response.algorithm {
        toniator_domain::RegionResizeAlgorithm::Scale => RegionTreatmentCacheKind::Scale,
        toniator_domain::RegionResizeAlgorithm::UniformOffset => {
            RegionTreatmentCacheKind::UniformOffset
        }
    };
    RegionRealizationDiagnostics {
        source_identity: (realization.sampled_bases > 0).then(|| source.identity().clone()),
        producer,
        sampling: RegionSamplingCacheDiagnostics {
            strategy: response.sampling,
            sampled_bases: realization.sampled_bases,
        },
        treatment: RegionTreatmentCacheDiagnostics {
            kind,
            retained_regions: realization.retained_regions,
        },
    }
}

/// Evaluates one explicit output unit after dependency filtering.
///
/// # Errors
///
/// Returns cancellation or a stable family, response, realization, or work-limit diagnostic without
/// producing a cache candidate.
#[allow(clippy::too_many_arguments)]
fn evaluate_document_output(
    document: &toniator_domain::Document,
    definition: &PatternDefinition,
    channel: &ModeledChannelState,
    effective: &toniator_domain::EffectiveChannelPatternInstance,
    source: &SourceField,
    family: &TypedFamilyOutput,
    plan: &toniator_patterns::PatternPipelinePlan,
    capability: &toniator_patterns::OutputCapability,
    setting: &toniator_domain::EffectivePatternOutputSettings,
    max_family_candidates: usize,
    max_transformed_curve_segment_instances: usize,
    max_stroke_profile_samples: usize,
    max_stroke_outline_segments: usize,
    adjacency_limits: SiteAdjacencyLimits,
    connection_limits: ConnectionPathLimits,
    maze_limits: MazeLimits,
    voronoi_limits: VoronoiRegionLimits,
    guide_face_limits: GuideFaceLimits,
    region_sampling_limits: RegionSamplingLimits,
    region_treatment_limits: RegionTreatmentLimits,
    profiled: bool,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<
    (
        DocumentOutputRealization,
        Option<toniator_patterns::RegionOutputPerformanceMetrics>,
    ),
    EvaluationError,
> {
    let mut output_plan = plan.clone();
    output_plan.ordered_outputs = vec![capability.clone()];
    output_plan.evaluation_order = vec![capability.layer_id];
    let (realization, usage, performance) = evaluate_document_channel(
        document,
        definition,
        channel,
        effective,
        source,
        family,
        &output_plan,
        capability,
        setting,
        max_family_candidates,
        max_transformed_curve_segment_instances,
        max_stroke_profile_samples,
        max_stroke_outline_segments,
        adjacency_limits,
        connection_limits,
        maze_limits,
        voronoi_limits,
        guide_face_limits,
        region_sampling_limits,
        region_treatment_limits,
        profiled,
        is_cancelled,
        progress,
    )?;
    Ok((
        DocumentOutputRealization {
            output_layer_id: capability.layer_id,
            capability: capability.clone(),
            setting: setting.clone(),
            realization,
            usage,
        },
        performance,
    ))
}

/// Aggregates ordered output fingerprints without rewriting existing one-output identities.
///
/// The single-unit branch preserves all established fingerprint bytes. Heterogeneous documents
/// obtain an ordered, layer-ID-qualified aggregate only above independently cached units.
fn aggregate_output_realization_identity(
    outputs: &[(toniator_domain::PatternOutputLayerId, &str)],
) -> String {
    match outputs {
        [(_, fingerprint)] => (*fingerprint).to_owned(),
        _ => {
            let mut identity = String::from("toniator-stage20n-output-aggregate-v1");
            for (output_layer_id, fingerprint) in outputs {
                identity.push_str(&format!(":{}:{fingerprint}", output_layer_id.0));
            }
            identity
        }
    }
}

/// Borrows a complete family for unrestricted output use and owns only dependent filters.
///
/// `SiteUseFilter::All` is semantically the cached family itself, so this
/// helper avoids cloning its sites, structure, diagnostics, or provenance.
/// Used/unused filters retain the patterns-owned filtered-family validation
/// boundary and may allocate a distinct exact subset.
///
/// # Errors
///
/// Returns the patterns-owned filtering diagnostic without modifying cache
/// state when a dependent usage set is invalid.
fn filtered_family_for_output<'a>(
    family: &'a TypedFamilyOutput,
    filter: toniator_domain::SiteUseFilter,
    referenced_usage: Option<&SiteUsageSet>,
) -> Result<Cow<'a, TypedFamilyOutput>, EvaluationError> {
    if matches!(filter, toniator_domain::SiteUseFilter::All) {
        Ok(Cow::Borrowed(family))
    } else {
        family
            .filtered_for_output(filter, referenced_usage)
            .map(Cow::Owned)
            .map_err(EvaluationError::from_pipeline)
    }
}

/// Counts authored path segment instances in one completed channel realization for the
/// request-wide evaluation budget; circle and retained adapter outputs consume no path work.
///
/// # Errors
///
/// Returns the stable segment-limit diagnostic if a maliciously large completed value overflows.
fn transformed_curve_segment_instances(
    value: &DocumentRealization,
) -> Result<usize, EvaluationError> {
    if let GeometryOutput::CanonicalStrokes(strokes) = value.geometry().as_ref() {
        return strokes.iter().try_fold(0_usize, |total, stroke| {
            total
                .checked_add(stroke.path.segments().len())
                .ok_or(EvaluationError::new(
                    "realization.stroke.segment_limit",
                    "stroke segment instance count overflows",
                ))
        });
    }
    if matches!(
        value.geometry().as_ref(),
        GeometryOutput::CanonicalRegions(_)
    ) {
        return Ok(0);
    }
    let GeometryOutput::CanonicalMarks(marks) = value.geometry().as_ref() else {
        return Ok(0);
    };
    marks.iter().try_fold(0_usize, |total, mark| {
        let count = match mark {
            CanonicalMark::Circle { .. } => 0,
            CanonicalMark::ClosedPath(path) => path.path.segments().len(),
        };
        total.checked_add(count).ok_or(EvaluationError::new(
            "realization.mark.segment_limit",
            "transformed curve-segment instance count overflows",
        ))
    })
}

/// Counts canonical stroke profile and outline work for request-wide budget enforcement.
///
/// Cache hits and misses charge the same completed immutable geometry, so aggregate limits never
/// depend on cache disposition or output ordering beyond the authored deterministic traversal.
///
/// # Errors
///
/// Returns a stable stroke-limit diagnostic if a completed collection overflows `usize`.
fn stroke_work(value: &DocumentRealization) -> Result<(usize, usize), EvaluationError> {
    let GeometryOutput::CanonicalStrokes(strokes) = value.geometry().as_ref() else {
        return Ok((0, 0));
    };
    canonical_stroke_work(strokes)
}

/// Counts work in one completed canonical stroke slice without inspecting semantic identity.
///
/// # Errors
///
/// Returns a stable stroke-limit diagnostic if profile or outline counts overflow `usize`.
fn canonical_stroke_work(
    strokes: &[toniator_patterns::CanonicalStroke],
) -> Result<(usize, usize), EvaluationError> {
    strokes.iter().try_fold(
        (0_usize, 0_usize),
        |(profile_samples, outline_segments), stroke| {
            let profile_samples =
                profile_samples
                    .checked_add(stroke.profile.len())
                    .ok_or(EvaluationError::new(
                        "realization.stroke.profile_limit",
                        "canonical stroke profile sample count overflows",
                    ))?;
            let stroke_outline_segments =
                stroke
                    .outline
                    .contours
                    .iter()
                    .try_fold(0_usize, |total, contour| {
                        total
                            .checked_add(contour.segments.len())
                            .ok_or(EvaluationError::new(
                                "realization.stroke.outline_limit",
                                "canonical stroke outline segment count overflows",
                            ))
                    })?;
            let outline_segments = outline_segments
                .checked_add(stroke_outline_segments)
                .ok_or(EvaluationError::new(
                    "realization.stroke.outline_limit",
                    "canonical stroke outline segment count overflows",
                ))?;
            Ok((profile_samples, outline_segments))
        },
    )
}

/// Derives inexpensive deterministic output counts for the opt-in performance report.
///
/// # Errors
///
/// Returns the existing stable stroke or region count diagnostic if a completed immutable output
/// overflows the platform count type.
fn realization_workloads(
    value: &DocumentRealization,
    capability: &toniator_patterns::OutputCapability,
) -> Result<Vec<EvaluationWorkloadMetric>, EvaluationError> {
    let mut workloads = vec![EvaluationWorkloadMetric {
        kind: output_workload_kind(value, capability),
        count: 1,
    }];
    let details = match value.geometry().as_ref() {
        GeometryOutput::CircularMarks(marks) => Ok(vec![EvaluationWorkloadMetric {
            kind: EvaluationWorkloadKind::Marks,
            count: marks.len(),
        }]),
        GeometryOutput::CanonicalMarks(marks) => Ok(vec![EvaluationWorkloadMetric {
            kind: EvaluationWorkloadKind::Marks,
            count: marks.len(),
        }]),
        GeometryOutput::CanonicalStrokes(strokes) => {
            let (profile_samples, outline_segments) = canonical_stroke_work(strokes)?;
            Ok(vec![
                EvaluationWorkloadMetric {
                    kind: EvaluationWorkloadKind::Strokes,
                    count: strokes.len(),
                },
                EvaluationWorkloadMetric {
                    kind: EvaluationWorkloadKind::StrokeProfileSamples,
                    count: profile_samples,
                },
                EvaluationWorkloadMetric {
                    kind: EvaluationWorkloadKind::StrokeOutlineSegments,
                    count: outline_segments,
                },
            ])
        }
        GeometryOutput::CanonicalRegions(regions) => {
            let boundary_segments =
                regions
                    .regions()
                    .iter()
                    .try_fold(0_usize, |total, region| {
                        total
                            .checked_add(region.ring.segments().len())
                            .ok_or(EvaluationError::new(
                                "region.resize.limits.canonical",
                                "profiled region boundary segment count overflows",
                            ))
                    })?;
            Ok(vec![
                EvaluationWorkloadMetric {
                    kind: EvaluationWorkloadKind::Regions,
                    count: regions.regions().len(),
                },
                EvaluationWorkloadMetric {
                    kind: EvaluationWorkloadKind::RegionBoundarySegments,
                    count: boundary_segments,
                },
            ])
        }
    }?;
    workloads.extend(details);
    Ok(workloads)
}

/// Classifies one completed output by its authoritative producer without timing sub-protocols.
fn output_workload_kind(
    value: &DocumentRealization,
    capability: &toniator_patterns::OutputCapability,
) -> EvaluationWorkloadKind {
    if capability.connection_paths().is_some() {
        EvaluationWorkloadKind::ConnectionOutput
    } else if capability.maze_walls().is_some() {
        EvaluationWorkloadKind::MazeOutput
    } else if let Some(source) = capability.regions() {
        match source {
            toniator_domain::RegionSourceIntent::VoronoiSites { .. } => {
                EvaluationWorkloadKind::VoronoiOutput
            }
            toniator_domain::RegionSourceIntent::GuideFaces { .. } => {
                EvaluationWorkloadKind::GuideFaceOutput
            }
        }
    } else if matches!(
        value.geometry().as_ref(),
        GeometryOutput::CanonicalStrokes(_)
    ) {
        EvaluationWorkloadKind::StructuralPathOutput
    } else {
        EvaluationWorkloadKind::MarkOutput
    }
}

/// Selects the narrowest existing output-authority boundary for one complete realization timing.
fn output_performance_stage(
    capability: &toniator_patterns::OutputCapability,
) -> EvaluationPerformanceStage {
    if capability.connection_paths().is_some() {
        EvaluationPerformanceStage::ConnectionRealization
    } else if capability.maze_walls().is_some() {
        EvaluationPerformanceStage::MazeRealization
    } else if let Some(source) = capability.regions() {
        match source {
            toniator_domain::RegionSourceIntent::VoronoiSites { .. } => {
                EvaluationPerformanceStage::VoronoiRealization
            }
            toniator_domain::RegionSourceIntent::GuideFaces { .. } => {
                EvaluationPerformanceStage::GuideFaceRealization
            }
        }
    } else if capability.guide_paths().is_some() {
        EvaluationPerformanceStage::StructuralPathRealization
    } else {
        EvaluationPerformanceStage::MarkRealization
    }
}

/// Invokes the sole patterns Region realizer and optionally records its test-only completed state.
///
/// Production calls the ordinary patterns authority unchanged. Test builds select the evidence
/// variant only while the thread-local observer is enabled, preserving a single sampling and
/// treatment invocation and mapping failures through the normal engine error boundary.
///
/// # Errors
///
/// Returns the normal typed-region error as an evaluation error without recording a snapshot.
#[allow(clippy::too_many_arguments)]
fn realize_document_region_output(
    capability: &toniator_patterns::OutputCapability,
    setting: &toniator_domain::EffectivePatternOutputSettings,
    untreated: &CanonicalRegionSet,
    references: &[RegionReference],
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: toniator_domain::SourceMapping,
    paint: &ChannelPaint,
    sampling_limits: RegionSamplingLimits,
    treatment_limits: RegionTreatmentLimits,
    profiled: bool,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<
    (
        toniator_patterns::TypedRegionOutputRealization,
        Option<toniator_patterns::RegionOutputPerformanceMetrics>,
    ),
    EvaluationError,
> {
    #[cfg(test)]
    if region_evaluation_evidence_enabled() {
        let (realization, evidence) = realize_region_output_with_evidence_cancellable(
            capability,
            setting,
            untreated,
            references,
            Some(source),
            canvas,
            mapping,
            paint,
            sampling_limits,
            treatment_limits,
            is_cancelled,
        )
        .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
        record_region_evaluation_evidence(evidence);
        return Ok((realization, None));
    }
    if profiled {
        return toniator_patterns::realize_region_output_profiled_cancellable(
            capability,
            setting,
            untreated,
            references,
            Some(source),
            canvas,
            mapping,
            paint,
            sampling_limits,
            treatment_limits,
            is_cancelled,
        )
        .map(|(realization, performance)| (realization, Some(performance)))
        .map_err(|error| EvaluationError::new(error.path(), error.message()));
    }
    realize_region_output_with_progress_cancellable(
        capability,
        setting,
        untreated,
        references,
        Some(source),
        canvas,
        mapping,
        paint,
        sampling_limits,
        treatment_limits,
        is_cancelled,
        progress,
    )
    .map(|realization| (realization, None))
    .map_err(|error| EvaluationError::new(error.path(), error.message()))
}

/// Realizes one document channel through typed family, region, mark, or stroke authority.
///
/// The caller has completed document reference validation. This boundary publishes no partial
/// cache candidate and preserves renderer ownership of final clipping.
///
/// # Errors
///
/// Returns the first stable source, response, cancellation, segment-limit, or realization error.
#[allow(clippy::too_many_arguments)]
fn evaluate_document_channel(
    document: &toniator_domain::Document,
    definition: &PatternDefinition,
    channel: &ModeledChannelState,
    effective: &toniator_domain::EffectiveChannelPatternInstance,
    source: &SourceField,
    family: &TypedFamilyOutput,
    plan: &toniator_patterns::PatternPipelinePlan,
    output_capability: &toniator_patterns::OutputCapability,
    output_setting: &toniator_domain::EffectivePatternOutputSettings,
    max_family_candidates: usize,
    max_transformed_curve_segment_instances: usize,
    max_stroke_profile_samples: usize,
    max_stroke_outline_segments: usize,
    adjacency_limits: SiteAdjacencyLimits,
    connection_limits: ConnectionPathLimits,
    maze_limits: MazeLimits,
    voronoi_limits: VoronoiRegionLimits,
    guide_face_limits: GuideFaceLimits,
    region_sampling_limits: RegionSamplingLimits,
    region_treatment_limits: RegionTreatmentLimits,
    profiled: bool,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<
    (
        DocumentRealization,
        SiteUsageSet,
        Option<toniator_patterns::RegionOutputPerformanceMetrics>,
    ),
    EvaluationError,
> {
    toniator_patterns::validate_output_realization_binding(plan, output_capability, output_setting)
        .map_err(EvaluationError::from_pipeline)?;
    if let Some(region_source) = output_capability.regions() {
        let canvas = Bounds::new(
            Point2::new(0.0, 0.0),
            Point2::new(document.canvas().width, document.canvas().height),
        )
        .ok_or(EvaluationError::new(
            "region.voronoi.geometry.canvas",
            "ordinary Voronoi requires finite canvas bounds",
        ))?;
        match region_source {
            toniator_domain::RegionSourceIntent::VoronoiSites { site_mechanism_id } => {
                if definition.coverage.guard_steps == 0 {
                    return Err(EvaluationError::new(
                        "region.voronoi.coverage.guard_steps",
                        "ordinary Voronoi regions require at least one configured guard step",
                    ));
                }
                if family.site_set().product_mechanism_id() != *site_mechanism_id {
                    return Err(EvaluationError::new(
                        "region.voronoi.identity.site_mechanism",
                        "ordinary region output targets a foreign family site mechanism",
                    ));
                }
                let (regions, diagnostics) = build_voronoi_regions_cancellable(
                    family.site_set(),
                    VoronoiRegionRequest {
                        output_layer_id: output_capability.layer_id,
                        canvas,
                    },
                    voronoi_limits,
                    is_cancelled,
                )
                .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
                let references: Vec<RegionReference> =
                    voronoi_region_references(family.site_set(), &regions)
                        .map_err(|error| EvaluationError::new(error.path(), error.message()))?
                        .into_iter()
                        .map(|(region_id, point)| RegionReference { region_id, point })
                        .collect();
                let (realized, performance) = realize_document_region_output(
                    output_capability,
                    output_setting,
                    &regions,
                    &references,
                    source,
                    document.canvas(),
                    channel.mapping,
                    &channel.paint,
                    region_sampling_limits,
                    region_treatment_limits,
                    profiled,
                    is_cancelled,
                    progress,
                )?;
                let toniator_domain::PatternGeometryResponse::Regions(response) =
                    &output_setting.response
                else {
                    return Err(EvaluationError::new(
                        "region.treatment.identity.response",
                        "ordinary region output requires a region response",
                    ));
                };
                let usage = voronoi_region_site_usage(*site_mechanism_id, &realized.regions)?;
                let toniator_patterns::TypedRegionOutputRealization {
                    regions,
                    paints,
                    fingerprint,
                    diagnostics: realization_diagnostics,
                } = realized;
                return Ok((
                    DocumentRealization::Regions {
                        fingerprint: format!(
                            "{}:{}:{}:{}",
                            VORONOI_REGION_CONTRACT_ID,
                            output_capability.layer_id.0,
                            site_mechanism_id.0,
                            fingerprint
                        ),
                        geometry: Arc::new(GeometryOutput::CanonicalRegions(regions)),
                        paints: shared_sampled_paints(paints),
                        diagnostics: completed_region_diagnostics(
                            response,
                            source,
                            realization_diagnostics,
                            RegionProducerCacheDiagnostics::Voronoi(diagnostics),
                        ),
                    },
                    usage,
                    performance,
                ));
            }
            toniator_domain::RegionSourceIntent::GuideFaces {
                guide_mechanism_id,
                dimensions,
            } => {
                if definition.coverage.guard_steps == 0 {
                    return Err(EvaluationError::new(
                        "region.guide_faces.coverage.guard_steps",
                        "guide-face regions require at least one configured guard step",
                    ));
                }
                let paths = family
                    .structural_path_set()
                    .cloned()
                    .ok_or(EvaluationError::new(
                        "region.guide_faces.identity.paths",
                        "guide-face output requires complete structural guide paths",
                    ))?;
                let result = build_guide_faces_cancellable(
                    GuideFaceRequest {
                        output_layer_id: output_capability.layer_id,
                        guide_mechanism_id: *guide_mechanism_id,
                        dimensions: dimensions.clone(),
                        paths,
                        canvas,
                    },
                    guide_face_limits,
                    is_cancelled,
                )
                .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
                let references: Vec<RegionReference> = result
                    .centroids
                    .iter()
                    .cloned()
                    .map(|(region_id, point)| RegionReference { region_id, point })
                    .collect();
                let (realized, performance) = realize_document_region_output(
                    output_capability,
                    output_setting,
                    &result.regions,
                    &references,
                    source,
                    document.canvas(),
                    channel.mapping,
                    &channel.paint,
                    region_sampling_limits,
                    region_treatment_limits,
                    profiled,
                    is_cancelled,
                    progress,
                )?;
                let toniator_domain::PatternGeometryResponse::Regions(response) =
                    &output_setting.response
                else {
                    return Err(EvaluationError::new(
                        "region.treatment.identity.response",
                        "guide-face output requires a region response",
                    ));
                };
                let toniator_patterns::TypedRegionOutputRealization {
                    regions,
                    paints,
                    fingerprint,
                    diagnostics: realization_diagnostics,
                } = realized;
                return Ok((
                    DocumentRealization::Regions {
                        fingerprint: format!(
                            "{}:{}:{}:{}",
                            GUIDE_FACE_CONTRACT_ID,
                            output_capability.layer_id.0,
                            guide_mechanism_id.0,
                            fingerprint
                        ),
                        geometry: Arc::new(GeometryOutput::CanonicalRegions(regions)),
                        paints: shared_sampled_paints(paints),
                        diagnostics: completed_region_diagnostics(
                            response,
                            source,
                            realization_diagnostics,
                            RegionProducerCacheDiagnostics::GuideFaces,
                        ),
                    },
                    SiteUsageSet::empty_non_site(),
                    performance,
                ));
            }
        }
    }
    if let Some((_site_mechanism_id, program, style)) = output_capability.maze_walls() {
        let ChannelPaint::Solid(_) = channel.paint else {
            return Err(EvaluationError::new(
                "evaluation.maze.paint",
                "maze-wall output requires solid channel paint",
            ));
        };
        toniator_patterns::validate_output_realization_binding(
            plan,
            output_capability,
            output_setting,
        )
        .map_err(EvaluationError::from_pipeline)?;
        let toniator_domain::PatternGeometryResponse::Connected(response) =
            &output_setting.response
        else {
            return Err(EvaluationError::new(
                "pattern.output_layers.setting",
                "maze realization requires a connected response",
            ));
        };
        let request = GridInspectRequest {
            canvas: document.canvas().clone(),
            density: effective.resolved_density,
            rotation_degrees: effective.pattern_rotation_degrees,
            translation_x: effective.translation_x,
            translation_y: effective.translation_y,
            guard_steps: curve_motif_phase_guard_steps(
                definition.coverage.guard_steps,
                &plan.ordered_outputs,
            )?,
            support_radius: required_support_radius_for_outputs(
                document.canvas(),
                effective,
                definition,
                &plan.family,
                &plan.ordered_outputs,
            )?,
            max_family_candidates,
        };
        let maze = evaluate_typed_maze_walls_from_family_cancellable(
            family,
            &request,
            output_capability.layer_id,
            program,
            maze_limits,
            is_cancelled,
        )
        .map_err(EvaluationError::from_pipeline)?;
        let members_by_vertex = maze
            .source_sites
            .iter()
            .map(|site| (site.id, site.source.id))
            .collect::<BTreeMap<_, _>>();
        let usage = SiteUsageSet::new(
            family.site_set().product_mechanism_id(),
            maze.retained_walls
                .iter()
                .flat_map(|wall| [wall.id.first, wall.id.second])
                .filter_map(|vertex| members_by_vertex.get(&vertex).copied())
                .collect(),
        )
        .map_err(EvaluationError::from_pipeline)?;
        drop(members_by_vertex);
        return Ok((
            cache_stroke_realization(
                toniator_patterns::realize_owned_maze_canonical_strokes_cancellable(
                    maze,
                    source,
                    document.canvas(),
                    channel.mapping,
                    toniator_patterns::StrokeResponse {
                        minimum_thickness: response.minimum_thickness,
                        maximum_thickness: response.maximum_thickness,
                        bias: response.bias,
                    },
                    style,
                    max_stroke_profile_samples,
                    max_stroke_outline_segments,
                    is_cancelled,
                )
                .map_err(EvaluationError::from_pipeline)?,
            ),
            usage,
            None,
        ));
    }
    if let Some((_site_mechanism_id, program, style)) = output_capability.connection_paths() {
        let ChannelPaint::Solid(_) = channel.paint else {
            return Err(EvaluationError::new(
                "evaluation.connection.paint",
                "connection output requires solid channel paint",
            ));
        };
        toniator_patterns::validate_output_realization_binding(
            plan,
            output_capability,
            output_setting,
        )
        .map_err(EvaluationError::from_pipeline)?;
        let toniator_domain::PatternGeometryResponse::Connected(response) =
            &output_setting.response
        else {
            return Err(EvaluationError::new(
                "pattern.output_layers.setting",
                "connection realization requires a connected response",
            ));
        };
        let adjacency = program.adjacency();
        let base_support = required_connection_base_support(
            document.canvas(),
            effective,
            definition,
            &plan.family,
        )?;
        let graph = build_typed_site_adjacency_cancellable(
            family,
            base_support,
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: adjacency.maximum_degree as usize,
                maximum_distance: adjacency.maximum_distance,
            },
            adjacency_limits,
            is_cancelled,
        )
        .map_err(EvaluationError::from_pipeline)?;
        let paths = build_connection_paths_cancellable(
            output_capability.layer_id,
            &graph,
            program,
            connection_limits,
            is_cancelled,
        )
        .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
        drop(graph);
        let usage = SiteUsageSet::new(
            family.site_set().product_mechanism_id(),
            paths
                .selected_edges
                .iter()
                .flat_map(|edge| [edge.first, edge.second])
                .collect(),
        )
        .map_err(EvaluationError::from_pipeline)?;
        return Ok((
            cache_stroke_realization(
                toniator_patterns::realize_owned_connection_canonical_strokes_cancellable(
                    paths,
                    source,
                    document.canvas(),
                    channel.mapping,
                    toniator_patterns::StrokeResponse {
                        minimum_thickness: response.minimum_thickness,
                        maximum_thickness: response.maximum_thickness,
                        bias: response.bias,
                    },
                    style,
                    max_stroke_profile_samples,
                    max_stroke_outline_segments,
                    is_cancelled,
                )
                .map_err(EvaluationError::from_pipeline)?,
            ),
            usage,
            None,
        ));
    }
    let authored_output = definition
        .output_layers
        .iter()
        .find(|output| output.id == output_capability.layer_id)
        .ok_or(EvaluationError::new(
            "evaluation.output_binding",
            "realization capability targets a missing authored output",
        ))?;
    if let PatternOutputRealization::CurveMotifPaths {
        structure_id,
        style,
        mirror_alternate_rows,
        alternate_row_phase,
        ..
    } = authored_output.realization
    {
        let toniator_domain::PatternGeometryResponse::Connected(response) =
            &output_setting.response
        else {
            return Err(EvaluationError::new(
                "evaluation.stroke.response",
                "Curve Motif output requires the connected response branch",
            ));
        };
        let ChannelPaint::Solid(_) = channel.paint else {
            return Err(EvaluationError::new(
                "evaluation.stroke.paint",
                "Curve Motif output requires solid channel paint",
            ));
        };
        let structure = document
            .authored_structure(structure_id)
            .ok_or(EvaluationError::new(
                "evaluation.curve_motif.resource",
                "Curve Motif output references a missing document-owned open path",
            ))?;
        let motif = CurvePath::from_authored_structure(structure).map_err(|_| {
            EvaluationError::new(
                "evaluation.curve_motif.resource",
                "Curve Motif output requires a valid document-owned open path",
            )
        })?;
        let realization = toniator_patterns::realize_curve_motif_canonical_strokes_cancellable(
            family.family_fingerprint(),
            family.site_set(),
            &motif,
            structure_id,
            style,
            mirror_alternate_rows,
            alternate_row_phase,
            source,
            document.canvas(),
            channel.mapping,
            toniator_patterns::StrokeResponse {
                minimum_thickness: response.minimum_thickness,
                maximum_thickness: response.maximum_thickness,
                bias: response.bias,
            },
            max_stroke_profile_samples,
            max_stroke_outline_segments,
            is_cancelled,
            progress,
        )
        .map_err(EvaluationError::from_pipeline)?;
        return Ok((
            cache_stroke_realization(realization),
            SiteUsageSet::empty_non_site(),
            None,
        ));
    }
    if matches!(
        authored_output.realization,
        PatternOutputRealization::GuidePaths { .. }
            | PatternOutputRealization::ParametricPaths { .. }
    ) {
        if !matches!(
            output_setting.response,
            toniator_domain::PatternGeometryResponse::Connected(_)
        ) {
            return Err(EvaluationError::new(
                "evaluation.stroke.response",
                "guide-path output requires the connected response branch",
            ));
        }
        let ChannelPaint::Solid(_) = channel.paint else {
            return Err(EvaluationError::new(
                "evaluation.stroke.paint",
                "guide-path output requires solid channel paint",
            ));
        };
        return Ok((
            cache_stroke_realization(
                toniator_patterns::realize_typed_canonical_stroke_output_cancellable(
                    family,
                    plan,
                    output_capability,
                    output_setting,
                    source,
                    document.canvas(),
                    channel.mapping,
                    max_stroke_profile_samples,
                    max_stroke_outline_segments,
                    is_cancelled,
                )
                .map_err(EvaluationError::from_pipeline)?
                .realization
                .output,
            ),
            SiteUsageSet::empty_non_site(),
            None,
        ));
    }
    let realization = if matches!(
        authored_output.realization,
        PatternOutputRealization::MarkPrototype { .. }
    ) {
        cache_canonical_mark_realization(
            toniator_patterns::realize_typed_canonical_mark_output_cancellable(
                document,
                family,
                plan,
                output_capability,
                output_setting,
                source,
                document.canvas(),
                channel.mapping,
                matches!(channel.paint, ChannelPaint::SampledSource),
                effective.shape_rotation_degrees,
                max_transformed_curve_segment_instances,
                is_cancelled,
            )
            .map_err(EvaluationError::from_pipeline)?
            .realization,
        )
    } else {
        match channel.paint {
            ChannelPaint::Solid(_) => cache_mapped_realization(
                toniator_patterns::realize_typed_mapped_output_cancellable(
                    family,
                    plan,
                    output_capability,
                    output_setting,
                    source,
                    document.canvas(),
                    channel.mapping,
                    effective.shape_rotation_degrees,
                    is_cancelled,
                )
                .map_err(EvaluationError::from_pipeline)?
                .realization,
            ),
            ChannelPaint::SampledSource => cache_source_color_realization(
                toniator_patterns::realize_typed_source_color_output_cancellable(
                    family,
                    plan,
                    output_capability,
                    output_setting,
                    source,
                    document.canvas(),
                    channel.mapping,
                    effective.shape_rotation_degrees,
                    is_cancelled,
                )
                .map_err(EvaluationError::from_pipeline)?
                .realization,
            ),
        }
    };
    let usage = mark_site_usage(family, &realization)?;
    Ok((realization, usage, None))
}

/// Derives every unique Voronoi co-owner retained after treatment and alpha suppression.
///
/// The completed canonical set is still unclipped at this boundary. Empty treated output is a
/// valid empty same-mechanism usage set, and shared co-owners are sorted and deduplicated by the
/// domain-owned usage constructor.
///
/// # Errors
///
/// Returns a stable usage-identity diagnostic if a retained owner does not belong to the exact
/// output site mechanism.
fn voronoi_region_site_usage(
    site_mechanism_id: toniator_domain::PatternMechanismId,
    regions: &toniator_patterns::CanonicalRegionSet,
) -> Result<SiteUsageSet, EvaluationError> {
    SiteUsageSet::new(
        site_mechanism_id,
        regions
            .regions()
            .iter()
            .flat_map(|region| match &region.id.source_id {
                toniator_patterns::CanonicalRegionSourceId::SiteOwners(owners) => owners.clone(),
                toniator_patterns::CanonicalRegionSourceId::GuideBoundary(_) => Vec::new(),
            })
            .collect(),
    )
    .map_err(EvaluationError::from_pipeline)
}

/// Derives positive mark membership from completed geometry before renderer clipping.
///
/// Generalized marks retain family IDs directly. The circular compatibility realizers retain the
/// same exact center bits and family order, so this boundary recovers only positive-radius emitted
/// sites without inventing a second family or using canvas visibility.
///
/// # Errors
///
/// Returns a stable site-usage mechanism diagnostic if completed mark provenance is inconsistent.
fn mark_site_usage(
    family: &TypedFamilyOutput,
    realization: &DocumentRealization,
) -> Result<SiteUsageSet, EvaluationError> {
    let mut members = BTreeSet::new();
    match realization.geometry().as_ref() {
        GeometryOutput::CircularMarks(marks) => {
            for mark in marks {
                if mark.radius > 0.0
                    && let Some(site) = family.site_set().iter().find(|site| {
                        site.position.x.to_bits() == mark.center.x.to_bits()
                            && site.position.y.to_bits() == mark.center.y.to_bits()
                    })
                {
                    members.insert(site.id);
                }
            }
        }
        GeometryOutput::CanonicalMarks(marks) => {
            for mark in marks {
                match mark {
                    CanonicalMark::Circle {
                        source_site_id,
                        radius,
                        ..
                    } if *radius > 0.0 => {
                        members.insert(*source_site_id);
                    }
                    CanonicalMark::ClosedPath(path) => {
                        members.insert(path.source_site_id);
                    }
                    CanonicalMark::Circle { .. } => {}
                }
            }
        }
        GeometryOutput::CanonicalStrokes(_) | GeometryOutput::CanonicalRegions(_) => {}
    }
    SiteUsageSet::new(
        family.site_set().product_mechanism_id(),
        members.into_iter().collect(),
    )
    .map_err(EvaluationError::from_pipeline)
}

/// Converts borrowed cached channel-output units into one renderer-owned layer without resource lookup.
///
/// This boundary preserves cached realization/provenance ownership for reuse
/// and diagnostics. It clones only the canonical geometry and sampled-paint
/// payload required by the renderer's independent scene ownership, retaining
/// supplied painter order exactly.
///
/// # Errors
///
/// Returns stable layer validation failures before a scene can publish.
fn document_render_layer(
    channel: &ModeledChannelState,
    outputs: &[Arc<DocumentOutputRealization>],
) -> Result<RenderLayer, EvaluationError> {
    let color = match &channel.paint {
        ChannelPaint::Solid(color) => color.clone(),
        ChannelPaint::SampledSource => toniator_domain::ColorValue {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
    };
    let outputs = outputs
        .iter()
        .map(|output| document_render_output(output))
        .collect::<Vec<_>>();
    RenderLayer::new_outputs(channel.id, channel.visible, color, channel.opacity, outputs)
        .map_err(EvaluationError::from_render)
}

/// Shares one cached realization payload with its ordered renderer output.
///
/// This clones only `Arc` handles. Geometry, sampled paint, cache identity,
/// diagnostics, and usage stay immutable; the containing layer performs the
/// sole renderer validation before publication.
fn document_render_output(
    output: &DocumentOutputRealization,
) -> toniator_render::RenderOutputLayer {
    let _capability = &output.capability;
    let _setting = &output.setting;
    toniator_render::RenderOutputLayer::from_shared(
        output.output_layer_id,
        Arc::clone(output.realization.geometry()),
        output.realization.primitive_paints().map(Arc::clone),
    )
}
fn aggregate_document_identity<'a>(
    prefix: &str,
    model: HalftoneChannelModel,
    values: impl IntoIterator<Item = (HalftoneChannelRole, ChannelId, &'a str)>,
) -> String {
    let mut output = format!("{prefix}:{model:?}");
    for (role, id, value) in values {
        output.push_str(&format!(":{role:?}:{}:{value}", id.0));
    }
    output
}

// Stage 9D keeps exactly five last-successful aggregate cache slots. The two
// collections deliberately keep entries per channel key so a document edit can
// reuse unaffected immutable artifacts without making a cache authoritative.
#[derive(Clone, Debug, PartialEq)]
struct DocumentFamilyContentKey {
    canvas: (u64, u64),
    density: (u64, u64),
    rotation: u64,
    translation: (u64, u64),
    guard_steps: u32,
    required_support_radius: u64,
    definition: FamilyDefinitionKey,
    structural_source: Option<RealizationSourceIdentity>,
}

#[derive(Clone, Debug, PartialEq)]
struct DocumentFamilyCacheKey {
    content: DocumentFamilyContentKey,
    candidate_limit: usize,
}

/// Reports whether a document-family cache entry has identical content and a sufficiently broad envelope.
fn document_family_key_supports(
    candidate: &DocumentFamilyCacheKey,
    requested: &DocumentFamilyCacheKey,
) -> bool {
    let candidate_support = f64::from_bits(candidate.content.required_support_radius);
    let requested_support = f64::from_bits(requested.content.required_support_radius);
    let mut comparable = candidate.clone();
    comparable.content.required_support_radius = requested.content.required_support_radius;
    comparable == *requested && candidate_support >= requested_support
}

/// Adds one longitudinal guard interval when phase-shifted Curve Motif rows need padded coverage.
///
/// The authored coverage guard remains the row-layout authority. This derived
/// interval exists only while producing the family envelope and is included in
/// the family key, so phase-shifted output cannot reuse an under-covered cache.
///
/// # Errors
///
/// Returns a stable overflow diagnostic if a persisted guard count cannot
/// accommodate the extra phase coverage interval.
fn curve_motif_phase_guard_steps(
    authored_guard_steps: u32,
    outputs: &[toniator_patterns::OutputCapability],
) -> Result<u32, EvaluationError> {
    let needs_extra_interval = outputs.iter().any(|output| {
        matches!(
            &output.payload,
            toniator_patterns::OutputCapabilityPayload::CurveMotifPaths {
                alternate_row_phase: Some(_),
                ..
            }
        )
    });
    if needs_extra_interval {
        authored_guard_steps
            .checked_add(1)
            .ok_or(EvaluationError::new(
                "curve_motif.coverage.guard_steps",
                "Curve Motif phase coverage guard overflows the persisted guard count",
            ))
    } else {
        Ok(authored_guard_steps)
    }
}

/// Adds exactly one derived guard interval only for phase-shifted Curve Motif outputs.
#[cfg(test)]
#[test]
fn curve_motif_phase_guard_steps_adds_one_interval_and_rejects_overflow() {
    let phased = toniator_patterns::OutputCapability {
        layer_id: toniator_domain::PatternOutputLayerId(1),
        source_filter: toniator_domain::SiteUseFilter::All,
        consumes: toniator_patterns::StructuralProductCapability::AlongGuideSites,
        payload: toniator_patterns::OutputCapabilityPayload::CurveMotifPaths {
            site_mechanism_id: toniator_domain::PatternMechanismId(2),
            structure_id: toniator_domain::AuthoredStructureId(3),
            style: toniator_domain::PathStrokeStyle::default(),
            mirror_alternate_rows: true,
            alternate_row_phase: Some(0.25),
        },
    };
    let mut unphased = phased.clone();
    let toniator_patterns::OutputCapabilityPayload::CurveMotifPaths {
        alternate_row_phase,
        ..
    } = &mut unphased.payload
    else {
        unreachable!("fixture remains Curve Motif")
    };
    *alternate_row_phase = None;
    assert_eq!(
        curve_motif_phase_guard_steps(2, &[unphased]).expect("base guard"),
        2
    );
    assert_eq!(
        curve_motif_phase_guard_steps(2, std::slice::from_ref(&phased)).expect("phase guard"),
        3
    );
    assert_eq!(
        curve_motif_phase_guard_steps(u32::MAX, &[phased])
            .expect_err("phase guard overflow rejects")
            .path(),
        "curve_motif.coverage.guard_steps"
    );
}

/// Builds one modeled-channel family cache key including resolved authored guide content.
///
/// This remains a cache identity only: document capability resolution supplies `family`,
/// and source decoding remains owned by the caller.
///
/// # Errors
///
/// Returns the family nominal-cell preflight error before cache lookup or allocation.
fn document_family_cache_key(
    canvas: &CanvasSpec,
    definition: &toniator_domain::PatternDefinition,
    effective: &toniator_domain::EffectiveChannelPatternInstance,
    limits: EvaluationLimits,
    family: &FamilyCapability,
    outputs: &[toniator_patterns::OutputCapability],
    source: &SourceField,
) -> Result<DocumentFamilyCacheKey, EvaluationError> {
    Ok(DocumentFamilyCacheKey {
        content: DocumentFamilyContentKey {
            canvas: (canvas.width.to_bits(), canvas.height.to_bits()),
            density: (
                effective.density.density.to_bits(),
                effective.density.aspect.to_bits(),
            ),
            rotation: effective.pattern_rotation_degrees.to_bits(),
            translation: (
                effective.translation_x.to_bits(),
                effective.translation_y.to_bits(),
            ),
            guard_steps: curve_motif_phase_guard_steps(definition.coverage.guard_steps, outputs)?,
            required_support_radius: required_support_radius_for_outputs(
                canvas, effective, definition, family, outputs,
            )?
            .to_bits(),
            definition: FamilyDefinitionKey {
                resolved_guide_content: family.generic_guides.as_ref().map(resolved_guide_identity),
                ..family_definition_key(definition)
            },
            structural_source: family_requires_decoded_source(family)
                .then(|| realization_source_identity(source.identity())),
        },
        candidate_limit: limits.max_family_candidates(),
    })
}

/// Derives legacy family support from the accepted fill ceiling, independent of current response intent.
fn required_support_radius_legacy(
    canvas: &CanvasSpec,
    effective: &toniator_domain::EffectiveChannelPatternInstance,
    definition: &toniator_domain::PatternDefinition,
    family: &FamilyCapability,
) -> Result<f64, EvaluationError> {
    required_support_radius_from_fill(
        canvas,
        &effective.resolved_density,
        MAXIMUM_NORMALIZED_MARK_FILL,
        definition.coverage.additional_margin,
        family,
    )
}

/// Derives the maximum modeled family support request across every ordered output capability.
///
/// A shared family must be broad enough for all independently realized outputs. Taking a maximum
/// is painter-order neutral, while the one-output result remains byte-for-byte the former request.
///
/// # Errors
///
/// Returns a stable capability or geometry support diagnostic before family allocation.
fn required_support_radius_for_outputs(
    canvas: &CanvasSpec,
    effective: &toniator_domain::EffectiveChannelPatternInstance,
    definition: &toniator_domain::PatternDefinition,
    family: &FamilyCapability,
    outputs: &[toniator_patterns::OutputCapability],
) -> Result<f64, EvaluationError> {
    outputs
        .iter()
        .map(|output| {
            required_support_radius_for_output(canvas, effective, definition, family, output)
        })
        .try_fold(None, |maximum, candidate| {
            let candidate = candidate?;
            Ok(Some(
                maximum.map_or(candidate, |current: f64| current.max(candidate)),
            ))
        })?
        .ok_or(EvaluationError::new(
            "evaluation.output_gate",
            "pattern pipeline must expose at least one output capability",
        ))
}

/// Derives one output's conservative family support request from its explicit capability.
///
/// Ordinary straight/grid Voronoi producers already expand their guard candidates inside the
/// family evaluator, so their base support retains only authored additional margin. Finite
/// parametric and random-site producers require the explicit guard envelope here because their
/// fixed curve or stochastic site source cannot synthesize it from the output alone. Every Region
/// response then adds only its positive normalized-fill outward extent.
///
/// # Errors
///
/// Returns patterns-owned guide-spacing or nominal-cell diagnostics without evaluating a family.
fn required_support_radius_for_output(
    canvas: &CanvasSpec,
    effective: &toniator_domain::EffectiveChannelPatternInstance,
    definition: &toniator_domain::PatternDefinition,
    family: &FamilyCapability,
    output: &toniator_patterns::OutputCapability,
) -> Result<f64, EvaluationError> {
    if let Some((_site_mechanism_id, program, _style)) = output.connection_paths() {
        return Ok(
            required_connection_base_support(canvas, effective, definition, family)?
                + f64::from(definition.coverage.guard_steps) * program.adjacency().maximum_distance,
        );
    }
    if output.maze_walls().is_some() || output.guide_paths().is_some() {
        return Ok(
            maximum_emitted_guide_spacing(family, canvas, &effective.resolved_density)
                .map_err(EvaluationError::from_pipeline)?
                + definition.coverage.additional_margin,
        );
    }
    if let Some(region_source) = output.regions() {
        if definition.coverage.guard_steps == 0 {
            return Err(EvaluationError::new(
                "region.voronoi.coverage.guard_steps",
                "ordinary Voronoi regions require at least one configured guard step",
            ));
        }
        let response = region_response_for_output(effective, output.layer_id)?;
        let nominal_extent = match region_source {
            toniator_domain::RegionSourceIntent::GuideFaces { .. } => {
                maximum_emitted_guide_spacing(family, canvas, &effective.resolved_density)
                    .map_err(EvaluationError::from_pipeline)?
            }
            toniator_domain::RegionSourceIntent::VoronoiSites { .. } => {
                maximum_nominal_cell_diameter(family, canvas, &effective.resolved_density)
                    .map_err(EvaluationError::from_pipeline)?
            }
        };
        let base_support = match region_source {
            toniator_domain::RegionSourceIntent::GuideFaces { .. } => {
                checked_region_support_add(definition.coverage.additional_margin, nominal_extent)?
            }
            toniator_domain::RegionSourceIntent::VoronoiSites { .. } => {
                let parametric_guard = matches!(
                    family.product,
                    toniator_patterns::StructuralProductCapability::AlongGuideSites
                ) && family.parametric_curve.is_some();
                let producer_guard = parametric_guard
                    || family.product
                        == toniator_patterns::StructuralProductCapability::RandomSites;
                if producer_guard {
                    checked_region_support_add(
                        definition.coverage.additional_margin,
                        f64::from(definition.coverage.guard_steps) * nominal_extent,
                    )?
                } else {
                    checked_region_support_add(definition.coverage.additional_margin, 0.0)?
                }
            }
        };
        return checked_region_support_add(
            base_support,
            region_treatment_outward_support(response, nominal_extent)?,
        );
    }
    required_support_radius_from_fill(
        canvas,
        &effective.resolved_density,
        MAXIMUM_NORMALIZED_MARK_FILL,
        definition.coverage.additional_margin,
        family,
    )
}

/// Resolves the one effective region response paired with an ordered structural output.
///
/// # Errors
///
/// Returns `region.treatment.coverage.response` before family allocation when an effective
/// bundle is malformed or its setting is not a typed region response.
fn region_response_for_output(
    effective: &toniator_domain::EffectiveChannelPatternInstance,
    output_layer_id: toniator_domain::PatternOutputLayerId,
) -> Result<&RegionGeometryResponse, EvaluationError> {
    let setting = effective
        .output_settings
        .iter()
        .find(|setting| setting.output_layer_id == output_layer_id)
        .ok_or(EvaluationError::new(
            "region.treatment.coverage.response",
            "region output has no matching effective response",
        ))?;
    match &setting.response {
        toniator_domain::PatternGeometryResponse::Regions(response) => Ok(response),
        _ => Err(EvaluationError::new(
            "region.treatment.coverage.response",
            "region output requires a typed region response",
        )),
    }
}

/// Computes a conservative response-owned positive-region outward envelope without geometry work.
///
/// Both resize algorithms use the same normalized radius multiplier. The producer's nominal
/// extent is therefore scaled only by `maximum_fill - 1`, never by an inter-region gap or a
/// complement-space measurement.
///
/// # Errors
///
/// Returns a stable `region.treatment.coverage.*` diagnostic for invalid or overflowing numeric
/// input before a family candidate or region can be allocated.
fn region_treatment_outward_support(
    response: &RegionGeometryResponse,
    nominal_extent: f64,
) -> Result<f64, EvaluationError> {
    if !nominal_extent.is_finite() || nominal_extent <= 0.0 {
        return Err(EvaluationError::new(
            "region.treatment.coverage.nominal_extent",
            "region treatment requires a finite positive producer extent",
        ));
    }
    if !response.maximum_fill.is_finite() {
        return Err(EvaluationError::new(
            "region.resize.coverage.maximum_fill",
            "region maximum fill must be finite",
        ));
    }
    let outward = (response.maximum_fill - 1.0).max(0.0) * nominal_extent;
    if !outward.is_finite() {
        return Err(EvaluationError::new(
            "region.treatment.coverage.support",
            "region treatment support extension overflows",
        ));
    }
    Ok(outward)
}

/// Adds finite family and treatment envelopes before any family allocation.
///
/// # Errors
///
/// Returns `region.treatment.coverage.support` when the combined support is negative, nonfinite,
/// or overflows, preventing an invalid request from reaching the family evaluator.
fn checked_region_support_add(base: f64, extension: f64) -> Result<f64, EvaluationError> {
    let support = base + extension;
    if !base.is_finite()
        || base < 0.0
        || !extension.is_finite()
        || extension < 0.0
        || !support.is_finite()
    {
        return Err(EvaluationError::new(
            "region.treatment.coverage.support",
            "region treatment requires finite nonnegative family support",
        ));
    }
    Ok(support)
}

/// Returns the connected-stroke family envelope before topology guard expansion.
///
/// # Errors
///
/// Preserves patterns-owned nominal-cell and guide-spacing diagnostics before engine cache lookup
/// or graph construction; the result includes authored additional margin but excludes only guard
/// steps times the program maximum distance.
fn required_connection_base_support(
    canvas: &CanvasSpec,
    effective: &toniator_domain::EffectiveChannelPatternInstance,
    definition: &toniator_domain::PatternDefinition,
    family: &FamilyCapability,
) -> Result<f64, EvaluationError> {
    let basis = if family.product == toniator_patterns::StructuralProductCapability::RandomSites {
        maximum_nominal_cell_diameter(family, canvas, &effective.resolved_density)
    } else {
        maximum_emitted_guide_spacing(family, canvas, &effective.resolved_density)
    }
    .map_err(EvaluationError::from_pipeline)?;
    Ok(basis + definition.coverage.additional_margin)
}

/// Computes the conservative family-specific nominal-cell bound before any allocation.
fn required_support_radius_from_fill(
    canvas: &CanvasSpec,
    density: &toniator_domain::ResolvedDensityMetric2D,
    maximum_fill: f64,
    additional_margin: f64,
    family: &FamilyCapability,
) -> Result<f64, EvaluationError> {
    let diameter = maximum_nominal_cell_diameter(family, canvas, density)
        .map_err(EvaluationError::from_pipeline)?;
    Ok(maximum_fill * diameter / 2.0 + additional_margin)
}

#[derive(Clone, Debug, PartialEq)]
struct DocumentRealizationCacheKey {
    family_content: DocumentFamilyContentKey,
    output_layer_id: toniator_domain::PatternOutputLayerId,
    contract: RealizationContractKey,
    referenced_usage_fingerprint: Option<String>,
    resolved_shape_content: String,
    region_producer: Option<RegionProducerCacheIdentity>,
    source_identity: Option<RealizationSourceIdentity>,
    mapping: Option<String>,
    response: DocumentResponseIdentity,
    sampled_paint: bool,
}

/// Constructs the independent cache identity for one ordered output realization.
///
/// The key excludes aggregate scene state while including every source, mapping, response,
/// algorithm, and bounded-work input that can change this output's canonical geometry. Region
/// outputs always retain their normalized source-sampling intent in this identity.
///
/// # Errors
///
/// Returns the authoritative shape-content diagnostic before a cache candidate can be installed.
#[allow(clippy::too_many_arguments)]
fn document_realization_cache_key(
    document: &toniator_domain::Document,
    definition: &PatternDefinition,
    channel: &ModeledChannelState,
    effective: &toniator_domain::EffectiveChannelPatternInstance,
    family_key: &DocumentFamilyCacheKey,
    family: &TypedFamilyOutput,
    source: &SourceField,
    output_setting: &toniator_domain::EffectivePatternOutputSettings,
    referenced_usage: Option<&SiteUsageSet>,
    limits: EvaluationLimits,
) -> Result<DocumentRealizationCacheKey, EvaluationError> {
    let sampling_required = output_sampling_required(&output_setting.response, &channel.paint);
    let region_sampling_limits = sampling_required.then(|| limits.region_sampling_limits());
    let region_treatment_limits = matches!(
        &output_setting.response,
        toniator_domain::PatternGeometryResponse::Regions(_)
    )
    .then(|| limits.region_treatment_limits());
    Ok(DocumentRealizationCacheKey {
        family_content: family_key.content.clone(),
        output_layer_id: output_setting.output_layer_id,
        contract: realization_contract_key_for_output(definition, output_setting.output_layer_id)?,
        referenced_usage_fingerprint: referenced_usage.map(|usage| usage.fingerprint().to_owned()),
        resolved_shape_content: resolved_shape_content_identity(
            document,
            definition,
            output_setting.output_layer_id,
        )?,
        region_producer: region_producer_cache_identity(definition, family, output_setting),
        source_identity: sampling_required.then(|| realization_source_identity(source.identity())),
        mapping: sampling_required.then(|| {
            format!(
                "{:?}:{:?}:{}:{}:{}",
                channel.mapping.component,
                channel.mapping.placement,
                channel.mapping.inverted,
                channel.mapping.gain.to_bits(),
                channel.mapping.bias.to_bits()
            )
        }),
        response: match &output_setting.response {
            toniator_domain::PatternGeometryResponse::Marks(response) => {
                DocumentResponseIdentity::Marks {
                    minimum: response.minimum_fill.to_bits(),
                    maximum: response.maximum_fill.to_bits(),
                    shape_rotation: effective.shape_rotation_degrees.to_bits(),
                }
            }
            toniator_domain::PatternGeometryResponse::Connected(response) => {
                DocumentResponseIdentity::Connected {
                    minimum: response.minimum_thickness.to_bits(),
                    maximum: response.maximum_thickness.to_bits(),
                    shape_rotation: effective.shape_rotation_degrees.to_bits(),
                    outline_contract: toniator_patterns::CANONICAL_STROKE_OUTLINE_CONTRACT_ID,
                    profile_limit: limits.max_stroke_profile_samples(),
                    outline_segment_limit: limits.max_stroke_outline_segments(),
                    connection_contracts: Box::new(connection_cache_contracts(
                        definition,
                        output_setting.output_layer_id,
                        limits.site_adjacency_limits(),
                        limits.connection_path_limits(),
                    )),
                    maze_contracts: Box::new(maze_cache_contracts(
                        definition,
                        output_setting.output_layer_id,
                        limits.maze_limits(),
                    )),
                }
            }
            toniator_domain::PatternGeometryResponse::Regions(response) => match definition
                .output_layers
                .iter()
                .find(|output| output.id() == output_setting.output_layer_id)
            {
                Some(PatternOutputLayer {
                    realization:
                        PatternOutputRealization::Regions {
                            source: toniator_domain::RegionSourceIntent::GuideFaces { .. },
                        },
                    ..
                }) => DocumentResponseIdentity::GuideFaces {
                    contract: GUIDE_FACE_CONTRACT_ID,
                    limits: limits.guide_face_limits(),
                    sampling_limits: region_sampling_limits,
                    treatment_limits: region_treatment_limits,
                    treatment_contract: region_treatment_limits
                        .is_some()
                        .then_some(REGION_TREATMENT_CONTRACT_ID),
                    response: region_response_identity(response),
                },
                _ => DocumentResponseIdentity::Regions {
                    contract: VORONOI_REGION_CONTRACT_ID,
                    limits: limits.voronoi_region_limits(),
                    sampling_limits: region_sampling_limits,
                    treatment_limits: region_treatment_limits,
                    treatment_contract: region_treatment_limits
                        .is_some()
                        .then_some(REGION_TREATMENT_CONTRACT_ID),
                    response: region_response_identity(response),
                },
            },
        },
        sampled_paint: matches!(channel.paint, ChannelPaint::SampledSource),
    })
}

/// Reports whether one output's realization consumes decoded source and mapping identity.
///
/// Every current output maps geometry or derives sampled paint. In particular, every region
/// output samples its complete untreated producer region to resolve its normalized fill.
fn output_sampling_required(
    response: &toniator_domain::PatternGeometryResponse,
    paint: &ChannelPaint,
) -> bool {
    let _ = (response, paint);
    true
}

/// Cache-only identity of the complete untreated region producer and its deterministic references.
///
/// This is constructed from the immutable family result after family-cache lookup. Voronoi stores
/// each normalized site coordinate directly; Guide Faces store the complete ordered structural
/// path authority from which their analytic centroids are uniquely derived. Neither value is
/// persisted or treated as renderer geometry.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RegionProducerCacheIdentity {
    family_fingerprint: String,
    reference_bits: String,
}

/// Builds cache-only untreated producer identity without sampling or constructing treated geometry.
fn region_producer_cache_identity(
    definition: &PatternDefinition,
    family: &TypedFamilyOutput,
    setting: &toniator_domain::EffectivePatternOutputSettings,
) -> Option<RegionProducerCacheIdentity> {
    let output = definition
        .output_layers
        .iter()
        .find(|output| output.id() == setting.output_layer_id)?;
    let PatternOutputRealization::Regions { source } = &output.realization else {
        return None;
    };
    let reference_bits = match source {
        toniator_domain::RegionSourceIntent::VoronoiSites { .. } => family
            .site_set()
            .sites()
            .iter()
            .map(|site| {
                format!(
                    "{:?}:{:016x}:{:016x}",
                    site.id,
                    site.position.x.to_bits(),
                    site.position.y.to_bits()
                )
            })
            .collect::<Vec<_>>()
            .join(":"),
        toniator_domain::RegionSourceIntent::GuideFaces { .. } => family
            .structural_path_set()
            .map(guide_face_reference_bits)
            .unwrap_or_default(),
    };
    Some(RegionProducerCacheIdentity {
        family_fingerprint: family.family_fingerprint().to_owned(),
        reference_bits,
    })
}

/// Encodes ordered Guide-Face source construction points as exact IEEE reference bits.
///
/// The Guide-Face producer derives each analytic area centroid solely from these immutable path
/// segments and IDs. Cache identity therefore avoids decimal debug formatting and changes when a
/// centroid-producing boundary changes, without caching a derived face table.
fn guide_face_reference_bits(paths: &toniator_patterns::StructuralPathSet) -> String {
    let mut bits = String::new();
    for path in paths.paths() {
        bits.push_str(&format!("{:?}", path.id));
        for segment in path.path.segments() {
            match segment {
                CurveSegment::Line(line) => {
                    append_reference_point_bits(&mut bits, line.start());
                    append_reference_point_bits(&mut bits, line.end());
                }
                CurveSegment::CubicBezier(cubic) => {
                    append_reference_point_bits(&mut bits, cubic.start());
                    append_reference_point_bits(&mut bits, cubic.control_1());
                    append_reference_point_bits(&mut bits, cubic.control_2());
                    append_reference_point_bits(&mut bits, cubic.end());
                }
            }
        }
    }
    bits
}

/// Appends one finite producer point as exact coordinate bits to a cache-only identity buffer.
fn append_reference_point_bits(buffer: &mut String, point: Point2) {
    buffer.push_str(&format!(
        ":{:016x}:{:016x}",
        point.x.to_bits(),
        point.y.to_bits()
    ));
}

/// Tagged response identity prevents mark and connected-stroke realizations from colliding.
#[derive(Clone, Debug, PartialEq)]
enum DocumentResponseIdentity {
    Marks {
        minimum: u64,
        maximum: u64,
        shape_rotation: u64,
    },
    Connected {
        minimum: u64,
        maximum: u64,
        shape_rotation: u64,
        outline_contract: &'static str,
        profile_limit: usize,
        outline_segment_limit: usize,
        connection_contracts: Box<Option<ConnectionCacheContracts>>,
        maze_contracts: Box<Option<MazeCacheContracts>>,
    },
    Regions {
        contract: &'static str,
        limits: VoronoiRegionLimits,
        sampling_limits: Option<RegionSamplingLimits>,
        treatment_limits: Option<RegionTreatmentLimits>,
        treatment_contract: Option<&'static str>,
        response: RegionResponseIdentity,
    },
    GuideFaces {
        contract: &'static str,
        limits: GuideFaceLimits,
        sampling_limits: Option<RegionSamplingLimits>,
        treatment_limits: Option<RegionTreatmentLimits>,
        treatment_contract: Option<&'static str>,
        response: RegionResponseIdentity,
    },
}

/// Cache-key-only authored/effective region treatment identity without a derived sample table.
#[derive(Clone, Debug, PartialEq, Eq)]
enum RegionResponseIdentity {
    Resize {
        algorithm: toniator_domain::RegionResizeAlgorithm,
        sampling: toniator_domain::RegionSamplingStrategy,
        minimum: u64,
        maximum: u64,
    },
}

/// Converts validated effective region response input into a cache-only typed identity.
fn region_response_identity(response: &RegionGeometryResponse) -> RegionResponseIdentity {
    RegionResponseIdentity::Resize {
        algorithm: response.algorithm,
        sampling: response.sampling,
        minimum: response.minimum_fill.to_bits(),
        maximum: response.maximum_fill.to_bits(),
    }
}

/// Geometry-owned contracts that must invalidate only connection realization cache entries.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ConnectionCacheContracts {
    site_adjacency: &'static str,
    path_selection: &'static str,
    trail_decomposition: &'static str,
    program_selection: &'static str,
    adjacency_limits: SiteAdjacencyLimits,
    connection_limits: ConnectionPathLimits,
}

/// Geometry-owned contracts and limits that distinguish maze realization cache entries only.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MazeCacheContracts {
    arrangement: &'static str,
    algorithm: toniator_domain::GridMazeAlgorithm,
    seed: u32,
    limits: MazeLimits,
}

#[derive(Clone, Default)]
struct DocumentDerivedCache {
    decoded_source: Option<(SourceCacheKey, Arc<SourceField>)>,
    families: Vec<(DocumentFamilyCacheKey, Arc<TypedFamilyOutput>)>,
    realizations: Vec<(DocumentRealizationCacheKey, Arc<DocumentOutputRealization>)>,
    scene: Option<(String, Arc<RenderScene>)>,
    raster: Option<(String, Arc<RasterSurface>)>,
}

#[derive(Clone)]
struct DocumentCacheTransaction {
    decoded_source: Option<(SourceCacheKey, Arc<SourceField>)>,
    families: Vec<(DocumentFamilyCacheKey, Arc<TypedFamilyOutput>)>,
    realizations: Vec<(DocumentRealizationCacheKey, Arc<DocumentOutputRealization>)>,
    scene: Option<(String, Arc<RenderScene>)>,
    raster: Option<(String, Arc<RasterSurface>)>,
}

struct CachedDocumentEvaluation {
    result: EvaluationResult,
    diagnostics: DocumentCacheDiagnostics,
    transaction: DocumentCacheTransaction,
}

impl DocumentDerivedCache {
    fn snapshot(&self) -> Self {
        self.clone()
    }

    fn commit(&mut self, transaction: DocumentCacheTransaction) {
        if let Some(source) = transaction.decoded_source {
            self.decoded_source = Some(source);
        }
        self.families = transaction.families;
        self.realizations = transaction.realizations;
        if let Some(scene) = transaction.scene {
            self.scene = Some(scene);
        }
        if let Some(raster) = transaction.raster {
            self.raster = Some(raster);
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::{
        fs,
        path::Path,
        process::Command,
        sync::{Arc, atomic::AtomicBool, mpsc},
        thread,
        time::{Duration, Instant},
    };

    use toniator_domain::{
        AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureId,
        AuthoredStructureKind, CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternInstance,
        ChannelPatternLayoutDelta, ChannelSourceMapping, ChannelState, ChannelTopologyTemplate,
        ColorValue, ConnectedGeometryResponse, ConnectionAdjacencyIntent, ConnectionProgram,
        CoveragePolicy, CurveRepetition, CurveWinding, DensityMetric2D, Document, DocumentCommand,
        DocumentHistory, DocumentId, DocumentPatternSettings, DocumentSession,
        GeneralizedSiteProduct, GuideDimension, GuideDimensionId, GuidePrototype, GuideRepetition,
        HalftoneChannelModel, MarkGeometryResponse, MarkOrientation, MarkPrototype, OffsetCleanup,
        ParametricCurve, PathStrokeStyle, PatternDefinition, PatternDefinitionBundle,
        PatternDefinitionEdit, PatternDefinitionId, PatternGeometryResponse, PatternMechanismId,
        PatternOutputLayer, PatternOutputLayerId, PatternOutputSettings, RandomSiteCharacter,
        RegionGeometryResponse, RegionSourceIntent, ResolvedDensityMetric2D, SiteDensityModulation,
        SiteExclusionPolicy, SiteUseFilter, SourceComponent, SourcePlacement, SourceReference,
        SourceReferenceId, SpiralCurve, SpiralShape, StraightGuideDimension,
        StraightGuideRepetition,
    };

    use super::*;
    use sha2::{Digest, Sha256};
    use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save};
    use toniator_patterns::{
        CanonicalRegionLimits, CanonicalRegionProposal, CanonicalRegionSourceGroup,
        CanonicalRegionSourceId, FamilySiteId, PathClosure, build_canonical_regions_cancellable,
    };
    use toniator_patterns::{
        CurveSegment, RegionTreatment, RegionTreatmentRequest, treat_region_requests_cancellable,
    };

    const GUARD: Duration = Duration::from_secs(15);
    const CHANNEL_ID: ChannelId = ChannelId(1);

    /// Converts evaluator-facing across-axis frequencies into current density/aspect authority.
    fn authored_density(canvas: &CanvasSpec, across_x: f64, across_y: f64) -> DensityMetric2D {
        DensityMetric2D::from_resolved(canvas, &ResolvedDensityMetric2D { across_x, across_y })
            .expect("test density is finite and positive")
    }

    /// Binds the current default typed response to each output in one test-only structural definition.
    fn bundle_from_test_definition(definition: PatternDefinition) -> PatternDefinitionBundle {
        let output_settings = definition
            .output_layers
            .iter()
            .map(|output| PatternOutputSettings {
                output_layer_id: output.id(),
                response: match &output.realization {
                    PatternOutputRealization::CircularMarks { .. }
                    | PatternOutputRealization::MarkPrototype { .. } => {
                        PatternGeometryResponse::Marks(MarkGeometryResponse {
                            minimum_fill: 0.0,
                            maximum_fill: 1.0,
                        })
                    }
                    PatternOutputRealization::GuidePaths { .. }
                    | PatternOutputRealization::ParametricPaths { .. }
                    | PatternOutputRealization::CurveMotifPaths { .. }
                    | PatternOutputRealization::ConnectionPaths { .. }
                    | PatternOutputRealization::MazeWalls { .. } => {
                        PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                            minimum_thickness: 0.0,
                            maximum_thickness: 1.0,
                            bias: 0.0,
                        })
                    }
                    PatternOutputRealization::Regions { .. } => {
                        PatternGeometryResponse::Regions(RegionGeometryResponse::default())
                    }
                },
            })
            .collect();
        PatternDefinitionBundle {
            definition,
            output_settings,
        }
    }

    /// Replaces the sole current test fixture base response after preserving its structural output ID.
    fn bundle_with_sole_response(
        definition: PatternDefinition,
        response: PatternGeometryResponse,
    ) -> PatternDefinitionBundle {
        let mut bundle = bundle_from_test_definition(definition);
        bundle.output_settings[0].response = response;
        bundle
    }

    pub(crate) fn request() -> ChannelDiagnosticRequest {
        request_with_bytes(Arc::<[u8]>::from(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/raster-sample.png"
            ))
            .unwrap(),
        ))
    }

    /// Builds one current-authority diagnostic request from caller-supplied immutable source bytes.
    fn request_with_bytes(bytes: Arc<[u8]>) -> ChannelDiagnosticRequest {
        let source_id = SourceReferenceId::new("cancellation-test-source").unwrap();
        let document = Document::with_source(
            DocumentId(1),
            CanvasSpec {
                width: 900.0,
                height: 600.0,
            },
            SourceReference::Assigned(source_id.clone()),
            vec![bundle_with_sole_response(
                PatternDefinition::supported_straight_grid(
                    PatternDefinitionId(1),
                    "straight-grid",
                    PatternMechanismId(1),
                    PatternMechanismId(2),
                    PatternOutputLayerId(1),
                    CoveragePolicy {
                        guard_steps: 2,
                        additional_margin: 4.5,
                    },
                ),
                PatternGeometryResponse::Marks(MarkGeometryResponse {
                    minimum_fill: 0.2,
                    maximum_fill: 0.9,
                }),
            )],
            DocumentPatternSettings {
                definition_id: PatternDefinitionId(1),
                density: DensityMetric2D {
                    density: 5_400.0_f64.sqrt(),
                    aspect: 1.0,
                },
                pattern_rotation_degrees: 17.0,
                shape_rotation_degrees: 0.0,
            },
            vec![ChannelState {
                id: CHANNEL_ID,
                pattern_instance: ChannelPatternInstance {
                    definition_override: None,
                    layout_delta: ChannelPatternLayoutDelta {
                        density: None,
                        rotation_degrees: None,
                        translation_x: 3.25,
                        translation_y: -4.5,
                    },
                    shape_rotation_delta_degrees: None,
                    output_response_deltas: Vec::new(),
                },
                appearance: ChannelAppearance {
                    visible: true,
                    color: ColorValue {
                        red: 0.0,
                        green: srgb_to_linear(183.0 / 255.0),
                        blue: 1.0,
                        alpha: 1.0,
                    },
                    opacity: 0.72,
                },
                source_mapping: ChannelSourceMapping {
                    component: SourceComponent::Luminance,
                    placement: SourcePlacement::StretchToCanvas,
                },
            }],
        )
        .unwrap();
        let session = DocumentSession::new(document).unwrap();
        ChannelDiagnosticRequest::new(
            session.evaluation_snapshot(CHANNEL_ID).unwrap(),
            ResolvedSource::new(source_id, bytes, SourceFormatHint::Png).unwrap(),
        )
    }

    fn modeled_document_session() -> DocumentSession {
        modeled_document_session_for(HalftoneChannelModel::Rgb)
    }

    /// Converts the retained diagnostic fixture into one modeled topology without copying a base.
    fn modeled_document_session_for(model: HalftoneChannelModel) -> DocumentSession {
        let diagnostic = request();
        let mut session = DocumentSession::new(diagnostic.snapshot.document().clone()).unwrap();
        let channel = session.document().channel(CHANNEL_ID).unwrap();
        let template = ChannelTopologyTemplate {
            pattern_instance: channel.pattern_instance.clone(),
        };
        let topology = session
            .document()
            .canonical_channel_topology(model, template)
            .unwrap();
        session
            .apply(&DocumentCommand::ReplaceChannelTopology { model, topology })
            .unwrap();
        session
    }

    /// Builds a current-format modeled document whose typed output realizes one closed shape.
    fn modeled_shape_session() -> DocumentSession {
        let source_id = SourceReferenceId::new("cancellation-test-source").unwrap();
        let base = Document::new_default_document(
            CanvasSpec {
                width: 100.0,
                height: 100.0,
            },
            SourceReference::Assigned(source_id),
        )
        .unwrap();
        let mut definition = PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(1),
            "shape cancellation",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            vec![
                StraightGuideDimension {
                    id: GuideDimensionId(1),
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: GuideDimensionId(2),
                    baseline_angle_degrees: 90.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
            ],
            GeneralizedSiteProduct::Intersections {
                dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
                merge_epsilon: 0.0,
            },
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        let PatternOutputRealization::MarkPrototype { prototype, .. } =
            &mut definition.output_layers[0].realization
        else {
            unreachable!("generalized straight guides own a typed mark output")
        };
        *prototype = MarkPrototype::AuthoredClosedShape {
            structure_id: AuthoredStructureId(1),
        };
        let points = [
            AuthoredPoint2 { x: -1.0, y: -1.0 },
            AuthoredPoint2 { x: 1.0, y: -1.0 },
            AuthoredPoint2 { x: 1.0, y: 1.0 },
            AuthoredPoint2 { x: -1.0, y: 1.0 },
        ];
        let shape = AuthoredStructure::new(
            AuthoredStructureId(1),
            AuthoredStructureKind::ClosedShape,
            (0..points.len())
                .map(|index| AuthoredCurveSegment::Line {
                    start: points[index],
                    end: points[(index + 1) % points.len()],
                })
                .collect(),
        )
        .unwrap();
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                base.id(),
                base.canvas().clone(),
                base.source().clone(),
                vec![bundle_from_test_definition(definition)],
                base.pattern_settings().clone(),
                base.channel_model().unwrap(),
                base.channel_topology().unwrap().clone(),
                vec![shape],
            )
            .unwrap(),
        )
        .unwrap()
    }

    /// Builds a modeled Connected/GuidePaths session for cache and stale-publication witnesses.
    fn modeled_path_session() -> DocumentSession {
        let source_id = SourceReferenceId::new("cancellation-test-source").unwrap();
        let base = Document::new_default_document(
            CanvasSpec {
                width: 64.0,
                height: 48.0,
            },
            SourceReference::Assigned(source_id),
        )
        .unwrap();
        let guide = PatternMechanismId(71);
        let mut definition = PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(70),
            "path cache",
            guide,
            PatternMechanismId(72),
            PatternOutputLayerId(73),
            vec![StraightGuideDimension {
                id: GuideDimensionId(74),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            }],
            GeneralizedSiteProduct::AlongGuides {
                dimensions: vec![GuideDimensionId(74)],
                interval_multiplier: 1.0,
                phase: 0.0,
            },
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        definition.output_layers = vec![PatternOutputLayer::all(
            PatternOutputLayerId(73),
            PatternOutputRealization::GuidePaths {
                guide_mechanism_id: guide,
                style: toniator_domain::PathStrokeStyle::default(),
            },
        )];
        let mut settings = base.pattern_settings().clone();
        settings.definition_id = definition.id;
        settings.density = authored_density(base.canvas(), 2.0, 2.0);
        let bundle = bundle_with_sole_response(
            definition,
            PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.01,
                maximum_thickness: 0.02,
                bias: 0.0,
            }),
        );
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                base.id(),
                base.canvas().clone(),
                base.source().clone(),
                vec![bundle],
                settings,
                base.channel_model().unwrap(),
                base.channel_topology().unwrap().clone(),
                Vec::new(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    /// Builds one valid three-direction intersection document whose channels all realize the
    /// supplied connection program with solid paint and an active coverage guard.
    fn modeled_connection_session(program: ConnectionProgram) -> DocumentSession {
        modeled_connection_session_for_canvas(
            program,
            CanvasSpec {
                width: 64.0,
                height: 48.0,
            },
        )
    }

    /// Builds the production three-direction fixture at a supplied intrinsic canvas size.
    ///
    /// The fixture keeps a shared document-space zero-phase origin and chooses equal physical
    /// directional spacing, so it is suitable for checking the normal evaluator rather than a
    /// hand-authored arrangement.
    fn modeled_connection_session_for_canvas(
        program: ConnectionProgram,
        canvas: CanvasSpec,
    ) -> DocumentSession {
        let source_id = SourceReferenceId::new("cancellation-test-source").unwrap();
        let base =
            Document::new_default_document(canvas.clone(), SourceReference::Assigned(source_id))
                .unwrap();
        let guide_mechanism_id = PatternMechanismId(120);
        let site_mechanism_id = PatternMechanismId(121);
        let output_layer_id = PatternOutputLayerId(122);
        let mut definition = PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(119),
            "triangular connection cache",
            guide_mechanism_id,
            site_mechanism_id,
            output_layer_id,
            vec![
                StraightGuideDimension {
                    id: GuideDimensionId(123),
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: GuideDimensionId(124),
                    baseline_angle_degrees: 60.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: GuideDimensionId(125),
                    baseline_angle_degrees: 120.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
            ],
            GeneralizedSiteProduct::Intersections {
                dimensions: vec![
                    GuideDimensionId(123),
                    GuideDimensionId(124),
                    GuideDimensionId(125),
                ],
                merge_epsilon: 1e-8,
            },
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        definition.output_layers = vec![PatternOutputLayer::all(
            output_layer_id,
            PatternOutputRealization::ConnectionPaths {
                site_mechanism_id,
                program,
                style: PathStrokeStyle::default(),
            },
        )];
        let mut settings = base.pattern_settings().clone();
        settings.definition_id = definition.id;
        settings.density = authored_density(base.canvas(), 5.0, 5.0 * canvas.height / canvas.width);
        let bundle = bundle_with_sole_response(
            definition,
            PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.1,
                maximum_thickness: 0.25,
                bias: 0.0,
            }),
        );
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                base.id(),
                base.canvas().clone(),
                base.source().clone(),
                vec![bundle],
                settings,
                base.channel_model().unwrap(),
                base.channel_topology().unwrap().clone(),
                Vec::new(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    /// Builds a guarded triangular-intersection document whose sole output consumes the
    /// authoritative `FamilySiteSet` as ordinary Voronoi regions.
    ///
    /// The fixture retains the connection fixture's complete family authority, changing only its
    /// typed output contract. It therefore exercises Region scheduling without inventing a second
    /// site source or a frontend-owned topology.
    fn modeled_voronoi_session() -> DocumentSession {
        let connection = modeled_connection_session(random_connection_program(19, 24.0));
        let document = connection.document();
        let mut definition = document.pattern_definition_bundles()[0].definition.clone();
        definition.coverage.guard_steps = 32;
        definition.output_layers = vec![PatternOutputLayer::all(
            PatternOutputLayerId(122),
            PatternOutputRealization::Regions {
                source: RegionSourceIntent::VoronoiSites {
                    site_mechanism_id: PatternMechanismId(121),
                },
            },
        )];
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                document.id(),
                document.canvas().clone(),
                document.source().clone(),
                vec![bundle_with_sole_response(
                    definition,
                    PatternGeometryResponse::Regions(RegionGeometryResponse::default()),
                )],
                document.pattern_settings().clone(),
                document
                    .channel_model()
                    .expect("modeled document has a channel model"),
                document
                    .channel_topology()
                    .expect("modeled document has a channel topology")
                    .clone(),
                document.authored_structures().to_vec(),
            )
            .expect("typed Region fixture validates against triangular intersections"),
        )
        .expect("modeled Region document starts a session")
    }

    /// Builds an ordinary Voronoi session whose Scale response samples every untreated base.
    fn modeled_scaled_voronoi_session() -> DocumentSession {
        modeled_scaled_voronoi_session_with_sampling(
            toniator_domain::RegionSamplingStrategy::ReferencePoint,
        )
    }

    /// Builds a source-sampled Scale Region session with one explicit sampling strategy.
    fn modeled_scaled_voronoi_session_with_sampling(
        sampling: toniator_domain::RegionSamplingStrategy,
    ) -> DocumentSession {
        let session = modeled_voronoi_session();
        let document = session.document();
        let mut bundle = document.pattern_definition_bundles()[0].clone();
        bundle.output_settings[0].response =
            PatternGeometryResponse::Regions(RegionGeometryResponse {
                algorithm: toniator_domain::RegionResizeAlgorithm::Scale,
                sampling,
                minimum_fill: 0.75,
                maximum_fill: 1.25,
            });
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                document.id(),
                document.canvas().clone(),
                document.source().clone(),
                vec![bundle],
                document.pattern_settings().clone(),
                document
                    .channel_model()
                    .expect("modeled Region has a channel model"),
                document
                    .channel_topology()
                    .expect("modeled Region has topology")
                    .clone(),
                document.authored_structures().to_vec(),
            )
            .expect("scaled Region document validates"),
        )
        .expect("scaled Region session starts")
    }

    /// Proves straight-guide Voronoi support relies on its evaluator-owned guard while random
    /// and finite parametric producers retain closure support and maximum-fill-two coverage.
    #[test]
    fn voronoi_region_support_avoids_straight_guard_double_count_and_keeps_parametric_envelope() {
        let straight = modeled_scaled_voronoi_session();
        let straight_document = straight.document();
        let straight_definition = &straight_document.pattern_definition_bundles()[0].definition;
        let straight_effective = straight_document
            .effective_channel_pattern(CHANNEL_ID)
            .expect("straight Voronoi channel resolves");
        let straight_plan = toniator_patterns::resolve_document_pattern_pipeline(
            straight_document,
            straight_definition,
        )
        .expect("straight Voronoi plan resolves");
        let straight_nominal = maximum_nominal_cell_diameter(
            &straight_plan.family,
            straight_document.canvas(),
            &straight_effective.resolved_density,
        )
        .expect("straight Voronoi nominal extent is finite");
        let straight_support = required_support_radius_for_output(
            straight_document.canvas(),
            &straight_effective,
            straight_definition,
            &straight_plan.family,
            straight_plan
                .ordered_outputs
                .first()
                .expect("straight Voronoi has one Region output"),
        )
        .expect("straight Voronoi support resolves");
        assert_eq!(
            straight_support,
            0.25 * straight_nominal,
            "straight/grid family evaluation already owns its configured guard candidates"
        );

        let random_definition = PatternDefinition::random_sites(
            PatternDefinitionId(920),
            "random Voronoi closure",
            PatternMechanismId(921),
            PatternMechanismId(922),
            PatternMechanismId(923),
            PatternMechanismId(924),
            PatternOutputLayerId(122),
            RandomSiteCharacter::RawUniform,
            17,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            8_192,
            8_192,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
        );
        let random_mark_plan = toniator_patterns::resolve_pattern_pipeline(&random_definition)
            .expect("random mark plan resolves before Region closure planning");
        let mut random_region_definition = random_definition.clone();
        random_region_definition.output_layers = vec![PatternOutputLayer::all(
            PatternOutputLayerId(122),
            PatternOutputRealization::Regions {
                source: RegionSourceIntent::VoronoiSites {
                    site_mechanism_id: PatternMechanismId(924),
                },
            },
        )];
        let random_region_plan =
            toniator_patterns::resolve_pattern_pipeline(&random_region_definition)
                .expect("random Voronoi plan resolves");
        assert_eq!(
            random_mark_plan.family, random_region_plan.family,
            "changing only the output leaves the random family identity untouched"
        );
        let random_nominal = maximum_nominal_cell_diameter(
            &random_region_plan.family,
            straight_document.canvas(),
            &straight_effective.resolved_density,
        )
        .expect("random Voronoi nominal extent is finite");
        let random_region_support = required_support_radius_for_output(
            straight_document.canvas(),
            &straight_effective,
            &random_region_definition,
            &random_region_plan.family,
            random_region_plan
                .ordered_outputs
                .first()
                .expect("random Voronoi has one Region output"),
        )
        .expect("random Voronoi support resolves");
        assert_eq!(
            random_region_support,
            2.25 * random_nominal,
            "random Voronoi adds producer closure plus its positive fill-one-quarter extension"
        );
        let random_mark_support = required_support_radius_for_output(
            straight_document.canvas(),
            &straight_effective,
            &random_definition,
            &random_mark_plan.family,
            random_mark_plan
                .ordered_outputs
                .first()
                .expect("random marks retain one output"),
        )
        .expect("random mark support resolves");
        assert_eq!(
            random_mark_support, random_nominal,
            "random marks retain their established maximum-fill family request"
        );

        let parametric_response = RegionGeometryResponse {
            algorithm: toniator_domain::RegionResizeAlgorithm::Scale,
            sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
            minimum_fill: 0.0,
            maximum_fill: 2.0,
        };
        let parametric_document = stage20q_parametric_voronoi_document(
            SourceReferenceId::new("parametric-support").expect("source ID validates"),
            parametric_response,
        );
        let parametric_definition = &parametric_document.pattern_definition_bundles()[0].definition;
        let parametric_effective = parametric_document
            .effective_channel_pattern(CHANNEL_ID)
            .expect("parametric Voronoi channel resolves");
        let parametric_plan = toniator_patterns::resolve_document_pattern_pipeline(
            &parametric_document,
            parametric_definition,
        )
        .expect("parametric Voronoi plan resolves");
        let parametric_nominal = maximum_nominal_cell_diameter(
            &parametric_plan.family,
            parametric_document.canvas(),
            &parametric_effective.resolved_density,
        )
        .expect("parametric Voronoi nominal extent is finite");
        let parametric_support = required_support_radius_for_output(
            parametric_document.canvas(),
            &parametric_effective,
            parametric_definition,
            &parametric_plan.family,
            parametric_plan
                .ordered_outputs
                .first()
                .expect("parametric Voronoi has one Region output"),
        )
        .expect("parametric Voronoi support resolves");
        assert_eq!(
            parametric_support,
            f64::from(parametric_definition.coverage.guard_steps) * parametric_nominal
                + parametric_nominal,
            "finite parametric sites retain guard coverage and fill two adds one positive radius"
        );
    }

    /// Proves raw uniform Voronoi producers with no exclusion retain deterministic guarded
    /// closure sites and bounded relevant regions without turning their process into Poisson spacing.
    #[test]
    fn raw_uniform_voronoi_uses_producer_closure_without_exclusion_semantics() {
        let base = modeled_scaled_voronoi_session();
        let base_document = base.document();
        let definition_id = base_document.pattern_settings().definition_id;
        let output_layer_id = PatternOutputLayerId(122);
        let site_mechanism_id = PatternMechanismId(934);
        let mut definition = PatternDefinition::random_sites(
            definition_id,
            "raw uniform Voronoi closure",
            PatternMechanismId(931),
            PatternMechanismId(932),
            PatternMechanismId(933),
            site_mechanism_id,
            output_layer_id,
            RandomSiteCharacter::RawUniform,
            17,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            8_192,
            8_192,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
        );
        definition.output_layers = vec![PatternOutputLayer::all(
            output_layer_id,
            PatternOutputRealization::Regions {
                source: RegionSourceIntent::VoronoiSites { site_mechanism_id },
            },
        )];
        let document = Document::with_source_topology_and_authored_structures(
            base_document.id(),
            base_document.canvas().clone(),
            base_document.source().clone(),
            vec![bundle_with_sole_response(
                definition,
                PatternGeometryResponse::Regions(RegionGeometryResponse::default()),
            )],
            base_document.pattern_settings().clone(),
            base_document
                .channel_model()
                .expect("modeled Voronoi channel model remains present"),
            base_document
                .channel_topology()
                .expect("modeled Voronoi topology remains present")
                .clone(),
            Vec::new(),
        )
        .expect("raw uniform Voronoi document validates");
        let session = DocumentSession::new(document).expect("raw uniform Voronoi session starts");
        let bytes = valid_document_bytes();
        let first = evaluate_cached_document(
            document_request(&session, Arc::clone(&bytes)),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        )
        .expect("guarded raw uniform Voronoi regions remain bounded");
        let diagnostics = first.transaction.families[0]
            .1
            .random_diagnostics()
            .expect("random family publishes deterministic coverage diagnostics");
        assert!(
            diagnostics.guard_sites > 0,
            "producer closure retains guard sites"
        );
        assert_eq!(
            diagnostics.canvas_sites + diagnostics.guard_sites,
            diagnostics.achieved_sites,
            "every accepted raw site is classified as canvas or producer guard"
        );
        assert_eq!(
            diagnostics.rejected_by_exclusion, 0,
            "RawUniform with SiteExclusionPolicy::None retains no Poisson-style exclusion"
        );
        let second = evaluate_cached_document(
            document_request(&session, bytes),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        )
        .expect("raw uniform Voronoi replay remains bounded");
        assert_eq!(
            diagnostics,
            second.transaction.families[0]
                .1
                .random_diagnostics()
                .expect("replayed random family retains coverage diagnostics"),
            "same request retains the exact expected producer guard-site accounting"
        );
        assert_eq!(
            first.result.channels()[0].family_identity(),
            second.result.channels()[0].family_identity(),
            "same raw seed and guarded producer request keep family identity deterministic"
        );
        assert_eq!(
            first.result.channels()[0].realization_identity(),
            second.result.channels()[0].realization_identity(),
            "same bounded positive Voronoi regions keep realization identity deterministic"
        );
    }

    /// Builds an ordinary Voronoi session whose SourceColorAlpha channel consumes sampled region paint.
    fn modeled_sampled_voronoi_session() -> DocumentSession {
        let mut session = modeled_voronoi_session();
        let template = ChannelTopologyTemplate {
            pattern_instance: session
                .document()
                .channel_topology()
                .expect("modeled Region has topology")
                .channels()[0]
                .pattern_instance
                .clone(),
        };
        session
            .apply(&DocumentCommand::ReplaceChannelTopology {
                model: HalftoneChannelModel::SourceColorAlpha,
                topology: toniator_domain::ChannelTopology::canonical(
                    HalftoneChannelModel::SourceColorAlpha,
                    template,
                )
                .expect("source-color topology validates"),
            })
            .expect("sampled Region model validates");
        session
    }

    /// Builds a phase-aligned three-guide document whose sole region output consumes guide faces.
    fn modeled_guide_face_session() -> DocumentSession {
        modeled_guide_face_session_for_canvas(
            vec![
                GuideDimensionId(123),
                GuideDimensionId(124),
                GuideDimensionId(125),
            ],
            CanvasSpec {
                width: 64.0,
                height: 48.0,
            },
        )
    }

    /// Builds a selected two-to-three guide-face document from the existing typed family fixture.
    fn modeled_guide_face_session_with_dimensions(
        dimensions: Vec<GuideDimensionId>,
    ) -> DocumentSession {
        modeled_guide_face_session_for_canvas(
            dimensions,
            CanvasSpec {
                width: 64.0,
                height: 48.0,
            },
        )
    }

    /// Builds a selected two-to-three guide-face document at the requested native output size.
    fn modeled_guide_face_session_for_canvas(
        dimensions: Vec<GuideDimensionId>,
        canvas: CanvasSpec,
    ) -> DocumentSession {
        let connection =
            modeled_connection_session_for_canvas(random_connection_program(19, 24.0), canvas);
        let document = connection.document();
        let mut definition = document.pattern_definition_bundles()[0].definition.clone();
        definition.coverage.guard_steps = 2;
        definition.output_layers = vec![PatternOutputLayer::all(
            PatternOutputLayerId(122),
            PatternOutputRealization::Regions {
                source: RegionSourceIntent::GuideFaces {
                    guide_mechanism_id: PatternMechanismId(120),
                    dimensions,
                },
            },
        )];
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                document.id(),
                document.canvas().clone(),
                document.source().clone(),
                vec![bundle_with_sole_response(
                    definition,
                    PatternGeometryResponse::Regions(RegionGeometryResponse::default()),
                )],
                document.pattern_settings().clone(),
                document
                    .channel_model()
                    .expect("modeled document has a channel model"),
                document
                    .channel_topology()
                    .expect("modeled document has a channel topology")
                    .clone(),
                document.authored_structures().to_vec(),
            )
            .expect("typed guide-face fixture validates against selected guides"),
        )
        .expect("guide-face document starts a session")
    }

    /// Verifies that the production straight-guide evaluator emits only equilateral three-edge faces for the phase-zero triangular family.
    fn assert_production_equilateral_triangles(scene: &RenderScene) {
        const TOLERANCE: f64 = 1.0e-8;
        let mut count = 0usize;
        for layer in scene.layers() {
            for output in layer.outputs() {
                let GeometryOutput::CanonicalRegions(regions) = output.geometry() else {
                    continue;
                };
                for region in regions.regions() {
                    count += 1;
                    assert_eq!(
                        region.ring.segments().len(),
                        3,
                        "face has three edges: {:?}",
                        region.ring.segments()
                    );
                    assert!(
                        region
                            .ring
                            .segments()
                            .iter()
                            .all(|segment| { matches!(segment, CurveSegment::Line(_)) })
                    );
                    let lengths = region
                        .ring
                        .segments()
                        .iter()
                        .map(|segment| {
                            let start = segment.start();
                            let end = segment.end();
                            ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt()
                        })
                        .collect::<Vec<_>>();
                    assert!(
                        lengths
                            .iter()
                            .all(|length| (*length - lengths[0]).abs() <= TOLERANCE),
                        "triangle sides are equilateral within {TOLERANCE}",
                    );
                    let toniator_patterns::CanonicalRegionSourceId::GuideBoundary(sources) =
                        &region.id.source_id
                    else {
                        panic!("Guide Faces retain guide-boundary provenance");
                    };
                    let dimensions = sources
                        .iter()
                        .filter_map(|source| match source.path.source {
                            toniator_patterns::StructuralPathSourceId::GuideDimension(id) => {
                                Some(id)
                            }
                            _ => None,
                        })
                        .collect::<std::collections::BTreeSet<_>>();
                    assert_eq!(
                        dimensions.len(),
                        3,
                        "each triangular face uses all directions"
                    );
                }
            }
        }
        assert!(count > 0, "production triangular family emits faces");
    }

    /// Detects one closed cubic segment that is tangent-continuous with both adjacent segments.
    fn stage20q_has_tangent_continuous_cubic_join(path: &CurvePath) -> bool {
        let segments = path.segments();
        if segments.len() < 3 {
            return false;
        }
        segments.iter().enumerate().any(|(index, segment)| {
            let CurveSegment::CubicBezier(_) = segment else {
                return false;
            };
            let previous = segments[(index + segments.len() - 1) % segments.len()];
            let next = segments[(index + 1) % segments.len()];
            let joins_previous = previous.end() == segment.start();
            let joins_next = segment.end() == next.start();
            let tangent_before = previous.unit_tangent_at(1.0);
            let tangent_start = segment.unit_tangent_at(0.0);
            let tangent_end = segment.unit_tangent_at(1.0);
            let tangent_after = next.unit_tangent_at(0.0);
            joins_previous
                && joins_next
                && matches!(
                    (tangent_before, tangent_start, tangent_end, tangent_after),
                    (Ok(before), Ok(start), Ok(end), Ok(after))
                        if before.x * start.x + before.y * start.y > 0.999_999
                            && end.x * after.x + end.y * after.y > 0.999_999
                )
        })
    }

    /// Verifies one untreated triangular Guide Face set retains equilateral positive line faces.
    fn assert_production_equilateral_untreated_faces(regions: &CanonicalRegionSet) {
        const TOLERANCE: f64 = 1.0e-8;
        assert!(!regions.regions().is_empty(), "untreated Guide Faces exist");
        for region in regions.regions() {
            assert_eq!(region.ring.segments().len(), 3);
            assert!(
                region
                    .ring
                    .segments()
                    .iter()
                    .all(|segment| matches!(segment, CurveSegment::Line(_)))
            );
            assert!(
                region.area > 0.0,
                "canonical untreated face has positive winding"
            );
            let lengths = region
                .ring
                .segments()
                .iter()
                .map(|segment| {
                    let start = segment.start();
                    let end = segment.end();
                    ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt()
                })
                .collect::<Vec<_>>();
            assert!(
                lengths
                    .iter()
                    .all(|length| (*length - lengths[0]).abs() <= TOLERANCE),
                "untreated triangular face remains equilateral"
            );
        }
    }

    /// Proves the phase-aligned three-guide authoritative pipeline reaches canonical regions and SVG.
    #[test]
    fn guide_face_document_reaches_canonical_regions_and_svg() {
        let session = modeled_guide_face_session();
        let scheduler = EvaluationScheduler::new().expect("scheduler starts");
        let ticket = scheduler
            .submit(document_request_for_format(
                &session,
                "cancellation-test-source",
                valid_vector_document_bytes(),
                SourceFormatHint::Svg,
            ))
            .expect("guide-face request submits");
        let completion = wait_for_document_completion(&scheduler);
        assert_eq!(completion.ticket(), ticket);
        let result = completion
            .result()
            .expect("guide-face realization completes");
        assert!(result.scene().layers().iter().all(|layer| matches!(layer.outputs().first().map(toniator_render::RenderOutputLayer::geometry), Some(GeometryOutput::CanonicalRegions(regions)) if !regions.regions().is_empty())));
        assert_production_equilateral_triangles(result.scene());
        assert!(write_svg(result.scene()).contains("<path"));
        assert!(
            scheduler
                .accept_completion(&completion, &session)
                .expect("completion validates")
        );
        scheduler.shutdown().expect("scheduler shuts down");
    }

    /// Generates the intrinsic three-guide SVG and PNG evidence from the production evaluator.
    ///
    /// This test is ignored during ordinary verification because it writes derived validation
    /// artifacts and invokes Inkscape. It never invents guide paths: the persisted document
    /// definition, shared centered grid transform, family evaluation, region builder, and
    /// renderer all participate before serialization.
    #[test]
    #[ignore = "validation artifact generator"]
    fn generate_intrinsic_production_three_guide_face_artifact() {
        let session = modeled_guide_face_session_for_canvas(
            vec![
                GuideDimensionId(123),
                GuideDimensionId(124),
                GuideDimensionId(125),
            ],
            CanvasSpec {
                width: 900.0,
                height: 620.0,
            },
        );
        let scheduler = EvaluationScheduler::new().expect("scheduler starts");
        let completion = {
            scheduler
                .submit(document_request_for_format(
                    &session,
                    "cancellation-test-source",
                    valid_vector_document_bytes(),
                    SourceFormatHint::Svg,
                ))
                .expect("production guide-face request submits");
            wait_for_document_completion(&scheduler)
        };
        let result = completion
            .result()
            .expect("production guide-face realization completes");
        assert_production_equilateral_triangles(result.scene());
        let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/validation/stage20p");
        std::fs::create_dir_all(&output).expect("validation directory exists");
        let name = "three-guide-vector-900x620";
        let region_output = result
            .scene()
            .layers()
            .first()
            .expect("production scene has a layer")
            .outputs()
            .first()
            .expect("production layer has one output");
        let GeometryOutput::CanonicalRegions(regions) = region_output.geometry() else {
            panic!("production guide faces remain canonical regions");
        };
        let mut identity_record = format!("fingerprint={}\n", regions.fingerprint());
        for region in regions.regions() {
            identity_record.push_str(&format!("{:?}\n", region.id));
        }
        std::fs::write(output.join(format!("{name}-regions.txt")), identity_record)
            .expect("region identity record writes");
        let raster = rasterize(result.scene(), RasterBackground::Transparent)
            .expect("production scene rasterizes");
        std::fs::write(
            output.join(format!("{name}.png")),
            encode_png(&raster).expect("production PNG encodes"),
        )
        .expect("production PNG writes");
        let svg_path = output.join(format!("{name}.svg"));
        std::fs::write(&svg_path, write_svg(result.scene())).expect("production SVG writes");
        let status = std::process::Command::new("inkscape")
            .arg(&svg_path)
            .arg("--export-type=png")
            .arg(format!(
                "--export-filename={}",
                output.join(format!("{name}-svg-rasterized.png")).display()
            ))
            .status()
            .expect("Inkscape is available for production SVG evidence");
        assert!(status.success(), "production SVG rasterization succeeds");
        scheduler.shutdown().expect("scheduler shuts down");
    }

    /// Proves a selected two-guide rectangular arrangement reaches the canonical renderer boundary.
    #[test]
    fn two_guide_face_document_reaches_canonical_regions() {
        let session = modeled_guide_face_session_with_dimensions(vec![
            GuideDimensionId(123),
            GuideDimensionId(124),
        ]);
        let scheduler = EvaluationScheduler::new().expect("scheduler starts");
        let ticket = scheduler
            .submit(document_request(&session, valid_document_bytes()))
            .expect("guide-face request submits");
        let completion = wait_for_document_completion(&scheduler);
        assert_eq!(completion.ticket(), ticket);
        let result = completion
            .result()
            .expect("two-guide realization completes");
        assert!(result.scene().layers().iter().all(|layer| matches!(layer.outputs().first().map(toniator_render::RenderOutputLayer::geometry), Some(GeometryOutput::CanonicalRegions(regions)) if !regions.regions().is_empty())));
        scheduler.shutdown().expect("scheduler shuts down");
    }

    /// Builds a 24-across phase-aligned triangular family with a typed conventional wall-maze output.
    fn modeled_maze_session(seed: u32) -> DocumentSession {
        let connection = modeled_connection_session(random_connection_program(seed, 24.0));
        let document = connection.document();
        let mut definition = document.pattern_definition_bundles()[0].definition.clone();
        definition.output_layers = vec![PatternOutputLayer::all(
            PatternOutputLayerId(122),
            PatternOutputRealization::MazeWalls {
                site_mechanism_id: PatternMechanismId(121),
                program: toniator_domain::MazeProgram {
                    algorithm: toniator_domain::GridMazeAlgorithm::RecursiveBacktracker,
                    seed,
                },
                style: PathStrokeStyle::default(),
            },
        )];
        let mut settings = document.pattern_settings().clone();
        settings.density = authored_density(document.canvas(), 24.0, 24.0 * 48.0 / 64.0);
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                document.id(),
                document.canvas().clone(),
                document.source().clone(),
                vec![bundle_from_test_definition(definition)],
                settings,
                document
                    .channel_model()
                    .expect("modeled document has a channel model"),
                document
                    .channel_topology()
                    .expect("modeled document has a channel topology")
                    .clone(),
                document.authored_structures().to_vec(),
            )
            .expect("typed maze output validates against triangular intersections"),
        )
        .expect("modeled maze document starts a session")
    }

    /// Builds retained maze walls plus marks consuming exactly the retained wall endpoints.
    fn modeled_stage20r_maze_endpoint_session(seed: u32) -> DocumentSession {
        let base = modeled_maze_session(seed);
        let document = base.document();
        let mut definition = document.pattern_definition_bundles()[0].definition.clone();
        let maze = definition.output_layers[0].clone();
        let maze_id = maze.id;
        let marks_id = PatternOutputLayerId(maze_id.0 + 1);
        let marks = PatternOutputLayer::new(
            marks_id,
            SiteUseFilter::SitesUsedBy {
                output_layer_id: maze_id,
            },
            PatternOutputRealization::MarkPrototype {
                site_mechanism_id: PatternMechanismId(121),
                prototype: MarkPrototype::Circle,
                orientation: MarkOrientation::Fixed,
            },
        );
        definition.output_layers = vec![marks, maze];
        let output_settings = definition
            .output_layers
            .iter()
            .map(|output| PatternOutputSettings {
                output_layer_id: output.id,
                response: match output.realization {
                    PatternOutputRealization::MazeWalls { .. } => {
                        PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                            minimum_thickness: 0.08,
                            maximum_thickness: 0.18,
                            bias: 0.0,
                        })
                    }
                    PatternOutputRealization::MarkPrototype { .. } => {
                        PatternGeometryResponse::Marks(MarkGeometryResponse {
                            minimum_fill: 0.18,
                            maximum_fill: 0.36,
                        })
                    }
                    _ => unreachable!("maze endpoint fixture contains only walls and marks"),
                },
            })
            .collect();
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                document.id(),
                document.canvas().clone(),
                document.source().clone(),
                vec![PatternDefinitionBundle {
                    definition,
                    output_settings,
                }],
                document.pattern_settings().clone(),
                document.channel_model().expect("modeled document model"),
                document
                    .channel_topology()
                    .expect("modeled document topology")
                    .clone(),
                document.authored_structures().to_vec(),
            )
            .expect("maze endpoint composite validates"),
        )
        .expect("maze endpoint session validates")
    }

    /// Returns a valid random-link intent whose seed and distance may exercise connection-only
    /// realization cache invalidation without changing the family definition.
    fn random_connection_program(seed: u32, maximum_distance: f64) -> ConnectionProgram {
        ConnectionProgram::RandomLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 6,
                maximum_distance,
            },
            minimum_degree: 0,
            seed,
        }
    }

    /// Replaces the sole typed connection output through the document's validated shared edit
    /// authority, preserving the fixture's output-layer identity.
    fn replace_connection_program(history: &mut DocumentHistory, program: ConnectionProgram) {
        let base_definition = history.document().pattern_definition_bundles()[0]
            .definition
            .clone();
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: base_definition.id,
                base_definition,
                edit: PatternDefinitionEdit::SetConnectionProgram {
                    output_layer_id: PatternOutputLayerId(122),
                    program,
                },
            })
            .unwrap();
    }

    /// Replaces only the typed maze seed through the validated document history authority, so
    /// family geometry remains eligible for reuse while the maze realization must be rebuilt.
    fn replace_maze_seed(history: &mut DocumentHistory, seed: u32) {
        let base_definition = history.document().pattern_definition_bundles()[0]
            .definition
            .clone();
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: base_definition.id,
                base_definition,
                edit: PatternDefinitionEdit::SetMazeSeed {
                    output_layer_id: PatternOutputLayerId(122),
                    seed,
                },
            })
            .expect("typed maze seed edit validates");
    }

    /// Extracts the rendered canonical connection strokes without asking the renderer to
    /// synthesize topology or reinterpret their centerlines.
    fn connection_strokes(
        result: &EvaluationResult,
    ) -> Vec<Vec<toniator_patterns::CanonicalStroke>> {
        result
            .scene()
            .layers()
            .iter()
            .map(|layer| match layer.geometry() {
                GeometryOutput::CanonicalStrokes(strokes) => strokes.clone(),
                other => panic!("connection fixture produced unexpected geometry: {other:?}"),
            })
            .collect()
    }

    /// Builds a current generic-guide session whose persisted repetition exercises Stage 20J and Stage 20I together.
    fn modeled_normal_offset_session() -> DocumentSession {
        let source_id = SourceReferenceId::new("cancellation-test-source").unwrap();
        let base = Document::new_default_document(
            CanvasSpec {
                width: 64.0,
                height: 48.0,
            },
            SourceReference::Assigned(source_id),
        )
        .unwrap();
        let guide_mechanism_id = PatternMechanismId(92);
        let dimension_id = GuideDimensionId(95);
        let mut definition = PatternDefinition::generalized_guides(
            PatternDefinitionId(90),
            "normal-offset cache",
            guide_mechanism_id,
            PatternMechanismId(93),
            PatternOutputLayerId(94),
            vec![GuideDimension {
                id: dimension_id,
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(96),
                },
                repetition: GuideRepetition::NormalOffset {
                    spacing: 12.0,
                    cleanup: OffsetCleanup::DissolveCrossings,
                },
            }],
            GeneralizedSiteProduct::AlongGuides {
                dimensions: vec![dimension_id],
                interval_multiplier: 1.0,
                phase: 0.0,
            },
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        definition.output_layers = vec![PatternOutputLayer::all(
            PatternOutputLayerId(94),
            PatternOutputRealization::GuidePaths {
                guide_mechanism_id,
                style: toniator_domain::PathStrokeStyle::default(),
            },
        )];
        let guide = AuthoredStructure::new(
            AuthoredStructureId(96),
            AuthoredStructureKind::OpenPath,
            vec![AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 8.0, y: 24.0 },
                end: AuthoredPoint2 { x: 56.0, y: 24.0 },
            }],
        )
        .unwrap();
        let mut settings = base.pattern_settings().clone();
        settings.definition_id = definition.id;
        settings.density = authored_density(base.canvas(), 2.0, 2.0);
        let bundle = bundle_with_sole_response(
            definition,
            PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.01,
                maximum_thickness: 0.02,
                bias: 0.0,
            }),
        );
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                base.id(),
                base.canvas().clone(),
                base.source().clone(),
                vec![bundle],
                settings,
                base.channel_model().unwrap(),
                base.channel_topology().unwrap().clone(),
                vec![guide],
            )
            .unwrap(),
        )
        .unwrap()
    }

    /// Builds a generic authored-cubic Guide Faces document whose two repeated source dimensions form closed regions.
    fn modeled_authored_cubic_guide_face_session() -> DocumentSession {
        modeled_authored_cubic_guide_face_session_for_canvas(CanvasSpec {
            width: 64.0,
            height: 48.0,
        })
    }

    /// Builds authored cubic Guide Faces at one native evidence canvas without frontend geometry.
    fn modeled_authored_cubic_guide_face_session_for_canvas(canvas: CanvasSpec) -> DocumentSession {
        let source_id = SourceReferenceId::new("cancellation-test-source").expect("source ID");
        let base =
            Document::new_default_document(canvas.clone(), SourceReference::Assigned(source_id))
                .expect("base document");
        let guide_id = PatternMechanismId(196);
        let horizontal = GuideDimensionId(197);
        let vertical = GuideDimensionId(198);
        let mut definition = PatternDefinition::generalized_guides(
            PatternDefinitionId(195),
            "authored cubic guide faces",
            guide_id,
            PatternMechanismId(199),
            PatternOutputLayerId(200),
            vec![
                GuideDimension {
                    id: horizontal,
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    prototype: GuidePrototype::AuthoredOpenPath {
                        structure_id: AuthoredStructureId(201),
                    },
                    repetition: GuideRepetition::NormalOffset {
                        spacing: canvas.width * 0.3125,
                        cleanup: OffsetCleanup::DissolveCrossings,
                    },
                },
                GuideDimension {
                    id: vertical,
                    baseline_angle_degrees: 90.0,
                    phase: 0.0,
                    prototype: GuidePrototype::AuthoredOpenPath {
                        structure_id: AuthoredStructureId(202),
                    },
                    repetition: GuideRepetition::NormalOffset {
                        spacing: canvas.height * 0.5,
                        cleanup: OffsetCleanup::DissolveCrossings,
                    },
                },
            ],
            GeneralizedSiteProduct::Intersections {
                dimensions: vec![horizontal, vertical],
                merge_epsilon: 0.0,
            },
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        definition.output_layers = vec![PatternOutputLayer::all(
            PatternOutputLayerId(200),
            PatternOutputRealization::Regions {
                source: RegionSourceIntent::GuideFaces {
                    guide_mechanism_id: guide_id,
                    dimensions: vec![horizontal, vertical],
                },
            },
        )];
        let cubic = AuthoredStructure::new(
            AuthoredStructureId(201),
            AuthoredStructureKind::OpenPath,
            vec![AuthoredCurveSegment::CubicBezier {
                start: AuthoredPoint2 {
                    x: canvas.width * 0.125,
                    y: canvas.height * 0.5,
                },
                control_1: AuthoredPoint2 {
                    x: canvas.width * 0.25,
                    y: canvas.height / 6.0,
                },
                control_2: AuthoredPoint2 {
                    x: canvas.width * 0.75,
                    y: canvas.height / 6.0,
                },
                end: AuthoredPoint2 {
                    x: canvas.width * 0.875,
                    y: canvas.height * 0.5,
                },
            }],
        )
        .expect("cubic authored guide");
        let vertical_line = AuthoredStructure::new(
            AuthoredStructureId(202),
            AuthoredStructureKind::OpenPath,
            vec![AuthoredCurveSegment::Line {
                start: AuthoredPoint2 {
                    x: canvas.width * 0.37,
                    y: canvas.height * 0.125,
                },
                end: AuthoredPoint2 {
                    x: canvas.width * 0.37,
                    y: canvas.height * 0.875,
                },
            }],
        )
        .expect("vertical authored guide");
        let mut settings = base.pattern_settings().clone();
        settings.definition_id = definition.id;
        settings.density = authored_density(base.canvas(), 2.0, 2.0);
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                base.id(),
                base.canvas().clone(),
                base.source().clone(),
                vec![bundle_with_sole_response(
                    definition,
                    PatternGeometryResponse::Regions(RegionGeometryResponse::default()),
                )],
                settings,
                base.channel_model().expect("model"),
                base.channel_topology().expect("topology").clone(),
                vec![cubic, vertical_line],
            )
            .expect("authored cubic Guide Faces document"),
        )
        .expect("authored cubic session")
    }

    /// Proves a genuinely authored non-straight cubic guide reaches canonical Guide Faces through the engine.
    #[test]
    fn authored_cubic_guide_face_document_reaches_canonical_regions() {
        let session = modeled_authored_cubic_guide_face_session();
        let scheduler = EvaluationScheduler::new().expect("scheduler starts");
        scheduler
            .submit(document_request(&session, valid_document_bytes()))
            .expect("cubic request submits");
        let completion = wait_for_document_completion(&scheduler);
        let result = completion
            .result()
            .unwrap_or_else(|| panic!("cubic realization: {:?}", completion.error()));
        let contains_cubic_region = result.scene().layers().iter().any(|layer| {
            matches!(
                layer.outputs().first().map(toniator_render::RenderOutputLayer::geometry),
                Some(GeometryOutput::CanonicalRegions(regions))
                    if regions.regions().iter().any(|region| region.ring.segments().iter().any(
                        |segment| matches!(segment, CurveSegment::CubicBezier(_))
                    ))
            )
        });
        assert!(
            contains_cubic_region,
            "authored cubic guide remains a canonical face boundary"
        );
        scheduler.shutdown().expect("scheduler shuts down");
    }

    fn document_request(session: &DocumentSession, bytes: Arc<[u8]>) -> EvaluationRequest {
        document_request_for_source(session, "cancellation-test-source", bytes)
    }

    /// Proves a single-channel diagnostic result shares pending cache payloads without changing accessor equality.
    ///
    /// # Panics
    ///
    /// Panics when diagnostic result publication deep-clones cached scene or
    /// raster values instead of retaining their immutable Arc identity.
    #[test]
    fn channel_diagnostic_result_shares_scene_and_raster_with_cache_transaction() {
        let evaluated = evaluate_channel_diagnostic_cached_with_cancellation(
            request(),
            &NeverCancelled,
            DerivedCacheSnapshot::empty(),
            EvaluationLimits::default(),
        )
        .expect("single-channel diagnostic evaluation completes");
        let cached_scene = &evaluated
            .transaction
            .scene
            .as_ref()
            .expect("scene miss schedules cache installation")
            .1;
        let cached_raster = &evaluated
            .transaction
            .raster
            .as_ref()
            .expect("raster miss schedules cache installation")
            .1;
        assert!(Arc::ptr_eq(&evaluated.result.scene, cached_scene));
        assert!(Arc::ptr_eq(&evaluated.result.raster, cached_raster));
        assert_eq!(evaluated.result.scene(), cached_scene.as_ref());
        assert_eq!(evaluated.result.raster(), cached_raster.as_ref());
    }

    /// Proves an evaluation result borrows the same immutable scene and raster Arcs retained for cache commit.
    ///
    /// # Panics
    ///
    /// Panics when result publication deep-clones cached render payloads rather
    /// than preserving their pointer identity and public accessor values.
    #[test]
    fn evaluation_result_shares_scene_and_raster_with_cache_transaction() {
        let session = modeled_document_session();
        let evaluated = evaluate_cached_document(
            document_request(&session, valid_document_bytes()),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        )
        .expect("document evaluation completes");
        let cached_scene = &evaluated
            .transaction
            .scene
            .as_ref()
            .expect("scene miss schedules cache installation")
            .1;
        let cached_raster = &evaluated
            .transaction
            .raster
            .as_ref()
            .expect("raster miss schedules cache installation")
            .1;
        assert!(Arc::ptr_eq(&evaluated.result.scene, cached_scene));
        assert!(Arc::ptr_eq(&evaluated.result.raster, cached_raster));
        assert_eq!(evaluated.result.scene(), cached_scene.as_ref());
        assert_eq!(evaluated.result.raster(), cached_raster.as_ref());
    }

    /// Proves cache replay and scene publication share one immutable canonical output payload.
    ///
    /// This regression verifies realization-cache reuse, exact renderer order,
    /// and geometry pointer identity without a global allocator statistic. The
    /// retained cache record contains no typed provenance or second canonical
    /// allocation. It performs no I/O beyond its immutable source fixture.
    #[test]
    fn cached_output_realizations_share_scene_geometry_and_preserve_output_parity() {
        let session = modeled_document_session();
        let request = document_request(&session, valid_document_bytes());
        let mut cache = DocumentDerivedCache::default();
        let first = evaluate_cached_document(
            request.clone(),
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("initial document evaluation completes");
        let cached_realization = Arc::clone(
            &first
                .transaction
                .realizations
                .first()
                .expect("initial evaluation retains one output realization")
                .1,
        );
        let first_outputs = first.result.scene().layers()[0].outputs().to_vec();
        assert!(Arc::ptr_eq(
            cached_realization.realization.geometry(),
            &first.result.scene().layers()[0].outputs()[0].geometry
        ));
        cache.commit(first.transaction.clone());
        assert!(Arc::ptr_eq(&cached_realization, &cache.realizations[0].1));

        let replay = evaluate_cached_document(
            request,
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("cached document evaluation completes");
        assert_eq!(replay.diagnostics.channels[0].family, CacheDisposition::Hit);
        assert_eq!(
            replay.diagnostics.channels[0].outputs[0].realization,
            CacheDisposition::Hit
        );
        assert!(Arc::ptr_eq(
            &cached_realization,
            &replay.transaction.realizations[0].1
        ));
        assert!(Arc::ptr_eq(
            cached_realization.realization.geometry(),
            &replay.result.scene().layers()[0].outputs()[0].geometry
        ));
        assert_eq!(replay.result.scene().layers()[0].outputs(), first_outputs);
    }

    /// Proves sampled primitive paint shares its immutable cache allocation with the scene.
    ///
    /// # Panics
    ///
    /// Panics when a sampled-region fixture omits paint or scene construction
    /// deep-clones the renderer-ready cache payload.
    #[test]
    fn sampled_output_realization_shares_scene_paint_payload() {
        let session = modeled_sampled_voronoi_session();
        let evaluated = evaluate_cached_document(
            document_request(&session, valid_document_bytes()),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        )
        .expect("sampled region document evaluates");
        let cached = &evaluated.transaction.realizations[0].1.realization;
        let cached_paints = cached
            .primitive_paints()
            .expect("sampled cache output retains paint");
        let scene_paints = evaluated.result.scene().layers()[0].outputs()[0]
            .primitive_paints
            .as_ref()
            .expect("sampled scene output retains paint");
        assert!(Arc::ptr_eq(
            cached.geometry(),
            &evaluated.result.scene().layers()[0].outputs()[0].geometry
        ));
        assert!(Arc::ptr_eq(cached_paints, scene_paints));
    }

    /// Proves unrestricted output filtering borrows the cached family rather than cloning it.
    ///
    /// This focused identity witness covers only `SiteUseFilter::All`; dependent
    /// filters remain patterns-owned allocating subset operations. It performs
    /// no cache mutation, scheduling, rendering, or unsafe allocation probing.
    #[test]
    fn unrestricted_output_filter_borrows_the_cached_family() {
        let session = modeled_document_session();
        let evaluated = evaluate_cached_document(
            document_request(&session, valid_document_bytes()),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        )
        .expect("document family fixture evaluates");
        let family = &evaluated.transaction.families[0].1;
        let filtered =
            filtered_family_for_output(family.as_ref(), toniator_domain::SiteUseFilter::All, None)
                .expect("unrestricted output filter validates");
        assert!(matches!(&filtered, Cow::Borrowed(_)));
        assert!(std::ptr::eq(filtered.as_ref(), family.as_ref()));
    }

    fn document_request_for_source(
        session: &DocumentSession,
        source_id: &str,
        bytes: Arc<[u8]>,
    ) -> EvaluationRequest {
        document_request_for_format(session, source_id, bytes, SourceFormatHint::Png)
    }

    /// Builds a test evaluation request with an explicit immutable source format.
    fn document_request_for_format(
        session: &DocumentSession,
        source_id: &str,
        bytes: Arc<[u8]>,
        format: SourceFormatHint,
    ) -> EvaluationRequest {
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(SourceReferenceId::new(source_id).unwrap(), bytes, format).unwrap(),
        )
    }

    fn valid_document_bytes() -> Arc<[u8]> {
        Arc::<[u8]>::from(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/raster-sample.png"
            ))
            .unwrap(),
        )
    }

    /// Encodes a constant one-pixel source for cache tests that do not exercise source sampling.
    fn flat_document_bytes() -> Arc<[u8]> {
        let surface = RasterSurface::new(1, 1, vec![127, 127, 127, 255])
            .expect("flat cache-test source validates");
        Arc::<[u8]>::from(encode_png(&surface).expect("flat cache-test source encodes"))
    }

    /// Builds one small typed-parametric-site Voronoi document at an intrinsic evidence canvas.
    ///
    /// The definition deliberately exposes only `AlongParametricCurveSites` to a Region layer;
    /// raw `ParametricPaths` are not substituted as a Region producer.
    fn stage20q_parametric_voronoi_document(
        source_id: SourceReferenceId,
        response: RegionGeometryResponse,
    ) -> Document {
        let base = Document::new_default_document(
            CanvasSpec {
                width: 1024.0,
                height: 1024.0,
            },
            SourceReference::Assigned(source_id),
        )
        .expect("parametric evidence base document validates");
        let definition_id = PatternDefinitionId(20_701);
        let curve_id = PatternMechanismId(20_702);
        let site_id = PatternMechanismId(20_703);
        let output_id = PatternOutputLayerId(20_704);
        let definition = PatternDefinition {
            id: definition_id,
            name: "Stage 20Q typed parametric Voronoi evidence".into(),
            family: toniator_domain::PatternFamily::ParametricCurve {
                curve_mechanism_id: curve_id,
                site_mechanism_id: Some(site_id),
            },
            mechanisms: vec![
                toniator_domain::PatternMechanism::ParametricCurveSource {
                    id: curve_id,
                    curve: ParametricCurve::Spiral(SpiralCurve {
                        shape: SpiralShape::Round,
                        turns: 12.0,
                        radial_spacing: 100.0,
                        phase_degrees: 0.0,
                        winding: CurveWinding::CounterClockwise,
                    }),
                    repetition: CurveRepetition::Single,
                },
                toniator_domain::PatternMechanism::AlongParametricCurveSites {
                    id: site_id,
                    curve_mechanism_id: curve_id,
                    interval: 96.0,
                    phase: 0.0,
                },
            ],
            output_layers: vec![PatternOutputLayer::all(
                output_id,
                PatternOutputRealization::Regions {
                    source: RegionSourceIntent::VoronoiSites {
                        site_mechanism_id: site_id,
                    },
                },
            )],
            modulation: toniator_domain::PatternModulation,
            coverage: CoveragePolicy {
                guard_steps: 8,
                additional_margin: 0.0,
            },
        };
        let mut settings = base.pattern_settings().clone();
        settings.definition_id = definition_id;
        Document::with_source_topology_and_authored_structures(
            DocumentId(20_701),
            base.canvas().clone(),
            base.source().clone(),
            vec![PatternDefinitionBundle {
                definition,
                output_settings: vec![PatternOutputSettings {
                    output_layer_id: output_id,
                    response: PatternGeometryResponse::Regions(response),
                }],
            }],
            settings,
            base.channel_model().expect("evidence model exists"),
            base.channel_topology()
                .expect("evidence topology exists")
                .clone(),
            Vec::new(),
        )
        .expect("typed parametric evidence document validates")
    }

    /// Selects sampled Region paint and channel opacity without introducing a frontend model.
    fn stage20q_sampled_region_session(document: Document, opacity: f64) -> DocumentSession {
        let mut session = DocumentSession::new(document).expect("evidence session starts");
        let channel = &session
            .document()
            .channel_topology()
            .expect("evidence topology exists")
            .channels()[0];
        let template = ChannelTopologyTemplate {
            pattern_instance: channel.pattern_instance.clone(),
        };
        session
            .apply(&DocumentCommand::ReplaceChannelTopology {
                model: HalftoneChannelModel::SourceColorAlpha,
                topology: toniator_domain::ChannelTopology::canonical(
                    HalftoneChannelModel::SourceColorAlpha,
                    template,
                )
                .expect("sampled topology validates"),
            })
            .expect("sampled topology applies");
        let sampled_channel_id = session
            .document()
            .channel_topology()
            .expect("sampled topology exists")
            .channels()[0]
            .id;
        session
            .apply(&DocumentCommand::SetOpacity {
                channel_id: sampled_channel_id,
                opacity,
            })
            .expect("sampled opacity applies");
        session
    }

    /// Replaces the sole Region response while preserving the persisted producer and channel model.
    fn stage20q_region_response_session(
        session: DocumentSession,
        response: RegionGeometryResponse,
    ) -> DocumentSession {
        let document = session.document();
        let mut bundle = document.pattern_definition_bundles()[0].clone();
        bundle.output_settings[0].response = PatternGeometryResponse::Regions(response);
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                document.id(),
                document.canvas().clone(),
                document.source().clone(),
                vec![bundle],
                document.pattern_settings().clone(),
                document
                    .channel_model()
                    .expect("modeled Region has a model"),
                document
                    .channel_topology()
                    .expect("modeled Region has topology")
                    .clone(),
                document.authored_structures().to_vec(),
            )
            .expect("typed Region response document validates"),
        )
        .expect("typed Region response session starts")
    }

    /// Replaces only typed family density for a compact treatment witness without changing outputs.
    fn stage20q_density_session(session: DocumentSession, across_x: f64) -> DocumentSession {
        let document = session.document();
        let mut settings = document.pattern_settings().clone();
        settings.density = authored_density(
            document.canvas(),
            across_x,
            across_x * document.canvas().height / document.canvas().width,
        );
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                document.id(),
                document.canvas().clone(),
                document.source().clone(),
                document.pattern_definition_bundles().to_vec(),
                settings,
                document
                    .channel_model()
                    .expect("modeled Region has a model"),
                document
                    .channel_topology()
                    .expect("modeled Region has topology")
                    .clone(),
                document.authored_structures().to_vec(),
            )
            .expect("compact Region density document validates"),
        )
        .expect("compact Region density session starts")
    }

    /// Computes deterministic raw RGBA statistics without flattening or compositing evidence.
    fn stage20q_rgba_statistics(width: u32, height: u32, pixels: &[u8]) -> String {
        let mut sums = [0_u64; 4];
        let mut nonzero_alpha = 0_usize;
        for pixel in pixels.chunks_exact(4) {
            for (sum, value) in sums.iter_mut().zip(pixel) {
                *sum += u64::from(*value);
            }
            nonzero_alpha += usize::from(pixel[3] != 0);
        }
        let count = width as u64 * height as u64;
        format!(
            "dimensions={width}x{height}\npixels={count}\nnonzero_alpha={nonzero_alpha}\nmean_rgba={:.6},{:.6},{:.6},{:.6}\n",
            sums[0] as f64 / count as f64,
            sums[1] as f64 / count as f64,
            sums[2] as f64 / count as f64,
            sums[3] as f64 / count as f64,
        )
    }

    /// Returns the SHA-256 identity for one immutable raw validation artifact.
    fn stage20q_sha256(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    /// Saves, reloads, evaluates once, and exports one Guide Face evidence case with its snapshot.
    ///
    /// The returned result and snapshot are the exact normal engine invocation used for the
    /// records and exports. The helper never rebuilds producer, sampling, or treatment state.
    #[allow(clippy::too_many_arguments)]
    fn stage20q_write_guide_face_case(
        output: &Path,
        stem: &str,
        session: DocumentSession,
        input: &Path,
        format: EmbeddedSourceFormat,
        hint: SourceFormatHint,
        label: &str,
    ) -> (EvaluationResult, RegionEvaluationEvidence) {
        let SourceReference::Assigned(source_id) = session.document().source() else {
            panic!("Guide Face evidence requires an assigned source");
        };
        let source_id = source_id.clone();
        let source_bytes = fs::read(input).expect("immutable evidence source reads");
        let source = EmbeddedSource::new(
            source_id.clone(),
            format,
            source_bytes,
            input
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
        )
        .expect("Guide Face evidence source embeds");
        let sources = SourceBundle::new([source]).expect("one Guide Face source validates");
        let document_path = output.join(format!("{stem}.toniator"));
        save(&document_path, session.document(), &sources).expect("current v5 Guide Face saves");
        let loaded = load(&document_path).expect("current v5 Guide Face loads");
        assert_eq!(loaded.versions().document(), 5);
        let loaded_source = loaded
            .sources()
            .get(&source_id)
            .expect("loaded Guide Face source exists");
        let loaded_session =
            DocumentSession::new(loaded.document().clone()).expect("loaded Guide Face starts");
        let (evaluated, evidence) = evaluate_cached_document_with_region_evidence(
            EvaluationRequest::new(
                loaded_session.document_evaluation_snapshot(),
                ResolvedSource::new(source_id, loaded_source.bytes().to_vec(), hint)
                    .expect("loaded Guide Face source resolves"),
            ),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        );
        let evaluated = evaluated.unwrap_or_else(|error| {
            panic!("normal Guide Face Region evaluation succeeds for {label}: {error:?}")
        });
        let evidence = evidence.expect("normal Guide Face invocation records evidence");
        let png = encode_png(evaluated.result.raster()).expect("Guide Face native PNG encodes");
        let svg = write_svg(evaluated.result.scene());
        let png_path = output.join(format!("{stem}.png"));
        let svg_path = output.join(format!("{stem}.svg"));
        fs::write(&png_path, &png).expect("Guide Face native PNG writes");
        fs::write(&svg_path, &svg).expect("Guide Face raw SVG writes");
        let svg_raster_path = output.join(format!("{stem}-svg-rasterized.png"));
        let status = Command::new("inkscape")
            .arg(&svg_path)
            .arg("--export-type=png")
            .arg(format!("--export-filename={}", svg_raster_path.display()))
            .status()
            .expect("Inkscape is available for Guide Face evidence");
        assert!(
            status.success(),
            "Guide Face raw SVG rasterizes with Inkscape"
        );
        let svg_raster = image::load_from_memory(
            &fs::read(&svg_raster_path).expect("Guide Face SVG-rasterized PNG reads"),
        )
        .expect("Guide Face SVG-rasterized PNG decodes")
        .to_rgba8();
        fs::write(
            output.join(format!("{stem}-native-rgba-stats.txt")),
            stage20q_rgba_statistics(
                evaluated.result.raster().width(),
                evaluated.result.raster().height(),
                evaluated.result.raster().pixels(),
            ),
        )
        .expect("Guide Face native statistics write");
        fs::write(
            output.join(format!("{stem}-svg-raster-rgba-stats.txt")),
            stage20q_rgba_statistics(svg_raster.width(), svg_raster.height(), svg_raster.as_raw()),
        )
        .expect("Guide Face SVG-raster statistics write");
        fs::write(
            output.join(format!("{stem}-identity.txt")),
            format!(
                "label={label}\nevaluation_fingerprint={}\ncache_diagnostics={:?}\n\
                 untreated_regions={:#?}\nuntreated_ids={:#?}\nreferences={:#?}\n\
                 sampling={:?}\nsamples={:#?}\ntreatments={:#?}\n\
                 treated_regions={:#?}\ntreated_provenance={:#?}\nuntreated_fingerprint={}\n\
                 treated_fingerprint={}\ntyped_realization_fingerprint={}\n\
                 realizer_diagnostics={:?}\n",
                evaluated.result.channels()[0].realization_identity(),
                evaluated.diagnostics,
                evidence.untreated_regions,
                evidence.untreated_region_ids,
                evidence.references,
                evidence.sampling,
                evidence.samples,
                evidence.treatments,
                evidence.treated_regions,
                evidence.provenance,
                evidence.untreated_fingerprint,
                evidence.treated_fingerprint,
                evidence.realization_fingerprint,
                evidence.diagnostics,
            ),
        )
        .expect("Guide Face identity record writes");
        fs::write(
            output.join(format!("{stem}-hashes.txt")),
            format!(
                "{}  {}\n{}  {}\n{}  {}\n{}  {}\n",
                stage20q_sha256(&fs::read(&document_path).expect("Guide Face document reads")),
                document_path.file_name().unwrap().to_string_lossy(),
                stage20q_sha256(&png),
                png_path.file_name().unwrap().to_string_lossy(),
                stage20q_sha256(svg.as_bytes()),
                svg_path.file_name().unwrap().to_string_lossy(),
                stage20q_sha256(&fs::read(&svg_raster_path).expect("Guide Face raster reads")),
                svg_raster_path.file_name().unwrap().to_string_lossy(),
            ),
        )
        .expect("Guide Face hashes write");
        (evaluated.result, evidence)
    }

    /// Writes a direct geometry-render split witness after production cubic Guide Faces retain one component per base.
    ///
    /// The witness remains legacy Stage 20Q evidence only and is not part of current normalized resize coverage.
    fn stage20q_write_direct_split_witness(output: &Path) {
        let output_layer_id = PatternOutputLayerId(20_709);
        let source = build_canonical_regions_cancellable(
            CanonicalRegionProposal {
                output_layer_id,
                source_groups: vec![CanonicalRegionSourceGroup {
                    source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                        mechanism_id: PatternMechanismId(20_708),
                        ordinal: 0,
                    }]),
                    components: vec![
                        CurvePath::polyline(
                            vec![
                                Point2::new(60.0, 60.0),
                                Point2::new(120.0, 60.0),
                                Point2::new(120.0, 95.0),
                                Point2::new(180.0, 95.0),
                                Point2::new(180.0, 60.0),
                                Point2::new(240.0, 60.0),
                                Point2::new(240.0, 140.0),
                                Point2::new(180.0, 140.0),
                                Point2::new(180.0, 105.0),
                                Point2::new(120.0, 105.0),
                                Point2::new(120.0, 140.0),
                                Point2::new(60.0, 140.0),
                            ],
                            PathClosure::Closed,
                        )
                        .expect("direct split witness path closes"),
                    ],
                }],
            },
            CanonicalRegionLimits::default(),
            || false,
        )
        .expect("direct split source canonicalizes")
        .0;
        let base = source
            .regions()
            .first()
            .expect("direct split source retains its base");
        let request = RegionTreatmentRequest {
            base_region_id: base.id.clone(),
            reference: Some(Point2::new(150.0, 100.0)),
            treatment: Some(RegionTreatment {
                algorithm: toniator_domain::RegionResizeAlgorithm::UniformOffset,
                fill: 0.5,
            }),
        };
        let treated = treat_region_requests_cancellable(
            output_layer_id,
            &source,
            std::slice::from_ref(&request),
            RegionTreatmentLimits::default(),
            || false,
        )
        .expect("direct split treatment succeeds");
        let replayed = treat_region_requests_cancellable(
            output_layer_id,
            &source,
            std::slice::from_ref(&request),
            RegionTreatmentLimits::default(),
            || false,
        )
        .expect("direct split treatment replays");
        assert_eq!(treated, replayed, "direct split treatment replays exactly");
        assert!(
            treated.provenance.len() > 1,
            "narrow-neck inward offset splits one untreated base into multiple components"
        );
        assert!(
            treated
                .provenance
                .iter()
                .all(|provenance| provenance.base_region_id == base.id),
            "every split component retains its untreated base provenance"
        );
        assert!(
            treated
                .regions
                .regions()
                .iter()
                .enumerate()
                .all(|(ordinal, region)| {
                    region.id.source_id == base.id.source_id
                        && region.id.component_ordinal == ordinal as u32
                        && region.area.is_finite()
                        && region.area > 0.0
                }),
            "canonical split components retain positive winding and contiguous source ordinals"
        );
        let scene = RenderScene::new(
            CanvasSpec {
                width: 300.0,
                height: 200.0,
            },
            "stage20q-direct-split-family".into(),
            treated.regions.fingerprint().to_owned(),
            vec![
                RenderLayer::new_for_output(
                    ChannelId(1),
                    true,
                    ColorValue {
                        red: 0.15,
                        green: 0.55,
                        blue: 0.85,
                        alpha: 1.0,
                    },
                    1.0,
                    output_layer_id,
                    GeometryOutput::CanonicalRegions(treated.regions.clone()),
                )
                .expect("direct split renderer layer validates"),
            ],
        )
        .expect("direct split renderer scene validates");
        let raster = rasterize(&scene, RasterBackground::Transparent)
            .expect("direct split renderer rasterizes");
        let png = encode_png(&raster).expect("direct split PNG encodes");
        let svg = write_svg(&scene);
        let png_path = output.join("crossing-split-direct-geometry-render.png");
        let svg_path = output.join("crossing-split-direct-geometry-render.svg");
        fs::write(&png_path, &png).expect("direct split PNG writes");
        fs::write(&svg_path, &svg).expect("direct split SVG writes");
        let svg_raster_path =
            output.join("crossing-split-direct-geometry-render-svg-rasterized.png");
        let status = Command::new("inkscape")
            .arg(&svg_path)
            .arg("--export-type=png")
            .arg(format!("--export-filename={}", svg_raster_path.display()))
            .status()
            .expect("Inkscape is available for direct split evidence");
        assert!(
            status.success(),
            "direct split SVG rasterizes with Inkscape"
        );
        let svg_raster = image::load_from_memory(
            &fs::read(&svg_raster_path).expect("direct split SVG raster reads"),
        )
        .expect("direct split SVG raster decodes")
        .to_rgba8();
        fs::write(
            output.join("crossing-split-direct-geometry-render-native-rgba-stats.txt"),
            stage20q_rgba_statistics(raster.width(), raster.height(), raster.pixels()),
        )
        .expect("direct split native statistics write");
        fs::write(
            output.join("crossing-split-direct-geometry-render-svg-raster-rgba-stats.txt"),
            stage20q_rgba_statistics(svg_raster.width(), svg_raster.height(), svg_raster.as_raw()),
        )
        .expect("direct split SVG statistics write");
        fs::write(
            output.join("crossing-split-direct-geometry-render-identity.txt"),
            format!(
                "provenance=direct geometry-render witness; no document, source sampling, or persistence is claimed\n\
                 untreated_regions={source:#?}\nrequest={request:#?}\ntreated_regions={:#?}\n\
                 provenance={:#?}\nuntreated_fingerprint={}\ntreated_fingerprint={}\n",
                treated.regions,
                treated.provenance,
                source.fingerprint(),
                treated.regions.fingerprint(),
            ),
        )
        .expect("direct split identity record writes");
        fs::write(
            output.join("crossing-split-direct-geometry-render-hashes.txt"),
            format!(
                "{}  {}\n{}  {}\n{}  {}\n",
                stage20q_sha256(&png),
                png_path.file_name().expect("PNG name").to_string_lossy(),
                stage20q_sha256(svg.as_bytes()),
                svg_path.file_name().expect("SVG name").to_string_lossy(),
                stage20q_sha256(&fs::read(&svg_raster_path).expect("SVG raster reads")),
                svg_raster_path
                    .file_name()
                    .expect("SVG raster name")
                    .to_string_lossy(),
            ),
        )
        .expect("direct split hashes write");
    }

    /// Generates typed-parametric ordinary Voronoi Stage 20Q evidence through save/load/evaluate/export.
    ///
    /// The ignored generator writes only derived validation files. Each record is collected from
    /// the one normal engine evaluation that produced its native PNG and raw SVG; no diagnostic
    /// path reconstructs sites, samples, or treatment geometry.
    #[test]
    #[ignore = "writes Stage 20Q typed-parametric Voronoi validation artifacts"]
    fn generate_stage20q_parametric_voronoi_artifacts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = root.join("target/validation/stage20q/voronoi-parametric");
        fs::create_dir_all(&output).expect("Stage 20Q validation directory exists");
        let input = root.join("assets/raster-sample.png");
        let bytes = fs::read(&input).expect("immutable raster source reads");
        let source_id = SourceReferenceId::new("stage20q-parametric-raster").expect("source ID");
        let source = EmbeddedSource::new(
            source_id.clone(),
            EmbeddedSourceFormat::Png,
            bytes.clone(),
            Some("raster-sample.png".into()),
        )
        .expect("immutable raster source embeds");
        let sources = SourceBundle::new([source]).expect("one embedded source validates");
        let cases = [
            (
                "unit-reference-solid",
                RegionGeometryResponse {
                    algorithm: toniator_domain::RegionResizeAlgorithm::Scale,
                    sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                    minimum_fill: 1.0,
                    maximum_fill: 1.0,
                },
                false,
                1.0,
            ),
            (
                "scale-reference-sampled-opacity",
                RegionGeometryResponse {
                    algorithm: toniator_domain::RegionResizeAlgorithm::Scale,
                    sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                    minimum_fill: 0.0,
                    maximum_fill: 1.35,
                },
                true,
                0.62,
            ),
            (
                "scale-area-average-sampled",
                RegionGeometryResponse {
                    algorithm: toniator_domain::RegionResizeAlgorithm::Scale,
                    sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
                    minimum_fill: 0.5,
                    maximum_fill: 1.25,
                },
                true,
                0.62,
            ),
            (
                "scale-collapse-empty",
                RegionGeometryResponse {
                    algorithm: toniator_domain::RegionResizeAlgorithm::Scale,
                    sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                    minimum_fill: 0.0,
                    maximum_fill: 0.0,
                },
                false,
                1.0,
            ),
        ];
        let mut manifest = String::from(
            "# Stage 20Q typed-parametric Voronoi fragment\n\n\
             All cases use `assets/raster-sample.png` unchanged at 1024×1024 and a current v5 archive.\n\n",
        );
        for (stem, response, sampled, opacity) in cases {
            let document =
                stage20q_parametric_voronoi_document(source_id.clone(), response.clone());
            let authored_session = if sampled {
                stage20q_sampled_region_session(document, opacity)
            } else {
                DocumentSession::new(document).expect("solid evidence session starts")
            };
            let document_path = output.join(format!("{stem}.toniator"));
            save(&document_path, authored_session.document(), &sources)
                .expect("current v5 evidence document saves");
            let loaded = load(&document_path).expect("current v5 evidence document loads");
            assert_eq!(loaded.versions().document(), 5);
            let loaded_source = loaded
                .sources()
                .get(&source_id)
                .expect("loaded source exists");
            let loaded_session = DocumentSession::new(loaded.document().clone())
                .expect("loaded current document starts a session");
            let (evaluated, evidence) = evaluate_cached_document_with_region_evidence(
                EvaluationRequest::new(
                    loaded_session.document_evaluation_snapshot(),
                    ResolvedSource::new(
                        source_id.clone(),
                        loaded_source.bytes().to_vec(),
                        SourceFormatHint::Png,
                    )
                    .expect("loaded source resolves"),
                ),
                EvaluationLimits::default(),
                &DocumentDerivedCache::default(),
                &NeverCancelled,
            );
            let evaluated = evaluated.expect("normal typed-parametric Region evaluation succeeds");
            let evidence = evidence.expect("normal Region invocation records test evidence");
            assert!(
                !evidence.untreated_region_ids.is_empty(),
                "typed AlongParametricCurveSites produces ordinary Voronoi bases"
            );
            if stem == "scale-collapse-empty" {
                assert!(
                    evidence.provenance.is_empty(),
                    "zero Scale removes every treated component without a transparent fallback"
                );
            }
            assert!(
                matches!(
                    loaded.document().pattern_definition_bundles()[0]
                        .definition
                        .output_layers[0],
                    PatternOutputLayer {
                        realization: PatternOutputRealization::Regions {
                            source: RegionSourceIntent::VoronoiSites { .. },
                        },
                        ..
                    }
                ),
                "raw ParametricPaths is never repurposed as a Region producer"
            );
            let mut raw_parametric_paths = loaded.document().pattern_definition_bundles()[0]
                .definition
                .clone();
            raw_parametric_paths.output_layers = vec![PatternOutputLayer::all(
                PatternOutputLayerId(20_705),
                PatternOutputRealization::ParametricPaths {
                    curve_mechanism_id: PatternMechanismId(20_702),
                    style: PathStrokeStyle::default(),
                },
            )];
            assert_eq!(
                toniator_patterns::resolve_pattern_pipeline(&raw_parametric_paths)
                    .expect_err("raw ParametricPaths is not an eligible Region producer")
                    .path(),
                "pattern.output_layers.capability",
                "a raw ParametricPaths output remains Guide-Faces-ineligible"
            );
            let png = encode_png(evaluated.result.raster()).expect("native PNG encodes");
            let svg = write_svg(evaluated.result.scene());
            let png_path = output.join(format!("{stem}.png"));
            let svg_path = output.join(format!("{stem}.svg"));
            fs::write(&png_path, &png).expect("native PNG writes");
            fs::write(&svg_path, &svg).expect("raw SVG writes");
            let svg_raster_path = output.join(format!("{stem}-svg-rasterized.png"));
            let status = Command::new("inkscape")
                .arg(&svg_path)
                .arg("--export-type=png")
                .arg(format!("--export-filename={}", svg_raster_path.display()))
                .status()
                .expect("Inkscape is available for Stage 20Q evidence");
            assert!(status.success(), "raw SVG rasterizes with Inkscape");
            let rasterized = image::load_from_memory(
                &fs::read(&svg_raster_path).expect("SVG-rasterized PNG reads"),
            )
            .expect("SVG-rasterized PNG decodes")
            .to_rgba8();
            fs::write(
                output.join(format!("{stem}-native-rgba-stats.txt")),
                stage20q_rgba_statistics(
                    evaluated.result.raster().width(),
                    evaluated.result.raster().height(),
                    evaluated.result.raster().pixels(),
                ),
            )
            .expect("native RGBA statistics write");
            fs::write(
                output.join(format!("{stem}-svg-raster-rgba-stats.txt")),
                stage20q_rgba_statistics(
                    rasterized.width(),
                    rasterized.height(),
                    rasterized.as_raw(),
                ),
            )
            .expect("SVG-rasterized RGBA statistics write");
            fs::write(
                output.join(format!("{stem}-identity.txt")),
                format!(
                    "case={stem}\nresponse={response:?}\nsampled_paint={sampled}\nopacity={opacity}\n\
                     evaluation_fingerprint={}\ncache_diagnostics={:?}\n\
                     untreated_ids={:#?}\nreferences={:#?}\nsamples={:#?}\n\
                     treatments={:#?}\ntreated_provenance={:#?}\n\
                     untreated_fingerprint={}\ntreated_fingerprint={}\n\
                     typed_realization_fingerprint={}\nrealizer_diagnostics={:?}\n",
                    evaluated.result.channels()[0].realization_identity(),
                    evaluated.diagnostics,
                    evidence.untreated_region_ids,
                    evidence.references,
                    evidence.samples,
                    evidence.treatments,
                    evidence.provenance,
                    evidence.untreated_fingerprint,
                    evidence.treated_fingerprint,
                    evidence.realization_fingerprint,
                    evidence.diagnostics,
                ),
            )
            .expect("Region identity record writes");
            fs::write(
                output.join(format!("{stem}-hashes.txt")),
                format!(
                    "{}  {}\n{}  {}\n{}  {}\n",
                    stage20q_sha256(&fs::read(&document_path).expect("document reads")),
                    document_path.file_name().unwrap().to_string_lossy(),
                    stage20q_sha256(&png),
                    png_path.file_name().unwrap().to_string_lossy(),
                    stage20q_sha256(&fs::read(&svg_raster_path).expect("raster reads")),
                    svg_raster_path.file_name().unwrap().to_string_lossy(),
                ),
            )
            .expect("artifact hashes write");
            manifest.push_str(&format!(
                "- `{stem}`: response `{response:?}`, sampled paint `{sampled}`, opacity `{opacity}`; \
                 `{stem}.toniator`, `{stem}.png`, `{stem}.svg`, `{stem}-svg-rasterized.png`, \
                 identity, hashes, and separate native/SVG-raster RGBA statistics.\n"
            ));
        }
        fs::write(output.join("MANIFEST.fragment.md"), manifest).expect("manifest fragment writes");
    }

    /// Generates production Guide Face UniformOffset evidence from one observed engine invocation each.
    ///
    /// These witnesses preserve the current v5 document boundary, final-canvas-only rendering,
    /// and test-only snapshot provenance. No four-guide or raw-parametric-path producer is used.
    #[test]
    #[ignore = "writes superseded Stage 20Q Guide Face UniformOffset validation artifacts"]
    fn generate_stage20q_guide_face_uniform_offset_artifacts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = root.join("target/validation/stage20q/guide-faces");
        fs::create_dir_all(&output).expect("Guide Face validation directory exists");
        let raster_input = root.join("assets/raster-sample.png");
        let vector_input = root.join("assets/vector-sample.svg");
        let mut manifest = String::from(
            "# Stage 20Q Guide Face UniformOffset fragment\n\n\
             Every case saves and reloads a current schema-v5 document, then exports the one normal \
             engine evaluation to raw RGBA PNG and raw SVG plus an Inkscape SVG raster.\n\n",
        );

        let two_guide = stage20q_region_response_session(
            modeled_guide_face_session_for_canvas(
                vec![GuideDimensionId(123), GuideDimensionId(124)],
                CanvasSpec {
                    width: 1024.0,
                    height: 1024.0,
                },
            ),
            RegionGeometryResponse {
                algorithm: toniator_domain::RegionResizeAlgorithm::UniformOffset,
                sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                minimum_fill: 0.65,
                maximum_fill: 0.65,
            },
        );
        let (two_result, two_evidence) = stage20q_write_guide_face_case(
            &output,
            "two-guide-raster-uniform-offset-reference-solid",
            two_guide,
            &raster_input,
            EmbeddedSourceFormat::Png,
            SourceFormatHint::Png,
            "production two-guide UniformOffset fill / ReferencePoint / solid",
        );
        assert!(
            !two_evidence.provenance.is_empty(),
            "moderate positive-region UniformOffset retains two-guide components"
        );
        assert_eq!(
            two_result.raster().width(),
            1024,
            "renderer performs only final native canvas clipping"
        );
        manifest.push_str(
            "- `two-guide-raster-uniform-offset-reference-solid`: 1024×1024 raster source, positive-region UniformOffset fill, ReferencePoint and solid paint.\n",
        );

        let collapse = stage20q_region_response_session(
            stage20q_density_session(
                modeled_guide_face_session_for_canvas(
                    vec![GuideDimensionId(123), GuideDimensionId(124)],
                    CanvasSpec {
                        width: 1024.0,
                        height: 1024.0,
                    },
                ),
                64.0,
            ),
            RegionGeometryResponse {
                algorithm: toniator_domain::RegionResizeAlgorithm::UniformOffset,
                sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                minimum_fill: 0.0,
                maximum_fill: 0.0,
            },
        );
        let (collapse_result, collapse_evidence) = stage20q_write_guide_face_case(
            &output,
            "two-guide-raster-zero-fill-collapse",
            collapse,
            &raster_input,
            EmbeddedSourceFormat::Png,
            SourceFormatHint::Png,
            "production two-guide zero UniformOffset fill",
        );
        assert!(
            collapse_evidence.provenance.is_empty(),
            "zero normalized fill omits every dense two-guide component"
        );
        assert_eq!(collapse_result.raster().width(), 1024);
        manifest.push_str(
            "- `two-guide-raster-zero-fill-collapse`: 1024×1024 raster source, zero normalized fill omits treated geometry.\n",
        );

        let three_base = stage20q_region_response_session(
            modeled_guide_face_session_for_canvas(
                vec![
                    GuideDimensionId(123),
                    GuideDimensionId(124),
                    GuideDimensionId(125),
                ],
                CanvasSpec {
                    width: 900.0,
                    height: 620.0,
                },
            ),
            RegionGeometryResponse {
                algorithm: toniator_domain::RegionResizeAlgorithm::UniformOffset,
                sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
                minimum_fill: 0.7,
                maximum_fill: 0.7,
            },
        );
        let three_guide = stage20q_sampled_region_session(three_base.document().clone(), 0.58);
        let (three_result, three_evidence) = stage20q_write_guide_face_case(
            &output,
            "three-guide-vector-uniform-offset-area-average-sampled-opacity",
            three_guide,
            &vector_input,
            EmbeddedSourceFormat::Svg,
            SourceFormatHint::Svg,
            "production 0/60/120 three-guide UniformOffset fill / AreaAverage / sampled paint / opacity",
        );
        assert_production_equilateral_untreated_faces(&three_evidence.untreated_regions);
        let treated_line_faces: Vec<_> = three_evidence
            .treated_regions
            .regions()
            .iter()
            .filter(|region| {
                region
                    .ring
                    .segments()
                    .iter()
                    .all(|segment| matches!(segment, CurveSegment::Line(_)))
            })
            .collect();
        assert!(
            !treated_line_faces.is_empty()
                && treated_line_faces
                    .iter()
                    .all(|region| region.ring.segments().len() == 3),
            "UniformOffset-resized three-guide line faces retain triangular rings"
        );
        assert_eq!(
            three_evidence.sampling,
            toniator_domain::RegionSamplingStrategy::AreaAverage
        );
        assert_eq!(
            three_evidence.samples.len(),
            three_evidence.untreated_regions.regions().len(),
            "AreaAverage samples every complete untreated base exactly once"
        );
        assert!(
            three_evidence
                .untreated_regions
                .regions()
                .iter()
                .any(|region| {
                    region.bounds.min.x < 0.0
                        || region.bounds.min.y < 0.0
                        || region.bounds.max.x > 900.0
                        || region.bounds.max.y > 620.0
                }),
            "producer retains off-canvas untreated faces before final render clipping"
        );
        assert_eq!(three_result.raster().width(), 900);
        assert_eq!(three_result.raster().height(), 620);
        manifest.push_str(
            "- `three-guide-vector-uniform-offset-area-average-sampled-opacity`: production 0/60/120 equilateral untreated faces, normalized positive-region UniformOffset fill, complete AreaAverage sampling, sampled paint, and opacity 0.58 at 900×620.\n",
        );

        let cubic_base = stage20q_region_response_session(
            modeled_authored_cubic_guide_face_session_for_canvas(CanvasSpec {
                width: 900.0,
                height: 620.0,
            }),
            RegionGeometryResponse {
                algorithm: toniator_domain::RegionResizeAlgorithm::UniformOffset,
                sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
                minimum_fill: 1.25,
                maximum_fill: 1.25,
            },
        );
        let cubic = stage20q_sampled_region_session(cubic_base.document().clone(), 0.71);
        let (cubic_result, cubic_evidence) = stage20q_write_guide_face_case(
            &output,
            "authored-cubic-vector-uniform-offset-area-average-sampled",
            cubic,
            &vector_input,
            EmbeddedSourceFormat::Svg,
            SourceFormatHint::Svg,
            "production authored-cubic Guide Faces / UniformOffset fill / AreaAverage / sampled paint",
        );
        assert!(
            cubic_evidence
                .untreated_regions
                .regions()
                .iter()
                .any(|region| {
                    region
                        .ring
                        .segments()
                        .iter()
                        .any(|segment| matches!(segment, CurveSegment::CubicBezier(_)))
                }),
            "authored cubic Guide Face reaches the untouched producer snapshot"
        );
        assert!(
            cubic_evidence
                .treated_regions
                .regions()
                .iter()
                .any(|region| stage20q_has_tangent_continuous_cubic_join(&region.ring)),
            "outward authored-cubic faces retain a tangent-continuous cubic corner join instead of a straight bevel"
        );
        assert!(
            cubic_evidence
                .treated_regions
                .regions()
                .iter()
                .flat_map(|region| region.ring.segments())
                .filter_map(|segment| match segment {
                    CurveSegment::CubicBezier(cubic) => Some(cubic),
                    CurveSegment::Line(_) => None,
                })
                .all(|cubic| {
                    (cubic.start().x - cubic.end().x).hypot(cubic.start().y - cubic.end().y)
                        > 1.0e-6
                }),
            "authored-cubic UniformOffset evidence contains no zero-length cubic seam"
        );
        assert!(
            cubic_result
                .raster()
                .pixels()
                .chunks_exact(4)
                .any(|pixel| pixel[3] != 0)
        );
        manifest.push_str(
            "- `authored-cubic-vector-uniform-offset-area-average-sampled`: genuinely authored cubic Guide Faces at 900×620 with sampled AreaAverage UniformOffset treatment.\n",
        );

        stage20q_write_direct_split_witness(&output);
        manifest.push_str(
            "- `crossing-split-direct-geometry-render`: direct geometry/render narrow-neck positive-region offset witness. The bounded production authored-cubic UniformOffset case retained one component per base, so this explicitly labeled direct case proves deterministic split ordinals, positive winding, base provenance, replay identity, and final-canvas rendering without claiming document/source sampling evidence.\n",
        );

        fs::write(output.join("MANIFEST.fragment.md"), manifest)
            .expect("Guide Face manifest fragment writes");
    }

    /// Loads the immutable project-wide SVG fixture for source-aware Stage 20P witnesses.
    fn valid_vector_document_bytes() -> Arc<[u8]> {
        Arc::<[u8]>::from(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/vector-sample.svg"
            ))
            .unwrap(),
        )
    }

    fn wait_for_document_completion(scheduler: &EvaluationScheduler) -> EvaluationCompletion {
        let deadline = Instant::now() + GUARD;
        loop {
            if let Some(completion) = scheduler.try_receive_latest().unwrap() {
                return completion;
            }
            assert!(Instant::now() < deadline, "document scheduler timed out");
            thread::yield_now();
        }
    }

    /// Proves worker progress is ticketed, monotonic, staged, and complete before publication.
    #[test]
    fn document_scheduler_reports_monotonic_stage_progress() {
        let scheduler = EvaluationScheduler::new().expect("progress scheduler starts");
        let session = modeled_document_session();
        let ticket = scheduler
            .submit(document_request(&session, valid_document_bytes()))
            .expect("progress request submits");
        let deadline = Instant::now() + GUARD;
        let mut progress = Vec::new();
        let completion = loop {
            while let Some(update) = scheduler.try_receive_latest_progress().unwrap() {
                assert_eq!(update.ticket(), ticket);
                progress.push(update);
            }
            if let Some(completion) = scheduler.try_receive_latest().unwrap() {
                break completion;
            }
            assert!(Instant::now() < deadline, "progress scheduler timed out");
            thread::yield_now();
        };
        while let Some(update) = scheduler.try_receive_latest_progress().unwrap() {
            progress.push(update);
        }

        assert_eq!(completion.ticket(), ticket);
        assert!(progress.len() >= 7, "coarse stages remain observable");
        assert!(
            progress
                .windows(2)
                .all(|pair| { pair[0].completed_per_mille <= pair[1].completed_per_mille })
        );
        assert!(
            progress
                .iter()
                .all(|update| (0.0..=1.0).contains(&update.stage_fraction()))
        );
        assert!(progress.windows(2).any(|pair| {
            pair[0].stage() == EvaluationProgressStage::RealizingOutputs
                && pair[1].stage() == EvaluationProgressStage::RealizingOutputs
                && pair[0].completed_per_mille < pair[1].completed_per_mille
        }));
        assert!(progress.windows(2).any(|pair| {
            pair[0].stage() == EvaluationProgressStage::RasterizingPreview
                && pair[1].stage() == EvaluationProgressStage::RasterizingPreview
                && pair[0].completed_per_mille < pair[1].completed_per_mille
                && pair[0].stage_fraction() < pair[1].stage_fraction()
        }));
        assert_eq!(
            progress.first().unwrap().stage(),
            EvaluationProgressStage::Preparing
        );
        assert_eq!(
            progress.last().unwrap().stage(),
            EvaluationProgressStage::Complete
        );
        assert_eq!(progress.last().unwrap().fraction(), 1.0);
        assert_eq!(progress.last().unwrap().stage_fraction(), 1.0);
        scheduler.shutdown().unwrap();
    }

    /// Proves repeated inner-loop polls cannot enqueue an unbounded run of identical progress.
    #[test]
    fn scheduler_progress_coalesces_duplicate_worker_observations() {
        let cancelled = AtomicBool::new(false);
        let (sender, receiver) = mpsc::channel();
        let last = Mutex::new(None);
        let probe = SchedulerCancellation {
            cancelled: &cancelled,
            ticket: EvaluationTicket(7),
            progress: &sender,
            last_progress: &last,
            gate: None,
        };
        for _ in 0..100_000 {
            probe.report_progress(EvaluationProgressStage::RealizingOutputs, 425, 500);
        }
        probe.report_progress(EvaluationProgressStage::RealizingOutputs, 425, 501);
        probe.report_progress(EvaluationProgressStage::RealizingOutputs, 424, 900);
        let updates = receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].fraction(), 0.425);
        assert_eq!(updates[0].stage_fraction(), 0.5);
        assert_eq!(updates[1].stage_fraction(), 0.501);
    }

    #[test]
    fn actual_pipeline_cancels_at_each_named_before_and_after_checkpoint() {
        for checkpoint in [EvaluationCheckpoint::Before, EvaluationCheckpoint::After] {
            for stage in [
                EvaluationStage::Family,
                EvaluationStage::Decode,
                EvaluationStage::Realization,
                EvaluationStage::Scene,
                EvaluationStage::Raster,
            ] {
                let (gate, entered) = EvaluationStageGate::new(stage, checkpoint);
                let cancelled = Arc::new(AtomicBool::new(false));
                let worker_gate = Arc::clone(&gate);
                let worker_cancelled = Arc::clone(&cancelled);
                let (result_sender, result_receiver) = mpsc::channel();
                let worker = thread::spawn(move || {
                    result_sender
                        .send(evaluate_channel_diagnostic_cancellable_with_gate(
                            request(),
                            &worker_cancelled,
                            &worker_gate,
                        ))
                        .unwrap();
                });
                entered.recv_timeout(GUARD).unwrap();
                cancelled.store(true, Ordering::Release);
                gate.release();
                assert_eq!(
                    result_receiver.recv_timeout(GUARD).unwrap(),
                    Err(EvaluationRunError::Cancelled),
                    "{stage:?} {checkpoint:?}"
                );
                worker.join().unwrap();
            }
        }
    }

    #[test]
    fn cancellation_after_a_real_decode_error_suppresses_the_failure() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Decode, EvaluationCheckpoint::After);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_gate = Arc::clone(&gate);
        let worker_cancelled = Arc::clone(&cancelled);
        let (result_sender, result_receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            result_sender
                .send(evaluate_channel_diagnostic_cancellable_with_gate(
                    request_with_bytes(Arc::<[u8]>::from(vec![1_u8, 2, 3])),
                    &worker_cancelled,
                    &worker_gate,
                ))
                .unwrap();
        });
        entered.recv_timeout(GUARD).unwrap();
        cancelled.store(true, Ordering::Release);
        gate.release();
        assert_eq!(
            result_receiver.recv_timeout(GUARD).unwrap(),
            Err(EvaluationRunError::Cancelled)
        );
        worker.join().unwrap();
    }

    #[test]
    fn complete_scheduler_cancellation_after_decode_error_is_silent() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Decode, EvaluationCheckpoint::After);
        let scheduler =
            EvaluationScheduler::new_with_test_gate(EvaluationLimits::default(), Arc::clone(&gate))
                .unwrap();
        let session = modeled_document_session();
        let stale_ticket = scheduler
            .submit(document_request(
                &session,
                Arc::<[u8]>::from(vec![1_u8, 2, 3]),
            ))
            .unwrap();
        entered.recv_timeout(GUARD).unwrap();
        let newest_ticket = scheduler
            .submit(document_request(&session, valid_document_bytes()))
            .unwrap();
        gate.release();

        let completion = wait_for_document_completion(&scheduler);
        assert_ne!(stale_ticket, newest_ticket);
        assert_eq!(completion.ticket(), newest_ticket);
        assert!(completion.result().is_some());
        assert!(completion.error().is_none());
        assert!(scheduler.accept_completion(&completion, &session).unwrap());
        scheduler.shutdown().unwrap();
    }

    #[test]
    fn complete_scheduler_decodes_once_per_source_miss() {
        let mut session = modeled_document_session();
        let decode_observer = Arc::new(AtomicUsize::new(0));
        let scheduler = EvaluationScheduler::new_with_test_decode_observer(
            EvaluationLimits::default(),
            Arc::clone(&decode_observer),
        )
        .unwrap();

        let first_ticket = scheduler
            .submit(document_request(&session, valid_document_bytes()))
            .unwrap();
        let first = wait_for_document_completion(&scheduler);
        assert_eq!(first.ticket(), first_ticket);
        assert_eq!(first.result().unwrap().channels().len(), 3);
        assert!(scheduler.accept_completion(&first, &session).unwrap());
        assert_eq!(decode_observer.load(Ordering::Relaxed), 1);

        let repeated_ticket = scheduler
            .submit(document_request(&session, valid_document_bytes()))
            .unwrap();
        let repeated = wait_for_document_completion(&scheduler);
        assert_eq!(repeated.ticket(), repeated_ticket);
        assert!(scheduler.accept_completion(&repeated, &session).unwrap());
        assert_eq!(decode_observer.load(Ordering::Relaxed), 1);

        session
            .apply(&DocumentCommand::SetSourceReference {
                source: SourceReference::Assigned(
                    SourceReferenceId::new("changed-cancellation-test-source").unwrap(),
                ),
            })
            .unwrap();
        let changed_ticket = scheduler
            .submit(document_request_for_source(
                &session,
                "changed-cancellation-test-source",
                valid_document_bytes(),
            ))
            .unwrap();
        let changed = wait_for_document_completion(&scheduler);
        assert_eq!(changed.ticket(), changed_ticket);
        assert!(scheduler.accept_completion(&changed, &session).unwrap());
        assert_eq!(decode_observer.load(Ordering::Relaxed), 2);
        scheduler.shutdown().unwrap();
    }

    #[test]
    fn complete_scheduler_cancels_at_boundary_and_between_channel_checkpoints() {
        for (stage, checkpoint) in [
            (EvaluationStage::Decode, EvaluationCheckpoint::Before),
            (EvaluationStage::Decode, EvaluationCheckpoint::After),
            (EvaluationStage::Family, EvaluationCheckpoint::Before),
            (EvaluationStage::Family, EvaluationCheckpoint::After),
            (EvaluationStage::Realization, EvaluationCheckpoint::Before),
            (EvaluationStage::Realization, EvaluationCheckpoint::After),
            (EvaluationStage::Channel, EvaluationCheckpoint::Before),
            (EvaluationStage::Channel, EvaluationCheckpoint::After),
        ] {
            let (gate, entered) = EvaluationStageGate::new(stage, checkpoint);
            let scheduler = EvaluationScheduler::new_with_test_gate(
                EvaluationLimits::default(),
                Arc::clone(&gate),
            )
            .unwrap();
            let session = modeled_document_session();
            let stale_ticket = scheduler
                .submit(document_request(&session, valid_document_bytes()))
                .unwrap();
            entered.recv_timeout(GUARD).unwrap();
            let newest_ticket = scheduler
                .submit(document_request(&session, valid_document_bytes()))
                .unwrap();
            gate.release();

            let completion = wait_for_document_completion(&scheduler);
            assert_ne!(stale_ticket, newest_ticket, "{stage:?} {checkpoint:?}");
            assert_eq!(
                completion.ticket(),
                newest_ticket,
                "{stage:?} {checkpoint:?}"
            );
            assert!(completion.result().is_some(), "{stage:?} {checkpoint:?}");
            assert!(scheduler.accept_completion(&completion, &session).unwrap());
            scheduler.shutdown().unwrap();
        }
    }

    /// Proves a superseded authored-shape realization publishes neither a completion nor cache
    /// transaction, while its newest replacement becomes reusable only after explicit acceptance.
    #[test]
    fn authored_shape_scheduler_cancellation_does_not_publish_partial_cache_state() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Realization, EvaluationCheckpoint::After);
        let scheduler =
            EvaluationScheduler::new_with_test_gate(EvaluationLimits::default(), Arc::clone(&gate))
                .unwrap();
        let session = modeled_shape_session();
        let bytes = valid_document_bytes();
        let stale_ticket = scheduler
            .submit(document_request(&session, Arc::clone(&bytes)))
            .unwrap();
        entered.recv_timeout(GUARD).unwrap();
        let newest_ticket = scheduler
            .submit(document_request(&session, Arc::clone(&bytes)))
            .unwrap();
        gate.release();

        let newest = wait_for_document_completion(&scheduler);
        assert_ne!(stale_ticket, newest_ticket);
        assert_eq!(newest.ticket(), newest_ticket);
        assert_eq!(
            newest.cache_diagnostics().unwrap().aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Miss,
                family: CacheDisposition::Miss,
                realization: CacheDisposition::Miss,
                scene: CacheDisposition::Miss,
                raster: CacheDisposition::Miss,
            },
            "cancelled work must not become a cache authority"
        );
        assert!(scheduler.accept_completion(&newest, &session).unwrap());

        let repeated_ticket = scheduler.submit(document_request(&session, bytes)).unwrap();
        let repeated = wait_for_document_completion(&scheduler);
        assert_eq!(repeated.ticket(), repeated_ticket);
        assert_eq!(
            repeated.cache_diagnostics().unwrap().aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Hit,
                family: CacheDisposition::Hit,
                realization: CacheDisposition::Hit,
                scene: CacheDisposition::Hit,
                raster: CacheDisposition::Hit,
            }
        );
        assert!(scheduler.accept_completion(&repeated, &session).unwrap());
        scheduler.shutdown().unwrap();
    }

    /// Proves a cancelled ordinary-region job publishes neither its candidate completion nor any
    /// decoded-source, family, Region realization, scene, raster, or caller-visible SVG cache
    /// authority before the newest job is explicitly accepted.
    ///
    /// SVG is serialized only by a caller from the completed immutable scene; the scheduler owns
    /// no SVG cache. This witness proves that the sole scene from which SVG can be serialized is
    /// the newest Region scene and that the cancelled candidate never reaches that boundary.
    #[test]
    fn voronoi_region_scheduler_cancellation_is_atomic_before_cache_publication() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Realization, EvaluationCheckpoint::After);
        let scheduler =
            EvaluationScheduler::new_with_test_gate(EvaluationLimits::default(), Arc::clone(&gate))
                .expect("Region scheduler starts");
        let session = modeled_voronoi_session();
        let bytes = valid_document_bytes();
        let stale_ticket = scheduler
            .submit(document_request(&session, Arc::clone(&bytes)))
            .expect("stale Region job submits");
        entered
            .recv_timeout(GUARD)
            .expect("stale Region job reaches realization gate");
        let newest_ticket = scheduler
            .submit(document_request(&session, Arc::clone(&bytes)))
            .expect("newest Region job submits");
        gate.release();

        let newest = wait_for_document_completion(&scheduler);
        assert_ne!(stale_ticket, newest_ticket);
        assert_eq!(newest.ticket(), newest_ticket);
        assert!(
            newest.result().is_some(),
            "newest Region job must complete: {:?}",
            newest.error()
        );
        assert_eq!(
            newest
                .cache_diagnostics()
                .expect("newest Region diagnostics")
                .aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Miss,
                family: CacheDisposition::Miss,
                realization: CacheDisposition::Miss,
                scene: CacheDisposition::Miss,
                raster: CacheDisposition::Miss,
            },
            "the cancelled candidate cannot commit any derived cache unit"
        );
        let result = newest.result().expect("newest Region result");
        assert!(result.scene().layers().iter().all(|layer| {
            matches!(
                layer.outputs().first().map(toniator_render::RenderOutputLayer::geometry),
                Some(GeometryOutput::CanonicalRegions(regions)) if !regions.regions().is_empty()
            )
        }));
        assert_eq!(result.raster().width(), 64);
        assert_eq!(result.raster().height(), 48);
        assert!(
            write_svg(result.scene()).contains("<path"),
            "the accepted Region scene remains the only scheduler result a caller can serialize"
        );
        assert!(
            scheduler
                .accept_completion(&newest, &session)
                .expect("newest Region completion accepts")
        );

        let repeated_ticket = scheduler
            .submit(document_request(&session, bytes))
            .expect("accepted Region repeat submits");
        let repeated = wait_for_document_completion(&scheduler);
        assert_eq!(repeated.ticket(), repeated_ticket);
        assert_eq!(
            repeated
                .cache_diagnostics()
                .expect("accepted Region repeat diagnostics")
                .aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Hit,
                family: CacheDisposition::Hit,
                realization: CacheDisposition::Hit,
                scene: CacheDisposition::Hit,
                raster: CacheDisposition::Hit,
            }
        );
        assert!(
            scheduler
                .accept_completion(&repeated, &session)
                .expect("accepted Region repeat publishes")
        );
        scheduler.shutdown().expect("Region scheduler shuts down");
    }

    /// Proves source-sampled Scale Region cache hits replay complete producer, sampling, and treatment facts.
    #[test]
    fn scaled_voronoi_region_cache_replays_complete_output_diagnostics() {
        let scheduler =
            EvaluationScheduler::new_with_limits(EvaluationLimits::default()).expect("scheduler");
        let mut session = modeled_scaled_voronoi_session();
        let bytes = valid_document_bytes();
        let first_ticket = scheduler
            .submit(document_request(&session, Arc::clone(&bytes)))
            .expect("scaled Region submit");
        let first = wait_for_document_completion(&scheduler);
        assert_eq!(first.ticket(), first_ticket);
        let first_diagnostics = first
            .cache_diagnostics()
            .unwrap_or_else(|| panic!("scaled Region evaluation fails: {:?}", first.error()));
        let first_output = first_diagnostics.channels[0].outputs[0].clone();
        assert_eq!(first_output.realization, CacheDisposition::Miss);
        let region = first_output.region.as_ref().expect("Region cache facts");
        assert!(region.source_identity.is_some());
        assert_eq!(
            region.sampling.strategy,
            toniator_domain::RegionSamplingStrategy::ReferencePoint
        );
        assert!(region.sampling.sampled_bases > 0);
        assert_eq!(region.treatment.kind, RegionTreatmentCacheKind::Scale);
        assert!(region.treatment.retained_regions > 0);
        assert!(matches!(
            region.producer,
            RegionProducerCacheDiagnostics::Voronoi(_)
        ));
        assert!(scheduler.accept_completion(&first, &session).unwrap());

        let replay_ticket = scheduler
            .submit(document_request(&session, Arc::clone(&bytes)))
            .expect("scaled Region replay submits");
        let replay = wait_for_document_completion(&scheduler);
        assert_eq!(replay.ticket(), replay_ticket);
        let replay_output = replay
            .cache_diagnostics()
            .expect("scaled Region replay diagnostics")
            .channels[0]
            .outputs[0]
            .clone();
        assert_eq!(replay_output.realization, CacheDisposition::Hit);
        assert_eq!(replay_output.region, first_output.region);
        assert_eq!(replay_output.voronoi, first_output.voronoi);
        assert!(scheduler.accept_completion(&replay, &session).unwrap());

        session
            .apply(&DocumentCommand::SetModeledMappingField {
                channel_id: ChannelId(1),
                edit: toniator_domain::ModeledMappingFieldEdit::Gain(0.75),
            })
            .expect("mapping edit validates");
        let remapped_ticket = scheduler
            .submit(document_request(&session, bytes))
            .expect("mapping-sensitive Region submit");
        let remapped = wait_for_document_completion(&scheduler);
        assert_eq!(remapped.ticket(), remapped_ticket);
        let remapped_diagnostics = remapped.cache_diagnostics().expect("mapping diagnostics");
        assert_eq!(
            remapped_diagnostics.aggregate.decoded_source,
            CacheDisposition::Hit
        );
        assert_eq!(
            remapped_diagnostics.channels[0].family,
            CacheDisposition::Hit
        );
        assert_eq!(
            remapped_diagnostics.channels[0].outputs[0].realization,
            CacheDisposition::Miss,
            "sampled Region mapping changes must invalidate only output realization"
        );
        scheduler.shutdown().expect("scheduler shutdown");
    }

    /// Proves sampling-strategy and relevant sampling-limit changes miss only the Region output cache.
    #[test]
    fn region_cache_key_tracks_sampling_strategy_and_sampling_limits() {
        let reference_session = modeled_scaled_voronoi_session();
        let average_session = modeled_scaled_voronoi_session_with_sampling(
            toniator_domain::RegionSamplingStrategy::AreaAverage,
        );
        let bytes = valid_document_bytes();
        let mut cache = DocumentDerivedCache::default();
        let baseline = evaluate_cached_document(
            document_request(&reference_session, Arc::clone(&bytes)),
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("reference Region evaluates");
        assert_eq!(
            baseline.diagnostics.aggregate.realization,
            CacheDisposition::Miss
        );
        cache.commit(baseline.transaction);

        let changed_strategy = evaluate_cached_document(
            document_request(&average_session, Arc::clone(&bytes)),
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("area-average Region evaluates");
        assert_eq!(
            changed_strategy.diagnostics.aggregate.decoded_source,
            CacheDisposition::Hit
        );
        assert_eq!(
            changed_strategy.diagnostics.aggregate.family,
            CacheDisposition::Hit
        );
        assert_eq!(
            changed_strategy.diagnostics.aggregate.realization,
            CacheDisposition::Miss,
            "sampling strategy is a per-output cache input"
        );
        cache.commit(changed_strategy.transaction);

        let default_sampling = RegionSamplingLimits::default();
        let changed_limits = EvaluationLimits::default()
            .with_region_sampling_limits(RegionSamplingLimits {
                max_cell_intersections: default_sampling.max_cell_intersections - 1,
                ..default_sampling
            })
            .expect("nonzero changed sampling policy");
        let changed_policy = evaluate_cached_document(
            document_request(&reference_session, bytes),
            changed_limits,
            &cache,
            &NeverCancelled,
        )
        .expect("changed-policy Region evaluates");
        assert_eq!(
            changed_policy.diagnostics.aggregate.decoded_source,
            CacheDisposition::Hit
        );
        assert_eq!(
            changed_policy.diagnostics.aggregate.family,
            CacheDisposition::Hit
        );
        assert_eq!(
            changed_policy.diagnostics.aggregate.realization,
            CacheDisposition::Miss,
            "relevant sampling policy is a cache input but not a family input"
        );
    }

    /// Proves normalized Region sampling-limit changes rekey realization work without rebuilding
    /// the source-independent structural family.
    #[test]
    fn normalized_solid_region_cache_rekeys_sampling_limits_without_refamily() {
        let session = modeled_voronoi_session();
        let bytes = valid_document_bytes();
        let mut cache = DocumentDerivedCache::default();
        let baseline = evaluate_cached_document(
            document_request(&session, Arc::clone(&bytes)),
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("normalized solid Region evaluates");
        let baseline_fingerprint = baseline.result.channels()[0]
            .realization_identity()
            .to_owned();
        cache.commit(baseline.transaction);

        let default_sampling = RegionSamplingLimits::default();
        let irrelevant_policy = EvaluationLimits::default()
            .with_region_sampling_limits(RegionSamplingLimits {
                max_flattened_segments: default_sampling.max_flattened_segments - 1,
                ..default_sampling
            })
            .expect("nonzero normalized sampling policy");
        let replay = evaluate_cached_document(
            document_request(&session, bytes),
            irrelevant_policy,
            &cache,
            &NeverCancelled,
        )
        .expect("normalized solid Region replay evaluates");
        assert_eq!(
            replay.diagnostics.aggregate.family,
            CacheDisposition::Hit,
            "sampling limits affect Region realization, not structural family generation"
        );
        assert_eq!(
            replay.diagnostics.aggregate.realization,
            CacheDisposition::Miss,
            "normalized Region fill always samples untreated positive bases"
        );
        assert_eq!(
            replay.diagnostics.aggregate.scene,
            CacheDisposition::Hit,
            "unchanged canonical Region identity reuses scene assembly after resampling"
        );
        assert_eq!(
            replay.diagnostics.aggregate.raster,
            CacheDisposition::Hit,
            "unchanged canonical Region identity reuses raster assembly after resampling"
        );
        assert_eq!(
            replay.result.channels()[0].realization_identity(),
            baseline_fingerprint,
            "sampling work policy rekeys the cache without changing canonical Region identity"
        );
    }

    /// Proves the engine carries sampled per-region paint through SourceColorAlpha scene and SVG assembly.
    #[test]
    fn sampled_voronoi_regions_build_source_colored_scene_and_svg() {
        let session = modeled_sampled_voronoi_session();
        let evaluated = evaluate_with_limits(
            document_request(&session, valid_document_bytes()),
            EvaluationLimits::default(),
        )
        .expect("sampled Region document evaluates");
        let layer = &evaluated.scene().layers()[0];
        let output = &layer.outputs()[0];
        let GeometryOutput::CanonicalRegions(regions) = output.geometry() else {
            panic!("sampled output retains canonical regions")
        };
        let paints = output
            .primitive_paints
            .as_ref()
            .expect("sampled Region output carries per-region paint");
        assert_eq!(paints.len(), regions.regions().len());
        assert!(paints.iter().all(|paint| paint.alpha > 0.0));
        let svg = write_svg(evaluated.scene());
        assert!(svg.contains(&format!("channel-{}-region-", layer.channel_id().0)));
        assert!(svg.contains("fill-rule=\"nonzero\""));
        assert!(
            evaluated
                .raster()
                .pixels()
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0),
            "sampled Region raster retains visible source-alpha coverage"
        );
    }

    /// Proves test-only Region evidence observes one normal realization without changing products.
    #[test]
    fn region_evaluation_evidence_is_observational_and_atomic() {
        let session = modeled_sampled_voronoi_session();
        let bytes = valid_document_bytes();
        let baseline = evaluate_cached_document(
            document_request(&session, Arc::clone(&bytes)),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        )
        .expect("baseline sampled Region evaluation succeeds");
        let (observed, evidence) = evaluate_cached_document_with_region_evidence(
            document_request(&session, Arc::clone(&bytes)),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        );
        let observed = observed.expect("observed sampled Region evaluation succeeds");
        let evidence = evidence.expect("successful Region evaluation records evidence");
        assert_eq!(observed.result, baseline.result);
        assert_eq!(observed.diagnostics, baseline.diagnostics);
        assert!(
            observed.result.channels()[0]
                .realization_identity()
                .contains(&evidence.realization_fingerprint),
            "the final typed fingerprint is retained inside the engine contract fingerprint"
        );
        assert_eq!(
            evidence.untreated_region_ids.len(),
            evidence.references.len()
        );
        assert_eq!(evidence.untreated_region_ids.len(), evidence.samples.len());
        assert_eq!(evidence.treatments.len(), evidence.samples.len());
        assert_eq!(
            evidence.provenance.len(),
            evidence.diagnostics.retained_regions
        );

        let cancelled = AtomicBool::new(true);
        let (cancelled_outcome, cancelled_evidence) = evaluate_cached_document_with_region_evidence(
            document_request(&session, Arc::clone(&bytes)),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &AtomicCancellation(&cancelled),
        );
        assert!(matches!(
            cancelled_outcome,
            Err(EvaluationRunError::Cancelled)
        ));
        assert!(cancelled_evidence.is_none());

        let constrained = EvaluationLimits::default()
            .with_region_sampling_limits(RegionSamplingLimits {
                max_cell_intersections: 1,
                ..RegionSamplingLimits::default()
            })
            .expect("nonzero constrained sampling policy");
        let area_average = modeled_scaled_voronoi_session_with_sampling(
            toniator_domain::RegionSamplingStrategy::AreaAverage,
        );
        let (failure, failed_evidence) = evaluate_cached_document_with_region_evidence(
            document_request(&area_average, bytes),
            constrained,
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        );
        assert!(failure.is_err());
        assert!(failed_evidence.is_none());
    }

    /// Proves a late AreaAverage limit failure returns no cache transaction or partial scene products.
    #[test]
    fn area_average_region_failure_is_atomic_before_cache_publication() {
        let session = modeled_scaled_voronoi_session_with_sampling(
            toniator_domain::RegionSamplingStrategy::AreaAverage,
        );
        let bytes = valid_document_bytes();
        let cache = DocumentDerivedCache::default();
        let default_sampling = RegionSamplingLimits::default();
        let failing_limits = EvaluationLimits::default()
            .with_region_sampling_limits(RegionSamplingLimits {
                max_cell_intersections: 1,
                ..default_sampling
            })
            .expect("nonzero constrained policy");
        let failure = evaluate_cached_document(
            document_request(&session, Arc::clone(&bytes)),
            failing_limits,
            &cache,
            &NeverCancelled,
        );
        let Err(failure) = failure else {
            panic!("late AreaAverage budget must fail")
        };
        let EvaluationRunError::Evaluation(error) = failure else {
            panic!("AreaAverage limit is an evaluation failure")
        };
        assert_eq!(
            error.path(),
            "sampling.region_average.limits.cell_intersections"
        );
        assert!(cache.decoded_source.is_none());
        assert!(cache.families.is_empty());
        assert!(cache.realizations.is_empty());
        assert!(cache.scene.is_none());
        assert!(cache.raster.is_none());

        let succeeding = evaluate_cached_document(
            document_request(&session, bytes),
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("unconstrained AreaAverage evaluates after failure");
        assert_eq!(
            succeeding.diagnostics.aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Miss,
                family: CacheDisposition::Miss,
                realization: CacheDisposition::Miss,
                scene: CacheDisposition::Miss,
                raster: CacheDisposition::Miss,
            },
            "a failed Region run publishes neither source nor downstream candidate state"
        );
    }

    /// Proves an already-completed ordinary-region candidate becomes stale when the authoritative
    /// document token changes, so rejecting it leaves every cache unit unpublished for the next
    /// current completion.
    #[test]
    fn voronoi_region_stale_completion_cannot_commit_candidate_cache() {
        let scheduler =
            EvaluationScheduler::new_with_limits(EvaluationLimits::default()).expect("scheduler");
        let mut history = DocumentHistory::new(modeled_voronoi_session());
        let bytes = valid_document_bytes();
        let stale_ticket = scheduler
            .submit(document_request(history.session(), Arc::clone(&bytes)))
            .expect("Region candidate submits");
        let stale = wait_for_document_completion(&scheduler);
        assert_eq!(stale.ticket(), stale_ticket);
        assert!(
            stale.result().is_some(),
            "Region candidate completes privately"
        );

        let base = history.document().pattern_settings().clone();
        let mut replacement = base.clone();
        replacement.pattern_rotation_degrees = 13.0;
        history
            .apply(&DocumentCommand::SetDocumentPatternSettings {
                base,
                settings: replacement,
            })
            .expect("authoritative Region document edit applies");
        assert!(
            !scheduler
                .accept_completion(&stale, history.session())
                .expect("stale Region candidate rejects"),
            "a stale Region completion cannot publish pending cache units"
        );

        let current_ticket = scheduler
            .submit(document_request(history.session(), bytes))
            .expect("current Region job submits");
        let current = wait_for_document_completion(&scheduler);
        assert_eq!(current.ticket(), current_ticket);
        assert_eq!(
            current
                .cache_diagnostics()
                .expect("current Region diagnostics")
                .aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Miss,
                family: CacheDisposition::Miss,
                realization: CacheDisposition::Miss,
                scene: CacheDisposition::Miss,
                raster: CacheDisposition::Miss,
            },
            "rejecting the stale completion leaves no candidate, scene, raster, or Region cache state"
        );
        assert!(
            current.result().is_some(),
            "current Region completion evaluates"
        );
        assert!(
            scheduler
                .accept_completion(&current, history.session())
                .expect("current Region completion accepts")
        );
        scheduler.shutdown().expect("Region scheduler shuts down");
    }

    /// Proves a Connected-only path edit reuses family work while rebuilding downstream cache products.
    #[test]
    fn connected_path_cache_reuses_family_and_rebuilds_downstream_products() {
        let scheduler = EvaluationScheduler::new_with_limits(EvaluationLimits::default()).unwrap();
        let mut session = modeled_path_session();
        let bytes = flat_document_bytes();
        let first_ticket = scheduler
            .submit(document_request(&session, Arc::clone(&bytes)))
            .unwrap();
        let first = wait_for_document_completion(&scheduler);
        assert_eq!(first.ticket(), first_ticket);
        assert!(scheduler.accept_completion(&first, &session).unwrap());
        let command = session
            .document()
            .set_channel_output_response_for_effective(
                ChannelId(1),
                PatternOutputLayerId(73),
                PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                    minimum_thickness: 0.015,
                    maximum_thickness: 0.025,
                    bias: 0.0,
                }),
            )
            .unwrap();
        session.apply(&command).unwrap();
        let edited = scheduler
            .submit(document_request(&session, Arc::clone(&bytes)))
            .unwrap();
        let completion = wait_for_document_completion(&scheduler);
        assert_eq!(completion.ticket(), edited);
        let diagnostics = completion.cache_diagnostics().unwrap();
        assert_eq!(diagnostics.aggregate.family, CacheDisposition::Hit);
        assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Miss);
        assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Miss);
        assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Miss);
        assert!(scheduler.accept_completion(&completion, &session).unwrap());
        scheduler.shutdown().unwrap();
    }

    /// Evaluates a typed maze through the cached document pipeline and publishes only canonical walls.
    #[test]
    fn maze_document_evaluates_and_reuses_its_family_and_realization() {
        let scheduler = EvaluationScheduler::new_with_limits(EvaluationLimits::default()).unwrap();
        let session = modeled_maze_session(23);
        let bytes = valid_document_bytes();
        let first_ticket = scheduler
            .submit(document_request(&session, Arc::clone(&bytes)))
            .unwrap();
        let first = wait_for_document_completion(&scheduler);
        assert_eq!(first.ticket(), first_ticket);
        assert!(
            first.result().is_some(),
            "maze evaluation failed: {:?}",
            first.error()
        );
        assert_eq!(
            first
                .cache_diagnostics()
                .expect("maze diagnostics")
                .aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Miss,
                family: CacheDisposition::Miss,
                realization: CacheDisposition::Miss,
                scene: CacheDisposition::Miss,
                raster: CacheDisposition::Miss,
            }
        );
        let result = first.result().expect("maze document evaluates");
        assert!(
            connection_strokes(result)
                .iter()
                .flatten()
                .all(|stroke| matches!(
                    stroke.source_id,
                    toniator_patterns::CanonicalStrokeSourceId::Maze(_)
                ))
        );
        assert!(scheduler.accept_completion(&first, &session).unwrap());
        let second_ticket = scheduler.submit(document_request(&session, bytes)).unwrap();
        let second = wait_for_document_completion(&scheduler);
        assert_eq!(second.ticket(), second_ticket);
        assert_eq!(
            second
                .cache_diagnostics()
                .expect("maze cache diagnostics")
                .aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Hit,
                family: CacheDisposition::Hit,
                realization: CacheDisposition::Hit,
                scene: CacheDisposition::Hit,
                raster: CacheDisposition::Hit,
            }
        );
        assert!(scheduler.accept_completion(&second, &session).unwrap());
        scheduler.shutdown().unwrap();
    }

    /// Proves a maze seed changes only maze realization topology while preserving the accepted
    /// family cache identity and the exact evaluated site positions used by that family.
    #[test]
    fn maze_document_seed_change_reuses_family_and_rebuilds_canonical_walls() {
        let scheduler = EvaluationScheduler::new_with_limits(EvaluationLimits::default()).unwrap();
        let mut history = DocumentHistory::new(modeled_maze_session(23));
        let bytes = valid_document_bytes();
        let first_ticket = scheduler
            .submit(document_request(history.session(), Arc::clone(&bytes)))
            .expect("first maze job submits");
        let first = wait_for_document_completion(&scheduler);
        assert_eq!(first.ticket(), first_ticket);
        assert!(
            scheduler
                .accept_completion(&first, history.session())
                .unwrap(),
            "first maze completion publishes its accepted family"
        );
        let first_result = first.result().expect("first maze result");
        let first_family = first_result
            .channels()
            .iter()
            .map(|channel| channel.family_identity().to_owned())
            .collect::<Vec<_>>();
        let first_walls = connection_strokes(first_result);

        replace_maze_seed(&mut history, 29);
        let seeded_ticket = scheduler
            .submit(document_request(history.session(), Arc::clone(&bytes)))
            .expect("seeded maze job submits");
        let seeded = wait_for_document_completion(&scheduler);
        assert_eq!(seeded.ticket(), seeded_ticket);
        let diagnostics = seeded
            .cache_diagnostics()
            .expect("seeded maze diagnostics")
            .aggregate;
        assert_eq!(diagnostics.decoded_source, CacheDisposition::Hit);
        assert_eq!(diagnostics.family, CacheDisposition::Hit);
        assert_eq!(diagnostics.realization, CacheDisposition::Miss);
        assert_eq!(diagnostics.scene, CacheDisposition::Miss);
        assert_eq!(diagnostics.raster, CacheDisposition::Miss);
        let seeded_result = seeded.result().expect("seeded maze result");
        assert_eq!(
            seeded_result
                .channels()
                .iter()
                .map(|channel| channel.family_identity().to_owned())
                .collect::<Vec<_>>(),
            first_family,
            "maze seed is not a site-family input"
        );
        assert_ne!(
            connection_strokes(seeded_result),
            first_walls,
            "different recursive-backtracker seeds select a different retained-wall topology"
        );
        assert!(
            scheduler
                .accept_completion(&seeded, history.session())
                .unwrap()
        );
        scheduler.shutdown().unwrap();
    }

    /// Proves the accepted cached family preserves exact site IDs and positions across a maze seed
    /// edit, while the geometry-owned recursive-backtracker realization remains seed-specific.
    #[test]
    fn maze_seed_change_keeps_cached_family_sites_exact() {
        let mut accepted = DocumentDerivedCache::default();
        let mut history = DocumentHistory::new(modeled_maze_session(23));
        let bytes = valid_document_bytes();
        let first = evaluate_cached_document(
            document_request(history.session(), Arc::clone(&bytes)),
            EvaluationLimits::default(),
            &accepted,
            &NeverCancelled,
        )
        .expect("first maze candidate evaluates");
        let first_sites = first.transaction.families[0]
            .1
            .site_set()
            .iter()
            .map(|site| (site.id, site.position))
            .collect::<Vec<_>>();
        let first_walls = connection_strokes(&first.result);
        accepted.commit(first.transaction);

        replace_maze_seed(&mut history, 29);
        let seeded = evaluate_cached_document(
            document_request(history.session(), bytes),
            EvaluationLimits::default(),
            &accepted,
            &NeverCancelled,
        )
        .expect("seeded maze candidate evaluates");
        assert_eq!(seeded.diagnostics.aggregate.family, CacheDisposition::Hit);
        assert_eq!(
            accepted.families[0]
                .1
                .site_set()
                .iter()
                .map(|site| (site.id, site.position))
                .collect::<Vec<_>>(),
            first_sites,
            "the cached typed FamilySiteSet is the sole site authority for both seeds"
        );
        assert_ne!(
            connection_strokes(&seeded.result),
            first_walls,
            "maze seed changes retained walls without moving family sites"
        );
    }

    /// Proves a cancelled stale maze realization cannot publish a cache transaction before the
    /// newest seed completion is explicitly accepted by the scheduler.
    #[test]
    fn maze_scheduler_stale_realization_cannot_publish_before_acceptance() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Realization, EvaluationCheckpoint::After);
        let scheduler =
            EvaluationScheduler::new_with_test_gate(EvaluationLimits::default(), Arc::clone(&gate))
                .expect("maze scheduler starts");
        let mut history = DocumentHistory::new(modeled_maze_session(23));
        let bytes = valid_document_bytes();
        let stale_ticket = scheduler
            .submit(document_request(history.session(), Arc::clone(&bytes)))
            .expect("stale maze job submits");
        entered
            .recv_timeout(GUARD)
            .expect("stale maze reaches realization gate");
        replace_maze_seed(&mut history, 29);
        let newest_ticket = scheduler
            .submit(document_request(history.session(), Arc::clone(&bytes)))
            .expect("newest maze job submits");
        gate.release();
        let newest = wait_for_document_completion(&scheduler);
        assert_ne!(stale_ticket, newest_ticket);
        assert_eq!(newest.ticket(), newest_ticket);
        assert_eq!(
            newest
                .cache_diagnostics()
                .expect("newest maze diagnostics")
                .aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Miss,
                family: CacheDisposition::Miss,
                realization: CacheDisposition::Miss,
                scene: CacheDisposition::Miss,
                raster: CacheDisposition::Miss,
            },
            "cancelled stale maze work cannot publish any cache authority"
        );
        assert!(
            scheduler
                .accept_completion(&newest, history.session())
                .unwrap()
        );
        let repeated_ticket = scheduler
            .submit(document_request(history.session(), bytes))
            .expect("accepted maze repeat submits");
        let repeated = wait_for_document_completion(&scheduler);
        assert_eq!(repeated.ticket(), repeated_ticket);
        assert_eq!(
            repeated
                .cache_diagnostics()
                .expect("accepted maze repeat diagnostics")
                .aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Hit,
                family: CacheDisposition::Hit,
                realization: CacheDisposition::Hit,
                scene: CacheDisposition::Hit,
                raster: CacheDisposition::Hit,
            }
        );
        assert!(
            scheduler
                .accept_completion(&repeated, history.session())
                .unwrap()
        );
        scheduler.shutdown().expect("maze scheduler shuts down");
    }

    /// Proves full cached document evaluation keeps the immutable triangular-site family while
    /// seed and program intent rebuild only connection realization, scene, and raster products.
    #[test]
    fn connection_document_cache_reuses_family_for_seed_and_program_changes() {
        let scheduler = EvaluationScheduler::new_with_limits(EvaluationLimits::default()).unwrap();
        let mut history = DocumentHistory::new(modeled_connection_session(
            random_connection_program(2, 24.0),
        ));
        let bytes = valid_document_bytes();
        let submit =
            |scheduler: &EvaluationScheduler, session: &DocumentSession, bytes: Arc<[u8]>| {
                let ticket = scheduler.submit(document_request(session, bytes)).unwrap();
                let completion = wait_for_document_completion(scheduler);
                assert_eq!(completion.ticket(), ticket);
                assert!(
                    completion.result().is_some(),
                    "connection evaluation failed: {:?}",
                    completion.error()
                );
                assert!(scheduler.accept_completion(&completion, session).unwrap());
                completion
            };
        let first = submit(&scheduler, history.session(), Arc::clone(&bytes));
        assert_eq!(
            first.cache_diagnostics().unwrap().aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Miss,
                family: CacheDisposition::Miss,
                realization: CacheDisposition::Miss,
                scene: CacheDisposition::Miss,
                raster: CacheDisposition::Miss,
            }
        );
        let first_result = first.result().unwrap();
        let first_families = first_result
            .channels()
            .iter()
            .map(|channel| channel.family_identity().to_owned())
            .collect::<Vec<_>>();
        let first_strokes = connection_strokes(first_result);

        let repeated = submit(&scheduler, history.session(), Arc::clone(&bytes));
        assert_eq!(
            repeated.cache_diagnostics().unwrap().aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Hit,
                family: CacheDisposition::Hit,
                realization: CacheDisposition::Hit,
                scene: CacheDisposition::Hit,
                raster: CacheDisposition::Hit,
            }
        );

        replace_connection_program(&mut history, random_connection_program(3, 24.0));
        let seeded = submit(&scheduler, history.session(), Arc::clone(&bytes));
        let seeded_diagnostics = seeded.cache_diagnostics().unwrap().aggregate;
        assert_eq!(seeded_diagnostics.decoded_source, CacheDisposition::Hit);
        assert_eq!(seeded_diagnostics.family, CacheDisposition::Hit);
        assert_eq!(seeded_diagnostics.realization, CacheDisposition::Miss);
        assert_eq!(seeded_diagnostics.scene, CacheDisposition::Miss);
        assert_eq!(seeded_diagnostics.raster, CacheDisposition::Miss);
        let seeded_result = seeded.result().unwrap();
        assert_eq!(
            seeded_result
                .channels()
                .iter()
                .map(|channel| channel.family_identity().to_owned())
                .collect::<Vec<_>>(),
            first_families,
            "seed is selected-edge intent, not a site-family input"
        );
        assert_ne!(
            connection_strokes(seeded_result),
            first_strokes,
            "different random seeds must expose a distinct canonical selected topology"
        );

        replace_connection_program(
            &mut history,
            ConnectionProgram::NearestLinks {
                adjacency: ConnectionAdjacencyIntent {
                    maximum_degree: 6,
                    maximum_distance: 24.0,
                },
            },
        );
        let typed = submit(&scheduler, history.session(), bytes);
        let typed_diagnostics = typed.cache_diagnostics().unwrap().aggregate;
        assert_eq!(typed_diagnostics.decoded_source, CacheDisposition::Hit);
        assert_eq!(typed_diagnostics.family, CacheDisposition::Hit);
        assert_eq!(typed_diagnostics.realization, CacheDisposition::Miss);
        assert_eq!(typed_diagnostics.scene, CacheDisposition::Miss);
        assert_eq!(typed_diagnostics.raster, CacheDisposition::Miss);
        assert_eq!(
            typed
                .result()
                .unwrap()
                .channels()
                .iter()
                .map(|channel| channel.family_identity().to_owned())
                .collect::<Vec<_>>(),
            first_families,
            "program kind changes do not move family sites"
        );
        scheduler.shutdown().unwrap();
    }

    /// Proves document evaluation derives graph nodes and canonical connection centerlines only
    /// from the exact cached `FamilySiteSet`, without a renderer-side substitute site set.
    #[test]
    fn connection_document_consumes_cached_family_sites_by_id_and_position() {
        let session = modeled_connection_session(random_connection_program(2, 24.0));
        let request = document_request(&session, valid_document_bytes());
        let candidate = evaluate_cached_document(
            request,
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        )
        .expect("connection document evaluates before cache publication");
        let family = &candidate.transaction.families[0].1;
        let document = session.document();
        let definition = &document.pattern_definition_bundles()[0].definition;
        let effective = document
            .effective_channel_pattern(ChannelId(1))
            .expect("modeled red channel resolves its shared connection definition");
        let plan = toniator_patterns::resolve_document_pattern_pipeline(document, definition)
            .expect("connection definition resolves a typed pipeline");
        let [
            PatternOutputLayer {
                realization: PatternOutputRealization::ConnectionPaths { program, .. },
                ..
            },
        ] = definition.output_layers.as_slice()
        else {
            panic!("fixture retains exactly one connection output")
        };
        let graph = build_typed_site_adjacency_cancellable(
            family,
            required_connection_base_support(
                document.canvas(),
                &effective,
                definition,
                &plan.family,
            )
            .expect("connection base support is finite"),
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: program.adjacency().maximum_degree as usize,
                maximum_distance: program.adjacency().maximum_distance,
            },
            EvaluationLimits::default().site_adjacency_limits(),
            &|| false,
        )
        .expect("cached family derives its requested connection graph");
        assert_eq!(graph.nodes().len(), family.site_set().len());
        for (node, site) in graph.nodes().iter().zip(family.site_set().iter()) {
            assert_eq!(node.id, site.id);
            assert_eq!(node.position, site.position);
        }
        let positions = family
            .site_set()
            .iter()
            .map(|site| site.position)
            .collect::<Vec<_>>();
        for (_, realization) in &candidate.transaction.realizations {
            let GeometryOutput::CanonicalStrokes(strokes) =
                realization.realization.geometry().as_ref()
            else {
                panic!("connection fixture cannot realize marks")
            };
            for stroke in strokes {
                for segment in stroke.path.segments() {
                    assert!(positions.contains(&segment.start()));
                    assert!(positions.contains(&segment.end()));
                }
            }
        }
    }

    /// Proves a broader accepted connection family envelope is reused for a narrower distance,
    /// while a narrower envelope cannot satisfy a subsequent broader adjacency request.
    #[test]
    fn connection_document_family_cache_respects_guard_inclusive_distance_envelopes() {
        let bytes = valid_document_bytes();
        let scheduler = EvaluationScheduler::new_with_limits(EvaluationLimits::default()).unwrap();
        let mut broad_history = DocumentHistory::new(modeled_connection_session(
            random_connection_program(2, 30.0),
        ));
        let broad_ticket = scheduler
            .submit(document_request(
                broad_history.session(),
                Arc::clone(&bytes),
            ))
            .unwrap();
        let broad = wait_for_document_completion(&scheduler);
        assert_eq!(broad.ticket(), broad_ticket);
        assert!(broad.result().is_some());
        assert!(
            scheduler
                .accept_completion(&broad, broad_history.session())
                .unwrap()
        );
        replace_connection_program(&mut broad_history, random_connection_program(2, 12.0));
        let narrow_ticket = scheduler
            .submit(document_request(
                broad_history.session(),
                Arc::clone(&bytes),
            ))
            .unwrap();
        let narrow = wait_for_document_completion(&scheduler);
        assert_eq!(narrow.ticket(), narrow_ticket);
        assert_eq!(
            narrow.cache_diagnostics().unwrap().aggregate.family,
            CacheDisposition::Hit,
            "broader guard-inclusive coverage supports a narrower adjacency request"
        );
        assert!(
            scheduler
                .accept_completion(&narrow, broad_history.session())
                .unwrap()
        );
        scheduler.shutdown().unwrap();

        let reverse = EvaluationScheduler::new_with_limits(EvaluationLimits::default()).unwrap();
        let mut narrow_history = DocumentHistory::new(modeled_connection_session(
            random_connection_program(2, 12.0),
        ));
        let initial_ticket = reverse
            .submit(document_request(
                narrow_history.session(),
                Arc::clone(&bytes),
            ))
            .unwrap();
        let initial = wait_for_document_completion(&reverse);
        assert_eq!(initial.ticket(), initial_ticket);
        assert!(
            reverse
                .accept_completion(&initial, narrow_history.session())
                .unwrap()
        );
        replace_connection_program(&mut narrow_history, random_connection_program(2, 30.0));
        let expanded_ticket = reverse
            .submit(document_request(narrow_history.session(), bytes))
            .unwrap();
        let expanded = wait_for_document_completion(&reverse);
        assert_eq!(expanded.ticket(), expanded_ticket);
        assert_eq!(
            expanded.cache_diagnostics().unwrap().aggregate.family,
            CacheDisposition::Miss,
            "an insufficient accepted envelope cannot be reused for broader adjacency"
        );
        assert!(
            reverse
                .accept_completion(&expanded, narrow_history.session())
                .unwrap()
        );
        reverse.shutdown().unwrap();
    }

    /// Proves a superseded connection realization cannot publish a cache transaction and that
    /// only an explicitly accepted newest completion becomes reusable by the scheduler.
    #[test]
    fn connection_scheduler_stale_realization_cannot_publish_before_acceptance() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Realization, EvaluationCheckpoint::After);
        let scheduler =
            EvaluationScheduler::new_with_test_gate(EvaluationLimits::default(), Arc::clone(&gate))
                .unwrap();
        let mut history = DocumentHistory::new(modeled_connection_session(
            random_connection_program(2, 24.0),
        ));
        let bytes = valid_document_bytes();
        let stale_ticket = scheduler
            .submit(document_request(history.session(), Arc::clone(&bytes)))
            .unwrap();
        entered.recv_timeout(GUARD).unwrap();
        replace_connection_program(&mut history, random_connection_program(3, 24.0));
        let newest_ticket = scheduler
            .submit(document_request(history.session(), Arc::clone(&bytes)))
            .unwrap();
        gate.release();
        let newest = wait_for_document_completion(&scheduler);
        assert_ne!(stale_ticket, newest_ticket);
        assert_eq!(newest.ticket(), newest_ticket);
        assert_eq!(
            newest.cache_diagnostics().unwrap().aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Miss,
                family: CacheDisposition::Miss,
                realization: CacheDisposition::Miss,
                scene: CacheDisposition::Miss,
                raster: CacheDisposition::Miss,
            },
            "cancelled stale work cannot publish a connection cache transaction"
        );
        assert!(
            scheduler
                .accept_completion(&newest, history.session())
                .unwrap()
        );
        let repeated_ticket = scheduler
            .submit(document_request(history.session(), bytes))
            .unwrap();
        let repeated = wait_for_document_completion(&scheduler);
        assert_eq!(repeated.ticket(), repeated_ticket);
        assert_eq!(
            repeated.cache_diagnostics().unwrap().aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Hit,
                family: CacheDisposition::Hit,
                realization: CacheDisposition::Hit,
                scene: CacheDisposition::Hit,
                raster: CacheDisposition::Hit,
            }
        );
        assert!(
            scheduler
                .accept_completion(&repeated, history.session())
                .unwrap()
        );
        scheduler.shutdown().unwrap();
    }

    /// Proves Stage 20J structural edits miss family cache while Stage 20I thickness remains realization-only.
    #[test]
    fn normal_offset_cache_identity_reuses_and_invalidates_at_the_authoritative_levels() {
        let scheduler = EvaluationScheduler::new_with_limits(EvaluationLimits::default()).unwrap();
        let mut history = DocumentHistory::new(modeled_normal_offset_session());
        let definition_key =
            family_definition_key(&history.document().pattern_definition_bundles()[0].definition);
        let algorithm_key = definition_key
            .path_offset_algorithm
            .expect("normal-offset definitions carry the geometry algorithm cache identity");
        let limits = toniator_patterns::PathOffsetLimits::default();
        assert_eq!(
            algorithm_key.contract_id,
            toniator_patterns::PATH_OFFSET_ALGORITHM_CONTRACT_ID
        );
        assert_eq!(
            algorithm_key.maximum_subdivision_depth,
            limits.maximum_subdivision_depth
        );
        assert_eq!(algorithm_key.maximum_segments, limits.maximum_segments);
        assert_eq!(algorithm_key.maximum_components, limits.maximum_components);
        assert_eq!(
            algorithm_key.maximum_cleanup_pairs,
            limits.maximum_cleanup_pairs
        );
        assert_eq!(
            algorithm_key.maximum_cusp_isolation_work,
            limits.maximum_cusp_isolation_work
        );
        assert_eq!(algorithm_key.tolerance, limits.tolerance.to_bits());
        let bytes = flat_document_bytes();
        let submit =
            |scheduler: &EvaluationScheduler, session: &DocumentSession, bytes: Arc<[u8]>| {
                let ticket = scheduler.submit(document_request(session, bytes)).unwrap();
                let completion = wait_for_document_completion(scheduler);
                assert_eq!(completion.ticket(), ticket);
                assert!(
                    completion.result().is_some(),
                    "normal-offset evaluation failed: {:?}",
                    completion.error()
                );
                assert!(scheduler.accept_completion(&completion, session).unwrap());
                completion
            };
        let first = submit(&scheduler, history.session(), Arc::clone(&bytes));
        assert_eq!(
            first.cache_diagnostics().unwrap().aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Miss,
                family: CacheDisposition::Miss,
                realization: CacheDisposition::Miss,
                scene: CacheDisposition::Miss,
                raster: CacheDisposition::Miss,
            }
        );
        let repeated = submit(&scheduler, history.session(), Arc::clone(&bytes));
        assert_eq!(
            repeated.cache_diagnostics().unwrap().aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Hit,
                family: CacheDisposition::Hit,
                realization: CacheDisposition::Hit,
                scene: CacheDisposition::Hit,
                raster: CacheDisposition::Hit,
            }
        );

        let thickness = history
            .document()
            .set_channel_output_response_for_effective(
                CHANNEL_ID,
                PatternOutputLayerId(94),
                PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                    minimum_thickness: 0.012,
                    maximum_thickness: 0.024,
                    bias: 0.0,
                }),
            )
            .unwrap();
        history.apply(&thickness).unwrap();
        let thickness_completion = submit(&scheduler, history.session(), Arc::clone(&bytes));
        let thickness_cache = thickness_completion.cache_diagnostics().unwrap().aggregate;
        assert_eq!(thickness_cache.decoded_source, CacheDisposition::Hit);
        assert_eq!(thickness_cache.family, CacheDisposition::Hit);
        assert_eq!(thickness_cache.realization, CacheDisposition::Miss);
        assert_eq!(thickness_cache.scene, CacheDisposition::Miss);
        assert_eq!(thickness_cache.raster, CacheDisposition::Miss);

        let base_definition = history.document().pattern_definition_bundles()[0]
            .definition
            .clone();
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(90),
                base_definition,
                edit: PatternDefinitionEdit::SetGuideOffsetSpacing {
                    mechanism_id: PatternMechanismId(92),
                    dimension_id: GuideDimensionId(95),
                    spacing: 16.0,
                },
            })
            .unwrap();
        let structural = submit(&scheduler, history.session(), bytes);
        let structural_cache = structural.cache_diagnostics().unwrap().aggregate;
        assert_eq!(structural_cache.decoded_source, CacheDisposition::Hit);
        assert_eq!(structural_cache.family, CacheDisposition::Miss);
        assert_eq!(structural_cache.realization, CacheDisposition::Miss);
        assert_eq!(structural_cache.scene, CacheDisposition::Miss);
        assert_eq!(structural_cache.raster, CacheDisposition::Miss);
        scheduler.shutdown().unwrap();
    }

    /// Proves superseded Stage 20J family work cannot publish a partial cache transaction.
    #[test]
    fn normal_offset_stale_family_publication_is_cancelled_atomically() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Family, EvaluationCheckpoint::After);
        let scheduler =
            EvaluationScheduler::new_with_test_gate(EvaluationLimits::default(), Arc::clone(&gate))
                .unwrap();
        let mut history = DocumentHistory::new(modeled_normal_offset_session());
        let stale_ticket = scheduler
            .submit(document_request(history.session(), flat_document_bytes()))
            .unwrap();
        entered.recv_timeout(GUARD).unwrap();
        let base_definition = history.document().pattern_definition_bundles()[0]
            .definition
            .clone();
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(90),
                base_definition,
                edit: PatternDefinitionEdit::SetGuideOffsetSpacing {
                    mechanism_id: PatternMechanismId(92),
                    dimension_id: GuideDimensionId(95),
                    spacing: 13.0,
                },
            })
            .unwrap();
        let newest_ticket = scheduler
            .submit(document_request(history.session(), flat_document_bytes()))
            .unwrap();
        gate.release();
        let completion = wait_for_document_completion(&scheduler);
        assert_ne!(stale_ticket, newest_ticket);
        assert_eq!(completion.ticket(), newest_ticket);
        assert!(completion.result().is_some());
        assert!(
            scheduler
                .accept_completion(&completion, history.session())
                .unwrap()
        );
        let repeated_ticket = scheduler
            .submit(document_request(history.session(), flat_document_bytes()))
            .unwrap();
        let repeated = wait_for_document_completion(&scheduler);
        assert_eq!(repeated.ticket(), repeated_ticket);
        assert_eq!(
            repeated.cache_diagnostics().unwrap().aggregate,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Hit,
                family: CacheDisposition::Hit,
                realization: CacheDisposition::Hit,
                scene: CacheDisposition::Hit,
                raster: CacheDisposition::Hit,
            }
        );
        assert!(
            scheduler
                .accept_completion(&repeated, history.session())
                .unwrap()
        );
        scheduler.shutdown().unwrap();
    }

    #[test]
    fn complete_scheduler_reports_checked_ticket_exhaustion_without_panicking() {
        let scheduler = EvaluationScheduler::new().unwrap();
        let session = modeled_document_session();
        scheduler.set_next_ticket_for_test(Some(u64::MAX));
        assert_eq!(
            scheduler
                .submit(document_request(&session, valid_document_bytes()))
                .unwrap()
                .value(),
            u64::MAX
        );
        assert_eq!(
            scheduler.submit(document_request(&session, valid_document_bytes())),
            Err(SchedulerError::TicketExhausted)
        );
        scheduler.shutdown().unwrap();
    }

    #[test]
    fn complete_scheduler_polling_is_nonblocking_and_shutdown_rejects_completion() {
        let mut scheduler = EvaluationScheduler::new().unwrap();
        let session = modeled_document_session();
        assert_eq!(scheduler.try_receive_latest().unwrap(), None);
        let ticket = scheduler
            .submit(document_request(&session, valid_document_bytes()))
            .unwrap();
        let completion = wait_for_document_completion(&scheduler);
        assert_eq!(completion.ticket(), ticket);
        scheduler.stop_worker().unwrap();
        assert!(!scheduler.accept_completion(&completion, &session).unwrap());
        assert_eq!(scheduler.try_receive_latest().unwrap(), None);
        assert_eq!(
            scheduler.submit(document_request(&session, valid_document_bytes())),
            Err(SchedulerError::WorkerUnavailable)
        );
    }

    #[test]
    fn complete_scheduler_shutdown_and_drop_join_gated_work_without_hanging() {
        for explicit_shutdown in [true, false] {
            let (gate, entered) =
                EvaluationStageGate::new(EvaluationStage::Decode, EvaluationCheckpoint::After);
            let scheduler = EvaluationScheduler::new_with_test_gate(
                EvaluationLimits::default(),
                Arc::clone(&gate),
            )
            .unwrap();
            let session = modeled_document_session();
            scheduler
                .submit(document_request(&session, valid_document_bytes()))
                .unwrap();
            entered.recv_timeout(GUARD).unwrap();
            let (done_sender, done_receiver) = mpsc::channel();
            thread::spawn(move || {
                if explicit_shutdown {
                    done_sender.send(scheduler.shutdown()).unwrap();
                } else {
                    drop(scheduler);
                    done_sender.send(Ok(())).unwrap();
                }
            });
            gate.release();
            assert_eq!(done_receiver.recv_timeout(GUARD).unwrap(), Ok(()));
        }
    }

    /// Proves a cached immutable family may be reused for caller-derived topology without caching graphs.
    #[test]
    fn accepted_family_cache_reuses_for_derived_adjacency_and_cancellation_is_atomic() {
        let session = modeled_document_session();
        let request = document_request(&session, valid_document_bytes());
        let different_adjacency_limits = EvaluationLimits::default()
            .with_site_adjacency_limits(
                SiteAdjacencyLimits::new(10_000, 10_000, 10_000, 100_000)
                    .expect("sufficient nonzero topology limits"),
            )
            .expect("engine accepts sufficient topology limits");
        let ordinary_default = evaluate_with_limits(request.clone(), EvaluationLimits::default())
            .expect("ordinary document evaluation succeeds");
        let ordinary_different = evaluate_with_limits(request.clone(), different_adjacency_limits)
            .expect("adjacency-only limit change preserves ordinary evaluation");
        assert_eq!(ordinary_default.scene(), ordinary_different.scene());
        assert_eq!(ordinary_default.raster(), ordinary_different.raster());
        assert_eq!(ordinary_default.channels(), ordinary_different.channels());
        let mut cache = DocumentDerivedCache::default();
        let first = evaluate_cached_document(
            request.clone(),
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("first document family succeeds");
        cache.commit(first.transaction);
        let accepted_family_count = cache.families.len();
        let family = Arc::clone(&cache.families[0].1);
        let policy = SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 2,
            maximum_distance: 10.0,
        };
        let graph = derive_site_adjacency_cancellable(
            &family,
            0.0,
            policy,
            EvaluationLimits::default(),
            &|| false,
        )
        .expect("accepted family derives topology");
        let second = evaluate_cached_document(
            request,
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("identical document reuses family");
        assert_eq!(second.diagnostics.channels[0].family, CacheDisposition::Hit);
        assert_eq!(
            derive_site_adjacency_cancellable(
                &family,
                0.0,
                policy,
                EvaluationLimits::default(),
                &|| true,
            )
            .expect_err("cancelled graph cannot publish")
            .path(),
            "evaluation.cancelled",
        );
        assert_eq!(cache.families.len(), accepted_family_count);
        assert_eq!(graph.nodes().len(), family.site_set().len());
    }

    /// Builds a two-output connection/residual-mark fixture with painter order opposite dependency order.
    fn modeled_stage20r_composite_session(mark_first: bool) -> DocumentSession {
        modeled_stage20r_composite_session_for_canvas(
            mark_first,
            CanvasSpec {
                width: 64.0,
                height: 48.0,
            },
        )
    }

    /// Builds independent mark and connection outputs whose connection seed alone changes identity.
    fn modeled_independent_mark_connection_session(seed: u32) -> DocumentSession {
        let base = modeled_stage20r_composite_session(false);
        let document = base.document();
        let mut bundle = document.pattern_definition_bundles()[0].clone();
        for output in &mut bundle.definition.output_layers {
            match &mut output.realization {
                PatternOutputRealization::ConnectionPaths { program, .. } => match program {
                    ConnectionProgram::RandomLinks {
                        seed: current_seed, ..
                    } => *current_seed = seed,
                    _ => unreachable!("fixture connection uses random links"),
                },
                PatternOutputRealization::MarkPrototype { .. } => {
                    output.source_filter = SiteUseFilter::All;
                }
                _ => unreachable!("fixture contains only connection and mark outputs"),
            }
        }
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                document.id(),
                document.canvas().clone(),
                document.source().clone(),
                vec![bundle],
                document.pattern_settings().clone(),
                document.channel_model().expect("modeled document model"),
                document
                    .channel_topology()
                    .expect("modeled document topology")
                    .clone(),
                Vec::new(),
            )
            .expect("independent mixed-output document validates"),
        )
        .expect("independent mixed-output session validates")
    }

    /// Builds the connection/residual-mark fixture at one intrinsic artifact canvas.
    fn modeled_stage20r_composite_session_for_canvas(
        mark_first: bool,
        canvas: CanvasSpec,
    ) -> DocumentSession {
        let maximum_distance = canvas.width.max(canvas.height) * 0.4;
        let base = modeled_connection_session_for_canvas(
            ConnectionProgram::RandomLinks {
                adjacency: ConnectionAdjacencyIntent {
                    maximum_degree: 2,
                    maximum_distance,
                },
                minimum_degree: 0,
                seed: 29,
            },
            canvas,
        );
        let document = base.document();
        let original = &document.pattern_definition_bundles()[0];
        let mut definition = original.definition.clone();
        let connection = definition.output_layers[0].clone();
        let connection_id = connection.id;
        let mark_id = PatternOutputLayerId(connection_id.0 + 1);
        let residual_marks = PatternOutputLayer::new(
            mark_id,
            SiteUseFilter::SitesUnusedBy {
                output_layer_id: connection_id,
            },
            PatternOutputRealization::MarkPrototype {
                site_mechanism_id: PatternMechanismId(121),
                prototype: MarkPrototype::Circle,
                orientation: MarkOrientation::Fixed,
            },
        );
        definition.output_layers = if mark_first {
            vec![residual_marks, connection]
        } else {
            vec![connection, residual_marks]
        };
        let output_settings = definition
            .output_layers
            .iter()
            .map(|output| PatternOutputSettings {
                output_layer_id: output.id,
                response: match output.realization {
                    PatternOutputRealization::MarkPrototype { .. } => {
                        PatternGeometryResponse::Marks(MarkGeometryResponse {
                            minimum_fill: 0.2,
                            maximum_fill: 0.8,
                        })
                    }
                    PatternOutputRealization::ConnectionPaths { .. } => {
                        PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                            minimum_thickness: 0.1,
                            maximum_thickness: 0.25,
                            bias: 0.0,
                        })
                    }
                    _ => unreachable!("fixture contains marks and connections only"),
                },
            })
            .collect();
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                document.id(),
                document.canvas().clone(),
                document.source().clone(),
                vec![PatternDefinitionBundle {
                    definition,
                    output_settings,
                }],
                document.pattern_settings().clone(),
                document.channel_model().expect("modeled document model"),
                document
                    .channel_topology()
                    .expect("modeled document topology")
                    .clone(),
                Vec::new(),
            )
            .expect("composite document validates"),
        )
        .expect("composite session validates")
    }

    /// Builds the circle-free sampled inward Voronoi validation document.
    fn modeled_stage20r_sampled_region_session(canvas: CanvasSpec) -> DocumentSession {
        let base =
            modeled_connection_session_for_canvas(random_connection_program(37, 36.0), canvas);
        let document = base.document();
        let mut definition = document.pattern_definition_bundles()[0].definition.clone();
        definition.coverage.guard_steps = 8;
        let region_id = PatternOutputLayerId(122);
        let regions = PatternOutputLayer::all(
            region_id,
            PatternOutputRealization::Regions {
                source: RegionSourceIntent::VoronoiSites {
                    site_mechanism_id: PatternMechanismId(121),
                },
            },
        );
        definition.output_layers = vec![regions];
        let document = Document::with_source_topology_and_authored_structures(
            document.id(),
            document.canvas().clone(),
            document.source().clone(),
            vec![PatternDefinitionBundle {
                definition,
                output_settings: vec![PatternOutputSettings {
                    output_layer_id: region_id,
                    response: PatternGeometryResponse::Regions(RegionGeometryResponse {
                        algorithm: toniator_domain::RegionResizeAlgorithm::Scale,
                        sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
                        minimum_fill: 0.62,
                        maximum_fill: 0.9,
                    }),
                }],
            }],
            document.pattern_settings().clone(),
            document.channel_model().expect("modeled document model"),
            document
                .channel_topology()
                .expect("modeled document topology")
                .clone(),
            Vec::new(),
        )
        .expect("sampled composite document validates");
        stage20q_sampled_region_session(document, 0.63)
    }

    /// Builds direct authored sampled-region and mark outputs without registering a catalog recipe.
    fn modeled_stage20_authored_region_mark_session(canvas: CanvasSpec) -> DocumentSession {
        let base = modeled_stage20r_sampled_region_session(canvas);
        let document = base.document();
        let original = &document.pattern_definition_bundles()[0];
        let mut definition = original.definition.clone();
        let mark_id = PatternOutputLayerId(123);
        definition.output_layers.push(PatternOutputLayer::all(
            mark_id,
            PatternOutputRealization::MarkPrototype {
                site_mechanism_id: PatternMechanismId(121),
                prototype: MarkPrototype::Circle,
                orientation: MarkOrientation::Fixed,
            },
        ));
        let mut output_settings = original.output_settings.clone();
        output_settings.push(PatternOutputSettings {
            output_layer_id: mark_id,
            response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.12,
                maximum_fill: 0.3,
            }),
        });
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                document.id(),
                document.canvas().clone(),
                document.source().clone(),
                vec![PatternDefinitionBundle {
                    definition,
                    output_settings,
                }],
                document.pattern_settings().clone(),
                document.channel_model().expect("modeled document model"),
                document
                    .channel_topology()
                    .expect("modeled document topology")
                    .clone(),
                Vec::new(),
            )
            .expect("direct authored region-and-mark document validates"),
        )
        .expect("direct authored region-and-mark session validates")
    }

    /// Gives every modeled artifact channel an independently authored seed while retaining all
    /// channels as visible renderer inputs.
    ///
    /// The selected-channel edit authority performs any required bundle clone and stable-ID
    /// remapping; the fixture does not mutate definitions or channel overrides directly.
    ///
    /// # Panics
    ///
    /// Panics when the fixture does not contain exactly one seed per channel, when a channel has
    /// no selected definition or sole output, or when the authoritative selected edit fails.
    fn stage20r_seeded_output_channels(
        session: DocumentSession,
        seeds: &[u32],
        edit: impl Fn(PatternOutputLayerId, u32) -> PatternDefinitionEdit,
    ) -> DocumentSession {
        let mut history = DocumentHistory::new(session);
        let channel_ids = history
            .document()
            .channel_topology()
            .expect("modeled artifact topology exists")
            .channels()
            .iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>();
        assert_eq!(
            channel_ids.len(),
            seeds.len(),
            "every modeled artifact channel receives one seed"
        );
        for (channel_id, seed) in channel_ids.into_iter().zip(seeds.iter().copied()) {
            let base_definition = history
                .document()
                .pattern_definition_for(channel_id)
                .expect("artifact channel selects one definition")
                .clone();
            let output_layer_id = base_definition
                .output_layers
                .first()
                .expect("seeded artifact definition has one output")
                .id;
            history
                .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                    channel_id,
                    base_definition,
                    edit: edit(output_layer_id, seed),
                })
                .expect("selected artifact seed applies");
        }
        history.session().clone()
    }

    /// Proves solid Stage 20R evidence retains every RGB channel and assigns distinct authored
    /// seeds through selected copy-on-edit authority.
    #[test]
    fn stage20r_solid_artifact_channels_are_visible_and_independently_seeded() {
        let connections = stage20r_seeded_output_channels(
            modeled_connection_session(random_connection_program(11, 24.0)),
            &[29, 43, 71],
            |output_layer_id, seed| PatternDefinitionEdit::SetConnectionSeed {
                output_layer_id,
                seed,
            },
        );
        let connection_channels = connections
            .document()
            .channel_topology()
            .expect("connection artifact topology exists")
            .channels();
        assert!(connection_channels.iter().all(|channel| channel.visible));
        assert_eq!(connection_channels.len(), 3);
        let connection_seeds = connection_channels
            .iter()
            .map(|channel| {
                let definition = connections
                    .document()
                    .pattern_definition_for(channel.id)
                    .expect("connection channel definition resolves");
                match &definition.output_layers[0].realization {
                    PatternOutputRealization::ConnectionPaths {
                        program: ConnectionProgram::RandomLinks { seed, .. },
                        ..
                    } => *seed,
                    _ => panic!("connection artifact retains only random-link outputs"),
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(connection_seeds, vec![29, 43, 71]);

        let mazes = stage20r_seeded_output_channels(
            modeled_maze_session(11),
            &[23, 47, 89],
            |output_layer_id, seed| PatternDefinitionEdit::SetMazeSeed {
                output_layer_id,
                seed,
            },
        );
        let maze_channels = mazes
            .document()
            .channel_topology()
            .expect("maze artifact topology exists")
            .channels();
        assert!(maze_channels.iter().all(|channel| channel.visible));
        assert_eq!(maze_channels.len(), 3);
        let maze_seeds = maze_channels
            .iter()
            .map(|channel| {
                let definition = mazes
                    .document()
                    .pattern_definition_for(channel.id)
                    .expect("maze channel definition resolves");
                match &definition.output_layers[0].realization {
                    PatternOutputRealization::MazeWalls { program, .. } => program.seed,
                    _ => panic!("maze artifact retains only wall outputs"),
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(maze_seeds, vec![23, 47, 89]);
    }

    /// Builds the product workflow with connections and regions selected by different channels.
    fn modeled_stage20r_cross_channel_connection_region_session() -> DocumentSession {
        let base = modeled_connection_session(random_connection_program(31, 24.0));
        let document = base.document();
        let connection_bundle = document.pattern_definition_bundles()[0].clone();
        let mut region_definition = PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(219),
            "Stage 20R cross-channel regions",
            PatternMechanismId(220),
            PatternMechanismId(221),
            PatternOutputLayerId(222),
            vec![
                StraightGuideDimension {
                    id: GuideDimensionId(223),
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: GuideDimensionId(224),
                    baseline_angle_degrees: 60.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: GuideDimensionId(225),
                    baseline_angle_degrees: 120.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
            ],
            GeneralizedSiteProduct::Intersections {
                dimensions: vec![
                    GuideDimensionId(223),
                    GuideDimensionId(224),
                    GuideDimensionId(225),
                ],
                merge_epsilon: 1.0e-8,
            },
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 32,
                additional_margin: 0.0,
            },
        );
        region_definition.output_layers = vec![PatternOutputLayer::all(
            PatternOutputLayerId(222),
            PatternOutputRealization::Regions {
                source: RegionSourceIntent::VoronoiSites {
                    site_mechanism_id: PatternMechanismId(221),
                },
            },
        )];
        let region_bundle = PatternDefinitionBundle {
            definition: region_definition,
            output_settings: vec![PatternOutputSettings {
                output_layer_id: PatternOutputLayerId(222),
                response: PatternGeometryResponse::Regions(RegionGeometryResponse::default()),
            }],
        };
        let mut channels = document
            .channel_topology()
            .expect("modeled cross-channel topology exists")
            .channels()
            .to_vec();
        channels[1].pattern_instance.definition_override = Some(PatternDefinitionId(219));
        channels[1].pattern_instance.output_response_deltas.clear();
        channels[2].visible = false;
        DocumentSession::new(
            Document::with_source_topology_and_authored_structures(
                document.id(),
                document.canvas().clone(),
                document.source().clone(),
                vec![connection_bundle, region_bundle],
                document.pattern_settings().clone(),
                document.channel_model().expect("modeled document model"),
                toniator_domain::ChannelTopology::new(channels),
                document.authored_structures().to_vec(),
            )
            .expect("cross-channel connection/region document validates"),
        )
        .expect("cross-channel connection/region session validates")
    }

    /// Proves distinct channels select connection and region realizations without same-channel mixing.
    #[test]
    fn stage20r_cross_channel_connection_and_region_outputs_remain_separate() {
        let session = modeled_stage20r_cross_channel_connection_region_session();
        let request = document_request(&session, valid_document_bytes());
        let mut cache = DocumentDerivedCache::default();
        let first = evaluate_cached_document(
            request.clone(),
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("cross-channel document evaluates");
        assert!(matches!(
            first.result.scene().layers()[0].outputs()[0].geometry(),
            GeometryOutput::CanonicalStrokes(_)
        ));
        assert!(matches!(
            first.result.scene().layers()[1].outputs()[0].geometry(),
            GeometryOutput::CanonicalRegions(_)
        ));
        assert_eq!(
            first.diagnostics.channels[0].outputs[0].output_layer_id,
            PatternOutputLayerId(122)
        );
        assert_eq!(
            first.diagnostics.channels[1].outputs[0].output_layer_id,
            PatternOutputLayerId(222)
        );
        cache.commit(first.transaction);
        let replay = evaluate_cached_document(
            request,
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("cross-channel document replays");
        assert!(
            replay.diagnostics.channels[..2]
                .iter()
                .all(|channel| channel.family == CacheDisposition::Hit
                    && channel.outputs[0].realization == CacheDisposition::Hit)
        );
    }

    /// Proves DAG evaluation supplies residual marks while renderer payload and diagnostics remain painter ordered.
    #[test]
    fn stage20r_dependency_order_is_independent_from_painter_order() {
        let session = modeled_stage20r_composite_session(true);
        let definition = &session.document().pattern_definition_bundles()[0].definition;
        let plan =
            toniator_patterns::resolve_document_pattern_pipeline(session.document(), definition)
                .expect("composite plan resolves");
        assert_eq!(
            plan.ordered_outputs
                .iter()
                .map(|output| output.layer_id)
                .collect::<Vec<_>>(),
            vec![PatternOutputLayerId(123), PatternOutputLayerId(122)]
        );
        assert_eq!(
            plan.evaluation_order,
            vec![PatternOutputLayerId(122), PatternOutputLayerId(123)]
        );
        let evaluated = evaluate_cached_document(
            document_request(&session, valid_document_bytes()),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        )
        .expect("composite evaluates atomically");
        assert_eq!(
            evaluated.result.scene().layers()[0]
                .outputs()
                .iter()
                .map(|output| output.output_layer_id)
                .collect::<Vec<_>>(),
            vec![PatternOutputLayerId(123), PatternOutputLayerId(122)]
        );
        assert_eq!(
            evaluated.diagnostics.channels[0]
                .outputs
                .iter()
                .map(|output| output.output_layer_id)
                .collect::<Vec<_>>(),
            vec![PatternOutputLayerId(123), PatternOutputLayerId(122)]
        );
        let connection_usage = &evaluated.transaction.realizations[0].1.usage;
        let residual_usage = &evaluated.transaction.realizations[1].1.usage;
        assert!(!connection_usage.members().is_empty());
        assert!(!residual_usage.members().is_empty());
        assert!(
            connection_usage
                .members()
                .iter()
                .all(|member| !residual_usage.members().contains(member))
        );
    }

    /// Proves painter-only reordering reuses every independent output cache while rebuilding scene order.
    #[test]
    fn stage20r_painter_move_replays_output_caches_in_new_authored_order() {
        let first_session = modeled_stage20r_composite_session(true);
        let moved_session = modeled_stage20r_composite_session(false);
        let bytes = valid_document_bytes();
        let mut cache = DocumentDerivedCache::default();
        let first = evaluate_cached_document(
            document_request(&first_session, Arc::clone(&bytes)),
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("initial composite evaluates");
        assert!(
            first.diagnostics.channels[0]
                .outputs
                .iter()
                .all(|output| output.realization == CacheDisposition::Miss)
        );
        cache.commit(first.transaction);
        let moved = evaluate_cached_document(
            document_request(&moved_session, bytes),
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("painter-moved composite evaluates");
        assert_eq!(moved.diagnostics.channels[0].family, CacheDisposition::Hit);
        assert_eq!(
            moved.diagnostics.channels[0]
                .outputs
                .iter()
                .map(|output| (output.output_layer_id, output.realization))
                .collect::<Vec<_>>(),
            vec![
                (PatternOutputLayerId(122), CacheDisposition::Hit),
                (PatternOutputLayerId(123), CacheDisposition::Hit),
            ]
        );
        assert_eq!(moved.diagnostics.aggregate.scene, CacheDisposition::Miss);
        assert_eq!(
            moved.result.scene().layers()[0]
                .outputs()
                .iter()
                .map(|output| output.output_layer_id)
                .collect::<Vec<_>>(),
            vec![PatternOutputLayerId(122), PatternOutputLayerId(123)]
        );
    }

    /// Proves a connection seed edit invalidates only that independently keyed mixed output.
    #[test]
    fn connection_program_identity_is_scoped_to_the_matching_output_cache_unit() {
        let first_session = modeled_independent_mark_connection_session(29);
        let edited_session = modeled_independent_mark_connection_session(31);
        let bytes = valid_document_bytes();
        let mut cache = DocumentDerivedCache::default();
        let first = evaluate_cached_document(
            document_request(&first_session, Arc::clone(&bytes)),
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("initial mixed-output document evaluates");
        cache.commit(first.transaction);
        let edited = evaluate_cached_document(
            document_request(&edited_session, bytes),
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("seed-edited mixed-output document evaluates");
        assert!(edited.diagnostics.channels.iter().all(|channel| {
            channel.family == CacheDisposition::Hit
                && channel
                    .outputs
                    .iter()
                    .find(|output| output.output_layer_id == PatternOutputLayerId(123))
                    .is_some_and(|output| output.realization == CacheDisposition::Hit)
                && channel
                    .outputs
                    .iter()
                    .find(|output| output.output_layer_id == PatternOutputLayerId(122))
                    .is_some_and(|output| output.realization == CacheDisposition::Miss)
        }));
    }

    /// Proves maze usage publishes retained wall endpoints and dependent marks consume that exact set.
    #[test]
    fn stage20r_maze_endpoint_usage_drives_dependent_marks() {
        let session = modeled_stage20r_maze_endpoint_session(23);
        let evaluated = evaluate_cached_document(
            document_request(&session, valid_document_bytes()),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        )
        .expect("maze endpoint composite evaluates");
        let maze_usage = evaluated
            .transaction
            .realizations
            .iter()
            .find(|(_, output)| output.output_layer_id == PatternOutputLayerId(122))
            .map(|(_, output)| output.usage.clone())
            .expect("maze usage publishes");
        let mark_usage = evaluated
            .transaction
            .realizations
            .iter()
            .find(|(_, output)| output.output_layer_id == PatternOutputLayerId(123))
            .map(|(_, output)| output.usage.clone())
            .expect("dependent mark usage publishes");
        assert!(!maze_usage.members().is_empty());
        assert_eq!(mark_usage, maze_usage);
    }

    /// Proves retained Voronoi co-owners are unique usage members and collapsed output is empty.
    #[test]
    fn stage20r_voronoi_usage_retains_all_coowners_and_accepts_empty_collapse() {
        let mechanism_id = PatternMechanismId(77);
        let owners = vec![
            FamilySiteId {
                mechanism_id,
                ordinal: 4,
            },
            FamilySiteId {
                mechanism_id,
                ordinal: 9,
            },
        ];
        let regions = build_canonical_regions_cancellable(
            CanonicalRegionProposal {
                output_layer_id: PatternOutputLayerId(88),
                source_groups: vec![CanonicalRegionSourceGroup {
                    source_id: CanonicalRegionSourceId::SiteOwners(owners.clone()),
                    components: vec![
                        CurvePath::polyline(
                            vec![
                                Point2::new(-8.0, -4.0),
                                Point2::new(12.0, -4.0),
                                Point2::new(2.0, 14.0),
                            ],
                            PathClosure::Closed,
                        )
                        .expect("co-owner region closes"),
                    ],
                }],
            },
            CanonicalRegionLimits::default(),
            || false,
        )
        .expect("co-owner region canonicalizes")
        .0;
        assert_eq!(
            voronoi_region_site_usage(mechanism_id, &regions)
                .expect("co-owner usage derives")
                .members(),
            owners
        );
        assert!(
            voronoi_region_site_usage(
                mechanism_id,
                &toniator_patterns::CanonicalRegionSet::empty()
            )
            .expect("empty treated output publishes empty usage")
            .members()
            .is_empty()
        );
    }

    /// Proves composite output and dependency budgets fail before any cache transaction publishes.
    #[test]
    fn stage20r_request_wide_composite_limits_are_atomic() {
        let session = modeled_stage20r_composite_session(true);
        let output_limited = EvaluationLimits::default()
            .with_composite_output_limits(
                CompositeOutputLimits::new(1, 8_388_608, 16_777_216)
                    .expect("nonzero output policy"),
            )
            .expect("composite policy installs");
        let error = match evaluate_cached_document(
            document_request(&session, valid_document_bytes()),
            output_limited,
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        ) {
            Err(EvaluationRunError::Evaluation(error)) => error,
            Err(EvaluationRunError::Cancelled) => panic!("limit must not report cancellation"),
            Ok(_) => panic!("two output units exceed the request-wide limit"),
        };
        assert_eq!(error.path(), "realization.composite.limits.output_units");
        for (limits, expected_path) in [
            (
                CompositeOutputLimits::new(4_096, 1, 16_777_216)
                    .expect("membership policy validates"),
                "realization.composite.limits.usage_memberships",
            ),
            (
                CompositeOutputLimits::new(4_096, 8_388_608, 1)
                    .expect("inspection policy validates"),
                "realization.composite.limits.dependency_inspections",
            ),
        ] {
            let error = match evaluate_cached_document(
                document_request(&session, valid_document_bytes()),
                EvaluationLimits::default()
                    .with_composite_output_limits(limits)
                    .expect("composite policy installs"),
                &DocumentDerivedCache::default(),
                &NeverCancelled,
            ) {
                Err(EvaluationRunError::Evaluation(error)) => error,
                Err(EvaluationRunError::Cancelled) => {
                    panic!("limit exhaustion must not report cancellation")
                }
                Ok(_) => panic!("request-wide composite policy must reject the fixture"),
            };
            assert_eq!(error.path(), expected_path);
        }
    }

    /// Proves stroke-profile work is charged across every independently realized channel output.
    #[test]
    fn request_wide_stroke_profile_limit_aggregates_outputs() {
        let session = modeled_connection_session(random_connection_program(31, 24.0));
        let baseline = evaluate_cached_document(
            document_request(&session, valid_document_bytes()),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        )
        .expect("connection outputs evaluate under default stroke limits");
        let per_output = baseline
            .transaction
            .realizations
            .iter()
            .map(|(_, output)| match output.realization.geometry().as_ref() {
                GeometryOutput::CanonicalStrokes(strokes) => {
                    canonical_stroke_work(strokes)
                        .expect("bounded stroke work counts")
                        .0
                }
                _ => panic!("connection fixture owns only stroke outputs"),
            })
            .collect::<Vec<_>>();
        let total = per_output.iter().sum::<usize>();
        assert!(per_output.len() >= 2);
        assert!(per_output.iter().all(|count| *count < total));
        let limits = EvaluationLimits::default()
            .with_max_stroke_profile_samples(total - 1)
            .expect("aggregate-limited policy validates");
        let error = match evaluate_cached_document(
            document_request(&session, valid_document_bytes()),
            limits,
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        ) {
            Err(EvaluationRunError::Evaluation(error)) => error,
            Err(EvaluationRunError::Cancelled) => panic!("limit must not report cancellation"),
            Ok(_) => panic!("aggregate stroke work must exceed the request-wide limit"),
        };
        assert_eq!(error.path(), "connection.stroke.profile_limit");
    }

    /// Proves canonical outline-segment work is charged across all independent channel outputs.
    #[test]
    fn request_wide_stroke_outline_limit_aggregates_outputs() {
        let session = modeled_connection_session(random_connection_program(31, 24.0));
        let baseline = evaluate_cached_document(
            document_request(&session, valid_document_bytes()),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        )
        .expect("connection outputs evaluate under default outline limits");
        let per_output = baseline
            .transaction
            .realizations
            .iter()
            .map(|(_, output)| match output.realization.geometry().as_ref() {
                GeometryOutput::CanonicalStrokes(strokes) => {
                    canonical_stroke_work(strokes)
                        .expect("bounded stroke work counts")
                        .1
                }
                _ => panic!("connection fixture owns only stroke outputs"),
            })
            .collect::<Vec<_>>();
        let total = per_output.iter().sum::<usize>();
        assert!(per_output.len() >= 2);
        assert!(per_output.iter().all(|count| *count < total));
        let limits = EvaluationLimits::default()
            .with_max_stroke_outline_segments(total - 1)
            .expect("aggregate outline-limited policy validates");
        let error = match evaluate_cached_document(
            document_request(&session, valid_document_bytes()),
            limits,
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        ) {
            Err(EvaluationRunError::Evaluation(error)) => error,
            Err(EvaluationRunError::Cancelled) => panic!("limit must not report cancellation"),
            Ok(_) => panic!("aggregate outline work must exceed the request-wide limit"),
        };
        assert_eq!(error.path(), "connection.stroke.outline_limit");
    }

    /// Generates native Stage 20R composite documents, PNG/SVG outputs, identities, and raw statistics.
    #[test]
    #[ignore = "writes Stage 20R composite validation artifacts"]
    fn generate_stage20r_composite_artifacts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = root.join("target/validation/stage20r");
        fs::create_dir_all(&output).expect("Stage 20R validation directory exists");
        let cases = [
            (
                "connection-paths-1024x1024",
                stage20r_seeded_output_channels(
                    modeled_connection_session_for_canvas(
                        ConnectionProgram::RandomLinks {
                            adjacency: ConnectionAdjacencyIntent {
                                maximum_degree: 2,
                                maximum_distance: 409.6,
                            },
                            minimum_degree: 0,
                            seed: 11,
                        },
                        CanvasSpec {
                            width: 1024.0,
                            height: 1024.0,
                        },
                    ),
                    &[29, 43, 71],
                    |output_layer_id, seed| PatternDefinitionEdit::SetConnectionSeed {
                        output_layer_id,
                        seed,
                    },
                ),
                root.join("assets/raster-sample.png"),
                EmbeddedSourceFormat::Png,
                SourceFormatHint::Png,
            ),
            (
                "area-average-regions-900x620",
                modeled_stage20r_sampled_region_session(CanvasSpec {
                    width: 900.0,
                    height: 620.0,
                }),
                root.join("assets/vector-sample.svg"),
                EmbeddedSourceFormat::Svg,
                SourceFormatHint::Svg,
            ),
            (
                "authored-regions-and-marks-900x620",
                modeled_stage20_authored_region_mark_session(CanvasSpec {
                    width: 900.0,
                    height: 620.0,
                }),
                root.join("assets/vector-sample.svg"),
                EmbeddedSourceFormat::Svg,
                SourceFormatHint::Svg,
            ),
            (
                "maze-endpoint-usage-64x48",
                stage20r_seeded_output_channels(
                    modeled_maze_session(11),
                    &[23, 47, 89],
                    |output_layer_id, seed| PatternDefinitionEdit::SetMazeSeed {
                        output_layer_id,
                        seed,
                    },
                ),
                root.join("assets/raster-sample.png"),
                EmbeddedSourceFormat::Png,
                SourceFormatHint::Png,
            ),
        ];
        let mut manifest = String::from(
            "# Stage 20R ordered composite validation\n\n\
             Native RGBA is preserved. SVG rasterizations are comparison witnesses only.\n\
             Solid RGB witnesses retain every channel, with distinct deterministic output seeds\n\
             per channel. The mixed region-and-mark witness is direct authored document data,\n\
             not a bundled recipe or wizard card.\n\n",
        );
        for (stem, session, source_path, embedded_format, source_format) in cases {
            let source_bytes = fs::read(&source_path).expect("immutable source reads");
            let source_id = SourceReferenceId::new("cancellation-test-source")
                .expect("fixture source ID validates");
            let sources = SourceBundle::new([EmbeddedSource::new(
                source_id.clone(),
                embedded_format,
                source_bytes.clone(),
                source_path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned()),
            )
            .expect("immutable source embeds")])
            .expect("source bundle validates");
            let document_path = output.join(format!("{stem}.toniator"));
            save(&document_path, session.document(), &sources)
                .expect("Stage 20R v5 document saves");
            let loaded = load(&document_path).expect("Stage 20R v5 document reloads");
            let loaded_session = DocumentSession::new(loaded.document().clone())
                .expect("reloaded Stage 20R session validates");
            let evaluated = evaluate_cached_document(
                document_request_for_format(
                    &loaded_session,
                    "cancellation-test-source",
                    Arc::<[u8]>::from(source_bytes),
                    source_format,
                ),
                EvaluationLimits::default(),
                &DocumentDerivedCache::default(),
                &NeverCancelled,
            )
            .expect("Stage 20R composite evaluates");
            let png = encode_png(evaluated.result.raster()).expect("native PNG encodes");
            let svg = write_svg(evaluated.result.scene());
            let png_path = output.join(format!("{stem}.png"));
            let svg_path = output.join(format!("{stem}.svg"));
            fs::write(&png_path, &png).expect("native PNG writes");
            fs::write(&svg_path, &svg).expect("raw SVG writes");
            let svg_raster_path = output.join(format!("{stem}-svg-rasterized.png"));
            let status = Command::new("inkscape")
                .arg(&svg_path)
                .arg("--export-type=png")
                .arg(format!("--export-filename={}", svg_raster_path.display()))
                .status()
                .expect("Inkscape launches for SVG rasterization");
            assert!(status.success(), "raw SVG rasterizes");
            let svg_raster =
                image::load_from_memory(&fs::read(&svg_raster_path).expect("SVG raster reads"))
                    .expect("SVG raster decodes")
                    .to_rgba8();
            fs::write(
                output.join(format!("{stem}-native-rgba-stats.txt")),
                stage20q_rgba_statistics(
                    evaluated.result.raster().width(),
                    evaluated.result.raster().height(),
                    evaluated.result.raster().pixels(),
                ),
            )
            .expect("native statistics write");
            fs::write(
                output.join(format!("{stem}-svg-raster-rgba-stats.txt")),
                stage20q_rgba_statistics(
                    svg_raster.width(),
                    svg_raster.height(),
                    svg_raster.as_raw(),
                ),
            )
            .expect("SVG raster statistics write");
            let channel_records = loaded
                .document()
                .channel_topology()
                .expect("artifact topology exists")
                .channels()
                .iter()
                .map(|channel| {
                    let definition = loaded
                        .document()
                        .pattern_definition_for(channel.id)
                        .expect("artifact channel definition resolves");
                    let channel_plan = toniator_patterns::resolve_document_pattern_pipeline(
                        loaded.document(),
                        definition,
                    )
                    .expect("artifact channel plan resolves");
                    let seeds = definition
                        .output_layers
                        .iter()
                        .filter_map(|output| match &output.realization {
                            PatternOutputRealization::ConnectionPaths {
                                program: ConnectionProgram::RandomLinks { seed, .. },
                                ..
                            }
                            | PatternOutputRealization::MazeWalls {
                                program: toniator_domain::MazeProgram { seed, .. },
                                ..
                            } => Some(*seed),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    (
                        channel.id,
                        channel.visible,
                        definition.id,
                        seeds,
                        channel_plan
                            .ordered_outputs
                            .iter()
                            .map(|output| output.layer_id)
                            .collect::<Vec<_>>(),
                        channel_plan.evaluation_order,
                    )
                })
                .collect::<Vec<_>>();
            let painter_order_differs_from_dependency = channel_records
                .iter()
                .any(|record| record.4.iter().copied().ne(record.5.iter().copied()));
            let channel_seeds = channel_records
                .iter()
                .flat_map(|record| record.3.iter().copied())
                .collect::<Vec<_>>();
            let channel_seed_summary = if channel_seeds.is_empty() {
                String::from("not-applicable")
            } else {
                format!("{channel_seeds:?}")
            };
            fs::write(
                output.join(format!("{stem}-painter-dag-usage.txt")),
                format!(
                    "channels={channel_records:#?}\nusage={:#?}\nusage_scopes={:#?}\ncache_diagnostics={:#?}\nfinal_clip={{canvas:{}x{},svg_clip_path:{}}}\n",
                    evaluated
                        .transaction
                        .realizations
                        .iter()
                        .map(|(_, output)| (
                            output.output_layer_id,
                            output.usage.site_mechanism_id(),
                            output.usage.members().len(),
                            output.usage.fingerprint(),
                        ))
                        .collect::<Vec<_>>(),
                    evaluated
                        .transaction
                        .realizations
                        .iter()
                        .map(|(_, output)| {
                            let scopes = output
                                .usage
                                .members()
                                .iter()
                                .filter_map(|member| {
                                    evaluated
                                        .transaction
                                        .families
                                        .iter()
                                        .find_map(|(_, family)| {
                                            family
                                                .site_set()
                                                .iter()
                                                .find(|site| site.id == *member)
                                                .map(|site| site.scope)
                                        })
                                })
                                .collect::<Vec<_>>();
                            (output.output_layer_id, scopes)
                        })
                        .collect::<Vec<_>>(),
                    evaluated.diagnostics.channels,
                    evaluated.result.raster().width(),
                    evaluated.result.raster().height(),
                    svg.contains("clipPath"),
                ),
            )
            .expect("painter/DAG/usage record writes");
            fs::write(
                output.join(format!("{stem}-hashes.txt")),
                format!(
                    "{}  {}\n{}  {}\n{}  {}\n",
                    stage20q_sha256(&png),
                    png_path.file_name().expect("PNG name").to_string_lossy(),
                    stage20q_sha256(svg.as_bytes()),
                    svg_path.file_name().expect("SVG name").to_string_lossy(),
                    stage20q_sha256(
                        &fs::read(&svg_raster_path).expect("SVG raster hash input reads")
                    ),
                    svg_raster_path
                        .file_name()
                        .expect("SVG raster name")
                        .to_string_lossy(),
                ),
            )
            .expect("artifact hashes write");
            manifest.push_str(&format!(
                "- `{stem}`: canvas={}x{}, source={}, channel_seeds={channel_seed_summary}, painter_order_differs_from_dependency={}.\n",
                evaluated.result.raster().width(),
                evaluated.result.raster().height(),
                source_path
                    .strip_prefix(&root)
                    .unwrap_or(&source_path)
                    .display(),
                painter_order_differs_from_dependency,
            ));
        }
        let filter_session = modeled_stage20r_composite_session(true);
        let filter_definition =
            &filter_session.document().pattern_definition_bundles()[0].definition;
        let filter_plan = toniator_patterns::resolve_document_pattern_pipeline(
            filter_session.document(),
            filter_definition,
        )
        .expect("filter evidence plan resolves");
        let filter_evaluated = evaluate_cached_document(
            document_request(&filter_session, valid_document_bytes()),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &NeverCancelled,
        )
        .expect("filter evidence evaluates");
        let connection_usage = filter_evaluated
            .transaction
            .realizations
            .iter()
            .find(|(_, output)| output.output_layer_id == PatternOutputLayerId(122))
            .map(|(_, output)| output.usage.clone())
            .expect("connection usage exists");
        let residual_usage = filter_evaluated
            .transaction
            .realizations
            .iter()
            .find(|(_, output)| output.output_layer_id == PatternOutputLayerId(123))
            .map(|(_, output)| output.usage.clone())
            .expect("residual usage exists");
        let swapped_session = modeled_stage20r_composite_session(false);
        let swapped_definition =
            &swapped_session.document().pattern_definition_bundles()[0].definition;
        let swapped_plan = toniator_patterns::resolve_document_pattern_pipeline(
            swapped_session.document(),
            swapped_definition,
        )
        .expect("swapped filter evidence plan resolves");
        fs::write(
            output.join("ordered-filter-semantics-record.txt"),
            format!(
                "visualized=false\nreason=filter semantics are recorded without overpainting the connection witness\nfilter=SitesUnusedBy(PatternOutputLayerId(122))\npainter_order={:?}\nevaluation_order={:?}\nswapped_painter_order={:?}\nswapped_evaluation_order={:?}\nconnection_usage_count={}\nconnection_usage_fingerprint={}\nresidual_usage_count={}\nresidual_usage_fingerprint={}\ndisjoint={}\n",
                filter_plan
                    .ordered_outputs
                    .iter()
                    .map(|output| output.layer_id)
                    .collect::<Vec<_>>(),
                filter_plan.evaluation_order,
                swapped_plan
                    .ordered_outputs
                    .iter()
                    .map(|output| output.layer_id)
                    .collect::<Vec<_>>(),
                swapped_plan.evaluation_order,
                connection_usage.members().len(),
                connection_usage.fingerprint(),
                residual_usage.members().len(),
                residual_usage.fingerprint(),
                connection_usage
                    .members()
                    .iter()
                    .all(|member| !residual_usage.members().contains(member)),
            ),
        )
        .expect("ordered filter semantics record writes");
        manifest.push_str(
            "- `ordered-filter-semantics-record.txt`: nonvisual `SitesUnusedBy`, painter-order swap, evaluation-DAG, usage-identity, and disjointness evidence.\n",
        );
        let mechanism_id = PatternMechanismId(20_701);
        let coowners = vec![
            FamilySiteId {
                mechanism_id,
                ordinal: 3,
            },
            FamilySiteId {
                mechanism_id,
                ordinal: 8,
            },
        ];
        let coowned_regions = build_canonical_regions_cancellable(
            CanonicalRegionProposal {
                output_layer_id: PatternOutputLayerId(20_702),
                source_groups: vec![CanonicalRegionSourceGroup {
                    source_id: CanonicalRegionSourceId::SiteOwners(coowners.clone()),
                    components: vec![
                        CurvePath::polyline(
                            vec![
                                Point2::new(-12.0, -8.0),
                                Point2::new(18.0, -8.0),
                                Point2::new(3.0, 20.0),
                            ],
                            PathClosure::Closed,
                        )
                        .expect("supplemental co-owner region closes"),
                    ],
                }],
            },
            CanonicalRegionLimits::default(),
            || false,
        )
        .expect("supplemental co-owner region canonicalizes")
        .0;
        let coowner_usage = voronoi_region_site_usage(mechanism_id, &coowned_regions)
            .expect("supplemental co-owner usage derives");
        let empty_usage = voronoi_region_site_usage(
            mechanism_id,
            &toniator_patterns::CanonicalRegionSet::empty(),
        )
        .expect("supplemental collapsed usage derives");
        fs::write(
            output.join("supplemental-region-and-clipping-record.txt"),
            format!(
                "duplicate_coowners={:?}\nunique_usage={:?}\nempty_or_collapsed_usage={:?}\ncoowned_region_bounds={:?}\noff_canvas_geometry_preserved_before_final_clip=true\nrenderer_topology_repair=false\n",
                coowners,
                coowner_usage.members(),
                empty_usage.members(),
                coowned_regions
                    .regions()
                    .iter()
                    .map(|region| region.bounds)
                    .collect::<Vec<_>>(),
            ),
        )
        .expect("supplemental Stage 20R record writes");
        manifest.push_str(
            "- `supplemental-region-and-clipping-record.txt`: duplicate co-owner, empty/collapse, off-canvas canonical geometry, and final-clipping authority record.\n",
        );
        fs::write(output.join("MANIFEST.md"), manifest).expect("Stage 20R manifest writes");
    }
}

#[cfg(test)]
mod cache_key_tests {
    use super::*;

    /// Builds one deterministic family key with authoritative density/aspect identity.
    fn family(density_aspect: f64) -> FamilyCacheKey {
        FamilyCacheKey {
            canvas: (900.0_f64.to_bits(), 600.0_f64.to_bits()),
            density: (5_400.0_f64.sqrt().to_bits(), density_aspect.to_bits()),
            rotation: 17.0_f64.to_bits(),
            translation: (3.25_f64.to_bits(), (-4.5_f64).to_bits()),
            guard_steps: 2,
            definition: FamilyDefinitionKey {
                definition_id: 1,
                family: toniator_domain::PatternFamily::GuideIntersections {
                    guide_mechanism_id: toniator_domain::PatternMechanismId(1),
                    site_mechanism_id: toniator_domain::PatternMechanismId(2),
                },
                mechanisms: vec![],
                resolved_guide_content: None,
                path_offset_algorithm: None,
            },
            required_support_radius: 4.5_f64.to_bits(),
            max_family_candidates: EvaluationLimits::DEFAULT_MAX_FAMILY_CANDIDATES,
            structural_source: None,
        }
    }

    /// Proves a lower maximum fill reuses a broad family envelope while a wider request misses.
    #[test]
    fn family_cache_reuses_only_an_envelope_broad_enough_for_normalized_fill() {
        let mut broad = family(1.0);
        broad.required_support_radius = 8.0_f64.to_bits();
        let mut narrow = broad.clone();
        narrow.required_support_radius = 4.0_f64.to_bits();

        assert!(family_key_supports(&broad, &narrow));
        assert!(!family_key_supports(&narrow, &broad));
    }

    /// Proves topology-supporting family envelopes reuse only when the accepted support is broader.
    #[test]
    fn family_cache_key_reuses_broader_topology_envelope_and_rejects_insufficient_one() {
        let base_support = 4.0_f64;
        let guard_steps = 2.0_f64;
        let mut broad = family(1.0);
        broad.required_support_radius = (base_support + guard_steps * 8.0).to_bits();
        let mut requested = broad.clone();
        requested.required_support_radius = (base_support + guard_steps * 5.0).to_bits();
        let mut insufficient = broad.clone();
        insufficient.required_support_radius = (base_support + guard_steps * 3.0).to_bits();

        assert!(family_key_supports(&broad, &requested));
        assert!(!family_key_supports(&insufficient, &requested));
    }

    fn contract(layer_id: u64) -> RealizationContractKey {
        RealizationContractKey {
            output_layers: vec![PatternOutputLayer::all(
                toniator_domain::PatternOutputLayerId(layer_id),
                PatternOutputRealization::CircularMarks {
                    site_mechanism_id: toniator_domain::PatternMechanismId(2),
                },
            )],
            modulation: toniator_domain::PatternModulation,
        }
    }

    fn realization(
        family: FamilyCacheKey,
        content: &str,
        decoded: &str,
        contract: RealizationContractKey,
    ) -> RealizationCacheKey {
        RealizationCacheKey {
            family,
            contract,
            source_identity: RealizationSourceIdentity {
                format: toniator_sampling::SourceFormat::Png,
                width: 900,
                height: 600,
                content_hash: content.to_owned(),
                decoded_pixel_hash: decoded.to_owned(),
            },
            canvas: (900.0_f64.to_bits(), 600.0_f64.to_bits()),
            source_component: 1,
            placement: 1,
            response: (0.25_f64.to_bits(), 1.0_f64.to_bits(), 0.0_f64.to_bits()),
        }
    }

    /// Includes geometry-owned connection algorithm contracts only in connection cache discrimination.
    #[test]
    fn connection_cache_contracts_track_program_kind_and_leave_mark_keys_empty() {
        let mut connection = PatternDefinition::supported_straight_grid(
            toniator_domain::PatternDefinitionId(8),
            "connection",
            toniator_domain::PatternMechanismId(9),
            toniator_domain::PatternMechanismId(10),
            toniator_domain::PatternOutputLayerId(11),
            toniator_domain::CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        connection.output_layers.push(PatternOutputLayer::all(
            toniator_domain::PatternOutputLayerId(12),
            PatternOutputRealization::ConnectionPaths {
                site_mechanism_id: toniator_domain::PatternMechanismId(10),
                program: toniator_domain::ConnectionProgram::RandomLinks {
                    adjacency: toniator_domain::ConnectionAdjacencyIntent {
                        maximum_degree: 2,
                        maximum_distance: 12.0,
                    },
                    minimum_degree: 0,
                    seed: 7,
                },
                style: toniator_domain::PathStrokeStyle::default(),
            },
        ));
        let maze = connection_cache_contracts(
            &connection,
            toniator_domain::PatternOutputLayerId(12),
            SiteAdjacencyLimits::default(),
            ConnectionPathLimits::default(),
        )
        .expect("connection contracts");
        let mut tree = connection.clone();
        let PatternOutputRealization::ConnectionPaths { program, .. } =
            &mut tree.output_layers[1].realization
        else {
            panic!("connection fixture retains its second output")
        };
        *program = toniator_domain::ConnectionProgram::GridSpanningTree {
            adjacency: toniator_domain::ConnectionAdjacencyIntent {
                maximum_degree: 2,
                maximum_distance: 12.0,
            },
            algorithm: toniator_domain::GridSpanningTreeAlgorithm::RandomizedPrim,
            seed: 7,
        };
        assert_ne!(
            maze,
            connection_cache_contracts(
                &tree,
                toniator_domain::PatternOutputLayerId(12),
                SiteAdjacencyLimits::default(),
                ConnectionPathLimits::default(),
            )
            .expect("tree contracts")
        );
        assert_eq!(
            connection_cache_contracts(
                &connection,
                toniator_domain::PatternOutputLayerId(11),
                SiteAdjacencyLimits::default(),
                ConnectionPathLimits::default(),
            ),
            None,
            "the adjacent mark output never inherits connection identity"
        );
        let marks = PatternDefinition::supported_straight_grid(
            toniator_domain::PatternDefinitionId(12),
            "marks",
            toniator_domain::PatternMechanismId(13),
            toniator_domain::PatternMechanismId(14),
            toniator_domain::PatternOutputLayerId(15),
            toniator_domain::CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        assert_eq!(
            connection_cache_contracts(
                &marks,
                toniator_domain::PatternOutputLayerId(15),
                SiteAdjacencyLimits::default(),
                ConnectionPathLimits::default(),
            ),
            None
        );
    }

    /// Keeps maze family identity structural while seed, fixed recursive-backtracker contract, and
    /// bounded maze limits discriminate only the derived realization cache key.
    #[test]
    fn maze_cache_contracts_track_seed_contract_and_limits_without_changing_family_key() {
        let mut definition = PatternDefinition::supported_straight_grid(
            toniator_domain::PatternDefinitionId(21),
            "maze",
            toniator_domain::PatternMechanismId(22),
            toniator_domain::PatternMechanismId(23),
            toniator_domain::PatternOutputLayerId(24),
            toniator_domain::CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        definition.output_layers.push(PatternOutputLayer::all(
            toniator_domain::PatternOutputLayerId(25),
            PatternOutputRealization::MazeWalls {
                site_mechanism_id: toniator_domain::PatternMechanismId(23),
                program: toniator_domain::MazeProgram {
                    algorithm: toniator_domain::GridMazeAlgorithm::RecursiveBacktracker,
                    seed: 7,
                },
                style: toniator_domain::PathStrokeStyle::default(),
            },
        ));
        let baseline_limits = MazeLimits::default();
        let baseline = maze_cache_contracts(
            &definition,
            toniator_domain::PatternOutputLayerId(25),
            baseline_limits,
        )
        .expect("maze contracts");
        assert_eq!(baseline.arrangement, MAZE_WALL_CONTRACT_ID);
        assert_eq!(
            baseline.algorithm,
            toniator_domain::GridMazeAlgorithm::RecursiveBacktracker,
            "the only current algorithm still participates as an explicit contract field"
        );
        let mut seeded = definition.clone();
        let PatternOutputRealization::MazeWalls { program, .. } =
            &mut seeded.output_layers[1].realization
        else {
            panic!("maze fixture retains its second output")
        };
        program.seed = 11;
        assert_ne!(
            baseline,
            maze_cache_contracts(
                &seeded,
                toniator_domain::PatternOutputLayerId(25),
                baseline_limits,
            )
            .expect("seeded maze contracts")
        );
        let changed_limits = MazeLimits {
            maximum_faces: baseline_limits.maximum_faces - 1,
            ..baseline_limits
        };
        assert_ne!(
            baseline,
            maze_cache_contracts(
                &definition,
                toniator_domain::PatternOutputLayerId(25),
                changed_limits,
            )
            .expect("limited maze contracts")
        );
        assert_eq!(
            maze_cache_contracts(
                &definition,
                toniator_domain::PatternOutputLayerId(24),
                baseline_limits,
            ),
            None,
            "the adjacent mark output never inherits maze identity"
        );
        assert_eq!(
            family(1.0),
            family(1.0),
            "maze realization intent never enters the structural family cache key"
        );
    }

    /// Keeps ordinary connected guide/parametric cache identities independent of connection-only limits.
    #[test]
    fn ordinary_connected_response_key_omits_connection_limits_and_contracts() {
        let ordinary = DocumentResponseIdentity::Connected {
            minimum: 0.25_f64.to_bits(),
            maximum: 1.0_f64.to_bits(),
            shape_rotation: 0.0_f64.to_bits(),
            outline_contract: toniator_patterns::CANONICAL_STROKE_OUTLINE_CONTRACT_ID,
            profile_limit: 128,
            outline_segment_limit: 256,
            connection_contracts: Box::new(None),
            maze_contracts: Box::new(None),
        };
        assert_eq!(ordinary, ordinary.clone());
        let connection = ConnectionCacheContracts {
            site_adjacency: SITE_ADJACENCY_CONTRACT_ID,
            path_selection: CONNECTION_PATH_CONTRACT_ID,
            trail_decomposition: CONNECTION_TRAIL_CONTRACT_ID,
            program_selection: connection_program_contract_id(
                &toniator_domain::ConnectionProgram::NearestLinks {
                    adjacency: toniator_domain::ConnectionAdjacencyIntent {
                        maximum_degree: 1,
                        maximum_distance: 1.0,
                    },
                },
            ),
            adjacency_limits: SiteAdjacencyLimits::default(),
            connection_limits: ConnectionPathLimits::default(),
        };
        assert_ne!(
            ordinary,
            DocumentResponseIdentity::Connected {
                minimum: 0.25_f64.to_bits(),
                maximum: 1.0_f64.to_bits(),
                shape_rotation: 0.0_f64.to_bits(),
                outline_contract: toniator_patterns::CANONICAL_STROKE_OUTLINE_CONTRACT_ID,
                profile_limit: 128,
                outline_segment_limit: 256,
                connection_contracts: Box::new(Some(connection)),
                maze_contracts: Box::new(None),
            }
        );
    }

    #[test]
    fn family_key_is_structural_while_decoded_pixels_invalidate_every_downstream_key() {
        let first_family = family(1.0);
        let second_family = family(1.0);
        assert_eq!(first_family, second_family);
        assert_ne!(family(1.0), family(2.0));
        let first_realization = realization(first_family, "content-a", "pixels-a", contract(1));
        let second_realization = realization(second_family, "content-a", "pixels-b", contract(1));
        assert_ne!(first_realization, second_realization);
        assert_ne!(
            first_realization,
            realization(family(1.0), "content-b", "pixels-a", contract(1))
        );
        let scene = |realization: RealizationCacheKey| SceneCacheKey {
            realization,
            canvas: (900.0_f64.to_bits(), 600.0_f64.to_bits()),
            channel_id: 1,
            visible: true,
            color: (0, 0, 0, 1.0_f64.to_bits()),
            opacity: 0.72_f64.to_bits(),
        };
        let first_scene = scene(first_realization);
        let second_scene = scene(second_realization);
        assert_ne!(first_scene, second_scene);
        assert_ne!(
            RasterCacheKey {
                scene: first_scene,
                transparent_raster_contract: TRANSPARENT_RASTER_CONTRACT_ID,
                max_flattened_raster_edges: EvaluationLimits::default()
                    .max_flattened_raster_edges(),
            },
            RasterCacheKey {
                scene: second_scene,
                transparent_raster_contract: TRANSPARENT_RASTER_CONTRACT_ID,
                max_flattened_raster_edges: EvaluationLimits::default()
                    .max_flattened_raster_edges(),
            },
        );
    }

    #[test]
    fn family_definition_id_keys_cached_provenance_but_name_is_presentation_only() {
        let definition = PatternDefinition::supported_straight_grid(
            toniator_domain::PatternDefinitionId(1),
            "first name",
            toniator_domain::PatternMechanismId(1),
            toniator_domain::PatternMechanismId(2),
            toniator_domain::PatternOutputLayerId(1),
            toniator_domain::CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        );
        let mut renamed = definition.clone();
        renamed.name = "presentation-only rename".to_owned();
        let mut different_provenance = definition.clone();
        different_provenance.id = toniator_domain::PatternDefinitionId(2);
        assert_eq!(
            family_definition_key(&definition),
            family_definition_key(&renamed)
        );
        assert_ne!(
            family_definition_key(&definition),
            family_definition_key(&different_provenance)
        );
    }

    #[test]
    fn logical_source_lookup_misses_decode_but_reuses_decoded_realization_identity() {
        let baseline = SourceCacheKey {
            reference_id: "source-a".to_owned(),
            bytes: Arc::<[u8]>::from(vec![1_u8, 2, 3]),
            format: SourceFormatHint::Png,
            decoder_contract: "decoder-a",
        };
        let changed_lookup = SourceCacheKey {
            reference_id: "source-b".to_owned(),
            ..baseline.clone()
        };
        assert_ne!(baseline, changed_lookup);
        assert_eq!(family(1.0), family(1.0));
        // Decode cache lookup deliberately sees the logical reference, while
        // realization consumes only the immutable decoder result.
        assert_eq!(
            realization(family(1.0), "content-a", "pixels-a", contract(1)),
            realization(family(1.0), "content-a", "pixels-a", contract(1)),
        );
        assert_eq!(family(1.0), family(1.0));
        assert_ne!(
            realization(family(1.0), "content", "pixels", contract(1)),
            realization(family(1.0), "content", "pixels", contract(2)),
        );
    }
}
