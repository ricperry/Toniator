use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use toniator_domain::{
    CanvasSpec, ConnectionAdjacencyIntent, ConnectionProgram, CoveragePolicy, CurveWinding,
    GeneralizedSiteProduct, GridMazeAlgorithm, GridSpanningTreeAlgorithm, GuideDimensionId,
    GuideRepetition, MarkOrientation, MarkPrototype, ParametricCurve, PathStrokeStyle,
    PatternDefinition, PatternDefinitionId, PatternFamily, PatternMechanism, PatternMechanismId,
    PatternModulation, PatternOutputLayer, PatternOutputLayerId, PatternOutputRealization,
    RandomSiteCharacter, ResolvedDensityMetric2D, SiteDensityModulation, SiteExclusionPolicy,
    SourceMapping, SourceMappingComponent, SpiralCurve, SpiralShape, StraightGuideDimension,
    StraightGuideRepetition,
};
use toniator_patterns::{
    ConnectionPathLimits, GridInspectRequest, SiteAdjacencyLimits, StrokeResponse,
    StructuralProductCapability, evaluate_typed_connection_paths_with_source_cancellable,
    realize_connection_canonical_strokes_cancellable, realize_maze_canonical_strokes_cancellable,
    realize_owned_connection_canonical_strokes_cancellable,
    realize_owned_maze_canonical_strokes_cancellable, resolve_pattern_pipeline,
};
use toniator_sampling::{SourceField, SourceFormatHint, decode_source};

/// Builds a bounded active-guard request whose support is the connected-stroke base support.
fn request() -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec {
            width: 80.0,
            height: 60.0,
        },
        density: ResolvedDensityMetric2D {
            across_x: 8.0,
            across_y: 6.0,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 1,
        support_radius: 2.0,
        max_family_candidates: 20_000,
    }
}

/// Builds the 900x620 24-across artifact-scale request without producing a validation artifact.
fn artifact_scale_request() -> GridInspectRequest {
    artifact_scale_request_for(900.0, 620.0)
}

/// Builds one intrinsic 24-across artifact request with aspect-locked physical spacing.
fn artifact_scale_request_for(width: f64, height: f64) -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec { width, height },
        density: ResolvedDensityMetric2D {
            across_x: 24.0,
            across_y: 24.0 * height / width,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 1,
        support_radius: 192.0,
        max_family_candidates: 1_048_576,
    }
}

/// Builds a typed straight-grid intersection family without presentation-driven routing.
fn grid() -> PatternDefinition {
    PatternDefinition::supported_straight_grid(
        PatternDefinitionId(1),
        "neutral grid name",
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternOutputLayerId(4),
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    )
}

/// Builds a phase-aligned three-direction intersection family with equal physical guide spacing.
fn triangular_grid() -> PatternDefinition {
    PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(50),
        "triangular connection lattice",
        PatternMechanismId(51),
        PatternMechanismId(52),
        PatternOutputLayerId(53),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(54),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(55),
                baseline_angle_degrees: 60.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(56),
                baseline_angle_degrees: 120.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![
                GuideDimensionId(54),
                GuideDimensionId(55),
                GuideDimensionId(56),
            ],
            merge_epsilon: 1e-6,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    )
}

/// Returns the deterministic recursive-backtracker intent for the conventional wall-maze output.
fn triangular_maze_program(seed: u32) -> toniator_domain::MazeProgram {
    toniator_domain::MazeProgram {
        algorithm: GridMazeAlgorithm::RecursiveBacktracker,
        seed,
    }
}

/// Builds an eligible generalized along-guide site family.
fn along() -> PatternDefinition {
    PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(10),
        "neutral along name",
        PatternMechanismId(11),
        PatternMechanismId(12),
        PatternOutputLayerId(13),
        vec![StraightGuideDimension {
            id: GuideDimensionId(14),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(14)],
            interval_multiplier: 0.75,
            phase: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    )
}

/// Builds an eligible random dispersion family.
fn random() -> PatternDefinition {
    PatternDefinition::random_sites(
        PatternDefinitionId(20),
        "neutral dispersion name",
        PatternMechanismId(21),
        PatternMechanismId(22),
        PatternMechanismId(23),
        PatternMechanismId(24),
        PatternOutputLayerId(25),
        RandomSiteCharacter::Even {
            minimum_center_distance: 8.0,
        },
        17,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        2_000,
        20_000,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    )
}

/// Builds a parametric equal-arc site family or its raw non-site counterpart.
fn parametric(sites: bool) -> PatternDefinition {
    let curve = PatternMechanismId(31);
    let site = PatternMechanismId(32);
    PatternDefinition {
        id: PatternDefinitionId(30),
        name: "neutral parametric name".into(),
        family: PatternFamily::ParametricCurve {
            curve_mechanism_id: curve,
            site_mechanism_id: sites.then_some(site),
        },
        mechanisms: if sites {
            vec![
                PatternMechanism::ParametricCurveSource {
                    id: curve,
                    curve: ParametricCurve::Spiral(SpiralCurve {
                        shape: SpiralShape::Round,
                        turns: 2.0,
                        radial_spacing: 18.0,
                        phase_degrees: 0.0,
                        winding: CurveWinding::CounterClockwise,
                    }),
                    repetition: GuideRepetition::Single,
                },
                PatternMechanism::AlongParametricCurveSites {
                    id: site,
                    curve_mechanism_id: curve,
                    interval: 10.0,
                    phase: 0.0,
                },
            ]
        } else {
            vec![PatternMechanism::ParametricCurveSource {
                id: curve,
                curve: ParametricCurve::Spiral(SpiralCurve {
                    shape: SpiralShape::Round,
                    turns: 2.0,
                    radial_spacing: 18.0,
                    phase_degrees: 0.0,
                    winding: CurveWinding::CounterClockwise,
                }),
                repetition: GuideRepetition::Single,
            }]
        },
        output_layers: if sites {
            vec![PatternOutputLayer::all(
                PatternOutputLayerId(33),
                PatternOutputRealization::MarkPrototype {
                    site_mechanism_id: site,
                    prototype: MarkPrototype::Circle,
                    orientation: MarkOrientation::Fixed,
                },
            )]
        } else {
            vec![PatternOutputLayer::all(
                PatternOutputLayerId(33),
                PatternOutputRealization::ParametricPaths {
                    curve_mechanism_id: curve,
                    style: PathStrokeStyle::default(),
                },
            )]
        },
        modulation: PatternModulation,
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    }
}

/// Returns one valid connection program for the supplied kind witness.
fn program(kind: usize) -> ConnectionProgram {
    let adjacency = ConnectionAdjacencyIntent {
        maximum_degree: 2,
        maximum_distance: 14.0,
    };
    match kind {
        0 => ConnectionProgram::NearestLinks { adjacency },
        1 => ConnectionProgram::RandomLinks {
            adjacency,
            minimum_degree: 1,
            seed: 7,
        },
        _ => ConnectionProgram::GridSpanningTree {
            adjacency,
            algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
            seed: 7,
        },
    }
}

/// Evaluates one connection family through the public typed capability boundary.
fn connections(
    definition: &PatternDefinition,
    program: &ConnectionProgram,
) -> Result<toniator_patterns::ConnectionPathEvaluation, toniator_patterns::PatternPipelineError> {
    let plan = resolve_pattern_pipeline(definition).expect("resolved family");
    evaluate_typed_connection_paths_with_source_cancellable(
        &plan.family,
        &request(),
        None,
        PatternOutputLayerId(40),
        program,
        SiteAdjacencyLimits::default(),
        ConnectionPathLimits::default(),
        &|| false,
    )
}

/// Builds one fully white finite source so every connection profile reaches its authored maximum.
fn white_source() -> SourceField {
    decode_source(
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="white"/></svg>"#,
        SourceFormatHint::Svg,
    )
    .expect("white source decodes")
}

/// Realizes one connection result through the public canonical-stroke boundary.
fn realize(
    paths: &toniator_patterns::ConnectionPathEvaluation,
    response: StrokeResponse,
    max_profile_samples: usize,
    max_outline_segments: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<toniator_patterns::CanonicalStrokeRealization, toniator_patterns::PatternPipelineError>
{
    realize_connection_canonical_strokes_cancellable(
        &paths.paths,
        &white_source(),
        &request().canvas,
        SourceMapping::canonical(SourceMappingComponent::Luminance),
        response,
        PathStrokeStyle::default(),
        max_profile_samples,
        max_outline_segments,
        is_cancelled,
    )
}

/// Accepts nearest/random programs for every typed site product, including equal-arc parametric sites.
#[test]
fn nearest_and_random_connections_follow_typed_site_capability() {
    for definition in [grid(), along(), random(), parametric(true)] {
        for kind in 0..2 {
            let result = connections(&definition, &program(kind)).expect("eligible connection");
            assert_eq!(result.family.planned_support_radius(), 16.0);
        }
    }
}

/// Restricts maze/tree programs to guide intersections before any family or graph allocation.
#[test]
fn maze_and_tree_reject_nonintersection_products_and_raw_paths_early() {
    for definition in [along(), random(), parametric(true), parametric(false)] {
        for kind in 2..4 {
            let error = connections(&definition, &program(kind)).expect_err("ineligible product");
            assert_eq!(error.path(), "connection.family.product");
        }
    }
    let raw = resolve_pattern_pipeline(&parametric(false)).expect("raw parametric plan");
    assert_eq!(
        raw.family.product,
        StructuralProductCapability::ParametricPaths
    );
    let error = evaluate_typed_connection_paths_with_source_cancellable(
        &raw.family,
        &request(),
        None,
        PatternOutputLayerId(40),
        &program(0),
        SiteAdjacencyLimits::default(),
        ConnectionPathLimits::default(),
        &|| true,
    );
    assert_eq!(
        error.expect_err("raw paths fail first").path(),
        "connection.family.product"
    );
}

/// Requires at least one guard step before active connection coverage or graph construction.
#[test]
fn connections_reject_guardless_coverage_before_graph_work() {
    let plan = resolve_pattern_pipeline(&grid()).expect("grid plan");
    let mut guardless = request();
    guardless.guard_steps = 0;
    let error = evaluate_typed_connection_paths_with_source_cancellable(
        &plan.family,
        &guardless,
        None,
        PatternOutputLayerId(40),
        &program(0),
        SiteAdjacencyLimits::default(),
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect_err("guardless connection rejects");
    assert_eq!(error.path(), "adjacency.coverage.guard_steps");
}

/// Rejects direct and accepted-family maze requests whose guard policy cannot retain a complete
/// outer cell ring, before arrangement topology allocates walls or faces.
#[test]
fn mazes_reject_zero_or_mismatched_guard_policy_before_topology_work() {
    let plan = resolve_pattern_pipeline(&triangular_grid()).expect("triangular family resolves");
    let mut guardless = request();
    guardless.guard_steps = 0;
    assert_eq!(
        toniator_patterns::evaluate_typed_maze_walls_cancellable(
            &plan.family,
            &guardless,
            PatternOutputLayerId(53),
            &triangular_maze_program(23),
            toniator_patterns::MazeLimits::default(),
            &|| false,
        )
        .expect_err("direct guardless maze rejects before family evaluation")
        .path(),
        "maze.coverage.guard_steps"
    );
    let accepted = toniator_patterns::evaluate_typed_family_product_cancellable(
        &plan.family,
        &request(),
        &|| false,
    )
    .expect("guarded family evaluates");
    let zero_guard_family = toniator_patterns::evaluate_typed_family_product_cancellable(
        &plan.family,
        &guardless,
        &|| false,
    )
    .expect("generic family construction remains available for an explicit rejection witness");
    assert_eq!(
        toniator_patterns::evaluate_typed_maze_walls_from_family_cancellable(
            &accepted,
            &guardless,
            PatternOutputLayerId(53),
            &triangular_maze_program(23),
            toniator_patterns::MazeLimits::default(),
            &|| false,
        )
        .expect_err("accepted family rejects a guardless request")
        .path(),
        "maze.coverage.guard_steps"
    );
    assert_eq!(
        toniator_patterns::evaluate_typed_maze_walls_from_family_cancellable(
            &zero_guard_family,
            &request(),
            PatternOutputLayerId(53),
            &triangular_maze_program(23),
            toniator_patterns::MazeLimits::default(),
            &|| false,
        )
        .expect_err("zero-guard accepted family rejects before topology")
        .path(),
        "maze.coverage.family_guard_steps"
    );
    let mut mismatched = request();
    mismatched.guard_steps = 2;
    assert_eq!(
        toniator_patterns::evaluate_typed_maze_walls_from_family_cancellable(
            &accepted,
            &mismatched,
            PatternOutputLayerId(53),
            &triangular_maze_program(23),
            toniator_patterns::MazeLimits::default(),
            &|| false,
        )
        .expect_err("accepted family guard policy must match")
        .path(),
        "maze.coverage.family_guard_steps"
    );
}

/// Proves a phase-aligned three-guide output derives conventional retained walls from typed sites.
#[test]
fn triangular_recursive_backtracker_derives_conventional_wall_maze() {
    let mut definition = triangular_grid();
    definition.output_layers = vec![PatternOutputLayer::all(
        PatternOutputLayerId(53),
        PatternOutputRealization::MazeWalls {
            site_mechanism_id: PatternMechanismId(52),
            program: triangular_maze_program(23),
            style: PathStrokeStyle::default(),
        },
    )];
    let plan = resolve_pattern_pipeline(&definition).expect("triangular family resolves");
    assert_eq!(plan.family.dimensions.len(), 3);
    assert_eq!(
        plan.family
            .dimensions
            .iter()
            .map(|dimension| (dimension.baseline_angle_degrees, dimension.phase))
            .collect::<Vec<_>>(),
        vec![(0.0, 0.0), (60.0, 0.0), (120.0, 0.0)]
    );
    let family = toniator_patterns::evaluate_typed_family_product_cancellable(
        &plan.family,
        &request(),
        &|| false,
    )
    .expect("triangular sites evaluate");
    assert!(family.site_set().iter().all(|site| matches!(
        &site.provenance,
        toniator_geometry::FamilySiteProvenance::GuideIntersection { contributors }
            if contributors.len() == 3
    )));
    let maze = toniator_patterns::evaluate_typed_maze_walls_cancellable(
        &plan.family,
        &request(),
        PatternOutputLayerId(53),
        &triangular_maze_program(23),
        toniator_patterns::MazeLimits::default(),
        &|| false,
    )
    .expect("triangular wall maze evaluates");
    assert!(!maze.cells.is_empty());
    let canvas_scopes = maze
        .source_sites
        .iter()
        .map(|site| (site.id, site.source.scope))
        .collect::<std::collections::BTreeMap<_, _>>();
    let canvas_cells = maze
        .cells
        .iter()
        .filter(|cell| {
            cell.vertices
                .iter()
                .all(|vertex| canvas_scopes[vertex] == toniator_geometry::SiteScope::Canvas)
        })
        .collect::<Vec<_>>();
    assert!(!canvas_cells.is_empty());
    assert!(canvas_cells.iter().all(|cell| cell.vertices.len() == 3));
    let source = maze
        .source_walls
        .iter()
        .map(|wall| wall.id)
        .collect::<BTreeSet<_>>();
    let retained = maze
        .retained_walls
        .iter()
        .map(|wall| wall.id)
        .collect::<BTreeSet<_>>();
    let passages = maze
        .removed_passage_walls
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let removed = passages
        .union(&BTreeSet::from([maze.entrance.wall, maze.exit.wall]))
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        source
            .difference(&removed)
            .copied()
            .collect::<BTreeSet<_>>(),
        retained
    );
    assert!(matches!(
        (maze.entrance.side, maze.exit.side),
        (
            toniator_geometry::MazeOpeningSide::Left,
            toniator_geometry::MazeOpeningSide::Right
        ) | (
            toniator_geometry::MazeOpeningSide::Right,
            toniator_geometry::MazeOpeningSide::Left
        ) | (
            toniator_geometry::MazeOpeningSide::Top,
            toniator_geometry::MazeOpeningSide::Bottom
        ) | (
            toniator_geometry::MazeOpeningSide::Bottom,
            toniator_geometry::MazeOpeningSide::Top
        )
    ));
    assert_eq!(maze.removed_passage_walls.len(), maze.cells.len() - 1);
    let positions = &maze.source_site_positions;
    let axes = [
        (0.0_f64, 1.0_f64),
        (-3.0_f64.sqrt() / 2.0, 0.5_f64),
        (3.0_f64.sqrt() / 2.0, 0.5_f64),
    ];
    assert!(maze.source_walls.iter().all(|wall| {
        let first = positions[&wall.id.first];
        let second = positions[&wall.id.second];
        let length = (second.x - first.x).hypot(second.y - first.y);
        axes.iter().any(|(axis_x, axis_y)| {
            ((second.x - first.x) * axis_y - (second.y - first.y) * axis_x).abs() <= length * 1e-8
        })
    }));
    let source_degree = maze
        .source_walls
        .iter()
        .fold(BTreeMap::new(), |mut degree, wall| {
            *degree.entry(wall.id.first).or_insert(0_usize) += 1;
            *degree.entry(wall.id.second).or_insert(0_usize) += 1;
            degree
        });
    assert!(
        maze.source_sites.iter().any(|site| site.source.scope
            == toniator_geometry::SiteScope::Canvas
            && source_degree.get(&site.id) == Some(&6)),
        "an interior triangular-lattice source vertex has six guide-axis walls"
    );
    let removed = maze
        .removed_passage_walls
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for ((first, second), wall) in maze
        .solution
        .cells
        .windows(2)
        .map(|cells| (cells[0], cells[1]))
        .zip(&maze.solution.passage_walls)
    {
        assert!(removed.contains(wall));
        assert!(maze.dual_edges.iter().any(|edge| {
            edge.shared_wall == *wall
                && ((edge.first == first && edge.second == second)
                    || (edge.first == second && edge.second == first))
        }));
    }
    let mut reached = BTreeSet::from([maze.entrance.cell]);
    loop {
        let mut changed = false;
        for edge in &maze.dual_edges {
            if removed.contains(&edge.shared_wall)
                && (reached.contains(&edge.first) ^ reached.contains(&edge.second))
            {
                changed |= reached.insert(if reached.contains(&edge.first) {
                    edge.second
                } else {
                    edge.first
                });
            }
        }
        if !changed {
            break;
        }
    }
    assert_eq!(reached.len(), maze.cells.len());
    assert!(maze.diagnostics.off_solution_cells > 0);
    assert!(maze.diagnostics.dead_end_cells > 0);
    assert!(maze.diagnostics.branch_cells > 0);
    let replay = toniator_patterns::evaluate_typed_maze_walls_cancellable(
        &plan.family,
        &request(),
        PatternOutputLayerId(53),
        &triangular_maze_program(23),
        toniator_patterns::MazeLimits::default(),
        &|| false,
    )
    .expect("same seed replays");
    assert_eq!(maze, replay);
}

/// Preserves every inclusive 24-across candidate site and connected bounded face.
///
/// This artifact-scale headless regression exercises both the regular two-guide and aligned
/// 0/60/120 three-guide families. It proves family coverage, not a renderer crop: all and only
/// in-canvas evaluated sites remain inspectable maze source authority, and every bounded face in
/// the selected connected component remains an emitted maze cell.
#[test]
fn artifact_scale_maze_core_preserves_inclusive_candidate_site_authority() {
    for (name, definition) in [("square", grid()), ("triangular", triangular_grid())] {
        let plan = resolve_pattern_pipeline(&definition).expect("artifact family resolves");
        let request = artifact_scale_request();
        let family = toniator_patterns::evaluate_typed_family_product_cancellable(
            &plan.family,
            &request,
            &|| false,
        )
        .expect("artifact-scale family evaluates");
        let maze = toniator_patterns::evaluate_typed_maze_walls_cancellable(
            &plan.family,
            &request,
            PatternOutputLayerId(53),
            &triangular_maze_program(23),
            toniator_patterns::MazeLimits::default(),
            &|| false,
        )
        .expect("artifact-scale maze evaluates");
        let candidates = family
            .site_set()
            .iter()
            .filter(|site| {
                (0.0..=request.canvas.width).contains(&site.position.x)
                    && (0.0..=request.canvas.height).contains(&site.position.y)
            })
            .map(|site| site.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            maze.source_sites
                .iter()
                .map(|site| site.source.id)
                .collect::<BTreeSet<_>>(),
            candidates,
            "{name} retains every inclusive family site as candidate source authority"
        );
        assert!(maze.source_sites.iter().all(|site| {
            (0.0..=request.canvas.width).contains(&site.source.position.x)
                && (0.0..=request.canvas.height).contains(&site.source.position.y)
        }));
        let source_vertices = maze
            .source_sites
            .iter()
            .map(|site| site.id)
            .collect::<BTreeSet<_>>();
        assert!(
            maze.wall_paths
                .iter()
                .flat_map(|path| path.vertices)
                .all(|vertex| source_vertices.contains(&vertex))
        );
        assert!(
            maze.source_walls
                .iter()
                .all(|wall| { maze.cells.iter().any(|cell| cell.walls.contains(&wall.id)) })
        );
        let used_vertices = maze
            .cells
            .iter()
            .flat_map(|cell| cell.vertices.iter().copied())
            .collect::<BTreeSet<_>>();
        assert!(
            used_vertices
                .iter()
                .all(|vertex| source_vertices.contains(vertex)),
            "{name} emitted cells use only inclusive candidate sites"
        );
        assert!(
            maze.source_sites.iter().any(|site| {
                (site.source.position.x == 0.0
                    || site.source.position.x == request.canvas.width
                    || site.source.position.y == 0.0
                    || site.source.position.y == request.canvas.height)
                    && used_vertices.contains(&site.id)
            }),
            "{name} retains a boundary-adjacent valid site in its selected bounded-face component"
        );
    }
}

/// Preserves centered artifact-scale family sites across maze authority and numerical boundaries.
///
/// This regression keeps the centered zero-phase origin and floating-point boundary snap
/// together: ordinary and 0/60/120 grids retain their established candidate counts, every exact
/// canvas-edge candidate remains a selected maze-cell vertex, true guard rows stay out of maze
/// authority, an authored translation can place the triangular lattice on the top edge without
/// losing its recovered intersections, and a broader cached family replays the same maze identity.
#[test]
fn artifact_scale_intersections_snap_only_numerical_canvas_boundary_noise() {
    for (name, definition, width, height, expected_count, expected_boundary_count) in [
        ("square-raster", grid(), 1024.0, 1024.0, 625_usize, 96_usize),
        ("square-vector", grid(), 900.0, 620.0, 425_usize, 34_usize),
        (
            "triangular-raster",
            triangular_grid(),
            1024.0,
            1024.0,
            513_usize,
            42_usize,
        ),
        (
            "triangular-vector",
            triangular_grid(),
            900.0,
            620.0,
            363_usize,
            30_usize,
        ),
    ] {
        let request = artifact_scale_request_for(width, height);
        let plan = resolve_pattern_pipeline(&definition).expect("artifact family resolves");
        let family = toniator_patterns::evaluate_typed_family_product_cancellable(
            &plan.family,
            &request,
            &|| false,
        )
        .expect("artifact family evaluates");
        let canvas_sites = family
            .site_set()
            .iter()
            .filter(|site| {
                (0.0..=width).contains(&site.position.x)
                    && (0.0..=height).contains(&site.position.y)
            })
            .collect::<Vec<_>>();
        let boundary_sites = canvas_sites
            .iter()
            .copied()
            .filter(|site| {
                site.position.x == 0.0
                    || site.position.x == width
                    || site.position.y == 0.0
                    || site.position.y == height
            })
            .collect::<Vec<_>>();
        assert_eq!(
            canvas_sites.len(),
            expected_count,
            "{name} canvas site count"
        );
        assert_eq!(
            boundary_sites.len(),
            expected_boundary_count,
            "{name} exact canvas-edge site count"
        );
        assert!(
            boundary_sites
                .iter()
                .all(|site| site.scope == toniator_geometry::SiteScope::Canvas)
        );
        assert!(family.site_set().iter().any(|site| {
            site.scope == toniator_geometry::SiteScope::Guard
                && (!(0.0..=width).contains(&site.position.x)
                    || !(0.0..=height).contains(&site.position.y))
        }));

        let maze = toniator_patterns::evaluate_typed_maze_walls_cancellable(
            &plan.family,
            &request,
            PatternOutputLayerId(53),
            &triangular_maze_program(23),
            toniator_patterns::MazeLimits::default(),
            &|| false,
        )
        .expect("artifact maze evaluates");
        assert_eq!(
            maze.source_sites.len(),
            expected_count,
            "{name} maze excludes guards"
        );
        let used = maze
            .cells
            .iter()
            .flat_map(|cell| cell.vertices.iter().copied())
            .collect::<BTreeSet<_>>();
        let boundary_vertices = maze
            .source_sites
            .iter()
            .filter(|site| {
                site.source.position.x == 0.0
                    || site.source.position.x == width
                    || site.source.position.y == 0.0
                    || site.source.position.y == height
            })
            .map(|site| site.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            boundary_vertices.len(),
            expected_boundary_count,
            "{name} maze receives every exact canvas-edge site"
        );
        assert!(
            boundary_vertices.is_subset(&used),
            "{name} selected maze cells retain every exact canvas-edge site"
        );
        let centered = toniator_geometry::Point2::new(width * 0.5, height * 0.5);
        let centered_family_site = family
            .site_set()
            .iter()
            .find(|site| {
                (site.position.x - centered.x).abs() < 1.0e-9
                    && (site.position.y - centered.y).abs() < 1.0e-9
            })
            .expect("centered family site remains published");
        assert!(maze.source_sites.iter().any(|site| {
            site.source.id == centered_family_site.id
                && (site.source.position.x - centered.x).abs() < 1.0e-9
                && (site.source.position.y - centered.y).abs() < 1.0e-9
        }));
    }

    for (name, width, height) in [
        ("triangular-raster", 1024.0, 1024.0),
        ("triangular-vector", 900.0, 620.0),
    ] {
        let mut boundary_request = artifact_scale_request_for(width, height);
        boundary_request.translation_x = -width * 0.5;
        boundary_request.translation_y = -height * 0.5;
        let plan =
            resolve_pattern_pipeline(&triangular_grid()).expect("triangular family resolves");
        let family = toniator_patterns::evaluate_typed_family_product_cancellable(
            &plan.family,
            &boundary_request,
            &|| false,
        )
        .expect("translated boundary family evaluates");
        let top_sites = family
            .site_set()
            .iter()
            .filter(|site| {
                site.scope == toniator_geometry::SiteScope::Canvas && site.position.y == 0.0
            })
            .collect::<Vec<_>>();
        assert_eq!(top_sites.len(), 13, "{name} retains its exact top-edge row");
        if name == "triangular-raster" {
            assert_eq!(
                top_sites
                    .iter()
                    .filter(|site| site.position.x > 0.0 && site.position.x < width)
                    .count(),
                11,
                "the eleven raster interior top-edge intersections are exact canvas sites"
            );
        }
    }

    let plan = resolve_pattern_pipeline(&triangular_grid()).expect("triangular family resolves");
    let request = artifact_scale_request();
    let exact = toniator_patterns::evaluate_typed_maze_walls_cancellable(
        &plan.family,
        &request,
        PatternOutputLayerId(53),
        &triangular_maze_program(23),
        toniator_patterns::MazeLimits::default(),
        &|| false,
    )
    .expect("exact triangular maze evaluates");
    let mut broader_request = request.clone();
    broader_request.support_radius += 80.0;
    let broader = toniator_patterns::evaluate_typed_family_product_cancellable(
        &plan.family,
        &broader_request,
        &|| false,
    )
    .expect("broader triangular family evaluates");
    let reused = toniator_patterns::evaluate_typed_maze_walls_from_family_cancellable(
        &broader,
        &request,
        PatternOutputLayerId(53),
        &triangular_maze_program(23),
        toniator_patterns::MazeLimits::default(),
        &|| false,
    )
    .expect("broader family reuses the exact triangular maze envelope");
    assert_eq!(exact.fingerprint(), reused.fingerprint());
}

/// Produces site-provenance overlays for all current intrinsic maze validation images.
///
/// This ignored diagnostic test reads only existing maze PNGs and writes only test-owned overlays
/// and legends under `target/validation/stage20m`; it does not alter maze geometry or rendering.
/// Cyan discs identify sites used by selected maze cells, while magenta crosses identify inclusive
/// candidate sites not used by those cells.
#[test]
#[ignore = "writes test-owned maze site-provenance diagnostic overlays"]
fn generate_maze_site_provenance_overlays() -> Result<(), Box<dyn std::error::Error>> {
    let output_directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/validation/stage20m");
    fs::create_dir_all(&output_directory)?;
    let mut summary = String::from("Stage 20M maze site provenance overlays\n");
    for (maze_name, definition, seed) in [
        ("maze2", grid(), 17_u32),
        ("maze3", triangular_grid(), 23_u32),
    ] {
        let plan = resolve_pattern_pipeline(&definition)?;
        for (surface_name, width, height) in [("raster", 1024.0, 1024.0), ("vector", 900.0, 620.0)]
        {
            let request = artifact_scale_request_for(width, height);
            let maze = toniator_patterns::evaluate_typed_maze_walls_cancellable(
                &plan.family,
                &request,
                PatternOutputLayerId(53),
                &triangular_maze_program(seed),
                toniator_patterns::MazeLimits::default(),
                &|| false,
            )?;
            let artifact_name = format!("{maze_name}-{surface_name}");
            let counts = write_maze_site_provenance_overlay(
                &output_directory,
                &artifact_name,
                &maze,
                &request,
            )?;
            let legend = format!(
                "{artifact_name}-sites.png\ncyan filled disc (#00E5FF): selected maze-cell site ({})\nmagenta cross (#FF00C8): inclusive candidate site not used by selected cells ({})\nboundary candidates/selected: {}/{}\n",
                counts.candidates,
                counts.candidates - counts.used,
                counts.boundary_candidates,
                counts.boundary_used,
            );
            fs::write(
                output_directory.join(format!("{artifact_name}-sites-legend.txt")),
                &legend,
            )?;
            summary.push_str(&legend);
        }
    }
    fs::write(
        output_directory.join("maze-site-provenance-legends.txt"),
        summary,
    )?;
    Ok(())
}

/// Records the candidate and selected-site counts displayed by one diagnostic overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SiteOverlayCounts {
    /// Counts every inclusive in-canvas candidate source site.
    candidates: usize,
    /// Counts candidate source sites referenced by selected maze cells.
    used: usize,
    /// Counts candidates on an intrinsic canvas boundary.
    boundary_candidates: usize,
    /// Counts selected sites on an intrinsic canvas boundary.
    boundary_used: usize,
}

/// Composites one existing maze image on white and writes its selected/candidate site overlay.
///
/// # Errors
///
/// Returns image or filesystem errors without modifying production artifacts or geometry.
fn write_maze_site_provenance_overlay(
    output_directory: &Path,
    artifact_name: &str,
    maze: &toniator_patterns::MazeProgramResult,
    request: &GridInspectRequest,
) -> Result<SiteOverlayCounts, Box<dyn std::error::Error>> {
    let mut image = image::open(output_directory.join(format!("{artifact_name}.png")))?.to_rgba8();
    let (width, height) = image.dimensions();
    if width != request.canvas.width as u32 || height != request.canvas.height as u32 {
        return Err(format!(
            "{artifact_name}.png dimensions {width}x{height} do not match artifact request {}x{}",
            request.canvas.width, request.canvas.height
        )
        .into());
    }
    for pixel in image.pixels_mut() {
        let alpha = u16::from(pixel[3]);
        for channel in 0..3 {
            pixel[channel] =
                ((u16::from(pixel[channel]) * alpha + 255 * (255 - alpha)) / 255) as u8;
        }
        pixel[3] = 255;
    }
    let used = maze
        .cells
        .iter()
        .flat_map(|cell| cell.vertices.iter().copied())
        .collect::<BTreeSet<_>>();
    let mut boundary_candidates = 0_usize;
    let mut boundary_used = 0_usize;
    for site in &maze.source_sites {
        let x = site.source.position.x.round() as i32;
        let y = site.source.position.y.round() as i32;
        let selected = used.contains(&site.id);
        let boundary = site.source.position.x.abs() <= 1e-9
            || (site.source.position.x - request.canvas.width).abs() <= 1e-9
            || site.source.position.y.abs() <= 1e-9
            || (site.source.position.y - request.canvas.height).abs() <= 1e-9;
        if boundary {
            boundary_candidates += 1;
            if selected {
                boundary_used += 1;
            }
        }
        if selected {
            overlay_site_marker(&mut image, x, y, [0, 229, 255, 255], true);
        } else {
            overlay_site_marker(&mut image, x, y, [255, 0, 200, 255], false);
        }
    }
    image.save(output_directory.join(format!("{artifact_name}-sites.png")))?;
    Ok(SiteOverlayCounts {
        candidates: maze.source_sites.len(),
        used: used.len(),
        boundary_candidates,
        boundary_used,
    })
}

/// Draws one clipped diagnostic site marker without changing the underlying maze geometry.
fn overlay_site_marker(
    image: &mut image::RgbaImage,
    center_x: i32,
    center_y: i32,
    color: [u8; 4],
    selected: bool,
) {
    for offset_y in -4_i32..=4 {
        for offset_x in -4_i32..=4 {
            let distance_squared = offset_x * offset_x + offset_y * offset_y;
            let draw = if selected {
                distance_squared <= 16
            } else {
                offset_x.abs() == offset_y.abs() && offset_x.abs() <= 4
            };
            if !draw {
                continue;
            }
            let x = center_x + offset_x;
            let y = center_y + offset_y;
            if x >= 0 && y >= 0 && x < image.width() as i32 && y < image.height() as i32 {
                image.put_pixel(x as u32, y as u32, image::Rgba(color));
            }
        }
    }
}

/// Reuses a broader accepted family only after preserving the exact requested maze envelope.
#[test]
fn maze_from_broader_typed_family_matches_exact_clipped_walls_and_solution() {
    let plan = resolve_pattern_pipeline(&triangular_grid()).expect("triangular family resolves");
    let exact_request = request();
    let exact = toniator_patterns::evaluate_typed_maze_walls_cancellable(
        &plan.family,
        &exact_request,
        PatternOutputLayerId(53),
        &triangular_maze_program(41),
        toniator_patterns::MazeLimits::default(),
        &|| false,
    )
    .expect("exact maze evaluates");
    let mut broader_request = exact_request.clone();
    broader_request.support_radius = 18.0;
    let broader = toniator_patterns::evaluate_typed_family_product_cancellable(
        &plan.family,
        &broader_request,
        &|| false,
    )
    .expect("broader family evaluates");
    let reused = toniator_patterns::evaluate_typed_maze_walls_from_family_cancellable(
        &broader,
        &exact_request,
        PatternOutputLayerId(53),
        &triangular_maze_program(41),
        toniator_patterns::MazeLimits::default(),
        &|| false,
    )
    .expect("broader family subsets to exact maze envelope");
    let wall_geometry = |maze: &toniator_patterns::MazeProgramResult| {
        maze.wall_paths
            .iter()
            .map(|wall| {
                let segment = &wall.path.segments()[0];
                let first = (
                    (segment.start().x * 1e9).round() as i64,
                    (segment.start().y * 1e9).round() as i64,
                );
                let second = (
                    (segment.end().x * 1e9).round() as i64,
                    (segment.end().y * 1e9).round() as i64,
                );
                if first < second {
                    vec![first, second]
                } else {
                    vec![second, first]
                }
            })
            .collect::<BTreeSet<_>>()
    };
    let source_geometry = |maze: &toniator_patterns::MazeProgramResult| {
        maze.source_walls
            .iter()
            .map(|wall| {
                let point = |id| {
                    let site = maze.source_site_positions[&id];
                    ((site.x * 1e9).round() as i64, (site.y * 1e9).round() as i64)
                };
                let first = point(wall.id.first);
                let second = point(wall.id.second);
                if first < second {
                    vec![first, second]
                } else {
                    vec![second, first]
                }
            })
            .collect::<BTreeSet<_>>()
    };
    let solution_geometry = |maze: &toniator_patterns::MazeProgramResult| {
        std::iter::once(&maze.solution)
            .map(|solution| {
                solution
                    .cells
                    .iter()
                    .map(|id| {
                        let cell = &maze.cells[id.0 as usize];
                        cell.vertices
                            .iter()
                            .map(|vertex| {
                                let site = maze.source_site_positions[vertex];
                                ((site.x * 1e9).round() as i64, (site.y * 1e9).round() as i64)
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let exact_source = source_geometry(&exact);
    let reused_source = source_geometry(&reused);
    assert_eq!(exact_source, reused_source);
    assert_eq!(wall_geometry(&exact), wall_geometry(&reused));
    assert_eq!(solution_geometry(&exact), solution_geometry(&reused));
    assert_eq!(exact.fingerprint(), reused.fingerprint());
    let insufficient = toniator_patterns::evaluate_typed_family_product_cancellable(
        &plan.family,
        &GridInspectRequest {
            support_radius: 1.0,
            ..exact_request.clone()
        },
        &|| false,
    )
    .expect("narrow family evaluates");
    assert_eq!(
        toniator_patterns::evaluate_typed_maze_walls_from_family_cancellable(
            &insufficient,
            &exact_request,
            PatternOutputLayerId(53),
            &triangular_maze_program(41),
            toniator_patterns::MazeLimits::default(),
            &|| false,
        )
        .expect_err("insufficient family rejects before arrangement")
        .path(),
        "maze.coverage.support"
    );
}

/// Cancels accepted-family exact-envelope selection before it can allocate or enter maze geometry.
#[test]
fn accepted_family_maze_selection_observes_cancellation_before_geometry() {
    let plan = resolve_pattern_pipeline(&triangular_grid()).expect("triangular family resolves");
    let mut broad_request = request();
    broad_request.support_radius = 18.0;
    let accepted = toniator_patterns::evaluate_typed_family_product_cancellable(
        &plan.family,
        &broad_request,
        &|| false,
    )
    .expect("broader accepted family evaluates");
    assert_eq!(
        toniator_patterns::evaluate_typed_maze_walls_from_family_cancellable(
            &accepted,
            &request(),
            PatternOutputLayerId(53),
            &triangular_maze_program(23),
            toniator_patterns::MazeLimits::default(),
            &|| true,
        )
        .expect_err("cancelled accepted-family selection returns no maze result")
        .path(),
        "evaluation.cancelled"
    );
}

/// Realizes normalized connection thickness against the minimum retained trail basis, never pixels.
#[test]
fn connection_strokes_use_normalized_thickness_times_minimum_trail_basis() {
    let evaluated = connections(&grid(), &program(0)).expect("grid connections");
    let realized = realize(
        &evaluated,
        StrokeResponse {
            minimum_thickness: 2.0,
            maximum_thickness: 2.0,
            bias: 0.0,
        },
        100_000,
        100_000,
        &|| false,
    )
    .expect("normalized connection strokes");
    assert!(!realized.strokes.is_empty());
    for stroke in &realized.strokes {
        assert!(
            stroke.nominal_basis > 2.0,
            "fixture excludes pixel-size coincidence"
        );
        assert!(stroke.profile.iter().all(|sample| {
            (sample.normalized_thickness - 2.0).abs() < 1e-12
                && (sample.width - 2.0 * stroke.nominal_basis).abs() < 1e-12
        }));
    }
}

/// Keeps connection realization identity sensitive to geometry-owned algorithm contracts and seeds.
#[test]
fn connection_realization_identity_tracks_program_contract_and_seed() {
    let nearest = connections(&grid(), &program(0)).expect("nearest connections");
    let random = connections(&grid(), &program(1)).expect("random connections");
    let tree = connections(&grid(), &program(2)).expect("tree connections");
    let response = StrokeResponse {
        minimum_thickness: 1.0,
        maximum_thickness: 1.0,
        bias: 0.0,
    };
    let identities = [&nearest, &random, &tree]
        .iter()
        .map(|paths| {
            realize(paths, response, 100_000, 100_000, &|| false)
                .expect("connection realization")
                .realization_fingerprint
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(identities.len(), 3);
    let alternate = ConnectionProgram::RandomLinks {
        adjacency: ConnectionAdjacencyIntent {
            maximum_degree: 2,
            maximum_distance: 14.0,
        },
        minimum_degree: 1,
        seed: 99,
    };
    let alternate = connections(&grid(), &alternate).expect("alternate random connections");
    assert_ne!(
        realize(&random, response, 100_000, 100_000, &|| false)
            .expect("seed seven")
            .realization_fingerprint,
        realize(&alternate, response, 100_000, 100_000, &|| false)
            .expect("seed ninety-nine")
            .realization_fingerprint
    );
}

/// Fails connection profile, outline, and cancellation work atomically at the public realizer.
#[test]
fn connection_realizer_limits_and_cancellation_return_stable_errors() {
    let evaluated = connections(&grid(), &program(0)).expect("grid connections");
    let response = StrokeResponse {
        minimum_thickness: 1.0,
        maximum_thickness: 1.0,
        bias: 0.0,
    };
    assert_eq!(
        realize(&evaluated, response, 1, 100_000, &|| false)
            .expect_err("profile limit")
            .path(),
        "connection.stroke.profile_limit"
    );
    assert_eq!(
        realize(&evaluated, response, 100_000, 1, &|| false)
            .expect_err("outline limit")
            .path(),
        "connection.stroke.outline_limit"
    );
    assert_eq!(
        realize(&evaluated, response, 100_000, 100_000, &|| true)
            .expect_err("cancellation")
            .path(),
        "evaluation.cancelled"
    );
}

/// Enforces maze profile/outline limits and cancellation before canonical strokes can publish.
#[test]
fn maze_realizer_limits_and_cancellation_are_atomic() {
    let plan = resolve_pattern_pipeline(&triangular_grid()).expect("triangular family resolves");
    let maze = toniator_patterns::evaluate_typed_maze_walls_cancellable(
        &plan.family,
        &request(),
        PatternOutputLayerId(53),
        &triangular_maze_program(23),
        toniator_patterns::MazeLimits::default(),
        &|| false,
    )
    .expect("maze evaluates before canonical stroke realization");
    let realize_maze =
        |max_profile_samples, max_outline_segments, is_cancelled: &dyn Fn() -> bool| {
            realize_maze_canonical_strokes_cancellable(
                &maze,
                &white_source(),
                &request().canvas,
                SourceMapping::canonical(SourceMappingComponent::Luminance),
                StrokeResponse {
                    minimum_thickness: 1.0,
                    maximum_thickness: 1.0,
                    bias: 0.0,
                },
                PathStrokeStyle::default(),
                max_profile_samples,
                max_outline_segments,
                is_cancelled,
            )
        };
    assert_eq!(
        realize_maze(1, 100_000, &|| false)
            .expect_err("profile limit")
            .path(),
        "maze.stroke.profile_limit"
    );
    assert_eq!(
        realize_maze(100_000, 1, &|| false)
            .expect_err("outline limit")
            .path(),
        "maze.stroke.outline_limit"
    );
    assert_eq!(
        realize_maze(100_000, 100_000, &|| true)
            .expect_err("cancelled maze stroke realization has no partial result")
            .path(),
        "evaluation.cancelled"
    );
}

/// Proves consuming connection and maze realization preserves borrowed output identity exactly.
#[test]
fn consuming_stroke_realizers_preserve_existing_canonical_output() {
    let response = StrokeResponse {
        minimum_thickness: 0.25,
        maximum_thickness: 0.75,
        bias: 0.0,
    };
    let evaluated = connections(&grid(), &program(0)).expect("connection paths evaluate");
    let borrowed_connection = realize(&evaluated, response, usize::MAX, usize::MAX, &|| false)
        .expect("borrowed connection stroke realization succeeds");
    let owned_connection = realize_owned_connection_canonical_strokes_cancellable(
        evaluated.paths,
        &white_source(),
        &request().canvas,
        SourceMapping::canonical(SourceMappingComponent::Luminance),
        response,
        PathStrokeStyle::default(),
        usize::MAX,
        usize::MAX,
        &|| false,
    )
    .expect("consuming connection stroke realization succeeds");
    assert_eq!(owned_connection, borrowed_connection);

    let plan = resolve_pattern_pipeline(&triangular_grid()).expect("triangular family resolves");
    let maze = toniator_patterns::evaluate_typed_maze_walls_cancellable(
        &plan.family,
        &request(),
        PatternOutputLayerId(53),
        &triangular_maze_program(23),
        toniator_patterns::MazeLimits::default(),
        &|| false,
    )
    .expect("maze evaluates");
    let borrowed_maze = realize_maze_canonical_strokes_cancellable(
        &maze,
        &white_source(),
        &request().canvas,
        SourceMapping::canonical(SourceMappingComponent::Luminance),
        response,
        PathStrokeStyle::default(),
        usize::MAX,
        usize::MAX,
        &|| false,
    )
    .expect("borrowed maze stroke realization succeeds");
    let owned_maze = realize_owned_maze_canonical_strokes_cancellable(
        maze,
        &white_source(),
        &request().canvas,
        SourceMapping::canonical(SourceMappingComponent::Luminance),
        response,
        PathStrokeStyle::default(),
        usize::MAX,
        usize::MAX,
        &|| false,
    )
    .expect("consuming maze stroke realization succeeds");
    assert_eq!(owned_maze, borrowed_maze);
}
