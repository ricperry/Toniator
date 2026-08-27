use toniator_domain::{
    CanvasSpec, CoveragePolicy, CurveWinding, DensityMetric2D, GeneralizedSiteProduct,
    GuideDimensionId, GuideRepetition, MarkOrientation, MarkPrototype, ParametricCurve,
    PathStrokeStyle, PatternDefinition, PatternDefinitionId, PatternFamily, PatternMechanism,
    PatternMechanismId, PatternModulation, PatternOutputLayer, PatternOutputLayerId,
    PatternOutputRealization, RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy,
    SpiralCurve, SpiralShape, StraightGuideDimension, StraightGuideRepetition,
};
use toniator_patterns::{
    GridInspectRequest, SiteAdjacencyLimits, SiteAdjacencyPolicy, StructuralProductCapability,
    build_site_adjacency_cancellable, evaluate_typed_family_product_cancellable,
    evaluate_typed_site_adjacency_with_source_cancellable, resolve_pattern_pipeline,
};

/// Builds a small finite family request with active guard coverage.
fn request() -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec {
            width: 80.0,
            height: 60.0,
        },
        density: DensityMetric2D {
            across_x: 8.0,
            across_y: 6.0,
            aspect_locked: true,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 1,
        support_radius: 0.0,
        max_family_candidates: 20_000,
    }
}

/// Builds a parametric site or raw-path product without referring to mechanism names at evaluation.
fn spiral_definition(sites: bool) -> PatternDefinition {
    let curve_id = PatternMechanismId(91);
    let site_id = PatternMechanismId(92);
    PatternDefinition {
        id: PatternDefinitionId(90),
        name: "adjacency fixture".into(),
        family: PatternFamily::ParametricCurve {
            curve_mechanism_id: curve_id,
            site_mechanism_id: sites.then_some(site_id),
        },
        mechanisms: if sites {
            vec![
                PatternMechanism::ParametricCurveSource {
                    id: curve_id,
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
                    id: site_id,
                    curve_mechanism_id: curve_id,
                    interval: 10.0,
                    phase: 0.0,
                },
            ]
        } else {
            vec![PatternMechanism::ParametricCurveSource {
                id: curve_id,
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
                PatternOutputLayerId(93),
                PatternOutputRealization::MarkPrototype {
                    site_mechanism_id: site_id,
                    prototype: MarkPrototype::Circle,
                    orientation: MarkOrientation::Fixed,
                },
            )]
        } else {
            vec![PatternOutputLayer::all(
                PatternOutputLayerId(93),
                PatternOutputRealization::ParametricPaths {
                    curve_mechanism_id: curve_id,
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

/// Builds a named-presentation-neutral grid definition whose sites are eligible by capability.
fn grid_definition() -> PatternDefinition {
    PatternDefinition::supported_straight_grid(
        PatternDefinitionId(12),
        "presentation text does not select adjacency",
        PatternMechanismId(13),
        PatternMechanismId(14),
        PatternOutputLayerId(15),
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    )
}

/// Builds one uniform or even dispersion product without introducing topology mechanism dispatch.
fn random_definition(character: RandomSiteCharacter) -> PatternDefinition {
    PatternDefinition::random_sites(
        PatternDefinitionId(20),
        "dispersion presentation name",
        PatternMechanismId(21),
        PatternMechanismId(22),
        PatternMechanismId(23),
        PatternMechanismId(24),
        PatternOutputLayerId(25),
        character,
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

/// Builds an ordinary typed along-guide site product without referring to source provenance at topology time.
fn along_guide_definition() -> PatternDefinition {
    PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(30),
        "along guide fixture",
        PatternMechanismId(31),
        PatternMechanismId(32),
        PatternOutputLayerId(33),
        vec![StraightGuideDimension {
            id: GuideDimensionId(34),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(34)],
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

/// Evaluates one capability-derived graph with topology support independent of canvas graph input.
fn adjacency(definition: &PatternDefinition) -> toniator_patterns::SiteAdjacencyEvaluation {
    let plan = resolve_pattern_pipeline(definition).expect("valid resolved family capability");
    evaluate_typed_site_adjacency_with_source_cancellable(
        &plan.family,
        &request(),
        None,
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 2,
            maximum_distance: 14.0,
        },
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("eligible family product derives topology")
}

/// Returns canonical edges whose two endpoints are canvas sites, independent of guard-only nodes.
fn canvas_edges(
    graph: &toniator_patterns::SiteAdjacencyGraph,
) -> Vec<(
    toniator_patterns::FamilySiteId,
    toniator_patterns::FamilySiteId,
)> {
    graph
        .edges()
        .iter()
        .filter(|edge| {
            graph.nodes()[edge.first.ordinal].scope == toniator_patterns::SiteScope::Canvas
                && graph.nodes()[edge.second.ordinal].scope == toniator_patterns::SiteScope::Canvas
        })
        .map(|edge| (edge.first, edge.second))
        .collect()
}

/// Proves grid intersections and uniform/even dispersion products are accepted by capability alone.
#[test]
fn grid_uniform_and_even_site_products_are_eligible() {
    let grid = adjacency(&grid_definition());
    assert!(grid.graph.nodes().len() > 1);
    for character in [
        RandomSiteCharacter::RawUniform,
        RandomSiteCharacter::Even {
            minimum_center_distance: 8.0,
        },
    ] {
        let graph = adjacency(&random_definition(character));
        assert_eq!(graph.graph.nodes().len(), graph.family.site_set().len());
    }
    let along = adjacency(&along_guide_definition());
    assert!(along.graph.nodes().len() > 1);
}

/// Proves topology support augments the complete family envelope by guard steps times distance.
#[test]
fn guard_inclusive_construction_expands_family_support() {
    let graph = adjacency(&grid_definition());
    assert!(graph.family.planned_support_radius() >= 14.0);
    assert!(
        graph
            .family
            .site_set()
            .iter()
            .any(|site| matches!(site.scope, toniator_patterns::SiteScope::Guard))
    );
}

/// Proves canvas-grid topology matches an independently much broader family envelope.
#[test]
fn grid_canvas_adjacency_matches_independently_broader_envelope() {
    let definition = grid_definition();
    let actual = adjacency(&definition);
    let plan = resolve_pattern_pipeline(&definition).expect("valid grid plan");
    let mut broad = request();
    broad.support_radius = 80.0;
    let family = evaluate_typed_family_product_cancellable(&plan.family, &broad, &|| false)
        .expect("independently broad grid family");
    let reference = build_site_adjacency_cancellable(
        family.site_set(),
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 2,
            maximum_distance: 14.0,
        },
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("broader grid topology");
    assert_eq!(canvas_edges(&actual.graph), canvas_edges(&reference));
}

/// Proves canvas parametric equal-arc topology matches an independently broader envelope.
#[test]
fn parametric_canvas_adjacency_matches_independently_broader_envelope() {
    let definition = spiral_definition(true);
    let actual = adjacency(&definition);
    let plan = resolve_pattern_pipeline(&definition).expect("valid parametric plan");
    let mut broad = request();
    broad.support_radius = 80.0;
    let family = evaluate_typed_family_product_cancellable(&plan.family, &broad, &|| false)
        .expect("independently broad parametric family");
    let reference = build_site_adjacency_cancellable(
        family.site_set(),
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 2,
            maximum_distance: 14.0,
        },
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("broader parametric topology");
    assert_eq!(canvas_edges(&actual.graph), canvas_edges(&reference));
}

/// Proves topology follows resolved site capability and geometry rather than family presentation IDs.
#[test]
fn equivalent_resolved_families_are_mechanism_name_neutral() {
    let first = adjacency(&grid_definition());
    let second_definition = PatternDefinition::supported_straight_grid(
        PatternDefinitionId(42),
        "entirely different presentation name",
        PatternMechanismId(43),
        PatternMechanismId(44),
        PatternOutputLayerId(45),
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    let second = adjacency(&second_definition);
    assert_eq!(
        first
            .graph
            .edges()
            .iter()
            .map(|edge| (edge.first.ordinal, edge.second.ordinal))
            .collect::<Vec<_>>(),
        second
            .graph
            .edges()
            .iter()
            .map(|edge| (edge.first.ordinal, edge.second.ordinal))
            .collect::<Vec<_>>(),
    );
}

/// Proves eligible equal-arc parametric sites evaluate family coverage before deriving topology.
#[test]
fn equal_arc_parametric_sites_publish_derived_adjacency() {
    let plan = resolve_pattern_pipeline(&spiral_definition(true)).expect("valid site plan");
    assert_eq!(
        plan.family.product,
        StructuralProductCapability::AlongGuideSites
    );
    let evaluation = evaluate_typed_site_adjacency_with_source_cancellable(
        &plan.family,
        &request(),
        None,
        SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 2,
            maximum_distance: 12.0,
        },
        SiteAdjacencyLimits::default(),
        &|| false,
    )
    .expect("equal-arc sites form topology");
    assert!(evaluation.family.planned_support_radius() >= 12.0);
    assert_eq!(
        evaluation.graph.nodes().len(),
        evaluation.family.site_set().len()
    );
}

/// Proves homogeneous raw parametric paths fail before coverage or graph allocation.
#[test]
fn raw_parametric_paths_are_rejected_for_adjacency() {
    let plan = resolve_pattern_pipeline(&spiral_definition(false)).expect("valid raw-path plan");
    assert_eq!(
        plan.family.product,
        StructuralProductCapability::ParametricPaths
    );
    assert_eq!(
        evaluate_typed_site_adjacency_with_source_cancellable(
            &plan.family,
            &request(),
            None,
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: 1,
                maximum_distance: 10.0
            },
            SiteAdjacencyLimits::default(),
            &|| false,
        )
        .expect_err("raw paths have no site topology")
        .path(),
        "adjacency.family.product",
    );
}

/// Proves active adjacency requires a guard step instead of treating canvas bounds as topology input.
#[test]
fn adjacency_rejects_guardless_family_requests() {
    let plan = resolve_pattern_pipeline(&spiral_definition(true)).expect("valid site plan");
    let mut guardless = request();
    guardless.guard_steps = 0;
    assert_eq!(
        evaluate_typed_site_adjacency_with_source_cancellable(
            &plan.family,
            &guardless,
            None,
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: 1,
                maximum_distance: 10.0
            },
            SiteAdjacencyLimits::default(),
            &|| false,
        )
        .expect_err("guardless topology is not active")
        .path(),
        "adjacency.coverage.guard_steps",
    );
}
