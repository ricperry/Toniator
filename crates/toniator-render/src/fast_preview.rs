//! Optional, explicitly approximate, circle-only final consumer of immutable canonical scenes.
//!
//! Native objects remain on one worker. Completeness records describe whole-scene execution;
//! they grant no Accurate area/error certificate or document/presentation publication authority.

use super::{GeometryOutput, PreviewRasterTarget, RasterBackground, RasterSurface, RenderScene};
use crate::preview_tiles::{Frame, Region};
use std::{
    fmt,
    future::Future,
    mem::size_of,
    pin::pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    task::{Context, Poll, Wake, Waker},
    thread,
    time::{Duration, Instant},
};
use toniator_domain::HalftoneChannelModel;
use toniator_geometry::{CanonicalMark, Point2};
use vello_common::{
    color::{AlphaColor, Srgb},
    kurbo::{Affine, BezPath, Circle, PathEl, Rect, Shape},
    multi_atlas::AtlasConfig,
    peniko::{BlendMode, Compose, Fill, Mix},
};

/// Identifies the pinned refined contour and fixed model-composition algorithm.
pub const ALGORITHM_VERSION: &str = "vello-hybrid-72c2fb0-refined-circle-linear-v1";
const MIB: u64 = 1024 * 1024;
const MAX_DIMENSION: u32 = 4096;
const MAX_CIRCLE_RADIUS_PIXELS: f64 = 8192.0;
const WORK_TIMEOUT: Duration = Duration::from_secs(15);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Typed atomic-refusal reasons for the caller's Accurate CPU fallback policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FastPreviewError {
    UnsupportedScene(&'static str),
    Unavailable(String),
    Limits(&'static str),
    Allocation,
    Device(String),
    Render(String),
    Readback(String),
    Timeout,
    Cancelled,
}
impl fmt::Display for FastPreviewError {
    /// Presents a concise failure reason without suggesting a partial image is usable.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FastPreviewError {}

/// Optional-consumer payload reservations, independent of authored creative-work limits.
#[derive(Clone, Copy, Debug)]
pub struct FastPreviewLimits {
    /// Remaining allowance in the SINGLE combined CPU/Fast derived-resource budget.
    pub available_payload_bytes: u64,
    /// Additional GPU reservation cap; this is not an available-VRAM query or an RSS bound.
    pub gpu_reservation_bytes: u64,
}
impl Default for FastPreviewLimits {
    /// Uses the agreed 8 GiB combined soft allowance and 2 GiB additional GPU pilot cap.
    fn default() -> Self {
        Self {
            available_payload_bytes: 8 * 1024 * MIB,
            gpu_reservation_bytes: 2 * 1024 * MIB,
        }
    }
}

/// Conservative pinned-subset reservations, including temporary preparation and readback ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastPreviewReservation {
    pub host_bytes: u64,
    pub gpu_bytes: u64,
    pub temporary_path_bytes: u64,
    pub path_elements: u64,
    pub alpha_bytes_upper_bound: u64,
    pub strip_count_upper_bound: u64,
    pub gpu_strip_bytes_upper_bound: u64,
    pub upload_bytes_upper_bound: u64,
    pub upload_other_bytes_upper_bound: u64,
    pub host_metadata_bytes_upper_bound: u64,
}
impl FastPreviewReservation {
    /// Returns combined reserved payload; admission already checks addition overflow.
    pub fn total_bytes(self) -> u64 {
        self.host_bytes + self.gpu_bytes
    }
}

/// Whole-scene admission and exact-scale projection, created before native encoding/device setup.
pub struct FastPreviewPlan {
    scene: Arc<RenderScene>,
    frame: Frame,
    offset: (f64, f64),
    per_channel_elements: Vec<usize>,
    reservation: FastPreviewReservation,
    limits: FastPreviewLimits,
}
impl FastPreviewPlan {
    /// Admits a natural/exact-zoom root; callers reuse it for all complete in-bounds region paints.
    /// # Errors
    /// Refuses unsupported complete scenes, invalid/bounded targets, cancellation or resource excess.
    pub fn new(
        scene: Arc<RenderScene>,
        scale: f64,
        background: RasterBackground,
        limits: FastPreviewLimits,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Self, FastPreviewError> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err(FastPreviewError::Limits("invalid exact zoom"));
        }
        let width = checked_virtual_dimension(scene.canvas().width * scale)?;
        let height = checked_virtual_dimension(scene.canvas().height * scale)?;
        Self::admit(
            scene,
            Frame {
                width,
                height,
                scale,
                background,
            },
            (0.0, 0.0),
            limits,
            cancelled,
        )
    }

    /// Admits the existing renderer's fitted/cropped target transform, including letterbox clipping.
    /// No second Fit formula is introduced; canonical geometry and authored canvas remain unchanged.
    /// # Errors
    /// Refuses unsupported scenes, invalid transforms, cancellation or resource excess atomically.
    pub fn for_target(
        scene: Arc<RenderScene>,
        target: PreviewRasterTarget,
        limits: FastPreviewLimits,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Self, FastPreviewError> {
        checked_dimension(f64::from(target.width()))?;
        checked_dimension(f64::from(target.height()))?;
        let transform = super::PreviewTransform::for_scene(&scene, target);
        if [
            transform.scale,
            transform.offset_x,
            transform.offset_y,
            transform.right,
            transform.bottom,
        ]
        .iter()
        .any(|value| !value.is_finite() || !(*value as f32).is_finite())
        {
            return Err(FastPreviewError::Limits(
                "resolved canvas clip is not natively representable",
            ));
        }
        let frame = Frame {
            width: target.width(),
            height: target.height(),
            scale: transform.scale,
            background: target.background(),
        };
        Self::admit(
            scene,
            frame,
            (transform.offset_x, transform.offset_y),
            limits,
            cancelled,
        )
    }

    /// Scans all canonical categories and projects exact refined counts before native work.
    /// Per-circle radius admission bounds one noninterruptible kurbo flatten call; cancellation is
    /// checked between circles and the count pass stops before allocating a compound path.
    /// # Errors
    /// Returns typed whole-scene, resource, cancellation and fallible allocation refusals.
    fn admit(
        scene: Arc<RenderScene>,
        frame: Frame,
        offset: (f64, f64),
        limits: FastPreviewLimits,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Self, FastPreviewError> {
        check_cancel(cancelled)?;
        if limits.gpu_reservation_bytes > 2 * 1024 * MIB
            || limits.available_payload_bytes > 8 * 1024 * MIB
        {
            return Err(FastPreviewError::Limits(
                "Fast pilot resource policy cannot be enlarged",
            ));
        }
        validate_scene(&scene)?;
        if [
            scene.canvas().width,
            scene.canvas().height,
            offset.0 + scene.canvas().width * frame.scale,
            offset.1 + scene.canvas().height * frame.scale,
        ]
        .iter()
        .any(|value| !value.is_finite() || !(*value as f32).is_finite())
        {
            return Err(FastPreviewError::Limits(
                "native canvas clip is not representable",
            ));
        }
        checked_virtual_dimension(f64::from(frame.width))?;
        checked_virtual_dimension(f64::from(frame.height))?;
        // Pinned hybrid allocates the complete recorder.scene_size when the root is a model
        // blend target, before viewport scheduling (render/common/mod.rs:210–218). A small
        // region cannot evade this approved intermediate-texture cap.
        if frame.width > MAX_DIMENSION || frame.height > MAX_DIMENSION {
            return Err(FastPreviewError::Limits(
                "pinned model blending requires the full virtual root to fit 4096 intermediate textures",
            ));
        }

        if !frame.scale.is_finite()
            || frame.scale <= 0.0
            || !(frame.scale as f32).is_finite()
            || frame.scale as f32 == 0.0
            || !(offset.0 as f32).is_finite()
            || !(offset.1 as f32).is_finite()
            || !offset.0.is_finite()
            || !offset.1.is_finite()
        {
            return Err(FastPreviewError::Limits("unrepresentable transform"));
        }
        let mut counts = Vec::new();
        counts
            .try_reserve_exact(scene.layers().len())
            .map_err(|_| FastPreviewError::Allocation)?;
        let mut total = 0_u64;
        let mut maximum_lines = 0_u64;
        let mut maximum_tiles = 0_u64;
        let mut alpha = 0_u64;
        let mut strips = 0_u64;
        let mut draws = 0_u64;
        for layer in scene.layers() {
            let mut elements = 0_u64;
            let mut tiles = 0_u64;
            let opaque = layer.color().alpha == 1.0 && layer.opacity() == 1.0;
            visit_circles(layer, |center, radius| {
                check_cancel(cancelled)?;
                let projected_radius = radius * frame.scale;
                if [center.x, center.y, radius].iter().any(|value| {
                    !(*value as f32).is_finite()
                        || !(value * frame.scale).is_finite()
                        || (value * frame.scale).abs() > f64::from(f32::MAX) / 16.0
                }) {
                    return Err(FastPreviewError::Limits(
                        "native circle coordinates exceed finite f32 representation",
                    ));
                }
                if !projected_radius.is_finite() || projected_radius > MAX_CIRCLE_RADIUS_PIXELS {
                    return Err(FastPreviewError::Limits(
                        "circle contour work exceeds pilot bound",
                    ));
                }
                if !layer.visible() {
                    return Ok(());
                }
                let (n, visits) = projected_path(center, radius, frame.scale, offset, None)?;
                elements = add(elements, n)?;
                if elements > u64::from(vello_common::tile::MAX_LINES_PER_PATH) {
                    return Err(FastPreviewError::Limits(
                        "native per-path line index capacity",
                    ));
                }
                tiles = add(tiles, visits)?;
                // Translucent primitives remain separate ordered draws; each receives its own
                // conservative bbox tile/alpha allowance, including off-canvas/zero guards.
                if !opaque {
                    let span_x =
                        (2.0 * projected_radius).ceil().min(f64::from(frame.width)) as u64 + 12;
                    let span_y =
                        (2.0 * projected_radius).ceil().min(f64::from(frame.height)) as u64 + 12;
                    let cells = mul(span_x.div_ceil(4), span_y.div_ceil(4))?;
                    alpha = add(alpha, mul(cells, 16)?)?;
                    strips = add(strips, add(mul(cells, 2)?, 2)?)?;
                    draws = add(draws, 1)?;
                }
                // These are resource limits on the optional consumer, never scene changes.
                if mul(elements, size_of::<PathEl>() as u64)? > limits.available_payload_bytes {
                    return Err(FastPreviewError::Limits(
                        "refined path reservation exceeds remaining payload",
                    ));
                }
                Ok(())
            })?;
            if layer.visible() && opaque {
                let cells = mul(
                    u64::from(frame.width).div_ceil(4) + 2,
                    u64::from(frame.height).div_ceil(4) + 2,
                )?;
                alpha = add(alpha, mul(cells, 16)?)?;
                strips = add(strips, add(mul(cells, 2)?, 2)?)?;
                draws = add(draws, 1)?;
            }
            counts.push(usize::try_from(elements).map_err(|_| FastPreviewError::Allocation)?);
            total = add(total, elements)?;
            maximum_lines = maximum_lines.max(elements);
            maximum_tiles = maximum_tiles.max(tiles);
        }
        // Axis-aligned canvas clipping uses the pinned fast rectangle path: at most two wide
        // edge rows plus two edge tiles per interior row (vello_common::rect). This bound also
        // covers the path fallback's sentinel/captive rows; it does not charge a full-canvas mask.
        let columns = u64::from(frame.width).div_ceil(4) + 2;
        let rows = u64::from(frame.height).div_ceil(4) + 2;
        let clip_strips = add(mul(rows, 2)?, 2)?;
        alpha = add(alpha, mul(add(columns, rows)?, 64)?)?;
        let path_strips = strips;
        strips = add(strips, clip_strips)?;
        if alpha >= 256 * MIB || alpha >= (1_u64 << 31) {
            return Err(FastPreviewError::Limits(
                "native alpha texture/address capacity",
            ));
        }
        let path = mul(
            counts.iter().copied().max().unwrap_or(0) as u64,
            size_of::<PathEl>() as u64,
        )?;
        let layers = scene.layers().len() as u64;
        let nodes = add(mul(layers, 2)?, 2)?;
        let blend_segments = mul(mul(layers, 2)?, clip_strips)?;
        // visit_strip_fill_segments emits at most two segments per adjacent strip pair.
        // Blending the one-level channel layers adds bounded clip segments and blit quads.
        let gpu_strips = add(
            add(mul(path_strips, 2)?, blend_segments)?,
            mul(add(layers, 1)?, 16)?,
        )?;
        let strip_frame_bytes =
            mul(gpu_strips, size_of::<vello_gpu::GpuStrip>() as u64)?.next_multiple_of(256);
        // The requested default max_buffer_size is 256 MiB; never rely on the native arena's
        // cap-and-truncate behavior when the complete scheduled upload cannot fit.
        if strip_frame_bytes > 256 * MIB {
            return Err(FastPreviewError::Limits(
                "native complete strip arena exceeds device buffer limit",
            ));
        }
        let alpha_texture_bytes = alpha.next_multiple_of(16 * u64::from(MAX_DIMENSION));
        let geometry = add(
            mul(
                maximum_lines,
                3 * size_of::<vello_common::flatten::Line>() as u64,
            )?,
            mul(
                maximum_tiles,
                4 * size_of::<vello_common::tile::Tile>() as u64,
            )?,
        )?;
        // Source-derived growing-Vec policy includes old+new allocation peaks (3× length):
        // output+temporary StripStorage; alpha; DrawBuffers; aggregate strip ranges; blend/copy
        // instances; RecordedDraw; public recorder nodes/layers; bounded per-layer schedule pools.
        let host_metadata = [
            mul(
                nodes,
                3 * (size_of::<vello_common::record::Node>() as u64
                    + size_of::<vello_common::record::RecordedLayer>() as u64
                    + 512
                    + 32),
            )?,
            mul(layers, 3 * 128)?,
            mul(add(layers, 4)?, 4096)?,
        ]
        .into_iter()
        .try_fold(0, add)?;
        let native_host = [
            mul(alpha, 6)?,
            mul(strips, 6 * size_of::<vello_common::strip::Strip>() as u64)?,
            mul(
                gpu_strips,
                3 * (size_of::<vello_gpu::GpuStrip>() as u64 + 16),
            )?,
            mul(blend_segments, 3 * (16 + 32 + 16))?,
            mul(draws, 3 * 64)?,
            host_metadata,
        ]
        .into_iter()
        .try_fold(0, add)?;
        let pixels = mul(
            u64::from(frame.width.min(MAX_DIMENSION)),
            u64::from(frame.height.min(MAX_DIMENSION)),
        )?;
        let target = mul(pixels, 8)?;
        let readback = mul(
            (u64::from(frame.width.min(MAX_DIMENSION)) * 4).div_ceil(256) * 256,
            u64::from(frame.height.min(MAX_DIMENSION)),
        )?;
        let upload_other = add(mul(blend_segments, 64)?, MIB)?;
        let upload_one_frame = add(add(alpha_texture_bytes, strip_frame_bytes)?, upload_other)?;
        let upload = mul(upload_one_frame, 4)?;
        let gpu = gpu_resource_peak(
            alpha_texture_bytes,
            strip_frame_bytes,
            upload_one_frame,
            target,
            readback,
        )?;
        let host = add(
            64 * MIB,
            add(
                add(path, geometry)?,
                add(native_host, add(upload, mul(pixels, 12)?)?)?,
            )?,
        )?;
        let reservation = FastPreviewReservation {
            host_bytes: host,
            gpu_bytes: limits.gpu_reservation_bytes,
            temporary_path_bytes: path,
            path_elements: total,
            alpha_bytes_upper_bound: alpha,
            strip_count_upper_bound: strips,
            gpu_strip_bytes_upper_bound: strip_frame_bytes,
            upload_bytes_upper_bound: upload_one_frame,
            upload_other_bytes_upper_bound: upload_other,
            host_metadata_bytes_upper_bound: host_metadata,
        };
        if gpu > limits.gpu_reservation_bytes
            || add(host, limits.gpu_reservation_bytes)? > limits.available_payload_bytes
        {
            return Err(FastPreviewError::Limits(
                "combined Fast reservation exceeds remaining payload or GPU pilot cap",
            ));
        }
        check_cancel(cancelled)?;
        Ok(Self {
            scene,
            frame,
            offset,
            per_channel_elements: counts,
            reservation,
            limits,
        })
    }
    /// Returns the checked physical frame without reevaluating canonical geometry.
    pub fn frame(&self) -> Frame {
        self.frame
    }
    /// Returns conservative payload admission facts; this is no RSS or exclusive-VRAM guarantee.
    pub fn reservation(&self) -> FastPreviewReservation {
        self.reservation
    }
}

/// Checks the ENTIRE scene, including invisible outputs, before any encoding or adapter attempt.
/// # Errors
/// Rejects unmodeled/source-color, multi-output, sampled paint or any non-circle canonical category.
pub fn validate_scene(scene: &RenderScene) -> Result<(), FastPreviewError> {
    if !matches!(
        scene.model(),
        Some(HalftoneChannelModel::Rgb | HalftoneChannelModel::Cmyk)
    ) {
        return Err(FastPreviewError::UnsupportedScene(
            "explicit RGB/CMYK required",
        ));
    }
    for layer in scene.layers() {
        if layer.outputs().len() != 1 {
            return Err(FastPreviewError::UnsupportedScene(
                "exactly one output per layer required",
            ));
        }
        let output = &layer.outputs()[0];
        if output.primitive_paints().is_some() {
            return Err(FastPreviewError::UnsupportedScene(
                "per-primitive paints are unsupported",
            ));
        }
        match output.geometry() {
            GeometryOutput::CircularMarks(_) => {}
            GeometryOutput::CanonicalMarks(marks)
                if marks
                    .iter()
                    .all(|mark| matches!(mark, CanonicalMark::Circle { .. })) => {}
            _ => {
                return Err(FastPreviewError::UnsupportedScene(
                    "only complete circle outputs are supported",
                ));
            }
        }
        visit_circles(layer, |center, radius| {
            if !center.x.is_finite() || !center.y.is_finite() || !radius.is_finite() || radius < 0.0
            {
                return Err(FastPreviewError::UnsupportedScene(
                    "invalid canonical circle",
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}

/// Retains device, pipelines and bounded reusable renderer resources on the owning worker.
/// Native scenes are deliberately not shared through Arc or a frontend mutex.
pub struct FastPreviewDevice {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: vello_gpu::Renderer,
    resources: vello_gpu::Resources,
    id: u64,
    failure: Arc<Mutex<Option<String>>>,
    adapter_name: String,
    reservation: FastPreviewReservation,
}
impl FastPreviewDevice {
    /// Automatically requests one hardware Vulkan adapter and initializes bounded solid-only resources.
    /// The admitted plan MUST precede initialization. This blocking function belongs on a worker.
    /// # Errors
    /// Returns unavailable/software/limits/init/device/timeout/cancellation errors without an image.
    pub fn new(
        plan: &FastPreviewPlan,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Self, FastPreviewError> {
        check_cancel(cancelled)?;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = await_bounded(
            instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                force_fallback_adapter: false,
                compatible_surface: None,
                ..Default::default()
            }),
            cancelled,
        )?
        .map_err(|error| FastPreviewError::Unavailable(error.to_string()))?;
        let info = adapter.get_info();
        if info.backend != wgpu::Backend::Vulkan
            || !matches!(
                info.device_type,
                wgpu::DeviceType::DiscreteGpu | wgpu::DeviceType::IntegratedGpu
            )
        {
            return Err(FastPreviewError::Unavailable(
                "hardware Vulkan adapter required".into(),
            ));
        }
        if adapter.limits().max_texture_dimension_2d < MAX_DIMENSION
            || !adapter.features().contains(wgpu::Features::CLEAR_TEXTURE)
        {
            return Err(FastPreviewError::Limits(
                "required native texture limits/features unavailable",
            ));
        }
        let required = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
        if !adapter
            .get_texture_format_features(wgpu::TextureFormat::Rgba8Unorm)
            .allowed_usages
            .contains(required)
        {
            return Err(FastPreviewError::Limits(
                "linear RGBA8 render/readback unsupported",
            ));
        }
        let (device, queue) = await_bounded(
            adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("Toniator approximate Fast preview"),
                required_features: wgpu::Features::CLEAR_TEXTURE,
                required_limits: wgpu::Limits {
                    max_texture_dimension_2d: MAX_DIMENSION,
                    ..Default::default()
                },
                ..Default::default()
            }),
            cancelled,
        )?
        .map_err(|error| FastPreviewError::Device(error.to_string()))?;
        let failure = Arc::new(Mutex::new(None));
        let errors = Arc::clone(&failure);
        device.on_uncaptured_error(Arc::new(move |error| {
            if let Ok(mut value) = errors.lock() {
                *value = Some(error.to_string());
            }
        }));
        let lost = Arc::clone(&failure);
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut value) = lost.lock() {
                *value = Some(format!("device lost: {reason:?}: {message}"));
            }
        });
        let settings =
            vello_gpu::RenderSettings::default().with_memory_settings(vello_gpu::MemorySettings {
                image_atlas_config: AtlasConfig {
                    initial_atlas_count: 0,
                    max_atlases: 0,
                    auto_grow: false,
                    ..Default::default()
                },
                layers_config: vello_gpu::LayersConfig {
                    max_textures: Some(4),
                    min_texture_size: vello_common::geometry::SizeU16::new(512),
                    max_texture_size: vello_common::geometry::SizeU16::new(MAX_DIMENSION as u16),
                },
            });
        let (renderer, resources) = vello_gpu::Renderer::new_with(
            &device,
            &queue,
            &vello_gpu::RenderTargetConfig {
                format: wgpu::TextureFormat::Rgba8Unorm,
                width: plan.frame.width.min(MAX_DIMENSION) as u16,
                height: plan.frame.height.min(MAX_DIMENSION) as u16,
            },
            settings,
        );
        check_cancel(cancelled)?;
        let value = Self {
            device,
            queue,
            renderer,
            resources,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            failure,
            adapter_name: info.name,
            reservation: plan.reservation,
        };
        value.check_device()?;
        Ok(value)
    }

    /// Preflights the complete next one-device/one-current-root high-water charge without encoding.
    /// The caller retires the old native root before preparation. This reservation already includes
    /// root preparation/temp/output and GPU resources; Root.reservation overlaps and is not additive.
    /// # Errors
    /// Returns arithmetic, retained-device or remaining-budget refusals before any native allocation.
    pub fn reservation_for_plan(
        &self,
        plan: &FastPreviewPlan,
    ) -> Result<FastPreviewReservation, FastPreviewError> {
        // Preserve each retained resource high-water independently: the largest alpha texture
        // and largest strip arena may originate in different roots. A smaller plan never resets
        // their charge. The caller retires the previous root before replacing preparation.
        let mut reservation = plan.reservation;
        let old = self.reservation;
        let current = plan.reservation;
        let alpha = current
            .alpha_bytes_upper_bound
            .max(old.alpha_bytes_upper_bound);
        let strips = current
            .gpu_strip_bytes_upper_bound
            .max(old.gpu_strip_bytes_upper_bound);
        let upload_other = current
            .upload_other_bytes_upper_bound
            .max(old.upload_other_bytes_upper_bound);
        let metadata = current
            .host_metadata_bytes_upper_bound
            .max(old.host_metadata_bytes_upper_bound);
        let upload = add(
            add(
                alpha.next_multiple_of(16 * u64::from(MAX_DIMENSION)),
                strips,
            )?,
            upload_other,
        )?;
        let pixels = mul(
            u64::from(plan.frame.width.min(MAX_DIMENSION)),
            u64::from(plan.frame.height.min(MAX_DIMENSION)),
        )?;
        let readback = mul(
            (u64::from(plan.frame.width.min(MAX_DIMENSION)) * 4).div_ceil(256) * 256,
            u64::from(plan.frame.height.min(MAX_DIMENSION)),
        )?;
        let gpu_peak = gpu_resource_peak(
            alpha.next_multiple_of(16 * u64::from(MAX_DIMENSION)),
            strips,
            upload,
            mul(pixels, 8)?,
            readback,
        )?;
        // Scheduled CPU GpuStrip+Range capacities use 5×F; blend scratch uses 3× its upload
        // instance bytes. Source-derived per-root geometry/recorder storage is not device-owned.
        let extra = add(
            add(
                mul(strips - current.gpu_strip_bytes_upper_bound, 5)?,
                mul(upload_other - current.upload_other_bytes_upper_bound, 3)?,
            )?,
            add(
                mul(upload - current.upload_bytes_upper_bound, 4)?,
                metadata - current.host_metadata_bytes_upper_bound,
            )?,
        )?;
        reservation.host_bytes = add(current.host_bytes, extra)?.max(old.host_bytes);
        reservation.gpu_bytes = current.gpu_bytes.max(old.gpu_bytes);
        reservation.alpha_bytes_upper_bound = alpha;
        reservation.gpu_strip_bytes_upper_bound = strips;
        reservation.upload_bytes_upper_bound = upload;
        reservation.upload_other_bytes_upper_bound = upload_other;
        reservation.host_metadata_bytes_upper_bound = metadata;
        if gpu_peak > plan.limits.gpu_reservation_bytes
            || add(reservation.host_bytes, reservation.gpu_bytes)?
                > plan.limits.available_payload_bytes
            || reservation.gpu_bytes > plan.limits.gpu_reservation_bytes
        {
            return Err(FastPreviewError::Limits(
                "retained device high-water exceeds remaining payload",
            ));
        }
        Ok(reservation)
    }

    /// Encodes admitted canonical circles once per exact zoom, preserving IDs/order/radii and guards.
    /// Full opaque channels form one positive compound path; translucent circles remain ordered.
    /// # Errors
    /// Returns cancellation/allocation/resource/device failure without a publishable partial root.
    pub fn prepare(
        &mut self,
        mut plan: FastPreviewPlan,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<FastPreviewRoot, FastPreviewError> {
        self.check_device()?;
        check_cancel(cancelled)?;
        plan.reservation = self.reservation_for_plan(&plan)?;
        self.reservation = plan.reservation;
        let mut native = vello_gpu::Scene::new(plan.frame.width as u16, plan.frame.height as u16);
        native.set_aliasing_threshold(None);
        native.set_transform(Affine::new([
            plan.frame.scale,
            0.0,
            0.0,
            plan.frame.scale,
            plan.offset.0,
            plan.offset.1,
        ]));
        native.push_clip_rect(&Rect::new(
            0.0,
            0.0,
            plan.scene.canvas().width,
            plan.scene.canvas().height,
        ));
        let mode = match plan.scene.model() {
            Some(HalftoneChannelModel::Rgb) => BlendMode::new(Mix::Normal, Compose::PlusLighter),
            _ => BlendMode::new(Mix::Multiply, Compose::SrcOver),
        };
        for (index, layer) in plan.scene.layers().iter().enumerate() {
            check_cancel(cancelled)?;
            if !layer.visible() {
                continue;
            }
            native.push_layer(None, Some(mode), Some(1.0), None, None);
            let color = layer.color();
            native.set_paint(AlphaColor::<Srgb>::new([
                color.red as f32,
                color.green as f32,
                color.blue as f32,
                (color.alpha * layer.opacity()) as f32,
            ]));
            native.set_fill_rule(Fill::NonZero);
            let opaque = color.alpha == 1.0 && layer.opacity() == 1.0;
            let mut compound = Vec::new();
            if opaque {
                compound
                    .try_reserve_exact(plan.per_channel_elements[index])
                    .map_err(|_| FastPreviewError::Allocation)?;
            }
            visit_circles(layer, |center, radius| {
                check_cancel(cancelled)?;
                if opaque {
                    projected_path(
                        center,
                        radius,
                        plan.frame.scale,
                        plan.offset,
                        Some(&mut compound),
                    )?;
                } else {
                    let (count, _) =
                        projected_path(center, radius, plan.frame.scale, plan.offset, None)?;
                    let mut path = Vec::new();
                    path.try_reserve_exact(count as usize)
                        .map_err(|_| FastPreviewError::Allocation)?;
                    projected_path(
                        center,
                        radius,
                        plan.frame.scale,
                        plan.offset,
                        Some(&mut path),
                    )?;
                    native.fill_path(&BezPath::from_vec(path));
                }
                Ok(())
            })?;
            if opaque {
                if compound.len() != plan.per_channel_elements[index] {
                    return Err(FastPreviewError::Limits("contour prepass differs"));
                }
                native.fill_path(&BezPath::from_vec(compound));
            }
            native.pop_layer();
        }
        native.pop_clip();
        check_cancel(cancelled)?;
        self.check_device()?;
        Ok(FastPreviewRoot {
            plan,
            native,
            device_id: self.id,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        })
    }

    /// Paints one complete checked region (including any caller-requested one-pixel gutter).
    /// The retained canonical scene/contours/device survive pans; no geometry regenerates here.
    /// Blocking fence/readback stays on the worker and has bounded polling/cancellation.
    /// # Errors
    /// Returns typed refusal/device/render/readback/timeout/cancellation errors with no partial pixels.
    pub fn render_region(
        &mut self,
        root: &FastPreviewRoot,
        region: Region,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<FastPreviewOutput, FastPreviewError> {
        self.check_device()?;
        check_cancel(cancelled)?;
        if root.device_id != self.id {
            return Err(FastPreviewError::Device(
                "root belongs to another device".into(),
            ));
        }
        if region.width == 0
            || region.height == 0
            || region.width > MAX_DIMENSION
            || region.height > MAX_DIMENSION
            || region
                .x
                .checked_add(region.width)
                .is_none_or(|x| x > root.plan.frame.width)
            || region
                .y
                .checked_add(region.height)
                .is_none_or(|y| y > root.plan.frame.height)
        {
            return Err(FastPreviewError::Limits("region is outside admitted frame"));
        }
        let result = self.render_region_inner(root, region, cancelled);
        if let Err(error) = &result
            && let Ok(mut failure) = self.failure.lock()
        {
            *failure = Some(format!(
                "consumer retired after incomplete native work: {error}"
            ));
        }
        result
    }

    /// Executes one admitted native job; any failure poisons this device in the public wrapper.
    /// A cancelled/timed-out submission may finish obsolete on the GPU, but this retained context
    /// cannot enqueue another job and accumulate unresolved staging/fence ownership.
    /// # Errors
    /// Propagates native allocation/render/readback/device, timeout and cancellation failures.
    fn render_region_inner(
        &mut self,
        root: &FastPreviewRoot,
        region: Region,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<FastPreviewOutput, FastPreviewError> {
        let size = wgpu::Extent3d {
            width: region.width,
            height: region.height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Fast preview linear region"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let depth = self
            .renderer
            .create_depth_texture_view(&vello_gpu::RenderSize {
                width: region.width as u16,
                height: region.height as u16,
            });
        let stride = (region.width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let bytes = u64::from(stride) * u64::from(region.height);
        if bytes > self.device.limits().max_buffer_size {
            return Err(FastPreviewError::Limits(
                "readback exceeds device buffer limit",
            ));
        }
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Fast preview padded readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut options = vello_gpu::RenderOptions::default();
        options.offset = (-(region.x as i32), -(region.y as i32));
        self.renderer
            .render(
                &root.native,
                &mut self.resources,
                &mut encoder,
                vello_gpu::RenderTarget {
                    view: &view,
                    depth_view: Some(&depth),
                },
                &vello_gpu::TextureBindings::new(),
                options,
            )
            .map_err(|error| FastPreviewError::Render(error.to_string()))?;
        self.check_device()?;
        check_cancel(cancelled)?;
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
            size,
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        let start = Instant::now();
        loop {
            check_cancel(cancelled)?;
            self.check_device()?;
            self.device
                .poll(wgpu::PollType::Poll)
                .map_err(|error| FastPreviewError::Readback(error.to_string()))?;
            match rx.try_recv() {
                Ok(result) => {
                    result.map_err(|error| FastPreviewError::Readback(error.to_string()))?;
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(FastPreviewError::Readback(
                        "map callback disconnected".into(),
                    ));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            if start.elapsed() > WORK_TIMEOUT {
                return Err(FastPreviewError::Timeout);
            }
            thread::sleep(Duration::from_millis(2));
        }
        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|error| FastPreviewError::Readback(error.to_string()))?;
        let mut output = Vec::new();
        output
            .try_reserve_exact((u64::from(region.width) * u64::from(region.height) * 4) as usize)
            .map_err(|_| FastPreviewError::Allocation)?;
        for row in mapped.chunks_exact(stride as usize) {
            check_cancel(cancelled)?;
            for pixel in row[..region.width as usize * 4].chunks_exact(4) {
                output.extend_from_slice(&linear_pixel_to_srgba(pixel, root.plan.frame.background));
            }
        }
        drop(mapped);
        buffer.unmap();
        self.check_device()?;
        check_cancel(cancelled)?;
        let surface = RasterSurface::new(region.width, region.height, output)
            .map_err(|error| FastPreviewError::Readback(error.to_string()))?;
        Ok(FastPreviewOutput {
            surface,
            provenance: FastPreviewProvenance {
                algorithm: ALGORITHM_VERSION,
                scene_identity: root.plan.scene.identity().scene_fingerprint().to_owned(),
                exact_scale_bits: root.plan.frame.scale.to_bits(),
                device_id: self.id,
                root_id: root.id,
                region,
                background: root.plan.frame.background,
            },
        })
    }
    /// Returns the living device's conservative high-water charge, released only by worker-side drop.
    /// The full GPU pilot allowance remains charged even when a new smaller root is prepared.
    pub fn reservation(&self) -> FastPreviewReservation {
        self.reservation
    }
    /// Returns adapter diagnostics without making vendor-specific admission or performance claims.
    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }
    /// Detects asynchronous native errors/device loss before publication.
    /// # Errors
    /// Returns the retained failure; the caller owns epoch retirement and Accurate fallback.
    fn check_device(&self) -> Result<(), FastPreviewError> {
        match self.failure.lock() {
            Ok(value) => match &*value {
                Some(error) => Err(FastPreviewError::Device(error.clone())),
                None => Ok(()),
            },
            Err(_) => Err(FastPreviewError::Device(
                "device diagnostic mutex poisoned".into(),
            )),
        }
    }
}

/// Worker-owned exact-scale native preparation; it is intentionally neither a CPU root nor certificate.
pub struct FastPreviewRoot {
    plan: FastPreviewPlan,
    native: vello_gpu::Scene,
    device_id: u64,
    id: u64,
}
impl FastPreviewRoot {
    /// Returns the accepted plan's complete physical frame.
    pub fn frame(&self) -> Frame {
        self.plan.frame
    }
    /// Returns its conservative admission charge, retained across compatible pans.
    pub fn reservation(&self) -> FastPreviewReservation {
        self.plan.reservation
    }
}

/// Completeness/provenance only; no geometric accuracy or error-bound claim is present.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FastPreviewProvenance {
    pub algorithm: &'static str,
    pub scene_identity: String,
    pub exact_scale_bits: u64,
    pub device_id: u64,
    pub root_id: u64,
    pub region: Region,
    pub background: RasterBackground,
}
/// A complete approximate region, still requiring the caller's document/epoch publication checks.
pub struct FastPreviewOutput {
    pub surface: RasterSurface,
    pub provenance: FastPreviewProvenance,
}

/// Visits supported canonical circles in original painter order, retaining zero/off-canvas guards.
/// # Errors
/// Propagates visitor refusal atomically; capability validation precedes this internal traversal.
fn visit_circles(
    layer: &super::RenderLayer,
    mut visit: impl FnMut(Point2, f64) -> Result<(), FastPreviewError>,
) -> Result<(), FastPreviewError> {
    match layer.outputs()[0].geometry() {
        GeometryOutput::CircularMarks(marks) => {
            for mark in marks {
                visit(mark.center, mark.radius)?;
            }
        }
        GeometryOutput::CanonicalMarks(marks) => {
            for mark in marks {
                if let CanonicalMark::Circle { center, radius, .. } = mark {
                    visit(*center, *radius)?;
                }
            }
        }
        _ => return Err(FastPreviewError::UnsupportedScene("noncircle output")),
    }
    Ok(())
}

/// Counts or emits the SAME line-only contour, with separate 1/64 Bézier and 1/128 flatten targets.
/// The per-circle projected-radius bound limits this noninterruptible library iterator. Tile visits
/// conservatively cover f32 line conversion and every crossed 4×4 cell, including sentinel padding.
/// # Errors
/// Refuses count/capacity overflow; it never silently truncates a contour.
fn projected_path(
    center: Point2,
    radius: f64,
    scale: f64,
    offset: (f64, f64),
    mut output: Option<&mut Vec<PathEl>>,
) -> Result<(u64, u64), FastPreviewError> {
    let mut count = 0_u64;
    let mut visits = 0_u64;
    let mut previous = None;
    let mut first = None;
    let mut overflow = false;
    vello_common::kurbo::flatten(
        Circle::new((center.x, center.y), radius).path_elements(1.0 / (64.0 * scale)),
        1.0 / (128.0 * scale),
        |element| {
            count = count.saturating_add(1);
            let point = match element {
                PathEl::MoveTo(p) => {
                    first = Some(p);
                    Some(p)
                }
                PathEl::LineTo(p) => Some(p),
                PathEl::ClosePath => first,
                _ => {
                    overflow = true;
                    None
                }
            };
            if !matches!(element, PathEl::MoveTo(_))
                && let (Some(a), Some(b)) = (previous, point)
            {
                let a: vello_common::kurbo::Point = a;
                let ax = (a.x * scale + offset.0) as f32;
                let ay = (a.y * scale + offset.1) as f32;
                let bx = (b.x * scale + offset.0) as f32;
                let by = (b.y * scale + offset.1) as f32;
                if [ax, ay, bx, by].iter().any(|v| !v.is_finite()) {
                    overflow = true;
                }
                let crossings = ((f64::from(ax - bx).abs() / 4.0).ceil()
                    + (f64::from(ay - by).abs() / 4.0).ceil()
                    + 12.0) as u64;
                visits = visits.saturating_add(crossings);
            }
            previous = point;
            if let Some(path) = &mut output {
                if path.len() < path.capacity() {
                    path.push(element);
                } else {
                    overflow = true;
                }
            }
        },
    );
    if overflow || count == u64::MAX || visits == u64::MAX {
        Err(FastPreviewError::Limits(
            "refined contour count/capacity overflow",
        ))
    } else {
        Ok((count, visits))
    }
}

/// Converts premultiplied numerical-LINEAR native bytes to final straight sRGBA8.
/// The fixed backdrop applies BEFORE unassociation and transfer, preserving alpha-zero semantics.
fn linear_pixel_to_srgba(pixel: &[u8], background: RasterBackground) -> [u8; 4] {
    let mut alpha = f64::from(pixel[3]) / 255.0;
    let backdrop = match background {
        RasterBackground::Transparent => None,
        RasterBackground::OpaqueBlack => Some(0.0),
        RasterBackground::OpaqueWhite => Some(1.0),
    };
    let mut rgb = [0_u8; 3];
    for channel in 0..3 {
        let mut value = f64::from(pixel[channel]) / 255.0;
        if let Some(backdrop) = backdrop {
            value += backdrop * (1.0 - alpha);
        }
        let output_alpha = if backdrop.is_some() { 1.0 } else { alpha };
        rgb[channel] = if output_alpha == 0.0 {
            0
        } else {
            (super::linear_to_srgb((value / output_alpha).clamp(0.0, 1.0)) * 255.0).round() as u8
        };
    }
    if backdrop.is_some() {
        alpha = 1.0;
    }
    [rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8]
}

/// Reserves serial completed-frame texture growth, strip-arena replacement and upload-belt peaks.
/// Layer caches clear before alpha replacement; their old/new allocation maxima do not coincide.
/// # Errors
/// Returns resource arithmetic overflow rather than admitting a partial native buffer.
fn gpu_resource_peak(
    alpha_texture: u64,
    strip_frame: u64,
    upload_frame: u64,
    target: u64,
    readback: u64,
) -> Result<u64, FastPreviewError> {
    let layer_textures = 256 * MIB;
    let texture_peak = add(mul(layer_textures, 2)?, alpha_texture)?
        .max(add(layer_textures, mul(alpha_texture, 2)?)?);
    add(
        add(texture_peak, 4 * MIB)?,
        add(
            add(target, readback)?,
            add(mul(strip_frame, 3)?, mul(upload_frame, 4)?)?,
        )?,
    )
}

/// Checks cancellation at a bounded consumer boundary without granting partial completion.
/// # Errors
/// Returns `Cancelled` when the caller retires this operation.
fn check_cancel(cancelled: &(dyn Fn() -> bool + Sync)) -> Result<(), FastPreviewError> {
    if cancelled() {
        Err(FastPreviewError::Cancelled)
    } else {
        Ok(())
    }
}
/// Checks finite positive native/u16/device pilot dimensions before casts or allocation.
/// # Errors
/// Refuses dimensions beyond the fixed intermediate-texture cap.
fn checked_dimension(value: f64) -> Result<u32, FastPreviewError> {
    if !value.is_finite() || value <= 0.0 || value.ceil() > f64::from(MAX_DIMENSION) {
        Err(FastPreviewError::Limits(
            "target dimensions exceed pilot native/device bound",
        ))
    } else {
        Ok(value.ceil() as u32)
    }
}
/// Checks the virtual encoded-scene extent independently of actual target/intermediate textures.
/// Native retained-pan offsets support u16 roots larger than the device's render-region dimension.
/// # Errors
/// Refuses nonfinite/nonpositive or non-u16 virtual dimensions before native casts.
fn checked_virtual_dimension(value: f64) -> Result<u32, FastPreviewError> {
    if !value.is_finite() || value <= 0.0 || value.ceil() > f64::from(u16::MAX) {
        Err(FastPreviewError::Limits(
            "virtual root exceeds native u16 representation",
        ))
    } else {
        Ok(value.ceil() as u32)
    }
}
/// Adds resource counts with atomic refusal on overflow.
/// # Errors
/// Returns the resource-overflow fallback reason.
fn add(a: u64, b: u64) -> Result<u64, FastPreviewError> {
    a.checked_add(b)
        .ok_or(FastPreviewError::Limits("resource reservation overflow"))
}
/// Multiplies resource counts with atomic refusal on overflow.
/// # Errors
/// Returns the resource-overflow fallback reason.
fn mul(a: u64, b: u64) -> Result<u64, FastPreviewError> {
    a.checked_mul(b)
        .ok_or(FastPreviewError::Limits("resource reservation overflow"))
}

/// Wakes the worker parking a native asynchronous initialization future.
struct ThreadWake(thread::Thread);
impl Wake for ThreadWake {
    /// Unparks the worker without touching GTK or the user's desktop thread.
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
/// Polls native adapter/device initialization with a finite deadline and cooperative cancellation.
/// # Errors
/// Returns timeout/cancellation; native initialization errors remain the future's typed output.
fn await_bounded<T>(
    future: impl Future<Output = T>,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<T, FastPreviewError> {
    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut context = Context::from_waker(&waker);
    let start = Instant::now();
    loop {
        check_cancel(cancelled)?;
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return Ok(value);
        }
        if start.elapsed() > WORK_TIMEOUT {
            return Err(FastPreviewError::Timeout);
        }
        thread::park_timeout(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests;
