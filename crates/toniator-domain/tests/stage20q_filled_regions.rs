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

/// Proves public preset validation owns typed treatment bounds rather than accepting a fallback.
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
    for response in [
        RegionGeometryResponse::Full {
            sampling: RegionSamplingStrategy::ReferencePoint,
        },
        RegionGeometryResponse::Scale {
            sampling: RegionSamplingStrategy::AreaAverage,
            minimum_scale: 0.0,
            maximum_scale: 2.0,
        },
        RegionGeometryResponse::ConstantGap {
            sampling: RegionSamplingStrategy::AreaAverage,
            minimum_gap: -3.0,
            maximum_gap: 4.0,
        },
    ] {
        validate_preset_record(&valid(response)).expect("finite ordered treatment validates");
    }
    let error = validate_preset_record(&valid(RegionGeometryResponse::Scale {
        sampling: RegionSamplingStrategy::ReferencePoint,
        minimum_scale: -0.1,
        maximum_scale: 1.0,
    }))
    .expect_err("Scale cannot become negative");
    assert_eq!(error.path(), "pattern.region.scale.range");
}
