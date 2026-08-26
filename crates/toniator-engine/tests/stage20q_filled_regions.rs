//! Focused Stage 20Q engine policy and cache-regression integration coverage.

use toniator_engine::EvaluationLimits;
use toniator_sampling::RegionSamplingLimits;

/// Proves output-local sampling limits are typed engine policy and survive immutable copying.
#[test]
fn stage20q_engine_sampling_limits_are_explicit_cache_inputs() {
    let limits = RegionSamplingLimits {
        max_cell_intersections: 17,
        max_flattened_segments: 19,
        max_subdivision_depth: 23,
    };
    let configured = EvaluationLimits::default()
        .with_region_sampling_limits(limits)
        .expect("nonzero Region sampling limits configure");
    assert_eq!(configured.region_sampling_limits(), limits);
}

/// Connects the public policy boundary to the Stage 20Q source/mapping relevance, support-envelope,
/// broader-reuse, diagnostic replay, cancellation, failure, and stale-publication cache witnesses
/// in `toniator_engine::stage20q_region_cache_tests` and the engine test-support module.
#[test]
fn stage20q_engine_cache_atomicity_regression_anchor() {
    assert!(
        EvaluationLimits::default()
            .with_region_sampling_limits(RegionSamplingLimits {
                max_cell_intersections: 0,
                ..RegionSamplingLimits::default()
            })
            .is_err(),
        "disabled policy cannot publish a Region output candidate"
    );
}
