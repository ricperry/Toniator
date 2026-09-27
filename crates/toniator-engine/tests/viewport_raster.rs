//! Viewport requests retain source/canonical caches and allocate only the requested raster.

use std::{fs, path::Path};
use toniator_domain::{CanvasSpec, Document, DocumentSession, SourceReference, SourceReferenceId};
use toniator_engine::{
    CacheDisposition, EvaluationLimits, EvaluationProfileCache, EvaluationRequest,
    PreviewRasterTarget, ResolvedSource, SourceFormatHint, evaluate_profiled_cached_with_limits,
};
use toniator_render::encode_png;

/// Exercises both immutable intrinsic sources with crop-only raster invalidation and native files.
/// Artifact writes are confined to this task's validation directory; source bytes remain unchanged.
#[test]
fn viewport_raster_reuses_source_scene_and_keys_pan_zoom_hidpi() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = root.join("target/validation/stage-viewport-export-20260927");
    fs::create_dir_all(&output).unwrap();
    for (asset, width, height, format) in [
        ("raster-sample.png", 1024.0, 1024.0, SourceFormatHint::Png),
        ("vector-sample.svg", 900.0, 620.0, SourceFormatHint::Svg),
    ] {
        let id = SourceReferenceId::new(asset).unwrap();
        let session = DocumentSession::new(
            Document::new_default_document(
                CanvasSpec { width, height },
                SourceReference::Assigned(id.clone()),
            )
            .unwrap(),
        )
        .unwrap();
        let source = ResolvedSource::new(
            id,
            fs::read(root.join("assets").join(asset)).unwrap(),
            format,
        )
        .unwrap();
        let request = EvaluationRequest::new(session.document_evaluation_snapshot(), source);
        let mut cache = EvaluationProfileCache::default();
        let fit = evaluate_profiled_cached_with_limits(
            request
                .clone()
                .for_preview(PreviewRasterTarget::new(384, 256).unwrap()),
            EvaluationLimits::default(),
            &mut cache,
        )
        .unwrap();
        fs::write(
            output.join(format!("{asset}-fit.png")),
            encode_png(fit.result.raster()).unwrap(),
        )
        .unwrap();
        let scene_identity = fit.result.scene().identity().clone();
        for (ordinal, target) in [
            PreviewRasterTarget::for_viewport(384, 256, 100.0, 70.0, 2.0).unwrap(),
            PreviewRasterTarget::for_viewport(384, 256, 140.0, 90.0, 2.0).unwrap(),
            PreviewRasterTarget::for_viewport(384, 256, 140.0, 90.0, 4.0).unwrap(),
            PreviewRasterTarget::for_viewport(768, 512, 140.0, 90.0, 4.0).unwrap(),
        ]
        .into_iter()
        .enumerate()
        {
            let evaluated = evaluate_profiled_cached_with_limits(
                request.clone().for_preview(target),
                EvaluationLimits::default(),
                &mut cache,
            )
            .unwrap();
            let diagnostics = evaluated.diagnostics.aggregate;
            assert_eq!(diagnostics.decoded_source, CacheDisposition::Hit);
            assert_eq!(diagnostics.family, CacheDisposition::Hit);
            assert_eq!(diagnostics.realization, CacheDisposition::Hit);
            assert_eq!(diagnostics.scene, CacheDisposition::Hit);
            assert_eq!(diagnostics.raster, CacheDisposition::Miss);
            assert_eq!(*evaluated.result.scene().identity(), scene_identity);
            assert_eq!(
                (
                    evaluated.result.raster().width(),
                    evaluated.result.raster().height()
                ),
                (target.width(), target.height())
            );
            fs::write(
                output.join(format!("{asset}-crop-{ordinal}.png")),
                encode_png(evaluated.result.raster()).unwrap(),
            )
            .unwrap();
            let repeated = evaluate_profiled_cached_with_limits(
                request.clone().for_preview(target),
                EvaluationLimits::default(),
                &mut cache,
            )
            .unwrap();
            assert_eq!(repeated.diagnostics.aggregate.raster, CacheDisposition::Hit);
            assert_eq!(repeated.result.raster(), evaluated.result.raster());
        }
    }
}
