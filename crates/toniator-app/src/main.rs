#![forbid(unsafe_code)]

//! The bounded Stage 10 native, view-only preview frontend.

use std::{
    cell::RefCell,
    env, fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        mpsc::{self, Receiver, TryRecvError},
    },
    thread,
    time::Duration,
};

use adw::prelude::*;
use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelSourceMapping,
    ChannelState, ChannelTopologyTemplate, ColorValue, DensityMetric2D, Document, DocumentCommand,
    DocumentId, DocumentSession, HalftoneChannelModel, MarkGeometryResponse, PatternDefinition,
    PatternDefinitionId, PatternOutput, PatternStructure, SourceComponent, SourcePlacement,
    SourceReference, SourceReferenceId,
};
use toniator_engine::{
    EvaluationRequest, EvaluationScheduler, RasterSurface, ResolvedSource, SourceFormatHint,
    SourceIdentity, resolve_source_identity,
};

const APP_ID: &str = "com.silentbutdigital.Toniator";
const RESOURCE_PREFIX: &str = "/com/silentbutdigital/Toniator";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreviewModel {
    Rgb,
    Cmyk,
    SourceColorAlpha,
}

impl PreviewModel {
    const ALL: [Self; 3] = [Self::Rgb, Self::Cmyk, Self::SourceColorAlpha];

    const fn label(self) -> &'static str {
        match self {
            Self::Rgb => "RGB",
            Self::Cmyk => "CMYK",
            Self::SourceColorAlpha => "Source color + alpha",
        }
    }

    const fn domain(self) -> HalftoneChannelModel {
        match self {
            Self::Rgb => HalftoneChannelModel::Rgb,
            Self::Cmyk => HalftoneChannelModel::Cmyk,
            Self::SourceColorAlpha => HalftoneChannelModel::SourceColorAlpha,
        }
    }

    const fn css_class(self) -> &'static str {
        match self {
            Self::Rgb => "toniator-viewer-rgb",
            Self::Cmyk => "toniator-viewer-cmyk",
            Self::SourceColorAlpha => "toniator-viewer-source-color-alpha",
        }
    }
}

#[derive(Clone, Debug)]
struct LoadedSource {
    bytes: Arc<[u8]>,
    format: SourceFormatHint,
    identity: SourceIdentity,
    reference_id: SourceReferenceId,
    display_name: String,
}

struct PendingLoad {
    generation: u64,
    receiver: Receiver<Result<LoadedSource, String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Empty,
    Loading,
    Error,
    Success,
}

/// The small application-owned state that can change what the user sees.
/// Scheduler ticket handling remains entirely in `EvaluationScheduler`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PresentationState {
    page: Page,
    accepted_generation: Option<u64>,
    title: Option<String>,
}

impl PresentationState {
    const fn empty() -> Self {
        Self {
            page: Page::Empty,
            accepted_generation: None,
            title: None,
        }
    }

    fn set_page(&mut self, page: Page) {
        self.page = page;
    }

    fn reset_source_metadata(&mut self) {
        self.accepted_generation = None;
        self.title = None;
    }

    /// Installs source display metadata only when the asynchronous loader is
    /// still current. This is deliberately separate from scheduler tickets.
    fn accept_source(
        &mut self,
        current_generation: u64,
        candidate_generation: u64,
        title: String,
    ) -> bool {
        if !is_current_generation(current_generation, candidate_generation) {
            return false;
        }
        self.accepted_generation = Some(candidate_generation);
        self.title = Some(title);
        true
    }
}

impl Page {
    const fn name(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Loading => "loading",
            Self::Error => "error",
            Self::Success => "success",
        }
    }
}

struct AppState {
    scheduler: EvaluationScheduler,
    session: Option<DocumentSession>,
    source: Option<LoadedSource>,
    pending_load: Option<PendingLoad>,
    generation: u64,
    model: PreviewModel,
    presentation: PresentationState,
    window: adw::ApplicationWindow,
    window_title: adw::WindowTitle,
    stack: gtk::Stack,
    picture: gtk::Picture,
    viewer: gtk::Box,
    error: gtk::Label,
    banner: adw::Banner,
    selector: gtk::DropDown,
    preview: Option<gtk::gdk::Texture>,
    preview_target: Option<toniator_engine::PreviewRasterTarget>,
}

fn main() {
    gio::resources_register_include!("toniator.gresource")
        .expect("failed to register compiled Toniator GResource");
    let initial_path = match parse_args(env::args_os().skip(1).collect()) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("toniator-app: {message}");
            std::process::exit(2);
        }
    };

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| build_window(app, initial_path.clone()));
    // The optional source path is consumed by our bounded parser above. Do not
    // hand it back to GApplication as an unsupported open-file request.
    app.run_with_args(&["toniator-app"]);
}

fn parse_args(arguments: Vec<std::ffi::OsString>) -> Result<Option<PathBuf>, String> {
    match arguments.as_slice() {
        [] => Ok(None),
        [path] if !path.to_string_lossy().starts_with('-') => Ok(Some(PathBuf::from(path))),
        _ => Err("usage: toniator-app [PATH]".to_owned()),
    }
}

fn build_window(app: &adw::Application, initial_path: Option<PathBuf>) {
    install_css();
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Toniator")
        .default_width(960)
        .default_height(720)
        .build();
    window.set_size_request(480, 360);

    let header = adw::HeaderBar::new();
    let open = gtk::Button::with_label("Open");
    open.set_tooltip_text(Some("Open PNG or SVG artwork (Ctrl+O)"));
    header.pack_start(&open);
    let model_label = gtk::Label::new(Some("_Model"));
    model_label.set_use_underline(true);
    model_label.add_css_class("dim-label");
    let selector = gtk::DropDown::from_strings(&PreviewModel::ALL.map(PreviewModel::label));
    selector.set_tooltip_text(Some("Authoritative channel model"));
    model_label.set_mnemonic_widget(Some(&selector));
    header.pack_end(&selector);
    header.pack_end(&model_label);
    let window_title = adw::WindowTitle::new("Toniator", "View-only preview");
    header.set_title_widget(Some(&window_title));

    let banner = adw::Banner::new("");
    banner.set_revealed(false);
    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.add_named(
        &status_page("Open a PNG or SVG artwork to preview it."),
        Some(Page::Empty.name()),
    );
    stack.add_named(
        &status_page("Loading source and evaluating the current model…"),
        Some(Page::Loading.name()),
    );
    let error = gtk::Label::new(None);
    error.set_wrap(true);
    error.set_max_width_chars(70);
    error.set_justify(gtk::Justification::Center);
    stack.add_named(&centered(error.clone()), Some(Page::Error.name()));
    let picture = gtk::Picture::new();
    picture.set_focusable(false);
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    let viewer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    viewer.add_css_class("toniator-viewer");
    viewer.add_css_class(PreviewModel::Rgb.css_class());
    viewer.append(&picture);
    stack.add_named(&viewer, Some(Page::Success.name()));
    stack.set_visible_child_name(Page::Empty.name());

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&banner);
    content.append(&stack);
    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&content));
    window.set_content(Some(&toolbar_view));

    let state = Rc::new(RefCell::new(AppState {
        scheduler: EvaluationScheduler::new().expect("failed to start evaluation scheduler"),
        session: None,
        source: None,
        pending_load: None,
        generation: 0,
        model: PreviewModel::Rgb,
        presentation: PresentationState::empty(),
        window: window.clone(),
        window_title,
        stack,
        picture,
        viewer,
        error,
        banner,
        selector,
        preview: None,
        preview_target: None,
    }));

    let action = gio::SimpleAction::new("open", None);
    {
        let state = Rc::clone(&state);
        let window = window.clone();
        action.connect_activate(move |_, _| choose_source(&window, &state));
    }
    app.add_action(&action);
    app.set_accels_for_action("app.open", &["<Primary>o"]);
    {
        let state = Rc::clone(&state);
        let window = window.clone();
        open.connect_clicked(move |_| choose_source(&window, &state));
    }
    {
        let state = Rc::clone(&state);
        let selector = state.borrow().selector.clone();
        selector.connect_selected_notify(move |selector| {
            let index = selector.selected() as usize;
            if let Some(model) = PreviewModel::ALL.get(index).copied() {
                change_model(&state, model);
            }
        });
    }
    {
        let state = Rc::clone(&state);
        glib::timeout_add_local(Duration::from_millis(16), move || poll(&state));
    }

    window.present();
    if let Some(path) = initial_path {
        start_load(&state, path);
    }
}

fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_resource(&format!("{RESOURCE_PREFIX}/toniator.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn centered(child: impl IsA<gtk::Widget>) -> gtk::Box {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 12);
    box_.set_halign(gtk::Align::Center);
    box_.set_valign(gtk::Align::Center);
    box_.append(&child);
    box_
}

fn status_page(message: &str) -> gtk::Box {
    let label = gtk::Label::new(Some(message));
    label.set_wrap(true);
    label.set_justify(gtk::Justification::Center);
    centered(label)
}

fn choose_source(window: &adw::ApplicationWindow, state: &Rc<RefCell<AppState>>) {
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Open artwork");
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("PNG and SVG artwork"));
    filter.add_mime_type("image/png");
    filter.add_mime_type("image/svg+xml");
    filter.add_pattern("*.png");
    filter.add_pattern("*.svg");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));
    let state = Rc::clone(state);
    dialog.open(Some(window), None::<&gio::Cancellable>, move |result| {
        if let Ok(file) = result
            && let Some(path) = file.path()
        {
            start_load(&state, path);
        }
    });
}

fn start_load(state: &Rc<RefCell<AppState>>, path: PathBuf) {
    let (sender, receiver) = mpsc::channel();
    let generation = {
        let mut state = state.borrow_mut();
        state.generation = state.generation.saturating_add(1);
        clear_preview(&mut state);
        state.preview_target = None;
        state.session = None;
        state.source = None;
        state.presentation.reset_source_metadata();
        update_source_title(&mut state);
        state.banner.set_revealed(false);
        state.pending_load = Some(PendingLoad {
            generation: state.generation,
            receiver,
        });
        set_page(&mut state, Page::Loading);
        state.generation
    };
    thread::spawn(move || {
        let loaded = load_source(&path);
        let _ = sender.send(loaded);
        let _ = generation;
    });
}

fn load_source(path: &Path) -> Result<LoadedSource, String> {
    let format = format_hint_for_path(path)?;
    let bytes: Arc<[u8]> = fs::read(path)
        .map(Arc::<[u8]>::from)
        .map_err(|error| format!("source.read: could not read {}: {error}", path.display()))?;
    let identity = resolve_source_identity(&bytes, format).map_err(|error| error.to_string())?;
    let reference_id = SourceReferenceId::new(format!(
        "app-source-{}",
        &identity.content_hash[..16.min(identity.content_hash.len())]
    ))
    .map_err(|error| error.to_string())?;
    Ok(LoadedSource {
        bytes,
        format,
        identity,
        reference_id,
        display_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled source")
            .to_owned(),
    })
}

fn format_hint_for_path(path: &Path) -> Result<SourceFormatHint, String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("png") => Ok(SourceFormatHint::Png),
        Some(extension) if extension.eq_ignore_ascii_case("svg") => Ok(SourceFormatHint::Svg),
        _ => Err("source.format: only PNG and SVG source formats are supported".to_owned()),
    }
}

fn change_model(state: &Rc<RefCell<AppState>>, model: PreviewModel) {
    let mut state = state.borrow_mut();
    if state.model == model || matches!(state.presentation.page, Page::Loading) {
        return;
    }
    if state.source.is_some()
        && let Some(session) = state.session.as_mut()
        && let Err(error) = replace_model_topology(session, model)
    {
        show_error(&mut state, error);
        return;
    }
    state.model = model;
    update_backdrop(&mut state);
    if state.source.is_some() {
        clear_preview(&mut state);
        // The session revision changed through ReplaceChannelTopology. A same
        // viewport must therefore submit a fresh authoritative evaluation.
        state.preview_target = None;
        set_page(&mut state, Page::Loading);
        submit_if_viewport_ready(&mut state);
    }
}

fn poll(state: &Rc<RefCell<AppState>>) -> glib::ControlFlow {
    let pending = {
        let mut state = state.borrow_mut();
        state.pending_load.take()
    };
    if let Some(pending) = pending {
        match pending.receiver.try_recv() {
            Ok(result) => {
                let mut state = state.borrow_mut();
                if is_current_generation(state.generation, pending.generation) {
                    match result {
                        Ok(source) => {
                            let current_generation = state.generation;
                            if !state.presentation.accept_source(
                                current_generation,
                                pending.generation,
                                source.display_name.clone(),
                            ) {
                                return glib::ControlFlow::Continue;
                            }
                            update_source_title(&mut state);
                            state.source = Some(source);
                            show_svg_diagnostic(&mut state);
                            let session = match document_session_for(
                                state
                                    .source
                                    .as_ref()
                                    .expect("loaded source was just installed"),
                                state.model,
                            ) {
                                Ok(session) => session,
                                Err(error) => {
                                    show_error(&mut state, error);
                                    return glib::ControlFlow::Continue;
                                }
                            };
                            state.session = Some(session);
                            submit_if_viewport_ready(&mut state);
                        }
                        Err(error) => show_error(&mut state, error),
                    }
                }
            }
            Err(TryRecvError::Empty) => state.borrow_mut().pending_load = Some(pending),
            Err(TryRecvError::Disconnected) => {
                let mut state = state.borrow_mut();
                if is_current_generation(state.generation, pending.generation) {
                    show_error(&mut state, "Source loader stopped unexpectedly.".to_owned());
                }
            }
        }
    }

    {
        let mut state = state.borrow_mut();
        if state.source.is_some() && state.session.is_some() {
            submit_if_viewport_ready(&mut state);
        }
    }
    let completion = state.borrow().scheduler.try_receive_latest();
    match completion {
        Ok(Some(completion)) => {
            let mut state = state.borrow_mut();
            let accepted = match state.session.as_ref() {
                Some(session) => state.scheduler.accept_completion(&completion, session),
                None => Ok(false),
            };
            match accepted {
                Ok(true) => match completion.result() {
                    Some(result) => match texture_from_surface(result.raster()) {
                        Ok(texture) => {
                            state.picture.set_paintable(Some(&texture));
                            state.preview = Some(texture);
                            set_page(&mut state, Page::Success);
                        }
                        Err(error) => show_error(&mut state, error),
                    },
                    None => show_error(
                        &mut state,
                        completion
                            .error()
                            .expect("failed completion has an error")
                            .to_string(),
                    ),
                },
                Ok(false) => {}
                Err(error) => show_error(&mut state, error.to_string()),
            }
        }
        Ok(None) => {}
        Err(error) => show_error(&mut state.borrow_mut(), error.to_string()),
    }
    glib::ControlFlow::Continue
}

fn submit_if_viewport_ready(state: &mut AppState) {
    let Some(target) = preview_target_for(&state.stack) else {
        return;
    };
    if !should_submit_target(state.preview_target, target) {
        return;
    }
    if let Err(error) = submit_current_source(state, target) {
        show_error(state, error);
    }
}

fn should_submit_target(
    last: Option<toniator_engine::PreviewRasterTarget>,
    target: toniator_engine::PreviewRasterTarget,
) -> bool {
    last != Some(target)
}

fn submit_current_source(
    state: &mut AppState,
    target: toniator_engine::PreviewRasterTarget,
) -> Result<(), String> {
    let source = state
        .source
        .clone()
        .ok_or_else(|| "No source is loaded.".to_owned())?;
    let session = state
        .session
        .as_ref()
        .ok_or_else(|| "No authoritative preview document is available.".to_owned())?;
    let resolved = ResolvedSource::new(source.reference_id.clone(), source.bytes, source.format)
        .map_err(|error| error.to_string())?;
    let request = EvaluationRequest::with_preview_target(
        session.document_evaluation_snapshot(),
        resolved,
        target,
    );
    state
        .scheduler
        .submit(request)
        .map_err(|error| error.to_string())?;
    state.preview_target = Some(target);
    if state.preview.is_none() {
        set_page(state, Page::Loading);
    }
    Ok(())
}

/// The stack remains allocated while Empty/Loading/Error pages are visible;
/// GtkPicture is intentionally only the hidden-on-loading presenter.
fn preview_target_for(stack: &gtk::Stack) -> Option<toniator_engine::PreviewRasterTarget> {
    preview_target_from_allocation(
        stack.allocated_width(),
        stack.allocated_height(),
        stack.scale_factor(),
    )
}

fn preview_target_from_allocation(
    width: i32,
    height: i32,
    scale: i32,
) -> Option<toniator_engine::PreviewRasterTarget> {
    if width <= 0 || height <= 0 || scale <= 0 {
        return None;
    }
    let width = u32::try_from(width)
        .ok()?
        .checked_mul(u32::try_from(scale).ok()?)?;
    let height = u32::try_from(height)
        .ok()?
        .checked_mul(u32::try_from(scale).ok()?)?;
    toniator_engine::PreviewRasterTarget::new(width, height).ok()
}

fn document_session_for(
    source: &LoadedSource,
    model: PreviewModel,
) -> Result<DocumentSession, String> {
    let width = f64::from(source.identity.width);
    let height = f64::from(source.identity.height);
    let layout = ChannelPatternLayout {
        density: DensityMetric2D {
            across_x: width / 10.0,
            across_y: height / 10.0,
            aspect_locked: true,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
    };
    let response = MarkGeometryResponse {
        minimum_size: 2.0,
        maximum_size: 9.0,
    };
    let definition = PatternDefinition {
        id: PatternDefinitionId(1),
        name: "Straight circular marks".to_owned(),
        structure: PatternStructure::StraightGrid,
        output: PatternOutput::CircularMarks,
        guard_steps: 2,
        maximum_support_radius: 4.5,
    };
    let legacy_channel = ChannelState {
        id: ChannelId(1),
        pattern_definition_id: definition.id,
        layout: layout.clone(),
        appearance: ChannelAppearance {
            visible: true,
            color: ColorValue {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            opacity: 1.0,
        },
        mark_geometry_response: response.clone(),
        source_mapping: ChannelSourceMapping {
            component: SourceComponent::Luminance,
            placement: SourcePlacement::StretchToCanvas,
        },
    };
    let document = Document::with_source(
        DocumentId(1),
        CanvasSpec { width, height },
        SourceReference::Assigned(source.reference_id.clone()),
        vec![definition],
        vec![legacy_channel],
    )
    .map_err(|error| error.to_string())?;
    let mut session = DocumentSession::new(document).map_err(|error| error.to_string())?;
    let template = ChannelTopologyTemplate {
        pattern_definition_id: PatternDefinitionId(1),
        layout,
        mark_geometry_response: response,
    };
    let topology = session
        .document()
        .canonical_channel_topology(model.domain(), template)
        .map_err(|error| error.to_string())?;
    session
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: model.domain(),
            topology,
        })
        .map_err(|error| error.to_string())?;
    Ok(session)
}

fn replace_model_topology(
    session: &mut DocumentSession,
    model: PreviewModel,
) -> Result<(), String> {
    let canvas = session.document().canvas();
    let template = ChannelTopologyTemplate {
        pattern_definition_id: PatternDefinitionId(1),
        layout: ChannelPatternLayout {
            density: DensityMetric2D {
                across_x: canvas.width / 10.0,
                across_y: canvas.height / 10.0,
                aspect_locked: true,
            },
            rotation_degrees: 0.0,
            translation_x: 0.0,
            translation_y: 0.0,
        },
        mark_geometry_response: MarkGeometryResponse {
            minimum_size: 2.0,
            maximum_size: 9.0,
        },
    };
    let topology = session
        .document()
        .canonical_channel_topology(model.domain(), template)
        .map_err(|error| error.to_string())?;
    session
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: model.domain(),
            topology,
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn texture_from_surface(surface: &RasterSurface) -> Result<gtk::gdk::Texture, String> {
    let layout = raw_texture_layout(surface)?;
    let bytes = glib::Bytes::from_owned(surface.pixels().to_vec());
    Ok(gtk::gdk::MemoryTexture::new(
        layout.width,
        layout.height,
        gtk::gdk::MemoryFormat::R8g8b8a8,
        &bytes,
        layout.stride,
    )
    .upcast())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TextureLayout {
    width: i32,
    height: i32,
    stride: usize,
}

fn raw_texture_layout(surface: &RasterSurface) -> Result<TextureLayout, String> {
    let width = i32::try_from(surface.width())
        .map_err(|_| "Preview width exceeds GTK limits.".to_owned())?;
    let height = i32::try_from(surface.height())
        .map_err(|_| "Preview height exceeds GTK limits.".to_owned())?;
    let stride = usize::try_from(surface.width())
        .map_err(|_| "Preview width exceeds addressable memory.".to_owned())?
        .checked_mul(4)
        .ok_or_else(|| "Preview stride overflowed.".to_owned())?;
    if surface.pixels().len()
        != stride
            * usize::try_from(surface.height())
                .map_err(|_| "Preview height exceeds addressable memory.".to_owned())?
    {
        return Err("Preview raster bytes do not match dimensions.".to_owned());
    }
    Ok(TextureLayout {
        width,
        height,
        stride,
    })
}

const fn is_current_generation(current: u64, candidate: u64) -> bool {
    current == candidate
}

fn clear_preview(state: &mut AppState) {
    state.picture.set_paintable(None::<&gtk::gdk::Paintable>);
    state.preview = None;
}

fn update_backdrop(state: &mut AppState) {
    for model in PreviewModel::ALL {
        state.viewer.remove_css_class(model.css_class());
    }
    state.viewer.add_css_class(state.model.css_class());
}

fn show_svg_diagnostic(state: &mut AppState) {
    let diagnostic = state
        .source
        .as_ref()
        .and_then(|source| source.identity.svg_text.as_ref());
    if should_reveal_svg_live_text(diagnostic) {
        let diagnostic = diagnostic.expect("live-text banner requires diagnostic");
        state.banner.set_title(&format!(
            "SVG live text detected; {}.",
            diagnostic.font_policy
        ));
        state.banner.set_revealed(true);
    } else {
        state.banner.set_revealed(false);
    }
}

fn should_reveal_svg_live_text(diagnostic: Option<&toniator_engine::SvgTextDiagnostic>) -> bool {
    diagnostic.is_some_and(|diagnostic| diagnostic.has_live_text_node)
}

fn set_page(state: &mut AppState, page: Page) {
    state.presentation.set_page(page);
    state.stack.set_visible_child_name(page.name());
    state.selector.set_sensitive(!matches!(page, Page::Loading));
}

fn update_source_title(state: &mut AppState) {
    match state.presentation.title.as_deref() {
        Some(title) => {
            state.window_title.set_title(title);
            state.window.set_title(Some(&format!("{title} — Toniator")));
        }
        None => {
            state.window_title.set_title("Toniator");
            state.window.set_title(Some("Toniator"));
        }
    }
}

fn show_error(state: &mut AppState, message: String) {
    clear_preview(state);
    state.error.set_label(&message);
    state.banner.set_title(&message);
    state.banner.set_revealed(true);
    set_page(state, Page::Error);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_an_optional_path() {
        assert_eq!(parse_args(vec![]).unwrap(), None);
        assert_eq!(
            parse_args(vec!["art.png".into()]).unwrap(),
            Some(PathBuf::from("art.png"))
        );
        assert!(parse_args(vec!["--canvas".into(), "900x600".into()]).is_err());
        assert!(parse_args(vec!["one.png".into(), "two.svg".into()]).is_err());
    }

    #[test]
    fn template_uses_intrinsic_canvas_and_all_canonical_topologies() {
        let source = LoadedSource {
            bytes: Arc::from(Vec::<u8>::new()),
            format: SourceFormatHint::Png,
            identity: SourceIdentity {
                format: toniator_engine::SourceFormat::Png,
                width: 1024,
                height: 620,
                content_hash: "identity".to_owned(),
                decoded_pixel_hash: "pixels".to_owned(),
                svg_text: None,
            },
            reference_id: SourceReferenceId::new("test-source").unwrap(),
            display_name: "intrinsic.png".to_owned(),
        };
        for model in PreviewModel::ALL {
            let session = document_session_for(&source, model).unwrap();
            assert_eq!(
                session.document().canvas(),
                &CanvasSpec {
                    width: 1024.0,
                    height: 620.0
                }
            );
            assert_eq!(session.document().channel_model(), Some(model.domain()));
            let definition = &session.document().pattern_definitions()[0];
            assert_eq!(definition.guard_steps, 2);
            assert_eq!(definition.maximum_support_radius, 4.5);
            for channel in session.document().channel_topology().unwrap().channels() {
                assert_eq!(channel.layout.density.across_x, 102.4);
                assert_eq!(channel.layout.density.across_y, 62.0);
                assert!(channel.layout.density.aspect_locked);
                assert_eq!(channel.layout.rotation_degrees, 0.0);
                assert_eq!(channel.layout.translation_x, 0.0);
                assert_eq!(channel.layout.translation_y, 0.0);
                assert_eq!(channel.mark_geometry_response.minimum_size, 2.0);
                assert_eq!(channel.mark_geometry_response.maximum_size, 9.0);
                assert_eq!(channel.opacity, 1.0);
            }
        }
    }

    #[test]
    fn raw_texture_layout_is_exact_and_does_not_transform_bytes() {
        let pixels = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let surface = RasterSurface::new(2, 1, pixels.clone()).unwrap();
        assert_eq!(
            raw_texture_layout(&surface).unwrap(),
            TextureLayout {
                width: 2,
                height: 1,
                stride: 8
            }
        );
        assert_eq!(surface.pixels(), pixels);
    }

    #[test]
    fn presentation_states_and_loader_generation_gate_protect_visible_title() {
        let mut presentation = PresentationState::empty();
        assert_eq!(presentation.page, Page::Empty);
        presentation.set_page(Page::Loading);
        assert!(presentation.accept_source(1, 1, "first.png".to_owned()));
        assert_eq!(presentation.title.as_deref(), Some("first.png"));
        presentation.set_page(Page::Loading);
        assert!(!presentation.accept_source(2, 1, "stale.svg".to_owned()));
        assert_eq!(presentation.title.as_deref(), Some("first.png"));
        presentation.set_page(Page::Error);
        assert_eq!(presentation.page, Page::Error);
        assert!(presentation.accept_source(2, 2, "second.svg".to_owned()));
        presentation.set_page(Page::Success);
        assert_eq!(presentation.page, Page::Success);
        assert_eq!(presentation.title.as_deref(), Some("second.svg"));
    }

    #[test]
    fn replacement_load_reset_clears_source_metadata_before_gating_new_loader() {
        let mut presentation = PresentationState::empty();
        assert!(presentation.accept_source(1, 1, "previous.svg".to_owned()));
        presentation.reset_source_metadata();
        presentation.set_page(Page::Loading);
        assert_eq!(presentation.accepted_generation, None);
        assert_eq!(presentation.title, None);
        assert!(!presentation.accept_source(2, 1, "stale.svg".to_owned()));
        assert_eq!(presentation.title, None);
    }

    #[test]
    fn svg_banner_reveals_only_for_live_text() {
        let live = toniator_engine::SvgTextDiagnostic {
            has_live_text_node: true,
            font_policy: "system-fonts".to_owned(),
            rendered_glyph_coverage: true,
        };
        let no_live_text = toniator_engine::SvgTextDiagnostic {
            has_live_text_node: false,
            font_policy: "system-fonts".to_owned(),
            rendered_glyph_coverage: false,
        };
        assert!(should_reveal_svg_live_text(Some(&live)));
        assert!(!should_reveal_svg_live_text(Some(&no_live_text)));
        assert!(!should_reveal_svg_live_text(None));
    }

    #[test]
    fn local_load_failures_have_stable_schema_style_prefixes() {
        assert_eq!(
            format_hint_for_path(Path::new("unsupported.jpg")).unwrap_err(),
            "source.format: only PNG and SVG source formats are supported"
        );
        let error =
            load_source(Path::new("target/validation/stage-10/missing-source.png")).unwrap_err();
        assert!(error.starts_with("source.read: could not read "));
    }

    #[test]
    fn viewport_target_uses_allocation_device_scale_and_checked_bounds() {
        assert_eq!(
            preview_target_from_allocation(960, 540, 2)
                .map(|value| (value.width(), value.height())),
            Some((1920, 1080))
        );
        assert_eq!(
            preview_target_from_allocation(300, 900, 1)
                .map(|value| (value.width(), value.height())),
            Some((300, 900))
        );
        assert!(preview_target_from_allocation(0, 900, 1).is_none());
        assert!(preview_target_from_allocation(20_000, 20_000, 1).is_none());
    }

    #[test]
    fn authority_change_invalidates_same_viewport_target_but_repeat_coalesces() {
        let target = toniator_engine::PreviewRasterTarget::new(512, 512).unwrap();
        let mut submitted = Some(target);
        assert_eq!(submitted, Some(target));
        submitted = None; // ReplaceChannelTopology authority change.
        assert_ne!(submitted, Some(target));
        submitted = Some(target);
        assert_eq!(submitted, Some(target)); // Same token/target coalesces.
    }

    #[test]
    fn loading_stack_allocation_waits_then_submits_once_and_coalesces() {
        assert!(preview_target_from_allocation(0, 720, 1).is_none());
        let target = preview_target_from_allocation(960, 720, 1).unwrap();
        assert!(should_submit_target(None, target));
        assert!(!should_submit_target(Some(target), target));
    }

    #[test]
    fn model_replacement_advances_the_authoritative_revision_and_invalidates_old_ticket() {
        let source = LoadedSource {
            bytes: Arc::from(Vec::<u8>::new()),
            format: SourceFormatHint::Png,
            identity: SourceIdentity {
                format: toniator_engine::SourceFormat::Png,
                width: 20,
                height: 10,
                content_hash: "identity".to_owned(),
                decoded_pixel_hash: "pixels".to_owned(),
                svg_text: None,
            },
            reference_id: SourceReferenceId::new("test-source").unwrap(),
            display_name: "model.png".to_owned(),
        };
        let mut session = document_session_for(&source, PreviewModel::Rgb).unwrap();
        let old = session.document_evaluation_token();
        replace_model_topology(&mut session, PreviewModel::Cmyk).unwrap();
        assert_eq!(
            session.document().channel_model(),
            Some(HalftoneChannelModel::Cmyk)
        );
        assert_eq!(
            session
                .document()
                .channel_topology()
                .unwrap()
                .channels()
                .len(),
            4
        );
        assert!(!session.accepts_document_evaluation(old));
    }
}
