//! Output-pixel area coverage for canonical AA-on geometry and opaque same-paint unions.

use super::*;
use i_overlay::{
    core::checked::{CheckedOverlayControl, CheckedOverlayError},
    core::{
        fill_rule::FillRule,
        overlay::ShapeType,
        overlay_rule::OverlayRule,
        solver::{Precision, Solver},
    },
    float::overlay::{FloatOverlay, OverlayOptions},
    i_float::{adapter::FloatPointAdapter, float::rect::FloatRect},
};

type AreaPoint = [f64; 2];
type AreaContour = Vec<AreaPoint>;
type AreaShape = Vec<AreaContour>;
type AreaShapes = Vec<AreaShape>;
const AREA_BOOLEAN_SCALE: f64 = 1.0e12;
const CURVE_PIXEL_TOLERANCE: f64 = 1.0 / 64.0;
const MIN_CIRCLE_SIDES: usize = 40;
// Less than one millionth of one output pixel across an entire layer.
const MAX_OMITTED_PIXEL_AREA: f64 = 1.0e-6;
const AREA_GRID_DISPLACEMENT: f64 = 2.0 / AREA_BOOLEAN_SCALE;

/// One typed primitive before its fill rule is normalized on the checked shared grid.
struct RawAreaPrimitive {
    shape: AreaShape,
    fill_rule: FillRule,
    omitted_area_bound: f64,
}

/// Tracks conservative absolute fill error below representable output-pixel geometry.
#[derive(Default)]
struct AreaOmissionBudget(AreaSum);

impl AreaOmissionBudget {
    /// Charges a conservative area bound across all primitives in one layer.
    ///
    /// # Errors
    /// Rejects nonfinite or cumulatively visible absolute fill error, including lost holes.
    fn record(&mut self, bound: f64) -> Result<(), RenderError> {
        if !bound.is_finite() || bound < 0.0 {
            return Err(area_error(
                "raster.area.precision",
                "omitted area bound is not finite",
            ));
        }
        self.0.add(bound);
        if self.0.value() > MAX_OMITTED_PIXEL_AREA {
            return Err(area_error(
                "raster.area.precision",
                "geometry exceeds the absolute fill approximation budget",
            ));
        }
        Ok(())
    }
}

/// One linear signed-edge contribution limited to a concrete output-pixel row.
#[derive(Clone, Copy)]
struct AreaRowPiece {
    x0: f64,
    x1: f64,
    height: f64,
    sign: f64,
}

/// Accumulates small signed coverage terms without dropping a positive tail.
#[derive(Clone, Copy, Default)]
struct AreaSum {
    total: f64,
    correction: f64,
    absolute: f64,
}

impl AreaSum {
    /// Adds one f64 contribution using Neumaier compensation and records its magnitude.
    fn add(&mut self, value: f64) {
        let next = self.total + value;
        if self.total.abs() >= value.abs() {
            self.correction += (self.total - next) + value;
        } else {
            self.correction += (value - next) + self.total;
        }
        self.total = next;
        self.absolute += value.abs();
    }

    /// Returns the compensated sum without quantizing a small positive value.
    fn value(self) -> f64 {
        self.total + self.correction
    }
}

/// Returns a stable area-consumer error without changing scene or source state.
fn area_error(path: &'static str, message: &'static str) -> RenderError {
    RenderError::new(path, message)
}

/// Maps one canonical fill rule before any cross-primitive union is considered.
fn area_fill_rule(value: CanonicalFillRule) -> FillRule {
    match value {
        CanonicalFillRule::EvenOdd => FillRule::EvenOdd,
        CanonicalFillRule::NonZero => FillRule::NonZero,
    }
}

/// Appends one finite point while charging the caller's original request-wide edge budget.
///
/// # Errors
/// Returns cancellation, edge-budget, coordinate, or allocation diagnostics.
fn area_push_point(
    points: &mut AreaContour,
    point: Point2,
    work: &mut RasterWork<'_>,
) -> Result<(), RenderError> {
    work.edge()?;
    if !point.is_finite() {
        return Err(area_error(
            "raster.area.coordinate",
            "output-pixel point is not finite",
        ));
    }
    points
        .try_reserve(1)
        .map_err(|_| area_error("raster.allocation", "area contour allocation failed"))?;
    points.push([point.x, point.y]);
    Ok(())
}

/// Flattens an authored cubic in output pixels, retaining small curved features by a local extent tolerance.
///
/// # Errors
/// Returns cancellation, numeric nonconvergence, edge-budget, or allocation diagnostics.
fn area_flatten_cubic(
    p: [Point2; 4],
    tolerance: f64,
    depth: u8,
    points: &mut AreaContour,
    work: &mut RasterWork<'_>,
) -> Result<(), RenderError> {
    work.check()?;
    let flatness = point_line_distance(p[1], p[0], p[3]).max(point_line_distance(p[2], p[0], p[3]));
    if flatness <= tolerance || control_polygon_extent(p) <= tolerance {
        return area_push_point(points, p[3], work);
    }
    let a = midpoint(p[0], p[1]);
    let b = midpoint(p[1], p[2]);
    let c = midpoint(p[2], p[3]);
    let d = midpoint(a, b);
    let e = midpoint(b, c);
    let f = midpoint(d, e);
    if depth >= 60 || f == p[0] || f == p[3] {
        return Err(area_error(
            "raster.flatten.numeric",
            "area cubic cannot meet output-pixel tolerance",
        ));
    }
    area_flatten_cubic([p[0], a, d, f], tolerance, depth + 1, points, work)?;
    area_flatten_cubic([f, e, c, p[3]], tolerance, depth + 1, points, work)
}

/// Converts one exact canonical contour to a closed output-pixel polygon without changing its fill rule.
///
/// # Errors
/// Returns cancellation, flattening, edge-budget, coordinate, or allocation diagnostics.
fn area_contour(
    segments: &[CurveSegment],
    transform: CanonicalRasterTransform,
    work: &mut RasterWork<'_>,
) -> Result<Option<AreaContour>, RenderError> {
    let Some(first) = segments.first() else {
        return Ok(None);
    };
    let mut points = Vec::new();
    let start = transform.point(first.start());
    work.check()?;
    if !start.is_finite() {
        return Err(area_error(
            "raster.area.coordinate",
            "output-pixel point is not finite",
        ));
    }
    points
        .try_reserve(1)
        .map_err(|_| area_error("raster.allocation", "area contour allocation failed"))?;
    points.push([start.x, start.y]);
    for segment in segments {
        work.check()?;
        match segment {
            CurveSegment::Line(line) => {
                area_push_point(&mut points, transform.point(line.end()), work)?
            }
            CurveSegment::CubicBezier(cubic) => {
                let p = [
                    transform.point(cubic.start()),
                    transform.point(cubic.control_1()),
                    transform.point(cubic.control_2()),
                    transform.point(cubic.end()),
                ];
                let extent = control_polygon_extent(p);
                // An absolute one-pixel rule alone can flatten a tiny positive cap to a line.
                let tolerance = CURVE_PIXEL_TOLERANCE
                    .min(extent / 64.0)
                    .max(f64::MIN_POSITIVE);
                area_flatten_cubic(p, tolerance, 0, &mut points, work)?;
            }
        }
    }
    if points.last() == points.first() {
        points.pop();
    }
    if points.len() < 3 {
        if segments
            .iter()
            .any(|segment| matches!(segment, CurveSegment::CubicBezier(_)))
        {
            return Err(area_error(
                "raster.area.precision",
                "curved contour collapsed below output-pixel precision",
            ));
        }
        // A one- or two-point line-only closed contour has exactly zero area.
        return Ok(None);
    }
    Ok(Some(points))
}

/// Tessellates a positive transformed circle with bounded absolute sagitta and sub-1% relative area error.
///
/// # Errors
/// Returns cancellation, representability, edge-budget, or allocation diagnostics.
fn area_circle(
    center: Point2,
    radius: f64,
    transform: CanonicalRasterTransform,
    work: &mut RasterWork<'_>,
) -> Result<(Option<AreaShape>, f64), RenderError> {
    if radius == 0.0 {
        return Ok((None, 0.0));
    }
    let mapped = transform.point(center);
    let rx = radius * transform.scale_x;
    let ry = radius * transform.scale_y;
    if !mapped.is_finite() || !rx.is_finite() || !ry.is_finite() || rx < 0.0 || ry < 0.0 {
        return Err(area_error(
            "raster.area.circle",
            "positive circle cannot be mapped to finite output pixels",
        ));
    }
    if rx == 0.0 || ry == 0.0 {
        return Ok((None, 4.0 * rx * ry));
    }
    let largest = rx.max(ry);
    let ratio = CURVE_PIXEL_TOLERANCE / (2.0 * largest);
    if ratio == 0.0 {
        return Err(area_error(
            "raster.area.circle",
            "circle sagitta cannot be represented at output scale",
        ));
    }
    let angle = 2.0 * ratio.min(1.0).sqrt().asin();
    let count = (std::f64::consts::PI / angle)
        .ceil()
        .max(MIN_CIRCLE_SIDES as f64);
    if !count.is_finite() || count > usize::MAX as f64 {
        return Err(area_error(
            "raster.area.circle",
            "circle polygon count is not representable",
        ));
    }
    let sides = count as usize;
    if sides > work.remaining_edges {
        return Err(area_error(
            "raster.limits.flattened_edges",
            "flattened raster edge limit exceeded",
        ));
    }
    let mut contour = Vec::new();
    contour
        .try_reserve_exact(sides)
        .map_err(|_| area_error("raster.allocation", "circle contour allocation failed"))?;
    for index in 0..sides {
        work.check()?;
        let theta = std::f64::consts::TAU * index as f64 / sides as f64;
        area_push_point(
            &mut contour,
            Point2::new(mapped.x + rx * theta.cos(), mapped.y + ry * theta.sin()),
            work,
        )?;
    }
    if contour.iter().all(|point| *point == contour[0]) {
        return Ok((None, 4.0 * rx * ry));
    }
    Ok((Some(vec![contour]), 4.0 * rx * ry))
}

/// Rejects only circles whose finite transformed bounds lie strictly outside visible output pixels.
///
/// A tangent or sub-ULP positive circle remains in precision accounting; this only avoids
/// tessellating geometry that cannot contribute to the target or its fitted preview clip.
fn area_circle_outside_visible(
    center: Point2,
    radius: f64,
    transform: CanonicalRasterTransform,
    visible: [f64; 4],
) -> bool {
    let mapped = transform.point(center);
    let rx = radius * transform.scale_x;
    let ry = radius * transform.scale_y;
    mapped.is_finite()
        && rx.is_finite()
        && ry.is_finite()
        && rx >= 0.0
        && ry >= 0.0
        && (mapped.x + rx < visible[0]
            || mapped.y + ry < visible[1]
            || mapped.x - rx > visible[2]
            || mapped.y - ry > visible[3])
}

/// Intersects the output canvas with the optional fitted-preview geometric clip.
fn area_visible_bounds(width: u32, height: u32, clip: Option<[f64; 4]>) -> [f64; 4] {
    let [left, top, right, bottom] = clip.unwrap_or([0.0, 0.0, width as f64, height as f64]);
    [
        left.max(0.0),
        top.max(0.0),
        right.min(width as f64),
        bottom.min(height as f64),
    ]
}

/// Visits primitives in their canonical painter order, preserving each original paint index.
///
/// # Errors
/// Returns cancellation, flattening, edge-budget, coordinate, or allocation diagnostics.
fn for_each_area_primitive(
    geometry: &GeometryOutput,
    transform: CanonicalRasterTransform,
    visible: [f64; 4],
    work: &mut RasterWork<'_>,
    mut visit: impl FnMut(usize, RawAreaPrimitive, &mut RasterWork<'_>) -> Result<(), RenderError>,
) -> Result<usize, RenderError> {
    let mut count = 0;
    match geometry {
        GeometryOutput::CircularMarks(marks) => {
            for (index, mark) in marks.iter().enumerate() {
                if area_circle_outside_visible(mark.center, mark.radius, transform, visible) {
                    count += 1;
                    continue;
                }
                let (shape, omitted_area_bound) =
                    area_circle(mark.center, mark.radius, transform, work)?;
                if shape.is_some() || omitted_area_bound > 0.0 {
                    visit(
                        index,
                        RawAreaPrimitive {
                            shape: shape.unwrap_or_default(),
                            fill_rule: FillRule::NonZero,
                            omitted_area_bound,
                        },
                        work,
                    )?;
                }
                count += 1;
            }
        }
        GeometryOutput::CanonicalMarks(marks) => {
            for (index, mark) in marks.iter().enumerate() {
                let raw = match mark {
                    CanonicalMark::Circle { center, radius, .. } => {
                        if area_circle_outside_visible(*center, *radius, transform, visible) {
                            count += 1;
                            continue;
                        }
                        let (shape, omitted_area_bound) =
                            area_circle(*center, *radius, transform, work)?;
                        (shape.is_some() || omitted_area_bound > 0.0).then(|| RawAreaPrimitive {
                            shape: shape.unwrap_or_default(),
                            fill_rule: FillRule::NonZero,
                            omitted_area_bound,
                        })
                    }
                    CanonicalMark::ClosedPath(mark) => {
                        area_contour(mark.path.segments(), transform, work)?.map(|contour| {
                            RawAreaPrimitive {
                                shape: vec![contour],
                                fill_rule: area_fill_rule(mark.fill_rule),
                                omitted_area_bound: 0.0,
                            }
                        })
                    }
                };
                if let Some(raw) = raw {
                    visit(index, raw, work)?;
                }
                count += 1;
            }
        }
        GeometryOutput::CanonicalStrokes(strokes) => {
            for (index, stroke) in strokes.iter().enumerate() {
                let mut shape = Vec::new();
                for contour in &stroke.outline.contours {
                    if let Some(contour) = area_contour(&contour.segments, transform, work)? {
                        shape.try_reserve(1).map_err(|_| {
                            area_error("raster.allocation", "stroke contour list allocation failed")
                        })?;
                        shape.push(contour);
                    }
                }
                if !shape.is_empty() {
                    visit(
                        index,
                        RawAreaPrimitive {
                            shape,
                            fill_rule: area_fill_rule(stroke.outline.fill_rule),
                            omitted_area_bound: 0.0,
                        },
                        work,
                    )?;
                }
                count += 1;
            }
        }
        GeometryOutput::CanonicalRegions(regions) => {
            for (index, region) in regions.regions().iter().enumerate() {
                if let Some(contour) = area_contour(region.ring.segments(), transform, work)? {
                    visit(
                        index,
                        RawAreaPrimitive {
                            shape: vec![contour],
                            fill_rule: FillRule::NonZero,
                            omitted_area_bound: 0.0,
                        },
                        work,
                    )?;
                }
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Builds one conservative, shared i64 adapter over actual run geometry and optional fitted-canvas clip.
///
/// # Errors
/// Returns a coordinate or grid representability diagnostic instead of lowering precision.
fn area_adapter(
    raw: &[RawAreaPrimitive],
    clip: Option<[f64; 4]>,
    work: &RasterWork<'_>,
) -> Result<FloatRect<f64>, RenderError> {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (index, point) in raw
        .iter()
        .flat_map(|primitive| primitive.shape.iter())
        .flat_map(|contour| contour.iter())
        .enumerate()
    {
        if index & 1023 == 0 {
            work.check()?;
        }
        min_x = min_x.min(point[0]);
        max_x = max_x.max(point[0]);
        min_y = min_y.min(point[1]);
        max_y = max_y.max(point[1]);
    }
    if let Some([left, top, right, bottom]) = clip {
        min_x = min_x.min(left);
        max_x = max_x.max(right);
        min_y = min_y.min(top);
        max_y = max_y.max(bottom);
    }
    let rect = FloatRect::new(min_x, max_x, min_y, max_y).map_err(|_| {
        area_error(
            "raster.area.coordinate",
            "area bounds are not finite and ordered",
        )
    })?;
    FloatPointAdapter::<AreaPoint, i64>::try_with_scale_conservative(rect, AREA_BOOLEAN_SCALE)
        .map_err(|_| {
            area_error(
                "raster.area.precision",
                "area Boolean grid cannot represent geometry bounds",
            )
        })?;
    Ok(rect)
}

/// Bounds the input-edge area swept by the checked grid neighborhood.
///
/// A fill that is exactly empty may still have a nonzero swept-edge bound; charging that small
/// uncertainty avoids treating its entire bounding box as positive geometry. This is an input
/// snapping bound, not a universal displacement guarantee for Boolean topology repair.
///
/// # Errors
/// Returns cancellation or a nonfinite input-edge bound diagnostic.
fn area_perturbation_bound(
    shape: &[AreaContour],
    work: &RasterWork<'_>,
) -> Result<f64, RenderError> {
    let mut perimeter = AreaSum::default();
    let mut edges = 0_usize;
    for contour in shape {
        for index in 0..contour.len() {
            if edges & 1023 == 0 {
                work.check()?;
            }
            let a = contour[index];
            let b = contour[(index + 1) % contour.len()];
            perimeter.add((b[0] - a[0]).hypot(b[1] - a[1]));
            edges = edges
                .checked_add(1)
                .ok_or(area_error("raster.area.work", "area edge count overflows"))?;
        }
    }
    let bound = 2.0 * AREA_GRID_DISPLACEMENT * perimeter.value()
        + std::f64::consts::PI * AREA_GRID_DISPLACEMENT.powi(2) * edges as f64;
    if !bound.is_finite() {
        return Err(area_error(
            "raster.area.precision",
            "snapped-edge area bound is not finite",
        ));
    }
    Ok(bound)
}

/// Certifies original coincident contours under their typed fill, charging every search step.
///
/// Only opposite traversals cancel NonZero; either traversal cancels EvenOdd. Original f64
/// equality is required, so snapped coincidence cannot exempt approximation error.
///
/// # Errors
/// Returns cancellation or the caller's shared work-limit diagnostic for an ambiguous search.
fn exact_contours_cancel(
    a: &AreaContour,
    b: &AreaContour,
    rule: FillRule,
    work: &mut RasterWork<'_>,
) -> Result<bool, RenderError> {
    work.edge()?;
    if a.len() != b.len() || a.is_empty() {
        return Ok(false);
    }
    let n = a.len();
    for (offset, point) in b.iter().enumerate() {
        work.edge()?;
        if *point != a[0] {
            continue;
        }
        for reverse in [true, false] {
            if !reverse && rule != FillRule::EvenOdd {
                continue;
            }
            let mut equal = true;
            for (index, point) in a.iter().enumerate() {
                work.edge()?;
                let other = if reverse {
                    (offset + n - index) % n
                } else {
                    (offset + index) % n
                };
                if *point != b[other] {
                    equal = false;
                    break;
                }
            }
            if equal {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Detects a contour lacking any noncollinear triangle on the actual shared Boolean grid.
///
/// This narrow check does not certify interactions between individually nondegenerate rings.
///
/// # Errors
/// Returns cancellation, checked grid conversion, or integer determinant overflow diagnostics.
fn area_contour_collapses(
    contour: &AreaContour,
    adapter: &FloatPointAdapter<AreaPoint, i64>,
    work: &RasterWork<'_>,
) -> Result<bool, RenderError> {
    let mut origin = None;
    let mut direction = None;
    for (index, point) in contour.iter().enumerate() {
        if index & 1023 == 0 {
            work.check()?;
        }
        let point = adapter.try_float_to_int(point).map_err(|_| {
            area_error(
                "raster.area.precision",
                "contour point cannot be mapped to the Boolean grid",
            )
        })?;
        let Some(first) = origin else {
            origin = Some(point);
            continue;
        };
        let dx = i128::from(point.x) - i128::from(first.x);
        let dy = i128::from(point.y) - i128::from(first.y);
        let Some((bx, by)) = direction else {
            if dx != 0 || dy != 0 {
                direction = Some((dx, dy));
            }
            continue;
        };
        let determinant = dx
            .checked_mul(by)
            .and_then(|left| dy.checked_mul(bx).and_then(|right| left.checked_sub(right)))
            .ok_or(area_error(
                "raster.area.precision",
                "Boolean grid triangle exceeds checked integer bounds",
            ))?;
        if determinant != 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Bounds independently collapsed contours, pairing only exact original typed cancellations.
///
/// Healthy contours add no global perimeter charge. Losing a hole can add paint, so the swept
/// edge charge bounds absolute fill error rather than only missing positive components.
///
/// # Errors
/// Returns allocation, cancellation, shared work-limit, coordinate, or precision diagnostics.
fn area_collapsed_contour_bound(
    shape: &AreaShape,
    rule: FillRule,
    rect: FloatRect<f64>,
    work: &mut RasterWork<'_>,
) -> Result<f64, RenderError> {
    let adapter =
        FloatPointAdapter::<AreaPoint, i64>::try_with_scale_conservative(rect, AREA_BOOLEAN_SCALE)
            .map_err(|_| {
                area_error(
                    "raster.area.precision",
                    "contour grid cannot represent area bounds",
                )
            })?;
    let mut collapsed = Vec::new();
    for (index, contour) in shape.iter().enumerate() {
        work.check()?;
        if area_contour_collapses(contour, &adapter, work)? {
            collapsed.try_reserve(1).map_err(|_| {
                area_error("raster.allocation", "collapsed contour allocation failed")
            })?;
            collapsed.push(index);
        }
    }
    let mut bound = AreaSum::default();
    while let Some(index) = collapsed.pop() {
        let mut partner = None;
        for (position, other) in collapsed.iter().enumerate() {
            if exact_contours_cancel(&shape[index], &shape[*other], rule, work)? {
                partner = Some(position);
                break;
            }
        }
        if let Some(position) = partner {
            collapsed.swap_remove(position);
        } else {
            bound.add(area_perturbation_bound(
                std::slice::from_ref(&shape[index]),
                work,
            )?);
        }
    }
    Ok(bound.value())
}

/// Applies one typed fill with bounded checked retries on the run's common integer grid.
///
/// # Errors
/// Returns Boolean or precision diagnostics; there is no minimum positive-area threshold.
fn area_overlay(
    shape: &[AreaShape],
    rect: FloatRect<f64>,
    rule: FillRule,
    work: &mut RasterWork<'_>,
    omissions: &mut AreaOmissionBudget,
) -> Result<AreaShapes, RenderError> {
    work.check()?;
    let adapter =
        FloatPointAdapter::<AreaPoint, i64>::try_with_scale_conservative(rect, AREA_BOOLEAN_SCALE)
            .map_err(|_| {
                area_error(
                    "raster.area.precision",
                    "area Boolean grid cannot represent geometry bounds",
                )
            })?;
    let mut options = OverlayOptions::<f64, i64>::default();
    options.min_output_area = 0.0;
    options.clean_result = false;
    let solver = Solver {
        precision: Precision::ABSOLUTE,
        ..Solver::AUTO
    };
    let edge_count = shape
        .iter()
        .flat_map(|part| part.iter())
        .try_fold(0_usize, |sum, contour| sum.checked_add(contour.len()))
        .ok_or(area_error("raster.area.work", "area edge count overflows"))?;
    let mut overlay =
        FloatOverlay::<AreaPoint, i64>::new_custom(adapter, options, solver, edge_count)
            .unsafe_add_source(shape, ShapeType::Subject);
    area_checked_overlay(&mut overlay, OverlayRule::Subject, rule, work, omissions)
}

/// Charges accepted split-mutation uncertainty and all Boolean attempts before returning shapes.
///
/// The exact doubled-area certificate concerns the already-quantized integer input. Initial float
/// quantization, curve flattening, and upstream graph extraction are separate authority boundaries.
/// Healthy unchanged contours receive no global perimeter charge. Overflow and bounded convergence
/// failures discard local geometry; no partial pixels are published.
///
/// # Errors
/// Returns cancellation, Boolean work exhaustion, precision, or cumulative layer-error diagnostics.
fn area_checked_overlay(
    overlay: &mut FloatOverlay<AreaPoint, i64>,
    overlay_rule: OverlayRule,
    fill_rule: FillRule,
    work: &mut RasterWork<'_>,
    omissions: &mut AreaOmissionBudget,
) -> Result<AreaShapes, RenderError> {
    work.check()?;
    let mut control =
        CheckedOverlayControl::new(work.remaining_boolean_operations, work.is_cancelled);
    let result = overlay.overlay_checked(overlay_rule, fill_rule, &mut control);
    work.remaining_boolean_operations -= control.operations();
    let (shapes, doubled_area) = result.map_err(|error| match error {
        CheckedOverlayError::Cancelled => {
            area_error("evaluation.cancelled", "rasterization was cancelled")
        }
        CheckedOverlayError::WorkLimit => area_error(
            "raster.limits.boolean_operations",
            "Boolean operation limit exceeded",
        ),
        CheckedOverlayError::NonConvergent => area_error(
            "raster.area.precision",
            "Boolean splitting did not converge within bounded precision attempts",
        ),
        CheckedOverlayError::UncertifiedCleanup => area_error(
            "raster.area.precision",
            "Boolean output cleanup has no split certificate",
        ),
        CheckedOverlayError::CertificateOverflow => area_error(
            "raster.area.precision",
            "Boolean split certificate exceeds checked integer bounds",
        ),
    })?;
    work.check()?;
    if doubled_area != 0 {
        // Both conversion and division round upward; denominator rounds downward. No cancellation
        // between absolute per-split triangles can reduce this accumulated uncertainty.
        let numerator = (doubled_area as f64).next_up();
        let denominator = (2.0 * AREA_BOOLEAN_SCALE * AREA_BOOLEAN_SCALE).next_down();
        omissions.record((numerator / denominator).next_up())?;
    }
    Ok(shapes)
}

/// Normalizes every primitive independently, then unions only their already-positive fills.
///
/// Independently grid-collapsed contours consume the layer's absolute fill-error budget even
/// beside retained paint. Checked split-mutation certificates share that same cumulative budget.
///
/// # Errors
/// Returns cancellation, precision, allocation, or Boolean diagnostics without painting partial pixels.
fn normalize_and_union(
    raw: &[RawAreaPrimitive],
    clip: Option<[f64; 4]>,
    work: &mut RasterWork<'_>,
    omissions: &mut AreaOmissionBudget,
) -> Result<AreaShapes, RenderError> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let mut omitted = AreaSum::default();
    for primitive in raw.iter().filter(|primitive| primitive.shape.is_empty()) {
        omitted.add(primitive.omitted_area_bound);
    }
    if !raw.iter().any(|primitive| !primitive.shape.is_empty()) {
        omissions.record(omitted.value())?;
        return Ok(Vec::new());
    }
    let rect = area_adapter(raw, clip, work)?;
    let mut positive = Vec::new();
    for primitive in raw.iter().filter(|primitive| !primitive.shape.is_empty()) {
        work.check()?;
        let mut bound =
            area_collapsed_contour_bound(&primitive.shape, primitive.fill_rule, rect, work)?;
        let normalized = area_overlay(
            std::slice::from_ref(&primitive.shape),
            rect,
            primitive.fill_rule,
            work,
            omissions,
        )?;
        let exact_empty = normalized.is_empty()
            && primitive.shape.len() == 2
            && exact_contours_cancel(
                &primitive.shape[0],
                &primitive.shape[1],
                primitive.fill_rule,
                work,
            )?;
        if normalized.is_empty() && !exact_empty {
            bound = bound
                .max(primitive.omitted_area_bound)
                .max(area_perturbation_bound(&primitive.shape, work)?);
        }
        omitted.add(bound);
        positive.try_reserve(normalized.len()).map_err(|_| {
            area_error(
                "raster.allocation",
                "normalized area shape allocation failed",
            )
        })?;
        positive.extend(normalized);
    }
    omissions.record(omitted.value())?;
    work.check()?;
    let mut unioned = area_overlay(&positive, rect, FillRule::NonZero, work, omissions)?;
    if let Some([left, top, right, bottom]) = clip {
        work.check()?;
        let rectangle = vec![vec![vec![
            [left, top],
            [right, top],
            [right, bottom],
            [left, bottom],
        ]]];
        let adapter = FloatPointAdapter::<AreaPoint, i64>::try_with_scale_conservative(
            rect,
            AREA_BOOLEAN_SCALE,
        )
        .map_err(|_| {
            area_error(
                "raster.area.precision",
                "clip grid cannot represent area bounds",
            )
        })?;
        let mut options = OverlayOptions::<f64, i64>::default();
        options.min_output_area = 0.0;
        options.clean_result = false;
        let mut overlay = FloatOverlay::<AreaPoint, i64>::new_custom(
            adapter,
            options,
            Solver {
                precision: Precision::ABSOLUTE,
                ..Solver::AUTO
            },
            4,
        )
        .unsafe_add_source(&unioned, ShapeType::Subject)
        .unsafe_add_source(&rectangle, ShapeType::Clip);
        unioned = area_checked_overlay(
            &mut overlay,
            OverlayRule::Intersect,
            FillRule::NonZero,
            work,
            omissions,
        )?;
    }
    Ok(unioned)
}

/// Computes one contour's recentered signed Cartesian area for stable outer/hole orientation.
fn area_signed_contour(contour: &[AreaPoint], work: &RasterWork<'_>) -> Result<f64, RenderError> {
    if contour.len() < 3 {
        return Ok(0.0);
    }
    let origin = contour[0];
    let mut sum = AreaSum::default();
    for index in 0..contour.len() {
        if index & 1023 == 0 {
            work.check()?;
        }
        let a = contour[index];
        let b = contour[(index + 1) % contour.len()];
        sum.add((a[0] - origin[0]) * (b[1] - origin[1]) - (b[0] - origin[0]) * (a[1] - origin[1]));
    }
    Ok(sum.value() * 0.5)
}

/// Integrates one affine directed edge's clamped contribution across a pixel row exactly in f64.
fn area_edge_right(piece: AreaRowPiece, left: f64) -> f64 {
    let right = left + 1.0;
    let mut cuts = [0.0, 1.0, 1.0, 1.0];
    let mut count = 2;
    if piece.x0 != piece.x1 {
        for boundary in [left, right] {
            let t = (boundary - piece.x0) / (piece.x1 - piece.x0);
            if t > 0.0 && t < 1.0 {
                cuts[count] = t;
                count += 1;
            }
        }
    }
    cuts[..count].sort_by(f64::total_cmp);
    let mut sum = AreaSum::default();
    for pair in cuts[..count].windows(2) {
        let a = pair[0];
        let b = pair[1];
        if b <= a {
            continue;
        }
        let middle = (a + b) * 0.5;
        let edge_x = piece.x0 + (piece.x1 - piece.x0) * middle;
        let value = if edge_x <= left {
            1.0
        } else if edge_x >= right {
            0.0
        } else {
            right - edge_x
        };
        sum.add((b - a) * value);
    }
    piece.height * sum.value()
}

/// Visits positive geometric pixel areas only inside the shapes' target-clipped bounding box.
///
/// Inner row work and prefix accumulation remain f64 until the outer caller applies its paint.
///
/// # Errors
/// Returns cancellation, finite-coordinate, allocation, or out-of-range area diagnostics.
fn for_each_area_pixel(
    shapes: &AreaShapes,
    width: u32,
    height: u32,
    work: &mut RasterWork<'_>,
    paint: impl FnMut(usize, f64),
) -> Result<(), RenderError> {
    for_each_area_pixel_in_rows(shapes, width, height, 0, height, work, paint)
}

/// Integrates unchanged global geometry only in one half-open global pixel-row interval.
///
/// Row bins cover this interval only. Floating-point coordinates, contour orientation,
/// and edge arithmetic retain the full target's coordinate system. Returned indices address
/// local storage beginning at `first_row`, so complete-image indexing need not fit `usize`.
///
/// # Errors
/// Returns cancellation, allocation, coordinate, or coverage diagnostics without changing geometry.
fn for_each_area_pixel_in_rows(
    shapes: &AreaShapes,
    width: u32,
    height: u32,
    first_row: u32,
    end_row: u32,
    work: &mut RasterWork<'_>,
    mut paint: impl FnMut(usize, f64),
) -> Result<(), RenderError> {
    if shapes.is_empty() {
        return Ok(());
    }
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (index, point) in shapes
        .iter()
        .flat_map(|shape| shape.iter())
        .flat_map(|contour| contour.iter())
        .enumerate()
    {
        if index & 1023 == 0 {
            work.check()?;
        }
        min_x = min_x.min(point[0]);
        max_x = max_x.max(point[0]);
        min_y = min_y.min(point[1]);
        max_y = max_y.max(point[1]);
    }
    if ![min_x, max_x, min_y, max_y].into_iter().all(f64::is_finite) {
        return Err(area_error(
            "raster.area.coordinate",
            "normalized area bounds are not finite",
        ));
    }
    let x0 = min_x.floor().clamp(0.0, f64::from(width)) as usize;
    let x1 = max_x.ceil().clamp(0.0, f64::from(width)) as usize;
    let y0 = (min_y.floor().clamp(0.0, f64::from(height)) as usize).max(first_row as usize);
    let y1 = (max_y.ceil().clamp(0.0, f64::from(height)) as usize).min(end_row as usize);
    if x0 >= x1 || y0 >= y1 {
        return Ok(());
    }
    let crop_width = x1 - x0;
    let crop_height = y1 - y0;
    let mut rows: Vec<Vec<AreaRowPiece>> = Vec::new();
    rows.try_reserve_exact(crop_height)
        .map_err(|_| area_error("raster.allocation", "area row-bin allocation failed"))?;
    rows.resize_with(crop_height, Vec::new);
    for shape in shapes {
        work.check()?;
        let outer = shape.first().ok_or(area_error(
            "raster.area.shape",
            "normalized area shape is empty",
        ))?;
        let outer_area = area_signed_contour(outer, work)?;
        if !outer_area.is_finite() || outer_area == 0.0 {
            return Err(area_error(
                "raster.area.shape",
                "normalized outer contour has no finite area",
            ));
        }
        let outer_sign = outer_area.signum();
        for contour in shape {
            if contour.len() < 3 {
                return Err(area_error(
                    "raster.area.shape",
                    "normalized area contour has fewer than three vertices",
                ));
            }
            for index in 0..contour.len() {
                if index & 1023 == 0 {
                    work.check()?;
                }
                let a = contour[index];
                let b = contour[(index + 1) % contour.len()];
                if a[1] == b[1] {
                    continue;
                }
                let low = a[1].min(b[1]);
                let high = a[1].max(b[1]);
                let first = low.floor().max(y0 as f64).min(y1 as f64) as usize;
                let last = high.ceil().max(y0 as f64).min(y1 as f64) as usize;
                let sign = if b[1] > a[1] { -outer_sign } else { outer_sign };
                for row in first..last {
                    work.check()?;
                    let ya = low.max(row as f64);
                    let yb = high.min((row + 1) as f64);
                    if yb <= ya {
                        continue;
                    }
                    let slope = (b[0] - a[0]) / (b[1] - a[1]);
                    let xa = a[0] + (ya - a[1]) * slope;
                    let xb = a[0] + (yb - a[1]) * slope;
                    if !xa.is_finite() || !xb.is_finite() {
                        return Err(area_error(
                            "raster.area.coordinate",
                            "area row edge is not finite",
                        ));
                    }
                    let bin = &mut rows[row - y0];
                    bin.try_reserve(1).map_err(|_| {
                        area_error("raster.allocation", "area row-piece allocation failed")
                    })?;
                    bin.push(AreaRowPiece {
                        x0: xa,
                        x1: xb,
                        height: yb - ya,
                        sign,
                    });
                }
            }
        }
    }
    let mut partial = Vec::new();
    partial
        .try_reserve_exact(crop_width)
        .map_err(|_| area_error("raster.allocation", "area partial-row allocation failed"))?;
    partial.resize(crop_width, AreaSum::default());
    let mut events = Vec::new();
    events
        .try_reserve_exact(crop_width + 1)
        .map_err(|_| area_error("raster.allocation", "area prefix-row allocation failed"))?;
    events.resize(crop_width + 1, AreaSum::default());
    for (row, pieces) in rows.iter().enumerate() {
        work.check()?;
        if pieces.is_empty() {
            continue;
        }
        let left = pieces
            .iter()
            .map(|piece| piece.x0.min(piece.x1))
            .fold(f64::INFINITY, f64::min);
        let right = pieces
            .iter()
            .map(|piece| piece.x0.max(piece.x1))
            .fold(f64::NEG_INFINITY, f64::max);
        let start = left.floor().max(x0 as f64).min(x1 as f64) as usize;
        let end = right.ceil().max(x0 as f64).min(x1 as f64) as usize;
        if start >= end {
            continue;
        }
        for piece in pieces {
            let first = piece.x0.min(piece.x1).floor().max(x0 as f64).min(x1 as f64) as usize;
            let last = piece.x0.max(piece.x1).ceil().max(x0 as f64).min(x1 as f64) as usize;
            for x in first..last {
                if x & 1023 == 0 {
                    work.check()?;
                }
                partial[x - x0].add(piece.sign * area_edge_right(*piece, x as f64));
            }
            if last < x1 {
                events[last - x0].add(piece.sign * piece.height);
            }
        }
        let mut prefix = AreaSum::default();
        for x in start..end {
            if x & 1023 == 0 {
                work.check()?;
            }
            prefix.add(events[x - x0].value());
            let mut sum = partial[x - x0];
            sum.add(prefix.value());
            let raw = sum.value();
            let roundoff = 64.0 * f64::EPSILON * (1.0 + sum.absolute + prefix.absolute);
            if !raw.is_finite() || raw < -roundoff || raw > 1.0 + roundoff {
                return Err(area_error(
                    "raster.area.coverage",
                    "geometric pixel area is outside one pixel",
                ));
            }
            let coverage = raw.clamp(0.0, 1.0);
            if coverage > 0.0 {
                paint(
                    (y0 + row - first_row as usize) * width as usize + x,
                    coverage,
                );
            }
            partial[x - x0] = AreaSum::default();
            events[x - x0] = AreaSum::default();
        }
        events[end - x0] = AreaSum::default();
    }
    Ok(())
}

/// Paints one typed shape after independent fill normalization with its original ordered paint.
///
/// # Errors
/// Returns cancellation, precision, allocation, or area diagnostics without publishing a final surface.
#[expect(
    clippy::too_many_arguments,
    reason = "ordered paint retains explicit target, paint, work, and omission state"
)]
fn paint_ordered_primitive(
    pixels: &mut Vec<PremultipliedLinearPixel>,
    raw: RawAreaPrimitive,
    width: u32,
    height: u32,
    clip: Option<[f64; 4]>,
    color: &ColorValue,
    opacity: f64,
    work: &mut RasterWork<'_>,
    omissions: &mut AreaOmissionBudget,
) -> Result<(), RenderError> {
    let shapes = normalize_and_union(&[raw], clip, work, omissions)?;
    for_each_area_pixel(&shapes, width, height, work, |index, coverage| {
        pixels.paint_shape(index, coverage, color, opacity)
    })
}

/// Paints a maximal contiguous same-color opaque output run exactly once after geometric union.
///
/// # Errors
/// Returns cancellation, precision, allocation, or area diagnostics before exposing a final raster.
#[expect(
    clippy::too_many_arguments,
    reason = "opaque union retains explicit target, transform, work, and omission state"
)]
fn paint_opaque_run(
    pixels: &mut Vec<PremultipliedLinearPixel>,
    outputs: &[&RenderOutputLayer],
    layer: &RenderLayer,
    width: u32,
    height: u32,
    transform: CanonicalRasterTransform,
    clip: Option<[f64; 4]>,
    work: &mut RasterWork<'_>,
    omissions: &mut AreaOmissionBudget,
) -> Result<(), RenderError> {
    let (unioned, units) =
        prepare_opaque_run(outputs, width, height, transform, clip, work, omissions)?;
    for_each_area_pixel(&unioned, width, height, work, |index, coverage| {
        pixels.paint_shape(index, coverage, &layer.color, 1.0)
    })?;
    for _ in 0..units {
        work.primitive();
    }
    Ok(())
}

/// Transforms, flattens, and unions one opaque run once in the complete target coordinates.
///
/// # Errors
/// Returns shared request-wide precision, edge, Boolean-work, cancellation, or allocation errors.
fn prepare_opaque_run(
    outputs: &[&RenderOutputLayer],
    width: u32,
    height: u32,
    transform: CanonicalRasterTransform,
    clip: Option<[f64; 4]>,
    work: &mut RasterWork<'_>,
    omissions: &mut AreaOmissionBudget,
) -> Result<(AreaShapes, usize), RenderError> {
    if outputs.is_empty() {
        return Ok((Vec::new(), 0));
    }
    let mut raw = Vec::new();
    let mut units = 0_usize;
    let visible = area_visible_bounds(width, height, clip);
    for output in outputs {
        units = units
            .checked_add(for_each_area_primitive(
                output.geometry(),
                transform,
                visible,
                work,
                |_, primitive, work| {
                    work.check()?;
                    raw.try_reserve(1).map_err(|_| {
                        area_error(
                            "raster.allocation",
                            "opaque primitive list allocation failed",
                        )
                    })?;
                    raw.push(primitive);
                    Ok(())
                },
            )?)
            .ok_or(area_error(
                "raster.area.work",
                "opaque primitive progress count overflows",
            ))?;
    }
    let unioned = normalize_and_union(&raw, clip, work, omissions)?;
    Ok((unioned, units))
}

/// Retains normalized global AA geometry in its original paint order for row-band integration.
pub(super) struct PreparedAreaLayer {
    paints: Vec<PreparedAreaPaint>,
}

/// Associates one normalized primitive or opaque union with its original channel paint.
struct PreparedAreaPaint {
    shapes: AreaShapes,
    color: ColorValue,
    opacity: f64,
}

impl PreparedAreaLayer {
    /// Appends a normalized paint through fallible reservation, preserving ordering and empty runs.
    ///
    /// # Errors
    /// Returns an allocation diagnostic before adding this paint.
    fn push(
        &mut self,
        shapes: AreaShapes,
        color: &ColorValue,
        opacity: f64,
    ) -> Result<(), RenderError> {
        if !shapes.is_empty() {
            self.paints.try_reserve(1).map_err(|_| {
                area_error("raster.allocation", "prepared area paint allocation failed")
            })?;
            self.paints.push(PreparedAreaPaint {
                shapes,
                color: color.clone(),
                opacity,
            });
        }
        Ok(())
    }

    /// Reports retained vector capacity for geometry diagnostics, excluding allocator metadata.
    pub(super) fn storage_bytes(&self) -> usize {
        self.paints
            .capacity()
            .saturating_mul(std::mem::size_of::<PreparedAreaPaint>())
            .saturating_add(
                self.paints
                    .iter()
                    .map(|paint| {
                        paint
                            .shapes
                            .capacity()
                            .saturating_mul(std::mem::size_of::<AreaShape>())
                            .saturating_add(
                                paint
                                    .shapes
                                    .iter()
                                    .map(|shape| {
                                        shape
                                            .capacity()
                                            .saturating_mul(std::mem::size_of::<AreaContour>())
                                            .saturating_add(
                                                shape
                                                    .iter()
                                                    .map(|contour| {
                                                        contour.capacity().saturating_mul(
                                                            std::mem::size_of::<AreaPoint>(),
                                                        )
                                                    })
                                                    .sum::<usize>(),
                                            )
                                    })
                                    .sum::<usize>(),
                            )
                    })
                    .sum::<usize>(),
            )
    }

    /// Paints one band using global row arithmetic and local pixel storage without renormalizing.
    ///
    /// # Errors
    /// Returns cancellation or geometric integration/allocation errors before exposing a band.
    pub(super) fn paint_band(
        &self,
        pixels: &mut Vec<PremultipliedLinearPixel>,
        width: u32,
        height: u32,
        first_row: u32,
        end_row: u32,
        work: &mut RasterWork<'_>,
    ) -> Result<(), RenderError> {
        for paint in &self.paints {
            work.check()?;
            for_each_area_pixel_in_rows(
                &paint.shapes,
                width,
                height,
                first_row,
                end_row,
                work,
                |index, coverage| pixels.paint_shape(index, coverage, &paint.color, paint.opacity),
            )?;
        }
        Ok(())
    }
}

/// Prepares a visible layer once using the same fill normalization and opaque-run authority.
///
/// The complete target determines circle tessellation and all Boolean adapters. Each layer retains
/// one omission budget across its entire original traversal; bands never participate in preparation.
///
/// # Errors
/// Returns cancellation, shared edge/Boolean/precision limits, or fallible allocation failures.
pub(super) fn prepare_area_layer(
    layer: &RenderLayer,
    width: u32,
    height: u32,
    transform: CanonicalRasterTransform,
    work: &mut RasterWork<'_>,
) -> Result<PreparedAreaLayer, RenderError> {
    let mut prepared = PreparedAreaLayer { paints: Vec::new() };
    if !layer.visible {
        return Ok(prepared);
    }
    let mut pending = Vec::new();
    let mut omissions = AreaOmissionBudget::default();
    for output in &layer.outputs {
        work.check()?;
        if output.primitive_paints().is_none() && layer.color.alpha == 1.0 && layer.opacity == 1.0 {
            pending.try_reserve(1).map_err(|_| {
                area_error("raster.allocation", "opaque output-run allocation failed")
            })?;
            pending.push(output);
            continue;
        }
        let (shapes, _) = prepare_opaque_run(
            &pending,
            width,
            height,
            transform,
            None,
            work,
            &mut omissions,
        )?;
        prepared.push(shapes, &layer.color, 1.0)?;
        pending.clear();
        for_each_area_primitive(
            output.geometry(),
            transform,
            area_visible_bounds(width, height, None),
            work,
            |index, raw, work| {
                let shapes = normalize_and_union(&[raw], None, work, &mut omissions)?;
                prepared.push(
                    shapes,
                    output_primitive_paint(layer, Some(output), index),
                    layer.opacity,
                )
            },
        )?;
    }
    let (shapes, _) = prepare_opaque_run(
        &pending,
        width,
        height,
        transform,
        None,
        work,
        &mut omissions,
    )?;
    prepared.push(shapes, &layer.color, 1.0)?;
    Ok(prepared)
}

/// Rasterizes one layer with floating geometric AA, keeping ineligible paint in canonical order.
///
/// Eligible outputs form maximal contiguous runs; the model's cross-channel composition remains
/// outside this function. Preview clipping is a geometric rectangle before pixel-area integration.
///
/// # Errors
/// Returns target, cancellation, precision, allocation, flattening, or area diagnostics.
pub(super) fn rasterize_area_layer(
    layer: &RenderLayer,
    width: u32,
    height: u32,
    transform: CanonicalRasterTransform,
    clip: Option<[f64; 4]>,
    work: &mut RasterWork<'_>,
) -> Result<Vec<PremultipliedLinearPixel>, RenderError> {
    let mut pixels = allocate_linear_pixels(
        width,
        height,
        background_pixel(RasterBackground::Transparent),
    )?;
    if !layer.visible {
        return Ok(pixels);
    }
    let mut pending: Vec<&RenderOutputLayer> = Vec::new();
    let mut omissions = AreaOmissionBudget::default();
    for output in &layer.outputs {
        let eligible =
            output.primitive_paints().is_none() && layer.color.alpha == 1.0 && layer.opacity == 1.0;
        if eligible {
            pending.try_reserve(1).map_err(|_| {
                area_error("raster.allocation", "opaque output-run allocation failed")
            })?;
            pending.push(output);
            continue;
        }
        paint_opaque_run(
            &mut pixels,
            &pending,
            layer,
            width,
            height,
            transform,
            clip,
            work,
            &mut omissions,
        )?;
        pending.clear();
        for_each_area_primitive(
            output.geometry(),
            transform,
            area_visible_bounds(width, height, clip),
            work,
            |index, primitive, work| {
                let color = output_primitive_paint(layer, Some(output), index);
                paint_ordered_primitive(
                    &mut pixels,
                    primitive,
                    width,
                    height,
                    clip,
                    color,
                    layer.opacity,
                    work,
                    &mut omissions,
                )
            },
        )?;
        // Zero-geometry primitives still count toward the original progress cardinality.
        let count = match output.geometry() {
            GeometryOutput::CircularMarks(marks) => marks.len(),
            GeometryOutput::CanonicalMarks(marks) => marks.len(),
            GeometryOutput::CanonicalStrokes(strokes) => strokes.len(),
            GeometryOutput::CanonicalRegions(regions) => regions.regions().len(),
        };
        for _ in 0..count {
            work.primitive();
        }
    }
    paint_opaque_run(
        &mut pixels,
        &pending,
        layer,
        width,
        height,
        transform,
        clip,
        work,
        &mut omissions,
    )?;
    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds one counter-clockwise analytic rectangular contour.
    fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> AreaContour {
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }

    /// Returns one f64 coverage value at a selected pixel without quantizing the integrator.
    fn pixel_area(shapes: &AreaShapes, x: usize, y: usize) -> f64 {
        let cancelled = || false;
        let mut work = RasterWork::new(
            RasterizationLimits::new(1_000_000).unwrap(),
            RasterAntialiasing::On,
            &cancelled,
        );
        let mut found = 0.0;
        for_each_area_pixel(
            shapes,
            (x + 1) as u32,
            (y + 1) as u32,
            &mut work,
            |index, q| {
                if index == y * (x + 1) + x {
                    found = q;
                }
            },
        )
        .unwrap();
        found
    }

    /// Checks thin horizontal and vertical areas against their exact pixel intersections.
    #[test]
    fn f64_area_retains_phase_and_tiny_positive_slivers() {
        for width in [0.005, 0.001, 1.0e-7] {
            for phase in [0.0, 0.03125, 0.06249, 0.06251, 0.5, 0.93749, 0.93751, 1.0] {
                for origin in [0_usize, 1024] {
                    let o = origin as f64;
                    let low = o + phase - width * 0.5;
                    let high = o + phase + width * 0.5;
                    let expected = (high.min(o + 1.0) - low.max(o)).max(0.0);
                    for contour in [
                        rectangle(o - 1.0, low, o + 2.0, high),
                        rectangle(low, o - 1.0, high, o + 2.0),
                    ] {
                        let area = pixel_area(&vec![vec![contour]], origin, origin);
                        assert!(area > 0.0, "width={width} phase={phase} origin={origin}");
                        assert!(
                            (area - expected).abs() < 2.0e-10,
                            "area={area} expected={expected} width={width} phase={phase} origin={origin}"
                        );
                    }
                }
            }
        }
    }

    /// Checks typed per-primitive union of touching and overlapping opaque shapes.
    #[test]
    fn opaque_union_counts_touching_and_overlap_once() {
        let cancelled = || false;
        let mut work = RasterWork::new(
            RasterizationLimits::new(100).unwrap(),
            RasterAntialiasing::On,
            &cancelled,
        );
        let raw = [
            RawAreaPrimitive {
                shape: vec![rectangle(0.0, 0.0, 0.6, 1.0)],
                fill_rule: FillRule::NonZero,
                omitted_area_bound: 0.0,
            },
            RawAreaPrimitive {
                shape: vec![rectangle(0.3, 0.0, 0.9, 1.0)],
                fill_rule: FillRule::NonZero,
                omitted_area_bound: 0.0,
            },
        ];
        let unioned =
            normalize_and_union(&raw, None, &mut work, &mut AreaOmissionBudget::default()).unwrap();
        assert!((pixel_area(&unioned, 0, 0) - 0.9).abs() < 1.0e-12);
        let touching = [
            RawAreaPrimitive {
                shape: vec![rectangle(0.0, 0.0, 0.5, 1.0)],
                fill_rule: FillRule::NonZero,
                omitted_area_bound: 0.0,
            },
            RawAreaPrimitive {
                shape: vec![rectangle(0.5, 0.0, 1.0, 1.0)],
                fill_rule: FillRule::NonZero,
                omitted_area_bound: 0.0,
            },
        ];
        let unioned = normalize_and_union(
            &touching,
            None,
            &mut work,
            &mut AreaOmissionBudget::default(),
        )
        .unwrap();
        assert!((pixel_area(&unioned, 0, 0) - 1.0).abs() < 1.0e-12);
    }

    /// Accepts exact even-odd and opposite-winding cancellation without inventing lost fill.
    #[test]
    fn intentional_empty_typed_fills_do_not_fail_precision_budget() {
        let cancelled = || false;
        let mut work = RasterWork::new(
            RasterizationLimits::new(100).unwrap(),
            RasterAntialiasing::On,
            &cancelled,
        );
        let square = rectangle(0.0, 0.0, 1.0, 1.0);
        let mut reversed = square.clone();
        reversed.reverse();
        let raw = [
            RawAreaPrimitive {
                shape: vec![square.clone(), square.clone()],
                fill_rule: FillRule::EvenOdd,
                omitted_area_bound: 0.0,
            },
            RawAreaPrimitive {
                shape: vec![square, reversed],
                fill_rule: FillRule::NonZero,
                omitted_area_bound: 0.0,
            },
        ];
        let mut omissions = AreaOmissionBudget::default();
        let normalized = normalize_and_union(&raw, None, &mut work, &mut omissions).unwrap();
        assert!(normalized.is_empty());
        assert_eq!(omissions.0.value(), 0.0);
    }

    /// Checks small-circle relative area, anisotropic ellipse mapping, and a fractional geometric clip.
    #[test]
    fn circles_and_fractional_clip_keep_output_pixel_area() {
        let cancelled = || false;
        let mut work = RasterWork::new(
            RasterizationLimits::new(100_000).unwrap(),
            RasterAntialiasing::On,
            &cancelled,
        );
        for radius in [0.25, 0.05, 0.005, 0.0001] {
            let (shape, bound) = area_circle(
                Point2::new(0.5, 0.5),
                radius,
                CanonicalRasterTransform::native(),
                &mut work,
            )
            .unwrap();
            let raw = [RawAreaPrimitive {
                shape: shape.unwrap(),
                fill_rule: FillRule::NonZero,
                omitted_area_bound: bound,
            }];
            let normalized =
                normalize_and_union(&raw, None, &mut work, &mut AreaOmissionBudget::default())
                    .unwrap();
            let expected = std::f64::consts::PI * radius * radius;
            assert!((pixel_area(&normalized, 0, 0) - expected).abs() / expected < 0.007);
        }
        let (shape, bound) = area_circle(
            Point2::new(0.25, 1.0 / 3.0),
            0.1,
            CanonicalRasterTransform {
                scale_x: 2.0,
                scale_y: 1.5,
                offset_x: 0.0,
                offset_y: 0.0,
            },
            &mut work,
        )
        .unwrap();
        let raw = [RawAreaPrimitive {
            shape: shape.unwrap(),
            fill_rule: FillRule::NonZero,
            omitted_area_bound: bound,
        }];
        let ellipse =
            normalize_and_union(&raw, None, &mut work, &mut AreaOmissionBudget::default()).unwrap();
        let expected = std::f64::consts::PI * 0.2 * 0.15;
        assert!((pixel_area(&ellipse, 0, 0) - expected).abs() / expected < 0.007);
        let clipped = [RawAreaPrimitive {
            shape: vec![rectangle(-1.0, -1.0, 2.0, 2.0)],
            fill_rule: FillRule::NonZero,
            omitted_area_bound: 0.0,
        }];
        let clipped = normalize_and_union(
            &clipped,
            Some([0.25, 0.25, 0.75, 0.75]),
            &mut work,
            &mut AreaOmissionBudget::default(),
        )
        .unwrap();
        assert!((pixel_area(&clipped, 0, 0) - 0.25).abs() < 1.0e-12);
    }

    /// Checks that sampled or translucent primitives retain separate ordered source-over terms.
    #[test]
    fn ordered_translucent_paint_keeps_repeated_faint_contributions() {
        let cancelled = || false;
        let mut work = RasterWork::new(
            RasterizationLimits::new(100).unwrap(),
            RasterAntialiasing::On,
            &cancelled,
        );
        let color = ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.5,
        };
        let mut pixels =
            allocate_linear_pixels(1, 1, background_pixel(RasterBackground::Transparent)).unwrap();
        let mut omissions = AreaOmissionBudget::default();
        for _ in 0..3 {
            let raw = RawAreaPrimitive {
                shape: vec![rectangle(0.0, 0.0, 0.5, 1.0)],
                fill_rule: FillRule::NonZero,
                omitted_area_bound: 0.0,
            };
            paint_ordered_primitive(
                &mut pixels,
                raw,
                1,
                1,
                None,
                &color,
                1.0,
                &mut work,
                &mut omissions,
            )
            .unwrap();
        }
        assert!((pixels[0].alpha - (1.0 - 0.75_f64.powi(3))).abs() < 1.0e-12);
    }

    /// Measures a uniform isolated-dot midtone before and after final byte quantization.
    #[test]
    fn uniform_midtone_dots_have_bounded_polygon_area_loss() {
        let cancelled = || false;
        let mut work = RasterWork::new(
            RasterizationLimits::new(20_000).unwrap(),
            RasterAntialiasing::On,
            &cancelled,
        );
        let radius = (0.5 / std::f64::consts::PI).sqrt();
        let mut raw = Vec::new();
        for y in 0..16 {
            for x in 0..16 {
                let (shape, bound) = area_circle(
                    Point2::new(x as f64 + 0.5, y as f64 + 0.5),
                    radius,
                    CanonicalRasterTransform::native(),
                    &mut work,
                )
                .unwrap();
                raw.push(RawAreaPrimitive {
                    shape: shape.unwrap(),
                    fill_rule: FillRule::NonZero,
                    omitted_area_bound: bound,
                });
            }
        }
        let normalized =
            normalize_and_union(&raw, None, &mut work, &mut AreaOmissionBudget::default()).unwrap();
        let mut values = vec![0.0; 256];
        for_each_area_pixel(&normalized, 16, 16, &mut work, |index, q| values[index] = q).unwrap();
        let mean = values.iter().sum::<f64>() / 256.0;
        let expected = 0.5
            * (MIN_CIRCLE_SIDES as f64 * (std::f64::consts::TAU / MIN_CIRCLE_SIDES as f64).sin()
                / std::f64::consts::TAU);
        assert!(
            (mean - expected).abs() < 2.0e-10,
            "mean={mean} expected={expected}"
        );
        assert!(0.5 - mean < 0.0025, "midpoint area loss={}", 0.5 - mean);
        let encoded_mean = values
            .iter()
            .map(|q| (linear_to_srgb(*q) * 255.0).round())
            .sum::<f64>()
            / 256.0;
        println!(
            "UNIFORM_MIDTONE sides={MIN_CIRCLE_SIDES} analytic_q=0.5 measured_q={mean:.12} encoded_srgb_mean={encoded_mean:.3} exact_encoded_srgb={:.3}",
            (linear_to_srgb(0.5) * 255.0).round()
        );
    }

    /// Preflights circle allocation and charges only bounded unrepresentable positive area.
    #[test]
    fn circle_budget_and_collapsed_area_are_explicit() {
        let cancelled = || false;
        let mut narrow = RasterWork::new(
            RasterizationLimits::new(39).unwrap(),
            RasterAntialiasing::On,
            &cancelled,
        );
        let error = area_circle(
            Point2::new(0.5, 0.5),
            0.1,
            CanonicalRasterTransform::native(),
            &mut narrow,
        )
        .unwrap_err();
        assert_eq!(error.path(), "raster.limits.flattened_edges");
        let mut work = RasterWork::new(
            RasterizationLimits::new(10_000).unwrap(),
            RasterAntialiasing::On,
            &cancelled,
        );
        let mut omissions = AreaOmissionBudget::default();
        let (shape, bound) = area_circle(
            Point2::new(1.0e20, 1.0e20),
            1.0e-7,
            CanonicalRasterTransform::native(),
            &mut work,
        )
        .unwrap();
        assert!(shape.is_none());
        let raw = [RawAreaPrimitive {
            shape: Vec::new(),
            fill_rule: FillRule::NonZero,
            omitted_area_bound: bound,
        }];
        assert!(
            normalize_and_union(&raw, None, &mut work, &mut omissions)
                .unwrap()
                .is_empty()
        );
        let (shape, bound) = area_circle(
            Point2::new(1.0e20, 1.0e20),
            1.0,
            CanonicalRasterTransform::native(),
            &mut work,
        )
        .unwrap();
        assert!(shape.is_none());
        let raw = [RawAreaPrimitive {
            shape: Vec::new(),
            fill_rule: FillRule::NonZero,
            omitted_area_bound: bound,
        }];
        assert_eq!(
            normalize_and_union(&raw, None, &mut work, &mut omissions)
                .unwrap_err()
                .path(),
            "raster.area.precision"
        );
    }

    /// Charges a grid-collapsed component or hole even when the same primitive retains paint.
    ///
    /// The original typed fill remains authoritative; the ledger bounds absolute fill error,
    /// including extra paint caused by losing a negative hole.
    ///
    /// # Panics
    /// Panics if a finite fixture fails normalization or its collapsed contour escapes the ledger.
    #[test]
    fn partial_contour_collapse_charges_absolute_fill_error() {
        let cancelled = || false;
        let h = 1.0e-13;
        for rule in [FillRule::NonZero, FillRule::EvenOdd] {
            for hole in [false, true] {
                let mut tiny = rectangle(-h, -h, h, h);
                let mut shape = if hole {
                    tiny.reverse();
                    vec![rectangle(-2.0, -1.0, 2.0, 1.0)]
                } else {
                    vec![
                        rectangle(-2.0, -1.0, -1.0, 1.0),
                        rectangle(1.0, -1.0, 2.0, 1.0),
                    ]
                };
                shape.push(tiny);
                let mut work = RasterWork::new(
                    RasterizationLimits::new(100).unwrap(),
                    RasterAntialiasing::On,
                    &cancelled,
                );
                let mut omissions = AreaOmissionBudget::default();
                let normalized = normalize_and_union(
                    &[RawAreaPrimitive {
                        shape,
                        fill_rule: rule,
                        omitted_area_bound: 0.0,
                    }],
                    None,
                    &mut work,
                    &mut omissions,
                )
                .unwrap();
                assert!(!normalized.is_empty());
                assert!(omissions.0.value() > 0.0, "rule={rule:?} hole={hole}");
            }
        }
    }

    /// Enforces one cumulative layer budget without charging ordinary retained contour perimeters.
    ///
    /// # Panics
    /// Panics if healthy paint is charged, successive collapsed strips escape the shared budget,
    /// or an entirely empty primitive charges its same collapsed contour twice.
    #[test]
    fn collapsed_contours_share_budget_without_global_perimeter_charge() {
        let cancelled = || false;
        let mut work = RasterWork::new(
            RasterizationLimits::new(10_000).unwrap(),
            RasterAntialiasing::On,
            &cancelled,
        );
        let clip = Some([-10_000.0, -2.0, 10_000.0, 2.0]);
        let healthy = rectangle(-10_000.0, -1.0, 10_000.0, 1.0);
        let mut omissions = AreaOmissionBudget::default();
        let raw = [RawAreaPrimitive {
            shape: vec![healthy.clone()],
            fill_rule: FillRule::NonZero,
            omitted_area_bound: 0.0,
        }];
        normalize_and_union(&raw, clip, &mut work, &mut omissions).unwrap();
        assert_eq!(omissions.0.value(), 0.0);

        let strip = rectangle(-10_000.0, -1.0e-13, 10_000.0, 1.0e-13);
        let bound = area_perturbation_bound(std::slice::from_ref(&strip), &work).unwrap();
        assert!((bound - 1.6e-7).abs() < 1.0e-18);
        let raw = [RawAreaPrimitive {
            shape: vec![healthy, strip.clone()],
            fill_rule: FillRule::NonZero,
            omitted_area_bound: 0.0,
        }];
        omissions
            .record(MAX_OMITTED_PIXEL_AREA - 1.5 * bound)
            .unwrap();
        normalize_and_union(&raw, clip, &mut work, &mut omissions).unwrap();
        assert_eq!(
            normalize_and_union(&raw, clip, &mut work, &mut omissions)
                .unwrap_err()
                .path(),
            "raster.area.precision"
        );

        let mut omissions = AreaOmissionBudget::default();
        omissions
            .record(MAX_OMITTED_PIXEL_AREA - 1.5 * bound)
            .unwrap();
        let raw = [RawAreaPrimitive {
            shape: vec![strip],
            fill_rule: FillRule::NonZero,
            omitted_area_bound: bound * 0.5,
        }];
        assert!(
            normalize_and_union(&raw, clip, &mut work, &mut omissions)
                .unwrap()
                .is_empty()
        );
        assert!((omissions.0.value() - (MAX_OMITTED_PIXEL_AREA - 0.5 * bound)).abs() < 1.0e-20);
    }

    /// Exempts original exact typed cancellation beside retained paint but never snapped equality.
    ///
    /// # Panics
    /// Panics if cancellation changes fill authority, an unmatched duplicate is exempted,
    /// or differently authored rings gain a snapped-coordinate exemption.
    #[test]
    fn collapsed_contour_cancellation_uses_original_typed_fill() {
        let cancelled = || false;
        let tiny = rectangle(-1.0e-13, -1.0e-13, 1.0e-13, 1.0e-13);
        let mut opposite = tiny.clone();
        opposite.reverse();
        opposite.rotate_left(1);
        let mut shifted = tiny.clone();
        shifted.rotate_left(2);
        let different = rectangle(-2.0e-13, -2.0e-13, 2.0e-13, 2.0e-13);
        for (rule, contours, expected_charge) in [
            (
                FillRule::NonZero,
                vec![tiny.clone(), opposite.clone()],
                false,
            ),
            (
                FillRule::EvenOdd,
                vec![tiny.clone(), opposite.clone()],
                false,
            ),
            (
                FillRule::EvenOdd,
                vec![tiny.clone(), shifted.clone()],
                false,
            ),
            (FillRule::NonZero, vec![tiny.clone(), shifted], true),
            (
                FillRule::EvenOdd,
                vec![tiny.clone(), tiny.clone(), opposite],
                true,
            ),
            (FillRule::EvenOdd, vec![tiny, different], true),
        ] {
            let mut work = RasterWork::new(
                RasterizationLimits::new(1_000).unwrap(),
                RasterAntialiasing::On,
                &cancelled,
            );
            let mut shape = vec![rectangle(-2.0, -1.0, 2.0, 1.0)];
            shape.extend(contours);
            let mut omissions = AreaOmissionBudget::default();
            normalize_and_union(
                &[RawAreaPrimitive {
                    shape,
                    fill_rule: rule,
                    omitted_area_bound: 0.0,
                }],
                None,
                &mut work,
                &mut omissions,
            )
            .unwrap();
            assert_eq!(omissions.0.value() > 0.0, expected_charge, "rule={rule:?}");
        }
    }

    /// Keeps exact-cancellation search under request-wide work and cancellation authority.
    ///
    /// # Panics
    /// Panics if a bounded exact-pair search ignores work exhaustion or cancellation.
    #[test]
    fn collapsed_contour_pair_search_obeys_work_and_cancellation() {
        let tiny = rectangle(-1.0e-13, -1.0e-13, 1.0e-13, 1.0e-13);
        let mut opposite = tiny.clone();
        opposite.reverse();
        let cancelled = || false;
        let mut work = RasterWork::new(
            RasterizationLimits::new(1).unwrap(),
            RasterAntialiasing::On,
            &cancelled,
        );
        assert_eq!(
            exact_contours_cancel(&tiny, &opposite, FillRule::NonZero, &mut work)
                .unwrap_err()
                .path(),
            "raster.limits.flattened_edges"
        );
        let cancelled = || true;
        let mut work = RasterWork::new(
            RasterizationLimits::new(100).unwrap(),
            RasterAntialiasing::On,
            &cancelled,
        );
        assert_eq!(
            exact_contours_cancel(&tiny, &opposite, FillRule::NonZero, &mut work)
                .unwrap_err()
                .path(),
            "evaluation.cancelled"
        );
    }
    /// Proves split-mutation certificates share the layer budget before retained paint can change pixels.
    ///
    /// # Panics
    /// Panics if nondegenerate rounded intersections are uncharged, cumulative error is ignored,
    /// or precision failure paints any local surface.
    #[test]
    fn checked_split_certificate_budget_precedes_retained_paint() {
        let cancelled = || false;
        let limits = RasterizationLimits::new(100_000).unwrap();
        let raw_shape = vec![
            rectangle(0.0, 0.0, 0.9, 0.9),
            vec![[0.05, 0.1], [0.95, 0.3], [0.2, 0.97]],
        ];
        let raw = || RawAreaPrimitive {
            shape: raw_shape.clone(),
            fill_rule: FillRule::EvenOdd,
            omitted_area_bound: 0.0,
        };
        let mut work = RasterWork::new(limits, RasterAntialiasing::On, &cancelled);
        let mut observed = AreaOmissionBudget::default();
        let shapes = normalize_and_union(&[raw()], None, &mut work, &mut observed).unwrap();
        assert!(!shapes.is_empty());
        let bound = observed.0.value();
        assert!(bound > 0.0 && bound < MAX_OMITTED_PIXEL_AREA);
        let mut work = RasterWork::new(limits, RasterAntialiasing::On, &cancelled);
        let mut omissions = AreaOmissionBudget::default();
        omissions
            .record(MAX_OMITTED_PIXEL_AREA - bound * 0.5)
            .unwrap();
        let mut pixels =
            allocate_linear_pixels(1, 1, background_pixel(RasterBackground::Transparent)).unwrap();
        let color = ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        };
        let error = paint_ordered_primitive(
            &mut pixels,
            raw(),
            1,
            1,
            None,
            &color,
            1.0,
            &mut work,
            &mut omissions,
        )
        .unwrap_err();
        assert_eq!(error.path, "raster.area.precision");
        assert_eq!(pixels[0].alpha, 0.0);
        assert!(work.remaining_boolean_operations < limits.max_boolean_operations());
    }

    /// Proves the separate zero Boolean allowance rejects a healthy fill without consuming edge admission.
    ///
    /// # Panics
    /// Panics if Boolean exhaustion is mislabeled or shares the flattened-edge admission counter.
    #[test]
    fn boolean_work_limit_is_separate_from_flattened_edges() {
        let cancelled = || false;
        let limits = RasterizationLimits::new(100)
            .unwrap()
            .with_max_boolean_operations(0);
        let mut work = RasterWork::new(limits, RasterAntialiasing::On, &cancelled);
        let raw = RawAreaPrimitive {
            shape: vec![rectangle(0.0, 0.0, 1.0, 1.0)],
            fill_rule: FillRule::NonZero,
            omitted_area_bound: 0.0,
        };
        let error =
            normalize_and_union(&[raw], None, &mut work, &mut AreaOmissionBudget::default())
                .unwrap_err();
        assert_eq!(error.path, "raster.limits.boolean_operations");
        assert!(work.remaining_edges > 0);
        assert_eq!(work.remaining_boolean_operations, 0);
        assert_eq!(
            RasterizationLimits::new(usize::MAX)
                .unwrap()
                .max_boolean_operations(),
            usize::MAX
        );
    }
}
