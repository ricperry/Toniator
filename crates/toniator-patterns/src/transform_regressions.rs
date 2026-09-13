//! Current Gate 3 transform witnesses through the production family dispatch.

use super::*;
use toniator_domain::{
    CoveragePolicy, DocumentSession, GeneralizedSiteProduct, PatternDefinitionId,
};

/// Resolves representative legacy, generalized, authored-guide and parametric dispatch inputs.
/// Document-owned resources resolve through the same authority as engine evaluation.
///
/// # Panics
/// Panics when current fixture construction or bundled recipe resolution fails.
fn families(canvas: &CanvasSpec) -> Vec<(&'static str, FamilyCapability)> {
    let mut result = Vec::new();
    for (name, angles, along) in [
        ("legacy grid", vec![0.0, 90.0], false),
        ("generalized intersections", vec![0.0, 60.0, 120.0], false),
        ("generalized along guides", vec![0.0, 60.0], true),
    ] {
        let dimensions: Vec<_> = angles
            .into_iter()
            .enumerate()
            .map(|(index, angle)| StraightGuideDimension {
                id: GuideDimensionId(index as u64 + 1),
                baseline_angle_degrees: angle,
                phase: 0.0,
                repetition: toniator_domain::StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            })
            .collect();
        let ids = dimensions.iter().map(|dimension| dimension.id).collect();
        let product = if along {
            GeneralizedSiteProduct::AlongGuides {
                dimensions: ids,
                interval_multiplier: 0.5,
                phase: 0.0,
            }
        } else {
            GeneralizedSiteProduct::Intersections {
                dimensions: ids,
                merge_epsilon: 1e-9,
            }
        };
        let definition = PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(1),
            name,
            PatternMechanismId(2),
            PatternMechanismId(3),
            PatternOutputLayerId(4),
            dimensions,
            product,
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
        );
        result.push((name, resolve_pattern_pipeline(&definition).unwrap().family));
    }
    for name in [
        "three-guide-cells-scale",
        "residual-sites-along-guide",
        "round-spiral-line",
        "round-spiral-marks",
    ] {
        let document =
            Document::new_default_document(canvas.clone(), SourceReference::Unassigned).unwrap();
        let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
        PresetRegistry::bundled()
            .apply_to_document_base(&mut history, name)
            .unwrap();
        let definition = history
            .document()
            .pattern_definition_for(ChannelId(1))
            .unwrap();
        let mut plan = resolve_document_pattern_pipeline(history.document(), definition).unwrap();
        // Keep this affine witness inside the clip envelope. CoverCanvas recipes intentionally
        // extend beyond it, where rotation changes clipping and therefore segment correspondence.
        if let Some(parametric) = plan.family.parametric_curve.as_mut() {
            let ParametricCurve::Spiral(spiral) = &mut parametric.curve;
            spiral.turns = 3.0;
        }
        result.push((name, plan.family));
    }
    result
}

/// Verifies rotation and independent X/Y offsets move sites and paths across all non-random dispatches.
/// Stable source provenance must follow independent center-based trigonometry. Finite guard lines may
/// grow with coverage, but must retain their transformed supporting line and direction. Parametric
/// segments retain their transformed sample points. Separate weighted tests verify random reweighting.
///
/// # Panics
/// Panics on failed evaluation, missing shared geometry, ignored transforms or displaced geometry.
#[test]
fn gate3_dispatch_transforms_sites_and_structural_paths() {
    let canvas = CanvasSpec {
        width: 320.0,
        height: 240.0,
    };
    let neutral = GridInspectRequest {
        canvas: canvas.clone(),
        density: ResolvedDensityMetric2D {
            across_x: 12.0,
            across_y: 8.0,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 2,
        support_radius: 20.0,
        max_family_candidates: 100_000,
    };
    for (name, family) in families(&canvas) {
        let evaluate = |request: &GridInspectRequest| {
            evaluate_typed_family_product_cancellable(&family, request, &|| false)
                .unwrap_or_else(|error| panic!("{name}: {error}"))
        };
        let original = evaluate(&neutral);
        for (angle, dx, dy) in [
            (23.0_f64, 0.0, 0.0),
            (0.0, 7.25, 0.0),
            (0.0, 0.0, -4.5),
            (23.0, 7.25, -4.5),
        ] {
            let request = GridInspectRequest {
                rotation_degrees: angle,
                translation_x: dx,
                translation_y: dy,
                ..neutral.clone()
            };
            let moved = evaluate(&request);
            let (sin, cos) = angle.to_radians().sin_cos();
            let transform = |point: Point2| {
                Point2::new(
                    canvas.width / 2.0 + (point.x - canvas.width / 2.0) * cos
                        - (point.y - canvas.height / 2.0) * sin
                        + dx,
                    canvas.height / 2.0
                        + (point.x - canvas.width / 2.0) * sin
                        + (point.y - canvas.height / 2.0) * cos
                        + dy,
                )
            };
            let close = |actual: Point2, expected: Point2| {
                assert!(
                    (actual.x - expected.x).hypot(actual.y - expected.y) < 1e-6,
                    "{name} {angle}/{dx}/{dy}: {actual:?} != {expected:?}"
                );
            };
            let mut sites_checked = 0;
            for site in moved.site_set().sites() {
                if let Some(before) = original
                    .site_set()
                    .sites()
                    .iter()
                    .find(|before| same_site_source(&before.provenance, &site.provenance))
                {
                    close(site.position, transform(before.position));
                    sites_checked += 1;
                }
            }
            if !original.site_set().is_empty() {
                assert!(sites_checked > 3, "{name}: shared site witnesses");
            }
            let mut paths_checked = 0;
            if let Some(paths) = original.structural_path_set() {
                let moved_paths = moved
                    .structural_path_set()
                    .expect("paths survive transform");
                for before in paths.paths() {
                    let Some(after) = moved_paths
                        .paths()
                        .iter()
                        .find(|after| after.id == before.id)
                    else {
                        continue;
                    };
                    if family.parametric_curve.is_some() {
                        for (a, b) in before.path.segments().iter().zip(after.path.segments()) {
                            close(
                                b.point_at(0.5).unwrap(),
                                transform(a.point_at(0.5).unwrap()),
                            );
                        }
                    } else {
                        let expected_start = transform(before.path.start());
                        let expected_end = transform(before.path.end());
                        let vx = expected_end.x - expected_start.x;
                        let vy = expected_end.y - expected_start.y;
                        for point in [after.path.start(), after.path.end()] {
                            let distance = ((point.x - expected_start.x) * vy
                                - (point.y - expected_start.y) * vx)
                                .abs()
                                / vx.hypot(vy);
                            assert!(
                                distance < 1e-6,
                                "{name}: transformed guide distance {distance}"
                            );
                        }
                    }
                    paths_checked += 1;
                }
                assert!(
                    paths_checked > 0,
                    "{name}: shared structural path witnesses"
                );
            }
            assert!(
                sites_checked + paths_checked > 0,
                "{name}: nonempty geometry"
            );
        }
    }
}

/// Matches geometric source identities across coverage-dependent ordinal and parameter changes.
/// Family ordinals are local to each evaluated result, while guide indices and arc sequences
/// identify the same construction witness after a transform.
fn same_site_source(a: &FamilySiteProvenance, b: &FamilySiteProvenance) -> bool {
    match (a, b) {
        (
            FamilySiteProvenance::GuideIntersection { contributors: a },
            FamilySiteProvenance::GuideIntersection { contributors: b },
        ) => a == b,
        (
            FamilySiteProvenance::AlongGuide {
                guide_id: a,
                sequence: sa,
                ..
            },
            FamilySiteProvenance::AlongGuide {
                guide_id: b,
                sequence: sb,
                ..
            },
        ) => a == b && sa == sb,
        (
            FamilySiteProvenance::CurveGuideIntersection { contributors: a },
            FamilySiteProvenance::CurveGuideIntersection { contributors: b },
        ) => a.iter().map(|p| p.path).eq(b.iter().map(|p| p.path)),
        (
            FamilySiteProvenance::CurveAlongGuide {
                location: a,
                sequence: sa,
                ..
            },
            FamilySiteProvenance::CurveAlongGuide {
                location: b,
                sequence: sb,
                ..
            },
        )
        | (
            FamilySiteProvenance::AlongParametricCurve {
                location: a,
                sequence: sa,
                ..
            },
            FamilySiteProvenance::AlongParametricCurve {
                location: b,
                sequence: sb,
                ..
            },
        ) => a.path == b.path && sa == sb,
        _ => false,
    }
}
