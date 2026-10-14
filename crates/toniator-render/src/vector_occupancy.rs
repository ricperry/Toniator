//! Source-independent opaque union of already realized canonical geometry.

use super::{GeometryOutput, RasterizationLimits, RenderError, validate_canvas, validate_geometry};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::scale::FixedScaleFloatOverlay,
};
use kurbo::{Circle, CubicBez, ParamCurve as _, PathEl, Point, Shape as _, flatten};
use std::time::{Duration, Instant};
use toniator_domain::CanvasSpec;
use toniator_geometry::{CanonicalFillRule, CanonicalMark, CurveSegment};

type Contour = Vec<[f64; 2]>;
type FilledShape = Vec<Contour>;
type FilledShapes = Vec<FilledShape>;

/// Two canvas-relative precision settings for independent vector measurements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VectorOccupancyPrecision {
    Baseline,
    Tight,
}

impl VectorOccupancyPrecision {
    /// Returns normalized flatten tolerance and i64 fixed scale.
    fn settings(self) -> (f64, f64) {
        match self {
            Self::Baseline => (1.0 / 131_072.0, 1_000_000_000.0),
            Self::Tight => (1.0 / 262_144.0, 5_000_000_000.0),
        }
    }
}

/// Measured opaque canvas fraction and bounded consumer work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VectorOccupancyMeasurement {
    pub fraction: f64,
    pub vertices: usize,
    /// Time constructing the opaque union intersected with the canvas in one Boolean operation.
    pub union_duration: Duration,
}

/// Measures a single canonical output's opaque union, independently of paint and channel model.
///
/// Each primitive is normalized under its own fill rule before its positive subject fill is
/// unioned and intersected with the canvas in one Boolean operation. Overlap, reversed
/// windings, holes, and off-canvas shapes therefore retain their canonical meaning. Coordinates
/// and tolerances are normalized by the canvas long axis before i64 Boolean quantization.
/// Flattening polls cancellation during bounded subdivision and Kurbo callbacks; a callback
/// observes cancellation but the bounded current span must return before flattening can stop.
/// Ordinary iOverlay calls retain only pre/post cancellation checks, without interior polling.
/// The flattened-edge budget bounds retained input vertices. This consumer returns coverage,
/// rather than drawing RGBA/SVG; newly fitted curves may subsequently affect rendered output.
///
/// # Errors
/// Rejects invalid geometry/canvas, cancellation, edge budget, unsafe quantization, Boolean
/// failure, or invalid area without publishing a partial result.
pub fn measure_geometry_vector_occupancy(
    geometry: &GeometryOutput,
    canvas: &CanvasSpec,
    precision: VectorOccupancyPrecision,
    limits: RasterizationLimits,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<VectorOccupancyMeasurement, RenderError> {
    validate_canvas(canvas)?;
    validate_geometry(geometry)?;
    check_cancel(is_cancelled)?;
    let axis = canvas.width.max(canvas.height);
    let normalized_canvas = [canvas.width / axis, canvas.height / axis];
    let canvas_area = normalized_canvas[0] * normalized_canvas[1];
    if !canvas_area.is_finite() || canvas_area <= 0.0 {
        return Err(error(
            "vector.canvas",
            "normalized canvas area is not representable",
        ));
    }
    let (tolerance, scale) = precision.settings();
    if normalized_canvas[0].min(normalized_canvas[1]) * scale < 4.0 {
        return Err(error(
            "vector.precision.canvas",
            "canvas short axis is below vector quantization resolution",
        ));
    }
    let budget = limits.max_flattened_edges();
    let empty: Contour = Vec::new();
    let mut normalized = FilledShapes::new();
    let mut vertices = 0_usize;
    let mut append = |shape: Option<(FilledShape, FillRule)>| -> Result<(), RenderError> {
        check_cancel(is_cancelled)?;
        let Some((shape, rule)) = shape else {
            return Ok(());
        };
        for contour in &shape {
            vertices = vertices.checked_add(contour.len()).ok_or(error(
                "vector.limits.flattened_edges",
                "flattened vector edge count overflows",
            ))?;
            if vertices > budget {
                return Err(error(
                    "vector.limits.flattened_edges",
                    "flattened vector edge limit exceeded",
                ));
            }
            for point in contour {
                for coordinate in point {
                    if !coordinate.is_finite()
                        || coordinate.abs() > (i64::MAX as f64 / 16.0) / scale
                    {
                        return Err(error(
                            "vector.precision.coordinate",
                            "geometry cannot be quantized safely",
                        ));
                    }
                }
            }
        }
        check_cancel(is_cancelled)?;
        let mut one = shape
            .overlay_with_fixed_scale_as::<i64>(&empty, OverlayRule::Subject, rule, scale)
            .map_err(|_| {
                error(
                    "vector.overlay.primitive",
                    "canonical primitive normalization failed",
                )
            })?;
        check_cancel(is_cancelled)?;
        normalized.append(&mut one);
        Ok(())
    };
    match geometry {
        GeometryOutput::CircularMarks(marks) => {
            for mark in marks {
                check_cancel(is_cancelled)?;
                append(circle_shape(
                    mark.center.x,
                    mark.center.y,
                    mark.radius,
                    axis,
                    tolerance,
                    budget,
                    is_cancelled,
                )?)?;
            }
        }
        GeometryOutput::CanonicalMarks(marks) => {
            for mark in marks {
                check_cancel(is_cancelled)?;
                let shape = match mark {
                    CanonicalMark::Circle { center, radius, .. } => circle_shape(
                        center.x,
                        center.y,
                        *radius,
                        axis,
                        tolerance,
                        budget,
                        is_cancelled,
                    )?,
                    CanonicalMark::ClosedPath(path) => Some((
                        vec![flatten_contour(
                            path.path.segments(),
                            axis,
                            tolerance,
                            budget,
                            is_cancelled,
                        )?],
                        fill_rule(path.fill_rule),
                    )),
                };
                append(shape)?;
            }
        }
        GeometryOutput::CanonicalStrokes(strokes) => {
            for stroke in strokes {
                check_cancel(is_cancelled)?;
                if stroke.outline.contours.is_empty() {
                    continue;
                }
                let mut shape = FilledShape::new();
                for contour in &stroke.outline.contours {
                    shape.push(flatten_contour(
                        &contour.segments,
                        axis,
                        tolerance,
                        budget,
                        is_cancelled,
                    )?);
                }
                append(Some((shape, fill_rule(stroke.outline.fill_rule))))?;
            }
        }
        GeometryOutput::CanonicalRegions(regions) => {
            for region in regions.regions() {
                check_cancel(is_cancelled)?;
                append(Some((
                    vec![flatten_contour(
                        region.ring.segments(),
                        axis,
                        tolerance,
                        budget,
                        is_cancelled,
                    )?],
                    FillRule::NonZero,
                )))?;
            }
        }
    }
    check_cancel(is_cancelled)?;
    let started = Instant::now();
    let rect = vec![
        [0.0, 0.0],
        [normalized_canvas[0], 0.0],
        [normalized_canvas[0], normalized_canvas[1]],
        [0.0, normalized_canvas[1]],
    ];
    // Primitive normalization makes each subject's winding nonnegative on its filled set.
    // NonZero subject/clip intersection therefore computes union(subjects) intersect canvas
    // directly, without materializing and requantizing an intermediate full union.
    let clipped = normalized
        .overlay_with_fixed_scale_as::<i64>(&rect, OverlayRule::Intersect, FillRule::NonZero, scale)
        .map_err(|_| {
            error(
                "vector.overlay.clip",
                "opaque union canvas intersection failed",
            )
        })?;
    let union_duration = started.elapsed();
    check_cancel(is_cancelled)?;
    let area = area_of_shapes(&clipped, is_cancelled)?;
    let fraction = area / canvas_area;
    if !fraction.is_finite() || !(-1e-8..=1.0 + 1e-8).contains(&fraction) {
        return Err(error(
            "vector.area",
            "opaque union area is outside the canvas",
        ));
    }
    Ok(VectorOccupancyMeasurement {
        fraction: fraction.clamp(0.0, 1.0),
        vertices,
        union_duration,
    })
}

/// Maps the canonical per-primitive fill rule without altering contour winding.
fn fill_rule(rule: CanonicalFillRule) -> FillRule {
    match rule {
        CanonicalFillRule::EvenOdd => FillRule::EvenOdd,
        CanonicalFillRule::NonZero => FillRule::NonZero,
    }
}

/// Flattens a canonical circle through Kurbo's official circle path and line flattener.
///
/// # Errors
/// Returns invalid normalization, cancellation, or edge-budget diagnostics.
fn circle_shape(
    x: f64,
    y: f64,
    radius: f64,
    axis: f64,
    tolerance: f64,
    budget: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<(FilledShape, FillRule)>, RenderError> {
    if radius == 0.0 {
        return Ok(None);
    }
    let circle = Circle::new(Point::new(x / axis, y / axis), radius / axis);
    if !circle.is_finite() {
        return Err(error("vector.circle", "circle cannot be normalized"));
    }
    let max_coordinate = (i64::MAX as f64 / 16.0) / 10_000_000_000.0;
    if [circle.center.x, circle.center.y, circle.radius]
        .into_iter()
        .any(|value| !value.is_finite() || value.abs() > max_coordinate)
    {
        return Err(error(
            "vector.precision.coordinate",
            "circle cannot be quantized safely",
        ));
    }
    let contour = flatten_elements(
        circle.path_elements(tolerance),
        tolerance,
        budget,
        cancelled,
    )?;
    Ok(Some((vec![contour], FillRule::NonZero)))
}

/// Converts canonical line/cubic segments to a closed Kurbo path without changing topology.
///
/// # Errors
/// Returns empty, cancelled, invalid, or over-budget contour diagnostics.
pub(super) fn flatten_contour(
    segments: &[CurveSegment],
    axis: f64,
    tolerance: f64,
    budget: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Contour, RenderError> {
    let first = segments
        .first()
        .ok_or(error("vector.contour", "filled contour has no segments"))?;
    if segments.len() > budget {
        return Err(error(
            "vector.limits.flattened_edges",
            "canonical contour exceeds vector edge limit",
        ));
    }
    for segment in segments {
        check_cancel(cancelled)?;
        let points: &[toniator_geometry::Point2] = match segment {
            CurveSegment::Line(line) => &[line.start(), line.end()],
            CurveSegment::CubicBezier(cubic) => &[
                cubic.start(),
                cubic.control_1(),
                cubic.control_2(),
                cubic.end(),
            ],
        };
        if points.iter().any(|point| !point.is_finite()) {
            return Err(error(
                "vector.contour",
                "canonical contour coordinates must be finite",
            ));
        }
    }
    let start = first.start();
    let mut path = Vec::with_capacity(segments.len() + 2);
    path.push(PathEl::MoveTo(Point::new(start.x / axis, start.y / axis)));
    for segment in segments {
        check_cancel(cancelled)?;
        match segment {
            CurveSegment::Line(line) => path.push(PathEl::LineTo(Point::new(
                line.end().x / axis,
                line.end().y / axis,
            ))),
            CurveSegment::CubicBezier(cubic) => path.push(PathEl::CurveTo(
                Point::new(cubic.control_1().x / axis, cubic.control_1().y / axis),
                Point::new(cubic.control_2().x / axis, cubic.control_2().y / axis),
                Point::new(cubic.end().x / axis, cubic.end().y / axis),
            )),
        }
    }
    path.push(PathEl::ClosePath);
    flatten_elements(path, tolerance, budget, cancelled)
}

/// Retains one finite bounded contour from Kurbo flattening.
///
/// # Errors
/// Returns cancellation, degeneracy, or the existing flattened-edge budget diagnostic.
fn flatten_elements(
    path: impl IntoIterator<Item = PathEl>,
    tolerance: f64,
    budget: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Contour, RenderError> {
    check_cancel(cancelled)?;
    let mut points = Vec::new();
    let mut last = None;
    let mut closed = false;
    let mut path_elements = 0_usize;
    let mut split_pieces = 0_usize;
    for element in path {
        check_cancel(cancelled)?;
        path_elements = path_elements.checked_add(1).ok_or(error(
            "vector.limits.flattened_edges",
            "vector path element count overflows",
        ))?;
        if path_elements > budget {
            return Err(error(
                "vector.limits.flattened_edges",
                "vector path elements exceed edge limit",
            ));
        }
        match element {
            PathEl::MoveTo(point) => {
                if !points.is_empty() || !point.x.is_finite() || !point.y.is_finite() {
                    return Err(error("vector.contour", "filled path has invalid move"));
                }
                points.push([point.x, point.y]);
                last = Some(point);
            }
            PathEl::LineTo(point) => {
                if last.is_none() || !point.x.is_finite() || !point.y.is_finite() {
                    return Err(error("vector.contour", "filled path has invalid line"));
                }
                if points.len() >= budget {
                    return Err(error(
                        "vector.limits.flattened_edges",
                        "flattened vector edge limit exceeded",
                    ));
                }
                points.push([point.x, point.y]);
                last = Some(point);
            }
            PathEl::CurveTo(p1, p2, p3) => {
                let p0 = last.ok_or(error("vector.contour", "cubic has no starting point"))?;
                if [p1, p2, p3]
                    .into_iter()
                    .any(|point| !point.x.is_finite() || !point.y.is_finite())
                {
                    return Err(error("vector.contour", "filled cubic is not finite"));
                }
                let mut pending = vec![CubicBez { p0, p1, p2, p3 }];
                while let Some(cubic) = pending.pop() {
                    check_cancel(cancelled)?;
                    split_pieces = split_pieces.checked_add(1).ok_or(error(
                        "vector.limits.flattened_edges",
                        "vector subdivision count overflows",
                    ))?;
                    if split_pieces > budget {
                        return Err(error(
                            "vector.limits.flattened_edges",
                            "vector subdivision exceeds edge limit",
                        ));
                    }
                    let xs = [cubic.p0.x, cubic.p1.x, cubic.p2.x, cubic.p3.x];
                    let ys = [cubic.p0.y, cubic.p1.y, cubic.p2.y, cubic.p3.y];
                    let span = (xs.into_iter().fold(f64::NEG_INFINITY, f64::max)
                        - xs.into_iter().fold(f64::INFINITY, f64::min))
                    .max(
                        ys.into_iter().fold(f64::NEG_INFINITY, f64::max)
                            - ys.into_iter().fold(f64::INFINITY, f64::min),
                    );
                    if !span.is_finite() {
                        return Err(error("vector.contour", "cubic hull is not finite"));
                    }
                    if span > tolerance * 1_024.0 {
                        let (left, right) = cubic.subdivide();
                        pending.push(right);
                        pending.push(left);
                        continue;
                    }
                    let mut overflow = false;
                    let mut interrupted = false;
                    flatten(
                        [
                            PathEl::MoveTo(cubic.p0),
                            PathEl::CurveTo(cubic.p1, cubic.p2, cubic.p3),
                        ],
                        tolerance,
                        |flat| {
                            if cancelled() {
                                interrupted = true;
                                return;
                            }
                            if let PathEl::LineTo(point) = flat {
                                if points.len() >= budget {
                                    overflow = true;
                                } else {
                                    points.push([point.x, point.y]);
                                }
                            }
                        },
                    );
                    if interrupted {
                        return Err(error(
                            "evaluation.cancelled",
                            "vector occupancy was cancelled",
                        ));
                    }
                    if overflow {
                        return Err(error(
                            "vector.limits.flattened_edges",
                            "flattened vector edge limit exceeded",
                        ));
                    }
                }
                last = Some(p3);
            }
            PathEl::ClosePath => {
                closed = true;
                last = None;
            }
            PathEl::QuadTo(_, _) => {
                return Err(error(
                    "vector.contour",
                    "canonical filled contour has an unsupported quadratic",
                ));
            }
        }
    }
    check_cancel(cancelled)?;
    if points.len() >= 2 && points[0] == points[points.len() - 1] {
        points.pop();
    }
    if !closed || points.len() < 3 || points.iter().flatten().any(|value| !value.is_finite()) {
        return Err(error(
            "vector.contour",
            "flattened filled contour is not finite and closed",
        ));
    }
    Ok(points)
}

/// Sums iOverlay-normalized outer/hole contours with finite shoelace arithmetic.
///
/// # Errors
/// Rejects cancellation, degenerate library results, or nonfinite area.
fn area_of_shapes(
    shapes: &FilledShapes,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<f64, RenderError> {
    let mut total = 0.0;
    for shape in shapes {
        check_cancel(cancelled)?;
        if shape.is_empty() {
            return Err(error("vector.area", "union has an empty filled shape"));
        }
        let mut shape_area = 0.0;
        for contour in shape {
            if contour.len() < 3 {
                return Err(error("vector.area", "union has a degenerate contour"));
            }
            let mut signed = 0.0;
            for (index, point) in contour.iter().enumerate() {
                let next = contour[(index + 1) % contour.len()];
                signed += point[0] * next[1] - next[0] * point[1];
            }
            shape_area += signed * 0.5;
        }
        total += shape_area.abs();
        if !total.is_finite() {
            return Err(error("vector.area", "union area is not finite"));
        }
    }
    Ok(total)
}

/// Polls cancellation before a library call or canonical primitive.
///
/// # Errors
/// Returns the existing evaluation cancellation path.
fn check_cancel(cancelled: &(dyn Fn() -> bool + Sync)) -> Result<(), RenderError> {
    if cancelled() {
        Err(error(
            "evaluation.cancelled",
            "vector occupancy was cancelled",
        ))
    } else {
        Ok(())
    }
}

/// Builds a stable renderer diagnostic for the vector measurement consumer.
fn error(path: &'static str, message: &'static str) -> RenderError {
    RenderError::new(path, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_domain::PatternMechanismId;
    use toniator_geometry::{
        CanonicalPathMark, CurvePath, FamilySiteId, FamilySiteProvenance, PathClosure, Point2,
        SiteScope,
    };

    /// Returns one opaque fixture mark with identity irrelevant to vector occupancy.
    fn circle(ordinal: usize, x: f64, y: f64, radius: f64) -> CanonicalMark {
        CanonicalMark::Circle {
            source_site_id: FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal,
            },
            center: Point2::new(x, y),
            radius,
            scope: SiteScope::Canvas,
            provenance: FamilySiteProvenance::Random {
                candidate_ordinal: ordinal,
                accepted_ordinal: ordinal,
                exclusion_neighbor_ordinal: None,
            },
            fill_rule: CanonicalFillRule::EvenOdd,
        }
    }

    /// Duplicates cannot count twice, and normalizing by canvas axis preserves circle area.
    ///
    /// # Panics
    /// Panics if library union or either scaled fixture misses its analytic circle area.
    #[test]
    fn duplicate_circles_keep_analytic_union_across_canvas_scales() {
        for scale in [0.001, 10.0, 1_000_000.0] {
            let canvas = CanvasSpec {
                width: 10.0 * scale,
                height: 10.0 * scale,
            };
            let geometry = GeometryOutput::CanonicalMarks(vec![
                circle(1, 5.0 * scale, 5.0 * scale, 2.0 * scale),
                circle(2, 5.0 * scale, 5.0 * scale, 2.0 * scale),
            ]);
            for precision in [
                VectorOccupancyPrecision::Baseline,
                VectorOccupancyPrecision::Tight,
            ] {
                let measured = measure_geometry_vector_occupancy(
                    &geometry,
                    &canvas,
                    precision,
                    RasterizationLimits::default(),
                    &|| false,
                )
                .unwrap();
                assert!((measured.fraction - std::f64::consts::PI * 4.0 / 100.0).abs() < 0.0001);
            }
        }
    }

    /// Keeps independently filled reversed closed marks from cancelling one another.
    ///
    /// # Panics
    /// Panics if accepted closed paths or their vector union do not retain the square's area.
    #[test]
    fn reversed_duplicate_closed_marks_remain_opaque() {
        let vertices = vec![
            Point2::new(2.0, 2.0),
            Point2::new(6.0, 2.0),
            Point2::new(6.0, 6.0),
            Point2::new(2.0, 6.0),
        ];
        let mut marks = Vec::new();
        for (ordinal, points) in [vertices.clone(), vertices.into_iter().rev().collect()]
            .into_iter()
            .enumerate()
        {
            let path = CurvePath::polyline(points, PathClosure::Closed).unwrap();
            marks.push(CanonicalMark::ClosedPath(
                CanonicalPathMark::new(
                    FamilySiteId {
                        mechanism_id: PatternMechanismId(1),
                        ordinal,
                    },
                    path,
                    SiteScope::Canvas,
                    FamilySiteProvenance::Random {
                        candidate_ordinal: ordinal,
                        accepted_ordinal: ordinal,
                        exclusion_neighbor_ordinal: None,
                    },
                    CanonicalFillRule::EvenOdd,
                )
                .unwrap(),
            ));
        }
        let result = measure_geometry_vector_occupancy(
            &GeometryOutput::CanonicalMarks(marks),
            &CanvasSpec {
                width: 10.0,
                height: 10.0,
            },
            VectorOccupancyPrecision::Tight,
            RasterizationLimits::default(),
            &|| false,
        )
        .unwrap();
        assert!((result.fraction - 0.16).abs() < 1e-8);
    }

    /// Rejects cancellation and malformed raw geometry before publishing an occupancy value.
    #[test]
    fn invalid_or_cancelled_vector_measurement_fails() {
        let canvas = CanvasSpec {
            width: 10.0,
            height: 10.0,
        };
        let geometry = GeometryOutput::CanonicalMarks(vec![circle(1, 5.0, 5.0, 2.0)]);
        assert_eq!(
            measure_geometry_vector_occupancy(
                &geometry,
                &canvas,
                VectorOccupancyPrecision::Baseline,
                RasterizationLimits::default(),
                &|| true
            )
            .unwrap_err()
            .path(),
            "evaluation.cancelled"
        );
        let invalid = GeometryOutput::CanonicalMarks(vec![circle(1, 5.0, 5.0, -1.0)]);
        assert!(
            measure_geometry_vector_occupancy(
                &invalid,
                &canvas,
                VectorOccupancyPrecision::Baseline,
                RasterizationLimits::default(),
                &|| false
            )
            .is_err()
        );
    }

    /// Refuses a valid full-canvas rectangle when its short axis collapses in i64 quantization.
    ///
    /// # Panics
    /// Panics if the fixture is invalid or a false zero/full area is returned as measurement.
    #[test]
    fn unresolvable_thin_canvas_is_an_error_not_zero_coverage() {
        let path = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(1.0, 1e-12),
                Point2::new(0.0, 1e-12),
            ],
            PathClosure::Closed,
        )
        .unwrap();
        let mark = CanonicalPathMark::new(
            FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal: 1,
            },
            path,
            SiteScope::Canvas,
            FamilySiteProvenance::Random {
                candidate_ordinal: 1,
                accepted_ordinal: 1,
                exclusion_neighbor_ordinal: None,
            },
            CanonicalFillRule::EvenOdd,
        )
        .unwrap();
        let result = measure_geometry_vector_occupancy(
            &GeometryOutput::CanonicalMarks(vec![CanonicalMark::ClosedPath(mark)]),
            &CanvasSpec {
                width: 1.0,
                height: 1e-12,
            },
            VectorOccupancyPrecision::Baseline,
            RasterizationLimits::default(),
            &|| false,
        );
        assert_eq!(result.unwrap_err().path(), "vector.precision.canvas");
    }

    /// Stops finite circle and cubic flattening under a deliberately small shared edge budget.
    ///
    /// # Panics
    /// Panics if either canonical curve escapes the bound or changes its limit diagnostic.
    #[test]
    fn finite_curves_obey_small_flatten_budget() {
        let canvas = CanvasSpec {
            width: 100.0,
            height: 100.0,
        };
        let circle_geometry = GeometryOutput::CanonicalMarks(vec![circle(1, 50.0, 50.0, 40.0)]);
        let limits = RasterizationLimits::new(16).unwrap();
        assert_eq!(
            measure_geometry_vector_occupancy(
                &circle_geometry,
                &canvas,
                VectorOccupancyPrecision::Tight,
                limits,
                &|| false,
            )
            .unwrap_err()
            .path(),
            "vector.limits.flattened_edges",
        );
        let elements = [
            PathEl::MoveTo(Point::new(0.0, 0.0)),
            PathEl::CurveTo(
                Point::new(100.0, 300.0),
                Point::new(-100.0, -300.0),
                Point::new(50.0, 50.0),
            ),
            PathEl::LineTo(Point::new(0.0, 0.0)),
            PathEl::ClosePath,
        ];
        assert_eq!(
            flatten_elements(elements, 1.0 / 262_144.0, 16, &|| false)
                .unwrap_err()
                .path(),
            "vector.limits.flattened_edges",
        );
    }
    /// Proves a single clipped union preserves typed positive fill, holes, touching and empty boundaries.
    ///
    /// Analytic polygons independently constrain the result; curved circles retain the existing
    /// flattening envelope. Reversed and self-intersecting outlines normalize before combination.
    ///
    /// # Panics
    /// Panics if valid current geometry fails or its canvas fraction violates analytic fill authority.
    #[test]
    fn clipped_union_keeps_typed_holes_self_overlap_and_canvas_boundaries() {
        let canvas = CanvasSpec {
            width: 10.0,
            height: 10.0,
        };
        for (name, geometry, expected) in scalar_contract_cases::cases() {
            for precision in [
                VectorOccupancyPrecision::Baseline,
                VectorOccupancyPrecision::Tight,
            ] {
                let measured = measure_geometry_vector_occupancy(
                    &geometry,
                    &canvas,
                    precision,
                    RasterizationLimits::default(),
                    &|| false,
                )
                .unwrap();
                let repeated = measure_geometry_vector_occupancy(
                    &geometry,
                    &canvas,
                    precision,
                    RasterizationLimits::default(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(measured.fraction.to_bits(), repeated.fraction.to_bits());
                assert_eq!(measured.vertices, repeated.vertices);
                let tolerance =
                    if name == "circle-crossing-canvas" || name == "off-grid-curved-hole" {
                        1e-5
                    } else {
                        1e-8
                    };
                assert!(
                    (measured.fraction - expected).abs() < tolerance,
                    "{name} {precision:?}: {} vs {expected}",
                    measured.fraction
                );
            }
        }
    }

    mod scalar_contract_cases {
        //! Exact analytic current-geometry witnesses for independent scalar-consumer comparisons.
        use crate::GeometryOutput;
        use toniator_domain::{
            GuideDimensionId, PathStrokeStyle, PatternMechanismId, PatternOutputLayerId,
        };
        use toniator_geometry::{
            CanonicalFillRule, CanonicalFilledOutline, CanonicalMark, CanonicalOutlineContour,
            CanonicalPathMark, CanonicalRegionProposal, CanonicalRegionSourceGroup,
            CanonicalRegionSourceId, CanonicalStroke, CubicBezierSegment, CurvePath, CurveSegment,
            FamilySiteId, FamilySiteProvenance, PathClosure, PathLocation, Point2, SiteScope,
            StrokeProfileSample, StructuralPathInstanceId, build_canonical_regions,
        };

        /// Builds one valid stable site identity without influencing measurement geometry.
        fn site(ordinal: usize) -> FamilySiteId {
            FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal,
            }
        }
        /// Supplies source-independent provenance for typed consumer witnesses.
        fn provenance() -> FamilySiteProvenance {
            FamilySiteProvenance::Random {
                candidate_ordinal: 0,
                accepted_ordinal: 0,
                exclusion_neighbor_ordinal: None,
            }
        }
        /// Builds a closed path from explicit document-space vertices.
        /// # Panics
        /// Panics if the finite fixture is invalid.
        fn path(points: &[[f64; 2]]) -> CurvePath {
            CurvePath::polyline(
                points.iter().map(|p| Point2::new(p[0], p[1])).collect(),
                PathClosure::Closed,
            )
            .unwrap()
        }
        /// Builds a typed outline with holes or self-intersection without reinterpreting its NonZero authority.
        /// # Panics
        /// Panics if the finite canonical consumer fixture fails validation.
        fn stroke(contours: Vec<CurvePath>) -> CanonicalStroke {
            let bounds = contours[0].bounds().unwrap();
            let outline = CanonicalFilledOutline {
                contours: contours
                    .into_iter()
                    .map(|p| CanonicalOutlineContour {
                        segments: p.segments().to_vec(),
                    })
                    .collect(),
                fill_rule: CanonicalFillRule::NonZero,
                bounds: Some(bounds),
            };
            let centerline = CurvePath::line(Point2::new(0., 5.), Point2::new(10., 5.)).unwrap();
            let profile = vec![StrokeProfileSample {
                location: PathLocation::new(0, 0.).unwrap(),
                center: Point2::new(0., 5.),
                normalized_thickness: 1.,
                width: 1.,
            }];
            CanonicalStroke::new(
                StructuralPathInstanceId::guide_dimension(GuideDimensionId(1), 0, 0),
                None,
                centerline,
                1.,
                PathStrokeStyle::default(),
                profile,
                outline,
            )
            .unwrap()
        }
        /// Supplies marks, stroked holes/self-intersections, and regions with analytic clipped areas.
        /// # Panics
        /// Panics if any current canonical fixture is rejected.
        pub fn cases() -> Vec<(&'static str, GeometryOutput, f64)> {
            let mut cases = Vec::new();
            cases.push((
                "circle-crossing-canvas",
                GeometryOutput::CanonicalMarks(vec![CanonicalMark::Circle {
                    source_site_id: site(0),
                    center: Point2::new(0., 5.),
                    radius: 2.,
                    scope: SiteScope::Canvas,
                    provenance: provenance(),
                    fill_rule: CanonicalFillRule::EvenOdd,
                }]),
                std::f64::consts::PI * 2. / 100.,
            ));
            let bowtie = path(&[[-2., -2.], [12., 12.], [-2., 12.], [12., -2.]]);
            cases.push((
                "self-intersecting-closed-mark",
                GeometryOutput::CanonicalMarks(vec![CanonicalMark::ClosedPath(
                    CanonicalPathMark::new(
                        site(1),
                        bowtie.clone(),
                        SiteScope::Canvas,
                        provenance(),
                        CanonicalFillRule::EvenOdd,
                    )
                    .unwrap(),
                )]),
                0.5,
            ));
            cases.push((
                "self-intersecting-stroke",
                GeometryOutput::CanonicalStrokes(vec![stroke(vec![bowtie])]),
                0.5,
            ));
            let degenerate = path(&[[0., 0.], [5., 0.], [10., 0.]]);
            cases.push((
                "zero-area-touching-boundary",
                GeometryOutput::CanonicalMarks(vec![CanonicalMark::ClosedPath(
                    CanonicalPathMark::new(
                        site(3),
                        degenerate,
                        SiteScope::Canvas,
                        provenance(),
                        CanonicalFillRule::NonZero,
                    )
                    .unwrap(),
                )]),
                0.,
            ));
            let touching: Vec<_> = [
                path(&[[-2., 0.], [5., 0.], [5., 10.], [-2., 10.]]),
                path(&[[5., 0.], [12., 0.], [12., 10.], [5., 10.]]),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                CanonicalMark::ClosedPath(
                    CanonicalPathMark::new(
                        site(i + 4),
                        p,
                        SiteScope::Canvas,
                        provenance(),
                        CanonicalFillRule::NonZero,
                    )
                    .unwrap(),
                )
            })
            .collect();
            cases.push((
                "touching-marks-cover-canvas",
                GeometryOutput::CanonicalMarks(touching),
                1.,
            ));
            let outer = path(&[[-2., -2.], [12., -2.], [12., 12.], [-2., 12.]]);
            let hole_a = path(&[[1., 1.], [1., 6.], [6., 6.], [6., 1.]]);
            let hole_b = path(&[[4., 4.], [4., 9.], [9., 9.], [9., 4.]]);
            cases.push((
                "overlapping-independent-holes",
                GeometryOutput::CanonicalStrokes(vec![
                    stroke(vec![outer.clone(), hole_a]),
                    stroke(vec![outer.clone(), hole_b]),
                ]),
                0.96,
            ));
            let off_outer = path(&[
                [-2.13765432109, -1.73123456789],
                [12.16123456789, -1.73123456789],
                [12.16123456789, 13.31765432109],
                [-2.13765432109, 13.31765432109],
            ]);
            let cx = 4.12345678913;
            let cy = 4.76543219876;
            let dx = 1.23456789123;
            let dy = 1.87654321987;
            let diagonal_hole = path(&[[cx - dx, cy], [cx, cy + dy], [cx + dx, cy], [cx, cy - dy]]);
            cases.push((
                "off-grid-diagonal-hole",
                GeometryOutput::CanonicalStrokes(vec![stroke(vec![
                    off_outer.clone(),
                    diagonal_hole,
                ])]),
                1. - 2. * dx * dy / 100.,
            ));
            let a = 2.13765432109;
            let w = 5.77654321987;
            let y = 4.25987654321;
            let h = 1.07123456789;
            let left = Point2::new(a, y);
            let right = Point2::new(a + w, y);
            let lens = CurvePath::new(
                vec![
                    CurveSegment::CubicBezier(
                        CubicBezierSegment::new(
                            left,
                            Point2::new(a + w / 3., y + h),
                            Point2::new(a + 2. * w / 3., y + h),
                            right,
                        )
                        .unwrap(),
                    ),
                    CurveSegment::CubicBezier(
                        CubicBezierSegment::new(
                            right,
                            Point2::new(a + 2. * w / 3., y - h),
                            Point2::new(a + w / 3., y - h),
                            left,
                        )
                        .unwrap(),
                    ),
                ],
                PathClosure::Closed,
            )
            .unwrap();
            cases.push((
                "off-grid-curved-hole",
                GeometryOutput::CanonicalStrokes(vec![stroke(vec![off_outer.clone(), lens])]),
                1. - w * h / 100.,
            ));
            let thin = path(&[
                [-0.12345678913, 4.12345678913],
                [-0.12345678913, 4.12345680113],
                [2.87654321087, 4.12345680115],
                [2.87654321087, 4.12345678915],
            ]);
            cases.push((
                "off-grid-near-collinear-boundary-hole",
                GeometryOutput::CanonicalStrokes(vec![stroke(vec![off_outer, thin])]),
                1. - 2.87654321087 * 0.000000012 / 100.,
            ));
            let hole = path(&[[2., 2.], [2., 8.], [8., 8.], [8., 2.]]);
            let holed = stroke(vec![outer.clone(), hole]);
            cases.push((
                "duplicate-stroke-with-hole",
                GeometryOutput::CanonicalStrokes(vec![holed.clone(), holed]),
                0.64,
            ));
            for (name, ring, expected) in [
                ("region-covering-canvas", outer, 1.),
                (
                    "region-outside-canvas",
                    path(&[[20., 20.], [21., 20.], [21., 21.], [20., 21.]]),
                    0.,
                ),
            ] {
                let regions = build_canonical_regions(CanonicalRegionProposal {
                    output_layer_id: PatternOutputLayerId(1),
                    source_groups: vec![CanonicalRegionSourceGroup {
                        source_id: CanonicalRegionSourceId::SiteOwners(vec![site(2)]),
                        components: vec![ring],
                    }],
                })
                .unwrap();
                cases.push((name, GeometryOutput::CanonicalRegions(regions), expected));
            }
            cases
        }
    }
}
