use toniator_domain::*;

/// Creates a current document with three modeled channels and nearest-mode adaptive outputs.
/// # Panics
/// Panics if the canonical starter or adaptive scoped command fails validation.
fn adaptive_document() -> Document {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 128.0,
            height: 96.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let definition = &document.pattern_definition_bundles()[0].definition;
    document
        .edit_pattern_adaptive_square_configuration(
            definition,
            definition.output_layers[0].id,
            None,
        )
        .unwrap()
        .bind(&document)
        .unwrap()
}

/// Reads channel order from the actual modeled topology rather than assuming channel IDs.
/// # Panics
/// Panics if the modeled test document loses its channel topology.
fn channels(document: &Document) -> Vec<ChannelId> {
    document
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect()
}

/// Captures a blue-noise assignment against the selected channel's current definition.
/// # Panics
/// Panics if its typed active adaptive output cannot accept this policy.
fn edit(document: &Document, channel: ChannelId, enabled: bool) -> DocumentCommand {
    let definition = document.pattern_definition_for(channel).unwrap().clone();
    DocumentCommand::EditSelectedChannelPatternDefinition {
        channel_id: channel,
        edit: PatternDefinitionEdit::SetAdaptiveBlueNoise {
            output_layer_id: definition.output_layers[0].id,
            enabled,
        },
        base_definition: definition,
    }
}

/// Preserves unrelated recipes, mappings, hidden defaults and stable named identity across history.
/// # Panics
/// Panics if typed policy, descriptor readback, exact undo/redo or named isolation regress.
#[test]
fn adaptive_blue_noise_named_and_all_history_preserve_unrelated_authority() {
    let original = adaptive_document();
    let ids = channels(&original);
    let mut history = DocumentHistory::new(DocumentSession::new(original.clone()).unwrap());
    history.apply(&edit(&original, ids[0], true)).unwrap();
    let named = history.document().clone();
    let binding = CalibrationBinding::for_channel(&named, ids[0]).unwrap();
    assert!(binding.uses_adaptive_blue_noise());
    assert!(
        named
            .pattern_definition_for(ids[0])
            .unwrap()
            .uses_adaptive_square_marks()
    );
    assert_eq!(
        named.pattern_definition_for(ids[1]),
        original.pattern_definition_for(ids[1])
    );
    assert_eq!(named.pattern_settings(), original.pattern_settings());
    assert_eq!(named.response_settings(), original.response_settings());
    assert_eq!(named.temporal_authority(), original.temporal_authority());
    history.undo().unwrap();
    assert_eq!(history.document(), &original);
    history.redo().unwrap();
    assert_eq!(history.document(), &named);
    // A non-adaptive channel remains excluded even when the representative is adaptive.
    let definition = named.pattern_definition_for(ids[1]).unwrap().clone();
    history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: ids[1],
            edit: PatternDefinitionEdit::SetOutputMarkPrototype {
                output_layer_id: definition.output_layers[0].id,
                prototype: MarkPrototype::Circle,
            },
            base_definition: definition,
        })
        .unwrap();
    let before = history.document().clone();
    let representative = before.pattern_definition_for(ids[0]).unwrap();
    let configuration = before
        .edit_all_pattern_definition_configuration(
            representative,
            &PatternDefinitionEdit::SetAdaptiveBlueNoise {
                output_layer_id: representative.output_layers[0].id,
                enabled: true,
            },
        )
        .unwrap();
    history
        .apply_document_configuration(&before, history.revision(), &configuration)
        .unwrap();
    let after = history.document().clone();
    assert_eq!(
        after.pattern_definition_for(ids[1]),
        before.pattern_definition_for(ids[1])
    );
    assert!(
        CalibrationBinding::for_channel(&after, ids[2])
            .unwrap()
            .uses_adaptive_blue_noise()
    );
    assert!(
        after
            .baseline_calibration_binding()
            .unwrap()
            .uses_adaptive_blue_noise()
    );
    history.undo().unwrap();
    assert_eq!(history.document(), &before);
    history.redo().unwrap();
    assert_eq!(history.document(), &after);
    let descriptors = after.property_values();
    assert!(descriptors.iter().any(|value| value.descriptor.field
        == PropertyFieldId::AdaptiveBlueNoise
        && value.value == PropertyCurrentValueKind::Boolean(true)));
    let circle = after.pattern_definition_for(ids[1]).unwrap();
    assert!(!descriptors.iter().any(|value| value.descriptor.field == PropertyFieldId::AdaptiveBlueNoise && matches!(value.descriptor.target, PropertyTarget::OutputLayer(id, _) if id == circle.id)));
}

/// Rejects applying an adaptive-only policy to an ordinary mark instead of silently changing it.
/// # Panics
/// Panics if the domain accepts an inactive control or nearest and mixed bindings compare equal.
#[test]
fn adaptive_blue_noise_policy_is_typed_and_inactive_for_circle() {
    let document = adaptive_document();
    let ids = channels(&document);
    let nearest = CalibrationBinding::for_channel(&document, ids[0]).unwrap();
    let on = document
        .apply_command(&edit(&document, ids[0], true))
        .unwrap()
        .0;
    let mixed = CalibrationBinding::for_channel(&on, ids[0]).unwrap();
    assert!(!nearest.is_compatible_with(&mixed));
    assert_ne!(
        nearest.profile_for_reference(CALIBRATION_PROFILE_ADAPTIVE_SQUARE),
        mixed.profile_for_reference(CALIBRATION_PROFILE_ADAPTIVE_SQUARE)
    );
    let ordinary =
        Document::new_default_document(document.canvas().clone(), SourceReference::Unassigned)
            .unwrap();
    assert!(
        ordinary
            .apply_command(&edit(&ordinary, ids[0], true))
            .is_err()
    );
}

/// Retains independently authored blue-noise policy when ALL adds adaptive shapes to a circle.
/// # Panics
/// Panics if shape-only selection resets an existing adaptive mode or changes its stable definition.
#[test]
fn adaptive_blue_noise_shape_reselection_preserves_existing_modes() {
    let original = adaptive_document();
    let ids = channels(&original);
    let on = original
        .apply_command(&edit(&original, ids[0], true))
        .unwrap()
        .0;
    let circle = on.pattern_definition_for(ids[1]).unwrap().clone();
    let before = on
        .apply_command(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: ids[1],
            base_definition: circle.clone(),
            edit: PatternDefinitionEdit::SetOutputMarkPrototype {
                output_layer_id: circle.output_layers[0].id,
                prototype: MarkPrototype::Circle,
            },
        })
        .unwrap()
        .0;
    let source = before.pattern_definition_for(ids[0]).unwrap();
    let configuration = before
        .edit_pattern_adaptive_square_configuration(source, source.output_layers[0].id, None)
        .unwrap();
    let after = configuration.bind(&before).unwrap();
    assert_eq!(
        after.pattern_definition_for(ids[0]),
        before.pattern_definition_for(ids[0])
    );
    assert!(
        CalibrationBinding::for_channel(&after, ids[0])
            .unwrap()
            .uses_adaptive_blue_noise()
    );
    assert!(
        !CalibrationBinding::for_channel(&after, ids[1])
            .unwrap()
            .uses_adaptive_blue_noise()
    );
    assert!(
        after
            .pattern_definition_for(ids[1])
            .unwrap()
            .uses_adaptive_square_marks()
    );
}
