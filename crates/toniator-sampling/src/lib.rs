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
use toniator_geometry::Point2;

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
