//! Bounded curved constant-width approximations of certified circular cubic carriers.
use super::*;

/// Builds curved rails from the actual radial cubic carrier without replacing it by an ideal circle.
/// Every accepted cubic has one angular winding and a certified radial/normal error below0.006.
/// Homothetic rails and radial end edges prove a nonnegative winding ribbon; adding positive
/// half-caps and cancelling equal/opposite diameter edges yields one editable open boundary,
/// even when a near-full-turn gap lets the caps overlap.
/// Radius collapse, noncircular/distorted paths or insufficient accuracy use the ordinary fallback.
/// # Errors
/// Returns validation, cancellation, allocation or segment-budget failures before publication.
#[allow(clippy::too_many_arguments)]
pub fn build_circular_stroke_outline_cancellable(
    path: &CurvePath,
    center: Point2,
    radius: f64,
    winding: f64,
    width: f64,
    bias: f64,
    tolerance: f64,
    limits: VariableWidthOutlineLimits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<CanonicalFilledOutline>, CurveError> {
    check_curve_cancelled(cancelled)?;
    if !width.is_finite()
        || width <= 0.0
        || !bias.is_finite()
        || bias.abs() > 1.0
        || !tolerance.is_finite()
        || tolerance <= 0.0
    {
        return Err(CurveError::new(
            "curve.outline.width",
            "circular stroke width, bias and tolerance must be valid",
        ));
    }
    if !center.is_finite()
        || !radius.is_finite()
        || radius <= 0.0
        || !matches!(winding, -1.0 | 1.0)
        || path.segments().len() > 5
    {
        return Ok(None);
    }
    let half = width * 0.5;
    let outer_distance = half * (1.0 + winding * bias);
    let inner_distance = -half * (1.0 - winding * bias);
    let outer_scale = 1.0 + outer_distance / radius;
    let inner_scale = 1.0 + inner_distance / radius;
    if !outer_scale.is_finite() || inner_scale <= 0.0 || !inner_scale.is_finite() {
        return Ok(None);
    }
    let extent = path
        .segments()
        .iter()
        .flat_map(outline_segment_points)
        .map(|p| p.x.abs().max(p.y.abs()))
        .fold(center.x.abs().max(center.y.abs()), f64::max);
    let roundoff = 128.0 * f64::EPSILON * extent;
    let maximum_offset = outer_distance.max(-inner_distance);
    if radius * inner_scale <= 16.0 * roundoff {
        return Ok(None);
    }
    // Includes both rail error and endpoint half-cap center/radius/orientation error.
    if 0.012 * maximum_offset + roundoff > tolerance.min(0.02 * half) {
        return Ok(None);
    }
    for segment in path.segments() {
        let CurveSegment::CubicBezier(cubic) = segment else {
            return Ok(None);
        };
        if !radial_certificate(*cubic, center, radius, winding, 0, cancelled)? {
            return Ok(None);
        }
    }
    let mut remaining = limits.maximum_segments();
    let mut outer = Vec::new();
    let mut inner = Vec::new();
    for segment in path.segments() {
        check_curve_cancelled(cancelled)?;
        let CurveSegment::CubicBezier(c) = segment else {
            unreachable!("certified cubics")
        };
        for (destination, scale) in [(&mut outer, outer_scale), (&mut inner, inner_scale)] {
            let map = |p: Point2| {
                Point2::new(
                    center.x + (p.x - center.x) * scale,
                    center.y + (p.y - center.y) * scale,
                )
            };
            push_cubic(
                destination,
                map(c.start()),
                map(c.control_1()),
                map(c.control_2()),
                map(c.end()),
                &mut remaining,
            )?;
        }
    }
    if winding < 0.0 {
        reverse_contour(&mut outer)?;
        reverse_contour(&mut inner)?;
    }
    let mut contours = Vec::new();
    if path.closure() == PathClosure::Closed {
        reverse_contour(&mut inner)?;
        reserve_curve(&mut contours, 2)?;
        contours.push(CanonicalOutlineContour { segments: outer });
        contours.push(CanonicalOutlineContour { segments: inner });
    } else {
        let outer_start = outer[0].start();
        let outer_end = outer.last().unwrap().end();
        let inner_start = inner[0].start();
        let inner_end = inner.last().unwrap().end();
        let mut end_cap = Vec::new();
        let mut start_cap = Vec::new();
        for (cap, a, b, forward) in [
            (&mut end_cap, outer_end, inner_end, true),
            (&mut start_cap, inner_start, outer_start, false),
        ] {
            let p = if forward { outer_end } else { outer_start };
            let radial = Vector2::new(p.x - center.x, p.y - center.y);
            let length = radial.x.hypot(radial.y);
            let tangent = Vector2::new(-radial.y / length, radial.x / length);
            push_round_cap(cap, a, b, tangent, forward, &mut remaining)?;
        }
        // The positive body and two positive half-caps have equal/opposite diameter edges.
        // Cancelling those exact directed edges preserves their total NonZero winding, including
        // overlapping caps, and leaves one natural editable boundary without artificial subpaths.
        let mut body = outer;
        reserve_curve(&mut body, end_cap.len() + inner.len() + start_cap.len())?;
        body.extend(end_cap);
        reverse_contour(&mut inner)?;
        body.extend(inner);
        body.extend(start_cap);
        reserve_curve(&mut contours, 1)?;
        contours.push(CanonicalOutlineContour { segments: body });
    }
    certified_curved_outline(contours, limits, cancelled).map(Some)
}

/// Reverses exact line/cubic contour traversal without changing any stored point.
/// # Errors
/// Returns finite segment validation failures without inventing alternate geometry.
fn reverse_contour(segments: &mut [CurveSegment]) -> Result<(), CurveError> {
    segments.reverse();
    for segment in segments {
        *segment = match *segment {
            CurveSegment::Line(c) => CurveSegment::Line(LineSegment::new(c.end(), c.start())?),
            CurveSegment::CubicBezier(c) => CurveSegment::CubicBezier(CubicBezierSegment::new(
                c.end(),
                c.control_2(),
                c.control_1(),
                c.start(),
            )?),
        };
    }
    Ok(())
}

/// Bounds actual normalized radial and normal deviation using Bernstein polynomial convex hulls.
/// At most five subdivision levels inspect a fixed cubic; subdivision changes proof only, never rails.
/// Positive angular derivative certifies ribbon winding even for cap overlap and repeated angles.
/// # Errors
/// Returns cancellation from every bounded proof node.
fn radial_certificate(
    c: CubicBezierSegment,
    center: Point2,
    radius: f64,
    winding: f64,
    depth: u8,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, CurveError> {
    check_curve_cancelled(cancelled)?;
    let controls = [c.start(), c.control_1(), c.control_2(), c.end()];
    let magnitude = controls
        .iter()
        .map(|p| p.x.abs().max(p.y.abs()))
        .fold(center.x.abs().max(center.y.abs()), f64::max);
    let uncertainty = 64.0 * f64::EPSILON * (magnitude / radius + 1.0);
    if !uncertainty.is_finite() || uncertainty > 1e-8 {
        return Ok(false);
    }
    let x = controls.map(|p| (p.x - center.x) / radius);
    let y = controls.map(|p| (p.y - center.y) / radius);
    let dx = std::array::from_fn::<_, 3, _>(|i| 3.0 * (x[i + 1] - x[i]));
    let dy = std::array::from_fn::<_, 3, _>(|i| 3.0 * (y[i + 1] - y[i]));
    let (rlo, rhi) = product_bounds(&x, &x, &y, &y, false, uncertainty);
    let (slo, _) = product_bounds(&dx, &dx, &dy, &dy, false, uncertainty);
    let (dlo, dhi) = product_bounds(&x, &dx, &y, &dy, false, uncertainty);
    let (clo, chi) = product_bounds(&x, &dy, &y, &dx, true, uncertainty);
    let directed = if winding > 0.0 { clo > 0.0 } else { chi < 0.0 };
    if directed && rlo > 0.0 && slo > 0.0 && rhi.is_finite() {
        let radial = (rhi.sqrt().next_up() - 1.0)
            .abs()
            .max((rlo.sqrt().next_down() - 1.0).abs())
            .next_up();
        let denominator = (rlo * slo).next_down().sqrt().next_down();
        let sine = (dlo.abs().max(dhi.abs()).next_up() / denominator).next_up();
        if (radial + 2.0 * sine).next_up() < 0.006 {
            return Ok(true);
        }
    }
    if depth == 5 {
        return Ok(false);
    }
    let (a, b) = c.split(0.5)?;
    Ok(
        radial_certificate(a, center, radius, winding, depth + 1, cancelled)?
            && radial_certificate(b, center, radius, winding, depth + 1, cancelled)?,
    )
}

/// Encloses the coefficients of two summed/subtracted Bernstein products with outward roundoff padding.
/// Degree is bounded by six and all normalized input arithmetic contributes to the conservative pad.
fn product_bounds(
    a: &[f64],
    b: &[f64],
    c: &[f64],
    d: &[f64],
    subtract: bool,
    uncertainty: f64,
) -> (f64, f64) {
    const CHOOSE: [[f64; 7]; 7] = [
        [1., 0., 0., 0., 0., 0., 0.],
        [1., 1., 0., 0., 0., 0., 0.],
        [1., 2., 1., 0., 0., 0., 0.],
        [1., 3., 3., 1., 0., 0., 0.],
        [1., 4., 6., 4., 1., 0., 0.],
        [1., 5., 10., 10., 5., 1., 0.],
        [1., 6., 15., 20., 15., 6., 1.],
    ];
    let n = a.len() - 1;
    let m = b.len() - 1;
    let mut lower = f64::INFINITY;
    let mut upper = f64::NEG_INFINITY;
    for (k, denominator) in CHOOSE[n + m].iter().enumerate().take(n + m + 1) {
        let mut value = 0.0;
        let mut absolute = 0.0;
        for i in 0..=n {
            if k < i || k - i > m {
                continue;
            }
            let j = k - i;
            let weight = CHOOSE[n][i] * CHOOSE[m][j] / *denominator;
            let first = weight * a[i] * b[j];
            let second = weight * c[i] * d[j];
            value += first + if subtract { -second } else { second };
            absolute += first.abs() + second.abs();
        }
        let padding = 512.0 * f64::EPSILON * (absolute + 1.0) + 256.0 * uncertainty;
        lower = lower.min((value - padding).next_down());
        upper = upper.max((value + padding).next_up());
    }
    (lower, upper)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PathLocation, resolve_guide_prototype};
    use toniator_domain::{AuthoredPoint2, GuidePrototype};

    /// Constructs the same canonical quarter-cubic carrier used by structural circle guides.
    /// # Panics
    /// Panics if the current finite circle constructor or explicit closed seam fails.
    fn carrier(closed: bool, near_full: bool, winding: f64) -> CurvePath {
        let path = resolve_guide_prototype(
            &GuidePrototype::CircularArc {
                center: AuthoredPoint2 { x: 100., y: 200. },
                radius: 64.,
                start_angle_degrees: 13.,
                sweep_angle_degrees: winding * 360.,
            },
            None,
        )
        .unwrap();
        let mut segments = path.segments().to_vec();
        let CurveSegment::CubicBezier(last) = *segments.last().unwrap() else {
            unreachable!()
        };
        if near_full {
            *segments.last_mut().unwrap() =
                CurveSegment::CubicBezier(last.split(0.9999).unwrap().0);
        }
        if closed {
            *segments.last_mut().unwrap() = CurveSegment::CubicBezier(
                CubicBezierSegment::new(
                    last.start(),
                    last.control_1(),
                    last.control_2(),
                    segments[0].start(),
                )
                .unwrap(),
            );
        }
        CurvePath::new(
            segments,
            if closed {
                PathClosure::Closed
            } else {
                PathClosure::Open
            },
        )
        .unwrap()
    }

    /// Samples an independent ray winding oracle for filled-contour and cap-overlap checks.
    /// # Panics
    /// Panics if finite canonical outline evaluation fails.
    fn winding_at(outline: &CanonicalFilledOutline, point: Point2) -> i32 {
        let mut winding = 0;
        for contour in &outline.contours {
            for segment in &contour.segments {
                let mut a = segment.start();
                for i in 1..=32 {
                    let b = segment.point_at(i as f64 / 32.).unwrap();
                    if (a.y <= point.y && point.y < b.y) || (b.y <= point.y && point.y < a.y) {
                        let x = a.x + (point.y - a.y) * (b.x - a.x) / (b.y - a.y);
                        if x > point.x {
                            winding += if b.y > a.y { 1 } else { -1 };
                        }
                    }
                    a = b;
                }
            }
        }
        winding
    }

    /// Checks true curved rails, opposite-winding holes, cap overlap and exact private certificate binding.
    /// # Panics
    /// Panics if angular winding becomes negative, holes disappear or edited contours retain proof.
    #[test]
    fn circular_curved_outline_keeps_holes_caps_and_certificate() {
        for direction in [-1., 1.] {
            for closed in [false, true] {
                let path = carrier(closed, !closed, direction);
                let outline = build_circular_stroke_outline_cancellable(
                    &path,
                    Point2::new(100., 200.),
                    64.,
                    direction,
                    2.,
                    0.,
                    0.04,
                    VariableWidthOutlineLimits::new(64).unwrap(),
                    &|| false,
                )
                .unwrap()
                .unwrap();
                assert!(outline.has_nonnegative_winding_certificate());
                assert!(outline.logical_segment_count().unwrap() <= 24);
                assert_eq!(outline.contours.len(), if closed { 2 } else { 1 });
                assert_eq!(winding_at(&outline, Point2::new(100., 200.)), 0);
                for x in (30..171).step_by(4) {
                    for y in (130..271).step_by(4) {
                        assert!(
                            winding_at(&outline, Point2::new(x as f64 + 0.123, y as f64 + 0.321))
                                >= 0
                        );
                    }
                }
                let complete = materialize_prepared_outline_cancellable(
                    outline.prepared.as_ref().unwrap().clone(),
                    MaterializationCoverage::Complete,
                    VariableWidthOutlineLimits::new(64).unwrap(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(complete.contours, outline.contours);
                assert!(complete.has_nonnegative_winding_certificate());
                let mut changed = outline.clone();
                changed.contours[0].segments.pop();
                assert!(!changed.has_nonnegative_winding_certificate());
            }
        }
    }

    /// Measures radial rail error against the actual cubic normal, independently of the nominal circle.
    /// # Panics
    /// Panics if biased widths exceed the existing absolute or half-width-relative outline bounds.
    #[test]
    fn circular_curved_rails_meet_actual_normal_offset_tolerance() {
        let path = carrier(true, false, 1.);
        for bias in [-0.5, 0., 0.5] {
            let outline = build_circular_stroke_outline_cancellable(
                &path,
                Point2::new(100., 200.),
                64.,
                1.,
                2.,
                bias,
                0.04,
                VariableWidthOutlineLimits::new(64).unwrap(),
                &|| false,
            )
            .unwrap()
            .unwrap();
            for (i, segment) in path.segments().iter().enumerate() {
                for t in [0., 0.125, 0.25, 0.5, 0.75, 0.875, 1.] {
                    let p = segment.point_at(t).unwrap();
                    let normal = path
                        .unit_normal_at(PathLocation::new(i, t).unwrap())
                        .unwrap();
                    let actual = outline.contours[0].segments[i].point_at(t).unwrap();
                    let expected =
                        Point2::new(p.x - normal.x * (1. + bias), p.y - normal.y * (1. + bias));
                    assert!((actual.x - expected.x).hypot(actual.y - expected.y) < 0.02);
                }
            }
        }
    }

    /// Retains a genuine source-clipped angular gap while preserving the selected curved band.
    /// # Panics
    /// Panics if positive compound certification fills a real gap beyond the round caps.
    #[test]
    fn circular_curved_open_gap_remains_empty() {
        let original = carrier(false, false, 1.);
        let CurveSegment::CubicBezier(last) = *original.segments().last().unwrap() else {
            unreachable!()
        };
        let mut segments = original.segments().to_vec();
        *segments.last_mut().unwrap() = CurveSegment::CubicBezier(last.split(0.8).unwrap().0);
        let path = CurvePath::new(segments, PathClosure::Open).unwrap();
        let outline = build_circular_stroke_outline_cancellable(
            &path,
            Point2::new(100., 200.),
            64.,
            1.,
            2.,
            0.,
            0.04,
            VariableWidthOutlineLimits::new(64).unwrap(),
            &|| false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(outline.contours.len(), 1);
        assert_eq!(
            winding_at(
                &outline,
                CurveSegment::CubicBezier(last).point_at(0.9).unwrap()
            ),
            0
        );
        assert!(
            winding_at(
                &outline,
                CurveSegment::CubicBezier(last).point_at(0.5).unwrap()
            ) > 0
        );
    }

    /// Retains ordinary fallback for collapse, distortion and numerical uncertainty, with bounded errors.
    /// # Panics
    /// Panics if unsupported geometry acquires proof or cancellation/segment ceilings are weakened.
    #[test]
    fn circular_curved_outline_fallback_and_limits_remain_truthful() {
        let path = carrier(true, false, 1.);
        let limits = VariableWidthOutlineLimits::new(64).unwrap();
        assert!(
            build_circular_stroke_outline_cancellable(
                &path,
                Point2::new(100., 200.),
                0.5,
                1.,
                2.,
                0.,
                0.04,
                limits,
                &|| false
            )
            .unwrap()
            .is_none()
        );
        let wrong = path
            .transformed(
                crate::AffineTransform2D::scale_then_rotate_about_then_translate(
                    Point2::new(100., 200.),
                    1.1,
                    1.,
                    0.,
                    Vector2::new(0., 0.),
                )
                .unwrap(),
            )
            .unwrap();
        assert!(
            build_circular_stroke_outline_cancellable(
                &wrong,
                Point2::new(100., 200.),
                64.,
                1.,
                2.,
                0.,
                0.04,
                limits,
                &|| false
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(
            build_circular_stroke_outline_cancellable(
                &path,
                Point2::new(100., 200.),
                64.,
                1.,
                2.,
                0.,
                0.04,
                limits,
                &|| true
            )
            .unwrap_err()
            .path(),
            "evaluation.cancelled"
        );
        assert_eq!(
            build_circular_stroke_outline_cancellable(
                &path,
                Point2::new(100., 200.),
                64.,
                1.,
                2.,
                0.,
                0.04,
                VariableWidthOutlineLimits::new(1).unwrap(),
                &|| false
            )
            .unwrap_err()
            .path(),
            "curve.outline.segment_limit"
        );
    }
}
