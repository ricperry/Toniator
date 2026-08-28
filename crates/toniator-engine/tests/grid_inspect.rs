use toniator_domain::{CanvasSpec, ResolvedDensityMetric2D};
use toniator_engine::{GridInspectRequest, inspect_straight_grid};

#[test]
fn inspect_orchestration_preserves_family_output_without_document_mutation() {
    let output = inspect_straight_grid(&GridInspectRequest {
        canvas: CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        density: ResolvedDensityMetric2D {
            across_x: 90.0,
            across_y: 60.0,
        },
        rotation_degrees: 17.0,
        translation_x: 3.25,
        translation_y: -4.5,
        guard_steps: 2,
        support_radius: 4.5,
        max_family_candidates: 1_048_576,
    })
    .expect("valid grid");

    assert!(!output.guides.is_empty());
    assert!(!output.sites.is_empty());
    assert!(output.has_only_finite_geometry());
}
