//! Requested Maze construction retains complete guard topology rather than a canvas boundary.
use toniator_domain::*;
use toniator_patterns::*;

/// Builds a current two/three-dimension intersection family without changing mark normalization.
fn definition(angles: &[f64]) -> PatternDefinition {
    PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(1),
        "Maze coverage",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        angles
            .iter()
            .enumerate()
            .map(|(index, angle)| StraightGuideDimension {
                id: GuideDimensionId(index as u64 + 1),
                baseline_angle_degrees: *angle,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            })
            .collect(),
        GeneralizedSiteProduct::Intersections {
            dimensions: (1..=angles.len() as u64).map(GuideDimensionId).collect(),
            merge_epsilon: 1e-9,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    )
}

/// Supplies the precise thin-fringe report with ten-unit pitch and translated finite grid phase.
fn request() -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        density: ResolvedDensityMetric2D {
            across_x: 10.0,
            across_y: 10.0,
        },
        rotation_degrees: 0.0,
        translation_x: 3.0,
        translation_y: 0.0,
        guard_steps: 2,
        support_radius: 10.0,
        max_family_candidates: 100_000,
    }
}

/// Tests convex finite cells geometrically; wall opacity is not a coverage witness.
fn contains_cell(vertices: &[toniator_geometry::Point2], point: toniator_geometry::Point2) -> bool {
    let crosses = vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
        .map(|(a, b)| (b.x - a.x) * (point.y - a.y) - (b.y - a.y) * (point.x - a.x))
        .collect::<Vec<_>>();
    crosses.iter().all(|v| *v >= -1e-7) || crosses.iter().all(|v| *v <= 1e-7)
}

/// Requires complete bounded cells over the final canvas and a genuine connected finite puzzle.
/// # Panics
/// Panics if a sampled canvas point falls outside all cells or finite Maze invariants fail.
fn assert_complete(maze: &MazeProgramResult, req: &GridInspectRequest) {
    for y in 0..=4 {
        for x in 0..=4 {
            let point = toniator_geometry::Point2::new(
                req.canvas.width * f64::from(x) / 4.0,
                req.canvas.height * f64::from(y) / 4.0,
            );
            assert!(
                maze.cells.iter().any(|cell| contains_cell(
                    &cell
                        .vertices
                        .iter()
                        .map(|id| maze.source_site_positions[id])
                        .collect::<Vec<_>>(),
                    point
                )),
                "missing bounded-cell coverage at {point:?} for {req:?}"
            );
        }
    }
    assert!(!maze.cells.is_empty());
    assert_eq!(maze.removed_passage_walls.len() + 1, maze.cells.len());
    assert_ne!(maze.entrance.wall, maze.exit.wall);
    assert!(!maze.solution.cells.is_empty());
    assert!(maze.perimeter_walls.contains(&maze.entrance.wall));
    assert!(maze.perimeter_walls.contains(&maze.exit.wall));
    let canvas = toniator_geometry::Bounds::new(
        toniator_geometry::Point2::new(0.0, 0.0),
        toniator_geometry::Point2::new(req.canvas.width, req.canvas.height),
    )
    .unwrap();
    assert!(!canvas.contains(maze.entrance.point));
    assert!(!canvas.contains(maze.exit.point));
}

/// Corrects the recorded thin fringe and preserves exact requested topology under broader family reuse.
/// # Panics
/// Panics if square/triangular/skew cells miss canvas or broader guard support changes seeded output.
#[test]
fn requested_guard_maze_covers_canvas_and_matches_broader_family_reuse() {
    for (angles, width, height, aspect, rotation, x, y, guards) in [
        (vec![0.0, 90.0], 100.0, 100.0, 1.0, 0.0, 3.0, 0.0, 2),
        (
            vec![0.0, 60.0, 120.0],
            160.0,
            80.0,
            2.0,
            37.0,
            -240.0,
            160.0,
            1,
        ),
        (vec![0.0, 90.0], 80.0, 160.0, 0.5, -29.0, 160.0, -320.0, 0),
        (vec![0.0, 1.0], 80.0, 60.0, 1.0, 17.0, -96.0, 128.0, 0),
    ] {
        let definition = definition(&angles);
        let plan = resolve_pattern_pipeline(&definition).unwrap();
        let mut req = request();
        req.canvas = CanvasSpec { width, height };
        let metric = DensityMetric2D::from_resolved(
            &req.canvas,
            &ResolvedDensityMetric2D {
                across_x: width / 10.0,
                across_y: height / 10.0,
            },
        )
        .unwrap();
        req.density = DensityMetric2D {
            density: metric.density,
            aspect,
        }
        .resolve(&req.canvas)
        .unwrap();
        req.rotation_degrees = rotation;
        req.translation_x = x;
        req.translation_y = y;
        req.guard_steps = guards;
        req.max_family_candidates = 1_000_000;
        let reach = straight_topology_cell_reach(
            &plan.family,
            &req.canvas,
            &req.density,
            rotation,
            &plan.family.site_selection,
        )
        .unwrap();
        req.support_radius = req.support_radius.max(reach);
        let exact = evaluate_typed_family(&definition, &req).unwrap();
        let program = MazeProgram {
            algorithm: GridMazeAlgorithm::RecursiveBacktracker,
            seed: 31,
        };
        let maze = evaluate_typed_maze_walls_from_family_cancellable(
            &exact,
            &req,
            PatternOutputLayerId(1),
            &program,
            MazeLimits::default(),
            &|| false,
        )
        .unwrap();
        assert_complete(&maze, &req);
        let mut broad = req.clone();
        broad.support_radius += 30.0;
        let broader = evaluate_typed_family(&definition, &broad).unwrap();
        let reused = evaluate_typed_maze_walls_from_family_cancellable(
            &broader,
            &req,
            PatternOutputLayerId(1),
            &program,
            MazeLimits::default(),
            &|| false,
        )
        .unwrap();
        assert!(
            maze.source_site_positions == reused.source_site_positions,
            "source positions changed: {angles:?} {req:?}"
        );
        assert!(
            maze.source_walls == reused.source_walls,
            "source walls changed: {angles:?} {req:?}"
        );
        assert!(
            maze.cells == reused.cells,
            "cells changed: {angles:?} {req:?}"
        );
        assert!(
            maze.dual_edges == reused.dual_edges,
            "dual changed: {angles:?} {req:?}"
        );
        assert!(
            maze.perimeter_walls == reused.perimeter_walls,
            "perimeter changed: {angles:?} {req:?}"
        );
        assert!(
            maze.removed_passage_walls == reused.removed_passage_walls,
            "passages changed: {angles:?} {req:?}"
        );
        assert!(
            maze.entrance == reused.entrance && maze.exit == reused.exit,
            "openings changed: {angles:?} {req:?}"
        );
        assert!(
            maze.retained_walls == reused.retained_walls && maze.wall_paths == reused.wall_paths,
            "walls changed: {angles:?} {req:?}"
        );
        assert!(
            maze.solution == reused.solution,
            "solution changed: {angles:?} {req:?}"
        );
        assert_eq!(
            maze.fingerprint(),
            reused.fingerprint(),
            "fingerprint changed: {angles:?} {req:?}"
        );
        for (left, right) in maze.source_sites.iter().zip(&reused.source_sites) {
            assert_eq!(left.id, right.id);
            assert_eq!(left.source.position, right.source.position);
            assert_eq!(left.source.provenance, right.source.provenance);
            assert_eq!(
                left.source.nominal_cell_basis,
                right.source.nominal_cell_basis
            );
        }
        assert!(
            maze == reused,
            "exact regeneration changes published Maze result: {angles:?} {req:?}"
        );
        let accepted = broader
            .site_set()
            .iter()
            .filter_map(|site| match &site.provenance {
                toniator_geometry::FamilySiteProvenance::GuideIntersection { contributors } => {
                    Some((contributors, site.id))
                }
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        for site in &reused.source_sites {
            let toniator_geometry::FamilySiteProvenance::GuideIntersection { contributors } =
                &site.source.provenance
            else {
                panic!("Maze requires intersection provenance")
            };
            assert!(
                accepted.contains_key(contributors),
                "exact requested site has no accepted contributor identity: {contributors:?} {angles:?}"
            );
        }
        let usage = maze_site_usage_for_family_cancellable(&broader, &reused, &|| false).unwrap();
        let filtered = broader
            .filtered_for_output(
                SiteUseFilter::SitesUsedBy {
                    output_layer_id: PatternOutputLayerId(1),
                },
                Some(&usage),
            )
            .unwrap();
        assert_eq!(filtered.site_set().len(), usage.members().len());
        for site in filtered.site_set().iter() {
            let FamilySiteProvenance::GuideIntersection { contributors } = &site.provenance else {
                unreachable!()
            };
            assert!(reused.source_sites.iter().any(|candidate| matches!(&candidate.source.provenance,FamilySiteProvenance::GuideIntersection {contributors: keys} if keys == contributors)));
        }
        let white =
            toniator_sampling::SourceField::from_straight_rgba8(1, 1, vec![255, 255, 255, 255])
                .unwrap();
        let marks = realize_typed_diagnostic_outputs(
            &filtered,
            &plan,
            &white,
            &req.canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            MarkResponse {
                minimum_fill: 0.5,
                maximum_fill: 0.5,
                rotation_offset_degrees: 0.0,
            },
        )
        .unwrap();
        assert_eq!(marks.output.marks.len(), usage.members().len());
        assert!(marks.output.marks.iter().all(|mark| {
            filtered
                .site_set()
                .iter()
                .any(|site| site.position == mark.center)
        }));
        let mut missing = reused.clone();
        let endpoint = missing.retained_walls[0].id.first;
        missing
            .source_sites
            .iter_mut()
            .find(|site| site.id == endpoint)
            .unwrap()
            .source
            .provenance = FamilySiteProvenance::GuideIntersection {
            contributors: vec![],
        };
        assert_eq!(
            maze_site_usage_for_family_cancellable(&broader, &missing, &|| false)
                .unwrap_err()
                .path(),
            "maze.site_usage.identity.missing"
        );
        assert_eq!(
            maze_site_usage_for_family_cancellable(&broader, &reused, &|| true)
                .unwrap_err()
                .path(),
            "evaluation.cancelled"
        );
        assert!(
            maze.source_sites
                .iter()
                .any(|site| site.source.position.x < 0.0)
        );
        assert!(
            maze.source_sites
                .iter()
                .any(|site| site.source.position.x > width)
        );
    }
}

/// Resolves true skew cell diameter from selected document metrics without changing nominal mark basis.
/// # Panics
/// Panics if aspect/angle reach is replaced by mark normalization or an unrelated guide alters selected reach.
#[test]
fn selected_straight_cell_reach_uses_reciprocal_document_lattice() {
    let req = request();
    let orthogonal = resolve_pattern_pipeline(&definition(&[0.0, 90.0])).unwrap();
    let reach = straight_topology_cell_reach(
        &orthogonal.family,
        &req.canvas,
        &req.density,
        0.0,
        &orthogonal.family.site_selection,
    )
    .unwrap();
    assert!((reach - 200.0_f64.sqrt()).abs() < 1e-10);
    let skew = resolve_pattern_pipeline(&definition(&[0.0, 1.0])).unwrap();
    let reach = straight_topology_cell_reach(
        &skew.family,
        &req.canvas,
        &req.density,
        37.0,
        &skew.family.site_selection,
    )
    .unwrap();
    assert!((reach - 10.0 / 0.5_f64.to_radians().sin()).abs() < 1e-7);
    assert!(reach > 1000.0);
    let triple = resolve_pattern_pipeline(&definition(&[0.0, 1.0, 90.0])).unwrap();
    let selected = [GuideDimensionId(1), GuideDimensionId(3)];
    assert!(
        (straight_topology_cell_reach(&triple.family, &req.canvas, &req.density, 0.0, &selected)
            .unwrap()
            - 200.0_f64.sqrt())
        .abs()
            < 1e-10
    );
    let four = resolve_pattern_pipeline(&definition(&[0.0, 1.0, 90.0, 135.0])).unwrap();
    assert!(
        (straight_topology_cell_reach(
            &four.family,
            &req.canvas,
            &req.density,
            0.0,
            &four.family.site_selection
        )
        .unwrap()
            - 200.0_f64.sqrt())
        .abs()
            < 1e-10
    );
    let parallel = resolve_pattern_pipeline(&definition(&[0.0, 1e-12])).unwrap();
    assert_eq!(
        straight_topology_cell_reach(
            &parallel.family,
            &req.canvas,
            &req.density,
            0.0,
            &parallel.family.site_selection
        )
        .unwrap_err()
        .path(),
        "coverage.straight_topology.cell_reach"
    );
}

/// Enforces unchanged family/Maze resource and cancellation bounds after complete support reservation.
/// # Panics
/// Panics if a limited or cancelled computation publishes partial topology.
#[test]
fn complete_maze_retains_candidate_limits_and_cancellation() {
    let definition = definition(&[0.0, 1.0]);
    let plan = resolve_pattern_pipeline(&definition).unwrap();
    let mut req = request();
    req.support_radius = straight_topology_cell_reach(
        &plan.family,
        &req.canvas,
        &req.density,
        0.0,
        &plan.family.site_selection,
    )
    .unwrap();
    req.max_family_candidates = 10;
    assert!(evaluate_typed_family(&definition, &req).is_err());
    req.max_family_candidates = 1_000_000;
    let family = evaluate_typed_family(&definition, &req).unwrap();
    let error = evaluate_typed_maze_walls_from_family_cancellable(
        &family,
        &req,
        PatternOutputLayerId(1),
        &MazeProgram {
            algorithm: GridMazeAlgorithm::RecursiveBacktracker,
            seed: 31,
        },
        MazeLimits::default(),
        &|| true,
    )
    .unwrap_err();
    assert_eq!(error.path(), "evaluation.cancelled");
    let limits = MazeLimits::new(2, 2, 2, 2, 2, 2, 2).unwrap();
    assert!(
        evaluate_typed_maze_walls_from_family_cancellable(
            &family,
            &req,
            PatternOutputLayerId(1),
            &MazeProgram {
                algorithm: GridMazeAlgorithm::RecursiveBacktracker,
                seed: 31
            },
            limits,
            &|| false
        )
        .is_err()
    );
}

/// Retains actual relevant canonical polygons independently of producer-local region identifiers.
/// # Panics
/// Panics if a straight cell boundary or canvas clip cannot be represented.
fn relevant_polygons(regions: &CanonicalRegionSet, canvas: Bounds) -> Vec<Vec<(u64, u64)>> {
    let mut values = regions
        .regions()
        .iter()
        .filter_map(|region| {
            let vertices = region
                .ring
                .segments()
                .iter()
                .map(CurveSegment::start)
                .collect::<Vec<_>>();
            let relevant = vertices.iter().any(|point| canvas.contains(*point))
                || canvas
                    .corners()
                    .into_iter()
                    .any(|point| contains_cell(&vertices, point))
                || !region.ring.clip_to_bounds(canvas).unwrap().is_empty();
            relevant.then(|| {
                let mut points = vertices
                    .iter()
                    .map(|point| (point.x.to_bits(), point.y.to_bits()))
                    .collect::<Vec<_>>();
                points.sort_unstable();
                points
            })
        })
        .collect::<Vec<_>>();
    values.sort_unstable();
    values
}

/// Compares legal one-degree unmerged Guide Faces against independently broader complete polygons.
/// # Panics
/// Panics if diameter support leaves relevant cells incomplete or merely reports bounded success.
#[test]
fn unmerged_one_degree_guide_faces_match_broader_complete_cell_polygons() {
    let mut definition = definition(&[0.0, 1.0]);
    for mechanism in &mut definition.mechanisms {
        if let PatternMechanism::SelectedGuideIntersections { merge_epsilon, .. } = mechanism {
            *merge_epsilon = 0.0;
        }
    }
    let plan = resolve_pattern_pipeline(&definition).unwrap();
    let mut req = request();
    req.canvas = CanvasSpec {
        width: 80.0,
        height: 60.0,
    };
    req.density = ResolvedDensityMetric2D {
        across_x: 8.0,
        across_y: 6.0,
    };
    req.guard_steps = 0;
    req.translation_x = 0.0;
    req.max_family_candidates = 1_000_000;
    req.support_radius = straight_topology_cell_reach(
        &plan.family,
        &req.canvas,
        &req.density,
        0.0,
        &plan.family.site_selection,
    )
    .unwrap();
    let canvas = Bounds::new(Point2::new(0.0, 0.0), Point2::new(80.0, 60.0)).unwrap();
    let build = |request: &GridInspectRequest| {
        let family = evaluate_typed_family(&definition, request).unwrap();
        build_guide_faces_cancellable(
            GuideFaceRequest {
                output_layer_id: PatternOutputLayerId(1),
                guide_mechanism_id: plan.family.provenance.mechanism_ids[0],
                dimensions: plan.family.site_selection.clone(),
                paths: family.structural_path_set().unwrap().clone(),
                canvas,
            },
            GuideFaceLimits::default(),
            || false,
        )
        .unwrap()
    };
    let complete = build(&req);
    let mut broad = req.clone();
    broad.support_radius *= 2.0;
    let reference = build(&broad);
    assert_eq!(
        relevant_polygons(&complete.regions, canvas),
        relevant_polygons(&reference.regions, canvas)
    );
    assert!(!relevant_polygons(&complete.regions, canvas).is_empty());
}

/// Preserves the current legal one-degree Voronoi before witness before any support-contract repair.
/// # Panics
/// Panics if either current minimal support or an independently broad lattice cannot be evaluated.
#[test]
fn insufficient_one_degree_voronoi_support_rejects_before_topology_publication() {
    let mut definition = definition(&[0.0, 1.0]);
    for mechanism in &mut definition.mechanisms {
        if let PatternMechanism::SelectedGuideIntersections { merge_epsilon, .. } = mechanism {
            *merge_epsilon = 0.0;
        }
    }
    let plan = resolve_pattern_pipeline(&definition).unwrap();
    let mut req = request();
    req.canvas = CanvasSpec {
        width: 80.0,
        height: 60.0,
    };
    req.density = ResolvedDensityMetric2D {
        across_x: 8.0,
        across_y: 6.0,
    };
    req.guard_steps = 2;
    req.support_radius = 0.0;
    req.translation_x = 0.0;
    req.max_family_candidates = 1_000_000;
    let canvas = Bounds::new(Point2::new(0.0, 0.0), Point2::new(80.0, 60.0)).unwrap();
    let family = evaluate_typed_family(&definition, &req).unwrap();
    let current = build_voronoi_regions_cancellable(
        family.site_set(),
        VoronoiRegionRequest {
            output_layer_id: PatternOutputLayerId(1),
            canvas,
        },
        VoronoiRegionLimits::default(),
        || false,
    );
    let reach = straight_topology_cell_reach(
        &plan.family,
        &req.canvas,
        &req.density,
        0.0,
        &plan.family.site_selection,
    )
    .unwrap();
    let mut broad = req.clone();
    broad.support_radius = 3.0 * reach;
    let reference_family = evaluate_typed_family(&definition, &broad).unwrap();
    let (reference, _) = build_voronoi_regions_cancellable(
        reference_family.site_set(),
        VoronoiRegionRequest {
            output_layer_id: PatternOutputLayerId(1),
            canvas,
        },
        VoronoiRegionLimits::default(),
        || false,
    )
    .unwrap();
    assert!(!reference.regions().is_empty());
    assert!(
        current.is_err(),
        "current narrow support unexpectedly succeeds; compare exact relevant polygons before concluding coverage"
    );
}

/// Keys relevant finite Voronoi cells by exact absolute guide contributors rather than envelope ordinals.
/// # Panics
/// Panics if a straight Voronoi owner loses its actual family-site provenance or canonical ring.
fn voronoi_cells_by_contributors(
    family: &TypedFamilyOutput,
    regions: &CanonicalRegionSet,
    canvas: Bounds,
) -> std::collections::BTreeMap<Vec<Vec<GuideInstanceId>>, Vec<Point2>> {
    regions
        .regions()
        .iter()
        .filter_map(|region| {
            let vertices = region
                .ring
                .segments()
                .iter()
                .map(CurveSegment::start)
                .collect::<Vec<_>>();
            if !vertices.iter().any(|point| canvas.contains(*point))
                && !canvas
                    .corners()
                    .into_iter()
                    .any(|point| contains_cell(&vertices, point))
                && region.ring.clip_to_bounds(canvas).unwrap().is_empty()
            {
                return None;
            }
            let CanonicalRegionSourceId::SiteOwners(owners) = &region.id.source_id else {
                panic!("Voronoi requires site owners")
            };
            let mut keys = owners
                .iter()
                .map(|id| {
                    let site = &family.site_set().sites()[id.ordinal];
                    assert_eq!(site.id, *id);
                    let FamilySiteProvenance::GuideIntersection { contributors } = &site.provenance
                    else {
                        panic!("straight intersections required")
                    };
                    contributors.clone()
                })
                .collect::<Vec<_>>();
            keys.sort();
            Some((keys, vertices))
        })
        .collect()
}

/// Compares unmerged same-owner/facet topology within existing curve policy and checks default-merge full cells.
/// Different positive-epsilon populations need not have the same greedy topology. Same requests remain exact.
/// # Panics
/// Panics if finite support misses actual complete cells or independent unmerged facets exceed fixed policy.
#[test]
fn one_degree_voronoi_neighbor_bound_preserves_full_cells_and_unmerged_facets() {
    for epsilon in [0.0, 1e-9] {
        let mut definition = definition(&[0.0, 1.0]);
        for mechanism in &mut definition.mechanisms {
            if let PatternMechanism::SelectedGuideIntersections { merge_epsilon, .. } = mechanism {
                *merge_epsilon = epsilon;
            }
        }
        let plan = resolve_pattern_pipeline(&definition).unwrap();
        let mut req = request();
        req.canvas = CanvasSpec {
            width: 80.0,
            height: 60.0,
        };
        req.density = ResolvedDensityMetric2D {
            across_x: 8.0,
            across_y: 6.0,
        };
        req.guard_steps = 2;
        req.translation_x = 0.0;
        req.max_family_candidates = 1_000_000;
        let diameter = straight_topology_cell_reach(
            &plan.family,
            &req.canvas,
            &req.density,
            0.0,
            &plan.family.site_selection,
        )
        .unwrap();
        req.support_radius = 1.5 * diameter + 3.0 * epsilon;
        let canvas = Bounds::new(Point2::new(0.0, 0.0), Point2::new(80.0, 60.0)).unwrap();
        let build = |request: &GridInspectRequest| {
            let family = evaluate_typed_family(&definition, request).unwrap();
            let regions = build_voronoi_regions_cancellable(
                family.site_set(),
                VoronoiRegionRequest {
                    output_layer_id: PatternOutputLayerId(1),
                    canvas,
                },
                VoronoiRegionLimits::default(),
                || false,
            )
            .unwrap()
            .0;
            (family, regions)
        };
        let (family, candidate) = build(&req);
        let mut broad = req.clone();
        broad.support_radius = 3.0 * diameter + 3.0 * epsilon;
        let (reference_family, reference) = build(&broad);
        let actual = voronoi_cells_by_contributors(&family, &candidate, canvas);
        let expected = voronoi_cells_by_contributors(&reference_family, &reference, canvas);
        assert!(!actual.is_empty());
        for cells in [&actual, &expected] {
            for y in [0.0, 30.0, 60.0] {
                for x in [0.0, 40.0, 80.0] {
                    assert!(
                        cells
                            .values()
                            .any(|vertices| contains_cell(vertices, Point2::new(x, y))),
                        "finite Voronoi cells miss({x},{y}),epsilon={epsilon}"
                    );
                }
            }
        }
        if epsilon == 0.0 {
            assert_eq!(
                actual.keys().collect::<Vec<_>>(),
                expected.keys().collect::<Vec<_>>()
            );
            for (owner, vertices) in &actual {
                let other = &expected[owner];
                assert_eq!(vertices.len(), other.len());
                // Existing canonical curve policy: curves/mod.rs ABSOLUTE_TOLERANCE plus
                // RELATIVE_TOLERANCE*coordinate scale. This comparison adds no product epsilon.
                let scale = vertices.iter().chain(other).fold(1.0_f64, |scale, point| {
                    scale.max(point.x.abs()).max(point.y.abs())
                });
                let bound = 1e-9 + 64.0 * f64::EPSILON * scale;
                assert!(
                    (0..other.len()).any(|shift| vertices.iter().enumerate().all(
                        |(index, point)| {
                            let reference = other[(index + shift) % other.len()];
                            (point.x - reference.x).hypot(point.y - reference.y) <= bound
                        }
                    )),
                    "same-owner cyclic facets exceed canonical policy"
                );
            }
        }
        let (same_family, same_regions) = build(&req);
        assert_eq!(family, same_family);
        assert_eq!(candidate, same_regions);
    }
}
