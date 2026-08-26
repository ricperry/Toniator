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
pub const PATH_OFFSET_ALGORITHM_CONTRACT_ID: &str =
    "toniator.path-offset.v5.endpoint-envelope-collapse";

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
}

/// One cleaned component with a source interval that is never reconstructed from output bounds.
#[derive(Clone, Debug)]
struct CleanedOffsetPath {
    path: CurvePath,
    source_start: PathLocation,
    source_end: PathLocation,
    earliest_source: PathLocation,
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

/// One dyadic source-cubic interval proven to retain authored offset traversal.
#[derive(Clone, Copy, Debug)]
struct RetainedCubicInterval {
    source_start: f64,
    source_end: f64,
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
    offset_path_with_work_join_policy_cancellable(
        request,
        work,
        is_cancelled,
        OffsetJoinPolicy::CompactRound,
    )
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
    )
}

/// Selects the geometry-owned construction policy for source-adjacent offset corners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OffsetJoinPolicy {
    /// Retains accepted compact round and deterministic bevel behavior for path products.
    CompactRound,
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
                limits.maximum_segments,
                &mut emitted_segments,
            )?;
        }
    }
    let mut cleanup_budget = CleanupBudget {
        examined_pairs: work.cleanup_pairs,
        maximum_pairs: limits.maximum_cleanup_pairs,
    };
    dissolve_cross_run_reversal_crossings(
        &mut runs,
        authored_start,
        authored_end,
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
        let mut cleaned = match request.cleanup {
            PathOffsetCleanup::DissolveCrossings => dissolve_crossings_with_budget(
                run.segments,
                closure,
                (closure == PathClosure::Closed)
                    .then_some(retained_winding)
                    .flatten(),
                &mut cleanup_budget,
                limits.maximum_components,
                limits.tolerance,
                join_policy == OffsetJoinPolicy::RegionRound && request.signed_distance > 0.0,
                is_cancelled,
            )?,
        };
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
            })
            .collect(),
    ))
}

/// Removes the non-authoritative side of a candidate's first and last barrier crossings.
///
/// Nearer same-side offsets are immutable barriers. A candidate crossed on both terminal
/// extensions has exhausted its extended envelope and collapses; other crossings retain only
/// source-authoritative pieces. Cleanup never mutates or joins a previously published repetition.
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
/// Returns stable cleanup-limit, cancellation, overlap, subdivision, or numeric diagnostics before
/// the caller mutates any candidate run.
fn crossing_barrier_span(
    runs: &[OrderedOffsetRun],
    barriers: &[&CurvePath],
    budget: &mut CleanupBudget,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Option<(RunBarrierCrossing, RunBarrierCrossing)>, CurveError> {
    let mut first = None;
    let mut last = None;
    for (run_index, run) in runs.iter().enumerate() {
        for (segment_index, candidate) in run.segments.iter().enumerate() {
            for barrier in barriers {
                for barrier_segment in barrier.segments() {
                    if is_cancelled() {
                        return Err(cancelled());
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
                    for intersection in candidate.segment.intersections(barrier_segment)? {
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
/// branches. Runs remain disconnected across every discarded source interval.
///
/// # Errors
///
/// Returns bounded intersection, cleanup-limit, cancellation, or finite split diagnostics without
/// publishing partially trimmed runs.
fn dissolve_cross_run_reversal_crossings(
    runs: &mut Vec<OrderedOffsetRun>,
    authored_start: PathLocation,
    authored_end: PathLocation,
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
            let (first_prefix, _) = split_traced_segment(first_crossed, first_parameter, point)?;
            let (_, second_suffix) = split_traced_segment(second_crossed, second_parameter, point)?;
            runs[first_run].segments.truncate(first_segment);
            runs[first_run].segments.push(first_prefix);
            let mut retained_suffix = vec![second_suffix];
            retained_suffix.extend_from_slice(&runs[second_run].segments[(second_segment + 1)..]);
            runs[second_run].segments = retained_suffix;
            if second_run > first_run + 1 {
                runs.drain((first_run + 1)..second_run);
            }
        }
    }
    Ok(())
}

/// Finds the earliest source-ordered transverse crossing between distinct retained runs.
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
    for first_run in 0..runs.len() {
        for second_run in (first_run + 1)..runs.len() {
            for (first_segment, first) in runs[first_run].segments.iter().enumerate() {
                for (second_segment, second) in runs[second_run].segments.iter().enumerate() {
                    if is_cancelled() {
                        return Err(cancelled());
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
                        if intersection.kind() == crate::IntersectionKind::Crossing
                            && strictly_interior(intersection.first_parameter())
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
fn append_ordered_offset_run(
    runs: &mut Vec<OrderedOffsetRun>,
    mut offsets: Vec<TracedOffsetSegment>,
    source_segment: CurveSegment,
    distance: f64,
    join_policy: OffsetJoinPolicy,
    maximum_segments: usize,
    emitted_segments: &mut usize,
) -> Result<(), CurveError> {
    if offsets.is_empty() {
        return Ok(());
    }
    let joins_previous = runs.last().is_some_and(|previous| {
        source_locations_are_adjacent(
            previous
                .segments
                .last()
                .expect("ordered offset run is nonempty")
                .source_end,
            offsets
                .first()
                .expect("new ordered offset run is nonempty")
                .source_start,
        )
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
fn connect_adjacent_offset_segments(
    derived: &mut Vec<TracedOffsetSegment>,
    next: &mut [TracedOffsetSegment],
    previous_source: CurveSegment,
    next_source: CurveSegment,
    source_boundary: PathLocation,
    distance: f64,
    join_policy: OffsetJoinPolicy,
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
    if gap <= 1.0e-6 {
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
        let previous_direction = run.last_source_segment.unit_tangent_at(1.0)?;
        let next_direction = run.first_source_segment.unit_tangent_at(0.0)?;
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

/// Splits transverse self-crossings into source-ordered components without reversing any source edge.
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
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CleanedOffsetPath>, CurveError> {
    let mut pending = vec![(segments, closure)];
    let mut cleaned = Vec::new();
    while let Some((segments, closure)) = pending.pop() {
        if is_cancelled() {
            return Err(cancelled());
        }
        let plain = segments
            .iter()
            .map(|segment| segment.segment)
            .collect::<Vec<_>>();
        let Some(crossing) = first_transverse_crossing_with_budget(
            &plain,
            budget,
            dissolve_coincident_overlaps,
            is_cancelled,
        )?
        else {
            if segments.is_empty() {
                continue;
            }
            let path = CurvePath::new(plain, closure)?;
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
            if earliest_source == latest_source {
                continue;
            }
            cleaned.push(CleanedOffsetPath {
                path,
                source_start: earliest_source,
                source_end: latest_source,
                earliest_source,
            });
            if cleaned.len() + pending.len() > maximum_components {
                return Err(CurveError::new(
                    "curve.offset.component_limit",
                    "path offset component limit exceeded",
                ));
            }
            continue;
        };
        let split = split_at_crossing(segments, closure, crossing)?;
        if cleaned.len() + pending.len() + split.len() > maximum_components {
            return Err(CurveError::new(
                "curve.offset.component_limit",
                "path offset component limit exceeded",
            ));
        }
        for component in split.into_iter().rev() {
            pending.push(component);
        }
    }
    cleaned.sort_by(|left, right| {
        source_location_key(left.earliest_source).cmp(&source_location_key(right.earliest_source))
    });
    Ok(cleaned)
}

/// Supplies a fresh cleanup budget for focused cleanup-only tests.
///
/// # Errors
///
/// Returns the same bounded cleanup diagnostics as the request-wide implementation.
#[cfg(test)]
fn dissolve_crossings(
    segments: Vec<TracedOffsetSegment>,
    closure: PathClosure,
    retained_winding: Option<WindingDirection>,
    maximum_pairs: usize,
    maximum_components: usize,
    tolerance: f64,
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
    for (first_index, first) in segments.iter().enumerate() {
        for (second_index, second) in segments.iter().enumerate().skip(first_index + 2) {
            if is_cancelled() {
                return Err(cancelled());
            }
            budget.examined_pairs += 1;
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
        },
        TracedOffsetSegment {
            segment: suffix,
            source_start: source_middle,
            source_end: segment.source_end,
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
    let previous_direction = previous_source.unit_tangent_at(1.0)?;
    let next_direction = next_source.unit_tangent_at(0.0)?;
    let turn = previous_direction.x * next_direction.y - previous_direction.y * next_direction.x;
    if turn.abs() <= 1.0e-12 {
        return Ok(false);
    }
    let uses_outer_join = match join_policy {
        OffsetJoinPolicy::CompactRound => turn * distance < 0.0,
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
    let previous_direction = previous_source.unit_tangent_at(1.0)?;
    let next_direction = next_source.unit_tangent_at(0.0)?;
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
            }]])
        }
        CurveSegment::CubicBezier(cubic) => cubic_offset_runs(
            cubic,
            source_segment_index,
            distance,
            limits,
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
    cusp_budget: &mut CuspIsolationBudget,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<TracedOffsetSegment>>, CurveError> {
    let intervals =
        isolate_cubic_offset_intervals(cubic, distance, limits, cusp_budget, is_cancelled)?;
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
        let retained_cubic = cubic_subinterval(cubic, source_start, source_end)?;
        let fitted = adaptive_cubic_offsets(
            retained_cubic,
            source_segment_index,
            source_start,
            source_end,
            distance,
            limits,
            is_cancelled,
        )?;
        if !fitted.is_empty() {
            runs.push(fitted);
        }
    }
    Ok(runs)
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
/// Intervals with positive `|v|^3 - distance * cross(v, a)` retain authored traversal;
/// negative intervals are omitted. An unresolved interval is omitted only after its conservative
/// offset-locus length is within the fixed geometry tolerance.
///
/// # Errors
///
/// Returns stable work/depth, cancellation, or non-finite-arithmetic diagnostics atomically.
fn isolate_cubic_offset_intervals(
    cubic: crate::CubicBezierSegment,
    distance: f64,
    limits: PathOffsetLimits,
    budget: &mut CuspIsolationBudget,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<RetainedCubicInterval>>, CurveError> {
    let mut pending = vec![(cubic, 0_u8, 0.0_f64, 1.0_f64)];
    let mut retained = Vec::new();
    while let Some((candidate, depth, source_start, source_end)) = pending.pop() {
        if is_cancelled() {
            return Err(cancelled());
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
            OffsetOrientation::Retained => retained.push(RetainedCubicInterval {
                source_start,
                source_end,
            }),
            OffsetOrientation::Reversed => {}
            OffsetOrientation::Uncertain => {
                if cubic_interval_has_zero_witness(candidate, distance)?
                    && uncertain_offset_locus_length_bound(bounds)? <= limits.tolerance
                {
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
            .is_some_and(|previous| previous.source_end == interval.source_start)
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
fn adaptive_cubic_offsets(
    cubic: crate::CubicBezierSegment,
    source_segment_index: usize,
    source_parameter_start: f64,
    source_parameter_end: f64,
    distance: f64,
    limits: PathOffsetLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<TracedOffsetSegment>, CurveError> {
    let mut pending = vec![(cubic, 0_u8, source_parameter_start, source_parameter_end)];
    let mut pieces: Vec<TracedOffsetSegment> = Vec::new();
    while let Some((candidate, depth, source_start, source_end)) = pending.pop() {
        if is_cancelled() {
            return Err(cancelled());
        }
        let exact_start = exact_cubic_offset_point(candidate, distance, 0.0)?;
        let exact_end = exact_cubic_offset_point(candidate, distance, 1.0)?;
        if cubic_offset_line_error(candidate, exact_start, exact_end, distance)? <= limits.tolerance
        {
            if (exact_start.x - exact_end.x).hypot(exact_start.y - exact_end.y) > limits.tolerance {
                let mut segment = CurveSegment::Line(LineSegment::new(exact_start, exact_end)?);
                if let Some(previous) = pieces.last() {
                    segment = replace_segment_start(segment, previous.segment.end())?;
                }
                pieces.push(TracedOffsetSegment {
                    segment,
                    source_start: PathLocation::new(source_segment_index, source_start)?,
                    source_end: PathLocation::new(source_segment_index, source_end)?,
                });
                if pieces.len() > limits.maximum_segments {
                    return Err(CurveError::new(
                        "curve.offset.segment_limit",
                        "path offset segment limit exceeded",
                    ));
                }
            }
            continue;
        }
        let fitted = fit_offset_cubic(candidate, distance)?;
        if cubic_offset_error(candidate, fitted, distance)? <= limits.tolerance {
            let mut segment = CurveSegment::CubicBezier(fitted);
            if let Some(previous) = pieces.last() {
                segment = replace_segment_start(segment, previous.segment.end())?;
            }
            pieces.push(TracedOffsetSegment {
                segment,
                source_start: PathLocation::new(source_segment_index, source_start)?,
                source_end: PathLocation::new(source_segment_index, source_end)?,
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
        let (left, right) = candidate.split(0.5)?;
        let source_middle = (source_start + source_end) * 0.5;
        pending.push((right, depth + 1, source_middle, source_end));
        pending.push((left, depth + 1, source_start, source_middle));
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
    let normal = segment.unit_normal_at(parameter)?;
    let offset = Point2::new(point.x + normal.x * distance, point.y + normal.y * distance);
    if !offset.is_finite() {
        return Err(CurveError::new(
            "curve.offset.numeric_overflow",
            "cubic offset point overflowed",
        ));
    }
    Ok(offset)
}

/// Measures exact offset samples against a direct compact line candidate.
///
/// # Errors
///
/// Returns source tangent or finite-coordinate diagnostics instead of accepting an unverified line.
fn cubic_offset_line_error(
    cubic: crate::CubicBezierSegment,
    start: Point2,
    end: Point2,
    distance: f64,
) -> Result<f64, CurveError> {
    let mut greatest = 0.0_f64;
    for parameter in [0.25, 0.5, 0.75] {
        let expected = exact_cubic_offset_point(cubic, distance, parameter)?;
        let actual = Point2::new(
            start.x + (end.x - start.x) * parameter,
            start.y + (end.y - start.y) * parameter,
        );
        greatest = greatest.max((expected.x - actual.x).hypot(expected.y - actual.y));
    }
    Ok(greatest)
}

/// Constructs one cubic Hermite offset fit from exact offset endpoints and endpoint tangents.
///
/// # Errors
///
/// Returns exact tangent and finite-coordinate diagnostics without synthesizing a fallback normal.
fn fit_offset_cubic(
    cubic: crate::CubicBezierSegment,
    distance: f64,
) -> Result<crate::CubicBezierSegment, CurveError> {
    let start = exact_cubic_offset_point(cubic, distance, 0.0)?;
    let end = exact_cubic_offset_point(cubic, distance, 1.0)?;
    let start_derivative = offset_cubic_endpoint_derivative(cubic, distance, false)?;
    let end_derivative = offset_cubic_endpoint_derivative(cubic, distance, true)?;
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

/// Returns the analytic endpoint derivative of one signed cubic offset curve.
///
/// The derivative combines the source velocity with the derivative of its unit left normal, so
/// fitted controls preserve direction and magnitude instead of depending on a chord heuristic.
///
/// # Errors
///
/// Returns a stable stationary-tangent or finite-arithmetic diagnostic without normalizing invalid data.
fn offset_cubic_endpoint_derivative(
    cubic: crate::CubicBezierSegment,
    distance: f64,
    at_end: bool,
) -> Result<Vector2, CurveError> {
    let (velocity, acceleration) = if at_end {
        (
            Vector2::new(
                3.0 * (cubic.end().x - cubic.control_2().x),
                3.0 * (cubic.end().y - cubic.control_2().y),
            ),
            Vector2::new(
                6.0 * (cubic.end().x - 2.0 * cubic.control_2().x + cubic.control_1().x),
                6.0 * (cubic.end().y - 2.0 * cubic.control_2().y + cubic.control_1().y),
            ),
        )
    } else {
        (
            Vector2::new(
                3.0 * (cubic.control_1().x - cubic.start().x),
                3.0 * (cubic.control_1().y - cubic.start().y),
            ),
            Vector2::new(
                6.0 * (cubic.control_2().x - 2.0 * cubic.control_1().x + cubic.start().x),
                6.0 * (cubic.control_2().y - 2.0 * cubic.control_1().y + cubic.start().y),
            ),
        )
    };
    let speed = velocity.x.hypot(velocity.y);
    if !speed.is_finite() || speed == 0.0 {
        return Err(CurveError::new(
            "curve.path.tangent.stationary",
            "curve segment tangent is stationary",
        ));
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
fn cubic_offset_error(
    cubic: crate::CubicBezierSegment,
    fitted: crate::CubicBezierSegment,
    distance: f64,
) -> Result<f64, CurveError> {
    let fit = CurveSegment::CubicBezier(fitted);
    let mut greatest = 0.0_f64;
    for parameter in [0.25, 0.5, 0.75] {
        let expected = exact_cubic_offset_point(cubic, distance, parameter)?;
        let actual = fit.point_at(parameter)?;
        greatest = greatest.max((expected.x - actual.x).hypot(expected.y - actual.y));
    }
    Ok(greatest)
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
                    assert!(
                        source_tangent.x * output_tangent.x + source_tangent.y * output_tangent.y
                            > 0.999,
                        "distance {distance} retains authored traversal"
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
            "toniator.path-offset.v5.endpoint-envelope-collapse"
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
            3.0,
            PathOffsetLimits {
                maximum_subdivision_depth: 1,
                tolerance: 1.0e-12,
                ..PathOffsetLimits::default()
            },
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
