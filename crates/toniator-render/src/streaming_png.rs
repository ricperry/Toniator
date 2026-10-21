//! Global canonical preparation and bounded row-band PNG consumption.

use super::*;
use crate::GeometryOutputPart;
use std::io::{self, Write};

// Two f64 linear RGBA buffers plus the final straight RGBA bytes, with headroom below 64 MiB.
const PIXEL_STORAGE_BUDGET: u64 = 63 * 1024 * 1024;
const WORKING_BYTES_PER_PIXEL: u64 = 68;
const PNG_MAX_DIMENSION: u32 = i32::MAX as u32;

/// Checked PNG dimensions independent of full-surface preview and calibration admission.
///
/// Admission checks PNG axes, platform byte arithmetic, and space for at least one working row.
/// Geometry, row-edge scratch, source decoding, and compression are outside the pixel-storage budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StreamingRasterTarget {
    width: u32,
    height: u32,
}

impl StreamingRasterTarget {
    /// Checks format dimensions and row addressability without allocating a complete raster.
    ///
    /// # Errors
    /// Returns `png.target` for invalid axes, platform arithmetic, or an infeasible working row.
    pub fn new(width: u32, height: u32) -> Result<Self, RenderError> {
        if width == 0 || height == 0 || width > PNG_MAX_DIMENSION || height > PNG_MAX_DIMENSION {
            return Err(RenderError::new(
                "png.target",
                "PNG axes must be positive and at most 2,147,483,647 pixels",
            ));
        }
        if u64::from(width) * WORKING_BYTES_PER_PIXEL > PIXEL_STORAGE_BUDGET
            || usize::try_from(u64::from(width) * WORKING_BYTES_PER_PIXEL).is_err()
        {
            return Err(RenderError::new(
                "png.target",
                "one output row exceeds the streaming pixel-storage budget",
            ));
        }
        Ok(Self { width, height })
    }

    /// Resolves native integral canvas dimensions without applying a full-raster pixel ceiling.
    ///
    /// # Errors
    /// Returns native-canvas or streaming dimension diagnostics without changing the canvas.
    pub fn for_canvas(canvas: &CanvasSpec) -> Result<Self, RenderError> {
        Self::new(
            integral_dimension(canvas.width)?,
            integral_dimension(canvas.height)?,
        )
    }

    /// Returns the exact requested output width.
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Returns the exact requested output height.
    pub const fn height(self) -> u32 {
        self.height
    }

    /// Returns exact requested pixels without allocating or imposing a complete-raster limit.
    pub const fn pixel_count(self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Chooses a positive band height within the pixel-storage budget and caller's row ceiling.
    fn band_rows(self, maximum: u32) -> u32 {
        (PIXEL_STORAGE_BUDGET / (u64::from(self.width) * WORKING_BYTES_PER_PIXEL))
            .min(u64::from(self.height))
            .min(u64::from(maximum)) as u32
    }
}

/// Final-consumer PNG options; none changes canonical geometry, scene identity, or calibration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PngStreamOptions {
    pub background: RasterBackground,
    pub antialiasing: RasterAntialiasing,
    /// Thresholds quantized alpha at 128 before applying captured backing; no global cleanup occurs.
    pub binary_alpha: bool,
    pub pixels_per_metre: Option<u32>,
    /// Bounds rows per band further; the shared pixel-storage budget always takes precedence.
    pub maximum_band_rows: u32,
}

impl Default for PngStreamOptions {
    /// Supplies transparent ordinary AA output with the shared pixel-storage budget.
    fn default() -> Self {
        Self {
            background: RasterBackground::Transparent,
            antialiasing: RasterAntialiasing::On,
            binary_alpha: false,
            pixels_per_metre: None,
            maximum_band_rows: u32::MAX,
        }
    }
}

/// Completed PNG diagnostics; pixel storage is bounded independently of retained global geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PngStreamSummary {
    pub target: StreamingRasterTarget,
    pub band_rows: u32,
    /// Conservative maximum live pixel-buffer bytes; this is not a process RSS bound.
    pub maximum_band_pixel_bytes: u64,
    /// Retained prepared vector capacity, excluding allocator metadata and integration scratch.
    pub prepared_geometry_bytes: usize,
}

/// Prepares geometry once and streams exact straight-sRGBA bands to a caller-owned PNG sink.
///
/// Full output coordinates govern flattening, Boolean normalization, sample phase, and paint order.
/// The resolved finite positive target frame is checked before geometry materialization.
/// Progress reports completed global rows after preparation and after each encoded band. The sink
/// can contain partial PNG bytes on error; callers publish only a successful completed staging file.
/// Optional binary alpha is local pixel preparation and never applies global artistic cleanup.
///
/// # Errors
/// Returns invalid options, request-wide geometry limits/precision/allocation, cancellation, or
/// encoding/write errors. Both PNG writer layers finish explicitly before success is returned.
pub fn write_png_streaming<W: Write>(
    scene: &RenderScene,
    target: StreamingRasterTarget,
    options: PngStreamOptions,
    limits: RasterizationLimits,
    sink: W,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    report_rows: &(dyn Fn(u32, u32) + Sync),
) -> Result<PngStreamSummary, RenderError> {
    if options.maximum_band_rows == 0 || options.pixels_per_metre == Some(0) {
        return Err(RenderError::new(
            "png.options",
            "band rows and optional pixel density must be positive",
        ));
    }
    let mut work = RasterWork::with_progress(
        limits,
        options.antialiasing,
        is_cancelled,
        0,
        &ignore_raster_progress,
    );
    work.check()?;
    let transform = CanonicalRasterTransform {
        scale_x: f64::from(target.width) / scene.canvas.width,
        scale_y: f64::from(target.height) / scene.canvas.height,
        offset_x: 0.0,
        offset_y: 0.0,
    };
    transform
        .validate()
        .map_err(|_| RenderError::new("png.target", "output transform is not finite"))?;
    let prepared_scene = super::materialization::prepare_scene(scene, is_cancelled)?;
    let scene = prepared_scene.as_ref();

    let mut layers = Vec::new();
    for layer in &scene.layers {
        work.check()?;
        layers.try_reserve(1).map_err(|_| {
            RenderError::new("raster.allocation", "prepared layer allocation failed")
        })?;
        layers.push(match options.antialiasing {
            RasterAntialiasing::On => PreparedLayer::Area(area_raster::prepare_area_layer(
                layer,
                target.width,
                target.height,
                transform,
                &mut work,
            )?),
            RasterAntialiasing::Off => {
                PreparedLayer::Sampled(prepare_sampled_layer(layer, target, transform, &mut work)?)
            }
        });
    }
    let band_rows = target.band_rows(options.maximum_band_rows);
    let summary = PngStreamSummary {
        target,
        band_rows,
        maximum_band_pixel_bytes: u64::from(target.width)
            * u64::from(band_rows)
            * WORKING_BYTES_PER_PIXEL,
        prepared_geometry_bytes: layers
            .capacity()
            .saturating_mul(std::mem::size_of::<PreparedLayer>())
            .saturating_add(
                layers
                    .iter()
                    .map(PreparedLayer::storage_bytes)
                    .sum::<usize>(),
            ),
    };
    report_rows(0, target.height);
    work.check()?;
    let mut encoder = png::Encoder::new(
        CancellableSink {
            inner: sink,
            is_cancelled,
        },
        target.width,
        target.height,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    if let Some(density) = options.pixels_per_metre {
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: density,
            yppu: density,
            unit: png::Unit::Meter,
        }));
    }
    let mut writer = encoder
        .write_header()
        .map_err(|_| png_error(is_cancelled))?;
    {
        let mut stream = writer
            .stream_writer()
            .map_err(|_| png_error(is_cancelled))?;
        let mut first_row = 0;
        while first_row < target.height {
            work.check()?;
            let end_row = first_row.saturating_add(band_rows).min(target.height);
            let rows = end_row - first_row;
            let mut pixels = match scene.model {
                Some(model) => begin_model_composition(model, target.width, rows)?,
                None => begin_source_over_composition(target.width, rows)?,
            };
            for layer in &layers {
                work.check()?;
                let mut transient = allocate_linear_pixels(
                    target.width,
                    rows,
                    background_pixel(RasterBackground::Transparent),
                )?;
                match layer {
                    PreparedLayer::Area(layer) => layer.paint_band(
                        &mut transient,
                        target.width,
                        target.height,
                        first_row,
                        end_row,
                        &mut work,
                    )?,
                    PreparedLayer::Sampled(layer) => paint_sampled_band(
                        layer,
                        &mut transient,
                        target.width,
                        first_row,
                        end_row,
                        &mut work,
                    )?,
                }
                match scene.model {
                    Some(model) => {
                        compose_model_layer(model, &mut pixels, &transient, is_cancelled)?
                    }
                    None => compose_source_over_layer(&mut pixels, &transient, is_cancelled)?,
                }
            }
            if let Some(model) = scene.model {
                finish_model_composition(model, &mut pixels, is_cancelled)?;
            }
            if !options.binary_alpha {
                apply_background(&mut pixels, options.background, is_cancelled)?;
            }
            let mut surface = pixels_from_linear(target.width, rows, pixels, is_cancelled)?;
            if options.binary_alpha {
                threshold_band(&mut surface.pixels, options.background, is_cancelled)?;
            }
            work.check()?;
            stream
                .write_all(surface.pixels())
                .map_err(|_| png_error(is_cancelled))?;
            first_row = end_row;
            report_rows(first_row, target.height);
        }
        work.check()?;
        stream.finish().map_err(|_| png_error(is_cancelled))?;
    }
    work.check()?;
    writer.finish().map_err(|_| png_error(is_cancelled))?;
    work.check()?;
    Ok(summary)
}

/// Keeps distinct AA-on unioned fills and the original AA-off sampled coverage paths.
enum PreparedLayer {
    Area(area_raster::PreparedAreaLayer),
    Sampled(Vec<SampledPaint>),
}

impl PreparedLayer {
    /// Reports retained vector capacity without claiming a bound on process or allocator memory.
    fn storage_bytes(&self) -> usize {
        match self {
            Self::Area(layer) => layer.storage_bytes(),
            Self::Sampled(paints) => paints
                .capacity()
                .saturating_mul(std::mem::size_of::<SampledPaint>())
                .saturating_add(
                    paints
                        .iter()
                        .map(|paint| match &paint.shape {
                            SampledShape::Ellipse(_) => 0,
                            SampledShape::Polygon { edges, .. } => edges
                                .capacity()
                                .saturating_mul(std::mem::size_of::<(Point2, Point2)>()),
                            SampledShape::Stroke(starts) => starts
                                .capacity()
                                .saturating_mul(std::mem::size_of::<OutlineRowEdgeStart>()),
                        })
                        .sum::<usize>(),
                ),
        }
    }
}

/// One original AA-off primitive with captured paint and complete-target pixel bounds.
struct SampledPaint {
    shape: SampledShape,
    bounds: [u32; 4],
    color: ColorValue,
    opacity: f64,
}

/// Retains AA-off primitives and certified sampled decisions without substituting area AA.
enum SampledShape {
    Ellipse(MappedSampledEllipse),
    Polygon {
        edges: Vec<(Point2, Point2)>,
        even_odd: bool,
    },
    Stroke(Vec<OutlineRowEdgeStart>),
}

/// Computes original complete-target sampled bounds with shared finite-arithmetic validation.
/// # Errors
/// Returns the sampled-numeric diagnostic before support is clamped or cast into pixel indices.
fn sample_bounds(min: Point2, max: Point2, margin: f64, target: StreamingRasterTarget) -> Result<[u32; 4], RenderError> {
    sampled_raster_bounds(min, max, margin, target.width, target.height)
}

/// Appends one original sampled primitive using fallible storage growth and unchanged paint.
///
/// # Errors
/// Returns cancellation or allocation errors before exposing a prepared layer.
fn push_sampled(
    paints: &mut Vec<SampledPaint>,
    shape: SampledShape,
    bounds: [u32; 4],
    color: &ColorValue,
    opacity: f64,
    work: &RasterWork<'_>,
) -> Result<(), RenderError> {
    work.check()?;
    paints.try_reserve(1).map_err(|_| {
        RenderError::new(
            "raster.allocation",
            "prepared sampled paint allocation failed",
        )
    })?;
    paints.push(SampledPaint {
        shape,
        bounds,
        color: color.clone(),
        opacity,
    });
    Ok(())
}

/// Captures one mapped ellipse with authored-zero authority and outward canonical support.
/// The nominal distance test remains intact after provably outside samples are excluded.
///
/// # Errors
/// Returns cancellation or fallible prepared-storage allocation errors.
#[expect(
    clippy::too_many_arguments,
    reason = "original ellipse and capture authorities remain explicit"
)]
/// Returns `raster.sample.numeric` for nonfinite mapped/support or sampled flattening arithmetic.
fn prepare_ellipse(
    paints: &mut Vec<SampledPaint>,
    center: Point2,
    radius: f64,
    transform: CanonicalRasterTransform,
    target: StreamingRasterTarget,
    color: &ColorValue,
    opacity: f64,
    work: &RasterWork<'_>,
) -> Result<(), RenderError> {
    work.check()?;
    let Some(ellipse) = mapped_sampled_ellipse(center, radius, transform)? else {
        return Ok(());
    };
    let bounds = sampled_ellipse_bounds(
        ellipse.center.x,
        ellipse.center.y,
        ellipse.radius_x,
        ellipse.radius_y,
        target.width,
        target.height,
    )?;
    push_sampled(
        paints,
        SampledShape::Ellipse(ellipse),
        bounds,
        color,
        opacity,
        work,
    )
}

/// Flattens each original AA-off path once under the same request-wide edge authority.
///
/// Marks retain their canonical fill rule, regions retain nonzero point tests, and strokes retain the
/// existing active-edge scanline arithmetic. Canonical source geometry and clipping remain intact.
///
/// # Errors
/// Returns cancellation, flattening limits/precision, or fallible allocation errors.
/// Returns `raster.sample.numeric` for nonfinite mapped/support or sampled flattening arithmetic.
fn prepare_sampled_layer(
    layer: &RenderLayer,
    target: StreamingRasterTarget,
    transform: CanonicalRasterTransform,
    work: &mut RasterWork<'_>,
) -> Result<Vec<SampledPaint>, RenderError> {
    let mut paints = Vec::new();
    if !layer.visible {
        return Ok(paints);
    }
    for output in &layer.outputs {
        work.check()?;
        for part in output.geometry().parts() {
            match part {
                GeometryOutputPart::CircularMarks(marks) => {
                    for (index, mark) in marks.iter().enumerate() {
                        prepare_ellipse(
                            &mut paints,
                            mark.center,
                            mark.radius,
                            transform,
                            target,
                            output_primitive_paint(layer, Some(output), index),
                            layer.opacity,
                            work,
                        )?;
                    }
                }
                GeometryOutputPart::CanonicalMarks(marks) => {
                    for (index, mark) in marks.iter().enumerate() {
                        let color = output_primitive_paint(layer, Some(output), index);
                        match mark {
CanonicalMark::DeferredPath(_) => {},

                            CanonicalMark::Circle { center, radius, .. } => prepare_ellipse(
                                &mut paints,
                                *center,
                                *radius,
                                transform,
                                target,
                                color,
                                layer.opacity,
                                work,
                            )?,
                            CanonicalMark::ClosedPath(path) => {
                                let edges = flattened_path_edges(&path.path, transform, work)?;
                                let bounds = sample_bounds(
                                    transform.point(path.bounds.min),
                                    transform.point(path.bounds.max),
                                    1.0 / 64.0,
                                    target,
                                )?;
                                push_sampled(
                                    &mut paints,
                                    SampledShape::Polygon {
                                        edges,
                                        even_odd: path.fill_rule == CanonicalFillRule::EvenOdd,
                                    },
                                    bounds,
                                    color,
                                    layer.opacity,
                                    work,
                                )?;
                            }
                        }
                    }
                }
                GeometryOutputPart::CanonicalRegions(regions) => {
                    for (index, region) in regions.regions().iter().enumerate() {
                        let edges = flattened_path_edges(&region.ring, transform, work)?;
                        let bounds = sample_bounds(
                            transform.point(region.bounds.min),
                            transform.point(region.bounds.max),
                            1.0,
                            target,
                        )?;
                        push_sampled(
                            &mut paints,
                            SampledShape::Polygon {
                                edges,
                                even_odd: false,
                            },
                            bounds,
                            output_primitive_paint(layer, Some(output), index),
                            layer.opacity,
                            work,
                        )?;
                    }
                }
                GeometryOutputPart::CanonicalStrokes(strokes) => {
                    for stroke in strokes {
                        let Some(bounds) = stroke.outline.bounds else {
                            continue;
                        };
                        let bounds = sample_bounds(
                            transform.point(bounds.min),
                            transform.point(bounds.max),
                            1.0,
                            target,
                        )?;
                        let edges = flattened_outline_edges(&stroke.outline, transform, work)?;
                        let starts = outline_row_edge_starts(&edges, bounds[1], bounds[3], work)?;
                        push_sampled(
                            &mut paints,
                            SampledShape::Stroke(starts),
                            bounds,
                            &layer.color,
                            layer.opacity,
                            work,
                        )?;
                    }
                }
            }
        }
    }
    Ok(paints)
}

/// Paints captured AA-off primitives at global sample centers into local row-band storage.
///
/// # Errors
/// Returns cancellation or existing sampling/scratch allocation diagnostics.
fn paint_sampled_band(
    paints: &[SampledPaint],
    pixels: &mut Vec<PremultipliedLinearPixel>,
    width: u32,
    first_row: u32,
    end_row: u32,
    work: &mut RasterWork<'_>,
) -> Result<(), RenderError> {
    for paint in paints {
        work.check()?;
        let [x0, y0, x1, y1] = paint.bounds;
        let start = y0.max(first_row);
        let end = y1.min(end_row);
        if start >= end || x0 >= x1 {
            continue;
        }
        match &paint.shape {
            SampledShape::Stroke(starts) => {
                paint_stroke_band(starts, paint, pixels, width, first_row, start, end, work)?
            }
            shape => {
                for y in start..end {
                    work.check()?;
                    for x in x0..x1 {
                        let coverage = match shape {
                            SampledShape::Ellipse(ellipse) => ellipse_coverage(
                                *ellipse,
                                x,
                                y,
                                RasterAntialiasing::Off,
                                Some(work),
                            )?,
                            SampledShape::Polygon {
                                edges,
                                even_odd: true,
                            } => polygon_coverage_even_odd(edges, x, y, work)?,
                            SampledShape::Polygon {
                                edges,
                                even_odd: false,
                            } => polygon_coverage_nonzero(edges, x, y, work)?,
                            SampledShape::Stroke(_) => {
                                return Err(RenderError::new(
                                    "raster.composition",
                                    "stroke dispatch requires its prepared scanline path",
                                ));
                            }
                        };
                        if coverage > 0.0 {
                            let index = (y - first_row) as usize * width as usize + x as usize;
                            if matches!(shape, SampledShape::Ellipse(_)) {
                                pixels.paint_ellipse(index, coverage, &paint.color, paint.opacity);
                            } else {
                                pixels.paint_shape(index, coverage, &paint.color, paint.opacity);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Reuses the original stroke scanline solver without translating or flattening band geometry.
///
/// # Errors
/// Returns cancellation or fallible active-edge and coverage scratch allocation failures.
#[expect(
    clippy::too_many_arguments,
    reason = "global scanline bounds remain separate from local storage"
)]
fn paint_stroke_band(
    starts: &[OutlineRowEdgeStart],
    paint: &SampledPaint,
    pixels: &mut Vec<PremultipliedLinearPixel>,
    width: u32,
    first_row: u32,
    start: u32,
    end: u32,
    work: &RasterWork<'_>,
) -> Result<(), RenderError> {
    let mut edges = Vec::new();
    let mut ends = Vec::new();
    edges
        .try_reserve_exact(starts.len())
        .map_err(|_| RenderError::new("raster.allocation", "stroke row-edge allocation failed"))?;
    ends.try_reserve_exact(starts.len())
        .map_err(|_| RenderError::new("raster.allocation", "stroke row-end allocation failed"))?;
    let mut covered = Vec::new();
    let mut crossings = Vec::new();
    let mut uncertain = Vec::new();
    let mut next = 0;
    for y in start..end {
        work.check()?;
        advance_outline_edges_for_pixel_row(starts, &mut next, y, &mut edges, &mut ends);
        let _ = accumulate_nonzero_scanline_coverage(
            &edges,
            y,
            paint.bounds[0],
            paint.bounds[2],
            1,
            &mut covered,
            &mut crossings,
            &mut uncertain,
            work,
        )?;
        for &(x, coverage) in &covered {
            pixels.paint_shape(
                (y - first_row) as usize * width as usize + x as usize,
                f64::from(coverage),
                &paint.color,
                paint.opacity,
            );
        }
    }
    Ok(())
}

/// Applies the existing prepared-PNG byte threshold before backing without allocating another band.
///
/// # Errors
/// Returns cancellation before mutating another bounded pixel interval.
fn threshold_band(
    pixels: &mut [u8],
    background: RasterBackground,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), RenderError> {
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        if index % 4096 == 0 {
            check_parallel_raster_cancellation(is_cancelled)?;
        }
        if pixel[3] < 128 {
            pixel.copy_from_slice(&match background {
                RasterBackground::Transparent => [0, 0, 0, 0],
                RasterBackground::OpaqueBlack => [0, 0, 0, 255],
                RasterBackground::OpaqueWhite => [255, 255, 255, 255],
            });
        } else {
            pixel[3] = 255;
        }
    }
    Ok(())
}

/// Polls cancellation at the compression sink as well as geometric and pixel boundaries.
struct CancellableSink<'a, W> {
    inner: W,
    is_cancelled: &'a (dyn Fn() -> bool + Sync),
}

impl<W: Write> Write for CancellableSink<'_, W> {
    /// Writes an encoded chunk only while its owning export remains current.
    ///
    /// # Errors
    /// Returns cancellation as a non-retried IO error or preserves the underlying write failure.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if (self.is_cancelled)() {
            return Err(io::Error::other("PNG export cancelled"));
        }
        self.inner.write(bytes)
    }

    /// Flushes encoded chunks only while the owning export remains current.
    ///
    /// # Errors
    /// Returns cancellation or the underlying flush failure.
    fn flush(&mut self) -> io::Result<()> {
        if (self.is_cancelled)() {
            return Err(io::Error::other("PNG export cancelled"));
        }
        self.inner.flush()
    }
}

/// Distinguishes cancellation from a failed PNG encoding or sink operation.
fn png_error(is_cancelled: &(dyn Fn() -> bool + Sync)) -> RenderError {
    if is_cancelled() {
        RenderError::new("evaluation.cancelled", "PNG export was cancelled")
    } else {
        RenderError::new("png.encode", "could not complete streaming PNG output")
    }
}
