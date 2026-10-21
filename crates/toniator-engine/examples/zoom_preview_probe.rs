//! Read-only current-project Main-preview diagnostic; evidence directories must be fresh.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::PathBuf, time::Instant};
use toniator_domain::*;
use toniator_engine::*;
use toniator_patterns::{ConstructionContext, ConstructionMode};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Creates a natural-size immutable-artwork control through the current bundled Triagrid command.
/// The genuine compatible companion supplies saved authority; no measurement or relabelling occurs.
/// # Errors
/// Returns fresh-directory, source decode, registry, authority and portable-save errors.
fn prepare_control(source_path: &str, out: &std::path::Path) -> Result<()> {
    if out.exists() {
        return Err("fresh control directory required".into());
    }
    fs::create_dir_all(out)?;
    let bytes = fs::read(source_path)?;
    let svg = source_path.ends_with(".svg");
    let hint = if svg {
        SourceFormatHint::Svg
    } else {
        SourceFormatHint::Png
    };
    let decoded = toniator_sampling::decode_source(&bytes, hint)?;
    let id = SourceReferenceId::new("immutable-zoom-control")?;
    let document = Document::new_default_document(
        CanvasSpec {
            width: f64::from(decoded.identity().width),
            height: f64::from(decoded.identity().height),
        },
        SourceReference::Assigned(id.clone()),
    )?;
    let mut history = DocumentHistory::new(DocumentSession::new(document)?);
    toniator_patterns::PresetRegistry::bundled()
        .apply_to_document_base(&mut history, "triagrid-spanning-tree")?;
    history.document().require_saved_calibration_authority()?;
    let sources = toniator_io::SourceBundle::new([toniator_io::EmbeddedSource::new(
        id,
        if svg {
            toniator_io::EmbeddedSourceFormat::Svg
        } else {
            toniator_io::EmbeddedSourceFormat::Png
        },
        bytes,
        None,
    )?])?;
    toniator_io::save(&out.join("fixture.toniator"), history.document(), &sources)?;
    fs::write(
        out.join("authority.txt"),
        format!("{:#?}", history.document()),
    )?;
    Ok(())
}

/// Evaluates the exact saved document and calibration through the production scene/Main raster route.
/// Only construction mode and preview presentation target vary; authored state stays untouched.
/// Intrinsic controls also retain native PNG/RGBA and editable SVG with an independent resvg decode.
/// # Errors
/// Returns argument, current-project, saved-authority, evaluation and fresh evidence I/O errors.
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("prepare") && args.len() == 4 {
        return prepare_control(&args[2], &PathBuf::from(&args[3]));
    }
    if args.len() != 5 {
        return Err("usage: probe PROJECT FRESH_OUTPUT complete|canvas fit|zoom|intrinsic".into());
    }
    let project = toniator_io::load(&PathBuf::from(&args[1]))?;
    let out = PathBuf::from(&args[2]);
    if out.exists() {
        return Err("fresh output required".into());
    }
    fs::create_dir_all(&out)?;
    let mode = match args[3].as_str() {
        "complete" => ConstructionMode::Complete,
        "canvas" => ConstructionMode::Canvas,
        _ => return Err("mode".into()),
    };
    let target = match args[4].as_str() {
        "fit" => PreviewRasterTarget::new(3118, 1916)?,
        "zoom" => PreviewRasterTarget::for_viewport(3118, 1916, 0., 0., 3.)?,
        "intrinsic" => PreviewRasterTarget::new(
            project.document().canvas().width as u32,
            project.document().canvas().height as u32,
        )?,
        _ => return Err("target".into()),
    }
    .with_background(RasterBackground::OpaqueBlack);
    let document = project.document().clone();
    document.require_saved_calibration_authority()?;
    fs::write(out.join("intent.txt"), format!("{document:#?}"))?;
    let SourceReference::Assigned(id) = document.source() else {
        return Err("source required".into());
    };
    let source = project.sources().get(id).ok_or("embedded source")?;
    fs::write(out.join("embedded-source.bin"), source.bytes())?;
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
    let started = Instant::now();
    let evaluated = context.run(|| {
        evaluate_with_limits(
            request,
            EvaluationLimits::default().with_construction_mode(mode),
        )
    })?;
    let scene = evaluated.scene();
    fs::write(
        out.join("scene-identity.txt"),
        format!("{:#?}", scene.identity()),
    )?;
    fs::write(
        out.join("scene-summary.json"),
        serde_json::to_vec_pretty(
            &scene
                .layers()
                .iter()
                .map(|layer| {
                    format!(
                        "channel={:?} outputs={}",
                        layer.channel_id(),
                        layer.outputs().len()
                    )
                })
                .collect::<Vec<_>>(),
        )?,
    )?;
    let before = context.measurements.as_ref().unwrap().snapshot();
    let rendered = context.run(|| {
        toniator_render::rasterize_preview_cancellable(
            scene,
            target,
            RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())
                .unwrap(),
            &|| false,
        )
    });
    let (outcome, error) = match rendered {
        Ok(surface) => {
            fs::write(out.join("preview.rgba"), surface.pixels())?;
            fs::write(out.join("preview.png"), encode_png(&surface)?)?;
            ("success", None)
        }
        Err(error) => ("failed", Some(error.to_string())),
    };
    if args[4] == "intrinsic" && error.is_none() {
        let native = context.run(|| {
            toniator_render::rasterize_cancellable(
                scene,
                RasterBackground::Transparent,
                RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())
                    .unwrap(),
                &|| false,
            )
        })?;
        fs::write(out.join("native.rgba"), native.pixels())?;
        fs::write(out.join("native.png"), encode_png(&native)?)?;
        let svg = context.run(|| write_svg(scene))?;
        if svg.contains("<image") {
            return Err("editable SVG required".into());
        }
        fs::write(out.join("native.svg"), &svg)?;
        let consumer = toniator_sampling::reduced_preview_png(
            svg.as_bytes(),
            SourceFormatHint::Svg,
            document.canvas().width.max(document.canvas().height) as u32,
        )?;
        fs::write(out.join("svg-consumer.png"), consumer.png_bytes)?;
    }
    let after = context.measurements.as_ref().unwrap().snapshot();
    let record = json!({"mode":format!("{mode:?}"),"project_sha256":format!("{:x}",Sha256::digest(fs::read(&args[1])?)),"source_sha256":format!("{:x}",Sha256::digest(source.bytes())),"canvas":[document.canvas().width,document.canvas().height],"calibration":"exact saved; zero fits requested","target":format!("{target:?}"),"outcome":outcome,"error":error,"rematerializations":after.full_rematerializations-before.full_rematerializations,"seconds":started.elapsed().as_secs_f64()});
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&record)?)?;
    println!("{record}");
    Ok(())
}
