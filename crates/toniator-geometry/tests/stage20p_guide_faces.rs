//! Public Stage 20P guide-arrangement integration witnesses.

use std::cell::Cell;

use toniator_domain::{
    GuideDimensionId, PatternMechanismId, PatternOutputLayerId, RegionResizeAlgorithm,
};
use toniator_geometry::{
    Bounds, CanonicalRegionSourceId, CubicBezierSegment, CurvePath, CurveSegment, GuideFaceLimits,
    GuideFaceRequest, GuideFaceResult, PathClosure, Point2, RegionTreatment, RegionTreatmentLimits,
    RegionTreatmentRequest, StructuralPathInstance, StructuralPathInstanceId, StructuralPathSet,
    StructuralPathSourceId, build_guide_faces_cancellable, treat_region_requests_cancellable,
};

/// Creates a deterministic ordered guide set for public guide-face integration tests.
fn paths(values: Vec<(u64, CurvePath)>) -> StructuralPathSet {
    StructuralPathSet::new(
        "stage20p-integration".into(),
        PatternMechanismId(7),
        values
            .into_iter()
            .enumerate()
            .map(|(ordinal, (dimension, path))| StructuralPathInstance {
                id: StructuralPathInstanceId {
                    source: StructuralPathSourceId::GuideDimension(GuideDimensionId(dimension)),
                    repetition_index: ordinal as i64,
                    component_ordinal: 0,
                },
                source_structure_id: None,
                path,
            })
            .collect(),
    )
    .expect("ordered finite guides")
}

/// Builds a public request with fixed output ownership and canvas relevance.
fn request(dimensions: Vec<GuideDimensionId>, paths: StructuralPathSet) -> GuideFaceRequest {
    GuideFaceRequest {
        output_layer_id: PatternOutputLayerId(9),
        guide_mechanism_id: PatternMechanismId(7),
        dimensions,
        paths,
        canvas: Bounds::new(Point2::new(-1.0, -1.0), Point2::new(1.0, 1.0)).expect("finite canvas"),
    }
}

/// Asserts that a direct 0/60/120 witness contains only complete equilateral guide triangles.
///
/// # Panics
///
/// Panics when a retained canonical face has a non-line edge, is not a triangle, is not
/// equal-sided within the construction tolerance, or does not retain all three guide sources.
fn assert_equilateral_triangular_faces(output: &GuideFaceResult) {
    const TOLERANCE: f64 = 1.0e-8;
    assert!(
        !output.regions.regions().is_empty(),
        "triangular faces remain retained"
    );
    for region in output.regions.regions() {
        assert_eq!(region.ring.segments().len(), 3);
        assert!(
            region
                .ring
                .segments()
                .iter()
                .all(|segment| matches!(segment, CurveSegment::Line(_)))
        );
        let lengths: Vec<_> = region
            .ring
            .segments()
            .iter()
            .map(|segment| {
                let start = segment.start();
                let end = segment.end();
                (end.x - start.x).hypot(end.y - start.y)
            })
            .collect();
        assert!(
            lengths
                .iter()
                .all(|length| (*length - lengths[0]).abs() <= TOLERANCE)
        );
        let CanonicalRegionSourceId::GuideBoundary(sources) = &region.id.source_id else {
            panic!("Guide Faces preserve guide-boundary provenance");
        };
        let dimensions: std::collections::BTreeSet<_> = sources
            .iter()
            .filter_map(|source| match source.path.source {
                StructuralPathSourceId::GuideDimension(id) => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(dimensions.len(), 3);
    }
}

/// Proves selected two-guide and 0/60/120 three-guide arrangements both reach canonical regions.
#[test]
fn rectangular_and_phase_aligned_triangular_arrangements_are_canonical() {
    let line = |a, b| CurvePath::line(a, b).expect("finite line");
    for (dimensions, values) in [
        (
            vec![GuideDimensionId(1), GuideDimensionId(2)],
            vec![
                (1, line(Point2::new(-2.0, -5.0), Point2::new(-2.0, 5.0))),
                (1, line(Point2::new(2.0, -5.0), Point2::new(2.0, 5.0))),
                (2, line(Point2::new(-5.0, -2.0), Point2::new(5.0, -2.0))),
                (2, line(Point2::new(-5.0, 2.0), Point2::new(5.0, 2.0))),
            ],
        ),
        (
            vec![
                GuideDimensionId(1),
                GuideDimensionId(2),
                GuideDimensionId(3),
            ],
            vec![
                (1, line(Point2::new(-8.0, -2.0), Point2::new(8.0, -2.0))),
                (1, line(Point2::new(-8.0, -1.0), Point2::new(8.0, -1.0))),
                (1, line(Point2::new(-8.0, 0.0), Point2::new(8.0, 0.0))),
                (1, line(Point2::new(-8.0, 1.0), Point2::new(8.0, 1.0))),
                (1, line(Point2::new(-8.0, 2.0), Point2::new(8.0, 2.0))),
                (
                    2,
                    line(
                        Point2::new(-8.0, -17.856406460551018),
                        Point2::new(8.0, 9.856406460551018),
                    ),
                ),
                (
                    2,
                    line(
                        Point2::new(-8.0, -15.856406460551018),
                        Point2::new(8.0, 11.856406460551018),
                    ),
                ),
                (
                    2,
                    line(
                        Point2::new(-8.0, -13.856406460551018),
                        Point2::new(8.0, 13.856406460551018),
                    ),
                ),
                (
                    2,
                    line(
                        Point2::new(-8.0, -11.856406460551018),
                        Point2::new(8.0, 15.856406460551018),
                    ),
                ),
                (
                    2,
                    line(
                        Point2::new(-8.0, -9.856406460551018),
                        Point2::new(8.0, 17.856406460551018),
                    ),
                ),
                (
                    3,
                    line(
                        Point2::new(-8.0, 9.856406460551018),
                        Point2::new(8.0, -17.856406460551018),
                    ),
                ),
                (
                    3,
                    line(
                        Point2::new(-8.0, 11.856406460551018),
                        Point2::new(8.0, -15.856406460551018),
                    ),
                ),
                (
                    3,
                    line(
                        Point2::new(-8.0, 13.856406460551018),
                        Point2::new(8.0, -13.856406460551018),
                    ),
                ),
                (
                    3,
                    line(
                        Point2::new(-8.0, 15.856406460551018),
                        Point2::new(8.0, -11.856406460551018),
                    ),
                ),
                (
                    3,
                    line(
                        Point2::new(-8.0, 17.856406460551018),
                        Point2::new(8.0, -9.856406460551018),
                    ),
                ),
            ],
        ),
    ] {
        let triangular = dimensions.len() == 3;
        let output = build_guide_faces_cancellable(
            request(dimensions, paths(values)),
            GuideFaceLimits::default(),
            || false,
        )
        .expect("guide arrangement builds");
        assert!(!output.regions.regions().is_empty());
        assert_eq!(output.centroids.len(), output.regions.regions().len());
        assert!(
            output
                .centroids
                .iter()
                .all(|(_, centroid)| centroid.is_finite()),
            "analytic area centroids remain finite and region keyed",
        );
        if triangular {
            assert_equilateral_triangular_faces(&output);
        }
    }
}

/// Proves a current rectangular Guide Face uses UniformOffset fill two to double geometric radius.
#[test]
fn guide_face_uniform_offset_fill_two_targets_four_times_area() {
    let output =
        build_guide_faces_cancellable(rectangular_request(), GuideFaceLimits::default(), || false)
            .expect("rectangular Guide Faces build");
    let base = &output.regions.regions()[0];
    let resized = treat_region_requests_cancellable(
        PatternOutputLayerId(9),
        &output.regions,
        &[RegionTreatmentRequest {
            base_region_id: base.id.clone(),
            reference: None,
            treatment: Some(RegionTreatment {
                algorithm: RegionResizeAlgorithm::UniformOffset,
                fill: 2.0,
            }),
        }],
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("Guide Face uniform offset resolves");
    assert!((resized.regions.regions()[0].area - base.area * 4.0).abs() < 1e-6);
}

/// Proves a non-straight cubic guide boundary remains present in a canonical closed face.
#[test]
fn authored_cubic_boundary_is_not_flattened_or_dropped() {
    let cubic = |start, first, second, end| {
        CurvePath::new(
            vec![CurveSegment::CubicBezier(
                CubicBezierSegment::new(start, first, second, end).expect("finite cubic"),
            )],
            PathClosure::Open,
        )
        .expect("open cubic")
    };
    let line = |a, b| CurvePath::line(a, b).expect("finite line");
    let output = build_guide_faces_cancellable(
        request(
            vec![GuideDimensionId(1), GuideDimensionId(2)],
            paths(vec![
                (
                    1,
                    cubic(
                        Point2::new(-5.0, -2.0),
                        Point2::new(-2.0, -3.0),
                        Point2::new(2.0, -3.0),
                        Point2::new(5.0, -2.0),
                    ),
                ),
                (
                    1,
                    cubic(
                        Point2::new(-5.0, 2.0),
                        Point2::new(-2.0, 3.0),
                        Point2::new(2.0, 3.0),
                        Point2::new(5.0, 2.0),
                    ),
                ),
                (2, line(Point2::new(-2.0, -5.0), Point2::new(-2.0, 5.0))),
                (2, line(Point2::new(2.0, -5.0), Point2::new(2.0, 5.0))),
            ]),
        ),
        GuideFaceLimits::default(),
        || false,
    )
    .expect("cubic guide arrangement builds");
    assert!(output.regions.regions().iter().any(|region| {
        region
            .ring
            .segments()
            .iter()
            .any(|segment| matches!(segment, CurveSegment::CubicBezier(_)))
    }));
    assert!(
        output
            .centroids
            .iter()
            .all(|(_, centroid)| centroid.is_finite()),
        "cubic Green-moment centroids remain finite",
    );
    let requests = output
        .regions
        .regions()
        .iter()
        .map(|region| RegionTreatmentRequest {
            base_region_id: region.id.clone(),
            reference: None,
            treatment: Some(RegionTreatment {
                algorithm: RegionResizeAlgorithm::UniformOffset,
                fill: 1.25,
            }),
        })
        .collect::<Vec<_>>();
    let resized = treat_region_requests_cancellable(
        PatternOutputLayerId(9),
        &output.regions,
        &requests,
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("current authored cubic Guide Faces use the bounded UniformOffset fallback");
    assert!(!resized.regions.regions().is_empty());
    let base_area: f64 = output
        .regions
        .regions()
        .iter()
        .map(|region| region.area)
        .sum();
    let resized_area: f64 = resized
        .regions
        .regions()
        .iter()
        .map(|region| region.area)
        .sum();
    assert!(
        (resized_area - base_area * 1.25 * 1.25).abs() <= base_area * 1.25 * 1.25 * 1.0e-6 + 1.0e-9
    );
}

/// Proves cancellation and configured work limits reject atomically before any partial region set escapes.
#[test]
fn guide_face_cancellation_and_limits_are_stable() {
    let line = |a, b| CurvePath::line(a, b).expect("finite line");
    let input = request(
        vec![GuideDimensionId(1), GuideDimensionId(2)],
        paths(vec![
            (1, line(Point2::new(-2.0, -5.0), Point2::new(-2.0, 5.0))),
            (1, line(Point2::new(2.0, -5.0), Point2::new(2.0, 5.0))),
            (2, line(Point2::new(-5.0, -2.0), Point2::new(5.0, -2.0))),
            (2, line(Point2::new(-5.0, 2.0), Point2::new(5.0, 2.0))),
        ]),
    );
    assert_eq!(
        build_guide_faces_cancellable(
            input.clone(),
            GuideFaceLimits {
                max_source_paths: 1,
                ..GuideFaceLimits::default()
            },
            || false
        )
        .expect_err("source limit rejects")
        .path(),
        "region.guide_faces.limits.source_paths",
    );
    assert_eq!(
        build_guide_faces_cancellable(input, GuideFaceLimits::default(), || true)
            .expect_err("cancellation rejects")
            .path(),
        "evaluation.cancelled",
    );
    let polls = Cell::new(0usize);
    assert_eq!(
        build_guide_faces_cancellable(rectangular_request(), GuideFaceLimits::default(), || {
            let count = polls.get();
            polls.set(count + 1);
            count >= 2
        },)
        .expect_err("cancellation after intersection work rejects atomically")
        .path(),
        "evaluation.cancelled",
    );
    assert!(polls.get() >= 3, "cancellation is polled after validation");
}

/// Proves cancellation is polled through later arrangement, canonical-handoff, and centroid work rather than only input intersections.
#[test]
fn guide_face_cancellation_reaches_late_work_phases() {
    let total_polls = Cell::new(0usize);
    build_guide_faces_cancellable(rectangular_request(), GuideFaceLimits::default(), || {
        total_polls.set(total_polls.get() + 1);
        false
    })
    .expect("baseline arrangement completes");
    assert!(total_polls.get() > 12, "fixture reaches late build phases");
    for threshold in [total_polls.get() / 2, total_polls.get() - 1] {
        let polls = Cell::new(0usize);
        assert_eq!(
            build_guide_faces_cancellable(
                rectangular_request(),
                GuideFaceLimits::default(),
                || {
                    let current = polls.get();
                    polls.set(current + 1);
                    current >= threshold
                },
            )
            .expect_err("late cancellation rejects without publication")
            .path(),
            "evaluation.cancelled",
        );
        assert!(polls.get() > threshold, "requested late poll was reached");
    }
}

/// Proves source-order, missing-source, tangency, and overlap diagnostics reject ambiguous arrangement authority.
#[test]
fn guide_face_identity_and_contact_diagnostics_are_stable() {
    let line = |a, b| CurvePath::line(a, b).expect("finite line");
    let ordered = paths(vec![
        (1, line(Point2::new(-2.0, -5.0), Point2::new(-2.0, 5.0))),
        (2, line(Point2::new(-5.0, -2.0), Point2::new(5.0, -2.0))),
    ]);
    assert_eq!(
        build_guide_faces_cancellable(
            request(
                vec![GuideDimensionId(2), GuideDimensionId(1)],
                ordered.clone()
            ),
            GuideFaceLimits::default(),
            || false,
        )
        .expect_err("reordered dimensions reject")
        .path(),
        "region.guide_faces.identity.dimension_order",
    );
    assert_eq!(
        build_guide_faces_cancellable(
            request(vec![GuideDimensionId(1), GuideDimensionId(3)], ordered),
            GuideFaceLimits::default(),
            || false,
        )
        .expect_err("missing dimension paths reject")
        .path(),
        "region.guide_faces.identity.dimension_paths",
    );
    let tangent = paths(vec![
        (1, line(Point2::new(0.0, -5.0), Point2::new(0.0, 5.0))),
        (2, line(Point2::new(0.0, 0.0), Point2::new(5.0, 0.0))),
    ]);
    assert_eq!(
        build_guide_faces_cancellable(
            request(vec![GuideDimensionId(1), GuideDimensionId(2)], tangent),
            GuideFaceLimits::default(),
            || false,
        )
        .expect_err("tangent rejects")
        .path(),
        "region.guide_faces.geometry.tangency",
    );
    let overlap = paths(vec![
        (1, line(Point2::new(-5.0, 0.0), Point2::new(5.0, 0.0))),
        (2, line(Point2::new(-3.0, 0.0), Point2::new(3.0, 0.0))),
    ]);
    assert_eq!(
        build_guide_faces_cancellable(
            request(vec![GuideDimensionId(1), GuideDimensionId(2)], overlap),
            GuideFaceLimits::default(),
            || false,
        )
        .expect_err("overlap rejects")
        .path(),
        "region.guide_faces.geometry.overlap",
    );
}

/// Proves each reachable arrangement resource category has an independent stable limit diagnostic.
#[test]
fn guide_face_runtime_limit_categories_are_independent() {
    let line = |a, b| CurvePath::line(a, b).expect("finite line");
    let rectangular = request(
        vec![GuideDimensionId(1), GuideDimensionId(2)],
        paths(vec![
            (1, line(Point2::new(-2.0, -5.0), Point2::new(-2.0, 5.0))),
            (1, line(Point2::new(2.0, -5.0), Point2::new(2.0, 5.0))),
            (2, line(Point2::new(-5.0, -2.0), Point2::new(5.0, -2.0))),
            (2, line(Point2::new(-5.0, 2.0), Point2::new(5.0, 2.0))),
        ]),
    );
    for (limits, path) in [
        (
            GuideFaceLimits {
                max_source_segments: 1,
                ..GuideFaceLimits::default()
            },
            "region.guide_faces.limits.source_segments",
        ),
        (
            GuideFaceLimits {
                max_intersection_contacts: 1,
                ..GuideFaceLimits::default()
            },
            "region.guide_faces.limits.intersection_contacts",
        ),
        (
            GuideFaceLimits {
                max_split_segments: 1,
                ..GuideFaceLimits::default()
            },
            "region.guide_faces.limits.split_segments",
        ),
        (
            GuideFaceLimits {
                max_vertices: 1,
                ..GuideFaceLimits::default()
            },
            "region.guide_faces.limits.vertices",
        ),
        (
            GuideFaceLimits {
                max_half_edges: 1,
                ..GuideFaceLimits::default()
            },
            "region.guide_faces.limits.half_edges",
        ),
        (
            GuideFaceLimits {
                max_inspections: 1,
                ..GuideFaceLimits::default()
            },
            "region.guide_faces.limits.inspections",
        ),
    ] {
        assert_eq!(
            build_guide_faces_cancellable(rectangular.clone(), limits, || false)
                .expect_err("configured limit rejects")
                .path(),
            path,
        );
    }
    assert_eq!(
        build_guide_faces_cancellable(
            rectangular,
            GuideFaceLimits {
                max_ring_segments: 1,
                ..GuideFaceLimits::default()
            },
            || false,
        )
        .expect_err("ring limit rejects")
        .path(),
        "region.guide_faces.limits.ring_segments",
    );
    let triangular = request(
        vec![
            GuideDimensionId(1),
            GuideDimensionId(2),
            GuideDimensionId(3),
        ],
        paths(vec![
            (1, line(Point2::new(-8.0, -2.0), Point2::new(8.0, -2.0))),
            (1, line(Point2::new(-8.0, -1.0), Point2::new(8.0, -1.0))),
            (1, line(Point2::new(-8.0, 0.0), Point2::new(8.0, 0.0))),
            (1, line(Point2::new(-8.0, 1.0), Point2::new(8.0, 1.0))),
            (1, line(Point2::new(-8.0, 2.0), Point2::new(8.0, 2.0))),
            (
                2,
                line(
                    Point2::new(-8.0, -15.856406460551018),
                    Point2::new(8.0, 11.856406460551018),
                ),
            ),
            (
                2,
                line(
                    Point2::new(-8.0, -13.856406460551018),
                    Point2::new(8.0, 13.856406460551018),
                ),
            ),
            (
                2,
                line(
                    Point2::new(-8.0, -11.856406460551018),
                    Point2::new(8.0, 15.856406460551018),
                ),
            ),
            (
                3,
                line(
                    Point2::new(-8.0, 11.856406460551018),
                    Point2::new(8.0, -15.856406460551018),
                ),
            ),
            (
                3,
                line(
                    Point2::new(-8.0, 13.856406460551018),
                    Point2::new(8.0, -13.856406460551018),
                ),
            ),
            (
                3,
                line(
                    Point2::new(-8.0, 15.856406460551018),
                    Point2::new(8.0, -11.856406460551018),
                ),
            ),
        ]),
    );
    assert!(
        build_guide_faces_cancellable(triangular.clone(), GuideFaceLimits::default(), || false)
            .expect("triangular faces")
            .regions
            .regions()
            .len()
            > 1
    );
    assert_eq!(
        build_guide_faces_cancellable(
            triangular,
            GuideFaceLimits {
                max_faces: 1,
                ..GuideFaceLimits::default()
            },
            || false,
        )
        .expect_err("face limit rejects")
        .path(),
        "region.guide_faces.limits.faces",
    );
}

/// Builds the reusable two-guide rectangle that forces several post-validation arrangement phases.
fn rectangular_request() -> GuideFaceRequest {
    let line = |a, b| CurvePath::line(a, b).expect("finite line");
    request(
        vec![GuideDimensionId(1), GuideDimensionId(2)],
        paths(vec![
            (1, line(Point2::new(-2.0, -5.0), Point2::new(-2.0, 5.0))),
            (1, line(Point2::new(2.0, -5.0), Point2::new(2.0, 5.0))),
            (2, line(Point2::new(-5.0, -2.0), Point2::new(5.0, -2.0))),
            (2, line(Point2::new(-5.0, 2.0), Point2::new(5.0, 2.0))),
        ]),
    )
}

/// Proves a complete face containing the canvas remains relevant without adding canvas topology.
#[test]
fn guide_face_containing_canvas_is_retained_by_exact_relevance() {
    let line = |a, b| CurvePath::line(a, b).expect("finite line");
    let output = build_guide_faces_cancellable(
        request(
            vec![GuideDimensionId(1), GuideDimensionId(2)],
            paths(vec![
                (1, line(Point2::new(-5.0, -8.0), Point2::new(-5.0, 8.0))),
                (1, line(Point2::new(5.0, -8.0), Point2::new(5.0, 8.0))),
                (2, line(Point2::new(-8.0, -5.0), Point2::new(8.0, -5.0))),
                (2, line(Point2::new(-8.0, 5.0), Point2::new(8.0, 5.0))),
            ]),
        ),
        GuideFaceLimits::default(),
        || false,
    )
    .expect("containing face remains relevant");
    assert_eq!(output.regions.regions().len(), 1);
}

/// Proves an unselected family dimension cannot consume the selected Guide Faces source-path budget.
#[test]
fn guide_face_source_path_limit_counts_only_selected_dimensions() {
    let line = |a, b| CurvePath::line(a, b).expect("finite line");
    let output = build_guide_faces_cancellable(
        request(
            vec![GuideDimensionId(1), GuideDimensionId(2)],
            paths(vec![
                (1, line(Point2::new(-2.0, -5.0), Point2::new(-2.0, 5.0))),
                (1, line(Point2::new(2.0, -5.0), Point2::new(2.0, 5.0))),
                (2, line(Point2::new(-5.0, -2.0), Point2::new(5.0, -2.0))),
                (2, line(Point2::new(-5.0, 2.0), Point2::new(5.0, 2.0))),
                (3, line(Point2::new(-8.0, 0.0), Point2::new(8.0, 0.0))),
            ]),
        ),
        GuideFaceLimits {
            max_source_paths: 4,
            ..GuideFaceLimits::default()
        },
        || false,
    )
    .expect("unselected guide paths do not count against the selected budget");
    assert_eq!(output.regions.regions().len(), 1);
}

/// Proves a complete face sharing a canvas boundary remains relevant without treating the canvas as guide topology.
#[test]
fn guide_face_aligned_to_canvas_boundary_is_retained() {
    let line = |a, b| CurvePath::line(a, b).expect("finite line");
    let output = build_guide_faces_cancellable(
        GuideFaceRequest {
            output_layer_id: PatternOutputLayerId(9),
            guide_mechanism_id: PatternMechanismId(7),
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
            paths: paths(vec![
                (1, line(Point2::new(-2.0, -5.0), Point2::new(-2.0, 5.0))),
                (1, line(Point2::new(2.0, -5.0), Point2::new(2.0, 5.0))),
                (2, line(Point2::new(-5.0, 0.0), Point2::new(5.0, 0.0))),
                (2, line(Point2::new(-5.0, 2.0), Point2::new(5.0, 2.0))),
            ]),
            canvas: Bounds::new(Point2::new(0.0, 0.0), Point2::new(1.0, 1.0))
                .expect("finite canvas"),
        },
        GuideFaceLimits::default(),
        || false,
    )
    .expect("canvas-aligned guide edge is relevance, not an overlap failure");
    assert_eq!(output.regions.regions().len(), 1);
}

/// Proves Guide Faces reject closed structural components before arrangement construction.
#[test]
fn guide_face_rejects_selected_closed_paths() {
    let points = [
        Point2::new(-2.0, -2.0),
        Point2::new(2.0, -2.0),
        Point2::new(0.0, 2.0),
    ];
    let closed = CurvePath::new(
        (0..points.len())
            .map(|index| {
                CurveSegment::Line(
                    toniator_geometry::LineSegment::new(
                        points[index],
                        points[(index + 1) % points.len()],
                    )
                    .expect("finite closed edge"),
                )
            })
            .collect(),
        PathClosure::Closed,
    )
    .expect("closed path");
    let line = CurvePath::line(Point2::new(-5.0, 0.0), Point2::new(5.0, 0.0)).expect("finite line");
    let error = build_guide_faces_cancellable(
        request(
            vec![GuideDimensionId(1), GuideDimensionId(2)],
            paths(vec![(1, closed), (2, line)]),
        ),
        GuideFaceLimits::default(),
        || false,
    )
    .expect_err("closed selected guide rejects");
    assert_eq!(error.path(), "region.guide_faces.geometry.path");
}
