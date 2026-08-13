use crate::{Bounds, Point2};

use super::{
    CurveError, CurvePath, CurveSegment, LineSegment, MAX_INTERSECTIONS, MAX_SEGMENT_PAIRS,
    MAX_SUBDIVISION_DEPTH, MAX_WORK_ITEMS, PARAMETER_TOLERANCE, PathLocation, coincident, distance,
    parameter, snap_parameter, tolerance,
};

/// Deterministic classification of a finite curve contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntersectionKind {
    /// The two nonstationary derivatives meet transversely.
    Crossing,
    /// The contact is endpoint, stationary, derivative-parallel, or otherwise nontransverse.
    Tangent,
}

/// One finite intersection expressed in both segment-local parameter spaces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentIntersection {
    first_parameter: f64,
    second_parameter: f64,
    point: Point2,
    kind: IntersectionKind,
}

impl SegmentIntersection {
    /// Creates a normalized intersection only for bounded internal deterministic refinement.
    pub(crate) fn new(
        first_parameter: f64,
        second_parameter: f64,
        point: Point2,
        kind: IntersectionKind,
    ) -> Result<Self, CurveError> {
        Ok(Self {
            first_parameter: parameter(first_parameter)?,
            second_parameter: parameter(second_parameter)?,
            point,
            kind,
        })
    }

    /// Returns the first segment's snapped local parameter.
    pub const fn first_parameter(&self) -> f64 {
        self.first_parameter
    }

    /// Returns the second segment's snapped local parameter.
    pub const fn second_parameter(&self) -> f64 {
        self.second_parameter
    }

    /// Returns the finite coincident contact point.
    pub const fn point(&self) -> Point2 {
        self.point
    }

    /// Returns the deterministic contact classification.
    pub const fn kind(&self) -> IntersectionKind {
        self.kind
    }
}

/// One finite intersection expressed in two path-local locations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathIntersection {
    first_location: PathLocation,
    second_location: PathLocation,
    point: Point2,
    kind: IntersectionKind,
}

impl PathIntersection {
    /// Returns the canonical first path location.
    pub const fn first_location(&self) -> PathLocation {
        self.first_location
    }

    /// Returns the canonical second path location.
    pub const fn second_location(&self) -> PathLocation {
        self.second_location
    }

    /// Returns the finite contact point retained after deterministic deduplication.
    pub const fn point(&self) -> Point2 {
        self.point
    }

    /// Returns the retained crossing or tangency classification.
    pub const fn kind(&self) -> IntersectionKind {
        self.kind
    }
}

/// Intersects two segments under the public fixed work and result budgets.
pub(crate) fn segment_intersections(
    first: CurveSegment,
    second: CurveSegment,
) -> Result<Vec<SegmentIntersection>, CurveError> {
    let mut budget = IntersectionBudget::new(MAX_WORK_ITEMS);
    let mut intersections = segment_intersections_with_budget(first, second, &mut budget)?;
    sort_and_deduplicate(&mut intersections)?;
    if intersections.len() > MAX_INTERSECTIONS {
        return Err(CurveError::new(
            "curve.path.intersections.result_limit",
            "intersection result limit exceeded",
        ));
    }
    Ok(intersections)
}

/// Intersects segments while charging a caller-owned shared subdivision budget.
fn segment_intersections_with_budget(
    first: CurveSegment,
    second: CurveSegment,
    budget: &mut IntersectionBudget,
) -> Result<Vec<SegmentIntersection>, CurveError> {
    match (first, second) {
        (CurveSegment::Line(first), CurveSegment::Line(second)) => line_line(first, second),
        _ => subdivision_intersections(first, second, MAX_SUBDIVISION_DEPTH, budget),
    }
}

/// Intersects path segment pairs and canonicalizes only equivalent topological endpoints.
pub(crate) fn path_intersections(
    first: &CurvePath,
    second: &CurvePath,
) -> Result<Vec<PathIntersection>, CurveError> {
    let pairs = first
        .segments()
        .len()
        .checked_mul(second.segments().len())
        .ok_or(CurveError::new(
            "curve.path.intersections.pair_limit",
            "path intersection pair limit exceeded",
        ))?;
    if pairs > MAX_SEGMENT_PAIRS {
        return Err(CurveError::new(
            "curve.path.intersections.pair_limit",
            "path intersection pair limit exceeded",
        ));
    }
    let mut output = Vec::new();
    let mut budget = IntersectionBudget::new(MAX_WORK_ITEMS);
    for (first_index, first_segment) in first.segments().iter().copied().enumerate() {
        for (second_index, second_segment) in second.segments().iter().copied().enumerate() {
            for intersection in
                segment_intersections_with_budget(first_segment, second_segment, &mut budget)?
            {
                output.push(PathIntersection {
                    first_location: canonical_location(
                        first,
                        first_index,
                        intersection.first_parameter(),
                    )?,
                    second_location: canonical_location(
                        second,
                        second_index,
                        intersection.second_parameter(),
                    )?,
                    point: intersection.point(),
                    kind: intersection.kind(),
                });
            }
        }
    }
    output.sort_by(path_intersection_order);
    output.dedup_by(|first_value, second_value| {
        locations_equivalent(first_value.first_location, second_value.first_location)
            && locations_equivalent(first_value.second_location, second_value.second_location)
            && coincident(first_value.point, second_value.point).unwrap_or(false)
    });
    if output.len() > MAX_INTERSECTIONS {
        return Err(CurveError::new(
            "curve.path.intersections.result_limit",
            "intersection result limit exceeded",
        ));
    }
    Ok(output)
}

/// Converts equivalent adjacent endpoint visits into the earliest stored topological location.
fn canonical_location(
    path: &CurvePath,
    index: usize,
    parameter_value: f64,
) -> Result<PathLocation, CurveError> {
    let parameter_value = snap_parameter(parameter_value);
    if parameter_value == 0.0 && index > 0 {
        if path.segments()[index].start() == path.segments()[index].end() {
            let mut earliest = index;
            while earliest > 0
                && path.segments()[earliest - 1].start() == path.segments()[earliest - 1].end()
                && path.segments()[earliest - 1].end() == path.segments()[earliest].start()
            {
                earliest -= 1;
            }
            if earliest != index {
                return PathLocation::new(earliest, 0.0);
            }
        }
        return PathLocation::new(index - 1, 1.0);
    }
    if parameter_value == 1.0
        && index + 1 == path.segments().len()
        && path.closure() == super::PathClosure::Closed
    {
        return PathLocation::new(0, 0.0);
    }
    PathLocation::new(index, parameter_value)
}

/// Orders path intersections lexicographically by first then second topological location.
fn path_intersection_order(
    first: &PathIntersection,
    second: &PathIntersection,
) -> std::cmp::Ordering {
    first
        .first_location
        .segment_index()
        .cmp(&second.first_location.segment_index())
        .then_with(|| {
            first
                .first_location
                .parameter()
                .total_cmp(&second.first_location.parameter())
        })
        .then_with(|| {
            first
                .second_location
                .segment_index()
                .cmp(&second.second_location.segment_index())
        })
        .then_with(|| {
            first
                .second_location
                .parameter()
                .total_cmp(&second.second_location.parameter())
        })
}

/// Tests path locations for parameter-tolerance equivalence after canonicalization.
fn locations_equivalent(first: PathLocation, second: PathLocation) -> bool {
    first.segment_index() == second.segment_index()
        && (first.parameter() - second.parameter()).abs() <= PARAMETER_TOLERANCE
}

/// Sorts and deduplicates segment contacts only when both parameters and points agree.
fn sort_and_deduplicate(intersections: &mut Vec<SegmentIntersection>) -> Result<(), CurveError> {
    intersections.sort_by(|first, second| {
        first
            .first_parameter()
            .total_cmp(&second.first_parameter())
            .then_with(|| {
                first
                    .second_parameter()
                    .total_cmp(&second.second_parameter())
            })
    });
    let mut deduplicated = Vec::with_capacity(intersections.len());
    for intersection in intersections.drain(..) {
        let duplicate = deduplicated
            .last()
            .is_some_and(|previous: &SegmentIntersection| {
                (previous.first_parameter() - intersection.first_parameter()).abs()
                    <= PARAMETER_TOLERANCE
                    && (previous.second_parameter() - intersection.second_parameter()).abs()
                        <= PARAMETER_TOLERANCE
                    && coincident(previous.point(), intersection.point()).unwrap_or(false)
            });
        if !duplicate {
            deduplicated.push(intersection);
        }
    }
    *intersections = deduplicated;
    Ok(())
}

/// Uses analytical cross products for finite line/line and degenerate point contacts.
fn line_line(
    first: LineSegment,
    second: LineSegment,
) -> Result<Vec<SegmentIntersection>, CurveError> {
    let first_delta = delta(first.start(), first.end())?;
    let second_delta = delta(second.start(), second.end())?;
    let offset = delta(first.start(), second.start())?;
    let first_length = first_delta.0.hypot(first_delta.1);
    let second_length = second_delta.0.hypot(second_delta.1);
    let geometric_tolerance =
        tolerance([first.start(), first.end(), second.start(), second.end()])?;
    if first_length <= geometric_tolerance && second_length <= geometric_tolerance {
        return if coincident(first.start(), second.start())? {
            Ok(vec![SegmentIntersection::new(
                0.0,
                0.0,
                first.start(),
                IntersectionKind::Tangent,
            )?])
        } else {
            Ok(Vec::new())
        };
    }
    if first_length <= geometric_tolerance {
        return point_on_line(first.start(), second, false);
    }
    if second_length <= geometric_tolerance {
        return point_on_line(second.start(), first, true);
    }
    let cross_value = cross(first_delta, second_delta);
    let offset_cross = cross(offset, first_delta);
    let parallel_threshold = super::RELATIVE_TOLERANCE * first_length * second_length;
    if cross_value.abs() <= parallel_threshold {
        if offset_cross.abs() / first_length > geometric_tolerance {
            return Ok(Vec::new());
        }
        return collinear_lines(first, second, first_delta, geometric_tolerance);
    }
    let first_parameter = cross(offset, second_delta) / cross_value;
    let second_parameter = cross(offset, first_delta) / cross_value;
    if !first_parameter.is_finite() || !second_parameter.is_finite() {
        return Err(CurveError::new(
            "curve.path.numeric_overflow",
            "curve-path arithmetic must remain finite",
        ));
    }
    if !(-PARAMETER_TOLERANCE..=1.0 + PARAMETER_TOLERANCE).contains(&first_parameter)
        || !(-PARAMETER_TOLERANCE..=1.0 + PARAMETER_TOLERANCE).contains(&second_parameter)
    {
        return Ok(Vec::new());
    }
    let first_parameter = snap_parameter(first_parameter.clamp(0.0, 1.0));
    let second_parameter = snap_parameter(second_parameter.clamp(0.0, 1.0));
    let point = Point2::new(
        first.start().x + first_delta.0 * first_parameter,
        first.start().y + first_delta.1 * first_parameter,
    );
    let kind = if first_parameter == 0.0
        || first_parameter == 1.0
        || second_parameter == 0.0
        || second_parameter == 1.0
    {
        IntersectionKind::Tangent
    } else {
        IntersectionKind::Crossing
    };
    Ok(vec![SegmentIntersection::new(
        first_parameter,
        second_parameter,
        point,
        kind,
    )?])
}

/// Resolves a degenerate point against a nondegenerate line as one tangency when coincident.
fn point_on_line(
    point: Point2,
    line: LineSegment,
    point_is_first: bool,
) -> Result<Vec<SegmentIntersection>, CurveError> {
    let direction = delta(line.start(), line.end())?;
    let offset = delta(line.start(), point)?;
    let length_squared = dot(direction, direction);
    let parameter_value = dot(offset, direction) / length_squared;
    if cross(direction, offset).abs() / length_squared.sqrt()
        > tolerance([point, line.start(), line.end()])?
        || !(-PARAMETER_TOLERANCE..=1.0 + PARAMETER_TOLERANCE).contains(&parameter_value)
    {
        return Ok(Vec::new());
    }
    let parameter_value = snap_parameter(parameter_value.clamp(0.0, 1.0));
    Ok(vec![if point_is_first {
        SegmentIntersection::new(0.0, parameter_value, point, IntersectionKind::Tangent)?
    } else {
        SegmentIntersection::new(parameter_value, 0.0, point, IntersectionKind::Tangent)?
    }])
}

/// Distinguishes positive collinear overlap from one endpoint tangent contact.
fn collinear_lines(
    first: LineSegment,
    second: LineSegment,
    delta_value: (f64, f64),
    geometric_tolerance: f64,
) -> Result<Vec<SegmentIntersection>, CurveError> {
    let denominator = dot(delta_value, delta_value);
    let second_start = dot(delta(first.start(), second.start())?, delta_value) / denominator;
    let second_end = dot(delta(first.start(), second.end())?, delta_value) / denominator;
    let lower = second_start.min(second_end).max(0.0);
    let upper = second_start.max(second_end).min(1.0);
    if upper < lower - PARAMETER_TOLERANCE {
        return Ok(Vec::new());
    }
    if (upper - lower) * denominator.sqrt() > geometric_tolerance {
        return Err(CurveError::new(
            "curve.path.intersections.overlap",
            "positive-length coincident curve intervals are not discrete intersections",
        ));
    }
    let first_parameter = snap_parameter(((lower + upper) * 0.5).clamp(0.0, 1.0));
    let point = Point2::new(
        first.start().x + delta_value.0 * first_parameter,
        first.start().y + delta_value.1 * first_parameter,
    );
    let second_delta = delta(second.start(), second.end())?;
    let second_parameter = snap_parameter(
        (dot(delta(second.start(), point)?, second_delta) / dot(second_delta, second_delta))
            .clamp(0.0, 1.0),
    );
    Ok(vec![SegmentIntersection::new(
        first_parameter,
        second_parameter,
        point,
        IntersectionKind::Tangent,
    )?])
}

/// Subdivides parameter boxes in fixed order for every pair involving a cubic.
fn subdivision_intersections(
    first: CurveSegment,
    second: CurveSegment,
    maximum_depth: u8,
    budget: &mut IntersectionBudget,
) -> Result<Vec<SegmentIntersection>, CurveError> {
    if coincident_curve_interval(first, second)? {
        return Err(CurveError::new(
            "curve.path.intersections.overlap",
            "positive-length coincident curve intervals are not discrete intersections",
        ));
    }
    let mut stack = vec![(first, 0.0, 1.0, second, 0.0, 1.0, 0_u8)];
    let mut output = Vec::new();
    while let Some((
        first_node,
        first_start,
        first_end,
        second_node,
        second_start,
        second_end,
        depth,
    )) = stack.pop()
    {
        budget.consume()?;
        let first_bounds = first_node.bounds()?;
        let second_bounds = second_node.bounds()?;
        if !bounds_overlap(first_bounds, second_bounds)? {
            continue;
        }
        let scale = operation_tolerance(first_node, second_node, &[])?;
        let first_flat = flat_enough(first_node, scale)?;
        let second_flat = flat_enough(second_node, scale)?;
        if (first_flat && second_flat) || depth >= maximum_depth {
            if depth >= maximum_depth && !(first_flat && second_flat) {
                return Err(CurveError::new(
                    "curve.path.intersections.subdivision_limit",
                    "intersection subdivision depth limit exceeded",
                ));
            }
            let line_first = LineSegment::new(first_node.start(), first_node.end())?;
            let line_second = LineSegment::new(second_node.start(), second_node.end())?;
            for contact in line_line(line_first, line_second)? {
                let first_parameter =
                    first_start + (first_end - first_start) * contact.first_parameter();
                let second_parameter =
                    second_start + (second_end - second_start) * contact.second_parameter();
                let Some((first_parameter, second_parameter, point)) =
                    refine_candidate(first, second, first_parameter, second_parameter)?
                else {
                    continue;
                };
                let kind = classify_contact(first, second, first_parameter, second_parameter)?;
                output.push(SegmentIntersection::new(
                    first_parameter,
                    second_parameter,
                    point,
                    kind,
                )?);
            }
            continue;
        }
        let first_size = bounds_size(first_bounds)?;
        let second_size = bounds_size(second_bounds)?;
        if first_size >= second_size {
            let (left, right) = split_segment(first_node)?;
            let middle = (first_start + first_end) * 0.5;
            stack.push((
                right,
                middle,
                first_end,
                second_node,
                second_start,
                second_end,
                depth + 1,
            ));
            stack.push((
                left,
                first_start,
                middle,
                second_node,
                second_start,
                second_end,
                depth + 1,
            ));
        } else {
            let (left, right) = split_segment(second_node)?;
            let middle = (second_start + second_end) * 0.5;
            stack.push((
                first_node,
                first_start,
                first_end,
                right,
                middle,
                second_end,
                depth + 1,
            ));
            stack.push((
                first_node,
                first_start,
                first_end,
                left,
                second_start,
                middle,
                depth + 1,
            ));
        }
    }
    Ok(output)
}

/// Refines a chord-derived candidate against both source curves and discards residual misses.
fn refine_candidate(
    first: CurveSegment,
    second: CurveSegment,
    mut first_parameter: f64,
    mut second_parameter: f64,
) -> Result<Option<(f64, f64, Point2)>, CurveError> {
    for _ in 0..12 {
        let first_point = first.point_at(first_parameter)?;
        let second_point = second.point_at(second_parameter)?;
        let residual = delta(second_point, first_point)?;
        let tolerance = operation_tolerance(first, second, &[first_point, second_point])?;
        if residual.0.hypot(residual.1) <= tolerance {
            return Ok(Some((
                snap_parameter(first_parameter),
                snap_parameter(second_parameter),
                Point2::new(
                    (first_point.x + second_point.x) * 0.5,
                    (first_point.y + second_point.y) * 0.5,
                ),
            )));
        }
        let first_derivative = first.derivative_at(first_parameter)?;
        let second_derivative = second.derivative_at(second_parameter)?;
        let determinant =
            first_derivative.x * -second_derivative.y - (-second_derivative.x) * first_derivative.y;
        let scale = first_derivative.x.hypot(first_derivative.y)
            * second_derivative.x.hypot(second_derivative.y);
        if !determinant.is_finite() || !scale.is_finite() {
            return Err(CurveError::new(
                "curve.path.numeric_overflow",
                "curve-path arithmetic must remain finite",
            ));
        }
        if determinant.abs() <= super::RELATIVE_TOLERANCE * scale {
            break;
        }
        let first_step =
            (residual.0 * -second_derivative.y - (-second_derivative.x) * residual.1) / determinant;
        let second_step =
            (first_derivative.x * residual.1 - residual.0 * first_derivative.y) / determinant;
        let next_first = first_parameter - first_step;
        let next_second = second_parameter - second_step;
        if !next_first.is_finite() || !next_second.is_finite() {
            return Err(CurveError::new(
                "curve.path.numeric_overflow",
                "curve-path arithmetic must remain finite",
            ));
        }
        first_parameter = next_first.clamp(0.0, 1.0);
        second_parameter = next_second.clamp(0.0, 1.0);
    }
    let first_point = first.point_at(first_parameter)?;
    let second_point = second.point_at(second_parameter)?;
    (distance(first_point, second_point)?
        <= operation_tolerance(first, second, &[first_point, second_point])?)
    .then_some((
        snap_parameter(first_parameter),
        snap_parameter(second_parameter),
        Point2::new(
            (first_point.x + second_point.x) * 0.5,
            (first_point.y + second_point.y) * 0.5,
        ),
    ))
    .ok_or_else(|| {
        CurveError::new(
            "curve.path.intersections.subdivision_limit",
            "intersection refinement did not satisfy the finite residual bound",
        )
    })
    .map(Some)
    .or_else(|error| {
        if error.path() == "curve.path.intersections.subdivision_limit" {
            Ok(None)
        } else {
            Err(error)
        }
    })
}

/// Detects exact or reversed construction coincidence before discrete cubic refinement.
fn coincident_curve_interval(
    first: CurveSegment,
    second: CurveSegment,
) -> Result<bool, CurveError> {
    let direct = first.start() == second.start() && first.end() == second.end();
    let reverse = first.start() == second.end() && first.end() == second.start();
    let controls_match = match (first, second) {
        (CurveSegment::Line(_), CurveSegment::Line(_)) => direct || reverse,
        (CurveSegment::Line(line), CurveSegment::CubicBezier(cubic))
        | (CurveSegment::CubicBezier(cubic), CurveSegment::Line(line)) => {
            point_on_infinite_line(cubic.control_1(), line)?
                && point_on_infinite_line(cubic.control_2(), line)?
        }
        (CurveSegment::CubicBezier(first), CurveSegment::CubicBezier(second)) => {
            (direct
                && first.control_1() == second.control_1()
                && first.control_2() == second.control_2())
                || (reverse
                    && first.control_1() == second.control_2()
                    && first.control_2() == second.control_1())
        }
    };
    Ok(controls_match
        && (direct || reverse)
        && distance(first.start(), first.end())? > tolerance([first.start(), first.end()])?)
}

/// Tests whether a cubic control point lies on a line's supporting axis within the fixed tolerance.
fn point_on_infinite_line(point: Point2, line: LineSegment) -> Result<bool, CurveError> {
    let delta_value = delta(line.start(), line.end())?;
    Ok(
        cross(delta_value, delta(line.start(), point)?).abs() / delta_value.0.hypot(delta_value.1)
            <= tolerance([point, line.start(), line.end()])?,
    )
}

/// Derives one fixed-policy tolerance from complete curve construction plus optional evaluated contacts.
fn operation_tolerance(
    first: CurveSegment,
    second: CurveSegment,
    extra: &[Point2],
) -> Result<f64, CurveError> {
    let mut points = Vec::with_capacity(10 + extra.len());
    append_construction_points(&mut points, first);
    append_construction_points(&mut points, second);
    points.extend_from_slice(extra);
    tolerance(points)
}

/// Appends complete line or cubic construction geometry so tolerance never ignores control extent.
fn append_construction_points(points: &mut Vec<Point2>, segment: CurveSegment) {
    match segment {
        CurveSegment::Line(line) => points.extend([line.start(), line.end()]),
        CurveSegment::CubicBezier(cubic) => points.extend([
            cubic.start(),
            cubic.control_1(),
            cubic.control_2(),
            cubic.end(),
        ]),
    }
}

/// Classifies a refined contact from the two original analytic derivatives.
fn classify_contact(
    first: CurveSegment,
    second: CurveSegment,
    first_parameter: f64,
    second_parameter: f64,
) -> Result<IntersectionKind, CurveError> {
    if first_parameter == 0.0
        || first_parameter == 1.0
        || second_parameter == 0.0
        || second_parameter == 1.0
    {
        return Ok(IntersectionKind::Tangent);
    }
    match (
        first.unit_tangent_at(first_parameter),
        second.unit_tangent_at(second_parameter),
    ) {
        (Ok(first_tangent), Ok(second_tangent)) => {
            let cross_value =
                first_tangent.x * second_tangent.y - first_tangent.y * second_tangent.x;
            Ok(if cross_value.abs() > 1.0e-9 {
                IntersectionKind::Crossing
            } else {
                IntersectionKind::Tangent
            })
        }
        (Err(error), _) | (_, Err(error)) if error.path() == "curve.path.tangent.stationary" => {
            Ok(IntersectionKind::Tangent)
        }
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}

/// Returns a fixed midpoint split while retaining original line/cubic segment kind.
fn split_segment(segment: CurveSegment) -> Result<(CurveSegment, CurveSegment), CurveError> {
    match segment {
        CurveSegment::Line(line) => {
            let midpoint = Point2::new(
                (line.start().x + line.end().x) * 0.5,
                (line.start().y + line.end().y) * 0.5,
            );
            Ok((
                CurveSegment::Line(LineSegment::new(line.start(), midpoint)?),
                CurveSegment::Line(LineSegment::new(midpoint, line.end())?),
            ))
        }
        CurveSegment::CubicBezier(cubic) => {
            let (left, right) = super::segment::split_cubic(cubic, 0.5)?;
            Ok((
                CurveSegment::CubicBezier(left),
                CurveSegment::CubicBezier(right),
            ))
        }
    }
}

/// Determines whether a node's control polygon is within the requested flatness scale.
fn flat_enough(segment: CurveSegment, scale: f64) -> Result<bool, CurveError> {
    match segment {
        CurveSegment::Line(_) => Ok(true),
        CurveSegment::CubicBezier(cubic) => {
            let chord = distance(cubic.start(), cubic.end())?;
            let polygon = super::segment::cubic_polygon_length(cubic)?;
            Ok(polygon - chord <= scale)
        }
    }
}

/// Tests conservative finite bounds for overlap under the fixed scale-aware policy.
fn bounds_overlap(first: Bounds, second: Bounds) -> Result<bool, CurveError> {
    let margin = tolerance([first.min, first.max, second.min, second.max])?;
    Ok(first.min.x <= second.max.x + margin
        && first.max.x + margin >= second.min.x
        && first.min.y <= second.max.y + margin
        && first.max.y + margin >= second.min.y)
}

/// Returns one finite conservative bound size for fixed subdivision tie-breaking.
fn bounds_size(bounds: Bounds) -> Result<f64, CurveError> {
    let width = bounds.max.x - bounds.min.x;
    let height = bounds.max.y - bounds.min.y;
    let size = width.hypot(height);
    size.is_finite().then_some(size).ok_or(CurveError::new(
        "curve.path.numeric_overflow",
        "curve-path arithmetic must remain finite",
    ))
}

/// Returns a finite coordinate delta from first to second point.
fn delta(first: Point2, second: Point2) -> Result<(f64, f64), CurveError> {
    let x = second.x - first.x;
    let y = second.y - first.y;
    (x.is_finite() && y.is_finite())
        .then_some((x, y))
        .ok_or(CurveError::new(
            "curve.path.numeric_overflow",
            "curve-path arithmetic must remain finite",
        ))
}

/// Returns a finite two-dimensional dot product.
fn dot(first: (f64, f64), second: (f64, f64)) -> f64 {
    first.0.mul_add(second.0, first.1 * second.1)
}

/// Returns a finite two-dimensional cross product under already finite bounded inputs.
fn cross(first: (f64, f64), second: (f64, f64)) -> f64 {
    first.0 * second.1 - first.1 * second.0
}

/// Tracks subdivision work shared by all curve pairs in one public intersection operation.
struct IntersectionBudget {
    work_items: usize,
    maximum_work_items: usize,
}

impl IntersectionBudget {
    /// Creates a private fixed shared work budget for one deterministic intersection query.
    const fn new(maximum_work_items: usize) -> Self {
        Self {
            work_items: 0,
            maximum_work_items,
        }
    }

    /// Charges one parameter-box node and returns the stable atomic subdivision failure on exhaustion.
    fn consume(&mut self) -> Result<(), CurveError> {
        self.work_items = self.work_items.checked_add(1).ok_or(CurveError::new(
            "curve.path.intersections.subdivision_limit",
            "intersection subdivision work limit exceeded",
        ))?;
        (self.work_items <= self.maximum_work_items)
            .then_some(())
            .ok_or(CurveError::new(
                "curve.path.intersections.subdivision_limit",
                "intersection subdivision work limit exceeded",
            ))
    }
}

#[cfg(test)]
mod tests {
    use crate::{CubicBezierSegment, CurveSegment, Point2};

    use super::*;

    /// Proves private reduced subdivision budgets fail before returning any candidate contact.
    #[test]
    fn reduced_intersection_work_budget_is_atomic() {
        let first = CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(0.0, 0.0),
                Point2::new(0.0, 50.0),
                Point2::new(100.0, 50.0),
                Point2::new(100.0, 0.0),
            )
            .expect("finite cubic"),
        );
        let second = CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(0.0, 1.0),
                Point2::new(30.0, -50.0),
                Point2::new(70.0, -50.0),
                Point2::new(100.0, 1.0),
            )
            .expect("finite cubic"),
        );
        assert_eq!(
            subdivision_intersections(first, second, 48, &mut IntersectionBudget::new(1))
                .expect_err("one work item is insufficient")
                .path(),
            "curve.path.intersections.subdivision_limit"
        );
    }
}
