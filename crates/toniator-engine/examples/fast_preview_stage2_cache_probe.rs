//! Stage 2 native cache provenance/resource witness, not a product CLI or timing benchmark.
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::Path,
    sync::atomic::AtomicBool,
    thread,
    time::{Duration, Instant},
};
use toniator_engine::{
    EvaluationScheduler,
    preview_cache::{
        self as cache, FastPlan, PreviewCache, Quality, Region, RootKey, View, ViewPermission,
        WorkerState,
    },
};
use toniator_patterns::CanonicalMark;
use toniator_render::{
    GeometryOutput, RasterBackground, RasterizationLimits, RenderScene, encode_png,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// Evaluates exactly one accepted canonical scene, then paints multiple demands on a retained worker.
/// # Errors
/// Returns authority/native/cache/IO failures; the output directory must be new.
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let project = Path::new(args.get(1).ok_or("expected calibrated project")?);
    let output = Path::new(args.get(2).ok_or("expected fresh evidence directory")?);
    if output.exists() {
        return Err("evidence directory must be fresh".into());
    }
    fs::create_dir_all(output)?;
    fs::write(output.join("phase.txt"), "accepted-raster-free-scene")?;
    let loaded = toniator_io::load(project)?;
    loaded.document().require_saved_calibration_authority()?;
    let session = toniator_domain::DocumentSession::new(loaded.document().clone())?;
    let mut media = toniator_engine::open_source_media(
        loaded.sources(),
        toniator_engine::MediaTools::default(),
        &|| false,
    )?;
    let request = toniator_engine::frame_evaluation_request(
        &session,
        &mut media,
        session.document().project_timing().frame_range().start(),
        &|| false,
    )?;
    let scheduler = EvaluationScheduler::new()?;
    scheduler.submit_scene(request)?;
    let start = Instant::now();
    let completion = loop {
        if let Some(value) = scheduler.try_receive_latest()? {
            break value;
        }
        if start.elapsed() > Duration::from_secs(180) {
            return Err("scene scheduler deadline".into());
        }
        thread::sleep(Duration::from_millis(2));
    };
    if let Some(error) = completion.error() {
        return Err(error.to_string().into());
    }
    if !scheduler.accept_completion(&completion, &session)? {
        return Err("scene candidate rejected".into());
    }
    let result = completion
        .scene_result()
        .ok_or("scene-only payload required")?;
    let scene = result.shared_scene();
    let canonical = audit(&scene)?;

    let mut cache = PreviewCache::default();
    cache.accept_scene((1, 1, 0), scene.clone(), cache::estimate_scene(&scene)?);
    cache.set_quality(Quality::Fast);
    let mut worker = WorkerState::default();
    let mut records = Vec::new();
    let mut retained = None;
    for (i, x) in [0, 1536, 0].into_iter().enumerate() {
        fs::write(output.join("phase.txt"), format!("cache-pan-{i}"))?;
        let region = cache::gutter(
            Region {
                x,
                y: 0,
                width: 1536,
                height: 1536,
            },
            cache::Frame {
                width: 3600,
                height: 2480,
                scale: 4.,
                background: RasterBackground::Transparent,
            },
        );
        let view = ready(&mut cache, &mut worker, 4., region)?;
        let ViewPermission::Fast(permission) = &view.permission else {
            return Err("unexpected accurate certificate".into());
        };
        let ids = (permission.device, permission.root);
        if retained.is_some_and(|old| old != ids) {
            return Err("pan regenerated native root/device".into());
        }
        retained = Some(ids);
        let surface = &view.pixels[0].surface;
        fs::write(
            output.join(format!("fast-cell-{i}.png")),
            encode_png(surface)?,
        )?;
        let upload = cache
            .take_fast_upload(view.key, view.pixels[0].identity(), region)
            .ok_or("whole upload missing")?;
        if upload != surface.pixels() {
            return Err("owned upload differs from complete native region".into());
        }
        if !cache.publish_fast(view.key, view.generation) {
            return Err("current upload permission rejected".into());
        }
        records.push(serde_json::json!({"region":[region.x,region.y,region.width,region.height],"device":ids.0,"root":ids.1,"generation":view.generation,"rgba_sha256":format!("{:x}",Sha256::digest(surface.pixels())),"stats":format!("{:?}",cache.statistics())}));
        drop(upload);
        drop(view);
    }
    let stats = cache.statistics();
    if stats.fast_preparations != 1 || stats.fast_device_initializations != 1 {
        return Err("pan prepared or initialized more than once".into());
    }
    fs::write(output.join("phase.txt"), "cache-zoom-3")?;
    let zoom = ready(
        &mut cache,
        &mut worker,
        3.,
        Region {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        },
    )?;
    let ViewPermission::Fast(z) = &zoom.permission else {
        return Err("zoom permission".into());
    };
    if Some(z.device) != retained.map(|v| v.0) || Some(z.root) == retained.map(|v| v.1) {
        return Err("zoom did not reuse device and replace preparation".into());
    }
    let zoom_facts = serde_json::json!({"device":z.device,"root":z.root,"preparations":cache.statistics().fast_preparations,"device_initializations":cache.statistics().fast_device_initializations,"same_canonical_arc":cache.has_scene((1,1,0),&scene),"reservation_highwater":cache.statistics().fast_reserved_bytes});
    if cache.statistics().fast_preparations != 2
        || cache.statistics().fast_device_initializations != 1
    {
        return Err("zoom preparation count".into());
    }
    drop(zoom);
    // None invalidation rebind authorizes the same canonical Arc without any evaluator work.
    if !cache.rebind_unchanged((1, 1, 0), (1, 2, 0)) || !cache.has_scene((1, 2, 0), &scene) {
        return Err("canonical None rebind failed".into());
    }
    fs::write(
        output.join("results.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"canonical":canonical,"scene_evaluations":1,"zoom":zoom_facts,"pan":records,"preparations":stats.fast_preparations,"device_initializations":stats.fast_device_initializations,"performance_claim":false,"reservation_charge_once":true}),
        )?,
    )?;
    cache.clear();
    while let Some(job) = cache.take_job() {
        cache.complete(worker.run(job, &AtomicBool::new(false)));
    }
    let ack = worker.finish();
    cache.worker_retired(ack);
    if cache.statistics().fast_reserved_bytes != 0 {
        return Err("native worker charge survived retirement".into());
    }
    scheduler.shutdown()?;
    fs::write(output.join("phase.txt"), "complete")?;
    Ok(())
}
/// Drives bounded serialized cache jobs until one complete requested region is ready.
/// # Errors
/// Returns typed fallback/no-progress/deadline failures without accepting partial pixels.
fn ready(
    cache: &mut PreviewCache,
    worker: &mut WorkerState,
    scale: f64,
    region: Region,
) -> Result<View> {
    let start = Instant::now();
    loop {
        let key = RootKey::new(
            scale,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        )
        .with_provenance(cache.provenance());
        match cache.plan_fast((1, 1, 0), key, region)? {
            FastPlan::Ready(view) => return Ok(view),
            FastPlan::CpuFallback { reason, .. } => {
                return Err(format!("Fast fallback: {reason:?}").into());
            }
            FastPlan::Unavailable => return Err("scene unavailable".into()),
            FastPlan::Waiting => {}
        }
        let job = cache.take_job().ok_or("waiting without queued work")?;
        cache.complete(worker.run(job, &AtomicBool::new(false)));
        if start.elapsed() > Duration::from_secs(180) {
            return Err("cache witness deadline".into());
        }
    }
}
/// Audits every canonical circle ID/order/radius/coordinate bit, including zero/off-canvas guards.
/// # Errors
/// Rejects unsupported geometry rather than producing an incomplete canonical fingerprint.
fn audit(scene: &RenderScene) -> Result<serde_json::Value> {
    let mut hash = Sha256::new();
    let mut counts = BTreeMap::<u64, [usize; 4]>::new();
    for layer in scene.layers() {
        for output in layer.outputs() {
            let GeometryOutput::CanonicalMarks(marks) = output.geometry() else {
                return Err("fixture expected canonical circles".into());
            };
            for mark in marks {
                let CanonicalMark::Circle {
                    source_site_id,
                    center,
                    radius,
                    ..
                } = mark
                else {
                    return Err("fixture contains noncircle".into());
                };
                hash.update(layer.channel_id().0.to_le_bytes());
                hash.update(output.output_layer_id.0.to_le_bytes());
                hash.update(format!("{source_site_id:?}").as_bytes());
                for value in [center.x, center.y, *radius] {
                    hash.update(value.to_bits().to_le_bytes());
                }
                let count = counts.entry(layer.channel_id().0).or_default();
                count[0] += 1;
                count[1] += usize::from(*radius == 0.);
                count[2] += usize::from(!layer.visible());
                count[3] += usize::from(
                    center.x - radius < 0.
                        || center.y - radius < 0.
                        || center.x + radius > scene.canvas().width
                        || center.y + radius > scene.canvas().height,
                );
            }
        }
    }
    Ok(
        serde_json::json!({"scene_identity":scene.identity().scene_fingerprint(),"family_identity":scene.identity().family_fingerprint(),"realization_identity":scene.identity().realization_fingerprint(),"circle_order_id_coordinate_radius_bits_sha256":format!("{:x}",hash.finalize()),"channel_counts_total_zero_invisible_boundary_crossing":counts,"canvas":[scene.canvas().width,scene.canvas().height]}),
    )
}
