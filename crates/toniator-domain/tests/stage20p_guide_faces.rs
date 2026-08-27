//! Public Stage 20P recipe and source-authority integration witnesses.

use toniator_domain::{
    CoveragePolicy, GeneralizedSiteProductDraft, GuideDimensionDraft, MarkOrientationDraft,
    PatternDefinitionRecipe, PatternGeometryResponse, PatternOutputRealizationRecipe,
    PatternOutputSettingsRecipe, PatternStructureRecipe, PresetMetadata, PresetRecord,
    RegionGeometryResponse, SiteUseFilterRecipe, validate_preset_record,
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

/// Proves Guide Faces is a complete validated default-response recipe, not evaluator-side intent.
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
    assert_eq!(
        recipe.output_settings[0].response,
        PatternGeometryResponse::Regions(RegionGeometryResponse::default())
    );
    let invalid = PatternDefinitionRecipe::guide_faces(three_guide_structure(), vec![1, 0]);
    assert!(
        validate_preset_record(&PresetRecord {
            metadata: record.metadata,
            recipe: invalid
        })
        .is_err()
    );
}

/// Rejects duplicate and reversed Guide Faces selections inside ordered-output recipes.
#[test]
fn ordered_guide_face_output_requires_unique_increasing_dimensions() {
    let recipe = |dimension_indices| PatternDefinitionRecipe {
        structure: PatternStructureRecipe::OrderedOutputs {
            definition: Box::new(three_guide_structure()),
            outputs: vec![PatternOutputRealizationRecipe::GuideFaceRegions { dimension_indices }],
        },
        output_settings: vec![PatternOutputSettingsRecipe {
            source_filter: SiteUseFilterRecipe::All,
            response: PatternGeometryResponse::Regions(RegionGeometryResponse::default()),
        }],
    };
    let record = |recipe| PresetRecord {
        metadata: PresetMetadata {
            id: "ordered-guide-faces".into(),
            name: "Ordered guide faces".into(),
            category: "test".into(),
            description: "ordered Guide Faces validation fixture".into(),
            thumbnail: None,
        },
        recipe,
    };
    validate_preset_record(&record(recipe(vec![0, 1, 2])))
        .expect("unique increasing indices validate");
    assert_eq!(
        validate_preset_record(&record(recipe(vec![0, 1, 1])))
            .expect_err("duplicate indices reject")
            .path(),
        "preset.recipe.outputs.guide_faces.dimension_indices"
    );
    assert_eq!(
        validate_preset_record(&record(recipe(vec![2, 1, 0])))
            .expect_err("reversed indices reject")
            .path(),
        "preset.recipe.outputs.guide_faces.dimension_indices"
    );
}
