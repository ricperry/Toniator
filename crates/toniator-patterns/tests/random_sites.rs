use std::fs;

use toniator_domain::{
    ArtworkWeightResponse, CanvasSpec, CoveragePolicy, DensityMetric2D, PatternDefinition,
    PatternDefinitionId, PatternMechanism, PatternMechanismId, PatternOutputLayerId,
    RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy, SourceMapping,
    SourceMappingComponent, SourcePlacement, validate_pattern_definition,
};
use toniator_patterns::{
    GridInspectRequest, MarkResponse, RandomSiteDiagnostics, StructuralProductCapability,
    TypedFamilyOutput, evaluate_typed_family_product_cancellable,
    evaluate_typed_family_product_with_source_cancellable, realize_typed_mapped_outputs,
    realize_typed_source_color_outputs, resolve_pattern_pipeline,
};
use toniator_sampling::{SourceFormatHint, decode_source};

fn request() -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec {
            width: 120.0,
            height: 80.0,
        },
        density: DensityMetric2D {
            across_x: 10.0,
            across_y: 6.0,
            aspect_locked: false,
        },
        rotation_degrees: 17.0,
        translation_x: 3.25,
        translation_y: -4.5,
        guard_steps: 2,
        support_radius: 4.0,
        max_family_candidates: 20_000,
    }
}

fn natural_request() -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        density: DensityMetric2D {
            across_x: 102.0,
            across_y: 102.0,
            aspect_locked: true,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 2,
        support_radius: 4.5,
        max_family_candidates: 1_048_576,
    }
}

fn definition(
    character: RandomSiteCharacter,
    modulation: SiteDensityModulation,
    exclusion: SiteExclusionPolicy,
    maximum_attempts: u32,
) -> PatternDefinition {
    PatternDefinition::random_sites(
        PatternDefinitionId(41),
        "site mechanisms",
        PatternMechanismId(101),
        PatternMechanismId(102),
        PatternMechanismId(103),
        PatternMechanismId(104),
        PatternOutputLayerId(105),
        character,
        0x1234_5678,
        modulation,
        exclusion,
        maximum_attempts,
        16_000_000,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.0,
        },
    )
}

fn output(definition: &PatternDefinition) -> TypedFamilyOutput {
    let plan = resolve_pattern_pipeline(definition).unwrap();
    assert_eq!(
        plan.family.product,
        StructuralProductCapability::RandomSites
    );
    evaluate_typed_family_product_cancellable(&plan.family, &request(), &|| false).unwrap()
}

fn output_with_seed(mut definition: PatternDefinition, seed: u32) -> TypedFamilyOutput {
    let PatternMechanism::RandomSiteProcess { seed: stored, .. } = &mut definition.mechanisms[0]
    else {
        panic!("expected random process");
    };
    *stored = seed;
    output(&definition)
}

/// Computes the observed lower pair distance from truthful evaluator sites.
fn pairwise_minimum(output: &TypedFamilyOutput) -> f64 {
    let sites = output.site_set().sites();
    sites
        .iter()
        .enumerate()
        .flat_map(|(index, first)| sites[index + 1..].iter().map(move |second| (first, second)))
        .map(|(first, second)| {
            let dx = first.position.x - second.position.x;
            let dy = first.position.y - second.position.y;
            (dx * dx + dy * dy).sqrt()
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn raw_even_and_clustered_are_seeded_distinct_and_even_is_not_an_alias() {
    let raw = output(&definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        32,
    ));
    let raw_repeat = output(&definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        32,
    ));
    let even = output(&definition(
        RandomSiteCharacter::Even {
            minimum_center_distance: 8.0,
        },
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        32,
    ));
    let clustered = output(&definition(
        RandomSiteCharacter::Clustered {
            cluster_density: 0.15,
            cluster_spread: 5.0,
            cluster_strength: 0.9,
        },
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        32,
    ));
    assert_eq!(raw, raw_repeat);
    assert_ne!(raw.family_fingerprint(), even.family_fingerprint());
    assert_ne!(raw.family_fingerprint(), clustered.family_fingerprint());
    assert!(pairwise_minimum(&even) >= 8.0);
    assert!(pairwise_minimum(&raw) < 8.0);
}

/// Locks random-family geometry and identity outside the centered grid-local transform contract.
///
/// # Panics
///
/// Panics when a straight-grid origin correction changes random-site distribution authority.
#[test]
fn random_family_geometry_and_identity_ignore_grid_local_origin_corrections() {
    let raw = output(&definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        32,
    ));
    assert_eq!(
        raw.family_fingerprint(),
        "fnv1a64:17bc903a56a17094:nominal-cell-basis:fnv1a64:196b26aef625706e"
    );
    assert_eq!(
        raw.site_set()
            .sites()
            .iter()
            .take(3)
            .map(|site| site.position)
            .collect::<Vec<_>>(),
        vec![
            toniator_patterns::Point2::new(27.267939708601112, 30.89005276569381),
            toniator_patterns::Point2::new(8.362231366714191, 29.374360154250454),
            toniator_patterns::Point2::new(118.74257947936493, 21.419246591692083),
        ]
    );
}

#[test]
/// Proves zero-seed normalization and process metrics remain deterministic.
fn zero_seed_is_repeatable_distinct_and_quality_metrics_separate_processes() {
    let raw_definition = definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        64,
    );
    let raw_zero = output_with_seed(raw_definition.clone(), 0);
    assert_eq!(raw_zero, output_with_seed(raw_definition.clone(), 0));
    assert_ne!(
        raw_zero.family_fingerprint(),
        output_with_seed(raw_definition.clone(), 1).family_fingerprint()
    );
    let even = output_with_seed(
        definition(
            RandomSiteCharacter::Even {
                minimum_center_distance: 7.0,
            },
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            64,
        ),
        0,
    );
    let clustered = output_with_seed(
        definition(
            RandomSiteCharacter::Clustered {
                cluster_density: 0.1,
                cluster_spread: 4.0,
                cluster_strength: 0.95,
            },
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            64,
        ),
        0,
    );
    let occupancy = |value: &TypedFamilyOutput| {
        let mut cells = std::collections::BTreeSet::new();
        for site in value.site_set().sites() {
            cells.insert((
                (site.position.x / 20.0).floor() as i32,
                (site.position.y / 20.0).floor() as i32,
            ));
        }
        cells.len()
    };
    assert!(pairwise_minimum(&even) > pairwise_minimum(&raw_zero));
    assert!(occupancy(&clustered) < occupancy(&raw_zero));
    assert!(pairwise_minimum(&even) >= 7.0);
}

/// Proves minimum-center exclusion and bounded unsatisfied diagnostics remain truthful.
#[test]
fn minimum_center_exclusion_preserves_spacing_and_reports_unsatisfiable_density() {
    let center = output(&definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::MinimumCenterDistance { minimum: 9.0 },
        64,
    ));
    assert!(pairwise_minimum(&center) >= 9.0);
    let constrained = output(&definition(
        RandomSiteCharacter::Even {
            minimum_center_distance: 40.0,
        },
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        1,
    ));
    let diagnostics = constrained
        .random_diagnostics()
        .expect("random diagnostics");
    assert!(diagnostics.achieved_sites < diagnostics.requested_sites);
    assert_eq!(diagnostics.achieved_sites, constrained.site_set().len());
    assert_eq!(
        diagnostics.canvas_sites + diagnostics.guard_sites,
        diagnostics.achieved_sites
    );
    assert!(diagnostics.rejected_by_exclusion > 0);
}

/// Proves natural random sites remain the authority before both circle realizers.
///
/// # Panics
///
/// Panics when either realizer no longer projects the evaluated random site set.
#[test]
fn natural_random_product_publishes_sites_before_mapped_and_source_color_realization() {
    let constrained_definition = definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::MinimumCenterDistance { minimum: 8.0 },
        32,
    );
    let plan = resolve_pattern_pipeline(&constrained_definition).unwrap();
    let mut family_request = natural_request();
    family_request.support_radius = 10.0;
    let family =
        evaluate_typed_family_product_cancellable(&plan.family, &family_request, &|| false)
            .unwrap();
    let diagnostics = family.random_diagnostics().expect("random diagnostics");
    assert!(diagnostics.requested_sites >= 10_404);
    assert!(diagnostics.achieved_sites > 0);
    assert!(diagnostics.canvas_sites > 0);
    assert_eq!(diagnostics.achieved_sites, family.site_set().len());
    assert!(
        diagnostics.rejected_by_density
            + diagnostics.rejected_by_exclusion
            + diagnostics.rejected_outside_envelope
            <= diagnostics.candidates_considered
    );
    let assets = format!("{}/../../assets", env!("CARGO_MANIFEST_DIR"));
    let source = decode_source(
        &fs::read(format!("{assets}/raster-sample.png")).unwrap(),
        SourceFormatHint::Png,
    )
    .unwrap();
    let mapping = SourceMapping {
        component: SourceMappingComponent::Luminance,
        placement: SourcePlacement::StretchToCanvas,
        inverted: false,
        gain: 1.0,
        bias: 0.0,
    };
    let response = MarkResponse {
        minimum_fill: 0.2,
        maximum_fill: 0.9,
        rotation_offset_degrees: 0.0,
    };
    let mapped = realize_typed_mapped_outputs(
        &family,
        &plan,
        &source,
        &family_request.canvas,
        mapping,
        response,
    )
    .unwrap();
    assert_eq!(mapped.output.marks.len(), family.site_set().len());
    assert!(mapped.output.marks.iter().any(|mark| mark.radius > 0.0));
    let source_color = realize_typed_source_color_outputs(
        &family,
        &plan,
        &source,
        &family_request.canvas,
        mapping,
        response,
    )
    .unwrap();
    assert!(!source_color.output.marks.is_empty());
    assert!(source_color.output.marks.len() <= family.site_set().len());
    assert!(
        source_color
            .output
            .marks
            .iter()
            .any(|mark| mark.mark.radius > 0.0)
    );
}

/// Measures bounded natural distributions from truthful site sets without adapters.
///
/// # Panics
///
/// Panics when deterministic random distribution bounds no longer hold.
#[test]
fn natural_random_distribution_metrics_are_bounded_and_structurally_distinct() {
    let source_path = format!(
        "{}/../../assets/raster-sample.png",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = decode_source(&fs::read(source_path).unwrap(), SourceFormatHint::Png).unwrap();
    let mapping = SourceMapping {
        component: SourceMappingComponent::Luminance,
        placement: SourcePlacement::StretchToCanvas,
        inverted: false,
        gain: 1.0,
        bias: 0.0,
    };
    let raw_definition = definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        32,
    );
    let even_definition = definition(
        RandomSiteCharacter::Even {
            minimum_center_distance: 8.0,
        },
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        32,
    );
    let clustered_definition = definition(
        RandomSiteCharacter::Clustered {
            cluster_density: 0.001,
            cluster_spread: 18.0,
            cluster_strength: 1.0,
        },
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::MinimumCenterDistance { minimum: 8.0 },
        32,
    );
    let weighted_definition = definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::ArtworkWeighted {
            mapping,
            strength: 0.85,
            response: ArtworkWeightResponse::Linear,
        },
        SiteExclusionPolicy::MinimumCenterDistance { minimum: 8.0 },
        32,
    );
    let evaluate = |definition: &PatternDefinition, weighted: bool| {
        let plan = resolve_pattern_pipeline(definition).unwrap();
        evaluate_typed_family_product_with_source_cancellable(
            &plan.family,
            &natural_request(),
            weighted.then_some(&source),
            &|| false,
        )
        .unwrap()
    };
    let raw = evaluate(&raw_definition, false);
    let even = evaluate(&even_definition, false);
    let clustered = evaluate(&clustered_definition, false);
    let weighted = evaluate(&weighted_definition, true);
    let occupancy = |value: &TypedFamilyOutput| {
        let mut bins = std::collections::BTreeMap::<(i32, i32), usize>::new();
        for site in value.site_set().sites() {
            if !(0.0..=1024.0).contains(&site.position.x)
                || !(0.0..=1024.0).contains(&site.position.y)
            {
                continue;
            }
            let cell = (
                (site.position.x / 32.0).floor() as i32,
                (site.position.y / 32.0).floor() as i32,
            );
            *bins.entry(cell).or_default() += 1;
        }
        (bins.len(), bins.values().copied().max().unwrap_or(0))
    };
    let response_mean = |value: &TypedFamilyOutput| {
        value
            .site_set()
            .sites()
            .iter()
            .map(|site| {
                source
                    .sample_density_weight(site.position, &natural_request().canvas, mapping)
                    .unwrap()
            })
            .sum::<f64>()
            / value.site_set().len() as f64
    };
    fn random(value: &TypedFamilyOutput) -> &RandomSiteDiagnostics {
        value.random_diagnostics().expect("random diagnostics")
    }
    for value in [&raw, &even, &clustered, &weighted] {
        assert!(random(value).canvas_sites > 0);
        assert!(random(value).achieved_sites > 0);
        assert_eq!(random(value).achieved_sites, value.site_set().len());
    }
    let (raw_bins, _raw_peak) = occupancy(&raw);
    let (even_bins, _even_peak) = occupancy(&even);
    let (clustered_bins, _clustered_peak) = occupancy(&clustered);
    assert!(even_bins >= raw_bins.saturating_sub(8));
    assert!(clustered_bins * 2 < raw_bins);
    assert!(response_mean(&weighted) > response_mean(&raw));
}

#[test]
fn artwork_weighting_uses_decoded_pixel_identity_only_for_weighted_structure() {
    let mapping = SourceMapping {
        component: SourceMappingComponent::Luminance,
        placement: SourcePlacement::StretchToCanvas,
        inverted: false,
        gain: 1.0,
        bias: 0.0,
    };
    let weighted = definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::ArtworkWeighted {
            mapping,
            strength: 1.0,
            response: ArtworkWeightResponse::Linear,
        },
        SiteExclusionPolicy::MinimumCenterDistance { minimum: 4.0 },
        64,
    );
    let plan = resolve_pattern_pipeline(&weighted).unwrap();
    let assets = format!("{}/../../assets", env!("CARGO_MANIFEST_DIR"));
    let raster = decode_source(
        &fs::read(format!("{assets}/raster-sample.png")).unwrap(),
        SourceFormatHint::Png,
    )
    .unwrap();
    let vector = decode_source(
        &fs::read(format!("{assets}/vector-sample.svg")).unwrap(),
        SourceFormatHint::Svg,
    )
    .unwrap();
    let raster_output = evaluate_typed_family_product_with_source_cancellable(
        &plan.family,
        &request(),
        Some(&raster),
        &|| false,
    )
    .unwrap();
    let vector_output = evaluate_typed_family_product_with_source_cancellable(
        &plan.family,
        &request(),
        Some(&vector),
        &|| false,
    )
    .unwrap();
    assert_ne!(
        raster_output.family_fingerprint(),
        vector_output.family_fingerprint()
    );
    let error = evaluate_typed_family_product_with_source_cancellable(
        &plan.family,
        &request(),
        None,
        &|| false,
    )
    .unwrap_err();
    assert_eq!(error.path(), "pattern.family.random_sites.source");
}

#[test]
/// Proves artwork response changes only the truthful random structural result.
fn artwork_weight_response_changes_accepted_density_for_both_project_fields() {
    let mapping = SourceMapping {
        component: SourceMappingComponent::Luminance,
        placement: SourcePlacement::StretchToCanvas,
        inverted: false,
        gain: 1.0,
        bias: 0.0,
    };
    let assets = format!("{}/../../assets", env!("CARGO_MANIFEST_DIR"));
    for (path, format) in [
        (format!("{assets}/raster-sample.png"), SourceFormatHint::Png),
        (format!("{assets}/vector-sample.svg"), SourceFormatHint::Svg),
    ] {
        let field = decode_source(&fs::read(path).unwrap(), format).unwrap();
        let raw = definition(
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            64,
        );
        let linear = definition(
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::ArtworkWeighted {
                mapping,
                strength: 1.0,
                response: ArtworkWeightResponse::Linear,
            },
            SiteExclusionPolicy::None,
            64,
        );
        let smoothstep = definition(
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::ArtworkWeighted {
                mapping,
                strength: 1.0,
                response: ArtworkWeightResponse::Smoothstep,
            },
            SiteExclusionPolicy::None,
            64,
        );
        let raw_plan = resolve_pattern_pipeline(&raw).unwrap();
        let linear_plan = resolve_pattern_pipeline(&linear).unwrap();
        let smoothstep_plan = resolve_pattern_pipeline(&smoothstep).unwrap();
        let raw = evaluate_typed_family_product_with_source_cancellable(
            &raw_plan.family,
            &request(),
            Some(&field),
            &|| false,
        )
        .unwrap();
        let linear = evaluate_typed_family_product_with_source_cancellable(
            &linear_plan.family,
            &request(),
            Some(&field),
            &|| false,
        )
        .unwrap();
        let smoothstep = evaluate_typed_family_product_with_source_cancellable(
            &smoothstep_plan.family,
            &request(),
            Some(&field),
            &|| false,
        )
        .unwrap();
        let mean = |value: &TypedFamilyOutput| {
            value
                .site_set()
                .sites()
                .iter()
                .map(|site| {
                    field
                        .sample_density_weight(site.position, &request().canvas, mapping)
                        .unwrap()
                })
                .sum::<f64>()
                / value.site_set().len() as f64
        };
        assert!(mean(&linear) > mean(&raw));
        assert_ne!(linear.family_fingerprint(), smoothstep.family_fingerprint());
        assert_ne!(linear.site_set().sites(), smoothstep.site_set().sites());
    }
}

#[test]
fn validation_rejects_random_chain_order_nonfinite_and_bad_limits_before_evaluation() {
    let mut invalid = definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        16,
    );
    invalid.mechanisms.swap(0, 1);
    assert_eq!(
        validate_pattern_definition(&invalid).unwrap_err().path(),
        "pattern_definitions.family"
    );
    let invalid = definition(
        RandomSiteCharacter::Even {
            minimum_center_distance: f64::NAN,
        },
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        16,
    );
    assert_eq!(
        validate_pattern_definition(&invalid).unwrap_err().path(),
        "pattern_definitions.mechanisms.random_sites.minimum_center_distance"
    );
    let mut zero_attempts = definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        16,
    );
    let PatternMechanism::RandomSiteProduct {
        maximum_attempts, ..
    } = &mut zero_attempts.mechanisms[3]
    else {
        panic!("expected site product");
    };
    *maximum_attempts = 0;
    assert_eq!(
        validate_pattern_definition(&zero_attempts)
            .unwrap_err()
            .path(),
        "pattern_definitions.mechanisms.random_sites.maximum_attempts"
    );
    let mut zero_neighbors = definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        16,
    );
    let PatternMechanism::RandomSiteProduct {
        maximum_neighbor_checks,
        ..
    } = &mut zero_neighbors.mechanisms[3]
    else {
        panic!("expected site product");
    };
    *maximum_neighbor_checks = 0;
    assert_eq!(
        validate_pattern_definition(&zero_neighbors)
            .unwrap_err()
            .path(),
        "pattern_definitions.mechanisms.random_sites.maximum_neighbor_checks"
    );
}

#[test]
fn random_candidate_and_spatial_exclusion_work_obey_cancellation_and_limits() {
    let constrained_definition = definition(
        RandomSiteCharacter::Even {
            minimum_center_distance: 4.0,
        },
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::MinimumCenterDistance { minimum: 4.0 },
        64,
    );
    let plan = resolve_pattern_pipeline(&constrained_definition).unwrap();
    let polls = std::cell::Cell::new(0_u32);
    let cancelled = evaluate_typed_family_product_cancellable(&plan.family, &request(), &|| {
        let next = polls.get() + 1;
        polls.set(next);
        next > 12
    })
    .unwrap_err();
    assert_eq!(cancelled.path(), "evaluation.cancelled");
    let mut limited = request();
    limited.max_family_candidates = 1;
    let error =
        evaluate_typed_family_product_cancellable(&plan.family, &limited, &|| false).unwrap_err();
    assert_eq!(error.path(), "coverage.candidate_limit");
    let mut neighbor_limited = definition(
        RandomSiteCharacter::RawUniform,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::MinimumCenterDistance { minimum: 4.0 },
        64,
    );
    let PatternMechanism::RandomSiteProduct {
        maximum_neighbor_checks,
        ..
    } = &mut neighbor_limited.mechanisms[3]
    else {
        panic!("expected site product");
    };
    *maximum_neighbor_checks = 1;
    let plan = resolve_pattern_pipeline(&neighbor_limited).unwrap();
    let error =
        evaluate_typed_family_product_cancellable(&plan.family, &request(), &|| false).unwrap_err();
    assert_eq!(error.path(), "coverage.random_sites.neighbor_limit");
}
