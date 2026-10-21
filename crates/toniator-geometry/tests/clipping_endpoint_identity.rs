//! Clipping preserves exact original solved endpoints when the retained interval includes them.
use toniator_geometry::*;

/// Supplies the exact rank-three Constant-gap line endpoints from the admitted runtime trace.
fn points() -> (Point2, Point2) {
    (
        Point2::new(-172.32666633521444, 174.95030048832723),
        Point2::new(-36.40790909903062, 130.20420557640384),
    )
}

/// Returns the original complete generation envelope, independent of the final presentation canvas.
fn bounds() -> Bounds {
    Bounds::new(Point2::new(-45.5, -45.5), Point2::new(165.5, 365.5)).unwrap()
}

/// Keeps the traced solved line join bit-exact instead of splitting one path into two ULP-separated pieces.
/// # Panics
/// Panics if retained t1 replays the original endpoint or changes the true snapped clipping boundary.
#[test]
fn line_clipping_preserves_original_join_endpoint_bits() {
    let (start, end) = points();
    let next = Point2::new(10.0, 170.0);
    let path = CurvePath::polyline(vec![start, end, next], PathClosure::Open).unwrap();
    let clipped = path.clip_to_bounds(bounds()).unwrap();
    assert_eq!(clipped.len(), 1);
    assert_eq!(clipped[0].segments()[0].end().x.to_bits(), end.x.to_bits());
    assert_eq!(clipped[0].segments()[0].end().y.to_bits(), end.y.to_bits());
    assert_eq!(clipped[0].segments()[1].start(), end);
    assert_eq!(clipped[0].start().x, -45.5);
    let reversed = CurvePath::line(end, start)
        .unwrap()
        .clip_to_bounds(bounds())
        .unwrap();
    assert_eq!(reversed[0].start(), end);
}

/// Demonstrates the analogous cubic range endpoint replay and retains source identity in both directions.
/// # Panics
/// Panics if a partially clipped cubic loses its exact source t0/t1 endpoint.
#[test]
fn cubic_clipping_preserves_original_range_endpoint_bits() {
    let (start, end) = points();
    let c1 = start;
    let c2 = start;
    for (a, b, c, d) in [(start, c1, c2, end), (end, c2, c1, start)] {
        let segment = CurveSegment::CubicBezier(CubicBezierSegment::new(a, b, c, d).unwrap());
        let path = CurvePath::new(vec![segment], PathClosure::Open).unwrap();
        let clipped = path.clip_to_bounds(bounds()).unwrap();
        assert_eq!(clipped.len(), 1);
        if a == end {
            assert_eq!(clipped[0].start(), end);
        } else {
            assert_eq!(clipped[0].end().x.to_bits(), end.x.to_bits());
            assert_eq!(clipped[0].end().y.to_bits(), end.y.to_bits());
        }
    }
}

/// Retains cancellation and true rejection while preserving ordinary clipping boundary snapping.
/// # Panics
/// Panics if exact endpoint handling bypasses cancellation, creates exterior geometry or loses snapped bounds.
#[test]
fn endpoint_specialization_retains_clipping_rejection_and_cancellation() {
    let (start, end) = points();
    let path = CurvePath::line(start, end).unwrap();
    let outside = Bounds::new(Point2::new(1000.0, 1000.0), Point2::new(1100.0, 1100.0)).unwrap();
    assert!(path.clip_to_bounds(outside).unwrap().is_empty());
    let error = path
        .clip_to_bounds_with_limits_cancellable(bounds(), CurveWorkLimits::default(), &|| true)
        .unwrap_err();
    assert_eq!(error.path(), "evaluation.cancelled");
}
