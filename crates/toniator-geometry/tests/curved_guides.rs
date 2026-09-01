use toniator_domain::{AuthoredPoint2, GuideDimensionId, PatternMechanismId};
use toniator_geometry::{
    CubicBezierSegment, CurvePath, CurveSegment, IntersectionKind, PathClosure, PathOffsetLimits,
    Point2, StructuralPathInstance, StructuralPathInstanceId, StructuralPathSet,
    construct_circular_arc, insert_solved_crossing_nodes_cancellable,
};

/// Proves authored and fixed procedural prototypes expose deterministic ordered open curve paths.
#[test]
fn authored_and_circular_arc_prototypes_resolve_to_exact_ordered_guide_paths() {
    let arc = construct_circular_arc(AuthoredPoint2 { x: 2.0, y: -3.0 }, 10.0, 0.0, 360.0)
        .expect("full arc is a bounded open guide");
    assert_eq!(arc.closure(), PathClosure::Open);
    assert_eq!(arc.segments().len(), 4, "fixed policy splits at 90 degrees");
    assert_eq!(arc.start().x.to_bits(), 12.0_f64.to_bits());
    assert_eq!(arc.start().y.to_bits(), (-3.0_f64).to_bits());
    assert!((arc.start().x - arc.end().x).abs() < 1.0e-12);
    assert!((arc.start().y - arc.end().y).abs() < 1.0e-12);
    assert_eq!(
        construct_circular_arc(AuthoredPoint2 { x: 0.0, y: 0.0 }, 4.0, 0.0, 180.0)
            .unwrap()
            .segments()
            .len(),
        2
    );
}

/// Proves single and stack guide identities retain stored dimension order while each stack index ascends.
#[test]
fn single_and_transform_stack_coverage_emit_complete_deterministic_instances() {
    let path = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)).unwrap();
    let set = StructuralPathSet::new(
        "stage20d-order".into(),
        PatternMechanismId(17),
        vec![
            StructuralPathInstance {
                id: StructuralPathInstanceId::guide_dimension(GuideDimensionId(41), 0, 0),
                source_structure_id: None,
                path: path.clone(),
            },
            StructuralPathInstance {
                id: StructuralPathInstanceId::guide_dimension(GuideDimensionId(3), -1, 0),
                source_structure_id: None,
                path: path.clone(),
            },
            StructuralPathInstance {
                id: StructuralPathInstanceId::guide_dimension(GuideDimensionId(3), 0, 0),
                source_structure_id: None,
                path: path.clone(),
            },
            StructuralPathInstance {
                id: StructuralPathInstanceId::guide_dimension(GuideDimensionId(3), 1, 0),
                source_structure_id: None,
                path,
            },
        ],
    )
    .expect("a single guide followed by a transform stack preserves authored dimension order");
    assert_eq!(
        set.paths().iter().map(|guide| guide.id).collect::<Vec<_>>(),
        vec![
            StructuralPathInstanceId::guide_dimension(GuideDimensionId(41), 0, 0),
            StructuralPathInstanceId::guide_dimension(GuideDimensionId(3), -1, 0),
            StructuralPathInstanceId::guide_dimension(GuideDimensionId(3), 0, 0),
            StructuralPathInstanceId::guide_dimension(GuideDimensionId(3), 1, 0),
        ]
    );
    let invalid = StructuralPathSet::new(
        "stage20d-index".into(),
        PatternMechanismId(17),
        vec![
            set.paths()[0].clone(),
            StructuralPathInstance {
                id: StructuralPathInstanceId::guide_dimension(GuideDimensionId(3), 1, 0),
                source_structure_id: None,
                path: set.paths()[1].path.clone(),
            },
            StructuralPathInstance {
                id: StructuralPathInstanceId::guide_dimension(GuideDimensionId(3), 0, 0),
                source_structure_id: None,
                path: set.paths()[1].path.clone(),
            },
        ],
    )
    .unwrap_err();
    assert_eq!(invalid.path(), "curve.guide.path_set");
}

/// Proves curve intersections retain ordered segment locations, tangencies, and overlap rejection.
#[test]
fn curved_path_intersections_and_arc_length_sites_preserve_locations_and_limits() {
    let arc = construct_circular_arc(AuthoredPoint2 { x: 0.0, y: 0.0 }, 5.0, 0.0, 180.0).unwrap();
    let crossing = CurvePath::line(Point2::new(3.0, -8.0), Point2::new(3.0, 8.0)).unwrap();
    let contacts = arc.intersections(&crossing).unwrap();
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].first_location().segment_index(), 0);
    assert_eq!(contacts[0].second_location().segment_index(), 0);
    assert_eq!(contacts[0].kind(), IntersectionKind::Crossing);
    let tangent = CurvePath::line(Point2::new(-8.0, 5.0), Point2::new(8.0, 5.0)).unwrap();
    let tangent_contacts = arc.intersections(&tangent).unwrap();
    assert_eq!(tangent_contacts.len(), 1);
    assert_eq!(tangent_contacts[0].kind(), IntersectionKind::Tangent);
    let overlapping = CurvePath::line(arc.start(), arc.segments()[0].end()).unwrap();
    assert_eq!(
        overlapping.intersections(&overlapping).unwrap_err().path(),
        "curve.path.intersections.overlap"
    );
}

/// Proves arc-length measurement follows variable tangent geometry and supports exact anisotropic bounds upstream.
#[test]
fn curved_along_guide_coverage_uses_exact_anisotropic_interval_upper_bound() {
    let arc = construct_circular_arc(AuthoredPoint2 { x: 0.0, y: 0.0 }, 10.0, 0.0, 90.0).unwrap();
    let measured = arc.measure_arc_length().unwrap();
    let midpoint = measured
        .location_at_length(measured.total_length() / 2.0)
        .unwrap();
    let tangent = arc.unit_tangent_at(midpoint).unwrap();
    assert!(tangent.x < -0.6 && tangent.y > 0.6);
    assert!(
        (measured.total_length() - std::f64::consts::FRAC_PI_2 * 10.0).abs() < 0.01,
        "the measured curved interval must remain close to the finite quarter-arc authority"
    );
}

/// Proves transverse centerlines retain every branch and share one exact solved vector node.
///
/// # Panics
///
/// Panics when planarization deletes a branch, fails to subdivide both paths, moves a terminal,
/// or gives the two crossing paths different coordinates for their shared node.
#[test]
fn solved_crossing_planarization_preserves_branches_and_shared_nodes() {
    let first = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0)).unwrap();
    let second = CurvePath::line(Point2::new(0.0, 10.0), Point2::new(10.0, 0.0)).unwrap();
    let planarized = insert_solved_crossing_nodes_cancellable(
        &[first.clone(), second.clone()],
        PathOffsetLimits::default(),
        &|| false,
    )
    .expect("one exact crossing planarizes");
    assert_eq!(planarized.len(), 2);
    assert_eq!(planarized[0].segments().len(), 2);
    assert_eq!(planarized[1].segments().len(), 2);
    assert_eq!(planarized[0].start(), first.start());
    assert_eq!(planarized[0].end(), first.end());
    assert_eq!(planarized[1].start(), second.start());
    assert_eq!(planarized[1].end(), second.end());
    let shared = Point2::new(5.0, 5.0);
    assert_eq!(planarized[0].segments()[0].end(), shared);
    assert_eq!(planarized[0].segments()[1].start(), shared);
    assert_eq!(planarized[1].segments()[0].end(), shared);
    assert_eq!(planarized[1].segments()[1].start(), shared);
}

/// Proves a stored cusp node exposes its one-sided edge direction without weakening ordinary tangents.
///
/// # Panics
///
/// Panics when a stationary terminal handle is accepted by the ordinary tangent API or the
/// limiting API fails to recover the cubic's first moving control direction.
#[test]
fn stationary_cubic_endpoint_has_an_explicit_one_sided_tangent_limit() {
    let segment = CurveSegment::CubicBezier(
        CubicBezierSegment::new(
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 0.0),
            Point2::new(3.0, 0.0),
            Point2::new(4.0, 1.0),
        )
        .expect("finite stationary-endpoint cubic"),
    );
    assert_eq!(
        segment.unit_tangent_at(0.0).unwrap_err().path(),
        "curve.path.tangent.stationary"
    );
    let limiting = segment
        .limiting_unit_tangent_at(0.0)
        .expect("one-sided endpoint tangent exists");
    assert!((limiting.x - 1.0).abs() <= 1.0e-12);
    assert!(limiting.y.abs() <= 1.0e-12);
}
