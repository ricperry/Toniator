use std::cell::Cell;
use toniator_geometry::{
    ArcLengthWorkLimits, CurvePath, CurveSegment, CubicBezierSegment, LineSegment, PathClosure,
    PathLocation, Point2,
};

/// Exercises reached arc preparation beyond the old aggregate leaf admission with unchanged numerics.
/// # Panics
/// Panics if a machine-addressable long path hits a hidden aggregate cap or explicit policy is ignored.
#[test]
fn artistic_arc_admission_is_explicit_and_machine_addressable() {
    let path = CurvePath::new(
        (0..70_000)
            .map(|index| {
                CurveSegment::Line(
                    LineSegment::new(
                        Point2::new(f64::from(index), 0.0),
                        Point2::new(f64::from(index) + 1.0, 0.0),
                    )
                    .unwrap(),
                )
            })
            .collect(),
        PathClosure::Open,
    )
    .unwrap();
    let measure = path
        .measure_arc_length_cancellable(ArcLengthWorkLimits::default(), &|| false)
        .unwrap();
    assert_eq!(measure.total_length(), 70_000.0);
    assert_eq!(
        measure
            .location_at_length_cancellable(69_999.5, &|| false)
            .unwrap(),
        PathLocation::new(69_999, 0.5).unwrap()
    );
    assert!(
        path.measure_arc_length_cancellable(
            ArcLengthWorkLimits {
                maximum_work_items: 100,
                maximum_leaves: 100
            },
            &|| false
        )
        .is_err()
    );
    assert!(path.measure_arc_length().is_err());
}

/// Cancels both forward quadrature and inverse bisection rather than publishing partial metrics.
/// # Panics
/// Panics if reached inverse/forward numerical refinement fails to poll cancellation or recovery.
#[test]
fn reached_forward_and_inverse_arc_refinement_are_cancellable() {
    let path = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(0.0, 0.0),
                Point2::new(0.0, 30.0),
                Point2::new(20.0, -5.0),
                Point2::new(30.0, 20.0),
            )
            .unwrap(),
        )],
        PathClosure::Open,
    )
    .unwrap();
    let metric = path
        .measure_arc_length_cancellable(ArcLengthWorkLimits::default(), &|| false)
        .unwrap();
    let calls = Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() > 3
    };
    assert_eq!(
        metric
            .location_at_length_cancellable(metric.total_length() * 0.413, &cancel)
            .unwrap_err()
            .path(),
        "evaluation.cancelled"
    );
    calls.set(0);
    assert_eq!(
        metric
            .length_at_location_cancellable(PathLocation::new(0, 0.413).unwrap(), &cancel)
            .unwrap_err()
            .path(),
        "evaluation.cancelled"
    );
    let location = metric
        .location_at_length_cancellable(metric.total_length() * 0.413, &|| false)
        .unwrap();
    assert!(
        (metric
            .length_at_location_cancellable(location, &|| false)
            .unwrap()
            - metric.total_length() * 0.413)
            .abs()
            < 1e-6
    );
}
