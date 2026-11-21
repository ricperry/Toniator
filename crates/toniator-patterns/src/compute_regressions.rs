//! Exact CPU controls for production compute-assist prerequisites.
use super::*;

/// Retains the original ordered linear-search basis as an independent numeric control.
/// # Errors
/// Returns the original missing/nonparallel contributor diagnostic.
fn reference_basis(
    guides: &[StraightGuide],
    coverage: &[GuideCoverage],
    contributors: &[GuideInstanceId],
) -> Result<NominalCellBasis, PatternPipelineError> {
    let mut best: Option<(f64, NominalCellBasis)> = None;
    for (left_index, left) in contributors.iter().enumerate() {
        for right in &contributors[left_index + 1..] {
            let Some(left_guide) = guides.iter().find(|guide| guide.id == *left) else {
                continue;
            };
            let Some(right_guide) = guides.iter().find(|guide| guide.id == *right) else {
                continue;
            };
            let Some(left_spacing) = coverage
                .iter()
                .find(|value| value.dimension_id == left.dimension_id)
                .map(|value| value.spacing)
            else {
                continue;
            };
            let Some(right_spacing) = coverage
                .iter()
                .find(|value| value.dimension_id == right.dimension_id)
                .map(|value| value.spacing)
            else {
                continue;
            };
            let cross = left_guide.tangent.x.mul_add(
                right_guide.tangent.y,
                -left_guide.tangent.y * right_guide.tangent.x,
            );
            if !cross.is_finite() || cross.abs() <= 1.0e-12 {
                continue;
            }
            let Ok(basis) = NominalCellBasis::new(
                left_guide.tangent.scale(right_spacing),
                right_guide.tangent.scale(left_spacing),
            ) else {
                continue;
            };
            let candidate = basis.diameter();
            if best.is_none_or(|(diameter, _)| candidate < diameter) {
                best = Some((candidate, basis));
            }
        }
    }
    best.map(|(_, basis)| basis).ok_or_else(|| {
        PatternPipelineError::new(
            "pattern.family.nominal_cell_basis",
            "guide intersection lacks a finite nonparallel contributor basis",
        )
    })
}

/// Proves indexing/caching preserves basis bits, contributor ties, duplicate IDs and nonuniform dimensions.
/// # Panics
/// Panics if the optimized lookup differs from its retained original control.
#[test]
fn indexed_basis_matches_original_ordered_pairs() {
    let mut guides = Vec::new();
    let mut coverage = Vec::new();
    for dimension in 1..=4_u64 {
        let angle = (dimension as f64 * 37.0).to_radians();
        let tangent = Vector2::new(angle.cos(), angle.sin());
        coverage.push(GuideCoverage {
            dimension_id: dimension,
            spacing: dimension as f64 * 0.7,
            normalized_phase: 0.0,
            first_index: -3,
            last_index: 3,
        });
        for index in -3..=3 {
            guides.push(StraightGuide {
                id: GuideInstanceId::new(GuideDimensionId(dimension), index),
                normal: tangent.perpendicular(),
                tangent,
                offset: index as f64,
                anchor: Point2::new(0.0, 0.0),
                start: Point2::new(0.0, 0.0),
                end: Point2::new(10.0, 10.0),
            });
        }
    }
    for variant in 0..3 {
        if variant == 1 {
            guides[0].tangent = Vector2::new(0.3, 0.8);
        }
        if variant == 2 {
            let mut duplicate = guides[0].clone();
            duplicate.tangent = Vector2::new(-0.7, 0.2);
            guides.push(duplicate);
        }
        let lookup = StraightBasisIndex::new(&guides, &coverage);
        for index in -3..=3 {
            for dimensions in [[1, 2, 3, 4], [4, 3, 2, 1], [1, 1, 2, 2], [0, 1, 2, 3]] {
                let ids = dimensions
                    .map(|dimension| GuideInstanceId::new(GuideDimensionId(dimension), index));
                let expected = reference_basis(&guides, &coverage, &ids);
                let actual = lookup.basis(&ids);
                assert_eq!(actual, expected);
            }
        }
    }
}

/// Preserves tangent and normal orientation bits through indexed first-occurrence lookup.
/// # Panics
/// Panics if rotated/translated family orientation differs from its original linear guide lookup.
#[test]
fn indexed_tangent_normal_orientation_matches_original_bits() {
    use toniator_domain::{CoveragePolicy, PatternDefinitionId};
    let definition = PatternDefinition::supported_straight_grid(
        PatternDefinitionId(7),
        "lookup-control",
        PatternMechanismId(11),
        PatternMechanismId(12),
        PatternOutputLayerId(13),
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    );
    for rotation in [0.0, 17.0, -39.5] {
        let family = evaluate_typed_family(
            &definition,
            &GridInspectRequest {
                canvas: CanvasSpec {
                    width: 90.0,
                    height: 60.0,
                },
                density: ResolvedDensityMetric2D {
                    across_x: 9.0,
                    across_y: 6.0,
                },
                rotation_degrees: rotation,
                translation_x: 3.25,
                translation_y: -4.5,
                guard_steps: 2,
                support_radius: 10.0,
                max_family_candidates: 100_000,
            },
        )
        .unwrap();
        let lookup = StraightBasisIndex::new(family.straight_guides(), &[]);
        for site in family.site_set().sites() {
            let FamilySiteProvenance::GuideIntersection { contributors } = &site.provenance else {
                panic!("intersection fixture")
            };
            for id in contributors {
                let guide = family
                    .straight_guides()
                    .iter()
                    .find(|guide| guide.id == *id)
                    .unwrap();
                for normal in [false, true] {
                    let orientation = if normal {
                        MarkOrientation::GuideNormal {
                            dimension_id: GuideDimensionId(id.dimension_id),
                        }
                    } else {
                        MarkOrientation::GuideTangent {
                            dimension_id: GuideDimensionId(id.dimension_id),
                        }
                    };
                    let expected = guide.tangent.y.atan2(guide.tangent.x).to_degrees()
                        + if normal { 90.0 } else { 0.0 }
                        + 30.0;
                    let actual =
                        site_orientation_degrees(&family, &lookup, site, &orientation, 30.0)
                            .unwrap();
                    assert_eq!(actual.to_bits(), expected.to_bits());
                }
            }
        }
    }
}
