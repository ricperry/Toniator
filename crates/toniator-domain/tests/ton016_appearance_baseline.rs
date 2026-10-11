use toniator_domain::{
    CanvasSpec, ChannelTopologyTemplate, Document, DocumentCommand, DocumentHistory,
    DocumentSession, HalftoneChannelModel, PatternDefinitionRecipe, PatternStructureRecipe,
    PropertyFieldId, PropertyTarget, SourceReference,
};

const LAST_ALL: [(PropertyFieldId, f64); 5] = [
    (PropertyFieldId::Density, 37.0),
    (PropertyFieldId::DensityAspect, 1.6),
    (PropertyFieldId::RotationDegrees, 23.0),
    (PropertyFieldId::TranslationX, 0.17),
    (PropertyFieldId::TranslationY, -0.21),
];

/// Creates one ordinary RGB document history with no stored last-ALL intent.
///
/// # Panics
/// Panics if the current default document or its initial session fails validation.
fn history() -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 80.0,
            height: 60.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document validates");
    DocumentHistory::new(DocumentSession::new(document).expect("initial session validates"))
}

/// Publishes one ALL Start assignment through the source-aware configuration history boundary.
///
/// # Panics
/// Panics if the batch cannot validate or publish as one exact history transition.
fn assign_all(history: &mut DocumentHistory, edits: &[(PropertyFieldId, f64)]) {
    let base = history.document().clone();
    let configuration = base
        .edit_all_channel_start_configuration(edits)
        .expect("ALL Start assignments validate");
    history
        .apply_document_configuration(&base, history.revision(), &configuration)
        .expect("ALL Start configuration publishes");
}

/// Compares a replacement topology's effective scalar values to the stored shared intent.
///
/// # Panics
/// Panics if a channel lacks an effective Pattern or differs beyond the small arithmetic tolerance.
fn assert_effective_baseline(document: &Document, baseline: &[(PropertyFieldId, f64)]) {
    let channels = document
        .channel_topology()
        .expect("replacement model has a topology")
        .channels();
    for channel in channels {
        let effective = document
            .effective_channel_pattern(channel.id)
            .expect("replacement channel resolves");
        for &(field, expected) in baseline {
            let actual = match field {
                PropertyFieldId::Density => effective.density.density,
                PropertyFieldId::DensityAspect => effective.density.aspect,
                PropertyFieldId::RotationDegrees => effective.pattern_rotation_degrees,
                PropertyFieldId::TranslationX => effective.translation_x,
                PropertyFieldId::TranslationY => effective.translation_y,
                _ => panic!("unexpected baseline field {field:?}"),
            };
            assert!(
                (actual - expected).abs() <= 1.0e-12,
                "channel {:?} {field:?}: {actual} != {expected}",
                channel.id
            );
        }
    }
}

/// Persists explicit ALL values through named deviations and RGB/CMYK/alpha model replacements.
///
/// # Panics
/// Panics if one property assignment changes other baseline fields, a model switch loses the
/// baseline, or exact named/model-switch Undo and Redo do not restore document authority.
#[test]
fn ton016_all_baseline_survives_named_edits_and_model_replacements() {
    let mut history = history();
    let initial_document = history.document().clone();
    assign_all(&mut history, &LAST_ALL);
    let initial_all = history.document().clone();
    let baseline = *history.document().appearance_baseline();
    history.undo().unwrap().expect("ALL assignment is undoable");
    assert_eq!(history.document(), &initial_document);
    history.redo().unwrap().expect("ALL assignment is redoable");
    assert_eq!(history.document(), &initial_all);

    assign_all(&mut history, &[(PropertyFieldId::Density, 41.0)]);
    let one_field_baseline = *history.document().appearance_baseline();
    assert_eq!(one_field_baseline.density, Some(41.0));
    assert_eq!(one_field_baseline.density_aspect, baseline.density_aspect);
    assert_eq!(
        one_field_baseline.pattern_rotation_degrees,
        baseline.pattern_rotation_degrees
    );
    assert_eq!(one_field_baseline.translation_x, baseline.translation_x);
    assert_eq!(one_field_baseline.translation_y, baseline.translation_y);

    let named_channel = history
        .document()
        .channel_topology()
        .expect("initial model has topology")
        .channels()[0]
        .id;
    let before_named = history.document().clone();
    let named_configuration = before_named
        .edit_channel_start_configuration(&[
            (
                PropertyTarget::Channel(named_channel),
                PropertyFieldId::Density,
                49.0,
            ),
            (
                PropertyTarget::Channel(named_channel),
                PropertyFieldId::TranslationX,
                0.72,
            ),
        ])
        .expect("named Start assignments validate");
    history
        .apply_document_configuration(&before_named, history.revision(), &named_configuration)
        .expect("named Start configuration publishes");
    let named_document = history.document().clone();
    assert_eq!(
        history.document().appearance_baseline(),
        &one_field_baseline
    );
    assert_eq!(
        history
            .document()
            .effective_channel_pattern(named_channel)
            .unwrap()
            .density
            .density,
        49.0
    );
    history.undo().unwrap().expect("named edit is undoable");
    assert_eq!(history.document(), &before_named);
    history.redo().unwrap().expect("named edit is redoable");
    assert_eq!(history.document(), &named_document);

    for model in [
        HalftoneChannelModel::Cmyk,
        HalftoneChannelModel::SourceColorAlpha,
        HalftoneChannelModel::Rgb,
    ] {
        let before_model = history.document().clone();
        let current = history.document();
        let topology = current
            .canonical_channel_topology_with_appearance_baseline(
                model,
                ChannelTopologyTemplate::document_base(),
            )
            .expect("baseline-seeded topology validates");
        history
            .apply(&DocumentCommand::ReplaceChannelTopology { model, topology })
            .expect("model replacement publishes");
        assert_eq!(
            history.document().appearance_baseline(),
            &one_field_baseline
        );
        assert_effective_baseline(
            history.document(),
            &[
                (PropertyFieldId::Density, 41.0),
                (PropertyFieldId::DensityAspect, 1.6),
                (PropertyFieldId::RotationDegrees, 23.0),
                (PropertyFieldId::TranslationX, 0.17),
                (PropertyFieldId::TranslationY, -0.21),
            ],
        );
        let after_model = history.document().clone();
        history.undo().unwrap().expect("model switch is undoable");
        assert_eq!(history.document(), &before_model);
        history.redo().unwrap().expect("model switch is redoable");
        assert_eq!(history.document(), &after_model);
    }
    let after_models = history.document().clone();
    let end_command = history
        .document()
        .edit_all_channel_end_command(&[(PropertyFieldId::Density, 43.0)])
        .expect("ALL End assignment validates");
    history
        .apply_temporal(&end_command)
        .expect("ALL End assignment publishes");
    assert_eq!(
        history.document().appearance_baseline(),
        &one_field_baseline
    );
    let with_end = history.document().clone();
    history.undo().unwrap().expect("End assignment is undoable");
    assert_eq!(history.document(), &after_models);
    history.redo().unwrap().expect("End assignment is redoable");
    assert_eq!(history.document(), &with_end);
    history.undo().unwrap().expect("model switch is undoable");
    history.redo().unwrap().expect("model switch is redoable");
    assert_eq!(history.document(), &with_end);

    // The first ALL transition is also one exact history entry from an unset baseline.
    while history.can_undo() {
        history.undo().unwrap();
    }
    assert_eq!(history.document(), &initial_document);
    assign_all(&mut history, &LAST_ALL);
    assert_eq!(history.document(), &initial_all);
}

/// Records a same-value ALL assignment as history and persistence intent without render invalidation.
///
/// # Panics
/// Panics if no-op values fail to store the explicit baseline, dirty history, or round-trip exactly.
#[test]
fn ton016_same_value_all_assignment_is_baseline_only_history() {
    let mut history = history();
    let before = history.document().clone();
    let first_channel = before
        .channel_topology()
        .expect("default model has topology")
        .channels()[0]
        .id;
    let effective = before
        .effective_channel_pattern(first_channel)
        .expect("default channel resolves");
    let edits = [
        (PropertyFieldId::Density, effective.density.density),
        (PropertyFieldId::DensityAspect, effective.density.aspect),
        (
            PropertyFieldId::RotationDegrees,
            effective.pattern_rotation_degrees,
        ),
        (PropertyFieldId::TranslationX, effective.translation_x),
        (PropertyFieldId::TranslationY, effective.translation_y),
    ];
    let configuration = before
        .edit_all_channel_start_configuration(&edits)
        .expect("same-value ALL assignments validate");
    assert!(configuration.appearance_baseline().density.is_some());
    let after = configuration.bind(&before).expect("baseline binds");
    assert_ne!(after, before);
    assert_eq!(
        after.effective_channel_pattern(first_channel).unwrap(),
        before.effective_channel_pattern(first_channel).unwrap()
    );
    let result = history
        .apply_document_configuration(&before, history.revision(), &configuration)
        .expect("baseline-only configuration publishes");
    assert!(!result.unchanged);
    assert!(result.affected_channels.is_empty());
    assert_eq!(result.invalidation, None);
    assert!(history.can_undo());
    assert_ne!(history.document(), &before);
    history
        .undo()
        .unwrap()
        .expect("baseline record is undoable");
    assert_eq!(history.document(), &before);
    history
        .redo()
        .unwrap()
        .expect("baseline record is redoable");
    assert_eq!(history.document(), &after);
}

/// Replaces one base Pattern and one named Pattern to verify their distinct baseline reset rules.
///
/// # Panics
/// Panics if ALL replacement retains recipe-relative baseline fields or named replacement alters
/// any shared baseline field, or if replacement history fails exact Undo/Redo.
#[test]
fn ton016_all_pattern_replacement_resets_recipe_baseline_but_named_replacement_retains_it() {
    let mut all_history = history();
    assign_all(&mut all_history, &LAST_ALL);
    let before_all_replacement = all_history.document().clone();
    let base = before_all_replacement.pattern_settings().clone();
    let base_definition = before_all_replacement
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .unwrap()
        .definition
        .clone();
    let recipe = PatternDefinitionRecipe::marks(PatternStructureRecipe::StraightGrid(
        toniator_domain::PatternDefinitionDraft {
            name: "TON-016 replacement pattern".into(),
            coverage: toniator_domain::CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        },
    ));
    all_history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: recipe.clone(),
        })
        .expect("ALL Pattern replacement validates");
    let after_all_replacement = all_history.document().clone();
    let all_baseline = after_all_replacement.appearance_baseline();
    assert_eq!(all_baseline.density, None);
    assert_eq!(all_baseline.density_aspect, None);
    assert_eq!(all_baseline.pattern_rotation_degrees, None);
    assert_eq!(all_baseline.translation_x, Some(0.17));
    assert_eq!(all_baseline.translation_y, Some(-0.21));
    for channel in after_all_replacement.channel_topology().unwrap().channels() {
        let effective = after_all_replacement
            .effective_channel_pattern(channel.id)
            .unwrap();
        assert_eq!(effective.translation_x, 0.17);
        assert_eq!(effective.translation_y, -0.21);
    }
    all_history
        .undo()
        .unwrap()
        .expect("ALL Pattern replacement is undoable");
    assert_eq!(all_history.document(), &before_all_replacement);
    all_history
        .redo()
        .unwrap()
        .expect("ALL Pattern replacement is redoable");
    assert_eq!(all_history.document(), &after_all_replacement);

    let mut named_history = history();
    assign_all(&mut named_history, &LAST_ALL);
    let before_named_replacement = named_history.document().clone();
    let base = before_named_replacement.pattern_settings().clone();
    let base_definition = before_named_replacement
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .unwrap()
        .definition
        .clone();
    let named_channel = before_named_replacement
        .channel_topology()
        .unwrap()
        .channels()[0]
        .id;
    named_history
        .apply(
            &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base,
                channel_id: named_channel,
                base_definition,
                recipe,
            },
        )
        .expect("named Pattern replacement validates");
    let after_named_replacement = named_history.document().clone();
    assert_eq!(
        named_history.document().appearance_baseline(),
        before_named_replacement.appearance_baseline()
    );
    named_history
        .undo()
        .unwrap()
        .expect("named Pattern replacement is undoable");
    assert_eq!(named_history.document(), &before_named_replacement);
    named_history
        .redo()
        .unwrap()
        .expect("named Pattern replacement is redoable");
    assert_eq!(named_history.document(), &after_named_replacement);
}

/// Rejects invalid stored effective values before they can seed a future model topology.
///
/// # Panics
/// Panics if a nonpositive density/aspect or nonfinite appearance scalar passes document validation.
#[test]
fn ton016_invalid_appearance_baselines_are_rejected() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 80.0,
            height: 60.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document validates");
    for baseline in [
        toniator_domain::DocumentAppearanceBaseline {
            density: Some(0.0),
            ..Default::default()
        },
        toniator_domain::DocumentAppearanceBaseline {
            density_aspect: Some(f64::NAN),
            ..Default::default()
        },
        toniator_domain::DocumentAppearanceBaseline {
            pattern_rotation_degrees: Some(f64::INFINITY),
            ..Default::default()
        },
        toniator_domain::DocumentAppearanceBaseline {
            translation_x: Some(f64::NAN),
            ..Default::default()
        },
        toniator_domain::DocumentAppearanceBaseline {
            translation_y: Some(f64::NEG_INFINITY),
            ..Default::default()
        },
    ] {
        assert!(document.clone().with_appearance_baseline(baseline).is_err());
    }
}
