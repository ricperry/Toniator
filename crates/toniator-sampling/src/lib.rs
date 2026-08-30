#![forbid(unsafe_code)]

//! Byte-oriented source decoding and deterministic source-field sampling.

use std::{
    error::Error,
    fmt,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use image::{ColorType, ImageEncoder, ImageFormat, ImageReader, codecs::png::PngEncoder};
use rayon::prelude::*;
use resvg::{tiny_skia, usvg};
use serde::Serialize;
use sha2::{Digest, Sha256};
use toniator_domain::CanvasSpec;
pub use toniator_domain::{
    SourceComponent, SourceMapping, SourceMappingComponent, SourcePlacement,
};
use toniator_geometry::{CurvePath, CurveSegment, Point2};

const MAX_SOURCE_PIXELS: u64 = 64 * 1024 * 1024;

/// Versioned identity for the decoder behavior that participates in derived
/// cache keys. Bump it whenever decoding can yield different source pixels for
/// the same bytes and format hint.
pub const DECODER_CONTRACT_ID: &str =
    "toniator-sampling-decoder-v3-still-image-linear-source-fields";

/// The supported single-still source formats at the sampling boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceFormat {
    Png,
    Svg,
    Jpeg,
    Webp,
    Bmp,
    Tiff,
    OpenExr,
    Avif,
}

/// A caller-supplied decoding hint. Decoding never opens a filesystem path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFormatHint {
    Png,
    Svg,
    Jpeg,
    Webp,
    Bmp,
    Tiff,
    OpenExr,
    Avif,
    Unsupported,
}

/// Decoded straight-sRGB color and independent normalized alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourcePixel {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

/// Stores a deterministic PNG proxy decoded from the supplied source for a private preview.
///
/// The proxy is derived only from decoded source pixels and preserves their aspect ratio and
/// alpha coverage. It is presentation-only: callers retain the original source reference and
/// must never substitute these bytes into a document, export, or main evaluation request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReducedPreviewSource {
    /// Contains the supported PNG byte stream submitted to the canonical evaluator.
    pub png_bytes: Vec<u8>,
    /// Identifies the deterministic proxy width after long-edge reduction.
    pub width: u32,
    /// Identifies the deterministic proxy height after long-edge reduction.
    pub height: u32,
}

/// A straight linear-light source color associated with a mark response.
///
/// `alpha` is always one for a present paint. An absent paint is represented
/// by [`SourceColorSample::paint`] being `None`, which makes exact-zero alpha
/// suppression explicit instead of encoding it as transparent paint.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct SampledSourcePaint {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

/// The independently sampled mark response and evaluated SourceColor paint.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct SourceColorSample {
    /// The mapping-derived scalar response. For the canonical SourceColor
    /// mapping this is source alpha, applied exactly once to mark size.
    pub response: f64,
    /// Straight linear source paint for a positive sampled alpha, or `None`
    /// for an exact-zero alpha sample.
    pub paint: Option<SampledSourcePaint>,
}

/// Bounded work configuration for deterministic region sampling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionSamplingLimits {
    /// Caps decoded-source pixel-footprint classification work for a complete request.
    pub max_cell_intersections: usize,
    /// Caps flattened boundary segments for a complete request.
    pub max_flattened_segments: usize,
    /// Caps deterministic cubic subdivision depth.
    pub max_subdivision_depth: usize,
}

impl Default for RegionSamplingLimits {
    /// Supplies the approved Stage 20Q request-wide sampling limits.
    fn default() -> Self {
        Self {
            max_cell_intersections: 33_554_432,
            max_flattened_segments: 8_388_608,
            max_subdivision_depth: 48,
        }
    }
}

/// One scalar response and optional associated source paint for a base region.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionSourceSample {
    /// Supplies the mapped scalar that interpolates a treatment response.
    pub response: f64,
    /// Supplies sampled paint only for positive alpha, suppressing hidden RGB at exact zero.
    pub paint: Option<SampledSourcePaint>,
}

/// Non-authoritative request work counts for one completed AreaAverage batch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionSamplingDiagnostics {
    /// Counts emitted deterministic flattened boundary chords across all sampled regions.
    pub flattened_segments: usize,
    /// Counts candidate literal source-pixel footprints across all sampled regions.
    pub cell_intersections: usize,
}

/// SVG-specific decoder behavior surfaced to headless diagnostics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SvgTextDiagnostic {
    pub has_live_text_node: bool,
    pub font_policy: String,
    pub rendered_glyph_coverage: bool,
}

/// Identity and decoding diagnostics retained with the immutable field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceIdentity {
    pub format: SourceFormat,
    pub width: u32,
    pub height: u32,
    pub content_hash: String,
    /// Hash of the decoded sampling pixels, including SVG font resolution.
    pub decoded_pixel_hash: String,
    pub svg_text: Option<SvgTextDiagnostic>,
}

/// Immutable decoded pixels with deterministic point sampling.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceField {
    identity: SourceIdentity,
    pixels: Vec<SourcePixel>,
}

impl SourceField {
    pub fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<SourcePixel> {
        (x < self.identity.width && y < self.identity.height)
            .then(|| self.pixels[y as usize * self.identity.width as usize + x as usize])
    }

    /// Bilinearly samples a raw normalized component with edge clamping.
    ///
    /// This retains the independently inspectable source-component contract.
    /// Realization must instead call [`Self::sample_mark_ink`] so color-derived
    /// response is associated with alpha before interpolation.
    pub fn sample(
        &self,
        point: Point2,
        canvas: &CanvasSpec,
        placement: SourcePlacement,
        component: SourceComponent,
    ) -> Result<f64, SamplingError> {
        validate_canvas(canvas)?;
        if !point.is_finite() {
            return Err(SamplingError::new("sampling.point", "point must be finite"));
        }
        match placement {
            SourcePlacement::StretchToCanvas => {
                self.sample_stretch_with(point, canvas, |pixel| component_value(pixel, component))
            }
        }
    }

    /// Bilinearly samples the effective mark-ink response used by canonical
    /// circle realization. Color-derived ink is alpha-associated per source
    /// sample before interpolation; Alpha remains an independent response.
    pub fn sample_mark_ink(
        &self,
        point: Point2,
        canvas: &CanvasSpec,
        placement: SourcePlacement,
        component: SourceComponent,
    ) -> Result<f64, SamplingError> {
        validate_canvas(canvas)?;
        if !point.is_finite() {
            return Err(SamplingError::new("sampling.point", "point must be finite"));
        }
        match placement {
            SourcePlacement::StretchToCanvas => self
                .sample_stretch_with(point, canvas, |pixel| effective_mark_ink(pixel, component)),
        }
    }

    /// Samples the decoder-owned scalar field used by structural
    /// artwork-weighted site placement.  It intentionally reuses the
    /// authoritative mapped-response interpolation without adding source
    /// decoding, identity, or placement policy at the pattern layer.
    pub fn sample_density_weight(
        &self,
        point: Point2,
        canvas: &CanvasSpec,
        mapping: SourceMapping,
    ) -> Result<f64, SamplingError> {
        self.sample_mapping_response(point, canvas, mapping)
    }

    /// Samples a complete Stage 9 mapping. Color-derived fields are converted
    /// from straight sRGB to linear light, transformed, then associated with
    /// source alpha exactly once before interpolation. Alpha remains an
    /// independent transformed scalar.
    pub fn sample_mapping_response(
        &self,
        point: Point2,
        canvas: &CanvasSpec,
        mapping: SourceMapping,
    ) -> Result<f64, SamplingError> {
        validate_canvas(canvas)?;
        validate_mapping(mapping)?;
        if !point.is_finite() {
            return Err(SamplingError::new("sampling.point", "point must be finite"));
        }
        match mapping.placement {
            SourcePlacement::StretchToCanvas => {
                self.sample_stretch_with(point, canvas, |pixel| mapped_response(pixel, mapping))
            }
        }
    }

    /// Samples SourceColorAlpha's associated linear RGB and independent alpha.
    ///
    /// The returned paint is straight linear and fully opaque when source alpha
    /// is positive. At exactly zero alpha it is absent, so a nonzero minimum
    /// mark size cannot expose hidden RGB or a transparent paint fringe.
    pub fn sample_source_color(
        &self,
        point: Point2,
        canvas: &CanvasSpec,
        mapping: SourceMapping,
    ) -> Result<SourceColorSample, SamplingError> {
        validate_canvas(canvas)?;
        validate_mapping(mapping)?;
        if !point.is_finite() {
            return Err(SamplingError::new("sampling.point", "point must be finite"));
        }
        match mapping.placement {
            SourcePlacement::StretchToCanvas => {
                let (red, green, blue, alpha) =
                    self.sample_stretch_associated_rgb(point, canvas)?;
                let paint = (alpha > 0.0).then(|| SampledSourcePaint {
                    red: (red / alpha).clamp(0.0, 1.0),
                    green: (green / alpha).clamp(0.0, 1.0),
                    blue: (blue / alpha).clamp(0.0, 1.0),
                    alpha: 1.0,
                });
                let response = self.sample_mapping_response(point, canvas, mapping)?;
                Ok(SourceColorSample { response, paint })
            }
        }
    }

    fn sample_stretch_with<F>(
        &self,
        point: Point2,
        canvas: &CanvasSpec,
        value: F,
    ) -> Result<f64, SamplingError>
    where
        F: Fn(SourcePixel) -> f64,
    {
        let x = map_axis(point.x, canvas.width, self.identity.width);
        let y = map_axis(point.y, canvas.height, self.identity.height);
        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(self.identity.width - 1);
        let y1 = (y0 + 1).min(self.identity.height - 1);
        let tx = x - f64::from(x0);
        let ty = y - f64::from(y0);
        let sampled_value = |x, y| value(self.pixel(x, y).expect("mapped pixel"));
        let top = sampled_value(x0, y0).mul_add(1.0 - tx, sampled_value(x1, y0) * tx);
        let bottom = sampled_value(x0, y1).mul_add(1.0 - tx, sampled_value(x1, y1) * tx);
        let sampled = top.mul_add(1.0 - ty, bottom * ty);
        if sampled.is_finite() {
            Ok(sampled.clamp(0.0, 1.0))
        } else {
            Err(SamplingError::new(
                "sampling.value",
                "sampled value must be finite",
            ))
        }
    }

    fn sample_stretch_associated_rgb(
        &self,
        point: Point2,
        canvas: &CanvasSpec,
    ) -> Result<(f64, f64, f64, f64), SamplingError> {
        let x = map_axis(point.x, canvas.width, self.identity.width);
        let y = map_axis(point.y, canvas.height, self.identity.height);
        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(self.identity.width - 1);
        let y1 = (y0 + 1).min(self.identity.height - 1);
        let tx = x - f64::from(x0);
        let ty = y - f64::from(y0);
        let sample = |x, y| associated_linear(self.pixel(x, y).expect("mapped pixel"));
        let interpolate = |index: usize| {
            let top = sample(x0, y0)[index].mul_add(1.0 - tx, sample(x1, y0)[index] * tx);
            let bottom = sample(x0, y1)[index].mul_add(1.0 - tx, sample(x1, y1)[index] * tx);
            top.mul_add(1.0 - ty, bottom * ty)
        };
        let sampled = (
            interpolate(0),
            interpolate(1),
            interpolate(2),
            interpolate(3),
        );
        if [sampled.0, sampled.1, sampled.2, sampled.3]
            .into_iter()
            .all(f64::is_finite)
        {
            Ok((
                sampled.0.clamp(0.0, 1.0),
                sampled.1.clamp(0.0, 1.0),
                sampled.2.clamp(0.0, 1.0),
                sampled.3.clamp(0.0, 1.0),
            ))
        } else {
            Err(SamplingError::new(
                "sampling.value",
                "sampled value must be finite",
            ))
        }
    }
}

/// Samples one producer-owned reference point through the existing mapped and associated-color authority.
///
/// # Errors
///
/// Propagates only source/canvas/mapping failures and never derives geometry or caches at this boundary.
pub fn sample_region_reference(
    field: &SourceField,
    reference: Point2,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
) -> Result<RegionSourceSample, SamplingError> {
    let source = field.sample_source_color(reference, canvas, mapping)?;
    Ok(RegionSourceSample {
        response: source.response,
        paint: source.paint,
    })
}

/// Selects and equally averages full decoded pixels for one complete untreated closed region.
///
/// A literal source-pixel footprint contributes its complete mapped scalar and associated RGBA
/// exactly once when exact polygon overlap is at least `50%`; smaller overlap contributes nothing.
/// Off-source unit footprints retain their edge-clamped decoded value. This is not continuous
/// bilinear integration and never substitutes pixel-center inclusion.
///
/// This convenience boundary owns one request-wide budget for the single region. Callers that
/// sample multiple base regions must use [`sample_region_area_average_batch`] so their work shares
/// the same limits.
///
/// # Errors
///
/// Returns stable geometry, limit, sampling, or cancellation failures without a partial sample.
pub fn sample_region_area_average(
    field: &SourceField,
    region: &CurvePath,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    limits: RegionSamplingLimits,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<RegionSourceSample, SamplingError> {
    validate_canvas(canvas)?;
    validate_mapping(mapping)?;
    let work = RegionSamplingWork::new(limits, cancelled)?;
    sample_region_area_average_with_work(field, region, canvas, mapping, &work)
}

/// Selects and equally averages full decoded pixels for every untreated base region with one shared work budget.
///
/// Each result uses exact `>=50%` literal footprint selection and equal full-value averaging,
/// including edge-clamped exterior unit footprints.
///
/// Results retain input order and are returned only if every region completes, so failures and
/// cancellation never leak a partially aligned paint/sample table.
///
/// # Errors
///
/// Returns the first stable sampling, geometry, allocation, limit, or cancellation failure.
pub fn sample_region_area_average_batch(
    field: &SourceField,
    regions: &[CurvePath],
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    limits: RegionSamplingLimits,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<RegionSourceSample>, SamplingError> {
    sample_region_area_average_batch_with_diagnostics(
        field, regions, canvas, mapping, limits, cancelled,
    )
    .map(|(samples, _)| samples)
}

/// Samples one ordered literal-footprint AreaAverage batch and reports lightweight request work counts.
///
/// Diagnostics are produced from the same shared atomic budget counters, remain outside source
/// identity and persistence, and are returned only after every indexed result succeeds.
///
/// # Errors
///
/// Returns the first stable ordered sampling, geometry, allocation, limit, or cancellation
/// failure without returning samples or partial diagnostics.
pub fn sample_region_area_average_batch_with_diagnostics(
    field: &SourceField,
    regions: &[CurvePath],
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    limits: RegionSamplingLimits,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(Vec<RegionSourceSample>, RegionSamplingDiagnostics), SamplingError> {
    sample_region_area_average_batch_with_diagnostics_impl(
        field, regions, canvas, mapping, limits, cancelled, None,
    )
}

/// Samples one ordered AreaAverage batch while reporting completed candidate-pixel-footprint work.
///
/// Progress is observational and per-mille coalesced before invoking the callback, so parallel
/// footprint classification cannot flood a frontend queue or alter deterministic sample ordering.
///
/// # Errors
///
/// Returns the same stable failures as [`sample_region_area_average_batch_with_diagnostics`]
/// without returning partial samples or treating progress as publication authority.
#[allow(clippy::too_many_arguments)]
pub fn sample_region_area_average_batch_with_diagnostics_and_progress(
    field: &SourceField,
    regions: &[CurvePath],
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    limits: RegionSamplingLimits,
    cancelled: &(dyn Fn() -> bool + Sync),
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<(Vec<RegionSourceSample>, RegionSamplingDiagnostics), SamplingError> {
    sample_region_area_average_batch_with_diagnostics_impl(
        field,
        regions,
        canvas,
        mapping,
        limits,
        cancelled,
        Some(progress),
    )
}

/// Implements ordered exact-footprint AreaAverage sampling with an optional observational progress sink.
///
/// # Errors
///
/// Returns stable validation, geometry, allocation, limit, or cancellation failures atomically.
#[allow(clippy::too_many_arguments)]
fn sample_region_area_average_batch_with_diagnostics_impl(
    field: &SourceField,
    regions: &[CurvePath],
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    limits: RegionSamplingLimits,
    cancelled: &(dyn Fn() -> bool + Sync),
    progress: Option<&(dyn Fn(usize, usize) + Sync)>,
) -> Result<(Vec<RegionSourceSample>, RegionSamplingDiagnostics), SamplingError> {
    validate_canvas(canvas)?;
    validate_mapping(mapping)?;
    let work = RegionSamplingWork::new(limits, cancelled)?;
    let prepared = regions
        .iter()
        .map(|region| prepare_region_area_average(field, region, canvas, &work))
        .collect::<Result<Vec<_>, _>>()?;
    let progress = RegionSamplingProgress::new(work.diagnostics().cell_intersections, progress);
    let results = prepared
        .par_iter()
        .map_init(RegionClipScratch::default, |scratch, region| {
            integrate_prepared_region_average(field, region, mapping, &work, &progress, scratch)
        })
        .collect::<Vec<_>>();
    let samples = results.into_iter().collect::<Result<Vec<_>, _>>()?;
    Ok((samples, work.diagnostics()))
}

/// Selects full decoded source pixels whose footprints are at least half covered by one region.
///
/// The caller owns `work`, which makes every flattening and footprint charge request-wide. Scalar
/// response and associated RGB/alpha use the same exact polygon-footprint classification. Each
/// included pixel contributes its full value once; fractional coverage never multiplies a value.
///
/// # Errors
///
/// Returns stable sampling failures and never exposes a partially accumulated sample.
fn sample_region_area_average_with_work(
    field: &SourceField,
    region: &CurvePath,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    work: &RegionSamplingWork<'_>,
) -> Result<RegionSourceSample, SamplingError> {
    let prepared = prepare_region_area_average(field, region, canvas, work)?;
    let progress = RegionSamplingProgress::new(work.diagnostics().cell_intersections, None);
    integrate_prepared_region_average(
        field,
        &prepared,
        mapping,
        work,
        &progress,
        &mut RegionClipScratch::default(),
    )
}

/// Prepares one region and charges request budgets in authoritative input order.
///
/// Flattening, geometry validation, source-footprint run planning, and candidate-footprint charges stay
/// serial across a batch. This makes bounded failures identical for every worker count while the
/// independent footprint classifications remain eligible for indexed parallel execution.
///
/// # Errors
///
/// Returns the first ordered geometry, allocation, cancellation, or aggregate-limit failure.
fn prepare_region_area_average(
    field: &SourceField,
    region: &CurvePath,
    canvas: &CanvasSpec,
    work: &RegionSamplingWork<'_>,
) -> Result<PreparedRegionAverage, SamplingError> {
    let polygon = flatten_region_source_space_with_work(
        region,
        canvas,
        field.identity.width,
        field.identity.height,
        work,
    )?;
    let complete_area = polygon_signed_area(&polygon);
    if !complete_area.is_finite() || complete_area.abs() <= f64::EPSILON {
        return Err(SamplingError::new(
            "sampling.region_average.geometry",
            "region average requires nonzero finite area",
        ));
    }
    let (minimum, maximum) = polygon_bounds(&polygon)?;
    let xs = SourceAxisPartitions::new(minimum.x, maximum.x, field.identity.width)?;
    let ys = SourceAxisPartitions::new(minimum.y, maximum.y, field.identity.height)?;
    let candidate_count = xs.count()?.checked_mul(ys.count()?).ok_or_else(|| {
        SamplingError::new(
            "sampling.region_average.limits.cell_intersections",
            "source footprint count is unsafe",
        )
    })?;
    work.poll()?;
    work.charge_cells(candidate_count)?;
    Ok(PreparedRegionAverage { polygon, xs, ys })
}

/// Classifies one fully budgeted region’s literal pixel footprints without mutating workload counters.
///
/// Every candidate footprint is clipped exactly, selected only at `>=50%` coverage, and contributes
/// its complete decoded value once to the equal average.
///
/// # Errors
///
/// Returns canonical cancellation without publishing a partial sample.
fn integrate_prepared_region_average(
    field: &SourceField,
    prepared: &PreparedRegionAverage,
    mapping: SourceMapping,
    work: &RegionSamplingWork<'_>,
    progress: &RegionSamplingProgress<'_>,
    scratch: &mut RegionClipScratch,
) -> Result<RegionSourceSample, SamplingError> {
    let mut totals = [0.0; 5];
    let mut included = 0usize;
    for y in prepared.ys.iter() {
        for x in prepared.xs.iter() {
            work.poll()?;
            clip_polygon_to_rect_into_cancellable(
                &prepared.polygon,
                x.start,
                x.end,
                y.start,
                y.end,
                work.cancelled,
                &mut scratch.clipped,
                &mut scratch.intermediate,
            )?;
            if !scratch.clipped.is_empty() {
                let coverage = polygon_signed_area(&scratch.clipped).abs();
                if coverage >= 0.5 {
                    let pixel = field
                        .pixel(x.cell, y.cell)
                        .expect("validated source pixel footprint");
                    totals[0] += mapped_response(pixel, mapping);
                    for (total, value) in totals[1..].iter_mut().zip(associated_linear(pixel)) {
                        *total += value;
                    }
                    included = included.saturating_add(1);
                }
            }
            progress.complete_cell();
        }
    }
    if included == 0 {
        return Ok(RegionSourceSample {
            response: 0.0,
            paint: None,
        });
    }
    let count = included as f64;
    let response = (totals[0] / count).clamp(0.0, 1.0);
    let alpha = (totals[4] / count).clamp(0.0, 1.0);
    let paint = (alpha > 0.0).then(|| SampledSourcePaint {
        red: (totals[1] / count / alpha).clamp(0.0, 1.0),
        green: (totals[2] / count / alpha).clamp(0.0, 1.0),
        blue: (totals[3] / count / alpha).clamp(0.0, 1.0),
        alpha: 1.0,
    });
    Ok(RegionSourceSample { response, paint })
}

/// One ordered region after deterministic source-space preparation and budget accounting.
struct PreparedRegionAverage {
    polygon: Vec<Point2>,
    xs: SourceAxisPartitions,
    ys: SourceAxisPartitions,
}

/// Reuses both Sutherland-Hodgman buffers across every source-pixel footprint handled by one worker.
#[derive(Default)]
struct RegionClipScratch {
    clipped: Vec<Point2>,
    intermediate: Vec<Point2>,
}

/// Coalesces parallel completed-footprint observations before invoking a caller progress sink.
struct RegionSamplingProgress<'a> {
    total: usize,
    completed: AtomicUsize,
    reported_per_mille: AtomicUsize,
    callback_lock: Mutex<()>,
    callback: Option<&'a (dyn Fn(usize, usize) + Sync)>,
}

impl<'a> RegionSamplingProgress<'a> {
    /// Creates an observational progress counter over the complete prepared footprint workload.
    fn new(total: usize, callback: Option<&'a (dyn Fn(usize, usize) + Sync)>) -> Self {
        Self {
            total,
            completed: AtomicUsize::new(0),
            reported_per_mille: AtomicUsize::new(0),
            callback_lock: Mutex::new(()),
            callback,
        }
    }

    /// Records one completed footprint and publishes only the first observation of each per-mille.
    fn complete_cell(&self) {
        let Some(callback) = self.callback else {
            return;
        };
        let completed = self
            .completed
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1)
            .min(self.total);
        let per_mille = completed.saturating_mul(1_000) / self.total.max(1);
        if self
            .reported_per_mille
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |reported| {
                (per_mille > reported).then_some(per_mille)
            })
            .is_ok()
            && let Ok(_guard) = self.callback_lock.lock()
            && self.reported_per_mille.load(Ordering::Relaxed) == per_mille
        {
            callback(per_mille, 1_000);
        }
    }
}

/// Flattens one closed region into unclamped decoded-source coordinates.
///
/// Canvas edges map to decoded pixel edges: canvas `0..width` becomes source
/// `-0.5..width-0.5`. The transform preserves off-source geometry for later unit edge-clamped
/// footprint classification. Cubics use ordered `t = 0.5` De Casteljau subdivision with a
/// `1/64` pixel chord tolerance and never append a duplicate closure point.
///
/// # Errors
///
/// Returns stable geometry, allocation, flattening-limit, or cancellation diagnostics without
/// exposing a partially flattened candidate.
#[allow(dead_code)]
fn flatten_region_source_space(
    region: &CurvePath,
    canvas: &CanvasSpec,
    source_width: u32,
    source_height: u32,
    limits: RegionSamplingLimits,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<Point2>, SamplingError> {
    let work = RegionSamplingWork::new(limits, cancelled)?;
    flatten_region_source_space_with_work(region, canvas, source_width, source_height, &work)
}

/// Flattens one closed boundary into source space while charging the caller-owned request budget.
///
/// Source dimensions determine the pixel-edge transform; no pixel data is read before exact
/// footprint classification. Off-canvas coordinates remain unclamped so every exterior unit
/// footprint retains its edge-clamped decoded value.
///
/// # Errors
///
/// Returns geometry, allocation, cancellation, or request-wide flattening-limit diagnostics.
fn flatten_region_source_space_with_work(
    region: &CurvePath,
    canvas: &CanvasSpec,
    source_width: u32,
    source_height: u32,
    work: &RegionSamplingWork<'_>,
) -> Result<Vec<Point2>, SamplingError> {
    if region.closure() != toniator_geometry::PathClosure::Closed {
        return Err(SamplingError::new(
            "sampling.region_average.geometry",
            "area averaging requires a closed region",
        ));
    }
    validate_canvas(canvas)?;
    let map = |point: Point2| {
        Point2::new(
            point.x * f64::from(source_width) / canvas.width - 0.5,
            point.y * f64::from(source_height) / canvas.height - 0.5,
        )
    };
    let mut output = Vec::new();
    output.try_reserve(region.segments().len()).map_err(|_| {
        SamplingError::new(
            "sampling.region_average.allocation.flattening",
            "source-space flattening allocation failed",
        )
    })?;
    for segment in region.segments() {
        work.poll()?;
        match segment {
            CurveSegment::Line(line) => push_flattened_point(&mut output, map(line.start()), work)?,
            CurveSegment::CubicBezier(cubic) => flatten_cubic_source_space(
                map(cubic.start()),
                map(cubic.control_1()),
                map(cubic.control_2()),
                map(cubic.end()),
                0,
                work,
                &mut output,
            )?,
        }
    }
    if output.len() < 3 {
        return Err(SamplingError::new(
            "sampling.region_average.geometry",
            "flattened region requires at least three vertices",
        ));
    }
    Ok(output)
}

/// Emits one cubic's ordered source-space chords through deterministic De Casteljau bisection.
///
/// # Errors
///
/// Returns stable cancellation, depth, or flattened-segment limit diagnostics.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
fn flatten_cubic_source_space(
    start: Point2,
    control_1: Point2,
    control_2: Point2,
    end: Point2,
    depth: usize,
    work: &RegionSamplingWork<'_>,
    output: &mut Vec<Point2>,
) -> Result<(), SamplingError> {
    work.poll()?;
    if cubic_source_flat_enough(start, control_1, control_2, end) {
        return push_flattened_point(output, start, work);
    }
    if depth >= work.limits.max_subdivision_depth {
        return Err(SamplingError::new(
            "sampling.region_average.limits.flattening",
            "cubic subdivision depth limit exceeded",
        ));
    }
    let midpoint = |left: Point2, right: Point2| {
        Point2::new((left.x + right.x) / 2.0, (left.y + right.y) / 2.0)
    };
    let a = midpoint(start, control_1);
    let b = midpoint(control_1, control_2);
    let c = midpoint(control_2, end);
    let d = midpoint(a, b);
    let e = midpoint(b, c);
    let middle = midpoint(d, e);
    flatten_cubic_source_space(start, a, d, middle, depth + 1, work, output)?;
    flatten_cubic_source_space(middle, e, c, end, depth + 1, work, output)
}

/// Tests cubic controls against the source-space endpoint chord at the fixed `1/64` tolerance.
#[allow(dead_code)]
fn cubic_source_flat_enough(
    start: Point2,
    control_1: Point2,
    control_2: Point2,
    end: Point2,
) -> bool {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = dx.hypot(dy);
    if length == 0.0 {
        return (control_1.x - start.x)
            .hypot(control_1.y - start.y)
            .max((control_2.x - start.x).hypot(control_2.y - start.y))
            <= 1.0 / 64.0;
    }
    let distance =
        |point: Point2| ((point.x - start.x) * dy - (point.y - start.y) * dx).abs() / length;
    distance(control_1).max(distance(control_2)) <= 1.0 / 64.0
}

/// Appends one source-space vertex while enforcing the request-wide chord limit and join uniqueness.
#[allow(dead_code)]
fn push_flattened_point(
    output: &mut Vec<Point2>,
    point: Point2,
    work: &RegionSamplingWork<'_>,
) -> Result<(), SamplingError> {
    if !point.is_finite() {
        return Err(SamplingError::new(
            "sampling.region_average.geometry",
            "source-space flattening produced nonfinite geometry",
        ));
    }
    if output.last().copied() == Some(point) {
        return Ok(());
    }
    work.charge_flattened()?;
    output.try_reserve(1).map_err(|_| {
        SamplingError::new(
            "sampling.region_average.allocation.flattening",
            "source-space flattening allocation failed",
        )
    })?;
    output.push(point);
    Ok(())
}

/// Stores exact signed polygon area and raw first/mixed moments in source coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
struct PolygonMoments {
    area: f64,
    mx: f64,
    my: f64,
    mxy: f64,
}

/// Tracks request-wide flattened-segment and pixel-footprint work without publishing samples.
#[allow(dead_code)]
struct RegionSamplingWork<'a> {
    limits: RegionSamplingLimits,
    cancelled: &'a (dyn Fn() -> bool + Sync),
    flattened_segments: AtomicUsize,
    cell_intersections: AtomicUsize,
}

#[allow(dead_code)]
impl<'a> RegionSamplingWork<'a> {
    /// Builds a shared nonzero work budget for a complete region-sampling request.
    fn new(
        limits: RegionSamplingLimits,
        cancelled: &'a (dyn Fn() -> bool + Sync),
    ) -> Result<Self, SamplingError> {
        if limits.max_flattened_segments == 0
            || limits.max_cell_intersections == 0
            || limits.max_subdivision_depth == 0
        {
            return Err(SamplingError::new(
                "sampling.region_average.limits.flattening",
                "region sampling limits must be nonzero",
            ));
        }
        Ok(Self {
            limits,
            cancelled,
            flattened_segments: AtomicUsize::new(0),
            cell_intersections: AtomicUsize::new(0),
        })
    }
    /// Polls cancellation using the canonical evaluation failure path.
    fn poll(&self) -> Result<(), SamplingError> {
        poll_region(self.cancelled)
    }
    /// Snapshots completed aggregate work after every ordered result succeeds.
    fn diagnostics(&self) -> RegionSamplingDiagnostics {
        RegionSamplingDiagnostics {
            flattened_segments: self.flattened_segments.load(Ordering::Relaxed),
            cell_intersections: self.cell_intersections.load(Ordering::Relaxed),
        }
    }
    /// Charges one emitted flattened chord across every region using this request.
    fn charge_flattened(&self) -> Result<(), SamplingError> {
        self.flattened_segments
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (current < self.limits.max_flattened_segments).then_some(current + 1)
            })
            .map(|_| ())
            .map_err(|_| {
                SamplingError::new(
                    "sampling.region_average.limits.flattening",
                    "flattened segment limit exceeded",
                )
            })
    }
    /// Charges one candidate pixel-footprint classification across every region using this request.
    fn charge_cell(&self) -> Result<(), SamplingError> {
        self.charge_cells(1)
    }
    /// Charges an already bounded number of candidate source footprints atomically.
    ///
    /// The caller computes this count before allocating or iterating a footprint run, so an
    /// oversized complete untreated region fails through the ordinary request-wide limit without
    /// exposing partial sampling work.
    fn charge_cells(&self, count: usize) -> Result<(), SamplingError> {
        self.cell_intersections
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current
                    .checked_add(count)
                    .filter(|next| *next <= self.limits.max_cell_intersections)
            })
            .map(|_| ())
            .map_err(|_| {
                SamplingError::new(
                    "sampling.region_average.limits.cell_intersections",
                    "pixel-footprint classification limit exceeded",
                )
            })
    }
}

/// One finite source-space pixel-footprint partition paired with its edge-clamped decoded pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
struct SourceAxisInterval {
    start: f64,
    end: f64,
    cell: u32,
}

/// Represents a finite run of literal decoded-pixel footprints without allocating one entry per pixel.
///
/// Footprints retain integer-centered source coordinates and clamp their decoded-pixel lookup at
/// the source edge. The run never supplies values itself; callers must visit every interval so
/// binary inclusion retains the required exterior edge-pixel multiplicity.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SourceAxisPartitions {
    minimum: f64,
    maximum: f64,
    first: i64,
    last: i64,
    extent: u32,
}

impl SourceAxisPartitions {
    /// Plans all intersected unit pixel footprints for one finite source-space axis.
    ///
    /// # Errors
    ///
    /// Returns the established region geometry diagnostic for nonfinite, inverted, or unbounded
    /// axis input. It does not allocate the run or charge evaluation work.
    fn new(minimum: f64, maximum: f64, extent: u32) -> Result<Self, SamplingError> {
        if !minimum.is_finite() || !maximum.is_finite() || maximum < minimum || extent == 0 {
            return Err(SamplingError::new(
                "sampling.region_average.geometry",
                "source interval bounds must be finite and ordered",
            ));
        }
        let first_value = (minimum + 0.5).floor();
        let last_edge = (maximum + 0.5).ceil();
        let i64_min = -(2_f64.powi(63));
        let i64_upper_exclusive = 2_f64.powi(63);
        if !first_value.is_finite()
            || !last_edge.is_finite()
            || first_value < i64_min
            || first_value >= i64_upper_exclusive
            || last_edge < i64_min
            || last_edge >= i64_upper_exclusive
        {
            return Err(SamplingError::new(
                "sampling.region_average.limits.cell_intersections",
                "source footprint range is unsafe",
            ));
        }
        let first = first_value as i64;
        let last = (last_edge as i64).checked_sub(1).ok_or_else(|| {
            SamplingError::new(
                "sampling.region_average.limits.cell_intersections",
                "source footprint range is unsafe",
            )
        })?;
        Ok(Self {
            minimum,
            maximum,
            first,
            last,
            extent,
        })
    }

    /// Returns the exact finite count of candidate literal footprints without allocation.
    fn count(self) -> Result<usize, SamplingError> {
        let count = self
            .last
            .checked_sub(self.first)
            .and_then(|value| value.checked_add(1))
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                SamplingError::new(
                    "sampling.region_average.limits.cell_intersections",
                    "source footprint range is unsafe",
                )
            })?;
        Ok(count)
    }

    /// Iterates exact clipped unit footprints in ascending source-coordinate order.
    fn iter(self) -> impl Iterator<Item = SourceAxisInterval> {
        (self.first..=self.last).map(move |index| SourceAxisInterval {
            start: self.minimum.max(index as f64 - 0.5),
            end: self.maximum.min(index as f64 + 0.5),
            cell: index.clamp(0, i64::from(self.extent - 1)) as u32,
        })
    }
}

/// Enumerates low exterior, ordered decoded-pixel footprints, then high exterior for one source axis.
///
/// Source coordinates identify pixel centers, so each literal pixel footprint spans
/// `center - 0.5 .. center + 0.5`. Exterior partitions retain one unit footprint per source
/// coordinate and clamp each decoded lookup to the nearest edge pixel.
///
/// # Errors
///
/// Returns stable geometry diagnostics for nonfinite/inverted ranges or a zero source extent.
#[allow(dead_code)]
fn source_axis_intervals(
    minimum: f64,
    maximum: f64,
    extent: u32,
) -> Result<Vec<SourceAxisInterval>, SamplingError> {
    let partitions = SourceAxisPartitions::new(minimum, maximum, extent)?;
    let mut result = Vec::new();
    result.try_reserve(partitions.count()?).map_err(|_| {
        SamplingError::new(
            "sampling.region_average.allocation.partition",
            "source interval allocation failed",
        )
    })?;
    result.extend(partitions.iter());
    Ok(result)
}

/// Computes finite source-space bounds for one already flattened polygon.
///
/// # Errors
///
/// Returns the stable geometry diagnostic when a flattened point is nonfinite.
fn polygon_bounds(points: &[Point2]) -> Result<(Point2, Point2), SamplingError> {
    let mut minimum = Point2::new(f64::INFINITY, f64::INFINITY);
    let mut maximum = Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for point in points {
        if !point.is_finite() {
            return Err(SamplingError::new(
                "sampling.region_average.geometry",
                "flattened region bounds must be finite",
            ));
        }
        minimum.x = minimum.x.min(point.x);
        minimum.y = minimum.y.min(point.y);
        maximum.x = maximum.x.max(point.x);
        maximum.y = maximum.y.max(point.y);
    }
    Ok((minimum, maximum))
}

/// Polls the shared cancellation callback and uses the canonical evaluation diagnostic.
#[allow(dead_code)]
fn poll_region(cancelled: &dyn Fn() -> bool) -> Result<(), SamplingError> {
    (!cancelled()).then_some(()).ok_or(SamplingError::new(
        "evaluation.cancelled",
        "evaluation cancelled",
    ))
}

/// Clips one finite polygon to a rectangle in fixed left/right/bottom/top order.
///
/// Adjacent duplicate vertices and zero-area output are discarded so callers never integrate a
/// boundary-touch sliver. Cancellation is polled at each clip edge and source edge.
///
/// # Errors
///
/// Returns `evaluation.cancelled` without exposing a partially clipped polygon.
#[allow(dead_code)]
fn clip_polygon_to_rect_cancellable(
    polygon: &[Point2],
    left: f64,
    right: f64,
    bottom: f64,
    top: f64,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Point2>, SamplingError> {
    let mut output = Vec::new();
    let mut scratch = Vec::new();
    clip_polygon_to_rect_into_cancellable(
        polygon,
        left,
        right,
        bottom,
        top,
        cancelled,
        &mut output,
        &mut scratch,
    )?;
    Ok(output)
}

/// Clips one polygon into caller-owned reusable buffers in fixed edge order.
///
/// Reusing the two buffers avoids allocating and freeing several polygons for every source-pixel footprint
/// in a large AreaAverage request. The accepted vertices and moment calculations remain identical
/// to [`clip_polygon_to_rect_cancellable`].
///
/// # Errors
///
/// Returns cancellation or a fallible clip-buffer allocation failure without exposing a partial
/// polygon as complete.
#[allow(clippy::too_many_arguments)]
fn clip_polygon_to_rect_into_cancellable(
    polygon: &[Point2],
    left: f64,
    right: f64,
    bottom: f64,
    top: f64,
    cancelled: &dyn Fn() -> bool,
    output: &mut Vec<Point2>,
    scratch: &mut Vec<Point2>,
) -> Result<(), SamplingError> {
    output.clear();
    scratch.clear();
    output.try_reserve(polygon.len()).map_err(|_| {
        SamplingError::new(
            "sampling.region_average.allocation.clip",
            "region clip-buffer allocation failed",
        )
    })?;
    output.extend_from_slice(polygon);
    for edge in [
        ClipEdge::Left(left),
        ClipEdge::Right(right),
        ClipEdge::Bottom(bottom),
        ClipEdge::Top(top),
    ] {
        poll_region(cancelled)?;
        if output.is_empty() {
            return Ok(());
        }
        scratch.clear();
        for (start, end) in output
            .iter()
            .zip(output.iter().cycle().skip(1))
            .take(output.len())
        {
            poll_region(cancelled)?;
            let start_inside = edge.contains(*start);
            let end_inside = edge.contains(*end);
            if start_inside {
                push_distinct_fallible(scratch, *start)?;
            }
            if start_inside != end_inside {
                push_distinct_fallible(scratch, edge.intersection(*start, *end))?;
            }
        }
        if scratch.len() > 1 && scratch.first() == scratch.last() {
            scratch.pop();
        }
        std::mem::swap(output, scratch);
    }
    if output.len() < 3 || polygon_signed_area(output).abs() <= f64::EPSILON {
        output.clear();
    }
    Ok(())
}

/// Names one Sutherland-Hodgman rectangle half-plane in deterministic processing order.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
enum ClipEdge {
    Left(f64),
    Right(f64),
    Bottom(f64),
    Top(f64),
}

#[allow(dead_code)]
impl ClipEdge {
    /// Reports whether a point is retained by this inclusive rectangle half-plane.
    fn contains(self, point: Point2) -> bool {
        match self {
            Self::Left(value) => point.x >= value,
            Self::Right(value) => point.x <= value,
            Self::Bottom(value) => point.y >= value,
            Self::Top(value) => point.y <= value,
        }
    }

    /// Computes the stable finite boundary crossing of one segment known to straddle this edge.
    fn intersection(self, start: Point2, end: Point2) -> Point2 {
        let (start_value, end_value, boundary, vertical) = match self {
            Self::Left(value) | Self::Right(value) => (start.x, end.x, value, true),
            Self::Bottom(value) | Self::Top(value) => (start.y, end.y, value, false),
        };
        let t = (boundary - start_value) / (end_value - start_value);
        if vertical {
            Point2::new(boundary, start.y + (end.y - start.y) * t)
        } else {
            Point2::new(start.x + (end.x - start.x) * t, boundary)
        }
    }
}

/// Appends a point only when it is not the exact adjacent duplicate of the prior point.
#[allow(dead_code)]
fn push_distinct(points: &mut Vec<Point2>, point: Point2) {
    if points.last().copied() != Some(point) {
        points.push(point);
    }
}

/// Appends one distinct clip vertex through a fallible reusable-buffer growth boundary.
///
/// # Errors
///
/// Returns a stable allocation diagnostic without changing an already complete prior result.
fn push_distinct_fallible(points: &mut Vec<Point2>, point: Point2) -> Result<(), SamplingError> {
    if points.last().copied() != Some(point) {
        points.try_reserve(1).map_err(|_| {
            SamplingError::new(
                "sampling.region_average.allocation.clip",
                "region clip-buffer allocation failed",
            )
        })?;
        points.push(point);
    }
    Ok(())
}

/// Computes signed area plus raw first and mixed moments from ordered polygon edges.
#[allow(dead_code)]
fn polygon_moments(points: &[Point2]) -> PolygonMoments {
    let mut doubled_area = 0.0;
    let mut mx = 0.0;
    let mut my = 0.0;
    let mut mxy = 0.0;
    for (left, right) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        let cross = left.x * right.y - right.x * left.y;
        doubled_area += cross;
        mx += (left.x + right.x) * cross;
        my += (left.y + right.y) * cross;
        mxy +=
            (left.x * right.y + 2.0 * left.x * left.y + 2.0 * right.x * right.y + right.x * left.y)
                * cross;
    }
    PolygonMoments {
        area: doubled_area / 2.0,
        mx: mx / 6.0,
        my: my / 6.0,
        mxy: mxy / 24.0,
    }
}

/// Computes only the signed polygon area needed by footprint classification and sliver rejection.
///
/// This hot path intentionally avoids first and mixed moment work because literal AreaAverage
/// selection depends solely on exact overlap area. Empty and degenerate input returns zero.
fn polygon_signed_area(points: &[Point2]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(left, right)| left.x * right.y - right.x * left.y)
        .sum::<f64>()
        / 2.0
}

/// Translates raw polygon moments to a bilinear source cell-local origin.
#[allow(dead_code)]
fn translate_polygon_moments(moments: PolygonMoments, origin: Point2) -> PolygonMoments {
    PolygonMoments {
        area: moments.area,
        mx: moments.mx - origin.x * moments.area,
        my: moments.my - origin.y * moments.area,
        mxy: moments.mxy - origin.x * moments.my - origin.y * moments.mx
            + origin.x * origin.y * moments.area,
    }
}

/// Decodes a source only from supplied bytes and an explicit supported-format hint.
pub fn decode_source(bytes: &[u8], hint: SourceFormatHint) -> Result<SourceField, SamplingError> {
    if bytes.is_empty() {
        return Err(SamplingError::new(
            "source.bytes",
            "source must not be empty",
        ));
    }
    match hint {
        SourceFormatHint::Png => decode_png(bytes),
        SourceFormatHint::Svg => decode_svg(bytes),
        SourceFormatHint::Jpeg => decode_raster(bytes, ImageFormat::Jpeg, SourceFormat::Jpeg),
        SourceFormatHint::Webp => decode_raster(bytes, ImageFormat::WebP, SourceFormat::Webp),
        SourceFormatHint::Bmp => decode_raster(bytes, ImageFormat::Bmp, SourceFormat::Bmp),
        SourceFormatHint::Tiff => decode_raster(bytes, ImageFormat::Tiff, SourceFormat::Tiff),
        SourceFormatHint::OpenExr => decode_openexr(bytes),
        SourceFormatHint::Avif => decode_avif(bytes),
        SourceFormatHint::Unsupported => Err(SamplingError::new(
            "source.format",
            "unsupported source format",
        )),
    }
}

/// Decodes a supported still source and encodes an aspect-preserving PNG proxy with a bounded long edge.
///
/// Alpha is retained in every encoded pixel, while RGB is deterministically quantized from the
/// sampling field's straight normalized values. Downsampling writes directly into the bounded
/// proxy buffer and never materializes a second full-resolution RGBA image. `maximum_long_edge`
/// must be nonzero. This helper owns decoding and resampling policy only; source IDs, document
/// authority, evaluation requests, and cache policy remain with the caller.
///
/// # Errors
///
/// Returns the established decoder diagnostics, a nonzero-bound diagnostic, or a PNG encoding
/// diagnostic. It never reads a path, mutates source bytes, or silently falls back to full size.
pub fn reduced_preview_png(
    bytes: &[u8],
    hint: SourceFormatHint,
    maximum_long_edge: u32,
) -> Result<ReducedPreviewSource, SamplingError> {
    if maximum_long_edge == 0 {
        return Err(SamplingError::new(
            "preview.proxy",
            "preview proxy long edge must be greater than zero",
        ));
    }
    let source = decode_source(bytes, hint)?;
    let source_width = source.identity.width;
    let source_height = source.identity.height;
    let source_long_edge = source_width.max(source_height);
    let (width, height) = if source_long_edge <= maximum_long_edge {
        (source_width, source_height)
    } else if source_width >= source_height {
        (
            maximum_long_edge,
            rounded_scaled_dimension(source_height, maximum_long_edge, source_width),
        )
    } else {
        (
            rounded_scaled_dimension(source_width, maximum_long_edge, source_height),
            maximum_long_edge,
        )
    };
    let proxy = bounded_preview_rgba(&source, width, height)?;
    let mut png_bytes = Vec::new();
    PngEncoder::new(&mut png_bytes)
        .write_image(&proxy, width, height, ColorType::Rgba8.into())
        .map_err(|_| SamplingError::new("preview.proxy", "could not encode preview proxy PNG"))?;
    Ok(ReducedPreviewSource {
        png_bytes,
        width,
        height,
    })
}

/// Downsamples one decoded field directly into its bounded private-preview RGBA byte buffer.
///
/// Each proxy pixel box-averages overlapping decoded source-pixel footprints in straight RGBA.
/// The output allocation is exactly `width * height * 4`; the helper never creates another
/// full-resolution image beside the authoritative decoded [`SourceField`].
///
/// # Errors
///
/// Returns a stable proxy diagnostic when source storage is inconsistent or the bounded output
/// byte count cannot be represented or reserved. It does not mutate the source field.
fn bounded_preview_rgba(
    source: &SourceField,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, SamplingError> {
    let source_width = source.identity.width;
    let source_height = source.identity.height;
    let source_pixels = usize::try_from(source_width)
        .ok()
        .and_then(|width| {
            usize::try_from(source_height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| SamplingError::new("preview.proxy", "source dimensions are unsafe"))?;
    if source.pixels.len() != source_pixels || width == 0 || height == 0 {
        return Err(SamplingError::new(
            "preview.proxy",
            "preview proxy dimensions or decoded source pixels are invalid",
        ));
    }
    let output_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| {
            SamplingError::new("preview.proxy", "preview proxy dimensions are unsafe")
        })?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_bytes)
        .map_err(|_| SamplingError::new("preview.proxy", "preview proxy byte allocation failed"))?;
    for destination_y in 0..height {
        let top = f64::from(destination_y) * f64::from(source_height) / f64::from(height);
        let bottom = f64::from(destination_y + 1) * f64::from(source_height) / f64::from(height);
        let first_y = top.floor() as u32;
        let last_y = bottom.ceil() as u32;
        for destination_x in 0..width {
            let left = f64::from(destination_x) * f64::from(source_width) / f64::from(width);
            let right = f64::from(destination_x + 1) * f64::from(source_width) / f64::from(width);
            let first_x = left.floor() as u32;
            let last_x = right.ceil() as u32;
            let mut totals = [0.0; 4];
            for source_y in first_y..last_y {
                let y_coverage =
                    (bottom.min(f64::from(source_y + 1)) - top.max(f64::from(source_y))).max(0.0);
                for source_x in first_x..last_x {
                    let x_coverage = (right.min(f64::from(source_x + 1))
                        - left.max(f64::from(source_x)))
                    .max(0.0);
                    let weight = x_coverage * y_coverage;
                    let pixel = source.pixels
                        [source_y as usize * source_width as usize + source_x as usize];
                    totals[0] += pixel.red * weight;
                    totals[1] += pixel.green * weight;
                    totals[2] += pixel.blue * weight;
                    totals[3] += pixel.alpha * weight;
                }
            }
            let area = (right - left) * (bottom - top);
            output.extend(totals.map(|value| normalized_preview_byte(value / area)));
        }
    }
    Ok(output)
}

/// Rounds one positive aspect-preserving dimension without allowing a zero-sized proxy.
///
/// The integer calculation avoids platform-dependent floating-point rounding and is used only
/// after validated nonzero source dimensions and a nonzero requested long edge.
fn rounded_scaled_dimension(value: u32, maximum_long_edge: u32, source_long_edge: u32) -> u32 {
    let numerator = u64::from(value) * u64::from(maximum_long_edge);
    let rounded =
        numerator.saturating_add(u64::from(source_long_edge) / 2) / u64::from(source_long_edge);
    u32::try_from(rounded.max(1)).expect("source preview proxy dimension fits u32")
}

/// Converts one finite normalized sampling component to deterministic 8-bit PNG storage.
///
/// Decoder-owned source fields guarantee finite normalized values, but this defensive clamp keeps
/// an invalid internal value from escaping the private preview boundary.
fn normalized_preview_byte(value: f64) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Decodes one finite still raster through the explicitly selected image codec.
///
/// The hint is authoritative: signature mismatches and malformed content fail rather than being
/// guessed. Decoded samples remain straight normalized RGBA; OpenEXR values are deterministically
/// clamped into that finite range without tone mapping or color-management policy.
fn decode_raster(
    bytes: &[u8],
    format: ImageFormat,
    source_format: SourceFormat,
) -> Result<SourceField, SamplingError> {
    let signature_matches = if matches!(format, ImageFormat::Tiff) {
        matches!(
            bytes.get(..4),
            Some(b"II*\0" | b"MM\0*" | b"II+\0" | b"MM\0+")
        )
    } else {
        image::guess_format(bytes).ok() == Some(format)
    };
    if !signature_matches {
        return Err(SamplingError::new(
            "source.format",
            "bytes do not match raster format hint",
        ));
    }
    if matches!(format, ImageFormat::WebP) && webp_is_animated(bytes) {
        return Err(SamplingError::new(
            "source.sequence",
            "animated WebP sources are not supported",
        ));
    }
    if matches!(format, ImageFormat::Tiff) && tiff_has_multiple_pages(bytes)? {
        return Err(SamplingError::new(
            "source.sequence",
            "multipage TIFF sources are not supported",
        ));
    }
    let reader = ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| SamplingError::new("source.decode", "malformed raster source"))?;
    validate_dimensions(width, height)?;
    let image = ImageReader::with_format(std::io::Cursor::new(bytes), format)
        .decode()
        .map_err(|_| SamplingError::new("source.decode", "malformed raster source"))?
        .to_rgba8();
    let decoded_pixel_hash = sha256(image.as_raw());
    let pixels = image
        .pixels()
        .map(|pixel| SourcePixel {
            red: f64::from(pixel[0]) / 255.0,
            green: f64::from(pixel[1]) / 255.0,
            blue: f64::from(pixel[2]) / 255.0,
            alpha: f64::from(pixel[3]) / 255.0,
        })
        .collect();
    Ok(SourceField {
        identity: SourceIdentity {
            format: source_format,
            width,
            height,
            content_hash: sha256(bytes),
            decoded_pixel_hash,
            svg_text: None,
        },
        pixels,
    })
}

/// Decodes a non-deep OpenEXR source as linear floating-point RGBA without tone mapping.
///
/// Finite channels are clamped to normalized `0.0..=1.0`; non-finite channels become zero so
/// source sampling remains finite. The decoded identity hashes the canonical f64 bit stream rather
/// than a lossy 8-bit conversion.
fn decode_openexr(bytes: &[u8]) -> Result<SourceField, SamplingError> {
    if image::guess_format(bytes).ok() != Some(ImageFormat::OpenExr) {
        return Err(SamplingError::new(
            "source.format",
            "bytes do not match OpenEXR hint",
        ));
    }
    let reader = ImageReader::with_format(std::io::Cursor::new(bytes), ImageFormat::OpenExr);
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| SamplingError::new("source.decode", "malformed OpenEXR source"))?;
    validate_dimensions(width, height)?;
    let image = ImageReader::with_format(std::io::Cursor::new(bytes), ImageFormat::OpenExr)
        .decode()
        .map_err(|_| SamplingError::new("source.decode", "malformed OpenEXR source"))?
        .to_rgba32f();
    let pixels: Vec<_> = image
        .pixels()
        .map(|pixel| SourcePixel {
            red: normalized_exr_channel(pixel[0]),
            green: normalized_exr_channel(pixel[1]),
            blue: normalized_exr_channel(pixel[2]),
            alpha: normalized_exr_channel(pixel[3]),
        })
        .collect();
    let mut canonical = Vec::with_capacity(pixels.len() * 32);
    for pixel in &pixels {
        for channel in [pixel.red, pixel.green, pixel.blue, pixel.alpha] {
            canonical.extend(channel.to_bits().to_le_bytes());
        }
    }
    Ok(SourceField {
        identity: SourceIdentity {
            format: SourceFormat::OpenExr,
            width,
            height,
            content_hash: sha256(bytes),
            decoded_pixel_hash: sha256(&canonical),
            svg_text: None,
        },
        pixels,
    })
}

/// Maps one OpenEXR linear sample into finite normalized sampling authority without tone mapping.
const fn normalized_exr_channel(value: f32) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        0.0
    } else if value >= 1.0 {
        1.0
    } else {
        value as f64
    }
}

/// Rejects AVIF sequence containers before the still-image decoder selects a primary item.
fn decode_avif(bytes: &[u8]) -> Result<SourceField, SamplingError> {
    if image::guess_format(bytes).ok() != Some(ImageFormat::Avif) {
        return Err(SamplingError::new(
            "source.format",
            "bytes do not match AVIF hint",
        ));
    }
    if avif_is_animated(bytes) {
        return Err(SamplingError::new(
            "source.sequence",
            "animated AVIF sources are not supported",
        ));
    }
    decode_raster(bytes, ImageFormat::Avif, SourceFormat::Avif)
}

/// Detects animated WebP RIFF chunks without accepting a first frame implicitly.
fn webp_is_animated(bytes: &[u8]) -> bool {
    if bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WEBP") {
        return false;
    }
    let mut offset = 12usize;
    while let Some(header) = bytes.get(offset..offset.saturating_add(8)) {
        let kind = &header[..4];
        if kind == b"ANIM" || kind == b"ANMF" {
            return true;
        }
        let size = u32::from_le_bytes(header[4..8].try_into().expect("four WebP size bytes"));
        let Ok(size) = usize::try_from(size) else {
            return false;
        };
        let Some(next) = offset
            .checked_add(8)
            .and_then(|value| value.checked_add(size))
            .and_then(|value| value.checked_add(size % 2))
        else {
            return false;
        };
        if next <= offset || next > bytes.len() {
            return false;
        }
        offset = next;
    }
    false
}

/// Detects an AVIF movie track, which denotes a sequence rather than a single AVIF image item.
fn avif_is_animated(bytes: &[u8]) -> bool {
    bmff_boxes(bytes).any(|box_| {
        box_.kind == *b"moov" && bmff_boxes(box_.payload).any(|child| child.kind == *b"trak")
    })
}

/// Describes one bounds-checked ISO base media file box payload.
struct BmffBox<'a> {
    kind: [u8; 4],
    payload: &'a [u8],
}

/// Iterates complete ISO base media file boxes and stops at the first malformed boundary.
fn bmff_boxes(bytes: &[u8]) -> impl Iterator<Item = BmffBox<'_>> {
    let mut offset = 0usize;
    std::iter::from_fn(move || {
        let header = bytes.get(offset..offset.checked_add(8)?)?;
        let size32 = u32::from_be_bytes(header[..4].try_into().expect("four BMFF size bytes"));
        let kind = header[4..8].try_into().expect("four BMFF type bytes");
        let (header_len, size) = match size32 {
            0 => (8usize, bytes.len().checked_sub(offset)?),
            1 => {
                let extended = bytes.get(offset.checked_add(8)?..offset.checked_add(16)?)?;
                let size = u64::from_be_bytes(
                    extended.try_into().expect("eight extended BMFF size bytes"),
                );
                (16usize, usize::try_from(size).ok()?)
            }
            size => (8usize, usize::try_from(size).ok()?),
        };
        if size < header_len {
            return None;
        }
        let end = offset.checked_add(size)?;
        let payload_start = offset.checked_add(header_len)?;
        let payload = bytes.get(payload_start..end)?;
        offset = end;
        Some(BmffBox { kind, payload })
    })
}

/// Reports whether a classic TIFF or BigTIFF has a subsequent IFD page.
///
/// Malformed offset widths, directory bounds, and arithmetic fail explicitly
/// before the image decoder can select an implicit first page.
fn tiff_has_multiple_pages(bytes: &[u8]) -> Result<bool, SamplingError> {
    if bytes.len() < 8 {
        return Err(SamplingError::new("source.decode", "malformed TIFF source"));
    }
    let little_endian = match &bytes[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Err(SamplingError::new("source.decode", "malformed TIFF source")),
    };
    let read_u16 = |offset: usize| -> Option<u16> {
        bytes.get(offset..offset + 2).map(|value| {
            if little_endian {
                u16::from_le_bytes(value.try_into().expect("two TIFF bytes"))
            } else {
                u16::from_be_bytes(value.try_into().expect("two TIFF bytes"))
            }
        })
    };
    let read_u32 = |offset: usize| -> Option<u32> {
        bytes.get(offset..offset + 4).map(|value| {
            if little_endian {
                u32::from_le_bytes(value.try_into().expect("four TIFF bytes"))
            } else {
                u32::from_be_bytes(value.try_into().expect("four TIFF bytes"))
            }
        })
    };
    let read_u64 = |offset: usize| -> Option<u64> {
        bytes.get(offset..offset + 8).map(|value| {
            if little_endian {
                u64::from_le_bytes(value.try_into().expect("eight TIFF bytes"))
            } else {
                u64::from_be_bytes(value.try_into().expect("eight TIFF bytes"))
            }
        })
    };
    match read_u16(2) {
        Some(42) => {
            let ifd = usize::try_from(
                read_u32(4)
                    .ok_or_else(|| SamplingError::new("source.decode", "malformed TIFF source"))?,
            )
            .map_err(|_| SamplingError::new("source.decode", "malformed TIFF source"))?;
            let entries = usize::from(
                read_u16(ifd)
                    .ok_or_else(|| SamplingError::new("source.decode", "malformed TIFF source"))?,
            );
            let next_offset = ifd
                .checked_add(2)
                .and_then(|value| value.checked_add(entries.checked_mul(12)?))
                .ok_or_else(|| SamplingError::new("source.decode", "malformed TIFF source"))?;
            Ok(read_u32(next_offset)
                .ok_or_else(|| SamplingError::new("source.decode", "malformed TIFF source"))?
                != 0)
        }
        Some(43) if read_u16(4) == Some(8) && read_u16(6) == Some(0) => {
            let ifd =
                usize::try_from(read_u64(8).ok_or_else(|| {
                    SamplingError::new("source.decode", "malformed BigTIFF source")
                })?)
                .map_err(|_| SamplingError::new("source.decode", "malformed BigTIFF source"))?;
            let entries =
                usize::try_from(read_u64(ifd).ok_or_else(|| {
                    SamplingError::new("source.decode", "malformed BigTIFF source")
                })?)
                .map_err(|_| SamplingError::new("source.decode", "malformed BigTIFF source"))?;
            let next_offset = ifd
                .checked_add(8)
                .and_then(|value| value.checked_add(entries.checked_mul(20)?))
                .ok_or_else(|| SamplingError::new("source.decode", "malformed BigTIFF source"))?;
            Ok(read_u64(next_offset)
                .ok_or_else(|| SamplingError::new("source.decode", "malformed BigTIFF source"))?
                != 0)
        }
        _ => Err(SamplingError::new(
            "source.decode",
            "unsupported TIFF container",
        )),
    }
}

/// Decodes PNG with its historical explicit signature check retained for stable diagnostics.
fn decode_png(bytes: &[u8]) -> Result<SourceField, SamplingError> {
    const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if !bytes.starts_with(PNG_SIGNATURE) {
        return Err(SamplingError::new(
            "source.format",
            "bytes do not match PNG hint",
        ));
    }
    let (width, height) = png_dimensions(bytes)?;
    validate_dimensions(width, height)?;
    let image = ImageReader::with_format(std::io::Cursor::new(bytes), ImageFormat::Png)
        .decode()
        .map_err(|_| SamplingError::new("source.decode", "malformed PNG source"))?
        .to_rgba8();
    let decoded_pixel_hash = sha256(image.as_raw());
    let pixels = image
        .pixels()
        .map(|pixel| SourcePixel {
            red: f64::from(pixel[0]) / 255.0,
            green: f64::from(pixel[1]) / 255.0,
            blue: f64::from(pixel[2]) / 255.0,
            alpha: f64::from(pixel[3]) / 255.0,
        })
        .collect();
    Ok(SourceField {
        identity: SourceIdentity {
            format: SourceFormat::Png,
            width,
            height,
            content_hash: sha256(bytes),
            decoded_pixel_hash,
            svg_text: None,
        },
        pixels,
    })
}

fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32), SamplingError> {
    if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
        return Err(SamplingError::new("source.decode", "malformed PNG source"));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("four bytes"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("four bytes"));
    Ok((width, height))
}

fn decode_svg(bytes: &[u8]) -> Result<SourceField, SamplingError> {
    decode_svg_with_system_fonts(bytes, true)
}

fn decode_svg_with_system_fonts(
    bytes: &[u8],
    load_system_fonts: bool,
) -> Result<SourceField, SamplingError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| SamplingError::new("source.format", "bytes do not match SVG hint"))?;
    if !text.contains("<svg") {
        return Err(SamplingError::new(
            "source.format",
            "bytes do not match SVG hint",
        ));
    }
    let has_live_text_node = text.contains("<text") || text.contains(":text");
    let mut options = usvg::Options {
        font_family: "sans-serif".to_owned(),
        ..usvg::Options::default()
    };
    if load_system_fonts {
        options.fontdb_mut().load_system_fonts();
    }
    let sans_query = usvg::fontdb::Query {
        families: &[usvg::fontdb::Family::SansSerif],
        ..usvg::fontdb::Query::default()
    };
    if options.fontdb_mut().query(&sans_query).is_none() {
        return Err(SamplingError::new(
            "source.svg.font_policy",
            "no usable system sans-serif font is available",
        ));
    }
    let tree = usvg::Tree::from_data(bytes, &options)
        .map_err(|_| SamplingError::new("source.decode", "malformed SVG source"))?;
    let size = tree.size();
    let width = size.width().round() as u32;
    let height = size.height().round() as u32;
    validate_dimensions(width, height)?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or(SamplingError::new(
        "source.decode",
        "SVG allocation is unsafe",
    ))?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    let decoded_pixel_hash = sha256(pixmap.data());
    let pixels = pixmap
        .data()
        .chunks_exact(4)
        .map(|pixel| unpremultiply_rgba(pixel[0], pixel[1], pixel[2], pixel[3]))
        .collect();
    let rendered_glyph_coverage = if has_live_text_node {
        let without_text = strip_text_nodes(text);
        let textless_tree =
            usvg::Tree::from_data(without_text.as_bytes(), &options).map_err(|_| {
                SamplingError::new("source.decode", "could not inspect SVG text coverage")
            })?;
        let mut textless = tiny_skia::Pixmap::new(width, height).ok_or(SamplingError::new(
            "source.decode",
            "SVG allocation is unsafe",
        ))?;
        resvg::render(
            &textless_tree,
            tiny_skia::Transform::default(),
            &mut textless.as_mut(),
        );
        pixmap.data() != textless.data()
    } else {
        false
    };
    Ok(SourceField {
        identity: SourceIdentity {
            format: SourceFormat::Svg,
            width,
            height,
            content_hash: sha256(bytes),
            decoded_pixel_hash,
            svg_text: Some(SvgTextDiagnostic {
                has_live_text_node,
                font_policy: "system sans-serif fallback required".to_owned(),
                rendered_glyph_coverage,
            }),
        },
        pixels,
    })
}

fn strip_text_nodes(svg: &str) -> String {
    let mut output = String::with_capacity(svg.len());
    let mut remaining = svg;
    while let Some(start) = remaining.find("<text") {
        output.push_str(&remaining[..start]);
        let Some(end) = remaining[start..].find("</text>") else {
            return svg.to_owned();
        };
        remaining = &remaining[start + end + "</text>".len()..];
    }
    output.push_str(remaining);
    output
}

fn unpremultiply_rgba(red: u8, green: u8, blue: u8, alpha: u8) -> SourcePixel {
    let alpha = f64::from(alpha) / 255.0;
    let straight = |channel: u8| {
        if alpha == 0.0 {
            0.0
        } else {
            (f64::from(channel) / 255.0 / alpha).clamp(0.0, 1.0)
        }
    };
    SourcePixel {
        red: straight(red),
        green: straight(green),
        blue: straight(blue),
        alpha,
    }
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), SamplingError> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 {
        return Err(SamplingError::new(
            "source.dimensions",
            "source must not be zero-sized",
        ));
    }
    if pixels > MAX_SOURCE_PIXELS {
        return Err(SamplingError::new(
            "source.dimensions",
            "source allocation is unsafe",
        ));
    }
    Ok(())
}

fn validate_canvas(canvas: &CanvasSpec) -> Result<(), SamplingError> {
    if canvas.width.is_finite()
        && canvas.height.is_finite()
        && canvas.width > 0.0
        && canvas.height > 0.0
    {
        Ok(())
    } else {
        Err(SamplingError::new(
            "sampling.canvas",
            "canvas dimensions must be positive and finite",
        ))
    }
}

fn map_axis(value: f64, canvas_extent: f64, source_extent: u32) -> f64 {
    if source_extent <= 1 {
        return 0.0;
    }
    (value / canvas_extent * f64::from(source_extent - 1)).clamp(0.0, f64::from(source_extent - 1))
}

fn component_value(pixel: SourcePixel, component: SourceComponent) -> f64 {
    match component {
        SourceComponent::Luminance => rec709_luminance(pixel.red, pixel.green, pixel.blue),
        SourceComponent::Alpha => pixel.alpha,
    }
}

/// Returns the requested Stage 9 scalar field for one decoded straight-sRGB pixel.
///
/// RGB and CMYK are calculated in linear light. CMYK retains `K = 1 - max(R, G, B)`
/// and normalizes C, M, and Y by `1 - K` so pure-process subtractive transmittance
/// reconstructs the linear source color. Exact black has no chromatic denominator and
/// therefore returns zero C, M, and Y. This function never associates color fields
/// with alpha; [`mapped_response`] performs that operation exactly once after mapping.
pub fn mapping_component_value(pixel: SourcePixel, component: SourceMappingComponent) -> f64 {
    let (red, green, blue) = linear_rgb(pixel);
    let maximum = red.max(green).max(blue);
    let black = (1.0 - maximum).clamp(0.0, 1.0);
    let normalized_chromatic = |value: f64| {
        if maximum == 0.0 {
            0.0
        } else {
            (1.0 - value / maximum).clamp(0.0, 1.0)
        }
    };
    match component {
        SourceMappingComponent::Red => red,
        SourceMappingComponent::Green => green,
        SourceMappingComponent::Blue => blue,
        SourceMappingComponent::Cyan => normalized_chromatic(red),
        SourceMappingComponent::Magenta => normalized_chromatic(green),
        SourceMappingComponent::Yellow => normalized_chromatic(blue),
        SourceMappingComponent::Black => black,
        SourceMappingComponent::Alpha => pixel.alpha.clamp(0.0, 1.0),
        SourceMappingComponent::Luminance => rec709_luminance_linear(red, green, blue),
    }
}

fn mapped_response(pixel: SourcePixel, mapping: SourceMapping) -> f64 {
    let value = mapping_component_value(pixel, mapping.component);
    let transformed = transform_mapping(value, mapping);
    match mapping.component {
        SourceMappingComponent::Alpha => transformed,
        _ => (transformed * pixel.alpha).clamp(0.0, 1.0),
    }
}

fn validate_mapping(mapping: SourceMapping) -> Result<(), SamplingError> {
    if !mapping.gain.is_finite() || mapping.gain < 0.0 {
        return Err(SamplingError::new(
            "sampling.mapping.gain",
            "mapping gain must be finite and nonnegative",
        ));
    }
    if !mapping.bias.is_finite() {
        return Err(SamplingError::new(
            "sampling.mapping.bias",
            "mapping bias must be finite",
        ));
    }
    Ok(())
}

fn transform_mapping(value: f64, mapping: SourceMapping) -> f64 {
    let value = if mapping.inverted { 1.0 - value } else { value };
    (mapping.gain * value + mapping.bias).clamp(0.0, 1.0)
}

fn linear_rgb(pixel: SourcePixel) -> (f64, f64, f64) {
    (
        srgb_to_linear(pixel.red.clamp(0.0, 1.0)),
        srgb_to_linear(pixel.green.clamp(0.0, 1.0)),
        srgb_to_linear(pixel.blue.clamp(0.0, 1.0)),
    )
}

fn associated_linear(pixel: SourcePixel) -> [f64; 4] {
    let (red, green, blue) = linear_rgb(pixel);
    let alpha = pixel.alpha.clamp(0.0, 1.0);
    [red * alpha, green * alpha, blue * alpha, alpha]
}

/// Converts one raw source pixel into the realization's normalized mark-ink
/// response. This happens before bilinear interpolation.
pub fn effective_mark_ink(pixel: SourcePixel, component: SourceComponent) -> f64 {
    match component {
        SourceComponent::Luminance => (pixel.alpha
            * (1.0 - rec709_luminance(pixel.red, pixel.green, pixel.blue)))
        .clamp(0.0, 1.0),
        // Alpha is its own source component. Keep its existing "low alpha is
        // high ink" polarity and never multiply this response by alpha again.
        SourceComponent::Alpha => (1.0 - pixel.alpha).clamp(0.0, 1.0),
    }
}

/// Converts one straight-sRGB component to linear light.
pub fn srgb_to_linear(value: f64) -> f64 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Computes Rec.709 luminance in linear light, never multiplying by alpha.
pub fn rec709_luminance(red: f64, green: f64, blue: f64) -> f64 {
    0.2126 * srgb_to_linear(red) + 0.7152 * srgb_to_linear(green) + 0.0722 * srgb_to_linear(blue)
}

fn rec709_luminance_linear(red: f64, green: f64, blue: f64) -> f64 {
    (0.2126 * red + 0.7152 * green + 0.0722 * blue).clamp(0.0, 1.0)
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// A stable failure at the decoding or sampling boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SamplingError {
    path: &'static str,
    message: &'static str,
}

impl SamplingError {
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }

    pub const fn path(&self) -> &'static str {
        self.path
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for SamplingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for SamplingError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/../../assets/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn baseline_assets_have_documented_hashes_and_properties() {
        let png = decode_source(&asset("raster-sample.png"), SourceFormatHint::Png).unwrap();
        assert_eq!(
            png.identity().content_hash,
            "sha256:324ac232e319002a13fbcfac46538ca5d7e8ba8a127eea2eaf20e8ddb3ed2ef2"
        );
        assert_eq!((png.identity().width, png.identity().height), (1024, 1024));
        let alpha: Vec<_> = png.pixels.iter().map(|pixel| pixel.alpha).collect();
        assert!(alpha.contains(&0.0));
        assert!(alpha.contains(&1.0));
        assert!(alpha.iter().any(|value| *value > 0.0 && *value < 1.0));
        let svg = decode_source(&asset("vector-sample.svg"), SourceFormatHint::Svg).unwrap();
        assert_eq!(
            svg.identity().content_hash,
            "sha256:42eb5e23111a5dbad66f2b1802a7cc06391c7ede829b99eb28aeb1ac91596e2e"
        );
        assert_eq!((svg.identity().width, svg.identity().height), (900, 620));
        let diagnostic = svg.identity().svg_text.as_ref().unwrap();
        assert!(diagnostic.has_live_text_node);
        assert!(diagnostic.rendered_glyph_coverage);
    }

    #[test]
    fn rejects_hint_mismatches_unsupported_empty_and_fontless_svg() {
        let png = asset("raster-sample.png");
        assert_eq!(
            decode_source(&[], SourceFormatHint::Png)
                .unwrap_err()
                .path(),
            "source.bytes"
        );
        assert_eq!(
            decode_source(&png, SourceFormatHint::Svg)
                .unwrap_err()
                .path(),
            "source.format"
        );
        assert_eq!(
            decode_source(&png, SourceFormatHint::Unsupported)
                .unwrap_err()
                .path(),
            "source.format"
        );
        assert_eq!(
            decode_svg_with_system_fonts(&asset("vector-sample.svg"), false)
                .unwrap_err()
                .path(),
            "source.svg.font_policy"
        );
        assert!(decode_source(b"\x89PNG\r\n\x1a\n", SourceFormatHint::Png).is_err());
        assert!(decode_source(b"<svg width='900'", SourceFormatHint::Svg).is_err());
        assert!(decode_source(&png_header(0, 10), SourceFormatHint::Png).is_err());
        assert_eq!(
            decode_source(&png_header(100_000, 100_000), SourceFormatHint::Png)
                .unwrap_err()
                .path(),
            "source.dimensions"
        );
    }

    /// Exercises each locally encodable Stage 21A still codec through the shared decoder boundary.
    #[test]
    fn supported_still_raster_codecs_decode_as_finite_rgba_fields() {
        let source = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            3,
            image::Rgba([32, 96, 192, 128]),
        ));
        for (format, hint, expected) in [
            (
                image::ImageFormat::Jpeg,
                SourceFormatHint::Jpeg,
                SourceFormat::Jpeg,
            ),
            (
                image::ImageFormat::WebP,
                SourceFormatHint::Webp,
                SourceFormat::Webp,
            ),
            (
                image::ImageFormat::Bmp,
                SourceFormatHint::Bmp,
                SourceFormat::Bmp,
            ),
            (
                image::ImageFormat::Tiff,
                SourceFormatHint::Tiff,
                SourceFormat::Tiff,
            ),
        ] {
            let mut bytes = std::io::Cursor::new(Vec::new());
            source
                .write_to(&mut bytes, format)
                .expect("test codec encodes");
            let decoded = decode_source(bytes.get_ref(), hint).expect("test codec decodes");
            assert_eq!(decoded.identity().format, expected);
            assert_eq!(
                (decoded.identity().width, decoded.identity().height),
                (2, 3)
            );
            assert!(decoded.pixels.iter().all(|pixel| pixel.red.is_finite()
                && pixel.green.is_finite()
                && pixel.blue.is_finite()
                && pixel.alpha.is_finite()));
            if expected == SourceFormat::Jpeg {
                assert!(decoded.pixels.iter().all(|pixel| pixel.alpha == 1.0));
            }
            assert_eq!(
                decode_source(bytes.get_ref(), SourceFormatHint::Png)
                    .expect_err("explicit mismatched hint rejects")
                    .path(),
                "source.format"
            );
            assert!(
                decode_source(&bytes.get_ref()[..bytes.get_ref().len() / 2], hint).is_err(),
                "truncated {expected:?} input rejects"
            );
        }
    }

    /// Decodes a deterministic single-image AVIF witness through the native dav1d boundary.
    #[test]
    fn avif_still_image_decodes_without_sequence_fallback() {
        const STILL_AVIF: &[u8] = &[
            0, 0, 0, 32, 102, 116, 121, 112, 97, 118, 105, 102, 0, 0, 0, 0, 97, 118, 105, 102, 109,
            105, 102, 49, 109, 105, 97, 102, 77, 65, 49, 65, 0, 0, 0, 249, 109, 101, 116, 97, 0, 0,
            0, 0, 0, 0, 0, 47, 104, 100, 108, 114, 0, 0, 0, 0, 0, 0, 0, 0, 112, 105, 99, 116, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 80, 105, 99, 116, 117, 114, 101, 72, 97, 110, 100, 108,
            101, 114, 0, 0, 0, 0, 14, 112, 105, 116, 109, 0, 0, 0, 0, 0, 1, 0, 0, 0, 30, 105, 108,
            111, 99, 0, 0, 0, 0, 68, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0, 1, 33, 0, 0, 0, 27, 0, 0, 0,
            40, 105, 105, 110, 102, 0, 0, 0, 0, 0, 1, 0, 0, 0, 26, 105, 110, 102, 101, 2, 0, 0, 0,
            0, 1, 0, 0, 97, 118, 48, 49, 67, 111, 108, 111, 114, 0, 0, 0, 0, 106, 105, 112, 114,
            112, 0, 0, 0, 75, 105, 112, 99, 111, 0, 0, 0, 20, 105, 115, 112, 101, 0, 0, 0, 0, 0, 0,
            0, 2, 0, 0, 0, 4, 0, 0, 0, 16, 112, 105, 120, 105, 0, 0, 0, 0, 3, 8, 8, 8, 0, 0, 0, 12,
            97, 118, 49, 67, 129, 32, 0, 0, 0, 0, 0, 19, 99, 111, 108, 114, 110, 99, 108, 120, 0,
            2, 0, 2, 0, 2, 0, 0, 0, 0, 23, 105, 112, 109, 97, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 4, 1,
            2, 131, 4, 0, 0, 0, 35, 109, 100, 97, 116, 10, 5, 56, 0, 123, 96, 128, 50, 18, 24, 0,
            0, 0, 80, 0, 0, 0, 0, 176, 19, 142, 113, 219, 133, 218, 34, 128,
        ];
        let decoded = decode_source(STILL_AVIF, SourceFormatHint::Avif)
            .expect("single-image AVIF decodes through dav1d");
        assert_eq!(decoded.identity().format, SourceFormat::Avif);
        assert_eq!(
            (decoded.identity().width, decoded.identity().height),
            (2, 4)
        );
        assert!(decoded.pixels.iter().all(|pixel| pixel.red.is_finite()
            && pixel.green.is_finite()
            && pixel.blue.is_finite()
            && pixel.alpha == 1.0));
        assert_eq!(
            decode_source(STILL_AVIF, SourceFormatHint::Png)
                .expect_err("AVIF bytes cannot satisfy a PNG hint")
                .path(),
            "source.format"
        );
        assert!(
            decode_source(&STILL_AVIF[..STILL_AVIF.len() / 2], SourceFormatHint::Avif).is_err()
        );
    }

    /// Preserves finite OpenEXR linear samples beyond 8-bit precision and clamps HDR values.
    #[test]
    fn openexr_decoding_uses_linear_float_samples_without_tone_mapping() {
        let source = image::DynamicImage::ImageRgba32F(image::Rgba32FImage::from_pixel(
            1,
            1,
            image::Rgba([0.125, 1.5, -0.5, 0.75]),
        ));
        let mut bytes = std::io::Cursor::new(Vec::new());
        source
            .write_to(&mut bytes, image::ImageFormat::OpenExr)
            .expect("test OpenEXR encodes");
        let decoded = decode_source(bytes.get_ref(), SourceFormatHint::OpenExr)
            .expect("test OpenEXR decodes");
        let pixel = decoded.pixels[0];
        assert!((pixel.red - 0.125).abs() < 1e-12);
        assert_eq!(pixel.green, 1.0);
        assert_eq!(pixel.blue, 0.0);
        assert!((pixel.alpha - 0.75).abs() < 1e-12);
        assert_eq!(
            decode_source(bytes.get_ref(), SourceFormatHint::Jpeg)
                .expect_err("OpenEXR bytes cannot satisfy a JPEG hint")
                .path(),
            "source.format"
        );
        assert!(
            decode_source(
                &bytes.get_ref()[..bytes.get_ref().len() / 2],
                SourceFormatHint::OpenExr,
            )
            .is_err()
        );
    }

    /// Rejects sequence containers before an image decoder can silently choose a primary frame.
    #[test]
    fn sequence_container_detection_rejects_webp_avif_and_multipage_tiff() {
        let animated_webp = b"RIFF\x0c\0\0\0WEBPANIM\0\0\0\0";
        assert!(webp_is_animated(animated_webp));
        assert_eq!(
            decode_source(animated_webp, SourceFormatHint::Webp)
                .expect_err("animated WebP rejects before frame decoding")
                .path(),
            "source.sequence"
        );
        assert!(!webp_is_animated(b"RIFF\x0c\0\0\0WEBPVP8 \x04\0\0\0ANIM"));
        let animated_avif = [
            0, 0, 0, 16, b'f', b't', b'y', b'p', b'a', b'v', b'i', b'f', 0, 0, 0, 0, 0, 0, 0, 16,
            b'm', b'o', b'o', b'v', 0, 0, 0, 8, b't', b'r', b'a', b'k',
        ];
        assert!(avif_is_animated(&animated_avif));
        assert_eq!(
            decode_source(&animated_avif, SourceFormatHint::Avif)
                .expect_err("animated AVIF rejects before primary-item decoding")
                .path(),
            "source.sequence"
        );
        let still_payload_with_track_text = [
            0, 0, 0, 16, b'f', b't', b'y', b'p', b'a', b'v', b'i', b'f', b't', b'r', b'a', b'k',
        ];
        assert!(!avif_is_animated(&still_payload_with_track_text));
        let multipage_little_endian_tiff = [
            b'I', b'I', 42, 0, 8, 0, 0, 0, // header and first IFD offset
            0, 0, // zero entries
            16, 0, 0, 0, // nonzero next IFD offset
        ];
        assert!(tiff_has_multiple_pages(&multipage_little_endian_tiff).unwrap());
        assert_eq!(
            decode_source(&multipage_little_endian_tiff, SourceFormatHint::Tiff,)
                .expect_err("multipage TIFF rejects before page decoding")
                .path(),
            "source.sequence"
        );
        assert_eq!(
            tiff_has_multiple_pages(b"II*").unwrap_err().path(),
            "source.decode"
        );
        let multipage_big_tiff = [
            b'I', b'I', 43, 0, 8, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, // zero 64-bit entry count
            32, 0, 0, 0, 0, 0, 0, 0, // nonzero next 64-bit IFD offset
        ];
        assert!(tiff_has_multiple_pages(&multipage_big_tiff).unwrap());
        assert_eq!(
            decode_source(&multipage_big_tiff, SourceFormatHint::Tiff)
                .expect_err("multipage BigTIFF rejects before page decoding")
                .path(),
            "source.sequence"
        );
    }

    #[test]
    fn luminance_is_linear_and_alpha_is_independent() {
        let red = SourcePixel {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.25,
        };
        assert!((component_value(red, SourceComponent::Luminance) - 0.2126).abs() < 1e-12);
        assert_eq!(component_value(red, SourceComponent::Alpha), 0.25);
        assert!((effective_mark_ink(red, SourceComponent::Luminance) - 0.19685).abs() < 1e-12);
        assert_eq!(effective_mark_ink(red, SourceComponent::Alpha), 0.75);
        assert!((srgb_to_linear(0.5) - 0.21404114048223255).abs() < 1e-12);
    }

    #[test]
    fn baseline_fields_are_repeatable_and_sample_both_components() {
        let bytes = asset("raster-sample.png");
        let first = decode_source(&bytes, SourceFormatHint::Png).unwrap();
        let second = decode_source(&bytes, SourceFormatHint::Png).unwrap();
        assert_eq!(first, second);
        let canvas = CanvasSpec {
            width: 900.0,
            height: 600.0,
        };
        let luminance = first
            .sample(
                Point2::new(450.0, 300.0),
                &canvas,
                SourcePlacement::StretchToCanvas,
                SourceComponent::Luminance,
            )
            .unwrap();
        let alpha = first
            .sample(
                Point2::new(450.0, 300.0),
                &canvas,
                SourcePlacement::StretchToCanvas,
                SourceComponent::Alpha,
            )
            .unwrap();
        assert!(luminance.is_finite() && alpha.is_finite());
        assert!(luminance != alpha);
        assert_eq!(
            first
                .sample(
                    Point2::new(f64::NAN, 0.0),
                    &canvas,
                    SourcePlacement::StretchToCanvas,
                    SourceComponent::Alpha
                )
                .unwrap_err()
                .path(),
            "sampling.point"
        );
        assert_eq!(
            first
                .sample(
                    Point2::new(0.0, 0.0),
                    &CanvasSpec {
                        width: f64::INFINITY,
                        height: 1.0
                    },
                    SourcePlacement::StretchToCanvas,
                    SourceComponent::Alpha
                )
                .unwrap_err()
                .path(),
            "sampling.canvas"
        );
    }

    #[test]
    fn stretch_mapping_bilinear_sampling_and_clamping_are_deterministic() {
        let field = SourceField {
            identity: SourceIdentity {
                format: SourceFormat::Png,
                width: 2,
                height: 2,
                content_hash: "sha256:test".to_owned(),
                decoded_pixel_hash: "sha256:test-pixels".to_owned(),
                svg_text: None,
            },
            pixels: vec![
                SourcePixel {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 0.0,
                },
                SourcePixel {
                    red: 1.0,
                    green: 1.0,
                    blue: 1.0,
                    alpha: 1.0,
                },
                SourcePixel {
                    red: 1.0,
                    green: 1.0,
                    blue: 1.0,
                    alpha: 1.0,
                },
                SourcePixel {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 0.0,
                },
            ],
        };
        let canvas = CanvasSpec {
            width: 10.0,
            height: 10.0,
        };
        assert_eq!(
            field
                .sample(
                    Point2::new(0.0, 0.0),
                    &canvas,
                    SourcePlacement::StretchToCanvas,
                    SourceComponent::Alpha
                )
                .unwrap(),
            0.0
        );
        assert_eq!(
            field
                .sample(
                    Point2::new(10.0, 0.0),
                    &canvas,
                    SourcePlacement::StretchToCanvas,
                    SourceComponent::Alpha
                )
                .unwrap(),
            1.0
        );
        assert!(
            (field
                .sample(
                    Point2::new(5.0, 5.0),
                    &canvas,
                    SourcePlacement::StretchToCanvas,
                    SourceComponent::Alpha
                )
                .unwrap()
                - 0.5)
                .abs()
                < 1e-12
        );
        assert_eq!(
            field
                .sample(
                    Point2::new(-10.0, 20.0),
                    &canvas,
                    SourcePlacement::StretchToCanvas,
                    SourceComponent::Alpha
                )
                .unwrap(),
            1.0
        );
    }

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend(width.to_be_bytes());
        bytes.extend(height.to_be_bytes());
        bytes.extend([8, 6, 0, 0, 0]);
        bytes.extend(crc32(&bytes[12..]).to_be_bytes());
        bytes
    }

    #[test]
    fn decoded_png_keeps_hidden_rgb_and_alpha_as_independent_fields_and_clamps_guards() {
        let image = image::RgbaImage::from_raw(
            4,
            1,
            vec![
                0, 0, 0, 0, // transparent black
                255, 0, 0, 0, // transparent saturated red
                255, 255, 255, 128, // partial white
                255, 255, 255, 255, // opaque white
            ],
        )
        .unwrap();
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let field = decode_source(&bytes, SourceFormatHint::Png).unwrap();
        let canvas = CanvasSpec {
            width: 3.0,
            height: 1.0,
        };
        let luminance = |x| {
            field
                .sample(
                    Point2::new(x, 0.0),
                    &canvas,
                    SourcePlacement::StretchToCanvas,
                    SourceComponent::Luminance,
                )
                .unwrap()
        };
        let alpha = |x| {
            field
                .sample(
                    Point2::new(x, 0.0),
                    &canvas,
                    SourcePlacement::StretchToCanvas,
                    SourceComponent::Alpha,
                )
                .unwrap()
        };
        assert_eq!(alpha(0.0), 0.0);
        assert_eq!(alpha(1.0), 0.0);
        assert!((alpha(2.0) - 128.0 / 255.0).abs() < 1e-12);
        assert_eq!(alpha(3.0), 1.0);
        assert_ne!(
            luminance(0.0),
            luminance(1.0),
            "hidden RGB remains straight RGBA at alpha zero"
        );
        assert_eq!(
            luminance(2.0),
            luminance(3.0),
            "same RGB has alpha-independent luminance"
        );
        assert_eq!(luminance(-10.0), luminance(0.0));
        assert_eq!(luminance(10.0), luminance(3.0));
    }

    #[test]
    fn decoded_png_associates_luminance_ink_before_bilinear_interpolation() {
        let image = image::RgbaImage::from_raw(
            8,
            1,
            vec![
                0, 0, 0, 0, // transparent black
                255, 255, 255, 0, // transparent white
                255, 0, 0, 0, // transparent red
                0, 0, 0, 255, // opaque black
                255, 255, 255, 255, // opaque white
                0, 0, 0, 0, // black alpha 0
                0, 0, 0, 128, // black alpha about 0.5
                0, 0, 0, 255, // black alpha 1
            ],
        )
        .unwrap();
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let field = decode_source(&bytes, SourceFormatHint::Png).unwrap();
        assert_eq!(field.pixel(1, 0).unwrap().red, 1.0);
        assert_eq!(field.pixel(2, 0).unwrap().red, 1.0);
        assert_eq!(field.pixel(2, 0).unwrap().green, 0.0);
        let canvas = CanvasSpec {
            width: 7.0,
            height: 1.0,
        };
        let ink = |x, component| {
            field
                .sample_mark_ink(
                    Point2::new(x, 0.0),
                    &canvas,
                    SourcePlacement::StretchToCanvas,
                    component,
                )
                .unwrap()
        };
        for x in [0.0, 1.0, 2.0, 5.0] {
            assert_eq!(ink(x, SourceComponent::Luminance), 0.0);
        }
        assert_eq!(ink(3.0, SourceComponent::Luminance), 1.0);
        assert_eq!(ink(4.0, SourceComponent::Luminance), 0.0);
        assert!((ink(6.0, SourceComponent::Luminance) - 128.0 / 255.0).abs() < 1e-12);
        assert_eq!(ink(7.0, SourceComponent::Luminance), 1.0);
        assert_eq!(ink(5.0, SourceComponent::Alpha), 1.0);
        assert!((ink(6.0, SourceComponent::Alpha) - 127.0 / 255.0).abs() < 1e-12);
        assert_eq!(ink(7.0, SourceComponent::Alpha), 0.0);

        let edge = image::RgbaImage::from_raw(2, 1, vec![0, 0, 0, 255, 255, 255, 255, 0]).unwrap();
        let mut edge_bytes = Vec::new();
        image::DynamicImage::ImageRgba8(edge)
            .write_to(
                &mut std::io::Cursor::new(&mut edge_bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let edge_field = decode_source(&edge_bytes, SourceFormatHint::Png).unwrap();
        let edge_canvas = CanvasSpec {
            width: 1.0,
            height: 1.0,
        };
        assert!(
            (edge_field
                .sample_mark_ink(
                    Point2::new(0.5, 0.0),
                    &edge_canvas,
                    SourcePlacement::StretchToCanvas,
                    SourceComponent::Luminance,
                )
                .unwrap()
                - 0.5)
                .abs()
                < 1e-12
        );
    }

    fn synthetic_field(pixels: Vec<SourcePixel>) -> SourceField {
        SourceField {
            identity: SourceIdentity {
                format: SourceFormat::Png,
                width: pixels.len() as u32,
                height: 1,
                content_hash: "sha256:synthetic".to_owned(),
                decoded_pixel_hash: "sha256:synthetic-pixels".to_owned(),
                svg_text: None,
            },
            pixels,
        }
    }

    /// Verifies linear RGB and normalized CMYK fields for canonical opaque colors.
    #[test]
    fn stage9_linear_rgb_and_normalized_cmyk_fields_cover_synthetic_colors() {
        let field = synthetic_field(vec![
            SourcePixel {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            SourcePixel {
                red: 1.0,
                green: 1.0,
                blue: 1.0,
                alpha: 1.0,
            },
            SourcePixel {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            SourcePixel {
                red: 0.0,
                green: 1.0,
                blue: 0.0,
                alpha: 1.0,
            },
            SourcePixel {
                red: 0.0,
                green: 0.0,
                blue: 1.0,
                alpha: 1.0,
            },
            SourcePixel {
                red: 0.0,
                green: 1.0,
                blue: 1.0,
                alpha: 1.0,
            },
            SourcePixel {
                red: 1.0,
                green: 0.0,
                blue: 1.0,
                alpha: 1.0,
            },
            SourcePixel {
                red: 1.0,
                green: 1.0,
                blue: 0.0,
                alpha: 1.0,
            },
            SourcePixel {
                red: 0.5,
                green: 0.5,
                blue: 0.5,
                alpha: 1.0,
            },
            SourcePixel {
                red: 0.8,
                green: 0.4,
                blue: 0.2,
                alpha: 1.0,
            },
        ]);
        let component = |pixel, component| mapping_component_value(pixel, component);
        let black = field.pixel(0, 0).unwrap();
        assert_eq!(component(black, SourceMappingComponent::Black), 1.0);
        assert_eq!(component(black, SourceMappingComponent::Cyan), 0.0);
        assert_eq!(component(black, SourceMappingComponent::Magenta), 0.0);
        assert_eq!(component(black, SourceMappingComponent::Yellow), 0.0);
        let white = field.pixel(1, 0).unwrap();
        assert_eq!(component(white, SourceMappingComponent::Black), 0.0);
        assert_eq!(component(white, SourceMappingComponent::Cyan), 0.0);
        assert_eq!(component(white, SourceMappingComponent::Magenta), 0.0);
        assert_eq!(component(white, SourceMappingComponent::Yellow), 0.0);
        let red = field.pixel(2, 0).unwrap();
        assert_eq!(component(red, SourceMappingComponent::Red), 1.0);
        assert_eq!(component(red, SourceMappingComponent::Cyan), 0.0);
        assert_eq!(component(red, SourceMappingComponent::Magenta), 1.0);
        assert_eq!(component(red, SourceMappingComponent::Yellow), 1.0);
        let green = field.pixel(3, 0).unwrap();
        assert_eq!(component(green, SourceMappingComponent::Green), 1.0);
        assert_eq!(component(green, SourceMappingComponent::Cyan), 1.0);
        assert_eq!(component(green, SourceMappingComponent::Magenta), 0.0);
        assert_eq!(component(green, SourceMappingComponent::Yellow), 1.0);
        let blue = field.pixel(4, 0).unwrap();
        assert_eq!(component(blue, SourceMappingComponent::Blue), 1.0);
        assert_eq!(component(blue, SourceMappingComponent::Cyan), 1.0);
        assert_eq!(component(blue, SourceMappingComponent::Magenta), 1.0);
        assert_eq!(component(blue, SourceMappingComponent::Yellow), 0.0);
        let cyan = field.pixel(5, 0).unwrap();
        assert_eq!(component(cyan, SourceMappingComponent::Red), 0.0);
        assert_eq!(component(cyan, SourceMappingComponent::Cyan), 1.0);
        assert_eq!(component(cyan, SourceMappingComponent::Magenta), 0.0);
        assert_eq!(component(cyan, SourceMappingComponent::Yellow), 0.0);
        let magenta = field.pixel(6, 0).unwrap();
        assert_eq!(component(magenta, SourceMappingComponent::Cyan), 0.0);
        assert_eq!(component(magenta, SourceMappingComponent::Magenta), 1.0);
        assert_eq!(component(magenta, SourceMappingComponent::Yellow), 0.0);
        let yellow = field.pixel(7, 0).unwrap();
        assert_eq!(component(yellow, SourceMappingComponent::Cyan), 0.0);
        assert_eq!(component(yellow, SourceMappingComponent::Magenta), 0.0);
        assert_eq!(component(yellow, SourceMappingComponent::Yellow), 1.0);
        let gray = field.pixel(8, 0).unwrap();
        let linear_gray = srgb_to_linear(0.5);
        assert!((component(gray, SourceMappingComponent::Red) - linear_gray).abs() < 1e-12);
        assert!(
            (component(gray, SourceMappingComponent::Black) - (1.0 - linear_gray)).abs() < 1e-12
        );
        assert_eq!(component(gray, SourceMappingComponent::Cyan), 0.0);
        assert_eq!(component(gray, SourceMappingComponent::Magenta), 0.0);
        assert_eq!(component(gray, SourceMappingComponent::Yellow), 0.0);
        assert!(component(gray, SourceMappingComponent::Luminance) > 0.21);
        let chromatic_midtone = field.pixel(9, 0).unwrap();
        let linear_red = srgb_to_linear(0.8);
        let linear_green = srgb_to_linear(0.4);
        let linear_blue = srgb_to_linear(0.2);
        let chromatic_black = 1.0 - linear_red.max(linear_green).max(linear_blue);
        let normalized_magenta = 1.0 - linear_green / linear_red;
        assert!(
            (component(chromatic_midtone, SourceMappingComponent::Black) - chromatic_black).abs()
                < 1e-12
        );
        assert!(
            (component(chromatic_midtone, SourceMappingComponent::Magenta) - normalized_magenta)
                .abs()
                < 1e-12
        );
        assert!(
            (component(chromatic_midtone, SourceMappingComponent::Yellow)
                - (1.0 - linear_blue / linear_red))
                .abs()
                < 1e-12
        );
        assert!(
            (component(chromatic_midtone, SourceMappingComponent::Magenta)
                - (1.0 - linear_green - chromatic_black))
                .abs()
                > 0.1,
            "normalized CMY must differ from unnormalized full UCR for a chromatic midtone"
        );
        assert_eq!(
            DECODER_CONTRACT_ID,
            "toniator-sampling-decoder-v3-still-image-linear-source-fields"
        );
    }

    /// Verifies that normalized CMYK and pure-process transmittance reconstruct linear RGB.
    #[test]
    fn normalized_cmyk_reconstructs_representative_linear_rgb_through_pure_inks() {
        for pixel in [
            SourcePixel {
                red: 0.8,
                green: 0.4,
                blue: 0.2,
                alpha: 1.0,
            },
            SourcePixel {
                red: 0.35,
                green: 0.65,
                blue: 0.9,
                alpha: 1.0,
            },
            SourcePixel {
                red: 0.5,
                green: 0.5,
                blue: 0.5,
                alpha: 1.0,
            },
        ] {
            let (red, green, blue) = linear_rgb(pixel);
            let cyan = mapping_component_value(pixel, SourceMappingComponent::Cyan);
            let magenta = mapping_component_value(pixel, SourceMappingComponent::Magenta);
            let yellow = mapping_component_value(pixel, SourceMappingComponent::Yellow);
            let black = mapping_component_value(pixel, SourceMappingComponent::Black);
            assert!(((1.0 - black) * (1.0 - cyan) - red).abs() < 1e-12);
            assert!(((1.0 - black) * (1.0 - magenta) - green).abs() < 1e-12);
            assert!(((1.0 - black) * (1.0 - yellow) - blue).abs() < 1e-12);
        }
    }

    /// Verifies normalized CMYK retains exactly-once alpha association and zero-alpha suppression.
    #[test]
    fn normalized_cmyk_associates_source_alpha_once_and_suppresses_hidden_rgb() {
        let mapping = SourceMapping {
            component: SourceMappingComponent::Magenta,
            placement: SourcePlacement::StretchToCanvas,
            inverted: false,
            gain: 1.0,
            bias: 0.0,
        };
        let partial_red = SourcePixel {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.25,
        };
        assert_eq!(
            mapping_component_value(partial_red, SourceMappingComponent::Magenta),
            1.0
        );
        assert_eq!(mapped_response(partial_red, mapping), 0.25);

        let hidden_red = SourcePixel {
            alpha: 0.0,
            ..partial_red
        };
        assert_eq!(
            mapping_component_value(hidden_red, SourceMappingComponent::Magenta),
            1.0,
            "straight hidden RGB remains inspectable before association"
        );
        assert_eq!(mapped_response(hidden_red, mapping), 0.0);
    }

    #[test]
    fn stage9_mapping_transform_associates_color_once_but_not_alpha() {
        let field = synthetic_field(vec![
            SourcePixel {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 0.25,
            },
            SourcePixel {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 0.0,
            },
        ]);
        let canvas = CanvasSpec {
            width: 1.0,
            height: 1.0,
        };
        let red = SourceMapping {
            component: SourceMappingComponent::Red,
            placement: SourcePlacement::StretchToCanvas,
            inverted: true,
            gain: 2.0,
            bias: -0.5,
        };
        // red is 1 -> inverted 0 -> transformed/clamped 0, then alpha once.
        assert_eq!(
            field
                .sample_mapping_response(Point2::new(0.0, 0.0), &canvas, red)
                .unwrap(),
            0.0
        );
        let blue = SourceMapping {
            component: SourceMappingComponent::Blue,
            inverted: true,
            gain: 0.5,
            bias: 0.25,
            ..red
        };
        // blue is 0 -> inverted 1 -> 0.75, then alpha = 0.1875.
        assert!(
            (field
                .sample_mapping_response(Point2::new(0.0, 0.0), &canvas, blue)
                .unwrap()
                - 0.1875)
                .abs()
                < 1e-12
        );
        let alpha = SourceMapping {
            component: SourceMappingComponent::Alpha,
            inverted: false,
            gain: 2.0,
            bias: 0.1,
            ..red
        };
        // Alpha is transformed and clamped but never multiplied by itself.
        assert_eq!(
            field
                .sample_mapping_response(Point2::new(0.0, 0.0), &canvas, alpha)
                .unwrap(),
            0.6
        );
        assert_eq!(
            field
                .sample_mapping_response(
                    Point2::new(0.0, 0.0),
                    &canvas,
                    SourceMapping { bias: 2.0, ..alpha }
                )
                .unwrap(),
            1.0,
            "mapping clamp occurs before the independent alpha response"
        );
        assert_eq!(
            field
                .sample_mapping_response(
                    Point2::new(0.0, 0.0),
                    &canvas,
                    SourceMapping {
                        gain: -1.0,
                        ..alpha
                    }
                )
                .unwrap_err()
                .path(),
            "sampling.mapping.gain"
        );
    }

    #[test]
    fn source_color_associates_unassociates_and_suppresses_zero_alpha() {
        let field = synthetic_field(vec![
            SourcePixel {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            SourcePixel {
                red: 0.0,
                green: 1.0,
                blue: 0.0,
                alpha: 0.0,
            },
            SourcePixel {
                red: 0.0,
                green: 0.0,
                blue: 1.0,
                alpha: 0.5,
            },
            SourcePixel {
                red: 1.0,
                green: 1.0,
                blue: 1.0,
                alpha: 0.0,
            },
        ]);
        let canvas = CanvasSpec {
            width: 3.0,
            height: 1.0,
        };
        let alpha = SourceMapping::canonical(SourceMappingComponent::Alpha);
        let opaque = field
            .sample_source_color(Point2::new(0.0, 0.0), &canvas, alpha)
            .unwrap();
        assert_eq!(opaque.response, 1.0);
        assert_eq!(
            opaque.paint.unwrap(),
            SampledSourcePaint {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0
            }
        );
        let edge = field
            .sample_source_color(Point2::new(0.5, 0.0), &canvas, alpha)
            .unwrap();
        // Associated interpolation ignores transparent green; response still comes from alpha.
        assert_eq!(edge.response, 0.5);
        assert_eq!(
            edge.paint.unwrap(),
            SampledSourcePaint {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0
            }
        );
        let partial = field
            .sample_source_color(Point2::new(2.0, 0.0), &canvas, alpha)
            .unwrap();
        assert_eq!(partial.response, 0.5);
        assert_eq!(
            partial.paint.unwrap(),
            SampledSourcePaint {
                red: 0.0,
                green: 0.0,
                blue: 1.0,
                alpha: 1.0
            }
        );
        let hidden = field
            .sample_source_color(Point2::new(3.0, 0.0), &canvas, alpha)
            .unwrap();
        assert_eq!(hidden.response, 0.0);
        assert_eq!(hidden.paint, None);
    }

    /// Verifies exact rectangle area and first/mixed moments.
    #[test]
    fn stage20q_rectangle_polygon_moments_are_exact() {
        let moments = polygon_moments(&[
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(2.0, 3.0),
            Point2::new(0.0, 3.0),
        ]);
        assert_eq!(moments.area, 6.0);
        assert_eq!(moments.mx, 6.0);
        assert_eq!(moments.my, 9.0);
        assert_eq!(moments.mxy, 9.0);
    }

    /// Verifies winding reverses every signed polygon moment.
    #[test]
    fn stage20q_clockwise_polygon_moments_reverse_sign() {
        let ccw = polygon_moments(&[
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(0.0, 2.0),
        ]);
        let cw = polygon_moments(&[
            Point2::new(0.0, 2.0),
            Point2::new(2.0, 0.0),
            Point2::new(0.0, 0.0),
        ]);
        assert_eq!(cw.area, -ccw.area);
        assert_eq!(cw.mx, -ccw.mx);
        assert_eq!(cw.my, -ccw.my);
        assert_eq!(cw.mxy, -ccw.mxy);
    }

    /// Confirms the AreaAverage hot path's area-only helper matches full polygon moments for either winding.
    #[test]
    fn stage21a_polygon_signed_area_matches_full_moments_without_extra_moment_work() {
        let points = [
            Point2::new(-2.0, 1.0),
            Point2::new(3.0, 1.0),
            Point2::new(3.0, 4.0),
            Point2::new(-2.0, 4.0),
        ];
        assert_eq!(polygon_signed_area(&points), polygon_moments(&points).area);
        let reversed = points.into_iter().rev().collect::<Vec<_>>();
        assert_eq!(
            polygon_signed_area(&reversed),
            polygon_moments(&reversed).area
        );
    }

    /// Verifies cell-local translation preserves exact local rectangle moments.
    #[test]
    fn stage20q_translated_polygon_moments_are_cell_local() {
        let global = polygon_moments(&[
            Point2::new(4.0, 7.0),
            Point2::new(6.0, 7.0),
            Point2::new(6.0, 10.0),
            Point2::new(4.0, 10.0),
        ]);
        let local = translate_polygon_moments(global, Point2::new(4.0, 7.0));
        assert_eq!(local.area, 6.0);
        assert_eq!(local.mx, 6.0);
        assert_eq!(local.my, 9.0);
        assert_eq!(local.mxy, 9.0);
    }

    /// Verifies clipping a multi-edge polygon retains only the requested rectangle area.
    #[test]
    fn stage20q_rectangle_clipping_is_deterministic_and_rejects_boundary_slivers() {
        let polygon = [
            Point2::new(-1.0, 0.5),
            Point2::new(0.5, -1.0),
            Point2::new(2.0, 0.5),
            Point2::new(0.5, 2.0),
        ];
        let clipped =
            clip_polygon_to_rect_cancellable(&polygon, 0.0, 1.0, 0.0, 1.0, &|| false).unwrap();
        assert!(!clipped.is_empty());
        assert!(polygon_moments(&clipped).area > 0.0);
        let touch = clip_polygon_to_rect_cancellable(
            &[
                Point2::new(-1.0, 0.0),
                Point2::new(0.0, 0.0),
                Point2::new(-1.0, 1.0),
            ],
            0.0,
            1.0,
            0.0,
            1.0,
            &|| false,
        )
        .unwrap();
        assert!(touch.is_empty());
    }

    /// Verifies rectangle clipping reports the canonical cancellation diagnostic before output.
    #[test]
    fn stage20q_rectangle_clipping_cancellation_is_atomic() {
        let error = clip_polygon_to_rect_cancellable(
            &[
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(0.0, 1.0),
            ],
            0.0,
            1.0,
            0.0,
            1.0,
            &|| true,
        )
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
    }

    /// Builds a closed source-space ring for flattening tests.
    fn stage20q_flattening_ring(segments: Vec<CurveSegment>) -> CurvePath {
        CurvePath::new(segments, toniator_geometry::PathClosure::Closed).unwrap()
    }

    /// Verifies the inverse StretchToCanvas transform remains deliberately unclamped.
    #[test]
    fn stage20q_source_space_flattening_preserves_off_canvas_coordinates() {
        let ring = CurvePath::polyline(
            vec![
                Point2::new(-5.0, 0.0),
                Point2::new(15.0, 0.0),
                Point2::new(15.0, 10.0),
                Point2::new(-5.0, 10.0),
            ],
            toniator_geometry::PathClosure::Closed,
        )
        .unwrap();
        let points = flatten_region_source_space(
            &ring,
            &CanvasSpec {
                width: 10.0,
                height: 10.0,
            },
            11,
            11,
            RegionSamplingLimits::default(),
            &|| false,
        )
        .unwrap();
        assert_eq!(points[0], Point2::new(-6.0, -0.5));
        assert_eq!(points[1], Point2::new(16.0, -0.5));
    }

    /// Verifies a straight cubic emits one stable chord and does not duplicate joins.
    #[test]
    fn stage20q_straight_cubic_source_flattening_is_stable() {
        let cubic = toniator_geometry::CubicBezierSegment::new(
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(6.0, 0.0),
        )
        .unwrap();
        let ring = stage20q_flattening_ring(vec![
            CurveSegment::CubicBezier(cubic),
            CurveSegment::Line(
                toniator_geometry::LineSegment::new(Point2::new(6.0, 0.0), Point2::new(0.0, 6.0))
                    .unwrap(),
            ),
            CurveSegment::Line(
                toniator_geometry::LineSegment::new(Point2::new(0.0, 6.0), Point2::new(0.0, 0.0))
                    .unwrap(),
            ),
        ]);
        let first = flatten_region_source_space(
            &ring,
            &CanvasSpec {
                width: 6.0,
                height: 6.0,
            },
            7,
            7,
            RegionSamplingLimits::default(),
            &|| false,
        )
        .unwrap();
        let second = flatten_region_source_space(
            &ring,
            &CanvasSpec {
                width: 6.0,
                height: 6.0,
            },
            7,
            7,
            RegionSamplingLimits::default(),
            &|| false,
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first[0], Point2::new(-0.5, -0.5));
    }

    /// Verifies a genuine cubic flattens deterministically and honors the global segment limit.
    #[test]
    fn stage20q_genuine_cubic_flattening_is_deterministic_and_bounded() {
        let cubic = toniator_geometry::CubicBezierSegment::new(
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 8.0),
            Point2::new(8.0, 8.0),
            Point2::new(8.0, 0.0),
        )
        .unwrap();
        let ring = stage20q_flattening_ring(vec![
            CurveSegment::CubicBezier(cubic),
            CurveSegment::Line(
                toniator_geometry::LineSegment::new(Point2::new(8.0, 0.0), Point2::new(0.0, 0.0))
                    .unwrap(),
            ),
        ]);
        let points = flatten_region_source_space(
            &ring,
            &CanvasSpec {
                width: 8.0,
                height: 8.0,
            },
            9,
            9,
            RegionSamplingLimits::default(),
            &|| false,
        )
        .unwrap();
        assert!(points.len() > 3);
        let error = flatten_region_source_space(
            &ring,
            &CanvasSpec {
                width: 8.0,
                height: 8.0,
            },
            9,
            9,
            RegionSamplingLimits {
                max_flattened_segments: 1,
                ..RegionSamplingLimits::default()
            },
            &|| false,
        )
        .unwrap_err();
        assert_eq!(error.path(), "sampling.region_average.limits.flattening");
    }

    /// Verifies cancellation prevents any source-space flattening candidate from escaping.
    #[test]
    fn stage20q_source_space_flattening_cancellation_is_exact() {
        let ring = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(0.0, 1.0),
            ],
            toniator_geometry::PathClosure::Closed,
        )
        .unwrap();
        let error = flatten_region_source_space(
            &ring,
            &CanvasSpec {
                width: 1.0,
                height: 1.0,
            },
            2,
            2,
            RegionSamplingLimits::default(),
            &|| true,
        )
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
    }

    /// Verifies literal footprint enumeration retains every exterior edge-clamped contribution.
    #[test]
    fn stage21a_source_intervals_preserve_each_edge_clamped_footprint_in_order() {
        let intervals = source_axis_intervals(-2.0, 4.0, 4).unwrap();
        assert_eq!(
            intervals,
            vec![
                SourceAxisInterval {
                    start: -2.0,
                    end: -1.5,
                    cell: 0
                },
                SourceAxisInterval {
                    start: -1.5,
                    end: -0.5,
                    cell: 0
                },
                SourceAxisInterval {
                    start: -0.5,
                    end: 0.5,
                    cell: 0
                },
                SourceAxisInterval {
                    start: 0.5,
                    end: 1.5,
                    cell: 1
                },
                SourceAxisInterval {
                    start: 1.5,
                    end: 2.5,
                    cell: 2
                },
                SourceAxisInterval {
                    start: 2.5,
                    end: 3.5,
                    cell: 3
                },
                SourceAxisInterval {
                    start: 3.5,
                    end: 4.0,
                    cell: 3
                }
            ]
        );
    }

    /// Verifies a one-pixel source axis partitions every edge-clamped unit footprint.
    #[test]
    fn stage21a_one_pixel_source_axis_partitions_edge_clamped_unit_footprints() {
        let intervals = source_axis_intervals(-3.0, 7.0, 1).unwrap();
        assert_eq!(intervals.len(), 11);
        assert!(intervals.iter().all(|interval| interval.cell == 0));
        assert_eq!(intervals.first().map(|interval| interval.start), Some(-3.0));
        assert_eq!(intervals.last().map(|interval| interval.end), Some(7.0));
    }

    /// Verifies shared request counters aggregate limits and canonical cancellation failures.
    #[test]
    fn stage20q_sampling_work_is_request_wide_and_bounded() {
        let work = RegionSamplingWork::new(
            RegionSamplingLimits {
                max_flattened_segments: 2,
                max_cell_intersections: 1,
                max_subdivision_depth: 48,
            },
            &|| false,
        )
        .unwrap();
        work.charge_flattened().unwrap();
        work.charge_flattened().unwrap();
        assert_eq!(
            work.charge_flattened().unwrap_err().path(),
            "sampling.region_average.limits.flattening"
        );
        work.charge_cell().unwrap();
        assert_eq!(
            work.charge_cell().unwrap_err().path(),
            "sampling.region_average.limits.cell_intersections"
        );
        assert_eq!(
            RegionSamplingWork::new(RegionSamplingLimits::default(), &|| true)
                .unwrap()
                .poll()
                .unwrap_err()
                .path(),
            "evaluation.cancelled"
        );
    }

    /// Builds a finite two-dimensional decoded field for focused deterministic sampling witnesses.
    fn stage20q_field(width: u32, height: u32, pixels: Vec<SourcePixel>) -> SourceField {
        assert_eq!(pixels.len(), (width * height) as usize);
        SourceField {
            identity: SourceIdentity {
                format: SourceFormat::Png,
                width,
                height,
                content_hash: "sha256:stage20q".to_owned(),
                decoded_pixel_hash: "sha256:stage20q-pixels".to_owned(),
                svg_text: None,
            },
            pixels,
        }
    }

    /// Builds a closed source-aligned rectangle in canvas coordinates for exact averaging tests.
    fn stage20q_rectangle(left: f64, right: f64, bottom: f64, top: f64) -> CurvePath {
        CurvePath::polyline(
            vec![
                Point2::new(left, bottom),
                Point2::new(right, bottom),
                Point2::new(right, top),
                Point2::new(left, top),
            ],
            toniator_geometry::PathClosure::Closed,
        )
        .unwrap()
    }

    /// Verifies constant fields make ReferencePoint and exact AreaAverage sampling identical.
    #[test]
    fn stage20q_area_average_matches_constant_reference_point() {
        let pixel = SourcePixel {
            red: 0.5,
            green: 0.25,
            blue: 0.75,
            alpha: 0.4,
        };
        let field = stage20q_field(2, 2, vec![pixel; 4]);
        let canvas = CanvasSpec {
            width: 1.0,
            height: 1.0,
        };
        let mapping = SourceMapping::canonical(SourceMappingComponent::Alpha);
        let reference =
            sample_region_reference(&field, Point2::new(0.25, 0.75), &canvas, mapping).unwrap();
        let average = sample_region_area_average(
            &field,
            &stage20q_rectangle(0.0, 1.0, 0.0, 1.0),
            &canvas,
            mapping,
            RegionSamplingLimits::default(),
            &|| false,
        )
        .unwrap();
        assert!((average.response - reference.response).abs() < 1e-12);
        assert_eq!(average.paint, reference.paint);
    }

    /// Verifies fully covered literal pixel footprints equally average full alpha values without point sampling.
    #[test]
    fn stage21a_area_average_equally_averages_full_covered_pixel_values() {
        let field = stage20q_field(
            2,
            2,
            vec![
                SourcePixel {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 0.0,
                },
                SourcePixel {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                SourcePixel {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                SourcePixel {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 0.0,
                },
            ],
        );
        let sample = sample_region_area_average(
            &field,
            &stage20q_rectangle(0.0, 1.0, 0.0, 1.0),
            &CanvasSpec {
                width: 1.0,
                height: 1.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Alpha),
            RegionSamplingLimits::default(),
            &|| false,
        )
        .unwrap();
        assert!((sample.response - 0.5).abs() < 1e-12);
    }

    /// Verifies associated RGB is averaged before positive-alpha unassociation.
    #[test]
    fn stage20q_area_average_associates_then_unassociates_paint() {
        let opaque_red = SourcePixel {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        };
        let transparent_hidden = SourcePixel {
            red: 0.0,
            green: 1.0,
            blue: 1.0,
            alpha: 0.0,
        };
        let field = stage20q_field(
            2,
            2,
            vec![
                opaque_red,
                transparent_hidden,
                transparent_hidden,
                transparent_hidden,
            ],
        );
        let sample = sample_region_area_average(
            &field,
            &stage20q_rectangle(0.0, 1.0, 0.0, 1.0),
            &CanvasSpec {
                width: 1.0,
                height: 1.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Alpha),
            RegionSamplingLimits::default(),
            &|| false,
        )
        .unwrap();
        assert!((sample.response - 0.25).abs() < 1e-12);
        assert_eq!(
            sample.paint,
            Some(SampledSourcePaint {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0
            })
        );
    }

    /// Classifies literal source-pixel footprints below, at, and above the exact 50% threshold.
    ///
    /// The one-pixel field makes the full decoded scalar contribution unambiguous: sub-threshold
    /// coverage excludes it, while exact and super-threshold coverage include the complete value
    /// without fractional multiplication or point-center substitution.
    #[test]
    fn stage21a_area_average_binary_footprint_threshold_is_inclusive() {
        let field = stage20q_field(
            1,
            1,
            vec![SourcePixel {
                red: 1.0,
                green: 1.0,
                blue: 1.0,
                alpha: 1.0,
            }],
        );
        let sample = |right| {
            sample_region_area_average(
                &field,
                &stage20q_rectangle(0.0, right, 0.0, 1.0),
                &CanvasSpec {
                    width: 1.0,
                    height: 1.0,
                },
                SourceMapping::canonical(SourceMappingComponent::Alpha),
                RegionSamplingLimits::default(),
                &|| false,
            )
            .expect("finite threshold rectangle samples")
        };
        let below = sample(0.49);
        let at = sample(0.50);
        let above = sample(0.51);
        assert_eq!(below.response, 0.0);
        assert_eq!(below.paint, None);
        for included in [at, above] {
            assert_eq!(included.response, 1.0);
            assert_eq!(
                included.paint,
                Some(SampledSourcePaint {
                    red: 1.0,
                    green: 1.0,
                    blue: 1.0,
                    alpha: 1.0,
                })
            );
        }
    }

    /// Proves a sub-threshold source pixel is excluded while a separately covered pixel keeps its full value.
    #[test]
    fn stage21a_area_average_excludes_partial_pixel_without_diluting_full_neighbor() {
        let full_pixel = SourcePixel {
            red: 0.8,
            green: 0.4,
            blue: 0.2,
            alpha: 1.0,
        };
        let field = stage20q_field(
            2,
            1,
            vec![
                SourcePixel {
                    red: 0.2,
                    green: 0.1,
                    blue: 0.0,
                    alpha: 1.0,
                },
                full_pixel,
            ],
        );
        let sample = sample_region_area_average(
            &field,
            &stage20q_rectangle(0.255, 1.0, 0.0, 1.0),
            &CanvasSpec {
                width: 1.0,
                height: 1.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Red),
            RegionSamplingLimits::default(),
            &|| false,
        )
        .expect("mixed-coverage region samples");
        let expected_response = mapping_component_value(full_pixel, SourceMappingComponent::Red);
        let (red, green, blue) = linear_rgb(full_pixel);
        assert!(
            (sample.response - expected_response).abs() < 1e-12,
            "only the fully covered second pixel contributes: {sample:?}"
        );
        assert_eq!(
            sample.paint,
            Some(SampledSourcePaint {
                red,
                green,
                blue,
                alpha: 1.0,
            })
        );
    }

    /// Rejects extreme finite footprint coordinates before lossy integer conversion can overflow.
    #[test]
    fn stage21a_area_average_extreme_finite_footprint_bounds_reject_without_panic() {
        for bounds in [(f64::MAX, f64::MAX), (f64::MIN, f64::MIN)] {
            assert_eq!(
                SourceAxisPartitions::new(bounds.0, bounds.1, 1)
                    .expect_err("extreme finite footprint bounds reject")
                    .path(),
                "sampling.region_average.limits.cell_intersections"
            );
        }
    }

    /// Verifies exact-zero average alpha suppresses hidden RGB instead of publishing transparent paint.
    #[test]
    fn stage20q_area_average_suppresses_all_zero_alpha() {
        let field = stage20q_field(
            2,
            2,
            vec![
                SourcePixel {
                    red: 1.0,
                    green: 0.5,
                    blue: 0.25,
                    alpha: 0.0
                };
                4
            ],
        );
        let sample = sample_region_area_average(
            &field,
            &stage20q_rectangle(0.0, 1.0, 0.0, 1.0),
            &CanvasSpec {
                width: 1.0,
                height: 1.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Red),
            RegionSamplingLimits::default(),
            &|| false,
        )
        .unwrap();
        assert_eq!(sample.response, 0.0);
        assert_eq!(sample.paint, None);
    }

    /// Verifies complete off-canvas area retains all edge-clamped exterior footprint contributions.
    #[test]
    fn stage21a_area_average_includes_complete_off_canvas_edge_clamped_footprints() {
        let pixel = SourcePixel {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.75,
        };
        let field = stage20q_field(2, 2, vec![pixel; 4]);
        let sample = sample_region_area_average(
            &field,
            &stage20q_rectangle(-1.0, 2.0, -1.0, 2.0),
            &CanvasSpec {
                width: 1.0,
                height: 1.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Alpha),
            RegionSamplingLimits::default(),
            &|| false,
        )
        .unwrap();
        assert!((sample.response - 0.75).abs() < 1e-12);
    }

    /// Verifies clockwise and counter-clockwise rings normalize their signed moment results equally.
    #[test]
    fn stage20q_area_average_is_winding_independent() {
        let field = stage20q_field(
            2,
            2,
            vec![
                SourcePixel {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 0.0,
                },
                SourcePixel {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                SourcePixel {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                SourcePixel {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 0.0,
                },
            ],
        );
        let counter_clockwise = stage20q_rectangle(0.0, 1.0, 0.0, 1.0);
        let clockwise = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(0.0, 1.0),
                Point2::new(1.0, 1.0),
                Point2::new(1.0, 0.0),
            ],
            toniator_geometry::PathClosure::Closed,
        )
        .unwrap();
        let canvas = CanvasSpec {
            width: 1.0,
            height: 1.0,
        };
        let mapping = SourceMapping::canonical(SourceMappingComponent::Alpha);
        let first = sample_region_area_average(
            &field,
            &counter_clockwise,
            &canvas,
            mapping,
            RegionSamplingLimits::default(),
            &|| false,
        )
        .unwrap();
        let second = sample_region_area_average(
            &field,
            &clockwise,
            &canvas,
            mapping,
            RegionSamplingLimits::default(),
            &|| false,
        )
        .unwrap();
        assert_eq!(first, second);
    }

    /// Verifies each partition rectangle contributes once and preserves the complete polygon area.
    #[test]
    fn stage20q_partition_area_conserves_without_double_counting() {
        let polygon = vec![
            Point2::new(-1.0, -1.0),
            Point2::new(2.0, -1.0),
            Point2::new(2.0, 2.0),
            Point2::new(-1.0, 2.0),
        ];
        let xs = source_axis_intervals(-1.0, 2.0, 2).unwrap();
        let ys = source_axis_intervals(-1.0, 2.0, 2).unwrap();
        let mut partitioned = 0.0;
        for y in ys {
            for x in &xs {
                let clipped = clip_polygon_to_rect_cancellable(
                    &polygon,
                    x.start,
                    x.end,
                    y.start,
                    y.end,
                    &|| false,
                )
                .unwrap();
                partitioned += polygon_moments(&clipped).area;
            }
        }
        assert!((partitioned - polygon_moments(&polygon).area).abs() < 1e-12);
    }

    /// Verifies batch sampling charges footprint work across regions and returns no partial table.
    #[test]
    fn stage20q_area_average_batch_exhausts_aggregate_cell_budget() {
        let field = stage20q_field(
            2,
            2,
            vec![
                SourcePixel {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0
                };
                4
            ],
        );
        let regions = vec![stage20q_rectangle(0.0, 1.0, 0.0, 1.0); 2];
        let error = sample_region_area_average_batch(
            &field,
            &regions,
            &CanvasSpec {
                width: 1.0,
                height: 1.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Alpha),
            RegionSamplingLimits {
                max_cell_intersections: 1,
                ..RegionSamplingLimits::default()
            },
            &|| false,
        )
        .unwrap_err();
        assert_eq!(
            error.path(),
            "sampling.region_average.limits.cell_intersections"
        );
    }

    /// Proves indexed AreaAverage workers preserve samples, workloads, and bounded failure order.
    #[test]
    fn stage20q_parallel_area_average_matches_single_worker_reference() {
        let field = stage20q_field(
            2,
            2,
            vec![
                SourcePixel {
                    red: 0.2,
                    green: 0.4,
                    blue: 0.8,
                    alpha: 0.75,
                };
                4
            ],
        );
        let regions = (0..64)
            .map(|index| {
                let inset = f64::from(index % 8) / 128.0;
                stage20q_rectangle(inset, 1.0 - inset, inset, 1.0 - inset)
            })
            .collect::<Vec<_>>();
        let sample = || {
            sample_region_area_average_batch_with_diagnostics(
                &field,
                &regions,
                &CanvasSpec {
                    width: 1.0,
                    height: 1.0,
                },
                SourceMapping::canonical(SourceMappingComponent::Alpha),
                RegionSamplingLimits::default(),
                &|| false,
            )
            .expect("complete AreaAverage batch evaluates")
        };
        let one = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("one-worker pool builds")
            .install(sample);
        let many = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .expect("four-worker pool builds")
            .install(sample);
        assert_eq!(one, many);
        let limited = |workers| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .expect("bounded sampling pool builds")
                .install(|| {
                    sample_region_area_average_batch_with_diagnostics(
                        &field,
                        &regions,
                        &CanvasSpec {
                            width: 1.0,
                            height: 1.0,
                        },
                        SourceMapping::canonical(SourceMappingComponent::Alpha),
                        RegionSamplingLimits {
                            max_cell_intersections: 10,
                            ..RegionSamplingLimits::default()
                        },
                        &|| false,
                    )
                    .expect_err("aggregate cell budget rejects the batch")
                })
        };
        let expected = limited(1);
        for _ in 0..8 {
            assert_eq!(limited(4), expected);
        }
    }

    /// Proves AreaAverage progress is completed-footprint based, monotonic, bounded, and complete.
    #[test]
    fn stage21a_area_average_progress_is_per_mille_coalesced() {
        let field = stage20q_field(
            8,
            8,
            vec![
                SourcePixel {
                    red: 0.25,
                    green: 0.5,
                    blue: 0.75,
                    alpha: 1.0,
                };
                64
            ],
        );
        let regions = vec![stage20q_rectangle(0.0, 1.0, 0.0, 1.0); 16];
        let updates = std::sync::Mutex::new(Vec::new());
        sample_region_area_average_batch_with_diagnostics_and_progress(
            &field,
            &regions,
            &CanvasSpec {
                width: 1.0,
                height: 1.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Alpha),
            RegionSamplingLimits::default(),
            &|| false,
            &|completed, total| {
                updates
                    .lock()
                    .expect("progress lock")
                    .push((completed, total));
            },
        )
        .expect("progressed AreaAverage batch evaluates");
        let updates = updates.into_inner().expect("progress lock");
        assert!(!updates.is_empty());
        assert!(updates.len() <= 1_000);
        assert!(updates.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert_eq!(updates.last().copied(), Some((1_000, 1_000)));
    }

    /// Verifies cancellation is reported before any area-average candidate can publish.
    #[test]
    fn stage20q_area_average_cancellation_is_exact() {
        let field = stage20q_field(
            2,
            2,
            vec![
                SourcePixel {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0
                };
                4
            ],
        );
        let error = sample_region_area_average(
            &field,
            &stage20q_rectangle(0.0, 1.0, 0.0, 1.0),
            &CanvasSpec {
                width: 1.0,
                height: 1.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Alpha),
            RegionSamplingLimits::default(),
            &|| true,
        )
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
    }

    /// Produces deterministic alpha-preserving PNG proxies for both immutable still baselines.
    ///
    /// This test exercises the sampling-owned decoder/resizer boundary only. It never mutates
    /// either source asset, constructs a document, or claims proxy bytes are export authority.
    #[test]
    fn reduced_preview_png_bounds_raster_and_svg_baselines_deterministically() {
        for (name, hint, expected) in [
            ("raster-sample.png", SourceFormatHint::Png, (128, 128)),
            ("vector-sample.svg", SourceFormatHint::Svg, (128, 88)),
        ] {
            let bytes = asset(name);
            let first = reduced_preview_png(&bytes, hint, 128)
                .expect("immutable baseline builds a bounded preview proxy");
            let second = reduced_preview_png(&bytes, hint, 128)
                .expect("immutable baseline repeats the bounded preview proxy");
            assert_eq!((first.width, first.height), expected);
            assert_eq!(first, second, "{name} proxy stays deterministic");
            let decoded = decode_source(&first.png_bytes, SourceFormatHint::Png)
                .expect("generated proxy remains supported PNG bytes");
            assert_eq!(
                (decoded.identity().width, decoded.identity().height),
                expected
            );
            assert!(
                decoded
                    .pixels
                    .iter()
                    .all(|pixel| pixel.alpha.is_finite() && (0.0..=1.0).contains(&pixel.alpha))
            );
        }
    }

    /// Keeps direct private-preview resampling bounded by proxy bytes without a second source-sized RGBA image.
    #[test]
    fn stage21a_reduced_preview_resampler_uses_only_bounded_proxy_storage() {
        let source = stage20q_field(
            4_096,
            1,
            vec![
                SourcePixel {
                    red: 0.25,
                    green: 0.5,
                    blue: 0.75,
                    alpha: 0.6,
                };
                4_096
            ],
        );
        let proxy = bounded_preview_rgba(&source, 128, 1)
            .expect("bounded proxy directly resamples decoded source pixels");
        assert_eq!(proxy.len(), 128 * 4);
        assert!(proxy.len() < source.pixels.len());
        assert!(
            proxy
                .chunks_exact(4)
                .all(|pixel| pixel == [64, 128, 191, 153])
        );
    }

    /// Computes the ZIP CRC32 value required by small synthetic PNG builders.
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xffff_ffff_u32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
}
