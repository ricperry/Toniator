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
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use toniator::model::{ClosedShapePath, SettingKey, ShapeAnchor, ShapePoint, SourceArtwork};
use toniator::persistence::{clear_recovery_if_matches, recovery_path};
use toniator::{
    AlternateTileTransform, CurveLayout, CurvePath, CurvePoint, Document, DocumentEditor, Ink,
    MotifCoverage, RenderGate, RenderVariant, Settings, Treatment, ValueMode, WebCurveChannel,
    WebCurveSettings, WebShape, WebShapeSettings, export_svg, render_document_preview,
    save_document_atomic,
};

const EXAMPLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="960" height="680" viewBox="0 0 960 680">
<defs><linearGradient id="warm" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ffcf33"/><stop offset="0.48" stop-color="#ec008c"/><stop offset="1" stop-color="#0047ff"/></linearGradient><radialGradient id="cool" cx="42%" cy="40%" r="70%"><stop offset="0" stop-color="#fff"/><stop offset="0.45" stop-color="#00aeef"/><stop offset="1" stop-color="#08111f"/></radialGradient></defs>
<rect width="100%" height="100%" fill="url(#warm)"/><circle cx="330" cy="310" r="235" fill="url(#cool)" opacity="0.92"/><rect x="565" y="115" width="260" height="365" rx="44" fill="#101114" opacity="0.78"/><path d="M90 555 C225 420 350 665 510 535 S745 440 870 585" fill="none" stroke="#fff" stroke-width="58" stroke-linecap="round" opacity="0.82"/><text x="620" y="345" font-family="sans-serif" font-size="122" font-weight="800" fill="#fff">T</text></svg>"##;

const BUNDLED_PRESETS: [(&str, &[u8]); 3] = [
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
];
const START_HERO: &[u8] = include_bytes!("../assets/splash-hero.png");
const PREVIEW_INDICATOR_SVG: &[u8] = include_bytes!("../assets/preview-indicator.svg");
const COLOR_SOURCE_SVG: &[u8] = include_bytes!("../icons/ColorSource.svg");
const COLOR_TO_CMYK_SVG: &[u8] = include_bytes!("../icons/ColorToCMYK.svg");
const VALUE_SOURCE_SVG: &[u8] = include_bytes!("../icons/ValueSource.svg");
const VALUE_TO_ONE_INK_SVG: &[u8] = include_bytes!("../icons/ValueToOneInk.svg");
const VALUE_TO_CMYK_SVG: &[u8] = include_bytes!("../icons/ValueToCMYK.svg");
const VALUE_TO_CROSSHATCH_SVG: &[u8] = include_bytes!("../icons/ValueToCrosshatch.svg");
const PREVIEW_INDICATOR_WIDTH: i32 = 40;
const PREVIEW_INDICATOR_HEIGHT: i32 = 28;
const PREVIEW_INDICATOR_RASTER_SCALE: i32 = 4;

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
    match &document.render {
        RenderVariant::WebShapeV1 { settings } => (settings.output_width, settings.output_height),
        RenderVariant::WebCurveV1 { settings } => (settings.output_width, settings.output_height),
        RenderVariant::NativeBasicV1 => (900, 620),
    }
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
        PreviewView::Source => cache.document.document_id == document.document_id,
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
            "Updating rendered preview"
        } else {
            "Rendered preview"
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
            Some(0.5) => "Updating rendered preview",
            Some(1.0) => "Rendered preview",
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

struct RenderRequest {
    generation: u64,
    document: Document,
    compare_source: bool,
    max_dimension: u32,
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
    result: anyhow::Result<()>,
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
    paned: gtk::Paned,
    desired_width: Cell<i32>,
    pending_width: Cell<Option<i32>>,
    user_dragging: Cell<bool>,
    state_path: Option<PathBuf>,
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
    fn new(paned: &gtk::Paned, desired_width: i32, state_path: Option<PathBuf>) -> Rc<Self> {
        Rc::new(Self {
            paned: paned.clone(),
            desired_width: Cell::new(desired_width.clamp(INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH)),
            pending_width: Cell::new(None),
            user_dragging: Cell::new(false),
            state_path,
        })
    }

    fn maintain(&self) {
        let total = self.paned.width();
        let actual = self.current_width();
        if total <= 0 || actual <= 0 || self.user_dragging.get() {
            return;
        }
        let target = constrained_inspector_width(self.desired_width.get(), total);
        if (actual - target).abs() <= 1 {
            self.pending_width.set(None);
            return;
        }
        self.pending_width.set(Some(target));
        let corrected = (total - target).clamp(0, total);
        if corrected != self.paned.position() {
            self.paned.set_position(corrected);
        }
    }

    fn begin_user_drag(&self, x: f64) {
        if (x - self.paned.position() as f64).abs() <= 18.0 {
            self.pending_width.set(None);
            self.user_dragging.set(true);
        }
    }

    fn finish_user_drag(&self) {
        if !self.user_dragging.replace(false) {
            return;
        }
        let width = self
            .current_width()
            .clamp(INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH);
        self.desired_width.set(width);
        if let Some(path) = self.state_path.as_ref()
            && let Err(error) = save_inspector_width(path, width)
        {
            eprintln!("Could not save inspector width: {error}");
        }
        self.maintain();
    }

    fn current_width(&self) -> i32 {
        (self.paned.width() - self.paned.position()).max(0)
    }
}

pub struct AppUi {
    window: adw::ApplicationWindow,
    open: gtk::Button,
    stack: gtk::Stack,
    toast_overlay: adw::ToastOverlay,
    title: gtk::Label,
    picture: gtk::Picture,
    canvas: gtk::ScrolledWindow,
    canvas_content: gtk::Overlay,
    inspector_pane: Rc<InspectorPaneController>,
    source_label: gtk::Label,
    preview_indicator: PreviewIndicator,
    autosave_status: gtk::Label,
    detail: gtk::Scale,
    coverage: gtk::Scale,
    contrast: gtk::Scale,
    angle: gtk::Scale,
    dots: gtk::ToggleButton,
    squares: gtk::ToggleButton,
    lines: gtk::ToggleButton,
    curves: gtk::ToggleButton,
    legacy: gtk::ToggleButton,
    treatment_modes: gtk::Stack,
    preset_import: gtk::Button,
    preset_save: gtk::Button,
    web_value_mode: gtk::DropDown,
    web_output_ink: gtk::DropDown,
    web_output_ink_row: gtk::Widget,
    web_shared: gtk::CheckButton,
    web_shape: gtk::DropDown,
    web_shape_row: gtk::Widget,
    web_mixed_shape_label: gtk::Label,
    web_mixed_shape_apply: gtk::DropDown,
    web_mixed_shape_apply_row: gtk::Widget,
    web_polygon_sides: gtk::SpinButton,
    web_polygon_sides_label: gtk::Label,
    web_edit_shape: gtk::Button,
    web_target: gtk::DropDown,
    web_target_label: gtk::Label,
    web_visible_label: gtk::Label,
    web_visible: [gtk::CheckButton; 4],
    web_color: gtk::Entry,
    web_color_row: gtk::Widget,
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
    web_opacity_status: gtk::Label,
    web_detail: gtk::Scale,
    web_detail_status: gtk::Label,
    web_mixed: gtk::Label,
    web_geometry_note: gtk::Label,
    curve_value_mode: gtk::DropDown,
    curve_output_ink: gtk::DropDown,
    curve_output_ink_row: gtk::Widget,
    curve_layout: gtk::DropDown,
    curve_profile: gtk::DropDown,
    curve_editor_label: gtk::Label,
    curve_editor: gtk::DrawingArea,
    curve_reset: gtk::Button,
    curve_shared: gtk::CheckButton,
    curve_target: gtk::DropDown,
    curve_target_label: gtk::Label,
    curve_visible_label: gtk::Label,
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
    capture_root: gtk::Widget,
    capture_paintable: gtk::WidgetPaintable,
    capture_override: RefCell<Option<gtk::Window>>,
    deferred_candidate_artifact: bool,
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

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(180)
            .build();
        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&stack));

        let title = gtk::Label::builder()
            .label("Toniator")
            .css_classes(["window-title"])
            .build();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&title));
        let new = action_button("New", "Start a new project");
        let open = action_button("Open", "Open artwork or document");
        let save = action_button("Save", "Save Toniator document");
        let undo = icon_button("edit-undo-symbolic", "Undo");
        let redo = icon_button("edit-redo-symbolic", "Redo");
        let export = action_button("Export…", "Export editable SVG or PNG image");
        header.pack_start(&new);
        header.pack_start(&open);
        header.pack_start(&save);
        header.pack_start(&undo);
        header.pack_start(&redo);
        header.pack_end(&export);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&toast_overlay));
        let window = adw::ApplicationWindow::builder()
            .application(application)
            .title("Toniator")
            .default_width(window_width.max(720))
            .default_height(window_height.max(520))
            .content(&toolbar)
            .build();

        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Contain)
            .can_shrink(true)
            .hexpand(true)
            .vexpand(true)
            .css_classes(["artboard"])
            .build();
        let source_label = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .build();
        let preview_indicator = PreviewIndicator::new(options.indicator_phase());
        let autosave_status = gtk::Label::builder()
            .label(if recovery_enabled {
                "Recovery is ready"
            } else {
                "Recovery is isolated for artifact capture"
            })
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label", "caption"])
            .build();
        let detail = control_scale(0.0, 100.0, 1.0);
        let coverage = control_scale(0.0, 160.0, 1.0);
        let contrast = control_scale(0.0, 200.0, 1.0);
        let angle = control_scale(-180.0, 180.0, 1.0);
        detail.set_format_value_func(|_, value| format!("{value:0.0}"));
        coverage.set_format_value_func(|_, value| format!("{value:0.0}%"));
        contrast.set_format_value_func(|_, value| format!("{value:0.0}%"));
        angle.set_format_value_func(|_, value| format!("{value:0.0}°"));
        let dots = gtk::ToggleButton::with_label("Shapes");
        let squares = gtk::ToggleButton::with_label("Squares");
        let lines = gtk::ToggleButton::with_label("Lines");
        let curves = gtk::ToggleButton::with_label("Curves");
        let legacy = gtk::ToggleButton::with_label("Legacy");
        squares.set_group(Some(&dots));
        lines.set_group(Some(&dots));
        curves.set_group(Some(&dots));
        legacy.set_group(Some(&dots));
        legacy.set_visible(false);
        dots.set_active(true);
        let compare = gtk::ToggleButton::with_label("Source");

        let start = build_start_view(recovery_enabled && recovery_path().exists());
        stack.add_named(&start.container, Some("start"));
        let editor_view = build_editor_view(
            &picture,
            &source_label,
            &preview_indicator.area,
            &autosave_status,
            &detail,
            &coverage,
            &contrast,
            &angle,
            &dots,
            &squares,
            &lines,
            &curves,
            &legacy,
            &compare,
            initial_inspector_width,
            window_width,
        );
        stack.add_named(&editor_view.container, Some("editor"));
        stack.set_visible_child_name("start");
        let fit = editor_view.fit.clone();
        let zoom = editor_view.zoom.clone();
        let zoom_entry = editor_view.zoom_entry.clone();
        let inspector_pane = InspectorPaneController::new(
            &editor_view.paned,
            initial_inspector_width,
            inspector_state_path,
        );

        let ui = Rc::new(Self {
            window,
            open: open.clone(),
            stack,
            toast_overlay,
            title,
            picture,
            canvas: editor_view.canvas.clone(),
            canvas_content: editor_view.canvas_content.clone(),
            inspector_pane,
            source_label,
            preview_indicator,
            autosave_status,
            detail,
            coverage,
            contrast,
            angle,
            dots,
            squares,
            lines,
            curves,
            legacy,
            treatment_modes: editor_view.treatment_modes.clone(),
            preset_import: editor_view.preset_import.clone(),
            preset_save: editor_view.preset_save.clone(),
            web_value_mode: editor_view.web_value_mode.clone(),
            web_output_ink: editor_view.web_output_ink.clone(),
            web_output_ink_row: editor_view.web_output_ink_row.clone(),
            web_shared: editor_view.web_shared.clone(),
            web_shape: editor_view.web_shape.clone(),
            web_shape_row: editor_view.web_shape_row.clone(),
            web_mixed_shape_label: editor_view.web_mixed_shape_label.clone(),
            web_mixed_shape_apply: editor_view.web_mixed_shape_apply.clone(),
            web_mixed_shape_apply_row: editor_view.web_mixed_shape_apply_row.clone(),
            web_polygon_sides: editor_view.web_polygon_sides.clone(),
            web_polygon_sides_label: editor_view.web_polygon_sides_label.clone(),
            web_edit_shape: editor_view.web_edit_shape.clone(),
            web_target: editor_view.web_target.clone(),
            web_target_label: editor_view.web_target_label.clone(),
            web_visible_label: editor_view.web_visible_label.clone(),
            web_visible: editor_view.web_visible.clone(),
            web_color: editor_view.web_color.clone(),
            web_color_row: editor_view.web_color_row.clone(),
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
            web_opacity_status: editor_view.web_opacity_status.clone(),
            web_detail: editor_view.web_detail.clone(),
            web_detail_status: editor_view.web_detail_status.clone(),
            web_mixed: editor_view.web_mixed.clone(),
            web_geometry_note: editor_view.web_geometry_note.clone(),
            curve_value_mode: editor_view.curve_value_mode.clone(),
            curve_output_ink: editor_view.curve_output_ink.clone(),
            curve_output_ink_row: editor_view.curve_output_ink_row.clone(),
            curve_layout: editor_view.curve_layout.clone(),
            curve_profile: editor_view.curve_profile.clone(),
            curve_editor_label: editor_view.curve_editor_label.clone(),
            curve_editor: editor_view.curve_editor.clone(),
            curve_reset: editor_view.curve_reset.clone(),
            curve_shared: editor_view.curve_shared.clone(),
            curve_target: editor_view.curve_target.clone(),
            curve_target_label: editor_view.curve_target_label.clone(),
            curve_visible_label: editor_view.curve_visible_label.clone(),
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
            capture_root: toolbar.clone().upcast(),
            capture_paintable: gtk::WidgetPaintable::new(Some(&toolbar)),
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
                let curves_active = ui.curves.is_active();
                if source_mapping_from_index(mapping).is_none() {
                    eprintln!("Source Mapping artifact index {mapping} is outside 0 through 3");
                } else if curves_active {
                    ui.curve_value_mode.set_selected(mapping);
                } else {
                    ui.web_value_mode.set_selected(mapping);
                }
                if mapping == 1 {
                    if curves_active {
                        ui.curve_output_ink.set_selected(0);
                    } else {
                        ui.web_output_ink.set_selected(0);
                    }
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
        } else if let Some(path) = options.artwork.as_ref() {
            ui.import_artwork(path);
        } else if let Some(path) = options.document.as_ref() {
            ui.open_document_path(path);
        }
        ui
    }

    pub fn present(self: &Rc<Self>) {
        self.window.present();
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
        let pane_events = gtk::EventControllerLegacy::new();
        pane_events.set_propagation_phase(gtk::PropagationPhase::Capture);
        let controller = Rc::downgrade(&self.inspector_pane);
        pane_events.connect_event(move |_, event| {
            let Some(button) = event.downcast_ref::<gdk::ButtonEvent>() else {
                return glib::Propagation::Proceed;
            };
            if button.button() != gdk::BUTTON_PRIMARY {
                return glib::Propagation::Proceed;
            }
            if let Some(controller) = controller.upgrade() {
                match event.event_type() {
                    gdk::EventType::ButtonPress => {
                        let (x, _) = event.position().unwrap_or_default();
                        controller.begin_user_drag(x);
                    }
                    gdk::EventType::ButtonRelease => controller.finish_user_drag(),
                    _ => {}
                }
            }
            glib::Propagation::Proceed
        });
        self.inspector_pane.paned.add_controller(pane_events);
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
        self.web_target.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |_| if !ui.state.borrow().syncing_controls {
                ui.sync_controls();
            }
        ));
        self.web_value_mode.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |combo| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let Some(mode) = source_mapping_from_index(combo.selected()) else {
                    return;
                };
                if mode == ValueMode::CrosshatchLuminance {
                    ui.activate_crosshatch_from_shape();
                } else {
                    ui.change_web_treatment(move |settings, _| settings.value_mode = mode);
                }
            }
        ));
        self.web_output_ink.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |combo| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let ink = match combo.selected() {
                    0 => Ink::Cyan,
                    1 => Ink::Magenta,
                    2 => Ink::Yellow,
                    3 => Ink::Black,
                    _ => return,
                };
                ui.change_web_treatment(move |settings, _| settings.single_channel = ink);
            }
        ));
        self.web_shared.connect_toggled(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                if !button.is_active() {
                    ui.change_web_treatment(|settings, _| {
                        let path = settings.resolved_custom_shape_path();
                        for ink in Ink::ALL {
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
                let target_ink = ui.selected_web_inks().first().copied();
                ui.change_web_treatment(move |settings, _| {
                    if settings.use_shared_mark {
                        settings.shared_shape = shape;
                    } else if target == 0 {
                        let path = settings.resolved_custom_shape_path();
                        let polygon_sides = settings.polygon_sides;
                        for ink in Ink::ALL {
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
                    ui.change_web_treatment(move |settings, _| {
                        settings.use_shared_mark = false;
                        let path = settings.resolved_custom_shape_path();
                        let polygon_sides = settings.polygon_sides;
                        for ink in Ink::ALL {
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
                let target_ink = ui.selected_web_inks().first().copied();
                ui.change_web_treatment(move |settings, _| {
                    if settings.use_shared_mark || target == 0 {
                        settings.polygon_sides = sides;
                        for ink in Ink::ALL {
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
                    let crosshatch = ui.state.borrow().editor.as_ref().is_some_and(|editor| {
                        matches!(&editor.document().render, RenderVariant::WebShapeV1 { settings }
                            if settings.value_mode == ValueMode::CrosshatchLuminance)
                    });
                    let ink = ink_for_visible_slot(index, crosshatch);
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
                    ui.show_error("Use a six-digit hex ink color such as #111111");
                    ui.sync_controls();
                }
                ui.end_setting_edit();
            }
        ));
        self.web_color.add_controller(color_focus);
        self.curve_value_mode.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |combo| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let Some(mode) = source_mapping_from_index(combo.selected()) else {
                    return;
                };
                ui.change_curve_treatment(move |settings, _| {
                    if mode == ValueMode::CrosshatchLuminance {
                        settings.configure_crosshatch();
                    } else {
                        settings.value_mode = mode;
                    }
                });
            }
        ));
        self.curve_output_ink.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |combo| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let ink = match combo.selected() {
                    0 => Ink::Cyan,
                    1 => Ink::Magenta,
                    2 => Ink::Yellow,
                    3 => Ink::Black,
                    _ => return,
                };
                ui.change_curve_treatment(move |settings, _| settings.single_channel = ink);
            }
        ));
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
                ui.sync_controls();
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
            let crosshatch = ui.state.borrow().editor.as_ref().is_some_and(|editor| {
                matches!(&editor.document().render, RenderVariant::WebCurveV1 { settings }
                    if settings.value_mode == ValueMode::CrosshatchLuminance)
            });
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
                ui.change_curve_treatment(move |settings, inks| {
                    if shared && !settings.use_shared_curve {
                        let ink = inks.first().copied().unwrap_or(Ink::Black);
                        settings.shared_path = settings.channels.get(ink).path.clone();
                        settings.shared_close_ends = settings.channels.get(ink).close_ends;
                        settings.shared_smooth_join = settings.channels.get(ink).smooth_join;
                    } else if !shared && settings.use_shared_curve {
                        for ink in Ink::ALL {
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
                    let crosshatch = ui.state.borrow().editor.as_ref().is_some_and(|editor| {
                        matches!(&editor.document().render, RenderVariant::WebCurveV1 { settings }
                            if settings.value_mode == ValueMode::CrosshatchLuminance)
                    });
                    let ink = ink_for_visible_slot(index, crosshatch);
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
                        ui.show_message("Please wait for the SVG export to finish before closing.");
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
        let state = self.state.borrow();
        let editor = state.editor.as_ref()?;
        let RenderVariant::WebCurveV1 { settings } = &editor.document().render else {
            return None;
        };
        let ink = self.selected_curve_inks().first().copied()?;
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
        let state = self.state.borrow();
        let editor = state.editor.as_ref()?;
        let RenderVariant::WebCurveV1 { settings } = &editor.document().render else {
            return None;
        };
        if settings.layout != CurveLayout::MotifPattern {
            return None;
        }
        let ink = self.selected_curve_inks().first().copied()?;
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
                matches!(
                    &editor.document().render,
                    RenderVariant::WebCurveV1 { settings }
                        if settings.layout == CurveLayout::MotifPattern
                )
            });
        self.motif_overlay.set_visible(active);
        if active {
            self.motif_overlay.queue_draw();
        }
    }

    fn current_curve_path(&self) -> Option<CurvePath> {
        let state = self.state.borrow();
        let editor = state.editor.as_ref()?;
        let RenderVariant::WebCurveV1 { settings } = &editor.document().render else {
            return None;
        };
        let inks = self.selected_curve_inks();
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
        let state = self.state.borrow();
        let Some(editor) = state.editor.as_ref() else {
            return (0.2, 0.55, 1.0);
        };
        let RenderVariant::WebCurveV1 { settings } = &editor.document().render else {
            return (0.2, 0.55, 1.0);
        };
        let inks = self.selected_curve_inks();
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
        self.title.set_text("Toniator");
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
        let document = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                return;
            };
            editor.document().clone()
        };
        let initial_name = match &document.render {
            RenderVariant::WebCurveV1 { .. } => "Curves Preset.tntr",
            _ => "Shapes Preset.tntr",
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
                        match toniator::preset::document_treatment_preset_bytes(&name, &document) {
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
            let parsed = (|| -> anyhow::Result<toniator::preset::ParsedTreatment> {
                let bytes = match source {
                    PresetSource::Path(path) => std::fs::read(path)?,
                    PresetSource::Bundled(bytes) => bytes.to_vec(),
                };
                let dimensions = toniator::render::source_dimensions(&document.source)?;
                let treatment = toniator::preset::parse_treatment(&bytes, dimensions)?;
                let mut candidate = document.clone();
                candidate.render = treatment.render.clone();
                if let Some(settings) = treatment.native_settings {
                    candidate.settings = settings;
                }
                candidate.validate()?;
                Ok(treatment)
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
                        Ok(treatment) => {
                            let canvas_normalized = treatment.canvas_normalized;
                            let changed = {
                                let mut state = ui.state.borrow_mut();
                                let Some(editor) = state.editor.as_mut() else {
                                    return glib::ControlFlow::Break;
                                };
                                if editor.document().document_id != document_id {
                                    return glib::ControlFlow::Break;
                                }
                                editor.set_treatment(treatment.render, treatment.native_settings)
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
                (|| -> anyhow::Result<(Document, toniator::persistence::DocumentMigration)> {
                    let (document, migration) = if is_document {
                        let loaded =
                            toniator::persistence::load_document_with_migration(&path_for_worker)?;
                        toniator::render::decode_source(&loaded.document.source, 128)?;
                        (loaded.document, loaded.migration)
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
                            toniator::persistence::DocumentMigration::default(),
                        )
                    };
                    Ok((document, migration))
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
                        Ok((document, migration)) => {
                            let install_path = if is_document && !recovered {
                                Some(path.clone())
                            } else {
                                None
                            };
                            ui.gate_dirty_transition(move |ui| {
                                ui.install_document_migrated(
                                    document.clone(),
                                    install_path.clone(),
                                    migration != toniator::persistence::DocumentMigration::default(),
                                );
                                if migration.canvas_aspect {
                                    ui.show_message("Canvas proportions were updated to match the source artwork; save to keep this change.");
                                } else if migration.crosshatch_treatment {
                                    ui.show_message("Legacy crosshatch was updated to genuine curve layers; save to keep this change.");
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
        self.install_document_migrated(document, path, false);
    }

    fn install_document_migrated(
        self: &Rc<Self>,
        mut document: Document,
        path: Option<PathBuf>,
        migrated_dirty: bool,
    ) {
        if let Ok((width, height)) = toniator::render::source_dimensions(&document.source) {
            document.normalize_canvas_aspect(width, height);
        }
        let should_autosave = path.is_none();
        let recovery_document = should_autosave.then(|| document.clone());
        {
            let mut state = self.state.borrow_mut();
            state.editor = Some(DocumentEditor::new_with_migration(document, migrated_dirty));
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

    fn selected_web_inks(&self) -> Vec<Ink> {
        let crosshatch = self.state.borrow().editor.as_ref().is_some_and(|editor| {
            matches!(&editor.document().render, RenderVariant::WebShapeV1 { settings }
                if settings.value_mode == ValueMode::CrosshatchLuminance)
        });
        match self.web_target.selected() {
            1 => vec![if crosshatch { Ink::Black } else { Ink::Cyan }],
            2 => vec![if crosshatch { Ink::Cyan } else { Ink::Magenta }],
            3 => vec![if crosshatch {
                Ink::Magenta
            } else {
                Ink::Yellow
            }],
            4 => vec![if crosshatch { Ink::Yellow } else { Ink::Black }],
            _ => Ink::ALL.to_vec(),
        }
    }

    fn open_shape_editor(self: &Rc<Self>) {
        let target = self.web_target.selected();
        let target_ink = self.selected_web_inks().first().copied();
        let Some(shape_path) =
            self.state.borrow().editor.as_ref().and_then(|editor| {
                match &editor.document().render {
                    RenderVariant::WebShapeV1 { settings } => {
                        Some(if settings.use_shared_mark || target == 0 {
                            settings.resolved_custom_shape_path()
                        } else {
                            let ink = target_ink.unwrap_or(Ink::Black);
                            settings.resolved_channel_shape_path(settings.channels.get(ink))
                        })
                    }
                    _ => None,
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
        dialog.present();
        area.grab_focus();
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

    fn selected_curve_inks(&self) -> Vec<Ink> {
        let crosshatch = self.state.borrow().editor.as_ref().is_some_and(|editor| {
            matches!(&editor.document().render, RenderVariant::WebCurveV1 { settings }
                if settings.value_mode == ValueMode::CrosshatchLuminance)
        });
        match self.curve_target.selected() {
            1 => vec![if crosshatch { Ink::Black } else { Ink::Cyan }],
            2 => vec![if crosshatch { Ink::Cyan } else { Ink::Magenta }],
            3 => vec![if crosshatch {
                Ink::Magenta
            } else {
                Ink::Yellow
            }],
            4 => vec![if crosshatch { Ink::Yellow } else { Ink::Black }],
            _ => Ink::ALL.to_vec(),
        }
    }

    fn activate_curve_treatment(self: &Rc<Self>) {
        let document = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            let settings = editor
                .document()
                .saved_web_curve
                .clone()
                .unwrap_or_else(|| Box::new(WebCurveSettings::default()));
            if !editor.set_render_variant(RenderVariant::WebCurveV1 { settings }) {
                return;
            }
            editor.document().clone()
        };
        self.after_treatment_edit(document);
    }

    fn activate_crosshatch_from_shape(self: &Rc<Self>) {
        let document = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            if !editor.convert_shape_to_crosshatch() {
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
            let settings = editor
                .document()
                .saved_web_shape
                .clone()
                .unwrap_or_else(|| Box::new(WebShapeSettings::default()));
            if !editor.set_render_variant(RenderVariant::WebShapeV1 { settings }) {
                return;
            }
            editor.document().clone()
        };
        self.after_treatment_edit(document);
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
        let inks = self.selected_curve_inks();
        let document = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            let RenderVariant::WebCurveV1 { settings } = &editor.document().render else {
                return;
            };
            let mut settings = (**settings).clone();
            update(&mut settings, inks);
            if !editor.set_render_variant(RenderVariant::WebCurveV1 {
                settings: Box::new(settings),
            }) {
                return;
            }
            editor.document().clone()
        };
        self.after_treatment_edit(document);
    }

    fn after_treatment_edit(&self, document: Document) {
        self.state.borrow_mut().rendered_cache = None;
        self.queue_autosave(document);
        self.sync_controls();
        self.request_rendered_preview();
        self.update_actions();
    }

    fn change_web_treatment(
        self: &Rc<Self>,
        update: impl FnOnce(&mut toniator::WebShapeSettings, Vec<Ink>),
    ) {
        let inks = self.selected_web_inks();
        let document = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            let RenderVariant::WebShapeV1 { settings } = &editor.document().render else {
                return;
            };
            let mut settings = (**settings).clone();
            update(&mut settings, inks);
            if !editor.set_render_variant(RenderVariant::WebShapeV1 {
                settings: Box::new(settings),
            }) {
                return;
            }
            editor.document().clone()
        };
        self.state.borrow_mut().rendered_cache = None;
        self.queue_autosave(document);
        self.sync_controls();
        self.request_rendered_preview();
        self.update_actions();
    }

    fn enable_shared_shape(self: &Rc<Self>) {
        let selected_ink = self.selected_web_inks().first().copied();
        let (target, equal) = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                return;
            };
            let RenderVariant::WebShapeV1 { settings } = &editor.document().render else {
                return;
            };
            let first = settings.channels.get(Ink::Cyan);
            let equal = Ink::ALL.into_iter().skip(1).all(|ink| {
                let channel = settings.channels.get(ink);
                channel.shape == first.shape
                    && channel.polygon_sides == first.polygon_sides
                    && settings.resolved_channel_shape_path(channel)
                        == settings.resolved_channel_shape_path(first)
            });
            (self.web_target.selected(), equal)
        };
        if equal {
            self.share_shape_from(Ink::Cyan);
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
            .body("This replaces the other inks' shape geometry as one undoable change.")
            .build();
        if target == 0 {
            dialog.add_responses(&[
                ("cancel", "Cancel"),
                ("c", "C"),
                ("m", "M"),
                ("y", "Y"),
                ("k", "K"),
            ]);
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

    fn sync_controls(&self) {
        let Some((settings, render, source_text)) =
            self.state.borrow().editor.as_ref().map(|editor| {
                (
                    editor.document().settings,
                    editor.document().render.clone(),
                    editor_source_text(editor.document()),
                )
            })
        else {
            return;
        };
        self.state.borrow_mut().syncing_controls = true;
        self.detail.set_value(settings.detail as f64);
        self.coverage.set_value(settings.coverage as f64);
        self.contrast.set_value(settings.contrast as f64);
        self.angle.set_value(settings.angle as f64);
        match settings.treatment {
            Treatment::Dots => self.dots.set_active(true),
            Treatment::Squares => self.squares.set_active(true),
            Treatment::Lines => self.lines.set_active(true),
        }
        self.angle
            .set_sensitive(settings.treatment != Treatment::Dots);
        match render {
            RenderVariant::NativeBasicV1 => {
                self.legacy.set_label(match settings.treatment {
                    Treatment::Dots => "Legacy Dots",
                    Treatment::Squares => "Legacy Squares",
                    Treatment::Lines => "Legacy Lines",
                });
                self.legacy.set_visible(true);
                self.legacy.set_active(true);
                self.treatment_modes.set_visible_child_name("native");
            }
            RenderVariant::WebShapeV1 { settings } => {
                self.legacy.set_visible(false);
                self.dots.set_active(true);
                self.treatment_modes.set_visible_child_name("web");
                self.web_value_mode.set_selected(match settings.value_mode {
                    ValueMode::Cmyk => 0,
                    ValueMode::SingleChannel => 1,
                    ValueMode::Luminance => 2,
                    ValueMode::CrosshatchLuminance => 3,
                });
                self.web_output_ink_row
                    .set_visible(settings.value_mode == ValueMode::SingleChannel);
                sync_layer_terminology(
                    &self.web_target,
                    &self.web_target_label,
                    &self.web_visible_label,
                    settings.value_mode == ValueMode::CrosshatchLuminance,
                );
                self.web_crosshatch_color_row
                    .set_visible(settings.value_mode == ValueMode::CrosshatchLuminance);
                self.web_color_row
                    .set_visible(settings.value_mode != ValueMode::CrosshatchLuminance);
                self.web_crosshatch_color
                    .set_text(&settings.crosshatch_color);
                self.web_output_ink
                    .set_selected(match settings.single_channel {
                        Ink::Cyan => 0,
                        Ink::Magenta => 1,
                        Ink::Yellow => 2,
                        Ink::Black => 3,
                    });
                self.web_shared.set_active(settings.use_shared_mark);
                let crosshatch = settings.value_mode == ValueMode::CrosshatchLuminance;
                let all_target = self.web_target.selected() == 0;
                let first_geometry = settings.channels.get(Ink::Cyan);
                let geometry_mixed = !settings.use_shared_mark
                    && Ink::ALL.into_iter().skip(1).any(|ink| {
                        let channel = settings.channels.get(ink);
                        channel.shape != first_geometry.shape
                            || channel.polygon_sides != first_geometry.polygon_sides
                            || settings.resolved_channel_shape_path(channel)
                                != settings.resolved_channel_shape_path(first_geometry)
                    });
                let selected_channel = self
                    .web_target
                    .selected()
                    .checked_sub(1)
                    .map(|index| ink_for_visible_slot(index as usize, crosshatch))
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
                        "One shape shared by all inks."
                    } else if all_target && geometry_mixed {
                        "Shapes differ. Choose a mark to apply it to all inks, or edit one ink."
                    } else {
                        "Editing this ink's shape."
                    });
                for (index, button) in self.web_visible.iter().enumerate() {
                    let ink = ink_for_visible_slot(index, crosshatch);
                    button.set_label(Some(if crosshatch {
                        ["1 K", "2 C", "3 M", "4 Y"][index]
                    } else {
                        ["C", "M", "Y", "K"][index]
                    }));
                    button.set_active(settings.channels.get(ink).enabled);
                }
                let inks = self.selected_web_inks();
                let all_inks = self.web_target.selected() == 0;
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
                        "Changing a Mixed control applies one value to every selected ink."
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
                    "Select one ink"
                } else if colors_mixed {
                    "Mixed"
                } else {
                    "#RRGGBB"
                }));
                self.web_color_status.set_text(if all_inks {
                    "Select one ink"
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
                );
                sync_web_scale(
                    &self.web_angle,
                    &self.web_angle_status,
                    first.grid_rotation,
                    mixed_fields[1],
                    "Rotate ink screen",
                );
                sync_web_scale(
                    &self.web_mark_angle,
                    &self.web_mark_angle_status,
                    first.rotation,
                    mixed_fields[2],
                    "Rotate marks",
                );
                sync_web_scale(
                    &self.web_width_scale,
                    &self.web_width_scale_status,
                    first.width_scale,
                    mixed_fields[3],
                    "Horizontal mark scale",
                );
                sync_web_scale(
                    &self.web_height_scale,
                    &self.web_height_scale_status,
                    first.height_scale,
                    mixed_fields[4],
                    "Vertical mark scale",
                );
                sync_web_scale(
                    &self.web_threshold,
                    &self.web_threshold_status,
                    first.threshold,
                    mixed_fields[5],
                    "Hide light marks",
                );
                sync_web_scale(
                    &self.web_opacity,
                    &self.web_opacity_status,
                    first.opacity,
                    mixed_fields[6],
                    "Transparent — Solid",
                );
                sync_web_scale(
                    &self.web_detail,
                    &self.web_detail_status,
                    first.resolution_scale,
                    mixed_fields[7],
                    "Sample density",
                );
            }
            RenderVariant::WebCurveV1 { settings } => {
                self.legacy.set_visible(false);
                self.curves.set_active(true);
                self.treatment_modes.set_visible_child_name("curve");
                self.curve_value_mode
                    .set_selected(match settings.value_mode {
                        ValueMode::Cmyk => 0,
                        ValueMode::SingleChannel => 1,
                        ValueMode::Luminance => 2,
                        ValueMode::CrosshatchLuminance => 3,
                    });
                self.curve_output_ink_row
                    .set_visible(settings.value_mode == ValueMode::SingleChannel);
                sync_layer_terminology(
                    &self.curve_target,
                    &self.curve_target_label,
                    &self.curve_visible_label,
                    settings.value_mode == ValueMode::CrosshatchLuminance,
                );
                self.curve_crosshatch_color_row
                    .set_visible(settings.value_mode == ValueMode::CrosshatchLuminance);
                self.curve_color_row
                    .set_visible(settings.value_mode != ValueMode::CrosshatchLuminance);
                self.curve_crosshatch_color
                    .set_text(&settings.crosshatch_color);
                self.curve_output_ink
                    .set_selected(match settings.single_channel {
                        Ink::Cyan => 0,
                        Ink::Magenta => 1,
                        Ink::Yellow => 2,
                        Ink::Black => 3,
                    });
                self.curve_layout.set_selected(match settings.layout {
                    CurveLayout::FullWidth => 0,
                    CurveLayout::MotifPattern => 1,
                });
                self.motif_controls
                    .set_visible(settings.layout == CurveLayout::MotifPattern);
                self.curve_shared.set_active(settings.use_shared_curve);
                let crosshatch = settings.value_mode == ValueMode::CrosshatchLuminance;
                self.curve_shared.set_label(Some(if crosshatch {
                    "Use One Hatch Path for All Layers"
                } else {
                    "Use One Curve for All Inks"
                }));
                self.curve_reset.set_label(if crosshatch {
                    "Reset to Straight Hatch"
                } else {
                    "Reset to Soft Wave"
                });
                for (index, button) in self.curve_visible.iter().enumerate() {
                    let ink = ink_for_visible_slot(index, crosshatch);
                    button.set_label(Some(if crosshatch {
                        ["1 K", "2 C", "3 M", "4 Y"][index]
                    } else {
                        ["C", "M", "Y", "K"][index]
                    }));
                    button.set_active(settings.channels.get(ink).enabled);
                }
                let inks = self.selected_curve_inks();
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
                );
                sync_web_scale(
                    &self.curve_angle,
                    &self.curve_angle_status,
                    first.grid_rotation,
                    mixed_fields[1],
                    "Rotate ink screen",
                );
                sync_web_scale(
                    &self.curve_position_x,
                    &self.curve_position_x_status,
                    first.offset_x,
                    mixed_fields[2],
                    "Move across",
                );
                sync_web_scale(
                    &self.curve_position_y,
                    &self.curve_position_y_status,
                    first.offset_y,
                    mixed_fields[3],
                    "Move vertically",
                );
                sync_web_scale(
                    &self.curve_opacity,
                    &self.curve_opacity_status,
                    first.opacity,
                    mixed_fields[4],
                    "Transparent — Solid",
                );
                sync_web_scale(
                    &self.curve_threshold,
                    &self.curve_threshold_status,
                    first.threshold,
                    mixed_fields[5],
                    "Hide light marks",
                );
                sync_web_scale(
                    &self.curve_detail,
                    &self.curve_detail_status,
                    first.resolution_scale,
                    mixed_fields[6],
                    "Sample density",
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
        }
        self.source_label.set_text(&source_text);
        self.state.borrow_mut().syncing_controls = false;
        self.sync_motif_overlay();
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
        });
    }

    fn queue_preview_request(&self, request: RenderRequest) {
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
                self.preview_generation.set(outcome.generation);
                self.install_preview(image, outcome.generation, outcome.view)
            }
            Err(error) => {
                if !self.gate.accepts(outcome.generation) {
                    return;
                }
                self.preview_indicator.failed(outcome.generation);
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
            self.capture_paintable
                .set_widget(Option::<&gtk::Widget>::None);
            self.capture_paintable.set_widget(Some(&self.capture_root));
            self.capture_paintable.invalidate_contents();
            self.capture_root.queue_draw();
            if let Some(window) = self.capture_override.borrow().as_ref() {
                window.queue_draw();
            }
            let frames = Rc::new(Cell::new(0u8));
            self.capture_root.add_tick_callback(glib::clone!(
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
            // A static start screen may not acquire another frame clock tick
            // after its first presentation. Keep screenshot-only New/start
            // artifacts finite; editor captures normally complete via the
            // two-frame path above before this fallback runs.
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
        if self.cli_artifacts_written.replace(true) {
            return;
        }
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
                self.inspector_pane.paned.position(),
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
        if let Some(path) = self.screenshot_path.as_ref()
            && let Err(error) = self.capture_window(path)
        {
            self.report_cli_artifact_error(format!(
                "Could not write window screenshot {}: {error:#}",
                path.display()
            ));
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

    fn capture_window(&self, path: &Path) -> anyhow::Result<()> {
        use gtk::gdk::prelude::PaintableExt;
        let override_window = self.capture_override.borrow().clone();
        let widget: gtk::Widget = override_window
            .as_ref()
            .map(|window| window.clone().upcast())
            .unwrap_or_else(|| self.capture_root.clone());
        let width = widget.width().max(1) as u32;
        let height = widget.height().max(1) as u32;
        let paintable = override_window.as_ref().map_or_else(
            || self.capture_paintable.clone(),
            |window| gtk::WidgetPaintable::new(Some(window)),
        );
        paintable.invalidate_contents();
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, width as f64, height as f64);
        let content_node = snapshot
            .to_node()
            .ok_or_else(|| anyhow::anyhow!("GTK produced no render node"))?;
        let node = opaque_capture_node(&content_node, width, height, capture_window_background());
        let surface = override_window
            .as_ref()
            .and_then(gtk::prelude::NativeExt::surface)
            .or_else(|| self.window.surface())
            .ok_or_else(|| anyhow::anyhow!("window has no surface"))?;
        let renderer = gtk::gsk::Renderer::for_surface(&surface)
            .ok_or_else(|| anyhow::anyhow!("could not create GTK renderer"))?;
        let viewport = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
        let texture = renderer.render_texture(&node, Some(&viewport));
        texture.save_to_png(path)?;
        Ok(())
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
        content.append(&combo_row("Size", &size));
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
        let background = gtk::DropDown::from_strings(&["White Paper", "Transparent"]);
        content.append(&combo_row("Background", &background));
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
        let update_summary: Rc<dyn Fn()> = Rc::new(glib::clone!(
            #[weak]
            summary,
            #[weak]
            width,
            #[weak]
            height,
            #[weak]
            background,
            move || summary.set_text(&format!(
                "PNG · {} × {} px · {}",
                width.value_as_int(),
                height.value_as_int(),
                if background.selected() == 0 {
                    "White Paper"
                } else {
                    "Transparent"
                }
            ))
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
                    white_background: background.selected() == 0,
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
        self.show_message(&format!("Exporting {}…", path.display()));
        let results = Arc::clone(&self.export_results);
        std::thread::spawn(move || {
            let result = export_svg(&path, &document);
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
        self.show_message(&format!("Exporting PNG {}…", path.display()));
        let results = Arc::clone(&self.export_results);
        std::thread::spawn(move || {
            let result = toniator::export_png(&path, &document, options);
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
        self.update_actions();
        match outcome.result {
            Ok(()) => self.show_message(&format!(
                "Exported {}: {}",
                outcome.kind,
                outcome.path.display()
            )),
            Err(error) => self.show_error(&format!(
                "Could not export {}: {error:#}",
                outcome.path.display()
            )),
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
        let display_title = if dirty {
            format!("{name} •")
        } else {
            name.clone()
        };
        let window_title = if dirty {
            format!("{name} — Unsaved — Toniator")
        } else {
            format!("{name} — Toniator")
        };
        self.title.set_text(&display_title);
        self.window.set_title(Some(&window_title));
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
            toniator::render::decode_source(&request.document.source, request.max_dimension)
        } else {
            render_document_preview(&request.document, request.max_dimension, request.generation)
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

fn build_start_view(has_recovery: bool) -> StartWidgets {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 18);
    page.set_halign(gtk::Align::Center);
    page.set_valign(gtk::Align::Center);
    page.set_margin_start(32);
    page.set_margin_end(32);
    let hero = gtk::Picture::new();
    if let Ok(texture) = gdk::Texture::from_bytes(&glib::Bytes::from_static(START_HERO)) {
        hero.set_paintable(Some(&texture));
    }
    hero.set_content_fit(gtk::ContentFit::Contain);
    hero.set_can_shrink(true);
    hero.set_hexpand(true);
    hero.set_vexpand(true);
    hero.update_property(&[gtk::accessible::Property::Label(
        "Toniator halftone artwork",
    )]);
    let title = gtk::Label::builder()
        .label("Turn artwork into print-ready halftones")
        .css_classes(["title-1"])
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    let subtitle = gtk::Label::builder()
        .label("Start with a useful result, then shape the halftone to fit your work.")
        .css_classes(["dim-label", "title-4"])
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    let open_artwork = gtk::Button::with_label("Open Artwork");
    open_artwork.add_css_class("suggested-action");
    open_artwork.add_css_class("pill");
    let open_document = gtk::Button::with_label("Open Toniator Document");
    open_document.add_css_class("pill");
    let try_example = gtk::Button::with_label("Try Example");
    try_example.add_css_class("flat");
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.set_halign(gtk::Align::Center);
    actions.append(&open_artwork);
    actions.append(&open_document);
    let hero_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    hero_frame.set_size_request(520, 240);
    hero_frame.set_halign(gtk::Align::Center);
    hero_frame.set_valign(gtk::Align::Start);
    hero_frame.set_hexpand(false);
    hero_frame.set_vexpand(false);
    hero_frame.set_overflow(gtk::Overflow::Hidden);
    hero_frame.append(&hero);
    page.append(&hero_frame);
    page.append(&title);
    page.append(&subtitle);
    page.append(&actions);
    page.append(&try_example);
    let recover = has_recovery.then(|| {
        let button = gtk::Button::with_label("Recover Autosaved Work");
        button.add_css_class("flat");
        button.add_css_class("accent");
        page.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        page.append(&button);
        button
    });
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&page)
        .build();
    StartWidgets {
        container: scroll.upcast(),
        open_artwork,
        open_document,
        try_example,
        recover,
    }
}

struct EditorWidgets {
    container: gtk::Widget,
    paned: gtk::Paned,
    canvas: gtk::ScrolledWindow,
    canvas_content: gtk::Overlay,
    fit: gtk::ToggleButton,
    zoom_out: gtk::Button,
    zoom: gtk::Scale,
    zoom_entry: gtk::Entry,
    zoom_in: gtk::Button,
    treatment_modes: gtk::Stack,
    preset_import: gtk::Button,
    preset_save: gtk::Button,
    web_value_mode: gtk::DropDown,
    web_output_ink: gtk::DropDown,
    web_output_ink_row: gtk::Widget,
    web_shared: gtk::CheckButton,
    web_shape: gtk::DropDown,
    web_shape_row: gtk::Widget,
    web_mixed_shape_label: gtk::Label,
    web_mixed_shape_apply: gtk::DropDown,
    web_mixed_shape_apply_row: gtk::Widget,
    web_polygon_sides: gtk::SpinButton,
    web_polygon_sides_label: gtk::Label,
    web_edit_shape: gtk::Button,
    web_target: gtk::DropDown,
    web_target_label: gtk::Label,
    web_visible_label: gtk::Label,
    web_visible: [gtk::CheckButton; 4],
    web_color: gtk::Entry,
    web_color_row: gtk::Widget,
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
    web_opacity_status: gtk::Label,
    web_detail: gtk::Scale,
    web_detail_status: gtk::Label,
    web_mixed: gtk::Label,
    web_geometry_note: gtk::Label,
    curve_value_mode: gtk::DropDown,
    curve_output_ink: gtk::DropDown,
    curve_output_ink_row: gtk::Widget,
    curve_layout: gtk::DropDown,
    curve_profile: gtk::DropDown,
    curve_editor_label: gtk::Label,
    curve_editor: gtk::DrawingArea,
    curve_reset: gtk::Button,
    curve_shared: gtk::CheckButton,
    curve_target: gtk::DropDown,
    curve_target_label: gtk::Label,
    curve_visible_label: gtk::Label,
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

#[allow(clippy::too_many_arguments)]
fn build_editor_view(
    picture: &gtk::Picture,
    source_label: &gtk::Label,
    preview_indicator: &gtk::DrawingArea,
    autosave_status: &gtk::Label,
    detail: &gtk::Scale,
    coverage: &gtk::Scale,
    contrast: &gtk::Scale,
    angle: &gtk::Scale,
    dots: &gtk::ToggleButton,
    squares: &gtk::ToggleButton,
    lines: &gtk::ToggleButton,
    curves: &gtk::ToggleButton,
    legacy: &gtk::ToggleButton,
    compare: &gtk::ToggleButton,
    inspector_width: i32,
    initial_layout_width: i32,
) -> EditorWidgets {
    let layout = gtk::Paned::new(gtk::Orientation::Horizontal);
    layout.set_wide_handle(true);
    layout.set_resize_start_child(true);
    layout.set_resize_end_child(false);
    layout.set_shrink_start_child(true);
    layout.set_shrink_end_child(true);
    let canvas_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    canvas_box.set_hexpand(true);
    let canvas_overlay = gtk::Overlay::new();
    canvas_overlay.set_child(Some(picture));
    let motif_overlay = gtk::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .focusable(true)
        .can_target(true)
        .build();
    motif_overlay.set_visible(false);
    canvas_overlay.add_overlay(&motif_overlay);
    // The stage deliberately allocates the artwork at its requested zoom size,
    // centers slack on both axes, and reports that requested size as its own
    // minimum so explicit zoom overflow remains scrollable.
    let canvas_stage = CenterStage::new(&canvas_overlay);
    canvas_stage.set_hexpand(true);
    canvas_stage.set_vexpand(true);
    let canvas = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .css_classes(["canvas"])
        .child(&canvas_stage)
        .build();
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    controls.set_margin_top(8);
    controls.set_margin_bottom(8);
    controls.set_margin_start(12);
    controls.set_margin_end(12);
    let fit = gtk::ToggleButton::with_label("Fit");
    fit.add_css_class("flat");
    let zoom_out = icon_button("zoom-out-symbolic", "Zoom out 25 percentage points");
    let zoom_in = icon_button("zoom-in-symbolic", "Zoom in 25 percentage points");
    let zoom = gtk::Scale::with_range(gtk::Orientation::Horizontal, ZOOM_MIN, ZOOM_MAX, 1.0);
    disable_pointer_scroll_adjustment(&zoom);
    zoom.set_draw_value(false);
    zoom.set_hexpand(true);
    zoom.set_value(100.0);
    zoom.set_size_request(80, -1);
    zoom.set_tooltip_text(Some("Canvas zoom"));
    fit.set_tooltip_text(Some(
        "Fit complete artwork; fitted percentage may be below 5% for very large artwork",
    ));
    zoom.update_property(&[gtk::accessible::Property::Label("Canvas zoom")]);
    let zoom_entry = gtk::Entry::builder()
        .text("100")
        .width_chars(4)
        .max_width_chars(5)
        .build();
    zoom_entry.set_tooltip_text(Some(
        "Explicit zoom from 5% to 800%; Fit may calculate a smaller value",
    ));
    zoom_entry.set_width_chars(7);
    zoom_entry.update_property(&[gtk::accessible::Property::Label("Zoom percentage")]);
    let rendered_view = gtk::ToggleButton::with_label("Rendered");
    rendered_view.set_group(Some(compare));
    rendered_view.set_active(true);
    let view_switch = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    view_switch.add_css_class("linked");
    view_switch.append(&rendered_view);
    view_switch.append(compare);
    controls.append(&fit);
    controls.append(&zoom_out);
    controls.append(&zoom);
    controls.append(&zoom_entry);
    controls.append(&gtk::Label::new(Some("%")));
    controls.append(&zoom_in);
    controls.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    controls.append(&view_switch);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    controls.append(&spacer);
    controls.append(preview_indicator);
    canvas_box.append(&canvas);
    canvas_box.append(&controls);

    let inspector = gtk::Box::new(gtk::Orientation::Vertical, 14);
    inspector.set_margin_top(18);
    inspector.set_margin_bottom(18);
    inspector.set_margin_start(18);
    inspector.set_margin_end(18);
    inspector.add_css_class("inspector");
    let inspector_title = gtk::Label::builder()
        .label("Halftone")
        .xalign(0.0)
        .css_classes(["title-2"])
        .build();
    let treatment_caption = gtk::Label::builder()
        .label("Pattern")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let treatment_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    treatment_buttons.add_css_class("linked");
    dots.set_hexpand(true);
    squares.set_visible(false);
    lines.set_visible(false);
    curves.set_hexpand(true);
    treatment_buttons.append(dots);
    treatment_buttons.append(squares);
    treatment_buttons.append(lines);
    treatment_buttons.append(curves);
    treatment_buttons.append(legacy);
    for (button, label) in [
        (dots, "Shapes pattern"),
        (squares, "Squares treatment"),
        (lines, "Lines treatment"),
        (curves, "Curves pattern"),
    ] {
        button.update_property(&[gtk::accessible::Property::Label(label)]);
    }
    inspector.append(&inspector_title);
    inspector.append(&treatment_caption);
    inspector.append(&treatment_buttons);
    let preset_actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    preset_actions.add_css_class("linked");
    let preset_import = gtk::Button::with_label("Load Preset…");
    preset_import.set_hexpand(true);
    preset_import.set_tooltip_text(Some("Load a Toniator halftone preset (.tntr)"));
    let preset_save = gtk::Button::with_label("Save Preset…");
    preset_save.set_hexpand(true);
    preset_save.set_tooltip_text(Some("Save this halftone setup without the artwork"));
    preset_actions.append(&preset_import);
    preset_actions.append(&preset_save);
    inspector.append(&preset_actions);

    let native_panel = gtk::Box::new(gtk::Orientation::Vertical, 12);
    native_panel.append(&control_row("Detail", "Coarse — Fine", detail));
    native_panel.append(&control_row(
        "Coverage",
        "How much ink fills the page",
        coverage,
    ));
    native_panel.append(&control_row(
        "Contrast",
        "Separate light and dark areas",
        contrast,
    ));
    native_panel.append(&control_row(
        "Angle",
        "Rotate square and line screens",
        angle,
    ));

    let channel_copy = gtk::Label::builder()
        .label("Automatic CMYK separation")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    channel_copy.set_tooltip_text(Some(
        "Toniator automatically separates artwork into Cyan, Magenta, Yellow, and Black inks",
    ));
    native_panel.append(&channel_copy);

    let web_panel = gtk::Box::new(gtk::Orientation::Vertical, 10);
    let web_value_mode = source_mapping_dropdown();
    web_panel.append(&combo_row("Source Mapping", &web_value_mode));
    web_panel.append(&source_mapping_hint(&web_value_mode));
    let web_output_ink = gtk::DropDown::from_strings(&["Cyan", "Magenta", "Yellow", "Black"]);
    let web_output_ink_row = combo_row("Output Ink", &web_output_ink);
    web_output_ink_row.set_visible(false);
    web_panel.append(&web_output_ink_row);
    let web_shared = gtk::CheckButton::with_label("Use One Shape for All Inks");
    web_shared.set_active(true);
    web_panel.append(&web_shared);
    let web_shape = gtk::DropDown::from_strings(&["Circle", "Regular Polygon", "User Defined"]);
    let web_shape_row = combo_row("Mark", &web_shape);
    web_panel.append(&web_shape_row);
    let web_mixed_shape_label = gtk::Label::builder()
        .label("Mark: Mixed shapes")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    web_mixed_shape_label.set_visible(false);
    web_panel.append(&web_mixed_shape_label);
    let web_mixed_shape_apply = gtk::DropDown::from_strings(&[
        "Choose a mark…",
        "Circle",
        "Regular Polygon",
        "User Defined",
    ]);
    let web_mixed_shape_apply_row = combo_row("Apply Mark to All", &web_mixed_shape_apply);
    web_mixed_shape_apply_row.set_visible(false);
    web_panel.append(&web_mixed_shape_apply_row);
    let web_polygon_sides = gtk::SpinButton::with_range(3.0, 6.0, 1.0);
    disable_pointer_scroll_adjustment(&web_polygon_sides);
    web_polygon_sides.set_value(4.0);
    let web_polygon_sides_label = gtk::Label::builder()
        .label("Polygon Sides (3–6)")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    web_panel.append(&web_polygon_sides_label);
    web_panel.append(&web_polygon_sides);
    let web_edit_shape = gtk::Button::with_label("Edit User-Defined Mark…");
    web_panel.append(&web_edit_shape);
    let web_geometry_note = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label", "caption"])
        .build();
    web_panel.append(&web_geometry_note);
    let web_target =
        gtk::DropDown::from_strings(&["All Inks", "Cyan", "Magenta", "Yellow", "Black"]);
    let (web_target_row, web_target_label) = labeled_combo_row("Edit Ink", &web_target);
    web_panel.append(&web_target_row);
    let visible_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let web_visible = [
        gtk::CheckButton::with_label("C"),
        gtk::CheckButton::with_label("M"),
        gtk::CheckButton::with_label("Y"),
        gtk::CheckButton::with_label("K"),
    ];
    for button in &web_visible {
        button.set_tooltip_text(Some("Toggle this ink in the output"));
        visible_row.append(button);
    }
    let web_visible_label = gtk::Label::builder()
        .label("Visible Inks")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    web_panel.append(&web_visible_label);
    web_panel.append(&visible_row);
    let web_mixed = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label", "caption"])
        .build();
    web_panel.append(&web_mixed);
    let web_color = gtk::Entry::builder()
        .placeholder_text("#RRGGBB")
        .tooltip_text("Hex ink color; valid colors apply automatically")
        .build();
    let (web_color_row, web_color_status) = entry_status_row("Ink Color", "Hex color", &web_color);
    web_panel.append(&web_color_row);
    let web_crosshatch_color = gtk::Entry::builder()
        .placeholder_text("#111111")
        .tooltip_text("One monochrome color used by every crosshatch layer")
        .build();
    let web_crosshatch_color_row =
        entry_status_row("Crosshatch Color", "Hex color", &web_crosshatch_color).0;
    web_crosshatch_color_row.set_visible(false);
    web_panel.append(&web_crosshatch_color_row);
    let web_coverage = control_scale(0.0, 5.0, 0.05);
    let web_angle = control_scale(-360.0, 360.0, 1.0);
    let web_mark_angle = control_scale(-180.0, 180.0, 1.0);
    let web_width_scale = control_scale(0.1, 4.0, 0.05);
    let web_height_scale = control_scale(0.1, 4.0, 0.05);
    let (web_coverage_row, web_coverage_status) =
        control_status_row("Coverage", "Mark size", &web_coverage);
    web_panel.append(&web_coverage_row);
    let (web_angle_row, web_angle_status) =
        control_status_row("Grid Angle", "Rotate sampling grid", &web_angle);
    web_panel.append(&web_angle_row);
    let (web_mark_angle_row, web_mark_angle_status) = control_status_row(
        "Mark Angle",
        "Rotate marks within the grid",
        &web_mark_angle,
    );
    web_panel.append(&web_mark_angle_row);
    let (web_width_scale_row, web_width_scale_status) =
        control_status_row("Width Scale", "Horizontal mark scale", &web_width_scale);
    web_panel.append(&web_width_scale_row);
    let (web_height_scale_row, web_height_scale_status) =
        control_status_row("Height Scale", "Vertical mark scale", &web_height_scale);
    web_panel.append(&web_height_scale_row);
    let advanced = gtk::Expander::builder()
        .label("Advanced")
        .expanded(false)
        .build();
    let advanced_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let web_threshold = control_scale(0.0, 1.0, 0.01);
    let web_opacity = control_scale(0.0, 1.0, 0.01);
    let web_detail = control_scale(0.1, 8.0, 0.1);
    let (web_threshold_row, web_threshold_status) =
        control_status_row("Remove Faint Marks", "Hide light marks", &web_threshold);
    advanced_box.append(&web_threshold_row);
    let (web_opacity_row, web_opacity_status) =
        control_status_row("Ink Opacity", "Transparent — Solid", &web_opacity);
    advanced_box.append(&web_opacity_row);
    let (web_detail_row, web_detail_status) =
        control_status_row("Detail", "Sample density", &web_detail);
    advanced_box.append(&web_detail_row);
    advanced.set_child(Some(&advanced_box));
    web_panel.append(&advanced);

    let curve_panel = gtk::Box::new(gtk::Orientation::Vertical, 10);
    let curve_value_mode = source_mapping_dropdown();
    curve_panel.append(&combo_row("Source Mapping", &curve_value_mode));
    curve_panel.append(&source_mapping_hint(&curve_value_mode));
    let curve_output_ink = gtk::DropDown::from_strings(&["Cyan", "Magenta", "Yellow", "Black"]);
    let curve_output_ink_row = combo_row("Output Ink", &curve_output_ink);
    curve_output_ink_row.set_visible(false);
    curve_panel.append(&curve_output_ink_row);
    let curve_layout = gtk::DropDown::from_strings(&["Across Page", "Repeated Motif"]);
    curve_panel.append(&combo_row("Layout", &curve_layout));
    let curve_weight = control_scale(1.0, 200.0, 1.0);
    curve_panel.append(&control_row(
        "Weight (All Inks)",
        "Global curve thickness",
        &curve_weight,
    ));
    let curve_spacing = control_scale(8.0, 220.0, 1.0);
    curve_panel.append(&control_row(
        "Spacing (All Inks)",
        "Global distance between curves",
        &curve_spacing,
    ));
    let curve_profile = gtk::DropDown::from_strings(&[
        "Straight",
        "Soft Wave",
        "Deep Wave",
        "Custom",
        "Mixed — Select One Ink",
    ]);
    curve_panel.append(&combo_row("Curve Shape", &curve_profile));
    let curve_editor_label = gtk::Label::builder()
        .label("All Inks Curve")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    curve_panel.append(&curve_editor_label);
    let curve_editor = gtk::DrawingArea::builder()
        .content_width(300)
        .content_height(220)
        .hexpand(true)
        .focusable(true)
        .tooltip_text("Drag curve anchors and handles; double-click a segment to add a point")
        .css_classes(["curve-editor"])
        .build();
    curve_panel.append(&curve_editor);
    curve_panel.append(
        &gtk::Label::builder()
            .label("Drag white points to shape the curve; blue points adjust bends. Double-click the line to add a point; Delete removes the selected point.")
            .wrap(true)
            .xalign(0.0)
            .css_classes(["dim-label", "caption"])
            .build(),
    );
    let curve_actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let curve_reset = gtk::Button::with_label("Reset to Soft Wave");
    curve_reset.add_css_class("flat");
    let curve_shared = gtk::CheckButton::with_label("Use One Curve for All Inks");
    curve_shared.set_active(true);
    curve_actions.append(&curve_reset);
    curve_panel.append(&curve_actions);
    curve_panel.append(&curve_shared);
    let curve_target =
        gtk::DropDown::from_strings(&["All Inks", "Cyan", "Magenta", "Yellow", "Black"]);
    let (curve_target_row, curve_target_label) = labeled_combo_row("Edit Ink", &curve_target);
    curve_panel.append(&curve_target_row);
    let motif_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    motif_box.append(
        &gtk::Label::builder()
            .label("Pattern")
            .xalign(0.0)
            .css_classes(["heading"])
            .build(),
    );
    let motif_coverage = gtk::DropDown::from_strings(&["Fill Canvas Automatically", "Custom Grid"]);
    motif_box.append(&combo_row("Canvas Coverage", &motif_coverage));
    let motif_size = control_scale(4.0, 200.0, 1.0);
    motif_box.append(&control_status_row("Motif Size", "Repeated curve width", &motif_size).0);
    let motif_columns = control_scale(1.0, 40.0, 1.0);
    motif_columns.set_digits(0);
    motif_box.append(&control_status_row("Columns", "Copies across", &motif_columns).0);
    let motif_rows = control_scale(1.0, 80.0, 1.0);
    motif_rows.set_digits(0);
    motif_box.append(&control_status_row("Rows", "Layered rows", &motif_rows).0);
    let motif_row_spacing = control_scale(1.0, 160.0, 1.0);
    motif_box
        .append(&control_status_row("Row Spacing", "Distance between rows", &motif_row_spacing).0);
    let motif_stagger = control_scale(-200.0, 200.0, 1.0);
    motif_box.append(&control_status_row("Stagger Rows", "Alternate row shift", &motif_stagger).0);
    let motif_alternate = gtk::DropDown::from_strings(&["None", "Mirror", "Half Turn"]);
    motif_box.append(&combo_row("Alternate Copies", &motif_alternate));
    let motif_arrange = gtk::CheckButton::with_label("Arrange on Canvas");
    motif_arrange.set_tooltip_text(Some(
        "Drag the center, rotation, and spacing handles on the artwork",
    ));
    motif_box.append(&motif_arrange);
    let motif_mixed = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label", "caption"])
        .build();
    motif_box.append(&motif_mixed);
    motif_box.append(
        &gtk::Label::builder()
            .label("Drag the center to move, R to rotate, and S to separate rows. Esc cancels the drag.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label", "caption"])
            .build(),
    );
    let motif_controls: gtk::Widget = motif_box.upcast();
    motif_controls.set_visible(false);
    curve_panel.append(&motif_controls);
    let curve_visible_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let curve_visible = [
        gtk::CheckButton::with_label("C"),
        gtk::CheckButton::with_label("M"),
        gtk::CheckButton::with_label("Y"),
        gtk::CheckButton::with_label("K"),
    ];
    for button in &curve_visible {
        button.set_tooltip_text(Some("Toggle this ink in the output"));
        curve_visible_row.append(button);
    }
    let curve_visible_label = gtk::Label::builder()
        .label("Visible Inks")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    curve_panel.append(&curve_visible_label);
    curve_panel.append(&curve_visible_row);
    let curve_mixed = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label", "caption"])
        .build();
    curve_panel.append(&curve_mixed);
    let curve_color = gtk::Entry::builder()
        .placeholder_text("#RRGGBB")
        .tooltip_text("Hex ink color; valid colors apply automatically")
        .build();
    let (curve_color_row, curve_color_status) =
        entry_status_row("Ink Color", "Hex color", &curve_color);
    curve_panel.append(&curve_color_row);
    let curve_crosshatch_color = gtk::Entry::builder()
        .placeholder_text("#111111")
        .tooltip_text("One monochrome color used by every crosshatch layer")
        .build();
    let curve_crosshatch_color_row =
        entry_status_row("Crosshatch Color", "Hex color", &curve_crosshatch_color).0;
    curve_crosshatch_color_row.set_visible(false);
    curve_panel.append(&curve_crosshatch_color_row);
    let curve_coverage = control_scale(0.0, 5.0, 0.05);
    let (curve_coverage_row, curve_coverage_status) =
        control_status_row("Coverage", "Curve scale", &curve_coverage);
    curve_panel.append(&curve_coverage_row);
    let curve_angle = control_scale(-360.0, 360.0, 1.0);
    let (curve_angle_row, curve_angle_status) =
        control_status_row("Angle", "Rotate ink screen", &curve_angle);
    curve_panel.append(&curve_angle_row);
    let curve_position_x = control_scale(-1000.0, 1000.0, 1.0);
    let (curve_position_x_row, curve_position_x_status) =
        control_status_row("Position X", "Move across", &curve_position_x);
    curve_panel.append(&curve_position_x_row);
    let curve_position_y = control_scale(-1000.0, 1000.0, 1.0);
    let (curve_position_y_row, curve_position_y_status) =
        control_status_row("Position Y", "Move vertically", &curve_position_y);
    curve_panel.append(&curve_position_y_row);
    let curve_opacity = control_scale(0.0, 1.0, 0.01);
    let (curve_opacity_row, curve_opacity_status) =
        control_status_row("Ink Opacity", "Transparent — Solid", &curve_opacity);
    curve_panel.append(&curve_opacity_row);
    let curve_advanced = gtk::Expander::builder()
        .label("Advanced")
        .expanded(false)
        .build();
    let curve_advanced_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let curve_threshold = control_scale(0.0, 1.0, 0.01);
    let (curve_threshold_row, curve_threshold_status) =
        control_status_row("Remove Faint Curves", "Hide light marks", &curve_threshold);
    curve_advanced_box.append(&curve_threshold_row);
    let curve_detail = control_scale(0.1, 8.0, 0.1);
    let (curve_detail_row, curve_detail_status) =
        control_status_row("Detail", "Sample density", &curve_detail);
    curve_advanced_box.append(&curve_detail_row);
    let curve_close_ends = gtk::CheckButton::with_label("Close Ends");
    let curve_smooth_join = gtk::CheckButton::with_label("Smooth Join");
    curve_advanced_box.append(&curve_close_ends);
    curve_advanced_box.append(&curve_smooth_join);
    curve_advanced.set_child(Some(&curve_advanced_box));
    curve_panel.append(&curve_advanced);
    let treatment_modes = gtk::Stack::new();
    treatment_modes.add_named(&native_panel, Some("native"));
    treatment_modes.add_named(&web_panel, Some("web"));
    treatment_modes.add_named(&curve_panel, Some("curve"));
    treatment_modes.set_visible_child_name("native");
    inspector.append(&treatment_modes);

    let document = gtk::Expander::builder()
        .label("Document")
        .expanded(false)
        .build();
    let document_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    document_box.set_margin_top(10);
    document_box.append(source_label);
    document_box.append(autosave_status);
    document.set_child(Some(&document_box));
    inspector.append(&document);

    let inspector_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&inspector)
        .build();
    inspector_scroll.add_css_class("inspector-pane");
    inspector_scroll.set_size_request(0, -1);
    layout.set_start_child(Some(&canvas_box));
    layout.set_end_child(Some(&inspector_scroll));
    layout.set_position((initial_layout_width - inspector_width).max(CANVAS_MIN_WIDTH));
    EditorWidgets {
        container: layout.clone().upcast(),
        paned: layout,
        canvas,
        canvas_content: canvas_overlay,
        fit,
        zoom_out,
        zoom,
        zoom_entry,
        zoom_in,
        treatment_modes,
        preset_import,
        preset_save,
        web_value_mode,
        web_output_ink,
        web_output_ink_row,
        web_shared,
        web_shape,
        web_shape_row,
        web_mixed_shape_label,
        web_mixed_shape_apply,
        web_mixed_shape_apply_row,
        web_polygon_sides,
        web_polygon_sides_label,
        web_edit_shape,
        web_target,
        web_target_label,
        web_visible_label,
        web_visible,
        web_color,
        web_color_row,
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
        web_opacity_status,
        web_detail,
        web_detail_status,
        web_mixed,
        web_geometry_note,
        curve_value_mode,
        curve_output_ink,
        curve_output_ink_row,
        curve_layout,
        curve_profile,
        curve_editor_label,
        curve_editor,
        curve_reset,
        curve_shared,
        curve_target,
        curve_target_label,
        curve_visible_label,
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

fn combo_row(title: &str, combo: &gtk::DropDown) -> gtk::Widget {
    labeled_combo_row(title, combo).0
}

fn labeled_combo_row(title: &str, combo: &gtk::DropDown) -> (gtk::Widget, gtk::Label) {
    combo.set_hexpand(true);
    combo.set_size_request(0, -1);
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let label = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    row.append(&label);
    row.append(combo);
    (row.upcast(), label)
}

#[derive(Clone, Copy)]
struct SourceMappingOption {
    name: &'static str,
    description: &'static str,
    mode: ValueMode,
    source_svg: &'static [u8],
    result_svg: &'static [u8],
    source_description: &'static str,
    result_description: &'static str,
}

const SOURCE_MAPPING_OPTIONS: [SourceMappingOption; 4] = [
    SourceMappingOption {
        name: "Color → CMYK Inks",
        description: "Separate source color into cyan, magenta, yellow, and black inks.",
        mode: ValueMode::Cmyk,
        source_svg: COLOR_SOURCE_SVG,
        result_svg: COLOR_TO_CMYK_SVG,
        source_description: "Color source artwork",
        result_description: "Result separated into cyan, magenta, yellow, and black inks",
    },
    SourceMappingOption {
        name: "Value → One Ink",
        description: "Map source tonal value to one selected ink.",
        mode: ValueMode::SingleChannel,
        source_svg: VALUE_SOURCE_SVG,
        result_svg: VALUE_TO_ONE_INK_SVG,
        source_description: "Source artwork represented by tonal value",
        result_description: "Tonal value mapped to one selected ink",
    },
    SourceMappingOption {
        name: "Value → All Inks",
        description: "Apply the same source tonal value to every enabled ink.",
        mode: ValueMode::Luminance,
        source_svg: VALUE_SOURCE_SVG,
        result_svg: VALUE_TO_CMYK_SVG,
        source_description: "Source artwork represented by tonal value",
        result_description: "Tonal value mapped to all enabled inks",
    },
    SourceMappingOption {
        name: "Value → Crosshatch",
        description: "Build tonal value with monochrome K +45°, C -45°, M horizontal, and Y vertical hatch layers.",
        mode: ValueMode::CrosshatchLuminance,
        source_svg: VALUE_SOURCE_SVG,
        result_svg: VALUE_TO_CROSSHATCH_SVG,
        source_description: "Source artwork represented by tonal value",
        result_description: "Tonal value mapped to four directional crosshatch layers",
    },
];

const SOURCE_MAPPING_ARTWORK_SIZE: i32 = 92;

fn source_mapping_from_index(index: u32) -> Option<ValueMode> {
    SOURCE_MAPPING_OPTIONS
        .get(index as usize)
        .map(|option| option.mode)
}

fn ink_for_visible_slot(index: usize, crosshatch: bool) -> Ink {
    if crosshatch {
        [Ink::Black, Ink::Cyan, Ink::Magenta, Ink::Yellow][index]
    } else {
        Ink::ALL[index]
    }
}

fn source_mapping_dropdown() -> gtk::DropDown {
    let names = SOURCE_MAPPING_OPTIONS.map(|option| option.name);
    gtk::DropDown::from_strings(&names)
}

fn sync_layer_terminology(
    dropdown: &gtk::DropDown,
    target_label: &gtk::Label,
    visible_label: &gtk::Label,
    crosshatch: bool,
) {
    let wanted = if crosshatch { "Edit Layer" } else { "Edit Ink" };
    if target_label.text() == wanted {
        return;
    }
    let selected = dropdown.selected();
    target_label.set_text(wanted);
    visible_label.set_text(if crosshatch {
        "Crosshatch Layers"
    } else {
        "Visible Inks"
    });
    let values = if crosshatch {
        [
            "All Layers",
            "Layer 1 (K)",
            "Layer 2 (C)",
            "Layer 3 (M)",
            "Layer 4 (Y)",
        ]
    } else {
        ["All Inks", "Cyan", "Magenta", "Yellow", "Black"]
    };
    dropdown.set_model(Some(&gtk::StringList::new(&values)));
    dropdown.set_selected(selected);
}

fn render_embedded_svg_texture(bytes: &'static [u8]) -> Result<gdk::MemoryTexture, String> {
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default())
        .map_err(|error| format!("could not parse embedded Source Mapping SVG: {error}"))?;
    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 {
        return Err("embedded Source Mapping SVG has no usable size".into());
    }
    let scale = SOURCE_MAPPING_ARTWORK_SIZE as f32 / size.width().max(size.height());
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| "could not allocate embedded Source Mapping texture".to_owned())?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let stride = width as usize * 4;
    let bytes = glib::Bytes::from_owned(pixmap.take());
    Ok(gdk::MemoryTexture::new(
        width as i32,
        height as i32,
        gdk::MemoryFormat::R8g8b8a8Premultiplied,
        &bytes,
        stride,
    ))
}

fn source_mapping_picture(bytes: &'static [u8], description: &'static str) -> gtk::Picture {
    let texture = render_embedded_svg_texture(bytes)
        .expect("embedded Source Mapping SVGs are validated application assets");
    let picture = gtk::Picture::builder()
        .paintable(&texture)
        .content_fit(gtk::ContentFit::Contain)
        .can_shrink(false)
        .hexpand(true)
        .width_request(SOURCE_MAPPING_ARTWORK_SIZE)
        .height_request(SOURCE_MAPPING_ARTWORK_SIZE)
        .accessible_role(gtk::AccessibleRole::Img)
        .build();
    picture.update_property(&[gtk::accessible::Property::Label(description)]);
    picture
}

fn source_mapping_hint(dropdown: &gtk::DropDown) -> gtk::Widget {
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(120)
        .build();
    for (index, option) in SOURCE_MAPPING_OPTIONS.into_iter().enumerate() {
        let group = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let artwork = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        artwork.set_homogeneous(false);
        artwork.append(&source_mapping_picture(
            option.source_svg,
            option.source_description,
        ));
        artwork.append(
            &gtk::Label::builder()
                .label("→")
                .accessible_role(gtk::AccessibleRole::Presentation)
                .css_classes(["title-2", "dim-label"])
                .build(),
        );
        artwork.append(&source_mapping_picture(
            option.result_svg,
            option.result_description,
        ));
        group.append(&artwork);
        group.append(
            &gtk::Label::builder()
                .label(option.description)
                .xalign(0.0)
                .wrap(true)
                .hexpand(true)
                .css_classes(["dim-label", "caption"])
                .build(),
        );
        stack.add_named(&group, Some(&index.to_string()));
    }
    stack.set_visible_child_name("0");
    dropdown.connect_selected_notify(glib::clone!(
        #[weak]
        stack,
        move |dropdown| {
            if source_mapping_from_index(dropdown.selected()).is_some() {
                stack.set_visible_child_name(&dropdown.selected().to_string());
            }
        }
    ));
    stack.upcast()
}

fn entry_status_row(title: &str, status: &str, entry: &gtk::Entry) -> (gtk::Widget, gtk::Label) {
    entry.set_hexpand(true);
    entry.set_size_request(0, -1);
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let labels = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let status = gtk::Label::builder()
        .label(status)
        .xalign(1.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["dim-label", "caption"])
        .build();
    labels.append(&title);
    labels.append(&status);
    row.append(&labels);
    row.append(entry);
    (row.upcast(), status)
}

fn control_status_row(title: &str, status: &str, scale: &gtk::Scale) -> (gtk::Widget, gtk::Label) {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let labels = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let status = gtk::Label::builder()
        .label(status)
        .xalign(1.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["dim-label", "caption"])
        .build();
    labels.append(&title);
    labels.append(&status);
    row.append(&labels);
    row.append(&precision_scale_control(scale));
    scale.update_relation(&[gtk::accessible::Relation::LabelledBy(&[title.upcast_ref()])]);
    (row.upcast(), status)
}

fn sync_web_scale(
    scale: &gtk::Scale,
    status: &gtk::Label,
    value: f64,
    mixed: bool,
    normal_status: &str,
) {
    scale.set_draw_value(false);
    if let Some(entry) = precision_entry(scale) {
        entry.set_visible(!mixed);
    }
    if mixed {
        scale.add_css_class("mixed-scale");
        scale.update_property(&[gtk::accessible::Property::Description(
            "Mixed values; changing this control applies one value to all selected inks",
        )]);
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

fn control_row(title: &str, subtitle: &str, scale: &gtk::Scale) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let labels = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let subtitle = gtk::Label::builder()
        .label(subtitle)
        .xalign(1.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["dim-label", "caption"])
        .build();
    labels.append(&title);
    labels.append(&subtitle);
    row.append(&labels);
    row.append(&precision_scale_control(scale));
    scale.update_relation(&[gtk::accessible::Relation::LabelledBy(&[title.upcast_ref()])]);
    row.upcast()
}

fn control_scale(minimum: f64, maximum: f64, step: f64) -> gtk::Scale {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, minimum, maximum, step);
    disable_pointer_scroll_adjustment(&scale);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    scale
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

fn precision_scale_control(scale: &gtk::Scale) -> gtk::Widget {
    let control = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let adjustment = scale.adjustment();
    let step = adjustment.step_increment().abs();
    let digits = if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else {
        2
    };
    let entry = gtk::SpinButton::new(Some(&adjustment), step.max(0.01), digits);
    disable_pointer_scroll_adjustment(&entry);
    entry.set_width_chars(5);
    entry.set_max_width_chars(7);
    entry.set_numeric(true);
    entry.set_tooltip_text(Some("Enter an exact value"));
    entry.update_property(&[gtk::accessible::Property::Label("Exact value")]);
    control.append(scale);
    control.append(&entry);
    control.upcast()
}

fn precision_entry(scale: &gtk::Scale) -> Option<gtk::SpinButton> {
    scale
        .parent()
        .and_then(|parent| parent.last_child())
        .and_then(|child| child.downcast::<gtk::SpinButton>().ok())
}

fn action_button(label: &str, tooltip: &str) -> gtk::Button {
    gtk::Button::builder()
        .label(label)
        .tooltip_text(tooltip)
        .build()
}

fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .build()
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
    fn export_inhibits_close_until_completion() {
        assert_eq!(close_policy(true, false, false), ClosePolicy::InhibitExport);
        assert_eq!(close_policy(true, true, false), ClosePolicy::InhibitExport);
        assert_eq!(close_policy(false, false, false), ClosePolicy::Proceed);
        assert_eq!(close_policy(false, false, true), ClosePolicy::CheckDirty);
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
            ["Comic Book", "Skinny Curve", "Chunky Fingerprints"]
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
        paned.set_resize_start_child(true);
        paned.set_resize_end_child(false);
        paned.set_start_child(Some(&canvas));
        paned.set_end_child(Some(&inspector));
        paned.set_position(800);
        let window = gtk::Window::builder()
            .default_width(1200)
            .default_height(600)
            .child(&paned)
            .build();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ui-state.json");
        let controller = InspectorPaneController::new(&paned, 400, Some(path.clone()));
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
        paned.set_position(paned.position() - 110);
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
        eprintln!(
            "paned stability: 25 repeated settings/status/preview-install mutations kept inspector={initial}px; deliberate drag persisted desired={deliberate}px; temporary 760px window clamp preserved desired and 1200px restore returned inspector={deliberate}px"
        );
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
        assert_eq!(activity.accessible_label(), "Updating rendered preview");
        activity.failed(5);
        assert_eq!(activity.resting_phase(), 0.0);
        assert_eq!(activity.accessible_label(), "Source preview");
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

    fn verify_realized_preview_indicator() {
        let indicator = PreviewIndicator::new(None);
        assert_eq!(indicator.area.width_request(), 40);
        assert_eq!(indicator.area.height_request(), 28);
        assert_eq!(indicator.area.accessible_role(), gtk::AccessibleRole::Img);
        assert_eq!(
            indicator.area.tooltip_text().as_deref(),
            Some("Rendered preview")
        );
        indicator.request(1, PreviewView::Rendered);
        let epoch = indicator
            .epoch
            .get()
            .expect("render request starts animation");
        assert!(indicator.tick.borrow().is_some());
        assert_eq!(
            indicator.area.tooltip_text().as_deref(),
            Some("Updating rendered preview")
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
            Some("Rendered preview")
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
        assert!(editor.set_render_variant(RenderVariant::WebCurveV1 {
            settings: Box::new(moved),
        }));
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
        assert!(editor.set_render_variant(RenderVariant::WebCurveV1 {
            settings: Box::new(cancelled),
        }));
        assert!(editor.cancel_edit());
        assert!(matches!(
            &editor.document().render,
            RenderVariant::WebCurveV1 { settings } if settings.shared_path == moved_path
        ));
    }

    #[test]
    fn realized_numeric_controls_leave_continuous_scroll_to_parent() {
        gtk::init().unwrap();
        use gtk::gdk::prelude::PaintableExt;

        let mapping_picture = source_mapping_picture(
            SOURCE_MAPPING_OPTIONS[0].source_svg,
            SOURCE_MAPPING_OPTIONS[0].source_description,
        );
        assert!(!mapping_picture.can_shrink());
        assert_eq!(mapping_picture.width_request(), SOURCE_MAPPING_ARTWORK_SIZE);
        assert_eq!(
            mapping_picture.height_request(),
            SOURCE_MAPPING_ARTWORK_SIZE
        );
        assert_eq!(mapping_picture.accessible_role(), gtk::AccessibleRole::Img);

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
        verify_realized_paned_owns_inspector_width();
        verify_realized_shape_double_clicks();
        verify_realized_preview_indicator();

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
        let bytes = toniator::preset::treatment_preset_bytes(
            "Base Test",
            &RenderVariant::WebShapeV1 {
                settings: Box::new(shape.clone()),
            },
        )
        .unwrap();
        let parsed = toniator::preset::parse_treatment(&bytes, (900, 600)).unwrap();
        shape.output_height = 600;
        assert_eq!(
            parsed.render,
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
    fn source_mapping_names_hints_and_enum_indices_are_one_table_order() {
        let expected = [
            ValueMode::Cmyk,
            ValueMode::SingleChannel,
            ValueMode::Luminance,
            ValueMode::CrosshatchLuminance,
        ];
        for index in 0..4 {
            assert_eq!(
                source_mapping_from_index(index),
                Some(expected[index as usize])
            );
            assert!(
                !SOURCE_MAPPING_OPTIONS[index as usize]
                    .description
                    .is_empty()
            );
        }
        assert_eq!(source_mapping_from_index(4), None);
        assert_eq!(
            SOURCE_MAPPING_OPTIONS.map(|option| option.name),
            [
                "Color → CMYK Inks",
                "Value → One Ink",
                "Value → All Inks",
                "Value → Crosshatch",
            ]
        );
        let user_facing = SOURCE_MAPPING_OPTIONS
            .iter()
            .flat_map(|option| [option.name, option.description])
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!user_facing.contains("Darkness"));
        assert!(!user_facing.contains("Lightness"));
        assert!(SOURCE_MAPPING_OPTIONS[3].description.contains("K +45°"));
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
        let description = SOURCE_MAPPING_OPTIONS[3].description;
        for expected in ["K +45°", "C -45°", "M horizontal", "Y vertical"] {
            assert!(description.contains(expected));
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
        let before = settings.clone();
        document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(settings.clone()),
        };
        let mut editor = DocumentEditor::new(document);
        reset_crosshatch_curve_path(&mut settings, &[Ink::Black]);
        assert!(editor.set_render_variant(RenderVariant::WebCurveV1 {
            settings: Box::new(settings.clone()),
        }));
        assert_eq!(settings.channels.k.path, CurvePath::straight());
        assert!(!settings.channels.k.close_ends && !settings.channels.k.smooth_join);
        assert_eq!(settings.channels.c.path, CurvePath::deep_wave());
        assert!(editor.undo());
        assert!(matches!(
            &editor.document().render,
            RenderVariant::WebCurveV1 { settings } if **settings == before
        ));
        assert!(editor.redo());
        assert!(matches!(
            &editor.document().render,
            RenderVariant::WebCurveV1 { settings: redone } if **redone == settings
        ));
    }

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
        for option in &SOURCE_MAPPING_OPTIONS[1..] {
            assert!(std::ptr::eq(option.source_svg, VALUE_SOURCE_SVG));
        }
        for (option, expected) in SOURCE_MAPPING_OPTIONS.iter().zip([
            COLOR_TO_CMYK_SVG,
            VALUE_TO_ONE_INK_SVG,
            VALUE_TO_CMYK_SVG,
            VALUE_TO_CROSSHATCH_SVG,
        ]) {
            assert!(!option.source_description.is_empty());
            assert!(!option.result_description.is_empty());
            assert!(std::ptr::eq(option.result_svg, expected));
            for bytes in [option.source_svg, option.result_svg] {
                let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).unwrap();
                assert!(tree.size().width() > 0.0 && tree.size().height() > 0.0);
                let texture = render_embedded_svg_texture(bytes).unwrap();
                assert!(texture.width() > 0 && texture.height() > 0);
                assert_eq!(
                    texture.width().max(texture.height()),
                    SOURCE_MAPPING_ARTWORK_SIZE
                );
            }
        }
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
