//! Default-off diagnostic prepared-AA crop consumption; no app, cache or publication authority.
use super::*;
use area_raster::{PixelRegion, PreparedAreaLayer};
use std::time::{Duration, Instant};

/// One requested half-open root-pixel rectangle; run validates its checked end against the root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl Crop {
    /// Admits positive dimensions and checked endpoints without choosing a new raster frame.
    /// # Errors
    /// Returns raster.crop.region for empty or overflowing dimensions.
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, RenderError> {
        if width == 0
            || height == 0
            || x.checked_add(width).is_none()
            || y.checked_add(height).is_none()
        {
            return Err(error("raster.crop.region", "crop is empty or overflows"));
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }
    /// Checks this rectangle against the original root, including public-field construction.
    /// # Errors
    /// Returns raster.crop.region for empty, overflowing or outside rectangles.
    fn region(self, target: PreviewRasterTarget) -> Result<PixelRegion, RenderError> {
        Self::new(self.x, self.y, self.width, self.height)?;
        PixelRegion::new(
            self.x,
            self.y,
            self.x + self.width,
            self.y + self.height,
            target.width,
            target.height,
        )
    }
}

/// Explicit diagnostic payload admission, not a portable total-RSS limit.
/// Default disables profiling; zero default resource/batch fields require explicit configuration.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// Maximum estimated live payload, including declared caller buffers and prepared capacity.
    pub maximum_estimated_payload_bytes: usize,
    /// Actual caller-owned reference/stitch/other retained buffer capacity, declared separately.
    pub caller_retained_bytes: usize,
    /// Positive jobs per batch, no greater than the supplied pool's worker count.
    pub maximum_in_flight: usize,
    /// Opt-in coarse diagnostic clocks; false/default performs no clock reads.
    pub profiling: bool,
}

/// Small completed-run facts; no geometry, collected tiles or invented constructor counters.
#[derive(Clone, Debug)]
pub struct Summary {
    pub delivered_tiles: usize,
    pub maximum_batch_tiles: usize,
    pub prepared_capacity_bytes: usize,
    pub maximum_estimated_payload_bytes: usize,
    pub normalized_edges: usize,
    pub prepared_paints: usize,
    pub root_primitives: usize,
    pub consumed_flattened_edges: usize,
    pub consumed_boolean_operations: usize,
    /// None for primary unprofiled runs; worker sums/maxima are not additive wall phases.
    pub timings: Option<Timings>,
}

/// Opt-in coarse diagnostic durations; worker phases overlap across jobs and nested Rayon tasks.
#[derive(Clone, Debug, Default)]
pub struct Timings {
    pub materialization: Duration,
    pub preparation: Duration,
    pub admission: Duration,
    pub worker_batch_wall: Duration,
    pub sink: Duration,
    pub integration_job_sum: Duration,
    pub integration_job_max: Duration,
    pub composition_job_sum: Duration,
    pub composition_job_max: Duration,
    pub quantization_job_sum: Duration,
    pub quantization_job_max: Duration,
}

/// Retains one bounded job's optional phase observations beside its local completed pixels.
#[derive(Clone, Debug, Default)]
struct JobTimings {
    integration: Duration,
    composition: Duration,
    quantization: Duration,
}

/// Starts a coarse clock only for an explicitly profiled diagnostic run.
fn clock(enabled: bool) -> Option<Instant> {
    enabled.then(Instant::now)
}

/// Reads elapsed time only for a previously enabled coarse diagnostic boundary.
fn elapsed(start: Option<Instant>) -> Duration {
    start.map(|t| t.elapsed()).unwrap_or_default()
}

/// Constructs a diagnostic error without changing renderer numerical policy.
fn error(path: &'static str, message: &'static str) -> RenderError {
    RenderError::new(path, message)
}

/// Adds payload bytes with checked platform arithmetic; allocator overhead stays excluded.
/// # Errors
/// Returns raster.crop.memory rather than saturating overflow.
fn add(a: usize, b: usize) -> Result<usize, RenderError> {
    a.checked_add(b)
        .ok_or(error("raster.crop.memory", "estimated payload overflows"))
}

/// Multiplies payload dimensions with checked platform arithmetic.
/// # Errors
/// Returns raster.crop.memory rather than admitting a wrapped byte count.
fn mul(a: usize, b: usize) -> Result<usize, RenderError> {
    a.checked_mul(b)
        .ok_or(error("raster.crop.memory", "estimated payload overflows"))
}

/// Checked virtual physical extent for preparation only; it never admits or allocates a pixel surface.
/// This is independent of the small Main target supplying the original scale/translation/frame.
#[derive(Clone, Copy, Debug)]
pub struct VirtualExtent {
    pub width: u32,
    pub height: u32,
}
impl VirtualExtent {
    /// Checks positive dimensions and platform pixel-index arithmetic without a pixel-allocation cap.
    /// # Errors
    /// Returns raster.crop.frame for zero dimensions or a virtual index product that cannot fit usize.
    pub fn new(width: u32, height: u32) -> Result<Self, RenderError> {
        if width == 0 || height == 0 || (width as usize).checked_mul(height as usize).is_none() {
            return Err(error(
                "raster.crop.frame",
                "virtual extent is empty or overflows indexing",
            ));
        }
        Ok(Self { width, height })
    }
}

/// Actual completed-layer capacity/cardinality facts; no geometry or pixel buffer is returned.
#[derive(Debug)]
pub struct PreparationLayerFacts {
    pub channel: ChannelId,
    pub prepared_capacity_bytes: usize,
    pub paints: usize,
    pub normalized_edges: usize,
}

/// Observes one preparation attempt's actual work and completed layers, including failed attempts.
/// Per-layer certificates/retry remain private in the unchanged area renderer; they are not inferred.
#[derive(Debug, Default)]
pub struct PreparationFacts {
    pub layers: Vec<PreparationLayerFacts>,
    pub failed_channel: Option<ChannelId>,
    pub root_primitives: usize,
    pub consumed_flattened_edges: usize,
    pub consumed_boolean_operations: usize,
}

/// Retains the actual renderer error and consumed work without publishing partial prepared geometry.
#[derive(Debug)]
pub struct PreparationFailure {
    pub error: RenderError,
    pub facts: PreparationFacts,
}

/// Probes full virtual-root preparation using the existing Main frame and unchanged layer authority.
/// Whole virtual dimensions govern early circle visibility; the small target only supplies the
/// original viewport transform. Canvas clipping adds its actual rectangle; false requests the
/// existing unclipped positive union, whose final global split certificate is still charged.
/// Each call owns one fresh request work ledger and the existing per-layer omission/single retry.
/// Completed layers drop after observing their actual capacity; no root pixels are allocated and
/// this diagnostic does not establish retained-root storage, painting, or presentation correctness.
/// # Errors
/// Returns the actual validation/materialization/preparation/allocation/cancellation diagnostic with
/// consumed work and completed-layer facts. Certificate values/adapter rectangles are not exposed.
pub fn prepare_only(
    scene: &RenderScene,
    target: PreviewRasterTarget,
    extent: VirtualExtent,
    clip_to_canvas: bool,
    limits: RasterizationLimits,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<PreparationFacts, PreparationFailure> {
    let mut facts = PreparationFacts::default();
    let mut work = RasterWork::with_progress(
        limits,
        RasterAntialiasing::On,
        is_cancelled,
        0,
        &ignore_raster_progress,
    );
    let attempted = (|| -> Result<(), RenderError> {
        VirtualExtent::new(extent.width, extent.height)?;
        if target.viewport().is_none() {
            return Err(error(
                "raster.crop.frame",
                "preparation requires an explicit Main viewport frame",
            ));
        }
        let transform = PreviewTransform::for_scene(scene, target);
        if ![
            transform.scale,
            transform.offset_x,
            transform.offset_y,
            transform.right,
            transform.bottom,
        ]
        .into_iter()
        .all(f64::is_finite)
        {
            return Err(error(
                "preview.target",
                "resolved viewport transform exceeds finite raster coordinates",
            ));
        }
        work.check()?;
        let materialized = materialization::prepare_scene(scene, is_cancelled)?;
        let scene = materialized.as_ref();
        work.preview_clip = Some(transform);
        let clip = clip_to_canvas.then_some([
            transform.offset_x,
            transform.offset_y,
            transform.right,
            transform.bottom,
        ]);
        facts.root_primitives = scene
            .layers
            .iter()
            .filter(|layer| layer.visible)
            .flat_map(|layer| &layer.outputs)
            .try_fold(0usize, |n, output| {
                add(n, output.geometry().primitive_count())
            })?;
        for layer in &scene.layers {
            work.check()?;
            facts.failed_channel = Some(layer.channel_id);
            let prepared = area_raster::prepare_area_layer_with_clip(
                layer,
                extent.width,
                extent.height,
                CanonicalRasterTransform::preview(transform),
                clip,
                &mut work,
            )?;
            let (bytes, paints, edges) = prepared.pilot_storage()?;
            facts.layers.try_reserve(1).map_err(|_| {
                error(
                    "raster.allocation",
                    "preparation observation allocation failed",
                )
            })?;
            facts.layers.push(PreparationLayerFacts {
                channel: layer.channel_id,
                prepared_capacity_bytes: bytes,
                paints,
                normalized_edges: edges,
            });
        }
        facts.failed_channel = None;
        Ok(())
    })();
    facts.consumed_flattened_edges = limits.max_flattened_edges() - work.remaining_edges;
    facts.consumed_boolean_operations =
        limits.max_boolean_operations() - work.remaining_boolean_operations;
    match attempted {
        Ok(()) => Ok(facts),
        Err(error) => Err(PreparationFailure { error, facts }),
    }
}

/// Prepares one borrowed actual Main scene once, then delivers ordered bounded crop batches.
/// Root transform/canvas clip, adapter/grid, approximation ledgers and the single typed retry are
/// shared. Workers borrow immutable prepared layers and have zero edge/Boolean allowance, catching
/// accidental preparation. They preserve global-X prefix arithmetic and local Y bins only.
/// At most maximum_in_flight<=pool workers' results coexist; sink runs on this caller thread in
/// input order and each returned tile drops before the next batch. Earlier sink deliveries may
/// remain on failure: callers own staging/atomic publication and must accept only a final Ok.
/// Payload admission includes actual prepared capacity, declared caller buffers, borrowed crop-plan
/// payload and worst-paint row bins/global-X scratch plus68bytes/crop pixel and result slots.
/// Current-toolchain vector growth/transient allowance is an estimate: scene/vendor preparation
/// transients, allocator metadata/over-allocation and Rayon internals are not a guaranteed RSS cap.
/// # Errors
/// Returns region/options/admission, allocation, root precision/work, cancellation, painting or sink
/// errors without returning a completed summary. Smaller crops never reset root work/certificates.
#[expect(
    clippy::too_many_arguments,
    reason = "diagnostic facade keeps root, resource, pool and sink authority explicit"
)]
pub fn run(
    scene: &RenderScene,
    target: PreviewRasterTarget,
    limits: RasterizationLimits,
    crops: &[Crop],
    options: Options,
    pool: &rayon::ThreadPool,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    mut sink: impl FnMut(Crop, &RasterSurface) -> Result<(), RenderError>,
) -> Result<Summary, RenderError> {
    if crops.is_empty()
        || options.maximum_in_flight == 0
        || options.maximum_in_flight > pool.current_num_threads()
    {
        return Err(error(
            "raster.crop.options",
            "a nonempty plan and bounded positive worker batch are required",
        ));
    }
    let mut work = RasterWork::with_progress(
        limits,
        RasterAntialiasing::On,
        is_cancelled,
        0,
        &ignore_raster_progress,
    );
    work.check()?;
    for crop in crops {
        work.check()?;
        crop.region(target)?;
    }
    let declared = add(
        options.caller_retained_bytes,
        mul(crops.len(), std::mem::size_of::<Crop>())?,
    )?;
    if declared > options.maximum_estimated_payload_bytes {
        return Err(error(
            "raster.crop.memory",
            "declared caller payload exceeds admission",
        ));
    }
    let transform = PreviewTransform::for_scene(scene, target);
    if ![
        transform.scale,
        transform.offset_x,
        transform.offset_y,
        transform.right,
        transform.bottom,
    ]
    .into_iter()
    .all(f64::is_finite)
    {
        return Err(error(
            "preview.target",
            "resolved viewport transform exceeds finite raster coordinates",
        ));
    }
    let materialization_started = clock(options.profiling);
    let prepared_scene = materialization::prepare_scene(scene, is_cancelled)?;
    let materialization_time = elapsed(materialization_started);
    let preparation_started = clock(options.profiling);
    let scene = prepared_scene.as_ref();
    work.preview_clip = Some(transform);
    let clip = Some([
        transform.offset_x,
        transform.offset_y,
        transform.right,
        transform.bottom,
    ]);
    let mut layers = Vec::new();
    for layer in &scene.layers {
        work.check()?;
        layers.try_reserve(1).map_err(|_| {
            error(
                "raster.allocation",
                "pilot prepared layer allocation failed",
            )
        })?;
        layers.push(area_raster::prepare_area_layer_with_clip(
            layer,
            target.width,
            target.height,
            CanonicalRasterTransform::preview(transform),
            clip,
            &mut work,
        )?);
    }
    let mut prepared_bytes = mul(layers.capacity(), std::mem::size_of::<PreparedAreaLayer>())?;
    let mut paints = 0usize;
    let mut edges = 0usize;
    for layer in &layers {
        let (bytes, p, e) = layer.pilot_storage()?;
        prepared_bytes = add(prepared_bytes, bytes)?;
        paints = add(paints, p)?;
        edges = add(edges, e)?;
    }
    let base = add(declared, prepared_bytes)?;
    if base > options.maximum_estimated_payload_bytes {
        return Err(error(
            "raster.crop.memory",
            "prepared/caller payload exceeds admission",
        ));
    }
    let root_primitives = scene
        .layers
        .iter()
        .filter(|l| l.visible)
        .flat_map(|l| &l.outputs)
        .try_fold(0usize, |n, o| add(n, o.geometry().primitive_count()))?;
    let model = scene.model;
    // Prepared layers own their normalized geometry and colors. Release any owned materialization
    // before worker admission; caller-owned original scene storage remains explicitly excluded.
    drop(prepared_scene);
    let preparation_time = elapsed(preparation_started);
    let mut summary = Summary {
        delivered_tiles: 0,
        maximum_batch_tiles: 0,
        prepared_capacity_bytes: prepared_bytes,
        maximum_estimated_payload_bytes: base,
        normalized_edges: edges,
        prepared_paints: paints,
        root_primitives,
        consumed_flattened_edges: limits.max_flattened_edges() - work.remaining_edges,
        consumed_boolean_operations: limits.max_boolean_operations()
            - work.remaining_boolean_operations,
        timings: options.profiling.then_some(Timings {
            materialization: materialization_time,
            preparation: preparation_time,
            ..Timings::default()
        }),
    };
    for batch in crops.chunks(options.maximum_in_flight) {
        work.check()?;
        let admission_started = clock(options.profiling);
        let mut estimate = add(
            base,
            mul(
                batch.len(),
                std::mem::size_of::<Result<(RasterSurface, JobTimings), RenderError>>(),
            )?,
        )?;
        for crop in batch {
            let region = crop.region(target)?;
            let scratch = layers.iter().try_fold(0usize, |n, l| {
                Ok::<_, RenderError>(n.max(l.pilot_scratch(
                    target.width,
                    target.height,
                    region,
                    &work,
                )?))
            })?;
            estimate = add(
                estimate,
                add(
                    mul(mul(crop.width as usize, crop.height as usize)?, 68)?,
                    scratch,
                )?,
            )?;
        }
        if estimate > options.maximum_estimated_payload_bytes {
            return Err(error(
                "raster.crop.memory",
                "batch estimated payload exceeds admission",
            ));
        }
        summary.maximum_estimated_payload_bytes =
            summary.maximum_estimated_payload_bytes.max(estimate);
        summary.maximum_batch_tiles = summary.maximum_batch_tiles.max(batch.len());
        // Indexed parallel collection preserves request order; explicit reservation is fallible.
        let mut results = Vec::new();
        results
            .try_reserve_exact(batch.len())
            .map_err(|_| error("raster.allocation", "pilot result allocation failed"))?;
        if let Some(t) = &mut summary.timings {
            t.admission += elapsed(admission_started);
        }
        let worker_started = clock(options.profiling);
        pool.install(|| {
            batch
                .par_iter()
                .map(|&crop| {
                    paint(
                        model,
                        &layers,
                        target,
                        crop,
                        is_cancelled,
                        options.profiling,
                    )
                })
                .collect_into_vec(&mut results)
        });
        if let Some(t) = &mut summary.timings {
            t.worker_batch_wall += elapsed(worker_started);
        }
        for (&crop, result) in batch.iter().zip(results) {
            work.check()?;
            let (surface, job) = result?;
            if let Some(t) = &mut summary.timings {
                t.integration_job_sum += job.integration;
                t.integration_job_max = t.integration_job_max.max(job.integration);
                t.composition_job_sum += job.composition;
                t.composition_job_max = t.composition_job_max.max(job.composition);
                t.quantization_job_sum += job.quantization;
                t.quantization_job_max = t.quantization_job_max.max(job.quantization);
            }
            let sink_started = clock(options.profiling);
            sink(crop, &surface)?;
            if let Some(t) = &mut summary.timings {
                t.sink += elapsed(sink_started);
            }
            work.check()?;
            summary.delivered_tiles += 1;
        }
    }
    work.check()?;
    Ok(summary)
}

/// Paints immutable root layers into one local crop with no geometry/Boolean work allowance.
/// # Errors
/// Returns unchanged cancellation, integration, allocation, composition or quantization errors.
/// Optional phase clocks cover allocation/integration, composition and conversion only at coarse boundaries.
fn paint(
    model: Option<HalftoneChannelModel>,
    layers: &[PreparedAreaLayer],
    target: PreviewRasterTarget,
    crop: Crop,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    profiling: bool,
) -> Result<(RasterSurface, JobTimings), RenderError> {
    let region = crop.region(target)?;
    let mut work = RasterWork::with_progress(
        RasterizationLimits {
            max_flattened_edges: 0,
            max_boolean_operations: 0,
        },
        RasterAntialiasing::On,
        is_cancelled,
        0,
        &ignore_raster_progress,
    );
    work.check()?;
    let mut timing = JobTimings::default();
    let composition_started = clock(profiling);
    let mut pixels = match model {
        Some(m) => begin_model_composition(m, crop.width, crop.height)?,
        None => begin_source_over_composition(crop.width, crop.height)?,
    };
    timing.composition += elapsed(composition_started);
    for layer in layers {
        work.check()?;
        let integration_started = clock(profiling);
        let mut transient = allocate_linear_pixels(
            crop.width,
            crop.height,
            background_pixel(RasterBackground::Transparent),
        )?;
        layer.paint_region(
            &mut transient,
            target.width,
            target.height,
            region,
            &mut work,
        )?;
        timing.integration += elapsed(integration_started);
        let composition_started = clock(profiling);
        match model {
            Some(m) => compose_model_layer(m, &mut pixels, &transient, is_cancelled)?,
            None => compose_source_over_layer(&mut pixels, &transient, is_cancelled)?,
        }
        timing.composition += elapsed(composition_started);
    }
    let composition_started = clock(profiling);
    if let Some(m) = model {
        finish_model_composition(m, &mut pixels, is_cancelled)?;
    }
    apply_background(&mut pixels, target.background(), is_cancelled)?;
    timing.composition += elapsed(composition_started);
    let quantization_started = clock(profiling);
    let surface = pixels_from_linear(crop.width, crop.height, pixels, is_cancelled)?;
    timing.quantization += elapsed(quantization_started);
    Ok((surface, timing))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use toniator_domain::{CanvasSpec, PatternMechanismId};
    use toniator_geometry::{
        CanonicalPathMark, CurvePath, FamilySiteId, FamilySiteProvenance, PathClosure, SiteScope,
    };

    /// Prepares an off-small-viewport circle on a >64M-pixel virtual canvas without root pixels.
    /// # Panics
    /// Panics if small-target visibility leaks into full-root preparation or validation/budgets change.
    #[test]
    fn prepare_only_uses_whole_virtual_visibility_and_existing_resource_authority() {
        let circle = CanonicalMark::Circle {
            source_site_id: FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal: 0,
            },
            center: Point2::new(9000., 9000.),
            radius: 1.,
            scope: SiteScope::Canvas,
            provenance: FamilySiteProvenance::Random {
                candidate_ordinal: 0,
                accepted_ordinal: 0,
                exclusion_neighbor_ordinal: None,
            },
            fill_rule: CanonicalFillRule::NonZero,
        };
        let scene = RenderScene::new(
            CanvasSpec {
                width: 10000.,
                height: 10000.,
            },
            "virtual".into(),
            "circle".into(),
            vec![
                RenderLayer::new(
                    ChannelId(1),
                    true,
                    ColorValue {
                        red: 1.,
                        green: 0.,
                        blue: 0.,
                        alpha: 1.,
                    },
                    1.,
                    GeometryOutput::CanonicalMarks(vec![circle]),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let target = PreviewRasterTarget::for_viewport(1, 1, 0., 0., 1.).unwrap();
        let extent = VirtualExtent::new(10000, 10000).unwrap();
        assert!(PreviewRasterTarget::new(extent.width, extent.height).is_err());
        let limits = RasterizationLimits::default();
        for clip in [false, true] {
            let whole = prepare_only(&scene, target, extent, clip, limits, &|| false).unwrap();
            assert_eq!(whole.root_primitives, 1);
            assert!(whole.layers[0].normalized_edges > 0 && whole.layers[0].paints == 1);
            assert!(whole.consumed_flattened_edges > 0 && whole.consumed_boolean_operations > 0);
            let small = prepare_only(
                &scene,
                target,
                VirtualExtent::new(1, 1).unwrap(),
                clip,
                limits,
                &|| false,
            )
            .unwrap();
            assert_eq!(small.layers[0].normalized_edges, 0);
            let failure = prepare_only(
                &scene,
                target,
                extent,
                clip,
                limits.with_max_boolean_operations(1),
                &|| false,
            )
            .unwrap_err();
            assert_eq!(failure.error.path(), "raster.limits.boolean_operations");
            // The first checked Boolean batch is refused before spending its larger charge.
            assert_eq!(failure.facts.consumed_boolean_operations, 0);
            assert!(failure.facts.layers.is_empty());
        }
        let cancelled = prepare_only(&scene, target, extent, true, limits, &|| true).unwrap_err();
        assert_eq!(cancelled.error.path(), "evaluation.cancelled");
        assert_eq!(cancelled.facts.consumed_flattened_edges, 0);
        assert!(VirtualExtent::new(0, 1).is_err());
        assert_eq!(
            prepare_only(
                &scene,
                target,
                VirtualExtent {
                    width: 0,
                    height: 1
                },
                true,
                limits,
                &|| false
            )
            .unwrap_err()
            .error
            .path(),
            "raster.crop.frame"
        );
    }

    /// Cancels inside an already-prepared painter; zero edge/Boolean authority remains sufficient.
    /// The actual preparation is complete before the independently counted cancellation polls start.
    /// # Panics
    /// Panics on fixture/preparation failure or a painter that ignores an in-progress cancellation.
    #[test]
    fn prepared_painter_polls_cancellation_without_repreparing() {
        let mark = CanonicalMark::ClosedPath(
            CanonicalPathMark::new(
                FamilySiteId {
                    mechanism_id: PatternMechanismId(1),
                    ordinal: 0,
                },
                CurvePath::polyline(
                    vec![
                        Point2::new(0., 0.),
                        Point2::new(16., 0.),
                        Point2::new(16., 16.),
                        Point2::new(0., 16.),
                    ],
                    PathClosure::Closed,
                )
                .unwrap(),
                SiteScope::Canvas,
                FamilySiteProvenance::Random {
                    candidate_ordinal: 0,
                    accepted_ordinal: 0,
                    exclusion_neighbor_ordinal: None,
                },
                CanonicalFillRule::NonZero,
            )
            .unwrap(),
        );
        let scene = RenderScene::new(
            CanvasSpec {
                width: 16.,
                height: 16.,
            },
            "cancel-prepared".into(),
            "rectangle".into(),
            vec![
                RenderLayer::new(
                    ChannelId(1),
                    true,
                    ColorValue {
                        red: 1.,
                        green: 0.,
                        blue: 0.,
                        alpha: 1.,
                    },
                    1.,
                    GeometryOutput::CanonicalMarks(vec![mark]),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let target = PreviewRasterTarget::new(16, 16).unwrap();
        let transform = PreviewTransform::for_scene(&scene, target);
        let mut work = RasterWork::with_progress(
            RasterizationLimits::default(),
            RasterAntialiasing::On,
            &|| false,
            0,
            &ignore_raster_progress,
        );
        let layers = vec![
            area_raster::prepare_area_layer_with_clip(
                &scene.layers[0],
                16,
                16,
                CanonicalRasterTransform::preview(transform),
                Some([0., 0., 16., 16.]),
                &mut work,
            )
            .unwrap(),
        ];
        let checks = AtomicUsize::new(0);
        let error = paint(
            None,
            &layers,
            target,
            Crop::new(0, 0, 16, 16).unwrap(),
            &|| checks.fetch_add(1, Ordering::Relaxed) >= 8,
            false,
        )
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
        assert!(checks.load(Ordering::Relaxed) >= 9);
        assert!(
            paint(
                None,
                &layers,
                target,
                Crop::new(0, 0, 16, 16).unwrap(),
                &|| false,
                false
            )
            .is_ok()
        );
    }
}
