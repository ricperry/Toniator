use crate::CancellationToken;
use crate::model::{
    Document, DocumentAppearance, ExportBackground, Ink, OutputMode, PreviewSurface, RenderVariant,
    RgbaColor, Settings, SourceArtwork, Treatment, ValueMode, WebShape, WebShapeChannel,
    WebShapeSettings, parse_hex_color,
};
use anyhow::{Context, Result, bail};
use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage, imageops::FilterType};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use tiny_skia::{BlendMode, Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cmyk {
    pub c: f32,
    pub m: f32,
    pub y: f32,
    pub k: f32,
}

pub fn rgb_to_cmyk(r: u8, g: u8, b: u8) -> Cmyk {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let k = 1.0 - r.max(g).max(b);
    if k >= 0.999 {
        return Cmyk {
            c: 0.0,
            m: 0.0,
            y: 0.0,
            k: 1.0,
        };
    }
    let denominator = 1.0 - k;
    Cmyk {
        c: ((1.0 - r - k) / denominator).clamp(0.0, 1.0),
        m: ((1.0 - g - k) / denominator).clamp(0.0, 1.0),
        y: ((1.0 - b - k) / denominator).clamp(0.0, 1.0),
        k: k.clamp(0.0, 1.0),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Cyan,
    Magenta,
    Yellow,
    Black,
    Red,
    Green,
    Blue,
}

impl From<Ink> for Channel {
    fn from(value: Ink) -> Self {
        match value {
            Ink::Cyan => Self::Cyan,
            Ink::Magenta => Self::Magenta,
            Ink::Yellow => Self::Yellow,
            Ink::Black => Self::Black,
            Ink::Red => Self::Red,
            Ink::Green => Self::Green,
            Ink::Blue => Self::Blue,
        }
    }
}

impl From<Channel> for Ink {
    fn from(value: Channel) -> Self {
        match value {
            Channel::Cyan => Self::Cyan,
            Channel::Magenta => Self::Magenta,
            Channel::Yellow => Self::Yellow,
            Channel::Black => Self::Black,
            Channel::Red => Self::Red,
            Channel::Green => Self::Green,
            Channel::Blue => Self::Blue,
        }
    }
}

impl Channel {
    pub const ALL: [Channel; 4] = [
        Channel::Cyan,
        Channel::Magenta,
        Channel::Yellow,
        Channel::Black,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Channel::Cyan => "cyan",
            Channel::Magenta => "magenta",
            Channel::Yellow => "yellow",
            Channel::Black => "black",
            Channel::Red => "red",
            Channel::Green => "green",
            Channel::Blue => "blue",
        }
    }

    pub fn color(self) -> (u8, u8, u8) {
        match self {
            Channel::Cyan => (0, 174, 239),
            Channel::Magenta => (236, 0, 140),
            Channel::Yellow => (255, 242, 0),
            Channel::Black => (20, 20, 24),
            Channel::Red => (255, 0, 0),
            Channel::Green => (0, 255, 0),
            Channel::Blue => (0, 0, 255),
        }
    }

    fn angle_offset(self) -> f32 {
        match self {
            Channel::Cyan => 15.0,
            Channel::Magenta => 75.0,
            Channel::Yellow => 0.0,
            Channel::Black => 45.0,
            Channel::Red | Channel::Green | Channel::Blue => 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub channel: Channel,
    pub x: f32,
    pub y: f32,
    pub extent: f32,
    pub thickness: f32,
    pub angle: f32,
    pub treatment: Treatment,
    pub geometry: MarkGeometry,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarkGeometry {
    Native,
    WebShape(ResolvedWebShape),
}

pub type CubicShapeSegment = (f32, f32, f32, f32, f32, f32);

#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedWebShape {
    Circle,
    Polygon(Arc<[(f32, f32)]>),
    Cubic {
        start: (f32, f32),
        segments: Arc<[CubicShapeSegment]>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct InkLayer {
    pub channel: Channel,
    pub enabled: bool,
    pub color: (u8, u8, u8),
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkSet {
    pub width: u32,
    pub height: u32,
    pub marks: Vec<Mark>,
    pub layers: Vec<InkLayer>,
}

pub fn decode_source(source: &SourceArtwork, max_dimension: u32) -> Result<RgbaImage> {
    let media = source.media_type.to_ascii_lowercase();
    let mut decoded = if media.contains("svg") || source.name.to_ascii_lowercase().ends_with(".svg")
    {
        decode_svg(&source.bytes, max_dimension)?
    } else {
        image::load_from_memory(&source.bytes)
            .with_context(|| format!("could not decode {}", source.name))?
            .to_rgba8()
    };

    if decoded.width().max(decoded.height()) > max_dimension {
        let scale = max_dimension as f32 / decoded.width().max(decoded.height()) as f32;
        let width = (decoded.width() as f32 * scale).round().max(1.0) as u32;
        let height = (decoded.height() as f32 * scale).round().max(1.0) as u32;
        decoded = image::imageops::resize(&decoded, width, height, FilterType::Lanczos3);
    }
    Ok(decoded)
}

pub fn source_dimensions(source: &SourceArtwork) -> Result<(u32, u32)> {
    let media = source.media_type.to_ascii_lowercase();
    if media.contains("svg") || source.name.to_ascii_lowercase().ends_with(".svg") {
        let options = svg_options();
        let tree = usvg::Tree::from_data(&source.bytes, &options)
            .context("could not parse SVG artwork")?;
        let size = tree.size();
        let width = size.width().round().max(1.0) as u32;
        let height = size.height().round().max(1.0) as u32;
        return Ok((width, height));
    }
    let reader = image::ImageReader::new(Cursor::new(&source.bytes))
        .with_guessed_format()
        .context("could not identify artwork format")?;
    reader
        .into_dimensions()
        .with_context(|| format!("could not read dimensions of {}", source.name))
}

fn decode_svg(bytes: &[u8], max_dimension: u32) -> Result<RgbaImage> {
    let options = svg_options();
    let tree = usvg::Tree::from_data(bytes, &options).context("could not parse SVG artwork")?;
    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 {
        bail!("SVG artwork has no usable size");
    }
    let scale = (max_dimension as f32 / size.width().max(size.height())).min(1.0);
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut pixmap = Pixmap::new(width, height).context("SVG artwork is too large")?;
    let transform = Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, pixmap.take())
        .context("SVG rasterizer returned an invalid buffer")
}

/// SVG text needs an explicit font database. `usvg::Options::default()` owns an
/// empty database, which otherwise makes valid `<text>` elements disappear.
/// System font discovery is relatively expensive, so all parses share one
/// immutable database after its first initialization.
fn svg_options() -> usvg::Options<'static> {
    usvg::Options {
        font_family: default_font_family().to_owned(),
        fontdb: Arc::clone(system_font_database()),
        ..usvg::Options::default()
    }
}

fn system_font_database() -> &'static Arc<usvg::fontdb::Database> {
    static FONT_DATABASE: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONT_DATABASE.get_or_init(|| {
        let mut database = usvg::fontdb::Database::new();
        database.load_system_fonts();

        let sans = installed_family(
            &database,
            &["Noto Sans", "DejaVu Sans", "Liberation Sans", "Cantarell"],
            false,
        );
        let serif = installed_family(
            &database,
            &["Noto Serif", "DejaVu Serif", "Liberation Serif"],
            false,
        )
        .or_else(|| sans.clone());
        let mono = installed_family(
            &database,
            &["Noto Sans Mono", "DejaVu Sans Mono", "Liberation Mono"],
            true,
        )
        .or_else(|| sans.clone());
        let cursive = installed_family(
            &database,
            &["Comic Sans MS", "URW Chancery L", "TeX Gyre Chorus"],
            false,
        )
        .or_else(|| sans.clone());
        let fantasy = installed_family(
            &database,
            &["Impact", "Noto Sans Display", "DejaVu Sans"],
            false,
        )
        .or_else(|| sans.clone());

        if let Some(family) = serif {
            database.set_serif_family(family);
        }
        if let Some(family) = sans {
            database.set_sans_serif_family(family);
        }
        if let Some(family) = mono {
            database.set_monospace_family(family);
        }
        if let Some(family) = cursive {
            database.set_cursive_family(family);
        }
        if let Some(family) = fantasy {
            database.set_fantasy_family(family);
        }

        Arc::new(database)
    })
}

fn default_font_family() -> &'static str {
    static DEFAULT_FAMILY: OnceLock<String> = OnceLock::new();
    DEFAULT_FAMILY
        .get_or_init(|| {
            installed_family(
                system_font_database(),
                &["Noto Sans", "DejaVu Sans", "Liberation Sans", "Cantarell"],
                false,
            )
            .or_else(|| first_installed_family(system_font_database(), false))
            .unwrap_or_else(|| "sans-serif".to_owned())
        })
        .as_str()
}

fn installed_family(
    database: &usvg::fontdb::Database,
    preferences: &[&str],
    monospaced: bool,
) -> Option<String> {
    preferences.iter().find_map(|preference| {
        database
            .faces()
            .filter(|face| face.monospaced == monospaced)
            .flat_map(|face| face.families.iter())
            .find(|(name, _)| name.eq_ignore_ascii_case(preference))
            .map(|(name, _)| name.clone())
    })
}

fn first_installed_family(database: &usvg::fontdb::Database, monospaced: bool) -> Option<String> {
    database
        .faces()
        .find(|face| face.monospaced == monospaced)
        .and_then(|face| face.families.first())
        .map(|(name, _)| name.clone())
}

pub fn generate_marks(source: &RgbaImage, settings: Settings) -> MarkSet {
    generate_marks_cancellable(source, settings, &CancellationToken::new())
        .expect("fresh cancellation token cannot cancel")
}

pub fn generate_marks_cancellable(
    source: &RgbaImage,
    settings: Settings,
    token: &CancellationToken,
) -> Result<MarkSet> {
    let settings = settings.sanitized();
    let longest = source.width().max(source.height()) as f32;
    let desired_cells = 22.0 + settings.detail * 0.92;
    let spacing = (longest / desired_cells).max(3.0);
    let columns = (source.width() as f32 / spacing).ceil() as u32;
    let rows = (source.height() as f32 / spacing).ceil() as u32;
    let mut marks = Vec::with_capacity((columns * rows * 3) as usize);
    let coverage = settings.coverage / 100.0;
    let contrast = settings.contrast / 100.0;

    for row in 0..rows {
        token.checkpoint()?;
        for column in 0..columns {
            let x = ((column as f32 + 0.5) * spacing).min(source.width() as f32 - 0.5);
            let y = ((row as f32 + 0.5) * spacing).min(source.height() as f32 - 0.5);
            let pixel = source.get_pixel(x as u32, y as u32).0;
            if pixel[3] == 0 {
                continue;
            }
            let cmyk = rgb_to_cmyk(pixel[0], pixel[1], pixel[2]);
            let values = [cmyk.c, cmyk.m, cmyk.y, cmyk.k];
            for (channel, value) in Channel::ALL.into_iter().zip(values) {
                let value =
                    ((value - 0.5) * contrast + 0.5).clamp(0.0, 1.0) * (pixel[3] as f32 / 255.0);
                if value <= 0.006 {
                    continue;
                }
                let (extent, thickness) = match settings.treatment {
                    Treatment::Dots => (spacing * 0.47 * coverage * value.sqrt(), 0.0),
                    Treatment::Squares => (spacing * 0.86 * coverage * value.sqrt(), 0.0),
                    Treatment::Lines => {
                        (spacing * 1.12, spacing * 0.72 * coverage * value.max(0.025))
                    }
                };
                marks.push(Mark {
                    channel,
                    x,
                    y,
                    extent,
                    thickness,
                    angle: settings.angle + channel.angle_offset(),
                    treatment: settings.treatment,
                    geometry: MarkGeometry::Native,
                });
            }
        }
    }

    Ok(MarkSet {
        width: source.width(),
        height: source.height(),
        marks,
        layers: Channel::ALL
            .into_iter()
            .map(|channel| InkLayer {
                channel,
                enabled: true,
                color: channel.color(),
                opacity: 205.0 / 255.0,
            })
            .collect(),
    })
}

fn generate_rgb_marks_cancellable(
    source: &RgbaImage,
    settings: Settings,
    token: &CancellationToken,
) -> Result<MarkSet> {
    let settings = settings.sanitized();
    let longest = source.width().max(source.height()) as f32;
    let spacing = (longest / (22.0 + settings.detail * 0.92)).max(3.0);
    let columns = (source.width() as f32 / spacing).ceil() as u32;
    let rows = (source.height() as f32 / spacing).ceil() as u32;
    let mut marks = Vec::with_capacity((columns * rows * 3) as usize);
    for row in 0..rows {
        token.checkpoint()?;
        for column in 0..columns {
            let x = ((column as f32 + 0.5) * spacing).min(source.width() as f32 - 0.5);
            let y = ((row as f32 + 0.5) * spacing).min(source.height() as f32 - 0.5);
            let pixel = source.get_pixel(x as u32, y as u32).0;
            for (ink, value) in Ink::RGB.into_iter().zip([pixel[0], pixel[1], pixel[2]]) {
                let value = value as f32 / 255.0 * (pixel[3] as f32 / 255.0);
                if value <= 0.006 {
                    continue;
                }
                let extent = spacing * 0.47 * (settings.coverage / 100.0) * value.sqrt();
                marks.push(Mark {
                    channel: ink.into(),
                    x,
                    y,
                    extent,
                    thickness: 0.0,
                    angle: settings.angle,
                    treatment: settings.treatment,
                    geometry: MarkGeometry::Native,
                });
            }
        }
    }
    Ok(MarkSet {
        width: source.width(),
        height: source.height(),
        marks,
        layers: Ink::RGB
            .into_iter()
            .map(|ink| InkLayer {
                channel: ink.into(),
                enabled: true,
                color: Channel::from(ink).color(),
                opacity: 1.0,
            })
            .collect(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WebGrid {
    pub cols: u32,
    pub rows: u32,
    pub cell_width: f64,
    pub cell_height: f64,
}

pub fn calculate_web_grid(width: u32, height: u32, long_edge_cells: f64) -> WebGrid {
    let aspect = width as f64 / height as f64;
    let safe_long_edge = long_edge_cells.round().max(2.0) as u32;
    let (cols, rows) = if aspect >= 1.0 {
        (
            safe_long_edge,
            ((safe_long_edge as f64 / aspect).round() as u32).max(1),
        )
    } else {
        (
            ((safe_long_edge as f64 * aspect).round() as u32).max(1),
            safe_long_edge,
        )
    };
    WebGrid {
        cols,
        rows,
        cell_width: width as f64 / cols as f64,
        cell_height: height as f64 / rows as f64,
    }
}

pub fn generate_document_marks(document: &Document) -> Result<MarkSet> {
    generate_document_marks_cancellable(document, &CancellationToken::new())
}

pub fn generate_document_marks_cancellable(
    document: &Document,
    token: &CancellationToken,
) -> Result<MarkSet> {
    token.checkpoint()?;
    let mut canonical = document.clone();
    let dimensions = source_dimensions(&canonical.source)?;
    canonical.normalize_canvas_aspect(dimensions.0, dimensions.1);
    canonical.validate()?;
    let source = decode_source(&canonical.source, 2400)?;
    token.checkpoint()?;
    Ok(match &canonical.render {
        RenderVariant::NativeBasicV1 => {
            if canonical.output_mode == crate::model::OutputMode::RgbScreen {
                generate_rgb_marks_cancellable(&source, canonical.settings, token)?
            } else {
                generate_marks_cancellable(&source, canonical.settings, token)?
            }
        }
        RenderVariant::WebShapeV1 { settings } => generate_web_shape_marks_for_output_mode(
            &source,
            settings,
            canonical.output_mode,
            token,
        )?,
        RenderVariant::WebCurveV1 { .. } => {
            bail!("full-width curve rendering is not available yet")
        }
    })
}

pub fn generate_web_shape_marks(source: &RgbaImage, settings: &WebShapeSettings) -> MarkSet {
    generate_web_shape_marks_cancellable(source, settings, &CancellationToken::new())
        .expect("fresh cancellation token cannot cancel")
}

pub fn generate_web_shape_marks_cancellable(
    source: &RgbaImage,
    settings: &WebShapeSettings,
    token: &CancellationToken,
) -> Result<MarkSet> {
    generate_web_shape_marks_for_output_mode(source, settings, OutputMode::CmykInks, token)
}

fn generate_web_shape_marks_for_output_mode(
    source: &RgbaImage,
    settings: &WebShapeSettings,
    output_mode: OutputMode,
    token: &CancellationToken,
) -> Result<MarkSet> {
    let output_inks = if settings.value_mode == ValueMode::CrosshatchLuminance {
        &Ink::ALL[..]
    } else if output_mode == OutputMode::RgbScreen || settings.value_mode == ValueMode::Rgb {
        &Ink::RGB[..]
    } else {
        &Ink::ALL[..]
    };
    let enabled: Vec<Ink> = output_inks
        .iter()
        .copied()
        .filter(|ink| settings.channels.get(*ink).enabled)
        .collect();
    let mut marks = Vec::new();
    let mut sample_cache = HashMap::new();

    for &ink in output_inks {
        token.checkpoint()?;
        let channel = settings.channels.get(ink);
        if !channel.enabled {
            continue;
        }
        let long_edge_cells = (settings.long_edge_cells * channel.resolution_scale.max(0.05))
            .round()
            .max(2.0);
        let grid = calculate_web_grid(
            settings.output_width,
            settings.output_height,
            long_edge_cells,
        );
        let samples = cached_web_samples(&mut sample_cache, source, grid.cols, grid.rows, token)?;
        let ranges = web_grid_ranges(settings, channel, grid);
        let shape = if settings.use_shared_mark {
            settings.shared_shape
        } else {
            channel.shape
        };
        let shape = resolve_web_shape(shape, settings, channel);

        for row in ranges.2..=ranges.3 {
            token.checkpoint()?;
            for col in ranges.0..=ranges.1 {
                let placement = web_shape_placement(col, row, settings, channel, grid);
                if !placement.visible {
                    continue;
                }
                let sample =
                    samples[(placement.sample_row * grid.cols + placement.sample_col) as usize];
                let mut raw = map_web_pixel(
                    sample,
                    settings.value_mode,
                    settings.single_channel,
                    &enabled,
                )[ink_index(ink)];
                // RGB Screen is additive light on a transparent surface. Unlike
                // CMYK, brightness-driven shapes retain sampled source coverage
                // at antialiased edges. Direct RGB mapping already does this.
                if output_mode == OutputMode::RgbScreen
                    && matches!(
                        settings.value_mode,
                        ValueMode::Luminance | ValueMode::SingleChannel
                    )
                {
                    raw *= sample[3] as f64 / 255.0;
                }
                let value = map_web_threshold(raw, channel.threshold);
                if value <= 0.0 {
                    continue;
                }
                let cell_size = grid.cell_width.min(grid.cell_height);
                let min = (settings.min_mark / 100.0).max(0.0);
                let max = (settings.max_mark / 100.0).max(min) * (channel.max_size / 100.0);
                let size = cell_size * (min + (max - min) * value) * channel.scale;
                marks.push(Mark {
                    channel: ink.into(),
                    x: placement.x as f32,
                    y: placement.y as f32,
                    extent: (size * settings.grid_scale / 100.0 * channel.width_scale) as f32,
                    thickness: (size * settings.grid_scale / 100.0 * channel.height_scale) as f32,
                    angle: channel.rotation as f32,
                    treatment: Treatment::Dots,
                    geometry: MarkGeometry::WebShape(shape.clone()),
                });
            }
        }
    }

    let layers = output_inks
        .iter()
        .copied()
        .map(|ink| {
            let channel = settings.channels.get(ink);
            let color = if settings.value_mode == ValueMode::CrosshatchLuminance {
                parse_hex_color(&settings.crosshatch_color).unwrap_or((17, 17, 17))
            } else {
                parse_hex_color(&channel.color).unwrap_or_else(|| Channel::from(ink).color())
            };
            InkLayer {
                channel: ink.into(),
                enabled: channel.enabled,
                color,
                opacity: channel.opacity as f32,
            }
        })
        .collect();

    Ok(MarkSet {
        width: settings.output_width,
        height: settings.output_height,
        marks,
        layers,
    })
}

fn cached_web_samples<'a>(
    cache: &'a mut HashMap<(u32, u32), Vec<[u8; 4]>>,
    source: &RgbaImage,
    cols: u32,
    rows: u32,
    token: &CancellationToken,
) -> Result<&'a [[u8; 4]]> {
    token.checkpoint()?;
    if let std::collections::hash_map::Entry::Vacant(entry) = cache.entry((cols, rows)) {
        entry.insert(sample_web_image_cancellable(source, cols, rows, token)?);
    }
    Ok(cache
        .get(&(cols, rows))
        .expect("sample cache entry was inserted")
        .as_slice())
}

#[allow(dead_code)]
pub(crate) fn sample_web_image(source: &RgbaImage, cols: u32, rows: u32) -> Vec<[u8; 4]> {
    sample_web_image_cancellable(source, cols, rows, &CancellationToken::new())
        .expect("fresh cancellation token cannot cancel")
}

pub(crate) fn sample_web_image_cancellable(
    source: &RgbaImage,
    cols: u32,
    rows: u32,
    token: &CancellationToken,
) -> Result<Vec<[u8; 4]>> {
    // Canvas drawImage uses interpolated area reduction. Triangle filtering is
    // deterministic; explicitly filtering premultiplied colors matches Canvas
    // compositing at translucent edges before getImageData unpremultiplies.
    let mut premultiplied = image::Rgba32FImage::new(source.width(), source.height());
    for y in 0..source.height() {
        token.checkpoint()?;
        for x in 0..source.width() {
            let pixel = source.get_pixel(x, y).0;
            let alpha = pixel[3] as f32 / 255.0;
            premultiplied.put_pixel(
                x,
                y,
                image::Rgba([
                    pixel[0] as f32 / 255.0 * alpha,
                    pixel[1] as f32 / 255.0 * alpha,
                    pixel[2] as f32 / 255.0 * alpha,
                    alpha,
                ]),
            );
        }
    }
    token.checkpoint()?;
    let resized = image::imageops::resize(&premultiplied, cols, rows, FilterType::Triangle);
    token.checkpoint()?;
    let mut result = Vec::with_capacity((cols * rows) as usize);
    for y in 0..rows {
        token.checkpoint()?;
        for x in 0..cols {
            let pixel = resized.get_pixel(x, y);
            let alpha = pixel[3].clamp(0.0, 1.0);
            if alpha <= f32::EPSILON {
                result.push([0, 0, 0, 0]);
                continue;
            }
            result.push([
                (pixel[0] / alpha * 255.0).round().clamp(0.0, 255.0) as u8,
                (pixel[1] / alpha * 255.0).round().clamp(0.0, 255.0) as u8,
                (pixel[2] / alpha * 255.0).round().clamp(0.0, 255.0) as u8,
                (alpha * 255.0).round() as u8,
            ]);
        }
    }
    Ok(result)
}

pub fn map_web_pixel(
    pixel: [u8; 4],
    mode: ValueMode,
    single_channel: Ink,
    enabled_channels: &[Ink],
) -> [f64; 4] {
    if pixel[3] == 0 {
        return [0.0; 4];
    }
    if mode == ValueMode::Cmyk {
        let r = pixel[0] as f64 / 255.0;
        let g = pixel[1] as f64 / 255.0;
        let b = pixel[2] as f64 / 255.0;
        let k = 1.0 - r.max(g).max(b);
        if k >= 0.999 {
            return [0.0, 0.0, 0.0, 1.0];
        }
        let denominator = 1.0 - k;
        return [
            ((1.0 - r - k) / denominator).clamp(0.0, 1.0),
            ((1.0 - g - k) / denominator).clamp(0.0, 1.0),
            ((1.0 - b - k) / denominator).clamp(0.0, 1.0),
            k.clamp(0.0, 1.0),
        ];
    }
    if mode == ValueMode::Rgb {
        let alpha = pixel[3] as f64 / 255.0;
        return [
            pixel[0] as f64 / 255.0 * alpha,
            pixel[1] as f64 / 255.0 * alpha,
            pixel[2] as f64 / 255.0 * alpha,
            0.0,
        ];
    }
    let darkness = 1.0
        - (0.2126 * pixel[0] as f64 + 0.7152 * pixel[1] as f64 + 0.0722 * pixel[2] as f64) / 255.0;
    match mode {
        ValueMode::Cmyk | ValueMode::Rgb => unreachable!(),
        ValueMode::Luminance => [darkness; 4],
        ValueMode::SingleChannel => {
            let mut values = [0.0; 4];
            values[ink_index(single_channel)] = darkness;
            values
        }
        ValueMode::CrosshatchLuminance => {
            let order = [Ink::Black, Ink::Cyan, Ink::Magenta, Ink::Yellow];
            let active: Vec<Ink> = order
                .into_iter()
                .filter(|ink| enabled_channels.contains(ink))
                .collect();
            if active.is_empty() {
                return [0.0; 4];
            }
            let span = 1.0 / active.len() as f64;
            let darkness = snap_unit_interval(darkness);
            let mut values = [0.0; 4];
            for (index, ink) in active.into_iter().enumerate() {
                values[ink_index(ink)] =
                    snap_unit_interval((darkness - index as f64 * span).clamp(0.0, span));
            }
            values
        }
    }
}

fn snap_unit_interval(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    if value <= 1e-12 {
        0.0
    } else if value >= 1.0 - 1e-12 {
        1.0
    } else {
        value
    }
}

fn ink_index(ink: Ink) -> usize {
    match ink {
        Ink::Cyan => 0,
        Ink::Magenta => 1,
        Ink::Yellow => 2,
        Ink::Black => 3,
        Ink::Red => 0,
        Ink::Green => 1,
        Ink::Blue => 2,
    }
}

pub fn map_web_threshold(value: f64, threshold: f64) -> f64 {
    if value < threshold {
        0.0
    } else if threshold >= 0.999 {
        if value >= threshold { 1.0 } else { 0.0 }
    } else {
        (value - threshold) / (1.0 - threshold)
    }
}

#[derive(Clone, Copy)]
struct WebPlacement {
    x: f64,
    y: f64,
    visible: bool,
    sample_col: u32,
    sample_row: u32,
}

fn web_shape_placement(
    col: i32,
    row: i32,
    settings: &WebShapeSettings,
    channel: &WebShapeChannel,
    grid: WebGrid,
) -> WebPlacement {
    let phase_x = wrap_signed_grid_offset(channel.offset_x, grid.cell_width);
    let phase_y = wrap_signed_grid_offset(channel.offset_y, grid.cell_height);
    let logical_x = if channel.grid_rotation.abs() <= 0.0001 {
        positive_modulo(
            (col as f64 + 0.5) * grid.cell_width + phase_x,
            settings.output_width as f64,
        )
    } else {
        (col as f64 + 0.5) * grid.cell_width + phase_x
    };
    let logical_y = if channel.grid_rotation.abs() <= 0.0001 {
        positive_modulo(
            (row as f64 + 0.5) * grid.cell_height + phase_y,
            settings.output_height as f64,
        )
    } else {
        (row as f64 + 0.5) * grid.cell_height + phase_y
    };
    let (x, y) = rotate_web_point(
        logical_x,
        logical_y,
        settings,
        channel,
        channel.grid_rotation,
    );
    let margin = max_web_shape_extent(settings, channel, grid);
    WebPlacement {
        x,
        y,
        visible: x >= -margin
            && x <= settings.output_width as f64 + margin
            && y >= -margin
            && y <= settings.output_height as f64 + margin,
        sample_col: ((x / grid.cell_width).floor() as i64).clamp(0, grid.cols as i64 - 1) as u32,
        sample_row: ((y / grid.cell_height).floor() as i64).clamp(0, grid.rows as i64 - 1) as u32,
    }
}

fn web_grid_ranges(
    settings: &WebShapeSettings,
    channel: &WebShapeChannel,
    grid: WebGrid,
) -> (i32, i32, i32, i32) {
    if channel.grid_rotation.abs() <= 0.0001 {
        return (0, grid.cols as i32 - 1, 0, grid.rows as i32 - 1);
    }
    let margin = max_web_shape_extent(settings, channel, grid);
    let corners = [
        (-margin, -margin),
        (settings.output_width as f64 + margin, -margin),
        (
            settings.output_width as f64 + margin,
            settings.output_height as f64 + margin,
        ),
        (-margin, settings.output_height as f64 + margin),
    ];
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x, y) in corners {
        let (x, y) = rotate_web_point(x, y, settings, channel, -channel.grid_rotation);
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let phase_x = wrap_signed_grid_offset(channel.offset_x, grid.cell_width);
    let phase_y = wrap_signed_grid_offset(channel.offset_y, grid.cell_height);
    (
        ((min_x - phase_x) / grid.cell_width - 0.5).floor() as i32,
        ((max_x - phase_x) / grid.cell_width - 0.5).ceil() as i32,
        ((min_y - phase_y) / grid.cell_height - 0.5).floor() as i32,
        ((max_y - phase_y) / grid.cell_height - 0.5).ceil() as i32,
    )
}

fn max_web_shape_extent(
    settings: &WebShapeSettings,
    channel: &WebShapeChannel,
    grid: WebGrid,
) -> f64 {
    let min = (settings.min_mark / 100.0).max(0.0);
    let max = (settings.max_mark / 100.0).max(min) * channel.max_size / 100.0;
    let shape = if settings.use_shared_mark {
        settings.shared_shape
    } else {
        channel.shape
    };
    let resolved = resolve_web_shape(shape, settings, channel);
    let radius = match resolved {
        ResolvedWebShape::Circle => 0.5 * channel.width_scale.max(channel.height_scale),
        ResolvedWebShape::Polygon(points) => points
            .iter()
            .map(|(x, y)| {
                let x = *x as f64 * channel.width_scale;
                let y = *y as f64 * channel.height_scale;
                x.hypot(y)
            })
            .fold(0.0, f64::max),
        ResolvedWebShape::Cubic { start, segments } => std::iter::once(start)
            .chain(segments.iter().flat_map(|segment| {
                [
                    (segment.0, segment.1),
                    (segment.2, segment.3),
                    (segment.4, segment.5),
                ]
            }))
            .map(|(x, y)| {
                let x = x as f64 * channel.width_scale;
                let y = y as f64 * channel.height_scale;
                x.hypot(y)
            })
            .fold(0.0, f64::max),
    };
    grid.cell_width.min(grid.cell_height) * max * channel.scale * settings.grid_scale / 100.0
        * radius
}

fn rotate_web_point(
    x: f64,
    y: f64,
    settings: &WebShapeSettings,
    channel: &WebShapeChannel,
    degrees: f64,
) -> (f64, f64) {
    if degrees.abs() <= 0.0001 {
        return (x, y);
    }
    let pivot_x = settings.output_width as f64 / 2.0 + channel.grid_pivot_x;
    let pivot_y = settings.output_height as f64 / 2.0 + channel.grid_pivot_y;
    let radians = degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    let dx = x - pivot_x;
    let dy = y - pivot_y;
    (pivot_x + dx * cos - dy * sin, pivot_y + dx * sin + dy * cos)
}

fn wrap_signed_grid_offset(offset: f64, spacing: f64) -> f64 {
    if !offset.is_finite() || !spacing.is_finite() || spacing <= 0.0 {
        0.0
    } else {
        positive_modulo(offset + spacing / 2.0, spacing) - spacing / 2.0
    }
}

fn positive_modulo(value: f64, modulus: f64) -> f64 {
    ((value % modulus) + modulus) % modulus
}

pub struct RenderResult {
    pub generation: u64,
    pub image: RgbaImage,
}

pub fn render_preview(
    source: &SourceArtwork,
    settings: Settings,
    max_dimension: u32,
    generation: u64,
) -> Result<RenderResult> {
    let source_image = decode_source(source, max_dimension)?;
    let marks = generate_marks(&source_image, settings);
    render_mark_set(&marks, max_dimension, generation)
}

pub fn render_document_preview(
    document: &Document,
    max_dimension: u32,
    generation: u64,
) -> Result<RenderResult> {
    render_document_preview_cancellable(
        document,
        max_dimension,
        generation,
        &CancellationToken::new(),
    )
}

pub fn render_document_preview_cancellable(
    document: &Document,
    max_dimension: u32,
    generation: u64,
    token: &CancellationToken,
) -> Result<RenderResult> {
    token.checkpoint()?;
    let mut canonical = document.clone();
    let dimensions = source_dimensions(&canonical.source)?;
    canonical.normalize_canvas_aspect(dimensions.0, dimensions.1);
    // The common white canvas path retains the long-established native
    // rasterization behavior (notably multiply blending at antialiased
    // edges). Non-white/translucent surfaces are composed after transparent
    // artwork so they remain presentation-only.
    let legacy_white_preview = matches!(canonical.appearance.preview_surface, PreviewSurface::Color { color } if color == RgbaColor::WHITE)
        && matches!(
            canonical.appearance.export_background,
            ExportBackground::None
                | ExportBackground::Color {
                    color: RgbaColor::WHITE
                }
        );
    if let RenderVariant::WebCurveV1 { settings } = &canonical.render {
        canonical.validate()?;
        let source = decode_source(&canonical.source, 2400)?;
        token.checkpoint()?;
        let geometry = crate::curve_render::generate_curve_geometry_for_output_mode(
            &source,
            settings,
            canonical.output_mode,
            token,
        )?;
        token.checkpoint()?;
        let rendered = if legacy_white_preview {
            let scale = max_dimension as f32 / geometry.width.max(geometry.height) as f32;
            let width = (geometry.width as f32 * scale).round().max(1.0) as u32;
            let height = (geometry.height as f32 * scale).round().max(1.0) as u32;
            RenderResult {
                generation,
                image: crate::curve_render::render_curve_geometry_output_cancellable(
                    &geometry, width, height, true, None, token,
                )?,
            }
        } else {
            let scale = max_dimension as f32 / geometry.width.max(geometry.height) as f32;
            let width = (geometry.width as f32 * scale).round().max(1.0) as u32;
            let height = (geometry.height as f32 * scale).round().max(1.0) as u32;
            RenderResult {
                generation,
                image: crate::curve_render::render_curve_geometry_output_cancellable(
                    &geometry, width, height, false, None, token,
                )?,
            }
        };
        if legacy_white_preview {
            return Ok(rendered);
        }
        return Ok(RenderResult {
            generation: rendered.generation,
            image: composite_preview(rendered.image, canonical.appearance),
        });
    }
    let marks = generate_document_marks_cancellable(&canonical, token)?;
    token.checkpoint()?;
    if legacy_white_preview {
        return render_mark_set_cancellable(&marks, max_dimension, generation, token);
    }
    let rendered =
        render_mark_set_transparent_cancellable(&marks, max_dimension, generation, token)?;
    Ok(RenderResult {
        generation: rendered.generation,
        image: composite_preview(rendered.image, canonical.appearance),
    })
}

fn render_mark_set(marks: &MarkSet, max_dimension: u32, generation: u64) -> Result<RenderResult> {
    render_mark_set_cancellable(marks, max_dimension, generation, &CancellationToken::new())
}

fn render_mark_set_cancellable(
    marks: &MarkSet,
    max_dimension: u32,
    generation: u64,
    token: &CancellationToken,
) -> Result<RenderResult> {
    let scale = max_dimension as f32 / marks.width.max(marks.height) as f32;
    let width = (marks.width as f32 * scale).round().max(1.0) as u32;
    let height = (marks.height as f32 * scale).round().max(1.0) as u32;
    let image = render_mark_set_output_cancellable(marks, width, height, true, None, token)?;
    Ok(RenderResult { generation, image })
}

fn render_mark_set_transparent_cancellable(
    marks: &MarkSet,
    max_dimension: u32,
    generation: u64,
    token: &CancellationToken,
) -> Result<RenderResult> {
    let scale = max_dimension as f32 / marks.width.max(marks.height) as f32;
    let width = (marks.width as f32 * scale).round().max(1.0) as u32;
    let height = (marks.height as f32 * scale).round().max(1.0) as u32;
    let image = render_mark_set_output_cancellable(marks, width, height, false, None, token)?;
    Ok(RenderResult { generation, image })
}

pub fn render_document_output(
    document: &Document,
    width: u32,
    height: u32,
    white_background: bool,
    channel: Option<Ink>,
) -> Result<RgbaImage> {
    render_document_output_cancellable(
        document,
        width,
        height,
        white_background,
        channel,
        &CancellationToken::new(),
    )
}

pub fn render_document_output_cancellable(
    document: &Document,
    width: u32,
    height: u32,
    white_background: bool,
    channel: Option<Ink>,
    token: &CancellationToken,
) -> Result<RgbaImage> {
    token.checkpoint()?;
    let mut canonical = document.clone();
    let dimensions = source_dimensions(&canonical.source)?;
    canonical.normalize_canvas_aspect(dimensions.0, dimensions.1);
    canonical.validate()?;
    anyhow::ensure!(
        width > 0 && height > 0,
        "output dimensions must be positive"
    );
    let pixels = u64::from(width) * u64::from(height);
    anyhow::ensure!(
        pixels <= 64_000_000,
        "PNG output exceeds the safe 64 megapixel limit"
    );
    if let RenderVariant::WebCurveV1 { settings } = &canonical.render {
        let source = decode_source(&canonical.source, 2400)?;
        token.checkpoint()?;
        let geometry = crate::curve_render::generate_curve_geometry_for_output_mode(
            &source,
            settings,
            canonical.output_mode,
            token,
        )?;
        return crate::curve_render::render_curve_geometry_output_cancellable(
            &geometry,
            width,
            height,
            white_background,
            channel,
            token,
        );
    }
    let marks = generate_document_marks_cancellable(&canonical, token)?;
    render_mark_set_output_cancellable(&marks, width, height, white_background, channel, token)
}

/// Renders artwork using the document's export-background setting. This is
/// intentionally separate from preview composition: no checkerboard or
/// preview-only surface can enter exported pixels.
pub fn render_document_export_cancellable(
    document: &Document,
    width: u32,
    height: u32,
    channel: Option<Ink>,
    token: &CancellationToken,
) -> Result<RgbaImage> {
    let artwork =
        render_document_output_cancellable(document, width, height, false, channel, token)?;
    Ok(composite_export_background(
        artwork,
        document.appearance.export_background,
    ))
}

pub fn composite_preview(mut artwork: RgbaImage, appearance: DocumentAppearance) -> RgbaImage {
    for (x, y, pixel) in artwork.enumerate_pixels_mut() {
        let mut backdrop = checkerboard_pixel(x, y);
        if let PreviewSurface::Color { color } = appearance.preview_surface {
            backdrop = over(color, backdrop);
        }
        if let ExportBackground::Color { color } = appearance.export_background {
            backdrop = over(color, backdrop);
        }
        *pixel = image::Rgba(
            over(
                RgbaColor {
                    red: pixel[0],
                    green: pixel[1],
                    blue: pixel[2],
                    alpha: pixel[3],
                },
                backdrop,
            )
            .into(),
        );
    }
    artwork
}

pub fn composite_export_background(
    mut artwork: RgbaImage,
    background: ExportBackground,
) -> RgbaImage {
    let ExportBackground::Color { color } = background else {
        return artwork;
    };
    for pixel in artwork.pixels_mut() {
        *pixel = image::Rgba(
            over(
                RgbaColor {
                    red: pixel[0],
                    green: pixel[1],
                    blue: pixel[2],
                    alpha: pixel[3],
                },
                color,
            )
            .into(),
        );
    }
    artwork
}

fn checkerboard_pixel(x: u32, y: u32) -> RgbaColor {
    let light = ((x / 12) + (y / 12)).is_multiple_of(2);
    if light {
        RgbaColor::opaque(232, 232, 232)
    } else {
        RgbaColor::opaque(196, 196, 196)
    }
}

fn over(foreground: RgbaColor, background: RgbaColor) -> RgbaColor {
    let fa = foreground.alpha as u32;
    let ba = background.alpha as u32;
    let out_a = fa + (ba * (255 - fa) + 127) / 255;
    if out_a == 0 {
        return RgbaColor {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 0,
        };
    }
    let blend = |fg: u8, bg: u8| {
        let numerator = u32::from(fg) * fa * 255 + u32::from(bg) * ba * (255 - fa);
        ((numerator + out_a * 127) / (out_a * 255)) as u8
    };
    RgbaColor {
        red: blend(foreground.red, background.red),
        green: blend(foreground.green, background.green),
        blue: blend(foreground.blue, background.blue),
        alpha: out_a as u8,
    }
}

impl From<RgbaColor> for [u8; 4] {
    fn from(value: RgbaColor) -> Self {
        [value.red, value.green, value.blue, value.alpha]
    }
}

pub fn render_mark_set_output(
    marks: &MarkSet,
    width: u32,
    height: u32,
    white_background: bool,
    channel: Option<Ink>,
) -> Result<RgbaImage> {
    render_mark_set_output_cancellable(
        marks,
        width,
        height,
        white_background,
        channel,
        &CancellationToken::new(),
    )
}

pub fn render_mark_set_output_cancellable(
    marks: &MarkSet,
    width: u32,
    height: u32,
    white_background: bool,
    channel: Option<Ink>,
    token: &CancellationToken,
) -> Result<RgbaImage> {
    let mut pixmap = Pixmap::new(width, height).context("output is too large")?;
    if white_background {
        pixmap.fill(Color::WHITE);
    }
    let scale_x = width as f32 / marks.width as f32;
    let scale_y = height as f32 / marks.height as f32;

    for layer in marks.layers.iter().filter(|layer| layer.enabled) {
        token.checkpoint()?;
        if channel.is_some_and(|ink| layer.channel != Channel::from(ink)) {
            continue;
        }
        let paint = layer_paint(layer);
        for (index, mark) in marks
            .marks
            .iter()
            .filter(|mark| mark.channel == layer.channel)
            .enumerate()
        {
            if index % 256 == 0 {
                token.checkpoint()?;
            }
            if let Some(path) = mark_path(mark) {
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::from_scale(scale_x, scale_y),
                    None,
                );
            }
        }
    }

    ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, pixmap.take())
        .context("renderer returned an invalid output buffer")
}

fn layer_paint(layer: &InkLayer) -> Paint<'static> {
    let (r, g, b) = layer.color;
    let mut paint = Paint::default();
    paint.set_color_rgba8(
        r,
        g,
        b,
        (layer.opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
    );
    paint.blend_mode = match layer.channel {
        Channel::Red | Channel::Green | Channel::Blue => BlendMode::Screen,
        _ => BlendMode::Multiply,
    };
    paint.anti_alias = true;
    paint
}

fn mark_path(mark: &Mark) -> Option<tiny_skia::Path> {
    if let MarkGeometry::WebShape(shape) = &mark.geometry {
        return web_shape_path(
            mark.x,
            mark.y,
            mark.extent,
            mark.thickness,
            mark.angle,
            shape,
        );
    }
    match mark.treatment {
        Treatment::Dots => {
            let mut path = PathBuilder::new();
            path.push_circle(mark.x, mark.y, mark.extent);
            path.finish()
        }
        Treatment::Squares => {
            rotated_rect_path(mark.x, mark.y, mark.extent, mark.extent, mark.angle)
        }
        Treatment::Lines => {
            rotated_rect_path(mark.x, mark.y, mark.extent, mark.thickness, mark.angle)
        }
    }
}

fn web_shape_path(
    cx: f32,
    cy: f32,
    scale: f32,
    scale_y: f32,
    degrees: f32,
    shape: &ResolvedWebShape,
) -> Option<tiny_skia::Path> {
    let mut path = PathBuilder::new();
    let transform = |x: f32, y: f32| {
        let radians = degrees.to_radians();
        let (sin, cos) = radians.sin_cos();
        let x = x * scale;
        let y = y * scale_y;
        (cx + x * cos - y * sin, cy + x * sin + y * cos)
    };
    if matches!(shape, ResolvedWebShape::Circle) {
        let points = [
            (0.0, -0.5),
            (0.276, -0.5),
            (0.5, -0.276),
            (0.5, 0.0),
            (0.5, 0.276),
            (0.276, 0.5),
            (0.0, 0.5),
            (-0.276, 0.5),
            (-0.5, 0.276),
            (-0.5, 0.0),
            (-0.5, -0.276),
            (-0.276, -0.5),
            (0.0, -0.5),
        ];
        let first = transform(points[0].0, points[0].1);
        path.move_to(first.0, first.1);
        for curve in points[1..].chunks_exact(3) {
            let c1 = transform(curve[0].0, curve[0].1);
            let c2 = transform(curve[1].0, curve[1].1);
            let end = transform(curve[2].0, curve[2].1);
            path.cubic_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
        }
    } else if let ResolvedWebShape::Polygon(points) = shape {
        let first = transform(points[0].0, points[0].1);
        path.move_to(first.0, first.1);
        for &(x, y) in &points[1..] {
            let point = transform(x, y);
            path.line_to(point.0, point.1);
        }
    } else if let ResolvedWebShape::Cubic { start, segments } = shape {
        let first = transform(start.0, start.1);
        path.move_to(first.0, first.1);
        for segment in segments.iter() {
            let c1 = transform(segment.0, segment.1);
            let c2 = transform(segment.2, segment.3);
            let end = transform(segment.4, segment.5);
            path.cubic_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
        }
    }
    path.close();
    path.finish()
}

fn resolve_web_shape(
    shape: WebShape,
    settings: &WebShapeSettings,
    channel: &WebShapeChannel,
) -> ResolvedWebShape {
    if shape == WebShape::Circle {
        return ResolvedWebShape::Circle;
    }
    let points: Vec<(f32, f32)> = match shape {
        WebShape::Circle => unreachable!(),
        WebShape::RegularPolygon => regular_polygon_points(if settings.use_shared_mark {
            settings.polygon_sides
        } else {
            channel.polygon_sides
        }),
        WebShape::UserDefined => {
            let path = settings.resolved_channel_shape_path(channel);
            let start = path.anchors[0].point;
            let segments = path
                .anchors
                .iter()
                .enumerate()
                .map(|(index, anchor)| {
                    let next = path.anchors[(index + 1) % path.anchors.len()];
                    (
                        anchor.outgoing.x as f32,
                        anchor.outgoing.y as f32,
                        next.incoming.x as f32,
                        next.incoming.y as f32,
                        next.point.x as f32,
                        next.point.y as f32,
                    )
                })
                .collect::<Vec<_>>();
            return ResolvedWebShape::Cubic {
                start: (start.x as f32, start.y as f32),
                segments: segments.into(),
            };
        }
        WebShape::Rectangle => vec![(-0.45, -0.45), (0.45, -0.45), (0.45, 0.45), (-0.45, 0.45)],
        WebShape::Triangle => vec![(0.0, -0.52), (0.5, 0.4), (-0.5, 0.4)],
        WebShape::Pentagon => regular_polygon_points(5),
        WebShape::Hexagon => regular_polygon_points(6),
    };
    ResolvedWebShape::Polygon(points.into())
}

fn regular_polygon_points(sides: u8) -> Vec<(f32, f32)> {
    let sides = sides.clamp(3, 6);
    let start = -std::f32::consts::FRAC_PI_2
        + if sides.is_multiple_of(2) {
            std::f32::consts::PI / sides as f32
        } else {
            0.0
        };
    (0..sides)
        .map(|index| {
            let angle = start + std::f32::consts::TAU * index as f32 / sides as f32;
            (angle.cos() * 0.5, angle.sin() * 0.5)
        })
        .collect()
}

fn rotated_rect_path(
    cx: f32,
    cy: f32,
    width: f32,
    height: f32,
    degrees: f32,
) -> Option<tiny_skia::Path> {
    let radians = degrees.to_radians();
    let cos = radians.cos();
    let sin = radians.sin();
    let points = [
        (-width / 2.0, -height / 2.0),
        (width / 2.0, -height / 2.0),
        (width / 2.0, height / 2.0),
        (-width / 2.0, height / 2.0),
    ];
    let transform = |(x, y): (f32, f32)| (cx + x * cos - y * sin, cy + x * sin + y * cos);
    let mut path = PathBuilder::new();
    let first = transform(points[0]);
    path.move_to(first.0, first.1);
    for point in points.into_iter().skip(1) {
        let point = transform(point);
        path.line_to(point.0, point.1);
    }
    path.close();
    path.finish()
}

/// Monotonic generation gate. The UI only installs a result if `accepts` is true.
#[derive(Default)]
pub struct RenderGate(AtomicU64);

impl RenderGate {
    pub fn next(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn current(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }

    pub fn accepts(&self, generation: u64) -> bool {
        generation == self.current()
    }
}

pub fn source_preview(source: &SourceArtwork, max_dimension: u32) -> Result<DynamicImage> {
    Ok(DynamicImage::ImageRgba8(decode_source(
        source,
        max_dimension,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_png() -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([80, 90, 100, 255])))
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    fn web_settings() -> WebShapeSettings {
        let mut settings = WebShapeSettings {
            output_width: 100,
            output_height: 100,
            long_edge_cells: 2.0,
            grid_scale: 100.0,
            min_mark: 100.0,
            max_mark: 100.0,
            value_mode: ValueMode::Cmyk,
            ..Default::default()
        };
        for ink in Ink::ALL {
            settings.channels.get_mut(ink).enabled = ink == Ink::Black;
            settings.channels.get_mut(ink).grid_rotation = 0.0;
        }
        settings
    }

    fn dark_pixels(image: &RgbaImage, bounds: (u32, u32, u32, u32)) -> usize {
        let (left, top, right, bottom) = bounds;
        (top..bottom)
            .flat_map(|y| (left..right).map(move |x| image.get_pixel(x, y).0))
            .filter(|pixel| pixel[3] > 128 && pixel[0] < 80 && pixel[1] < 80 && pixel[2] < 80)
            .count()
    }

    #[test]
    fn svg_text_is_rasterized_into_the_expected_region() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="100">
            <rect width="120" height="100" fill="white"/>
            <text x="28" y="85" font-family="sans-serif" font-size="80" font-weight="700" fill="black">T</text>
        </svg>"##;
        let image = decode_svg(svg, 120).unwrap();
        assert!(
            dark_pixels(&image, (20, 15, 95, 95)) > 250,
            "the text glyph should produce visible dark pixels"
        );
        assert_eq!(dark_pixels(&image, (0, 0, 15, 15)), 0);
    }

    #[test]
    fn missing_named_svg_font_falls_back_to_an_installed_generic() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="100">
            <rect width="120" height="100" fill="white"/>
            <text x="28" y="85" font-family="Definitely Missing Toniator Font" font-size="80" fill="black">T</text>
        </svg>"##;
        let image = decode_svg(svg, 120).unwrap();
        assert!(dark_pixels(&image, (20, 15, 95, 95)) > 150);
    }

    #[test]
    fn cmyk_matches_reference_mapping() {
        assert_eq!(
            rgb_to_cmyk(0, 0, 0),
            Cmyk {
                c: 0.0,
                m: 0.0,
                y: 0.0,
                k: 1.0
            }
        );
        let red = rgb_to_cmyk(255, 0, 0);
        assert!((red.m - 1.0).abs() < 1e-6);
        assert!((red.y - 1.0).abs() < 1e-6);
        assert_eq!(red.c, 0.0);
        assert_eq!(red.k, 0.0);
        let gray = rgb_to_cmyk(128, 128, 128);
        assert!((gray.k - (127.0 / 255.0)).abs() < 1e-5);
    }

    #[test]
    fn rgb_screen_mapping_is_direct_and_alpha_weighted() {
        assert_eq!(
            map_web_pixel([255, 0, 0, 255], ValueMode::Rgb, Ink::Red, &Ink::RGB),
            [1.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            map_web_pixel([255, 255, 255, 255], ValueMode::Rgb, Ink::Red, &Ink::RGB),
            [1.0, 1.0, 1.0, 0.0]
        );
        assert_eq!(
            map_web_pixel([255, 0, 0, 128], ValueMode::Rgb, Ink::Red, &Ink::RGB),
            [128.0 / 255.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            map_web_pixel([0, 0, 0, 255], ValueMode::Rgb, Ink::Red, &Ink::RGB),
            [0.0; 4]
        );
    }

    #[test]
    fn rgb_output_uses_rgb_layers_for_neutral_shape_mapping() {
        let source = image::RgbaImage::from_pixel(4, 4, image::Rgba([180, 120, 60, 255]));
        let settings = WebShapeSettings {
            output_width: 4,
            output_height: 4,
            long_edge_cells: 2.0,
            value_mode: ValueMode::Luminance,
            ..Default::default()
        };
        let marks = generate_web_shape_marks_for_output_mode(
            &source,
            &settings,
            OutputMode::RgbScreen,
            &CancellationToken::new(),
        )
        .unwrap();
        assert!(marks.layers.iter().all(|layer| {
            matches!(layer.channel, Channel::Red | Channel::Green | Channel::Blue)
        }));
        assert!(
            marks
                .layers
                .iter()
                .any(|layer| layer.channel == Channel::Red)
        );
        assert!(
            marks
                .layers
                .iter()
                .any(|layer| layer.channel == Channel::Green)
        );
        assert!(
            marks
                .layers
                .iter()
                .any(|layer| layer.channel == Channel::Blue)
        );
    }

    #[test]
    fn rgb_brightness_shapes_preserve_alpha_coverage_and_channel_output() {
        let source = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 128]));
        let mut settings = WebShapeSettings {
            output_width: 40,
            output_height: 40,
            long_edge_cells: 2.0,
            min_mark: 0.0,
            max_mark: 100.0,
            value_mode: ValueMode::Luminance,
            ..Default::default()
        };
        for ink in Ink::ALL {
            settings.channels.get_mut(ink).enabled = Ink::RGB.contains(&ink);
        }
        let half_alpha = generate_web_shape_marks_for_output_mode(
            &source,
            &settings,
            OutputMode::RgbScreen,
            &CancellationToken::new(),
        )
        .unwrap();
        let half_render = render_mark_set_output(&half_alpha, 40, 40, false, None).unwrap();
        assert!(half_render.pixels().any(|pixel| pixel[3] > 0));
        assert!(
            half_render
                .pixels()
                .all(|pixel| pixel[0] == pixel[1] && pixel[1] == pixel[2])
        );

        let opaque = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        let opaque_marks = generate_web_shape_marks_for_output_mode(
            &opaque,
            &settings,
            OutputMode::RgbScreen,
            &CancellationToken::new(),
        )
        .unwrap();
        let opaque_alpha = render_mark_set_output(&opaque_marks, 40, 40, false, None)
            .unwrap()
            .pixels()
            .map(|pixel| pixel[3] as u32)
            .sum::<u32>();
        let half_alpha_coverage = half_render
            .pixels()
            .map(|pixel| pixel[3] as u32)
            .sum::<u32>();
        assert!(
            half_alpha_coverage < opaque_alpha,
            "half-alpha brightness must render less coverage than opaque brightness"
        );

        let mut one_channel = settings.clone();
        one_channel.value_mode = ValueMode::SingleChannel;
        one_channel.single_channel = Ink::Red;
        let one_channel_coverage = |source: &RgbaImage| {
            let marks = generate_web_shape_marks_for_output_mode(
                source,
                &one_channel,
                OutputMode::RgbScreen,
                &CancellationToken::new(),
            )
            .unwrap();
            render_mark_set_output(&marks, 40, 40, false, None)
                .unwrap()
                .pixels()
                .map(|pixel| pixel[3] as u32)
                .sum::<u32>()
        };
        assert!(
            one_channel_coverage(&source) < one_channel_coverage(&opaque),
            "half-alpha one-channel brightness must render less coverage than opaque brightness"
        );

        let transparent = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 0]));
        let transparent_marks = generate_web_shape_marks_for_output_mode(
            &transparent,
            &settings,
            OutputMode::RgbScreen,
            &CancellationToken::new(),
        )
        .unwrap();
        assert!(transparent_marks.marks.is_empty());
        assert!(
            render_mark_set_output(&transparent_marks, 40, 40, false, None)
                .unwrap()
                .pixels()
                .all(|pixel| pixel[3] == 0)
        );

        let red_only = render_mark_set_output(&half_alpha, 40, 40, false, Some(Ink::Red)).unwrap();
        assert!(
            red_only
                .pixels()
                .all(|pixel| pixel[1] == 0 && pixel[2] == 0)
        );
    }

    #[test]
    fn rgb_shapes_keep_channel_independence_through_raster_png_and_svg() {
        let source_image = RgbaImage::from_pixel(4, 4, Rgba([255, 128, 64, 255]));
        let mut settings = WebShapeSettings {
            output_width: 64,
            output_height: 64,
            long_edge_cells: 4.0,
            grid_scale: 100.0,
            min_mark: 0.0,
            max_mark: 100.0,
            value_mode: ValueMode::Rgb,
            use_shared_mark: false,
            ..Default::default()
        };
        for ink in Ink::ALL {
            settings.channels.get_mut(ink).enabled = Ink::RGB.contains(&ink);
            settings.channels.get_mut(ink).grid_rotation = 0.0;
        }
        settings.channels.r.shape = WebShape::Circle;
        settings.channels.g.shape = WebShape::RegularPolygon;
        settings.channels.g.polygon_sides = 3;
        settings.channels.b.shape = WebShape::RegularPolygon;
        settings.channels.b.polygon_sides = 6;
        settings.channels.r.opacity = 1.0;
        settings.channels.g.opacity = 0.6;
        settings.channels.b.opacity = 0.3;

        let marks = generate_web_shape_marks_for_output_mode(
            &source_image,
            &settings,
            OutputMode::RgbScreen,
            &CancellationToken::new(),
        )
        .unwrap();
        let max_extent = |channel| {
            marks
                .marks
                .iter()
                .filter(|mark| mark.channel == Channel::from(channel))
                .map(|mark| mark.extent)
                .fold(0.0, f32::max)
        };
        assert!(max_extent(Ink::Red) > max_extent(Ink::Green));
        assert!(max_extent(Ink::Green) > max_extent(Ink::Blue));
        assert!(matches!(
            marks
                .marks
                .iter()
                .find(|mark| mark.channel == Channel::Green)
                .unwrap()
                .geometry,
            MarkGeometry::WebShape(ResolvedWebShape::Polygon(ref points)) if points.len() == 3
        ));
        assert!(matches!(
            marks
                .marks
                .iter()
                .find(|mark| mark.channel == Channel::Blue)
                .unwrap()
                .geometry,
            MarkGeometry::WebShape(ResolvedWebShape::Polygon(ref points)) if points.len() == 6
        ));

        let red_only = render_mark_set_output(&marks, 64, 64, false, Some(Ink::Red)).unwrap();
        assert!(
            red_only
                .pixels()
                .all(|pixel| pixel[1] == 0 && pixel[2] == 0)
        );
        let blue_only = render_mark_set_output(&marks, 64, 64, false, Some(Ink::Blue)).unwrap();
        assert!(
            blue_only
                .pixels()
                .all(|pixel| pixel[0] == 0 && pixel[1] == 0)
        );
        let combined = render_mark_set_output(&marks, 64, 64, false, None).unwrap();
        assert!(combined.pixels().any(|pixel| pixel[0] > 0 && pixel[1] > 0));
        assert!(combined.pixels().any(|pixel| pixel[2] > 0));
        let green_only = render_mark_set_output(&marks, 64, 64, false, Some(Ink::Green)).unwrap();
        let red_alpha = red_only.pixels().map(|pixel| pixel[3]).max().unwrap_or(0);
        let green_alpha = green_only.pixels().map(|pixel| pixel[3]).max().unwrap_or(0);
        assert!(
            green_alpha < red_alpha,
            "per-channel opacity must affect output alpha"
        );

        let mut shared = settings.clone();
        shared.use_shared_mark = true;
        shared.shared_shape = WebShape::RegularPolygon;
        shared.polygon_sides = 5;
        let shared_marks = generate_web_shape_marks_for_output_mode(
            &source_image,
            &shared,
            OutputMode::RgbScreen,
            &CancellationToken::new(),
        )
        .unwrap();
        assert!(shared_marks.marks.iter().all(|mark| matches!(
            mark.geometry,
            MarkGeometry::WebShape(ResolvedWebShape::Polygon(ref points)) if points.len() == 5
        )));

        let mut png_bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(source_image)
            .write_to(&mut png_bytes, image::ImageFormat::Png)
            .unwrap();
        let mut document = Document::new(SourceArtwork {
            name: "rgb-shapes.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from(png_bytes.into_inner()),
        });
        document.output_mode = OutputMode::RgbScreen;
        document.render = RenderVariant::WebShapeV1 {
            settings: Box::new(settings),
        };
        document.appearance.preview_surface = PreviewSurface::Color {
            color: RgbaColor::opaque(12, 18, 28),
        };
        document.appearance.export_background = ExportBackground::Color {
            color: RgbaColor::opaque(4, 8, 16),
        };
        let preview = render_document_preview(&document, 64, 1).unwrap().image;
        let png = crate::png_export::png_bytes(
            &document,
            crate::png_export::PngExportOptions {
                width: 64,
                height: 64,
                background: crate::png_export::PngBackground::Document,
                channel: None,
            },
        )
        .unwrap();
        assert_eq!(
            image::load_from_memory(&png).unwrap().to_rgba8(),
            preview,
            "RGB Shapes preview and document-background PNG must share the rendered result"
        );

        let directory = tempfile::tempdir().unwrap();
        let svg_path = directory.path().join("rgb-shapes.svg");
        crate::export_svg(&svg_path, &document).unwrap();
        let svg = std::fs::read_to_string(svg_path).unwrap();
        for (id, label) in [("red", "Red"), ("green", "Green"), ("blue", "Blue")] {
            assert!(svg.contains(&format!("id=\"toniator-{id}\"")));
            assert!(svg.contains(&format!("inkscape:label=\"{label}\"")));
        }
        assert!(svg.contains("mix-blend-mode:screen"));
        assert!(!svg.contains("toniator-cyan"));
        assert!(!svg.contains("toniator-black"));
        usvg::Tree::from_data(svg.as_bytes(), &usvg::Options::default()).unwrap();
    }

    #[test]
    fn cancelled_rgb_shapes_preview_is_discarded_before_stale_install() {
        let mut document = Document::new(SourceArtwork {
            name: "dense-rgb.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from(test_png()),
        });
        document.output_mode = OutputMode::RgbScreen;
        document.render = RenderVariant::WebShapeV1 {
            settings: Box::new(WebShapeSettings {
                output_width: 900,
                output_height: 900,
                long_edge_cells: 900.0,
                value_mode: ValueMode::Rgb,
                ..Default::default()
            }),
        };
        let token = CancellationToken::new();
        assert!(token.cancel());
        assert!(render_document_preview_cancellable(&document, 900, 11, &token).is_err());

        let gate = RenderGate::default();
        let stale_generation = gate.next();
        let current_generation = gate.next();
        assert!(!gate.accepts(stale_generation));
        assert!(gate.accepts(current_generation));
    }

    #[test]
    fn rgb_shape_sampling_reuses_matching_grids_and_checks_cancellation_on_hits() {
        let source = RgbaImage::from_pixel(4, 4, Rgba([24, 96, 192, 255]));
        let token = CancellationToken::new();
        let mut cache = HashMap::new();

        let first = cached_web_samples(&mut cache, &source, 3, 2, &token).unwrap();
        assert_eq!(first.len(), 6);
        assert_eq!(cache.len(), 1);
        let second = cached_web_samples(&mut cache, &source, 3, 2, &token).unwrap();
        assert_eq!(second.len(), 6);
        assert_eq!(cache.len(), 1, "matching RGB channel grids share samples");
        cached_web_samples(&mut cache, &source, 4, 2, &token).unwrap();
        assert_eq!(
            cache.len(),
            2,
            "different resolution scales keep distinct grids"
        );

        assert!(token.cancel());
        assert!(cached_web_samples(&mut cache, &source, 3, 2, &token).is_err());
    }

    #[test]
    fn all_web_value_modes_match_reference_vectors_and_alpha_semantics() {
        let enabled = Ink::ALL;
        let gray = [192, 192, 192, 255];
        let darkness = 63.0 / 255.0;
        assert_eq!(
            map_web_pixel([255, 0, 0, 255], ValueMode::Cmyk, Ink::Black, &enabled),
            [0.0, 1.0, 1.0, 0.0]
        );
        assert_eq!(
            map_web_pixel(gray, ValueMode::Luminance, Ink::Black, &enabled),
            [darkness; 4]
        );
        assert_eq!(
            map_web_pixel(gray, ValueMode::SingleChannel, Ink::Magenta, &enabled),
            [0.0, darkness, 0.0, 0.0]
        );
        assert_eq!(
            map_web_pixel(
                [0, 0, 0, 255],
                ValueMode::CrosshatchLuminance,
                Ink::Black,
                &enabled
            ),
            [0.25, 0.25, 0.25, 0.25]
        );
        assert_eq!(
            map_web_pixel(
                [0, 0, 0, 255],
                ValueMode::CrosshatchLuminance,
                Ink::Black,
                &[Ink::Cyan, Ink::Black]
            ),
            [0.5, 0.0, 0.0, 0.5]
        );
        assert_eq!(
            map_web_pixel([20, 30, 40, 0], ValueMode::Cmyk, Ink::Black, &enabled),
            [0.0; 4]
        );
        assert_eq!(
            map_web_pixel([0, 0, 0, 1], ValueMode::Cmyk, Ink::Black, &enabled),
            [0.0, 0.0, 0.0, 1.0],
            "web mapping ignores nonzero alpha rather than multiplying values"
        );
    }

    #[test]
    fn web_grid_and_threshold_match_reference_boundaries() {
        assert_eq!(
            calculate_web_grid(900, 620, 92.0),
            WebGrid {
                cols: 92,
                rows: 63,
                cell_width: 900.0 / 92.0,
                cell_height: 620.0 / 63.0,
            }
        );
        let effective = calculate_web_grid(900, 620, 92.0 * 2.0);
        assert_eq!((effective.cols, effective.rows), (184, 127));
        assert_eq!(map_web_threshold(0.49, 0.5), 0.0);
        assert!((map_web_threshold(0.75, 0.5) - 0.5).abs() < f64::EPSILON);
        assert_eq!(map_web_threshold(1.0, 0.999), 1.0);
    }

    #[test]
    fn web_downsampling_is_deterministic_and_premultiplies_alpha_like_canvas() {
        let source = RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                Rgba([0, 0, 0, 0])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let first = sample_web_image(&source, 1, 1);
        let second = sample_web_image(&source, 1, 1);
        assert_eq!(first, second);
        assert_eq!(first, vec![[255, 255, 255, 128]]);
    }

    #[test]
    fn web_lattice_phase_rotation_and_pivot_have_golden_positions() {
        let source = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        let base = generate_web_shape_marks(&source, &web_settings());
        assert!(
            base.marks
                .iter()
                .any(|mark| mark.x == 25.0 && mark.y == 25.0)
        );

        let mut phase = web_settings();
        phase.channels.k.offset_x = 60.0;
        let phased = generate_web_shape_marks(&source, &phase);
        assert!(
            phased
                .marks
                .iter()
                .any(|mark| mark.x == 35.0 && mark.y == 25.0)
        );

        let mut rotated = web_settings();
        rotated.channels.k.grid_rotation = 90.0;
        let marks = generate_web_shape_marks(&source, &rotated);
        assert!(
            marks
                .marks
                .iter()
                .any(|mark| (mark.x - 75.0).abs() < 0.001 && (mark.y - 25.0).abs() < 0.001)
        );

        rotated.channels.k.grid_pivot_x = 10.0;
        let pivoted = generate_web_shape_marks(&source, &rotated);
        assert!(
            pivoted
                .marks
                .iter()
                .any(|mark| (mark.x - 85.0).abs() < 0.001 && (mark.y - 15.0).abs() < 0.001)
        );
    }

    #[test]
    fn per_channel_resolution_and_all_five_shapes_are_resolved() {
        let source = RgbaImage::from_pixel(8, 4, Rgba([0, 0, 0, 255]));
        let mut settings = web_settings();
        settings.output_width = 100;
        settings.output_height = 50;
        settings.long_edge_cells = 4.0;
        settings.channels.k.resolution_scale = 2.0;
        let marks = generate_web_shape_marks(&source, &settings);
        assert_eq!(marks.marks.len(), 32);

        settings.use_shared_mark = true;
        for shape in [
            WebShape::Circle,
            WebShape::Rectangle,
            WebShape::Triangle,
            WebShape::Pentagon,
            WebShape::Hexagon,
        ] {
            settings.shared_shape = shape;
            let marks = generate_web_shape_marks(&source, &settings);
            assert!(marks.marks.iter().all(|mark| matches!(
                mark.geometry,
                MarkGeometry::WebShape(ResolvedWebShape::Circle)
                    | MarkGeometry::WebShape(ResolvedWebShape::Polygon(_))
            )));
        }

        settings.use_shared_mark = false;
        settings.channels.k.shape = WebShape::Hexagon;
        assert!(
            generate_web_shape_marks(&source, &settings)
                .marks
                .iter()
                .all(|mark| matches!(
                    mark.geometry,
                    MarkGeometry::WebShape(ResolvedWebShape::Polygon(_))
                ))
        );
    }

    #[test]
    fn independent_channels_render_circle_triangle_hexagon_and_cubic_together() {
        let source = RgbaImage::from_pixel(32, 24, image::Rgba([0, 0, 0, 255]));
        let mut settings = WebShapeSettings {
            output_width: 320,
            output_height: 240,
            long_edge_cells: 8.0,
            value_mode: ValueMode::Luminance,
            use_shared_mark: false,
            ..Default::default()
        };
        settings.channels.c.shape = WebShape::Circle;
        settings.channels.m.shape = WebShape::RegularPolygon;
        settings.channels.m.polygon_sides = 3;
        settings.channels.y.shape = WebShape::RegularPolygon;
        settings.channels.y.polygon_sides = 6;
        settings.channels.k.shape = WebShape::UserDefined;
        settings.channels.k.custom_shape_path = Some(crate::model::ClosedShapePath::from_polygon(
            &crate::model::default_shape_nodes(),
        ));
        let marks = generate_web_shape_marks(&source, &settings);
        let geometry = |channel| {
            marks
                .marks
                .iter()
                .find(|mark| mark.channel == channel)
                .unwrap()
                .geometry
                .clone()
        };
        assert!(matches!(
            geometry(Channel::Cyan),
            MarkGeometry::WebShape(ResolvedWebShape::Circle)
        ));
        assert!(
            matches!(geometry(Channel::Magenta), MarkGeometry::WebShape(ResolvedWebShape::Polygon(points)) if points.len() == 3)
        );
        assert!(
            matches!(geometry(Channel::Yellow), MarkGeometry::WebShape(ResolvedWebShape::Polygon(points)) if points.len() == 6)
        );
        assert!(matches!(
            geometry(Channel::Black),
            MarkGeometry::WebShape(ResolvedWebShape::Cubic { .. })
        ));
    }

    #[test]
    fn user_polygon_is_resolved_once_and_anisotropic_rotation_renders() {
        let source = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        let mut settings = web_settings();
        settings.shared_shape = WebShape::UserDefined;
        settings.custom_nodes = vec![
            crate::model::ShapePoint { x: -0.5, y: -0.4 },
            crate::model::ShapePoint { x: 0.5, y: -0.4 },
            crate::model::ShapePoint { x: 0.0, y: 0.5 },
        ];
        settings.channels.k.width_scale = 2.0;
        settings.channels.k.height_scale = 0.5;
        settings.channels.k.rotation = 37.0;
        let marks = generate_web_shape_marks(&source, &settings);
        let cubics: Vec<_> = marks
            .marks
            .iter()
            .filter_map(|mark| match &mark.geometry {
                MarkGeometry::WebShape(ResolvedWebShape::Cubic { segments, .. }) => Some(segments),
                _ => None,
            })
            .collect();
        assert!(!cubics.is_empty());
        assert!(cubics.windows(2).all(|pair| Arc::ptr_eq(pair[0], pair[1])));
        assert!(marks.marks.iter().all(|mark| mark.extent > mark.thickness));
        assert!(marks.marks.iter().all(|mark| mark_path(mark).is_some()));
    }

    #[test]
    fn transformed_custom_radius_retains_marks_entering_artboard_edge() {
        let mut settings = web_settings();
        settings.output_width = 100;
        settings.output_height = 100;
        settings.long_edge_cells = 4.0;
        settings.max_mark = 100.0;
        settings.grid_scale = 100.0;
        settings.shared_shape = WebShape::UserDefined;
        settings.custom_nodes = vec![
            crate::model::ShapePoint { x: -1.2, y: -0.2 },
            crate::model::ShapePoint { x: 1.2, y: -0.2 },
            crate::model::ShapePoint { x: 0.0, y: 0.4 },
        ];
        let channel = &mut settings.channels.k;
        channel.grid_rotation = 31.0;
        channel.rotation = 47.0;
        channel.width_scale = 2.0;
        channel.height_scale = 0.5;
        let grid = calculate_web_grid(100, 100, 4.0);
        let margin = max_web_shape_extent(&settings, &settings.channels.k, grid);
        assert!(
            margin > 50.0,
            "custom radius must exceed the old 25px bound"
        );
        let ranges = web_grid_ranges(&settings, &settings.channels.k, grid);
        let entering = (ranges.2..=ranges.3).any(|row| {
            (ranges.0..=ranges.1).any(|col| {
                let placement =
                    web_shape_placement(col, row, &settings, &settings.channels.k, grid);
                placement.visible
                    && (placement.x < 0.0
                        || placement.x > 100.0
                        || placement.y < 0.0
                        || placement.y > 100.0)
                    && (-margin..=100.0 + margin).contains(&placement.x)
                    && (-margin..=100.0 + margin).contains(&placement.y)
            })
        });
        assert!(
            entering,
            "an off-artboard center whose mark enters must be retained"
        );
    }

    #[test]
    fn stale_render_result_is_rejected() {
        let gate = RenderGate::default();
        let first = gate.next();
        let second = gate.next();
        assert!(!gate.accepts(first));
        assert!(gate.accepts(second));
    }

    #[test]
    fn appearance_compositing_is_exact_and_mark_generation_is_invariant() {
        let artwork = RgbaImage::from_pixel(1, 1, Rgba([200, 100, 0, 128]));
        let preview = composite_preview(
            artwork.clone(),
            DocumentAppearance {
                preview_surface: PreviewSurface::Color {
                    color: RgbaColor {
                        red: 20,
                        green: 40,
                        blue: 60,
                        alpha: 128,
                    },
                },
                export_background: ExportBackground::Color {
                    color: RgbaColor::opaque(10, 20, 30),
                },
            },
        );
        assert_eq!(preview.get_pixel(0, 0).0, [105, 60, 15, 255]);
        let checker = composite_preview(
            RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 0])),
            DocumentAppearance {
                preview_surface: PreviewSurface::Checkerboard,
                export_background: ExportBackground::None,
            },
        );
        assert_eq!(checker.get_pixel(0, 0).0, [232, 232, 232, 255]);
        let export = composite_export_background(artwork, ExportBackground::None);
        assert_eq!(
            export.get_pixel(0, 0).0[3],
            128,
            "checkerboard never enters exports"
        );

        let mut document = crate::model::Document::new(SourceArtwork {
            name: "fixture.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from(test_png()),
        });
        let marks = generate_document_marks(&document).unwrap();
        document.appearance.preview_surface = PreviewSurface::Checkerboard;
        document.appearance.export_background = ExportBackground::Color {
            color: RgbaColor::opaque(4, 5, 6),
        };
        assert_eq!(generate_document_marks(&document).unwrap(), marks);

        let mut curve = document.clone();
        curve.render = RenderVariant::WebCurveV1 {
            settings: Box::new(crate::model::WebCurveSettings {
                output_width: 80,
                output_height: 60,
                ..Default::default()
            }),
        };
        let source = decode_source(&curve.source, 2400).unwrap();
        let before = crate::curve_render::generate_curve_geometry(
            &source,
            match &curve.render {
                RenderVariant::WebCurveV1 { settings } => settings,
                _ => unreachable!(),
            },
        )
        .unwrap();
        curve.appearance.preview_surface = PreviewSurface::Color {
            color: RgbaColor {
                red: 90,
                green: 80,
                blue: 70,
                alpha: 60,
            },
        };
        let after = crate::curve_render::generate_curve_geometry(
            &source,
            match &curve.render {
                RenderVariant::WebCurveV1 { settings } => settings,
                _ => unreachable!(),
            },
        )
        .unwrap();
        assert_eq!(before, after, "appearance cannot affect curve geometry");
    }

    #[test]
    fn opaque_export_background_makes_full_document_preview_match_export_for_shapes_and_curves() {
        let source = SourceArtwork {
            name: "equivalence.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from(test_png()),
        };
        let mut shape = crate::model::Document::new(source.clone());
        shape.appearance = DocumentAppearance {
            preview_surface: PreviewSurface::Color {
                color: RgbaColor::opaque(240, 240, 240),
            },
            export_background: ExportBackground::Color {
                color: RgbaColor::opaque(12, 34, 56),
            },
        };
        let preview = render_document_preview(&shape, 2, 1).unwrap().image;
        let export =
            render_document_export_cancellable(&shape, 2, 2, None, &CancellationToken::new())
                .unwrap();
        assert_eq!(preview, export);
        shape.appearance.preview_surface = PreviewSurface::Checkerboard;
        assert_eq!(
            render_document_export_cancellable(&shape, 2, 2, None, &CancellationToken::new())
                .unwrap(),
            export,
            "preview-only surface cannot leak into export"
        );

        let mut curve = crate::model::Document::new(source);
        curve.render = RenderVariant::WebCurveV1 {
            settings: Box::new(crate::model::WebCurveSettings {
                output_width: 80,
                output_height: 80,
                ..Default::default()
            }),
        };
        curve.appearance = shape.appearance;
        let preview = render_document_preview(&curve, 80, 2).unwrap().image;
        let export =
            render_document_export_cancellable(&curve, 80, 80, None, &CancellationToken::new())
                .unwrap();
        assert_eq!(preview, export);
    }

    #[test]
    fn preview_ink_uses_multiply_blending() {
        let layer = InkLayer {
            channel: Channel::Cyan,
            enabled: true,
            color: Channel::Cyan.color(),
            opacity: 1.0,
        };
        assert_eq!(layer_paint(&layer).blend_mode, BlendMode::Multiply);
    }

    #[test]
    fn invalid_source_validation_does_not_mutate_current_document() {
        let current = crate::model::Document::new(SourceArtwork {
            name: "current.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from([1]),
        });
        let before = current.clone();
        let invalid = SourceArtwork {
            name: "broken.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from([0, 1, 2, 3]),
        };
        assert!(decode_source(&invalid, 128).is_err());
        assert_eq!(current, before);
    }
}
