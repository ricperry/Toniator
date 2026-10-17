//! Opt-in bounded performance measurements through current canonical headless APIs.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use toniator_domain::*;
use toniator_engine::*;
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Hashes already-produced bytes without changing native output representation.
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Emits one independently parseable diagnostic record; timing never enters product state.
fn emit(value: Value) {
    println!("{value}");
}

/// Builds a current document from the immutable source and a current owned recipe.
///
/// # Errors
/// Returns source, current-format binding, or validation errors; writes no inputs.
fn fixture(source: &str, recipe: &str) -> Result<(Document, SourceBundle, ResolvedSource)> {
    let (model, recipe) = if let Some(recipe) = recipe.strip_prefix("cmyk:") {
        (Some(HalftoneChannelModel::Cmyk), recipe)
    } else if let Some(recipe) = recipe.strip_prefix("sca:") {
        (Some(HalftoneChannelModel::SourceColorAlpha), recipe)
    } else {
        (None, recipe)
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (file, w, h, format, hint) = match source {
        "png" => (
            "raster-sample.png",
            1024,
            1024,
            EmbeddedSourceFormat::Png,
            SourceFormatHint::Png,
        ),
        "svg" => (
            "vector-sample.svg",
            900,
            620,
            EmbeddedSourceFormat::Svg,
            SourceFormatHint::Svg,
        ),
        _ => return Err("source must be png or svg".into()),
    };
    let id = SourceReferenceId::new(format!("performance-{source}"))?;
    let bytes = fs::read(root.join("assets").join(file))?;
    let destination = Document::new_default_document(
        CanvasSpec {
            width: w as f64,
            height: h as f64,
        },
        SourceReference::Assigned(id.clone()),
    )?;
    let document = if let Some(name) = recipe.strip_prefix("fixture:") {
        let source_name = if source == "png" { "raster" } else { "vector" };
        let loaded = toniator_io::load(&root.join(format!("target/validation/ton018-family-regression-20261013/native-projects/{name}-{source_name}-f025.toniator")))?;
        DocumentConfiguration::capture(loaded.document()).bind(&destination)?
    } else if recipe == "default" || recipe == "fine" || recipe == "tiny" {
        let density = if recipe == "fine" {
            400.0
        } else if recipe == "tiny" {
            12.5
        } else {
            100.0
        };
        destination
            .edit_all_channel_start_configuration(&[(PropertyFieldId::Density, density)])?
            .bind(&destination)?
    } else {
        toniator_io::load_document_preset(
            &root.join(format!("assets/document-presets/{recipe}.toniator-preset")),
            &destination,
        )?
        .bind(&destination)?
    };
    let document = if let Some(model) = model {
        let template = ChannelTopologyTemplate {
            pattern_instance: document
                .channel_topology()
                .ok_or("missing topology")?
                .channels()[0]
                .pattern_instance
                .clone(),
        };
        let topology = document.canonical_channel_topology(model, template)?;
        let mut history = DocumentHistory::new(DocumentSession::new(document)?);
        history.apply(&DocumentCommand::ReplaceChannelTopology { model, topology })?;
        history.document().clone()
    } else {
        document
    };
    document.require_saved_calibration_authority()?;
    let bundle = SourceBundle::new([EmbeddedSource::new(
        id.clone(),
        format,
        bytes.clone(),
        Some(file.into()),
    )?])?;
    Ok((document, bundle, ResolvedSource::new(id, bytes, hint)?))
}

/// Selects a checked diagnostic Fit extent/backing without changing document geometry.
///
/// Absent P1 options retain the P0 native transparent target. P1's explicit viewport
/// extent defaults to black, matching the recorded RGB GUI presentation.
///
/// # Errors
/// Returns malformed dimensions/background or the existing renderer target admission error.
fn requested_preview_target(canvas: &CanvasSpec) -> Result<PreviewRasterTarget> {
    let dimensions = env::var("P1_PREVIEW_TARGET").ok();
    let (width, height) = if let Some(value) = &dimensions {
        let (width, height) = value
            .split_once('x')
            .ok_or("preview extent must be WIDTHxHEIGHT")?;
        (width.parse()?, height.parse()?)
    } else {
        (canvas.width as u32, canvas.height as u32)
    };
    let default = if dimensions.is_some() {
        "black"
    } else {
        "transparent"
    };
    let background = match env::var("P1_PREVIEW_BACKGROUND")
        .as_deref()
        .unwrap_or(default)
    {
        "black" => RasterBackground::OpaqueBlack,
        "white" => RasterBackground::OpaqueWhite,
        "transparent" => RasterBackground::Transparent,
        _ => return Err("preview backing must be black, white or transparent".into()),
    };
    Ok(PreviewRasterTarget::new(width, height)?.with_background(background))
}

/// Projects existing opt-in phase metrics and exact scene/raster fingerprints.
///
/// The projection performs no geometry conversion and preserves record order.
fn metrics(label: &str, repeat: usize, profile: &ProfiledEvaluation) {
    let records: Vec<_> = profile.performance.records.iter().map(|r| json!({"stage":format!("{:?}",r.stage),"channel":r.channel_id.map(|x|x.0),"output":r.output_layer_id.map(|x|x.0),"us":r.elapsed.as_micros(),"cache":format!("{:?}",r.cache),"execution":format!("{:?}",r.execution),"workloads":r.workloads.iter().map(|w| json!({"kind":format!("{:?}",w.kind),"count":w.count})).collect::<Vec<_>>()})).collect();
    emit(
        json!({"kind":"evaluation","label":label,"repeat":repeat,"workers":profile.performance.configured_worker_count,"observed_workers":profile.performance.observed_worker_count,"records":records,"scene":profile.result.scene().identity().scene_fingerprint(),"family":profile.result.scene().identity().family_fingerprint(),"realization":profile.result.scene().identity().realization_fingerprint(),"rgba_sha256":hash(profile.result.raster().pixels())}),
    );
}

/// Captures exact native PNG/SVG artifacts only after successful canonical evaluation.
///
/// # Errors
/// Returns filesystem or encoding errors. SVG diagnostics are retained because some current
/// authored contours legitimately fail the existing serializer's independent bounds.
fn artifacts(directory: &Path, stem: &str, result: &EvaluationResult) -> Result<()> {
    let start = Instant::now();
    let png = encode_png(result.raster())?;
    emit(
        json!({"kind":"export","label":"png_encode","us":start.elapsed().as_micros(),"bytes":png.len(),"sha256":hash(&png)}),
    );
    fs::write(directory.join(format!("{stem}.png")), png)?;
    let start = Instant::now();
    match write_svg(result.scene()) {
        Ok(svg) => {
            emit(
                json!({"kind":"export","label":"svg","us":start.elapsed().as_micros(),"bytes":svg.len(),"sha256":hash(svg.as_bytes())}),
            );
            fs::write(directory.join(format!("{stem}.svg")), svg)?;
        }
        Err(e) => {
            emit(
                json!({"kind":"export_error","label":"svg","error":e.to_string(),"us":start.elapsed().as_micros()}),
            );
            fs::write(
                directory.join(format!("{stem}-svg-error.txt")),
                e.to_string(),
            )?;
        }
    }
    Ok(())
}

/// Runs isolated repeated cold/warm evaluations and canonical small/native/fine previews.
///
/// # Errors
/// Returns canonical evaluation, export, or filesystem errors without changing document inputs.
///
/// # Panics
/// Panics when repeated, accepted-cache or resized-preview scenes violate exact equivalence.
fn evaluate_case(
    document: Document,
    source: ResolvedSource,
    directory: &Path,
    repeats: usize,
    previews: bool,
    native_preview: bool,
) -> Result<()> {
    let canvas = document.canvas().clone();
    let session = DocumentSession::new(document)?;
    let mut request = EvaluationRequest::new(session.document_evaluation_snapshot(), source);
    if native_preview {
        request = request.for_preview(requested_preview_target(&canvas)?);
    }
    let mut expected = None;
    for repeat in 0..repeats {
        let mut cache = EvaluationProfileCache::default();
        let cold = evaluate_profiled_cached_with_limits(
            request.clone(),
            EvaluationLimits::default(),
            &mut cache,
        )?;
        let identity = (
            cold.result.scene().clone(),
            hash(cold.result.raster().pixels()),
        );
        if let Some(ref previous) = expected {
            assert_eq!(previous, &identity, "repeated exact output contract");
        } else {
            expected = Some(identity);
        }
        metrics("cold", repeat, &cold);
        artifacts(directory, &format!("native-{repeat}"), &cold.result)?;
        let warm = evaluate_profiled_cached_with_limits(
            request.clone(),
            EvaluationLimits::default(),
            &mut cache,
        )?;
        assert_eq!(
            warm.result.scene(),
            cold.result.scene(),
            "warm canonical equality"
        );
        assert_eq!(
            warm.result.raster(),
            cold.result.raster(),
            "warm exact RGBA equality"
        );
        metrics("warm", repeat, &warm);
        for (label, scale) in [
            ("small", 256.0 / canvas.width.max(canvas.height)),
            ("native", 1.0),
            ("fine2x", 2.0),
        ] {
            if !previews {
                break;
            }
            let target = PreviewRasterTarget::new(
                (canvas.width * scale).round().max(1.0) as u32,
                (canvas.height * scale).round().max(1.0) as u32,
            )?;
            let profile = evaluate_profiled_cached_with_limits(
                request.clone().for_preview(target),
                EvaluationLimits::default(),
                &mut cache,
            )?;
            assert_eq!(
                profile.result.scene(),
                cold.result.scene(),
                "preview canonical equality"
            );
            metrics(label, repeat, &profile);
            if repeat == 0 {
                fs::write(
                    directory.join(format!("preview-{label}.png")),
                    encode_png(profile.result.raster())?,
                )?;
            }
        }
    }
    Ok(())
}

/// Measures one cold acceptance followed by bounded warm requests in the same process.
///
/// The ordinary evaluator owns all output and cache semantics; timings exclude exports.
///
/// # Errors
/// Returns canonical evaluation or artifact I/O failures without modifying fixture inputs.
///
/// # Panics
/// Panics if cache reuse changes canonical scene or any RGBA byte.
fn warm_case(
    document: Document,
    source: ResolvedSource,
    directory: &Path,
    repeats: usize,
) -> Result<()> {
    let target = requested_preview_target(document.canvas())?;
    let session = DocumentSession::new(document)?;
    let request =
        EvaluationRequest::new(session.document_evaluation_snapshot(), source).for_preview(target);
    let mut cache = EvaluationProfileCache::default();
    let cold = evaluate_profiled_cached_with_limits(
        request.clone(),
        EvaluationLimits::default(),
        &mut cache,
    )?;
    metrics("cold", 0, &cold);
    artifacts(directory, "cold", &cold.result)?;
    for repeat in 0..repeats {
        let warm = evaluate_profiled_cached_with_limits(
            request.clone(),
            EvaluationLimits::default(),
            &mut cache,
        )?;
        metrics("warm", repeat, &warm);
        assert_eq!(warm.result.scene(), cold.result.scene());
        assert_eq!(warm.result.raster(), cold.result.raster());
    }
    Ok(())
}

/// Measures decode and one million deterministic source-field samples independently.
///
/// # Errors
/// Returns canonical decode/sampling errors without modifying source identity or pixels.
fn sample_case(document: &Document, source: &ResolvedSource, repeats: usize) -> Result<()> {
    let bytes = source.bytes().ok_or("expected encoded immutable source")?;
    let hint = source.format().ok_or("expected source format")?;
    let canvas = document.canvas();
    for repeat in 0..repeats {
        let started = Instant::now();
        let field = toniator_sampling::decode_source(bytes, hint)?;
        emit(
            json!({"kind":"sampling","label":"decode","repeat":repeat,"us":started.elapsed().as_micros(),"decoded_sha256":field.identity().decoded_pixel_hash}),
        );
        let started = Instant::now();
        let mut sum = 0.0;
        for index in 0..1_000_000 {
            let point = Point2 {
                x: f64::from(index % 1000) * canvas.width / 999.0,
                y: f64::from(index / 1000) * canvas.height / 999.0,
            };
            sum += field.sample_mark_ink(
                point,
                canvas,
                SourcePlacement::StretchToCanvas,
                SourceComponent::Luminance,
            )?;
        }
        std::hint::black_box(sum);
        emit(
            json!({"kind":"sampling","label":"million_bilinear_luminance","repeat":repeat,"us":started.elapsed().as_micros(),"samples":1_000_000,"sum":sum}),
        );
    }
    Ok(())
}

/// Measures explicit source-independent fit separately from ordinary saved-companion reuse.
///
/// # Errors
/// Returns bounded fitting/cancellation errors without attaching or publishing new calibration.
///
/// # Panics
/// Panics when repeated explicit measurements produce unequal saved artifacts.
fn calibrate_case(document: &Document, repeats: usize, directory: &Path) -> Result<()> {
    let channel = document
        .channel_topology()
        .ok_or("missing topology")?
        .channels()[0]
        .id;
    let mut expected = None;
    for repeat in 0..repeats {
        let started = Instant::now();
        let saved = reference_calibration::calibrate_saved_pattern(
            document,
            channel,
            EvaluationLimits::default(),
            &|| false,
        )?;
        let elapsed = started.elapsed();
        if let Some(ref previous) = expected {
            assert_eq!(previous, &saved);
        } else {
            expected = Some(saved.clone());
        }
        let bytes = serde_json::to_vec_pretty(
            &json!({"profile":saved.profile(),"exact_debug":format!("{saved:?}")}),
        )?;
        emit(
            json!({"kind":"calibration","repeat":repeat,"us":elapsed.as_micros(),"profile":saved.profile(),"sha256":hash(&bytes)}),
        );
        fs::write(directory.join(format!("calibration-{repeat}.json")), bytes)?;
    }
    Ok(())
}

/// Measures ten immutable materialized frames against one accepted cache per trajectory.
///
/// # Errors
/// Returns temporal validation/evaluation failures without mutating or publishing project state.
fn animation_case(document: Document, source: ResolvedSource, repeats: usize) -> Result<()> {
    for (label, field, end) in [
        ("opacity", PropertyFieldId::Opacity, 0.5),
        ("aspect", PropertyFieldId::DensityAspect, 1.2),
    ] {
        let animated = document.clone().with_temporal_authority(
            ProjectTiming::new(FrameRate::new(24, 1)?, FrameRange::new(0, 10)?),
            vec![TemporalEndOverride::Scalar(ScalarEndOverride {
                target: if field == PropertyFieldId::Opacity {
                    PropertyTarget::Channel(
                        document
                            .channel_topology()
                            .ok_or("missing topology")?
                            .channels()[0]
                            .id,
                    )
                } else {
                    PropertyTarget::Document
                },
                field,
                end,
                easing: Easing::Linear,
            })],
        )?;
        let session = DocumentSession::new(animated)?;
        for repeat in 0..repeats {
            let mut cache = EvaluationProfileCache::default();
            for frame in 0..10 {
                let request = EvaluationRequest::new(
                    session.document_evaluation_snapshot_at_frame(frame)?,
                    source.clone(),
                );
                let profile = evaluate_profiled_cached_with_limits(
                    request,
                    EvaluationLimits::default(),
                    &mut cache,
                )?;
                metrics(&format!("{label}-frame{frame}"), repeat, &profile);
            }
        }
    }
    Ok(())
}

/// Streams native-equivalent and large PNG from one canonical scene without full large buffers.
///
/// # Errors
/// Returns geometry/raster/encoding/I/O failures; only stage-owned output files receive bytes.
fn stream_case(
    document: Document,
    source: ResolvedSource,
    directory: &Path,
    repeats: usize,
) -> Result<()> {
    let canvas = document.canvas().clone();
    let request = EvaluationRequest::new(
        DocumentSession::new(document)?.document_evaluation_snapshot(),
        source,
    )
    .for_scene()?;
    let profile = evaluate_profiled_with_limits(request, EvaluationLimits::default())?;
    metrics("scene", 0, &profile);
    for repeat in 0..repeats {
        for (label, scale) in [("native", 1), ("large8x", 8)] {
            let target = StreamingRasterTarget::new(
                canvas.width as u32 * scale,
                canvas.height as u32 * scale,
            )?;
            let path = directory.join(format!("stream-{label}-{repeat}.png"));
            let started = Instant::now();
            let summary = write_png_streaming(
                profile.result.scene(),
                target,
                PngStreamOptions::default(),
                RasterizationLimits::default(),
                fs::File::create(&path)?,
                &|| false,
                &|_, _| {},
            )?;
            let elapsed = started.elapsed();
            let bytes = fs::read(path)?;
            emit(
                json!({"kind":"stream","label":label,"repeat":repeat,"us":elapsed.as_micros(),"width":target.width(),"height":target.height(),"band_rows":summary.band_rows,"band_bytes":summary.maximum_band_pixel_bytes,"prepared_geometry_bytes":summary.prepared_geometry_bytes,"bytes":bytes.len(),"sha256":hash(&bytes)}),
            );
        }
    }
    Ok(())
}

/// Measures in-flight cooperative cancellation after a fixed real delay on current canonical APIs.
///
/// # Errors
/// Returns malformed/out-of-range delay, target/session setup or worker-panic errors.
/// A completed result is recorded without claiming it preceded the signal; no partial result publishes.
fn cancel_case(document: Document, source: ResolvedSource, repeats: usize) -> Result<()> {
    let delay_ms: u64 = env::var("P0_CANCEL_DELAY_MS")
        .unwrap_or_else(|_| "50".into())
        .parse()?;
    if !(1..=10_000).contains(&delay_ms) {
        return Err("cancellation delay must be 1 through 10000 ms".into());
    }
    for repeat in 0..repeats {
        let mut request = EvaluationRequest::new(
            DocumentSession::new(document.clone())?.document_evaluation_snapshot(),
            source.clone(),
        )
        .for_output(
            RasterBackground::Transparent,
            Some(OutputRasterTarget::new(8192, 8192)?),
            RasterAntialiasing::On,
        );
        if env::var_os("P1_PREVIEW_TARGET").is_some() {
            request = request.for_preview(requested_preview_target(document.canvas())?);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancelled.clone();
        let worker = std::thread::spawn(move || {
            evaluate_cancellable_with_limits(request, EvaluationLimits::default(), &worker_cancel)
        });
        std::thread::sleep(Duration::from_millis(delay_ms));
        let started = Instant::now();
        cancelled.store(true, Ordering::Release);
        let outcome = worker.join().map_err(|_| "cancel worker panic")?;
        emit(
            json!({"kind":"cancel","repeat":repeat,"delay_ms":delay_ms,"us_after_signal":started.elapsed().as_micros(),"outcome":match outcome { Err(EvaluationRunError::Cancelled)=>"cancelled".into(),Err(e)=>format!("{e:?}"),Ok(_)=>"completed_without_cancellation_error".into() }}),
        );
    }
    Ok(())
}

/// Dispatches one bounded evidence mode and stores only P0/P1/P2 performance output.
///
/// # Errors
/// Returns malformed arguments, out-of-scope or already-used output paths, or canonical errors.
fn run() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 5 {
        return Err(
            "usage: performance_probe MODE png|svg RECIPE PERFORMANCE_OUTPUT REPEATS".into(),
        );
    }
    let directory = PathBuf::from(&args[3]);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?
        .join("target/validation/performance-20261016");
    if !(directory.starts_with(root.join("p0"))
        || directory.starts_with(root.join("p1"))
        || directory.starts_with(root.join("p2")))
        || directory
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err("output must be lexically within the P0/P1/P2 directories".into());
    }
    if directory.join("input.toniator").exists() {
        return Err("output already contains a probe input; choose a fresh directory".into());
    }
    fs::create_dir_all(&directory)?;
    if !directory.canonicalize()?.starts_with(root) {
        return Err("output must be within the performance directory".into());
    }
    let repeats: usize = args[4].parse()?;
    if !(1..=10).contains(&repeats) {
        return Err("repeats must be 1 through 10".into());
    }
    let started = Instant::now();
    let (document, bundle, source) = fixture(&args[1], &args[2])?;
    emit(
        json!({"kind":"setup","us":started.elapsed().as_micros(),"source":args[1],"recipe":args[2],"mode":args[0],"repeats":repeats,"source_sha256":source.bytes().map(hash),"model":format!("{:?}",document.channel_model()),"preview_target":if args[0]=="preview-native" || env::var_os("P1_PREVIEW_TARGET").is_some(){Some(requested_preview_target(document.canvas())?.raster_identity())}else{None}}),
    );
    toniator_io::save(&directory.join("input.toniator"), &document, &bundle)?;
    match args[0].as_str() {
        "warm" => warm_case(document, source, &directory, repeats),
        "evaluate" => evaluate_case(document, source, &directory, repeats, true, false),
        "native" => evaluate_case(document, source, &directory, repeats, false, false),
        "preview-native" => evaluate_case(document, source, &directory, repeats, false, true),
        "sample" => sample_case(&document, &source, repeats),
        "calibrate" => calibrate_case(&document, repeats, &directory),
        "animation" => animation_case(document, source, repeats),
        "stream" => stream_case(document, source, &directory, repeats),
        "cancel" => cancel_case(document, source, repeats),
        _ => Err("unknown mode".into()),
    }
}

/// Reports bounded probe failures with a process exit code suitable for an external supervisor.
fn main() {
    if let Err(error) = run() {
        eprintln!("performance_probe: {error}");
        std::process::exit(2);
    }
}
