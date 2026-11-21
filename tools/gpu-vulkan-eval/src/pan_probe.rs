//! Fixed-zoom production-cell cache-miss diagnostic; no GTK or product cache implementation changes.
use super::*;
use std::{collections::BTreeMap, sync::Arc};
use toniator_engine::preview_cache::{gutter, intersection, missing};
use toniator_render::preview_tiles::{Frame, PreviewRoot, Region};
const TILE: u32 = 1536;
const TRACE: [(u32, u32); 9] = [
    (0, 0),
    (1024, 0),
    (1536, 0),
    (2560, 0),
    (2960, 0),
    (2960, 1280),
    (0, 2000),
    (1536, 2000),
    (0, 0),
];
struct Cell {
    interior: Region,
    storage: Region,
    bytes: Vec<u8>,
}

/// Mirrors the pinned production private1536 grid enumeration; public gutter/missing remain authority.
/// The caller supplies a nonempty in-bounds viewport; this diagnostic never changes product caching.
fn cells(view: Region, frame: Frame) -> Vec<Region> {
    let mut result = Vec::new();
    for row in view.y / TILE..=(view.y + view.height - 1) / TILE {
        for col in view.x / TILE..=(view.x + view.width - 1) / TILE {
            let x = col * TILE;
            let y = row * TILE;
            result.push(Region {
                x,
                y,
                width: TILE.min(frame.width - x),
                height: TILE.min(frame.height - y),
            });
        }
    }
    result
}
/// Copies disjoint cached interiors into a viewport without alpha blending or background compositing.
/// # Errors
/// Rejects missing pixel coverage; storage gutters remain intact and are never double-composed.
fn viewport(view: Region, cache: &BTreeMap<(u32, u32), Cell>) -> Result<Vec<u8>> {
    if !missing(view, cache.values().map(|c| c.interior)).is_empty() {
        return Err("viewport missing cells".into());
    }
    let mut out = vec![0; view.width as usize * view.height as usize * 4];
    for cell in cache.values() {
        if let Some(r) = intersection(view, cell.interior) {
            for y in r.y..r.y + r.height {
                let src = (((y - cell.storage.y) * cell.storage.width + r.x - cell.storage.x) * 4)
                    as usize;
                let dst = (((y - view.y) * view.width + r.x - view.x) * 4) as usize;
                let n = r.width as usize * 4;
                out[dst..dst + n].copy_from_slice(&cell.bytes[src..src + n]);
            }
        }
    }
    Ok(out)
}
/// Copies a native full-root crop; this independent reference check has no pan timing contribution.
fn crop(bytes: &[u8], width: u32, r: Region) -> Vec<u8> {
    let mut out = Vec::with_capacity(r.width as usize * r.height as usize * 4);
    for y in r.y..r.y + r.height {
        let p = ((y * width + r.x) * 4) as usize;
        out.extend_from_slice(&bytes[p..p + r.width as usize * 4]);
    }
    out
}
/// Reports separate alpha, visible/hidden RGB differences without declaring perceptual equivalence.
/// # Panics
/// Panics if caller violates the equal checked RGBA dimension invariant.
fn difference(a: &[u8], b: &[u8]) -> Value {
    assert_eq!(a.len(), b.len());
    let mut alpha = 0usize;
    let mut visible = 0usize;
    let mut hidden = 0usize;
    let mut max_alpha = 0u8;
    let mut max_rgb = 0u8;
    for (a, b) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        if a[3] != b[3] {
            alpha += 1;
        }
        max_alpha = max_alpha.max(a[3].abs_diff(b[3]));
        if a[..3] != b[..3] {
            if a[3] > 0 || b[3] > 0 {
                visible += 1
            } else {
                hidden += 1
            }
        }
        for k in 0..3 {
            max_rgb = max_rgb.max(a[k].abs_diff(b[k]));
        }
    }
    json!({"alpha_pixels":alpha,"visible_rgb_pixels":visible,"hidden_rgb_pixels":hidden,"max_alpha_delta":max_alpha,"max_rgb_delta":max_rgb})
}
/// Runs three pixel-cache-cold rounds retaining one canonical scene, prepared root or encoded GPU scene.
/// Real production-sized cells/gutters are painted on misses; hits perform only viewport byte copying.
/// # Errors
/// Rejects wrong worker count/route/output, unsupported scene, root authority failure, and native errors.
/// The fixed vector4x trace and externally supervised budgets are research inputs, not product policy.
pub(super) fn run(path: &Path, route: &str, args: Vec<String>) -> Result<()> {
    if std::env::var("RAYON_NUM_THREADS").ok().as_deref() != Some("4") {
        return Err("pan requires4workers".into());
    }
    if !matches!(route, "cpu" | "hybrid" | "hybrid-refined") {
        return Err("compute native camera offset unavailable; pan deferred".into());
    }
    if path.exists() {
        return Err("fresh pan output required".into());
    }
    fs::create_dir_all(path)?;
    fs::write(path.join("phase.txt"), "common-canonical")?;
    let project = args
        .first()
        .ok_or("pan requires calibrated vector project")?;
    let setup = Instant::now();
    let loaded = toniator_io::load(Path::new(project))?;
    loaded.document().require_saved_calibration_authority()?;
    let session = toniator_domain::DocumentSession::new(loaded.document().clone())?;
    let mut media = toniator_engine::open_source_media(
        loaded.sources(),
        toniator_engine::MediaTools::default(),
        &|| false,
    )?;
    let frame_number = session.document().project_timing().frame_range().start();
    let request =
        toniator_engine::frame_evaluation_request(&session, &mut media, frame_number, &|| false)?
            .for_scene()?;
    let mut evaluation_cache = toniator_engine::EvaluationProfileCache::default();
    let result = toniator_engine::evaluate_profiled_cached_with_limits(
        request,
        toniator_engine::EvaluationLimits::default(),
        &mut evaluation_cache,
    )?;
    let scene: Arc<RenderScene> = result.result.shared_scene();
    let identity = perf_probe::audit(&scene)?;
    if scene.canvas().width != 900. || scene.canvas().height != 620. {
        return Err("fixed trace requires genuine900x620vector".into());
    }
    let common = setup.elapsed().as_secs_f64();
    let scale = 4.;
    let frame = Frame {
        width: 3600,
        height: 2480,
        scale,
        background: RasterBackground::Transparent,
    };
    let t = Instant::now();
    fs::write(path.join("phase.txt"), "retained-preparation")?;
    let mut cpu = if route == "cpu" {
        Some(PreviewRoot::new(
            scene.clone(),
            scale,
            RasterBackground::Transparent,
            RasterizationLimits::new(
                toniator_engine::EvaluationLimits::default().max_flattened_raster_edges(),
            )?,
            &|| false,
        )?)
    } else {
        None
    };
    let mut bridge = Value::Null;
    let prefix = args
        .get(1)
        .filter(|value| value.as_str() != "all" && value.as_str() != "projection")
        .map(|value| value.parse::<usize>())
        .transpose()?;
    if args.get(1).is_some_and(|value| value == "projection") {
        let projection = perf_probe::refined_projection(&scene, scale, None)?;
        fs::write(
            path.join("projection.json"),
            serde_json::to_vec_pretty(&json!({"canonical":identity,"projection":projection}))?,
        )?;
        return Ok(());
    }
    let mut gpu = if route.starts_with("hybrid") {
        let (encoded, record) =
            perf_probe::encode_scene_scaled(&scene, route, prefix, 3600, 2480, scale)?;
        bridge = record;
        let gpu = pollster::block_on(Gpu::new())?;
        fs::write(
            path.join("adapter.json"),
            serde_json::to_vec_pretty(&gpu.info)?,
        )?;
        Some(perf_probe::Resident::new(gpu, encoded, 1, 1)?)
    } else {
        None
    };
    let prep = t.elapsed().as_secs_f64();
    let mut events = Vec::new();
    let mut checks = Vec::new();
    for round in 0..3 {
        // Only pixel coverage is cleared. CPU root debt/generation and GPU scene/resources persist.
        let mut cache = BTreeMap::new();
        let mut cell_images = Vec::new();
        for (step, (x, y)) in TRACE.iter().copied().enumerate() {
            fs::write(
                path.join("phase.txt"),
                if step == 0 {
                    "initial-cached-cell"
                } else {
                    "pan-events"
                },
            )?;
            let view = Region {
                x,
                y,
                width: 640,
                height: 480,
            };
            let t = Instant::now();
            let mut rendered = Vec::new();
            for interior in cells(view, frame) {
                if cache.contains_key(&(interior.x, interior.y)) {
                    continue;
                }
                let storage = gutter(interior, frame);
                let c = Instant::now();
                let bytes = if let Some(root) = &mut cpu {
                    let cert = root.certify(storage, &|| false)?;
                    let snap = root.snapshot(&cert)?;
                    let tile = snap.paint(storage, &cert, &|| false)?;
                    tile.surface(root, &cert)?.pixels().to_vec()
                } else {
                    let resident = gpu.as_mut().ok_or("missing GPU")?;
                    resident
                        .resize_region(storage.width.try_into()?, storage.height.try_into()?)?;
                    let raw = resident.render_offset((-(storage.x as i32), -(storage.y as i32)))?;
                    resident.encode(raw)
                };
                let seconds = c.elapsed().as_secs_f64();
                rendered.push(json!({"interior":format!("{interior:?}"),"storage":format!("{storage:?}"),"render_certify_convert_seconds":seconds}));
                cache.insert(
                    (interior.x, interior.y),
                    Cell {
                        interior,
                        storage,
                        bytes,
                    },
                );
                cell_images.push((interior.x, interior.y));
            }
            // Production presentation needs one whole sampled-view certificate even on cache hits.
            let certification_started = Instant::now();
            if let Some(root) = &mut cpu {
                let certificate = root.certify(gutter(view, frame), &|| false)?;
                if !certificate.covers(view) {
                    return Err("whole viewport not certified".into());
                }
            }
            let display_certification = certification_started.elapsed().as_secs_f64();
            let render_elapsed = t.elapsed().as_secs_f64();
            let t = Instant::now();
            let bytes = viewport(view, &cache)?;
            let ownership = bytes.clone();
            std::hint::black_box(&ownership);
            let copy = t.elapsed().as_secs_f64();
            events.push(json!({"round":round,"step":step,"viewport":[x,y,640,480],"initial_cached_setup":step==0,"cache_hit":rendered.is_empty(),"new_cells":rendered,"cell_work_seconds_including_whole_view_certificate":render_elapsed,"whole_view_certificate_seconds":display_certification,"viewport_copy_and_upload_ownership_proxy_seconds":copy,"total_miss_or_hit_seconds":render_elapsed+copy}));
            // PNG evidence is deliberately outside the pan event clock.
            if round == 0 {
                perf_probe::save(path, &format!("viewport-{step}"), 640, 480, bytes)?;
            }
        }
        fs::write(
            path.join("events-partial.json"),
            serde_json::to_vec_pretty(&events)?,
        )?;
        if round == 0 {
            for cell in cache.values() {
                perf_probe::save(
                    path,
                    &format!("cell-{}-{}", cell.interior.x, cell.interior.y),
                    cell.storage.width.try_into()?,
                    cell.storage.height.try_into()?,
                    cell.bytes.clone(),
                )?;
            }
        }
        if round == 2 {
            fs::write(path.join("phase.txt"), "offset-reference-outside-pan-clock")?;
            let full = Region {
                x: 0,
                y: 0,
                width: frame.width,
                height: frame.height,
            };
            let reference = if let Some(root) = &mut cpu {
                let cert = root.certify(full, &|| false)?;
                let snap = root.snapshot(&cert)?;
                let tile = snap.paint(full, &cert, &|| false)?;
                tile.surface(root, &cert)?.pixels().to_vec()
            } else {
                let resident = gpu.as_mut().ok_or("missing GPU")?;
                resident.resize_region(3600, 2480)?;
                let raw = resident.render_offset((0, 0))?;
                resident.encode(raw)
            };
            perf_probe::save(path, "full-zoom-reference", 3600, 2480, reference.clone())?;
            for cell in cache.values() {
                checks.push(json!({"storage":format!("{:?}",cell.storage),"region_vs_native_full_crop":difference(&cell.bytes,&crop(&reference,3600,cell.storage))}));
            }
        }
    }
    let after = perf_probe::audit(&scene)?;
    if identity != after {
        return Err("canonical scene changed".into());
    }
    let report = json!({"route":route,"prefix_admission":prefix,"scale":scale,"frame":[3600,2480],"viewport":[640,480],"tile_size":TILE,"gutter_pixels":1,"common_canonical_for_scene_seconds":common,"retained_preparation_seconds":prep,"canonical":identity,"canonical_after":after,"bridge":bridge,"events":events,"offset_reference_checks":checks,"cpu_root_facts":cpu.as_ref().map(|root|root.facts().map(|f|format!("{f:?}"))).transpose()?,"cache_model":"three independent pixel-cache-cold rounds; initialcellcached; backgroundprefilldisabled only in benchmark; geometry/debt/GPU resources retained; per-region target allocation included; all-scene GPU alpha upload/scheduling included","upload_proxy":"RGBA viewport assembly+ownership-copy only, no actualGTK/GDK upload/compositor","contract":"approximate diagnostic candidate; no quality acceptance"});
    fs::write(
        path.join("results.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", report);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    /// Verifies the approved real-grid trace has five new cells and returns to a true cache hit.
    #[test]
    fn production_grid_trace_has_five_fresh_cells() {
        let f = Frame {
            width: 3600,
            height: 2480,
            scale: 4.,
            background: RasterBackground::Transparent,
        };
        let mut seen = std::collections::BTreeSet::new();
        let mut counts = Vec::new();
        for (x, y) in TRACE {
            let mut n = 0;
            for c in cells(
                Region {
                    x,
                    y,
                    width: 640,
                    height: 480,
                },
                f,
            ) {
                n += usize::from(seen.insert((c.x, c.y)));
            }
            counts.push(n);
        }
        assert_eq!(counts, vec![1, 1, 0, 1, 0, 2, 1, 0, 0]);
    }
    /// Verifies neighboring cache interiors copy exact RGBA through their real storage gutters.
    #[test]
    fn viewport_copy_preserves_gutter_phase() {
        let mut cache = BTreeMap::new();
        for (x, color) in [(0, 7), (2, 19)] {
            let r = Region {
                x,
                y: 0,
                width: 2,
                height: 1,
            };
            let s = Region {
                x: x.saturating_sub(1),
                y: 0,
                width: 3,
                height: 1,
            };
            cache.insert(
                (x, 0),
                Cell {
                    interior: r,
                    storage: s,
                    bytes: vec![color; 12],
                },
            );
        }
        assert_eq!(
            viewport(
                Region {
                    x: 1,
                    y: 0,
                    width: 2,
                    height: 1
                },
                &cache
            )
            .unwrap(),
            vec![7, 7, 7, 7, 19, 19, 19, 19]
        );
    }
}
