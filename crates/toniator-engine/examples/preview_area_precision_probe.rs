//! Bounded current-gallery precision verification; outputs require an explicit fresh directory.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::PathBuf, time::Instant};
use toniator_domain::*;
use toniator_engine::*;
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle};
use toniator_patterns::{ConstructionContext, ConstructionMode};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Applies the actual gallery to a fresh document and reuses only genuinely compatible saved fits.
/// Different intrinsic recipient bindings receive their own strict current fit, never a relabelled donor.
/// # Errors
/// Returns current recipe, exact binding, fit, source or document-validation diagnostics.
fn prepare_document(
    donor: &Document,
    canvas: CanvasSpec,
    feature: f64,
    intrinsic: bool,
) -> Result<(Document, usize)> {
    let registry = toniator_patterns::PresetRegistry::bundled();
    let gallery = registry
        .entries()
        .iter()
        .find(|p| p.metadata.id == "round-spiral-line")
        .ok_or("gallery missing")?;
    let base = Document::new_default_document(
        canvas,
        SourceReference::Assigned(SourceReferenceId::new("precision-user-source")?),
    )?;
    let mut document = base
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: base.pattern_settings().clone(),
            base_definition: base.pattern_definition_bundles()[0].definition.clone(),
            recipe: gallery.recipe.clone(),
        })?
        .0;
    let recipe = document
        .reconstruct_pattern_definition_recipe(document.pattern_settings().definition_id)?;
    if !intrinsic
        && recipe
            != donor
                .reconstruct_pattern_definition_recipe(donor.pattern_settings().definition_id)?
    {
        return Err("materialized current gallery differs from genuine recipient".into());
    }
    let mut pool = donor
        .saved_calibrations()
        .iter()
        .map(|e| e.artifact.clone())
        .collect::<Vec<_>>();
    if let Some(artifact) = donor.baseline_saved_calibration()? {
        pool.push(artifact.clone());
    }
    let channels = document
        .channel_topology()
        .ok_or("channels")?
        .channels()
        .iter()
        .map(|c| c.id)
        .collect::<Vec<_>>();
    let mut fits = 0;
    for channel in channels {
        let binding = CalibrationBinding::for_channel(&document, channel)?;
        let artifact = if let Some(artifact) = pool
            .iter()
            .find(|a| a.binding().is_compatible_with(&binding))
        {
            artifact.clone()
        } else {
            fits += 1;
            let artifact = reference_calibration::calibrate_saved_binding(
                &binding,
                EvaluationLimits::default(),
                &|| false,
            )?;
            pool.push(artifact.clone());
            artifact
        };
        document = document.with_saved_calibration(channel, artifact)?;
    }
    let binding = document.baseline_calibration_binding()?;
    let artifact = if let Some(artifact) = pool
        .iter()
        .find(|a| a.binding().is_compatible_with(&binding))
    {
        artifact.clone()
    } else {
        fits += 1;
        reference_calibration::calibrate_saved_binding(
            &binding,
            EvaluationLimits::default(),
            &|| false,
        )?
    };
    document = document.with_saved_baseline_calibration(artifact)?;
    document = document
        .edit_all_channel_start_configuration(&[(PropertyFieldId::Density, 100.0 / feature)])?
        .bind(&document)?;
    document.require_saved_calibration_authority()?;
    Ok((document, fits))
}

/// Records one exact user preview or intrinsic control through current production consumers.
/// Intrinsic mode additionally retains native PNG/raw RGBA, streamed PNG and editable SVG/decoder PNG.
/// # Errors
/// Returns current preparation/evaluation/rendering or evidence I/O errors; no old artifact is overwritten.
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 6 || args.len() > 7 {
        return Err("usage: probe DONOR SOURCE OUTPUT complete|canvas FEATURE [intrinsic]".into());
    }
    let donor = toniator_io::load(&PathBuf::from(&args[1]))?;
    let bytes = fs::read(&args[2])?;
    let out = PathBuf::from(&args[3]);
    if out.join("result.json").exists() {
        return Err("fresh output required".into());
    }
    fs::create_dir_all(&out)?;
    let intrinsic = args.get(6).map(String::as_str) == Some("intrinsic");
    let hint = if args[2].ends_with(".svg") {
        SourceFormatHint::Svg
    } else {
        SourceFormatHint::Png
    };
    let decoded = toniator_sampling::decode_source(&bytes, hint)?;
    let canvas = CanvasSpec {
        width: f64::from(decoded.identity().width),
        height: f64::from(decoded.identity().height),
    };
    if !intrinsic && (canvas.width != 1024. || canvas.height != 1024.) {
        return Err("exact user canvas must be1024square".into());
    }
    let feature = args[5].parse::<f64>()?;
    let mode = match args[4].as_str() {
        "complete" => ConstructionMode::Complete,
        "canvas" => ConstructionMode::Canvas,
        _ => return Err("mode".into()),
    };
    let started = Instant::now();
    let (document, fresh_fits) =
        prepare_document(donor.document(), canvas.clone(), feature, intrinsic)?;
    let SourceReference::Assigned(id) = document.source() else {
        return Err("source required".into());
    };
    let id = id.clone();
    let bundle = SourceBundle::new([EmbeddedSource::new(
        id.clone(),
        if hint == SourceFormatHint::Svg {
            EmbeddedSourceFormat::Svg
        } else {
            EmbeddedSourceFormat::Png
        },
        bytes.clone(),
        Some("Precision verification source".into()),
    )?])?;
    toniator_io::save(&out.join("fixture.toniator"), &document, &bundle)?;
    fs::write(out.join("intent.txt"), format!("{document:#?}"))?;
    let request = EvaluationRequest::new(
        DocumentSession::new(document)?.document_evaluation_snapshot(),
        ResolvedSource::new(id.clone(), bytes.clone(), hint)?,
    )
    .for_scene()?;
    let context = ConstructionContext::new(mode, None, true);
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
    let before = context.measurements.as_ref().unwrap().snapshot();
    let target =
        PreviewRasterTarget::new(3118, 1916)?.with_background(RasterBackground::OpaqueBlack);
    let rendered = context.run(|| toniator_render::rasterize_preview(scene, target));
    let (outcome, error) = match rendered {
        Ok(surface) => {
            fs::write(out.join("preview.rgba"), surface.pixels())?;
            fs::write(out.join("preview.png"), encode_png(&surface)?)?;
            ("success", None)
        }
        Err(error) => ("failed", Some(error.to_string())),
    };
    let after_preview = context.measurements.as_ref().unwrap().snapshot();
    let mut exports = json!(null);
    if intrinsic && error.is_none() {
        let native =
            context.run(|| toniator_render::rasterize(scene, RasterBackground::Transparent))?;
        fs::write(out.join("native.rgba"), native.pixels())?;
        fs::write(out.join("native.png"), encode_png(&native)?)?;
        let after_native = context.measurements.as_ref().unwrap().snapshot();
        let mut stream = Vec::new();
        context.run(|| {
            write_png_streaming(
                scene,
                StreamingRasterTarget::for_canvas(&canvas).unwrap(),
                PngStreamOptions::default(),
                RasterizationLimits::default(),
                &mut stream,
                &|| false,
                &|_, _| {},
            )
        })?;
        fs::write(out.join("stream.png"), stream)?;
        let after_stream = context.measurements.as_ref().unwrap().snapshot();
        let svg = context.run(|| write_svg(scene))?;
        fs::write(out.join("native.svg"), &svg)?;
        let consumer = toniator_sampling::reduced_preview_png(
            svg.as_bytes(),
            SourceFormatHint::Svg,
            decoded.identity().width.max(decoded.identity().height),
        )?;
        fs::write(out.join("svg-consumer.png"), consumer.png_bytes)?;
        exports = json!({"native_rematerializations":after_native.full_rematerializations-after_preview.full_rematerializations,"stream_rematerializations":after_stream.full_rematerializations-after_native.full_rematerializations,"svg_editable":!svg.contains("<image"),"native_dimensions":[native.width(),native.height()]});
    }
    let record = json!({"mode":format!("{mode:?}"),"fixture_recipe":"fresh current gallery via authoritative command","calibration":"genuine compatible donor or separately measured strict fit","new_strict_fits":fresh_fits,"feature":feature,"density":100.0/feature,"canvas":[canvas.width,canvas.height],"source_path":args[2],"source_sha256":format!("{:x}",Sha256::digest(&bytes)),"target":format!("{target:?}"),"outcome":outcome,"error":error,"preview_rematerializations":after_preview.full_rematerializations-before.full_rematerializations,"exports":exports,"seconds":started.elapsed().as_secs_f64()});
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&record)?)?;
    println!("{record}");
    Ok(())
}
