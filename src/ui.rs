use crate::CliOptions;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use image::RgbaImage;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, Once};
use std::time::{Duration, Instant};
#[cfg(test)]
use toniator::RenderVariant;
use toniator::artwork_pipeline::{
    ArtworkPipelineSettings, ArtworkSource, AutomaticSeparationStrategy, ChannelAssignment,
    LegacyCompatibilityAssignment, OutputChannelId, OutputModel, SourceAlphaPolicy,
};
use toniator::model::{
    ClosedShapePath, SettingKey, ShapeAnchor, ShapePoint, SourceArtwork,
    WeightedVoronoiArrangementPolicy, WeightedVoronoiPlacementMode, WeightedVoronoiSettings,
};
use toniator::pattern::{PATTERN_REGISTRY, PatternId, PatternInspectorPanel};
use toniator::persistence::{clear_recovery_if_matches, recovery_path};
#[cfg(test)]
use toniator::render_document_preview;
use toniator::{
    AlternateTileTransform, CancellationToken, CurveLayout, CurvePath, CurvePoint,
    DistributionLimits, Document, DocumentAppearance, DocumentEditor, ExportBackground, Ink,
    MotifCoverage, OperationCancelled, OutputMode, PreviewSurface, RenderGate, RgbaColor, Settings,
    Treatment, ValueMode, WebCurveChannel, WebCurveSettings, WebShape, WebShapeSettings,
    export_svg, export_svg_cancellable, render_document_preview_cancellable, save_document_atomic,
};

const EXAMPLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="960" height="680" viewBox="0 0 960 680">
<defs><linearGradient id="warm" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ffcf33"/><stop offset="0.48" stop-color="#ec008c"/><stop offset="1" stop-color="#0047ff"/></linearGradient><radialGradient id="cool" cx="42%" cy="40%" r="70%"><stop offset="0" stop-color="#fff"/><stop offset="0.45" stop-color="#00aeef"/><stop offset="1" stop-color="#08111f"/></radialGradient></defs>
<rect width="100%" height="100%" fill="url(#warm)"/><circle cx="330" cy="310" r="235" fill="url(#cool)" opacity="0.92"/><rect x="565" y="115" width="260" height="365" rx="44" fill="#101114" opacity="0.78"/><path d="M90 555 C225 420 350 665 510 535 S745 440 870 585" fill="none" stroke="#fff" stroke-width="58" stroke-linecap="round" opacity="0.82"/><text x="620" y="345" font-family="sans-serif" font-size="122" font-weight="800" fill="#fff">T</text></svg>"##;

#[cfg(test)]
const PREVIEW_SURFACE_LABEL: &str = "Preview Surface — Canvas only · not exported";
#[cfg(test)]
const EXPORT_BACKGROUND_LABEL: &str = "Export Background — Used for SVG and by default for PNG";

const BUNDLED_PRESETS: [(&str, &[u8]); 6] = [
    (
        "Comic Book",
        include_bytes!("../assets/presets/ComicBook.tntr"),
    ),
    (
        "Skinny Curve",
        include_bytes!("../assets/presets/Skinny Curve.tntr"),
    ),
    (
        "Chunky Fingerprints",
        include_bytes!("../assets/presets/Chunky Fingerprints.tntr"),
    ),
    (
        "Tiled Stacked Motif Stress Test",
        include_bytes!("../assets/presets/Tiled Stacked Motif Stress Test.tntr"),
    ),
    (
        "Polygon Six",
        include_bytes!("../assets/presets/Polygon Six.tntr"),
    ),
    (
        "Motif Ladder",
        include_bytes!("../assets/presets/Motif Ladder.tntr"),
    ),
];
const START_HERO: &[u8] = include_bytes!("../assets/splash-hero.png");
const PREVIEW_INDICATOR_SVG: &[u8] = include_bytes!("../assets/preview-indicator.svg");
const WINDOW_UI_RESOURCE: &str = "/com/toniator/Toniator/toniator-window.ui";
const CHANNEL_CONTROLS_UI_RESOURCE: &str = "/com/toniator/Toniator/toniator-channel-controls.ui";
const AGGREGATE_CHANNEL_CONTROLS_UI_RESOURCE: &str =
    "/com/toniator/Toniator/toniator-aggregate-channel-controls.ui";
static UI_RESOURCES_REGISTERED: Once = Once::new();

fn register_ui_resources() {
    UI_RESOURCES_REGISTERED.call_once(|| {
        gio::resources_register_include!("toniator.gresource")
            .expect("Toniator GResource must register");
    });
}
#[cfg(test)]
const WINDOW_BLP: &str = include_str!("../resources/toniator-window.blp");
#[cfg(test)]
const CHANNEL_BLP: &str = include_str!("../resources/toniator-channel-controls.blp");
#[cfg(test)]
const AGGREGATE_CHANNEL_BLP: &str =
    include_str!("../resources/toniator-aggregate-channel-controls.blp");
#[cfg(test)]
const TOP_LEVEL_SHELL_OBJECT_IDS: [&str; 13] = [
    "main_window",
    "main_toolbar_view",
    "main_header_bar",
    "toast_overlay",
    "main_stack",
    "window_title",
    "new_project_button",
    "open_button",
    "save_button",
    "undo_button",
    "redo_button",
    "controls_toggle",
    "export_button",
];
const PREVIEW_INDICATOR_WIDTH: i32 = 40;
const PREVIEW_INDICATOR_HEIGHT: i32 = 28;
const PREVIEW_INDICATOR_RASTER_SCALE: i32 = 4;
const CROSSHATCH_INK_ORDER: [Ink; 4] = [Ink::Black, Ink::Cyan, Ink::Magenta, Ink::Yellow];

#[derive(Clone)]
enum PresetSource {
    Path(PathBuf),
    Bundled(&'static [u8]),
}

fn user_preset_dir(data_home: Option<&Path>, home: Option<&Path>) -> PathBuf {
    data_home
        .map(Path::to_path_buf)
        .or_else(|| home.map(|path| path.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"))
        .join("toniator/presets")
}

fn native_user_preset_dir() -> PathBuf {
    user_preset_dir(
        std::env::var_os("XDG_DATA_HOME").as_deref().map(Path::new),
        std::env::var_os("HOME").as_deref().map(Path::new),
    )
}

fn normalized_preset_path(path: &Path) -> PathBuf {
    if path
        .extension()
        .is_some_and(|value| value.eq_ignore_ascii_case("tntr"))
    {
        path.to_path_buf()
    } else {
        let mut value = path.as_os_str().to_owned();
        value.push(".tntr");
        PathBuf::from(value)
    }
}

fn preset_name_from_path(path: &Path) -> String {
    normalized_preset_path(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Untitled Preset")
        .to_owned()
}

fn shape_node_hit_test(nodes: &[ShapePoint], point: ShapePoint, radius: f64) -> Option<usize> {
    nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            let distance = (node.x - point.x).hypot(node.y - point.y);
            (distance <= radius).then_some((index, distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index)
}

#[cfg(test)]
fn update_shape_drag(
    nodes: &mut [ShapePoint],
    index: Option<usize>,
    origin: ShapePoint,
    dx: f64,
    dy: f64,
) -> bool {
    let Some(node) = index.and_then(|index| nodes.get_mut(index)) else {
        return false;
    };
    *node = ShapePoint {
        x: (origin.x + dx).clamp(-0.75, 0.75),
        y: (origin.y + dy).clamp(-0.75, 0.75),
    };
    true
}

fn translate_shape_anchor(path: &mut ClosedShapePath, index: usize, point: ShapePoint) {
    let Some(anchor) = path.anchors.get_mut(index) else {
        return;
    };
    let dx = point.x - anchor.point.x;
    let dy = point.y - anchor.point.y;
    anchor.point = point;
    anchor.incoming.x += dx;
    anchor.incoming.y += dy;
    anchor.outgoing.x += dx;
    anchor.outgoing.y += dy;
}

fn split_shape_segment(path: &mut ClosedShapePath, index: usize, amount: f64) {
    if path.anchors.len() < 2 {
        return;
    }
    let next_index = (index + 1) % path.anchors.len();
    let a = path.anchors[index];
    let b = path.anchors[next_index];
    let p01 = shape_point_lerp(a.point, a.outgoing, amount);
    let p12 = shape_point_lerp(a.outgoing, b.incoming, amount);
    let p23 = shape_point_lerp(b.incoming, b.point, amount);
    let p012 = shape_point_lerp(p01, p12, amount);
    let p123 = shape_point_lerp(p12, p23, amount);
    let point = shape_point_lerp(p012, p123, amount);
    path.anchors[index].outgoing = p01;
    path.anchors[next_index].incoming = p23;
    path.anchors.insert(
        next_index,
        ShapeAnchor {
            point,
            incoming: p012,
            outgoing: p123,
        },
    );
}

fn delete_shape_anchor(path: &mut ClosedShapePath, index: usize) -> bool {
    if path.anchors.len() <= 3 || index >= path.anchors.len() {
        return false;
    }
    path.anchors.remove(index);
    true
}

fn shape_point_lerp(a: ShapePoint, b: ShapePoint, amount: f64) -> ShapePoint {
    ShapePoint {
        x: a.x + (b.x - a.x) * amount,
        y: a.y + (b.y - a.y) * amount,
    }
}

fn curved_shape_fixture() -> ClosedShapePath {
    let mut path = ClosedShapePath::from_polygon(&toniator::model::default_shape_nodes());
    path.anchors[0].outgoing = ShapePoint { x: 0.05, y: -0.53 };
    path.anchors[1].incoming = ShapePoint { x: 0.28, y: -0.12 };
    path.anchors[1].outgoing = ShapePoint { x: 0.72, y: 0.02 };
    path.anchors[2].incoming = ShapePoint { x: 0.18, y: 0.38 };
    path
}

fn cubic_shape_point(a: ShapeAnchor, b: ShapeAnchor, t: f64) -> ShapePoint {
    let ab = shape_point_lerp(a.point, a.outgoing, t);
    let bc = shape_point_lerp(a.outgoing, b.incoming, t);
    let cd = shape_point_lerp(b.incoming, b.point, t);
    shape_point_lerp(shape_point_lerp(ab, bc, t), shape_point_lerp(bc, cd, t), t)
}

fn nearest_shape_segment(
    path: &ClosedShapePath,
    point: ShapePoint,
    tolerance: f64,
) -> Option<(usize, f64)> {
    let mut best = (0, 0.5, f64::INFINITY);
    for index in 0..path.anchors.len() {
        let a = path.anchors[index];
        let b = path.anchors[(index + 1) % path.anchors.len()];
        for step in 2..=30 {
            let t = step as f64 / 32.0;
            let candidate = cubic_shape_point(a, b, t);
            let distance = (candidate.x - point.x).hypot(candidate.y - point.y);
            if distance < best.2 {
                best = (index, t, distance);
            }
        }
    }
    (best.2 <= tolerance).then_some((best.0, best.1))
}

/// Inserts an anchor on the closest visible cubic segment in one mutable path
/// transaction. The returned index is the newly inserted anchor.
fn insert_nearest_shape_anchor(
    path: &mut ClosedShapePath,
    point: ShapePoint,
    tolerance: f64,
) -> Option<usize> {
    let (segment, amount) = nearest_shape_segment(path, point, tolerance)?;
    split_shape_segment(path, segment, amount);
    Some((segment + 1) % path.anchors.len())
}

fn connect_shape_editor_click(
    area: &gtk::DrawingArea,
    nodes: &Rc<RefCell<Vec<ShapePoint>>>,
    shape_path: &Rc<RefCell<ClosedShapePath>>,
    selected: &Rc<Cell<usize>>,
    selected_part: &Rc<Cell<i8>>,
) -> gtk::GestureClick {
    let click = gtk::GestureClick::new();
    click.connect_pressed(glib::clone!(
        #[strong]
        nodes,
        #[strong]
        shape_path,
        #[strong]
        selected,
        #[strong]
        selected_part,
        #[weak]
        area,
        move |_, count, x, y| {
            area.grab_focus();
            let width = area.width() as f64;
            let height = area.height() as f64;
            let side = width.min(height) * 0.82;
            if side <= 0.0 {
                return;
            }
            let point = ShapePoint {
                x: (x - width / 2.0) / side,
                y: (y - height / 2.0) / side,
            };
            if count == 2 {
                let inserted = {
                    let mut path = shape_path.borrow_mut();
                    let inserted = insert_nearest_shape_anchor(&mut path, point, 12.0 / side);
                    inserted.map(|index| {
                        let snapshot = path.anchors.iter().map(|anchor| anchor.point).collect();
                        (index, snapshot)
                    })
                };
                if let Some((index, snapshot)) = inserted {
                    *nodes.borrow_mut() = snapshot;
                    selected.set(index);
                    selected_part.set(0);
                }
            } else if let Some(index) = shape_node_hit_test(&nodes.borrow(), point, 0.045) {
                selected.set(index);
                selected_part.set(0);
            } else {
                let anchor = shape_path.borrow().anchors[selected.get()];
                for (part, handle) in [(-1, anchor.incoming), (1, anchor.outgoing)] {
                    if (handle.x - point.x).hypot(handle.y - point.y) <= 0.045 {
                        selected_part.set(part);
                    }
                }
            }
            area.queue_draw();
        }
    ));
    area.add_controller(click.clone());
    click
}

const PREVIEW_DEFAULT_MAX: u32 = 1400;
const PREVIEW_REFINEMENT_MAX: u32 = 4096;
const INSPECTOR_DEFAULT_WIDTH: i32 = 400;
const INSPECTOR_MIN_WIDTH: i32 = 340;
const INSPECTOR_MAX_WIDTH: i32 = 640;
const CANVAS_MIN_WIDTH: i32 = 360;
const NARROW_CONTROLS_BREAKPOINT: i32 = 820;
const EXPORT_CLOSE_INHIBIT_MESSAGE: &str = "Please wait for the export to finish before closing.";

mod center_stage {
    use super::*;

    #[derive(Default)]
    pub struct CenterStage {
        pub(super) child: RefCell<Option<gtk::Widget>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CenterStage {
        const NAME: &'static str = "ToniatorCenterStage";
        type Type = super::CenterStage;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for CenterStage {
        fn dispose(&self) {
            if let Some(child) = self.child.borrow_mut().take() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for CenterStage {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.child.borrow().as_ref().cloned() else {
                return (0, 0, -1, -1);
            };
            let minimum = child.measure(orientation, for_size).0.max(1);
            (minimum, minimum, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let Some(child) = self.child.borrow().as_ref().cloned() else {
                return;
            };
            let child_width = child.measure(gtk::Orientation::Horizontal, -1).0.max(1);
            let child_height = child
                .measure(gtk::Orientation::Vertical, child_width)
                .0
                .max(1);
            let x = ((width - child_width) / 2).max(0);
            let y = ((height - child_height) / 2).max(0);
            let transform = gtk::gsk::Transform::new()
                .translate(&gtk::graphene::Point::new(x as f32, y as f32));
            child.allocate(child_width, child_height, -1, Some(transform));
        }
    }
}

glib::wrapper! {
    pub struct CenterStage(ObjectSubclass<center_stage::CenterStage>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl CenterStage {
    #[cfg(test)]
    fn new(child: &impl IsA<gtk::Widget>) -> Self {
        let stage: Self = glib::Object::new();
        let child = child.clone().upcast::<gtk::Widget>();
        child.set_parent(&stage);
        stage.imp().child.replace(Some(child));
        stage
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct UiStateFile {
    version: u32,
    inspector_width: i32,
}

fn ui_state_path(state_home: Option<&Path>, home: Option<&Path>) -> PathBuf {
    state_home
        .map(Path::to_path_buf)
        .or_else(|| home.map(|path| path.join(".local/state")))
        .unwrap_or_else(std::env::temp_dir)
        .join("toniator/ui-state.json")
}

fn native_ui_state_path() -> PathBuf {
    ui_state_path(
        std::env::var_os("XDG_STATE_HOME").as_deref().map(Path::new),
        std::env::var_os("HOME").as_deref().map(Path::new),
    )
}

fn load_inspector_width(path: &Path) -> i32 {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<UiStateFile>(&bytes).ok())
        .filter(|state| state.version == 1)
        .map_or(INSPECTOR_DEFAULT_WIDTH, |state| {
            state
                .inspector_width
                .clamp(INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH)
        })
}

#[cfg(test)]
fn save_inspector_width(path: &Path, width: i32) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(&UiStateFile {
        version: 1,
        inspector_width: width.clamp(INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH),
    })?;
    toniator::persistence::atomic_write(path, &bytes)
}

fn constrained_inspector_width(desired: i32, total_width: i32) -> i32 {
    desired
        .clamp(INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH)
        .min((total_width - CANVAS_MIN_WIDTH).max(0))
}

fn scaled_artboard_size(width: u32, height: u32, zoom: f64, scale_factor: i32) -> (i32, i32) {
    let device_scale = scale_factor.max(1) as f64;
    (
        (width as f64 * zoom / device_scale).round().max(1.0) as i32,
        (height as f64 * zoom / device_scale).round().max(1.0) as i32,
    )
}

fn preview_target_dimension(width: u32, height: u32, zoom: f64) -> u32 {
    ((width.max(height) as f64 * zoom).ceil() as u32).clamp(1, PREVIEW_REFINEMENT_MAX)
}

fn preview_target_for_zoom(artboard: (u32, u32), zoom_mode: ZoomMode) -> u32 {
    PREVIEW_DEFAULT_MAX.max(preview_target_dimension(
        artboard.0,
        artboard.1,
        zoom_mode.percent() / 100.0,
    ))
}

fn shifted_effective(value: f64, delta: f64, lower: f64, upper: f64) -> f64 {
    (value + delta).clamp(lower, upper)
}

fn reset_crosshatch_curve_path(settings: &mut WebCurveSettings, inks: &[Ink]) {
    let path = CurvePath::straight();
    if settings.use_shared_curve {
        settings.shared_path = path;
        settings.shared_close_ends = false;
        settings.shared_smooth_join = false;
    } else {
        for ink in inks {
            let channel = settings.channels.get_mut(*ink);
            channel.path = path.clone();
            channel.close_ends = false;
            channel.smooth_join = false;
        }
    }
}

fn document_artboard_size(document: &Document) -> (u32, u32) {
    match document.pattern_state.selected_pattern_id() {
        Some(PatternId::COMPATIBILITY_SHAPES_V1) => document
            .pattern_state
            .shape_settings()
            .map(|settings| (settings.output_width, settings.output_height))
            .unwrap_or((900, 620)),
        Some(PatternId::COMPATIBILITY_CURVES_V1) => document
            .pattern_state
            .curve_settings()
            .map(|settings| (settings.output_width, settings.output_height))
            .unwrap_or((900, 620)),
        Some(PatternId::WEIGHTED_VORONOI_V1) => (900, 620),
        None => (900, 620),
    }
}

fn pipeline_uses_crosshatch(pipeline: &ArtworkPipelineSettings) -> bool {
    matches!(
        pipeline.assignment,
        ChannelAssignment::LegacyCompatibility(
            LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1
        )
    )
}

fn document_uses_crosshatch(document: &Document) -> bool {
    pipeline_uses_crosshatch(&document.artwork_pipeline)
}

fn rgba_color(color: gdk::RGBA) -> RgbaColor {
    let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    RgbaColor {
        red: byte(color.red()),
        green: byte(color.green()),
        blue: byte(color.blue()),
        alpha: byte(color.alpha()),
    }
}

fn gdk_rgba(color: RgbaColor) -> gdk::RGBA {
    gdk::RGBA::new(
        color.red as f32 / 255.0,
        color.green as f32 / 255.0,
        color.blue as f32 / 255.0,
        color.alpha as f32 / 255.0,
    )
}

fn rgba_hex(color: RgbaColor) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.red, color.green, color.blue, color.alpha
    )
}

fn export_background_from_selection(selected: u32, current: ExportBackground) -> ExportBackground {
    if selected == 0 {
        ExportBackground::None
    } else {
        match current {
            ExportBackground::Color { .. } => current,
            ExportBackground::None => ExportBackground::Color {
                color: RgbaColor::WHITE,
            },
        }
    }
}

fn export_background_color_label(background: ExportBackground) -> String {
    match background {
        ExportBackground::None => "Background Color · None (transparent)".into(),
        ExportBackground::Color { color } => format!("Background Color · {}", rgba_hex(color)),
    }
}

fn sync_export_background_color_control(
    label: &gtk::Label,
    button: &gtk::ColorDialogButton,
    background: ExportBackground,
) {
    let text = export_background_color_label(background);
    let color = match background {
        ExportBackground::None => RgbaColor::WHITE,
        ExportBackground::Color { color } => color,
    };
    label.set_text(&text);
    label.set_tooltip_text(Some(&text));
    button.set_rgba(&gdk_rgba(color));
    button.set_tooltip_text(Some(&text));
    button.update_property(&[
        gtk::accessible::Property::Label("Export Background Color"),
        gtk::accessible::Property::Description(&text),
    ]);
}

fn png_background_selection_summary(
    selected: u32,
    document_background: ExportBackground,
) -> String {
    match selected {
        0 => match document_background {
            ExportBackground::None => "Document Export Background: None (transparent)".into(),
            ExportBackground::Color { color } => format!(
                "Document Export Background: #{:02X}{:02X}{:02X}{:02X}",
                color.red, color.green, color.blue, color.alpha
            ),
        },
        1 => "Transparent Override (ignores saved Export Background)".into(),
        _ => "White Override (ignores saved Export Background)".into(),
    }
}

fn parse_artifact_rgba(value: &str) -> Option<RgbaColor> {
    let value = value.strip_prefix('#')?;
    if value.len() != 8 {
        return None;
    }
    Some(RgbaColor {
        red: u8::from_str_radix(&value[0..2], 16).ok()?,
        green: u8::from_str_radix(&value[2..4], 16).ok()?,
        blue: u8::from_str_radix(&value[4..6], 16).ok()?,
        alpha: u8::from_str_radix(&value[6..8], 16).ok()?,
    })
}

struct AppState {
    editor: Option<DocumentEditor>,
    path: Option<PathBuf>,
    syncing_controls: bool,
    preview_size: Option<(u32, u32)>,
    compare_source: bool,
    zoom_mode: ZoomMode,
    source_cache: Option<PreviewCache>,
    rendered_cache: Option<PreviewCache>,
}

fn clear_document_for_new_project(state: &mut AppState) {
    state.editor = None;
    state.path = None;
    state.preview_size = None;
    state.compare_source = false;
    state.zoom_mode = ZoomMode::Fit(100.0);
    state.source_cache = None;
    state.rendered_cache = None;
}

#[derive(Clone)]
struct PreviewCache {
    document: Document,
    image: RgbaImage,
}

fn preview_cache_matches(cache: &PreviewCache, document: &Document, view: PreviewView) -> bool {
    match view {
        // Source pixels do not depend on treatment, while appearance changes
        // their displayed backdrop and must invalidate the cache.
        PreviewView::Source => {
            cache.document.document_id == document.document_id
                && cache.document.appearance == document.appearance
        }
        PreviewView::Rendered => cache.document == *document,
    }
}

fn preview_cache_is_sufficient(cache: &PreviewCache, target: u32) -> bool {
    cache.image.width().max(cache.image.height()) >= target
}

type FitAllocationInput = ((u32, u32), (i32, i32), i32);

#[derive(Debug, Default)]
struct FitAllocationState {
    input: Option<FitAllocationInput>,
    refinement_generation: u64,
}

impl FitAllocationState {
    fn observe(&mut self, input: FitAllocationInput) -> Option<u64> {
        if self.input == Some(input) {
            return None;
        }
        self.input = Some(input);
        self.refinement_generation = self.refinement_generation.wrapping_add(1);
        Some(self.refinement_generation)
    }

    fn reset(&mut self) {
        self.input = None;
        self.refinement_generation = self.refinement_generation.wrapping_add(1);
    }

    fn accepts(&self, generation: u64) -> bool {
        self.refinement_generation == generation
    }
}

fn fit_refinement_target(
    artboard: (u32, u32),
    zoom_mode: ZoomMode,
    preview_size: Option<(u32, u32)>,
) -> Option<u32> {
    if !matches!(zoom_mode, ZoomMode::Fit(_)) {
        return None;
    }
    let target = preview_target_for_zoom(artboard, zoom_mode);
    preview_size
        .is_none_or(|(width, height)| width.max(height) < target)
        .then_some(target)
}

#[derive(Clone, Copy)]
struct MotifDrag {
    kind: u8,
    start_x: f64,
    start_y: f64,
    offset_x: f64,
    offset_y: f64,
    angle: f64,
    spacing: f64,
}

#[derive(Clone, Copy)]
enum ZoomMode {
    Fit(f64),
    Explicit(f64),
}

#[derive(Debug, Clone, Copy)]
enum ZoomIntent {
    Slider(f64),
    Entry(f64),
    Increase,
    Decrease,
}

#[derive(Debug, Clone)]
enum ZoomControlCommand {
    Fit,
    Manual(ZoomIntent),
    Entry(String),
}

const ZOOM_MIN: f64 = 5.0;
const ZOOM_MAX: f64 = 800.0;
const ZOOM_STEP: f64 = 25.0;
fn fitted_zoom_percent(artboard: (u32, u32), viewport: (i32, i32), scale_factor: i32) -> f64 {
    let usable_width = viewport.0.max(1) as f64;
    let usable_height = viewport.1.max(1) as f64;
    let device_scale = scale_factor.max(1) as f64;
    (usable_width * device_scale / artboard.0.max(1) as f64)
        .min(usable_height * device_scale / artboard.1.max(1) as f64)
        * 100.0
}

fn fitted_artwork_size(
    artboard: (u32, u32),
    viewport: (i32, i32),
    scale_factor: i32,
) -> (i32, i32) {
    scaled_artboard_size(
        artboard.0,
        artboard.1,
        fitted_zoom_percent(artboard, viewport, scale_factor) / 100.0,
        scale_factor,
    )
}

fn fit_edge_deltas(artboard: (u32, u32), viewport: (i32, i32), scale_factor: i32) -> (i32, i32) {
    let fitted = fitted_artwork_size(artboard, viewport, scale_factor);
    (
        (viewport.0 - fitted.0).max(0),
        (viewport.1 - fitted.1).max(0),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CanvasAllocationMetrics {
    origin: (i32, i32),
    slack: (i32, i32, i32, i32),
}

impl CanvasAllocationMetrics {
    fn centered(viewport: (i32, i32), artwork: (i32, i32)) -> Self {
        let (origin_x, left, right) = centered_axis_allocation(viewport.0, artwork.0);
        let (origin_y, top, bottom) = centered_axis_allocation(viewport.1, artwork.1);
        Self {
            origin: (origin_x, origin_y),
            slack: (left, right, top, bottom),
        }
    }

    fn horizontal_delta(self) -> i32 {
        (self.slack.0 - self.slack.1).abs()
    }

    fn vertical_delta(self) -> i32 {
        (self.slack.2 - self.slack.3).abs()
    }
}

fn centered_axis_allocation(viewport: i32, artwork: i32) -> (i32, i32, i32) {
    let slack = viewport - artwork;
    if slack <= 0 {
        return (0, 0, slack);
    }
    let before = slack / 2;
    (before, before, slack - before)
}

fn opaque_capture_node(
    content: &gtk::gsk::RenderNode,
    width: u32,
    height: u32,
    mut background: gdk::RGBA,
) -> gtk::gsk::RenderNode {
    background.set_alpha(1.0);
    let snapshot = gtk::Snapshot::new();
    snapshot.append_color(
        &background,
        &gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32),
    );
    snapshot.append_node(content);
    snapshot
        .to_node()
        .expect("opaque capture background always produces a render node")
}

fn capture_window_background() -> gdk::RGBA {
    if adw::StyleManager::default().is_dark() {
        gdk::RGBA::new(0.141, 0.141, 0.141, 1.0)
    } else {
        gdk::RGBA::new(0.98, 0.98, 0.98, 1.0)
    }
}

impl ZoomMode {
    fn percent(self) -> f64 {
        match self {
            Self::Fit(value) | Self::Explicit(value) => value,
        }
    }

    fn update_fit(self, artboard: (u32, u32), viewport: (i32, i32), scale: i32) -> Self {
        Self::Fit(fitted_zoom_percent(artboard, viewport, scale))
    }

    fn apply_manual(self, intent: ZoomIntent) -> Self {
        let value = match intent {
            ZoomIntent::Slider(value) | ZoomIntent::Entry(value) => value,
            ZoomIntent::Increase => self.percent() + ZOOM_STEP,
            ZoomIntent::Decrease => self.percent() - ZOOM_STEP,
        };
        Self::Explicit(value.clamp(ZOOM_MIN, ZOOM_MAX))
    }
}

fn zoom_percent_text(percent: f64) -> String {
    format!("{percent:0.3}")
}

fn connect_zoom_control_commands(
    fit: &gtk::ToggleButton,
    zoom_out: &gtk::Button,
    zoom: &gtk::Scale,
    zoom_entry: &gtk::Entry,
    zoom_in: &gtk::Button,
    command: Rc<dyn Fn(ZoomControlCommand)>,
) {
    let callback = Rc::clone(&command);
    fit.connect_clicked(move |_| callback(ZoomControlCommand::Fit));
    let callback = Rc::clone(&command);
    zoom_out.connect_clicked(move |_| callback(ZoomControlCommand::Manual(ZoomIntent::Decrease)));
    let callback = Rc::clone(&command);
    zoom_in.connect_clicked(move |_| callback(ZoomControlCommand::Manual(ZoomIntent::Increase)));
    let callback = Rc::clone(&command);
    zoom.connect_value_changed(move |scale| {
        callback(ZoomControlCommand::Manual(ZoomIntent::Slider(
            scale.value(),
        )))
    });
    let callback = Rc::clone(&command);
    zoom_entry
        .connect_activate(move |entry| callback(ZoomControlCommand::Entry(entry.text().into())));
    zoom_entry.connect_has_focus_notify(move |entry| {
        if !entry.has_focus() {
            command(ZoomControlCommand::Entry(entry.text().into()));
        }
    });
}

fn sync_zoom_control_widgets(
    fit: &gtk::ToggleButton,
    zoom: &gtk::Scale,
    zoom_entry: &gtk::Entry,
    percent: f64,
    fitted: bool,
) {
    fit.set_active(fitted);
    zoom.adjustment()
        .set_lower(zoom.adjustment().lower().min(percent));
    zoom.adjustment()
        .set_upper(zoom.adjustment().upper().max(percent));
    zoom.set_value(percent);
    zoom_entry.set_text(&zoom_percent_text(percent));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewView {
    Source,
    Rendered,
}

#[derive(Debug, Default)]
struct PreviewActivity {
    requested: Option<(u64, PreviewView)>,
    terminal: Option<(u64, PreviewTerminal)>,
    installed: Option<(u64, PreviewView)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewTerminal {
    Installed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArtifactPreviewReadiness {
    Ready,
    Waiting,
    Failed,
}

fn artifact_preview_readiness(
    activity: &PreviewActivity,
    desired_view: PreviewView,
    cache_ready: bool,
    picture_ready: bool,
) -> ArtifactPreviewReadiness {
    if let Some((generation, requested_view)) = activity.requested {
        if requested_view == desired_view {
            if activity.terminal == Some((generation, PreviewTerminal::Failed)) {
                return ArtifactPreviewReadiness::Failed;
            }
            if activity.terminal != Some((generation, PreviewTerminal::Installed))
                || activity.installed != Some((generation, desired_view))
            {
                return ArtifactPreviewReadiness::Waiting;
            }
        } else if activity.active() {
            return ArtifactPreviewReadiness::Waiting;
        }
    }
    if cache_ready
        && picture_ready
        && activity
            .installed
            .is_some_and(|(_, installed_view)| installed_view == desired_view)
    {
        ArtifactPreviewReadiness::Ready
    } else {
        ArtifactPreviewReadiness::Waiting
    }
}

impl PreviewActivity {
    fn request(&mut self, generation: u64, view: PreviewView) {
        self.requested = Some((generation, view));
    }
    fn installed(&mut self, generation: u64, view: PreviewView) {
        if self.requested == Some((generation, view)) {
            self.terminal = Some((generation, PreviewTerminal::Installed));
            self.installed = Some((generation, view));
        }
    }
    fn failed(&mut self, generation: u64) {
        if self
            .requested
            .is_some_and(|(requested, _)| requested == generation)
        {
            self.terminal = Some((generation, PreviewTerminal::Failed));
        }
    }
    fn cancelled(&mut self, generation: u64) {
        if self
            .requested
            .is_some_and(|(requested, _)| requested == generation)
        {
            self.terminal = Some((generation, PreviewTerminal::Cancelled));
        }
    }
    fn active(&self) -> bool {
        self.requested.is_some_and(|(generation, _)| {
            !matches!(self.terminal, Some((terminal, _)) if terminal == generation)
        })
    }
    fn render_busy(&self) -> bool {
        self.active() && matches!(self.requested, Some((_, PreviewView::Rendered)))
    }
    fn source_override(&self) -> bool {
        matches!(self.requested, Some((_, PreviewView::Source)))
    }
    fn resting_phase(&self) -> f64 {
        if self.source_override() || matches!(self.installed, Some((_, PreviewView::Source))) {
            0.0
        } else {
            1.0
        }
    }
    fn accessible_label(&self) -> &'static str {
        if self.source_override()
            || (!self.render_busy() && matches!(self.installed, Some((_, PreviewView::Source))))
        {
            "Source preview"
        } else if self.render_busy() {
            "Updating halftone preview"
        } else {
            "Halftone preview"
        }
    }
}

fn preview_animation_phase(elapsed: Duration, reduced_motion: bool) -> f64 {
    if reduced_motion {
        return 0.5;
    }
    let one_way = 1.8;
    let position = elapsed.as_secs_f64() % (one_way * 2.0);
    let linear = if position <= one_way {
        position / one_way
    } else {
        (one_way * 2.0 - position) / one_way
    };
    (1.0 - (std::f64::consts::PI * linear).cos()) * 0.5
}

#[derive(Clone)]
struct PreviewIndicator {
    area: gtk::DrawingArea,
    activity: Rc<RefCell<PreviewActivity>>,
    epoch: Rc<Cell<Option<Instant>>>,
    tick: Rc<RefCell<Option<gtk::TickCallbackId>>>,
    artifact_phase: Rc<Cell<Option<f64>>>,
}

#[derive(Clone)]
struct SvgMask {
    alpha: Arc<[u8]>,
    width: i32,
    height: i32,
    stride: i32,
    x: f64,
    y: f64,
    raster_scale: f64,
}

struct PreviewIndicatorArtwork {
    solid: SvgMask,
    dots: SvgMask,
}

impl PreviewIndicatorArtwork {
    fn from_embedded_svg() -> Result<Self, String> {
        let tree = usvg::Tree::from_data(PREVIEW_INDICATOR_SVG, &usvg::Options::default())
            .map_err(|error| format!("could not parse preview indicator SVG: {error}"))?;
        if tree.size().width() != PREVIEW_INDICATOR_WIDTH as f32
            || tree.size().height() != PREVIEW_INDICATOR_HEIGHT as f32
        {
            return Err("preview indicator SVG must use a 40x28 canvas".into());
        }
        Ok(Self {
            solid: render_svg_group_mask(&tree, "solid-t")?,
            dots: render_svg_group_mask(&tree, "halftone-dots")?,
        })
    }
}

fn render_svg_group_mask(tree: &usvg::Tree, id: &str) -> Result<SvgMask, String> {
    let node = tree
        .node_by_id(id)
        .ok_or_else(|| format!("preview indicator SVG is missing #{id}"))?;
    let bbox = node
        .abs_layer_bounding_box()
        .ok_or_else(|| format!("preview indicator SVG group #{id} has no bounds"))?;
    let scale = PREVIEW_INDICATOR_RASTER_SCALE as f32;
    let width = (bbox.width() * scale).ceil().max(1.0) as u32;
    let height = (bbox.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| format!("could not allocate preview indicator mask #{id}"))?;
    resvg::render_node(
        node,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    )
    .ok_or_else(|| format!("preview indicator SVG group #{id} has no renderable geometry"))?;

    let stride = gtk::cairo::Format::A8
        .stride_for_width(width)
        .map_err(|error| format!("invalid preview indicator mask stride: {error}"))?;
    let mut alpha = vec![0; stride as usize * height as usize];
    for row in 0..height as usize {
        for column in 0..width as usize {
            alpha[row * stride as usize + column] =
                pixmap.data()[(row * width as usize + column) * 4 + 3];
        }
    }
    Ok(SvgMask {
        alpha: alpha.into(),
        width: width as i32,
        height: height as i32,
        stride,
        x: bbox.x() as f64,
        y: bbox.y() as f64,
        raster_scale: PREVIEW_INDICATOR_RASTER_SCALE as f64,
    })
}

impl PreviewIndicator {
    fn new(artifact_phase: Option<f64>) -> Self {
        let artwork = Rc::new(
            PreviewIndicatorArtwork::from_embedded_svg()
                .expect("bundled preview indicator SVG must be valid"),
        );
        let area = gtk::DrawingArea::builder()
            .width_request(PREVIEW_INDICATOR_WIDTH)
            .height_request(PREVIEW_INDICATOR_HEIGHT)
            .hexpand(false)
            .vexpand(false)
            .accessible_role(gtk::AccessibleRole::Img)
            .css_classes(["preview-indicator"])
            .build();
        area.set_content_width(PREVIEW_INDICATOR_WIDTH);
        area.set_content_height(PREVIEW_INDICATOR_HEIGHT);
        area.set_halign(gtk::Align::Center);
        area.set_valign(gtk::Align::Center);
        let activity = Rc::new(RefCell::new(PreviewActivity::default()));
        let epoch: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
        let artifact_phase = Rc::new(Cell::new(artifact_phase));
        area.set_draw_func(glib::clone!(
            #[strong]
            activity,
            #[strong]
            epoch,
            #[strong]
            artifact_phase,
            #[strong]
            artwork,
            move |area, cr, width, height| {
                let activity = activity.borrow();
                let phase = artifact_phase.get().unwrap_or_else(|| {
                    if activity.source_override() {
                        0.0
                    } else if activity.render_busy() {
                        preview_animation_phase(
                            epoch.get().map_or(Duration::ZERO, |start| start.elapsed()),
                            !adw::is_animations_enabled(area),
                        )
                    } else {
                        activity.resting_phase()
                    }
                });
                draw_preview_indicator(cr, width, height, phase, area.color(), &artwork);
            }
        ));
        let indicator = Self {
            area,
            activity,
            epoch,
            tick: Rc::new(RefCell::new(None)),
            artifact_phase,
        };
        indicator.sync_accessibility();
        indicator
    }

    fn request(&self, generation: u64, view: PreviewView) {
        let was_busy = self.activity.borrow().render_busy();
        self.activity.borrow_mut().request(generation, view);
        if !was_busy && self.activity.borrow().render_busy() {
            self.epoch.set(Some(Instant::now()));
        }
        self.sync();
    }

    fn installed(&self, generation: u64, view: PreviewView) {
        self.activity.borrow_mut().installed(generation, view);
        self.sync();
    }

    fn failed(&self, generation: u64) {
        self.activity.borrow_mut().failed(generation);
        self.sync();
    }

    fn cancelled(&self, generation: u64) {
        self.activity.borrow_mut().cancelled(generation);
        self.sync();
    }

    fn selected(&self, view: PreviewView) {
        let mut activity = self.activity.borrow_mut();
        activity.requested = None;
        activity.terminal = None;
        activity.installed = Some((0, view));
        drop(activity);
        self.sync();
    }

    fn sync(&self) {
        let animate = self.activity.borrow().render_busy()
            && adw::is_animations_enabled(&self.area)
            && self.artifact_phase.get().is_none();
        if animate && self.tick.borrow().is_none() {
            let id = self.area.add_tick_callback(glib::clone!(
                #[weak(rename_to = area)]
                self.area,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move |_, _| {
                    area.queue_draw();
                    glib::ControlFlow::Continue
                }
            ));
            self.tick.borrow_mut().replace(id);
        } else if !animate && let Some(id) = self.tick.borrow_mut().take() {
            id.remove();
        }
        self.sync_accessibility();
        self.area.queue_draw();
    }

    fn sync_accessibility(&self) {
        self.area.set_tooltip_text(Some(self.effective_label()));
        self.area
            .update_property(&[gtk::accessible::Property::Label(self.effective_label())]);
        self.area.update_state(&[gtk::accessible::State::Busy(
            self.activity.borrow().render_busy(),
        )]);
    }

    fn effective_busy(&self) -> bool {
        self.artifact_phase.get() == Some(0.5) || self.activity.borrow().render_busy()
    }

    fn effective_label(&self) -> &'static str {
        match self.artifact_phase.get() {
            Some(0.0) => "Source preview",
            Some(0.5) => "Updating halftone preview",
            Some(1.0) => "Halftone preview",
            _ => self.activity.borrow().accessible_label(),
        }
    }

    fn phase(&self) -> f64 {
        self.artifact_phase.get().unwrap_or_else(|| {
            let activity = self.activity.borrow();
            if activity.source_override() {
                0.0
            } else if activity.render_busy() {
                preview_animation_phase(
                    self.epoch
                        .get()
                        .map_or(Duration::ZERO, |start| start.elapsed()),
                    !adw::is_animations_enabled(&self.area),
                )
            } else {
                activity.resting_phase()
            }
        })
    }
}

fn draw_preview_indicator(
    cr: &gtk::cairo::Context,
    width: i32,
    height: i32,
    phase: f64,
    color: gdk::RGBA,
    artwork: &PreviewIndicatorArtwork,
) {
    let (solid_opacity, dot_opacity) = preview_indicator_layers(phase);
    let ox = (width as f64 - PREVIEW_INDICATOR_WIDTH as f64) * 0.5;
    let oy = (height as f64 - PREVIEW_INDICATOR_HEIGHT as f64) * 0.5;
    let surface = composed_svg_indicator(artwork, color, solid_opacity, dot_opacity);
    let physical_width = PREVIEW_INDICATOR_WIDTH * PREVIEW_INDICATOR_RASTER_SCALE;
    let physical_height = PREVIEW_INDICATOR_HEIGHT * PREVIEW_INDICATOR_RASTER_SCALE;
    let _ = cr.save();
    cr.translate(ox, oy);
    cr.scale(
        1.0 / PREVIEW_INDICATOR_RASTER_SCALE as f64,
        1.0 / PREVIEW_INDICATOR_RASTER_SCALE as f64,
    );
    cr.rectangle(
        0.0,
        0.0,
        f64::from(physical_width),
        f64::from(physical_height),
    );
    cr.clip();
    let _ = cr.set_source_surface(&surface, 0.0, 0.0);
    let _ = cr.paint();
    let _ = cr.restore();
}

fn composed_svg_indicator(
    artwork: &PreviewIndicatorArtwork,
    color: gdk::RGBA,
    solid_opacity: f64,
    dot_opacity: f64,
) -> gtk::cairo::ImageSurface {
    let width = PREVIEW_INDICATOR_WIDTH * PREVIEW_INDICATOR_RASTER_SCALE;
    let height = PREVIEW_INDICATOR_HEIGHT * PREVIEW_INDICATOR_RASTER_SCALE;
    let stride = gtk::cairo::Format::ARgb32
        .stride_for_width(width as u32)
        .expect("SVG indicator width has a valid Cairo stride");
    let mut pixels = vec![0; stride as usize * height as usize];
    let alpha_at = |mask: &SvgMask, x: i32, y: i32| {
        let local_x = x - (mask.x * mask.raster_scale).round() as i32;
        let local_y = y - (mask.y * mask.raster_scale).round() as i32;
        if local_x < 0 || local_y < 0 || local_x >= mask.width || local_y >= mask.height {
            0.0
        } else {
            f64::from(mask.alpha[local_y as usize * mask.stride as usize + local_x as usize])
                / 255.0
        }
    };
    for row in 0..height {
        for column in 0..width {
            let solid = alpha_at(&artwork.solid, column, row) * solid_opacity;
            let dots = alpha_at(&artwork.dots, column, row) * dot_opacity;
            let alpha = ((solid + dots * (1.0 - solid)) * 255.0).round() as u8;
            let offset = row as usize * stride as usize + column as usize * 4;
            pixels[offset] = (color.blue() as f64 * f64::from(alpha)).round() as u8;
            pixels[offset + 1] = (color.green() as f64 * f64::from(alpha)).round() as u8;
            pixels[offset + 2] = (color.red() as f64 * f64::from(alpha)).round() as u8;
            pixels[offset + 3] = alpha;
        }
    }
    gtk::cairo::ImageSurface::create_for_data(
        pixels,
        gtk::cairo::Format::ARgb32,
        width,
        height,
        stride,
    )
    .expect("SVG indicator pixels form a valid Cairo surface")
}

fn preview_indicator_layers(phase: f64) -> (f64, f64) {
    let phase = phase.clamp(0.0, 1.0);
    (1.0 - phase, phase)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClosePolicy {
    InhibitExport,
    Proceed,
    CheckDirty,
}

fn close_policy(export_running: bool, close_approved: bool, dirty: bool) -> ClosePolicy {
    if export_running {
        ClosePolicy::InhibitExport
    } else if close_approved || !dirty {
        ClosePolicy::Proceed
    } else {
        ClosePolicy::CheckDirty
    }
}

fn crosshatch_context(treatment: &str, target: u32, angle: f64) -> String {
    match target {
        0 => format!("{treatment} · Crosshatch · All layers"),
        1 => format!("{treatment} · Crosshatch · Layer 1 (Black) · {angle:.0}°"),
        2 => format!("{treatment} · Crosshatch · Layer 2 (Cyan) · {angle:.0}°"),
        3 => format!("{treatment} · Crosshatch · Layer 3 (Magenta) · {angle:.0}°"),
        4 => format!("{treatment} · Crosshatch · Layer 4 (Yellow) · {angle:.0}°"),
        _ => format!("{treatment} · Crosshatch · All layers"),
    }
}

struct RenderRequest {
    generation: u64,
    document: Document,
    compare_source: bool,
    max_dimension: u32,
    token: CancellationToken,
}

fn build_render_request(
    generation: u64,
    document: &Document,
    compare_source: bool,
    zoom_mode: ZoomMode,
) -> RenderRequest {
    RenderRequest {
        generation,
        document: document.clone(),
        compare_source,
        max_dimension: preview_target_for_zoom(document_artboard_size(document), zoom_mode),
        token: CancellationToken::new(),
    }
}

struct RenderOutcome {
    generation: u64,
    view: PreviewView,
    document: Document,
    result: anyhow::Result<RgbaImage>,
}

struct AutosaveOutcome {
    document_id: String,
    result: anyhow::Result<()>,
}

struct AutosaveRequest {
    generation: u64,
    document: Document,
}

struct ExportOutcome {
    path: PathBuf,
    kind: &'static str,
    result: ExportResult,
}

enum ExportResult {
    Completed,
    Cancelled,
    Failed(anyhow::Error),
}

struct LatestSlot<T> {
    value: Mutex<Option<T>>,
    ready: Condvar,
}

impl<T> Default for LatestSlot<T> {
    fn default() -> Self {
        Self {
            value: Mutex::new(None),
            ready: Condvar::new(),
        }
    }
}

impl<T> LatestSlot<T> {
    fn replace(&self, value: T) {
        *self.value.lock().expect("latest-value lock poisoned") = Some(value);
        self.ready.notify_one();
    }

    fn take(&self) -> Option<T> {
        self.value
            .lock()
            .expect("latest-value lock poisoned")
            .take()
    }

    fn wait_take(&self) -> T {
        let mut guard = self.value.lock().expect("latest-value lock poisoned");
        loop {
            if let Some(value) = guard.take() {
                return value;
            }
            guard = self.ready.wait(guard).expect("latest-value lock poisoned");
        }
    }
}

struct InspectorPaneController {
    paned: adw::OverlaySplitView,
    controls: gtk::ToggleButton,
    desired_width: Cell<i32>,
    pending_width: Cell<Option<i32>>,
    collapsed: Cell<bool>,
    narrow: Cell<bool>,
}

#[derive(Clone, Copy)]
struct ArtifactAllocation {
    inspector_width: i32,
    viewport: (i32, i32),
    fit_edge_deltas: (i32, i32),
    canvas_metrics: CanvasAllocationMetrics,
    preview_size: (u32, u32),
}

impl InspectorPaneController {
    fn new(
        paned: &adw::OverlaySplitView,
        controls: &gtk::ToggleButton,
        desired_width: i32,
    ) -> Rc<Self> {
        Rc::new(Self {
            paned: paned.clone(),
            controls: controls.clone(),
            desired_width: Cell::new(desired_width.clamp(INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH)),
            pending_width: Cell::new(None),
            collapsed: Cell::new(false),
            narrow: Cell::new(false),
        })
    }

    fn maintain(&self) {
        if self.narrow.get() {
            return;
        }
        if self.collapsed.get() {
            return;
        }
        let total = self.paned.width();
        let actual = self.current_width();
        if total <= 0 || actual <= 0 {
            return;
        }
        let target = constrained_inspector_width(self.desired_width.get(), total);
        if (actual - target).abs() <= 1 {
            self.pending_width.set(None);
            return;
        }
        self.pending_width.set(Some(target));
        let corrected = target.clamp(INSPECTOR_MIN_WIDTH, total);
        if (corrected - self.current_width()).abs() > 1 {
            self.set_sidebar_width(corrected);
        }
    }

    fn current_width(&self) -> i32 {
        let total = self.paned.width();
        if total <= 0 {
            return 0;
        }
        (self.paned.sidebar_width_fraction() * total as f64).round() as i32
    }

    fn set_sidebar_width(&self, width: i32) {
        let total = self.paned.width();
        if total > 0 {
            self.paned
                .set_sidebar_width_fraction((width as f64 / total as f64).clamp(0.1, 0.9));
        }
    }

    fn set_collapsed(&self, collapsed: bool) {
        if self.collapsed.replace(collapsed) == collapsed {
            return;
        }
        self.pending_width.set(None);
        if self.narrow.get() {
            self.paned.set_show_sidebar(!collapsed);
            if collapsed {
                self.controls.grab_focus();
            }
            return;
        }
        self.paned.set_show_sidebar(!collapsed);
        if collapsed {
            self.paned.set_collapsed(false);
        } else {
            let target = constrained_inspector_width(self.desired_width.get(), self.paned.width());
            self.set_sidebar_width(target);
            self.maintain();
        }
    }

    fn update_layout(&self) {
        let narrow = self.paned.width() > 0 && self.paned.width() < NARROW_CONTROLS_BREAKPOINT;
        if self.narrow.replace(narrow) == narrow {
            return;
        }
        if narrow {
            self.paned.set_collapsed(true);
            self.paned.set_show_sidebar(false);
            self.collapsed.set(true);
            self.controls.set_active(false);
        } else {
            self.paned.set_collapsed(false);
            let collapsed = self.collapsed.get();
            self.paned.set_show_sidebar(!collapsed);
            if !collapsed {
                let target =
                    constrained_inspector_width(self.desired_width.get(), self.paned.width());
                self.set_sidebar_width(target);
            }
        }
    }
}

pub struct AppUi {
    window: adw::ApplicationWindow,
    open: gtk::Button,
    stack: gtk::Stack,
    toast_overlay: adw::ToastOverlay,
    title: adw::WindowTitle,
    new_project_button: gtk::Button,
    controls_toggle: gtk::ToggleButton,
    picture: gtk::Picture,
    canvas: gtk::ScrolledWindow,
    canvas_content: gtk::Overlay,
    inspector_pane: Rc<InspectorPaneController>,
    #[cfg(test)]
    inspector_root: gtk::Box,
    source_label: gtk::Label,
    preview_indicator: PreviewIndicator,
    autosave_status: gtk::Label,
    workspace_status: gtk::Label,
    cancel_preview: gtk::Button,
    cancel_export: gtk::Button,
    editing_context: gtk::Label,
    detail: gtk::Scale,
    coverage: gtk::Scale,
    contrast: gtk::Scale,
    angle: gtk::Scale,
    dots: gtk::ToggleButton,
    squares: gtk::ToggleButton,
    lines: gtk::ToggleButton,
    curves: gtk::ToggleButton,
    weighted_voronoi: gtk::ToggleButton,
    legacy: gtk::ToggleButton,
    treatment_modes: gtk::Stack,
    weighted_voronoi_channel: gtk::DropDown,
    weighted_voronoi_cell_count: gtk::Scale,
    weighted_voronoi_visible: [gtk::CheckButton; 4],
    weighted_voronoi_arrangement: gtk::DropDown,
    weighted_voronoi_placement: gtk::DropDown,
    weighted_voronoi_density_strength: gtk::Scale,
    weighted_voronoi_response_strength: gtk::Scale,
    weighted_voronoi_boundary_gap: gtk::Scale,
    weighted_voronoi_seed: gtk::Entry,
    preset_import: gtk::Button,
    preset_save: gtk::Button,
    source_section: gtk::Expander,
    output_section: gtk::Expander,
    channel_settings_section: gtk::Expander,
    output_mode: gtk::DropDown,
    artwork_source: gtk::DropDown,
    artwork_source_note: gtk::Label,
    source_alpha: gtk::DropDown,
    source_alpha_row: gtk::Widget,
    source_alpha_note: gtk::Label,
    channel_assignment: gtk::DropDown,
    channel_assignment_note: gtk::Label,
    active_channel: gtk::DropDown,
    active_channel_row: gtk::Widget,
    channel_scope: gtk::DropDown,
    channel_panel_stack: gtk::Stack,
    channel_controls: Vec<ChannelControlWidgets>,
    aggregate_channel_controls: AggregateChannelControlWidgets,
    crosshatch_action: gtk::Button,
    crosshatch_note: gtk::Label,
    preview_surface: gtk::DropDown,
    preview_color: gtk::ColorDialogButton,
    export_background: gtk::DropDown,
    export_color_label: gtk::Label,
    export_color: gtk::ColorDialogButton,
    web_shared: gtk::CheckButton,
    web_shared_help: Option<HelpHandle>,
    web_shape: gtk::DropDown,
    web_shape_row: gtk::Widget,
    web_mixed_shape_label: gtk::Label,
    web_mixed_shape_apply: gtk::DropDown,
    web_mixed_shape_apply_row: gtk::Widget,
    web_polygon_sides: gtk::SpinButton,
    web_polygon_sides_row: gtk::Widget,
    web_polygon_sides_label: gtk::Label,
    web_edit_shape: gtk::Button,
    web_target: gtk::DropDown,
    web_target_label: gtk::Label,
    web_target_help: Option<HelpHandle>,
    web_visible_label: gtk::Label,
    web_visible_help: HelpHandle,
    web_visible: [gtk::CheckButton; 4],
    web_color: gtk::Entry,
    web_color_row: gtk::Widget,
    web_color_heading: gtk::Label,
    web_color_help: HelpHandle,
    web_crosshatch_color: gtk::Entry,
    web_crosshatch_color_row: gtk::Widget,
    web_color_status: gtk::Label,
    web_coverage: gtk::Scale,
    web_coverage_status: gtk::Label,
    web_angle: gtk::Scale,
    web_angle_status: gtk::Label,
    web_mark_angle: gtk::Scale,
    web_mark_angle_status: gtk::Label,
    web_width_scale: gtk::Scale,
    web_width_scale_status: gtk::Label,
    web_height_scale: gtk::Scale,
    web_height_scale_status: gtk::Label,
    web_threshold: gtk::Scale,
    web_threshold_status: gtk::Label,
    web_opacity: gtk::Scale,
    web_opacity_heading: gtk::Label,
    web_opacity_help: HelpHandle,
    web_opacity_status: gtk::Label,
    web_detail: gtk::Scale,
    web_detail_status: gtk::Label,
    web_mixed: gtk::Label,
    web_geometry_note: gtk::Label,
    curve_layout: gtk::DropDown,
    curve_profile: gtk::DropDown,
    curve_editor_label: gtk::Label,
    curve_editor: gtk::DrawingArea,
    curve_reset: gtk::Button,
    curve_shared: gtk::CheckButton,
    curve_shared_help: Option<HelpHandle>,
    curve_target: gtk::DropDown,
    curve_target_label: gtk::Label,
    curve_target_help: Option<HelpHandle>,
    curve_visible_label: gtk::Label,
    curve_visible_help: HelpHandle,
    curve_visible: [gtk::CheckButton; 4],
    curve_color: gtk::Entry,
    curve_color_row: gtk::Widget,
    curve_crosshatch_color: gtk::Entry,
    curve_crosshatch_color_row: gtk::Widget,
    curve_color_status: gtk::Label,
    curve_weight: gtk::Scale,
    curve_spacing: gtk::Scale,
    curve_coverage: gtk::Scale,
    curve_coverage_status: gtk::Label,
    curve_angle: gtk::Scale,
    curve_angle_status: gtk::Label,
    curve_position_x: gtk::Scale,
    curve_position_x_status: gtk::Label,
    curve_position_y: gtk::Scale,
    curve_position_y_status: gtk::Label,
    curve_opacity: gtk::Scale,
    curve_opacity_status: gtk::Label,
    curve_threshold: gtk::Scale,
    curve_threshold_status: gtk::Label,
    curve_detail: gtk::Scale,
    curve_detail_status: gtk::Label,
    curve_close_ends: gtk::CheckButton,
    curve_smooth_join: gtk::CheckButton,
    curve_mixed: gtk::Label,
    motif_controls: gtk::Widget,
    motif_coverage: gtk::DropDown,
    motif_size: gtk::Scale,
    motif_columns: gtk::Scale,
    motif_rows: gtk::Scale,
    motif_row_spacing: gtk::Scale,
    motif_stagger: gtk::Scale,
    motif_alternate: gtk::DropDown,
    motif_arrange: gtk::CheckButton,
    motif_mixed: gtk::Label,
    motif_overlay: gtk::DrawingArea,
    motif_drag: Cell<Option<MotifDrag>>,
    curve_selected_handle: Cell<i32>,
    curve_drag_start: Cell<Option<CurvePoint>>,
    compare: gtk::ToggleButton,
    fit: gtk::ToggleButton,
    zoom: gtk::Scale,
    zoom_entry: gtk::Entry,
    save: gtk::Button,
    undo: gtk::Button,
    redo: gtk::Button,
    export: gtk::Button,
    state: RefCell<AppState>,
    gate: Arc<RenderGate>,
    candidate_gate: Arc<RenderGate>,
    preset_gate: Arc<RenderGate>,
    render_requests: Arc<LatestSlot<RenderRequest>>,
    render_results: Arc<LatestSlot<RenderOutcome>>,
    autosave_requests: Arc<LatestSlot<AutosaveRequest>>,
    autosave_results: Arc<LatestSlot<AutosaveOutcome>>,
    autosave_generation: Arc<AtomicU64>,
    recovery_io_lock: Arc<Mutex<()>>,
    export_results: Arc<LatestSlot<ExportOutcome>>,
    export_running: Cell<bool>,
    preview_token: RefCell<Option<CancellationToken>>,
    export_token: RefCell<Option<CancellationToken>>,
    recovery_enabled: bool,
    close_approved: Cell<bool>,
    screenshot_path: Option<PathBuf>,
    export_path: Option<PathBuf>,
    png_export_path: Option<PathBuf>,
    save_artifact_path: Option<PathBuf>,
    save_treatment_path: Option<PathBuf>,
    cli_artifacts_written: Cell<bool>,
    cli_artifact_failed: Cell<bool>,
    capture_prepared: Cell<bool>,
    capture_attempts: Cell<u8>,
    preview_generation: Cell<u64>,
    zoom_settle_generation: Cell<u64>,
    fit_allocation: RefCell<FitAllocationState>,
    preset_pending: Cell<bool>,
    compare_source_artifact: bool,
    arrange_motif_artifact: bool,
    allocation_report_path: Option<PathBuf>,
    indicator_report_path: Option<PathBuf>,
    artifact_resize_window: Option<(i32, i32)>,
    artifact_resize_started: Cell<bool>,
    artifact_resize_before: Cell<Option<ArtifactAllocation>>,
    artifact_shape_editor: bool,
    artifact_controls_shown: bool,
    capture_override: RefCell<Option<gtk::Window>>,
    deferred_candidate_artifact: bool,
}

#[derive(Clone, Copy)]
enum WeightedVoronoiScalarSetting {
    DensityStrength,
    ResponseStrength,
    BoundaryGap,
}

struct ShellWidgets {
    window: adw::ApplicationWindow,
    stack: gtk::Stack,
    toast_overlay: adw::ToastOverlay,
    title: adw::WindowTitle,
    new_project: gtk::Button,
    open: gtk::Button,
    save: gtk::Button,
    undo: gtk::Button,
    redo: gtk::Button,
    controls_toggle: gtk::ToggleButton,
    export: gtk::Button,
}

struct InspectorHierarchyWidgets {
    source_section: gtk::Expander,
    output_section: gtk::Expander,
    channel_settings_section: gtk::Expander,
    channel_panel_stack: gtk::Stack,
}

#[derive(Clone)]
struct ChannelControlWidgets {
    channel: OutputChannelId,
    root: gtk::Box,
    heading: gtk::Label,
    inclusion_status: gtk::Label,
}

#[derive(Clone)]
struct AggregateChannelControlWidgets {
    root: gtk::Box,
    heading: gtk::Label,
    mixed_message: gtk::Label,
}

fn build_top_level_shell(
    builder: &gtk::Builder,
    application: &adw::Application,
    window_width: i32,
    window_height: i32,
) -> ShellWidgets {
    let window = builder
        .object::<adw::ApplicationWindow>("main_window")
        .expect("toniator-window.blp must define main_window");
    window.set_application(Some(application));
    window.set_default_size(window_width.max(720), window_height.max(520));

    ShellWidgets {
        window,
        stack: builder
            .object("main_stack")
            .expect("toniator-window.blp must define main_stack"),
        toast_overlay: builder
            .object("toast_overlay")
            .expect("toniator-window.blp must define toast_overlay"),
        title: builder
            .object("window_title")
            .expect("toniator-window.blp must define window_title"),
        new_project: builder
            .object("new_project_button")
            .expect("toniator-window.blp must define new_project_button"),
        open: builder
            .object("open_button")
            .expect("toniator-window.blp must define open_button"),
        save: builder
            .object("save_button")
            .expect("toniator-window.blp must define save_button"),
        undo: builder
            .object("undo_button")
            .expect("toniator-window.blp must define undo_button"),
        redo: builder
            .object("redo_button")
            .expect("toniator-window.blp must define redo_button"),
        controls_toggle: builder
            .object("controls_toggle")
            .expect("toniator-window.blp must define controls_toggle"),
        export: builder
            .object("export_button")
            .expect("toniator-window.blp must define export_button"),
    }
}

fn build_inspector_hierarchy(builder: &gtk::Builder) -> InspectorHierarchyWidgets {
    InspectorHierarchyWidgets {
        source_section: builder
            .object("source_section")
            .expect("toniator-window.blp must define source_section"),
        output_section: builder
            .object("output_section")
            .expect("toniator-window.blp must define output_section"),
        channel_settings_section: builder
            .object("channel_settings_section")
            .expect("toniator-window.blp must define channel_settings_section"),
        channel_panel_stack: builder
            .object("channel_panel_stack")
            .expect("toniator-window.blp must define channel_panel_stack"),
    }
}

fn channel_heading(channel: OutputChannelId) -> String {
    format!(
        "{} {}",
        channel.label(),
        if channel.output_model() == OutputModel::CmykPrint {
            "Ink"
        } else {
            "Channel"
        }
    )
}

fn build_channel_controls(channel: OutputChannelId) -> ChannelControlWidgets {
    let builder = gtk::Builder::from_resource(CHANNEL_CONTROLS_UI_RESOURCE);
    let root = builder
        .object::<gtk::Box>("channel_controls")
        .expect("toniator-channel-controls.blp must define channel_controls");
    let heading = builder
        .object::<gtk::Label>("channel_heading")
        .expect("toniator-channel-controls.blp must define channel_heading");
    let inclusion_status = builder
        .object::<gtk::Label>("channel_inclusion_status")
        .expect("toniator-channel-controls.blp must define channel_inclusion_status");
    let content_host = builder
        .object::<gtk::Box>("channel_content_host")
        .expect("toniator-channel-controls.blp must define channel_content_host");
    heading.set_text(&channel_heading(channel));
    content_host.append(
        &gtk::Label::builder()
            .label("Treatment edits apply to this real output channel when it is selected above.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label", "caption"])
            .build(),
    );
    ChannelControlWidgets {
        channel,
        root,
        heading,
        inclusion_status,
    }
}

fn build_aggregate_channel_controls() -> AggregateChannelControlWidgets {
    let builder = gtk::Builder::from_resource(AGGREGATE_CHANNEL_CONTROLS_UI_RESOURCE);
    let root = builder
        .object::<gtk::Box>("aggregate_channel_controls")
        .expect("toniator-aggregate-channel-controls.blp must define aggregate_channel_controls");
    let heading = builder
        .object::<gtk::Label>("aggregate_heading")
        .expect("toniator-aggregate-channel-controls.blp must define aggregate_heading");
    let mixed_message = builder
        .object::<gtk::Label>("aggregate_mixed_message")
        .expect("toniator-aggregate-channel-controls.blp must define aggregate_mixed_message");
    let content_host = builder
        .object::<gtk::Box>("aggregate_content_host")
        .expect("toniator-aggregate-channel-controls.blp must define aggregate_content_host");
    content_host.append(
        &gtk::Label::builder()
            .label("Treatment edits apply to every included ink or channel. Mixed values remain explicit.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label", "caption"])
            .build(),
    );
    AggregateChannelControlWidgets {
        root,
        heading,
        mixed_message,
    }
}

type TransitionContinuation = Rc<dyn Fn(&Rc<AppUi>)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirtyTransitionChoice {
    Cancel,
    Save,
    Discard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirtyTransitionAction {
    Prompt,
    Save,
    ClearRecovery,
    Continue,
    Stay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveTransitionOutcome {
    Saved,
    WriteFailed,
    RecoveryCleanupFailed,
}

/// The production state machine that decides whether a destructive document
/// transition may run. GTK dialogs and filesystem operations are adapters;
/// only `Continue` is allowed to reach `finish_new_project` (or another
/// transition continuation).
#[derive(Debug, Clone, Copy)]
struct DirtyTransitionCoordinator;

impl DirtyTransitionCoordinator {
    fn begin(dirty: bool) -> DirtyTransitionAction {
        if dirty {
            DirtyTransitionAction::Prompt
        } else {
            DirtyTransitionAction::Continue
        }
    }

    fn choose(choice: DirtyTransitionChoice) -> DirtyTransitionAction {
        match choice {
            DirtyTransitionChoice::Cancel => DirtyTransitionAction::Stay,
            DirtyTransitionChoice::Save => DirtyTransitionAction::Save,
            DirtyTransitionChoice::Discard => DirtyTransitionAction::ClearRecovery,
        }
    }

    fn save_finished(outcome: SaveTransitionOutcome) -> DirtyTransitionAction {
        match outcome {
            SaveTransitionOutcome::Saved => DirtyTransitionAction::Continue,
            SaveTransitionOutcome::WriteFailed | SaveTransitionOutcome::RecoveryCleanupFailed => {
                DirtyTransitionAction::Stay
            }
        }
    }

    fn cleanup_finished(success: bool) -> DirtyTransitionAction {
        if success {
            DirtyTransitionAction::Continue
        } else {
            DirtyTransitionAction::Stay
        }
    }
}

impl AppUi {
    pub fn new(application: &adw::Application, options: CliOptions) -> Rc<Self> {
        install_styles();

        let artifact_mode = options.artifact_mode();
        let load_example =
            options.loads_example() && options.artwork.is_none() && options.document.is_none();
        let recovery_enabled = !artifact_mode;
        let inspector_state_path = recovery_enabled.then(native_ui_state_path);
        let initial_inspector_width = options.artifact_inspector_width.map_or_else(
            || {
                inspector_state_path
                    .as_deref()
                    .map_or(INSPECTOR_DEFAULT_WIDTH, load_inspector_width)
            },
            |width| width.clamp(INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH),
        );
        let (window_width, window_height) = options.artifact_window_size.unwrap_or((1280, 820));
        let render_requests = Arc::new(LatestSlot::default());
        let render_results = Arc::new(LatestSlot::default());
        let worker_requests = Arc::clone(&render_requests);
        let worker_results = Arc::clone(&render_results);
        std::thread::Builder::new()
            .name("toniator-preview".into())
            .spawn(move || render_worker(worker_requests, worker_results))
            .expect("could not start preview worker");
        let autosave_requests = Arc::new(LatestSlot::default());
        let autosave_results = Arc::new(LatestSlot::default());
        let autosave_generation = Arc::new(AtomicU64::new(0));
        let recovery_io_lock = Arc::new(Mutex::new(()));
        if recovery_enabled {
            let worker_requests = Arc::clone(&autosave_requests);
            let worker_results = Arc::clone(&autosave_results);
            let worker_generation = Arc::clone(&autosave_generation);
            let worker_io_lock = Arc::clone(&recovery_io_lock);
            std::thread::Builder::new()
                .name("toniator-autosave".into())
                .spawn(move || {
                    autosave_worker(
                        worker_requests,
                        worker_results,
                        worker_generation,
                        worker_io_lock,
                    )
                })
                .expect("could not start autosave worker");
        }
        let export_results = Arc::new(LatestSlot::default());

        register_ui_resources();
        let builder = gtk::Builder::from_resource(WINDOW_UI_RESOURCE);
        if options.artifact_controls_shown {
            for (id, expanded) in [
                ("source_section", false),
                ("output_section", false),
                ("treatment_section", true),
            ] {
                builder
                    .object::<gtk::Expander>(id)
                    .unwrap_or_else(|| panic!("toniator-window.blp must define {id}"))
                    .set_expanded(expanded);
            }
        }

        let ShellWidgets {
            window,
            stack,
            toast_overlay,
            title,
            new_project: new,
            open,
            save,
            undo,
            redo,
            controls_toggle,
            export,
        } = build_top_level_shell(&builder, application, window_width, window_height);
        controls_toggle.update_property(&[
            gtk::accessible::Property::Label("Controls"),
            gtk::accessible::Property::Description("Hide Controls"),
        ]);

        let picture = builder
            .object::<gtk::Picture>("picture")
            .expect("toniator-window.blp must define picture");
        let source_label = builder
            .object::<gtk::Label>("source_label")
            .expect("toniator-window.blp must define source_label");
        let autosave_status = builder
            .object::<gtk::Label>("autosave_status")
            .expect("toniator-window.blp must define autosave_status");
        let compare = builder
            .object::<gtk::ToggleButton>("compare")
            .expect("toniator-window.blp must define compare");
        let preview_indicator = PreviewIndicator::new(options.indicator_phase());

        let start = build_start_view(&builder, recovery_enabled && recovery_path().exists());
        let editor_view = build_editor_view(
            &builder,
            &preview_indicator.area,
            recovery_enabled,
            initial_inspector_width,
            window_width,
        );
        stack.page(&start.container).set_name("start");
        stack.page(&editor_view.container).set_name("editor");
        stack.set_visible_child_name("start");
        let fit = editor_view.fit.clone();
        let zoom = editor_view.zoom.clone();
        let zoom_entry = editor_view.zoom_entry.clone();
        let inspector_pane = InspectorPaneController::new(
            &editor_view.paned,
            &controls_toggle,
            initial_inspector_width,
        );
        let sidebar_controller = inspector_pane.clone();
        controls_toggle.connect_toggled(move |button| {
            let collapsed = !button.is_active();
            sidebar_controller.set_collapsed(collapsed);
            let label = if collapsed {
                "Show Controls"
            } else {
                "Hide Controls"
            };
            button.set_tooltip_text(Some(label));
            button.update_property(&[gtk::accessible::Property::Description(label)]);
        });
        if options.artifact_controls_hidden {
            controls_toggle.set_active(false);
        }
        let layout_controller = inspector_pane.clone();
        editor_view
            .paned
            .connect_notify_local(Some("width"), move |_, _| {
                layout_controller.update_layout();
            });

        let ui = Rc::new(Self {
            window,
            open: open.clone(),
            stack,
            toast_overlay,
            title,
            new_project_button: new.clone(),
            controls_toggle: controls_toggle.clone(),
            picture,
            canvas: editor_view.canvas.clone(),
            canvas_content: editor_view.canvas_content.clone(),
            inspector_pane,
            #[cfg(test)]
            inspector_root: editor_view.inspector_root.clone(),
            source_label,
            preview_indicator,
            autosave_status,
            workspace_status: editor_view.workspace_status.clone(),
            cancel_preview: editor_view.cancel_preview.clone(),
            cancel_export: editor_view.cancel_export.clone(),
            editing_context: editor_view.editing_context.clone(),
            detail: editor_view.detail.clone(),
            coverage: editor_view.coverage.clone(),
            contrast: editor_view.contrast.clone(),
            angle: editor_view.angle.clone(),
            dots: editor_view.dots.clone(),
            squares: editor_view.squares.clone(),
            lines: editor_view.lines.clone(),
            curves: editor_view.curves.clone(),
            weighted_voronoi: editor_view.weighted_voronoi.clone(),
            legacy: editor_view.legacy.clone(),
            treatment_modes: editor_view.treatment_modes.clone(),
            weighted_voronoi_channel: editor_view.weighted_voronoi_channel.clone(),
            weighted_voronoi_cell_count: editor_view.weighted_voronoi_cell_count.clone(),
            weighted_voronoi_visible: editor_view.weighted_voronoi_visible.clone(),
            weighted_voronoi_arrangement: editor_view.weighted_voronoi_arrangement.clone(),
            weighted_voronoi_placement: editor_view.weighted_voronoi_placement.clone(),
            weighted_voronoi_density_strength: editor_view
                .weighted_voronoi_density_strength
                .clone(),
            weighted_voronoi_response_strength: editor_view
                .weighted_voronoi_response_strength
                .clone(),
            weighted_voronoi_boundary_gap: editor_view.weighted_voronoi_boundary_gap.clone(),
            weighted_voronoi_seed: editor_view.weighted_voronoi_seed.clone(),
            preset_import: editor_view.preset_import.clone(),
            preset_save: editor_view.preset_save.clone(),
            source_section: editor_view.source_section.clone(),
            output_section: editor_view.output_section.clone(),
            channel_settings_section: editor_view.channel_settings_section.clone(),
            output_mode: editor_view.output_mode.clone(),
            artwork_source: editor_view.artwork_source.clone(),
            artwork_source_note: editor_view.artwork_source_note.clone(),
            source_alpha: editor_view.source_alpha.clone(),
            source_alpha_row: editor_view.source_alpha_row.clone(),
            source_alpha_note: editor_view.source_alpha_note.clone(),
            channel_assignment: editor_view.channel_assignment.clone(),
            channel_assignment_note: editor_view.channel_assignment_note.clone(),
            active_channel: editor_view.active_channel.clone(),
            active_channel_row: editor_view.active_channel_row.clone(),
            channel_scope: editor_view.channel_scope.clone(),
            channel_panel_stack: editor_view.channel_panel_stack.clone(),
            channel_controls: editor_view.channel_controls.clone(),
            aggregate_channel_controls: editor_view.aggregate_channel_controls.clone(),
            crosshatch_action: editor_view.crosshatch_action.clone(),
            crosshatch_note: editor_view.crosshatch_note.clone(),
            preview_surface: editor_view.preview_surface.clone(),
            preview_color: editor_view.preview_color.clone(),
            export_background: editor_view.export_background.clone(),
            export_color_label: editor_view.export_color_label.clone(),
            export_color: editor_view.export_color.clone(),
            web_shared: editor_view.web_shared.clone(),
            web_shared_help: editor_view.web_shared_help.clone(),
            web_shape: editor_view.web_shape.clone(),
            web_shape_row: editor_view.web_shape_row.clone(),
            web_mixed_shape_label: editor_view.web_mixed_shape_label.clone(),
            web_mixed_shape_apply: editor_view.web_mixed_shape_apply.clone(),
            web_mixed_shape_apply_row: editor_view.web_mixed_shape_apply_row.clone(),
            web_polygon_sides: editor_view.web_polygon_sides.clone(),
            web_polygon_sides_row: editor_view.web_polygon_sides_row.clone(),
            web_polygon_sides_label: editor_view.web_polygon_sides_label.clone(),
            web_edit_shape: editor_view.web_edit_shape.clone(),
            web_target: editor_view.web_target.clone(),
            web_target_label: editor_view.web_target_label.clone(),
            web_target_help: editor_view.web_target_help.clone(),
            web_visible_label: editor_view.web_visible_label.clone(),
            web_visible_help: editor_view.web_visible_help.clone(),
            web_visible: editor_view.web_visible.clone(),
            web_color: editor_view.web_color.clone(),
            web_color_row: editor_view.web_color_row.clone(),
            web_color_heading: editor_view.web_color_heading.clone(),
            web_color_help: editor_view.web_color_help.clone(),
            web_crosshatch_color: editor_view.web_crosshatch_color.clone(),
            web_crosshatch_color_row: editor_view.web_crosshatch_color_row.clone(),
            web_color_status: editor_view.web_color_status.clone(),
            web_coverage: editor_view.web_coverage.clone(),
            web_coverage_status: editor_view.web_coverage_status.clone(),
            web_angle: editor_view.web_angle.clone(),
            web_angle_status: editor_view.web_angle_status.clone(),
            web_mark_angle: editor_view.web_mark_angle.clone(),
            web_mark_angle_status: editor_view.web_mark_angle_status.clone(),
            web_width_scale: editor_view.web_width_scale.clone(),
            web_width_scale_status: editor_view.web_width_scale_status.clone(),
            web_height_scale: editor_view.web_height_scale.clone(),
            web_height_scale_status: editor_view.web_height_scale_status.clone(),
            web_threshold: editor_view.web_threshold.clone(),
            web_threshold_status: editor_view.web_threshold_status.clone(),
            web_opacity: editor_view.web_opacity.clone(),
            web_opacity_heading: editor_view.web_opacity_heading.clone(),
            web_opacity_help: editor_view.web_opacity_help.clone(),
            web_opacity_status: editor_view.web_opacity_status.clone(),
            web_detail: editor_view.web_detail.clone(),
            web_detail_status: editor_view.web_detail_status.clone(),
            web_mixed: editor_view.web_mixed.clone(),
            web_geometry_note: editor_view.web_geometry_note.clone(),
            curve_layout: editor_view.curve_layout.clone(),
            curve_profile: editor_view.curve_profile.clone(),
            curve_editor_label: editor_view.curve_editor_label.clone(),
            curve_editor: editor_view.curve_editor.clone(),
            curve_reset: editor_view.curve_reset.clone(),
            curve_shared: editor_view.curve_shared.clone(),
            curve_shared_help: editor_view.curve_shared_help.clone(),
            curve_target: editor_view.curve_target.clone(),
            curve_target_label: editor_view.curve_target_label.clone(),
            curve_target_help: editor_view.curve_target_help.clone(),
            curve_visible_label: editor_view.curve_visible_label.clone(),
            curve_visible_help: editor_view.curve_visible_help.clone(),
            curve_visible: editor_view.curve_visible.clone(),
            curve_color: editor_view.curve_color.clone(),
            curve_color_row: editor_view.curve_color_row.clone(),
            curve_crosshatch_color: editor_view.curve_crosshatch_color.clone(),
            curve_crosshatch_color_row: editor_view.curve_crosshatch_color_row.clone(),
            curve_color_status: editor_view.curve_color_status.clone(),
            curve_weight: editor_view.curve_weight.clone(),
            curve_spacing: editor_view.curve_spacing.clone(),
            curve_coverage: editor_view.curve_coverage.clone(),
            curve_coverage_status: editor_view.curve_coverage_status.clone(),
            curve_angle: editor_view.curve_angle.clone(),
            curve_angle_status: editor_view.curve_angle_status.clone(),
            curve_position_x: editor_view.curve_position_x.clone(),
            curve_position_x_status: editor_view.curve_position_x_status.clone(),
            curve_position_y: editor_view.curve_position_y.clone(),
            curve_position_y_status: editor_view.curve_position_y_status.clone(),
            curve_opacity: editor_view.curve_opacity.clone(),
            curve_opacity_status: editor_view.curve_opacity_status.clone(),
            curve_threshold: editor_view.curve_threshold.clone(),
            curve_threshold_status: editor_view.curve_threshold_status.clone(),
            curve_detail: editor_view.curve_detail.clone(),
            curve_detail_status: editor_view.curve_detail_status.clone(),
            curve_close_ends: editor_view.curve_close_ends.clone(),
            curve_smooth_join: editor_view.curve_smooth_join.clone(),
            curve_mixed: editor_view.curve_mixed.clone(),
            motif_controls: editor_view.motif_controls.clone(),
            motif_coverage: editor_view.motif_coverage.clone(),
            motif_size: editor_view.motif_size.clone(),
            motif_columns: editor_view.motif_columns.clone(),
            motif_rows: editor_view.motif_rows.clone(),
            motif_row_spacing: editor_view.motif_row_spacing.clone(),
            motif_stagger: editor_view.motif_stagger.clone(),
            motif_alternate: editor_view.motif_alternate.clone(),
            motif_arrange: editor_view.motif_arrange.clone(),
            motif_mixed: editor_view.motif_mixed.clone(),
            motif_overlay: editor_view.motif_overlay.clone(),
            motif_drag: Cell::new(None),
            curve_selected_handle: Cell::new(-1),
            curve_drag_start: Cell::new(None),
            compare,
            fit,
            zoom,
            zoom_entry,
            save,
            undo,
            redo,
            export,
            state: RefCell::new(AppState {
                editor: None,
                path: None,
                syncing_controls: false,
                preview_size: None,
                compare_source: false,
                zoom_mode: ZoomMode::Fit(100.0),
                source_cache: None,
                rendered_cache: None,
            }),
            gate: Arc::new(RenderGate::default()),
            candidate_gate: Arc::new(RenderGate::default()),
            preset_gate: Arc::new(RenderGate::default()),
            render_requests,
            render_results,
            autosave_requests,
            autosave_results,
            autosave_generation,
            recovery_io_lock,
            export_results,
            export_running: Cell::new(false),
            preview_token: RefCell::new(None),
            export_token: RefCell::new(None),
            recovery_enabled,
            close_approved: Cell::new(false),
            screenshot_path: options.screenshot,
            export_path: options.export_svg,
            png_export_path: options.export_png,
            save_artifact_path: options.save_document,
            save_treatment_path: options.save_treatment,
            cli_artifacts_written: Cell::new(false),
            cli_artifact_failed: Cell::new(false),
            capture_prepared: Cell::new(false),
            capture_attempts: Cell::new(0),
            preview_generation: Cell::new(0),
            zoom_settle_generation: Cell::new(0),
            fit_allocation: RefCell::new(FitAllocationState::default()),
            preset_pending: Cell::new(false),
            compare_source_artifact: options.compare_source,
            arrange_motif_artifact: options.arrange_motif,
            allocation_report_path: options.allocation_report,
            indicator_report_path: options.indicator_report,
            artifact_resize_window: options.artifact_resize_window,
            artifact_resize_started: Cell::new(false),
            artifact_resize_before: Cell::new(None),
            artifact_shape_editor: options.edit_shape,
            artifact_controls_shown: options.artifact_controls_shown,
            capture_override: RefCell::new(None),
            deferred_candidate_artifact: options.artwork.is_some() || options.document.is_some(),
        });

        ui.connect_actions(new, open, start, editor_view);
        ui.update_actions();
        if load_example {
            ui.load_example();
            if options.demo_adjusted {
                ui.apply_demo_adjustment();
            }
            if let Some(path) = options.preset.as_ref() {
                ui.import_preset_path(path);
            } else if options.demo_curves {
                ui.activate_curve_treatment();
            }
            if options.compare_source && options.preset.is_none() {
                ui.compare.set_active(true);
            }
            if let Some(mapping) = options.source_mapping {
                if let Some(mapping) = source_mapping_from_index(mapping) {
                    // CLI artifacts retain their historic mapping vocabulary;
                    // the normal GTK inspector never routes through this adapter.
                    let document = {
                        let mut state = ui.state.borrow_mut();
                        state.editor.as_mut().and_then(|editor| {
                            editor
                                .apply_legacy_mapping_action(mapping)
                                .then(|| editor.document().clone())
                        })
                    };
                    if let Some(document) = document {
                        ui.after_treatment_edit(document);
                    }
                } else {
                    eprintln!("Artwork Mapping artifact index {mapping} is outside 0 through 4");
                }
            }
            if options.independent_shapes {
                ui.install_independent_shape_fixture();
            }
            if let Some(zoom) = options.artifact_zoom {
                ui.set_explicit_zoom(ZoomIntent::Entry(zoom * 100.0));
            }
            if options.edit_shape {
                ui.install_curved_shape_fixture();
                ui.open_shape_editor();
            } else if options.curved_shape {
                ui.install_curved_shape_fixture();
            }
            ui.apply_artifact_appearance(
                options.preview_surface.as_deref(),
                options.export_background.as_deref(),
                options.expand_document,
            );
        } else if let Some(path) = options.artwork.as_ref() {
            ui.import_artwork(path);
        } else if let Some(path) = options.document.as_ref() {
            ui.open_document_path(path);
        }
        ui
    }

    pub fn present(self: &Rc<Self>) {
        self.window.present();
        if self.artifact_controls_shown {
            glib::timeout_add_local_once(
                Duration::from_millis(120),
                glib::clone!(
                    #[weak(rename_to = ui)]
                    self,
                    move || {
                        ui.controls_toggle.set_active(true);
                    }
                ),
            );
        }
        if self.screenshot_path.is_some()
            && self.state.borrow().editor.is_none()
            && !self.deferred_candidate_artifact
        {
            glib::timeout_add_local_once(
                Duration::from_millis(700),
                glib::clone!(
                    #[weak(rename_to = ui)]
                    self,
                    move || ui.write_cli_artifacts()
                ),
            );
        }
    }

    pub fn cli_artifact_failed(&self) -> bool {
        self.cli_artifact_failed.get()
    }

    fn report_cli_artifact_error(&self, message: String) {
        self.cli_artifact_failed.set(true);
        eprintln!("{message}");
        self.show_error(&message);
    }

    fn finish_cli_artifact_failure(&self, message: String) {
        if self.cli_artifacts_written.replace(true) {
            return;
        }
        self.report_cli_artifact_error(message);
        if !self.recovery_enabled {
            self.close_approved.set(true);
            self.window.close();
        }
    }

    fn connect_actions(
        self: &Rc<Self>,
        new: gtk::Button,
        open: gtk::Button,
        start: StartWidgets,
        editor: EditorWidgets,
    ) {
        connect_clicked(&new, self, |ui| ui.new_project());
        connect_clicked(&open, self, |ui| ui.open_menu());
        connect_clicked(&self.save, self, |ui| ui.save_document());
        connect_clicked(&self.undo, self, |ui| ui.undo());
        connect_clicked(&self.redo, self, |ui| ui.redo());
        connect_clicked(&self.export, self, |ui| ui.export_document());
        connect_clicked(&self.cancel_preview, self, |ui| ui.cancel_preview());
        connect_clicked(&self.cancel_export, self, |ui| ui.cancel_export());
        connect_clicked(&start.open_artwork, self, |ui| ui.open_artwork_dialog());
        connect_clicked(&start.open_document, self, |ui| ui.open_document_dialog());
        connect_clicked(&start.try_example, self, |ui| ui.request_example());
        if let Some(recover) = start.recover {
            connect_clicked(&recover, self, |ui| ui.recover_document());
        }
        connect_zoom_control_commands(
            &editor.fit,
            &editor.zoom_out,
            &editor.zoom,
            &editor.zoom_entry,
            &editor.zoom_in,
            Rc::new(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |command| {
                    if ui.state.borrow().syncing_controls {
                        return;
                    }
                    match command {
                        ZoomControlCommand::Fit => ui.set_fit(),
                        ZoomControlCommand::Manual(intent) => ui.set_explicit_zoom(intent),
                        ZoomControlCommand::Entry(text) => ui.commit_zoom_text(&text),
                    }
                }
            )),
        );
        self.canvas.add_tick_callback(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[upgrade_or]
            glib::ControlFlow::Break,
            move |_, _| {
                ui.inspector_pane.maintain();
                if matches!(ui.state.borrow().zoom_mode, ZoomMode::Fit(_)) {
                    ui.apply_fit_zoom();
                }
                glib::ControlFlow::Continue
            }
        ));
        self.compare.connect_toggled(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| {
                ui.state.borrow_mut().compare_source = button.is_active();
                ui.select_preview_view();
            }
        ));

        self.dots.connect_toggled(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| if button.is_active() && !ui.state.borrow().syncing_controls {
                ui.activate_shape_treatment();
            }
        ));
        self.curves.connect_toggled(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| {
                if button.is_active() && !ui.state.borrow().syncing_controls {
                    ui.activate_curve_treatment();
                }
            }
        ));
        self.weighted_voronoi.connect_toggled(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| {
                if button.is_active() && !ui.state.borrow().syncing_controls {
                    ui.activate_weighted_voronoi_treatment();
                }
            }
        ));
        self.weighted_voronoi_channel
            .connect_selected_notify(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |_| if !ui.state.borrow().syncing_controls {
                    // This is presentation-only selection for a persisted
                    // semantic channel; it does not mutate the document.
                    ui.sync_controls();
                }
            ));
        for (index, button) in self.weighted_voronoi_visible.iter().enumerate() {
            button.connect_toggled(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |button| if !ui.state.borrow().syncing_controls {
                    let enabled = button.is_active();
                    let Some(channel) = ui.weighted_voronoi_channel_at(index) else {
                        return;
                    };
                    ui.change_weighted_voronoi_settings_for_channel(
                        channel,
                        move |settings, channel| {
                            settings
                                .channel_settings_mut(channel)
                                .expect("registered semantic channel")
                                .enabled = enabled;
                        },
                    );
                }
            ));
        }
        self.weighted_voronoi_cell_count
            .connect_value_changed(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |control| if !ui.state.borrow().syncing_controls {
                    let value = control.value().round() as u32;
                    ui.change_weighted_voronoi_settings(move |settings, channel| {
                        settings
                            .channel_settings_mut(channel)
                            .expect("registered semantic channel")
                            .cell_count = value;
                    });
                }
            ));
        self.weighted_voronoi_arrangement
            .connect_selected_notify(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |control| if !ui.state.borrow().syncing_controls {
                    let arrangement = if control.selected() == 1 {
                        WeightedVoronoiArrangementPolicy::Independent
                    } else {
                        WeightedVoronoiArrangementPolicy::Shared
                    };
                    ui.change_weighted_voronoi_settings(move |settings, channel| {
                        settings
                            .channel_settings_mut(channel)
                            .expect("registered semantic channel")
                            .arrangement = arrangement;
                    });
                }
            ));
        self.weighted_voronoi_placement
            .connect_selected_notify(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |control| if !ui.state.borrow().syncing_controls {
                    let placement = if control.selected() == 1 {
                        WeightedVoronoiPlacementMode::Uniform
                    } else {
                        WeightedVoronoiPlacementMode::SourceWeighted
                    };
                    ui.change_weighted_voronoi_settings(move |settings, channel| {
                        settings
                            .channel_settings_mut(channel)
                            .expect("registered semantic channel")
                            .placement = placement;
                    });
                }
            ));
        for (control, update) in [
            (
                self.weighted_voronoi_density_strength.clone(),
                WeightedVoronoiScalarSetting::DensityStrength,
            ),
            (
                self.weighted_voronoi_response_strength.clone(),
                WeightedVoronoiScalarSetting::ResponseStrength,
            ),
            (
                self.weighted_voronoi_boundary_gap.clone(),
                WeightedVoronoiScalarSetting::BoundaryGap,
            ),
        ] {
            control.connect_value_changed(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |control| if !ui.state.borrow().syncing_controls {
                    let value = control.value();
                    ui.change_weighted_voronoi_settings(move |settings, channel| {
                        let settings = settings
                            .channel_settings_mut(channel)
                            .expect("registered semantic channel");
                        match update {
                            WeightedVoronoiScalarSetting::DensityStrength => {
                                settings.density_strength = value
                            }
                            WeightedVoronoiScalarSetting::ResponseStrength => {
                                settings.response_strength = value
                            }
                            WeightedVoronoiScalarSetting::BoundaryGap => {
                                settings.boundary_gap = value
                            }
                        }
                    });
                }
            ));
        }
        self.weighted_voronoi_seed.connect_activate(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |entry| ui.apply_weighted_voronoi_seed(entry)
        ));
        let seed_focus = gtk::EventControllerFocus::new();
        seed_focus.connect_leave(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_| ui.apply_weighted_voronoi_seed(&ui.weighted_voronoi_seed)
        ));
        self.weighted_voronoi_seed.add_controller(seed_focus);
        self.connect_scale(&self.detail, SettingKey::Detail, |settings, value| {
            settings.detail = value
        });
        self.connect_scale(&self.coverage, SettingKey::Coverage, |settings, value| {
            settings.coverage = value
        });
        self.connect_scale(&self.contrast, SettingKey::Contrast, |settings, value| {
            settings.contrast = value
        });
        self.connect_scale(&self.angle, SettingKey::Angle, |settings, value| {
            settings.angle = value
        });
        self.connect_slider_gesture(&self.detail, SettingKey::Detail);
        self.connect_slider_gesture(&self.coverage, SettingKey::Coverage);
        self.connect_slider_gesture(&self.contrast, SettingKey::Contrast);
        self.connect_slider_gesture(&self.angle, SettingKey::Angle);
        self.preset_import.connect_clicked(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| ui.open_preset_dialog(button.upcast_ref())
        ));
        connect_clicked(&self.preset_save, self, |ui| ui.save_treatment_dialog());
        self.preview_surface.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |control| if !ui.state.borrow().syncing_controls {
                ui.update_appearance(|appearance| {
                    appearance.preview_surface = if control.selected() == 0 {
                        PreviewSurface::Checkerboard
                    } else {
                        PreviewSurface::Color {
                            color: rgba_color(ui.preview_color.rgba()),
                        }
                    };
                });
            }
        ));
        self.export_background.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |control| if !ui.state.borrow().syncing_controls {
                ui.update_appearance(|appearance| {
                    appearance.export_background = export_background_from_selection(
                        control.selected(),
                        appearance.export_background,
                    );
                });
            }
        ));
        self.preview_color.connect_rgba_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| if !ui.state.borrow().syncing_controls {
                let color = rgba_color(button.rgba());
                ui.update_appearance(|appearance| {
                    appearance.preview_surface = PreviewSurface::Color { color }
                });
            }
        ));
        self.export_color.connect_rgba_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| if !ui.state.borrow().syncing_controls {
                let color = rgba_color(button.rgba());
                ui.update_appearance(|appearance| {
                    appearance.export_background = ExportBackground::Color { color }
                });
            }
        ));
        self.output_mode.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |control| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let mode = if control.selected() == 1 {
                    OutputMode::RgbScreen
                } else {
                    OutputMode::CmykInks
                };
                let document = {
                    let mut state = ui.state.borrow_mut();
                    let Some(editor) = state.editor.as_mut() else {
                        return;
                    };
                    if !editor.set_output_mode(mode) {
                        return;
                    }
                    editor.document().clone()
                };
                ui.after_output_mode_edit(document);
            }
        ));
        self.web_target.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_| if !ui.state.borrow().syncing_controls {
                ui.sync_controls_when_idle();
            }
        ));
        self.artwork_source.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |control| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let Some(source) = artwork_source_from_index(control.selected()) else {
                    return;
                };
                let document = {
                    let mut state = ui.state.borrow_mut();
                    let Some(editor) = state.editor.as_mut() else {
                        return;
                    };
                    let pipeline = pipeline_for_source(&editor.document().artwork_pipeline, source);
                    if !editor.set_artwork_pipeline(pipeline) {
                        return;
                    }
                    editor.document().clone()
                };
                ui.after_treatment_edit(document);
            }
        ));
        self.source_alpha.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |control| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let Some(alpha_policy) = source_alpha_from_index(control.selected()) else {
                    return;
                };
                let document = {
                    let mut state = ui.state.borrow_mut();
                    let Some(editor) = state.editor.as_mut() else {
                        return;
                    };
                    let mut pipeline = editor.document().artwork_pipeline.clone();
                    pipeline.alpha_policy = alpha_policy;
                    if !editor.set_artwork_pipeline(pipeline) {
                        return;
                    }
                    editor.document().clone()
                };
                ui.after_treatment_edit(document);
            }
        ));
        self.channel_assignment
            .connect_selected_notify(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |control| {
                    if ui.state.borrow().syncing_controls {
                        return;
                    }
                    let document = {
                        let mut state = ui.state.borrow_mut();
                        let Some(editor) = state.editor.as_mut() else {
                            return;
                        };
                        let current = editor.document().artwork_pipeline.clone();
                        let Some(assignment) =
                            channel_assignment_from_index(control.selected(), current.source)
                        else {
                            return;
                        };
                        let pipeline = pipeline_for_assignment(&current, assignment);
                        if !editor.set_artwork_pipeline(pipeline) {
                            return;
                        }
                        editor.document().clone()
                    };
                    ui.after_treatment_edit(document);
                }
            ));
        self.active_channel.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |control| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let output = ui
                    .state
                    .borrow()
                    .editor
                    .as_ref()
                    .map(|editor| editor.document().artwork_pipeline.output_model);
                let Some(channel) = output.and_then(|output| {
                    OutputChannelId::from_legacy_slot(control.selected(), output).ok()
                }) else {
                    return;
                };
                let document = {
                    let mut state = ui.state.borrow_mut();
                    let Some(editor) = state.editor.as_mut() else {
                        return;
                    };
                    let mut pipeline = editor.document().artwork_pipeline.clone();
                    if !matches!(pipeline.assignment, ChannelAssignment::ActiveChannel) {
                        return;
                    }
                    pipeline.active_channel = Some(channel);
                    if !editor.set_artwork_pipeline(pipeline) {
                        return;
                    }
                    editor.document().clone()
                };
                ui.after_treatment_edit(document);
            }
        ));
        self.channel_scope.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |control| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let Some((output_model, crosshatch)) =
                    ui.state.borrow().editor.as_ref().map(|editor| {
                        let pipeline = &editor.document().artwork_pipeline;
                        (
                            pipeline.output_model,
                            matches!(
                                pipeline.assignment,
                                ChannelAssignment::LegacyCompatibility(
                                    LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1
                                )
                            ),
                        )
                    })
                else {
                    return;
                };
                if crosshatch {
                    return;
                }
                let Some(scope_channel) = channel_scope_channel(control.selected(), output_model)
                else {
                    return;
                };
                let Some(target) = channel_scope_target_index(scope_channel, output_model) else {
                    return;
                };
                ui.state.borrow_mut().syncing_controls = true;
                ui.web_target.set_selected(target);
                ui.curve_target.set_selected(target);
                ui.state.borrow_mut().syncing_controls = false;
                // Target selection changes the projected treatment controls,
                // not the document pipeline. Reuse the established mixed-value
                // synchronization without creating a document edit.
                ui.sync_controls();
            }
        ));
        connect_clicked(&self.crosshatch_action, self, |ui| {
            let document = {
                let mut state = ui.state.borrow_mut();
                let Some(editor) = state.editor.as_mut() else {
                    return;
                };
                let changed = if matches!(
                    editor.document().artwork_pipeline.assignment,
                    ChannelAssignment::LegacyCompatibility(
                        LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1
                    )
                ) {
                    editor.exit_crosshatch_treatment()
                } else {
                    editor.apply_legacy_mapping_action(ValueMode::CrosshatchLuminance)
                };
                if !changed {
                    return;
                }
                editor.document().clone()
            };
            ui.after_treatment_edit(document);
        });
        self.web_shared.connect_toggled(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                if !button.is_active() {
                    let Some((rgb, crosshatch)) = ui.web_output_flags() else {
                        return;
                    };
                    ui.change_web_treatment(|settings, _| {
                        let path = settings.resolved_custom_shape_path();
                        for ink in output_channel_order(rgb, crosshatch).iter().copied() {
                            let channel = settings.channels.get_mut(ink);
                            channel.shape = settings.shared_shape;
                            channel.polygon_sides = settings.polygon_sides;
                            channel.custom_shape_path = Some(path.clone());
                        }
                        settings.use_shared_mark = false;
                    });
                } else {
                    ui.enable_shared_shape();
                }
            }
        ));
        self.web_shape.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |combo| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let shape = match combo.selected() {
                    0 => WebShape::Circle,
                    1 => WebShape::RegularPolygon,
                    2 => WebShape::UserDefined,
                    _ => return,
                };
                let target = ui.web_target.selected();
                let target_ink = ui
                    .selected_web_inks()
                    .and_then(|inks| inks.first().copied());
                let Some((rgb, crosshatch)) = ui.web_output_flags() else {
                    return;
                };
                ui.change_web_treatment(move |settings, _| {
                    if settings.use_shared_mark {
                        settings.shared_shape = shape;
                    } else if target == 0 {
                        let path = settings.resolved_custom_shape_path();
                        let polygon_sides = settings.polygon_sides;
                        for ink in output_channel_order(rgb, crosshatch).iter().copied() {
                            let channel = settings.channels.get_mut(ink);
                            channel.shape = shape;
                            channel.polygon_sides = polygon_sides;
                            if shape == WebShape::UserDefined {
                                channel.custom_shape_path = Some(path.clone());
                            }
                        }
                    } else if let Some(ink) = target_ink {
                        settings.channels.get_mut(ink).shape = shape;
                    }
                });
            }
        ));
        self.web_mixed_shape_apply
            .connect_selected_notify(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |combo| {
                    if ui.state.borrow().syncing_controls || combo.selected() == 0 {
                        return;
                    }
                    let shape = match combo.selected() {
                        1 => WebShape::Circle,
                        2 => WebShape::RegularPolygon,
                        3 => WebShape::UserDefined,
                        _ => return,
                    };
                    let Some((rgb, crosshatch)) = ui.web_output_flags() else {
                        return;
                    };
                    ui.change_web_treatment(move |settings, _| {
                        settings.use_shared_mark = false;
                        let path = settings.resolved_custom_shape_path();
                        let polygon_sides = settings.polygon_sides;
                        for ink in output_channel_order(rgb, crosshatch).iter().copied() {
                            let channel = settings.channels.get_mut(ink);
                            channel.shape = shape;
                            channel.polygon_sides = polygon_sides;
                            if shape == WebShape::UserDefined {
                                channel.custom_shape_path = Some(path.clone());
                            }
                        }
                    });
                }
            ));
        self.web_polygon_sides.connect_value_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |spin| if !ui.state.borrow().syncing_controls {
                let sides = spin.value_as_int().clamp(3, 6) as u8;
                let target = ui.web_target.selected();
                let target_ink = ui
                    .selected_web_inks()
                    .and_then(|inks| inks.first().copied());
                let Some((rgb, crosshatch)) = ui.web_output_flags() else {
                    return;
                };
                ui.change_web_treatment(move |settings, _| {
                    if settings.use_shared_mark || target == 0 {
                        settings.polygon_sides = sides;
                        for ink in output_channel_order(rgb, crosshatch).iter().copied() {
                            settings.channels.get_mut(ink).polygon_sides = sides;
                        }
                    } else if let Some(ink) = target_ink {
                        settings.channels.get_mut(ink).polygon_sides = sides;
                    }
                });
            }
        ));
        connect_clicked(&self.web_edit_shape, self, |ui| ui.open_shape_editor());
        for (index, button) in self.web_visible.iter().enumerate() {
            button.connect_toggled(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |button| {
                    if ui.state.borrow().syncing_controls {
                        return;
                    }
                    let Some((rgb, crosshatch)) = ui.web_output_flags() else {
                        return;
                    };
                    let Some(ink) = visible_ink_for_slot(index, rgb, crosshatch) else {
                        return;
                    };
                    let visible = button.is_active();
                    ui.change_web_treatment(move |settings, _| {
                        settings.channels.get_mut(ink).enabled = visible
                    });
                }
            ));
        }
        self.web_color.connect_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |entry| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let color = entry.text().to_string();
                if toniator::model::parse_hex_color(&color).is_none() {
                    return;
                }
                ui.change_web_treatment(move |settings, inks| {
                    for ink in inks {
                        settings.channels.get_mut(ink).color.clone_from(&color);
                    }
                });
            }
        ));
        self.web_crosshatch_color.connect_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |entry| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let color = entry.text().to_string();
                if toniator::model::parse_hex_color(&color).is_some() {
                    ui.change_web_treatment(move |settings, _| settings.crosshatch_color = color);
                }
            }
        ));
        let color_focus = gtk::EventControllerFocus::new();
        color_focus.connect_enter(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_| ui.begin_setting_edit(SettingKey::WebColor)
        ));
        color_focus.connect_leave(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_| {
                let color = ui.web_color.text();
                if !color.is_empty() && toniator::model::parse_hex_color(&color).is_none() {
                    ui.show_error(web_color_validation_message(ui.web_uses_channel_copy()));
                    ui.sync_controls();
                }
                ui.end_setting_edit();
            }
        ));
        self.web_color.add_controller(color_focus);
        self.curve_layout.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |combo| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let layout = if combo.selected() == 1 {
                    CurveLayout::MotifPattern
                } else {
                    CurveLayout::FullWidth
                };
                ui.change_curve_treatment(move |settings, _| settings.layout = layout);
            }
        ));
        self.curve_target.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_| if !ui.state.borrow().syncing_controls {
                ui.sync_controls_when_idle();
            }
        ));
        self.curve_profile.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |combo| {
                if ui.state.borrow().syncing_controls || combo.selected() >= 3 {
                    return;
                }
                let path = match combo.selected() {
                    0 => CurvePath::straight(),
                    1 => CurvePath::soft_wave(),
                    2 => CurvePath::deep_wave(),
                    _ => return,
                };
                ui.apply_curve_profile(path);
            }
        ));
        connect_clicked(&self.curve_reset, self, |ui| {
            let Some((_, crosshatch)) = ui.curve_output_flags() else {
                return;
            };
            if crosshatch {
                ui.reset_crosshatch_path();
            } else {
                ui.apply_curve_profile(CurvePath::soft_wave());
            }
        });
        self.curve_shared.connect_toggled(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let shared = button.is_active();
                let Some((rgb, crosshatch)) = ui.curve_output_flags() else {
                    return;
                };
                ui.change_curve_treatment(move |settings, inks| {
                    if shared && !settings.use_shared_curve {
                        let ink = inks.first().copied().unwrap_or(Ink::Black);
                        settings.shared_path = settings.channels.get(ink).path.clone();
                        settings.shared_close_ends = settings.channels.get(ink).close_ends;
                        settings.shared_smooth_join = settings.channels.get(ink).smooth_join;
                    } else if !shared && settings.use_shared_curve {
                        for ink in output_channel_order(rgb, crosshatch).iter().copied() {
                            let channel = settings.channels.get_mut(ink);
                            channel.path = settings.shared_path.clone();
                            channel.close_ends = settings.shared_close_ends;
                            channel.smooth_join = settings.shared_smooth_join;
                        }
                    }
                    settings.use_shared_curve = shared;
                });
            }
        ));
        for (index, button) in self.curve_visible.iter().enumerate() {
            button.connect_toggled(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |button| {
                    if ui.state.borrow().syncing_controls {
                        return;
                    }
                    let Some((rgb, crosshatch)) = ui.curve_output_flags() else {
                        return;
                    };
                    let Some(ink) = visible_ink_for_slot(index, rgb, crosshatch) else {
                        return;
                    };
                    let visible = button.is_active();
                    ui.change_curve_treatment(move |settings, _| {
                        settings.channels.get_mut(ink).enabled = visible
                    });
                }
            ));
        }
        self.curve_color.connect_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |entry| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let color = entry.text().to_string();
                if toniator::model::parse_hex_color(&color).is_none() {
                    return;
                }
                ui.change_curve_treatment(move |settings, inks| {
                    for ink in inks {
                        settings.channels.get_mut(ink).color.clone_from(&color);
                    }
                });
            }
        ));
        self.curve_crosshatch_color.connect_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |entry| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let color = entry.text().to_string();
                if toniator::model::parse_hex_color(&color).is_some() {
                    ui.change_curve_treatment(move |settings, _| settings.crosshatch_color = color);
                }
            }
        ));
        let curve_color_focus = gtk::EventControllerFocus::new();
        curve_color_focus.connect_enter(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_| ui.begin_setting_edit(SettingKey::CurveColor)
        ));
        curve_color_focus.connect_leave(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_| {
                let color = ui.curve_color.text();
                if !color.is_empty() && toniator::model::parse_hex_color(&color).is_none() {
                    ui.show_error("Use a six-digit hex ink color such as #111111");
                    ui.sync_controls();
                }
                ui.end_setting_edit();
            }
        ));
        self.curve_color.add_controller(curve_color_focus);
        self.curve_weight.connect_value_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |scale| if !ui.state.borrow().syncing_controls {
                let value = scale.value();
                ui.change_curve_treatment(move |settings, _| settings.max_mark = value);
            }
        ));
        self.connect_slider_gesture(&self.curve_weight, SettingKey::CurveWeight);
        self.curve_spacing.connect_value_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |scale| if !ui.state.borrow().syncing_controls {
                let value = scale.value();
                ui.change_curve_treatment(move |settings, _| settings.long_edge_cells = value);
            }
        ));
        self.connect_slider_gesture(&self.curve_spacing, SettingKey::CurveSpacing);
        self.connect_curve_scale(
            &self.curve_coverage,
            SettingKey::CurveCoverage,
            |channel| channel.scale,
            |channel, value| channel.scale = value,
        );
        self.connect_curve_scale(
            &self.curve_angle,
            SettingKey::CurveAngle,
            |channel| channel.grid_rotation,
            |channel, value| channel.grid_rotation = value,
        );
        self.connect_curve_scale(
            &self.curve_position_x,
            SettingKey::CurvePositionX,
            |channel| channel.offset_x,
            |channel, value| channel.offset_x = value,
        );
        self.connect_curve_scale(
            &self.curve_position_y,
            SettingKey::CurvePositionY,
            |channel| channel.offset_y,
            |channel, value| channel.offset_y = value,
        );
        self.connect_curve_scale(
            &self.curve_opacity,
            SettingKey::CurveOpacity,
            |channel| channel.opacity,
            |channel, value| channel.opacity = value,
        );
        self.connect_curve_scale(
            &self.curve_threshold,
            SettingKey::CurveThreshold,
            |channel| channel.threshold,
            |channel, value| channel.threshold = value,
        );
        self.connect_curve_scale(
            &self.curve_detail,
            SettingKey::CurveDetail,
            |channel| channel.resolution_scale,
            |channel, value| channel.resolution_scale = value,
        );
        self.motif_coverage.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |combo| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let coverage = if combo.selected() == 0 {
                    MotifCoverage::Auto
                } else {
                    MotifCoverage::Manual
                };
                let all = ui.curve_target.selected() == 0;
                ui.change_curve_treatment(move |settings, inks| {
                    if all {
                        settings.base_channel.motif_coverage = coverage;
                    }
                    for ink in inks {
                        settings.channels.get_mut(ink).motif_coverage = coverage;
                    }
                });
            }
        ));
        self.connect_curve_scale(
            &self.motif_size,
            SettingKey::MotifSize,
            |channel| channel.curve_scale,
            |channel, value| channel.curve_scale = value,
        );
        self.connect_curve_scale(
            &self.motif_columns,
            SettingKey::MotifColumns,
            |channel| channel.tile_count as f64,
            |channel, value| channel.tile_count = value.round().clamp(1.0, 10_000.0) as u32,
        );
        self.connect_curve_scale(
            &self.motif_rows,
            SettingKey::MotifRows,
            |channel| channel.stack_count as f64,
            |channel, value| channel.stack_count = value.round().clamp(1.0, 10_000.0) as u32,
        );
        self.connect_curve_scale(
            &self.motif_row_spacing,
            SettingKey::MotifRowSpacing,
            |channel| channel.stack_spacing,
            |channel, value| channel.stack_spacing = value,
        );
        self.connect_curve_scale(
            &self.motif_stagger,
            SettingKey::MotifStagger,
            |channel| channel.alternate_stack_offset,
            |channel, value| channel.alternate_stack_offset = value,
        );
        self.motif_alternate.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |combo| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let transform = match combo.selected() {
                    1 => AlternateTileTransform::Flip,
                    2 => AlternateTileTransform::Rotate180,
                    _ => AlternateTileTransform::None,
                };
                let all = ui.curve_target.selected() == 0;
                ui.change_curve_treatment(move |settings, inks| {
                    if all {
                        settings.base_channel.alternate_tile_transform = transform;
                    }
                    for ink in inks {
                        settings.channels.get_mut(ink).alternate_tile_transform = transform;
                    }
                });
            }
        ));
        self.motif_arrange.connect_toggled(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_| ui.sync_motif_overlay()
        ));
        self.curve_close_ends.connect_toggled(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let active = button.is_active();
                ui.change_curve_treatment(move |settings, inks| {
                    if settings.use_shared_curve {
                        settings.shared_close_ends = active;
                    } else {
                        for ink in inks {
                            settings.channels.get_mut(ink).close_ends = active;
                        }
                    }
                });
            }
        ));
        self.curve_smooth_join.connect_toggled(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let active = button.is_active();
                ui.change_curve_treatment(move |settings, inks| {
                    if settings.use_shared_curve {
                        settings.shared_smooth_join = active;
                    } else {
                        for ink in inks {
                            settings.channels.get_mut(ink).smooth_join = active;
                        }
                    }
                });
            }
        ));
        self.connect_curve_editor();
        self.connect_motif_overlay();
        self.connect_web_scale(
            &self.web_coverage,
            SettingKey::WebCoverage,
            |channel| channel.scale,
            |channel, value| channel.scale = value,
        );
        self.connect_web_scale(
            &self.web_angle,
            SettingKey::WebAngle,
            |channel| channel.grid_rotation,
            |channel, value| channel.grid_rotation = value,
        );
        self.connect_web_scale(
            &self.web_mark_angle,
            SettingKey::WebMarkAngle,
            |channel| channel.rotation,
            |channel, value| channel.rotation = value,
        );
        self.connect_web_scale(
            &self.web_width_scale,
            SettingKey::WebWidthScale,
            |channel| channel.width_scale,
            |channel, value| channel.width_scale = value,
        );
        self.connect_web_scale(
            &self.web_height_scale,
            SettingKey::WebHeightScale,
            |channel| channel.height_scale,
            |channel, value| channel.height_scale = value,
        );
        self.connect_web_scale(
            &self.web_threshold,
            SettingKey::WebThreshold,
            |channel| channel.threshold,
            |channel, value| channel.threshold = value,
        );
        self.connect_web_scale(
            &self.web_opacity,
            SettingKey::WebOpacity,
            |channel| channel.opacity,
            |channel, value| channel.opacity = value,
        );
        self.connect_web_scale(
            &self.web_detail,
            SettingKey::WebDetail,
            |channel| channel.resolution_scale,
            |channel, value| channel.resolution_scale = value,
        );

        let drop_target = gtk::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);
        drop_target.connect_drop(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[upgrade_or]
            false,
            move |_, value, _, _| {
                let Ok(file) = value.get::<gio::File>() else {
                    return false;
                };
                let Some(path) = file.path() else {
                    return false;
                };
                ui.open_path(&path);
                true
            }
        ));
        self.window.add_controller(drop_target);

        self.window.connect_close_request(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_| {
                match close_policy(
                    ui.export_running.get(),
                    ui.close_approved.get(),
                    ui.has_dirty_document(),
                ) {
                    ClosePolicy::InhibitExport => {
                        ui.show_message(EXPORT_CLOSE_INHIBIT_MESSAGE);
                        return glib::Propagation::Stop;
                    }
                    ClosePolicy::Proceed => return glib::Propagation::Proceed,
                    ClosePolicy::CheckDirty => {}
                }
                if !ui.flush_recovery_sync() {
                    return glib::Propagation::Stop;
                }
                ui.gate_dirty_transition(|ui| {
                    ui.close_approved.set(true);
                    ui.window.close();
                });
                glib::Propagation::Stop
            }
        ));

        self.install_shortcuts();

        glib::timeout_add_local(
            Duration::from_millis(20),
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move || {
                    ui.poll_render_results();
                    ui.poll_autosave_results();
                    ui.poll_export_results();
                    glib::ControlFlow::Continue
                }
            ),
        );
    }

    fn install_shortcuts(self: &Rc<Self>) {
        let Some(application) = self.window.application() else {
            return;
        };
        for (name, accelerators, callback) in [
            (
                "new",
                &["<primary>n"][..],
                Self::new_project as fn(&Rc<Self>),
            ),
            (
                "open",
                &["<primary>o"][..],
                Self::open_artwork_dialog as fn(&Rc<Self>),
            ),
            ("save", &["<primary>s"][..], Self::save_document),
            ("undo", &["<primary>z"][..], Self::undo),
            ("redo", &["<primary><shift>z", "<primary>y"][..], Self::redo),
            ("export", &["<primary>e"][..], Self::export_document),
        ] {
            let action = gio::SimpleAction::new(name, None);
            action.connect_activate(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |_, _| callback(&ui)
            ));
            self.window.add_action(&action);
            application.set_accels_for_action(&format!("win.{name}"), accelerators);
        }
        for (name, accelerators, callback) in [
            (
                "zoom-in",
                &["<primary>plus", "<primary>equal"][..],
                ZoomIntent::Increase,
            ),
            ("zoom-out", &["<primary>minus"][..], ZoomIntent::Decrease),
        ] {
            let action = gio::SimpleAction::new(name, None);
            action.connect_activate(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |_, _| ui.set_explicit_zoom(callback)
            ));
            self.window.add_action(&action);
            application.set_accels_for_action(&format!("win.{name}"), accelerators);
        }
        let fit = gio::SimpleAction::new("zoom-fit", None);
        fit.connect_activate(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, _| ui.set_fit()
        ));
        self.window.add_action(&fit);
        application.set_accels_for_action("win.zoom-fit", &["<primary>0"]);
        let controls = gio::SimpleAction::new("toggle-controls", None);
        controls.connect_activate(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, _| {
                ui.controls_toggle
                    .set_active(!ui.controls_toggle.is_active());
            }
        ));
        self.window.add_action(&controls);
        application.set_accels_for_action("win.toggle-controls", &["F9"]);
    }

    fn connect_scale(
        self: &Rc<Self>,
        scale: &gtk::Scale,
        key: SettingKey,
        setter: impl Fn(&mut Settings, f32) + 'static,
    ) {
        scale.connect_value_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |scale| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let value = scale.value() as f32;
                ui.change_setting(key, |settings| setter(settings, value));
            }
        ));
    }

    fn connect_web_scale(
        self: &Rc<Self>,
        scale: &gtk::Scale,
        key: SettingKey,
        getter: impl Fn(&toniator::WebShapeChannel) -> f64 + 'static,
        setter: impl Fn(&mut toniator::WebShapeChannel, f64) + 'static,
    ) {
        let lower = scale.adjustment().lower();
        let upper = scale.adjustment().upper();
        scale.connect_value_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |scale| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let value = scale.value();
                let all = ui.web_target.selected() == 0;
                ui.change_web_treatment(|settings, inks| {
                    if all {
                        let delta = value - getter(&settings.base_channel);
                        setter(&mut settings.base_channel, value);
                        for ink in inks {
                            let effective = shifted_effective(
                                getter(settings.channels.get(ink)),
                                delta,
                                lower,
                                upper,
                            );
                            setter(settings.channels.get_mut(ink), effective);
                        }
                    } else {
                        setter(settings.channels.get_mut(inks[0]), value);
                    }
                });
            }
        ));
        self.connect_slider_gesture(scale, key);
    }

    fn connect_curve_scale(
        self: &Rc<Self>,
        scale: &gtk::Scale,
        key: SettingKey,
        getter: impl Fn(&WebCurveChannel) -> f64 + 'static,
        setter: impl Fn(&mut WebCurveChannel, f64) + 'static,
    ) {
        let lower = scale.adjustment().lower();
        let upper = scale.adjustment().upper();
        scale.connect_value_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |scale| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let value = scale.value();
                let all = ui.curve_target.selected() == 0;
                ui.change_curve_treatment(|settings, inks| {
                    if all {
                        let delta = value - getter(&settings.base_channel);
                        setter(&mut settings.base_channel, value);
                        for ink in inks {
                            let effective = shifted_effective(
                                getter(settings.channels.get(ink)),
                                delta,
                                lower,
                                upper,
                            );
                            setter(settings.channels.get_mut(ink), effective);
                        }
                    } else {
                        setter(settings.channels.get_mut(inks[0]), value);
                    }
                });
            }
        ));
        self.connect_slider_gesture(scale, key);
    }

    fn connect_curve_editor(self: &Rc<Self>) {
        self.curve_editor.set_draw_func(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, context, width, height| {
                draw_curve_editor(
                    context,
                    width,
                    height,
                    ui.current_curve_path().as_ref(),
                    ui.curve_selected_handle.get(),
                    ui.current_curve_color(),
                );
            }
        ));

        let drag = gtk::GestureDrag::new();
        drag.connect_drag_begin(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, x, y| {
                let Some(path) = ui.current_curve_path() else {
                    return;
                };
                let handle = nearest_curve_handle(
                    &path,
                    x,
                    y,
                    ui.curve_editor.width(),
                    ui.curve_editor.height(),
                );
                ui.curve_selected_handle.set(handle);
                ui.curve_drag_start
                    .set((handle >= 0).then(|| curve_handle_points(&path)[handle as usize]));
                if handle >= 0 {
                    ui.begin_setting_edit(SettingKey::CurvePath);
                }
                ui.curve_editor.queue_draw();
            }
        ));
        drag.connect_drag_update(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, offset_x, offset_y| {
                let handle = ui.curve_selected_handle.get();
                let Some(start) = ui.curve_drag_start.get() else {
                    return;
                };
                let scale = curve_editor_scale(ui.curve_editor.width(), ui.curve_editor.height());
                let point = CurvePoint {
                    x: (start.x + offset_x / scale).clamp(-1.5, 1.5),
                    y: (start.y - offset_y / scale).clamp(-1.5, 1.5),
                };
                ui.change_curve_treatment(move |settings, inks| {
                    edit_curve_paths(settings, &inks, |path| {
                        set_curve_handle(path, handle as usize, point)
                    });
                });
            }
        ));
        drag.connect_drag_end(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, _, _| {
                ui.curve_drag_start.set(None);
                ui.end_setting_edit();
            }
        ));
        drag.connect_cancel(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, _| {
                if ui.curve_drag_start.take().is_some() {
                    ui.cancel_active_edit();
                    ui.curve_editor.queue_draw();
                }
            }
        ));
        self.curve_editor.add_controller(drag);

        let click = gtk::GestureClick::new();
        click.connect_pressed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, presses, x, y| {
                let Some(path) = ui.current_curve_path() else {
                    return;
                };
                if presses == 2 {
                    let point = editor_to_curve_point(
                        x,
                        y,
                        ui.curve_editor.width(),
                        ui.curve_editor.height(),
                    );
                    let (segment, amount) = nearest_curve_segment(&path, point);
                    ui.change_curve_treatment(move |settings, inks| {
                        edit_curve_paths(settings, &inks, |path| {
                            split_curve_segment(path, segment, amount)
                        });
                    });
                    return;
                }
                ui.curve_selected_handle.set(nearest_curve_handle(
                    &path,
                    x,
                    y,
                    ui.curve_editor.width(),
                    ui.curve_editor.height(),
                ));
                ui.curve_editor.grab_focus();
                ui.curve_editor.queue_draw();
            }
        ));
        self.curve_editor.add_controller(click);

        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                if key == gdk::Key::Escape {
                    ui.cancel_active_edit();
                    ui.curve_drag_start.set(None);
                    ui.curve_editor.queue_draw();
                    return glib::Propagation::Stop;
                }
                let delta = match key {
                    gdk::Key::Left => Some((-0.005, 0.0)),
                    gdk::Key::Right => Some((0.005, 0.0)),
                    gdk::Key::Up => Some((0.0, 0.005)),
                    gdk::Key::Down => Some((0.0, -0.005)),
                    _ => None,
                };
                if let Some((dx, dy)) = delta {
                    let handle = ui.curve_selected_handle.get();
                    if handle >= 0
                        && let Some(path) = ui.current_curve_path()
                        && let Some(start) = curve_handle_points(&path).get(handle as usize)
                    {
                        let point = CurvePoint {
                            x: start.x + dx,
                            y: start.y + dy,
                        };
                        ui.begin_setting_edit(SettingKey::CurvePath);
                        ui.change_curve_treatment(move |settings, inks| {
                            edit_curve_paths(settings, &inks, |path| {
                                set_curve_handle(path, handle as usize, point)
                            });
                        });
                        ui.end_setting_edit();
                        ui.curve_editor.queue_draw();
                    }
                    return glib::Propagation::Stop;
                }
                if key != gdk::Key::Delete && key != gdk::Key::BackSpace {
                    return glib::Propagation::Proceed;
                }
                let handle = ui.curve_selected_handle.get();
                if handle < 0 || handle % 3 != 0 {
                    return glib::Propagation::Proceed;
                }
                ui.change_curve_treatment(move |settings, inks| {
                    edit_curve_paths(settings, &inks, |path| {
                        delete_curve_anchor(path, handle as usize)
                    });
                });
                ui.curve_selected_handle.set(-1);
                glib::Propagation::Stop
            }
        ));
        self.curve_editor.add_controller(keys);
    }

    fn connect_motif_overlay(self: &Rc<Self>) {
        self.motif_overlay.set_draw_func(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, context, width, height| {
                let Some((_, _, center_x, center_y, angle_x, angle_y, spacing_x, spacing_y)) =
                    ui.motif_overlay_geometry(width as f64, height as f64)
                else {
                    return;
                };
                context.set_source_rgba(0.15, 0.55, 1.0, 0.9);
                context.set_line_width(2.0);
                context.move_to(center_x - 12.0, center_y);
                context.line_to(center_x + 12.0, center_y);
                context.move_to(center_x, center_y - 12.0);
                context.line_to(center_x, center_y + 12.0);
                context.move_to(center_x, center_y);
                context.line_to(angle_x, angle_y);
                context.move_to(center_x, center_y);
                context.line_to(spacing_x, spacing_y);
                let _ = context.stroke();
                for (x, y, radius) in [
                    (center_x, center_y, 7.0),
                    (angle_x, angle_y, 6.0),
                    (spacing_x, spacing_y, 6.0),
                ] {
                    context.arc(x, y, radius, 0.0, std::f64::consts::TAU);
                    context.set_source_rgba(0.95, 0.98, 1.0, 0.95);
                    let _ = context.fill_preserve();
                    context.set_source_rgba(0.15, 0.55, 1.0, 1.0);
                    let _ = context.stroke();
                }
                context.set_font_size(11.0);
                context.set_source_rgba(0.05, 0.2, 0.38, 1.0);
                context.move_to(angle_x - 3.5, angle_y + 4.0);
                let _ = context.show_text("R");
                context.move_to(spacing_x - 3.5, spacing_y + 4.0);
                let _ = context.show_text("S");
            }
        ));
        let drag = gtk::GestureDrag::new();
        drag.connect_drag_begin(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, x, y| {
                let width = ui.motif_overlay.width() as f64;
                let height = ui.motif_overlay.height() as f64;
                let Some((_, _, cx, cy, ax, ay, sx, sy)) = ui.motif_overlay_geometry(width, height)
                else {
                    return;
                };
                let distance = |px: f64, py: f64| (x - px).hypot(y - py);
                let kind = if distance(cx, cy) <= 18.0 {
                    0
                } else if distance(ax, ay) <= 18.0 {
                    1
                } else if distance(sx, sy) <= 18.0 {
                    2
                } else {
                    return;
                };
                let Some((offset_x, offset_y, angle, spacing)) = ui.current_motif_arrangement()
                else {
                    return;
                };
                ui.motif_drag.set(Some(MotifDrag {
                    kind,
                    start_x: x,
                    start_y: y,
                    offset_x,
                    offset_y,
                    angle,
                    spacing,
                }));
                ui.motif_overlay.grab_focus();
                ui.begin_setting_edit(match kind {
                    0 => SettingKey::CurvePositionX,
                    1 => SettingKey::CurveAngle,
                    _ => SettingKey::MotifRowSpacing,
                });
            }
        ));
        drag.connect_drag_update(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, dx, dy| {
                let Some(drag) = ui.motif_drag.get() else {
                    return;
                };
                let Some((scale, _, cx, cy, _, _, _, _)) = ui.motif_overlay_geometry(
                    ui.motif_overlay.width() as f64,
                    ui.motif_overlay.height() as f64,
                ) else {
                    return;
                };
                match drag.kind {
                    0 => ui.change_curve_treatment(move |settings, inks| {
                        for ink in inks {
                            let channel = settings.channels.get_mut(ink);
                            channel.offset_x = drag.offset_x + dx / scale;
                            channel.offset_y = drag.offset_y + dy / scale;
                        }
                    }),
                    1 => {
                        let degrees = (drag.start_y + dy - cy)
                            .atan2(drag.start_x + dx - cx)
                            .to_degrees();
                        ui.change_curve_treatment(move |settings, inks| {
                            for ink in inks {
                                settings.channels.get_mut(ink).grid_rotation = degrees;
                            }
                        });
                    }
                    _ => {
                        let radians = (drag.angle + 90.0).to_radians();
                        let projected = dx * radians.cos() + dy * radians.sin();
                        let value = (drag.spacing + projected / scale).abs().max(1.0);
                        ui.change_curve_treatment(move |settings, inks| {
                            for ink in inks {
                                settings.channels.get_mut(ink).stack_spacing = value;
                            }
                        });
                    }
                }
            }
        ));
        drag.connect_drag_end(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, _, _| {
                if ui.motif_drag.take().is_some() {
                    ui.end_setting_edit();
                }
            }
        ));
        drag.connect_cancel(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, _| ui.cancel_motif_drag()
        ));
        self.motif_overlay.add_controller(drag);
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                if key != gdk::Key::Escape || ui.motif_drag.take().is_none() {
                    return glib::Propagation::Proceed;
                }
                ui.cancel_active_edit();
                glib::Propagation::Stop
            }
        ));
        self.motif_overlay.add_controller(keys);
    }

    fn current_motif_arrangement(&self) -> Option<(f64, f64, f64, f64)> {
        let settings = {
            let state = self.state.borrow();
            state
                .editor
                .as_ref()?
                .document()
                .pattern_state
                .curve_settings()
                .ok()?
        };
        let ink = self.selected_curve_inks()?.first().copied()?;
        let channel = settings.channels.get(ink);
        Some((
            channel.offset_x,
            channel.offset_y,
            channel.grid_rotation,
            channel.stack_spacing.abs(),
        ))
    }

    fn cancel_motif_drag(self: &Rc<Self>) {
        if self.motif_drag.take().is_some() {
            self.cancel_active_edit();
        }
    }

    fn cancel_active_edit(self: &Rc<Self>) {
        let changed = self
            .state
            .borrow_mut()
            .editor
            .as_mut()
            .is_some_and(DocumentEditor::cancel_edit);
        if changed {
            self.after_history_change();
        }
    }

    #[allow(clippy::type_complexity)]
    fn motif_overlay_geometry(
        &self,
        width: f64,
        height: f64,
    ) -> Option<(f64, f64, f64, f64, f64, f64, f64, f64)> {
        let settings = {
            let state = self.state.borrow();
            state
                .editor
                .as_ref()?
                .document()
                .pattern_state
                .curve_settings()
                .ok()?
        };
        if settings.layout != CurveLayout::MotifPattern {
            return None;
        }
        let ink = self.selected_curve_inks()?.first().copied()?;
        let channel = settings.channels.get(ink);
        let scale = (width / settings.output_width as f64)
            .min(height / settings.output_height as f64)
            .max(0.0001);
        let left = (width - settings.output_width as f64 * scale) / 2.0;
        let top = (height - settings.output_height as f64 * scale) / 2.0;
        let center_x = left + (settings.output_width as f64 / 2.0 + channel.offset_x) * scale;
        let center_y = top + (settings.output_height as f64 / 2.0 + channel.offset_y) * scale;
        let radians = channel.grid_rotation.to_radians();
        let handle = 72.0;
        let angle_x = center_x + radians.cos() * handle;
        let angle_y = center_y + radians.sin() * handle;
        let spacing_handle = (channel.stack_spacing.abs() * scale).clamp(42.0, 110.0);
        let spacing_x = center_x + (radians + std::f64::consts::FRAC_PI_2).cos() * spacing_handle;
        let spacing_y = center_y + (radians + std::f64::consts::FRAC_PI_2).sin() * spacing_handle;
        Some((
            scale, left, center_x, center_y, angle_x, angle_y, spacing_x, spacing_y,
        ))
    }

    fn sync_motif_overlay(&self) {
        let active = self.motif_arrange.is_active()
            && self.state.borrow().editor.as_ref().is_some_and(|editor| {
                editor
                    .document()
                    .pattern_state
                    .curve_settings()
                    .is_ok_and(|settings| settings.layout == CurveLayout::MotifPattern)
            });
        self.motif_overlay.set_visible(active);
        if active {
            self.motif_overlay.queue_draw();
        }
    }

    fn current_curve_path(&self) -> Option<CurvePath> {
        let settings = {
            let state = self.state.borrow();
            state
                .editor
                .as_ref()?
                .document()
                .pattern_state
                .curve_settings()
                .ok()?
        };
        let inks = self.selected_curve_inks()?;
        if !settings.use_shared_curve
            && inks
                .iter()
                .skip(1)
                .any(|ink| settings.channels.get(*ink).path != settings.channels.get(inks[0]).path)
        {
            return None;
        }
        Some(if settings.use_shared_curve {
            settings.shared_path.clone()
        } else {
            settings.channels.get(inks[0]).path.clone()
        })
    }

    fn current_curve_color(&self) -> (f64, f64, f64) {
        let settings = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                return (0.2, 0.55, 1.0);
            };
            let Ok(settings) = editor.document().pattern_state.curve_settings() else {
                return (0.2, 0.55, 1.0);
            };
            settings
        };
        let Some(inks) = self.selected_curve_inks() else {
            return (0.2, 0.55, 1.0);
        };
        if inks.len() != 1 {
            return (0.2, 0.55, 1.0);
        }
        toniator::model::parse_hex_color(&settings.channels.get(inks[0]).color)
            .map(|(r, g, b)| (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0))
            .unwrap_or((0.2, 0.55, 1.0))
    }

    fn connect_slider_gesture(self: &Rc<Self>, scale: &gtk::Scale, key: SettingKey) {
        let gesture = gtk::GestureClick::new();
        gesture.connect_pressed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, _, _, _| ui.begin_setting_edit(key)
        ));
        gesture.connect_released(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, _, _, _| ui.end_setting_edit()
        ));
        gesture.connect_cancel(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_, _| ui.end_setting_edit()
        ));
        scale.add_controller(gesture);
        if let Some(entry) = precision_entry(scale) {
            let focus = gtk::EventControllerFocus::new();
            focus.connect_enter(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |_| ui.begin_setting_edit(key)
            ));
            focus.connect_leave(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |_| ui.end_setting_edit()
            ));
            entry.add_controller(focus);
        }
    }

    fn open_menu(self: &Rc<Self>) {
        let popover = gtk::Popover::new();
        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 4);
        box_.set_margin_top(8);
        box_.set_margin_bottom(8);
        box_.set_margin_start(8);
        box_.set_margin_end(8);
        let artwork = gtk::Button::with_label("Open Artwork…");
        let document = gtk::Button::with_label("Open Toniator Document…");
        let preset = gtk::Button::with_label("Load Preset…");
        artwork.add_css_class("flat");
        document.add_css_class("flat");
        preset.add_css_class("flat");
        preset.set_sensitive(self.state.borrow().editor.is_some());
        box_.append(&artwork);
        box_.append(&document);
        box_.append(&preset);
        popover.set_child(Some(&box_));
        popover.set_parent(&self.window);
        popover.connect_closed(|popover| popover.unparent());
        connect_clicked(&artwork, self, |ui| ui.open_artwork_dialog());
        connect_clicked(&document, self, |ui| ui.open_document_dialog());
        preset.connect_clicked(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[strong]
            popover,
            move |_| {
                popover.popdown();
                ui.open_preset_dialog(ui.open.upcast_ref());
            }
        ));
        popover.popup();
    }

    fn new_project(self: &Rc<Self>) {
        self.gate_dirty_transition(|ui| ui.finish_new_project());
    }

    fn finish_new_project(&self) {
        self.gate.next();
        self.candidate_gate.next();
        self.preset_gate.next();
        self.render_requests.take();
        self.render_results.take();
        self.zoom_settle_generation
            .set(self.zoom_settle_generation.get().wrapping_add(1));
        self.preview_generation.set(0);
        self.fit_allocation.borrow_mut().reset();
        {
            let mut state = self.state.borrow_mut();
            clear_document_for_new_project(&mut state);
        }
        self.compare.set_active(false);
        self.picture.set_paintable(Option::<&gdk::Paintable>::None);
        self.source_label.set_text("");
        self.stack.set_visible_child_name("start");
        self.title.set_title("Toniator");
        self.title.set_subtitle("Start a project");
        self.update_actions();
    }

    fn open_artwork_dialog(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("Open Artwork")
            .modal(true)
            .build();
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        let artwork = gtk::FileFilter::new();
        artwork.set_name(Some("Artwork (PNG, JPEG, WebP, SVG)"));
        for mime in ["image/png", "image/jpeg", "image/webp", "image/svg+xml"] {
            artwork.add_mime_type(mime);
        }
        let all = gtk::FileFilter::new();
        all.set_name(Some("All files"));
        all.add_pattern("*");
        filters.append(&artwork);
        filters.append(&all);
        dialog.set_filters(Some(&filters));
        dialog.set_default_filter(Some(&artwork));
        dialog.open(
            Some(&self.window),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |result| {
                    if let Ok(file) = result
                        && let Some(path) = file.path()
                    {
                        ui.import_artwork(&path);
                    }
                }
            ),
        );
    }

    fn open_document_dialog(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("Open Toniator Document")
            .modal(true)
            .build();
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        let documents = gtk::FileFilter::new();
        documents.set_name(Some("Toniator Documents"));
        documents.add_pattern("*.toniator");
        let all = gtk::FileFilter::new();
        all.set_name(Some("All files"));
        all.add_pattern("*");
        filters.append(&documents);
        filters.append(&all);
        dialog.set_filters(Some(&filters));
        dialog.set_default_filter(Some(&documents));
        dialog.open(
            Some(&self.window),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |result| {
                    if let Ok(file) = result
                        && let Some(path) = file.path()
                    {
                        ui.open_document_path(&path);
                    }
                }
            ),
        );
    }

    fn open_preset_dialog(self: &Rc<Self>, anchor: &gtk::Widget) {
        if self.state.borrow().editor.is_none() {
            return;
        }
        let popover = gtk::Popover::new();
        let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
        list.set_margin_top(8);
        list.set_margin_bottom(8);
        list.set_margin_start(8);
        list.set_margin_end(8);
        list.append(
            &gtk::Label::builder()
                .label("Curated")
                .xalign(0.0)
                .css_classes(["heading"])
                .build(),
        );
        for (label, bytes) in BUNDLED_PRESETS {
            let button = gtk::Button::with_label(label);
            button.add_css_class("flat");
            list.append(&button);
            connect_clicked(&button, self, move |ui| {
                ui.import_preset_source(PresetSource::Bundled(bytes))
            });
        }
        let user_dir = native_user_preset_dir();
        let mut user_presets: Vec<PathBuf> = std::fs::read_dir(&user_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|value| value.eq_ignore_ascii_case("tntr"))
            })
            .collect();
        user_presets.sort_by_key(|path| path.file_name().map(|name| name.to_ascii_lowercase()));
        if !user_presets.is_empty() {
            list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            list.append(
                &gtk::Label::builder()
                    .label("My Presets")
                    .xalign(0.0)
                    .css_classes(["heading"])
                    .build(),
            );
            for path in user_presets {
                let label = preset_name_from_path(&path);
                let button = gtk::Button::with_label(&label);
                button.add_css_class("flat");
                list.append(&button);
                connect_clicked(&button, self, move |ui| {
                    ui.import_preset_source(PresetSource::Path(path.clone()))
                });
            }
        }
        list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let browse = gtk::Button::with_label("Browse…");
        browse.add_css_class("flat");
        list.append(&browse);
        connect_clicked(&browse, self, |ui| ui.browse_preset_dialog());
        popover.set_child(Some(&list));
        popover.set_parent(anchor);
        popover.connect_closed(|popover| popover.unparent());
        popover.popup();
    }

    fn browse_preset_dialog(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("Load Halftone Preset")
            .modal(true)
            .build();
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        let presets = gtk::FileFilter::new();
        presets.set_name(Some("Toniator Halftone Presets"));
        presets.add_pattern("*.tntr");
        filters.append(&presets);
        dialog.set_filters(Some(&filters));
        dialog.set_default_filter(Some(&presets));
        dialog.open(
            Some(&self.window),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |result| {
                    if let Ok(file) = result
                        && let Some(path) = file.path()
                    {
                        ui.import_preset_path(&path);
                    }
                }
            ),
        );
    }

    fn save_treatment_dialog(self: &Rc<Self>) {
        let popover = gtk::Popover::new();
        let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
        list.set_margin_top(8);
        list.set_margin_bottom(8);
        list.set_margin_start(8);
        list.set_margin_end(8);
        list.append(
            &gtk::Label::builder()
                .label("Save preset scope")
                .xalign(0.0)
                .css_classes(["heading"])
                .build(),
        );
        for (scope, detail) in [
            (
                toniator::preset::PresetScope::Treatment,
                "Treatment — geometry and shared settings",
            ),
            (
                toniator::preset::PresetScope::Pipeline,
                "Pipeline — source, alpha, output, assignment",
            ),
            (
                toniator::preset::PresetScope::Channel,
                "Current Channel — appearance and channel settings",
            ),
            (
                toniator::preset::PresetScope::CompleteWorkflow,
                "Complete Workflow — pipeline, treatment, and channels",
            ),
        ] {
            let button = gtk::Button::with_label(detail);
            button.add_css_class("flat");
            list.append(&button);
            connect_clicked(&button, self, move |ui| {
                ui.save_treatment_dialog_with_scope(scope)
            });
        }
        popover.set_child(Some(&list));
        popover.set_parent(&self.preset_save);
        popover.connect_closed(|popover| popover.unparent());
        popover.popup();
    }

    fn save_treatment_dialog_with_scope(self: &Rc<Self>, scope: toniator::preset::PresetScope) {
        let document = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                return;
            };
            editor.document().clone()
        };
        let initial_name = match document.pattern_state.selected_pattern_id() {
            Some(PatternId::COMPATIBILITY_CURVES_V1) => "Curves Preset.tntr",
            Some(PatternId::WEIGHTED_VORONOI_V1) => "Weighted Voronoi Preset.tntr",
            Some(PatternId::COMPATIBILITY_SHAPES_V1) | None => "Shapes Preset.tntr",
        };
        let directory = native_user_preset_dir();
        if let Err(error) = std::fs::create_dir_all(&directory) {
            self.show_error(&format!("Could not create preset folder: {error}"));
            return;
        }
        let dialog = gtk::FileDialog::builder()
            .title("Save Halftone Preset")
            .modal(true)
            .initial_folder(&gio::File::for_path(&directory))
            .initial_name(initial_name)
            .build();
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        let treatments = gtk::FileFilter::new();
        treatments.set_name(Some("Toniator Halftone Preset (.tntr)"));
        treatments.add_pattern("*.tntr");
        filters.append(&treatments);
        dialog.set_filters(Some(&filters));
        dialog.set_default_filter(Some(&treatments));
        dialog.save(
            Some(&self.window),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |result| {
                    let Ok(file) = result else { return };
                    let Some(path) = file.path() else { return };
                    let path = normalized_preset_path(&path);
                    let name = preset_name_from_path(&path);
                    let bytes =
                        match toniator::preset::document_preset_bytes(&name, &document, scope) {
                            Ok(bytes) => bytes,
                            Err(error) => {
                                ui.show_error(&format!("Could not save preset: {error:#}"));
                                return;
                            }
                        };
                    match toniator::persistence::atomic_write(&path, &bytes) {
                        Ok(()) => ui.show_message(&format!("Saved preset {}", path.display())),
                        Err(error) => ui.show_error(&format!("Could not save preset: {error:#}")),
                    }
                }
            ),
        );
    }

    fn open_path(self: &Rc<Self>, path: &Path) {
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("tntr"))
        {
            self.import_preset_path(path);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("toniator"))
        {
            self.open_document_path(path);
        } else {
            self.import_artwork(path);
        }
    }

    fn import_artwork(self: &Rc<Self>, path: &Path) {
        self.load_candidate_async(path.to_owned(), false, false);
    }

    fn open_document_path(self: &Rc<Self>, path: &Path) {
        self.load_candidate_async(path.to_owned(), true, false);
    }

    fn recover_document(self: &Rc<Self>) {
        if self.recovery_enabled {
            self.load_candidate_async(recovery_path(), true, true);
        }
    }

    fn import_preset_path(self: &Rc<Self>, path: &Path) {
        self.import_preset_source(PresetSource::Path(path.to_owned()));
    }

    fn import_preset_source(self: &Rc<Self>, source: PresetSource) {
        let document = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                self.show_message("Open artwork before loading a halftone preset.");
                return;
            };
            editor.document().clone()
        };
        let document_id = document.document_id.clone();
        let generation = self.preset_gate.next();
        self.preset_pending.set(true);
        let result = Arc::new(LatestSlot::default());
        let worker_result = Arc::clone(&result);
        if self.recovery_enabled {
            self.show_message("Reading halftone preset…");
        }
        std::thread::spawn(move || {
            let parsed = (|| -> anyhow::Result<(toniator::preset::ParsedTreatment, Document)> {
                let bytes = match source {
                    PresetSource::Path(path) => std::fs::read(path)?,
                    PresetSource::Bundled(bytes) => bytes.to_vec(),
                };
                let dimensions = toniator::render::source_dimensions(&document.source)?;
                let treatment = toniator::preset::parse_treatment(&bytes, dimensions)?;
                let candidate = treatment.candidate_for(&document)?;
                Ok((treatment, candidate))
            })();
            worker_result.replace(parsed);
        });
        glib::timeout_add_local(
            Duration::from_millis(20),
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move || {
                    let Some(result) = result.take() else {
                        return glib::ControlFlow::Continue;
                    };
                    if !ui.preset_gate.accepts(generation) {
                        return glib::ControlFlow::Break;
                    }
                    ui.preset_pending.set(false);
                    match result {
                        Ok((treatment, candidate)) => {
                            let canvas_normalized = treatment.canvas_normalized;
                            let changed = {
                                let mut state = ui.state.borrow_mut();
                                let Some(editor) = state.editor.as_mut() else {
                                    return glib::ControlFlow::Break;
                                };
                                if editor.document().document_id != document_id {
                                    return glib::ControlFlow::Break;
                                }
                                editor.replace_with_preset_candidate(candidate)
                            };
                            if changed {
                                if ui.state.borrow().compare_source && !ui.compare_source_artifact {
                                    ui.state.borrow_mut().compare_source = false;
                                    ui.compare.set_active(false);
                                }
                                if ui.compare_source_artifact {
                                    ui.state.borrow_mut().compare_source = true;
                                    ui.compare.set_active(true);
                                }
                                ui.sync_controls();
                                if ui.arrange_motif_artifact {
                                    ui.curve_target.set_selected(4);
                                    ui.motif_arrange.set_active(true);
                                }
                                if let Some(document) = ui
                                    .state
                                    .borrow()
                                    .editor
                                    .as_ref()
                                    .map(|editor| editor.document().clone())
                                {
                                    ui.queue_autosave(document);
                                }
                                ui.request_preview();
                                ui.update_actions();
                                if ui.recovery_enabled {
                                    if canvas_normalized {
                                        ui.show_message("Preset applied; legacy canvas dimensions were ignored to preserve the source aspect ratio.");
                                    } else {
                                        ui.show_message("Halftone preset loaded");
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            ui.show_error(&format!("Could not load halftone preset: {error:#}"))
                        }
                    }
                    glib::ControlFlow::Break
                }
            ),
        );
    }

    fn load_candidate_async(self: &Rc<Self>, path: PathBuf, is_document: bool, recovered: bool) {
        let generation = self.candidate_gate.next();
        let result = Arc::new(LatestSlot::default());
        let worker_result = Arc::clone(&result);
        let path_for_worker = path.clone();
        if self.recovery_enabled {
            self.show_message(if is_document {
                "Opening Toniator document…"
            } else {
                "Validating artwork…"
            });
        }
        std::thread::spawn(move || {
            let candidate =
                (|| -> anyhow::Result<(Document, toniator::persistence::LoadAdjustments)> {
                    let (document, adjustments) = if is_document {
                        let loaded = toniator::persistence::load_document_with_adjustments(
                            &path_for_worker,
                        )?;
                        toniator::render::decode_source(&loaded.document.source, 128)?;
                        (loaded.document, loaded.adjustments)
                    } else {
                        let bytes = std::fs::read(&path_for_worker)?;
                        let source = SourceArtwork {
                            name: path_for_worker
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                            media_type: media_type(&path_for_worker),
                            bytes: Arc::from(bytes),
                        };
                        toniator::render::decode_source(&source, 128)?;
                        let (width, height) = toniator::render::source_dimensions(&source)?;
                        (
                            Document::new_with_artboard(source, width, height),
                            toniator::persistence::LoadAdjustments::default(),
                        )
                    };
                    Ok((document, adjustments))
                })();
            worker_result.replace(candidate);
        });
        glib::timeout_add_local(
            Duration::from_millis(20),
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move || {
                    let Some(candidate) = result.take() else {
                        return glib::ControlFlow::Continue;
                    };
                    if !ui.candidate_gate.accepts(generation) {
                        return glib::ControlFlow::Break;
                    }
                    match candidate {
                        Ok((document, adjustments)) => {
                            let install_path = if is_document && !recovered {
                                Some(path.clone())
                            } else {
                                None
                            };
                            ui.gate_dirty_transition(move |ui| {
                                ui.install_loaded_document(
                                    document.clone(),
                                    install_path.clone(),
                                    adjustments != toniator::persistence::LoadAdjustments::default(),
                                );
                                if adjustments.canvas_aspect {
                                    ui.show_message("Canvas proportions were updated to match the source artwork; save to keep this change.");
                                } else if adjustments.crosshatch_geometry {
                                    ui.show_message("Crosshatch treatment was normalized to curve layers; save to keep this change.");
                                }
                                if recovered {
                                    ui.show_message(
                                        "Recovered autosaved work — save it when ready.",
                                    );
                                }
                            });
                        }
                        Err(error) => {
                            let message = format!(
                                "Could not open {}: {error:#}",
                                if is_document { "document" } else { "artwork" }
                            );
                            ui.show_error(&message);
                            if ui.deferred_candidate_artifact {
                                ui.report_cli_artifact_error(message);
                                ui.cli_artifacts_written.set(true);
                                ui.close_approved.set(true);
                                ui.window.close();
                            }
                        }
                    }
                    glib::ControlFlow::Break
                }
            ),
        );
    }

    fn request_example(self: &Rc<Self>) {
        self.gate_dirty_transition(|ui| ui.load_example());
    }

    fn load_example(self: &Rc<Self>) {
        let source = SourceArtwork {
            name: "Toniator Example.svg".into(),
            media_type: "image/svg+xml".into(),
            bytes: Arc::from(EXAMPLE_SVG.as_bytes()),
        };
        self.install_document(Document::new_with_artboard(source, 960, 680), None);
    }

    fn apply_demo_adjustment(self: &Rc<Self>) {
        self.change_setting(SettingKey::Treatment, |settings| {
            settings.treatment = Treatment::Lines
        });
        self.change_setting(SettingKey::Detail, |settings| settings.detail = 64.0);
        self.change_setting(SettingKey::Coverage, |settings| settings.coverage = 108.0);
        self.change_setting(SettingKey::Contrast, |settings| settings.contrast = 128.0);
        self.change_setting(SettingKey::Angle, |settings| settings.angle = -8.0);
        self.sync_controls();
    }

    fn install_document(self: &Rc<Self>, document: Document, path: Option<PathBuf>) {
        self.install_loaded_document(document, path, false);
    }

    fn install_loaded_document(
        self: &Rc<Self>,
        mut document: Document,
        path: Option<PathBuf>,
        adjusted_on_load_dirty: bool,
    ) {
        if let Ok((width, height)) = toniator::render::source_dimensions(&document.source) {
            document.normalize_canvas_aspect(width, height);
        }
        let should_autosave = path.is_none();
        let recovery_document = should_autosave.then(|| document.clone());
        {
            let mut state = self.state.borrow_mut();
            state.editor = Some(DocumentEditor::new_with_load_adjustment(
                document,
                adjusted_on_load_dirty,
            ));
            state.path = path;
            state.compare_source = false;
            state.preview_size = None;
            state.source_cache = None;
            state.rendered_cache = None;
            state.zoom_mode = ZoomMode::Fit(100.0);
            self.fit_allocation.borrow_mut().reset();
        }
        self.compare.set_active(false);
        self.stack.set_visible_child_name("editor");
        self.sync_controls();
        self.request_preview();
        self.update_actions();
        if let Some(document) = recovery_document {
            self.queue_autosave(document);
        }
    }

    fn change_setting(self: &Rc<Self>, key: SettingKey, update: impl FnOnce(&mut Settings)) {
        let document = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            let mut settings = editor.document().settings;
            update(&mut settings);
            if !editor.set_settings(key, settings) {
                return;
            }
            editor.document().clone()
        };
        self.state.borrow_mut().rendered_cache = None;
        self.queue_autosave(document);
        self.request_rendered_preview();
        self.update_actions();
    }

    fn web_output_flags(&self) -> Option<(bool, bool)> {
        let state = self.state.borrow();
        let document = state.editor.as_ref()?.document();
        let crosshatch = document_uses_crosshatch(document);
        Some((
            document.artwork_pipeline.output_model == OutputModel::RgbScreen && !crosshatch,
            crosshatch,
        ))
    }

    fn curve_output_flags(&self) -> Option<(bool, bool)> {
        let state = self.state.borrow();
        let document = state.editor.as_ref()?.document();
        let crosshatch = document_uses_crosshatch(document);
        Some((
            document.artwork_pipeline.output_model == OutputModel::RgbScreen && !crosshatch,
            crosshatch,
        ))
    }

    fn selected_web_inks(&self) -> Option<Vec<Ink>> {
        let (rgb, crosshatch) = self.web_output_flags()?;
        web_inks_for_target(self.web_target.selected(), rgb, crosshatch)
    }

    fn open_shape_editor(self: &Rc<Self>) {
        let target = self.web_target.selected();
        let target_ink = self
            .selected_web_inks()
            .and_then(|inks| inks.first().copied());
        let Some(shape_path) = self
            .state
            .borrow()
            .editor
            .as_ref()
            .and_then(|editor| editor.document().pattern_state.shape_settings().ok())
            .map(|settings| {
                if settings.use_shared_mark || target == 0 {
                    settings.resolved_custom_shape_path()
                } else {
                    let ink = target_ink.unwrap_or(Ink::Black);
                    settings.resolved_channel_shape_path(settings.channels.get(ink))
                }
            })
        else {
            return;
        };
        let shape_path = Rc::new(RefCell::new(shape_path));
        let nodes = Rc::new(RefCell::new(
            shape_path
                .borrow()
                .anchors
                .iter()
                .map(|a| a.point)
                .collect::<Vec<_>>(),
        ));
        let selected = Rc::new(Cell::new(0usize));
        if self.artifact_shape_editor {
            let mut path = shape_path.borrow_mut();
            for _ in 0..3 {
                let target = cubic_shape_point(path.anchors[0], path.anchors[1], 0.5);
                if let Some(index) = insert_nearest_shape_anchor(&mut path, target, 0.02) {
                    selected.set(index);
                }
            }
            *nodes.borrow_mut() = path.anchors.iter().map(|anchor| anchor.point).collect();
            eprintln!(
                "artifact User Defined editor: production insertion helper completed 3 repeated inserts (4 -> {} anchors)",
                path.anchors.len()
            );
        }
        let selected_part = Rc::new(Cell::new(0));
        let dialog = adw::Window::builder()
            .transient_for(&self.window)
            .modal(true)
            .title("Edit User-Defined Mark")
            .default_width(560)
            .default_height(620)
            .build();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.set_margin_top(16);
        root.set_margin_bottom(16);
        root.set_margin_start(16);
        root.set_margin_end(16);
        let instructions = gtk::Label::builder()
            .label("Drag anchors or either independent Bézier handle. Moving an anchor carries its handles. Double-click a curve to insert without changing its shape. Delete removes an anchor; Escape cancels.")
            .wrap(true).xalign(0.0).css_classes(["dim-label"]).build();
        let feedback = gtk::Label::builder()
            .xalign(0.0)
            .css_classes(["error"])
            .build();
        let area = gtk::DrawingArea::builder()
            .hexpand(true)
            .vexpand(true)
            .focusable(true)
            .build();
        area.set_tooltip_text(Some(
            "Edit the selected mark's anchors and independent Bézier handles.",
        ));
        area.update_property(&[
            gtk::accessible::Property::Label("User-defined mark editor"),
            gtk::accessible::Property::Description(
                "Drag anchors or independent Bézier handles. Double-click a curve to insert an anchor. Delete removes an anchor, arrow keys move the selection, and Escape cancels.",
            ),
        ]);
        area.set_draw_func(glib::clone!(
            #[strong]
            shape_path,
            #[strong]
            selected,
            #[strong]
            selected_part,
            move |_, cr, width, height| {
                let side = width.min(height) as f64 * 0.82;
                let ox = width as f64 / 2.0;
                let oy = height as f64 / 2.0;
                let to_screen = |p: ShapePoint| (ox + p.x * side, oy + p.y * side);
                let path = shape_path.borrow();
                if path.anchors.is_empty() {
                    return;
                }
                let (x, y) = to_screen(path.anchors[0].point);
                cr.move_to(x, y);
                for index in 0..path.anchors.len() {
                    let anchor = path.anchors[index];
                    let next = path.anchors[(index + 1) % path.anchors.len()];
                    let c1 = to_screen(anchor.outgoing);
                    let c2 = to_screen(next.incoming);
                    let end = to_screen(next.point);
                    cr.curve_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
                }
                cr.close_path();
                cr.set_source_rgba(0.15, 0.55, 0.95, 0.18);
                let _ = cr.fill_preserve();
                cr.set_source_rgb(0.15, 0.55, 0.95);
                cr.set_line_width(2.0);
                let _ = cr.stroke();
                let active = path.anchors[selected.get()];
                let anchor_screen = to_screen(active.point);
                for (part, handle) in [(-1, active.incoming), (1, active.outgoing)] {
                    let (hx, hy) = to_screen(handle);
                    cr.move_to(anchor_screen.0, anchor_screen.1);
                    cr.line_to(hx, hy);
                    cr.set_source_rgba(0.45, 0.68, 1.0, 0.7);
                    cr.set_line_width(1.5);
                    let _ = cr.stroke();
                    cr.arc(
                        hx,
                        hy,
                        if selected_part.get() == part {
                            7.0
                        } else {
                            4.5
                        },
                        0.0,
                        std::f64::consts::TAU,
                    );
                    if selected_part.get() == part {
                        cr.set_source_rgb(1.0, 0.35, 0.12);
                    } else {
                        cr.set_source_rgb(0.45, 0.68, 1.0);
                    }
                    let _ = cr.fill();
                    if selected_part.get() == part {
                        cr.arc(hx, hy, 7.0, 0.0, std::f64::consts::TAU);
                        cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
                        cr.set_line_width(1.5);
                        let _ = cr.stroke();
                    }
                }
                for (index, anchor) in path.anchors.iter().enumerate() {
                    let (x, y) = to_screen(anchor.point);
                    cr.arc(
                        x,
                        y,
                        if index == selected.get() { 7.0 } else { 5.0 },
                        0.0,
                        std::f64::consts::TAU,
                    );
                    if index == selected.get() {
                        cr.set_source_rgb(1.0, 0.35, 0.12)
                    } else {
                        cr.set_source_rgb(0.15, 0.55, 0.95)
                    };
                    let _ = cr.fill();
                }
            }
        ));
        connect_shape_editor_click(&area, &nodes, &shape_path, &selected, &selected_part);
        let drag = gtk::GestureDrag::new();
        let drag_origin = Rc::new(Cell::new(ShapePoint { x: 0.0, y: 0.0 }));
        let drag_node = Rc::new(Cell::new(None::<usize>));
        let drag_part = Rc::new(Cell::new(0i8));
        drag.connect_drag_begin(glib::clone!(
            #[strong]
            nodes,
            #[strong]
            shape_path,
            #[strong]
            selected,
            #[strong]
            selected_part,
            #[strong]
            drag_origin,
            #[strong]
            drag_node,
            #[strong]
            drag_part,
            #[weak]
            area,
            move |_, x, y| {
                let side = area.width().min(area.height()) as f64 * 0.82;
                let point = ShapePoint {
                    x: (x - area.width() as f64 / 2.0) / side,
                    y: (y - area.height() as f64 / 2.0) / side,
                };
                let mut hit = shape_node_hit_test(&nodes.borrow(), point, 0.045);
                let mut part = 0;
                if hit.is_none() {
                    let anchor = shape_path.borrow().anchors[selected.get()];
                    for (candidate, handle) in [(-1, anchor.incoming), (1, anchor.outgoing)] {
                        if (handle.x - point.x).hypot(handle.y - point.y) <= 0.045 {
                            hit = Some(selected.get());
                            part = candidate;
                        }
                    }
                }
                drag_node.set(hit);
                drag_part.set(part);
                if let Some(index) = hit {
                    selected.set(index);
                    selected_part.set(part);
                    let anchor = shape_path.borrow().anchors[index];
                    drag_origin.set(match part {
                        -1 => anchor.incoming,
                        1 => anchor.outgoing,
                        _ => anchor.point,
                    });
                    area.queue_draw();
                }
            }
        ));
        drag.connect_drag_update(glib::clone!(
            #[strong]
            nodes,
            #[strong]
            shape_path,
            #[strong]
            drag_origin,
            #[strong]
            drag_node,
            #[strong]
            drag_part,
            #[weak]
            area,
            move |_, dx, dy| {
                let side = area.width().min(area.height()) as f64 * 0.82;
                let origin = drag_origin.get();
                if let Some(index) = drag_node.get() {
                    let point = ShapePoint {
                        x: (origin.x + dx / side).clamp(-0.75, 0.75),
                        y: (origin.y + dy / side).clamp(-0.75, 0.75),
                    };
                    let mut path = shape_path.borrow_mut();
                    match drag_part.get() {
                        -1 => path.anchors[index].incoming = point,
                        1 => path.anchors[index].outgoing = point,
                        _ => translate_shape_anchor(&mut path, index, point),
                    }
                    nodes.borrow_mut()[index] = path.anchors[index].point;
                    area.queue_draw();
                }
            }
        ));
        area.add_controller(drag);
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[strong]
            nodes,
            #[strong]
            shape_path,
            #[strong]
            selected,
            #[strong]
            selected_part,
            #[strong]
            area,
            #[strong]
            feedback,
            #[weak]
            dialog,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                if key == gdk::Key::Escape {
                    dialog.close();
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::Delete || key == gdk::Key::BackSpace {
                    let mut path = shape_path.borrow_mut();
                    if delete_shape_anchor(&mut path, selected.get()) {
                        *nodes.borrow_mut() = path.anchors.iter().map(|a| a.point).collect();
                        selected.set(selected.get().min(path.anchors.len() - 1));
                        selected_part.set(0);
                        feedback.set_text("");
                    } else {
                        feedback.set_text("A mark needs at least three nodes.");
                    }
                    area.queue_draw();
                    return glib::Propagation::Stop;
                }
                let (dx, dy) = match key {
                    gdk::Key::Left => (-0.005, 0.0),
                    gdk::Key::Right => (0.005, 0.0),
                    gdk::Key::Up => (0.0, -0.005),
                    gdk::Key::Down => (0.0, 0.005),
                    _ => (0.0, 0.0),
                };
                if dx != 0.0 || dy != 0.0 {
                    let mut path = shape_path.borrow_mut();
                    let index = selected.get();
                    let mut point = match selected_part.get() {
                        -1 => path.anchors[index].incoming,
                        1 => path.anchors[index].outgoing,
                        _ => path.anchors[index].point,
                    };
                    point.x = (point.x + dx).clamp(-0.75, 0.75);
                    point.y = (point.y + dy).clamp(-0.75, 0.75);
                    match selected_part.get() {
                        -1 => path.anchors[index].incoming = point,
                        1 => path.anchors[index].outgoing = point,
                        _ => translate_shape_anchor(&mut path, index, point),
                    }
                    nodes.borrow_mut()[index] = path.anchors[index].point;
                    area.queue_draw();
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            }
        ));
        area.add_controller(keys);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        let cancel = gtk::Button::with_label("Cancel");
        let done = gtk::Button::with_label("Done");
        done.add_css_class("suggested-action");
        actions.append(&cancel);
        actions.append(&done);
        connect_clicked(
            &cancel,
            self,
            glib::clone!(
                #[weak]
                dialog,
                move |_| dialog.close()
            ),
        );
        connect_clicked(
            &done,
            self,
            glib::clone!(
                #[strong]
                shape_path,
                #[strong]
                feedback,
                #[weak]
                dialog,
                move |ui| {
                    let candidate = shape_path.borrow().clone();
                    if let Err(error) = toniator::model::validate_shape_path(&candidate) {
                        feedback.set_text(&error.to_string());
                        return;
                    }
                    ui.change_web_treatment(move |settings, inks| {
                        if settings.use_shared_mark {
                            settings.custom_nodes = candidate
                                .anchors
                                .iter()
                                .map(|anchor| anchor.point)
                                .collect();
                            settings.custom_shape_path = Some(candidate.clone());
                            settings.shared_shape = WebShape::UserDefined;
                        } else {
                            for ink in inks {
                                let channel = settings.channels.get_mut(ink);
                                channel.custom_shape_path = Some(candidate.clone());
                                channel.shape = WebShape::UserDefined;
                            }
                        }
                    });
                    dialog.close();
                }
            ),
        );
        root.append(&instructions);
        root.append(&area);
        root.append(&feedback);
        root.append(&actions);
        dialog.set_content(Some(&root));
        if self.artifact_shape_editor {
            self.capture_override
                .borrow_mut()
                .replace(dialog.clone().upcast());
        }
        dialog.connect_map(glib::clone!(
            #[weak]
            area,
            move |dialog| gtk::prelude::GtkWindowExt::set_focus(dialog, Some(&area))
        ));
        dialog.present();
    }

    fn install_curved_shape_fixture(self: &Rc<Self>) {
        let path = curved_shape_fixture();
        self.change_web_treatment(move |settings, _| {
            settings.custom_nodes = path.anchors.iter().map(|anchor| anchor.point).collect();
            settings.custom_shape_path = Some(path.clone());
            settings.shared_shape = WebShape::UserDefined;
        });
    }

    fn install_independent_shape_fixture(self: &Rc<Self>) {
        let cubic = curved_shape_fixture();
        self.change_web_treatment(move |settings, _| {
            settings.use_shared_mark = false;
            settings.channels.c.shape = WebShape::Circle;
            settings.channels.m.shape = WebShape::RegularPolygon;
            settings.channels.m.polygon_sides = 3;
            settings.channels.y.shape = WebShape::RegularPolygon;
            settings.channels.y.polygon_sides = 6;
            settings.channels.k.shape = WebShape::UserDefined;
            settings.channels.k.custom_shape_path = Some(cubic);
        });
    }

    fn selected_curve_inks(&self) -> Option<Vec<Ink>> {
        let (rgb, crosshatch) = self.curve_output_flags()?;
        web_inks_for_target(self.curve_target.selected(), rgb, crosshatch)
    }

    fn activate_curve_treatment(self: &Rc<Self>) {
        let document = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            if matches!(
                editor.document().artwork_pipeline.assignment,
                ChannelAssignment::LegacyCompatibility(
                    LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1
                )
            ) {
                if !editor.exit_crosshatch_treatment() {
                    return;
                }
            } else if editor.document().pattern_state.selected_pattern_id()
                != Some(PatternId::COMPATIBILITY_CURVES_V1)
                && editor.document().saved_web_curve.is_some()
            {
                if !editor.restore_saved_curve() {
                    return;
                }
            } else if editor.document().pattern_state.selected_pattern_id()
                != Some(PatternId::COMPATIBILITY_CURVES_V1)
                && !editor.select_pattern(PatternId::COMPATIBILITY_CURVES_V1)
            {
                return;
            }
            editor.document().clone()
        };
        self.after_treatment_edit(document);
    }

    fn activate_shape_treatment(self: &Rc<Self>) {
        let document = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            if editor.document().pattern_state.selected_pattern_id()
                != Some(PatternId::COMPATIBILITY_SHAPES_V1)
                && editor.document().saved_web_shape.is_some()
            {
                if !editor.restore_saved_shape() {
                    return;
                }
            } else if editor.document().pattern_state.selected_pattern_id()
                != Some(PatternId::COMPATIBILITY_SHAPES_V1)
                && !editor.select_pattern(PatternId::COMPATIBILITY_SHAPES_V1)
            {
                return;
            }
            editor.document().clone()
        };
        self.after_treatment_edit(document);
    }

    fn activate_weighted_voronoi_treatment(self: &Rc<Self>) {
        let document = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            if editor.document().pattern_state.selected_pattern_id()
                != Some(PatternId::WEIGHTED_VORONOI_V1)
                && !editor.select_pattern(PatternId::WEIGHTED_VORONOI_V1)
            {
                return;
            }
            editor.document().clone()
        };
        self.after_treatment_edit(document);
    }

    fn selected_weighted_voronoi_channel(&self) -> Option<OutputChannelId> {
        let output_model = self
            .state
            .borrow()
            .editor
            .as_ref()
            .map(|editor| editor.document().artwork_pipeline.output_model)?;
        output_model
            .channels()
            .get(self.weighted_voronoi_channel.selected() as usize)
            .copied()
    }

    fn weighted_voronoi_channel_at(&self, index: usize) -> Option<OutputChannelId> {
        let output_model = self
            .state
            .borrow()
            .editor
            .as_ref()
            .map(|editor| editor.document().artwork_pipeline.output_model)?;
        output_model.channels().get(index).copied()
    }

    fn change_weighted_voronoi_settings(
        &self,
        update: impl FnOnce(&mut WeightedVoronoiSettings, OutputChannelId),
    ) {
        let Some(channel) = self.selected_weighted_voronoi_channel() else {
            return;
        };
        self.change_weighted_voronoi_settings_for_channel(channel, update);
    }

    fn change_weighted_voronoi_settings_for_channel(
        &self,
        channel: OutputChannelId,
        update: impl FnOnce(&mut WeightedVoronoiSettings, OutputChannelId),
    ) {
        let document = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            let Ok(mut settings) = editor.document().pattern_state.weighted_voronoi_settings()
            else {
                return;
            };
            update(&mut settings, channel);
            if !editor.set_weighted_voronoi_settings(settings) {
                return;
            }
            editor.document().clone()
        };
        self.after_treatment_edit(document);
    }

    fn apply_weighted_voronoi_seed(&self, entry: &gtk::Entry) {
        if self.state.borrow().syncing_controls {
            return;
        }
        let Ok(seed) = entry.text().parse::<u64>() else {
            self.sync_controls();
            return;
        };
        self.change_weighted_voronoi_settings(move |settings, channel| {
            settings
                .channel_settings_mut(channel)
                .expect("registered semantic channel")
                .seed = seed;
        });
    }

    fn apply_curve_profile(self: &Rc<Self>, path: CurvePath) {
        self.change_curve_treatment(move |settings, inks| {
            if settings.use_shared_curve {
                settings.shared_path = path;
            } else {
                for ink in inks {
                    settings.channels.get_mut(ink).path = path.clone();
                }
            }
        });
    }

    fn reset_crosshatch_path(self: &Rc<Self>) {
        self.change_curve_treatment(|settings, inks| reset_crosshatch_curve_path(settings, &inks));
    }

    fn change_curve_treatment(
        self: &Rc<Self>,
        update: impl FnOnce(&mut WebCurveSettings, Vec<Ink>),
    ) {
        let Some(inks) = self.selected_curve_inks() else {
            return;
        };
        let (document, output_changed) = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            let output_mode = editor.document().output_mode;
            let Ok(mut settings) = editor.document().pattern_state.curve_settings() else {
                return;
            };
            update(&mut settings, inks);
            if !editor.set_curve_settings(settings) {
                return;
            }
            (
                editor.document().clone(),
                editor.document().output_mode != output_mode,
            )
        };
        if output_changed {
            self.after_output_mode_edit(document);
        } else {
            self.after_treatment_edit(document);
        }
    }

    fn after_treatment_edit(&self, document: Document) {
        self.state.borrow_mut().rendered_cache = None;
        self.queue_autosave(document);
        self.sync_controls();
        self.request_rendered_preview();
        self.update_actions();
    }

    fn after_output_mode_edit(self: &Rc<Self>, document: Document) {
        self.state.borrow_mut().rendered_cache = None;
        self.queue_autosave(document);
        self.request_rendered_preview();
        self.update_actions();
        // Output changes alter the source-mapping labels and target/output
        // item lists.  Do that work after the active DropDown's
        // selected-notify stack has returned, rather than splicing a live
        // GtkListView model from its activation callback.
        self.sync_controls_when_idle();
    }

    fn sync_controls_when_idle(self: &Rc<Self>) {
        glib::idle_add_local_once(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move || ui.sync_controls()
        ));
    }

    fn update_appearance(&self, change: impl FnOnce(&mut DocumentAppearance)) {
        let changed = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            let mut appearance = editor.document().appearance;
            change(&mut appearance);
            editor.set_appearance(appearance)
        };
        if changed {
            // Keep the RefCell borrow scoped to this clone.  Calling
            // after_treatment_edit while it is live re-enters state through
            // borrow_mut (to clear the rendered cache) from the DropDown
            // selected-notify trampoline.
            let document = {
                let state = self.state.borrow();
                state.editor.as_ref().unwrap().document().clone()
            };
            self.after_treatment_edit(document);
        }
    }

    fn apply_artifact_appearance(
        &self,
        preview_surface: Option<&str>,
        export_background: Option<&str>,
        expand_document: bool,
    ) {
        if expand_document {
            self.source_section.set_expanded(true);
            self.output_section.set_expanded(true);
            self.channel_settings_section.set_expanded(true);
        }
        let mut state = self.state.borrow_mut();
        let Some(editor) = state.editor.as_mut() else {
            return;
        };
        let mut appearance = editor.document().appearance;
        if let Some(value) = preview_surface {
            appearance.preview_surface = if value == "checkerboard" {
                PreviewSurface::Checkerboard
            } else if let Some(color) = parse_artifact_rgba(value) {
                PreviewSurface::Color { color }
            } else {
                return;
            };
        }
        if let Some(value) = export_background {
            appearance.export_background = if value == "none" {
                ExportBackground::None
            } else if let Some(color) = parse_artifact_rgba(value) {
                ExportBackground::Color { color }
            } else {
                return;
            };
        }
        editor.set_appearance(appearance);
        drop(state);
        self.sync_controls();
        self.request_preview();
    }

    fn change_web_treatment(
        self: &Rc<Self>,
        update: impl FnOnce(&mut toniator::WebShapeSettings, Vec<Ink>),
    ) {
        let Some(inks) = self.selected_web_inks() else {
            return;
        };
        let (document, output_changed) = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            let output_mode = editor.document().output_mode;
            if editor.document().pattern_state.selected_pattern_id()
                != Some(PatternId::COMPATIBILITY_SHAPES_V1)
            {
                return;
            }
            let Ok(mut settings) = editor.document().pattern_state.shape_settings() else {
                return;
            };
            update(&mut settings, inks);
            if !editor.set_shape_settings(settings) {
                return;
            }
            (
                editor.document().clone(),
                editor.document().output_mode != output_mode,
            )
        };
        if output_changed {
            self.after_output_mode_edit(document);
        } else {
            self.after_treatment_edit(document);
        }
    }

    fn enable_shared_shape(self: &Rc<Self>) {
        let selected_ink = self
            .selected_web_inks()
            .and_then(|inks| inks.first().copied());
        let (target, equal, rgb) = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                return;
            };
            if editor.document().pattern_state.selected_pattern_id()
                != Some(PatternId::COMPATIBILITY_SHAPES_V1)
            {
                return;
            }
            let Ok(settings) = editor.document().pattern_state.shape_settings() else {
                return;
            };
            let crosshatch = document_uses_crosshatch(editor.document());
            let rgb = editor.document().artwork_pipeline.output_model == OutputModel::RgbScreen
                && !crosshatch;
            let channel_order = output_channel_order(rgb, crosshatch);
            let first = settings.channels.get(channel_order[0]);
            let equal = channel_order.iter().skip(1).copied().all(|ink| {
                let channel = settings.channels.get(ink);
                channel.shape == first.shape
                    && channel.polygon_sides == first.polygon_sides
                    && settings.resolved_channel_shape_path(channel)
                        == settings.resolved_channel_shape_path(first)
            });
            (self.web_target.selected(), equal, rgb)
        };
        if equal {
            self.share_shape_from(if rgb { Ink::Red } else { Ink::Cyan });
            return;
        }
        self.state.borrow_mut().syncing_controls = true;
        self.web_shared.set_active(false);
        self.state.borrow_mut().syncing_controls = false;
        let dialog = adw::AlertDialog::builder()
            .heading(if target == 0 {
                "Choose Shape to Share"
            } else {
                "Share this ink's shape?"
            })
            .body(if rgb {
                "This replaces the other channels' shape geometry as one undoable change."
            } else {
                "This replaces the other inks' shape geometry as one undoable change."
            })
            .build();
        if target == 0 {
            if rgb {
                dialog.add_responses(&[
                    ("cancel", "Cancel"),
                    ("r", "Red"),
                    ("g", "Green"),
                    ("b", "Blue"),
                ]);
            } else {
                dialog.add_responses(&[
                    ("cancel", "Cancel"),
                    ("c", "C"),
                    ("m", "M"),
                    ("y", "Y"),
                    ("k", "K"),
                ]);
            }
        } else {
            dialog.add_responses(&[("cancel", "Cancel"), ("share", "Share")]);
            dialog.set_response_appearance("share", adw::ResponseAppearance::Suggested);
        }
        dialog.set_close_response("cancel");
        dialog.choose(
            Some(&self.window),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |response| {
                    let ink = if target == 0 {
                        match response.as_str() {
                            "r" if rgb => Some(Ink::Red),
                            "g" if rgb => Some(Ink::Green),
                            "b" if rgb => Some(Ink::Blue),
                            "c" => Some(Ink::Cyan),
                            "m" => Some(Ink::Magenta),
                            "y" => Some(Ink::Yellow),
                            "k" => Some(Ink::Black),
                            _ => None,
                        }
                    } else if response == "share" {
                        selected_ink
                    } else {
                        None
                    };
                    if let Some(ink) = ink {
                        ui.share_shape_from(ink);
                    }
                }
            ),
        );
    }

    fn share_shape_from(self: &Rc<Self>, ink: Ink) {
        self.change_web_treatment(move |settings, _| {
            let source = settings.channels.get(ink).clone();
            let path = settings.resolved_channel_shape_path(&source);
            settings.shared_shape = source.shape;
            settings.polygon_sides = source.polygon_sides;
            settings.custom_shape_path = Some(path);
            settings.use_shared_mark = true;
        });
    }

    fn begin_setting_edit(&self, key: SettingKey) {
        if let Some(editor) = self.state.borrow_mut().editor.as_mut() {
            editor.begin_edit(key);
        }
    }

    fn end_setting_edit(&self) {
        if let Some(editor) = self.state.borrow_mut().editor.as_mut() {
            editor.end_edit();
        }
        self.update_actions();
    }

    fn undo(self: &Rc<Self>) {
        let changed = self
            .state
            .borrow_mut()
            .editor
            .as_mut()
            .is_some_and(DocumentEditor::undo);
        if changed {
            self.after_history_change();
        }
    }

    fn redo(self: &Rc<Self>) {
        let changed = self
            .state
            .borrow_mut()
            .editor
            .as_mut()
            .is_some_and(DocumentEditor::redo);
        if changed {
            self.after_history_change();
        }
    }

    fn after_history_change(self: &Rc<Self>) {
        self.sync_controls();
        let history_state = {
            let state = self.state.borrow();
            state.editor.as_ref().map(|editor| {
                (
                    editor.document().clone(),
                    state.path.is_some() && !editor.is_dirty(),
                )
            })
        };
        if let Some((document, clean_saved)) = history_state {
            if clean_saved {
                match self.invalidate_and_clear_recovery(&document.document_id) {
                    Ok(()) => self
                        .autosave_status
                        .set_text("Saved state restored — no recovery needed"),
                    Err(error) => self.show_error(&format!(
                        "Could not reconcile recovery with saved state: {error:#}"
                    )),
                }
            } else {
                self.queue_autosave(document);
            }
        }
        self.request_preview();
        self.update_actions();
    }

    fn sync_shapes_schema_metadata(&self) {
        let descriptor = |control_id| {
            PATTERN_REGISTRY
                .parameter_for_control(PatternId::COMPATIBILITY_SHAPES_V1, control_id)
                .expect("Shapes control must have registered schema metadata")
        };
        let shared = descriptor("web_shared");
        self.web_shared.set_tooltip_text(Some(shared.help));
        self.web_shared
            .update_property(&[gtk::accessible::Property::Description(shared.help)]);

        let mark = descriptor("web_shape");
        self.web_shape.set_tooltip_text(Some(mark.help));
        self.web_shape
            .update_property(&[gtk::accessible::Property::Description(mark.help)]);

        let polygon_sides = descriptor("web_polygon_sides");
        self.web_polygon_sides_label.set_text(polygon_sides.label);
        self.web_polygon_sides
            .set_tooltip_text(Some(polygon_sides.help));
        self.web_polygon_sides
            .update_property(&[gtk::accessible::Property::Description(polygon_sides.help)]);

        let edit_shape = descriptor("web_edit_shape");
        self.web_edit_shape.set_label(edit_shape.label);
        self.web_edit_shape.set_tooltip_text(Some(edit_shape.help));
        self.web_edit_shape.update_property(&[
            gtk::accessible::Property::Label(edit_shape.label),
            gtk::accessible::Property::Description(edit_shape.help),
        ]);

        let visible_channels = descriptor("web_visible_row");
        self.web_visible_label
            .set_tooltip_text(Some(visible_channels.help));
        self.web_visible_label
            .update_property(&[gtk::accessible::Property::Description(
                visible_channels.help,
            )]);
    }

    fn sync_curves_schema_metadata(&self) {
        let descriptor = |control_id| {
            PATTERN_REGISTRY
                .parameter_for_control(PatternId::COMPATIBILITY_CURVES_V1, control_id)
                .expect("Curves control must have registered schema metadata")
        };
        let set_description = |control: &gtk::Widget, control_id| {
            let descriptor = descriptor(control_id);
            control.set_tooltip_text(Some(descriptor.help));
            control.update_property(&[gtk::accessible::Property::Description(descriptor.help)]);
        };

        set_description(self.curve_layout.upcast_ref(), "curve_layout");
        set_description(self.curve_editor.upcast_ref(), "curve_editor");
        set_description(self.curve_shared.upcast_ref(), "curve_shared");
        set_description(self.curve_color.upcast_ref(), "curve_color");
        set_description(self.curve_crosshatch_color.upcast_ref(), "curve_color");
        set_description(self.curve_weight.upcast_ref(), "curve_weight_scale");
        set_description(self.curve_spacing.upcast_ref(), "curve_spacing_scale");
        set_description(self.curve_coverage.upcast_ref(), "curve_coverage_scale");
        set_description(self.curve_angle.upcast_ref(), "curve_angle_scale");
        set_description(self.curve_position_x.upcast_ref(), "curve_position_x_scale");
        set_description(self.curve_position_y.upcast_ref(), "curve_position_y_scale");
        set_description(self.curve_opacity.upcast_ref(), "curve_opacity_scale");
        set_description(self.curve_threshold.upcast_ref(), "curve_threshold_scale");
        set_description(self.curve_detail.upcast_ref(), "curve_detail_scale");
        set_description(self.motif_controls.upcast_ref(), "motif_controls");

        let visible_channels = descriptor("curve_visible_row");
        self.curve_visible_label
            .set_tooltip_text(Some(visible_channels.help));
        self.curve_visible_label
            .update_property(&[gtk::accessible::Property::Description(
                visible_channels.help,
            )]);
    }

    fn sync_weighted_voronoi_controls(
        &self,
        settings: &WeightedVoronoiSettings,
        output_model: OutputModel,
    ) {
        let descriptor = |control_id| {
            PATTERN_REGISTRY
                .parameter_for_control(PatternId::WEIGHTED_VORONOI_V1, control_id)
                .expect("Weighted Voronoi control must have registered schema metadata")
        };
        let set_description = |control: &gtk::Widget, control_id| {
            let descriptor = descriptor(control_id);
            control.set_tooltip_text(Some(descriptor.help));
            control.update_property(&[gtk::accessible::Property::Description(descriptor.help)]);
        };
        set_description(
            self.weighted_voronoi_cell_count.upcast_ref(),
            "weighted_voronoi_cell_count",
        );
        set_description(
            self.weighted_voronoi_arrangement.upcast_ref(),
            "weighted_voronoi_arrangement",
        );
        set_description(
            self.weighted_voronoi_placement.upcast_ref(),
            "weighted_voronoi_placement",
        );
        set_description(
            self.weighted_voronoi_density_strength.upcast_ref(),
            "weighted_voronoi_density_strength",
        );
        set_description(
            self.weighted_voronoi_response_strength.upcast_ref(),
            "weighted_voronoi_response_strength",
        );
        set_description(
            self.weighted_voronoi_boundary_gap.upcast_ref(),
            "weighted_voronoi_boundary_gap",
        );
        set_description(
            self.weighted_voronoi_seed.upcast_ref(),
            "weighted_voronoi_seed",
        );
        let visible = descriptor("weighted_voronoi_visible");
        for button in &self.weighted_voronoi_visible {
            button.set_tooltip_text(Some(visible.help));
            button.update_property(&[gtk::accessible::Property::Description(visible.help)]);
        }

        let channels = output_model.channels();
        sync_dropdown_strings(
            &self.weighted_voronoi_channel,
            &channels
                .iter()
                .map(|channel| channel.label())
                .collect::<Vec<_>>(),
        );
        let selected = self.weighted_voronoi_channel.selected() as usize;
        let selected = if selected < channels.len() {
            selected
        } else {
            0
        };
        self.weighted_voronoi_channel.set_selected(selected as u32);
        let channel = channels[selected];
        let channel_settings = settings
            .channel_settings(channel)
            .expect("persisted settings must contain every semantic channel");

        self.weighted_voronoi_cell_count
            .set_value(channel_settings.cell_count as f64);
        self.weighted_voronoi_arrangement
            .set_selected(match channel_settings.arrangement {
                WeightedVoronoiArrangementPolicy::Shared => 0,
                WeightedVoronoiArrangementPolicy::Independent => 1,
            });
        self.weighted_voronoi_placement
            .set_selected(match channel_settings.placement {
                WeightedVoronoiPlacementMode::SourceWeighted => 0,
                WeightedVoronoiPlacementMode::Uniform => 1,
            });
        self.weighted_voronoi_density_strength
            .set_value(channel_settings.density_strength);
        self.weighted_voronoi_response_strength
            .set_value(channel_settings.response_strength);
        self.weighted_voronoi_boundary_gap
            .set_value(channel_settings.boundary_gap);
        self.weighted_voronoi_seed
            .set_text(&channel_settings.seed.to_string());
        self.weighted_voronoi_density_strength
            .set_sensitive(matches!(
                channel_settings.placement,
                WeightedVoronoiPlacementMode::SourceWeighted
            ));

        for (index, button) in self.weighted_voronoi_visible.iter().enumerate() {
            let Some(channel) = channels.get(index).copied() else {
                button.set_visible(false);
                continue;
            };
            button.set_visible(true);
            button.set_label(Some(match channel {
                OutputChannelId::CmykCyan => "C",
                OutputChannelId::CmykMagenta => "M",
                OutputChannelId::CmykYellow => "Y",
                OutputChannelId::CmykBlack => "K",
                OutputChannelId::RgbRed => "R",
                OutputChannelId::RgbGreen => "G",
                OutputChannelId::RgbBlue => "B",
            }));
            button.set_active(
                settings
                    .channel_settings(channel)
                    .expect("persisted settings must contain every semantic channel")
                    .enabled,
            );
        }
    }

    fn sync_pattern_selector(
        &self,
        selected_pattern: Option<PatternId>,
        selected_panel: Option<PatternInspectorPanel>,
        native_treatment: Treatment,
    ) {
        let shapes = PATTERN_REGISTRY
            .get(PatternId::COMPATIBILITY_SHAPES_V1)
            .expect("Shapes selector must have registered metadata");
        let curves = PATTERN_REGISTRY
            .get(PatternId::COMPATIBILITY_CURVES_V1)
            .expect("Curves selector must have registered metadata");
        let weighted_voronoi = PATTERN_REGISTRY
            .get(PatternId::WEIGHTED_VORONOI_V1)
            .expect("Weighted Voronoi selector must have registered metadata");
        for (button, metadata) in [
            (&self.dots, shapes),
            (&self.curves, curves),
            (&self.weighted_voronoi, weighted_voronoi),
        ] {
            button.set_label(metadata.selector.label);
            button.set_tooltip_text(Some(metadata.selector.help));
            button.update_property(&[
                gtk::accessible::Property::Label(metadata.selector.label),
                gtk::accessible::Property::Description(metadata.selector.help),
            ]);
        }

        let shapes_selected = matches!(
            (selected_pattern, selected_panel),
            (
                Some(PatternId::COMPATIBILITY_SHAPES_V1),
                Some(PatternInspectorPanel::Shapes)
            )
        );
        let curves_selected = matches!(
            (selected_pattern, selected_panel),
            (
                Some(PatternId::COMPATIBILITY_CURVES_V1),
                Some(PatternInspectorPanel::Curves)
            )
        );
        let weighted_voronoi_selected = matches!(
            (selected_pattern, selected_panel),
            (
                Some(PatternId::WEIGHTED_VORONOI_V1),
                Some(PatternInspectorPanel::WeightedVoronoi)
            )
        );

        self.dots.set_active(shapes_selected);
        self.curves.set_active(curves_selected);
        self.weighted_voronoi.set_active(weighted_voronoi_selected);
        self.squares.set_active(false);
        self.lines.set_active(false);
        self.legacy
            .set_visible(!shapes_selected && !curves_selected && !weighted_voronoi_selected);
        self.legacy
            .set_active(!shapes_selected && !curves_selected && !weighted_voronoi_selected);

        if shapes_selected {
            self.treatment_modes.set_visible_child_name("web");
        } else if curves_selected {
            self.treatment_modes.set_visible_child_name("curve");
        } else if weighted_voronoi_selected {
            self.treatment_modes
                .set_visible_child_name("weighted-voronoi");
        } else {
            self.legacy.set_label(match native_treatment {
                Treatment::Dots => "Legacy Dots",
                Treatment::Squares => "Legacy Squares",
                Treatment::Lines => "Legacy Lines",
            });
            self.treatment_modes.set_visible_child_name("native");
        }
    }

    fn sync_controls(&self) {
        let Some((
            settings,
            appearance,
            pipeline,
            source_text,
            selected_pattern,
            selected_panel,
            shape_settings,
            curve_settings,
            weighted_voronoi_settings,
        )) = self.state.borrow().editor.as_ref().map(|editor| {
            let document = editor.document();
            (
                document.settings,
                document.appearance,
                document.artwork_pipeline.clone(),
                editor_source_text(document),
                document.pattern_state.selected_pattern_id(),
                document
                    .pattern_state
                    .selected_metadata()
                    .map(|metadata| metadata.selector.inspector_panel),
                (document.pattern_state.selected_pattern_id()
                    == Some(PatternId::COMPATIBILITY_SHAPES_V1))
                .then(|| document.pattern_state.shape_settings())
                .transpose()
                .ok()
                .flatten(),
                (document.pattern_state.selected_pattern_id()
                    == Some(PatternId::COMPATIBILITY_CURVES_V1))
                .then(|| document.pattern_state.curve_settings())
                .transpose()
                .ok()
                .flatten(),
                (document.pattern_state.selected_pattern_id()
                    == Some(PatternId::WEIGHTED_VORONOI_V1))
                .then(|| document.pattern_state.weighted_voronoi_settings())
                .transpose()
                .ok()
                .flatten(),
            )
        })
        else {
            return;
        };
        let output_model = pipeline.output_model;
        let output_mode = output_model.to_legacy();
        let crosshatch = pipeline_uses_crosshatch(&pipeline);
        self.state.borrow_mut().syncing_controls = true;
        self.output_mode
            .set_selected(if output_mode == OutputMode::RgbScreen {
                1
            } else {
                0
            });
        sync_dropdown_strings(
            &self.artwork_source,
            &artwork_source_labels(matches!(
                pipeline.source,
                ArtworkSource::LegacyBrightness(_)
            )),
        );
        self.artwork_source
            .set_selected(artwork_source_index(pipeline.source));
        self.artwork_source_note
            .set_text(artwork_source_guidance(pipeline.source));
        sync_dropdown_strings(
            &self.source_alpha,
            &source_alpha_labels(pipeline.alpha_policy == SourceAlphaPolicy::LegacyCurrentV1),
        );
        self.source_alpha
            .set_selected(source_alpha_index(pipeline.alpha_policy));
        let alpha_is_source = pipeline.source == ArtworkSource::Alpha;
        self.source_alpha_row
            .set_visible(!crosshatch && !alpha_is_source);
        self.source_alpha_note.set_visible(alpha_is_source);
        self.source_alpha
            .set_sensitive(!crosshatch && !alpha_is_source);
        sync_dropdown_strings(
            &self.channel_assignment,
            &channel_assignment_labels(pipeline.source == ArtworkSource::FullColor, output_model),
        );
        self.channel_assignment
            .set_selected(channel_assignment_index(
                pipeline.assignment,
                pipeline.source,
            ));
        let full_color = pipeline.source == ArtworkSource::FullColor;
        self.channel_assignment
            .set_sensitive(!crosshatch && !full_color);
        self.channel_assignment_note.set_visible(full_color);
        self.channel_assignment_note.set_text(match output_model {
            OutputModel::CmykPrint => {
                "Automatic CMYK Separation derives cyan, magenta, yellow, and black inks."
            }
            OutputModel::RgbScreen => {
                "Direct RGB Channels map encoded red, green, and blue components."
            }
        });
        sync_dropdown_strings(&self.active_channel, &output_channel_labels(output_model));
        self.active_channel.set_selected(
            pipeline
                .active_channel
                .map(OutputChannelId::legacy_slot)
                .unwrap_or(0),
        );
        self.active_channel_row.set_visible(matches!(
            pipeline.assignment,
            ChannelAssignment::ActiveChannel
        ));
        self.active_channel.set_sensitive(!crosshatch);
        self.artwork_source.set_sensitive(!crosshatch);
        self.crosshatch_action.set_label(if crosshatch {
            "Exit Legacy Crosshatch"
        } else {
            "Use Legacy Crosshatch"
        });
        self.crosshatch_note.set_text(if crosshatch {
            "Legacy Crosshatch is active in Curves. Exit restores ordinary Curves."
        } else {
            "Legacy Crosshatch temporarily switches to Curves. Exit restores ordinary Curves."
        });
        if output_mode == OutputMode::RgbScreen {
            for target in [&self.web_target, &self.curve_target] {
                sync_dropdown_strings(target, &["All Channels", "Red", "Green", "Blue"]);
            }
            self.web_target_label.set_text("Adjust Channel");
            self.curve_target_label.set_text("Adjust Channel");
            self.web_visible_label.set_text("Visible RGB Channels");
            self.curve_visible_label.set_text("Visible RGB Channels");
        } else {
            for target in [&self.web_target, &self.curve_target] {
                sync_dropdown_strings(target, &["All Inks", "Cyan", "Magenta", "Yellow", "Black"]);
            }
        }
        match appearance.preview_surface {
            PreviewSurface::Checkerboard => self.preview_surface.set_selected(0),
            PreviewSurface::Color { color } => {
                self.preview_surface.set_selected(1);
                self.preview_color.set_rgba(&gdk_rgba(color));
            }
        }
        self.preview_color.set_sensitive(matches!(
            appearance.preview_surface,
            PreviewSurface::Color { .. }
        ));
        match appearance.export_background {
            ExportBackground::None => self.export_background.set_selected(0),
            ExportBackground::Color { .. } => self.export_background.set_selected(1),
        }
        sync_export_background_color_control(
            &self.export_color_label,
            &self.export_color,
            appearance.export_background,
        );
        self.export_color.set_sensitive(matches!(
            appearance.export_background,
            ExportBackground::Color { .. }
        ));
        self.detail.set_value(settings.detail as f64);
        self.coverage.set_value(settings.coverage as f64);
        self.contrast.set_value(settings.contrast as f64);
        self.angle.set_value(settings.angle as f64);
        self.sync_pattern_selector(selected_pattern, selected_panel, settings.treatment);
        if let Some(settings) = weighted_voronoi_settings.as_ref() {
            self.sync_weighted_voronoi_controls(settings, output_model);
        }
        self.angle
            .set_sensitive(settings.treatment != Treatment::Dots);
        if let Some(settings) = shape_settings.as_ref() {
            self.sync_shapes_schema_metadata();
            sync_layer_terminology(
                &self.web_target,
                &self.web_target_label,
                self.web_target_help.as_ref(),
                &self.web_visible_label,
                output_mode == OutputMode::RgbScreen,
                crosshatch,
            );
            if output_mode == OutputMode::RgbScreen && !crosshatch {
                self.web_target_label.set_text("Adjust Channel");
                self.web_visible_label.set_text("Visible RGB Channels");
            }
            self.web_crosshatch_color_row.set_visible(crosshatch);
            self.web_color_row.set_visible(!crosshatch);
            self.web_crosshatch_color
                .set_text(&settings.crosshatch_color);
            self.web_shared.set_active(settings.use_shared_mark);
            let shared_label = PATTERN_REGISTRY
                .parameter_for_control(PatternId::COMPATIBILITY_SHAPES_V1, "web_shared")
                .expect("Shapes shared control must have registered schema metadata")
                .label;
            self.web_shared.set_label(Some(&format!(
                "{shared_label} Across {}",
                if output_mode == OutputMode::RgbScreen && !crosshatch {
                    "Channels"
                } else {
                    "Inks"
                }
            )));
            if let Some(help) = self.web_shared_help.as_ref() {
                help.set_spec(
                    help_for(if output_mode == OutputMode::RgbScreen && !crosshatch {
                        "Share Mark Shape Across Channels"
                    } else {
                        "Share Mark Shape Across Inks"
                    })
                    .unwrap(),
                );
            }
            let channel_copy = output_mode == OutputMode::RgbScreen && !crosshatch;
            let mixed_target = web_mixed_target(channel_copy);
            self.sync_web_color_terminology(channel_copy);
            let visible_spec = help_for(if output_mode == OutputMode::RgbScreen && !crosshatch {
                "Visible RGB Channels"
            } else if crosshatch {
                "Visible Crosshatch Layers"
            } else {
                "Visible Inks"
            })
            .unwrap();
            self.web_visible_help.set_spec(visible_spec);
            for button in &self.web_visible {
                button.set_tooltip_text(Some(visible_spec.summary));
                button.update_property(&[gtk::accessible::Property::Description(
                    visible_spec.summary,
                )]);
            }
            if crosshatch {
                set_crosshatch_target_directions(
                    &self.web_target,
                    [
                        settings.channels.k.grid_rotation,
                        settings.channels.c.grid_rotation,
                        settings.channels.m.grid_rotation,
                        settings.channels.y.grid_rotation,
                    ],
                );
            }
            let selected_target = self.web_target.selected();
            let Some(inks) = web_inks_for_target(
                selected_target,
                output_mode == OutputMode::RgbScreen,
                crosshatch,
            ) else {
                // GTK briefly reports INVALID_LIST_POSITION while its model is
                // being replaced. Leave the current shape controls untouched.
                self.state.borrow_mut().syncing_controls = false;
                return;
            };
            let all_target = selected_target == 0;
            let channel_order =
                output_channel_order(output_mode == OutputMode::RgbScreen, crosshatch);
            let first_geometry = settings.channels.get(channel_order[0]);
            let geometry_mixed = !settings.use_shared_mark
                && channel_order.iter().skip(1).copied().any(|ink| {
                    let channel = settings.channels.get(ink);
                    channel.shape != first_geometry.shape
                        || channel.polygon_sides != first_geometry.polygon_sides
                        || settings.resolved_channel_shape_path(channel)
                            != settings.resolved_channel_shape_path(first_geometry)
                });
            let selected_channel = selected_target
                .checked_sub(1)
                .and_then(|index| {
                    visible_ink_for_slot(
                        index as usize,
                        output_mode == OutputMode::RgbScreen,
                        crosshatch,
                    )
                })
                .map(|ink| settings.channels.get(ink));
            let displayed_shape = if settings.use_shared_mark {
                settings.shared_shape
            } else {
                selected_channel.unwrap_or(first_geometry).shape
            };
            self.web_shape
                .set_selected(if all_target && geometry_mixed {
                    gtk::INVALID_LIST_POSITION
                } else {
                    match displayed_shape {
                        WebShape::Circle => 0,
                        WebShape::RegularPolygon
                        | WebShape::Rectangle
                        | WebShape::Triangle
                        | WebShape::Pentagon
                        | WebShape::Hexagon => 1,
                        WebShape::UserDefined => 2,
                    }
                });
            let displayed_sides = if settings.use_shared_mark {
                settings.polygon_sides
            } else {
                selected_channel.map_or(settings.polygon_sides, |channel| channel.polygon_sides)
            };
            self.web_polygon_sides.set_value(displayed_sides as f64);
            let polygon_active = !(all_target && geometry_mixed)
                && matches!(
                    displayed_shape,
                    WebShape::RegularPolygon
                        | WebShape::Rectangle
                        | WebShape::Triangle
                        | WebShape::Pentagon
                        | WebShape::Hexagon
                );
            self.web_polygon_sides.set_visible(polygon_active);
            self.web_polygon_sides_row.set_visible(polygon_active);
            self.web_polygon_sides_label.set_visible(polygon_active);
            self.web_edit_shape.set_visible(
                !(all_target && geometry_mixed) && displayed_shape == WebShape::UserDefined,
            );
            self.web_shape_row
                .set_visible(!(all_target && geometry_mixed));
            self.web_mixed_shape_label
                .set_visible(all_target && geometry_mixed);
            self.web_mixed_shape_apply_row
                .set_visible(all_target && geometry_mixed);
            self.web_mixed_shape_apply.set_selected(0);
            self.web_geometry_note
                    .set_text(if settings.use_shared_mark {
                        if output_mode == OutputMode::RgbScreen && !crosshatch {
                            "One shape shared by all channels."
                        } else {
                            "One shape shared by all inks."
                        }
                    } else if all_target && geometry_mixed {
                        if output_mode == OutputMode::RgbScreen && !crosshatch {
                            "Shapes differ. Choose a mark to apply it to all channels, or edit one channel."
                        } else {
                            "Shapes differ. Choose a mark to apply it to all inks, or edit one ink."
                        }
                    } else {
                        if output_mode == OutputMode::RgbScreen && !crosshatch {
                            if all_target {
                                "Editing all channels' shapes."
                            } else {
                                "Editing this channel's shape."
                            }
                        } else {
                            if all_target {
                                "Editing all inks' shapes."
                            } else {
                                "Editing this ink's shape."
                            }
                        }
                    });
            for (index, button) in self.web_visible.iter().enumerate() {
                if output_mode == OutputMode::RgbScreen && !crosshatch && index == 3 {
                    button.set_visible(false);
                    continue;
                }
                button.set_visible(true);
                if output_mode == OutputMode::RgbScreen && !crosshatch {
                    let Some(ink) = Ink::RGB.get(index).copied() else {
                        continue;
                    };
                    button.set_label(Some(["Red", "Green", "Blue"][index]));
                    button.set_active(settings.channels.get(ink).enabled);
                    continue;
                }
                let Some(ink) =
                    visible_ink_for_slot(index, output_mode == OutputMode::RgbScreen, crosshatch)
                else {
                    continue;
                };
                button.set_label(Some(if crosshatch {
                    ["1 K", "2 C", "3 M", "4 Y"][index]
                } else {
                    ["C", "M", "Y", "K"][index]
                }));
                button.set_active(settings.channels.get(ink).enabled);
            }
            let all_inks = selected_target == 0;
            let first = if all_inks {
                &settings.base_channel
            } else {
                settings.channels.get(inks[0])
            };
            let differs = |value: fn(&toniator::WebShapeChannel) -> f64| {
                inks.iter()
                    .skip(1)
                    .any(|ink| (value(settings.channels.get(*ink)) - value(first)).abs() > 1e-9)
            };
            let mixed_fields = if all_inks {
                [false; 8]
            } else {
                [
                    differs(|c| c.scale),
                    differs(|c| c.grid_rotation),
                    differs(|c| c.rotation),
                    differs(|c| c.width_scale),
                    differs(|c| c.height_scale),
                    differs(|c| c.threshold),
                    differs(|c| c.opacity),
                    differs(|c| c.resolution_scale),
                ]
            };
            self.web_mixed
                .set_text(if mixed_fields.into_iter().any(|mixed| mixed) {
                    if channel_copy {
                        "Changing a Mixed control applies one value to every selected channel."
                    } else {
                        "Changing a Mixed control applies one value to every selected ink."
                    }
                } else {
                    ""
                });
            let colors_mixed = inks
                .iter()
                .skip(1)
                .any(|ink| settings.channels.get(*ink).color != first.color);
            self.web_color.set_sensitive(!all_inks);
            self.web_color.set_text(if all_inks || colors_mixed {
                ""
            } else {
                &first.color
            });
            self.web_color.set_placeholder_text(Some(if all_inks {
                if channel_copy {
                    "Select one channel"
                } else {
                    "Select one ink"
                }
            } else if colors_mixed {
                "Mixed"
            } else {
                "#RRGGBB"
            }));
            self.web_color_status.set_text(if all_inks {
                if channel_copy {
                    "Select one channel"
                } else {
                    "Select one ink"
                }
            } else if colors_mixed {
                "Mixed"
            } else {
                "Hex color"
            });
            sync_web_scale(
                &self.web_coverage,
                &self.web_coverage_status,
                first.scale,
                mixed_fields[0],
                "Mark size",
                mixed_target,
            );
            sync_web_scale(
                &self.web_angle,
                &self.web_angle_status,
                first.grid_rotation,
                mixed_fields[1],
                "Rotate ink screen",
                mixed_target,
            );
            sync_web_scale(
                &self.web_mark_angle,
                &self.web_mark_angle_status,
                first.rotation,
                mixed_fields[2],
                "Rotate marks",
                mixed_target,
            );
            sync_web_scale(
                &self.web_width_scale,
                &self.web_width_scale_status,
                first.width_scale,
                mixed_fields[3],
                "Horizontal mark scale",
                mixed_target,
            );
            sync_web_scale(
                &self.web_height_scale,
                &self.web_height_scale_status,
                first.height_scale,
                mixed_fields[4],
                "Vertical mark scale",
                mixed_target,
            );
            sync_web_scale(
                &self.web_threshold,
                &self.web_threshold_status,
                first.threshold,
                mixed_fields[5],
                "Hide light marks",
                mixed_target,
            );
            sync_web_scale(
                &self.web_opacity,
                &self.web_opacity_status,
                first.opacity,
                mixed_fields[6],
                "Transparent — Solid",
                mixed_target,
            );
            sync_web_scale(
                &self.web_detail,
                &self.web_detail_status,
                first.resolution_scale,
                mixed_fields[7],
                "Sample density",
                mixed_target,
            );
        }
        if let Some(settings) = curve_settings.as_ref() {
            self.sync_curves_schema_metadata();
            sync_layer_terminology(
                &self.curve_target,
                &self.curve_target_label,
                self.curve_target_help.as_ref(),
                &self.curve_visible_label,
                output_mode == OutputMode::RgbScreen,
                crosshatch,
            );
            if output_mode == OutputMode::RgbScreen && !crosshatch {
                self.curve_target_label.set_text("Adjust Channel");
                self.curve_visible_label.set_text("Visible RGB Channels");
            }
            self.curve_crosshatch_color_row.set_visible(crosshatch);
            self.curve_color_row.set_visible(!crosshatch);
            self.curve_crosshatch_color
                .set_text(&settings.crosshatch_color);
            self.curve_layout.set_selected(match settings.layout {
                CurveLayout::FullWidth => 0,
                CurveLayout::MotifPattern => 1,
            });
            self.motif_controls
                .set_visible(settings.layout == CurveLayout::MotifPattern);
            self.curve_shared.set_active(settings.use_shared_curve);
            let visible_spec = help_for(if output_mode == OutputMode::RgbScreen && !crosshatch {
                "Visible RGB Channels"
            } else if crosshatch {
                "Visible Crosshatch Layers"
            } else {
                "Visible Inks"
            })
            .unwrap();
            self.curve_visible_help.set_spec(visible_spec);
            for button in &self.curve_visible {
                button.set_tooltip_text(Some(visible_spec.summary));
                button.update_property(&[gtk::accessible::Property::Description(
                    visible_spec.summary,
                )]);
            }
            let shared_label = PATTERN_REGISTRY
                .parameter_for_control(PatternId::COMPATIBILITY_CURVES_V1, "curve_shared")
                .expect("Curves shared control must have registered schema metadata")
                .label;
            let shared_label = if crosshatch {
                "Share Hatch Path Across Layers".to_owned()
            } else if output_mode == OutputMode::RgbScreen {
                format!("{shared_label} Across Channels")
            } else {
                format!("{shared_label} Across Inks")
            };
            self.curve_shared.set_label(Some(&shared_label));
            if let Some(help) = self.curve_shared_help.as_ref() {
                help.set_spec(
                    help_for(if crosshatch {
                        "Share Hatch Path Across Layers"
                    } else if output_mode == OutputMode::RgbScreen {
                        "Share Line Shape Across Channels"
                    } else {
                        "Share Line Shape Across Inks"
                    })
                    .unwrap(),
                );
            }
            if crosshatch {
                set_crosshatch_target_directions(
                    &self.curve_target,
                    [
                        settings.channels.k.grid_rotation,
                        settings.channels.c.grid_rotation,
                        settings.channels.m.grid_rotation,
                        settings.channels.y.grid_rotation,
                    ],
                );
            }
            self.curve_reset.set_label(if crosshatch {
                "Reset to Straight Hatch"
            } else {
                "Reset to Soft Wave"
            });
            for (index, button) in self.curve_visible.iter().enumerate() {
                if output_mode == OutputMode::RgbScreen && !crosshatch && index == 3 {
                    button.set_visible(false);
                    continue;
                }
                button.set_visible(true);
                if output_mode == OutputMode::RgbScreen && !crosshatch {
                    let Some(ink) = Ink::RGB.get(index).copied() else {
                        continue;
                    };
                    button.set_label(Some(["Red", "Green", "Blue"][index]));
                    button.set_active(settings.channels.get(ink).enabled);
                    continue;
                }
                let Some(ink) =
                    visible_ink_for_slot(index, output_mode == OutputMode::RgbScreen, crosshatch)
                else {
                    continue;
                };
                button.set_label(Some(if crosshatch {
                    ["1 K", "2 C", "3 M", "4 Y"][index]
                } else {
                    ["C", "M", "Y", "K"][index]
                }));
                button.set_active(settings.channels.get(ink).enabled);
            }
            let Some(inks) = self.selected_curve_inks() else {
                // Do not turn GTK's transient invalid selection into the
                // all-inks target while controls are being synchronized.
                self.source_label.set_text(&source_text);
                self.state.borrow_mut().syncing_controls = false;
                self.sync_motif_overlay();
                self.update_editing_context();
                return;
            };
            let all_inks = self.curve_target.selected() == 0;
            let first = if all_inks {
                &settings.base_channel
            } else {
                settings.channels.get(inks[0])
            };
            let pattern_mixed = !all_inks
                && inks.iter().skip(1).any(|ink| {
                    let channel = settings.channels.get(*ink);
                    channel.motif_coverage != first.motif_coverage
                        || (channel.curve_scale - first.curve_scale).abs() > 1e-9
                        || channel.tile_count != first.tile_count
                        || channel.stack_count != first.stack_count
                        || (channel.stack_spacing - first.stack_spacing).abs() > 1e-9
                        || (channel.alternate_stack_offset - first.alternate_stack_offset).abs()
                            > 1e-9
                        || channel.alternate_tile_transform != first.alternate_tile_transform
                });
            let arrangement_mixed = !all_inks
                && inks.iter().skip(1).any(|ink| {
                    let channel = settings.channels.get(*ink);
                    (channel.grid_rotation - first.grid_rotation).abs() > 1e-9
                        || (channel.offset_x - first.offset_x).abs() > 1e-9
                        || (channel.offset_y - first.offset_y).abs() > 1e-9
                        || (channel.stack_spacing - first.stack_spacing).abs() > 1e-9
                });
            self.curve_editor_label
                .set_text(if crosshatch && settings.use_shared_curve {
                    "All Layers Hatch Path"
                } else if crosshatch && inks.len() == 1 {
                    match inks[0] {
                        Ink::Black => "Layer 1 Hatch Path",
                        Ink::Cyan => "Layer 2 Hatch Path",
                        Ink::Magenta => "Layer 3 Hatch Path",
                        Ink::Yellow => "Layer 4 Hatch Path",
                        Ink::Red => "Red Screen Path",
                        Ink::Green => "Green Screen Path",
                        Ink::Blue => "Blue Screen Path",
                    }
                } else if settings.use_shared_curve {
                    if settings.layout == CurveLayout::MotifPattern {
                        "All Inks Motif Shape"
                    } else {
                        "All Inks Curve"
                    }
                } else if inks.len() == 1 {
                    match inks[0] {
                        Ink::Cyan => "Cyan Curve",
                        Ink::Magenta => "Magenta Curve",
                        Ink::Yellow => "Yellow Curve",
                        Ink::Black => "Black Curve",
                        Ink::Red => "Red Screen Curve",
                        Ink::Green => "Green Screen Curve",
                        Ink::Blue => "Blue Screen Curve",
                    }
                } else if inks.iter().skip(1).any(|ink| {
                    settings.channels.get(*ink).path != settings.channels.get(inks[0]).path
                }) {
                    "Mixed Curves — Select One Ink to Edit"
                } else {
                    "Selected Ink Curves"
                });
            let differs = |value: fn(&WebCurveChannel) -> f64| {
                inks.iter()
                    .skip(1)
                    .any(|ink| (value(settings.channels.get(*ink)) - value(first)).abs() > 1e-9)
            };
            let mixed_fields = if all_inks {
                [false; 7]
            } else {
                [
                    differs(|channel| channel.scale),
                    differs(|channel| channel.grid_rotation),
                    differs(|channel| channel.offset_x),
                    differs(|channel| channel.offset_y),
                    differs(|channel| channel.opacity),
                    differs(|channel| channel.threshold),
                    differs(|channel| channel.resolution_scale),
                ]
            };
            let colors_mixed = inks
                .iter()
                .skip(1)
                .any(|ink| settings.channels.get(*ink).color != first.color);
            self.curve_color.set_sensitive(!all_inks);
            self.curve_color.set_text(if all_inks || colors_mixed {
                ""
            } else {
                &first.color
            });
            self.curve_color.set_placeholder_text(Some(if all_inks {
                "Select one ink"
            } else if colors_mixed {
                "Mixed"
            } else {
                "#RRGGBB"
            }));
            self.curve_color_status.set_text(if all_inks {
                "Select one ink"
            } else if colors_mixed {
                "Mixed"
            } else {
                "Hex color"
            });
            self.curve_weight.set_value(settings.max_mark);
            self.curve_spacing.set_value(settings.long_edge_cells);
            self.motif_coverage
                .set_selected(match first.motif_coverage {
                    MotifCoverage::Auto => 0,
                    MotifCoverage::Manual => 1,
                });
            let manual_pattern = first.motif_coverage == MotifCoverage::Manual;
            self.motif_coverage.set_sensitive(!pattern_mixed);
            self.motif_size.set_sensitive(!pattern_mixed);
            self.motif_columns
                .set_sensitive(manual_pattern && !pattern_mixed);
            self.motif_rows
                .set_sensitive(manual_pattern && !pattern_mixed);
            self.motif_row_spacing.set_sensitive(!pattern_mixed);
            self.motif_stagger.set_sensitive(!pattern_mixed);
            self.motif_alternate.set_sensitive(!pattern_mixed);
            self.motif_arrange
                .set_sensitive(!pattern_mixed && !arrangement_mixed);
            if pattern_mixed || arrangement_mixed {
                self.motif_arrange.set_active(false);
            }
            self.motif_mixed.set_text(if pattern_mixed {
                "Mixed pattern values — select one ink to edit its motif arrangement."
            } else if arrangement_mixed {
                "Ink angles or positions differ — select one ink to arrange on the canvas."
            } else {
                ""
            });
            self.motif_size.set_value(first.curve_scale);
            self.motif_columns.set_value(first.tile_count as f64);
            self.motif_rows.set_value(first.stack_count as f64);
            self.motif_row_spacing.set_value(first.stack_spacing.abs());
            self.motif_stagger.set_value(first.alternate_stack_offset);
            self.motif_alternate
                .set_selected(match first.alternate_tile_transform {
                    AlternateTileTransform::None => 0,
                    AlternateTileTransform::Flip => 1,
                    AlternateTileTransform::Rotate180 => 2,
                });
            sync_web_scale(
                &self.curve_coverage,
                &self.curve_coverage_status,
                first.scale,
                mixed_fields[0],
                "Curve scale",
                "inks",
            );
            sync_web_scale(
                &self.curve_angle,
                &self.curve_angle_status,
                first.grid_rotation,
                mixed_fields[1],
                "Rotate ink screen",
                "inks",
            );
            sync_web_scale(
                &self.curve_position_x,
                &self.curve_position_x_status,
                first.offset_x,
                mixed_fields[2],
                "Move across",
                "inks",
            );
            sync_web_scale(
                &self.curve_position_y,
                &self.curve_position_y_status,
                first.offset_y,
                mixed_fields[3],
                "Move vertically",
                "inks",
            );
            sync_web_scale(
                &self.curve_opacity,
                &self.curve_opacity_status,
                first.opacity,
                mixed_fields[4],
                "Transparent — Solid",
                "inks",
            );
            sync_web_scale(
                &self.curve_threshold,
                &self.curve_threshold_status,
                first.threshold,
                mixed_fields[5],
                "Hide light marks",
                "inks",
            );
            sync_web_scale(
                &self.curve_detail,
                &self.curve_detail_status,
                first.resolution_scale,
                mixed_fields[6],
                "Sample density",
                "inks",
            );
            let active_path = if settings.use_shared_curve {
                &settings.shared_path
            } else {
                &first.path
            };
            let paths_mixed = !settings.use_shared_curve
                && inks
                    .iter()
                    .skip(1)
                    .any(|ink| settings.channels.get(*ink).path != *active_path);
            self.curve_profile.set_selected(if paths_mixed {
                4
            } else if *active_path == CurvePath::straight() {
                0
            } else if *active_path == CurvePath::soft_wave() {
                1
            } else if *active_path == CurvePath::deep_wave() {
                2
            } else {
                3
            });
            let close_first = if settings.use_shared_curve {
                settings.shared_close_ends
            } else {
                first.close_ends
            };
            let smooth_first = if settings.use_shared_curve {
                settings.shared_smooth_join
            } else {
                first.smooth_join
            };
            let close_mixed = !settings.use_shared_curve
                && inks
                    .iter()
                    .skip(1)
                    .any(|ink| settings.channels.get(*ink).close_ends != close_first);
            let smooth_mixed = !settings.use_shared_curve
                && inks
                    .iter()
                    .skip(1)
                    .any(|ink| settings.channels.get(*ink).smooth_join != smooth_first);
            self.curve_close_ends.set_inconsistent(close_mixed);
            self.curve_close_ends.set_active(close_first);
            self.curve_smooth_join.set_inconsistent(smooth_mixed);
            self.curve_smooth_join.set_active(smooth_first);
            self.curve_smooth_join
                .set_sensitive(close_first && !close_mixed);
            self.curve_mixed.set_text(
                if mixed_fields.into_iter().any(|mixed| mixed)
                    || colors_mixed
                    || close_mixed
                    || smooth_mixed
                    || paths_mixed
                {
                    "Changing a Mixed control applies one value to every selected ink."
                } else {
                    ""
                },
            );
            self.curve_editor.queue_draw();
        }
        self.sync_channel_scope_panels(&pipeline, crosshatch);
        self.source_label.set_text(&source_text);
        self.state.borrow_mut().syncing_controls = false;
        self.sync_motif_overlay();
        self.update_editing_context();
    }

    fn sync_channel_scope_panels(&self, pipeline: &ArtworkPipelineSettings, crosshatch: bool) {
        let output_model = pipeline.output_model;
        sync_dropdown_strings(
            &self.channel_scope,
            &channel_scope_labels(output_model, crosshatch),
        );
        self.channel_scope.set_selected(channel_scope_index(
            self.web_target.selected(),
            output_model,
            crosshatch,
        ));
        self.channel_scope.set_sensitive(!crosshatch);

        for controls in &self.channel_controls {
            controls
                .heading
                .set_text(&channel_heading(controls.channel));
            controls.inclusion_status.set_text(if controls.channel.belongs_to(output_model) {
                "Included in the current output model and available as a treatment editing scope."
            } else {
                "Retained for this document and unavailable in the current output model."
            });
        }

        let (aggregate_heading, mixed_message, panel_name) = if crosshatch {
            (
                "All Layers",
                "Legacy Crosshatch edits its explicit layers together. Mixed treatment values remain mixed.",
                "aggregate".to_owned(),
            )
        } else if self.channel_scope.selected() == 0 {
            (
                if output_model == OutputModel::CmykPrint {
                    "All Inks"
                } else {
                    "All Channels"
                },
                "Treatment edits apply to every included output channel. Mixed values are shown without coercion.",
                "aggregate".to_owned(),
            )
        } else if let Some(channel) =
            channel_scope_channel(self.channel_scope.selected(), output_model).flatten()
        {
            ("", "", channel.stable_id().to_owned())
        } else {
            (
                "All Channels",
                "Choose a treatment editing scope before editing channel-specific settings.",
                "aggregate".to_owned(),
            )
        };
        self.aggregate_channel_controls
            .heading
            .set_text(aggregate_heading);
        self.aggregate_channel_controls
            .mixed_message
            .set_text(mixed_message);
        self.channel_panel_stack.set_visible_child_name(&panel_name);
    }

    fn web_uses_channel_copy(&self) -> bool {
        self.state.borrow().editor.as_ref().is_some_and(|editor| {
            editor.document().artwork_pipeline.output_model == OutputModel::RgbScreen
                && !document_uses_crosshatch(editor.document())
        })
    }

    fn sync_web_color_terminology(&self, channel_copy: bool) {
        let copy = web_color_copy(channel_copy);
        self.web_color_heading.set_text(copy.color_heading);
        self.web_color_help
            .set_spec(help_for(copy.color_heading).unwrap());
        self.web_color.set_tooltip_text(Some(copy.color_tooltip));
        self.web_color
            .update_property(&[gtk::accessible::Property::Description(copy.color_tooltip)]);
        self.web_opacity_heading.set_text(copy.opacity_heading);
        self.web_opacity_help
            .set_spec(help_for(copy.opacity_heading).unwrap());
        self.web_opacity
            .set_tooltip_text(Some(copy.opacity_tooltip));
        self.web_opacity
            .update_property(&[gtk::accessible::Property::Description(copy.opacity_tooltip)]);
    }

    fn select_preview_view(self: &Rc<Self>) {
        let (view, target, cache) = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                return;
            };
            let document = editor.document();
            let view = if state.compare_source {
                PreviewView::Source
            } else {
                PreviewView::Rendered
            };
            let target = preview_target_for_zoom(document_artboard_size(document), state.zoom_mode);
            let cache = match view {
                PreviewView::Source => state
                    .source_cache
                    .as_ref()
                    .filter(|cache| preview_cache_matches(cache, document, view)),
                PreviewView::Rendered => state
                    .rendered_cache
                    .as_ref()
                    .filter(|cache| preview_cache_matches(cache, document, view)),
            }
            .cloned();
            (view, target, cache)
        };
        let sufficient = cache
            .as_ref()
            .is_some_and(|cache| preview_cache_is_sufficient(cache, target));
        if let Some(cache) = cache {
            self.install_preview(cache.image, self.gate.current(), view);
            self.preview_indicator.selected(view);
        }
        if !sufficient {
            self.request_preview_at(target);
        }
    }

    fn request_preview(&self) {
        let request = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                return;
            };
            let generation = self.gate.next();
            build_render_request(
                generation,
                editor.document(),
                state.compare_source,
                state.zoom_mode,
            )
        };
        self.queue_preview_request(request);
    }

    fn request_rendered_preview(&self) {
        let request = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                return;
            };
            build_render_request(self.gate.next(), editor.document(), false, state.zoom_mode)
        };
        self.queue_preview_request(request);
    }

    fn request_preview_at(&self, max_dimension: u32) {
        let (document, compare_source) = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                return;
            };
            (editor.document().clone(), state.compare_source)
        };
        let generation = self.gate.next();
        self.queue_preview_request(RenderRequest {
            generation,
            document,
            compare_source,
            max_dimension,
            token: CancellationToken::new(),
        });
    }

    fn queue_preview_request(&self, request: RenderRequest) {
        if let Some(token) = self
            .preview_token
            .borrow_mut()
            .replace(request.token.clone())
        {
            token.cancel();
        }
        let generation = request.generation;
        let requested_view = if request.compare_source {
            PreviewView::Source
        } else {
            PreviewView::Rendered
        };
        if self.state.borrow().compare_source && requested_view == PreviewView::Rendered {
            self.preview_indicator.selected(PreviewView::Source);
        } else {
            self.preview_indicator.request(generation, requested_view);
        }
        self.render_requests.replace(request);
        self.cancel_preview.set_visible(true);
    }

    fn cancel_preview(&self) {
        if self
            .preview_token
            .borrow()
            .as_ref()
            .is_some_and(CancellationToken::cancel)
        {
            let generation = self.gate.current();
            self.render_requests.take();
            self.preview_indicator.cancelled(generation);
            self.gate.next();
            self.preview_token.take();
            self.cancel_preview.set_visible(false);
            self.set_workspace_status("Preview cancelled");
        }
    }

    fn schedule_zoom_refinement(self: &Rc<Self>) {
        let token = self.zoom_settle_generation.get().wrapping_add(1);
        self.zoom_settle_generation.set(token);
        glib::timeout_add_local_once(
            Duration::from_millis(180),
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move || {
                    if ui.zoom_settle_generation.get() != token {
                        return;
                    }
                    let (target, sufficient) = {
                        let state = ui.state.borrow();
                        let Some(editor) = state.editor.as_ref() else {
                            return;
                        };
                        let ZoomMode::Explicit(percent) = state.zoom_mode else {
                            return;
                        };
                        let zoom = percent / 100.0;
                        let (width, height) = document_artboard_size(editor.document());
                        let target = preview_target_dimension(width, height, zoom);
                        let sufficient =
                            state.preview_size.is_some_and(|(w, h)| w.max(h) >= target);
                        (target, sufficient)
                    };
                    if !sufficient {
                        ui.request_preview_at(target);
                    }
                }
            ),
        );
    }

    fn poll_render_results(self: &Rc<Self>) {
        let Some(outcome) = self.render_results.take() else {
            return;
        };
        match outcome.result {
            Ok(image) => {
                let desired_view = if self.state.borrow().compare_source {
                    PreviewView::Source
                } else {
                    PreviewView::Rendered
                };
                let current_document = self
                    .state
                    .borrow()
                    .editor
                    .as_ref()
                    .map(|editor| editor.document().clone());
                if current_document
                    .as_ref()
                    .is_some_and(|document| document.document_id == outcome.document.document_id)
                {
                    let mut state = self.state.borrow_mut();
                    let slot = match outcome.view {
                        PreviewView::Source => &mut state.source_cache,
                        PreviewView::Rendered => &mut state.rendered_cache,
                    };
                    let replace = slot.as_ref().is_none_or(|cache| {
                        let old = cache.image.width().max(cache.image.height());
                        let new = image.width().max(image.height());
                        cache.document != outcome.document || new >= old
                    });
                    if replace {
                        *slot = Some(PreviewCache {
                            document: outcome.document.clone(),
                            image: image.clone(),
                        });
                    }
                }
                if !self.gate.accepts(outcome.generation) || desired_view != outcome.view {
                    return;
                }
                self.cancel_preview.set_visible(false);
                self.preview_generation.set(outcome.generation);
                self.install_preview(image, outcome.generation, outcome.view)
            }
            Err(error) => {
                if !self.gate.accepts(outcome.generation) {
                    return;
                }
                if error.downcast_ref::<OperationCancelled>().is_some() {
                    self.set_workspace_status("Preview cancelled");
                    self.cancel_preview.set_visible(false);
                    return;
                }
                self.preview_indicator.failed(outcome.generation);
                self.set_workspace_status("Preview error — adjust the document or try again");
                self.show_error(&format!("Could not render preview: {error:#}"));
                if self.screenshot_path.is_some()
                    || self.export_path.is_some()
                    || self.png_export_path.is_some()
                    || self.save_artifact_path.is_some()
                    || self.save_treatment_path.is_some()
                {
                    self.write_cli_artifacts();
                }
            }
        }
    }

    fn install_preview(self: &Rc<Self>, image: RgbaImage, generation: u64, view: PreviewView) {
        let (width, height) = image.dimensions();
        let stride = width as usize * 4;
        let bytes = glib::Bytes::from_owned(image.into_raw());
        let texture = gdk::MemoryTexture::new(
            width as i32,
            height as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &bytes,
            stride,
        );
        self.picture.set_paintable(Some(&texture));
        self.state.borrow_mut().preview_size = Some((width, height));
        self.preview_indicator.installed(generation, view);
        self.set_workspace_status(match view {
            PreviewView::Source => "Showing source artwork",
            PreviewView::Rendered => "Preview ready",
        });
        self.apply_zoom_mode();
        if let Some((width, height)) = self.artifact_resize_window
            && !self.artifact_resize_started.replace(true)
        {
            let width = width.max(720);
            let height = height.max(520);
            glib::timeout_add_local_once(
                Duration::from_millis(100),
                glib::clone!(
                    #[weak(rename_to = ui)]
                    self,
                    move || {
                        ui.artifact_resize_before.set(ui.artifact_allocation());
                        ui.window.set_default_size(width, height);
                        ui.window.set_size_request(width, height);
                        glib::timeout_add_local_once(
                            Duration::from_millis(100),
                            glib::clone!(
                                #[weak]
                                ui,
                                move || {
                                    ui.fit_allocation.borrow_mut().reset();
                                    ui.apply_fit_zoom();
                                }
                            ),
                        );
                    }
                ),
            );
        }

        if self.screenshot_path.is_some() || self.artifact_resize_window.is_some() {
            glib::timeout_add_local_once(
                Duration::from_millis(1_600),
                glib::clone!(
                    #[weak(rename_to = ui)]
                    self,
                    move || ui.write_cli_artifacts()
                ),
            );
        } else {
            self.write_cli_artifacts();
        }
    }

    fn artifact_allocation(&self) -> Option<ArtifactAllocation> {
        let state = self.state.borrow();
        let editor = state.editor.as_ref()?;
        let artboard = document_artboard_size(editor.document());
        let viewport = self.canvas_viewport_size();
        Some(ArtifactAllocation {
            inspector_width: self.inspector_pane.current_width(),
            viewport,
            fit_edge_deltas: fit_edge_deltas(artboard, viewport, self.window.scale_factor()),
            canvas_metrics: self.canvas_allocation_metrics(viewport),
            preview_size: state.preview_size.unwrap_or((0, 0)),
        })
    }

    fn cli_preview_readiness(&self) -> ArtifactPreviewReadiness {
        let state = self.state.borrow();
        let Some(editor) = state.editor.as_ref() else {
            return if self.deferred_candidate_artifact {
                ArtifactPreviewReadiness::Waiting
            } else {
                ArtifactPreviewReadiness::Ready
            };
        };
        let document = editor.document();
        let desired_view = if state.compare_source {
            PreviewView::Source
        } else {
            PreviewView::Rendered
        };
        let target = preview_target_for_zoom(document_artboard_size(document), state.zoom_mode);
        let cache = match desired_view {
            PreviewView::Source => state.source_cache.as_ref(),
            PreviewView::Rendered => state.rendered_cache.as_ref(),
        };
        let cache_ready = cache.is_some_and(|cache| {
            preview_cache_matches(cache, document, desired_view)
                && preview_cache_is_sufficient(cache, target)
        });
        artifact_preview_readiness(
            &self.preview_indicator.activity.borrow(),
            desired_view,
            cache_ready,
            state
                .preview_size
                .is_some_and(|(width, height)| width > 0 && height > 0)
                && self.picture.paintable().is_some(),
        )
    }

    fn canvas_allocation_metrics(&self, viewport: (i32, i32)) -> CanvasAllocationMetrics {
        let artwork = (self.canvas_content.width(), self.canvas_content.height());
        let Some(bounds) = self.canvas_content.compute_bounds(&self.canvas) else {
            return CanvasAllocationMetrics::centered(viewport, artwork);
        };
        let origin = (bounds.x().round() as i32, bounds.y().round() as i32);
        CanvasAllocationMetrics {
            origin,
            slack: (
                origin.0,
                viewport.0 - origin.0 - artwork.0,
                origin.1,
                viewport.1 - origin.1 - artwork.1,
            ),
        }
    }

    fn write_cli_artifacts(self: &Rc<Self>) {
        if self.preset_pending.get() {
            return;
        }
        match self.cli_preview_readiness() {
            ArtifactPreviewReadiness::Ready => {}
            ArtifactPreviewReadiness::Waiting => return,
            ArtifactPreviewReadiness::Failed => {
                if !self.cli_artifacts_written.replace(true) {
                    self.report_cli_artifact_error(
                        "Could not capture requested artifacts: the newest preview render failed"
                            .to_owned(),
                    );
                }
                if !self.recovery_enabled {
                    self.close_approved.set(true);
                    self.window.close();
                }
                return;
            }
        }
        if self.artifact_resize_window.is_some() {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                return;
            };
            let ZoomMode::Fit(_) = state.zoom_mode else {
                return;
            };
            let artboard = document_artboard_size(editor.document());
            let viewport = self.canvas_viewport_size();
            let expected = fitted_artwork_size(artboard, viewport, self.window.scale_factor());
            let allocated = (self.canvas_content.width(), self.canvas_content.height());
            let preview_ready =
                fit_refinement_target(artboard, state.zoom_mode, state.preview_size).is_none();
            if !preview_ready
                || (allocated.0 - expected.0).abs() > 1
                || (allocated.1 - expected.1).abs() > 1
            {
                return;
            }
        }
        if self.allocation_report_path.is_some() {
            let state = self.state.borrow();
            if matches!(state.zoom_mode, ZoomMode::Fit(_)) {
                let viewport = self.canvas_viewport_size();
                let metrics = self.canvas_allocation_metrics(viewport);
                if metrics.horizontal_delta() > 1 || metrics.vertical_delta() > 1 {
                    return;
                }
            }
        }
        if self.screenshot_path.is_some() && !self.capture_prepared.replace(true) {
            let capture_window = self
                .capture_override
                .borrow()
                .clone()
                .unwrap_or_else(|| self.window.clone().upcast());
            capture_window.queue_draw();
            let frames = Rc::new(Cell::new(0u8));
            capture_window.add_tick_callback(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                #[strong]
                frames,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move |_, _| {
                    let next = frames.get() + 1;
                    frames.set(next);
                    if next < 2 {
                        glib::ControlFlow::Continue
                    } else {
                        ui.write_cli_artifacts();
                        glib::ControlFlow::Break
                    }
                }
            ));
            glib::timeout_add_local_once(
                Duration::from_millis(300),
                glib::clone!(
                    #[weak(rename_to = ui)]
                    self,
                    move || ui.write_cli_artifacts()
                ),
            );
            return;
        }
        if self.cli_artifacts_written.get() {
            return;
        }
        if let Some(path) = self.screenshot_path.as_ref() {
            match self.capture_window(path) {
                Ok(true) => {}
                Ok(false) => {
                    let attempts = self.capture_attempts.get() + 1;
                    self.capture_attempts.set(attempts);
                    if attempts >= 50 {
                        self.finish_cli_artifact_failure("Could not write window screenshot: GTK did not produce a render node within 5 seconds".into());
                        return;
                    }
                    self.window.queue_draw();
                    glib::timeout_add_local_once(
                        Duration::from_millis(100),
                        glib::clone!(
                            #[weak(rename_to = ui)]
                            self,
                            move || ui.write_cli_artifacts()
                        ),
                    );
                    return;
                }
                Err(error) => {
                    self.finish_cli_artifact_failure(format!(
                        "Could not write window screenshot {}: {error:#}",
                        path.display()
                    ));
                    return;
                }
            }
        }
        self.cli_artifacts_written.set(true);
        if let Some(path) = self.allocation_report_path.as_ref() {
            let state = self.state.borrow();
            let mode = match state.zoom_mode {
                ZoomMode::Fit(percent) => format!("fit:{percent:0.3}%"),
                ZoomMode::Explicit(percent) => format!("{percent:0.3}%"),
            };
            let viewport = self.canvas_viewport_size();
            let artboard = state
                .editor
                .as_ref()
                .map(|editor| document_artboard_size(editor.document()))
                .unwrap_or((0, 0));
            let edge_deltas = if artboard.0 > 0 && artboard.1 > 0 {
                fit_edge_deltas(artboard, viewport, self.window.scale_factor())
            } else {
                (0, 0)
            };
            let metrics = self.canvas_allocation_metrics(viewport);
            let report = format!(
                "zoom={mode}\npaned_position={}\ninspector_width={}\ninspector_desired_width={}\nartwork_width={}\nartwork_height={}\ncontent_width={}\ncontent_height={}\nviewport_width={}\nviewport_height={}\nartwork_origin_x={}\nartwork_origin_y={}\nslack_left={}\nslack_right={}\nslack_top={}\nslack_bottom={}\nslack_delta_x={}\nslack_delta_y={}\nfit_edge_delta_x={}\nfit_edge_delta_y={}\npreview_width={}\npreview_height={}\n",
                self.inspector_pane.current_width(),
                self.inspector_pane.current_width(),
                self.inspector_pane.desired_width.get(),
                self.picture.width(),
                self.picture.height(),
                self.canvas_content.width(),
                self.canvas_content.height(),
                viewport.0,
                viewport.1,
                metrics.origin.0,
                metrics.origin.1,
                metrics.slack.0,
                metrics.slack.1,
                metrics.slack.2,
                metrics.slack.3,
                metrics.horizontal_delta(),
                metrics.vertical_delta(),
                edge_deltas.0,
                edge_deltas.1,
                state.preview_size.map_or(0, |size| size.0),
                state.preview_size.map_or(0, |size| size.1),
            );
            let report = if let Some(before) = self.artifact_resize_before.get() {
                format!(
                    "before_inspector_width={}\nbefore_viewport_width={}\nbefore_viewport_height={}\nbefore_artwork_origin_x={}\nbefore_artwork_origin_y={}\nbefore_slack_left={}\nbefore_slack_right={}\nbefore_slack_top={}\nbefore_slack_bottom={}\nbefore_slack_delta_x={}\nbefore_slack_delta_y={}\nbefore_fit_edge_delta_x={}\nbefore_fit_edge_delta_y={}\nbefore_preview_width={}\nbefore_preview_height={}\n{report}",
                    before.inspector_width,
                    before.viewport.0,
                    before.viewport.1,
                    before.canvas_metrics.origin.0,
                    before.canvas_metrics.origin.1,
                    before.canvas_metrics.slack.0,
                    before.canvas_metrics.slack.1,
                    before.canvas_metrics.slack.2,
                    before.canvas_metrics.slack.3,
                    before.canvas_metrics.horizontal_delta(),
                    before.canvas_metrics.vertical_delta(),
                    before.fit_edge_deltas.0,
                    before.fit_edge_deltas.1,
                    before.preview_size.0,
                    before.preview_size.1,
                )
            } else {
                report
            };
            if let Err(error) = std::fs::write(path, report) {
                self.report_cli_artifact_error(format!(
                    "Could not write allocation report {}: {error}",
                    path.display()
                ));
            }
        }
        if let Some(path) = self.indicator_report_path.as_ref() {
            let activity = self.preview_indicator.activity.borrow();
            let phase = self.preview_indicator.phase();
            let report = format!(
                "generation={}\nrequested_view={}\ninstalled_view={}\nbusy={}\nlabel={}\ntooltip={}\naccessible_role={:?}\nphase={phase:0.6}\nsolid_layer={:0.6}\ndot_layer={phase:0.6}\nwidth={}\nheight={}\ngeometry_source=embedded-svg:assets/preview-indicator.svg#solid-t,#halftone-dots\n",
                activity.requested.map_or(0, |(generation, _)| generation),
                activity.requested.map_or("none", |(_, view)| match view {
                    PreviewView::Source => "source",
                    PreviewView::Rendered => "rendered",
                }),
                activity.installed.map_or("none", |(_, view)| match view {
                    PreviewView::Source => "source",
                    PreviewView::Rendered => "rendered",
                }),
                self.preview_indicator.effective_busy(),
                self.preview_indicator.effective_label(),
                self.preview_indicator
                    .area
                    .tooltip_text()
                    .unwrap_or_default(),
                self.preview_indicator.area.accessible_role(),
                preview_indicator_layers(phase).0,
                self.preview_indicator.area.width(),
                self.preview_indicator.area.height(),
            );
            if let Err(error) = std::fs::write(path, report) {
                self.report_cli_artifact_error(format!(
                    "Could not write indicator report {}: {error}",
                    path.display()
                ));
            }
        }
        if let Some(path) = self.export_path.as_ref()
            && let Some(document) = self
                .state
                .borrow()
                .editor
                .as_ref()
                .map(|editor| editor.document().clone())
        {
            match export_svg(path, &document) {
                Ok(()) => self.show_message(&format!("Exported {}", path.display())),
                Err(error) => self.report_cli_artifact_error(format!(
                    "Could not export SVG {}: {error:#}",
                    path.display()
                )),
            }
        }
        if let Some(path) = self.png_export_path.as_ref()
            && let Some(document) = self
                .state
                .borrow()
                .editor
                .as_ref()
                .map(|editor| editor.document().clone())
        {
            match toniator::PngExportOptions::document_size(&document)
                .and_then(|options| toniator::export_png(path, &document, options))
            {
                Ok(()) => self.show_message(&format!("Exported PNG {}", path.display())),
                Err(error) => self.report_cli_artifact_error(format!(
                    "Could not export PNG {}: {error:#}",
                    path.display()
                )),
            }
        }
        if let Some(path) = self.save_artifact_path.as_ref()
            && let Some(document) = self
                .state
                .borrow()
                .editor
                .as_ref()
                .map(|editor| editor.document().clone())
            && let Err(error) = save_document_atomic(path, &document)
        {
            self.report_cli_artifact_error(format!(
                "Could not save artifact document {}: {error:#}",
                path.display()
            ));
        }
        if let Some(path) = self.save_treatment_path.as_ref()
            && let Some(document) = self
                .state
                .borrow()
                .editor
                .as_ref()
                .map(|editor| editor.document().clone())
        {
            match toniator::preset::document_treatment_preset_bytes("Artifact Treatment", &document)
                .and_then(|bytes| toniator::persistence::atomic_write(path, &bytes))
            {
                Ok(()) => self.show_message(&format!("Saved treatment {}", path.display())),
                Err(error) => self.report_cli_artifact_error(format!(
                    "Could not save treatment {}: {error:#}",
                    path.display()
                )),
            }
        }
        if !self.recovery_enabled {
            self.close_approved.set(true);
            self.window.close();
        }
    }

    fn capture_window(&self, path: &Path) -> anyhow::Result<bool> {
        let window: gtk::Window = self
            .capture_override
            .borrow()
            .clone()
            .unwrap_or_else(|| self.window.clone().upcast());
        let width = window.width().max(1) as u32;
        let height = window.height().max(1) as u32;
        let paintable = gtk::WidgetPaintable::new(Some(&window));
        paintable.invalidate_contents();
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, width as f64, height as f64);
        let Some(content_node) = snapshot.to_node() else {
            return Ok(false);
        };
        let node = opaque_capture_node(&content_node, width, height, capture_window_background());
        let surface = window
            .surface()
            .ok_or_else(|| anyhow::anyhow!("window has no surface"))?;
        let renderer = gtk::gsk::Renderer::for_surface(&surface)
            .ok_or_else(|| anyhow::anyhow!("could not create GTK renderer"))?;
        let viewport = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
        let texture = renderer.render_texture(&node, Some(&viewport));
        texture.save_to_png(path)?;
        Ok(true)
    }

    fn save_document(self: &Rc<Self>) {
        self.save_then(|_| {});
    }

    fn save_then(self: &Rc<Self>, continuation: impl Fn(&Rc<Self>) + 'static) {
        if let Some(path) = self.state.borrow().path.clone() {
            if DirtyTransitionCoordinator::save_finished(self.save_to_path(&path))
                == DirtyTransitionAction::Continue
            {
                continuation(self);
            }
            return;
        }
        let continuation = Rc::new(continuation);
        let dialog = gtk::FileDialog::builder()
            .title("Save Toniator Document")
            .initial_name("Untitled.toniator")
            .modal(true)
            .build();
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        let documents = gtk::FileFilter::new();
        documents.set_name(Some("Toniator Document (.toniator)"));
        documents.add_pattern("*.toniator");
        filters.append(&documents);
        dialog.set_filters(Some(&filters));
        dialog.set_default_filter(Some(&documents));
        dialog.save(
            Some(&self.window),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |result| {
                    if let Ok(file) = result
                        && let Some(path) = file.path()
                    {
                        let outcome = ui.save_to_path(&ensure_extension(path, "toniator"));
                        if DirtyTransitionCoordinator::save_finished(outcome)
                            == DirtyTransitionAction::Continue
                        {
                            continuation(&ui);
                        }
                    }
                }
            ),
        );
    }

    fn save_to_path(&self, path: &Path) -> SaveTransitionOutcome {
        let document = self
            .state
            .borrow()
            .editor
            .as_ref()
            .map(|editor| editor.document().clone());
        let Some(document) = document else {
            return SaveTransitionOutcome::WriteFailed;
        };
        match save_document_atomic(path, &document) {
            Ok(()) => {
                if self.recovery_enabled {
                    self.autosave_generation.fetch_add(1, Ordering::SeqCst);
                    self.autosave_requests.take();
                    let _guard = self
                        .recovery_io_lock
                        .lock()
                        .expect("recovery I/O lock poisoned");
                    if let Err(error) =
                        clear_recovery_if_matches(&recovery_path(), &document.document_id)
                    {
                        self.show_error(&format!(
                            "Document saved, but recovery cleanup failed: {error:#}"
                        ));
                        self.update_actions();
                        return SaveTransitionOutcome::RecoveryCleanupFailed;
                    }
                }
                let mut state = self.state.borrow_mut();
                state.path = Some(path.to_owned());
                if let Some(editor) = state.editor.as_mut() {
                    editor.mark_clean();
                }
                drop(state);
                self.update_actions();
                self.show_message(&format!("Saved {}", path.display()));
                SaveTransitionOutcome::Saved
            }
            Err(error) => {
                self.show_error(&format!("Could not save document: {error:#}"));
                SaveTransitionOutcome::WriteFailed
            }
        }
    }

    fn has_dirty_document(&self) -> bool {
        let state = self.state.borrow();
        state
            .editor
            .as_ref()
            .is_some_and(|editor| editor.is_dirty() || state.path.is_none())
    }

    fn gate_dirty_transition(self: &Rc<Self>, continuation: impl Fn(&Rc<Self>) + 'static) {
        if DirtyTransitionCoordinator::begin(self.has_dirty_document())
            == DirtyTransitionAction::Continue
        {
            continuation(self);
            return;
        }
        let continuation: TransitionContinuation = Rc::new(continuation);
        let dialog = adw::AlertDialog::builder()
            .heading("Save changes before continuing?")
            .body("Saving preserves this Toniator document. Discard is the only way to continue without saving.")
            .build();
        dialog.add_responses(&[
            ("cancel", "Cancel"),
            ("discard", "Discard"),
            ("save", "Save"),
        ]);
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("save"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.choose(
            Some(&self.window),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |response| match DirtyTransitionCoordinator::choose(match response.as_str() {
                    "save" => DirtyTransitionChoice::Save,
                    "discard" => DirtyTransitionChoice::Discard,
                    _ => DirtyTransitionChoice::Cancel,
                },)
                {
                    DirtyTransitionAction::Save => {
                        let continuation = Rc::clone(&continuation);
                        ui.save_then(move |ui| continuation(ui));
                    }
                    DirtyTransitionAction::ClearRecovery => {
                        match ui.clear_current_recovery() {
                            Ok(())
                                if DirtyTransitionCoordinator::cleanup_finished(true)
                                    == DirtyTransitionAction::Continue =>
                            {
                                continuation(&ui)
                            }
                            Ok(()) => {}
                            Err(error) => {
                                debug_assert_eq!(
                                    DirtyTransitionCoordinator::cleanup_finished(false),
                                    DirtyTransitionAction::Stay
                                );
                                ui.show_error(&format!(
                                    "Could not safely discard recovery: {error:#}"
                                ));
                            }
                        }
                    }
                    DirtyTransitionAction::Stay => {}
                    DirtyTransitionAction::Prompt | DirtyTransitionAction::Continue => {
                        unreachable!("dialog response cannot produce this transition action")
                    }
                }
            ),
        );
    }

    fn clear_current_recovery(&self) -> anyhow::Result<()> {
        if !self.recovery_enabled {
            return Ok(());
        }
        let Some(document_id) = self
            .state
            .borrow()
            .editor
            .as_ref()
            .map(|editor| editor.document().document_id.clone())
        else {
            return Ok(());
        };
        self.invalidate_and_clear_recovery(&document_id)?;
        Ok(())
    }

    fn invalidate_and_clear_recovery(&self, document_id: &str) -> anyhow::Result<()> {
        if !self.recovery_enabled {
            return Ok(());
        }
        self.autosave_generation.fetch_add(1, Ordering::SeqCst);
        self.autosave_requests.take();
        let _guard = self
            .recovery_io_lock
            .lock()
            .expect("recovery I/O lock poisoned");
        clear_recovery_if_matches(&recovery_path(), document_id)?;
        Ok(())
    }

    fn queue_autosave(&self, document: Document) {
        if self.recovery_enabled {
            self.autosave_status.set_text("Recovery save pending…");
            let generation = self.autosave_generation.fetch_add(1, Ordering::SeqCst) + 1;
            self.autosave_requests.replace(AutosaveRequest {
                generation,
                document,
            });
        }
    }

    fn flush_recovery_sync(&self) -> bool {
        if !self.recovery_enabled || !self.has_dirty_document() {
            return true;
        }
        let Some(document) = self
            .state
            .borrow()
            .editor
            .as_ref()
            .map(|editor| editor.document().clone())
        else {
            return true;
        };
        self.autosave_generation.fetch_add(1, Ordering::SeqCst);
        self.autosave_requests.take();
        let _guard = self
            .recovery_io_lock
            .lock()
            .expect("recovery I/O lock poisoned");
        match save_document_atomic(&recovery_path(), &document) {
            Ok(()) => {
                self.autosave_status.set_text("Recovery saved");
                true
            }
            Err(error) => {
                self.autosave_status.set_text("Recovery save failed");
                self.show_error(&format!("Could not write recovery snapshot: {error:#}"));
                false
            }
        }
    }

    fn poll_autosave_results(&self) {
        let Some(outcome) = self.autosave_results.take() else {
            return;
        };
        let is_current = self
            .state
            .borrow()
            .editor
            .as_ref()
            .is_some_and(|editor| editor.document().document_id == outcome.document_id);
        if !is_current {
            return;
        }
        match outcome.result {
            Ok(()) => self.autosave_status.set_text("Recovery saved"),
            Err(error) => {
                self.autosave_status.set_text("Recovery save failed");
                self.show_error(&format!("Autosave failed: {error:#}"));
            }
        }
    }

    fn export_document(self: &Rc<Self>) {
        if self.export_running.replace(true) {
            return;
        }
        self.export.set_sensitive(false);
        self.set_workspace_status("Choose an export format");
        self.update_actions();
        let dialog = adw::AlertDialog::builder()
            .heading("Export")
            .body("Choose editable vector artwork or a flattened image for sharing and printing.")
            .build();
        dialog.add_responses(&[
            ("cancel", "Cancel"),
            ("png", "PNG Image"),
            ("svg", "Editable SVG"),
        ]);
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("svg"));
        dialog.set_response_appearance("svg", adw::ResponseAppearance::Suggested);
        dialog.choose(
            Some(&self.window),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |response| match response.as_str() {
                    "svg" => ui.export_svg_dialog(),
                    "png" => ui.configure_png_export(),
                    _ => {
                        ui.export_running.set(false);
                        ui.update_actions();
                    }
                }
            ),
        );
    }

    fn export_svg_dialog(self: &Rc<Self>) {
        let source_name = self
            .state
            .borrow()
            .editor
            .as_ref()
            .map(|editor| {
                Path::new(&editor.document().source.name)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_else(|| "Toniator Export".into());
        let dialog = gtk::FileDialog::builder()
            .title("Export Editable SVG")
            .initial_name(format!("{source_name} — Halftone.svg"))
            .modal(true)
            .build();
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        let svg = gtk::FileFilter::new();
        svg.set_name(Some("Editable SVG (.svg)"));
        svg.add_pattern("*.svg");
        filters.append(&svg);
        dialog.set_filters(Some(&filters));
        dialog.set_default_filter(Some(&svg));
        dialog.save(
            Some(&self.window),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |result| {
                    if let Ok(file) = result
                        && let Some(path) = file.path()
                    {
                        let path = ensure_extension(path, "svg");
                        let document = ui
                            .state
                            .borrow()
                            .editor
                            .as_ref()
                            .map(|editor| editor.document().clone());
                        if let Some(document) = document {
                            ui.start_export(path, document);
                            return;
                        }
                    }
                    ui.export_running.set(false);
                    ui.update_actions();
                }
            ),
        );
    }

    fn configure_png_export(self: &Rc<Self>) {
        let Some(document) = self
            .state
            .borrow()
            .editor
            .as_ref()
            .map(|editor| editor.document().clone())
        else {
            self.export_running.set(false);
            self.update_actions();
            return;
        };
        let (base_width, base_height) = match toniator::document_artboard(&document) {
            Ok(size) => size,
            Err(error) => {
                self.export_running.set(false);
                self.update_actions();
                self.show_error(&format!("Could not prepare PNG export: {error:#}"));
                return;
            }
        };
        let window = adw::Window::builder()
            .title("Export PNG Image")
            .transient_for(&self.window)
            .modal(true)
            .default_width(430)
            .build();
        let proceeding = Rc::new(Cell::new(false));
        window.connect_close_request(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[strong]
            proceeding,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_| {
                if !proceeding.get() {
                    ui.export_running.set(false);
                    ui.update_actions();
                }
                glib::Propagation::Proceed
            }
        ));
        let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        content.set_margin_top(20);
        content.set_margin_bottom(20);
        content.set_margin_start(20);
        content.set_margin_end(20);
        content.append(
            &gtk::Label::builder()
                .label("PNG Image")
                .xalign(0.0)
                .css_classes(["title-2"])
                .build(),
        );
        content.append(
            &gtk::Label::builder()
                .label("A flattened image for sharing or printing. Width and height stay linked to the artwork.")
                .xalign(0.0)
                .wrap(true)
                .css_classes(["dim-label"])
                .build(),
        );
        let size = gtk::DropDown::from_strings(&["Document Size", "2×", "Custom"]);
        content.append(&combo_row("PNG Size", &size));
        let dimensions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let width = gtk::SpinButton::with_range(1.0, 16_000.0, 1.0);
        let height = gtk::SpinButton::with_range(1.0, 16_000.0, 1.0);
        disable_pointer_scroll_adjustment(&width);
        disable_pointer_scroll_adjustment(&height);
        width.set_value(base_width as f64);
        height.set_value(base_height as f64);
        width.set_hexpand(true);
        height.set_hexpand(true);
        dimensions.append(&width);
        dimensions.append(&gtk::Label::new(Some("×")));
        dimensions.append(&height);
        content.append(
            &gtk::Label::builder()
                .label("Pixels")
                .xalign(0.0)
                .css_classes(["heading"])
                .build(),
        );
        content.append(&dimensions);
        let background = gtk::DropDown::from_strings(&[
            "Document Export Background",
            "Transparent Override",
            "White Override",
        ]);
        content.append(&combo_row("PNG Background", &background));
        let summary = gtk::Label::builder()
            .xalign(0.0)
            .css_classes(["dim-label", "caption"])
            .build();
        content.append(&summary);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        let cancel = gtk::Button::with_label("Cancel");
        let confirm = gtk::Button::with_label("Export…");
        confirm.add_css_class("suggested-action");
        actions.append(&cancel);
        actions.append(&confirm);
        content.append(&actions);
        window.set_content(Some(&content));

        let syncing = Rc::new(Cell::new(false));
        let document_export_background = document.appearance.export_background;
        let update_summary: Rc<dyn Fn()> = Rc::new(glib::clone!(
            #[weak]
            summary,
            #[weak]
            width,
            #[weak]
            height,
            #[weak]
            background,
            move || {
                let selection = png_background_selection_summary(
                    background.selected(),
                    document_export_background,
                );
                let text = format!(
                    "PNG · {} × {} px · {selection}",
                    width.value_as_int(),
                    height.value_as_int(),
                );
                summary.set_text(&text);
                summary.set_tooltip_text(Some(&text));
                summary.update_property(&[
                    gtk::accessible::Property::Label(&text),
                    gtk::accessible::Property::Description(
                        "Current PNG export summary. Preview Surface never affects exported pixels.",
                    ),
                ]);
                background.update_property(&[gtk::accessible::Property::Description(&text)]);
            }
        ));
        update_summary();
        size.connect_selected_notify(glib::clone!(
            #[weak]
            width,
            #[weak]
            height,
            #[strong]
            syncing,
            #[strong]
            update_summary,
            move |size| {
                syncing.set(true);
                match size.selected() {
                    0 => {
                        width.set_value(base_width as f64);
                        height.set_value(base_height as f64);
                    }
                    1 => {
                        width.set_value((base_width * 2) as f64);
                        height.set_value((base_height * 2) as f64);
                    }
                    _ => {}
                }
                let custom = size.selected() == 2;
                width.set_sensitive(custom);
                height.set_sensitive(custom);
                syncing.set(false);
                update_summary();
            }
        ));
        width.set_sensitive(false);
        height.set_sensitive(false);
        width.connect_value_changed(glib::clone!(
            #[weak]
            height,
            #[strong]
            syncing,
            #[strong]
            update_summary,
            move |width| {
                if syncing.get() {
                    return;
                }
                syncing.set(true);
                height.set_value((width.value() * base_height as f64 / base_width as f64).round());
                syncing.set(false);
                update_summary();
            }
        ));
        height.connect_value_changed(glib::clone!(
            #[weak]
            width,
            #[strong]
            syncing,
            #[strong]
            update_summary,
            move |height| {
                if syncing.get() {
                    return;
                }
                syncing.set(true);
                width.set_value((height.value() * base_width as f64 / base_height as f64).round());
                syncing.set(false);
                update_summary();
            }
        ));
        background.connect_selected_notify(glib::clone!(
            #[strong]
            update_summary,
            move |_| update_summary()
        ));
        cancel.connect_clicked(glib::clone!(
            #[weak]
            window,
            move |_| window.close()
        ));
        confirm.connect_clicked(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            #[weak]
            window,
            #[weak]
            width,
            #[weak]
            height,
            #[weak]
            background,
            #[strong]
            proceeding,
            move |_| {
                let options = toniator::PngExportOptions {
                    width: width.value_as_int().max(1) as u32,
                    height: height.value_as_int().max(1) as u32,
                    background: match background.selected() {
                        0 => toniator::PngBackground::Document,
                        1 => toniator::PngBackground::Transparent,
                        _ => toniator::PngBackground::White,
                    },
                    channel: None,
                };
                proceeding.set(true);
                window.close();
                ui.export_png_dialog(document.clone(), options);
            }
        ));
        window.present();
    }

    fn export_png_dialog(self: &Rc<Self>, document: Document, options: toniator::PngExportOptions) {
        let source_name = Path::new(&document.source.name)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        let dialog = gtk::FileDialog::builder()
            .title("Export PNG Image")
            .initial_name(format!("{source_name} — Halftone.png"))
            .modal(true)
            .build();
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        let png = gtk::FileFilter::new();
        png.set_name(Some("PNG Image (.png)"));
        png.add_pattern("*.png");
        filters.append(&png);
        dialog.set_filters(Some(&filters));
        dialog.set_default_filter(Some(&png));
        dialog.save(
            Some(&self.window),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |result| {
                    if let Ok(file) = result
                        && let Some(path) = file.path()
                    {
                        ui.start_png_export(ensure_extension(path, "png"), document, options);
                        return;
                    }
                    ui.export_running.set(false);
                    ui.update_actions();
                }
            ),
        );
    }

    fn start_export(&self, path: PathBuf, document: Document) {
        self.export.set_sensitive(false);
        self.set_workspace_status("Exporting editable SVG…");
        self.cancel_export.set_visible(true);
        self.show_message(&format!("Exporting {}…", path.display()));
        let results = Arc::clone(&self.export_results);
        let token = CancellationToken::new();
        self.export_token.replace(Some(token.clone()));
        std::thread::spawn(move || {
            let result = match export_svg_cancellable(&path, &document, &token) {
                Ok(()) => ExportResult::Completed,
                Err(error) if error.downcast_ref::<OperationCancelled>().is_some() => {
                    ExportResult::Cancelled
                }
                Err(error) => ExportResult::Failed(error),
            };
            results.replace(ExportOutcome {
                path,
                kind: "editable SVG",
                result,
            });
        });
    }

    fn start_png_export(
        &self,
        path: PathBuf,
        document: Document,
        options: toniator::PngExportOptions,
    ) {
        self.export.set_sensitive(false);
        self.set_workspace_status("Exporting PNG image…");
        self.cancel_export.set_visible(true);
        self.show_message(&format!("Exporting PNG {}…", path.display()));
        let results = Arc::clone(&self.export_results);
        let token = CancellationToken::new();
        self.export_token.replace(Some(token.clone()));
        std::thread::spawn(move || {
            let result = match toniator::export_png_cancellable(&path, &document, options, &token) {
                Ok(()) => ExportResult::Completed,
                Err(error) if error.downcast_ref::<OperationCancelled>().is_some() => {
                    ExportResult::Cancelled
                }
                Err(error) => ExportResult::Failed(error),
            };
            results.replace(ExportOutcome {
                path,
                kind: "PNG",
                result,
            });
        });
    }

    fn poll_export_results(&self) {
        let Some(outcome) = self.export_results.take() else {
            return;
        };
        self.export_running.set(false);
        self.export_token.take();
        self.cancel_export.set_visible(false);
        self.update_actions();
        match outcome.result {
            ExportResult::Completed => {
                self.set_workspace_status(&format!("Exported {}", outcome.kind));
                self.show_message(&format!(
                    "Exported {}: {}",
                    outcome.kind,
                    outcome.path.display()
                ));
            }
            ExportResult::Cancelled => {
                self.set_workspace_status("Export cancelled");
            }
            ExportResult::Failed(error) => {
                self.set_workspace_status(
                    "Export could not finish cleanly — check the destination file",
                );
                self.show_error(&format!(
                    "Could not export {}: {error:#}",
                    outcome.path.display()
                ));
            }
        }
    }

    fn cancel_export(&self) {
        if self
            .export_token
            .borrow()
            .as_ref()
            .is_some_and(CancellationToken::cancel)
        {
            self.set_workspace_status("Cancelling export…");
        }
    }

    fn update_actions(&self) {
        let state = self.state.borrow();
        let has_document = state.editor.is_some();
        self.save.set_sensitive(has_document);
        self.preset_import.set_sensitive(has_document);
        self.preset_save.set_sensitive(has_document);
        self.export
            .set_sensitive(has_document && !self.export_running.get());
        self.undo
            .set_sensitive(state.editor.as_ref().is_some_and(DocumentEditor::can_undo));
        self.redo
            .set_sensitive(state.editor.as_ref().is_some_and(DocumentEditor::can_redo));
        self.new_project_button.set_visible(has_document);
        self.save.set_visible(has_document);
        self.undo.set_visible(has_document);
        self.redo.set_visible(has_document);
        self.controls_toggle.set_visible(has_document);
        self.export.set_visible(has_document);
        let (name, dirty) = state
            .editor
            .as_ref()
            .map(|editor| {
                let name = state
                    .path
                    .as_ref()
                    .and_then(|path| path.file_stem())
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| editor.document().source.name.clone());
                (name, editor.is_dirty() || state.path.is_none())
            })
            .unwrap_or_else(|| ("Toniator".into(), false));
        let status = if self.export_running.get() {
            "Exporting"
        } else if dirty {
            "Unsaved"
        } else if has_document {
            "Saved"
        } else {
            "Start a project"
        };
        let window_title = if dirty {
            format!("{name} — Unsaved — Toniator")
        } else {
            format!("{name} — Toniator")
        };
        self.title.set_title(&name);
        self.title.set_subtitle(status);
        self.window.set_title(Some(&window_title));
    }

    fn set_workspace_status(&self, status: &str) {
        self.workspace_status.set_text(status);
    }

    fn update_editing_context(&self) {
        let state = self.state.borrow();
        let Some(editor) = state.editor.as_ref() else {
            self.editing_context.set_text("No artwork open");
            return;
        };
        let document = editor.document();
        let context = if document.pattern_state.selected_pattern_id()
            == Some(PatternId::COMPATIBILITY_SHAPES_V1)
        {
            let Ok(settings) = document.pattern_state.shape_settings() else {
                return;
            };
            if document_uses_crosshatch(document) {
                crosshatch_context("Shapes", self.web_target.selected(), self.web_angle.value())
            } else {
                let layer = if document.artwork_pipeline.output_model == OutputModel::RgbScreen {
                    match self.web_target.selected() {
                        1 => "Red channel",
                        2 => "Green channel",
                        3 => "Blue channel",
                        _ => "All channels",
                    }
                } else {
                    match self.web_target.selected() {
                        1 => "Cyan",
                        2 => "Magenta",
                        3 => "Yellow",
                        4 => "Black",
                        _ => "All inks",
                    }
                };
                if document.artwork_pipeline.output_model == OutputModel::RgbScreen {
                    let visibility = rgb_visibility_summary(&settings)
                        .map(|summary| format!(" · {summary}"))
                        .unwrap_or_default();
                    format!("Shapes · RGB Screen · {layer}{visibility}")
                } else {
                    format!("Shapes · {layer}")
                }
            }
        } else if document.pattern_state.selected_pattern_id()
            == Some(PatternId::COMPATIBILITY_CURVES_V1)
        {
            if document_uses_crosshatch(document) {
                crosshatch_context(
                    "Curves",
                    self.curve_target.selected(),
                    self.curve_angle.value(),
                )
            } else {
                let layer = if document.artwork_pipeline.output_model == OutputModel::RgbScreen {
                    match self.curve_target.selected() {
                        1 => "Red channel",
                        2 => "Green channel",
                        3 => "Blue channel",
                        _ => "All channels",
                    }
                } else {
                    match self.curve_target.selected() {
                        1 => "Cyan",
                        2 => "Magenta",
                        3 => "Yellow",
                        4 => "Black",
                        _ => "All inks",
                    }
                };
                if document.artwork_pipeline.output_model == OutputModel::RgbScreen {
                    format!("Curves · RGB Screen · {layer}")
                } else {
                    format!("Curves · {layer}")
                }
            }
        } else {
            "Shapes · Basic treatment".into()
        };
        let operation = if self.motif_drag.get().is_some() {
            " · Adjusting motif on canvas"
        } else if self.curve_selected_handle.get() >= 0 {
            " · Curve point selected"
        } else {
            ""
        };
        self.editing_context
            .set_text(&format!("{context}{operation}"));
    }

    fn set_fit(self: &Rc<Self>) {
        self.state.borrow_mut().zoom_mode = ZoomMode::Fit(100.0);
        self.fit_allocation.borrow_mut().reset();
        self.apply_fit_zoom();
    }

    fn apply_zoom_mode(self: &Rc<Self>) {
        let mode = self.state.borrow().zoom_mode;
        match mode {
            ZoomMode::Fit(_) => self.apply_fit_zoom(),
            ZoomMode::Explicit(percent) => self.apply_zoom_percent(percent, false),
        }
    }

    fn apply_fit_zoom(self: &Rc<Self>) {
        let Some(artboard) = self
            .state
            .borrow()
            .editor
            .as_ref()
            .map(|editor| document_artboard_size(editor.document()))
        else {
            return;
        };
        let viewport = self.canvas_viewport_size();
        let input = (artboard, viewport, self.window.scale_factor());
        let Some(refinement_generation) = self.fit_allocation.borrow_mut().observe(input) else {
            return;
        };
        let mode = self.state.borrow().zoom_mode.update_fit(
            artboard,
            viewport,
            self.window.scale_factor(),
        );
        let percent = mode.percent();
        if !matches!(self.state.borrow().zoom_mode, ZoomMode::Fit(current) if current == percent) {
            self.state.borrow_mut().zoom_mode = mode;
        }
        self.apply_zoom_percent(percent, true);
        self.schedule_fit_refinement(refinement_generation);
    }

    fn schedule_fit_refinement(self: &Rc<Self>, refinement_generation: u64) {
        glib::timeout_add_local_once(
            Duration::from_millis(180),
            glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move || {
                    if !ui.fit_allocation.borrow().accepts(refinement_generation) {
                        return;
                    }
                    let target = {
                        let state = ui.state.borrow();
                        let Some(editor) = state.editor.as_ref() else {
                            return;
                        };
                        fit_refinement_target(
                            document_artboard_size(editor.document()),
                            state.zoom_mode,
                            state.preview_size,
                        )
                    };
                    if let Some(target) = target {
                        ui.request_preview_at(target);
                    }
                }
            ),
        );
    }

    fn apply_zoom_percent(&self, percent: f64, fit: bool) {
        let text = zoom_percent_text(percent);
        if (self.zoom.value() - percent).abs() > 1e-9 || self.zoom_entry.text() != text {
            self.state.borrow_mut().syncing_controls = true;
            sync_zoom_control_widgets(&self.fit, &self.zoom, &self.zoom_entry, percent, fit);
            self.state.borrow_mut().syncing_controls = false;
        } else {
            self.fit.set_active(fit);
        }
        let Some((width, height)) = self
            .state
            .borrow()
            .editor
            .as_ref()
            .map(|editor| document_artboard_size(editor.document()))
        else {
            return;
        };
        let (logical_width, logical_height) =
            scaled_artboard_size(width, height, percent / 100.0, self.window.scale_factor());
        self.canvas_content.set_hexpand(false);
        self.canvas_content.set_vexpand(false);
        self.canvas_content.set_halign(gtk::Align::Center);
        self.canvas_content.set_valign(gtk::Align::Center);
        if self.canvas_content.width_request() != logical_width
            || self.canvas_content.height_request() != logical_height
        {
            self.canvas_content
                .set_size_request(logical_width, logical_height);
            if let Some(stage) = self.canvas_content.parent() {
                stage.queue_resize();
            }
            self.canvas.queue_resize();
        }
        self.picture.set_hexpand(true);
        self.picture.set_vexpand(true);
        self.picture.set_content_fit(gtk::ContentFit::Contain);
    }

    fn canvas_viewport_size(&self) -> (i32, i32) {
        let vertical_scrollbar = self.canvas.vscrollbar();
        let vertical_scrollbar = if vertical_scrollbar.is_visible() {
            vertical_scrollbar.width()
        } else {
            0
        };
        let horizontal_scrollbar = self.canvas.hscrollbar();
        let horizontal_scrollbar = if horizontal_scrollbar.is_visible() {
            horizontal_scrollbar.height()
        } else {
            0
        };
        (
            (self.canvas.width() - vertical_scrollbar).max(1),
            (self.canvas.height() - horizontal_scrollbar).max(1),
        )
    }

    fn set_explicit_zoom(self: &Rc<Self>, intent: ZoomIntent) {
        let mode = self.state.borrow().zoom_mode.apply_manual(intent);
        let percent = mode.percent();
        self.zoom.adjustment().set_lower(ZOOM_MIN);
        self.zoom.adjustment().set_upper(ZOOM_MAX);
        self.state.borrow_mut().zoom_mode = mode;
        self.apply_zoom_percent(percent, false);
        self.schedule_zoom_refinement();
    }

    fn commit_zoom_text(self: &Rc<Self>, text: &str) {
        let current = match self.state.borrow().zoom_mode {
            ZoomMode::Fit(percent) | ZoomMode::Explicit(percent) => percent,
        };
        let percent = text
            .trim()
            .trim_end_matches('%')
            .parse::<f64>()
            .unwrap_or(current);
        self.set_explicit_zoom(ZoomIntent::Entry(percent));
    }

    fn show_message(&self, text: &str) {
        self.toast_overlay.add_toast(adw::Toast::new(text));
    }

    fn show_error(&self, text: &str) {
        let toast = adw::Toast::new(text);
        toast.set_timeout(8);
        self.toast_overlay.add_toast(toast);
    }
}

fn render_worker(
    requests: Arc<LatestSlot<RenderRequest>>,
    results: Arc<LatestSlot<RenderOutcome>>,
) {
    loop {
        let request = requests.wait_take();
        let result = if request.compare_source {
            request
                .token
                .checkpoint()
                .map_err(anyhow::Error::from)
                .and_then(|_| {
                    toniator::render::decode_source(&request.document.source, request.max_dimension)
                })
                .and_then(|image| {
                    request.token.checkpoint().map_err(anyhow::Error::from)?;
                    Ok(toniator::composite_preview(
                        image,
                        request.document.appearance,
                    ))
                })
        } else {
            render_document_preview_cancellable(
                &request.document,
                request.max_dimension,
                request.generation,
                &request.token,
            )
            .map(|rendered| rendered.image)
        };
        results.replace(RenderOutcome {
            generation: request.generation,
            view: if request.compare_source {
                PreviewView::Source
            } else {
                PreviewView::Rendered
            },
            document: request.document,
            result,
        });
    }
}

fn autosave_worker(
    requests: Arc<LatestSlot<AutosaveRequest>>,
    results: Arc<LatestSlot<AutosaveOutcome>>,
    current_generation: Arc<AtomicU64>,
    io_lock: Arc<Mutex<()>>,
) {
    loop {
        let mut request = requests.wait_take();
        loop {
            std::thread::sleep(Duration::from_millis(450));
            let Some(newer) = requests.take() else {
                break;
            };
            request = newer;
        }
        let document_id = request.document.document_id.clone();
        let _guard = io_lock.lock().expect("recovery I/O lock poisoned");
        if current_generation.load(Ordering::SeqCst) != request.generation {
            continue;
        }
        let result = save_document_atomic(&recovery_path(), &request.document);
        results.replace(AutosaveOutcome {
            document_id,
            result,
        });
    }
}

struct StartWidgets {
    container: gtk::Widget,
    open_artwork: gtk::Button,
    open_document: gtk::Button,
    try_example: gtk::Button,
    recover: Option<gtk::Button>,
}

fn build_start_view(builder: &gtk::Builder, has_recovery: bool) -> StartWidgets {
    let page = builder
        .object::<gtk::ScrolledWindow>("start_page")
        .expect("toniator-window.blp must define start_page");
    let hero = builder
        .object::<gtk::Picture>("start_hero")
        .expect("toniator-window.blp must define start_hero");
    if let Ok(texture) = gdk::Texture::from_bytes(&glib::Bytes::from_static(START_HERO)) {
        hero.set_paintable(Some(&texture));
    }
    hero.update_property(&[gtk::accessible::Property::Label(
        "Toniator halftone artwork",
    )]);
    let open_artwork = builder
        .object::<gtk::Button>("open_artwork")
        .expect("toniator-window.blp must define open_artwork");
    let open_document = builder
        .object::<gtk::Button>("open_document")
        .expect("toniator-window.blp must define open_document");
    let try_example = builder
        .object::<gtk::Button>("try_example")
        .expect("toniator-window.blp must define try_example");
    let recover = has_recovery.then(|| {
        let button = gtk::Button::with_label("Recover Autosaved Work");
        button.add_css_class("flat");
        button.add_css_class("accent");
        let host = builder
            .object::<gtk::Box>("start_recovery_host")
            .expect("toniator-window.blp must define start_recovery_host");
        host.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        host.append(&button);
        button
    });
    StartWidgets {
        container: page.upcast(),
        open_artwork,
        open_document,
        try_example,
        recover,
    }
}

struct EditorWidgets {
    container: gtk::Widget,
    paned: adw::OverlaySplitView,
    #[cfg(test)]
    inspector_root: gtk::Box,
    workspace_status: gtk::Label,
    cancel_preview: gtk::Button,
    cancel_export: gtk::Button,
    editing_context: gtk::Label,
    canvas: gtk::ScrolledWindow,
    canvas_content: gtk::Overlay,
    fit: gtk::ToggleButton,
    zoom_out: gtk::Button,
    zoom: gtk::Scale,
    zoom_entry: gtk::Entry,
    zoom_in: gtk::Button,
    detail: gtk::Scale,
    coverage: gtk::Scale,
    contrast: gtk::Scale,
    angle: gtk::Scale,
    dots: gtk::ToggleButton,
    squares: gtk::ToggleButton,
    lines: gtk::ToggleButton,
    curves: gtk::ToggleButton,
    weighted_voronoi: gtk::ToggleButton,
    legacy: gtk::ToggleButton,
    treatment_modes: gtk::Stack,
    weighted_voronoi_channel: gtk::DropDown,
    weighted_voronoi_cell_count: gtk::Scale,
    weighted_voronoi_visible: [gtk::CheckButton; 4],
    weighted_voronoi_arrangement: gtk::DropDown,
    weighted_voronoi_placement: gtk::DropDown,
    weighted_voronoi_density_strength: gtk::Scale,
    weighted_voronoi_response_strength: gtk::Scale,
    weighted_voronoi_boundary_gap: gtk::Scale,
    weighted_voronoi_seed: gtk::Entry,
    preset_import: gtk::Button,
    preset_save: gtk::Button,
    source_section: gtk::Expander,
    output_section: gtk::Expander,
    channel_settings_section: gtk::Expander,
    output_mode: gtk::DropDown,
    artwork_source: gtk::DropDown,
    artwork_source_note: gtk::Label,
    source_alpha: gtk::DropDown,
    source_alpha_row: gtk::Widget,
    source_alpha_note: gtk::Label,
    channel_assignment: gtk::DropDown,
    channel_assignment_note: gtk::Label,
    active_channel: gtk::DropDown,
    active_channel_row: gtk::Widget,
    channel_scope: gtk::DropDown,
    channel_panel_stack: gtk::Stack,
    channel_controls: Vec<ChannelControlWidgets>,
    aggregate_channel_controls: AggregateChannelControlWidgets,
    crosshatch_action: gtk::Button,
    crosshatch_note: gtk::Label,
    preview_surface: gtk::DropDown,
    preview_color: gtk::ColorDialogButton,
    export_background: gtk::DropDown,
    export_color_label: gtk::Label,
    export_color: gtk::ColorDialogButton,
    web_shared: gtk::CheckButton,
    web_shared_help: Option<HelpHandle>,
    web_shape: gtk::DropDown,
    web_shape_row: gtk::Widget,
    web_mixed_shape_label: gtk::Label,
    web_mixed_shape_apply: gtk::DropDown,
    web_mixed_shape_apply_row: gtk::Widget,
    web_polygon_sides: gtk::SpinButton,
    web_polygon_sides_row: gtk::Widget,
    web_polygon_sides_label: gtk::Label,
    web_edit_shape: gtk::Button,
    web_target: gtk::DropDown,
    web_target_label: gtk::Label,
    web_target_help: Option<HelpHandle>,
    web_visible_label: gtk::Label,
    web_visible_help: HelpHandle,
    web_visible: [gtk::CheckButton; 4],
    web_color: gtk::Entry,
    web_color_row: gtk::Widget,
    web_color_heading: gtk::Label,
    web_color_help: HelpHandle,
    web_crosshatch_color: gtk::Entry,
    web_crosshatch_color_row: gtk::Widget,
    web_color_status: gtk::Label,
    web_coverage: gtk::Scale,
    web_coverage_status: gtk::Label,
    web_angle: gtk::Scale,
    web_angle_status: gtk::Label,
    web_mark_angle: gtk::Scale,
    web_mark_angle_status: gtk::Label,
    web_width_scale: gtk::Scale,
    web_width_scale_status: gtk::Label,
    web_height_scale: gtk::Scale,
    web_height_scale_status: gtk::Label,
    web_threshold: gtk::Scale,
    web_threshold_status: gtk::Label,
    web_opacity: gtk::Scale,
    web_opacity_heading: gtk::Label,
    web_opacity_help: HelpHandle,
    web_opacity_status: gtk::Label,
    web_detail: gtk::Scale,
    web_detail_status: gtk::Label,
    web_mixed: gtk::Label,
    web_geometry_note: gtk::Label,
    curve_layout: gtk::DropDown,
    curve_profile: gtk::DropDown,
    curve_editor_label: gtk::Label,
    curve_editor: gtk::DrawingArea,
    curve_reset: gtk::Button,
    curve_shared: gtk::CheckButton,
    curve_shared_help: Option<HelpHandle>,
    curve_target: gtk::DropDown,
    curve_target_label: gtk::Label,
    curve_target_help: Option<HelpHandle>,
    curve_visible_label: gtk::Label,
    curve_visible_help: HelpHandle,
    curve_visible: [gtk::CheckButton; 4],
    curve_color: gtk::Entry,
    curve_color_row: gtk::Widget,
    curve_crosshatch_color: gtk::Entry,
    curve_crosshatch_color_row: gtk::Widget,
    curve_color_status: gtk::Label,
    curve_weight: gtk::Scale,
    curve_spacing: gtk::Scale,
    curve_coverage: gtk::Scale,
    curve_coverage_status: gtk::Label,
    curve_angle: gtk::Scale,
    curve_angle_status: gtk::Label,
    curve_position_x: gtk::Scale,
    curve_position_x_status: gtk::Label,
    curve_position_y: gtk::Scale,
    curve_position_y_status: gtk::Label,
    curve_opacity: gtk::Scale,
    curve_opacity_status: gtk::Label,
    curve_threshold: gtk::Scale,
    curve_threshold_status: gtk::Label,
    curve_detail: gtk::Scale,
    curve_detail_status: gtk::Label,
    curve_close_ends: gtk::CheckButton,
    curve_smooth_join: gtk::CheckButton,
    curve_mixed: gtk::Label,
    motif_controls: gtk::Widget,
    motif_coverage: gtk::DropDown,
    motif_size: gtk::Scale,
    motif_columns: gtk::Scale,
    motif_rows: gtk::Scale,
    motif_row_spacing: gtk::Scale,
    motif_stagger: gtk::Scale,
    motif_alternate: gtk::DropDown,
    motif_arrange: gtk::CheckButton,
    motif_mixed: gtk::Label,
    motif_overlay: gtk::DrawingArea,
}

struct AppearanceControlWidgets {
    #[cfg(test)]
    container: gtk::Box,
    preview_surface: gtk::DropDown,
    preview_color: gtk::ColorDialogButton,
    #[cfg(test)]
    preview_help: gtk::MenuButton,
    export_background: gtk::DropDown,
    export_color_label: gtk::Label,
    export_color: gtk::ColorDialogButton,
    #[cfg(test)]
    export_help: gtk::MenuButton,
}

fn build_appearance_controls(builder: &gtk::Builder) -> AppearanceControlWidgets {
    #[cfg(test)]
    let container = builder
        .object::<gtk::Box>("appearance_controls")
        .expect("toniator-window.blp must define appearance_controls");
    let preview_surface = builder
        .object::<gtk::DropDown>("preview_surface")
        .expect("ToniatorEditorControls.ui must define preview_surface");
    sync_dropdown_strings(
        &preview_surface,
        &["Checkerboard", "Color over checkerboard"],
    );
    preview_surface.set_tooltip_text(Some(help_for("Preview Surface").unwrap().summary));
    preview_surface.update_property(&[gtk::accessible::Property::Label("Preview Surface")]);
    let preview_color = builder
        .object::<gtk::ColorDialogButton>("preview_color")
        .expect("ToniatorEditorControls.ui must define preview_color");
    preview_color.set_dialog(
        &gtk::ColorDialog::builder()
            .title("Preview Surface Color")
            .with_alpha(true)
            .build(),
    );
    preview_color.update_property(&[gtk::accessible::Property::Label("Preview Surface Color")]);
    let preview_help = help_button(help_for("Preview Surface").unwrap());
    preview_help.set_tooltip_text(Some("Help: Preview Surface"));
    builder
        .object::<gtk::Box>("preview_help_host")
        .expect("ToniatorEditorControls.ui must define preview_help_host")
        .append(&preview_help);
    let export_background = builder
        .object::<gtk::DropDown>("export_background")
        .expect("ToniatorEditorControls.ui must define export_background");
    sync_dropdown_strings(&export_background, &["None", "Color"]);
    export_background.set_tooltip_text(Some(help_for("Export Background").unwrap().summary));
    export_background.update_property(&[gtk::accessible::Property::Label("Export Background")]);
    let export_color = builder
        .object::<gtk::ColorDialogButton>("export_color")
        .expect("ToniatorEditorControls.ui must define export_color");
    let export_color_label = builder
        .object::<gtk::Label>("export_color_label")
        .expect("ToniatorEditorControls.ui must define export_color_label");
    export_color.set_dialog(
        &gtk::ColorDialog::builder()
            .title("Export Background Color")
            .with_alpha(true)
            .build(),
    );
    sync_export_background_color_control(
        &export_color_label,
        &export_color,
        ExportBackground::None,
    );
    let export_help = help_button(help_for("Export Background").unwrap());
    export_help.set_tooltip_text(Some("Help: Export Background"));
    builder
        .object::<gtk::Box>("export_help_host")
        .expect("ToniatorEditorControls.ui must define export_help_host")
        .append(&export_help);
    AppearanceControlWidgets {
        #[cfg(test)]
        container,
        preview_surface,
        preview_color,
        #[cfg(test)]
        preview_help,
        export_background,
        export_color_label,
        export_color,
        #[cfg(test)]
        export_help,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_editor_view(
    builder: &gtk::Builder,
    preview_indicator: &gtk::DrawingArea,
    recovery_enabled: bool,
    inspector_width: i32,
    initial_layout_width: i32,
) -> EditorWidgets {
    let layout = builder
        .object::<adw::OverlaySplitView>("editor_split_view")
        .expect("toniator-window.blp must define editor_split_view");
    layout.set_sidebar_width_unit(adw::LengthUnit::Px);
    layout.set_min_sidebar_width(INSPECTOR_MIN_WIDTH as f64);
    layout.set_max_sidebar_width(INSPECTOR_MAX_WIDTH as f64);
    layout.set_sidebar_width_fraction(
        (constrained_inspector_width(inspector_width, initial_layout_width) as f64
            / initial_layout_width.max(1) as f64)
            .clamp(0.1, 0.9),
    );
    let canvas_overlay = builder
        .object::<gtk::Overlay>("canvas_content")
        .expect("toniator-window.blp must define canvas_content");
    let motif_overlay = builder
        .object::<gtk::DrawingArea>("motif_overlay")
        .expect("toniator-window.blp must define motif_overlay");
    let canvas = builder
        .object::<gtk::ScrolledWindow>("canvas")
        .expect("toniator-window.blp must define canvas");
    let fit = builder
        .object::<gtk::ToggleButton>("fit")
        .expect("toniator-window.blp must define fit");
    let zoom_out = builder
        .object::<gtk::Button>("zoom_out")
        .expect("toniator-window.blp must define zoom_out");
    let zoom_in = builder
        .object::<gtk::Button>("zoom_in")
        .expect("toniator-window.blp must define zoom_in");
    let zoom = builder
        .object::<gtk::Scale>("zoom")
        .expect("toniator-window.blp must define zoom");
    zoom.set_range(ZOOM_MIN, ZOOM_MAX);
    zoom.set_increments(1.0, 25.0);
    disable_pointer_scroll_adjustment(&zoom);
    zoom.set_value(100.0);
    zoom.set_tooltip_text(Some("Canvas zoom"));
    fit.set_tooltip_text(Some(
        "Fit complete artwork; fitted percentage may be below 5% for very large artwork",
    ));
    compact_control_help(&fit, "Fit Artwork");
    zoom.update_property(&[gtk::accessible::Property::Label("Canvas zoom")]);
    let zoom_entry = builder
        .object::<gtk::Entry>("zoom_entry")
        .expect("toniator-window.blp must define zoom_entry");
    zoom_entry.set_tooltip_text(Some(
        "Explicit zoom from 5% to 800%; Fit may calculate a smaller value",
    ));
    zoom_entry.set_width_chars(7);
    zoom_entry.update_property(&[gtk::accessible::Property::Label("Zoom percentage")]);
    compact_control_help(&zoom, "Canvas Zoom");
    compact_control_help(&zoom_entry, "Canvas Zoom");
    let rendered_view = builder
        .object::<gtk::ToggleButton>("rendered_view")
        .expect("toniator-window.blp must define rendered_view");
    let compare = builder
        .object::<gtk::ToggleButton>("compare")
        .expect("toniator-window.blp must define compare");
    rendered_view.set_group(Some(&compare));
    rendered_view.set_active(true);
    builder
        .object::<gtk::Box>("preview_indicator_host")
        .expect("toniator-window.blp must define preview_indicator_host")
        .append(preview_indicator);
    let workspace_status = builder
        .object::<gtk::Label>("workspace_status")
        .expect("toniator-window.blp must define workspace_status");
    let cancel_preview = builder
        .object::<gtk::Button>("cancel_preview")
        .expect("toniator-window.blp must define cancel_preview");
    let cancel_export = builder
        .object::<gtk::Button>("cancel_export")
        .expect("toniator-window.blp must define cancel_export");
    let autosave_status = builder
        .object::<gtk::Label>("autosave_status")
        .expect("toniator-window.blp must define autosave_status");
    autosave_status.set_text(if recovery_enabled {
        "Recovery is ready"
    } else {
        "Recovery is isolated for artifact capture"
    });
    let treatment_builder = builder.clone();
    let dots = treatment_builder
        .object::<gtk::ToggleButton>("dots")
        .expect("ToniatorEditorControls.ui must define dots");
    let squares = treatment_builder
        .object::<gtk::ToggleButton>("squares")
        .expect("ToniatorEditorControls.ui must define squares");
    let lines = treatment_builder
        .object::<gtk::ToggleButton>("lines")
        .expect("ToniatorEditorControls.ui must define lines");
    let curves = treatment_builder
        .object::<gtk::ToggleButton>("curves")
        .expect("ToniatorEditorControls.ui must define curves");
    let legacy = treatment_builder
        .object::<gtk::ToggleButton>("legacy")
        .expect("ToniatorEditorControls.ui must define legacy");
    squares.set_group(Some(&dots));
    lines.set_group(Some(&dots));
    curves.set_group(Some(&dots));
    legacy.set_group(Some(&dots));
    legacy.set_visible(false);
    dots.set_active(true);
    dots.set_hexpand(true);
    squares.set_visible(false);
    lines.set_visible(false);
    curves.set_hexpand(true);
    for (button, label) in [
        (&dots, "Shapes pattern"),
        (&squares, "Squares treatment"),
        (&lines, "Lines treatment"),
        (&curves, "Curves pattern"),
    ] {
        button.update_property(&[gtk::accessible::Property::Label(label)]);
    }
    let preset_import = treatment_builder
        .object::<gtk::Button>("preset_import")
        .expect("ToniatorEditorControls.ui must define preset_import");
    preset_import.set_hexpand(true);
    preset_import.set_tooltip_text(Some("Load a Toniator halftone preset (.tntr)"));
    let preset_save = treatment_builder
        .object::<gtk::Button>("preset_save")
        .expect("ToniatorEditorControls.ui must define preset_save");
    preset_save.set_hexpand(true);
    preset_save.set_tooltip_text(Some("Save this halftone setup without the artwork"));
    if let Some(spec) = help_for("Load Preset") {
        preset_import.set_tooltip_text(Some(spec.summary));
        preset_import.update_property(&[gtk::accessible::Property::Description(spec.summary)]);
    }
    if let Some(spec) = help_for("Save Preset") {
        preset_save.set_tooltip_text(Some(spec.summary));
        preset_save.update_property(&[gtk::accessible::Property::Description(spec.summary)]);
    }

    let native_panel = treatment_builder
        .object::<gtk::Box>("native_panel")
        .expect("ToniatorEditorControls.ui must define native_panel");
    let detail = builder_control_scale(&treatment_builder, NATIVE_CONTROL_SCALES[0]);
    detail.set_format_value_func(|_, value| format!("{value:0.0}"));
    let coverage = builder_control_scale(&treatment_builder, NATIVE_CONTROL_SCALES[1]);
    coverage.set_format_value_func(|_, value| format!("{value:0.0}%"));
    let contrast = builder_control_scale(&treatment_builder, NATIVE_CONTROL_SCALES[2]);
    contrast.set_format_value_func(|_, value| format!("{value:0.0}%"));
    let angle = builder_control_scale(&treatment_builder, NATIVE_CONTROL_SCALES[3]);
    angle.set_format_value_func(|_, value| format!("{value:0.0}°"));

    let native_channel_copy = treatment_builder
        .object::<gtk::Label>("native_channel_copy")
        .expect("ToniatorEditorControls.ui must define native_channel_copy");
    native_channel_copy.set_tooltip_text(Some(
        "Toniator automatically separates artwork into Cyan, Magenta, Yellow, and Black inks",
    ));
    native_panel.remove(&native_channel_copy);
    native_panel.append(&native_channel_copy);

    let web_panel_host = treatment_builder
        .object::<gtk::Box>("web_panel_host")
        .expect("ToniatorEditorControls.ui must define web_panel_host");
    let web_shared = treatment_builder
        .object::<gtk::CheckButton>("web_shared")
        .expect("ToniatorEditorControls.ui must define web_shared");
    let web_shared_help = help_for("Share Mark Shape Across Inks").map(help_handle);
    if let Some(help) = &web_shared_help {
        treatment_builder
            .object::<gtk::Box>("web_shared_help_host")
            .expect("ToniatorEditorControls.ui must define web_shared_help_host")
            .append(&help.button);
    }

    let web_shape = treatment_builder
        .object::<gtk::DropDown>("web_shape")
        .expect("ToniatorEditorControls.ui must define web_shape");
    let web_shape_label = treatment_builder
        .object::<gtk::Label>("web_shape_label")
        .expect("ToniatorEditorControls.ui must define web_shape_label");
    configure_dropdown_accessibility(&web_shape, &web_shape_label, "Mark");
    sync_dropdown_strings(&web_shape, &["Circle", "Regular Polygon", "User Defined"]);
    let web_shape_row = treatment_builder
        .object::<gtk::Box>("web_shape_row")
        .expect("ToniatorEditorControls.ui must define web_shape_row")
        .upcast();
    let web_mixed_shape_label = treatment_builder
        .object::<gtk::Label>("web_mixed_shape_label")
        .expect("ToniatorEditorControls.ui must define web_mixed_shape_label");
    let web_mixed_shape_apply = treatment_builder
        .object::<gtk::DropDown>("web_mixed_shape_apply")
        .expect("ToniatorEditorControls.ui must define web_mixed_shape_apply");
    let web_mixed_shape_apply_label = treatment_builder
        .object::<gtk::Label>("web_mixed_shape_apply_label")
        .expect("ToniatorEditorControls.ui must define web_mixed_shape_apply_label");
    configure_dropdown_accessibility(
        &web_mixed_shape_apply,
        &web_mixed_shape_apply_label,
        "Apply Mark to All",
    );
    sync_dropdown_strings(
        &web_mixed_shape_apply,
        &[
            "Choose a mark…",
            "Circle",
            "Regular Polygon",
            "User Defined",
        ],
    );
    let web_mixed_shape_apply_row = treatment_builder
        .object::<gtk::Box>("web_mixed_shape_apply_row")
        .expect("ToniatorEditorControls.ui must define web_mixed_shape_apply_row")
        .upcast();
    let web_polygon_sides = treatment_builder
        .object::<gtk::SpinButton>("web_polygon_sides")
        .expect("ToniatorEditorControls.ui must define web_polygon_sides");
    web_polygon_sides.set_range(3.0, 6.0);
    web_polygon_sides.set_increments(1.0, 1.0);
    web_polygon_sides.set_value(4.0);
    disable_pointer_scroll_adjustment(&web_polygon_sides);
    let web_polygon_sides_label = treatment_builder
        .object::<gtk::Label>("web_polygon_sides_label")
        .expect("ToniatorEditorControls.ui must define web_polygon_sides_label");
    let web_polygon_sides_row: gtk::Widget = treatment_builder
        .object::<gtk::Box>("web_polygon_sides_row")
        .expect("ToniatorEditorControls.ui must define web_polygon_sides_row")
        .upcast();
    web_polygon_sides.update_property(&[gtk::accessible::Property::Description(
        help_for("Polygon Sides (3–6)").unwrap().summary,
    )]);
    web_polygon_sides.update_relation(&[gtk::accessible::Relation::LabelledBy(&[
        web_polygon_sides_label.upcast_ref(),
    ])]);
    treatment_builder
        .object::<gtk::Box>("web_polygon_sides_help_host")
        .expect("ToniatorEditorControls.ui must define web_polygon_sides_help_host")
        .append(&help_button(help_for("Polygon Sides (3–6)").unwrap()));
    let web_edit_shape = treatment_builder
        .object::<gtk::Button>("web_edit_shape")
        .expect("ToniatorEditorControls.ui must define web_edit_shape");
    if let Some(spec) = help_for("Edit User-Defined Mark") {
        web_edit_shape.set_tooltip_text(Some(spec.summary));
        web_edit_shape.update_property(&[gtk::accessible::Property::Description(spec.summary)]);
        treatment_builder
            .object::<gtk::Box>("web_edit_shape_help_host")
            .expect("ToniatorEditorControls.ui must define web_edit_shape_help_host")
            .append(&help_button(spec));
    }
    let web_geometry_note = treatment_builder
        .object::<gtk::Label>("web_geometry_note")
        .expect("ToniatorEditorControls.ui must define web_geometry_note");
    let web_target = treatment_builder
        .object::<gtk::DropDown>("web_target")
        .expect("ToniatorEditorControls.ui must define web_target");
    let web_target_label = treatment_builder
        .object::<gtk::Label>("web_target_label")
        .expect("ToniatorEditorControls.ui must define web_target_label");
    configure_dropdown_accessibility(&web_target, &web_target_label, "Adjust Ink");
    sync_dropdown_strings(
        &web_target,
        &["All Inks", "Cyan", "Magenta", "Yellow", "Black"],
    );
    let web_target_row: gtk::Widget = treatment_builder
        .object::<gtk::Box>("web_target_row")
        .expect("ToniatorEditorControls.ui must define web_target_row")
        .upcast();
    web_target_row.set_visible(false);
    let web_target_help = help_for("Adjust Ink").map(|spec| {
        let handle = help_handle(spec);
        treatment_builder
            .object::<gtk::Box>("web_target_help_host")
            .expect("ToniatorEditorControls.ui must define web_target_help_host")
            .append(&handle.button);
        handle
    });
    let web_visible_label = treatment_builder
        .object::<gtk::Label>("web_visible_label")
        .expect("ToniatorEditorControls.ui must define web_visible_label");
    let web_visible = [
        treatment_builder
            .object::<gtk::CheckButton>("web_visible_c")
            .expect("ToniatorEditorControls.ui must define web_visible_c"),
        treatment_builder
            .object::<gtk::CheckButton>("web_visible_m")
            .expect("ToniatorEditorControls.ui must define web_visible_m"),
        treatment_builder
            .object::<gtk::CheckButton>("web_visible_y")
            .expect("ToniatorEditorControls.ui must define web_visible_y"),
        treatment_builder
            .object::<gtk::CheckButton>("web_visible_k")
            .expect("ToniatorEditorControls.ui must define web_visible_k"),
    ];
    for button in &web_visible {
        button.set_tooltip_text(Some("Toggle this ink in the output"));
    }
    let web_visible_help = help_handle(help_for("Visible Inks").unwrap());
    treatment_builder
        .object::<gtk::Box>("web_visible_help_host")
        .expect("ToniatorEditorControls.ui must define web_visible_help_host")
        .append(&web_visible_help.button);
    let web_mixed = treatment_builder
        .object::<gtk::Label>("web_mixed")
        .expect("ToniatorEditorControls.ui must define web_mixed");
    let web_color = treatment_builder
        .object::<gtk::Entry>("web_color")
        .expect("ToniatorEditorControls.ui must define web_color");
    web_color.set_tooltip_text(Some("Hex ink color; valid colors apply automatically"));
    let web_color_row = treatment_builder
        .object::<gtk::Box>("web_color_row")
        .expect("ToniatorEditorControls.ui must define web_color_row")
        .upcast();
    let web_color_heading = treatment_builder
        .object::<gtk::Label>("web_color_heading")
        .expect("ToniatorEditorControls.ui must define web_color_heading");
    let web_color_status = treatment_builder
        .object::<gtk::Label>("web_color_status")
        .expect("ToniatorEditorControls.ui must define web_color_status");
    let web_color_help = help_handle(help_for("Ink Color").unwrap());
    treatment_builder
        .object::<gtk::Box>("web_color_help_host")
        .expect("ToniatorEditorControls.ui must define web_color_help_host")
        .append(&web_color_help.button);
    web_color.update_relation(&[gtk::accessible::Relation::LabelledBy(&[
        web_color_heading.upcast_ref()
    ])]);
    let web_crosshatch_color = treatment_builder
        .object::<gtk::Entry>("web_crosshatch_color")
        .expect("ToniatorEditorControls.ui must define web_crosshatch_color");
    web_crosshatch_color
        .set_tooltip_text(Some("One monochrome color used by every crosshatch layer"));
    let web_crosshatch_color_row = treatment_builder
        .object::<gtk::Box>("web_crosshatch_color_row")
        .expect("ToniatorEditorControls.ui must define web_crosshatch_color_row")
        .upcast();

    let web_coverage = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "web_coverage_scale",
            control_id: "web_coverage_control",
            label_id: "web_coverage_label",
            accessible_name: "Coverage",
            minimum: 0.0,
            maximum: 5.0,
            step: 0.05,
        },
    );
    let web_coverage_status = treatment_builder
        .object::<gtk::Label>("web_coverage_status")
        .expect("ToniatorEditorControls.ui must define web_coverage_status");
    let web_angle = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "web_angle_scale",
            control_id: "web_angle_control",
            label_id: "web_angle_label",
            accessible_name: "Screen Angle",
            minimum: -360.0,
            maximum: 360.0,
            step: 1.0,
        },
    );
    let web_angle_status = treatment_builder
        .object::<gtk::Label>("web_angle_status")
        .expect("ToniatorEditorControls.ui must define web_angle_status");
    let web_mark_angle = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "web_mark_angle_scale",
            control_id: "web_mark_angle_control",
            label_id: "web_mark_angle_label",
            accessible_name: "Mark Rotation",
            minimum: -180.0,
            maximum: 180.0,
            step: 1.0,
        },
    );
    let web_mark_angle_status = treatment_builder
        .object::<gtk::Label>("web_mark_angle_status")
        .expect("ToniatorEditorControls.ui must define web_mark_angle_status");
    let web_width_scale = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "web_width_scale",
            control_id: "web_width_scale_control",
            label_id: "web_width_scale_label",
            accessible_name: "Mark Width",
            minimum: 0.1,
            maximum: 4.0,
            step: 0.05,
        },
    );
    let web_width_scale_status = treatment_builder
        .object::<gtk::Label>("web_width_scale_status")
        .expect("ToniatorEditorControls.ui must define web_width_scale_status");
    let web_height_scale = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "web_height_scale",
            control_id: "web_height_scale_control",
            label_id: "web_height_scale_label",
            accessible_name: "Mark Height",
            minimum: 0.1,
            maximum: 4.0,
            step: 0.05,
        },
    );
    let web_height_scale_status = treatment_builder
        .object::<gtk::Label>("web_height_scale_status")
        .expect("ToniatorEditorControls.ui must define web_height_scale_status");
    let web_threshold = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "web_threshold_scale",
            control_id: "web_threshold_control",
            label_id: "web_threshold_label",
            accessible_name: "Light-Tone Cutoff",
            minimum: 0.0,
            maximum: 1.0,
            step: 0.01,
        },
    );
    let web_threshold_status = treatment_builder
        .object::<gtk::Label>("web_threshold_status")
        .expect("ToniatorEditorControls.ui must define web_threshold_status");
    let web_opacity = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "web_opacity_scale",
            control_id: "web_opacity_control",
            label_id: "web_opacity_label",
            accessible_name: "Ink Opacity",
            minimum: 0.0,
            maximum: 1.0,
            step: 0.01,
        },
    );
    let web_opacity_heading = treatment_builder
        .object::<gtk::Label>("web_opacity_label")
        .expect("ToniatorEditorControls.ui must define web_opacity_label");
    let web_opacity_status = treatment_builder
        .object::<gtk::Label>("web_opacity_status")
        .expect("ToniatorEditorControls.ui must define web_opacity_status");
    let web_opacity_help = help_handle(help_for("Ink Opacity").unwrap());
    treatment_builder
        .object::<gtk::Box>("web_opacity_help_host")
        .expect("ToniatorEditorControls.ui must define web_opacity_help_host")
        .append(&web_opacity_help.button);
    let web_detail = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "web_detail_scale",
            control_id: "web_detail_control",
            label_id: "web_detail_label",
            accessible_name: "Sampling Detail",
            minimum: 0.1,
            maximum: 8.0,
            step: 0.1,
        },
    );
    let web_detail_status = treatment_builder
        .object::<gtk::Label>("web_detail_status")
        .expect("ToniatorEditorControls.ui must define web_detail_status");

    let curve_panel_host = treatment_builder
        .object::<gtk::Box>("curve_panel_host")
        .expect("ToniatorEditorControls.ui must define curve_panel_host");
    let curve_layout = treatment_builder
        .object::<gtk::DropDown>("curve_layout")
        .expect("ToniatorEditorControls.ui must define curve_layout");
    let curve_layout_label = treatment_builder
        .object::<gtk::Label>("curve_layout_label")
        .expect("ToniatorEditorControls.ui must define curve_layout_label");
    configure_dropdown_accessibility(&curve_layout, &curve_layout_label, "Layout");
    sync_dropdown_strings(&curve_layout, &["Across Artwork", "Repeated Motif"]);
    let curve_weight = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "curve_weight_scale",
            control_id: "curve_weight_control",
            label_id: "curve_weight_label",
            accessible_name: "Line Weight",
            minimum: 1.0,
            maximum: 200.0,
            step: 1.0,
        },
    );
    let curve_spacing = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "curve_spacing_scale",
            control_id: "curve_spacing_control",
            label_id: "curve_spacing_label",
            accessible_name: "Line Spacing",
            minimum: 8.0,
            maximum: 220.0,
            step: 1.0,
        },
    );
    let curve_profile = treatment_builder
        .object::<gtk::DropDown>("curve_profile")
        .expect("ToniatorEditorControls.ui must define curve_profile");
    let curve_profile_label = treatment_builder
        .object::<gtk::Label>("curve_profile_label")
        .expect("ToniatorEditorControls.ui must define curve_profile_label");
    configure_dropdown_accessibility(&curve_profile, &curve_profile_label, "Line Shape");
    sync_dropdown_strings(
        &curve_profile,
        &[
            "Straight",
            "Soft Wave",
            "Deep Wave",
            "Custom",
            "Mixed — Select One Ink",
        ],
    );
    let curve_editor_label = treatment_builder
        .object::<gtk::Label>("curve_editor_label")
        .expect("ToniatorEditorControls.ui must define curve_editor_label");
    let curve_editor = treatment_builder
        .object::<gtk::DrawingArea>("curve_editor")
        .expect("toniator-window.blp must define curve_editor");
    if let Some(spec) = help_for("Curve Editor") {
        curve_editor.set_tooltip_text(Some(spec.summary));
        curve_editor.update_property(&[gtk::accessible::Property::Description(spec.summary)]);
    }
    let curve_reset = treatment_builder
        .object::<gtk::Button>("curve_reset")
        .expect("ToniatorEditorControls.ui must define curve_reset");
    if let Some(spec) = help_for("Reset Line") {
        curve_reset.set_tooltip_text(Some(spec.summary));
        curve_reset.update_property(&[gtk::accessible::Property::Description(spec.summary)]);
    }
    let curve_shared = treatment_builder
        .object::<gtk::CheckButton>("curve_shared")
        .expect("ToniatorEditorControls.ui must define curve_shared");
    let curve_shared_help = help_for("Share Line Shape Across Inks").map(help_handle);
    if let Some(help) = &curve_shared_help {
        treatment_builder
            .object::<gtk::Box>("curve_shared_help_host")
            .expect("ToniatorEditorControls.ui must define curve_shared_help_host")
            .append(&help.button);
    }
    let curve_target = treatment_builder
        .object::<gtk::DropDown>("curve_target")
        .expect("ToniatorEditorControls.ui must define curve_target");
    let curve_target_label = treatment_builder
        .object::<gtk::Label>("curve_target_label")
        .expect("ToniatorEditorControls.ui must define curve_target_label");
    configure_dropdown_accessibility(&curve_target, &curve_target_label, "Adjust Ink");
    sync_dropdown_strings(
        &curve_target,
        &["All Inks", "Cyan", "Magenta", "Yellow", "Black"],
    );
    let curve_target_row: gtk::Widget = treatment_builder
        .object::<gtk::Box>("curve_target_row")
        .expect("ToniatorEditorControls.ui must define curve_target_row")
        .upcast();
    curve_target_row.set_visible(false);
    let curve_target_help = help_for("Adjust Ink").map(|spec| {
        let handle = help_handle(spec);
        treatment_builder
            .object::<gtk::Box>("curve_target_help_host")
            .expect("ToniatorEditorControls.ui must define curve_target_help_host")
            .append(&handle.button);
        handle
    });
    let motif_controls: gtk::Widget = treatment_builder
        .object::<gtk::Box>("motif_controls")
        .expect("ToniatorEditorControls.ui must define motif_controls")
        .upcast();
    motif_controls.set_visible(false);
    let motif_coverage = treatment_builder
        .object::<gtk::DropDown>("motif_coverage")
        .expect("ToniatorEditorControls.ui must define motif_coverage");
    let motif_coverage_label = treatment_builder
        .object::<gtk::Label>("motif_coverage_label")
        .expect("ToniatorEditorControls.ui must define motif_coverage_label");
    configure_dropdown_accessibility(&motif_coverage, &motif_coverage_label, "Artwork Coverage");
    sync_dropdown_strings(
        &motif_coverage,
        &["Cover Artwork Automatically", "Set Rows and Columns"],
    );
    let motif_size = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "motif_size_scale",
            control_id: "motif_size_control",
            label_id: "motif_size_label",
            accessible_name: "Motif Size",
            minimum: 4.0,
            maximum: 200.0,
            step: 1.0,
        },
    );
    let motif_columns = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "motif_columns_scale",
            control_id: "motif_columns_control",
            label_id: "motif_columns_label",
            accessible_name: "Columns",
            minimum: 1.0,
            maximum: 40.0,
            step: 1.0,
        },
    );
    motif_columns.set_digits(0);
    let motif_rows = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "motif_rows_scale",
            control_id: "motif_rows_control",
            label_id: "motif_rows_label",
            accessible_name: "Rows",
            minimum: 1.0,
            maximum: 80.0,
            step: 1.0,
        },
    );
    motif_rows.set_digits(0);
    let motif_row_spacing = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "motif_row_spacing_scale",
            control_id: "motif_row_spacing_control",
            label_id: "motif_row_spacing_label",
            accessible_name: "Row Spacing",
            minimum: 1.0,
            maximum: 160.0,
            step: 1.0,
        },
    );
    let motif_stagger = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "motif_stagger_scale",
            control_id: "motif_stagger_control",
            label_id: "motif_stagger_label",
            accessible_name: "Alternate Row Offset",
            minimum: -200.0,
            maximum: 200.0,
            step: 1.0,
        },
    );
    let motif_alternate = treatment_builder
        .object::<gtk::DropDown>("motif_alternate")
        .expect("ToniatorEditorControls.ui must define motif_alternate");
    let motif_alternate_label = treatment_builder
        .object::<gtk::Label>("motif_alternate_label")
        .expect("ToniatorEditorControls.ui must define motif_alternate_label");
    configure_dropdown_accessibility(&motif_alternate, &motif_alternate_label, "Alternate Copies");
    sync_dropdown_strings(&motif_alternate, &["None", "Mirror", "Half Turn"]);
    let motif_arrange = treatment_builder
        .object::<gtk::CheckButton>("motif_arrange")
        .expect("ToniatorEditorControls.ui must define motif_arrange");
    motif_arrange.set_tooltip_text(Some(
        "Drag the center, rotation, and spacing handles on the artwork",
    ));
    let motif_mixed = treatment_builder
        .object::<gtk::Label>("motif_mixed")
        .expect("ToniatorEditorControls.ui must define motif_mixed");

    let curve_visible_label = treatment_builder
        .object::<gtk::Label>("curve_visible_label")
        .expect("ToniatorEditorControls.ui must define curve_visible_label");
    let curve_visible = [
        treatment_builder
            .object::<gtk::CheckButton>("curve_visible_c")
            .expect("ToniatorEditorControls.ui must define curve_visible_c"),
        treatment_builder
            .object::<gtk::CheckButton>("curve_visible_m")
            .expect("ToniatorEditorControls.ui must define curve_visible_m"),
        treatment_builder
            .object::<gtk::CheckButton>("curve_visible_y")
            .expect("ToniatorEditorControls.ui must define curve_visible_y"),
        treatment_builder
            .object::<gtk::CheckButton>("curve_visible_k")
            .expect("ToniatorEditorControls.ui must define curve_visible_k"),
    ];
    for button in &curve_visible {
        button.set_tooltip_text(Some("Toggle this ink in the output"));
    }
    let curve_visible_help = help_handle(help_for("Visible Inks").unwrap());
    treatment_builder
        .object::<gtk::Box>("curve_visible_help_host")
        .expect("ToniatorEditorControls.ui must define curve_visible_help_host")
        .append(&curve_visible_help.button);
    let curve_mixed = treatment_builder
        .object::<gtk::Label>("curve_mixed")
        .expect("ToniatorEditorControls.ui must define curve_mixed");
    let curve_color = treatment_builder
        .object::<gtk::Entry>("curve_color")
        .expect("ToniatorEditorControls.ui must define curve_color");
    curve_color.set_tooltip_text(Some("Hex ink color; valid colors apply automatically"));
    let curve_color_row = treatment_builder
        .object::<gtk::Box>("curve_color_row")
        .expect("ToniatorEditorControls.ui must define curve_color_row")
        .upcast();
    let curve_color_status = treatment_builder
        .object::<gtk::Label>("curve_color_status")
        .expect("ToniatorEditorControls.ui must define curve_color_status");
    let curve_crosshatch_color = treatment_builder
        .object::<gtk::Entry>("curve_crosshatch_color")
        .expect("ToniatorEditorControls.ui must define curve_crosshatch_color");
    curve_crosshatch_color
        .set_tooltip_text(Some("One monochrome color used by every crosshatch layer"));
    let curve_crosshatch_color_row = treatment_builder
        .object::<gtk::Box>("curve_crosshatch_color_row")
        .expect("ToniatorEditorControls.ui must define curve_crosshatch_color_row")
        .upcast();

    let curve_coverage = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "curve_coverage_scale",
            control_id: "curve_coverage_control",
            label_id: "curve_coverage_label",
            accessible_name: "Line Coverage",
            minimum: 0.0,
            maximum: 5.0,
            step: 0.05,
        },
    );
    let curve_coverage_status = treatment_builder
        .object::<gtk::Label>("curve_coverage_status")
        .expect("ToniatorEditorControls.ui must define curve_coverage_status");
    let curve_angle = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "curve_angle_scale",
            control_id: "curve_angle_control",
            label_id: "curve_angle_label",
            accessible_name: "Screen Angle",
            minimum: -360.0,
            maximum: 360.0,
            step: 1.0,
        },
    );
    let curve_angle_status = treatment_builder
        .object::<gtk::Label>("curve_angle_status")
        .expect("ToniatorEditorControls.ui must define curve_angle_status");
    let curve_position_x = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "curve_position_x_scale",
            control_id: "curve_position_x_control",
            label_id: "curve_position_x_label",
            accessible_name: "Position X",
            minimum: -1000.0,
            maximum: 1000.0,
            step: 1.0,
        },
    );
    let curve_position_x_status = treatment_builder
        .object::<gtk::Label>("curve_position_x_status")
        .expect("ToniatorEditorControls.ui must define curve_position_x_status");
    let curve_position_y = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "curve_position_y_scale",
            control_id: "curve_position_y_control",
            label_id: "curve_position_y_label",
            accessible_name: "Position Y",
            minimum: -1000.0,
            maximum: 1000.0,
            step: 1.0,
        },
    );
    let curve_position_y_status = treatment_builder
        .object::<gtk::Label>("curve_position_y_status")
        .expect("ToniatorEditorControls.ui must define curve_position_y_status");
    let curve_opacity = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "curve_opacity_scale",
            control_id: "curve_opacity_control",
            label_id: "curve_opacity_label",
            accessible_name: "Ink Opacity",
            minimum: 0.0,
            maximum: 1.0,
            step: 0.01,
        },
    );
    let curve_opacity_status = treatment_builder
        .object::<gtk::Label>("curve_opacity_status")
        .expect("ToniatorEditorControls.ui must define curve_opacity_status");
    let curve_threshold = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "curve_threshold_scale",
            control_id: "curve_threshold_control",
            label_id: "curve_threshold_label",
            accessible_name: "Light-Tone Cutoff",
            minimum: 0.0,
            maximum: 1.0,
            step: 0.01,
        },
    );
    let curve_threshold_status = treatment_builder
        .object::<gtk::Label>("curve_threshold_status")
        .expect("ToniatorEditorControls.ui must define curve_threshold_status");
    let curve_detail = builder_control_scale(
        &treatment_builder,
        BuilderScaleSpec {
            scale_id: "curve_detail_scale",
            control_id: "curve_detail_control",
            label_id: "curve_detail_label",
            accessible_name: "Sampling Detail",
            minimum: 0.1,
            maximum: 8.0,
            step: 0.1,
        },
    );
    let curve_detail_status = treatment_builder
        .object::<gtk::Label>("curve_detail_status")
        .expect("ToniatorEditorControls.ui must define curve_detail_status");
    let curve_close_ends = treatment_builder
        .object::<gtk::CheckButton>("curve_close_ends")
        .expect("ToniatorEditorControls.ui must define curve_close_ends");
    let curve_smooth_join = treatment_builder
        .object::<gtk::CheckButton>("curve_smooth_join")
        .expect("ToniatorEditorControls.ui must define curve_smooth_join");

    let weighted_voronoi_panel = treatment_builder
        .object::<gtk::Box>("weighted_voronoi_panel")
        .expect("toniator-window.blp must define weighted_voronoi_panel");
    let weighted_voronoi_channel = treatment_builder
        .object::<gtk::DropDown>("weighted_voronoi_channel")
        .expect("toniator-window.blp must define weighted_voronoi_channel");
    let weighted_voronoi_channel_label = treatment_builder
        .object::<gtk::Label>("weighted_voronoi_channel_label")
        .expect("toniator-window.blp must define weighted_voronoi_channel_label");
    configure_dropdown_accessibility(
        &weighted_voronoi_channel,
        &weighted_voronoi_channel_label,
        "Edit Channel",
    );
    let weighted_voronoi_cell_count = treatment_builder
        .object::<gtk::Scale>("weighted_voronoi_cell_count")
        .expect("toniator-window.blp must define weighted_voronoi_cell_count");
    weighted_voronoi_cell_count.set_range(2.0, DistributionLimits::default().max_sites as f64);
    weighted_voronoi_cell_count.set_increments(1.0, 32.0);
    weighted_voronoi_cell_count.set_digits(0);
    let weighted_voronoi_visible = [
        treatment_builder
            .object::<gtk::CheckButton>("weighted_voronoi_visible_0")
            .expect("toniator-window.blp must define weighted_voronoi_visible_0"),
        treatment_builder
            .object::<gtk::CheckButton>("weighted_voronoi_visible_1")
            .expect("toniator-window.blp must define weighted_voronoi_visible_1"),
        treatment_builder
            .object::<gtk::CheckButton>("weighted_voronoi_visible_2")
            .expect("toniator-window.blp must define weighted_voronoi_visible_2"),
        treatment_builder
            .object::<gtk::CheckButton>("weighted_voronoi_visible_3")
            .expect("toniator-window.blp must define weighted_voronoi_visible_3"),
    ];
    let weighted_voronoi_arrangement = treatment_builder
        .object::<gtk::DropDown>("weighted_voronoi_arrangement")
        .expect("toniator-window.blp must define weighted_voronoi_arrangement");
    let weighted_voronoi_arrangement_label = treatment_builder
        .object::<gtk::Label>("weighted_voronoi_arrangement_label")
        .expect("toniator-window.blp must define weighted_voronoi_arrangement_label");
    configure_dropdown_accessibility(
        &weighted_voronoi_arrangement,
        &weighted_voronoi_arrangement_label,
        "Arrangement",
    );
    sync_dropdown_strings(&weighted_voronoi_arrangement, &["Shared", "Independent"]);
    let weighted_voronoi_placement = treatment_builder
        .object::<gtk::DropDown>("weighted_voronoi_placement")
        .expect("toniator-window.blp must define weighted_voronoi_placement");
    let weighted_voronoi_placement_label = treatment_builder
        .object::<gtk::Label>("weighted_voronoi_placement_label")
        .expect("toniator-window.blp must define weighted_voronoi_placement_label");
    configure_dropdown_accessibility(
        &weighted_voronoi_placement,
        &weighted_voronoi_placement_label,
        "Placement",
    );
    sync_dropdown_strings(&weighted_voronoi_placement, &["Source Weighted", "Uniform"]);
    let weighted_voronoi_density_strength = treatment_builder
        .object::<gtk::Scale>("weighted_voronoi_density_strength")
        .expect("toniator-window.blp must define weighted_voronoi_density_strength");
    weighted_voronoi_density_strength.set_range(0.001, 16.0);
    weighted_voronoi_density_strength.set_increments(0.05, 0.5);
    weighted_voronoi_density_strength.set_digits(3);
    let weighted_voronoi_response_strength = treatment_builder
        .object::<gtk::Scale>("weighted_voronoi_response_strength")
        .expect("toniator-window.blp must define weighted_voronoi_response_strength");
    weighted_voronoi_response_strength.set_range(0.0, 16.0);
    weighted_voronoi_response_strength.set_increments(0.05, 0.5);
    weighted_voronoi_response_strength.set_digits(2);
    let weighted_voronoi_boundary_gap = treatment_builder
        .object::<gtk::Scale>("weighted_voronoi_boundary_gap")
        .expect("toniator-window.blp must define weighted_voronoi_boundary_gap");
    weighted_voronoi_boundary_gap.set_range(0.0, 64.0);
    weighted_voronoi_boundary_gap.set_increments(0.25, 2.0);
    weighted_voronoi_boundary_gap.set_digits(2);
    let weighted_voronoi_seed = treatment_builder
        .object::<gtk::Entry>("weighted_voronoi_seed")
        .expect("toniator-window.blp must define weighted_voronoi_seed");
    weighted_voronoi_seed.set_tooltip_text(Some("Enter an exact deterministic seed"));

    let treatment_modes = treatment_builder
        .object::<gtk::Stack>("treatment_modes")
        .expect("ToniatorEditorControls.ui must define treatment_modes");
    treatment_modes.page(&native_panel).set_name("native");
    treatment_modes.page(&web_panel_host).set_name("web");
    treatment_modes.page(&curve_panel_host).set_name("curve");
    treatment_modes
        .page(&weighted_voronoi_panel)
        .set_name("weighted-voronoi");
    treatment_modes.set_visible_child_name("native");
    let hierarchy = build_inspector_hierarchy(builder);
    let controls_builder = builder;
    let artwork_source = controls_builder
        .object::<gtk::DropDown>("artwork_source")
        .expect("ToniatorEditorControls.ui must define artwork_source");
    let artwork_source_label = controls_builder
        .object::<gtk::Label>("artwork_source_label")
        .expect("ToniatorEditorControls.ui must define artwork_source_label");
    configure_dropdown_accessibility(&artwork_source, &artwork_source_label, "Artwork Source");
    sync_dropdown_strings(&artwork_source, &artwork_source_labels(false));
    let artwork_source_note = controls_builder
        .object::<gtk::Label>("artwork_source_note")
        .expect("ToniatorEditorControls.ui must define artwork_source_note");
    let source_alpha = controls_builder
        .object::<gtk::DropDown>("source_alpha")
        .expect("ToniatorEditorControls.ui must define source_alpha");
    let source_alpha_label = controls_builder
        .object::<gtk::Label>("source_alpha_label")
        .expect("ToniatorEditorControls.ui must define source_alpha_label");
    configure_dropdown_accessibility(&source_alpha, &source_alpha_label, "Source Alpha");
    sync_dropdown_strings(&source_alpha, &source_alpha_labels(false));
    let source_alpha_row: gtk::Widget = controls_builder
        .object::<gtk::Box>("source_alpha_row")
        .expect("ToniatorEditorControls.ui must define source_alpha_row")
        .upcast();
    let source_alpha_note = controls_builder
        .object::<gtk::Label>("source_alpha_note")
        .expect("ToniatorEditorControls.ui must define source_alpha_note");
    source_alpha_note.set_visible(false);
    let output_mode = controls_builder
        .object::<gtk::DropDown>("output_mode")
        .expect("ToniatorEditorControls.ui must define output_mode");
    let output_mode_label = controls_builder
        .object::<gtk::Label>("output_mode_label")
        .expect("ToniatorEditorControls.ui must define output_mode_label");
    configure_dropdown_accessibility(&output_mode, &output_mode_label, "Output Model");
    sync_dropdown_strings(&output_mode, &["CMYK Print", "RGB Screen"]);
    output_mode.set_tooltip_text(Some("Choose subtractive CMYK inks for print or additive RGB screens for transparent, light-based output."));
    let channel_assignment = controls_builder
        .object::<gtk::DropDown>("channel_assignment")
        .expect("ToniatorEditorControls.ui must define channel_assignment");
    let channel_assignment_label = controls_builder
        .object::<gtk::Label>("channel_assignment_label")
        .expect("ToniatorEditorControls.ui must define channel_assignment_label");
    configure_dropdown_accessibility(
        &channel_assignment,
        &channel_assignment_label,
        "Channel Assignment",
    );
    sync_dropdown_strings(
        &channel_assignment,
        &channel_assignment_labels(true, OutputModel::CmykPrint),
    );
    let channel_assignment_note = controls_builder
        .object::<gtk::Label>("channel_assignment_note")
        .expect("ToniatorEditorControls.ui must define channel_assignment_note");
    let active_channel = controls_builder
        .object::<gtk::DropDown>("active_channel")
        .expect("ToniatorEditorControls.ui must define active_channel");
    let active_channel_label = controls_builder
        .object::<gtk::Label>("active_channel_label")
        .expect("ToniatorEditorControls.ui must define active_channel_label");
    configure_dropdown_accessibility(&active_channel, &active_channel_label, "Active Channel");
    sync_dropdown_strings(
        &active_channel,
        &output_channel_labels(OutputModel::CmykPrint),
    );
    let active_channel_row: gtk::Widget = controls_builder
        .object::<gtk::Box>("active_channel_row")
        .expect("ToniatorEditorControls.ui must define active_channel_row")
        .upcast();
    active_channel_row.set_visible(false);
    let crosshatch_action = controls_builder
        .object::<gtk::Button>("crosshatch_action")
        .expect("ToniatorEditorControls.ui must define crosshatch_action");
    crosshatch_action.set_tooltip_text(Some(
        "Temporarily use the legacy brightness crosshatch treatment with the current output model.",
    ));
    let crosshatch_note = controls_builder
        .object::<gtk::Label>("crosshatch_note")
        .expect("ToniatorEditorControls.ui must define crosshatch_note");
    let channel_scope = controls_builder
        .object::<gtk::DropDown>("channel_scope")
        .expect("toniator-window.blp must define channel_scope");
    channel_scope.set_tooltip_text(Some(
        "Choose which included inks or channels Shapes and Curves will edit.",
    ));
    channel_scope.update_property(&[gtk::accessible::Property::Label("Treatment Editing Scope")]);
    let aggregate_channel_controls = build_aggregate_channel_controls();
    hierarchy
        .channel_panel_stack
        .add_named(&aggregate_channel_controls.root, Some("aggregate"));
    let mut channel_controls = Vec::new();
    for channel in OutputChannelId::CMYK
        .into_iter()
        .chain(OutputChannelId::RGB)
    {
        let controls = build_channel_controls(channel);
        hierarchy
            .channel_panel_stack
            .add_named(&controls.root, Some(channel.stable_id()));
        channel_controls.push(controls);
    }
    hierarchy
        .channel_panel_stack
        .set_visible_child_name("aggregate");

    let appearance_controls = build_appearance_controls(builder);
    let preview_surface = appearance_controls.preview_surface.clone();
    let preview_color = appearance_controls.preview_color.clone();
    let export_background = appearance_controls.export_background.clone();
    let export_color_label = appearance_controls.export_color_label.clone();
    let export_color = appearance_controls.export_color.clone();
    let editing_context = controls_builder
        .object::<gtk::Label>("editing_context")
        .expect("toniator-window.blp must define editing_context");
    let editor_page = controls_builder
        .object::<gtk::Box>("editor_page")
        .expect("toniator-window.blp must define editor_page");
    #[cfg(test)]
    let inspector = controls_builder
        .object::<gtk::Box>("editor_controls")
        .expect("toniator-window.blp must define editor_controls");
    EditorWidgets {
        container: editor_page.upcast(),
        paned: layout,
        #[cfg(test)]
        inspector_root: inspector,
        workspace_status,
        cancel_preview,
        cancel_export,
        editing_context,
        canvas,
        canvas_content: canvas_overlay,
        fit,
        zoom_out,
        zoom,
        zoom_entry,
        zoom_in,
        detail,
        coverage,
        contrast,
        angle,
        dots,
        squares,
        lines,
        curves,
        weighted_voronoi: treatment_builder
            .object::<gtk::ToggleButton>("weighted_voronoi")
            .expect("toniator-window.blp must define weighted_voronoi"),
        legacy,
        treatment_modes,
        weighted_voronoi_channel,
        weighted_voronoi_cell_count,
        weighted_voronoi_visible,
        weighted_voronoi_arrangement,
        weighted_voronoi_placement,
        weighted_voronoi_density_strength,
        weighted_voronoi_response_strength,
        weighted_voronoi_boundary_gap,
        weighted_voronoi_seed,
        preset_import,
        preset_save,
        source_section: hierarchy.source_section,
        output_section: hierarchy.output_section,
        channel_settings_section: hierarchy.channel_settings_section,
        output_mode,
        artwork_source,
        artwork_source_note,
        source_alpha,
        source_alpha_row,
        source_alpha_note,
        channel_assignment,
        channel_assignment_note,
        active_channel,
        active_channel_row,
        channel_scope,
        channel_panel_stack: hierarchy.channel_panel_stack,
        channel_controls,
        aggregate_channel_controls,
        crosshatch_action,
        crosshatch_note,
        preview_surface,
        preview_color,
        export_background,
        export_color_label,
        export_color,
        web_shared,
        web_shared_help,
        web_shape,
        web_shape_row,
        web_mixed_shape_label,
        web_mixed_shape_apply,
        web_mixed_shape_apply_row,
        web_polygon_sides,
        web_polygon_sides_row,
        web_polygon_sides_label,
        web_edit_shape,
        web_target,
        web_target_label,
        web_target_help,
        web_visible_label,
        web_visible_help,
        web_visible,
        web_color,
        web_color_row,
        web_color_heading,
        web_color_help,
        web_crosshatch_color,
        web_crosshatch_color_row,
        web_color_status,
        web_coverage,
        web_coverage_status,
        web_angle,
        web_angle_status,
        web_mark_angle,
        web_mark_angle_status,
        web_width_scale,
        web_width_scale_status,
        web_height_scale,
        web_height_scale_status,
        web_threshold,
        web_threshold_status,
        web_opacity,
        web_opacity_heading,
        web_opacity_help,
        web_opacity_status,
        web_detail,
        web_detail_status,
        web_mixed,
        web_geometry_note,
        curve_layout,
        curve_profile,
        curve_editor_label,
        curve_editor,
        curve_reset,
        curve_shared,
        curve_shared_help,
        curve_target,
        curve_target_label,
        curve_target_help,
        curve_visible_label,
        curve_visible_help,
        curve_visible,
        curve_color,
        curve_color_row,
        curve_crosshatch_color,
        curve_crosshatch_color_row,
        curve_color_status,
        curve_weight,
        curve_spacing,
        curve_coverage,
        curve_coverage_status,
        curve_angle,
        curve_angle_status,
        curve_position_x,
        curve_position_x_status,
        curve_position_y,
        curve_position_y_status,
        curve_opacity,
        curve_opacity_status,
        curve_threshold,
        curve_threshold_status,
        curve_detail,
        curve_detail_status,
        curve_close_ends,
        curve_smooth_join,
        curve_mixed,
        motif_controls,
        motif_coverage,
        motif_size,
        motif_columns,
        motif_rows,
        motif_row_spacing,
        motif_stagger,
        motif_alternate,
        motif_arrange,
        motif_mixed,
        motif_overlay,
    }
}

#[derive(Clone, Copy)]
struct HelpSpec {
    control: &'static str,
    heading: &'static str,
    summary: &'static str,
    body: &'static str,
}

// Keep help copy declarative: it is used by the popover, tooltip, and tests so
// the concise and expanded explanations cannot silently drift apart.
const HELP_SPECS: &[HelpSpec] = &[
    HelpSpec {
        control: "Artwork Mapping",
        heading: "Artwork Mapping",
        summary: "Choose which artwork information drives the halftone.",
        body: "Choose whether color or brightness drives the halftone. The illustration below shows the result; this changes both preview and export.",
    },
    HelpSpec {
        control: "Output Channel",
        heading: "Output Channel",
        summary: "Choose the channel used for a one-channel result.",
        body: "Choose the ink or RGB channel for Brightness → One Ink. This changes both preview and export.",
    },
    HelpSpec {
        control: "Mark",
        heading: "Mark",
        summary: "Choose the shape repeated across the screen.",
        body: "Choose the mark drawn at each sampled position. Polygon and user-defined options reveal related controls; this changes both preview and export.",
    },
    HelpSpec {
        control: "Adjust Ink",
        heading: "Adjust Ink",
        summary: "Choose which ink receives the next adjustments.",
        body: "Choose All Inks for shared adjustments or one ink for a separation. In Crosshatch this becomes Adjust Layer; changes affect both preview and export.",
    },
    HelpSpec {
        control: "Adjust Layer",
        heading: "Adjust Layer",
        summary: "Choose which crosshatch layer receives the next adjustments.",
        body: "Choose All Layers for shared adjustments or one layer for a directional hatch. Changes affect both preview and export.",
    },
    HelpSpec {
        control: "Adjust Channel",
        heading: "Adjust Channel",
        summary: "Choose which RGB channel receives the next adjustments.",
        body: "Choose All Channels for shared adjustments or Red, Green, or Blue for one additive channel. Changes affect both preview and export.",
    },
    HelpSpec {
        control: "Screen Angle",
        heading: "Screen Angle",
        summary: "Rotate the sampling screen behind the marks.",
        body: "Increase or decrease to rotate the screen. Mark Rotation changes the mark itself instead; this changes both preview and export.",
    },
    HelpSpec {
        control: "Mark Rotation",
        heading: "Mark Rotation",
        summary: "Rotate marks without rotating their sampling screen.",
        body: "Increase or decrease to rotate each mark. Screen Angle rotates the grid instead; this changes both preview and export.",
    },
    HelpSpec {
        control: "Mark Width",
        heading: "Mark Width",
        summary: "Change mark width independently from height.",
        body: "Increase for wider marks or decrease for narrower marks. This changes both preview and export.",
    },
    HelpSpec {
        control: "Mark Height",
        heading: "Mark Height",
        summary: "Change mark height independently from width.",
        body: "Increase for taller marks or decrease for shorter marks. This changes both preview and export.",
    },
    HelpSpec {
        control: "Light-Tone Cutoff",
        heading: "Light-Tone Cutoff",
        summary: "Hide faint marks or line sections in lighter artwork.",
        body: "Increase to remove more faint marks or line sections, or decrease to keep them. This changes both preview and export.",
    },
    HelpSpec {
        control: "Sampling Detail",
        heading: "Sampling Detail",
        summary: "Control how densely Toniator samples artwork.",
        body: "Increase for finer sampling or decrease for a coarser pattern. This changes both preview and export.",
    },
    HelpSpec {
        control: "Line Weight",
        heading: "Line Weight",
        summary: "Change the thickness of every curve line.",
        body: "Increase for heavier lines or decrease for lighter lines. This changes both preview and export.",
    },
    HelpSpec {
        control: "Line Spacing",
        heading: "Line Spacing",
        summary: "Change the distance between curve lines.",
        body: "Increase for more space or decrease for a denser pattern. This changes both preview and export.",
    },
    HelpSpec {
        control: "Line Shape",
        heading: "Line Shape",
        summary: "Choose the curve profile used by the pattern.",
        body: "Choose a preset profile or Custom to edit points below. This changes both preview and export.",
    },
    HelpSpec {
        control: "Artwork Coverage",
        heading: "Artwork Coverage",
        summary: "Choose automatic coverage or a fixed motif grid.",
        body: "Cover Artwork Automatically fills the artwork; Set Rows and Columns reveals manual layout controls. This changes both preview and export.",
    },
    HelpSpec {
        control: "Ink Opacity",
        heading: "Ink Opacity",
        summary: "Change how solid each ink appears.",
        body: "Increase for more opaque ink or decrease for more transparency. This changes both preview and export.",
    },
    HelpSpec {
        control: "Channel Opacity",
        heading: "Channel Opacity",
        summary: "Change how solid each RGB channel appears.",
        body: "Increase for a more solid RGB channel or decrease for more transparency. This changes both preview and export.",
    },
    HelpSpec {
        control: "Ink Color",
        heading: "Ink Color",
        summary: "Set the displayed color for the selected ink.",
        body: "Enter a hex color such as #00AEEF. The color is applied when valid and changes both preview and export.",
    },
    HelpSpec {
        control: "Channel Color",
        heading: "Channel Color",
        summary: "Set the displayed color for the selected RGB channel.",
        body: "Enter a hex color such as #00AEEF. The color is applied when valid and changes both preview and export.",
    },
    HelpSpec {
        control: "Crosshatch Color",
        heading: "Crosshatch Color",
        summary: "Set the shared monochrome crosshatch color.",
        body: "Enter a hex color such as #111111. This applies to every crosshatch layer in preview and export.",
    },
    HelpSpec {
        control: "Layout",
        heading: "Layout",
        summary: "Choose continuous lines or a repeated motif.",
        body: "Across Artwork draws continuous lines; Repeated Motif reveals motif controls. This changes both preview and export.",
    },
    HelpSpec {
        control: "Motif Size",
        heading: "Motif Size",
        summary: "Change the width of each repeated motif.",
        body: "Increase for larger motifs or decrease for smaller motifs. This changes both preview and export.",
    },
    HelpSpec {
        control: "Rows",
        heading: "Rows",
        summary: "Set how many motif rows are drawn.",
        body: "Increase for more rows or decrease for fewer rows. Available with Set Rows and Columns; changes both preview and export.",
    },
    HelpSpec {
        control: "Columns",
        heading: "Columns",
        summary: "Set how many motif copies span the artwork.",
        body: "Increase for more copies or decrease for fewer copies. Available with Set Rows and Columns; changes both preview and export.",
    },
    HelpSpec {
        control: "Row Spacing",
        heading: "Row Spacing",
        summary: "Change the distance between motif rows.",
        body: "Increase for more space or decrease for a denser layout. This changes both preview and export.",
    },
    HelpSpec {
        control: "Alternate Row Offset",
        heading: "Alternate Row Offset",
        summary: "Offset every other motif row.",
        body: "Increase or decrease to shift alternating rows. This changes both preview and export.",
    },
    HelpSpec {
        control: "Alternate Copies",
        heading: "Alternate Copies",
        summary: "Mirror or rotate alternating motif copies.",
        body: "Choose an alternating transform for repeated motifs. This changes both preview and export.",
    },
    HelpSpec {
        control: "PNG Background",
        heading: "PNG Background",
        summary: "Use the saved Export Background or make a PNG-only override.",
        body: "Document Export Background uses the saved document setting, including its alpha; the export dialog summary shows its current None/transparent or color value. Transparent Override preserves artwork alpha; White Override flattens only this PNG export. Overrides never change the document or SVG.",
    },
    HelpSpec {
        control: "Preview Surface",
        heading: "Preview Surface",
        summary: "Choose the canvas-only backdrop for transparent artwork.",
        body: "Checkerboard exposes transparency. A color is composited over checkerboard for the preview canvas only and is never exported or sampled.",
    },
    HelpSpec {
        control: "Export Background",
        heading: "Export Background",
        summary: "Choose an optional saved background for SVG and default PNG export.",
        body: "None preserves transparent output. Choosing Color starts with opaque white; then set its color, including alpha. It is emitted as the SVG export background layer and used by default for PNG. It is compositing-only and does not change sampling or marks.",
    },
    HelpSpec {
        control: "PNG Size",
        heading: "PNG Size",
        summary: "Choose document, double, or custom PNG dimensions.",
        body: "Custom enables width and height, which stay linked to the artwork ratio. This affects PNG export only.",
    },
    HelpSpec {
        control: "Share Mark Shape Across Inks",
        heading: "Share Mark Shape Across Inks",
        summary: "Use one mark shape for every ink.",
        body: "Turn this on to edit one shared mark, or off to give inks independent marks. This changes both preview and export.",
    },
    HelpSpec {
        control: "Share Mark Shape Across Channels",
        heading: "Share Mark Shape Across Channels",
        summary: "Use one mark shape for every RGB channel.",
        body: "Turn this on to edit one shared mark shape, or off to give red, green, and blue independent shapes. This changes both preview and export.",
    },
    HelpSpec {
        control: "Share Line Shape Across Inks",
        heading: "Share Line Shape Across Inks",
        summary: "Use one line shape for every ink.",
        body: "Turn this on to edit one shared line, or off to give inks independent lines. In Crosshatch it shares the hatch path across layers; this changes preview and export.",
    },
    HelpSpec {
        control: "Share Line Shape Across Channels",
        heading: "Share Line Shape Across Channels",
        summary: "Use one line shape for every RGB channel.",
        body: "Turn this on to edit one shared line shape, or off to give red, green, and blue independent shapes. This changes both preview and export.",
    },
    HelpSpec {
        control: "Adjust Layout on Artwork",
        heading: "Adjust Layout on Artwork",
        summary: "Move and rotate the repeated motif directly on the artwork.",
        body: "Turn this on, then drag the handles on the artwork to move, rotate, or separate rows. This changes both preview and export.",
    },
    HelpSpec {
        control: "Close Ends",
        heading: "Close Ends",
        summary: "Join the start and end of a curve.",
        body: "Turn this on to close the curve into a loop. It is most useful for repeated motifs and changes both preview and export.",
    },
    HelpSpec {
        control: "Smooth Join",
        heading: "Smooth Join",
        summary: "Smooth the join where a closed curve meets itself.",
        body: "Turn this on after Close Ends to smooth the seam. It changes both preview and export.",
    },
    HelpSpec {
        control: "Coverage",
        heading: "Coverage",
        summary: "Change mark size and the inked area of the artwork.",
        body: "Increase for larger marks and more inked area; decrease for smaller marks. Combine with spacing or Sampling Detail to control density. This changes both preview and export.",
    },
    HelpSpec {
        control: "Line Coverage",
        heading: "Line Coverage",
        summary: "Control how strongly artwork tone changes line width and inked area.",
        body: "Increase for stronger tonal width changes and more inked line area; decrease for gentler changes. Line Weight sets base thickness. This changes both preview and export.",
    },
    HelpSpec {
        control: "Contrast",
        heading: "Contrast",
        summary: "Increase separation between lighter and darker artwork areas.",
        body: "Increase for stronger tonal separation or decrease for a gentler result. This changes both preview and export.",
    },
    HelpSpec {
        control: "Apply Mark to All",
        heading: "Apply Mark to All",
        summary: "Replace mixed selected marks with one choice.",
        body: "Choose a mark to apply it across the selected inks. This changes both preview and export.",
    },
    HelpSpec {
        control: "Polygon Sides (3–6)",
        heading: "Polygon Sides",
        summary: "Set the number of sides in a polygon mark.",
        body: "Increase for more sides or decrease for fewer. Available only for Regular Polygon marks; this changes both preview and export.",
    },
    HelpSpec {
        control: "Edit User-Defined Mark",
        heading: "Edit User-Defined Mark",
        summary: "Edit the custom mark's anchors and curves.",
        body: "Open the mark editor to move points and handles. Available only for User Defined marks; this changes both preview and export.",
    },
    HelpSpec {
        control: "Curve Editor",
        heading: "Curve Editor",
        summary: "Shape the line directly with anchors and handles.",
        body: "Drag points to shape the line, double-click to add a point, and Delete to remove one. This changes both preview and export.",
    },
    HelpSpec {
        control: "Reset Line",
        heading: "Reset Line",
        summary: "Restore the current line to a preset shape.",
        body: "Reset returns the selected line or hatch path to its default profile. This changes both preview and export.",
    },
    HelpSpec {
        control: "Visible Inks",
        heading: "Visible Inks",
        summary: "Include or exclude inks from the halftone.",
        body: "Turn an ink on to include it or off to hide it. This changes both preview and export.",
    },
    HelpSpec {
        control: "Visible Crosshatch Layers",
        heading: "Visible Crosshatch Layers",
        summary: "Include or exclude monochrome hatch directions.",
        body: "Turn a hatch layer on to include it or off to hide it. Screen Angle changes that layer's direction; this changes preview and export.",
    },
    HelpSpec {
        control: "Visible RGB Channels",
        heading: "Visible RGB Channels",
        summary: "Include or exclude red, green, and blue channels.",
        body: "Turn a channel on to include it or off to hide it from the additive screen. This changes both preview and export.",
    },
    HelpSpec {
        control: "Position X",
        heading: "Position X",
        summary: "Move the selected line horizontally.",
        body: "Increase to move right or decrease to move left. This changes both preview and export.",
    },
    HelpSpec {
        control: "Position Y",
        heading: "Position Y",
        summary: "Move the selected line vertically.",
        body: "Increase to move down or decrease to move up. This changes both preview and export.",
    },
    HelpSpec {
        control: "Load Preset",
        heading: "Load Preset",
        summary: "Apply a saved halftone setup to the current artwork.",
        body: "Loading a preset changes treatment controls but keeps the current artwork. It changes preview and later export.",
    },
    HelpSpec {
        control: "Save Preset",
        heading: "Save Preset",
        summary: "Save this halftone setup without the artwork.",
        body: "Save reusable settings for later artwork. This affects preset storage only; it does not export or change the current preview.",
    },
    HelpSpec {
        control: "Canvas Zoom",
        heading: "Canvas Zoom",
        summary: "Magnify the artwork view without changing output.",
        body: "Increase to zoom in or decrease to zoom out. This affects the canvas preview only, never export.",
    },
    HelpSpec {
        control: "Fit Artwork",
        heading: "Fit Artwork",
        summary: "Fit the complete artwork into the canvas view.",
        body: "Turn this on to fit the full artwork. This affects the canvas preview only, never export.",
    },
    HelpSpec {
        control: "Share Hatch Path Across Layers",
        heading: "Share Hatch Path Across Layers",
        summary: "Use one hatch path for every monochrome layer.",
        body: "Turn this on to edit one shared hatch path, or off to give layers independent paths. This changes both preview and export.",
    },
];

fn help_for(control: &str) -> Option<&'static HelpSpec> {
    HELP_SPECS.iter().find(|spec| spec.control == control)
}

#[derive(Clone)]
struct HelpHandle {
    button: gtk::MenuButton,
    popover: gtk::Popover,
    heading: gtk::Label,
    body: gtk::Label,
}

impl HelpHandle {
    fn set_spec(&self, spec: &HelpSpec) {
        self.button.set_tooltip_text(Some(spec.summary));
        self.button
            .update_property(&[gtk::accessible::Property::Label(&format!(
                "Help for {}",
                spec.control
            ))]);
        self.heading.set_text(spec.heading);
        self.body.set_text(spec.body);
        self.popover.queue_resize();
        self.popover.queue_draw();
    }
}

fn help_handle(spec: &HelpSpec) -> HelpHandle {
    let button = gtk::MenuButton::builder()
        .icon_name("help-about-symbolic")
        .focusable(true)
        .build();
    button.add_css_class("flat");
    let heading = gtk::Label::builder()
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let body = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .width_chars(34)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&heading);
    content.append(&body);
    let popover = gtk::Popover::builder()
        .has_arrow(true)
        .child(&content)
        .build();
    button.set_popover(Some(&popover));
    let handle = HelpHandle {
        button,
        popover,
        heading,
        body,
    };
    handle.set_spec(spec);
    handle
}

fn help_button(spec: &HelpSpec) -> gtk::MenuButton {
    help_handle(spec).button
}

#[cfg(test)]
#[cfg(any())]
fn button_with_help(button: &gtk::Button, control: &str) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.set_visible(button.is_visible());
    button.connect_visible_notify(glib::clone!(
        #[weak]
        row,
        move |button| row.set_visible(button.is_visible())
    ));
    row.append(button);
    if let Some(spec) = help_for(control) {
        button.set_tooltip_text(Some(spec.summary));
        button.update_property(&[gtk::accessible::Property::Description(spec.summary)]);
        row.append(&help_button(spec));
    }
    row.upcast()
}

fn compact_control_help(widget: &(impl IsA<gtk::Widget> + IsA<gtk::Accessible>), control: &str) {
    if let Some(spec) = help_for(control) {
        widget.set_tooltip_text(Some(spec.summary));
        widget.update_property(&[gtk::accessible::Property::Description(spec.summary)]);
    }
}

fn combo_row(title: &str, combo: &gtk::DropDown) -> gtk::Widget {
    labeled_combo_row(title, combo).0
}

fn labeled_combo_row(title: &str, combo: &gtk::DropDown) -> (gtk::Widget, gtk::Label) {
    let (row, label, _) = labeled_combo_row_with_help(title, combo);
    (row, label)
}

fn labeled_combo_row_with_help(
    title: &str,
    combo: &gtk::DropDown,
) -> (gtk::Widget, gtk::Label, Option<HelpHandle>) {
    combo.set_hexpand(true);
    combo.set_size_request(0, -1);
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let labels = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    labels.append(&label);
    let help = help_for(title).map(help_handle);
    if let Some(handle) = &help {
        labels.append(&handle.button);
    }
    row.append(&labels);
    row.append(combo);
    combo.update_relation(&[gtk::accessible::Relation::LabelledBy(&[label.upcast_ref()])]);
    if help.is_some() {
        combo.update_property(&[gtk::accessible::Property::Description(
            help_for(title).unwrap().summary,
        )]);
        combo.set_tooltip_text(help_for(title).map(|spec| spec.summary));
    }
    (row.upcast(), label, help)
}

fn source_mapping_from_index(index: u32) -> Option<ValueMode> {
    match index {
        0 => Some(ValueMode::Cmyk),
        1 => Some(ValueMode::SingleChannel),
        2 => Some(ValueMode::Luminance),
        3 => Some(ValueMode::CrosshatchLuminance),
        4 => Some(ValueMode::Rgb),
        _ => None,
    }
}

fn artwork_source_labels(include_legacy: bool) -> Vec<&'static str> {
    let mut labels = vec![
        "Full Color",
        "Red",
        "Green",
        "Blue",
        "Value",
        "Perceptual Lightness",
        "Alpha",
    ];
    if include_legacy {
        labels.push("Legacy Brightness");
    }
    labels
}

fn artwork_source_from_index(index: u32) -> Option<ArtworkSource> {
    [
        ArtworkSource::FullColor,
        ArtworkSource::Red,
        ArtworkSource::Green,
        ArtworkSource::Blue,
        ArtworkSource::Value,
        ArtworkSource::PerceptualLightness,
        ArtworkSource::Alpha,
    ]
    .get(index as usize)
    .copied()
}

fn artwork_source_index(source: ArtworkSource) -> u32 {
    match source {
        ArtworkSource::FullColor => 0,
        ArtworkSource::Red => 1,
        ArtworkSource::Green => 2,
        ArtworkSource::Blue => 3,
        ArtworkSource::Value => 4,
        ArtworkSource::PerceptualLightness => 5,
        ArtworkSource::Alpha => 6,
        ArtworkSource::LegacyBrightness(_) => 7,
    }
}

fn artwork_source_guidance(source: ArtworkSource) -> &'static str {
    match source {
        ArtworkSource::FullColor => {
            "Separate the source color automatically for the selected output model."
        }
        ArtworkSource::Red => "Use the encoded red component as sampled content.",
        ArtworkSource::Green => "Use the encoded green component as sampled content.",
        ArtworkSource::Blue => "Use the encoded blue component as sampled content.",
        ArtworkSource::Value => {
            "Use the strongest RGB component (HSV value semantics) as sampled content."
        }
        ArtworkSource::PerceptualLightness => {
            "Use OKLab L for perceptual lightness as sampled content."
        }
        ArtworkSource::Alpha => "Use sampled alpha as content.",
        ArtworkSource::LegacyBrightness(_) => {
            "Legacy brightness is retained only for compatibility-loaded state."
        }
    }
}

fn source_alpha_labels(include_legacy: bool) -> Vec<&'static str> {
    let mut labels = vec!["Preserve Source Alpha", "Ignore Source Alpha"];
    if include_legacy {
        labels.push("Legacy Source Alpha");
    }
    labels
}

fn source_alpha_from_index(index: u32) -> Option<SourceAlphaPolicy> {
    match index {
        0 => Some(SourceAlphaPolicy::Preserve),
        1 => Some(SourceAlphaPolicy::Ignore),
        2 => Some(SourceAlphaPolicy::LegacyCurrentV1),
        _ => None,
    }
}

fn source_alpha_index(alpha: SourceAlphaPolicy) -> u32 {
    match alpha {
        SourceAlphaPolicy::Preserve => 0,
        SourceAlphaPolicy::Ignore => 1,
        SourceAlphaPolicy::LegacyCurrentV1 => 2,
    }
}

fn channel_assignment_labels(full_color: bool, output_model: OutputModel) -> Vec<&'static str> {
    if full_color {
        vec![match output_model {
            OutputModel::CmykPrint => "Automatic CMYK Separation",
            OutputModel::RgbScreen => "Direct RGB Channels",
        }]
    } else {
        vec!["Apply To Active Channel", "Apply To All Channels"]
    }
}

fn channel_assignment_from_index(index: u32, source: ArtworkSource) -> Option<ChannelAssignment> {
    match (source == ArtworkSource::FullColor, index) {
        (true, 0) => Some(ChannelAssignment::automatic(
            AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1,
        )),
        (false, 0) => Some(ChannelAssignment::ActiveChannel),
        (false, 1) => Some(ChannelAssignment::AllChannels),
        _ => None,
    }
}

fn channel_assignment_index(assignment: ChannelAssignment, source: ArtworkSource) -> u32 {
    if source == ArtworkSource::FullColor {
        0
    } else {
        match assignment {
            ChannelAssignment::AllChannels => 1,
            _ => 0,
        }
    }
}

fn output_channel_labels(output: OutputModel) -> Vec<&'static str> {
    output
        .channels()
        .iter()
        .map(|channel| channel.label())
        .collect()
}

fn channel_scope_labels(output: OutputModel, crosshatch: bool) -> Vec<&'static str> {
    if crosshatch {
        return vec!["All Layers"];
    }
    match output {
        OutputModel::CmykPrint => vec![
            "All Inks",
            "Cyan Ink",
            "Magenta Ink",
            "Yellow Ink",
            "Black Ink",
        ],
        OutputModel::RgbScreen => vec![
            "All Channels",
            "Red Channel",
            "Green Channel",
            "Blue Channel",
        ],
    }
}

/// Converts a transient treatment-scope presentation position into the current
/// model's semantic channel only at the callback boundary. The cached widget
/// identity remains the `OutputChannelId`; no index is retained as widget
/// identity, and the aggregate scope remains `None` rather than an ID.
fn channel_scope_channel(selected: u32, output: OutputModel) -> Option<Option<OutputChannelId>> {
    if selected == 0 {
        return Some(None);
    }
    output
        .channels()
        .get(selected.checked_sub(1)? as usize)
        .copied()
        .map(Some)
}

fn channel_scope_target_index(
    channel: Option<OutputChannelId>,
    output: OutputModel,
) -> Option<u32> {
    match channel {
        None => Some(0),
        Some(channel) => output
            .channels()
            .iter()
            .position(|candidate| *candidate == channel)
            .map(|index| index as u32 + 1),
    }
}

fn channel_scope_index(selected_target: u32, output: OutputModel, crosshatch: bool) -> u32 {
    if crosshatch {
        return 0;
    }
    channel_scope_channel(selected_target, output)
        .and_then(|channel| channel_scope_target_index(channel, output))
        .unwrap_or(0)
}

fn pipeline_for_source(
    current: &ArtworkPipelineSettings,
    source: ArtworkSource,
) -> ArtworkPipelineSettings {
    let mut pipeline = current.clone();
    pipeline.source = source;
    if source == ArtworkSource::FullColor {
        pipeline.assignment = ChannelAssignment::automatic(match pipeline.output_model {
            OutputModel::CmykPrint => AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1,
            OutputModel::RgbScreen => AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1,
        });
        pipeline.active_channel = pipeline
            .active_channel
            .filter(|channel| channel.belongs_to(pipeline.output_model));
    } else if !matches!(
        pipeline.assignment,
        ChannelAssignment::ActiveChannel | ChannelAssignment::AllChannels
    ) {
        pipeline.assignment = ChannelAssignment::AllChannels;
        pipeline.active_channel = None;
    }
    if matches!(pipeline.assignment, ChannelAssignment::ActiveChannel) {
        pipeline.active_channel = pipeline
            .active_channel
            .filter(|channel| channel.belongs_to(pipeline.output_model))
            .or_else(|| Some(pipeline.output_model.default_channel()));
    }
    pipeline
}

fn pipeline_for_assignment(
    current: &ArtworkPipelineSettings,
    assignment: ChannelAssignment,
) -> ArtworkPipelineSettings {
    let mut pipeline = current.clone();
    pipeline.assignment = match assignment {
        ChannelAssignment::Automatic { .. } => {
            ChannelAssignment::automatic(match pipeline.output_model {
                OutputModel::CmykPrint => AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1,
                OutputModel::RgbScreen => AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1,
            })
        }
        assignment => assignment,
    };
    pipeline.active_channel = if matches!(pipeline.assignment, ChannelAssignment::ActiveChannel) {
        pipeline
            .active_channel
            .filter(|channel| channel.belongs_to(pipeline.output_model))
            .or_else(|| Some(pipeline.output_model.default_channel()))
    } else {
        None
    };
    pipeline
}

fn output_channel_order(rgb: bool, crosshatch: bool) -> &'static [Ink] {
    if rgb && !crosshatch {
        &Ink::RGB
    } else if crosshatch {
        &CROSSHATCH_INK_ORDER
    } else {
        &Ink::ALL
    }
}

fn visible_ink_for_slot(index: usize, rgb: bool, crosshatch: bool) -> Option<Ink> {
    output_channel_order(rgb, crosshatch).get(index).copied()
}

fn web_inks_for_target(selected: u32, rgb: bool, crosshatch: bool) -> Option<Vec<Ink>> {
    let channels = output_channel_order(rgb, crosshatch);
    match selected {
        0 => Some(channels.to_vec()),
        slot => channels
            .get(slot.checked_sub(1)? as usize)
            .copied()
            .map(|ink| vec![ink]),
    }
}

fn dropdown_strings_match(dropdown: &gtk::DropDown, values: &[&str]) -> bool {
    dropdown.model().is_some_and(|model| {
        model.n_items() == values.len() as u32
            && values.iter().enumerate().all(|(index, value)| {
                model
                    .item(index as u32)
                    .and_then(|item| item.downcast::<gtk::StringObject>().ok())
                    .is_some_and(|item| item.string() == *value)
            })
    })
}

fn sync_dropdown_strings(dropdown: &gtk::DropDown, values: &[&str]) {
    if dropdown_strings_match(dropdown, values) {
        return;
    }

    let selected = dropdown.selected();
    let new_selected = if selected == gtk::INVALID_LIST_POSITION {
        0
    } else {
        selected.min(values.len().saturating_sub(1) as u32)
    };
    if let Some(model) = dropdown
        .model()
        .and_then(|model| model.downcast::<gtk::StringList>().ok())
    {
        // Keep the model object installed while a DropDown activation is being
        // dispatched.  Replacing it invalidates GTK's live list selection.
        model.splice(0, model.n_items(), values);
    } else {
        dropdown.set_model(Some(&gtk::StringList::new(values)));
    }
    dropdown.set_selected(new_selected);
}

fn layer_terminology(rgb: bool, crosshatch: bool) -> (&'static str, &'static str) {
    if rgb && !crosshatch {
        ("Adjust Channel", "Visible RGB Channels")
    } else if crosshatch {
        ("Adjust Layer", "Visible Crosshatch Layers")
    } else {
        ("Adjust Ink", "Visible Inks")
    }
}

struct WebColorCopy {
    color_heading: &'static str,
    color_tooltip: &'static str,
    opacity_heading: &'static str,
    opacity_tooltip: &'static str,
}

fn web_color_copy(channel_copy: bool) -> WebColorCopy {
    if channel_copy {
        WebColorCopy {
            color_heading: "Channel Color",
            color_tooltip: "Set the displayed color for the selected RGB channel.",
            opacity_heading: "Channel Opacity",
            opacity_tooltip: "Change how solid each RGB channel appears.",
        }
    } else {
        WebColorCopy {
            color_heading: "Ink Color",
            color_tooltip: "Set the displayed color for the selected ink.",
            opacity_heading: "Ink Opacity",
            opacity_tooltip: "Change how solid each ink appears.",
        }
    }
}

fn web_color_validation_message(channel_copy: bool) -> &'static str {
    if channel_copy {
        "Use a six-digit hex channel color such as #111111"
    } else {
        "Use a six-digit hex ink color such as #111111"
    }
}

fn web_mixed_target(channel_copy: bool) -> &'static str {
    if channel_copy { "channels" } else { "inks" }
}

fn rgb_visibility_summary(settings: &WebShapeSettings) -> Option<String> {
    let visible = [
        (Ink::Red, "Red"),
        (Ink::Green, "Green"),
        (Ink::Blue, "Blue"),
    ]
    .into_iter()
    .filter_map(|(ink, label)| settings.channels.get(ink).enabled.then_some(label))
    .collect::<Vec<_>>();
    match visible.len() {
        3 => None,
        0 => Some("Visible: none".into()),
        _ => Some(format!("Visible: {}", visible.join(" + "))),
    }
}

fn set_crosshatch_target_directions(dropdown: &gtk::DropDown, angles: [f64; 4]) {
    let values = [
        "All Layers".to_owned(),
        format!("Layer 1 · {:.0}° (K)", angles[0]),
        format!("Layer 2 · {:.0}° (C)", angles[1]),
        format!("Layer 3 · {:.0}° (M)", angles[2]),
        format!("Layer 4 · {:.0}° (Y)", angles[3]),
    ];
    let refs = values.iter().map(String::as_str).collect::<Vec<_>>();
    sync_dropdown_strings(dropdown, &refs);
}

fn sync_layer_terminology(
    dropdown: &gtk::DropDown,
    target_label: &gtk::Label,
    target_help: Option<&HelpHandle>,
    visible_label: &gtk::Label,
    rgb: bool,
    crosshatch: bool,
) {
    let (wanted, visible) = layer_terminology(rgb, crosshatch);
    target_label.set_text(wanted);
    if let Some(spec) = help_for(wanted) {
        dropdown.update_property(&[gtk::accessible::Property::Description(spec.summary)]);
        if let Some(help) = target_help {
            help.set_spec(spec);
        }
    }
    visible_label.set_text(visible);
    let values: &[&str] = if rgb && !crosshatch {
        &["All Channels", "Red", "Green", "Blue"]
    } else if crosshatch {
        &[
            "All Layers",
            "Layer 1 (K)",
            "Layer 2 (C)",
            "Layer 3 (M)",
            "Layer 4 (Y)",
        ]
    } else {
        &["All Inks", "Cyan", "Magenta", "Yellow", "Black"]
    };
    sync_dropdown_strings(dropdown, values);
}

#[cfg(test)]
#[cfg(any())]
fn control_status_row(title: &str, status: &str, scale: &gtk::Scale) -> (gtk::Widget, gtk::Label) {
    let (row, _, status, _) = control_status_row_with_help(title, status, scale);
    (row, status)
}

#[cfg(test)]
#[cfg(any())]
fn control_status_row_with_help(
    title: &str,
    status: &str,
    scale: &gtk::Scale,
) -> (gtk::Widget, gtk::Label, gtk::Label, Option<HelpHandle>) {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let labels = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let heading = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    labels.append(&heading);
    let help = help_for(title).map(help_handle);
    if let Some(handle) = &help {
        labels.append(&handle.button);
    }
    let status = gtk::Label::builder()
        .label(status)
        .xalign(1.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["dim-label", "caption"])
        .build();
    labels.append(&status);
    row.append(&labels);
    row.append(&precision_scale_control(scale));
    scale.update_relation(&[gtk::accessible::Relation::LabelledBy(&[
        heading.upcast_ref()
    ])]);
    if let Some(spec) = help_for(title) {
        scale.update_property(&[gtk::accessible::Property::Description(spec.summary)]);
        scale.set_tooltip_text(Some(spec.summary));
    }
    (row.upcast(), heading, status, help)
}

fn sync_web_scale(
    scale: &gtk::Scale,
    status: &gtk::Label,
    value: f64,
    mixed: bool,
    normal_status: &str,
    mixed_target: &str,
) {
    scale.set_draw_value(false);
    if let Some(entry) = precision_entry(scale) {
        entry.set_visible(!mixed);
    }
    if mixed {
        scale.add_css_class("mixed-scale");
        let description = format!(
            "Mixed values; changing this control applies one value to all selected {mixed_target}"
        );
        scale.update_property(&[gtk::accessible::Property::Description(&description)]);
    } else {
        scale.remove_css_class("mixed-scale");
        scale.update_property(&[gtk::accessible::Property::Description(normal_status)]);
    }
    status.set_text(if mixed { "Mixed" } else { normal_status });
    scale.set_value(value);
}

fn edit_curve_paths(
    settings: &mut WebCurveSettings,
    inks: &[Ink],
    mut edit: impl FnMut(&mut CurvePath),
) {
    if settings.use_shared_curve {
        edit(&mut settings.shared_path);
    } else {
        for ink in inks {
            edit(&mut settings.channels.get_mut(*ink).path);
        }
    }
}

fn curve_handle_points(path: &CurvePath) -> Vec<CurvePoint> {
    let mut points = Vec::with_capacity(1 + path.segments.len() * 3);
    points.push(path.start);
    for segment in &path.segments {
        points.extend([segment.control_1, segment.control_2, segment.end]);
    }
    points
}

fn set_curve_handle(path: &mut CurvePath, handle: usize, point: CurvePoint) {
    if handle.is_multiple_of(3) {
        let old = curve_handle_points(path)
            .get(handle)
            .copied()
            .unwrap_or(point);
        let delta = CurvePoint {
            x: point.x - old.x,
            y: point.y - old.y,
        };
        translate_curve_anchor(path, handle / 3, delta);
        return;
    }
    set_curve_component(path, handle, point);
}

fn set_curve_component(path: &mut CurvePath, handle: usize, point: CurvePoint) {
    if handle == 0 {
        path.start = point;
        return;
    }
    let segment = (handle - 1) / 3;
    let component = (handle - 1) % 3;
    let Some(segment) = path.segments.get_mut(segment) else {
        return;
    };
    match component {
        0 => segment.control_1 = point,
        1 => segment.control_2 = point,
        _ => segment.end = point,
    }
}

fn translate_curve_anchor(path: &mut CurvePath, anchor: usize, delta: CurvePoint) {
    let shift = |point: &mut CurvePoint| {
        point.x += delta.x;
        point.y += delta.y;
    };
    if anchor == 0 {
        shift(&mut path.start);
        if let Some(first) = path.segments.first_mut() {
            shift(&mut first.control_1);
        }
    } else if anchor <= path.segments.len() {
        shift(&mut path.segments[anchor - 1].end);
        shift(&mut path.segments[anchor - 1].control_2);
        if let Some(next) = path.segments.get_mut(anchor) {
            shift(&mut next.control_1);
        }
    }
}

fn curve_editor_scale(width: i32, height: i32) -> f64 {
    (((width - 32).max(1) as f64) / 1.1).min(((height - 32).max(1) as f64) / 0.42)
}

fn curve_to_editor_point(point: CurvePoint, width: i32, height: i32) -> (f64, f64) {
    let scale = curve_editor_scale(width, height);
    (
        width as f64 / 2.0 + point.x * scale,
        height as f64 / 2.0 - point.y * scale,
    )
}

fn editor_to_curve_point(x: f64, y: f64, width: i32, height: i32) -> CurvePoint {
    let scale = curve_editor_scale(width, height);
    CurvePoint {
        x: ((x - width as f64 / 2.0) / scale).clamp(-1.5, 1.5),
        y: ((height as f64 / 2.0 - y) / scale).clamp(-1.5, 1.5),
    }
}

fn nearest_curve_handle(path: &CurvePath, x: f64, y: f64, width: i32, height: i32) -> i32 {
    curve_handle_points(path)
        .into_iter()
        .enumerate()
        .map(|(index, point)| {
            let point = curve_to_editor_point(point, width, height);
            (index as i32, (point.0 - x).hypot(point.1 - y))
        })
        .filter(|(_, distance)| *distance <= 14.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index)
        .unwrap_or(-1)
}

fn draw_curve_editor(
    context: &gtk::cairo::Context,
    width: i32,
    height: i32,
    path: Option<&CurvePath>,
    selected: i32,
    color: (f64, f64, f64),
) {
    context.set_source_rgba(0.12, 0.13, 0.15, 1.0);
    let _ = context.paint();
    context.set_source_rgba(1.0, 1.0, 1.0, 0.16);
    context.set_line_width(1.0);
    let center = curve_to_editor_point(CurvePoint::default(), width, height);
    context.move_to(12.0, center.1);
    context.line_to(width as f64 - 12.0, center.1);
    let _ = context.stroke();
    let Some(path) = path else { return };
    let mut start = path.start;
    for segment in &path.segments {
        let a = curve_to_editor_point(start, width, height);
        let b = curve_to_editor_point(segment.control_1, width, height);
        let c = curve_to_editor_point(segment.control_2, width, height);
        let d = curve_to_editor_point(segment.end, width, height);
        context.set_source_rgba(0.45, 0.68, 1.0, 0.45);
        context.move_to(a.0, a.1);
        context.line_to(b.0, b.1);
        context.move_to(c.0, c.1);
        context.line_to(d.0, d.1);
        let _ = context.stroke();
        start = segment.end;
    }
    let start = curve_to_editor_point(path.start, width, height);
    context.move_to(start.0, start.1);
    for segment in &path.segments {
        let c1 = curve_to_editor_point(segment.control_1, width, height);
        let c2 = curve_to_editor_point(segment.control_2, width, height);
        let end = curve_to_editor_point(segment.end, width, height);
        context.curve_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
    }
    context.set_source_rgba(color.0, color.1, color.2, 1.0);
    context.set_line_width(3.0);
    let _ = context.stroke();
    for (index, point) in curve_handle_points(path).into_iter().enumerate() {
        let point = curve_to_editor_point(point, width, height);
        let anchor = index == 0 || index % 3 == 0;
        context.arc(
            point.0,
            point.1,
            if anchor { 5.0 } else { 3.5 },
            0.0,
            std::f64::consts::TAU,
        );
        if index as i32 == selected {
            context.set_source_rgb(1.0, 0.75, 0.2);
        } else if anchor {
            context.set_source_rgb(0.95, 0.95, 0.98);
        } else {
            context.set_source_rgb(color.0, color.1, color.2);
        }
        let _ = context.fill();
    }
}

fn nearest_curve_segment(path: &CurvePath, point: CurvePoint) -> (usize, f64) {
    let mut best = (0, 0.5, f64::INFINITY);
    let mut start = path.start;
    for (index, segment) in path.segments.iter().enumerate() {
        for step in 0..=32 {
            let amount = step as f64 / 32.0;
            let candidate = cubic_editor_point(start, *segment, amount);
            let distance = (candidate.x - point.x).hypot(candidate.y - point.y);
            if distance < best.2 {
                best = (index, amount.clamp(0.08, 0.92), distance);
            }
        }
        start = segment.end;
    }
    (best.0, best.1)
}

fn cubic_editor_point(
    start: CurvePoint,
    segment: toniator::model::CubicCurveSegment,
    amount: f64,
) -> CurvePoint {
    let inverse = 1.0 - amount;
    CurvePoint {
        x: inverse.powi(3) * start.x
            + 3.0 * inverse.powi(2) * amount * segment.control_1.x
            + 3.0 * inverse * amount.powi(2) * segment.control_2.x
            + amount.powi(3) * segment.end.x,
        y: inverse.powi(3) * start.y
            + 3.0 * inverse.powi(2) * amount * segment.control_1.y
            + 3.0 * inverse * amount.powi(2) * segment.control_2.y
            + amount.powi(3) * segment.end.y,
    }
}

fn split_curve_segment(path: &mut CurvePath, index: usize, amount: f64) {
    let Some(segment) = path.segments.get(index).copied() else {
        return;
    };
    let start = if index == 0 {
        path.start
    } else {
        path.segments[index - 1].end
    };
    let a = curve_lerp(start, segment.control_1, amount);
    let b = curve_lerp(segment.control_1, segment.control_2, amount);
    let c = curve_lerp(segment.control_2, segment.end, amount);
    let d = curve_lerp(a, b, amount);
    let e = curve_lerp(b, c, amount);
    let midpoint = curve_lerp(d, e, amount);
    path.segments[index] = toniator::model::CubicCurveSegment {
        control_1: a,
        control_2: d,
        end: midpoint,
    };
    path.segments.insert(
        index + 1,
        toniator::model::CubicCurveSegment {
            control_1: e,
            control_2: c,
            end: segment.end,
        },
    );
}

fn delete_curve_anchor(path: &mut CurvePath, handle: usize) {
    if path.segments.len() <= 1 || !handle.is_multiple_of(3) {
        return;
    }
    let anchor = handle / 3;
    if anchor == 0 {
        path.start = path.segments[0].end;
        path.segments.remove(0);
    } else if anchor >= path.segments.len() {
        path.segments.pop();
    } else {
        let after = path.segments.remove(anchor);
        let before = &mut path.segments[anchor - 1];
        before.control_2 = after.control_2;
        before.end = after.end;
    }
}

fn curve_lerp(a: CurvePoint, b: CurvePoint, amount: f64) -> CurvePoint {
    CurvePoint {
        x: a.x + (b.x - a.x) * amount,
        y: a.y + (b.y - a.y) * amount,
    }
}

#[cfg(test)]
#[cfg(any())]
fn control_scale(minimum: f64, maximum: f64, step: f64) -> gtk::Scale {
    let scale = gtk::Scale::new(gtk::Orientation::Horizontal, None::<&gtk::Adjustment>);
    configure_control_scale(&scale, minimum, maximum, step);
    scale
}

fn configure_control_scale(scale: &gtk::Scale, minimum: f64, maximum: f64, step: f64) {
    scale.set_range(minimum, maximum);
    scale.set_increments(step, step * 10.0);
    disable_pointer_scroll_adjustment(scale);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
}

#[derive(Clone, Copy)]
struct BuilderScaleSpec {
    scale_id: &'static str,
    control_id: &'static str,
    label_id: &'static str,
    accessible_name: &'static str,
    minimum: f64,
    maximum: f64,
    step: f64,
}

const NATIVE_CONTROL_SCALES: [BuilderScaleSpec; 4] = [
    BuilderScaleSpec {
        scale_id: "native_sampling_detail_scale",
        control_id: "native_sampling_detail_control",
        label_id: "native_sampling_detail_label",
        accessible_name: "Sampling Detail",
        minimum: 0.0,
        maximum: 100.0,
        step: 1.0,
    },
    BuilderScaleSpec {
        scale_id: "native_coverage_scale",
        control_id: "native_coverage_control",
        label_id: "native_coverage_label",
        accessible_name: "Coverage",
        minimum: 0.0,
        maximum: 160.0,
        step: 1.0,
    },
    BuilderScaleSpec {
        scale_id: "native_contrast_scale",
        control_id: "native_contrast_control",
        label_id: "native_contrast_label",
        accessible_name: "Contrast",
        minimum: 0.0,
        maximum: 200.0,
        step: 1.0,
    },
    BuilderScaleSpec {
        scale_id: "native_screen_angle_scale",
        control_id: "native_screen_angle_control",
        label_id: "native_screen_angle_label",
        accessible_name: "Screen Angle",
        minimum: -180.0,
        maximum: 180.0,
        step: 1.0,
    },
];

fn builder_control_scale(builder: &gtk::Builder, spec: BuilderScaleSpec) -> gtk::Scale {
    let scale = builder
        .object::<gtk::Scale>(spec.scale_id)
        .unwrap_or_else(|| panic!("ToniatorEditorControls.ui must define {}", spec.scale_id));
    let control = builder
        .object::<gtk::Box>(spec.control_id)
        .unwrap_or_else(|| panic!("ToniatorEditorControls.ui must define {}", spec.control_id));
    let label = builder
        .object::<gtk::Label>(spec.label_id)
        .unwrap_or_else(|| panic!("ToniatorEditorControls.ui must define {}", spec.label_id));
    configure_control_scale(&scale, spec.minimum, spec.maximum, spec.step);
    attach_precision_entry(&scale, &control);
    scale.update_property(&[gtk::accessible::Property::Label(spec.accessible_name)]);
    scale.update_relation(&[gtk::accessible::Relation::LabelledBy(&[label.upcast_ref()])]);
    scale
}

fn configure_dropdown_accessibility(
    dropdown: &gtk::DropDown,
    label: &gtk::Label,
    accessible_name: &str,
) {
    dropdown.set_focusable(true);
    dropdown.update_property(&[gtk::accessible::Property::Label(accessible_name)]);
    dropdown.update_relation(&[gtk::accessible::Relation::LabelledBy(&[label.upcast_ref()])]);
}

fn disable_pointer_scroll_adjustment(widget: &impl IsA<gtk::Widget>) -> usize {
    let controllers = widget.observe_controllers();
    let mut disabled = 0;
    for index in 0..controllers.n_items() {
        let Some(scroll) = controllers
            .item(index)
            .and_then(|item| item.downcast::<gtk::EventControllerScroll>().ok())
        else {
            continue;
        };
        // GtkScale and GtkSpinButton install target-phase wheel controllers.
        // Disabling those built-ins lets the original GDK scroll event continue
        // to the containing GtkScrolledWindow, including smooth/kinetic deltas.
        // Pointer drag, keyboard actions, editing and accessibility controllers
        // remain installed and unchanged.
        scroll.set_propagation_phase(gtk::PropagationPhase::None);
        disabled += 1;
    }
    disabled
}

#[cfg(test)]
#[cfg(any())]
fn precision_scale_control(scale: &gtk::Scale) -> gtk::Widget {
    let control = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    attach_precision_entry(scale, &control);
    control.upcast()
}

fn attach_precision_entry(scale: &gtk::Scale, control: &gtk::Box) {
    if scale.parent().is_none() {
        control.append(scale);
    }
    let adjustment = scale.adjustment();
    let step = adjustment.step_increment().abs();
    let digits = if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else {
        2
    };
    let entry = control
        .observe_children()
        .iter::<glib::Object>()
        .flatten()
        .find_map(|child| child.downcast::<gtk::SpinButton>().ok())
        .unwrap_or_else(|| gtk::SpinButton::new(Some(&adjustment), step.max(0.01), digits));
    entry.set_adjustment(&adjustment);
    entry.set_digits(digits);
    disable_pointer_scroll_adjustment(&entry);
    entry.set_width_chars(5);
    entry.set_max_width_chars(7);
    entry.set_numeric(true);
    entry.set_tooltip_text(Some("Enter an exact value"));
    entry.update_property(&[gtk::accessible::Property::Label("Exact value")]);
    if entry.parent().is_none() {
        control.append(&entry);
    }
}

fn precision_entry(scale: &gtk::Scale) -> Option<gtk::SpinButton> {
    scale
        .parent()
        .and_then(|parent| parent.last_child())
        .and_then(|child| child.downcast::<gtk::SpinButton>().ok())
}

fn connect_clicked(
    button: &impl IsA<gtk::Button>,
    ui: &Rc<AppUi>,
    action: impl Fn(&Rc<AppUi>) + 'static,
) {
    button.connect_clicked(glib::clone!(
        #[weak]
        ui,
        move |_| action(&ui)
    ));
}

fn media_type(path: &Path) -> String {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
    .into()
}

fn ensure_extension(mut path: PathBuf, extension: &str) -> PathBuf {
    if !path
        .extension()
        .is_some_and(|current| current.eq_ignore_ascii_case(extension))
    {
        path.set_extension(extension);
    }
    path
}

fn editor_source_text(document: &Document) -> String {
    format!("Source: {}", document.source.name)
}

fn install_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        r#"
        .canvas { background: #23252a; }
        .artboard { background: transparent; }
        .inspector-pane, .inspector { background: @window_bg_color; }
        .inspector-shell { background: @window_bg_color; }
        .editing-context {
            padding: 10px 14px;
            font-weight: 600;
            border-bottom: 1px solid alpha(@borders, 0.5);
            background: alpha(@accent_bg_color, 0.08);
        }
        .workspace-status { padding: 0 12px 6px; }
        .workflow-group {
            padding: 12px;
            border-radius: 12px;
            background: alpha(@card_bg_color, 0.72);
            border: 1px solid alpha(@borders, 0.55);
        }
        .mapping-artwork {
            background: #f6f7f8;
            border-radius: 6px;
            padding: 4px;
        }
        .preview-indicator { color: @accent_color; }
        paned > separator.wide { min-width: 10px; }
        scale value { min-width: 42px; }
        scale.mixed-scale highlight, scale.mixed-scale slider { opacity: 0; }
        "#,
    );
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_shell_resource_declares_required_ids_without_display() {
        for id in TOP_LEVEL_SHELL_OBJECT_IDS {
            assert!(
                WINDOW_BLP.contains(&format!(" {id} ")),
                "toniator-window.blp must retain the {id} stable ID"
            );
        }
        for class in [
            "ApplicationWindow",
            "ToolbarView",
            "HeaderBar",
            "ToastOverlay",
            "Stack",
        ] {
            assert!(
                WINDOW_BLP.contains(class),
                "toniator-window.blp must retain the {class} shell layer"
            );
        }
        assert!(WINDOW_BLP.contains("transition-type: crossfade"));
        assert!(WINDOW_BLP.contains("transition-duration: 180"));
    }

    #[test]
    fn inspector_and_channel_resources_keep_the_stage_two_boundary_without_display() {
        for id in [
            "source_section",
            "source_content_host",
            "output_section",
            "output_content_host",
            "channel_settings_section",
            "channel_scope_host",
            "channel_scope_note",
            "channel_panel_stack",
            "appearance_section",
            "treatment_section",
        ] {
            assert!(
                WINDOW_BLP.contains(id),
                "toniator-window.blp must retain {id}"
            );
        }
        let source = WINDOW_BLP.find("source_section").unwrap();
        let output = WINDOW_BLP.find("output_section").unwrap();
        let channels = WINDOW_BLP.find("channel_settings_section").unwrap();
        assert!(source < output && output < channels);
        assert!(WINDOW_BLP.contains("Treatment Editing Scope"));
        assert!(WINDOW_BLP.contains(
            "Choose the inks or channels edited by Shapes and Curves. Output routing remains in Output."
        ));
        for id in [
            "channel_controls",
            "channel_heading",
            "channel_inclusion_status",
            "channel_content_host",
        ] {
            assert!(
                CHANNEL_BLP.contains(id),
                "toniator-channel-controls.blp must retain {id}"
            );
        }
        assert!(!CHANNEL_BLP.contains("aggregate_channel_controls"));
        for id in [
            "aggregate_channel_controls",
            "aggregate_heading",
            "aggregate_mixed_message",
            "aggregate_content_host",
        ] {
            assert!(
                AGGREGATE_CHANNEL_BLP.contains(id),
                "toniator-aggregate-channel-controls.blp must retain {id}"
            );
        }
        assert!(!AGGREGATE_CHANNEL_BLP.contains("Box channel_controls {"));
    }

    #[test]
    fn editor_controls_resource_exposes_static_editor_structure_without_display() {
        for id in [
            "source_controls",
            "source_dynamic_host",
            "artwork_source",
            "artwork_source_label",
            "source_alpha",
            "source_alpha_label",
            "output_controls",
            "output_mode",
            "output_mode_label",
            "channel_assignment",
            "channel_assignment_label",
            "active_channel",
            "active_channel_label",
            "crosshatch_action",
            "appearance_controls",
            "preview_surface",
            "preview_color",
            "export_background",
            "export_color_label",
            "export_color",
            "treatment_chrome",
            "treatment_pattern_buttons",
            "treatment_preset_actions",
            "treatment_modes",
            "native_panel",
            "native_sampling_detail_row",
            "native_sampling_detail_control",
            "native_sampling_detail_scale",
            "native_coverage_row",
            "native_coverage_control",
            "native_coverage_scale",
            "native_contrast_row",
            "native_contrast_control",
            "native_contrast_scale",
            "native_screen_angle_row",
            "native_screen_angle_control",
            "native_screen_angle_scale",
            "web_panel_host",
            "web_shared",
            "web_shape",
            "web_polygon_sides",
            "web_edit_shape",
            "web_color",
            "web_coverage_scale",
            "web_coverage_entry",
            "web_advanced",
            "curve_panel_host",
            "curve_layout",
            "curve_weight_scale",
            "curve_weight_entry",
            "curve_profile",
            "curve_editor_host",
            "curve_reset",
            "curve_shared",
            "motif_controls",
            "motif_coverage",
            "motif_size_scale",
            "motif_size_entry",
            "motif_arrange",
            "curve_close_ends",
            "curve_smooth_join",
            "weighted_voronoi",
            "weighted_voronoi_panel",
            "weighted_voronoi_channel",
            "weighted_voronoi_cell_count",
            "weighted_voronoi_visible_0",
            "weighted_voronoi_arrangement",
            "weighted_voronoi_placement",
            "weighted_voronoi_density_strength",
            "weighted_voronoi_response_strength",
            "weighted_voronoi_boundary_gap",
            "weighted_voronoi_seed",
        ] {
            assert!(
                WINDOW_BLP.contains(id),
                "toniator-window.blp must retain {id}"
            );
        }
        assert!(WINDOW_BLP.contains("ColorDialogButton"));
        assert!(WINDOW_BLP.contains("DropDown"));
        assert!(WINDOW_BLP.contains("Stack"));
    }

    fn verify_realized_top_level_shell_builder() {
        register_ui_resources();
        let builder = gtk::Builder::from_resource(WINDOW_UI_RESOURCE);
        assert!(
            builder
                .object::<adw::ApplicationWindow>("main_window")
                .is_some()
        );
        assert!(
            builder
                .object::<adw::ToolbarView>("main_toolbar_view")
                .is_some()
        );
        assert!(
            builder
                .object::<adw::HeaderBar>("main_header_bar")
                .is_some()
        );
        assert!(
            builder
                .object::<adw::ToastOverlay>("toast_overlay")
                .is_some()
        );
        let stack = builder.object::<gtk::Stack>("main_stack").unwrap();
        assert_eq!(stack.transition_type(), gtk::StackTransitionType::Crossfade);
        assert_eq!(stack.transition_duration(), 180);
        assert!(builder.object::<adw::WindowTitle>("window_title").is_some());
        for id in [
            "new_project_button",
            "open_button",
            "save_button",
            "undo_button",
            "redo_button",
            "export_button",
        ] {
            assert!(builder.object::<gtk::Button>(id).is_some());
        }
        assert!(
            builder
                .object::<gtk::ToggleButton>("controls_toggle")
                .is_some()
        );
    }

    fn verify_realized_editor_controls_builder() {
        register_ui_resources();
        let builder = gtk::Builder::from_resource(WINDOW_UI_RESOURCE);
        let dropdowns = [
            ("artwork_source", "artwork_source_label", "Artwork Source"),
            ("source_alpha", "source_alpha_label", "Source Alpha"),
            ("output_mode", "output_mode_label", "Output Model"),
            (
                "channel_assignment",
                "channel_assignment_label",
                "Channel Assignment",
            ),
            ("active_channel", "active_channel_label", "Active Channel"),
        ];
        for (id, label_id, accessible_name) in dropdowns {
            let dropdown = builder
                .object::<gtk::DropDown>(id)
                .unwrap_or_else(|| panic!("missing {id}"));
            let label = builder
                .object::<gtk::Label>(label_id)
                .unwrap_or_else(|| panic!("missing {label_id}"));
            assert_eq!(label.label(), accessible_name);
            configure_dropdown_accessibility(&dropdown, &label, accessible_name);
            sync_dropdown_strings(&dropdown, &["First option", "Second option"]);
        }
        for spec in NATIVE_CONTROL_SCALES {
            let scale = builder_control_scale(&builder, spec);
            assert_eq!(scale.adjustment().lower(), spec.minimum);
            assert_eq!(scale.adjustment().upper(), spec.maximum);
            assert_eq!(scale.adjustment().step_increment(), spec.step);
            assert!(precision_entry(&scale).is_some());
            assert_eq!(
                builder
                    .object::<gtk::Box>(spec.control_id)
                    .unwrap()
                    .observe_children()
                    .n_items(),
                2,
                "{} must have one Builder scale and one precision entry",
                spec.accessible_name
            );
        }
        for id in [
            "web_coverage_scale",
            "web_angle_scale",
            "web_mark_angle_scale",
            "web_width_scale",
            "web_height_scale",
            "web_threshold_scale",
            "web_opacity_scale",
            "web_detail_scale",
            "curve_weight_scale",
            "curve_spacing_scale",
            "motif_size_scale",
            "motif_columns_scale",
            "motif_rows_scale",
            "motif_row_spacing_scale",
            "motif_stagger_scale",
            "curve_coverage_scale",
            "curve_angle_scale",
            "curve_position_x_scale",
            "curve_position_y_scale",
            "curve_opacity_scale",
            "curve_threshold_scale",
            "curve_detail_scale",
        ] {
            assert!(builder.object::<gtk::Scale>(id).is_some(), "missing {id}");
            let entry_id = id.replace("_scale", "_entry");
            assert!(
                builder.object::<gtk::SpinButton>(&entry_id).is_some(),
                "missing Builder precision entry {entry_id}"
            );
        }
        for id in [
            "web_shape",
            "web_mixed_shape_apply",
            "web_target",
            "curve_layout",
            "curve_profile",
            "curve_target",
            "motif_coverage",
            "motif_alternate",
            "weighted_voronoi_channel",
            "weighted_voronoi_arrangement",
            "weighted_voronoi_placement",
        ] {
            assert!(
                builder.object::<gtk::DropDown>(id).is_some(),
                "missing {id}"
            );
        }
        for id in ["preview_color", "export_color"] {
            assert!(
                builder.object::<gtk::ColorDialogButton>(id).is_some(),
                "missing {id}"
            );
        }
        let stack = builder.object::<gtk::Stack>("treatment_modes").unwrap();
        assert_eq!(stack.transition_type(), gtk::StackTransitionType::Crossfade);
        assert_eq!(stack.observe_children().n_items(), 4);
        for id in [
            "weighted_voronoi",
            "weighted_voronoi_panel",
            "weighted_voronoi_cell_count",
            "weighted_voronoi_density_strength",
            "weighted_voronoi_response_strength",
            "weighted_voronoi_boundary_gap",
            "weighted_voronoi_seed",
        ] {
            assert!(builder.object::<gtk::Widget>(id).is_some(), "missing {id}");
        }

        let root = builder.object::<gtk::Box>("editor_controls").unwrap();
        assert!(root.parent().is_some());
        let stack = builder.object::<gtk::Stack>("main_stack").unwrap();
        let editor_page = builder.object::<gtk::Box>("editor_page").unwrap();
        stack.page(&editor_page).set_name("editor");
        stack.set_visible_child_name("editor");
        let window = builder
            .object::<adw::ApplicationWindow>("main_window")
            .unwrap();
        window.set_default_size(960, 720);
        window.present();
        while glib::MainContext::default().iteration(false) {}
        for (id, _, accessible_name) in dropdowns {
            let dropdown = builder.object::<gtk::DropDown>(id).unwrap();
            assert_eq!(dropdown.accessible_role(), gtk::AccessibleRole::ComboBox);
            assert!(
                dropdown.is_focusable(),
                "{accessible_name} must remain keyboard focusable"
            );
            assert!(
                dropdown.grab_focus(),
                "{accessible_name} should accept focus"
            );
            while glib::MainContext::default().iteration(false) {}
            assert!(
                dropdown.bounds().is_some_and(|bounds| bounds.3 > 0),
                "{accessible_name} should be realized"
            );
        }
        window.close();
    }

    #[test]
    fn export_inhibits_close_until_completion() {
        assert_eq!(close_policy(true, false, false), ClosePolicy::InhibitExport);
        assert_eq!(close_policy(true, true, false), ClosePolicy::InhibitExport);
        assert_eq!(close_policy(false, false, false), ClosePolicy::Proceed);
        assert_eq!(close_policy(false, false, true), ClosePolicy::CheckDirty);
    }

    #[test]
    fn png_background_summary_exposes_the_saved_document_value_and_overrides() {
        assert_eq!(
            png_background_selection_summary(0, ExportBackground::None),
            "Document Export Background: None (transparent)"
        );
        assert_eq!(
            png_background_selection_summary(
                0,
                ExportBackground::Color {
                    color: RgbaColor {
                        red: 12,
                        green: 34,
                        blue: 56,
                        alpha: 128,
                    },
                }
            ),
            "Document Export Background: #0C223880"
        );
        assert_eq!(
            png_background_selection_summary(1, ExportBackground::None),
            "Transparent Override (ignores saved Export Background)"
        );
        assert_eq!(
            png_background_selection_summary(2, ExportBackground::None),
            "White Override (ignores saved Export Background)"
        );
    }

    #[test]
    fn export_background_color_selection_defaults_to_opaque_white_and_keeps_saved_color() {
        assert_eq!(
            export_background_from_selection(
                0,
                ExportBackground::Color {
                    color: RgbaColor::opaque(12, 34, 56),
                }
            ),
            ExportBackground::None
        );
        assert_eq!(
            export_background_from_selection(1, ExportBackground::None),
            ExportBackground::Color {
                color: RgbaColor::WHITE,
            }
        );
        let saved = ExportBackground::Color {
            color: RgbaColor::opaque(12, 34, 56),
        };
        assert_eq!(export_background_from_selection(1, saved), saved);
        assert_eq!(
            export_background_color_label(ExportBackground::None),
            "Background Color · None (transparent)"
        );
        assert_eq!(
            export_background_color_label(saved),
            "Background Color · #0C2238FF"
        );
    }

    #[test]
    fn explicit_preview_cancel_clears_queued_work_without_waiting_for_old_worker() {
        let active = CancellationToken::new();
        let queued = CancellationToken::new();
        let pending = LatestSlot::default();
        pending.replace(42_u64);
        assert!(active.cancel(), "replacement cancels the running request");
        assert!(
            queued.cancel(),
            "explicit cancel cancels the queued request"
        );
        assert_eq!(
            pending.take(),
            Some(42),
            "queued work is removed immediately"
        );
        assert!(active.is_cancelled());
        assert!(queued.is_cancelled());
        // The active worker can acknowledge later; UI state must not depend on it.
        assert!(pending.take().is_none());
    }

    #[test]
    fn export_close_inhibition_is_format_neutral() {
        for format in ["editable SVG", "PNG image"] {
            assert!(matches!(
                close_policy(true, false, false),
                ClosePolicy::InhibitExport
            ));
            assert!(!EXPORT_CLOSE_INHIBIT_MESSAGE.contains(format));
        }
        assert_eq!(
            EXPORT_CLOSE_INHIBIT_MESSAGE,
            "Please wait for the export to finish before closing."
        );
    }

    #[test]
    fn crosshatch_context_uses_layer_terminology_and_only_selected_layer_angles() {
        assert_eq!(
            crosshatch_context("Shapes", 0, 0.0),
            "Shapes · Crosshatch · All layers"
        );
        assert_eq!(
            crosshatch_context("Curves", 1, 45.0),
            "Curves · Crosshatch · Layer 1 (Black) · 45°"
        );
        assert_eq!(
            crosshatch_context("Shapes", 4, 90.0),
            "Shapes · Crosshatch · Layer 4 (Yellow) · 90°"
        );
    }

    #[test]
    fn rgb_shapes_use_channel_color_copy_without_changing_ink_or_curve_copy() {
        let channel = web_color_copy(true);
        assert_eq!(channel.color_heading, "Channel Color");
        assert_eq!(channel.opacity_heading, "Channel Opacity");
        assert!(channel.color_tooltip.contains("RGB channel"));
        assert_eq!(
            web_color_validation_message(true),
            "Use a six-digit hex channel color such as #111111"
        );
        let ink = web_color_copy(false);
        assert_eq!(ink.color_heading, "Ink Color");
        assert_eq!(ink.opacity_heading, "Ink Opacity");
        assert_eq!(
            web_color_validation_message(false),
            "Use a six-digit hex ink color such as #111111"
        );
        assert_eq!(web_mixed_target(true), "channels");
        assert_eq!(web_mixed_target(false), "inks");

        let mut settings = WebShapeSettings::default();
        assert_eq!(rgb_visibility_summary(&settings), None);
        settings.channels.g.enabled = false;
        assert_eq!(
            rgb_visibility_summary(&settings).as_deref(),
            Some("Visible: Red + Blue")
        );
        settings.channels.r.enabled = false;
        settings.channels.b.enabled = false;
        assert_eq!(
            rgb_visibility_summary(&settings).as_deref(),
            Some("Visible: none")
        );
    }

    #[test]
    fn candidate_generation_is_latest_request_wins() {
        let gate = RenderGate::default();
        let slow_open = gate.next();
        let newer_drop = gate.next();
        assert!(!gate.accepts(slow_open));
        assert!(gate.accepts(newer_drop));
    }

    #[test]
    fn curve_editor_split_and_delete_preserve_a_valid_path() {
        let mut path = CurvePath::straight();
        split_curve_segment(&mut path, 0, 0.5);
        assert_eq!(path.segments.len(), 2);
        let midpoint = path.segments[0].end;
        assert!((midpoint.x).abs() < 1e-9);
        delete_curve_anchor(&mut path, 3);
        assert_eq!(path.segments.len(), 1);
        assert_eq!(path.start, CurvePath::straight().start);
        assert_eq!(path.segments[0].end, CurvePath::straight().segments[0].end);
    }

    #[test]
    fn preset_paths_names_and_bundled_inventory_are_deterministic() {
        assert_eq!(
            user_preset_dir(Some(Path::new("/data")), Some(Path::new("/home/me"))),
            PathBuf::from("/data/toniator/presets")
        );
        assert_eq!(
            user_preset_dir(None, Some(Path::new("/home/me"))),
            PathBuf::from("/home/me/.local/share/toniator/presets")
        );
        assert_eq!(
            normalized_preset_path(Path::new("My Ink")),
            PathBuf::from("My Ink.tntr")
        );
        assert_eq!(preset_name_from_path(Path::new("My Ink.tntr")), "My Ink");
        assert_eq!(
            BUNDLED_PRESETS.map(|item| item.0),
            [
                "Comic Book",
                "Skinny Curve",
                "Chunky Fingerprints",
                "Tiled Stacked Motif Stress Test",
                "Polygon Six",
                "Motif Ladder",
            ]
        );
        assert!(BUNDLED_PRESETS.iter().all(|(_, bytes)| !bytes.is_empty()));
    }

    #[test]
    fn shape_drag_targets_pressed_node_and_empty_drag_changes_nothing() {
        let mut nodes = toniator::model::default_shape_nodes();
        let pressed = ShapePoint { x: 0.44, y: 0.44 };
        let hit = shape_node_hit_test(&nodes, pressed, 0.05);
        assert_eq!(hit, Some(2));
        let origin = nodes[2];
        assert!(update_shape_drag(&mut nodes, hit, origin, -0.1, 0.05));
        assert_eq!(nodes[2], ShapePoint { x: 0.35, y: 0.5 });
        let unchanged = nodes.clone();
        assert_eq!(
            shape_node_hit_test(&nodes, ShapePoint { x: 0.0, y: 0.0 }, 0.05),
            None
        );
        assert!(!update_shape_drag(
            &mut nodes,
            None,
            ShapePoint { x: 0.0, y: 0.0 },
            0.2,
            0.2
        ));
        assert_eq!(nodes, unchanged);
    }

    #[test]
    fn zoom_allocation_is_aspect_safe_scale_aware_and_monotonic() {
        assert_eq!(scaled_artboard_size(3840, 2160, 1.0, 2), (1920, 1080));
        assert_eq!(scaled_artboard_size(900, 600, 1.0, 1), (900, 600));
        let widths: Vec<_> = (5..=40)
            .map(|step| scaled_artboard_size(900, 600, step as f64 * 0.05, 1).0)
            .collect();
        assert!(widths.windows(2).all(|pair| pair[1] > pair[0]));
        assert_eq!(preview_target_dimension(3840, 2160, 2.0), 4096);
        assert_eq!(preview_target_dimension(900, 600, 1.25), 1125);
        assert_eq!(
            preview_target_for_zoom((900, 600), ZoomMode::Explicit(800.0)),
            4096
        );
        assert_eq!(
            preview_target_for_zoom((900, 600), ZoomMode::Explicit(100.0)),
            PREVIEW_DEFAULT_MAX
        );
    }

    #[test]
    fn settings_refresh_at_800_percent_requests_and_installs_latest_4096_raster() {
        use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
        use std::io::Cursor;

        let source = RgbaImage::from_pixel(2, 1, Rgba([0, 0, 0, 255]));
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(source)
            .write_to(&mut encoded, ImageFormat::Png)
            .unwrap();
        let mut document = Document::new(SourceArtwork {
            name: "resolution.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from(encoded.into_inner()),
        });
        let mut settings = toniator::model::WebShapeSettings {
            output_width: 512,
            output_height: 256,
            ..Default::default()
        };
        for ink in toniator::model::Ink::ALL {
            settings.channels.get_mut(ink).enabled = false;
        }
        document.render = RenderVariant::WebShapeV1 {
            settings: Box::new(settings),
        };
        let zoom_mode = ZoomMode::Explicit(800.0);
        let gate = RenderGate::default();

        let initial = build_render_request(gate.next(), &document, false, zoom_mode);
        assert_eq!(initial.max_dimension, PREVIEW_REFINEMENT_MAX);
        let initial_image =
            render_document_preview(&initial.document, initial.max_dimension, initial.generation)
                .unwrap()
                .image;
        assert_eq!(initial_image.dimensions(), (4096, 2048));

        let RenderVariant::WebShapeV1 { settings } = &mut document.render else {
            unreachable!()
        };
        settings.grid_scale = 73.0;
        let refreshed = build_render_request(gate.next(), &document, false, zoom_mode);
        assert_eq!(refreshed.max_dimension, PREVIEW_REFINEMENT_MAX);
        assert!(!gate.accepts(initial.generation));
        let refreshed_image = render_document_preview(
            &refreshed.document,
            refreshed.max_dimension,
            refreshed.generation,
        )
        .unwrap()
        .image;
        assert!(gate.accepts(refreshed.generation));
        assert_eq!(refreshed_image.dimensions(), (4096, 2048));
        eprintln!(
            "production preview flow: explicit zoom=800%; generation {} installed 4096x2048; grid fill changed to 73 without zoom input; generation {} requested max_dimension=4096 and installed 4096x2048 as latest",
            initial.generation, refreshed.generation
        );
    }

    #[test]
    fn fit_zoom_handles_wide_tall_resize_and_explicit_rules() {
        let fit = ZoomMode::Fit(100.0);
        let wide = fit.update_fit((1600, 400), (900, 700), 1);
        let tall = fit.update_fit((400, 1600), (900, 700), 1);
        assert!((wide.percent() - 56.25).abs() < 0.01);
        assert!((tall.percent() - 43.75).abs() < 0.01);
        assert!(fit.update_fit((1600, 400), (1200, 700), 1).percent() > wide.percent());
        let tiny_fit = fit.update_fit((u32::MAX, u32::MAX), (1, 1), 1);
        assert!(matches!(tiny_fit, ZoomMode::Fit(value) if value < ZOOM_MIN));
        for intent in [
            ZoomIntent::Slider(137.25),
            ZoomIntent::Entry(246.75),
            ZoomIntent::Increase,
            ZoomIntent::Decrease,
        ] {
            assert!(matches!(fit.apply_manual(intent), ZoomMode::Explicit(_)));
        }
        assert_eq!(
            ZoomMode::Explicit(12.5)
                .apply_manual(ZoomIntent::Decrease)
                .percent(),
            5.0
        );
        assert_eq!(
            ZoomMode::Explicit(790.5)
                .apply_manual(ZoomIntent::Increase)
                .percent(),
            800.0
        );
        assert_eq!(
            ZoomMode::Explicit(137.25)
                .apply_manual(ZoomIntent::Increase)
                .percent(),
            162.25
        );
        assert_eq!(
            ZoomMode::Fit(144.375)
                .apply_manual(ZoomIntent::Slider(144.38))
                .percent(),
            144.38
        );
        assert_eq!(zoom_percent_text(144.375), "144.375");
    }

    #[test]
    fn fit_uses_full_viewport_with_rounding_hidpi_and_no_clipping() {
        for (artboard, viewport, scale) in [
            ((1600, 400), (901, 701), 1),
            ((400, 1600), (901, 701), 1),
            ((3840, 2160), (1001, 777), 2),
            ((997, 991), (643, 641), 1),
            ((997, 991), (643, 641), 2),
        ] {
            let fitted = fitted_artwork_size(artboard, viewport, scale);
            let deltas = fit_edge_deltas(artboard, viewport, scale);
            assert!(fitted.0 <= viewport.0 && fitted.1 <= viewport.1);
            assert!(
                deltas.0 <= 1 || deltas.1 <= 1,
                "one fitted axis must meet its viewport edge: {artboard:?} {viewport:?} scale={scale} fitted={fitted:?} deltas={deltas:?}"
            );
            let source_aspect = artboard.0 as f64 / artboard.1 as f64;
            let fitted_aspect = fitted.0 as f64 / fitted.1 as f64;
            assert!((source_aspect - fitted_aspect).abs() <= 0.01);
        }
    }

    #[test]
    fn centered_canvas_balances_odd_slack_and_keeps_overflow_scrollable() {
        let wide = CanvasAllocationMetrics::centered((1100, 604), (853, 604));
        assert_eq!(wide.origin, (123, 0));
        assert_eq!(wide.slack, (123, 124, 0, 0));
        assert_eq!(wide.horizontal_delta(), 1);
        assert_eq!(wide.vertical_delta(), 0);

        let tall = CanvasAllocationMetrics::centered((760, 904), (760, 539));
        assert_eq!(tall.origin, (0, 182));
        assert_eq!(tall.slack, (0, 0, 182, 183));
        assert_eq!(tall.horizontal_delta(), 0);
        assert_eq!(tall.vertical_delta(), 1);

        let overflow = CanvasAllocationMetrics::centered((640, 480), (900, 700));
        assert_eq!(overflow.origin, (0, 0));
        assert_eq!(overflow.slack, (0, -260, 0, -220));
        assert_eq!(centered_axis_allocation(901, 640), (130, 130, 131));
    }

    #[test]
    fn fit_enlargement_requests_one_physical_pixel_refinement_and_ignores_unchanged_ticks() {
        let artboard = (1000, 500);
        let small = (artboard, (700, 500), 2);
        let enlarged = (artboard, (1000, 700), 2);
        let mut allocation = FitAllocationState::default();
        let small_token = allocation.observe(small).unwrap();
        assert!(allocation.accepts(small_token));
        let small_mode = ZoomMode::Fit(100.0).update_fit(artboard, small.1, small.2);
        assert_eq!(small_mode.percent(), 140.0);
        assert_eq!(
            fit_refinement_target(artboard, small_mode, Some((1400, 700))),
            None
        );

        let enlarged_token = allocation.observe(enlarged).unwrap();
        assert!(!allocation.accepts(small_token));
        assert!(allocation.accepts(enlarged_token));
        assert_eq!(allocation.observe(enlarged), None);
        let enlarged_mode = small_mode.update_fit(artboard, enlarged.1, enlarged.2);
        assert_eq!(enlarged_mode.percent(), 200.0);
        let target = fit_refinement_target(artboard, enlarged_mode, Some((1400, 700))).unwrap();
        assert_eq!(target, 2000);

        let mut requests = Vec::new();
        if allocation.accepts(enlarged_token) {
            requests.push(target);
        }
        assert_eq!(requests, vec![2000]);
        assert_eq!(allocation.observe(enlarged), None);
        assert_eq!(requests.len(), 1);

        let gate = RenderGate::default();
        let superseded = gate.next();
        let refinement = gate.next();
        assert!(!gate.accepts(superseded));
        assert!(gate.accepts(refinement));
        let installed = Some((2000, 1000));
        assert_eq!(
            fit_refinement_target(artboard, enlarged_mode, installed),
            None
        );

        let shrunk_mode = enlarged_mode.update_fit(artboard, (600, 400), 2);
        assert_eq!(
            fit_refinement_target(artboard, shrunk_mode, installed),
            None
        );
        assert_eq!(
            fit_refinement_target(artboard, ZoomMode::Explicit(200.0), Some((1400, 700))),
            None,
            "explicit zoom resize never enters Fit refinement"
        );
        eprintln!(
            "Fit refinement: 700x500@2x with installed 1400x700 was sufficient; enlargement to 1000x700@2x scheduled exactly one 2000px request, unchanged allocation scheduled none, generation {refinement} installed 2000x1000 as latest; shrink and explicit resize scheduled none"
        );
    }

    #[test]
    fn inspector_width_constraints_and_ui_state_roundtrip() {
        assert_eq!(constrained_inspector_width(400, 1280), 400);
        assert_eq!(constrained_inspector_width(700, 1280), 640);
        assert_eq!(constrained_inspector_width(200, 1280), 340);
        assert_eq!(constrained_inspector_width(500, 760), 400);
        assert_eq!(constrained_inspector_width(500, 650), 290);

        let directory = tempfile::tempdir().unwrap();
        let path = ui_state_path(Some(directory.path()), None);
        assert_eq!(load_inspector_width(&path), INSPECTOR_DEFAULT_WIDTH);
        save_inspector_width(&path, 517).unwrap();
        assert_eq!(load_inspector_width(&path), 517);
        let decoded: UiStateFile = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            decoded,
            UiStateFile {
                version: 1,
                inspector_width: 517
            }
        );
        assert!(!path.ends_with("recovery.toniator"));
    }

    #[cfg(any())]
    fn verify_realized_paned_owns_inspector_width() {
        let canvas = gtk::Box::new(gtk::Orientation::Vertical, 0);
        canvas.set_hexpand(true);
        canvas.set_size_request(CANVAS_MIN_WIDTH, -1);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let scale = control_scale(0.0, 100.0, 1.0);
        content.append(&control_status_row("Detail", "Sample density", &scale).0);
        let dropdown = gtk::DropDown::from_strings(&["A", "A deliberately long choice"]);
        content.append(&combo_row("Mode", &dropdown));
        let inspector = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&content)
            .build();
        inspector.set_size_request(0, -1);
        let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
        paned.set_wide_handle(true);
        paned.set_resize_start_child(false);
        paned.set_resize_end_child(true);
        paned.set_start_child(Some(&inspector));
        paned.set_end_child(Some(&canvas));
        paned.set_position(400);
        let controls_toggle = gtk::ToggleButton::with_label("Controls");
        controls_toggle.set_active(true);
        controls_toggle.set_tooltip_text(Some("Hide Controls"));
        controls_toggle.update_property(&[
            gtk::accessible::Property::Label("Controls"),
            gtk::accessible::Property::Description("Hide Controls"),
        ]);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&controls_toggle);
        root.append(&paned);
        let window = gtk::Window::builder()
            .default_width(1200)
            .default_height(600)
            .child(&root)
            .build();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ui-state.json");
        let controller = InspectorPaneController::new(
            &paned,
            &inspector,
            &controls_toggle,
            400,
            Some(path.clone()),
        );
        let toggle_controller = controller.clone();
        controls_toggle.connect_toggled(move |button| {
            let collapsed = !button.is_active();
            toggle_controller.set_collapsed(collapsed);
            let action = if collapsed {
                "Show Controls"
            } else {
                "Hide Controls"
            };
            button.set_tooltip_text(Some(action));
            button.update_property(&[gtk::accessible::Property::Description(action)]);
        });
        window.present();
        for _ in 0..20 {
            glib::MainContext::default().iteration(false);
            controller.maintain();
        }
        let initial = controller.current_width();
        assert!(
            (initial - 400).abs() <= 10,
            "realized inspector width={initial}, paned width={}, position={}",
            paned.width(),
            paned.position()
        );
        assert_eq!(inspector.hscrollbar_policy(), gtk::PolicyType::Never);
        assert!(dropdown.width() <= inspector.width());
        assert!(paned.start_child().is_some_and(|child| child == inspector));
        assert!(paned.end_child().is_some_and(|child| child == canvas));
        assert!(controls_toggle.is_focusable());
        controls_toggle.emit_clicked();
        while glib::MainContext::default().iteration(false) {}
        assert!(!controls_toggle.is_active());
        assert!(!inspector.is_visible());
        assert_eq!(
            controls_toggle.tooltip_text().as_deref(),
            Some("Show Controls")
        );
        controls_toggle.emit_clicked();
        while glib::MainContext::default().iteration(false) {}
        assert!(controls_toggle.is_active());
        assert!(inspector.is_visible());
        assert_eq!(
            controls_toggle.tooltip_text().as_deref(),
            Some("Hide Controls")
        );

        let status = gtk::Label::new(Some(&"status changed ".repeat(20)));
        status.set_wrap(true);
        status.set_ellipsize(gtk::pango::EllipsizeMode::End);
        status.set_size_request(0, -1);
        content.append(&status);
        dropdown.set_selected(1);
        for _ in 0..10 {
            glib::MainContext::default().iteration(false);
            controller.maintain();
        }
        assert!((controller.current_width() - initial).abs() <= 1);
        for generation in 1..=25 {
            status.set_text(&format!(
                "preview generation {generation} installed; settings synchronized"
            ));
            dropdown.set_selected((generation % 2) as u32);
            glib::MainContext::default().iteration(false);
            controller.maintain();
            assert!((controller.current_width() - initial).abs() <= 1);
        }

        controller.begin_user_drag(paned.position() as f64);
        paned.set_position(paned.position() + 110);
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(5));
            glib::MainContext::default().iteration(false);
        }
        controller.finish_user_drag();
        let deliberate = controller.desired_width.get();
        assert!(
            deliberate > initial + 80,
            "dragged inspector desired={deliberate}, initial={initial}, actual={}, position={}",
            controller.current_width(),
            paned.position()
        );
        assert_eq!(load_inspector_width(&path), deliberate);

        controller.set_collapsed(true);
        for _ in 0..3 {
            glib::MainContext::default().iteration(false);
        }
        assert!(!inspector.is_visible());
        assert_eq!(paned.position(), 0);
        controller.set_collapsed(false);
        for _ in 0..3 {
            controller.maintain();
            glib::MainContext::default().iteration(false);
        }
        assert!(inspector.is_visible());
        assert!((controller.current_width() - deliberate).abs() <= 1);

        for _ in 0..3 {
            paned.allocate(760, 600, -1, None);
            controller.maintain();
        }
        assert_eq!(controller.desired_width.get(), deliberate);
        assert!(
            controller.current_width() <= 400,
            "temporary paned={}, inspector={}, position={}",
            paned.width(),
            controller.current_width(),
            paned.position()
        );
        for _ in 0..3 {
            paned.allocate(1200, 600, -1, None);
            controller.maintain();
        }
        assert!((controller.current_width() - deliberate).abs() <= 1);

        // Narrow windows move the one inspector shell into an overlay instead
        // of squeezing the canvas. The header toggle remains the single
        // discoverable control and returns focus after closing it.
        paned.allocate(760, 600, -1, None);
        controller.update_layout();
        while glib::MainContext::default().iteration(false) {}
        assert!(controller.narrow.get());
        assert!(paned.start_child().is_none());
        assert!(controller.overlay.child().is_some());
        assert!(!controls_toggle.is_active());
        controls_toggle.emit_clicked();
        while glib::MainContext::default().iteration(false) {}
        assert!(controls_toggle.is_active());
        assert!(controller.overlay.is_visible());
        controls_toggle.emit_clicked();
        while glib::MainContext::default().iteration(false) {}
        assert!(!controls_toggle.is_active());
        controls_toggle.grab_focus();
        assert!(controls_toggle.is_focusable());
        paned.allocate(1200, 600, -1, None);
        controller.update_layout();
        while glib::MainContext::default().iteration(false) {}
        assert!(!controller.narrow.get());
        assert!(paned.start_child().is_some());
        eprintln!(
            "paned stability: 25 repeated settings/status/preview-install mutations kept inspector={initial}px; deliberate drag persisted desired={deliberate}px; temporary 760px window clamp preserved desired and 1200px restore returned inspector={deliberate}px"
        );
        let help_spec = help_for("Artwork Mapping").unwrap();
        let help_handle = help_handle(help_spec);
        let help_window = gtk::Window::builder().child(&help_handle.button).build();
        help_window.present();
        while glib::MainContext::default().iteration(false) {}
        assert!(help_handle.button.is_focusable());
        assert_eq!(
            help_handle.button.tooltip_text().as_deref(),
            Some(help_spec.summary)
        );
        assert!(help_handle.button.popover().is_some());
        help_handle.button.popup();
        assert!(help_handle.popover.is_visible());
        help_handle.set_spec(help_for("Adjust Layer").unwrap());
        assert_eq!(help_handle.heading.text(), "Adjust Layer");
        assert!(help_handle.body.text().contains("directional hatch"));
        let visible_help = super::help_handle(help_for("Visible Inks").unwrap());
        visible_help.set_spec(help_for("Visible Crosshatch Layers").unwrap());
        assert_eq!(visible_help.heading.text(), "Visible Crosshatch Layers");
        assert!(
            visible_help
                .button
                .tooltip_text()
                .unwrap()
                .contains("hatch")
        );
        visible_help.set_spec(help_for("Visible Inks").unwrap());
        assert_eq!(visible_help.heading.text(), "Visible Inks");
        help_handle.popover.popdown();
        help_window.close();
        let conditional_action = gtk::Button::with_label("Conditional Action");
        let conditional_row = button_with_help(&conditional_action, "Edit User-Defined Mark");
        let conditional_window = gtk::Window::builder().child(&conditional_row).build();
        conditional_window.present();
        while glib::MainContext::default().iteration(false) {}
        conditional_action.set_visible(false);
        assert!(!conditional_row.is_visible());
        conditional_window.close();
        window.close();
    }

    fn verify_realized_overlay_split_owns_inspector_width() {
        register_ui_resources();
        let builder = gtk::Builder::from_resource(WINDOW_UI_RESOURCE);
        let split = builder
            .object::<adw::OverlaySplitView>("editor_split_view")
            .unwrap();
        let controls = builder
            .object::<gtk::ToggleButton>("controls_toggle")
            .unwrap();
        let controller = InspectorPaneController::new(&split, &controls, 400);
        let window = builder
            .object::<adw::ApplicationWindow>("main_window")
            .unwrap();
        window.set_default_size(1200, 720);
        window.present();
        for _ in 0..10 {
            glib::MainContext::default().iteration(false);
            controller.maintain();
        }
        assert!(controller.current_width() >= 0);
        controller.set_collapsed(true);
        while glib::MainContext::default().iteration(false) {}
        assert!(!split.shows_sidebar());
        controller.set_collapsed(false);
        while glib::MainContext::default().iteration(false) {}
        assert!(split.shows_sidebar());
        window.close();
    }

    fn verify_realized_zoom_controls_drive_one_canonical_mode_and_actual_allocation() {
        let fit = gtk::ToggleButton::with_label("Fit");
        let minus = gtk::Button::with_label("-");
        let zoom = gtk::Scale::with_range(gtk::Orientation::Horizontal, ZOOM_MIN, ZOOM_MAX, 1.0);
        let entry = gtk::Entry::new();
        let plus = gtk::Button::with_label("+");
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        for widget in [
            fit.clone().upcast::<gtk::Widget>(),
            minus.clone().upcast(),
            zoom.clone().upcast(),
            entry.clone().upcast(),
            plus.clone().upcast(),
        ] {
            controls.append(&widget);
        }
        let artwork = gtk::Box::new(gtk::Orientation::Vertical, 0);
        artwork.set_size_request(1600, 400);
        let canvas = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .child(&artwork)
            .build();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&canvas);
        root.append(&controls);
        let window = gtk::Window::builder()
            .default_width(900)
            .default_height(700)
            .child(&root)
            .build();
        let mode = Rc::new(Cell::new(ZoomMode::Fit(100.0)));
        let artboard = Rc::new(Cell::new((1600, 400)));
        let syncing = Rc::new(Cell::new(false));
        let synchronize = {
            let mode = Rc::clone(&mode);
            let artboard = Rc::clone(&artboard);
            let syncing = Rc::clone(&syncing);
            let canvas = canvas.clone();
            let fit = fit.clone();
            let zoom = zoom.clone();
            let entry = entry.clone();
            Rc::new(move || {
                let next = match mode.get() {
                    ZoomMode::Fit(_) => mode.get().update_fit(
                        artboard.get(),
                        (canvas.width().max(1), canvas.height().max(1)),
                        1,
                    ),
                    explicit => explicit,
                };
                mode.set(next);
                syncing.set(true);
                sync_zoom_control_widgets(
                    &fit,
                    &zoom,
                    &entry,
                    next.percent(),
                    matches!(next, ZoomMode::Fit(_)),
                );
                syncing.set(false);
            })
        };
        connect_zoom_control_commands(&fit, &minus, &zoom, &entry, &plus, {
            let mode = Rc::clone(&mode);
            let syncing = Rc::clone(&syncing);
            let synchronize = Rc::clone(&synchronize);
            Rc::new(move |command| {
                if syncing.get() {
                    return;
                }
                let next = match command {
                    ZoomControlCommand::Fit => ZoomMode::Fit(mode.get().percent()),
                    ZoomControlCommand::Manual(intent) => mode.get().apply_manual(intent),
                    ZoomControlCommand::Entry(text) => {
                        let value = text
                            .trim()
                            .trim_end_matches('%')
                            .parse()
                            .unwrap_or(mode.get().percent());
                        mode.get().apply_manual(ZoomIntent::Entry(value))
                    }
                };
                mode.set(next);
                synchronize();
            })
        });
        canvas.add_tick_callback({
            let synchronize = Rc::clone(&synchronize);
            move |_, _| {
                synchronize();
                glib::ControlFlow::Continue
            }
        });
        window.present();
        for _ in 0..20 {
            glib::MainContext::default().iteration(false);
        }

        synchronize();
        let wide = mode.get().percent();
        assert!(fit.is_active());
        assert!((zoom.value() - wide).abs() < 1e-9);
        assert_eq!(entry.text(), zoom_percent_text(wide));

        artboard.set((400, 1600));
        artwork.set_size_request(400, 1600);
        fit.emit_clicked();
        for _ in 0..10 {
            glib::MainContext::default().iteration(false);
        }
        let tall = mode.get().percent();
        assert!(matches!(mode.get(), ZoomMode::Fit(_)));
        assert!(tall < wide);

        window.set_default_size(1200, 700);
        for _ in 0..30 {
            glib::MainContext::default().iteration(false);
        }
        assert!(matches!(mode.get(), ZoomMode::Fit(_)));
        assert!(mode.get().percent() >= tall);

        zoom.set_value(137.0);
        assert!(matches!(mode.get(), ZoomMode::Explicit(137.0)));
        assert!(!fit.is_active());
        entry.set_text("246.75%");
        entry.emit_activate();
        assert!((mode.get().percent() - 246.75).abs() < 1e-9);
        plus.emit_clicked();
        assert!((mode.get().percent() - 271.75).abs() < 1e-9);
        minus.emit_clicked();
        assert!((mode.get().percent() - 246.75).abs() < 1e-9);
        entry.set_text("900");
        entry.emit_activate();
        assert_eq!(mode.get().percent(), 800.0);
        plus.emit_clicked();
        assert_eq!(mode.get().percent(), 800.0);
        entry.set_text("1");
        entry.emit_activate();
        assert_eq!(mode.get().percent(), 5.0);
        minus.emit_clicked();
        assert_eq!(mode.get().percent(), 5.0);

        artboard.set((u32::MAX, u32::MAX));
        fit.emit_clicked();
        synchronize();
        assert!(matches!(mode.get(), ZoomMode::Fit(value) if value < ZOOM_MIN));
        assert_eq!(zoom.value(), mode.get().percent());
        assert_eq!(entry.text(), zoom_percent_text(mode.get().percent()));
        window.close();
    }

    #[test]
    fn preview_activity_ignores_superseded_completion() {
        let mut activity = PreviewActivity::default();
        activity.request(1, PreviewView::Rendered);
        assert!(activity.active());
        assert!(activity.render_busy());
        activity.request(2, PreviewView::Rendered);
        activity.installed(1, PreviewView::Rendered);
        assert!(activity.active());
        activity.failed(1);
        assert!(activity.active());
        activity.installed(2, PreviewView::Rendered);
        assert!(!activity.active());
        assert_eq!(activity.terminal, Some((2, PreviewTerminal::Installed)));
        assert_eq!(activity.resting_phase(), 1.0);
        activity.request(3, PreviewView::Rendered);
        activity.failed(3);
        assert!(!activity.active());
        assert_eq!(activity.terminal, Some((3, PreviewTerminal::Failed)));
        assert_eq!(activity.resting_phase(), 1.0);
        activity.request(4, PreviewView::Source);
        assert!(activity.active());
        assert!(!activity.render_busy());
        assert_eq!(activity.resting_phase(), 0.0);
        assert_eq!(activity.accessible_label(), "Source preview");
        activity.installed(4, PreviewView::Source);
        assert!(!activity.active());
        assert_eq!(activity.terminal, Some((4, PreviewTerminal::Installed)));
        activity.request(5, PreviewView::Rendered);
        assert!(activity.render_busy());
        assert_eq!(activity.accessible_label(), "Updating halftone preview");
        activity.failed(5);
        assert_eq!(activity.resting_phase(), 0.0);
        assert_eq!(activity.accessible_label(), "Source preview");
    }

    #[test]
    fn preview_activity_cancellation_settles_only_the_matching_request() {
        let mut activity = PreviewActivity {
            installed: Some((1, PreviewView::Source)),
            ..PreviewActivity::default()
        };
        activity.request(2, PreviewView::Rendered);
        assert!(activity.render_busy());
        activity.cancelled(2);
        assert!(!activity.render_busy());
        assert!(!activity.active());
        assert_eq!(activity.installed, Some((1, PreviewView::Source)));
        assert_eq!(activity.resting_phase(), 0.0);
        assert_eq!(activity.accessible_label(), "Source preview");

        activity.request(3, PreviewView::Rendered);
        activity.cancelled(2);
        assert!(
            activity.render_busy(),
            "stale cancellation must not settle request 3"
        );
        activity.cancelled(3);
        assert!(!activity.render_busy());
    }

    #[test]
    fn artifact_readiness_waits_for_newest_desired_render_and_surfaces_failure() {
        let mut activity = PreviewActivity::default();
        activity.request(10, PreviewView::Rendered);
        assert_eq!(
            artifact_preview_readiness(&activity, PreviewView::Rendered, false, false),
            ArtifactPreviewReadiness::Waiting
        );
        activity.request(11, PreviewView::Rendered);
        activity.installed(10, PreviewView::Rendered);
        assert_eq!(
            artifact_preview_readiness(&activity, PreviewView::Rendered, true, true),
            ArtifactPreviewReadiness::Waiting,
            "a delayed superseded completion cannot release screenshot capture"
        );
        activity.installed(11, PreviewView::Rendered);
        assert_eq!(
            artifact_preview_readiness(&activity, PreviewView::Rendered, false, true),
            ArtifactPreviewReadiness::Waiting,
            "the accepted render must also populate a sufficient matching cache"
        );
        assert_eq!(
            artifact_preview_readiness(&activity, PreviewView::Rendered, true, true),
            ArtifactPreviewReadiness::Ready
        );

        activity.request(12, PreviewView::Rendered);
        activity.failed(12);
        assert_eq!(
            artifact_preview_readiness(&activity, PreviewView::Rendered, false, true),
            ArtifactPreviewReadiness::Failed,
            "terminal newest-render failure must fail artifacts instead of hanging"
        );
        activity.requested = None;
        activity.terminal = None;
        activity.installed = Some((0, PreviewView::Source));
        assert_eq!(
            artifact_preview_readiness(&activity, PreviewView::Source, true, true),
            ArtifactPreviewReadiness::Ready,
            "an already installed sufficient source view is immediately capturable"
        );
    }

    #[test]
    fn preview_animation_is_eased_bounded_and_ping_pongs() {
        assert_eq!(preview_animation_phase(Duration::ZERO, false), 0.0);
        assert!((preview_animation_phase(Duration::from_millis(900), false) - 0.5).abs() < 1e-9);
        assert!((preview_animation_phase(Duration::from_millis(1800), false) - 1.0).abs() < 1e-9);
        assert!((preview_animation_phase(Duration::from_millis(2700), false) - 0.5).abs() < 1e-9);
        assert!(preview_animation_phase(Duration::from_millis(250), false) < 250.0 / 1800.0);
        for milliseconds in 0..=7200 {
            let phase = preview_animation_phase(Duration::from_millis(milliseconds), false);
            assert!((0.0..=1.0).contains(&phase));
        }
        assert_eq!(preview_animation_phase(Duration::ZERO, true), 0.5);
        assert_eq!(preview_animation_phase(Duration::from_secs(99), true), 0.5);
        assert_eq!(preview_indicator_layers(0.0), (1.0, 0.0));
        assert_eq!(preview_indicator_layers(0.5), (0.5, 0.5));
        assert_eq!(preview_indicator_layers(1.0), (0.0, 1.0));
    }

    #[test]
    fn preview_indicator_geometry_is_svg_backed_and_layered() {
        let source = std::str::from_utf8(PREVIEW_INDICATOR_SVG).expect("indicator is UTF-8 SVG");
        assert!(source.contains("viewBox=\"0 0 40 28\""));
        assert!(source.contains("id=\"solid-t\""));
        assert!(source.contains("id=\"halftone-dots\""));

        let tree = usvg::Tree::from_data(PREVIEW_INDICATOR_SVG, &usvg::Options::default())
            .expect("embedded indicator parses");
        assert_eq!(tree.size().width(), 40.0);
        assert_eq!(tree.size().height(), 28.0);
        let usvg::Node::Group(solid) = tree.node_by_id("solid-t").expect("solid group") else {
            panic!("solid-t must remain a named SVG group");
        };
        let usvg::Node::Group(dots) = tree.node_by_id("halftone-dots").expect("halftone group")
        else {
            panic!("halftone-dots must remain a named SVG group");
        };
        assert_eq!(solid.children().len(), 1);
        assert_eq!(solid.children()[0].id(), "solid-t-shape");
        assert!(dots.children().len() >= 12);
        assert!(
            dots.children()
                .iter()
                .all(|node| node.id().starts_with("dot-"))
        );
        assert!(PreviewIndicatorArtwork::from_embedded_svg().is_ok());
    }

    fn rendered_indicator_pixels(phase: f64, color: gdk::RGBA) -> Vec<u8> {
        let artwork = PreviewIndicatorArtwork::from_embedded_svg().expect("indicator artwork");
        let mut surface = gtk::cairo::ImageSurface::create(
            gtk::cairo::Format::ARgb32,
            PREVIEW_INDICATOR_WIDTH,
            PREVIEW_INDICATOR_HEIGHT,
        )
        .expect("test surface");
        {
            let cr = gtk::cairo::Context::new(&surface).expect("test context");
            draw_preview_indicator(
                &cr,
                PREVIEW_INDICATOR_WIDTH,
                PREVIEW_INDICATOR_HEIGHT,
                phase,
                color,
                &artwork,
            );
        }
        surface.flush();
        surface.data().expect("surface pixels").to_vec()
    }

    #[test]
    fn preview_indicator_endpoints_and_theme_tint_are_deterministic() {
        let red_source = rendered_indicator_pixels(0.0, gdk::RGBA::new(1.0, 0.0, 0.0, 1.0));
        let red_rendered = rendered_indicator_pixels(1.0, gdk::RGBA::new(1.0, 0.0, 0.0, 1.0));
        let blue_rendered = rendered_indicator_pixels(1.0, gdk::RGBA::new(0.0, 0.0, 1.0, 1.0));
        assert_ne!(red_source, red_rendered, "solid and dot endpoints differ");

        let alpha = |pixels: &[u8]| {
            pixels
                .chunks_exact(4)
                .map(|pixel| u64::from(pixel[3]))
                .sum::<u64>()
        };
        assert_eq!(alpha(&red_rendered), alpha(&blue_rendered));
        assert!(red_rendered.chunks_exact(4).any(|pixel| pixel[2] > 0));
        assert!(red_rendered.chunks_exact(4).all(|pixel| pixel[0] == 0));
        assert!(blue_rendered.chunks_exact(4).any(|pixel| pixel[0] > 0));
        assert!(blue_rendered.chunks_exact(4).all(|pixel| pixel[2] == 0));
    }

    #[test]
    fn curve_mapping_is_isotropic_and_anchor_translation_preserves_vectors() {
        let origin = curve_to_editor_point(CurvePoint::default(), 640, 280);
        let x = curve_to_editor_point(CurvePoint { x: 0.2, y: 0.0 }, 640, 280);
        let y = curve_to_editor_point(CurvePoint { x: 0.0, y: 0.2 }, 640, 280);
        assert!(((x.0 - origin.0).abs() - (y.1 - origin.1).abs()).abs() < 1e-9);
        let mut path = CurvePath::soft_wave();
        let old_anchor = path.segments[0].end;
        let old_in = path.segments[0].control_2;
        let old_out = path.segments[1].control_1;
        set_curve_handle(
            &mut path,
            3,
            CurvePoint {
                x: old_anchor.x + 0.1,
                y: old_anchor.y - 0.05,
            },
        );
        assert_eq!(path.segments[0].control_2.x - old_in.x, 0.1);
        assert_eq!(path.segments[1].control_1.y - old_out.y, -0.05);
    }

    #[test]
    fn cubic_shape_split_is_exact_and_anchor_move_carries_handles() {
        let mut path = ClosedShapePath::from_polygon(&toniator::model::default_shape_nodes());
        let before = cubic_shape_point(path.anchors[0], path.anchors[1], 0.5);
        split_shape_segment(&mut path, 0, 0.5);
        assert_eq!(path.anchors.len(), 5);
        assert_eq!(path.anchors[1].point, before);
        let old = path.anchors[1];
        translate_shape_anchor(
            &mut path,
            1,
            ShapePoint {
                x: old.point.x + 0.1,
                y: old.point.y + 0.2,
            },
        );
        assert!((path.anchors[1].incoming.x - old.incoming.x - 0.1).abs() < 1e-9);
        assert!((path.anchors[1].outgoing.y - old.outgoing.y - 0.2).abs() < 1e-9);
        let active = path.anchors[1];
        path.anchors[1].outgoing.x += 0.2;
        assert_eq!(path.anchors[1].point, active.point);
        assert_eq!(path.anchors[1].incoming, active.incoming);
        assert!(delete_shape_anchor(&mut path, 1));
        assert_eq!(path.anchors.len(), 4);
        assert!(delete_shape_anchor(&mut path, 1));
        assert_eq!(path.anchors.len(), 3);
        assert!(!delete_shape_anchor(&mut path, 1));
        assert!(nearest_shape_segment(&path, ShapePoint { x: 1.4, y: 1.4 }, 0.02).is_none());
    }

    #[test]
    fn nearest_shape_insertion_is_atomic_exact_and_safe_for_empty_or_far_paths() {
        let mut path = curved_shape_fixture();
        for _ in 0..4 {
            let before_len = path.anchors.len();
            let original_start = path.anchors[0];
            let original_end = path.anchors[1];
            let expected = cubic_shape_point(original_start, original_end, 0.5);
            let inserted = insert_nearest_shape_anchor(&mut path, expected, 0.02)
                .expect("known on-curve point must insert");
            assert_eq!(path.anchors.len(), before_len + 1);
            let actual = path.anchors[inserted].point;
            assert!((actual.x - expected.x).hypot(actual.y - expected.y) < 0.02);
            for step in 0..=100 {
                let t = step as f64 / 100.0;
                let before = cubic_shape_point(original_start, original_end, t);
                let after = if t <= 0.5 {
                    cubic_shape_point(path.anchors[0], path.anchors[inserted], t * 2.0)
                } else {
                    cubic_shape_point(
                        path.anchors[inserted],
                        path.anchors[inserted + 1],
                        (t - 0.5) * 2.0,
                    )
                };
                assert!((after.x - before.x).hypot(after.y - before.y) < 1e-12);
            }
        }
        let unchanged = path.clone();
        assert_eq!(
            insert_nearest_shape_anchor(&mut path, ShapePoint { x: 9.0, y: 9.0 }, 0.01),
            None
        );
        assert_eq!(path, unchanged);
        let mut empty = ClosedShapePath { anchors: vec![] };
        assert_eq!(
            insert_nearest_shape_anchor(&mut empty, ShapePoint { x: 0.0, y: 0.0 }, 1.0),
            None
        );
        assert!(empty.anchors.is_empty());
    }

    fn verify_realized_inspector_shell_keeps_controls_visible() {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let known_control = gtk::DropDown::from_strings(&["Circle", "Regular Polygon"]);
        content.append(&combo_row("Mark", &known_control));
        for index in 0..20 {
            content.append(&gtk::Label::new(Some(&format!("Control group {index}"))));
        }
        let scroll = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&content)
            .build();
        let context = gtk::Label::new(Some("Shapes · All inks"));
        let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
        shell.set_vexpand(true);
        shell.append(&context);
        shell.append(&scroll);
        let window = gtk::Window::builder()
            .default_width(400)
            .default_height(600)
            .child(&shell)
            .build();
        window.present();
        for _ in 0..20 {
            glib::MainContext::default().iteration(false);
        }
        assert!(context.height() > 0);
        assert!(scroll.height() > 400, "scroll height={}", scroll.height());
        assert!(known_control.is_visible());
        assert!(
            known_control.height() > 0,
            "known control was not allocated"
        );
        assert!(known_control.compute_bounds(&window).is_some());
        window.close();
    }

    fn verify_realized_shape_double_clicks() {
        let path = Rc::new(RefCell::new(curved_shape_fixture()));
        let nodes = Rc::new(RefCell::new(
            path.borrow()
                .anchors
                .iter()
                .map(|anchor| anchor.point)
                .collect(),
        ));
        let selected = Rc::new(Cell::new(0));
        let selected_part = Rc::new(Cell::new(0));
        let area = gtk::DrawingArea::builder()
            .content_width(520)
            .content_height(360)
            .focusable(true)
            .build();
        let click = connect_shape_editor_click(&area, &nodes, &path, &selected, &selected_part);
        let window = gtk::Window::builder()
            .default_width(520)
            .default_height(360)
            .child(&area)
            .build();
        window.present();
        while glib::MainContext::default().iteration(false) {}
        for expected_len in 5..=8 {
            let target = {
                let path = path.borrow();
                cubic_shape_point(path.anchors[0], path.anchors[1], 0.5)
            };
            let side = area.width().min(area.height()) as f64 * 0.82;
            let x = area.width() as f64 / 2.0 + target.x * side;
            let y = area.height() as f64 / 2.0 + target.y * side;
            click.emit_by_name::<()>("pressed", &[&2i32, &x, &y]);
            while glib::MainContext::default().iteration(false) {}
            assert_eq!(path.borrow().anchors.len(), expected_len);
            assert_eq!(nodes.borrow().len(), expected_len);
            assert!(selected.get() < expected_len);
            assert_eq!(selected_part.get(), 0);
        }
        let before = path.borrow().clone();
        let selected_before = selected.get();
        click.emit_by_name::<()>("pressed", &[&2i32, &1.0f64, &1.0f64]);
        while glib::MainContext::default().iteration(false) {}
        assert_eq!(*path.borrow(), before);
        assert_eq!(selected.get(), selected_before);
        eprintln!(
            "realized User Defined editor check: production GestureClick seam accepted four repeated double-click insertions (4 -> 8 anchors), preserved selection, and ignored a far double-click without panic"
        );
        window.close();
    }

    fn shape_editor_descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut pending = vec![root.clone()];
        let mut descendants = Vec::new();
        while let Some(widget) = pending.pop() {
            let mut child = widget.first_child();
            while let Some(current) = child {
                child = current.next_sibling();
                pending.push(current);
            }
            descendants.push(widget);
        }
        descendants
    }

    fn shape_editor_area(dialog: &gtk::Window) -> gtk::DrawingArea {
        shape_editor_descendants(&dialog.clone().upcast())
            .into_iter()
            .find_map(|widget| widget.downcast::<gtk::DrawingArea>().ok())
            .expect("shape editor dialog must contain its drawing area")
    }

    fn shape_editor_button(dialog: &gtk::Window, label: &str) -> gtk::Button {
        shape_editor_descendants(&dialog.clone().upcast())
            .into_iter()
            .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
            .find(|button| button.label().as_deref() == Some(label))
            .unwrap_or_else(|| panic!("shape editor dialog must contain {label}"))
    }

    fn shape_editor_click(area: &gtk::DrawingArea) -> gtk::GestureClick {
        let controllers = area.observe_controllers();
        (0..controllers.n_items())
            .filter_map(|index| controllers.item(index))
            .find_map(|controller| controller.downcast::<gtk::GestureClick>().ok())
            .expect("shape editor drawing area must retain its click controller")
    }

    fn verify_realized_resource_shape_editor_authority_workflow() {
        let application = adw::Application::builder()
            .application_id("dev.toniator.shape-editor-resource-authority")
            .build();
        application.register(None::<&gio::Cancellable>).unwrap();
        let ui = AppUi::new(
            &application,
            CliOptions {
                demo: true,
                edit_shape: true,
                artifact_window_size: Some((1000, 760)),
                ..CliOptions::default()
            },
        );
        ui.window.present();
        drain_ui_callbacks();

        // The artifact fixture opens once during construction. Close it, then
        // enter through the production Blueprint button below.
        ui.capture_override
            .borrow()
            .as_ref()
            .expect("artifact fixture must expose its shape editor dialog")
            .close();
        drain_ui_callbacks();
        ui.controls_toggle.set_active(true);
        drain_ui_callbacks();
        ui.dots.set_active(true);
        drain_ui_callbacks();
        ui.install_curved_shape_fixture();
        drain_ui_callbacks();

        let mut authoritative = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .clone();
        let before = authoritative
            .pattern_state
            .shape_settings()
            .unwrap()
            .resolved_custom_shape_path();
        authoritative.render = RenderVariant::WebShapeV1 {
            settings: Box::new(WebShapeSettings::default()),
        };
        ui.state.borrow_mut().editor = Some(DocumentEditor::new(authoritative));
        ui.sync_controls();
        drain_ui_callbacks();

        let descriptor = PATTERN_REGISTRY
            .parameter_for_control(PatternId::COMPATIBILITY_SHAPES_V1, "web_edit_shape")
            .unwrap();
        assert!(ui.web_edit_shape.is_visible());
        assert!(ui.web_edit_shape.is_sensitive());
        assert!(ui.web_edit_shape.is_focusable());
        assert_eq!(ui.web_edit_shape.label().as_deref(), Some(descriptor.label));
        assert!(ui.web_edit_shape.parent().is_some());

        ui.web_edit_shape.emit_clicked();
        drain_ui_callbacks();
        let dialog = ui
            .capture_override
            .borrow()
            .as_ref()
            .cloned()
            .expect("the production entry control must open the editor dialog");
        assert!(dialog.is_visible());
        let area = shape_editor_area(&dialog);
        assert!(area.is_focusable());
        assert_eq!(
            gtk::prelude::GtkWindowExt::focus(&dialog),
            Some(area.clone().upcast()),
            "shape editor canvas must become the dialog's focused widget"
        );
        assert_eq!(
            area.tooltip_text().as_deref(),
            Some("Edit the selected mark's anchors and independent Bézier handles.")
        );

        let click = shape_editor_click(&area);
        let target = cubic_shape_point(before.anchors[0], before.anchors[1], 0.5);
        let side = area.width().min(area.height()) as f64 * 0.82;
        let x = area.width() as f64 / 2.0 + target.x * side;
        let y = area.height() as f64 / 2.0 + target.y * side;
        click.emit_by_name::<()>("pressed", &[&2i32, &x, &y]);
        drain_ui_callbacks();
        shape_editor_button(&dialog, "Done").emit_clicked();
        drain_ui_callbacks();

        let completed = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .pattern_state
            .shape_settings()
            .unwrap()
            .resolved_custom_shape_path();
        assert_eq!(completed.anchors.len(), before.anchors.len() + 4);
        assert_ne!(completed, before);
        assert!(ui.web_edit_shape.is_visible());
        ui.undo();
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .pattern_state
                .shape_settings()
                .unwrap()
                .resolved_custom_shape_path(),
            before
        );
        ui.redo();
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .pattern_state
                .shape_settings()
                .unwrap()
                .resolved_custom_shape_path(),
            completed
        );

        ui.web_edit_shape.emit_clicked();
        drain_ui_callbacks();
        let cancelled_dialog = ui
            .capture_override
            .borrow()
            .as_ref()
            .cloned()
            .expect("the entry control must reopen the editor");
        shape_editor_button(&cancelled_dialog, "Cancel").emit_clicked();
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .pattern_state
                .shape_settings()
                .unwrap()
                .resolved_custom_shape_path(),
            completed
        );
        assert!(!cancelled_dialog.is_visible());
        assert!(ui.window.is_visible());
        assert!(ui.web_edit_shape.is_visible());
        assert_eq!(
            ui.treatment_modes.visible_child_name().as_deref(),
            Some("web")
        );
        ui.inspector_pane.paned.allocate(760, 600, -1, None);
        ui.inspector_pane.update_layout();
        drain_ui_callbacks();
        assert!(ui.inspector_pane.narrow.get());
        assert!(ui.inspector_pane.paned.is_collapsed());
        assert!(!ui.inspector_pane.paned.shows_sidebar());
        assert!(!ui.controls_toggle.is_active());
        ui.controls_toggle.emit_clicked();
        drain_ui_callbacks();
        assert!(ui.controls_toggle.is_active());
        assert!(ui.inspector_pane.paned.shows_sidebar());
        ui.controls_toggle.emit_clicked();
        drain_ui_callbacks();
        assert!(!ui.controls_toggle.is_active());
        ui.inspector_pane.paned.allocate(1000, 600, -1, None);
        ui.inspector_pane.update_layout();
        drain_ui_callbacks();
        assert!(!ui.inspector_pane.narrow.get());
        assert!(!ui.inspector_pane.paned.is_collapsed());
        eprintln!(
            "realized resource User Defined editor check: Blueprint entry opened the GResource-backed dialog, authoritative state overrode a contradictory adapter, a production double-click insertion committed as one undoable edit, and Cancel returned to Shapes without persistence"
        );
        ui.window.close();
    }

    fn verify_realized_resource_polygon_sides_authority_workflow() {
        let application = adw::Application::builder()
            .application_id("dev.toniator.polygon-sides-resource-authority")
            .build();
        application.register(None::<&gio::Cancellable>).unwrap();
        let ui = AppUi::new(
            &application,
            CliOptions {
                demo: true,
                artifact_controls_shown: true,
                artifact_window_size: Some((1000, 760)),
                ..CliOptions::default()
            },
        );
        ui.window.present();
        ui.activate_shape_treatment();
        drain_ui_callbacks();

        let descriptor = PATTERN_REGISTRY
            .parameter_for_control(PatternId::COMPATIBILITY_SHAPES_V1, "web_polygon_sides")
            .unwrap();
        assert_eq!(ui.web_shape.selected(), 0);
        assert_eq!(ui.web_target.selected(), 0);
        assert_eq!(ui.web_polygon_sides.value_as_int(), 4);
        assert!(!ui.web_polygon_sides.is_visible());
        assert_eq!(ui.web_polygon_sides_label.text(), descriptor.label);
        assert_eq!(
            ui.web_polygon_sides.tooltip_text().as_deref(),
            Some(descriptor.help)
        );

        assert!(!ui.state.borrow().syncing_controls);
        ui.web_shape.set_selected(1);
        drain_ui_callbacks();
        let selected_settings = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .pattern_state
            .shape_settings()
            .unwrap();
        assert!(selected_settings.use_shared_mark);
        assert_eq!(selected_settings.shared_shape, WebShape::RegularPolygon);
        ui.sync_controls();
        assert!(ui.web_polygon_sides.is_visible());
        assert!(ui.web_polygon_sides.is_sensitive());
        assert_eq!(ui.web_polygon_sides.value_as_int(), 4);
        assert!(
            ui.web_polygon_sides
                .parent()
                .is_some_and(|row| row.is_visible())
        );

        let source = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .clone();
        let mut editor = DocumentEditor::new(source);
        let mut authoritative_settings = editor.document().pattern_state.shape_settings().unwrap();
        authoritative_settings.shared_shape = WebShape::RegularPolygon;
        authoritative_settings.polygon_sides = 6;
        for ink in output_channel_order(false, false).iter().copied() {
            authoritative_settings.channels.get_mut(ink).polygon_sides = 6;
        }
        assert!(editor.set_shape_settings(authoritative_settings));
        let mut contradictory = editor.document().clone();
        contradictory.render = RenderVariant::WebShapeV1 {
            settings: Box::new(WebShapeSettings {
                shared_shape: WebShape::Circle,
                polygon_sides: 3,
                ..Default::default()
            }),
        };
        ui.state.borrow_mut().editor = Some(DocumentEditor::new(contradictory));
        ui.sync_controls();
        drain_ui_callbacks();
        assert_eq!(ui.web_shape.selected(), 1);
        assert_eq!(ui.web_polygon_sides.value_as_int(), 6);
        assert!(ui.web_polygon_sides.is_visible());

        ui.web_polygon_sides.set_value(3.0);
        drain_ui_callbacks();
        let shared_three = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .clone();
        let shared_three_settings = shared_three.pattern_state.shape_settings().unwrap();
        assert_eq!(shared_three_settings.polygon_sides, 3);
        assert!(
            output_channel_order(false, false)
                .iter()
                .copied()
                .all(|ink| shared_three_settings.channels.get(ink).polygon_sides == 3)
        );
        assert!(matches!(
            shared_three.render,
            RenderVariant::WebShapeV1 { settings }
                if settings.shared_shape == WebShape::RegularPolygon && settings.polygon_sides == 3
        ));

        ui.web_polygon_sides.set_value(6.0);
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .pattern_state
                .shape_settings()
                .unwrap()
                .polygon_sides,
            6
        );

        ui.web_shape.set_selected(0);
        drain_ui_callbacks();
        assert!(!ui.web_polygon_sides.is_visible());
        ui.web_shape.set_selected(2);
        drain_ui_callbacks();
        assert!(!ui.web_polygon_sides.is_visible());
        assert!(ui.web_edit_shape.is_visible());

        ui.web_shape.set_selected(1);
        drain_ui_callbacks();
        ui.web_shared.set_active(false);
        drain_ui_callbacks();
        ui.web_target.set_selected(2); // Magenta only.
        drain_ui_callbacks();
        ui.web_polygon_sides.set_value(3.0);
        drain_ui_callbacks();
        let per_target = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .pattern_state
            .shape_settings()
            .unwrap();
        assert!(!per_target.use_shared_mark);
        assert_eq!(per_target.channels.m.polygon_sides, 3);
        assert_eq!(per_target.channels.c.polygon_sides, 6);
        ui.web_target.set_selected(0);
        drain_ui_callbacks();
        assert!(!ui.web_polygon_sides.is_visible());
        assert!(ui.web_mixed_shape_apply_row.is_visible());
        eprintln!(
            "realized resource polygon sides check: shipping Regular Polygon control defaulted to 4, ignored a contradictory adapter, persisted shared 3/6 and Magenta-only 3 authority edits, and hid for Circle/User Defined/mixed shapes"
        );
        ui.window.close();
    }

    fn verify_realized_export_background_authoring() {
        let application = adw::Application::builder()
            .application_id("dev.toniator.export-background-authoring")
            .build();
        application.register(None::<&gio::Cancellable>).unwrap();
        let ui = AppUi::new(
            &application,
            CliOptions {
                demo: true,
                artifact_window_size: Some((960, 720)),
                ..CliOptions::default()
            },
        );
        ui.window.present();
        ui.controls_toggle.set_active(true);
        drain_ui_callbacks();

        assert!(ui.export_background.is_visible());
        assert!(ui.export_color_label.is_visible());
        assert!(ui.export_color.is_visible());
        assert!(!ui.export_color.is_sensitive());
        assert_eq!(
            ui.export_color_label.text(),
            "Background Color · None (transparent)"
        );
        assert_eq!(rgba_color(ui.export_color.rgba()), RgbaColor::WHITE);

        let preview_before = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .appearance
            .preview_surface;
        ui.export_background.set_selected(1);
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .appearance,
            DocumentAppearance {
                preview_surface: preview_before,
                export_background: ExportBackground::Color {
                    color: RgbaColor::WHITE,
                },
            }
        );
        assert!(ui.export_color.is_sensitive());
        assert_eq!(ui.export_color_label.text(), "Background Color · #FFFFFFFF");
        assert_eq!(
            ui.export_color.tooltip_text().as_deref(),
            Some("Background Color · #FFFFFFFF")
        );

        let chosen = RgbaColor::opaque(12, 34, 56);
        ui.export_color.set_rgba(&gdk_rgba(chosen));
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .appearance,
            DocumentAppearance {
                preview_surface: preview_before,
                export_background: ExportBackground::Color { color: chosen },
            }
        );
        assert_eq!(ui.export_color_label.text(), "Background Color · #0C2238FF");

        ui.export_background.set_selected(0);
        drain_ui_callbacks();
        assert!(matches!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .appearance
                .export_background,
            ExportBackground::None
        ));
        assert_eq!(
            ui.export_color_label.text(),
            "Background Color · None (transparent)"
        );
        ui.export_background.set_selected(1);
        drain_ui_callbacks();
        assert!(matches!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .appearance
                .export_background,
            ExportBackground::Color {
                color: RgbaColor::WHITE
            }
        ));
        assert_eq!(ui.export_color_label.text(), "Background Color · #FFFFFFFF");
        ui.undo();
        drain_ui_callbacks();
        assert!(matches!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .appearance
                .export_background,
            ExportBackground::None
        ));
        ui.redo();
        drain_ui_callbacks();
        assert!(matches!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .appearance
                .export_background,
            ExportBackground::Color {
                color: RgbaColor::WHITE
            }
        ));
        eprintln!(
            "realized export-background authoring check: shipping Appearance controls expose None/transparent, create opaque white when Color is selected, accept a direct color, preserve Preview Surface, and retain undo/redo"
        );
        ui.window.close();
    }

    fn verify_realized_preview_indicator() {
        let indicator = PreviewIndicator::new(None);
        assert_eq!(indicator.area.width_request(), 40);
        assert_eq!(indicator.area.height_request(), 28);
        assert_eq!(indicator.area.accessible_role(), gtk::AccessibleRole::Img);
        assert_eq!(
            indicator.area.tooltip_text().as_deref(),
            Some("Halftone preview")
        );
        indicator.request(1, PreviewView::Rendered);
        let epoch = indicator
            .epoch
            .get()
            .expect("render request starts animation");
        assert!(indicator.tick.borrow().is_some());
        assert_eq!(
            indicator.area.tooltip_text().as_deref(),
            Some("Updating halftone preview")
        );
        indicator.request(2, PreviewView::Rendered);
        assert_eq!(indicator.epoch.get(), Some(epoch));
        indicator.installed(1, PreviewView::Rendered);
        assert!(indicator.effective_busy());
        indicator.installed(2, PreviewView::Rendered);
        assert!(!indicator.effective_busy());
        assert!(indicator.tick.borrow().is_none());
        assert_eq!(indicator.phase(), 1.0);
        assert_eq!(
            indicator.area.tooltip_text().as_deref(),
            Some("Halftone preview")
        );
        indicator.request(3, PreviewView::Source);
        assert!(!indicator.effective_busy());
        assert_eq!(indicator.phase(), 0.0);
        assert_eq!(indicator.effective_label(), "Source preview");
        assert_eq!(
            indicator.area.tooltip_text().as_deref(),
            Some("Source preview")
        );
        indicator.failed(3);
        assert_eq!(indicator.phase(), 0.0);

        let settings = gtk::Settings::default().expect("GTK settings");
        let animations = settings.is_gtk_enable_animations();
        settings.set_gtk_enable_animations(false);
        let reduced = PreviewIndicator::new(None);
        reduced.request(1, PreviewView::Rendered);
        assert_eq!(reduced.phase(), 0.5);
        assert!(reduced.tick.borrow().is_none());
        settings.set_gtk_enable_animations(animations);
    }

    #[test]
    fn curve_anchor_handle_edit_is_atomic_and_cancellation_restores_all_components() {
        let mut document = Document::new(SourceArtwork {
            name: "curve.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from([1]),
        });
        let original = WebCurveSettings::default();
        document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(original.clone()),
        };
        let mut editor = DocumentEditor::new(document);
        assert!(editor.select_pattern(toniator::PatternId::COMPATIBILITY_CURVES_V1));

        editor.begin_edit(SettingKey::CurvePath);
        let mut moved = original.clone();
        let anchor = moved.shared_path.segments[0].end;
        set_curve_handle(
            &mut moved.shared_path,
            3,
            CurvePoint {
                x: anchor.x + 0.13,
                y: anchor.y - 0.07,
            },
        );
        let moved_path = moved.shared_path.clone();
        assert!(editor.set_curve_settings(moved));
        assert!(editor.end_edit());
        assert!(editor.undo());
        assert_eq!(
            editor.document().render,
            RenderVariant::WebCurveV1 {
                settings: Box::new(original.clone())
            }
        );
        assert!(editor.redo());
        assert!(matches!(
            &editor.document().render,
            RenderVariant::WebCurveV1 { settings } if settings.shared_path == moved_path
        ));

        editor.begin_edit(SettingKey::CurvePath);
        let mut cancelled = match &editor.document().render {
            RenderVariant::WebCurveV1 { settings } => (**settings).clone(),
            _ => unreachable!(),
        };
        cancelled.shared_path.segments[0].control_1.x += 0.22;
        cancelled.shared_path.segments[0].control_1.y -= 0.19;
        assert!(editor.set_curve_settings(cancelled));
        assert!(editor.cancel_edit());
        assert!(matches!(
            &editor.document().render,
            RenderVariant::WebCurveV1 { settings } if settings.shared_path == moved_path
        ));
    }

    #[test]
    fn realized_numeric_controls_leave_continuous_scroll_to_parent() {
        gtk::init().unwrap();
        verify_realized_top_level_shell_builder();
        verify_realized_editor_controls_builder();
        verify_realized_dropdown_sync_keeps_the_live_model_and_valid_selection();
        verify_realized_semantic_pipeline_callbacks();
        verify_realized_channel_scope_composites();
        verify_realized_pattern_selector_uses_authoritative_state_over_transient_adapter();
        verify_realized_export_background_authoring();
        register_ui_resources();
        let appearance_builder = gtk::Builder::from_resource(WINDOW_UI_RESOURCE);
        let appearance = build_appearance_controls(&appearance_builder);
        appearance_builder
            .object::<gtk::Expander>("appearance_section")
            .unwrap()
            .set_expanded(true);
        assert!(appearance.preview_color.dialog().unwrap().is_with_alpha());
        assert!(appearance.export_color.dialog().unwrap().is_with_alpha());
        assert!(appearance.preview_help.popover().is_some());
        assert!(appearance.export_help.popover().is_some());
        assert_eq!(
            appearance.preview_help.tooltip_text().as_deref(),
            Some("Help: Preview Surface")
        );
        assert_eq!(
            appearance.export_help.tooltip_text().as_deref(),
            Some("Help: Export Background")
        );
        assert_eq!(
            appearance.preview_surface.accessible_role(),
            gtk::AccessibleRole::ComboBox
        );
        assert_eq!(
            appearance.export_background.accessible_role(),
            gtk::AccessibleRole::ComboBox
        );
        assert_eq!(
            appearance.export_color_label.text(),
            "Background Color · None (transparent)"
        );
        appearance.preview_surface.set_selected(0);
        appearance
            .preview_color
            .set_visible(appearance.preview_surface.selected() == 1);
        appearance.preview_surface.set_selected(1);
        appearance.preview_color.set_visible(true);
        let appearance_window = appearance_builder
            .object::<adw::ApplicationWindow>("main_window")
            .unwrap();
        let appearance_stack = appearance_builder
            .object::<gtk::Stack>("main_stack")
            .unwrap();
        let appearance_editor_page = appearance_builder
            .object::<gtk::Box>("editor_page")
            .unwrap();
        appearance_stack
            .page(&appearance_editor_page)
            .set_name("editor");
        appearance_stack.set_visible_child_name("editor");
        appearance_window.set_default_size(960, 720);
        appearance_window.present();
        while glib::MainContext::default().iteration(false) {}
        assert!(appearance.preview_color.is_visible());
        assert!(appearance.container.bounds().expect("appearance bounds").3 > 0);
        assert!(
            appearance
                .preview_surface
                .bounds()
                .expect("preview bounds")
                .3
                > 0
        );
        assert!(
            appearance
                .export_background
                .bounds()
                .expect("export bounds")
                .3
                > 0
        );
        assert!(
            appearance
                .preview_help
                .bounds()
                .expect("preview help bounds")
                .3
                > 0
        );
        assert!(
            appearance
                .export_help
                .bounds()
                .expect("export help bounds")
                .3
                > 0
        );
        appearance_window.close();
        let capture_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let capture_paintable = gtk::WidgetPaintable::new(Some(&capture_content));
        let capture_window = gtk::Window::builder()
            .default_width(320)
            .default_height(180)
            .child(&capture_content)
            .build();
        capture_window.present();
        capture_content.append(&gtk::Label::new(Some("Dynamic editor content")));
        while glib::MainContext::default().iteration(false) {}
        let capture_snapshot = gtk::Snapshot::new();
        capture_paintable.snapshot(&capture_snapshot, 320.0, 180.0);
        let capture_node = capture_snapshot.to_node();
        assert!(
            capture_node.is_some(),
            "a paintable retained from UI construction must track dynamic editor invalidations"
        );
        let opaque_node = opaque_capture_node(
            &capture_node.unwrap(),
            320,
            180,
            gdk::RGBA::new(0.08, 0.09, 0.11, 0.25),
        );
        let surface = capture_window
            .surface()
            .expect("capture window has a surface");
        let renderer = gtk::gsk::Renderer::for_surface(&surface).expect("capture renderer");
        let texture = renderer.render_texture(
            &opaque_node,
            Some(&gtk::graphene::Rect::new(0.0, 0.0, 320.0, 180.0)),
        );
        let capture_path = std::env::temp_dir().join(format!(
            "toniator-opaque-capture-{}.png",
            std::process::id()
        ));
        texture.save_to_png(&capture_path).unwrap();
        let decoded = image::open(&capture_path).unwrap().into_rgba8();
        assert_eq!(decoded.dimensions(), (320, 180));
        assert!(decoded.pixels().all(|pixel| pixel.0[3] == 255));
        std::fs::remove_file(capture_path).unwrap();
        capture_window.close();

        let oversized = gtk::Box::new(gtk::Orientation::Vertical, 0);
        oversized.set_size_request(900, 700);
        let overflow_stage = CenterStage::new(&oversized);
        overflow_stage.set_hexpand(true);
        overflow_stage.set_vexpand(true);
        let overflow_scroller = gtk::ScrolledWindow::builder()
            .child(&overflow_stage)
            .build();
        let overflow_window = gtk::Window::builder()
            .default_width(640)
            .default_height(480)
            .child(&overflow_scroller)
            .build();
        overflow_window.present();
        while glib::MainContext::default().iteration(false) {}
        assert_eq!((oversized.width(), oversized.height()), (900, 700));
        assert!(
            overflow_scroller.hadjustment().upper() > overflow_scroller.hadjustment().page_size()
        );
        assert!(
            overflow_scroller.vadjustment().upper() > overflow_scroller.vadjustment().page_size()
        );
        overflow_window.close();

        let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        scale.set_value(42.0);
        let spin = gtk::SpinButton::with_range(0.0, 100.0, 1.0);
        spin.set_value(17.0);
        assert!(disable_pointer_scroll_adjustment(&scale) > 0);
        assert!(disable_pointer_scroll_adjustment(&spin) > 0);
        for widget in [
            &scale.clone().upcast::<gtk::Widget>(),
            &spin.clone().upcast(),
        ] {
            let controllers = widget.observe_controllers();
            assert!((0..controllers.n_items()).filter_map(|index| controllers.item(index))
                .filter_map(|item| item.downcast::<gtk::EventControllerScroll>().ok())
                .all(|controller| controller.propagation_phase() == gtk::PropagationPhase::None));
        }
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_size_request(200, 1000);
        content.append(&scale);
        content.append(&spin);
        let scroller = gtk::ScrolledWindow::builder()
            .min_content_height(120)
            .child(&content)
            .build();
        let window = gtk::Window::builder()
            .default_width(240)
            .default_height(160)
            .child(&scroller)
            .build();
        window.present();
        while glib::MainContext::default().iteration(false) {}
        let before_scale = scale.value();
        let before_spin = spin.value();
        let routed_parent =
            gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        routed_parent.set_propagation_phase(gtk::PropagationPhase::Bubble);
        routed_parent.connect_scroll(glib::clone!(
            #[weak]
            scroller,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, _, dy| {
                let adjustment = scroller.vadjustment();
                adjustment.set_value(adjustment.value() + dy * 40.0);
                glib::Propagation::Stop
            }
        ));
        scroller.add_controller(routed_parent.clone());
        for numeric in [
            &scale.clone().upcast::<gtk::Widget>(),
            &spin.clone().upcast(),
        ] {
            let bounds = numeric
                .compute_bounds(&window)
                .expect("numeric control is allocated");
            let picked = window
                .pick(
                    bounds.x() as f64 + bounds.width() as f64 / 2.0,
                    bounds.y() as f64 + bounds.height() as f64 / 2.0,
                    gtk::PickFlags::DEFAULT,
                )
                .expect("numeric control is pickable at its allocated center");
            assert!(picked == *numeric || picked.is_ancestor(numeric));
            let before_scroll = scroller.vadjustment().value();
            let _: bool = routed_parent.emit_by_name("scroll", &[&0.0f64, &1.25f64]);
            while glib::MainContext::default().iteration(false) {}
            assert_eq!(scale.value(), before_scale);
            assert_eq!(spin.value(), before_spin);
            assert!(scroller.vadjustment().value() > before_scroll);
        }
        scale.emit_by_name::<()>("move-slider", &[&gtk::ScrollType::StepForward]);
        assert!(
            scale.value() > before_scale,
            "native keyboard action remains enabled"
        );
        spin.emit_by_name::<()>("change-value", &[&gtk::ScrollType::StepForward]);
        assert!(
            spin.value() > before_spin,
            "spin keyboard action remains enabled"
        );
        let controllers = scale.observe_controllers();
        assert!(
            (0..controllers.n_items()).any(|index| controllers
                .item(index)
                .is_some_and(|item| item.is::<gtk::GestureDrag>())),
            "native drag gesture remains installed"
        );
        eprintln!(
            "realized GTK route check: picking each realized GtkScale/SpinButton center targets that numeric child; controller-chain continuous dy=1.25 reaches a bubble-phase scroller controller and advances its adjustment while values remain 42/17; GTK 4.22 GDK exposes gdk_display_put_event but no public GdkScrollEvent constructor, so this is event-pick/controller-chain injection rather than compositor input synthesis; native keyboard/drag controllers remain"
        );
        window.close();
        verify_realized_zoom_controls_drive_one_canonical_mode_and_actual_allocation();
        verify_realized_overlay_split_owns_inspector_width();
        verify_realized_inspector_shell_keeps_controls_visible();
        verify_realized_shape_double_clicks();
        verify_realized_resource_shape_editor_authority_workflow();
        verify_realized_resource_polygon_sides_authority_workflow();
        verify_realized_preview_indicator();
        verify_realized_output_mode_keeps_fixture_authority_over_transient_adapters();

        let base = 1.0;
        let effective = [0.8, 1.0, 1.2, 1.4];
        let new_base = 1.25;
        let shifted: Vec<_> = effective
            .into_iter()
            .map(|value| shifted_effective(value, new_base - base, 0.0, 5.0))
            .collect();
        assert_eq!(shifted, vec![1.05, 1.25, 1.45, 1.65]);
        assert_eq!(shifted[2] - new_base, effective[2] - base);
    }

    #[test]
    fn native_preset_roundtrips_base_and_effective_ink_values() {
        let mut shape = WebShapeSettings::default();
        shape.base_channel.scale = 1.25;
        shape.channels.c.scale = 1.05;
        shape.channels.m.scale = 1.45;
        shape.shared_shape = WebShape::UserDefined;
        shape.custom_shape_path = Some(curved_shape_fixture());
        let mut document = Document::new(SourceArtwork {
            name: "base-test".into(),
            media_type: "application/octet-stream".into(),
            bytes: Arc::from([1]),
        });
        document.render = RenderVariant::WebShapeV1 {
            settings: Box::new(shape.clone()),
        };
        let mut editor = DocumentEditor::new(document);
        assert!(editor.set_shape_settings(shape.clone()));
        let bytes = toniator::preset::document_preset_bytes(
            "Base Test",
            editor.document(),
            toniator::preset::PresetScope::CompleteWorkflow,
        )
        .unwrap();
        let parsed = toniator::preset::parse_treatment(&bytes, (900, 600)).unwrap();
        let rendered = parsed
            .candidate_for(&Document::new(SourceArtwork {
                name: "target".into(),
                media_type: "application/octet-stream".into(),
                bytes: Arc::from([1]),
            }))
            .unwrap()
            .render;
        shape.output_height = 600;
        assert_eq!(
            rendered,
            RenderVariant::WebShapeV1 {
                settings: Box::new(shape)
            }
        );
    }

    #[test]
    fn warmed_preview_caches_swap_without_requests_and_invalidate_independently() {
        let source = SourceArtwork {
            name: "cache.svg".into(),
            media_type: "image/svg+xml".into(),
            bytes: Arc::from(b"<svg/>".as_slice()),
        };
        let document = Document::new(source);
        let high = PreviewCache {
            document: document.clone(),
            image: RgbaImage::new(1600, 900),
        };
        let mut requests = 0;
        for view in [
            PreviewView::Source,
            PreviewView::Rendered,
            PreviewView::Source,
            PreviewView::Rendered,
        ] {
            if !preview_cache_matches(&high, &document, view)
                || !preview_cache_is_sufficient(&high, 1400)
            {
                requests += 1;
            }
        }
        assert_eq!(
            requests, 0,
            "warmed Rendered/Source toggles schedule no work"
        );
        assert!(preview_cache_is_sufficient(&high, 800));
        assert!(!preview_cache_is_sufficient(&high, 2000));

        let mut changed = document.clone();
        if let RenderVariant::WebShapeV1 { settings } = &mut changed.render {
            settings.grid_scale += 1.0;
        }
        assert!(preview_cache_matches(&high, &changed, PreviewView::Source));
        assert!(!preview_cache_matches(
            &high,
            &changed,
            PreviewView::Rendered
        ));

        let isolated = Document::new(SourceArtwork {
            name: "new.svg".into(),
            media_type: "image/svg+xml".into(),
            bytes: Arc::from(b"<svg/>".as_slice()),
        });
        assert!(!preview_cache_matches(
            &high,
            &isolated,
            PreviewView::Source
        ));
    }

    #[test]
    fn appearance_controls_have_clear_export_scope_and_accessible_names() {
        assert_eq!(
            PREVIEW_SURFACE_LABEL,
            "Preview Surface — Canvas only · not exported"
        );
        assert_eq!(
            EXPORT_BACKGROUND_LABEL,
            "Export Background — Used for SVG and by default for PNG"
        );
        // The construction path applies these exact accessible labels to both
        // dropdowns and their alpha-capable ColorDialogButtons.
        assert!(PREVIEW_SURFACE_LABEL.contains("not exported"));
        assert!(EXPORT_BACKGROUND_LABEL.contains("by default for PNG"));
    }

    #[test]
    fn artifact_appearance_values_are_strict_rgba_or_named_controls() {
        assert_eq!(
            parse_artifact_rgba("#12345680"),
            Some(RgbaColor {
                red: 18,
                green: 52,
                blue: 86,
                alpha: 128
            })
        );
        assert!(parse_artifact_rgba("#123456").is_none());
        assert!(parse_artifact_rgba("12345680").is_none());
    }

    #[test]
    fn semantic_artwork_controls_use_stable_labels_and_legacy_cli_mapping_only() {
        assert_eq!(
            artwork_source_labels(false),
            [
                "Full Color",
                "Red",
                "Green",
                "Blue",
                "Value",
                "Perceptual Lightness",
                "Alpha"
            ]
        );
        assert_eq!(
            source_alpha_labels(false),
            ["Preserve Source Alpha", "Ignore Source Alpha"]
        );
        assert_eq!(
            channel_assignment_labels(true, OutputModel::CmykPrint),
            ["Automatic CMYK Separation"]
        );
        assert_eq!(
            channel_assignment_labels(false, OutputModel::CmykPrint),
            ["Apply To Active Channel", "Apply To All Channels"]
        );
        assert_eq!(
            artwork_source_guidance(ArtworkSource::Value),
            "Use the strongest RGB component (HSV value semantics) as sampled content."
        );
        assert_eq!(
            artwork_source_guidance(ArtworkSource::PerceptualLightness),
            "Use OKLab L for perceptual lightness as sampled content."
        );
        assert_eq!(
            output_channel_labels(OutputModel::RgbScreen),
            ["Red", "Green", "Blue"]
        );
        assert_eq!(
            source_mapping_from_index(3),
            Some(ValueMode::CrosshatchLuminance)
        );
        assert_eq!(source_mapping_from_index(5), None);
    }

    #[test]
    fn help_catalog_entries_are_actionable_and_scoped() {
        let required = [
            "Artwork Mapping",
            "Coverage",
            "Contrast",
            "Mark",
            "Apply Mark to All",
            "Polygon Sides (3–6)",
            "Edit User-Defined Mark",
            "Adjust Ink",
            "Adjust Layer",
            "Adjust Channel",
            "Visible Inks",
            "Visible Crosshatch Layers",
            "Visible RGB Channels",
            "Screen Angle",
            "Mark Rotation",
            "Sampling Detail",
            "Line Weight",
            "Line Spacing",
            "Line Shape",
            "Curve Editor",
            "Reset Line",
            "Position X",
            "Position Y",
            "Artwork Coverage",
            "PNG Background",
            "Preview Surface",
            "Export Background",
            "PNG Size",
            "Load Preset",
            "Save Preset",
            "Canvas Zoom",
            "Fit Artwork",
        ];
        for control in required {
            let spec = help_for(control).expect("required control has contextual help");
            assert!(!spec.heading.trim().is_empty());
            assert!(!spec.summary.trim().is_empty());
            assert!(!spec.body.trim().is_empty());
            assert!(
                spec.body.contains("preview") || spec.body.contains("export"),
                "{control}"
            );
            assert!(spec.body.len() > spec.summary.len());
        }
    }

    #[test]
    fn layer_terminology_uses_creator_facing_ink_and_layer_names() {
        assert_eq!(
            layer_terminology(false, false),
            ("Adjust Ink", "Visible Inks")
        );
        assert_eq!(
            layer_terminology(false, true),
            ("Adjust Layer", "Visible Crosshatch Layers")
        );
        assert_eq!(
            layer_terminology(true, false),
            ("Adjust Channel", "Visible RGB Channels")
        );
        assert_eq!(visible_ink_for_slot(0, true, false), Some(Ink::Red));
        assert_eq!(visible_ink_for_slot(2, true, false), Some(Ink::Blue));
    }

    #[test]
    fn shape_channel_slots_reject_invalid_dropdown_positions_without_fallback() {
        assert_eq!(visible_ink_for_slot(3, true, false), None);
        assert_eq!(visible_ink_for_slot(usize::MAX, true, false), None);
        assert_eq!(
            web_inks_for_target(gtk::INVALID_LIST_POSITION, true, false),
            None
        );
        assert_eq!(web_inks_for_target(4, true, false), None);
        assert_eq!(web_inks_for_target(0, true, false), Some(Ink::RGB.to_vec()));
        assert_eq!(web_inks_for_target(2, true, false), Some(vec![Ink::Green]));

        assert_eq!(visible_ink_for_slot(0, false, false), Some(Ink::Cyan));
        assert_eq!(visible_ink_for_slot(3, false, false), Some(Ink::Black));
        assert_eq!(visible_ink_for_slot(4, false, false), None);
        assert_eq!(visible_ink_for_slot(usize::MAX, false, false), None);
        assert_eq!(
            web_inks_for_target(gtk::INVALID_LIST_POSITION, false, false),
            None
        );
        assert_eq!(web_inks_for_target(5, false, false), None);
        assert_eq!(web_inks_for_target(4, false, false), Some(vec![Ink::Black]));
    }

    fn verify_realized_dropdown_sync_keeps_the_live_model_and_valid_selection() {
        let dropdown =
            gtk::DropDown::from_strings(&["All Inks", "Cyan", "Magenta", "Yellow", "Black"]);
        let model = dropdown.model().unwrap();
        dropdown.set_selected(4);

        // A normal control resync must not replace the model beneath GTK's
        // selected-notify/list-view activation path.
        sync_dropdown_strings(
            &dropdown,
            &["All Inks", "Cyan", "Magenta", "Yellow", "Black"],
        );
        assert_eq!(dropdown.model().unwrap(), model);
        assert_eq!(dropdown.selected(), 4);

        // CMYK -> RGB changes the effective items in place and clamps K to a
        // real RGB channel instead of leaving INVALID_LIST_POSITION behind.
        sync_dropdown_strings(&dropdown, &["All Channels", "Red", "Green", "Blue"]);
        assert_eq!(dropdown.model().unwrap(), model);
        assert_eq!(dropdown.selected(), 3);
        assert_eq!(
            web_inks_for_target(dropdown.selected(), true, false),
            Some(vec![Ink::Blue])
        );

        assert_eq!(
            web_inks_for_target(gtk::INVALID_LIST_POSITION, true, false),
            None
        );
        eprintln!(
            "realized GTK DropDown sync retained its StringList across CMYK/RGB changes, clamped K to Blue, and rejected INVALID_LIST_POSITION without an all-channels fallback"
        );
    }

    fn verify_realized_channel_scope_composites() {
        register_ui_resources();
        let builder = gtk::Builder::from_resource(WINDOW_UI_RESOURCE);
        let hierarchy = build_inspector_hierarchy(&builder);
        assert!(hierarchy.source_section.is_expanded());
        assert!(hierarchy.output_section.is_expanded());
        assert!(!hierarchy.channel_settings_section.is_expanded());

        let application = adw::Application::builder()
            .application_id("dev.toniator.channel-scope-regression")
            .build();
        application.register(None::<&gio::Cancellable>).unwrap();
        let ui = AppUi::new(
            &application,
            CliOptions {
                demo: true,
                artifact_window_size: Some((900, 680)),
                ..CliOptions::default()
            },
        );
        ui.window.present();
        drain_ui_callbacks();

        let hierarchy_child = ui
            .inspector_root
            .first_child()
            .expect("hierarchy is the first inspector child");
        let first_hierarchy_section = hierarchy_child
            .first_child()
            .expect("Source is the first hierarchy section")
            .downcast::<gtk::Expander>()
            .expect("first hierarchy section is an expander");
        assert_eq!(first_hierarchy_section.label().as_deref(), Some("Source"));

        assert_eq!(ui.channel_controls.len(), 7);
        assert_eq!(
            ui.channel_controls
                .iter()
                .map(|controls| controls.channel)
                .collect::<Vec<_>>(),
            [
                OutputChannelId::CmykCyan,
                OutputChannelId::CmykMagenta,
                OutputChannelId::CmykYellow,
                OutputChannelId::CmykBlack,
                OutputChannelId::RgbRed,
                OutputChannelId::RgbGreen,
                OutputChannelId::RgbBlue,
            ]
        );
        let scope_model = ui.channel_scope.model().unwrap();
        let roots = ui
            .channel_controls
            .iter()
            .map(|controls| controls.root.clone())
            .collect::<Vec<_>>();
        let aggregate_root = ui.aggregate_channel_controls.root.clone();
        assert_eq!(
            ui.channel_panel_stack.visible_child(),
            Some(aggregate_root.clone().upcast())
        );
        assert_eq!(ui.channel_scope.model().unwrap().n_items(), 5);

        // Full Color keeps automatic output routing, but treatment scope is
        // still editable. The top control is the sole authoring locus for
        // both Shapes and Curves.
        assert!(ui.channel_scope.is_sensitive());
        ui.activate_shape_treatment();
        drain_ui_callbacks();
        let web_target_model = ui.web_target.model().unwrap();
        let curve_target_model = ui.curve_target.model().unwrap();
        assert_eq!(ui.web_target.selected(), 0);
        assert_eq!(ui.curve_target.selected(), 0);
        let full_color_pipeline = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .artwork_pipeline
            .clone();
        ui.channel_scope.set_selected(4); // Black Ink, resolved through OutputChannelId.
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .artwork_pipeline
                .clone(),
            full_color_pipeline
        );
        assert_eq!(ui.web_target.model().unwrap(), web_target_model);
        assert_eq!(ui.curve_target.model().unwrap(), curve_target_model);
        assert_eq!(ui.web_target.selected(), 4);
        assert_eq!(ui.curve_target.selected(), 4);
        assert_eq!(ui.selected_web_inks(), Some(vec![Ink::Black]));
        assert_eq!(ui.selected_curve_inks(), Some(vec![Ink::Black]));

        // Output controls remain independent from treatment scope. Give the
        // scalar pipeline its own active Black channel, then change treatment
        // scope to Cyan without touching ChannelAssignment or active_channel.
        ui.artwork_source.set_selected(4); // Value
        drain_ui_callbacks();
        ui.channel_assignment.set_selected(0); // Apply To Active Channel
        drain_ui_callbacks();
        ui.active_channel.set_selected(3); // Black
        drain_ui_callbacks();
        let output_pipeline = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .artwork_pipeline
            .clone();
        assert_eq!(
            output_pipeline.active_channel,
            Some(OutputChannelId::CmykBlack)
        );
        ui.channel_scope.set_selected(1); // Cyan Ink treatment scope.
        drain_ui_callbacks();
        assert_eq!(ui.web_target.selected(), 1);
        assert_eq!(ui.curve_target.selected(), 1);
        assert_eq!(ui.selected_web_inks(), Some(vec![Ink::Cyan]));
        assert_eq!(ui.selected_curve_inks(), Some(vec![Ink::Cyan]));
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .artwork_pipeline
                .clone(),
            output_pipeline
        );
        assert_eq!(
            ui.channel_panel_stack.visible_child_name().as_deref(),
            Some(OutputChannelId::CmykCyan.stable_id())
        );

        ui.curves.set_active(true);
        drain_ui_callbacks();
        assert_eq!(ui.selected_curve_inks(), Some(vec![Ink::Cyan]));

        for output in [
            OutputMode::RgbScreen,
            OutputMode::CmykInks,
            OutputMode::RgbScreen,
            OutputMode::CmykInks,
        ] {
            ui.output_mode
                .set_selected((output == OutputMode::RgbScreen) as u32);
            drain_ui_callbacks();
            assert_eq!(ui.channel_scope.model().unwrap(), scope_model);
            assert_eq!(ui.web_target.model().unwrap(), web_target_model);
            assert_eq!(ui.curve_target.model().unwrap(), curve_target_model);
            assert_eq!(
                ui.channel_scope.model().unwrap().n_items(),
                if output == OutputMode::RgbScreen {
                    4
                } else {
                    5
                }
            );
            for (controls, root) in ui.channel_controls.iter().zip(&roots) {
                assert_eq!(controls.root, *root);
            }
        }

        ui.crosshatch_action.emit_clicked();
        drain_ui_callbacks();
        assert_eq!(ui.channel_scope.model().unwrap(), scope_model);
        assert_eq!(ui.channel_scope.model().unwrap().n_items(), 1);
        assert!(!ui.channel_scope.is_sensitive());
        assert_eq!(
            ui.channel_panel_stack.visible_child(),
            Some(aggregate_root.clone().upcast())
        );
        assert_eq!(ui.aggregate_channel_controls.heading.text(), "All Layers");
        assert!(
            ui.aggregate_channel_controls
                .mixed_message
                .text()
                .contains("Mixed")
        );
        ui.window.close();
    }

    fn verify_realized_pattern_selector_uses_authoritative_state_over_transient_adapter() {
        verify_realized_dropdown_sync_keeps_the_live_model_and_valid_selection();

        let application = adw::Application::builder()
            .application_id("dev.toniator.pattern-selector-authority")
            .build();
        application.register(None::<&gio::Cancellable>).unwrap();
        let ui = AppUi::new(
            &application,
            CliOptions {
                demo: true,
                artifact_window_size: Some((900, 680)),
                ..CliOptions::default()
            },
        );
        ui.window.present();
        drain_ui_callbacks();

        let shapes = PATTERN_REGISTRY
            .get(PatternId::COMPATIBILITY_SHAPES_V1)
            .unwrap();
        let curves = PATTERN_REGISTRY
            .get(PatternId::COMPATIBILITY_CURVES_V1)
            .unwrap();
        assert_eq!(ui.dots.label().as_deref(), Some(shapes.selector.label));
        assert_eq!(ui.curves.label().as_deref(), Some(curves.selector.label));

        let source = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .clone();
        let mut shape_editor = DocumentEditor::new(source);
        let mut authoritative_settings = shape_editor
            .document()
            .pattern_state
            .shape_settings()
            .unwrap();
        authoritative_settings.use_shared_mark = true;
        authoritative_settings.base_channel.scale = 0.65;
        assert!(shape_editor.set_shape_settings(authoritative_settings.clone()));
        let mut authoritative_shapes = shape_editor.document().clone();
        let contradictory_adapter_settings = WebShapeSettings {
            use_shared_mark: false,
            base_channel: toniator::WebShapeChannel {
                scale: 1.9,
                ..Default::default()
            },
            ..Default::default()
        };
        authoritative_shapes.render = RenderVariant::WebShapeV1 {
            settings: Box::new(contradictory_adapter_settings),
        };
        ui.state.borrow_mut().editor = Some(DocumentEditor::new(authoritative_shapes));
        ui.sync_controls();
        assert!(ui.dots.is_active());
        assert!(!ui.curves.is_active());
        assert!(!ui.legacy.is_visible());
        assert_eq!(
            ui.treatment_modes.visible_child_name().as_deref(),
            Some("web")
        );
        assert!(ui.web_shared.is_active());
        assert!((ui.web_coverage.value() - 0.65).abs() < 1e-9);
        let edit_shape = PATTERN_REGISTRY
            .parameter_for_control(PatternId::COMPATIBILITY_SHAPES_V1, "web_edit_shape")
            .unwrap();
        assert_eq!(ui.web_edit_shape.label().as_deref(), Some(edit_shape.label));

        ui.web_coverage.set_value(0.8);
        drain_ui_callbacks();
        let edited_settings = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .pattern_state
            .shape_settings()
            .unwrap();
        assert!((edited_settings.base_channel.scale - 0.8).abs() < 1e-9);

        let source = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .clone();
        let mut curve_editor = DocumentEditor::new(source);
        assert!(curve_editor.select_pattern(PatternId::COMPATIBILITY_CURVES_V1));
        let mut authoritative_curve_settings = curve_editor
            .document()
            .pattern_state
            .curve_settings()
            .unwrap();
        authoritative_curve_settings.max_mark = 37.0;
        authoritative_curve_settings.base_channel.scale = 0.65;
        authoritative_curve_settings.output_width = 640;
        authoritative_curve_settings.output_height = 480;
        authoritative_curve_settings.layout = CurveLayout::MotifPattern;
        authoritative_curve_settings.use_shared_curve = true;
        authoritative_curve_settings.shared_path = CurvePath::deep_wave();
        authoritative_curve_settings.channels.c.color = "#123456".into();
        authoritative_curve_settings.channels.c.offset_x = 33.0;
        authoritative_curve_settings.channels.c.offset_y = -17.0;
        authoritative_curve_settings.channels.c.grid_rotation = 31.0;
        authoritative_curve_settings.channels.c.stack_spacing = 50.0;
        assert!(curve_editor.set_curve_settings(authoritative_curve_settings.clone()));
        let mut authoritative_curves = curve_editor.document().clone();
        assert_eq!(document_artboard_size(&authoritative_curves), (640, 480));
        authoritative_curves.render = RenderVariant::WebCurveV1 {
            settings: Box::new(WebCurveSettings {
                max_mark: 17.0,
                base_channel: WebCurveChannel {
                    scale: 1.9,
                    ..Default::default()
                },
                ..Default::default()
            }),
        };
        ui.state.borrow_mut().editor = Some(DocumentEditor::new(authoritative_curves));
        ui.sync_controls();
        assert!(!ui.dots.is_active());
        assert!(ui.curves.is_active());
        assert!(!ui.legacy.is_visible());
        assert_eq!(
            ui.treatment_modes.visible_child_name().as_deref(),
            Some("curve")
        );
        assert!((ui.curve_weight.value() - 37.0).abs() < 1e-9);
        assert!((ui.curve_coverage.value() - 0.65).abs() < 1e-9);
        assert!(ui.motif_controls.is_visible());
        assert_eq!(ui.current_curve_path(), Some(CurvePath::deep_wave()));
        let curve_weight = PATTERN_REGISTRY
            .parameter_for_control(PatternId::COMPATIBILITY_CURVES_V1, "curve_weight_scale")
            .unwrap();
        assert_eq!(
            ui.curve_weight.tooltip_text().as_deref(),
            Some(curve_weight.help)
        );
        let curve_editor_descriptor = PATTERN_REGISTRY
            .parameter_for_control(PatternId::COMPATIBILITY_CURVES_V1, "curve_editor")
            .unwrap();
        assert_eq!(
            ui.curve_editor.tooltip_text().as_deref(),
            Some(curve_editor_descriptor.help)
        );

        ui.curve_weight.set_value(42.0);
        drain_ui_callbacks();
        let edited_curve_settings = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .pattern_state
            .curve_settings()
            .unwrap();
        assert!((edited_curve_settings.max_mark - 42.0).abs() < 1e-9);

        ui.curve_target.set_selected(1);
        drain_ui_callbacks();
        assert_eq!(
            ui.current_curve_color(),
            (
                0x12 as f64 / 255.0,
                0x34 as f64 / 255.0,
                0x56 as f64 / 255.0
            )
        );
        assert_eq!(
            ui.current_motif_arrangement(),
            Some((33.0, -17.0, 31.0, 50.0))
        );
        ui.motif_arrange.set_active(true);
        drain_ui_callbacks();
        assert!(ui.motif_overlay.is_visible());
        assert!(ui.motif_overlay_geometry(800.0, 600.0).is_some());

        ui.curve_profile.set_selected(1);
        drain_ui_callbacks();
        let profiled_curve_settings = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .pattern_state
            .curve_settings()
            .unwrap();
        assert_eq!(profiled_curve_settings.shared_path, CurvePath::soft_wave());
        ui.update_editing_context();
        assert_eq!(ui.editing_context.text(), "Curves · Cyan");

        ui.dots.set_active(true);
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .pattern_state
                .selected_pattern_id(),
            Some(PatternId::COMPATIBILITY_SHAPES_V1)
        );
        assert_eq!(
            ui.treatment_modes.visible_child_name().as_deref(),
            Some("web")
        );

        ui.curves.set_active(true);
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .pattern_state
                .selected_pattern_id(),
            Some(PatternId::COMPATIBILITY_CURVES_V1)
        );
        assert_eq!(
            ui.treatment_modes.visible_child_name().as_deref(),
            Some("curve")
        );

        ui.weighted_voronoi.set_active(true);
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .pattern_state
                .selected_pattern_id(),
            Some(PatternId::WEIGHTED_VORONOI_V1)
        );
        assert_eq!(
            ui.treatment_modes.visible_child_name().as_deref(),
            Some("weighted-voronoi")
        );
        assert!(ui.weighted_voronoi_cell_count.is_visible());
        ui.weighted_voronoi_cell_count.set_value(333.0);
        ui.weighted_voronoi_visible[1].set_active(false);
        ui.weighted_voronoi_arrangement.set_selected(1);
        ui.weighted_voronoi_placement.set_selected(1);
        ui.weighted_voronoi_seed.set_text("18446744073709551615");
        ui.weighted_voronoi_seed.emit_activate();
        drain_ui_callbacks();
        let weighted = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .pattern_state
            .weighted_voronoi_settings()
            .unwrap();
        let cyan = weighted
            .channel_settings(OutputChannelId::CmykCyan)
            .unwrap();
        assert_eq!(cyan.cell_count, 333);
        assert_eq!(
            cyan.arrangement,
            WeightedVoronoiArrangementPolicy::Independent
        );
        assert_eq!(cyan.placement, WeightedVoronoiPlacementMode::Uniform);
        assert_eq!(cyan.seed, u64::MAX);
        assert!(
            !weighted
                .channel_settings(OutputChannelId::CmykMagenta)
                .unwrap()
                .enabled
        );

        // Continue the existing Curves fixture from an authoritative pattern
        // selection after exercising the new Weighted Voronoi controls.
        ui.curves.set_active(true);
        drain_ui_callbacks();

        let mut pipeline_authoritative_curves = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .clone();
        pipeline_authoritative_curves.artwork_pipeline.assignment =
            ChannelAssignment::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1,
            );
        pipeline_authoritative_curves.render = RenderVariant::WebCurveV1 {
            settings: Box::new(WebCurveSettings::default()),
        };
        ui.state.borrow_mut().editor = Some(DocumentEditor::new(pipeline_authoritative_curves));
        ui.sync_controls();
        ui.update_editing_context();
        assert_eq!(ui.selected_curve_inks(), Some(vec![Ink::Black]));
        assert!(
            ui.editing_context
                .text()
                .starts_with("Curves · Crosshatch · Layer 1 (Black)")
        );

        let target_model = ui.web_target.model().unwrap();
        ui.sync_controls_when_idle();
        drain_ui_callbacks();
        assert_eq!(ui.web_target.model().unwrap(), target_model);
        assert!(!ui.state.borrow().syncing_controls);
        ui.window.close();
    }

    fn contradictory_adapter_for(pattern: PatternId) -> RenderVariant {
        match pattern {
            PatternId::COMPATIBILITY_SHAPES_V1 => RenderVariant::WebCurveV1 {
                settings: Box::new(WebCurveSettings {
                    output_width: 19,
                    output_height: 13,
                    long_edge_cells: 2.0,
                    max_mark: 91.0,
                    ..Default::default()
                }),
            },
            PatternId::COMPATIBILITY_CURVES_V1 => RenderVariant::WebShapeV1 {
                settings: Box::new(WebShapeSettings {
                    output_width: 17,
                    output_height: 11,
                    long_edge_cells: 2.0,
                    grid_scale: 77.0,
                    polygon_sides: 3,
                    ..Default::default()
                }),
            },
            PatternId::WEIGHTED_VORONOI_V1 => RenderVariant::NativeBasicV1,
        }
    }

    fn assert_realized_output_fixture_authority(
        ui: &AppUi,
        document: &Document,
        selected: PatternId,
    ) {
        assert_eq!(document.pattern_state.selected_pattern_id(), Some(selected));
        assert!(!ui.state.borrow().syncing_controls);
        match selected {
            PatternId::COMPATIBILITY_SHAPES_V1 => {
                let settings = document.pattern_state.shape_settings().unwrap();
                assert!(ui.dots.is_active());
                assert!(!ui.curves.is_active());
                assert_eq!(
                    ui.treatment_modes.visible_child_name().as_deref(),
                    Some("web")
                );
                assert_eq!(
                    ui.web_polygon_sides.value_as_int(),
                    settings.polygon_sides as i32
                );
                assert!((ui.web_coverage.value() - settings.base_channel.scale).abs() < 1e-9);
            }
            PatternId::COMPATIBILITY_CURVES_V1 => {
                let settings = document.pattern_state.curve_settings().unwrap();
                assert!(!ui.dots.is_active());
                assert!(ui.curves.is_active());
                assert_eq!(
                    ui.treatment_modes.visible_child_name().as_deref(),
                    Some("curve")
                );
                assert!((ui.curve_weight.value() - settings.max_mark).abs() < 1e-9);
                assert!((ui.curve_coverage.value() - settings.base_channel.scale).abs() < 1e-9);
                assert!(ui.motif_controls.is_visible());
            }
            PatternId::WEIGHTED_VORONOI_V1 => unreachable!("compatibility fixture"),
        }
    }

    fn verify_realized_output_mode_keeps_fixture_authority_over_transient_adapters() {
        let application = adw::Application::builder()
            .application_id("dev.toniator.output-cache-ui-authority")
            .build();
        application.register(None::<&gio::Cancellable>).unwrap();
        let ui = AppUi::new(
            &application,
            CliOptions {
                demo: true,
                artifact_window_size: Some((1000, 760)),
                ..CliOptions::default()
            },
        );
        ui.window.present();
        ui.controls_toggle.set_active(true);
        drain_ui_callbacks();

        let cmyk_preview = PreviewSurface::Color {
            color: RgbaColor::opaque(241, 236, 225),
        };
        let export_background = ExportBackground::Color {
            color: RgbaColor::opaque(11, 22, 33),
        };
        let output_model = ui.output_mode.model().unwrap();
        let base_source = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .clone();

        for (name, bytes, selected) in [
            (
                "Polygon Six",
                include_bytes!("../assets/presets/Polygon Six.tntr").as_slice(),
                PatternId::COMPATIBILITY_SHAPES_V1,
            ),
            (
                "Motif Ladder",
                include_bytes!("../assets/presets/Motif Ladder.tntr").as_slice(),
                PatternId::COMPATIBILITY_CURVES_V1,
            ),
        ] {
            // Start each fixture from the same production document so an RGB
            // cache deliberately created for the preceding fixture cannot
            // become an accidental input to this fixture's transition.
            let source = base_source.clone();
            let dimensions = toniator::render::source_dimensions(&source.source).unwrap();
            let candidate = toniator::preset::parse_treatment(bytes, dimensions)
                .unwrap_or_else(|error| panic!("{name} did not parse: {error:#}"))
                .candidate_for(&source)
                .unwrap_or_else(|error| panic!("{name} did not apply: {error:#}"));
            let mut editor = DocumentEditor::new(source);
            assert!(editor.replace_with_preset_candidate(candidate));
            let appearance = DocumentAppearance {
                preview_surface: cmyk_preview,
                export_background,
            };
            if editor.document().appearance != appearance {
                assert!(editor.set_appearance(appearance));
            }
            let cmyk_authority = editor.document().pattern_state.clone();
            let mut contradictory = editor.document().clone();
            contradictory.render = contradictory_adapter_for(selected);
            ui.state.borrow_mut().editor = Some(DocumentEditor::new(contradictory));
            ui.sync_controls();
            drain_ui_callbacks();

            let cmyk_document = ui
                .state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .clone();
            assert_realized_output_fixture_authority(&ui, &cmyk_document, selected);
            assert_eq!(ui.output_mode.selected(), 0);
            assert_eq!(ui.preview_surface.selected(), 1);
            assert_eq!(
                rgba_color(ui.preview_color.rgba()),
                RgbaColor::opaque(241, 236, 225)
            );
            assert_eq!(ui.export_background.selected(), 1);
            assert_eq!(
                rgba_color(ui.export_color.rgba()),
                RgbaColor::opaque(11, 22, 33)
            );
            assert_eq!(ui.export_color_label.text(), "Background Color · #0B1621FF");

            // This is the shipping Blueprint dropdown callback. It must defer
            // control-model synchronization until after selected-notify.
            ui.output_mode.set_selected(1);
            drain_ui_callbacks();
            // The document transition has completed above. Explicitly settle
            // its normal UI projection before reading widgets so this
            // realized regression remains deterministic when the full suite
            // has other GTK sources pending on the shared main context.
            ui.sync_controls();
            drain_ui_callbacks();
            let rgb_document = ui
                .state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .clone();
            assert_eq!(
                rgb_document.artwork_pipeline.output_model,
                OutputModel::RgbScreen
            );
            assert_eq!(rgb_document.pattern_state, cmyk_authority);
            assert_realized_output_fixture_authority(&ui, &rgb_document, selected);
            assert_eq!(ui.output_mode.selected(), 1);
            assert_eq!(ui.preview_surface.selected(), 1);
            assert_eq!(
                rgba_color(ui.preview_color.rgba()),
                RgbaColor::opaque(0, 0, 0)
            );
            assert_eq!(rgb_document.appearance.export_background, export_background);
            assert_eq!(ui.export_background.selected(), 1);
            assert_eq!(
                rgba_color(ui.export_color.rgba()),
                RgbaColor::opaque(11, 22, 33)
            );

            // Corrupt only the inactive CMYK facade. The active RGB selector
            // and controls must continue to use its typed pattern authority.
            let mut inactive_contradiction = rgb_document;
            inactive_contradiction
                .inactive_cmyk
                .as_mut()
                .expect("CMYK state is cached while RGB is active")
                .render = contradictory_adapter_for(selected);
            ui.state.borrow_mut().editor = Some(DocumentEditor::new(inactive_contradiction));
            ui.sync_controls();
            drain_ui_callbacks();
            let active_rgb_document = ui
                .state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .clone();
            assert_realized_output_fixture_authority(&ui, &active_rgb_document, selected);

            match selected {
                PatternId::COMPATIBILITY_SHAPES_V1 => ui.web_polygon_sides.set_value(3.0),
                PatternId::COMPATIBILITY_CURVES_V1 => ui.curve_weight.set_value(61.0),
                PatternId::WEIGHTED_VORONOI_V1 => unreachable!("compatibility fixture"),
            }
            drain_ui_callbacks();
            let edited_rgb_document = ui
                .state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .clone();
            match selected {
                PatternId::COMPATIBILITY_SHAPES_V1 => assert_eq!(
                    edited_rgb_document
                        .pattern_state
                        .shape_settings()
                        .unwrap()
                        .polygon_sides,
                    3
                ),
                PatternId::COMPATIBILITY_CURVES_V1 => assert!(
                    (edited_rgb_document
                        .pattern_state
                        .curve_settings()
                        .unwrap()
                        .max_mark
                        - 61.0)
                        .abs()
                        < 1e-9
                ),
                PatternId::WEIGHTED_VORONOI_V1 => unreachable!("compatibility fixture"),
            }
            assert_realized_output_fixture_authority(&ui, &edited_rgb_document, selected);

            ui.output_mode.set_selected(0);
            drain_ui_callbacks();
            ui.sync_controls();
            drain_ui_callbacks();
            let restored_cmyk_document = ui
                .state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .clone();
            assert_eq!(
                restored_cmyk_document.artwork_pipeline.output_model,
                OutputModel::CmykPrint
            );
            assert_eq!(restored_cmyk_document.pattern_state, cmyk_authority);
            assert_realized_output_fixture_authority(&ui, &restored_cmyk_document, selected);
            assert_eq!(
                restored_cmyk_document.appearance.preview_surface,
                cmyk_preview
            );
            assert_eq!(
                restored_cmyk_document.appearance.export_background,
                export_background
            );
            assert_eq!(
                rgba_color(ui.preview_color.rgba()),
                RgbaColor::opaque(241, 236, 225)
            );
            assert_eq!(
                rgba_color(ui.export_color.rgba()),
                RgbaColor::opaque(11, 22, 33)
            );

            ui.undo();
            drain_ui_callbacks();
            let undone_rgb_document = ui
                .state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .clone();
            assert_eq!(
                undone_rgb_document.artwork_pipeline.output_model,
                OutputModel::RgbScreen
            );
            assert_eq!(
                undone_rgb_document.pattern_state,
                edited_rgb_document.pattern_state
            );
            assert_realized_output_fixture_authority(&ui, &undone_rgb_document, selected);
            ui.redo();
            drain_ui_callbacks();
            let redone_cmyk_document = ui
                .state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .clone();
            assert_eq!(redone_cmyk_document.pattern_state, cmyk_authority);
            assert_realized_output_fixture_authority(&ui, &redone_cmyk_document, selected);
            assert_eq!(ui.output_mode.model().unwrap(), output_model);
        }

        eprintln!(
            "realized AppUi output-cache authority check: shipping GResource controls kept Polygon Six and Motif Ladder selection/typed values authoritative through CMYK/RGB transitions, contradictory active/inactive adapters, RGB edits, undo/redo, and independent preview/export presentation state"
        );
        ui.window.close();
    }

    fn drain_ui_callbacks() {
        for _ in 0..32 {
            if !glib::MainContext::default().iteration(false) {
                break;
            }
        }
    }

    fn verify_realized_semantic_pipeline_callbacks() {
        let application = adw::Application::builder()
            .application_id("dev.toniator.semantic-pipeline-regression")
            .build();
        application.register(None::<&gio::Cancellable>).unwrap();
        let ui = AppUi::new(
            &application,
            CliOptions {
                demo: true,
                artifact_window_size: Some((900, 680)),
                ..CliOptions::default()
            },
        );
        ui.window.present();
        drain_ui_callbacks();
        ui.activate_shape_treatment();
        drain_ui_callbacks();

        let source_model = ui.artwork_source.model().unwrap();
        let active_model = ui.active_channel.model().unwrap();
        assert!(!ui.state.borrow().syncing_controls);
        assert!(!ui.channel_assignment.is_sensitive());
        assert_eq!(
            ui.channel_assignment_note.text(),
            "Automatic CMYK Separation derives cyan, magenta, yellow, and black inks."
        );
        for source in 0..7 {
            ui.artwork_source.set_selected(source);
            drain_ui_callbacks();
            assert_eq!(ui.artwork_source.selected(), source);
            let pipeline = ui
                .state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .artwork_pipeline
                .clone();
            assert_eq!(pipeline.source, artwork_source_from_index(source).unwrap());
            if source == 0 {
                assert!(matches!(
                    pipeline.assignment,
                    ChannelAssignment::Automatic { .. }
                ));
            } else {
                assert!(matches!(
                    pipeline.assignment,
                    ChannelAssignment::AllChannels
                ));
            }
            if source == 6 {
                assert!(!ui.source_alpha_row.is_visible());
                assert!(ui.source_alpha_note.is_visible());
                assert_eq!(
                    ui.source_alpha_note.text(),
                    "Alpha is the source; source alpha is not applied again."
                );
            }
        }
        assert_eq!(ui.artwork_source.model().unwrap(), source_model);

        ui.channel_assignment.set_selected(0);
        drain_ui_callbacks();
        ui.active_channel.set_selected(3);
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .artwork_pipeline
                .active_channel,
            Some(OutputChannelId::CmykBlack)
        );
        ui.active_channel.set_selected(gtk::INVALID_LIST_POSITION);
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .artwork_pipeline
                .active_channel,
            Some(OutputChannelId::CmykBlack)
        );

        for output in [
            OutputMode::RgbScreen,
            OutputMode::CmykInks,
            OutputMode::RgbScreen,
            OutputMode::CmykInks,
            OutputMode::RgbScreen,
        ] {
            ui.output_mode
                .set_selected((output == OutputMode::RgbScreen) as u32);
            drain_ui_callbacks();
            assert_eq!(
                ui.state
                    .borrow()
                    .editor
                    .as_ref()
                    .unwrap()
                    .document()
                    .output_mode,
                output
            );
            assert_eq!(ui.active_channel.model().unwrap(), active_model);
        }
        assert_eq!(ui.active_channel.model().unwrap().n_items(), 3);
        ui.artwork_source.set_selected(0);
        drain_ui_callbacks();
        assert!(!ui.channel_assignment.is_sensitive());
        assert_eq!(
            ui.channel_assignment_note.text(),
            "Direct RGB Channels map encoded red, green, and blue components."
        );
        ui.artwork_source.set_selected(4);
        drain_ui_callbacks();
        ui.source_alpha.set_selected(1);
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .artwork_pipeline
                .alpha_policy,
            SourceAlphaPolicy::Ignore
        );
        ui.channel_assignment.set_selected(0);
        drain_ui_callbacks();
        ui.active_channel.set_selected(2);
        drain_ui_callbacks();
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .artwork_pipeline
                .active_channel,
            Some(OutputChannelId::RgbBlue)
        );
        ui.artwork_source.set_selected(6);
        drain_ui_callbacks();
        assert!(!ui.source_alpha.is_sensitive());

        ui.crosshatch_action.emit_clicked();
        drain_ui_callbacks();
        let pipeline = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .artwork_pipeline
            .clone();
        assert!(matches!(
            pipeline.assignment,
            ChannelAssignment::LegacyCompatibility(_)
        ));
        assert_eq!(pipeline.output_model, OutputModel::RgbScreen);
        assert!(matches!(
            ui.state.borrow().editor.as_ref().unwrap().document().render,
            RenderVariant::WebCurveV1 { .. }
        ));
        assert_eq!(
            ui.crosshatch_note.text(),
            "Legacy Crosshatch is active in Curves. Exit restores ordinary Curves."
        );
        ui.crosshatch_action.emit_clicked();
        drain_ui_callbacks();
        let pipeline = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .artwork_pipeline
            .clone();
        assert_eq!(pipeline.output_model, OutputModel::RgbScreen);
        assert_eq!(pipeline.source, ArtworkSource::Value);
        assert!(matches!(
            pipeline.assignment,
            ChannelAssignment::AllChannels
        ));
        assert_eq!(
            ui.crosshatch_note.text(),
            "Legacy Crosshatch temporarily switches to Curves. Exit restores ordinary Curves."
        );
        ui.window.close();
    }

    // Superseded by the semantic-pipeline selector regression below.  Keep
    // the historic fixture out of the test build while its compatibility
    // artifact vocabulary remains available only through the CLI adapter.
    #[cfg(any())]
    fn assert_selector_state(ui: &AppUi, output: OutputMode, curve: bool) {
        let state = ui.state.borrow();
        assert!(!state.syncing_controls);
        let editor = state
            .editor
            .as_ref()
            .expect("realized fixture has a document");
        assert_eq!(editor.document().output_mode, output);
        let expected_render = if curve {
            matches!(editor.document().render, RenderVariant::WebCurveV1 { .. })
        } else {
            matches!(editor.document().render, RenderVariant::WebShapeV1 { .. })
        };
        assert!(
            expected_render,
            "expected {} render, got {:?}",
            if curve { "Curves" } else { "Shapes" },
            editor.document().render
        );
        let crosshatch = matches!(
            &editor.document().render,
            RenderVariant::WebCurveV1 { settings }
                if settings.value_mode == ValueMode::CrosshatchLuminance
        ) || matches!(
            &editor.document().render,
            RenderVariant::WebShapeV1 { settings }
                if settings.value_mode == ValueMode::CrosshatchLuminance
        );
        drop(state);
        let target_count = if output == OutputMode::RgbScreen && !crosshatch {
            4
        } else {
            5
        };
        let output_count = if output == OutputMode::RgbScreen {
            3
        } else {
            4
        };
        let (target, output_control, visible) = if curve {
            (&ui.curve_target, &ui.curve_output_ink, &ui.curve_visible)
        } else {
            (&ui.web_target, &ui.web_output_ink, &ui.web_visible)
        };
        assert_eq!(target.model().unwrap().n_items(), target_count);
        assert_eq!(output_control.model().unwrap().n_items(), output_count);
        assert!(target.selected() < target_count);
        if output == OutputMode::RgbScreen && !crosshatch {
            assert!(!visible[3].is_visible());
        }
    }

    #[cfg(any())]
    fn verify_realized_appui_selector_callbacks() {
        let application = adw::Application::builder()
            .application_id("dev.toniator.selector-regression")
            .build();
        application.register(None::<&gio::Cancellable>).unwrap();
        let ui = AppUi::new(
            &application,
            CliOptions {
                demo: true,
                artifact_window_size: Some((900, 680)),
                ..CliOptions::default()
            },
        );
        ui.window.present();
        drain_ui_callbacks();
        ui.activate_shape_treatment();
        drain_ui_callbacks();
        assert_selector_state(&ui, OutputMode::CmykInks, false);

        // UI synchronization is a read-only projection of the semantic
        // pipeline, even if a temporary renderer facade is contradictory.
        let mut contradictory = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .clone();
        contradictory
            .apply_legacy_mapping_action(ValueMode::SingleChannel)
            .unwrap();
        contradictory
            .select_active_output_channel(toniator::artwork_pipeline::OutputChannelId::CmykBlack)
            .unwrap();
        contradictory.output_mode = OutputMode::RgbScreen;
        let RenderVariant::WebShapeV1 { settings } = &mut contradictory.render else {
            panic!("realized fixture is Shapes")
        };
        settings.value_mode = ValueMode::Rgb;
        settings.single_channel = Ink::Red;
        ui.state.borrow_mut().editor = Some(DocumentEditor::new(contradictory));
        ui.sync_controls();
        assert_eq!(ui.output_mode.selected(), 0);
        assert_eq!(ui.web_value_mode.selected(), 1);
        assert_eq!(ui.web_output_ink.selected(), 3);
        assert_eq!(
            ui.state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .artwork_pipeline
                .active_channel,
            Some(toniator::artwork_pipeline::OutputChannelId::CmykBlack)
        );
        let mut canonical = ui
            .state
            .borrow()
            .editor
            .as_ref()
            .unwrap()
            .document()
            .clone();
        canonical.sync_legacy_projection().unwrap();
        ui.state.borrow_mut().editor = Some(DocumentEditor::new(canonical));
        ui.sync_controls();

        for selected in [0, 1, 0, 1] {
            ui.preview_surface.set_selected(selected);
            ui.export_background.set_selected(selected);
            drain_ui_callbacks();
            let appearance = ui
                .state
                .borrow()
                .editor
                .as_ref()
                .unwrap()
                .document()
                .appearance;
            assert_eq!(
                matches!(appearance.preview_surface, PreviewSurface::Color { .. }),
                selected == 1
            );
            assert_eq!(
                matches!(appearance.export_background, ExportBackground::Color { .. }),
                selected == 1
            );
        }

        // Repeat the output transition while the real selected-notify handler
        // defers its model synchronization to the idle queue.
        for output in [
            OutputMode::RgbScreen,
            OutputMode::CmykInks,
            OutputMode::RgbScreen,
            OutputMode::CmykInks,
            OutputMode::RgbScreen,
        ] {
            ui.output_mode
                .set_selected((output == OutputMode::RgbScreen) as u32);
            drain_ui_callbacks();
            assert_selector_state(&ui, output, false);
        }

        // Shapes: every mapping, output channel, target, and mark choice uses
        // the production selected-notify callbacks.
        for mapping in [0, 1, 2, 4] {
            ui.web_value_mode.set_selected(mapping);
            drain_ui_callbacks();
            assert_selector_state(
                &ui,
                if mapping == 4 {
                    OutputMode::RgbScreen
                } else {
                    OutputMode::CmykInks
                },
                false,
            );
        }
        // The Shapes mapping's Crosshatch entry transitions to the production
        // curve-based hatch treatment. Exercise every layer target after that
        // transition, including the deferred target selected-notify sync.
        ui.web_value_mode.set_selected(3);
        drain_ui_callbacks();
        assert_selector_state(&ui, OutputMode::RgbScreen, true);
        for target in 0..ui.curve_target.model().unwrap().n_items() {
            ui.curve_target.set_selected(target);
            drain_ui_callbacks();
            assert_eq!(
                ui.selected_curve_inks(),
                web_inks_for_target(ui.curve_target.selected(), false, true)
            );
        }
        ui.dots.set_active(true);
        drain_ui_callbacks();
        ui.web_value_mode.set_selected(4);
        drain_ui_callbacks();
        for target in 0..ui.web_target.model().unwrap().n_items() {
            ui.web_target.set_selected(target);
            drain_ui_callbacks();
            assert_eq!(
                ui.selected_web_inks(),
                web_inks_for_target(target, true, false)
            );
        }
        ui.web_value_mode.set_selected(1);
        drain_ui_callbacks();
        for channel in 0..ui.web_output_ink.model().unwrap().n_items() {
            ui.web_output_ink.set_selected(channel);
            drain_ui_callbacks();
            let state = ui.state.borrow();
            let editor = state.editor.as_ref().unwrap();
            let RenderVariant::WebShapeV1 { settings } = &editor.document().render else {
                panic!("Shapes output selector changed treatment");
            };
            assert_eq!(
                settings.single_channel,
                output_ink_for_slot(channel, true).unwrap()
            );
        }
        for mark in 0..ui.web_shape.model().unwrap().n_items() {
            ui.web_shape.set_selected(mark);
            drain_ui_callbacks();
            let expected = match mark {
                0 => WebShape::Circle,
                1 => WebShape::RegularPolygon,
                2 => WebShape::UserDefined,
                _ => continue,
            };
            let state = ui.state.borrow();
            let editor = state.editor.as_ref().unwrap();
            let RenderVariant::WebShapeV1 { settings } = &editor.document().render else {
                panic!("Shapes mark selector changed treatment");
            };
            if settings.use_shared_mark {
                assert_eq!(settings.shared_shape, expected);
            } else {
                for ink in ui.selected_web_inks().unwrap() {
                    assert_eq!(settings.channels.get(ink).shape, expected);
                }
            }
        }
        for button in &ui.web_visible[..3] {
            button.set_active(!button.is_active());
            button.set_active(!button.is_active());
        }
        drain_ui_callbacks();
        ui.web_value_mode.set_selected(0);
        drain_ui_callbacks();
        ui.web_value_mode.set_selected(4);
        drain_ui_callbacks();
        assert_selector_state(&ui, OutputMode::RgbScreen, false);

        // Curves repeats the same lifecycle, including layout/profile selectors
        // and all valid active channel positions.
        ui.curves.set_active(true);
        drain_ui_callbacks();
        assert_selector_state(&ui, OutputMode::RgbScreen, true);
        for mapping in 0..5 {
            ui.curve_value_mode.set_selected(mapping);
            drain_ui_callbacks();
            assert_selector_state(
                &ui,
                if mapping == 4 {
                    OutputMode::RgbScreen
                } else {
                    OutputMode::CmykInks
                },
                true,
            );
        }
        ui.curve_value_mode.set_selected(4);
        drain_ui_callbacks();
        for target in 0..ui.curve_target.model().unwrap().n_items() {
            ui.curve_target.set_selected(target);
            drain_ui_callbacks();
            assert_eq!(
                ui.selected_curve_inks(),
                web_inks_for_target(target, true, false)
            );
        }
        ui.curve_value_mode.set_selected(1);
        drain_ui_callbacks();
        for channel in 0..ui.curve_output_ink.model().unwrap().n_items() {
            ui.curve_output_ink.set_selected(channel);
            drain_ui_callbacks();
            let state = ui.state.borrow();
            let editor = state.editor.as_ref().unwrap();
            let RenderVariant::WebCurveV1 { settings } = &editor.document().render else {
                panic!("Curves output selector changed treatment");
            };
            assert_eq!(
                settings.single_channel,
                output_ink_for_slot(channel, true).unwrap()
            );
        }
        for layout in 0..2 {
            ui.curve_layout.set_selected(layout);
            drain_ui_callbacks();
            let state = ui.state.borrow();
            let editor = state.editor.as_ref().unwrap();
            let RenderVariant::WebCurveV1 { settings } = &editor.document().render else {
                panic!("Curves layout selector changed treatment");
            };
            assert_eq!(
                settings.layout,
                if layout == 1 {
                    CurveLayout::MotifPattern
                } else {
                    CurveLayout::FullWidth
                }
            );
        }
        for profile in 0..3 {
            ui.curve_profile.set_selected(profile);
            drain_ui_callbacks();
            let expected = match profile {
                0 => CurvePath::straight(),
                1 => CurvePath::soft_wave(),
                2 => CurvePath::deep_wave(),
                _ => unreachable!(),
            };
            let state = ui.state.borrow();
            let editor = state.editor.as_ref().unwrap();
            let RenderVariant::WebCurveV1 { settings } = &editor.document().render else {
                panic!("Curves profile selector changed treatment");
            };
            assert_eq!(settings.shared_path, expected);
        }
        for button in &ui.curve_visible[..3] {
            button.set_active(!button.is_active());
            button.set_active(!button.is_active());
        }
        drain_ui_callbacks();
        assert_selector_state(&ui, OutputMode::RgbScreen, true);

        // Return through CMYK and Shapes, which proves repeated treatment and
        // list synchronization remains stable after both selector families.
        ui.output_mode.set_selected(0);
        drain_ui_callbacks();
        ui.dots.set_active(true);
        drain_ui_callbacks();
        assert_selector_state(&ui, OutputMode::CmykInks, false);
        assert_eq!(ui.web_target.model().unwrap().n_items(), 5);
        eprintln!(
            "realized AppUi selector regression exercised repeated output/treatment switches, all mapping, target/output, mark, curve layout/profile, visibility, and appearance DropDown callbacks without a RefCell conflict"
        );
        ui.window.close();
    }

    #[test]
    fn crosshatch_hint_endpoints_exactly_match_production_default_directions() {
        let mut settings = WebCurveSettings::default();
        settings.configure_crosshatch();
        for (ink, angle) in [
            (Ink::Black, 45.0),
            (Ink::Cyan, -45.0),
            (Ink::Magenta, 0.0),
            (Ink::Yellow, 90.0),
        ] {
            assert_eq!(settings.channels.get(ink).grid_rotation, angle);
        }
    }

    #[test]
    fn crosshatch_straight_reset_is_one_undoable_production_edit() {
        let mut document = Document::new(SourceArtwork {
            name: "hatch.svg".into(),
            media_type: "image/svg+xml".into(),
            bytes: Arc::from(b"<svg/>".as_slice()),
        });
        let mut settings = WebCurveSettings::default();
        settings.configure_crosshatch();
        settings.use_shared_curve = false;
        for ink in Ink::ALL {
            let channel = settings.channels.get_mut(ink);
            channel.path = CurvePath::deep_wave();
            channel.close_ends = true;
            channel.smooth_join = true;
        }
        document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(settings.clone()),
        };
        let mut editor = DocumentEditor::new(document);
        assert!(editor.select_pattern(toniator::PatternId::COMPATIBILITY_CURVES_V1));
        assert!(editor.set_curve_settings(settings.clone()));
        assert!(editor.apply_legacy_mapping_action(ValueMode::CrosshatchLuminance));
        let expected_after_mapping = editor.document().render.clone();
        reset_crosshatch_curve_path(&mut settings, &[Ink::Black]);
        assert!(editor.set_curve_settings(settings.clone()));
        assert_eq!(settings.channels.k.path, CurvePath::straight());
        assert!(!settings.channels.k.close_ends && !settings.channels.k.smooth_join);
        assert_eq!(settings.channels.c.path, CurvePath::deep_wave());
        assert!(editor.undo());
        assert!(matches!(
            &editor.document().render,
            RenderVariant::WebCurveV1 { settings } if **settings == match &expected_after_mapping {
                RenderVariant::WebCurveV1 { settings } => (**settings).clone(),
                _ => unreachable!(),
            }
        ));
        assert!(editor.redo());
        assert!(matches!(
            &editor.document().render,
            RenderVariant::WebCurveV1 { settings: redone } if **redone == settings
        ));
    }

    #[cfg(any())]
    #[test]
    fn source_mapping_embedded_svg_pairs_parse_render_and_match_the_table() {
        assert!(std::ptr::eq(
            SOURCE_MAPPING_OPTIONS[0].source_svg,
            COLOR_SOURCE_SVG
        ));
        assert!(std::ptr::eq(
            SOURCE_MAPPING_OPTIONS[0].result_svg,
            COLOR_TO_CMYK_SVG
        ));
        for option in &SOURCE_MAPPING_OPTIONS[1..4] {
            assert!(std::ptr::eq(option.source_svg, VALUE_SOURCE_SVG));
        }
        for (option, expected) in SOURCE_MAPPING_OPTIONS.iter().zip([
            COLOR_TO_CMYK_SVG,
            VALUE_TO_ONE_INK_SVG,
            VALUE_TO_CMYK_SVG,
            VALUE_TO_CROSSHATCH_SVG,
            COLOR_TO_RGB_SVG,
        ]) {
            assert!(!option.source_description.is_empty());
            assert!(!option.result_description.is_empty());
            assert!(std::ptr::eq(option.result_svg, expected));
            for bytes in [option.source_svg, option.result_svg] {
                assert!(
                    bytes.windows(4).any(|window| window == b"<svg"),
                    "Artwork Mapping entries must remain embedded SVG bytes"
                );
                let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).unwrap();
                assert!(tree.size().width() > 0.0 && tree.size().height() > 0.0);
                let texture =
                    render_embedded_svg_texture(bytes, SOURCE_MAPPING_ARTWORK_SIZE, 1).unwrap();
                assert!(texture.width() > 0 && texture.height() > 0);
                assert_eq!(
                    texture.width().max(texture.height()),
                    SOURCE_MAPPING_ARTWORK_SIZE
                );
                let hidpi =
                    render_embedded_svg_texture(bytes, SOURCE_MAPPING_ARTWORK_SIZE, 2).unwrap();
                assert_eq!(
                    hidpi.width().max(hidpi.height()),
                    SOURCE_MAPPING_ARTWORK_SIZE * 2
                );
            }
        }
        assert_ne!(COLOR_TO_RGB_SVG, COLOR_TO_CMYK_SVG);
        assert!(
            std::str::from_utf8(COLOR_TO_RGB_SVG)
                .unwrap()
                .contains("RGB additive screen channels")
        );
    }

    #[test]
    fn new_guard_production_coordinator_preserves_or_clears_real_state() {
        #[derive(Clone, Copy, Debug)]
        enum Case {
            Clean,
            Cancel,
            SaveAsCancel,
            WriteFailure,
            SaveCleanupFailure,
            DiscardCleanupFailure,
            Saved,
            Discarded,
        }

        #[derive(Debug, PartialEq)]
        struct Snapshot {
            document_id: Option<String>,
            path: Option<PathBuf>,
            dirty: bool,
            source_cache: bool,
            rendered_cache: bool,
            preview_size: Option<(u32, u32)>,
            started: bool,
            recovery_exists: bool,
        }

        fn snapshot(state: &AppState, started: bool, recovery_exists: bool) -> Snapshot {
            Snapshot {
                document_id: state
                    .editor
                    .as_ref()
                    .map(|editor| editor.document().document_id.clone()),
                path: state.path.clone(),
                dirty: state.editor.as_ref().is_some_and(DocumentEditor::is_dirty),
                source_cache: state.source_cache.is_some(),
                rendered_cache: state.rendered_cache.is_some(),
                preview_size: state.preview_size,
                started,
                recovery_exists,
            }
        }

        fn fixture(dirty: bool) -> AppState {
            let document = Document::new(SourceArtwork {
                name: "guard.svg".into(),
                media_type: "image/svg+xml".into(),
                bytes: Arc::from(b"<svg/>".as_slice()),
            });
            let cache = PreviewCache {
                document: document.clone(),
                image: RgbaImage::new(320, 180),
            };
            let mut editor = DocumentEditor::new(document);
            editor.mark_clean();
            if dirty {
                let mut settings = editor.document().settings;
                settings.coverage += 1.0;
                assert!(editor.set_settings(SettingKey::Coverage, settings));
            }
            AppState {
                editor: Some(editor),
                path: Some(PathBuf::from("current.toniator")),
                syncing_controls: false,
                preview_size: Some((320, 180)),
                compare_source: false,
                zoom_mode: ZoomMode::Fit(100.0),
                source_cache: Some(cache.clone()),
                rendered_cache: Some(cache),
            }
        }

        fn run(case: Case) -> (Snapshot, Snapshot, bool) {
            let dirty = !matches!(case, Case::Clean);
            let mut state = fixture(dirty);
            if matches!(case, Case::SaveAsCancel) {
                state.path = None;
            }
            let mut recovery_exists = dirty;
            let mut started = false;
            let before = snapshot(&state, started, recovery_exists);
            let mut action = DirtyTransitionCoordinator::begin(dirty);
            if action == DirtyTransitionAction::Prompt {
                action = DirtyTransitionCoordinator::choose(match case {
                    Case::Cancel => DirtyTransitionChoice::Cancel,
                    Case::Discarded | Case::DiscardCleanupFailure => DirtyTransitionChoice::Discard,
                    _ => DirtyTransitionChoice::Save,
                });
            }
            match action {
                DirtyTransitionAction::Save => {
                    let outcome = match case {
                        Case::SaveAsCancel => None,
                        Case::WriteFailure => Some(SaveTransitionOutcome::WriteFailed),
                        Case::SaveCleanupFailure => {
                            Some(SaveTransitionOutcome::RecoveryCleanupFailed)
                        }
                        Case::Saved => Some(SaveTransitionOutcome::Saved),
                        _ => unreachable!(),
                    };
                    action = outcome.map_or(DirtyTransitionAction::Stay, |outcome| {
                        if outcome == SaveTransitionOutcome::Saved {
                            recovery_exists = false;
                            state.path = Some(PathBuf::from("saved.toniator"));
                            state.editor.as_mut().unwrap().mark_clean();
                        }
                        DirtyTransitionCoordinator::save_finished(outcome)
                    });
                }
                DirtyTransitionAction::ClearRecovery => {
                    let cleanup_ok = matches!(case, Case::Discarded);
                    if cleanup_ok {
                        recovery_exists = false;
                    }
                    action = DirtyTransitionCoordinator::cleanup_finished(cleanup_ok);
                }
                _ => {}
            }
            let saved_path_was_observed =
                state.path.as_deref() == Some(Path::new("saved.toniator"));
            if action == DirtyTransitionAction::Continue {
                clear_document_for_new_project(&mut state);
                started = true;
            }
            (
                before,
                snapshot(&state, started, recovery_exists),
                saved_path_was_observed,
            )
        }

        for case in [
            Case::Cancel,
            Case::SaveAsCancel,
            Case::WriteFailure,
            Case::SaveCleanupFailure,
            Case::DiscardCleanupFailure,
        ] {
            let (before, after, _) = run(case);
            assert_eq!(after, before, "{case:?} must preserve working state");
        }
        let (_, clean, _) = run(Case::Clean);
        assert!(clean.started);
        assert!(clean.document_id.is_none() && clean.path.is_none());
        assert!(!clean.source_cache && !clean.rendered_cache && clean.preview_size.is_none());
        assert!(!clean.recovery_exists);

        let (_, saved, saved_path_was_observed) = run(Case::Saved);
        assert!(saved_path_was_observed);
        assert!(saved.started && saved.document_id.is_none() && saved.path.is_none());
        assert!(!saved.dirty && !saved.recovery_exists);
        assert!(!saved.source_cache && !saved.rendered_cache && saved.preview_size.is_none());

        let (_, discarded, _) = run(Case::Discarded);
        assert!(discarded.started && discarded.document_id.is_none());
        assert!(discarded.path.is_none() && !discarded.dirty && !discarded.recovery_exists);
        assert!(!discarded.source_cache && !discarded.rendered_cache);
        eprintln!(
            "production New guard: clean=continued; cancel/save-as-cancel/write-failure/save-cleanup-failure/discard-cleanup-failure=preserved document+path+dirty+caches+start+recovery; saved=observed saved path then cleaned recovery and started; discarded=cleaned recovery and started"
        );
    }
}
