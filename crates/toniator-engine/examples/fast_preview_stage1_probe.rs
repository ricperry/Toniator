//! Bounded Stage 1 evidence driver; this is not a product CLI command or a performance benchmark.
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::Path,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use toniator_engine::{EvaluationLimits, EvaluationScheduler};
use toniator_patterns::CanonicalMark;
use toniator_render::{
    GeometryOutput, RasterBackground, RasterSurface, RenderScene, encode_png,
    fast_preview::{FastPreviewDevice, FastPreviewLimits, FastPreviewPlan},
    preview_tiles::Region,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Evaluates and accepts a genuine current calibrated project, then writes native CPU/GPU artifacts.
/// One invocation owns one scene/device and runs serially inside the external guarded scope.
/// # Errors
/// Propagates input/authority/timeout/consumer/IO failures without overwriting an evidence directory.
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
    if let Some(scale) = args.get(3) {
        zoom_artifacts(
            output,
            Arc::clone(&scene),
            scale.parse()?,
            serde_json::json!({"canonical":canonical,"source_identity":result.source_identity(),"project":project}),
        )?;
        scheduler.shutdown()?;
        return Ok(());
    }

    fs::write(output.join("phase.txt"), "accurate-cpu-reference")?;
    let cpu = toniator_render::rasterize_cancellable(
        &scene,
        RasterBackground::Transparent,
        toniator_render::RasterizationLimits::new(
            EvaluationLimits::default().max_flattened_raster_edges(),
        )?,
        &|| false,
    )?;
    save(output, "cpu-full", &cpu)?;
    fs::write(output.join("phase.txt"), "admitted-refined-preparation")?;
    let plan = FastPreviewPlan::new(
        Arc::clone(&scene),
        1.0,
        RasterBackground::Transparent,
        FastPreviewLimits::default(),
        &|| false,
    )?;
    let reservation = plan.reservation();
    let mut device = FastPreviewDevice::new(&plan, &|| false)?;
    let adapter = device.adapter_name().to_owned();
    let root = device.prepare(plan, &|| false)?;
    fs::write(
        output.join("phase.txt"),
        "gpu-full-and-retained-region-readback",
    )?;
    let frame = root.frame();
    let gpu = device.render_region(
        &root,
        Region {
            x: 0,
            y: 0,
            width: frame.width,
            height: frame.height,
        },
        &|| false,
    )?;
    save(output, "fast-full", &gpu.surface)?;
    let locations = [
        (0, 0),
        (frame.width / 2 - 64, frame.height / 2 - 64),
        (frame.width - 128, frame.height - 128),
    ];
    let mut regions = Vec::new();
    for (index, (x, y)) in locations.into_iter().enumerate() {
        let region = Region {
            x,
            y,
            width: 128,
            height: 128,
        };
        let cropped_cpu = crop(&cpu, region)?;
        let cropped_gpu = crop(&gpu.surface, region)?;
        let retained = device.render_region(&root, region, &|| false)?;
        if retained.surface != cropped_gpu {
            return Err("retained region differs from native full crop".into());
        }
        save(output, &format!("cpu-crop-{index}"), &cropped_cpu)?;
        save(output, &format!("fast-crop-{index}"), &retained.surface)?;
        regions.push(serde_json::json!({"region_xywh":[x,y,128,128],"retained_native_offset_equals_full_crop":true,"difference":difference(&cropped_cpu,&cropped_gpu)}));
    }
    let value = serde_json::json!({
        "stage":"1 headless only; no UI/runtime epochs or performance claim",
        "project":project,"project_sha256":format!("{:x}", Sha256::digest(fs::read(project)?)),
        "source_identity":result.source_identity(),"canonical":canonical,
        "scene_ready_raster_payload":false,"scheduler_accepted":true,
        "adapter":adapter,"reservation":format!("{reservation:?}"),
        "algorithm":gpu.provenance.algorithm,"full_difference":difference(&cpu,&gpu.surface),"crops":regions,
        "approximation":"Separate 1/64 output-pixel Bezier and 1/128 kurbo flattening targets; no analytic coverage/error certificate. Model blend and final backdrop remain fixed.",
        "font_caveat":"SVG live text uses the installed font resolver; decoded_pixel_hash pins the actual pixels. Exact cross-machine text pixels are not a portable golden."
    });
    fs::write(
        output.join("results.json"),
        serde_json::to_vec_pretty(&value)?,
    )?;
    fs::write(output.join("phase.txt"), "complete")?;
    scheduler.shutdown()?;
    Ok(())
}

/// Paints two actual production cells with shared retained scene/device/exact-zoom preparation.
/// CPU comparison comes from the separately pinned exact whole-root reference, never a GPU proxy.
/// # Errors
/// Propagates admission/native/IO failures; no timing or whole-application performance claim follows.
fn zoom_artifacts(
    output: &Path,
    scene: Arc<RenderScene>,
    scale: f64,
    mut provenance: serde_json::Value,
) -> Result<()> {
    fs::write(output.join("phase.txt"), "exact-zoom-resource-admission")?;
    let plan = FastPreviewPlan::new(
        scene,
        scale,
        RasterBackground::Transparent,
        FastPreviewLimits::default(),
        &|| false,
    )?;
    let mut device = FastPreviewDevice::new(&plan, &|| false)?;
    let root = device.prepare(plan, &|| false)?;
    let frame = root.frame();
    let mut cells = Vec::new();
    let mut root_id = None;
    for (index, x) in [0, 1536].into_iter().enumerate() {
        let interior = Region {
            x,
            y: 0,
            width: 1536.min(frame.width - x),
            height: 1536.min(frame.height),
        };
        let region = toniator_engine::preview_cache::gutter(interior, frame);
        let result = device.render_region(&root, region, &|| false)?;
        if let Some(id) = root_id
            && id != result.provenance.root_id
        {
            return Err("pan replaced encoded root".into());
        }
        root_id = Some(result.provenance.root_id);
        save(output, &format!("fast-cell-{index}"), &result.surface)?;
        cells.push(serde_json::json!({"storage_xywh":[region.x,region.y,region.width,region.height],"root_id":result.provenance.root_id}));
    }
    provenance["scale"] = serde_json::json!(scale);
    provenance["frame"] = serde_json::json!([frame.width, frame.height]);
    provenance["cells"] = serde_json::json!(cells);
    provenance["reservation"] = serde_json::json!(format!("{:?}", device.reservation()));
    provenance["adapter"] = serde_json::json!(device.adapter_name());
    provenance["algorithm"] = serde_json::json!(toniator_render::fast_preview::ALGORITHM_VERSION);
    provenance["performance_claim"] = serde_json::json!(false);
    fs::write(
        output.join("results.json"),
        serde_json::to_vec_pretty(&provenance)?,
    )?;
    fs::write(output.join("phase.txt"), "complete")?;
    Ok(())
}

/// Writes a native straight-RGBA PNG and a separately labeled alpha-plane diagnostic.
/// # Errors
/// Propagates PNG/IO errors; no flattening or background composite is performed.
fn save(directory: &Path, name: &str, surface: &RasterSurface) -> Result<()> {
    fs::write(directory.join(format!("{name}.png")), encode_png(surface)?)?;
    let mut alpha = Vec::with_capacity(surface.pixels().len());
    for pixel in surface.pixels().chunks_exact(4) {
        alpha.extend_from_slice(&[pixel[3], pixel[3], pixel[3], 255]);
    }
    let alpha = RasterSurface::new(surface.width(), surface.height(), alpha)?;
    fs::write(
        directory.join(format!("{name}-alpha-plane.png")),
        encode_png(&alpha)?,
    )?;
    Ok(())
}

/// Copies a complete in-bounds native crop without changing RGB/alpha representation.
/// # Errors
/// Returns the surface constructor's dimension/buffer error on a broken evidence invariant.
fn crop(surface: &RasterSurface, region: Region) -> Result<RasterSurface> {
    let mut bytes = Vec::with_capacity(region.width as usize * region.height as usize * 4);
    for y in region.y..region.y + region.height {
        let offset = ((y * surface.width() + region.x) * 4) as usize;
        bytes.extend_from_slice(&surface.pixels()[offset..offset + region.width as usize * 4]);
    }
    Ok(RasterSurface::new(region.width, region.height, bytes)?)
}

/// Reports separate alpha, visible/hidden RGB and associated-linear differences without acceptance claims.
/// # Panics
/// Panics if this driver violates its equal checked dimension invariant.
fn difference(cpu: &RasterSurface, gpu: &RasterSurface) -> serde_json::Value {
    assert_eq!((cpu.width(), cpu.height()), (gpu.width(), gpu.height()));
    let mut alpha_pixels = 0_u64;
    let mut visible_rgb = 0_u64;
    let mut hidden_rgb = 0_u64;
    let mut alpha_sum = 0_u64;
    let mut max_alpha = 0_u8;
    let mut associated_sum = 0_f64;
    for (a, b) in cpu
        .pixels()
        .chunks_exact(4)
        .zip(gpu.pixels().chunks_exact(4))
    {
        let delta = a[3].abs_diff(b[3]);
        alpha_pixels += u64::from(delta != 0);
        alpha_sum += u64::from(delta);
        max_alpha = max_alpha.max(delta);
        if a[..3] != b[..3] {
            if a[3] > 0 || b[3] > 0 {
                visible_rgb += 1;
            } else {
                hidden_rgb += 1;
            }
        }
        let mut associated = 0_f64;
        for k in 0..3 {
            associated = associated.max(
                (toniator_render::srgb_to_linear(f64::from(a[k]) / 255.) * f64::from(a[3]) / 255.
                    - toniator_render::srgb_to_linear(f64::from(b[k]) / 255.) * f64::from(b[3])
                        / 255.)
                    .abs(),
            );
        }
        associated_sum += associated;
    }
    let count = f64::from(cpu.width()) * f64::from(cpu.height());
    serde_json::json!({"alpha_pixels":alpha_pixels,"alpha_mae_bytes":alpha_sum as f64/count,"max_alpha_delta":max_alpha,"visible_rgb_pixels":visible_rgb,"hidden_rgb_pixels":hidden_rgb,"associated_linear_max_channel_mean":associated_sum/count})
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
