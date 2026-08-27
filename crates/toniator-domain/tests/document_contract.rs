use toniator_domain::{
    CanvasSpec, Document, DocumentCommand, DocumentId, DocumentSession, HalftoneChannelModel,
    InvalidationLevel, PatternGeometryResponse, Revision, SourceReference, SourceReferenceId,
};

/// Builds one current modeled document through the public factory authority.
fn document() -> Document {
    Document::new_default_document(
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        SourceReference::Assigned(
            SourceReferenceId::new("document-contract-source").expect("source ID validates"),
        ),
    )
    .expect("current default document validates")
}

/// Proves the default factory emits aligned v5 bundles and modeled channel instances.
#[test]
fn default_document_factory_builds_current_modeled_authority() {
    let document = document();
    assert_eq!(document.channel_model(), Some(HalftoneChannelModel::Rgb));
    assert_eq!(document.pattern_definition_bundles().len(), 1);
    let bundle = &document.pattern_definition_bundles()[0];
    bundle.validate().expect("default bundle aligns");
    assert_eq!(bundle.definition.output_layers.len(), 1);
    assert_eq!(bundle.output_settings.len(), 1);
    assert!(matches!(
        bundle.output_settings[0].response,
        PatternGeometryResponse::Marks(_)
    ));
    assert_eq!(
        document
            .channel_topology()
            .expect("modeled topology")
            .channels()
            .len(),
        3
    );
    for channel in document
        .channel_topology()
        .expect("modeled topology")
        .channels()
    {
        assert!(channel.pattern_instance.definition_override.is_none());
        let effective = document
            .effective_channel_pattern(channel.id)
            .expect("effective channel pattern resolves");
        assert_eq!(
            effective.definition_id,
            document.pattern_settings().definition_id
        );
        assert_eq!(effective.output_settings.len(), 1);
    }
    document.validate().expect("whole document validates");
    document
        .validate_property_descriptors()
        .expect("descriptor surface is complete");
}

/// Proves invalid canvas input rejects before any document authority can be published.
#[test]
fn default_document_rejects_invalid_canvas() {
    let error = Document::new_default_document(
        CanvasSpec {
            width: 0.0,
            height: 600.0,
        },
        SourceReference::Unassigned,
    )
    .expect_err("zero-width canvas rejects");
    assert_eq!(error.path(), "canvas.width");
}

/// Proves source edits advance revision and invalidate source-derived state atomically.
#[test]
fn source_command_advances_document_token_and_invalidates_source() {
    let document = document();
    let channel_ids = document
        .channel_topology()
        .expect("modeled topology")
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    let mut session = DocumentSession::new(document.clone()).expect("session validates");
    let before = session.document_evaluation_snapshot();
    let result = session
        .apply(&DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(
                SourceReferenceId::new("replacement-source").expect("source ID validates"),
            ),
        })
        .expect("source edit applies");
    assert_eq!(result.invalidation, Some(InvalidationLevel::Source));
    assert_eq!(result.affected_channels, channel_ids);
    assert_eq!(session.revision(), Revision(1));
    assert!(!session.accepts_document_evaluation(before.token()));
    let after = session.document_evaluation_snapshot();
    assert_eq!(after.token().document_id(), DocumentId(1));
    assert_eq!(after.token().revision(), Revision(1));
    assert_eq!(after.document(), session.document());
}
