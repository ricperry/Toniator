//! Renderer-independent conversion of variable-width centerlines into filled outlines.

use crate::{
    Bounds, CanonicalFillRule, CubicBezierSegment, CurveError, CurvePath, CurveSegment,
    LineSegment, PathClosure, PathLocation, Point2, Vector2,
};

/// One exact centerline location and document-space width consumed by the outline service.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VariableWidthPathSample {
    /// The immutable addressed centerline location in authored segment order.
    pub location: PathLocation,
    /// The finite nonnegative document-space width at `location`.
    pub width: f64,
}

/// One ordered closed contour in a derived filled outline.
///
/// Its segment storage is independent from the authored `CurvePath` segment bound because an
/// outline is derived render geometry rather than editable construction geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalOutlineContour {
    /// Connected line/cubic outline segments with explicit closure in the final endpoint.
    pub segments: Vec<CurveSegment>,
}

/// Immutable nonzero filled geometry derived from one variable-width centerline.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalFilledOutline {
    /// Ordered independent contours; empty means an all-zero-width stroke.
    pub contours: Vec<CanonicalOutlineContour>,
    /// The fixed semantic required to retain self-overlap without Boolean cleanup.
    pub fill_rule: CanonicalFillRule,
    /// Finite derived bounds, absent only for an empty all-zero outline.
    pub bounds: Option<Bounds>,
}

/// Bounded allocation policy for one reusable variable-width outline request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VariableWidthOutlineLimits {
    maximum_segments: usize,
}

impl VariableWidthOutlineLimits {
    /// Creates a nonzero derived-outline segment bound.
    ///
    /// # Errors
    ///
    /// Returns `curve.outline.segment_limit` when callers disable the required work bound.
    pub fn new(maximum_segments: usize) -> Result<Self, CurveError> {
        if maximum_segments == 0 {
            return Err(CurveError::new(
                "curve.outline.segment_limit",
                "variable-width outline segment limit must be nonzero",
            ));
        }
        Ok(Self { maximum_segments })
    }

    /// Returns the complete derived-outline segment budget.
    pub const fn maximum_segments(self) -> usize {
        self.maximum_segments
    }
}

/// Builds compact nonzero filled contours from exact variable-width path samples.
///
/// Width samples are validated against the supplied authored path, preserve segment boundaries
/// and zero-width run boundaries, and are only simplified by the caller's already adaptive
/// profile. The builder deterministically simplifies center and half-width independently within
/// `tolerance`, while preserving segment boundaries, zero transitions, closed seams, and tangent
/// discontinuities.
/// Open runs receive true cubic round caps, while closed positive paths use opposite-winding
/// rails without endpoint caps. Self-overlap deliberately remains a deterministic nonzero fill.
///
/// # Errors
///
/// Returns stable validation, cancellation, numeric, or segment-limit diagnostics without
/// exposing a partial outline.
#[allow(clippy::too_many_arguments)]
pub fn build_variable_width_outline_cancellable(
    path: &CurvePath,
    samples: &[VariableWidthPathSample],
    _style: toniator_domain::PathStrokeStyle,
    bias: f64,
    tolerance: f64,
    limits: VariableWidthOutlineLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CanonicalFilledOutline, CurveError> {
    if !bias.is_finite() || !(-1.0..=1.0).contains(&bias) {
        return Err(CurveError::new(
            "curve.outline.bias",
            "curve response bias must be finite and within -1.0..=1.0",
        ));
    }
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(CurveError::new(
            "curve.outline.tolerance",
            "variable-width outline tolerance must be positive and finite",
        ));
    }
    if samples.is_empty() {
        return Err(CurveError::new(
            "curve.outline.samples.empty",
            "variable-width outlines require at least one width sample",
        ));
    }
    validate_samples(path, samples)?;
    if samples.iter().all(|sample| sample.width == 0.0) {
        return Ok(CanonicalFilledOutline {
            contours: Vec::new(),
            fill_rule: CanonicalFillRule::NonZero,
            bounds: None,
        });
    }
    let rotated: Vec<VariableWidthPathSample>;
    let samples = if path.closure() == PathClosure::Closed
        && samples[0].width > 0.0
        && samples.last().expect("nonempty samples").width > 0.0
        && samples.iter().any(|sample| sample.width == 0.0)
    {
        let zero = samples
            .iter()
            .position(|sample| sample.width == 0.0)
            .expect("checked zero sample");
        rotated = samples[zero..]
            .iter()
            .chain(samples[..zero].iter())
            .copied()
            .chain(std::iter::once(samples[zero]))
            .collect::<Vec<_>>();
        rotated.as_slice()
    } else {
        samples
    };
    let mut contours = Vec::new();
    let mut remaining_segments = limits.maximum_segments();
    let mut start = 0_usize;
    while start < samples.len() {
        if is_cancelled() {
            return Err(cancelled());
        }
        while start < samples.len() && samples[start].width == 0.0 {
            start += 1;
        }
        if start == samples.len() {
            break;
        }
        let mut end = start + 1;
        while end < samples.len() && samples[end].width > 0.0 {
            end += 1;
        }
        let run_start = if start > 0 && samples[start - 1].width == 0.0 {
            start - 1
        } else {
            start
        };
        let run_end = if end < samples.len() && samples[end].width == 0.0 {
            end + 1
        } else {
            end
        };
        let run = simplify_positive_run(path, &samples[run_start..run_end], tolerance)?;
        if run.len() == 1 {
            contours.push(single_sample_disc(
                path,
                run[0],
                bias,
                &mut remaining_segments,
                is_cancelled,
            )?);
        } else {
            contours.extend(build_positive_run(
                path,
                &run,
                bias,
                path.closure() == PathClosure::Closed && start == 0 && end == samples.len(),
                &mut remaining_segments,
                is_cancelled,
            )?);
        }
        start = end;
    }
    let bounds = Bounds::from_points(
        contours
            .iter()
            .flat_map(|contour| contour.segments.iter().flat_map(outline_segment_points)),
    );
    Ok(CanonicalFilledOutline {
        contours,
        fill_rule: CanonicalFillRule::NonZero,
        bounds,
    })
}

/// Validates finite exact width samples in nondecreasing authored path order.
///
/// # Errors
///
/// Returns an address, width, order, or centerline failure before any derived contour allocation.
fn validate_samples(
    path: &CurvePath,
    samples: &[VariableWidthPathSample],
) -> Result<(), CurveError> {
    let mut previous: Option<PathLocation> = None;
    for sample in samples {
        if !sample.width.is_finite() || sample.width < 0.0 {
            return Err(CurveError::new(
                "curve.outline.width",
                "variable-width outline samples require finite nonnegative widths",
            ));
        }
        path.point_at(sample.location).map_err(|_| {
            CurveError::new(
                "curve.outline.location",
                "variable-width outline samples must address the supplied path",
            )
        })?;
        if let Some(previous) = previous
            && (sample.location.segment_index() < previous.segment_index()
                || (sample.location.segment_index() == previous.segment_index()
                    && sample.location.parameter() < previous.parameter()))
        {
            return Err(CurveError::new(
                "curve.outline.order",
                "variable-width outline samples must follow authored path order",
            ));
        }
        previous = Some(sample.location);
    }
    Ok(())
}

/// Builds a closed circular outline for one positive zero-length run.
///
/// # Errors
///
/// Returns cancellation, tangent, numeric, or segment-limit diagnostics without a partial contour.
fn single_sample_disc(
    path: &CurvePath,
    sample: VariableWidthPathSample,
    bias: f64,
    remaining_segments: &mut usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CanonicalOutlineContour, CurveError> {
    let centerline = path.point_at(sample.location)?;
    let tangent = path.limiting_unit_tangent_at(sample.location)?;
    let normal = tangent.perpendicular();
    let radius = sample.width * 0.5;
    let center = add(centerline, normal.scale(-bias * radius));
    let left = add(center, normal.scale(radius));
    let right = add(center, normal.scale(-radius));
    let mut segments = Vec::with_capacity(4);
    let forward = add(center, tangent.scale(radius));
    let backward = add(center, tangent.scale(-radius));
    push_round_arc(
        &mut segments,
        center,
        left,
        forward,
        true,
        remaining_segments,
    )?;
    if is_cancelled() {
        return Err(cancelled());
    }
    push_round_arc(
        &mut segments,
        center,
        forward,
        right,
        true,
        remaining_segments,
    )?;
    push_round_arc(
        &mut segments,
        center,
        right,
        backward,
        true,
        remaining_segments,
    )?;
    push_round_arc(
        &mut segments,
        center,
        backward,
        left,
        true,
        remaining_segments,
    )?;
    Ok(CanonicalOutlineContour { segments })
}

/// Builds one positive run with cubic rails and true round endpoint caps where required.
///
/// # Errors
///
/// Returns cancellation, tangent, numeric, or segment-limit diagnostics without a partial contour.
fn build_positive_run(
    path: &CurvePath,
    samples: &[VariableWidthPathSample],
    bias: f64,
    closed: bool,
    remaining_segments: &mut usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CanonicalOutlineContour>, CurveError> {
    let vertices = samples
        .iter()
        .map(|sample| outline_vertex(path, *sample, bias))
        .collect::<Result<Vec<_>, _>>()?;
    if closed {
        let left = build_closed_rail(&vertices, true, remaining_segments, is_cancelled)?;
        let right = build_closed_rail(&vertices, false, remaining_segments, is_cancelled)?;
        return Ok(vec![left, right]);
    }
    let mut segments = Vec::new();
    for pair in vertices.windows(2) {
        if is_cancelled() {
            return Err(cancelled());
        }
        push_join_or_rail(&mut segments, pair[0], pair[1], true, remaining_segments)?;
    }
    let last = *vertices.last().expect("two vertices");
    let first = vertices[0];
    push_round_cap(
        &mut segments,
        last.left,
        last.right,
        last.tangent,
        true,
        remaining_segments,
    )?;
    for pair in vertices.windows(2).rev() {
        push_join_or_rail(&mut segments, pair[1], pair[0], false, remaining_segments)?;
    }
    push_round_cap(
        &mut segments,
        first.right,
        first.left,
        first.tangent,
        false,
        remaining_segments,
    )?;
    Ok(vec![CanonicalOutlineContour { segments }])
}

/// Builds one independently closed rail for a positive closed centerline.
///
/// The caller requests the right rail in reverse winding so a closed stroke remains two
/// independent opposite-winding contours instead of a joined composite.
///
/// # Errors
///
/// Returns cancellation, numeric, or segment-limit diagnostics without a partial contour.
fn build_closed_rail(
    vertices: &[OutlineVertex],
    left: bool,
    remaining_segments: &mut usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CanonicalOutlineContour, CurveError> {
    let ordered = if left {
        vertices.to_vec()
    } else {
        vertices.iter().copied().rev().collect()
    };
    let mut segments = Vec::new();
    for pair in ordered.windows(2) {
        if is_cancelled() {
            return Err(cancelled());
        }
        push_join_or_rail(&mut segments, pair[0], pair[1], left, remaining_segments)?;
    }
    let last = *ordered.last().expect("two vertices");
    push_join_or_rail(&mut segments, last, ordered[0], left, remaining_segments)?;
    Ok(CanonicalOutlineContour { segments })
}

/// Simplifies one positive run with independent center and half-width error bounds.
///
/// Segment transitions are mandatory retain points, so explicit seams and tangent discontinuities
/// survive even when their centers coincide. The recursive RDP pass keeps a sample whenever the
/// center chord or half-width interpolation differs by more than `tolerance`.
///
/// # Errors
///
/// Propagates exact centerline evaluation failures without accepting an inferred sample position.
fn simplify_positive_run(
    path: &CurvePath,
    samples: &[VariableWidthPathSample],
    tolerance: f64,
) -> Result<Vec<VariableWidthPathSample>, CurveError> {
    if samples.len() <= 2 {
        return Ok(samples.to_vec());
    }
    let mut keep = vec![false; samples.len()];
    keep[0] = true;
    keep[samples.len() - 1] = true;
    for index in 1..samples.len() - 1 {
        if samples[index - 1].location.segment_index() != samples[index].location.segment_index()
            || samples[index].location.segment_index()
                != samples[index + 1].location.segment_index()
        {
            keep[index] = true;
        }
    }
    let mandatory = keep
        .iter()
        .enumerate()
        .filter_map(|(index, retained)| retained.then_some(index))
        .collect::<Vec<_>>();
    for pair in mandatory.windows(2) {
        retain_simplification(path, samples, pair[0], pair[1], tolerance, &mut keep)?;
    }
    Ok(samples
        .iter()
        .enumerate()
        .filter_map(|(index, sample)| keep[index].then_some(*sample))
        .collect())
}

/// Recursively retains the maximal center or half-width deviation in one stable sample interval.
///
/// # Errors
///
/// Propagates exact path evaluation failures while preserving the no-partial-result outline rule.
fn retain_simplification(
    path: &CurvePath,
    samples: &[VariableWidthPathSample],
    start: usize,
    end: usize,
    tolerance: f64,
    keep: &mut [bool],
) -> Result<(), CurveError> {
    if end <= start + 1 {
        return Ok(());
    }
    let first = path.point_at(samples[start].location)?;
    let last = path.point_at(samples[end].location)?;
    let mut greatest = 0.0_f64;
    let mut retained = None;
    for index in start + 1..end {
        let start_measure = sample_measure(samples[start].location);
        let end_measure = sample_measure(samples[end].location);
        let fraction = if end_measure > start_measure {
            (sample_measure(samples[index].location) - start_measure)
                / (end_measure - start_measure)
        } else {
            0.5
        };
        let expected = Point2::new(
            first.x + (last.x - first.x) * fraction,
            first.y + (last.y - first.y) * fraction,
        );
        let actual = path.point_at(samples[index].location)?;
        let center_error =
            ((actual.x - expected.x).powi(2) + (actual.y - expected.y).powi(2)).sqrt();
        let expected_half_width =
            (samples[start].width + (samples[end].width - samples[start].width) * fraction) * 0.5;
        let width_error = (samples[index].width * 0.5 - expected_half_width).abs();
        let error = center_error.max(width_error);
        if error > greatest {
            greatest = error;
            retained = Some(index);
        }
    }
    if greatest > tolerance {
        let index = retained.expect("nonempty simplification interval");
        keep[index] = true;
        retain_simplification(path, samples, start, index, tolerance, keep)?;
        retain_simplification(path, samples, index, end, tolerance, keep)?;
    }
    Ok(())
}

/// Maps one exact path location to a stable authored-order interpolation coordinate.
fn sample_measure(location: PathLocation) -> f64 {
    location.segment_index() as f64 + location.parameter()
}

/// Resolves one centerline sample into paired rails using its exact edge tangent or cusp limit.
///
/// # Errors
///
/// Propagates exact path location/tangent failures instead of inventing an interior fallback normal.
fn outline_vertex(
    path: &CurvePath,
    sample: VariableWidthPathSample,
    bias: f64,
) -> Result<OutlineVertex, CurveError> {
    let centerline = path.point_at(sample.location)?;
    let tangent = path.limiting_unit_tangent_at(sample.location)?;
    let normal = tangent.perpendicular();
    let half_width = sample.width * 0.5;
    let center = add(centerline, normal.scale(-bias * half_width));
    Ok(OutlineVertex {
        center,
        left: add(center, normal.scale(half_width)),
        right: add(center, normal.scale(-half_width)),
        tangent,
        half_width,
    })
}

/// Adds a rail or the explicit outer round join at a retained tangent discontinuity.
///
/// # Errors
///
/// Returns finite-coordinate or segment-limit errors without leaving a partial contour.
fn push_join_or_rail(
    segments: &mut Vec<CurveSegment>,
    first: OutlineVertex,
    second: OutlineVertex,
    left: bool,
    remaining_segments: &mut usize,
) -> Result<(), CurveError> {
    let start = if left { first.left } else { first.right };
    let end = if left { second.left } else { second.right };
    if start == end {
        return Ok(());
    }
    if (start.x - end.x).hypot(start.y - end.y) <= 1.0e-12 {
        return push_line(segments, start, end, remaining_segments);
    }
    let same_center = first.center == second.center;
    let turn = first.tangent.x * second.tangent.y - first.tangent.y * second.tangent.x;
    // The right rail is traversed in reverse by `build_positive_run`, so `turn` is already in
    // each rail's local ordered direction. Its exterior join is therefore always the local
    // right turn, and its short arc is clockwise; the other rail remains a direct inner join.
    let outer = turn < 0.0;
    if same_center && outer && first.half_width == second.half_width {
        return push_round_arc(segments, first.center, start, end, true, remaining_segments);
    }
    push_rail(segments, start, end, remaining_segments)
}

/// Appends a cubic straight rail so derived contour storage remains uniformly curve-capable.
///
/// # Errors
///
/// Returns the segment-limit or finite-coordinate error before appending a partial contour.
fn push_rail(
    segments: &mut Vec<CurveSegment>,
    start: Point2,
    end: Point2,
    remaining_segments: &mut usize,
) -> Result<(), CurveError> {
    let delta = Vector2::new(end.x - start.x, end.y - start.y);
    push_cubic(
        segments,
        start,
        add(start, delta.scale(1.0 / 3.0)),
        add(start, delta.scale(2.0 / 3.0)),
        end,
        remaining_segments,
    )
}

/// Appends one finite direct rail for a numerically tiny but topologically required join.
///
/// # Errors
///
/// Returns the segment-limit or finite-coordinate diagnostic before mutating the contour.
fn push_line(
    segments: &mut Vec<CurveSegment>,
    start: Point2,
    end: Point2,
    remaining_segments: &mut usize,
) -> Result<(), CurveError> {
    if *remaining_segments == 0 {
        return Err(CurveError::new(
            "curve.outline.segment_limit",
            "variable-width outline exceeds the configured segment limit",
        ));
    }
    segments.push(CurveSegment::Line(LineSegment::new(start, end)?));
    *remaining_segments -= 1;
    Ok(())
}

/// Appends a true two-cubic semicircular endpoint cap.
///
/// # Errors
///
/// Returns a finite-coordinate or segment-limit error before exposing a partial cap.
fn push_round_cap(
    segments: &mut Vec<CurveSegment>,
    start: Point2,
    end: Point2,
    tangent: Vector2,
    forward: bool,
    remaining_segments: &mut usize,
) -> Result<(), CurveError> {
    let radius = ((start.x - end.x).powi(2) + (start.y - end.y).powi(2)).sqrt() * 0.5;
    if radius == 0.0 {
        return Ok(());
    }
    let direction = tangent.scale(if forward { 1.0 } else { -1.0 });
    let center = Point2::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5);
    let midpoint = add(center, direction.scale(radius));
    push_round_arc(segments, center, start, midpoint, true, remaining_segments)?;
    push_round_arc(segments, center, midpoint, end, true, remaining_segments)
}

/// Appends one <=90-degree circular arc as its standard cubic Bézier approximation.
///
/// # Errors
///
/// Returns a segment-limit or finite-coordinate error before mutating the destination contour.
fn push_round_arc(
    segments: &mut Vec<CurveSegment>,
    center: Point2,
    start: Point2,
    end: Point2,
    _clockwise: bool,
    remaining_segments: &mut usize,
) -> Result<(), CurveError> {
    let start_vector = Vector2::new(start.x - center.x, start.y - center.y);
    let end_vector = Vector2::new(end.x - center.x, end.y - center.y);
    let radius = (start_vector.x.powi(2) + start_vector.y.powi(2)).sqrt();
    let start_angle = start_vector.y.atan2(start_vector.x);
    let end_angle = end_vector.y.atan2(end_vector.x);
    let mut sweep = end_angle - start_angle;
    // Every caller represents a cap quarter or an exterior corner no wider than a right angle.
    // Select that shortest geometric sweep defensively instead of permitting a requested winding
    // to turn a local join into an almost-full circle.
    if sweep > std::f64::consts::PI {
        sweep -= std::f64::consts::TAU;
    } else if sweep < -std::f64::consts::PI {
        sweep += std::f64::consts::TAU;
    }
    let parts = (sweep.abs() / (std::f64::consts::FRAC_PI_2))
        .ceil()
        .max(1.0) as usize;
    let step = sweep / parts as f64;
    let mut angle = start_angle;
    for index in 0..parts {
        let next = angle + step;
        let constructed_start = Point2::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        );
        let constructed_end = Point2::new(
            center.x + radius * next.cos(),
            center.y + radius * next.sin(),
        );
        let a = if index == 0 { start } else { constructed_start };
        let b = if index + 1 == parts {
            end
        } else {
            constructed_end
        };
        let handle = 4.0 / 3.0 * (step * 0.25).tan() * radius;
        let tangent_a = Vector2::new(-angle.sin(), angle.cos()).scale(handle);
        let tangent_b = Vector2::new(-next.sin(), next.cos()).scale(handle);
        push_cubic(
            segments,
            a,
            add(a, tangent_a),
            add(b, tangent_b.scale(-1.0)),
            b,
            remaining_segments,
        )?;
        angle = next;
    }
    Ok(())
}

/// Appends one finite cubic while enforcing the complete derived-outline budget.
///
/// # Errors
///
/// Returns a limit or finite-coordinate error before mutating the segment collection.
fn push_cubic(
    segments: &mut Vec<CurveSegment>,
    start: Point2,
    control_1: Point2,
    control_2: Point2,
    end: Point2,
    remaining_segments: &mut usize,
) -> Result<(), CurveError> {
    if *remaining_segments == 0 {
        return Err(CurveError::new(
            "curve.outline.segment_limit",
            "variable-width outline exceeds the configured segment limit",
        ));
    }
    segments.push(CurveSegment::CubicBezier(CubicBezierSegment::new(
        start, control_1, control_2, end,
    )?));
    *remaining_segments -= 1;
    Ok(())
}

/// Returns all construction points retained by one derived contour segment.
fn outline_segment_points(segment: &CurveSegment) -> Vec<Point2> {
    match segment {
        CurveSegment::Line(line) => vec![line.start(), line.end()],
        CurveSegment::CubicBezier(cubic) => vec![
            cubic.start(),
            cubic.control_1(),
            cubic.control_2(),
            cubic.end(),
        ],
    }
}

/// Adds two finite geometric values without hiding numeric overflow.
fn add(point: Point2, vector: Vector2) -> Point2 {
    Point2::new(point.x + vector.x, point.y + vector.y)
}

/// Constructs the shared cancellation diagnostic for outline construction.
fn cancelled() -> CurveError {
    CurveError::new("evaluation.cancelled", "evaluation was cancelled")
}

/// Resolved paired rails and tangent data for one retained outline sample.
#[derive(Clone, Copy)]
struct OutlineVertex {
    center: Point2,
    left: Point2,
    right: Point2,
    tangent: Vector2,
    half_width: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proves either 90-degree exterior corner and an endpoint cap remain compact local arcs.
    #[test]
    fn right_angle_corners_and_round_caps_never_form_full_circle_loops_or_spikes() {
        for end in [Point2::new(0.0, 1.0), Point2::new(0.0, -1.0)] {
            let mut segments = Vec::new();
            let mut remaining = 8;
            push_round_arc(
                &mut segments,
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                end,
                true,
                &mut remaining,
            )
            .expect("right-angle corner arc");
            assert_eq!(segments.len(), 1, "a right angle remains one short cubic");
            assert!(
                outline_segment_points(&segments[0])
                    .into_iter()
                    .all(|point| point.x.abs() <= 1.1 && point.y.abs() <= 1.1)
            );
        }
        let mut cap = Vec::new();
        let mut remaining = 8;
        push_round_cap(
            &mut cap,
            Point2::new(0.0, 1.0),
            Point2::new(0.0, -1.0),
            Vector2::new(1.0, 0.0),
            true,
            &mut remaining,
        )
        .expect("round cap");
        assert_eq!(cap.len(), 2, "a round cap remains two short quarter arcs");
        assert!(
            cap.into_iter()
                .flat_map(|segment| outline_segment_points(&segment))
                .all(|point| point.x.abs() <= 1.1 && point.y.abs() <= 1.1)
        );
    }
}
