use toniator_geometry::{
    CubicBezierSegment, CurvePath, CurveSegment, LineSegment, PathClosure, PathLocation, Point2,
};

/// Builds a two-line open path used by immutable editing witnesses.
fn open_path() -> CurvePath {
    CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(6.0, 0.0),
            Point2::new(6.0, 6.0),
        ],
        PathClosure::Open,
    )
    .expect("finite open path")
}

/// Proves anchor/control movement and line/cubic conversion preserve immutable connected topology.
#[test]
fn path_edits_move_construction_points_and_convert_segment_kinds() {
    let cubic = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(5.0, 0.0),
                Point2::new(6.0, 0.0),
            )
            .expect("finite cubic"),
        )],
        PathClosure::Open,
    )
    .expect("valid path");
    let moved = cubic
        .move_anchor(0, Point2::new(2.0, 3.0))
        .expect("anchor moves");
    let CurveSegment::CubicBezier(segment) = moved.segments()[0] else {
        panic!("retains cubic")
    };
    assert_eq!(segment.start(), Point2::new(2.0, 3.0));
    assert_eq!(segment.control_1(), Point2::new(3.0, 3.0));
    let control = moved
        .move_cubic_control(0, false, Point2::new(4.0, 8.0))
        .expect("control moves");
    let CurveSegment::CubicBezier(segment) = control.segments()[0] else {
        panic!("retains cubic")
    };
    assert_eq!(segment.control_2(), Point2::new(4.0, 8.0));
    let line = control.toggle_segment_kind(0).expect("cubic becomes line");
    assert!(matches!(line.segments()[0], CurveSegment::Line(_)));
    let round_trip = line.toggle_segment_kind(0).expect("line becomes cubic");
    assert!(matches!(
        round_trip.segments()[0],
        CurveSegment::CubicBezier(_)
    ));
}

/// Proves exact line and cubic subdivision retain the inserted anchor and ordered continuity.
#[test]
fn insertion_splits_lines_and_cubics_exactly() {
    let line = CurvePath::new(
        vec![CurveSegment::Line(
            LineSegment::new(Point2::new(0.0, 0.0), Point2::new(8.0, 0.0)).expect("line"),
        )],
        PathClosure::Open,
    )
    .expect("path");
    let split = line
        .insert_node(PathLocation::new(0, 0.25).expect("location"))
        .expect("split");
    assert_eq!(split.segments()[0].end(), Point2::new(2.0, 0.0));
    assert_eq!(split.segments()[0].end(), split.segments()[1].start());
    let cubic = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(0.0, 0.0),
                Point2::new(0.0, 6.0),
                Point2::new(6.0, 6.0),
                Point2::new(6.0, 0.0),
            )
            .expect("cubic"),
        )],
        PathClosure::Open,
    )
    .expect("path");
    let expected = cubic
        .point_at(PathLocation::new(0, 0.5).expect("location"))
        .expect("point");
    let split = cubic
        .insert_node(PathLocation::new(0, 0.5).expect("location"))
        .expect("split");
    assert_eq!(split.segments()[0].end(), expected);
    assert_eq!(split.segments()[0].end(), split.segments()[1].start());
}

/// Proves endpoint, interior, and closed deletion retain two-node bounds and connected replacements.
#[test]
fn deletion_reconnects_paths_and_refuses_the_two_node_minimum() {
    let endpoint = open_path().delete_node(0).expect("endpoint deletion");
    assert_eq!(endpoint.segments().len(), 1);
    let interior = open_path().delete_node(1).expect("interior deletion");
    assert_eq!(interior.segments().len(), 1);
    assert_eq!(interior.start(), Point2::new(0.0, 0.0));
    assert_eq!(interior.end(), Point2::new(6.0, 6.0));
    assert_eq!(
        interior
            .delete_node(0)
            .expect_err("two node minimum")
            .path(),
        "curve.path.edit.node_minimum"
    );
    let closed = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(2.0, 4.0),
        ],
        PathClosure::Closed,
    )
    .expect("closed path");
    let deleted = closed.delete_node(1).expect("closed deletion");
    assert_eq!(deleted.closure(), PathClosure::Closed);
    assert_eq!(deleted.segments().len(), 2);
}

/// Proves deterministic deletion retains the outer endpoints and outward cubic directions.
#[test]
fn deletion_fit_retains_endpoints_directions_and_is_deterministic() {
    let path = CurvePath::new(
        vec![
            CurveSegment::CubicBezier(
                CubicBezierSegment::new(
                    Point2::new(0.0, 0.0),
                    Point2::new(2.0, 0.0),
                    Point2::new(4.0, 3.0),
                    Point2::new(5.0, 4.0),
                )
                .expect("first cubic"),
            ),
            CurveSegment::CubicBezier(
                CubicBezierSegment::new(
                    Point2::new(5.0, 4.0),
                    Point2::new(7.0, 6.0),
                    Point2::new(9.0, 2.0),
                    Point2::new(10.0, 0.0),
                )
                .expect("second cubic"),
            ),
        ],
        PathClosure::Open,
    )
    .expect("connected path");
    let first = path.delete_node(1).expect("fitted deletion");
    let second = path.delete_node(1).expect("repeat fitted deletion");
    assert_eq!(first, second);
    assert_eq!(first.start(), Point2::new(0.0, 0.0));
    assert_eq!(first.end(), Point2::new(10.0, 0.0));
    if let CurveSegment::CubicBezier(segment) = first.segments()[0] {
        let retained_incoming = Point2::new(5.0 - 4.0, 4.0 - 3.0);
        let retained_outgoing = Point2::new(7.0 - 5.0, 6.0 - 4.0);
        let fitted_incoming = Point2::new(
            segment.control_1().x - segment.start().x,
            segment.control_1().y - segment.start().y,
        );
        let fitted_outgoing = Point2::new(
            segment.end().x - segment.control_2().x,
            segment.end().y - segment.control_2().y,
        );
        assert!(
            retained_incoming.x * fitted_incoming.x + retained_incoming.y * fitted_incoming.y
                >= 0.0
        );
        assert!(
            retained_outgoing.x * fitted_outgoing.x + retained_outgoing.y * fitted_outgoing.y
                >= 0.0
        );
    }
}

/// Proves deleting the closed seam retains closure and exact connected endpoints.
#[test]
fn deletion_reconnects_the_closed_seam() {
    let path = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(6.0, 0.0),
            Point2::new(6.0, 6.0),
            Point2::new(0.0, 6.0),
        ],
        PathClosure::Closed,
    )
    .expect("closed path");
    let deleted = path.delete_node(0).expect("seam deletion");
    assert_eq!(deleted.closure(), PathClosure::Closed);
    assert_eq!(deleted.segments().len(), 3);
    assert_eq!(
        deleted.segments().last().expect("segment").end(),
        deleted.start()
    );
    for pair in deleted.segments().windows(2) {
        assert_eq!(pair[0].end(), pair[1].start());
    }
}

/// Proves public collinear deletion reaches the singular bounded-fit fallback without partial output.
#[test]
fn deletion_reconnects_a_singular_collinear_pair_with_finite_nonnegative_handles() {
    let path = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(5.0, 0.0),
            Point2::new(10.0, 0.0),
        ],
        PathClosure::Open,
    )
    .expect("collinear path");
    let deleted = path.delete_node(1).expect("singular reconnect");
    assert_eq!(deleted.start(), Point2::new(0.0, 0.0));
    assert_eq!(deleted.end(), Point2::new(10.0, 0.0));
    assert!(
        deleted
            .segments()
            .iter()
            .all(|segment| segment.start().is_finite() && segment.end().is_finite())
    );
    if let CurveSegment::CubicBezier(segment) = deleted.segments()[0] {
        assert!(segment.control_1().x >= segment.start().x);
        assert!(segment.control_2().x <= segment.end().x);
    }
}

/// Proves constructor validation rejects nonfinite geometry and bounded path limits before editing.
#[test]
fn deletion_inputs_remain_finite_and_bounded() {
    assert_eq!(
        LineSegment::new(Point2::new(f64::NAN, 0.0), Point2::new(1.0, 0.0))
            .expect_err("nonfinite line")
            .path(),
        "curve.segment.coordinates"
    );
    let segment = CurveSegment::Line(
        LineSegment::new(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)).expect("line"),
    );
    assert_eq!(
        CurvePath::new(vec![segment; 4_097], PathClosure::Open)
            .expect_err("segment limit")
            .path(),
        "curve.path.segments.limit"
    );
}
