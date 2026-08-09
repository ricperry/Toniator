#![forbid(unsafe_code)]

//! GTK document lifecycle around the headless document, history, engine, and
//! portable-container boundaries.  The workspace below is controller state;
//! `DocumentHistory` remains the only mutable document authority.

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
    CanvasSpec, ChannelTopologyTemplate, Document, DocumentCommand, DocumentHistory,
    DocumentSession, HalftoneChannelModel, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    EvaluationLimits, EvaluationRequest, EvaluationScheduler, OutputRasterTarget,
    RasterAntialiasing, RasterBackground, RasterSurface, ResolvedSource, SourceFormatHint,
    SourceIdentity, encode_png, evaluate_with_limits, rasterize_output, resolve_source_identity,
    write_svg,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load as load_container,
    save as save_container,
};

const APP_ID: &str = "com.silentbutdigital.Toniator";
const RESOURCE_PREFIX: &str = "/com/silentbutdigital/Toniator";
const DEFAULT_CANVAS: CanvasSpec = CanvasSpec {
    width: 1024.0,
    height: 1024.0,
};
const OPEN_FILTER_LABELS: [&str; 3] = [
    "Toniator documents (.toniator)",
    "PNG artwork",
    "SVG artwork",
];
const SAVE_FILTER_LABEL: &str = "Toniator documents (.toniator)";
const EXPORT_FILTER_LABELS: [&str; 2] = ["PNG image", "SVG vector image"];
const LIFECYCLE_BUTTONS: [(&str, &str, &str); 6] = [
    ("_New", "app.new", "New document (Ctrl+N)"),
    ("_Open", "app.open", "Open a document or artwork (Ctrl+O)"),
    ("_Save", "app.save", "Save document (Ctrl+S)"),
    ("Save _As", "app.save-as", "Save document as (Ctrl+Shift+S)"),
    ("_Export", "app.export", "Export PNG or SVG (Ctrl+E)"),
    ("_Close", "app.close", "Close document (Ctrl+W)"),
];

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

    const fn from_domain(model: HalftoneChannelModel) -> Self {
        match model {
            HalftoneChannelModel::Rgb => Self::Rgb,
            HalftoneChannelModel::Cmyk => Self::Cmyk,
            HalftoneChannelModel::SourceColorAlpha => Self::SourceColorAlpha,
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

#[derive(Clone, Debug, PartialEq)]
struct SavedContent {
    document: Document,
    sources: SourceBundle,
}

#[derive(Clone, Debug)]
struct SourcePresentation {
    id: SourceReferenceId,
    format: SourceFormatHint,
    identity: SourceIdentity,
}

/// Private lifecycle state.  It holds no independently mutable document:
/// every change to `document()` goes through `history`.
#[derive(Debug)]
struct Workspace {
    history: DocumentHistory,
    sources: SourceBundle,
    location: Option<PathBuf>,
    display_name: String,
    source_presentation: Option<SourcePresentation>,
    migration_notice: bool,
    savepoint: Option<SavedContent>,
}

impl Workspace {
    fn from_new() -> Result<Self, String> {
        let document = Document::new_default_document(DEFAULT_CANVAS, SourceReference::Unassigned)
            .map_err(|error| error.to_string())?;
        let sources = SourceBundle::new([]).map_err(|error| error.to_string())?;
        let savepoint = Some(SavedContent {
            document: document.clone(),
            sources: sources.clone(),
        });
        Ok(Self {
            history: fresh_history(document)?,
            sources,
            location: None,
            display_name: "Untitled".to_owned(),
            source_presentation: None,
            migration_notice: false,
            savepoint,
        })
    }

    fn from_direct(
        bytes: Arc<[u8]>,
        format: SourceFormatHint,
        display_name: String,
    ) -> Result<Self, String> {
        let identity =
            resolve_source_identity(&bytes, format).map_err(|error| error.to_string())?;
        let id = source_id_from_content(&identity)?;
        let embedded_format = embedded_format(format)?;
        let source = EmbeddedSource::new(
            id.clone(),
            embedded_format,
            bytes,
            Some(display_name.clone()),
        )
        .map_err(|error| error.to_string())?;
        let sources = SourceBundle::new([source]).map_err(|error| error.to_string())?;
        let document = Document::new_default_document(
            CanvasSpec {
                width: f64::from(identity.width),
                height: f64::from(identity.height),
            },
            SourceReference::Assigned(id.clone()),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            history: fresh_history(document)?,
            sources,
            location: None,
            display_name,
            source_presentation: Some(SourcePresentation {
                id,
                format,
                identity,
            }),
            migration_notice: false,
            // Direct artwork has not been accepted as persisted document
            // content, even though it has an in-memory source bundle.
            savepoint: None,
        })
    }

    fn from_container(path: &Path) -> Result<Self, String> {
        // This is deliberately the only container-opening call site.
        let loaded = load_container(path).map_err(|error| error.to_string())?;
        let document = loaded.document().clone();
        let sources = loaded.sources().clone();
        let source = match document.source() {
            SourceReference::Assigned(id) => sources
                .get(id)
                .ok_or_else(|| "source.document: loaded source is absent".to_owned())?,
            SourceReference::Unassigned => {
                return Err("source.document: a .toniator document must have a source".to_owned());
            }
        };
        let format = source_format_hint(source.format());
        let identity =
            resolve_source_identity(source.bytes(), format).map_err(|error| error.to_string())?;
        let display_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled.toniator")
            .to_owned();
        let migration_notice = !loaded.migration_report().is_empty();
        Ok(Self {
            history: fresh_history(document.clone())?,
            sources: sources.clone(),
            location: Some(path.to_owned()),
            display_name,
            source_presentation: Some(SourcePresentation {
                id: source.id().clone(),
                format,
                identity,
            }),
            migration_notice,
            savepoint: Some(SavedContent { document, sources }),
        })
    }

    fn document(&self) -> &Document {
        self.history.document()
    }

    fn is_dirty(&self) -> bool {
        self.savepoint
            .as_ref()
            .is_none_or(|saved| saved.document != *self.document() || saved.sources != self.sources)
    }

    fn can_save(&self) -> bool {
        self.source_presentation.is_some() && !self.sources.is_empty()
    }

    fn snapshot(&self) -> SavedContent {
        SavedContent {
            document: self.document().clone(),
            sources: self.sources.clone(),
        }
    }

    fn accept_saved_snapshot(&mut self, path: PathBuf, snapshot: SavedContent) {
        self.location = Some(path.clone());
        self.display_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled.toniator")
            .to_owned();
        self.savepoint = Some(snapshot);
    }

    fn title(&self) -> String {
        let marker = if self.is_dirty() { "*" } else { "" };
        format!("{}{marker} — Toniator", self.display_name)
    }
}

fn fresh_history(document: Document) -> Result<DocumentHistory, String> {
    DocumentSession::new(document)
        .map(DocumentHistory::new)
        .map_err(|error| error.to_string())
}

fn source_id_from_content(identity: &SourceIdentity) -> Result<SourceReferenceId, String> {
    // Decoder hashes are namespaced (`sha256:<hex>`); the archive component
    // deliberately retains only the deterministic digest portion.
    let digest = identity
        .content_hash
        .strip_prefix("sha256:")
        .unwrap_or(&identity.content_hash);
    SourceReferenceId::new(format!("source-{digest}")).map_err(|error| error.to_string())
}

fn embedded_format(format: SourceFormatHint) -> Result<EmbeddedSourceFormat, String> {
    match format {
        SourceFormatHint::Png => Ok(EmbeddedSourceFormat::Png),
        SourceFormatHint::Svg => Ok(EmbeddedSourceFormat::Svg),
        SourceFormatHint::Unsupported => {
            Err("source.format: only PNG and SVG source formats are supported".to_owned())
        }
    }
}

const fn source_format_hint(format: EmbeddedSourceFormat) -> SourceFormatHint {
    match format {
        EmbeddedSourceFormat::Png => SourceFormatHint::Png,
        EmbeddedSourceFormat::Svg => SourceFormatHint::Svg,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Empty,
    Loading,
    Error,
    Success,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LifecycleAction {
    New,
    Open,
    Close,
    WindowClose,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct WindowCloseController {
    requested: bool,
    deferred: bool,
    allowed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowCloseRequest {
    Dispatch,
    Ignore,
    Proceed,
}

impl WindowCloseController {
    fn request(&mut self) -> WindowCloseRequest {
        if self.allowed {
            WindowCloseRequest::Proceed
        } else if self.requested {
            WindowCloseRequest::Ignore
        } else {
            self.requested = true;
            WindowCloseRequest::Dispatch
        }
    }

    fn cancel(&mut self) {
        if !self.allowed {
            self.requested = false;
            self.deferred = false;
        }
    }

    fn defer(&mut self) -> bool {
        if self.requested && !self.deferred && !self.allowed {
            self.deferred = true;
            true
        } else {
            false
        }
    }

    fn accept_deferred(&mut self) -> bool {
        if self.requested && self.deferred && !self.allowed {
            self.deferred = false;
            self.allowed = true;
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnsavedDecision {
    Cancel,
    Discard,
    Save,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LifecycleDisposition {
    Execute(LifecycleAction),
    Prompt(LifecycleAction),
    SaveThen(LifecycleAction),
    Noop,
}

fn begin_lifecycle(dirty: bool, action: LifecycleAction) -> LifecycleDisposition {
    if dirty {
        LifecycleDisposition::Prompt(action)
    } else {
        LifecycleDisposition::Execute(action)
    }
}

fn resolve_unsaved_decision(
    action: LifecycleAction,
    decision: UnsavedDecision,
) -> LifecycleDisposition {
    match decision {
        UnsavedDecision::Cancel => LifecycleDisposition::Noop,
        UnsavedDecision::Discard => LifecycleDisposition::Execute(action),
        UnsavedDecision::Save => LifecycleDisposition::SaveThen(action),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UiPolicy {
    new_enabled: bool,
    open_enabled: bool,
    close_enabled: bool,
    save_enabled: bool,
    save_as_enabled: bool,
    export_enabled: bool,
    selector_enabled: bool,
    title: String,
}

#[derive(Debug, PartialEq, Eq)]
enum SaveRoute {
    Unavailable,
    Existing(PathBuf),
    SaveAs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExportFormat {
    Png,
    Svg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExportSettings {
    format: ExportFormat,
    background: RasterBackground,
    antialiasing: RasterAntialiasing,
    output_target: Option<OutputRasterTarget>,
}

fn save_route(workspace: Option<&Workspace>) -> SaveRoute {
    match workspace {
        Some(workspace) if workspace.can_save() => workspace
            .location
            .clone()
            .map_or(SaveRoute::SaveAs, SaveRoute::Existing),
        _ => SaveRoute::Unavailable,
    }
}

fn ui_policy(
    workspace: Option<&Workspace>,
    loading: bool,
    saving: bool,
    exporting: bool,
) -> UiPolicy {
    let busy = loading || saving || exporting;
    let can_save = workspace.is_some_and(Workspace::can_save) && !busy;
    UiPolicy {
        new_enabled: !busy,
        open_enabled: !busy,
        close_enabled: workspace.is_some() && !busy,
        save_enabled: can_save,
        save_as_enabled: can_save,
        export_enabled: can_save,
        selector_enabled: !loading
            && !exporting
            && workspace.is_some_and(|workspace| workspace.source_presentation.is_some()),
        title: workspace.map_or_else(|| "Toniator".to_owned(), Workspace::title),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BannerPolicy {
    Hidden,
    Message(String),
}

fn banner_policy(
    migration_notice: bool,
    live_text: Option<&toniator_engine::SvgTextDiagnostic>,
    error: Option<&str>,
) -> BannerPolicy {
    if let Some(error) = error {
        return BannerPolicy::Message(error.to_owned());
    }
    if migration_notice {
        return BannerPolicy::Message("Document migration information is available.".to_owned());
    }
    if let Some(diagnostic) = live_text.filter(|diagnostic| diagnostic.has_live_text_node) {
        return BannerPolicy::Message(format!(
            "SVG live text detected; {}.",
            diagnostic.font_policy
        ));
    }
    BannerPolicy::Hidden
}

fn apply_banner_policy(banner: &adw::Banner, policy: BannerPolicy) {
    match policy {
        BannerPolicy::Hidden => banner.set_revealed(false),
        BannerPolicy::Message(message) => {
            banner.set_title(&message);
            banner.set_revealed(true);
        }
    }
}

struct PendingLoad {
    generation: u64,
    receiver: Receiver<Result<Workspace, String>>,
}

struct PendingSave {
    generation: u64,
    path: PathBuf,
    snapshot: SavedContent,
    receiver: Receiver<Result<(), String>>,
    after: Option<LifecycleAction>,
}

struct PendingExport {
    generation: u64,
    workspace_generation: u64,
    receiver: Receiver<Result<(), String>>,
}

/// App-owned lifecycle identity paired with an engine scheduler ticket.  The
/// scheduler remains authoritative for ticket/revision/cache acceptance; this
/// merely prevents two distinct revision-zero workspaces from looking equal to
/// GTK presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PreviewSubmission {
    workspace_generation: u64,
    ticket: u64,
}

enum CompletionInstall<T> {
    Install(T),
    Preserve,
}

fn lifecycle_completion_policy<T>(
    current_generation: u64,
    candidate_generation: u64,
    candidate: T,
) -> CompletionInstall<T> {
    if is_current_generation(current_generation, candidate_generation) {
        CompletionInstall::Install(candidate)
    } else {
        CompletionInstall::Preserve
    }
}

fn accepts_preview_submission(
    current_workspace_generation: u64,
    submission: Option<PreviewSubmission>,
    completion_ticket: u64,
) -> bool {
    submission.is_some_and(|submission| {
        submission.workspace_generation == current_workspace_generation
            && submission.ticket == completion_ticket
    })
}

#[derive(Clone)]
struct Actions {
    new: gio::SimpleAction,
    open: gio::SimpleAction,
    save: gio::SimpleAction,
    save_as: gio::SimpleAction,
    export: gio::SimpleAction,
    close: gio::SimpleAction,
}

struct AppState {
    scheduler: EvaluationScheduler,
    workspace: Option<Workspace>,
    pending_load: Option<PendingLoad>,
    pending_save: Option<PendingSave>,
    pending_export: Option<PendingExport>,
    generation: u64,
    workspace_generation: u64,
    preview_submission: Option<PreviewSubmission>,
    model: PreviewModel,
    syncing_model: bool,
    window_close: WindowCloseController,
    actions: Actions,
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
    for (label, action, tooltip) in LIFECYCLE_BUTTONS {
        let button = gtk::Button::with_mnemonic(label);
        button.set_action_name(Some(action));
        button.set_tooltip_text(Some(tooltip));
        header.pack_start(&button);
    }
    let model_label = gtk::Label::new(Some("_Model"));
    model_label.set_use_underline(true);
    model_label.add_css_class("dim-label");
    let selector = gtk::DropDown::from_strings(&PreviewModel::ALL.map(PreviewModel::label));
    selector.set_tooltip_text(Some("Authoritative channel model"));
    model_label.set_mnemonic_widget(Some(&selector));
    header.pack_end(&selector);
    header.pack_end(&model_label);
    let window_title = adw::WindowTitle::new("Toniator", "Document lifecycle");
    header.set_title_widget(Some(&window_title));

    let banner = adw::Banner::new("");
    banner.set_revealed(false);
    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.add_named(
        &status_page("Create or open a Toniator document."),
        Some(Page::Empty.name()),
    );
    stack.add_named(&status_page("Working…"), Some(Page::Loading.name()));
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

    let actions = Actions {
        new: gio::SimpleAction::new("new", None),
        open: gio::SimpleAction::new("open", None),
        save: gio::SimpleAction::new("save", None),
        save_as: gio::SimpleAction::new("save-as", None),
        export: gio::SimpleAction::new("export", None),
        close: gio::SimpleAction::new("close", None),
    };
    for action in [
        &actions.new,
        &actions.open,
        &actions.save,
        &actions.save_as,
        &actions.export,
        &actions.close,
    ] {
        app.add_action(action);
    }
    app.set_accels_for_action("app.new", &["<Primary>n"]);
    app.set_accels_for_action("app.open", &["<Primary>o"]);
    app.set_accels_for_action("app.save", &["<Primary>s"]);
    app.set_accels_for_action("app.save-as", &["<Primary><Shift>s"]);
    app.set_accels_for_action("app.export", &["<Primary>e"]);
    app.set_accels_for_action("app.close", &["<Primary>w"]);
    let state = Rc::new(RefCell::new(AppState {
        scheduler: EvaluationScheduler::new().expect("failed to start evaluation scheduler"),
        workspace: None,
        pending_load: None,
        pending_save: None,
        pending_export: None,
        generation: 0,
        workspace_generation: 0,
        preview_submission: None,
        model: PreviewModel::Rgb,
        syncing_model: false,
        window_close: WindowCloseController::default(),
        actions,
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
    connect_actions(&state);
    {
        let state = Rc::clone(&state);
        let selector = state.borrow().selector.clone();
        selector.connect_selected_notify(move |selector| {
            let model = PreviewModel::ALL.get(selector.selected() as usize).copied();
            if let Some(model) = model {
                change_model(&state, model);
            }
        });
    }
    {
        let state = Rc::clone(&state);
        window.connect_close_request(move |_| {
            let request = {
                let mut state = state.borrow_mut();
                let busy = lifecycle_is_busy(&state);
                request_window_close(&mut state.window_close, busy)
            };
            match request {
                WindowCloseRequest::Proceed => glib::Propagation::Proceed,
                WindowCloseRequest::Ignore => glib::Propagation::Stop,
                WindowCloseRequest::Dispatch => {
                    request_lifecycle(&state, LifecycleAction::WindowClose);
                    glib::Propagation::Stop
                }
            }
        });
    }
    {
        let state = Rc::clone(&state);
        glib::timeout_add_local(Duration::from_millis(16), move || poll(&state));
    }
    sync_ui(&mut state.borrow_mut());
    window.present();
    if let Some(path) = initial_path {
        start_load(&state, path);
    }
}

fn connect_actions(state: &Rc<RefCell<AppState>>) {
    for (action, lifecycle) in [
        (state.borrow().actions.new.clone(), LifecycleAction::New),
        (state.borrow().actions.open.clone(), LifecycleAction::Open),
        (state.borrow().actions.close.clone(), LifecycleAction::Close),
    ] {
        let state = Rc::clone(state);
        action.connect_activate(move |_, _| request_lifecycle(&state, lifecycle));
    }
    {
        let state = Rc::clone(state);
        let action = state.borrow().actions.save.clone();
        action.connect_activate(move |_, _| start_save(&state, None));
    }
    {
        let state = Rc::clone(state);
        let action = state.borrow().actions.save_as.clone();
        action.connect_activate(move |_, _| choose_save_as(&state, None));
    }
    {
        let state = Rc::clone(state);
        let action = state.borrow().actions.export.clone();
        action.connect_activate(move |_, _| choose_export(&state));
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

fn request_lifecycle(state: &Rc<RefCell<AppState>>, action: LifecycleAction) {
    if lifecycle_is_busy(&state.borrow()) {
        return;
    }
    let dirty = state
        .borrow()
        .workspace
        .as_ref()
        .is_some_and(Workspace::is_dirty);
    match begin_lifecycle(dirty, action) {
        LifecycleDisposition::Prompt(action) => choose_unsaved_resolution(state, action),
        LifecycleDisposition::Execute(action) => execute_lifecycle(state, action),
        LifecycleDisposition::SaveThen(_) | LifecycleDisposition::Noop => {
            unreachable!("only a request starts lifecycle routing")
        }
    }
}

fn choose_unsaved_resolution(state: &Rc<RefCell<AppState>>, action: LifecycleAction) {
    let dialog = gtk::AlertDialog::builder()
        .message("Save changes before continuing?")
        .detail("Your current document has unsaved changes.")
        .build();
    dialog.set_buttons(&["Cancel", "Discard", "Save"]);
    dialog.set_cancel_button(0);
    dialog.set_default_button(2);
    let state = Rc::clone(state);
    let window = state.borrow().window.clone();
    dialog.choose(Some(&window), None::<&gio::Cancellable>, move |response| {
        match resolve_unsaved_decision(
            action,
            match response {
                Ok(1) => UnsavedDecision::Discard,
                Ok(2) => UnsavedDecision::Save,
                _ => UnsavedDecision::Cancel,
            },
        ) {
            LifecycleDisposition::Execute(action) => execute_lifecycle(&state, action),
            LifecycleDisposition::SaveThen(action) => start_save(&state, Some(action)),
            LifecycleDisposition::Noop => {
                cancel_window_close_after(&mut state.borrow_mut(), Some(action));
            }
            LifecycleDisposition::Prompt(_) => unreachable!("decision resolution is terminal"),
        }
    });
}

fn execute_lifecycle(state: &Rc<RefCell<AppState>>, action: LifecycleAction) {
    match action {
        LifecycleAction::New => match Workspace::from_new() {
            Ok(workspace) => install_workspace(state, workspace),
            Err(error) => show_error(&mut state.borrow_mut(), error),
        },
        LifecycleAction::Open => choose_open(state),
        LifecycleAction::Close => clear_workspace(&mut state.borrow_mut()),
        LifecycleAction::WindowClose => defer_window_close(state),
    }
}

fn defer_window_close(state: &Rc<RefCell<AppState>>) {
    if !state.borrow_mut().window_close.defer() {
        return;
    }
    let state = Rc::clone(state);
    glib::idle_add_local_once(move || {
        let window = {
            let mut state = state.borrow_mut();
            if !state.window_close.accept_deferred() {
                return;
            }
            state.window.clone()
        };
        window.close();
    });
}

fn cancel_window_close_after(state: &mut AppState, after: Option<LifecycleAction>) {
    if after == Some(LifecycleAction::WindowClose) {
        state.window_close.cancel();
    }
}

fn lifecycle_is_busy(state: &AppState) -> bool {
    state.pending_load.is_some() || state.pending_save.is_some()
}

fn request_window_close(
    controller: &mut WindowCloseController,
    lifecycle_busy: bool,
) -> WindowCloseRequest {
    if lifecycle_busy {
        WindowCloseRequest::Ignore
    } else {
        controller.request()
    }
}

fn choose_open(state: &Rc<RefCell<AppState>>) {
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Open document or artwork");
    dialog.set_filters(Some(&open_filters()));
    let state = Rc::clone(state);
    let window = state.borrow().window.clone();
    dialog.open(Some(&window), None::<&gio::Cancellable>, move |result| {
        if let Ok(file) = result
            && let Some(path) = file.path()
        {
            start_load(&state, path);
        }
    });
}

fn open_filters() -> gio::ListStore {
    let documents = gtk::FileFilter::new();
    documents.set_name(Some(OPEN_FILTER_LABELS[0]));
    documents.add_pattern("*.toniator");
    let png = gtk::FileFilter::new();
    png.set_name(Some(OPEN_FILTER_LABELS[1]));
    png.add_mime_type("image/png");
    png.add_pattern("*.png");
    let svg = gtk::FileFilter::new();
    svg.set_name(Some(OPEN_FILTER_LABELS[2]));
    svg.add_mime_type("image/svg+xml");
    svg.add_pattern("*.svg");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&documents);
    filters.append(&png);
    filters.append(&svg);
    filters
}

fn choose_save_as(state: &Rc<RefCell<AppState>>, after: Option<LifecycleAction>) {
    if !state
        .borrow()
        .workspace
        .as_ref()
        .is_some_and(Workspace::can_save)
    {
        let mut state = state.borrow_mut();
        show_error(
            &mut state,
            "Save requires PNG or SVG source artwork.".to_owned(),
        );
        cancel_window_close_after(&mut state, after);
        return;
    }
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Save Toniator document");
    dialog.set_filters(Some(&save_filters()));
    let initial_name = suggested_filename(
        state
            .borrow()
            .workspace
            .as_ref()
            .expect("checked workspace"),
    );
    dialog.set_initial_name(Some(&initial_name));
    let state = Rc::clone(state);
    let window = state.borrow().window.clone();
    dialog.save(Some(&window), None::<&gio::Cancellable>, move |result| {
        if let Ok(file) = result
            && let Some(path) = file.path()
        {
            start_save_to(&state, with_toniator_extension(path), after);
        } else {
            cancel_window_close_after(&mut state.borrow_mut(), after);
        }
    });
}

fn save_filters() -> gio::ListStore {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some(SAVE_FILTER_LABEL));
    filter.add_pattern("*.toniator");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    filters
}

fn choose_export(state: &Rc<RefCell<AppState>>) {
    if !state
        .borrow()
        .workspace
        .as_ref()
        .is_some_and(Workspace::can_save)
    {
        show_error(
            &mut state.borrow_mut(),
            "Export requires an open source-backed document.".to_owned(),
        );
        return;
    }
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Export final consumer output");
    dialog.set_filters(Some(&export_filters()));
    let initial_name = suggested_export_filename(
        state
            .borrow()
            .workspace
            .as_ref()
            .expect("checked workspace"),
        ExportFormat::Png,
    );
    dialog.set_initial_name(Some(&initial_name));
    let state = Rc::clone(state);
    let window = state.borrow().window.clone();
    dialog.save(Some(&window), None::<&gio::Cancellable>, move |result| {
        let Ok(file) = result else { return };
        let Some(path) = file.path() else { return };
        let format = match export_format_for_path(&path) {
            Ok(format) => format,
            Err(error) => {
                show_error(&mut state.borrow_mut(), error);
                return;
            }
        };
        match format {
            ExportFormat::Png => choose_png_export_options(&state, path),
            ExportFormat::Svg => start_export(
                &state,
                path,
                ExportSettings {
                    format,
                    background: RasterBackground::Transparent,
                    antialiasing: RasterAntialiasing::On,
                    output_target: None,
                },
            ),
        }
    });
}

fn export_filters() -> gio::ListStore {
    let png = gtk::FileFilter::new();
    png.set_name(Some(EXPORT_FILTER_LABELS[0]));
    png.add_mime_type("image/png");
    png.add_pattern("*.png");
    let svg = gtk::FileFilter::new();
    svg.set_name(Some(EXPORT_FILTER_LABELS[1]));
    svg.add_mime_type("image/svg+xml");
    svg.add_pattern("*.svg");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&png);
    filters.append(&svg);
    filters
}

#[allow(deprecated)] // GTK 4.10's lightweight custom-content dialog remains available on Fedora.
fn choose_png_export_options(state: &Rc<RefCell<AppState>>, path: PathBuf) {
    let dialog = gtk::Dialog::builder()
        .title("PNG export options")
        .modal(true)
        .transient_for(&state.borrow().window)
        .build();
    dialog.add_button("_Cancel", gtk::ResponseType::Cancel);
    dialog.add_button("_Export", gtk::ResponseType::Accept);
    dialog.set_default_response(gtk::ResponseType::Accept);
    let content = dialog.content_area();
    content.set_spacing(12);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);
    let grid = gtk::Grid::builder()
        .row_spacing(12)
        .column_spacing(12)
        .build();
    let background_label = gtk::Label::new(Some("_Background"));
    background_label.set_use_underline(true);
    background_label.set_halign(gtk::Align::Start);
    let background = gtk::DropDown::from_strings(&["Transparent", "Black", "White"]);
    background.set_tooltip_text(Some("PNG-only final-consumer backing"));
    background_label.set_mnemonic_widget(Some(&background));
    let antialiasing_label = gtk::Label::new(Some("_Antialiasing"));
    antialiasing_label.set_use_underline(true);
    antialiasing_label.set_halign(gtk::Align::Start);
    let antialiasing = gtk::DropDown::from_strings(&["On", "Off"]);
    antialiasing.set_tooltip_text(Some("PNG edge rasterization"));
    antialiasing_label.set_mnemonic_widget(Some(&antialiasing));
    let dimensions_label = gtk::Label::new(Some("Output _size"));
    dimensions_label.set_use_underline(true);
    dimensions_label.set_halign(gtk::Align::Start);
    let dimensions = gtk::Entry::new();
    dimensions.set_placeholder_text(Some("Document canvas (for example 1200x800)"));
    dimensions.set_tooltip_text(Some("Optional PNG pixel dimensions"));
    dimensions_label.set_mnemonic_widget(Some(&dimensions));
    grid.attach(&background_label, 0, 0, 1, 1);
    grid.attach(&background, 1, 0, 1, 1);
    grid.attach(&antialiasing_label, 0, 1, 1, 1);
    grid.attach(&antialiasing, 1, 1, 1, 1);
    grid.attach(&dimensions_label, 0, 2, 1, 1);
    grid.attach(&dimensions, 1, 2, 1, 1);
    content.append(&grid);
    let state = Rc::clone(state);
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            let output_target = match parse_output_target(dimensions.text().as_str()) {
                Ok(target) => target,
                Err(error) => {
                    show_error(&mut state.borrow_mut(), error);
                    dialog.close();
                    return;
                }
            };
            let background = match background.selected() {
                1 => RasterBackground::OpaqueBlack,
                2 => RasterBackground::OpaqueWhite,
                _ => RasterBackground::Transparent,
            };
            let antialiasing = if antialiasing.selected() == 1 {
                RasterAntialiasing::Off
            } else {
                RasterAntialiasing::On
            };
            start_export(
                &state,
                path.clone(),
                ExportSettings {
                    format: ExportFormat::Png,
                    background,
                    antialiasing,
                    output_target,
                },
            );
        }
        dialog.close();
    });
    dialog.present();
}

fn suggested_export_filename(workspace: &Workspace, format: ExportFormat) -> String {
    let stem = workspace
        .display_name
        .strip_suffix(".toniator")
        .or_else(|| {
            workspace
                .display_name
                .rsplit_once('.')
                .map(|(stem, _)| stem)
        })
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Untitled");
    let extension = match format {
        ExportFormat::Png => "png",
        ExportFormat::Svg => "svg",
    };
    format!("{stem}.{extension}")
}

fn export_format_for_path(path: &Path) -> Result<ExportFormat, String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("png") => Ok(ExportFormat::Png),
        Some(extension) if extension.eq_ignore_ascii_case("svg") => Ok(ExportFormat::Svg),
        _ => Err("export.format: choose a .png or .svg filename".to_owned()),
    }
}

fn parse_output_target(value: &str) -> Result<Option<OutputRasterTarget>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let (width, height) = value
        .split_once('x')
        .or_else(|| value.split_once('X'))
        .ok_or_else(|| "export.dimensions: use WIDTHxHEIGHT".to_owned())?;
    let width = width
        .parse::<u32>()
        .map_err(|_| "export.dimensions: width must be a positive integer".to_owned())?;
    let height = height
        .parse::<u32>()
        .map_err(|_| "export.dimensions: height must be a positive integer".to_owned())?;
    OutputRasterTarget::new(width, height)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn suggested_filename(workspace: &Workspace) -> String {
    let stem = workspace
        .display_name
        .strip_suffix(".toniator")
        .or_else(|| {
            workspace
                .display_name
                .rsplit_once('.')
                .map(|(stem, _)| stem)
        })
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Untitled");
    format!("{stem}.toniator")
}

fn with_toniator_extension(mut path: PathBuf) -> PathBuf {
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toniator"))
    {
        path.set_extension("toniator");
    }
    path
}

fn start_load(state: &Rc<RefCell<AppState>>, path: PathBuf) {
    let (sender, receiver) = mpsc::channel();
    let generation = {
        let mut state = state.borrow_mut();
        state.generation = state.generation.saturating_add(1);
        let generation = state.generation;
        state.pending_load = Some(PendingLoad {
            generation,
            receiver,
        });
        if state.workspace.is_none() {
            set_page(&mut state, Page::Loading);
        }
        sync_ui(&mut state);
        generation
    };
    thread::spawn(move || {
        let candidate = load_workspace(&path);
        let _ = sender.send(candidate);
        let _ = generation;
    });
}

fn load_workspace(path: &Path) -> Result<Workspace, String> {
    if is_container_path(path) {
        return Workspace::from_container(path);
    }
    let format = format_hint_for_path(path)?;
    let bytes: Arc<[u8]> = fs::read(path)
        .map(Arc::<[u8]>::from)
        .map_err(|error| format!("source.read: could not read {}: {error}", path.display()))?;
    let display_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Untitled source")
        .to_owned();
    Workspace::from_direct(bytes, format, display_name)
}

fn is_container_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toniator"))
}

fn format_hint_for_path(path: &Path) -> Result<SourceFormatHint, String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("png") => Ok(SourceFormatHint::Png),
        Some(extension) if extension.eq_ignore_ascii_case("svg") => Ok(SourceFormatHint::Svg),
        _ => Err("source.format: supported inputs are .toniator, PNG, and SVG".to_owned()),
    }
}

fn start_save(state: &Rc<RefCell<AppState>>, after: Option<LifecycleAction>) {
    let route = { save_route(state.borrow().workspace.as_ref()) };
    match route {
        SaveRoute::Unavailable => {
            let mut state = state.borrow_mut();
            show_error(
                &mut state,
                "Save requires PNG or SVG source artwork.".to_owned(),
            );
            cancel_window_close_after(&mut state, after);
        }
        SaveRoute::Existing(path) => start_save_to(state, path, after),
        SaveRoute::SaveAs => choose_save_as(state, after),
    }
}

fn start_save_to(state: &Rc<RefCell<AppState>>, path: PathBuf, after: Option<LifecycleAction>) {
    {
        let mut state = state.borrow_mut();
        let Some(workspace) = state.workspace.as_ref() else {
            cancel_window_close_after(&mut state, after);
            return;
        };
        let snapshot = workspace.snapshot();
        let generation = state.generation;
        let (sender, receiver) = mpsc::channel();
        let document = snapshot.document.clone();
        let sources = snapshot.sources.clone();
        let save_path = path.clone();
        thread::spawn(move || {
            let result =
                save_container(&save_path, &document, &sources).map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
        state.pending_save = Some(PendingSave {
            generation,
            path,
            snapshot,
            receiver,
            after,
        });
        sync_ui(&mut state);
    }
}

fn start_export(state: &Rc<RefCell<AppState>>, path: PathBuf, settings: ExportSettings) {
    let (snapshot, presentation, generation, workspace_generation) = {
        let mut state = state.borrow_mut();
        if state.pending_export.is_some() {
            return;
        }
        let Some(workspace) = state.workspace.as_ref() else {
            return;
        };
        let Some(presentation) = workspace.source_presentation.clone() else {
            show_error(&mut state, "Export requires an active source.".to_owned());
            return;
        };
        (
            workspace.snapshot(),
            presentation,
            state.generation,
            state.workspace_generation,
        )
    };
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(export_snapshot(snapshot, presentation, path, settings));
    });
    let mut state = state.borrow_mut();
    state.pending_export = Some(PendingExport {
        generation,
        workspace_generation,
        receiver,
    });
    sync_ui(&mut state);
}

fn export_snapshot(
    snapshot: SavedContent,
    presentation: SourcePresentation,
    path: PathBuf,
    settings: ExportSettings,
) -> Result<(), String> {
    let source = snapshot
        .sources
        .get(&presentation.id)
        .ok_or_else(|| "source.document: source bundle is missing the active source".to_owned())?;
    let session = DocumentSession::new(snapshot.document).map_err(|error| error.to_string())?;
    let result = evaluate_with_limits(
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                presentation.id,
                Arc::<[u8]>::from(source.bytes()),
                presentation.format,
            )
            .map_err(|error| error.to_string())?,
        ),
        EvaluationLimits::default(),
    )
    .map_err(|error| error.to_string())?;
    match settings.format {
        ExportFormat::Png => {
            let surface = rasterize_output(
                result.scene(),
                settings.background,
                settings.output_target,
                settings.antialiasing,
            )
            .map_err(|error| error.to_string())?;
            fs::write(
                &path,
                encode_png(&surface).map_err(|error| error.to_string())?,
            )
            .map_err(|error| format!("output.write: could not write {}: {error}", path.display()))
        }
        ExportFormat::Svg => fs::write(&path, write_svg(result.scene()))
            .map_err(|error| format!("output.write: could not write {}: {error}", path.display())),
    }
}

fn poll(state: &Rc<RefCell<AppState>>) -> glib::ControlFlow {
    poll_load(state);
    poll_save(state);
    poll_export(state);
    {
        let mut state = state.borrow_mut();
        if state
            .workspace
            .as_ref()
            .is_some_and(|workspace| workspace.source_presentation.is_some())
        {
            submit_if_viewport_ready(&mut state);
        }
    }
    let completion = state.borrow().scheduler.try_receive_latest();
    match completion {
        Ok(Some(completion)) => {
            let mut state = state.borrow_mut();
            let accepted = if accepts_preview_submission(
                state.workspace_generation,
                state.preview_submission,
                completion.ticket().value(),
            ) {
                match state.workspace.as_ref() {
                    Some(workspace) => state
                        .scheduler
                        .accept_completion(&completion, workspace.history.session()),
                    None => Ok(false),
                }
            } else {
                Ok(false)
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

fn poll_export(state: &Rc<RefCell<AppState>>) {
    let pending = state.borrow_mut().pending_export.take();
    let Some(pending) = pending else { return };
    match pending.receiver.try_recv() {
        Ok(result) => {
            let mut state = state.borrow_mut();
            if is_current_generation(state.generation, pending.generation)
                && is_current_generation(state.workspace_generation, pending.workspace_generation)
                && let Err(error) = result
            {
                show_error(&mut state, format!("export.failed: {error}"));
            }
            sync_ui(&mut state);
        }
        Err(TryRecvError::Empty) => state.borrow_mut().pending_export = Some(pending),
        Err(TryRecvError::Disconnected) => {
            let mut state = state.borrow_mut();
            if is_current_generation(state.generation, pending.generation)
                && is_current_generation(state.workspace_generation, pending.workspace_generation)
            {
                show_error(&mut state, "Export stopped unexpectedly.".to_owned());
            }
            sync_ui(&mut state);
        }
    }
}

fn poll_load(state: &Rc<RefCell<AppState>>) {
    let pending = state.borrow_mut().pending_load.take();
    let Some(pending) = pending else { return };
    match pending.receiver.try_recv() {
        Ok(result) => {
            let completion =
                lifecycle_completion_policy(state.borrow().generation, pending.generation, result);
            if let CompletionInstall::Install(result) = completion {
                match result {
                    Ok(workspace) => install_workspace(state, workspace),
                    Err(error) => {
                        let mut state = state.borrow_mut();
                        show_error(&mut state, error);
                        sync_ui(&mut state);
                    }
                }
            } else {
                sync_ui(&mut state.borrow_mut());
            }
        }
        Err(TryRecvError::Empty) => state.borrow_mut().pending_load = Some(pending),
        Err(TryRecvError::Disconnected) => {
            let mut state = state.borrow_mut();
            if state.generation == pending.generation {
                show_error(
                    &mut state,
                    "Document loader stopped unexpectedly.".to_owned(),
                );
            }
            sync_ui(&mut state);
        }
    }
}

fn poll_save(state: &Rc<RefCell<AppState>>) {
    let pending = state.borrow_mut().pending_save.take();
    let Some(pending) = pending else { return };
    match pending.receiver.try_recv() {
        Ok(result) => {
            let after_action = {
                let mut app_state = state.borrow_mut();
                let mut after_action = None;
                if let CompletionInstall::Install(result) =
                    lifecycle_completion_policy(app_state.generation, pending.generation, result)
                {
                    match result {
                        Ok(()) => {
                            if let Some(workspace) = app_state.workspace.as_mut() {
                                // This is the exact snapshot accepted by atomic IO;
                                // later history changes remain dirty by comparison.
                                workspace
                                    .accept_saved_snapshot(pending.path.clone(), pending.snapshot);
                            }
                            sync_ui(&mut app_state);
                            after_action = pending.after;
                        }
                        Err(error) => {
                            show_error(&mut app_state, format!("save.failed: {error}"));
                            cancel_window_close_after(&mut app_state, pending.after);
                            sync_ui(&mut app_state);
                        }
                    }
                } else {
                    cancel_window_close_after(&mut app_state, pending.after);
                    sync_ui(&mut app_state);
                }
                after_action
            };
            if let Some(action) = after_action {
                // The snapshot may have become an old savepoint while the
                // user changed model/content during asynchronous saving.
                // Re-enter the one unsaved-work boundary instead of discarding
                // that newer authoritative content.
                request_lifecycle(state, action);
            }
        }
        Err(TryRecvError::Empty) => state.borrow_mut().pending_save = Some(pending),
        Err(TryRecvError::Disconnected) => {
            let mut state = state.borrow_mut();
            if is_current_generation(state.generation, pending.generation) {
                show_error(&mut state, "Document save stopped unexpectedly.".to_owned());
            }
            cancel_window_close_after(&mut state, pending.after);
            sync_ui(&mut state);
        }
    }
}

fn install_workspace(state: &Rc<RefCell<AppState>>, workspace: Workspace) {
    let model = {
        let mut state = state.borrow_mut();
        state.workspace_generation = state.workspace_generation.saturating_add(1);
        state.preview_submission = None;
        state.workspace = Some(workspace);
        state.model = state
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.document().channel_model())
            .map(PreviewModel::from_domain)
            .unwrap_or(PreviewModel::Rgb);
        state.model
    };
    sync_model_selector(state, model);

    let mut state = state.borrow_mut();
    update_backdrop(&mut state);
    clear_preview(&mut state);
    state.preview_target = None;
    show_source_diagnostic(&mut state);
    let page = if state
        .workspace
        .as_ref()
        .is_some_and(|workspace| workspace.source_presentation.is_some())
    {
        Page::Loading
    } else {
        Page::Empty
    };
    set_page(&mut state, page);
    sync_ui(&mut state);
}

/// Synchronize the GTK control without allowing its synchronous notification
/// to turn a loaded document into a model-edit command.
fn sync_model_selector(state: &Rc<RefCell<AppState>>, model: PreviewModel) {
    let selector = {
        let mut state = state.borrow_mut();
        state.syncing_model = true;
        state.selector.clone()
    };
    selector.set_selected(
        PreviewModel::ALL
            .iter()
            .position(|candidate| *candidate == model)
            .unwrap_or(0) as u32,
    );
    state.borrow_mut().syncing_model = false;
}

fn clear_workspace(state: &mut AppState) {
    state.generation = state.generation.saturating_add(1);
    state.workspace_generation = state.workspace_generation.saturating_add(1);
    state.preview_submission = None;
    state.workspace = None;
    state.pending_load = None;
    state.pending_save = None;
    state.pending_export = None;
    clear_preview(state);
    state.preview_target = None;
    state.banner.set_revealed(false);
    set_page(state, Page::Empty);
    sync_ui(state);
}

fn change_model(state: &Rc<RefCell<AppState>>, model: PreviewModel) {
    let mut state = state.borrow_mut();
    if !should_apply_model_change(
        state.syncing_model,
        state.model,
        model,
        state.pending_load.is_some(),
        state
            .workspace
            .as_ref()
            .is_some_and(|workspace| workspace.source_presentation.is_some()),
    ) {
        return;
    }
    let workspace = state
        .workspace
        .as_mut()
        .expect("model change requires a workspace");
    if let Err(error) = replace_model_topology(&mut workspace.history, model) {
        show_error(&mut state, error);
        return;
    }
    state.model = model;
    update_backdrop(&mut state);
    clear_preview(&mut state);
    state.preview_target = None;
    state.preview_submission = None;
    set_page(&mut state, Page::Loading);
    sync_ui(&mut state);
}

fn should_apply_model_change(
    syncing_model: bool,
    current_model: PreviewModel,
    selected_model: PreviewModel,
    loading: bool,
    has_source_presentation: bool,
) -> bool {
    !syncing_model && current_model != selected_model && !loading && has_source_presentation
}

fn replace_model_topology(
    history: &mut DocumentHistory,
    model: PreviewModel,
) -> Result<(), String> {
    let document = history.document();
    let channel = document
        .channel_topology()
        .and_then(|topology| topology.channels().first())
        .ok_or_else(|| {
            "document.channel_topology: model switching requires stored channel topology".to_owned()
        })?;
    let template = ChannelTopologyTemplate {
        pattern_definition_id: channel.pattern_definition_id,
        layout: channel.layout.clone(),
        mark_geometry_response: channel.mark_geometry_response.clone(),
    };
    let topology = document
        .canonical_channel_topology(model.domain(), template)
        .map_err(|error| error.to_string())?;
    history
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: model.domain(),
            topology,
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn submit_if_viewport_ready(state: &mut AppState) {
    let Some(target) = preview_target_for(&state.stack) else {
        return;
    };
    if state.preview_target == Some(target) {
        return;
    }
    if let Err(error) = submit_current_source(state, target) {
        show_error(state, error);
    }
}

fn submit_current_source(
    state: &mut AppState,
    target: toniator_engine::PreviewRasterTarget,
) -> Result<(), String> {
    let workspace = state
        .workspace
        .as_ref()
        .ok_or_else(|| "No document is open.".to_owned())?;
    let presentation = workspace
        .source_presentation
        .as_ref()
        .ok_or_else(|| "No source is loaded.".to_owned())?;
    let source = workspace
        .sources
        .get(&presentation.id)
        .ok_or_else(|| "source.document: source bundle is missing the active source".to_owned())?;
    let resolved = ResolvedSource::new(
        presentation.id.clone(),
        Arc::<[u8]>::from(source.bytes()),
        presentation.format,
    )
    .map_err(|error| error.to_string())?;
    let request = EvaluationRequest::with_preview_target(
        workspace.history.session().document_evaluation_snapshot(),
        resolved,
        target,
    );
    let ticket = state
        .scheduler
        .submit(request)
        .map_err(|error| error.to_string())?;
    state.preview_target = Some(target);
    state.preview_submission = Some(PreviewSubmission {
        workspace_generation: state.workspace_generation,
        ticket: ticket.value(),
    });
    Ok(())
}

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

const fn is_current_generation(current: u64, candidate: u64) -> bool {
    current == candidate
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

fn show_source_diagnostic(state: &mut AppState) {
    let migration_notice = state
        .workspace
        .as_ref()
        .is_some_and(|workspace| workspace.migration_notice);
    let diagnostic = state
        .workspace
        .as_ref()
        .and_then(|workspace| workspace.source_presentation.as_ref())
        .and_then(|source| source.identity.svg_text.as_ref());
    apply_banner_policy(
        &state.banner,
        banner_policy(migration_notice, diagnostic, None),
    );
}

fn set_page(state: &mut AppState, page: Page) {
    state.stack.set_visible_child_name(page.name());
}

fn show_error(state: &mut AppState, message: String) {
    state.error.set_label(&message);
    apply_banner_policy(&state.banner, banner_policy(false, None, Some(&message)));
    // A failed lifecycle operation must not destroy an accepted preview.
    if state.preview.is_none() {
        set_page(state, Page::Error);
    }
}

fn sync_ui(state: &mut AppState) {
    let policy = ui_policy(
        state.workspace.as_ref(),
        state.pending_load.is_some(),
        state.pending_save.is_some(),
        state.pending_export.is_some(),
    );
    state.actions.new.set_enabled(policy.new_enabled);
    state.actions.open.set_enabled(policy.open_enabled);
    state.actions.close.set_enabled(policy.close_enabled);
    state.actions.save.set_enabled(policy.save_enabled);
    state.actions.save_as.set_enabled(policy.save_as_enabled);
    state.actions.export.set_enabled(policy.export_enabled);
    state.selector.set_sensitive(policy.selector_enabled);
    state.window.set_title(Some(&policy.title));
    state
        .window_title
        .set_title(policy.title.trim_end_matches(" — Toniator"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn asset(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(name)
    }

    fn temporary(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/validation/stage-13a")
            .join(format!(
                "{name}-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
    }

    #[derive(Clone, Debug, PartialEq)]
    struct WorkspaceProbe {
        document: Document,
        sources: SourceBundle,
        location: Option<PathBuf>,
        savepoint: Option<SavedContent>,
        title: String,
        revision: u64,
    }

    fn probe(workspace: &Workspace) -> WorkspaceProbe {
        WorkspaceProbe {
            document: workspace.document().clone(),
            sources: workspace.sources.clone(),
            location: workspace.location.clone(),
            savepoint: workspace.savepoint.clone(),
            title: workspace.title(),
            revision: workspace.history.revision().0,
        }
    }

    #[test]
    fn startup_inputs_and_extensions_are_stable() {
        assert_eq!(parse_args(vec![]).unwrap(), None);
        assert_eq!(
            parse_args(vec!["file.toniator".into()]).unwrap(),
            Some(PathBuf::from("file.toniator"))
        );
        assert!(parse_args(vec!["one.png".into(), "two.svg".into()]).is_err());
        assert!(is_container_path(Path::new("A.TONIATOR")));
        assert!(
            format_hint_for_path(Path::new("x.jpg"))
                .unwrap_err()
                .contains(".toniator, PNG, and SVG")
        );
    }

    #[test]
    fn new_and_direct_workspaces_have_revision_zero_and_expected_save_state() {
        let new = Workspace::from_new().unwrap();
        assert_eq!(new.history.revision().0, 0);
        assert!(!new.history.can_undo() && !new.history.can_redo());
        assert!(!new.is_dirty() && !new.can_save());
        let png = Workspace::from_direct(
            Arc::from(fs::read(asset("raster-sample.png")).unwrap()),
            SourceFormatHint::Png,
            "raster-sample.png".into(),
        )
        .unwrap();
        assert_eq!(png.history.revision().0, 0);
        assert!(png.is_dirty() && png.can_save());
        assert_eq!(
            png.document().canvas(),
            &CanvasSpec {
                width: 1024.0,
                height: 1024.0
            }
        );
        assert!(png.title().starts_with("raster-sample.png*"));
    }

    #[test]
    fn both_baselines_and_frozen_containers_cross_the_proper_boundary() {
        for (source, format) in [
            ("raster-sample.png", SourceFormatHint::Png),
            ("vector-sample.svg", SourceFormatHint::Svg),
        ] {
            let workspace = load_workspace(&asset(source)).unwrap();
            assert_eq!(
                workspace.source_presentation.as_ref().unwrap().format,
                format
            );
            assert!(workspace.is_dirty());
        }
        for fixture in ["raster-sample-v1.toniator", "vector-sample-v1.toniator"] {
            let expected = load_container(&asset(fixture)).unwrap();
            let workspace = load_workspace(&asset(fixture)).unwrap();
            assert_eq!(workspace.document(), expected.document());
            assert_eq!(&workspace.sources, expected.sources());
            assert_eq!(workspace.history.revision().0, 0);
            assert!(!workspace.is_dirty());
            assert_eq!(
                workspace.location.as_deref(),
                Some(asset(fixture).as_path())
            );
        }
    }

    #[test]
    fn dirty_is_exact_content_not_revision_and_save_snapshot_is_independent() {
        let mut workspace = Workspace::from_direct(
            Arc::from(fs::read(asset("raster-sample.png")).unwrap()),
            SourceFormatHint::Png,
            "raster.png".into(),
        )
        .unwrap();
        let saved = workspace.snapshot();
        workspace.savepoint = Some(saved.clone());
        assert!(!workspace.is_dirty());
        let id = workspace.document().channel_topology().unwrap().channels()[0].id;
        workspace
            .history
            .apply(&DocumentCommand::SetVisibility {
                channel_id: id,
                visible: false,
            })
            .unwrap();
        assert!(workspace.is_dirty());
        workspace.history.undo().unwrap();
        assert!(workspace.history.revision().0 > 0);
        assert!(!workspace.is_dirty());
        workspace.history.redo().unwrap();
        assert!(workspace.is_dirty());
        let async_snapshot = workspace.snapshot();
        workspace.history.undo().unwrap();
        workspace.savepoint = Some(async_snapshot);
        assert!(workspace.is_dirty());
    }

    #[test]
    fn semantic_noop_remains_clean_even_when_history_revision_advances() {
        let mut workspace = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        let id = workspace.document().channel_topology().unwrap().channels()[0].id;
        let revision = workspace.history.revision();
        workspace
            .history
            .apply(&DocumentCommand::SetVisibility {
                channel_id: id,
                visible: true,
            })
            .unwrap();
        assert!(workspace.history.revision() > revision);
        assert!(!workspace.is_dirty());
    }

    #[test]
    fn save_as_is_atomic_and_embeds_direct_artwork_without_the_original_path() {
        let source_path = temporary("direct-source").with_extension("png");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::copy(asset("raster-sample.png"), &source_path).unwrap();
        let mut workspace = load_workspace(&source_path).unwrap();
        let output = temporary("saved-document").with_extension("toniator");
        let snapshot = workspace.snapshot();
        save_container(&output, &snapshot.document, &snapshot.sources).unwrap();
        workspace.accept_saved_snapshot(output.clone(), snapshot);
        assert!(!workspace.is_dirty());
        assert_eq!(workspace.location.as_deref(), Some(output.as_path()));
        fs::remove_file(&source_path).unwrap();
        let reopened = load_workspace(&output).unwrap();
        assert!(!reopened.is_dirty());
        assert_eq!(reopened.sources.len(), 1);
        assert_ne!(
            reopened.source_presentation.as_ref().unwrap().id.as_str(),
            source_path.to_string_lossy()
        );
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn failed_save_and_stale_candidate_leave_existing_workspace_content_unchanged() {
        let mut workspace = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        let original_location = workspace.location.clone();
        let original_savepoint = workspace.savepoint.clone();
        let original_title = workspace.title();
        let original_revision = workspace.history.revision();
        let id = workspace.document().channel_topology().unwrap().channels()[0].id;
        workspace
            .history
            .apply(&DocumentCommand::SetVisibility {
                channel_id: id,
                visible: false,
            })
            .unwrap();
        let bad_path = temporary("missing-parent/document").with_extension("toniator");
        let failed_snapshot = workspace.snapshot();
        assert!(
            save_container(
                &bad_path,
                &failed_snapshot.document,
                &failed_snapshot.sources
            )
            .is_err()
        );
        assert_eq!(workspace.document(), &failed_snapshot.document);
        assert_eq!(workspace.sources, failed_snapshot.sources);
        assert_eq!(workspace.location, original_location);
        assert_eq!(workspace.savepoint, original_savepoint);
        assert_eq!(
            workspace.title(),
            format!(
                "{}* — Toniator",
                original_title.trim_end_matches(" — Toniator")
            )
        );
        assert!(workspace.history.revision() > original_revision);
        assert!(workspace.is_dirty());
        assert!(is_current_generation(9, 9));
        assert!(!is_current_generation(9, 8));
    }

    #[test]
    fn workspace_preview_gate_rejects_same_document_id_and_revision_after_new() {
        let direct = Workspace::from_direct(
            Arc::from(fs::read(asset("raster-sample.png")).unwrap()),
            SourceFormatHint::Png,
            "raster.png".into(),
        )
        .unwrap();
        let new = Workspace::from_new().unwrap();
        assert_eq!(
            direct.history.session().document_evaluation_token(),
            new.history.session().document_evaluation_token(),
            "the headless session gate alone cannot distinguish these workspaces"
        );
        let old = PreviewSubmission {
            workspace_generation: 41,
            ticket: 7,
        };
        assert!(accepts_preview_submission(41, Some(old), 7));
        assert!(!accepts_preview_submission(42, Some(old), 7));
        assert!(!accepts_preview_submission(42, None, 7));
        assert!(!accepts_preview_submission(41, Some(old), 8));
    }

    #[test]
    fn stale_lifecycle_completion_policy_preserves_current_workspace_atomically() {
        let current = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        let before = probe(&current);
        let load_candidate = WorkspaceProbe {
            document: Workspace::from_new().unwrap().document().clone(),
            sources: SourceBundle::new([]).unwrap(),
            location: None,
            savepoint: None,
            title: "replacement".into(),
            revision: 0,
        };
        assert!(matches!(
            lifecycle_completion_policy(8, 7, load_candidate),
            CompletionInstall::Preserve
        ));
        assert_eq!(probe(&current), before, "stale load cannot replace state");
        let stale_save = (PathBuf::from("other.toniator"), current.snapshot());
        assert!(matches!(
            lifecycle_completion_policy(8, 7, stale_save),
            CompletionInstall::Preserve
        ));
        assert_eq!(
            probe(&current),
            before,
            "stale save cannot change savepoint or location"
        );
    }

    #[test]
    fn unsaved_decisions_and_save_follow_up_preserve_lifecycle_intent() {
        let action = LifecycleAction::Close;
        assert_eq!(
            begin_lifecycle(true, action),
            LifecycleDisposition::Prompt(action)
        );
        assert_eq!(
            resolve_unsaved_decision(action, UnsavedDecision::Cancel),
            LifecycleDisposition::Noop
        );
        assert_eq!(
            resolve_unsaved_decision(action, UnsavedDecision::Discard),
            LifecycleDisposition::Execute(action)
        );
        assert_eq!(
            resolve_unsaved_decision(action, UnsavedDecision::Save),
            LifecycleDisposition::SaveThen(action)
        );
        // Successful Save continues only when the saved snapshot is still the
        // current content. Save As cancellation and Save failure keep the
        // current dirty content and therefore do not execute the action.
        assert_eq!(
            begin_lifecycle(false, action),
            LifecycleDisposition::Execute(action)
        );
        assert_eq!(
            begin_lifecycle(true, action),
            LifecycleDisposition::Prompt(action)
        );
        let mut workspace = load_workspace(&asset("raster-sample.png")).unwrap();
        let saving_snapshot = workspace.snapshot();
        replace_model_topology(&mut workspace.history, PreviewModel::Cmyk).unwrap();
        workspace.accept_saved_snapshot(PathBuf::from("saved.toniator"), saving_snapshot);
        assert!(workspace.is_dirty());
        assert_eq!(
            begin_lifecycle(workspace.is_dirty(), action),
            LifecycleDisposition::Prompt(action),
            "completion must return through the unsaved-work boundary"
        );
    }

    #[test]
    fn window_close_controller_defers_once_and_releases_cancelled_or_failed_flows() {
        let mut controller = WindowCloseController::default();
        assert_eq!(
            request_window_close(&mut controller, true),
            WindowCloseRequest::Ignore,
            "a busy lifecycle cannot leave a close request pending"
        );
        assert_eq!(controller, WindowCloseController::default());

        assert_eq!(
            request_window_close(&mut controller, false),
            WindowCloseRequest::Dispatch
        );
        assert_eq!(
            begin_lifecycle(false, LifecycleAction::WindowClose),
            LifecycleDisposition::Execute(LifecycleAction::WindowClose)
        );
        assert!(controller.defer());
        assert_eq!(
            request_window_close(&mut controller, false),
            WindowCloseRequest::Ignore,
            "repeated clean close requests cannot schedule another close"
        );
        assert!(!controller.defer());
        assert!(controller.accept_deferred());
        assert_eq!(
            request_window_close(&mut controller, false),
            WindowCloseRequest::Proceed
        );

        let mut cancelled = WindowCloseController::default();
        assert_eq!(
            request_window_close(&mut cancelled, false),
            WindowCloseRequest::Dispatch
        );
        assert_eq!(
            begin_lifecycle(true, LifecycleAction::WindowClose),
            LifecycleDisposition::Prompt(LifecycleAction::WindowClose)
        );
        assert_eq!(
            resolve_unsaved_decision(LifecycleAction::WindowClose, UnsavedDecision::Cancel),
            LifecycleDisposition::Noop
        );
        cancelled.cancel();
        assert_eq!(
            request_window_close(&mut cancelled, false),
            WindowCloseRequest::Dispatch,
            "Cancel releases the next close request"
        );

        let mut discard = WindowCloseController::default();
        assert_eq!(
            request_window_close(&mut discard, false),
            WindowCloseRequest::Dispatch
        );
        assert_eq!(
            resolve_unsaved_decision(LifecycleAction::WindowClose, UnsavedDecision::Discard),
            LifecycleDisposition::Execute(LifecycleAction::WindowClose)
        );
        assert!(discard.defer());

        let mut save = WindowCloseController::default();
        assert_eq!(
            request_window_close(&mut save, false),
            WindowCloseRequest::Dispatch
        );
        assert_eq!(
            resolve_unsaved_decision(LifecycleAction::WindowClose, UnsavedDecision::Save),
            LifecycleDisposition::SaveThen(LifecycleAction::WindowClose)
        );
        assert_eq!(
            request_window_close(&mut save, true),
            WindowCloseRequest::Ignore,
            "Save keeps one WindowClose request in flight"
        );
        assert!(save.defer(), "successful Save re-enters one deferred close");

        let mut failed_save = WindowCloseController::default();
        assert_eq!(
            request_window_close(&mut failed_save, false),
            WindowCloseRequest::Dispatch
        );
        failed_save.cancel();
        assert_eq!(
            request_window_close(&mut failed_save, false),
            WindowCloseRequest::Dispatch,
            "Save failure, disconnect, stale completion, or Save As cancellation releases retry"
        );
    }

    #[test]
    fn container_state_is_not_frontend_defaulted_and_failures_are_candidate_only() {
        let loaded = load_container(&asset("vector-sample-v1.toniator")).unwrap();
        let workspace = Workspace::from_container(&asset("vector-sample-v1.toniator")).unwrap();
        assert_eq!(
            workspace.document().channel_model(),
            loaded.document().channel_model()
        );
        assert_eq!(
            workspace.document().pattern_definitions(),
            loaded.document().pattern_definitions()
        );
        assert!(load_workspace(Path::new("unsupported.jpg")).is_err());
        assert!(Workspace::from_container(Path::new("missing.toniator")).is_err());
    }

    #[test]
    fn lifecycle_names_filters_titles_and_raw_presentation_contracts_are_stable() {
        let workspace = Workspace::from_new().unwrap();
        assert_eq!(suggested_filename(&workspace), "Untitled.toniator");
        assert_eq!(
            with_toniator_extension(PathBuf::from("design")),
            PathBuf::from("design.toniator")
        );
        assert_eq!(
            with_toniator_extension(PathBuf::from("design.TONIATOR")),
            PathBuf::from("design.TONIATOR")
        );
        assert_eq!(
            OPEN_FILTER_LABELS,
            [
                "Toniator documents (.toniator)",
                "PNG artwork",
                "SVG artwork"
            ]
        );
        assert_eq!(SAVE_FILTER_LABEL, "Toniator documents (.toniator)");
        assert_eq!(EXPORT_FILTER_LABELS, ["PNG image", "SVG vector image"]);
        assert_eq!(
            LIFECYCLE_BUTTONS.map(|(label, _, _)| label),
            ["_New", "_Open", "_Save", "Save _As", "_Export", "_Close"]
        );
        let surface = RasterSurface::new(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        assert_eq!(
            raw_texture_layout(&surface).unwrap(),
            TextureLayout {
                width: 2,
                height: 1,
                stride: 8
            }
        );
        assert_eq!(surface.pixels(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(preview_target_from_allocation(960, 540, 2).is_some());
        let untitled = Workspace::from_new().unwrap();
        assert_eq!(untitled.title(), "Untitled — Toniator");
        assert_eq!(
            ui_policy(None, false, false, false),
            UiPolicy {
                new_enabled: true,
                open_enabled: true,
                close_enabled: false,
                save_enabled: false,
                save_as_enabled: false,
                export_enabled: false,
                selector_enabled: false,
                title: "Toniator".into(),
            }
        );
        assert_eq!(save_route(Some(&untitled)), SaveRoute::Unavailable);
        let direct = Workspace::from_direct(
            Arc::from(fs::read(asset("raster-sample.png")).unwrap()),
            SourceFormatHint::Png,
            "raster.png".into(),
        )
        .unwrap();
        let ready = ui_policy(Some(&direct), false, false, false);
        assert!(ready.save_enabled && ready.save_as_enabled && ready.selector_enabled);
        assert_eq!(ready.title, "raster.png* — Toniator");
        let saving = ui_policy(Some(&direct), false, true, false);
        assert!(!saving.save_enabled && !saving.save_as_enabled && saving.selector_enabled);
        let exporting = ui_policy(Some(&direct), false, false, true);
        assert!(
            !exporting.new_enabled
                && !exporting.open_enabled
                && !exporting.close_enabled
                && !exporting.save_enabled
                && !exporting.save_as_enabled
                && !exporting.export_enabled
                && !exporting.selector_enabled
        );
        assert_eq!(
            suggested_export_filename(&direct, ExportFormat::Png),
            "raster.png"
        );
        assert_eq!(
            suggested_export_filename(&direct, ExportFormat::Svg),
            "raster.svg"
        );
        assert_eq!(
            export_format_for_path(Path::new("output.PNG")),
            Ok(ExportFormat::Png)
        );
        assert_eq!(
            export_format_for_path(Path::new("output.svg")),
            Ok(ExportFormat::Svg)
        );
        assert!(parse_output_target("96x64").unwrap().is_some());
        assert!(parse_output_target("96").is_err());
        assert!(matches!(save_route(Some(&direct)), SaveRoute::SaveAs));
        let mut container = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        assert_eq!(container.title(), "raster-sample-v1.toniator — Toniator");
        let id = container.document().channel_topology().unwrap().channels()[0].id;
        container
            .history
            .apply(&DocumentCommand::SetVisibility {
                channel_id: id,
                visible: false,
            })
            .unwrap();
        assert_eq!(container.title(), "raster-sample-v1.toniator* — Toniator");
    }

    #[test]
    fn banner_policy_prioritizes_errors_then_migration_then_svg_live_text() {
        let live = toniator_engine::SvgTextDiagnostic {
            has_live_text_node: true,
            font_policy: "system-fonts".into(),
            rendered_glyph_coverage: true,
        };
        assert_eq!(banner_policy(false, None, None), BannerPolicy::Hidden);
        assert_eq!(
            banner_policy(false, Some(&live), None),
            BannerPolicy::Message("SVG live text detected; system-fonts.".into())
        );
        assert_eq!(
            banner_policy(true, Some(&live), None),
            BannerPolicy::Message("Document migration information is available.".into())
        );
        assert_eq!(
            banner_policy(true, Some(&live), Some("save.failed: denied")),
            BannerPolicy::Message("save.failed: denied".into())
        );
    }

    #[test]
    fn model_switch_is_a_history_command_and_preserves_scheduler_token_rules() {
        let mut workspace = Workspace::from_direct(
            Arc::from(fs::read(asset("raster-sample.png")).unwrap()),
            SourceFormatHint::Png,
            "raster.png".into(),
        )
        .unwrap();
        let old = workspace.history.session().document_evaluation_token();
        replace_model_topology(&mut workspace.history, PreviewModel::Cmyk).unwrap();
        assert_eq!(
            workspace.document().channel_model(),
            Some(HalftoneChannelModel::Cmyk)
        );
        assert!(!workspace.history.session().accepts_document_evaluation(old));
    }

    #[test]
    fn programmatic_model_sync_keeps_loaded_source_color_alpha_clean_and_user_changes_apply() {
        let mut source_color_alpha = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        replace_model_topology(
            &mut source_color_alpha.history,
            PreviewModel::SourceColorAlpha,
        )
        .unwrap();
        let saved = temporary("source-color-alpha-v2").with_extension("toniator");
        let snapshot = source_color_alpha.snapshot();
        save_container(&saved, &snapshot.document, &snapshot.sources).unwrap();

        let mut loaded = load_workspace(&saved).unwrap();
        assert_eq!(
            loaded.document().channel_model(),
            Some(HalftoneChannelModel::SourceColorAlpha)
        );
        assert_eq!(loaded.history.revision().0, 0);
        assert!(!loaded.is_dirty());
        assert!(
            !should_apply_model_change(
                true,
                PreviewModel::SourceColorAlpha,
                PreviewModel::SourceColorAlpha,
                false,
                true,
            ),
            "selector synchronization must not apply an authoritative command"
        );
        assert!(should_apply_model_change(
            false,
            PreviewModel::SourceColorAlpha,
            PreviewModel::Cmyk,
            false,
            true,
        ));
        replace_model_topology(&mut loaded.history, PreviewModel::Cmyk).unwrap();
        assert_eq!(
            loaded.document().channel_model(),
            Some(HalftoneChannelModel::Cmyk)
        );
        assert!(loaded.is_dirty());

        assert!(should_apply_model_change(
            false,
            PreviewModel::Cmyk,
            PreviewModel::Rgb,
            false,
            true,
        ));
        replace_model_topology(&mut loaded.history, PreviewModel::Rgb).unwrap();
        assert_eq!(
            loaded.document().channel_model(),
            Some(HalftoneChannelModel::Rgb)
        );
        assert!(
            !should_apply_model_change(false, PreviewModel::Rgb, PreviewModel::Rgb, false, true),
            "the selected current model remains a no-op"
        );
        fs::remove_file(saved).unwrap();
    }

    #[test]
    fn export_uses_immutable_workspace_snapshots_and_keeps_lifecycle_state_unchanged() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/validation/stage-13b/app-tests");
        fs::create_dir_all(&directory).unwrap();
        for entry in fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                fs::remove_file(path).unwrap();
            }
        }
        for input in [
            "raster-sample.png",
            "vector-sample.svg",
            "raster-sample-v1.toniator",
            "vector-sample-v1.toniator",
        ] {
            for model in PreviewModel::ALL {
                let mut workspace = load_workspace(&asset(input)).unwrap();
                if workspace.source_presentation.is_some() {
                    replace_model_topology(&mut workspace.history, model).unwrap();
                }
                let before = probe(&workspace);
                let presentation = workspace.source_presentation.clone().unwrap();
                let source_name = match input {
                    "raster-sample.png" => "raster-direct",
                    "vector-sample.svg" => "vector-direct",
                    "raster-sample-v1.toniator" => "raster-v1",
                    "vector-sample-v1.toniator" => "vector-v1",
                    _ => unreachable!("fixed app-test input"),
                };
                let model_name = match model {
                    PreviewModel::Rgb => "rgb",
                    PreviewModel::Cmyk => "cmyk",
                    PreviewModel::SourceColorAlpha => "source-color-alpha",
                };
                let base = format!("{source_name}-{model_name}");
                let width = workspace.document().canvas().width as u32;
                let height = workspace.document().canvas().height as u32;
                assert_eq!(f64::from(width), workspace.document().canvas().width);
                assert_eq!(f64::from(height), workspace.document().canvas().height);
                assert!(width.max(height) >= 900);
                let target = OutputRasterTarget::new(width, height).unwrap();
                let png = directory.join(format!("{base}-aa-off.png"));
                export_snapshot(
                    workspace.snapshot(),
                    presentation.clone(),
                    png.clone(),
                    ExportSettings {
                        format: ExportFormat::Png,
                        background: RasterBackground::Transparent,
                        antialiasing: RasterAntialiasing::Off,
                        output_target: Some(target),
                    },
                )
                .unwrap();
                let bytes = fs::read(&png).unwrap();
                assert_eq!(png_dimensions(&bytes), (width, height));
                assert_eq!(probe(&workspace), before);

                if input == "raster-sample.png" {
                    let aa_on = directory.join(format!("{base}-aa-on.png"));
                    export_snapshot(
                        workspace.snapshot(),
                        presentation.clone(),
                        aa_on.clone(),
                        ExportSettings {
                            format: ExportFormat::Png,
                            background: RasterBackground::Transparent,
                            antialiasing: RasterAntialiasing::On,
                            output_target: Some(target),
                        },
                    )
                    .unwrap();
                    assert_eq!(png_dimensions(&fs::read(aa_on).unwrap()), (1024, 1024));
                    assert_eq!(probe(&workspace), before);
                }

                let svg = directory.join(format!("{base}.svg"));
                export_snapshot(
                    workspace.snapshot(),
                    presentation,
                    svg.clone(),
                    ExportSettings {
                        format: ExportFormat::Svg,
                        background: RasterBackground::Transparent,
                        antialiasing: RasterAntialiasing::On,
                        output_target: None,
                    },
                )
                .unwrap();
                let svg = fs::read_to_string(svg).unwrap();
                assert!(svg.contains("<circle "));
                assert!(svg.contains("<clipPath"));
                assert_eq!(probe(&workspace), before);
            }
        }

        let workspace = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        let before = probe(&workspace);
        let failure = export_snapshot(
            workspace.snapshot(),
            workspace.source_presentation.clone().unwrap(),
            directory.join("missing-parent/output.png"),
            ExportSettings {
                format: ExportFormat::Png,
                background: RasterBackground::Transparent,
                antialiasing: RasterAntialiasing::On,
                output_target: None,
            },
        );
        assert!(failure.is_err());
        assert_eq!(probe(&workspace), before, "failed export is a no-op");
        assert!(matches!(
            lifecycle_completion_policy(9, 8, ()),
            CompletionInstall::Preserve
        ));

        let workspace = load_workspace(&asset("vector-sample.svg")).unwrap();
        let before = probe(&workspace);
        let default_png = directory.join("default-native.png");
        export_snapshot(
            workspace.snapshot(),
            workspace.source_presentation.clone().unwrap(),
            default_png.clone(),
            ExportSettings {
                format: ExportFormat::Png,
                background: RasterBackground::OpaqueBlack,
                antialiasing: RasterAntialiasing::On,
                output_target: None,
            },
        )
        .unwrap();
        let default_bytes = fs::read(default_png).unwrap();
        assert_eq!(png_dimensions(&default_bytes), (900, 620));
        assert_eq!(probe(&workspace), before);
        let persisted = directory.join("export-state-absent.toniator");
        let snapshot = workspace.snapshot();
        save_container(&persisted, &snapshot.document, &snapshot.sources).unwrap();
        let persisted_bytes = fs::read(persisted).unwrap();
        let serialized = String::from_utf8_lossy(&persisted_bytes);
        for forbidden in [
            "export",
            "antialias",
            "output_target",
            "preview",
            "original_path",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "v1 document JSON must not persist {forbidden}"
            );
        }
    }

    fn png_dimensions(bytes: &[u8]) -> (u32, u32) {
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        (
            u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
            u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
        )
    }
}
