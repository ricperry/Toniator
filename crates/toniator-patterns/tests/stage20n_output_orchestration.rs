//! Focused Stage 20N explicit output-capability realization binding coverage.

use toniator_domain::{
    CanvasSpec, Document, EffectivePatternOutputSettings, PatternOutputLayerId, SourceReference,
};
use toniator_patterns::{resolve_document_pattern_pipeline, validate_output_realization_binding};

/// Proves a resolved output setting binds only to the exact ordered capability that realizes it.
#[test]
fn explicit_output_binding_requires_matching_layer_identity_and_response_kind() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document");
    let bundle = &document.pattern_definition_bundles()[0];
    let effective = document
        .effective_channel_pattern(toniator_domain::ChannelId(1))
        .expect("effective channel");
    let plan = resolve_document_pattern_pipeline(&document, &bundle.definition)
        .expect("current document resolves one output plan");
    let capability = plan
        .output_capability(effective.output_settings[0].output_layer_id)
        .expect("matching output capability");
    validate_output_realization_binding(&plan, capability, &effective.output_settings[0])
        .expect("matched binding");
    let foreign = EffectivePatternOutputSettings {
        output_layer_id: PatternOutputLayerId(999),
        response: effective.output_settings[0].response.clone(),
    };
    assert_eq!(
        validate_output_realization_binding(&plan, capability, &foreign)
            .expect_err("foreign output setting")
            .path(),
        "pattern.output_layers.setting"
    );
}
