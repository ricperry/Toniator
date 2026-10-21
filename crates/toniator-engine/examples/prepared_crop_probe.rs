//! Default-off accepted-scene raster benchmark and fresh-process proof; no frontend integration.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, io::Write, path::PathBuf, time::Instant};
use toniator_domain::{DocumentSession, SourceReference};
use toniator_engine::*;
use toniator_patterns::{ConstructionContext, ConstructionMode};
use toniator_render::prepared_crop_pilot::{self as pilot, Crop, Options, Summary, Timings};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Chooses unchanged direct production consumption or one fixed P1-proved partition.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Direct,
    Full,
    Bands,
    Xy,
}
/// Retains only configured scheduling facts, never invented observed-worker participation.
#[derive(Clone, Copy)]
struct Config {
    name: &'static str,
    workers: usize,
    kind: Kind,
}
/// Owns fixed request plans outside raster timing; their capacities enter pilot admission.
struct Plans {
    full: Vec<Crop>,
    bands: Vec<Crop>,
    xy: Vec<Crop>,
    capacity_bytes: usize,
}
impl Plans {
    /// Creates fixed uneven P1 XY/row partitions without changing root phase or coordinates.
    /// # Errors
    /// Returns crop/admission, arithmetic or fallible request-plan allocation errors.
    fn new(width: u32, height: u32) -> Result<Self> {
        let mut full = Vec::new();
        full.try_reserve_exact(1)?;
        full.push(Crop::new(0, 0, width, height)?);
        let mut bands = Vec::new();
        let mut xy = Vec::new();
        let mut y = 0;
        let mut row = 0;
        while y < height {
            let h = [137, 251, 113][row % 3].min(height - y);
            bands.try_reserve(1)?;
            bands.push(Crop::new(0, y, width, h)?);
            let mut x = 0;
            let mut col = 0;
            while x < width {
                let w = [257, 511, 173][col % 3].min(width - x);
                xy.try_reserve(1)?;
                xy.push(Crop::new(x, y, w, h)?);
                x += w;
                col += 1;
            }
            y += h;
            row += 1;
        }
        let offset = 7.min(xy.len());
        xy.rotate_left(offset);
        let capacity_bytes = full
            .capacity()
            .checked_add(bands.capacity())
            .and_then(|n| n.checked_add(xy.capacity()))
            .and_then(|n| n.checked_mul(std::mem::size_of::<Crop>()))
            .ok_or("plan payload overflow")?;
        Ok(Self {
            full,
            bands,
            xy,
            capacity_bytes,
        })
    }
    /// Borrows one fixed caller-owned plan; direct mode uses no prepared facade.
    fn get(&self, kind: Kind) -> &[Crop] {
        match kind {
            Kind::Direct => &[],
            Kind::Full => &self.full,
            Kind::Bands => &self.bands,
            Kind::Xy => &self.xy,
        }
    }
}

/// Enumerates the bounded approved configurations without a tile-size sweep.
fn configs() -> [Config; 8] {
    [
        Config {
            name: "D1",
            workers: 1,
            kind: Kind::Direct,
        },
        Config {
            name: "D2",
            workers: 2,
            kind: Kind::Direct,
        },
        Config {
            name: "D4",
            workers: 4,
            kind: Kind::Direct,
        },
        Config {
            name: "full1",
            workers: 1,
            kind: Kind::Full,
        },
        Config {
            name: "bands1",
            workers: 1,
            kind: Kind::Bands,
        },
        Config {
            name: "XY1",
            workers: 1,
            kind: Kind::Xy,
        },
        Config {
            name: "XY2",
            workers: 2,
            kind: Kind::Xy,
        },
        Config {
            name: "XY4",
            workers: 4,
            kind: Kind::Xy,
        },
    ]
}

/// Borrows the already-built/warmed pool with the configured worker count.
/// # Errors
/// Returns an argument error rather than substituting another scheduler.
fn pool_for(pools: &[(usize, rayon::ThreadPool)], workers: usize) -> Result<&rayon::ThreadPool> {
    pools
        .iter()
        .find(|(n, _)| *n == workers)
        .map(|(_, p)| p)
        .ok_or_else(|| "missing configured pool".into())
}

/// Projects actual optional coarse phase observations; overlapping job sums/maxima are not wall totals.
fn timings(t: &Timings) -> Value {
    json!({"materialization_seconds":t.materialization.as_secs_f64(),"preparation_seconds":t.preparation.as_secs_f64(),"admission_seconds":t.admission.as_secs_f64(),"worker_batch_wall_seconds":t.worker_batch_wall.as_secs_f64(),"sink_seconds":t.sink.as_secs_f64(),"integration_job_sum_seconds":t.integration_job_sum.as_secs_f64(),"integration_job_max_seconds":t.integration_job_max.as_secs_f64(),"composition_job_sum_seconds":t.composition_job_sum.as_secs_f64(),"composition_job_max_seconds":t.composition_job_max.as_secs_f64(),"quantization_job_sum_seconds":t.quantization_job_sum.as_secs_f64(),"quantization_job_max_seconds":t.quantization_job_max.as_secs_f64()})
}

/// Projects actual facade work/capacity observations without production counter instrumentation.
fn summary(s: &Summary) -> Value {
    json!({"delivered_tiles":s.delivered_tiles,"maximum_batch_tiles":s.maximum_batch_tiles,"prepared_capacity_bytes":s.prepared_capacity_bytes,"maximum_estimated_payload_bytes":s.maximum_estimated_payload_bytes,"normalized_edges":s.normalized_edges,"prepared_paints":s.prepared_paints,"root_primitives":s.root_primitives,"consumed_flattened_edges":s.consumed_flattened_edges,"consumed_boolean_operations":s.consumed_boolean_operations,"coarse_timings":s.timings.as_ref().map(timings)})
}

/// Copies original row-major RGBA into caller-owned root staging, with no evidence work in the sink.
/// # Panics
/// Panics only if internal admitted crop/tile/root dimensions or buffer lengths disagree.
fn stitch(target: PreviewRasterTarget, crop: Crop, tile: &RasterSurface, assembled: &mut [u8]) {
    for y in 0..crop.height as usize {
        let dst = ((crop.y as usize + y) * target.width() as usize + crop.x as usize) * 4;
        let src = y * crop.width as usize * 4;
        let bytes = crop.width as usize * 4;
        assembled[dst..dst + bytes].copy_from_slice(&tile.pixels()[src..src + bytes]);
    }
}

/// Times accepted-scene to admitted complete RGBA, including all required output assembly work.
/// Timer starts before branch allocations and pool/context installation. Direct returns the normal
/// complete production surface without extra assembly/copy. Pilot includes its required allocation,
/// zero-init, preparation, integration/composition/conversion, sink copies and final admission.
/// Both use the same worker-count pool; hash/compare/encoding/evidence occur afterward.
/// Caller reference and all fixed plan capacities are declared; sink retains no tile or geometry.
/// # Errors
/// Returns allocation/admission, renderer/pilot or exact-reference mismatch diagnostics.
#[expect(
    clippy::too_many_arguments,
    reason = "diagnostic timing boundary keeps captured scene/resource/reference authority explicit"
)]
fn render(
    config: Config,
    scene: &toniator_render::RenderScene,
    target: PreviewRasterTarget,
    limits: RasterizationLimits,
    plans: &Plans,
    pool: &rayon::ThreadPool,
    reference: &Vec<u8>,
    profiling: bool,
) -> Result<(RasterSurface, Value)> {
    let started = Instant::now();
    let (surface, stats, assembly_capacity, caller_bytes) = if config.kind == Kind::Direct {
        // Production already returns a complete admitted surface. No extra root allocation,
        // zero-initialization, copy or admission belongs to this baseline's contract.
        let surface = pool.install(|| {
            let context = ConstructionContext::new(ConstructionMode::Canvas, None, false);
            context.run(|| {
                toniator_render::rasterize_preview_cancellable(scene, target, limits, &|| false)
            })
        })?;
        (surface, None, None, None)
    } else {
        let mut assembled = Vec::new();
        assembled.try_reserve_exact(reference.len())?;
        assembled.resize(reference.len(), 0u8);
        let capacity = assembled.capacity();
        let crops = plans.get(config.kind);
        let admission = usize::try_from(4_u64 * 1024 * 1024 * 1024)?;
        let declared = reference
            .capacity()
            .checked_add(capacity)
            .and_then(|n| n.checked_add(plans.capacity_bytes))
            .and_then(|n| n.checked_sub(std::mem::size_of_val(crops)))
            .ok_or("caller payload overflow")?;
        let stats = pool.install(|| {
            let context = ConstructionContext::new(ConstructionMode::Canvas, None, false);
            context.run(|| {
                pilot::run(
                    scene,
                    target,
                    limits,
                    crops,
                    Options {
                        maximum_estimated_payload_bytes: admission,
                        caller_retained_bytes: declared,
                        maximum_in_flight: config.workers,
                        profiling,
                    },
                    pool,
                    &|| false,
                    |crop, tile| {
                        stitch(target, crop, tile, &mut assembled);
                        Ok(())
                    },
                )
            })
        })?;
        let surface = RasterSurface::new(target.width(), target.height(), assembled)?;
        (surface, Some(stats), Some(capacity), Some(declared))
    };
    let outer_seconds = started.elapsed().as_secs_f64();
    if surface.pixels() != reference {
        return Err("timed complete RGBA differs from pinned P1 reference".into());
    }
    let record = json!({"config":config.name,"configured_pool_workers":config.workers,"outer_seconds":outer_seconds,"profiling_enabled":profiling,"rgba_exact":true,"rgba_sha256":format!("{:x}",Sha256::digest(surface.pixels())),"reference_capacity_bytes":reference.capacity(),"assembly_capacity_bytes":assembly_capacity,"all_plan_capacity_bytes":plans.capacity_bytes,"declared_caller_payload_bytes":caller_bytes,"facade_summary":stats.as_ref().map(summary)});
    Ok((surface, record))
}

/// Writes one completed object-valued observation outside raster timing and flushes progress.
/// Its input is the diagnostic render object, never arbitrary user JSON.
/// # Errors
/// Returns evidence write/serialization failures; no measurement is synthesized on renderer failure.
fn record(
    file: &mut fs::File,
    mut sample: Value,
    phase: &str,
    candidate: &str,
    round: usize,
    position: usize,
) -> Result<()> {
    sample["phase"] = json!(phase);
    sample["pair_candidate"] = json!(candidate);
    sample["round"] = json!(round);
    sample["position"] = json!(position);
    writeln!(file, "{sample}")?;
    file.flush()?;
    println!(
        "{phase} {candidate} round={round} position={position} config={} seconds={}",
        sample["config"], sample["outer_seconds"]
    );
    Ok(())
}

/// Executes the approved unprofiled paired matrix or one separate profile/RSS render.
/// Saved scene evaluation and pool construction/warming are outside raster timing. Bench mode has
/// one target warmup per distinct config and three alternating pairs with rotated candidate order.
/// Single/profile modes perform exactly one actual target render plus existing for_scene1px evaluation;
/// fresh-process RSS therefore includes loading/evaluation/allocator lifetime, not isolated raster RAM.
/// # Errors
/// Returns argument/current-source/saved-authority/pool/renderer/parity or fresh evidence I/O errors.
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 6 || args.len() > 7 {
        return Err("usage: probe bench|single|profile PROJECT FRESH_OUTPUT main400|natural P1_RGBA [CONFIG]".into());
    }
    let mode = &args[1];
    if !["bench", "single", "profile"].contains(&mode.as_str())
        || (mode == "bench") != (args.len() == 6)
    {
        return Err("mode/config arguments".into());
    }
    let out = PathBuf::from(&args[3]);
    if out.exists() {
        return Err("fresh output required".into());
    }
    fs::create_dir_all(&out)?;
    let project = toniator_io::load(&PathBuf::from(&args[2]))?;
    let document = project.document();
    document.require_saved_calibration_authority()?;
    let SourceReference::Assigned(id) = document.source() else {
        return Err("embedded source required".into());
    };
    let source = project.sources().get(id).ok_or("missing source")?;
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
    let mut pools = Vec::new();
    for workers in [1, 2, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()?;
        pool.broadcast(|_| std::hint::black_box(()));
        pools.push((workers, pool));
    }
    let evaluated = pool_for(&pools, 4)?.install(|| {
        let context = ConstructionContext::new(ConstructionMode::Canvas, None, false);
        context.run(|| {
            evaluate_with_limits(
                request,
                EvaluationLimits::default().with_construction_mode(ConstructionMode::Canvas),
            )
        })
    })?;
    let scene = evaluated.scene();
    let target = match args[4].as_str() {
        "main400" => PreviewRasterTarget::for_viewport(3118, 1916, 0., 0., 8.)?,
        "natural" => {
            let t = OutputRasterTarget::for_canvas(document.canvas())?;
            PreviewRasterTarget::new(t.width(), t.height())?
        }
        _ => return Err("target policy".into()),
    }
    .with_background(RasterBackground::OpaqueBlack);
    let limits =
        RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())?;
    let reference = fs::read(&args[5])?;
    if reference.len() != target.width() as usize * target.height() as usize * 4 {
        return Err("reference dimensions".into());
    }
    let plans = Plans::new(target.width(), target.height())?;
    let context = json!({"mode":mode,"project":args[2],"project_sha256":format!("{:x}",Sha256::digest(fs::read(&args[2])?)),"source_sha256":format!("{:x}",Sha256::digest(source.bytes())),"saved_authority_sha256":format!("{:x}",Sha256::digest(authority.as_bytes())),"calibration_source":"required exact saved authority; no fitting requested; no runtime count","scene_identity":format!("{:?}",scene.identity()),"source_identity":format!("{:?}",evaluated.source_identity()),"target":format!("{target:?}"),"limits":format!("{limits:?}"),"reference_sha256":format!("{:x}",Sha256::digest(&reference)),"construction_measurements_enabled":false,"load_before":fs::read_to_string("/proc/loadavg")?,"timing_scope":"accepted scene to complete RGBA: native Direct returns production surface without extra assembly/copy; pilot includes required assembly allocation/init, fresh preparation, paint/convert, sink copies and final admission; pool/context included for both; compare/hash/encode/IO excluded","memory_scope":"facade estimated payload is not RSS; original scene/source, preparation transients, allocator/Rayon and diagnostic metadata excluded; process RSS includes entire load/evaluation/render lifetime"});
    fs::write(
        out.join("context.json"),
        serde_json::to_vec_pretty(&context)?,
    )?;
    let mut samples = fs::File::create_new(out.join("samples.jsonl"))?;
    let all = configs();
    if mode == "bench" {
        let candidates = all
            .iter()
            .copied()
            .filter(|c| {
                c.kind != Kind::Direct
                    && (args[4] == "main400" || ["full1", "XY4"].contains(&c.name))
            })
            .collect::<Vec<_>>();
        let needed = all
            .iter()
            .copied()
            .filter(|c| {
                candidates
                    .iter()
                    .any(|x| x.name == c.name || (c.kind == Kind::Direct && x.workers == c.workers))
            })
            .collect::<Vec<_>>();
        for config in needed {
            let (_, sample) = render(
                config,
                scene,
                target,
                limits,
                &plans,
                pool_for(&pools, config.workers)?,
                &reference,
                false,
            )?;
            record(&mut samples, sample, "warmup", config.name, 0, 0)?;
        }
        for round in 0..3 {
            for offset in 0..candidates.len() {
                let candidate = candidates[(offset + round) % candidates.len()];
                let baseline = all
                    .iter()
                    .copied()
                    .find(|c| c.kind == Kind::Direct && c.workers == candidate.workers)
                    .ok_or("baseline config")?;
                let order = if round % 2 == 0 {
                    [baseline, candidate]
                } else {
                    [candidate, baseline]
                };
                for (position, config) in order.into_iter().enumerate() {
                    let (surface, sample) = render(
                        config,
                        scene,
                        target,
                        limits,
                        &plans,
                        pool_for(&pools, config.workers)?,
                        &reference,
                        false,
                    )?;
                    record(
                        &mut samples,
                        sample,
                        "paired",
                        candidate.name,
                        round + 1,
                        position + 1,
                    )?;
                    if round == 0 && config.name == "XY4" {
                        fs::write(out.join("representative.rgba"), surface.pixels())?;
                        fs::write(out.join("representative.png"), encode_png(&surface)?)?;
                    }
                }
            }
        }
    } else {
        let config = all
            .iter()
            .copied()
            .find(|c| c.name == args[6])
            .ok_or("unknown config")?;
        if mode == "profile" && config.kind == Kind::Direct {
            return Err(
                "direct production phase profiling is unavailable; no production instrumentation"
                    .into(),
            );
        }
        let (_, sample) = render(
            config,
            scene,
            target,
            limits,
            &plans,
            pool_for(&pools, config.workers)?,
            &reference,
            mode == "profile",
        )?;
        record(&mut samples, sample, mode, config.name, 0, 0)?;
    }
    fs::write(out.join("load-after.txt"), fs::read("/proc/loadavg")?)?;
    Ok(())
}
