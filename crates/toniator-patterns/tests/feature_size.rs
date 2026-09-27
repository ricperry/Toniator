use std::fs;

use toniator_domain::{
    CanvasSpec, CoveragePolicy, CurveWinding, DensityMetric2D, GuideRepetition, MarkOrientation,
    MarkPrototype, ParametricCurve, PathStrokeStyle, PatternDefinition, PatternDefinitionId,
    PatternFamily, PatternMechanism, PatternMechanismId, PatternModulation, PatternOutputLayer,
    PatternOutputLayerId, PatternOutputRealization, RandomSiteCharacter, ResolvedDensityMetric2D,
    SiteDensityModulation, SiteExclusionPolicy, SpiralCurve, SpiralShape,
};
use toniator_patterns::{
    GridInspectRequest, StructuralProductCapability,
    evaluate_typed_family_product_with_source_cancellable, resolve_pattern_pipeline,
};
use toniator_sampling::{SourceFormatHint, decode_source};

/// Builds a request whose resolved density represents one artist-facing Feature size value.
///
/// # Panics
/// Panics if the fixed positive test canvas cannot resolve its density.
fn request_for_feature_size(feature_size: f64) -> GridInspectRequest {
    let canvas = CanvasSpec {
        width: 120.0,
        height: 80.0,
    };
    let default_density = DensityMetric2D::default_for_canvas(&canvas)
        .expect("feature-size fixture canvas is valid")
        .resolve(&canvas)
        .expect("feature-size fixture default density resolves");
    GridInspectRequest {
        canvas,
        density: ResolvedDensityMetric2D {
            across_x: default_density.across_x / feature_size,
            across_y: default_density.across_y / feature_size,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 1,
        support_radius: 1.0,
        max_family_candidates: 200_000,
    }
}

/// Builds one random-site family with a nontrivial authored spacing and exclusion policy.
fn random_definition(character: RandomSiteCharacter) -> PatternDefinition {
    PatternDefinition::random_sites(
        PatternDefinitionId(801),
        "Feature-size random sites",
        PatternMechanismId(811),
        PatternMechanismId(812),
        PatternMechanismId(813),
        PatternMechanismId(814),
        PatternOutputLayerId(815),
        character,
        0x1234_5678,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        200_000,
        20_000_000,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    )
}

/// Builds a finite parametric family with one raw path output and optional along-path sites.
fn parametric_definition(with_sites: bool) -> PatternDefinition {
    let curve_id = PatternMechanismId(821);
    let site_id = PatternMechanismId(822);
    let curve = PatternMechanism::ParametricCurveSource {
        id: curve_id,
        curve: ParametricCurve::Spiral(SpiralCurve {
            shape: SpiralShape::Round,
            turns: 4.0,
            radial_spacing: 8.0,
            phase_degrees: 0.0,
            winding: CurveWinding::CounterClockwise,
        }),
        repetition: GuideRepetition::Single,
    };
    let mut mechanisms = vec![curve];
    let (site_mechanism_id, output) = if with_sites {
        mechanisms.push(PatternMechanism::AlongParametricCurveSites {
            id: site_id,
            curve_mechanism_id: curve_id,
            interval: 18.0,
            phase: 0.0,
        });
        (
            Some(site_id),
            PatternOutputLayer::all(
                PatternOutputLayerId(824),
                PatternOutputRealization::MarkPrototype {
                    site_mechanism_id: site_id,
                    prototype: MarkPrototype::Circle,
                    orientation: MarkOrientation::Fixed,
                },
            ),
        )
    } else {
        (
            None,
            PatternOutputLayer::all(
                PatternOutputLayerId(824),
                PatternOutputRealization::ParametricPaths {
                    curve_mechanism_id: curve_id,
                    style: PathStrokeStyle::default(),
                },
            ),
        )
    };
    PatternDefinition {
        id: PatternDefinitionId(820),
        name: "Feature-size parametric spiral".into(),
        family: PatternFamily::ParametricCurve {
            curve_mechanism_id: curve_id,
            site_mechanism_id,
        },
        mechanisms,
        output_layers: vec![output],
        modulation: PatternModulation,
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    }
}

/// Decodes both immutable project artwork inputs at their natural source dimensions.
///
/// # Panics
/// Panics if either immutable fixture cannot be read or decoded.
fn baseline_sources() -> Vec<toniator_sampling::SourceField> {
    let assets = format!("{}/../../assets", env!("CARGO_MANIFEST_DIR"));
    [
        ("raster-sample.png", SourceFormatHint::Png),
        ("vector-sample.svg", SourceFormatHint::Svg),
    ]
    .into_iter()
    .map(|(name, format)| {
        decode_source(
            &fs::read(format!("{assets}/{name}")).expect("immutable source reads"),
            format,
        )
        .expect("immutable source decodes")
    })
    .collect()
}

/// Proves random-site population follows Feature size for both immutable artwork inputs.
///
/// # Panics
/// Panics if family evaluation fails or finer density does not increase population.
#[test]
fn random_sites_follow_feature_size_for_both_baseline_sources() {
    let definition = random_definition(RandomSiteCharacter::RawUniform);
    let plan = resolve_pattern_pipeline(&definition).expect("random family resolves");
    assert_eq!(
        plan.family.product,
        StructuralProductCapability::RandomSites
    );
    for source in baseline_sources() {
        let outputs = [0.5, 1.0, 2.0]
            .into_iter()
            .map(|feature_size| {
                evaluate_typed_family_product_with_source_cancellable(
                    &plan.family,
                    &request_for_feature_size(feature_size),
                    Some(&source),
                    &|| false,
                )
                .expect("random family evaluates")
            })
            .collect::<Vec<_>>();
        let requested = outputs
            .iter()
            .map(|output| {
                output
                    .random_diagnostics()
                    .expect("random diagnostics")
                    .requested_sites
            })
            .collect::<Vec<_>>();
        assert!(requested[0] > requested[1]);
        assert!(requested[1] > requested[2]);
        assert!(outputs[0].site_set().len() > outputs[2].site_set().len());
    }
}

/// Proves reserved recipe effort fields cannot reject otherwise valid normal scatter construction.
/// Explicit request limits remain authoritative independently of those persisted fields.
///
/// # Panics
/// Panics if changing reserved values changes a population or an explicit caller limit is ignored.
#[test]
fn reserved_scatter_effort_fields_do_not_limit_normal_construction() {
    let mut plan = resolve_pattern_pipeline(&random_definition(RandomSiteCharacter::Even {
        minimum_center_distance: 8.0,
    }))
    .unwrap();
    let mut request = request_for_feature_size(1.0);
    request.max_family_candidates = usize::MAX;
    let expected = evaluate_typed_family_product_with_source_cancellable(
        &plan.family,
        &request,
        None,
        &|| false,
    )
    .unwrap();
    let random = plan.family.random.as_mut().unwrap();
    random.maximum_attempts = 1;
    random.maximum_neighbor_checks = 1;
    let actual = evaluate_typed_family_product_with_source_cancellable(
        &plan.family,
        &request,
        None,
        &|| false,
    )
    .unwrap();
    assert_eq!(actual.site_set(), expected.site_set());
    request.max_family_candidates = 1;
    assert_eq!(
        evaluate_typed_family_product_with_source_cancellable(
            &plan.family,
            &request,
            None,
            &|| false
        )
        .unwrap_err()
        .path(),
        "coverage.candidate_limit"
    );
}

/// Proves authored scatter spacing follows Feature size while population density remains
/// controlled by the shared density authority.
///
/// # Panics
/// Panics if evaluation fails or emitted sites violate their scaled separation.
#[test]
fn even_spacing_and_exclusion_follow_feature_size() {
    let definition = PatternDefinition::random_sites(
        PatternDefinitionId(831),
        "Feature-size even sites",
        PatternMechanismId(841),
        PatternMechanismId(842),
        PatternMechanismId(843),
        PatternMechanismId(844),
        PatternOutputLayerId(845),
        RandomSiteCharacter::Even {
            minimum_center_distance: 0.5,
        },
        0x2345_6789,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::MinimumCenterDistance { minimum: 0.5 },
        200_000,
        20_000_000,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    let plan = resolve_pattern_pipeline(&definition).expect("even family resolves");
    for (feature_size, expected_minimum) in [(0.5, 0.25), (1.0, 0.5), (2.0, 1.0)] {
        let output = evaluate_typed_family_product_with_source_cancellable(
            &plan.family,
            &request_for_feature_size(feature_size),
            None,
            &|| false,
        )
        .expect("even family evaluates");
        let sites = output.site_set().sites();
        assert!(
            sites.len() > 100,
            "fixture should retain a useful site population"
        );
        let minimum = sites
            .iter()
            .enumerate()
            .flat_map(|(index, first)| {
                sites[index + 1..].iter().map(move |second| {
                    (first.position.x - second.position.x)
                        .hypot(first.position.y - second.position.y)
                })
            })
            .fold(f64::INFINITY, f64::min);
        assert!(
            minimum + 1.0e-9 >= expected_minimum,
            "minimum {minimum} must retain scaled exclusion {expected_minimum}"
        );
    }
}

/// Proves parametric paths retain an approximately stable outer footprint while their pitch
/// becomes finer or coarser, for both immutable artwork inputs.
/// The small authored spiral stays within the canvas so clipped path fragments cannot masquerade as endpoints.
///
/// # Panics
/// Panics if evaluation fails, the outer radius changes, or path length does not respond to pitch.
#[test]
fn parametric_spiral_scales_pitch_without_losing_outer_footprint() {
    let definition = parametric_definition(false);
    let plan = resolve_pattern_pipeline(&definition).expect("parametric family resolves");
    for source in baseline_sources() {
        let outputs = [0.5, 1.0, 2.0]
            .into_iter()
            .map(|feature_size| {
                evaluate_typed_family_product_with_source_cancellable(
                    &plan.family,
                    &request_for_feature_size(feature_size),
                    Some(&source),
                    &|| false,
                )
                .expect("parametric family evaluates")
            })
            .collect::<Vec<_>>();
        let paths = outputs
            .iter()
            .map(|output| {
                output
                    .structural_path_set()
                    .expect("raw parametric path")
                    .paths()[0]
                    .path
                    .clone()
            })
            .collect::<Vec<_>>();
        let radii = paths
            .iter()
            .map(|path| {
                let start = path.start();
                let end = path.end();
                (end.x - start.x).hypot(end.y - start.y)
            })
            .collect::<Vec<_>>();
        assert!((radii[0] - radii[1]).abs() < 2.0, "radii {radii:?}");
        assert!((radii[2] - radii[1]).abs() < 2.0, "radii {radii:?}");
        assert!(
            paths[0].measure_arc_length().unwrap().total_length()
                > paths[1].measure_arc_length().unwrap().total_length()
        );
        assert!(
            paths[1].measure_arc_length().unwrap().total_length()
                > paths[2].measure_arc_length().unwrap().total_length()
        );
    }
}

/// Proves along-parametric site intervals adapt with the same Feature size as the source path.
///
/// # Panics
/// Panics if evaluation fails or finer spacing does not increase the site population.
#[test]
fn parametric_sites_follow_feature_size() {
    let definition = parametric_definition(true);
    let plan = resolve_pattern_pipeline(&definition).expect("parametric site family resolves");
    let outputs = [0.5, 1.0, 2.0]
        .into_iter()
        .map(|feature_size| {
            evaluate_typed_family_product_with_source_cancellable(
                &plan.family,
                &request_for_feature_size(feature_size),
                None,
                &|| false,
            )
            .expect("parametric site family evaluates")
        })
        .collect::<Vec<_>>();
    assert!(outputs[0].site_set().len() > outputs[1].site_set().len());
    assert!(outputs[1].site_set().len() > outputs[2].site_set().len());
}
