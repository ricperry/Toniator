//! Matched diagnostic performance and natural-artwork rendering; incompatible output is explicit.
use super::*;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use toniator_render::GeometryOutputPart;
use vello_common::{
    kurbo::{Affine, Circle, Rect},
    peniko::{BlendMode, Compose, Mix},
};

const CIRCLE_TOLERANCE: f64 = 1.0 / 64.0;

/// Visits every canonical circle in order and rejects unsupported geometry instead of dropping it.
/// # Errors
/// Returns an explicit unsupported-geometry error for noncircular primitives or sampled paints.
fn circles(
    scene: &RenderScene,
    mut visit: impl FnMut(&RenderLayer, u64, &str, toniator_geometry::Point2, f64) -> Result<()>,
) -> Result<()> {
    for layer in scene.layers() {
        for output in layer.outputs() {
            if output.primitive_paints().is_some() {
                return Err("performance bridge rejects per-primitive paints".into());
            }
            for part in output.geometry().parts() {
                match part {
                    GeometryOutputPart::CircularMarks(marks) => {
                        for m in marks {
                            visit(
                                layer,
                                output.output_layer_id.0,
                                &format!("{:?}", m.source_site_id),
                                m.center,
                                m.radius,
                            )?
                        }
                    }
                    GeometryOutputPart::CanonicalMarks(marks) => {
                        for m in marks {
                            match m {
                                CanonicalMark::Circle {
                                    source_site_id,
                                    center,
                                    radius,
                                    ..
                                } => visit(
                                    layer,
                                    output.output_layer_id.0,
                                    &format!("{source_site_id:?}"),
                                    *center,
                                    *radius,
                                )?,
                                _ => {
                                    return Err(
                                        "performance bridge rejects noncircle canonical mark"
                                            .into(),
                                    );
                                }
                            }
                        }
                    }
                    _ => return Err("performance bridge rejects noncircle geometry output".into()),
                }
            }
        }
    }
    Ok(())
}

/// Audits exact immutable canonical IDs/coordinate/radius bits and complete channel populations.
/// Invisible, zero-radius and off-canvas circles remain counted; nothing is spatially thinned.
/// # Errors
/// Rejects unsupported geometry before any candidate timing can be treated as matched.
pub(super) fn audit(scene: &RenderScene) -> Result<Value> {
    let mut digest = Sha256::new();
    let mut counts = std::collections::BTreeMap::<u64, [usize; 4]>::new();
    circles(scene, |layer, output, id, center, radius| {
        digest.update(layer.channel_id().0.to_le_bytes());
        digest.update(output.to_le_bytes());
        digest.update(id.as_bytes());
        for value in [center.x, center.y, radius] {
            digest.update(value.to_bits().to_le_bytes())
        }
        let c = counts.entry(layer.channel_id().0).or_default();
        c[0] += 1;
        c[1] += usize::from(radius == 0.);
        c[2] += usize::from(!layer.visible());
        c[3] += usize::from(
            center.x - radius < 0.
                || center.y - radius < 0.
                || center.x + radius > scene.canvas().width
                || center.y + radius > scene.canvas().height,
        );
        Ok(())
    })?;
    Ok(
        json!({"scene_identity":scene.identity().scene_fingerprint(),"family_identity":scene.identity().family_fingerprint(),"realization_identity":scene.identity().realization_fingerprint(),"model":format!("{:?}",scene.model()),"canvas":[scene.canvas().width,scene.canvas().height],"circle_order_id_coordinate_radius_bits_sha256":format!("{:x}",digest.finalize()),"channel_counts_total_zero_invisible_boundary_crossing":counts,"scope_provenance_fill_rule_authority":"retained exact scene identity covers scope/provenance/fill rules; separate audit covers ordered IDs and raw coordinate/radius bits"}),
    )
}

/// Constructs a direct native circle adapter with channel-owned model blending and linear payloads.
/// Eligible opaque runs use a positive compound path; other circles retain painter order.
/// Prefix admission only limits native submission, never changes the retained canonical input.
/// # Errors
/// Rejects unsupported model/geometry/paint and returns native scene creation errors.
fn encode_scene(
    scene: &RenderScene,
    route: &str,
    prefix: Option<usize>,
    width: u16,
    height: u16,
) -> Result<(CandidateScene, Value)> {
    encode_scene_scaled(scene, route, prefix, width, height, 1.)
}

/// Encodes the same canonical circles at a fixed view scale with at most1/64 output-pixel path error.
/// # Errors
/// Rejects unsupported geometry/model, nonfinite scale, and invalid native scene input.
pub(super) fn encode_scene_scaled(
    scene: &RenderScene,
    route: &str,
    prefix: Option<usize>,
    width: u16,
    height: u16,
    scale: f64,
) -> Result<(CandidateScene, Value)> {
    let refine = route == "hybrid-refined";
    if !scale.is_finite() || scale <= 0. {
        return Err("invalid scale".into());
    }
    if scene
        .layers()
        .iter()
        .any(|layer| layer.outputs().len() != 1)
    {
        return Err("initial performance bridge requires exactly one output per channel".into());
    }
    let mut native = match route {
        "hybrid" | "hybrid-refined" => {
            let mut s = vello_gpu::Scene::new(width, height);
            s.set_aliasing_threshold(None);
            s.set_transform(Affine::scale(scale));
            CandidateScene::Hybrid(Box::new(s))
        }
        "compute-area" => {
            CandidateScene::Compute(Box::new(vello::Scene::new()), vello::AaConfig::Area)
        }
        _ => return Err("performance route must be cpu, hybrid or compute-area".into()),
    };
    let model = scene
        .model()
        .ok_or("performance bridge requires explicit model")?;
    let mode = match model {
        HalftoneChannelModel::Rgb => BlendMode::new(Mix::Normal, Compose::PlusLighter),
        HalftoneChannelModel::Cmyk => BlendMode::new(Mix::Multiply, Compose::SrcOver),
        HalftoneChannelModel::SourceColorAlpha => {
            return Err("performance circle bridge rejects sampled source-color model".into());
        }
    };
    let mut grouped =
        std::collections::BTreeMap::<u64, Vec<(toniator_geometry::Point2, f64)>>::new();
    let mut submitted = 0usize;
    circles(scene, |layer, _, _, center, radius| {
        if layer.visible() && prefix.is_none_or(|limit| submitted < limit) {
            grouped
                .entry(layer.channel_id().0)
                .or_default()
                .push((center, radius));
            submitted += 1;
        }
        Ok(())
    })?;
    let projection = if refine {
        refined_projection(scene, scale, prefix)?
    } else {
        Value::Null
    };
    let mut elements = 0usize;
    for layer in scene.layers() {
        let Some(marks) = grouped.remove(&layer.channel_id().0) else {
            continue;
        };
        match &mut native {
            CandidateScene::Hybrid(s) => s.push_layer(None, Some(mode), Some(1.), None, None),
            CandidateScene::Compute(s, _) => s.push_layer(
                Fill::NonZero,
                mode,
                1.,
                Affine::IDENTITY,
                &Rect::new(0., 0., width.into(), height.into()),
            ),
        }
        let color = layer.color();
        let paint = Color::new([
            color.red as f32,
            color.green as f32,
            color.blue as f32,
            (color.alpha * layer.opacity()) as f32,
        ]);
        let eligible = color.alpha == 1. && layer.opacity() == 1.;
        let projected = projection["per_channel_path_elements"][layer.channel_id().0.to_string()]
            .as_u64()
            .unwrap_or(0);
        let mut compound = BezPath::with_capacity(usize::try_from(projected)?);
        for (center, radius) in marks {
            let path = native_circle_path(center, radius, scale, refine);
            elements += path.elements().len();
            if eligible {
                compound.extend(path.elements().iter().copied())
            } else {
                native_fill(&mut native, &path, paint, scale)
            }
        }
        if eligible {
            native_fill(&mut native, &compound, paint, scale)
        }
        match &mut native {
            CandidateScene::Hybrid(s) => s.pop_layer(),
            CandidateScene::Compute(s, _) => s.pop_layer(),
        }
    }
    Ok((
        native,
        json!({"submitted_circles":submitted,"path_elements":elements,"circle_to_native_bezier_tolerance_document_units":CIRCLE_TOLERANCE/scale,"fixed_zoom":scale,"numeric_linear_carrier":true,"channel_blend":format!("{mode:?}"),"prefix_admission_not_full_artwork":prefix.is_some(),"refined_line_adapter":refine,"flatten_target_output_pixels":if refine {Some(1./128.)}else{None},"projection":projection,"tolerance_note":"Bezier1/64outputpixel plus optional kurbo flatten Hausdorff target1/128; not a combined analytic guarantee"}),
    ))
}

/// Builds the existing native Bezier or ONE refined line-only consumer approximation.
/// Authored center/radius remain unchanged. Bezier1/64 and flatten1/128 output-pixel targets add;
/// kurbo's Hausdorff target is not a shared analytic-area certificate or combined1/128 guarantee.
pub(super) fn native_circle_path(
    center: toniator_geometry::Point2,
    radius: f64,
    scale: f64,
    refine: bool,
) -> BezPath {
    let bez = Circle::new((center.x, center.y), radius).to_path(CIRCLE_TOLERANCE / scale);
    if !refine {
        return bez;
    }
    let mut lines = BezPath::new();
    vello_common::kurbo::flatten(
        bez.elements().iter().copied(),
        1. / (128. * scale),
        |element| lines.push(element),
    );
    lines
}

/// Counts the exact refined consumer path before allocating native compound storage or GPU work.
/// One per-circle path is temporary; exact per-channel preallocation avoids compound growth spikes.
/// # Errors
/// Rejects unsupported geometry and checked element/byte-count overflow without changing the scene.
pub(super) fn refined_projection(
    scene: &RenderScene,
    scale: f64,
    prefix: Option<usize>,
) -> Result<Value> {
    let mut counts = std::collections::BTreeMap::<u64, usize>::new();
    let mut total = 0usize;
    let mut submitted = 0usize;
    let mut bezier_area = 0.;
    let mut flat_area = 0.;
    circles(scene, |layer, _, _, center, radius| {
        if !layer.visible() || prefix.is_some_and(|limit| submitted >= limit) {
            return Ok(());
        }
        submitted += 1;
        let bez = Circle::new((center.x, center.y), radius).to_path(CIRCLE_TOLERANCE / scale);
        let lines = native_circle_path(center, radius, scale, true);
        bezier_area += bez.area().abs() * scale * scale;
        flat_area += lines.area().abs() * scale * scale;
        let n = lines.elements().len();
        total = total.checked_add(n).ok_or("path count overflow")?;
        let entry = counts.entry(layer.channel_id().0).or_default();
        *entry = entry.checked_add(n).ok_or("channel path count overflow")?;
        Ok(())
    })?;
    let largest = counts.values().copied().max().unwrap_or(0);
    let element_bytes = std::mem::size_of::<PathEl>();
    Ok(
        json!({"per_channel_path_elements":counts,"total_path_elements":total,"path_element_bytes":element_bytes,"largest_preallocated_compound_bytes":largest.checked_mul(element_bytes).ok_or("path bytes overflow")?,"submitted_circles":submitted,"sum_native_bezier_areas_before_union_output_pixels":bezier_area,"sum_flattened_areas_before_union_output_pixels":flat_area,"preallocation_note":"one channel compound at a time; count pass included in preparation; no global union or authoredgeometrychange"}),
    )
}

/// Applies one native filled path using the existing backend, without geometric compensation.
fn native_fill(scene: &mut CandidateScene, path: &BezPath, paint: Color, scale: f64) {
    match scene {
        CandidateScene::Hybrid(s) => {
            s.set_fill_rule(Fill::NonZero);
            s.set_paint(paint);
            s.fill_path(path)
        }
        CandidateScene::Compute(s, _) => {
            s.fill(Fill::NonZero, Affine::scale(scale), paint, None, path)
        }
    }
}

/// Removes mandatory256-byte GPU row padding while retaining every native RGBA byte.
/// # Errors
/// Rejects truncated buffers, short strides, overflow, or dimensions inconsistent with the copy.
fn strip_padding(mapped: &[u8], width: u16, height: u16, stride: u32) -> Result<Vec<u8>> {
    let row = usize::from(width).checked_mul(4).ok_or("row overflow")?;
    let stride = usize::try_from(stride)?;
    let expected = stride
        .checked_mul(usize::from(height))
        .ok_or("buffer overflow")?;
    if stride < row || mapped.len() != expected {
        return Err("native padded-row buffer dimensions mismatch".into());
    }
    let mut bytes = Vec::with_capacity(row * usize::from(height));
    for source in mapped.chunks_exact(stride) {
        bytes.extend_from_slice(&source[..row]);
    }
    Ok(bytes)
}

enum Backend {
    Hybrid {
        renderer: Box<vello_gpu::Renderer>,
        resources: Box<vello_gpu::Resources>,
        depth: wgpu::TextureView,
    },
    Compute(Box<vello::Renderer>),
}
pub(super) struct Resident {
    gpu: Gpu,
    scene: CandidateScene,
    backend: Backend,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    buffer: wgpu::Buffer,
    width: u16,
    height: u16,
    stride: u32,
}
impl Resident {
    /// Initializes persistent device, renderer/pipelines, target and readback for genuine warm reraster.
    /// # Errors
    /// Propagates adapter/pipeline errors; native dimensions are bounded by checked u16 inputs.
    pub(super) fn new(gpu: Gpu, scene: CandidateScene, width: u16, height: u16) -> Result<Self> {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("diagnostic-native-artwork"),
            size: wgpu::Extent3d {
                width: width.into(),
                height: height.into(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let stride = (u32::from(width) * 4).next_multiple_of(256);
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("padded-native-readback"),
            size: u64::from(stride) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let backend = match &scene {
            CandidateScene::Hybrid(_) => {
                let (renderer, resources) = vello_gpu::Renderer::new(
                    &gpu.device,
                    &gpu.queue,
                    &vello_gpu::RenderTargetConfig {
                        format,
                        width,
                        height,
                    },
                );
                let depth =
                    renderer.create_depth_texture_view(&vello_gpu::RenderSize { width, height });
                Backend::Hybrid {
                    renderer: Box::new(renderer),
                    resources: Box::new(resources),
                    depth,
                }
            }
            CandidateScene::Compute(_, aa) => Backend::Compute(Box::new(vello::Renderer::new(
                &gpu.device,
                vello::RendererOptions {
                    use_cpu: false,
                    antialiasing_support: [*aa].into_iter().collect(),
                    num_init_threads: std::num::NonZeroUsize::new(1),
                    ..Default::default()
                },
            )?)),
        };
        Ok(Self {
            gpu,
            scene,
            backend,
            texture,
            view,
            buffer,
            width,
            height,
            stride,
        })
    }
    /// Replaces only the region target/readback/depth while preserving device, renderer and encoded scene.
    /// Allocation is charged to each region render; no pixel cache or prior target is reused here.
    /// # Errors
    /// Rejects empty target dimensions and compute use; native allocation errors surface through wgpu.
    pub(super) fn resize_region(&mut self, width: u16, height: u16) -> Result<()> {
        if width == 0 || height == 0 {
            return Err("empty region".into());
        }
        let Backend::Hybrid {
            renderer, depth, ..
        } = &mut self.backend
        else {
            return Err("pan requires hybrid".into());
        };
        self.texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("pan-region-target"),
            size: wgpu::Extent3d {
                width: width.into(),
                height: height.into(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.view = self.texture.create_view(&Default::default());
        self.stride = (u32::from(width) * 4).next_multiple_of(256);
        self.buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pan-readback"),
            size: u64::from(self.stride) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        *depth = renderer.create_depth_texture_view(&vello_gpu::RenderSize { width, height });
        self.width = width;
        self.height = height;
        Ok(())
    }
    /// Checks upstream compute allocation counters without changing capacities or enabling overlays.
    /// This separate validation pass is excluded from standard performance runs and downloads lines.
    /// # Errors
    /// Rejects missing/failed/overflowing counters; upstream API errors propagate without retry.
    #[allow(
        deprecated,
        reason = "The pinned upstream experimental API is the only public allocation-counter seam"
    )]
    fn validate_compute_capacity(&mut self) -> Result<Value> {
        let (Backend::Compute(renderer), CandidateScene::Compute(scene, aa)) =
            (&mut self.backend, &self.scene)
        else {
            return Err("compute validation requires compute backend".into());
        };
        use vello::low_level::vello_encoding::{RenderConfig, resolve_solid_paths_only};
        let mut packed = Vec::new();
        let layout = resolve_solid_paths_only(scene.encoding(), &mut packed);
        let config = RenderConfig::new(
            &layout,
            self.width.into(),
            self.height.into(),
            &Color::TRANSPARENT,
        );
        drop(packed);
        let params = vello::RenderParams {
            base_color: Color::TRANSPARENT,
            width: self.width.into(),
            height: self.height.into(),
            antialiasing_method: *aa,
        };
        let bump = vello::util::block_on_wgpu(
            &self.gpu.device,
            renderer.render_to_texture_async(
                &self.gpu.device,
                &self.gpu.queue,
                scene,
                &self.view,
                &params,
                vello::low_level::DebugLayers::none(),
            ),
        )?
        .ok_or("upstream allocation counters unavailable")?;
        let capacity = &config.buffer_sizes;
        let checks = [
            ("lines", bump.lines, capacity.lines.len()),
            (
                "binning",
                bump.binning,
                capacity
                    .bin_data
                    .len()
                    .checked_sub(layout.bin_data_start)
                    .ok_or("binning static prefix exceeds capacity")?,
            ),
            ("tile", bump.tile, capacity.tiles.len()),
            ("seg_counts", bump.seg_counts, capacity.seg_counts.len()),
            ("segments", bump.segments, capacity.segments.len()),
            ("blend", bump.blend, capacity.blend_spill.len()),
            ("ptcl", bump.ptcl, capacity.ptcl.len()),
        ];
        let record = json!({"failed":bump.failed,"counts_and_capacities":checks,"all_within_capacity":checks.iter().all(|(_,count,cap)|count<=cap),"debug_layers":"none (no overlays)","feature":"debug_layers enables robust counter download; no resizing/retry; dedicated validation process only"});
        if bump.failed != 0 || checks.iter().any(|(_, count, cap)| count > cap) {
            return Err(format!("compute capacity validation failed: {record}").into());
        }
        Ok(record)
    }

    /// Renders the retained encoded scene with persistent resources, synchronizes and strips row padding.
    /// Native data stays unchanged; numerical-linear encoding occurs only in a later output step.
    /// # Errors
    /// Returns native-render, synchronization and mapping errors without partial output publication.
    pub(super) fn render(&mut self) -> Result<Vec<u8>> {
        self.render_offset((0, 0))
    }

    /// Renders a retained hybrid scene at a native integer camera offset; compute offset is rejected.
    /// All scene scheduling/uploads, synchronization and readback remain inside this call.
    /// # Errors
    /// Propagates rendering/mapping errors and rejects unsupported compute camera translations.
    pub(super) fn render_offset(&mut self, offset: (i32, i32)) -> Result<Vec<u8>> {
        if matches!(self.scene, CandidateScene::Compute(..)) && offset != (0, 0) {
            return Err("compute native offset unavailable".into());
        }
        let mut options = vello_gpu::RenderOptions::default();
        options.offset = offset;
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        match (&mut self.backend, &self.scene) {
            (
                Backend::Hybrid {
                    renderer,
                    resources,
                    depth,
                },
                CandidateScene::Hybrid(scene),
            ) => renderer.render(
                scene,
                resources,
                &mut encoder,
                vello_gpu::RenderTarget {
                    view: &self.view,
                    depth_view: Some(depth),
                },
                &vello_gpu::TextureBindings::new(),
                options,
            )?,
            (Backend::Compute(renderer), CandidateScene::Compute(scene, aa)) => renderer
                .render_to_texture(
                    &self.gpu.device,
                    &self.gpu.queue,
                    scene,
                    &self.view,
                    &vello::RenderParams {
                        base_color: Color::TRANSPARENT,
                        width: self.width.into(),
                        height: self.height.into(),
                        antialiasing_method: *aa,
                    },
                )?,
            _ => return Err("resident backend/scene mismatch".into()),
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.stride),
                    rows_per_image: None,
                },
            },
            self.texture.size(),
        );
        self.gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
        self.gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;
        let mapped = self.buffer.slice(..).get_mapped_range()?;
        let bytes = strip_padding(&mapped, self.width, self.height, self.stride)?;
        drop(mapped);
        self.buffer.unmap();
        Ok(bytes)
    }
    /// Downloads the current completed target without submitting any additional drawing.
    /// This validation-only readback preserves the async counter API image before standard rerender.
    /// # Errors
    /// Propagates mapping/polling/checked-row errors without modifying the target texture.
    fn copy_target_without_render(&self) -> Result<Vec<u8>> {
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.stride),
                    rows_per_image: None,
                },
            },
            self.texture.size(),
        );
        self.gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
        self.gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;
        let mapped = self.buffer.slice(..).get_mapped_range()?;
        let bytes = strip_padding(&mapped, self.width, self.height, self.stride)?;
        drop(mapped);
        self.buffer.unmap();
        Ok(bytes)
    }

    /// Converts native alpha representation and final numerical-linear carrier to straight sRGBA8.
    pub(super) fn encode(&self, bytes: Vec<u8>) -> Vec<u8> {
        let mut straight = match self.scene {
            CandidateScene::Hybrid(_) => {
                Pixmap::from_parts(bytes, self.width, self.height, PixelMetadata::default())
                    .take_rgba8(ImageAlphaType::Alpha)
            }
            CandidateScene::Compute(_, _) => bytes,
        };
        for pixel in straight.chunks_exact_mut(4) {
            for v in &mut pixel[..3] {
                *v = (toniator_render::linear_to_srgb(f64::from(*v) / 255.) * 255.)
                    .round()
                    .clamp(0., 255.) as u8;
            }
        }
        straight
    }
}

/// Writes untouched straight RGBA and native PNG at actual artwork dimensions.
/// # Errors
/// Propagates checked raster creation, encoding and output errors.
pub(super) fn save(root: &Path, name: &str, width: u16, height: u16, bytes: Vec<u8>) -> Result<()> {
    fs::write(root.join(format!("{name}.rgba")), &bytes)?;
    let surface = toniator_render::RasterSurface::new(width.into(), height.into(), bytes)?;
    fs::write(
        root.join(format!("{name}.png")),
        toniator_render::encode_png(&surface)?,
    )?;
    Ok(())
}

/// Measures the public production prepared-root reuse path separately from fresh canonical reraster.
/// One root/certificate/snapshot is retained; repeated paint performs no Boolean preparation.
/// # Errors
/// Returns existing root/certificate/paint/encoding diagnostics without weakening authority.
fn prepared_cpu_control(
    scene: Arc<RenderScene>,
    root_path: &Path,
    width: u16,
    height: u16,
    cold: &[u8],
) -> Result<Value> {
    use toniator_render::preview_tiles::{PreviewRoot, Region};
    let t = Instant::now();
    let mut root = PreviewRoot::new(
        scene,
        1.,
        RasterBackground::Transparent,
        RasterizationLimits::new(
            toniator_engine::EvaluationLimits::default().max_flattened_raster_edges(),
        )?,
        &|| false,
    )?;
    let preparation = t.elapsed().as_secs_f64();
    let frame = root.frame();
    let region = Region {
        x: 0,
        y: 0,
        width: frame.width,
        height: frame.height,
    };
    let t = Instant::now();
    let certificate = root.certify(region, &|| false)?;
    let certification = t.elapsed().as_secs_f64();
    let snapshot = root.snapshot(&certificate)?;
    let t = Instant::now();
    let first = snapshot.paint(region, &certificate, &|| false)?;
    let first_seconds = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let next = snapshot.paint(region, &certificate, &|| false)?;
    let repeated = t.elapsed().as_secs_f64();
    let surface = next.surface(&root, &certificate)?;
    let equal_native = surface.pixels() == cold;
    let equal_first = surface.pixels() == first.surface(&root, &certificate)?.pixels();
    let t = Instant::now();
    save(
        root_path,
        "prepared-resident",
        width,
        height,
        surface.pixels().to_vec(),
    )?;
    let encoding = t.elapsed().as_secs_f64();
    Ok(
        json!({"preparation_seconds":preparation,"whole_frame_certification_seconds":certification,"first_paint_seconds":first_seconds,"repeated_prepared_paint_seconds":repeated,"repeated_encode_write_seconds":encoding,"repeated_equals_first_bytes":equal_first,"prepared_equals_native_bytes":equal_native,"root_facts":format!("{:?}",root.facts()?),"layer_bounds":certificate.layer_bounds(),"scale":1.,"background":"Transparent","authority":"one retained root/certificate/snapshot; existing prepared geometry and request authority; no Boolean budget reset"}),
    )
}

/// Executes one fresh matched CPU or incompatible-candidate native workflow and one warm control.
/// # Errors
/// Rejects existing output, malformed arguments, unsupported geometry, or any load/evaluation/render error.
/// This probe never alters the genuine project or lowers density, iterations or numerical authorities.
pub(super) fn run(root: &Path, route: &str, args: Vec<String>) -> Result<()> {
    if std::env::var("RAYON_NUM_THREADS").ok().as_deref() != Some("4") {
        return Err("matched performance requires RAYON_NUM_THREADS=4".into());
    }
    let project = args
        .first()
        .ok_or("perf requires genuine calibrated project path and optional prefix")?;
    let prefix = args
        .get(1)
        .filter(|s| s.as_str() != "all")
        .map(|s| s.parse::<usize>())
        .transpose()?;
    let validation_only = args.get(2).is_some_and(|s| s == "validate");
    let cold_only = args.get(2).is_some_and(|s| s == "cold-only");
    if root.exists() {
        return Err("fresh performance evidence directory required".into());
    }
    fs::create_dir_all(root)?;
    fs::write(root.join("phase.txt"), "cold")?;
    let started = Instant::now();
    let loaded = toniator_io::load(Path::new(project))?;
    loaded.document().require_saved_calibration_authority()?;
    let session = toniator_domain::DocumentSession::new(loaded.document().clone())?;
    let mut media = toniator_engine::open_source_media(
        loaded.sources(),
        toniator_engine::MediaTools::default(),
        &|| false,
    )?;
    let frame = session.document().project_timing().frame_range().start();
    let request =
        toniator_engine::frame_evaluation_request(&session, &mut media, frame, &|| false)?;
    let target = if route == "cpu" {
        request
            .clone()
            .for_output(RasterBackground::Transparent, None, RasterAntialiasing::On)
    } else {
        request.clone().for_scene()?
    };
    let load_seconds = started.elapsed().as_secs_f64();
    let t = Instant::now();
    let mut cache = toniator_engine::EvaluationProfileCache::default();
    let result = toniator_engine::evaluate_profiled_cached_with_limits(
        target.clone(),
        toniator_engine::EvaluationLimits::default(),
        &mut cache,
    )?;
    let evaluation_seconds = t.elapsed().as_secs_f64();
    let scene: Arc<RenderScene> = result.result.shared_scene();
    let width = u16::try_from(scene.canvas().width.round() as u32)?;
    let height = u16::try_from(scene.canvas().height.round() as u32)?;
    let t = Instant::now();
    let identity = audit(&scene)?;
    let audit_seconds = t.elapsed().as_secs_f64();
    let mut report = json!({"route":route,"project":project,"input_project_sha256":format!("{:x}",Sha256::digest(fs::read(project)?)),"prefix_admission":prefix,"native_dimensions":[width,height],"canonical":identity,"load_source_request_seconds":load_seconds,"native_evaluation_or_for_scene_seconds":evaluation_seconds,"canonical_audit_seconds":audit_seconds,"engine_stages":format!("{:#?}",result.performance),"fixed_rayon_workers":std::env::var("RAYON_NUM_THREADS").ok(),"for_scene_disposable_overhead":"GPU common evaluation includes1x1AAOff derived raster; CPU native evaluator includes fullAA raster","contract":"Incompatible diagnostic only; no performance result establishes quality acceptance or production integration"});
    if route == "cpu" {
        let t = Instant::now();
        save(
            root,
            "cold",
            width,
            height,
            result.result.raster().pixels().to_vec(),
        )?;
        report["cold_encode_write_seconds"] = json!(t.elapsed().as_secs_f64());
        report["cold_complete_workflow_seconds"] = json!(started.elapsed().as_secs_f64());
        fs::write(
            root.join("results-cold.json"),
            serde_json::to_vec_pretty(&report)?,
        )?;
        if cold_only {
            report["cold_only_no_warm_controls"] = json!(true);
            report["canonical_after"] = audit(&scene)?;
            if report["canonical"] != report["canonical_after"] {
                return Err("canonical mutation".into());
            }
            fs::write(
                root.join("results.json"),
                serde_json::to_vec_pretty(&report)?,
            )?;
            println!("{}", report);
            return Ok(());
        }

        fs::write(root.join("phase.txt"), "completed-result-cache-hit")?;
        let t = Instant::now();
        let hit = toniator_engine::evaluate_profiled_cached_with_limits(
            target,
            toniator_engine::EvaluationLimits::default(),
            &mut cache,
        )?;
        report["completed_native_result_cache_hit_seconds"] = json!(t.elapsed().as_secs_f64());
        report["completed_cache_dispositions"] = json!(format!("{:?}", hit.diagnostics));
        fs::write(root.join("phase.txt"), "canonical-reraster")?;
        let t = Instant::now();
        let raster = toniator_render::rasterize_output_cancellable(
            &scene,
            RasterBackground::Transparent,
            None,
            RasterAntialiasing::On,
            RasterizationLimits::new(
                toniator_engine::EvaluationLimits::default().max_flattened_raster_edges(),
            )?,
            &|| false,
        )?;
        report["retained_canonical_cpu_raster_seconds"] = json!(t.elapsed().as_secs_f64());
        let t = Instant::now();
        save(root, "retained", width, height, raster.pixels().to_vec())?;
        report["retained_encode_write_seconds"] = json!(t.elapsed().as_secs_f64());
        fs::write(root.join("phase.txt"), "prepared-cpu-root")?;
        eprintln!("phase=prepared-cpu-root-control");
        match prepared_cpu_control(
            scene.clone(),
            root,
            width,
            height,
            result.result.raster().pixels(),
        ) {
            Ok(value) => report["prepared_cpu_resident_control"] = value,
            Err(error) => report["prepared_cpu_resident_error"] = json!(error.to_string()),
        };
        report["retained_native_bytes_equal_cold"] =
            json!(raster.pixels() == result.result.raster().pixels());
    } else {
        let t = Instant::now();
        let (encoded, bridge) = encode_scene(&scene, route, prefix, width, height)?;
        report["canonical_native_bridge_seconds"] = json!(t.elapsed().as_secs_f64());
        report["bridge"] = bridge;
        let t = Instant::now();
        let gpu = pollster::block_on(Gpu::new())?;
        fs::write(
            root.join("adapter.json"),
            serde_json::to_vec_pretty(&gpu.info)?,
        )?;
        report["device_init_seconds"] = json!(t.elapsed().as_secs_f64());
        let t = Instant::now();
        let mut resident = Resident::new(gpu, encoded, width, height)?;
        let mut validation_image = None;
        if validation_only {
            fs::write(root.join("phase.txt"), "compute-capacity-validation")?;
            let t = Instant::now();
            let check = resident.validate_compute_capacity()?;
            let raw = resident.copy_target_without_render()?;
            fs::write(root.join("validation-async.gpu-native-linear.rgba"), &raw)?;
            let image = resident.encode(raw);
            save(root, "validation-async", width, height, image.clone())?;
            validation_image = Some(image);
            report["compute_buffer_validation"] = check;
            report["capacity_validation_seconds_excluded_from_benchmark"] =
                json!(t.elapsed().as_secs_f64());
            report["validation_only_not_performance"] = json!(true);
            fs::write(
                root.join("compute-capacity-validation.json"),
                serde_json::to_vec_pretty(&report["compute_buffer_validation"])?,
            )?;
        }
        report["renderer_pipeline_target_init_seconds"] = json!(t.elapsed().as_secs_f64());
        let t = Instant::now();
        let raw = resident.render()?;
        report["cold_gpu_render_upload_sync_readback_seconds"] = json!(t.elapsed().as_secs_f64());
        let t = Instant::now();
        fs::write(root.join("cold.gpu-native-linear.rgba"), &raw)?;
        let bytes = resident.encode(raw);
        if let Some(reference) = validation_image {
            let alpha_differences = reference
                .iter()
                .skip(3)
                .step_by(4)
                .zip(bytes.iter().skip(3).step_by(4))
                .filter(|(a, b)| a != b)
                .count();
            let visible_rgb_differences = reference
                .chunks_exact(4)
                .zip(bytes.chunks_exact(4))
                .filter(|(a, b)| (a[3] > 0 || b[3] > 0) && a[..3] != b[..3])
                .count();
            let hidden_rgb_differences = reference
                .chunks_exact(4)
                .zip(bytes.chunks_exact(4))
                .filter(|(a, b)| a[3] == 0 && b[3] == 0 && a[..3] != b[..3])
                .count();
            report["async_counter_api_vs_standard_api_image"] = json!({"byte_equal":reference==bytes,"different_alpha_pixels":alpha_differences,"different_visible_rgb_pixels":visible_rgb_differences,"different_hidden_rgb_pixels":hidden_rgb_differences,"capture":"async completed target downloaded BEFORE standard render, no debug overlays"});
        }
        save(root, "cold", width, height, bytes.clone())?;
        report["cold_alpha_transfer_encode_write_seconds"] = json!(t.elapsed().as_secs_f64());
        report["cold_complete_workflow_seconds"] = json!(started.elapsed().as_secs_f64());
        fs::write(
            root.join("results-cold.json"),
            serde_json::to_vec_pretty(&report)?,
        )?;
        if cold_only {
            report["cold_only_no_warm_controls"] = json!(true);
            report["canonical_after"] = audit(&scene)?;
            if report["canonical"] != report["canonical_after"] {
                return Err("canonical mutation".into());
            }
            fs::write(
                root.join("results.json"),
                serde_json::to_vec_pretty(&report)?,
            )?;
            println!("{}", report);
            return Ok(());
        }

        fs::write(root.join("phase.txt"), "resident-gpu-reraster")?;
        let t = Instant::now();
        let raw = resident.render()?;
        report["resident_same_encoded_scene_gpu_reraster_sync_readback_seconds"] =
            json!(t.elapsed().as_secs_f64());
        let t = Instant::now();
        let warm = resident.encode(raw);
        report["resident_warm_bytes_equal_cold"] = json!(bytes == warm);
        save(root, "resident", width, height, warm)?;
        report["resident_alpha_transfer_encode_write_seconds"] = json!(t.elapsed().as_secs_f64());
    }
    report["process_body_with_warm_controls_seconds"] = json!(started.elapsed().as_secs_f64());
    report["canonical_after"] = audit(&scene)?;
    if report["canonical"] != report["canonical_after"] {
        return Err("canonical geometry mutated".into());
    }
    fs::write(
        root.join("results.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", report);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Verifies refined circle consumption bypasses native cubic flattening without collapsing area.
    /// # Panics
    /// Panics if a curve remains or the half-pixel circle loses more than the small target approximation.
    #[test]
    fn refined_circle_is_line_only_and_retains_area() {
        let path = native_circle_path(toniator_geometry::Point2::new(8., 8.), 0.5, 1., true);
        assert!(
            path.elements()
                .iter()
                .all(|e| matches!(e, PathEl::MoveTo(_) | PathEl::LineTo(_) | PathEl::ClosePath))
        );
        assert!(path.area() > 0.76 && path.area() < 0.79);
    }
    /// Proves a900px native row is copied across its256-byte padding boundary without pixel loss.
    /// # Panics
    /// Panics if the padding sentinel leaks into data, the last pixel disappears, or truncation passes.
    #[test]
    fn native_900_pixel_row_padding_preserves_pixels() {
        let row = 900 * 4;
        let stride = (row as u32).next_multiple_of(256) as usize;
        let source = (0..row * 2).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        let mut padded = vec![255; stride * 2];
        padded[..row].copy_from_slice(&source[..row]);
        padded[stride..stride + row].copy_from_slice(&source[row..]);
        assert_eq!(
            strip_padding(&padded, 900, 2, stride as u32).unwrap(),
            source
        );
        assert!(strip_padding(&padded[..padded.len() - 1], 900, 2, stride as u32).is_err());
    }
    /// Proves the circle-only adapter fails explicitly for unsupported geometry instead of dropping it.
    /// # Panics
    /// Panics if the existing canonical closed-path witness is silently admitted as a circle workload.
    #[test]
    fn bridge_rejects_unsupported_geometry() {
        let witness = witnesses().remove(0);
        let scene = cpu_scene(&witness).unwrap();
        assert!(encode_scene(&scene, "hybrid", None, 64, 64).is_err());
    }
}
