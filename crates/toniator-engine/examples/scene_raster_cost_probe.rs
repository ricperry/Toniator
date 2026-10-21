//! Measures the current scene request's real one-pixel raster cost without changing authority.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::PathBuf, time::Instant};
use toniator_domain::{DocumentSession, SourceReference};
use toniator_engine::{
    evaluate_profiled_cached_with_limits, evaluate_profiled_with_limits, EvaluationLimits,
    EvaluationProfileCache, EvaluationRequest, ProfiledEvaluation, ResolvedSource,
    SourceFormatHint,
};
use toniator_patterns::{ConstructionContext, ConstructionMode};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Projects already-measured public records after timing; no counters or scene dumps are invented.
fn record(label: &str, wall_seconds: f64, profiled: &ProfiledEvaluation) -> Value {
    json!({
        "label":label,"whole_call_seconds":wall_seconds,
        "scene_identity":format!("{:?}",profiled.result.scene().identity()),
        "source_identity":format!("{:?}",profiled.result.source_identity()),
        "raster_dimensions":[profiled.result.raster().width(),profiled.result.raster().height()],
        "raster_rgba":profiled.result.raster().pixels(),
        "cache_diagnostics":format!("{:?}",profiled.diagnostics),
        "configured_workers":profiled.performance.configured_worker_count,
        "observed_workers":profiled.performance.observed_worker_count,
        "worker_registrations":profiled.performance.worker_registration_count,
        "records":profiled.performance.records.iter().map(|r|json!({
            "stage":format!("{:?}",r.stage),"seconds":r.elapsed.as_secs_f64(),
            "channel":r.channel_id.map(|id|id.0),"output":r.output_layer_id.map(|id|id.0),
            "cache":format!("{:?}",r.cache),"execution":format!("{:?}",r.execution),
            "workloads":r.workloads.iter().map(|w|json!({"kind":format!("{:?}",w.kind),"count":w.count})).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    })
}

/// Measures one frozen saved project with a caller-owned four-worker Rayon pool outside timings.
/// Each timed pool installation creates an unmeasured Canvas construction context on its worker.
/// Warmup, three independent empty-cache runs and a separate cold-cache/three-warm series use
/// unchanged for_scene Output1x1 AA-off policy. Loading, hashing and serialization are untimed.
/// # Errors
/// Returns argument, fresh-output, saved-authority, pool, evaluation or evidence I/O diagnostics.
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("usage: scene_raster_cost_probe PROJECT FRESH_OUTPUT".into());
    }
    let path = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    if out.exists() {
        return Err("fresh output required".into());
    }
    fs::create_dir_all(&out)?;
    let bytes = fs::read(&path)?;
    let project = toniator_io::load(&path)?;
    let document = project.document();
    document.require_saved_calibration_authority()?;
    let SourceReference::Assigned(id) = document.source() else {
        return Err("embedded source required".into());
    };
    let source = project.sources().get(id).ok_or("embedded source missing")?;
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
    let limits = EvaluationLimits::default().with_construction_mode(ConstructionMode::Canvas);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(4).build()?;
    let mut runs = Vec::new();
    for label in ["warmup", "cold-1", "cold-2", "cold-3"] {
        let call = request.clone();
        let start = Instant::now();
        let evaluated = pool.install(|| {
            let context = ConstructionContext::new(ConstructionMode::Canvas, None, false);
            context.run(|| evaluate_profiled_with_limits(call, limits))
        })?;
        let elapsed = start.elapsed().as_secs_f64();
        runs.push(record(label, elapsed, &evaluated));
    }
    let mut cache = EvaluationProfileCache::default();
    for label in [
        "cache-seed-cold",
        "warm-cache-1",
        "warm-cache-2",
        "warm-cache-3",
    ] {
        let call = request.clone();
        let start = Instant::now();
        let evaluated = pool.install(|| {
            let context = ConstructionContext::new(ConstructionMode::Canvas, None, false);
            context.run(|| evaluate_profiled_cached_with_limits(call, limits, &mut cache))
        })?;
        let elapsed = start.elapsed().as_secs_f64();
        runs.push(record(label, elapsed, &evaluated));
    }
    let output = json!({
        "project":path,"project_sha256":format!("{:x}",Sha256::digest(&bytes)),
        "source_id":format!("{id:?}"),"source_sha256":format!("{:x}",Sha256::digest(source.bytes())),
        "saved_authority_sha256":format!("{:x}",Sha256::digest(authority.as_bytes())),
        "calibration_source":"required unchanged saved project authority; no fitting requested; no runtime fit counter",
        "request":"existing for_scene: Output1x1 Transparent AAOff", "construction_mode":"Canvas",
        "construction_measurements_enabled":false,"pool_workers":4,"limits":format!("{limits:?}"),
        "timing_scope":"whole call includes pool.install/context creation; request clone, source loading, hashes and evidence serialization excluded",
        "rss_scope":"external /usr/bin/time -v covers this entire owned process, including all runs and evidence",
        "runs":runs
    });
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&output)?)?;
    Ok(())
}
