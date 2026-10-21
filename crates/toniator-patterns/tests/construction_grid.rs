//! Exact fixed-grid candidate pruning through the current greenfield production dispatch.
use toniator_domain::*;
use toniator_geometry::{Bounds, ConstructionContext, ConstructionMode, Point2};
use toniator_patterns::*;
/// Resolves a current fixed straight-grid recipe with full guard/support authority.
fn request(rotation: f64) -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec {
            width: 320.,
            height: 240.,
        },
        density: ResolvedDensityMetric2D {
            across_x: 160.,
            across_y: 120.,
        },
        rotation_degrees: rotation,
        translation_x: 3.25,
        translation_y: -4.5,
        guard_steps: 2,
        support_radius: 6.,
        max_family_candidates: usize::MAX,
    }
}
/// Locks full output equality, actual candidate-work savings and explicit full-range policy.
/// # Panics
/// Panics on changed sites, guides, IDs, fingerprint, ordering or resource admission.
#[test]
fn fixed_grid_slabs_preserve_complete_output_and_reduce_reached_positions() {
    for angle in [0., 17., 45., 89.5, 137., 270.] {
        let req = request(angle);
        let canvas = Bounds::new(Point2::new(0., 0.), Point2::new(320., 240.)).unwrap();
        let complete = ConstructionContext::new(ConstructionMode::Complete, Some(canvas), true);
        let pruned = ConstructionContext::new(ConstructionMode::Canvas, Some(canvas), true);
        let a = complete.run(|| evaluate_straight_grid(&req)).unwrap();
        let b = pruned.run(|| evaluate_straight_grid(&req)).unwrap();
        assert_eq!(a, b);
        let ac = complete.measurements.as_ref().unwrap().snapshot();
        let bc = pruned.measurements.as_ref().unwrap().snapshot();
        assert_eq!(ac.logical_grid_candidates, bc.logical_grid_candidates);
        if angle == 45. {
            assert!(
                bc.grid_positions + bc.grid_search_positions < ac.grid_positions,
                "{ac:?} {bc:?}"
            );
        }
        let mut bounded = req.clone();
        bounded.max_family_candidates = 10;
        assert_eq!(
            pruned
                .run(|| evaluate_straight_grid(&bounded))
                .unwrap_err()
                .path(),
            "coverage.candidate_limit"
        );
    }
}
/// Proves current typed StraightGuides dispatch reaches the optimized greenfield fixed-grid evaluator.
/// # Panics
/// Panics if a current recipe is routed only through an inspect-only or retired implementation.
#[test]
fn current_typed_fixed_grid_uses_candidate_pruning() {
    let definition = PatternDefinition::supported_straight_grid(
        PatternDefinitionId(1),
        "Current fixed grid",
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternOutputLayerId(4),
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.,
        },
    );
    let context = ConstructionContext::new(ConstructionMode::Canvas, None, true);
    let output = context
        .run(|| evaluate_typed_family(&definition, &request(45.)))
        .unwrap();
    assert!(!output.site_set().is_empty());
    let metrics = context.measurements.as_ref().unwrap().snapshot();
    assert!(metrics.grid_search_positions > 0);
    assert!(metrics.grid_positions < metrics.logical_grid_candidates);
}

/// Checks bounded inclusive-slab numeric controls and cooperative cancellation through production.
/// # Panics
/// Panics on any changed complete family, original finite rejection, or cancellation authority.
#[test]
fn fixed_grid_slabs_numeric_boundaries_and_cancellation_match_complete() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    for (width, height, angle, tx, ty, density) in [
        (257., 1., 0., 0., 0., 8.),
        (1., 257., 90., -0.0, 1e-9, 8.),
        (
            320.,
            240.,
            45.,
            6.0_f64.next_down(),
            -6.0_f64.next_up(),
            64.,
        ),
        (
            320.,
            240.,
            45.0_f64.next_up(),
            6.0_f64.next_up(),
            -6.0_f64.next_down(),
            64.,
        ),
        (320., 240., 0.00000001, 1e8, -1e8, 32.),
        (320., 240., 90., 1e16, -1e16, 8.),
    ] {
        let mut req = request(angle);
        req.canvas = CanvasSpec { width, height };
        req.translation_x = tx;
        req.translation_y = ty;
        req.density = ResolvedDensityMetric2D {
            across_x: density,
            across_y: density,
        };
        let a = ConstructionContext::new(ConstructionMode::Complete, None, false)
            .run(|| evaluate_straight_grid(&req));
        let b = ConstructionContext::new(ConstructionMode::Canvas, None, false)
            .run(|| evaluate_straight_grid(&req));
        assert_eq!(a, b, "{req:?}");
    }
    let definition = PatternDefinition::supported_straight_grid(
        PatternDefinitionId(1),
        "cancelled fixed grid",
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternOutputLayerId(4),
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.,
        },
    );
    let polls = AtomicUsize::new(0);
    let error = ConstructionContext::new(ConstructionMode::Canvas, None, false)
        .run(|| {
            evaluate_typed_family_cancellable(&definition, &request(45.), &|| {
                polls.fetch_add(1, Ordering::Relaxed) > 20
            })
        })
        .unwrap_err();
    assert_eq!(error.path(), "evaluation.cancelled");
}

/// Keeps direct multi-stroke aggregate outline admission independent of physically omitted commands.
/// # Panics
/// Panics if canvas pruning weakens a cumulative caller bound or changes full logical charge.
#[test]
fn pruned_multi_stroke_producer_charges_complete_aggregate_outline_commands() {
    let canvas = CanvasSpec {
        width: 64.,
        height: 48.,
    };
    let definition = PatternDefinition::parametric_curve(
        PatternDefinitionId(1),
        "aggregate outline",
        PatternMechanismId(1),
        None,
        PatternOutputLayerId(1),
        ParametricCurve::Spiral(SpiralCurve {
            shape: SpiralShape::Round,
            turns: 5.375,
            radial_spacing: 10.,
            phase_degrees: 13.,
            winding: CurveWinding::Clockwise,
        }),
        GuideRepetition::TransformStack {
            direction_degrees: 90.,
            spacing_multiplier: 4.,
        },
        None,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.,
        },
    );
    let plan = resolve_pattern_pipeline(&definition).unwrap();
    let family = evaluate_typed_family(
        &definition,
        &GridInspectRequest {
            canvas: canvas.clone(),
            density: ResolvedDensityMetric2D {
                across_x: 8.,
                across_y: 6.,
            },
            rotation_degrees: 0.,
            translation_x: 0.,
            translation_y: 0.,
            guard_steps: 1,
            support_radius: 2.,
            max_family_candidates: 100_000,
        },
    )
    .unwrap();
    let source=toniator_sampling::decode_source(br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="black"/></svg>"#,toniator_sampling::SourceFormatHint::Svg).unwrap();
    let run = |mode, limit| {
        ConstructionContext::new(
            mode,
            Some(Bounds::new(Point2::new(0., 0.), Point2::new(64., 48.)).unwrap()),
            false,
        )
        .run(|| {
            realize_typed_canonical_strokes_with_response_cancellable(
                &family,
                &plan,
                &source,
                &canvas,
                SourceMapping::canonical(SourceMappingComponent::Alpha),
                StrokeResponse {
                    minimum_thickness: 0.4,
                    maximum_thickness: 0.8,
                    bias: 0.,
                },
                1.,
                100_000,
                limit,
                GeometryResponseInput::RawProbe(0.7),
                &|| false,
            )
        })
    };
    let full = run(ConstructionMode::Complete, usize::MAX).unwrap().output;
    let pruned = run(ConstructionMode::Canvas, usize::MAX).unwrap().output;
    assert!(full.strokes.len() > 1);
    let total = full
        .strokes
        .iter()
        .map(|s| s.outline.logical_segment_count().unwrap())
        .sum::<usize>();
    let charged = pruned
        .strokes
        .iter()
        .map(|s| s.outline.logical_segment_count().unwrap())
        .sum::<usize>();
    let physical = pruned
        .strokes
        .iter()
        .flat_map(|s| &s.outline.contours)
        .map(|c| c.segments.len())
        .sum::<usize>();
    assert_eq!(total, charged);
    assert!(physical < total);
    let a = run(ConstructionMode::Complete, total - 1).unwrap_err();
    let b = run(ConstructionMode::Canvas, total - 1).unwrap_err();
    assert_eq!(a.path(), b.path());
    assert!(a.path().contains("outline_limit"));
}
