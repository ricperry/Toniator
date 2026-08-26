#![forbid(unsafe_code)]

//! Byte-oriented source decoding and deterministic source-field sampling.

use std::{error::Error, fmt};

use image::{ImageFormat, ImageReader};
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
pub const DECODER_CONTRACT_ID: &str = "toniator-sampling-decoder-v2-linear-source-fields";

/// The only source formats supported by the bounded Stage 4 decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceFormat {
    Png,
    Svg,
}

/// A caller-supplied decoding hint. Decoding never opens a filesystem path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFormatHint {
    Png,
    Svg,
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
    /// Caps decoded-source cell intersection work for a complete request.
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

/// Deterministically averages one untreated closed region through the complete edge-clamped field.
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
    cancelled: &dyn Fn() -> bool,
) -> Result<RegionSourceSample, SamplingError> {
    validate_canvas(canvas)?;
    validate_mapping(mapping)?;
    let mut work = RegionSamplingWork::new(limits, cancelled)?;
    sample_region_area_average_with_work(field, region, canvas, mapping, &mut work)
}

/// Deterministically averages every supplied untreated base region with one shared work budget.
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
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RegionSourceSample>, SamplingError> {
    validate_canvas(canvas)?;
    validate_mapping(mapping)?;
    let mut work = RegionSamplingWork::new(limits, cancelled)?;
    let mut samples = Vec::new();
    samples.try_reserve(regions.len()).map_err(|_| {
        SamplingError::new(
            "sampling.region_average.allocation.samples",
            "region sample allocation failed",
        )
    })?;
    for region in regions {
        work.poll()?;
        samples.push(sample_region_area_average_with_work(
            field, region, canvas, mapping, &mut work,
        )?);
    }
    Ok(samples)
}

/// Integrates one flattened region over all intersected source cells and exterior clamp bands.
///
/// The caller owns `work`, which makes every flattening and cell charge request-wide. Scalar
/// response and associated RGB/alpha are integrated against the same exact polygon moments.
///
/// # Errors
///
/// Returns stable sampling failures and never exposes a partially accumulated sample.
fn sample_region_area_average_with_work(
    field: &SourceField,
    region: &CurvePath,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    work: &mut RegionSamplingWork<'_>,
) -> Result<RegionSourceSample, SamplingError> {
    let polygon = flatten_region_source_space_with_work(
        region,
        canvas,
        field.identity.width,
        field.identity.height,
        work,
    )?;
    let complete = polygon_moments(&polygon);
    if !complete.area.is_finite() || complete.area.abs() <= f64::EPSILON {
        return Err(SamplingError::new(
            "sampling.region_average.geometry",
            "region average requires nonzero finite area",
        ));
    }
    let (minimum, maximum) = polygon_bounds(&polygon)?;
    let xs = source_axis_intervals(minimum.x, maximum.x, field.identity.width)?;
    let ys = source_axis_intervals(minimum.y, maximum.y, field.identity.height)?;
    let mut totals = [0.0; 5];
    for y in ys {
        for x in &xs {
            work.poll()?;
            work.charge_cell()?;
            let clipped = clip_polygon_to_rect_cancellable(
                &polygon,
                x.start,
                x.end,
                y.start,
                y.end,
                work.cancelled,
            )?;
            if clipped.is_empty() {
                continue;
            }
            let moments = translate_polygon_moments(
                polygon_moments(&clipped),
                Point2::new(f64::from(x.cell), f64::from(y.cell)),
            );
            let values = region_cell_values(field, mapping, *x, y);
            for (total, coefficients) in totals.iter_mut().zip(values) {
                *total += coefficients[0] * moments.area
                    + coefficients[1] * moments.mx
                    + coefficients[2] * moments.my
                    + coefficients[3] * moments.mxy;
            }
        }
    }
    let orientation = complete.area.signum();
    let area = complete.area.abs();
    let response = (totals[0] * orientation / area).clamp(0.0, 1.0);
    let alpha = (totals[4] * orientation / area).clamp(0.0, 1.0);
    let paint = (alpha > 0.0).then(|| SampledSourcePaint {
        red: (totals[1] * orientation / area / alpha).clamp(0.0, 1.0),
        green: (totals[2] * orientation / area / alpha).clamp(0.0, 1.0),
        blue: (totals[3] * orientation / area / alpha).clamp(0.0, 1.0),
        alpha: 1.0,
    });
    Ok(RegionSourceSample { response, paint })
}

/// Flattens one closed region into unclamped decoded-source coordinates.
///
/// The transform intentionally preserves off-source geometry for later exterior clamp-band
/// integration. Cubics use ordered `t = 0.5` De Casteljau subdivision with a `1/64` pixel
/// chord tolerance and never append a duplicate closure point.
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
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Point2>, SamplingError> {
    let mut work = RegionSamplingWork::new(limits, cancelled)?;
    flatten_region_source_space_with_work(region, canvas, source_width, source_height, &mut work)
}

/// Flattens one closed boundary into source space while charging the caller-owned request budget.
///
/// Source dimensions determine the transform; no pixel data is read before exact cell
/// integration. Off-canvas coordinates remain unclamped so the exterior clamp bands retain area.
///
/// # Errors
///
/// Returns geometry, allocation, cancellation, or request-wide flattening-limit diagnostics.
fn flatten_region_source_space_with_work(
    region: &CurvePath,
    canvas: &CanvasSpec,
    source_width: u32,
    source_height: u32,
    work: &mut RegionSamplingWork<'_>,
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
            point.x * f64::from(source_width.saturating_sub(1)) / canvas.width,
            point.y * f64::from(source_height.saturating_sub(1)) / canvas.height,
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
    work: &mut RegionSamplingWork<'_>,
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
    work: &mut RegionSamplingWork<'_>,
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

/// Tracks request-wide flattened-segment and cell-intersection work without publishing samples.
#[allow(dead_code)]
struct RegionSamplingWork<'a> {
    limits: RegionSamplingLimits,
    cancelled: &'a dyn Fn() -> bool,
    flattened_segments: usize,
    cell_intersections: usize,
}

#[allow(dead_code)]
impl<'a> RegionSamplingWork<'a> {
    /// Builds a shared nonzero work budget for a complete region-sampling request.
    fn new(
        limits: RegionSamplingLimits,
        cancelled: &'a dyn Fn() -> bool,
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
            flattened_segments: 0,
            cell_intersections: 0,
        })
    }
    /// Polls cancellation using the canonical evaluation failure path.
    fn poll(&self) -> Result<(), SamplingError> {
        poll_region(self.cancelled)
    }
    /// Charges one emitted flattened chord across every region using this request.
    fn charge_flattened(&mut self) -> Result<(), SamplingError> {
        self.flattened_segments =
            self.flattened_segments
                .checked_add(1)
                .ok_or(SamplingError::new(
                    "sampling.region_average.limits.flattening",
                    "flattened segment counter overflowed",
                ))?;
        if self.flattened_segments > self.limits.max_flattened_segments {
            return Err(SamplingError::new(
                "sampling.region_average.limits.flattening",
                "flattened segment limit exceeded",
            ));
        }
        Ok(())
    }
    /// Charges one candidate cell intersection across every region using this request.
    fn charge_cell(&mut self) -> Result<(), SamplingError> {
        self.cell_intersections =
            self.cell_intersections
                .checked_add(1)
                .ok_or(SamplingError::new(
                    "sampling.region_average.limits.cell_intersections",
                    "cell intersection counter overflowed",
                ))?;
        if self.cell_intersections > self.limits.max_cell_intersections {
            return Err(SamplingError::new(
                "sampling.region_average.limits.cell_intersections",
                "cell intersection limit exceeded",
            ));
        }
        Ok(())
    }
}

/// One finite source-space interval paired with its edge-clamped bilinear-cell index.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
struct SourceAxisInterval {
    start: f64,
    end: f64,
    cell: u32,
}

/// Enumerates low exterior, ordered interior cells, then high exterior for one source axis.
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
    if !minimum.is_finite() || !maximum.is_finite() || maximum < minimum || extent == 0 {
        return Err(SamplingError::new(
            "sampling.region_average.geometry",
            "source interval bounds must be finite and ordered",
        ));
    }
    let mut result = Vec::new();
    result.try_reserve(3).map_err(|_| {
        SamplingError::new(
            "sampling.region_average.allocation.partition",
            "source interval allocation failed",
        )
    })?;
    if extent == 1 {
        result.push(SourceAxisInterval {
            start: minimum,
            end: maximum,
            cell: 0,
        });
        return Ok(result);
    }
    let last = f64::from(extent - 1);
    if minimum < 0.0 {
        result.push(SourceAxisInterval {
            start: minimum,
            end: maximum.min(0.0),
            cell: 0,
        });
    }
    let first = minimum.max(0.0).floor() as u32;
    let end = maximum.min(last).ceil() as u32;
    for cell in first..end.min(extent - 1) {
        let start = minimum.max(f64::from(cell));
        let finish = maximum.min(f64::from(cell) + 1.0);
        if finish > start {
            result.push(SourceAxisInterval {
                start,
                end: finish,
                cell,
            });
        }
    }
    if maximum > last {
        result.push(SourceAxisInterval {
            start: minimum.max(last),
            end: maximum,
            cell: extent - 2,
        });
    }
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

/// Computes scalar and associated RGBA bilinear coefficients for one partition rectangle.
///
/// Coefficients operate on local coordinates relative to the selected source cell. Exterior
/// bands collapse the corresponding coordinate to the required edge-clamped zero or one value.
fn region_cell_values(
    field: &SourceField,
    mapping: SourceMapping,
    x: SourceAxisInterval,
    y: SourceAxisInterval,
) -> [[f64; 4]; 5] {
    let x1 = (x.cell + 1).min(field.identity.width - 1);
    let y1 = (y.cell + 1).min(field.identity.height - 1);
    let pixels = [
        field.pixel(x.cell, y.cell).expect("validated source pixel"),
        field.pixel(x1, y.cell).expect("validated source pixel"),
        field.pixel(x.cell, y1).expect("validated source pixel"),
        field.pixel(x1, y1).expect("validated source pixel"),
    ];
    let mut result = [[0.0; 4]; 5];
    let scalar = pixels.map(|pixel| mapped_response(pixel, mapping));
    result[0] = bilinear_coefficients(scalar, x, y, field.identity.width, field.identity.height);
    for channel in 0..4 {
        result[channel + 1] = bilinear_coefficients(
            pixels.map(|pixel| associated_linear(pixel)[channel]),
            x,
            y,
            field.identity.width,
            field.identity.height,
        );
    }
    result
}

/// Converts four grid-corner values into local bilinear coefficients with edge-clamp collapse.
fn bilinear_coefficients(
    values: [f64; 4],
    x: SourceAxisInterval,
    y: SourceAxisInterval,
    width: u32,
    height: u32,
) -> [f64; 4] {
    let x_mode = interval_axis_mode(x, width);
    let y_mode = interval_axis_mode(y, height);
    match (x_mode, y_mode) {
        (AxisMode::Variable, AxisMode::Variable) => [
            values[0],
            values[1] - values[0],
            values[2] - values[0],
            values[3] - values[1] - values[2] + values[0],
        ],
        (AxisMode::Zero, AxisMode::Variable) => [values[0], 0.0, values[2] - values[0], 0.0],
        (AxisMode::One, AxisMode::Variable) => [values[1], 0.0, values[3] - values[1], 0.0],
        (AxisMode::Variable, AxisMode::Zero) => [values[0], values[1] - values[0], 0.0, 0.0],
        (AxisMode::Variable, AxisMode::One) => [values[2], values[3] - values[2], 0.0, 0.0],
        (AxisMode::Zero, AxisMode::Zero) => [values[0], 0.0, 0.0, 0.0],
        (AxisMode::One, AxisMode::Zero) => [values[1], 0.0, 0.0, 0.0],
        (AxisMode::Zero, AxisMode::One) => [values[2], 0.0, 0.0, 0.0],
        (AxisMode::One, AxisMode::One) => [values[3], 0.0, 0.0, 0.0],
    }
}

/// Distinguishes a variable interior source coordinate from a clamped exterior coordinate.
fn interval_axis_mode(interval: SourceAxisInterval, extent: u32) -> AxisMode {
    if extent <= 1 || interval.end <= 0.0 {
        AxisMode::Zero
    } else if interval.start >= f64::from(extent - 1) {
        AxisMode::One
    } else {
        AxisMode::Variable
    }
}

/// Names the coordinate behavior for one source partition interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AxisMode {
    Zero,
    Variable,
    One,
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
    let mut output = polygon.to_vec();
    for edge in [
        ClipEdge::Left(left),
        ClipEdge::Right(right),
        ClipEdge::Bottom(bottom),
        ClipEdge::Top(top),
    ] {
        poll_region(cancelled)?;
        let input = std::mem::take(&mut output);
        if input.is_empty() {
            return Ok(Vec::new());
        }
        for (start, end) in input
            .iter()
            .zip(input.iter().cycle().skip(1))
            .take(input.len())
        {
            poll_region(cancelled)?;
            let start_inside = edge.contains(*start);
            let end_inside = edge.contains(*end);
            if start_inside {
                push_distinct(&mut output, *start);
            }
            if start_inside != end_inside {
                push_distinct(&mut output, edge.intersection(*start, *end));
            }
        }
        if output.len() > 1 && output.first() == output.last() {
            output.pop();
        }
    }
    if output.len() < 3 || polygon_moments(&output).area.abs() <= f64::EPSILON {
        Ok(Vec::new())
    } else {
        Ok(output)
    }
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
        SourceFormatHint::Unsupported => Err(SamplingError::new(
            "source.format",
            "only PNG and SVG source formats are supported",
        )),
    }
}

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

/// Returns the requested Stage 9 scalar field for one decoded straight-sRGB
/// pixel. RGB and CMYK are always calculated in linear light and CMY is the
/// unnormalized full-UCR separation, not a `(1-K)` normalized variant.
pub fn mapping_component_value(pixel: SourcePixel, component: SourceMappingComponent) -> f64 {
    let (red, green, blue) = linear_rgb(pixel);
    let black = (1.0 - red.max(green).max(blue)).clamp(0.0, 1.0);
    match component {
        SourceMappingComponent::Red => red,
        SourceMappingComponent::Green => green,
        SourceMappingComponent::Blue => blue,
        SourceMappingComponent::Cyan => (1.0 - red - black).clamp(0.0, 1.0),
        SourceMappingComponent::Magenta => (1.0 - green - black).clamp(0.0, 1.0),
        SourceMappingComponent::Yellow => (1.0 - blue - black).clamp(0.0, 1.0),
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

    #[test]
    fn stage9_linear_rgb_and_full_ucr_fields_cover_synthetic_colors() {
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
        let white = field.pixel(1, 0).unwrap();
        assert_eq!(component(white, SourceMappingComponent::Black), 0.0);
        assert_eq!(component(white, SourceMappingComponent::Cyan), 0.0);
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
        assert!(component(gray, SourceMappingComponent::Luminance) > 0.21);
        let chromatic_midtone = field.pixel(9, 0).unwrap();
        let linear_red = srgb_to_linear(0.8);
        let linear_green = srgb_to_linear(0.4);
        let linear_blue = srgb_to_linear(0.2);
        let chromatic_black = 1.0 - linear_red.max(linear_green).max(linear_blue);
        let unnormalized_magenta = 1.0 - linear_green - chromatic_black;
        let normalized_magenta = unnormalized_magenta / (1.0 - chromatic_black);
        assert!(
            (component(chromatic_midtone, SourceMappingComponent::Black) - chromatic_black).abs()
                < 1e-12
        );
        assert!(
            (component(chromatic_midtone, SourceMappingComponent::Magenta) - unnormalized_magenta)
                .abs()
                < 1e-12
        );
        assert!(
            (component(chromatic_midtone, SourceMappingComponent::Yellow)
                - (1.0 - linear_blue - chromatic_black))
                .abs()
                < 1e-12
        );
        assert!(
            (component(chromatic_midtone, SourceMappingComponent::Magenta) - normalized_magenta)
                .abs()
                > 0.1,
            "full UCR CMY is intentionally not normalized by (1-K)"
        );
        assert_eq!(
            DECODER_CONTRACT_ID,
            "toniator-sampling-decoder-v2-linear-source-fields"
        );
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
        assert_eq!(points[0], Point2::new(-5.0, 0.0));
        assert_eq!(points[1], Point2::new(15.0, 0.0));
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
        assert_eq!(first[0], Point2::new(0.0, 0.0));
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

    /// Verifies source interval enumeration orders low exterior, cells, and high exterior.
    #[test]
    fn stage20q_source_intervals_cover_all_clamp_bands_in_order() {
        let intervals = source_axis_intervals(-2.0, 4.0, 4).unwrap();
        assert_eq!(
            intervals,
            vec![
                SourceAxisInterval {
                    start: -2.0,
                    end: 0.0,
                    cell: 0
                },
                SourceAxisInterval {
                    start: 0.0,
                    end: 1.0,
                    cell: 0
                },
                SourceAxisInterval {
                    start: 1.0,
                    end: 2.0,
                    cell: 1
                },
                SourceAxisInterval {
                    start: 2.0,
                    end: 3.0,
                    cell: 2
                },
                SourceAxisInterval {
                    start: 3.0,
                    end: 4.0,
                    cell: 2
                }
            ]
        );
    }

    /// Verifies one-pixel source axes retain one finite constant clamp interval.
    #[test]
    fn stage20q_one_pixel_source_axis_has_one_clamp_interval() {
        assert_eq!(
            source_axis_intervals(-3.0, 7.0, 1).unwrap(),
            vec![SourceAxisInterval {
                start: -3.0,
                end: 7.0,
                cell: 0
            }]
        );
    }

    /// Verifies shared request counters aggregate limits and canonical cancellation failures.
    #[test]
    fn stage20q_sampling_work_is_request_wide_and_bounded() {
        let mut work = RegionSamplingWork::new(
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

    /// Builds a finite two-dimensional decoded field for exact Stage 20Q integration witnesses.
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

    /// Verifies exact moments integrate a unit-cell bilinear alpha field without point sampling.
    #[test]
    fn stage20q_area_average_integrates_bilinear_scalar_exactly() {
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

    /// Verifies complete off-canvas area is included through the finite exterior clamp bands.
    #[test]
    fn stage20q_area_average_integrates_complete_off_canvas_clamp_bands() {
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

    /// Verifies batch sampling charges cell work across regions and returns no partial table.
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
