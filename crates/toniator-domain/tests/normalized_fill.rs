use toniator_domain::{
    CanvasSpec, Document, MarkGeometryFieldEdit, SourceReference, SourceReferenceId,
};

/// Proves new documents expose the Stage 20E1 normalized fill defaults and rotation authority.
#[test]
fn default_document_uses_normalized_fill_defaults() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 80.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("source").unwrap()),
    )
    .unwrap();
    let channel = document.channel_model().unwrap();
    let topology = document.channel_topology().unwrap();
    assert_eq!(channel, toniator_domain::HalftoneChannelModel::Rgb);
    for modeled in topology.channels() {
        assert_eq!(modeled.mark_geometry_response.minimum_fill, 0.0);
        assert_eq!(modeled.mark_geometry_response.maximum_fill, 1.0);
        assert_eq!(modeled.mark_geometry_response.rotation_offset_degrees, 0.0);
    }
}

/// Proves fill commands retain realization invalidation while rejecting values outside 0..=2.
#[test]
fn normalized_fill_edits_are_bounded_and_realization_only() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 80.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("source").unwrap()),
    )
    .unwrap();
    let channel_id = document.channel_topology().unwrap().channels()[0].id;
    let (_, result) = document
        .apply_command(&toniator_domain::DocumentCommand::SetMarkGeometryField {
            channel_id,
            edit: MarkGeometryFieldEdit::MaximumFill(2.0),
        })
        .unwrap();
    assert_eq!(
        result.invalidation,
        toniator_domain::InvalidationLevel::Realization
    );
    assert!(
        document
            .apply_command(&toniator_domain::DocumentCommand::SetMarkGeometryField {
                channel_id,
                edit: MarkGeometryFieldEdit::MaximumFill(2.01),
            })
            .is_err()
    );
}
