//! Genuine calibration and bounded production-consumer evidence for explicit current coverage intent.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::PathBuf, time::Instant};
use toniator_domain::*;
use toniator_engine::*;
use toniator_patterns::{ConstructionContext, ConstructionMode};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Prepares genuine exact-binding artifacts or evaluates an explicitly current project.
/// Historical project derivation remains an external, declared evidence operation, never a load adapter.
/// # Errors
/// Returns explicit arguments, load, preparation, evaluation or fresh evidence-directory diagnostics.
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("domains") {
        return domains(&PathBuf::from(&args[2]), &PathBuf::from(&args[3]));
    }
    if args.get(1).map(String::as_str) == Some("movement") {
        if args.len() != 4 {
            return Err("usage: probe movement PROJECT FRESH_OUTPUT".into());
        }
        return movement(&PathBuf::from(&args[2]), &PathBuf::from(&args[3]));
    }
    if args.get(1).map(String::as_str) == Some("case") {
        if args.len() != 5 {
            return Err("usage: probe case CASE SOURCE FRESH_OUTPUT".into());
        }
        return create_case(&args[2], &PathBuf::from(&args[3]), &PathBuf::from(&args[4]));
    }
    if args.get(1).map(String::as_str) == Some("prepare") {
        if !(4..=5).contains(&args.len()) {
            return Err(
                "usage: probe prepare CURRENT_PROJECT FRESH_OUTPUT [GENUINE_DONOR_PROJECT]".into(),
            );
        }
        return prepare(
            &PathBuf::from(&args[2]),
            &PathBuf::from(&args[3]),
            args.get(4).map(PathBuf::from).as_deref(),
        );
    }
    render(&args)
}

/// Creates current gallery or explicitly authored coverage intent on immutable artwork, then genuinely prepares it.
/// Each case stays an ordinary typed recipe; no renderer branch, saved-fit relabel or source edit occurs.
/// # Errors
/// Propagates recipe, artwork decode, current project, strict fit and fresh-directory diagnostics.
fn create_case(case: &str, source: &std::path::Path, out: &std::path::Path) -> Result<()> {
    if out.exists() {
        return Err("fresh case directory required".into());
    }
    fs::create_dir_all(out)?;
    let bytes = fs::read(source)?;
    let svg = source.extension().and_then(|value| value.to_str()) == Some("svg");
    let decoded = toniator_sampling::decode_source(
        &bytes,
        if svg {
            SourceFormatHint::Svg
        } else {
            SourceFormatHint::Png
        },
    )?;
    let canvas = CanvasSpec {
        width: f64::from(decoded.identity().width),
        height: f64::from(decoded.identity().height),
    };
    let id = SourceReferenceId::new("coverage-case-source")?;
    let sources = toniator_io::SourceBundle::new([toniator_io::EmbeddedSource::new(
        id.clone(),
        if svg {
            toniator_io::EmbeddedSourceFormat::Svg
        } else {
            toniator_io::EmbeddedSourceFormat::Png
        },
        bytes,
        None,
    )?])?;
    let registry = toniator_patterns::PresetRegistry::bundled();
    let recipe = if case == "authored-stacked" || case == "authored-normal" {
        let family = PatternStructureRecipe::GenericGuides {
            name: case.into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
            dimensions: vec![GenericGuideDimensionDraft {
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: GenericGuidePrototypeDraft::AuthoredOpenPathReference {
                    resource_index: 0,
                },
                repetition: if case == "authored-normal" {
                    GuideRepetition::NormalOffset {
                        spacing: 16.0,
                        cleanup: OffsetCleanup::DissolveCrossings,
                    }
                } else {
                    GuideRepetition::TransformStack {
                        direction_degrees: 90.0,
                        spacing_multiplier: 1.0,
                    }
                },
            }],
            product: GeneralizedSiteProductDraft::AlongGuides {
                dimension_indices: vec![0],
                interval_multiplier: 1.0,
                phase: 0.0,
            },
            orientation: MarkOrientationDraft::Fixed,
        };
        let mut recipe =
            PatternDefinitionRecipe::connected(PatternStructureRecipe::OrderedOutputs {
                definition: Box::new(family),
                outputs: vec![PatternOutputRealizationRecipe::StructuralPaths {
                    tone_sample_spacing: ToneSampleSpacing::default(),
                    style: Default::default(),
                }],
            });
        recipe.structure = PatternStructureRecipe::AuthoredResources {
            resources: vec![AuthoredStructureDraft::new(
                AuthoredStructureKind::OpenPath,
                vec![AuthoredCurveSegment::CubicBezier {
                    start: AuthoredPoint2 { x: -64.0, y: 0.0 },
                    control_1: AuthoredPoint2 { x: -20.0, y: -24.0 },
                    control_2: AuthoredPoint2 { x: 20.0, y: 0.0 },
                    end: AuthoredPoint2 { x: 64.0, y: 0.0 },
                }],
            )?],
            definition: Box::new(recipe.structure),
        };
        recipe
    } else {
        let requested = if case == "square-spiral-line" {
            "round-spiral-line"
        } else {
            case
        };
        let mut recipe = registry
            .entries()
            .iter()
            .find(|entry| entry.metadata.id == requested)
            .ok_or("unknown case")?
            .recipe
            .clone();
        if case == "square-spiral-line" {
            if let PatternStructureRecipe::ParametricCurve {
                curve: ParametricCurve::Spiral(curve),
                ..
            } = &mut recipe.structure
            {
                curve.shape = SpiralShape::Square;
            } else {
                return Err("square case requires analytic source".into());
            }
        }
        recipe
    };
    let base = Document::new_default_document(canvas, SourceReference::Assigned(id))?;
    let document = base
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: base.pattern_settings().clone(),
            base_definition: base.pattern_definition_bundles()[0].definition.clone(),
            recipe,
        })?
        .0;
    let input = out.join("created-current.toniator");
    toniator_io::save(&input, &document, &sources)?;
    prepare(&input, &out.join("genuine-prepared"), None)
}

/// Resolves the exact binding in a pool of current saved artifacts, otherwise invokes the real solver.
/// Cached artifacts retain their original typed binding/profile/raw payload; numeric relabelling is absent.
/// # Errors
/// Propagates strict reference fitting, bounded evaluation or evidence serialization failures.
fn genuine_artifact(
    binding: &CalibrationBinding,
    pool: &mut Vec<SavedCalibration>,
    records: &mut Vec<serde_json::Value>,
) -> Result<SavedCalibration> {
    if let Some(artifact) = pool
        .iter()
        .find(|artifact| artifact.binding().is_compatible_with(binding))
    {
        return Ok(artifact.clone());
    }
    let started = Instant::now();
    eprintln!(
        "preparing genuine binding {}",
        binding.profile_for_reference(CALIBRATION_PROFILE)
    );
    let artifact = reference_calibration::calibrate_saved_binding(
        binding,
        EvaluationLimits::default(),
        &|| false,
    )?;
    records.push(json!({"profile":artifact.profile(),"binding":format!("{:?}",artifact.binding()),"raw":format!("{:?}",artifact.raw()),"seconds":started.elapsed().as_secs_f64(),"authority":"reference_calibration::calibrate_saved_binding"}));
    pool.push(artifact.clone());
    Ok(artifact)
}

/// Prepares every named and hidden ALL target from exact genuine measurements, preserving authored intent.
/// The fresh evidence project is saved after each completed target so an interrupted fit loses no artifact.
/// # Errors
/// Propagates load, exact binding, genuine solver, strict authority and fresh-directory write diagnostics.
fn prepare(
    input: &std::path::Path,
    out: &std::path::Path,
    donor: Option<&std::path::Path>,
) -> Result<()> {
    if out.exists() {
        return Err("fresh preparation directory required".into());
    }
    fs::create_dir_all(out)?;
    let project = toniator_io::load(input)?;
    let mut document = project.document().clone();
    let mut pool = document
        .saved_calibrations()
        .iter()
        .map(|entry| entry.artifact.clone())
        .collect::<Vec<_>>();
    if let Some(artifact) = document.baseline_saved_calibration()? {
        pool.push(artifact.clone());
    }
    if let Some(path) = donor {
        let donor = toniator_io::load(path)?;
        pool.extend(
            donor
                .document()
                .saved_calibrations()
                .iter()
                .map(|entry| entry.artifact.clone()),
        );
        if let Some(artifact) = donor.document().baseline_saved_calibration()? {
            pool.push(artifact.clone());
        }
    }
    let mut records = Vec::new();
    let started = Instant::now();
    let ids = document
        .channel_topology()
        .ok_or("channel topology")?
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    for id in ids {
        let binding = CalibrationBinding::for_channel(&document, id)?;
        let artifact = genuine_artifact(&binding, &mut pool, &mut records)?;
        document = document.with_saved_calibration(id, artifact)?;
        toniator_io::save(&out.join("prepared.toniator"), &document, project.sources())?;
        fs::write(
            out.join("genuine-fits.json"),
            serde_json::to_vec_pretty(&records)?,
        )?;
    }
    let binding = document.baseline_calibration_binding()?;
    let artifact = genuine_artifact(&binding, &mut pool, &mut records)?;
    document = document.with_saved_baseline_calibration(artifact)?;
    document.require_saved_calibration_authority()?;
    toniator_io::save(&out.join("prepared.toniator"), &document, project.sources())?;
    let reopened = toniator_io::load(&out.join("prepared.toniator"))?;
    assert_eq!(reopened.document(), &document);
    fs::write(
        out.join("genuine-fits.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    fs::write(
        out.join("result.json"),
        serde_json::to_vec_pretty(
            &json!({"new_genuine_fit_calls":records.len(),"pooled_artifacts":pool.len(),"strict_saved_authority":"accepted","save_reopen_equal":true,"seconds":started.elapsed().as_secs_f64()}),
        )?,
    )?;
    Ok(())
}

/// Evaluates the admitted document or a translation-only control through production consumers.
/// Outputs remain native RGBA; the probe neither changes source intent nor relabels saved fits.
/// # Errors
/// Returns current load, strict calibration, evaluation, render, export, and fresh-directory errors.
fn render(args: &[String]) -> Result<()> {
    if !(5..=7).contains(&args.len()) {
        return Err(
            "usage: probe PROJECT OUTPUT complete|canvas exact|zero [intrinsic|main] [FEATURE]"
                .into(),
        );
    }
    let main_preview = args.get(5).map(String::as_str) == Some("main");
    let project = toniator_io::load(&PathBuf::from(&args[1]))?;
    let out = PathBuf::from(&args[2]);
    if out.exists() {
        return Err("fresh output directory required".into());
    }
    fs::create_dir_all(&out)?;
    let mode = match args[3].as_str() {
        "complete" => ConstructionMode::Complete,
        "canvas" => ConstructionMode::Canvas,
        _ => return Err("mode".into()),
    };
    let mut document = project.document().clone();
    if args[4] == "zero" {
        let ids = document
            .channel_topology()
            .ok_or("channels")?
            .channels()
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>();
        for channel_id in ids {
            for edited_axis in [TranslationEditedAxis::X, TranslationEditedAxis::Y] {
                match document.apply_command(&DocumentCommand::SetTranslationAxis {
                    channel_id,
                    edited_axis,
                    value: 0.0,
                }) {
                    Ok((next, _)) => document = next,
                    Err(error)
                        if error.path() == "command"
                            && error.message() == "command is a semantic no-op" => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
    } else if args[4] != "exact" {
        return Err("control".into());
    }
    if let Some(feature) = args.get(6) {
        document = document
            .edit_all_channel_start_configuration(&[(
                PropertyFieldId::Density,
                100.0 / feature.parse::<f64>()?,
            )])?
            .bind(&document)?;
    }
    document.require_saved_calibration_authority()?;
    toniator_io::save(&out.join("fixture.toniator"), &document, project.sources())?;
    fs::write(out.join("intent.txt"), format!("{document:#?}"))?;
    let SourceReference::Assigned(id) = document.source() else {
        return Err("source".into());
    };
    let source = project.sources().get(id).ok_or("embedded source")?;
    let started = Instant::now();
    let request = EvaluationRequest::new(
        DocumentSession::new(document.clone())?.document_evaluation_snapshot(),
        ResolvedSource::new(
            id.clone(),
            source.bytes().to_vec(),
            if source.format() == toniator_io::EmbeddedSourceFormat::Svg {
                SourceFormatHint::Svg
            } else {
                SourceFormatHint::Png
            },
        )?,
    )
    .for_scene()?;
    let context = ConstructionContext::new(mode, None, true);
    let evaluated = context.run(|| {
        evaluate_with_limits(
            request,
            EvaluationLimits::default().with_construction_mode(mode),
        )
    })?;
    let mut families = Vec::new();
    for channel in document.channel_topology().ok_or("channels")?.channels() {
        let family = evaluated.family_output(channel.id).ok_or("family")?;
        let logical = family.logical_tone_paths().iter().map(|p| Ok(json!({
            "id":format!("{:?}",p.instance.id), "bounds":format!("{:?}",p.instance.path.bounds()?),
            "start":format!("{:?}",p.instance.path.start()), "end":format!("{:?}",p.instance.path.end()),
            "segments":p.instance.path.segments().len(), "nominal_basis":p.nominal_basis,
        }))).collect::<Result<Vec<_>>>()?;
        families.push(json!({"channel":channel.id.0,"role":format!("{:?}",channel.role),"fingerprint":family.family_fingerprint(),"support":family.planned_support_radius(),"structural_paths":family.structural_path_set().map(|s|s.paths().len()),"logical_paths":logical}));
    }
    let scene = evaluated.scene();
    fs::write(
        out.join("scene-identity.txt"),
        format!("{:#?}", scene.identity()),
    )?;
    let mut native_hash = None;
    let mut svg_hash = None;
    let mut stream_equal = None;
    let mut svg_editable = None;
    if !main_preview {
        let native =
            context.run(|| toniator_render::rasterize(scene, RasterBackground::Transparent))?;
        fs::write(out.join("native.rgba"), native.pixels())?;
        fs::write(out.join("native.png"), encode_png(&native)?)?;
        native_hash = Some(format!("{:x}", Sha256::digest(native.pixels())));
        let mut stream = Vec::new();
        context.run(|| {
            write_png_streaming(
                scene,
                StreamingRasterTarget::for_canvas(document.canvas()).unwrap(),
                PngStreamOptions::default(),
                RasterizationLimits::default(),
                &mut stream,
                &|| false,
                &|_, _| {},
            )
        })?;
        let decoded = image::load_from_memory(&stream)?.to_rgba8();
        if decoded.as_raw() != native.pixels() {
            return Err("stream/native decoded pixel mismatch".into());
        }
        stream_equal = Some(true);
        fs::write(out.join("stream.png"), stream)?;
        let svg = context.run(|| write_svg(scene))?;
        svg_hash = Some(format!("{:x}", Sha256::digest(svg.as_bytes())));
        svg_editable = Some(!svg.contains("<image"));
        fs::write(out.join("native.svg"), &svg)?;
        let consumer = toniator_sampling::reduced_preview_png(
            svg.as_bytes(),
            SourceFormatHint::Svg,
            document.canvas().width.max(document.canvas().height) as u32,
        )?;
        fs::write(out.join("svg-consumer.png"), consumer.png_bytes)?;
    }
    let target = if main_preview {
        PreviewRasterTarget::new(3118, 1916)?
    } else {
        PreviewRasterTarget::new(1024, 1024)?
    }
    .with_background(RasterBackground::OpaqueBlack);
    let preview = context.run(|| toniator_render::rasterize_preview(scene, target))?;
    fs::write(out.join("preview.png"), encode_png(&preview)?)?;
    let record = json!({"mode":format!("{mode:?}"),"control":args[4],"feature":args.get(6),"main_preview":main_preview,"preview_dimensions":[preview.width(),preview.height()],"preview_rgba_sha256":format!("{:x}",Sha256::digest(preview.pixels())),"strict_saved_calibration":"accepted unchanged","source_sha256":format!("{:x}",Sha256::digest(source.bytes())),"native_rgba_sha256":native_hash,"stream_decoded_pixels_exact":stream_equal,"svg_sha256":svg_hash,"svg_editable":svg_editable,"families":families,"construction":context.measurements.as_ref().unwrap().snapshot(),"seconds":started.elapsed().as_secs_f64()});
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&record)?)?;
    println!("{record}");
    Ok(())
}

/// Serializes only the authoritative saved artifacts and authored response endpoints from a current Preset.
/// Placement fields are deliberately excluded; every named and hidden ALL numeric/profile/binding byte remains included.
/// # Errors
/// Propagates archive persistence, JSON decoding and current destination validation failures.
fn authority_bytes(document: &Document, path: &std::path::Path) -> Result<Vec<u8>> {
    use std::io::Read;
    toniator_io::save_document_preset(
        path,
        &DocumentConfiguration::capture(document),
        &toniator_io::capture_document_preset_destination(path)?,
    )?;
    let mut archive = zip::ZipArchive::new(fs::File::open(path)?)?;
    let mut bytes = Vec::new();
    archive.by_name("preset.json")?.read_to_end(&mut bytes)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let configuration = &value["configuration"];
    let mut authority = serde_json::Map::new();
    for field in [
        "saved_calibrations",
        "baseline_saved_calibration",
        "response_settings",
        "channel_response_overrides",
        "end_overrides",
    ] {
        authority.insert(field.into(), configuration[field].clone());
    }
    Ok(serde_json::to_vec(&authority)?)
}

/// Carries one document through zero, reported, combined X/Y/rotation and restored-zero commands in order.
/// Cold and exact-warm evaluations cannot invoke a fitter; full authority bytes and native pixels are compared.
/// Every transition records its actual prior/next placement and applied/no-op command counts.
/// # Errors
/// Returns command, exact-authority, evaluation, pixel parity or fresh evidence diagnostics.
fn movement(input: &std::path::Path, out: &std::path::Path) -> Result<()> {
    if out.exists() {
        return Err("fresh movement directory required".into());
    }
    fs::create_dir_all(out)?;
    let project = toniator_io::load(input)?;
    let mut seeded = project.document().clone();
    let output = seeded
        .effective_channel_pattern(ChannelId(1))?
        .output_settings[0]
        .output_layer_id;
    let mut response = seeded
        .effective_channel_pattern(ChannelId(1))?
        .output_settings[0]
        .response
        .clone();
    if let PatternGeometryResponse::Connected(value) = &mut response {
        value.maximum_thickness = 0.71;
        let command =
            seeded.set_channel_output_response_for_effective(ChannelId(1), output, response)?;
        seeded = seeded.apply_command(&command)?.0;
        let initialized = seeded.initialize_end_command()?;
        seeded = seeded.with_temporal_authority(
            ProjectTiming::new(FrameRate::new(24, 1)?, FrameRange::new(0, 2)?),
            initialized.replacement().end_overrides.clone(),
        )?;
        let end = seeded.edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::ChannelOutput(ChannelId(1), output),
            field: PropertyFieldId::ConnectedMaximumThickness,
            effective_end: 0.83,
            easing: Easing::SmoothStep,
        }])?;
        seeded = seeded.with_temporal_authority(
            end.replacement().project_timing.clone(),
            end.replacement().end_overrides.clone(),
        )?;
        if seeded.temporal_end_overrides().is_empty() {
            return Err("nonempty authored End required".into());
        }
    }
    let original = &seeded;
    original.require_saved_calibration_authority()?;
    let expected = authority_bytes(original, &out.join("original.toniator-preset"))?;
    fs::write(out.join("authority.json"), &expected)?;
    let ids = original
        .channel_topology()
        .ok_or("channels")?
        .channels()
        .iter()
        .map(|c| c.id)
        .collect::<Vec<_>>();
    let mut records = Vec::new();
    let mut zero_pixels = None;
    let mut document = original.clone();
    let mut previous_pose = "input";
    for pose in ["zero", "reported", "combined", "restored-zero"] {
        let from_placement = placement_state(&document)?;
        let mut applied_commands = 0;
        let mut semantic_noops = 0;
        for (index, &channel_id) in ids.iter().enumerate() {
            let (x, y, rotation) = match pose {
                "reported" => {
                    let effective = original.effective_channel_pattern(channel_id)?;
                    (
                        effective.translation_x,
                        effective.translation_y,
                        effective.pattern_rotation_degrees
                            - original.pattern_settings().pattern_rotation_degrees,
                    )
                }
                "combined" => [
                    (-384.0, 512.0, 37.0),
                    (320.0, -448.0, -29.0),
                    (-256.0, 384.0, 91.0),
                ][index % 3],
                _ => (0.0, 0.0, 0.0),
            };
            let commands = [
                DocumentCommand::SetTranslationAxis {
                    channel_id,
                    edited_axis: TranslationEditedAxis::X,
                    value: x,
                },
                DocumentCommand::SetTranslationAxis {
                    channel_id,
                    edited_axis: TranslationEditedAxis::Y,
                    value: y,
                },
                DocumentCommand::SetChannelPatternRotationDelta {
                    base: document.pattern_settings().clone(),
                    channel_id,
                    rotation_degrees: rotation,
                },
            ];
            for command in commands {
                match document.apply_command(&command) {
                    Ok((next, _)) => {
                        document = next;
                        applied_commands += 1;
                    }
                    Err(error)
                        if error.path() == "command"
                            && error.message() == "command is a semantic no-op" =>
                    {
                        semantic_noops += 1;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        }
        document.require_saved_calibration_authority()?;
        let actual = authority_bytes(&document, &out.join(format!("{pose}.toniator-preset")))?;
        if actual != expected {
            return Err(
                format!("{pose}: artifact/profile/binding/response/End bytes changed").into(),
            );
        }
        toniator_io::save(
            &out.join(format!("{pose}.toniator")),
            &document,
            project.sources(),
        )?;
        let SourceReference::Assigned(id) = document.source() else {
            return Err("source".into());
        };
        let source = project.sources().get(id).ok_or("source")?;
        let request = || -> Result<EvaluationRequest> {
            Ok(EvaluationRequest::new(
                DocumentSession::new(document.clone())?.document_evaluation_snapshot(),
                ResolvedSource::new(
                    id.clone(),
                    source.bytes().to_vec(),
                    if source.format() == toniator_io::EmbeddedSourceFormat::Svg {
                        SourceFormatHint::Svg
                    } else {
                        SourceFormatHint::Png
                    },
                )?,
            )
            .for_scene()?)
        };
        let mut cache = EvaluationProfileCache::default();
        let limits = EvaluationLimits::default().with_construction_mode(ConstructionMode::Canvas);
        let cold = evaluate_profiled_cached_with_limits(request()?, limits, &mut cache)?;
        let warm = evaluate_profiled_cached_with_limits(request()?, limits, &mut cache)?;
        if !warm
            .diagnostics
            .channels
            .iter()
            .all(|c| c.family == CacheDisposition::Hit && c.realization == CacheDisposition::Hit)
        {
            return Err("exact-warm cache miss".into());
        }
        let native =
            toniator_render::rasterize(cold.result.scene(), RasterBackground::Transparent)?;
        let repeated =
            toniator_render::rasterize(warm.result.scene(), RasterBackground::Transparent)?;
        if native.pixels() != repeated.pixels() {
            return Err("cold/warm pixel mismatch".into());
        }
        if pose == "zero" {
            zero_pixels = Some(native.pixels().to_vec());
        }
        if pose == "restored-zero" && zero_pixels.as_deref() != Some(native.pixels()) {
            return Err("restored zero differs".into());
        }
        fs::write(out.join(format!("{pose}.png")), encode_png(&native)?)?;
        records.push(json!({"pose":pose,"from_pose":previous_pose,"from_placement":from_placement,"to_placement":placement_state(&document)?,"attempted_commands":ids.len()*3,"applied_commands":applied_commands,"semantic_noops":semantic_noops,"authority_bytes_sha256":format!("{:x}",Sha256::digest(&actual)),"native_rgba_sha256":format!("{:x}",Sha256::digest(native.pixels())),"strict_saved_authority":"accepted","new_fit_calls":0,"cold_diagnostics":format!("{:?}",cold.diagnostics),"warm_diagnostics":format!("{:?}",warm.diagnostics),"cold_warm_pixels_exact":true}));
        previous_pose = pose;
    }
    fs::write(
        out.join("result.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    Ok(())
}

/// Records final canonical component domains against exactly the public consumer arc metric.
/// This diagnostic performs no fit, tolerance change, source mutation or tone realization.
/// # Errors
/// Returns current load, family, arc metric and fresh evidence diagnostics.
fn domains(input: &std::path::Path, out: &std::path::Path) -> Result<()> {
    let project = toniator_io::load(input)?;
    let document = project.document();
    let mut records = Vec::new();
    for id in [ChannelId(1), ChannelId(2), ChannelId(3)] {
        let effective = document.effective_channel_pattern(id)?;
        let definition = document.pattern_definition_for(id).ok_or("definition")?;
        let family = toniator_patterns::evaluate_document_typed_family_cancellable(
            document,
            definition,
            &toniator_patterns::GridInspectRequest {
                canvas: document.canvas().clone(),
                density: effective.resolved_density,
                rotation_degrees: effective.pattern_rotation_degrees,
                translation_x: effective.translation_x,
                translation_y: effective.translation_y,
                guard_steps: definition.coverage.guard_steps,
                support_radius: 16.0,
                max_family_candidates: 200_000,
            },
            &|| false,
        )?;
        let mut previous: Option<(String, f64)> = None;
        for logical in family.logical_tone_paths() {
            let key = format!(
                "{:?}:{}",
                logical.instance.id.source, logical.instance.id.repetition_index
            );
            let length = logical
                .instance
                .path
                .measure_arc_length_cancellable(Default::default(), &|| false)?
                .total_length();
            let start = logical.domain.start_distance;
            let end = start + length;
            if let Some((old_key, old_end)) = &previous
                && old_key == &key
            {
                let tolerance = 32.0 * f64::EPSILON * end.abs().max(start.abs()).max(1.0);
                if (start - old_end).abs() > tolerance {
                    records.push(json!({"channel":id.0,"id":format!("{:?}",logical.instance.id),"start":start,"previous_end":old_end,"gap":start-old_end,"tolerance":tolerance,"start_bits":start.to_bits(),"previous_end_bits":old_end.to_bits(),"length":length,"end":end,"segments":logical.instance.path.segments().len()}));
                }
            }
            previous = Some((key, end));
        }
    }
    fs::write(out, serde_json::to_vec_pretty(&records)?)?;
    Ok(())
}

/// Captures actual effective channel placement for an independently inspectable sequential transition.
/// # Errors
/// Propagates missing topology or invalid effective-channel diagnostics without altering document state.
fn placement_state(document: &Document) -> Result<serde_json::Value> {
    let mut channels = Vec::new();
    for channel in document.channel_topology().ok_or("channels")?.channels() {
        let effective = document.effective_channel_pattern(channel.id)?;
        channels.push(json!({"channel":channel.id.0,"x":effective.translation_x,"y":effective.translation_y,"rotation_degrees":effective.pattern_rotation_degrees}));
    }
    Ok(json!(channels))
}
