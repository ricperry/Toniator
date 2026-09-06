//! Focused frame-provider, temporal-snapshot, and transactional cache integration evidence.

use super::*;
use toniator_domain::{
    Document, Easing, FrameRange, FrameRate, ProjectTiming, PropertyFieldId, PropertyTarget,
    RationalTime, ScalarEndOverride, TemporalEndOverride,
};

/// Creates a tiny immutable decoded frame with truthful cache identity and no encoded-image bytes.
fn frame(index: u64, rgba: [u8; 4]) -> SourceFrame {
    let field = Arc::new(SourceField::from_straight_rgba8(1, 1, rgba.to_vec()).unwrap());
    SourceFrame {
        identity: FrameIdentity {
            source_fingerprint: "test-media".into(),
            stream_index: 0,
            original_pts: index as i64,
            time_base: RationalTime::new(1, 30).unwrap(),
            decoder_contract: "test-decoder-v1".into(),
            color_policy: "straight-srgb".into(),
            decoded_pixel_hash: field.identity().decoded_pixel_hash.clone(),
        },
        field,
        index,
        normalized_time: RationalTime::new(index, 30).unwrap(),
    }
}

/// Creates one source-backed document whose End animation affects only channel presentation.
fn session() -> DocumentSession {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 32.0,
            height: 32.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("temporal-source").unwrap()),
    )
    .unwrap()
    .with_temporal_authority(
        ProjectTiming::new(
            FrameRate::new(30, 1).unwrap(),
            FrameRange::new(0, 3).unwrap(),
        ),
        vec![TemporalEndOverride::Scalar(ScalarEndOverride {
            target: PropertyTarget::Channel(ChannelId(1)),
            field: PropertyFieldId::Opacity,
            end: 0.25,
            easing: Easing::Linear,
        })],
    )
    .unwrap();
    DocumentSession::new(document).unwrap()
}

/// Builds a frame-specific immutable request under the original document revision token.
fn request(session: &DocumentSession, output_frame: u64, source: SourceFrame) -> EvaluationRequest {
    EvaluationRequest::new(
        session
            .document_evaluation_snapshot_at_frame(output_frame)
            .unwrap(),
        ResolvedSource::from_frame(SourceReferenceId::new("temporal-source").unwrap(), source)
            .unwrap(),
    )
}

/// Proves that awaiting an external media decode cancels candidates without losing accepted cache.
///
/// # Panics
/// Panics on a five-second worker timeout, stale publication, lost cache reuse or ticket reuse.
#[test]
fn media_decode_cancellation_retains_accepted_cache_and_rejects_candidate() {
    let session = session();
    let scheduler = EvaluationScheduler::new().unwrap();
    let receive = || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(completion) = scheduler.try_receive_latest().unwrap() {
                break completion;
            }
            assert!(std::time::Instant::now() < deadline, "scheduler deadline");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    };
    let first_ticket = scheduler
        .submit(request(&session, 0, frame(0, [20, 30, 40, 255])))
        .unwrap();
    let first = receive();
    assert!(scheduler.accept_completion(&first, &session).unwrap());
    scheduler.cancel_pending();
    assert!(!scheduler.is_latest(first_ticket));
    let candidate_ticket = scheduler
        .submit(request(&session, 2, frame(2, [50, 60, 70, 255])))
        .unwrap();
    let candidate = receive();
    scheduler.cancel_pending();
    assert!(!scheduler.accept_completion(&candidate, &session).unwrap());
    let final_ticket = scheduler
        .submit(request(&session, 0, frame(0, [20, 30, 40, 255])))
        .unwrap();
    let restored = receive();
    assert!(final_ticket.value() > candidate_ticket.value());
    assert_eq!(
        restored.cache_diagnostics().unwrap().aggregate.raster,
        CacheDisposition::Hit
    );
    assert!(scheduler.accept_completion(&restored, &session).unwrap());
}

/// Proves direct decoded fields bypass image decoding and reject forged pixel identity.
#[test]
fn decoded_frame_boundary_retains_pixels_and_checks_identity() {
    let source = frame(0, [13, 41, 128, 0]);
    let resolved = ResolvedSource::from_frame(
        SourceReferenceId::new("temporal-source").unwrap(),
        source.clone(),
    )
    .unwrap();
    assert!(resolved.bytes().is_none());
    assert!(resolved.format().is_none());
    assert!(Arc::ptr_eq(
        &resolved.decoded_field().unwrap(),
        &source.field
    ));
    let mut forged = source;
    forged.identity.decoded_pixel_hash = "wrong".into();
    assert!(
        ResolvedSource::from_frame(SourceReferenceId::new("temporal-source").unwrap(), forged)
            .is_err()
    );
}

/// Proves frame identity invalidates decoding while unchanged pixels and presentation preserve geometry reuse.
#[test]
fn frames_and_animation_reuse_only_valid_cache_layers() {
    let session = session();
    let mut cache = DocumentDerivedCache::default();
    let first = evaluate_cached_document(
        request(&session, 0, frame(0, [100, 150, 200, 255])),
        EvaluationLimits::default(),
        &cache,
        &NeverCancelled,
    )
    .unwrap();
    assert_eq!(first.result.token(), session.document_evaluation_token());
    cache.commit(first.transaction);

    let end = evaluate_cached_document(
        request(&session, 2, frame(1, [100, 150, 200, 255])),
        EvaluationLimits::default(),
        &cache,
        &NeverCancelled,
    )
    .unwrap();
    assert_eq!(
        end.diagnostics.aggregate.decoded_source,
        CacheDisposition::Miss
    );
    assert!(
        end.diagnostics
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Hit
                && channel.realization == CacheDisposition::Hit)
    );
    assert_eq!(end.diagnostics.aggregate.scene, CacheDisposition::Miss);
    cache.commit(end.transaction);

    let repeat = evaluate_cached_document(
        request(&session, 2, frame(1, [100, 150, 200, 255])),
        EvaluationLimits::default(),
        &cache,
        &NeverCancelled,
    )
    .unwrap();
    assert_eq!(
        repeat.diagnostics.aggregate.decoded_source,
        CacheDisposition::Hit
    );
    assert_eq!(repeat.diagnostics.aggregate.raster, CacheDisposition::Hit);

    let changed = evaluate_cached_document(
        request(&session, 2, frame(2, [20, 30, 50, 255])),
        EvaluationLimits::default(),
        &cache,
        &NeverCancelled,
    )
    .unwrap();
    assert!(
        changed
            .diagnostics
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Hit
                && channel.realization == CacheDisposition::Miss)
    );
    // A cancelled candidate never replaces the last successful decoded frame or geometry.
    let key_before = cache.decoded_source.as_ref().unwrap().0.clone();
    let cancelled = AtomicBool::new(true);
    assert!(matches!(
        evaluate_cached_document(
            request(&session, 1, frame(2, [20, 30, 50, 255])),
            EvaluationLimits::default(),
            &cache,
            &AtomicCancellation(&cancelled)
        ),
        Err(EvaluationRunError::Cancelled)
    ));
    assert_eq!(cache.decoded_source.as_ref().unwrap().0, key_before);
}

/// Writes current-stage intrinsic PNG/SVG endpoint witnesses through the decoded-frame boundary.
#[test]
fn native_endpoint_artifacts_use_both_immutable_sources() {
    use toniator_sampling::{FrameSource, StillImageSource};
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = root.join("target/validation/stage22-frame-engine");
    std::fs::create_dir_all(&output).unwrap();
    for (stem, name, hint) in [
        ("raster", "raster-sample.png", SourceFormatHint::Png),
        ("vector", "vector-sample.svg", SourceFormatHint::Svg),
    ] {
        let mut source =
            StillImageSource::new(std::fs::read(root.join("assets").join(name)).unwrap(), hint)
                .unwrap();
        let canvas = CanvasSpec {
            width: f64::from(source.metadata().width),
            height: f64::from(source.metadata().height),
        };
        let template = session();
        let document = Document::new_default_document(canvas, template.document().source().clone())
            .unwrap()
            .with_temporal_authority(
                template.document().project_timing().clone(),
                template.document().temporal_end_overrides().to_vec(),
            )
            .unwrap();
        let session = DocumentSession::new(document).unwrap();
        let source_frame = source.frame_at(RationalTime::default(), &|| false).unwrap();
        let mut cache = DocumentDerivedCache::default();
        for (index, label) in [(0, "start"), (2, "end")] {
            let evaluation = evaluate_cached_document(
                request(&session, index, source_frame.clone()),
                EvaluationLimits::default(),
                &cache,
                &NeverCancelled,
            )
            .unwrap();
            let svg = write_svg(evaluation.result.scene());
            let raster = toniator_render::rasterize_output(
                evaluation.result.scene(),
                RasterBackground::default_for_model(Some(HalftoneChannelModel::Rgb)),
                None,
                RasterAntialiasing::On,
            )
            .unwrap();
            std::fs::write(
                output.join(format!("{stem}-{label}.png")),
                encode_png(&raster).unwrap(),
            )
            .unwrap();
            std::fs::write(output.join(format!("{stem}-{label}.svg")), &svg).unwrap();
            if stem == "vector" {
                use image::ImageEncoder;
                let field = decode_source(svg.as_bytes(), SourceFormatHint::Svg).unwrap();
                let (width, height) = (field.identity().width, field.identity().height);
                let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
                for y in 0..height {
                    for x in 0..width {
                        let pixel = field.pixel(x, y).unwrap();
                        rgba.extend(
                            [pixel.red, pixel.green, pixel.blue, pixel.alpha]
                                .map(|value| (value * 255.0).round() as u8),
                        );
                    }
                }
                let mut png = Vec::new();
                image::codecs::png::PngEncoder::new(&mut png)
                    .write_image(&rgba, width, height, image::ColorType::Rgba8.into())
                    .unwrap();
                std::fs::write(output.join(format!("{stem}-{label}.svg.png")), png).unwrap();
            }
            cache.commit(evaluation.transaction);
        }
    }
}
