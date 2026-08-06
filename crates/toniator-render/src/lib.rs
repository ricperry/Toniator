#![forbid(unsafe_code)]

//! Headless consumers for immutable canonical circle geometry.
//!
//! `RenderScene` deliberately knows nothing about source artwork, sampling, or
//! pattern settings. Raster compositing happens in linear premultiplied RGBA;
//! `RasterSurface` exposes only straight sRGBA bytes at the output boundary.

use std::{collections::HashSet, error::Error, fmt};

use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use toniator_domain::{CanvasSpec, ChannelId, ColorValue};
use toniator_geometry::CanonicalCircleMark;

const SUBPIXEL_GRID: u32 = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct RenderScene {
    canvas: CanvasSpec,
    layers: Vec<RenderLayer>,
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
}

#[derive(Clone, Debug, PartialEq)]
pub enum GeometryOutput {
    CircularMarks(Vec<CanonicalCircleMark>),
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
        let mut channel_ids = HashSet::new();
        for layer in &layers {
            validate_layer(layer)?;
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
            &layers,
        );
        Ok(Self {
            canvas,
            layers,
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
    Ok(())
}

fn scene_fingerprint(
    canvas: &CanvasSpec,
    family: &str,
    realization: &str,
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
    let width = integral_dimension(scene.canvas.width)?;
    let height = integral_dimension(scene.canvas.height)?;
    let background = match background {
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
    };
    let mut linear_pixels = vec![background; width as usize * height as usize];
    for layer in &scene.layers {
        if !layer.visible {
            continue;
        }
        let GeometryOutput::CircularMarks(marks) = &layer.geometry;
        for mark in marks {
            composite_circle(
                &mut linear_pixels,
                width,
                height,
                mark,
                &layer.color,
                layer.opacity,
            );
        }
    }
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
