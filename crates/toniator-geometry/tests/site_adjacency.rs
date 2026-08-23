use std::cell::Cell;

use toniator_domain::{GuideDimensionId, PatternMechanismId};
use toniator_geometry::{
    FamilySite, FamilySiteId, FamilySiteProvenance, FamilySiteSet, GuideInstanceId,
    NominalCellBasis, Point2, SiteAdjacencyLimits, SiteAdjacencyPolicy, SiteScope,
    StructuralPathInstanceId, StructuralPathLocationProvenance, Vector2,
    build_site_adjacency_cancellable,
};

/// Builds one evaluator-ordered finite site product for topology-only witnesses.
fn sites(points: &[(f64, f64)]) -> FamilySiteSet {
    FamilySiteSet::new(
        "family-test".into(),
        PatternMechanismId(9),
        points
            .iter()
            .enumerate()
            .map(|(ordinal, &(x, y))| FamilySite {
                id: FamilySiteId {
                    mechanism_id: PatternMechanismId(9),
                    ordinal,
                },
                position: Point2::new(x, y),
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(1.0, 0.0),
                    Vector2::new(0.0, 1.0),
                )
                .expect("finite unit basis is valid"),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: ordinal,
                    accepted_ordinal: ordinal,
                    exclusion_neighbor_ordinal: None,
                },
            })
            .collect(),
    )
    .expect("ordered unique finite test sites are valid")
}

/// Builds the ordinary one-neighbour, finite-distance policy.
fn policy() -> SiteAdjacencyPolicy {
    SiteAdjacencyPolicy::MutualNearest {
        maximum_degree: 1,
        maximum_distance: 2.0,
    }
}

/// Proves canonical endpoint ordering, mutuality, duplicate suppression, and ordered components.
#[test]
fn mutual_edges_are_canonical_and_components_include_isolates() {
    let graph = build_site_adjacency_cancellable(
        &sites(&[(0.0, 0.0), (1.0, 0.0), (10.0, 0.0)]),
        policy(),
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("finite sites build topology");
    assert_eq!(graph.nodes().len(), 3);
    assert_eq!(graph.edges().len(), 1);
    assert_eq!(graph.edges()[0].first.ordinal, 0);
    assert_eq!(graph.edges()[0].second.ordinal, 1);
    assert_eq!(graph.components().len(), 2);
    assert_eq!(
        graph.components()[0]
            .members
            .iter()
            .map(|id| id.ordinal)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(graph.components()[1].members[0].ordinal, 2);
}

/// Proves equal distances select the smaller stable family-site ID.
#[test]
fn equidistant_candidates_break_ties_by_family_site_id() {
    let graph = build_site_adjacency_cancellable(
        &sites(&[(0.0, 0.0), (-1.0, 0.0), (1.0, 0.0)]),
        policy(),
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("finite sites build topology");
    assert_eq!(graph.edges().len(), 1);
    assert_eq!(graph.edges()[0].first.ordinal, 0);
    assert_eq!(graph.edges()[0].second.ordinal, 1);
}

/// Proves coincident sites remain nodes but never receive zero-length edges.
#[test]
fn coincident_sites_remain_distinct_without_zero_length_edge() {
    let graph = build_site_adjacency_cancellable(
        &sites(&[(0.0, 0.0), (0.0, 0.0)]),
        policy(),
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("coincident finite sites are valid nodes");
    assert_eq!(graph.nodes().len(), 2);
    assert!(graph.edges().is_empty());
    assert_eq!(graph.components().len(), 2);
}

/// Proves fingerprints include policy and ordered graph content but exclude operational limits.
#[test]
fn fingerprint_is_stable_and_resource_limit_independent() {
    let product = sites(&[(0.0, 0.0), (1.0, 0.0)]);
    let first = build_site_adjacency_cancellable(
        &product,
        policy(),
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("default bounded topology succeeds");
    let second = build_site_adjacency_cancellable(
        &product,
        policy(),
        SiteAdjacencyLimits::new(4, 4, 4, 20).expect("nonzero limits are valid"),
        &|| false,
    )
    .expect("smaller sufficient limit succeeds");
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_ne!(
        first.fingerprint(),
        build_site_adjacency_cancellable(
            &product,
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: 1,
                maximum_distance: 1.5
            },
            SiteAdjacencyLimits::default(),
            &|| false,
        )
        .expect("changed valid policy succeeds")
        .fingerprint(),
    );
}

/// Proves cancellation and resource failure return stable diagnostics before a graph can escape.
#[test]
fn cancellation_and_limits_are_atomic() {
    let checks = Cell::new(0_u32);
    let cancelled = build_site_adjacency_cancellable(
        &sites(&[(0.0, 0.0), (1.0, 0.0)]),
        policy(),
        SiteAdjacencyLimits::default(),
        &|| {
            checks.set(checks.get() + 1);
            checks.get() >= 3
        },
    )
    .expect_err("cancellation prevents graph publication");
    assert_eq!(cancelled.path(), "evaluation.cancelled");
    assert_eq!(
        build_site_adjacency_cancellable(
            &sites(&[(0.0, 0.0), (1.0, 0.0)]),
            policy(),
            SiteAdjacencyLimits::new(1, 4, 4, 4).expect("nonzero limits are valid"),
            &|| false,
        )
        .expect_err("node limit prevents graph publication")
        .path(),
        "adjacency.limits.nodes",
    );
}

/// Proves policy validation rejects disabled degree and nonfinite or nonpositive distance inputs.
#[test]
fn policy_bounds_are_explicit() {
    for policy in [
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 0,
            maximum_distance: 1.0,
        },
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 33,
            maximum_distance: 1.0,
        },
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 1,
            maximum_distance: 0.0,
        },
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 1,
            maximum_distance: f64::NAN,
        },
    ] {
        assert!(policy.validate().is_err());
    }
}

/// Proves one-sided nearest selections never become non-mutual edges and distance bounds filter pairs.
#[test]
fn mutuality_and_distance_bounds_filter_edges() {
    let graph = build_site_adjacency_cancellable(
        &sites(&[(0.0, 0.0), (1.0, 0.0), (1.1, 0.0)]),
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 1,
            maximum_distance: 2.0,
        },
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("finite sites build topology");
    assert_eq!(graph.edges().len(), 1);
    assert_eq!(graph.edges()[0].first.ordinal, 1);
    assert_eq!(graph.edges()[0].second.ordinal, 2);
    assert!(
        build_site_adjacency_cancellable(
            &sites(&[(0.0, 0.0), (2.0, 0.0)]),
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: 1,
                maximum_distance: 1.0,
            },
            SiteAdjacencyLimits::default(),
            &|| false,
        )
        .expect("finite separated nodes are valid")
        .edges()
        .is_empty()
    );
}

/// Proves empty and singleton products retain canonical component behaviour without edge allocation.
#[test]
fn empty_and_singleton_products_are_canonical() {
    assert!(
        build_site_adjacency_cancellable(
            &sites(&[]),
            policy(),
            SiteAdjacencyLimits::default(),
            &|| false,
        )
        .expect("empty product is valid")
        .components()
        .is_empty()
    );
    let singleton = build_site_adjacency_cancellable(
        &sites(&[(0.0, 0.0)]),
        policy(),
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("singleton product is valid");
    assert_eq!(singleton.components()[0].members[0].ordinal, 0);
}

/// Proves every checked work budget and geometry-coordinate diagnostic remains stable.
#[test]
fn bounded_work_and_nonfinite_geometry_fail_stably() {
    let product = sites(&[(0.0, 0.0), (1.0, 0.0)]);
    assert_eq!(
        build_site_adjacency_cancellable(
            &product,
            policy(),
            SiteAdjacencyLimits::new(4, 1, 4, 4).expect("nonzero limits"),
            &|| false,
        )
        .expect_err("two retained selections exceed membership limit")
        .path(),
        "adjacency.limits.neighbor_memberships",
    );
    assert_eq!(
        build_site_adjacency_cancellable(
            &product,
            policy(),
            SiteAdjacencyLimits::new(4, 4, 1, 1).expect("nonzero limits"),
            &|| false,
        )
        .expect_err("two directional distance checks exceed limit")
        .path(),
        "adjacency.limits.distance_checks",
    );
    assert_eq!(
        build_site_adjacency_cancellable(
            &sites(&[(0.0, 0.0), (1.0, 0.0), (0.5, 0.5)]),
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: 2,
                maximum_distance: 2.0,
            },
            SiteAdjacencyLimits::new(4, 8, 1, 20).expect("nonzero limits"),
            &|| false,
        )
        .expect_err("three mutual pairs exceed edge limit")
        .path(),
        "adjacency.limits.edges",
    );
    assert_eq!(
        build_site_adjacency_cancellable(
            &sites(&[(1.0e308, 0.0)]),
            policy(),
            SiteAdjacencyLimits::default(),
            &|| false,
        )
        .expect_err("unrepresentable spatial coordinate is rejected")
        .path(),
        "adjacency.cell_coordinate",
    );
    assert_eq!(
        build_site_adjacency_cancellable(
            &sites(&[(1.0e308, 0.0), (-1.0e308, 0.0)]),
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: 1,
                maximum_distance: f64::MAX,
            },
            SiteAdjacencyLimits::default(),
            &|| false,
        )
        .expect_err("overflowing pair distance is rejected")
        .path(),
        "adjacency.distance",
    );
}

/// Builds one product spanning every current truthful provenance variant in deliberate evaluator order.
fn sites_with_all_provenance() -> Vec<FamilySite> {
    let guide = GuideInstanceId::new(GuideDimensionId(17), -2);
    let guide_location = StructuralPathLocationProvenance {
        path: StructuralPathInstanceId::guide_dimension(GuideDimensionId(18), 3, 1),
        segment_index: 2,
        parameter_bits: 0.25_f64.to_bits(),
    };
    let parametric_location = StructuralPathLocationProvenance {
        path: StructuralPathInstanceId::parametric_curve(PatternMechanismId(19), 4, 0),
        segment_index: 5,
        parameter_bits: 0.75_f64.to_bits(),
    };
    vec![
        FamilySite {
            id: FamilySiteId {
                mechanism_id: PatternMechanismId(9),
                ordinal: 0,
            },
            position: Point2::new(8.0, 1.0),
            nominal_cell_basis: NominalCellBasis::new(
                Vector2::new(2.0, 0.0),
                Vector2::new(0.0, 3.0),
            )
            .expect("finite basis"),
            scope: SiteScope::Guard,
            provenance: FamilySiteProvenance::GuideIntersection {
                contributors: vec![guide, GuideInstanceId::new(GuideDimensionId(20), 1)],
            },
        },
        FamilySite {
            id: FamilySiteId {
                mechanism_id: PatternMechanismId(9),
                ordinal: 1,
            },
            position: Point2::new(1.0, 2.0),
            nominal_cell_basis: NominalCellBasis::new(
                Vector2::new(3.0, 0.0),
                Vector2::new(0.0, 2.0),
            )
            .expect("finite basis"),
            scope: SiteScope::Canvas,
            provenance: FamilySiteProvenance::AlongGuide {
                guide_id: guide,
                guide_order: 7,
                sequence: -4,
                absolute_arc_position_bits: 8.5_f64.to_bits(),
                local_arc_position_bits: 1.5_f64.to_bits(),
            },
        },
        FamilySite {
            id: FamilySiteId {
                mechanism_id: PatternMechanismId(9),
                ordinal: 2,
            },
            position: Point2::new(6.0, 3.0),
            nominal_cell_basis: NominalCellBasis::new(
                Vector2::new(4.0, 0.0),
                Vector2::new(0.0, 1.0),
            )
            .expect("finite basis"),
            scope: SiteScope::Guard,
            provenance: FamilySiteProvenance::Random {
                candidate_ordinal: 9,
                accepted_ordinal: 4,
                exclusion_neighbor_ordinal: Some(3),
            },
        },
        FamilySite {
            id: FamilySiteId {
                mechanism_id: PatternMechanismId(9),
                ordinal: 3,
            },
            position: Point2::new(2.0, 4.0),
            nominal_cell_basis: NominalCellBasis::new(
                Vector2::new(1.0, 1.0),
                Vector2::new(-1.0, 1.0),
            )
            .expect("finite basis"),
            scope: SiteScope::Canvas,
            provenance: FamilySiteProvenance::CurveGuideIntersection {
                contributors: vec![
                    guide_location,
                    StructuralPathLocationProvenance {
                        path: StructuralPathInstanceId::guide_dimension(GuideDimensionId(21), 1, 0),
                        segment_index: 0,
                        parameter_bits: 0.5_f64.to_bits(),
                    },
                ],
            },
        },
        FamilySite {
            id: FamilySiteId {
                mechanism_id: PatternMechanismId(9),
                ordinal: 4,
            },
            position: Point2::new(5.0, 5.0),
            nominal_cell_basis: NominalCellBasis::new(
                Vector2::new(1.5, 0.0),
                Vector2::new(0.0, 2.5),
            )
            .expect("finite basis"),
            scope: SiteScope::Guard,
            provenance: FamilySiteProvenance::CurveAlongGuide {
                location: guide_location,
                guide_order: 6,
                sequence: 2,
                absolute_arc_position_bits: 3.5_f64.to_bits(),
                local_arc_position_bits: 0.5_f64.to_bits(),
            },
        },
        FamilySite {
            id: FamilySiteId {
                mechanism_id: PatternMechanismId(9),
                ordinal: 5,
            },
            position: Point2::new(3.0, 6.0),
            nominal_cell_basis: NominalCellBasis::new(
                Vector2::new(2.5, 0.0),
                Vector2::new(0.0, 1.5),
            )
            .expect("finite basis"),
            scope: SiteScope::Canvas,
            provenance: FamilySiteProvenance::AlongParametricCurve {
                location: parametric_location,
                path_order: 8,
                sequence: 3,
                absolute_arc_position_bits: 9.5_f64.to_bits(),
                local_arc_position_bits: 2.5_f64.to_bits(),
            },
        },
    ]
}

/// Proves graph nodes retain full evaluator order and every site payload without provenance reinterpretation.
#[test]
fn nodes_retain_order_and_all_truthful_site_payload_variants() {
    let expected = sites_with_all_provenance();
    let product = FamilySiteSet::new(
        "all-provenance".into(),
        PatternMechanismId(9),
        expected.clone(),
    )
    .expect("all variants are valid");
    let graph = build_site_adjacency_cancellable(
        &product,
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 2,
            maximum_distance: 1.1,
        },
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("payload-preserving graph succeeds");
    assert_eq!(graph.nodes().len(), expected.len());
    for (node, site) in graph.nodes().iter().zip(&expected) {
        assert_eq!(node.id, site.id);
        assert_eq!(node.position, site.position);
        assert_eq!(node.scope, site.scope);
        assert_eq!(node.nominal_cell_basis, site.nominal_cell_basis);
        assert_eq!(node.provenance, site.provenance);
    }
}

/// Proves no dense-fixture graph node exceeds the caller-supplied maximum degree.
#[test]
fn dense_graph_never_exceeds_policy_maximum_degree_per_node() {
    let graph = build_site_adjacency_cancellable(
        &sites(&[(0.0, 0.0), (0.2, 0.0), (0.0, 0.2), (0.2, 0.2), (0.1, 0.1)]),
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 2,
            maximum_distance: 1.0,
        },
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("dense finite graph succeeds");
    for node in graph.nodes() {
        let degree = graph
            .edges()
            .iter()
            .filter(|edge| edge.first == node.id || edge.second == node.id)
            .count();
        assert!(degree <= 2, "node {} has degree {degree}", node.id.ordinal);
    }
}
