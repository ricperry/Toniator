use std::cell::{Cell, RefCell};
use toniator_patterns::{
    CurvePath, CurveSegment, CubicBezierSegment, Point2, TonePathDomain,
    prepare_tone_profile_cancellable, prepare_tone_group_profiles_cancellable,
};

/// Builds a genuine finite straight centerline used by the reached production sampler.
/// # Panics
/// Panics if the fixed finite endpoints fail canonical validation.
fn line(a: f64, b: f64) -> CurvePath {
    CurvePath::line(Point2::new(a, 0.0), Point2::new(b, 0.0)).unwrap()
}

/// Proves artistic knots are exact integer lattice anchors and refinement never queries artwork.
/// # Panics
/// Panics if the production profile rejects finite geometry or adds adaptive source samples.
#[test]
fn production_lattice_and_completed_response_are_independent_of_geometry_refinement() {
    let path = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(0.0, 0.0),
                Point2::new(0.0, 30.0),
                Point2::new(30.0, -30.0),
                Point2::new(30.0, 0.0),
            )
            .unwrap(),
        )],
        toniator_patterns::PathClosure::Open,
    )
    .unwrap();
    let calls = Cell::new(0);
    let profile = prepare_tone_profile_cancellable(
        &path,
        TonePathDomain::finite(),
        10.0,
        2.0,
        100_000,
        &|_| {
            calls.set(calls.get() + 1);
            Ok(if calls.get() == 2 { 0.0 } else { 1.0 })
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(calls.get(), profile.knots.len());
    assert!(profile.samples.len() > profile.knots.len());
    assert_eq!(profile.knots[1].distance, 10.0);
    assert_eq!(profile.knots[1].normalized_thickness, 0.0);
    assert!(
        profile
            .samples
            .iter()
            .any(|sample| sample.normalized_thickness == 0.0 && sample.width == 0.0)
    );
    for sample in &profile.samples {
        assert_eq!(sample.width, 2.0 * sample.normalized_thickness);
    }
}

/// Proves inserted component endpoints reconstruct from shared real-path knots, never tangent samples.
/// # Panics
/// Panics if a split group changes sampling positions, loses a zero run or restarts its lattice.
#[test]
fn disconnected_components_share_one_real_path_lattice_and_zero_runs() {
    let first = line(0.0, 8.0);
    let second = line(20.0, 42.0);
    let calls = RefCell::new(Vec::new());
    let profiles = prepare_tone_group_profiles_cancellable(
        &[
            (
                &first,
                TonePathDomain {
                    start_distance: 0.0,
                    genuine_start: true,
                    genuine_end: false,
                    ..TonePathDomain::finite()
                },
            ),
            (
                &second,
                TonePathDomain {
                    start_distance: 8.0,
                    genuine_start: false,
                    genuine_end: true,
                    ..TonePathDomain::finite()
                },
            ),
        ],
        10.0,
        1.0,
        10_000,
        &|point| {
            calls.borrow_mut().push(point.x);
            Ok(if point.x >= 22.0 && point.x <= 32.0 {
                0.0
            } else {
                1.0
            })
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(*calls.borrow(), vec![0.0, 22.0, 32.0, 42.0]);
    assert!(std::sync::Arc::ptr_eq(
        &profiles[0].knots,
        &profiles[1].knots
    ));
    assert!((profiles[0].samples.last().unwrap().normalized_thickness - 0.2).abs() < 1e-12);
    assert!((profiles[1].samples[0].normalized_thickness - 0.2).abs() < 1e-12);
    assert!(
        profiles[1]
            .samples
            .iter()
            .filter(|sample| sample.center.x >= 22.0 && sample.center.x <= 32.0)
            .all(|sample| sample.width == 0.0)
    );
}

/// Proves cubic subdivision does not add artistic source knots or move their logical anchors.
/// # Panics
/// Panics if the fixed numerical arc certificate cannot recover the same source positions.
#[test]
fn cubic_segmentation_preserves_lattice_queries() {
    let cubic = CubicBezierSegment::new(
        Point2::new(0.0, 0.0),
        Point2::new(0.0, 20.0),
        Point2::new(20.0, 20.0),
        Point2::new(20.0, 0.0),
    )
    .unwrap();
    let (a, b) = cubic.split(0.5).unwrap();
    let whole = CurvePath::new(
        vec![CurveSegment::CubicBezier(cubic)],
        toniator_patterns::PathClosure::Open,
    )
    .unwrap();
    let split = CurvePath::new(
        vec![CurveSegment::CubicBezier(a), CurveSegment::CubicBezier(b)],
        toniator_patterns::PathClosure::Open,
    )
    .unwrap();
    let run = |path: &CurvePath| {
        let calls = RefCell::new(Vec::new());
        prepare_tone_profile_cancellable(
            path,
            TonePathDomain::finite(),
            7.0,
            1.0,
            10_000,
            &|point| {
                calls.borrow_mut().push(point);
                Ok(0.5)
            },
            &|| false,
        )
        .unwrap();
        calls.into_inner()
    };
    let a = run(&whole);
    let b = run(&split);
    assert_eq!(a.len(), b.len());
    for (a, b) in a.into_iter().zip(b) {
        assert!((a.x - b.x).hypot(a.y - b.y) < 1e-6);
    }
}

/// Rejects malformed domains/invalid multipliers and cancels before any partial profile is returned.
/// # Panics
/// Panics if invalid current input is accepted or a cancelled preparation publishes geometry.
#[test]
fn invalids_and_cancellation_remain_atomic() {
    let path = line(0.0, 30.0);
    for interval in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            prepare_tone_profile_cancellable(
                &path,
                TonePathDomain::finite(),
                interval,
                1.0,
                100,
                &|_| Ok(1.0),
                &|| false
            )
            .is_err()
        );
    }
    assert_eq!(
        prepare_tone_group_profiles_cancellable(
            &[
                (&path, TonePathDomain::finite()),
                (&path, TonePathDomain::finite())
            ],
            10.0,
            1.0,
            100,
            &|_| Ok(1.0),
            &|| false
        )
        .unwrap_err()
        .path(),
        "realization.tone.domain"
    );
    let calls = Cell::new(0);
    let error = prepare_tone_profile_cancellable(
        &path,
        TonePathDomain::finite(),
        1.0,
        1.0,
        100,
        &|_| {
            calls.set(calls.get() + 1);
            Ok(1.0)
        },
        &|| calls.get() >= 2,
    )
    .unwrap_err();
    assert_eq!(error.path(), "evaluation.cancelled");
    assert_eq!(calls.get(), 2);
    assert!(
        prepare_tone_profile_cancellable(
            &path,
            TonePathDomain::finite(),
            1.0,
            1.0,
            100,
            &|_| Ok(1.0),
            &|| false
        )
        .is_ok()
    );
}

/// Reaches an authored terminal-ray policy when its stored finite endpoint segment remains cubic.
/// Finite paths still sample genuine endpoints; no generic tangent is invented for finite/motif kinds.
/// # Panics
/// Panics if coarse spacing rejects an authorized authored curve or ignores finite continuation authority.
#[test]
fn authored_terminal_cubic_accepts_coarse_spacing_without_a_materialized_ray() {
    let path = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(0.0, 0.0),
                Point2::new(0.0, 10.0),
                Point2::new(20.0, 10.0),
                Point2::new(20.0, 0.0),
            )
            .unwrap(),
        )],
        toniator_patterns::PathClosure::Open,
    )
    .unwrap();
    let domain = TonePathDomain {
        start_distance: 3.0,
        genuine_start: false,
        genuine_end: false,
        continuation: toniator_patterns::TonePathContinuation::AuthoredTerminalRays,
    };
    let queries = RefCell::new(Vec::new());
    let tone = prepare_tone_profile_cancellable(
        &path,
        domain,
        100.0,
        1.0,
        10_000,
        &|point| {
            queries.borrow_mut().push(point);
            Ok(0.5)
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(
        tone.knots
            .iter()
            .map(|knot| knot.distance)
            .collect::<Vec<_>>(),
        vec![0.0, 100.0]
    );
    assert_eq!(queries.borrow().len(), 2);
    assert!(queries.borrow()[0].y < 0.0);
    assert!(queries.borrow()[1].y < 0.0);
    let finite = prepare_tone_profile_cancellable(
        &path,
        TonePathDomain::finite(),
        100.0,
        1.0,
        10_000,
        &|_| Ok(0.5),
        &|| false,
    )
    .unwrap();
    assert_eq!(finite.knots[0].distance, 0.0);
    assert!(finite.knots[1].distance < 100.0);
    let invalid = TonePathDomain {
        continuation: toniator_patterns::TonePathContinuation::Finite,
        ..domain
    };
    assert_eq!(
        prepare_tone_profile_cancellable(&path, invalid, 100.0, 1.0, 10_000, &|_| Ok(0.5), &|| {
            false
        })
        .unwrap_err()
        .path(),
        "realization.tone.continuation"
    );
}

/// Preserves exact sampled zeros at both segment-joint addresses and both closed-seam addresses.
/// Positive neighboring knots retain their exact positive values; no proximity snapping is used.
/// # Panics
/// Panics if an inverse/forward residual makes a zero break positive or a seam queries artwork twice.
#[test]
fn exact_sampled_zero_survives_joint_and_closed_seam_aliases() {
    use toniator_patterns::{PathClosure, PathLocation};
    let point = |x, y| Point2::new(x, y);
    for closed in [false, true] {
        let vertices = if closed {
            vec![
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 10.0),
                point(0.0, 10.0),
                point(0.0, 0.0),
            ]
        } else {
            vec![point(0.0, 0.0), point(10.0, 0.0), point(10.0, 10.0)]
        };
        let path = CurvePath::polyline(
            vertices,
            if closed {
                PathClosure::Closed
            } else {
                PathClosure::Open
            },
        )
        .unwrap();
        let calls = Cell::new(0);
        let profile = prepare_tone_profile_cancellable(
            &path,
            TonePathDomain::finite(),
            5.0,
            2.0,
            10_000,
            &|position| {
                calls.set(calls.get() + 1);
                Ok(
                    if position == point(10.0, 0.0) || (closed && position == point(0.0, 0.0)) {
                        0.0
                    } else {
                        1.0
                    },
                )
            },
            &|| false,
        )
        .unwrap();
        let at = |segment, parameter| {
            profile
                .samples
                .iter()
                .find(|sample| sample.location == PathLocation::new(segment, parameter).unwrap())
                .unwrap()
                .width
        };
        assert_eq!(at(0, 1.0), 0.0);
        assert_eq!(at(1, 0.0), 0.0);
        assert_eq!(at(0, 0.5), 2.0);
        assert_eq!(at(1, 0.5), 2.0);
        if closed {
            assert_eq!(at(0, 0.0), 0.0);
            assert_eq!(at(3, 1.0), 0.0);
            assert_eq!(calls.get() + 1, profile.knots.len());
        } else {
            assert_eq!(calls.get(), profile.knots.len());
        }
    }
}
