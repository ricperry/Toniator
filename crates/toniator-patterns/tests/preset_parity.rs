use toniator_domain::{
    CanvasSpec, ChannelId, Document, DocumentHistory, DocumentSession, SourceReference,
};
use toniator_patterns::{
    GridInspectRequest, PresetRegistry, evaluate_typed_family, resolve_pattern_pipeline,
};

/// Creates a fresh default document whose output is compared against a preset
/// reconstruction through the shared canonical typed family boundary.
fn history() -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    DocumentHistory::new(DocumentSession::new(document).unwrap())
}

/// Resolves the current selected channel through document-owned inheritance
/// before constructing the shared family request; this test never recreates
/// effective layout arithmetic or adds a preset-specific evaluation path.
fn request(document: &Document) -> GridInspectRequest {
    let channel = document
        .effective_channel_pattern(ChannelId(1))
        .expect("the default selected channel resolves through document authority");
    GridInspectRequest {
        canvas: document.canvas().clone(),
        density: channel.density,
        rotation_degrees: channel.pattern_rotation_degrees,
        translation_x: channel.translation_x,
        translation_y: channel.translation_y,
        guard_steps: 2,
        support_radius: 4.5,
        max_family_candidates: 1_000_000,
    }
}

/// Proves representative grid and random recipes enter exactly the existing
/// canonical family evaluator with deterministic output and no preset-name path.
#[test]
fn bundled_grid_and_random_recipes_have_deterministic_canonical_family_parity() {
    let registry = PresetRegistry::bundled();
    let mut history = history();
    for id in ["straight-grid-circles", "even-random-circles"] {
        registry
            .apply_to_selected(&mut history, ChannelId(1), id)
            .unwrap();
        let definition = history
            .document()
            .pattern_definition_for(ChannelId(1))
            .unwrap();
        let first = evaluate_typed_family(definition, &request(history.document())).unwrap();
        let second = evaluate_typed_family(definition, &request(history.document())).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            resolve_pattern_pipeline(definition)
                .unwrap()
                .family
                .provenance
                .definition_id,
            definition.id.0
        );
    }
}
