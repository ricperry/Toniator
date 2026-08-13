use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureId,
    AuthoredStructureKind,
};
use toniator_geometry::{CurvePath, CurveSegment, PathClosure, Point2};

/// Builds one authored point whose IEEE-754 coordinates must cross the geometry boundary unchanged.
fn point(x: f64, y: f64) -> AuthoredPoint2 {
    AuthoredPoint2 { x, y }
}

/// Converts document-owned explicit segments into Stage 20B construction geometry without new semantics.
#[test]
fn document_authored_structures_resolve_to_exact_stage20b_curve_paths() {
    let structure = AuthoredStructure::new(
        AuthoredStructureId(5),
        AuthoredStructureKind::ClosedShape,
        vec![
            AuthoredCurveSegment::Line {
                start: point(-0.0, 1.25),
                end: point(2.5, 3.75),
            },
            AuthoredCurveSegment::CubicBezier {
                start: point(2.5, 3.75),
                control_1: point(4.0, -2.0),
                control_2: point(6.0, 8.0),
                end: point(-0.0, 1.25),
            },
        ],
    )
    .unwrap();
    let path = CurvePath::from_authored_structure(&structure).unwrap();
    assert_eq!(path.closure(), PathClosure::Closed);
    assert_eq!(path.segments().len(), 2);
    assert_eq!(path.start(), Point2::new(-0.0, 1.25));
    assert_eq!(path.end(), Point2::new(-0.0, 1.25));
    assert!(matches!(path.segments()[0], CurveSegment::Line(_)));
    assert!(matches!(path.segments()[1], CurveSegment::CubicBezier(_)));
    let CurveSegment::CubicBezier(cubic) = path.segments()[1] else {
        panic!("authored cubic must retain its construction variant");
    };
    assert_eq!(cubic.control_1(), Point2::new(4.0, -2.0));
    assert_eq!(cubic.control_2(), Point2::new(6.0, 8.0));
    assert_eq!(path.start().x.to_bits(), (-0.0_f64).to_bits());
}
