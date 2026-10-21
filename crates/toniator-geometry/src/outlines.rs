//! Renderer-independent conversion of variable-width centerlines into filled outlines.

use crate::curves::{check_curve_cancelled, reserve_curve};
use crate::{ConstructionContext, ConstructionCounter, MaterializationCoverage};
use std::sync::Arc;

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
    /// Complete original procedural command count, independent of certified segment omission.
    pub full_segment_count: Option<usize>,
    /// Complete globally compacted, source-free preparation for compatible reuse or full fallback.
    pub prepared: Option<Arc<PreparedVariableWidthOutline>>,
    /// Names the canvas contract of any omitted exterior materialization.
    pub coverage: MaterializationCoverage,
    /// Ordered independent contours; empty means an all-zero-width stroke.
    pub contours: Vec<CanonicalOutlineContour>,
    /// The fixed semantic required to retain self-overlap without Boolean cleanup.
    pub fill_rule: CanonicalFillRule,
    /// Finite derived bounds, absent only for an empty all-zero outline.
    pub bounds: Option<Bounds>,
}

impl CanonicalFilledOutline {
    /// Charges complete original procedural commands while retaining actual storage as a lower bound.
    /// # Errors
    /// Returns the original segment-limit authority if stored contour accounting overflows.
    pub fn logical_segment_count(&self) -> Result<usize, CurveError> {
        let actual = self
            .contours
            .iter()
            .try_fold(0_usize, |total, contour| {
                total.checked_add(contour.segments.len())
            })
            .ok_or(CurveError::new(
                "curve.outline.segment_limit",
                "outline segment accounting overflows",
            ))?;
        Ok(self.full_segment_count.unwrap_or(actual).max(actual))
    }
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
/// profile. The builder simplifies centers within `tolerance` and each positive sampled
/// half-width within the smaller of `tolerance` or 2% of that half-width. Segment boundaries,
/// zero transitions, closed seams, and tangent discontinuities remain explicit.
/// Open runs receive true cubic round caps, while closed positive paths use opposite-winding
/// rails without endpoint caps. Self-overlap deliberately remains a deterministic nonzero fill.
///
/// # Errors
///
/// Returns stable validation, cancellation, allocation, numeric, or segment-limit diagnostics without
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
    validate_samples(path, samples, is_cancelled)?;
    let mut first_zero = None;
    let mut all_zero = true;
    for (index, sample) in samples.iter().enumerate() {
        check_curve_cancelled(is_cancelled)?;
        if sample.width == 0.0 {
            if first_zero.is_none() {
                first_zero = Some(index);
            }
        } else {
            all_zero = false;
        }
    }
    if all_zero {
        return Ok(CanonicalFilledOutline {
            full_segment_count: Some(0),
            prepared: None,
            coverage: MaterializationCoverage::Complete,
            contours: Vec::new(),
            fill_rule: CanonicalFillRule::NonZero,
            bounds: None,
        });
    }
    let mut rotated: Vec<VariableWidthPathSample> = Vec::new();
    let samples = if path.closure() == PathClosure::Closed
        && samples[0].width > 0.0
        && samples.last().expect("nonempty samples").width > 0.0
        && let Some(zero) = first_zero
    {
        for sample in samples[zero..]
            .iter()
            .chain(samples[..zero].iter())
            .copied()
            .chain(std::iter::once(samples[zero]))
        {
            check_curve_cancelled(is_cancelled)?;
            reserve_curve(&mut rotated, 1)?;
            rotated.push(sample);
        }
        rotated.as_slice()
    } else {
        samples
    };
    let mut runs = Vec::new();
    let mut start = 0_usize;
    while start < samples.len() {
        if is_cancelled() {
            return Err(cancelled());
        }
        while start < samples.len() && samples[start].width == 0.0 {
            check_curve_cancelled(is_cancelled)?;
            start += 1;
        }
        if start == samples.len() {
            break;
        }
        let mut end = start + 1;
        while end < samples.len() && samples[end].width > 0.0 {
            check_curve_cancelled(is_cancelled)?;
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
        let run =
            simplify_positive_run(path, &samples[run_start..run_end], tolerance, is_cancelled)?;
        let mut vertices = Vec::new();
        for sample in &run {
            check_curve_cancelled(is_cancelled)?;
            reserve_curve(&mut vertices, 1)?;
            vertices.push(outline_vertex(path, *sample, bias)?);
        }
        reserve_curve(&mut runs, 1)?;
        runs.push(PreparedOutlineRun {
            vertices,
            closed: path.closure() == PathClosure::Closed && start == 0 && end == samples.len(),
        });
        start = end;
    }
    let context = ConstructionContext::current();
    context.count_storage(ConstructionCounter::PreparedOutlineBytes, &runs);
    for run in &runs {
        context.count_storage(ConstructionCounter::PreparedOutlineBytes, &run.vertices);
    }
    let prepared = Arc::new(PreparedVariableWidthOutline { runs });
    materialize_prepared_outline_cancellable(
        prepared,
        ConstructionContext::current().coverage(),
        limits,
        is_cancelled,
    )
}

/// Validates finite exact width samples in nondecreasing authored path order.
///
/// # Errors
///
/// Returns an address, width, order, or centerline failure before any derived contour allocation.
fn validate_samples(
    path: &CurvePath,
    samples: &[VariableWidthPathSample],
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), CurveError> {
    let mut previous: Option<PathLocation> = None;
    for sample in samples {
        check_curve_cancelled(is_cancelled)?;
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

/// Immutable complete compacted vertices; materialization never samples artwork or changes runs.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedVariableWidthOutline {
    runs: Vec<PreparedOutlineRun>,
}
/// One original zero-separated positive run, including its unchanged closed seam authority.
#[derive(Clone, Debug, PartialEq)]
struct PreparedOutlineRun {
    vertices: Vec<OutlineVertex>,
    closed: bool,
}

/// Materializes complete prepared runs without changing original primitive arithmetic.
/// # Errors
/// Returns cancellation, numeric, allocation or explicit segment-policy failures atomically.
pub fn materialize_prepared_outline_cancellable(
    prepared: Arc<PreparedVariableWidthOutline>,
    coverage: MaterializationCoverage,
    limits: VariableWidthOutlineLimits,
    cancelled: &dyn Fn() -> bool,
) -> Result<CanonicalFilledOutline, CurveError> {
    let context = ConstructionContext::current();
    let mut endpoints = EndpointScan(None);
    let mut full_segment_count = None;
    if !matches!(coverage, MaterializationCoverage::Complete) {
        let mut ignored = limits.maximum_segments();
        for run in &prepared.runs {
            for side in 0..if run.closed && run.vertices.len() > 1 {
                2
            } else {
                1
            } {
                check_curve_cancelled(cancelled)?;
                emit_prepared_run(run, side, &mut endpoints, &mut ignored, cancelled)?;
            }
        }
        full_segment_count = Some(limits.maximum_segments() - ignored);
    }
    let mut contours = Vec::new();
    let mut any_pruned = false;
    let mut remaining = limits.maximum_segments();
    for run in &prepared.runs {
        check_curve_cancelled(cancelled)?;
        for side in 0..if run.closed && run.vertices.len() > 1 {
            2
        } else {
            1
        } {
            let mut sink = MaterializingSink {
                segments: Vec::new(),
                coverage,
                endpoints: endpoints.0,
                pending: None,
                pruned: false,
                context: context.clone(),
            };
            emit_prepared_run(run, side, &mut sink, &mut remaining, cancelled)?;
            sink.flush(&mut remaining)?;
            any_pruned |= sink.pruned;
            if !sink.segments.is_empty() {
                reserve_curve(&mut contours, 1)?;
                contours.push(CanonicalOutlineContour {
                    segments: sink.segments,
                });
            }
        }
    }
    let mut extrema: Option<(Point2, Point2)> = None;
    for contour in &contours {
        for segment in &contour.segments {
            check_curve_cancelled(cancelled)?;
            for point in outline_segment_points(segment) {
                if let Some((min, max)) = &mut extrema {
                    min.x = min.x.min(point.x);
                    min.y = min.y.min(point.y);
                    max.x = max.x.max(point.x);
                    max.y = max.y.max(point.y);
                } else {
                    extrema = Some((point, point));
                }
            }
        }
    }
    context.count_storage(ConstructionCounter::OutlineIndexBytes, &contours);
    Ok(CanonicalFilledOutline {
        full_segment_count: Some(
            full_segment_count.unwrap_or(limits.maximum_segments() - remaining),
        ),
        contours,
        fill_rule: CanonicalFillRule::NonZero,
        bounds: extrema.and_then(|(min, max)| Bounds::new(min, max)),
        prepared: Some(prepared),
        coverage: if any_pruned {
            coverage
        } else {
            MaterializationCoverage::Complete
        },
    })
}
/// Emits one original contour from globally prepared vertices, preserving caps, seams and winding.
/// # Errors
/// Propagates cancellation and original primitive-construction diagnostics.
fn emit_prepared_run(
    run: &PreparedOutlineRun,
    side: usize,
    segments: &mut impl OutlineEmitter,
    remaining: &mut usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CurveError> {
    let v = &run.vertices;
    if v.len() == 1 {
        let p = v[0];
        let forward = add(p.center, p.tangent.scale(p.half_width));
        let backward = add(p.center, p.tangent.scale(-p.half_width));
        for (a, b) in [
            (p.left, forward),
            (forward, p.right),
            (p.right, backward),
            (backward, p.left),
        ] {
            check_curve_cancelled(cancelled)?;
            push_round_arc(segments, p.center, a, b, true, remaining)?;
        }
    } else if run.closed {
        if side == 0 {
            for pair in v.windows(2) {
                check_curve_cancelled(cancelled)?;
                push_join_or_rail(segments, pair[0], pair[1], true, remaining)?;
            }
            push_join_or_rail(segments, v[v.len() - 1], v[0], true, remaining)?;
        } else {
            for i in (1..v.len()).rev() {
                check_curve_cancelled(cancelled)?;
                push_join_or_rail(segments, v[i], v[i - 1], false, remaining)?;
            }
            push_join_or_rail(segments, v[0], v[v.len() - 1], false, remaining)?;
        }
    } else {
        for pair in v.windows(2) {
            check_curve_cancelled(cancelled)?;
            push_join_or_rail(segments, pair[0], pair[1], true, remaining)?;
        }
        let last = v[v.len() - 1];
        let first = v[0];
        push_round_cap(
            segments,
            last.left,
            last.right,
            last.tangent,
            true,
            remaining,
        )?;
        for pair in v.windows(2).rev() {
            check_curve_cancelled(cancelled)?;
            push_join_or_rail(segments, pair[1], pair[0], false, remaining)?;
        }
        push_round_cap(
            segments,
            first.right,
            first.left,
            first.tangent,
            false,
            remaining,
        )?;
    }
    Ok(())
}

/// Builds a closed circular outline for one positive zero-length run.
///
/// # Errors
///
/// Returns cancellation, tangent, numeric, or segment-limit diagnostics without a partial contour.
#[allow(dead_code)]
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
    let mut segments = Vec::new();
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
#[allow(dead_code)]
fn build_positive_run(
    path: &CurvePath,
    samples: &[VariableWidthPathSample],
    bias: f64,
    closed: bool,
    remaining_segments: &mut usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CanonicalOutlineContour>, CurveError> {
    let mut vertices = Vec::new();
    for sample in samples {
        check_curve_cancelled(is_cancelled)?;
        reserve_curve(&mut vertices, 1)?;
        vertices.push(outline_vertex(path, *sample, bias)?);
    }
    if closed {
        let left = build_closed_rail(&vertices, true, remaining_segments, is_cancelled)?;
        let right = build_closed_rail(&vertices, false, remaining_segments, is_cancelled)?;
        let mut contours = Vec::new();
        reserve_curve(&mut contours, 2)?;
        contours.push(left);
        contours.push(right);
        return Ok(contours);
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
        check_curve_cancelled(is_cancelled)?;
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
    let mut contours = Vec::new();
    reserve_curve(&mut contours, 1)?;
    contours.push(CanonicalOutlineContour { segments });
    Ok(contours)
}

/// Builds one independently closed rail for a positive closed centerline.
///
/// The caller requests the right rail in reverse winding so a closed stroke remains two
/// independent opposite-winding contours instead of a joined composite.
///
/// # Errors
///
/// Returns cancellation, numeric, or segment-limit diagnostics without a partial contour.
#[allow(dead_code)]
fn build_closed_rail(
    vertices: &[OutlineVertex],
    left: bool,
    remaining_segments: &mut usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CanonicalOutlineContour, CurveError> {
    let mut ordered = Vec::new();
    for index in 0..vertices.len() {
        check_curve_cancelled(is_cancelled)?;
        reserve_curve(&mut ordered, 1)?;
        ordered.push(
            vertices[if left {
                index
            } else {
                vertices.len() - 1 - index
            }],
        );
    }
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

/// Simplifies one positive run with independent absolute center and local relative width bounds.
///
/// Segment transitions, zero run endpoints, and tangent discontinuities remain mandatory. The
/// iterative pass keeps the largest normalized violation until both bounds hold.
///
/// # Errors
///
/// Propagates cancellation, allocation, or exact centerline failures without a partial outline.
fn simplify_positive_run(
    path: &CurvePath,
    samples: &[VariableWidthPathSample],
    tolerance: f64,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<VariableWidthPathSample>, CurveError> {
    let mut keep = Vec::new();
    for _ in samples {
        check_curve_cancelled(is_cancelled)?;
        reserve_curve(&mut keep, 1)?;
        keep.push(false);
    }
    keep[0] = true;
    keep[samples.len() - 1] = true;
    for index in 1..samples.len() - 1 {
        if index & 63 == 0 && is_cancelled() {
            return Err(cancelled());
        }
        if samples[index - 1].location.segment_index() != samples[index].location.segment_index()
            || samples[index].location.segment_index()
                != samples[index + 1].location.segment_index()
        {
            keep[index] = true;
        }
    }
    let mut stack = Vec::new();
    let mut previous = 0;
    for index in 1..samples.len() {
        if index == samples.len() - 1
            || samples[index - 1].location.segment_index()
                != samples[index].location.segment_index()
            || samples[index].location.segment_index()
                != samples[index + 1].location.segment_index()
        {
            retain_simplification(
                path,
                samples,
                previous,
                index,
                tolerance,
                &mut keep,
                &mut stack,
                is_cancelled,
            )?;
            previous = index;
        }
    }
    let mut retained = Vec::new();
    for (index, sample) in samples.iter().enumerate() {
        check_curve_cancelled(is_cancelled)?;
        if keep[index] {
            reserve_curve(&mut retained, 1)?;
            retained.push(*sample);
        }
    }
    Ok(retained)
}

/// Iteratively retains the maximal normalized center or half-width violation in stable intervals.
///
/// # Errors
///
/// Propagates cancellation, allocation, or exact path failures without publishing partial geometry.
#[allow(clippy::too_many_arguments)]
fn retain_simplification(
    path: &CurvePath,
    samples: &[VariableWidthPathSample],
    start: usize,
    end: usize,
    tolerance: f64,
    keep: &mut [bool],
    stack: &mut Vec<(usize, usize)>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), CurveError> {
    stack
        .try_reserve(1)
        .map_err(|_| simplification_allocation_error())?;
    stack.push((start, end));
    while let Some((start, end)) = stack.pop() {
        if is_cancelled() {
            return Err(cancelled());
        }
        if end <= start + 1 {
            continue;
        }
        let first = path.point_at(samples[start].location)?;
        let last = path.point_at(samples[end].location)?;
        let start_measure = sample_measure(samples[start].location);
        let end_measure = sample_measure(samples[end].location);
        let mut greatest = 1.0_f64;
        let mut retained = None;
        for index in start + 1..end {
            if index & 63 == 0 && is_cancelled() {
                return Err(cancelled());
            }
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
            let expected_half_width = (samples[start].width
                + (samples[end].width - samples[start].width) * fraction)
                * 0.5;
            let width_error = (samples[index].width * 0.5 - expected_half_width).abs();
            let width_bound = tolerance.min(0.02 * (samples[index].width * 0.5));
            let width_violation = if width_bound == 0.0 {
                if width_error == 0.0 {
                    0.0
                } else {
                    f64::INFINITY
                }
            } else {
                width_error / width_bound
            };
            let violation = (center_error / tolerance).max(width_violation);
            if violation > greatest {
                greatest = violation;
                retained = Some(index);
            }
        }
        if let Some(index) = retained {
            keep[index] = true;
            stack
                .try_reserve(2)
                .map_err(|_| simplification_allocation_error())?;
            stack.push((index, end));
            stack.push((start, index));
        }
    }
    Ok(())
}

/// Returns the stable allocation failure for a bounded outline simplification request.
fn simplification_allocation_error() -> CurveError {
    CurveError::new(
        "curve.outline.allocation",
        "variable-width outline simplification allocation failed",
    )
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
    segments: &mut impl OutlineEmitter,
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
    segments: &mut impl OutlineEmitter,
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
    segments: &mut impl OutlineEmitter,
    start: Point2,
    end: Point2,
    remaining_segments: &mut usize,
) -> Result<(), CurveError> {
    segments.emit(OutlineCommand::Line(start, end), remaining_segments)
}

/// Appends a true two-cubic semicircular endpoint cap.
///
/// # Errors
///
/// Returns a finite-coordinate or segment-limit error before exposing a partial cap.
fn push_round_cap(
    segments: &mut impl OutlineEmitter,
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
    segments: &mut impl OutlineEmitter,
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
    segments: &mut impl OutlineEmitter,
    start: Point2,
    control_1: Point2,
    control_2: Point2,
    end: Point2,
    remaining_segments: &mut usize,
) -> Result<(), CurveError> {
    segments.emit(
        OutlineCommand::Cubic(start, control_1, control_2, end),
        remaining_segments,
    )
}

/// Returns all construction points retained by one derived contour segment.
fn outline_segment_points(segment: &CurveSegment) -> impl Iterator<Item = Point2> + use<> {
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

/// Adds two finite geometric values without hiding numeric overflow.
fn add(point: Point2, vector: Vector2) -> Point2 {
    Point2::new(point.x + vector.x, point.y + vector.y)
}

/// Constructs the shared cancellation diagnostic for outline construction.
fn cancelled() -> CurveError {
    CurveError::new("evaluation.cancelled", "evaluation was cancelled")
}

/// Resolved paired rails and tangent data for one retained outline sample.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    use std::cell::Cell;

    /// Constructs ordered exact samples on one straight centerline for simplification witnesses.
    fn line_samples(widths: &[f64]) -> Vec<VariableWidthPathSample> {
        widths
            .iter()
            .enumerate()
            .map(|(index, width)| VariableWidthPathSample {
                location: PathLocation::new(0, index as f64 / (widths.len() - 1) as f64)
                    .expect("finite line location"),
                width: *width,
            })
            .collect()
    }

    /// Proves a descending subpixel valley survives and every omitted positive width meets 2%.
    #[test]
    fn relative_compaction_retains_near_zero_valley() {
        let path =
            CurvePath::line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)).expect("finite line");
        let samples = line_samples(&[0.02, 0.01, 0.0008, 0.001, 0.02]);
        let retained = simplify_positive_run(&path, &samples, 0.125, &|| false)
            .expect("relative simplification");
        assert!(
            retained.contains(&samples[2]),
            "narrow valley remains explicit"
        );
        for pair in retained.windows(2) {
            let start = samples
                .iter()
                .position(|sample| sample == &pair[0])
                .expect("retained start");
            let end = samples
                .iter()
                .position(|sample| sample == &pair[1])
                .expect("retained end");
            for (index, sample) in samples.iter().enumerate().take(end).skip(start + 1) {
                let fraction = (index - start) as f64 / (end - start) as f64;
                let expected = pair[0].width + (pair[1].width - pair[0].width) * fraction;
                let half_error = (sample.width - expected).abs() * 0.5;
                assert!(half_error <= (0.02 * sample.width * 0.5).min(0.125));
            }
        }
    }

    /// Proves straight-path relative selection is scale-invariant below the absolute bound.
    #[test]
    fn relative_compaction_scales_with_positive_width() {
        let path =
            CurvePath::line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)).expect("finite line");
        let widths = [0.02, 0.01, 0.0008, 0.001, 0.02];
        let base = line_samples(&widths);
        let scaled = line_samples(&widths.map(|width| width * 10.0));
        let base = simplify_positive_run(&path, &base, 0.125, &|| false).expect("base");
        let scaled = simplify_positive_run(&path, &scaled, 0.125, &|| false).expect("scaled");
        assert_eq!(
            base.iter()
                .map(|sample| sample.location)
                .collect::<Vec<_>>(),
            scaled
                .iter()
                .map(|sample| sample.location)
                .collect::<Vec<_>>()
        );
    }

    /// Proves constant-width outlines match two-endpoint geometry and zeros still split runs.
    #[test]
    fn relative_compaction_preserves_constant_outline_and_zero_break() {
        let path =
            CurvePath::line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)).expect("finite line");
        let build = |samples: &[VariableWidthPathSample]| {
            build_variable_width_outline_cancellable(
                &path,
                samples,
                toniator_domain::PathStrokeStyle::default(),
                0.0,
                0.125,
                VariableWidthOutlineLimits::new(128).expect("segment limit"),
                &|| false,
            )
            .expect("outline")
        };
        assert_eq!(
            build(&line_samples(&[2.0, 2.0, 2.0, 2.0, 2.0])),
            build(&line_samples(&[2.0, 2.0]))
        );
        let split = build(&line_samples(&[2.0, 0.0, 2.0]));
        assert_eq!(split.contours.len(), 2);
    }

    /// Proves a curved center still exceeds the unchanged absolute tolerance and survives.
    #[test]
    fn relative_compaction_retains_curved_center() {
        let path = CurvePath::new(
            vec![CurveSegment::CubicBezier(
                CubicBezierSegment::new(
                    Point2::new(0.0, 0.0),
                    Point2::new(0.0, 4.0),
                    Point2::new(10.0, 4.0),
                    Point2::new(10.0, 0.0),
                )
                .expect("finite cubic"),
            )],
            PathClosure::Open,
        )
        .expect("curved path");
        let samples = line_samples(&[2.0, 2.0, 2.0]);
        let retained = simplify_positive_run(&path, &samples, 0.125, &|| false)
            .expect("curved simplification");
        assert_eq!(retained, samples);
    }

    /// Proves a long scan observes cancellation and a tighter segment budget fails atomically.
    #[test]
    fn relative_compaction_cancels_during_scan_and_observes_segment_limit() {
        let path =
            CurvePath::line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)).expect("finite line");
        let samples = line_samples(&vec![0.01; 512]);
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() >= 9
        };
        let error =
            simplify_positive_run(&path, &samples, 0.125, &cancelled).expect_err("scan cancels");
        assert_eq!(error.path(), "evaluation.cancelled");
        assert_eq!(calls.get(), 9, "cancellation reaches the interval scan");
        let error = build_variable_width_outline_cancellable(
            &path,
            &line_samples(&[0.01, 0.01, 0.01]),
            toniator_domain::PathStrokeStyle::default(),
            0.0,
            0.125,
            VariableWidthOutlineLimits::new(1).expect("segment limit"),
            &|| false,
        )
        .expect_err("outline does not publish partial geometry");
        assert_eq!(error.path(), "curve.outline.segment_limit");
    }

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

/// One original procedural primitive, represented on the stack before canonical construction.
#[derive(Clone, Copy)]
enum OutlineCommand {
    Line(Point2, Point2),
    Cubic(Point2, Point2, Point2, Point2),
}
impl OutlineCommand {
    /// Returns the exact original directed endpoint pair.
    fn ends(self) -> (Point2, Point2) {
        match self {
            Self::Line(a, b) | Self::Cubic(a, _, _, b) => (a, b),
        }
    }
    /// Encloses every original control point outward, including arc handle bulges.
    fn bounds(self) -> Option<Bounds> {
        match self {
            Self::Line(a, b) => crate::construction_bounds([a, b]),
            Self::Cubic(a, b, c, d) => crate::construction_bounds([a, b, c, d]),
        }
    }
    /// Constructs one original primitive only after its materialization decision.
    /// # Errors
    /// Propagates the unchanged finite-control-point diagnostic.
    fn construct(self) -> Result<CurveSegment, CurveError> {
        match self {
            Self::Line(a, b) => Ok(CurveSegment::Line(LineSegment::new(a, b)?)),
            Self::Cubic(a, b, c, d) => Ok(CurveSegment::CubicBezier(CubicBezierSegment::new(
                a, b, c, d,
            )?)),
        }
    }
}
/// A procedural sink can inspect an original command without allocating a canonical segment.
trait OutlineEmitter {
    /// Emits or certifiably replaces one command under the caller's actual storage policy.
    /// # Errors
    /// Propagates finite geometry, count or allocation diagnostics.
    fn emit(&mut self, command: OutlineCommand, remaining: &mut usize) -> Result<(), CurveError>;
}
/// Preserves the complete direct helper route used by focused geometry tests.
impl OutlineEmitter for Vec<CurveSegment> {
    /// Constructs the unchanged command through checked fallible canonical storage.
    /// # Errors
    /// Propagates original finite-coordinate, reserve or policy diagnostics.
    fn emit(&mut self, command: OutlineCommand, remaining: &mut usize) -> Result<(), CurveError> {
        let context = ConstructionContext::current();
        context.count(ConstructionCounter::OutlineCommands, 1);
        emit_outline_command(self, command, remaining, &context)
    }
}
/// Computes only the original endpoint extrema; no segment arrays or constructors are reached.
struct EndpointScan(Option<Bounds>);
impl OutlineEmitter for EndpointScan {
    /// Retains the original endpoint envelope for quantization-frame anchor certification.
    fn emit(&mut self, command: OutlineCommand, remaining: &mut usize) -> Result<(), CurveError> {
        if *remaining == 0 {
            return Err(CurveError::new(
                "curve.outline.segment_limit",
                "variable-width outline exceeds the configured segment limit",
            ));
        }
        *remaining -= 1;
        let (a, b) = command.ends();
        for p in [a, b] {
            self.0 = Some(match self.0 {
                None => Bounds { min: p, max: p },
                Some(v) => Bounds {
                    min: Point2::new(v.min.x.min(p.x), v.min.y.min(p.y)),
                    max: Point2::new(v.max.x.max(p.x), v.max.y.max(p.y)),
                },
            });
        }
        Ok(())
    }
}
/// Streams materialized commands and common-half-plane exterior connectors in original contour order.
struct MaterializingSink {
    segments: Vec<CurveSegment>,
    coverage: MaterializationCoverage,
    endpoints: Option<Bounds>,
    pending: Option<(Point2, Point2, u8)>,
    pruned: bool,
    context: ConstructionContext,
}
impl MaterializingSink {
    /// Finishes a pending exterior excursion without inserting an endpoint cap.
    /// # Errors
    /// Propagates original finite/storage diagnostics before contour publication.
    fn flush(&mut self, remaining: &mut usize) -> Result<(), CurveError> {
        if let Some((a, b, _)) = self.pending.take()
            && a != b
        {
            self.context
                .count(ConstructionCounter::OutlineConnectors, 1);
            emit_outline_command(
                &mut self.segments,
                OutlineCommand::Line(a, b),
                remaining,
                &self.context,
            )?;
        }
        Ok(())
    }
}
impl OutlineEmitter for MaterializingSink {
    /// Preserves extrema-bearing originals and certifies each omitted command before construction.
    /// # Errors
    /// Returns finite/count/allocation failure without publishing a partial outline.
    fn emit(&mut self, command: OutlineCommand, remaining: &mut usize) -> Result<(), CurveError> {
        self.context.count(ConstructionCounter::OutlineCommands, 1);
        let mask = command.bounds().map_or(0, |bounds| {
            let anchor = self.endpoints.is_none_or(|e| {
                bounds.min.x <= e.min.x
                    || bounds.min.y <= e.min.y
                    || bounds.max.x >= e.max.x
                    || bounds.max.y >= e.max.y
            });
            if anchor {
                0
            } else {
                self.coverage.exterior(bounds)
            }
        });
        if mask != 0 {
            self.pruned = true;
            let (a, b) = command.ends();
            if let Some((start, end, old)) = self.pending
                && end == a
                && old & mask != 0
            {
                self.pending = Some((start, b, old & mask));
                return Ok(());
            }
            self.flush(remaining)?;
            self.pending = Some((a, b, mask));
            return Ok(());
        }
        self.flush(remaining)?;
        emit_outline_command(&mut self.segments, command, remaining, &self.context)
    }
}
/// Records real constructor/reserved-capacity work, rather than counting retained vector lengths.
/// # Errors
/// Returns finite/control, actual allocator or explicit segment-policy failures before append.
fn emit_outline_command(
    segments: &mut Vec<CurveSegment>,
    command: OutlineCommand,
    remaining: &mut usize,
    context: &ConstructionContext,
) -> Result<(), CurveError> {
    if *remaining == 0 {
        return Err(CurveError::new(
            "curve.outline.segment_limit",
            "variable-width outline exceeds the configured segment limit",
        ));
    }
    let old = segments.capacity();
    reserve_curve(segments, 1)?;
    context.count(
        ConstructionCounter::OutlineReservedBytes,
        segments
            .capacity()
            .saturating_sub(old)
            .saturating_mul(std::mem::size_of::<CurveSegment>()),
    );
    let segment = command.construct()?;
    context.count(ConstructionCounter::OutlineConstructors, 1);
    segments.push(segment);
    *remaining -= 1;
    Ok(())
}
