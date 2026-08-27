//! Focused Stage 20Q public region-authority integration coverage.
//!
//! The existing focused Stage 20O test target owns the detailed stale-bundle, delta-remap,
//! descriptor, shared-edit, invalidation, and exact undo/redo fault cases because its fixtures
//! construct the required three-channel shared definitions without exposing test-only APIs.

use toniator_domain::{
    CoveragePolicy, PatternDefinitionDraft, PatternDefinitionRecipe, PatternGeometryResponse,
    PatternStructureRecipe, PresetMetadata, PresetRecord, RegionGeometryResponse,
    RegionSamplingStrategy, validate_preset_record,
};

/// Proves public preset validation owns finite ordered 0.0..=2.0 fill bounds for both algorithms.
#[test]
fn stage20q_typed_responses_validate_only_their_compatible_numeric_contract() {
    let valid = |response| PresetRecord {
        metadata: PresetMetadata {
            id: "stage20q-domain".into(),
            name: "Stage 20Q domain".into(),
            category: "test".into(),
            description: "typed region response".into(),
            thumbnail: None,
        },
        recipe: {
            let mut recipe = PatternDefinitionRecipe::regions(
                PatternStructureRecipe::StraightGrid(PatternDefinitionDraft {
                    name: "regions".into(),
                    coverage: CoveragePolicy {
                        guard_steps: 1,
                        additional_margin: 0.0,
                    },
                }),
            );
            recipe.output_settings[0].response = PatternGeometryResponse::Regions(response);
            recipe
        },
    };
    for algorithm in [
        toniator_domain::RegionResizeAlgorithm::Scale,
        toniator_domain::RegionResizeAlgorithm::UniformOffset,
    ] {
        validate_preset_record(&valid(RegionGeometryResponse {
            algorithm,
            sampling: RegionSamplingStrategy::AreaAverage,
            minimum_fill: 0.0,
            maximum_fill: 2.0,
        }))
        .expect("finite ordered fill response validates");
    }
    let error = validate_preset_record(&valid(RegionGeometryResponse {
        algorithm: toniator_domain::RegionResizeAlgorithm::Scale,
        sampling: RegionSamplingStrategy::ReferencePoint,
        minimum_fill: -0.1,
        maximum_fill: 1.0,
    }))
    .expect_err("fill cannot become negative");
    assert_eq!(error.path(), "pattern.region.fill.range");
}
