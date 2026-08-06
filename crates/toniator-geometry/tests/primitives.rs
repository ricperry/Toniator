use toniator_geometry::{AffineTransform2D, Bounds, Point2, Vector2};

#[test]
fn rotation_about_center_then_document_translation_round_trips() {
    let transform = AffineTransform2D::rotate_about_then_translate(
        Point2::new(450.0, 300.0),
        17.0,
        Vector2::new(3.25, -4.5),
    )
    .expect("finite transform");
    let point = Point2::new(100.0, 200.0);
    let document = transform.apply_point(point);
    let restored = transform.inverse_point(document);

    assert!((restored.x - point.x).abs() < 1e-10);
    assert!((restored.y - point.y).abs() < 1e-10);
    assert_eq!(
        transform.apply_point(Point2::new(450.0, 300.0)),
        Point2::new(453.25, 295.5)
    );
}

#[test]
fn inverse_transform_maps_every_padded_corner_to_finite_local_bounds() {
    let transform = AffineTransform2D::rotate_about_then_translate(
        Point2::new(450.0, 300.0),
        89.5,
        Vector2::new(-20.0, 30.0),
    )
    .expect("finite transform");
    let bounds =
        Bounds::new(Point2::new(-25.5, -25.5), Point2::new(925.5, 625.5)).expect("finite bounds");
    let local = transform
        .inverse_bounds(bounds)
        .expect("finite local bounds");

    assert!(local.min.is_finite());
    assert!(local.max.is_finite());
    for corner in bounds.corners() {
        assert!(local.contains(transform.inverse_point(corner)));
    }
}
