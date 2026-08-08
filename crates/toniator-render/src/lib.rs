#![forbid(unsafe_code)]

//! Headless consumers for immutable canonical circle geometry.
//!
//! `RenderScene` deliberately knows nothing about source artwork, sampling, or
//! pattern settings. Raster compositing happens in linear premultiplied RGBA;
//! `RasterSurface` exposes only straight sRGBA bytes at the output boundary.

use std::{collections::HashSet, error::Error, fmt};

use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use toniator_domain::{CanvasSpec, ChannelId, ColorValue, HalftoneChannelModel};
use toniator_geometry::CanonicalCircleMark;

const SUBPIXEL_GRID: u32 = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct RenderScene {
    canvas: CanvasSpec,
    layers: Vec<RenderLayer>,
    /// `None` is the accepted Stage 5 single-layer scene contract. Modeled
    /// scenes opt into the fixed Stage 9C equations without reinterpreting
    /// existing callers before complete-document evaluation is authorized.
    model: Option<HalftoneChannelModel>,
    identity: SceneIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneIdentity {
    family_fingerprint: String,
    realization_fingerprint: String,
    scene_fingerprint: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderLayer {
    channel_id: ChannelId,
    visible: bool,
    /// Canonical linear RGBA. It is converted only at output boundaries.
    color: ColorValue,
    opacity: f64,
    geometry: GeometryOutput,
    /// SourceColorAlpha carries immutable straight-linear paint per canonical
    /// mark. Solid layers leave this as `None`.
    mark_paints: Option<Vec<ColorValue>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GeometryOutput {
    CircularMarks(Vec<CanonicalCircleMark>),
}

/// Renderer-owned immutable source-colored circle. Stage 9D may adapt the
/// accepted Stage 9B realization into this DTO without making rendering depend
/// on pattern realization or source sampling crates.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceColorCircle {
    pub mark: CanonicalCircleMark,
    pub paint: ColorValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderError {
    path: &'static str,
    message: &'static str,
}

impl RenderError {
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

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for RenderError {}

impl RenderScene {
    pub fn new(
        canvas: CanvasSpec,
        family_fingerprint: String,
        realization_fingerprint: String,
        layers: Vec<RenderLayer>,
    ) -> Result<Self, RenderError> {
        Self::build(
            canvas,
            family_fingerprint,
            realization_fingerprint,
            None,
            layers,
        )
    }

    /// Constructs a Stage 9C scene with fixed, non-selectable model semantics.
    pub fn new_modeled(
        canvas: CanvasSpec,
        family_fingerprint: String,
        realization_fingerprint: String,
        model: HalftoneChannelModel,
        layers: Vec<RenderLayer>,
    ) -> Result<Self, RenderError> {
        Self::build(
            canvas,
            family_fingerprint,
            realization_fingerprint,
            Some(model),
            layers,
        )
    }

    fn build(
        canvas: CanvasSpec,
        family_fingerprint: String,
        realization_fingerprint: String,
        model: Option<HalftoneChannelModel>,
        layers: Vec<RenderLayer>,
    ) -> Result<Self, RenderError> {
        validate_canvas(&canvas)?;
        if family_fingerprint.is_empty() || realization_fingerprint.is_empty() {
            return Err(RenderError::new(
                "scene.identity",
                "family and realization identities must not be empty",
            ));
        }
        if layers.is_empty() {
            return Err(RenderError::new(
                "scene.layers",
                "at least one layer is required",
            ));
        }
        if matches!(model, Some(HalftoneChannelModel::SourceColorAlpha)) && layers.len() != 1 {
            return Err(RenderError::new(
                "scene.layers",
                "SourceColorAlpha requires exactly one ordered source-colored layer",
            ));
        }
        let mut channel_ids = HashSet::new();
        for layer in &layers {
            validate_layer(layer)?;
            match model {
                None => {
                    if layer.mark_paints.is_some() {
                        return Err(RenderError::new(
                            "scene.layers",
                            "unmodeled legacy scenes cannot carry sampled per-mark paints",
                        ));
                    }
                }
                Some(HalftoneChannelModel::Rgb | HalftoneChannelModel::Cmyk) => {
                    if layer.mark_paints.is_some() {
                        return Err(RenderError::new(
                            "scene.layers",
                            "RGB and CMYK layers must use solid paint",
                        ));
                    }
                }
                Some(HalftoneChannelModel::SourceColorAlpha) => {
                    if layer.mark_paints.is_none() {
                        return Err(RenderError::new(
                            "scene.layers",
                            "SourceColorAlpha requires sampled per-mark paint",
                        ));
                    }
                }
            }
            if !channel_ids.insert(layer.channel_id) {
                return Err(RenderError::new(
                    "scene.layers",
                    "layer channel IDs must be unique while preserving supplied order",
                ));
            }
        }
        let scene_fingerprint = scene_fingerprint(
            &canvas,
            &family_fingerprint,
            &realization_fingerprint,
            model,
            &layers,
        );
        Ok(Self {
            canvas,
            layers,
            model,
            identity: SceneIdentity {
                family_fingerprint,
                realization_fingerprint,
                scene_fingerprint,
            },
        })
    }

    pub fn circular_mark_count(&self) -> usize {
        self.layers
            .iter()
            .map(|layer| match &layer.geometry {
                GeometryOutput::CircularMarks(marks) => marks.len(),
            })
            .sum()
    }

    pub fn canvas(&self) -> &CanvasSpec {
        &self.canvas
    }
    pub fn layers(&self) -> &[RenderLayer] {
        &self.layers
    }
    pub const fn model(&self) -> Option<HalftoneChannelModel> {
        self.model
    }
    pub fn identity(&self) -> &SceneIdentity {
        &self.identity
    }
}

impl SceneIdentity {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn realization_fingerprint(&self) -> &str {
        &self.realization_fingerprint
    }
    pub fn scene_fingerprint(&self) -> &str {
        &self.scene_fingerprint
    }
}

impl RenderLayer {
    pub fn new(
        channel_id: ChannelId,
        visible: bool,
        color: ColorValue,
        opacity: f64,
        geometry: GeometryOutput,
    ) -> Result<Self, RenderError> {
        let layer = Self {
            channel_id,
            visible,
            color,
            opacity,
            geometry,
            mark_paints: None,
        };
        validate_layer(&layer)?;
        Ok(layer)
    }

    /// Builds the one SourceColorAlpha layer from Stage 9B's immutable marked
    /// paint. The per-mark source alpha has already determined inclusion and
    /// color sampling; it is applied here exactly once with layer opacity.
    pub fn new_source_color(
        channel_id: ChannelId,
        visible: bool,
        opacity: f64,
        marks: Vec<SourceColorCircle>,
    ) -> Result<Self, RenderError> {
        let geometry = GeometryOutput::CircularMarks(
            marks
                .iter()
                .map(|source_mark| source_mark.mark.clone())
                .collect(),
        );
        let mark_paints = marks
            .into_iter()
            .map(|source_mark| ColorValue {
                red: source_mark.paint.red,
                green: source_mark.paint.green,
                blue: source_mark.paint.blue,
                alpha: source_mark.paint.alpha,
            })
            .collect();
        let layer = Self {
            channel_id,
            visible,
            color: ColorValue {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            opacity,
            geometry,
            mark_paints: Some(mark_paints),
        };
        validate_layer(&layer)?;
        Ok(layer)
    }

    pub const fn channel_id(&self) -> ChannelId {
        self.channel_id
    }
    pub const fn visible(&self) -> bool {
        self.visible
    }
    pub fn color(&self) -> &ColorValue {
        &self.color
    }
    pub const fn opacity(&self) -> f64 {
        self.opacity
    }
    pub fn geometry(&self) -> &GeometryOutput {
        &self.geometry
    }

    fn mark_paint(&self, index: usize) -> &ColorValue {
        self.mark_paints
            .as_ref()
            .and_then(|paints| paints.get(index))
            .unwrap_or(&self.color)
    }
}

fn validate_canvas(canvas: &CanvasSpec) -> Result<(), RenderError> {
    if !canvas.width.is_finite()
        || !canvas.height.is_finite()
        || canvas.width <= 0.0
        || canvas.height <= 0.0
    {
        return Err(RenderError::new(
            "scene.canvas",
            "canvas dimensions must be positive and finite",
        ));
    }
    Ok(())
}

fn validate_layer(layer: &RenderLayer) -> Result<(), RenderError> {
    for value in [
        layer.color.red,
        layer.color.green,
        layer.color.blue,
        layer.color.alpha,
        layer.opacity,
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(RenderError::new(
                "scene.layer.presentation",
                "color and opacity must be finite values within 0.0..=1.0",
            ));
        }
    }
    match &layer.geometry {
        GeometryOutput::CircularMarks(marks) => {
            if marks.iter().any(|mark| {
                !mark.center.is_finite() || !mark.radius.is_finite() || mark.radius < 0.0
            }) {
                return Err(RenderError::new(
                    "scene.layer.geometry",
                    "canonical circle geometry must be finite and nonnegative",
                ));
            }
        }
    }
    if let Some(paints) = &layer.mark_paints {
        let GeometryOutput::CircularMarks(marks) = &layer.geometry;
        if paints.len() != marks.len() {
            return Err(RenderError::new(
                "scene.layer.source_color",
                "source-colored paint count must match canonical mark count",
            ));
        }
        for paint in paints {
            for value in [paint.red, paint.green, paint.blue, paint.alpha] {
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(RenderError::new(
                        "scene.layer.source_color",
                        "source-colored paint must be finite values within 0.0..=1.0",
                    ));
                }
            }
            if paint.alpha != 1.0 {
                return Err(RenderError::new(
                    "scene.layer.source_color",
                    "sampled per-mark paint alpha must be exactly 1.0",
                ));
            }
        }
    }
    Ok(())
}

fn scene_fingerprint(
    canvas: &CanvasSpec,
    family: &str,
    realization: &str,
    model: Option<HalftoneChannelModel>,
    layers: &[RenderLayer],
) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    fn add(hash: &mut u64, bytes: impl IntoIterator<Item = u8>) {
        for byte in bytes {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    add(
        &mut hash,
        b"toniator-stage-5-render-scene-v2".iter().copied(),
    );
    add(&mut hash, family.bytes());
    add(&mut hash, realization.bytes());
    if let Some(model) = model {
        add(
            &mut hash,
            [match model {
                HalftoneChannelModel::Rgb => 1,
                HalftoneChannelModel::Cmyk => 2,
                HalftoneChannelModel::SourceColorAlpha => 3,
            }],
        );
    }
    add(&mut hash, canvas.width.to_bits().to_le_bytes());
    add(&mut hash, canvas.height.to_bits().to_le_bytes());
    // The complete scene identity includes ordered presentation. Family and
    // realization identities remain the independent geometry identities.
    for layer in layers {
        add(&mut hash, layer.channel_id.0.to_le_bytes());
        add(&mut hash, [u8::from(layer.visible)]);
        add(&mut hash, layer.color.red.to_bits().to_le_bytes());
        add(&mut hash, layer.color.green.to_bits().to_le_bytes());
        add(&mut hash, layer.color.blue.to_bits().to_le_bytes());
        add(&mut hash, layer.color.alpha.to_bits().to_le_bytes());
        add(&mut hash, layer.opacity.to_bits().to_le_bytes());
        match &layer.geometry {
            GeometryOutput::CircularMarks(marks) => {
                add(&mut hash, (marks.len() as u64).to_le_bytes());
                for mark in marks {
                    add(
                        &mut hash,
                        mark.source_site_id.first_dimension_id.to_le_bytes(),
                    );
                    add(&mut hash, mark.source_site_id.first_index.to_le_bytes());
                    add(
                        &mut hash,
                        mark.source_site_id.second_dimension_id.to_le_bytes(),
                    );
                    add(&mut hash, mark.source_site_id.second_index.to_le_bytes());
                    add(&mut hash, mark.center.x.to_bits().to_le_bytes());
                    add(&mut hash, mark.center.y.to_bits().to_le_bytes());
                    add(&mut hash, mark.radius.to_bits().to_le_bytes());
                    add(
                        &mut hash,
                        [match mark.scope {
                            toniator_geometry::SiteScope::Canvas => 1,
                            toniator_geometry::SiteScope::Guard => 2,
                        }],
                    );
                    for contributor in &mark.provenance.contributors {
                        add(&mut hash, contributor.dimension_id.to_le_bytes());
                        add(&mut hash, contributor.index.to_le_bytes());
                    }
                }
            }
        }
        if model.is_some() {
            if let Some(paints) = &layer.mark_paints {
                add(&mut hash, [1]);
                for paint in paints {
                    add(&mut hash, paint.red.to_bits().to_le_bytes());
                    add(&mut hash, paint.green.to_bits().to_le_bytes());
                    add(&mut hash, paint.blue.to_bits().to_le_bytes());
                    add(&mut hash, paint.alpha.to_bits().to_le_bytes());
                }
            } else {
                add(&mut hash, [0]);
            }
        }
    }
    format!("fnv1a64:{hash:016x}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterBackground {
    OpaqueBlack,
    OpaqueWhite,
    Transparent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterSurface {
    width: u32,
    height: u32,
    /// Straight, 8-bit sRGBA in row-major pixel order.
    pixels: Vec<u8>,
}

impl RasterSurface {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, RenderError> {
        if width == 0 || height == 0 {
            return Err(RenderError::new(
                "raster.surface",
                "dimensions must be positive",
            ));
        }
        if pixels.len() != width as usize * height as usize * 4 {
            return Err(RenderError::new(
                "raster.surface",
                "straight sRGBA buffer length does not match dimensions",
            ));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }
    pub const fn height(&self) -> u32 {
        self.height
    }
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

#[derive(Clone, Copy, Debug)]
struct PremultipliedLinearPixel {
    red: f64,
    green: f64,
    blue: f64,
    alpha: f64,
}

pub fn rasterize(
    scene: &RenderScene,
    background: RasterBackground,
) -> Result<RasterSurface, RenderError> {
    if scene.model.is_none() {
        return rasterize_stage5(scene, background);
    }

    let width = integral_dimension(scene.canvas.width)?;
    let height = integral_dimension(scene.canvas.height)?;
    let layer_pixels = scene
        .layers
        .iter()
        .map(|layer| rasterize_layer(layer, width, height))
        .collect::<Vec<_>>();
    let mut linear_pixels = compose_model(scene.model.expect("modeled scene"), &layer_pixels);
    apply_background(&mut linear_pixels, background);
    pixels_from_linear(width, height, linear_pixels)
}

/// Retains the accepted Stage 5 raster path byte-for-byte for callers which
/// have not opted into an explicit Stage 9C model.
fn rasterize_stage5(
    scene: &RenderScene,
    background: RasterBackground,
) -> Result<RasterSurface, RenderError> {
    let width = integral_dimension(scene.canvas.width)?;
    let height = integral_dimension(scene.canvas.height)?;
    let background = background_pixel(background);
    let mut linear_pixels = vec![background; width as usize * height as usize];
    for layer in &scene.layers {
        if !layer.visible {
            continue;
        }
        let GeometryOutput::CircularMarks(marks) = &layer.geometry;
        for (index, mark) in marks.iter().enumerate() {
            composite_circle(
                &mut linear_pixels,
                width,
                height,
                mark,
                layer.mark_paint(index),
                layer.opacity,
            );
        }
    }
    pixels_from_linear(width, height, linear_pixels)
}

fn background_pixel(background: RasterBackground) -> PremultipliedLinearPixel {
    match background {
        RasterBackground::OpaqueBlack => PremultipliedLinearPixel {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        RasterBackground::OpaqueWhite => PremultipliedLinearPixel {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
            alpha: 1.0,
        },
        RasterBackground::Transparent => PremultipliedLinearPixel {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        },
    }
}

fn rasterize_layer(layer: &RenderLayer, width: u32, height: u32) -> Vec<PremultipliedLinearPixel> {
    let mut pixels =
        vec![background_pixel(RasterBackground::Transparent); width as usize * height as usize];
    if !layer.visible {
        return pixels;
    }
    let GeometryOutput::CircularMarks(marks) = &layer.geometry;
    for (index, mark) in marks.iter().enumerate() {
        composite_circle(
            &mut pixels,
            width,
            height,
            mark,
            layer.mark_paint(index),
            layer.opacity,
        );
    }
    pixels
}

fn compose_model(
    model: HalftoneChannelModel,
    layers: &[Vec<PremultipliedLinearPixel>],
) -> Vec<PremultipliedLinearPixel> {
    let count = layers.first().map_or(0, Vec::len);
    match model {
        HalftoneChannelModel::Rgb => (0..count)
            .map(|index| {
                let mut pixel = background_pixel(RasterBackground::Transparent);
                for layer in layers {
                    let source = layer[index];
                    pixel.red = (pixel.red + source.red).clamp(0.0, 1.0);
                    pixel.green = (pixel.green + source.green).clamp(0.0, 1.0);
                    pixel.blue = (pixel.blue + source.blue).clamp(0.0, 1.0);
                    pixel.alpha = (pixel.alpha + source.alpha).clamp(0.0, 1.0);
                }
                pixel
            })
            .collect(),
        HalftoneChannelModel::Cmyk => (0..count)
            .map(|index| {
                let mut transmittance = [1.0; 3];
                let mut uncovered = 1.0;
                for layer in layers {
                    let source = layer[index];
                    if source.alpha > 0.0 {
                        let straight = [
                            source.red / source.alpha,
                            source.green / source.alpha,
                            source.blue / source.alpha,
                        ];
                        for component in 0..3 {
                            transmittance[component] *=
                                1.0 - source.alpha * (1.0 - straight[component]);
                        }
                        uncovered *= 1.0 - source.alpha;
                    }
                }
                let alpha = (1.0 - uncovered).clamp(0.0, 1.0);
                PremultipliedLinearPixel {
                    red: boundary_clamp(transmittance[0] - (1.0 - alpha)),
                    green: boundary_clamp(transmittance[1] - (1.0 - alpha)),
                    blue: boundary_clamp(transmittance[2] - (1.0 - alpha)),
                    alpha,
                }
            })
            .collect(),
        HalftoneChannelModel::SourceColorAlpha => (0..count)
            .map(|index| {
                let mut destination = background_pixel(RasterBackground::Transparent);
                for layer in layers {
                    source_over(&mut destination, layer[index]);
                }
                destination
            })
            .collect(),
    }
}

fn boundary_clamp(value: f64) -> f64 {
    const EPSILON: f64 = 1e-12;
    if (-EPSILON..0.0).contains(&value) {
        0.0
    } else if (1.0..=1.0 + EPSILON).contains(&value) {
        1.0
    } else {
        value
    }
}

fn source_over(destination: &mut PremultipliedLinearPixel, source: PremultipliedLinearPixel) {
    let remaining = 1.0 - source.alpha;
    destination.red = source.red + destination.red * remaining;
    destination.green = source.green + destination.green * remaining;
    destination.blue = source.blue + destination.blue * remaining;
    destination.alpha = source.alpha + destination.alpha * remaining;
}

fn apply_background(pixels: &mut [PremultipliedLinearPixel], background: RasterBackground) {
    if matches!(background, RasterBackground::Transparent) {
        return;
    }
    let background = background_pixel(background);
    for pixel in pixels {
        let remaining = 1.0 - pixel.alpha;
        pixel.red += background.red * remaining;
        pixel.green += background.green * remaining;
        pixel.blue += background.blue * remaining;
        pixel.alpha = 1.0;
    }
}

fn pixels_from_linear(
    width: u32,
    height: u32,
    linear_pixels: Vec<PremultipliedLinearPixel>,
) -> Result<RasterSurface, RenderError> {
    let mut pixels = Vec::with_capacity(linear_pixels.len() * 4);
    for pixel in linear_pixels {
        let alpha = pixel.alpha.clamp(0.0, 1.0);
        let (red, green, blue) = if alpha == 0.0 {
            (0.0, 0.0, 0.0)
        } else {
            (pixel.red / alpha, pixel.green / alpha, pixel.blue / alpha)
        };
        pixels.extend([
            quantize_srgb(red),
            quantize_srgb(green),
            quantize_srgb(blue),
            quantize_linear(alpha),
        ]);
    }
    RasterSurface::new(width, height, pixels)
}

fn integral_dimension(value: f64) -> Result<u32, RenderError> {
    if !value.is_finite() || value <= 0.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
        return Err(RenderError::new(
            "raster.canvas",
            "canvas dimensions must be positive finite integral document units",
        ));
    }
    Ok(value as u32)
}

fn composite_circle(
    pixels: &mut [PremultipliedLinearPixel],
    width: u32,
    height: u32,
    mark: &CanonicalCircleMark,
    color: &ColorValue,
    opacity: f64,
) {
    let min_x = (mark.center.x - mark.radius - 1.0).floor().max(0.0) as u32;
    let min_y = (mark.center.y - mark.radius - 1.0).floor().max(0.0) as u32;
    let max_x = (mark.center.x + mark.radius + 1.0)
        .ceil()
        .min(f64::from(width)) as u32;
    let max_y = (mark.center.y + mark.radius + 1.0)
        .ceil()
        .min(f64::from(height)) as u32;
    for y in min_y..max_y {
        for x in min_x..max_x {
            let coverage = circle_coverage(mark, x, y);
            if coverage == 0.0 {
                continue;
            }
            let source_alpha = (color.alpha * opacity * coverage).clamp(0.0, 1.0);
            let destination = &mut pixels[y as usize * width as usize + x as usize];
            let remaining = 1.0 - source_alpha;
            destination.red = color.red * source_alpha + destination.red * remaining;
            destination.green = color.green * source_alpha + destination.green * remaining;
            destination.blue = color.blue * source_alpha + destination.blue * remaining;
            destination.alpha = source_alpha + destination.alpha * remaining;
        }
    }
}

fn circle_coverage(mark: &CanonicalCircleMark, x: u32, y: u32) -> f64 {
    let mut inside = 0_u32;
    for sample_y in 0..SUBPIXEL_GRID {
        for sample_x in 0..SUBPIXEL_GRID {
            let point_x = f64::from(x) + (f64::from(sample_x) + 0.5) / f64::from(SUBPIXEL_GRID);
            let point_y = f64::from(y) + (f64::from(sample_y) + 0.5) / f64::from(SUBPIXEL_GRID);
            let dx = point_x - mark.center.x;
            let dy = point_y - mark.center.y;
            if dx.mul_add(dx, dy * dy) <= mark.radius * mark.radius {
                inside += 1;
            }
        }
    }
    f64::from(inside) / f64::from(SUBPIXEL_GRID * SUBPIXEL_GRID)
}

pub fn encode_png(surface: &RasterSurface) -> Result<Vec<u8>, RenderError> {
    let mut output = Vec::new();
    PngEncoder::new(&mut output)
        .write_image(
            surface.pixels(),
            surface.width(),
            surface.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|_| RenderError::new("png.encode", "could not encode RasterSurface"))?;
    Ok(output)
}

/// Converts canonical linear RGB to sRGB at a presentation/output boundary.
pub fn linear_to_srgb(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

/// Converts sRGB to canonical linear RGB at a presentation/input boundary.
pub fn srgb_to_linear(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn quantize_srgb(value: f64) -> u8 {
    (linear_to_srgb(value) * 255.0).round() as u8
}
fn quantize_linear(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub fn write_svg(scene: &RenderScene) -> String {
    match scene.model {
        None => write_stage5_svg(scene),
        Some(model) => write_modeled_svg(scene, model),
    }
}

fn write_stage5_svg(scene: &RenderScene) -> String {
    let width = compact_number(scene.canvas.width);
    let height = compact_number(scene.canvas.height);
    let mut document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n<metadata>family={};realization={};scene={}</metadata>\n<defs><clipPath id=\"canvas-clip\"><rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\"/></clipPath></defs>\n",
        xml_escape(&scene.identity.family_fingerprint),
        xml_escape(&scene.identity.realization_fingerprint),
        xml_escape(&scene.identity.scene_fingerprint),
    );
    for layer in &scene.layers {
        if !layer.visible {
            continue;
        }
        let GeometryOutput::CircularMarks(marks) = &layer.geometry;
        let color = color_hex(&layer.color);
        let opacity = compact_number(layer.color.alpha * layer.opacity);
        document.push_str(&format!("<g id=\"channel-{}\" clip-path=\"url(#canvas-clip)\" fill=\"{color}\" fill-opacity=\"{opacity}\">\n", layer.channel_id.0));
        for mark in marks {
            document.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>\n",
                compact_number(mark.center.x),
                compact_number(mark.center.y),
                compact_number(mark.radius)
            ));
        }
        document.push_str("</g>\n");
    }
    document.push_str("</svg>\n");
    document
}

fn write_modeled_svg(scene: &RenderScene, model: HalftoneChannelModel) -> String {
    let width = compact_number(scene.canvas.width);
    let height = compact_number(scene.canvas.height);
    let title = match model {
        HalftoneChannelModel::Rgb => "Toniator RGB halftone",
        HalftoneChannelModel::Cmyk => "Toniator CMYK halftone",
        HalftoneChannelModel::SourceColorAlpha => "Toniator source-colored halftone",
    };
    let mut document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n<title>{title}</title>\n<metadata>family={};realization={};scene={}</metadata>\n<defs><clipPath id=\"canvas-clip\"><rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\"/></clipPath></defs>\n",
        xml_escape(&scene.identity.family_fingerprint),
        xml_escape(&scene.identity.realization_fingerprint),
        xml_escape(&scene.identity.scene_fingerprint),
    );
    document.push_str(
        "<g id=\"canvas\" clip-path=\"url(#canvas-clip)\" style=\"isolation:isolate\">\n",
    );
    let blend_mode = match model {
        HalftoneChannelModel::Rgb => Some("screen"),
        HalftoneChannelModel::Cmyk => Some("multiply"),
        HalftoneChannelModel::SourceColorAlpha => None,
    };
    for layer in &scene.layers {
        write_svg_channel_group(&mut document, layer, blend_mode);
    }
    document.push_str("</g>\n");
    document.push_str("</svg>\n");
    document
}

fn write_svg_channel_group(document: &mut String, layer: &RenderLayer, blend_mode: Option<&str>) {
    let mut styles = Vec::new();
    if let Some(mode) = blend_mode {
        styles.push(format!("mix-blend-mode:{mode}"));
    }
    if !layer.visible {
        styles.push("display:none".to_owned());
    }
    let style = (!styles.is_empty()).then(|| format!(" style=\"{}\"", styles.join(";")));
    document.push_str(&format!(
        "<g id=\"channel-{}\"{}>\n",
        layer.channel_id.0,
        style.unwrap_or_default(),
    ));
    let GeometryOutput::CircularMarks(marks) = &layer.geometry;
    for (index, mark) in marks.iter().enumerate() {
        let paint = layer.mark_paint(index);
        document.push_str(&format!(
            "<circle id=\"channel-{}-mark-{index}\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" fill-opacity=\"{}\"/>\n",
            layer.channel_id.0,
            compact_number(mark.center.x),
            compact_number(mark.center.y),
            compact_number(mark.radius),
            color_hex(paint),
            compact_number(paint.alpha * layer.opacity),
        ));
    }
    document.push_str("</g>\n");
}

fn color_hex(color: &ColorValue) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        quantize_srgb(color.red),
        quantize_srgb(color.green),
        quantize_srgb(color.blue)
    )
}

fn compact_number(value: f64) -> String {
    let mut text = format!("{value:.6}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
