//! Current typed family witnesses for the single-path/no-along-sites capacity pilot.
use toniator_domain::*;
use toniator_patterns::*;

/// Builds current fixed-turn raw path intent without arc-length, offset or site consumers.
fn definition(shape: SpiralShape, turns: f64) -> PatternDefinition {
    PatternDefinition::parametric_curve(
        PatternDefinitionId(901),
        "Capacity single path",
        PatternMechanismId(902),
        None,
        PatternOutputLayerId(903),
        ParametricCurve::Spiral(SpiralCurve {
            shape,
            turns,
            radial_spacing: 16.0,
            phase_degrees: 37.0,
            winding: CurveWinding::Clockwise,
        }),
        CurveRepetition::Single,
        None,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    )
}

/// Requests the default 1024-square feature scale with explicit finite caller storage policy.
fn request() -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        density: ResolvedDensityMetric2D {
            across_x: 100.0,
            across_y: 100.0,
        },
        rotation_degrees: 13.0,
        translation_x: 7.0,
        translation_y: -9.0,
        guard_steps: 1,
        support_radius: 2.0,
        max_family_candidates: usize::MAX,
    }
}

/// Locks deterministic canonical fragments/identity through placement, baseline and channel transforms.
///
/// # Panics
/// Panics on changed source ordering, exact joins or non-deterministic family fingerprint/geometry.
#[test]
fn transformed_clipped_small_paths_repeat_exactly() {
    for shape in [SpiralShape::Round, SpiralShape::Square] {
        let definition = definition(shape, 5.375);
        let first = evaluate_typed_family(&definition, &request()).expect("small family");
        let repeat = evaluate_typed_family(&definition, &request()).expect("repeat family");
        assert_eq!(first, repeat);
        let paths = first.structural_path_set().expect("raw canonical paths");
        assert!(!paths.paths().is_empty());
        for path in paths.paths() {
            assert!(
                path.path
                    .segments()
                    .windows(2)
                    .all(|pair| pair[0].end() == pair[1].start())
            );
        }
    }
}

/// Publishes large single-path families in deterministic ordered fragments after canvas crossings.
/// # Panics
/// Panics if larger intent loses canonical fragments or fails independent finite-policy admission.
#[test]
fn large_family_publishes_ordered_clipped_paths_without_creative_cap() {
    for (shape, turns) in [
        (SpiralShape::Round, 223.25),
        (SpiralShape::Square, 1024.375),
    ] {
        let plan = resolve_pattern_pipeline(&definition(shape, turns)).expect("plan");
        let first = evaluate_typed_family_product_with_curve_limits_progress_cancellable(
            &plan.family,
            &request(),
            None,
            CurveWorkLimits::default(),
            &|| false,
            &|_, _| {},
        )
        .expect("large family");
        let repeat = evaluate_typed_family_product_with_curve_limits_progress_cancellable(
            &plan.family,
            &request(),
            None,
            CurveWorkLimits::default(),
            &|| false,
            &|_, _| {},
        )
        .expect("repeat");
        assert_eq!(first, repeat);
        assert!(!first.structural_path_set().unwrap().paths().is_empty());
        let error = evaluate_typed_family_product_with_curve_limits_progress_cancellable(
            &plan.family,
            &request(),
            None,
            CurveWorkLimits::new(4096, usize::MAX, usize::MAX, usize::MAX).unwrap(),
            &|| false,
            &|_, _| {},
        )
        .unwrap_err();
        assert_eq!(error.path(), "curve.parametric.segment_limit");
    }
}

/// Cancels centered placement and downstream family publication after construction completes.
/// # Panics
/// Panics if post-construction cancellation is ignored or publishes a partial family.
#[test]
fn controlled_family_cancellation_after_construction_is_atomic() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let plan = resolve_pattern_pipeline(&definition(SpiralShape::Round, 223.25)).unwrap();
    let stop = AtomicBool::new(false);
    let error = evaluate_typed_family_product_with_curve_limits_progress_cancellable(
        &plan.family,
        &request(),
        None,
        CurveWorkLimits::default(),
        &|| stop.load(Ordering::Relaxed),
        &|completed, _| {
            if completed >= 50 {
                stop.store(true, Ordering::Relaxed);
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.path(), "evaluation.cancelled");
}
