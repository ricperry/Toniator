//! Cancellable canonical region construction from selected complete guide paths.

use std::collections::BTreeSet;

use toniator_domain::{GuideDimensionId, PatternMechanismId, PatternOutputLayerId};

use crate::{
    Bounds, CanonicalRegionDiagnostics, CanonicalRegionId, CanonicalRegionLimits,
    CanonicalRegionProposal, CanonicalRegionSet, CanonicalRegionSourceGroup,
    CanonicalRegionSourceId, CurvePath, CurveSegment, IntersectionKind, LineSegment, PathClosure,
    Point2, StructuralPathLocationProvenance, StructuralPathSet,
    build_canonical_regions_cancellable,
    planar_arrangement::{self, ArrangementPiece, VertexKey},
};

/// Stable identity for the Stage 20P guide-arrangement-face contract.
pub const GUIDE_FACE_CONTRACT_ID: &str = "toniator.guide-faces.v1";

/// Configurable bounded work for one guide-face build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuideFaceLimits {
    pub max_source_paths: usize,
    pub max_source_segments: usize,
    pub max_intersection_contacts: usize,
    pub max_split_segments: usize,
    pub max_vertices: usize,
    pub max_half_edges: usize,
    pub max_faces: usize,
    pub max_ring_segments: usize,
    pub max_inspections: usize,
}

impl Default for GuideFaceLimits {
    /// Returns the accepted finite Stage 20P arrangement limits.
    fn default() -> Self {
        Self {
            max_source_paths: 1_048_576,
            max_source_segments: 8_388_608,
            max_intersection_contacts: 8_388_608,
            max_split_segments: 8_388_608,
            max_vertices: 8_388_608,
            max_half_edges: 16_777_216,
            max_faces: 1_048_576,
            max_ring_segments: 8_388_608,
            max_inspections: 67_108_864,
        }
    }
}

/// One request bound to one output, complete family paths, and final canvas relevance.
#[derive(Clone, Debug, PartialEq)]
pub struct GuideFaceRequest {
    pub output_layer_id: PatternOutputLayerId,
    pub guide_mechanism_id: PatternMechanismId,
    pub dimensions: Vec<GuideDimensionId>,
    pub paths: StructuralPathSet,
    pub canvas: Bounds,
}

/// Stable non-partial guide-face construction error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuideFaceError {
    path: &'static str,
    message: &'static str,
}

impl GuideFaceError {
    /// Constructs a stable failure without leaking partial arrangement state.
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }
    /// Returns the stable producer-owned diagnostic path.
    pub const fn path(&self) -> &'static str {
        self.path
    }
    /// Returns the stable producer-owned diagnostic message.
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

/// Non-authoritative aggregate work facts excluded from geometry fingerprints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuideFaceDiagnostics {
    pub source_paths: usize,
    pub source_segments: usize,
    pub contacts: usize,
    pub faces: usize,
    pub inspections: usize,
}

/// Atomic canonical regions and their bounded diagnostic facts.
#[derive(Clone, Debug, PartialEq)]
pub struct GuideFaceResult {
    pub regions: CanonicalRegionSet,
    /// Ordered analytic centroids keyed by canonical region identity and excluded from region fingerprints.
    pub centroids: Vec<(CanonicalRegionId, crate::Point2)>,
    pub diagnostics: GuideFaceDiagnostics,
}

/// Builds all complete positive bounded guide faces relevant to the final canvas.
///
/// # Errors
///
/// Returns `evaluation.cancelled` for cancellation and stable `region.guide_faces.*`
/// diagnostics for malformed identity, geometry, coverage, limits, and allocation failures.
pub fn build_guide_faces_cancellable(
    request: GuideFaceRequest,
    limits: GuideFaceLimits,
    cancelled: impl Fn() -> bool,
) -> Result<GuideFaceResult, GuideFaceError> {
    validate_request(&request, limits)?;
    let mut inspections = 0usize;
    let selected = &request.dimensions;
    let mut paths = Vec::new();
    reserve(
        &mut paths,
        request.paths.paths().len(),
        "selected-path allocation failed",
    )?;
    for path in request.paths.paths() {
        if matches!(path.id.source, crate::StructuralPathSourceId::GuideDimension(id) if selected.contains(&id))
        {
            paths.push(path);
        }
    }
    if paths.len() > limits.max_source_paths {
        return Err(GuideFaceError::new(
            "region.guide_faces.limits.source_paths",
            "guide-face selected source-path limit exceeded",
        ));
    }
    if paths
        .iter()
        .any(|path| path.path.closure() != PathClosure::Open)
    {
        return Err(GuideFaceError::new(
            "region.guide_faces.geometry.path",
            "guide-face selected paths must be authored open components",
        ));
    }
    let mut first_source_order = Vec::new();
    reserve(
        &mut first_source_order,
        selected.len(),
        "selected-dimension order allocation failed",
    )?;
    for path in &paths {
        let crate::StructuralPathSourceId::GuideDimension(id) = path.id.source else {
            continue;
        };
        if !first_source_order.contains(&id) {
            first_source_order.push(id);
        }
    }
    if request
        .dimensions
        .iter()
        .any(|id| !first_source_order.contains(id))
    {
        return Err(GuideFaceError::new(
            "region.guide_faces.identity.dimension_paths",
            "each selected guide dimension must emit at least one structural path",
        ));
    }
    if first_source_order != request.dimensions {
        return Err(GuideFaceError::new(
            "region.guide_faces.identity.dimension_order",
            "guide-face request dimensions must follow first structural-path occurrence order",
        ));
    }
    let source_segments = paths
        .iter()
        .map(|path| path.path.segments().len())
        .sum::<usize>();
    if source_segments > limits.max_source_segments {
        return Err(GuideFaceError::new(
            "region.guide_faces.limits.source_segments",
            "guide-face source-segment limit exceeded",
        ));
    }
    let mut parameters = Vec::<Vec<Vec<f64>>>::new();
    reserve(
        &mut parameters,
        paths.len(),
        "split-parameter allocation failed",
    )?;
    for path in &paths {
        let mut per_path = Vec::new();
        reserve(
            &mut per_path,
            path.path.segments().len(),
            "split-parameter allocation failed",
        )?;
        for _ in path.path.segments() {
            let mut values = Vec::new();
            reserve(&mut values, 2, "split-parameter allocation failed")?;
            values.extend([0.0, 1.0]);
            per_path.push(values);
        }
        parameters.push(per_path);
    }
    let mut contacts = 0usize;
    for (path_index, path) in paths.iter().enumerate() {
        for first_segment in 0..path.path.segments().len() {
            for second_segment in (first_segment + 2)..path.path.segments().len() {
                poll(&cancelled)?;
                inspections = inspections.checked_add(1).ok_or(GuideFaceError::new(
                    "region.guide_faces.limits.inspections",
                    "guide-face inspection count overflowed",
                ))?;
                if inspections > limits.max_inspections {
                    return Err(GuideFaceError::new(
                        "region.guide_faces.limits.inspections",
                        "guide-face inspection limit exceeded",
                    ));
                }
                for intersection in path.path.segments()[first_segment]
                    .intersections(&path.path.segments()[second_segment])
                    .map_err(intersection_error)?
                {
                    if intersection.kind() != IntersectionKind::Crossing {
                        return Err(GuideFaceError::new(
                            "region.guide_faces.geometry.tangency",
                            "guide-face arrangements reject self tangent contacts",
                        ));
                    }
                    contacts += 1;
                    if contacts > limits.max_intersection_contacts {
                        return Err(GuideFaceError::new(
                            "region.guide_faces.limits.intersection_contacts",
                            "guide-face contact limit exceeded",
                        ));
                    }
                    reserve(
                        &mut parameters[path_index][first_segment],
                        1,
                        "split-contact allocation failed",
                    )?;
                    parameters[path_index][first_segment].push(intersection.first_parameter());
                    reserve(
                        &mut parameters[path_index][second_segment],
                        1,
                        "split-contact allocation failed",
                    )?;
                    parameters[path_index][second_segment].push(intersection.second_parameter());
                }
            }
        }
    }
    for first in 0..paths.len() {
        for second in first + 1..paths.len() {
            poll(&cancelled)?;
            inspections = inspections.checked_add(1).ok_or(GuideFaceError::new(
                "region.guide_faces.limits.inspections",
                "guide-face inspection count overflowed",
            ))?;
            if inspections > limits.max_inspections {
                return Err(GuideFaceError::new(
                    "region.guide_faces.limits.inspections",
                    "guide-face inspection limit exceeded",
                ));
            }
            let intersections = paths[first]
                .path
                .intersections(&paths[second].path)
                .map_err(intersection_error)?;
            for intersection in intersections {
                if intersection.kind() != IntersectionKind::Crossing {
                    return Err(GuideFaceError::new(
                        "region.guide_faces.geometry.tangency",
                        "guide-face arrangements reject tangent or endpoint contacts",
                    ));
                }
                contacts += 1;
                if contacts > limits.max_intersection_contacts {
                    return Err(GuideFaceError::new(
                        "region.guide_faces.limits.intersection_contacts",
                        "guide-face contact limit exceeded",
                    ));
                }
                let first_segment = intersection.first_location().segment_index();
                let second_segment = intersection.second_location().segment_index();
                reserve(
                    &mut parameters[first][first_segment],
                    1,
                    "split-contact allocation failed",
                )?;
                parameters[first][first_segment].push(intersection.first_location().parameter());
                reserve(
                    &mut parameters[second][second_segment],
                    1,
                    "split-contact allocation failed",
                )?;
                parameters[second][second_segment].push(intersection.second_location().parameter());
            }
        }
    }
    let mut pieces = Vec::<ArrangementPiece<PieceProvenance>>::new();
    pieces.try_reserve(source_segments).map_err(|_| {
        GuideFaceError::new(
            "region.guide_faces.allocation.split_segments",
            "guide-face split-segment allocation failed",
        )
    })?;
    for (path_index, path) in paths.iter().enumerate() {
        for (segment_index, segment) in path.path.segments().iter().copied().enumerate() {
            let values = &mut parameters[path_index][segment_index];
            values.sort_by(f64::total_cmp);
            values.dedup_by(|left, right| (*left - *right).abs() <= 1e-12);
            for pair in values.windows(2) {
                poll(&cancelled)?;
                if pair[1] - pair[0] <= 1e-12 {
                    continue;
                }
                let fragment = segment_range(segment, pair[0], pair[1]).map_err(curve_error)?;
                let start =
                    planar_arrangement::vertex_key(fragment.start()).map_err(curve_error)?;
                let end = planar_arrangement::vertex_key(fragment.end()).map_err(curve_error)?;
                if start == end {
                    return Err(GuideFaceError::new(
                        "region.guide_faces.geometry.path",
                        "guide-face split piece has zero length",
                    ));
                }
                let fragment = normalize_segment(fragment, start, end).map_err(curve_error)?;
                inspections = inspections.checked_add(1).ok_or(GuideFaceError::new(
                    "region.guide_faces.limits.inspections",
                    "guide-face inspection count overflowed",
                ))?;
                if inspections > limits.max_inspections {
                    return Err(GuideFaceError::new(
                        "region.guide_faces.limits.inspections",
                        "guide-face inspection limit exceeded",
                    ));
                }
                pieces.push(ArrangementPiece {
                    segment: fragment,
                    start,
                    end,
                    payload: PieceProvenance {
                        path: path.id,
                        segment_index,
                        start: pair[0],
                        end: pair[1],
                    },
                });
                if pieces.len() > limits.max_split_segments {
                    return Err(GuideFaceError::new(
                        "region.guide_faces.limits.split_segments",
                        "guide-face split-segment limit exceeded",
                    ));
                }
            }
        }
    }
    if pieces.len() > limits.max_half_edges / 2 {
        return Err(GuideFaceError::new(
            "region.guide_faces.limits.half_edges",
            "guide-face half-edge limit exceeded",
        ));
    }
    // Split parameters only govern construction of `pieces`; retaining them through
    // embedding and face extraction multiplies peak live arrangement state.
    drop(parameters);
    let vertex_capacity = pieces.len().checked_mul(2).ok_or(GuideFaceError::new(
        "region.guide_faces.allocation.vertices",
        "guide-face vertex capacity overflowed",
    ))?;
    let mut vertices = Vec::new();
    reserve(
        &mut vertices,
        vertex_capacity,
        "guide-face vertex allocation failed",
    )?;
    for piece in &pieces {
        vertices.extend([piece.start, piece.end]);
    }
    vertices.sort_unstable();
    vertices.dedup();
    if vertices.len() > limits.max_vertices {
        return Err(GuideFaceError::new(
            "region.guide_faces.limits.vertices",
            "guide-face vertex limit exceeded",
        ));
    }
    poll(&cancelled)?;
    let (edges, outgoing) = planar_arrangement::embed(&pieces, &cancelled).map_err(curve_error)?;
    // Vertex keys are fully represented by the embedded pieces and edge records now.
    drop(vertices);
    charge(
        &mut inspections,
        pieces
            .len()
            .checked_add(edges.len())
            .ok_or(GuideFaceError::new(
                "region.guide_faces.limits.inspections",
                "guide-face embedding inspection count overflowed",
            ))?,
        limits,
    )?;
    if edges.len() > limits.max_half_edges {
        return Err(GuideFaceError::new(
            "region.guide_faces.limits.half_edges",
            "guide-face half-edge limit exceeded",
        ));
    }
    let mut used = Vec::new();
    used.try_reserve(edges.len()).map_err(|_| {
        GuideFaceError::new(
            "region.guide_faces.allocation.faces",
            "guide-face face-state allocation failed",
        )
    })?;
    used.resize(edges.len(), false);
    let mut groups = Vec::new();
    groups
        .try_reserve(limits.max_faces.min(edges.len()))
        .map_err(|_| {
            GuideFaceError::new(
                "region.guide_faces.allocation.faces",
                "guide-face face allocation failed",
            )
        })?;
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        poll(&cancelled)?;
        let mut walk = Vec::new();
        walk.try_reserve(3).map_err(|_| {
            GuideFaceError::new(
                "region.guide_faces.allocation.faces",
                "guide-face face-walk allocation failed",
            )
        })?;
        let mut seen = Vec::new();
        reserve(
            &mut seen,
            edges.len(),
            "guide-face face-walk state allocation failed",
        )?;
        seen.resize(edges.len(), false);
        let mut current = start;
        loop {
            poll(&cancelled)?;
            inspections = inspections.checked_add(1).ok_or(GuideFaceError::new(
                "region.guide_faces.limits.inspections",
                "guide-face inspection count overflowed",
            ))?;
            if inspections > limits.max_inspections {
                return Err(GuideFaceError::new(
                    "region.guide_faces.limits.inspections",
                    "guide-face inspection limit exceeded",
                ));
            }
            if seen[current] {
                break;
            }
            seen[current] = true;
            reserve(&mut walk, 1, "guide-face face-walk allocation failed")?;
            walk.push(current);
            current = planar_arrangement::successor(current, &edges, &outgoing).ok_or(
                GuideFaceError::new(
                    "region.guide_faces.geometry.half_edge",
                    "guide-face half-edge successor is missing",
                ),
            )?;
            if current == start {
                break;
            }
            if walk.len() > pieces.len() {
                break;
            }
        }
        if current != start || walk.len() < 3 {
            continue;
        }
        for edge in &walk {
            used[*edge] = true;
        }
        let mut segments = Vec::new();
        segments.try_reserve(walk.len()).map_err(|_| {
            GuideFaceError::new(
                "region.guide_faces.allocation.faces",
                "guide-face ring allocation failed",
            )
        })?;
        for edge in &walk {
            poll(&cancelled)?;
            segments.push(
                directed_segment(&pieces[edges[*edge].piece].segment, edges[*edge].forward)
                    .map_err(curve_error)?,
            );
        }
        let ring = CurvePath::new(segments, PathClosure::Closed).map_err(curve_error)?;
        poll(&cancelled)?;
        charge(&mut inspections, ring.segments().len(), limits)?;
        if signed_exact_area_cancellable(&ring, &cancelled)? <= 1e-12 {
            continue;
        }
        if !ring_is_simple_cancellable(&ring, &mut inspections, limits, &cancelled)? {
            continue;
        }
        charge(
            &mut inspections,
            ring.segments().len().saturating_add(4),
            limits,
        )?;
        if !ring_overlaps_canvas(&ring, request.canvas, &cancelled)? {
            continue;
        }
        let source_capacity = walk.len().checked_mul(2).ok_or(GuideFaceError::new(
            "region.guide_faces.allocation.sources",
            "guide-face source provenance capacity overflowed",
        ))?;
        let mut sources = Vec::new();
        reserve(
            &mut sources,
            source_capacity,
            "guide-face source provenance allocation failed",
        )?;
        for edge in &walk {
            poll(&cancelled)?;
            let payload = &pieces[edges[*edge].piece].payload;
            sources.extend([
                location(payload.path, payload.segment_index, payload.start),
                location(payload.path, payload.segment_index, payload.end),
            ]);
        }
        sources.sort_unstable();
        sources.dedup();
        charge(&mut inspections, sources.len(), limits)?;
        let mut components = Vec::new();
        reserve(&mut components, 1, "guide-face component allocation failed")?;
        components.push(ring);
        reserve(&mut groups, 1, "guide-face face allocation failed")?;
        groups.push(CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::GuideBoundary(sources),
            components,
        });
        if groups.len() > limits.max_faces {
            return Err(GuideFaceError::new(
                "region.guide_faces.limits.faces",
                "guide-face retained-face limit exceeded",
            ));
        }
    }
    // The retained face groups own every result needed by the canonical handoff.
    // Releasing the transient arrangement before canonicalization avoids an otherwise
    // avoidable overlap between the split graph and canonical output.
    drop(used);
    drop(outgoing);
    drop(edges);
    drop(pieces);
    reject_nested_positive_cycles(&groups, &mut inspections, limits, &cancelled)?;
    if groups.is_empty() {
        return Err(GuideFaceError::new(
            "region.guide_faces.coverage.empty",
            "guide arrangement has no complete canvas-relevant bounded face",
        ));
    }
    let remaining_inspections =
        limits
            .max_inspections
            .checked_sub(inspections)
            .ok_or(GuideFaceError::new(
                "region.guide_faces.limits.inspections",
                "guide-face inspection budget is exhausted before canonical handoff",
            ))?;
    let (regions, canonical_diagnostics): (CanonicalRegionSet, CanonicalRegionDiagnostics) =
        build_canonical_regions_cancellable(
            CanonicalRegionProposal {
                output_layer_id: request.output_layer_id,
                source_groups: groups,
            },
            CanonicalRegionLimits::new(
                limits.max_faces,
                limits.max_faces,
                limits.max_ring_segments,
                remaining_inspections,
            )
            .map_err(canonical_handoff_error)?,
            &cancelled,
        )
        .map_err(canonical_handoff_error)?;
    charge(&mut inspections, canonical_diagnostics.inspections, limits)?;
    let mut centroids = Vec::new();
    reserve(
        &mut centroids,
        regions.regions().len(),
        "guide-face centroid allocation failed",
    )?;
    for region in regions.regions() {
        poll(&cancelled)?;
        charge(&mut inspections, region.ring.segments().len(), limits)?;
        centroids.push((
            region.id.clone(),
            centroid_for_ring_cancellable(&region.ring, region.area, &cancelled)?,
        ));
    }
    Ok(GuideFaceResult {
        diagnostics: GuideFaceDiagnostics {
            source_paths: paths.len(),
            source_segments,
            contacts,
            faces: regions.regions().len(),
            inspections,
        },
        regions,
        centroids,
    })
}

#[derive(Clone, Copy, Debug)]
struct PieceProvenance {
    path: crate::StructuralPathInstanceId,
    segment_index: usize,
    start: f64,
    end: f64,
}

/// Validates authoritative request identity and nonzero limits before geometry work.
fn validate_request(
    request: &GuideFaceRequest,
    limits: GuideFaceLimits,
) -> Result<(), GuideFaceError> {
    if request.output_layer_id.0 == 0
        || request.guide_mechanism_id.0 == 0
        || !(2..=3).contains(&request.dimensions.len())
        || request.dimensions.iter().any(|id| id.0 == 0)
        || request.dimensions.iter().collect::<BTreeSet<_>>().len() != request.dimensions.len()
    {
        return Err(GuideFaceError::new(
            "region.guide_faces.identity.dimensions",
            "guide-face requests require two or three unique nonzero dimensions",
        ));
    }
    if !request.canvas.min.is_finite() || !request.canvas.max.is_finite() {
        return Err(GuideFaceError::new(
            "region.guide_faces.geometry.canvas",
            "guide-face canvas must be finite",
        ));
    }
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
        return Err(GuideFaceError::new(
            "region.guide_faces.limits.zero",
            "guide-face limits must be nonzero",
        ));
    }
    if request.paths.source_mechanism_id() != request.guide_mechanism_id {
        return Err(GuideFaceError::new(
            "region.guide_faces.identity.guide_mechanism",
            "guide-face paths belong to a foreign guide mechanism",
        ));
    }
    Ok(())
}

/// Polls cancellation at a bounded arrangement work boundary.
fn poll(cancelled: &impl Fn() -> bool) -> Result<(), GuideFaceError> {
    (!cancelled()).then_some(()).ok_or(GuideFaceError::new(
        "evaluation.cancelled",
        "guide-face evaluation was cancelled",
    ))
}

/// Reserves one material Guide Faces vector through the stable allocation namespace.
///
/// Standard-library ordered maps and sets remain only for bounded lookup state where
/// insertion cannot report allocation failure; complete exported geometry products use
/// this fallible vector seam before they are published.
///
/// # Errors
///
/// Returns `region.guide_faces.allocation.material` when the requested capacity cannot be reserved.
fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    message: &'static str,
) -> Result<(), GuideFaceError> {
    values
        .try_reserve(additional)
        .map_err(|_| GuideFaceError::new("region.guide_faces.allocation.material", message))
}

/// Charges one planned arrangement phase against the single request-wide inspection budget.
///
/// # Errors
///
/// Returns `region.guide_faces.limits.inspections` before a phase can exceed the configured budget.
fn charge(
    inspections: &mut usize,
    additional: usize,
    limits: GuideFaceLimits,
) -> Result<(), GuideFaceError> {
    *inspections = inspections
        .checked_add(additional)
        .ok_or(GuideFaceError::new(
            "region.guide_faces.limits.inspections",
            "guide-face inspection count overflowed",
        ))?;
    if *inspections > limits.max_inspections {
        return Err(GuideFaceError::new(
            "region.guide_faces.limits.inspections",
            "guide-face inspection limit exceeded",
        ));
    }
    Ok(())
}

/// Maps reusable curve diagnostics into the guide-face diagnostic namespace.
fn curve_error(error: crate::CurveError) -> GuideFaceError {
    if error.path() == "evaluation.cancelled" {
        return GuideFaceError::new("evaluation.cancelled", error.message());
    }
    GuideFaceError::new(
        "region.guide_faces.geometry.path",
        "guide-face arrangement path operation failed",
    )
}

/// Maps curve-intersection overlap diagnostics into the dedicated Guide Faces contract.
fn intersection_error(error: crate::CurveError) -> GuideFaceError {
    if error.path() == "curve.path.intersections.overlap" {
        GuideFaceError::new(
            "region.guide_faces.geometry.overlap",
            "guide-face arrangements reject overlapping guide intervals",
        )
    } else {
        curve_error(error)
    }
}

/// Maps canonical-region handoff failures back into the Guide Faces producer namespace.
fn canonical_handoff_error(error: crate::CanonicalRegionError) -> GuideFaceError {
    match error.path() {
        "evaluation.cancelled" => GuideFaceError::new("evaluation.cancelled", error.message()),
        "region.limits.segments" => GuideFaceError::new(
            "region.guide_faces.limits.ring_segments",
            "guide-face retained-ring segment limit exceeded",
        ),
        path if path.starts_with("region.limits.") => GuideFaceError::new(
            "region.guide_faces.limits.canonical",
            "guide-face canonical-region limit exceeded",
        ),
        path if path.starts_with("region.allocation.") => GuideFaceError::new(
            "region.guide_faces.allocation.canonical",
            "guide-face canonical-region allocation failed",
        ),
        path if path.starts_with("region.identity.") => GuideFaceError::new(
            "region.guide_faces.identity.canonical",
            "guide-face canonical-region identity is invalid",
        ),
        _ => GuideFaceError::new(
            "region.guide_faces.geometry.canonical",
            "guide-face canonical-region geometry is invalid",
        ),
    }
}

/// Rejects a closed walk with nonadjacent boundary contacts before canonical publication.
///
/// Guide-face half-edge traversal can encounter a positive exterior walk around an open
/// arrangement. Such a walk is not one bounded face and therefore has no valid canonical
/// region authority. Adjacent endpoints retain their ordinary ring connection; every other
/// contact makes the walk ineligible rather than turning one unrelated malformed exterior
/// traversal into an atomic failure for otherwise valid cells.
///
/// # Errors
///
/// Returns cancellation, configured inspection-limit, or curve-intersection diagnostics while no
/// partial face group has been retained for the candidate walk.
fn ring_is_simple_cancellable(
    ring: &CurvePath,
    inspections: &mut usize,
    limits: GuideFaceLimits,
    cancelled: &impl Fn() -> bool,
) -> Result<bool, GuideFaceError> {
    for first in 0..ring.segments().len() {
        for second in first + 1..ring.segments().len() {
            poll(cancelled)?;
            if second == first + 1 || (first == 0 && second + 1 == ring.segments().len()) {
                continue;
            }
            charge(inspections, 1, limits)?;
            match ring.segments()[first].intersections(&ring.segments()[second]) {
                Ok(intersections) if !intersections.is_empty() => return Ok(false),
                Ok(_) => {}
                Err(error) if error.path() == "curve.path.intersections.overlap" => {
                    // A positive-length retrace makes this traversal non-simple just as a
                    // discrete nonadjacent contact does. Source-guide overlap has already been
                    // rejected before splitting; this private candidate walk is discarded so it
                    // cannot turn an exterior traversal into a fatal document error.
                    return Ok(false);
                }
                Err(error) => return Err(intersection_error(error)),
            }
        }
    }
    Ok(true)
}

/// Returns a finite exact subrange of a line or cubic segment.
fn segment_range(
    segment: CurveSegment,
    start: f64,
    end: f64,
) -> Result<CurveSegment, crate::CurveError> {
    match segment {
        CurveSegment::Line(_) => Ok(CurveSegment::Line(LineSegment::new(
            segment.point_at(start)?,
            segment.point_at(end)?,
        )?)),
        CurveSegment::CubicBezier(cubic) => {
            let (left, _) = cubic.split(end)?;
            if start == 0.0 {
                Ok(CurveSegment::CubicBezier(left))
            } else {
                let (_, range) = left.split(start / end)?;
                Ok(CurveSegment::CubicBezier(range))
            }
        }
    }
}

/// Replaces split endpoints with their one canonical lattice coordinate.
fn normalize_segment(
    segment: CurveSegment,
    start: VertexKey,
    end: VertexKey,
) -> Result<CurveSegment, crate::CurveError> {
    let first = planar_arrangement::point_for_key(start);
    let last = planar_arrangement::point_for_key(end);
    match segment {
        CurveSegment::Line(_) => Ok(CurveSegment::Line(LineSegment::new(first, last)?)),
        CurveSegment::CubicBezier(cubic) => Ok(CurveSegment::CubicBezier(
            crate::CubicBezierSegment::new(first, cubic.control_1(), cubic.control_2(), last)?,
        )),
    }
}

/// Returns a directed segment without approximating a cubic boundary.
fn directed_segment(
    segment: &CurveSegment,
    forward: bool,
) -> Result<CurveSegment, crate::CurveError> {
    if forward {
        return Ok(*segment);
    }
    match segment {
        CurveSegment::Line(line) => Ok(CurveSegment::Line(LineSegment::new(
            line.end(),
            line.start(),
        )?)),
        CurveSegment::CubicBezier(cubic) => {
            Ok(CurveSegment::CubicBezier(crate::CubicBezierSegment::new(
                cubic.end(),
                cubic.control_2(),
                cubic.control_1(),
                cubic.start(),
            )?))
        }
    }
}

/// Builds stable structural provenance for one split endpoint.
fn location(
    path: crate::StructuralPathInstanceId,
    segment_index: usize,
    parameter: f64,
) -> StructuralPathLocationProvenance {
    StructuralPathLocationProvenance {
        path,
        segment_index,
        parameter_bits: if parameter == 0.0 {
            0.0_f64.to_bits()
        } else {
            parameter.to_bits()
        },
    }
}

/// Determines exact curve-boundary or strict-containment relevance without introducing canvas topology.
fn ring_overlaps_canvas(
    ring: &CurvePath,
    canvas: Bounds,
    cancelled: &impl Fn() -> bool,
) -> Result<bool, GuideFaceError> {
    let ring_bounds = ring.bounds().map_err(curve_error)?;
    if ring_bounds.max.x < canvas.min.x
        || ring_bounds.min.x > canvas.max.x
        || ring_bounds.max.y < canvas.min.y
        || ring_bounds.min.y > canvas.max.y
    {
        return Ok(false);
    }
    let canvas_ring = canvas_edges(canvas)?;
    for segment in ring.segments() {
        poll(cancelled)?;
        for edge in &canvas_ring {
            match segment.intersections(edge) {
                Ok(contacts) if !contacts.is_empty() => return Ok(true),
                Ok(_) => {}
                Err(error) if error.path() == "curve.path.intersections.overlap" => {
                    // The canvas only classifies a complete face; collinearity here never
                    // changes arrangement topology and therefore establishes relevance.
                    return Ok(true);
                }
                Err(error) => return Err(intersection_error(error)),
            }
        }
        if point_in_bounds(segment.start(), canvas) {
            return Ok(true);
        }
    }
    for corner in [
        canvas.min,
        Point2::new(canvas.max.x, canvas.min.y),
        canvas.max,
        Point2::new(canvas.min.x, canvas.max.y),
    ] {
        poll(cancelled)?;
        if point_in_ring(corner, ring, cancelled)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Rejects disconnected positive cycles that would require unsupported hole semantics.
///
/// Finite ring bounds are sorted by minimum X so only X-overlapping candidates
/// can consume containment work. This preserves exact analytic classification
/// while avoiding all-pairs inspection for ordinary dense guide lattices.
/// Charged inspections mutate only the caller-owned diagnostic budget.
///
/// # Errors
///
/// Returns cancellation, curve, containment, or configured inspection-limit
/// diagnostics without publishing partial region authority.
fn reject_nested_positive_cycles(
    groups: &[CanonicalRegionSourceGroup],
    inspections: &mut usize,
    limits: GuideFaceLimits,
    cancelled: &impl Fn() -> bool,
) -> Result<(), GuideFaceError> {
    let mut bounded = Vec::new();
    reserve(
        &mut bounded,
        groups.len(),
        "guide-face hole bounds allocation failed",
    )?;
    for (index, group) in groups.iter().enumerate() {
        bounded.push((index, group.components[0].bounds().map_err(curve_error)?));
    }
    bounded.sort_by(|left, right| {
        left.1
            .min
            .x
            .total_cmp(&right.1.min.x)
            .then(left.1.max.x.total_cmp(&right.1.max.x))
            .then(left.0.cmp(&right.0))
    });
    for (left_position, (left_index, left_bounds)) in bounded.iter().copied().enumerate() {
        poll(cancelled)?;
        let left = &groups[left_index];
        let left_ring = &left.components[0];
        for (right_index, right_bounds) in bounded.iter().copied().skip(left_position + 1) {
            if right_bounds.min.x >= left_bounds.max.x {
                break;
            }
            poll(cancelled)?;
            charge(inspections, 1, limits)?;
            if !bounds_contains(left_bounds, right_bounds) {
                continue;
            }
            let right = &groups[right_index];
            let right_ring = &right.components[0];
            let shares_boundary_source = match (&left.source_id, &right.source_id) {
                (
                    CanonicalRegionSourceId::GuideBoundary(left_sources),
                    CanonicalRegionSourceId::GuideBoundary(right_sources),
                ) => left_sources
                    .iter()
                    .any(|source| right_sources.contains(source)),
                _ => false,
            };
            let nested = if !shares_boundary_source {
                let representative = centroid_for_ring_cancellable(
                    right_ring,
                    signed_exact_area_cancellable(right_ring, cancelled)?,
                    cancelled,
                )?;
                point_in_ring(representative, left_ring, cancelled)?
            } else {
                false
            };
            if nested {
                return Err(GuideFaceError::new(
                    "region.guide_faces.geometry.holes",
                    "nested disconnected guide faces would require unsupported holes",
                ));
            }
        }
    }
    Ok(())
}

/// Reports strict finite bounds containment for disconnected-cycle hole preflight.
fn bounds_contains(outer: Bounds, inner: Bounds) -> bool {
    outer.min.x < inner.min.x
        && outer.max.x > inner.max.x
        && outer.min.y < inner.min.y
        && outer.max.y > inner.max.y
}

/// Returns the four finite canvas boundary segments used only for relevance classification.
fn canvas_edges(canvas: Bounds) -> Result<[CurveSegment; 4], GuideFaceError> {
    let lower_left = canvas.min;
    let lower_right = Point2::new(canvas.max.x, canvas.min.y);
    let upper_right = canvas.max;
    let upper_left = Point2::new(canvas.min.x, canvas.max.y);
    Ok([
        CurveSegment::Line(LineSegment::new(lower_left, lower_right).map_err(curve_error)?),
        CurveSegment::Line(LineSegment::new(lower_right, upper_right).map_err(curve_error)?),
        CurveSegment::Line(LineSegment::new(upper_right, upper_left).map_err(curve_error)?),
        CurveSegment::Line(LineSegment::new(upper_left, lower_left).map_err(curve_error)?),
    ])
}

/// Reports strict finite canvas containment without applying a clip.
fn point_in_bounds(point: Point2, bounds: Bounds) -> bool {
    point.x > bounds.min.x
        && point.x < bounds.max.x
        && point.y > bounds.min.y
        && point.y < bounds.max.y
}

/// Classifies a point against an analytic closed ring with deterministic finite crossing rays.
///
/// The classifier retries alternate directions when a guide edge overlaps a classification ray.
/// Those overlaps do not alter arrangement topology and must not turn a relevance or hole check
/// into the producer's topology-overlap diagnostic.
fn point_in_ring(
    point: Point2,
    ring: &CurvePath,
    cancelled: &impl Fn() -> bool,
) -> Result<bool, GuideFaceError> {
    let bounds = ring.bounds().map_err(curve_error)?;
    let extent = (bounds.max.x - bounds.min.x)
        .abs()
        .max((bounds.max.y - bounds.min.y).abs())
        .max(1.0)
        * 4.0;
    for (dx, dy) in [
        (1.0, 0.123_456_789),
        (0.347, 1.0),
        (-1.0, 0.271),
        (0.271, -1.0),
    ] {
        let ray = CurveSegment::Line(
            LineSegment::new(
                point,
                Point2::new(point.x + extent * dx, point.y + extent * dy),
            )
            .map_err(curve_error)?,
        );
        let mut crossings = 0usize;
        let mut overlaps = false;
        for segment in ring.segments() {
            poll(cancelled)?;
            match segment.intersections(&ray) {
                Ok(contacts) => {
                    crossings += contacts
                        .into_iter()
                        .filter(|contact| {
                            contact.kind() == IntersectionKind::Crossing
                                && (contact.point().x - point.x)
                                    .mul_add(dx, (contact.point().y - point.y) * dy)
                                    > 1.0e-12
                        })
                        .count();
                }
                Err(error) if error.path() == "curve.path.intersections.overlap" => {
                    overlaps = true;
                    break;
                }
                Err(error) => return Err(intersection_error(error)),
            }
        }
        if !overlaps {
            return Ok(crossings % 2 == 1);
        }
    }
    Err(GuideFaceError::new(
        "region.guide_faces.geometry.overlap",
        "guide-face point classification could not select a non-overlapping ray",
    ))
}

/// Integrates exact line/cubic Green area to discriminate positive arrangement walks.
fn signed_exact_area(ring: &CurvePath) -> f64 {
    ring.segments()
        .iter()
        .map(|segment| {
            let points = match segment {
                CurveSegment::Line(line) => [line.start(), line.end(), line.end(), line.end()],
                CurveSegment::CubicBezier(cubic) => [
                    cubic.start(),
                    cubic.control_1(),
                    cubic.control_2(),
                    cubic.end(),
                ],
            };
            let x = [
                points[0].x,
                3.0 * (points[1].x - points[0].x),
                3.0 * (points[0].x - 2.0 * points[1].x + points[2].x),
                -points[0].x + 3.0 * points[1].x - 3.0 * points[2].x + points[3].x,
            ];
            let y = [
                points[0].y,
                3.0 * (points[1].y - points[0].y),
                3.0 * (points[0].y - 2.0 * points[1].y + points[2].y),
                -points[0].y + 3.0 * points[1].y - 3.0 * points[2].y + points[3].y,
            ];
            let mut integral = 0.0;
            for first in 0..4 {
                for second in 1..4 {
                    integral += (x[first] * y[second] - y[first] * x[second]) * second as f64
                        / (first + second) as f64;
                }
            }
            0.5 * integral
        })
        .sum()
}

/// Integrates exact signed area while polling cancellation once per analytic segment.
fn signed_exact_area_cancellable(
    ring: &CurvePath,
    cancelled: &impl Fn() -> bool,
) -> Result<f64, GuideFaceError> {
    let mut area = 0.0;
    for segment in ring.segments() {
        poll(cancelled)?;
        area += signed_exact_area(
            &CurvePath::new(vec![*segment], PathClosure::Open).map_err(curve_error)?,
        );
    }
    Ok(area)
}

/// Integrates one finite analytic centroid while polling cancellation once per boundary segment.
fn centroid_for_ring_cancellable(
    ring: &CurvePath,
    area: f64,
    cancelled: &impl Fn() -> bool,
) -> Result<crate::Point2, GuideFaceError> {
    let mut x_moment = 0.0;
    let mut y_moment = 0.0;
    for segment in ring.segments() {
        poll(cancelled)?;
        let (x, y) = power_coefficients(*segment);
        x_moment += integrate_product(square(x), [y[1], 2.0 * y[2], 3.0 * y[3]]);
        y_moment -= integrate_product(square(y), [x[1], 2.0 * x[2], 3.0 * x[3]]);
    }
    let centroid = crate::Point2::new(x_moment / (2.0 * area), y_moment / (2.0 * area));
    centroid
        .is_finite()
        .then_some(centroid)
        .ok_or(GuideFaceError::new(
            "region.guide_faces.geometry.centroid",
            "guide-face centroid must remain finite",
        ))
}

/// Converts a line or cubic curve segment into exact cubic power coefficients.
fn power_coefficients(segment: CurveSegment) -> ([f64; 4], [f64; 4]) {
    if let CurveSegment::Line(line) = segment {
        return (
            [line.start().x, line.end().x - line.start().x, 0.0, 0.0],
            [line.start().y, line.end().y - line.start().y, 0.0, 0.0],
        );
    }
    let CurveSegment::CubicBezier(cubic) = segment else {
        unreachable!("line returned above");
    };
    let points = [
        cubic.start(),
        cubic.control_1(),
        cubic.control_2(),
        cubic.end(),
    ];
    let coefficient = |values: [f64; 4]| {
        [
            values[0],
            3.0 * (values[1] - values[0]),
            3.0 * (values[0] - 2.0 * values[1] + values[2]),
            -values[0] + 3.0 * values[1] - 3.0 * values[2] + values[3],
        ]
    };
    (
        coefficient(points.map(|point| point.x)),
        coefficient(points.map(|point| point.y)),
    )
}

/// Squares a cubic polynomial into exact degree-six coefficients.
fn square(value: [f64; 4]) -> [f64; 7] {
    let mut output = [0.0; 7];
    for first in 0..4 {
        for second in 0..4 {
            output[first + second] += value[first] * value[second];
        }
    }
    output
}

/// Integrates a product of finite power-basis polynomials on the unit interval.
fn integrate_product<const FIRST: usize, const SECOND: usize>(
    first: [f64; FIRST],
    second: [f64; SECOND],
) -> f64 {
    let mut value = 0.0;
    for (left, first_value) in first.iter().enumerate() {
        for (right, second_value) in second.iter().enumerate() {
            value += first_value * second_value / (left + right + 1) as f64;
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CubicBezierSegment, Point2, StructuralPathInstance, StructuralPathInstanceId,
        StructuralPathSourceId,
    };

    /// Builds one ordered guide path set from dimension-tagged open finite paths.
    fn paths(values: Vec<(u64, CurvePath)>) -> StructuralPathSet {
        let source = PatternMechanismId(7);
        StructuralPathSet::new(
            "stage20p-test-family".into(),
            source,
            values
                .into_iter()
                .enumerate()
                .map(|(ordinal, (dimension, path))| StructuralPathInstance {
                    id: StructuralPathInstanceId {
                        source: StructuralPathSourceId::GuideDimension(GuideDimensionId(dimension)),
                        repetition_index: ordinal as i64,
                        component_ordinal: 0,
                    },
                    source_structure_id: None,
                    path,
                })
                .collect(),
        )
        .expect("ordered finite structural paths")
    }

    /// Proves a positive-length retrace classifies one private traversal as non-simple.
    ///
    /// Source-guide overlap remains an earlier fatal input error. Once half-edge traversal has
    /// formed a candidate ring, a nonadjacent retrace identifies an exterior/non-face walk and
    /// must be discarded without aborting unrelated valid cells.
    #[test]
    fn retraced_candidate_ring_is_non_simple_without_a_fatal_overlap_error() {
        let points = [
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 4.0),
            Point2::new(0.0, 4.0),
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(0.0, 0.0),
        ];
        let ring = CurvePath::new(
            points
                .windows(2)
                .map(|pair| {
                    CurveSegment::Line(
                        LineSegment::new(pair[0], pair[1]).expect("finite retraced ring edge"),
                    )
                })
                .collect(),
            PathClosure::Closed,
        )
        .expect("retraced walk remains a closed candidate path");
        let mut inspections = 0;
        assert!(
            !ring_is_simple_cancellable(
                &ring,
                &mut inspections,
                GuideFaceLimits::default(),
                &|| false,
            )
            .expect("candidate overlap classifies without a fatal source-guide error")
        );
        assert!(inspections > 0);
    }

    /// Builds a complete two-guide rectangular arrangement whose canvas-relevant cell is canonical.
    #[test]
    fn two_guide_rectangular_arrangement_builds_a_region() {
        let lines = |a, b| CurvePath::line(a, b).expect("finite line");
        let result = build_guide_faces_cancellable(
            GuideFaceRequest {
                output_layer_id: PatternOutputLayerId(9),
                guide_mechanism_id: PatternMechanismId(7),
                dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
                paths: paths(vec![
                    (1, lines(Point2::new(-2.0, -5.0), Point2::new(-2.0, 5.0))),
                    (1, lines(Point2::new(2.0, -5.0), Point2::new(2.0, 5.0))),
                    (2, lines(Point2::new(-5.0, -2.0), Point2::new(5.0, -2.0))),
                    (2, lines(Point2::new(-5.0, 2.0), Point2::new(5.0, 2.0))),
                ]),
                canvas: Bounds::new(Point2::new(-1.0, -1.0), Point2::new(1.0, 1.0))
                    .expect("finite canvas"),
            },
            GuideFaceLimits::default(),
            || false,
        )
        .expect("two guides form one face");
        assert_eq!(result.regions.regions().len(), 1);
    }

    /// Builds a phase-aligned Cartesian 0/60/120 arrangement with equilateral canonical faces.
    #[test]
    fn three_guide_arrangement_builds_regions() {
        let line = |a, b| CurvePath::line(a, b).expect("finite line");
        let result = build_guide_faces_cancellable(
            GuideFaceRequest {
                output_layer_id: PatternOutputLayerId(9),
                guide_mechanism_id: PatternMechanismId(7),
                dimensions: vec![
                    GuideDimensionId(1),
                    GuideDimensionId(2),
                    GuideDimensionId(3),
                ],
                paths: paths(vec![
                    (1, line(Point2::new(-8.0, -2.0), Point2::new(8.0, -2.0))),
                    (1, line(Point2::new(-8.0, -1.0), Point2::new(8.0, -1.0))),
                    (1, line(Point2::new(-8.0, 0.0), Point2::new(8.0, 0.0))),
                    (1, line(Point2::new(-8.0, 1.0), Point2::new(8.0, 1.0))),
                    (1, line(Point2::new(-8.0, 2.0), Point2::new(8.0, 2.0))),
                    (
                        2,
                        line(
                            Point2::new(-8.0, -17.856406460551018),
                            Point2::new(8.0, 9.856406460551018),
                        ),
                    ),
                    (
                        2,
                        line(
                            Point2::new(-8.0, -15.856406460551018),
                            Point2::new(8.0, 11.856406460551018),
                        ),
                    ),
                    (
                        2,
                        line(
                            Point2::new(-8.0, -13.856406460551018),
                            Point2::new(8.0, 13.856406460551018),
                        ),
                    ),
                    (
                        2,
                        line(
                            Point2::new(-8.0, -11.856406460551018),
                            Point2::new(8.0, 15.856406460551018),
                        ),
                    ),
                    (
                        2,
                        line(
                            Point2::new(-8.0, -9.856406460551018),
                            Point2::new(8.0, 17.856406460551018),
                        ),
                    ),
                    (
                        3,
                        line(
                            Point2::new(-8.0, 9.856406460551018),
                            Point2::new(8.0, -17.856406460551018),
                        ),
                    ),
                    (
                        3,
                        line(
                            Point2::new(-8.0, 11.856406460551018),
                            Point2::new(8.0, -15.856406460551018),
                        ),
                    ),
                    (
                        3,
                        line(
                            Point2::new(-8.0, 13.856406460551018),
                            Point2::new(8.0, -13.856406460551018),
                        ),
                    ),
                    (
                        3,
                        line(
                            Point2::new(-8.0, 15.856406460551018),
                            Point2::new(8.0, -11.856406460551018),
                        ),
                    ),
                    (
                        3,
                        line(
                            Point2::new(-8.0, 17.856406460551018),
                            Point2::new(8.0, -9.856406460551018),
                        ),
                    ),
                ]),
                canvas: Bounds::new(Point2::new(-1.0, -1.0), Point2::new(1.0, 1.0))
                    .expect("finite canvas"),
            },
            GuideFaceLimits::default(),
            || false,
        )
        .expect("three guides form canonical faces");
        assert!(!result.regions.regions().is_empty());
        for region in result.regions.regions() {
            assert_eq!(region.ring.segments().len(), 3);
            let lengths: Vec<_> = region
                .ring
                .segments()
                .iter()
                .map(|segment| {
                    let start = segment.start();
                    let end = segment.end();
                    (end.x - start.x).hypot(end.y - start.y)
                })
                .collect();
            assert!(
                lengths
                    .iter()
                    .all(|length| (*length - lengths[0]).abs() <= 1.0e-8)
            );
        }
    }

    /// Proves genuinely cubic guide boundaries survive splitting and produce a closed canonical face.
    #[test]
    fn authored_cubic_guides_participate_in_a_closed_face() {
        let cubic = |start, first, second, end| {
            CurvePath::new(
                vec![CurveSegment::CubicBezier(
                    CubicBezierSegment::new(start, first, second, end).expect("finite cubic"),
                )],
                PathClosure::Open,
            )
            .expect("open cubic")
        };
        let line = |a, b| CurvePath::line(a, b).expect("finite line");
        let result = build_guide_faces_cancellable(
            GuideFaceRequest {
                output_layer_id: PatternOutputLayerId(9),
                guide_mechanism_id: PatternMechanismId(7),
                dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
                paths: paths(vec![
                    (
                        1,
                        cubic(
                            Point2::new(-5.0, -2.0),
                            Point2::new(-2.0, -3.0),
                            Point2::new(2.0, -3.0),
                            Point2::new(5.0, -2.0),
                        ),
                    ),
                    (
                        1,
                        cubic(
                            Point2::new(-5.0, 2.0),
                            Point2::new(-2.0, 3.0),
                            Point2::new(2.0, 3.0),
                            Point2::new(5.0, 2.0),
                        ),
                    ),
                    (2, line(Point2::new(-2.0, -5.0), Point2::new(-2.0, 5.0))),
                    (2, line(Point2::new(2.0, -5.0), Point2::new(2.0, 5.0))),
                ]),
                canvas: Bounds::new(Point2::new(-1.0, -1.0), Point2::new(1.0, 1.0))
                    .expect("finite canvas"),
            },
            GuideFaceLimits::default(),
            || false,
        )
        .expect("cubic guides form a closed face");
        assert!(result.regions.regions().iter().any(|region| {
            region
                .ring
                .segments()
                .iter()
                .any(|segment| matches!(segment, CurveSegment::CubicBezier(_)))
        }));
    }

    /// Proves disconnected nested positive cycles fail before canonical regions could imply a hole.
    #[test]
    fn nested_disconnected_positive_cycles_reject_holes() {
        let square = |minimum: f64, maximum: f64| {
            let points = [
                Point2::new(minimum, minimum),
                Point2::new(maximum, minimum),
                Point2::new(maximum, maximum),
                Point2::new(minimum, maximum),
            ];
            CurvePath::new(
                (0..points.len())
                    .map(|index| {
                        CurveSegment::Line(
                            LineSegment::new(points[index], points[(index + 1) % points.len()])
                                .expect("finite square edge"),
                        )
                    })
                    .collect(),
                PathClosure::Closed,
            )
            .expect("closed square")
        };
        let groups = vec![
            CanonicalRegionSourceGroup {
                source_id: CanonicalRegionSourceId::GuideBoundary(Vec::new()),
                components: vec![square(-4.0, 4.0)],
            },
            CanonicalRegionSourceGroup {
                source_id: CanonicalRegionSourceId::GuideBoundary(Vec::new()),
                components: vec![square(-1.0, 1.0)],
            },
        ];
        let mut inspections = 0;
        assert_eq!(
            reject_nested_positive_cycles(
                &groups,
                &mut inspections,
                GuideFaceLimits::default(),
                &|| false,
            )
            .expect_err("nested disconnected cycles reject")
            .path(),
            "region.guide_faces.geometry.holes",
        );
    }

    /// Proves a concave outer bounds box alone cannot falsely classify a disjoint inner cycle as a hole.
    #[test]
    fn concave_bounds_containment_without_ring_containment_is_not_a_hole() {
        let ring = |points: Vec<Point2>| {
            CurvePath::new(
                (0..points.len())
                    .map(|index| {
                        CurveSegment::Line(
                            LineSegment::new(points[index], points[(index + 1) % points.len()])
                                .expect("finite ring edge"),
                        )
                    })
                    .collect(),
                PathClosure::Closed,
            )
            .expect("closed ring")
        };
        let groups = vec![
            CanonicalRegionSourceGroup {
                source_id: CanonicalRegionSourceId::GuideBoundary(Vec::new()),
                components: vec![ring(vec![
                    Point2::new(-4.0, -4.0),
                    Point2::new(4.0, -4.0),
                    Point2::new(4.0, -1.0),
                    Point2::new(-1.0, -1.0),
                    Point2::new(-1.0, 4.0),
                    Point2::new(-4.0, 4.0),
                ])],
            },
            CanonicalRegionSourceGroup {
                source_id: CanonicalRegionSourceId::GuideBoundary(Vec::new()),
                components: vec![ring(vec![
                    Point2::new(1.0, 1.0),
                    Point2::new(3.0, 1.0),
                    Point2::new(3.0, 3.0),
                    Point2::new(1.0, 3.0),
                ])],
            },
        ];
        let mut inspections = 0;
        reject_nested_positive_cycles(
            &groups,
            &mut inspections,
            GuideFaceLimits::default(),
            &|| false,
        )
        .expect("disjoint concave bounds do not imply a hole");
    }

    /// Proves the shared material-vector reservation mapper never exposes allocator wording.
    #[test]
    fn material_reservation_maps_overflow_to_guide_face_allocation() {
        let mut values = Vec::<u8>::new();
        assert_eq!(
            reserve(&mut values, usize::MAX, "test allocation")
                .expect_err("impossible capacity rejects")
                .path(),
            "region.guide_faces.allocation.material",
        );
    }
}
