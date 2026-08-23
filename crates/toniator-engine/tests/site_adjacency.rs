use toniator_domain::{
    CanvasSpec, CoveragePolicy, DensityMetric2D, PatternDefinition, PatternDefinitionId,
    PatternMechanismId, PatternOutputLayerId,
};
use toniator_engine::{
    EvaluationLimits, SiteAdjacencyLimits, SiteAdjacencyPolicy, derive_site_adjacency_cancellable,
};
use toniator_patterns::{
    GridInspectRequest, evaluate_typed_family_product_cancellable, resolve_pattern_pipeline,
};

/// Builds the regular site-producing family used to prove engine adjacency is derived-only.
fn family() -> toniator_patterns::FamilyCapability {
    resolve_pattern_pipeline(&PatternDefinition::supported_straight_grid(
        PatternDefinitionId(1),
        "engine adjacency fixture",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(3),
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    ))
    .expect("valid grid plan")
    .family
}

/// Builds a bounded request that exposes both canvas and guard sites to the family evaluator.
fn request() -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec {
            width: 40.0,
            height: 40.0,
        },
        density: DensityMetric2D {
            across_x: 4.0,
            across_y: 4.0,
            aspect_locked: true,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 1,
        support_radius: 10.0,
        max_family_candidates: 10_000,
    }
}

/// Proves caller policy reconstructs topology while ordinary immutable family identity remains unchanged.
#[test]
fn engine_derives_policy_specific_graph_without_mutating_family_output() {
    let family = evaluate_typed_family_product_cancellable(&family(), &request(), &|| false)
        .expect("finite family output");
    let identity = family.family_fingerprint().to_owned();
    let limits = EvaluationLimits::default()
        .with_site_adjacency_limits(
            SiteAdjacencyLimits::new(10_000, 10_000, 10_000, 100_000)
                .expect("nonzero adjacency limits"),
        )
        .expect("engine accepts nonzero adjacency limits");
    let sparse = derive_site_adjacency_cancellable(
        &family,
        0.0,
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 1,
            maximum_distance: 5.0,
        },
        limits,
        &|| false,
    )
    .expect("derived sparse graph");
    let connected = derive_site_adjacency_cancellable(
        &family,
        0.0,
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 4,
            maximum_distance: 8.0,
        },
        limits,
        &|| false,
    )
    .expect("derived broader graph");
    assert_eq!(family.family_fingerprint(), identity);
    assert_ne!(sparse.fingerprint(), connected.fingerprint());
    assert_eq!(
        derive_site_adjacency_cancellable(
            &family,
            0.0,
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: 1,
                maximum_distance: 11.0,
            },
            limits,
            &|| false,
        )
        .expect_err("ordinary envelope cannot satisfy broader topology support")
        .path(),
        "adjacency.coverage.support",
    );
}

/// Proves cancellation prevents engine callers from receiving a graph or changing family data.
#[test]
fn engine_adjacency_cancellation_is_atomic() {
    let family = evaluate_typed_family_product_cancellable(&family(), &request(), &|| false)
        .expect("finite family output");
    assert_eq!(
        derive_site_adjacency_cancellable(
            &family,
            0.0,
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: 1,
                maximum_distance: 5.0
            },
            EvaluationLimits::default(),
            &|| true,
        )
        .expect_err("cancelled topology cannot publish")
        .path(),
        "evaluation.cancelled",
    );
}
