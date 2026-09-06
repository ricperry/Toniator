use super::*;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};
use toniator_domain::{
    CanvasSpec, ChannelId, Easing, FrameRange, FrameRate, ProjectTiming, PropertyFieldId,
    PropertyTarget, SourceReferenceId, TemporalEndpointEdit,
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat};

/// Creates an exclusive scratch root and keeps native validation artifacts when requested.
struct Scratch {
    path: PathBuf,
    keep: bool,
}
impl Scratch {
    /// Allocates this test's own root without replacing any existing validation artifacts.
    ///
    /// # Panics
    /// Panics when test time or writable storage is unavailable.
    fn new(keep: bool) -> Self {
        let base = if keep {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/validation/stage22-sequence-export")
        } else {
            std::env::temp_dir()
        };
        fs::create_dir_all(&base).unwrap();
        let path = base.join(format!(
            "run-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self { path, keep }
    }
}
impl Drop for Scratch {
    /// Removes only private temporary test roots; intrinsic review artifacts remain available.
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Builds a current source-backed animation snapshot from one immutable input fixture.
///
/// # Panics
/// Panics if current domain or source constructors reject the known fixture.
pub(super) fn fixture(
    format: EmbeddedSourceFormat,
    bytes: &[u8],
    width: u32,
    height: u32,
    frames: u64,
) -> (Document, SourceBundle) {
    let id = SourceReferenceId::new("export-source").unwrap();
    let document = Document::new_default_document(
        CanvasSpec {
            width: width.into(),
            height: height.into(),
        },
        SourceReference::Assigned(id.clone()),
    )
    .unwrap();
    let command = document
        .edit_effective_end_command(
            &(1..=3)
                .map(|id| TemporalEndpointEdit {
                    target: PropertyTarget::Channel(ChannelId(id)),
                    field: PropertyFieldId::Opacity,
                    effective_end: 0.25,
                    easing: Easing::Linear,
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let document = document
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(6, 1).unwrap(),
                FrameRange::new(0, frames).unwrap(),
            ),
            command.replacement().end_overrides.clone(),
        )
        .unwrap();
    let sources =
        SourceBundle::new([EmbeddedSource::new(id, format, bytes, None).unwrap()]).unwrap();
    (document, sources)
}

/// Supplies transparent native output so PNG and SVG preserve authored alpha for inspection.
fn options(path: PathBuf, format: SequenceFormat) -> SequenceExportOptions {
    SequenceExportOptions {
        destination: path,
        format,
        background: Some(RasterBackground::Transparent),
        target: None,
        antialiasing: RasterAntialiasing::On,
        limits: EvaluationLimits::default(),
    }
}

/// Verifies intrinsic PNG/SVG sequences and the supplied video's complete ten-frame source order.
///
/// # Panics
/// Panics if native output, frame order, completion progress, or manifest timing fails.
#[test]
fn shared_sequence_job_exports_native_stills_and_ten_video_frames() {
    let scratch = Scratch::new(true);
    for (name, source_format, bytes, width, height, frames, output_format) in [
        (
            "raster",
            EmbeddedSourceFormat::Png,
            include_bytes!("../../../assets/raster-sample.png").as_slice(),
            1024,
            1024,
            2,
            SequenceFormat::Png,
        ),
        (
            "vector",
            EmbeddedSourceFormat::Svg,
            include_bytes!("../../../assets/vector-sample.svg").as_slice(),
            900,
            620,
            2,
            SequenceFormat::Svg,
        ),
        (
            "video",
            EmbeddedSourceFormat::Video,
            include_bytes!("../../../assets/video-sample0001-0010.mp4").as_slice(),
            48,
            48,
            10,
            SequenceFormat::Png,
        ),
    ] {
        let (document, sources) = fixture(source_format, bytes, width, height, frames);
        let job = SequenceExportJob::new(
            document.clone(),
            sources,
            options(scratch.path.join(name), output_format),
        )
        .unwrap();
        let progress = Mutex::new(Vec::new());
        let result = job
            .run(MediaTools::default(), &AtomicBool::new(false), &|event| {
                progress.lock().unwrap().push(event)
            })
            .unwrap();
        assert_eq!(result.frame_count, frames);
        assert_eq!(job.document, document);
        let manifest: SequenceManifest =
            serde_json::from_slice(&fs::read(result.directory.join("manifest.json")).unwrap())
                .unwrap();
        assert!(manifest.complete);
        assert_eq!(manifest.completed_frames, frames);
        assert_eq!((manifest.width, manifest.height), (width, height));
        let first = fs::read(
            result
                .directory
                .join(format!("frame-000000.{}", output_format.extension())),
        )
        .unwrap();
        let last = fs::read(result.directory.join(format!(
            "frame-{:06}.{}",
            frames - 1,
            output_format.extension()
        )))
        .unwrap();
        assert_ne!(first, last);
        if output_format == SequenceFormat::Png {
            assert_eq!(
                image::load_from_memory(&first)
                    .unwrap()
                    .to_rgba8()
                    .dimensions(),
                (width, height)
            );
        } else {
            let text = String::from_utf8(first).unwrap();
            assert!(text.contains("width=\"900\""));
            assert!(text.contains("height=\"620\""));
            svg_review_png(&result.directory.join("frame-000000.svg"));
            svg_review_png(&result.directory.join("frame-000001.svg"));
        }
        let events = progress.lock().unwrap();
        assert_eq!(events.first().unwrap().phase, ExportPhase::Preflight);
        assert_eq!(events.last().unwrap().phase, ExportPhase::Complete);
        assert_eq!(events.last().unwrap().completed_frames, frames);
        assert!(
            events
                .iter()
                .all(|event| (0.0..=1.0).contains(&event.frame_fraction))
        );
    }
    println!("native sequence evidence: {}", scratch.path.display());
}

/// Creates a labeled unflattened SVG rasterization solely for intrinsic artifact inspection.
///
/// # Panics
/// Panics when generated SVG cannot be decoded through the existing SVG sampling authority.
fn svg_review_png(path: &Path) {
    use image::ImageEncoder;
    let field =
        toniator_sampling::decode_source(&fs::read(path).unwrap(), crate::SourceFormatHint::Svg)
            .unwrap();
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
    fs::write(path.with_extension("svg.review.png"), png).unwrap();
}

/// Checks that output-only background/size/antialiasing affect the raster cache and preserve scene authority.
///
/// # Panics
/// Panics when consumer selection changes geometry, reuses wrong pixels, or diverges from rendering.
#[test]
fn output_consumer_reuses_geometry_and_keys_final_raster_policy() {
    let (document, sources) = fixture(
        EmbeddedSourceFormat::Png,
        include_bytes!("../../../assets/raster-sample.png"),
        48,
        48,
        2,
    );
    let session = DocumentSession::new(document).unwrap();
    let mut media = open_source_media(&sources, MediaTools::default(), &|| false).unwrap();
    let base = frame_evaluation_request(&session, &mut media, 0, &|| false).unwrap();
    let mut cache = DocumentDerivedCache::default();
    let first = evaluate_cached_document(
        base.clone(),
        EvaluationLimits::default(),
        &cache,
        &crate::NeverCancelled,
    )
    .unwrap();
    let original_scene = first.result.scene().clone();
    cache.commit(first.transaction);
    let target = OutputRasterTarget::new(64, 32).unwrap();
    let request = base.for_output(
        RasterBackground::OpaqueWhite,
        Some(target),
        RasterAntialiasing::Off,
    );
    let second = evaluate_cached_document(
        request.clone(),
        EvaluationLimits::default(),
        &cache,
        &crate::NeverCancelled,
    )
    .unwrap();
    assert_eq!(second.result.scene(), &original_scene);
    assert_eq!(
        second.diagnostics.aggregate.scene,
        crate::CacheDisposition::Hit
    );
    assert_eq!(
        second.diagnostics.aggregate.raster,
        crate::CacheDisposition::Miss
    );
    let reference = crate::rasterize_output(
        &original_scene,
        RasterBackground::OpaqueWhite,
        Some(target),
        RasterAntialiasing::Off,
    )
    .unwrap();
    assert_eq!(second.result.raster(), &reference);
    cache.commit(second.transaction);
    let repeated = evaluate_cached_document(
        request,
        EvaluationLimits::default(),
        &cache,
        &crate::NeverCancelled,
    )
    .unwrap();
    assert_eq!(
        repeated.diagnostics.aggregate.raster,
        crate::CacheDisposition::Hit
    );
}

/// Rejects invalid intermediate response crossings before creating any output directory.
///
/// # Panics
/// Panics if the valid endpoint pair or required interior-frame rejection differs from the domain.
#[test]
fn sequence_preflight_rejects_interior_crossings_before_publication() {
    let scratch = Scratch::new(false);
    let (document, sources) = fixture(
        EmbeddedSourceFormat::Png,
        include_bytes!("../../../assets/raster-sample.png"),
        48,
        48,
        5,
    );
    let output = document.pattern_definition_bundles()[0].output_settings()[0].output_layer_id;
    let command = document
        .edit_effective_end_command(&[
            TemporalEndpointEdit {
                target: PropertyTarget::ChannelOutput(ChannelId(1), output),
                field: PropertyFieldId::MarkMinimumFill,
                effective_end: 1.5,
                easing: Easing::Linear,
            },
            TemporalEndpointEdit {
                target: PropertyTarget::ChannelOutput(ChannelId(1), output),
                field: PropertyFieldId::MarkMaximumFill,
                effective_end: 1.6,
                easing: Easing::Hold,
            },
        ])
        .unwrap();
    let timing = document.project_timing().clone();
    let document = document
        .with_temporal_authority(timing, command.replacement().end_overrides.clone())
        .unwrap();
    let path = scratch.path.join("invalid");
    let job = SequenceExportJob::new(
        document,
        sources,
        options(path.clone(), SequenceFormat::Png),
    )
    .unwrap();
    let failure = job
        .run(MediaTools::default(), &AtomicBool::new(false), &|_| {})
        .unwrap_err();
    assert_eq!(failure.stage, "export.frame");
    assert_eq!(failure.frame, Some(3));
    assert!(!path.exists());
}

/// Preserves completed frames with an incomplete manifest and rejects finite-source overrun.
///
/// # Panics
/// Panics if cancellation publishes completion or a bad source interval creates output.
#[test]
fn sequence_cancel_retains_truthful_partial_and_range_errors_are_preflighted() {
    let scratch = Scratch::new(false);
    let (document, sources) = fixture(
        EmbeddedSourceFormat::Png,
        include_bytes!("../../../assets/raster-sample.png"),
        48,
        48,
        3,
    );
    let path = scratch.path.join("cancelled");
    let job = SequenceExportJob::new(
        document,
        sources,
        options(path.clone(), SequenceFormat::Png),
    )
    .unwrap();
    let cancelled = AtomicBool::new(false);
    let failure = job
        .run(MediaTools::default(), &cancelled, &|event| {
            if event.completed_frames == 1 {
                cancelled.store(true, Ordering::Release);
            }
        })
        .unwrap_err();
    assert!(failure.is_cancelled());
    assert_eq!(failure.output, Some(path.clone()));
    let manifest: SequenceManifest =
        serde_json::from_slice(&fs::read(path.join("manifest.json")).unwrap()).unwrap();
    assert!(!manifest.complete);
    assert_eq!(manifest.completed_frames, 1);
    assert!(path.join("frame-000000.png").exists());
    assert!(!path.join("frame-000001.png").exists());
    let (document, sources) = fixture(
        EmbeddedSourceFormat::Video,
        include_bytes!("../../../assets/video-sample0001-0010.mp4"),
        48,
        48,
        11,
    );
    let path = scratch.path.join("overrun");
    let job = SequenceExportJob::new(
        document,
        sources,
        options(path.clone(), SequenceFormat::Png),
    )
    .unwrap();
    let failure = job
        .run(MediaTools::default(), &AtomicBool::new(false), &|_| {})
        .unwrap_err();
    assert_eq!(failure.stage, "export.range");
    assert_eq!(failure.frame, Some(10));
    assert!(!path.exists());
}
