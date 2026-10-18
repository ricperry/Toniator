//! Stage-scoped native output and capacity evidence through the production engine.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    env,
    error::Error,
    fs,
    io::{BufWriter, Read, Write},
    path::Path,
    sync::atomic::AtomicBool,
    time::Instant,
};
use toniator_domain::*;
use toniator_engine::*;
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle};

pub(crate) type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Returns a SHA-256 fingerprint of already produced bytes without changing their encoding.
pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Attaches an explicitly synthetic monotone response through current binding validators.
/// This controlled fixture mapping is not a measured calibration or fitter-success claim.
/// All channels and hidden ALL own their current recipe/aspect/orientation binding.
///
/// # Errors
/// Returns current calibration binding, curve, or document validation failure.
fn fixture_calibration(mut document: Document) -> Result<Document> {
    let raw = vec![
        CalibrationKnot {
            input: 0.0,
            coverage: 0.0,
        },
        CalibrationKnot {
            input: 2.0,
            coverage: 1.0,
        },
    ];
    let channels: Vec<_> = document
        .channel_topology()
        .ok_or("missing modeled channels")?
        .channels()
        .iter()
        .map(|c| c.id)
        .collect();
    for channel in channels {
        let artifact = SavedCalibration::new(
            CalibrationBinding::for_channel(&document, channel)?,
            raw.clone(),
            None,
        )?;
        document = document.with_saved_calibration(channel, artifact)?;
    }
    let artifact = SavedCalibration::new(document.baseline_calibration_binding()?, raw, None)?;
    Ok(document.with_saved_baseline_calibration(artifact)?)
}

/// Builds a fresh current document and embedded immutable source without reading old projects.
/// Spiral fixtures retain default density, five partial turns, phase and winding; crossing
/// fixtures exercise canonical canvas clipping while contained fixtures preserve whole paths.
/// Other representatives use a small explicit density and current ID-free domain recipes.
///
/// # Errors
/// Returns source I/O, recipe, document, or current source-container validation failures.
pub(crate) fn fixture(
    source: &str,
    case: &str,
) -> Result<(Document, SourceBundle, ResolvedSource)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (name, width, height, format, hint) = match source {
        "png" => (
            "raster-sample.png",
            1024.0_f64,
            1024.0_f64,
            EmbeddedSourceFormat::Png,
            SourceFormatHint::Png,
        ),
        "svg" => (
            "vector-sample.svg",
            900.0_f64,
            620.0_f64,
            EmbeddedSourceFormat::Svg,
            SourceFormatHint::Svg,
        ),
        _ => return Err("source must be png or svg".into()),
    };
    let bytes = fs::read(root.join("assets").join(name))?;
    let id = SourceReferenceId::new(format!("preview-recovery-{source}"))?;
    let base = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(id.clone()),
    )?;
    let grid = base.reconstruct_pattern_definition_recipe(base.pattern_settings().definition_id)?;
    let coverage = CoveragePolicy {
        guard_steps: 1,
        additional_margin: 0.0,
    };
    let recipe = if case.starts_with("round-") || case.starts_with("square-") {
        let shape = if case.starts_with("round-") {
            SpiralShape::Round
        } else {
            SpiralShape::Square
        };
        let crosses = case.ends_with("crossing");
        let radial_spacing = match (shape, crosses) {
            (SpiralShape::Round, false) => width.min(height) * 0.025,
            (SpiralShape::Round, true) => width.hypot(height) * 0.15,
            (SpiralShape::Square, false) => width.min(height) * 0.05,
            (SpiralShape::Square, true) => width.max(height) * 0.3,
        };
        PatternDefinitionRecipe::connected(PatternStructureRecipe::ParametricCurve {
            name: format!("Preview recovery {case}"),
            coverage,
            curve: ParametricCurve::Spiral(SpiralCurve {
                shape,
                turns: 5.375,
                radial_spacing,
                phase_degrees: 37.0,
                winding: CurveWinding::Clockwise,
            }),
            spiral_coverage: SpiralCoveragePolicy::Fixed,
            repetition: CurveRepetition::Single,
            sites: None,
        })
    } else if case == "grid" {
        grid
    } else if case == "region" {
        let structure = match grid.structure {
            PatternStructureRecipe::OrderedOutputs { definition, .. } => *definition,
            structure => structure,
        };
        PatternDefinitionRecipe::regions(structure)
    } else if case == "random" {
        PatternDefinitionRecipe::marks(PatternStructureRecipe::RandomSites {
            name: "Preview recovery deterministic dispersion".into(),
            coverage,
            character: RandomSiteCharacter::Even {
                minimum_center_distance: 8.0,
            },
            nominal_spacing: None,
            seed: 907,
            density_modulation: SiteDensityModulation::Uniform,
            exclusion: SiteExclusionPolicy::None,
            maximum_attempts: 16_000_000,
            maximum_neighbor_checks: 16_000_000,
            refinement: RandomSiteRefinement::default(),
        })
    } else {
        return Err("unknown fixture case".into());
    };
    let settings = base.pattern_settings().clone();
    let definition = base
        .pattern_definition_bundles()
        .iter()
        .find(|b| b.definition.id == settings.definition_id)
        .ok_or("missing base definition")?
        .definition
        .clone();
    let mut history = DocumentHistory::new(DocumentSession::new(base)?);
    history.apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: settings,
        base_definition: definition,
        recipe,
    })?;
    let mut document = history.document().clone();
    if matches!(case, "grid" | "random" | "region") {
        document = document
            .edit_all_channel_start_configuration(&[(PropertyFieldId::Density, 12.5)])?
            .bind(&document)?;
    }
    document = fixture_calibration(document)?;
    document.require_saved_calibration_authority()?;
    let bundle = SourceBundle::new([EmbeddedSource::new(
        id.clone(),
        format,
        bytes.clone(),
        Some(name.into()),
    )?])?;
    Ok((document, bundle, ResolvedSource::new(id, bytes, hint)?))
}

/// Writes raw canonical f64 bits for every stroke path, profile and outline in painter order.
/// The adjacent complete Debug scene includes identities, all families, provenance and paint.
/// This evidence projection executes after evaluation timing and never alters canonical values.
///
/// # Errors
/// Returns artifact I/O failure; creates no input or product state.
fn stroke_bits(directory: &Path, result: &EvaluationResult) -> Result<()> {
    let mut writer = BufWriter::new(fs::File::create(directory.join("stroke-f64-le.bin"))?);
    let mut push = |value: f64| writer.write_all(&value.to_bits().to_le_bytes());
    let mut counts = Vec::new();
    for layer in result.scene().layers() {
        for output in layer.outputs() {
            if let GeometryOutput::CanonicalStrokes(strokes) = output.geometry() {
                for stroke in strokes {
                    let mut segments = stroke.path.segments().to_vec();
                    counts.push(json!({"path_segments":segments.len(),"profile_samples":stroke.profile.len(),"outline_contours":stroke.outline.contours.iter().map(|c|c.segments.len()).collect::<Vec<_>>() }));
                    for contour in &stroke.outline.contours {
                        segments.extend_from_slice(&contour.segments);
                    }
                    for segment in segments {
                        push(segment.start().x)?;
                        push(segment.start().y)?;
                        if let toniator_patterns::CurveSegment::CubicBezier(cubic) = segment {
                            push(cubic.control_1().x)?;
                            push(cubic.control_1().y)?;
                            push(cubic.control_2().x)?;
                            push(cubic.control_2().y)?;
                        }
                        push(segment.end().x)?;
                        push(segment.end().y)?;
                    }
                    for sample in &stroke.profile {
                        push(sample.location.parameter())?;
                        push(sample.center.x)?;
                        push(sample.center.y)?;
                        push(sample.normalized_thickness)?;
                        push(sample.width)?;
                    }
                }
            }
        }
    }
    writer.flush()?;
    fs::write(
        directory.join("stroke-counts.json"),
        serde_json::to_vec_pretty(&counts)?,
    )?;
    Ok(())
}

/// Records existing engine phase times/counts without serializing geometry inside the interval.
fn metrics(mode: &str, repeat: usize, elapsed: u128, profile: &ProfiledEvaluation) -> Value {
    json!({"mode":mode,"repeat":repeat,"wall_us":elapsed,"workers":profile.performance.configured_worker_count,"observed_workers":profile.performance.observed_worker_count,"records":profile.performance.records.iter().map(|r|json!({"stage":format!("{:?}",r.stage),"channel":r.channel_id.map(|c|c.0),"output":r.output_layer_id.map(|o|o.0),"us":r.elapsed.as_micros(),"execution":format!("{:?}",r.execution),"cache":format!("{:?}",r.cache),"workloads":r.workloads.iter().map(|w|json!({"kind":format!("{:?}",w.kind),"count":w.count})).collect::<Vec<_>>()})).collect::<Vec<_>>()})
}

/// Compares every artifact byte against its first successful result with bounded read storage.
/// This avoids retaining a second full raw raster/SVG/canonical dump in the probe's warm path.
///
/// # Errors
/// Returns I/O failure or an exact-byte discrepancy without replacing the first artifact.
fn compare_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::File::open(path)?;
    let mut buffer = [0_u8; 65_536];
    let mut offset = 0;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        if bytes.get(offset..offset + count) != Some(&buffer[..count]) {
            return Err(format!("exact artifact differs: {}", path.display()).into());
        }
        offset += count;
    }
    if offset != bytes.len() {
        return Err(format!("artifact length differs: {}", path.display()).into());
    }
    Ok(())
}

/// Runs repeated cold/warm native evaluation with exact output assertions and immutable artifacts.
/// A fresh process supplies a cold decode/cache; each repeat owns its own ordinary engine cache.
/// Failed evaluation returns no scene and never writes a partial PNG/SVG.
///
/// # Errors
/// Returns current engine, native encoder, serialization, or evidence I/O failure.
/// Output comparisons report exact byte discrepancies without changing the saved baseline.
pub(crate) fn run(
    document: Document,
    source: ResolvedSource,
    directory: &Path,
    repeats: usize,
) -> Result<()> {
    fs::create_dir_all(directory)?;
    let session = DocumentSession::new(document)?;
    let request = EvaluationRequest::new(session.document_evaluation_snapshot(), source);
    let mut first = true;
    let mut records = Vec::new();
    for repeat in 0..repeats {
        let mut cache = EvaluationProfileCache::default();
        for mode in ["cold", "warm"] {
            let start = Instant::now();
            let profile = evaluate_profiled_cached_with_limits(
                request.clone(),
                EvaluationLimits::default(),
                &mut cache,
            )?;
            let elapsed = start.elapsed().as_micros();
            records.push(metrics(mode, repeat, elapsed, &profile));
            // These full copies and encodes are diagnostic overhead outside the measured call.
            let canonical = format!("{:#?}", profile.result.scene()).into_bytes();
            let rgba = profile.result.raster().pixels().to_vec();
            let png = encode_png(profile.result.raster())?;
            let svg = write_svg(profile.result.scene())?;
            let fingerprints = json!({"canonical_debug_sha256":hash(&canonical),"rgba_sha256":hash(&rgba),"png_sha256":hash(&png),"svg_sha256":hash(svg.as_bytes()),"family":profile.result.scene().identity().family_fingerprint(),"realization":profile.result.scene().identity().realization_fingerprint(),"scene":profile.result.scene().identity().scene_fingerprint()});
            let values = (canonical, rgba, png, svg);
            if first {
                fs::write(directory.join("canonical-debug.txt"), &values.0)?;
                fs::write(directory.join("native.rgba"), &values.1)?;
                fs::write(directory.join("native.png"), &values.2)?;
                fs::write(directory.join("native.svg"), &values.3)?;
                fs::write(
                    directory.join("identity.json"),
                    serde_json::to_vec_pretty(&fingerprints)?,
                )?;
                stroke_bits(directory, &profile.result)?;
                first = false;
            } else {
                compare_bytes(&directory.join("canonical-debug.txt"), &values.0)?;
                compare_bytes(&directory.join("native.rgba"), &values.1)?;
                compare_bytes(&directory.join("native.png"), &values.2)?;
                compare_bytes(&directory.join("native.svg"), values.3.as_bytes())?;
            }
            println!(
                "{}",
                json!({"kind":"evaluation","directory":directory,"metrics":records.last(),"identity":fingerprints})
            );
        }
    }
    fs::write(
        directory.join("metrics.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    Ok(())
}

/// Publishes full-quality native PNG and measured API facts before observing native SVG.
/// Diagnostic geometry is projected to streamed f64 storage, avoiding a retained giant Debug/RGBA
/// copy while the ordinary editable SVG serializer runs. No product quality/policy is changed.
/// # Errors
/// Returns actual engine/export/evidence errors without replacing source project bytes.
fn run_native_project(
    document: Document,
    source: ResolvedSource,
    directory: &Path,
    repeats: usize,
) -> Result<()> {
    fs::create_dir_all(directory)?;
    let session = DocumentSession::new(document)?;
    let request = EvaluationRequest::new(session.document_evaluation_snapshot(), source);
    let mut records = Vec::new();
    let mut expected_scene = None;
    for repeat in 0..repeats {
        let mut cache = EvaluationProfileCache::default();
        for mode in ["cold", "warm"] {
            let start = Instant::now();
            let profile = evaluate_profiled_cached_with_limits(
                request.clone(),
                EvaluationLimits::default(),
                &mut cache,
            )?;
            let elapsed = start.elapsed().as_micros();
            let record = metrics(mode, repeat, elapsed, &profile);
            records.push(record.clone());
            println!(
                "{}",
                json!({"kind":"native_api_complete","mode":mode,"repeat":repeat,"metrics":record})
            );
            fs::write(
                directory.join("metrics.json"),
                serde_json::to_vec_pretty(&records)?,
            )?;
            if let Some(scene) = &expected_scene {
                if scene != profile.result.scene() {
                    return Err("canonical repeat differs".into());
                }
            } else {
                expected_scene = Some(profile.result.scene().clone());
            }
            let png_started = Instant::now();
            let png = encode_png(profile.result.raster())?;
            if repeat == 0 && mode == "cold" {
                fs::write(directory.join("native.png"), &png)?;
                fs::write(
                    directory.join("native.rgba"),
                    profile.result.raster().pixels(),
                )?;
                stroke_bits(directory, &profile.result)?;
                fs::write(
                    directory.join("identity.json"),
                    serde_json::to_vec_pretty(
                        &json!({"family":profile.result.scene().identity().family_fingerprint(),"realization":profile.result.scene().identity().realization_fingerprint(),"scene":profile.result.scene().identity().scene_fingerprint(),"rgba_sha256":hash(profile.result.raster().pixels()),"png_sha256":hash(&png)}),
                    )?,
                )?;
            } else {
                compare_bytes(&directory.join("native.png"), &png)?;
                compare_bytes(
                    &directory.join("native.rgba"),
                    profile.result.raster().pixels(),
                )?;
            }
            println!(
                "{}",
                json!({"kind":"native_png_published","us":png_started.elapsed().as_micros(),"png_bytes":png.len(),"mode":mode,"repeat":repeat})
            );
            drop(png);
            println!(
                "{}",
                json!({"kind":"svg_started","mode":mode,"repeat":repeat})
            );
            let svg_start = Instant::now();
            let svg = write_svg(profile.result.scene())?;
            if repeat == 0 && mode == "cold" {
                fs::write(directory.join("native.svg"), &svg)?;
            } else {
                compare_bytes(&directory.join("native.svg"), svg.as_bytes())?;
            }
            println!(
                "{}",
                json!({"kind":"svg_complete","us":svg_start.elapsed().as_micros(),"bytes":svg.len(),"mode":mode,"repeat":repeat})
            );
        }
    }
    Ok(())
}

/// Exercises safe typed boundary failures through the ordinary synchronous engine.
/// These headless witnesses do not establish GTK routing/texture/retained-pixel acceptance.
/// Cancellation uses the existing atomic API; reported allocation is covered independently
/// by impossible-capacity reserve tests, never an OS OOM experiment.
///
/// # Errors
/// Returns bad invocation, fixture/policy/I/O failure, or unexpected successful evaluation.
fn failure(source_name: &str, case: &str, boundary: &str, directory: &Path) -> Result<()> {
    let (document, _, mut source) = fixture(source_name, case)?;
    let limits = match boundary {
        "source" => {
            source = ResolvedSource::new(
                source.reference_id().clone(),
                b"invalid PNG".to_vec(),
                SourceFormatHint::Png,
            )?;
            EvaluationLimits::default()
        }
        "family" => EvaluationLimits::new(1)?,
        "profile" => EvaluationLimits::default().with_max_stroke_profile_samples(1)?,
        "outline" => EvaluationLimits::default().with_max_stroke_outline_segments(1)?,
        "raster" => EvaluationLimits::default().with_max_flattened_raster_edges(1)?,
        "cancel" => EvaluationLimits::default(),
        _ => {
            return Err(
                "failure boundary must be source, family, profile, outline, raster or cancel"
                    .into(),
            );
        }
    };
    let session = DocumentSession::new(document)?;
    let request = EvaluationRequest::new(session.document_evaluation_snapshot(), source);
    let started = Instant::now();
    let error = match evaluate_cancellable_with_limits(
        request,
        limits,
        &AtomicBool::new(boundary == "cancel"),
    ) {
        Ok(_) => return Err("failure witness unexpectedly succeeds".into()),
        Err(EvaluationRunError::Cancelled) => json!({"type":"cancelled"}),
        Err(EvaluationRunError::Evaluation(error)) => {
            json!({"type":"evaluation","path":error.path(),"message":error.message()})
        }
    };
    let record = json!({"source":source_name,"case":case,"boundary":boundary,"elapsed_us":started.elapsed().as_micros(),"outcome":error,"partial_output":false});
    fs::create_dir_all(directory)?;
    fs::write(
        directory.join("failure.json"),
        serde_json::to_vec_pretty(&record)?,
    )?;
    println!("{record}");
    Ok(())
}

/// Replays the current area-consumer cubic numeric diagnostic without editing the renderer.
/// The replay retains exact native coordinates/arithmetic and reports the first failed node;
/// it is diagnostic evidence, not a replacement rasterizer or quality acceptance.
fn flatten_failure(p: [toniator_patterns::Point2; 4], tolerance: f64, depth: u8) -> Option<Value> {
    let distance = |point: toniator_patterns::Point2,
                    start: toniator_patterns::Point2,
                    end: toniator_patterns::Point2| {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length = (dx * dx + dy * dy).sqrt();
        if length == 0.0 {
            ((point.x - start.x).powi(2) + (point.y - start.y).powi(2)).sqrt()
        } else {
            (dy * point.x - dx * point.y + end.x * start.y - end.y * start.x).abs() / length
        }
    };
    let extent = p
        .iter()
        .enumerate()
        .flat_map(|(i, a)| p[i + 1..].iter().map(move |b| (a.x - b.x).hypot(a.y - b.y)))
        .fold(0.0_f64, f64::max);
    let flatness = distance(p[1], p[0], p[3]).max(distance(p[2], p[0], p[3]));
    if flatness <= tolerance || extent <= tolerance {
        return None;
    }
    let midpoint = |a: toniator_patterns::Point2, b: toniator_patterns::Point2| {
        toniator_patterns::Point2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
    };
    let a = midpoint(p[0], p[1]);
    let b = midpoint(p[1], p[2]);
    let c = midpoint(p[2], p[3]);
    let d = midpoint(a, b);
    let e = midpoint(b, c);
    let f = midpoint(d, e);
    if depth >= 60 || f == p[0] || f == p[3] {
        let between =
            |value: f64, start: f64, end: f64| value >= start.min(end) && value <= start.max(end);
        if (p[1] == p[0] && p[2] == p[3])
            || (p.iter().all(|q| q.x == p[0].x)
                && between(p[1].y, p[0].y, p[3].y)
                && between(p[2].y, p[0].y, p[3].y))
            || (p.iter().all(|q| q.y == p[0].y)
                && between(p[1].x, p[0].x, p[3].x)
                && between(p[2].x, p[0].x, p[3].x))
        {
            return None;
        }
        return Some(
            json!({"depth":depth,"flatness":flatness,"extent":extent,"tolerance":tolerance,"mid_equals_start":f==p[0],"mid_equals_end":f==p[3],"points":p.iter().map(|q|json!({"x":q.x,"y":q.y,"x_bits":format!("{:016x}",q.x.to_bits()),"y_bits":format!("{:016x}",q.y.to_bits())})).collect::<Vec<_>>()}),
        );
    }
    flatten_failure([p[0], a, d, f], tolerance, depth + 1)
        .or_else(|| flatten_failure([f, e, c, p[3]], tolerance, depth + 1))
}

/// Evaluates the unchanged current project scene and isolates a native or exactly mapped Fit area failure.
/// The existing one-pixel AA-off scene request preserves full canonical geometry/source/calibration;
/// its raster is deliberately not saved or described as a native output.
/// # Errors
/// Returns current load/evaluation/I/O errors or absence of the expected diagnostic witness.
fn diagnose_project(input: &Path, directory: &Path, fit: Option<(u32, u32)>) -> Result<()> {
    let loaded = toniator_io::load(input)?;
    let SourceReference::Assigned(id) = loaded.document().source() else {
        return Err("assigned source required".into());
    };
    let source = loaded.sources().get(id).ok_or("missing source")?;
    let hint = match source.format() {
        EmbeddedSourceFormat::Png => SourceFormatHint::Png,
        EmbeddedSourceFormat::Svg => SourceFormatHint::Svg,
        _ => return Err("PNG/SVG required".into()),
    };
    let session = DocumentSession::new(loaded.document().clone())?;
    let request = EvaluationRequest::new(
        session.document_evaluation_snapshot(),
        ResolvedSource::new(id.clone(), source.shared_bytes(), hint)?,
    );
    fs::create_dir_all(directory)?;
    if let Some((width, height)) = fit {
        let started = Instant::now();
        let actual = evaluate_profiled_with_limits(
            request
                .clone()
                .for_preview(PreviewRasterTarget::new(width, height)?),
            EvaluationLimits::default(),
        );
        let record = match actual {
            Ok(_) => json!({"outcome":"success","wall_us":started.elapsed().as_micros()}),
            Err(error) => {
                json!({"outcome":"typed_failure","path":error.path(),"message":error.message(),"wall_us":started.elapsed().as_micros()})
            }
        };
        fs::write(
            directory.join("fit-request-reproduction.json"),
            serde_json::to_vec_pretty(&record)?,
        )?;
        println!("{record}");
    }
    let started = Instant::now();
    let profile = evaluate_profiled_with_limits(request.for_scene()?, EvaluationLimits::default())?;
    let elapsed = started.elapsed().as_micros();
    fs::create_dir_all(directory)?;
    fs::write(
        directory.join("scene-metrics.json"),
        serde_json::to_vec_pretty(&metrics("scene-AA-off-one-pixel", 0, elapsed, &profile))?,
    )?;
    stroke_bits(directory, &profile.result)?;
    let (scale, offset_x, offset_y) = fit.map_or((1.0, 0.0, 0.0), |(width, height)| {
        let canvas = profile.result.scene().canvas();
        let scale = (f64::from(width) / canvas.width).min(f64::from(height) / canvas.height);
        (
            scale,
            (f64::from(width) - canvas.width * scale) / 2.0,
            (f64::from(height) - canvas.height * scale) / 2.0,
        )
    });
    for (channel, layer) in profile.result.scene().layers().iter().enumerate() {
        for output in layer.outputs() {
            if let GeometryOutput::CanonicalStrokes(strokes) = output.geometry() {
                for (stroke_index, stroke) in strokes.iter().enumerate() {
                    for (contour_index, contour) in stroke.outline.contours.iter().enumerate() {
                        for (segment_index, segment) in contour.segments.iter().enumerate() {
                            if let toniator_patterns::CurveSegment::CubicBezier(cubic) = segment {
                                let source_points = [
                                    cubic.start(),
                                    cubic.control_1(),
                                    cubic.control_2(),
                                    cubic.end(),
                                ];
                                let p = source_points.map(|point| {
                                    toniator_patterns::Point2::new(
                                        offset_x + point.x * scale,
                                        offset_y + point.y * scale,
                                    )
                                });
                                let extent = p
                                    .iter()
                                    .enumerate()
                                    .flat_map(|(i, a)| {
                                        p[i + 1..].iter().map(move |b| (a.x - b.x).hypot(a.y - b.y))
                                    })
                                    .fold(0.0_f64, f64::max);
                                let tolerance =
                                    (1.0_f64 / 64.0).min(extent / 64.0).max(f64::MIN_POSITIVE);
                                if let Some(node) = flatten_failure(p, tolerance, 0) {
                                    let record = json!({"scope":"certificate-aware area replay of actual native/Fit consumer; canonical scene unchanged","fit_target":fit,"transform":{"scale":scale,"offset_x":offset_x,"offset_y":offset_y},"input_sha256":hash(&fs::read(input)?),"channel_index":channel,"output_id":output.output_layer_id.0,"stroke_index":stroke_index,"contour_index":contour_index,"segment_index":segment_index,"canonical_cubic":source_points.iter().map(|q|json!({"x":q.x,"y":q.y,"x_bits":format!("{:016x}",q.x.to_bits()),"y_bits":format!("{:016x}",q.y.to_bits())})).collect::<Vec<_>>(),"source_cubic":p.iter().map(|q|json!({"x":q.x,"y":q.y,"x_bits":format!("{:016x}",q.x.to_bits()),"y_bits":format!("{:016x}",q.y.to_bits())})).collect::<Vec<_>>(),"failed_node":node});
                                    fs::write(
                                        directory.join(if fit.is_some() {
                                            "fit-first-flatten-failure.json"
                                        } else {
                                            "native-first-flatten-failure.json"
                                        }),
                                        serde_json::to_vec_pretty(&record)?,
                                    )?;
                                    println!("{record}");
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Err("no matching native cubic replay failure found".into())
}

/// Evaluates unchanged current project geometry through fitted/cropped preview targets and exact warm reuse.
/// All source/calibration/canonical intent remains authoritative; PNGs are labeled by preview target.
/// # Errors
/// Returns load, checked target, evaluation, output or unexpected warm-equivalence failure.
fn preview_project(input: &Path, directory: &Path, target_index: Option<usize>) -> Result<()> {
    let loaded = toniator_io::load(input)?;
    let SourceReference::Assigned(id) = loaded.document().source() else {
        return Err("assigned source required".into());
    };
    let source = loaded.sources().get(id).ok_or("missing source")?;
    let hint = match source.format() {
        EmbeddedSourceFormat::Png => SourceFormatHint::Png,
        EmbeddedSourceFormat::Svg => SourceFormatHint::Svg,
        _ => return Err("PNG/SVG required".into()),
    };
    let session = DocumentSession::new(loaded.document().clone())?;
    let original = EvaluationRequest::new(
        session.document_evaluation_snapshot(),
        ResolvedSource::new(id.clone(), source.shared_bytes(), hint)?,
    );
    fs::create_dir_all(directory)?;
    let mut cache = EvaluationProfileCache::default();
    let mut records = Vec::new();
    for (index, (name, target)) in [
        ("fit-1140x908", PreviewRasterTarget::new(1140, 908)?),
        ("fit-640x480", PreviewRasterTarget::new(640, 480)?),
        ("fit-1800x1200", PreviewRasterTarget::new(1800, 1200)?),
        (
            "zoom-crop",
            PreviewRasterTarget::for_viewport(1140, 908, 1800.0, 1800.0, 0.5)?,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if target_index.is_some_and(|selected| selected != index) {
            continue;
        }
        let mut accepted = None;
        for mode in ["cold-target", "warm-target"] {
            let started = Instant::now();
            let evaluated = evaluate_profiled_cached_with_limits(
                original.clone().for_preview(target),
                EvaluationLimits::default(),
                &mut cache,
            );
            let elapsed = started.elapsed().as_micros();
            let profile = match evaluated {
                Ok(profile) => profile,
                Err(error) => {
                    let record = json!({"target":name,"raster_identity":target.raster_identity(),"mode":mode,"wall_us":elapsed,"outcome":"typed_failure","path":error.path(),"message":error.message()});
                    records.push(record.clone());
                    fs::write(
                        directory.join("preview-metrics.json"),
                        serde_json::to_vec_pretty(&records)?,
                    )?;
                    println!("{record}");
                    return Err(error.into());
                }
            };
            let rgba = profile.result.raster().pixels();
            let record = json!({"target":name,"raster_identity":target.raster_identity(),"mode":mode,"outcome":"success","rgba_sha256":hash(rgba),"metrics":metrics(mode,0,elapsed,&profile)});
            if let Some((scene, pixels)) = &accepted {
                if scene != profile.result.scene() || pixels != rgba {
                    return Err("preview warm scene/pixels differ".into());
                }
            } else {
                fs::write(
                    directory.join(format!("{name}.png")),
                    encode_png(profile.result.raster())?,
                )?;
                accepted = Some((profile.result.scene().clone(), rgba.to_vec()));
            }
            records.push(record.clone());
            fs::write(
                directory.join("preview-metrics.json"),
                serde_json::to_vec_pretty(&records)?,
            )?;
            println!("{record}");
        }
    }
    Ok(())
}

/// Measures cancellation from a controlled external-thread flag store to typed operation return.
/// Preparation, artifact output and thread joining stay outside the interval; local failure cleanup stays inside.
/// # Errors
/// Returns a missing checkpoint, unexpected success/diagnostic, controller or evidence failure.
fn timed_probe_cancel(
    phase: &str,
    threshold: usize,
    operation: impl FnOnce(&(dyn Fn() -> bool + Sync)) -> std::result::Result<(), String>,
) -> Result<Value> {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };
    let stop = AtomicBool::new(false);
    let polls = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let stopped = &stop;
        let controller = scope.spawn(move || {
            rx.recv().ok().map(|()| {
                let request = Instant::now();
                stopped.store(true, Ordering::SeqCst);
                request
            })
        });
        let result = operation(&|| {
            let count = polls.fetch_add(1, Ordering::Relaxed) + 1;
            if count == threshold {
                let _ = tx.send(());
            }
            stop.load(Ordering::SeqCst)
        });
        let returned = Instant::now();
        drop(tx);
        let requested = controller
            .join()
            .map_err(|_| "controller panicked")?
            .ok_or("checkpoint not reached")?;
        let Some(diagnostic) = result.err() else {
            return Ok(
                json!({"kind":"cancel_natural_completion","phase":phase,"mode":"nonblocking-inflight"}),
            );
        };
        if diagnostic != "evaluation.cancelled" && diagnostic != "svg.cancelled" {
            return Err(format!("unexpected diagnostic {diagnostic}").into());
        }
        Ok(
            json!({"kind":"cancel_latency","phase":phase,"mode":"nonblocking-inflight","request_to_typed_return_ns":returned.duration_since(requested).as_nanos(),"polls":polls.load(Ordering::Relaxed),"diagnostic":diagnostic,"partial_output":false}),
        )
    })
}

/// Prepares current original canonical geometry and both immutable decoded sources outside latency windows.
/// Profiles use explicit validated test intent and Identity response, not a saved-calibration claim.
/// # Errors
/// Returns current source/recipe/evaluation, cancellation-witness or evidence failures.
fn latency_project(input: &Path, directory: &Path, external_svg: bool) -> Result<()> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use toniator_patterns::*;
    fs::create_dir_all(directory)?;
    let loaded = toniator_io::load(input)?;
    let SourceReference::Assigned(id) = loaded.document().source() else {
        return Err("assigned source required".into());
    };
    let entry = loaded.sources().get(id).ok_or("missing source")?;
    let hint = match entry.format() {
        EmbeddedSourceFormat::Png => SourceFormatHint::Png,
        EmbeddedSourceFormat::Svg => SourceFormatHint::Svg,
        _ => return Err("PNG/SVG required".into()),
    };
    let session = DocumentSession::new(loaded.document().clone())?;
    let profile = evaluate_profiled_with_limits(
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(id.clone(), entry.shared_bytes(), hint)?,
        )
        .for_scene()?,
        EvaluationLimits::default(),
    )?;
    let scene = profile.result.scene();
    let mut records = Vec::new();
    if external_svg {
        for observation in 0..3 {
            let stop = AtomicBool::new(false);
            let polls = AtomicUsize::new(0);
            let record = std::thread::scope(|scope| -> Result<Value> {
                let stopped = &stop;
                let controller = scope.spawn(move || -> std::io::Result<Instant> {
                    let mut line = String::new();
                    std::io::stdin().read_line(&mut line)?;
                    let requested = Instant::now();
                    stopped.store(true, Ordering::SeqCst);
                    Ok(requested)
                });
                println!(
                    "{}",
                    json!({"kind":"svg_cancel_ready","observation":observation})
                );
                std::io::stdout().flush()?;
                let result = toniator_render::write_svg_cancellable_with_limits(
                    scene,
                    toniator_render::SvgSerializationLimits::default(),
                    &|| {
                        polls.fetch_add(1, Ordering::Relaxed);
                        stop.load(Ordering::SeqCst)
                    },
                );
                let returned = Instant::now();
                let requested = controller.join().map_err(|_| "controller panicked")??;
                let error = result.err().ok_or("SVG completed before cancellation")?;
                if error.path() != "svg.cancelled" {
                    return Err(error.into());
                }
                Ok(
                    json!({"kind":"cancel_latency","phase":"svg-external-normalization-marker","observation":observation,"request_to_typed_return_ns":returned.duration_since(requested).as_nanos(),"polls":polls.load(Ordering::Relaxed),"diagnostic":error.path(),"partial_output":false}),
                )
            })?;
            println!("{record}");
            records.push(record);
        }
    } else {
        for source_name in ["png", "svg"] {
            let (document, _, source) = fixture(source_name, "round-crossing")?;
            let field = toniator_sampling::decode_source(
                source.bytes().ok_or("encoded source required")?,
                source.format().ok_or("hint required")?,
            )?;
            let definition = PatternDefinition::parametric_curve(
                PatternDefinitionId(901),
                "Latency profile",
                PatternMechanismId(902),
                None,
                PatternOutputLayerId(903),
                ParametricCurve::Spiral(SpiralCurve {
                    shape: SpiralShape::Round,
                    turns: 5.375,
                    radial_spacing: 16.0,
                    phase_degrees: 37.0,
                    winding: CurveWinding::Clockwise,
                }),
                CurveRepetition::Single,
                None,
                CoveragePolicy {
                    guard_steps: 1,
                    additional_margin: 0.0,
                },
            );
            let plan = resolve_pattern_pipeline(&definition)?;
            let request = GridInspectRequest {
                canvas: document.canvas().clone(),
                density: ResolvedDensityMetric2D {
                    across_x: 16.0,
                    across_y: 16.0,
                },
                rotation_degrees: 13.0,
                translation_x: 7.0,
                translation_y: -9.0,
                guard_steps: 1,
                support_radius: 128.0,
                max_family_candidates: usize::MAX,
            };
            let family = evaluate_typed_family_product_with_curve_limits_progress_cancellable(
                &plan.family,
                &request,
                None,
                CurveWorkLimits::default(),
                &|| false,
                &|_, _| {},
            )?;
            for _ in 0..3 {
                let record = timed_probe_cancel(
                    &format!("{source_name}-adaptive-profile"),
                    128,
                    |cancel| {
                        realize_typed_canonical_strokes_with_response_cancellable(
                            &family,
                            &plan,
                            &field,
                            document.canvas(),
                            SourceMapping::canonical(SourceMappingComponent::Luminance),
                            StrokeResponse {
                                minimum_thickness: 0.0,
                                maximum_thickness: 1.0,
                                bias: 0.0,
                            },
                            1.0,
                            usize::MAX,
                            usize::MAX,
                            GeometryResponseInput::Identity,
                            cancel,
                        )
                        .map(|_| ())
                        .map_err(|e| e.path().to_string())
                    },
                )?;
                println!("{record}");
                records.push(record);
            }
        }
        let stroke = scene
            .layers()
            .iter()
            .flat_map(|layer| layer.outputs())
            .filter_map(|output| match output.geometry() {
                GeometryOutput::CanonicalStrokes(strokes) => Some(strokes),
                _ => None,
            })
            .flatten()
            .max_by_key(|stroke| stroke.profile.len())
            .ok_or("original has no strokes")?;
        let samples = stroke
            .profile
            .iter()
            .map(|sample| VariableWidthPathSample {
                location: sample.location,
                width: sample.width,
            })
            .collect::<Vec<_>>();
        for _ in 0..3 {
            let record = timed_probe_cancel(
                "largest-original-outline-after-validation",
                samples.len() * 2 + 128,
                |cancel| {
                    build_variable_width_outline_cancellable(
                        &stroke.path,
                        &samples,
                        stroke.style,
                        0.0,
                        1.0 / 8.0,
                        VariableWidthOutlineLimits::new(usize::MAX)
                            .map_err(|e| e.path().to_string())?,
                        cancel,
                    )
                    .map(|_| ())
                    .map_err(|e| e.path().to_string())
                },
            )?;
            println!("{record}");
            records.push(record);
        }
    }
    fs::write(
        directory.join("cancel-latency.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    Ok(())
}

/// Selects only explicit stage artifacts, preserving all source/project bytes.
/// `baseline OUT` builds fourteen current source/family cases; `project INPUT OUT [REPEATS]`
/// measures a current still project at native canvas size for later full GoatLocker verification.
///
/// # Errors
/// Returns malformed invocation, current-format load/binding, evaluation, or output failures.
fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("baseline") if (2..=3).contains(&args.len()) => {
            let directory = Path::new(&args[1]);
            for source in ["png", "svg"] {
                for case in [
                    "round-contained",
                    "round-crossing",
                    "square-contained",
                    "square-crossing",
                    "grid",
                    "random",
                    "region",
                ] {
                    let (document, bundle, source_value) = fixture(source, case)?;
                    let destination = directory.join(format!("{source}-{case}"));
                    fs::create_dir_all(&destination)?;
                    toniator_io::save(&destination.join("fixture.toniator"), &document, &bundle)?;
                    run(document, source_value, &destination, args.get(2).map(|s|s.parse()).transpose()?.unwrap_or(2))?;
                }
            }
            Ok(())
        }
        Some("project") if (3..=4).contains(&args.len()) => {
            let loaded = toniator_io::load(Path::new(&args[1]))?;
            let SourceReference::Assigned(id) = loaded.document().source() else {
                return Err("project must assign a still source".into());
            };
            let entry = loaded.sources().get(id).ok_or("missing embedded source")?;
            let hint = match entry.format() {
                EmbeddedSourceFormat::Png => SourceFormatHint::Png,
                EmbeddedSourceFormat::Svg => SourceFormatHint::Svg,
                _ => return Err("probe project source must be PNG or SVG".into()),
            };
            run_native_project(
                loaded.document().clone(),
                ResolvedSource::new(id.clone(), entry.shared_bytes(), hint)?,
                Path::new(&args[2]),
                args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(2),
            )
        }
        Some("preview") if (3..=4).contains(&args.len())=>preview_project(Path::new(&args[1]),Path::new(&args[2]),args.get(3).map(|s|s.parse()).transpose()?),
        Some("latency") if args.len()==3=>latency_project(Path::new(&args[1]),Path::new(&args[2]),false),
        Some("svg-cancel-external") if args.len()==3=>latency_project(Path::new(&args[1]),Path::new(&args[2]),true),
        Some("diagnose") if args.len() == 3 => diagnose_project(Path::new(&args[1]),Path::new(&args[2]),None),
        Some("diagnose-fit") if args.len()==5=>diagnose_project(Path::new(&args[1]),Path::new(&args[2]),Some((args[3].parse()?,args[4].parse()?))),
        Some("failure") if args.len() == 5 => failure(&args[1], &args[2], &args[3], Path::new(&args[4])),
        _ => Err("usage: preview_recovery_probe baseline OUT | project INPUT OUT [REPEATS] | failure SOURCE CASE BOUNDARY OUT".into()),
    }
}
