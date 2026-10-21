//! Current default-feature persistent sparse preview cache observations.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::atomic::AtomicBool;
use std::{error::Error, fs, path::PathBuf, time::Instant};
use toniator_domain::{DocumentSession, SourceReference};
use toniator_engine::preview_cache::{Plan, PreviewCache, Region, RootKey};
use toniator_engine::*;
use toniator_patterns::{ConstructionContext, ConstructionMode};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Evaluates saved authority once and observes actual cold/missing/warm production cache requests.
/// The existing scene request includes its tiny raster; preparation does not allocate full-root
/// pixels. Every view shares one consumed Main work policy; diagnostic wall/RSS facts
/// are not a clean performance benchmark. All observations use the current Canvas construction mode.
/// # Errors
/// Returns argument, saved-authority, evaluation, checked virtual-extent or fresh evidence I/O errors.
/// Root/cache failures terminate as explicit fallback observations, without changing numerical policy.
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 4 {
        return Err("usage: probe PROJECT FRESH_OUTPUT SCALE".into());
    }
    let project_path = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    if out.exists() {
        return Err("fresh output required".into());
    }
    fs::create_dir_all(&out)?;
    let scale = args[3].parse::<f64>()?;
    let mode = ConstructionMode::Canvas;
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
    let context = ConstructionContext::new(mode, None, false);
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
    let mut cache = PreviewCache::default();
    cache.accept_scene(
        (1, 2, 0),
        scene.clone(),
        preview_cache::estimate_scene(&scene)?,
    );
    let limits =
        RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())?;
    let key = RootKey::new(scale, RasterBackground::OpaqueBlack, limits);
    let dimensions = [
        (document.canvas().width * scale).ceil() as u32,
        (document.canvas().height * scale).ceil() as u32,
    ];
    let initial = Region {
        x: 0,
        y: 0,
        width: 3118.min(dimensions[0]),
        height: 1916.min(dimensions[1]),
    };
    let background_started = Instant::now();
    cache.plan((1, 2, 0), key, initial)?;
    let mut autonomous_jobs = 0u64;
    while let Some(job) = cache.take_job() {
        autonomous_jobs += 1;
        let completed = pool.install(|| context.run(|| job.run(&AtomicBool::new(false))));
        cache.complete(completed);
    }
    let background = cache.statistics();
    if background.completed_zooms != 1
        || background.painted_pixels != u64::from(dimensions[0]) * u64::from(dimensions[1])
    {
        return Err("autonomous full-zoom storage did not complete".into());
    }
    let background_record = json!({"seconds":background_started.elapsed().as_secs_f64(),
        "plan_calls":1,"worker_jobs":autonomous_jobs,"dimensions":dimensions,
        "statistics":format!("{background:?}"),"scope":"private stored pixels; not a whole-root certificate"});
    fs::write(
        out.join("autonomous-background.json"),
        serde_json::to_vec_pretty(&background_record)?,
    )?;
    let mut cases = vec![("cold", initial)];
    if scale == 8. {
        cases.push((
            "overlap",
            Region {
                x: 128,
                y: 96,
                ..initial
            },
        ));
    }
    cases.push(("reverse", initial));
    cases.push(("repeat", initial));
    if scale == 8. {
        cases.push((
            "interior",
            Region {
                x: 3458,
                y: 2412,
                ..initial
            },
        ));
        cases.push((
            "bottom-right",
            Region {
                x: 8001,
                y: 8322,
                width: dimensions[0].saturating_sub(8001),
                height: dimensions[1].saturating_sub(8322),
            },
        ));
        cases.push(("return", initial));
    }
    cases.retain(|(_, r)| {
        r.width > 0
            && r.height > 0
            && u64::from(r.x) + u64::from(r.width) <= u64::from(dimensions[0])
            && u64::from(r.y) + u64::from(r.height) <= u64::from(dimensions[1])
    });
    let mut observations = Vec::new();
    for (label, region) in cases {
        let before = cache.statistics();
        let started = Instant::now();
        let view = loop {
            match cache.plan((1, 2, 0), key, region)? {
                Plan::Ready(view) if view.complete => break view,
                Plan::Unavailable => return Err("ordinary fallback required".into()),
                _ => {}
            }
            let mut dispatched = false;
            while let Some(job) = cache.take_job() {
                dispatched = true;
                let completed = pool.install(|| context.run(|| job.run(&AtomicBool::new(false))));
                cache.complete(completed);
            }
            if !dispatched {
                return Err("cache plan did not produce bounded work".into());
            }
        };
        let seconds = started.elapsed().as_secs_f64();
        let after = cache.statistics();
        let bytes = view.copy_upload(region)?;
        if label == "cold" || label == "interior" || label == "bottom-right" {
            fs::write(out.join(format!("{label}.rgba")), &bytes)?;
            let surface = RasterSurface::new(region.width, region.height, bytes.clone())?;
            fs::write(out.join(format!("{label}.png")), encode_png(&surface)?)?;
        }
        observations.push(json!({"case":label,"region":[region.x,region.y,region.width,region.height],"seconds_before_copy_or_io":seconds,"root_preparation_delta":after.root_preparations-before.root_preparations,"paint_job_delta":after.paint_jobs-before.paint_jobs,"painted_pixel_delta":after.painted_pixels-before.painted_pixels,"rgba_sha256":format!("{:x}",Sha256::digest(&bytes)),"statistics":format!("{after:?}")}));
    }
    let record = json!({"project_sha256":format!("{:x}",Sha256::digest(fs::read(&project_path)?)),"source_sha256":format!("{:x}",Sha256::digest(source.bytes())),"saved_authority_sha256":format!("{:x}",Sha256::digest(authority.as_bytes())),"calibration_source":"required saved authority; no fitting requested","evaluation_seconds":evaluation_seconds,"scale":scale,"cases":observations,"autonomous_background":background_record,"scope":"actual production cache; default diagnostic feature OFF; construction counters OFF; warm timing excludes output copy/encoding/IO and GTK; one scene evaluation outside cache timing"});
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&record)?)?;
    println!("{record}");
    Ok(())
}
