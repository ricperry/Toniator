//! Captured engine scene consumers exercise immutable sources and atomic row-band PNG output.

use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use toniator_domain::{CanvasSpec, Document, DocumentSession, SourceReference, SourceReferenceId};
use toniator_engine::*;
use toniator_io::atomic_output::write_atomic_output;

/// Returns only this headless stage's native-output directory.
/// # Panics
/// Panics on stage-owned filesystem failures.
fn stage() -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018-png-streaming-20261013/engine");
    fs::create_dir_all(&directory).unwrap();
    directory
}

/// Evaluates each immutable source once at native geometry while retaining only a one-pixel raster.
/// # Panics
/// Panics on altered immutable inputs, captured-source evaluation, decoded parity, or atomic IO.
#[test]
fn both_immutable_sources_stream_native_scene_with_exact_rgba() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, width, height, hint, hash) in [
        (
            "raster-sample.png",
            1024,
            1024,
            SourceFormatHint::Png,
            "324ac232e319002a13fbcfac46538ca5d7e8ba8a127eea2eaf20e8ddb3ed2ef2",
        ),
        (
            "vector-sample.svg",
            900,
            620,
            SourceFormatHint::Svg,
            "42eb5e23111a5dbad66f2b1802a7cc06391c7ede829b99eb28aeb1ac91596e2e",
        ),
    ] {
        let bytes: Arc<[u8]> = fs::read(root.join("assets").join(name)).unwrap().into();
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), hash);
        let id = SourceReferenceId::new(format!("ton018-stream-{name}")).unwrap();
        let document = Document::new_default_document(
            CanvasSpec {
                width: f64::from(width),
                height: f64::from(height),
            },
            SourceReference::Assigned(id.clone()),
        )
        .unwrap();
        let original = document.clone();
        let session = DocumentSession::new(document).unwrap();
        let request = EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(id, bytes, hint).unwrap(),
        )
        .for_scene()
        .unwrap();
        let result = evaluate(request).unwrap();
        assert_eq!((result.raster().width(), result.raster().height()), (1, 1));
        assert_eq!(session.document(), &original);
        let target = StreamingRasterTarget::new(width, height).unwrap();
        let expected = rasterize_output(
            result.scene(),
            RasterBackground::Transparent,
            Some(OutputRasterTarget::new(width, height).unwrap()),
            RasterAntialiasing::On,
        )
        .unwrap();
        let output = stage().join(format!("{name}-streamed.png"));
        let summary = write_atomic_output(&output, &|| false, |file| {
            write_png_streaming(
                result.scene(),
                target,
                PngStreamOptions {
                    maximum_band_rows: 31,
                    ..Default::default()
                },
                toniator_render::RasterizationLimits::new(
                    EvaluationLimits::default().max_flattened_raster_edges(),
                )
                .unwrap(),
                file,
                &|| false,
                &|_, _| {},
            )
            .map_err(io::Error::other)
        })
        .unwrap();
        let decoded = image::open(&output).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (width, height));
        assert!(
            decoded.as_raw() == expected.pixels(),
            "native streamed pixels differ for {name}"
        );
        fs::write(
            stage().join(format!("{name}-summary.txt")),
            format!("{summary:#?}\n"),
        )
        .unwrap();
        let cancelled = AtomicBool::new(false);
        let prior = fs::read(&output).unwrap();
        let error = write_atomic_output(&output, &|| cancelled.load(Ordering::Acquire), |file| {
            write_png_streaming(
                result.scene(),
                target,
                PngStreamOptions {
                    maximum_band_rows: 31,
                    ..Default::default()
                },
                toniator_render::RasterizationLimits::new(
                    EvaluationLimits::default().max_flattened_raster_edges(),
                )
                .unwrap(),
                file,
                &|| cancelled.load(Ordering::Acquire),
                &|rows, _| {
                    if rows > 0 {
                        cancelled.store(true, Ordering::Release);
                    }
                },
            )
            .map_err(io::Error::other)
        })
        .unwrap_err();
        assert!(!error.published());
        assert_eq!(
            fs::read(&output).unwrap(),
            prior,
            "cancelled stream preserves approved complete PNG"
        );
    }
}

/// Streams the retained calibrated photo project at the user's requested exact 8x dimensions.
///
/// The project and embedded original source remain untouched. The complete frame is materialized
/// once through its existing saved-calibration authority; no fitting or new assignments occur.
/// # Panics
/// Panics on missing retained project, changed canvas or saved assignments, or output failure.
#[test]
#[ignore = "explicit calibrated-photo actual-size acceptance witness"]
fn retained_photo_project_streams_exact_16384_by_9312() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let loaded = toniator_io::load(&root.join("target/validation/ton018-builtin-document-presets-20261013/photo-r30-g60-b0-feature025-current.toniator")).unwrap();
    let document = loaded.document().clone();
    assert_eq!(
        document.canvas(),
        &CanvasSpec {
            width: 2048.0,
            height: 1164.0
        }
    );
    let saved = document.saved_calibrations().to_vec();
    let session = DocumentSession::new(document).unwrap();
    let cancelled = AtomicBool::new(false);
    let mut media = open_source_media(loaded.sources(), MediaTools::default(), &|| false).unwrap();
    let frame = session.document().project_timing().frame_range().start();
    let request = frame_evaluation_request(&session, &mut media, frame, &|| false)
        .unwrap()
        .for_scene()
        .unwrap();
    let result =
        evaluate_cancellable_with_limits(request, EvaluationLimits::default(), &cancelled).unwrap();
    assert_eq!(session.document().saved_calibrations(), saved);
    let target = StreamingRasterTarget::new(16_384, 9_312).unwrap();
    let output = stage().join("photo-r30-g60-b0-feature025-16384x9312.png");
    let summary = write_atomic_output(&output, &|| false, |file| {
        write_png_streaming(
            result.scene(),
            target,
            PngStreamOptions {
                background: RasterBackground::OpaqueBlack,
                pixels_per_metre: Some(11_811),
                ..Default::default()
            },
            toniator_render::RasterizationLimits::new(
                EvaluationLimits::default().max_flattened_raster_edges(),
            )
            .unwrap(),
            file,
            &|| false,
            &|_, _| {},
        )
        .map_err(io::Error::other)
    })
    .unwrap();
    fs::write(
        stage().join("photo-actual-summary.txt"),
        format!(
            "{summary:#?}\ncanonical primitives={}\n",
            result.scene().circular_mark_count()
        ),
    )
    .unwrap();
}

/// Compares six bounded retained family projects against exact prior native PNG references.
///
/// Existing project, source, calibration, and reference files are read-only. The retained runner uses
/// ordinary model defaults; actual reference alpha confirms RGB black and CMYK white backing.
/// Only this stage receives newly streamed PNGs.
/// # Panics
/// Panics on changed native pixels, unsupported current artifacts, or failed stage-owned IO.
#[test]
#[ignore = "explicit bounded prior-artifact parity witness"]
fn retained_curve_cells_and_maze_match_prior_native_pngs() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let projects = root.join("target/validation/ton018-family-regression-20261013/native-projects");
    let references = root.join("target/validation/ton018-checked-boolean-20261013/renders");
    let filter = std::env::var("TON018_STREAM_REFERENCE_FILTER").ok();
    let mut report = if filter.is_some() {
        fs::read_to_string(stage().join("prior-reference-parity.tsv")).unwrap_or_default()
    } else {
        String::new()
    };
    let mut checked = 0;
    for name in [
        "one-guide-lines-vector",
        "three-guide-cells-scale-raster",
        "two-guide-maze-vector",
    ] {
        for feature in ["f1", "f025"] {
            let stem = format!("{name}-{feature}");
            if filter.as_deref().is_some_and(|selected| selected != stem) {
                continue;
            }
            checked += 1;
            let loaded = toniator_io::load(&projects.join(format!("{stem}.toniator"))).unwrap();
            let session = DocumentSession::new(loaded.document().clone()).unwrap();
            let mut media =
                open_source_media(loaded.sources(), MediaTools::default(), &|| false).unwrap();
            let frame = session.document().project_timing().frame_range().start();
            let request = frame_evaluation_request(&session, &mut media, frame, &|| false)
                .unwrap()
                .for_scene()
                .unwrap();
            let result = evaluate(request).unwrap();
            let background =
                RasterBackground::default_for_model(session.document().channel_model());
            let target = StreamingRasterTarget::for_canvas(session.document().canvas()).unwrap();
            let prior = image::open(references.join(format!("{stem}.png")))
                .unwrap()
                .to_rgba8();
            let full = toniator_render::rasterize_output_cancellable_with_progress(
                result.scene(),
                background,
                Some(OutputRasterTarget::new(target.width(), target.height()).unwrap()),
                RasterAntialiasing::On,
                RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())
                    .unwrap(),
                &|| false,
                &|_, _| {},
            )
            .unwrap();
            assert!(
                full.pixels() == prior.as_raw(),
                "current full consumer differs from prior reference: {stem}"
            );
            let output = stage().join(format!("reference-{stem}.png"));
            write_atomic_output(&output, &|| false, |file| {
                write_png_streaming(
                    result.scene(),
                    target,
                    PngStreamOptions {
                        background,
                        maximum_band_rows: 31,
                        ..Default::default()
                    },
                    toniator_render::RasterizationLimits::new(
                        EvaluationLimits::default().max_flattened_raster_edges(),
                    )
                    .unwrap(),
                    file,
                    &|| false,
                    &|_, _| {},
                )
                .map_err(io::Error::other)
            })
            .unwrap();
            let actual = image::open(&output).unwrap().to_rgba8();
            assert_eq!(actual.dimensions(), prior.dimensions(), "{stem}");
            assert!(
                actual.as_raw() == prior.as_raw(),
                "native reference changed: {stem}"
            );
            report.push_str(&format!("{stem}\texact decoded RGBA equality\n"));
            fs::write(stage().join("prior-reference-parity.tsv"), &report).unwrap();
        }
    }
    assert!(
        checked > 0,
        "reference filter must select an existing bounded case"
    );
}
