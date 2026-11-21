//! Isolated compatibility experiment; this binary never replaces Toniator rendering authority.
use serde_json::{Value, json};
use std::{error::Error, fs, path::Path, time::Instant};
use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, HalftoneChannelModel, PatternMechanismId,
};
use toniator_geometry::{
    CanonicalFillRule, CanonicalMark, CanonicalPathMark, CubicBezierSegment, CurvePath,
    CurveSegment, FamilySiteId, FamilySiteProvenance, LineSegment, PathClosure, Point2, SiteScope,
};
use toniator_render::{
    GeometryOutput, OutputRasterTarget, RasterAntialiasing, RasterBackground, RasterizationLimits,
    RenderLayer, RenderScene,
};
use vello_common::{
    kurbo::{BezPath, PathEl, Shape},
    peniko::{Color, Fill, ImageAlphaType},
    pixmap::{PixelMetadata, Pixmap},
};

mod blend_probe;
mod circle_probe;
mod pan_probe;
mod perf_probe;

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const SIZE: u16 = 64;

struct Witness {
    name: &'static str,
    paths: Vec<&'static str>,
    fill: CanonicalFillRule,
    paints: Vec<ColorValue>,
    model: Option<HalftoneChannelModel>,
}

/// Constructs exact linear canonical paint; conversion to sRGB occurs only at renderer boundaries.
fn paint(red: f64, green: f64, blue: f64, alpha: f64) -> ColorValue {
    ColorValue {
        red,
        green,
        blue,
        alpha,
    }
}

/// Supplies tiny connected line/cubic witnesses, including holes encoded with a retraced bridge.
/// These are diagnostic inputs, never substitutes for the two project-wide artwork workloads.
fn witnesses() -> Vec<Witness> {
    let white = paint(1., 1., 1., 1.);
    let mut result = Vec::new();
    for (name, paths, fill) in [
        (
            "overlap_union",
            vec!["M0 0L0.6 0L0.6 1L0 1Z", "M0.3 0L0.9 0L0.9 1L0.3 1Z"],
            CanonicalFillRule::NonZero,
        ),
        (
            "preunion_control",
            vec!["M0 0L0.9 0L0.9 1L0 1Z"],
            CanonicalFillRule::NonZero,
        ),
        (
            "touching_halves",
            vec!["M0 0L0.5 0L0.5 1L0 1Z", "M0.5 0L1 0L1 1L0.5 1Z"],
            CanonicalFillRule::NonZero,
        ),
        (
            "holes_overlap",
            vec![
                "M2.25 2.25L30.75 2.25L30.75 30.75L2.25 30.75L2.25 2.25L10 10L10 23L23 23L23 10L10 10L2.25 2.25Z",
                "M16 8L40 8L40 26L16 26Z",
            ],
            CanonicalFillRule::EvenOdd,
        ),
        (
            "cross_canvas_cubic",
            vec!["M-20 10C20 -30 50 90 84 10L84 32C45 100 20 -10 -20 32Z"],
            CanonicalFillRule::NonZero,
        ),
        (
            "tangent_coincident",
            vec![
                "M2 2C2 20 30 20 30 2L2 2Z",
                "M2 2C2 20 30 20 30 2L2 2Z",
                "M30 2C30 20 58 20 58 2L30 2Z",
            ],
            CanonicalFillRule::NonZero,
        ),
        (
            "subpixel_sliver",
            vec!["M0 0L0.0000001 0L0.0000001 64L0 64Z"],
            CanonicalFillRule::NonZero,
        ),
        (
            "fractional_clip",
            vec!["M-1 -1L0.5 -1L0.5 0.5L-1 0.5Z"],
            CanonicalFillRule::NonZero,
        ),
        (
            "tile_seams",
            vec!["M15.999 0C16.2 20 31.8 40 32.001 64L32.02 64C31.82 40 16.22 20 16.019 0Z"],
            CanonicalFillRule::NonZero,
        ),
    ] {
        result.push(Witness {
            name,
            paints: vec![white.clone(); paths.len()],
            paths,
            fill,
            model: None,
        });
    }
    result.push(Witness {
        name: "translucent_source_colors",
        paths: vec![
            "M2 2L40 2L40 40L2 40Z",
            "M10 10L50 10L50 50L10 50Z",
            "M10 10L50 10L50 50L10 50Z",
        ],
        fill: CanonicalFillRule::NonZero,
        paints: vec![
            paint(1., 0., 0., 1.),
            paint(0., 1., 0., 1.),
            paint(0., 0., 1., 1.),
        ],
        model: Some(HalftoneChannelModel::SourceColorAlpha),
    });
    for (name, model, paints) in [
        (
            "rgb_linear",
            HalftoneChannelModel::Rgb,
            vec![paint(1., 0., 0., 0.5), paint(0., 1., 0., 0.5)],
        ),
        (
            "cmyk_linear",
            HalftoneChannelModel::Cmyk,
            vec![paint(0., 1., 1., 0.5), paint(1., 0., 1., 0.5)],
        ),
    ] {
        result.push(Witness {
            name,
            paths: vec!["M2 2L40 2L40 40L2 40Z", "M10 10L50 10L50 50L10 50Z"],
            fill: CanonicalFillRule::NonZero,
            paints,
            model: Some(model),
        });
    }
    result
}

/// Converts a single connected SVG diagnostic path into canonical line/cubic authority.
/// # Errors
/// Rejects unsupported quadratic/disconnected paths or canonical validation failures.
fn canonical(path: &str, index: usize, fill_rule: CanonicalFillRule) -> Result<CanonicalMark> {
    let bez = BezPath::from_svg(path)?;
    let mut current = Point2::new(0., 0.);
    let mut start = current;
    let mut segments = Vec::new();
    for element in bez.elements() {
        let point = |p: vello_common::kurbo::Point| Point2::new(p.x, p.y);
        match *element {
            PathEl::MoveTo(p) if segments.is_empty() => {
                current = point(p);
                start = current;
            }
            PathEl::LineTo(p) => {
                let end = point(p);
                segments.push(CurveSegment::Line(LineSegment::new(current, end)?));
                current = end;
            }
            PathEl::CurveTo(a, b, p) => {
                let end = point(p);
                segments.push(CurveSegment::CubicBezier(CubicBezierSegment::new(
                    current,
                    point(a),
                    point(b),
                    end,
                )?));
                current = end;
            }
            PathEl::ClosePath => {
                if current != start {
                    segments.push(CurveSegment::Line(LineSegment::new(current, start)?));
                }
            }
            _ => return Err("unsupported diagnostic path".into()),
        }
    }
    Ok(CanonicalMark::ClosedPath(CanonicalPathMark::new(
        FamilySiteId {
            mechanism_id: PatternMechanismId(1),
            ordinal: index,
        },
        CurvePath::new(segments, PathClosure::Closed)?,
        SiteScope::Canvas,
        FamilySiteProvenance::Random {
            candidate_ordinal: index,
            accepted_ordinal: index,
            exclusion_neighbor_ordinal: None,
        },
        fill_rule,
    )?))
}

/// Constructs a public canonical CPU scene without touching authored documents or production code.
/// # Errors
/// Propagates public geometry/scene validation errors.
fn cpu_scene(w: &Witness) -> Result<RenderScene> {
    let marks = w
        .paths
        .iter()
        .enumerate()
        .map(|(i, p)| canonical(p, i, w.fill))
        .collect::<Result<Vec<_>>>()?;
    let layers = match w.model {
        Some(HalftoneChannelModel::SourceColorAlpha) => {
            vec![RenderLayer::new_source_color_geometry(
                ChannelId(1),
                true,
                0.5,
                marks,
                w.paints.clone(),
            )?]
        }
        Some(_) => marks
            .into_iter()
            .zip(&w.paints)
            .enumerate()
            .map(|(i, (mark, color))| {
                RenderLayer::new(
                    ChannelId(i as u64 + 1),
                    true,
                    color.clone(),
                    1.,
                    GeometryOutput::CanonicalMarks(vec![mark]),
                )
            })
            .collect::<std::result::Result<Vec<_>, _>>()?,
        None => vec![RenderLayer::new(
            ChannelId(1),
            true,
            w.paints[0].clone(),
            1.,
            GeometryOutput::CanonicalMarks(marks),
        )?],
    };
    let canvas = CanvasSpec {
        width: SIZE.into(),
        height: SIZE.into(),
    };
    Ok(match w.model {
        Some(model) => RenderScene::new_modeled(
            canvas,
            "tiny-family".into(),
            "tiny-realization".into(),
            model,
            layers,
        )?,
        None => RenderScene::new(
            canvas,
            "tiny-family".into(),
            "tiny-realization".into(),
            layers,
        )?,
    })
}

/// Converts exact exported union geometry into the supported native Vello painting API.
/// Each primitive's fill and opaque-run union remain owned by production SVG preparation.
/// RGB/CMYK filters are deliberately unsupported and recorded as an incompatibility.
/// # Errors
/// Rejects missing geometry or paint indices, SVG errors and unsupported exported elements.
fn gpu_scene(w: &Witness, svg: &str, route: &str) -> Result<CandidateScene> {
    let document = roxmltree::Document::parse(svg)?;
    let mut scene = match route {
        "hybrid" => CandidateScene::Hybrid(Box::new(vello_gpu::Scene::new(SIZE, SIZE))),
        "compute-area" => {
            CandidateScene::Compute(Box::new(vello::Scene::new()), vello::AaConfig::Area)
        }
        "compute-msaa16" => {
            CandidateScene::Compute(Box::new(vello::Scene::new()), vello::AaConfig::Msaa16)
        }
        _ => return Err("route must be hybrid, compute-area or compute-msaa16".into()),
    };
    if let CandidateScene::Hybrid(scene) = &mut scene {
        scene.set_aliasing_threshold(None);
    }
    let mut count = 0;
    for node in document.descendants().filter(|n| {
        n.is_element()
            && n.attribute("id")
                .is_some_and(|id| id.starts_with("channel-"))
    }) {
        let path = match node.tag_name().name() {
            "path" => BezPath::from_svg(node.attribute("d").ok_or("missing d")?)?,
            "circle" => {
                let value = |name| {
                    node.attribute(name)
                        .ok_or("missing circle attribute")?
                        .parse::<f64>()
                        .map_err(|_| "invalid circle attribute")
                };
                vello_common::kurbo::Circle::new((value("cx")?, value("cy")?), value("r")?)
                    .to_path(0.001)
            }
            _ => continue,
        };
        let color = if w.model.is_none() {
            &w.paints[0]
        } else {
            w.paints.get(count).ok_or("paint index mismatch")?
        };
        let native_paint = Color::new([
            toniator_render::linear_to_srgb(color.red) as f32,
            toniator_render::linear_to_srgb(color.green) as f32,
            toniator_render::linear_to_srgb(color.blue) as f32,
            (color.alpha
                * if matches!(w.model, Some(HalftoneChannelModel::SourceColorAlpha)) {
                    0.5
                } else {
                    1.0
                }) as f32,
        ]);
        let fill = if node.attribute("fill-rule") == Some("evenodd") {
            Fill::EvenOdd
        } else {
            Fill::NonZero
        };
        match &mut scene {
            CandidateScene::Hybrid(scene) => {
                scene.set_paint(native_paint);
                scene.set_fill_rule(fill);
                scene.fill_path(&path);
            }
            CandidateScene::Compute(scene, _) => scene.fill(
                fill,
                vello::kurbo::Affine::IDENTITY,
                native_paint,
                None,
                &path,
            ),
        }
        count += 1;
    }
    if count == 0 {
        return Err("no exported union paths found".into());
    }
    Ok(scene)
}

enum CandidateScene {
    Hybrid(Box<vello_gpu::Scene>),
    Compute(Box<vello::Scene>, vello::AaConfig),
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: Value,
}
impl Gpu {
    /// Selects only an AMD discrete Vulkan adapter; software and other backends are rejected.
    /// # Errors
    /// Returns adapter/device initialization errors or missing256MiB storage-binding capability.
    /// The approved isolated capacity variant requests only supported local device limits.
    async fn new() -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapters = instance.enumerate_adapters(wgpu::Backends::VULKAN).await;
        let adapter = adapters
            .into_iter()
            .find(|a| {
                let i = a.get_info();
                i.vendor == 0x1002
                    && i.device_type == wgpu::DeviceType::DiscreteGpu
                    && i.backend == wgpu::Backend::Vulkan
                    && i.device_pci_bus_id == "0000:0b:00.0"
            })
            .ok_or("no AMD discrete Vulkan adapter")?;
        let required_limits = wgpu::Limits {
            max_storage_buffer_binding_size: 256 * 1024 * 1024,
            ..Default::default()
        };
        if adapter.limits().max_storage_buffer_binding_size
            < required_limits.max_storage_buffer_binding_size
        {
            return Err(
                "adapter lacks approved256MiBstorage binding for capacity experiment".into(),
            );
        }
        let mut info = json!({"adapter":format!("{:?}",adapter.get_info()),"features":format!("{:?}",adapter.features()),"limits":format!("{:?}",adapter.limits()),"requested_storage_binding_bytes":required_limits.max_storage_buffer_binding_size,"requested_max_buffer_bytes":required_limits.max_buffer_size,"requested_features":"CLEAR_TEXTURE only; capacityconfiguration requests supportedlimits, no globalchange"});
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("isolated-tiny-pilot"),
                required_features: wgpu::Features::CLEAR_TEXTURE,
                required_limits,
                ..Default::default()
            })
            .await?;
        info["actual_device_limits"] = json!(format!("{:?}", device.limits()));
        info["actual_storage_binding_bytes"] =
            json!(device.limits().max_storage_buffer_binding_size);
        info["actual_max_buffer_bytes"] = json!(device.limits().max_buffer_size);
        Ok(Self {
            device,
            queue,
            info,
        })
    }
    /// Renders one bounded 64x64 native RGBA target and reads back all rows synchronously.
    /// Saves unmodified native bytes; hybrid output is premultiplied, compute output is already straight.
    /// Only hybrid pixels pass through upstream unassociation before PNG encoding.
    /// # Errors
    /// Propagates render, poll, mapping and file errors; no partial image is published on failure.
    fn render(&self, scene: &CandidateScene, raw: &Path) -> Result<Vec<u8>> {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tiny-native"),
            size: wgpu::Extent3d {
                width: SIZE.into(),
                height: SIZE.into(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        match scene {
            CandidateScene::Hybrid(scene) => {
                let (mut renderer, mut resources) = vello_gpu::Renderer::new(
                    &self.device,
                    &self.queue,
                    &vello_gpu::RenderTargetConfig {
                        format,
                        width: SIZE,
                        height: SIZE,
                    },
                );
                let depth = renderer.create_depth_texture_view(&vello_gpu::RenderSize {
                    width: SIZE,
                    height: SIZE,
                });
                renderer.render(
                    scene,
                    &mut resources,
                    &mut encoder,
                    vello_gpu::RenderTarget {
                        view: &view,
                        depth_view: Some(&depth),
                    },
                    &vello_gpu::TextureBindings::new(),
                    vello_gpu::RenderOptions::default(),
                )?;
            }
            CandidateScene::Compute(scene, aa) => {
                let mut renderer = vello::Renderer::new(
                    &self.device,
                    vello::RendererOptions {
                        use_cpu: false,
                        antialiasing_support: [*aa].into_iter().collect(),
                        num_init_threads: std::num::NonZeroUsize::new(1),
                        ..Default::default()
                    },
                )?;
                renderer.render_to_texture(
                    &self.device,
                    &self.queue,
                    scene,
                    &view,
                    &vello::RenderParams {
                        base_color: Color::TRANSPARENT,
                        width: SIZE.into(),
                        height: SIZE.into(),
                        antialiasing_method: *aa,
                    },
                )?;
            }
        }
        let stride = u32::from(SIZE) * 4;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tiny-readback"),
            size: u64::from(stride) * u64::from(SIZE),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: None,
                },
            },
            texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;
        let bytes = buffer.slice(..).get_mapped_range()?.to_vec();
        buffer.unmap();
        fs::write(raw, &bytes)?;
        Ok(match scene {
            CandidateScene::Hybrid(_) => {
                Pixmap::from_parts(bytes, SIZE, SIZE, PixelMetadata::default())
                    .take_rgba8(ImageAlphaType::Alpha)
            }
            CandidateScene::Compute(_, _) => bytes,
        })
    }
}

/// Writes native straight RGBA bytes as PNG through the existing public encoder boundary.
/// # Errors
/// Propagates surface-validation, PNG-encoding and file errors.
fn write_png(path: &Path, bytes: Vec<u8>) -> Result<()> {
    let surface = toniator_render::RasterSurface::new(SIZE.into(), SIZE.into(), bytes)?;
    fs::write(path, toniator_render::encode_png(&surface)?)?;
    Ok(())
}

/// Reports independent RGBA and alpha differences; byte equality is never an area certificate.
/// # Panics
/// Panics for unequal or shorter-than-one-pixel arrays; callers supply complete 64x64 RGBA.
fn compare(cpu: &[u8], gpu: &[u8]) -> Value {
    assert_eq!(cpu.len(), gpu.len());
    let visible_rgb_differences = cpu
        .chunks_exact(4)
        .zip(gpu.chunks_exact(4))
        .filter(|(a, b)| (a[3] > 0 || b[3] > 0) && a[..3] != b[..3])
        .count();
    let hidden_rgb_differences = cpu
        .chunks_exact(4)
        .zip(gpu.chunks_exact(4))
        .filter(|(a, b)| a[3] == 0 && b[3] == 0 && a[..3] != b[..3])
        .count();
    let diff = cpu
        .iter()
        .zip(gpu)
        .map(|(a, b)| a.abs_diff(*b))
        .collect::<Vec<_>>();
    json!({"byte_equal":cpu==gpu,"different_visible_rgb_pixels":visible_rgb_differences,"different_hidden_rgb_pixels":hidden_rgb_differences,"different_bytes":diff.iter().filter(|d|**d!=0).count(),
        "max_rgba_byte_difference":diff.iter().max(),"different_alpha_pixels":diff.iter().skip(3).step_by(4).filter(|d|**d!=0).count(),
        "max_alpha_byte_difference":diff.iter().skip(3).step_by(4).max(),"cpu_alpha_sum":cpu.iter().skip(3).step_by(4).map(|v|u64::from(*v)).sum::<u64>(),
        "gpu_alpha_sum":gpu.iter().skip(3).step_by(4).map(|v|u64::from(*v)).sum::<u64>(),"cpu_first_pixel":&cpu[..4],"gpu_first_pixel":&gpu[..4]})
}

/// Runs the tiny native Vulkan matrix or the explicitly selected blend probe.
/// Both retain canonical CPU authority and native artifacts without production integration.
/// # Errors
/// Returns setup/render/validation errors; diagnostic scaling requires explicit authorized selection.
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output = args.next().ok_or(
        "usage: toniator-gpu-vulkan-eval OUTPUT_DIRECTORY [hybrid|compute-area|compute-msaa16]",
    )?;
    let route = args.next().unwrap_or_else(|| "hybrid".into());
    match args.next().as_deref() {
        Some("blend") => return blend_probe::run(Path::new(&output), &route),
        Some("circles") => return circle_probe::run(Path::new(&output), &route),
        Some("pan") => return pan_probe::run(Path::new(&output), &route, args.collect()),
        Some("perf") => return perf_probe::run(Path::new(&output), &route, args.collect()),
        Some(_) => return Err("unknown experiment".into()),
        None => {}
    }
    let output = Path::new(&output);
    if output.exists() {
        return Err("output exists; choose a fresh evidence directory".into());
    }
    fs::create_dir_all(output)?;
    let started = Instant::now();
    let gpu = pollster::block_on(Gpu::new())?;
    fs::write(
        output.join("adapter.json"),
        serde_json::to_vec_pretty(&gpu.info)?,
    )?;
    let init_ms = started.elapsed().as_secs_f64() * 1000.;
    let mut reports = Vec::new();
    for witness in witnesses() {
        let scene = cpu_scene(&witness)?;
        let identity = scene.identity().scene_fingerprint().to_owned();
        let t = Instant::now();
        let svg = toniator_render::write_svg(&scene)?;
        let preparation_ms = t.elapsed().as_secs_f64() * 1000.;
        fs::write(output.join(format!("{}.svg", witness.name)), &svg)?;
        let t = Instant::now();
        let cpu = toniator_render::rasterize_output_cancellable(
            &scene,
            RasterBackground::Transparent,
            Some(OutputRasterTarget::new(SIZE.into(), SIZE.into())?),
            RasterAntialiasing::On,
            RasterizationLimits::default(),
            &|| false,
        )?;
        let cpu_ms = t.elapsed().as_secs_f64() * 1000.;
        let t = Instant::now();
        let encoded = gpu_scene(&witness, &svg, &route)?;
        let preprocessing_ms = t.elapsed().as_secs_f64() * 1000.;
        let t = Instant::now();
        let bytes = gpu.render(
            &encoded,
            &output.join(format!("{}.gpu-native.rgba", witness.name)),
        )?;
        let gpu_ms = t.elapsed().as_secs_f64() * 1000.;
        let mut report = compare(cpu.pixels(), &bytes);
        report["name"] = json!(witness.name);
        report["canonical_scene_identity"] = json!(identity);
        report["production_svg_preparation_ms"] = json!(preparation_ms);
        report["vello_cpu_preprocessing_ms"] = json!(preprocessing_ms);
        report["gpu_init_pipeline_render_readback_ms"] = json!(gpu_ms);
        report["cpu_raster_ms"] = json!(cpu_ms);
        report["native_model_equation_available"] = json!(witness.model.is_none());
        report["area_certificate_supported"] = json!(false);
        report["native_readback_alpha"] = json!(if route == "hybrid" {
            "premultiplied"
        } else {
            "straight; upstream fine.wgsl unassociates before textureStore"
        });
        fs::write(
            output.join(format!("{}.cpu.png", witness.name)),
            toniator_render::encode_png(&cpu)?,
        )?;
        fs::write(
            output.join(format!("{}.cpu.rgba", witness.name)),
            cpu.pixels(),
        )?;
        fs::write(
            output.join(format!("{}.gpu-straight.rgba", witness.name)),
            &bytes,
        )?;
        write_png(&output.join(format!("{}.gpu.png", witness.name)), bytes)?;
        if identity != scene.identity().scene_fingerprint() {
            return Err("canonical authority changed".into());
        }
        println!("{}", report);
        reports.push(report);
    }
    fs::write(
        output.join("results.json"),
        serde_json::to_vec_pretty(
            &json!({"route":route,"cold_device_init_ms":init_ms,"witnesses":reports,"source_capability_gate":"no Toniator accumulated area/work certificates; native sRGB paint composition is not linear RGB/CMYK integration", "timing_boundary":"diagnostic only; cold pipelines per witness; production SVG primitive normalization/compound assembly excluded from GPU-only interval; certified-simple contours avoid global Boolean"}),
        )?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Proves union occurs geometrically before coverage rather than alpha-over/max substitution.
    /// # Panics
    /// Panics if the public CPU witness no longer reports 0.9 union or complete touching halves.
    #[test]
    fn canonical_union_controls() {
        for (name, expected) in [("overlap_union", 230), ("touching_halves", 255)] {
            let witness = witnesses().into_iter().find(|w| w.name == name).unwrap();
            let scene = cpu_scene(&witness).unwrap();
            let cpu = toniator_render::rasterize_output_cancellable(
                &scene,
                RasterBackground::Transparent,
                Some(OutputRasterTarget::new(64, 64).unwrap()),
                RasterAntialiasing::On,
                RasterizationLimits::default(),
                &|| false,
            )
            .unwrap();
            assert_eq!(cpu.pixels()[3], expected);
            let svg = toniator_render::write_svg(&scene).unwrap();
            let doc = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(
                doc.descendants()
                    .filter(|n| n.has_tag_name("path")
                        && n.attribute("id").is_some_and(|s| s.starts_with("channel-")))
                    .count(),
                1
            );
        }
    }
    /// Proves comparison accounts for alpha separately and does not silently accept close pixels.
    /// # Panics
    /// Panics if a one-byte alpha discrepancy is hidden by RGB equality.
    #[test]
    fn alpha_discrepancy_is_reported() {
        let result = compare(&[255, 255, 255, 230], &[255, 255, 255, 229]);
        assert_eq!(result["byte_equal"], false);
        assert_eq!(result["max_alpha_byte_difference"], 1);
    }
}
