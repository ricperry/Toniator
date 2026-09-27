use toniator_domain::{
    CanvasSpec, ChannelId, Document, DocumentHistory, DocumentSession, SourceReference,
    SourceReferenceId,
};
use toniator_engine::{EvaluationRequest, ResolvedSource, SourceFormatHint, evaluate};
use toniator_patterns::PresetRegistry;

/// Keeps guide and parametric paths channel-specific while scatter exposes no paths.
/// Both immutable source formats exercise the same evaluation result contract.
///
/// # Panics
/// Panics if evaluation fails, repeated guides disappear, or one channel borrows another's guides.
#[test]
fn evaluated_guides_follow_each_channels_family() {
    for (asset, hint) in [
        ("raster-sample.png", SourceFormatHint::Png),
        ("vector-sample.svg", SourceFormatHint::Svg),
    ] {
        let source_id = SourceReferenceId::new("guide-preview").unwrap();
        let document = Document::new_default_document(
            CanvasSpec {
                width: 96.0,
                height: 64.0,
            },
            SourceReference::Assigned(source_id.clone()),
        )
        .unwrap();
        let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
        PresetRegistry::bundled()
            .apply_to_selected(&mut history, ChannelId(1), "even-random-circles")
            .unwrap();
        PresetRegistry::bundled()
            .apply_to_selected(&mut history, ChannelId(3), "round-spiral-marks")
            .unwrap();
        let source = ResolvedSource::new(
            source_id,
            std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../assets")
                    .join(asset),
            )
            .unwrap(),
            hint,
        )
        .unwrap();
        let result = evaluate(EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            source,
        ))
        .unwrap();
        assert!(
            result
                .family_output(ChannelId(1))
                .unwrap()
                .structural_path_set()
                .is_none()
        );
        let guides = result
            .family_output(ChannelId(2))
            .unwrap()
            .structural_path_set()
            .unwrap();
        assert!(
            guides.paths().len() > 2,
            "retain every repeated guide, not just one per direction"
        );
        assert!(result.family_output(ChannelId(u64::MAX)).is_none());
        let curve = result
            .family_output(ChannelId(3))
            .unwrap()
            .structural_path_set()
            .unwrap();
        assert!(!curve.paths().is_empty());
        assert!(
            curve
                .paths()
                .iter()
                .all(|path| !path.path.segments().is_empty())
        );
    }
}
