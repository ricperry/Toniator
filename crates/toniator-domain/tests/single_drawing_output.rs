use toniator_domain::{
    CanvasSpec, Document, PatternDefinitionRecipe, PatternRecipeFamilyKind,
    PatternRecipeOutputKind, PresetMetadata, PresetRecord, validate_pattern_definition,
    validate_preset_record,
};

/// Wraps a recipe in current metadata without changing its drawing intent.
fn record(recipe: PatternDefinitionRecipe) -> PresetRecord {
    PresetRecord {
        metadata: PresetMetadata {
            id: "single-drawing-output".into(),
            name: "Single drawing output".into(),
            category: "test".into(),
            description: "one drawing per channel".into(),
            thumbnail: None,
        },
        recipe,
    }
}

/// Rejects empty and composite definitions before they enter documents or evaluation.
///
/// # Panics
/// Panics if one output is rejected or an empty/composite definition is accepted.
#[test]
fn definitions_require_exactly_one_drawing_output() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 120.0,
            height: 80.0,
        },
        Default::default(),
    )
    .unwrap();
    let definition = document.pattern_definition_bundles()[0].definition.clone();
    validate_pattern_definition(&definition).unwrap();
    let mut invalid = definition.clone();
    invalid.output_layers.clear();
    assert!(validate_pattern_definition(&invalid).is_err());
    invalid = definition;
    invalid.output_layers.push(invalid.output_layers[0].clone());
    let error = validate_pattern_definition(&invalid)
        .unwrap_err()
        .to_string();
    assert!(error.contains("exactly one drawing layer"), "{error}");
}

/// Preserves all family starters while refusing extra drawing responses and append operations.
///
/// # Panics
/// Panics if a starter fails, an extra output succeeds, or a rejected edit changes its recipe.
#[test]
fn recipes_and_authoring_keep_one_drawing_output() {
    for family in [
        PatternRecipeFamilyKind::Guides,
        PatternRecipeFamilyKind::Dispersion,
        PatternRecipeFamilyKind::Parametric,
    ] {
        let recipe = PatternDefinitionRecipe::starter_for_family(family);
        validate_preset_record(&record(recipe.clone())).unwrap();
        let before = recipe.clone();
        assert!(
            recipe
                .with_appended_output_kind(PatternRecipeOutputKind::Marks)
                .is_err()
        );
        assert_eq!(recipe, before);
        let mut invalid = recipe;
        invalid
            .output_settings
            .push(invalid.output_settings[0].clone());
        assert!(validate_preset_record(&record(invalid)).is_err());
    }
}
