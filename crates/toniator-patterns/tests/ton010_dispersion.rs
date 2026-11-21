use std::fs;
use toniator_domain::{
    CanvasSpec, CoveragePolicy, DensityMetric2D, DispersionLayout, PatternDefinition,
    PatternDefinitionId, PatternMechanismId, PatternOutputLayerId, RandomSiteCharacter,
    SiteDensityModulation, SiteExclusionPolicy, SourceMappingComponent, SourceWeighting,
};
use toniator_patterns::{
    GridInspectRequest, TypedFamilyOutput, evaluate_typed_family_product_with_source_cancellable,
    resolve_pattern_pipeline_for_channel,
};
use toniator_sampling::{SourceField, SourceFormatHint, decode_source};

const LAYOUTS: [DispersionLayout; 7] = [
    DispersionLayout::Bayer,
    DispersionLayout::FloydSteinberg,
    DispersionLayout::InkQuadtree,
    DispersionLayout::TangentSpacing,
    DispersionLayout::TangentSize,
    DispersionLayout::TangentBlend,
    DispersionLayout::ClusterScreen,
];

/// Builds one mark-only intrinsic layout through the same typed recipe used by the Wizard.
fn definition(layout: DispersionLayout) -> PatternDefinition {
    PatternDefinition::random_sites(
        PatternDefinitionId(1),
        "Dispersion",
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternMechanismId(4),
        PatternMechanismId(5),
        PatternOutputLayerId(6),
        RandomSiteCharacter::Dispersion { layout },
        43,
        SiteDensityModulation::ArtworkWeighted,
        SiteExclusionPolicy::None,
        1_000_000,
        1_000_000,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    )
}

/// Evaluates an intrinsic family with mapped channel tone and a canvas-independent Feature size.
/// # Panics
/// Panics if current domain projection or shared family generation rejects the test input.
fn evaluate(
    layout: DispersionLayout,
    source: &SourceField,
    component: SourceMappingComponent,
    size: f64,
) -> TypedFamilyOutput {
    let plan = resolve_pattern_pipeline_for_channel(
        &definition(layout),
        SourceWeighting::canonical(component),
    )
    .unwrap();
    let canvas = CanvasSpec {
        width: 256.0,
        height: 256.0,
    };
    let request = GridInspectRequest {
        density: DensityMetric2D {
            density: 100.0 / size,
            aspect: 1.0,
        }
        .resolve(&canvas)
        .unwrap(),
        canvas,
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 1,
        support_radius: 8.0,
        max_family_candidates: 100_000,
    };
    evaluate_typed_family_product_with_source_cancellable(
        &plan.family,
        &request,
        Some(source),
        &|| false,
    )
    .unwrap()
}

/// Proves all new placements replay deterministically, differ in arrangement, and honor channel identity.
/// # Panics
/// Panics if full ink contains missing sites, empty ink survives, or two layouts alias.
#[test]
fn dispersion_replay_channel_and_full_ink_contracts() {
    let cyan = SourceField::from_straight_rgba8(8, 8, [0, 255, 255, 255].repeat(64)).unwrap();
    let mut signatures = std::collections::BTreeSet::new();
    for layout in LAYOUTS {
        let first = evaluate(layout, &cyan, SourceMappingComponent::Cyan, 1.0);
        let again = evaluate(layout, &cyan, SourceMappingComponent::Cyan, 1.0);
        assert_eq!(first.site_set().sites(), again.site_set().sites());
        assert!(
            first.random_diagnostics().unwrap().canvas_sites > 400,
            "{layout:?}"
        );
        let empty = evaluate(layout, &cyan, SourceMappingComponent::Magenta, 1.0);
        if layout == DispersionLayout::TangentSize {
            assert!(empty.random_diagnostics().unwrap().canvas_sites > 400);
        } else {
            assert_eq!(
                empty.random_diagnostics().unwrap().canvas_sites,
                0,
                "{layout:?}"
            );
        }
        if !matches!(
            layout,
            DispersionLayout::TangentSize | DispersionLayout::TangentBlend
        ) {
            signatures.insert(format!(
                "{:?}",
                first
                    .site_set()
                    .sites()
                    .iter()
                    .take(12)
                    .map(|site| site.position)
                    .collect::<Vec<_>>()
            ));
        }
    }
    // Full-field Bayer and Floyd intentionally share a fully occupied lattice.
    assert!(signatures.len() >= 3);
}

/// Proves both immutable artworks exercise every layout and Feature size scales offered detail.
/// # Panics
/// Panics if sample decoding, cancellation, structural scaling or output replay fails.
#[test]
fn dispersion_both_artworks_feature_size_and_cancellation() {
    for (name, hint) in [
        ("raster-sample.png", SourceFormatHint::Png),
        ("vector-sample.svg", SourceFormatHint::Svg),
    ] {
        let bytes = fs::read(format!(
            "{}/../../assets/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let source = decode_source(&bytes, hint).unwrap();
        for layout in LAYOUTS {
            let fine = evaluate(layout, &source, SourceMappingComponent::Cyan, 1.0);
            let coarse = evaluate(layout, &source, SourceMappingComponent::Cyan, 2.0);
            assert!(
                fine.random_diagnostics().unwrap().canvas_sites
                    > coarse.random_diagnostics().unwrap().canvas_sites,
                "{name} {layout:?}"
            );
            let plan = resolve_pattern_pipeline_for_channel(
                &definition(layout),
                SourceWeighting::canonical(SourceMappingComponent::Cyan),
            )
            .unwrap();
            let canvas = CanvasSpec {
                width: 256.0,
                height: 256.0,
            };
            let request = GridInspectRequest {
                density: DensityMetric2D {
                    density: 100.0,
                    aspect: 1.0,
                }
                .resolve(&canvas)
                .unwrap(),
                canvas,
                rotation_degrees: 0.0,
                translation_x: 0.0,
                translation_y: 0.0,
                guard_steps: 1,
                support_radius: 8.0,
                max_family_candidates: 100_000,
            };
            assert!(
                evaluate_typed_family_product_with_source_cancellable(
                    &plan.family,
                    &request,
                    Some(&source),
                    &|| true
                )
                .is_err()
            );
        }
    }
}

/// Verifies tangent rows respond to orthogonal gradients and continuous radial curvature.
/// # Panics
/// Panics if tangent geometry becomes a fixed-angle carrier or grows a 64px grid void.
#[test]
fn tangent_rows_follow_orthogonal_and_radial_guides() {
    for axis in [0, 1, 2] {
        let mut rgba = Vec::new();
        for y in 0..256 {
            for x in 0..256 {
                let q = match axis {
                    0 => x as f64 / 255.0,
                    1 => y as f64 / 255.0,
                    _ => ((x as f64 - 128.0).hypot(y as f64 - 128.0) / 181.0).min(1.0),
                };
                rgba.extend([((1.0 - q) * 255.0).round() as u8, 255, 255, 255]);
            }
        }
        let source = SourceField::from_straight_rgba8(256, 256, rgba).unwrap();
        let family = evaluate(
            DispersionLayout::TangentSize,
            &source,
            SourceMappingComponent::Cyan,
            1.0,
        );
        let points = family.site_set().sites();
        let mut aligned = 0;
        let mut measured = 0;
        for site in points.iter().filter(|site| {
            site.position.x > 32.0
                && site.position.x < 224.0
                && site.position.y > 32.0
                && site.position.y < 224.0
        }) {
            // Same-contour adjacent sites are emitted in traversal order.
            if let Some(next) = points.get(site.id.ordinal + 1) {
                let dx = next.position.x - site.position.x;
                let dy = next.position.y - site.position.y;
                if dx.hypot(dy) < 16.0 && dx.hypot(dy) > 4.0 {
                    let (nx, ny) = match axis {
                        0 => (1.0, 0.0),
                        1 => (0.0, 1.0),
                        _ => (site.position.x - 128.0, site.position.y - 128.0),
                    };
                    let cosine = (dx * nx + dy * ny).abs() / (dx.hypot(dy) * nx.hypot(ny));
                    measured += 1;
                    if cosine < 0.35 {
                        aligned += 1;
                    }
                }
            }
        }
        assert!(measured > 100);
        assert!(
            aligned as f64 / measured as f64 > 0.8,
            "axis={axis} aligned={aligned}/{measured}"
        );
    }
}

/// Checks standard Bayer bit significance and cutoff precedence over nonzero desired bounds.
/// # Panics
/// Panics if the ordered weave forms blocks or enabled cutoff resurrects minimum ink.
#[test]
fn bayer_weave_and_dispersion_cutoff_precedence() {
    let source = SourceField::from_straight_rgba8(4, 4, [253, 255, 255, 255].repeat(16)).unwrap();
    let canvas = CanvasSpec {
        width: 256.0,
        height: 256.0,
    };
    let request = GridInspectRequest {
        density: DensityMetric2D {
            density: 100.0,
            aspect: 1.0,
        }
        .resolve(&canvas)
        .unwrap(),
        canvas,
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 1,
        support_radius: 8.0,
        max_family_candidates: 100_000,
    };
    for tone in [0.5, 2.25 / 64.0] {
        let mut weighting = SourceWeighting::canonical(SourceMappingComponent::Cyan);
        weighting.mapping.gain = 0.0;
        weighting.mapping.bias = tone;
        let plan =
            resolve_pattern_pipeline_for_channel(&definition(DispersionLayout::Bayer), weighting)
                .unwrap();
        let family = evaluate_typed_family_product_with_source_cancellable(
            &plan.family,
            &request,
            Some(&source),
            &|| false,
        )
        .unwrap();
        let cells = family
            .site_set()
            .sites()
            .iter()
            .filter(|site| site.position.x >= 0.0 && site.position.y >= 0.0)
            .map(|site| {
                (
                    (site.position.x / 10.24).floor() as i64,
                    (site.position.y / 10.24).floor() as i64,
                )
            })
            .collect::<std::collections::BTreeSet<_>>();
        if tone == 0.5 {
            assert!(cells.iter().all(|(x, y)| (x + y) % 2 == 0));
            assert!(cells.contains(&(0, 0)));
            assert!(!cells.contains(&(1, 0)));
        } else {
            assert!(cells.contains(&(4, 0)), "rank(4,0)=2");
            assert!(!cells.contains(&(1, 0)), "rank(1,0)=32");
        }
    }
    for layout in LAYOUTS
        .into_iter()
        .filter(|layout| *layout != DispersionLayout::TangentSize)
    {
        let mut weighting = SourceWeighting::canonical(SourceMappingComponent::Cyan);
        weighting.mapping.tone.cutoff = 0.05;
        let mut plan =
            resolve_pattern_pipeline_for_channel(&definition(layout), weighting).unwrap();
        plan.family.random.as_mut().unwrap().dispersion_target_range = Some((0.2, 0.2));
        let family = evaluate_typed_family_product_with_source_cancellable(
            &plan.family,
            &request,
            Some(&source),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            family.random_diagnostics().unwrap().canvas_sites,
            0,
            "{layout:?}: cutoff must precede nonzero minimum"
        );
    }
}

/// Conserves full-size opportunities across curved guides and retains independent Fill authority.
/// # Panics
/// Panics if curved coverage loses area or weighting-zero removes Fill-positive Size marks.
#[test]
fn tangent_area_and_independent_fill_mapping_contract() {
    use toniator_domain::{Document, SourceMapping, SourceReference};
    use toniator_patterns::{CanonicalMarkRequest, MarkResponse, realize_typed_canonical_marks};
    let cyan = SourceField::from_straight_rgba8(8, 8, [0, 255, 255, 255].repeat(64)).unwrap();
    let family = evaluate(
        DispersionLayout::TangentSize,
        &cyan,
        SourceMappingComponent::Magenta,
        1.0,
    );
    assert!(family.random_diagnostics().unwrap().canvas_sites > 600);
    let plan = resolve_pattern_pipeline_for_channel(
        &definition(DispersionLayout::TangentSize),
        SourceWeighting::canonical(SourceMappingComponent::Magenta),
    )
    .unwrap();
    let canvas = CanvasSpec {
        width: 256.0,
        height: 256.0,
    };
    let document =
        Document::new_default_document(canvas.clone(), SourceReference::Unassigned).unwrap();
    let realize = |source: &SourceField, component| {
        realize_typed_canonical_marks(
            &document,
            &family,
            &plan,
            source,
            &canvas,
            CanonicalMarkRequest {
                mapping: SourceMapping::canonical(component),
                sampled_paint: true,
                response: MarkResponse {
                    minimum_fill: 0.0,
                    maximum_fill: 1.0,
                    rotation_offset_degrees: 0.0,
                },
                max_transformed_curve_segment_instances: usize::MAX,
            },
        )
        .unwrap()
    };
    let solid = realize(&cyan, SourceMappingComponent::Cyan);
    assert!(
        !solid.output.marks.is_empty(),
        "weighting Magenta zero still offers Size marks for Cyan Fill"
    );
    assert!(
        realize(&cyan, SourceMappingComponent::Magenta)
            .output
            .marks
            .is_empty()
    );
    let half = SourceField::from_straight_rgba8(8, 8, [0, 255, 255, 128].repeat(64)).unwrap();
    let partial = realize(&half, SourceMappingComponent::Cyan);
    let radius = |mark: &toniator_geometry::CanonicalMark| match mark {
        toniator_geometry::CanonicalMark::Circle { radius, .. } => *radius,
        _ => panic!("circle fixture"),
    };
    assert!(
        (radius(&partial.output.marks[0]) / radius(&solid.output.marks[0]) - 128.0 / 255.0).abs()
            < 1e-10,
        "source alpha enters the Fill response once"
    );
    assert!(
        partial
            .output
            .paints
            .unwrap()
            .iter()
            .all(|paint| paint.alpha == 1.0)
    );
    for (name, hint) in [
        ("raster-sample.png", SourceFormatHint::Png),
        ("vector-sample.svg", SourceFormatHint::Svg),
    ] {
        let source = decode_source(
            &fs::read(format!(
                "{}/../../assets/{name}",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
            hint,
        )
        .unwrap();
        let family = evaluate(
            DispersionLayout::TangentSize,
            &source,
            SourceMappingComponent::Cyan,
            1.0,
        );
        let count = family.random_diagnostics().unwrap().canvas_sites;
        assert!(
            (600..=650).contains(&count),
            "{name}: expected 625 area opportunities, got {count}"
        );
    }
}

/// Avoids an exact-one numerical plateau inflating the fixed full-tone mark footprint.
/// # Panics
/// Panics if near-full selection ignores the first measured attainable near-full knot.
#[test]
fn dispersion_full_tone_uses_first_near_full_measured_footprint() {
    let curve = toniator_patterns::CoverageCurve::new(vec![
        toniator_patterns::CoverageKnot {
            response: 0.0,
            coverage: 0.0,
        },
        toniator_patterns::CoverageKnot {
            response: 0.5,
            coverage: 0.99999,
        },
        toniator_patterns::CoverageKnot {
            response: 0.75,
            coverage: 1.0,
        },
        toniator_patterns::CoverageKnot {
            response: 1.0,
            coverage: 1.0,
        },
    ])
    .unwrap();
    assert_eq!(curve.near_full_coverage(), 0.99999);
    assert_eq!(
        curve.inverse_absolute(curve.near_full_coverage()).unwrap(),
        0.5
    );
}
