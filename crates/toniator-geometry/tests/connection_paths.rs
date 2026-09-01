use std::{cell::Cell, collections::BTreeSet};

use toniator_domain::{
    ConnectionAdjacencyIntent, ConnectionProgram, GridMazeAlgorithm, GridSpanningTreeAlgorithm,
    GuideDimensionId, MazeProgram, PatternMechanismId, PatternOutputLayerId,
};
use toniator_geometry::{
    Bounds, CONNECTION_NEAREST_SELECTION_CONTRACT_ID, CONNECTION_PRIM_SELECTION_CONTRACT_ID,
    CONNECTION_RANDOM_SELECTION_CONTRACT_ID, ConnectionPathLimits, FamilySite, FamilySiteId,
    FamilySiteProvenance, FamilySiteSet, GuideInstanceId, MazeGuideAxis, MazeLimits,
    NominalCellBasis, Point2, SiteAdjacencyLimits, SiteAdjacencyPolicy, SiteScope, Vector2,
    build_connection_paths_cancellable, build_maze_walls_cancellable,
    build_site_adjacency_cancellable, connection_program_contract_id,
};

/// Configures one independently bounded maze work category for a focused runtime-limit witness.
type MazeLimitConfigurator = fn(&mut MazeLimits);

/// Builds one square-capable finite site product for deterministic connection witnesses.
fn sites() -> FamilySiteSet {
    sites_at(&[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)])
}

/// Builds an ordered finite site product for an explicit connection-topology witness.
fn sites_at(points: &[(f64, f64)]) -> FamilySiteSet {
    FamilySiteSet::new(
        "connection-test".into(),
        PatternMechanismId(9),
        points
            .iter()
            .copied()
            .enumerate()
            .map(|(ordinal, (x, y))| FamilySite {
                id: FamilySiteId {
                    mechanism_id: PatternMechanismId(9),
                    ordinal,
                },
                position: Point2::new(x, y),
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(1.0, 0.0),
                    Vector2::new(0.0, 1.0),
                )
                .expect("unit basis is valid"),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: ordinal,
                    accepted_ordinal: ordinal,
                    exclusion_neighbor_ordinal: None,
                },
            })
            .collect(),
    )
    .expect("finite sites are valid")
}

/// Builds finite sites with explicit nominal bases for connection-width authority witnesses.
fn sites_with_bases(points: &[(f64, f64)], bases: &[(Vector2, Vector2)]) -> FamilySiteSet {
    FamilySiteSet::new(
        "connection-basis-test".into(),
        PatternMechanismId(10),
        points
            .iter()
            .copied()
            .zip(bases.iter().copied())
            .enumerate()
            .map(|(ordinal, ((x, y), (across, down)))| FamilySite {
                id: FamilySiteId {
                    mechanism_id: PatternMechanismId(10),
                    ordinal,
                },
                position: Point2::new(x, y),
                nominal_cell_basis: NominalCellBasis::new(across, down).expect("finite basis"),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: ordinal,
                    accepted_ordinal: ordinal,
                    exclusion_neighbor_ordinal: None,
                },
            })
            .collect(),
    )
    .expect("finite sites are valid")
}

/// Builds a square lattice or a larger truthful triangular lattice for in-canvas arrangement tests.
fn arrangement_sites(triangular: bool) -> FamilySiteSet {
    let mut values = Vec::new();
    let coordinates = if triangular { -4_i64..=4 } else { -3_i64..=4 };
    for row in coordinates.clone() {
        for column in coordinates.clone() {
            let (x, y, contributors) = if triangular {
                let x = 5.0 * 3.0_f64.sqrt() * row as f64;
                let y = 10.0 * column as f64 + 5.0 * row as f64;
                (
                    x,
                    y,
                    vec![
                        GuideInstanceId::new(GuideDimensionId(1), row),
                        GuideInstanceId::new(GuideDimensionId(2), column + row),
                        GuideInstanceId::new(GuideDimensionId(3), column),
                    ],
                )
            } else {
                (
                    10.0 * column as f64,
                    10.0 * row as f64,
                    vec![
                        GuideInstanceId::new(GuideDimensionId(1), column),
                        GuideInstanceId::new(GuideDimensionId(2), row),
                    ],
                )
            };
            values.push(FamilySite {
                id: FamilySiteId {
                    mechanism_id: PatternMechanismId(44),
                    ordinal: values.len(),
                },
                position: Point2::new(x, y),
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(10.0, 0.0),
                    Vector2::new(0.0, 10.0),
                )
                .expect("finite lattice basis"),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::GuideIntersection { contributors },
            });
        }
    }
    FamilySiteSet::new(
        "maze-arrangement-sites".into(),
        PatternMechanismId(44),
        values,
    )
    .expect("truthful guide lattice")
}

/// Builds a boundary-aligned square or triangular guide family with bounded arrangement faces.
///
/// The fixture deliberately places actual guide-intersection sites on every relevant canvas edge.
/// Maze topology keeps those inclusive candidates and derives only their actual bounded faces;
/// final clipping remains a presentation concern rather than a source-site eligibility rule.
fn boundary_aligned_arrangement(triangular: bool) -> (FamilySiteSet, Bounds) {
    if triangular {
        let spacing = 5.0 * 3.0_f64.sqrt();
        (
            arrangement_sites(true),
            Bounds::new(
                Point2::new(-4.0 * spacing, -60.0),
                Point2::new(4.0 * spacing, 60.0),
            )
            .expect("finite boundary-aligned triangular canvas"),
        )
    } else {
        let mut values = Vec::new();
        for row in 0_i64..=6 {
            for column in 0_i64..=6 {
                values.push(FamilySite {
                    id: FamilySiteId {
                        mechanism_id: PatternMechanismId(48),
                        ordinal: values.len(),
                    },
                    position: Point2::new(10.0 * column as f64, 10.0 * row as f64),
                    nominal_cell_basis: NominalCellBasis::new(
                        Vector2::new(10.0, 0.0),
                        Vector2::new(0.0, 10.0),
                    )
                    .expect("finite boundary-aligned square basis"),
                    scope: SiteScope::Canvas,
                    provenance: FamilySiteProvenance::GuideIntersection {
                        contributors: vec![
                            GuideInstanceId::new(GuideDimensionId(1), column),
                            GuideInstanceId::new(GuideDimensionId(2), row),
                        ],
                    },
                });
            }
        }
        (
            FamilySiteSet::new(
                "boundary-aligned-square-maze-sites".into(),
                PatternMechanismId(48),
                values,
            )
            .expect("finite boundary-aligned square lattice"),
            Bounds::new(Point2::new(0.0, 0.0), Point2::new(60.0, 60.0))
                .expect("finite boundary-aligned square canvas"),
        )
    }
}

/// Moves a boundary-aligned fixture only one half unit inside its canvas without changing sites.
///
/// This near-boundary arrangement proves bounded-face selection does not erode inclusive
/// source-site authority with an inset, clearance, or fixed ring.
fn near_boundary_arrangement(triangular: bool) -> (FamilySiteSet, Bounds) {
    let (sites, boundary_canvas) = boundary_aligned_arrangement(triangular);
    (
        sites,
        Bounds::new(
            Point2::new(boundary_canvas.min.x - 0.5, boundary_canvas.min.y - 0.5),
            Point2::new(boundary_canvas.max.x + 0.5, boundary_canvas.max.y + 0.5),
        )
        .expect("finite near-boundary maze canvas"),
    )
}

/// Builds a nonuniform two-guide arrangement whose bounded cells have intentionally mixed areas.
fn mixed_quad_arrangement_sites() -> FamilySiteSet {
    let columns = [0.0, 2.0, 7.0, 13.0, 20.0, 28.0, 37.0];
    let rows = [0.0, 3.0, 10.0, 18.0, 27.0, 37.0, 48.0];
    let mut values = Vec::new();
    for (row_ordinal, y) in rows.into_iter().enumerate() {
        for (column_ordinal, x) in columns.into_iter().enumerate() {
            values.push(FamilySite {
                id: FamilySiteId {
                    mechanism_id: PatternMechanismId(45),
                    ordinal: values.len(),
                },
                position: Point2::new(x, y),
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(2.0, 0.0),
                    Vector2::new(0.0, 3.0),
                )
                .expect("finite nonuniform lattice basis"),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::GuideIntersection {
                    contributors: vec![
                        GuideInstanceId::new(GuideDimensionId(11), column_ordinal as i64),
                        GuideInstanceId::new(GuideDimensionId(12), row_ordinal as i64),
                    ],
                },
            });
        }
    }
    FamilySiteSet::new(
        "maze-mixed-arrangement-sites".into(),
        PatternMechanismId(45),
        values,
    )
    .expect("truthful mixed guide lattice")
}

/// Builds two complete two-guide components plus one disconnected single-cell component.
fn disconnected_dual_sites() -> (FamilySiteSet, Vec<MazeGuideAxis>) {
    let mut values = Vec::new();
    for (patch, (origin_x, origin_y, maximum_ordinal)) in [
        (-45.0, -45.0, 5_i64),
        (20.0, 25.0, 4_i64),
        (70.0, 25.0, 1_i64),
    ]
    .into_iter()
    .enumerate()
    {
        for row in 0_i64..=maximum_ordinal {
            for column in 0_i64..=maximum_ordinal {
                values.push(FamilySite {
                    id: FamilySiteId {
                        mechanism_id: PatternMechanismId(47),
                        ordinal: values.len(),
                    },
                    position: Point2::new(
                        origin_x + 10.0 * column as f64,
                        origin_y + 10.0 * row as f64,
                    ),
                    nominal_cell_basis: NominalCellBasis::new(
                        Vector2::new(10.0, 0.0),
                        Vector2::new(0.0, 10.0),
                    )
                    .expect("finite disconnected basis"),
                    scope: SiteScope::Canvas,
                    provenance: FamilySiteProvenance::GuideIntersection {
                        contributors: vec![
                            GuideInstanceId::new(GuideDimensionId(31), 10 * patch as i64 + column),
                            GuideInstanceId::new(GuideDimensionId(32), 10 * patch as i64 + row),
                        ],
                    },
                });
            }
        }
    }
    let sites = FamilySiteSet::new(
        "maze-disconnected-dual-sites".into(),
        PatternMechanismId(47),
        values,
    )
    .expect("finite disconnected sites");
    let axes = sites
        .iter()
        .flat_map(|site| match &site.provenance {
            FamilySiteProvenance::GuideIntersection { contributors } => {
                contributors.iter().copied()
            }
            _ => unreachable!("disconnected fixture uses guide intersections"),
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|id| {
            MazeGuideAxis::new(
                id,
                if id.dimension_id == 31 {
                    Vector2::new(0.0, 1.0)
                } else {
                    Vector2::new(1.0, 0.0)
                },
            )
            .expect("disconnected guide tangent is finite")
        })
        .collect();
    (sites, axes)
}

/// Builds a rotated two-guide arrangement so opening ranking cannot depend on axis-aligned walls.
fn rotated_quad_arrangement() -> (FamilySiteSet, Vec<MazeGuideAxis>) {
    let along = Vector2::new(5.0 * 3.0_f64.sqrt(), 5.0);
    let across = Vector2::new(-5.0, 5.0 * 3.0_f64.sqrt());
    let mut sites = Vec::new();
    for row in 0_i64..=6 {
        for column in 0_i64..=6 {
            let position = Point2::new(
                along.x * (column as f64 - 2.0) + across.x * (row as f64 - 2.0),
                along.y * (column as f64 - 2.0) + across.y * (row as f64 - 2.0),
            );
            sites.push(FamilySite {
                id: FamilySiteId {
                    mechanism_id: PatternMechanismId(46),
                    ordinal: sites.len(),
                },
                position,
                nominal_cell_basis: NominalCellBasis::new(along, across)
                    .expect("finite rotated basis"),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::GuideIntersection {
                    contributors: vec![
                        GuideInstanceId::new(GuideDimensionId(21), column),
                        GuideInstanceId::new(GuideDimensionId(22), row),
                    ],
                },
            });
        }
    }
    let sites = FamilySiteSet::new(
        "maze-rotated-arrangement-sites".into(),
        PatternMechanismId(46),
        sites,
    )
    .expect("finite rotated sites");
    let axes = sites
        .iter()
        .flat_map(|site| match &site.provenance {
            FamilySiteProvenance::GuideIntersection { contributors } => {
                contributors.iter().copied()
            }
            _ => unreachable!("rotated maze fixture uses guide intersections"),
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|id| {
            MazeGuideAxis::new(id, if id.dimension_id == 21 { across } else { along })
                .expect("rotated guide tangent is finite")
        })
        .collect();
    (sites, axes)
}

/// Returns whether two canvas-side classifications are opposite for opening ranking evidence.
fn opposite_opening_sides(
    first: toniator_geometry::MazeOpeningSide,
    second: toniator_geometry::MazeOpeningSide,
) -> bool {
    matches!(
        (first, second),
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
    )
}

/// Reconstructs the finite evaluated-guide tangents that own each fixture contributor ordering.
fn arrangement_axes(sites: &FamilySiteSet, triangular: bool) -> Vec<MazeGuideAxis> {
    let contributors = sites
        .iter()
        .flat_map(|site| match &site.provenance {
            FamilySiteProvenance::GuideIntersection { contributors } => {
                contributors.iter().copied()
            }
            _ => unreachable!("maze fixtures contain guide intersections only"),
        })
        .collect::<std::collections::BTreeSet<_>>();
    contributors
        .into_iter()
        .map(|id| {
            let tangent = if triangular {
                match id.dimension_id {
                    1 => Vector2::new(0.0, 1.0),
                    2 => Vector2::new(-15.0, 5.0 * 3.0_f64.sqrt()),
                    3 => Vector2::new(15.0, 5.0 * 3.0_f64.sqrt()),
                    _ => unreachable!("triangular fixture has exactly three guide dimensions"),
                }
            } else {
                match id.dimension_id {
                    1 => Vector2::new(0.0, 1.0),
                    2 => Vector2::new(1.0, 0.0),
                    11 => Vector2::new(0.0, 1.0),
                    12 => Vector2::new(1.0, 0.0),
                    _ => unreachable!("quad fixture has known guide dimensions"),
                }
            };
            MazeGuideAxis::new(id, tangent).expect("fixture guide tangent is valid")
        })
        .collect()
}

/// Returns the only current conventional wall-maze program.
fn maze_program(seed: u32) -> MazeProgram {
    MazeProgram {
        algorithm: GridMazeAlgorithm::RecursiveBacktracker,
        seed,
    }
}

/// Supplies a document canvas that contains the finite arrangement fixtures without creating walls.
fn arrangement_canvas() -> Bounds {
    Bounds::new(Point2::new(-40.0, -40.0), Point2::new(40.0, 40.0))
        .expect("finite maze fixture canvas")
}

/// Supplies a broad canvas that retains the full mixed-area fixture before fringe selection.
fn mixed_arrangement_canvas() -> Bounds {
    Bounds::new(Point2::new(-40.0, -40.0), Point2::new(50.0, 60.0))
        .expect("finite mixed-area maze fixture canvas")
}

/// Supplies the broad canvas that keeps both disconnected-core fixture patches inclusive.
fn disconnected_arrangement_canvas() -> Bounds {
    Bounds::new(Point2::new(-50.0, -50.0), Point2::new(80.0, 80.0))
        .expect("finite disconnected maze fixture canvas")
}

/// Returns the shared degree-two square adjacency graph.
fn graph() -> toniator_geometry::SiteAdjacencyGraph {
    graph_at(&[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)], 2, 2.0)
}

/// Builds a mutual-nearest graph for explicit deterministic connection witnesses.
fn graph_at(
    points: &[(f64, f64)],
    maximum_degree: usize,
    maximum_distance: f64,
) -> toniator_geometry::SiteAdjacencyGraph {
    build_site_adjacency_cancellable(
        &sites_at(points),
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree,
            maximum_distance,
        },
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("graph succeeds")
}

/// Asserts that paths cover every selected edge exactly once without introducing synthetic edges.
fn assert_exact_edge_cover(paths: &toniator_geometry::ConnectionPathSet) {
    let mut emitted = paths
        .paths
        .iter()
        .flat_map(|path| path.vertices.windows(2))
        .map(|pair| toniator_geometry::SiteAdjacencyEdge {
            first: pair[0].min(pair[1]),
            second: pair[0].max(pair[1]),
        })
        .collect::<Vec<_>>();
    emitted.sort();
    assert_eq!(emitted, paths.selected_edges);
}

/// Returns valid shared authored adjacency intent.
fn adjacency() -> ConnectionAdjacencyIntent {
    ConnectionAdjacencyIntent {
        maximum_degree: 2,
        maximum_distance: 2.0,
    }
}

/// Fixes enabled default work bounds and geometry-owned program contract identifiers.
#[test]
fn defaults_and_program_contract_identifiers_are_exact() {
    assert_eq!(
        ConnectionPathLimits::default(),
        ConnectionPathLimits::new(1_048_576, 1_048_576, 2_097_152, 33_554_432)
            .expect("default limits remain enabled")
    );
    let programs = [
        (
            ConnectionProgram::NearestLinks {
                adjacency: adjacency(),
            },
            CONNECTION_NEAREST_SELECTION_CONTRACT_ID,
        ),
        (
            ConnectionProgram::RandomLinks {
                adjacency: adjacency(),
                minimum_degree: 1,
                seed: 1,
            },
            CONNECTION_RANDOM_SELECTION_CONTRACT_ID,
        ),
        (
            ConnectionProgram::GridSpanningTree {
                adjacency: adjacency(),
                algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
                seed: 1,
            },
            CONNECTION_PRIM_SELECTION_CONTRACT_ID,
        ),
    ];
    for (program, expected) in programs {
        assert_eq!(connection_program_contract_id(&program), expected);
    }
}

/// Proves connected square and in-canvas triangular guide arrangements retain only non-passage walls.
#[test]
fn conventional_maze_walls_use_actual_guide_arrangements_and_one_dual_spanning_tree() {
    for triangular in [false, true] {
        let (sites, canvas) = if triangular {
            boundary_aligned_arrangement(true)
        } else {
            (arrangement_sites(false), arrangement_canvas())
        };
        let result = build_maze_walls_cancellable(
            PatternOutputLayerId(71),
            &sites,
            canvas,
            &arrangement_axes(&sites, triangular),
            &maze_program(23),
            MazeLimits::default(),
            &|| false,
        )
        .expect("bounded guide arrangement builds a conventional wall maze");
        assert!(!result.source_walls.is_empty());
        assert!(!result.cells.is_empty());
        assert!(
            result.cells.iter().all(|cell| {
                if triangular {
                    cell.vertices.len() == 3
                } else {
                    cell.vertices.len() == 4
                }
            }),
            "triangular={triangular} cell vertex counts: {:?}",
            result
                .cells
                .iter()
                .map(|cell| cell.vertices.len())
                .collect::<Vec<_>>()
        );
        let source = result
            .source_walls
            .iter()
            .map(|wall| wall.id)
            .collect::<std::collections::BTreeSet<_>>();
        let passages = result
            .removed_passage_walls
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let retained = result
            .retained_walls
            .iter()
            .map(|wall| wall.id)
            .collect::<std::collections::BTreeSet<_>>();
        assert!(passages.is_subset(&source));
        let removed = passages
            .union(&std::collections::BTreeSet::from([
                result.entrance.wall,
                result.exit.wall,
            ]))
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            source
                .difference(&removed)
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            retained
        );
        assert_eq!(result.removed_passage_walls.len(), result.cells.len() - 1);
        if triangular {
            assert!(opposite_opening_sides(
                result.entrance.side,
                result.exit.side
            ));
        }
        let solution = &result.solution;
        assert_eq!(solution.cells.first(), Some(&solution.entrance));
        assert_eq!(solution.cells.last(), Some(&solution.exit));
        assert_eq!(solution.passage_walls.len() + 1, solution.cells.len());
        assert!(
            solution
                .passage_walls
                .iter()
                .all(|wall| passages.contains(wall))
        );
        assert!(result.source_walls.iter().all(|wall| {
            result
                .cells
                .iter()
                .any(|cell| cell.walls.contains(&wall.id))
        }));
        assert!(result.retained_walls.iter().all(|wall| {
            result
                .cells
                .iter()
                .any(|cell| cell.walls.contains(&wall.id))
        }));
        let mut reached = std::collections::BTreeSet::from([result.entrance.cell]);
        loop {
            let mut changed = false;
            for edge in &result.dual_edges {
                if passages.contains(&edge.shared_wall)
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
        assert_eq!(reached.len(), result.cells.len());
        assert!(result.diagnostics.off_solution_cells > 0);
        assert!(result.diagnostics.dead_end_cells > 0);
        for opening in [result.entrance, result.exit] {
            assert_eq!(
                result
                    .cells
                    .iter()
                    .filter(|cell| cell.walls.contains(&opening.wall))
                    .count(),
                1,
                "openings are perimeter walls of exactly one bounded cell"
            );
        }
        let replay = build_maze_walls_cancellable(
            PatternOutputLayerId(71),
            &sites,
            canvas,
            &arrangement_axes(&sites, triangular),
            &maze_program(23),
            MazeLimits::default(),
            &|| false,
        )
        .expect("same seed replays");
        assert_eq!(result, replay);
    }
}

/// Classifies rotated arrangement openings from normalized canvas sides without axis assumptions.
#[test]
fn rotated_maze_openings_use_canvas_side_ranking_without_axis_assumptions() {
    let (sites, axes) = rotated_quad_arrangement();
    let maze = build_maze_walls_cancellable(
        PatternOutputLayerId(81),
        &sites,
        arrangement_canvas(),
        &axes,
        &maze_program(23),
        MazeLimits::default(),
        &|| false,
    )
    .expect("rotated connected arrangement builds");
    assert_ne!(maze.entrance.wall, maze.exit.wall);
    let directions = [
        Vector2::new(5.0 * 3.0_f64.sqrt(), 5.0),
        Vector2::new(-5.0, 5.0 * 3.0_f64.sqrt()),
    ];
    assert!(maze.source_walls.iter().all(|wall| {
        let first = maze.source_site_positions[&wall.id.first];
        let second = maze.source_site_positions[&wall.id.second];
        let edge = Vector2::new(second.x - first.x, second.y - first.y);
        directions
            .iter()
            .any(|axis| (edge.x * axis.y - edge.y * axis.x).abs() <= edge.x.hypot(edge.y) * 1e-10)
    }));
}

/// Discards disconnected smaller bounded-face components before selecting the largest component.
///
/// Inclusive source sites and the fingerprint preserve all finite candidate patches, but only the
/// largest connected bounded-face component reaches the dual/tree invariant and output walls.
#[test]
fn maze_discards_disconnected_smaller_face_components_before_constructing_one_tree() {
    let (sites, axes) = disconnected_dual_sites();
    let maze = build_maze_walls_cancellable(
        PatternOutputLayerId(79),
        &sites,
        disconnected_arrangement_canvas(),
        &axes,
        &maze_program(23),
        MazeLimits::default(),
        &|| false,
    )
    .expect("disconnected smaller components do not reject the largest bounded-face maze");
    assert_eq!(maze.source_sites.len(), sites.len());
    assert_eq!(maze.cells.len(), 25);
    assert_eq!(maze.removed_passage_walls.len(), maze.cells.len() - 1);
    assert!(maze.dual_edges.iter().all(|edge| {
        maze.cells.iter().any(|cell| cell.id == edge.first)
            && maze.cells.iter().any(|cell| cell.id == edge.second)
    }));
    assert!(
        maze.source_walls
            .iter()
            .all(|wall| { maze.cells.iter().any(|cell| cell.walls.contains(&wall.id)) })
    );
    let core_only_sites = FamilySiteSet::new(
        "maze-disconnected-dual-sites".into(),
        PatternMechanismId(47),
        sites.sites()[..36].to_vec(),
    )
    .expect("complete candidate patch remains finite");
    let core_only = build_maze_walls_cancellable(
        PatternOutputLayerId(79),
        &core_only_sites,
        disconnected_arrangement_canvas(),
        &axes,
        &maze_program(23),
        MazeLimits::default(),
        &|| false,
    )
    .expect("complete candidate patch evaluates independently");
    assert_eq!(maze.cells, core_only.cells);
    assert_eq!(maze.source_walls, core_only.source_walls);
    assert_ne!(maze.fingerprint(), core_only.fingerprint());
    let replay = build_maze_walls_cancellable(
        PatternOutputLayerId(79),
        &sites,
        disconnected_arrangement_canvas(),
        &axes,
        &maze_program(23),
        MazeLimits::default(),
        &|| false,
    );
    assert_eq!(maze, replay.expect("same candidate arrangement replays"));
}

/// Retains inclusive guard-filtered boundary sites and every bounded face in their component.
///
/// Every site on or inside the canvas remains explicit source-arrangement authority. The emitted
/// maze retains all 36 connected bounded square faces; only source-wall fragments with no bounded
/// face remain absent from output.
#[test]
fn maze_canvas_filter_excludes_outside_sites_and_dangling_wall_fragments() {
    let sites = arrangement_sites(false);
    let canvas = Bounds::new(Point2::new(-25.0, -25.0), Point2::new(45.0, 45.0))
        .expect("finite in-canvas square");
    let maze = build_maze_walls_cancellable(
        PatternOutputLayerId(80),
        &sites,
        canvas,
        &arrangement_axes(&sites, false),
        &maze_program(7),
        MazeLimits::default(),
        &|| false,
    )
    .expect("inclusive source arrangement leaves a connected bounded-face component");
    assert_eq!(maze.source_sites.len(), 49);
    assert!(
        maze.source_sites
            .iter()
            .all(|site| canvas.contains(site.source.position))
    );
    assert_eq!(maze.cells.len(), 36);
    assert_eq!(maze.removed_passage_walls.len(), 35);
    assert_ne!(maze.entrance.wall, maze.exit.wall);
    assert_eq!(maze.solution.cells.first(), Some(&maze.entrance.cell));
    assert_eq!(maze.solution.cells.last(), Some(&maze.exit.cell));
    assert_eq!(
        maze.solution.passage_walls.len() + 1,
        maze.solution.cells.len()
    );
    assert!(
        maze.source_walls
            .iter()
            .all(|wall| maze.cells.iter().any(|cell| cell.walls.contains(&wall.id)))
    );
    assert!(
        maze.retained_walls
            .iter()
            .all(|wall| maze.cells.iter().any(|cell| cell.walls.contains(&wall.id)))
    );
    for opening in [maze.entrance, maze.exit] {
        assert_eq!(
            maze.cells
                .iter()
                .filter(|cell| cell.walls.contains(&opening.wall))
                .count(),
            1
        );
    }
}

/// Preserves one inclusive boundary-aligned bounded cell as a valid maze with two openings.
///
/// This exercises the bounded-face singleton selection without changing inclusive source-site
/// authority or synthesizing canvas geometry: the one derived solution contains that cell and no
/// passage wall.
#[test]
fn single_cell_maze_keeps_two_openings_and_one_cell_solution() {
    let sites = FamilySiteSet::new(
        "singleton-maze-arrangement".into(),
        PatternMechanismId(49),
        [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0), (10.0, 10.0)]
            .into_iter()
            .enumerate()
            .map(|(ordinal, (x, y))| FamilySite {
                id: FamilySiteId {
                    mechanism_id: PatternMechanismId(49),
                    ordinal,
                },
                position: Point2::new(x, y),
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(10.0, 0.0),
                    Vector2::new(0.0, 10.0),
                )
                .expect("finite singleton basis"),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::GuideIntersection {
                    contributors: vec![
                        GuideInstanceId::new(GuideDimensionId(1), ordinal as i64 % 2),
                        GuideInstanceId::new(GuideDimensionId(2), ordinal as i64 / 2),
                    ],
                },
            })
            .collect(),
    )
    .expect("finite singleton guide intersections");
    let canvas = Bounds::new(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
        .expect("finite singleton canvas");
    let maze = build_maze_walls_cancellable(
        PatternOutputLayerId(86),
        &sites,
        canvas,
        &arrangement_axes(&sites, false),
        &maze_program(5),
        MazeLimits::default(),
        &|| false,
    )
    .expect("inclusive boundary singleton maze remains valid");
    assert_eq!(maze.cells.len(), 1);
    assert_ne!(maze.entrance.wall, maze.exit.wall);
    assert_eq!(maze.solution.cells, vec![maze.entrance.cell]);
    assert!(maze.solution.passage_walls.is_empty());
}

/// Keeps every square and triangular bounded face from inclusive boundary guide families.
///
/// Each fixture supplies an actual guide lattice at or only one half unit from the canvas
/// edge. The returned maze preserves every in-canvas source site and every bounded face in the
/// one connected component without a site inset, fixed ring, or stroke-width eligibility rule.
#[test]
fn boundary_aligned_maze_sites_retain_all_connected_bounded_faces() {
    for (triangular, near_boundary) in [(false, false), (true, false), (false, true), (true, true)]
    {
        let (sites, canvas) = if near_boundary {
            near_boundary_arrangement(triangular)
        } else {
            boundary_aligned_arrangement(triangular)
        };
        if near_boundary {
            assert!(sites.iter().any(|site| {
                let clearance = (site.position.x - canvas.min.x)
                    .min(canvas.max.x - site.position.x)
                    .min(site.position.y - canvas.min.y)
                    .min(canvas.max.y - site.position.y);
                clearance <= 0.5
            }));
        } else {
            assert!(
                sites.iter().any(|site| {
                    site.position.x == canvas.min.x
                        || site.position.x == canvas.max.x
                        || site.position.y == canvas.min.y
                        || site.position.y == canvas.max.y
                }),
                "fixture places actual sites on the authoritative canvas boundary"
            );
        }
        let maze = build_maze_walls_cancellable(
            PatternOutputLayerId(match (triangular, near_boundary) {
                (false, false) => 82,
                (true, false) => 83,
                (false, true) => 84,
                (true, true) => 85,
            }),
            &sites,
            canvas,
            &arrangement_axes(&sites, triangular),
            &maze_program(31),
            MazeLimits::default(),
            &|| false,
        )
        .expect("inclusive connected arrangement evaluates");
        assert_eq!(
            maze.source_sites
                .iter()
                .map(|site| site.source.id)
                .collect::<BTreeSet<_>>(),
            sites.iter().map(|site| site.id).collect::<BTreeSet<_>>(),
            "every inclusive candidate site remains inspectable arrangement authority"
        );
        assert!(
            maze.source_sites
                .iter()
                .all(|site| canvas.contains(site.source.position)),
            "inclusive source identity preserves every admitted boundary site"
        );
        let candidate_vertices = maze
            .source_sites
            .iter()
            .map(|site| site.id)
            .collect::<BTreeSet<_>>();
        let emitted_vertices = maze
            .wall_paths
            .iter()
            .flat_map(|wall| wall.vertices)
            .collect::<BTreeSet<_>>();
        assert!(emitted_vertices.is_subset(&candidate_vertices));
        assert!(
            maze.source_walls
                .iter()
                .all(|wall| { maze.cells.iter().any(|cell| cell.walls.contains(&wall.id)) })
        );
        assert_eq!(
            maze.cells.len(),
            if triangular { 128 } else { 36 },
            "every connected bounded face remains in the selected component"
        );
        assert_eq!(maze.removed_passage_walls.len(), maze.cells.len() - 1);
        assert_ne!(maze.entrance.wall, maze.exit.wall);
    }
}

/// Keeps every bounded mixed-area face while excluding only the oppositely wound exterior face.
#[test]
fn maze_faces_preserve_mixed_area_cells_without_canvas_edges() {
    let sites = mixed_quad_arrangement_sites();
    let maze = build_maze_walls_cancellable(
        PatternOutputLayerId(72),
        &sites,
        mixed_arrangement_canvas(),
        &arrangement_axes(&sites, false),
        &maze_program(3),
        MazeLimits::default(),
        &|| false,
    )
    .expect("mixed-area arrangement evaluates");
    assert_eq!(maze.cells.len(), 36);
    assert!(maze.cells.iter().all(|cell| cell.vertices.len() == 4));
    assert_eq!(maze.dual_edges.len(), 60);
    let expected_basis = 13.0_f64.sqrt();
    assert!(maze.wall_paths.iter().all(|path| {
        (path.nominal_basis - expected_basis).abs() < 1e-12
            && path.path.segments().iter().all(|segment| {
                (segment.end().x - segment.start().x).hypot(segment.end().y - segment.start().y)
                    != path.nominal_basis
            })
    }));
}

/// Uses one four-cell cycle to prove the maze selector advances one branch before backtracking.
#[test]
fn maze_recursive_backtracker_does_not_eagerly_select_all_root_neighbors() {
    let sites = mixed_quad_arrangement_sites();
    let maze = build_maze_walls_cancellable(
        PatternOutputLayerId(73),
        &sites,
        mixed_arrangement_canvas(),
        &arrangement_axes(&sites, false),
        &maze_program(19),
        MazeLimits::default(),
        &|| false,
    )
    .expect("four-cell maze evaluates");
    let first = maze.cells[0].id;
    let root_degree = maze
        .dual_edges
        .iter()
        .filter(|edge| {
            maze.removed_passage_walls.contains(&edge.shared_wall)
                && (edge.first == first || edge.second == first)
        })
        .count();
    assert_eq!(root_degree, 1);
    let replay = build_maze_walls_cancellable(
        PatternOutputLayerId(73),
        &sites,
        mixed_arrangement_canvas(),
        &arrangement_axes(&sites, false),
        &maze_program(19),
        MazeLimits::default(),
        &|| false,
    )
    .expect("same seed replays");
    assert_eq!(maze.removed_passage_walls, replay.removed_passage_walls);
    let larger = arrangement_sites(false);
    let baseline = build_maze_walls_cancellable(
        PatternOutputLayerId(74),
        &larger,
        arrangement_canvas(),
        &arrangement_axes(&larger, false),
        &maze_program(19),
        MazeLimits::default(),
        &|| false,
    )
    .expect("larger baseline maze evaluates");
    let alternative_exists = (20..=99).any(|seed| {
        build_maze_walls_cancellable(
            PatternOutputLayerId(74),
            &larger,
            arrangement_canvas(),
            &arrangement_axes(&larger, false),
            &maze_program(seed),
            MazeLimits::default(),
            &|| false,
        )
        .expect("alternate seeded maze evaluates")
        .removed_passage_walls
            != baseline.removed_passage_walls
    });
    assert!(
        alternative_exists,
        "different seeds alter a nontrivial dual maze"
    );
}

/// Validates every maze work limit at construction time and at its bounded runtime authority.
#[test]
fn maze_limits_reject_zero_and_each_runtime_category() {
    for zeroed in 0..7 {
        let mut values = [1_usize; 7];
        values[zeroed] = 0;
        assert_eq!(
            MazeLimits::new(
                values[0], values[1], values[2], values[3], values[4], values[5], values[6],
            )
            .expect_err("a disabled maze budget rejects")
            .path(),
            "maze.limits"
        );
    }
    let sites = mixed_quad_arrangement_sites();
    let axes = arrangement_axes(&sites, false);
    let categories: [(&str, MazeLimitConfigurator); 7] = [
        ("maze.limits.source_walls", |limits| {
            limits.maximum_source_walls = 1
        }),
        ("maze.limits.faces", |limits| limits.maximum_faces = 1),
        ("maze.limits.dual_adjacencies", |limits| {
            limits.maximum_dual_adjacencies = 1
        }),
        ("maze.limits.passages", |limits| limits.maximum_passages = 1),
        ("maze.limits.wall_trails", |limits| {
            limits.maximum_wall_trails = 1
        }),
        ("maze.limits.points", |limits| {
            limits.maximum_retained_points = 1
        }),
        ("maze.limits.inspections", |limits| {
            limits.maximum_inspections = 1
        }),
    ];
    for (expected, configure) in categories {
        let mut limits = MazeLimits::default();
        configure(&mut limits);
        assert_eq!(
            build_maze_walls_cancellable(
                PatternOutputLayerId(75),
                &sites,
                mixed_arrangement_canvas(),
                &axes,
                &maze_program(23),
                limits,
                &|| false,
            )
            .expect_err("bounded maze category rejects atomically")
            .path(),
            expected
        );
    }
}

/// Polls cancellation throughout arrangement, dual selection, retained-wall, and fingerprint work.
#[test]
fn maze_cancellation_remains_atomic_across_construction_stages() {
    let sites = arrangement_sites(true);
    let axes = arrangement_axes(&sites, true);
    for stop_after in [1_usize, 24, 96] {
        let polls = Cell::new(0_usize);
        let error = build_maze_walls_cancellable(
            PatternOutputLayerId(76),
            &sites,
            arrangement_canvas(),
            &axes,
            &maze_program(23),
            MazeLimits::default(),
            &|| {
                let next = polls.get() + 1;
                polls.set(next);
                next > stop_after
            },
        )
        .expect_err("cancelled maze returns no partial result");
        assert_eq!(error.path(), "evaluation.cancelled");
    }
}

/// Uses the smallest contributing site diameter as the connection trail's nominal basis.
#[test]
fn connection_path_nominal_basis_is_the_minimum_site_diameter() {
    let bases = [
        (Vector2::new(2.0, 0.0), Vector2::new(0.0, 2.0)),
        (Vector2::new(1.0, 0.0), Vector2::new(0.0, 1.0)),
    ];
    let sites = sites_with_bases(&[(0.0, 0.0), (1.0, 0.0)], &bases);
    let graph = build_site_adjacency_cancellable(
        &sites,
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 1,
            maximum_distance: 2.0,
        },
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("graph builds");
    let paths = build_connection_paths_cancellable(
        PatternOutputLayerId(17),
        &graph,
        &ConnectionProgram::NearestLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 1,
                maximum_distance: 2.0,
            },
        },
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect("path builds");
    assert_eq!(paths.paths.len(), 1);
    assert_eq!(
        paths.paths[0].nominal_basis,
        NominalCellBasis::new(bases[1].0, bases[1].1)
            .expect("minimum basis")
            .diameter()
    );
}

/// Reports isolated and best-effort under-connected nodes without changing selected path identity.
#[test]
fn random_links_reports_under_connected_nodes_and_isolates() {
    let graph = graph_at(&[(0.0, 0.0), (1.0, 0.0), (10.0, 0.0)], 1, 1.1);
    let paths = build_connection_paths_cancellable(
        PatternOutputLayerId(18),
        &graph,
        &ConnectionProgram::RandomLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 1,
                maximum_distance: 1.1,
            },
            minimum_degree: 1,
            seed: 3,
        },
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect("best-effort selection builds");
    let isolated = FamilySiteId {
        mechanism_id: PatternMechanismId(9),
        ordinal: 2,
    };
    assert_eq!(paths.diagnostics.isolated_nodes, vec![isolated]);
    assert_eq!(paths.diagnostics.under_connected_nodes, vec![isolated]);
    assert_exact_edge_cover(&paths);
}

/// Distinguishes authored seeds and output IDs even when they select the same one-edge topology.
#[test]
fn fingerprint_tracks_program_and_output_but_ignores_sufficient_limits() {
    let graph = graph_at(&[(0.0, 0.0), (1.0, 0.0)], 1, 2.0);
    let program = |seed| ConnectionProgram::RandomLinks {
        adjacency: ConnectionAdjacencyIntent {
            maximum_degree: 1,
            maximum_distance: 2.0,
        },
        minimum_degree: 1,
        seed,
    };
    let first = build_connection_paths_cancellable(
        PatternOutputLayerId(19),
        &graph,
        &program(1),
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect("first path");
    let different_seed = build_connection_paths_cancellable(
        PatternOutputLayerId(19),
        &graph,
        &program(2),
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect("different-seed path");
    assert_eq!(first.selected_edges, different_seed.selected_edges);
    assert_ne!(first.fingerprint(), different_seed.fingerprint());
    let sufficient_limits = build_connection_paths_cancellable(
        PatternOutputLayerId(19),
        &graph,
        &program(1),
        ConnectionPathLimits::new(8, 8, 8, 8_192).expect("sufficient limits"),
        &|| false,
    )
    .expect("sufficient-limit path");
    assert_eq!(first.fingerprint(), sufficient_limits.fingerprint());
    let different_output = build_connection_paths_cancellable(
        PatternOutputLayerId(20),
        &graph,
        &program(1),
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect("different-output path");
    assert_ne!(first.fingerprint(), different_output.fingerprint());
}

/// Polls cancellation during all seeded selection families before partial paths can publish.
#[test]
fn seeded_programs_observe_cancellation_during_bounded_selection_work() {
    let graph = graph();
    for program in [
        ConnectionProgram::RandomLinks {
            adjacency: adjacency(),
            minimum_degree: 1,
            seed: 1,
        },
        ConnectionProgram::GridSpanningTree {
            adjacency: adjacency(),
            algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
            seed: 1,
        },
    ] {
        let checks = Cell::new(0_u32);
        let error = build_connection_paths_cancellable(
            PatternOutputLayerId(21),
            &graph,
            &program,
            ConnectionPathLimits::default(),
            &|| {
                checks.set(checks.get() + 1);
                checks.get() > 6
            },
        )
        .expect_err("cancellation rejects");
        assert_eq!(error.path(), "evaluation.cancelled");
    }
}

/// Covers all four authored selections and proves every emitted path is open positive line geometry.
#[test]
fn programs_are_deterministic_and_emit_open_paths() {
    let programs = [
        ConnectionProgram::NearestLinks {
            adjacency: adjacency(),
        },
        ConnectionProgram::RandomLinks {
            adjacency: adjacency(),
            minimum_degree: 1,
            seed: 7,
        },
        ConnectionProgram::GridSpanningTree {
            adjacency: adjacency(),
            algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
            seed: 7,
        },
    ];
    for program in programs {
        let first = build_connection_paths_cancellable(
            PatternOutputLayerId(11),
            &graph(),
            &program,
            ConnectionPathLimits::default(),
            &|| false,
        )
        .expect("program succeeds");
        let second = build_connection_paths_cancellable(
            PatternOutputLayerId(11),
            &graph(),
            &program,
            ConnectionPathLimits::default(),
            &|| false,
        )
        .expect("replay succeeds");
        assert_eq!(first, second);
        assert_exact_edge_cover(&first);
        for path in &first.paths {
            assert_eq!(path.path.closure(), toniator_geometry::PathClosure::Open);
            assert!(
                path.path
                    .segments()
                    .iter()
                    .all(|segment| matches!(segment, toniator_geometry::CurveSegment::Line(_)))
            );
        }
    }
}

/// Proves disconnected, cyclic, and branched selections retain deterministic minimum open trails.
#[test]
fn trails_cover_disconnected_cycles_and_branches_once_with_stable_ids() {
    let disconnected = graph_at(&[(0.0, 0.0), (1.0, 0.0), (10.0, 0.0), (11.0, 0.0)], 1, 2.0);
    let disconnected_paths = build_connection_paths_cancellable(
        PatternOutputLayerId(12),
        &disconnected,
        &ConnectionProgram::NearestLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 1,
                maximum_distance: 2.0,
            },
        },
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect("disconnected paths");
    assert_eq!(disconnected_paths.paths.len(), 2);
    assert_exact_edge_cover(&disconnected_paths);
    assert!(
        disconnected_paths
            .paths
            .windows(2)
            .all(|pair| pair[0].id <= pair[1].id)
    );
    assert!(disconnected_paths.paths.iter().all(|path| {
        path.id.component_minimum == path.vertices.iter().copied().min().expect("path vertex")
    }));

    let cycle_paths = build_connection_paths_cancellable(
        PatternOutputLayerId(12),
        &graph(),
        &ConnectionProgram::NearestLinks {
            adjacency: adjacency(),
        },
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect("cycle paths");
    assert_eq!(
        cycle_paths.paths.len(),
        2,
        "an all-even component splits open"
    );
    assert_exact_edge_cover(&cycle_paths);

    let branch = graph_at(&[(0.0, 0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0)], 3, 2.0);
    let branch_paths = build_connection_paths_cancellable(
        PatternOutputLayerId(12),
        &branch,
        &ConnectionProgram::NearestLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 3,
                maximum_distance: 2.0,
            },
        },
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect("branch paths");
    assert_eq!(
        branch_paths.paths.len(),
        2,
        "four odd vertices require two trails"
    );
    assert_exact_edge_cover(&branch_paths);
}

/// Proves seeded programs retain replay identity while eventually selecting a distinct topology.
#[test]
fn seeded_programs_change_identity_and_selection_without_changing_input_graph() {
    let graph = graph_at(
        &[
            (0.0, 0.0),
            (1.0, 0.0),
            (2.0, 0.0),
            (0.0, 1.0),
            (1.0, 1.0),
            (2.0, 1.0),
        ],
        4,
        2.0,
    );
    let adjacency = ConnectionAdjacencyIntent {
        maximum_degree: 4,
        maximum_distance: 2.0,
    };
    for kind in 0..2 {
        let program = |seed| match kind {
            0 => ConnectionProgram::RandomLinks {
                adjacency,
                minimum_degree: 1,
                seed,
            },
            _ => ConnectionProgram::GridSpanningTree {
                adjacency,
                algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
                seed,
            },
        };
        let first = build_connection_paths_cancellable(
            PatternOutputLayerId(13),
            &graph,
            &program(1),
            ConnectionPathLimits::default(),
            &|| false,
        )
        .expect("seed one");
        let replay = build_connection_paths_cancellable(
            PatternOutputLayerId(13),
            &graph,
            &program(1),
            ConnectionPathLimits::default(),
            &|| false,
        )
        .expect("seed replay");
        assert_eq!(first, replay);
        assert!(
            (2..64).any(|seed| {
                build_connection_paths_cancellable(
                    PatternOutputLayerId(13),
                    &graph,
                    &program(seed),
                    ConnectionPathLimits::default(),
                    &|| false,
                )
                .is_ok_and(|candidate| candidate.fingerprint() != first.fingerprint())
            }),
            "a nontrivial graph exposes a different-seed witness"
        );
    }
}

/// Proves seed one with an authored zero minimum still distributes positive connection targets
/// across a nontrivial graph instead of collapsing every site to degree zero.
#[test]
fn zero_minimum_random_links_with_seed_one_do_not_collapse_to_empty() {
    let graph = graph_at(
        &[
            (0.0, 0.0),
            (1.0, 0.0),
            (2.0, 0.0),
            (0.0, 1.0),
            (1.0, 1.0),
            (2.0, 1.0),
        ],
        3,
        2.0,
    );
    let paths = build_connection_paths_cancellable(
        PatternOutputLayerId(14),
        &graph,
        &ConnectionProgram::RandomLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 3,
                maximum_distance: 2.0,
            },
            minimum_degree: 0,
            seed: 1,
        },
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect("zero-minimum random links build");
    assert!(
        !paths.selected_edges.is_empty(),
        "seed one must not suppress the complete eligible graph"
    );
    assert_exact_edge_cover(&paths);
}

/// Proves policy mismatch, cancellation, and bounded selected-edge failures are atomic stable diagnostics.
#[test]
fn policy_cancellation_and_limits_are_stable() {
    let program = ConnectionProgram::NearestLinks {
        adjacency: adjacency(),
    };
    let mismatch = build_connection_paths_cancellable(
        PatternOutputLayerId(11),
        &build_site_adjacency_cancellable(
            &sites(),
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: 1,
                maximum_distance: 2.0,
            },
            SiteAdjacencyLimits::default(),
            &|| false,
        )
        .expect("graph"),
        &program,
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect_err("mismatch rejects");
    assert_eq!(mismatch.path(), "connection.graph.policy");
    let checks = Cell::new(0_u32);
    let cancelled = build_connection_paths_cancellable(
        PatternOutputLayerId(11),
        &graph(),
        &program,
        ConnectionPathLimits::default(),
        &|| {
            checks.set(checks.get() + 1);
            checks.get() > 2
        },
    )
    .expect_err("cancellation rejects");
    assert_eq!(cancelled.path(), "evaluation.cancelled");
    let limited = build_connection_paths_cancellable(
        PatternOutputLayerId(11),
        &graph(),
        &program,
        ConnectionPathLimits::new(1, 8, 32, 128).expect("enabled limits"),
        &|| false,
    )
    .expect_err("edge limit rejects");
    assert_eq!(limited.path(), "connection.limits.selected_edges");
}

/// Proves random selection decomposes by selected-edge components rather than the source graph.
#[test]
fn disconnected_random_selected_components_cover_every_edge_once_in_stable_order() {
    let graph = graph_at(
        &[
            (0.0, 0.0),
            (1.0, 0.0),
            (2.0, 0.0),
            (3.0, 0.0),
            (4.0, 0.0),
            (5.0, 0.0),
        ],
        2,
        1.1,
    );
    let paths = build_connection_paths_cancellable(
        PatternOutputLayerId(14),
        &graph,
        &ConnectionProgram::RandomLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 2,
                maximum_distance: 1.1,
            },
            minimum_degree: 0,
            seed: 0,
        },
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect("selected components build");
    assert_exact_edge_cover(&paths);
    let components = paths
        .paths
        .iter()
        .map(|path| (path.id.component_minimum, path.id.component_ordinal))
        .collect::<Vec<_>>();
    assert!(components.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(components.windows(2).any(|pair| pair[0].0 != pair[1].0));
    for path in &paths.paths {
        assert_eq!(
            path.id.component_minimum,
            *path.vertices.iter().min().expect("path has vertices")
        );
    }
}

/// Preserves domain-owned invalid program paths and rejects each disabled connection work limit.
#[test]
fn validation_and_each_work_limit_report_stable_paths() {
    let graph = graph();
    for program in [
        ConnectionProgram::NearestLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 0,
                maximum_distance: 1.0,
            },
        },
        ConnectionProgram::NearestLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 1,
                maximum_distance: 0.0,
            },
        },
        ConnectionProgram::RandomLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 1,
                maximum_distance: 1.0,
            },
            minimum_degree: 2,
            seed: 0,
        },
    ] {
        let error = build_connection_paths_cancellable(
            PatternOutputLayerId(15),
            &graph,
            &program,
            ConnectionPathLimits::default(),
            &|| false,
        )
        .expect_err("invalid program rejects");
        assert!(error.path().starts_with("connection."));
        assert_ne!(error.path(), "connection.program");
    }
    for limits in [
        ConnectionPathLimits {
            maximum_selected_edges: 0,
            ..ConnectionPathLimits::default()
        },
        ConnectionPathLimits {
            maximum_trails: 0,
            ..ConnectionPathLimits::default()
        },
        ConnectionPathLimits {
            maximum_retained_path_points: 0,
            ..ConnectionPathLimits::default()
        },
        ConnectionPathLimits {
            maximum_inspections: 0,
            ..ConnectionPathLimits::default()
        },
    ] {
        let error = build_connection_paths_cancellable(
            PatternOutputLayerId(15),
            &graph,
            &ConnectionProgram::NearestLinks {
                adjacency: adjacency(),
            },
            limits,
            &|| false,
        )
        .expect_err("disabled limit rejects");
        assert_eq!(error.path(), "connection.limits");
    }
}

/// Enforces each positive runtime limit before returning selected paths or partial identity.
#[test]
fn selected_trail_point_and_inspection_limits_fail_individually() {
    let program = ConnectionProgram::NearestLinks {
        adjacency: adjacency(),
    };
    for (limits, expected) in [
        (
            ConnectionPathLimits::new(1, 64, 64, 4_096).expect("enabled limits"),
            "connection.limits.selected_edges",
        ),
        (
            ConnectionPathLimits::new(64, 1, 64, 4_096).expect("enabled limits"),
            "connection.limits.trails",
        ),
        (
            ConnectionPathLimits::new(64, 64, 2, 4_096).expect("enabled limits"),
            "connection.limits.path_points",
        ),
        (
            ConnectionPathLimits::new(64, 64, 64, 1).expect("enabled limits"),
            "connection.limits.inspections",
        ),
    ] {
        let error = build_connection_paths_cancellable(
            PatternOutputLayerId(16),
            &graph(),
            &program,
            limits,
            &|| false,
        )
        .expect_err("limit rejects before publication");
        assert_eq!(error.path(), expected);
    }
}

/// Rejects an otherwise positive adjacency edge that the shared curve authority classifies as stationary.
#[test]
fn micro_edges_fail_at_connection_geometry_before_stroke_realization() {
    let adjacency = ConnectionAdjacencyIntent {
        maximum_degree: 1,
        maximum_distance: 1.0,
    };
    let graph = build_site_adjacency_cancellable(
        &sites_at(&[(0.0, 0.0), (1.0e-10, 0.0)]),
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: adjacency.maximum_degree as usize,
            maximum_distance: adjacency.maximum_distance,
        },
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("adjacency preserves distinct finite sites");
    let error = build_connection_paths_cancellable(
        PatternOutputLayerId(11),
        &graph,
        &ConnectionProgram::NearestLinks { adjacency },
        ConnectionPathLimits::default(),
        &|| false,
    )
    .expect_err("connection rejects a stationary centerline");
    assert_eq!(error.path(), "connection.geometry");
}
