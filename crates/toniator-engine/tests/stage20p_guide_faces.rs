//! Stage 20P engine limit authority integration witness.

use toniator_engine::EvaluationLimits;
use toniator_patterns::GuideFaceLimits;

/// Proves guide-face limits are evaluator-owned, nonzero, and retained in the request policy.
#[test]
fn guide_face_limits_are_typed_engine_authority() {
    let defaults = EvaluationLimits::new(1).expect("evaluation limits");
    assert_eq!(defaults.guide_face_limits(), GuideFaceLimits::default());
    let invalid = GuideFaceLimits {
        max_faces: 0,
        ..GuideFaceLimits::default()
    };
    assert_eq!(
        defaults
            .with_guide_face_limits(invalid)
            .expect_err("zero limit rejects")
            .path(),
        "region.guide_faces.limits.zero"
    );
}
