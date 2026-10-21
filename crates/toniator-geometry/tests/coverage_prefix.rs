//! Coverage extends one canonical path without changing its authored finite prefix.
use std::sync::atomic::{AtomicUsize, Ordering};
use toniator_domain::{CurveWinding, ParametricCurve, SpiralCurve, SpiralShape};
use toniator_geometry::*;

/// Supplies a bounded analytic source with independent phase, winding and fractional extent.
fn curve(shape: SpiralShape, turns: f64, phase: f64, winding: CurveWinding) -> ParametricCurve {
    ParametricCurve::Spiral(SpiralCurve {
        shape,
        turns,
        radial_spacing: 7.0,
        phase_degrees: phase,
        winding,
    })
}

/// Preserves every prefix segment for both shapes and keeps default finite output exactly unchanged.
/// # Panics
/// Panics if extension repartitions authored geometry, changes a genuine endpoint or introduces closure.
#[test]
fn whole_and_fractional_extent_preserve_exact_canonical_prefix() {
    for shape in [SpiralShape::Round, SpiralShape::Square] {
        for turns in [1.0, 1.375, 2.1875] {
            for phase in [0.0, 37.0] {
                for winding in [CurveWinding::Clockwise, CurveWinding::CounterClockwise] {
                    let curve = curve(shape, turns, phase, winding);
                    let origin = Point2::new(13.0, -29.0);
                    let fixed =
                        construct_parametric_curve_path_cancellable(&curve, origin, &|| false)
                            .unwrap();
                    let same = extend_parametric_curve_path_with_limits_cancellable(
                        &curve,
                        turns,
                        origin,
                        CurveWorkLimits::default(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(fixed, same);
                    let extended = extend_parametric_curve_path_with_limits_cancellable(
                        &curve,
                        6.0,
                        origin,
                        CurveWorkLimits::default(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(
                        &extended.segments()[..fixed.segments().len()],
                        fixed.segments(),
                        "{shape:?} {turns} {phase} {winding:?}"
                    );
                    assert_eq!(extended.closure(), PathClosure::Open);
                    assert_eq!(extended.start(), fixed.start());
                    assert_eq!(
                        extended.segments()[fixed.segments().len()].start(),
                        fixed.end()
                    );
                    if shape == SpiralShape::Square && turns * 4.0 != (turns * 4.0).floor() {
                        let before = fixed
                            .segments()
                            .last()
                            .unwrap()
                            .unit_tangent_at(1.0)
                            .unwrap();
                        let after = extended.segments()[fixed.segments().len()]
                            .unit_tangent_at(0.0)
                            .unwrap();
                        assert!((before.x * after.y - before.y * after.x).abs() < 1e-12);
                        assert!(before.x * after.x + before.y * after.y > 0.999999999999);
                    }
                }
            }
        }
    }
}

/// Rejects nonfinite extent, insufficient material policy and cancellation without a partial path.
/// # Panics
/// Panics if a resource or cancellation bound publishes geometry or a finite Fixed replay differs.
#[test]
fn extension_keeps_finite_resource_and_cancellation_authority() {
    for shape in [SpiralShape::Round, SpiralShape::Square] {
        let curve = curve(shape, 1.375, 17.0, CurveWinding::Clockwise);
        let origin = Point2::new(0.0, 0.0);
        let complete = extend_parametric_curve_path_with_limits_cancellable(
            &curve,
            6.0,
            origin,
            CurveWorkLimits::default(),
            &|| false,
        )
        .unwrap();
        let limits = CurveWorkLimits::new(
            complete.segments().len() - 1,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
        assert!(
            extend_parametric_curve_path_with_limits_cancellable(
                &curve,
                6.0,
                origin,
                limits,
                &|| false
            )
            .is_err()
        );
        assert!(
            extend_parametric_curve_path_with_limits_cancellable(
                &curve,
                f64::INFINITY,
                origin,
                CurveWorkLimits::default(),
                &|| false
            )
            .is_err()
        );
        let polls = AtomicUsize::new(0);
        let error = extend_parametric_curve_path_with_limits_cancellable(
            &curve,
            6.0,
            origin,
            CurveWorkLimits::default(),
            &|| polls.fetch_add(1, Ordering::Relaxed) > 4,
        )
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
    }
}
