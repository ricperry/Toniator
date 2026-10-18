//! Independent analytic witnesses for the reached canonical construction/clipping route.
use std::cell::Cell;
use toniator_domain::{CurveWinding, ParametricCurve, SpiralCurve, SpiralShape};
use toniator_geometry::{
    AffineTransform2D, Bounds, CubicBezierSegment, CurvePath, CurveSegment, CurveWorkLimits,
    PathClosure, Point2, Vector2, construct_parametric_curve_path_cancellable,
    construct_parametric_curve_path_with_limits_cancellable,
};

/// Builds finite partial-turn intent without document, canvas or render authority.
fn spiral(shape: SpiralShape, turns: f64, winding: CurveWinding) -> SpiralCurve {
    SpiralCurve {
        shape,
        turns,
        radial_spacing: 16.0,
        phase_degrees: 37.0,
        winding,
    }
}

/// Evaluates the independent polar spiral equation at a nonnegative angular parameter.
/// This witness does not reproduce the producer's subdivision/count algorithm.
fn analytic_round(curve: &SpiralCurve, origin: Point2, theta: f64) -> Point2 {
    let sign = if curve.winding == CurveWinding::Clockwise {
        -1.0
    } else {
        1.0
    };
    let angle = curve.phase_degrees.to_radians() + sign * theta;
    let radius = curve.radial_spacing / std::f64::consts::TAU * theta;
    Point2::new(
        origin.x + radius * angle.cos(),
        origin.y + radius * angle.sin(),
    )
}

/// Computes a square-spiral vertex from its quarter index in an independent phase basis.
/// The closed-form four-corner blocks avoid reproducing the producer's accumulating loop.
fn analytic_square_corner(curve: &SpiralCurve, origin: Point2, quarter: usize) -> Point2 {
    let block = (quarter / 4) as f64;
    let (x, y) = match quarter % 4 {
        0 => (-block, -block),
        1 => (block + 1.0, -block),
        2 => (block + 1.0, block + 1.0),
        _ => (-block - 1.0, block + 1.0),
    };
    let sign = if curve.winding == CurveWinding::Clockwise {
        -1.0
    } else {
        1.0
    };
    let (sin, cos) = curve.phase_degrees.to_radians().sin_cos();
    Point2::new(
        origin.x + curve.radial_spacing * (x * cos - sign * y * sin),
        origin.y + curve.radial_spacing * (x * sin + sign * y * cos),
    )
}

/// Checks independently computed square corners, partial endpoint and exact published joins.
///
/// # Panics
/// Panics if the canonical exact-polyline count, corner order or phase/winding geometry changes.
fn check_square(curve: &SpiralCurve) {
    let origin = Point2::new(512.0, 310.0);
    let path = construct_parametric_curve_path_cancellable(
        &ParametricCurve::Spiral(curve.clone()),
        origin,
        &|| false,
    )
    .expect("finite square path");
    let quarters = curve.turns * 4.0;
    assert_eq!(path.segments().len(), quarters.ceil() as usize);
    let mut previous = origin;
    for (index, segment) in path.segments().iter().enumerate() {
        assert!(matches!(segment, CurveSegment::Line(_)));
        assert_eq!(segment.start(), previous);
        let start = analytic_square_corner(curve, origin, index);
        let end = analytic_square_corner(curve, origin, index + 1);
        let fraction = (quarters - index as f64).min(1.0);
        let expected = Point2::new(
            start.x + (end.x - start.x) * fraction,
            start.y + (end.y - start.y) * fraction,
        );
        assert!((expected.x - segment.end().x).hypot(expected.y - segment.end().y) < 1.0e-8);
        previous = segment.end();
    }
}

/// Locks independent square corner/partial-turn witnesses for both winding directions.
///
/// # Panics
/// Panics on changed line order, exact joins, phase basis or independent endpoint coordinates.
#[test]
fn valid_square_partial_turns_preserve_analytic_witnesses() {
    for winding in [CurveWinding::Clockwise, CurveWinding::CounterClockwise] {
        check_square(&spiral(SpiralShape::Square, 5.375, winding));
    }
}

/// Checks exact joins/order, analytic endpoints and the accepted three residual witnesses.
/// Dense samples are diagnostic witnesses only, never a stronger quality certification.
///
/// # Panics
/// Panics if cubic order/joins, analytic endpoints or the accepted 1/64-unit residual changes.
fn check_round(curve: &SpiralCurve) {
    let origin = Point2::new(512.0, 310.0);
    let path = construct_parametric_curve_path_cancellable(
        &ParametricCurve::Spiral(curve.clone()),
        origin,
        &|| false,
    )
    .expect("finite round path");
    let rate = curve.radial_spacing / std::f64::consts::TAU;
    let mut previous = origin;
    let mut previous_theta = 0.0;
    let mut dense_max = 0.0_f64;
    for segment in path.segments() {
        assert!(matches!(segment, CurveSegment::CubicBezier(_)));
        assert_eq!(segment.start(), previous);
        let end_theta = (segment.end().x - origin.x).hypot(segment.end().y - origin.y) / rate;
        assert!(end_theta > previous_theta);
        for fraction in [0.25, 0.5, 0.75] {
            let expected = analytic_round(
                curve,
                origin,
                previous_theta + (end_theta - previous_theta) * fraction,
            );
            let point = segment.point_at(fraction).expect("finite cubic point");
            assert!((point.x - expected.x).hypot(point.y - expected.y) <= 1.0 / 64.0 + 1.0e-8);
        }
        for index in 1..64 {
            let fraction = f64::from(index) / 64.0;
            let expected = analytic_round(
                curve,
                origin,
                previous_theta + (end_theta - previous_theta) * fraction,
            );
            let point = segment.point_at(fraction).expect("dense finite sample");
            dense_max = dense_max.max((point.x - expected.x).hypot(point.y - expected.y));
        }
        previous = segment.end();
        previous_theta = end_theta;
    }
    assert_eq!(path.start(), origin);
    assert_eq!(
        path.end(),
        analytic_round(curve, origin, std::f64::consts::TAU * curve.turns)
    );
    eprintln!(
        "round turns={} segments={} diagnostic_dense_max={dense_max}",
        curve.turns,
        path.segments().len()
    );
}

/// Locks valid partial-turn round endpoints and residuals for both windings before capacity edits.
///
/// # Panics
/// Panics if canonical geometry violates the analytic witness or exact continuity/order.
#[test]
fn valid_round_partial_turns_preserve_analytic_witnesses() {
    for winding in [CurveWinding::Clockwise, CurveWinding::CounterClockwise] {
        check_round(&spiral(SpiralShape::Round, 5.375, winding));
    }
}

/// Verifies complete >4096 round/square intent against independent analytic witnesses.
/// Partial turns, both windings and phase remain unchanged; repeats preserve exact order.
/// # Panics
/// Panics if creative capacity, analytic residuals/endpoints or deterministic output changes.
#[test]
fn large_round_and_square_paths_preserve_analytic_quality_and_order() {
    for winding in [CurveWinding::Clockwise, CurveWinding::CounterClockwise] {
        for turns in [223.25, 1024.375] {
            check_round(&spiral(SpiralShape::Round, turns, winding));
        }
        check_square(&spiral(SpiralShape::Square, 1024.375, winding));
        for shape in [SpiralShape::Round, SpiralShape::Square] {
            let curve = ParametricCurve::Spiral(spiral(shape, 1024.375, winding));
            let first =
                construct_parametric_curve_path_cancellable(&curve, Point2::new(0.0, 0.0), &|| {
                    false
                })
                .expect("large path");
            assert!(first.segments().len() > 4096);
            assert_eq!(
                first,
                construct_parametric_curve_path_cancellable(&curve, Point2::new(0.0, 0.0), &|| {
                    false
                })
                .expect("repeat")
            );
            let bounds =
                Bounds::new(Point2::new(-20.0, -20.0), Point2::new(20.0, 20.0)).expect("bounds");
            let clipped = first.clip_to_bounds(bounds).expect("crossing clip");
            assert!(!clipped.is_empty());
            assert_eq!(clipped, first.clip_to_bounds(bounds).expect("repeat clip"));
            assert!(clipped.iter().all(|path| {
                path.segments()
                    .windows(2)
                    .all(|pair| pair[0].end() == pair[1].start())
            }));
        }
    }
}

/// Verifies exact construction/contained/crossing policy boundaries and controlled copy/clip stops.
/// # Panics
/// Panics if finite callers are ignored or cancellation exposes partial transformed fragments.
#[test]
fn finite_boundaries_and_cancellation_cover_transforms_copies_and_clipping() {
    let curve = ParametricCurve::Spiral(spiral(SpiralShape::Round, 5.375, CurveWinding::Clockwise));
    let origin = Point2::new(0.0, 0.0);
    let path =
        construct_parametric_curve_path_cancellable(&curve, origin, &|| false).expect("path");
    let n = path.segments().len();
    let limits = CurveWorkLimits::new(n, usize::MAX, usize::MAX, usize::MAX).expect("limits");
    assert_eq!(
        path,
        construct_parametric_curve_path_with_limits_cancellable(&curve, origin, limits, &|| false)
            .expect("exact finite bound")
    );
    assert!(
        construct_parametric_curve_path_with_limits_cancellable(
            &curve,
            origin,
            CurveWorkLimits::new(n - 1, usize::MAX, usize::MAX, usize::MAX).unwrap(),
            &|| false
        )
        .is_err()
    );
    let contained = Bounds::new(Point2::new(-100.0, -100.0), Point2::new(100.0, 100.0)).unwrap();
    assert_eq!(
        path.clip_to_bounds_with_limits_cancellable(
            contained,
            CurveWorkLimits::new(n, usize::MAX, 1, n).unwrap(),
            &|| false
        )
        .unwrap(),
        vec![path.clone()]
    );
    assert_eq!(
        path.clip_to_bounds_with_limits_cancellable(
            contained,
            CurveWorkLimits::new(n, usize::MAX, 1, n - 1).unwrap(),
            &|| false
        )
        .unwrap_err()
        .path(),
        "curve.path.clipping.segment_limit"
    );
    let crossing = Bounds::new(Point2::new(-20.0, -20.0), Point2::new(20.0, 20.0)).unwrap();
    assert_eq!(
        path.clip_to_bounds_with_limits_cancellable(
            crossing,
            CurveWorkLimits::new(n, 1, usize::MAX, usize::MAX).unwrap(),
            &|| false
        )
        .unwrap_err()
        .path(),
        "curve.path.clipping.subdivision_limit"
    );
    let expected = path.clip_to_bounds(crossing).unwrap();
    let total = expected.iter().map(|p| p.segments().len()).sum();
    assert_eq!(
        expected,
        path.clip_to_bounds_with_limits_cancellable(
            crossing,
            CurveWorkLimits::new(n, usize::MAX, expected.len(), total).unwrap(),
            &|| false
        )
        .unwrap()
    );
    assert!(expected.len() > 1 && total > 1);
    for (fragments, segments, diagnostic) in [
        (
            expected.len() - 1,
            total,
            "curve.path.clipping.fragment_limit",
        ),
        (
            expected.len(),
            total - 1,
            "curve.path.clipping.segment_limit",
        ),
    ] {
        assert_eq!(
            path.clip_to_bounds_with_limits_cancellable(
                crossing,
                CurveWorkLimits::new(n, usize::MAX, fragments, segments).unwrap(),
                &|| false,
            )
            .unwrap_err()
            .path(),
            diagnostic
        );
    }
    let polls = Cell::new(0);
    assert_eq!(
        path.clip_to_bounds_with_limits_cancellable(contained, limits, &|| {
            polls.set(polls.get() + 1);
            polls.get() == n + 8
        })
        .unwrap_err()
        .path(),
        "evaluation.cancelled"
    );
    assert_eq!(
        polls.get(),
        n + 8,
        "cancellation reaches the contained copy after its full containment scan"
    );
    let transform =
        AffineTransform2D::rotate_about_then_translate(origin, 13.0, Vector2::new(7.0, -9.0))
            .unwrap();
    for mode in 0..3 {
        let polls = Cell::new(0);
        let cancelled = || {
            let next = polls.get() + 1;
            polls.set(next);
            next == 8
        };
        let error = match mode {
            0 => path
                .cloned_with_limits_cancellable(limits, &cancelled)
                .unwrap_err(),
            1 => path
                .transformed_with_limits_cancellable(transform, limits, &cancelled)
                .unwrap_err(),
            _ => path
                .clip_to_bounds_with_limits_cancellable(crossing, limits, &cancelled)
                .unwrap_err(),
        };
        assert_eq!(error.path(), "evaluation.cancelled");
        assert_eq!(polls.get(), 8);
    }
}

/// Stops real construction at a deterministic cancellation poll without publishing a partial path.
///
/// # Panics
/// Panics if either shape misses the cancellation checkpoint or returns a geometry failure.
#[test]
fn construction_cancellation_is_controlled_without_wall_clock_races() {
    for shape in [SpiralShape::Round, SpiralShape::Square] {
        let polls = Cell::new(0_usize);
        let error = construct_parametric_curve_path_cancellable(
            &ParametricCurve::Spiral(spiral(shape, 5.375, CurveWinding::Clockwise)),
            Point2::new(0.0, 0.0),
            &|| {
                let value = polls.get() + 1;
                polls.set(value);
                value == 8
            },
        )
        .expect_err("controlled cancellation");
        assert_eq!(polls.get(), 8);
        assert_eq!(error.path(), "evaluation.cancelled");
    }
}

/// Establishes safe machine-capacity failure without attempting an allocator/OS OOM event.
/// Production rejects checked conversion/byte-addressability failures before storage growth;
/// the dedicated unit seam separately exercises a reported reserve failure.
///
/// # Panics
/// Panics if machine overflow or impossible byte addressability can be admitted.
#[test]
fn capacity_overflow_witness_never_requests_large_memory() {
    assert!(usize::MAX.checked_add(1).is_none());
    assert!(
        usize::MAX
            .checked_mul(std::mem::size_of::<CurveSegment>())
            .is_none()
    );
    let mut storage: Vec<CurveSegment> = Vec::new();
    assert!(storage.try_reserve(usize::MAX).is_err());
    assert!(storage.is_empty());
    for shape in [SpiralShape::Round, SpiralShape::Square] {
        let turns = (isize::MAX as usize / std::mem::size_of::<CurveSegment>()) as f64 / 2.0;
        let error = construct_parametric_curve_path_cancellable(
            &ParametricCurve::Spiral(spiral(shape, turns, CurveWinding::Clockwise)),
            Point2::new(0.0, 0.0),
            &|| false,
        )
        .expect_err("proven impossible byte count before allocation");
        assert_eq!(error.path(), "curve.path.count_overflow");
    }
    for turns in [f64::MAX, usize::MAX as f64] {
        let error = construct_parametric_curve_path_cancellable(
            &ParametricCurve::Spiral(spiral(SpiralShape::Round, turns, CurveWinding::Clockwise)),
            Point2::new(0.0, 0.0),
            &|| false,
        )
        .expect_err("unaddressable count rejected");
        assert_eq!(error.path(), "curve.path.count_overflow");
    }
}

/// Cancels inside root isolation for one crossing cubic, after its single containment check.
/// # Panics
/// Panics if clipping misses the controlled root-work checkpoint or publishes partial fragments.
#[test]
fn crossing_cubic_root_isolation_observes_cancellation() {
    let path = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(-4.0, 0.0),
                Point2::new(4.0, 6.0),
                Point2::new(-4.0, -6.0),
                Point2::new(4.0, 0.0),
            )
            .unwrap(),
        )],
        PathClosure::Open,
    )
    .unwrap();
    let bounds = Bounds::new(Point2::new(-1.0, -1.0), Point2::new(1.0, 1.0)).unwrap();
    let polls = Cell::new(0);
    assert_eq!(
        path.clip_to_bounds_with_limits_cancellable(bounds, CurveWorkLimits::default(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == 12
        })
        .unwrap_err()
        .path(),
        "evaluation.cancelled"
    );
    assert_eq!(polls.get(), 12);
    assert!(!path.clip_to_bounds(bounds).unwrap().is_empty());
}

/// Cancels late in the public outline builder after full validation and rail construction.
/// # Panics
/// Panics if a late bounds/publication cancellation returns a partial outline or harms exact recovery.
#[test]
fn outline_validation_and_publication_cancellation_are_atomic() {
    use toniator_geometry::{
        PathLocation, VariableWidthOutlineLimits, VariableWidthPathSample,
        build_variable_width_outline_cancellable,
    };
    let path = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)).unwrap();
    let samples = [0.0, 0.5, 1.0].map(|t| VariableWidthPathSample {
        location: PathLocation::new(0, t).unwrap(),
        width: 2.0,
    });
    let polls = Cell::new(0);
    let build = |cancelled: &dyn Fn() -> bool| {
        build_variable_width_outline_cancellable(
            &path,
            &samples,
            toniator_domain::PathStrokeStyle::default(),
            0.0,
            0.125,
            VariableWidthOutlineLimits::new(100).unwrap(),
            cancelled,
        )
    };
    let expected = build(&|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let complete = polls.get();
    assert!(complete > 12);
    for stop in [2, complete - 2] {
        polls.set(0);
        assert_eq!(
            build(&|| {
                polls.set(polls.get() + 1);
                polls.get() == stop
            })
            .unwrap_err()
            .path(),
            "evaluation.cancelled"
        );
        assert_eq!(polls.get(), stop);
    }
    assert_eq!(build(&|| false).unwrap(), expected);
}

/// Rejects malformed public square intent and finite arithmetic overflow before unbounded vertex staging.
/// # Panics
/// Panics if nonfinite origin/spacing/phase or computed vertices can reach publication or long growth.
#[test]
fn malformed_square_intent_rejects_before_vertex_staging() {
    let base = spiral(SpiralShape::Square, 1024.375, CurveWinding::Clockwise);
    for (curve, origin, diagnostic) in [
        (
            base.clone(),
            Point2::new(f64::NAN, 0.0),
            "curve.path.polyline.vertices",
        ),
        (
            SpiralCurve {
                radial_spacing: f64::INFINITY,
                ..base.clone()
            },
            Point2::new(0.0, 0.0),
            "curve.path.polyline.vertices",
        ),
        (
            SpiralCurve {
                phase_degrees: f64::NAN,
                ..base.clone()
            },
            Point2::new(0.0, 0.0),
            "curve.path.polyline.vertices",
        ),
        (
            SpiralCurve {
                radial_spacing: f64::MAX,
                phase_degrees: 0.0,
                ..base
            },
            Point2::new(0.0, 0.0),
            "curve.path.polyline.vertices",
        ),
    ] {
        let polls = Cell::new(0);
        let error = construct_parametric_curve_path_cancellable(
            &ParametricCurve::Spiral(curve),
            origin,
            &|| {
                polls.set(polls.get() + 1);
                false
            },
        )
        .unwrap_err();
        assert_eq!(error.path(), diagnostic);
        assert!(polls.get() < 12, "early public-boundary rejection");
    }
}

/// Measures typed cancellation from an externally stored flag through the actual geometry operation return.
/// Preparation and controller joining are outside the interval; failed local storage cleanup is inside it.
/// # Panics
/// Panics if a controlled checkpoint is not reached or the operation publishes success/another failure.
fn measured_geometry_cancel(
    phase: &str,
    threshold: usize,
    operation: impl FnOnce(&dyn Fn() -> bool) -> Result<(), toniator_geometry::CurveError>,
) {
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    };
    let stop = AtomicBool::new(false);
    let polls = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let stopped = &stop;
        let controller = scope.spawn(move || {
            rx.recv().ok().map(|()| {
                let requested = std::time::Instant::now();
                stopped.store(true, Ordering::SeqCst);
                requested
            })
        });
        let result = operation(&|| {
            let count = polls.fetch_add(1, Ordering::Relaxed) + 1;
            if count == threshold {
                tx.send(()).unwrap();
            }
            stop.load(Ordering::SeqCst)
        });
        let returned = std::time::Instant::now();
        drop(tx);
        let requested = controller.join().unwrap().expect("checkpoint reached");
        if result.is_ok() {
            println!(
                "{{\"kind\":\"cancel_natural_completion\",\"phase\":\"{phase}\",\"mode\":\"nonblocking-inflight\"}}"
            );
            return;
        }
        assert_eq!(result.unwrap_err().path(), "evaluation.cancelled");
        println!(
            "{{\"kind\":\"cancel_latency\",\"phase\":\"{phase}\",\"mode\":\"nonblocking-inflight\",\"request_to_typed_return_ns\":{},\"polls\":{},\"partial_output\":false}}",
            returned.duration_since(requested).as_nanos(),
            polls.load(Ordering::Relaxed)
        );
    });
}

/// Captures three release-profile construction/copy/transform/crossing-clip cancellation observations.
/// # Panics
/// Panics if a reached producer ignores a controlled cancellation checkpoint.
#[test]
fn release_geometry_cancellation_latency_observations() {
    let round = ParametricCurve::Spiral(spiral(
        SpiralShape::Round,
        1024.375,
        CurveWinding::Clockwise,
    ));
    let square = ParametricCurve::Spiral(spiral(
        SpiralShape::Square,
        1024.375,
        CurveWinding::Clockwise,
    ));
    let origin = Point2::new(0.0, 0.0);
    let path = construct_parametric_curve_path_cancellable(&round, origin, &|| false).unwrap();
    let transform =
        AffineTransform2D::rotate_about_then_translate(origin, 13.0, Vector2::new(7.0, -9.0))
            .unwrap();
    let bounds = Bounds::new(Point2::new(-20.0, -20.0), Point2::new(20.0, 20.0)).unwrap();
    for _ in 0..3 {
        for (phase, curve) in [
            ("round-construction", &round),
            ("square-construction", &square),
        ] {
            measured_geometry_cancel(phase, 8, |cancel| {
                construct_parametric_curve_path_cancellable(curve, origin, cancel).map(|_| ())
            });
        }
        measured_geometry_cancel("path-copy", 128, |cancel| {
            path.cloned_with_limits_cancellable(CurveWorkLimits::default(), cancel)
                .map(|_| ())
        });
        measured_geometry_cancel("path-transform", 128, |cancel| {
            path.transformed_with_limits_cancellable(transform, CurveWorkLimits::default(), cancel)
                .map(|_| ())
        });
        measured_geometry_cancel(
            "crossing-clip-after-containment",
            path.segments().len() + 128,
            |cancel| {
                path.clip_to_bounds_with_limits_cancellable(
                    bounds,
                    CurveWorkLimits::default(),
                    cancel,
                )
                .map(|_| ())
            },
        );
    }
}
