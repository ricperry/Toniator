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
/// The default maximum normal-distance fitting error for one compact cubic offset piece.
pub const DEFAULT_PATH_OFFSET_TOLERANCE: f64 = 1.0 / 64.0;

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
    /// Bounds deterministic cubic offset fitting error in document units.
    pub tolerance: f64,
}

impl Default for PathOffsetLimits {
    /// Returns the fixed Stage 20J request-wide resource defaults.
    fn default() -> Self {
        Self {
            maximum_subdivision_depth: MAX_PATH_OFFSET_SUBDIVISION_DEPTH,
            maximum_segments: MAX_PATH_OFFSET_SEGMENTS,
            maximum_components: MAX_PATH_OFFSET_COMPONENTS,
            maximum_cleanup_pairs: MAX_PATH_OFFSET_CLEANUP_PAIRS,
            tolerance: DEFAULT_PATH_OFFSET_TOLERANCE,
        }
    }
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

/// Identifies two compact segments, their parameters, and the exact point of one transverse crossing.
type TransverseCrossing = (usize, usize, f64, f64, Point2);

/// Builds ordered compact signed normal offsets without handing construction work to renderers.
///
/// # Errors
///
/// Returns stable finite, tangent, cancellation, or configured-limit diagnostics without a partial result.
pub fn offset_path_cancellable(
    request: PathOffsetRequest<'_>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<PathOffsetResult, CurveError> {
    if !request.signed_distance.is_finite() {
        return Err(CurveError::new(
            "curve.offset.distance",
            "path offset distance must be finite",
        ));
    }
    if request.limits.maximum_subdivision_depth == 0
        || request.limits.maximum_segments == 0
        || request.limits.maximum_components == 0
        || request.limits.maximum_cleanup_pairs == 0
        || !request.limits.tolerance.is_finite()
        || request.limits.tolerance <= 0.0
    {
        return Err(CurveError::new(
            "curve.offset.limit",
            "path offset limits must be positive",
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
    let retained_winding = if request.path.closure() == PathClosure::Closed {
        Some(
            path_winding(request.path, request.limits.tolerance)?.ok_or(CurveError::new(
                "curve.offset.winding",
                "closed path offset requires a nonzero authored winding",
            ))?,
        )
    } else {
        None
    };
    let mut derived = Vec::with_capacity(source.path.segments().len().saturating_mul(2));
    let mut previous_end: Option<Point2> = None;
    let mut previous_source: Option<CurveSegment> = None;
    for (index, segment) in source.path.segments().iter().copied().enumerate() {
        if is_cancelled() {
            return Err(cancelled());
        }
        let Some(mut offsets) = offset_segment(
            segment,
            index,
            request.signed_distance,
            request.limits,
            is_cancelled,
        )?
        else {
            continue;
        };
        let source_interval = source.source_intervals[index];
        for offset in &mut offsets {
            offset.source_start =
                remap_extended_source_location(offset.source_start, index, source_interval)?;
            offset.source_end =
                remap_extended_source_location(offset.source_end, index, source_interval)?;
        }
        if offsets.is_empty() {
            continue;
        }
        let offset_start = offsets
            .first()
            .expect("nonempty segment offsets")
            .segment
            .start();
        if let Some(end) = previous_end {
            let gap = (end.x - offset_start.x).hypot(end.y - offset_start.y);
            if gap <= 1.0e-6 {
                offsets[0].segment = replace_segment_start(offsets[0].segment, end)?;
            } else if let Some(previous) = previous_source {
                if !join_offset_segments(
                    &mut derived,
                    &mut offsets,
                    previous,
                    segment,
                    source_interval.0,
                    request.signed_distance,
                )? {
                    let location = source_interval.0;
                    derived.push(TracedOffsetSegment {
                        segment: CurveSegment::Line(LineSegment::new(end, offset_start)?),
                        source_start: location,
                        source_end: location,
                    });
                }
            } else if end != offset_start {
                let location = source_interval.0;
                derived.push(TracedOffsetSegment {
                    segment: CurveSegment::Line(LineSegment::new(end, offset_start)?),
                    source_start: location,
                    source_end: location,
                });
            }
        }
        derived.extend(offsets);
        if derived.len() > request.limits.maximum_segments {
            return Err(CurveError::new(
                "curve.offset.segment_limit",
                "path offset segment limit exceeded",
            ));
        }
        previous_end = Some(
            derived
                .last()
                .expect("offset segment inserted")
                .segment
                .end(),
        );
        previous_source = Some(segment);
    }
    if derived.is_empty() {
        return Ok(PathOffsetResult::Collapsed);
    }
    if source.path.closure() == PathClosure::Closed
        && derived.last().expect("nonempty offset").segment.end() != derived[0].segment.start()
    {
        let location = authored_end;
        derived.push(TracedOffsetSegment {
            segment: CurveSegment::Line(LineSegment::new(
                derived.last().expect("nonempty offset").segment.end(),
                derived[0].segment.start(),
            )?),
            source_start: location,
            source_end: location,
        });
    }
    if derived.len() > request.limits.maximum_segments {
        return Err(CurveError::new(
            "curve.offset.segment_limit",
            "path offset segment limit exceeded",
        ));
    }
    let paths = match request.cleanup {
        PathOffsetCleanup::DissolveCrossings => dissolve_crossings(
            derived,
            source.path.closure(),
            retained_winding,
            request.limits.maximum_cleanup_pairs,
            request.limits.maximum_components,
            request.limits.tolerance,
            is_cancelled,
        )?,
    };
    if paths.is_empty() {
        return Ok(PathOffsetResult::Collapsed);
    }
    if paths.len() > request.limits.maximum_components {
        return Err(CurveError::new(
            "curve.offset.component_limit",
            "path offset component limit exceeded",
        ));
    }
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

/// Splits transverse self-crossings into source-ordered components without reversing any source edge.
///
/// # Errors
///
/// Returns canonical intersection, component-limit, cancellation, or finite path diagnostics without
/// publishing a partial cleanup.
fn dissolve_crossings(
    segments: Vec<TracedOffsetSegment>,
    closure: PathClosure,
    retained_winding: Option<WindingDirection>,
    maximum_pairs: usize,
    maximum_components: usize,
    tolerance: f64,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CleanedOffsetPath>, CurveError> {
    let mut pending = vec![(segments, closure)];
    let mut cleaned = Vec::new();
    let mut budget = CleanupBudget {
        examined_pairs: 0,
        maximum_pairs,
    };
    while let Some((segments, closure)) = pending.pop() {
        if is_cancelled() {
            return Err(cancelled());
        }
        let plain = segments
            .iter()
            .map(|segment| segment.segment)
            .collect::<Vec<_>>();
        let Some(crossing) =
            first_transverse_crossing_with_budget(&plain, &mut budget, is_cancelled)?
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
            let source_start = segments.first().expect("nonempty component").source_start;
            let source_end = segments.last().expect("nonempty component").source_end;
            let earliest_source = earliest_source_location(&segments);
            cleaned.push(CleanedOffsetPath {
                path,
                source_start,
                source_end,
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
            for intersection in first.intersections(second)? {
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
    if turn * distance < 0.0 {
        let join = match compact_round_join(
            previous_source.end(),
            previous_derived.segment.end(),
            next_derived.segment.start(),
            turn > 0.0,
        ) {
            Ok(join) => join,
            Err(error) if error.path() == "curve.offset.join" => return Ok(false),
            Err(error) => return Err(error),
        };
        derived.push(TracedOffsetSegment {
            segment: CurveSegment::CubicBezier(join),
            source_start: next_source_start,
            source_end: next_source_start,
        });
        return Ok(true);
    }
    let Some(intersection) = line_intersection(
        previous_derived.segment.end(),
        Point2::new(
            previous_derived.segment.end().x + previous_direction.x,
            previous_derived.segment.end().y + previous_direction.y,
        ),
        next_derived.segment.start(),
        Point2::new(
            next_derived.segment.start().x + next_direction.x,
            next_derived.segment.start().y + next_direction.y,
        ),
    ) else {
        return Ok(false);
    };
    let last = derived.len() - 1;
    derived[last].segment =
        replace_segment_end_preserving_tangent(derived[last].segment, intersection)?;
    next[0].segment = replace_segment_start_preserving_tangent(next[0].segment, intersection)?;
    Ok(true)
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

/// Offsets one nonstationary line or cubic while retaining a compact construction kind.
///
/// # Errors
///
/// Propagates source tangent and finite-coordinate errors rather than introducing a fallback normal.
fn offset_segment(
    segment: CurveSegment,
    source_segment_index: usize,
    distance: f64,
    limits: PathOffsetLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Option<Vec<TracedOffsetSegment>>, CurveError> {
    let start_normal = match segment.unit_normal_at(0.0) {
        Ok(normal) => normal,
        Err(error) if error.path() == "curve.path.tangent.stationary" => return Ok(None),
        Err(error) => return Err(error),
    };
    let end_normal = match segment.unit_normal_at(1.0) {
        Ok(normal) => normal,
        Err(error) if error.path() == "curve.path.tangent.stationary" => return Ok(None),
        Err(error) => return Err(error),
    };
    let translate = |point: Point2, normal: Vector2| {
        Point2::new(point.x + normal.x * distance, point.y + normal.y * distance)
    };
    match segment {
        CurveSegment::Line(line) => Ok(Some(vec![TracedOffsetSegment {
            segment: CurveSegment::Line(LineSegment::new(
                translate(line.start(), start_normal),
                translate(line.end(), end_normal),
            )?),
            source_start: PathLocation::new(source_segment_index, 0.0)?,
            source_end: PathLocation::new(source_segment_index, 1.0)?,
        }])),
        CurveSegment::CubicBezier(cubic) => {
            adaptive_cubic_offsets(cubic, source_segment_index, distance, limits, is_cancelled)
                .map(Some)
        }
    }
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
    distance: f64,
    limits: PathOffsetLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<TracedOffsetSegment>, CurveError> {
    let mut pending = vec![(cubic, 0_u8, 0.0_f64, 1.0_f64)];
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
