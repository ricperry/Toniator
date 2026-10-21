//! Current final-topology, rescue, structural singleton and bounded-work witnesses.
use std::cell::Cell;
use toniator_domain::{
    ConnectionAdjacencyIntent, ConnectionCyclePolicy, ConnectionProgram, ConnectionSelectionIntent,
    GridSpanningTreeAlgorithm, PatternMechanismId, PatternOutputLayerId,
};
use toniator_geometry::*;

/// Creates original emission IDs and unit nominal basis without graph-specific fixtures.
/// # Panics
/// Panics if finite sites violate current family authority.
fn sites(points: &[(f64, f64)]) -> FamilySiteSet {
    FamilySiteSet::new(
        "artistic-graph".into(),
        PatternMechanismId(9),
        points
            .iter()
            .enumerate()
            .map(|(ordinal, (x, y))| FamilySite {
                id: FamilySiteId {
                    mechanism_id: PatternMechanismId(9),
                    ordinal,
                },
                position: Point2::new(*x, *y),
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(1.0, 0.0),
                    Vector2::new(0.0, 1.0),
                )
                .unwrap(),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: ordinal,
                    accepted_ordinal: ordinal,
                    exclusion_neighbor_ordinal: None,
                },
            })
            .collect(),
    )
    .unwrap()
}
/// Builds exact candidate adjacency for one caller-selected reach and neighbor count.
/// # Panics
/// Panics if the finite candidate graph fails.
fn graph(sites: &FamilySiteSet, degree: u32, distance: f64) -> SiteAdjacencyGraph {
    build_site_adjacency_cancellable(
        sites,
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: degree as usize,
            maximum_distance: distance,
        },
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .unwrap()
}
/// Selects current positive topology under the given independent final policy.
/// # Panics
/// Panics if valid finite topology fails atomic construction.
fn run(
    graph: &SiteAdjacencyGraph,
    program: &ConnectionProgram,
    policy: ConnectionSelectionIntent,
) -> ConnectionPathSet {
    build_connection_paths_with_selection_cancellable(
        PatternOutputLayerId(11),
        graph,
        program,
        policy,
        ConnectionPathLimits::default(),
        &|| false,
    )
    .unwrap()
}
/// Constructs the authored adjacency matching a supplied candidate graph.
fn adjacency(degree: u32, distance: f64) -> ConnectionAdjacencyIntent {
    ConnectionAdjacencyIntent {
        maximum_degree: degree,
        maximum_distance: distance,
    }
}
/// Counts selected graph degree and checks forest identity independently of path decomposition.
/// # Panics
/// Panics on excess degree, repeated edges or a graph cycle when forbidden.
fn invariants(output: &ConnectionPathSet, cap: u32, forest: bool) {
    let mut degree = std::collections::BTreeMap::new();
    let mut sets = std::collections::BTreeMap::new();
    for edge in &output.selected_edges {
        for node in [edge.first, edge.second] {
            *degree.entry(node).or_insert(0u32) += 1;
            sets.entry(node).or_insert(node);
        }
        let root =
            |mut id: FamilySiteId,
             sets: &std::collections::BTreeMap<FamilySiteId, FamilySiteId>| {
                while sets[&id] != id {
                    id = sets[&id];
                }
                id
            };
        let a = root(edge.first, &sets);
        let b = root(edge.second, &sets);
        if forest {
            assert_ne!(a, b);
        }
        sets.insert(a, b);
    }
    assert!(degree.values().all(|degree| *degree <= cap));
    let segments: usize = output
        .paths
        .iter()
        .map(|path| path.vertices.len() - 1)
        .sum();
    assert_eq!(segments, output.selected_edges.len());
}
/// Exercises final degree/cycle restrictions for nearest, seeded random and constrained Prim forests.
/// # Panics
/// Panics if deterministic replay or independently measured graph invariants fail.
#[test]
fn all_programs_obey_independent_final_limits_and_replay() {
    let sites = sites(&[
        (0., 0.),
        (1., 0.),
        (0., 1.),
        (1., 1.),
        (2., 0.),
        (2., 1.),
        (8., 8.),
    ]);
    let graph = graph(&sites, 6, 4.);
    for program in [
        ConnectionProgram::NearestLinks {
            adjacency: adjacency(6, 4.),
        },
        ConnectionProgram::RandomLinks {
            adjacency: adjacency(6, 4.),
            minimum_degree: 3,
            seed: 17,
        },
        ConnectionProgram::GridSpanningTree {
            adjacency: adjacency(6, 4.),
            algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
            seed: 17,
        },
    ] {
        for cap in [1, 2, 3] {
            let policy = ConnectionSelectionIntent {
                maximum_final_degree: Some(cap),
                cycles: ConnectionCyclePolicy::Forbid,
                minimum_component_points: 1,
            };
            let output = run(&graph, &program, policy);
            invariants(&output, cap, true);
            assert_eq!(output, run(&graph, &program, policy));
            assert!(output.isolated_points.iter().any(|p| p.id.ordinal == 6));
        }
    }
}
/// Proves rescue can use a family vertex excluded by mutual candidate degree and combines before filtering.
/// # Panics
/// Panics if a short chain is hidden before its noncandidate rescue can create a valid group.
#[test]
fn rescue_uses_non_candidate_vertices_and_joins_before_component_filter() {
    let sites = sites(&[(0., 0.), (0.1, 0.), (0.3, 0.), (0.4, 0.)]);
    let graph = graph(&sites, 1, 1.);
    assert_eq!(graph.edges().len(), 2);
    let program = ConnectionProgram::NearestLinks {
        adjacency: adjacency(1, 1.),
    };
    let policy = ConnectionSelectionIntent {
        minimum_component_points: 4,
        ..Default::default()
    };
    let output = run(&graph, &program, policy);
    assert_eq!(output.selected_edges.len(), 3);
    assert!(
        output
            .selected_edges
            .iter()
            .any(|e| !graph.edges().contains(e))
    );
    assert!(output.isolated_points.is_empty());
    let valid = run(
        &graph,
        &program,
        ConnectionSelectionIntent {
            minimum_component_points: 2,
            ..Default::default()
        },
    );
    assert_eq!(valid.selected_edges.len(), 2);
}
/// Checks sparse filtered IDs, unresolved singleton preservation, and minimum-size suppression.
/// # Panics
/// Panics if an ordinal becomes a dense index or hidden components leave structural usage.
#[test]
fn filtered_sparse_ids_and_unresolved_singletons_preserve_site_facts() {
    let original = sites(&[(0., 0.), (2., 2.), (4., 4.), (10., 0.), (20., 0.)]);
    let selected = [
        original.sites()[0].id,
        original.sites()[3].id,
        original.sites()[4].id,
    ]
    .into_iter()
    .collect();
    let filtered = original.filtered(&selected);
    let candidate = graph(&filtered, 2, 0.5);
    let program = ConnectionProgram::NearestLinks {
        adjacency: adjacency(2, 0.5),
    };
    let output = run(&candidate, &program, Default::default());
    assert_eq!(
        output
            .isolated_points
            .iter()
            .map(|n| n.id.ordinal)
            .collect::<Vec<_>>(),
        vec![0, 3, 4]
    );
    let close = sites(&[
        (100., 100.),
        (200., 200.),
        (300., 300.),
        (0., 0.),
        (0.1, 0.),
    ]);
    let ids = [close.sites()[3].id, close.sites()[4].id]
        .into_iter()
        .collect();
    let filtered_close = close.filtered(&ids);
    let close_graph = graph(&filtered_close, 1, 0.5);
    assert_eq!(close_graph.edges().len(), 1);
    assert_eq!(close_graph.edges()[0].first.ordinal, 3);
    assert_eq!(close_graph.edges()[0].second.ordinal, 4);
    let control = graph(&sites(&[(0., 0.), (0.1, 0.)]), 1, 0.5);
    assert_eq!(control.edges().len(), 1);
    let suppressed = run(
        &candidate,
        &program,
        ConnectionSelectionIntent {
            minimum_component_points: 2,
            ..Default::default()
        },
    );
    assert!(suppressed.isolated_points.is_empty());
    assert!(suppressed.paths.is_empty());
}
/// Proves every cancellation or finite work failure leaves no partial result and the next request recovers.
/// # Panics
/// Panics if work/cancellation is swallowed or later replay changes.
#[test]
fn selection_cancellation_work_and_recovery_are_atomic() {
    let s = sites(
        &(0..40)
            .map(|i| (i as f64, (i % 3) as f64))
            .collect::<Vec<_>>(),
    );
    let g = graph(&s, 3, 3.);
    let p = ConnectionProgram::NearestLinks {
        adjacency: adjacency(3, 3.),
    };
    for threshold in [0, 20, 150, 500] {
        let polls = Cell::new(0);
        assert!(
            build_connection_paths_with_selection_cancellable(
                PatternOutputLayerId(11),
                &g,
                &p,
                Default::default(),
                ConnectionPathLimits::default(),
                &|| {
                    polls.set(polls.get() + 1);
                    polls.get() > threshold
                }
            )
            .is_err()
        );
    }
    let limits = ConnectionPathLimits::new(1000, 1000, 1000, 10).unwrap();
    assert!(
        build_connection_paths_with_selection_cancellable(
            PatternOutputLayerId(11),
            &g,
            &p,
            Default::default(),
            limits,
            &|| false
        )
        .is_err()
    );
    assert!(!run(&g, &p, Default::default()).selected_edges.is_empty());
}

/// Proves a later admissible rescue remains available beyond a full rejected radius page.
/// # Panics
/// Panics if 64 scratch neighbors become an artistic cutoff or valid groups are merged.
#[test]
fn rescue_reaches_later_target_after_more_than_64_degree_rejections() {
    let mut points = vec![(-1., 0.), (0., -0.01), (0., 0.01)];
    for i in 0..40 {
        let x = 0.1 + i as f64 * 0.1;
        points.push((x, 0.));
        points.push((x, 0.0001));
    }
    points.push((-100., 0.));
    let graph = graph(&sites(&points), 1, 200.);
    let program = ConnectionProgram::NearestLinks {
        adjacency: adjacency(1, 200.),
    };
    let output = run(
        &graph,
        &program,
        ConnectionSelectionIntent {
            maximum_final_degree: Some(1),
            ..Default::default()
        },
    );
    let last = points.len() - 1;
    assert!(
        output
            .selected_edges
            .iter()
            .any(|edge| edge.first.ordinal == 0 && edge.second.ordinal == last)
    );
    assert!(!graph.edges().iter().any(|edge| edge.first.ordinal == 0));
    invariants(&output, 1, true);
}
/// Counts graph-component vertices through branches, independently of edge/trail lengths.
/// # Panics
/// Panics if branch decomposition changes the four-point component threshold.
#[test]
fn component_threshold_counts_distinct_points_across_branches() {
    let graph = graph(&sites(&[(0., 0.), (1., 0.), (0., 1.), (-1., 0.)]), 3, 1.1);
    let program = ConnectionProgram::NearestLinks {
        adjacency: adjacency(3, 1.1),
    };
    let four = run(
        &graph,
        &program,
        ConnectionSelectionIntent {
            minimum_component_points: 4,
            ..Default::default()
        },
    );
    assert_eq!(four.selected_edges.len(), 3);
    let five = run(
        &graph,
        &program,
        ConnectionSelectionIntent {
            minimum_component_points: 5,
            ..Default::default()
        },
    );
    assert!(five.selected_edges.is_empty());
}
/// Keeps distant isolated populations within explicit normal work and rejects no overflowing far pair.
/// # Panics
/// Panics if singleton repair scans every component's whole family or outside-radius distance overflow fails.
#[test]
fn distant_singletons_scale_with_component_members_and_safe_radius_bounds() {
    let points = (0..6000).map(|i| (i as f64 * 10., 0.)).collect::<Vec<_>>();
    let g = graph(&sites(&points), 1, 1.);
    let p = ConnectionProgram::NearestLinks {
        adjacency: adjacency(1, 1.),
    };
    let output = run(&g, &p, Default::default());
    assert_eq!(output.isolated_points.len(), 6000);
    let g = graph(&sites(&[(-8e307, -8e307), (8e307, 8e307)]), 1, 1e300);
    assert!(g.edges().is_empty());
    let p = ConnectionProgram::NearestLinks {
        adjacency: adjacency(1, 1e300),
    };
    assert_eq!(run(&g, &p, Default::default()).isolated_points.len(), 2);
}
