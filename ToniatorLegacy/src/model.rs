use crate::artwork_pipeline::{
    ArtworkPipelineSettings, ArtworkSource, AutomaticSeparationStrategy, ChannelAssignment,
    LegacyBrightnessKind, LegacyCompatibilityAssignment, OutputChannelId, OutputModel,
};
use crate::pattern::{PATTERN_REGISTRY, PatternId, VersionedPatternParameters};
use crate::pattern_definition::{
    DefinitionParameterScope, LiteralValue, MAX_PATTERN_INSTANCE_OUTPUT_CHANNELS,
    MAX_PATTERN_INSTANCE_VALUES, MAX_TEXT_PARAMETER_BYTES, ParameterAuthoring, ParameterOwnership,
    PatternDefinition, PatternInstanceParameters, PatternInstanceValue,
    TNPATTERN_INSTANCE_FORMAT_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::Map;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DOCUMENT_FORMAT: &str = "toniator-document";
pub const DOCUMENT_VERSION: u32 = 10;

/// Determines how source colour is separated into output layers. CMYK is
/// subtractive ink; RGB is additive light on a transparent screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OutputMode {
    #[default]
    CmykInks,
    RgbScreen,
}

impl OutputMode {
    pub const fn inks(self) -> &'static [Ink] {
        match self {
            Self::CmykInks => &Ink::ALL,
            Self::RgbScreen => &Ink::RGB,
        }
    }
}

/// An sRGB color stored losslessly in a document. Alpha is straight (not
/// premultiplied), which keeps it suitable for SVG and PNG export alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl RgbaColor {
    pub const WHITE: Self = Self::opaque(255, 255, 255);

    pub const fn opaque(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 255,
        }
    }

    pub const fn is_opaque(self) -> bool {
        self.alpha == 255
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PreviewSurface {
    #[default]
    Checkerboard,
    Color {
        color: RgbaColor,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExportBackground {
    #[default]
    None,
    Color {
        color: RgbaColor,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentAppearance {
    pub preview_surface: PreviewSurface,
    pub export_background: ExportBackground,
}

impl Default for DocumentAppearance {
    fn default() -> Self {
        Self {
            // White is a surface rather than a paper object: new artwork
            // still exports with transparency unless the user opts in.
            preview_surface: PreviewSurface::Color {
                color: RgbaColor::WHITE,
            },
            export_background: ExportBackground::None,
        }
    }
}

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
    /// Direct source RGB component mapping. Only available in RGB Screen.
    Rgb,
    Luminance,
    CrosshatchLuminance,
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
    Red,
    Green,
    Blue,
}

impl Ink {
    pub const ALL: [Self; 4] = [Self::Cyan, Self::Magenta, Self::Yellow, Self::Black];
    pub const RGB: [Self; 3] = [Self::Red, Self::Green, Self::Blue];

    pub fn id(self) -> &'static str {
        match self {
            Self::Cyan => "c",
            Self::Magenta => "m",
            Self::Yellow => "y",
            Self::Black => "k",
            Self::Red => "r",
            Self::Green => "g",
            Self::Blue => "b",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cyan => "Cyan",
            Self::Magenta => "Magenta",
            Self::Yellow => "Yellow",
            Self::Black => "Black",
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Blue => "Blue",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WebShape {
    #[default]
    Circle,
    RegularPolygon,
    UserDefined,
    // Accepted when importing useful legacy web presets; not exposed for new work.
    Rectangle,
    Triangle,
    Pentagon,
    Hexagon,
}

/// Per-channel placement strategy for Shapes and project-embedded point-grid
/// recipes. Structural grid geometry remains pattern-definition state; these
/// values belong to the channel treatment controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum WebShapePointSampler {
    #[default]
    Grid,
    Uniform,
    Weighted,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapePoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapeAnchor {
    pub point: ShapePoint,
    pub incoming: ShapePoint,
    pub outgoing: ShapePoint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedShapePath {
    pub anchors: Vec<ShapeAnchor>,
}

impl ClosedShapePath {
    /// Converts a legacy polygon without changing its visible geometry. Each
    /// straight cubic edge uses controls at one-third and two-thirds.
    pub fn from_polygon(nodes: &[ShapePoint]) -> Self {
        let anchors = nodes
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let previous = nodes[(index + nodes.len() - 1) % nodes.len()];
                let next = nodes[(index + 1) % nodes.len()];
                ShapeAnchor {
                    point: *point,
                    incoming: shape_lerp(*point, previous, 1.0 / 3.0),
                    outgoing: shape_lerp(*point, next, 1.0 / 3.0),
                }
            })
            .collect();
        Self { anchors }
    }
}

fn shape_lerp(a: ShapePoint, b: ShapePoint, amount: f64) -> ShapePoint {
    ShapePoint {
        x: a.x + (b.x - a.x) * amount,
        y: a.y + (b.y - a.y) * amount,
    }
}

pub fn default_shape_nodes() -> Vec<ShapePoint> {
    vec![
        ShapePoint { x: -0.45, y: -0.45 },
        ShapePoint { x: 0.45, y: -0.45 },
        ShapePoint { x: 0.45, y: 0.45 },
        ShapePoint { x: -0.45, y: 0.45 },
    ]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
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
    #[serde(default = "one")]
    pub width_scale: f64,
    #[serde(default = "one")]
    pub height_scale: f64,
    pub threshold: f64,
    /// Percentage multiplier applied after the global maximum mark size.
    pub max_size: f64,
    pub resolution_scale: f64,
    /// Periodic lattice phase. Values are wrapped to one signed cell.
    pub offset_x: f64,
    pub offset_y: f64,
    pub opacity: f64,
    pub shape: WebShape,
    /// Number of sides used when this ink has an independent regular polygon.
    #[serde(default = "default_polygon_sides")]
    pub polygon_sides: u8,
    /// Independent custom geometry for this ink. When absent, the shared path
    /// is used as a backward-compatible fallback.
    #[serde(default)]
    pub custom_shape_path: Option<ClosedShapePath>,
    /// Per-channel site constructor selected in the main Channel Settings
    /// panel. Grid uses the pattern lattice; Uniform and Weighted use the
    /// neutral deterministic site-distribution service.
    #[serde(default)]
    pub point_sampler: WebShapePointSampler,
    /// Per-channel deterministic seed for random sites and jitter.
    #[serde(default)]
    pub random_seed: u64,
    /// Influence exponent for source-weighted site placement.
    #[serde(default = "one")]
    pub weight_influence: f64,
    /// Blend between uniform mark size (0) and source-responsive size (1).
    /// This is a channel control because each ink can use a different volume
    /// response while sharing the same placement recipe.
    #[serde(default = "one")]
    pub random_size_response: f64,
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
            width_scale: 1.0,
            height_scale: 1.0,
            threshold: 0.0,
            max_size: 100.0,
            resolution_scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            opacity: 1.0,
            shape: WebShape::Circle,
            polygon_sides: default_polygon_sides(),
            custom_shape_path: None,
            point_sampler: WebShapePointSampler::Grid,
            random_seed: 0,
            weight_influence: 1.0,
            random_size_response: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebShapeChannels {
    pub c: WebShapeChannel,
    pub m: WebShapeChannel,
    pub y: WebShapeChannel,
    pub k: WebShapeChannel,
    #[serde(default)]
    pub r: WebShapeChannel,
    #[serde(default)]
    pub g: WebShapeChannel,
    #[serde(default)]
    pub b: WebShapeChannel,
}

impl WebShapeChannels {
    pub fn get(&self, ink: Ink) -> &WebShapeChannel {
        match ink {
            Ink::Cyan => &self.c,
            Ink::Magenta => &self.m,
            Ink::Yellow => &self.y,
            Ink::Black => &self.k,
            Ink::Red => &self.r,
            Ink::Green => &self.g,
            Ink::Blue => &self.b,
        }
    }

    pub fn get_mut(&mut self, ink: Ink) -> &mut WebShapeChannel {
        match ink {
            Ink::Cyan => &mut self.c,
            Ink::Magenta => &mut self.m,
            Ink::Yellow => &mut self.y,
            Ink::Black => &mut self.k,
            Ink::Red => &mut self.r,
            Ink::Green => &mut self.g,
            Ink::Blue => &mut self.b,
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
            r: channel("#ff0000", 0.0),
            g: channel("#00ff00", 0.0),
            b: channel("#0000ff", 0.0),
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
    /// The single output color used by progressive crosshatching.
    #[serde(default = "default_crosshatch_color")]
    pub crosshatch_color: String,
    pub use_shared_mark: bool,
    pub shared_shape: WebShape,
    #[serde(default = "default_polygon_sides")]
    pub polygon_sides: u8,
    #[serde(default = "default_shape_nodes")]
    pub custom_nodes: Vec<ShapePoint>,
    /// Canonical editable cubic path. Older documents omit this and resolve
    /// through `custom_nodes`, preserving their polygon exactly.
    #[serde(default)]
    pub custom_shape_path: Option<ClosedShapePath>,
    /// Creative-facing numeric base shown by the All Inks inspector target.
    /// Renderers continue to consume `channels`, whose values include per-ink deltas.
    pub base_channel: WebShapeChannel,
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
            crosshatch_color: default_crosshatch_color(),
            use_shared_mark: true,
            shared_shape: WebShape::Circle,
            polygon_sides: 4,
            custom_nodes: default_shape_nodes(),
            custom_shape_path: None,
            base_channel: WebShapeChannel::default(),
            channels: WebShapeChannels::default(),
        }
    }
}

fn one() -> f64 {
    1.0
}

fn default_polygon_sides() -> u8 {
    4
}

fn default_crosshatch_color() -> String {
    "#111111".into()
}

impl WebShapeSettings {
    pub fn resolved_custom_shape_path(&self) -> ClosedShapePath {
        self.custom_shape_path
            .clone()
            .unwrap_or_else(|| ClosedShapePath::from_polygon(&self.custom_nodes))
    }

    pub fn resolved_channel_shape_path(&self, channel: &WebShapeChannel) -> ClosedShapePath {
        channel
            .custom_shape_path
            .clone()
            .unwrap_or_else(|| self.resolved_custom_shape_path())
    }
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
#[serde(deny_unknown_fields)]
pub struct CurvePoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CubicCurveSegment {
    pub control_1: CurvePoint,
    pub control_2: CurvePoint,
    pub end: CurvePoint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
                    control_1: CurvePoint { x: -0.32, y: -0.12 },
                    control_2: CurvePoint { x: -0.18, y: -0.12 },
                    end: CurvePoint { x: 0.0, y: 0.0 },
                },
                CubicCurveSegment {
                    control_1: CurvePoint { x: 0.18, y: 0.12 },
                    control_2: CurvePoint { x: 0.32, y: 0.12 },
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
#[serde(default, deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct WebCurveChannels {
    pub c: WebCurveChannel,
    pub m: WebCurveChannel,
    pub y: WebCurveChannel,
    pub k: WebCurveChannel,
    #[serde(default)]
    pub r: WebCurveChannel,
    #[serde(default)]
    pub g: WebCurveChannel,
    #[serde(default)]
    pub b: WebCurveChannel,
}

impl WebCurveChannels {
    pub fn get(&self, ink: Ink) -> &WebCurveChannel {
        match ink {
            Ink::Cyan => &self.c,
            Ink::Magenta => &self.m,
            Ink::Yellow => &self.y,
            Ink::Black => &self.k,
            Ink::Red => &self.r,
            Ink::Green => &self.g,
            Ink::Blue => &self.b,
        }
    }

    pub fn get_mut(&mut self, ink: Ink) -> &mut WebCurveChannel {
        match ink {
            Ink::Cyan => &mut self.c,
            Ink::Magenta => &mut self.m,
            Ink::Yellow => &mut self.y,
            Ink::Black => &mut self.k,
            Ink::Red => &mut self.r,
            Ink::Green => &mut self.g,
            Ink::Blue => &mut self.b,
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
            r: channel("#ff0000", 0.0),
            g: channel("#00ff00", 0.0),
            b: channel("#0000ff", 0.0),
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
    #[serde(default = "default_crosshatch_color")]
    pub crosshatch_color: String,
    pub layout: CurveLayout,
    pub use_shared_curve: bool,
    pub shared_path: CurvePath,
    pub shared_close_ends: bool,
    pub shared_smooth_join: bool,
    pub show_background: bool,
    /// Creative-facing numeric base shown by the All Inks inspector target.
    /// Effective per-ink values remain in `channels` for rendering and export.
    pub base_channel: WebCurveChannel,
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
            crosshatch_color: default_crosshatch_color(),
            layout: CurveLayout::FullWidth,
            use_shared_curve: true,
            shared_path: CurvePath::soft_wave(),
            shared_close_ends: false,
            shared_smooth_join: false,
            show_background: true,
            base_channel: WebCurveChannel::default(),
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

    /// Establishes Toniator's genuine progressive crosshatch treatment: four
    /// straight monochrome curve layers, ordered K, C, M, Y in the UI, with
    /// independently editable crossing angles.
    pub fn configure_crosshatch(&mut self) {
        self.value_mode = ValueMode::CrosshatchLuminance;
        self.layout = CurveLayout::FullWidth;
        self.use_shared_curve = true;
        self.shared_path = CurvePath::straight();
        self.shared_close_ends = false;
        self.shared_smooth_join = false;
        for (ink, angle) in [
            (Ink::Black, 45.0),
            (Ink::Cyan, -45.0),
            (Ink::Magenta, 0.0),
            (Ink::Yellow, 90.0),
        ] {
            let channel = self.channels.get_mut(ink);
            channel.enabled = true;
            channel.grid_rotation = angle;
            channel.path = CurvePath::straight();
            channel.close_ends = false;
            channel.smooth_join = false;
        }
    }

    pub fn crosshatch_from_shape(shape: &WebShapeSettings) -> Self {
        let mut curve = Self {
            output_width: shape.output_width,
            output_height: shape.output_height,
            long_edge_cells: shape.long_edge_cells,
            min_mark: shape.min_mark,
            max_mark: shape.max_mark,
            value_mode: ValueMode::CrosshatchLuminance,
            single_channel: shape.single_channel,
            crosshatch_color: shape.crosshatch_color.clone(),
            ..Self::default()
        };
        for ink in Ink::ALL {
            let source = shape.channels.get(ink);
            let target = curve.channels.get_mut(ink);
            target.color.clone_from(&source.color);
            target.scale = source.scale;
            target.threshold = source.threshold;
            target.max_size = source.max_size;
            target.resolution_scale = source.resolution_scale;
            target.offset_x = source.offset_x;
            target.offset_y = source.offset_y;
            target.opacity = source.opacity;
        }
        curve.configure_crosshatch();
        curve
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
    /// Derived dispatch only; persisted Weighted Voronoi data lives solely in
    /// `PatternDocumentState`.
    WeightedVoronoiCanonicalV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum WeightedVoronoiArrangementPolicy {
    #[default]
    Shared,
    Independent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum WeightedVoronoiPlacementMode {
    #[default]
    SourceWeighted,
    Uniform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum WeightedVoronoiDensityPolarity {
    #[default]
    DarkerMoreDense,
    LighterMoreDense,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedVoronoiChannelSettings {
    pub enabled: bool,
    pub arrangement: WeightedVoronoiArrangementPolicy,
    pub cell_count: u32,
    pub seed: u64,
    pub boundary_gap: f64,
    pub placement: WeightedVoronoiPlacementMode,
    pub density_polarity: WeightedVoronoiDensityPolarity,
    pub density_strength: f64,
    pub response_strength: f64,
    pub minimum_cell_scale: f64,
}

impl Default for WeightedVoronoiChannelSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            arrangement: WeightedVoronoiArrangementPolicy::Shared,
            cell_count: 256,
            seed: 0,
            boundary_gap: 1.0,
            placement: WeightedVoronoiPlacementMode::SourceWeighted,
            density_polarity: WeightedVoronoiDensityPolarity::DarkerMoreDense,
            density_strength: 1.0,
            response_strength: 1.0,
            minimum_cell_scale: 0.0,
        }
    }
}

impl WeightedVoronoiChannelSettings {
    fn validate(&self, channel: OutputChannelId) -> anyhow::Result<()> {
        let limits = crate::site_distribution::DistributionLimits::default();
        anyhow::ensure!(
            (2..=limits.max_sites).contains(&(self.cell_count as usize)),
            "Weighted Voronoi cell count for {} must be between 2 and {}",
            channel.stable_id(),
            limits.max_sites
        );
        for (label, value, range) in [
            ("boundary gap", self.boundary_gap, 0.0..=64.0),
            ("density strength", self.density_strength, 0.001..=16.0),
            ("response strength", self.response_strength, 0.0..=16.0),
            ("minimum cell scale", self.minimum_cell_scale, 0.0..=1.0),
        ] {
            anyhow::ensure!(
                value.is_finite() && range.contains(&value),
                "Weighted Voronoi {label} for {} is outside its supported range",
                channel.stable_id()
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedVoronoiSettings {
    pub channels: BTreeMap<String, WeightedVoronoiChannelSettings>,
}

impl Default for WeightedVoronoiSettings {
    fn default() -> Self {
        Self {
            channels: weighted_voronoi_channels()
                .map(|channel| {
                    (
                        channel.stable_id().to_owned(),
                        WeightedVoronoiChannelSettings::default(),
                    )
                })
                .collect(),
        }
    }
}

impl WeightedVoronoiSettings {
    pub fn validate(&self) -> anyhow::Result<()> {
        let expected: Vec<_> = weighted_voronoi_channels().collect();
        anyhow::ensure!(
            self.channels.len() == expected.len()
                && expected
                    .iter()
                    .all(|channel| self.channels.contains_key(channel.stable_id())),
            "Weighted Voronoi settings must retain every semantic channel"
        );
        for channel in expected {
            self.channel_settings(channel)?.validate(channel)?;
        }
        Ok(())
    }

    pub fn channel_settings(
        &self,
        channel: OutputChannelId,
    ) -> anyhow::Result<&WeightedVoronoiChannelSettings> {
        self.channels.get(channel.stable_id()).ok_or_else(|| {
            anyhow::anyhow!(
                "Weighted Voronoi settings are missing semantic channel {}",
                channel.stable_id()
            )
        })
    }

    pub fn channel_settings_mut(
        &mut self,
        channel: OutputChannelId,
    ) -> anyhow::Result<&mut WeightedVoronoiChannelSettings> {
        self.channels.get_mut(channel.stable_id()).ok_or_else(|| {
            anyhow::anyhow!(
                "Weighted Voronoi settings are missing semantic channel {}",
                channel.stable_id()
            )
        })
    }
}

fn weighted_voronoi_channels() -> impl Iterator<Item = OutputChannelId> {
    OutputChannelId::CMYK
        .into_iter()
        .chain(OutputChannelId::RGB)
}

/// The only persisted pattern authority. The selected pattern and each
/// built-in compatibility pattern's typed settings live here exactly once;
/// embedded custom Shapes recipes are project-owned and `RenderVariant`
/// remains a derived compatibility adapter only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatternSelection {
    NativeBasicV1,
    Registered(PatternId),
}

/// A portable project-owned definition and its current value-only instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddedPatternDefinition {
    pub definition: PatternDefinition,
    pub instance: PatternInstanceParameters,
}

/// The one narrowly recoverable validation failure for a current document.
///
/// This is deliberately internal to the document/persistence boundary: a
/// document carrying this state is not valid to render, edit, or save.  The
/// retained instance exists only so recovery can validate it against an exact
/// replacement definition before a complete candidate is installed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MissingSelectedPatternDefinition {
    pub requested_id: PatternId,
    pub retained_instance: Option<PatternInstanceParameters>,
}

/// Validates instance shape without a missing definition.  This intentionally
/// stops before definition-dependent parameter names, types, ranges, assets,
/// required-value completeness, and output-channel applicability; those are
/// checked later against the exact recovered definition.  It does reject data
/// that cannot safely be retained or discarded as a valid current instance.
fn validate_retained_instance_structure(
    instance: &PatternInstanceParameters,
    requested_id: &PatternId,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        instance.format_version == TNPATTERN_INSTANCE_FORMAT_VERSION,
        "retained instance for {requested_id} has an unsupported format version"
    );
    anyhow::ensure!(
        instance.pattern_id == *requested_id,
        "retained instance key {requested_id} contradicts instance {}",
        instance.pattern_id
    );
    anyhow::ensure!(
        instance.output_channel_values.len() <= MAX_PATTERN_INSTANCE_OUTPUT_CHANNELS,
        "retained instance for {requested_id} exceeds output-channel resource limits"
    );
    let total_values = instance
        .output_channel_values
        .iter()
        .try_fold(instance.pattern_values.len(), |total, channel| {
            total.checked_add(channel.values.len())
        })
        .ok_or_else(|| {
            anyhow::anyhow!("retained instance for {requested_id} exceeds value limits")
        })?;
    anyhow::ensure!(
        total_values <= MAX_PATTERN_INSTANCE_VALUES,
        "retained instance for {requested_id} exceeds value resource limits"
    );

    validate_retained_instance_values(&instance.pattern_values, "pattern")?;
    let mut channels = HashSet::new();
    for channel in &instance.output_channel_values {
        anyhow::ensure!(
            channel.channel.parse::<OutputChannelId>().is_ok(),
            "retained instance for {requested_id} references unknown output channel `{}`",
            channel.channel
        );
        anyhow::ensure!(
            channels.insert(channel.channel.as_str()),
            "retained instance for {requested_id} has duplicate output channel `{}`",
            channel.channel
        );
        validate_retained_instance_values(
            &channel.values,
            &format!("output channel `{}`", channel.channel),
        )?;
    }
    Ok(())
}

fn validate_retained_instance_values(
    values: &[PatternInstanceValue],
    location: &str,
) -> anyhow::Result<()> {
    let mut keys = HashSet::new();
    for entry in values {
        anyhow::ensure!(
            is_retained_stable_name(&entry.key),
            "retained instance has malformed parameter key `{}` for {location}",
            entry.key
        );
        anyhow::ensure!(
            keys.insert(entry.key.as_str()),
            "retained instance has duplicate parameter key `{}` for {location}",
            entry.key
        );
        anyhow::ensure!(
            retained_literal_is_well_formed(&entry.value),
            "retained instance has a malformed literal for parameter `{}` in {location}",
            entry.key
        );
    }
    Ok(())
}

fn is_retained_stable_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn retained_literal_is_well_formed(value: &LiteralValue) -> bool {
    match value {
        LiteralValue::Number(value) => value.is_finite(),
        LiteralValue::Text(value) => value.len() <= MAX_TEXT_PARAMETER_BYTES,
        LiteralValue::Choice(value) => !value.is_empty() && value.len() <= 256,
        LiteralValue::SvgAsset(value) => {
            value.len() == 71
                && value.starts_with("sha256:")
                && value[7..]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }
        LiteralValue::Integer(_) | LiteralValue::Boolean(_) => true,
    }
}

/// A bundled definition paired with the current value-only instance resolved
/// from persisted stable-ID document authority.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedBundledPatternDefinition {
    pub definition: PatternDefinition,
    pub instance: PatternInstanceParameters,
}

/// A selected declarative definition and its mutable project-local instance.
/// This is the shared read boundary for generic Channel Settings controls;
/// it deliberately has no renderer, family, display-name, or adapter input.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSelectedPatternDefinition {
    pub definition: PatternDefinition,
    pub instance: PatternInstanceParameters,
}

fn update_output_channel_parameter(
    definition: &PatternDefinition,
    instance: &mut PatternInstanceParameters,
    channel: OutputChannelId,
    key: &str,
    value: LiteralValue,
) -> anyhow::Result<()> {
    let parameter = definition
        .parameters
        .iter()
        .find(|parameter| parameter.key == key)
        .ok_or_else(|| anyhow::anyhow!("unknown output-channel parameter `{key}`"))?;
    anyhow::ensure!(
        parameter.scope == DefinitionParameterScope::OutputChannel,
        "parameter `{key}` is not output-channel scoped"
    );
    let ParameterAuthoring::Creator(metadata) = &parameter.authoring else {
        anyhow::bail!("parameter `{key}` is internal");
    };
    anyhow::ensure!(
        metadata.ownership == ParameterOwnership::ChannelInstance,
        "parameter `{key}` is not channel-instance owned"
    );
    let values = instance
        .output_channel_values
        .iter_mut()
        .find(|values| values.channel == channel.stable_id())
        .ok_or_else(|| anyhow::anyhow!("instance has no values for {}", channel.stable_id()))?;
    let entry = values
        .values
        .iter_mut()
        .find(|entry| entry.key == key)
        .ok_or_else(|| anyhow::anyhow!("instance is missing output-channel parameter `{key}`"))?;
    entry.value = value;
    definition
        .validate_instance_parameters(instance)
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternDocumentState {
    pub selected: PatternSelection,
    pub instances: BTreeMap<PatternId, VersionedPatternParameters>,
    /// Current strict instances for immutable bundled declarative definitions.
    /// This is distinct from compatibility projections and from project-owned
    /// embedded definitions; a format-v10 document must carry this field.
    pub bundled_definition_instances: BTreeMap<PatternId, PatternInstanceParameters>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub embedded_patterns: BTreeMap<PatternId, EmbeddedPatternDefinition>,
}

impl PatternDocumentState {
    fn new() -> Self {
        let mut instances = BTreeMap::new();
        instances.insert(
            PatternId::COMPATIBILITY_SHAPES_V1,
            pattern_parameters_from_settings(
                PatternId::COMPATIBILITY_SHAPES_V1,
                &WebShapeSettings::default(),
            ),
        );
        instances.insert(
            PatternId::COMPATIBILITY_CURVES_V1,
            pattern_parameters_from_settings(
                PatternId::COMPATIBILITY_CURVES_V1,
                &WebCurveSettings::default(),
            ),
        );
        Self {
            selected: PatternSelection::Registered(PatternId::COMPATIBILITY_SHAPES_V1),
            instances,
            bundled_definition_instances: BTreeMap::new(),
            embedded_patterns: BTreeMap::new(),
        }
    }

    /// Returns the registered selection from persisted authority, never from
    /// the transient `RenderVariant` execution adapter.
    pub fn selected_pattern_id(&self) -> Option<PatternId> {
        match &self.selected {
            PatternSelection::NativeBasicV1 => None,
            PatternSelection::Registered(id) => Some(id.clone()),
        }
    }

    /// Returns metadata for the persisted selected pattern, if it is
    /// registered. Native Basic deliberately has no registry entry.
    pub fn selected_metadata(&self) -> Option<&'static crate::pattern::PatternMetadata> {
        PATTERN_REGISTRY.get(self.selected_pattern_id()?)
    }

    /// Returns the persisted parameter record for the selected registered
    /// pattern. The record is intentionally read-only; edits remain routed
    /// through `DocumentEditor`.
    pub fn selected_parameters(&self) -> Option<&VersionedPatternParameters> {
        self.instances.get(&self.selected_pattern_id()?)
    }

    /// Returns the selected project-embedded custom recipe, if any.
    pub fn selected_embedded_pattern(&self) -> Option<&EmbeddedPatternDefinition> {
        self.embedded_patterns.get(&self.selected_pattern_id()?)
    }

    /// Resolves the selected immutable bundled definition and its persisted
    /// current instance by stable ID. Neither labels nor `RenderVariant`
    /// participate in this semantic lookup.
    pub fn resolve_selected_bundled_pattern(
        &self,
    ) -> anyhow::Result<Option<ResolvedBundledPatternDefinition>> {
        let Some(id) = self
            .selected_pattern_id()
            .filter(|id| self.bundled_definition_instances.contains_key(id))
        else {
            return Ok(None);
        };
        let registry = crate::load_bundled_pattern_definition_registry()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let resolved = registry
            .get(&id)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let instance = self
            .bundled_definition_instances
            .get(&id)
            .expect("selection filter guarantees a bundled instance")
            .clone();
        resolved
            .definition
            .validate_instance_parameters(&instance)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(Some(ResolvedBundledPatternDefinition {
            definition: resolved.definition.clone(),
            instance,
        }))
    }

    /// Resolves either selected immutable bundled content or a selected
    /// project-embedded copy through one stable-ID authority boundary.
    pub fn resolve_selected_definition(
        &self,
    ) -> anyhow::Result<Option<ResolvedSelectedPatternDefinition>> {
        if let Some(embedded) = self.selected_embedded_pattern() {
            embedded
                .definition
                .validate_instance_parameters(&embedded.instance)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            return Ok(Some(ResolvedSelectedPatternDefinition {
                definition: embedded.definition.clone(),
                instance: embedded.instance.clone(),
            }));
        }
        Ok(self.resolve_selected_bundled_pattern()?.map(|resolved| {
            ResolvedSelectedPatternDefinition {
                definition: resolved.definition,
                instance: resolved.instance,
            }
        }))
    }

    /// Updates one selected creator-owned output-channel value transactionally.
    /// Compatibility adapters never participate: the exact selected instance
    /// is the only authority, whether bundled or project embedded.
    pub(crate) fn set_selected_output_channel_parameter(
        &mut self,
        channel: OutputChannelId,
        key: &str,
        value: LiteralValue,
    ) -> anyhow::Result<()> {
        let id = self
            .selected_pattern_id()
            .ok_or_else(|| anyhow::anyhow!("native selection has no declarative channel values"))?;
        let mut next = self.clone();
        if let Some(embedded) = next.embedded_patterns.get_mut(&id) {
            update_output_channel_parameter(
                &embedded.definition,
                &mut embedded.instance,
                channel,
                key,
                value,
            )?;
        } else {
            let registry = crate::load_bundled_pattern_definition_registry()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let definition = registry
                .get(&id)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?
                .definition
                .clone();
            let instance = next
                .bundled_definition_instances
                .get_mut(&id)
                .ok_or_else(|| anyhow::anyhow!("selected pattern has no declarative instance"))?;
            update_output_channel_parameter(&definition, instance, channel, key, value)?;
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        for id in [
            PatternId::COMPATIBILITY_SHAPES_V1,
            PatternId::COMPATIBILITY_CURVES_V1,
        ] {
            let record = self
                .instances
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("missing authoritative pattern state for {id}"))?;
            anyhow::ensure!(
                record.pattern_id == id,
                "authoritative pattern state key {id} contradicts record {}",
                record.pattern_id
            );
            record.validate().map_err(anyhow::Error::new)?;
            if id == PatternId::COMPATIBILITY_SHAPES_V1 {
                let _: WebShapeSettings = pattern_settings_from_parameters(record)?;
            } else if id == PatternId::COMPATIBILITY_CURVES_V1 {
                let _: WebCurveSettings = pattern_settings_from_parameters(record)?;
            } else {
                unreachable!("core instances exclude optional Weighted Voronoi state");
            }
        }
        anyhow::ensure!(
            self.instances.len() == 2
                || (self.instances.len() == 3
                    && self.instances.contains_key(&PatternId::WEIGHTED_VORONOI_V1)),
            "authoritative pattern state contains an unsupported pattern instance"
        );
        if let Some(record) = self.instances.get(&PatternId::WEIGHTED_VORONOI_V1) {
            anyhow::ensure!(
                record.pattern_id == PatternId::WEIGHTED_VORONOI_V1,
                "authoritative Weighted Voronoi state has a contradictory record"
            );
            record.validate().map_err(anyhow::Error::new)?;
            weighted_voronoi_settings_from_parameters(record)?.validate()?;
        }
        let bundled_registry = crate::load_bundled_pattern_definition_registry()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        for (id, instance) in &self.bundled_definition_instances {
            anyhow::ensure!(
                !self.instances.contains_key(id) && !self.embedded_patterns.contains_key(id),
                "bundled definition instance {id} overlaps another pattern authority"
            );
            let resolved = bundled_registry
                .get(id)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            anyhow::ensure!(
                instance.pattern_id == *id,
                "bundled definition instance key {id} contradicts instance {}",
                instance.pattern_id
            );
            resolved
                .definition
                .validate_instance_parameters(instance)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        }
        for (id, embedded) in &self.embedded_patterns {
            anyhow::ensure!(
                PATTERN_REGISTRY.get(id.clone()).is_none(),
                "project-embedded pattern {id} conflicts with a built-in pattern"
            );
            anyhow::ensure!(
                bundled_registry.get(id).is_err(),
                "project-embedded pattern {id} conflicts with an immutable bundled definition"
            );
            anyhow::ensure!(
                embedded.definition.id == *id,
                "project-embedded pattern key {id} contradicts definition {}",
                embedded.definition.id
            );
            anyhow::ensure!(
                embedded.instance.pattern_id == *id,
                "project-embedded pattern {id} contradicts instance {}",
                embedded.instance.pattern_id
            );
            embedded
                .definition
                .validate()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            embedded
                .definition
                .validate_instance_parameters(&embedded.instance)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        }
        if let Some(id) = self.selected_pattern_id() {
            anyhow::ensure!(
                self.instances.contains_key(&id)
                    || self.bundled_definition_instances.contains_key(&id)
                    || self.embedded_patterns.contains_key(&id),
                "selected pattern {id} has no authoritative parameter state or embedded definition"
            );
        }
        Ok(())
    }

    /// Classifies a selected definition as missing only after all unrelated
    /// pattern authority remains valid.  In particular, an invalid bundled
    /// instance, malformed embedded definition, or unsupported extra state is
    /// never converted into a recovery prompt.
    pub(crate) fn missing_selected_definition(
        &self,
    ) -> anyhow::Result<MissingSelectedPatternDefinition> {
        let PatternSelection::Registered(requested_id) = &self.selected else {
            anyhow::bail!("the selected pattern is not a registered definition");
        };
        anyhow::ensure!(
            !self.instances.contains_key(requested_id)
                && !self.embedded_patterns.contains_key(requested_id),
            "selected pattern {requested_id} already has authoritative state"
        );

        let retained_instance = self.bundled_definition_instances.get(requested_id).cloned();
        if let Some(instance) = &retained_instance {
            validate_retained_instance_structure(instance, requested_id)?;
        }
        if retained_instance.is_some()
            && crate::load_bundled_pattern_definition_registry()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?
                .get(requested_id)
                .is_ok()
        {
            anyhow::bail!("selected bundled definition {requested_id} is available");
        }

        // Validate the rest of the persisted pattern authority after removing
        // precisely the missing selected entry.  This proves the failure is
        // not an excuse to accept any unrelated invalid state.
        let mut remaining = self.clone();
        remaining.selected = PatternSelection::NativeBasicV1;
        remaining.bundled_definition_instances.remove(requested_id);
        remaining.validate()?;

        Ok(MissingSelectedPatternDefinition {
            requested_id: requested_id.clone(),
            retained_instance,
        })
    }

    /// Replaces the orphan selected authority with an exact external
    /// definition.  The caller supplies either the retained values or a fresh
    /// validated default; no partial state is exposed on failure.
    pub(crate) fn recover_missing_selected_definition(
        &mut self,
        missing: &MissingSelectedPatternDefinition,
        definition: PatternDefinition,
        instance: PatternInstanceParameters,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            definition.id == missing.requested_id,
            "recovery definition {} does not match requested pattern {}",
            definition.id,
            missing.requested_id
        );
        self.missing_selected_definition()?;
        let mut recovered = self.clone();
        recovered
            .bundled_definition_instances
            .remove(&missing.requested_id);
        recovered.install_and_select_embedded_pattern(definition, instance)?;
        *self = recovered;
        Ok(())
    }

    /// Applies an explicitly chosen replacement with fresh defaults.  The
    /// old selected orphan is removed, while every unrelated authority record
    /// is preserved.  Bundled immutable definitions retain their established
    /// ID-plus-instance persistence boundary; other candidates embed complete
    /// portable definition authority into the recovered document.
    pub(crate) fn replace_missing_selected_definition(
        &mut self,
        missing: &MissingSelectedPatternDefinition,
        definition: PatternDefinition,
        source: crate::pattern_definition_registry::PatternDefinitionSource,
    ) -> anyhow::Result<()> {
        self.missing_selected_definition()?;
        definition
            .validate()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let instance = definition
            .default_instance_parameters(
                OutputChannelId::CMYK
                    .into_iter()
                    .chain(OutputChannelId::RGB),
            )
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        self.bundled_definition_instances
            .remove(&missing.requested_id);
        match source {
            crate::pattern_definition_registry::PatternDefinitionSource::Bundled => {
                anyhow::ensure!(
                    PATTERN_REGISTRY.get(definition.id.clone()).is_none(),
                    "replacement {} conflicts with a built-in pattern",
                    definition.id
                );
                self.embedded_patterns.remove(&definition.id);
                self.bundled_definition_instances
                    .insert(definition.id.clone(), instance);
                self.selected = PatternSelection::Registered(definition.id);
                self.validate()
            }
            crate::pattern_definition_registry::PatternDefinitionSource::UserLibrary
            | crate::pattern_definition_registry::PatternDefinitionSource::ProjectEmbedded => {
                self.embedded_patterns.remove(&definition.id);
                self.install_and_select_embedded_pattern(definition, instance)
            }
        }
    }

    pub(crate) fn adapter(&self) -> anyhow::Result<RenderVariant> {
        self.validate()?;
        match &self.selected {
            PatternSelection::NativeBasicV1 => Ok(RenderVariant::NativeBasicV1),
            PatternSelection::Registered(id) if *id == PatternId::COMPATIBILITY_SHAPES_V1 => {
                Ok(RenderVariant::WebShapeV1 {
                    settings: Box::new(pattern_settings_from_parameters(
                        self.instances
                            .get(&PatternId::COMPATIBILITY_SHAPES_V1)
                            .expect("validated Shapes state"),
                    )?),
                })
            }
            PatternSelection::Registered(id) if *id == PatternId::COMPATIBILITY_CURVES_V1 => {
                Ok(RenderVariant::WebCurveV1 {
                    settings: Box::new(pattern_settings_from_parameters(
                        self.instances
                            .get(&PatternId::COMPATIBILITY_CURVES_V1)
                            .expect("validated Curves state"),
                    )?),
                })
            }
            PatternSelection::Registered(id) if *id == PatternId::WEIGHTED_VORONOI_V1 => {
                Ok(RenderVariant::WeightedVoronoiCanonicalV1)
            }
            PatternSelection::Registered(id)
                if self.bundled_definition_instances.contains_key(id) =>
            {
                // The compatibility facade is deliberately non-semantic here.
                // Rendering resolves the bundled definition from persisted ID
                // and instance authority before consulting this derived value.
                Ok(RenderVariant::WebShapeV1 {
                    settings: Box::new(pattern_settings_from_parameters(
                        self.instances
                            .get(&PatternId::COMPATIBILITY_SHAPES_V1)
                            .expect("validated Shapes state"),
                    )?),
                })
            }
            PatternSelection::Registered(id) if self.embedded_patterns.contains_key(id) => {
                // Custom recipes dispatch before this compatibility facade.
                // Preserve a derived Shapes adapter for existing cache and
                // transition seams that still require a RenderVariant.
                Ok(RenderVariant::WebShapeV1 {
                    settings: Box::new(pattern_settings_from_parameters(
                        self.instances
                            .get(&PatternId::COMPATIBILITY_SHAPES_V1)
                            .expect("validated Shapes state"),
                    )?),
                })
            }
            PatternSelection::Registered(id) => anyhow::bail!("unregistered pattern {id}"),
        }
    }

    /// Selects a registered pattern explicitly. The transient renderer adapter
    /// is deliberately absent from this API: it cannot choose state.
    pub(crate) fn select_pattern(&mut self, pattern_id: PatternId) -> anyhow::Result<()> {
        if pattern_id == PatternId::WEIGHTED_VORONOI_V1 && !self.instances.contains_key(&pattern_id)
        {
            self.instances.insert(
                pattern_id.clone(),
                pattern_parameters_from_settings(
                    pattern_id.clone(),
                    &WeightedVoronoiSettings::default(),
                ),
            );
        }
        if !self.instances.contains_key(&pattern_id)
            && !self.embedded_patterns.contains_key(&pattern_id)
            && !self.bundled_definition_instances.contains_key(&pattern_id)
        {
            let registry = crate::load_bundled_pattern_definition_registry()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let resolved = registry
                .get(&pattern_id)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let instance = resolved
                .definition
                .default_instance_parameters(
                    OutputChannelId::CMYK
                        .into_iter()
                        .chain(OutputChannelId::RGB),
                )
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            self.bundled_definition_instances
                .insert(pattern_id.clone(), instance);
        }
        anyhow::ensure!(
            self.instances.contains_key(&pattern_id)
                || self.bundled_definition_instances.contains_key(&pattern_id)
                || self.embedded_patterns.contains_key(&pattern_id),
            "selected pattern {pattern_id} has no authoritative parameter state or embedded definition"
        );
        self.selected = PatternSelection::Registered(pattern_id);
        Ok(())
    }

    /// Validates and installs a project-owned native recipe before selecting
    /// it. Callers receive an error before this state changes.
    pub(crate) fn install_and_select_embedded_pattern(
        &mut self,
        definition: PatternDefinition,
        instance: PatternInstanceParameters,
    ) -> anyhow::Result<()> {
        let id = definition.id.clone();
        anyhow::ensure!(
            PATTERN_REGISTRY.get(id.clone()).is_none(),
            "project-embedded pattern {id} conflicts with a built-in pattern"
        );
        anyhow::ensure!(
            crate::load_bundled_pattern_definition_registry()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?
                .get(&id)
                .is_err(),
            "project-embedded pattern {id} conflicts with an immutable bundled definition"
        );
        anyhow::ensure!(
            instance.pattern_id == id,
            "project-embedded pattern {id} contradicts instance {}",
            instance.pattern_id
        );
        definition
            .validate()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        definition
            .validate_instance_parameters(&instance)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        self.embedded_patterns.insert(
            id.clone(),
            EmbeddedPatternDefinition {
                definition,
                instance,
            },
        );
        self.selected = PatternSelection::Registered(id);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn select_native_basic(&mut self) {
        self.selected = PatternSelection::NativeBasicV1;
    }

    /// Reads typed Shapes settings from authoritative registered parameters.
    /// This does not inspect the transient renderer adapter.
    pub fn shape_settings(&self) -> anyhow::Result<WebShapeSettings> {
        pattern_settings_from_parameters(
            self.instances
                .get(&PatternId::COMPATIBILITY_SHAPES_V1)
                .ok_or_else(|| anyhow::anyhow!("missing authoritative Shapes state"))?,
        )
    }

    /// Reads typed Curves settings from authoritative registered parameters.
    /// This does not inspect the transient renderer adapter.
    pub fn curve_settings(&self) -> anyhow::Result<WebCurveSettings> {
        pattern_settings_from_parameters(
            self.instances
                .get(&PatternId::COMPATIBILITY_CURVES_V1)
                .ok_or_else(|| anyhow::anyhow!("missing authoritative Curves state"))?,
        )
    }

    /// Reads typed Weighted Voronoi settings from persisted authority only.
    pub fn weighted_voronoi_settings(&self) -> anyhow::Result<WeightedVoronoiSettings> {
        weighted_voronoi_settings_from_parameters(
            self.instances
                .get(&PatternId::WEIGHTED_VORONOI_V1)
                .ok_or_else(|| anyhow::anyhow!("missing authoritative Weighted Voronoi state"))?,
        )
    }

    /// Replaces typed Shapes parameters while retaining the existing explicit
    /// selection. Callers choose a pattern separately through `select_pattern`.
    pub(crate) fn set_shape_settings(&mut self, settings: WebShapeSettings) {
        self.instances.insert(
            PatternId::COMPATIBILITY_SHAPES_V1,
            pattern_parameters_from_settings(PatternId::COMPATIBILITY_SHAPES_V1, &settings),
        );
    }

    /// Keep the output-channel parameters of a selected project recipe in
    /// lockstep with the typed Shapes settings edited by the inspector.  The
    /// built-in Shapes instance remains the compatibility authority, while a
    /// custom recipe must receive the same channel values because its native
    /// runtime reads its own validated instance.
    pub(crate) fn sync_embedded_shapes_from_settings(
        &mut self,
        settings: &WebShapeSettings,
    ) -> anyhow::Result<()> {
        let Some(pattern_id) = self
            .selected_pattern_id()
            .filter(|pattern_id| self.embedded_patterns.contains_key(pattern_id))
        else {
            return Ok(());
        };
        for channel in OutputChannelId::CMYK
            .into_iter()
            .chain(OutputChannelId::RGB)
        {
            let values =
                self.embedded_shape_channel_values(settings.channels.get(channel.to_legacy_ink()));
            for (key, value) in values {
                self.set_embedded_output_channel_value(&pattern_id, channel, key, value)?;
            }
        }
        Ok(())
    }

    fn embedded_shape_channel_values(
        &self,
        channel: &WebShapeChannel,
    ) -> Vec<(&'static str, LiteralValue)> {
        vec![
            ("enabled", LiteralValue::Boolean(channel.enabled)),
            ("color", LiteralValue::Text(channel.color.clone())),
            ("rotation", LiteralValue::Number(channel.rotation)),
            ("grid-rotation", LiteralValue::Number(channel.grid_rotation)),
            ("grid-pivot-x", LiteralValue::Number(channel.grid_pivot_x)),
            ("grid-pivot-y", LiteralValue::Number(channel.grid_pivot_y)),
            ("scale", LiteralValue::Number(channel.scale)),
            ("width-scale", LiteralValue::Number(channel.width_scale)),
            ("height-scale", LiteralValue::Number(channel.height_scale)),
            ("threshold", LiteralValue::Number(channel.threshold)),
            ("max-size", LiteralValue::Number(channel.max_size)),
            (
                "resolution-scale",
                LiteralValue::Number(channel.resolution_scale),
            ),
            (
                "random-size-response",
                LiteralValue::Number(channel.random_size_response),
            ),
            ("offset-x", LiteralValue::Number(channel.offset_x)),
            ("offset-y", LiteralValue::Number(channel.offset_y)),
            ("opacity", LiteralValue::Number(channel.opacity)),
            (
                "shape",
                LiteralValue::Choice(
                    match channel.shape {
                        WebShape::Circle => "circle",
                        WebShape::RegularPolygon => "regular-polygon",
                        WebShape::UserDefined => "user-defined",
                        WebShape::Rectangle => "rectangle",
                        WebShape::Triangle => "triangle",
                        WebShape::Pentagon => "pentagon",
                        WebShape::Hexagon => "hexagon",
                    }
                    .into(),
                ),
            ),
            (
                "channel-polygon-sides",
                LiteralValue::Integer(u64::from(channel.polygon_sides)),
            ),
        ]
    }

    fn set_embedded_output_channel_value(
        &mut self,
        pattern_id: &PatternId,
        channel: OutputChannelId,
        key: &str,
        value: LiteralValue,
    ) -> anyhow::Result<()> {
        let Some(embedded) = self.embedded_patterns.get_mut(pattern_id) else {
            return Ok(());
        };
        if !embedded
            .definition
            .parameters
            .iter()
            .any(|parameter| parameter.key == key)
        {
            return Ok(());
        }
        let channel_id = channel.stable_id();
        let values = embedded
            .instance
            .output_channel_values
            .iter_mut()
            .find(|values| values.channel == channel_id)
            .ok_or_else(|| {
                anyhow::anyhow!("embedded pattern is missing output channel {channel_id}")
            })?;
        if let Some(existing) = values.values.iter_mut().find(|entry| entry.key == key) {
            existing.value = value;
        } else {
            values.values.push(PatternInstanceValue {
                key: key.into(),
                value,
            });
        }
        Ok(())
    }

    /// Replaces typed Curves parameters while retaining the existing explicit
    /// selection. Callers choose a pattern separately through `select_pattern`.
    pub(crate) fn set_curve_settings(&mut self, settings: WebCurveSettings) {
        self.instances.insert(
            PatternId::COMPATIBILITY_CURVES_V1,
            pattern_parameters_from_settings(PatternId::COMPATIBILITY_CURVES_V1, &settings),
        );
    }

    pub(crate) fn set_weighted_voronoi_settings(&mut self, settings: WeightedVoronoiSettings) {
        self.instances.insert(
            PatternId::WEIGHTED_VORONOI_V1,
            pattern_parameters_from_settings(PatternId::WEIGHTED_VORONOI_V1, &settings),
        );
    }

    #[cfg(test)]
    pub(crate) fn set_selected_parameters_for_test(&mut self, render: &RenderVariant) {
        // Test fixtures may seed typed parameters through this convenience
        // method, but the fixture must select the authoritative pattern first.
        // In particular, never infer or mutate selection from a RenderVariant.
        match (&self.selected, render) {
            (PatternSelection::NativeBasicV1, RenderVariant::NativeBasicV1) => {}
            (PatternSelection::Registered(id), RenderVariant::WebShapeV1 { settings })
                if *id == PatternId::COMPATIBILITY_SHAPES_V1 =>
            {
                self.set_shape_settings((**settings).clone())
            }
            (PatternSelection::Registered(id), RenderVariant::WebCurveV1 { settings })
                if *id == PatternId::COMPATIBILITY_CURVES_V1 =>
            {
                self.set_curve_settings((**settings).clone())
            }
            (PatternSelection::Registered(id), RenderVariant::WeightedVoronoiCanonicalV1)
                if *id == PatternId::WEIGHTED_VORONOI_V1 => {}
            _ => {
                panic!("select the authoritative pattern explicitly before setting test parameters")
            }
        }
    }

    pub(crate) fn for_treatment_scope(&self) -> anyhow::Result<Self> {
        self.validate()?;
        let mut state = self.clone();
        let mut shapes = state.shape_settings()?;
        shapes.channels = WebShapeSettings::default().channels;
        state.set_shape_settings(shapes);
        let mut curves = state.curve_settings()?;
        curves.channels = WebCurveSettings::default().channels;
        state.set_curve_settings(curves);
        Ok(state)
    }

    pub(crate) fn restore_selected_channels_from(&mut self, source: &Self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.selected == source.selected,
            "cannot restore channels across different pattern selections"
        );
        match &self.selected {
            PatternSelection::NativeBasicV1 => {}
            PatternSelection::Registered(id) if *id == PatternId::COMPATIBILITY_SHAPES_V1 => {
                let mut settings = self.shape_settings()?;
                settings.channels = source.shape_settings()?.channels;
                self.set_shape_settings(settings);
            }
            PatternSelection::Registered(id) if *id == PatternId::COMPATIBILITY_CURVES_V1 => {
                let mut settings = self.curve_settings()?;
                settings.channels = source.curve_settings()?.channels;
                self.set_curve_settings(settings);
            }
            PatternSelection::Registered(id) if *id == PatternId::WEIGHTED_VORONOI_V1 => {}
            PatternSelection::Registered(id)
                if self.bundled_definition_instances.contains_key(id) => {}
            PatternSelection::Registered(id) if self.embedded_patterns.contains_key(id) => {}
            PatternSelection::Registered(id) => anyhow::bail!("unregistered pattern {id}"),
        }
        Ok(())
    }
}

fn pattern_parameters_from_settings<T: Serialize>(
    pattern_id: PatternId,
    settings: &T,
) -> VersionedPatternParameters {
    let metadata = PATTERN_REGISTRY
        .get(pattern_id.clone())
        .expect("built-in compatibility pattern must be registered");
    let mut values = Map::new();
    let mut serialized =
        serde_json::to_value(settings).expect("compatibility settings must serialize");
    let object = serialized
        .as_object_mut()
        .expect("compatibility settings serialize as objects");
    // These are projections of `ArtworkPipelineSettings`, never pattern
    // parameters. They are restored only while building the transient adapter.
    object.remove("value_mode");
    object.remove("single_channel");
    values.insert("settings".into(), serialized);
    VersionedPatternParameters {
        pattern_id,
        schema_version: metadata.parameter_schema_version,
        generator_version: metadata.generator_version,
        values,
    }
}

fn pattern_settings_from_parameters<T: for<'de> Deserialize<'de>>(
    parameters: &VersionedPatternParameters,
) -> anyhow::Result<T> {
    anyhow::ensure!(
        parameters.values.len() == 1 && parameters.values.contains_key("settings"),
        "authoritative pattern {} has malformed typed settings",
        parameters.pattern_id
    );
    let mut settings = parameters
        .values
        .get("settings")
        .expect("checked settings key")
        .clone();
    let object = settings.as_object_mut().ok_or_else(|| {
        anyhow::anyhow!(
            "authoritative pattern {} has malformed typed settings",
            parameters.pattern_id
        )
    })?;
    object.insert("value_mode".into(), serde_json::json!("cmyk"));
    object.insert("single_channel".into(), serde_json::json!("black"));
    serde_json::from_value(settings).map_err(|error| {
        anyhow::anyhow!(
            "authoritative pattern {} has malformed typed settings: {error}",
            parameters.pattern_id
        )
    })
}

fn weighted_voronoi_settings_from_parameters(
    parameters: &VersionedPatternParameters,
) -> anyhow::Result<WeightedVoronoiSettings> {
    anyhow::ensure!(
        parameters.values.len() == 1 && parameters.values.contains_key("settings"),
        "authoritative pattern {} has malformed typed settings",
        parameters.pattern_id
    );
    serde_json::from_value(
        parameters
            .values
            .get("settings")
            .expect("checked settings key")
            .clone(),
    )
    .map_err(|error| {
        anyhow::anyhow!(
            "authoritative pattern {} has malformed typed settings: {error}",
            parameters.pattern_id
        )
    })
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

/// The complete editable treatment for the output mode that is not currently
/// visible. Keeping this in the document makes switching output modes lossless
/// and makes undo/redo a single ordinary document edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputTreatmentCache {
    pub settings: Settings,
    pub pattern_state: PatternDocumentState,
    /// A derived, non-persisted adapter retained only for the legacy Shapes
    /// and Curves renderer during the TON-010 transition.
    #[serde(skip)]
    pub render: RenderVariant,
    /// Optional presentation snapshot for this output model. A missing current
    /// cache snapshot resolves to the model-specific default; document schema
    /// versions remain current-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_surface: Option<PreviewSurface>,
    #[serde(skip)]
    pub saved_web_shape: Option<Box<WebShapeSettings>>,
    #[serde(skip)]
    pub saved_web_curve: Option<Box<WebCurveSettings>>,
    pub artwork_pipeline: ArtworkPipelineSettings,
    #[serde(skip)]
    pub saved_web_shape_pipeline: Option<ArtworkPipelineSettings>,
    #[serde(skip)]
    pub saved_web_curve_pipeline: Option<ArtworkPipelineSettings>,
}

impl OutputTreatmentCache {
    fn canonicalize_pipeline_facades(&mut self) -> anyhow::Result<()> {
        self.pattern_state.validate()?;
        self.render = self.pattern_state.adapter()?;
        apply_pipeline_projection(&mut self.render, &self.artwork_pipeline)?;
        canonicalize_saved_shape_facade(&mut self.saved_web_shape, &self.saved_web_shape_pipeline)?;
        canonicalize_saved_curve_facade(&mut self.saved_web_curve, &self.saved_web_curve_pipeline)?;
        Ok(())
    }

    fn validate_for(&self, owner: OutputMode) -> anyhow::Result<()> {
        self.artwork_pipeline.validate()?;
        if matches!(
            self.artwork_pipeline.assignment,
            ChannelAssignment::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1
            )
        ) {
            anyhow::ensure!(
                matches!(self.render, RenderVariant::WebCurveV1 { .. }),
                "cached Crosshatch requires a Curves treatment"
            );
        }
        anyhow::ensure!(
            self.artwork_pipeline.output_model == OutputModel::from_legacy(owner),
            "inactive treatment pipeline belongs to the wrong output mode"
        );
        anyhow::ensure!(
            self.saved_web_shape.is_some() == self.saved_web_shape_pipeline.is_some(),
            "cached Shapes treatment and pipeline snapshot must be paired"
        );
        anyhow::ensure!(
            self.saved_web_curve.is_some() == self.saved_web_curve_pipeline.is_some(),
            "cached Curves treatment and pipeline snapshot must be paired"
        );
        if let Some(pipeline) = &self.saved_web_shape_pipeline {
            pipeline.validate()?;
            anyhow::ensure!(
                !matches!(
                    pipeline.assignment,
                    ChannelAssignment::LegacyCompatibility(
                        LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1
                    )
                ),
                "Crosshatch cannot be stored as a Shapes treatment"
            );
            anyhow::ensure!(
                pipeline.output_model == OutputModel::from_legacy(owner),
                "cached Shapes pipeline belongs to the wrong output mode"
            );
        }
        if let Some(pipeline) = &self.saved_web_curve_pipeline {
            pipeline.validate()?;
            anyhow::ensure!(
                pipeline.output_model == OutputModel::from_legacy(owner),
                "cached Curves pipeline belongs to the wrong output mode"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub format: String,
    pub version: u32,
    pub document_id: String,
    pub source: SourceArtwork,
    pub settings: Settings,
    pub appearance: DocumentAppearance,
    /// The only live source/output/assignment authority. Legacy render fields
    /// below are a renderer/UI compatibility projection.
    pub artwork_pipeline: ArtworkPipelineSettings,
    pub output_mode: OutputMode,
    pub pattern_state: PatternDocumentState,
    /// A derived, non-persisted adapter retained only for the legacy Shapes
    /// and Curves renderer during the TON-010 transition.
    #[serde(skip)]
    pub render: RenderVariant,
    #[serde(skip)]
    pub saved_web_shape: Option<Box<WebShapeSettings>>,
    #[serde(skip)]
    pub saved_web_shape_pipeline: Option<ArtworkPipelineSettings>,
    #[serde(skip)]
    pub saved_web_curve: Option<Box<WebCurveSettings>>,
    #[serde(skip)]
    pub saved_web_curve_pipeline: Option<ArtworkPipelineSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inactive_cmyk: Option<Box<OutputTreatmentCache>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inactive_rgb: Option<Box<OutputTreatmentCache>>,
}

impl Document {
    pub fn new(source: SourceArtwork) -> Self {
        Self {
            format: DOCUMENT_FORMAT.to_owned(),
            version: DOCUMENT_VERSION,
            document_id: new_document_id(),
            source,
            settings: Settings::default(),
            appearance: DocumentAppearance::default(),
            artwork_pipeline: ArtworkPipelineSettings::default(),
            output_mode: OutputMode::CmykInks,
            pattern_state: PatternDocumentState::new(),
            render: RenderVariant::WebShapeV1 {
                settings: Box::new(WebShapeSettings::default()),
            },
            saved_web_shape: None,
            saved_web_shape_pipeline: None,
            saved_web_curve: None,
            saved_web_curve_pipeline: None,
            inactive_cmyk: None,
            inactive_rgb: None,
        }
    }

    fn active_treatment(&self) -> OutputTreatmentCache {
        OutputTreatmentCache {
            settings: self.settings,
            pattern_state: self.pattern_state.clone(),
            render: self.render.clone(),
            preview_surface: Some(self.appearance.preview_surface),
            saved_web_shape: self.saved_web_shape.clone(),
            saved_web_shape_pipeline: self.saved_web_shape_pipeline.clone(),
            saved_web_curve: self.saved_web_curve.clone(),
            saved_web_curve_pipeline: self.saved_web_curve_pipeline.clone(),
            artwork_pipeline: self.artwork_pipeline.clone(),
        }
    }

    fn apply_treatment(&mut self, treatment: OutputTreatmentCache) {
        self.settings = treatment.settings;
        self.pattern_state = treatment.pattern_state;
        self.render = treatment.render;
        self.saved_web_shape = treatment.saved_web_shape;
        self.saved_web_shape_pipeline = treatment.saved_web_shape_pipeline;
        self.saved_web_curve = treatment.saved_web_curve;
        self.saved_web_curve_pipeline = treatment.saved_web_curve_pipeline;
        self.artwork_pipeline = treatment.artwork_pipeline;
    }

    fn default_preview_surface(output: OutputMode) -> PreviewSurface {
        match output {
            OutputMode::CmykInks => PreviewSurface::Color {
                color: RgbaColor::WHITE,
            },
            OutputMode::RgbScreen => PreviewSurface::Color {
                color: RgbaColor::opaque(0, 0, 0),
            },
        }
    }

    fn new_rgb_treatment(&self) -> OutputTreatmentCache {
        // The first RGB visit has no cached treatment yet.  Preserve the
        // semantic source/alpha/assignment state and translate only its
        // output-model-dependent channel before installing this treatment.
        let artwork_pipeline = self
            .artwork_pipeline
            .clone()
            .transition_output_model(OutputModel::RgbScreen, None)
            .expect("valid pipeline output transition");
        OutputTreatmentCache {
            settings: self.settings,
            pattern_state: self.pattern_state.clone(),
            render: self.render.clone(),
            preview_surface: None,
            saved_web_shape: None,
            saved_web_curve: None,
            artwork_pipeline,
            saved_web_shape_pipeline: None,
            saved_web_curve_pipeline: None,
        }
    }

    pub fn switch_output_mode(&mut self, target: OutputMode) -> bool {
        let current = self.artwork_pipeline.output_model.to_legacy();
        if target == current {
            return false;
        }
        let active = self.active_treatment();
        let replacement = match target {
            OutputMode::CmykInks => self
                .inactive_cmyk
                .take()
                .unwrap_or_else(|| Box::new(active.clone())),
            OutputMode::RgbScreen => self
                .inactive_rgb
                .take()
                .unwrap_or_else(|| Box::new(self.new_rgb_treatment())),
        };
        match current {
            OutputMode::CmykInks => self.inactive_cmyk = Some(Box::new(active)),
            OutputMode::RgbScreen => self.inactive_rgb = Some(Box::new(active)),
        }
        let preview_surface = replacement
            .preview_surface
            .unwrap_or_else(|| Self::default_preview_surface(target));
        self.apply_treatment(*replacement);
        self.appearance.preview_surface = preview_surface;
        let target_pipeline = self
            .artwork_pipeline
            .clone()
            .transition_output_model(OutputModel::from_legacy(target), None)
            .expect("valid pipeline output transition");
        self.artwork_pipeline = target_pipeline;
        self.sync_legacy_projection()
            .expect("projectable legacy compatibility state");
        true
    }

    /// Applies a preset-owned pipeline through the normal output transition so
    /// inactive mode caches remain paired with their semantic pipeline.  This
    /// is intentionally separate from ordinary pipeline edits: preset input
    /// may explicitly request a different output model.
    pub fn apply_preset_pipeline(
        &mut self,
        pipeline: ArtworkPipelineSettings,
    ) -> anyhow::Result<()> {
        self.apply_preset_pipeline_unchecked(pipeline)?;
        self.validate()
    }

    /// Stage a preset pipeline after its semantic validation but before the
    /// rest of a complete-workflow candidate is installed.  Crosshatch is the
    /// one transitional representation that needs its Curves treatment added
    /// before whole-document validation can succeed.
    pub(crate) fn apply_preset_pipeline_unchecked(
        &mut self,
        mut pipeline: ArtworkPipelineSettings,
    ) -> anyhow::Result<()> {
        let omitted_active_channel = pipeline.active_channel.is_none()
            && matches!(pipeline.assignment, ChannelAssignment::ActiveChannel);
        if omitted_active_channel {
            // Current presets may omit Active Channel to request the
            // receiving/output-transition destination. Validate every other
            // invariant against a representative valid destination first.
            let mut representative = pipeline.clone();
            representative.active_channel = Some(pipeline.output_model.default_channel());
            representative.validate()?;
        } else {
            pipeline.validate()?;
        }
        let prior_active_channel = self.artwork_pipeline.active_channel;
        if self.artwork_pipeline.output_model != pipeline.output_model {
            self.switch_output_mode(pipeline.output_model.to_legacy());
        }
        // An omitted active channel means "use the transition's last valid
        // channel", not "clear the current channel". Crosshatch is the one
        // compatibility assignment whose invariant requires no active
        // channel at all.
        if pipeline.active_channel.is_none()
            && !matches!(
                pipeline.assignment,
                ChannelAssignment::LegacyCompatibility(_)
            )
        {
            pipeline.active_channel = prior_active_channel
                .filter(|channel| channel.belongs_to(pipeline.output_model))
                .or_else(|| {
                    prior_active_channel.and_then(|channel| {
                        OutputChannelId::from_legacy_slot(
                            channel.legacy_slot(),
                            pipeline.output_model,
                        )
                        .ok()
                    })
                })
                .or_else(|| {
                    matches!(pipeline.assignment, ChannelAssignment::ActiveChannel)
                        .then(|| pipeline.output_model.default_channel())
                });
        }
        pipeline.validate()?;
        self.artwork_pipeline = pipeline;
        // The complete-workflow caller may still need to replace a Shapes
        // treatment with Crosshatch Curves.  Do not project that temporary,
        // intentionally incomplete candidate into the renderer facade yet.
        self.output_mode = self.artwork_pipeline.output_model.to_legacy();
        Ok(())
    }

    /// Replaces the active treatment while preserving the outgoing treatment
    /// snapshot used when the creator returns to its kind.  Preset parsing
    /// constructs and validates a complete candidate before this is called.
    pub fn apply_preset_treatment(
        &mut self,
        pattern_state: PatternDocumentState,
        native_settings: Option<Settings>,
    ) -> anyhow::Result<()> {
        pattern_state.validate()?;
        let next_render = pattern_state.adapter()?;
        if render_kind(&self.render) != render_kind(&next_render) {
            match &self.render {
                RenderVariant::WebShapeV1 { settings } => {
                    self.saved_web_shape = Some(settings.clone());
                    self.saved_web_shape_pipeline = Some(self.artwork_pipeline.clone());
                }
                RenderVariant::WebCurveV1 { settings } => {
                    self.saved_web_curve = Some(settings.clone());
                    self.saved_web_curve_pipeline = Some(self.artwork_pipeline.clone());
                }
                RenderVariant::NativeBasicV1 | RenderVariant::WeightedVoronoiCanonicalV1 => {}
            }
        }
        if let Some(settings) = native_settings {
            self.settings = settings.sanitized();
        }
        self.pattern_state = pattern_state;
        self.sync_legacy_projection()?;
        self.validate()
    }

    /// Rebuild the legacy facade exclusively from the semantic pipeline.
    /// Renderers continue to consume this snapshot while Stage 1B preserves
    /// their established formulas byte-for-byte.
    pub fn sync_legacy_projection(&mut self) -> anyhow::Result<()> {
        self.output_mode = self.artwork_pipeline.output_model.to_legacy();
        self.pattern_state.validate()?;
        self.render = self.pattern_state.adapter()?;
        apply_pipeline_projection(&mut self.render, &self.artwork_pipeline)?;
        Ok(())
    }

    pub fn canonicalize_pipeline_facades(&mut self) -> anyhow::Result<()> {
        self.sync_legacy_projection()?;
        canonicalize_saved_shape_facade(&mut self.saved_web_shape, &self.saved_web_shape_pipeline)?;
        canonicalize_saved_curve_facade(&mut self.saved_web_curve, &self.saved_web_curve_pipeline)?;
        for cache in [&mut self.inactive_cmyk, &mut self.inactive_rgb]
            .into_iter()
            .flatten()
        {
            cache.canonicalize_pipeline_facades()?;
        }
        Ok(())
    }

    // This is intentionally the only legacy-field writer.
    fn apply_projection_to_render(
        render: &mut RenderVariant,
        projection: crate::artwork_pipeline::LegacyValueModeProjection,
    ) {
        let apply = |settings: &mut ValueMode, channel: &mut Ink| {
            *settings = projection.value_mode;
            if let Some(destination) = projection.scalar_destination {
                *channel = destination;
            }
        };
        match render {
            RenderVariant::WebShapeV1 { settings } => {
                apply(&mut settings.value_mode, &mut settings.single_channel)
            }
            RenderVariant::WebCurveV1 { settings } => {
                apply(&mut settings.value_mode, &mut settings.single_channel)
            }
            RenderVariant::NativeBasicV1 | RenderVariant::WeightedVoronoiCanonicalV1 => {}
        }
    }

    pub fn projected_for_render(&self) -> anyhow::Result<Self> {
        let mut projected = self.clone();
        projected.canonicalize_pipeline_facades()?;
        Ok(projected)
    }

    pub fn apply_legacy_mapping_action(&mut self, mapping: ValueMode) -> anyhow::Result<()> {
        let forced_output = match mapping {
            ValueMode::Cmyk => Some(OutputModel::CmykPrint),
            ValueMode::Rgb => Some(OutputModel::RgbScreen),
            _ => None,
        };
        if let Some(output) = forced_output
            && self.artwork_pipeline.output_model != output
        {
            // Forced legacy Color/RGB mappings use the same lossless cache
            // transition as the Output control before changing assignment.
            self.switch_output_mode(output.to_legacy());
        }
        let output = forced_output.unwrap_or(self.artwork_pipeline.output_model);
        let retained_channel = self
            .artwork_pipeline
            .active_channel
            .filter(|channel| channel.belongs_to(output))
            .or_else(|| Some(output.default_channel()));
        self.artwork_pipeline = match mapping {
            ValueMode::Cmyk => ArtworkPipelineSettings {
                source: ArtworkSource::FullColor,
                alpha_policy: crate::artwork_pipeline::SourceAlphaPolicy::LegacyCurrentV1,
                output_model: OutputModel::CmykPrint,
                assignment: ChannelAssignment::automatic(
                    AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1,
                ),
                active_channel: retained_channel,
            },
            ValueMode::Rgb => ArtworkPipelineSettings {
                source: ArtworkSource::FullColor,
                alpha_policy: crate::artwork_pipeline::SourceAlphaPolicy::LegacyCurrentV1,
                output_model: OutputModel::RgbScreen,
                assignment: ChannelAssignment::automatic(
                    AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1,
                ),
                active_channel: retained_channel,
            },
            ValueMode::Luminance => ArtworkPipelineSettings {
                source: ArtworkSource::LegacyBrightness(
                    LegacyBrightnessKind::EncodedRec709InvertedV1,
                ),
                alpha_policy: crate::artwork_pipeline::SourceAlphaPolicy::LegacyCurrentV1,
                output_model: output,
                assignment: ChannelAssignment::AllChannels,
                active_channel: None,
            },
            ValueMode::SingleChannel => ArtworkPipelineSettings {
                source: ArtworkSource::LegacyBrightness(
                    LegacyBrightnessKind::EncodedRec709InvertedV1,
                ),
                alpha_policy: crate::artwork_pipeline::SourceAlphaPolicy::LegacyCurrentV1,
                output_model: output,
                assignment: ChannelAssignment::ActiveChannel,
                active_channel: retained_channel,
            },
            ValueMode::CrosshatchLuminance => ArtworkPipelineSettings {
                source: ArtworkSource::LegacyBrightness(
                    LegacyBrightnessKind::EncodedRec709InvertedV1,
                ),
                alpha_policy: crate::artwork_pipeline::SourceAlphaPolicy::LegacyCurrentV1,
                output_model: output,
                assignment: ChannelAssignment::LegacyCompatibility(
                    LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1,
                ),
                active_channel: None,
            },
        };
        self.sync_legacy_projection()
    }

    pub fn select_active_output_channel(&mut self, channel: OutputChannelId) -> anyhow::Result<()> {
        anyhow::ensure!(
            matches!(
                self.artwork_pipeline.assignment,
                ChannelAssignment::ActiveChannel
            ),
            "active channel requires one-channel mapping"
        );
        anyhow::ensure!(
            channel.belongs_to(self.artwork_pipeline.output_model),
            "active channel is outside the current output model"
        );
        self.artwork_pipeline.active_channel = Some(channel);
        self.sync_legacy_projection()
    }

    pub fn new_with_artboard(source: SourceArtwork, width: u32, height: u32) -> Self {
        let mut document = Self::new(source);
        let mut settings = document
            .pattern_state
            .shape_settings()
            .expect("new document has Shapes state");
        settings.output_width = width.max(1);
        settings.output_height = height.max(1);
        document.pattern_state.set_shape_settings(settings);
        document
            .sync_legacy_projection()
            .expect("new document pattern state projects");
        document
    }

    /// Makes every stored canvas use the decoded source aspect ratio. The
    /// requested long edge is preserved (subject to the document cap), so old
    /// presets retain their intended scale without retaining distortion.
    pub fn normalize_canvas_aspect(&mut self, source_width: u32, source_height: u32) -> bool {
        let mut changed = match &self.pattern_state.selected {
            PatternSelection::NativeBasicV1 => false,
            PatternSelection::Registered(id) if *id == PatternId::COMPATIBILITY_SHAPES_V1 => {
                let mut settings = self
                    .pattern_state
                    .shape_settings()
                    .expect("validated Shapes state");
                let changed = normalize_canvas_dimensions(
                    &mut settings.output_width,
                    &mut settings.output_height,
                    source_width,
                    source_height,
                );
                if changed {
                    self.pattern_state.set_shape_settings(settings);
                }
                changed
            }
            PatternSelection::Registered(id) if *id == PatternId::COMPATIBILITY_CURVES_V1 => {
                let mut settings = self
                    .pattern_state
                    .curve_settings()
                    .expect("validated Curves state");
                let changed = normalize_canvas_dimensions(
                    &mut settings.output_width,
                    &mut settings.output_height,
                    source_width,
                    source_height,
                );
                if changed {
                    self.pattern_state.set_curve_settings(settings);
                }
                changed
            }
            PatternSelection::Registered(id) if *id == PatternId::WEIGHTED_VORONOI_V1 => false,
            PatternSelection::Registered(_) => false,
        };
        if let Some(settings) = self.saved_web_shape.as_deref_mut() {
            changed |= normalize_canvas_dimensions(
                &mut settings.output_width,
                &mut settings.output_height,
                source_width,
                source_height,
            );
        }
        if let Some(settings) = self.saved_web_curve.as_deref_mut() {
            changed |= normalize_canvas_dimensions(
                &mut settings.output_width,
                &mut settings.output_height,
                source_width,
                source_height,
            );
        }
        if changed {
            let _ = self.sync_legacy_projection();
        }
        changed
    }

    /// Keeps the current Crosshatch compatibility assignment on its configured
    /// curve treatment so it never renders dot layers under a hatch label.
    pub fn normalize_crosshatch_treatment(&mut self) -> bool {
        let changed = matches!(
            self.artwork_pipeline.assignment,
            ChannelAssignment::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1
            )
        ) && matches!(
            &self.pattern_state.selected,
            PatternSelection::Registered(id) if *id == PatternId::COMPATIBILITY_SHAPES_V1
        );
        if changed {
            let shapes = self
                .pattern_state
                .shape_settings()
                .expect("validated Shapes state");
            self.pattern_state
                .set_curve_settings(WebCurveSettings::crosshatch_from_shape(&shapes));
            self.pattern_state
                .select_pattern(PatternId::COMPATIBILITY_CURVES_V1)
                .expect("registered Curves state");
            let _ = self.sync_legacy_projection();
        }
        changed
    }

    /// Verifies that a strictly decoded current document has exactly one
    /// recoverable defect: its active registered stable ID has lost all
    /// authority.  The returned document remains invalid and must never reach
    /// an editor; this exists solely for the persistence recovery candidate.
    pub(crate) fn missing_selected_pattern_definition(
        &self,
    ) -> anyhow::Result<MissingSelectedPatternDefinition> {
        let missing = self.pattern_state.missing_selected_definition()?;
        let mut remainder = self.clone();
        remainder.pattern_state.selected = PatternSelection::NativeBasicV1;
        remainder
            .pattern_state
            .bundled_definition_instances
            .remove(&missing.requested_id);
        remainder.canonicalize_pipeline_facades()?;
        remainder.validate()?;
        Ok(missing)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.format == DOCUMENT_FORMAT, "not a Toniator document");
        anyhow::ensure!(
            self.version == DOCUMENT_VERSION,
            "unsupported Toniator document version {}",
            self.version
        );
        self.artwork_pipeline.validate()?;
        self.pattern_state.validate()?;
        if matches!(
            self.artwork_pipeline.assignment,
            ChannelAssignment::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1
            )
        ) {
            anyhow::ensure!(
                matches!(self.render, RenderVariant::WebCurveV1 { .. }),
                "Crosshatch requires a Curves treatment"
            );
        }
        anyhow::ensure!(
            self.saved_web_shape.is_some() == self.saved_web_shape_pipeline.is_some(),
            "saved Shapes treatment and pipeline snapshot must be paired"
        );
        anyhow::ensure!(
            self.saved_web_curve.is_some() == self.saved_web_curve_pipeline.is_some(),
            "saved Curves treatment and pipeline snapshot must be paired"
        );
        if let Some(pipeline) = &self.saved_web_shape_pipeline {
            pipeline.validate()?;
            anyhow::ensure!(
                !matches!(
                    pipeline.assignment,
                    ChannelAssignment::LegacyCompatibility(
                        LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1
                    )
                ),
                "Crosshatch cannot be stored as a Shapes treatment"
            );
            anyhow::ensure!(
                pipeline.output_model == self.artwork_pipeline.output_model,
                "saved Shapes pipeline belongs to the wrong output mode"
            );
        }
        if let Some(pipeline) = &self.saved_web_curve_pipeline {
            pipeline.validate()?;
            anyhow::ensure!(
                pipeline.output_model == self.artwork_pipeline.output_model,
                "saved Curves pipeline belongs to the wrong output mode"
            );
        }
        if let Some(cache) = self.inactive_cmyk.as_deref() {
            cache.validate_for(OutputMode::CmykInks)?;
        }
        if let Some(cache) = self.inactive_rgb.as_deref() {
            cache.validate_for(OutputMode::RgbScreen)?;
        }
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
            let base = &settings.base_channel;
            anyhow::ensure!(
                [
                    base.rotation,
                    base.grid_rotation,
                    base.grid_pivot_x,
                    base.grid_pivot_y,
                    base.scale,
                    base.width_scale,
                    base.height_scale,
                    base.threshold,
                    base.max_size,
                    base.resolution_scale,
                    base.offset_x,
                    base.offset_y,
                    base.opacity,
                    base.weight_influence,
                    base.random_size_response,
                ]
                .into_iter()
                .all(f64::is_finite)
                    && (0.0..=100.0).contains(&base.scale)
                    && (0.01..=100.0).contains(&base.width_scale)
                    && (0.01..=100.0).contains(&base.height_scale)
                    && (0.0..=1.0).contains(&base.threshold)
                    && (0.0..=10_000.0).contains(&base.max_size)
                    && base.resolution_scale > 0.0
                    && base.resolution_scale <= 100.0
                    && (0.0..=1.0).contains(&base.opacity)
                    && (0.001..=16.0).contains(&base.weight_influence)
                    && (0.0..=1.0).contains(&base.random_size_response),
                "web shape base value is outside the supported range"
            );
            for &ink in self.output_mode.inks() {
                let channel = settings.channels.get(ink);
                anyhow::ensure!(
                    [
                        channel.rotation,
                        channel.grid_rotation,
                        channel.grid_pivot_x,
                        channel.grid_pivot_y,
                        channel.scale,
                        channel.width_scale,
                        channel.height_scale,
                        channel.threshold,
                        channel.max_size,
                        channel.resolution_scale,
                        channel.offset_x,
                        channel.offset_y,
                        channel.opacity,
                        channel.weight_influence,
                        channel.random_size_response,
                    ]
                    .into_iter()
                    .all(f64::is_finite),
                    "invalid {} channel setting",
                    ink.label()
                );
                if let Some(path) = channel.custom_shape_path.as_ref() {
                    validate_shape_path(path)?;
                }
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
                        && (0.01..=100.0).contains(&channel.width_scale)
                        && (0.01..=100.0).contains(&channel.height_scale)
                        && (0.0..=1.0).contains(&channel.threshold)
                        && (0.0..=10_000.0).contains(&channel.max_size)
                        && (0.0..=1.0).contains(&channel.opacity)
                        && (0.001..=16.0).contains(&channel.weight_influence)
                        && (0.0..=1.0).contains(&channel.random_size_response),
                    "{} channel value is outside the supported range",
                    ink.label()
                );
            }
            anyhow::ensure!(
                (3..=6).contains(&settings.polygon_sides),
                "regular polygon must have between 3 and 6 sides"
            );
            validate_shape_nodes(&settings.custom_nodes)?;
            validate_shape_path(&settings.resolved_custom_shape_path())?;
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
            let base = &settings.base_channel;
            anyhow::ensure!(
                [
                    base.grid_rotation,
                    base.grid_pivot_x,
                    base.grid_pivot_y,
                    base.scale,
                    base.threshold,
                    base.max_size,
                    base.resolution_scale,
                    base.offset_x,
                    base.offset_y,
                    base.opacity,
                    base.output_quality,
                    base.curve_scale,
                    base.motif_bleed,
                    base.tile_spacing,
                    base.tile_angle,
                    base.tile_offset,
                    base.stack_spacing,
                    base.stack_angle,
                    base.stack_offset,
                    base.alternate_stack_offset,
                ]
                .into_iter()
                .all(f64::is_finite)
                    && (0.0..=100.0).contains(&base.scale)
                    && (0.0..=1.0).contains(&base.threshold)
                    && (0.0..=10_000.0).contains(&base.max_size)
                    && base.resolution_scale > 0.0
                    && base.resolution_scale <= 100.0
                    && (0.0..=1.0).contains(&base.opacity)
                    && base.output_quality > 0.0
                    && base.output_quality <= 100.0
                    && (0.1..=500.0).contains(&base.curve_scale)
                    && (0.0..=100.0).contains(&base.motif_bleed)
                    && (1..=10_000).contains(&base.tile_count)
                    && (-10_000.0..=10_000.0).contains(&base.tile_spacing)
                    && (1..=10_000).contains(&base.stack_count)
                    && (-10_000.0..=10_000.0).contains(&base.stack_spacing),
                "web curve base value is outside the supported range"
            );
            for &ink in self.output_mode.inks() {
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
        for (saved, pipeline) in [
            (
                self.saved_web_shape
                    .as_ref()
                    .map(|settings| RenderVariant::WebShapeV1 {
                        settings: settings.clone(),
                    }),
                self.saved_web_shape_pipeline.as_ref(),
            ),
            (
                self.saved_web_curve
                    .as_ref()
                    .map(|settings| RenderVariant::WebCurveV1 {
                        settings: settings.clone(),
                    }),
                self.saved_web_curve_pipeline.as_ref(),
            ),
        ]
        .into_iter()
        .filter_map(|(render, pipeline)| render.zip(pipeline))
        {
            let mut candidate = self.clone();
            candidate.render = saved;
            candidate.artwork_pipeline = pipeline.clone();
            candidate.output_mode = pipeline.output_model.to_legacy();
            candidate.saved_web_shape = None;
            candidate.saved_web_shape_pipeline = None;
            candidate.saved_web_curve = None;
            candidate.saved_web_curve_pipeline = None;
            candidate.inactive_cmyk = None;
            candidate.inactive_rgb = None;
            candidate.validate()?;
        }
        for (owner, cache) in [
            (OutputMode::CmykInks, self.inactive_cmyk.as_deref()),
            (OutputMode::RgbScreen, self.inactive_rgb.as_deref()),
        ]
        .into_iter()
        .filter_map(|(owner, cache)| cache.map(|cache| (owner, cache)))
        {
            let mut candidate = self.clone();
            candidate.apply_treatment(cache.clone());
            candidate.output_mode = owner;
            candidate.inactive_cmyk = None;
            candidate.inactive_rgb = None;
            candidate.validate()?;
        }
        Ok(())
    }
}

fn canonicalize_saved_shape_facade(
    settings: &mut Option<Box<WebShapeSettings>>,
    pipeline: &Option<ArtworkPipelineSettings>,
) -> anyhow::Result<()> {
    match (settings, pipeline) {
        (Some(settings), Some(pipeline)) => {
            let mut render = RenderVariant::WebShapeV1 {
                settings: settings.clone(),
            };
            apply_pipeline_projection(&mut render, pipeline)?;
            let RenderVariant::WebShapeV1 {
                settings: projected,
            } = render
            else {
                unreachable!()
            };
            *settings = projected;
            Ok(())
        }
        (None, None) => Ok(()),
        _ => anyhow::bail!("saved Shapes treatment and pipeline snapshot must be paired"),
    }
}

fn canonicalize_saved_curve_facade(
    settings: &mut Option<Box<WebCurveSettings>>,
    pipeline: &Option<ArtworkPipelineSettings>,
) -> anyhow::Result<()> {
    match (settings, pipeline) {
        (Some(settings), Some(pipeline)) => {
            let mut render = RenderVariant::WebCurveV1 {
                settings: settings.clone(),
            };
            apply_pipeline_projection(&mut render, pipeline)?;
            let RenderVariant::WebCurveV1 {
                settings: projected,
            } = render
            else {
                unreachable!()
            };
            *settings = projected;
            Ok(())
        }
        (None, None) => Ok(()),
        _ => anyhow::bail!("saved Curves treatment and pipeline snapshot must be paired"),
    }
}

fn apply_pipeline_projection(
    render: &mut RenderVariant,
    pipeline: &ArtworkPipelineSettings,
) -> anyhow::Result<()> {
    let projection = crate::artwork_pipeline::project_legacy_value_mode(pipeline)?;
    Document::apply_projection_to_render(render, projection);
    Ok(())
}

pub fn normalize_crosshatch_render(render: &mut RenderVariant) -> bool {
    let RenderVariant::WebShapeV1 { settings } = render else {
        return false;
    };
    if settings.value_mode != ValueMode::CrosshatchLuminance {
        return false;
    }
    *render = RenderVariant::WebCurveV1 {
        settings: Box::new(WebCurveSettings::crosshatch_from_shape(settings)),
    };
    true
}

pub fn normalize_render_variant_canvas(
    render: &mut RenderVariant,
    source_width: u32,
    source_height: u32,
) -> bool {
    match render {
        RenderVariant::NativeBasicV1 | RenderVariant::WeightedVoronoiCanonicalV1 => false,
        RenderVariant::WebShapeV1 { settings } => normalize_canvas_dimensions(
            &mut settings.output_width,
            &mut settings.output_height,
            source_width,
            source_height,
        ),
        RenderVariant::WebCurveV1 { settings } => normalize_canvas_dimensions(
            &mut settings.output_width,
            &mut settings.output_height,
            source_width,
            source_height,
        ),
    }
}

pub fn aspect_locked_dimensions(
    source_width: u32,
    source_height: u32,
    requested_long_edge: u32,
) -> (u32, u32) {
    let source_width = source_width.max(1) as u64;
    let source_height = source_height.max(1) as u64;
    let long = requested_long_edge.clamp(1, 100_000) as u64;
    if source_width >= source_height {
        let height = ((long * source_height + source_width / 2) / source_width).max(1);
        (long as u32, height.min(100_000) as u32)
    } else {
        let width = ((long * source_width + source_height / 2) / source_height).max(1);
        (width.min(100_000) as u32, long as u32)
    }
}

pub(crate) fn normalize_canvas_dimensions(
    width: &mut u32,
    height: &mut u32,
    source_width: u32,
    source_height: u32,
) -> bool {
    let normalized = aspect_locked_dimensions(source_width, source_height, (*width).max(*height));
    let changed = (*width, *height) != normalized;
    (*width, *height) = normalized;
    changed
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

pub fn validate_shape_nodes(nodes: &[ShapePoint]) -> anyhow::Result<()> {
    anyhow::ensure!(
        nodes.len() >= 3,
        "a user-defined mark needs at least three nodes"
    );
    anyhow::ensure!(
        nodes
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite()),
        "user-defined mark contains an invalid node"
    );
    let twice_area: f64 = nodes
        .iter()
        .zip(nodes.iter().cycle().skip(1))
        .take(nodes.len())
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum();
    anyhow::ensure!(
        twice_area.abs() > 1e-9,
        "user-defined mark has no usable area"
    );
    Ok(())
}

pub fn validate_shape_path(path: &ClosedShapePath) -> anyhow::Result<()> {
    anyhow::ensure!(
        path.anchors.len() >= 3,
        "a user-defined mark needs at least three nodes"
    );
    anyhow::ensure!(
        path.anchors.len() <= 64,
        "a user-defined mark supports at most 64 nodes"
    );
    let nodes: Vec<_> = path.anchors.iter().map(|anchor| anchor.point).collect();
    validate_shape_nodes(&nodes)?;
    anyhow::ensure!(
        path.anchors
            .iter()
            .all(|anchor| [anchor.incoming, anchor.outgoing]
                .into_iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())),
        "user-defined mark contains an invalid handle"
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
    Appearance,
    Treatment,
    Detail,
    Coverage,
    Contrast,
    Angle,
    WebCoverage,
    WebAngle,
    WebMarkAngle,
    WebWidthScale,
    WebHeightScale,
    WebThreshold,
    WebOpacity,
    WebDetail,
    WebShapeSizeResponse,
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
    appearance: DocumentAppearance,
    output_mode: OutputMode,
    artwork_pipeline: ArtworkPipelineSettings,
    pattern_state: PatternDocumentState,
    render: RenderVariant,
    saved_web_shape: Option<Box<WebShapeSettings>>,
    saved_web_shape_pipeline: Option<ArtworkPipelineSettings>,
    saved_web_curve: Option<Box<WebCurveSettings>>,
    saved_web_curve_pipeline: Option<ArtworkPipelineSettings>,
    inactive_cmyk: Option<Box<OutputTreatmentCache>>,
    inactive_rgb: Option<Box<OutputTreatmentCache>>,
}

/// Document-level undo with short edits on the same control coalesced into one drag.
pub struct DocumentEditor {
    document: Document,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    clean_state: TreatmentState,
    active: Option<ActiveEdit>,
    adjusted_on_load_dirty: bool,
}

impl DocumentEditor {
    pub fn new(document: Document) -> Self {
        Self::new_with_load_adjustment(document, false)
    }

    pub fn new_with_load_adjustment(document: Document, adjusted_on_load_dirty: bool) -> Self {
        let clean_state = TreatmentState::from_document(&document);
        Self {
            document,
            undo: Vec::new(),
            redo: Vec::new(),
            clean_state,
            active: None,
            adjusted_on_load_dirty,
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
        self.adjusted_on_load_dirty
            || TreatmentState::from_document(&self.document) != self.clean_state
    }

    pub fn mark_clean(&mut self) {
        self.clean_state = TreatmentState::from_document(&self.document);
        self.adjusted_on_load_dirty = false;
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

    /// Appearance is a document edit, independent from treatment presets and
    /// source interpretation. It deliberately shares the ordinary undo model.
    pub fn set_appearance(&mut self, appearance: DocumentAppearance) -> bool {
        if self.document.appearance == appearance {
            return false;
        }
        let before = TreatmentState::from_document(&self.document);
        self.document.appearance = appearance;
        if self.active.is_none() {
            self.undo.push(Edit {
                before,
                after: TreatmentState::from_document(&self.document),
            });
        }
        self.redo.clear();
        true
    }

    pub fn set_output_mode(&mut self, mode: OutputMode) -> bool {
        if self.document.artwork_pipeline.output_model.to_legacy() == mode {
            return false;
        }
        let before = TreatmentState::from_document(&self.document);
        self.document.switch_output_mode(mode);
        if self.active.is_none() {
            self.undo.push(Edit {
                before,
                after: TreatmentState::from_document(&self.document),
            });
        }
        self.redo.clear();
        true
    }

    /// Applies one validated semantic artwork-pipeline edit.  This is the
    /// Stage 3 UI seam: the pipeline remains authoritative and the legacy
    /// renderer facade is updated only through `sync_legacy_projection`.
    pub fn set_artwork_pipeline(&mut self, pipeline: ArtworkPipelineSettings) -> bool {
        if pipeline.validate().is_err() || self.document.artwork_pipeline == pipeline {
            return false;
        }
        let before = TreatmentState::from_document(&self.document);
        self.document.artwork_pipeline = pipeline;
        if self.document.sync_legacy_projection().is_err() {
            before.apply(&mut self.document);
            return false;
        }
        if self.active.is_none() {
            self.undo.push(Edit {
                before,
                after: TreatmentState::from_document(&self.document),
            });
        }
        self.redo.clear();
        true
    }

    /// Leave the temporary compatibility Crosshatch treatment as ordinary
    /// Curves without changing the selected output model.
    pub fn exit_crosshatch_treatment(&mut self) -> bool {
        if !matches!(
            self.document.artwork_pipeline.assignment,
            ChannelAssignment::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1
            )
        ) {
            return false;
        }
        let before = TreatmentState::from_document(&self.document);
        let output_model = self.document.artwork_pipeline.output_model;
        let (settings, pipeline) = match (
            self.document.saved_web_curve.clone(),
            self.document.saved_web_curve_pipeline.clone(),
        ) {
            (Some(settings), Some(pipeline))
                if pipeline.output_model == output_model
                    && !matches!(
                        pipeline.assignment,
                        ChannelAssignment::LegacyCompatibility(_)
                    ) =>
            {
                (*settings, pipeline)
            }
            _ => (
                WebCurveSettings::default(),
                ArtworkPipelineSettings {
                    source: ArtworkSource::Value,
                    alpha_policy: crate::artwork_pipeline::SourceAlphaPolicy::Preserve,
                    output_model,
                    assignment: ChannelAssignment::AllChannels,
                    active_channel: None,
                },
            ),
        };
        self.document.pattern_state.set_curve_settings(settings);
        self.document
            .pattern_state
            .select_pattern(PatternId::COMPATIBILITY_CURVES_V1)
            .expect("registered Curves state");
        self.document.artwork_pipeline = pipeline;
        if self.document.sync_legacy_projection().is_err() {
            before.apply(&mut self.document);
            return false;
        }
        if self.active.is_none() {
            self.undo.push(Edit {
                before,
                after: TreatmentState::from_document(&self.document),
            });
        }
        self.redo.clear();
        true
    }

    pub fn apply_legacy_mapping_action(&mut self, mapping: ValueMode) -> bool {
        let before = TreatmentState::from_document(&self.document);
        let requested_kind =
            (mapping != ValueMode::CrosshatchLuminance).then(|| render_kind(&self.document.render));
        if mapping == ValueMode::CrosshatchLuminance {
            // Crosshatch is a legacy transition, but its source treatment is
            // still selected and parameterized only by persisted authority.
            // `render` may be a stale or deliberately contradictory adapter.
            match &self.document.pattern_state.selected {
                PatternSelection::Registered(id) if *id == PatternId::COMPATIBILITY_SHAPES_V1 => {
                    let settings = match self.document.pattern_state.shape_settings() {
                        Ok(settings) => settings,
                        Err(_) => return false,
                    };
                    let mut saved = Box::new(settings.clone());
                    saved.value_mode = ValueMode::Luminance;
                    self.document.saved_web_shape = Some(saved);
                    self.document.saved_web_shape_pipeline = Some(ArtworkPipelineSettings {
                        source: ArtworkSource::LegacyBrightness(
                            LegacyBrightnessKind::EncodedRec709InvertedV1,
                        ),
                        alpha_policy: crate::artwork_pipeline::SourceAlphaPolicy::LegacyCurrentV1,
                        output_model: self.document.artwork_pipeline.output_model,
                        assignment: ChannelAssignment::AllChannels,
                        active_channel: None,
                    });
                    self.document
                        .pattern_state
                        .set_curve_settings(WebCurveSettings::crosshatch_from_shape(&settings));
                    self.document
                        .pattern_state
                        .select_pattern(PatternId::COMPATIBILITY_CURVES_V1)
                        .expect("registered Curves state");
                }
                PatternSelection::Registered(id) if *id == PatternId::COMPATIBILITY_CURVES_V1 => {
                    // Keep the ordinary curve treatment intact so Exit can
                    // restore its geometry, visibility, and color settings.
                    let settings = match self.document.pattern_state.curve_settings() {
                        Ok(settings) => settings,
                        Err(_) => return false,
                    };
                    self.document.saved_web_curve = Some(Box::new(settings.clone()));
                    self.document.saved_web_curve_pipeline =
                        Some(self.document.artwork_pipeline.clone());
                    let mut configured = settings;
                    configured.configure_crosshatch();
                    self.document.pattern_state.set_curve_settings(configured);
                    self.document
                        .pattern_state
                        .select_pattern(PatternId::COMPATIBILITY_CURVES_V1)
                        .expect("registered Curves state");
                }
                PatternSelection::NativeBasicV1 | PatternSelection::Registered(_) => return false,
            }
        }
        if self.document.apply_legacy_mapping_action(mapping).is_err() {
            before.apply(&mut self.document);
            return false;
        }
        // A forced legacy Color/RGB mapping transitions the mode cache, but
        // changing a mapping from the Curves inspector must not silently
        // replace the selected treatment with the cached Shapes treatment.
        if matches!(mapping, ValueMode::Cmyk | ValueMode::Rgb)
            && render_kind(&self.document.render) != requested_kind.expect("non-Crosshatch mapping")
        {
            match &self.document.render {
                RenderVariant::WebShapeV1 { settings } => {
                    self.document.saved_web_shape = Some(settings.clone());
                    self.document.saved_web_shape_pipeline =
                        Some(self.document.artwork_pipeline.clone());
                }
                RenderVariant::WebCurveV1 { settings } => {
                    self.document.saved_web_curve = Some(settings.clone());
                    self.document.saved_web_curve_pipeline =
                        Some(self.document.artwork_pipeline.clone());
                }
                RenderVariant::NativeBasicV1 | RenderVariant::WeightedVoronoiCanonicalV1 => {}
            }
            self.document.pattern_state = before.pattern_state.clone();
            let _ = self.document.sync_legacy_projection();
        }
        if TreatmentState::from_document(&self.document) == before {
            return false;
        }
        if self.active.is_none() {
            self.undo.push(Edit {
                before,
                after: TreatmentState::from_document(&self.document),
            });
        }
        self.redo.clear();
        true
    }

    pub fn select_active_output_channel(&mut self, channel: OutputChannelId) -> bool {
        let before = TreatmentState::from_document(&self.document);
        if self.document.select_active_output_channel(channel).is_err() {
            return false;
        }
        if TreatmentState::from_document(&self.document) == before {
            return false;
        }
        if self.active.is_none() {
            self.undo.push(Edit {
                before,
                after: TreatmentState::from_document(&self.document),
            });
        }
        self.redo.clear();
        true
    }

    pub fn set_pattern_state(&mut self, pattern_state: PatternDocumentState) -> bool {
        let before = TreatmentState::from_document(&self.document);
        self.set_pattern_state_from_before(pattern_state, before)
    }

    /// Commits one schema-validated selected output-channel value as one
    /// undoable document edit. Invalid values leave both document and history
    /// unchanged.
    pub fn set_selected_output_channel_parameter(
        &mut self,
        channel: OutputChannelId,
        key: &str,
        value: LiteralValue,
    ) -> bool {
        let mut state = self.document.pattern_state.clone();
        if state
            .set_selected_output_channel_parameter(channel, key, value)
            .is_err()
        {
            return false;
        }
        self.set_pattern_state(state)
    }

    pub fn select_pattern(&mut self, pattern_id: PatternId) -> bool {
        let mut state = self.document.pattern_state.clone();
        if state.select_pattern(pattern_id).is_err() {
            return false;
        }
        self.set_pattern_state(state)
    }

    /// Atomically validates, installs, and selects one project-embedded native
    /// recipe as an ordinary undoable document edit.
    pub fn install_and_select_embedded_pattern(
        &mut self,
        definition: PatternDefinition,
        instance: PatternInstanceParameters,
    ) -> bool {
        let mut state = self.document.pattern_state.clone();
        if state
            .install_and_select_embedded_pattern(definition, instance)
            .is_err()
        {
            return false;
        }
        self.set_pattern_state(state)
    }

    pub fn set_shape_settings(&mut self, settings: WebShapeSettings) -> bool {
        let mut state = self.document.pattern_state.clone();
        state.set_shape_settings(settings);
        self.set_pattern_state(state)
    }

    /// Atomically updates typed Shapes settings and, when active, the
    /// selected embedded recipe's output-channel values as one undoable edit.
    pub fn set_shape_settings_and_sync_embedded(&mut self, settings: WebShapeSettings) -> bool {
        let mut state = self.document.pattern_state.clone();
        state.set_shape_settings(settings.clone());
        if let Err(error) = state.sync_embedded_shapes_from_settings(&settings) {
            eprintln!(
                "[toniator] embedded pattern channel update rejected before validation: {error}"
            );
            return false;
        }
        if let Err(error) = state.validate() {
            eprintln!(
                "[toniator] embedded pattern channel update rejected by pattern validation: {error}"
            );
            return false;
        }
        let applied = self.set_pattern_state(state);
        if !applied {
            eprintln!(
                "[toniator] embedded pattern channel update rejected while committing document state"
            );
        }
        applied
    }

    /// Updates the per-channel Shapes site constructor settings. These values
    /// are deliberately owned by the main channel inspector, while a selected
    /// embedded recipe receives the same typed values in its output-channel
    /// instance so the production recipe runtime remains authoritative.
    pub fn set_shape_channel_distribution(
        &mut self,
        channel: OutputChannelId,
        sampler: WebShapePointSampler,
        random_seed: u64,
        weight_influence: f64,
    ) -> bool {
        if !weight_influence.is_finite() || !(0.001..=16.0).contains(&weight_influence) {
            return false;
        }
        let mut state = self.document.pattern_state.clone();
        let mut settings = match state.shape_settings() {
            Ok(settings) => settings,
            Err(_) => return false,
        };
        let channel_settings = settings.channels.get_mut(channel.to_legacy_ink());
        channel_settings.point_sampler = sampler;
        channel_settings.random_seed = random_seed;
        channel_settings.weight_influence = weight_influence;
        state.set_shape_settings(settings);
        let embedded_pattern_id = state
            .selected_pattern_id()
            .filter(|pattern_id| state.embedded_patterns.contains_key(pattern_id));
        if let Some(pattern_id) = embedded_pattern_id {
            let failed = state
                .set_embedded_output_channel_value(
                    &pattern_id,
                    channel,
                    "point-sampler",
                    LiteralValue::Choice(
                        match sampler {
                            WebShapePointSampler::Grid => "grid",
                            WebShapePointSampler::Uniform => "uniform",
                            WebShapePointSampler::Weighted => "weighted",
                        }
                        .into(),
                    ),
                )
                .is_err()
                || state
                    .set_embedded_output_channel_value(
                        &pattern_id,
                        channel,
                        "channel-seed",
                        LiteralValue::Integer(random_seed),
                    )
                    .is_err()
                || state
                    .set_embedded_output_channel_value(
                        &pattern_id,
                        channel,
                        "channel-weight-influence",
                        LiteralValue::Number(weight_influence),
                    )
                    .is_err();
            if failed {
                return false;
            }
        }
        self.set_pattern_state(state)
    }

    /// Applies one deterministic seed to every channel in the active output
    /// model as one undoable channel-settings edit. Pattern structure remains
    /// untouched; a selected embedded recipe receives the same output-channel
    /// values so its runtime remains authoritative.
    pub fn set_shape_channel_seed_all(&mut self, output_model: OutputModel, seed: u64) -> bool {
        let mut state = self.document.pattern_state.clone();
        let mut settings = match state.shape_settings() {
            Ok(settings) => settings,
            Err(_) => return false,
        };
        for channel in output_model.channels().iter().copied() {
            settings
                .channels
                .get_mut(channel.to_legacy_ink())
                .random_seed = seed;
        }
        state.set_shape_settings(settings);
        let embedded_pattern_id = state
            .selected_pattern_id()
            .filter(|pattern_id| state.embedded_patterns.contains_key(pattern_id));
        if let Some(pattern_id) = embedded_pattern_id {
            for channel in output_model.channels().iter().copied() {
                if state
                    .set_embedded_output_channel_value(
                        &pattern_id,
                        channel,
                        "channel-seed",
                        LiteralValue::Integer(seed),
                    )
                    .is_err()
                {
                    return false;
                }
            }
        }
        self.set_pattern_state(state)
    }

    pub fn set_curve_settings(&mut self, settings: WebCurveSettings) -> bool {
        let mut state = self.document.pattern_state.clone();
        state.set_curve_settings(settings);
        self.set_pattern_state(state)
    }

    pub fn set_weighted_voronoi_settings(&mut self, settings: WeightedVoronoiSettings) -> bool {
        if settings.validate().is_err() {
            return false;
        }
        let mut state = self.document.pattern_state.clone();
        state.set_weighted_voronoi_settings(settings);
        self.set_pattern_state(state)
    }

    #[cfg(test)]
    pub(crate) fn select_pattern_for_test(&mut self, pattern_id: PatternId) -> bool {
        let mut state = self.document.pattern_state.clone();
        if state.select_pattern(pattern_id).is_err() {
            return false;
        }
        if self.document.pattern_state == state {
            return false;
        }
        self.document.pattern_state = state;
        self.document.sync_legacy_projection().is_ok()
    }

    fn set_pattern_state_from_before(
        &mut self,
        pattern_state: PatternDocumentState,
        before: TreatmentState,
    ) -> bool {
        if pattern_state.validate().is_err() || self.document.pattern_state == pattern_state {
            return false;
        }
        let next_render = match pattern_state.adapter() {
            Ok(render) => render,
            Err(_) => return false,
        };
        if render_kind(&self.document.render) != render_kind(&next_render) {
            match &self.document.render {
                RenderVariant::WebShapeV1 { settings } => {
                    self.document.saved_web_shape = Some(settings.clone());
                    self.document.saved_web_shape_pipeline =
                        Some(self.document.artwork_pipeline.clone());
                }
                RenderVariant::WebCurveV1 { settings } => {
                    self.document.saved_web_curve = Some(settings.clone());
                    self.document.saved_web_curve_pipeline =
                        Some(self.document.artwork_pipeline.clone());
                }
                RenderVariant::NativeBasicV1 | RenderVariant::WeightedVoronoiCanonicalV1 => {}
            }
        }
        self.document.pattern_state = pattern_state;
        if self.document.sync_legacy_projection().is_err() {
            before.apply(&mut self.document);
            return false;
        }
        if self.active.is_none() {
            let after = TreatmentState::from_document(&self.document);
            self.undo.push(Edit { before, after });
        }
        self.redo.clear();
        true
    }

    pub fn set_treatment(
        &mut self,
        pattern_state: PatternDocumentState,
        native_settings: Option<Settings>,
    ) -> bool {
        self.begin_edit(SettingKey::Treatment);
        if let Some(settings) = native_settings {
            self.set_settings(SettingKey::Treatment, settings);
        }
        self.set_pattern_state(pattern_state);
        self.end_edit()
    }

    pub fn set_treatment_with_pipeline(
        &mut self,
        pattern_state: PatternDocumentState,
        native_settings: Option<Settings>,
        pipeline: ArtworkPipelineSettings,
    ) -> bool {
        if pipeline.validate().is_err() {
            return false;
        }
        let before = TreatmentState::from_document(&self.document);
        if self.document.artwork_pipeline.output_model != pipeline.output_model {
            self.document
                .switch_output_mode(pipeline.output_model.to_legacy());
        }
        if let Some(settings) = native_settings {
            self.document.settings = settings.sanitized();
        }
        if pattern_state.validate().is_err() {
            return false;
        }
        let next_render = match pattern_state.adapter() {
            Ok(render) => render,
            Err(_) => return false,
        };
        if render_kind(&self.document.render) != render_kind(&next_render) {
            match &self.document.render {
                RenderVariant::WebShapeV1 { settings } => {
                    self.document.saved_web_shape = Some(settings.clone());
                    self.document.saved_web_shape_pipeline =
                        Some(self.document.artwork_pipeline.clone());
                }
                RenderVariant::WebCurveV1 { settings } => {
                    self.document.saved_web_curve = Some(settings.clone());
                    self.document.saved_web_curve_pipeline =
                        Some(self.document.artwork_pipeline.clone());
                }
                RenderVariant::NativeBasicV1 | RenderVariant::WeightedVoronoiCanonicalV1 => {}
            }
        }
        self.document.pattern_state = pattern_state;
        self.document.artwork_pipeline = pipeline;
        if self.document.sync_legacy_projection().is_err() {
            before.apply(&mut self.document);
            return false;
        }
        let after = TreatmentState::from_document(&self.document);
        if before == after {
            return false;
        }
        self.undo.push(Edit { before, after });
        self.redo.clear();
        true
    }

    /// Commit a fully parsed and validated preset candidate as one ordinary
    /// undo edit.  Callers must build the candidate without touching the live
    /// editor; a malformed preset therefore cannot mutate document or history.
    pub fn replace_with_preset_candidate(&mut self, candidate: Document) -> bool {
        if candidate.validate().is_err() {
            return false;
        }
        let before = TreatmentState::from_document(&self.document);
        let after = TreatmentState::from_document(&candidate);
        if before == after {
            return false;
        }
        self.document = candidate;
        self.undo.push(Edit { before, after });
        self.redo.clear();
        true
    }

    pub fn restore_saved_shape(&mut self) -> bool {
        let (Some(settings), Some(pipeline)) = (
            self.document.saved_web_shape.clone(),
            self.document.saved_web_shape_pipeline.clone(),
        ) else {
            return false;
        };
        let mut state = self.document.pattern_state.clone();
        state.set_shape_settings(*settings);
        if state
            .select_pattern(PatternId::COMPATIBILITY_SHAPES_V1)
            .is_err()
        {
            return false;
        }
        self.set_treatment_with_pipeline(state, None, pipeline)
    }

    pub fn restore_saved_curve(&mut self) -> bool {
        let (Some(settings), Some(pipeline)) = (
            self.document.saved_web_curve.clone(),
            self.document.saved_web_curve_pipeline.clone(),
        ) else {
            return false;
        };
        let mut state = self.document.pattern_state.clone();
        state.set_curve_settings(*settings);
        if state
            .select_pattern(PatternId::COMPATIBILITY_CURVES_V1)
            .is_err()
        {
            return false;
        }
        self.set_treatment_with_pipeline(state, None, pipeline)
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
            appearance: document.appearance,
            output_mode: document.output_mode,
            artwork_pipeline: document.artwork_pipeline.clone(),
            pattern_state: document.pattern_state.clone(),
            render: document.render.clone(),
            saved_web_shape: document.saved_web_shape.clone(),
            saved_web_shape_pipeline: document.saved_web_shape_pipeline.clone(),
            saved_web_curve: document.saved_web_curve.clone(),
            saved_web_curve_pipeline: document.saved_web_curve_pipeline.clone(),
            inactive_cmyk: document.inactive_cmyk.clone(),
            inactive_rgb: document.inactive_rgb.clone(),
        }
    }

    fn apply(&self, document: &mut Document) {
        document.settings = self.settings;
        document.appearance = self.appearance;
        document.output_mode = self.output_mode;
        document.artwork_pipeline = self.artwork_pipeline.clone();
        document.pattern_state = self.pattern_state.clone();
        document.render = self.render.clone();
        document.saved_web_shape = self.saved_web_shape.clone();
        document.saved_web_shape_pipeline = self.saved_web_shape_pipeline.clone();
        document.saved_web_curve = self.saved_web_curve.clone();
        document.saved_web_curve_pipeline = self.saved_web_curve_pipeline.clone();
        document.inactive_cmyk = self.inactive_cmyk.clone();
        document.inactive_rgb = self.inactive_rgb.clone();
    }
}

fn render_kind(render: &RenderVariant) -> u8 {
    match render {
        RenderVariant::NativeBasicV1 => 0,
        RenderVariant::WebShapeV1 { .. } => 1,
        RenderVariant::WebCurveV1 { .. } => 2,
        RenderVariant::WeightedVoronoiCanonicalV1 => 3,
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

    fn custom_shapes_recipe() -> (PatternDefinition, PatternInstanceParameters) {
        let mut definition = crate::load_bundled_shapes_definition().unwrap();
        definition.id = PatternId::new("custom.project-dots.v1").unwrap();
        definition.display.name = "Project Dots".into();
        definition.display.summary = "A project-owned Shapes recipe.".into();
        let instance = definition
            .default_instance_parameters(
                OutputChannelId::CMYK
                    .into_iter()
                    .chain(OutputChannelId::RGB),
            )
            .unwrap();
        (definition, instance)
    }

    #[test]
    fn embedded_custom_recipe_is_authoritative_and_undoable() {
        let mut editor = editor();
        let (definition, instance) = custom_shapes_recipe();
        let id = definition.id.clone();

        assert!(editor.install_and_select_embedded_pattern(definition.clone(), instance.clone()));
        assert_eq!(
            editor.document().pattern_state.selected_pattern_id(),
            Some(id.clone())
        );
        assert_eq!(
            editor
                .document()
                .pattern_state
                .selected_embedded_pattern()
                .map(|embedded| &embedded.definition),
            Some(&definition)
        );
        assert_eq!(
            editor
                .document()
                .pattern_state
                .selected_embedded_pattern()
                .map(|embedded| &embedded.instance),
            Some(&instance)
        );
        assert!(editor.undo());
        assert!(
            editor
                .document()
                .pattern_state
                .selected_embedded_pattern()
                .is_none()
        );
        assert!(editor.redo());
        assert_eq!(
            editor.document().pattern_state.selected_pattern_id(),
            Some(id)
        );
    }

    #[test]
    fn custom_selection_without_embedded_definition_fails_validation() {
        let mut state = PatternDocumentState::new();
        state.selected = PatternSelection::Registered(PatternId::new("custom.missing.v1").unwrap());
        assert!(state.validate().is_err());
    }

    #[test]
    fn invalid_embedded_recipe_install_is_inert() {
        let mut editor = editor();
        let before = editor.document().pattern_state.clone();
        let (definition, mut instance) = custom_shapes_recipe();
        instance.pattern_id = PatternId::new("custom.other-id.v1").unwrap();
        assert!(!editor.install_and_select_embedded_pattern(definition, instance));
        assert_eq!(editor.document().pattern_state, before);
        assert!(!editor.can_undo());
    }

    #[test]
    fn document_local_parametric_recipe_commits_and_undoes_as_one_complete_state_change() {
        let mut editor = editor();
        let before = editor.document().pattern_state.clone();
        let mut definition = crate::load_bundled_quadratic_radial_spiral_definition().unwrap();
        let custom_id = PatternId::new("custom.quadratic-radial-spiral.1").unwrap();
        definition.id = custom_id.clone();
        let instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();

        assert!(editor.install_and_select_embedded_pattern(definition.clone(), instance.clone()));
        assert_eq!(
            editor.document().pattern_state.selected_pattern_id(),
            Some(custom_id)
        );
        assert_eq!(
            editor.document().pattern_state.selected_embedded_pattern(),
            Some(&EmbeddedPatternDefinition {
                definition: definition.clone(),
                instance: instance.clone()
            })
        );
        editor.document().validate().unwrap();
        assert!(editor.undo());
        assert_eq!(editor.document().pattern_state, before);
        assert!(editor.redo());
        assert_eq!(
            editor.document().pattern_state.selected_embedded_pattern(),
            Some(&EmbeddedPatternDefinition {
                definition,
                instance
            })
        );
    }

    #[test]
    fn shared_recipe_draft_cancel_leaves_document_history_and_selection_unchanged() {
        let editor = editor();
        let before = editor.document().pattern_state.clone();
        let mut definition = crate::load_bundled_quadratic_radial_spiral_definition().unwrap();
        let custom_id = PatternId::new("custom.cancelled-spiral.1").unwrap();
        definition.id = custom_id.clone();
        let mut instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        instance.pattern_id = custom_id;
        let mut draft =
            crate::SharedRecipeEditorDraft::document_local(definition, instance).unwrap();
        let node_id = draft.definition().recipe.nodes[0].id.clone();
        draft.set_value("turns", LiteralValue::Number(9.5)).unwrap();
        draft
            .set_node_position(&node_id, crate::GraphPosition { x: 14.0, y: -6.0 })
            .unwrap();
        drop(draft);

        assert_eq!(editor.document().pattern_state, before);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn rejected_shared_recipe_apply_retains_draft_for_corrected_retry() {
        let mut editor = editor();
        let before = editor.document().pattern_state.clone();
        let definition = crate::load_bundled_quadratic_radial_spiral_definition().unwrap();
        let instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        let bundled = crate::SharedRecipeEditorDraft::bundled(definition, instance).unwrap();
        let rejected = bundled
            .duplicate_as_document_local(PatternId::QUADRATIC_RADIAL_SPIRAL_V1)
            .unwrap();
        assert!(rejected.validate_for_apply().is_err());
        // The model guard remains the final atomic boundary even if a caller
        // bypasses draft validation.
        assert!(!editor.install_and_select_embedded_pattern(
            rejected.definition().clone(),
            rejected.instance().clone()
        ));
        assert_eq!(editor.document().pattern_state, before);
        assert!(!editor.can_undo());

        let retry = rejected
            .duplicate_as_document_local(PatternId::new("custom.retry-spiral.1").unwrap())
            .unwrap();
        retry.validate_for_apply().unwrap();
        assert!(editor.install_and_select_embedded_pattern(
            retry.definition().clone(),
            retry.instance().clone()
        ));
        assert_eq!(
            editor.document().pattern_state.selected_pattern_id(),
            Some(PatternId::new("custom.retry-spiral.1").unwrap())
        );
    }

    #[test]
    fn new_document_initializes_authoritative_shapes_pattern_state() {
        let document = editor().document().clone();
        assert_eq!(
            document.pattern_state.selected,
            PatternSelection::Registered(PatternId::COMPATIBILITY_SHAPES_V1)
        );
        let record = document
            .pattern_state
            .instances
            .get(&PatternId::COMPATIBILITY_SHAPES_V1)
            .unwrap();
        assert_eq!(record.pattern_id, PatternId::COMPATIBILITY_SHAPES_V1);
        assert_eq!(record.schema_version, 1);
        assert_eq!(record.generator_version, 1);
        assert!(record.values.contains_key("settings"));
        document.validate().unwrap();
    }

    #[test]
    fn bundled_spiral_selection_installs_a_strict_stable_id_instance() {
        let mut editor = editor();
        assert!(editor.select_pattern(PatternId::QUADRATIC_RADIAL_SPIRAL_V1));
        let state = &editor.document().pattern_state;
        assert_eq!(
            state.selected_pattern_id(),
            Some(PatternId::QUADRATIC_RADIAL_SPIRAL_V1)
        );
        let instance = state
            .bundled_definition_instances
            .get(&PatternId::QUADRATIC_RADIAL_SPIRAL_V1)
            .unwrap();
        assert_eq!(instance.pattern_id, PatternId::QUADRATIC_RADIAL_SPIRAL_V1);
        assert_eq!(
            instance.output_channel_values.len(),
            OutputChannelId::CMYK.len() + OutputChannelId::RGB.len()
        );
        let resolved = state.resolve_selected_bundled_pattern().unwrap().unwrap();
        assert_eq!(
            resolved.definition.id,
            PatternId::QUADRATIC_RADIAL_SPIRAL_V1
        );
        assert_eq!(resolved.instance, *instance);
        editor.document().validate().unwrap();
        assert!(editor.undo());
        assert!(editor.redo());
        assert_eq!(
            editor.document().pattern_state.selected_pattern_id(),
            Some(PatternId::QUADRATIC_RADIAL_SPIRAL_V1)
        );
    }

    #[test]
    fn selected_channel_parameter_edits_are_schema_validated_undoable_and_independent() {
        let mut editor = editor();
        assert!(editor.select_pattern(PatternId::WAVE_LINE_FIELD_V1));
        let before = editor.document().pattern_state.clone();
        assert!(editor.set_selected_output_channel_parameter(
            OutputChannelId::CmykCyan,
            "line-width-min",
            LiteralValue::Number(0.8),
        ));
        let resolved = editor
            .document()
            .pattern_state
            .resolve_selected_definition()
            .unwrap()
            .unwrap();
        let cyan = resolved
            .instance
            .output_channel_values
            .iter()
            .find(|values| values.channel == OutputChannelId::CmykCyan.stable_id())
            .unwrap();
        let magenta = resolved
            .instance
            .output_channel_values
            .iter()
            .find(|values| values.channel == OutputChannelId::CmykMagenta.stable_id())
            .unwrap();
        assert_eq!(
            cyan.values
                .iter()
                .find(|value| value.key == "line-width-min")
                .unwrap()
                .value,
            LiteralValue::Number(0.8)
        );
        assert_eq!(
            magenta
                .values
                .iter()
                .find(|value| value.key == "line-width-min")
                .unwrap()
                .value,
            LiteralValue::Number(0.6)
        );
        assert!(!editor.set_selected_output_channel_parameter(
            OutputChannelId::CmykCyan,
            "line-width-min",
            LiteralValue::Number(1.4),
        ));
        assert!(editor.undo());
        assert_eq!(editor.document().pattern_state, before);
        assert!(editor.redo());
        assert_eq!(
            editor
                .document()
                .pattern_state
                .resolve_selected_definition()
                .unwrap()
                .unwrap()
                .instance,
            resolved.instance
        );
    }

    #[test]
    fn bundled_definition_resolution_rejects_unknown_or_contradictory_instances() {
        let mut state = PatternDocumentState::new();
        let definition = crate::load_bundled_quadratic_radial_spiral_definition().unwrap();
        let mut instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        let unknown = PatternId::new("parametric-paths.missing.v1").unwrap();
        instance.pattern_id = unknown.clone();
        state
            .bundled_definition_instances
            .insert(unknown.clone(), instance);
        state.selected = PatternSelection::Registered(unknown);
        assert!(state.validate().is_err());
    }

    #[test]
    fn output_mode_restoration_preserves_authoritative_pattern_values() {
        let mut document = Document::new(SourceArtwork {
            name: "pixel.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from([1]),
        });
        let cmyk_values = document
            .pattern_state
            .instances
            .get(&PatternId::COMPATIBILITY_SHAPES_V1)
            .unwrap()
            .values
            .clone();

        assert!(document.switch_output_mode(OutputMode::RgbScreen));
        assert_eq!(
            document
                .inactive_cmyk
                .as_ref()
                .unwrap()
                .pattern_state
                .instances
                .get(&PatternId::COMPATIBILITY_SHAPES_V1)
                .unwrap()
                .values,
            cmyk_values
        );

        document
            .pattern_state
            .instances
            .get_mut(&PatternId::COMPATIBILITY_SHAPES_V1)
            .unwrap()
            .values
            .get_mut("settings")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("grid_scale".into(), serde_json::json!(2.0));
        assert!(document.switch_output_mode(OutputMode::CmykInks));

        let restored = document
            .pattern_state
            .instances
            .get(&PatternId::COMPATIBILITY_SHAPES_V1)
            .unwrap();
        assert_eq!(restored.pattern_id, PatternId::COMPATIBILITY_SHAPES_V1);
        assert_eq!(restored.values, cmyk_values);
    }

    #[test]
    fn authoritative_pattern_state_overwrites_a_contradictory_transient_adapter() {
        let mut document = editor().document().clone();
        document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(WebCurveSettings::default()),
        };
        document.canonicalize_pipeline_facades().unwrap();
        assert_eq!(
            document.pattern_state.selected,
            PatternSelection::Registered(PatternId::COMPATIBILITY_SHAPES_V1)
        );
        assert!(matches!(document.render, RenderVariant::WebShapeV1 { .. }));
    }

    #[test]
    fn authority_read_accessors_ignore_a_contradictory_transient_adapter() {
        let mut document = editor().document().clone();
        let mut shapes = document.pattern_state.shape_settings().unwrap();
        shapes.grid_scale = 2.75;
        document.pattern_state.set_shape_settings(shapes.clone());

        // This is deliberately contradictory transient execution state. UI
        // reads must continue to use persisted pattern authority until the
        // normal projection seam rebuilds this adapter.
        let mut contradictory_curves = WebCurveSettings::default();
        contradictory_curves.base_channel.curve_scale = 99.0;
        document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(contradictory_curves),
        };

        assert_eq!(
            document.pattern_state.selected_pattern_id(),
            Some(PatternId::COMPATIBILITY_SHAPES_V1)
        );
        assert_eq!(
            document
                .pattern_state
                .selected_metadata()
                .map(|metadata| metadata.id.clone()),
            Some(PatternId::COMPATIBILITY_SHAPES_V1)
        );
        assert_eq!(
            document
                .pattern_state
                .selected_parameters()
                .map(|parameters| parameters.pattern_id.clone()),
            Some(PatternId::COMPATIBILITY_SHAPES_V1)
        );
        assert_eq!(document.pattern_state.shape_settings().unwrap(), shapes);
        assert!(matches!(document.render, RenderVariant::WebCurveV1 { .. }));
    }

    #[test]
    fn crosshatch_transition_uses_authoritative_shapes_not_a_contradictory_curve_adapter() {
        let mut initial = editor();
        let mut shapes = initial.document().pattern_state.shape_settings().unwrap();
        shapes.shared_shape = WebShape::RegularPolygon;
        shapes.polygon_sides = 6;
        shapes.grid_scale = 41.0;
        shapes.base_channel.rotation = 23.0;
        assert!(initial.set_shape_settings(shapes.clone()));

        let mut contradictory = initial.document().clone();
        contradictory.render = RenderVariant::WebCurveV1 {
            settings: Box::new(WebCurveSettings {
                output_width: 17,
                output_height: 13,
                long_edge_cells: 2.0,
                max_mark: 91.0,
                ..Default::default()
            }),
        };
        let mut editor = DocumentEditor::new(contradictory);
        assert!(editor.apply_legacy_mapping_action(ValueMode::CrosshatchLuminance));

        assert_eq!(
            editor.document().pattern_state.selected_pattern_id(),
            Some(PatternId::COMPATIBILITY_CURVES_V1)
        );
        let mut expected_crosshatch = WebCurveSettings::crosshatch_from_shape(&shapes);
        // The legacy adapter's value mode is still projected from the
        // authoritative Crosshatch pipeline after the transition.
        expected_crosshatch.value_mode = ValueMode::Cmyk;
        assert_eq!(
            editor.document().pattern_state.curve_settings().unwrap(),
            expected_crosshatch
        );
        let mut expected_saved = shapes.clone();
        expected_saved.value_mode = ValueMode::Luminance;
        assert_eq!(
            editor.document().saved_web_shape.as_deref(),
            Some(&expected_saved)
        );
        assert!(
            editor.document().saved_web_curve.is_none(),
            "a contradictory Curve adapter cannot choose the transition source"
        );

        assert!(editor.undo());
        assert_eq!(
            editor.document().pattern_state.selected_pattern_id(),
            Some(PatternId::COMPATIBILITY_SHAPES_V1)
        );
        assert_eq!(
            editor.document().pattern_state.shape_settings().unwrap(),
            shapes
        );
        assert!(editor.redo());
        assert_eq!(
            editor.document().pattern_state.curve_settings().unwrap(),
            expected_crosshatch
        );
    }

    #[test]
    fn pattern_selection_and_one_undo_restore_inactive_typed_settings() {
        let mut editor = editor();
        let mut curves = WebCurveSettings::default();
        curves.base_channel.curve_scale = 57.0;
        let mut curve_state = editor.document().pattern_state.clone();
        curve_state
            .select_pattern(PatternId::COMPATIBILITY_CURVES_V1)
            .unwrap();
        curve_state.set_curve_settings(curves.clone());
        assert!(editor.set_pattern_state(curve_state));
        let mut shape_state = editor.document().pattern_state.clone();
        shape_state
            .select_pattern(PatternId::COMPATIBILITY_SHAPES_V1)
            .unwrap();
        assert!(editor.set_pattern_state(shape_state));
        assert!(editor.undo());
        assert!(
            matches!(editor.document().render, RenderVariant::WebCurveV1 { ref settings } if **settings == curves)
        );
        assert_eq!(
            editor.document().pattern_state.selected,
            PatternSelection::Registered(PatternId::COMPATIBILITY_CURVES_V1)
        );
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
    fn semantic_pipeline_edits_are_validated_projected_and_one_undo_entry() {
        let mut editor = editor();
        let original = editor.document().clone();
        let pipeline = ArtworkPipelineSettings {
            source: ArtworkSource::PerceptualLightness,
            alpha_policy: crate::artwork_pipeline::SourceAlphaPolicy::Ignore,
            output_model: OutputModel::CmykPrint,
            assignment: ChannelAssignment::ActiveChannel,
            active_channel: Some(OutputChannelId::CmykBlack),
        };
        assert!(editor.set_artwork_pipeline(pipeline.clone()));
        assert_eq!(editor.document().artwork_pipeline, pipeline);
        let RenderVariant::WebShapeV1 { settings } = &editor.document().render else {
            panic!("fixture is Shapes");
        };
        assert_eq!(settings.value_mode, ValueMode::SingleChannel);
        assert_eq!(settings.single_channel, Ink::Black);
        assert!(editor.undo());
        assert_eq!(editor.document(), &original);
        assert!(editor.redo());
        assert_eq!(
            editor.document().artwork_pipeline.active_channel,
            Some(OutputChannelId::CmykBlack)
        );

        let mut invalid = editor.document().artwork_pipeline.clone();
        invalid.active_channel = Some(OutputChannelId::RgbRed);
        assert!(!editor.set_artwork_pipeline(invalid));
    }

    #[test]
    fn crosshatch_exit_restores_ordinary_curves_without_changing_output_model() {
        let mut editor = editor();
        assert!(editor.set_output_mode(OutputMode::RgbScreen));
        assert!(editor.apply_legacy_mapping_action(ValueMode::CrosshatchLuminance));
        assert_eq!(
            editor.document().artwork_pipeline.output_model,
            OutputModel::RgbScreen
        );
        assert!(editor.exit_crosshatch_treatment());
        assert!(matches!(
            editor.document().render,
            RenderVariant::WebCurveV1 { .. }
        ));
        assert_eq!(
            editor.document().artwork_pipeline.output_model,
            OutputModel::RgbScreen
        );
        assert_eq!(
            editor.document().artwork_pipeline.source,
            ArtworkSource::Value
        );
        assert!(matches!(
            editor.document().artwork_pipeline.assignment,
            ChannelAssignment::AllChannels
        ));
        assert!(editor.undo());
        assert!(matches!(
            editor.document().artwork_pipeline.assignment,
            ChannelAssignment::LegacyCompatibility(_)
        ));
        assert!(editor.redo());
        assert_eq!(
            editor.document().artwork_pipeline.output_model,
            OutputModel::RgbScreen
        );
    }

    #[test]
    fn first_uncached_rgb_transition_preserves_scalar_pipeline_and_restores_cmyk_cache() {
        for (source, alpha_policy) in [
            (
                ArtworkSource::Value,
                crate::artwork_pipeline::SourceAlphaPolicy::Preserve,
            ),
            (
                ArtworkSource::PerceptualLightness,
                crate::artwork_pipeline::SourceAlphaPolicy::Ignore,
            ),
        ] {
            let mut editor = editor();
            let cmyk_pipeline = ArtworkPipelineSettings {
                source,
                alpha_policy,
                output_model: OutputModel::CmykPrint,
                assignment: ChannelAssignment::ActiveChannel,
                active_channel: Some(OutputChannelId::CmykBlack),
            };
            assert!(editor.set_artwork_pipeline(cmyk_pipeline.clone()));
            assert!(editor.set_output_mode(OutputMode::RgbScreen));
            assert_eq!(editor.document().artwork_pipeline.source, source);
            assert_eq!(
                editor.document().artwork_pipeline.alpha_policy,
                alpha_policy
            );
            assert!(matches!(
                editor.document().artwork_pipeline.assignment,
                ChannelAssignment::ActiveChannel
            ));
            assert_eq!(
                editor.document().artwork_pipeline.active_channel,
                Some(OutputChannelId::RgbRed),
                "CMYK Black must never survive as an RGB active channel"
            );
            assert!(editor.set_output_mode(OutputMode::CmykInks));
            assert_eq!(editor.document().artwork_pipeline, cmyk_pipeline);
        }
    }

    #[test]
    fn crosshatch_exit_restores_saved_ordinary_curve_geometry_and_pipeline() {
        fn assert_restored_curve(document: &Document, expected: &WebCurveSettings) {
            let RenderVariant::WebCurveV1 { settings } = &document.render else {
                panic!("ordinary curve should be restored");
            };
            assert_eq!(settings.shared_path, expected.shared_path);
            assert_eq!(
                settings.channels.c.grid_rotation,
                expected.channels.c.grid_rotation
            );
            assert_eq!(settings.channels.c.color, expected.channels.c.color);
            assert_eq!(settings.channels.y.enabled, expected.channels.y.enabled);
            assert_eq!(settings.value_mode, ValueMode::Luminance);
        }

        let mut editor = editor();
        let mut ordinary_curve = WebCurveSettings {
            shared_path: CurvePath::deep_wave(),
            ..Default::default()
        };
        ordinary_curve.channels.c.grid_rotation = 27.0;
        ordinary_curve.channels.c.color = "#123456".into();
        ordinary_curve.channels.y.enabled = false;
        let ordinary_pipeline = ArtworkPipelineSettings {
            source: ArtworkSource::PerceptualLightness,
            alpha_policy: crate::artwork_pipeline::SourceAlphaPolicy::Ignore,
            output_model: OutputModel::CmykPrint,
            assignment: ChannelAssignment::AllChannels,
            active_channel: None,
        };
        assert!(editor.set_artwork_pipeline(ordinary_pipeline.clone()));
        assert!(editor.select_pattern_for_test(PatternId::COMPATIBILITY_CURVES_V1));
        assert!(editor.set_curve_settings(ordinary_curve.clone()));
        assert!(editor.apply_legacy_mapping_action(ValueMode::CrosshatchLuminance));
        assert!(editor.exit_crosshatch_treatment());
        assert_eq!(editor.document().artwork_pipeline, ordinary_pipeline);
        assert_restored_curve(editor.document(), &ordinary_curve);
        assert!(editor.undo());
        assert!(matches!(
            editor.document().artwork_pipeline.assignment,
            ChannelAssignment::LegacyCompatibility(_)
        ));
        assert!(editor.redo());
        assert_restored_curve(editor.document(), &ordinary_curve);
        assert!(editor.apply_legacy_mapping_action(ValueMode::CrosshatchLuminance));
        assert!(editor.exit_crosshatch_treatment());
        assert_restored_curve(editor.document(), &ordinary_curve);
    }

    #[test]
    fn appearance_defaults_and_undo_redo_are_document_edits() {
        let mut editor = editor();
        assert_eq!(editor.document().appearance, DocumentAppearance::default());
        assert!(!editor.is_dirty());
        let appearance = DocumentAppearance {
            preview_surface: PreviewSurface::Checkerboard,
            export_background: ExportBackground::Color {
                color: RgbaColor {
                    red: 18,
                    green: 52,
                    blue: 86,
                    alpha: 127,
                },
            },
        };
        assert!(editor.set_appearance(appearance));
        assert!(editor.is_dirty());
        assert!(editor.can_undo());
        assert!(editor.undo());
        assert_eq!(editor.document().appearance, DocumentAppearance::default());
        assert!(!editor.is_dirty());
        assert!(editor.redo());
        assert_eq!(editor.document().appearance, appearance);
    }

    #[test]
    fn export_background_is_authoritative_and_does_not_change_preview_surface() {
        let mut editor = editor();
        let preview_surface = editor.document().appearance.preview_surface;
        let appearance = DocumentAppearance {
            preview_surface,
            export_background: ExportBackground::Color {
                color: RgbaColor::WHITE,
            },
        };

        assert!(editor.set_appearance(appearance));
        assert_eq!(editor.document().appearance, appearance);
        assert!(editor.undo());
        assert_eq!(
            editor.document().appearance.export_background,
            ExportBackground::None
        );
        assert_eq!(
            editor.document().appearance.preview_surface,
            preview_surface
        );
        assert!(editor.redo());
        assert_eq!(editor.document().appearance, appearance);
    }

    #[test]
    fn output_modes_have_distinct_default_preview_surfaces() {
        let mut editor = editor();
        assert_eq!(
            editor.document().appearance.preview_surface,
            PreviewSurface::Color {
                color: RgbaColor::WHITE,
            }
        );

        assert!(editor.set_output_mode(OutputMode::RgbScreen));
        assert_eq!(
            editor.document().appearance.preview_surface,
            PreviewSurface::Color {
                color: RgbaColor::opaque(0, 0, 0),
            }
        );
    }

    #[test]
    fn output_mode_caches_retain_independent_preview_surfaces() {
        let mut editor = editor();
        let cmyk_surface = PreviewSurface::Color {
            color: RgbaColor::opaque(242, 238, 227),
        };
        let rgb_surface = PreviewSurface::Color {
            color: RgbaColor::opaque(13, 21, 34),
        };
        let export_background = ExportBackground::Color {
            color: RgbaColor::opaque(71, 83, 97),
        };
        editor.document.appearance.preview_surface = cmyk_surface;
        editor.document.appearance.export_background = export_background;

        assert!(editor.set_output_mode(OutputMode::RgbScreen));
        assert_eq!(
            editor.document().appearance.preview_surface,
            PreviewSurface::Color {
                color: RgbaColor::opaque(0, 0, 0),
            }
        );
        editor.document.appearance.preview_surface = rgb_surface;

        assert!(editor.set_output_mode(OutputMode::CmykInks));
        assert_eq!(editor.document().appearance.preview_surface, cmyk_surface);
        assert_eq!(
            editor.document().appearance.export_background,
            export_background
        );

        assert!(editor.set_output_mode(OutputMode::RgbScreen));
        assert_eq!(editor.document().appearance.preview_surface, rgb_surface);
        assert_eq!(
            editor.document().appearance.export_background,
            export_background
        );
    }

    #[test]
    fn output_mode_switch_restores_preview_surface_in_one_undo_redo() {
        let mut editor = editor();
        editor.document.appearance.preview_surface = PreviewSurface::Color {
            color: RgbaColor::opaque(231, 225, 211),
        };
        let original = editor.document().clone();

        assert!(editor.set_output_mode(OutputMode::RgbScreen));
        let switched = editor.document().clone();
        assert_eq!(
            switched.appearance.preview_surface,
            PreviewSurface::Color {
                color: RgbaColor::opaque(0, 0, 0),
            }
        );
        assert!(editor.undo());
        assert_eq!(editor.document(), &original);
        assert!(
            !editor.can_undo(),
            "the output transition is one undo entry"
        );
        assert!(editor.redo());
        assert_eq!(editor.document(), &switched);
    }

    #[test]
    fn rgb_mode_is_lossless_cached_and_one_undoable_edit() {
        let mut editor = editor();
        let original = editor.document().clone();
        assert!(editor.set_output_mode(OutputMode::RgbScreen));
        assert_eq!(editor.document().output_mode, OutputMode::RgbScreen);
        let RenderVariant::WebShapeV1 { settings } = &editor.document().render else {
            panic!("fixture is shapes")
        };
        assert_eq!(settings.value_mode, ValueMode::Rgb);
        assert!(
            Ink::RGB
                .into_iter()
                .all(|ink| settings.channels.get(ink).enabled)
        );
        assert!(editor.undo());
        assert_eq!(editor.document(), &original);
        assert!(editor.redo());
        assert!(editor.set_output_mode(OutputMode::CmykInks));
        assert_eq!(editor.document().render, original.render);
    }

    #[test]
    fn output_transition_uses_authoritative_pipeline_when_projection_is_stale() {
        let mut editor = editor();
        editor.document.artwork_pipeline = editor
            .document
            .artwork_pipeline
            .clone()
            .transition_output_model(OutputModel::RgbScreen, None)
            .expect("valid RGB pipeline transition");
        assert_eq!(editor.document.output_mode, OutputMode::CmykInks);

        assert!(editor.set_output_mode(OutputMode::CmykInks));
        assert_eq!(
            editor.document().artwork_pipeline.output_model,
            OutputModel::CmykPrint
        );
        assert_eq!(editor.document().output_mode, OutputMode::CmykInks);
    }

    #[test]
    fn customized_rgb_shapes_survive_cmyk_roundtrip_without_cross_mode_leakage() {
        let mut editor = editor();
        assert!(editor.set_output_mode(OutputMode::RgbScreen));
        let RenderVariant::WebShapeV1 { settings } = &editor.document().render else {
            panic!("fixture is shapes")
        };
        let mut rgb = (**settings).clone();
        rgb.value_mode = ValueMode::Rgb;
        rgb.use_shared_mark = false;
        rgb.channels.r.enabled = true;
        rgb.channels.g.enabled = false;
        rgb.channels.b.enabled = true;
        rgb.channels.r.opacity = 0.41;
        rgb.channels.b.grid_rotation = 37.0;
        rgb.channels.b.shape = WebShape::RegularPolygon;
        rgb.channels.b.polygon_sides = 6;
        assert!(editor.set_shape_settings(rgb.clone()));
        let rgb_before_switch = editor.document().render.clone();

        assert!(editor.set_output_mode(OutputMode::CmykInks));
        let cmyk_before_switch_back = editor.document().render.clone();
        assert!(editor.set_output_mode(OutputMode::RgbScreen));
        assert_eq!(editor.document().render, rgb_before_switch);
        assert_ne!(editor.document().render, cmyk_before_switch_back);
        let RenderVariant::WebShapeV1 { settings } = &editor.document().render else {
            panic!("RGB mode must restore Shapes")
        };
        assert_eq!(settings.channels.r.opacity, 0.41);
        assert!(!settings.channels.g.enabled);
        assert_eq!(settings.channels.b.grid_rotation, 37.0);
        assert_eq!(settings.channels.b.polygon_sides, 6);
    }

    #[test]
    fn generic_render_edits_cannot_infer_pipeline_output_mode() {
        let mut editor = editor();
        let original = editor.document().clone();
        let rgb = WebShapeSettings {
            value_mode: ValueMode::Rgb,
            ..Default::default()
        };
        assert!(!editor.set_shape_settings(rgb));
        assert_eq!(editor.document().output_mode, OutputMode::CmykInks);
        assert!(editor.apply_legacy_mapping_action(ValueMode::Rgb));
        assert_eq!(editor.document().output_mode, OutputMode::RgbScreen);
        assert!(editor.undo());
        assert_eq!(editor.document(), &original);
        assert!(editor.redo());

        let neutral = WebShapeSettings {
            value_mode: ValueMode::Luminance,
            ..Default::default()
        };
        assert!(!editor.set_shape_settings(neutral));
        assert_eq!(editor.document().output_mode, OutputMode::RgbScreen);

        let cmyk = WebShapeSettings {
            value_mode: ValueMode::Cmyk,
            ..Default::default()
        };
        assert!(!editor.set_shape_settings(cmyk));
        assert_eq!(editor.document().output_mode, OutputMode::RgbScreen);
        assert!(editor.apply_legacy_mapping_action(ValueMode::Cmyk));
        assert_eq!(editor.document().output_mode, OutputMode::CmykInks);
    }

    #[test]
    fn treatment_application_preserves_authoritative_output_and_appearance() {
        let mut editor = editor();
        editor.set_output_mode(OutputMode::RgbScreen);
        let appearance = editor.document().appearance;
        assert!(!editor.set_treatment(editor.document().pattern_state.clone(), None));
        assert_eq!(editor.document().appearance, appearance);
        assert_eq!(editor.document().output_mode, OutputMode::RgbScreen);
        let RenderVariant::WebShapeV1 { settings } = &editor.document().render else {
            panic!("shapes")
        };
        assert_eq!(settings.value_mode, ValueMode::Rgb);
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
        let mut native_state = editor.document().pattern_state.clone();
        native_state.select_native_basic();
        assert!(editor.set_pattern_state(native_state));
        editor.mark_clean();
        let mut shape_state = editor.document().pattern_state.clone();
        shape_state
            .select_pattern(PatternId::COMPATIBILITY_SHAPES_V1)
            .unwrap();
        assert!(editor.set_pattern_state(shape_state));
        assert!(matches!(
            editor.document().render,
            RenderVariant::WebShapeV1 { .. }
        ));
        assert!(editor.is_dirty());
        assert!(editor.undo());
        assert_eq!(editor.document().render, RenderVariant::NativeBasicV1);
        assert!(!editor.is_dirty());
        assert!(editor.redo());
        assert!(matches!(
            editor.document().render,
            RenderVariant::WebShapeV1 { .. }
        ));
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
    fn shape_channel_distribution_is_undoable_and_scoped() {
        let mut editor = editor();
        assert!(editor.set_shape_channel_distribution(
            OutputChannelId::CmykBlack,
            WebShapePointSampler::Weighted,
            77,
            2.5,
        ));
        let settings = editor.document().pattern_state.shape_settings().unwrap();
        assert_eq!(
            settings.channels.k.point_sampler,
            WebShapePointSampler::Weighted
        );
        assert_eq!(settings.channels.k.random_seed, 77);
        assert_eq!(settings.channels.k.weight_influence, 2.5);
        assert_eq!(
            settings.channels.c.point_sampler,
            WebShapePointSampler::Grid
        );
        assert!(editor.undo());
        let restored = editor.document().pattern_state.shape_settings().unwrap();
        assert_eq!(
            restored.channels.k.point_sampler,
            WebShapePointSampler::Grid
        );
        assert_eq!(restored.channels.k.random_seed, 0);
        assert_eq!(restored.channels.k.weight_influence, 1.0);
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
        document
            .pattern_state
            .set_selected_parameters_for_test(&document.render);
        assert!(document.validate().is_err());

        settings.channels.c.opacity = 1.0;
        settings.channels.m.threshold = -0.01;
        document.render = RenderVariant::WebShapeV1 {
            settings: Box::new(settings.clone()),
        };
        document
            .pattern_state
            .set_selected_parameters_for_test(&document.render);
        assert!(document.validate().is_err());

        settings.channels.m.threshold = 0.0;
        settings.grid_scale = 0.0;
        document.render = RenderVariant::WebShapeV1 {
            settings: Box::new(settings),
        };
        document
            .pattern_state
            .set_selected_parameters_for_test(&document.render);
        assert!(document.validate().is_err());
    }

    #[test]
    fn user_defined_shape_requires_finite_nondegenerate_polygon() {
        assert!(validate_shape_nodes(&default_shape_nodes()).is_ok());
        assert!(validate_shape_nodes(&[ShapePoint { x: 0.0, y: 0.0 }; 3]).is_err());
        assert!(
            validate_shape_nodes(&[
                ShapePoint { x: 0.0, y: 0.0 },
                ShapePoint { x: 1.0, y: 0.0 },
                ShapePoint {
                    x: f64::NAN,
                    y: 1.0
                },
            ])
            .is_err()
        );
    }

    #[test]
    fn legacy_shape_nodes_resolve_to_exact_straight_cubics_and_roundtrip() {
        let settings = WebShapeSettings::default();
        assert!(settings.custom_shape_path.is_none());
        let path = settings.resolved_custom_shape_path();
        assert_eq!(path.anchors.len(), 4);
        assert!((path.anchors[0].outgoing.x + 0.15).abs() < 1e-12);
        assert_eq!(path.anchors[0].outgoing.y, -0.45);
        assert!((path.anchors[1].incoming.x - 0.15).abs() < 1e-12);
        validate_shape_path(&path).unwrap();
        let encoded = serde_json::to_vec(&path).unwrap();
        assert_eq!(
            serde_json::from_slice::<ClosedShapePath>(&encoded).unwrap(),
            path
        );
    }

    #[test]
    fn nonstraight_custom_path_is_atomic_undoable_and_cancel_candidate_is_local() {
        let mut editor = editor();
        let before = editor.document().clone();
        let mut settings = WebShapeSettings {
            shared_shape: WebShape::UserDefined,
            ..Default::default()
        };
        let mut path = settings.resolved_custom_shape_path();
        path.anchors[0].outgoing.y -= 0.22;
        path.anchors[1].incoming.x -= 0.11;
        settings.custom_shape_path = Some(path.clone());
        let expected = RenderVariant::WebShapeV1 {
            settings: Box::new(settings),
        };
        let RenderVariant::WebShapeV1 { settings } = &expected else {
            unreachable!()
        };
        assert!(editor.set_shape_settings((**settings).clone()));
        assert_eq!(editor.document().render, expected);
        assert!(editor.undo());
        assert_eq!(editor.document(), &before);
        assert!(editor.redo());
        let committed = editor.document().clone();
        let mut dialog_candidate = path;
        dialog_candidate.anchors[0].outgoing.x += 0.4;
        // Cancel/Escape drops the dialog-local candidate; the document is untouched.
        drop(dialog_candidate);
        assert_eq!(editor.document(), &committed);
    }

    #[test]
    fn switching_treatments_preserves_inactive_curve_and_is_undoable() {
        let mut editor = editor();
        let curve = WebCurveSettings {
            shared_path: CurvePath::deep_wave(),
            ..Default::default()
        };
        let mut curve_state = editor.document().pattern_state.clone();
        curve_state
            .select_pattern(PatternId::COMPATIBILITY_CURVES_V1)
            .unwrap();
        curve_state.set_curve_settings(curve.clone());
        assert!(editor.set_pattern_state(curve_state));
        let mut native_state = editor.document().pattern_state.clone();
        native_state.select_native_basic();
        assert!(editor.set_pattern_state(native_state));
        assert_eq!(editor.document().saved_web_curve.as_deref(), Some(&curve));
        assert!(editor.undo());
        assert!(
            matches!(editor.document().render, RenderVariant::WebCurveV1 { ref settings } if **settings == curve)
        );
        assert!(editor.redo());
        assert_eq!(editor.document().render, RenderVariant::NativeBasicV1);
        assert_eq!(editor.document().saved_web_curve.as_deref(), Some(&curve));
    }

    #[test]
    fn saved_treatment_pipeline_snapshots_restore_atomically_across_mode_sequences() {
        let mut editor = editor();
        // CMYK Shapes -> RGB Shapes -> CMYK Shapes
        assert!(editor.apply_legacy_mapping_action(ValueMode::Luminance));
        assert!(editor.set_output_mode(OutputMode::RgbScreen));
        assert!(editor.select_pattern(PatternId::COMPATIBILITY_CURVES_V1));
        let saved_rgb_shape_pipeline = editor.document().saved_web_shape_pipeline.clone().unwrap();
        assert!(editor.restore_saved_shape());
        assert_eq!(editor.document().artwork_pipeline, saved_rgb_shape_pipeline);

        // RGB Shapes -> CMYK Curves -> RGB Shapes and CMYK Curves -> RGB Curves -> CMYK Curves.
        assert!(editor.set_output_mode(OutputMode::CmykInks));
        assert!(editor.select_pattern(PatternId::COMPATIBILITY_CURVES_V1));
        let cmyk_curve_pipeline = editor.document().artwork_pipeline.clone();
        let saved_shape_pipeline = editor.document().saved_web_shape_pipeline.clone().unwrap();
        assert!(editor.restore_saved_shape());
        assert_eq!(editor.document().artwork_pipeline, saved_shape_pipeline);
        assert!(editor.restore_saved_curve());
        assert_eq!(editor.document().artwork_pipeline, cmyk_curve_pipeline);

        // Crosshatch -> ordinary Curves -> Crosshatch keeps the compatibility state.
        assert!(editor.apply_legacy_mapping_action(ValueMode::CrosshatchLuminance));
        let hatch = editor.document().artwork_pipeline.clone();
        assert!(editor.restore_saved_shape());
        assert!(editor.restore_saved_curve());
        assert_eq!(editor.document().artwork_pipeline, hatch);
        assert!(editor.undo());
        assert!(editor.redo());
        assert_eq!(editor.document().artwork_pipeline, hatch);
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
        document
            .pattern_state
            .select_pattern(PatternId::COMPATIBILITY_CURVES_V1)
            .unwrap();
        document
            .pattern_state
            .set_selected_parameters_for_test(&document.render);
        let mut editor = DocumentEditor::new(document);
        editor.begin_edit(SettingKey::CurvePath);
        for y in [-0.2, -0.3, -0.35] {
            let mut changed = original.clone();
            changed.shared_path.segments[0].control_1.y = y;
            editor.set_curve_settings(changed);
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
        document
            .pattern_state
            .select_pattern(PatternId::COMPATIBILITY_CURVES_V1)
            .unwrap();
        document
            .pattern_state
            .set_selected_parameters_for_test(&document.render);
        let mut editor = DocumentEditor::new(document);
        editor.begin_edit(SettingKey::CurvePositionX);
        let mut changed = original.clone();
        changed.channels.c.offset_x = 75.0;
        assert!(editor.set_curve_settings(changed));
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
        let appearance = DocumentAppearance {
            preview_surface: PreviewSurface::Checkerboard,
            export_background: ExportBackground::Color {
                color: RgbaColor::opaque(12, 34, 56),
            },
        };
        assert!(editor.set_appearance(appearance));
        editor.mark_clean();
        let settings = Settings {
            treatment: Treatment::Lines,
            detail: 81.0,
            coverage: 119.0,
            contrast: 136.0,
            angle: -12.0,
        };
        let mut pattern_state = editor.document().pattern_state.clone();
        pattern_state.select_native_basic();
        assert!(editor.set_treatment(pattern_state, Some(settings)));
        assert_eq!(editor.document().settings, settings);
        assert_eq!(
            editor.document().appearance,
            appearance,
            "treatment/preset application must not alter appearance"
        );
        assert!(editor.undo());
        assert_eq!(editor.document().appearance, appearance);
        assert_eq!(editor.document().settings, original.settings);
        assert!(
            editor.can_undo(),
            "the earlier appearance edit remains independently undoable"
        );
    }

    #[test]
    fn weighted_voronoi_state_is_authoritative_undoable_and_rejects_old_generator_versions() {
        let mut editor = editor();
        assert!(editor.select_pattern(PatternId::WEIGHTED_VORONOI_V1));
        let mut settings = editor
            .document()
            .pattern_state
            .weighted_voronoi_settings()
            .unwrap();
        settings
            .channel_settings_mut(OutputChannelId::CmykCyan)
            .unwrap()
            .cell_count = 32;
        assert!(editor.set_weighted_voronoi_settings(settings.clone()));
        assert_eq!(
            editor
                .document()
                .pattern_state
                .weighted_voronoi_settings()
                .unwrap(),
            settings
        );
        assert!(editor.undo());
        assert_eq!(
            editor
                .document()
                .pattern_state
                .weighted_voronoi_settings()
                .unwrap(),
            WeightedVoronoiSettings::default()
        );
        assert!(editor.redo());
        let mut obsolete = editor.document().clone();
        obsolete
            .pattern_state
            .instances
            .get_mut(&PatternId::WEIGHTED_VORONOI_V1)
            .unwrap()
            .generator_version = 1;
        assert!(obsolete.validate().is_err());
    }

    #[test]
    fn current_pipeline_preset_applies_as_one_undo_edit() {
        let mut editor = editor();
        let original = editor.document().clone();
        let pipeline = ArtworkPipelineSettings {
            source: ArtworkSource::FullColor,
            alpha_policy: crate::artwork_pipeline::SourceAlphaPolicy::LegacyCurrentV1,
            output_model: OutputModel::RgbScreen,
            assignment: ChannelAssignment::automatic(
                AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1,
            ),
            active_channel: Some(OutputChannelId::RgbGreen),
        };
        assert!(editor.set_treatment_with_pipeline(
            editor.document().pattern_state.clone(),
            None,
            pipeline.clone(),
        ));
        assert_eq!(editor.document().artwork_pipeline, pipeline);
        assert!(
            editor.document().inactive_cmyk.is_some(),
            "preset output changes must preserve the outgoing treatment cache"
        );
        assert!(editor.undo());
        assert_eq!(editor.document(), &original);
        assert!(
            !editor.can_undo(),
            "preset application must create one undo step"
        );
        assert!(editor.redo());
        assert_eq!(editor.document().artwork_pipeline, pipeline);
    }

    #[test]
    fn curve_base_effective_values_roundtrip_and_undo_exactly() {
        let mut curve = WebCurveSettings::default();
        curve.base_channel.scale = 1.25;
        curve.base_channel.curve_scale = 48.0;
        curve.channels.c.scale = 1.05;
        curve.channels.m.scale = 1.45;
        curve.channels.c.curve_scale = 44.0;
        curve.channels.m.curve_scale = 53.0;

        let mut preset_document = Document::new(SourceArtwork {
            name: "curve-preset".into(),
            media_type: "application/octet-stream".into(),
            bytes: Arc::from([1]),
        });
        preset_document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(curve.clone()),
        };
        preset_document
            .pattern_state
            .select_pattern(PatternId::COMPATIBILITY_CURVES_V1)
            .unwrap();
        preset_document
            .pattern_state
            .set_selected_parameters_for_test(&preset_document.render);
        let bytes = crate::preset::document_preset_bytes(
            "Curve Base",
            &preset_document,
            crate::preset::PresetScope::CompleteWorkflow,
        )
        .unwrap();
        let parsed = crate::preset::parse_treatment(&bytes, (900, 620)).unwrap();
        let applied = parsed
            .candidate_for(&Document::new(SourceArtwork {
                name: "candidate".into(),
                media_type: "application/octet-stream".into(),
                bytes: Arc::from([1]),
            }))
            .unwrap();
        assert_eq!(
            applied.render,
            RenderVariant::WebCurveV1 {
                settings: Box::new(curve.clone())
            }
        );

        let mut editor = editor();
        assert!(
            editor.replace_with_preset_candidate(parsed.candidate_for(editor.document()).unwrap())
        );
        let before_shift = editor.document().render.clone();
        let mut shifted = curve.clone();
        let delta = 0.2;
        shifted.base_channel.scale += delta;
        for ink in Ink::ALL {
            shifted.channels.get_mut(ink).scale += delta;
        }
        assert!(editor.set_curve_settings(shifted));
        assert!(editor.undo());
        assert_eq!(editor.document().render, before_shift);
    }

    #[test]
    fn motif_base_shift_and_individual_edit_have_distinct_ownership() {
        let mut curve = WebCurveSettings::default();
        curve.base_channel.curve_scale = 40.0;
        curve.channels.c.curve_scale = 36.0;
        curve.channels.m.curve_scale = 45.0;
        let delta = 6.0;
        curve.base_channel.curve_scale += delta;
        for ink in Ink::ALL {
            curve.channels.get_mut(ink).curve_scale += delta;
        }
        assert_eq!(curve.channels.c.curve_scale, 42.0);
        assert_eq!(curve.channels.m.curve_scale, 51.0);
        let before = curve.clone();
        curve.channels.y.stack_spacing = 72.0;
        assert_eq!(curve.channels.c, before.channels.c);
        assert_eq!(curve.channels.m, before.channels.m);
        assert_eq!(curve.channels.k, before.channels.k);
        assert_eq!(curve.base_channel, before.base_channel);
        assert_eq!(curve.channels.y.stack_spacing, 72.0);
    }

    #[test]
    fn document_validation_rejects_invalid_shape_and_curve_bases() {
        let mut shape_document = editor().document().clone();
        let RenderVariant::WebShapeV1 { settings } = &mut shape_document.render else {
            panic!("expected shape render");
        };
        settings.base_channel.opacity = f64::NAN;
        assert!(shape_document.validate().is_err());

        let mut curve_document = editor().document().clone();
        curve_document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(WebCurveSettings::default()),
        };
        curve_document
            .pattern_state
            .select_pattern(PatternId::COMPATIBILITY_CURVES_V1)
            .unwrap();
        curve_document
            .pattern_state
            .set_selected_parameters_for_test(&curve_document.render);
        let RenderVariant::WebCurveV1 { settings } = &mut curve_document.render else {
            panic!("expected curve render");
        };
        settings.base_channel.stack_count = 0;
        assert!(curve_document.validate().is_err());
    }

    #[test]
    fn canvas_aspect_normalization_covers_active_saved_wide_tall_rounding_and_cap() {
        assert_eq!(aspect_locked_dimensions(16, 9, 901), (901, 507));
        assert_eq!(aspect_locked_dimensions(9, 16, 901), (507, 901));
        assert_eq!(aspect_locked_dimensions(1, 1000, 200_000), (100, 100_000));

        let mut document = editor().document().clone();
        let curve = WebCurveSettings {
            output_width: 777,
            output_height: 333,
            ..Default::default()
        };
        document.saved_web_curve = Some(Box::new(curve));
        document.saved_web_shape = Some(Box::new(WebShapeSettings::default()));
        assert!(document.normalize_canvas_aspect(16, 9));
        let RenderVariant::WebShapeV1 { settings } = &document.render else {
            panic!()
        };
        assert_eq!((settings.output_width, settings.output_height), (900, 506));
        assert_eq!(
            (
                document.saved_web_shape.as_ref().unwrap().output_width,
                document.saved_web_shape.as_ref().unwrap().output_height
            ),
            (900, 506)
        );
        assert_eq!(
            (
                document.saved_web_curve.as_ref().unwrap().output_width,
                document.saved_web_curve.as_ref().unwrap().output_height
            ),
            (777, 437)
        );
        assert!(!document.normalize_canvas_aspect(16, 9));
    }

    #[test]
    fn crosshatch_mapping_from_rgb_shapes_installs_curves_in_one_edit() {
        let mut editor = editor();
        assert!(editor.apply_legacy_mapping_action(ValueMode::Rgb));
        assert!(editor.apply_legacy_mapping_action(ValueMode::CrosshatchLuminance));
        assert!(matches!(
            editor.document().render,
            RenderVariant::WebCurveV1 { .. }
        ));
        assert!(matches!(
            editor.document().artwork_pipeline.assignment,
            ChannelAssignment::LegacyCompatibility(_)
        ));
        assert!(editor.undo());
        assert!(matches!(
            editor.document().render,
            RenderVariant::WebShapeV1 { .. }
        ));
    }

    #[test]
    fn load_adjustment_dirty_flag_survives_edits_and_clears_only_on_save_baseline() {
        let document = editor().document().clone();
        let mut editor = DocumentEditor::new_with_load_adjustment(document, true);
        assert!(editor.is_dirty());
        let mut settings = editor.document().settings;
        settings.coverage += 1.0;
        assert!(editor.set_settings(SettingKey::Coverage, settings));
        assert!(editor.undo());
        assert!(editor.is_dirty(), "undo must not hide an unsaved migration");
        editor.mark_clean();
        assert!(!editor.is_dirty());
    }

    #[test]
    fn value_mode_is_a_transient_adapter_projection_and_rejects_removed_inverse() {
        assert_eq!(
            serde_json::to_string(&ValueMode::Luminance).unwrap(),
            "\"luminance\""
        );
        assert!(serde_json::from_str::<ValueMode>("\"inverted-luminance\"").is_err());

        let mut document = editor().document().clone();
        let RenderVariant::WebShapeV1 { settings } = &mut document.render else {
            panic!()
        };
        settings.value_mode = ValueMode::Luminance;
        document
            .pattern_state
            .set_selected_parameters_for_test(&document.render);
        let json = serde_json::to_string(&document).unwrap();
        assert!(!json.contains("\"value_mode\""));
        assert!(!json.contains("\"render\""));
        assert!(!json.contains("inverted-luminance"));
        let decoded: Document = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded.render, RenderVariant::NativeBasicV1));
        let projected = decoded.projected_for_render().unwrap();
        assert!(
            matches!(projected.render, RenderVariant::WebShapeV1 { settings } if settings.value_mode == ValueMode::Cmyk)
        );
    }
}
