//! Exact ray/cusp overlaps reuse the established Planar Constant-gap source-traced relink authority.
use toniator_geometry::*;

/// Reconstructs the exact retained rank-eleven frontier and solved switch from the captured caller.
/// # Panics
/// Panics if finite admitted line/cubic coordinates cannot form their exact connected path.
fn frontier(sign: f64) -> (CurvePath, Point2) {
    let p = |x, y| Point2::new(x, sign * y);
    let path = CurvePath::new(
        vec![
            CurveSegment::Line(
                LineSegment::new(p(-527.9290335677354, 220.0), p(55.47076752102494, 220.0))
                    .unwrap(),
            ),
            CurveSegment::Line(
                LineSegment::new(
                    p(55.47076752102494, 220.0),
                    p(55.506666793006644, 219.9478695931948),
                )
                .unwrap(),
            ),
            CurveSegment::CubicBezier(
                CubicBezierSegment::new(
                    p(55.506666793006644, 219.9478695931948),
                    p(56.78775995204543, 219.97434252505354),
                    p(58.073432182030764, 219.99183971500293),
                    p(59.36134548426724, 220.0),
                )
                .unwrap(),
            ),
            CurveSegment::Line(
                LineSegment::new(p(59.36134548426724, 220.0), p(527.9290335677354, 220.0)).unwrap(),
            ),
        ],
        PathClosure::Open,
    )
    .unwrap();
    (path, p(55.506666793006644, 219.9478695931948))
}

/// Relinks the captured cross-run overlap in both signs without losing shared nodes or source intervals.
/// # Panics
/// Panics if Planar cleanup rejects the exact overlap or changes finite source endpoint/trace authority.
#[test]
fn planar_cross_run_ray_overlap_preserves_exact_nodes_and_source_intervals() {
    for sign in [1.0, -1.0] {
        let (path, switch) = frontier(sign);
        let PathOffsetResult::Paths(parts) = advance_planar_constant_gap_frontier_cancellable(
            &path,
            &[switch],
            20.0 * sign,
            PathOffsetLimits::default(),
            &|| false,
        )
        .unwrap() else {
            panic!("finite frontier must survive")
        };
        let open = parts
            .iter()
            .find(|part| part.path.closure() == PathClosure::Open)
            .unwrap();
        assert_eq!(
            open.path.start(),
            Point2::new(-527.9290335677354, 240.0 * sign)
        );
        assert_eq!(
            open.path.end(),
            Point2::new(527.9290335677354, 240.0 * sign)
        );
        assert_eq!(open.source_start, PathLocation::new(0, 0.0).unwrap());
        assert_eq!(open.source_end, PathLocation::new(3, 1.0).unwrap());
        for part in &parts {
            assert_eq!(part.source_intervals.len(), part.path.segments().len());
            for pair in part.path.segments().windows(2) {
                assert_eq!(pair[0].end(), pair[1].start());
            }
            for (start, end) in &part.source_intervals {
                assert!(start.segment_index() < 4 && end.segment_index() < 4);
                assert!((0.0..=1.0).contains(&start.parameter()));
                assert!((0.0..=1.0).contains(&end.parameter()));
            }
        }
        let closed = parts
            .iter()
            .filter(|part| part.path.closure() == PathClosure::Closed)
            .collect::<Vec<_>>();
        assert!(
            !closed.is_empty(),
            "the enclosed reversal remains explicit solved topology"
        );
        for event in closed {
            let node = event.path.start();
            assert_eq!(node, event.path.end());
            assert!(
                open.path
                    .segments()
                    .iter()
                    .any(|segment| segment.start() == node || segment.end() == node)
            );
        }
    }
}

/// Preserves generic exact-overlap rejection and cooperative cancellation outside Planar relink authority.
/// # Panics
/// Panics if generic segment intersections accept coincident intervals or cancellation publishes a result.
#[test]
fn planar_ray_extension_does_not_relax_generic_overlap_or_cancellation() {
    let first = CurveSegment::Line(
        LineSegment::new(
            Point2::new(-527.9290335677354, 240.0),
            Point2::new(65.98147012004321, 240.0),
        )
        .unwrap(),
    );
    let second = CurveSegment::Line(
        LineSegment::new(
            Point2::new(59.29798560526803, 240.0),
            Point2::new(527.9290335677354, 240.0),
        )
        .unwrap(),
    );
    assert_eq!(
        first.intersections(&second).unwrap_err().path(),
        "curve.path.intersections.overlap"
    );
    let (path, switch) = frontier(1.0);
    assert_eq!(
        advance_planar_constant_gap_frontier_cancellable(
            &path,
            &[switch],
            20.0,
            PathOffsetLimits::default(),
            &|| true
        )
        .unwrap_err()
        .path(),
        "evaluation.cancelled"
    );
}
