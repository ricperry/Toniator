//! Retained full-canvas preview-root preparation and whole-view certification evidence.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::PathBuf, time::Instant};
use toniator_domain::{DocumentSession, SourceReference};
use toniator_engine::*;
use toniator_patterns::{ConstructionContext, ConstructionMode};
use toniator_render::preview_tiles::{PreviewRoot, Region};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Evaluates immutable saved authority once, then prepares one root and certifies its complete views.
/// The existing scene request includes its tiny raster; preparation does not allocate full-root
/// pixels. Every view shares one consumed Main work policy; diagnostic wall/construction/RSS facts
/// are not a clean performance benchmark. Complete mode is an explicit construction-restriction foil.
/// # Errors
/// Returns argument, saved-authority, evaluation, checked virtual-extent or fresh evidence I/O errors.
/// Preparation or certification failures terminate with their unchanged diagnostic.
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 5 {
        return Err("usage: probe PROJECT FRESH_OUTPUT SCALE canvas|complete".into());
    }
    let project_path = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    if out.exists() {
        return Err("fresh output required".into());
    }
    fs::create_dir_all(&out)?;
    let scale = args[3].parse::<f64>()?;
    let mode = match args[4].as_str() {
        "canvas" => ConstructionMode::Canvas,
        "complete" => ConstructionMode::Complete,
        _ => return Err("construction mode".into()),
    };
    let project = toniator_io::load(&project_path)?;
    let document = project.document();
    document.require_saved_calibration_authority()?;
    let SourceReference::Assigned(id) = document.source() else {
        return Err("embedded source required".into());
    };
    let source = project.sources().get(id).ok_or("missing embedded source")?;
    let authority = format!(
        "saved={:?}\nbaseline={:?}\n",
        document.saved_calibrations(),
        document.baseline_saved_calibration()?
    );
    fs::write(out.join("saved-authority.txt"), &authority)?;
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
    let pool = rayon::ThreadPoolBuilder::new().num_threads(4).build()?;
    let context = ConstructionContext::new(mode, None, true);
    let started = Instant::now();
    let evaluated = pool.install(|| {
        context.run(|| {
            evaluate_with_limits(
                request,
                EvaluationLimits::default().with_construction_mode(mode),
            )
        })
    })?;
    let evaluation_seconds = started.elapsed().as_secs_f64();
    let scene = evaluated.shared_scene();
    fs::write(
        out.join("scene-identity.txt"),
        format!("{:#?}", scene.identity()),
    )?;
    let limits =
        RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())?;
    let started = Instant::now();
    let mut root = pool.install(|| {
        context.run(|| {
            PreviewRoot::new(scene, scale, RasterBackground::OpaqueBlack, limits, &|| {
                false
            })
        })
    })?;
    let preparation_seconds = started.elapsed().as_secs_f64();
    let before = root.facts()?;
    let frame = root.frame();
    let origins = if scale == 8. {
        vec![
            (0., 0.),
            (432.25 * scale, 301.5 * scale),
            (1000.125 * scale, 1040.25 * scale),
        ]
    } else {
        vec![(0., 0.)]
    };
    let mut views = Vec::new();
    for (index, (x, y)) in origins.into_iter().enumerate() {
        let region = if scale == 8. {
            root.sampled_region(x, y, 3118, 1916)?
        } else {
            Region {
                x: 0,
                y: 0,
                width: frame.width,
                height: frame.height,
            }
        };
        let started = Instant::now();
        let certificate = root.certify(region, &|| false)?;
        let certification_seconds = started.elapsed().as_secs_f64();
        let snapshot = root.snapshot(&certificate)?;
        let started = Instant::now();
        let tile = pool.install(|| snapshot.paint(region, &certificate, &|| false))?;
        let painting_seconds = started.elapsed().as_secs_f64();
        let surface = tile.surface(&root, &certificate)?;
        let raw = surface.pixels();
        fs::write(out.join(format!("view-{index}.rgba")), raw)?;
        fs::write(
            out.join(format!("view-{index}.png")),
            toniator_render::encode_png(surface)?,
        )?;
        views.push(json!({"region":[region.x,region.y,region.width,region.height],"layer_bounds":certificate.layer_bounds(),"generation":certificate.generation(),"certification_seconds":certification_seconds,"painting_seconds":painting_seconds,"rgba_sha256":format!("{:x}",Sha256::digest(raw))}));
    }
    let after = root.facts()?;
    let record = json!({"project":project_path,"project_sha256":format!("{:x}",Sha256::digest(fs::read(&project_path)?)),"source_sha256":format!("{:x}",Sha256::digest(source.bytes())),"saved_authority_sha256":format!("{:x}",Sha256::digest(authority.as_bytes())),"calibration_source":"existing required saved authority; no fitting requested","evaluation_seconds":evaluation_seconds,"preparation_seconds":preparation_seconds,"frame":[frame.width,frame.height,frame.scale],"facts_before":format!("{before:?}"),"facts_after":format!("{after:?}"),"views":views,"construction_observations":context.measurements.as_ref().ok_or("missing observations")?.snapshot(),"scope":"one retained full-canvas preparation; whole-view debt queries; fixed canvas grid; diagnostic counters enabled, not benchmark"});
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&record)?)?;
    println!("{record}");
    Ok(())
}
