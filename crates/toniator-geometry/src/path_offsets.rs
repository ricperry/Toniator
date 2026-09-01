//! Reusable finite normal offsets for canonical curve centerlines.

use crate::{
    Bounds, CurveError, CurvePath, CurveSegment, LineSegment, PathClosure, PathLocation, Point2,
    Vector2,
};

/// The deterministic maximum adaptive-offset depth accepted by the Stage 20J boundary.
pub const MAX_PATH_OFFSET_SUBDIVISION_DEPTH: u8 = 48;
/// The maximum number of compact derived segments accepted by one offset request.
pub const MAX_PATH_OFFSET_SEGMENTS: usize = 262_144;
/// The maximum number of published pieces accepted by one offset request.
pub const MAX_PATH_OFFSET_COMPONENTS: usize = 65_536;
/// The maximum nonadjacent segment pairs examined while dissolving one offset component.
pub const MAX_PATH_OFFSET_CLEANUP_PAIRS: usize = 1_048_576;
/// The maximum dyadic intervals examined while isolating cubic offset cusps and reversals.
pub const MAX_PATH_OFFSET_CUSP_ISOLATION_WORK: usize = 262_144;
/// The default maximum normal-distance fitting error for one compact cubic offset piece.
pub const DEFAULT_PATH_OFFSET_TOLERANCE: f64 = 1.0 / 64.0;
/// Versioned identity for the cusp-aware reusable path-offset algorithm and its fixed defaults.
pub const PATH_OFFSET_ALGORITHM_CONTRACT_ID: &str = "toniator.path-offset.v6.solved-crossing-nodes";

/// Endpoint handling requested before a normal offset is constructed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathOffsetEndpointPolicy {
    /// Retains the authored endpoints exactly apart from their normal displacement.
    Preserve,
    /// Extends open endpoint tangents until they reach the supplied finite planning bounds.
    TangentialExtension { bounds: Bounds },
}

/// The accepted deterministic topology-cleanup policy for an offset request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathOffsetCleanup {
    /// Retains ordered finite centerlines and relies on their canonical fill consumer for overlap.
    DissolveCrossings,
    /// Relinks source-ordered Constant-gap branches at exact solved crossing and cusp nodes.
    PlanarConstantGap,
}

/// Resource bounds owned by one path-offset request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathOffsetLimits {
    /// Caps recursive curve work; the compact fitted implementation never exceeds it.
    pub maximum_subdivision_depth: u8,
    /// Caps emitted compact line/cubic segments before any path is published.
    pub maximum_segments: usize,
    /// Caps ordered components before any result is published.
    pub maximum_components: usize,
    /// Bounds cleanup pair work independently from emitted geometry.
    pub maximum_cleanup_pairs: usize,
    /// Bounds dyadic cusp/reversal classification work independently from fitted geometry.
    pub maximum_cusp_isolation_work: usize,
    /// Bounds deterministic cubic offset fitting error in document units.
    pub tolerance: f64,
}

/// Mutable request-wide offset accounting shared by one or more path constructions.
///
/// The budget owns the exact configured limits and cumulative candidate segments, retained
/// components, cleanup-pair inspections, and cusp/reversal inspections. It never exposes a
/// partial geometry result; callers publish only after every shared charge has succeeded.
#[derive(Clone, Debug, PartialEq)]
pub struct PathOffsetWork {
    limits: PathOffsetLimits,
    emitted_segments: usize,
    retained_components: usize,
    cleanup_pairs: usize,
    cusp_inspections: usize,
}

impl PathOffsetWork {
    /// Creates an empty request-wide work budget with the accepted finite offset limits.
    ///
    /// # Errors
    ///
    /// Returns `curve.offset.limit` when a configured bound or fitting tolerance is invalid.
    pub fn new(limits: PathOffsetLimits) -> Result<Self, CurveError> {
        validate_offset_limits(limits)?;
        Ok(Self {
            limits,
            emitted_segments: 0,
            retained_components: 0,
            cleanup_pairs: 0,
            cusp_inspections: 0,
        })
    }

    /// Returns the immutable limits shared by every construction in this request.
    pub const fn limits(&self) -> PathOffsetLimits {
        self.limits
    }
}

impl Default for PathOffsetLimits {
    /// Returns the fixed Stage 20J request-wide resource defaults.
    fn default() -> Self {
        Self {
            maximum_subdivision_depth: MAX_PATH_OFFSET_SUBDIVISION_DEPTH,
            maximum_segments: MAX_PATH_OFFSET_SEGMENTS,
            maximum_components: MAX_PATH_OFFSET_COMPONENTS,
            maximum_cleanup_pairs: MAX_PATH_OFFSET_CLEANUP_PAIRS,
            maximum_cusp_isolation_work: MAX_PATH_OFFSET_CUSP_ISOLATION_WORK,
            tolerance: DEFAULT_PATH_OFFSET_TOLERANCE,
        }
    }
}

/// Validates fixed and additive path-offset bounds before one work budget can be created.
///
/// # Errors
///
/// Returns the existing stable offset-limit diagnostic for invalid caller limits.
fn validate_offset_limits(limits: PathOffsetLimits) -> Result<(), CurveError> {
    if limits.maximum_subdivision_depth == 0
        || limits.maximum_segments == 0
        || limits.maximum_components == 0
        || limits.maximum_cleanup_pairs == 0
        || limits.maximum_cusp_isolation_work == 0
        || !limits.tolerance.is_finite()
        || limits.tolerance <= 0.0
    {
        return Err(CurveError::new(
            "curve.offset.limit",
            "path offset limits must be positive",
        ));
    }
    Ok(())
}

/// Immutable input for one cancellable signed normal-offset construction.
#[derive(Clone, Copy)]
pub struct PathOffsetRequest<'a> {
    /// Supplies the immutable source centerline.
    pub path: &'a CurvePath,
    /// Selects the left/right signed document-space distance.
    pub signed_distance: f64,
    /// Selects whether open endpoints are preserved or planning-extended.
    pub endpoint_policy: PathOffsetEndpointPolicy,
    /// Names the accepted cleanup contract without permitting renderer topology work.
    pub cleanup: PathOffsetCleanup,
    /// Supplies nearer same-side open offsets that the new candidate may touch but never cross.
    pub crossing_barriers: &'a [&'a CurvePath],
    /// Limits all derived geometry before it can be published.
    pub limits: PathOffsetLimits,
}

/// One ordered compact component retained after offset construction and cleanup.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetPathComponent {
    /// Identifies this component within its signed repetition index.
    pub component_ordinal: u32,
    /// Retains the earliest source location contributing to this component.
    pub source_start: PathLocation,
    /// Retains the final source location contributing to this component.
    pub source_end: PathLocation,
    /// Stores reusable line/cubic centerline geometry without thickness primitives.
    pub path: CurvePath,
    /// Identifies exact relink nodes that remain construction switches on the next offset rank.
    pub planar_switch_nodes: Vec<Point2>,
}

/// Complete atomic outcome of one offset construction.
#[derive(Clone, Debug, PartialEq)]
pub enum PathOffsetResult {
    /// Publishes ordered compact centerline components.
    Paths(Vec<OffsetPathComponent>),
    /// Reports that all finite source motion collapsed under the requested offset.
    Collapsed,
}

/// One compact derived segment paired with the exact source interval used to construct it.
#[derive(Clone, Copy, Debug)]
struct TracedOffsetSegment {
    segment: CurveSegment,
    source_start: PathLocation,
    source_end: PathLocation,
    orientation: Option<OffsetOrientation>,
}

/// One cleaned component with a source interval that is never reconstructed from output bounds.
#[derive(Clone, Debug)]
struct CleanedOffsetPath {
    path: CurvePath,
    source_start: PathLocation,
    source_end: PathLocation,
    earliest_source: PathLocation,
    planar_switch_nodes: Vec<Point2>,
}

/// One endpoint-extended construction path plus its exact authored-source interval per segment.
struct ExtendedOffsetSource {
    path: CurvePath,
    source_intervals: Vec<(PathLocation, PathLocation)>,
}

/// The nonzero winding direction retained while dissolving closed reversal loops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindingDirection {
    Positive,
    Negative,
}

/// Shared request-wide cleanup work accounting across recursive component traversal.
struct CleanupBudget {
    examined_pairs: usize,
    maximum_pairs: usize,
}

/// Shared request-wide cusp/reversal work accounting across every cubic source interval.
struct CuspIsolationBudget {
    examined_intervals: usize,
    maximum_intervals: usize,
}

/// One source-adjacent run that may be joined and cleaned without crossing an omitted interval.
#[derive(Clone, Debug)]
struct OrderedOffsetRun {
    segments: Vec<TracedOffsetSegment>,
    first_source_segment: CurveSegment,
    last_source_segment: CurveSegment,
}

/// One dyadic source-cubic interval with a proven monotonic offset orientation.
#[derive(Clone, Copy, Debug)]
struct RetainedCubicInterval {
    source_start: f64,
    source_end: f64,
    orientation: OffsetOrientation,
}

/// Conservative signed classification of one cubic offset-orientation interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OffsetOrientation {
    Retained,
    Reversed,
    Uncertain,
}

/// Conservative bounds for the signed offset-orientation numerator and source speed.
#[derive(Clone, Copy, Debug)]
struct OffsetOrientationBounds {
    lower: f64,
    upper: f64,
    minimum_speed: f64,
}

/// Identifies two compact segments, their parameters, and the exact point of one transverse crossing.
type TransverseCrossing = (usize, usize, f64, f64, Point2);

/// Identifies two ordered runs, their compact segments, parameters, and one transverse crossing.
type CrossRunTransverseCrossing = (usize, usize, usize, usize, f64, f64, Point2);

/// Locates one candidate crossing in exact ordered-run traversal coordinates.
#[derive(Clone, Copy, Debug)]
struct RunBarrierCrossing {
    run_index: usize,
    segment_index: usize,
    parameter: f64,
    point: Point2,
}

/// Builds ordered compact signed normal offsets without handing construction work to renderers.
///
/// # Errors
///
/// Returns stable finite, tangent, cancellation, or configured-limit diagnostics without a partial result.
pub fn offset_path_cancellable(
    request: PathOffsetRequest<'_>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<PathOffsetResult, CurveError> {
    let mut work = PathOffsetWork::new(request.limits)?;
    offset_path_with_work_cancellable(request, &mut work, is_cancelled)
}

/// Advances one planar Constant-gap frontier while preserving solved crossing-switch authority.
///
/// Authored source corners intersect their adjacent offset tangents into one bounded vector node.
/// Nodes supplied in `planar_switch_nodes` are exact intersections created by the preceding rank,
/// so their adjacent segments remain separate offset runs until this rank solves and relinks their
/// displaced crossing. This prevents either node class from becoming a renderer-like round hook.
///
/// # Errors
///
/// Returns the bounded path-offset diagnostics without publishing a partial next-rank frontier.
pub fn advance_planar_constant_gap_frontier_cancellable(
    path: &CurvePath,
    planar_switch_nodes: &[Point2],
    signed_distance: f64,
    limits: PathOffsetLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<PathOffsetResult, CurveError> {
    let request = PathOffsetRequest {
        path,
        signed_distance,
        endpoint_policy: PathOffsetEndpointPolicy::Preserve,
        cleanup: PathOffsetCleanup::PlanarConstantGap,
        crossing_barriers: &[],
        limits,
    };
    let mut work = PathOffsetWork::new(limits)?;
    offset_path_with_work_join_policy_cancellable(
        request,
        &mut work,
        is_cancelled,
        OffsetJoinPolicy::PlanarVector,
        planar_switch_nodes,
    )
}

/// Stabilizes terminal Constant-gap curls while preserving every existing vector node.
///
/// Offset cleanup and later canvas-envelope clipping can each expose a cubic prefix whose fitted
/// controls pass a shared node and curl back into it. This bounded operation retains the fixed node,
/// truncates only a proven sub-tolerance terminal backtrack at its exact derivative root, and
/// extends the valid incoming tangent to the same node. A curled segment whose endpoints are
/// already within fitting tolerance is pruned instead of publishing stationary geometry. It is
/// safe to apply after clipping because it preserves path closure and every retained vector node.
///
/// # Errors
///
/// Returns invalid limit, finite tangent, split, cubic-construction, or canonical path diagnostics
/// without publishing a partially stabilized path.
pub fn stabilize_planar_constant_gap_path(
    path: &CurvePath,
    limits: PathOffsetLimits,
) -> Result<CurvePath, CurveError> {
    validate_offset_limits(limits)?;
    stabilize_planar_terminal_curls_at_fixed_nodes(path, limits.tolerance)
}

/// Inserts exact shared vector nodes at transverse crossings between distinct centerlines.
///
/// The operation preserves every input path and branch in stored order. It changes only segment
/// subdivision, using each solved intersection coordinate as the exact endpoint on both paths, so
/// downstream renderers do not receive a gap-producing deletion policy. Self-crossing offset
/// cleanup remains the responsibility of `offset_path_cancellable` before this arrangement step.
///
/// # Errors
///
/// Returns stable path, intersection, cancellation, pair-work, or segment-limit diagnostics
/// without exposing a partially planarized collection.
pub fn insert_solved_crossing_nodes_cancellable(
    paths: &[CurvePath],
    limits: PathOffsetLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CurvePath>, CurveError> {
    validate_offset_limits(limits)?;
    const MAXIMUM_SOLVED_NODE_PASSES: usize = 16;
    let mut planarized = paths.to_vec();
    let mut examined_pairs = 0_usize;
    for _ in 0..MAXIMUM_SOLVED_NODE_PASSES {
        planarized = planarized
            .iter()
            .map(|path| stabilize_planar_terminal_curls_at_fixed_nodes(path, limits.tolerance))
            .collect::<Result<Vec<_>, _>>()?;
        let insertions =
            collect_solved_crossing_nodes(&planarized, limits, &mut examined_pairs, is_cancelled)?;
        if insertions.iter().all(Vec::is_empty) {
            return Ok(planarized);
        }
        let previous_segment_count = planarized
            .iter()
            .map(|path| path.segments().len())
            .sum::<usize>();
        let mut emitted_segments = 0_usize;
        let rebuilt = planarized
            .iter()
            .zip(insertions)
            .map(|(path, nodes)| {
                insert_solved_nodes_into_path(path, nodes, limits, &mut emitted_segments)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let rebuilt_segment_count = rebuilt
            .iter()
            .map(|path| path.segments().len())
            .sum::<usize>();
        if rebuilt_segment_count <= previous_segment_count {
            return Err(CurveError::new(
                "curve.offset.cleanup_limit",
                "solved crossing nodes did not converge within the cleanup limit",
            ));
        }
        planarized = rebuilt;
    }
    Err(CurveError::new(
        "curve.offset.cleanup_limit",
        "solved crossing nodes did not converge within the cleanup limit",
    ))
}

/// Removes a terminal curl while preserving its already solved shared node.
///
/// Crossing insertion can split a cubic close enough to a cusp that the retained prefix passes its
/// fixed intersection node and curls back into it. The crossing coordinate belongs to every path
/// in the arrangement and therefore cannot move. For a proven positive-to-negative terminal
/// projection, this operation splits at the exact derivative root and extends the valid prefix
/// tangent back to the same fixed node. A sub-tolerance chord is removed and its preceding segment
/// is extended to the fixed node, avoiding a zero-length repaired cubic. The operation does not
/// change path closure or ordinary corners that preserve forward progress through their preceding
/// segment.
///
/// # Errors
///
/// Returns finite tangent, split, cubic-construction, or canonical path diagnostics without
/// publishing a partially stabilized path.
fn stabilize_planar_terminal_curls_at_fixed_nodes(
    path: &CurvePath,
    tolerance: f64,
) -> Result<CurvePath, CurveError> {
    let mut segments = path.segments().to_vec();
    let mut index = 0_usize;
    while index + 1 < segments.len() {
        let original = segments[index];
        let fixed_node = original.end();
        let chord = (fixed_node.x - original.start().x).hypot(fixed_node.y - original.start().y);
        let continuation = match segments[index + 1].limiting_unit_tangent_at(0.0) {
            Ok(tangent) => tangent,
            Err(error) if error.path() == "curve.path.tangent.stationary" => {
                index += 1;
                continue;
            }
            Err(error) => return Err(error),
        };
        let Some(parameter) = terminal_forward_loss_parameter(original, continuation)? else {
            index += 1;
            continue;
        };
        if chord <= tolerance {
            if index > 0 {
                segments[index - 1] =
                    replace_planar_growth_segment_end(segments[index - 1], fixed_node, tolerance)?;
            }
            segments.remove(index);
            index = index.saturating_sub(1);
            continue;
        }
        let CurveSegment::CubicBezier(cubic) = original else {
            index += 1;
            continue;
        };
        let (prefix, _) = cubic.split(parameter)?;
        let stabilized = replace_planar_growth_segment_end(
            CurveSegment::CubicBezier(prefix),
            fixed_node,
            tolerance,
        )?;
        segments[index] = stabilized;
        index += 1;
    }
    CurvePath::new(segments, path.closure())
}

/// Moves a planar-growth endpoint while retaining one usable incoming cubic handle.
///
/// Relinking normally translates the endpoint and adjacent control together. When that control is
/// exactly stationary at the old endpoint, the translation remains stationary and the next offset
/// rank cannot recover a derivative. This helper retains the pre-move limiting direction and gives
/// an otherwise stationary terminal handle a bounded length no greater than the fitting tolerance
/// or one third of the repaired chord. Lines and already nonstationary cubics use the ordinary
/// tangent-preserving replacement unchanged.
///
/// # Errors
///
/// Returns limiting-tangent or finite segment-construction diagnostics without publishing a
/// partially moved segment.
fn replace_planar_growth_segment_end(
    segment: CurveSegment,
    end: Point2,
    tolerance: f64,
) -> Result<CurveSegment, CurveError> {
    let incoming = segment.limiting_unit_tangent_at(1.0)?;
    let moved = replace_segment_end_preserving_tangent(segment, end)?;
    let CurveSegment::CubicBezier(cubic) = moved else {
        return Ok(moved);
    };
    if cubic.control_2() != cubic.end() {
        return Ok(moved);
    }
    let chord = (end.x - cubic.start().x).hypot(end.y - cubic.start().y);
    let handle_length = tolerance.min(chord / 3.0);
    if handle_length == 0.0 {
        return Ok(moved);
    }
    Ok(CurveSegment::CubicBezier(crate::CubicBezierSegment::new(
        cubic.start(),
        cubic.control_1(),
        Point2::new(
            end.x - incoming.x * handle_length,
            end.y - incoming.y * handle_length,
        ),
        end,
    )?))
}

/// Moves a planar-growth startpoint while retaining one usable outgoing cubic handle.
///
/// This is the start-side reciprocal of `replace_planar_growth_segment_end`. A translated cubic
/// whose first control remains stationary receives a bounded handle in its original one-sided
/// direction, allowing the next offset rank to evaluate the reconstructed tangent intersection.
/// Lines and already nonstationary cubics retain their ordinary tangent-preserving replacement.
///
/// # Errors
///
/// Returns limiting-tangent or finite segment-construction diagnostics without publishing a
/// partially moved segment.
fn replace_planar_growth_segment_start(
    segment: CurveSegment,
    start: Point2,
    tolerance: f64,
) -> Result<CurveSegment, CurveError> {
    let outgoing = segment.limiting_unit_tangent_at(0.0)?;
    let moved = replace_segment_start_preserving_tangent(segment, start)?;
    let CurveSegment::CubicBezier(cubic) = moved else {
        return Ok(moved);
    };
    if cubic.control_1() != cubic.start() {
        return Ok(moved);
    }
    let chord = (cubic.end().x - start.x).hypot(cubic.end().y - start.y);
    let handle_length = tolerance.min(chord / 3.0);
    if handle_length == 0.0 {
        return Ok(moved);
    }
    Ok(CurveSegment::CubicBezier(crate::CubicBezierSegment::new(
        start,
        Point2::new(
            start.x + outgoing.x * handle_length,
            start.y + outgoing.y * handle_length,
        ),
        cubic.control_2(),
        cubic.end(),
    )?))
}

/// Collects every transverse crossing that is not already a stored endpoint on each path.
///
/// Segment bounds reject irrelevant work before the exact line/cubic intersection solver runs.
/// The caller owns one cumulative pair budget across convergence passes.
///
/// # Errors
///
/// Returns bounded intersection, cancellation, location, or arithmetic diagnostics without
/// mutating the supplied paths or insertion lists.
fn collect_solved_crossing_nodes(
    paths: &[CurvePath],
    limits: PathOffsetLimits,
    examined_pairs: &mut usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<SolvedCrossingNode>>, CurveError> {
    let path_bounds = paths
        .iter()
        .map(CurvePath::bounds)
        .collect::<Result<Vec<_>, _>>()?;
    let segment_bounds = paths
        .iter()
        .map(|path| {
            path.segments()
                .iter()
                .map(CurveSegment::bounds)
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut insertions = vec![Vec::<SolvedCrossingNode>::new(); paths.len()];
    for first_path_index in 0..paths.len() {
        for second_path_index in (first_path_index + 1)..paths.len() {
            if is_cancelled() {
                return Err(cancelled());
            }
            if !offset_bounds_overlap(
                path_bounds[first_path_index],
                path_bounds[second_path_index],
                limits.tolerance,
            )? {
                continue;
            }
            for (first_segment_index, first_segment) in paths[first_path_index]
                .segments()
                .iter()
                .copied()
                .enumerate()
            {
                for (second_segment_index, second_segment) in paths[second_path_index]
                    .segments()
                    .iter()
                    .copied()
                    .enumerate()
                {
                    if !offset_bounds_overlap(
                        segment_bounds[first_path_index][first_segment_index],
                        segment_bounds[second_path_index][second_segment_index],
                        limits.tolerance,
                    )? {
                        continue;
                    }
                    *examined_pairs = examined_pairs.checked_add(1).ok_or(CurveError::new(
                        "curve.offset.cleanup_limit",
                        "path offset cleanup pair limit exceeded",
                    ))?;
                    if *examined_pairs > limits.maximum_cleanup_pairs {
                        return Err(CurveError::new(
                            "curve.offset.cleanup_limit",
                            "path offset cleanup pair limit exceeded",
                        ));
                    }
                    if is_cancelled() {
                        return Err(cancelled());
                    }
                    let contacts = match first_segment.intersections(&second_segment) {
                        Ok(contacts) => contacts,
                        Err(error) if error.path() == "curve.path.intersections.overlap" => {
                            continue;
                        }
                        Err(error) => return Err(error),
                    };
                    for contact in contacts {
                        if contact.kind() != crate::IntersectionKind::Crossing {
                            continue;
                        }
                        if solved_contact_coincides_with_shared_endpoint(
                            first_segment,
                            second_segment,
                            contact.point(),
                            limits.tolerance,
                        )? {
                            continue;
                        }
                        push_solved_crossing_node(
                            &mut insertions[first_path_index],
                            PathLocation::new(first_segment_index, contact.first_parameter())?,
                            contact.point(),
                            first_segment,
                        )?;
                        push_solved_crossing_node(
                            &mut insertions[second_path_index],
                            PathLocation::new(second_segment_index, contact.second_parameter())?,
                            contact.point(),
                            second_segment,
                        )?;
                    }
                }
            }
        }
    }
    Ok(insertions)
}

/// Recognizes a sub-tolerance contact already represented by one exact shared endpoint node.
///
/// Adaptive intersection refinement can report an infinite-looking sequence of transverse
/// contacts converging on a common cubic endpoint. The offset fitter cannot distinguish topology
/// below its declared tolerance, so a contact inside that tolerance belongs to the exact shared
/// node only when both segments already store that same endpoint. Nearby contacts without a
/// shared endpoint remain independent and are still inserted.
///
/// # Errors
///
/// Returns finite point-distance or coincidence diagnostics without changing either segment.
fn solved_contact_coincides_with_shared_endpoint(
    first: CurveSegment,
    second: CurveSegment,
    contact: Point2,
    tolerance: f64,
) -> Result<bool, CurveError> {
    for first_endpoint in [first.start(), first.end()] {
        for second_endpoint in [second.start(), second.end()] {
            if crate::curves::coincident(first_endpoint, second_endpoint)?
                && crate::curves::distance(contact, first_endpoint)? <= tolerance
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Tests conservative offset bounds for overlap under the caller's finite tolerance.
///
/// # Errors
///
/// Returns a stable numeric diagnostic when tolerance expansion cannot remain finite.
fn offset_bounds_overlap(
    first: Bounds,
    second: Bounds,
    tolerance: f64,
) -> Result<bool, CurveError> {
    let first_max_x = first.max.x + tolerance;
    let first_max_y = first.max.y + tolerance;
    let second_max_x = second.max.x + tolerance;
    let second_max_y = second.max.y + tolerance;
    if !first_max_x.is_finite()
        || !first_max_y.is_finite()
        || !second_max_x.is_finite()
        || !second_max_y.is_finite()
    {
        return Err(CurveError::new(
            "curve.offset.numeric_overflow",
            "path offset bounds arithmetic must remain finite",
        ));
    }
    Ok(first.min.x <= second_max_x
        && first_max_x >= second.min.x
        && first.min.y <= second_max_y
        && first_max_y >= second.min.y)
}

/// One solved path-local insertion backed by a shared intersection coordinate.
#[derive(Clone, Copy, Debug)]
struct SolvedCrossingNode {
    segment_index: usize,
    parameter: f64,
    point: Point2,
}

/// Records an interior solved crossing while recognizing an already stored vector node.
///
/// # Errors
///
/// Returns finite point-coincidence diagnostics without changing the insertion list.
fn push_solved_crossing_node(
    nodes: &mut Vec<SolvedCrossingNode>,
    location: PathLocation,
    point: Point2,
    segment: CurveSegment,
) -> Result<(), CurveError> {
    if strictly_interior(location.parameter())
        && !crate::curves::coincident(point, segment.start())?
        && !crate::curves::coincident(point, segment.end())?
    {
        nodes.push(SolvedCrossingNode {
            segment_index: location.segment_index(),
            parameter: location.parameter(),
            point,
        });
    }
    Ok(())
}

/// Rebuilds one path with every solved crossing inserted in source traversal order.
///
/// # Errors
///
/// Returns finite subdivision, continuity, or cumulative segment-limit diagnostics before the
/// caller can publish any path collection.
fn insert_solved_nodes_into_path(
    path: &CurvePath,
    mut nodes: Vec<SolvedCrossingNode>,
    limits: PathOffsetLimits,
    emitted_segments: &mut usize,
) -> Result<CurvePath, CurveError> {
    nodes.sort_by(|left, right| {
        left.segment_index
            .cmp(&right.segment_index)
            .then_with(|| left.parameter.total_cmp(&right.parameter))
    });
    nodes.dedup_by(|right, left| {
        right.segment_index == left.segment_index
            && (right.parameter - left.parameter).abs() <= 1.0e-9
    });
    let mut rebuilt = Vec::new();
    let mut node_index = 0_usize;
    for (segment_index, segment) in path.segments().iter().copied().enumerate() {
        let mut remainder = segment;
        let mut previous_parameter = 0.0;
        while node_index < nodes.len() && nodes[node_index].segment_index == segment_index {
            let node = nodes[node_index];
            node_index += 1;
            let local_parameter =
                (node.parameter - previous_parameter) / (1.0 - previous_parameter);
            if !strictly_interior(local_parameter)
                || node.point == remainder.start()
                || node.point == remainder.end()
            {
                previous_parameter = node.parameter;
                continue;
            }
            let (prefix, suffix) =
                split_curve_segment_at_solved_point(remainder, local_parameter, node.point)?;
            rebuilt.push(prefix);
            remainder = suffix;
            previous_parameter = node.parameter;
        }
        rebuilt.push(remainder);
    }
    *emitted_segments = emitted_segments
        .checked_add(rebuilt.len())
        .ok_or(CurveError::new(
            "curve.offset.segment_limit",
            "path offset segment limit exceeded",
        ))?;
    if *emitted_segments > limits.maximum_segments {
        return Err(CurveError::new(
            "curve.offset.segment_limit",
            "path offset segment limit exceeded",
        ));
    }
    CurvePath::new(rebuilt, path.closure())
}

/// Splits one line or cubic at a solved crossing and forces one identical shared endpoint.
///
/// # Errors
///
/// Returns finite segment-construction diagnostics without publishing either partial piece.
fn split_curve_segment_at_solved_point(
    segment: CurveSegment,
    parameter: f64,
    point: Point2,
) -> Result<(CurveSegment, CurveSegment), CurveError> {
    match segment {
        CurveSegment::Line(line) => Ok((
            CurveSegment::Line(LineSegment::new(line.start(), point)?),
            CurveSegment::Line(LineSegment::new(point, line.end())?),
        )),
        CurveSegment::CubicBezier(cubic) => {
            let (prefix, suffix) = cubic.split(parameter)?;
            Ok((
                CurveSegment::CubicBezier(replace_cubic_end(prefix, point)?),
                CurveSegment::CubicBezier(replace_cubic_start(suffix, point)?),
            ))
        }
    }
}

/// Builds one offset while charging a caller-owned mutable request-wide work budget.
///
/// The request must use the budget's exact immutable limits, so independently invoked paths
/// cannot reset candidate segment, component, cleanup, or cusp counters.
///
/// # Errors
///
/// Returns existing stable offset diagnostics and never publishes partial geometry.
pub fn offset_path_with_work_cancellable(
    request: PathOffsetRequest<'_>,
    work: &mut PathOffsetWork,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<PathOffsetResult, CurveError> {
    let join_policy = match request.cleanup {
        PathOffsetCleanup::DissolveCrossings => OffsetJoinPolicy::CompactRound,
        PathOffsetCleanup::PlanarConstantGap => OffsetJoinPolicy::PlanarVector,
    };
    offset_path_with_work_join_policy_cancellable(request, work, is_cancelled, join_policy, &[])
}

/// Builds a closed-region offset with subdivided circular joins at finite outward corners.
///
/// This crate-private seam is geometry authority for filled regions only; the public Stage 20J
/// path-offset request retains its accepted compact-round outer-join behavior.
///
/// # Errors
///
/// Returns the same atomic cancellation, bounded-work, and geometry diagnostics as the public
/// offset primitive without publishing a partial component list.
pub(crate) fn offset_path_with_work_region_round_cancellable(
    request: PathOffsetRequest<'_>,
    work: &mut PathOffsetWork,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<PathOffsetResult, CurveError> {
    offset_path_with_work_join_policy_cancellable(
        request,
        work,
        is_cancelled,
        OffsetJoinPolicy::RegionRound,
        &[],
    )
}

/// Selects the geometry-owned construction policy for source-adjacent offset corners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OffsetJoinPolicy {
    /// Retains accepted compact round and deterministic bevel behavior for path products.
    CompactRound,
    /// Intersects adjacent offset tangents into one exact vector node for Constant-gap paths.
    PlanarVector,
    /// Retains inward tangent intersections and splits outward circular joins for filled regions.
    RegionRound,
}

/// Builds one offset through the shared bounded implementation with an explicit private join policy.
///
/// # Errors
///
/// Returns existing stable offset diagnostics and never publishes partial geometry.
fn offset_path_with_work_join_policy_cancellable(
    request: PathOffsetRequest<'_>,
    work: &mut PathOffsetWork,
    is_cancelled: &dyn Fn() -> bool,
    join_policy: OffsetJoinPolicy,
    planar_switch_nodes: &[Point2],
) -> Result<PathOffsetResult, CurveError> {
    if !request.signed_distance.is_finite() {
        return Err(CurveError::new(
            "curve.offset.distance",
            "path offset distance must be finite",
        ));
    }
    validate_offset_limits(request.limits)?;
    if request.limits != work.limits {
        return Err(CurveError::new(
            "curve.offset.limit",
            "shared path offset work requires matching request limits",
        ));
    }
    if is_cancelled() {
        return Err(cancelled());
    }
    let source_bounds = request.path.bounds()?;
    if source_bounds.min == source_bounds.max {
        return Ok(PathOffsetResult::Collapsed);
    }
    let source = extend_endpoints(request.path, request.endpoint_policy)?;
    let authored_start = PathLocation::new(0, 0.0)?;
    let authored_end = PathLocation::new(request.path.segments().len() - 1, 1.0)?;
    if request.signed_distance == 0.0 {
        return Ok(PathOffsetResult::Paths(vec![OffsetPathComponent {
            component_ordinal: 0,
            source_start: authored_start,
            source_end: authored_end,
            path: source.path,
            planar_switch_nodes: planar_switch_nodes.to_vec(),
        }]));
    }
    let limits = work.limits;
    let retained_winding = if request.path.closure() == PathClosure::Closed {
        Some(
            path_winding(request.path, limits.tolerance)?.ok_or(CurveError::new(
                "curve.offset.winding",
                "closed path offset requires a nonzero authored winding",
            ))?,
        )
    } else {
        None
    };
    let mut runs = Vec::with_capacity(source.path.segments().len().saturating_mul(2));
    let mut emitted_segments = work.emitted_segments;
    let mut cusp_budget = CuspIsolationBudget {
        examined_intervals: work.cusp_inspections,
        maximum_intervals: limits.maximum_cusp_isolation_work,
    };
    for (index, segment) in source.path.segments().iter().copied().enumerate() {
        if is_cancelled() {
            return Err(cancelled());
        }
        let offset_runs = offset_segment_runs(
            segment,
            index,
            request.signed_distance,
            limits,
            request.cleanup == PathOffsetCleanup::PlanarConstantGap,
            &mut cusp_budget,
            is_cancelled,
        )?;
        let source_interval = source.source_intervals[index];
        for mut offsets in offset_runs {
            for offset in &mut offsets {
                offset.source_start =
                    remap_extended_source_location(offset.source_start, index, source_interval)?;
                offset.source_end =
                    remap_extended_source_location(offset.source_end, index, source_interval)?;
            }
            append_ordered_offset_run(
                &mut runs,
                offsets,
                segment,
                request.signed_distance,
                join_policy,
                planar_switch_nodes,
                limits.tolerance,
                limits.maximum_segments,
                &mut emitted_segments,
            )?;
        }
    }
    if runs.is_empty() {
        work.emitted_segments = emitted_segments;
        work.cusp_inspections = cusp_budget.examined_intervals;
        return Ok(PathOffsetResult::Collapsed);
    }
    if request.cleanup == PathOffsetCleanup::PlanarConstantGap {
        reconnect_collapsed_source_run_gaps(
            &mut runs,
            request.path,
            request.signed_distance,
            join_policy,
            limits.tolerance,
            limits.maximum_segments,
            &mut emitted_segments,
        )?;
        relink_planar_reversed_spans(&mut runs, request.signed_distance, limits.tolerance)?;
        relink_planar_terminal_curls(&mut runs, request.signed_distance, limits.tolerance)?;
        relink_planar_backtracking_spans(
            &mut runs,
            request.signed_distance,
            limits.tolerance,
            planar_switch_nodes,
        )?;
    }
    let mut run_closure = PathClosure::Open;
    if source.path.closure() == PathClosure::Closed {
        if runs.len() == 1 {
            close_unbroken_offset_run(
                &mut runs[0],
                authored_end,
                request.signed_distance,
                join_policy,
                limits.maximum_segments,
                &mut emitted_segments,
            )?;
            run_closure = PathClosure::Closed;
        } else {
            merge_closed_seam_runs(
                &mut runs,
                authored_end,
                request.signed_distance,
                join_policy,
                limits.tolerance,
                limits.maximum_segments,
                &mut emitted_segments,
            )?;
        }
    }
    let mut cleanup_budget = CleanupBudget {
        examined_pairs: work.cleanup_pairs,
        maximum_pairs: limits.maximum_cleanup_pairs,
    };
    let mut preserved_closed_runs = Vec::new();
    let mut derived_planar_switch_nodes = Vec::new();
    dissolve_cross_run_reversal_crossings(
        &mut runs,
        authored_start,
        authored_end,
        request.cleanup == PathOffsetCleanup::PlanarConstantGap,
        &mut preserved_closed_runs,
        &mut derived_planar_switch_nodes,
        limits.tolerance,
        &mut cleanup_budget,
        is_cancelled,
    )?;
    if request.path.closure() == PathClosure::Open
        && runs.len() > 1
        && !request.crossing_barriers.is_empty()
    {
        dissolve_crossing_barrier_middle(
            &mut runs,
            authored_start,
            authored_end,
            request.crossing_barriers,
            &mut cleanup_budget,
            is_cancelled,
        )?;
    }
    let mut paths = Vec::new();
    for run in runs {
        let closure = if run_closure == PathClosure::Closed {
            PathClosure::Closed
        } else {
            PathClosure::Open
        };
        let dissolve_coincident_overlaps = request.cleanup == PathOffsetCleanup::PlanarConstantGap
            || (join_policy == OffsetJoinPolicy::RegionRound && request.signed_distance > 0.0);
        let mut cleaned = dissolve_crossings_with_budget(
            run.segments,
            closure,
            (closure == PathClosure::Closed)
                .then_some(retained_winding)
                .flatten(),
            &mut cleanup_budget,
            limits.maximum_components,
            limits.tolerance,
            dissolve_coincident_overlaps,
            request.cleanup == PathOffsetCleanup::PlanarConstantGap,
            &derived_planar_switch_nodes,
            is_cancelled,
        )?;
        paths.append(&mut cleaned);
        if work
            .retained_components
            .checked_add(paths.len())
            .is_none_or(|total| total > limits.maximum_components)
        {
            return Err(CurveError::new(
                "curve.offset.component_limit",
                "path offset component limit exceeded",
            ));
        }
    }
    for segments in preserved_closed_runs {
        let mut cleaned = dissolve_crossings_with_budget(
            segments,
            PathClosure::Closed,
            None,
            &mut cleanup_budget,
            limits.maximum_components,
            limits.tolerance,
            true,
            true,
            &derived_planar_switch_nodes,
            is_cancelled,
        )?;
        paths.append(&mut cleaned);
        if work
            .retained_components
            .checked_add(paths.len())
            .is_none_or(|total| total > limits.maximum_components)
        {
            return Err(CurveError::new(
                "curve.offset.component_limit",
                "path offset component limit exceeded",
            ));
        }
    }
    if paths.is_empty() {
        work.emitted_segments = emitted_segments;
        work.cusp_inspections = cusp_budget.examined_intervals;
        work.cleanup_pairs = cleanup_budget.examined_pairs;
        return Ok(PathOffsetResult::Collapsed);
    }
    if request.cleanup == PathOffsetCleanup::PlanarConstantGap {
        for component in &mut paths {
            component.path =
                stabilize_planar_terminal_curls_at_fixed_nodes(&component.path, limits.tolerance)?;
        }
    }
    paths.sort_by(|left, right| {
        source_location_key(left.earliest_source).cmp(&source_location_key(right.earliest_source))
    });
    work.emitted_segments = emitted_segments;
    work.cusp_inspections = cusp_budget.examined_intervals;
    work.cleanup_pairs = cleanup_budget.examined_pairs;
    work.retained_components =
        work.retained_components
            .checked_add(paths.len())
            .ok_or(CurveError::new(
                "curve.offset.component_limit",
                "path offset component limit exceeded",
            ))?;
    Ok(PathOffsetResult::Paths(
        paths
            .into_iter()
            .enumerate()
            .map(|(component_ordinal, component)| OffsetPathComponent {
                component_ordinal: u32::try_from(component_ordinal)
                    .expect("component limit fits u32"),
                source_start: component.source_start,
                source_end: component.source_end,
                path: component.path,
                planar_switch_nodes: component.planar_switch_nodes,
            })
            .collect(),
    ))
}

/// Relinks offset runs separated only by source segments that collapsed at the next rank.
///
/// An iterated Constant-gap frontier can contain a compact node join whose inward offset has no
/// positive-length locus. The two neighboring source edges still own one ordered node, so this
/// operation canonicalizes endpoints already coincident under the request's fitting tolerance or
/// applies the same analytic corner join used for skipped whole source segments. It never connects
/// unrelated cusp runs and never inserts a smoothing bridge.
///
/// # Errors
///
/// Returns finite join, source-location, or request-wide segment-limit diagnostics before a
/// partially relinked frontier can escape.
fn reconnect_collapsed_source_run_gaps(
    runs: &mut Vec<OrderedOffsetRun>,
    source: &CurvePath,
    distance: f64,
    join_policy: OffsetJoinPolicy,
    tolerance: f64,
    maximum_segments: usize,
    emitted_segments: &mut usize,
) -> Result<(), CurveError> {
    let mut index = 0_usize;
    while index + 1 < runs.len() {
        let previous_location = runs[index]
            .segments
            .last()
            .expect("ordered offset run is nonempty")
            .source_end;
        let next_location = runs[index + 1]
            .segments
            .first()
            .expect("ordered offset run is nonempty")
            .source_start;
        let skips_whole_source_segments = previous_location.parameter() == 1.0
            && next_location.parameter() == 0.0
            && previous_location
                .segment_index()
                .checked_add(1)
                .is_some_and(|next| next < next_location.segment_index());
        let skips_source_interval = source_location_key(previous_location)
            < source_location_key(next_location)
            && !source_locations_are_adjacent(previous_location, next_location);
        if !skips_source_interval {
            index += 1;
            continue;
        }
        let mut next = runs.remove(index + 1);
        let previous_segment = source.segments()[previous_location.segment_index()];
        let next_segment = source.segments()[next_location.segment_index()];
        let before = runs[index].segments.len();
        let end = runs[index]
            .segments
            .last()
            .expect("ordered offset run is nonempty")
            .segment
            .end();
        let start = next
            .segments
            .first()
            .expect("ordered offset run is nonempty")
            .segment
            .start();
        let joined = if (end.x - start.x).hypot(end.y - start.y) <= tolerance {
            next.segments[0].segment = replace_segment_start(next.segments[0].segment, end)?;
            true
        } else if skips_whole_source_segments {
            join_offset_segments(
                &mut runs[index].segments,
                &mut next.segments,
                previous_segment,
                next_segment,
                next_location,
                distance,
                join_policy,
            )?
        } else {
            false
        };
        if !joined {
            runs.insert(index + 1, next);
            index += 1;
            continue;
        }
        let added = runs[index].segments.len() - before;
        *emitted_segments = emitted_segments.checked_add(added).ok_or(CurveError::new(
            "curve.offset.segment_limit",
            "path offset segment limit exceeded",
        ))?;
        if *emitted_segments > maximum_segments {
            return Err(CurveError::new(
                "curve.offset.segment_limit",
                "path offset segment limit exceeded",
            ));
        }
        runs[index].segments.extend(next.segments);
        runs[index].last_source_segment = next.last_source_segment;
    }
    Ok(())
}

/// Relinks bounded orientation-reversed Constant-gap spans at one exact vector node.
///
/// A smooth source can develop two offset cusps before its reversed middle crosses itself. The
/// reversed span is not part of the outward wavefront, but crossing-only cleanup cannot remove it.
/// This operation preserves both neighboring forward branches and intersects their limiting
/// tangent lines only when the suffix reaches the solved node by tracing backward within the
/// existing miter bound; the prefix may extend or shorten to the same node. The exact result
/// remains an ordinary vector corner, so the next rank offsets its
/// adjacent segments and intersects their tangents rather than misclassifying it as a displaced
/// crossing switch. Unsolved or unbounded spans remain unchanged instead of being bridged
/// approximately.
///
/// # Errors
///
/// Returns finite tangent or canonical segment-construction diagnostics without publishing a
/// partially relinked run.
fn relink_planar_reversed_spans(
    runs: &mut [OrderedOffsetRun],
    signed_distance: f64,
    tolerance: f64,
) -> Result<(), CurveError> {
    let maximum_extension = signed_distance.abs() * 8.0 + tolerance;
    for run in runs {
        let mut scan = 0_usize;
        while let Some(reversed_index) = run.segments[scan..]
            .iter()
            .position(|segment| segment.orientation == Some(OffsetOrientation::Reversed))
            .map(|relative| scan + relative)
        {
            let mut reversed_start = reversed_index;
            while reversed_start > 0 && run.segments[reversed_start - 1].orientation.is_none() {
                reversed_start -= 1;
            }
            let mut retained_after = reversed_index + 1;
            while retained_after < run.segments.len()
                && run.segments[retained_after].orientation != Some(OffsetOrientation::Retained)
            {
                retained_after += 1;
            }
            if reversed_start == 0 || retained_after == run.segments.len() {
                scan = retained_after;
                continue;
            }
            let previous_index = reversed_start - 1;
            let previous = run.segments[previous_index].segment;
            let next = run.segments[retained_after].segment;
            let previous_tangent = match previous.limiting_unit_tangent_at(1.0) {
                Ok(tangent) => tangent,
                Err(error) if error.path() == "curve.path.tangent.stationary" => {
                    scan = retained_after;
                    continue;
                }
                Err(error) => return Err(error),
            };
            let next_tangent = match next.limiting_unit_tangent_at(0.0) {
                Ok(tangent) => tangent,
                Err(error) if error.path() == "curve.path.tangent.stationary" => {
                    scan = retained_after;
                    continue;
                }
                Err(error) => return Err(error),
            };
            let Some(node) = line_intersection(
                previous.end(),
                Point2::new(
                    previous.end().x + previous_tangent.x,
                    previous.end().y + previous_tangent.y,
                ),
                next.start(),
                Point2::new(
                    next.start().x + next_tangent.x,
                    next.start().y + next_tangent.y,
                ),
            ) else {
                scan = retained_after;
                continue;
            };
            let previous_distance = (node.x - previous.end().x).hypot(node.y - previous.end().y);
            let next_distance = (node.x - next.start().x).hypot(node.y - next.start().y);
            let next_travel = (node.x - next.start().x) * next_tangent.x
                + (node.y - next.start().y) * next_tangent.y;
            if next_travel > tolerance
                || previous_distance > maximum_extension
                || next_distance > maximum_extension
            {
                scan = retained_after;
                continue;
            }
            run.segments[previous_index].segment =
                replace_segment_end_preserving_tangent(previous, node)?;
            run.segments[retained_after].segment =
                replace_segment_start_preserving_tangent(next, node)?;
            run.segments.drain(reversed_start..retained_after);
            scan = reversed_start.saturating_add(1).min(run.segments.len());
        }
    }
    Ok(())
}

/// Truncates a fitted Constant-gap segment at its first terminal loss of forward progress.
///
/// Iterated offsets may feed a previously fitted cubic back into the next rank. Near a cusp, that
/// cubic can travel beyond its stored endpoint and curl backward before the following segment
/// resumes the same forward direction. This pass proves the curl from the cubic derivative
/// projected onto the following segment's outgoing tangent, splits at the exact quadratic root,
/// and discards only the terminal backtrack. The retained incoming tangent and reciprocal outgoing
/// tangent are then intersected to construct one bounded replacement vector node; both meeting
/// endpoints and their adjacent handles move to that exact point. When those lines have no bounded
/// solution, the derivative root remains the conservative shared node. Authored corners, lines,
/// and cubics without a positive-to-negative terminal projection remain unchanged; a propagated
/// crossing node is repaired only when its adjacent fitted cubic independently proves this curl.
///
/// # Errors
///
/// Returns finite derivative, split, or canonical segment-construction diagnostics without
/// partially relinking a run.
fn relink_planar_terminal_curls(
    runs: &mut [OrderedOffsetRun],
    signed_distance: f64,
    tolerance: f64,
) -> Result<(), CurveError> {
    let maximum_extension = signed_distance.abs() * 8.0 + tolerance;
    for run in runs {
        let mut index = 0_usize;
        while index + 1 < run.segments.len() {
            let previous = run.segments[index];
            let next = run.segments[index + 1];
            let continuation = match next.segment.limiting_unit_tangent_at(0.0) {
                Ok(tangent) => tangent,
                Err(error) if error.path() == "curve.path.tangent.stationary" => {
                    index += 1;
                    continue;
                }
                Err(error) => return Err(error),
            };
            let Some(parameter) = terminal_forward_loss_parameter(previous.segment, continuation)?
            else {
                index += 1;
                continue;
            };
            let forward_loss_node = previous.segment.point_at(parameter)?;
            let (mut prefix, _) = split_traced_segment(previous, parameter, forward_loss_node)?;
            let tangent_intersection = match prefix.segment.limiting_unit_tangent_at(1.0) {
                Ok(incoming) => line_intersection(
                    forward_loss_node,
                    Point2::new(
                        forward_loss_node.x + incoming.x,
                        forward_loss_node.y + incoming.y,
                    ),
                    next.segment.start(),
                    Point2::new(
                        next.segment.start().x + continuation.x,
                        next.segment.start().y + continuation.y,
                    ),
                )
                .filter(|intersection| {
                    let incoming_travel = (intersection.x - forward_loss_node.x) * incoming.x
                        + (intersection.y - forward_loss_node.y) * incoming.y;
                    let outgoing_travel = (intersection.x - next.segment.start().x)
                        * continuation.x
                        + (intersection.y - next.segment.start().y) * continuation.y;
                    let incoming_distance = (intersection.x - forward_loss_node.x)
                        .hypot(intersection.y - forward_loss_node.y);
                    let outgoing_distance = (intersection.x - next.segment.start().x)
                        .hypot(intersection.y - next.segment.start().y);
                    let retained_chord = (intersection.x - prefix.segment.start().x)
                        .hypot(intersection.y - prefix.segment.start().y);
                    let continuation_chord = (next.segment.end().x - intersection.x)
                        .hypot(next.segment.end().y - intersection.y);
                    incoming_travel >= -tolerance
                        && outgoing_travel <= tolerance
                        && incoming_distance <= maximum_extension
                        && outgoing_distance <= maximum_extension
                        && retained_chord > tolerance
                        && continuation_chord > tolerance
                }),
                Err(error) if error.path() == "curve.path.tangent.stationary" => None,
                Err(error) => return Err(error),
            };
            let vector_node = tangent_intersection.unwrap_or(forward_loss_node);
            if tangent_intersection.is_some() {
                prefix.segment =
                    replace_planar_growth_segment_end(prefix.segment, vector_node, tolerance)?;
            }
            run.segments[index] = prefix;
            run.segments[index + 1].segment =
                replace_planar_growth_segment_start(next.segment, vector_node, tolerance)?;
            index = index.saturating_sub(1);
        }
    }
    Ok(())
}

/// Finds the last positive-to-negative terminal projection root of one cubic derivative.
///
/// The continuation tangent supplies the local forward axis. A qualifying root must separate a
/// positive derivative interval from a negative final interval, which distinguishes an actual
/// terminal curl from an ordinary curved segment or authored corner. Quadratic Bernstein controls
/// are converted exactly to the power basis and solved with the shared finite root policy.
///
/// # Errors
///
/// Returns tangent and finite control-polygon diagnostics without accepting a non-finite root.
fn terminal_forward_loss_parameter(
    segment: CurveSegment,
    continuation: Vector2,
) -> Result<Option<f64>, CurveError> {
    let CurveSegment::CubicBezier(cubic) = segment else {
        return Ok(None);
    };
    let derivative_controls = [
        Vector2::new(
            3.0 * (cubic.control_1().x - cubic.start().x),
            3.0 * (cubic.control_1().y - cubic.start().y),
        ),
        Vector2::new(
            3.0 * (cubic.control_2().x - cubic.control_1().x),
            3.0 * (cubic.control_2().y - cubic.control_1().y),
        ),
        Vector2::new(
            3.0 * (cubic.end().x - cubic.control_2().x),
            3.0 * (cubic.end().y - cubic.control_2().y),
        ),
    ];
    let projection = derivative_controls.map(|control| control.dot(continuation));
    if projection.iter().any(|value| !value.is_finite()) {
        return Err(CurveError::new(
            "curve.offset.numeric_overflow",
            "terminal curl projection arithmetic overflowed",
        ));
    }
    let mut roots = quadratic_real_roots([
        projection[0] - 2.0 * projection[1] + projection[2],
        2.0 * (projection[1] - projection[0]),
        projection[0],
    ]);
    roots.retain(|parameter| strictly_interior(*parameter) && parameter.is_finite());
    roots.sort_by(f64::total_cmp);
    roots.dedup_by(|right, left| (*right - *left).abs() <= 1.0e-12);
    let mut boundaries = Vec::with_capacity(roots.len() + 2);
    boundaries.push(0.0);
    boundaries.extend(roots.iter().copied());
    boundaries.push(1.0);
    let scale = projection
        .iter()
        .copied()
        .map(f64::abs)
        .fold(0.0_f64, f64::max);
    let epsilon = scale * 128.0 * f64::EPSILON;
    let interval_projection = boundaries
        .windows(2)
        .map(|interval| {
            let parameter = (interval[0] + interval[1]) * 0.5;
            let inverse = 1.0 - parameter;
            projection[0] * inverse * inverse
                + 2.0 * projection[1] * inverse * parameter
                + projection[2] * parameter * parameter
        })
        .collect::<Vec<_>>();
    if interval_projection
        .last()
        .is_none_or(|value| *value >= -epsilon)
    {
        return Ok(None);
    }
    Ok(roots.iter().enumerate().rev().find_map(|(index, root)| {
        (interval_projection[index] > epsilon && interval_projection[index + 1] < -epsilon)
            .then_some(*root)
    }))
}

/// Relinks a propagated Constant-gap backtrack at one bounded tangent intersection.
///
/// A fold that survives one rank becomes ordinary source geometry on the next, so fresh offset
/// orientation alone cannot identify it. This pass detects a non-authored turn beyond ninety
/// degrees, skips known crossing-switch nodes, and searches a fixed local segment window for the
/// first forward-progressing suffix whose agreeing tangent reaches the shared node by tracing
/// backward. A source-authored boundary remains eligible only after the intervening geometry
/// proves a near-180-degree hairpin or contains an orientation-reversed interval. The prefix and
/// suffix retain their exact limiting tangents; only the intervening fold is removed. No unbounded,
/// parallel, ordinary authored-corner, or unresolved candidate is changed.
///
/// # Errors
///
/// Returns finite tangent or canonical segment-construction diagnostics without partially
/// relinking a run.
fn relink_planar_backtracking_spans(
    runs: &mut [OrderedOffsetRun],
    signed_distance: f64,
    tolerance: f64,
    planar_switch_nodes: &[Point2],
) -> Result<(), CurveError> {
    const MAXIMUM_BACKTRACK_LOOKAHEAD: usize = 64;
    let maximum_extension = signed_distance.abs() * 8.0 + tolerance;
    for run in runs {
        let mut index = 0_usize;
        while index + 2 < run.segments.len() {
            let previous = run.segments[index];
            let folded = run.segments[index + 1];
            let node = previous.segment.end();
            if planar_switch_nodes.contains(&node) {
                index += 1;
                continue;
            }
            let previous_tangent = match previous.segment.limiting_unit_tangent_at(1.0) {
                Ok(tangent) => tangent,
                Err(error) if error.path() == "curve.path.tangent.stationary" => {
                    index += 1;
                    continue;
                }
                Err(error) => return Err(error),
            };
            let folded_tangent = match folded.segment.limiting_unit_tangent_at(0.0) {
                Ok(tangent) => tangent,
                Err(error) if error.path() == "curve.path.tangent.stationary" => {
                    index += 1;
                    continue;
                }
                Err(error) => return Err(error),
            };
            let initial_turn = previous_tangent.dot(folded_tangent);
            if initial_turn >= 0.0 {
                index += 1;
                continue;
            }
            let search_end = run
                .segments
                .len()
                .min(index.saturating_add(MAXIMUM_BACKTRACK_LOOKAHEAD));
            let mut replacement = None;
            let same_source_interval = previous.source_end.segment_index()
                == folded.source_start.segment_index()
                && previous.source_end.parameter() == folded.source_start.parameter();
            let mut saw_hairpin =
                folded.orientation == Some(OffsetOrientation::Reversed) || same_source_interval;
            for candidate_index in (index + 2)..search_end {
                let candidate = run.segments[candidate_index];
                let prior = run.segments[candidate_index - 1];
                let prior_tangent = match prior.segment.limiting_unit_tangent_at(1.0) {
                    Ok(tangent) => tangent,
                    Err(error) if error.path() == "curve.path.tangent.stationary" => continue,
                    Err(error) => return Err(error),
                };
                let candidate_tangent = match candidate.segment.limiting_unit_tangent_at(0.0) {
                    Ok(tangent) => tangent,
                    Err(error) if error.path() == "curve.path.tangent.stationary" => continue,
                    Err(error) => return Err(error),
                };
                saw_hairpin |= prior_tangent.dot(candidate_tangent) <= -0.95
                    || prior.orientation == Some(OffsetOrientation::Reversed);
                if candidate.orientation != Some(OffsetOrientation::Retained) {
                    continue;
                }
                if !saw_hairpin || previous_tangent.dot(candidate_tangent) <= 0.0 {
                    continue;
                }
                let forward_projection = (candidate.segment.start().x - node.x)
                    * previous_tangent.x
                    + (candidate.segment.start().y - node.y) * previous_tangent.y;
                if forward_projection <= tolerance {
                    continue;
                }
                let Some(mut intersection) = line_intersection(
                    node,
                    Point2::new(node.x + previous_tangent.x, node.y + previous_tangent.y),
                    candidate.segment.start(),
                    Point2::new(
                        candidate.segment.start().x + candidate_tangent.x,
                        candidate.segment.start().y + candidate_tangent.y,
                    ),
                ) else {
                    continue;
                };
                if (intersection.x - node.x).hypot(intersection.y - node.y) <= tolerance {
                    intersection = node;
                } else if (intersection.x - candidate.segment.start().x)
                    .hypot(intersection.y - candidate.segment.start().y)
                    <= tolerance
                {
                    intersection = candidate.segment.start();
                }
                let candidate_travel = (intersection.x - candidate.segment.start().x)
                    * candidate_tangent.x
                    + (intersection.y - candidate.segment.start().y) * candidate_tangent.y;
                let previous_distance = (intersection.x - node.x).hypot(intersection.y - node.y);
                let candidate_distance = (intersection.x - candidate.segment.start().x)
                    .hypot(intersection.y - candidate.segment.start().y);
                if candidate_travel > tolerance
                    || previous_distance > maximum_extension
                    || candidate_distance > maximum_extension
                    || (intersection.x - previous.segment.start().x)
                        .hypot(intersection.y - previous.segment.start().y)
                        <= tolerance
                    || (candidate.segment.end().x - intersection.x)
                        .hypot(candidate.segment.end().y - intersection.y)
                        <= tolerance
                {
                    continue;
                }
                replacement = Some((candidate_index, intersection));
                break;
            }
            let Some((candidate_index, intersection)) = replacement else {
                index += 1;
                continue;
            };
            run.segments[index].segment =
                replace_segment_end_preserving_tangent(previous.segment, intersection)?;
            run.segments[candidate_index].segment = replace_segment_start_preserving_tangent(
                run.segments[candidate_index].segment,
                intersection,
            )?;
            run.segments.drain((index + 1)..candidate_index);
            index = index.saturating_sub(1);
        }
    }
    Ok(())
}

/// Removes the non-authoritative middle lobe between a candidate's first and last barrier crossings.
///
/// Already accepted offsets are immutable barriers. A candidate crossed on both terminal
/// extensions has exhausted its extended envelope and collapses; a one-run candidate that departs
/// and re-enters a barrier is split into its two outer pieces. Cleanup never mutates or joins a
/// previously published repetition.
///
/// # Errors
///
/// Returns bounded intersection, cleanup-limit, cancellation, or finite split diagnostics without
/// publishing a partially trimmed candidate.
fn dissolve_crossing_barrier_middle(
    runs: &mut Vec<OrderedOffsetRun>,
    authored_start: PathLocation,
    authored_end: PathLocation,
    barriers: &[&CurvePath],
    budget: &mut CleanupBudget,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), CurveError> {
    let Some((first, last)) = crossing_barrier_span(runs, barriers, budget, is_cancelled)? else {
        return Ok(());
    };
    trim_runs_around_crossing_span(runs, authored_start, authored_end, first, last)
}

/// Finds the first and last source-ordered transverse contacts with immutable barrier paths.
///
/// # Errors
///
/// Returns stable cleanup-limit, cancellation, subdivision, or numeric diagnostics before the
/// caller mutates any candidate run. Coincident barrier intervals are not transverse crossings,
/// so they are ignored here and remain subject to the candidate's own crossing cleanup.
fn crossing_barrier_span(
    runs: &[OrderedOffsetRun],
    barriers: &[&CurvePath],
    budget: &mut CleanupBudget,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Option<(RunBarrierCrossing, RunBarrierCrossing)>, CurveError> {
    let barrier_bounds = barriers
        .iter()
        .map(|barrier| {
            barrier
                .segments()
                .iter()
                .map(CurveSegment::bounds)
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut first = None;
    let mut last = None;
    for (run_index, run) in runs.iter().enumerate() {
        for (segment_index, candidate) in run.segments.iter().enumerate() {
            let candidate_bounds = candidate.segment.bounds()?;
            for (barrier, bounds) in barriers.iter().zip(&barrier_bounds) {
                for (barrier_segment, barrier_bounds) in barrier.segments().iter().zip(bounds) {
                    if is_cancelled() {
                        return Err(cancelled());
                    }
                    if !offset_bounds_overlap(
                        candidate_bounds,
                        *barrier_bounds,
                        DEFAULT_PATH_OFFSET_TOLERANCE,
                    )? {
                        continue;
                    }
                    budget.examined_pairs =
                        budget.examined_pairs.checked_add(1).ok_or(CurveError::new(
                            "curve.offset.cleanup_limit",
                            "path offset cleanup pair limit exceeded",
                        ))?;
                    if budget.examined_pairs > budget.maximum_pairs {
                        return Err(CurveError::new(
                            "curve.offset.cleanup_limit",
                            "path offset cleanup pair limit exceeded",
                        ));
                    }
                    let intersections = match candidate.segment.intersections(barrier_segment) {
                        Ok(intersections) => intersections,
                        Err(error) if error.path() == "curve.path.intersections.overlap" => {
                            continue;
                        }
                        Err(error) => return Err(error),
                    };
                    for intersection in intersections {
                        if intersection.kind() != crate::IntersectionKind::Crossing
                            || !strictly_interior(intersection.first_parameter())
                            || !strictly_interior(intersection.second_parameter())
                        {
                            continue;
                        }
                        let crossing = RunBarrierCrossing {
                            run_index,
                            segment_index,
                            parameter: intersection.first_parameter(),
                            point: intersection.point(),
                        };
                        if first.is_none_or(|current| {
                            run_barrier_crossing_order(crossing, current)
                                == std::cmp::Ordering::Less
                        }) {
                            first = Some(crossing);
                        }
                        if last.is_none_or(|current| {
                            run_barrier_crossing_order(crossing, current)
                                == std::cmp::Ordering::Greater
                        }) {
                            last = Some(crossing);
                        }
                    }
                }
            }
        }
    }
    Ok(first.zip(last))
}

/// Orders candidate/barrier crossings by exact run, segment, and finite local parameter.
fn run_barrier_crossing_order(
    left: RunBarrierCrossing,
    right: RunBarrierCrossing,
) -> std::cmp::Ordering {
    left.run_index
        .cmp(&right.run_index)
        .then_with(|| left.segment_index.cmp(&right.segment_index))
        .then_with(|| left.parameter.total_cmp(&right.parameter))
}

/// Retains the authored side of the first and last barrier crossings without reconnecting runs.
///
/// When crossings span both terminal extensions, the exhausted candidate collapses rather than
/// publishing either exterior extensions or floating authored fragments. Other zero-span
/// crossings discard only the crossed construction extensions; non-extension crossings retain the
/// source-outer pieces.
///
/// # Errors
///
/// Returns finite split diagnostics without reconnecting the omitted crossing span.
fn trim_runs_around_crossing_span(
    runs: &mut Vec<OrderedOffsetRun>,
    authored_start: PathLocation,
    authored_end: PathLocation,
    first: RunBarrierCrossing,
    last: RunBarrierCrossing,
) -> Result<(), CurveError> {
    let first_crossed = runs[first.run_index].segments[first.segment_index];
    let last_crossed = runs[last.run_index].segments[last.segment_index];
    let crosses_both_terminal_extensions = first_crossed.source_start == authored_start
        && first_crossed.source_end == authored_start
        && last_crossed.source_start == authored_end
        && last_crossed.source_end == authored_end;
    if crosses_both_terminal_extensions {
        runs.clear();
        return Ok(());
    }
    let crosses_only_endpoint_extensions = first_crossed.source_start == first_crossed.source_end
        && last_crossed.source_start == last_crossed.source_end;
    if crosses_only_endpoint_extensions {
        if first.run_index == last.run_index {
            let original = runs[first.run_index].clone();
            let middle = if first.segment_index < last.segment_index {
                original.segments[(first.segment_index + 1)..last.segment_index].to_vec()
            } else {
                Vec::new()
            };
            runs[first.run_index].segments = middle;
            runs.retain(|run| !run.segments.is_empty());
            return Ok(());
        }
        runs[first.run_index].segments =
            runs[first.run_index].segments[(first.segment_index + 1)..].to_vec();
        runs[last.run_index].segments.truncate(last.segment_index);
        runs.retain(|run| !run.segments.is_empty());
        return Ok(());
    }
    let (first_prefix, _) = split_traced_segment(first_crossed, first.parameter, first.point)?;
    let (_, last_suffix) = split_traced_segment(last_crossed, last.parameter, last.point)?;
    if first.run_index == last.run_index {
        let original = runs[first.run_index].clone();
        let mut prefix = original.segments[..first.segment_index].to_vec();
        prefix.push(first_prefix);
        let mut suffix = vec![last_suffix];
        suffix.extend_from_slice(&original.segments[(last.segment_index + 1)..]);
        runs.splice(
            first.run_index..=first.run_index,
            [
                OrderedOffsetRun {
                    segments: prefix,
                    first_source_segment: original.first_source_segment,
                    last_source_segment: original.last_source_segment,
                },
                OrderedOffsetRun {
                    segments: suffix,
                    first_source_segment: original.first_source_segment,
                    last_source_segment: original.last_source_segment,
                },
            ],
        );
        return Ok(());
    }
    runs[first.run_index].segments.truncate(first.segment_index);
    runs[first.run_index].segments.push(first_prefix);
    let mut suffix = vec![last_suffix];
    suffix.extend_from_slice(&runs[last.run_index].segments[(last.segment_index + 1)..]);
    runs[last.run_index].segments = suffix;
    if last.run_index > first.run_index + 1 {
        runs.drain((first.run_index + 1)..last.run_index);
    }
    Ok(())
}

/// Dissolves reversal loops that cross between separately retained cusp runs.
///
/// A crossing between both terminal endpoint extensions collapses the exhausted extended envelope
/// rather than publishing floating authored fragments. Other zero-source-span crossings discard
/// only the crossed construction extensions; non-extension crossings retain the ordered outer
/// branches. Constant-gap callers relink those retained sides at the exact solved node; generic
/// callers preserve separate runs across the discarded interval. Constant-gap cleanup also emits
/// the intervening source span as a closed component rooted at the same exact solved node.
///
/// # Errors
///
/// Returns bounded intersection, cleanup-limit, cancellation, or finite split diagnostics without
/// publishing partially trimmed runs.
#[allow(clippy::too_many_arguments)]
fn dissolve_cross_run_reversal_crossings(
    runs: &mut Vec<OrderedOffsetRun>,
    authored_start: PathLocation,
    authored_end: PathLocation,
    relink_retained_sides: bool,
    preserved_closed_runs: &mut Vec<Vec<TracedOffsetSegment>>,
    planar_switch_nodes: &mut Vec<Point2>,
    tolerance: f64,
    budget: &mut CleanupBudget,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), CurveError> {
    while let Some(crossing) = first_cross_run_transverse_crossing(runs, budget, is_cancelled)? {
        let (
            first_run,
            first_segment,
            second_run,
            second_segment,
            first_parameter,
            second_parameter,
            point,
        ) = crossing;
        let first_crossed = runs[first_run].segments[first_segment];
        let second_crossed = runs[second_run].segments[second_segment];
        let crosses_both_terminal_extensions = first_crossed.source_start == authored_start
            && first_crossed.source_end == authored_start
            && second_crossed.source_start == authored_end
            && second_crossed.source_end == authored_end;
        if crosses_both_terminal_extensions {
            runs.clear();
            return Ok(());
        }
        let crosses_only_endpoint_extensions = first_crossed.source_start
            == first_crossed.source_end
            && second_crossed.source_start == second_crossed.source_end;
        if crosses_only_endpoint_extensions {
            runs[first_run].segments = runs[first_run].segments[(first_segment + 1)..].to_vec();
            runs[second_run].segments.truncate(second_segment);
            runs.retain(|run| !run.segments.is_empty());
        } else {
            let (first_prefix, first_suffix) =
                split_traced_segment(first_crossed, first_parameter, point)?;
            let (second_prefix, second_suffix) =
                split_traced_segment(second_crossed, second_parameter, point)?;
            let mut enclosed = vec![first_suffix];
            append_exact_cross_run_piece(
                &mut enclosed,
                &runs[first_run].segments[(first_segment + 1)..],
                tolerance,
            )?;
            for middle_run in &runs[(first_run + 1)..second_run] {
                append_exact_cross_run_piece(&mut enclosed, &middle_run.segments, tolerance)?;
            }
            append_exact_cross_run_piece(
                &mut enclosed,
                &runs[second_run].segments[..second_segment],
                tolerance,
            )?;
            append_exact_cross_run_piece(&mut enclosed, &[second_prefix], tolerance)?;
            runs[first_run].segments.truncate(first_segment);
            runs[first_run].segments.push(first_prefix);
            let mut retained_suffix = vec![second_suffix];
            retained_suffix.extend_from_slice(&runs[second_run].segments[(second_segment + 1)..]);
            if relink_retained_sides {
                runs[first_run].segments.extend(retained_suffix);
                runs[first_run].last_source_segment = runs[second_run].last_source_segment;
                runs.drain((first_run + 1)..=second_run);
                preserved_closed_runs.push(enclosed);
                planar_switch_nodes.push(point);
            } else {
                runs[second_run].segments = retained_suffix;
                if second_run > first_run + 1 {
                    runs.drain((first_run + 1)..second_run);
                }
            }
        }
    }
    Ok(())
}

/// Appends one source-ordered cross-run piece at its existing solved cusp or crossing node.
///
/// Constant-gap cusp isolation retains every positive-length offset interval. Adjacent pieces may
/// carry sub-tolerance fitting noise at their shared derived node; this operation canonicalizes
/// only that node while moving the adjacent cubic handle with it. It never bridges a real gap.
///
/// # Errors
///
/// Returns a stable source-interval or finite-construction diagnostic without partially appending
/// a piece whose first node does not coincide within the declared fitting tolerance.
fn append_exact_cross_run_piece(
    destination: &mut Vec<TracedOffsetSegment>,
    piece: &[TracedOffsetSegment],
    tolerance: f64,
) -> Result<(), CurveError> {
    if piece.is_empty() {
        return Ok(());
    }
    let mut piece = piece.to_vec();
    if let Some(previous) = destination.last() {
        let end = previous.segment.end();
        let start = piece[0].segment.start();
        let gap = (end.x - start.x).hypot(end.y - start.y);
        if !gap.is_finite() || gap > tolerance {
            append_exact_cusp_round_join(destination, &piece[0])?;
        } else {
            piece[0].segment = replace_segment_start_preserving_tangent(piece[0].segment, end)?;
        }
    }
    destination.extend(piece);
    Ok(())
}

/// Connects separated offset branches through the exact circular locus of one vector node.
///
/// A true circle through both preserved endpoints is solved on their perpendicular bisector. The
/// endpoint-normal candidates select the center with the smallest combined tangent residual. At a
/// true 180-degree reversal the one-sided offsets form a diameter, so their midpoint is the exact
/// cusp center. Tangent agreement selects the sweep, and the existing bounded region-arc
/// constructor emits at most ninety-degree canonical cubic pieces.
///
/// # Errors
///
/// Returns stationary-tangent, unsolved-center, unequal-radius, finite-arc, or cubic-construction
/// diagnostics without adding a straight bridge or a partial arc.
fn append_exact_cusp_round_join(
    destination: &mut Vec<TracedOffsetSegment>,
    next: &TracedOffsetSegment,
) -> Result<(), CurveError> {
    let previous = destination.last().ok_or(CurveError::new(
        "curve.offset.source_interval",
        "constant-gap cusp join requires a preceding offset branch",
    ))?;
    let start = previous.segment.end();
    let end = next.segment.start();
    let previous_tangent = previous.segment.limiting_unit_tangent_at(1.0)?;
    let next_tangent = next.segment.limiting_unit_tangent_at(0.0)?;
    let tangent_dot = previous_tangent.x * next_tangent.x + previous_tangent.y * next_tangent.y;
    let previous_normal = previous_tangent.perpendicular();
    let next_normal = next_tangent.perpendicular();
    let center = if tangent_dot < -1.0 + 1.0e-6 {
        Point2::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5)
    } else {
        let midpoint = Point2::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5);
        let chord_bisector = Vector2::new(-(end.y - start.y), end.x - start.x);
        let bisector_end =
            Point2::new(midpoint.x + chord_bisector.x, midpoint.y + chord_bisector.y);
        [
            line_intersection(
                midpoint,
                bisector_end,
                start,
                Point2::new(start.x + previous_normal.x, start.y + previous_normal.y),
            ),
            line_intersection(
                midpoint,
                bisector_end,
                end,
                Point2::new(end.x + next_normal.x, end.y + next_normal.y),
            ),
        ]
        .into_iter()
        .flatten()
        .min_by(|left, right| {
            let residual = |center: Point2| {
                let start_radius = Vector2::new(start.x - center.x, start.y - center.y);
                let end_radius = Vector2::new(end.x - center.x, end.y - center.y);
                (start_radius.x * previous_tangent.x + start_radius.y * previous_tangent.y).abs()
                    + (end_radius.x * next_tangent.x + end_radius.y * next_tangent.y).abs()
            };
            residual(*left).total_cmp(&residual(*right))
        })
        .ok_or(CurveError::new(
            "curve.offset.source_interval",
            "separated constant-gap branches have no finite shared corner center",
        ))?
    };
    let start_radius = Vector2::new(start.x - center.x, start.y - center.y);
    let end_radius = Vector2::new(end.x - center.x, end.y - center.y);
    let start_length = start_radius.x.hypot(start_radius.y);
    let end_length = end_radius.x.hypot(end_radius.y);
    let radius_tolerance =
        DEFAULT_PATH_OFFSET_TOLERANCE.max(1.0e-9 * start_length.max(end_length).max(1.0));
    if !start_length.is_finite()
        || !end_length.is_finite()
        || start_length <= 0.0
        || (start_length - end_length).abs() > radius_tolerance
    {
        return Err(CurveError::new(
            "curve.offset.source_interval",
            "separated constant-gap branches do not share one circular corner radius",
        ));
    }
    let ccw_start = start_radius.perpendicular();
    let ccw_end = end_radius.perpendicular();
    let ccw_score = ccw_start.x * previous_tangent.x
        + ccw_start.y * previous_tangent.y
        + ccw_end.x * next_tangent.x
        + ccw_end.y * next_tangent.y;
    let joins = region_round_join_segments(center, start, end, ccw_score >= 0.0)?;
    destination.extend(joins.into_iter().map(|join| TracedOffsetSegment {
        segment: CurveSegment::CubicBezier(join),
        source_start: next.source_start,
        source_end: next.source_start,
        orientation: None,
    }));
    Ok(())
}

/// Finds the nearest gap-closing transverse crossing between distinct retained runs.
///
/// Adjacent source runs are considered before more distant runs. Within one run pair, candidate
/// segments expand diagonally from the first run's end and the second run's start, which selects
/// the crossing that closes the omitted source interval without scanning unrelated outer lobes.
/// Conservative segment bounds reject impossible contacts before they consume intersection work.
///
/// # Errors
///
/// Returns stable cleanup-limit, cancellation, overlap, subdivision, or numeric diagnostics before
/// any caller mutates the ordered run set.
fn first_cross_run_transverse_crossing(
    runs: &[OrderedOffsetRun],
    budget: &mut CleanupBudget,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Option<CrossRunTransverseCrossing>, CurveError> {
    for run_separation in 1..runs.len() {
        for first_run in 0..(runs.len() - run_separation) {
            let second_run = first_run + run_separation;
            let first_segments = &runs[first_run].segments;
            let second_segments = &runs[second_run].segments;
            let first_bounds = first_segments
                .iter()
                .map(|segment| segment.segment.bounds())
                .collect::<Result<Vec<_>, _>>()?;
            let second_bounds = second_segments
                .iter()
                .map(|segment| segment.segment.bounds())
                .collect::<Result<Vec<_>, _>>()?;
            let maximum_distance = first_segments
                .len()
                .checked_add(second_segments.len())
                .and_then(|count| count.checked_sub(2))
                .ok_or(CurveError::new(
                    "curve.offset.numeric_overflow",
                    "path offset crossing traversal overflowed",
                ))?;
            for distance in 0..=maximum_distance {
                let first_distance_start = distance.saturating_sub(second_segments.len() - 1);
                let first_distance_end = distance.min(first_segments.len() - 1);
                for first_distance in first_distance_start..=first_distance_end {
                    let second_segment = distance - first_distance;
                    let first_segment = first_segments.len() - 1 - first_distance;
                    let first = &first_segments[first_segment];
                    let second = &second_segments[second_segment];
                    if is_cancelled() {
                        return Err(cancelled());
                    }
                    if !offset_bounds_overlap(
                        first_bounds[first_segment],
                        second_bounds[second_segment],
                        DEFAULT_PATH_OFFSET_TOLERANCE,
                    )? {
                        continue;
                    }
                    budget.examined_pairs =
                        budget.examined_pairs.checked_add(1).ok_or(CurveError::new(
                            "curve.offset.cleanup_limit",
                            "path offset cleanup pair limit exceeded",
                        ))?;
                    if budget.examined_pairs > budget.maximum_pairs {
                        return Err(CurveError::new(
                            "curve.offset.cleanup_limit",
                            "path offset cleanup pair limit exceeded",
                        ));
                    }
                    for intersection in first.segment.intersections(&second.segment)? {
                        if matches!(
                            intersection.kind(),
                            crate::IntersectionKind::Crossing | crate::IntersectionKind::Tangent
                        ) && strictly_interior(intersection.first_parameter())
                            && strictly_interior(intersection.second_parameter())
                        {
                            return Ok(Some((
                                first_run,
                                first_segment,
                                second_run,
                                second_segment,
                                intersection.first_parameter(),
                                intersection.second_parameter(),
                                intersection.point(),
                            )));
                        }
                    }
                }
            }
        }
    }
    Ok(None)
}

/// Adds one retained source-adjacent segment run, joining only an exact source boundary.
///
/// # Errors
///
/// Returns finite join or request-wide segment-limit diagnostics without publishing a result.
#[allow(clippy::too_many_arguments)]
fn append_ordered_offset_run(
    runs: &mut Vec<OrderedOffsetRun>,
    mut offsets: Vec<TracedOffsetSegment>,
    source_segment: CurveSegment,
    distance: f64,
    join_policy: OffsetJoinPolicy,
    planar_switch_nodes: &[Point2],
    tolerance: f64,
    maximum_segments: usize,
    emitted_segments: &mut usize,
) -> Result<(), CurveError> {
    if offsets.is_empty() {
        return Ok(());
    }
    let joins_previous = runs.last().is_some_and(|previous| {
        let previous_end = previous
            .segments
            .last()
            .expect("ordered offset run is nonempty")
            .source_end;
        let next_start = offsets
            .first()
            .expect("new ordered offset run is nonempty")
            .source_start;
        let crosses_segment_boundary = previous_end.parameter() == 1.0
            && next_start.parameter() == 0.0
            && previous_end.segment_index().checked_add(1) == Some(next_start.segment_index());
        let suppresses_planar_switch_join = crosses_segment_boundary
            && planar_switch_nodes
                .iter()
                .any(|node| *node == source_segment.start());
        source_locations_are_adjacent(previous_end, next_start) && !suppresses_planar_switch_join
    });
    let added = if joins_previous {
        let previous = runs.last_mut().expect("adjacent run exists");
        let before = previous.segments.len();
        let source_boundary = offsets
            .first()
            .expect("new ordered offset run is nonempty")
            .source_start;
        connect_adjacent_offset_segments(
            &mut previous.segments,
            &mut offsets,
            previous.last_source_segment,
            source_segment,
            source_boundary,
            distance,
            join_policy,
            tolerance,
        )?;
        previous.segments.extend(offsets);
        previous.last_source_segment = source_segment;
        previous.segments.len() - before
    } else {
        let added = offsets.len();
        runs.push(OrderedOffsetRun {
            segments: offsets,
            first_source_segment: source_segment,
            last_source_segment: source_segment,
        });
        added
    };
    *emitted_segments = emitted_segments.checked_add(added).ok_or(CurveError::new(
        "curve.offset.segment_limit",
        "path offset segment limit exceeded",
    ))?;
    if *emitted_segments > maximum_segments {
        return Err(CurveError::new(
            "curve.offset.segment_limit",
            "path offset segment limit exceeded",
        ));
    }
    Ok(())
}

/// Joins two source-adjacent offset segment lists without creating a renderer-owned repair.
///
/// # Errors
///
/// Returns finite line/cubic join diagnostics without connecting a caller-classified source gap.
#[allow(clippy::too_many_arguments)]
fn connect_adjacent_offset_segments(
    derived: &mut Vec<TracedOffsetSegment>,
    next: &mut [TracedOffsetSegment],
    previous_source: CurveSegment,
    next_source: CurveSegment,
    source_boundary: PathLocation,
    distance: f64,
    join_policy: OffsetJoinPolicy,
    tolerance: f64,
) -> Result<(), CurveError> {
    let end = derived
        .last()
        .expect("adjacent prior offset run is nonempty")
        .segment
        .end();
    let offset_start = next
        .first()
        .expect("adjacent next offset run is nonempty")
        .segment
        .start();
    let gap = (end.x - offset_start.x).hypot(end.y - offset_start.y);
    if gap <= tolerance {
        next[0].segment = replace_segment_start(next[0].segment, end)?;
    } else if !join_offset_segments(
        derived,
        next,
        previous_source,
        next_source,
        source_boundary,
        distance,
        join_policy,
    )? {
        derived.push(TracedOffsetSegment {
            segment: CurveSegment::Line(LineSegment::new(end, offset_start)?),
            source_start: source_boundary,
            source_end: source_boundary,
            orientation: None,
        });
    }
    Ok(())
}

/// Tests exact monotonic adjacency without treating a small omitted cusp interval as continuous.
fn source_locations_are_adjacent(end: PathLocation, start: PathLocation) -> bool {
    (end.segment_index() == start.segment_index() && end.parameter() == start.parameter())
        || (end.parameter() == 1.0
            && start.parameter() == 0.0
            && end.segment_index().checked_add(1) == Some(start.segment_index()))
}

/// Closes one unbroken closed-source run with the existing deterministic straight fallback.
///
/// # Errors
///
/// Returns finite construction or segment-limit diagnostics without changing closed winding policy.
fn close_unbroken_offset_run(
    run: &mut OrderedOffsetRun,
    authored_end: PathLocation,
    distance: f64,
    join_policy: OffsetJoinPolicy,
    maximum_segments: usize,
    emitted_segments: &mut usize,
) -> Result<(), CurveError> {
    let end = run
        .segments
        .last()
        .expect("unbroken closed offset run is nonempty")
        .segment
        .end();
    let start = run
        .segments
        .first()
        .expect("unbroken closed offset run is nonempty")
        .segment
        .start();
    if (end.x - start.x).hypot(end.y - start.y) <= 1.0e-6 {
        let last = run.segments.len() - 1;
        run.segments[last].segment =
            replace_segment_end_preserving_tangent(run.segments[last].segment, start)?;
        return Ok(());
    }
    if join_policy == OffsetJoinPolicy::RegionRound {
        let last = run.segments.len() - 1;
        let previous_direction = offset_limiting_unit_tangent(run.last_source_segment, 1.0)?;
        let next_direction = offset_limiting_unit_tangent(run.first_source_segment, 0.0)?;
        let turn =
            previous_direction.x * next_direction.y - previous_direction.y * next_direction.x;
        if turn > 1.0e-12 && distance < 0.0 {
            let joins = region_round_join_segments(
                run.first_source_segment.start(),
                run.segments[last].segment.end(),
                run.segments[0].segment.start(),
                turn > 0.0,
            )?;
            let added = joins.len();
            run.segments
                .extend(joins.into_iter().map(|join| TracedOffsetSegment {
                    segment: CurveSegment::CubicBezier(join),
                    source_start: authored_end,
                    source_end: authored_end,
                    orientation: None,
                }));
            *emitted_segments = emitted_segments.checked_add(added).ok_or(CurveError::new(
                "curve.offset.segment_limit",
                "path offset segment limit exceeded",
            ))?;
            if *emitted_segments > maximum_segments {
                return Err(CurveError::new(
                    "curve.offset.segment_limit",
                    "path offset segment limit exceeded",
                ));
            }
            return Ok(());
        }
        if let Some(intersection) = offset_tangent_intersection(
            run.segments[last].segment,
            run.segments[0].segment,
            run.last_source_segment,
            run.first_source_segment,
        )? {
            run.segments[last].segment =
                replace_segment_end_preserving_tangent(run.segments[last].segment, intersection)?;
            run.segments[0].segment =
                replace_segment_start_preserving_tangent(run.segments[0].segment, intersection)?;
            return Ok(());
        }
    }
    if end != start {
        run.segments.push(TracedOffsetSegment {
            segment: CurveSegment::Line(LineSegment::new(end, start)?),
            source_start: authored_end,
            source_end: authored_end,
            orientation: None,
        });
        *emitted_segments = emitted_segments.checked_add(1).ok_or(CurveError::new(
            "curve.offset.segment_limit",
            "path offset segment limit exceeded",
        ))?;
        if *emitted_segments > maximum_segments {
            return Err(CurveError::new(
                "curve.offset.segment_limit",
                "path offset segment limit exceeded",
            ));
        }
    }
    Ok(())
}

/// Merges the first and last broken closed-source runs only when both touch the authored seam.
///
/// Every broken result remains open; the merge merely preserves adjacency across the authored seam.
///
/// # Errors
///
/// Returns finite join or request-wide segment-limit diagnostics without bridging any interior gap.
fn merge_closed_seam_runs(
    runs: &mut Vec<OrderedOffsetRun>,
    authored_end: PathLocation,
    distance: f64,
    join_policy: OffsetJoinPolicy,
    tolerance: f64,
    maximum_segments: usize,
    emitted_segments: &mut usize,
) -> Result<(), CurveError> {
    if runs.len() < 2 {
        return Ok(());
    }
    let authored_start = PathLocation::new(0, 0.0)?;
    let touches_seam = runs
        .last()
        .and_then(|run| run.segments.last())
        .map(|segment| segment.source_end == authored_end)
        .unwrap_or(false)
        && runs
            .first()
            .and_then(|run| run.segments.first())
            .map(|segment| segment.source_start == authored_start)
            .unwrap_or(false);
    if !touches_seam {
        return Ok(());
    }
    let mut last = runs.pop().expect("closed seam has a last run");
    let mut first = runs.remove(0);
    let before = last.segments.len();
    connect_adjacent_offset_segments(
        &mut last.segments,
        &mut first.segments,
        last.last_source_segment,
        first.first_source_segment,
        authored_end,
        distance,
        join_policy,
        tolerance,
    )?;
    let first_segment_count = first.segments.len();
    last.segments.extend(first.segments);
    last.last_source_segment = first.last_source_segment;
    let added = last.segments.len() - before - first_segment_count;
    *emitted_segments = emitted_segments.checked_add(added).ok_or(CurveError::new(
        "curve.offset.segment_limit",
        "path offset segment limit exceeded",
    ))?;
    if *emitted_segments > maximum_segments {
        return Err(CurveError::new(
            "curve.offset.segment_limit",
            "path offset segment limit exceeded",
        ));
    }
    runs.insert(0, last);
    Ok(())
}

/// Splits transverse self-crossings and finite coincident line folds into source-ordered components.
///
/// Coincident cleanup prevents tiled open-guide seams and tight Constant-gap reversals from
/// surviving as overlapping hooks. It never approximates coincident cubic intervals.
///
/// # Errors
///
/// Returns canonical intersection, component-limit, cancellation, or finite path diagnostics without
/// publishing a partial cleanup.
#[allow(clippy::too_many_arguments)]
fn dissolve_crossings_with_budget(
    segments: Vec<TracedOffsetSegment>,
    closure: PathClosure,
    retained_winding: Option<WindingDirection>,
    budget: &mut CleanupBudget,
    maximum_components: usize,
    tolerance: f64,
    dissolve_coincident_overlaps: bool,
    relink_open_crossings: bool,
    planar_switch_nodes: &[Point2],
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CleanedOffsetPath>, CurveError> {
    let mut pending = vec![(segments, closure, planar_switch_nodes.to_vec())];
    let mut cleaned = Vec::new();
    while let Some((mut segments, closure, mut switches)) = pending.pop() {
        if is_cancelled() {
            return Err(cancelled());
        }
        if relink_open_crossings {
            canonicalize_planar_endpoint_handles(&mut segments, tolerance)?;
            canonicalize_planar_adjacent_nodes(&mut segments, tolerance)?;
        }
        let plain = segments
            .iter()
            .map(|segment| segment.segment)
            .collect::<Vec<_>>();
        let crossing = if relink_open_crossings && closure == PathClosure::Open {
            let crossing = first_transverse_crossing_with_budget(
                &plain,
                budget,
                dissolve_coincident_overlaps,
                is_cancelled,
            )?;
            if let Some(crossing) = crossing {
                switches.push(crossing.4);
                let split = relink_open_path_at_crossing_preserving_spans(segments, crossing)?;
                if cleaned.len() + pending.len() + split.len() > maximum_components {
                    return Err(CurveError::new(
                        "curve.offset.component_limit",
                        "path offset component limit exceeded",
                    ));
                }
                for (component, component_closure) in split.into_iter().rev() {
                    pending.push((component, component_closure, switches.clone()));
                }
                continue;
            }
            None
        } else {
            first_transverse_crossing_with_budget(
                &plain,
                budget,
                dissolve_coincident_overlaps,
                is_cancelled,
            )?
        };
        let Some(crossing) = crossing else {
            if segments.is_empty() {
                continue;
            }
            let path = CurvePath::new(plain, closure).map_err(|error| {
                CurveError::new("curve.offset.cleanup_continuity", error.message())
            })?;
            if path.measure_arc_length()?.total_length() <= tolerance {
                continue;
            }
            if closure == PathClosure::Closed
                && retained_winding.is_some()
                && path_winding(&path, tolerance)? != retained_winding
            {
                continue;
            }
            let earliest_source = earliest_source_location(&segments);
            let latest_source = latest_source_location(&segments);
            if earliest_source == latest_source && !relink_open_crossings {
                continue;
            }
            switches.retain(|node| {
                path.segments()
                    .iter()
                    .any(|segment| segment.start() == *node || segment.end() == *node)
            });
            switches.dedup();
            cleaned.push(CleanedOffsetPath {
                path,
                source_start: earliest_source,
                source_end: latest_source,
                earliest_source,
                planar_switch_nodes: switches,
            });
            if cleaned.len() + pending.len() > maximum_components {
                return Err(CurveError::new(
                    "curve.offset.component_limit",
                    "path offset component limit exceeded",
                ));
            }
            continue;
        };
        switches.push(crossing.4);
        let split = split_at_crossing(segments, closure, crossing)?;
        if cleaned.len() + pending.len() + split.len() > maximum_components {
            return Err(CurveError::new(
                "curve.offset.component_limit",
                "path offset component limit exceeded",
            ));
        }
        for (component, component_closure) in split.into_iter().rev() {
            pending.push((component, component_closure, switches.clone()));
        }
    }
    cleaned.sort_by(|left, right| {
        source_location_key(left.earliest_source).cmp(&source_location_key(right.earliest_source))
    });
    Ok(cleaned)
}

/// Snaps numerically stationary derived endpoint handles onto their exact vector nodes.
///
/// Iterated Constant-gap fronts can end a fitted cubic at a solved cusp or crossing node. Floating
/// subdivision may leave a sub-tolerance handle beside that node, which represents the same
/// topology but creates an artificial near-stationary reversal on the next rank. A cubic whose two
/// controls then equal its endpoints is exactly a straight locus and is stored as a line so overlap
/// cleanup can treat it canonically. This changes no endpoint or source interval and leaves every
/// resolvable handle untouched.
///
/// # Errors
///
/// Returns finite control-polygon or cubic-construction diagnostics without partially publishing a
/// path.
fn canonicalize_planar_endpoint_handles(
    segments: &mut [TracedOffsetSegment],
    fitting_tolerance: f64,
) -> Result<(), CurveError> {
    for traced in segments {
        let CurveSegment::CubicBezier(cubic) = traced.segment else {
            continue;
        };
        let canonical = canonicalize_cubic_endpoint_handles(cubic, fitting_tolerance)?;
        traced.segment = if canonical.control_1() == canonical.start()
            && canonical.control_2() == canonical.end()
        {
            CurveSegment::Line(LineSegment::new(canonical.start(), canonical.end())?)
        } else {
            CurveSegment::CubicBezier(canonical)
        };
    }
    Ok(())
}

/// Canonicalizes sub-tolerance adjacent endpoints onto one exact solved vector node.
///
/// Independently fitted or intersected branches can represent the same node with different final
/// floating-point bits. The preceding segment owns the canonical coordinate; moving the next
/// segment's start and its adjacent cubic handle together preserves its limiting tangent. Gaps
/// beyond the declared fitting tolerance remain errors at the canonical `CurvePath` boundary.
///
/// # Errors
///
/// Returns finite line or cubic reconstruction diagnostics without bridging a geometric gap.
fn canonicalize_planar_adjacent_nodes(
    segments: &mut [TracedOffsetSegment],
    tolerance: f64,
) -> Result<(), CurveError> {
    for index in 1..segments.len() {
        let end = segments[index - 1].segment.end();
        let start = segments[index].segment.start();
        if end == start {
            continue;
        }
        let gap = (end.x - start.x).hypot(end.y - start.y);
        if gap.is_finite() && gap <= tolerance {
            segments[index].segment =
                replace_segment_start_preserving_tangent(segments[index].segment, end)?;
        }
    }
    Ok(())
}

/// Canonicalizes sub-tolerance cubic terminal handles at their existing vector nodes.
///
/// This is the shared numeric form used both after planar relinking and immediately after a
/// Constant-gap source interval is split. It never moves a node or a resolvable handle.
///
/// # Errors
///
/// Returns finite control-polygon or cubic-construction diagnostics without publishing a partial
/// segment.
fn canonicalize_cubic_endpoint_handles(
    cubic: crate::CubicBezierSegment,
    fitting_tolerance: f64,
) -> Result<crate::CubicBezierSegment, CurveError> {
    let local_scale = CurveSegment::CubicBezier(cubic).control_polygon_length()?;
    let tolerance = fitting_tolerance.max(
        crate::curves::ABSOLUTE_TOLERANCE
            + crate::curves::RELATIVE_TOLERANCE * local_scale.max(1.0),
    );
    let control_1 = if (cubic.control_1().x - cubic.start().x)
        .hypot(cubic.control_1().y - cubic.start().y)
        <= tolerance
    {
        cubic.start()
    } else {
        cubic.control_1()
    };
    let control_2 = if (cubic.end().x - cubic.control_2().x)
        .hypot(cubic.end().y - cubic.control_2().y)
        <= tolerance
    {
        cubic.end()
    } else {
        cubic.control_2()
    };
    crate::CubicBezierSegment::new(cubic.start(), control_1, control_2, cubic.end())
}

/// Supplies a fresh cleanup budget for focused cleanup-only tests.
///
/// # Errors
///
/// Returns the same bounded cleanup diagnostics as the request-wide implementation.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn dissolve_crossings(
    segments: Vec<TracedOffsetSegment>,
    closure: PathClosure,
    retained_winding: Option<WindingDirection>,
    maximum_pairs: usize,
    maximum_components: usize,
    tolerance: f64,
    relink_open_crossings: bool,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CleanedOffsetPath>, CurveError> {
    dissolve_crossings_with_budget(
        segments,
        closure,
        retained_winding,
        &mut CleanupBudget {
            examined_pairs: 0,
            maximum_pairs,
        },
        maximum_components,
        tolerance,
        false,
        relink_open_crossings,
        &[],
        is_cancelled,
    )
}

/// Splits one earliest crossing into forward-traversal open fragments or closed loops.
///
/// # Errors
///
/// Returns finite subdivision/path diagnostics without reversing or reconnecting a retained edge.
fn split_at_crossing(
    segments: Vec<TracedOffsetSegment>,
    closure: PathClosure,
    crossing: (usize, usize, f64, f64, Point2),
) -> Result<Vec<(Vec<TracedOffsetSegment>, PathClosure)>, CurveError> {
    let (first, second, first_parameter, second_parameter, point) = crossing;
    let (first_prefix, first_suffix) =
        split_traced_segment(segments[first], first_parameter, point)?;
    let (second_prefix, second_suffix) =
        split_traced_segment(segments[second], second_parameter, point)?;
    if closure == PathClosure::Open {
        let mut prefix = segments[..first].to_vec();
        prefix.push(first_prefix);
        let mut suffix = vec![second_suffix];
        suffix.extend_from_slice(&segments[(second + 1)..]);
        return Ok([prefix, suffix]
            .into_iter()
            .filter(|component| !component.is_empty())
            .map(|component| (component, PathClosure::Open))
            .collect());
    }

    let mut enclosed = vec![first_suffix];
    enclosed.extend_from_slice(&segments[(first + 1)..second]);
    enclosed.push(second_prefix);
    let mut wrapping = vec![second_suffix];
    wrapping.extend_from_slice(&segments[(second + 1)..]);
    wrapping.extend_from_slice(&segments[..first]);
    wrapping.push(first_prefix);
    Ok([enclosed, wrapping]
        .into_iter()
        .filter(|component| !component.is_empty())
        .map(|component| (component, PathClosure::Closed))
        .collect())
}

/// Relinks one open frontier at an exact crossing without deleting the enclosed source span.
///
/// The outer open component joins its source-ordered prefix to its suffix at the solved vector
/// node. The intervening source interval becomes a closed component rooted at that same node, so
/// every input segment interval survives exactly once. Further crossings in either component are
/// handled by the caller's bounded pending-component traversal.
///
/// # Errors
///
/// Returns finite subdivision or source-location diagnostics without publishing a partial path.
fn relink_open_path_at_crossing_preserving_spans(
    segments: Vec<TracedOffsetSegment>,
    crossing: TransverseCrossing,
) -> Result<Vec<(Vec<TracedOffsetSegment>, PathClosure)>, CurveError> {
    let (first, second, first_parameter, second_parameter, point) = crossing;
    let first_segment = segments.first().ok_or(CurveError::new(
        "curve.offset.source_interval",
        "crossing relink requires at least one source segment",
    ))?;
    let last_segment = segments.len() - 1;
    let mut outer = Vec::new();
    append_traced_range(
        &mut outer,
        &segments,
        0,
        0.0,
        first_segment.segment.start(),
        first,
        first_parameter,
        point,
    )?;
    append_traced_range(
        &mut outer,
        &segments,
        second,
        second_parameter,
        point,
        last_segment,
        1.0,
        segments[last_segment].segment.end(),
    )?;
    let mut enclosed = Vec::new();
    append_traced_range(
        &mut enclosed,
        &segments,
        first,
        first_parameter,
        point,
        second,
        second_parameter,
        point,
    )?;
    Ok(
        [(outer, PathClosure::Open), (enclosed, PathClosure::Closed)]
            .into_iter()
            .filter(|(component, _)| !component.is_empty())
            .collect(),
    )
}

/// Appends one inclusive source-traversal interval with exact supplied boundary nodes.
///
/// # Errors
///
/// Returns ordered-interval, subdivision, or finite construction diagnostics without partially
/// changing the destination.
#[allow(clippy::too_many_arguments)]
fn append_traced_range(
    destination: &mut Vec<TracedOffsetSegment>,
    source: &[TracedOffsetSegment],
    start_segment: usize,
    start_parameter: f64,
    start_point: Point2,
    end_segment: usize,
    end_parameter: f64,
    end_point: Point2,
) -> Result<(), CurveError> {
    if start_segment > end_segment
        || (start_segment == end_segment && start_parameter >= end_parameter)
    {
        return Ok(());
    }
    let mut appended = Vec::new();
    for (segment_index, source_segment) in source
        .iter()
        .enumerate()
        .take(end_segment + 1)
        .skip(start_segment)
    {
        let local_start = if segment_index == start_segment {
            start_parameter
        } else {
            0.0
        };
        let local_end = if segment_index == end_segment {
            end_parameter
        } else {
            1.0
        };
        if local_start >= local_end {
            continue;
        }
        let mut retained = *source_segment;
        if strictly_interior(local_start) {
            retained = split_traced_segment(retained, local_start, start_point)?.1;
        }
        if strictly_interior(local_end) {
            let remaining_parameter = (local_end - local_start) / (1.0 - local_start);
            retained = split_traced_segment(retained, remaining_parameter, end_point)?.0;
        }
        if let Some(previous) = appended.last().or_else(|| destination.last()) {
            retained.segment = replace_segment_start(retained.segment, previous.segment.end())?;
        }
        appended.push(retained);
    }
    destination.extend(appended);
    Ok(())
}

/// Finds the earliest interior transverse contact between nonadjacent compact offset segments.
///
/// # Errors
///
/// Propagates bounded canonical segment-intersection failures without choosing an approximate crossing.
#[cfg(test)]
fn first_transverse_crossing(
    segments: &[CurveSegment],
    maximum_pairs: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Option<TransverseCrossing>, CurveError> {
    first_transverse_crossing_with_budget(
        segments,
        &mut CleanupBudget {
            examined_pairs: 0,
            maximum_pairs,
        },
        false,
        is_cancelled,
    )
}

/// Finds one crossing while charging every examined pair to the request-wide cleanup budget.
///
/// # Errors
///
/// Returns cancellation, intersection, or cleanup-limit diagnostics before a partial component escapes.
fn first_transverse_crossing_with_budget(
    segments: &[CurveSegment],
    budget: &mut CleanupBudget,
    dissolve_coincident_overlaps: bool,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Option<TransverseCrossing>, CurveError> {
    let bounds = segments
        .iter()
        .map(CurveSegment::bounds)
        .collect::<Result<Vec<_>, _>>()?;
    for (first_index, first) in segments.iter().enumerate() {
        for (second_index, second) in segments.iter().enumerate().skip(first_index + 2) {
            if is_cancelled() {
                return Err(cancelled());
            }
            if !offset_bounds_overlap(
                bounds[first_index],
                bounds[second_index],
                DEFAULT_PATH_OFFSET_TOLERANCE,
            )? {
                continue;
            }
            budget.examined_pairs = budget.examined_pairs.checked_add(1).ok_or(CurveError::new(
                "curve.offset.cleanup_limit",
                "path offset cleanup pair limit exceeded",
            ))?;
            if budget.examined_pairs > budget.maximum_pairs {
                return Err(CurveError::new(
                    "curve.offset.cleanup_limit",
                    "path offset cleanup pair limit exceeded",
                ));
            }
            let intersections = match first.intersections(second) {
                Ok(intersections) => intersections,
                Err(error)
                    if dissolve_coincident_overlaps
                        && error.path() == "curve.path.intersections.overlap" =>
                {
                    let shares_exact_node = [first.start(), first.end()].into_iter().any(|left| {
                        [second.start(), second.end()]
                            .into_iter()
                            .any(|right| left == right)
                    });
                    if shares_exact_node {
                        continue;
                    }
                    if !matches!(
                        (first, second),
                        (CurveSegment::Line(_), CurveSegment::Line(_))
                    ) {
                        return Err(error);
                    }
                    if let Some((first_parameter, second_parameter, point)) =
                        coincident_line_overlap_split(*first, *second)?
                    {
                        return Ok(Some((
                            first_index,
                            second_index,
                            first_parameter,
                            second_parameter,
                            point,
                        )));
                    }
                    return Err(error);
                }
                Err(error) => return Err(error),
            };
            for intersection in intersections {
                if intersection.kind() == crate::IntersectionKind::Crossing
                    && solved_contact_coincides_with_shared_endpoint(
                        *first,
                        *second,
                        intersection.point(),
                        DEFAULT_PATH_OFFSET_TOLERANCE,
                    )?
                {
                    continue;
                }
                if intersection.kind() == crate::IntersectionKind::Crossing
                    && strictly_interior(intersection.first_parameter())
                    && strictly_interior(intersection.second_parameter())
                {
                    return Ok(Some((
                        first_index,
                        second_index,
                        intersection.first_parameter(),
                        intersection.second_parameter(),
                        intersection.point(),
                    )));
                }
            }
        }
    }
    Ok(None)
}

/// Selects the midpoint of one positive-length coincident line overlap as a deterministic split.
///
/// This RegionRound-only recovery turns a collapsed inward neck into two source-ordered cleanup
/// candidates instead of exposing the generic discrete-intersection rejection.
///
/// # Errors
///
/// Returns finite-coordinate diagnostics when a candidate line cannot support stable projection.
fn coincident_line_overlap_split(
    first: CurveSegment,
    second: CurveSegment,
) -> Result<Option<(f64, f64, Point2)>, CurveError> {
    let (CurveSegment::Line(first), CurveSegment::Line(second)) = (first, second) else {
        return Ok(None);
    };
    let delta = Vector2::new(
        first.end().x - first.start().x,
        first.end().y - first.start().y,
    );
    let denominator = delta.x * delta.x + delta.y * delta.y;
    if !denominator.is_finite() || denominator == 0.0 {
        return Ok(None);
    }
    let second_start = ((second.start().x - first.start().x) * delta.x
        + (second.start().y - first.start().y) * delta.y)
        / denominator;
    let second_end = ((second.end().x - first.start().x) * delta.x
        + (second.end().y - first.start().y) * delta.y)
        / denominator;
    let lower = second_start.min(second_end).max(0.0);
    let upper = second_start.max(second_end).min(1.0);
    let first_parameter = (lower + upper) * 0.5;
    if !strictly_interior(first_parameter) {
        return Ok(None);
    }
    let point = Point2::new(
        first.start().x + delta.x * first_parameter,
        first.start().y + delta.y * first_parameter,
    );
    let second_delta = Vector2::new(
        second.end().x - second.start().x,
        second.end().y - second.start().y,
    );
    let second_denominator = second_delta.x * second_delta.x + second_delta.y * second_delta.y;
    if !second_denominator.is_finite() || second_denominator == 0.0 {
        return Ok(None);
    }
    let second_parameter = ((point.x - second.start().x) * second_delta.x
        + (point.y - second.start().y) * second_delta.y)
        / second_denominator;
    if !strictly_interior(second_parameter) || !point.is_finite() {
        return Ok(None);
    }
    Ok(Some((first_parameter, second_parameter, point)))
}

/// Splits one traced segment at an exact crossing point and interpolates its source parameter.
///
/// # Errors
///
/// Returns finite line/cubic construction diagnostics without widening the recorded source interval.
fn split_traced_segment(
    segment: TracedOffsetSegment,
    parameter: f64,
    point: Point2,
) -> Result<(TracedOffsetSegment, TracedOffsetSegment), CurveError> {
    let source_middle = interpolate_source_location(segment, parameter)?;
    let (prefix, suffix) = match segment.segment {
        CurveSegment::Line(line) => (
            CurveSegment::Line(LineSegment::new(line.start(), point)?),
            CurveSegment::Line(LineSegment::new(point, line.end())?),
        ),
        CurveSegment::CubicBezier(cubic) => {
            let (prefix, suffix) = cubic.split(parameter)?;
            (
                CurveSegment::CubicBezier(replace_cubic_end(prefix, point)?),
                CurveSegment::CubicBezier(replace_cubic_start(suffix, point)?),
            )
        }
    };
    Ok((
        TracedOffsetSegment {
            segment: prefix,
            source_start: segment.source_start,
            source_end: source_middle,
            orientation: segment.orientation,
        },
        TracedOffsetSegment {
            segment: suffix,
            source_start: source_middle,
            source_end: segment.source_end,
            orientation: segment.orientation,
        },
    ))
}

/// Interpolates within one source span; zero-width join spans retain their boundary location.
///
/// # Errors
///
/// Returns a stable provenance diagnostic when a nonzero derived piece crosses source segments.
fn interpolate_source_location(
    segment: TracedOffsetSegment,
    parameter: f64,
) -> Result<PathLocation, CurveError> {
    if segment.source_start.segment_index() != segment.source_end.segment_index() {
        return Err(CurveError::new(
            "curve.offset.source_interval",
            "derived segment crosses incompatible source intervals",
        ));
    }
    let source_parameter = segment.source_start.parameter()
        + (segment.source_end.parameter() - segment.source_start.parameter()) * parameter;
    PathLocation::new(segment.source_start.segment_index(), source_parameter)
}

/// Returns the earliest source location present in one traversal component, including wraparound loops.
fn earliest_source_location(segments: &[TracedOffsetSegment]) -> PathLocation {
    segments
        .iter()
        .flat_map(|segment| [segment.source_start, segment.source_end])
        .min_by_key(|location| source_location_key(*location))
        .expect("cleaned components are nonempty")
}

/// Returns the latest exact source location retained by one forward-traversal component.
fn latest_source_location(segments: &[TracedOffsetSegment]) -> PathLocation {
    segments
        .iter()
        .flat_map(|segment| [segment.source_start, segment.source_end])
        .max_by_key(|location| source_location_key(*location))
        .expect("cleaned component is nonempty")
}

/// Converts one finite source location into a deterministic total-order key.
fn source_location_key(location: PathLocation) -> (usize, u64) {
    (location.segment_index(), location.parameter().to_bits())
}

/// Replaces a split cubic endpoint with the canonical shared crossing coordinate.
///
/// # Errors
///
/// Returns finite cubic construction diagnostics without changing the retained controls.
fn replace_cubic_end(
    cubic: crate::CubicBezierSegment,
    end: Point2,
) -> Result<crate::CubicBezierSegment, CurveError> {
    crate::CubicBezierSegment::new(cubic.start(), cubic.control_1(), cubic.control_2(), end)
}

/// Replaces a split cubic start with the canonical shared crossing coordinate.
///
/// # Errors
///
/// Returns finite cubic construction diagnostics without changing the retained controls.
fn replace_cubic_start(
    cubic: crate::CubicBezierSegment,
    start: Point2,
) -> Result<crate::CubicBezierSegment, CurveError> {
    crate::CubicBezierSegment::new(start, cubic.control_1(), cubic.control_2(), cubic.end())
}

/// Returns whether one normalized intersection parameter is away from either segment endpoint.
fn strictly_interior(parameter: f64) -> bool {
    (1.0e-9..(1.0 - 1.0e-9)).contains(&parameter)
}

/// Resolves one adjacent analytic line-offset corner as an inner tangent intersection or compact outer round join.
///
/// # Errors
///
/// Returns finite construction diagnostics while leaving the caller's vectors unchanged when no line join applies.
fn join_offset_segments(
    derived: &mut Vec<TracedOffsetSegment>,
    next: &mut [TracedOffsetSegment],
    previous_source: CurveSegment,
    next_source: CurveSegment,
    next_source_start: PathLocation,
    distance: f64,
    join_policy: OffsetJoinPolicy,
) -> Result<bool, CurveError> {
    let Some(previous_derived) = derived.last().copied() else {
        return Ok(false);
    };
    let Some(next_derived) = next.first().copied() else {
        return Ok(false);
    };
    let previous_direction = offset_limiting_unit_tangent(previous_source, 1.0)?;
    let next_direction = offset_limiting_unit_tangent(next_source, 0.0)?;
    let turn = previous_direction.x * next_direction.y - previous_direction.y * next_direction.x;
    if turn.abs() <= 1.0e-12 {
        return Ok(false);
    }
    let uses_outer_join = match join_policy {
        OffsetJoinPolicy::CompactRound => turn * distance < 0.0,
        OffsetJoinPolicy::PlanarVector => false,
        OffsetJoinPolicy::RegionRound => turn > 0.0 && distance < 0.0,
    };
    if uses_outer_join {
        let joins = match join_policy {
            OffsetJoinPolicy::CompactRound => match compact_round_join(
                previous_source.end(),
                previous_derived.segment.end(),
                next_derived.segment.start(),
                turn > 0.0,
            ) {
                Ok(join) => vec![join],
                Err(error) if error.path() == "curve.offset.join" => return Ok(false),
                Err(error) => return Err(error),
            },
            OffsetJoinPolicy::PlanarVector => {
                unreachable!("planar vector joins never select an outer round")
            }
            OffsetJoinPolicy::RegionRound => region_round_join_segments(
                previous_source.end(),
                previous_derived.segment.end(),
                next_derived.segment.start(),
                turn > 0.0,
            )?,
        };
        derived.extend(joins.into_iter().map(|join| TracedOffsetSegment {
            segment: CurveSegment::CubicBezier(join),
            source_start: next_source_start,
            source_end: next_source_start,
            orientation: None,
        }));
        return Ok(true);
    }
    let Some(intersection) = offset_tangent_intersection(
        previous_derived.segment,
        next_derived.segment,
        previous_source,
        next_source,
    )?
    else {
        return Ok(false);
    };
    let maximum_miter = distance.abs() * 8.0 + DEFAULT_PATH_OFFSET_TOLERANCE;
    let previous_miter = (intersection.x - previous_derived.segment.end().x)
        .hypot(intersection.y - previous_derived.segment.end().y);
    let next_miter = (intersection.x - next_derived.segment.start().x)
        .hypot(intersection.y - next_derived.segment.start().y);
    if previous_miter > maximum_miter || next_miter > maximum_miter {
        let center = previous_source.end();
        let start_vector = Vector2::new(
            previous_derived.segment.end().x - center.x,
            previous_derived.segment.end().y - center.y,
        );
        let end_vector = Vector2::new(
            next_derived.segment.start().x - center.x,
            next_derived.segment.start().y - center.y,
        );
        let counter_clockwise =
            start_vector.x * end_vector.y - start_vector.y * end_vector.x >= 0.0;
        let joins = match region_round_join_segments(
            center,
            previous_derived.segment.end(),
            next_derived.segment.start(),
            counter_clockwise,
        ) {
            Ok(joins) => joins,
            Err(error) if error.path() == "curve.offset.join" => return Ok(false),
            Err(error) => return Err(error),
        };
        derived.extend(joins.into_iter().map(|join| TracedOffsetSegment {
            segment: CurveSegment::CubicBezier(join),
            source_start: next_source_start,
            source_end: next_source_start,
            orientation: None,
        }));
        return Ok(true);
    }
    let last = derived.len() - 1;
    derived[last].segment =
        replace_segment_end_preserving_tangent(derived[last].segment, intersection)?;
    next[0].segment = replace_segment_start_preserving_tangent(next[0].segment, intersection)?;
    Ok(true)
}

/// Finds the finite intersection of adjacent derived tangent lines without fabricating a corner.
///
/// # Errors
///
/// Propagates source tangent failures; parallel or nonfinite intersections deliberately return
/// `None` so the caller retains its existing deterministic fallback connector.
fn offset_tangent_intersection(
    previous_derived: CurveSegment,
    next_derived: CurveSegment,
    previous_source: CurveSegment,
    next_source: CurveSegment,
) -> Result<Option<Point2>, CurveError> {
    let previous_direction = offset_limiting_unit_tangent(previous_source, 1.0)?;
    let next_direction = offset_limiting_unit_tangent(next_source, 0.0)?;
    Ok(line_intersection(
        previous_derived.end(),
        Point2::new(
            previous_derived.end().x + previous_direction.x,
            previous_derived.end().y + previous_direction.y,
        ),
        next_derived.start(),
        Point2::new(
            next_derived.start().x + next_direction.x,
            next_derived.start().y + next_direction.y,
        ),
    ))
}

/// Intersects two finite-direction lines without accepting a parallel numerical fallback.
fn line_intersection(
    start: Point2,
    end: Point2,
    other_start: Point2,
    other_end: Point2,
) -> Option<Point2> {
    let first = Vector2::new(end.x - start.x, end.y - start.y);
    let second = Vector2::new(other_end.x - other_start.x, other_end.y - other_start.y);
    let denominator = first.x * second.y - first.y * second.x;
    if denominator.abs() <= 1.0e-12 {
        return None;
    }
    let delta = Vector2::new(other_start.x - start.x, other_start.y - start.y);
    let factor = (delta.x * second.y - delta.y * second.x) / denominator;
    let point = Point2::new(start.x + first.x * factor, start.y + first.y * factor);
    point.is_finite().then_some(point)
}

/// Builds one compact cubic approximation of the outer circular corner arc between analytic line offsets.
///
/// # Errors
///
/// Returns finite-coordinate diagnostics without allocating brush primitives or polygon swarms.
fn compact_round_join(
    center: Point2,
    start: Point2,
    end: Point2,
    counter_clockwise: bool,
) -> Result<crate::CubicBezierSegment, CurveError> {
    let start_vector = Vector2::new(start.x - center.x, start.y - center.y);
    let end_vector = Vector2::new(end.x - center.x, end.y - center.y);
    let radius = start_vector.x.hypot(start_vector.y);
    if radius == 0.0 || (end_vector.x.hypot(end_vector.y) - radius).abs() > 1.0e-6 {
        return Err(CurveError::new(
            "curve.offset.join",
            "outer offset join is not circular",
        ));
    }
    let start_angle = start_vector.y.atan2(start_vector.x);
    let mut sweep = end_vector.y.atan2(end_vector.x) - start_angle;
    if counter_clockwise && sweep < 0.0 {
        sweep += std::f64::consts::TAU;
    } else if !counter_clockwise && sweep > 0.0 {
        sweep -= std::f64::consts::TAU;
    }
    if sweep.abs() > std::f64::consts::FRAC_PI_2 + 1.0e-9 {
        return Err(CurveError::new(
            "curve.offset.join",
            "outer offset join requires deterministic bevel fallback",
        ));
    }
    let handle = 4.0 / 3.0 * (sweep * 0.25).tan() * radius;
    let end_angle = start_angle + sweep;
    crate::CubicBezierSegment::new(
        start,
        Point2::new(
            start.x - start_angle.sin() * handle,
            start.y + start_angle.cos() * handle,
        ),
        Point2::new(
            end.x + end_angle.sin() * handle,
            end.y - end_angle.cos() * handle,
        ),
        end,
    )
}

/// Splits a finite outer circular corner into deterministic cubic arcs no wider than ninety degrees.
///
/// This filled-region-only helper preserves exact offset endpoints and tangent continuity for
/// convex outward corners that exceed the single compact Stage 20J arc sweep.
///
/// # Errors
///
/// Returns the existing finite-circle diagnostic when the two offset endpoints cannot define one
/// stable circular join; callers retain their atomic bounded-work error boundary.
fn region_round_join_segments(
    center: Point2,
    start: Point2,
    end: Point2,
    counter_clockwise: bool,
) -> Result<Vec<crate::CubicBezierSegment>, CurveError> {
    if (start.x - end.x).hypot(start.y - end.y) <= 1.0e-6 {
        return Ok(Vec::new());
    }
    let start_vector = Vector2::new(start.x - center.x, start.y - center.y);
    let end_vector = Vector2::new(end.x - center.x, end.y - center.y);
    let radius = start_vector.x.hypot(start_vector.y);
    if radius == 0.0 || (end_vector.x.hypot(end_vector.y) - radius).abs() > 1.0e-6 {
        return Err(CurveError::new(
            "curve.offset.join",
            "outer offset join is not circular",
        ));
    }
    let start_angle = start_vector.y.atan2(start_vector.x);
    let mut sweep = end_vector.y.atan2(end_vector.x) - start_angle;
    if counter_clockwise && sweep < 0.0 {
        sweep += std::f64::consts::TAU;
    } else if !counter_clockwise && sweep > 0.0 {
        sweep -= std::f64::consts::TAU;
    }
    let count = (sweep.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize;
    let count = count.max(1);
    let step = sweep / count as f64;
    let mut joins = Vec::new();
    joins.try_reserve(count).map_err(|_| {
        CurveError::new(
            "curve.offset.allocation",
            "subdivided region round-join allocation failed",
        )
    })?;
    for ordinal in 0..count {
        let angle = start_angle + step * ordinal as f64;
        let next_angle = angle + step;
        let arc_start = if ordinal == 0 {
            start
        } else {
            Point2::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        };
        let arc_end = if ordinal + 1 == count {
            end
        } else {
            Point2::new(
                center.x + radius * next_angle.cos(),
                center.y + radius * next_angle.sin(),
            )
        };
        let handle = 4.0 / 3.0 * (step * 0.25).tan() * radius;
        joins.push(crate::CubicBezierSegment::new(
            arc_start,
            Point2::new(
                arc_start.x - angle.sin() * handle,
                arc_start.y + angle.cos() * handle,
            ),
            Point2::new(
                arc_end.x + next_angle.sin() * handle,
                arc_end.y - next_angle.cos() * handle,
            ),
            arc_end,
        )?);
    }
    Ok(joins)
}

/// Moves one derived endpoint and its adjacent cubic control together to preserve join tangency.
///
/// # Errors
///
/// Returns finite line/cubic construction diagnostics without changing the segment start.
fn replace_segment_end_preserving_tangent(
    segment: CurveSegment,
    end: Point2,
) -> Result<CurveSegment, CurveError> {
    match segment {
        CurveSegment::Line(line) => Ok(CurveSegment::Line(LineSegment::new(line.start(), end)?)),
        CurveSegment::CubicBezier(cubic) => {
            let delta = Vector2::new(end.x - cubic.end().x, end.y - cubic.end().y);
            Ok(CurveSegment::CubicBezier(crate::CubicBezierSegment::new(
                cubic.start(),
                cubic.control_1(),
                Point2::new(cubic.control_2().x + delta.x, cubic.control_2().y + delta.y),
                end,
            )?))
        }
    }
}

/// Extends only open paths to a finite planning envelope before the immutable offset step.
///
/// # Errors
///
/// Returns endpoint tangent or finite geometry diagnostics without altering the source path.
fn extend_endpoints(
    path: &CurvePath,
    policy: PathOffsetEndpointPolicy,
) -> Result<ExtendedOffsetSource, CurveError> {
    let PathOffsetEndpointPolicy::TangentialExtension { bounds } = policy else {
        return Ok(ExtendedOffsetSource {
            path: path.clone(),
            source_intervals: authored_source_intervals(path)?,
        });
    };
    if path.closure() == PathClosure::Closed {
        return Ok(ExtendedOffsetSource {
            path: path.clone(),
            source_intervals: authored_source_intervals(path)?,
        });
    }
    let first = path.unit_tangent_at(PathLocation::new(0, 0.0)?)?;
    let last_index = path.segments().len() - 1;
    let last = path.unit_tangent_at(PathLocation::new(last_index, 1.0)?)?;
    let start = ray_to_bounds(path.start(), first.scale(-1.0), bounds)?;
    let end = ray_to_bounds(path.end(), last, bounds)?;
    let mut segments = Vec::with_capacity(path.segments().len() + 2);
    let mut source_intervals = Vec::with_capacity(path.segments().len() + 2);
    let authored_start = PathLocation::new(0, 0.0)?;
    let authored_end = PathLocation::new(last_index, 1.0)?;
    if start != path.start() {
        segments.push(CurveSegment::Line(LineSegment::new(start, path.start())?));
        source_intervals.push((authored_start, authored_start));
    }
    segments.extend(path.segments().iter().copied());
    source_intervals.extend(authored_source_intervals(path)?);
    if path.end() != end {
        segments.push(CurveSegment::Line(LineSegment::new(path.end(), end)?));
        source_intervals.push((authored_end, authored_end));
    }
    Ok(ExtendedOffsetSource {
        path: CurvePath::new(segments, PathClosure::Open)?,
        source_intervals,
    })
}

/// Returns exact authored source intervals for every original path segment.
///
/// # Errors
///
/// Returns a stable location diagnostic only if an internal segment index cannot be represented.
fn authored_source_intervals(
    path: &CurvePath,
) -> Result<Vec<(PathLocation, PathLocation)>, CurveError> {
    (0..path.segments().len())
        .map(|index| {
            Ok((
                PathLocation::new(index, 0.0)?,
                PathLocation::new(index, 1.0)?,
            ))
        })
        .collect()
}

/// Maps one construction-path location back into its exact authored source interval.
///
/// # Errors
///
/// Returns a stable provenance diagnostic if the construction segment or authored interval is invalid.
fn remap_extended_source_location(
    location: PathLocation,
    construction_segment_index: usize,
    source_interval: (PathLocation, PathLocation),
) -> Result<PathLocation, CurveError> {
    if location.segment_index() != construction_segment_index {
        return Err(CurveError::new(
            "curve.offset.source_interval",
            "offset source provenance escaped its construction segment",
        ));
    }
    if source_interval.0 == source_interval.1 {
        return Ok(source_interval.0);
    }
    if source_interval.0.segment_index() != source_interval.1.segment_index() {
        return Err(CurveError::new(
            "curve.offset.source_interval",
            "offset source provenance crossed authored segments",
        ));
    }
    let parameter = source_interval.0.parameter()
        + (source_interval.1.parameter() - source_interval.0.parameter()) * location.parameter();
    PathLocation::new(source_interval.0.segment_index(), parameter)
}

/// Classifies one closed path's exact signed area while treating tolerance-sized loops as collapsed.
///
/// # Errors
///
/// Returns a stable numeric diagnostic when polynomial area integration is non-finite.
fn path_winding(path: &CurvePath, tolerance: f64) -> Result<Option<WindingDirection>, CurveError> {
    let area = path_signed_area(path)?;
    if area.abs() <= tolerance * tolerance {
        Ok(None)
    } else if area > 0.0 {
        Ok(Some(WindingDirection::Positive))
    } else {
        Ok(Some(WindingDirection::Negative))
    }
}

/// Integrates exact line/cubic polynomial signed area without flattening or renderer geometry.
///
/// # Errors
///
/// Returns a stable numeric diagnostic if finite source controls overflow during integration.
fn path_signed_area(path: &CurvePath) -> Result<f64, CurveError> {
    let mut double_area = 0.0;
    for segment in path.segments() {
        let (x, y) = match segment {
            CurveSegment::Line(line) => (
                [line.start().x, line.end().x - line.start().x, 0.0, 0.0],
                [line.start().y, line.end().y - line.start().y, 0.0, 0.0],
            ),
            CurveSegment::CubicBezier(cubic) => {
                let points = [
                    cubic.start(),
                    cubic.control_1(),
                    cubic.control_2(),
                    cubic.end(),
                ];
                (
                    cubic_polynomial_coefficients(
                        points[0].x,
                        points[1].x,
                        points[2].x,
                        points[3].x,
                    ),
                    cubic_polynomial_coefficients(
                        points[0].y,
                        points[1].y,
                        points[2].y,
                        points[3].y,
                    ),
                )
            }
        };
        let dx = [x[1], 2.0 * x[2], 3.0 * x[3]];
        let dy = [y[1], 2.0 * y[2], 3.0 * y[3]];
        let mut integrand = [0.0; 6];
        for (position_power, (x_coefficient, y_coefficient)) in x.into_iter().zip(y).enumerate() {
            for (derivative_power, (dx_coefficient, dy_coefficient)) in
                dx.into_iter().zip(dy).enumerate()
            {
                integrand[position_power + derivative_power] +=
                    x_coefficient * dy_coefficient - y_coefficient * dx_coefficient;
            }
        }
        double_area += integrand
            .into_iter()
            .enumerate()
            .map(|(power, coefficient)| coefficient / (power + 1) as f64)
            .sum::<f64>();
    }
    let area = double_area * 0.5;
    if area.is_finite() {
        Ok(area)
    } else {
        Err(CurveError::new(
            "curve.offset.numeric_overflow",
            "closed path winding arithmetic overflowed",
        ))
    }
}

/// Returns ascending-power coefficients for one cubic Bezier scalar coordinate.
fn cubic_polynomial_coefficients(start: f64, first: f64, second: f64, end: f64) -> [f64; 4] {
    [
        start,
        3.0 * (first - start),
        3.0 * (second - 2.0 * first + start),
        end - 3.0 * second + 3.0 * first - start,
    ]
}

/// Finds the first finite forward ray exit from one inclusive planning rectangle.
///
/// # Errors
///
/// Returns a stable tangent diagnostic when the ray cannot reach a rectangle boundary.
fn ray_to_bounds(origin: Point2, direction: Vector2, bounds: Bounds) -> Result<Point2, CurveError> {
    let mut candidates = Vec::with_capacity(4);
    if direction.x != 0.0 {
        for x in [bounds.min.x, bounds.max.x] {
            let t = (x - origin.x) / direction.x;
            if t.is_finite() && t >= 0.0 {
                let y = origin.y + direction.y * t;
                if y.is_finite() && (bounds.min.y..=bounds.max.y).contains(&y) {
                    candidates.push((t, Point2::new(x, y)));
                }
            }
        }
    }
    if direction.y != 0.0 {
        for y in [bounds.min.y, bounds.max.y] {
            let t = (y - origin.y) / direction.y;
            if t.is_finite() && t >= 0.0 {
                let x = origin.x + direction.x * t;
                if x.is_finite() && (bounds.min.x..=bounds.max.x).contains(&x) {
                    candidates.push((t, Point2::new(x, y)));
                }
            }
        }
    }
    candidates
        .into_iter()
        .max_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, point)| point)
        .ok_or(CurveError::new(
            "curve.offset.endpoint_tangent",
            "path endpoint tangent cannot reach the planning bounds",
        ))
}

/// Offsets one line or cubic into source-adjacent runs without bridging omitted source geometry.
///
/// # Errors
///
/// Propagates source tangent, cusp-isolation, finite-coordinate, cancellation, and limit errors.
fn offset_segment_runs(
    segment: CurveSegment,
    source_segment_index: usize,
    distance: f64,
    limits: PathOffsetLimits,
    retain_reversed: bool,
    cusp_budget: &mut CuspIsolationBudget,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<TracedOffsetSegment>>, CurveError> {
    let translate = |point: Point2, normal: Vector2| {
        Point2::new(point.x + normal.x * distance, point.y + normal.y * distance)
    };
    match segment {
        CurveSegment::Line(line) => {
            let start_normal = match segment.unit_normal_at(0.0) {
                Ok(normal) => normal,
                Err(error) if error.path() == "curve.path.tangent.stationary" => {
                    return Ok(Vec::new());
                }
                Err(error) => return Err(error),
            };
            let end_normal = segment.unit_normal_at(1.0)?;
            Ok(vec![vec![TracedOffsetSegment {
                segment: CurveSegment::Line(LineSegment::new(
                    translate(line.start(), start_normal),
                    translate(line.end(), end_normal),
                )?),
                source_start: PathLocation::new(source_segment_index, 0.0)?,
                source_end: PathLocation::new(source_segment_index, 1.0)?,
                orientation: Some(OffsetOrientation::Retained),
            }]])
        }
        CurveSegment::CubicBezier(cubic) => cubic_offset_runs(
            cubic,
            source_segment_index,
            distance,
            limits,
            retain_reversed,
            cusp_budget,
            is_cancelled,
        ),
    }
}

/// Isolates cubic offset orientation before fitting each retained source-adjacent run.
///
/// # Errors
///
/// Returns stable cusp work/depth, fitting, tangent, cancellation, or finite-arithmetic diagnostics.
fn cubic_offset_runs(
    cubic: crate::CubicBezierSegment,
    source_segment_index: usize,
    distance: f64,
    limits: PathOffsetLimits,
    retain_reversed: bool,
    cusp_budget: &mut CuspIsolationBudget,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<TracedOffsetSegment>>, CurveError> {
    let stationary_parameters = cubic_stationary_parameters(cubic)?;
    let mut source_breaks = Vec::with_capacity(stationary_parameters.len() + 2);
    source_breaks.push(0.0);
    source_breaks.extend(stationary_parameters);
    source_breaks.push(1.0);
    let mut intervals = Vec::new();
    for source_window in source_breaks.windows(2) {
        let source_start = source_window[0];
        let source_end = source_window[1];
        let source_piece = cubic_subinterval(cubic, source_start, source_end)?;
        let local_runs = isolate_cubic_offset_intervals(
            source_piece,
            distance,
            limits,
            retain_reversed,
            cusp_budget,
            is_cancelled,
        )?;
        for local_run in local_runs {
            intervals.push(
                local_run
                    .into_iter()
                    .map(|interval| RetainedCubicInterval {
                        source_start: source_start
                            + (source_end - source_start) * interval.source_start,
                        source_end: source_start
                            + (source_end - source_start) * interval.source_end,
                        orientation: interval.orientation,
                    })
                    .collect::<Vec<_>>(),
            );
        }
    }
    let mut runs = Vec::new();
    for interval_run in intervals {
        let source_start = interval_run
            .first()
            .expect("retained interval run is nonempty")
            .source_start;
        let source_end = interval_run
            .last()
            .expect("retained interval run is nonempty")
            .source_end;
        let mut retained_cubic = cubic_subinterval(cubic, source_start, source_end)?;
        if retain_reversed {
            retained_cubic = canonicalize_cubic_endpoint_handles(retained_cubic, limits.tolerance)?;
        }
        if CurveSegment::CubicBezier(retained_cubic).control_polygon_length()? == 0.0 {
            continue;
        }
        let fitted = adaptive_cubic_offsets(
            retained_cubic,
            source_segment_index,
            source_start,
            source_end,
            interval_run
                .first()
                .expect("retained interval run is nonempty")
                .orientation,
            distance,
            limits,
            retain_reversed,
            is_cancelled,
        )?;
        if !fitted.is_empty() {
            runs.push(fitted);
        }
    }
    Ok(runs)
}

/// Finds exact interior parameters where both cubic derivative coordinates vanish.
///
/// Stationary points are topology boundaries for normal offsets: splitting them before cusp
/// classification gives each side its own one-sided limiting tangent and prevents an offset fit
/// from sampling an undefined interior normal. The source cubic itself is not flattened or
/// otherwise approximated.
///
/// # Errors
///
/// Returns a stable numeric-overflow diagnostic when derivative coefficients cannot remain finite.
fn cubic_stationary_parameters(cubic: crate::CubicBezierSegment) -> Result<Vec<f64>, CurveError> {
    let coefficients = |start: f64, control_1: f64, control_2: f64, end: f64| {
        [
            -start + 3.0 * control_1 - 3.0 * control_2 + end,
            2.0 * (start - 2.0 * control_1 + control_2),
            control_1 - start,
        ]
    };
    let x = coefficients(
        cubic.start().x,
        cubic.control_1().x,
        cubic.control_2().x,
        cubic.end().x,
    );
    let y = coefficients(
        cubic.start().y,
        cubic.control_1().y,
        cubic.control_2().y,
        cubic.end().y,
    );
    if x.into_iter()
        .chain(y)
        .any(|coefficient| !coefficient.is_finite())
    {
        return Err(CurveError::new(
            "curve.offset.numeric_overflow",
            "cubic stationary-point arithmetic overflowed",
        ));
    }
    let x_scale = x.into_iter().map(f64::abs).fold(0.0_f64, f64::max);
    let y_scale = y.into_iter().map(f64::abs).fold(0.0_f64, f64::max);
    let selected = if x_scale >= y_scale { x } else { y };
    let mut candidates = quadratic_real_roots(selected);
    candidates.retain(|parameter| strictly_interior(*parameter));
    candidates.sort_by(f64::total_cmp);
    candidates.dedup_by(|right, left| (*right - *left).abs() <= 1.0e-12);
    let segment = CurveSegment::CubicBezier(cubic);
    let derivative_scale = segment.control_polygon_length()?.max(f64::MIN_POSITIVE);
    let mut stationary = Vec::new();
    for parameter in candidates {
        let derivative = segment.derivative_at(parameter)?;
        if derivative.x.hypot(derivative.y) <= derivative_scale * 1.0e-8 {
            stationary.push(parameter);
        }
    }
    Ok(stationary)
}

/// Solves one finite quadratic or linear polynomial without introducing complex roots.
fn quadratic_real_roots(coefficients: [f64; 3]) -> Vec<f64> {
    let [quadratic, linear, constant] = coefficients;
    let scale = quadratic.abs().max(linear.abs()).max(constant.abs());
    if scale == 0.0 {
        return Vec::new();
    }
    let epsilon = scale * 64.0 * f64::EPSILON;
    if quadratic.abs() <= epsilon {
        return (linear.abs() > epsilon)
            .then(|| -constant / linear)
            .into_iter()
            .collect();
    }
    let discriminant = linear.mul_add(linear, -4.0 * quadratic * constant);
    if discriminant < -epsilon * scale {
        return Vec::new();
    }
    if discriminant.abs() <= epsilon * scale {
        return vec![-linear / (2.0 * quadratic)];
    }
    let root = discriminant.max(0.0).sqrt();
    let q = -0.5 * (linear + linear.signum() * root);
    if q == 0.0 {
        return vec![(-linear + root) / (2.0 * quadratic)];
    }
    vec![q / quadratic, constant / q]
}

/// Extracts one finite cubic source interval without changing its authored parameter bounds.
///
/// # Errors
///
/// Returns canonical split diagnostics for an invalid or non-finite interval.
fn cubic_subinterval(
    cubic: crate::CubicBezierSegment,
    start: f64,
    end: f64,
) -> Result<crate::CubicBezierSegment, CurveError> {
    if start == 0.0 && end == 1.0 {
        return Ok(cubic);
    }
    if !start.is_finite() || !end.is_finite() || start < 0.0 || end > 1.0 || start >= end {
        return Err(CurveError::new(
            "curve.offset.source_interval",
            "retained cubic source interval must be finite and ordered",
        ));
    }
    let prefix = if end == 1.0 {
        cubic
    } else {
        cubic.split(end)?.0
    };
    if start == 0.0 {
        Ok(prefix)
    } else {
        Ok(prefix.split(start / end)?.1)
    }
}

/// Dyadically classifies cubic intervals by the signed offset-orientation numerator.
///
/// Positive and negative `|v|^3 - distance * cross(v, a)` intervals are emitted as separate
/// monotonic runs so cleanup can solve the complete reversal loop at its exact crossings. An
/// unresolved singular interval is omitted only after its conservative offset-locus length is
/// within the fixed geometry tolerance.
///
/// # Errors
///
/// Returns stable work/depth, cancellation, or non-finite-arithmetic diagnostics atomically.
fn isolate_cubic_offset_intervals(
    cubic: crate::CubicBezierSegment,
    distance: f64,
    limits: PathOffsetLimits,
    retain_reversed: bool,
    budget: &mut CuspIsolationBudget,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<RetainedCubicInterval>>, CurveError> {
    let mut pending = vec![(cubic, 0_u8, 0.0_f64, 1.0_f64)];
    let mut retained = Vec::new();
    while let Some((candidate, depth, source_start, source_end)) = pending.pop() {
        if is_cancelled() {
            return Err(cancelled());
        }
        let candidate_length = CurveSegment::CubicBezier(candidate).control_polygon_length()?;
        if candidate_length == 0.0 {
            continue;
        }
        budget.examined_intervals =
            budget
                .examined_intervals
                .checked_add(1)
                .ok_or(CurveError::new(
                    "curve.offset.cusp_limit",
                    "path offset cusp-isolation work limit exceeded",
                ))?;
        if budget.examined_intervals > budget.maximum_intervals {
            return Err(CurveError::new(
                "curve.offset.cusp_limit",
                "path offset cusp-isolation work limit exceeded",
            ));
        }
        let bounds = cubic_offset_orientation_bounds(candidate, distance)?;
        match classify_offset_orientation(bounds) {
            orientation @ OffsetOrientation::Retained => {
                retained.push(RetainedCubicInterval {
                    source_start,
                    source_end,
                    orientation,
                });
            }
            orientation @ OffsetOrientation::Reversed if retain_reversed => {
                retained.push(RetainedCubicInterval {
                    source_start,
                    source_end,
                    orientation,
                });
            }
            OffsetOrientation::Reversed => {}
            OffsetOrientation::Uncertain => {
                if cubic_interval_has_zero_witness(candidate, distance)?
                    && (uncertain_offset_locus_length_bound(bounds)? <= limits.tolerance
                        || candidate_length <= crate::curves::ABSOLUTE_TOLERANCE * 64.0)
                {
                    if retain_reversed {
                        retained.extend(resolve_constant_gap_cusp_intervals(
                            candidate,
                            distance,
                            source_start,
                            source_end,
                        )?);
                    }
                    continue;
                }
                if depth >= limits.maximum_subdivision_depth {
                    return Err(CurveError::new(
                        "curve.offset.subdivision_limit",
                        "cubic offset cusp cannot be isolated within the subdivision limit",
                    ));
                }
                let (left, right) = candidate.split(0.5)?;
                let source_middle = (source_start + source_end) * 0.5;
                pending.push((right, depth + 1, source_middle, source_end));
                pending.push((left, depth + 1, source_start, source_middle));
            }
        }
    }
    let mut runs = Vec::<Vec<RetainedCubicInterval>>::new();
    for interval in retained {
        if runs
            .last()
            .and_then(|run| run.last())
            .is_some_and(|previous| {
                previous.source_end == interval.source_start
                    && previous.orientation == interval.orientation
            })
        {
            runs.last_mut()
                .expect("adjacent retained interval has a run")
                .push(interval);
        } else {
            runs.push(vec![interval]);
        }
    }
    Ok(runs)
}

/// Resolves every sign-changing cusp inside one already isolated Constant-gap source interval.
///
/// The caller has proved that the complete offset locus of this interval is shorter than the
/// fitting tolerance. This routine still preserves its topology: it brackets each orientation
/// root on `[0, 0.5]` or `[0.5, 1]`, bisects to the final representable parameter, and returns
/// source intervals that meet at the exact same parameter. No cusp band is retained or omitted.
///
/// # Errors
///
/// Returns finite offset-orientation diagnostics without publishing a partially resolved interval.
fn resolve_constant_gap_cusp_intervals(
    cubic: crate::CubicBezierSegment,
    distance: f64,
    source_start: f64,
    source_end: f64,
) -> Result<Vec<RetainedCubicInterval>, CurveError> {
    let samples = [0.0, 0.5, 1.0];
    let values = samples
        .map(|parameter| cubic_offset_orientation_value(cubic, distance, parameter))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let mut roots = Vec::new();
    for index in 0..2 {
        let start = samples[index];
        let end = samples[index + 1];
        let start_value = values[index];
        let end_value = values[index + 1];
        if start_value == 0.0 {
            roots.push(start);
        }
        if start_value.signum() != end_value.signum() {
            roots.push(bisect_offset_orientation_root(
                cubic,
                distance,
                start,
                end,
                start_value,
            )?);
        }
        if index == 1 && end_value == 0.0 {
            roots.push(end);
        }
    }
    roots.retain(|parameter| strictly_interior(*parameter));
    roots.sort_by(f64::total_cmp);
    roots.dedup_by(|right, left| (*right - *left).abs() <= 1.0e-15);
    let mut boundaries = Vec::with_capacity(roots.len() + 2);
    boundaries.push(0.0);
    boundaries.extend(roots);
    boundaries.push(1.0);
    let mut intervals = Vec::with_capacity(boundaries.len().saturating_sub(1));
    for pair in boundaries.windows(2) {
        if pair[0] == pair[1] {
            continue;
        }
        let midpoint = (pair[0] + pair[1]) * 0.5;
        let value = cubic_offset_orientation_value(cubic, distance, midpoint)?;
        let orientation = if value < 0.0 {
            OffsetOrientation::Reversed
        } else {
            OffsetOrientation::Retained
        };
        intervals.push(RetainedCubicInterval {
            source_start: source_start + (source_end - source_start) * pair[0],
            source_end: source_start + (source_end - source_start) * pair[1],
            orientation,
        });
    }
    Ok(intervals)
}

/// Bisects one proven sign-changing cubic offset-orientation bracket deterministically.
///
/// # Errors
///
/// Returns finite orientation-evaluation diagnostics and never accepts an unbracketed root.
fn bisect_offset_orientation_root(
    cubic: crate::CubicBezierSegment,
    distance: f64,
    mut start: f64,
    mut end: f64,
    mut start_value: f64,
) -> Result<f64, CurveError> {
    for _ in 0..80 {
        let middle = (start + end) * 0.5;
        if middle == start || middle == end {
            break;
        }
        let value = cubic_offset_orientation_value(cubic, distance, middle)?;
        if value == 0.0 {
            return Ok(middle);
        }
        if value.signum() == start_value.signum() {
            start = middle;
            start_value = value;
        } else {
            end = middle;
        }
    }
    Ok((start + end) * 0.5)
}

/// Detects an exact dyadic zero or a signed reversal bracket inside one uncertain cubic interval.
///
/// # Errors
///
/// Returns a stable numeric-overflow diagnostic without using a non-finite sign witness.
fn cubic_interval_has_zero_witness(
    cubic: crate::CubicBezierSegment,
    distance: f64,
) -> Result<bool, CurveError> {
    let values = [0.0, 0.5, 1.0]
        .map(|parameter| cubic_offset_orientation_value(cubic, distance, parameter))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Ok(values.contains(&0.0) || (minimum < 0.0 && maximum > 0.0))
}

/// Evaluates the exact signed cubic offset-orientation numerator at one local parameter.
///
/// # Errors
///
/// Returns a stable numeric-overflow diagnostic instead of accepting a non-finite sign.
fn cubic_offset_orientation_value(
    cubic: crate::CubicBezierSegment,
    distance: f64,
    parameter: f64,
) -> Result<f64, CurveError> {
    let segment = CurveSegment::CubicBezier(cubic);
    let velocity = segment.derivative_at(parameter)?;
    let inverse = 1.0 - parameter;
    let acceleration_start = Vector2::new(
        6.0 * (cubic.control_2().x - 2.0 * cubic.control_1().x + cubic.start().x),
        6.0 * (cubic.control_2().y - 2.0 * cubic.control_1().y + cubic.start().y),
    );
    let acceleration_end = Vector2::new(
        6.0 * (cubic.end().x - 2.0 * cubic.control_2().x + cubic.control_1().x),
        6.0 * (cubic.end().y - 2.0 * cubic.control_2().y + cubic.control_1().y),
    );
    let acceleration = Vector2::new(
        inverse * acceleration_start.x + parameter * acceleration_end.x,
        inverse * acceleration_start.y + parameter * acceleration_end.y,
    );
    let speed = velocity.x.hypot(velocity.y);
    let value =
        speed.powi(3) - distance * (velocity.x * acceleration.y - velocity.y * acceleration.x);
    value.is_finite().then_some(value).ok_or(CurveError::new(
        "curve.offset.numeric_overflow",
        "cubic offset orientation arithmetic overflowed",
    ))
}

/// Computes conservative speed and signed orientation-numerator bounds from derivative control hulls.
///
/// # Errors
///
/// Returns a stable numeric-overflow diagnostic instead of classifying non-finite arithmetic.
fn cubic_offset_orientation_bounds(
    cubic: crate::CubicBezierSegment,
    distance: f64,
) -> Result<OffsetOrientationBounds, CurveError> {
    let velocity = [
        Vector2::new(
            3.0 * (cubic.control_1().x - cubic.start().x),
            3.0 * (cubic.control_1().y - cubic.start().y),
        ),
        Vector2::new(
            3.0 * (cubic.control_2().x - cubic.control_1().x),
            3.0 * (cubic.control_2().y - cubic.control_1().y),
        ),
        Vector2::new(
            3.0 * (cubic.end().x - cubic.control_2().x),
            3.0 * (cubic.end().y - cubic.control_2().y),
        ),
    ];
    if velocity
        .iter()
        .any(|value| !value.x.is_finite() || !value.y.is_finite())
    {
        return Err(CurveError::new(
            "curve.offset.numeric_overflow",
            "cubic offset orientation arithmetic overflowed",
        ));
    }
    let (minimum_x, maximum_x) = velocity.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(minimum, maximum), value| (minimum.min(value.x), maximum.max(value.x)),
    );
    let (minimum_y, maximum_y) = velocity.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(minimum, maximum), value| (minimum.min(value.y), maximum.max(value.y)),
    );
    let distance_to_x = if minimum_x <= 0.0 && maximum_x >= 0.0 {
        0.0
    } else {
        minimum_x.abs().min(maximum_x.abs())
    };
    let distance_to_y = if minimum_y <= 0.0 && maximum_y >= 0.0 {
        0.0
    } else {
        minimum_y.abs().min(maximum_y.abs())
    };
    let minimum_speed = distance_to_x.hypot(distance_to_y);
    let maximum_speed = velocity
        .iter()
        .map(|value| value.x.hypot(value.y))
        .fold(0.0_f64, f64::max);
    let cross = |left: Vector2, right: Vector2| left.x * right.y - left.y * right.x;
    let cross_controls = [
        2.0 * cross(velocity[0], velocity[1]),
        cross(velocity[0], velocity[2]),
        2.0 * cross(velocity[1], velocity[2]),
    ];
    let cross_minimum = cross_controls.iter().copied().fold(f64::INFINITY, f64::min);
    let cross_maximum = cross_controls
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let distance_cross_minimum = (distance * cross_minimum).min(distance * cross_maximum);
    let distance_cross_maximum = (distance * cross_minimum).max(distance * cross_maximum);
    let lower = minimum_speed.powi(3) - distance_cross_maximum;
    let upper = maximum_speed.powi(3) - distance_cross_minimum;
    if !minimum_speed.is_finite()
        || !maximum_speed.is_finite()
        || cross_controls.iter().any(|value| !value.is_finite())
        || !lower.is_finite()
        || !upper.is_finite()
    {
        return Err(CurveError::new(
            "curve.offset.numeric_overflow",
            "cubic offset orientation arithmetic overflowed",
        ));
    }
    Ok(OffsetOrientationBounds {
        lower,
        upper,
        minimum_speed,
    })
}

/// Classifies one conservative orientation interval without sampling a sign-changing midpoint.
fn classify_offset_orientation(bounds: OffsetOrientationBounds) -> OffsetOrientation {
    if bounds.lower > 0.0 {
        OffsetOrientation::Retained
    } else if bounds.upper < 0.0 {
        OffsetOrientation::Reversed
    } else {
        OffsetOrientation::Uncertain
    }
}

/// Bounds the complete uncertain offset-locus length from numerator and source-speed bounds.
///
/// # Errors
///
/// Returns a stable numeric-overflow diagnostic instead of accepting an unbounded finite interval.
fn uncertain_offset_locus_length_bound(bounds: OffsetOrientationBounds) -> Result<f64, CurveError> {
    if bounds.minimum_speed == 0.0 {
        return Ok(f64::INFINITY);
    }
    let numerator = bounds.lower.abs().max(bounds.upper.abs());
    let length = numerator / bounds.minimum_speed.powi(2);
    length.is_finite().then_some(length).ok_or(CurveError::new(
        "curve.offset.numeric_overflow",
        "cubic offset cusp-band arithmetic overflowed",
    ))
}

/// Fits a cubic's normal offset adaptively with deterministic dyadic De Casteljau subdivision.
///
/// # Errors
///
/// Returns cancellation, tangent, finite-coordinate, segment-limit, or subdivision-limit diagnostics
/// before publishing any fitted piece.
#[allow(clippy::too_many_arguments)]
fn adaptive_cubic_offsets(
    cubic: crate::CubicBezierSegment,
    source_segment_index: usize,
    source_parameter_start: f64,
    source_parameter_end: f64,
    orientation: OffsetOrientation,
    distance: f64,
    limits: PathOffsetLimits,
    retain_sub_tolerance_locus: bool,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<TracedOffsetSegment>, CurveError> {
    let mut pending = vec![(
        0_u8,
        0.0_f64,
        1.0_f64,
        source_parameter_start,
        source_parameter_end,
    )];
    let mut pieces: Vec<TracedOffsetSegment> = Vec::new();
    while let Some((depth, local_start, local_end, source_start, source_end)) = pending.pop() {
        if is_cancelled() {
            return Err(cancelled());
        }
        let exact_start = exact_cubic_offset_point(cubic, distance, local_start)?;
        let exact_end = exact_cubic_offset_point(cubic, distance, local_end)?;
        let line_error = cubic_offset_interval_line_error(
            cubic,
            exact_start,
            exact_end,
            distance,
            local_start,
            local_end,
        )?;
        if line_error <= limits.tolerance {
            if exact_start == exact_end {
                continue;
            }
            if retain_sub_tolerance_locus
                || (exact_start.x - exact_end.x).hypot(exact_start.y - exact_end.y)
                    > limits.tolerance
            {
                let candidate = CurveSegment::Line(LineSegment::new(exact_start, exact_end)?);
                if !candidate_preserves_offset_boundary_tangents(
                    cubic,
                    distance,
                    local_start,
                    local_end,
                    candidate,
                )? {
                    // A positionally accurate chord may still reverse at a cusp-adjacent public
                    // endpoint. Continue to the tangent-preserving cubic fits before subdividing.
                } else {
                    let mut segment = candidate;
                    if let Some(previous) = pieces.last() {
                        segment = replace_segment_start(segment, previous.segment.end())?;
                    }
                    pieces.push(TracedOffsetSegment {
                        segment,
                        source_start: PathLocation::new(source_segment_index, source_start)?,
                        source_end: PathLocation::new(source_segment_index, source_end)?,
                        orientation: Some(orientation),
                    });
                    if pieces.len() > limits.maximum_segments {
                        return Err(CurveError::new(
                            "curve.offset.segment_limit",
                            "path offset segment limit exceeded",
                        ));
                    }
                    continue;
                }
            } else {
                continue;
            }
        }
        let fitted = fit_offset_cubic_interval(cubic, distance, local_start, local_end)?;
        if cubic_offset_interval_error(cubic, fitted, distance, local_start, local_end)?
            <= limits.tolerance
        {
            let mut segment = CurveSegment::CubicBezier(fitted);
            if let Some(previous) = pieces.last() {
                segment = replace_segment_start(segment, previous.segment.end())?;
            }
            pieces.push(TracedOffsetSegment {
                segment,
                source_start: PathLocation::new(source_segment_index, source_start)?,
                source_end: PathLocation::new(source_segment_index, source_end)?,
                orientation: Some(orientation),
            });
            if pieces.len() > limits.maximum_segments {
                return Err(CurveError::new(
                    "curve.offset.segment_limit",
                    "path offset segment limit exceeded",
                ));
            }
            continue;
        }
        let interpolated =
            fit_offset_cubic_interpolating_interval(cubic, distance, local_start, local_end)?;
        if cubic_offset_interval_error(cubic, interpolated, distance, local_start, local_end)?
            <= limits.tolerance
            && candidate_preserves_offset_boundary_tangents(
                cubic,
                distance,
                local_start,
                local_end,
                CurveSegment::CubicBezier(interpolated),
            )?
        {
            let mut segment = CurveSegment::CubicBezier(interpolated);
            if let Some(previous) = pieces.last() {
                segment = replace_segment_start(segment, previous.segment.end())?;
            }
            pieces.push(TracedOffsetSegment {
                segment,
                source_start: PathLocation::new(source_segment_index, source_start)?,
                source_end: PathLocation::new(source_segment_index, source_end)?,
                orientation: Some(orientation),
            });
            if pieces.len() > limits.maximum_segments {
                return Err(CurveError::new(
                    "curve.offset.segment_limit",
                    "path offset segment limit exceeded",
                ));
            }
            continue;
        }
        if depth >= limits.maximum_subdivision_depth {
            return Err(CurveError::new(
                "curve.offset.subdivision_limit",
                "cubic offset cannot meet the requested tolerance within the subdivision limit",
            ));
        }
        let local_middle = (local_start + local_end) * 0.5;
        let source_middle = (source_start + source_end) * 0.5;
        if local_middle == local_start || local_middle == local_end {
            return Err(CurveError::new(
                "curve.offset.subdivision_limit",
                "cubic offset cannot meet the requested tolerance within the subdivision limit",
            ));
        }
        pending.push((
            depth + 1,
            local_middle,
            local_end,
            source_middle,
            source_end,
        ));
        pending.push((
            depth + 1,
            local_start,
            local_middle,
            source_start,
            source_middle,
        ));
    }
    Ok(pieces)
}

/// Evaluates one exact finite point on a cubic's signed normal-offset locus.
///
/// # Errors
///
/// Returns source parameter, tangent, or finite-coordinate diagnostics without fitting geometry.
fn exact_cubic_offset_point(
    cubic: crate::CubicBezierSegment,
    distance: f64,
    parameter: f64,
) -> Result<Point2, CurveError> {
    let segment = CurveSegment::CubicBezier(cubic);
    let point = segment.point_at(parameter)?;
    let normal = offset_limiting_unit_normal(segment, parameter)?;
    let offset = Point2::new(point.x + normal.x * distance, point.y + normal.y * distance);
    if !offset.is_finite() {
        return Err(CurveError::new(
            "curve.offset.numeric_overflow",
            "cubic offset point overflowed",
        ));
    }
    Ok(offset)
}

/// Measures one parameter interval of an exact offset against a direct line candidate.
///
/// # Errors
///
/// Returns source tangent or finite-coordinate diagnostics without reparameterizing the source
/// cubic into numerically tiny construction coordinates.
fn cubic_offset_interval_line_error(
    cubic: crate::CubicBezierSegment,
    start: Point2,
    end: Point2,
    distance: f64,
    parameter_start: f64,
    parameter_end: f64,
) -> Result<f64, CurveError> {
    let mut greatest = 0.0_f64;
    for local_parameter in [0.25, 0.5, 0.75] {
        let parameter = parameter_start + (parameter_end - parameter_start) * local_parameter;
        let expected = exact_cubic_offset_point(cubic, distance, parameter)?;
        let actual = Point2::new(
            start.x + (end.x - start.x) * local_parameter,
            start.y + (end.y - start.y) * local_parameter,
        );
        greatest = greatest.max((expected.x - actual.x).hypot(expected.y - actual.y));
    }
    Ok(greatest)
}

/// Checks one candidate against exact offset tangents at published run boundaries.
///
/// Internal subdivision boundaries may use positional tolerance alone. The first and last point
/// of a retained run remain artist-visible vector endpoints, so a positional line or interpolated
/// cubic fallback is accepted there only when its traversal agrees with the exact offset
/// derivative. A stationary exact boundary has no direction to preserve and therefore does not
/// reject an otherwise valid candidate.
///
/// # Errors
///
/// Returns exact offset-derivative, candidate-tangent, or finite-coordinate diagnostics without
/// accepting a reversed endpoint tangent.
fn candidate_preserves_offset_boundary_tangents(
    cubic: crate::CubicBezierSegment,
    distance: f64,
    parameter_start: f64,
    parameter_end: f64,
    candidate: CurveSegment,
) -> Result<bool, CurveError> {
    for (parameter, candidate_parameter) in [(parameter_start, 0.0), (parameter_end, 1.0)] {
        if parameter != 0.0 && parameter != 1.0 {
            continue;
        }
        let derivative = offset_cubic_derivative_at(cubic, distance, parameter)?;
        let derivative_length = derivative.x.hypot(derivative.y);
        if derivative_length == 0.0 {
            continue;
        }
        let candidate_tangent = match candidate.unit_tangent_at(candidate_parameter) {
            Ok(tangent) => tangent,
            Err(error) if error.path() == "curve.path.tangent.stationary" => return Ok(false),
            Err(error) => return Err(error),
        };
        let agreement = (candidate_tangent.x * derivative.x + candidate_tangent.y * derivative.y)
            / derivative_length;
        if !agreement.is_finite() || agreement <= 0.999 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Constructs one cubic Hermite offset fit from exact offset endpoints and endpoint tangents.
///
/// # Errors
///
/// Returns exact tangent and finite-coordinate diagnostics without synthesizing a fallback normal.
#[cfg(test)]
fn fit_offset_cubic(
    cubic: crate::CubicBezierSegment,
    distance: f64,
) -> Result<crate::CubicBezierSegment, CurveError> {
    fit_offset_cubic_interval(cubic, distance, 0.0, 1.0)
}

/// Constructs one cubic Hermite fit for a bounded source-parameter interval.
///
/// Exact positions and derivatives are evaluated on the original retained cubic and then scaled
/// into interval-local parameter space. This avoids derivative underflow from repeatedly splitting
/// large-coordinate cubics while retaining deterministic dyadic subdivision.
///
/// # Errors
///
/// Returns exact tangent and finite-coordinate diagnostics without synthesizing fallback geometry.
fn fit_offset_cubic_interval(
    cubic: crate::CubicBezierSegment,
    distance: f64,
    parameter_start: f64,
    parameter_end: f64,
) -> Result<crate::CubicBezierSegment, CurveError> {
    let start = exact_cubic_offset_point(cubic, distance, parameter_start)?;
    let end = exact_cubic_offset_point(cubic, distance, parameter_end)?;
    let parameter_span = parameter_end - parameter_start;
    let start_derivative =
        offset_cubic_derivative_at(cubic, distance, parameter_start)?.scale(parameter_span);
    let end_derivative =
        offset_cubic_derivative_at(cubic, distance, parameter_end)?.scale(parameter_span);
    crate::CubicBezierSegment::new(
        start,
        Point2::new(
            start.x + start_derivative.x / 3.0,
            start.y + start_derivative.y / 3.0,
        ),
        Point2::new(
            end.x - end_derivative.x / 3.0,
            end.y - end_derivative.y / 3.0,
        ),
        end,
    )
}

/// Fits one exact offset interval through its one-third and two-third positions.
///
/// This bounded fallback handles cusp-adjacent intervals whose parameter derivative becomes
/// singular even though the offset locus remains finite. The resulting cubic is still checked
/// against independent exact samples before publication; it is not a polyline, global resample,
/// or unverified corner cut.
///
/// # Errors
///
/// Returns exact source-normal or finite cubic-construction diagnostics atomically.
fn fit_offset_cubic_interpolating_interval(
    cubic: crate::CubicBezierSegment,
    distance: f64,
    parameter_start: f64,
    parameter_end: f64,
) -> Result<crate::CubicBezierSegment, CurveError> {
    let span = parameter_end - parameter_start;
    let start = exact_cubic_offset_point(cubic, distance, parameter_start)?;
    let one_third = exact_cubic_offset_point(cubic, distance, parameter_start + span / 3.0)?;
    let two_thirds =
        exact_cubic_offset_point(cubic, distance, parameter_start + span * (2.0 / 3.0))?;
    let end = exact_cubic_offset_point(cubic, distance, parameter_end)?;
    let first_right = Point2::new(
        27.0 * one_third.x - 8.0 * start.x - end.x,
        27.0 * one_third.y - 8.0 * start.y - end.y,
    );
    let second_right = Point2::new(
        27.0 * two_thirds.x - start.x - 8.0 * end.x,
        27.0 * two_thirds.y - start.y - 8.0 * end.y,
    );
    crate::CubicBezierSegment::new(
        start,
        Point2::new(
            (2.0 * first_right.x - second_right.x) / 18.0,
            (2.0 * first_right.y - second_right.y) / 18.0,
        ),
        Point2::new(
            (2.0 * second_right.x - first_right.x) / 18.0,
            (2.0 * second_right.y - first_right.y) / 18.0,
        ),
        end,
    )
}

/// Returns the analytic parameter derivative of one signed cubic offset curve.
///
/// The derivative combines the source velocity with the derivative of its unit left normal, so
/// fitted controls preserve direction and magnitude instead of depending on a chord heuristic.
///
/// # Errors
///
/// Returns a stable finite-arithmetic diagnostic without normalizing invalid data. A stationary
/// split endpoint keeps a zero parameter derivative; its one-sided offset position is resolved by
/// the caller and adaptive subdivision proves the fitted locus on that side.
fn offset_cubic_derivative_at(
    cubic: crate::CubicBezierSegment,
    distance: f64,
    parameter: f64,
) -> Result<Vector2, CurveError> {
    let segment = CurveSegment::CubicBezier(cubic);
    let velocity = segment.derivative_at(parameter)?;
    let inverse = 1.0 - parameter;
    let acceleration_start = Vector2::new(
        6.0 * (cubic.control_2().x - 2.0 * cubic.control_1().x + cubic.start().x),
        6.0 * (cubic.control_2().y - 2.0 * cubic.control_1().y + cubic.start().y),
    );
    let acceleration_end = Vector2::new(
        6.0 * (cubic.end().x - 2.0 * cubic.control_2().x + cubic.control_1().x),
        6.0 * (cubic.end().y - 2.0 * cubic.control_2().y + cubic.control_1().y),
    );
    let acceleration = Vector2::new(
        inverse * acceleration_start.x + parameter * acceleration_end.x,
        inverse * acceleration_start.y + parameter * acceleration_end.y,
    );
    let speed = velocity.x.hypot(velocity.y);
    if !speed.is_finite() {
        return Err(CurveError::new(
            "curve.offset.numeric_overflow",
            "cubic offset derivative overflowed",
        ));
    }
    if speed == 0.0 {
        return Ok(Vector2::new(0.0, 0.0));
    }
    let tangent = Vector2::new(velocity.x / speed, velocity.y / speed);
    let tangential_acceleration = tangent.x * acceleration.x + tangent.y * acceleration.y;
    let tangent_derivative = Vector2::new(
        (acceleration.x - tangent.x * tangential_acceleration) / speed,
        (acceleration.y - tangent.y * tangential_acceleration) / speed,
    );
    let normal_derivative = Vector2::new(-tangent_derivative.y, tangent_derivative.x);
    let derivative = Vector2::new(
        velocity.x + distance * normal_derivative.x,
        velocity.y + distance * normal_derivative.y,
    );
    if !derivative.x.is_finite() || !derivative.y.is_finite() {
        return Err(CurveError::new(
            "curve.offset.numeric_overflow",
            "cubic offset derivative overflowed",
        ));
    }
    Ok(derivative)
}

/// Measures a fitted cubic against exact normal-offset positions at fixed dyadic interior samples.
///
/// # Errors
///
/// Returns exact source tangent or finite-coordinate diagnostics rather than accepting an unverifiable fit.
#[cfg(test)]
fn cubic_offset_error(
    cubic: crate::CubicBezierSegment,
    fitted: crate::CubicBezierSegment,
    distance: f64,
) -> Result<f64, CurveError> {
    cubic_offset_interval_error(cubic, fitted, distance, 0.0, 1.0)
}

/// Measures a fitted cubic against one exact source-parameter interval.
///
/// # Errors
///
/// Returns exact source tangent or finite-coordinate diagnostics without rescaling the source
/// construction points during recursive fitting.
fn cubic_offset_interval_error(
    cubic: crate::CubicBezierSegment,
    fitted: crate::CubicBezierSegment,
    distance: f64,
    parameter_start: f64,
    parameter_end: f64,
) -> Result<f64, CurveError> {
    let fit = CurveSegment::CubicBezier(fitted);
    let mut greatest = 0.0_f64;
    for local_parameter in [0.25, 0.5, 0.75] {
        let parameter = parameter_start + (parameter_end - parameter_start) * local_parameter;
        let expected = exact_cubic_offset_point(cubic, distance, parameter)?;
        let actual = fit.point_at(local_parameter)?;
        greatest = greatest.max((expected.x - actual.x).hypot(expected.y - actual.y));
    }
    Ok(greatest)
}

/// Returns an analytic tangent or the first nonstationary one-sided cubic endpoint limit.
///
/// Exact intersection splitting can legitimately place a cubic cusp at a propagated wavefront
/// node. The ordinary curve API must continue to report that authored derivative as stationary,
/// while offset growth needs the incoming or outgoing limiting direction on its own side of the
/// node. Interior stationary parameters remain errors and are still isolated by cusp cleanup.
///
/// # Errors
///
/// Returns the original tangent diagnostic unless a finite nonzero cubic endpoint limit exists.
fn offset_limiting_unit_tangent(
    segment: CurveSegment,
    parameter: f64,
) -> Result<Vector2, CurveError> {
    segment.limiting_unit_tangent_at(parameter)
}

/// Returns the left unit normal from the offset-specific one-sided tangent policy.
///
/// # Errors
///
/// Propagates the limiting-tangent diagnostic without inventing an interior cusp direction.
fn offset_limiting_unit_normal(
    segment: CurveSegment,
    parameter: f64,
) -> Result<Vector2, CurveError> {
    Ok(offset_limiting_unit_tangent(segment, parameter)?.perpendicular())
}

/// Replaces one derived segment start exactly so numerical join noise never creates a stationary bridge.
///
/// # Errors
///
/// Returns finite-coordinate errors without changing the segment's terminal construction point.
fn replace_segment_start(segment: CurveSegment, start: Point2) -> Result<CurveSegment, CurveError> {
    match segment {
        CurveSegment::Line(line) => Ok(CurveSegment::Line(LineSegment::new(start, line.end())?)),
        CurveSegment::CubicBezier(cubic) => {
            Ok(CurveSegment::CubicBezier(crate::CubicBezierSegment::new(
                start,
                cubic.control_1(),
                cubic.control_2(),
                cubic.end(),
            )?))
        }
    }
}

/// Moves one derived start and its adjacent cubic control together to preserve join tangency.
///
/// # Errors
///
/// Returns finite line/cubic construction diagnostics without changing the segment end.
fn replace_segment_start_preserving_tangent(
    segment: CurveSegment,
    start: Point2,
) -> Result<CurveSegment, CurveError> {
    match segment {
        CurveSegment::Line(line) => Ok(CurveSegment::Line(LineSegment::new(start, line.end())?)),
        CurveSegment::CubicBezier(cubic) => {
            let delta = Vector2::new(start.x - cubic.start().x, start.y - cubic.start().y);
            Ok(CurveSegment::CubicBezier(crate::CubicBezierSegment::new(
                start,
                Point2::new(cubic.control_1().x + delta.x, cubic.control_1().y + delta.y),
                cubic.control_2(),
                cubic.end(),
            )?))
        }
    }
}

/// Returns the shared cancellation diagnostic before a partial offset can escape.
fn cancelled() -> CurveError {
    CurveError::new("evaluation.cancelled", "evaluation was cancelled")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the private RegionRound contract splits one canonical 120-degree convex outward
    /// corner into two tangent-continuous positive CCW cubic arcs with bounded radial fitting
    /// error and never represents that corner with a straight bevel segment.
    ///
    /// # Panics
    ///
    /// Panics only when this fixed finite unit-radius fixture cannot construct or evaluate a
    /// cubic, which would violate the geometry primitive's finite-coordinate contract.
    #[test]
    fn stage20q_region_round_join_splits_120_degree_corner_without_bevel() {
        let center = Point2::new(0.0, 0.0);
        let start = Point2::new(1.0, 0.0);
        let end = Point2::new(-0.5, 3.0_f64.sqrt() * 0.5);
        let arcs = region_round_join_segments(center, start, end, true)
            .expect("finite 120-degree circular corner splits");
        assert_eq!(arcs.len(), 2, "120 degrees divides into two 60-degree arcs");
        assert_eq!(arcs[0].start(), start);
        assert_eq!(arcs[0].end(), arcs[1].start());
        assert_eq!(arcs[1].end(), end);
        let ccw_sweep = |from: Point2, to: Point2| {
            let mut sweep = to.y.atan2(to.x) - from.y.atan2(from.x);
            if sweep < 0.0 {
                sweep += std::f64::consts::TAU;
            }
            sweep
        };
        let sweeps: Vec<_> = arcs
            .iter()
            .map(|arc| ccw_sweep(arc.start(), arc.end()))
            .collect();
        assert!(
            sweeps
                .iter()
                .all(|sweep| { *sweep > 0.0 && *sweep <= std::f64::consts::FRAC_PI_2 + 1.0e-12 })
        );
        assert!((sweeps.iter().sum::<f64>() - 2.0 * std::f64::consts::FRAC_PI_3).abs() < 1.0e-12);
        for arc in &arcs {
            for parameter in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let point = CurveSegment::CubicBezier(*arc)
                    .point_at(parameter)
                    .expect("finite cubic sample");
                assert!(
                    (point.x.hypot(point.y) - 1.0).abs() <= 1.0 / 64.0,
                    "cubic radial fit stays within the approved tolerance"
                );
            }
        }
        let start_tangent = CurveSegment::CubicBezier(arcs[0])
            .unit_tangent_at(0.0)
            .expect("finite first arc tangent");
        let join_left = CurveSegment::CubicBezier(arcs[0])
            .unit_tangent_at(1.0)
            .expect("finite left join tangent");
        let join_right = CurveSegment::CubicBezier(arcs[1])
            .unit_tangent_at(0.0)
            .expect("finite right join tangent");
        let end_tangent = CurveSegment::CubicBezier(arcs[1])
            .unit_tangent_at(1.0)
            .expect("finite final arc tangent");
        assert!(start_tangent.y > 0.999_999 && end_tangent.x < -0.8);
        assert!(join_left.x * join_right.x + join_left.y * join_right.y > 0.999_999);
    }

    /// Verifies one mutable work budget accumulates path work while the legacy wrapper replays bytes.
    #[test]
    fn stage20q_shared_offset_work_accumulates_without_reset() {
        let path = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)).unwrap();
        let limits = PathOffsetLimits {
            maximum_segments: 2,
            maximum_components: 1,
            ..PathOffsetLimits::default()
        };
        let request = PathOffsetRequest {
            path: &path,
            signed_distance: 1.0,
            endpoint_policy: PathOffsetEndpointPolicy::Preserve,
            cleanup: PathOffsetCleanup::DissolveCrossings,
            crossing_barriers: &[],
            limits,
        };
        let legacy = offset_path_cancellable(request, &|| false).unwrap();
        let mut work = PathOffsetWork::new(limits).unwrap();
        let first = offset_path_with_work_cancellable(request, &mut work, &|| false).unwrap();
        assert_eq!(first, legacy);
        let error = offset_path_with_work_cancellable(request, &mut work, &|| false).unwrap_err();
        assert_eq!(error.path(), "curve.offset.component_limit");
    }

    /// Proves signed line offsets retain compact direct centerlines and exact zero identity.
    #[test]
    fn signed_line_offsets_and_zero_identity_are_compact() {
        let path = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .expect("finite source line");
        let request = |signed_distance| PathOffsetRequest {
            path: &path,
            signed_distance,
            endpoint_policy: PathOffsetEndpointPolicy::Preserve,
            cleanup: PathOffsetCleanup::DissolveCrossings,
            crossing_barriers: &[],
            limits: PathOffsetLimits::default(),
        };
        let positive = offset_path_cancellable(request(3.0), &|| false).expect("positive offset");
        let PathOffsetResult::Paths(positive) = positive else {
            panic!("line remains finite")
        };
        assert_eq!(positive[0].path.start(), Point2::new(0.0, 3.0));
        let zero = offset_path_cancellable(request(0.0), &|| false).expect("zero identity");
        let PathOffsetResult::Paths(zero) = zero else {
            panic!("source remains finite")
        };
        assert_eq!(zero[0].path, path);
    }

    /// Proves tangential extension reaches the planning bounds before derived centerlines publish.
    #[test]
    fn open_paths_extend_to_the_generation_bounds() {
        let path = CurvePath::line(Point2::new(2.0, 5.0), Point2::new(8.0, 5.0))
            .expect("finite source line");
        let result = offset_path_cancellable(
            PathOffsetRequest {
                path: &path,
                signed_distance: 1.0,
                endpoint_policy: PathOffsetEndpointPolicy::TangentialExtension {
                    bounds: Bounds::new(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
                        .expect("finite planning bounds"),
                },
                cleanup: PathOffsetCleanup::DissolveCrossings,
                crossing_barriers: &[],
                limits: PathOffsetLimits::default(),
            },
            &|| false,
        )
        .expect("extension remains finite");
        let PathOffsetResult::Paths(paths) = result else {
            panic!("line remains finite")
        };
        assert_eq!(paths[0].path.start(), Point2::new(0.0, 6.0));
        assert_eq!(paths[0].path.end(), Point2::new(10.0, 6.0));
        assert_eq!(paths[0].source_start, PathLocation::new(0, 0.0).unwrap());
        assert_eq!(paths[0].source_end, PathLocation::new(0, 1.0).unwrap());
    }

    /// Proves analytic line corners use a compact outer round join and an inner tangent intersection.
    #[test]
    fn line_corner_joins_are_round_outside_and_tangent_inside() {
        let path = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(8.0, 0.0),
                Point2::new(8.0, 8.0),
            ],
            PathClosure::Open,
        )
        .expect("finite corner path");
        let request = |signed_distance| PathOffsetRequest {
            path: &path,
            signed_distance,
            endpoint_policy: PathOffsetEndpointPolicy::Preserve,
            cleanup: PathOffsetCleanup::DissolveCrossings,
            crossing_barriers: &[],
            limits: PathOffsetLimits::default(),
        };
        let PathOffsetResult::Paths(outer) =
            offset_path_cancellable(request(-1.0), &|| false).expect("outer offset")
        else {
            panic!("outer corner remains finite")
        };
        assert!(
            outer[0]
                .path
                .segments()
                .iter()
                .any(|segment| matches!(segment, CurveSegment::CubicBezier(_)))
        );
        let PathOffsetResult::Paths(inner) =
            offset_path_cancellable(request(1.0), &|| false).expect("inner offset")
        else {
            panic!("inner corner remains finite")
        };
        assert!(
            inner[0]
                .path
                .segments()
                .iter()
                .all(|segment| matches!(segment, CurveSegment::Line(_)))
        );
    }

    /// Proves cubic input becomes bounded direct centerline geometry with finite signed offsets.
    #[test]
    fn cubic_offsets_remain_compact_and_finite() {
        let path = CurvePath::new(
            vec![CurveSegment::CubicBezier(
                crate::CubicBezierSegment::new(
                    Point2::new(0.0, 0.0),
                    Point2::new(10.0, 20.0),
                    Point2::new(20.0, -20.0),
                    Point2::new(30.0, 0.0),
                )
                .expect("finite cubic"),
            )],
            PathClosure::Open,
        )
        .expect("connected cubic path");
        let result = offset_path_cancellable(
            PathOffsetRequest {
                path: &path,
                signed_distance: -4.0,
                endpoint_policy: PathOffsetEndpointPolicy::Preserve,
                cleanup: PathOffsetCleanup::DissolveCrossings,
                crossing_barriers: &[],
                limits: PathOffsetLimits::default(),
            },
            &|| false,
        )
        .expect("finite cubic offset");
        let PathOffsetResult::Paths(paths) = result else {
            panic!("moving cubic remains finite")
        };
        assert!(paths[0].path.segments().len() > 1);
        assert!(paths[0].path.segments().len() <= 64);
        assert!(
            paths[0]
                .path
                .segments()
                .iter()
                .all(|segment| matches!(segment, CurveSegment::CubicBezier(_)))
        );
        assert!(
            paths[0]
                .path
                .bounds()
                .expect("offset bounds")
                .min
                .is_finite()
        );
    }

    /// Builds the compact Stage 20J diagnostic cubic or its vertical mirror.
    fn diagnostic_cusp_path(mirrored: bool) -> CurvePath {
        let mirror = |y: f64| if mirrored { -y } else { y };
        CurvePath::new(
            vec![CurveSegment::CubicBezier(
                crate::CubicBezierSegment::new(
                    Point2::new(20.0, mirror(160.0)),
                    Point2::new(96.0, mirror(64.0)),
                    Point2::new(224.0, mirror(64.0)),
                    Point2::new(300.0, mirror(160.0)),
                )
                .expect("finite diagnostic cubic"),
            )],
            PathClosure::Open,
        )
        .expect("connected diagnostic cubic")
    }

    /// Proves the diagnostic cubic stays whole below its exact curvature-radius threshold.
    #[test]
    fn cubic_below_cusp_threshold_remains_one_source_consistent_component() {
        let path = diagnostic_cusp_path(false);
        let PathOffsetResult::Paths(components) = offset_path_cancellable(
            PathOffsetRequest {
                path: &path,
                signed_distance: 162.5,
                endpoint_policy: PathOffsetEndpointPolicy::Preserve,
                cleanup: PathOffsetCleanup::DissolveCrossings,
                crossing_barriers: &[],
                limits: PathOffsetLimits::default(),
            },
            &|| false,
        )
        .expect("sub-threshold offset remains regular") else {
            panic!("sub-threshold diagnostic cubic survives")
        };
        assert_eq!(components.len(), 1);
        assert_eq!(
            components[0].source_start,
            PathLocation::new(0, 0.0).unwrap()
        );
        assert_eq!(components[0].source_end, PathLocation::new(0, 1.0).unwrap());
    }

    /// Proves cusp-range offsets omit the reversed middle and publish regular ordered branches.
    #[test]
    fn cubic_cusps_split_and_reversal_intervals_disappear_deterministically() {
        for distance in [162.5625, 162.562_6, 168.0, 216.0] {
            let path = diagnostic_cusp_path(false);
            let PathOffsetResult::Paths(first) = offset_path_cancellable(
                PathOffsetRequest {
                    path: &path,
                    signed_distance: distance,
                    endpoint_policy: PathOffsetEndpointPolicy::Preserve,
                    cleanup: PathOffsetCleanup::DissolveCrossings,
                    crossing_barriers: &[],
                    limits: PathOffsetLimits::default(),
                },
                &|| false,
            )
            .unwrap_or_else(|error| panic!("distance {distance}: {error}")) else {
                panic!("distance {distance} retains two regular branches")
            };
            let PathOffsetResult::Paths(second) = offset_path_cancellable(
                PathOffsetRequest {
                    path: &path,
                    signed_distance: distance,
                    endpoint_policy: PathOffsetEndpointPolicy::Preserve,
                    cleanup: PathOffsetCleanup::DissolveCrossings,
                    crossing_barriers: &[],
                    limits: PathOffsetLimits::default(),
                },
                &|| false,
            )
            .expect("repeated cusp isolation succeeds") else {
                panic!("repeated cusp isolation retains branches")
            };
            assert_eq!(first, second, "distance {distance} is deterministic");
            assert_eq!(first.len(), 2, "distance {distance}");
            assert_eq!(first[0].component_ordinal, 0);
            assert_eq!(first[1].component_ordinal, 1);
            assert_eq!(first[0].source_start, PathLocation::new(0, 0.0).unwrap());
            assert_eq!(first[1].source_end, PathLocation::new(0, 1.0).unwrap());
            assert!(first.iter().all(|component| {
                source_location_key(component.source_start)
                    < source_location_key(component.source_end)
            }));
            assert!(first[0].source_end.parameter() < 0.5, "distance {distance}");
            assert!(
                first[1].source_start.parameter() > 0.5,
                "distance {distance}"
            );
            assert!(
                first[0].source_end.parameter() < first[1].source_start.parameter(),
                "distance {distance} retains a real source gap"
            );
            let branch_intersections = first[0]
                .path
                .intersections(&first[1].path)
                .expect("surviving cusp branches have bounded intersections");
            assert!(
                branch_intersections
                    .iter()
                    .all(|intersection| intersection.kind() != crate::IntersectionKind::Crossing),
                "distance {distance} must not publish cross-hatched surviving branches: {branch_intersections:?}"
            );
            for component in &first {
                for (source_location, output_location) in [
                    (
                        component.source_start,
                        PathLocation::new(0, 0.0).expect("component start location"),
                    ),
                    (
                        component.source_end,
                        PathLocation::new(component.path.segments().len() - 1, 1.0)
                            .expect("component end location"),
                    ),
                ] {
                    let source_tangent = path
                        .unit_tangent_at(source_location)
                        .expect("retained source endpoint is nonstationary");
                    let output_tangent = component
                        .path
                        .unit_tangent_at(output_location)
                        .expect("published offset endpoint is nonstationary");
                    let tangent_agreement =
                        source_tangent.x * output_tangent.x + source_tangent.y * output_tangent.y;
                    assert!(
                        tangent_agreement > 0.999,
                        "distance {distance} retains authored traversal at source {source_location:?} and output {output_location:?}: source {source_tangent:?}, output {output_tangent:?}, agreement {tangent_agreement}, segments {:?}",
                        component.path.segments()
                    );
                }
            }
        }
    }

    /// Proves offsets beyond the endpoint curvature radius discard extension-only remnants.
    #[test]
    fn cubic_wholly_reversed_offsets_collapse_without_endpoint_extensions() {
        for (path, sign) in [
            (diagnostic_cusp_path(false), 1.0),
            (diagnostic_cusp_path(true), -1.0),
        ] {
            let bounds = if sign > 0.0 {
                Bounds::new(Point2::new(-17.5, -17.5), Point2::new(337.5, 337.5))
            } else {
                Bounds::new(Point2::new(-17.5, -337.5), Point2::new(337.5, 17.5))
            }
            .expect("finite diagnostic bounds");
            for distance in [228.0, 240.0, 252.0] {
                let result = offset_path_cancellable(
                    PathOffsetRequest {
                        path: &path,
                        signed_distance: sign * distance,
                        endpoint_policy: PathOffsetEndpointPolicy::TangentialExtension { bounds },
                        cleanup: PathOffsetCleanup::DissolveCrossings,
                        crossing_barriers: &[],
                        limits: PathOffsetLimits::default(),
                    },
                    &|| false,
                )
                .unwrap_or_else(|error| panic!("distance {distance}: {error}"));
                assert_eq!(result, PathOffsetResult::Collapsed, "distance {distance}");
            }
        }
    }

    /// Proves crossing both terminal extensions collapses the exhausted extended envelope.
    #[test]
    fn terminal_endpoint_extension_crossing_collapses_without_floating_fragments() {
        for (path, signed_distance, bounds) in [
            (
                diagnostic_cusp_path(false),
                180.0_f64,
                Bounds::new(Point2::new(-17.5, -17.5), Point2::new(337.5, 337.5))
                    .expect("finite diagnostic bounds"),
            ),
            (
                diagnostic_cusp_path(true),
                -180.0_f64,
                Bounds::new(Point2::new(-17.5, -337.5), Point2::new(337.5, 17.5))
                    .expect("finite mirrored diagnostic bounds"),
            ),
        ] {
            let retained = offset_path_cancellable(
                PathOffsetRequest {
                    path: &path,
                    signed_distance: signed_distance.signum() * 168.0,
                    endpoint_policy: PathOffsetEndpointPolicy::TangentialExtension { bounds },
                    cleanup: PathOffsetCleanup::DissolveCrossings,
                    crossing_barriers: &[],
                    limits: PathOffsetLimits::default(),
                },
                &|| false,
            )
            .expect("pre-crossing terminal extensions remain finite");
            let PathOffsetResult::Paths(retained) = retained else {
                panic!("pre-crossing terminal extensions retain cusp branches")
            };
            assert_eq!(retained.len(), 2);
            let result = offset_path_cancellable(
                PathOffsetRequest {
                    path: &path,
                    signed_distance,
                    endpoint_policy: PathOffsetEndpointPolicy::TangentialExtension { bounds },
                    cleanup: PathOffsetCleanup::DissolveCrossings,
                    crossing_barriers: &[],
                    limits: PathOffsetLimits::default(),
                },
                &|| false,
            )
            .expect("terminal extension crossing collapses deterministically");
            assert_eq!(result, PathOffsetResult::Collapsed);
        }
    }

    /// Proves mirroring the diagnostic curve and signed distance preserves cusp splitting on the other side.
    #[test]
    fn mirrored_cubic_and_signed_distance_split_the_same_source_intervals() {
        let original = diagnostic_cusp_path(false);
        let mirrored = diagnostic_cusp_path(true);
        let offset = |path: &CurvePath, signed_distance| {
            let PathOffsetResult::Paths(components) = offset_path_cancellable(
                PathOffsetRequest {
                    path,
                    signed_distance,
                    endpoint_policy: PathOffsetEndpointPolicy::Preserve,
                    cleanup: PathOffsetCleanup::DissolveCrossings,
                    crossing_barriers: &[],
                    limits: PathOffsetLimits::default(),
                },
                &|| false,
            )
            .expect("mirrored cusp isolation succeeds") else {
                panic!("mirrored cusp branches survive")
            };
            components
        };
        let original_components = offset(&original, 216.0);
        let mirrored_components = offset(&mirrored, -216.0);
        assert_eq!(original_components.len(), 2);
        assert_eq!(mirrored_components.len(), 2);
        assert!(
            original_components
                .iter()
                .zip(&mirrored_components)
                .all(|(first, second)| {
                    first.source_start == second.source_start
                        && first.source_end == second.source_end
                        && (first.path.start().x - second.path.start().x).abs() < 1.0e-9
                        && (first.path.start().y + second.path.start().y).abs() < 1.0e-9
                        && (first.path.end().x - second.path.end().x).abs() < 1.0e-9
                        && (first.path.end().y + second.path.end().y).abs() < 1.0e-9
                })
        );
    }

    /// Proves cusp work and depth limits report distinct stable failures before publishing a branch.
    #[test]
    fn cusp_isolation_work_and_depth_limits_fail_atomically() {
        let path = diagnostic_cusp_path(false);
        let request = |limits| PathOffsetRequest {
            path: &path,
            signed_distance: 240.0,
            endpoint_policy: PathOffsetEndpointPolicy::Preserve,
            cleanup: PathOffsetCleanup::DissolveCrossings,
            crossing_barriers: &[],
            limits,
        };
        let work = offset_path_cancellable(
            request(PathOffsetLimits {
                maximum_cusp_isolation_work: 1,
                ..PathOffsetLimits::default()
            }),
            &|| false,
        )
        .expect_err("cusp work exhaustion publishes no component");
        assert_eq!(work.path(), "curve.offset.cusp_limit");
        let depth = offset_path_cancellable(
            request(PathOffsetLimits {
                maximum_subdivision_depth: 1,
                ..PathOffsetLimits::default()
            }),
            &|| false,
        )
        .expect_err("cusp depth exhaustion publishes no component");
        assert_eq!(depth.path(), "curve.offset.subdivision_limit");
        assert_eq!(MAX_PATH_OFFSET_CUSP_ISOLATION_WORK, 262_144);
        assert_eq!(
            PATH_OFFSET_ALGORITHM_CONTRACT_ID,
            "toniator.path-offset.v6.solved-crossing-nodes"
        );
    }

    /// Proves adaptive cubic pieces retain their requested normal-distance tolerance and reject exhausted depth.
    #[test]
    fn cubic_offset_tolerance_and_subdivision_exhaustion_are_explicit() {
        let cubic = crate::CubicBezierSegment::new(
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 24.0),
            Point2::new(20.0, -24.0),
            Point2::new(30.0, 0.0),
        )
        .expect("finite cubic");
        let gentle = crate::CubicBezierSegment::new(
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.01),
            Point2::new(20.0, -0.01),
            Point2::new(30.0, 0.0),
        )
        .expect("finite gentle cubic");
        let fitted = fit_offset_cubic(gentle, 3.0).expect("finite fit");
        assert!(
            cubic_offset_error(gentle, fitted, 3.0).expect("finite error")
                <= DEFAULT_PATH_OFFSET_TOLERANCE
        );
        let error = adaptive_cubic_offsets(
            cubic,
            0,
            0.0,
            1.0,
            OffsetOrientation::Retained,
            3.0,
            PathOffsetLimits {
                maximum_subdivision_depth: 1,
                tolerance: 1.0e-12,
                ..PathOffsetLimits::default()
            },
            false,
            &|| false,
        )
        .expect_err("insufficient depth rejects rather than loosening tolerance");
        assert_eq!(error.path(), "curve.offset.subdivision_limit");
    }

    /// Proves Hermite-fitted cubic offsets retain the exact source endpoint tangent directions.
    #[test]
    fn cubic_offset_fit_preserves_endpoint_tangent_directions() {
        let cubic = crate::CubicBezierSegment::new(
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 20.0),
            Point2::new(20.0, -8.0),
            Point2::new(30.0, 4.0),
        )
        .expect("finite cubic");
        let fitted = fit_offset_cubic(cubic, 2.0).expect("finite fit");
        let source = CurveSegment::CubicBezier(cubic);
        let offset = CurveSegment::CubicBezier(fitted);
        for parameter in [0.0, 1.0] {
            let expected = source.unit_tangent_at(parameter).expect("source tangent");
            let actual = offset.unit_tangent_at(parameter).expect("fitted tangent");
            assert!(expected.x * actual.x + expected.y * actual.y > 0.999_999);
        }
    }

    /// Proves stationary paths collapse while cancellation and explicit segment limits publish nothing.
    #[test]
    fn stationary_cancellation_and_limits_fail_atomically() {
        let stationary = CurvePath::line(Point2::new(1.0, 1.0), Point2::new(1.0, 1.0))
            .expect("finite stationary line");
        assert!(matches!(
            offset_path_cancellable(
                PathOffsetRequest {
                    path: &stationary,
                    signed_distance: 2.0,
                    endpoint_policy: PathOffsetEndpointPolicy::Preserve,
                    cleanup: PathOffsetCleanup::DissolveCrossings,
                    crossing_barriers: &[],
                    limits: PathOffsetLimits::default(),
                },
                &|| false,
            )
            .expect("stationary source is a collapsed result"),
            PathOffsetResult::Collapsed
        ));
        let line = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(2.0, 0.0))
            .expect("finite moving line");
        let request = PathOffsetRequest {
            path: &line,
            signed_distance: 1.0,
            endpoint_policy: PathOffsetEndpointPolicy::Preserve,
            cleanup: PathOffsetCleanup::DissolveCrossings,
            crossing_barriers: &[],
            limits: PathOffsetLimits {
                maximum_subdivision_depth: 1,
                maximum_segments: 1,
                maximum_components: 1,
                ..PathOffsetLimits::default()
            },
        };
        assert_eq!(
            offset_path_cancellable(request, &|| true)
                .expect_err("cancelled request publishes no path")
                .path(),
            "evaluation.cancelled"
        );
        let bounded = PathOffsetRequest {
            limits: PathOffsetLimits {
                maximum_segments: 0,
                ..PathOffsetLimits::default()
            },
            ..request
        };
        assert_eq!(
            offset_path_cancellable(bounded, &|| false)
                .expect_err("zero segment budget rejects before publication")
                .path(),
            "curve.offset.limit"
        );
    }

    /// Converts one test path into exact per-segment source spans for cleanup-only witnesses.
    fn traced(path: &CurvePath) -> Vec<TracedOffsetSegment> {
        path.segments()
            .iter()
            .copied()
            .enumerate()
            .map(|(index, segment)| TracedOffsetSegment {
                segment,
                source_start: PathLocation::new(index, 0.0).expect("valid source start"),
                source_end: PathLocation::new(index, 1.0).expect("valid source end"),
                orientation: Some(OffsetOrientation::Retained),
            })
            .collect()
    }

    /// Proves open transverse crossings split around the removed loop with exact ordered source intervals.
    #[test]
    fn crossing_cleanup_splits_ordered_side_consistent_components() {
        let path = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 4.0),
                Point2::new(0.0, 4.0),
                Point2::new(4.0, 0.0),
            ],
            PathClosure::Open,
        )
        .expect("finite crossing-shaped polyline");
        let paths = dissolve_crossings(
            traced(&path),
            PathClosure::Open,
            None,
            MAX_PATH_OFFSET_CLEANUP_PAIRS,
            MAX_PATH_OFFSET_COMPONENTS,
            DEFAULT_PATH_OFFSET_TOLERANCE,
            false,
            &|| false,
        )
        .expect("crossing cleanup remains bounded");
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].path.start(), Point2::new(0.0, 0.0));
        assert_eq!(paths[0].path.end(), Point2::new(2.0, 2.0));
        assert_eq!(paths[0].source_start, PathLocation::new(0, 0.0).unwrap());
        assert_eq!(paths[0].source_end, PathLocation::new(0, 0.5).unwrap());
        assert_eq!(paths[1].path.start(), Point2::new(2.0, 2.0));
        assert_eq!(paths[1].path.end(), Point2::new(4.0, 0.0));
        assert_eq!(paths[1].source_start, PathLocation::new(2, 0.5).unwrap());
        assert_eq!(paths[1].source_end, PathLocation::new(2, 1.0).unwrap());
        assert!(paths.iter().all(|component| {
            first_transverse_crossing(
                component.path.segments(),
                MAX_PATH_OFFSET_CLEANUP_PAIRS,
                &|| false,
            )
            .expect("cleaned component stays bounded")
            .is_none()
        }));
    }

    /// Proves Constant-gap cleanup relinks at one solved node without deleting the enclosed span.
    #[test]
    fn constant_gap_crossing_cleanup_preserves_every_source_span() {
        let path = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 4.0),
                Point2::new(0.0, 4.0),
                Point2::new(4.0, 0.0),
            ],
            PathClosure::Open,
        )
        .expect("finite crossing-shaped polyline");
        let paths = dissolve_crossings(
            traced(&path),
            PathClosure::Open,
            None,
            MAX_PATH_OFFSET_CLEANUP_PAIRS,
            MAX_PATH_OFFSET_COMPONENTS,
            DEFAULT_PATH_OFFSET_TOLERANCE,
            true,
            &|| false,
        )
        .expect("Constant-gap crossing relink remains bounded");
        assert_eq!(paths.len(), 2);
        let outer = paths
            .iter()
            .find(|component| component.path.closure() == PathClosure::Open)
            .expect("one relinked open frontier survives");
        let enclosed = paths
            .iter()
            .find(|component| component.path.closure() == PathClosure::Closed)
            .expect("the enclosed source interval survives as one closed component");
        assert_eq!(outer.path.start(), Point2::new(0.0, 0.0));
        assert_eq!(outer.path.end(), Point2::new(4.0, 0.0));
        assert_eq!(outer.path.segments().len(), 2);
        let shared = Point2::new(2.0, 2.0);
        assert_eq!(outer.path.segments()[0].end(), shared);
        assert_eq!(outer.path.segments()[1].start(), shared);
        assert_eq!(enclosed.path.start(), shared);
        assert_eq!(enclosed.path.end(), shared);
        let original_length = path
            .measure_arc_length()
            .expect("source length remains finite")
            .total_length();
        let retained_length = paths
            .iter()
            .map(|component| {
                component
                    .path
                    .measure_arc_length()
                    .expect("component length remains finite")
                    .total_length()
            })
            .sum::<f64>();
        assert!((retained_length - original_length).abs() <= 1.0e-12);
        assert!(paths.iter().all(|component| {
            first_transverse_crossing(
                component.path.segments(),
                MAX_PATH_OFFSET_CLEANUP_PAIRS,
                &|| false,
            )
            .expect("relinked component remains bounded")
            .is_none()
        }));
    }

    /// Proves a stationary-handle cubic with an exactly straight locus becomes canonical line work.
    #[test]
    fn constant_gap_endpoint_handle_canonicalization_stores_exact_linear_cubic_as_line() {
        let cubic = crate::CubicBezierSegment::new(
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 0.0),
        )
        .expect("finite exact linear cubic");
        let mut segments = vec![TracedOffsetSegment {
            segment: CurveSegment::CubicBezier(cubic),
            source_start: PathLocation::new(0, 0.0).expect("source start"),
            source_end: PathLocation::new(0, 1.0).expect("source end"),
            orientation: Some(OffsetOrientation::Retained),
        }];
        canonicalize_planar_endpoint_handles(&mut segments, DEFAULT_PATH_OFFSET_TOLERANCE)
            .expect("linear canonicalization remains finite");
        assert!(matches!(segments[0].segment, CurveSegment::Line(_)));
        assert_eq!(segments[0].segment.start(), cubic.start());
        assert_eq!(segments[0].segment.end(), cubic.end());
    }

    /// Proves overlap refinement cannot re-split an exact shared endpoint node as an interior fold.
    #[test]
    fn constant_gap_cleanup_ignores_overlap_already_owned_by_exact_shared_node() {
        let cubic = CurveSegment::CubicBezier(
            crate::CubicBezierSegment::new(
                Point2::new(0.0, 0.0),
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(4.0, 0.0),
            )
            .expect("finite exact linear cubic"),
        );
        let unrelated = CurveSegment::Line(
            LineSegment::new(Point2::new(10.0, 1.0), Point2::new(11.0, 1.0))
                .expect("finite unrelated line"),
        );
        let overlap = CurveSegment::Line(
            LineSegment::new(Point2::new(2.0, 0.0), Point2::new(4.0, 0.0))
                .expect("finite overlapping line"),
        );
        let crossing = first_transverse_crossing_with_budget(
            &[cubic, unrelated, overlap],
            &mut CleanupBudget {
                examined_pairs: 0,
                maximum_pairs: MAX_PATH_OFFSET_CLEANUP_PAIRS,
            },
            true,
            &|| false,
        )
        .expect("shared-node overlap cleanup remains bounded");
        assert!(crossing.is_none());
    }

    /// Proves both signed sides of an authored Constant-gap corner retain one vector node.
    ///
    /// # Panics
    ///
    /// Panics when either signed offset rounds the authored corner, inserts a third connector
    /// segment, or fails to share the exact adjacent-tangent intersection between both segments.
    #[test]
    fn constant_gap_authored_corner_uses_exact_vector_join_on_both_sides() {
        let source = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(4.0, 4.0),
            ],
            PathClosure::Open,
        )
        .expect("finite authored corner path");
        for (signed_distance, expected_node) in
            [(1.0, Point2::new(3.0, 1.0)), (-1.0, Point2::new(5.0, -1.0))]
        {
            let PathOffsetResult::Paths(paths) = advance_planar_constant_gap_frontier_cancellable(
                &source,
                &[],
                signed_distance,
                PathOffsetLimits::default(),
                &|| false,
            )
            .expect("authored vector join remains bounded") else {
                panic!("authored corner retains a nondegenerate offset");
            };
            let exterior = paths
                .iter()
                .find(|component| component.path.closure() == PathClosure::Open)
                .expect("authored corner publishes one exterior path");
            assert_eq!(exterior.path.segments().len(), 2);
            assert_eq!(exterior.path.segments()[0].end(), expected_node);
            assert_eq!(exterior.path.segments()[1].start(), expected_node);
        }
    }

    /// Proves a non-crossing reversed cusp span relinks at a bounded exact tangent node.
    ///
    /// # Panics
    ///
    /// Panics when the reversed middle survives, either forward branch is removed, the tangent
    /// intersection is approximated, or the solved node is not stored on both adjacent segments.
    #[test]
    fn constant_gap_reversed_cusp_span_relinks_at_vector_node() {
        let retained_prefix = CurveSegment::Line(
            LineSegment::new(Point2::new(0.0, 0.0), Point2::new(4.0, 0.0))
                .expect("finite retained prefix"),
        );
        let reversed = CurveSegment::Line(
            LineSegment::new(Point2::new(4.0, 0.0), Point2::new(3.0, 1.0))
                .expect("finite reversed middle"),
        );
        let retained_suffix = CurveSegment::Line(
            LineSegment::new(Point2::new(3.0, 1.0), Point2::new(3.0, 4.0))
                .expect("finite retained suffix"),
        );
        let traced = |segment, source_index, orientation| TracedOffsetSegment {
            segment,
            source_start: PathLocation::new(source_index, 0.0).expect("source start validates"),
            source_end: PathLocation::new(source_index, 1.0).expect("source end validates"),
            orientation: Some(orientation),
        };
        let mut runs = vec![OrderedOffsetRun {
            segments: vec![
                traced(retained_prefix, 0, OffsetOrientation::Retained),
                traced(reversed, 1, OffsetOrientation::Reversed),
                traced(retained_suffix, 2, OffsetOrientation::Retained),
            ],
            first_source_segment: retained_prefix,
            last_source_segment: retained_suffix,
        }];
        relink_planar_reversed_spans(&mut runs, 1.0, DEFAULT_PATH_OFFSET_TOLERANCE)
            .expect("bounded reversed cusp relink succeeds");
        let expected_node = Point2::new(3.0, 0.0);
        assert_eq!(runs[0].segments.len(), 2);
        assert_eq!(runs[0].segments[0].segment.end(), expected_node);
        assert_eq!(runs[0].segments[1].segment.start(), expected_node);
    }

    /// Proves a propagated retained-orientation fold relinks without touching an authored corner.
    ///
    /// # Panics
    ///
    /// Panics when geometric backtrack detection keeps the folded middle, changes either exterior
    /// branch tangent, or fails to store their exact bounded intersection as the shared node.
    #[test]
    fn constant_gap_propagated_backtrack_relinks_at_vector_node() {
        let prefix = CurveSegment::Line(
            LineSegment::new(Point2::new(0.0, 0.0), Point2::new(4.0, 0.0)).expect("finite prefix"),
        );
        let fold = CurveSegment::Line(
            LineSegment::new(Point2::new(4.0, 0.0), Point2::new(2.0, 1.0)).expect("finite fold"),
        );
        let middle = CurveSegment::Line(
            LineSegment::new(Point2::new(2.0, 1.0), Point2::new(5.0, 1.0))
                .expect("finite fold middle"),
        );
        let suffix = CurveSegment::Line(
            LineSegment::new(Point2::new(5.0, 1.0), Point2::new(8.0, 4.0)).expect("finite suffix"),
        );
        let traced = |segment, source_start, source_end| TracedOffsetSegment {
            segment,
            source_start: PathLocation::new(0, source_start).expect("source start validates"),
            source_end: PathLocation::new(0, source_end).expect("source end validates"),
            orientation: Some(OffsetOrientation::Retained),
        };
        let mut runs = vec![OrderedOffsetRun {
            segments: vec![
                traced(prefix, 0.0, 0.25),
                traced(fold, 0.25, 0.5),
                traced(middle, 0.5, 0.75),
                traced(suffix, 0.75, 1.0),
            ],
            first_source_segment: prefix,
            last_source_segment: suffix,
        }];
        relink_planar_backtracking_spans(&mut runs, 1.0, DEFAULT_PATH_OFFSET_TOLERANCE, &[])
            .expect("bounded propagated fold relinks");
        let expected_node = Point2::new(4.0, 0.0);
        assert_eq!(runs[0].segments.len(), 2);
        assert_eq!(runs[0].segments[0].segment.end(), expected_node);
        assert_eq!(runs[0].segments[1].segment.start(), expected_node);
    }

    /// Proves a cubic that overshoots its terminal node is truncated at its exact forward-loss root.
    ///
    /// # Panics
    ///
    /// Panics when the fixture's terminal curl is not detected, the discarded suffix survives,
    /// the following handle loses its direction, or the two retained segments do not share one
    /// exact vector node.
    #[test]
    fn constant_gap_terminal_curl_stops_at_forward_loss_node() {
        let curled = CurveSegment::CubicBezier(
            crate::CubicBezierSegment::new(
                Point2::new(0.0, 0.0),
                Point2::new(0.308_150_366_14, -0.163_731_304_68),
                Point2::new(0.217_563_263_50, -0.122_176_343_98),
                Point2::new(0.217_563_263_50, -0.122_176_343_98),
            )
            .expect("finite curled cubic"),
        );
        let continuation = CurveSegment::CubicBezier(
            crate::CubicBezierSegment::new(
                curled.end(),
                Point2::new(2.060_753_918_91, -1.017_509_745_24),
                Point2::new(4.273_596_275_06, -1.977_731_623_10),
                Point2::new(5.958_932_182_64, -2.667_417_520_94),
            )
            .expect("finite continuation cubic"),
        );
        let traced = |segment, source_start, source_end| TracedOffsetSegment {
            segment,
            source_start: PathLocation::new(0, source_start).expect("source start validates"),
            source_end: PathLocation::new(0, source_end).expect("source end validates"),
            orientation: Some(OffsetOrientation::Retained),
        };
        let mut runs = vec![OrderedOffsetRun {
            segments: vec![traced(curled, 0.0, 0.5), traced(continuation, 0.5, 1.0)],
            first_source_segment: curled,
            last_source_segment: continuation,
        }];
        let original_source_end = runs[0].segments[0].source_end;
        let continuation_tangent = continuation
            .limiting_unit_tangent_at(0.0)
            .expect("continuation tangent remains finite");
        let loss = terminal_forward_loss_parameter(curled, continuation_tangent)
            .expect("terminal curl analysis succeeds")
            .expect("fixture contains a terminal curl");
        assert!((0.0..1.0).contains(&loss));
        let forward_loss_node = curled
            .point_at(loss)
            .expect("forward-loss node remains finite");
        let incoming_tangent = curled
            .unit_tangent_at(loss)
            .expect("forward-loss tangent remains finite");
        let expected_node = line_intersection(
            forward_loss_node,
            Point2::new(
                forward_loss_node.x + incoming_tangent.x,
                forward_loss_node.y + incoming_tangent.y,
            ),
            continuation.start(),
            Point2::new(
                continuation.start().x + continuation_tangent.x,
                continuation.start().y + continuation_tangent.y,
            ),
        )
        .expect("reciprocal terminal tangents have one finite intersection");
        relink_planar_terminal_curls(&mut runs, 1.0, DEFAULT_PATH_OFFSET_TOLERANCE)
            .expect("terminal curl relinks");
        let prefix = runs[0].segments[0];
        let suffix = runs[0].segments[1];
        assert_eq!(prefix.segment.end(), suffix.segment.start());
        assert!(
            (prefix.segment.end().x - expected_node.x)
                .hypot(prefix.segment.end().y - expected_node.y)
                <= 1.0e-12
        );
        assert_eq!(
            prefix.source_end.segment_index(),
            original_source_end.segment_index()
        );
        assert!(prefix.source_end.parameter() < original_source_end.parameter());
        let incoming = prefix
            .segment
            .limiting_unit_tangent_at(1.0)
            .expect("trimmed incoming tangent remains finite");
        let outgoing = suffix
            .segment
            .limiting_unit_tangent_at(0.0)
            .expect("moved outgoing tangent remains finite");
        assert!(incoming.dot(outgoing) >= -1.0e-10);
    }

    /// Proves post-clipping stabilization removes a photographed curl without moving its vector node.
    ///
    /// # Panics
    ///
    /// Panics when the fixed finite fixture cannot construct, its terminal hairpin is absent, the
    /// stabilizer changes the shared node, or the repaired incoming tangent remains reversed.
    #[test]
    fn constant_gap_fixed_vector_node_survives_terminal_curl_stabilization() {
        let fixed_node = Point2::new(310.667_829, 48.317_194);
        let curled = CurveSegment::CubicBezier(
            crate::CubicBezierSegment::new(
                Point2::new(310.390_825, 49.079_58),
                Point2::new(311.608_187, 49.835_3),
                fixed_node,
                fixed_node,
            )
            .expect("finite photographed curl"),
        );
        let continuation = CurveSegment::CubicBezier(
            crate::CubicBezierSegment::new(
                fixed_node,
                Point2::new(310.759_5, 48.417_5),
                Point2::new(313.602_883, 51.530_22),
                Point2::new(313.602_883, 51.530_22),
            )
            .expect("finite photographed continuation"),
        );
        let continuation_tangent = continuation
            .limiting_unit_tangent_at(0.0)
            .expect("continuation tangent remains finite");
        let curled_incoming = curled
            .limiting_unit_tangent_at(1.0)
            .expect("curled incoming tangent remains finite");
        assert!(curled_incoming.dot(continuation_tangent) < -0.9);
        assert!(
            terminal_forward_loss_parameter(curled, continuation_tangent)
                .expect("terminal curl analysis succeeds")
                .is_some(),
            "fixture retains the photographed terminal reversal"
        );
        let path = CurvePath::new(vec![curled, continuation], PathClosure::Open)
            .expect("photographed fixture is connected");
        let stabilized = stabilize_planar_constant_gap_path(&path, PathOffsetLimits::default())
            .expect("fixed-node stabilization remains bounded");
        assert_eq!(stabilized.segments().len(), 2);
        assert_eq!(stabilized.segments()[0].end(), fixed_node);
        assert_eq!(stabilized.segments()[1].start(), fixed_node);
        let repaired_incoming = stabilized.segments()[0]
            .limiting_unit_tangent_at(1.0)
            .expect("repaired incoming tangent remains finite");
        assert!(repaired_incoming.dot(continuation_tangent) > -0.5);
    }

    /// Proves a solved crossing switch is displaced as separate segments and relinked exactly.
    ///
    /// # Panics
    ///
    /// Panics when the next rank rounds the preceding switch as an authored corner, loses the new
    /// exact intersection, or fails to carry that intersection as next-rank construction authority.
    #[test]
    fn constant_gap_crossing_switch_relinks_displaced_segments() {
        let source = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(4.0, 4.0),
            ],
            PathClosure::Open,
        )
        .expect("finite switched source path");
        let PathOffsetResult::Paths(paths) = advance_planar_constant_gap_frontier_cancellable(
            &source,
            &[Point2::new(4.0, 0.0)],
            1.0,
            PathOffsetLimits::default(),
            &|| false,
        )
        .expect("displaced crossing switch remains bounded") else {
            panic!("nondegenerate crossing switch retains an exterior path");
        };
        let exterior = paths
            .iter()
            .find(|component| component.path.closure() == PathClosure::Open)
            .expect("crossing switch publishes one exterior continuation");
        assert_eq!(exterior.path.segments().len(), 2);
        assert!(
            exterior
                .path
                .segments()
                .iter()
                .all(|segment| matches!(segment, CurveSegment::Line(_)))
        );
        let node = Point2::new(3.0, 1.0);
        assert_eq!(exterior.path.segments()[0].end(), node);
        assert_eq!(exterior.path.segments()[1].start(), node);
        assert_eq!(exterior.planar_switch_nodes, vec![node]);
        let PathOffsetResult::Paths(next_paths) = advance_planar_constant_gap_frontier_cancellable(
            &exterior.path,
            &exterior.planar_switch_nodes,
            1.0,
            PathOffsetLimits::default(),
            &|| false,
        )
        .expect("second displaced crossing switch remains bounded") else {
            panic!("second crossing-switch rank retains an exterior path");
        };
        let next_exterior = next_paths
            .iter()
            .find(|component| component.path.closure() == PathClosure::Open)
            .expect("second rank publishes one exterior continuation");
        let next_node = Point2::new(2.0, 2.0);
        assert_eq!(next_exterior.path.segments()[0].end(), next_node);
        assert_eq!(next_exterior.path.segments()[1].start(), next_node);
        assert_eq!(next_exterior.planar_switch_nodes, vec![next_node]);
    }

    /// Proves closed cleanup retains authored winding and dissolves the opposite reversal loop.
    #[test]
    fn closed_crossing_cleanup_dissolves_reversal_winding() {
        let path = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(6.0, 6.0),
                Point2::new(0.0, 6.0),
                Point2::new(4.0, 0.0),
            ],
            PathClosure::Closed,
        )
        .expect("finite closed crossing-shaped polyline");
        let retained_winding = path_winding(&path, DEFAULT_PATH_OFFSET_TOLERANCE)
            .expect("finite exact area")
            .expect("asymmetric crossing has nonzero winding");
        let components = dissolve_crossings(
            traced(&path),
            PathClosure::Closed,
            Some(retained_winding),
            MAX_PATH_OFFSET_CLEANUP_PAIRS,
            MAX_PATH_OFFSET_COMPONENTS,
            DEFAULT_PATH_OFFSET_TOLERANCE,
            false,
            &|| false,
        )
        .expect("closed cleanup stays bounded");
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].path.closure(), PathClosure::Closed);
        assert_eq!(
            path_winding(&components[0].path, DEFAULT_PATH_OFFSET_TOLERANCE).unwrap(),
            Some(retained_winding)
        );
    }

    /// Proves the public offset boundary publishes every cleaned component with distinct real provenance.
    #[test]
    fn offset_request_publishes_multiple_cleanup_components() {
        let path = CurvePath::polyline(
            vec![
                Point2::new(-2.0, -2.0),
                Point2::new(-1.0, -2.0),
                Point2::new(0.0, 0.0),
                Point2::new(8.0, 8.0),
                Point2::new(0.0, 8.0),
                Point2::new(8.0, 0.0),
            ],
            PathClosure::Open,
        )
        .expect("finite crossing source");
        let result = offset_path_cancellable(
            PathOffsetRequest {
                path: &path,
                signed_distance: 0.5,
                endpoint_policy: PathOffsetEndpointPolicy::Preserve,
                cleanup: PathOffsetCleanup::DissolveCrossings,
                crossing_barriers: &[],
                limits: PathOffsetLimits::default(),
            },
            &|| false,
        )
        .expect("crossing offset remains bounded");
        let PathOffsetResult::Paths(components) = result else {
            panic!("crossing offset retains finite components")
        };
        assert!(components.len() >= 2);
        assert!(components.windows(2).all(|pair| {
            pair[0].component_ordinal < pair[1].component_ordinal
                && (pair[0].source_start != pair[1].source_start
                    || pair[0].source_end != pair[1].source_end)
        }));
    }

    /// Proves request-local crossing-work and component limits fail atomically at the public boundary.
    #[test]
    fn cleanup_pair_and_component_limits_are_request_scoped() {
        let path = CurvePath::polyline(
            vec![
                Point2::new(-2.0, -2.0),
                Point2::new(-1.0, -2.0),
                Point2::new(0.0, 0.0),
                Point2::new(8.0, 8.0),
                Point2::new(0.0, 8.0),
                Point2::new(8.0, 0.0),
                Point2::new(10.0, 2.0),
                Point2::new(18.0, 10.0),
                Point2::new(10.0, 10.0),
                Point2::new(18.0, 2.0),
            ],
            PathClosure::Open,
        )
        .expect("finite crossing source");
        let request = |limits| PathOffsetRequest {
            path: &path,
            signed_distance: 0.5,
            endpoint_policy: PathOffsetEndpointPolicy::Preserve,
            cleanup: PathOffsetCleanup::DissolveCrossings,
            crossing_barriers: &[],
            limits,
        };
        let pair_error = offset_path_cancellable(
            request(PathOffsetLimits {
                maximum_cleanup_pairs: 1,
                ..PathOffsetLimits::default()
            }),
            &|| false,
        )
        .expect_err("cleanup pair exhaustion publishes no component");
        assert_eq!(pair_error.path(), "curve.offset.cleanup_limit");
        let component_error = offset_path_cancellable(
            request(PathOffsetLimits {
                maximum_components: 1,
                ..PathOffsetLimits::default()
            }),
            &|| false,
        )
        .expect_err("component exhaustion publishes no partial result");
        assert_eq!(component_error.path(), "curve.offset.component_limit");
    }

    /// Proves the compact cubic diagnostic offset ladder contains no stationary published line tangent.
    #[test]
    fn cubic_offset_ladder_has_no_stationary_published_segments() {
        let path = CurvePath::new(
            vec![CurveSegment::CubicBezier(
                crate::CubicBezierSegment::new(
                    Point2::new(48.0, 384.0),
                    Point2::new(230.4, 153.6),
                    Point2::new(537.6, 153.6),
                    Point2::new(720.0, 384.0),
                )
                .expect("finite diagnostic cubic"),
            )],
            PathClosure::Open,
        )
        .expect("connected diagnostic cubic");
        for index in -8..=8 {
            let result = offset_path_cancellable(
                PathOffsetRequest {
                    path: &path,
                    signed_distance: f64::from(index) * 64.0,
                    endpoint_policy: PathOffsetEndpointPolicy::TangentialExtension {
                        bounds: Bounds::new(Point2::new(-64.0, -64.0), Point2::new(832.0, 832.0))
                            .expect("finite diagnostic bounds"),
                    },
                    cleanup: PathOffsetCleanup::DissolveCrossings,
                    crossing_barriers: &[],
                    limits: PathOffsetLimits::default(),
                },
                &|| false,
            )
            .expect("offset construction remains finite");
            let PathOffsetResult::Paths(paths) = result else {
                continue;
            };
            for component in &paths {
                for (segment_index, segment) in component.path.segments().iter().enumerate() {
                    for parameter in [0.0, 0.25, 0.5, 0.75, 1.0] {
                        segment.unit_tangent_at(parameter).unwrap_or_else(|error| {
                            panic!(
                                "index {index}, segment {segment_index}, parameter {parameter}: {error}"
                            )
                        });
                    }
                }
            }
        }
    }
}
