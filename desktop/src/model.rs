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
    RegularPolygon,
    UserDefined,
    // Accepted when importing useful legacy web presets; not exposed for new work.
    Rectangle,
    Triangle,
    Pentagon,
    Hexagon,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShapePoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShapeAnchor {
    pub point: ShapePoint,
    pub incoming: ShapePoint,
    pub outgoing: ShapePoint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
            render: RenderVariant::WebShapeV1 {
                settings: Box::new(WebShapeSettings::default()),
            },
            saved_web_shape: None,
            saved_web_curve: None,
        }
    }

    pub fn new_with_artboard(source: SourceArtwork, width: u32, height: u32) -> Self {
        let mut document = Self::new(source);
        if let RenderVariant::WebShapeV1 { settings } = &mut document.render {
            settings.output_width = width.max(1);
            settings.output_height = height.max(1);
        }
        document
    }

    /// Makes every stored canvas use the decoded source aspect ratio. The
    /// requested long edge is preserved (subject to the document cap), so old
    /// presets retain their intended scale without retaining distortion.
    pub fn normalize_canvas_aspect(&mut self, source_width: u32, source_height: u32) -> bool {
        let mut changed =
            normalize_render_variant_canvas(&mut self.render, source_width, source_height);
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
        changed
    }

    /// Converts the legacy shape-based "crosshatch" approximation into the
    /// native curve treatment so desktop documents never render dot layers
    /// under a crosshatch label.
    pub fn normalize_crosshatch_treatment(&mut self) -> bool {
        let mut changed = normalize_crosshatch_render(&mut self.render);
        if let Some(settings) = self.saved_web_shape.as_deref_mut()
            && settings.value_mode == ValueMode::CrosshatchLuminance
        {
            settings.value_mode = ValueMode::Luminance;
            changed = true;
        }
        changed
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
                    && (0.0..=1.0).contains(&base.opacity),
                "web shape base value is outside the supported range"
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
                        channel.width_scale,
                        channel.height_scale,
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
                        && (0.0..=1.0).contains(&channel.opacity),
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
        RenderVariant::NativeBasicV1 => false,
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

fn normalize_canvas_dimensions(
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
    migrated_dirty: bool,
}

impl DocumentEditor {
    pub fn new(document: Document) -> Self {
        Self::new_with_migration(document, false)
    }

    pub fn new_with_migration(document: Document, migrated_dirty: bool) -> Self {
        let clean_state = TreatmentState::from_document(&document);
        Self {
            document,
            undo: Vec::new(),
            redo: Vec::new(),
            clean_state,
            active: None,
            migrated_dirty,
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
        self.migrated_dirty || TreatmentState::from_document(&self.document) != self.clean_state
    }

    pub fn mark_clean(&mut self) {
        self.clean_state = TreatmentState::from_document(&self.document);
        self.migrated_dirty = false;
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

    pub fn convert_shape_to_crosshatch(&mut self) -> bool {
        let RenderVariant::WebShapeV1 { settings } = &self.document.render else {
            return false;
        };
        let curve = WebCurveSettings::crosshatch_from_shape(settings);
        let mut saved_shape = settings.clone();
        saved_shape.value_mode = ValueMode::Luminance;
        self.begin_edit(SettingKey::Treatment);
        self.document.saved_web_shape = Some(saved_shape);
        self.document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(curve),
        };
        self.redo.clear();
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
        assert!(editor.set_render_variant(RenderVariant::NativeBasicV1));
        editor.mark_clean();
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
        assert!(editor.set_render_variant(expected.clone()));
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

    #[test]
    fn curve_base_effective_values_roundtrip_and_undo_exactly() {
        let mut curve = WebCurveSettings::default();
        curve.base_channel.scale = 1.25;
        curve.base_channel.curve_scale = 48.0;
        curve.channels.c.scale = 1.05;
        curve.channels.m.scale = 1.45;
        curve.channels.c.curve_scale = 44.0;
        curve.channels.m.curve_scale = 53.0;

        let bytes = crate::preset::treatment_preset_bytes(
            "Curve Base",
            &RenderVariant::WebCurveV1 {
                settings: Box::new(curve.clone()),
            },
        )
        .unwrap();
        let parsed = crate::preset::parse_treatment(&bytes, (900, 620)).unwrap();
        assert_eq!(
            parsed.render,
            RenderVariant::WebCurveV1 {
                settings: Box::new(curve.clone())
            }
        );

        let mut editor = editor();
        assert!(editor.set_render_variant(parsed.render));
        let before_shift = editor.document().render.clone();
        let mut shifted = curve.clone();
        let delta = 0.2;
        shifted.base_channel.scale += delta;
        for ink in Ink::ALL {
            shifted.channels.get_mut(ink).scale += delta;
        }
        assert!(editor.set_render_variant(RenderVariant::WebCurveV1 {
            settings: Box::new(shifted),
        }));
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
    fn shape_crosshatch_becomes_atomic_editable_curve_treatment() {
        let mut editor = editor();
        let RenderVariant::WebShapeV1 { settings } = &mut editor.document.render else {
            panic!()
        };
        settings.value_mode = ValueMode::CrosshatchLuminance;
        settings.crosshatch_color = "#234567".into();
        settings.channels.c.color = "#abcdef".into();

        assert!(editor.convert_shape_to_crosshatch());
        let RenderVariant::WebCurveV1 { settings } = &editor.document().render else {
            panic!("crosshatch must visibly switch to Curves")
        };
        assert_eq!(settings.value_mode, ValueMode::CrosshatchLuminance);
        assert_eq!(settings.layout, CurveLayout::FullWidth);
        assert_eq!(settings.shared_path, CurvePath::straight());
        assert_eq!(settings.crosshatch_color, "#234567");
        assert_eq!(settings.channels.c.color, "#abcdef");
        assert_eq!(settings.channels.k.grid_rotation, 45.0);
        assert_eq!(settings.channels.c.grid_rotation, -45.0);
        assert_eq!(settings.channels.m.grid_rotation, 0.0);
        assert_eq!(settings.channels.y.grid_rotation, 90.0);
        assert!(
            Ink::ALL
                .into_iter()
                .all(|ink| settings.channels.get(ink).enabled)
        );
        assert!(editor.undo());
        assert!(matches!(
            editor.document().render,
            RenderVariant::WebShapeV1 { .. }
        ));
        assert!(editor.redo());
        assert!(matches!(
            editor.document().render,
            RenderVariant::WebCurveV1 { .. }
        ));
    }

    #[test]
    fn migrated_dirty_flag_survives_edits_and_clears_only_on_save_baseline() {
        let document = editor().document().clone();
        let mut editor = DocumentEditor::new_with_migration(document, true);
        assert!(editor.is_dirty());
        let mut settings = editor.document().settings;
        settings.coverage += 1.0;
        assert!(editor.set_settings(SettingKey::Coverage, settings));
        assert!(editor.undo());
        assert!(editor.is_dirty(), "undo must not hide an unsaved migration");
        editor.mark_clean();
        assert!(!editor.is_dirty());
    }
}
