//! Public Stage 20P recipe and source-authority integration witnesses.

use toniator_domain::{
    CoveragePolicy, GeneralizedSiteProductDraft, GuideDimensionDraft, MarkOrientationDraft,
    PatternDefinitionRecipe, PatternGeometryResponse, PatternStructureRecipe, PresetMetadata,
    PresetRecord, RegionGeometryResponse, validate_preset_record,
};

/// Builds an ID-free phase-aligned three-guide recipe eligible for arrangement-face regions.
fn three_guide_structure() -> PatternStructureRecipe {
    PatternStructureRecipe::GeneralizedStraightGuides {
        name: "stage20p triangular guides".into(),
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
        dimensions: vec![
            GuideDimensionDraft {
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
            GuideDimensionDraft {
                baseline_angle_degrees: 60.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
            GuideDimensionDraft {
                baseline_angle_degrees: 120.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
        ],
        product: GeneralizedSiteProductDraft::Intersections {
            dimension_indices: vec![0, 1],
            merge_epsilon: 0.0,
        },
        orientation: MarkOrientationDraft::Fixed,
    }
}

/// Proves Guide Faces is a complete validated fixed-Full region recipe, not an evaluator-side intent.
#[test]
fn guide_face_recipe_requires_ordered_straight_dimensions() {
    let recipe = PatternDefinitionRecipe::guide_faces(three_guide_structure(), vec![0, 1, 2]);
    let record = PresetRecord {
        metadata: PresetMetadata {
            id: "stage20p-guide-faces".into(),
            name: "Guide faces".into(),
            category: "test".into(),
            description: "test recipe".into(),
            thumbnail: None,
        },
        recipe: recipe.clone(),
    };
    validate_preset_record(&record).expect("valid Guide Faces recipe");
    assert!(matches!(
        recipe.output_settings[0].response,
        PatternGeometryResponse::Regions(RegionGeometryResponse::Full { .. })
    ));
    let invalid = PatternDefinitionRecipe::guide_faces(three_guide_structure(), vec![1, 0]);
    assert!(
        validate_preset_record(&PresetRecord {
            metadata: record.metadata,
            recipe: invalid
        })
        .is_err()
    );
}
