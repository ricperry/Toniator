//! Current intersection capacity, finite policy and atomic cancellation witnesses.
use std::cell::Cell;
use toniator_geometry::{
    CubicBezierSegment, CurvePath, CurveSegment, IntersectionWorkLimits, PathClosure, Point2,
};

/// Builds perpendicular stored-order polylines without changing their analytic single crossing.
/// # Panics
/// Panics if finite connected lines cannot form the current explicit path type.
fn crossing(count: usize) -> (CurvePath, CurvePath) {
    let coordinates: Vec<_> = (0..=count)
        .map(|i| -1.0 + 2.0 * i as f64 / count as f64)
        .collect();
    (
        CurvePath::polyline(
            coordinates.iter().map(|&x| Point2::new(x, 0.0)).collect(),
            PathClosure::Open,
        )
        .unwrap(),
        CurvePath::polyline(
            coordinates.iter().map(|&y| Point2::new(0.0, y)).collect(),
            PathClosure::Open,
        )
        .unwrap(),
    )
}

/// Builds finite serpentine rows/columns with isolated crossings and disjoint connector bands.
/// # Panics
/// Panics if explicit source endpoints cannot form valid connected line paths.
fn serpentine(rows: usize) -> (CurvePath, CurvePath) {
    let mut points = Vec::new();
    for row in 0..rows {
        let y = -0.8 + 1.6 * row as f64 / (rows - 1) as f64;
        let (a, b) = if row % 2 == 0 {
            (-1.0, 1.0)
        } else {
            (1.0, -1.0)
        };
        points.extend([Point2::new(a, y), Point2::new(b, y)]);
    }
    (
        CurvePath::polyline(points.clone(), PathClosure::Open).unwrap(),
        CurvePath::polyline(
            points.iter().map(|p| Point2::new(p.y, p.x)).collect(),
            PathClosure::Open,
        )
        .unwrap(),
    )
}

/// Admits the proven pair-count witness and enforces exact explicit finite pair/result boundaries.
/// # Panics
/// Panics on count ceilings, lost contacts, changed ordering or pre-dedup result refusal.
#[test]
fn normal_pairs_exceed_old_ceiling_and_explicit_bounds_remain_exact() {
    for count in [512, 513] {
        let (a, b) = crossing(count);
        let expected = a.intersections(&b).unwrap();
        assert_eq!(expected.len(), 1);
        assert!(expected[0].point().x.abs() < 1e-12 && expected[0].point().y.abs() < 1e-12);
        let exact = IntersectionWorkLimits::new(count * count, usize::MAX, 1).unwrap();
        assert_eq!(
            a.intersections_with_limits_cancellable(&b, exact, &|| false)
                .unwrap(),
            expected
        );
        let short = IntersectionWorkLimits::new(count * count - 1, usize::MAX, 1).unwrap();
        assert_eq!(
            a.intersections_with_limits_cancellable(&b, short, &|| false)
                .unwrap_err()
                .path(),
            "curve.path.intersections.pair_limit"
        );
    }
    // Four endpoint visits canonicalize to one contact before the explicit result limit is checked.
    let (a, b) = crossing(2);
    assert_eq!(
        a.intersections_with_limits_cancellable(
            &b,
            IntersectionWorkLimits::new(4, usize::MAX, 1).unwrap(),
            &|| false
        )
        .unwrap()
        .len(),
        1
    );
    for tuple in [(0, 1, 1), (1, 0, 1), (1, 1, 0)] {
        assert_eq!(
            IntersectionWorkLimits::new(tuple.0, tuple.1, tuple.2)
                .unwrap_err()
                .path(),
            "curve.path.intersections.limits"
        );
    }
    assert_eq!(
        IntersectionWorkLimits::default().maximum_segment_pairs(),
        usize::MAX
    );
    assert_eq!(
        IntersectionWorkLimits::default().maximum_subdivision_work(),
        usize::MAX
    );
    assert_eq!(
        IntersectionWorkLimits::default().maximum_results(),
        usize::MAX
    );
}

/// Publishes all10,000 analytic crossings with exact repeated ordering and finite post-dedup refusal.
/// # Panics
/// Panics if contacts are truncated, duplicated, moved, reordered or finite limits stop being explicit.
#[test]
fn ten_thousand_true_contacts_preserve_canonical_order_and_locations() {
    let (a, b) = serpentine(100);
    assert_eq!(a.segments().len(), 199);
    let result = a.intersections(&b).unwrap();
    assert_eq!(result.len(), 10_000);
    assert_eq!(a.intersections(&b).unwrap(), result);
    for contact in &result {
        assert!(
            contact.first_location().segment_index() % 2 == 0
                && contact.second_location().segment_index() % 2 == 0
        );
        let row = contact.first_location().segment_index() / 2;
        let col = contact.second_location().segment_index() / 2;
        let x = -0.8 + 1.6 * col as f64 / 99.0;
        let y = -0.8 + 1.6 * row as f64 / 99.0;
        assert!((contact.point().x - x).abs() < 1e-12 && (contact.point().y - y).abs() < 1e-12);
    }
    assert_eq!(
        a.intersections_with_limits_cancellable(
            &b,
            IntersectionWorkLimits::new(39_601, usize::MAX, 10_000).unwrap(),
            &|| false
        )
        .unwrap(),
        result
    );
    for maximum in [4_096, 9_999] {
        assert_eq!(
            a.intersections_with_limits_cancellable(
                &b,
                IntersectionWorkLimits::new(39_601, usize::MAX, maximum).unwrap(),
                &|| false
            )
            .unwrap_err()
            .path(),
            "curve.path.intersections.result_limit"
        );
    }
}

/// Cancels after bounds preparation inside pair enumeration and inside actual cubic subdivision.
/// # Panics
/// Panics if obsolete work publishes contacts or an explicit subdivision policy is bypassed.
#[test]
fn cancellation_and_subdivision_policy_do_not_publish_partial_contacts() {
    let (a, b) = crossing(513);
    let polls = Cell::new(0);
    let cancel = || {
        polls.set(polls.get() + 1);
        polls.get() > 2_000
    };
    assert_eq!(
        a.intersections_with_limits_cancellable(&b, IntersectionWorkLimits::default(), &cancel)
            .unwrap_err()
            .path(),
        "evaluation.cancelled"
    );
    assert_eq!(a.intersections(&b).unwrap().len(), 1);
    let cubic = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(-1.0, 0.0),
                Point2::new(-1.0, 2.0),
                Point2::new(1.0, 2.0),
                Point2::new(1.0, 0.0),
            )
            .unwrap(),
        )],
        PathClosure::Open,
    )
    .unwrap();
    let line = CurvePath::line(Point2::new(-2.0, 0.75), Point2::new(2.0, 0.75)).unwrap();
    assert_eq!(
        cubic
            .intersections_with_limits_cancellable(
                &line,
                IntersectionWorkLimits::new(1, 1, 10).unwrap(),
                &|| false
            )
            .unwrap_err()
            .path(),
        "curve.path.intersections.subdivision_limit"
    );
    let polls = Cell::new(0);
    assert_eq!(
        cubic
            .intersections_with_limits_cancellable(
                &line,
                IntersectionWorkLimits::default(),
                &|| {
                    polls.set(polls.get() + 1);
                    polls.get() > 30
                }
            )
            .unwrap_err()
            .path(),
        "evaluation.cancelled"
    );
    assert_eq!(cubic.intersections(&line).unwrap().len(), 2);
}
