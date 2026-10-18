//! Final-consumer SVG compound paths for opaque channel runs.
//!
//! The immutable scene and every raster/animation consumer remain upstream of
//! this module. Each member is positive filled geometry before nonzero winding
//! combines members, so touching and overlapping members paint once.

use super::{GeometryOutput, RenderError, color_hex};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::scale::FixedScaleFloatOverlay,
};
use kurbo::{CubicBez, ParamCurve as _, ParamCurveArea as _, Point, QuadBez};
use std::fmt::Write as _;
use toniator_domain::{CanvasSpec, ColorValue};
use toniator_geometry::{CanonicalFillRule, CanonicalMark, CurveSegment, Point2};

const BOOLEAN_SCALE: f64 = 1.0e12;
const FLATTEN_TOLERANCE: f64 = 1.0e-8;

type Contour = Vec<[f64; 2]>;

/// Request-local work and cancellation authority for final SVG path emission.
pub(super) struct SvgJoin<'a> {
    axis: f64,
    width: f64,
    height: f64,
    remaining_edges: usize,
    cancelled: &'a (dyn Fn() -> bool + Sync),
}

impl<'a> SvgJoin<'a> {
    /// Establishes a finite canvas-relative Boolean coordinate system.
    ///
    /// # Errors
    /// Rejects invalid canvas dimensions, cancellation, or a disabled edge budget.
    pub(super) fn new(
        canvas: &CanvasSpec,
        max_edges: usize,
        cancelled: &'a (dyn Fn() -> bool + Sync),
    ) -> Result<Self, RenderError> {
        super::validate_canvas(canvas)?;
        if max_edges == 0 {
            return Err(err("svg.limits.edges", "SVG edge limit must be positive"));
        }
        let join = Self {
            axis: canvas.width.max(canvas.height),
            width: canvas.width,
            height: canvas.height,
            remaining_edges: max_edges,
            cancelled,
        };
        join.check()?;
        Ok(join)
    }

    /// Polls cancellation at each output, primitive, and Boolean call boundary.
    ///
    /// # Errors
    /// Returns a cancellation diagnostic without publishing partial SVG bytes.
    pub(super) fn check(&self) -> Result<(), RenderError> {
        if (self.cancelled)() {
            Err(err("svg.cancelled", "SVG serialization was cancelled"))
        } else {
            Ok(())
        }
    }

    /// Charges a checked request-wide count for canonical or flattened edges.
    ///
    /// # Errors
    /// Returns a bound diagnostic before an oversized path is serialized.
    pub(super) fn edges(&mut self, count: usize) -> Result<(), RenderError> {
        self.check()?;
        self.remaining_edges = self
            .remaining_edges
            .checked_sub(count)
            .ok_or(err("svg.limits.edges", "SVG edge limit exceeded"))?;
        Ok(())
    }

    /// Conservatively retains circles whose closed bounds touch the final canvas.
    pub(super) fn circle_intersects_canvas(&self, center: Point2, radius: f64) -> bool {
        center.x + radius >= 0.0
            && center.y + radius >= 0.0
            && center.x - radius <= self.width
            && center.y - radius <= self.height
    }

    /// Rejects only contours whose entire control hull lies strictly outside one canvas edge.
    pub(super) fn contour_intersects_canvas(&self, segments: &[CurveSegment]) -> bool {
        let mut bounds = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for segment in segments {
            let controls = match segment {
                CurveSegment::Line(line) => [line.start(), line.start(), line.end(), line.end()],
                CurveSegment::CubicBezier(cubic) => [
                    cubic.start(),
                    cubic.control_1(),
                    cubic.control_2(),
                    cubic.end(),
                ],
            };
            for point in controls {
                bounds[0] = bounds[0].min(point.x);
                bounds[1] = bounds[1].min(point.y);
                bounds[2] = bounds[2].max(point.x);
                bounds[3] = bounds[3].max(point.y);
            }
        }
        bounds[2] >= 0.0 && bounds[3] >= 0.0 && bounds[0] <= self.width && bounds[1] <= self.height
    }

    /// Emits one closed contour's intersection with the final canvas, preserving direction.
    ///
    /// Fully contained control hulls use the existing direct serializer unchanged.
    /// # Errors
    /// Returns cancellation, limits, malformed closure, allocation, or clipping precision errors.
    pub(super) fn curve_data(
        &mut self,
        data: &mut String,
        segments: &[CurveSegment],
        reverse: bool,
    ) -> Result<(), RenderError> {
        validate_contour(segments)?;
        if !self.contour_intersects_canvas(segments) {
            return Ok(());
        }
        if segments.iter().all(|segment| {
            let controls = match segment {
                CurveSegment::Line(l) => [l.start(), l.start(), l.end(), l.end()],
                CurveSegment::CubicBezier(c) => [c.start(), c.control_1(), c.control_2(), c.end()],
            };
            controls
                .iter()
                .all(|p| p.x >= 0.0 && p.y >= 0.0 && p.x <= self.width && p.y <= self.height)
        }) {
            self.edges(segments.len())?;
            return append_curve(data, segments, reverse, self.cancelled);
        }
        super::svg_clip::curve(
            data,
            segments,
            reverse,
            [0.0, 0.0, self.width, self.height],
            self,
        )
    }

    /// Reports whether a positive circle needs a real boundary intersection.
    pub(super) fn circle_needs_clipping(&self, center: Point2, radius: f64) -> bool {
        center.x < radius
            || center.y < radius
            || self.width - center.x < radius
            || self.height - center.y < radius
    }

    /// Emits an intersected circle as radius-preserving arcs for any ordered paint consumer.
    /// # Errors
    /// Returns cancellation, work-limit, allocation, or clipping precision diagnostics.
    pub(super) fn clipped_circle_data(
        &mut self,
        data: &mut String,
        center: Point2,
        radius: f64,
    ) -> Result<(), RenderError> {
        super::svg_clip::circle(
            data,
            center,
            radius,
            [0.0, 0.0, self.width, self.height],
            self,
        )
    }

    /// Emits one NonZero path for a maximal eligible opaque output run.
    ///
    /// The path retains exact circles and canonical certified positive regions.
    /// Unsafe fill topology is flattened and normalized per primitive on a
    /// checked 1e12 canvas-relative grid before entering the compound path.
    ///
    /// # Errors
    /// Returns cancellation, budget, precision, normalization, or allocation errors.
    pub(super) fn write_run(
        &mut self,
        document: &mut String,
        channel_id: u64,
        run_ordinal: usize,
        color: &ColorValue,
        outputs: &[&GeometryOutput],
    ) -> Result<(), RenderError> {
        let mut data = String::new();
        for geometry in outputs {
            self.check()?;
            match geometry {
                GeometryOutput::CircularMarks(marks) => {
                    for mark in marks {
                        self.circle(&mut data, mark.center, mark.radius)?;
                    }
                }
                GeometryOutput::CanonicalMarks(marks) => {
                    for mark in marks {
                        self.check()?;
                        match mark {
                            CanonicalMark::Circle { center, radius, .. } => {
                                self.circle(&mut data, *center, *radius)?
                            }
                            CanonicalMark::ClosedPath(mark) => {
                                self.path(&mut data, mark.path.segments(), mark.fill_rule)?;
                            }
                        }
                    }
                }
                GeometryOutput::CanonicalStrokes(strokes) => {
                    for stroke in strokes {
                        self.check()?;
                        if let [contour] = stroke.outline.contours.as_slice() {
                            self.path(&mut data, &contour.segments, stroke.outline.fill_rule)?;
                            continue;
                        }
                        let contours: Vec<_> = stroke
                            .outline
                            .contours
                            .iter()
                            .map(|contour| contour.segments.as_slice())
                            .collect();
                        self.contours(&mut data, &contours, stroke.outline.fill_rule)?;
                    }
                }
                GeometryOutput::CanonicalRegions(regions) => {
                    for region in regions.regions() {
                        self.check()?;
                        if !self.contour_intersects_canvas(region.ring.segments()) {
                            continue;
                        }
                        // CanonicalRegion validates counter-clockwise, simple,
                        // positive, hole-free geometry at construction.
                        self.curve_data(&mut data, region.ring.segments(), false)?;
                    }
                }
            }
        }
        if data.is_empty() {
            return Ok(());
        }
        self.check()?;
        document
            .try_reserve(data.len() + 160)
            .map_err(|_| err("svg.allocation", "SVG document allocation failed"))?;
        writeln!(document, "<path id=\"channel-{channel_id}-opaque-join-{run_ordinal}\" d=\"{data}\" fill=\"{}\" fill-rule=\"nonzero\"/>", color_hex(color))
            .map_err(|_| err("svg.format", "SVG path formatting failed"))?;
        Ok(())
    }

    /// Appends an exact positive circle through four relative quarter arcs.
    ///
    /// The center and radius use shortest-roundtrip text. The relative move
    /// avoids addition of center and radius, which can erase very small marks.
    ///
    /// # Errors
    /// Returns cancellation, bounds, or allocation diagnostics.
    fn circle(
        &mut self,
        data: &mut String,
        center: Point2,
        radius: f64,
    ) -> Result<(), RenderError> {
        self.check()?;
        if radius == 0.0 || !self.circle_intersects_canvas(center, radius) {
            return Ok(());
        }
        if self.circle_needs_clipping(center, radius) {
            return self.clipped_circle_data(data, center, radius);
        }
        self.edges(4)?;
        let x = center.x.to_string();
        let y = center.y.to_string();
        let r = radius.to_string();
        let neg = (-radius).to_string();
        data.try_reserve(x.len() + y.len() + r.len() * 10 + 100)
            .map_err(|_| err("svg.allocation", "SVG circle allocation failed"))?;
        write!(data, "M{x} {y}m{r} 0a{r} {r} 0 0 1 {neg} {r}a{r} {r} 0 0 1 {neg} {neg}a{r} {r} 0 0 1 {r} {neg}a{r} {r} 0 0 1 {r} {r}Z")
            .map_err(|_| err("svg.format", "SVG circle formatting failed"))?;
        Ok(())
    }

    /// Preserves a certified simple line/cubic contour's exact geometry and winding;
    /// delegates other closed paths to primitive-local fill normalization.
    ///
    /// # Errors
    /// Returns cancellation, budget, precision, or normalization diagnostics.
    fn path(
        &mut self,
        data: &mut String,
        segments: &[CurveSegment],
        rule: CanonicalFillRule,
    ) -> Result<(), RenderError> {
        self.check()?;
        validate_contour(segments)?;
        if !self.contour_intersects_canvas(segments) {
            return Ok(());
        }
        if let Some(reverse) = certified_simple_winding(segments, self.cancelled)? {
            return self.curve_data(data, segments, reverse);
        }
        self.contours(data, &[segments], rule)
    }

    /// Applies one primitive's fill rule before its polygons enter the run.
    ///
    /// Exactly point-collapsed contours contribute no fill under either rule;
    /// all other contours retain the existing certification and precision gates.
    /// Omitted canonical edges still count toward request work limits.
    ///
    /// # Errors
    /// Returns cancellation, edge-limit, precision, or normalization diagnostics.
    fn contours(
        &mut self,
        data: &mut String,
        contours: &[&[CurveSegment]],
        rule: CanonicalFillRule,
    ) -> Result<(), RenderError> {
        if contours.is_empty() {
            return Ok(());
        }
        let mut filled_contours = Vec::new();
        filled_contours.try_reserve(contours.len()).map_err(|_| {
            err(
                "svg.allocation",
                "SVG contour classification allocation failed",
            )
        })?;
        let mut omitted_point = false;
        for segments in contours {
            self.check()?;
            validate_contour(segments)?;
            if exact_point_contour(segments, self.cancelled)? {
                self.edges(segments.len())?;
                omitted_point = true;
            } else {
                filled_contours.push(*segments);
            }
        }
        let contours = filled_contours.as_slice();
        if contours.is_empty() {
            return Ok(());
        }
        if !contours
            .iter()
            .any(|segments| self.contour_intersects_canvas(segments))
        {
            return Ok(());
        }
        if (omitted_point && contours.len() == 1)
            || (rule == CanonicalFillRule::NonZero && contours.len() > 1)
        {
            let mut winding = None;
            let mut certified = true;
            for segments in contours {
                self.check()?;
                validate_contour(segments)?;
                let Some(reverse) = certified_simple_winding(segments, self.cancelled)? else {
                    certified = false;
                    break;
                };
                if winding.is_some_and(|previous| previous != reverse) {
                    certified = false;
                    break;
                }
                winding = Some(reverse);
            }
            if certified {
                // Same-oriented simple contours contribute winding of only
                // one sign, even where their interiors overlap. Reorienting
                // them together normalizes this primitive without flattening.
                for segments in contours {
                    self.curve_data(data, segments, winding.unwrap_or(false))?;
                }
                return Ok(());
            }
        }
        let mut shape = Vec::new();
        for segments in contours {
            self.check()?;
            validate_contour(segments)?;
            let contour =
                flatten_svg_contour(segments, self.axis, self.remaining_edges, self.cancelled)
                    .map_err(|error| match error.path() {
                        "evaluation.cancelled" => {
                            err("svg.cancelled", "SVG serialization was cancelled")
                        }
                        "vector.limits.flattened_edges" => {
                            err("svg.limits.edges", "SVG edge limit exceeded")
                        }
                        _ => error,
                    })?;
            self.edges(contour.len())?;
            for point in &contour {
                if point.iter().any(|value| {
                    !value.is_finite() || value.abs() > (i64::MAX as f64 / 16.0) / BOOLEAN_SCALE
                }) {
                    return Err(err(
                        "svg.precision.coordinate",
                        "SVG contour exceeds Boolean coordinate range",
                    ));
                }
            }
            ensure_grid_resolvable_contour(&contour, self.cancelled)?;
            shape.push(contour);
        }
        let fill = match rule {
            CanonicalFillRule::EvenOdd => FillRule::EvenOdd,
            CanonicalFillRule::NonZero => FillRule::NonZero,
        };
        self.check()?;
        let empty: Contour = Vec::new();
        let normalized = shape
            .overlay_with_fixed_scale_as::<i64>(&empty, OverlayRule::Subject, fill, BOOLEAN_SCALE)
            .map_err(|_| {
                err(
                    "svg.overlay.primitive",
                    "SVG primitive fill normalization failed",
                )
            })?;
        self.check()?;
        if normalized.is_empty() {
            if exact_empty_pair(contours, fill) {
                return Ok(());
            }
            return Err(err(
                "svg.precision.omitted",
                "SVG primitive may contain positive geometry below Boolean grid",
            ));
        }
        for filled in normalized {
            for (index, ring) in filled.iter().enumerate() {
                self.check()?;
                let hole = index != 0;
                let reverse = (signed_area(ring) < 0.0) != hole;
                if ring.iter().all(|p| {
                    p[0] * self.axis >= 0.0
                        && p[1] * self.axis >= 0.0
                        && p[0] * self.axis <= self.width
                        && p[1] * self.axis <= self.height
                }) {
                    self.edges(ring.len())?;
                    append_polygon(data, ring, self.axis, hole, self.cancelled)?;
                } else {
                    super::svg_clip::polygon(
                        data,
                        ring,
                        self.axis,
                        reverse,
                        [0.0, 0.0, self.width, self.height],
                        self,
                    )?;
                }
            }
        }
        Ok(())
    }
}

/// Appends one normalized SVG polygon vertex after exact work, cancellation and storage checks.
/// # Errors
/// Returns cancellation, finite-coordinate, edge-limit or reported allocation failure atomically.
fn push_svg_vertex(
    points: &mut Contour,
    point: Point,
    budget: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), RenderError> {
    if cancelled() {
        return Err(err("svg.cancelled", "SVG serialization was cancelled"));
    }
    if !point.is_finite() {
        return Err(err("vector.contour", "flattened SVG contour is not finite"));
    }
    if points.len() >= budget {
        return Err(err("svg.limits.edges", "SVG edge limit exceeded"));
    }
    points
        .try_reserve(1)
        .map_err(|_| err("svg.allocation", "SVG flattening allocation failed"))?;
    points.push([point.x, point.y]);
    Ok(())
}

/// Flattens one canonical contour using curvature accuracy rather than chord-length subdivision.
/// The unchanged 1e-8 canvas-relative tolerance is split between cubic-to-quadratic (10%)
/// and quadratic-to-polyline (90%) error. Lazy cubic intervals avoid a retained quadratic vector;
/// fallible storage and cancellation remain request-local. Other vector consumers stay unchanged.
/// # Errors
/// Returns cancellation, checked finite/work/storage diagnostics without publishing a partial contour.
fn flatten_svg_contour(
    segments: &[CurveSegment],
    axis: f64,
    budget: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Contour, RenderError> {
    let first = segments
        .first()
        .ok_or(err("vector.contour", "filled contour has no segments"))?;
    if segments.len() > budget {
        return Err(err("svg.limits.edges", "SVG edge limit exceeded"));
    }
    let mut points = Vec::new();
    push_svg_vertex(
        &mut points,
        Point::new(first.start().x / axis, first.start().y / axis),
        budget,
        cancelled,
    )?;
    let mut work = 0usize;
    for segment in segments {
        if cancelled() {
            return Err(err("svg.cancelled", "SVG serialization was cancelled"));
        }
        match segment {
            CurveSegment::Line(line) => push_svg_vertex(
                &mut points,
                Point::new(line.end().x / axis, line.end().y / axis),
                budget,
                cancelled,
            )?,
            CurveSegment::CubicBezier(c) => {
                let cubic = CubicBez::new(
                    (c.start().x / axis, c.start().y / axis),
                    (c.control_1().x / axis, c.control_1().y / axis),
                    (c.control_2().x / axis, c.control_2().y / axis),
                    (c.end().x / axis, c.end().y / axis),
                );
                if ![cubic.p0, cubic.p1, cubic.p2, cubic.p3]
                    .into_iter()
                    .all(|p| p.is_finite())
                {
                    return Err(err("vector.contour", "filled cubic is not finite"));
                }
                flatten_svg_cubic(cubic, &mut points, &mut work, budget, cancelled)?;
            }
        }
    }
    if cancelled() {
        return Err(err("svg.cancelled", "SVG serialization was cancelled"));
    }
    if points.len() >= 2 && points.first() == points.last() {
        points.pop();
    }
    if points.len() < 3 {
        return Err(err(
            "vector.contour",
            "flattened filled contour is not finite and closed",
        ));
    }
    Ok(points)
}

/// Emits an exact monotone chord or lazy accuracy-bounded quadratic intervals for one cubic.
/// Endpoint-collapsed controls and axis-aligned controls between endpoints certify exact chord geometry.
/// Kurbo's derived arithmetic/count is validated before its cast/iterator can cause unbounded work.
/// # Errors
/// Returns finite/count/addressability, configured work, cancellation or allocation failure.
fn flatten_svg_cubic(
    cubic: CubicBez,
    points: &mut Contour,
    work: &mut usize,
    budget: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), RenderError> {
    if cancelled() {
        return Err(err("svg.cancelled", "SVG serialization was cancelled"));
    }
    let between = |value: f64, a: f64, b: f64| value >= a.min(b) && value <= a.max(b);
    if (cubic.p1 == cubic.p0 && cubic.p2 == cubic.p3)
        || (cubic.p0.x == cubic.p1.x
            && cubic.p1.x == cubic.p2.x
            && cubic.p2.x == cubic.p3.x
            && between(cubic.p1.y, cubic.p0.y, cubic.p3.y)
            && between(cubic.p2.y, cubic.p0.y, cubic.p3.y))
        || (cubic.p0.y == cubic.p1.y
            && cubic.p1.y == cubic.p2.y
            && cubic.p2.y == cubic.p3.y
            && between(cubic.p1.x, cubic.p0.x, cubic.p3.x)
            && between(cubic.p2.x, cubic.p0.x, cubic.p3.x))
    {
        return push_svg_vertex(points, cubic.p3, budget, cancelled);
    }
    let accuracy = FLATTEN_TOLERANCE * 0.1;
    let p1x2 = 3.0 * cubic.p1.to_vec2() - cubic.p0.to_vec2();
    let p2x2 = 3.0 * cubic.p2.to_vec2() - cubic.p3.to_vec2();
    let ratio = (p2x2 - p1x2).hypot2() / (432.0 * accuracy * accuracy);
    if !ratio.is_finite() {
        return Err(err(
            "svg.precision.flatten",
            "SVG cubic subdivision arithmetic is not finite",
        ));
    }
    let quads = cubic.to_quads(accuracy);
    let (minimum, maximum) = quads.size_hint();
    let count = points
        .len()
        .checked_add(minimum)
        .ok_or(err("svg.limits.edges", "SVG flattening count overflows"))?;
    if maximum != Some(minimum)
        || count > budget
        || work.checked_add(minimum).is_none_or(|count| count > budget)
        || count
            .checked_mul(core::mem::size_of::<[f64; 2]>())
            .is_none_or(|bytes| bytes > isize::MAX as usize)
    {
        return Err(err(
            "svg.limits.edges",
            "SVG flattening exceeds work or addressable storage",
        ));
    }
    for (_, _, quad) in quads {
        flatten_svg_quad(quad, points, work, budget, cancelled)?;
    }
    Ok(())
}

/// Emits quadratic chords only when a convex-hull deviation certificate fits the remaining 90% error.
/// For any quadratic point, the control point has weight at most one half; distance to the chord
/// is convex, so half the control-to-segment distance bounds the entire curve in both directions.
/// Ordered de Casteljau subdivision observes cancellation at every bounded local interval and
/// retains cusp/reversal extrema that a zero-curvature line shortcut would otherwise omit.
/// # Errors
/// Returns checked work/storage, finite geometry or cancellation errors before contour publication.
fn flatten_svg_quad(
    quad: QuadBez,
    points: &mut Contour,
    work: &mut usize,
    budget: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), RenderError> {
    let mut pending = Vec::new();
    pending
        .try_reserve(1)
        .map_err(|_| err("svg.allocation", "SVG flattening allocation failed"))?;
    pending.push(quad);
    while let Some(quad) = pending.pop() {
        if cancelled() {
            return Err(err("svg.cancelled", "SVG serialization was cancelled"));
        }
        *work = work
            .checked_add(1)
            .filter(|count| *count <= budget)
            .ok_or(err(
                "svg.limits.edges",
                "SVG flattening work exceeds edge limit",
            ))?;
        let delta = quad.p2 - quad.p0;
        let length = delta.hypot();
        let offset = quad.p1 - quad.p0;
        let distance = if length == 0.0 {
            offset.hypot()
        } else {
            let direction = delta / length;
            (offset - direction * offset.dot(direction).clamp(0.0, length)).hypot()
        };
        if !distance.is_finite() {
            return Err(err("vector.contour", "quadratic hull is not finite"));
        }
        if distance * 0.5 > FLATTEN_TOLERANCE * 0.9 {
            let (left, right) = quad.subdivide();
            if left == quad || right == quad {
                return Err(err(
                    "svg.precision.flatten",
                    "SVG quadratic cannot meet flattening tolerance",
                ));
            }
            pending
                .try_reserve(2)
                .map_err(|_| err("svg.allocation", "SVG flattening allocation failed"))?;
            pending.push(right);
            pending.push(left);
        } else {
            push_svg_vertex(points, quad.p2, budget, cancelled)?;
        }
    }
    Ok(())
}

/// One conservative control hull for a line or coordinate-monotonic cubic.
#[derive(Clone, Copy)]
struct CertifiedHull {
    controls: [Point2; 4],
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
    monotone_x: bool,
    monotone_y: bool,
}

/// Certifies a single simple contour with exact line/cubic winding.
///
/// A cubic is individually injective when its control coordinates are
/// monotonic along at least one axis. Separating control-hull planes and adjacent
/// hulls touching only at their common endpoint then prove the whole contour
/// simple without flattening or treating a straight centerline as proof of a
/// safe filled outline. Uncertified shapes use primitive-local normalization.
///
/// # Errors
/// Returns cancellation while checking a contour.
fn certified_simple_winding(
    segments: &[CurveSegment],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<bool>, RenderError> {
    if segments.len() < 3 || segments.len() > 1024 {
        return Ok(None);
    }
    let origin = segments[0].start();
    let mut hulls = Vec::with_capacity(segments.len());
    let mut area: f64 = 0.0;
    let mut area_error = 0.0;
    for segment in segments {
        if cancelled() {
            return Err(err("svg.cancelled", "SVG serialization was cancelled"));
        }
        let controls = match segment {
            CurveSegment::Line(line) => [line.start(), line.start(), line.end(), line.end()],
            CurveSegment::CubicBezier(cubic) => [
                cubic.start(),
                cubic.control_1(),
                cubic.control_2(),
                cubic.end(),
            ],
        };
        let xs = controls.map(|p| p.x);
        let ys = controls.map(|p| p.y);
        let monotone = |v: [f64; 4]| -> bool {
            v[0] != v[3]
                && ((v[0] <= v[1] && v[1] <= v[2] && v[2] <= v[3])
                    || (v[0] >= v[1] && v[1] >= v[2] && v[2] >= v[3]))
        };
        let monotone_x = monotone(xs);
        let monotone_y = monotone(ys);
        if !monotone_x && !monotone_y {
            return Ok(None);
        }
        hulls.push(CertifiedHull {
            controls,
            min_x: xs.into_iter().fold(f64::INFINITY, f64::min),
            max_x: xs.into_iter().fold(f64::NEG_INFINITY, f64::max),
            min_y: ys.into_iter().fold(f64::INFINITY, f64::min),
            max_y: ys.into_iter().fold(f64::NEG_INFINITY, f64::max),
            monotone_x,
            monotone_y,
        });
        let relative = controls.map(|p| Point::new(p.x - origin.x, p.y - origin.y));
        let segment_area =
            CubicBez::new(relative[0], relative[1], relative[2], relative[3]).signed_area();
        let absolute_bound = controls
            .iter()
            .map(|p| p.x.abs().max(p.y.abs()))
            .fold(0.0, f64::max);
        let relative_bound = relative
            .iter()
            .map(|p| p.x.abs().max(p.y.abs()))
            .fold(0.0, f64::max);
        // Bounds recentering, the analytic cubic polynomial's weighted
        // products, and request-local accumulation. Uncertain signs fall back.
        area_error += 128.0 * f64::EPSILON * (absolute_bound + relative_bound) * relative_bound
            + 4.0 * f64::EPSILON * (area.abs() + segment_area.abs())
            + f64::MIN_POSITIVE;
        area += segment_area;
    }
    if !area.is_finite() || !area_error.is_finite() || area.abs() <= area_error {
        return Ok(None);
    }
    for i in 0..hulls.len() {
        if cancelled() {
            return Err(err("svg.cancelled", "SVG serialization was cancelled"));
        }
        for j in (i + 1)..hulls.len() {
            let a = hulls[i];
            let b = hulls[j];
            if a.max_x < b.min_x || b.max_x < a.min_x || a.max_y < b.min_y || b.max_y < a.min_y {
                continue;
            }
            let adjacent = j == i + 1 || (i == 0 && j + 1 == hulls.len());
            if !adjacent {
                if separated_control_hulls(&a.controls, &b.controls) {
                    continue;
                }
                // A monotone curve reaches a touching hull plane only at
                // its endpoint. Distinct endpoints prove the curves disjoint.
                let separated_x = a.monotone_x
                    && b.monotone_x
                    && ((a.max_x == b.min_x
                        && touching_endpoints_differ(&segments[i], &segments[j], a.max_x, true))
                        || (b.max_x == a.min_x
                            && touching_endpoints_differ(
                                &segments[i],
                                &segments[j],
                                b.max_x,
                                true,
                            )));
                let separated_y = a.monotone_y
                    && b.monotone_y
                    && ((a.max_y == b.min_y
                        && touching_endpoints_differ(&segments[i], &segments[j], a.max_y, false))
                        || (b.max_y == a.min_y
                            && touching_endpoints_differ(
                                &segments[i],
                                &segments[j],
                                b.max_y,
                                false,
                            )));
                if separated_x || separated_y {
                    continue;
                }
                return Ok(None);
            }
            let joint = if j == i + 1 {
                segments[j].start()
            } else {
                segments[0].start()
            };
            let x_touch = a.monotone_x
                && b.monotone_x
                && ((a.max_x == joint.x && b.min_x == joint.x)
                    || (b.max_x == joint.x && a.min_x == joint.x));
            let y_touch = a.monotone_y
                && b.monotone_y
                && ((a.max_y == joint.y && b.min_y == joint.y)
                    || (b.max_y == joint.y && a.min_y == joint.y));
            if !x_touch && !y_touch && !separated_at_joint(&a.controls, &b.controls, joint) {
                return Ok(None);
            }
        }
    }
    Ok(Some(area < 0.0))
}

/// Proves disjoint control hulls along a polygon-edge normal with rounding bounds.
///
/// Every cubic lies in its control hull. The separating-axis theorem requires
/// only normals of hull edges; checking all control-point pairs includes those
/// edges without constructing another approximate polygon. Uncertain or
/// touching intervals fail closed to primitive-local normalization.
fn separated_control_hulls(a: &[Point2; 4], b: &[Point2; 4]) -> bool {
    let origin = a[0];
    for controls in [a, b] {
        for i in 0..4 {
            for j in (i + 1)..4 {
                let normal =
                    Point2::new(controls[j].y - controls[i].y, controls[i].x - controls[j].x);
                if !normal.x.is_finite() || !normal.y.is_finite() || normal == Point2::new(0.0, 0.0)
                {
                    continue;
                }
                let interval = |points: &[Point2; 4]| {
                    points
                        .iter()
                        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), point| {
                            let (value, error) = bounded_projection(*point, origin, normal);
                            (lo.min(value - error), hi.max(value + error))
                        })
                };
                let (a_min, a_max) = interval(a);
                let (b_min, b_max) = interval(b);
                if a_max < b_min || b_max < a_min {
                    return true;
                }
            }
        }
    }
    false
}

/// Proves adjacent hulls occupy opposite open half-planes except for their joint.
///
/// Candidate planes bisect pairs of control rays, including narrow tapered
/// stroke tips whose rails share overlapping axis-aligned bounds. Strict signs
/// for every nonjoint control prove that each curve's interior stays on its own
/// side. Rounding uncertainty rejects the certificate rather than widening it.
fn separated_at_joint(a: &[Point2; 4], b: &[Point2; 4], joint: Point2) -> bool {
    for a_point in a.iter().filter(|point| **point != joint) {
        let a_ray = Point2::new(a_point.x - joint.x, a_point.y - joint.y);
        let a_length = a_ray.x.hypot(a_ray.y);
        for b_point in b.iter().filter(|point| **point != joint) {
            let b_ray = Point2::new(b_point.x - joint.x, b_point.y - joint.y);
            let b_length = b_ray.x.hypot(b_ray.y);
            if a_length == 0.0 || b_length == 0.0 || !a_length.is_finite() || !b_length.is_finite()
            {
                continue;
            }
            let normal = Point2::new(
                a_ray.x / a_length - b_ray.x / b_length,
                a_ray.y / a_length - b_ray.y / b_length,
            );
            let on_side = |points: &[Point2; 4], positive: bool| {
                points.iter().filter(|point| **point != joint).all(|point| {
                    let (value, error) = bounded_projection(*point, joint, normal);
                    if positive {
                        value > error
                    } else {
                        value < -error
                    }
                })
            };
            if on_side(a, true) && on_side(b, false) {
                return true;
            }
        }
    }
    false
}

/// Bounds subtraction, multiplication, and addition error for a separating projection.
///
/// The normal is an arbitrary finite f64 plane, so its construction need not
/// approximate a particular tangent. A deliberately conservative absolute bound
/// covers the original-coordinate subtraction and two products. Nonfinite
/// arithmetic produces an infinite bound and can never certify separation.
fn bounded_projection(point: Point2, origin: Point2, normal: Point2) -> (f64, f64) {
    let value = (point.x - origin.x) * normal.x + (point.y - origin.y) * normal.y;
    let error = 16.0
        * f64::EPSILON
        * (normal.x.abs() * (point.x.abs() + origin.x.abs())
            + normal.y.abs() * (point.y.abs() + origin.y.abs()));
    if value.is_finite() && error.is_finite() {
        (value, error.max(f64::MIN_POSITIVE))
    } else {
        (0.0, f64::INFINITY)
    }
}

/// Rejects whole contours that can collapse to a line or point on the Boolean grid.
///
/// A triangle spanning well beyond worst-case vertex rounding proves that a
/// contour retains two-dimensional extent, independently of its algebraic area
/// or self-intersections. This catches lost small holes/components even when a
/// larger contour leaves the overall normalized primitive nonempty. It does
/// not certify topology below the documented flattening/grid resolution.
///
/// # Errors
/// Returns a precision diagnostic for unresolvable or numerically uncertain extent.
fn ensure_grid_resolvable_contour(
    contour: &Contour,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), RenderError> {
    let Some(origin) = contour.first() else {
        return Err(err(
            "svg.precision.contour",
            "SVG contour has no grid-resolvable extent",
        ));
    };
    let mut far = origin;
    let mut distance = 0.0;
    for (index, point) in contour.iter().enumerate() {
        if index & 1023 == 0 && cancelled() {
            return Err(err("svg.cancelled", "SVG serialization was cancelled"));
        }
        let next = (point[0] - origin[0]).hypot(point[1] - origin[1]);
        if next > distance {
            far = point;
            distance = next;
        }
    }
    let dx = far[0] - origin[0];
    let dy = far[1] - origin[1];
    let grid = 2.0 / BOOLEAN_SCALE;
    for (index, point) in contour.iter().enumerate() {
        if index & 1023 == 0 && cancelled() {
            return Err(err("svg.cancelled", "SVG serialization was cancelled"));
        }
        let px = point[0] - origin[0];
        let py = point[1] - origin[1];
        let cross = dx * py - dy * px;
        let rounding = grid * (dx.abs() + dy.abs() + px.abs() + py.abs()) + 2.0 * grid * grid;
        let coordinate_bound = point[0]
            .abs()
            .max(point[1].abs())
            .max(far[0].abs())
            .max(far[1].abs())
            .max(origin[0].abs())
            .max(origin[1].abs());
        let floating = 32.0
            * f64::EPSILON
            * (dx.abs() * py.abs()
                + dy.abs() * px.abs()
                + coordinate_bound * (dx.abs() + dy.abs() + px.abs() + py.abs()));
        if cross.is_finite()
            && (rounding + floating).is_finite()
            && cross.abs() > rounding + floating
        {
            return Ok(());
        }
    }
    Err(err(
        "svg.precision.contour",
        "SVG contour is too narrow for Boolean normalization",
    ))
}

/// Proves monotone hulls meet one boundary plane at distinct curve endpoints.
fn touching_endpoints_differ(a: &CurveSegment, b: &CurveSegment, plane: f64, x_axis: bool) -> bool {
    let coordinate = |p: Point2| if x_axis { p.x } else { p.y };
    let a_point = if coordinate(a.start()) == plane {
        a.start()
    } else {
        a.end()
    };
    let b_point = if coordinate(b.start()) == plane {
        b.start()
    } else {
        b.end()
    };
    a_point != b_point
}

/// Checks finite controls, exact continuity, and closure before any SVG contour classification.
///
/// # Errors
/// Rejects a malformed public outline rather than certifying or drawing a different path.
fn validate_contour(segments: &[CurveSegment]) -> Result<(), RenderError> {
    let Some(first) = segments.first() else {
        return Err(err("svg.contour", "SVG filled contour has no segments"));
    };
    for segment in segments {
        let controls = match segment {
            CurveSegment::Line(line) => [line.start(), line.start(), line.end(), line.end()],
            CurveSegment::CubicBezier(cubic) => [
                cubic.start(),
                cubic.control_1(),
                cubic.control_2(),
                cubic.end(),
            ],
        };
        if controls.iter().any(|point| !point.is_finite()) {
            return Err(err("svg.contour", "SVG contour controls must be finite"));
        }
    }
    if segments.last().unwrap().end() != first.start()
        || segments
            .windows(2)
            .any(|pair| pair[0].end() != pair[1].start())
    {
        return Err(err(
            "svg.contour",
            "SVG filled contour is not connected and closed",
        ));
    }
    Ok(())
}

/// Proves a validated contour is exactly one point using every original control.
///
/// Equal endpoints alone do not prove a cubic empty. This certificate uses no
/// area, flattening, grid, or epsilon test and never omits a positive contour.
/// Callers validate finite controls and closure before using it and charge
/// omitted canonical edges to the request budget.
///
/// # Errors
/// Returns cancellation while examining original line or cubic controls.
fn exact_point_contour(
    segments: &[CurveSegment],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<bool, RenderError> {
    let Some(first) = segments.first() else {
        return Ok(false);
    };
    let origin = first.start();
    for segment in segments {
        if cancelled() {
            return Err(err("svg.cancelled", "SVG serialization was cancelled"));
        }
        let controls = match segment {
            CurveSegment::Line(line) => [line.start(), line.start(), line.end(), line.end()],
            CurveSegment::CubicBezier(cubic) => [
                cubic.start(),
                cubic.control_1(),
                cubic.control_2(),
                cubic.end(),
            ],
        };
        if controls.iter().any(|point| *point != origin) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Appends a canonical path with full roundtrip coordinates and selected winding.
///
/// # Errors
/// Returns allocation or formatting errors before publication.
fn append_curve(
    data: &mut String,
    segments: &[CurveSegment],
    reverse: bool,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), RenderError> {
    if segments.is_empty() {
        return Ok(());
    }
    data.try_reserve(segments.len().saturating_mul(160))
        .map_err(|_| err("svg.allocation", "SVG curve allocation failed"))?;
    let start = if reverse {
        segments.last().unwrap().end()
    } else {
        segments[0].start()
    };
    write!(data, "M{} {}", start.x, start.y)
        .map_err(|_| err("svg.format", "SVG curve formatting failed"))?;
    if reverse {
        for (index, segment) in segments.iter().rev().enumerate() {
            if index & 1023 == 0 && cancelled() {
                return Err(err("svg.cancelled", "SVG serialization was cancelled"));
            }
            append_segment(data, segment, true)?;
        }
    } else {
        for (index, segment) in segments.iter().enumerate() {
            if index & 1023 == 0 && cancelled() {
                return Err(err("svg.cancelled", "SVG serialization was cancelled"));
            }
            append_segment(data, segment, false)?;
        }
    }
    data.push('Z');
    Ok(())
}

/// Appends one oriented canonical line or cubic without rounding coordinates.
///
/// # Errors
/// Returns SVG formatting errors.
fn append_segment(
    data: &mut String,
    segment: &CurveSegment,
    reverse: bool,
) -> Result<(), RenderError> {
    match segment {
        CurveSegment::Line(line) => {
            let end = if reverse { line.start() } else { line.end() };
            write!(data, "L{} {}", end.x, end.y)
        }
        CurveSegment::CubicBezier(cubic) => {
            let (a, b, end) = if reverse {
                (cubic.control_2(), cubic.control_1(), cubic.start())
            } else {
                (cubic.control_1(), cubic.control_2(), cubic.end())
            };
            write!(data, "C{} {},{} {},{} {}", a.x, a.y, b.x, b.y, end.x, end.y)
        }
    }
    .map_err(|_| err("svg.format", "SVG segment formatting failed"))
}

/// Appends one normalized outer or hole polygon with the required NonZero orientation.
///
/// # Errors
/// Returns precision, allocation, or formatting diagnostics.
fn append_polygon(
    data: &mut String,
    ring: &[[f64; 2]],
    axis: f64,
    hole: bool,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), RenderError> {
    if ring.len() < 3 {
        return Err(err(
            "svg.overlay.primitive",
            "normalized SVG contour is degenerate",
        ));
    }
    let reverse = (signed_area(ring) < 0.0) != hole;
    data.try_reserve(ring.len().saturating_mul(60))
        .map_err(|_| err("svg.allocation", "SVG polygon allocation failed"))?;
    let first = if reverse {
        ring[ring.len() - 1]
    } else {
        ring[0]
    };
    write!(data, "M{} {}", first[0] * axis, first[1] * axis)
        .map_err(|_| err("svg.format", "SVG polygon formatting failed"))?;
    if reverse {
        for (index, p) in ring[..ring.len() - 1].iter().rev().enumerate() {
            if index & 1023 == 0 && cancelled() {
                return Err(err("svg.cancelled", "SVG serialization was cancelled"));
            }
            write!(data, "L{} {}", p[0] * axis, p[1] * axis)
                .map_err(|_| err("svg.format", "SVG polygon formatting failed"))?;
        }
    } else {
        for (index, p) in ring[1..].iter().enumerate() {
            if index & 1023 == 0 && cancelled() {
                return Err(err("svg.cancelled", "SVG serialization was cancelled"));
            }
            write!(data, "L{} {}", p[0] * axis, p[1] * axis)
                .map_err(|_| err("svg.format", "SVG polygon formatting failed"))?;
        }
    }
    data.push('Z');
    Ok(())
}

/// Computes recentered signed area so large translations do not erase narrow rings.
fn signed_area(ring: &[[f64; 2]]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let origin = ring[0];
    ring.iter()
        .enumerate()
        .map(|(i, a)| {
            let b = ring[(i + 1) % ring.len()];
            (a[0] - origin[0]) * (b[1] - origin[1]) - (b[0] - origin[0]) * (a[1] - origin[1])
        })
        .sum::<f64>()
        * 0.5
}

/// Recognizes cancellation only from identical canonical controls, before approximation.
///
/// Flattened equality cannot establish coincidence: a real narrow curved sliver
/// may flatten to the same polygon as its neighbor. Exact forward/reversed
/// segment cycles prove EvenOdd cancellation, and exact reversed cycles prove
/// NonZero cancellation; all other empty Boolean results fail closed.
fn exact_empty_pair(shape: &[&[CurveSegment]], rule: FillRule) -> bool {
    if shape.len() != 2 || shape[0].len() != shape[1].len() || shape[0].is_empty() {
        return false;
    }
    let a = &shape[0];
    let b = &shape[1];
    let n = a.len();
    b.iter().enumerate().any(|(offset, _)| {
        let same = (0..n).all(|i| a[i] == b[(offset + i) % n]);
        let reverse = (0..n).all(|i| reversed_segment_eq(&a[i], &b[(offset + n - i) % n]));
        match rule {
            FillRule::EvenOdd => same || reverse,
            FillRule::NonZero => reverse,
            _ => false,
        }
    })
}

/// Compares exact canonical controls under reversal without resampling either segment.
fn reversed_segment_eq(a: &CurveSegment, b: &CurveSegment) -> bool {
    match (a, b) {
        (CurveSegment::Line(a), CurveSegment::Line(b)) => {
            a.start() == b.end() && a.end() == b.start()
        }
        (CurveSegment::CubicBezier(a), CurveSegment::CubicBezier(b)) => {
            a.start() == b.end()
                && a.end() == b.start()
                && a.control_1() == b.control_2()
                && a.control_2() == b.control_1()
        }
        _ => false,
    }
}

/// Creates a stable final-consumer SVG error.
fn err(path: &'static str, message: &'static str) -> RenderError {
    RenderError::new(path, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_geometry::{CubicBezierSegment, CurvePath, PathClosure};

    /// Checks the geometric error budget independently on bowed, inflected, cusp, loop and tiny cubics.
    /// Cubic-to-quadratic third-difference bounds certify each lazy interval; dense Hausdorff samples
    /// supplement that proof and do not replace it. Straight monotone chords publish only their endpoint.
    /// # Panics
    /// Panics if error allocation, exact chord emission, finite preflight or cancellation changes.
    #[test]
    fn svg_adaptive_cubic_quality_and_exact_chords_are_bounded() {
        let cases = [
            CubicBez::new((0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)),
            CubicBez::new((0.0, 0.0), (0.2, 1.0), (0.8, -1.0), (1.0, 0.0)),
            CubicBez::new((0.0, 0.0), (1.0, 1.0), (-1.0, -1.0), (0.0, 0.0)),
            CubicBez::new((0.0, 0.0), (1.0, 1.0), (-1.0, 1.0), (0.0, 0.0)),
            CubicBez::new(
                (0.5, 0.5),
                (0.5 + 1e-12, 0.5),
                (0.5, 0.5 + 1e-12),
                (0.5 + 1e-12, 0.5 + 1e-12),
            ),
            CubicBez::new(
                (100.0, 100.0),
                (100.0, 101.0),
                (101.0, 101.0),
                (101.0, 100.0),
            ),
        ];
        for cubic in cases {
            // The proof is in real arithmetic; this separately bounded allowance accounts for
            // recentered f64 evaluation at the valid translated coordinates in the witness.
            let rounding = 32.0
                * f64::EPSILON
                * [cubic.p0, cubic.p1, cubic.p2, cubic.p3]
                    .into_iter()
                    .map(|p| p.x.abs().max(p.y.abs()))
                    .fold(1.0, f64::max);
            for (t0, t1, quad) in cubic.to_quads(FLATTEN_TOLERANCE * 0.1) {
                let sub = cubic.subsegment(t0..t1);
                let third = (sub.p3 - sub.p2) - 2.0 * (sub.p2 - sub.p1) + (sub.p1 - sub.p0);
                let bound = third.hypot() / (12.0 * 3.0_f64.sqrt());
                assert!(
                    bound <= FLATTEN_TOLERANCE * 0.1 + rounding,
                    "cubic/quad bound={bound}"
                );
                let mut quad_points = vec![[quad.p0.x, quad.p0.y]];
                flatten_svg_quad(quad, &mut quad_points, &mut 0, usize::MAX, &|| false).unwrap();
                use kurbo::ParamCurveNearest as _;
                // Noncollinear quadratics have unique local point addresses. Collinear reversal
                // extrema are checked below against their independent analytic closed form.
                let a = quad.p1 - quad.p0;
                let b = quad.p2 - quad.p0;
                if a.cross(b) != 0.0 {
                    for pair in quad_points.windows(2) {
                        let t0 = quad.nearest(Point::new(pair[0][0], pair[0][1]), 1e-14).t;
                        let t1 = quad.nearest(Point::new(pair[1][0], pair[1][1]), 1e-14).t;
                        let sub = quad.subsegment(t0..t1);
                        let chord_bound =
                            0.5 * test_point_segment_distance(sub.p1, pair[0], pair[1]);
                        assert!(
                            chord_bound <= FLATTEN_TOLERANCE * 0.9 + rounding,
                            "independent quadratic/chord bound={chord_bound} cubic={cubic:?} quad={quad:?} ts={t0},{t1} pair={pair:?}"
                        );
                    }
                }
                for i in 0..=16 {
                    let u = f64::from(i) / 16.0;
                    assert!(
                        (cubic.eval(t0 + (t1 - t0) * u) - quad.eval(u)).hypot() <= bound + rounding
                    );
                }
            }
            let mut points = vec![[cubic.p0.x, cubic.p0.y]];
            flatten_svg_cubic(cubic, &mut points, &mut 0, usize::MAX, &|| false).unwrap();
            if cubic == cases[2] {
                let extremum = 1.0 / (2.0 * 3.0_f64.sqrt());
                let maximum = points
                    .iter()
                    .map(|p| p[0])
                    .fold(f64::NEG_INFINITY, f64::max);
                let minimum = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
                assert!((maximum - extremum).abs() <= FLATTEN_TOLERANCE);
                assert!((minimum + extremum).abs() <= FLATTEN_TOLERANCE);
            }
            for i in 0..=1024 {
                let point = cubic.eval(f64::from(i) / 1024.0);
                let distance = points
                    .windows(2)
                    .map(|pair| test_point_segment_distance(point, pair[0], pair[1]))
                    .fold(f64::INFINITY, f64::min);
                assert!(
                    distance <= FLATTEN_TOLERANCE + rounding,
                    "cubic/polyline diagnostic={distance}"
                );
            }
        }
        for cubic in [
            CubicBez::new((0.0, 0.0), (0.0, 0.0), (1.0, 1.0), (1.0, 1.0)),
            CubicBez::new((0.0, 0.0), (1.0, 0.0), (1.0, 0.0), (4.0, 0.0)),
            CubicBez::new((0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)),
        ] {
            let mut points = vec![[cubic.p0.x, cubic.p0.y]];
            flatten_svg_cubic(cubic, &mut points, &mut 0, usize::MAX, &|| false).unwrap();
            assert_eq!(
                points,
                vec![[cubic.p0.x, cubic.p0.y], [cubic.p3.x, cubic.p3.y]]
            );
        }
        let mut points = Vec::new();
        assert_eq!(
            flatten_svg_cubic(cases[0], &mut points, &mut 0, 1, &|| false)
                .unwrap_err()
                .path(),
            "svg.limits.edges"
        );
        assert!(points.is_empty());
        let extreme = CubicBez::new((0.0, 0.0), (1e308, 1e308), (-1e308, 1e308), (1.0, 0.0));
        assert_eq!(
            flatten_svg_cubic(extreme, &mut points, &mut 0, usize::MAX, &|| false)
                .unwrap_err()
                .path(),
            "svg.precision.flatten"
        );
        let calls = std::sync::atomic::AtomicUsize::new(0);
        assert_eq!(
            flatten_svg_cubic(cases[0], &mut points, &mut 0, usize::MAX, &|| {
                calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1 == 12
            })
            .unwrap_err()
            .path(),
            "svg.cancelled"
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 12);
    }

    /// Computes an independent recentered point-to-closed-line-segment distance for quality witnesses.
    fn test_point_segment_distance(point: Point, a: [f64; 2], b: [f64; 2]) -> f64 {
        let start = Point::new(a[0], a[1]);
        let delta = Point::new(b[0], b[1]) - start;
        let t = if delta.hypot2() == 0.0 {
            0.0
        } else {
            ((point - start).dot(delta) / delta.hypot2()).clamp(0.0, 1.0)
        };
        (point - (start + delta * t)).hypot()
    }

    /// Preserves analytic circle holes, mixed winding, self-overlap and touching boundaries under both fills.
    /// Witnesses lie well away from boundaries; line clipping remains covered by its directed winding test.
    /// # Panics
    /// Panics if normalized fill membership changes, finite limits are ignored or cancellation is lost.
    #[test]
    fn svg_adaptive_normalization_preserves_fill_topology() {
        let circle = |radius: f64, reverse: bool| {
            let k = radius * 0.5522847498307936;
            let mut curves = vec![
                CubicBez::new(
                    (0.5 + radius, 0.5),
                    (0.5 + radius, 0.5 + k),
                    (0.5 + k, 0.5 + radius),
                    (0.5, 0.5 + radius),
                ),
                CubicBez::new(
                    (0.5, 0.5 + radius),
                    (0.5 - k, 0.5 + radius),
                    (0.5 - radius, 0.5 + k),
                    (0.5 - radius, 0.5),
                ),
                CubicBez::new(
                    (0.5 - radius, 0.5),
                    (0.5 - radius, 0.5 - k),
                    (0.5 - k, 0.5 - radius),
                    (0.5, 0.5 - radius),
                ),
                CubicBez::new(
                    (0.5, 0.5 - radius),
                    (0.5 + k, 0.5 - radius),
                    (0.5 + radius, 0.5 - k),
                    (0.5 + radius, 0.5),
                ),
            ];
            if reverse {
                curves = curves
                    .into_iter()
                    .rev()
                    .map(|c| CubicBez::new(c.p3, c.p2, c.p1, c.p0))
                    .collect();
            }
            curves
                .into_iter()
                .map(|c| {
                    CurveSegment::CubicBezier(
                        CubicBezierSegment::new(
                            Point2::new(c.p0.x, c.p0.y),
                            Point2::new(c.p1.x, c.p1.y),
                            Point2::new(c.p2.x, c.p2.y),
                            Point2::new(c.p3.x, c.p3.y),
                        )
                        .unwrap(),
                    )
                })
                .collect::<Vec<_>>()
        };
        for reverse in [false, true] {
            let outer = circle(0.4, false);
            let inner = circle(0.2, reverse);
            let shape = vec![
                flatten_svg_contour(&outer, 1.0, usize::MAX, &|| false).unwrap(),
                flatten_svg_contour(&inner, 1.0, usize::MAX, &|| false).unwrap(),
            ];
            for fill in [FillRule::EvenOdd, FillRule::NonZero] {
                let normalized = shape
                    .overlay_with_fixed_scale_as::<i64>(
                        &Vec::<[f64; 2]>::new(),
                        OverlayRule::Subject,
                        fill,
                        BOOLEAN_SCALE,
                    )
                    .unwrap();
                let rings: Vec<_> = normalized.iter().flatten().map(Vec::as_slice).collect();
                for (point, expected) in [
                    ([0.5, 0.5], fill == FillRule::NonZero && !reverse),
                    ([0.8, 0.5], true),
                    ([0.95, 0.5], false),
                    ([0.5, 0.95], false),
                ] {
                    assert_eq!(test_nonzero_contains(&rings, point), expected);
                }
            }
        }
        let rectangle = |x0, x1| {
            CurvePath::polyline(
                vec![
                    Point2::new(x0, 0.1),
                    Point2::new(x1, 0.1),
                    Point2::new(x1, 0.9),
                    Point2::new(x0, 0.9),
                ],
                PathClosure::Closed,
            )
            .unwrap()
        };
        let left = rectangle(-0.2, 0.5);
        let right = rectangle(0.5, 1.2);
        let shape = vec![
            flatten_svg_contour(left.segments(), 1.0, usize::MAX, &|| false).unwrap(),
            flatten_svg_contour(right.segments(), 1.0, usize::MAX, &|| false).unwrap(),
        ];
        let clip = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        for fill in [FillRule::EvenOdd, FillRule::NonZero] {
            let normalized = shape
                .overlay_with_fixed_scale_as::<i64>(
                    &clip,
                    OverlayRule::Intersect,
                    fill,
                    BOOLEAN_SCALE,
                )
                .unwrap();
            let rings: Vec<_> = normalized.iter().flatten().map(Vec::as_slice).collect();
            for (point, expected) in [
                ([0.25, 0.5], true),
                ([0.75, 0.5], true),
                ([-0.1, 0.5], false),
                ([1.1, 0.5], false),
                ([0.5, 0.95], false),
            ] {
                assert_eq!(test_nonzero_contains(&rings, point), expected);
            }
        }
        let bowtie = CurvePath::polyline(
            vec![
                Point2::new(0.1, 0.1),
                Point2::new(0.9, 0.9),
                Point2::new(0.1, 0.9),
                Point2::new(0.9, 0.1),
            ],
            PathClosure::Closed,
        )
        .unwrap();
        let shape =
            vec![flatten_svg_contour(bowtie.segments(), 1.0, usize::MAX, &|| false).unwrap()];
        for fill in [FillRule::EvenOdd, FillRule::NonZero] {
            let normalized = shape
                .overlay_with_fixed_scale_as::<i64>(
                    &Vec::<[f64; 2]>::new(),
                    OverlayRule::Subject,
                    fill,
                    BOOLEAN_SCALE,
                )
                .unwrap();
            let rings: Vec<_> = normalized.iter().flatten().map(Vec::as_slice).collect();
            assert!(test_nonzero_contains(&rings, [0.5, 0.2]));
            assert!(test_nonzero_contains(&rings, [0.5, 0.8]));
            assert!(!test_nonzero_contains(&rings, [0.2, 0.5]));
        }
    }

    /// Classifies far-from-boundary witnesses with an independent directed ray-crossing count.
    fn test_nonzero_contains(rings: &[&[[f64; 2]]], point: [f64; 2]) -> bool {
        let mut winding = 0i32;
        for ring in rings {
            for (index, a) in ring.iter().enumerate() {
                let b = ring[(index + 1) % ring.len()];
                if (a[1] <= point[1] && b[1] > point[1]) || (b[1] <= point[1] && a[1] > point[1]) {
                    let x = a[0] + (point[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
                    if x > point[0] {
                        winding += if b[1] > a[1] { 1 } else { -1 };
                    }
                }
            }
        }
        winding != 0
    }

    /// Exercises both compound fill rules directly without inventing EvenOdd canonical strokes.
    ///
    /// Canonical stroke construction permits only NonZero. The serializer's
    /// primitive-local routine nevertheless preserves either rule when exact
    /// point contours accompany positive, cancelling, or subgrid contours.
    /// # Panics
    /// Panics on lost fill semantics, changed exact controls, or silent subgrid omission.
    #[test]
    fn exact_point_members_preserve_both_compound_fill_rules() {
        let point = Point2::new(4.0, 4.0);
        let empty = vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(point, point, point, point).unwrap(),
        )];
        let square = |size: f64| {
            CurvePath::polyline(
                vec![
                    point,
                    Point2::new(point.x + size, point.y),
                    Point2::new(point.x + size, point.y + size),
                    Point2::new(point.x, point.y + size),
                ],
                PathClosure::Closed,
            )
            .unwrap()
            .segments()
            .to_vec()
        };
        let positive = square(2.0);
        let tiny = square(1.0e-13);
        let backtracking = CurvePath::polyline(
            vec![
                point,
                Point2::new(5.0, 4.0),
                Point2::new(6.0, 4.0),
                Point2::new(5.0, 4.0),
            ],
            PathClosure::Closed,
        )
        .unwrap();
        let disconnected = vec![
            empty[0],
            CurveSegment::Line(
                toniator_geometry::LineSegment::new(Point2::new(5.0, 5.0), Point2::new(5.0, 5.0))
                    .unwrap(),
            ),
        ];
        let serialize = |contours: &[&[CurveSegment]], rule| {
            let mut join = SvgJoin::new(
                &CanvasSpec {
                    width: 8.0,
                    height: 8.0,
                },
                100_000,
                &|| false,
            )
            .unwrap();
            let mut data = String::new();
            let result = if let [segments] = contours {
                join.path(&mut data, segments, rule)
            } else {
                join.contours(&mut data, contours, rule)
            };
            result.map(|()| data)
        };
        for rule in [CanonicalFillRule::EvenOdd, CanonicalFillRule::NonZero] {
            assert_eq!(
                serialize(&[&empty, backtracking.segments()], rule)
                    .expect_err("noncoincident backtracking contours remain fail-closed")
                    .path(),
                "svg.precision.contour"
            );
            assert_eq!(
                serialize(&[&disconnected], rule)
                    .expect_err("closure validation precedes exact empty classification")
                    .path(),
                "svg.contour"
            );
            assert_eq!(
                serialize(&[&empty, &positive, &empty], rule).unwrap(),
                serialize(&[&positive], rule).unwrap()
            );
            assert_eq!(serialize(&[&empty, &empty], rule).unwrap(), "");
            assert_eq!(
                serialize(&[&empty, &positive, &positive], rule).unwrap(),
                serialize(&[&positive, &positive], rule).unwrap()
            );
            if rule == CanonicalFillRule::EvenOdd {
                assert_eq!(
                    serialize(&[&empty, &positive, &positive], rule).unwrap(),
                    ""
                );
                assert_eq!(
                    serialize(&[&empty, &positive, &tiny], rule)
                        .expect_err("a real EvenOdd subgrid contour cannot disappear")
                        .path(),
                    "svg.precision.contour"
                );
            }
        }
    }
}
