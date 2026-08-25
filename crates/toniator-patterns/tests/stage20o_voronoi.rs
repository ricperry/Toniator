use toniator_domain::{
    CoveragePolicy, CurveRepetition, CurveWinding, GeneralizedSiteProduct, GuideDimensionId,
    MarkOrientation, ParametricCurve, PatternDefinition, PatternDefinitionId, PatternFamily,
    PatternMechanism, PatternMechanismId, PatternModulation, PatternOutputLayer,
    PatternOutputLayerId, RandomSiteCharacter, RegionSourceIntent, SiteDensityModulation,
    SiteExclusionPolicy, SpiralCurve, SpiralShape, StraightGuideDimension, StraightGuideRepetition,
};
use toniator_patterns::{OutputCapabilityPayload, resolve_pattern_pipeline};

/// Builds one validated parametric site-family definition with a fixed ordinary-region output.
fn parametric_region_definition() -> PatternDefinition {
    PatternDefinition {
        id: PatternDefinitionId(901),
        name: "parametric regions".into(),
        family: PatternFamily::ParametricCurve {
            curve_mechanism_id: PatternMechanismId(902),
            site_mechanism_id: Some(PatternMechanismId(903)),
        },
        mechanisms: vec![
            PatternMechanism::ParametricCurveSource {
                id: PatternMechanismId(902),
                curve: ParametricCurve::Spiral(SpiralCurve {
                    shape: SpiralShape::Round,
                    turns: 2.0,
                    radial_spacing: 30.0,
                    phase_degrees: 0.0,
                    winding: CurveWinding::CounterClockwise,
                }),
                repetition: CurveRepetition::Single,
            },
            PatternMechanism::AlongParametricCurveSites {
                id: PatternMechanismId(903),
                curve_mechanism_id: PatternMechanismId(902),
                interval: 8.0,
                phase: 0.0,
            },
        ],
        output_layers: vec![PatternOutputLayer::Regions {
            id: PatternOutputLayerId(904),
            source: RegionSourceIntent::VoronoiSites {
                site_mechanism_id: PatternMechanismId(903),
            },
        }],
        modulation: PatternModulation,
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    }
}

/// Proves `AlongParametricCurveSites` publishes an eligible site set rather than raw-path intent.
#[test]
fn parametric_sites_resolve_as_ordinary_region_capability() {
    let plan = resolve_pattern_pipeline(&parametric_region_definition()).expect("site pipeline");
    assert!(matches!(
        plan.ordered_outputs[0].payload,
        OutputCapabilityPayload::Regions {
            site_mechanism_id: PatternMechanismId(903)
        }
    ));
}

/// Proves a raw parametric-path family cannot claim a Regions output without a site product.
#[test]
fn raw_parametric_paths_reject_region_capability() {
    let mut definition = parametric_region_definition();
    definition.family = PatternFamily::ParametricCurve {
        curve_mechanism_id: PatternMechanismId(902),
        site_mechanism_id: None,
    };
    definition.mechanisms.truncate(1);
    let error = resolve_pattern_pipeline(&definition).expect_err("raw paths are ineligible");
    assert_eq!(error.path(), "pattern.output_layers.capability");
}

/// Proves a random site-set output is accepted by the same site-set authority.
#[test]
fn random_site_set_resolves_as_ordinary_region_capability() {
    let mut definition = PatternDefinition::random_sites(
        PatternDefinitionId(910),
        "random regions",
        PatternMechanismId(911),
        PatternMechanismId(912),
        PatternMechanismId(913),
        PatternMechanismId(914),
        PatternOutputLayerId(915),
        RandomSiteCharacter::RawUniform,
        17,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        2_000,
        2_000,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = vec![PatternOutputLayer::Regions {
        id: PatternOutputLayerId(915),
        source: RegionSourceIntent::VoronoiSites {
            site_mechanism_id: PatternMechanismId(914),
        },
    }];
    let plan = resolve_pattern_pipeline(&definition).expect("random site pipeline");
    assert!(matches!(
        plan.ordered_outputs[0].payload,
        OutputCapabilityPayload::Regions { .. }
    ));
}

/// Proves ordinary regions accept sites along guides through the site-set capability, not provenance.
#[test]
fn along_guide_sites_resolve_as_ordinary_region_capability() {
    let mut definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(920),
        "along-guide regions",
        PatternMechanismId(921),
        PatternMechanismId(922),
        PatternOutputLayerId(923),
        vec![StraightGuideDimension {
            id: GuideDimensionId(1),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(1)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = vec![PatternOutputLayer::Regions {
        id: PatternOutputLayerId(923),
        source: RegionSourceIntent::VoronoiSites {
            site_mechanism_id: PatternMechanismId(922),
        },
    }];
    let plan = resolve_pattern_pipeline(&definition).expect("along-guide site pipeline");
    assert!(matches!(
        plan.ordered_outputs[0].payload,
        OutputCapabilityPayload::Regions {
            site_mechanism_id: PatternMechanismId(922)
        }
    ));
}
