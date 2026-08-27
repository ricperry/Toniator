use toniator_domain::{
    CanvasSpec, Document, DocumentCommand, DocumentHistory, DocumentSession, DocumentSessionError,
    InvalidationLevel, MarkGeometryFieldEdit, PatternDefinitionDraft, PatternDefinitionEdit,
    Revision, SourceReference,
};

/// Builds one current modeled history fixture with shared base pattern authority.
fn history() -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default history document validates");
    DocumentHistory::new(DocumentSession::new(document).expect("history session validates"))
}

/// Proves structural definition commands require history and record an exact reversible snapshot.
#[test]
fn definition_commands_require_history_and_undo_exactly() {
    let source = history().document().clone();
    let definition_id = source.pattern_settings().definition_id;
    let base = source.pattern_definition_bundles()[0].definition.clone();
    let commands = [
        DocumentCommand::AddPatternDefinition {
            definition: PatternDefinitionDraft {
                name: "history-only definition".into(),
                coverage: base.coverage.clone(),
            },
        },
        DocumentCommand::DuplicatePatternDefinition { definition_id },
        DocumentCommand::EditSharedPatternDefinition {
            definition_id,
            base_definition: base,
            edit: PatternDefinitionEdit::SetCoverageGuardSteps { guard_steps: 3 },
        },
    ];
    let mut session = DocumentSession::new(source.clone()).expect("direct session validates");
    for command in &commands {
        assert_eq!(
            session.apply(command),
            Err(DocumentSessionError::HistoryRequired)
        );
        assert_eq!(session.document(), &source);
        assert_eq!(session.revision(), Revision(0));
    }

    let mut history = history();
    let result = history
        .apply(&commands[0])
        .expect("history applies definition addition");
    assert_eq!(result.invalidation, Some(InvalidationLevel::Family));
    let after = history.document().clone();
    assert_eq!(history.revision(), Revision(1));
    assert_eq!(history.undo().expect("undo succeeds"), Some(result.clone()));
    assert_eq!(history.document(), &source);
    assert_eq!(history.redo().expect("redo succeeds"), Some(result));
    assert_eq!(history.document(), &after);
}

/// Proves an output-keyed channel delta round-trips through history without materializing its companion.
#[test]
fn keyed_mark_delta_history_restores_exact_inherited_intent() {
    let mut history = history();
    let before = history.document().clone();
    let channel_id = before
        .channel_topology()
        .expect("modeled topology")
        .channels()[0]
        .id;
    let output_layer_id = before.pattern_definition_bundles()[0]
        .definition
        .output_layers[0]
        .id;
    let command = before
        .set_channel_mark_response_field_for_effective(
            channel_id,
            output_layer_id,
            MarkGeometryFieldEdit::MinimumFill(0.25),
        )
        .expect("keyed delta command builds");
    let result = history.apply(&command).expect("keyed delta applies");
    assert_eq!(result.invalidation, Some(InvalidationLevel::Realization));
    let after = history.document().clone();
    let delta = &after
        .channel_pattern_instance(channel_id)
        .expect("channel pattern instance")
        .output_response_deltas[0];
    assert_eq!(delta.output_layer_id, output_layer_id);
    assert_eq!(
        history.undo().expect("delta undo succeeds"),
        Some(result.clone())
    );
    assert_eq!(history.document(), &before);
    assert_eq!(history.redo().expect("delta redo succeeds"), Some(result));
    assert_eq!(history.document(), &after);
}
