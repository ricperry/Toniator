use toniator_domain::{
    CalibrationBinding, CalibrationKnot, CanvasSpec, ChannelId, DensityMetric2D, Document,
    DocumentCommand, DocumentHistory, DocumentSession, InvalidationLevel, PatternGeometryResponse,
    PatternStructureRecipe, PresetMetadata, PresetRecord, RegionSamplingStrategy, SavedCalibration,
    SourceReference, SourceWeightingFieldEdit,
};
use toniator_patterns::{LayeredPresetCatalog, PresetRegistry};

/// Proves every bundled preset validates with neutral structural response shells.
/// Region artwork sampling follows the canonical area-average preview default.
///
/// # Panics
/// Panics if a built-in recipe retains adjustable bounds or noncanonical sampling.
#[test]
fn bundled_patterns_have_only_neutral_response_shells() {
    let registry = PresetRegistry::bundled();
    assert!(!registry.entries().is_empty());
    for preset in registry.entries() {
        for output in &preset.recipe.output_settings {
            match &output.response {
                PatternGeometryResponse::Marks(value) => {
                    assert_eq!((value.minimum_fill, value.maximum_fill), (0.0, 1.0));
                }
                PatternGeometryResponse::Connected(value) => {
                    assert_eq!(
                        (value.minimum_thickness, value.maximum_thickness),
                        (0.0, 1.0)
                    );
                }
                PatternGeometryResponse::Regions(value) => {
                    assert_eq!((value.minimum_fill, value.maximum_fill), (0.0, 1.0));
                    assert_eq!(value.sampling, RegionSamplingStrategy::AreaAverage);
                }
            }
        }
    }
}

/// Keeps every bundled numeric companion attached to its exact canonical typed recipe binding.
///
/// Seed-bearing recipes use only current seed-neutral or bounded mean profiles; legacy v3 values
/// are admitted only after reconstruction as current v4 artifacts.
///
/// # Panics
/// Panics when a bundled recipe omits a companion, mismatches its canonical binding, or carries
/// a profile outside the current v4/v5 contract.
#[test]
fn bundled_companions_have_exact_current_bindings_and_profiles() {
    let registry = PresetRegistry::bundled();
    assert_eq!(registry.entries().len(), 16);
    for entry in registry.entries() {
        let saved = entry
            .saved_calibration
            .as_ref()
            .expect("each built-in carries a checked-in companion");
        toniator_domain::validate_preset_record(entry)
            .expect("the checked-in companion matches the canonical concrete recipe");
        assert!(
            saved.profile().ends_with("-v4") || saved.profile().ends_with("-v5"),
            "{} must use a current profile",
            entry.metadata.id
        );
        assert_eq!(
            saved.variation().is_some(),
            saved.profile().contains("mean-seed-neutral-v5"),
            "{} mean variation metadata agrees with its profile",
            entry.metadata.id
        );
    }
}

/// Counts enabled density-weighted Lloyd as a source-weighting family consumer.
///
/// # Panics
/// Panics if a weighting edit leaves weighted-Lloyd geometry cached as source-free.
#[test]
fn weighted_lloyd_alone_invalidates_family() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let mut recipe = PresetRegistry::bundled()
        .reconstruct("even-random-circles")
        .unwrap();
    if let PatternStructureRecipe::RandomSites { refinement, .. } = &mut recipe.structure {
        refinement.enabled = true;
        refinement.density_weighted = true;
        refinement.iterations = 2;
    } else {
        panic!("random fixture recipe changed");
    }
    let (document, _) = document
        .apply_command(
            &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base: document.pattern_settings().clone(),
                channel_id: ChannelId(1),
                base_definition: document
                    .pattern_definition_for(ChannelId(1))
                    .unwrap()
                    .clone(),
                recipe,
            },
        )
        .unwrap();
    assert!(toniator_domain::definition_uses_source_weighting(
        document.pattern_definition_for(ChannelId(1)).unwrap()
    ));
    let (_, result) = document
        .apply_command(&DocumentCommand::SetSourceWeightingField {
            channel_id: ChannelId(1),
            edit: SourceWeightingFieldEdit::Strength(0.6),
        })
        .unwrap();
    assert_eq!(result.invalidation, Some(InvalidationLevel::Family));
}

/// Confirms both catalog apply paths publish a matching Pattern companion in one undo step.
///
/// # Panics
/// Panics if the companion is dropped, copied before concrete binding, or separated from Undo.
#[test]
fn preset_companion_attaches_atomically() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let recipe = document
        .reconstruct_pattern_definition_recipe(document.pattern_settings().definition_id)
        .unwrap();
    let binding = CalibrationBinding::new(recipe.clone(), 1.0, 0.0).unwrap();
    let artifact = SavedCalibration::new(
        binding,
        vec![
            CalibrationKnot {
                input: 0.0,
                coverage: 0.0,
            },
            CalibrationKnot {
                input: 2.0,
                coverage: 1.0,
            },
        ],
        None,
    )
    .unwrap();
    let record = PresetRecord {
        metadata: PresetMetadata {
            id: "user-00000000-0000-4000-8000-000000000001".into(),
            name: "Saved Circle".into(),
            category: "Marks".into(),
            description: "Companion test".into(),
            thumbnail: None,
        },
        recipe,
        saved_calibration: Some(artifact.clone()),
    };
    let registry = PresetRegistry::new(1, vec![record.clone()]).unwrap();
    let layered = LayeredPresetCatalog::new(&PresetRegistry::bundled(), vec![record]).unwrap();
    for layered_path in [false, true] {
        let mut history = DocumentHistory::new(DocumentSession::new(document.clone()).unwrap());
        if layered_path {
            layered
                .apply_to_selected(
                    &mut history,
                    ChannelId(1),
                    "user-00000000-0000-4000-8000-000000000001",
                )
                .unwrap();
        } else {
            registry
                .apply_to_selected(
                    &mut history,
                    ChannelId(1),
                    "user-00000000-0000-4000-8000-000000000001",
                )
                .unwrap();
        }
        assert_eq!(
            history
                .document()
                .saved_calibration_for(ChannelId(1))
                .unwrap(),
            Some(&artifact)
        );
        history.undo().unwrap();
        assert_ne!(
            history
                .document()
                .saved_calibration_for(ChannelId(1))
                .unwrap(),
            Some(&artifact)
        );
    }
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    let aspect = history
        .document()
        .set_channel_density_for_effective(
            ChannelId(1),
            DensityMetric2D {
                density: 100.0,
                aspect: 2.0,
            },
        )
        .unwrap();
    history.apply(&aspect).unwrap();
    let before = history.document().clone();
    assert!(
        registry
            .apply_to_selected(
                &mut history,
                ChannelId(1),
                "user-00000000-0000-4000-8000-000000000001"
            )
            .is_err()
    );
    assert_eq!(
        history.document(),
        &before,
        "another compatible channel must not conceal the selected receiver's mismatch"
    );
}
