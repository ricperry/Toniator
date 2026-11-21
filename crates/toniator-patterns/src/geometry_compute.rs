//! Optional exact integer compute assists never publish a partial canonical family.
//!
//! Shaders use only integer operations. CPU geometry remains authoritative for
//! uncertain candidates, unsupported jobs, contention, and any device failure.

use std::{
    future::Future,
    pin::pin,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    task::{Context, Poll, Wake, Waker},
    thread,
    time::{Duration, Instant},
};

const MAX_BYTES: u64 = 64 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(10);

/// Pairs exact CPU axis coordinates in original candidate order and classifies interior sites.
/// Only zero-rotation transforms are separable; the caller enforces that contract.
/// Rounded support-margin predicates remain on the CPU for guard candidates.
#[allow(clippy::too_many_arguments)]
pub(super) fn rectangular_grid(
    first: (i64, i64),
    second: (i64, i64),
    spacing: (f64, f64),
    transform: crate::AffineTransform2D,
    canvas: crate::Bounds,
    margin: f64,
    cancel: &dyn Fn() -> bool,
) -> Option<Vec<(i64, i64, crate::Point2, crate::SiteScope)>> {
    // Paired complete-scene measurements find no grid crossover; automatic mode uses CPU.
    if std::env::var("TONIATOR_GEOMETRY_COMPUTE").as_deref() != Ok("gpu") {
        return None;
    }
    let nx = usize::try_from(first.1.checked_sub(first.0)?.checked_add(1)?).ok()?;
    let ny = usize::try_from(second.1.checked_sub(second.0)?.checked_add(1)?).ok()?;
    let count = nx.checked_mul(ny)?;
    if count.checked_mul(5)? > (MAX_BYTES / 4) as usize {
        return None;
    }
    let mut data = Vec::new();
    data.try_reserve_exact(12 + 2 * (nx + ny)).ok()?;
    data.resize(12, 0);
    data[0] = u32::try_from(count).ok()?;
    data[1] = u32::try_from(nx).ok()?;
    data[2] = u32::try_from(ny).ok()?;
    let bits = |value: f64| {
        let bits = value.to_bits();
        [bits as u32, (bits >> 32) as u32]
    };
    data[4..6].copy_from_slice(&bits(canvas.max.x));
    data[6..8].copy_from_slice(&bits(canvas.max.y));
    for i in first.0..=first.1 {
        if cancel() {
            return None;
        }
        let point = transform.apply_point(crate::Point2::new(i as f64 * spacing.0, 0.0));
        if !point.is_finite() {
            return None;
        }
        data.extend(bits(point.x));
    }
    for i in second.0..=second.1 {
        if cancel() {
            return None;
        }
        let point = transform.apply_point(crate::Point2::new(0.0, i as f64 * spacing.1));
        if !point.is_finite() {
            return None;
        }
        data.extend(bits(point.y));
    }
    let output = dispatch(Kernel::Grid, &data, count * 5, count, cancel)?;
    let mut sites = Vec::new();
    sites.try_reserve_exact(count).ok()?;
    for (ordinal, words) in output.chunks_exact(5).enumerate() {
        if ordinal.is_multiple_of(4096) && cancel() {
            return None;
        }
        let point = crate::Point2::new(
            f64::from_bits(u64::from(words[0]) | (u64::from(words[1]) << 32)),
            f64::from_bits(u64::from(words[2]) | (u64::from(words[3]) << 32)),
        );
        let scope = match words[4] {
            2 => crate::SiteScope::Canvas,
            1 => {
                if crate::distance_to_canvas(point, canvas) > margin {
                    continue;
                }
                crate::SiteScope::Guard
            }
            _ => return None,
        };
        sites.push((
            first.0 + (ordinal / ny) as i64,
            second.0 + (ordinal % ny) as i64,
            point,
            scope,
        ));
    }
    Some(sites)
}

/// A kernel selects its fixed, integer-only shader and output record width.
#[derive(Clone, Copy, Debug)]
pub(super) enum Kernel {
    Nearest,
    Grid,
}

/// Admits sufficiently large jobs; explicit CPU mode never initializes Vulkan.
pub(super) fn admitted(count: usize, minimum: usize) -> bool {
    match std::env::var("TONIATOR_GEOMETRY_COMPUTE").as_deref() {
        Ok("cpu") => false,
        Ok("gpu") => count > 0,
        _ => count >= minimum,
    }
}

/// Runs a bounded complete assist or atomically falls back to its caller's CPU path.
/// Contention never queues GPU work behind another family. A failing device is retired.
pub(super) fn dispatch(
    kernel: Kernel,
    input: &[u32],
    output_words: usize,
    invocations: usize,
    cancel: &dyn Fn() -> bool,
) -> Option<Vec<u32>> {
    if cancel()
        || input.len().checked_mul(4)? as u64 > MAX_BYTES
        || output_words.checked_mul(4)? as u64 > MAX_BYTES
        || invocations == 0
        || invocations.div_ceil(256) > 65_535
    {
        return None;
    }
    static DEVICE: OnceLock<Mutex<Option<Compute>>> = OnceLock::new();
    let mut state = DEVICE.get_or_init(|| Mutex::new(None)).try_lock().ok()?;
    static RETIRED: AtomicBool = AtomicBool::new(false);
    if RETIRED.load(Ordering::Relaxed) {
        return None;
    }
    let started = Instant::now();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if state.is_none() {
            *state = Some(Compute::new(cancel)?);
        }
        state
            .as_mut()?
            .run(kernel, input, output_words, invocations, cancel)
    }))
    .ok()
    .flatten();
    if result.is_none() {
        // Drop pending/mapped buffers after cancellation; a fresh job may initialize again.
        if !cancel() {
            RETIRED.store(true, Ordering::Relaxed);
        }
        *state = None;
    }
    if std::env::var_os("TONIATOR_GEOMETRY_COMPUTE_TRACE").is_some() {
        eprintln!(
            "geometry-compute kernel={kernel:?} invocations={invocations} output_words={output_words} completed={} elapsed_ms={:.3}",
            result.is_some(),
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
    result
}

/// Owns one hardware Vulkan device and retained bounded transfer buffers.
struct Compute {
    device: wgpu::Device,
    queue: wgpu::Queue,
    nearest: wgpu::ComputePipeline,
    grid: wgpu::ComputePipeline,
    failed: Arc<AtomicBool>,
    buffers: Option<Buffers>,
}

/// Retains one input/output/readback allocation across independent family jobs.
struct Buffers {
    input: wgpu::Buffer,
    output: wgpu::Buffer,
    readback: wgpu::Buffer,
    input_capacity: u64,
    output_capacity: u64,
}

impl Compute {
    /// Detects hardware Vulkan and creates integer-only pipelines without optional float features.
    /// Returns `None` for unavailable hardware, timeout, cancellation, or validation failure.
    fn new(cancel: &dyn Fn() -> bool) -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = await_bounded(
            instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
            cancel,
        )?
        .ok()?;
        let info = adapter.get_info();
        if !matches!(
            info.device_type,
            wgpu::DeviceType::DiscreteGpu | wgpu::DeviceType::IntegratedGpu
        ) {
            return None;
        }
        let (device, queue) = await_bounded(
            adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("Toniator certified geometry compute"),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Default::default()
            }),
            cancel,
        )?
        .ok()?;
        let failed = Arc::new(AtomicBool::new(false));
        let failure = Arc::clone(&failed);
        device.on_uncaptured_error(Arc::new(move |_| {
            failure.store(true, Ordering::Relaxed);
        }));
        let failure = Arc::clone(&failed);
        device.set_device_lost_callback(move |_, _| {
            failure.store(true, Ordering::Relaxed);
        });
        let pipeline = |source: &'static str| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Toniator certified integer kernel"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Toniator certified integer kernel"),
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let nearest = pipeline(include_str!("geometry_nearest.wgsl"));
        let grid = pipeline(include_str!("geometry_grid.wgsl"));
        if failed.load(Ordering::Relaxed) {
            return None;
        }
        if std::env::var_os("TONIATOR_GEOMETRY_COMPUTE_TRACE").is_some() {
            eprintln!(
                "geometry-compute adapter={} backend={:?} arithmetic=u32",
                info.name, info.backend
            );
        }
        Some(Self {
            device,
            queue,
            nearest,
            grid,
            failed,
            buffers: None,
        })
    }

    /// Submits one complete integer batch and reads it before the bounded deadline.
    /// Returns no output on cancellation/device errors; mapped storage never escapes.
    fn run(
        &mut self,
        kernel: Kernel,
        input: &[u32],
        output_words: usize,
        invocations: usize,
        cancel: &dyn Fn() -> bool,
    ) -> Option<Vec<u32>> {
        if self.failed.load(Ordering::Relaxed) || cancel() {
            return None;
        }
        let input_bytes = (input.len() as u64 * 4).max(4);
        let output_bytes = (output_words as u64 * 4).max(4);
        if self
            .buffers
            .as_ref()
            .is_none_or(|b| b.input_capacity < input_bytes || b.output_capacity < output_bytes)
        {
            let input_capacity = input_bytes.next_power_of_two().min(MAX_BYTES);
            let output_capacity = output_bytes.next_power_of_two().min(MAX_BYTES);
            let make = |size, usage| {
                self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Toniator bounded geometry transfer"),
                    size,
                    usage,
                    mapped_at_creation: false,
                })
            };
            self.buffers = Some(Buffers {
                input: make(
                    input_capacity,
                    wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                ),
                output: make(
                    output_capacity,
                    wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                ),
                readback: make(
                    output_capacity,
                    wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                ),
                input_capacity,
                output_capacity,
            });
        }
        let buffers = self.buffers.as_ref()?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(input.len() * 4).ok()?;
        for word in input {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        self.queue.write_buffer(&buffers.input, 0, &bytes);
        let pipeline = match kernel {
            Kernel::Nearest => &self.nearest,
            Kernel::Grid => &self.grid,
        };
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffers.output.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(invocations.div_ceil(256) as u32, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&buffers.output, 0, &buffers.readback, 0, output_bytes);
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = mpsc::channel();
        buffers
            .readback
            .slice(..output_bytes)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        let started = Instant::now();
        loop {
            if cancel() || started.elapsed() > TIMEOUT || self.failed.load(Ordering::Relaxed) {
                return None;
            }
            self.device.poll(wgpu::PollType::Poll).ok()?;
            match rx.try_recv() {
                Ok(result) => {
                    result.ok()?;
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => return None,
                Err(mpsc::TryRecvError::Empty) => thread::sleep(Duration::from_millis(1)),
            }
        }
        let mapped = buffers
            .readback
            .slice(..output_bytes)
            .get_mapped_range()
            .ok()?;
        let mut out = Vec::new();
        out.try_reserve_exact(output_words).ok()?;
        out.extend(
            mapped
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().expect("four-byte word"))),
        );
        drop(mapped);
        buffers.readback.unmap();
        (!self.failed.load(Ordering::Relaxed)).then_some(out)
    }
}

/// Supplies a no-op wake because bounded native polling drives adapter/device futures.
struct NoWake;
impl Wake for NoWake {
    /// Leaves polling to the bounded worker loop.
    fn wake(self: Arc<Self>) {}
}

/// Polls native setup without an async runtime, bounded by cancellation and ten seconds.
fn await_bounded<T>(future: impl Future<Output = T>, cancel: &dyn Fn() -> bool) -> Option<T> {
    let waker = Waker::from(Arc::new(NoWake));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    let started = Instant::now();
    loop {
        if cancel() || started.elapsed() > TIMEOUT {
            return None;
        }
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return Some(value);
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Exercises cancellation after submit/map request, then a fresh real hardware dispatch.
    /// # Panics
    /// Panics if integer hardware execution or cancellation cleanup fails.
    #[test]
    #[ignore = "requires the guarded hardware GPU slot"]
    fn cancelled_submission_restarts_hardware() {
        let input = [1, 10, 12, 14, 15, 1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 3, 0, 3, 0];
        assert_eq!(
            dispatch(Kernel::Nearest, &input, 1, 1, &|| false),
            Some(vec![0])
        );
        let checks = Cell::new(0);
        assert!(
            dispatch(Kernel::Nearest, &input, 1, 1, &|| {
                checks.set(checks.get() + 1);
                checks.get() >= 3
            })
            .is_none()
        );
        assert!(checks.get() >= 3, "probe reaches the post-submit map loop");
        assert_eq!(
            dispatch(Kernel::Nearest, &input, 1, 1, &|| false),
            Some(vec![0])
        );
    }

    /// Verifies raw signed-zero/translated axis bits against the original complete transform.
    /// # Panics
    /// Panics if GPU coordinate assembly or scope classification changes canonical geometry.
    #[test]
    #[ignore = "requires the guarded hardware GPU slot and TONIATOR_GEOMETRY_COMPUTE=gpu"]
    fn rectangular_grid_hardware_bits() {
        assert_eq!(
            std::env::var("TONIATOR_GEOMETRY_COMPUTE").as_deref(),
            Ok("gpu")
        );
        let canvas = crate::Bounds::new(
            crate::Point2::new(0.0, 0.0),
            crate::Point2::new(19.5, 13.25),
        )
        .unwrap();
        for rotation in [0.0, -0.0] {
            for translation in [
                crate::Vector2::new(0.0, -0.0),
                crate::Vector2::new(2.25, -3.0),
            ] {
                let transform = crate::AffineTransform2D::rotate_about_then_translate(
                    crate::Point2::new(0.0, 0.0),
                    rotation,
                    translation,
                )
                .unwrap();
                let sites = rectangular_grid(
                    (-20, 20),
                    (-20, 20),
                    (0.75, 0.625),
                    transform,
                    canvas,
                    2.0,
                    &|| false,
                )
                .unwrap();
                let mut expected = Vec::new();
                for x in -20..=20 {
                    for y in -20..=20 {
                        let point = transform
                            .apply_point(crate::Point2::new(x as f64 * 0.75, y as f64 * 0.625));
                        if crate::distance_to_canvas(point, canvas) <= 2.0 {
                            expected.push((
                                x,
                                y,
                                point,
                                if canvas.contains(point) {
                                    crate::SiteScope::Canvas
                                } else {
                                    crate::SiteScope::Guard
                                },
                            ));
                        }
                    }
                }
                assert_eq!(sites.len(), expected.len());
                for (actual, expected) in sites.iter().zip(expected.iter()) {
                    assert_eq!(
                        (
                            actual.0,
                            actual.1,
                            actual.2.x.to_bits(),
                            actual.2.y.to_bits(),
                            actual.3
                        ),
                        (
                            expected.0,
                            expected.1,
                            expected.2.x.to_bits(),
                            expected.2.y.to_bits(),
                            expected.3
                        )
                    );
                }
            }
        }
    }
}
