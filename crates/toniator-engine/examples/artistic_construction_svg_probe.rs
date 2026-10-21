//! Independent current SVG decoder verification for the final intrinsic complete/canvas pairs.
use serde_json::json;
use std::{error::Error, fs, path::PathBuf};
use toniator_sampling::{SourceFormatHint, decode_source, reduced_preview_png};
#[allow(dead_code)]
#[path = "artistic_construction_probe.rs"]
mod fixtures;
/// Verifies real intrinsic recipes across larger anisotropic PNG and smaller fitted preview targets.
/// # Errors
/// Returns source, evaluation, exact native pixel comparison or evidence I/O failures.
fn verify_scaled_pairs(root: &std::path::Path) -> Result<(), Box<dyn Error>> {
    use toniator_domain::{CanvasSpec, DocumentSession, SourceReferenceId};
    use toniator_engine::*;
    use toniator_patterns::ConstructionMode;
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut records = Vec::new();
    for source in ["raster", "vector"] {
        for kind in ["marks", "grid", "round", "motif", "connection"] {
            let (input, hint, canvas) = if source == "raster" {
                (
                    "raster-sample.png",
                    SourceFormatHint::Png,
                    CanvasSpec {
                        width: 1024.,
                        height: 1024.,
                    },
                )
            } else {
                (
                    "vector-sample.svg",
                    SourceFormatHint::Svg,
                    CanvasSpec {
                        width: 900.,
                        height: 620.,
                    },
                )
            };
            let bytes = fs::read(project.join("assets").join(input))?;
            let id = SourceReferenceId::new(format!("construction-{source}-{kind}"))?;
            let document = fixtures::fixture(kind, canvas.clone(), id.clone())?;
            let request = EvaluationRequest::new(
                DocumentSession::new(document)?.document_evaluation_snapshot(),
                ResolvedSource::new(id, bytes, hint)?,
            )
            .for_scene()?;
            let out = root
                .join("scaled-consumer")
                .join(format!("{source}-{kind}"));
            fs::create_dir_all(&out)?;
            let mut full = None;
            for mode in [ConstructionMode::Complete, ConstructionMode::Canvas] {
                let result = evaluate_with_limits(
                    request.clone(),
                    EvaluationLimits::default().with_construction_mode(mode),
                )?;
                let larger = toniator_render::rasterize_output(
                    result.scene(),
                    RasterBackground::Transparent,
                    Some(OutputRasterTarget::new(
                        (canvas.width * 1.25) as u32,
                        (canvas.height * 1.125) as u32,
                    )?),
                    RasterAntialiasing::On,
                )?;
                let preview = toniator_render::rasterize_preview(
                    result.scene(),
                    PreviewRasterTarget::new(211, 137)?,
                )?;
                fs::write(
                    out.join(format!("{mode:?}-larger.png")),
                    encode_png(&larger)?,
                )?;
                fs::write(
                    out.join(format!("{mode:?}-preview.png")),
                    encode_png(&preview)?,
                )?;
                let vector = if matches!(kind, "round" | "connection") {
                    result
                        .scene()
                        .layers()
                        .iter()
                        .flat_map(|layer| layer.outputs())
                        .map(|output| {
                            toniator_render::measure_geometry_vector_occupancy(
                                output.geometry(),
                                &canvas,
                                toniator_render::VectorOccupancyPrecision::Tight,
                                Default::default(),
                                &|| false,
                            )
                            .map(|value| value.fraction)
                        })
                        .collect::<std::result::Result<Vec<_>, _>>()?
                } else {
                    Vec::new()
                };
                match full {
                    None => {
                        full = Some((
                            larger.pixels().to_vec(),
                            preview.pixels().to_vec(),
                            vector.clone(),
                        ))
                    }
                    Some(ref full) => {
                        if full.2 != vector {
                            return Err(format!(
                                "vector occupancy mismatch for {source}-{kind}: {:?} {:?}",
                                full.2, vector
                            )
                            .into());
                        }
                        if full.0 != larger.pixels() || full.1 != preview.pixels() {
                            return Err(format!(
                                "scaled native pixel mismatch for {source}-{kind}"
                            )
                            .into());
                        }
                    }
                }
            }
            records.push(json!({"case":format!("{source}-{kind}"),"larger_anisotropic_rgba_exact":true,"small_fitted_preview_rgba_exact":true}));
        }
    }
    fs::write(
        root.join("scaled-native-equality.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    Ok(())
}
/// Checks native decoded SVG pixels without compositing or changing original paired artifacts.
/// # Errors
/// Returns source, decoder, equality or evidence publication failures.
fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/artistic-geometry-20261019/phase4");
    let mut records = Vec::new();
    for source in ["raster", "vector"] {
        for kind in ["marks", "grid", "round", "motif", "connection"] {
            let case = root.join("final").join(format!("{source}-{kind}"));
            let complete = fs::read(case.join("Complete/timing/native.svg"))?;
            let canvas = fs::read(case.join("Canvas/timing/native.svg"))?;
            let a = decode_source(&complete, SourceFormatHint::Svg)?;
            let b = decode_source(&canvas, SourceFormatHint::Svg)?;
            let output = root.join("svg-consumer").join(format!("{source}-{kind}"));
            fs::create_dir_all(&output)?;
            for (name, bytes) in [("complete", &complete), ("canvas", &canvas)] {
                let png = reduced_preview_png(
                    bytes,
                    SourceFormatHint::Svg,
                    a.identity().width.max(a.identity().height),
                )?;
                fs::write(output.join(format!("{name}.png")), png.png_bytes)?;
            }
            let equal = a.identity().decoded_pixel_hash == b.identity().decoded_pixel_hash;
            records.push(json!({"case":format!("{source}-{kind}"),"width":a.identity().width,"height":a.identity().height,"complete_decoded_pixel_sha256":a.identity().decoded_pixel_hash,"canvas_decoded_pixel_sha256":b.identity().decoded_pixel_hash,"native_pixels_exact":equal}));
            if !equal {
                return Err(format!("SVG native pixel mismatch for {source}-{kind}").into());
            }
        }
    }
    fs::write(
        root.join("svg-native-equality.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    verify_scaled_pairs(&root)?;
    println!("{} SVG native pairs exact", records.len());
    Ok(())
}
