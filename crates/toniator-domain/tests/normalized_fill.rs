use toniator_domain::{
    CanvasSpec, Document, InvalidationLevel, MarkGeometryFieldEdit, PatternGeometryResponse,
    SourceReference, SourceReferenceId,
};

/// Proves new documents expose normalized mark fill and independent shape-rotation authority.
#[test]
fn default_document_uses_normalized_fill_defaults() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 80.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("source").expect("source ID validates")),
    )
    .expect("default document validates");
    assert_eq!(
        document.channel_model(),
        Some(toniator_domain::HalftoneChannelModel::Rgb)
    );
    assert_eq!(document.pattern_settings().shape_rotation_degrees, 0.0);
    for channel in document
        .channel_topology()
        .expect("modeled topology")
        .channels()
    {
        let effective = document
            .effective_channel_pattern(channel.id)
            .expect("channel effective pattern resolves");
        let PatternGeometryResponse::Marks(response) = &effective.output_settings[0].response
        else {
            panic!("default output remains a mark response")
        };
        assert_eq!(response.minimum_fill, 0.0);
        assert_eq!(response.maximum_fill, 1.0);
        assert_eq!(effective.shape_rotation_degrees, 0.0);
    }
}

/// Proves keyed fill-delta commands retain realization invalidation and normalized bounds.
#[test]
fn normalized_fill_edits_are_bounded_and_realization_only() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 80.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("source").expect("source ID validates")),
    )
    .expect("default document validates");
    let channel_id = document
        .channel_topology()
        .expect("modeled topology")
        .channels()[0]
        .id;
    let output_layer_id = document.pattern_definition_bundles()[0]
        .definition
        .output_layers[0]
        .id;
    let command = document
        .set_channel_mark_response_field_for_effective(
            channel_id,
            output_layer_id,
            MarkGeometryFieldEdit::MaximumFill(2.0),
        )
        .expect("maximum-fill delta command builds");
    let (candidate, result) = document
        .apply_command(&command)
        .expect("maximum-fill delta applies");
    assert_eq!(result.invalidation, Some(InvalidationLevel::Realization));
    let PatternGeometryResponse::Marks(response) = &candidate
        .effective_channel_pattern(channel_id)
        .expect("edited response resolves")
        .output_settings[0]
        .response
    else {
        panic!("edited output remains a mark response")
    };
    assert_eq!(response.maximum_fill, 2.0);
    assert!(
        document
            .set_channel_mark_response_field_for_effective(
                channel_id,
                output_layer_id,
                MarkGeometryFieldEdit::MaximumFill(2.01),
            )
            .is_err()
    );
}
