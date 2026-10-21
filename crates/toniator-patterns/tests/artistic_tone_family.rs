use std::cell::RefCell;
use toniator_domain::*;
use toniator_patterns::*;
use toniator_sampling::{SourceFormatHint, decode_source};

/// Builds a current finite spiral output whose tone intent is independent of mark cadence.
fn definition(shape: SpiralShape, multiplier: f64) -> PatternDefinition {
    let mut definition = PatternDefinition::parametric_curve(
        PatternDefinitionId(1),
        "Tone family",
        PatternMechanismId(1),
        None,
        PatternOutputLayerId(1),
        ParametricCurve::Spiral(SpiralCurve {
            shape,
            turns: 2.0,
            radial_spacing: 4.0,
            phase_degrees: 13.0,
            winding: CurveWinding::CounterClockwise,
        }),
        GuideRepetition::Single,
        None,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    let PatternOutputRealization::ParametricPaths {
        tone_sample_spacing,
        ..
    } = &mut definition.output_layers[0].realization
    else {
        unreachable!()
    };
    *tone_sample_spacing = ToneSampleSpacing { multiplier };
    definition
}

/// Evaluates one current family with a true Feature metric on either canvas aspect.
/// # Panics
/// Panics if finite positive density or the current family producer fails.
fn family(
    definition: &PatternDefinition,
    canvas: CanvasSpec,
    feature: f64,
    support: f64,
) -> (TypedFamilyOutput, PatternPipelinePlan) {
    let density = DensityMetric2D::default_for_canvas(&canvas).unwrap();
    let density = DensityMetric2D {
        density: density.density / feature,
        aspect: 3.0,
    }
    .resolve(&canvas)
    .unwrap();
    let plan = resolve_pattern_pipeline(definition).unwrap();
    let output = evaluate_typed_family_product_cancellable(
        &plan.family,
        &GridInspectRequest {
            canvas,
            density,
            rotation_degrees: 0.0,
            translation_x: 0.0,
            translation_y: 0.0,
            guard_steps: 1,
            support_radius: support,
            max_family_candidates: 100_000,
        },
        &|| false,
    )
    .unwrap();
    (output, plan)
}

/// Proves the production lattice tracks Feature and multiplier proportionally for both spiral shapes.
/// # Panics
/// Panics if actual prepared knot cadence differs from the requested world-space interval.
#[test]
fn actual_spiral_lattice_is_feature_relative_on_square_and_rectangular_canvases() {
    for canvas in [
        CanvasSpec {
            width: 120.0,
            height: 120.0,
        },
        CanvasSpec {
            width: 120.0,
            height: 80.0,
        },
    ] {
        for shape in [SpiralShape::Round, SpiralShape::Square] {
            for feature in [0.5, 1.0, 2.0] {
                for multiplier in [0.5, 1.0, 2.0] {
                    let definition = definition(shape, multiplier);
                    let (family, _) = family(&definition, canvas.clone(), feature, 1.0);
                    let interval = family
                        .tone_sample_interval(ToneSampleSpacing { multiplier })
                        .unwrap();
                    assert!((interval - 1.2 * feature * multiplier).abs() < 1e-12);
                    let logical = &family.logical_tone_paths()[0];
                    let calls = RefCell::new(Vec::new());
                    let tone = prepare_tone_profile_cancellable(
                        &logical.instance.path,
                        logical.domain,
                        interval,
                        logical.nominal_basis,
                        100_000,
                        &|point| {
                            calls.borrow_mut().push(point);
                            Ok(0.6)
                        },
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(calls.borrow().len(), tone.knots.len());
                    for (ordinal, knot) in tone
                        .knots
                        .iter()
                        .enumerate()
                        .skip(1)
                        .take(tone.knots.len().saturating_sub(2))
                    {
                        assert!((knot.distance - ordinal as f64 * interval).abs() < 1e-9);
                    }
                }
            }
        }
    }
}

/// Proves actual canonical source sampling ignores decoded resolution while retaining smooth shape.
/// # Panics
/// Panics if equal uniform artwork at different pixel resolutions changes canonical profiles.
#[test]
fn uniform_source_resolution_does_not_change_canonical_curve_profiles() {
    let canvas = CanvasSpec {
        width: 120.0,
        height: 80.0,
    };
    let definition = definition(SpiralShape::Round, 2.0);
    let (family, plan) = family(&definition, canvas.clone(), 1.0, 1.0);
    let run = |size: u32| {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}"><rect width="{size}" height="{size}" fill="black"/></svg>"#
        );
        let source = decode_source(svg.as_bytes(), SourceFormatHint::Svg).unwrap();
        realize_typed_canonical_strokes_with_response_cancellable(
            &family,
            &plan,
            &source,
            &canvas,
            SourceMapping::canonical(SourceMappingComponent::Alpha),
            StrokeResponse {
                minimum_thickness: 0.1,
                maximum_thickness: 0.8,
                bias: 0.0,
            },
            1.0,
            100_000,
            1_000_000,
            GeometryResponseInput::Identity,
            &|| false,
        )
        .unwrap()
        .output
    };
    let first = run(16);
    let second = run(512);
    assert!(!first.strokes.is_empty());
    assert_eq!(first.strokes, second.strokes);
}

/// Proves changed finite presentation envelopes cannot restart a straight guide's authored lattice.
/// # Panics
/// Panics if guard/support clipping changes source anchors retained inside the same canvas.
#[test]
fn presentation_envelope_keeps_the_full_guide_lattice_origin() {
    let mut definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(1),
        "Anchored straight guide",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
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
    definition.output_layers[0].realization = PatternOutputRealization::GuidePaths {
        tone_sample_spacing: ToneSampleSpacing::default(),
        guide_mechanism_id: PatternMechanismId(1),
        style: PathStrokeStyle::default(),
    };
    let canvas = CanvasSpec {
        width: 120.0,
        height: 80.0,
    };
    let run = |support| {
        let (family, _) = family(&definition, canvas.clone(), 1.0, support);
        let logical = family
            .logical_tone_paths()
            .iter()
            .find(|path| path.instance.id.repetition_index == 0)
            .unwrap();
        let calls = RefCell::new(Vec::new());
        prepare_tone_profile_cancellable(
            &logical.instance.path,
            logical.domain,
            family
                .tone_sample_interval(ToneSampleSpacing::default())
                .unwrap(),
            logical.nominal_basis,
            100_000,
            &|point| {
                if point.x >= 0.0 && point.x <= 120.0 && point.y >= 0.0 && point.y <= 80.0 {
                    calls.borrow_mut().push(point);
                }
                Ok(1.0)
            },
            &|| false,
        )
        .unwrap();
        calls.into_inner()
    };
    let first = run(1.0);
    let second = run(20.0);
    assert!(!first.is_empty());
    assert_eq!(first.len(), second.len());
    for (a, b) in first.into_iter().zip(second) {
        assert!((a.x - b.x).hypot(a.y - b.y) < 1e-8);
    }
}
