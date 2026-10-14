//! Closed filled rectangle intersections owned solely by the SVG consumer.
//!
//! Four half-plane passes preserve directed contour winding. Boundary bridges
//! may retrace for disconnected intersections; their opposite winding cancels.

use std::fmt::Write as _;

use kurbo::common::solve_quadratic;
use toniator_geometry::{CurveSegment, Point2};

use super::{RenderError, svg_join::SvgJoin};

/// One final-output segment; circle arcs retain their original supporting radius.
#[derive(Clone, Copy, Debug)]
enum Segment {
    Line([Point2; 2]),
    Cubic([Point2; 4]),
    Arc {
        radius: f64,
        angles: [f64; 2],
        ends: [Point2; 2],
    },
}

impl Segment {
    /// Returns the segment's authoritative directed endpoints.
    fn ends(self) -> [Point2; 2] {
        match self {
            Self::Line(ends) | Self::Arc { ends, .. } => ends,
            Self::Cubic(p) => [p[0], p[3]],
        }
    }

    /// Evaluates finite line/cubic geometry stably or an origin-centered circular arc.
    fn point(self, t: f64) -> Point2 {
        match self {
            Self::Line(p) => mix(p[0], p[1], t),
            Self::Cubic(p) => split(p, t).0[3],
            Self::Arc {
                radius,
                angles,
                ends,
            } => {
                if t == 0.0 {
                    return ends[0];
                }
                if t == 1.0 {
                    return ends[1];
                }
                let a = angles[0] + (angles[1] - angles[0]) * t;
                Point2::new(radius * a.cos(), radius * a.sin())
            }
        }
    }

    /// Restricts a segment to a positive parameter interval with shared snapped endpoints.
    fn section(self, a: f64, b: f64, ends: [Point2; 2]) -> Self {
        match self {
            Self::Line(_) => Self::Line(ends),
            Self::Cubic(p) => {
                let prefix = if b == 1.0 { p } else { split(p, b).0 };
                let mut q = if a == 0.0 {
                    prefix
                } else {
                    split(prefix, a / b).1
                };
                q[0] = ends[0];
                q[3] = ends[1];
                Self::Cubic(q)
            }
            Self::Arc { radius, angles, .. } => Self::Arc {
                radius,
                angles: [
                    angles[0] + (angles[1] - angles[0]) * a,
                    angles[0] + (angles[1] - angles[0]) * b,
                ],
                ends,
            },
        }
    }

    /// Returns a conservative control box, including a quarter arc's possible cardinal extrema.
    fn bounds(self) -> [f64; 4] {
        let mut result = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        let mut include = |p: Point2| {
            result[0] = result[0].min(p.x);
            result[1] = result[1].min(p.y);
            result[2] = result[2].max(p.x);
            result[3] = result[3].max(p.y);
        };
        match self {
            Self::Line(p) => {
                include(p[0]);
                include(p[1]);
            }
            Self::Cubic(p) => {
                for point in p {
                    include(point);
                }
            }
            Self::Arc {
                radius,
                angles,
                ends,
            } => {
                include(ends[0]);
                include(ends[1]);
                for i in 0..=4 {
                    let a = i as f64 * std::f64::consts::FRAC_PI_2;
                    if angles[0] <= a && a <= angles[1] {
                        include(match i {
                            0 | 4 => Point2::new(radius, 0.0),
                            1 => Point2::new(0.0, radius),
                            2 => Point2::new(-radius, 0.0),
                            _ => Point2::new(0.0, -radius),
                        });
                    }
                }
            }
        }
        result
    }
}

/// One exact rectangle coordinate with the retained half-plane sign.
#[derive(Clone, Copy)]
struct Plane {
    axis: usize,
    value: f64,
    greater: bool,
}

impl Plane {
    /// Computes a signed coordinate distance whose nonnegative side is retained.
    fn distance(self, p: Point2) -> f64 {
        let d = if self.axis == 0 { p.x } else { p.y } - self.value;
        if self.greater { d } else { -d }
    }

    /// Places only a computed intersection coordinate exactly on this boundary.
    fn snap(self, mut p: Point2) -> Point2 {
        if self.axis == 0 {
            p.x = self.value;
        } else {
            p.y = self.value;
        }
        p
    }
}

/// Clips and serializes one closed canonical contour without changing upstream geometry.
///
/// Canonical cubic controls are retained before clipping or orientation.
/// # Errors
/// Returns cancellation, work-limit, allocation, malformed contour, or numerical diagnostics.
pub(super) fn curve(
    data: &mut String,
    source: &[CurveSegment],
    reverse: bool,
    bounds: [f64; 4],
    work: &mut SvgJoin<'_>,
) -> Result<(), RenderError> {
    work.edges(source.len())?;
    let mut segments = Vec::new();
    segments
        .try_reserve(source.len())
        .map_err(|_| error("svg.allocation", "SVG clipping allocation failed"))?;
    for source in source {
        work.check()?;
        segments.push(match source {
            CurveSegment::Line(line) => Segment::Line([line.start(), line.end()]),
            CurveSegment::CubicBezier(c) => {
                Segment::Cubic([c.start(), c.control_1(), c.control_2(), c.end()])
            }
        });
    }
    append(data, &clip(segments, bounds, work)?, reverse, None, work)
}

/// Clips one normalized polygon ring while preserving its outer/hole orientation.
/// # Errors
/// Returns cancellation, work-limit, allocation, or numerical diagnostics.
pub(super) fn polygon(
    data: &mut String,
    ring: &[[f64; 2]],
    axis: f64,
    reverse: bool,
    bounds: [f64; 4],
    work: &mut SvgJoin<'_>,
) -> Result<(), RenderError> {
    work.edges(ring.len())?;
    let mut segments = Vec::new();
    segments
        .try_reserve(ring.len())
        .map_err(|_| error("svg.allocation", "SVG polygon clipping allocation failed"))?;
    for (i, p) in ring.iter().enumerate() {
        work.check()?;
        let next = ring[(i + 1) % ring.len()];
        segments.push(Segment::Line([
            Point2::new(p[0] * axis, p[1] * axis),
            Point2::new(next[0] * axis, next[1] * axis),
        ]));
    }
    append(data, &clip(segments, bounds, work)?, reverse, None, work)
}

/// Intersects an exact circle in its local frame, retaining its original radius in SVG arcs.
///
/// Relative local commands avoid losing tiny radii when added to a large center.
/// # Errors
/// Returns cancellation, work-limit, allocation, or unresolvable numerical diagnostics.
pub(super) fn circle(
    data: &mut String,
    center: Point2,
    radius: f64,
    bounds: [f64; 4],
    work: &mut SvgJoin<'_>,
) -> Result<(), RenderError> {
    work.edges(4)?;
    let points = [
        Point2::new(radius, 0.0),
        Point2::new(0.0, radius),
        Point2::new(-radius, 0.0),
        Point2::new(0.0, -radius),
        Point2::new(radius, 0.0),
    ];
    let segments = (0..4)
        .map(|i| Segment::Arc {
            radius,
            angles: [
                i as f64 * std::f64::consts::FRAC_PI_2,
                (i + 1) as f64 * std::f64::consts::FRAC_PI_2,
            ],
            ends: [points[i], points[i + 1]],
        })
        .collect();
    let local = [
        bounds[0] - center.x,
        bounds[1] - center.y,
        bounds[2] - center.x,
        bounds[3] - center.y,
    ];
    if local.iter().any(|v| !v.is_finite()) {
        return Err(error(
            "svg.precision.clip",
            "SVG circle clipping coordinates are not finite",
        ));
    }
    append(
        data,
        &clip(segments, local, work)?,
        false,
        Some(center),
        work,
    )
}

/// Performs four closed half-plane intersections without introducing an interior edge.
/// # Errors
/// Returns cancellation, work-limit, allocation, closure, or numerical diagnostics.
fn clip(
    mut segments: Vec<Segment>,
    bounds: [f64; 4],
    work: &mut SvgJoin<'_>,
) -> Result<Vec<Segment>, RenderError> {
    for plane in [
        Plane {
            axis: 0,
            value: bounds[0],
            greater: true,
        },
        Plane {
            axis: 0,
            value: bounds[2],
            greater: false,
        },
        Plane {
            axis: 1,
            value: bounds[1],
            greater: true,
        },
        Plane {
            axis: 1,
            value: bounds[3],
            greater: false,
        },
    ] {
        work.check()?;
        let mut kept = Vec::new();
        kept.try_reserve(segments.len().saturating_mul(4))
            .map_err(|_| error("svg.allocation", "SVG clipped interval allocation failed"))?;
        for segment in segments {
            work.check()?;
            let b = segment.bounds();
            let (lo, hi) = (b[plane.axis], b[plane.axis + 2]);
            if (plane.greater && lo >= plane.value) || (!plane.greater && hi <= plane.value) {
                kept.push(segment);
                continue;
            }
            if (plane.greater && hi < plane.value) || (!plane.greater && lo > plane.value) {
                continue;
            }
            let cuts = crossings(segment, plane, work)?;
            for pair in cuts.windows(2) {
                let (a, b) = (pair[0].0, pair[1].0);
                let mid = a + (b - a) * 0.5;
                if !(a < mid && mid < b) {
                    return Err(error(
                        "svg.precision.clip",
                        "SVG clipping interval cannot be resolved",
                    ));
                }
                let d = plane.distance(segment.point(mid));
                if !d.is_finite() {
                    return Err(error(
                        "svg.precision.clip",
                        "SVG clipping evaluation is not finite",
                    ));
                }
                let piece = segment.section(a, b, [pair[0].1, pair[1].1]);
                let inside = if d == 0.0 {
                    let bounds = piece.bounds();
                    let lo = bounds[plane.axis];
                    let hi = bounds[plane.axis + 2];
                    if (plane.greater && lo >= plane.value) || (!plane.greater && hi <= plane.value)
                    {
                        true
                    } else if (plane.greater && hi <= plane.value)
                        || (!plane.greater && lo >= plane.value)
                    {
                        false
                    } else {
                        return Err(error(
                            "svg.precision.clip",
                            "SVG clipping interval has unresolved boundary coverage",
                        ));
                    }
                } else {
                    d > 0.0
                };
                if inside {
                    work.edges(1)?;
                    kept.push(piece);
                }
            }
        }
        if kept.is_empty() {
            return Ok(kept);
        }
        let mut closed = Vec::new();
        closed
            .try_reserve(kept.len().saturating_mul(2))
            .map_err(|_| error("svg.allocation", "SVG boundary clipping allocation failed"))?;
        for (i, segment) in kept.iter().copied().enumerate() {
            work.check()?;
            closed.push(segment);
            let from = segment.ends()[1];
            let to = kept[(i + 1) % kept.len()].ends()[0];
            if from != to {
                if plane.distance(from) != 0.0 || plane.distance(to) != 0.0 {
                    return Err(error(
                        "svg.precision.clip",
                        "SVG clipping produced an unresolved contour gap",
                    ));
                }
                work.edges(1)?;
                closed.push(Segment::Line([from, to]));
            }
        }
        segments = closed;
    }
    // A tangent intersection contained entirely in one canvas edge has no fill.
    let mut extent = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for segment in &segments {
        work.check()?;
        let b = segment.bounds();
        extent[0] = extent[0].min(b[0]);
        extent[1] = extent[1].min(b[1]);
        extent[2] = extent[2].max(b[2]);
        extent[3] = extent[3].max(b[3]);
    }
    if (extent[0] == extent[2] && (extent[0] == bounds[0] || extent[0] == bounds[2]))
        || (extent[1] == extent[3] && (extent[1] == bounds[1] || extent[1] == bounds[3]))
    {
        segments.clear();
    }
    Ok(segments)
}

/// Finds all ordered boundary cuts without merging distinct narrow parameter intervals.
/// # Errors
/// Returns cancellation, work-limit, or unresolvable root diagnostics.
fn crossings(
    segment: Segment,
    plane: Plane,
    work: &mut SvgJoin<'_>,
) -> Result<Vec<(f64, Point2)>, RenderError> {
    let mut roots = Vec::new();
    match segment {
        Segment::Line(p) => {
            let (a, b) = (plane.distance(p[0]), plane.distance(p[1]));
            if a != b {
                let t = a / (a - b);
                if t > 0.0 && t < 1.0 {
                    roots.push(t);
                }
            }
        }
        Segment::Cubic(p) => {
            let mut values = p.map(|v| plane.distance(v));
            let scale = values.iter().map(|v| v.abs()).fold(0.0, f64::max);
            if !scale.is_finite() {
                return Err(error(
                    "svg.precision.clip",
                    "SVG cubic clipping coordinates are not finite",
                ));
            }
            if scale != 0.0 {
                for value in &mut values {
                    *value /= scale;
                }
                let d = [
                    values[1] - values[0],
                    values[2] - values[1],
                    values[3] - values[2],
                ];
                let mut intervals = vec![0.0, 1.0];
                work.edges(1)?;
                intervals.extend(
                    solve_quadratic(d[0], 2.0 * (d[1] - d[0]), d[0] - 2.0 * d[1] + d[2])
                        .into_iter()
                        .filter(|t| *t > 0.0 && *t < 1.0),
                );
                intervals.sort_by(f64::total_cmp);
                intervals.dedup();
                for &t in &intervals {
                    if t > 0.0 && t < 1.0 && scalar(values, t) == 0.0 {
                        roots.push(t);
                    }
                }
                for pair in intervals.windows(2) {
                    let (a, b) = (scalar(values, pair[0]), scalar(values, pair[1]));
                    if a != 0.0 && b != 0.0 && a.is_sign_negative() != b.is_sign_negative() {
                        roots.push(root(values, pair[0], pair[1], work)?);
                    }
                }
            }
        }
        Segment::Arc { radius, angles, .. } => {
            let q = plane.value / radius;
            if q.abs() <= 1.0 {
                let other = ((1.0 - q.abs()) * (1.0 + q.abs())).sqrt();
                for sign in [-1.0, 1.0] {
                    let (x, y) = if plane.axis == 0 {
                        (q, sign * other)
                    } else {
                        (sign * other, q)
                    };
                    let mut a = y.atan2(x);
                    if a < 0.0 {
                        a += std::f64::consts::TAU;
                    }
                    if a > angles[0] && a < angles[1] {
                        roots.push((a - angles[0]) / (angles[1] - angles[0]));
                    }
                }
            }
        }
    }
    roots.sort_by(f64::total_cmp);
    roots.dedup();
    let mut result = vec![(0.0, segment.ends()[0])];
    for t in roots {
        work.edges(1)?;
        result.push((t, plane.snap(segment.point(t))));
    }
    result.push((1.0, segment.ends()[1]));
    Ok(result)
}

/// Refines one sign-changing root until its normalized value or parameter bracket resolves.
/// # Errors
/// Returns cancellation, work-limit, or a bounded unresolved-root diagnostic.
fn root(
    values: [f64; 4],
    mut a: f64,
    mut b: f64,
    work: &mut SvgJoin<'_>,
) -> Result<f64, RenderError> {
    let mut va = scalar(values, a);
    for _ in 0..128 {
        work.edges(1)?;
        let mid = a + (b - a) * 0.5;
        if mid == a || mid == b {
            return Ok(if va.abs() <= scalar(values, b).abs() {
                a
            } else {
                b
            });
        }
        let value = scalar(values, mid);
        if value == 0.0 {
            return Ok(mid);
        }
        if value.is_sign_negative() == va.is_sign_negative() {
            a = mid;
            va = value;
        } else {
            b = mid;
        }
    }
    Err(error(
        "svg.precision.clip",
        "SVG cubic boundary root exceeds bounded refinement",
    ))
}

/// Evaluates a normalized scalar Bernstein cubic without power-basis cancellation.
fn scalar(p: [f64; 4], t: f64) -> f64 {
    let q = [
        p[0] * (1.0 - t) + p[1] * t,
        p[1] * (1.0 - t) + p[2] * t,
        p[2] * (1.0 - t) + p[3] * t,
    ];
    let a = q[0] * (1.0 - t) + q[1] * t;
    let b = q[1] * (1.0 - t) + q[2] * t;
    a * (1.0 - t) + b * t
}

/// Interpolates finite construction coordinates in their original frame.
fn mix(a: Point2, b: Point2, t: f64) -> Point2 {
    Point2::new(a.x * (1.0 - t) + b.x * t, a.y * (1.0 - t) + b.y * t)
}

/// Splits a cubic with De Casteljau while sharing the new endpoint exactly.
fn split(p: [Point2; 4], t: f64) -> ([Point2; 4], [Point2; 4]) {
    let a = mix(p[0], p[1], t);
    let b = mix(p[1], p[2], t);
    let c = mix(p[2], p[3], t);
    let d = mix(a, b, t);
    let e = mix(b, c, t);
    let f = mix(d, e, t);
    ([p[0], a, d, f], [f, e, c, p[3]])
}

/// Serializes a directed clipped contour, retaining true cubics and exact-radius circular arcs.
/// # Errors
/// Returns cancellation, allocation, or formatting diagnostics before publication.
fn append(
    data: &mut String,
    segments: &[Segment],
    reverse: bool,
    origin: Option<Point2>,
    work: &mut SvgJoin<'_>,
) -> Result<(), RenderError> {
    if segments.is_empty() {
        return Ok(());
    }
    data.try_reserve(segments.len().saturating_mul(160))
        .map_err(|_| error("svg.allocation", "SVG clipped path allocation failed"))?;
    let first = if reverse {
        segments.last().unwrap().ends()[1]
    } else {
        segments[0].ends()[0]
    };
    if let Some(center) = origin {
        write!(data, "M{} {}m{} {}", center.x, center.y, first.x, first.y)
    } else {
        write!(data, "M{} {}", first.x, first.y)
    }
    .map_err(|_| error("svg.format", "SVG clipped path formatting failed"))?;
    for i in 0..segments.len() {
        work.check()?;
        let segment = segments[if reverse { segments.len() - 1 - i } else { i }];
        let ends = segment.ends();
        let (start, end) = if reverse {
            (ends[1], ends[0])
        } else {
            (ends[0], ends[1])
        };
        match segment {
            Segment::Line(_) if origin.is_some() => {
                write!(data, "l{} {}", end.x - start.x, end.y - start.y)
            }
            Segment::Line(_) => write!(data, "L{} {}", end.x, end.y),
            Segment::Cubic(p) => {
                let (a, b) = if reverse { (p[2], p[1]) } else { (p[1], p[2]) };
                write!(data, "C{} {},{} {},{} {}", a.x, a.y, b.x, b.y, end.x, end.y)
            }
            Segment::Arc { radius, .. } => write!(
                data,
                "a{radius} {radius} 0 0 {} {} {}",
                u8::from(!reverse),
                end.x - start.x,
                end.y - start.y
            ),
        }
        .map_err(|_| error("svg.format", "SVG clipped segment formatting failed"))?;
    }
    data.push('Z');
    Ok(())
}

/// Creates a stable final-consumer diagnostic without exposing partial SVG data.
fn error(path: &'static str, message: &'static str) -> RenderError {
    RenderError::new(path, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{BezPath, Shape};
    use toniator_domain::CanvasSpec;

    /// Builds a closed directed polygon without changing its supplied orientation.
    fn polygon(points: &[[f64; 2]]) -> Vec<Segment> {
        points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                Segment::Line([
                    Point2::new(p[0], p[1]),
                    Point2::new(
                        points[(i + 1) % points.len()][0],
                        points[(i + 1) % points.len()][1],
                    ),
                ])
            })
            .collect()
    }

    /// Converts line/cubic test contours into Kurbo's independent analytic winding consumer.
    /// # Panics
    /// Panics only if an arc is incorrectly supplied to this line/cubic helper.
    fn path(segments: &[Segment]) -> BezPath {
        let mut path = BezPath::new();
        if let Some(first) = segments.first() {
            path.move_to((first.ends()[0].x, first.ends()[0].y));
        }
        for segment in segments {
            match segment {
                Segment::Line(p) => path.line_to((p[1].x, p[1].y)),
                Segment::Cubic(p) => {
                    path.curve_to((p[1].x, p[1].y), (p[2].x, p[2].y), (p[3].x, p[3].y))
                }
                Segment::Arc { .. } => panic!("analytic line/cubic witness only"),
            }
        }
        path.close_path();
        path
    }

    /// Checks winding conservation, disconnected interiors, enclosing fills, and true cubic crossings.
    ///
    /// Kurbo independently evaluates winding at interior/exterior grid points;
    /// boundary samples are intentionally excluded from this fill assertion.
    /// # Panics
    /// Panics on clipping failure, changed winding, or emitted off-canvas geometry.
    #[test]
    fn closed_rectangle_intersection_preserves_directed_winding() {
        let mut cubic = polygon(&[[-2.0, 1.0], [10.0, 6.0], [10.0, 7.0], [-2.0, 7.0]]);
        cubic[0] = Segment::Cubic([
            Point2::new(-2.0, 1.0),
            Point2::new(1.0, 2.0),
            Point2::new(4.0, 5.0),
            Point2::new(10.0, 6.0),
        ]);
        let mut crossing = polygon(&[[-1.0, 0.1], [9.0, -0.1], [9.0, 7.0], [-1.0, 7.0]]);
        crossing[0] = Segment::Cubic([
            Point2::new(-1.0, 0.1),
            Point2::new(2.0, -2.0),
            Point2::new(6.0, 2.0),
            Point2::new(9.0, -0.1),
        ]);
        for source in [
            polygon(&[[-2.0, -2.0], [10.0, -2.0], [10.0, 10.0], [-2.0, 10.0]]),
            polygon(&[
                [1.0, 5.0],
                [3.0, 5.0],
                [3.0, -2.0],
                [5.0, -2.0],
                [5.0, 5.0],
                [7.0, 5.0],
                [7.0, -3.0],
                [1.0, -3.0],
            ]),
            cubic,
            crossing,
        ] {
            let canvas = CanvasSpec {
                width: 8.0,
                height: 8.0,
            };
            let mut work =
                SvgJoin::new(&canvas, usize::MAX, &|| false).expect("valid clipping work");
            let clipped = clip(source.clone(), [0.0, 0.0, 8.0, 8.0], &mut work)
                .expect("closed rectangle clip");
            let original = path(&source);
            let result = path(&clipped);
            for x in -2..20 {
                for y in -2..20 {
                    let p = kurbo::Point::new(x as f64 * 0.5 + 0.123, y as f64 * 0.5 + 0.217);
                    let expected = if p.x > 0.0 && p.x < 8.0 && p.y > 0.0 && p.y < 8.0 {
                        original.winding(p)
                    } else {
                        0
                    };
                    assert_eq!(result.winding(p), expected, "winding at {p:?}");
                }
            }
            for s in clipped {
                for i in 0..=32 {
                    let p = s.point(i as f64 / 32.0);
                    assert!(
                        (0.0..=8.0).contains(&p.x) && (0.0..=8.0).contains(&p.y),
                        "clipped point {p:?}"
                    );
                }
            }
        }
    }

    /// Proves boundary root work honors explicit limits and cancellation without partial data.
    /// # Panics
    /// Panics if a bounded/cancelled clip returns SVG bytes or the wrong diagnostic.
    #[test]
    fn boundary_root_work_is_bounded_and_cancelled() {
        let canvas = CanvasSpec {
            width: 8.0,
            height: 8.0,
        };
        let mut work = SvgJoin::new(&canvas, 4, &|| false).expect("valid bound");
        let mut data = String::new();
        let error = circle(
            &mut data,
            Point2::new(0.0, 4.0),
            2.0,
            [0.0, 0.0, 8.0, 8.0],
            &mut work,
        )
        .expect_err("circle clipping charges additional work");
        assert_eq!(error.path(), "svg.limits.edges");
        assert!(data.is_empty());
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let cancelled = || calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed) > 6;
        let mut work = SvgJoin::new(&canvas, usize::MAX, &cancelled).expect("initial work");
        let error = circle(
            &mut data,
            Point2::new(0.0, 4.0),
            2.0,
            [0.0, 0.0, 8.0, 8.0],
            &mut work,
        )
        .expect_err("cancel during clip");
        assert_eq!(error.path(), "svg.cancelled");
        assert!(data.is_empty());
    }
}
