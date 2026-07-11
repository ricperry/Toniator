use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DOCUMENT_FORMAT: &str = "toniator-document";
pub const DOCUMENT_VERSION: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Treatment {
    #[default]
    Dots,
    Squares,
    Lines,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub treatment: Treatment,
    /// Creative-facing scale: 0 is broad/coarse, 100 is fine/detailed.
    pub detail: f32,
    /// Overall mark size, expressed as a percentage.
    pub coverage: f32,
    /// Input channel contrast, expressed as a percentage.
    pub contrast: f32,
    /// Base screen angle in degrees.
    pub angle: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            treatment: Treatment::Dots,
            detail: 52.0,
            coverage: 92.0,
            contrast: 108.0,
            angle: 0.0,
        }
    }
}

impl Settings {
    pub fn sanitized(mut self) -> Self {
        self.detail = finite_clamp(self.detail, 0.0, 100.0, 52.0);
        self.coverage = finite_clamp(self.coverage, 0.0, 160.0, 92.0);
        self.contrast = finite_clamp(self.contrast, 0.0, 200.0, 108.0);
        self.angle = finite_clamp(self.angle, -180.0, 180.0, 0.0);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ValueMode {
    Cmyk,
    Luminance,
    CrosshatchLuminance,
    InvertedLuminance,
    #[default]
    SingleChannel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Ink {
    Cyan,
    Magenta,
    Yellow,
    #[default]
    Black,
}

impl Ink {
    pub const ALL: [Self; 4] = [Self::Cyan, Self::Magenta, Self::Yellow, Self::Black];

    pub fn id(self) -> &'static str {
        match self {
            Self::Cyan => "c",
            Self::Magenta => "m",
            Self::Yellow => "y",
            Self::Black => "k",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cyan => "Cyan",
            Self::Magenta => "Magenta",
            Self::Yellow => "Yellow",
            Self::Black => "Black",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WebShape {
    #[default]
    Circle,
    Rectangle,
    Triangle,
    Pentagon,
    Hexagon,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebShapeChannel {
    pub enabled: bool,
    pub color: String,
    /// Rotation of the mark within its sampling cell, in degrees.
    pub rotation: f64,
    /// Rotation of both sampling and placement lattice, in degrees.
    pub grid_rotation: f64,
    /// Artboard-space offsets from the artboard center.
    pub grid_pivot_x: f64,
    pub grid_pivot_y: f64,
    pub scale: f64,
    pub threshold: f64,
    /// Percentage multiplier applied after the global maximum mark size.
    pub max_size: f64,
    pub resolution_scale: f64,
    /// Periodic lattice phase. Values are wrapped to one signed cell.
    pub offset_x: f64,
    pub offset_y: f64,
    pub opacity: f64,
    pub shape: WebShape,
}

impl Default for WebShapeChannel {
    fn default() -> Self {
        Self {
            enabled: true,
            color: "#111111".into(),
            rotation: 0.0,
            grid_rotation: 0.0,
            grid_pivot_x: 0.0,
            grid_pivot_y: 0.0,
            scale: 1.0,
            threshold: 0.0,
            max_size: 100.0,
            resolution_scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            opacity: 1.0,
            shape: WebShape::Circle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebShapeChannels {
    pub c: WebShapeChannel,
    pub m: WebShapeChannel,
    pub y: WebShapeChannel,
    pub k: WebShapeChannel,
}

impl WebShapeChannels {
    pub fn get(&self, ink: Ink) -> &WebShapeChannel {
        match ink {
            Ink::Cyan => &self.c,
            Ink::Magenta => &self.m,
            Ink::Yellow => &self.y,
            Ink::Black => &self.k,
        }
    }

    pub fn get_mut(&mut self, ink: Ink) -> &mut WebShapeChannel {
        match ink {
            Ink::Cyan => &mut self.c,
            Ink::Magenta => &mut self.m,
            Ink::Yellow => &mut self.y,
            Ink::Black => &mut self.k,
        }
    }
}

impl Default for WebShapeChannels {
    fn default() -> Self {
        let channel = |color: &str, grid_rotation| WebShapeChannel {
            color: color.into(),
            grid_rotation,
            ..Default::default()
        };
        Self {
            c: channel("#00aeef", 15.0),
            m: channel("#ec008c", 75.0),
            y: channel("#ffd400", 0.0),
            k: channel("#111111", 45.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebShapeDeltas {
    pub rotation_delta: f64,
    pub grid_rotation_delta: f64,
    pub grid_pivot_x_delta: f64,
    pub grid_pivot_y_delta: f64,
    pub scale_multiplier: f64,
    pub resolution_multiplier: f64,
    pub threshold_delta: f64,
    pub max_size_multiplier: f64,
    pub opacity_multiplier: f64,
    pub offset_x_delta: f64,
    pub offset_y_delta: f64,
}

impl Default for WebShapeDeltas {
    fn default() -> Self {
        Self {
            rotation_delta: 0.0,
            grid_rotation_delta: 0.0,
            grid_pivot_x_delta: 0.0,
            grid_pivot_y_delta: 0.0,
            scale_multiplier: 1.0,
            resolution_multiplier: 1.0,
            threshold_delta: 0.0,
            max_size_multiplier: 1.0,
            opacity_multiplier: 1.0,
            offset_x_delta: 0.0,
            offset_y_delta: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebShapeSettings {
    pub output_width: u32,
    pub output_height: u32,
    pub long_edge_cells: f64,
    pub grid_scale: f64,
    pub min_mark: f64,
    pub max_mark: f64,
    pub value_mode: ValueMode,
    pub single_channel: Ink,
    pub use_shared_mark: bool,
    pub shared_shape: WebShape,
    pub channels: WebShapeChannels,
}

impl Default for WebShapeSettings {
    fn default() -> Self {
        Self {
            output_width: 900,
            output_height: 620,
            long_edge_cells: 92.0,
            grid_scale: 92.0,
            min_mark: 0.0,
            max_mark: 85.0,
            value_mode: ValueMode::Cmyk,
            single_channel: Ink::Black,
            use_shared_mark: true,
            shared_shape: WebShape::Circle,
            channels: WebShapeChannels::default(),
        }
    }
}

impl WebShapeSettings {
    /// Flattens the web preset's base-plus-delta representation into effective
    /// channels. The native model deliberately stores no live delta layer.
    pub fn apply_deltas(&mut self, deltas: WebShapeDeltas) {
        for ink in Ink::ALL {
            let channel = self.channels.get_mut(ink);
            channel.rotation += deltas.rotation_delta;
            channel.grid_rotation += deltas.grid_rotation_delta;
            channel.grid_pivot_x += deltas.grid_pivot_x_delta;
            channel.grid_pivot_y += deltas.grid_pivot_y_delta;
            channel.scale *= deltas.scale_multiplier;
            channel.resolution_scale *= deltas.resolution_multiplier;
            channel.threshold = (channel.threshold + deltas.threshold_delta).clamp(0.0, 1.0);
            channel.max_size *= deltas.max_size_multiplier;
            channel.opacity = (channel.opacity * deltas.opacity_multiplier).clamp(0.0, 1.0);
            channel.offset_x += deltas.offset_x_delta;
            channel.offset_y += deltas.offset_y_delta;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CurvePoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CubicCurveSegment {
    pub control_1: CurvePoint,
    pub control_2: CurvePoint,
    pub end: CurvePoint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurvePath {
    pub start: CurvePoint,
    pub segments: Vec<CubicCurveSegment>,
}

impl CurvePath {
    pub fn straight() -> Self {
        Self {
            start: CurvePoint { x: -0.45, y: 0.0 },
            segments: vec![CubicCurveSegment {
                control_1: CurvePoint { x: -0.15, y: 0.0 },
                control_2: CurvePoint { x: 0.15, y: 0.0 },
                end: CurvePoint { x: 0.45, y: 0.0 },
            }],
        }
    }

    pub fn soft_wave() -> Self {
        Self {
            start: CurvePoint { x: -0.5, y: 0.0 },
            segments: vec![
                CubicCurveSegment {
                    control_1: CurvePoint { x: -0.32, y: -0.4 },
                    control_2: CurvePoint { x: -0.18, y: -0.4 },
                    end: CurvePoint { x: 0.0, y: 0.0 },
                },
                CubicCurveSegment {
                    control_1: CurvePoint { x: 0.18, y: 0.4 },
                    control_2: CurvePoint { x: 0.32, y: 0.4 },
                    end: CurvePoint { x: 0.5, y: 0.0 },
                },
            ],
        }
    }

    pub fn deep_wave() -> Self {
        let mut path = Self::soft_wave();
        for segment in &mut path.segments {
            segment.control_1.y *= 1.45;
            segment.control_2.y *= 1.45;
        }
        path
    }

    pub fn points(&self) -> impl Iterator<Item = CurvePoint> + '_ {
        std::iter::once(self.start).chain(
            self.segments
                .iter()
                .flat_map(|segment| [segment.control_1, segment.control_2, segment.end]),
        )
    }
}

impl Default for CurvePath {
    fn default() -> Self {
        Self::soft_wave()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CurveLayout {
    #[default]
    FullWidth,
    MotifPattern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MotifCoverage {
    #[default]
    Auto,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AlternateTileTransform {
    #[default]
    None,
    Flip,
    Rotate180,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebCurveChannel {
    pub enabled: bool,
    pub color: String,
    pub grid_rotation: f64,
    pub grid_pivot_x: f64,
    pub grid_pivot_y: f64,
    pub scale: f64,
    pub threshold: f64,
    pub max_size: f64,
    pub resolution_scale: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub opacity: f64,
    pub output_quality: f64,
    pub curve_scale: f64,
    pub motif_coverage: MotifCoverage,
    pub motif_bleed: f64,
    pub tile_count: u32,
    pub tile_spacing: f64,
    pub tile_angle: f64,
    pub tile_offset: f64,
    pub stack_count: u32,
    pub stack_spacing: f64,
    pub stack_angle: f64,
    pub stack_offset: f64,
    pub alternate_stack_offset: f64,
    pub alternate_tile_transform: AlternateTileTransform,
    pub path: CurvePath,
    pub close_ends: bool,
    pub smooth_join: bool,
}

impl Default for WebCurveChannel {
    fn default() -> Self {
        Self {
            enabled: true,
            color: "#111111".into(),
            grid_rotation: 0.0,
            grid_pivot_x: 0.0,
            grid_pivot_y: 0.0,
            scale: 1.0,
            threshold: 0.04,
            max_size: 100.0,
            resolution_scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            opacity: 0.92,
            output_quality: 1.0,
            curve_scale: 32.0,
            motif_coverage: MotifCoverage::Auto,
            motif_bleed: 2.0,
            tile_count: 2,
            tile_spacing: 0.0,
            tile_angle: 0.0,
            tile_offset: 0.0,
            stack_count: 2,
            stack_spacing: 36.0,
            stack_angle: 0.0,
            stack_offset: 0.0,
            alternate_stack_offset: 0.0,
            alternate_tile_transform: AlternateTileTransform::None,
            path: CurvePath::default(),
            close_ends: false,
            smooth_join: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebCurveChannels {
    pub c: WebCurveChannel,
    pub m: WebCurveChannel,
    pub y: WebCurveChannel,
    pub k: WebCurveChannel,
}

impl WebCurveChannels {
    pub fn get(&self, ink: Ink) -> &WebCurveChannel {
        match ink {
            Ink::Cyan => &self.c,
            Ink::Magenta => &self.m,
            Ink::Yellow => &self.y,
            Ink::Black => &self.k,
        }
    }

    pub fn get_mut(&mut self, ink: Ink) -> &mut WebCurveChannel {
        match ink {
            Ink::Cyan => &mut self.c,
            Ink::Magenta => &mut self.m,
            Ink::Yellow => &mut self.y,
            Ink::Black => &mut self.k,
        }
    }
}

impl Default for WebCurveChannels {
    fn default() -> Self {
        let channel = |color: &str, grid_rotation| WebCurveChannel {
            color: color.into(),
            grid_rotation,
            ..Default::default()
        };
        Self {
            c: channel("#00aeef", 15.0),
            m: channel("#ec008c", 75.0),
            y: channel("#ffd400", 0.0),
            k: channel("#111111", 45.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebCurveSettings {
    pub output_width: u32,
    pub output_height: u32,
    pub long_edge_cells: f64,
    pub min_mark: f64,
    pub max_mark: f64,
    pub value_mode: ValueMode,
    pub single_channel: Ink,
    pub layout: CurveLayout,
    pub use_shared_curve: bool,
    pub shared_path: CurvePath,
    pub shared_close_ends: bool,
    pub shared_smooth_join: bool,
    pub show_background: bool,
    pub channels: WebCurveChannels,
}

impl Default for WebCurveSettings {
    fn default() -> Self {
        Self {
            output_width: 900,
            output_height: 620,
            long_edge_cells: 90.0,
            min_mark: 0.0,
            max_mark: 85.0,
            value_mode: ValueMode::Cmyk,
            single_channel: Ink::Black,
            layout: CurveLayout::FullWidth,
            use_shared_curve: true,
            shared_path: CurvePath::soft_wave(),
            shared_close_ends: false,
            shared_smooth_join: false,
            show_background: true,
            channels: WebCurveChannels::default(),
        }
    }
}

impl WebCurveSettings {
    pub fn apply_deltas(&mut self, deltas: WebShapeDeltas, output_quality_multiplier: f64) {
        for ink in Ink::ALL {
            let channel = self.channels.get_mut(ink);
            channel.grid_rotation += deltas.grid_rotation_delta;
            channel.grid_pivot_x += deltas.grid_pivot_x_delta;
            channel.grid_pivot_y += deltas.grid_pivot_y_delta;
            channel.scale *= deltas.scale_multiplier;
            channel.resolution_scale *= deltas.resolution_multiplier;
            channel.threshold = (channel.threshold + deltas.threshold_delta).clamp(0.0, 1.0);
            channel.max_size *= deltas.max_size_multiplier;
            channel.opacity = (channel.opacity * deltas.opacity_multiplier).clamp(0.0, 1.0);
            channel.offset_x += deltas.offset_x_delta;
            channel.offset_y += deltas.offset_y_delta;
            channel.output_quality *= output_quality_multiplier;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "variant", rename_all = "kebab-case")]
pub enum RenderVariant {
    #[default]
    NativeBasicV1,
    WebShapeV1 {
        settings: Box<WebShapeSettings>,
    },
    WebCurveV1 {
        settings: Box<WebCurveSettings>,
    },
}

fn finite_clamp(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceArtwork {
    pub name: String,
    pub media_type: String,
    #[serde(with = "base64_bytes")]
    pub bytes: Arc<[u8]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub format: String,
    pub version: u32,
    #[serde(default = "new_document_id")]
    pub document_id: String,
    pub source: SourceArtwork,
    pub settings: Settings,
    #[serde(default)]
    pub render: RenderVariant,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_web_shape: Option<Box<WebShapeSettings>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_web_curve: Option<Box<WebCurveSettings>>,
}

impl Document {
    pub fn new(source: SourceArtwork) -> Self {
        Self {
            format: DOCUMENT_FORMAT.to_owned(),
            version: DOCUMENT_VERSION,
            document_id: new_document_id(),
            source,
            settings: Settings::default(),
            render: RenderVariant::NativeBasicV1,
            saved_web_shape: None,
            saved_web_curve: None,
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.format == DOCUMENT_FORMAT, "not a Toniator document");
        anyhow::ensure!(
            self.version == DOCUMENT_VERSION,
            "unsupported Toniator document version {}",
            self.version
        );
        anyhow::ensure!(
            !self.source.bytes.is_empty(),
            "document has no source artwork"
        );
        if let RenderVariant::WebShapeV1 { settings } = &self.render {
            anyhow::ensure!(
                settings.output_width > 0 && settings.output_height > 0,
                "web shape artboard has no usable size"
            );
            anyhow::ensure!(
                settings.output_width <= 100_000 && settings.output_height <= 100_000,
                "web shape artboard is too large"
            );
            anyhow::ensure!(
                [
                    settings.long_edge_cells,
                    settings.grid_scale,
                    settings.min_mark,
                    settings.max_mark,
                ]
                .into_iter()
                .all(f64::is_finite),
                "invalid web shape global setting"
            );
            anyhow::ensure!(
                settings.long_edge_cells >= 2.0 && settings.long_edge_cells <= 10_000.0,
                "web shape grid is outside the supported range"
            );
            anyhow::ensure!(
                settings.grid_scale > 0.0 && settings.grid_scale <= 1_000.0,
                "web shape cell fill is outside the supported range"
            );
            anyhow::ensure!(
                settings.min_mark >= 0.0
                    && settings.max_mark >= settings.min_mark
                    && settings.max_mark <= 1_000.0,
                "web shape mark range is outside the supported range"
            );
            for ink in Ink::ALL {
                let channel = settings.channels.get(ink);
                anyhow::ensure!(
                    [
                        channel.rotation,
                        channel.grid_rotation,
                        channel.grid_pivot_x,
                        channel.grid_pivot_y,
                        channel.scale,
                        channel.threshold,
                        channel.max_size,
                        channel.resolution_scale,
                        channel.offset_x,
                        channel.offset_y,
                        channel.opacity,
                    ]
                    .into_iter()
                    .all(f64::is_finite),
                    "invalid {} channel setting",
                    ink.label()
                );
                anyhow::ensure!(
                    parse_hex_color(&channel.color).is_some(),
                    "invalid {} ink color",
                    ink.label()
                );
                anyhow::ensure!(
                    channel.resolution_scale > 0.0 && channel.resolution_scale <= 100.0,
                    "invalid {} channel resolution",
                    ink.label()
                );
                anyhow::ensure!(
                    (0.0..=100.0).contains(&channel.scale)
                        && (0.0..=1.0).contains(&channel.threshold)
                        && (0.0..=10_000.0).contains(&channel.max_size)
                        && (0.0..=1.0).contains(&channel.opacity),
                    "{} channel value is outside the supported range",
                    ink.label()
                );
            }
        }
        if let RenderVariant::WebCurveV1 { settings } = &self.render {
            anyhow::ensure!(
                settings.output_width > 0
                    && settings.output_height > 0
                    && settings.output_width <= 100_000
                    && settings.output_height <= 100_000,
                "web curve artboard is outside the supported range"
            );
            anyhow::ensure!(
                settings.long_edge_cells.is_finite()
                    && (2.0..=10_000.0).contains(&settings.long_edge_cells)
                    && settings.min_mark.is_finite()
                    && settings.max_mark.is_finite()
                    && settings.min_mark >= 0.0
                    && settings.max_mark >= settings.min_mark
                    && settings.max_mark <= 1_000.0,
                "web curve global setting is outside the supported range"
            );
            validate_curve_path(&settings.shared_path)?;
            for ink in Ink::ALL {
                let channel = settings.channels.get(ink);
                anyhow::ensure!(
                    [
                        channel.grid_rotation,
                        channel.grid_pivot_x,
                        channel.grid_pivot_y,
                        channel.scale,
                        channel.threshold,
                        channel.max_size,
                        channel.resolution_scale,
                        channel.offset_x,
                        channel.offset_y,
                        channel.opacity,
                        channel.output_quality,
                        channel.curve_scale,
                        channel.motif_bleed,
                        channel.tile_spacing,
                        channel.tile_angle,
                        channel.tile_offset,
                        channel.stack_spacing,
                        channel.stack_angle,
                        channel.stack_offset,
                        channel.alternate_stack_offset,
                    ]
                    .into_iter()
                    .all(f64::is_finite),
                    "invalid {} curve channel setting",
                    ink.label()
                );
                anyhow::ensure!(
                    parse_hex_color(&channel.color).is_some()
                        && (0.0..=100.0).contains(&channel.scale)
                        && (0.0..=1.0).contains(&channel.threshold)
                        && (0.0..=10_000.0).contains(&channel.max_size)
                        && channel.resolution_scale > 0.0
                        && channel.resolution_scale <= 100.0
                        && (0.0..=1.0).contains(&channel.opacity)
                        && channel.output_quality > 0.0
                        && channel.output_quality <= 100.0
                        && (0.1..=500.0).contains(&channel.curve_scale)
                        && (0.0..=100.0).contains(&channel.motif_bleed)
                        && (1..=10_000).contains(&channel.tile_count)
                        && (-10_000.0..=10_000.0).contains(&channel.tile_spacing)
                        && (1..=10_000).contains(&channel.stack_count)
                        && (-10_000.0..=10_000.0).contains(&channel.stack_spacing),
                    "{} curve channel value is outside the supported range",
                    ink.label()
                );
                validate_curve_path(&channel.path)?;
            }
        }
        for saved in [
            self.saved_web_shape
                .as_ref()
                .map(|settings| RenderVariant::WebShapeV1 {
                    settings: settings.clone(),
                }),
            self.saved_web_curve
                .as_ref()
                .map(|settings| RenderVariant::WebCurveV1 {
                    settings: settings.clone(),
                }),
        ]
        .into_iter()
        .flatten()
        {
            let mut candidate = self.clone();
            candidate.render = saved;
            candidate.saved_web_shape = None;
            candidate.saved_web_curve = None;
            candidate.validate()?;
        }
        Ok(())
    }
}

fn validate_curve_path(path: &CurvePath) -> anyhow::Result<()> {
    anyhow::ensure!(
        !path.segments.is_empty() && path.segments.len() <= 64,
        "curve must contain between 1 and 64 segments"
    );
    anyhow::ensure!(
        path.points()
            .all(|point| point.x.is_finite() && point.y.is_finite()),
        "curve contains an invalid point"
    );
    Ok(())
}

pub fn parse_hex_color(color: &str) -> Option<(u8, u8, u8)> {
    let value = color.strip_prefix('#')?;
    if value.len() != 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ))
}

pub(crate) fn new_document_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{:x}-{:x}-{:x}",
        nanos,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

mod base64_bytes {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;

    pub fn serialize<S>(bytes: &Arc<[u8]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Arc<[u8]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        STANDARD
            .decode(text)
            .map(Arc::from)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKey {
    Treatment,
    Detail,
    Coverage,
    Contrast,
    Angle,
    WebCoverage,
    WebAngle,
    WebThreshold,
    WebOpacity,
    WebDetail,
    WebColor,
    CurveProfile,
    CurveLayout,
    CurvePath,
    CurveWeight,
    CurveSpacing,
    CurveCoverage,
    CurveAngle,
    CurvePositionX,
    CurvePositionY,
    CurveOpacity,
    CurveThreshold,
    CurveDetail,
    CurveColor,
    MotifSize,
    MotifColumns,
    MotifRows,
    MotifRowSpacing,
    MotifStagger,
}

#[derive(Debug, Clone)]
struct Edit {
    before: TreatmentState,
    after: TreatmentState,
}

#[derive(Debug, Clone)]
struct ActiveEdit {
    key: SettingKey,
    before: TreatmentState,
}

#[derive(Debug, Clone, PartialEq)]
struct TreatmentState {
    settings: Settings,
    render: RenderVariant,
    saved_web_shape: Option<Box<WebShapeSettings>>,
    saved_web_curve: Option<Box<WebCurveSettings>>,
}

/// Document-level undo with short edits on the same control coalesced into one drag.
pub struct DocumentEditor {
    document: Document,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    clean_state: TreatmentState,
    active: Option<ActiveEdit>,
}

impl DocumentEditor {
    pub fn new(document: Document) -> Self {
        let clean_state = TreatmentState::from_document(&document);
        Self {
            document,
            undo: Vec::new(),
            redo: Vec::new(),
            clean_state,
            active: None,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn is_dirty(&self) -> bool {
        TreatmentState::from_document(&self.document) != self.clean_state
    }

    pub fn mark_clean(&mut self) {
        self.clean_state = TreatmentState::from_document(&self.document);
    }

    pub fn begin_edit(&mut self, key: SettingKey) {
        if self.active.is_some() {
            self.end_edit();
        }
        self.active = Some(ActiveEdit {
            key,
            before: TreatmentState::from_document(&self.document),
        });
    }

    pub fn set_settings(&mut self, key: SettingKey, settings: Settings) -> bool {
        let after = settings.sanitized();
        let before = TreatmentState::from_document(&self.document);
        if before.settings == after {
            return false;
        }
        if let Some(active) = &self.active {
            debug_assert_eq!(active.key, key);
            self.document.settings = after;
            self.redo.clear();
        } else {
            self.document.settings = after;
            let after_state = TreatmentState::from_document(&self.document);
            self.undo.push(Edit {
                before,
                after: after_state,
            });
            self.redo.clear();
        }
        true
    }

    pub fn set_render_variant(&mut self, render: RenderVariant) -> bool {
        if self.document.render == render {
            return false;
        }
        let before = TreatmentState::from_document(&self.document);
        if render_kind(&self.document.render) != render_kind(&render) {
            match &self.document.render {
                RenderVariant::WebShapeV1 { settings } => {
                    self.document.saved_web_shape = Some(settings.clone())
                }
                RenderVariant::WebCurveV1 { settings } => {
                    self.document.saved_web_curve = Some(settings.clone())
                }
                RenderVariant::NativeBasicV1 => {}
            }
        }
        self.document.render = render;
        if self.active.is_none() {
            let after = TreatmentState::from_document(&self.document);
            self.undo.push(Edit { before, after });
        }
        self.redo.clear();
        true
    }

    pub fn set_treatment(
        &mut self,
        render: RenderVariant,
        native_settings: Option<Settings>,
    ) -> bool {
        self.begin_edit(SettingKey::Treatment);
        if let Some(settings) = native_settings {
            self.set_settings(SettingKey::Treatment, settings);
        }
        self.set_render_variant(render);
        self.end_edit()
    }

    pub fn end_edit(&mut self) -> bool {
        let Some(active) = self.active.take() else {
            return false;
        };
        let after = TreatmentState::from_document(&self.document);
        if active.before == after {
            return false;
        }
        self.undo.push(Edit {
            before: active.before,
            after,
        });
        true
    }

    pub fn cancel_edit(&mut self) -> bool {
        let Some(active) = self.active.take() else {
            return false;
        };
        active.before.apply(&mut self.document);
        true
    }

    pub fn undo(&mut self) -> bool {
        self.end_edit();
        let Some(edit) = self.undo.pop() else {
            return false;
        };
        edit.before.apply(&mut self.document);
        self.redo.push(edit);
        true
    }

    pub fn redo(&mut self) -> bool {
        self.end_edit();
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        edit.after.apply(&mut self.document);
        self.undo.push(edit);
        true
    }
}

impl TreatmentState {
    fn from_document(document: &Document) -> Self {
        Self {
            settings: document.settings,
            render: document.render.clone(),
            saved_web_shape: document.saved_web_shape.clone(),
            saved_web_curve: document.saved_web_curve.clone(),
        }
    }

    fn apply(&self, document: &mut Document) {
        document.settings = self.settings;
        document.render = self.render.clone();
        document.saved_web_shape = self.saved_web_shape.clone();
        document.saved_web_curve = self.saved_web_curve.clone();
    }
}

fn render_kind(render: &RenderVariant) -> u8 {
    match render {
        RenderVariant::NativeBasicV1 => 0,
        RenderVariant::WebShapeV1 { .. } => 1,
        RenderVariant::WebCurveV1 { .. } => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> DocumentEditor {
        DocumentEditor::new(Document::new(SourceArtwork {
            name: "pixel.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from([1]),
        }))
    }

    #[test]
    fn undo_redo_and_dirty_state() {
        let mut editor = editor();
        let original = editor.document().settings;
        let mut changed = original;
        changed.coverage = 120.0;
        assert!(editor.set_settings(SettingKey::Coverage, changed));
        assert!(editor.is_dirty());
        assert!(editor.undo());
        assert_eq!(editor.document().settings, original);
        assert!(!editor.is_dirty());
        assert!(editor.redo());
        assert_eq!(editor.document().settings.coverage, 120.0);
    }

    #[test]
    fn paused_long_drag_remains_one_edit() {
        let mut editor = editor();
        editor.begin_edit(SettingKey::Detail);
        for detail in [54.0, 58.0, 62.0] {
            let mut settings = editor.document().settings;
            settings.detail = detail;
            editor.set_settings(SettingKey::Detail, settings);
        }
        std::thread::sleep(std::time::Duration::from_millis(700));
        let mut settings = editor.document().settings;
        settings.detail = 70.0;
        editor.set_settings(SettingKey::Detail, settings);
        assert!(editor.end_edit());
        assert!(editor.undo());
        assert_eq!(
            editor.document().settings.detail,
            Settings::default().detail
        );
        assert!(!editor.can_undo());
    }

    #[test]
    fn separate_quick_gestures_are_two_edits() {
        let mut editor = editor();
        for detail in [60.0, 70.0] {
            editor.begin_edit(SettingKey::Detail);
            let mut settings = editor.document().settings;
            settings.detail = detail;
            editor.set_settings(SettingKey::Detail, settings);
            assert!(editor.end_edit());
        }
        assert!(editor.undo());
        assert_eq!(editor.document().settings.detail, 60.0);
        assert!(editor.undo());
        assert_eq!(
            editor.document().settings.detail,
            Settings::default().detail
        );
    }

    #[test]
    fn web_shape_treatment_is_one_undoable_document_edit() {
        let mut editor = editor();
        let web = RenderVariant::WebShapeV1 {
            settings: Box::new(WebShapeSettings::default()),
        };
        assert!(editor.set_render_variant(web.clone()));
        assert_eq!(editor.document().render, web);
        assert!(editor.is_dirty());
        assert!(editor.undo());
        assert_eq!(editor.document().render, RenderVariant::NativeBasicV1);
        assert!(!editor.is_dirty());
        assert!(editor.redo());
        assert_eq!(editor.document().render, web);
    }

    #[test]
    fn web_deltas_flatten_to_effective_channel_values() {
        let mut settings = WebShapeSettings::default();
        settings.channels.c.rotation = 3.0;
        settings.channels.c.grid_pivot_x = -12.0;
        settings.channels.c.resolution_scale = 1.5;
        settings.channels.c.max_size = 80.0;
        settings.apply_deltas(WebShapeDeltas {
            rotation_delta: 7.0,
            grid_rotation_delta: -2.0,
            grid_pivot_x_delta: 5.0,
            grid_pivot_y_delta: 9.0,
            scale_multiplier: 0.5,
            resolution_multiplier: 2.0,
            threshold_delta: 0.1,
            max_size_multiplier: 3.0,
            opacity_multiplier: 0.8,
            offset_x_delta: 4.0,
            offset_y_delta: -6.0,
        });
        let cyan = &settings.channels.c;
        assert_eq!(cyan.rotation, 10.0);
        assert_eq!(cyan.grid_rotation, 13.0);
        assert_eq!(cyan.grid_pivot_x, -7.0);
        assert_eq!(cyan.grid_pivot_y, 9.0);
        assert_eq!(cyan.scale, 0.5);
        assert_eq!(cyan.resolution_scale, 3.0);
        assert_eq!(cyan.threshold, 0.1);
        assert_eq!(cyan.max_size, 240.0);
        assert_eq!(cyan.opacity, 0.8);
        assert_eq!(cyan.offset_x, 4.0);
        assert_eq!(cyan.offset_y, -6.0);
    }

    #[test]
    fn web_shape_document_rejects_out_of_range_values() {
        let mut document = Document::new(SourceArtwork {
            name: "pixel.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from([1]),
        });
        let mut settings = WebShapeSettings::default();
        settings.channels.c.opacity = 1.01;
        document.render = RenderVariant::WebShapeV1 {
            settings: Box::new(settings.clone()),
        };
        assert!(document.validate().is_err());

        settings.channels.c.opacity = 1.0;
        settings.channels.m.threshold = -0.01;
        document.render = RenderVariant::WebShapeV1 {
            settings: Box::new(settings.clone()),
        };
        assert!(document.validate().is_err());

        settings.channels.m.threshold = 0.0;
        settings.grid_scale = 0.0;
        document.render = RenderVariant::WebShapeV1 {
            settings: Box::new(settings),
        };
        assert!(document.validate().is_err());
    }

    #[test]
    fn switching_treatments_preserves_inactive_curve_and_is_undoable() {
        let mut editor = editor();
        let curve = WebCurveSettings {
            shared_path: CurvePath::deep_wave(),
            ..Default::default()
        };
        let curve_variant = RenderVariant::WebCurveV1 {
            settings: Box::new(curve.clone()),
        };
        assert!(editor.set_render_variant(curve_variant.clone()));
        assert!(editor.set_render_variant(RenderVariant::NativeBasicV1));
        assert_eq!(editor.document().saved_web_curve.as_deref(), Some(&curve));
        assert!(editor.undo());
        assert_eq!(editor.document().render, curve_variant);
        assert!(editor.redo());
        assert_eq!(editor.document().render, RenderVariant::NativeBasicV1);
        assert_eq!(editor.document().saved_web_curve.as_deref(), Some(&curve));
    }

    #[test]
    fn curve_drag_changes_coalesce_into_one_undo_step() {
        let original = WebCurveSettings::default();
        let mut document = Document::new(SourceArtwork {
            name: "pixel.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from([1]),
        });
        document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(original.clone()),
        };
        let mut editor = DocumentEditor::new(document);
        editor.begin_edit(SettingKey::CurvePath);
        for y in [-0.2, -0.3, -0.35] {
            let mut changed = original.clone();
            changed.shared_path.segments[0].control_1.y = y;
            editor.set_render_variant(RenderVariant::WebCurveV1 {
                settings: Box::new(changed),
            });
        }
        assert!(editor.end_edit());
        assert!(editor.undo());
        assert_eq!(
            editor.document().render,
            RenderVariant::WebCurveV1 {
                settings: Box::new(original)
            }
        );
        assert!(!editor.can_undo());
    }

    #[test]
    fn cancelled_canvas_drag_restores_state_without_undo() {
        let mut document = Document::new(SourceArtwork {
            name: "pixel.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from([1]),
        });
        let original = WebCurveSettings {
            layout: CurveLayout::MotifPattern,
            ..Default::default()
        };
        document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(original.clone()),
        };
        let mut editor = DocumentEditor::new(document);
        editor.begin_edit(SettingKey::CurvePositionX);
        let mut changed = original.clone();
        changed.channels.c.offset_x = 75.0;
        assert!(editor.set_render_variant(RenderVariant::WebCurveV1 {
            settings: Box::new(changed),
        }));
        assert!(editor.cancel_edit());
        assert_eq!(
            editor.document().render,
            RenderVariant::WebCurveV1 {
                settings: Box::new(original)
            }
        );
        assert!(!editor.can_undo());
        assert!(!editor.is_dirty());
    }

    #[test]
    fn native_preset_settings_and_renderer_apply_as_one_undo_edit() {
        let mut editor = editor();
        let original = editor.document().clone();
        let settings = Settings {
            treatment: Treatment::Lines,
            detail: 81.0,
            coverage: 119.0,
            contrast: 136.0,
            angle: -12.0,
        };
        assert!(editor.set_treatment(RenderVariant::NativeBasicV1, Some(settings)));
        assert_eq!(editor.document().settings, settings);
        assert!(editor.undo());
        assert_eq!(editor.document(), &original);
        assert!(!editor.can_undo());
    }
}
