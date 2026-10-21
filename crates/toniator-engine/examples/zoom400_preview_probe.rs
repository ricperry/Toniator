//! Exact current-project Main viewport diagnostics with fresh evidence and no authored edits.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::PathBuf, sync::Mutex, time::Instant};
use toniator_domain::*;
use toniator_engine::*;
use toniator_patterns::{ConstructionContext, ConstructionMode};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Evaluates saved authority once, then rasterizes one exact Main target with normal Main work policy.
/// Optional canonical pan origins retain the same physical target and shared Main work limits.
/// No fitting, source/settings changes or complete-scene debug serialization occurs.
/// # Errors
/// Returns argument, current-project, saved-authority, evaluation and fresh evidence I/O errors.
/// # Panics
/// Panics only if the nonzero default raster policy or requested construction measurement invariant breaks.
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 5 && args.len() != 7 {
        return Err(
            "usage: probe PROJECT FRESH_OUTPUT canvas|complete SCALE [ORIGIN_X ORIGIN_Y]".into(),
        );
    }
    let out = PathBuf::from(&args[2]);
    if out.exists() {
        return Err("fresh output required".into());
    }
    fs::create_dir_all(&out)?;
    let project = toniator_io::load(&PathBuf::from(&args[1]))?;
    let document = project.document().clone();
    document.require_saved_calibration_authority()?;
    let mode = match args[3].as_str() {
        "canvas" => ConstructionMode::Canvas,
        "complete" => ConstructionMode::Complete,
        _ => return Err("mode".into()),
    };
    let scale = args[4].parse::<f64>()?;
    let origin = if args.len() == 7 {
        [args[5].parse::<f64>()?, args[6].parse::<f64>()?]
    } else {
        [0.0, 0.0]
    };
    let target = PreviewRasterTarget::for_viewport(3118, 1916, origin[0], origin[1], scale)?
        .with_background(RasterBackground::OpaqueBlack);
    let SourceReference::Assigned(id) = document.source() else {
        return Err("embedded source required".into());
    };
    let source = project.sources().get(id).ok_or("embedded source missing")?;
    fs::write(out.join("document-intent.txt"), format!("{document:#?}"))?;
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
    fs::write(out.join("scene-counts.json"),serde_json::to_vec_pretty(&scene.layers().iter().map(|layer|json!({"channel":layer.channel_id().0,"outputs":layer.outputs().iter().map(|output|json!({"id":output.output_layer_id.0,"primitives":output.geometry().primitive_count()})).collect::<Vec<_>>()})).collect::<Vec<_>>())?)?;
    let evaluation_seconds = started.elapsed().as_secs_f64();
    let before = context.measurements.as_ref().unwrap().snapshot();
    let progress = Mutex::new((0_usize, 0_usize));
    let limits =
        RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges()).unwrap();
    let raster_started = Instant::now();
    let rendered = context.run(|| {
        toniator_render::rasterize_preview_cancellable_with_progress(
            scene,
            target,
            limits,
            &|| false,
            &|completed, total| {
                *progress.lock().unwrap() = (completed, total);
            },
        )
    });
    let raster_seconds = raster_started.elapsed().as_secs_f64();
    let (outcome, error) = match rendered {
        Ok(surface) => {
            fs::write(out.join("preview.rgba"), surface.pixels())?;
            fs::write(out.join("preview.png"), encode_png(&surface)?)?;
            ("success", None)
        }
        Err(error) => ("failed", Some(error.to_string())),
    };
    let after = context.measurements.as_ref().unwrap().snapshot();
    let record = json!({"project_sha256":format!("{:x}",Sha256::digest(fs::read(&args[1])?)),"source_sha256":format!("{:x}",Sha256::digest(source.bytes())),"calibration":"exact saved authority; no fit requested","mode":format!("{mode:?}"),"scale":scale,"logical_zoom_percent":scale*50.,"target":format!("{target:?}"),"outcome":outcome,"error":error,"full_rematerializations":after.full_rematerializations-before.full_rematerializations,"seconds":started.elapsed().as_secs_f64(),"evaluation_seconds":evaluation_seconds,"raster_seconds":raster_seconds,"origin":origin,"completed_logical_work":*progress.lock().unwrap(),"raster_limits":{"flattened_edges":limits.max_flattened_edges(),"boolean_operations":limits.max_boolean_operations()},"construction_before":before,"construction_after":after,"work_observation":"Progress and construction counters are observed; internal actual edge/Boolean counts are not exposed by the public renderer API"});
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&record)?)?;
    println!("{record}");
    Ok(())
}
