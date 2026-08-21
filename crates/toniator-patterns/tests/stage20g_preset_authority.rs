use toniator_domain::{
    CanvasSpec, ChannelId, Document, DocumentHistory, DocumentSession, SourceReference,
};
use toniator_patterns::PresetRegistry;

/// Builds a current modeled history with one inherited definition shared by all channels.
fn history() -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document is valid");
    DocumentHistory::new(DocumentSession::new(document).expect("valid session"))
}

/// Materializes the same ID-free registry recipe as either a fresh base or a selected override.
#[test]
fn one_preset_recipe_targets_document_base_or_selected_channel() {
    let registry = PresetRegistry::bundled();
    let recipe = registry
        .reconstruct("even-random-circles")
        .expect("bundled recipe exists");

    let mut base = history();
    let base_result = registry
        .apply_to_document_base(&mut base, "even-random-circles")
        .expect("base preset applies");
    assert_eq!(
        base_result.affected_channels,
        vec![ChannelId(1), ChannelId(2), ChannelId(3)]
    );
    let base_id = base.document().pattern_settings().definition_id;
    assert_ne!(base_id.0, 1);
    assert_eq!(base.document().pattern_definitions().len(), 2);

    let mut selected = history();
    let selected_result = registry
        .apply_to_selected(&mut selected, ChannelId(2), "even-random-circles")
        .expect("selected preset applies");
    assert_eq!(selected_result.affected_channels, vec![ChannelId(2)]);
    let override_id = selected
        .document()
        .channel_pattern_instance(ChannelId(2))
        .expect("selected intent exists")
        .definition_override
        .expect("selected override is explicit");
    assert_ne!(override_id.0, 1);
    assert_eq!(selected.document().pattern_settings().definition_id.0, 1);
    assert_eq!(selected.document().pattern_definitions().len(), 2);
    assert_eq!(
        base.document()
            .pattern_definitions()
            .iter()
            .find(|definition| definition.id == base_id)
            .expect("base definition")
            .name,
        selected
            .document()
            .pattern_definitions()
            .iter()
            .find(|definition| definition.id == override_id)
            .expect("override definition")
            .name
    );
    assert_eq!(
        registry
            .reconstruct("even-random-circles")
            .expect("recipe remains ID-free"),
        recipe
    );
}
