use crate::{Bounds, Point2};

use super::{
    CurveError, CurvePath, CurveSegment, LineSegment, MAX_SUBDIVISION_DEPTH, PARAMETER_TOLERANCE,
    PathLocation, check_curve_cancelled, coincident, curve_storage_count, distance, parameter,
    reserve_curve, snap_parameter, tolerance,
};

/// Caller-owned intersection work policy; normal geometry has no creative count ceiling.
/// Numerical predicates and deterministic subdivision depth remain independent authorities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IntersectionWorkLimits {
    maximum_segment_pairs: usize,
    maximum_subdivision_work: usize,
    maximum_results: usize,
}

impl IntersectionWorkLimits {
    /// Validates explicit nonzero pair, shared subdivision and post-dedup result bounds.
    /// # Errors
    /// Rejects zero bounds before any geometry or storage is produced.
    pub fn new(pairs: usize, work: usize, results: usize) -> Result<Self, CurveError> {
        if [pairs, work, results].contains(&0) {
            return Err(CurveError::new(
                "curve.path.intersections.limits",
                "intersection work limits must be nonzero",
            ));
        }
        Ok(Self {
            maximum_segment_pairs: pairs,
            maximum_subdivision_work: work,
            maximum_results: results,
        })
    }
    /// Returns the maximum Cartesian segment-pair count admitted by this explicit policy.
    pub const fn maximum_segment_pairs(self) -> usize {
        self.maximum_segment_pairs
    }
    /// Returns the shared number of parameter boxes admitted across all candidate pairs.
    pub const fn maximum_subdivision_work(self) -> usize {
        self.maximum_subdivision_work
    }
    /// Returns the maximum published contacts after canonical stable deduplication.
    pub const fn maximum_results(self) -> usize {
        self.maximum_results
    }
}

impl Default for IntersectionWorkLimits {
    /// Supplies machine-representable normal work without artistic pair/result ceilings.
    fn default() -> Self {
        Self {
            maximum_segment_pairs: usize::MAX,
            maximum_subdivision_work: usize::MAX,
            maximum_results: usize::MAX,
        }
    }
}

/// Stably orders Copy contacts with checked scratch storage and cancellation in every merge.
/// Equal keys keep the earlier traversal contact, preserving the original dedup winner.
/// # Errors
/// Returns cancellation, addressability or reported allocation failure without publishing results.
fn stable_sort_cancellable<T: Copy>(
    values: &mut Vec<T>,
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CurveError> {
    check_curve_cancelled(cancelled)?;
    if values.len() < 2 {
        return Ok(());
    }
    let mut scratch = Vec::new();
    reserve_curve(&mut scratch, values.len())?;
    for value in values.iter().copied() {
        check_curve_cancelled(cancelled)?;
        scratch.push(value);
    }
    let len = values.len();
    let mut width = 1;
    while width < len {
        let mut base = 0;
        while base < len {
            let middle = base.saturating_add(width).min(len);
            let end = middle.saturating_add(width).min(len);
            let (mut left, mut right) = (base, middle);
            for slot in &mut scratch[base..end] {
                check_curve_cancelled(cancelled)?;
                if left < middle
                    && (right >= end || !compare(&values[left], &values[right]).is_gt())
                {
                    *slot = values[left];
                    left += 1;
                } else {
                    *slot = values[right];
                    right += 1;
                }
            }
            base = end;
        }
        std::mem::swap(values, &mut scratch);
        width = width.checked_mul(2).unwrap_or(len);
    }
    check_curve_cancelled(cancelled)
}

/// Keeps the first adjacent equivalent contact with cancellable in-place compaction.
/// # Errors
/// Returns cancellation before locally staged contacts can be published.
fn deduplicate_cancellable<T: Copy>(
    values: &mut Vec<T>,
    equivalent: impl Fn(&T, &T) -> bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CurveError> {
    let mut written = 0;
    for index in 0..values.len() {
        check_curve_cancelled(cancelled)?;
        let value = values[index];
        if written == 0 || !equivalent(&values[written - 1], &value) {
            values[written] = value;
            written += 1;
        }
    }
    values.truncate(written);
    check_curve_cancelled(cancelled)
}

/// Stores one bounded analytic contact through the same checked production reserve boundary.
/// # Errors
/// Returns addressability or actual allocation failure before the contact is appended.
fn single_contact(contact: SegmentIntersection) -> Result<Vec<SegmentIntersection>, CurveError> {
    let mut values = Vec::new();
    reserve_curve(&mut values, 1)?;
    values.push(contact);
    Ok(values)
}

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

/// Intersects two segments with normal count policy and unchanged numerical depth/predicates.
/// # Errors
/// Returns numeric/overlap, addressability, allocation or deterministic depth diagnostics atomically.
pub(crate) fn segment_intersections(
    first: CurveSegment,
    second: CurveSegment,
) -> Result<Vec<SegmentIntersection>, CurveError> {
    let mut budget = IntersectionBudget::new(usize::MAX, &|| false);
    let mut intersections = segment_intersections_with_budget(first, second, &mut budget)?;
    sort_and_deduplicate(&mut intersections, budget.cancelled)?;
    Ok(intersections)
}

/// Intersects segments while charging caller-owned shared subdivision work and cancellation.
/// # Errors
/// Returns cancellation or original geometric diagnostics without partial contact publication.
fn segment_intersections_with_budget(
    first: CurveSegment,
    second: CurveSegment,
    budget: &mut IntersectionBudget<'_>,
) -> Result<Vec<SegmentIntersection>, CurveError> {
    budget.check()?;
    let result = match (first, second) {
        (CurveSegment::Line(first), CurveSegment::Line(second)) => line_line(first, second),
        _ => subdivision_intersections(first, second, MAX_SUBDIVISION_DEPTH, budget),
    }?;
    budget.check()?;
    Ok(result)
}

/// Collects finite segment bounds with exact reached storage and cancellation before each append.
/// # Errors
/// Returns cancellation, original coordinate, addressability or reported allocation diagnostics.
fn path_bounds(path: &CurvePath, cancelled: &dyn Fn() -> bool) -> Result<Vec<Bounds>, CurveError> {
    curve_storage_count::<Bounds>(path.segments().len())?;
    let mut bounds = Vec::new();
    for segment in path.segments() {
        check_curve_cancelled(cancelled)?;
        let bound = segment.bounds()?;
        reserve_curve(&mut bounds, 1)?;
        bounds.push(bound);
    }
    Ok(bounds)
}

/// Streams original-order segment pairs, rejects conservative disjoint bounds and canonicalizes contacts.
/// Pair policy counts Cartesian pairs; expensive subdivision only sees overlapping finite boxes.
/// Results are stably sorted/deduplicated before finite result admission and final cancellation.
/// # Errors
/// Returns explicit policy, cancellation, count/addressability, allocation or unchanged numerical errors.
pub(crate) fn path_intersections(
    first: &CurvePath,
    second: &CurvePath,
    limits: IntersectionWorkLimits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<PathIntersection>, CurveError> {
    check_curve_cancelled(cancelled)?;
    let pairs = first
        .segments()
        .len()
        .checked_mul(second.segments().len())
        .ok_or(CurveError::new(
            "curve.path.count_overflow",
            "intersection pair count exceeds machine representation",
        ))?;
    if pairs > limits.maximum_segment_pairs {
        return Err(CurveError::new(
            "curve.path.intersections.pair_limit",
            "path intersection pair limit exceeded",
        ));
    }
    let first_bounds = path_bounds(first, cancelled)?;
    let second_bounds = path_bounds(second, cancelled)?;
    let mut output = Vec::new();
    let mut budget = IntersectionBudget::new(limits.maximum_subdivision_work, cancelled);
    for (first_index, first_segment) in first.segments().iter().copied().enumerate() {
        for (second_index, second_segment) in second.segments().iter().copied().enumerate() {
            budget.check()?;
            if !bounds_overlap(first_bounds[first_index], second_bounds[second_index])? {
                continue;
            }
            for intersection in
                segment_intersections_with_budget(first_segment, second_segment, &mut budget)?
            {
                budget.check()?;
                let contact = PathIntersection {
                    first_location: canonical_location(
                        first,
                        first_index,
                        intersection.first_parameter(),
                        cancelled,
                    )?,
                    second_location: canonical_location(
                        second,
                        second_index,
                        intersection.second_parameter(),
                        cancelled,
                    )?,
                    point: intersection.point(),
                    kind: intersection.kind(),
                };
                reserve_curve(&mut output, 1)?;
                output.push(contact);
            }
        }
    }
    stable_sort_cancellable(&mut output, path_intersection_order, cancelled)?;
    deduplicate_cancellable(
        &mut output,
        |a, b| {
            locations_equivalent(a.first_location, b.first_location)
                && locations_equivalent(a.second_location, b.second_location)
                && coincident(a.point, b.point).unwrap_or(false)
        },
        cancelled,
    )?;
    if output.len() > limits.maximum_results {
        return Err(CurveError::new(
            "curve.path.intersections.result_limit",
            "intersection result limit exceeded",
        ));
    }
    budget.check()?;
    Ok(output)
}

/// Converts equivalent adjacent endpoint visits into the earliest stored topological location.
/// # Errors
/// Returns cancellation or invalid canonical parameter diagnostics; no storage is allocated.
fn canonical_location(
    path: &CurvePath,
    index: usize,
    parameter_value: f64,
    cancelled: &dyn Fn() -> bool,
) -> Result<PathLocation, CurveError> {
    check_curve_cancelled(cancelled)?;
    let parameter_value = snap_parameter(parameter_value);
    if parameter_value == 0.0 && index > 0 {
        if path.segments()[index].start() == path.segments()[index].end() {
            let mut earliest = index;
            while earliest > 0
                && path.segments()[earliest - 1].start() == path.segments()[earliest - 1].end()
                && path.segments()[earliest - 1].end() == path.segments()[earliest].start()
            {
                check_curve_cancelled(cancelled)?;
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

/// Stably sorts segment contacts and retains the original first equivalent parameter/point witness.
/// # Errors
/// Returns cancellation or checked scratch-allocation failure before publication.
fn sort_and_deduplicate(
    intersections: &mut Vec<SegmentIntersection>,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CurveError> {
    stable_sort_cancellable(
        intersections,
        |a, b| {
            a.first_parameter()
                .total_cmp(&b.first_parameter())
                .then_with(|| a.second_parameter().total_cmp(&b.second_parameter()))
        },
        cancelled,
    )?;
    deduplicate_cancellable(
        intersections,
        |a, b| {
            (a.first_parameter() - b.first_parameter()).abs() <= PARAMETER_TOLERANCE
                && (a.second_parameter() - b.second_parameter()).abs() <= PARAMETER_TOLERANCE
                && coincident(a.point(), b.point()).unwrap_or(false)
        },
        cancelled,
    )
}

/// Uses analytical cross products for finite line/line and degenerate point contacts.
/// # Errors
/// Returns original numeric/positive-overlap diagnostics or checked contact storage failure.
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
            single_contact(SegmentIntersection::new(
                0.0,
                0.0,
                first.start(),
                IntersectionKind::Tangent,
            )?)
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
    single_contact(SegmentIntersection::new(
        first_parameter,
        second_parameter,
        point,
        kind,
    )?)
}

/// Resolves a degenerate point against a nondegenerate line as one tangency when coincident.
/// # Errors
/// Returns original coordinate/parameter diagnostics or checked contact storage failure.
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
    single_contact(if point_is_first {
        SegmentIntersection::new(0.0, parameter_value, point, IntersectionKind::Tangent)?
    } else {
        SegmentIntersection::new(parameter_value, 0.0, point, IntersectionKind::Tangent)?
    })
}

/// Distinguishes positive collinear overlap from one endpoint tangent contact.
/// # Errors
/// Returns original coordinate/positive-overlap diagnostics or checked contact storage failure.
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
    single_contact(SegmentIntersection::new(
        first_parameter,
        second_parameter,
        point,
        IntersectionKind::Tangent,
    )?)
}

/// Subdivides parameter boxes in fixed order for every pair involving a cubic.
/// # Errors
/// Returns cancellation, explicit shared-work, unchanged numeric/depth or checked storage failure.
fn subdivision_intersections(
    first: CurveSegment,
    second: CurveSegment,
    maximum_depth: u8,
    budget: &mut IntersectionBudget<'_>,
) -> Result<Vec<SegmentIntersection>, CurveError> {
    if coincident_curve_interval(first, second)? {
        return Err(CurveError::new(
            "curve.path.intersections.overlap",
            "positive-length coincident curve intervals are not discrete intersections",
        ));
    }
    budget.check()?;
    let mut stack = Vec::new();
    reserve_curve(&mut stack, 1)?;
    stack.push((first, 0.0, 1.0, second, 0.0, 1.0, 0_u8));
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
                    refine_candidate(first, second, first_parameter, second_parameter, budget)?
                else {
                    continue;
                };
                let kind = classify_contact(first, second, first_parameter, second_parameter)?;
                budget.check()?;
                reserve_curve(&mut output, 1)?;
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
        budget.check()?;
        reserve_curve(&mut stack, 2)?;
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
/// # Errors
/// Returns cancellation or unchanged numeric/refinement failure without accepting an approximate contact.
fn refine_candidate(
    first: CurveSegment,
    second: CurveSegment,
    mut first_parameter: f64,
    mut second_parameter: f64,
    budget: &IntersectionBudget<'_>,
) -> Result<Option<(f64, f64, Point2)>, CurveError> {
    for _ in 0..12 {
        budget.check()?;
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
/// # Errors
/// Returns the original nonfinite-coordinate or tolerance arithmetic diagnostic.
fn operation_tolerance(
    first: CurveSegment,
    second: CurveSegment,
    extra: &[Point2],
) -> Result<f64, CurveError> {
    tolerance(
        construction_points(first)
            .chain(construction_points(second))
            .chain(extra.iter().copied()),
    )
}

/// Iterates complete construction points in the original tolerance order without allocation.
fn construction_points(segment: CurveSegment) -> impl Iterator<Item = Point2> {
    let (points, count) = match segment {
        CurveSegment::Line(line) => ([line.start(), line.end(), line.end(), line.end()], 2),
        CurveSegment::CubicBezier(cubic) => (
            [
                cubic.start(),
                cubic.control_1(),
                cubic.control_2(),
                cubic.end(),
            ],
            4,
        ),
    };
    points.into_iter().take(count)
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
struct IntersectionBudget<'a> {
    cancelled: &'a dyn Fn() -> bool,
    work_items: usize,
    maximum_work_items: usize,
}

impl<'a> IntersectionBudget<'a> {
    /// Creates an explicit shared work/cancellation budget for one deterministic intersection query.
    const fn new(maximum_work_items: usize, cancelled: &'a dyn Fn() -> bool) -> Self {
        Self {
            cancelled,
            work_items: 0,
            maximum_work_items,
        }
    }

    /// Rejects obsolete work before any further kernel or storage operation.
    /// # Errors
    /// Returns evaluation.cancelled without exposing staged contacts.
    fn check(&self) -> Result<(), CurveError> {
        check_curve_cancelled(self.cancelled)
    }

    /// Charges one parameter-box node without permitting counter wrap or obsolete work.
    /// # Errors
    /// Returns cancellation, count overflow or the explicit shared-work limit diagnostic.
    fn consume(&mut self) -> Result<(), CurveError> {
        self.check()?;
        self.work_items = self.work_items.checked_add(1).ok_or(CurveError::new(
            "curve.path.count_overflow",
            "intersection subdivision work count exceeds machine representation",
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

    /// Exercises the actual reserve helper and distinguishes addressability from allocator refusal.
    /// # Panics
    /// Panics if production storage masks a report, exposes partial output or cannot recover.
    #[test]
    fn production_intersection_reserve_failure_is_atomic_and_distinct() {
        let first = CurvePath::line(Point2::new(-1.0, 0.0), Point2::new(1.0, 0.0)).unwrap();
        let second = CurvePath::line(Point2::new(0.0, -1.0), Point2::new(0.0, 1.0)).unwrap();
        super::super::FAIL_RESERVATION.with(|flag| flag.set(true));
        assert_eq!(
            first.intersections(&second).unwrap_err().path(),
            "curve.path.allocation"
        );
        assert_eq!(first.intersections(&second).unwrap().len(), 1);
        let mut storage = Vec::<PathIntersection>::new();
        assert_eq!(
            reserve_curve(&mut storage, usize::MAX).unwrap_err().path(),
            "curve.path.count_overflow"
        );
        assert!(storage.is_empty());
    }

    /// Preserves stable equal-key winners and cancels actual merge and dedup helper loops.
    /// # Panics
    /// Panics on reordered ties, lost cancellation or a nonstable production sort.
    #[test]
    fn intersection_stable_sort_and_dedup_keep_first_and_cancel() {
        use std::cell::Cell;
        let original: Vec<_> = (0..128).rev().map(|i| (i % 7, i)).collect();
        let mut expected = original.clone();
        expected.sort_by_key(|value| value.0);
        let mut values = original.clone();
        stable_sort_cancellable(&mut values, |a, b| a.0.cmp(&b.0), &|| false).unwrap();
        assert_eq!(values, expected);
        let mut first = values.clone();
        deduplicate_cancellable(&mut first, |a, b| a.0 == b.0, &|| false).unwrap();
        let mut expected_first = expected;
        expected_first.dedup_by_key(|value| value.0);
        assert_eq!(first, expected_first);
        let polls = Cell::new(0);
        let mut values = original;
        assert_eq!(
            stable_sort_cancellable(&mut values, |a, b| a.0.cmp(&b.0), &|| {
                polls.set(polls.get() + 1);
                polls.get() > 140
            })
            .unwrap_err()
            .path(),
            "evaluation.cancelled"
        );
        let polls = Cell::new(0);
        assert_eq!(
            deduplicate_cancellable(&mut values, |a, b| a.0 == b.0, &|| {
                polls.set(polls.get() + 1);
                polls.get() > 10
            })
            .unwrap_err()
            .path(),
            "evaluation.cancelled"
        );
    }

    /// Reuses an observed actual subdivision count as an exact explicit finite boundary.
    /// # Panics
    /// Panics if the same shared budget succeeds below its consumed work or changes contacts.
    #[test]
    fn intersection_shared_subdivision_work_has_exact_boundary() {
        let first = CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(-1.0, 0.0),
                Point2::new(-1.0, 2.0),
                Point2::new(1.0, 2.0),
                Point2::new(1.0, 0.0),
            )
            .unwrap(),
        );
        let second = CurveSegment::Line(
            LineSegment::new(Point2::new(-2.0, 0.75), Point2::new(2.0, 0.75)).unwrap(),
        );
        let mut budget = IntersectionBudget::new(usize::MAX, &|| false);
        let expected = segment_intersections_with_budget(first, second, &mut budget).unwrap();
        let consumed = budget.work_items;
        assert!(consumed > 1);
        assert_eq!(
            segment_intersections_with_budget(
                first,
                second,
                &mut IntersectionBudget::new(consumed, &|| false)
            )
            .unwrap(),
            expected
        );
        assert_eq!(
            segment_intersections_with_budget(
                first,
                second,
                &mut IntersectionBudget::new(consumed - 1, &|| false)
            )
            .unwrap_err()
            .path(),
            "curve.path.intersections.subdivision_limit"
        );
    }

    /// Proves private reduced subdivision budgets fail before returning any candidate contact.
    /// # Panics
    /// Panics if the explicit shared work bound admits partial contacts.
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
            subdivision_intersections(
                first,
                second,
                48,
                &mut IntersectionBudget::new(1, &|| false)
            )
            .expect_err("one work item is insufficient")
            .path(),
            "curve.path.intersections.subdivision_limit"
        );
    }
}
