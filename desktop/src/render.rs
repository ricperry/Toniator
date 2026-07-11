use crate::model::{
    Document, Ink, RenderVariant, Settings, SourceArtwork, Treatment, ValueMode, WebShape,
    WebShapeChannel, WebShapeSettings, parse_hex_color,
};
use anyhow::{Context, Result, bail};
use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage, imageops::FilterType};
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
}

impl From<Ink> for Channel {
    fn from(value: Ink) -> Self {
        match value {
            Ink::Cyan => Self::Cyan,
            Ink::Magenta => Self::Magenta,
            Ink::Yellow => Self::Yellow,
            Ink::Black => Self::Black,
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
        }
    }

    pub fn color(self) -> (u8, u8, u8) {
        match self {
            Channel::Cyan => (0, 174, 239),
            Channel::Magenta => (236, 0, 140),
            Channel::Yellow => (255, 242, 0),
            Channel::Black => (20, 20, 24),
        }
    }

    fn angle_offset(self) -> f32 {
        match self {
            Channel::Cyan => 15.0,
            Channel::Magenta => 75.0,
            Channel::Yellow => 0.0,
            Channel::Black => 45.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkGeometry {
    Native,
    WebShape(WebShape),
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

    MarkSet {
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
    }
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
    document.validate()?;
    let source = decode_source(&document.source, 2400)?;
    Ok(match &document.render {
        RenderVariant::NativeBasicV1 => generate_marks(&source, document.settings),
        RenderVariant::WebShapeV1 { settings } => generate_web_shape_marks(&source, settings),
        RenderVariant::WebCurveV1 { .. } => {
            bail!("full-width curve rendering is not available yet")
        }
    })
}

pub fn generate_web_shape_marks(source: &RgbaImage, settings: &WebShapeSettings) -> MarkSet {
    let enabled: Vec<Ink> = Ink::ALL
        .into_iter()
        .filter(|ink| settings.channels.get(*ink).enabled)
        .collect();
    let mut marks = Vec::new();

    for ink in Ink::ALL {
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
        let samples = sample_web_image(source, grid.cols, grid.rows);
        let ranges = web_grid_ranges(settings, channel, grid);
        let shape = if settings.use_shared_mark {
            settings.shared_shape
        } else {
            channel.shape
        };

        for row in ranges.2..=ranges.3 {
            for col in ranges.0..=ranges.1 {
                let placement = web_shape_placement(col, row, settings, channel, grid);
                if !placement.visible {
                    continue;
                }
                let sample =
                    samples[(placement.sample_row * grid.cols + placement.sample_col) as usize];
                let raw = map_web_pixel(
                    sample,
                    settings.value_mode,
                    settings.single_channel,
                    &enabled,
                )[ink_index(ink)];
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
                    extent: (size * settings.grid_scale / 100.0) as f32,
                    thickness: 0.0,
                    angle: channel.rotation as f32,
                    treatment: Treatment::Dots,
                    geometry: MarkGeometry::WebShape(shape),
                });
            }
        }
    }

    let layers = Ink::ALL
        .into_iter()
        .map(|ink| {
            let channel = settings.channels.get(ink);
            let color = if settings.value_mode == ValueMode::CrosshatchLuminance {
                (17, 17, 17)
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

    MarkSet {
        width: settings.output_width,
        height: settings.output_height,
        marks,
        layers,
    }
}

pub(crate) fn sample_web_image(source: &RgbaImage, cols: u32, rows: u32) -> Vec<[u8; 4]> {
    // Canvas drawImage uses interpolated area reduction. Triangle filtering is
    // deterministic; explicitly filtering premultiplied colors matches Canvas
    // compositing at translucent edges before getImageData unpremultiplies.
    let premultiplied = image::Rgba32FImage::from_fn(source.width(), source.height(), |x, y| {
        let pixel = source.get_pixel(x, y).0;
        let alpha = pixel[3] as f32 / 255.0;
        image::Rgba([
            pixel[0] as f32 / 255.0 * alpha,
            pixel[1] as f32 / 255.0 * alpha,
            pixel[2] as f32 / 255.0 * alpha,
            alpha,
        ])
    });
    image::imageops::resize(&premultiplied, cols, rows, FilterType::Triangle)
        .pixels()
        .map(|pixel| {
            let alpha = pixel[3].clamp(0.0, 1.0);
            if alpha <= f32::EPSILON {
                return [0, 0, 0, 0];
            }
            [
                (pixel[0] / alpha * 255.0).round().clamp(0.0, 255.0) as u8,
                (pixel[1] / alpha * 255.0).round().clamp(0.0, 255.0) as u8,
                (pixel[2] / alpha * 255.0).round().clamp(0.0, 255.0) as u8,
                (alpha * 255.0).round() as u8,
            ]
        })
        .collect()
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
    let darkness = 1.0
        - (0.2126 * pixel[0] as f64 + 0.7152 * pixel[1] as f64 + 0.0722 * pixel[2] as f64) / 255.0;
    match mode {
        ValueMode::Cmyk => unreachable!(),
        ValueMode::Luminance => [darkness; 4],
        ValueMode::InvertedLuminance => [1.0 - darkness; 4],
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
    grid.cell_width.min(grid.cell_height) * max * channel.scale * settings.grid_scale / 100.0 / 2.0
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
    if let RenderVariant::WebCurveV1 { settings } = &document.render {
        document.validate()?;
        let source = decode_source(&document.source, 2400)?;
        let geometry = crate::curve_render::generate_curve_geometry(&source, settings)?;
        return crate::curve_render::render_curve_geometry(&geometry, max_dimension, generation);
    }
    let marks = generate_document_marks(document)?;
    render_mark_set(&marks, max_dimension, generation)
}

fn render_mark_set(marks: &MarkSet, max_dimension: u32, generation: u64) -> Result<RenderResult> {
    let scale = (max_dimension as f32 / marks.width.max(marks.height) as f32).min(1.0);
    let width = (marks.width as f32 * scale).round().max(1.0) as u32;
    let height = (marks.height as f32 * scale).round().max(1.0) as u32;
    let image = render_mark_set_output(marks, width, height, true, None)?;
    Ok(RenderResult { generation, image })
}

pub fn render_document_output(
    document: &Document,
    width: u32,
    height: u32,
    white_background: bool,
    channel: Option<Ink>,
) -> Result<RgbaImage> {
    document.validate()?;
    anyhow::ensure!(
        width > 0 && height > 0,
        "output dimensions must be positive"
    );
    let pixels = u64::from(width) * u64::from(height);
    anyhow::ensure!(
        pixels <= 64_000_000,
        "PNG output exceeds the safe 64 megapixel limit"
    );
    if let RenderVariant::WebCurveV1 { settings } = &document.render {
        let source = decode_source(&document.source, 2400)?;
        let geometry = crate::curve_render::generate_curve_geometry(&source, settings)?;
        return crate::curve_render::render_curve_geometry_output(
            &geometry,
            width,
            height,
            white_background,
            channel,
        );
    }
    let marks = generate_document_marks(document)?;
    render_mark_set_output(&marks, width, height, white_background, channel)
}

fn render_mark_set_output(
    marks: &MarkSet,
    width: u32,
    height: u32,
    white_background: bool,
    channel: Option<Ink>,
) -> Result<RgbaImage> {
    let mut pixmap = Pixmap::new(width, height).context("output is too large")?;
    if white_background {
        pixmap.fill(Color::WHITE);
    }
    let scale_x = width as f32 / marks.width as f32;
    let scale_y = height as f32 / marks.height as f32;

    for layer in marks.layers.iter().filter(|layer| layer.enabled) {
        if channel.is_some_and(|ink| layer.channel != Channel::from(ink)) {
            continue;
        }
        let paint = layer_paint(layer);
        for mark in marks
            .marks
            .iter()
            .filter(|mark| mark.channel == layer.channel)
        {
            if let Some(path) = mark_path(*mark) {
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
    paint.blend_mode = BlendMode::Multiply;
    paint.anti_alias = true;
    paint
}

fn mark_path(mark: Mark) -> Option<tiny_skia::Path> {
    if let MarkGeometry::WebShape(shape) = mark.geometry {
        return web_shape_path(mark.x, mark.y, mark.extent, mark.angle, shape);
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
    degrees: f32,
    shape: WebShape,
) -> Option<tiny_skia::Path> {
    let mut path = PathBuilder::new();
    let transform = |x: f32, y: f32| {
        let radians = degrees.to_radians();
        let (sin, cos) = radians.sin_cos();
        let x = x * scale;
        let y = y * scale;
        (cx + x * cos - y * sin, cy + x * sin + y * cos)
    };
    if shape == WebShape::Circle {
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
    } else {
        let points: &[(f32, f32)] = match shape {
            WebShape::Circle => unreachable!(),
            WebShape::Rectangle => &[(-0.45, -0.45), (0.45, -0.45), (0.45, 0.45), (-0.45, 0.45)],
            WebShape::Triangle => &[(0.0, -0.52), (0.5, 0.4), (-0.5, 0.4)],
            WebShape::Pentagon => &[
                (0.0, -0.5),
                (0.4755, -0.1545),
                (0.2939, 0.4045),
                (-0.2939, 0.4045),
                (-0.4755, -0.1545),
            ],
            WebShape::Hexagon => &[
                (0.433, -0.25),
                (0.433, 0.25),
                (0.0, 0.5),
                (-0.433, 0.25),
                (-0.433, -0.25),
                (0.0, -0.5),
            ],
        };
        let first = transform(points[0].0, points[0].1);
        path.move_to(first.0, first.1);
        for &(x, y) in &points[1..] {
            let point = transform(x, y);
            path.line_to(point.0, point.1);
        }
    }
    path.close();
    path.finish()
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
            map_web_pixel(gray, ValueMode::InvertedLuminance, Ink::Black, &enabled),
            [1.0 - darkness; 4]
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
            assert!(
                marks
                    .marks
                    .iter()
                    .all(|mark| mark.geometry == MarkGeometry::WebShape(shape))
            );
        }

        settings.use_shared_mark = false;
        settings.channels.k.shape = WebShape::Hexagon;
        assert!(
            generate_web_shape_marks(&source, &settings)
                .marks
                .iter()
                .all(|mark| mark.geometry == MarkGeometry::WebShape(WebShape::Hexagon))
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
