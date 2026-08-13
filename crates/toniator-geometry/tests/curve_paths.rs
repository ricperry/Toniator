use toniator_geometry::{
    AffineTransform2D, Bounds, CubicBezierSegment, CurvePath, CurveSegment, IntersectionKind,
    LineSegment, PathClosure, PathLocation, Point2, Vector2,
};

/// Compares finite coordinates under the fixed public geometric tolerance envelope.
fn close(first: Point2, second: Point2) -> bool {
    (first.x - second.x).abs() <= 2.0e-8 && (first.y - second.y).abs() <= 2.0e-8
}

/// Builds a finite cubic with construction points that expose one nontrivial extrema and tangent.
fn arch() -> CubicBezierSegment {
    CubicBezierSegment::new(
        Point2::new(0.0, 0.0),
        Point2::new(0.0, 8.0),
        Point2::new(10.0, 8.0),
        Point2::new(10.0, 0.0),
    )
    .expect("finite cubic")
}

/// Estimates traveled segment length by deterministic chord sampling for independent inverse witnesses.
fn sampled_length(segment: CurveSegment, parameter: f64) -> f64 {
    let steps = 32_768_usize;
    let mut total = 0.0;
    let mut previous = segment.point_at(0.0).expect("finite start");
    for step in 1..=steps {
        let current = segment
            .point_at(parameter * step as f64 / steps as f64)
            .expect("finite sample");
        total += (current.x - previous.x).hypot(current.y - previous.y);
        previous = current;
    }
    total
}

/// Proves line, polyline, and cubic construction retain explicit C0 topology and authored closure.
#[test]
fn line_polyline_and_cubic_paths_preserve_explicit_topology() {
    let line = CurvePath::line(Point2::new(1.0, 2.0), Point2::new(3.0, 4.0)).expect("line path");
    assert_eq!(line.closure(), PathClosure::Open);
    assert_eq!(line.segments().len(), 1);
    let closed = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(2.0, 2.0),
        ],
        PathClosure::Closed,
    )
    .expect("closed convenience polyline");
    assert_eq!(closed.segments().len(), 3);
    assert_eq!(closed.start(), closed.end());
    let cubic = CurveSegment::CubicBezier(arch());
    let path = CurvePath::new(vec![cubic], PathClosure::Open).expect("open cubic path");
    assert_eq!(path.start(), Point2::new(0.0, 0.0));
    assert_eq!(path.end(), Point2::new(10.0, 0.0));
    assert_eq!(
        CurvePath::new(
            vec![CurveSegment::Line(
                LineSegment::new(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0))
                    .expect("finite line"),
            )],
            PathClosure::Closed,
        )
        .expect_err("constructor never manufactures closure")
        .path(),
        "curve.path.closure"
    );
}

/// Proves segment-local evaluation, left normals, conservative extrema bounds, and degeneracy errors.
#[test]
fn curve_evaluation_tangent_normal_and_bounds_cover_extrema_and_degeneracy() {
    let segment = CurveSegment::CubicBezier(arch());
    assert!(close(
        segment.point_at(0.5).expect("midpoint"),
        Point2::new(5.0, 6.0)
    ));
    let tangent = segment.unit_tangent_at(0.5).expect("nonstationary tangent");
    let normal = segment.unit_normal_at(0.5).expect("nonstationary normal");
    assert!(tangent.x > 0.999 && tangent.y.abs() < 1.0e-9);
    assert!(normal.y > 0.999 && normal.x.abs() < 1.0e-9);
    let bounds = segment.bounds().expect("conservative extrema bounds");
    assert!(bounds.contains(segment.point_at(0.0).expect("start")));
    assert!(bounds.contains(segment.point_at(0.5).expect("middle")));
    assert!(bounds.max.y >= 6.0);
    let x_extrema = CurveSegment::CubicBezier(
        CubicBezierSegment::new(
            Point2::new(0.0, 0.0),
            Point2::new(8.0, 0.0),
            Point2::new(-8.0, 1.0),
            Point2::new(0.0, 1.0),
        )
        .expect("finite x-extrema cubic"),
    );
    let x_bounds = x_extrema.bounds().expect("conservative x extrema");
    assert!(x_bounds.min.x < -2.0 && x_bounds.max.x > 2.0);
    let translated = CurveSegment::CubicBezier(
        CubicBezierSegment::new(
            Point2::new(1.0e12, 1.0e12),
            Point2::new(1.0e12 + 8.0e6, 1.0e12 + 3.0e6),
            Point2::new(1.0e12 - 8.0e6, 1.0e12 + 6.0e6),
            Point2::new(1.0e12, 1.0e12 + 9.0e6),
        )
        .expect("finite translated cubic"),
    );
    let translated_bounds = translated
        .bounds()
        .expect("conservative translated extrema");
    for sample in 0..=128 {
        assert!(
            translated_bounds.contains(
                translated
                    .point_at(sample as f64 / 128.0)
                    .expect("translated sample")
            )
        );
    }
    let stationary = CurveSegment::CubicBezier(
        CubicBezierSegment::new(
            Point2::new(1.0, 1.0),
            Point2::new(1.0, 1.0),
            Point2::new(1.0, 1.0),
            Point2::new(1.0, 1.0),
        )
        .expect("finite stationary cubic"),
    );
    assert_eq!(
        stationary
            .unit_tangent_at(0.5)
            .expect_err("stationary derivative")
            .path(),
        "curve.path.tangent.stationary"
    );
}

/// Proves immutable arc-length tables invert distances monotonically at joins and degenerate paths.
#[test]
fn path_arc_length_and_inverse_lookup_are_deterministic_monotone_and_bounded() {
    let path = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(3.0, 0.0),
            Point2::new(3.0, 4.0),
        ],
        PathClosure::Open,
    )
    .expect("finite polyline");
    let measured = path.measure_arc_length().expect("measured path");
    assert_eq!(measured.total_length(), 7.0);
    assert_eq!(
        measured.location_at_length(3.0).expect("join location"),
        PathLocation::new(0, 1.0).expect("valid location")
    );
    let mut previous = PathLocation::new(0, 0.0).expect("valid location");
    for distance in [0.0, 0.5, 3.0, 4.0, 7.0] {
        let location = measured
            .location_at_length(distance)
            .expect("bounded inverse");
        assert!(
            location.segment_index() > previous.segment_index()
                || (location.segment_index() == previous.segment_index()
                    && location.parameter() >= previous.parameter())
        );
        previous = location;
    }
    assert_eq!(
        measured
            .location_at_length(7.1)
            .expect_err("distance past total")
            .path(),
        "curve.path.arc_length.distance"
    );
    let cubic = CurvePath::new(vec![CurveSegment::CubicBezier(arch())], PathClosure::Open)
        .expect("nonuniform cubic");
    let cubic_measurement = cubic.measure_arc_length().expect("measured cubic");
    for fraction in [0.125, 0.25, 0.5, 0.875] {
        let requested = cubic_measurement.total_length() * fraction;
        let location = cubic_measurement
            .location_at_length(requested)
            .expect("cubic inverse");
        if fraction == 0.25 {
            assert!(
                location.parameter() < 0.25,
                "early vertical motion is nonuniform"
            );
        }
        let reconstructed = sampled_length(cubic.segments()[0], location.parameter());
        assert!((reconstructed - requested).abs() < 2.0e-8);
    }
}

/// Proves ordered line, line/cubic, and cubic/cubic contacts classify, deduplicate, and reject overlap.
#[test]
fn path_intersections_order_deduplicate_and_classify_crossings_tangencies_and_overlaps() {
    let horizontal = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)).expect("line");
    let vertical = CurvePath::line(Point2::new(5.0, -2.0), Point2::new(5.0, 2.0)).expect("line");
    let crossing = horizontal.intersections(&vertical).expect("line crossing");
    assert_eq!(crossing.len(), 1);
    assert_eq!(crossing[0].kind(), IntersectionKind::Crossing);
    let cubic_line = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(0.0, -2.0),
                Point2::new(10.0 / 3.0, -2.0 / 3.0),
                Point2::new(20.0 / 3.0, 2.0 / 3.0),
                Point2::new(10.0, 2.0),
            )
            .expect("finite straight cubic"),
        )],
        PathClosure::Open,
    )
    .expect("cubic path");
    assert_eq!(
        horizontal
            .intersections(&cubic_line)
            .expect("line cubic")
            .len(),
        1
    );
    let opposite_cubic = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(0.0, 2.0),
                Point2::new(10.0 / 3.0, 2.0 / 3.0),
                Point2::new(20.0 / 3.0, -2.0 / 3.0),
                Point2::new(10.0, -2.0),
            )
            .expect("finite straight cubic"),
        )],
        PathClosure::Open,
    )
    .expect("cubic path");
    assert_eq!(
        cubic_line
            .intersections(&opposite_cubic)
            .expect("cubic cubic")
            .len(),
        1
    );
    let tangent =
        CurvePath::line(Point2::new(10.0, 0.0), Point2::new(12.0, 3.0)).expect("endpoint contact");
    assert_eq!(
        horizontal
            .intersections(&tangent)
            .expect("endpoint tangent")[0]
            .kind(),
        IntersectionKind::Tangent
    );
    assert_eq!(
        horizontal
            .intersections(
                &CurvePath::line(Point2::new(2.0, 0.0), Point2::new(8.0, 0.0)).expect("overlap")
            )
            .expect_err("positive overlap has no discrete output")
            .path(),
        "curve.path.intersections.overlap"
    );
    let shallow_first = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(1.0, 1.0e-10))
        .expect("near-parallel line");
    let shallow_second = CurvePath::line(Point2::new(0.0, 1.0e-10), Point2::new(1.0, 0.0))
        .expect("near-parallel line");
    assert_eq!(
        shallow_first
            .intersections(&shallow_second)
            .expect("shallow crossing")[0]
            .kind(),
        IntersectionKind::Crossing
    );
    assert!(
        shallow_first
            .intersections(
                &CurvePath::line(Point2::new(0.0, 2.0e-9), Point2::new(1.0, 2.1e-9))
                    .expect("parallel line")
            )
            .expect("parallel nonintersection")
            .is_empty()
    );
    let tangent_cubic = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(1.0, 1.0),
                Point2::new(1.0, -1.0 / 3.0),
                Point2::new(1.0, -1.0 / 3.0),
                Point2::new(1.0, 1.0),
            )
            .expect("finite tangent cubic"),
        )],
        PathClosure::Open,
    )
    .expect("tangent cubic path");
    let tangent_line =
        CurvePath::line(Point2::new(0.0, 0.0), Point2::new(2.0, 0.0)).expect("tangent line");
    let tangent_contacts = tangent_cubic
        .intersections(&tangent_line)
        .expect("cubic tangency");
    assert_eq!(tangent_contacts.len(), 1);
    assert_eq!(tangent_contacts[0].kind(), IntersectionKind::Tangent);
    let high_curve = CurvePath::new(vec![CurveSegment::CubicBezier(arch())], PathClosure::Open)
        .expect("high-curvature path");
    let high_crossings = high_curve
        .intersections(
            &CurvePath::line(Point2::new(-1.0, 3.0), Point2::new(11.0, 3.0)).expect("probe"),
        )
        .expect("two high-curvature crossings");
    assert_eq!(high_crossings.len(), 2);
    assert!(
        high_crossings
            .iter()
            .all(|contact| contact.kind() == IntersectionKind::Crossing)
    );
    for contact in &high_crossings {
        assert!(close(
            contact.point(),
            high_curve
                .point_at(contact.first_location())
                .expect("first residual")
        ));
    }
    let high_tangent = high_curve
        .intersections(
            &CurvePath::line(Point2::new(-1.0, 6.0), Point2::new(11.0, 6.0)).expect("probe"),
        )
        .expect("high-curvature tangent");
    assert_eq!(high_tangent.len(), 1);
    assert_eq!(high_tangent[0].kind(), IntersectionKind::Tangent);
    let zigzag = CurvePath::polyline(
        vec![
            Point2::new(1.0, -2.0),
            Point2::new(1.0, 2.0),
            Point2::new(9.0, -2.0),
            Point2::new(9.0, 2.0),
        ],
        PathClosure::Open,
    )
    .expect("ordered intersections");
    let expected = horizontal
        .intersections(&zigzag)
        .expect("three ordered contacts");
    assert_eq!(expected.len(), 3);
    for _ in 0..8 {
        assert_eq!(
            horizontal.intersections(&zigzag).expect("repeatable order"),
            expected
        );
    }
    let seam = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 1.0),
            Point2::new(-1.0, 1.0),
        ],
        PathClosure::Closed,
    )
    .expect("closed seam path");
    let seam_hits = seam
        .intersections(
            &CurvePath::line(Point2::new(-2.0, 0.0), Point2::new(2.0, 0.0)).expect("seam probe"),
        )
        .expect("seam dedup");
    assert_eq!(seam_hits.len(), 1);
    assert_eq!(
        seam_hits[0].first_location(),
        PathLocation::new(0, 0.0).expect("canonical seam")
    );
    let joined = CurvePath::polyline(
        vec![
            Point2::new(-1.0, -1.0),
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 1.0),
        ],
        PathClosure::Open,
    )
    .expect("joined path");
    let join_hits = joined
        .intersections(
            &CurvePath::line(Point2::new(-1.0, 0.0), Point2::new(1.0, 0.0)).expect("join probe"),
        )
        .expect("join dedup");
    assert_eq!(join_hits.len(), 1);
    assert_eq!(
        join_hits[0].first_location(),
        PathLocation::new(0, 1.0).expect("earliest join")
    );
    let zero_segments = vec![
        CurveSegment::Line(
            LineSegment::new(Point2::new(4.0, 4.0), Point2::new(4.0, 4.0)).expect("zero segment")
        );
        65
    ];
    let first_zero_path =
        CurvePath::new(zero_segments.clone(), PathClosure::Open).expect("zero path");
    let second_zero_path = CurvePath::new(zero_segments, PathClosure::Open).expect("zero path");
    let deduplicated_zero_contacts = first_zero_path
        .intersections(&second_zero_path)
        .expect("raw duplicate contacts");
    assert_eq!(deduplicated_zero_contacts.len(), 1);
    assert_eq!(
        deduplicated_zero_contacts[0].kind(),
        IntersectionKind::Tangent
    );
}

/// Proves clipping retains source kind/order and creates no rectangle edges, closure, or seam joins.
#[test]
fn path_clipping_preserves_ordered_fragments_without_inventing_boundary_topology() {
    let bounds = Bounds::new(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0)).expect("bounds");
    let path = CurvePath::polyline(
        vec![
            Point2::new(-5.0, 5.0),
            Point2::new(5.0, 5.0),
            Point2::new(15.0, 5.0),
        ],
        PathClosure::Open,
    )
    .expect("line path");
    let fragments = path.clip_to_bounds(bounds).expect("clipped path");
    assert_eq!(fragments.len(), 1);
    assert_eq!(fragments[0].closure(), PathClosure::Open);
    assert_eq!(fragments[0].segments().len(), 2);
    for segment in fragments[0].segments() {
        for sample in 0..=8 {
            assert!(
                bounds.contains(
                    segment
                        .point_at(sample as f64 / 8.0)
                        .expect("finite sample")
                )
            );
        }
    }
    let contained = CurvePath::new(vec![CurveSegment::CubicBezier(arch())], PathClosure::Open)
        .expect("cubic path");
    let wide = Bounds::new(Point2::new(-1.0, -1.0), Point2::new(11.0, 9.0)).expect("wide bounds");
    assert_eq!(
        contained.clip_to_bounds(wide).expect("exact clone"),
        vec![contained]
    );
    let crossing_cubic = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(-2.0, 5.0),
                Point2::new(2.0, 5.0),
                Point2::new(8.0, 5.0),
                Point2::new(12.0, 5.0),
            )
            .expect("finite cubic"),
        )],
        PathClosure::Open,
    )
    .expect("crossing cubic path");
    let cubic_fragments = crossing_cubic
        .clip_to_bounds(bounds)
        .expect("clipped cubic");
    assert_eq!(cubic_fragments.len(), 1);
    assert!(matches!(
        cubic_fragments[0].segments()[0],
        CurveSegment::CubicBezier(_)
    ));
    for sample in 0..=16 {
        assert!(
            bounds.contains(
                cubic_fragments[0].segments()[0]
                    .point_at(sample as f64 / 16.0)
                    .expect("finite clipped sample")
            )
        );
    }
    let isolated_tangency = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(1.0, 1.0),
                Point2::new(1.0, -1.0 / 3.0),
                Point2::new(1.0, -1.0 / 3.0),
                Point2::new(1.0, 1.0),
            )
            .expect("finite tangent cubic"),
        )],
        PathClosure::Open,
    )
    .expect("tangent path");
    let tangent_bounds =
        Bounds::new(Point2::new(0.0, -2.0), Point2::new(2.0, 0.0)).expect("bounds");
    assert!(
        isolated_tangency
            .clip_to_bounds(tangent_bounds)
            .expect("tangent clipping")
            .is_empty()
    );
    let corner_touch =
        CurvePath::line(Point2::new(-2.0, -2.0), Point2::new(0.0, 0.0)).expect("corner-touch line");
    assert!(
        corner_touch
            .clip_to_bounds(bounds)
            .expect("corner clipping")
            .is_empty()
    );
    let closed_partial = CurvePath::polyline(
        vec![
            Point2::new(-2.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(0.0, 2.0),
        ],
        PathClosure::Closed,
    )
    .expect("closed path");
    let partial = closed_partial
        .clip_to_bounds(
            Bounds::new(Point2::new(-0.5, -0.5), Point2::new(0.5, 0.5)).expect("bounds"),
        )
        .expect("partial closed clip");
    assert!(
        partial
            .iter()
            .all(|fragment| fragment.closure() == PathClosure::Open)
    );
}

/// Proves transformed curves retain existing affine authority, segment kinds, and sampled geometry.
#[test]
fn curve_operations_remain_consistent_under_existing_affine_transforms() {
    let path = CurvePath::new(vec![CurveSegment::CubicBezier(arch())], PathClosure::Open)
        .expect("cubic path");
    let transform = AffineTransform2D::rotate_about_then_translate(
        Point2::new(5.0, 3.0),
        90.0,
        Vector2::new(2.0, -1.0),
    )
    .expect("finite transform");
    let transformed = path
        .transformed(transform)
        .expect("finite transformed path");
    assert!(matches!(
        transformed.segments()[0],
        CurveSegment::CubicBezier(_)
    ));
    for sample in 0..=16 {
        let location = PathLocation::new(0, sample as f64 / 16.0).expect("valid parameter");
        assert!(close(
            transformed.point_at(location).expect("transformed point"),
            transform.apply_point(path.point_at(location).expect("source point"))
        ));
    }
    assert!(
        (path
            .measure_arc_length()
            .expect("source length")
            .total_length()
            - transformed
                .measure_arc_length()
                .expect("rotated length")
                .total_length())
        .abs()
            < 2.0e-8
    );
    let source_tangent = path
        .unit_tangent_at(PathLocation::new(0, 0.25).expect("location"))
        .expect("source tangent");
    let transformed_tangent = transformed
        .unit_tangent_at(PathLocation::new(0, 0.25).expect("location"))
        .expect("transformed tangent");
    let expected_tangent = transform.apply_vector(source_tangent);
    assert!((transformed_tangent.x - expected_tangent.x).abs() < 2.0e-9);
    assert!((transformed_tangent.y - expected_tangent.y).abs() < 2.0e-9);
    let source_normal = path
        .unit_normal_at(PathLocation::new(0, 0.25).expect("location"))
        .expect("source normal");
    let transformed_normal = transformed
        .unit_normal_at(PathLocation::new(0, 0.25).expect("location"))
        .expect("transformed normal");
    let expected_normal = transform.apply_vector(source_normal);
    assert!((transformed_normal.x - expected_normal.x).abs() < 2.0e-9);
    assert!((transformed_normal.y - expected_normal.y).abs() < 2.0e-9);
    let transformed_bounds = transformed.bounds().expect("transformed bounds");
    for sample in 0..=32 {
        assert!(
            transformed_bounds.contains(
                transformed
                    .point_at(PathLocation::new(0, sample as f64 / 32.0).expect("location"))
                    .expect("point")
            )
        );
    }
    let probe = CurvePath::line(Point2::new(-1.0, 3.0), Point2::new(11.0, 3.0)).expect("probe");
    let transformed_probe = probe.transformed(transform).expect("transformed probe");
    let source_contacts = path.intersections(&probe).expect("source intersections");
    let transformed_contacts = transformed
        .intersections(&transformed_probe)
        .expect("transformed intersections");
    assert_eq!(source_contacts.len(), transformed_contacts.len());
    for (source, transformed_contact) in source_contacts.iter().zip(&transformed_contacts) {
        assert_eq!(source.kind(), transformed_contact.kind());
        assert_eq!(
            source.first_location().segment_index(),
            transformed_contact.first_location().segment_index()
        );
        assert!(
            (source.first_location().parameter()
                - transformed_contact.first_location().parameter())
            .abs()
                <= 1.0e-12
        );
        assert_eq!(
            source.second_location().segment_index(),
            transformed_contact.second_location().segment_index()
        );
        assert!(
            (source.second_location().parameter()
                - transformed_contact.second_location().parameter())
            .abs()
                <= 1.0e-12
        );
        assert!(close(
            transform.apply_point(source.point()),
            transformed_contact.point()
        ));
    }
}

/// Proves stable validation paths reject malformed construction, locations, transforms, and bounds atomically.
#[test]
fn curve_failures_use_stable_paths_and_never_return_partial_results() {
    assert_eq!(
        LineSegment::new(Point2::new(f64::NAN, 0.0), Point2::new(0.0, 0.0))
            .expect_err("nonfinite coordinate")
            .path(),
        "curve.segment.coordinates"
    );
    assert_eq!(
        CurvePath::polyline(vec![Point2::new(0.0, 0.0)], PathClosure::Open)
            .expect_err("too few vertices")
            .path(),
        "curve.path.polyline.vertices"
    );
    let path = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(1.0, 1.0)).expect("line path");
    assert_eq!(
        path.point_at(PathLocation::new(2, 0.0).expect("well-formed location"))
            .expect_err("out of range segment")
            .path(),
        "curve.path.location.segment"
    );
    assert_eq!(
        path.clip_to_bounds(Bounds {
            min: Point2::new(2.0, 0.0),
            max: Point2::new(1.0, 1.0),
        })
        .expect_err("mutated invalid bounds")
        .path(),
        "curve.path.clipping.bounds"
    );
    assert_eq!(
        CurvePath::new(Vec::new(), PathClosure::Open)
            .expect_err("empty path")
            .message(),
        "curve paths require at least one segment"
    );
    assert_eq!(
        CurvePath::new(Vec::new(), PathClosure::Open)
            .expect_err("empty path")
            .path(),
        "curve.path.segments.empty"
    );
    assert_eq!(
        PathLocation::new(0, 2.0)
            .expect_err("invalid parameter")
            .path(),
        "curve.path.location.parameter"
    );
    let duplicate = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
        ],
        PathClosure::Open,
    )
    .expect("duplicate vertices are topology");
    assert_eq!(duplicate.segments().len(), 2);
    assert_eq!(
        duplicate
            .measure_arc_length()
            .expect("zero edge measurable")
            .total_length(),
        1.0
    );
}
