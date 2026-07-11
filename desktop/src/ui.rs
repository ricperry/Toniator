use crate::CliOptions;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use image::RgbaImage;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use toniator::model::{SettingKey, SourceArtwork};
use toniator::persistence::{clear_recovery_if_matches, recovery_path};
use toniator::{
    AlternateTileTransform, CurveLayout, CurvePath, CurvePoint, Document, DocumentEditor, Ink,
    MotifCoverage, RenderGate, RenderVariant, Settings, Treatment, ValueMode, WebCurveChannel,
    WebCurveSettings, WebShape, export_svg, load_document, render_document_preview,
    save_document_atomic,
};

const EXAMPLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="960" height="680" viewBox="0 0 960 680">
<defs><linearGradient id="warm" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ffcf33"/><stop offset=".48" stop-color="#ec008c"/><stop offset="1" stop-color="#0047ff"/></linearGradient><radialGradient id="cool" cx="42%" cy="40%" r="70%"><stop offset="0" stop-color="#fff"/><stop offset=".45" stop-color="#00aeef"/><stop offset="1" stop-color="#08111f"/></radialGradient></defs>
<rect width="100%" height="100%" fill="url(#warm)"/><circle cx="330" cy="310" r="235" fill="url(#cool)" opacity=".92"/><rect x="565" y="115" width="260" height="365" rx="44" fill="#101114" opacity=".78"/><path d="M90 555 C225 420 350 665 510 535 S745 440 870 585" fill="none" stroke="#fff" stroke-width="58" stroke-linecap="round" opacity=".82"/><text x="620" y="345" font-family="sans-serif" font-size="122" font-weight="800" fill="#fff">T</text></svg>"##;

struct AppState {
    editor: Option<DocumentEditor>,
    path: Option<PathBuf>,
    syncing_controls: bool,
    preview_size: Option<(u32, u32)>,
    compare_source: bool,
    zoom_mode: ZoomMode,
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
    Fit,
    Scale(f64),
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
}

struct RenderOutcome {
    generation: u64,
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

pub struct AppUi {
    window: adw::ApplicationWindow,
    stack: gtk::Stack,
    toast_overlay: adw::ToastOverlay,
    title: gtk::Label,
    picture: gtk::Picture,
    source_label: gtk::Label,
    render_status: gtk::Label,
    autosave_status: gtk::Label,
    detail: gtk::Scale,
    coverage: gtk::Scale,
    contrast: gtk::Scale,
    angle: gtk::Scale,
    dots: gtk::ToggleButton,
    squares: gtk::ToggleButton,
    lines: gtk::ToggleButton,
    curves: gtk::ToggleButton,
    treatment_modes: gtk::Stack,
    preset_import: gtk::Button,
    preset_save: gtk::Button,
    web_value_mode: gtk::DropDown,
    web_output_ink: gtk::DropDown,
    web_output_ink_row: gtk::Widget,
    web_shape: gtk::DropDown,
    web_shape_row: gtk::Widget,
    web_target: gtk::DropDown,
    web_visible: [gtk::CheckButton; 4],
    web_color: gtk::Entry,
    web_color_status: gtk::Label,
    web_coverage: gtk::Scale,
    web_coverage_status: gtk::Label,
    web_angle: gtk::Scale,
    web_angle_status: gtk::Label,
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
    curve_visible: [gtk::CheckButton; 4],
    curve_color: gtk::Entry,
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
    actual_size: gtk::ToggleButton,
    zoom: gtk::Scale,
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
    preview_generation: Cell<u64>,
    preset_pending: Cell<bool>,
    compare_source_artifact: bool,
    arrange_motif_artifact: bool,
}

type TransitionContinuation = Rc<dyn Fn(&Rc<AppUi>)>;

impl AppUi {
    pub fn new(application: &adw::Application, options: CliOptions) -> Rc<Self> {
        install_styles();

        let artifact_mode = options.artifact_mode();
        let load_example = options.loads_example();
        let recovery_enabled = !artifact_mode;
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
        let open = action_button("Open", "Open artwork or document");
        let save = action_button("Save", "Save Toniator document");
        let undo = icon_button("edit-undo-symbolic", "Undo");
        let redo = icon_button("edit-redo-symbolic", "Redo");
        let export = action_button("Export…", "Export editable SVG or PNG image");
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
            .default_width(1280)
            .default_height(820)
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
        let render_status = gtk::Label::builder()
            .label("Ready")
            .xalign(0.0)
            .css_classes(["dim-label", "caption"])
            .build();
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
        detail.set_format_value_func(|_, value| format!("{value:.0}"));
        coverage.set_format_value_func(|_, value| format!("{value:.0}%"));
        contrast.set_format_value_func(|_, value| format!("{value:.0}%"));
        angle.set_format_value_func(|_, value| format!("{value:.0}°"));
        let dots = gtk::ToggleButton::with_label("Dots");
        let squares = gtk::ToggleButton::with_label("Squares");
        let lines = gtk::ToggleButton::with_label("Lines");
        let curves = gtk::ToggleButton::with_label("Curves");
        squares.set_group(Some(&dots));
        lines.set_group(Some(&dots));
        curves.set_group(Some(&dots));
        dots.set_active(true);
        let compare = gtk::ToggleButton::with_label("Compare Source");

        let start = build_start_view(recovery_enabled && recovery_path().exists());
        stack.add_named(&start.container, Some("start"));
        let editor_view = build_editor_view(
            &picture,
            &source_label,
            &render_status,
            &autosave_status,
            &detail,
            &coverage,
            &contrast,
            &angle,
            &dots,
            &squares,
            &lines,
            &curves,
            &compare,
        );
        stack.add_named(&editor_view.container, Some("editor"));
        stack.set_visible_child_name("start");
        let fit = editor_view.fit.clone();
        let actual_size = editor_view.actual_size.clone();
        let zoom = editor_view.zoom.clone();

        let ui = Rc::new(Self {
            window,
            stack,
            toast_overlay,
            title,
            picture,
            source_label,
            render_status,
            autosave_status,
            detail,
            coverage,
            contrast,
            angle,
            dots,
            squares,
            lines,
            curves,
            treatment_modes: editor_view.treatment_modes.clone(),
            preset_import: editor_view.preset_import.clone(),
            preset_save: editor_view.preset_save.clone(),
            web_value_mode: editor_view.web_value_mode.clone(),
            web_output_ink: editor_view.web_output_ink.clone(),
            web_output_ink_row: editor_view.web_output_ink_row.clone(),
            web_shape: editor_view.web_shape.clone(),
            web_shape_row: editor_view.web_shape_row.clone(),
            web_target: editor_view.web_target.clone(),
            web_visible: editor_view.web_visible.clone(),
            web_color: editor_view.web_color.clone(),
            web_color_status: editor_view.web_color_status.clone(),
            web_coverage: editor_view.web_coverage.clone(),
            web_coverage_status: editor_view.web_coverage_status.clone(),
            web_angle: editor_view.web_angle.clone(),
            web_angle_status: editor_view.web_angle_status.clone(),
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
            curve_visible: editor_view.curve_visible.clone(),
            curve_color: editor_view.curve_color.clone(),
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
            actual_size,
            zoom,
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
                zoom_mode: ZoomMode::Fit,
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
            preview_generation: Cell::new(0),
            preset_pending: Cell::new(false),
            compare_source_artifact: options.compare_source,
            arrange_motif_artifact: options.arrange_motif,
        });

        ui.connect_actions(open, start, editor_view);
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
        }
        ui
    }

    pub fn present(self: &Rc<Self>) {
        self.window.present();
        if self.screenshot_path.is_some() && self.state.borrow().editor.is_none() {
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

    fn connect_actions(
        self: &Rc<Self>,
        open: gtk::Button,
        start: StartWidgets,
        editor: EditorWidgets,
    ) {
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
        connect_clicked(&editor.fit, self, |ui| ui.set_fit());
        connect_clicked(&editor.actual_size, self, |ui| ui.set_zoom(1.0));
        editor.zoom.connect_value_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |scale| {
                if !ui.state.borrow().syncing_controls {
                    ui.set_zoom(scale.value());
                }
            }
        ));
        self.compare.connect_toggled(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| {
                ui.state.borrow_mut().compare_source = button.is_active();
                ui.request_preview();
            }
        ));

        self.connect_treatment(&self.dots, Treatment::Dots);
        self.connect_treatment(&self.squares, Treatment::Squares);
        self.connect_treatment(&self.lines, Treatment::Lines);
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
        connect_clicked(&self.preset_import, self, |ui| ui.open_preset_dialog());
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
                let mode = match combo.selected() {
                    0 => ValueMode::Cmyk,
                    1 => ValueMode::SingleChannel,
                    2 => ValueMode::Luminance,
                    3 => ValueMode::CrosshatchLuminance,
                    4 => ValueMode::InvertedLuminance,
                    _ => return,
                };
                ui.change_web_treatment(move |settings, _| settings.value_mode = mode);
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
        self.web_shape.connect_selected_notify(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |combo| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let shape = match combo.selected() {
                    0 => WebShape::Circle,
                    1 => WebShape::Rectangle,
                    2 => WebShape::Triangle,
                    3 => WebShape::Pentagon,
                    4 => WebShape::Hexagon,
                    _ => return,
                };
                ui.change_web_treatment(move |settings, _| settings.shared_shape = shape);
            }
        ));
        for (index, button) in self.web_visible.iter().enumerate() {
            button.connect_toggled(glib::clone!(
                #[weak(rename_to = ui)]
                self,
                move |button| {
                    if ui.state.borrow().syncing_controls {
                        return;
                    }
                    let ink = Ink::ALL[index];
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
                let mode = match combo.selected() {
                    0 => ValueMode::Cmyk,
                    1 => ValueMode::SingleChannel,
                    2 => ValueMode::Luminance,
                    3 => ValueMode::CrosshatchLuminance,
                    4 => ValueMode::InvertedLuminance,
                    _ => return,
                };
                ui.change_curve_treatment(move |settings, _| settings.value_mode = mode);
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
            ui.apply_curve_profile(CurvePath::soft_wave());
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
                    let ink = Ink::ALL[index];
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
            |channel, value| channel.scale = value,
        );
        self.connect_curve_scale(
            &self.curve_angle,
            SettingKey::CurveAngle,
            |channel, value| channel.grid_rotation = value,
        );
        self.connect_curve_scale(
            &self.curve_position_x,
            SettingKey::CurvePositionX,
            |channel, value| channel.offset_x = value,
        );
        self.connect_curve_scale(
            &self.curve_position_y,
            SettingKey::CurvePositionY,
            |channel, value| channel.offset_y = value,
        );
        self.connect_curve_scale(
            &self.curve_opacity,
            SettingKey::CurveOpacity,
            |channel, value| channel.opacity = value,
        );
        self.connect_curve_scale(
            &self.curve_threshold,
            SettingKey::CurveThreshold,
            |channel, value| channel.threshold = value,
        );
        self.connect_curve_scale(
            &self.curve_detail,
            SettingKey::CurveDetail,
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
                ui.change_curve_treatment(move |settings, inks| {
                    for ink in inks {
                        settings.channels.get_mut(ink).motif_coverage = coverage;
                    }
                });
            }
        ));
        self.connect_curve_scale(&self.motif_size, SettingKey::MotifSize, |channel, value| {
            channel.curve_scale = value
        });
        self.connect_curve_scale(
            &self.motif_columns,
            SettingKey::MotifColumns,
            |channel, value| channel.tile_count = value.round().clamp(1.0, 10_000.0) as u32,
        );
        self.connect_curve_scale(&self.motif_rows, SettingKey::MotifRows, |channel, value| {
            channel.stack_count = value.round().clamp(1.0, 10_000.0) as u32
        });
        self.connect_curve_scale(
            &self.motif_row_spacing,
            SettingKey::MotifRowSpacing,
            |channel, value| channel.stack_spacing = value,
        );
        self.connect_curve_scale(
            &self.motif_stagger,
            SettingKey::MotifStagger,
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
                ui.change_curve_treatment(move |settings, inks| {
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
            |channel, value| channel.scale = value,
        );
        self.connect_web_scale(&self.web_angle, SettingKey::WebAngle, |channel, value| {
            channel.grid_rotation = value
        });
        self.connect_web_scale(
            &self.web_threshold,
            SettingKey::WebThreshold,
            |channel, value| channel.threshold = value,
        );
        self.connect_web_scale(
            &self.web_opacity,
            SettingKey::WebOpacity,
            |channel, value| channel.opacity = value,
        );
        self.connect_web_scale(&self.web_detail, SettingKey::WebDetail, |channel, value| {
            channel.resolution_scale = value
        });

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

    fn connect_treatment(self: &Rc<Self>, button: &gtk::ToggleButton, treatment: Treatment) {
        button.connect_toggled(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |button| {
                if !button.is_active() || ui.state.borrow().syncing_controls {
                    return;
                }
                ui.activate_native_treatment(treatment);
                ui.angle.set_sensitive(treatment != Treatment::Dots);
            }
        ));
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
        setter: impl Fn(&mut toniator::WebShapeChannel, f64) + 'static,
    ) {
        scale.connect_value_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |scale| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let value = scale.value();
                ui.change_web_treatment(|settings, inks| {
                    for ink in inks {
                        setter(settings.channels.get_mut(ink), value);
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
        setter: impl Fn(&mut WebCurveChannel, f64) + 'static,
    ) {
        scale.connect_value_changed(glib::clone!(
            #[weak(rename_to = ui)]
            self,
            move |scale| {
                if ui.state.borrow().syncing_controls {
                    return;
                }
                let value = scale.value();
                ui.change_curve_treatment(|settings, inks| {
                    for ink in inks {
                        setter(settings.channels.get_mut(ink), value);
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
                let width = (ui.curve_editor.width() - 32).max(1) as f64;
                let height = (ui.curve_editor.height() - 32).max(1) as f64;
                let point = CurvePoint {
                    x: (start.x + offset_x * 1.3 / width).clamp(-1.5, 1.5),
                    y: (start.y - offset_y * 1.3 / height).clamp(-1.5, 1.5),
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
        let preset = gtk::Button::with_label("Apply Treatment Preset…");
        artwork.add_css_class("flat");
        document.add_css_class("flat");
        preset.add_css_class("flat");
        preset.set_sensitive(self.state.borrow().editor.is_some());
        box_.append(&artwork);
        box_.append(&document);
        box_.append(&preset);
        popover.set_child(Some(&box_));
        popover.set_parent(&self.window);
        connect_clicked(&artwork, self, |ui| ui.open_artwork_dialog());
        connect_clicked(&document, self, |ui| ui.open_document_dialog());
        connect_clicked(&preset, self, |ui| ui.open_preset_dialog());
        popover.popup();
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

    fn open_preset_dialog(self: &Rc<Self>) {
        if self.state.borrow().editor.is_none() {
            return;
        }
        let dialog = gtk::FileDialog::builder()
            .title("Apply Treatment Preset")
            .modal(true)
            .build();
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        let presets = gtk::FileFilter::new();
        presets.set_name(Some("Toniator Treatment Presets"));
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
        let (document, name) = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                return;
            };
            let stem = Path::new(&editor.document().source.name)
                .file_stem()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .unwrap_or("Toniator");
            (editor.document().clone(), format!("{stem} Treatment"))
        };
        let dialog = gtk::FileDialog::builder()
            .title("Save Treatment Without Artwork")
            .modal(true)
            .initial_name(format!("{name}.tntr"))
            .build();
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        let treatments = gtk::FileFilter::new();
        treatments.set_name(Some("Toniator Treatment (.tntr)"));
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
                    let bytes =
                        match toniator::preset::document_treatment_preset_bytes(&name, &document) {
                            Ok(bytes) => bytes,
                            Err(error) => {
                                ui.show_error(&format!("Could not save treatment: {error:#}"));
                                return;
                            }
                        };
                    match toniator::persistence::atomic_write(&path, &bytes) {
                        Ok(()) => ui.show_message(&format!("Saved treatment {}", path.display())),
                        Err(error) => {
                            ui.show_error(&format!("Could not save treatment: {error:#}"))
                        }
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
        let document = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                self.show_message("Open artwork before applying a treatment preset.");
                return;
            };
            editor.document().clone()
        };
        let document_id = document.document_id.clone();
        let generation = self.preset_gate.next();
        self.preset_pending.set(true);
        let result = Arc::new(LatestSlot::default());
        let worker_result = Arc::clone(&result);
        let path = path.to_owned();
        if self.recovery_enabled {
            self.show_message("Reading treatment preset…");
        }
        std::thread::spawn(move || {
            let parsed = (|| -> anyhow::Result<toniator::preset::ParsedTreatment> {
                let bytes = std::fs::read(&path)?;
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
                                    ui.show_message("Treatment preset applied");
                                }
                            }
                        }
                        Err(error) => {
                            ui.show_error(&format!("Could not apply treatment preset: {error:#}"))
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
        self.show_message(if is_document {
            "Opening Toniator document…"
        } else {
            "Validating artwork…"
        });
        std::thread::spawn(move || {
            let candidate = (|| -> anyhow::Result<Document> {
                let document = if is_document {
                    load_document(&path_for_worker)?
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
                    Document::new(source)
                };
                toniator::render::decode_source(&document.source, 128)?;
                Ok(document)
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
                        Ok(document) => {
                            let install_path = if is_document && !recovered {
                                Some(path.clone())
                            } else {
                                None
                            };
                            ui.gate_dirty_transition(move |ui| {
                                ui.install_document(document.clone(), install_path.clone());
                                if recovered {
                                    ui.show_message(
                                        "Recovered autosaved work — save it when ready.",
                                    );
                                }
                            });
                        }
                        Err(error) => ui.show_error(&format!(
                            "Could not open {}: {error:#}",
                            if is_document { "document" } else { "artwork" }
                        )),
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
        self.install_document(Document::new(source), None);
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
        let should_autosave = path.is_none();
        let recovery_document = should_autosave.then(|| document.clone());
        {
            let mut state = self.state.borrow_mut();
            state.editor = Some(DocumentEditor::new(document));
            state.path = path;
            state.compare_source = false;
            state.preview_size = None;
            state.zoom_mode = ZoomMode::Fit;
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
        if self.state.borrow().compare_source {
            self.state.borrow_mut().compare_source = false;
            self.compare.set_active(false);
        }
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
        self.queue_autosave(document);
        self.request_preview();
        self.update_actions();
    }

    fn selected_web_inks(&self) -> Vec<Ink> {
        match self.web_target.selected() {
            1 => vec![Ink::Cyan],
            2 => vec![Ink::Magenta],
            3 => vec![Ink::Yellow],
            4 => vec![Ink::Black],
            _ => Ink::ALL.to_vec(),
        }
    }

    fn selected_curve_inks(&self) -> Vec<Ink> {
        match self.curve_target.selected() {
            1 => vec![Ink::Cyan],
            2 => vec![Ink::Magenta],
            3 => vec![Ink::Yellow],
            4 => vec![Ink::Black],
            _ => Ink::ALL.to_vec(),
        }
    }

    fn activate_native_treatment(self: &Rc<Self>, treatment: Treatment) {
        let document = {
            let mut state = self.state.borrow_mut();
            let Some(editor) = state.editor.as_mut() else {
                return;
            };
            editor.begin_edit(SettingKey::Treatment);
            let mut settings = editor.document().settings;
            settings.treatment = treatment;
            let changed_settings = editor.set_settings(SettingKey::Treatment, settings);
            let changed_render = editor.set_render_variant(RenderVariant::NativeBasicV1);
            editor.end_edit();
            if !changed_settings && !changed_render {
                return;
            }
            editor.document().clone()
        };
        self.after_treatment_edit(document);
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
            let document = editor.document().clone();
            state.compare_source = false;
            document
        };
        self.compare.set_active(false);
        self.after_treatment_edit(document);
    }

    fn after_treatment_edit(&self, document: Document) {
        self.queue_autosave(document);
        self.sync_controls();
        self.request_preview();
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
            let document = editor.document().clone();
            state.compare_source = false;
            document
        };
        self.compare.set_active(false);
        self.queue_autosave(document);
        self.sync_controls();
        self.request_preview();
        self.update_actions();
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
            RenderVariant::NativeBasicV1 => self.treatment_modes.set_visible_child_name("native"),
            RenderVariant::WebShapeV1 { settings } => {
                self.treatment_modes.set_visible_child_name("web");
                self.web_value_mode.set_selected(match settings.value_mode {
                    ValueMode::Cmyk => 0,
                    ValueMode::SingleChannel => 1,
                    ValueMode::Luminance => 2,
                    ValueMode::CrosshatchLuminance => 3,
                    ValueMode::InvertedLuminance => 4,
                });
                self.web_output_ink_row
                    .set_visible(settings.value_mode == ValueMode::SingleChannel);
                self.web_output_ink
                    .set_selected(match settings.single_channel {
                        Ink::Cyan => 0,
                        Ink::Magenta => 1,
                        Ink::Yellow => 2,
                        Ink::Black => 3,
                    });
                self.web_shape.set_selected(match settings.shared_shape {
                    WebShape::Circle => 0,
                    WebShape::Rectangle => 1,
                    WebShape::Triangle => 2,
                    WebShape::Pentagon => 3,
                    WebShape::Hexagon => 4,
                });
                self.web_shape_row.set_visible(settings.use_shared_mark);
                self.web_geometry_note.set_text(if settings.use_shared_mark { "One shape shared by all inks." } else { "Per-ink preset geometry is preserved. Independent geometry editing is not available yet." });
                for (ink, button) in Ink::ALL.into_iter().zip(&self.web_visible) {
                    button.set_active(settings.channels.get(ink).enabled);
                }
                let inks = self.selected_web_inks();
                let first = settings.channels.get(inks[0]);
                let differs = |value: fn(&toniator::WebShapeChannel) -> f64| {
                    inks.iter()
                        .skip(1)
                        .any(|ink| (value(settings.channels.get(*ink)) - value(first)).abs() > 1e-9)
                };
                let mixed_fields = [
                    differs(|c| c.scale),
                    differs(|c| c.grid_rotation),
                    differs(|c| c.threshold),
                    differs(|c| c.opacity),
                    differs(|c| c.resolution_scale),
                ];
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
                self.web_color
                    .set_text(if colors_mixed { "" } else { &first.color });
                self.web_color.set_placeholder_text(Some(if colors_mixed {
                    "Mixed"
                } else {
                    "#RRGGBB"
                }));
                self.web_color_status
                    .set_text(if colors_mixed { "Mixed" } else { "Hex color" });
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
                    &self.web_threshold,
                    &self.web_threshold_status,
                    first.threshold,
                    mixed_fields[2],
                    "Hide light marks",
                );
                sync_web_scale(
                    &self.web_opacity,
                    &self.web_opacity_status,
                    first.opacity,
                    mixed_fields[3],
                    "Transparent — Solid",
                );
                sync_web_scale(
                    &self.web_detail,
                    &self.web_detail_status,
                    first.resolution_scale,
                    mixed_fields[4],
                    "Sample density",
                );
            }
            RenderVariant::WebCurveV1 { settings } => {
                self.curves.set_active(true);
                self.treatment_modes.set_visible_child_name("curve");
                self.curve_value_mode
                    .set_selected(match settings.value_mode {
                        ValueMode::Cmyk => 0,
                        ValueMode::SingleChannel => 1,
                        ValueMode::Luminance => 2,
                        ValueMode::CrosshatchLuminance => 3,
                        ValueMode::InvertedLuminance => 4,
                    });
                self.curve_output_ink_row
                    .set_visible(settings.value_mode == ValueMode::SingleChannel);
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
                for (ink, button) in Ink::ALL.into_iter().zip(&self.curve_visible) {
                    button.set_active(settings.channels.get(ink).enabled);
                }
                let inks = self.selected_curve_inks();
                let first = settings.channels.get(inks[0]);
                let pattern_mixed = inks.iter().skip(1).any(|ink| {
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
                let arrangement_mixed = inks.iter().skip(1).any(|ink| {
                    let channel = settings.channels.get(*ink);
                    (channel.grid_rotation - first.grid_rotation).abs() > 1e-9
                        || (channel.offset_x - first.offset_x).abs() > 1e-9
                        || (channel.offset_y - first.offset_y).abs() > 1e-9
                        || (channel.stack_spacing - first.stack_spacing).abs() > 1e-9
                });
                self.curve_editor_label
                    .set_text(if settings.use_shared_curve {
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
                let mixed_fields = [
                    differs(|channel| channel.scale),
                    differs(|channel| channel.grid_rotation),
                    differs(|channel| channel.offset_x),
                    differs(|channel| channel.offset_y),
                    differs(|channel| channel.opacity),
                    differs(|channel| channel.threshold),
                    differs(|channel| channel.resolution_scale),
                ];
                let colors_mixed = inks
                    .iter()
                    .skip(1)
                    .any(|ink| settings.channels.get(*ink).color != first.color);
                self.curve_color
                    .set_text(if colors_mixed { "" } else { &first.color });
                self.curve_color.set_placeholder_text(Some(if colors_mixed {
                    "Mixed"
                } else {
                    "#RRGGBB"
                }));
                self.curve_color_status
                    .set_text(if colors_mixed { "Mixed" } else { "Hex color" });
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

    fn request_preview(&self) {
        let (document, compare_source) = {
            let state = self.state.borrow();
            let Some(editor) = state.editor.as_ref() else {
                return;
            };
            (editor.document().clone(), state.compare_source)
        };
        let generation = self.gate.next();
        self.render_status.set_text(if compare_source {
            "Loading source…"
        } else {
            "Updating preview…"
        });
        self.render_requests.replace(RenderRequest {
            generation,
            document,
            compare_source,
        });
    }

    fn poll_render_results(self: &Rc<Self>) {
        let Some(outcome) = self.render_results.take() else {
            return;
        };
        if !self.gate.accepts(outcome.generation) {
            return;
        }
        match outcome.result {
            Ok(image) => {
                self.preview_generation.set(outcome.generation);
                self.install_preview(image)
            }
            Err(error) => {
                self.render_status.set_text("Preview unavailable");
                self.show_error(&format!("Could not render preview: {error:#}"));
            }
        }
    }

    fn install_preview(self: &Rc<Self>, image: RgbaImage) {
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
        self.render_status
            .set_text(if self.state.borrow().compare_source {
                "Source artwork"
            } else {
                "Preview up to date"
            });
        self.apply_zoom_mode();

        if self.screenshot_path.is_some() {
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

    fn write_cli_artifacts(&self) {
        if self.preset_pending.get() || self.preview_generation.get() != self.gate.current() {
            return;
        }
        if self.cli_artifacts_written.replace(true) {
            return;
        }
        if let Some(path) = self.screenshot_path.as_ref()
            && let Err(error) = self.capture_window(path)
        {
            self.show_error(&format!("Could not write window screenshot: {error:#}"));
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
                Err(error) => self.show_error(&format!("Could not export SVG: {error:#}")),
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
                Err(error) => self.show_error(&format!("Could not export PNG: {error:#}")),
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
            self.show_error(&format!("Could not save artifact document: {error:#}"));
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
                Err(error) => self.show_error(&format!("Could not save treatment: {error:#}")),
            }
        }
        if !self.recovery_enabled {
            self.close_approved.set(true);
            self.window.close();
        }
    }

    fn capture_window(&self, path: &Path) -> anyhow::Result<()> {
        use gtk::gdk::prelude::PaintableExt;
        let width = self.window.width().max(1) as u32;
        let height = self.window.height().max(1) as u32;
        let paintable = gtk::WidgetPaintable::new(Some(&self.window));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, width as f64, height as f64);
        let node = snapshot
            .to_node()
            .ok_or_else(|| anyhow::anyhow!("GTK produced no render node"))?;
        let surface = self
            .window
            .surface()
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
            if self.save_to_path(&path) {
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
                        && ui.save_to_path(&ensure_extension(path, "toniator"))
                    {
                        continuation(&ui);
                    }
                }
            ),
        );
    }

    fn save_to_path(&self, path: &Path) -> bool {
        let document = self
            .state
            .borrow()
            .editor
            .as_ref()
            .map(|editor| editor.document().clone());
        let Some(document) = document else {
            return false;
        };
        match save_document_atomic(path, &document) {
            Ok(()) => {
                let mut state = self.state.borrow_mut();
                state.path = Some(path.to_owned());
                if let Some(editor) = state.editor.as_mut() {
                    editor.mark_clean();
                }
                drop(state);
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
                    }
                }
                self.update_actions();
                self.show_message(&format!("Saved {}", path.display()));
                true
            }
            Err(error) => {
                self.show_error(&format!("Could not save document: {error:#}"));
                false
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
        if !self.has_dirty_document() {
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
                move |response| match response.as_str() {
                    "save" => {
                        let continuation = Rc::clone(&continuation);
                        ui.save_then(move |ui| continuation(ui));
                    }
                    "discard" => match ui.clear_current_recovery() {
                        Ok(()) => continuation(&ui),
                        Err(error) =>
                            ui.show_error(&format!("Could not safely discard recovery: {error:#}")),
                    },
                    _ => {}
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

    fn set_fit(&self) {
        self.state.borrow_mut().zoom_mode = ZoomMode::Fit;
        self.apply_zoom_mode();
    }

    fn apply_zoom_mode(&self) {
        let mode = self.state.borrow().zoom_mode;
        match mode {
            ZoomMode::Fit => {
                self.fit.set_active(true);
                self.actual_size.set_active(false);
                self.zoom.set_sensitive(false);
                self.zoom.set_draw_value(false);
                self.picture.set_size_request(-1, -1);
                self.picture.set_hexpand(true);
                self.picture.set_vexpand(true);
                self.picture.set_content_fit(gtk::ContentFit::Contain);
            }
            ZoomMode::Scale(zoom) => self.apply_zoom_scale(zoom),
        }
    }

    fn apply_zoom_scale(&self, zoom: f64) {
        self.fit.set_active(false);
        self.actual_size.set_active((zoom - 1.0).abs() < 0.001);
        self.zoom.set_sensitive(true);
        self.zoom.set_draw_value(true);
        if (self.zoom.value() - zoom).abs() > f64::EPSILON {
            self.state.borrow_mut().syncing_controls = true;
            self.zoom.set_value(zoom);
            self.state.borrow_mut().syncing_controls = false;
        }
        self.picture.set_size_request(-1, -1);
        let Some((width, height)) = self.state.borrow().preview_size else {
            return;
        };
        self.picture.set_hexpand(false);
        self.picture.set_vexpand(false);
        self.picture.set_content_fit(gtk::ContentFit::Fill);
        self.picture.set_size_request(
            (width as f64 * zoom).round() as i32,
            (height as f64 * zoom).round() as i32,
        );
    }

    fn set_zoom(&self, zoom: f64) {
        self.state.borrow_mut().zoom_mode = ZoomMode::Scale(zoom);
        self.apply_zoom_scale(zoom);
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
            toniator::render::decode_source(&request.document.source, 1400)
        } else {
            render_document_preview(&request.document, 1400, request.generation)
                .map(|rendered| rendered.image)
        };
        results.replace(RenderOutcome {
            generation: request.generation,
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
    let icon = gtk::Image::from_icon_name("applications-graphics-symbolic");
    icon.set_pixel_size(64);
    icon.add_css_class("accent");
    let title = gtk::Label::builder()
        .label("Turn artwork into print-ready halftones")
        .css_classes(["title-1"])
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    let subtitle = gtk::Label::builder()
        .label("Start with a useful result, then shape the treatment to fit your work.")
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
    page.append(&icon);
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
    fit: gtk::ToggleButton,
    actual_size: gtk::ToggleButton,
    zoom: gtk::Scale,
    treatment_modes: gtk::Stack,
    preset_import: gtk::Button,
    preset_save: gtk::Button,
    web_value_mode: gtk::DropDown,
    web_output_ink: gtk::DropDown,
    web_output_ink_row: gtk::Widget,
    web_shape: gtk::DropDown,
    web_shape_row: gtk::Widget,
    web_target: gtk::DropDown,
    web_visible: [gtk::CheckButton; 4],
    web_color: gtk::Entry,
    web_color_status: gtk::Label,
    web_coverage: gtk::Scale,
    web_coverage_status: gtk::Label,
    web_angle: gtk::Scale,
    web_angle_status: gtk::Label,
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
    curve_visible: [gtk::CheckButton; 4],
    curve_color: gtk::Entry,
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
    render_status: &gtk::Label,
    autosave_status: &gtk::Label,
    detail: &gtk::Scale,
    coverage: &gtk::Scale,
    contrast: &gtk::Scale,
    angle: &gtk::Scale,
    dots: &gtk::ToggleButton,
    squares: &gtk::ToggleButton,
    lines: &gtk::ToggleButton,
    curves: &gtk::ToggleButton,
    compare: &gtk::ToggleButton,
) -> EditorWidgets {
    let layout = gtk::Box::new(gtk::Orientation::Horizontal, 0);
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
    let canvas = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .css_classes(["canvas"])
        .child(&canvas_overlay)
        .build();
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    controls.set_margin_top(8);
    controls.set_margin_bottom(8);
    controls.set_margin_start(12);
    controls.set_margin_end(12);
    let fit = gtk::ToggleButton::with_label("Fit");
    let actual_size = gtk::ToggleButton::with_label("Actual Size");
    fit.add_css_class("flat");
    actual_size.add_css_class("flat");
    let zoom = control_scale(0.25, 2.0, 0.05);
    zoom.set_value(1.0);
    zoom.set_width_request(150);
    zoom.set_tooltip_text(Some("Canvas zoom"));
    zoom.set_format_value_func(|_, value| format!("{:.0}%", value * 100.0));
    zoom.update_property(&[gtk::accessible::Property::Label("Canvas zoom")]);
    compare.add_css_class("flat");
    controls.append(&fit);
    controls.append(&actual_size);
    controls.append(&zoom);
    controls.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    controls.append(compare);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    controls.append(&spacer);
    controls.append(render_status);
    canvas_box.append(&canvas);
    canvas_box.append(&controls);

    let inspector = gtk::Box::new(gtk::Orientation::Vertical, 14);
    inspector.set_width_request(352);
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
        .label("Treatment")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let treatment_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    treatment_buttons.add_css_class("linked");
    dots.set_hexpand(true);
    squares.set_hexpand(true);
    lines.set_hexpand(true);
    curves.set_hexpand(true);
    treatment_buttons.append(dots);
    treatment_buttons.append(squares);
    treatment_buttons.append(lines);
    treatment_buttons.append(curves);
    for (button, label) in [
        (dots, "Dots treatment"),
        (squares, "Squares treatment"),
        (lines, "Lines treatment"),
        (curves, "Curves treatment"),
    ] {
        button.update_property(&[gtk::accessible::Property::Label(label)]);
    }
    inspector.append(&inspector_title);
    inspector.append(&treatment_caption);
    inspector.append(&treatment_buttons);
    let preset_actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    preset_actions.add_css_class("linked");
    let preset_import = gtk::Button::with_label("Apply Treatment…");
    preset_import.set_hexpand(true);
    preset_import.set_tooltip_text(Some("Apply a Toniator treatment (.tntr)"));
    let preset_save = gtk::Button::with_label("Save Treatment…");
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
    let web_value_mode = gtk::DropDown::from_strings(&[
        "Full Color (CMYK)",
        "Single Ink",
        "Layered Grayscale",
        "Progressive Layers",
        "Inverted Grayscale",
    ]);
    web_panel.append(&combo_row("Color Interpretation", &web_value_mode));
    let web_output_ink = gtk::DropDown::from_strings(&["Cyan", "Magenta", "Yellow", "Black"]);
    let web_output_ink_row = combo_row("Output Ink", &web_output_ink);
    web_output_ink_row.set_visible(false);
    web_panel.append(&web_output_ink_row);
    let web_shape = gtk::DropDown::from_strings(&[
        "Circle / Dots",
        "Rectangle / Squares",
        "Triangle",
        "Pentagon",
        "Hexagon",
    ]);
    let web_shape_row = combo_row("Shape", &web_shape);
    web_panel.append(&web_shape_row);
    let web_geometry_note = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label", "caption"])
        .build();
    web_panel.append(&web_geometry_note);
    let web_target =
        gtk::DropDown::from_strings(&["All Inks", "Cyan", "Magenta", "Yellow", "Black"]);
    web_panel.append(&combo_row("Edit Ink", &web_target));
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
    web_panel.append(
        &gtk::Label::builder()
            .label("Visible Inks")
            .xalign(0.0)
            .css_classes(["heading"])
            .build(),
    );
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
    let web_coverage = control_scale(0.0, 5.0, 0.05);
    let web_angle = control_scale(-360.0, 360.0, 1.0);
    let (web_coverage_row, web_coverage_status) =
        control_status_row("Coverage", "Mark size", &web_coverage);
    web_panel.append(&web_coverage_row);
    let (web_angle_row, web_angle_status) =
        control_status_row("Screen Angle", "Rotate ink screen", &web_angle);
    web_panel.append(&web_angle_row);
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
    let curve_value_mode = gtk::DropDown::from_strings(&[
        "Full Color (CMYK)",
        "Single Ink",
        "Layered Grayscale",
        "Progressive Layers",
        "Inverted Grayscale",
    ]);
    curve_panel.append(&combo_row("Color Interpretation", &curve_value_mode));
    let curve_output_ink = gtk::DropDown::from_strings(&["Cyan", "Magenta", "Yellow", "Black"]);
    let curve_output_ink_row = combo_row("Output Ink", &curve_output_ink);
    curve_output_ink_row.set_visible(false);
    curve_panel.append(&curve_output_ink_row);
    let curve_layout = gtk::DropDown::from_strings(&["Across Page", "Repeated Motif"]);
    curve_panel.append(&combo_row("Layout", &curve_layout));
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
        .content_height(150)
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
    let curve_shared = gtk::CheckButton::with_label("Use One Shape for All Inks");
    curve_shared.set_active(true);
    curve_actions.append(&curve_reset);
    curve_panel.append(&curve_actions);
    curve_panel.append(&curve_shared);
    let curve_target =
        gtk::DropDown::from_strings(&["All Inks", "Cyan", "Magenta", "Yellow", "Black"]);
    curve_panel.append(&combo_row("Edit Ink", &curve_target));
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
    curve_panel.append(
        &gtk::Label::builder()
            .label("Visible Inks")
            .xalign(0.0)
            .css_classes(["heading"])
            .build(),
    );
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
    let curve_weight = control_scale(1.0, 200.0, 1.0);
    curve_panel.append(&control_row(
        "Weight",
        "Overall curve thickness",
        &curve_weight,
    ));
    let curve_spacing = control_scale(8.0, 220.0, 1.0);
    curve_panel.append(&control_row(
        "Spacing",
        "Far apart — Close together",
        &curve_spacing,
    ));
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
    layout.append(&canvas_box);
    layout.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    layout.append(&inspector_scroll);
    EditorWidgets {
        container: layout.upcast(),
        fit,
        actual_size,
        zoom,
        treatment_modes,
        preset_import,
        preset_save,
        web_value_mode,
        web_output_ink,
        web_output_ink_row,
        web_shape,
        web_shape_row,
        web_target,
        web_visible,
        web_color,
        web_color_status,
        web_coverage,
        web_coverage_status,
        web_angle,
        web_angle_status,
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
        curve_visible,
        curve_color,
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
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    row.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["heading"])
            .build(),
    );
    row.append(combo);
    row.upcast()
}

fn entry_status_row(title: &str, status: &str, entry: &gtk::Entry) -> (gtk::Widget, gtk::Label) {
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
        .css_classes(["dim-label", "caption"])
        .build();
    labels.append(&title);
    labels.append(&status);
    row.append(&labels);
    row.append(scale);
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
    scale.set_draw_value(!mixed);
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

fn curve_to_editor_point(point: CurvePoint, width: i32, height: i32) -> (f64, f64) {
    let width = (width - 32).max(1) as f64;
    let height = (height - 32).max(1) as f64;
    (
        16.0 + (point.x + 0.65) / 1.3 * width,
        16.0 + (0.65 - point.y) / 1.3 * height,
    )
}

fn editor_to_curve_point(x: f64, y: f64, width: i32, height: i32) -> CurvePoint {
    let width = (width - 32).max(1) as f64;
    let height = (height - 32).max(1) as f64;
    CurvePoint {
        x: ((x - 16.0) / width * 1.3 - 0.65).clamp(-1.5, 1.5),
        y: (0.65 - (y - 16.0) / height * 1.3).clamp(-1.5, 1.5),
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
        .css_classes(["dim-label", "caption"])
        .build();
    labels.append(&title);
    labels.append(&subtitle);
    row.append(&labels);
    row.append(scale);
    scale.update_relation(&[gtk::accessible::Relation::LabelledBy(&[title.upcast_ref()])]);
    row.upcast()
}

fn control_scale(minimum: f64, maximum: f64, step: f64) -> gtk::Scale {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, minimum, maximum, step);
    scale.set_draw_value(true);
    scale.set_value_pos(gtk::PositionType::Right);
    scale.set_hexpand(true);
    scale
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
        .artboard { background: transparent; margin: 30px; }
        .inspector-pane, .inspector { background: @window_bg_color; }
        .inspector { min-width: 316px; }
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
}
