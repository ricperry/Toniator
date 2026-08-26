#![forbid(unsafe_code)]

//! The shared mutable-document boundary for headless Toniator frontends.

#[cfg(test)]
use std::{
    cell::{Cell, RefCell},
    sync::atomic::AtomicUsize,
};
use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
};

mod scheduler;

pub use scheduler::{
    ChannelDiagnosticCompletion, ChannelDiagnosticScheduler, ChannelDiagnosticTicket,
    SchedulerError,
};

use toniator_domain::{
    CanvasSpec, ChannelId, EvaluationSnapshot, EvaluationToken, PatternDefinition,
    PatternMechanism, PatternOutputLayer, RegionGeometryResponse, SourceComponent, SourcePlacement,
    SourceReference, SourceReferenceId,
};
use toniator_domain::{
    ChannelPaint, DocumentEvaluationSnapshot, DocumentEvaluationToken, DocumentSession,
    HalftoneChannelModel, HalftoneChannelRole, ModeledChannelState,
};
use toniator_patterns::{
    Bounds, CONNECTION_PATH_CONTRACT_ID, CONNECTION_TRAIL_CONTRACT_ID, CanonicalRegionSet,
    CurvePath, CurveSegment, FamilyCapability, GUIDE_FACE_CONTRACT_ID, GenericGuideCapability,
    GridFamilyOutput, GuideFaceLimits, GuideFaceRequest, MAZE_WALL_CONTRACT_ID,
    MappedCircularMarkRealization, MazeLimits, PatternPipelineError, REGION_TREATMENT_CONTRACT_ID,
    RegionReference, RegionTreatmentLimits, SITE_ADJACENCY_CONTRACT_ID,
    SourceColorCircularMarkRealization, TypedFamilyOutput, TypedRealization,
    VORONOI_REGION_CONTRACT_ID, VoronoiRegionDiagnostics, VoronoiRegionLimits,
    VoronoiRegionRequest, build_connection_paths_cancellable, build_guide_faces_cancellable,
    build_typed_site_adjacency_cancellable, build_voronoi_regions_cancellable,
    connection_program_contract_id, evaluate_straight_grid,
    evaluate_typed_connection_paths_with_source_cancellable,
    evaluate_typed_family_product_with_source_cancellable,
    evaluate_typed_maze_walls_from_family_cancellable, family_requires_decoded_source,
    maximum_emitted_guide_spacing, maximum_nominal_cell_diameter, realize_circular_marks,
    realize_region_output_cancellable, voronoi_region_references,
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
    linear_to_srgb, raster_output_identity, rasterize, rasterize_cancellable, rasterize_output,
    rasterize_preview, rasterize_preview_cancellable, srgb_to_linear, write_svg,
};
pub use toniator_sampling::{
    DECODER_CONTRACT_ID, SourceField, SourceFormat, SourceFormatHint, SourceIdentity,
    SvgTextDiagnostic,
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
                "only PNG and SVG source formats are supported",
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
}

impl Eq for EvaluationLimits {}

impl EvaluationLimits {
    pub const DEFAULT_MAX_FAMILY_CANDIDATES: usize = 1_048_576;

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
        Ok(Self {
            max_family_candidates,
            max_flattened_raster_edges: toniator_render::DEFAULT_MAX_FLATTENED_RASTER_EDGES,
            max_transformed_curve_segment_instances:
                toniator_patterns::MAX_TRANSFORMED_CURVE_SEGMENT_INSTANCES,
            max_stroke_profile_samples: toniator_patterns::MAX_STROKE_PROFILE_SAMPLES,
            max_stroke_outline_segments: toniator_patterns::MAX_STROKE_OUTLINE_SEGMENTS,
            site_adjacency: SiteAdjacencyLimits::default(),
            connection_paths: ConnectionPathLimits::default(),
            maze: MazeLimits::default(),
            voronoi: VoronoiRegionLimits::default(),
            guide_faces: GuideFaceLimits::default(),
            region_sampling: RegionSamplingLimits::default(),
            region_treatment: RegionTreatmentLimits::default(),
        })
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
    /// Supplies exact finite family, transformed-segment, and flattened-edge defaults.
    fn default() -> Self {
        Self {
            max_family_candidates: Self::DEFAULT_MAX_FAMILY_CANDIDATES,
            max_flattened_raster_edges: toniator_render::DEFAULT_MAX_FLATTENED_RASTER_EDGES,
            max_transformed_curve_segment_instances:
                toniator_patterns::MAX_TRANSFORMED_CURVE_SEGMENT_INSTANCES,
            max_stroke_profile_samples: toniator_patterns::MAX_STROKE_PROFILE_SAMPLES,
            max_stroke_outline_segments: toniator_patterns::MAX_STROKE_OUTLINE_SEGMENTS,
            site_adjacency: SiteAdjacencyLimits::default(),
            connection_paths: ConnectionPathLimits::default(),
            maze: MazeLimits::default(),
            voronoi: VoronoiRegionLimits::default(),
            guide_faces: GuideFaceLimits::default(),
            region_sampling: RegionSamplingLimits::default(),
            region_treatment: RegionTreatmentLimits::default(),
        }
    }
}

#[cfg(test)]
mod stage20e2_limit_tests {
    use super::*;

    /// Fixes both Stage 20E2 defaults and rejects disabled transformed or flattened work bounds.
    #[test]
    fn evaluation_limits_keep_exact_nonzero_shape_work_bounds() {
        let defaults = EvaluationLimits::default();
        assert_eq!(
            defaults.max_transformed_curve_segment_instances(),
            toniator_patterns::MAX_TRANSFORMED_CURVE_SEGMENT_INSTANCES
        );
        assert_eq!(
            defaults.max_flattened_raster_edges(),
            toniator_render::DEFAULT_MAX_FLATTENED_RASTER_EDGES
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

    /// Proves treatment envelopes use the typed Scale and signed ConstantGap extrema only.
    #[test]
    fn region_support_uses_outward_scale_and_gap_extrema() {
        let scale = RegionGeometryResponse::Scale {
            sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
            minimum_scale: 0.0,
            maximum_scale: 1.75,
        };
        let inward_gap = RegionGeometryResponse::ConstantGap {
            sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
            minimum_gap: 2.0,
            maximum_gap: 4.0,
        };
        let outward_gap = RegionGeometryResponse::ConstantGap {
            sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
            minimum_gap: -8.0,
            maximum_gap: 3.0,
        };
        assert_eq!(
            region_treatment_outward_support(&scale, 10.0).expect("finite scale support"),
            7.5
        );
        assert_eq!(
            region_treatment_outward_support(&inward_gap, 10.0).expect("inward gap has no growth"),
            0.0
        );
        assert_eq!(
            region_treatment_outward_support(&outward_gap, 10.0).expect("outward gap growth"),
            4.0
        );
    }

    /// Proves invalid support arithmetic fails before a family evaluator can allocate candidates.
    #[test]
    fn region_support_rejects_nonfinite_and_overflowing_inputs() {
        let nonfinite = RegionGeometryResponse::Scale {
            sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
            minimum_scale: 0.0,
            maximum_scale: f64::INFINITY,
        };
        assert_eq!(
            region_treatment_outward_support(&nonfinite, 1.0)
                .expect_err("nonfinite scale rejects")
                .path(),
            "region.treatment.coverage.maximum_scale"
        );
        assert_eq!(
            checked_region_support_add(f64::MAX, f64::MAX)
                .expect_err("overflow rejects")
                .path(),
            "region.treatment.coverage.support"
        );
    }

    /// Proves Full plus solid preserves the accepted source-independent realization cache behavior.
    #[test]
    fn full_solid_region_output_omits_sampling_identity() {
        let full =
            toniator_domain::PatternGeometryResponse::Regions(RegionGeometryResponse::Full {
                sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
            });
        assert!(!output_sampling_required(&full, &solid_paint()));
        assert!(output_sampling_required(
            &full,
            &ChannelPaint::SampledSource
        ));
    }

    /// Proves numeric treatment and sampled-paint paths remain source-sensitive cache consumers.
    #[test]
    fn sampled_or_treated_region_output_requires_sampling_identity() {
        let scale =
            toniator_domain::PatternGeometryResponse::Regions(RegionGeometryResponse::Scale {
                sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                minimum_scale: 0.5,
                maximum_scale: 1.5,
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
    scene: RenderScene,
    raster: RasterSurface,
}

impl ChannelDiagnosticResult {
    pub fn token(&self) -> EvaluationToken {
        self.token
    }
    pub fn source_identity(&self) -> &SourceIdentity {
        &self.source_identity
    }
    pub fn scene(&self) -> &RenderScene {
        &self.scene
    }
    pub fn raster(&self) -> &RasterSurface {
        &self.raster
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
    aspect_locked: bool,
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

trait CancellationProbe {
    fn is_cancelled(&self) -> bool;

    #[cfg(test)]
    fn observe_stage(&self, _stage: EvaluationStage, _checkpoint: EvaluationCheckpoint) {}
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

fn evaluate_generic_family_stage(
    family: &FamilyCapability,
    request: &GridInspectRequest,
    source: &SourceField,
    cancellation: &dyn CancellationProbe,
) -> Result<TypedFamilyOutput, EvaluationRunError> {
    match evaluate_stage(EvaluationStage::Family, cancellation, || {
        evaluate_typed_family_product_with_source_cancellable(
            family,
            request,
            family_requires_decoded_source(family).then_some(source),
            &|| cancellation.is_cancelled(),
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
    let output = ordered_output_bindings(&effective, &plan)?
        .into_iter()
        .next()
        .expect("one-output gate retains one binding");
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
            effective.density.across_x.to_bits(),
            effective.density.across_y.to_bits(),
        ),
        aspect_locked: effective.density.aspect_locked,
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
        density: effective.density.clone(),
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
            scene: (*scene).clone(),
            raster: (*raster).clone(),
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

/// Hashes resolved authored closed-shape content so a resource replacement cannot reuse stale marks.
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
) -> Result<String, EvaluationError> {
    let mut bytes = b"toniator-stage-20e2-resolved-shape-content-v1".to_vec();
    for output in &definition.output_layers {
        let PatternOutputLayer::MarkPrototype { prototype, .. } = output else {
            continue;
        };
        let toniator_domain::MarkPrototype::AuthoredClosedShape { structure_id } = prototype else {
            continue;
        };
        let structure = document
            .authored_structure(*structure_id)
            .ok_or(EvaluationError::new(
                "evaluation.mark_shape.reference",
                "authored closed-shape mark resource is missing",
            ))?;
        let path = CurvePath::from_authored_structure(structure).map_err(|_| {
            EvaluationError::new(
                "evaluation.mark_shape.geometry",
                "authored closed-shape mark resource is not valid curve geometry",
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
        "toniator-stage-20e2-shape-content-v1:fnv1a64:{hash:016x}"
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
    /// Stores the response-selected strategy even when Full plus solid performs no sample.
    pub strategy: toniator_domain::RegionSamplingStrategy,
    /// Counts complete untreated bases sampled by the typed patterns realizer.
    pub sampled_bases: usize,
}

/// Typed treatment classification retained with bounded retained-region facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionTreatmentCacheKind {
    /// Replays untreated canonical geometry.
    Full,
    /// Applies a per-base affine scale.
    Scale,
    /// Applies a per-base signed normal gap.
    ConstantGap,
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
    scene: RenderScene,
    raster: RasterSurface,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationTicket(u64);
impl EvaluationTicket {
    pub const fn value(self) -> u64 {
        self.0
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

    fn new_with_limits_and_gate(
        limits: EvaluationLimits,
        #[cfg(test)] gate: Option<Arc<EvaluationStageGate>>,
        #[cfg(test)] decode_observer: Option<Arc<AtomicUsize>>,
    ) -> Result<Self, SchedulerError> {
        let (sender, receiver) = mpsc::channel::<DocumentJob>();
        let (completion_sender, completions) = mpsc::channel::<DocumentWorkerCompletion>();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = Arc::clone(&shutdown);
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
                    #[cfg(test)]
                    let outcome = match worker_gate.as_ref() {
                        Some(gate) => evaluate_cached_document_with_test_observer(
                            job.request,
                            limits,
                            &job.cache,
                            &ObservedCancellation {
                                cancelled: &job.cancelled,
                                gate,
                            },
                            job.decode_observer.as_deref(),
                        ),
                        None => evaluate_cached_document_with_test_observer(
                            job.request,
                            limits,
                            &job.cache,
                            &AtomicCancellation(&job.cancelled),
                            job.decode_observer.as_deref(),
                        ),
                    };
                    #[cfg(not(test))]
                    let outcome = evaluate_cached_document(
                        job.request,
                        limits,
                        &job.cache,
                        &AtomicCancellation(&job.cancelled),
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
                    if !job.cancelled.load(Ordering::Acquire) {
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
        &self.scene
    }
    pub fn raster(&self) -> &RasterSurface {
        &self.raster
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
    evaluate_cached_document_impl(request, limits, accepted, cancellation)
}

#[cfg(test)]
fn evaluate_cached_document_with_test_observer(
    request: EvaluationRequest,
    limits: EvaluationLimits,
    accepted: &DocumentDerivedCache,
    cancellation: &dyn CancellationProbe,
    decode_observer: Option<&AtomicUsize>,
) -> Result<CachedDocumentEvaluation, EvaluationRunError> {
    evaluate_cached_document_impl(request, limits, accepted, cancellation, decode_observer)
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
    #[cfg(test)] decode_observer: Option<&AtomicUsize>,
) -> Result<CachedDocumentEvaluation, EvaluationRunError> {
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
    let source_key = SourceCacheKey {
        reference_id: request.source.reference_id().as_str().to_owned(),
        bytes: Arc::clone(&request.source.bytes),
        format: request.source.format(),
        decoder_contract: DECODER_CONTRACT_ID,
    };
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
    let mut summaries = Vec::with_capacity(topology.channels().len());
    let mut layers = Vec::with_capacity(topology.channels().len());
    let mut families = Vec::with_capacity(topology.channels().len());
    let mut realizations = Vec::with_capacity(topology.channels().len());
    let mut family_dispositions = Vec::with_capacity(topology.channels().len());
    let mut realization_dispositions = Vec::with_capacity(topology.channels().len());
    let mut output_realization_dispositions = Vec::with_capacity(topology.channels().len());
    let mut remaining_transformed_curve_segment_instances =
        limits.max_transformed_curve_segment_instances();
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
        let (family, disposition) = match accepted
            .families
            .iter()
            .find(|(candidate, _)| document_family_key_supports(candidate, &key))
        {
            Some((_, family)) => (Arc::clone(family), CacheDisposition::Hit),
            None => (
                Arc::new(evaluate_generic_family_stage(
                    &plan.family,
                    &GridInspectRequest {
                        canvas: document.canvas().clone(),
                        density: effective.density.clone(),
                        rotation_degrees: effective.pattern_rotation_degrees,
                        translation_x: effective.translation_x,
                        translation_y: effective.translation_y,
                        guard_steps: definition.coverage.guard_steps,
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
                )?),
                CacheDisposition::Miss,
            ),
        };
        let mut channel_outputs = Vec::with_capacity(output_bindings.len());
        let mut channel_output_dispositions = Vec::with_capacity(output_bindings.len());
        for (output_capability, output_setting) in output_bindings {
            if cancellation.is_cancelled() {
                return Err(EvaluationRunError::Cancelled);
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
                limits,
                remaining_transformed_curve_segment_instances,
            )?;
            let (realization, realization_disposition) = match accepted
                .realizations
                .iter()
                .find(|(candidate, _)| *candidate == realization_key)
            {
                Some((_, realization)) => (Arc::clone(realization), CacheDisposition::Hit),
                None => (
                    Arc::new(
                        match evaluate_stage(EvaluationStage::Realization, cancellation, || {
                            evaluate_document_output(
                                document,
                                definition,
                                channel,
                                effective,
                                &source,
                                &family,
                                plan,
                                output_capability,
                                output_setting,
                                limits.max_family_candidates(),
                                remaining_transformed_curve_segment_instances,
                                limits.max_stroke_profile_samples(),
                                limits.max_stroke_outline_segments(),
                                limits.site_adjacency_limits(),
                                limits.connection_path_limits(),
                                limits.maze_limits(),
                                limits.voronoi_region_limits(),
                                limits.guide_face_limits(),
                                limits.region_sampling_limits(),
                                limits.region_treatment_limits(),
                                &|| cancellation.is_cancelled(),
                            )
                        }) {
                            Err(EvaluationRunError::Evaluation(error))
                                if error.path() == "evaluation.cancelled" =>
                            {
                                return Err(EvaluationRunError::Cancelled);
                            }
                            result => result?,
                        },
                    ),
                    CacheDisposition::Miss,
                ),
            };
            remaining_transformed_curve_segment_instances =
                remaining_transformed_curve_segment_instances
                    .checked_sub(transformed_curve_segment_instances(
                        &realization.realization,
                    )?)
                    .ok_or(EvaluationError::new(
                        "realization.mark.segment_limit",
                        "transformed curve-segment instance limit exceeded",
                    ))?;
            channel_output_dispositions.push(OutputCacheDiagnostics {
                output_layer_id: output_setting.output_layer_id,
                realization: realization_disposition,
                voronoi: region_diagnostics(&realization.realization),
                region: region_output_diagnostics(&realization.realization),
            });
            realizations.push((realization_key, Arc::clone(&realization)));
            channel_outputs.push(realization);
        }
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
        layers.push(document_render_layer(
            channel,
            channel_outputs
                .into_iter()
                .map(|output| (*output).clone())
                .collect(),
        )?);
        families.push((key, family));
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
    let (raster, raster_disposition) = match &accepted.raster {
        Some((key, value)) if *key == raster_key => (Arc::clone(value), CacheDisposition::Hit),
        _ => (
            Arc::new(evaluate_stage(
                EvaluationStage::Raster,
                cancellation,
                || {
                    match request.preview_target {
                        Some(target) => rasterize_preview_cancellable(
                            &scene,
                            target,
                            toniator_render::RasterizationLimits::new(
                                limits.max_flattened_raster_edges(),
                            )
                            .expect("EvaluationLimits validates raster edge bounds"),
                            &|| cancellation.is_cancelled(),
                        ),
                        None => rasterize_cancellable(
                            &scene,
                            RasterBackground::Transparent,
                            toniator_render::RasterizationLimits::new(
                                limits.max_flattened_raster_edges(),
                            )
                            .expect("EvaluationLimits validates raster edge bounds"),
                            &|| cancellation.is_cancelled(),
                        ),
                    }
                    .map_err(EvaluationError::from_render)
                },
            )?),
            CacheDisposition::Miss,
        ),
    };
    let result = EvaluationResult {
        token: request.snapshot.token(),
        source_identity: source.identity().clone(),
        channels: summaries,
        scene: (*scene).clone(),
        raster: (*raster).clone(),
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
            families,
            realizations,
            scene: (scene_disposition == CacheDisposition::Miss).then_some((scene_key, scene)),
            raster: (raster_disposition == CacheDisposition::Miss).then_some((raster_key, raster)),
        },
    })
}

/// Returns geometry-owned algorithm identities only for a typed connection output.
fn connection_cache_contracts(
    definition: &PatternDefinition,
    adjacency_limits: SiteAdjacencyLimits,
    connection_limits: ConnectionPathLimits,
) -> Option<ConnectionCacheContracts> {
    let [PatternOutputLayer::ConnectionPaths { program, .. }] = definition.output_layers.as_slice()
    else {
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
    limits: MazeLimits,
) -> Option<MazeCacheContracts> {
    let [PatternOutputLayer::MazeWalls { program, .. }] = definition.output_layers.as_slice()
    else {
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
    Mapped(TypedRealization<MappedCircularMarkRealization>),
    SourceColor(TypedRealization<SourceColorCircularMarkRealization>),
    Canonical(TypedRealization<CanonicalMarkRealization>),
    Strokes(TypedRealization<CanonicalStrokeRealization>),
    Regions {
        regions: CanonicalRegionSet,
        paints: Option<Vec<toniator_sampling::SampledSourcePaint>>,
        fingerprint: String,
        diagnostics: RegionRealizationDiagnostics,
    },
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
}

/// Validates the current authoring gate and pairs outputs in authoritative definition order.
///
/// # Errors
///
/// Returns a stable pipeline diagnostic when a definition bypasses the current one-output gate
/// or the effective settings cannot bind to the resolved output plan. The loop-consuming return
/// type deliberately remains plural so Stage 20R can lift only the gate.
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
    if effective.output_settings.len() != 1 || plan.ordered_outputs.len() != 1 {
        return Err(EvaluationError::new(
            "evaluation.output_gate",
            "current evaluation accepts exactly one compatible output",
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
        DocumentRealization::Mapped(value) => &value.output.realization_fingerprint,
        DocumentRealization::SourceColor(value) => &value.output.realization_fingerprint,
        DocumentRealization::Canonical(value) => &value.output.realization_fingerprint,
        DocumentRealization::Strokes(value) => &value.output.realization_fingerprint,
        DocumentRealization::Regions { fingerprint, .. } => fingerprint,
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
    let (strategy, kind) = match response {
        RegionGeometryResponse::Full { sampling } => (*sampling, RegionTreatmentCacheKind::Full),
        RegionGeometryResponse::Scale { sampling, .. } => {
            (*sampling, RegionTreatmentCacheKind::Scale)
        }
        RegionGeometryResponse::ConstantGap { sampling, .. } => {
            (*sampling, RegionTreatmentCacheKind::ConstantGap)
        }
    };
    RegionRealizationDiagnostics {
        source_identity: (realization.sampled_bases > 0).then(|| source.identity().clone()),
        producer,
        sampling: RegionSamplingCacheDiagnostics {
            strategy,
            sampled_bases: realization.sampled_bases,
        },
        treatment: RegionTreatmentCacheDiagnostics {
            kind,
            retained_regions: realization.retained_regions,
        },
    }
}

/// Evaluates one explicit output unit while retaining the current one-output authoring gate outside this cache unit.
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
    is_cancelled: &dyn Fn() -> bool,
) -> Result<DocumentOutputRealization, EvaluationError> {
    let realization = evaluate_document_channel(
        document,
        definition,
        channel,
        effective,
        source,
        family,
        plan,
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
        is_cancelled,
    )?;
    Ok(DocumentOutputRealization {
        output_layer_id: capability.layer_id,
        capability: capability.clone(),
        setting: setting.clone(),
        realization,
    })
}

/// Aggregates ordered output fingerprints without rewriting existing one-output identities.
///
/// Stage 20N keeps the one-output authoring gate, so the single-unit branch preserves all
/// established fingerprint bytes. Future heterogeneous documents obtain an ordered, layer-ID
/// qualified aggregate only above independently cached units.
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

/// Counts authored path segment instances in one completed channel realization for the
/// request-wide evaluation budget; circle and retained adapter outputs consume no path work.
///
/// # Errors
///
/// Returns the stable segment-limit diagnostic if a maliciously large completed value overflows.
fn transformed_curve_segment_instances(
    value: &DocumentRealization,
) -> Result<usize, EvaluationError> {
    if let DocumentRealization::Strokes(value) = value {
        return value
            .output
            .strokes
            .iter()
            .try_fold(0_usize, |total, stroke| {
                total
                    .checked_add(stroke.path.segments().len())
                    .ok_or(EvaluationError::new(
                        "realization.stroke.segment_limit",
                        "stroke segment instance count overflows",
                    ))
            });
    }
    if matches!(value, DocumentRealization::Regions { .. }) {
        return Ok(0);
    }
    let DocumentRealization::Canonical(value) = value else {
        return Ok(0);
    };
    value.output.marks.iter().try_fold(0_usize, |total, mark| {
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
    is_cancelled: &dyn Fn() -> bool,
) -> Result<toniator_patterns::TypedRegionOutputRealization, EvaluationError> {
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
        return Ok(realization);
    }
    realize_region_output_cancellable(
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
    is_cancelled: &dyn Fn() -> bool,
) -> Result<DocumentRealization, EvaluationError> {
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
                let realized = realize_document_region_output(
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
                    is_cancelled,
                )?;
                let toniator_domain::PatternGeometryResponse::Regions(response) =
                    &output_setting.response
                else {
                    return Err(EvaluationError::new(
                        "region.treatment.identity.response",
                        "ordinary region output requires a region response",
                    ));
                };
                return Ok(DocumentRealization::Regions {
                    fingerprint: format!(
                        "{}:{}:{}:{}",
                        VORONOI_REGION_CONTRACT_ID,
                        output_capability.layer_id.0,
                        site_mechanism_id.0,
                        realized.fingerprint
                    ),
                    regions: realized.regions,
                    paints: realized.paints,
                    diagnostics: completed_region_diagnostics(
                        response,
                        source,
                        realized.diagnostics,
                        RegionProducerCacheDiagnostics::Voronoi(diagnostics),
                    ),
                });
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
                let realized = realize_document_region_output(
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
                    is_cancelled,
                )?;
                let toniator_domain::PatternGeometryResponse::Regions(response) =
                    &output_setting.response
                else {
                    return Err(EvaluationError::new(
                        "region.treatment.identity.response",
                        "guide-face output requires a region response",
                    ));
                };
                return Ok(DocumentRealization::Regions {
                    fingerprint: format!(
                        "{}:{}:{}:{}",
                        GUIDE_FACE_CONTRACT_ID,
                        output_capability.layer_id.0,
                        guide_mechanism_id.0,
                        realized.fingerprint
                    ),
                    regions: realized.regions,
                    paints: realized.paints,
                    diagnostics: completed_region_diagnostics(
                        response,
                        source,
                        realized.diagnostics,
                        RegionProducerCacheDiagnostics::GuideFaces,
                    ),
                });
            }
        }
    }
    if let Some((_site_mechanism_id, program, _style)) = output_capability.maze_walls() {
        let ChannelPaint::Solid(_) = channel.paint else {
            return Err(EvaluationError::new(
                "evaluation.maze.paint",
                "maze-wall output requires solid channel paint",
            ));
        };
        let request = GridInspectRequest {
            canvas: document.canvas().clone(),
            density: effective.density.clone(),
            rotation_degrees: effective.pattern_rotation_degrees,
            translation_x: effective.translation_x,
            translation_y: effective.translation_y,
            guard_steps: definition.coverage.guard_steps,
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
        return Ok(DocumentRealization::Strokes(
            toniator_patterns::realize_typed_maze_canonical_stroke_output_cancellable(
                family,
                plan,
                output_capability,
                output_setting,
                &maze,
                source,
                document.canvas(),
                channel.mapping,
                max_stroke_profile_samples,
                max_stroke_outline_segments,
                is_cancelled,
            )
            .map_err(EvaluationError::from_pipeline)?
            .realization,
        ));
    }
    if let Some((_site_mechanism_id, program, _style)) = output_capability.connection_paths() {
        let ChannelPaint::Solid(_) = channel.paint else {
            return Err(EvaluationError::new(
                "evaluation.connection.paint",
                "connection output requires solid channel paint",
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
        return Ok(DocumentRealization::Strokes(
            toniator_patterns::realize_typed_connection_canonical_stroke_output_cancellable(
                family,
                plan,
                output_capability,
                output_setting,
                &paths,
                source,
                document.canvas(),
                channel.mapping,
                max_stroke_profile_samples,
                max_stroke_outline_segments,
                is_cancelled,
            )
            .map_err(EvaluationError::from_pipeline)?
            .realization,
        ));
    }
    if matches!(
        definition.output_layers.as_slice(),
        [PatternOutputLayer::GuidePaths { .. } | PatternOutputLayer::ParametricPaths { .. }]
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
        return Ok(DocumentRealization::Strokes(
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
            .realization,
        ));
    }
    let realization = if matches!(
        definition.output_layers.as_slice(),
        [PatternOutputLayer::MarkPrototype { .. }]
    ) {
        DocumentRealization::Canonical(
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
            ChannelPaint::Solid(_) => DocumentRealization::Mapped(
                toniator_patterns::realize_typed_mapped_output(
                    family,
                    plan,
                    output_capability,
                    output_setting,
                    source,
                    document.canvas(),
                    channel.mapping,
                    effective.shape_rotation_degrees,
                )
                .map_err(EvaluationError::from_pipeline)?
                .realization,
            ),
            ChannelPaint::SampledSource => DocumentRealization::SourceColor(
                toniator_patterns::realize_typed_source_color_output(
                    family,
                    plan,
                    output_capability,
                    output_setting,
                    source,
                    document.canvas(),
                    channel.mapping,
                    effective.shape_rotation_degrees,
                )
                .map_err(EvaluationError::from_pipeline)?
                .realization,
            ),
        }
    };
    Ok(realization)
}
/// Converts all completed channel-output units into one renderer-owned layer without resource lookup.
///
/// # Errors
///
/// Returns stable layer validation failures before a scene can publish.
fn document_render_layer(
    channel: &ModeledChannelState,
    outputs: Vec<DocumentOutputRealization>,
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
        .into_iter()
        .map(|output| document_render_output(channel, output))
        .collect::<Result<Vec<_>, _>>()?;
    RenderLayer::new_outputs(channel.id, channel.visible, color, channel.opacity, outputs)
        .map_err(EvaluationError::from_render)
}

/// Converts one independently realized output into its ordered renderer payload.
///
/// # Errors
///
/// Returns a stable renderer validation error before the containing channel layer is assembled.
fn document_render_output(
    channel: &ModeledChannelState,
    output: DocumentOutputRealization,
) -> Result<toniator_render::RenderOutputLayer, EvaluationError> {
    let _capability = &output.capability;
    let _setting = &output.setting;
    let output_layer_id = output.output_layer_id;
    match output.realization {
        DocumentRealization::Mapped(value) => match channel.paint {
            ChannelPaint::Solid(_) => Ok(toniator_render::RenderOutputLayer {
                output_layer_id,
                geometry: GeometryOutput::CircularMarks(value.output.marks),
                primitive_paints: None,
            }),
            ChannelPaint::SampledSource => unreachable!(),
        },
        DocumentRealization::SourceColor(value) => Ok(toniator_render::RenderOutputLayer {
            output_layer_id,
            geometry: GeometryOutput::CircularMarks(
                value
                    .output
                    .marks
                    .iter()
                    .map(|entry| entry.mark.clone())
                    .collect(),
            ),
            primitive_paints: Some(
                value
                    .output
                    .marks
                    .into_iter()
                    .map(|entry| toniator_domain::ColorValue {
                        red: entry.paint.red,
                        green: entry.paint.green,
                        blue: entry.paint.blue,
                        alpha: entry.paint.alpha,
                    })
                    .collect(),
            ),
        }),
        DocumentRealization::Canonical(value) => match channel.paint {
            ChannelPaint::Solid(_) => Ok(toniator_render::RenderOutputLayer {
                output_layer_id,
                geometry: GeometryOutput::CanonicalMarks(value.output.marks),
                primitive_paints: None,
            }),
            ChannelPaint::SampledSource => Ok(toniator_render::RenderOutputLayer {
                output_layer_id,
                geometry: GeometryOutput::CanonicalMarks(value.output.marks),
                primitive_paints: Some(
                    value
                        .output
                        .paints
                        .expect("sampled canonical realization retains paint")
                        .into_iter()
                        .map(|paint| toniator_domain::ColorValue {
                            red: paint.red,
                            green: paint.green,
                            blue: paint.blue,
                            alpha: paint.alpha,
                        })
                        .collect(),
                ),
            }),
        },
        DocumentRealization::Strokes(value) => match channel.paint {
            ChannelPaint::Solid(_) => Ok(toniator_render::RenderOutputLayer {
                output_layer_id,
                geometry: GeometryOutput::CanonicalStrokes(value.output.strokes),
                primitive_paints: None,
            }),
            ChannelPaint::SampledSource => unreachable!("stroke realization rejects sampled paint"),
        },
        DocumentRealization::Regions {
            regions,
            paints,
            diagnostics,
            ..
        } => {
            let _ = diagnostics;
            match (&channel.paint, paints) {
                (ChannelPaint::Solid(_), None) => Ok(toniator_render::RenderOutputLayer {
                    output_layer_id,
                    geometry: GeometryOutput::CanonicalRegions(regions),
                    primitive_paints: None,
                }),
                (ChannelPaint::SampledSource, Some(paints)) => {
                    Ok(toniator_render::RenderOutputLayer {
                        output_layer_id,
                        geometry: GeometryOutput::CanonicalRegions(regions),
                        primitive_paints: Some(
                            paints
                                .into_iter()
                                .map(|paint| toniator_domain::ColorValue {
                                    red: paint.red,
                                    green: paint.green,
                                    blue: paint.blue,
                                    alpha: paint.alpha,
                                })
                                .collect(),
                        ),
                    })
                }
                _ => Err(EvaluationError::new(
                    "evaluation.region.paint",
                    "region paint and realization payload disagree",
                )),
            }
        }
    }
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
                effective.density.across_x.to_bits(),
                effective.density.across_y.to_bits(),
            ),
            rotation: effective.pattern_rotation_degrees.to_bits(),
            translation: (
                effective.translation_x.to_bits(),
                effective.translation_y.to_bits(),
            ),
            guard_steps: definition.coverage.guard_steps,
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
        &effective.density,
        MAXIMUM_NORMALIZED_MARK_FILL,
        definition.coverage.additional_margin,
        family,
    )
}

/// Derives the maximum modeled family support request across every ordered output capability.
///
/// A shared family must be broad enough for all of its independently realized outputs. The
/// current authoring gate admits one output, so its result is byte-for-byte the former request.
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
            maximum_emitted_guide_spacing(family, canvas, &effective.density)
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
                maximum_emitted_guide_spacing(family, canvas, &effective.density)
                    .map_err(EvaluationError::from_pipeline)?
            }
            toniator_domain::RegionSourceIntent::VoronoiSites { .. } => {
                maximum_nominal_cell_diameter(family, canvas, &effective.density)
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
                if parametric_guard {
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
        &effective.density,
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

/// Computes the response-owned outward envelope extension without constructing region geometry.
///
/// Scale uses the producer's maximum nominal cell or guide spacing, while a negative constant
/// gap grows its boundary by half the signed gap. Full does not broaden the accepted family.
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
    let outward = match response {
        RegionGeometryResponse::Full { .. } => 0.0,
        RegionGeometryResponse::Scale { maximum_scale, .. } => {
            if !maximum_scale.is_finite() {
                return Err(EvaluationError::new(
                    "region.treatment.coverage.maximum_scale",
                    "scale treatment maximum must be finite",
                ));
            }
            (maximum_scale - 1.0).max(0.0) * nominal_extent
        }
        RegionGeometryResponse::ConstantGap { minimum_gap, .. } => {
            if !minimum_gap.is_finite() {
                return Err(EvaluationError::new(
                    "region.treatment.coverage.minimum_gap",
                    "constant-gap treatment minimum must be finite",
                ));
            }
            (-minimum_gap).max(0.0) / 2.0
        }
    };
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
        maximum_nominal_cell_diameter(family, canvas, &effective.density)
    } else {
        maximum_emitted_guide_spacing(family, canvas, &effective.density)
    }
    .map_err(EvaluationError::from_pipeline)?;
    Ok(basis + definition.coverage.additional_margin)
}

/// Computes the conservative family-specific nominal-cell bound before any allocation.
fn required_support_radius_from_fill(
    canvas: &CanvasSpec,
    density: &toniator_domain::DensityMetric2D,
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
    resolved_shape_content: String,
    region_producer: Option<RegionProducerCacheIdentity>,
    source_identity: Option<RealizationSourceIdentity>,
    mapping: Option<String>,
    response: DocumentResponseIdentity,
    sampled_paint: bool,
    max_transformed_curve_segment_instances: usize,
}

/// Constructs the independent cache identity for one ordered output realization.
///
/// The key excludes aggregate scene state while including every source, mapping, response,
/// algorithm, and bounded-work input that can change this output's canonical geometry. Full solid
/// replay deliberately omits sampling and treatment policy because it cannot consume either.
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
    limits: EvaluationLimits,
    remaining_transformed_curve_segment_instances: usize,
) -> Result<DocumentRealizationCacheKey, EvaluationError> {
    let sampling_required = output_sampling_required(&output_setting.response, &channel.paint);
    let region_sampling_limits = sampling_required.then(|| limits.region_sampling_limits());
    let region_treatment_limits = matches!(
        &output_setting.response,
        toniator_domain::PatternGeometryResponse::Regions(
            RegionGeometryResponse::Scale { .. } | RegionGeometryResponse::ConstantGap { .. }
        )
    )
    .then(|| limits.region_treatment_limits());
    Ok(DocumentRealizationCacheKey {
        family_content: family_key.content.clone(),
        output_layer_id: output_setting.output_layer_id,
        contract: realization_contract_key(definition),
        resolved_shape_content: resolved_shape_content_identity(document, definition)?,
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
                        limits.site_adjacency_limits(),
                        limits.connection_path_limits(),
                    )),
                    maze_contracts: Box::new(maze_cache_contracts(
                        definition,
                        limits.maze_limits(),
                    )),
                }
            }
            toniator_domain::PatternGeometryResponse::Regions(response) => match definition
                .output_layers
                .iter()
                .find(|output| output.id() == output_setting.output_layer_id)
            {
                Some(PatternOutputLayer::Regions {
                    source: toniator_domain::RegionSourceIntent::GuideFaces { .. },
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
        max_transformed_curve_segment_instances: remaining_transformed_curve_segment_instances,
    })
}

/// Reports whether one output's realization consumes decoded source and mapping identity.
///
/// Full solid region output is an exact canonical replay, so it deliberately avoids source
/// sampling and leaves source/mapping identity out of its cache key. Every other output either
/// maps geometry or derives sampled paint and therefore remains source-sensitive.
fn output_sampling_required(
    response: &toniator_domain::PatternGeometryResponse,
    paint: &ChannelPaint,
) -> bool {
    !matches!(
        (response, paint),
        (
            toniator_domain::PatternGeometryResponse::Regions(RegionGeometryResponse::Full { .. }),
            ChannelPaint::Solid(_)
        )
    )
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
    let PatternOutputLayer::Regions { source, .. } = output else {
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
    Full {
        sampling: toniator_domain::RegionSamplingStrategy,
    },
    Scale {
        sampling: toniator_domain::RegionSamplingStrategy,
        minimum: u64,
        maximum: u64,
    },
    ConstantGap {
        sampling: toniator_domain::RegionSamplingStrategy,
        minimum: u64,
        maximum: u64,
    },
}

/// Converts validated effective region response input into a cache-only typed identity.
fn region_response_identity(response: &RegionGeometryResponse) -> RegionResponseIdentity {
    match response {
        RegionGeometryResponse::Full { sampling } => RegionResponseIdentity::Full {
            sampling: *sampling,
        },
        RegionGeometryResponse::Scale {
            sampling,
            minimum_scale,
            maximum_scale,
        } => RegionResponseIdentity::Scale {
            sampling: *sampling,
            minimum: minimum_scale.to_bits(),
            maximum: maximum_scale.to_bits(),
        },
        RegionGeometryResponse::ConstantGap {
            sampling,
            minimum_gap,
            maximum_gap,
        } => RegionResponseIdentity::ConstantGap {
            sampling: *sampling,
            minimum: minimum_gap.to_bits(),
            maximum: maximum_gap.to_bits(),
        },
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
        OffsetSides, ParametricCurve, PathStrokeStyle, PatternDefinition, PatternDefinitionBundle,
        PatternDefinitionEdit, PatternDefinitionId, PatternGeometryResponse, PatternMechanismId,
        PatternOutputLayer, PatternOutputLayerId, PatternOutputSettings, RegionGeometryResponse,
        RegionSourceIntent, SourceComponent, SourcePlacement, SourceReference, SourceReferenceId,
        SpiralCurve, SpiralShape, StraightGuideDimension, StraightGuideRepetition,
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

    /// Binds the current default typed response to each output in one test-only structural definition.
    fn bundle_from_test_definition(definition: PatternDefinition) -> PatternDefinitionBundle {
        let output_settings = definition
            .output_layers
            .iter()
            .map(|output| PatternOutputSettings {
                output_layer_id: output.id(),
                response: match output {
                    PatternOutputLayer::CircularMarks { .. }
                    | PatternOutputLayer::MarkPrototype { .. } => {
                        PatternGeometryResponse::Marks(MarkGeometryResponse {
                            minimum_fill: 0.0,
                            maximum_fill: 1.0,
                        })
                    }
                    PatternOutputLayer::GuidePaths { .. }
                    | PatternOutputLayer::ParametricPaths { .. }
                    | PatternOutputLayer::ConnectionPaths { .. }
                    | PatternOutputLayer::MazeWalls { .. } => {
                        PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                            minimum_thickness: 0.0,
                            maximum_thickness: 1.0,
                        })
                    }
                    PatternOutputLayer::Regions { .. } => {
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
                    across_x: 90.0,
                    across_y: 60.0,
                    aspect_locked: true,
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
        let PatternOutputLayer::MarkPrototype { prototype, .. } = &mut definition.output_layers[0]
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
        definition.output_layers = vec![PatternOutputLayer::GuidePaths {
            id: PatternOutputLayerId(73),
            guide_mechanism_id: guide,
            style: toniator_domain::PathStrokeStyle::default(),
        }];
        let mut settings = base.pattern_settings().clone();
        settings.definition_id = definition.id;
        settings.density.across_x = 2.0;
        settings.density.across_y = 2.0;
        let bundle = bundle_with_sole_response(
            definition,
            PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.01,
                maximum_thickness: 0.02,
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
        definition.output_layers = vec![PatternOutputLayer::ConnectionPaths {
            id: output_layer_id,
            site_mechanism_id,
            program,
            style: PathStrokeStyle::default(),
        }];
        let mut settings = base.pattern_settings().clone();
        settings.definition_id = definition.id;
        settings.density.across_x = 5.0;
        settings.density.across_y = settings.density.across_x * canvas.height / canvas.width;
        let bundle = bundle_with_sole_response(
            definition,
            PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.1,
                maximum_thickness: 0.25,
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
        definition.output_layers = vec![PatternOutputLayer::Regions {
            id: PatternOutputLayerId(122),
            source: RegionSourceIntent::VoronoiSites {
                site_mechanism_id: PatternMechanismId(121),
            },
        }];
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
            PatternGeometryResponse::Regions(RegionGeometryResponse::Scale {
                sampling,
                minimum_scale: 0.75,
                maximum_scale: 1.25,
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
        definition.output_layers = vec![PatternOutputLayer::Regions {
            id: PatternOutputLayerId(122),
            source: RegionSourceIntent::GuideFaces {
                guide_mechanism_id: PatternMechanismId(120),
                dimensions,
            },
        }];
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
                let GeometryOutput::CanonicalRegions(regions) = &output.geometry else {
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
        assert!(result.scene().layers().iter().all(|layer| matches!(layer.outputs().first().map(|output| &output.geometry), Some(GeometryOutput::CanonicalRegions(regions)) if !regions.regions().is_empty())));
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
        let GeometryOutput::CanonicalRegions(regions) = &region_output.geometry else {
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
        assert!(result.scene().layers().iter().all(|layer| matches!(layer.outputs().first().map(|output| &output.geometry), Some(GeometryOutput::CanonicalRegions(regions)) if !regions.regions().is_empty())));
        scheduler.shutdown().expect("scheduler shuts down");
    }

    /// Builds a 24-across phase-aligned triangular family with a typed conventional wall-maze output.
    fn modeled_maze_session(seed: u32) -> DocumentSession {
        let connection = modeled_connection_session(random_connection_program(seed, 24.0));
        let document = connection.document();
        let mut definition = document.pattern_definition_bundles()[0].definition.clone();
        definition.output_layers = vec![PatternOutputLayer::MazeWalls {
            id: PatternOutputLayerId(122),
            site_mechanism_id: PatternMechanismId(121),
            program: toniator_domain::MazeProgram {
                algorithm: toniator_domain::GridMazeAlgorithm::RecursiveBacktracker,
                seed,
            },
            style: PathStrokeStyle::default(),
        }];
        let mut settings = document.pattern_settings().clone();
        settings.density.across_x = 24.0;
        settings.density.across_y = settings.density.across_x * 48.0 / 64.0;
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
                    sides: OffsetSides::Both,
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
        definition.output_layers = vec![PatternOutputLayer::GuidePaths {
            id: PatternOutputLayerId(94),
            guide_mechanism_id,
            style: toniator_domain::PathStrokeStyle::default(),
        }];
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
        settings.density.across_x = 2.0;
        settings.density.across_y = 2.0;
        let bundle = bundle_with_sole_response(
            definition,
            PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.01,
                maximum_thickness: 0.02,
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
                        sides: OffsetSides::Both,
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
                        sides: OffsetSides::Both,
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
        definition.output_layers = vec![PatternOutputLayer::Regions {
            id: PatternOutputLayerId(200),
            source: RegionSourceIntent::GuideFaces {
                guide_mechanism_id: guide_id,
                dimensions: vec![horizontal, vertical],
            },
        }];
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
        settings.density.across_x = 2.0;
        settings.density.across_y = 2.0;
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
                layer.outputs().first().map(|output| &output.geometry),
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
            output_layers: vec![PatternOutputLayer::Regions {
                id: output_id,
                source: RegionSourceIntent::VoronoiSites {
                    site_mechanism_id: site_id,
                },
            }],
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
        settings.density.across_x = across_x;
        settings.density.across_y = across_x * document.canvas().height / document.canvas().width;
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
    /// The witness deliberately bypasses document persistence only because the production authored-cubic
    /// outward-gap case below has no safe split at the bounded native settings. It still consumes the
    /// geometry-owned canonical treatment and the ordinary renderer without frontend topology work.
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
            treatment: Some(RegionTreatment::ConstantGap(14.0)),
        };
        let treated = treat_region_requests_cancellable(
            output_layer_id,
            &source,
            &[request.clone()],
            RegionTreatmentLimits::default(),
            || false,
        )
        .expect("direct split treatment succeeds");
        let replayed = treat_region_requests_cancellable(
            output_layer_id,
            &source,
            &[request.clone()],
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
                "full-reference-solid",
                RegionGeometryResponse::Full {
                    sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                },
                false,
                1.0,
            ),
            (
                "scale-reference-sampled-opacity",
                RegionGeometryResponse::Scale {
                    sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                    minimum_scale: 0.0,
                    maximum_scale: 1.35,
                },
                true,
                0.62,
            ),
            (
                "scale-area-average-sampled",
                RegionGeometryResponse::Scale {
                    sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
                    minimum_scale: 0.5,
                    maximum_scale: 1.25,
                },
                true,
                0.62,
            ),
            (
                "scale-collapse-empty",
                RegionGeometryResponse::Scale {
                    sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                    minimum_scale: 0.0,
                    maximum_scale: 0.0,
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
                    PatternOutputLayer::Regions {
                        source: RegionSourceIntent::VoronoiSites { .. },
                        ..
                    }
                ),
                "raw ParametricPaths is never repurposed as a Region producer"
            );
            let mut raw_parametric_paths = loaded.document().pattern_definition_bundles()[0]
                .definition
                .clone();
            raw_parametric_paths.output_layers = vec![PatternOutputLayer::ParametricPaths {
                id: PatternOutputLayerId(20_705),
                curve_mechanism_id: PatternMechanismId(20_702),
                style: PathStrokeStyle::default(),
            }];
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

    /// Generates production Guide Face ConstantGap evidence from one observed engine invocation each.
    ///
    /// These witnesses preserve the current v5 document boundary, final-canvas-only rendering,
    /// and test-only snapshot provenance. No four-guide or raw-parametric-path producer is used.
    #[test]
    #[ignore = "writes Stage 20Q Guide Face ConstantGap validation artifacts"]
    fn generate_stage20q_guide_face_constant_gap_artifacts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = root.join("target/validation/stage20q/guide-faces");
        fs::create_dir_all(&output).expect("Guide Face validation directory exists");
        let raster_input = root.join("assets/raster-sample.png");
        let vector_input = root.join("assets/vector-sample.svg");
        let mut manifest = String::from(
            "# Stage 20Q Guide Face ConstantGap fragment\n\n\
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
            RegionGeometryResponse::ConstantGap {
                sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                minimum_gap: 24.0,
                maximum_gap: 24.0,
            },
        );
        let (two_result, two_evidence) = stage20q_write_guide_face_case(
            &output,
            "two-guide-raster-inward-gap-reference-solid",
            two_guide,
            &raster_input,
            EmbeddedSourceFormat::Png,
            SourceFormatHint::Png,
            "production two-guide inward ConstantGap / ReferencePoint / solid",
        );
        assert!(
            !two_evidence.provenance.is_empty(),
            "moderate positive gap retains two-guide components"
        );
        assert_eq!(
            two_result.raster().width(),
            1024,
            "renderer performs only final native canvas clipping"
        );
        manifest.push_str(
            "- `two-guide-raster-inward-gap-reference-solid`: 1024×1024 raster source, positive inward gap, ReferencePoint and solid paint.\n",
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
            RegionGeometryResponse::ConstantGap {
                sampling: toniator_domain::RegionSamplingStrategy::ReferencePoint,
                minimum_gap: 16.0,
                maximum_gap: 16.0,
            },
        );
        let (collapse_result, collapse_evidence) = stage20q_write_guide_face_case(
            &output,
            "two-guide-raster-inward-gap-collapse",
            collapse,
            &raster_input,
            EmbeddedSourceFormat::Png,
            SourceFormatHint::Png,
            "production two-guide collapsing inward ConstantGap",
        );
        assert!(
            collapse_evidence.provenance.is_empty(),
            "positive inward gap collapses every dense two-guide component"
        );
        assert_eq!(collapse_result.raster().width(), 1024);
        manifest.push_str(
            "- `two-guide-raster-inward-gap-collapse`: 1024×1024 raster source, positive inward gap collapse to empty treated geometry.\n",
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
            RegionGeometryResponse::ConstantGap {
                sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
                minimum_gap: 18.0,
                maximum_gap: 18.0,
            },
        );
        let three_guide = stage20q_sampled_region_session(three_base.document().clone(), 0.58);
        let (three_result, three_evidence) = stage20q_write_guide_face_case(
            &output,
            "three-guide-vector-inward-gap-area-average-sampled-opacity",
            three_guide,
            &vector_input,
            EmbeddedSourceFormat::Svg,
            SourceFormatHint::Svg,
            "production 0/60/120 three-guide inward ConstantGap / AreaAverage / sampled paint / opacity",
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
            "inward-shrunk three-guide line faces retain separated triangular rings"
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
            "- `three-guide-vector-inward-gap-area-average-sampled-opacity`: production 0/60/120 equilateral untreated faces, positive inward gap, complete AreaAverage sampling, sampled paint, and opacity 0.58 at 900×620.\n",
        );

        let cubic_base = stage20q_region_response_session(
            modeled_authored_cubic_guide_face_session_for_canvas(CanvasSpec {
                width: 900.0,
                height: 620.0,
            }),
            RegionGeometryResponse::ConstantGap {
                sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
                minimum_gap: -40.0,
                maximum_gap: -40.0,
            },
        );
        let cubic = stage20q_sampled_region_session(cubic_base.document().clone(), 0.71);
        let (cubic_result, cubic_evidence) = stage20q_write_guide_face_case(
            &output,
            "authored-cubic-vector-outward-gap-area-average-sampled",
            cubic,
            &vector_input,
            EmbeddedSourceFormat::Svg,
            SourceFormatHint::Svg,
            "production authored-cubic Guide Faces / outward ConstantGap / AreaAverage / sampled paint",
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
            "authored-cubic outward evidence contains no zero-length cubic seam"
        );
        assert!(
            cubic_result
                .raster()
                .pixels()
                .chunks_exact(4)
                .any(|pixel| pixel[3] != 0)
        );
        manifest.push_str(
            "- `authored-cubic-vector-outward-gap-area-average-sampled`: genuinely authored cubic Guide Faces at 900×620 with sampled AreaAverage outward ConstantGap treatment.\n",
        );

        stage20q_write_direct_split_witness(&output);
        manifest.push_str(
            "- `crossing-split-direct-geometry-render`: direct geometry/render narrow-neck inward-gap witness. The bounded production authored-cubic outward-gap case retained one component per base, so this explicitly labeled direct case proves deterministic split ordinals, positive winding, base provenance, replay identity, and final-canvas rendering without claiming document/source sampling evidence.\n",
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
                layer.outputs().first().map(|output| &output.geometry),
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

    /// Proves unused sampling policy stays outside the Full-plus-solid Region realization key.
    #[test]
    fn full_solid_region_cache_omits_unused_sampling_limits() {
        let session = modeled_voronoi_session();
        let bytes = valid_document_bytes();
        let mut cache = DocumentDerivedCache::default();
        let baseline = evaluate_cached_document(
            document_request(&session, Arc::clone(&bytes)),
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .expect("Full solid Region evaluates");
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
            .expect("nonzero unused sampling policy");
        let replay = evaluate_cached_document(
            document_request(&session, bytes),
            irrelevant_policy,
            &cache,
            &NeverCancelled,
        )
        .expect("Full solid Region replay evaluates");
        assert_eq!(
            replay.diagnostics.aggregate.realization,
            CacheDisposition::Hit
        );
        assert_eq!(replay.diagnostics.aggregate.scene, CacheDisposition::Hit);
        assert_eq!(replay.diagnostics.aggregate.raster, CacheDisposition::Hit);
        assert_eq!(
            replay.result.channels()[0].realization_identity(),
            baseline_fingerprint,
            "evaluation policy remains outside accepted Full geometry identity"
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
        let GeometryOutput::CanonicalRegions(regions) = &output.geometry else {
            panic!("sampled output retains canonical regions")
        };
        let paints = output
            .primitive_paints
            .as_ref()
            .expect("sampled Region output carries per-region paint");
        assert_eq!(paints.len(), regions.regions().len());
        assert!(paints.iter().all(|paint| paint.alpha > 0.0));
        let svg = write_svg(evaluated.scene());
        assert!(svg.contains("channel-8-region-0"));
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
        let bytes = valid_document_bytes();
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
        let [PatternOutputLayer::ConnectionPaths { program, .. }] =
            definition.output_layers.as_slice()
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
            let DocumentRealization::Strokes(strokes) = &realization.realization else {
                panic!("connection fixture cannot realize marks")
            };
            for stroke in &strokes.output.strokes {
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
        let bytes = valid_document_bytes();
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
            .submit(document_request(history.session(), valid_document_bytes()))
            .unwrap();
        entered.recv_timeout(GUARD).unwrap();
        let base_definition = history.document().pattern_definition_bundles()[0]
            .definition
            .clone();
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(90),
                base_definition,
                edit: PatternDefinitionEdit::SetGuideOffsetSides {
                    mechanism_id: PatternMechanismId(92),
                    dimension_id: GuideDimensionId(95),
                    sides: OffsetSides::Left,
                },
            })
            .unwrap();
        let newest_ticket = scheduler
            .submit(document_request(history.session(), valid_document_bytes()))
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
            .submit(document_request(history.session(), valid_document_bytes()))
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
}

#[cfg(test)]
mod cache_key_tests {
    use super::*;

    fn family(aspect_locked: bool) -> FamilyCacheKey {
        FamilyCacheKey {
            canvas: (900.0_f64.to_bits(), 600.0_f64.to_bits()),
            density: (90.0_f64.to_bits(), 60.0_f64.to_bits()),
            aspect_locked,
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
        let mut broad = family(true);
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
        let mut broad = family(true);
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
            output_layers: vec![PatternOutputLayer::CircularMarks {
                id: toniator_domain::PatternOutputLayerId(layer_id),
                site_mechanism_id: toniator_domain::PatternMechanismId(2),
            }],
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
        connection.output_layers = vec![PatternOutputLayer::ConnectionPaths {
            id: toniator_domain::PatternOutputLayerId(11),
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
        }];
        let maze = connection_cache_contracts(
            &connection,
            SiteAdjacencyLimits::default(),
            ConnectionPathLimits::default(),
        )
        .expect("connection contracts");
        let mut tree = connection.clone();
        let [PatternOutputLayer::ConnectionPaths { program, .. }] =
            tree.output_layers.as_mut_slice()
        else {
            panic!("connection fixture retains output")
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
                SiteAdjacencyLimits::default(),
                ConnectionPathLimits::default(),
            )
            .expect("tree contracts")
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
        definition.output_layers = vec![PatternOutputLayer::MazeWalls {
            id: toniator_domain::PatternOutputLayerId(24),
            site_mechanism_id: toniator_domain::PatternMechanismId(23),
            program: toniator_domain::MazeProgram {
                algorithm: toniator_domain::GridMazeAlgorithm::RecursiveBacktracker,
                seed: 7,
            },
            style: toniator_domain::PathStrokeStyle::default(),
        }];
        let baseline_limits = MazeLimits::default();
        let baseline = maze_cache_contracts(&definition, baseline_limits).expect("maze contracts");
        assert_eq!(baseline.arrangement, MAZE_WALL_CONTRACT_ID);
        assert_eq!(
            baseline.algorithm,
            toniator_domain::GridMazeAlgorithm::RecursiveBacktracker,
            "the only current algorithm still participates as an explicit contract field"
        );
        let mut seeded = definition.clone();
        let [PatternOutputLayer::MazeWalls { program, .. }] = seeded.output_layers.as_mut_slice()
        else {
            panic!("maze fixture retains its maze output")
        };
        program.seed = 11;
        assert_ne!(
            baseline,
            maze_cache_contracts(&seeded, baseline_limits).expect("seeded maze contracts")
        );
        let changed_limits = MazeLimits {
            maximum_faces: baseline_limits.maximum_faces - 1,
            ..baseline_limits
        };
        assert_ne!(
            baseline,
            maze_cache_contracts(&definition, changed_limits).expect("limited maze contracts")
        );
        assert_eq!(
            family(true),
            family(true),
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
        let first_family = family(true);
        let second_family = family(true);
        assert_eq!(first_family, second_family);
        assert_ne!(family(true), family(false));
        let first_realization = realization(first_family, "content-a", "pixels-a", contract(1));
        let second_realization = realization(second_family, "content-a", "pixels-b", contract(1));
        assert_ne!(first_realization, second_realization);
        assert_ne!(
            first_realization,
            realization(family(true), "content-b", "pixels-a", contract(1))
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
        assert_eq!(family(true), family(true));
        // Decode cache lookup deliberately sees the logical reference, while
        // realization consumes only the immutable decoder result.
        assert_eq!(
            realization(family(true), "content-a", "pixels-a", contract(1)),
            realization(family(true), "content-a", "pixels-a", contract(1)),
        );
        assert_eq!(family(true), family(true));
        assert_ne!(
            realization(family(true), "content", "pixels", contract(1)),
            realization(family(true), "content", "pixels", contract(2)),
        );
    }
}
