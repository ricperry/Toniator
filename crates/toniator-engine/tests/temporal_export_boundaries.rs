//! Current final-callback cancellation contracts with real media tools and immutable saved authority.
use std::{
    fs,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use toniator_domain::{
    CalibrationBinding, CanvasSpec, Document, DocumentHistory, DocumentSession, FrameRange,
    FrameRate, ProjectTiming, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    EvaluationLimits, MediaTools, OutputRasterTarget, RasterAntialiasing, RasterBackground,
    export::{
        ExportPhase, SequenceExportJob, SequenceExportOptions,
        video::{VideoCodec, VideoExportJob, VideoExportOptions, VideoPhase},
    },
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle,
    sequence::{SequenceFormat, SequenceManifest},
};

/// Selects a fresh explicit gate root or process-local scratch without touching earlier evidence.
/// # Panics
/// Panics when owned scratch cannot be created or resolved to its canonical test path.
fn root(test: &str) -> PathBuf {
    let base = std::env::var_os("TONIATOR_B4_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/validation/test-scratch")
                .join(format!("temporal-boundaries-{}", std::process::id()))
        });
    let root = base.join(test);
    fs::create_dir_all(&root).unwrap();
    fs::canonicalize(root).unwrap()
}

/// Constructs current natural-size authority from immutable artwork and its genuine bundled pattern.
/// Pattern application attaches the compatible saved artifact; no fitting, relabel or ignored target
/// input participates. Only fresh output timing changes to one frame and saved calibration stays exact.
/// # Panics
/// Panics on unknown input labels, source I/O, current constructor/preset/authority mismatch or save failure.
fn fixture(label: &str, out: &std::path::Path) -> (Document, SourceBundle) {
    let (asset, width, height, format) = match label {
        "raster" => (
            "raster-sample.png",
            1024.0,
            1024.0,
            EmbeddedSourceFormat::Png,
        ),
        "vector" => ("vector-sample.svg", 900.0, 620.0, EmbeddedSourceFormat::Svg),
        _ => panic!("unknown immutable source"),
    };
    let bytes = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(asset),
    )
    .unwrap();
    let source_id = SourceReferenceId::new("sampled-observer-immutable").unwrap();
    let base = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(source_id.clone()),
    )
    .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(base).unwrap());
    let registry = toniator_patterns::PresetRegistry::bundled();
    registry
        .apply_to_document_base(&mut history, "straight-grid-circles")
        .unwrap();
    let artifact = registry
        .find("straight-grid-circles")
        .unwrap()
        .saved_calibration
        .as_ref()
        .unwrap();
    let before = history.document();
    before.require_saved_calibration_authority().unwrap();
    for channel in before.channel_topology().unwrap().channels() {
        assert!(
            artifact
                .binding()
                .is_compatible_with(&CalibrationBinding::for_channel(before, channel.id).unwrap())
        );
        assert_eq!(
            before.saved_calibration_for(channel.id).unwrap(),
            Some(artifact)
        );
    }
    assert!(
        artifact
            .binding()
            .is_compatible_with(&before.baseline_calibration_binding().unwrap())
    );
    assert_eq!(before.baseline_saved_calibration().unwrap(), Some(artifact));
    let document = before
        .clone()
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(6, 1).unwrap(),
                FrameRange::new(0, 1).unwrap(),
            ),
            before.temporal_end_overrides().to_vec(),
        )
        .unwrap();
    document.require_saved_calibration_authority().unwrap();
    assert_eq!(document.saved_calibrations(), before.saved_calibrations());
    assert_eq!(
        document.baseline_saved_calibration().unwrap(),
        before.baseline_saved_calibration().unwrap()
    );
    let sources =
        SourceBundle::new([
            EmbeddedSource::new(source_id.clone(), format, bytes.clone(), None).unwrap(),
        ])
        .unwrap();
    assert_eq!(sources.get(&source_id).unwrap().bytes(), bytes);
    fs::create_dir_all(out).unwrap();
    toniator_io::save(&out.join("fixture.toniator"), &document, &sources).unwrap();
    (document, sources)
}

/// Reads the actual consumer manifest; completion truth never comes from a frontend status string.
/// # Panics
/// Panics on malformed/missing fresh sequence metadata.
fn manifest(path: &std::path::Path) -> SequenceManifest {
    serde_json::from_slice(&fs::read(path.join("manifest.json")).unwrap()).unwrap()
}

/// Cancels synchronously inside Finalizing, retaining an incomplete sequence and emitting no Complete.
/// # Panics
/// Panics if either immutable-input job publishes completion after its final callback cancels.
#[test]
fn finalizing_callback_cancellation_keeps_sequence_incomplete() {
    let root = root("sequence-finalizing");
    let mut failures = Vec::new();
    for label in ["raster", "vector"] {
        let folder = root.join(label);
        let (document, sources) = fixture(label, &folder);
        let destination = folder.join("cancelled-sequence");
        let job = SequenceExportJob::new(
            document.clone(),
            sources,
            SequenceExportOptions {
                destination: destination.clone(),
                format: SequenceFormat::Png,
                background: Some(RasterBackground::Transparent),
                target: Some(OutputRasterTarget::new(8, 8).unwrap()),
                antialiasing: RasterAntialiasing::On,
                limits: EvaluationLimits::default(),
            },
        )
        .unwrap();
        let cancelled = AtomicBool::new(false);
        let events = Mutex::new(Vec::new());
        let result = job.run(MediaTools::default(), &cancelled, &|event| {
            events.lock().unwrap().push(event);
            if event.phase == ExportPhase::Finalizing {
                cancelled.store(true, Ordering::Release);
            }
        });
        let saved = manifest(&destination);
        let events = events.into_inner().unwrap();
        let correct = result
            .as_ref()
            .is_err_and(|e| e.is_cancelled() && e.output.as_deref() == Some(destination.as_path()))
            && !saved.complete
            && saved.completed_frames == 1
            && !events.iter().any(|e| e.phase == ExportPhase::Complete);
        fs::write(
            folder.join("result.txt"),
            format!(
                "result={result:?}\nmanifest={saved:?}\nevents={events:?}\ncorrect={correct}\n"
            ),
        )
        .unwrap();
        if !correct {
            failures.push(label);
        }
        assert_eq!(document.project_timing().frame_range().frame_count(), 1);
    }
    assert!(
        failures.is_empty(),
        "cancelled callback still finalized: {failures:?}"
    );
}

/// Cancels the final SavingPngs callback, preserves original PNGs and retries only a fresh destination.
/// Real MediaTools capability checking precedes the generated one-frame8x8 recovery sequence.
/// # Panics
/// Panics on false completion, discarded recovery or changed successful retry bytes/ownership.
#[test]
fn final_saving_callback_preserves_recovery_and_fresh_retry() {
    let root = root("recovery-final-saving");
    let mut failures = Vec::new();
    for label in ["raster", "vector"] {
        let folder = root.join(label);
        let (document, sources) = fixture(label, &folder);
        let temporary = folder.join("temporary");
        fs::create_dir(&temporary).unwrap();
        let job = VideoExportJob::new(
            document,
            sources,
            VideoExportOptions {
                destination: folder.join("reserved-video.mkv"),
                codec: VideoCodec::Ffv1Matroska,
                temporary_directory: Some(temporary),
                background: Some(RasterBackground::Transparent),
                target: Some(OutputRasterTarget::new(8, 8).unwrap()),
                antialiasing: RasterAntialiasing::On,
                limits: EvaluationLimits::default(),
            },
        )
        .unwrap();
        let cancelled = AtomicBool::new(false);
        let mut recovery = job
            .render_frames(MediaTools::default(), &cancelled, &|_| {})
            .unwrap();
        let originals = recovery.frames_path().unwrap();
        let original_manifest = fs::read(originals.join("manifest.json")).unwrap();
        let png = fs::read(originals.join("frame-000000.png")).unwrap();
        fs::write(folder.join("original-manifest.json"), &original_manifest).unwrap();
        fs::write(folder.join("original-frame.png"), &png).unwrap();
        let destination = folder.join("cancelled-copy");
        let events = Mutex::new(Vec::new());
        let result = recovery.save_png_sequence(&destination, &cancelled, &|event| {
            events.lock().unwrap().push(event);
            if event.phase == VideoPhase::SavingPngs && event.completed_frames == event.total_frames
            {
                cancelled.store(true, Ordering::Release);
            }
        });
        let saved = manifest(&destination);
        let originals_retained = originals.exists()
            && fs::read(originals.join("manifest.json")).ok().as_deref()
                == Some(original_manifest.as_slice())
            && fs::read(originals.join("frame-000000.png")).ok().as_deref() == Some(png.as_slice());
        let correct = result
            .as_ref()
            .is_err_and(|e| e.is_cancelled() && e.output.as_deref() == Some(destination.as_path()))
            && !saved.complete
            && originals_retained
            && recovery.frames_path().as_deref() == Some(originals.as_path());
        fs::write(folder.join("result.txt"),format!("result={result:?}\nmanifest={saved:?}\noriginals={originals:?}\noriginals_retained={originals_retained}\nevents={:?}\ncorrect={correct}\n",events.into_inner().unwrap())).unwrap();
        if !correct {
            failures.push(label);
            continue;
        }
        cancelled.store(false, Ordering::Release);
        let retry = folder.join("fresh-successful-copy");
        let saved = recovery
            .save_png_sequence(&retry, &cancelled, &|_| {})
            .unwrap();
        assert_eq!(saved.directory, retry);
        assert!(saved.publication_warning.is_none() && saved.cleanup_warning.is_none());
        assert!(manifest(&retry).complete);
        assert_eq!(fs::read(retry.join("frame-000000.png")).unwrap(), png);
        assert!(!originals.exists());
        assert!(recovery.frames_path().is_none());
        assert!(!manifest(&destination).complete);
    }
    assert!(
        failures.is_empty(),
        "final callback destroyed recovery: {failures:?}"
    );
}

/// Exercises real successful retry publication and cleanup after one saved-authority8x8 frame.
/// # Panics
/// Panics on false completion/path reporting, retained generated PNGs or unexpected warnings.
#[test]
fn healthy_video_retry_completes_and_releases_generated_frames() {
    let root = root("healthy-video-retry");
    for label in ["raster", "vector"] {
        let folder = root.join(label);
        let (document, sources) = fixture(label, &folder);
        let temporary = folder.join("temporary");
        fs::create_dir(&temporary).unwrap();
        let job = VideoExportJob::new(
            document,
            sources,
            VideoExportOptions {
                destination: folder.join("initial-reservation.mkv"),
                codec: VideoCodec::Ffv1Matroska,
                temporary_directory: Some(temporary),
                background: Some(RasterBackground::Transparent),
                target: Some(OutputRasterTarget::new(8, 8).unwrap()),
                antialiasing: RasterAntialiasing::On,
                limits: EvaluationLimits::default(),
            },
        )
        .unwrap();
        let cancelled = AtomicBool::new(false);
        let mut recovery = job
            .render_frames(MediaTools::default(), &cancelled, &|_| {})
            .unwrap();
        let originals = recovery.frames_path().unwrap();
        fs::write(
            folder.join("original-frame.png"),
            fs::read(originals.join("frame-000000.png")).unwrap(),
        )
        .unwrap();
        let destination = folder.join("published-video.mkv");
        let events = Mutex::new(Vec::new());
        let result = recovery
            .retry(MediaTools::default(), &destination, &cancelled, &|event| {
                events.lock().unwrap().push(event)
            })
            .unwrap();
        assert_eq!(result.file, destination);
        assert_eq!(result.frame_count, 1);
        assert_eq!((result.width, result.height), (8, 8));
        assert!(
            result.publication_warning.is_none()
                && result.cleanup_warning.is_none()
                && result.retained_intermediates.is_none()
        );
        assert!(!originals.exists() && recovery.frames_path().is_none() && result.file.exists());
        let events = events.into_inner().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|e| e.phase == VideoPhase::Complete)
                .count(),
            1
        );
        fs::write(
            folder.join("result.txt"),
            format!("{result:?}\nevents={events:?}\n"),
        )
        .unwrap();
    }
}

/// Preserves saved completion when owned workspace cleanup refuses a root sentinel after copying.
/// The controlled sentinel is inserted by the final synchronous callback, before discard runs.
/// # Panics
/// Panics if complete exact media is reported as failed or remaining recovery ownership is lost.
#[test]
fn completed_png_recovery_reports_cleanup_warning_without_failure() {
    let root = root("completed-png-cleanup");
    let mut failures = Vec::new();
    for label in ["raster", "vector"] {
        let folder = root.join(label);
        let (document, sources) = fixture(label, &folder);
        let temporary = folder.join("temporary");
        fs::create_dir(&temporary).unwrap();
        let job = VideoExportJob::new(
            document,
            sources,
            VideoExportOptions {
                destination: folder.join("unused-reservation.mkv"),
                codec: VideoCodec::Ffv1Matroska,
                temporary_directory: Some(temporary),
                background: Some(RasterBackground::Transparent),
                target: Some(OutputRasterTarget::new(8, 8).unwrap()),
                antialiasing: RasterAntialiasing::On,
                limits: EvaluationLimits::default(),
            },
        )
        .unwrap();
        let cancelled = AtomicBool::new(false);
        let mut recovery = job
            .render_frames(MediaTools::default(), &cancelled, &|_| {})
            .unwrap();
        let originals = recovery.frames_path().unwrap();
        let original_manifest = fs::read(originals.join("manifest.json")).unwrap();
        let png = fs::read(originals.join("frame-000000.png")).unwrap();
        let sentinel = originals.parent().unwrap().join("keep-note.txt");
        let destination = folder.join("completed-copy");
        let result = recovery.save_png_sequence(&destination, &cancelled, &|event| {
            if event.phase == VideoPhase::SavingPngs && event.completed_frames == event.total_frames
            {
                fs::write(&sentinel, b"owned cleanup refusal witness").unwrap();
            }
        });
        let saved = manifest(&destination);
        let originals_retained = fs::read(originals.join("manifest.json")).unwrap()
            == original_manifest
            && fs::read(originals.join("frame-000000.png")).unwrap() == png;
        assert!(saved.complete && saved.completed_frames == 1 && originals_retained);
        assert_eq!(fs::read(destination.join("frame-000000.png")).unwrap(), png);
        assert_eq!(
            fs::read(destination.join("manifest.json")).unwrap(),
            original_manifest
        );
        assert_eq!(recovery.frames_path().as_deref(), Some(originals.as_path()));
        assert!(sentinel.is_file());
        fs::write(folder.join("result.txt"),format!("result={result:?}\nmanifest={saved:?}\noriginals={originals:?}\noriginals_retained={originals_retained}\n" )).unwrap();
        match result {
            Ok(result) => {
                assert_eq!(result.directory, destination);
                assert!(result.publication_warning.is_none());
                assert!(
                    result
                        .cleanup_warning
                        .as_deref()
                        .is_some_and(|warning| warning.contains("cleanup refused"))
                );
            }
            Err(_) => failures.push(label),
        }
    }
    assert!(
        failures.is_empty(),
        "complete exact copy returned failure after cleanup refusal: {failures:?}"
    );
}


/// Reports healthy complete publication with its actual directory and no durability warning.
/// # Panics
/// Panics if saved authority/media changes or normal export omits truthful Complete publication.
#[test]
fn healthy_sequence_returns_complete_directory_without_warning() {
    let root = root("healthy-sequence");
    for label in ["raster", "vector"] {
        let folder = root.join(label);
        let (document, sources) = fixture(label, &folder);
        let destination = folder.join("sequence");
        let job = SequenceExportJob::new(
            document,
            sources,
            SequenceExportOptions {
                destination: destination.clone(),
                format: SequenceFormat::Png,
                background: Some(RasterBackground::Transparent),
                target: Some(OutputRasterTarget::new(8, 8).unwrap()),
                antialiasing: RasterAntialiasing::On,
                limits: EvaluationLimits::default(),
            },
        )
        .unwrap();
        let events = Mutex::new(Vec::new());
        let result = job
            .run(MediaTools::default(), &AtomicBool::new(false), &|event| {
                events.lock().unwrap().push(event)
            })
            .unwrap();
        assert_eq!(result.directory, destination);
        assert_eq!((result.frame_count, result.width, result.height), (1, 8, 8));
        assert!(result.publication_warning.is_none());
        assert!(manifest(&destination).complete);
        assert_eq!(
            events
                .lock()
                .unwrap()
                .iter()
                .filter(|event| event.phase == ExportPhase::Complete)
                .count(),
            1
        );
        fs::write(
            folder.join("result.txt"),
            format!("{result:?}\nevents={:?}\n", events.into_inner().unwrap()),
        )
        .unwrap();
    }
}
