use std::fs;

use toniator_domain::{
    ArtworkWeightResponse, CanvasSpec, CoveragePolicy, DensityMetric2D, PatternDefinition,
    PatternDefinitionId, PatternMechanismId, PatternOutputLayerId, RandomSiteCharacter,
    SiteDensityModulation, SiteExclusionPolicy, SourceMapping, SourceMappingComponent,
};
use toniator_patterns::{
    GridInspectRequest, ResolvedSiteDensityModulation,
    evaluate_typed_family_product_with_source_cancellable, resolve_pattern_pipeline,
};
use toniator_sampling::{SourceFormatHint, decode_source};

/// Builds one seed-stable scatter mechanism without a count parameter.
fn family(
    character: RandomSiteCharacter,
    exclusion: SiteExclusionPolicy,
) -> toniator_patterns::FamilyCapability {
    let definition = PatternDefinition::random_sites(
        PatternDefinitionId(1),
        "Spatial scatter",
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternMechanismId(4),
        PatternMechanismId(5),
        PatternOutputLayerId(6),
        character,
        17,
        SiteDensityModulation::Uniform,
        exclusion,
        16_000_000,
        16_000_000,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    resolve_pattern_pipeline(&definition).unwrap().family
}

/// Resolves one canvas-independent Feature size to a complete guarded request.
fn request(width: f64, height: f64, feature_size: f64) -> GridInspectRequest {
    let canvas = CanvasSpec { width, height };
    GridInspectRequest {
        density: DensityMetric2D {
            density: 100.0 / feature_size,
            aspect: 1.0,
        }
        .resolve(&canvas)
        .unwrap(),
        canvas,
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 1,
        support_radius: 1.0,
        max_family_candidates: 200_000,
    }
}

/// Counts only sites in the receiving canvas, leaving the guard topology intact.
fn canvas_count(
    family: &toniator_patterns::FamilyCapability,
    request: &GridInspectRequest,
) -> usize {
    evaluate_typed_family_product_with_source_cancellable(family, request, None, &|| false)
        .unwrap()
        .random_diagnostics()
        .unwrap()
        .canvas_sites
}

/// Proves all four processes retain area density across canvas size/aspect and roughly
/// quadruple density when Feature size halves, without an accepted-count authority.
///
/// # Panics
/// Panics if a valid population fails or observed density departs materially from spatial intent.
#[test]
fn four_scatter_processes_follow_spatial_extent_and_feature_scale() {
    let characters = [
        RandomSiteCharacter::RawUniform,
        RandomSiteCharacter::Stratified { jitter: 0.7 },
        RandomSiteCharacter::Clustered {
            cluster_density: 12.0,
            cluster_spread: 5.0,
            cluster_strength: 0.7,
        },
        RandomSiteCharacter::Even {
            minimum_center_distance: 8.0,
        },
    ];
    for character in characters {
        let authored_spacing = match character {
            RandomSiteCharacter::Even {
                minimum_center_distance,
            } => minimum_center_distance,
            _ => 10.24,
        };
        let family = family(character.clone(), SiteExclusionPolicy::None);
        let small = canvas_count(&family, &request(160.0, 120.0, 1.0));
        let large = canvas_count(&family, &request(320.0, 240.0, 1.0));
        let wide = canvas_count(&family, &request(240.0, 80.0, 1.0));
        let fine = canvas_count(&family, &request(160.0, 120.0, 0.5));
        let area_ratio = large as f64 / small as f64;
        let aspect_ratio = wide as f64 / small as f64;
        let feature_ratio = fine as f64 / small as f64;
        println!(
            "spatial_population,{character:?},small={small},double_area_axes={large},equal_area_wide={wide},half_feature={fine},basis_at_feature_1={authored_spacing}"
        );
        assert!(
            (2.8..5.2).contains(&area_ratio),
            "{character:?}: area ratio {area_ratio}"
        );
        assert!(
            (0.65..1.35).contains(&aspect_ratio),
            "{character:?}: aspect ratio {aspect_ratio}"
        );
        assert!(
            (2.8..5.2).contains(&feature_ratio),
            "{character:?}: size ratio {feature_ratio}"
        );
        for (width, height, feature_size) in [
            (160.0, 120.0, 1.0),
            (320.0, 240.0, 1.0),
            (240.0, 80.0, 1.0),
            (160.0, 120.0, 0.5),
        ] {
            let output = evaluate_typed_family_product_with_source_cancellable(
                &family,
                &request(width, height, feature_size),
                None,
                &|| false,
            )
            .unwrap();
            let diameter = output.site_set().sites()[0].nominal_cell_basis.diameter();
            let measured_spacing = diameter / 2.0_f64.sqrt();
            assert!(
                (measured_spacing - authored_spacing * feature_size).abs() < 1.0e-9,
                "{character:?}: canvas {width}x{height}, size {feature_size}, spacing {measured_spacing}"
            );
        }
    }
}

/// Proves source weighting thins one complete offered field without replacing rejected sites.
/// Both immutable project sources exercise the same source-to-placement boundary.
///
/// # Panics
/// Panics if decoding, guarded generation, or one-pass thinning fails.
#[test]
fn artwork_weighting_does_not_replenish_rejected_candidates() {
    let normal = family(RandomSiteCharacter::RawUniform, SiteExclusionPolicy::None);
    let request = request(160.0, 120.0, 1.0);
    let unweighted =
        evaluate_typed_family_product_with_source_cancellable(&normal, &request, None, &|| false)
            .unwrap();
    let unweighted = unweighted.random_diagnostics().unwrap();
    assert!(unweighted.canvas_sites > 100);
    for (name, hint) in [
        ("raster-sample.png", SourceFormatHint::Png),
        ("vector-sample.svg", SourceFormatHint::Svg),
    ] {
        let path = format!("{}/../../assets/{name}", env!("CARGO_MANIFEST_DIR"));
        let source = decode_source(&fs::read(path).unwrap(), hint).unwrap();
        let mut weighted = normal.clone();
        let mapping = SourceMapping {
            gain: 0.0,
            ..SourceMapping::canonical(SourceMappingComponent::Luminance)
        };
        weighted.random.as_mut().unwrap().density_modulation =
            ResolvedSiteDensityModulation::ArtworkWeighted {
                mapping,
                strength: 1.0,
                response: ArtworkWeightResponse::Linear,
            };
        let result = evaluate_typed_family_product_with_source_cancellable(
            &weighted,
            &request,
            Some(&source),
            &|| false,
        )
        .unwrap();
        let diagnostics = result.random_diagnostics().unwrap();
        assert_eq!(diagnostics.canvas_sites, 0, "{name}");
        assert!(diagnostics.guard_sites > 0, "{name}");
        assert_eq!(
            diagnostics.offered_candidates,
            unweighted.offered_candidates
        );
        assert_eq!(
            diagnostics.candidates_considered,
            diagnostics.offered_candidates
        );
        assert!(diagnostics.rejected_by_density > 0, "{name}");
        weighted.random.as_mut().unwrap().density_modulation =
            ResolvedSiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping {
                    bias: 0.5,
                    ..mapping
                },
                strength: 1.0,
                response: ArtworkWeightResponse::Linear,
            };
        let half = evaluate_typed_family_product_with_source_cancellable(
            &weighted,
            &request,
            Some(&source),
            &|| false,
        )
        .unwrap();
        let half = half.random_diagnostics().unwrap();
        assert_eq!(half.offered_candidates, unweighted.offered_candidates);
        assert_eq!(half.candidates_considered, half.offered_candidates);
        assert!(
            half.canvas_sites as f64 > unweighted.canvas_sites as f64 * 0.25
                && (half.canvas_sites as f64) < unweighted.canvas_sites as f64 * 0.75,
            "{name}: half weight retained {} of {} canvas sites",
            half.canvas_sites,
            unweighted.canvas_sites
        );
    }
}

/// Proves strict visible-margin exclusion retains its requested separation and a bounded
/// candidate failure returns a diagnostic rather than an underfilled successful family.
///
/// # Panics
/// Panics if margin, guard population, or candidate-limit authority is violated.
#[test]
fn strict_margin_guards_and_budget_remain_authoritative() {
    let family = family(
        RandomSiteCharacter::RawUniform,
        SiteExclusionPolicy::VisibleMarkMargin { margin: 1.0 },
    );
    let mut request = request(160.0, 120.0, 1.0);
    request.support_radius = 3.0;
    let output =
        evaluate_typed_family_product_with_source_cancellable(&family, &request, None, &|| false)
            .unwrap();
    let diagnostics = output.random_diagnostics().unwrap();
    assert!(diagnostics.guard_sites > 0);
    let sites = output.site_set().sites();
    for (index, first) in sites.iter().enumerate() {
        for second in &sites[index + 1..] {
            assert!(first.position.x.hypot(first.position.y).is_finite());
            let distance =
                (first.position.x - second.position.x).hypot(first.position.y - second.position.y);
            assert!(distance + 1.0e-9 >= 7.0, "strict separation {distance}");
        }
    }
    request.max_family_candidates = 10;
    assert_eq!(
        evaluate_typed_family_product_with_source_cancellable(&family, &request, None, &|| false,)
            .unwrap_err()
            .path(),
        "coverage.candidate_limit"
    );
}
