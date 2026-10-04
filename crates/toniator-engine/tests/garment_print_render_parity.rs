use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use toniator_domain::{
    DocumentCommand, DocumentHistory, DocumentSession, PhysicalPrintSizeMm,
    PrintPreparationSettings,
};
use toniator_engine::{
    CacheDisposition, EvaluationLimits, EvaluationProfileCache, MediaTools, OutputRasterTarget,
    RasterAntialiasing, RasterBackground, encode_png, evaluate_profiled_cached_with_limits,
    frame_evaluation_request, open_source_media, write_svg,
};
use toniator_io::{load, save};

const SCHEMA10_BYTES: &[u8] =
    include_bytes!("../../toniator-io/tests/fixtures/garment-schema10-clean-fade.toniator");

/// Shows nonzero print intent preserves the actual PNG/SVG while warm derived caches survive.
#[test]
fn print_intent_edit_reuses_render_cache_and_preserves_native_exports() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "toniator-garment-engine-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&dir).unwrap();
    let old_path = dir.join("schema10.toniator");
    fs::write(&old_path, SCHEMA10_BYTES).unwrap();
    let loaded = load(&old_path).unwrap();
    let mut history =
        DocumentHistory::new(DocumentSession::new(loaded.document().clone()).unwrap());
    let mut media = open_source_media(loaded.sources(), MediaTools::default(), &|| false).unwrap();
    let mut cache = EvaluationProfileCache::default();
    let limits = EvaluationLimits::new(EvaluationLimits::UNBOUNDED_WORK_LIMIT).unwrap();
    let target = OutputRasterTarget::new(512, 192).unwrap();
    let request = frame_evaluation_request(history.session(), &mut media, 0, &|| false)
        .unwrap()
        .for_output(
            RasterBackground::Transparent,
            Some(target),
            RasterAntialiasing::On,
        );
    let baseline =
        evaluate_profiled_cached_with_limits(request, limits.clone(), &mut cache).unwrap();
    let original_png = encode_png(baseline.result.raster()).unwrap();
    let original_svg = write_svg(baseline.result.scene());
    let settings = PrintPreparationSettings::new(
        Some(PhysicalPrintSizeMm::new(101.6, 38.1).unwrap()),
        0.8,
        0.2,
    )
    .unwrap();
    let result = history
        .apply(&DocumentCommand::SetPrintPreparation {
            base: history.document().print_preparation().clone(),
            settings: settings.clone(),
        })
        .unwrap();
    assert_eq!(result.invalidation, None);
    let request = frame_evaluation_request(history.session(), &mut media, 0, &|| false)
        .unwrap()
        .for_output(
            RasterBackground::Transparent,
            Some(target),
            RasterAntialiasing::On,
        );
    let edited = evaluate_profiled_cached_with_limits(request, limits.clone(), &mut cache).unwrap();
    let diagnostics = edited.diagnostics.aggregate;
    assert_eq!(diagnostics.decoded_source, CacheDisposition::Hit);
    assert_eq!(diagnostics.family, CacheDisposition::Hit);
    assert_eq!(diagnostics.realization, CacheDisposition::Hit);
    assert_eq!(diagnostics.scene, CacheDisposition::Hit);
    assert_eq!(diagnostics.raster, CacheDisposition::Hit);
    assert_eq!(encode_png(edited.result.raster()).unwrap(), original_png);
    assert_eq!(write_svg(edited.result.scene()), original_svg);
    let saved_path = dir.join("schema11.toniator");
    save(&saved_path, history.document(), loaded.sources()).unwrap();
    let reopened = load(&saved_path).unwrap();
    assert_eq!(reopened.document().print_preparation(), &settings);
    let reopened_session = DocumentSession::new(reopened.document().clone()).unwrap();
    let mut reopened_media =
        open_source_media(reopened.sources(), MediaTools::default(), &|| false).unwrap();
    let request = frame_evaluation_request(&reopened_session, &mut reopened_media, 0, &|| false)
        .unwrap()
        .for_output(
            RasterBackground::Transparent,
            Some(target),
            RasterAntialiasing::On,
        );
    let replay = evaluate_profiled_cached_with_limits(
        request,
        limits,
        &mut EvaluationProfileCache::default(),
    )
    .unwrap();
    assert_eq!(encode_png(replay.result.raster()).unwrap(), original_png);
    assert_eq!(write_svg(replay.result.scene()), original_svg);
    fs::remove_dir_all(dir).unwrap();
}
