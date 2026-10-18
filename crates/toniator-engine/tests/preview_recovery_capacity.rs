//! Focused production evaluator witnesses; no historical fixture/container compatibility.
#[allow(dead_code)]
#[path = "../examples/preview_recovery_probe.rs"]
mod probe;

use std::sync::atomic::AtomicBool;
use toniator_domain::DocumentSession;
use toniator_engine::*;

/// Builds one immutable production request from a fresh current source/family fixture.
///
/// # Panics
/// Panics if the current domain builder or immutable artwork cannot produce a valid request.
fn request(case: &str) -> EvaluationRequest {
    let (document, _, source) = probe::fixture("png", case).expect("current fixture");
    EvaluationRequest::new(
        DocumentSession::new(document)
            .expect("session")
            .document_evaluation_snapshot(),
        source,
    )
}

/// Proves cold/warm exact scene/RGBA equality for current Grid, dispersion, Curve and Region.
/// The separate release probe captures both source types at natural dimensions.
///
/// # Panics
/// Panics on failed canonical evaluation, changed cache output, or absent family geometry.
#[test]
fn representative_families_reuse_exact_canonical_scene_and_rgba() {
    for case in ["grid", "random", "round-crossing", "region"] {
        let mut cache = EvaluationProfileCache::default();
        let request = request(case);
        let cold = evaluate_profiled_cached_with_limits(
            request.clone(),
            EvaluationLimits::default(),
            &mut cache,
        )
        .expect("cold evaluation");
        let warm =
            evaluate_profiled_cached_with_limits(request, EvaluationLimits::default(), &mut cache)
                .expect("warm evaluation");
        assert_eq!(cold.result.scene(), warm.result.scene(), "{case} canonical");
        assert_eq!(cold.result.raster(), warm.result.raster(), "{case} RGBA");
        assert!(
            cold.result.scene().circular_mark_count() > 0,
            "{case} geometry"
        );
    }
}

/// Proves stricter finite profile/outline/raster policies reject even a previously cached scene.
/// Failure leaves the cache transaction usable by the original unrestricted request.
///
/// # Panics
/// Panics if finite policy is ignored, diagnostics move boundaries, or failure corrupts reuse.
#[test]
fn finite_policy_rechecks_warm_cache_without_publishing_partial_output() {
    let request = request("round-crossing");
    let mut cache = EvaluationProfileCache::default();
    let accepted = evaluate_profiled_cached_with_limits(
        request.clone(),
        EvaluationLimits::default(),
        &mut cache,
    )
    .expect("accepted scene");
    for (limits, path) in [
        (
            EvaluationLimits::default()
                .with_max_stroke_profile_samples(1)
                .expect("profile policy"),
            "realization.stroke.profile_limit",
        ),
        (
            EvaluationLimits::default()
                .with_max_stroke_outline_segments(1)
                .expect("outline policy"),
            "connection.stroke.outline_limit",
        ),
        (
            EvaluationLimits::default()
                .with_max_flattened_raster_edges(1)
                .expect("raster policy"),
            "raster.limits.flattened_edges",
        ),
    ] {
        let error = match evaluate_profiled_cached_with_limits(request.clone(), limits, &mut cache)
        {
            Ok(_) => panic!("finite policy must reject"),
            Err(error) => error,
        };
        assert_eq!(error.path(), path);
        let recovered = evaluate_profiled_cached_with_limits(
            request.clone(),
            EvaluationLimits::default(),
            &mut cache,
        )
        .expect("cache remains usable");
        assert_eq!(recovered.result.scene(), accepted.result.scene());
        assert_eq!(recovered.result.raster(), accepted.result.raster());
    }
}

/// Distinguishes cancellation from family-independent typed source/family failures.
/// No partial result is published, and a later ordinary request still succeeds.
///
/// # Panics
/// Panics if cancellation becomes an evaluation error or a finite family policy is ignored.
#[test]
fn cancelled_and_finite_family_requests_are_distinct_terminal_outcomes() {
    for case in ["grid", "random", "region"] {
        let request = request(case);
        assert!(matches!(
            evaluate_cancellable_with_limits(
                request.clone(),
                EvaluationLimits::default(),
                &AtomicBool::new(true)
            ),
            Err(EvaluationRunError::Cancelled)
        ));
        assert!(
            evaluate_with_limits(
                request,
                EvaluationLimits::new(1).expect("finite family policy")
            )
            .is_err()
        );
    }
}

/// Rechecks preclip construction under a stricter policy even when a warm result is mostly clipped away.
/// The policy belongs in family cache keys and does not alter canonical content or poison recovery.
/// # Panics
/// Panics if warm downstream caches bypass construction admission or unrestricted output changes.
#[test]
fn finite_curve_construction_policy_rechecks_largely_clipped_warm_cache() {
    let request = request("round-crossing");
    let mut cache = EvaluationProfileCache::default();
    let accepted = evaluate_profiled_cached_with_limits(
        request.clone(),
        EvaluationLimits::default(),
        &mut cache,
    )
    .expect("unrestricted result");
    let clipped = accepted.result.scene().layers()[0]
        .outputs()
        .iter()
        .filter_map(|output| match output.geometry() {
            GeometryOutput::CanonicalStrokes(strokes) => Some(strokes),
            _ => None,
        })
        .flatten()
        .map(|stroke| stroke.path.segments().len())
        .sum::<usize>();
    let limits =
        toniator_patterns::CurveWorkLimits::new(clipped + 1, usize::MAX, usize::MAX, usize::MAX)
            .unwrap();
    let error = match evaluate_profiled_cached_with_limits(
        request.clone(),
        EvaluationLimits::default().with_curve_work_limits(limits),
        &mut cache,
    ) {
        Ok(_) => panic!("preclip construction must reject beyond the finite count"),
        Err(error) => error,
    };
    assert_eq!(error.path(), "curve.parametric.segment_limit");
    let recovered =
        evaluate_profiled_cached_with_limits(request, EvaluationLimits::default(), &mut cache)
            .expect("unrestricted recovery");
    assert_eq!(recovered.result.scene(), accepted.result.scene());
    assert_eq!(recovered.result.raster(), accepted.result.raster());
}
