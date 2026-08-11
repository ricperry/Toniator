#![forbid(unsafe_code)]

//! GTK document lifecycle around the headless document, history, engine, and
//! portable-container boundaries.  The workspace below is controller state;
//! `DocumentHistory` remains the only mutable document authority.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
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
    CanvasSpec, ChannelId, ChannelPaint, ChannelTopologyTemplate, ColorComponent,
    DensityEditedAxis, Document, DocumentCommand, DocumentHistory, DocumentSession,
    HalftoneChannelModel, LegacyMappingFieldEdit, MarkGeometryFieldEdit, MarkOrientationKind,
    MarkPrototype, ModeledMappingFieldEdit, PaintKind, PatternDefinitionEdit, PatternDefinitionId,
    PatternMechanismId, PatternOutputLayerId, PropertyCurrentValue, PropertyCurrentValueKind,
    PropertyDescriptor, PropertyEnumChoice, PropertyFieldId, PropertyReferenceValue,
    PropertyTarget, PropertyValueKind, RandomCharacterKind, SourceComponent,
    SourceMappingComponent, SourceReference, SourceReferenceId, TranslationEditedAxis,
    VariantTransitionDraft, VariantTransitionField, VariantTransitionFieldUpdate,
    VariantTransitionValue,
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

/// Frontend-only state.  It is deliberately absent from `Workspace`, document
/// snapshots, history, descriptors, evaluator requests, and persistence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DefinitionEditScope {
    #[default]
    SelectedCopy,
    Shared,
}

#[derive(Clone, Debug, PartialEq)]
enum InspectorInput {
    FiniteF64(f64),
    U32(u32),
    Boolean(bool),
    EnumChoice(PropertyEnumChoice),
    Reference(PropertyReferenceValue),
    ReferenceCollection(Vec<PropertyReferenceValue>),
}

#[derive(Clone, Debug, Default, PartialEq)]
struct InspectorRuntime {
    selected_channel: Option<ChannelId>,
    scope: DefinitionEditScope,
    drafts: BTreeMap<String, String>,
    expanded_groups: BTreeSet<String>,
    transition: Option<VariantTransitionDraft>,
    status: Option<String>,
    focus: Option<InspectorFocusIdentity>,
}

impl InspectorRuntime {
    fn reset_for_workspace(&mut self) {
        *self = Self::default();
    }
}

/// Runtime-only keyboard focus recovery. The descriptor key is the primary
/// identity; a guide scalar's descriptor-order ordinal permits deterministic
/// remapping after selected copy-on-edit gives every guide fresh stable IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
enum InspectorFocusIdentity {
    Descriptor {
        key: String,
        field: PropertyFieldId,
        collection_index: Option<usize>,
        guide_ordinal: Option<usize>,
    },
    TransitionField {
        field: PropertyFieldId,
        target: PropertyTarget,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InspectorControlRoute {
    FiniteNumber,
    WholeNumber,
    Toggle,
    Choice,
    Reference,
    ReferenceCollection,
}

fn control_route(value: &PropertyCurrentValue) -> Option<InspectorControlRoute> {
    match (&value.descriptor.value_kind, &value.value) {
        (PropertyValueKind::FiniteF64, PropertyCurrentValueKind::FiniteF64(_)) => {
            Some(InspectorControlRoute::FiniteNumber)
        }
        (PropertyValueKind::U32, PropertyCurrentValueKind::U32(_)) => {
            Some(InspectorControlRoute::WholeNumber)
        }
        (PropertyValueKind::Boolean, PropertyCurrentValueKind::Boolean(_)) => {
            Some(InspectorControlRoute::Toggle)
        }
        (PropertyValueKind::EnumChoice, PropertyCurrentValueKind::EnumChoice(_)) => {
            Some(InspectorControlRoute::Choice)
        }
        (PropertyValueKind::StableIdReference, PropertyCurrentValueKind::Reference(_)) => {
            Some(InspectorControlRoute::Reference)
        }
        (
            PropertyValueKind::StableIdReference,
            PropertyCurrentValueKind::ReferenceCollection(_),
        ) => Some(InspectorControlRoute::ReferenceCollection),
        _ => None,
    }
}

fn transition_control_route(value: &VariantTransitionValue) -> InspectorControlRoute {
    match value {
        VariantTransitionValue::FiniteF64(_) => InspectorControlRoute::FiniteNumber,
        VariantTransitionValue::U32(_) => InspectorControlRoute::WholeNumber,
        VariantTransitionValue::Boolean(_) => InspectorControlRoute::Toggle,
        VariantTransitionValue::EnumChoice(_) => InspectorControlRoute::Choice,
        VariantTransitionValue::StableReference(_) => InspectorControlRoute::Reference,
    }
}

fn selected_channel_after_transition(
    selected: Option<ChannelId>,
    authoritative_order: &[ChannelId],
) -> Option<ChannelId> {
    selected
        .filter(|id| authoritative_order.contains(id))
        .or_else(|| authoritative_order.first().copied())
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
    undo_enabled: bool,
    redo_enabled: bool,
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
        undo_enabled: !busy && workspace.is_some_and(|workspace| workspace.history.can_undo()),
        redo_enabled: !busy && workspace.is_some_and(|workspace| workspace.history.can_redo()),
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
    undo: gio::SimpleAction,
    redo: gio::SimpleAction,
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
    channel_selector: gtk::DropDown,
    inspector: gtk::Box,
    inspector_status: gtk::Label,
    inspector_runtime: InspectorRuntime,
    syncing_inspector: bool,
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
    let channel_label = gtk::Label::new(Some("_Channel"));
    channel_label.set_use_underline(true);
    channel_label.add_css_class("dim-label");
    let channel_selector = gtk::DropDown::from_strings(&["No channel"]);
    channel_selector.set_sensitive(false);
    channel_selector.set_tooltip_text(Some("Selected authoritative channel"));
    channel_label.set_mnemonic_widget(Some(&channel_selector));
    header.pack_end(&channel_selector);
    header.pack_end(&channel_label);
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
    let inspector = gtk::Box::new(gtk::Orientation::Vertical, 12);
    inspector.add_css_class("toniator-inspector");
    inspector.set_margin_top(12);
    inspector.set_margin_bottom(12);
    inspector.set_margin_start(12);
    inspector.set_margin_end(12);
    let inspector_status =
        gtk::Label::new(Some("Open a source-backed document to inspect channels."));
    inspector_status.set_wrap(true);
    inspector_status.set_xalign(0.0);
    inspector_status.add_css_class("dim-label");
    inspector.append(&inspector_status);
    let inspector_scroll = gtk::ScrolledWindow::new();
    inspector_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    inspector_scroll.set_min_content_width(300);
    inspector_scroll.set_max_content_width(420);
    inspector_scroll.set_child(Some(&inspector));
    let workspace_view = gtk::Paned::new(gtk::Orientation::Horizontal);
    workspace_view.set_wide_handle(true);
    workspace_view.set_start_child(Some(&stack));
    workspace_view.set_end_child(Some(&inspector_scroll));
    workspace_view.set_resize_start_child(true);
    workspace_view.set_shrink_start_child(false);
    workspace_view.set_resize_end_child(false);
    workspace_view.set_shrink_end_child(false);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&banner);
    content.append(&workspace_view);
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
        undo: gio::SimpleAction::new("undo", None),
        redo: gio::SimpleAction::new("redo", None),
    };
    for action in [
        &actions.new,
        &actions.open,
        &actions.save,
        &actions.save_as,
        &actions.export,
        &actions.close,
        &actions.undo,
        &actions.redo,
    ] {
        app.add_action(action);
    }
    app.set_accels_for_action("app.new", &["<Primary>n"]);
    app.set_accels_for_action("app.open", &["<Primary>o"]);
    app.set_accels_for_action("app.save", &["<Primary>s"]);
    app.set_accels_for_action("app.save-as", &["<Primary><Shift>s"]);
    app.set_accels_for_action("app.export", &["<Primary>e"]);
    app.set_accels_for_action("app.close", &["<Primary>w"]);
    app.set_accels_for_action("app.undo", &["<Primary>z"]);
    app.set_accels_for_action("app.redo", &["<Primary><Shift>z"]);
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
        channel_selector,
        inspector,
        inspector_status,
        inspector_runtime: InspectorRuntime::default(),
        syncing_inspector: false,
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
        let channel_selector = state.borrow().channel_selector.clone();
        channel_selector.connect_selected_notify(move |selector| {
            if state.borrow().syncing_inspector {
                return;
            }
            let ids = state
                .borrow()
                .workspace
                .as_ref()
                .map(|workspace| authoritative_channel_ids(workspace.document()))
                .unwrap_or_default();
            if let Some(channel_id) = ids.get(selector.selected() as usize).copied() {
                {
                    let mut app_state = state.borrow_mut();
                    app_state.inspector_runtime.selected_channel = Some(channel_id);
                    // A user-selected channel is a new inspector context.  Never
                    // carry a control identity from the previously selected
                    // channel into it.
                    app_state.inspector_runtime.focus = None;
                }
                rebuild_inspector(&state);
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
    rebuild_inspector(&state);
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
    for (action, redo) in [
        (state.borrow().actions.undo.clone(), false),
        (state.borrow().actions.redo.clone(), true),
    ] {
        let state = Rc::clone(state);
        action.connect_activate(move |_, _| apply_history_navigation(&state, redo));
    }
}

fn apply_history_navigation(state: &Rc<RefCell<AppState>>, redo: bool) {
    let result = {
        let mut app_state = state.borrow_mut();
        let Some(workspace) = app_state.workspace.as_mut() else {
            return;
        };
        if redo {
            workspace.history.redo()
        } else {
            workspace.history.undo()
        }
    };
    match result {
        Ok(Some(_)) => {
            let mut app_state = state.borrow_mut();
            set_preview_pending(&mut app_state);
            set_inspector_status(
                &mut app_state,
                if redo {
                    "Redo applied. Preview update is pending."
                } else {
                    "Undo applied. Preview update is pending."
                },
            );
            sync_ui(&mut app_state);
            drop(app_state);
            rebuild_inspector(state);
        }
        Ok(None) => {}
        Err(error) => show_error(&mut state.borrow_mut(), error.to_string()),
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

fn authoritative_channel_ids(document: &Document) -> Vec<ChannelId> {
    document
        .channels()
        .map(|channels| channels.iter().map(|channel| channel.id).collect())
        .unwrap_or_else(|| {
            document
                .channel_topology()
                .map(|topology| {
                    topology
                        .channels()
                        .iter()
                        .map(|channel| channel.id)
                        .collect()
                })
                .unwrap_or_default()
        })
}

fn channel_display_name(document: &Document, channel_id: ChannelId) -> String {
    if let Some(channel) = document.channel(channel_id) {
        return format!("Legacy channel (ID {})", channel.id.0);
    }
    document
        .modeled_channel(channel_id)
        .map(|channel| {
            format!(
                "{} channel (ID {})",
                channel_role_label(channel.role),
                channel.id.0
            )
        })
        .unwrap_or_else(|| format!("Missing channel (ID {})", channel_id.0))
}

fn channel_role_label(role: toniator_domain::HalftoneChannelRole) -> &'static str {
    match role {
        toniator_domain::HalftoneChannelRole::Red => "Red",
        toniator_domain::HalftoneChannelRole::Green => "Green",
        toniator_domain::HalftoneChannelRole::Blue => "Blue",
        toniator_domain::HalftoneChannelRole::Cyan => "Cyan",
        toniator_domain::HalftoneChannelRole::Magenta => "Magenta",
        toniator_domain::HalftoneChannelRole::Yellow => "Yellow",
        toniator_domain::HalftoneChannelRole::Black => "Black",
        toniator_domain::HalftoneChannelRole::SourceColor => "Source color",
    }
}

fn inspector_key(descriptor: &PropertyDescriptor) -> String {
    format!("{:?}:{:?}", descriptor.target, descriptor.field)
}

fn focus_for_descriptor(
    descriptor: &PropertyDescriptor,
    collection_index: Option<usize>,
) -> InspectorFocusIdentity {
    InspectorFocusIdentity::Descriptor {
        key: inspector_key(descriptor),
        field: descriptor.field,
        collection_index,
        guide_ordinal: None,
    }
}

fn focus_for_descriptor_in_values(
    values: &[PropertyCurrentValue],
    descriptor: &PropertyDescriptor,
    collection_index: Option<usize>,
) -> InspectorFocusIdentity {
    let guide_ordinal = match descriptor.target {
        PropertyTarget::GuideDimension(..) => values
            .iter()
            .filter(|value| {
                value.descriptor.field == descriptor.field
                    && matches!(value.descriptor.target, PropertyTarget::GuideDimension(..))
            })
            .position(|value| value.descriptor.target == descriptor.target),
        _ => None,
    };
    InspectorFocusIdentity::Descriptor {
        key: inspector_key(descriptor),
        field: descriptor.field,
        collection_index,
        guide_ordinal,
    }
}

fn focus_with_collection_index(
    focus: &InspectorFocusIdentity,
    collection_index: usize,
) -> InspectorFocusIdentity {
    match focus {
        InspectorFocusIdentity::Descriptor {
            key,
            field,
            guide_ordinal,
            ..
        } => InspectorFocusIdentity::Descriptor {
            key: key.clone(),
            field: *field,
            collection_index: Some(collection_index),
            guide_ordinal: *guide_ordinal,
        },
        InspectorFocusIdentity::TransitionField { .. } => focus.clone(),
    }
}

fn selected_property_values(
    document: &Document,
    channel_id: ChannelId,
) -> Vec<PropertyCurrentValue> {
    document
        .property_values()
        .into_iter()
        .filter(|value| match value.descriptor.target {
            PropertyTarget::Document => true,
            PropertyTarget::Channel(id) => id == channel_id,
            PropertyTarget::Definition(id)
            | PropertyTarget::Mechanism(id, _)
            | PropertyTarget::OutputLayer(id, _)
            | PropertyTarget::GuideDimension(id, _, _) => document
                .channel(channel_id)
                .map(|channel| channel.pattern_definition_id == id)
                .or_else(|| {
                    document
                        .modeled_channel(channel_id)
                        .map(|channel| channel.pattern_definition_id == id)
                })
                .unwrap_or(false),
        })
        .collect()
}

fn resolve_focus_after_document_change(
    document: &Document,
    selected_channel: Option<ChannelId>,
    focus: Option<InspectorFocusIdentity>,
) -> Option<InspectorFocusIdentity> {
    let values = selected_channel
        .map(|channel_id| selected_property_values(document, channel_id))
        .unwrap_or_default();
    resolve_descriptor_focus(&values, focus)
}

fn resolve_descriptor_focus(
    values: &[PropertyCurrentValue],
    focus: Option<InspectorFocusIdentity>,
) -> Option<InspectorFocusIdentity> {
    let Some(InspectorFocusIdentity::Descriptor {
        key,
        field,
        collection_index,
        guide_ordinal,
    }) = focus
    else {
        return focus;
    };
    if values
        .iter()
        .any(|value| inspector_key(&value.descriptor) == key)
    {
        return Some(InspectorFocusIdentity::Descriptor {
            key,
            field,
            collection_index,
            guide_ordinal,
        });
    }
    let matching = values
        .iter()
        .filter(|value| value.descriptor.field == field)
        .collect::<Vec<_>>();
    let replacement = if let Some(guide_ordinal) = guide_ordinal {
        matching
            .into_iter()
            .filter(|value| matches!(value.descriptor.target, PropertyTarget::GuideDimension(..)))
            .nth(guide_ordinal)
    } else {
        matching.into_iter().next()
    }?;
    Some(focus_for_descriptor_in_values(
        values,
        &replacement.descriptor,
        collection_index,
    ))
}

fn inspector_group(field: PropertyFieldId) -> &'static str {
    match field {
        PropertyFieldId::SourceReference
        | PropertyFieldId::LegacyMappingComponent
        | PropertyFieldId::LegacyMappingPlacement
        | PropertyFieldId::ModeledMappingComponent
        | PropertyFieldId::ModeledMappingPlacement
        | PropertyFieldId::ModeledMappingInverted
        | PropertyFieldId::ModeledMappingGain
        | PropertyFieldId::ModeledMappingBias
        | PropertyFieldId::ArtworkWeightMappingComponent
        | PropertyFieldId::ArtworkWeightMappingPlacement
        | PropertyFieldId::ArtworkWeightMappingInverted
        | PropertyFieldId::ArtworkWeightMappingGain
        | PropertyFieldId::ArtworkWeightMappingBias
        | PropertyFieldId::ArtworkWeightStrength
        | PropertyFieldId::ArtworkWeightResponse => "Source mapping",
        PropertyFieldId::Paint
        | PropertyFieldId::ColorRed
        | PropertyFieldId::ColorGreen
        | PropertyFieldId::ColorBlue
        | PropertyFieldId::ColorAlpha
        | PropertyFieldId::Opacity
        | PropertyFieldId::Visibility => "Channel appearance",
        PropertyFieldId::DensityAcrossX
        | PropertyFieldId::DensityAcrossY
        | PropertyFieldId::DensityAspectLocked
        | PropertyFieldId::RotationDegrees
        | PropertyFieldId::TranslationX
        | PropertyFieldId::TranslationY => "Density and layout",
        PropertyFieldId::MarkMinimumSize | PropertyFieldId::MarkMaximumSize => "Mark response",
        PropertyFieldId::DefinitionSelection => "Definition and sharing",
        PropertyFieldId::CoverageGuardSteps
        | PropertyFieldId::CoverageMaximumSupportRadius
        | PropertyFieldId::GuideBaselineAngle
        | PropertyFieldId::GuidePhase
        | PropertyFieldId::GuideSpacingMultiplier
        | PropertyFieldId::IntersectionDimensions
        | PropertyFieldId::IntersectionMergeEpsilon
        | PropertyFieldId::AlongGuideDimensions
        | PropertyFieldId::AlongGuideIntervalMultiplier
        | PropertyFieldId::AlongGuidePhase => "Active family",
        _ => "Active mechanism and output",
    }
}

fn inspector_field_label(field: PropertyFieldId) -> String {
    match field {
        PropertyFieldId::SourceReference => "Source artwork".into(),
        PropertyFieldId::DensityAcrossX => "Density across X".into(),
        PropertyFieldId::DensityAcrossY => "Density across Y".into(),
        PropertyFieldId::DensityAspectLocked => "Lock density aspect".into(),
        PropertyFieldId::RotationDegrees => "Rotation".into(),
        PropertyFieldId::TranslationX => "Horizontal translation".into(),
        PropertyFieldId::TranslationY => "Vertical translation".into(),
        PropertyFieldId::MarkMinimumSize => "Minimum mark size".into(),
        PropertyFieldId::MarkMaximumSize => "Maximum mark size".into(),
        PropertyFieldId::LegacyMappingComponent | PropertyFieldId::ModeledMappingComponent => {
            "Source component".into()
        }
        PropertyFieldId::LegacyMappingPlacement | PropertyFieldId::ModeledMappingPlacement => {
            "Source placement".into()
        }
        PropertyFieldId::ModeledMappingInverted => "Invert source mapping".into(),
        PropertyFieldId::ModeledMappingGain => "Source mapping gain".into(),
        PropertyFieldId::ModeledMappingBias => "Source mapping bias".into(),
        PropertyFieldId::Paint => "Channel paint".into(),
        PropertyFieldId::ColorRed => "Red".into(),
        PropertyFieldId::ColorGreen => "Green".into(),
        PropertyFieldId::ColorBlue => "Blue".into(),
        PropertyFieldId::ColorAlpha => "Alpha".into(),
        PropertyFieldId::Opacity => "Opacity".into(),
        PropertyFieldId::Visibility => "Visible".into(),
        PropertyFieldId::DefinitionSelection => "Pattern definition".into(),
        PropertyFieldId::CoverageGuardSteps => "Coverage guard steps".into(),
        PropertyFieldId::CoverageMaximumSupportRadius => "Maximum support radius".into(),
        PropertyFieldId::GuideBaselineAngle => "Guide baseline angle".into(),
        PropertyFieldId::GuidePhase => "Guide phase".into(),
        PropertyFieldId::GuideSpacingMultiplier => "Guide spacing multiplier".into(),
        PropertyFieldId::IntersectionDimensions => "Intersection guide dimensions".into(),
        PropertyFieldId::IntersectionMergeEpsilon => "Intersection merge tolerance".into(),
        PropertyFieldId::AlongGuideDimensions => "Along-guide dimensions".into(),
        PropertyFieldId::AlongGuideIntervalMultiplier => "Along-guide interval multiplier".into(),
        PropertyFieldId::AlongGuidePhase => "Along-guide phase".into(),
        PropertyFieldId::RandomCharacter => "Random distribution".into(),
        PropertyFieldId::RandomEvenMinimumCenterDistance => "Even minimum center distance".into(),
        PropertyFieldId::RandomClusterDensity => "Cluster density".into(),
        PropertyFieldId::RandomClusterSpread => "Cluster spread".into(),
        PropertyFieldId::RandomClusterStrength => "Cluster strength".into(),
        PropertyFieldId::RandomSeed => "Random seed".into(),
        PropertyFieldId::RandomDensityModulation => "Density modulation".into(),
        PropertyFieldId::ArtworkWeightMappingComponent => "Artwork weight component".into(),
        PropertyFieldId::ArtworkWeightMappingPlacement => "Artwork weight placement".into(),
        PropertyFieldId::ArtworkWeightMappingInverted => "Invert artwork weight".into(),
        PropertyFieldId::ArtworkWeightMappingGain => "Artwork weight gain".into(),
        PropertyFieldId::ArtworkWeightMappingBias => "Artwork weight bias".into(),
        PropertyFieldId::ArtworkWeightStrength => "Artwork weight strength".into(),
        PropertyFieldId::ArtworkWeightResponse => "Artwork weight response".into(),
        PropertyFieldId::RandomExclusion => "Site exclusion".into(),
        PropertyFieldId::ExclusionMinimumCenterDistance => {
            "Exclusion minimum center distance".into()
        }
        PropertyFieldId::VisibleMarkMargin => "Visible-mark margin".into(),
        PropertyFieldId::VisibleMarkSizingPolicy => "Visible-mark sizing policy".into(),
        PropertyFieldId::RandomMaximumAttempts => "Maximum random attempts".into(),
        PropertyFieldId::RandomMaximumNeighborChecks => "Maximum neighbor checks".into(),
        PropertyFieldId::OutputSiteProduct => "Output site product".into(),
        PropertyFieldId::OutputPrototype => "Mark prototype".into(),
        PropertyFieldId::OutputOrientation => "Mark orientation".into(),
        PropertyFieldId::OutputOrientationDimension => "Orientation guide dimension".into(),
    }
}

fn enum_choice_label(choice: PropertyEnumChoice) -> &'static str {
    match choice {
        PropertyEnumChoice::SourceMappingComponent(component) => match component {
            toniator_domain::SourceMappingComponent::Red => "Red",
            toniator_domain::SourceMappingComponent::Green => "Green",
            toniator_domain::SourceMappingComponent::Blue => "Blue",
            toniator_domain::SourceMappingComponent::Cyan => "Cyan",
            toniator_domain::SourceMappingComponent::Magenta => "Magenta",
            toniator_domain::SourceMappingComponent::Yellow => "Yellow",
            toniator_domain::SourceMappingComponent::Black => "Black",
            toniator_domain::SourceMappingComponent::Alpha => "Alpha",
            toniator_domain::SourceMappingComponent::Luminance => "Luminance",
        },
        PropertyEnumChoice::SourcePlacement(toniator_domain::SourcePlacement::StretchToCanvas) => {
            "Stretch to canvas"
        }
        PropertyEnumChoice::Paint(PaintKind::Solid) => "Solid color",
        PropertyEnumChoice::Paint(PaintKind::SampledSource) => "Sampled source color",
        PropertyEnumChoice::RandomCharacter(RandomCharacterKind::RawUniform) => "Raw uniform",
        PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even) => "Even spacing",
        PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Clustered) => "Clustered",
        PropertyEnumChoice::DensityModulation(toniator_domain::DensityModulationKind::Uniform) => {
            "Uniform"
        }
        PropertyEnumChoice::DensityModulation(
            toniator_domain::DensityModulationKind::ArtworkWeighted,
        ) => "Artwork weighted",
        PropertyEnumChoice::ArtworkWeightResponse(
            toniator_domain::ArtworkWeightResponse::Linear,
        ) => "Linear",
        PropertyEnumChoice::ArtworkWeightResponse(
            toniator_domain::ArtworkWeightResponse::Smoothstep,
        ) => "Smoothstep",
        PropertyEnumChoice::Exclusion(toniator_domain::ExclusionKind::None) => "None",
        PropertyEnumChoice::Exclusion(toniator_domain::ExclusionKind::MinimumCenterDistance) => {
            "Minimum center distance"
        }
        PropertyEnumChoice::Exclusion(toniator_domain::ExclusionKind::VisibleMarkMargin) => {
            "Visible-mark margin"
        }
        PropertyEnumChoice::VisibleMarkSizingPolicy(
            toniator_domain::VisibleMarkSizingPolicy::MaximumSupportRadius,
        ) => "Maximum support radius",
        PropertyEnumChoice::MarkPrototype(toniator_domain::MarkPrototypeKind::Circle) => "Circle",
        PropertyEnumChoice::MarkOrientation(MarkOrientationKind::Fixed) => "Fixed",
        PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideTangent) => "Guide tangent",
        PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideNormal) => "Guide normal",
    }
}

fn reference_label(reference: &PropertyReferenceValue) -> String {
    match reference {
        PropertyReferenceValue::Source(SourceReference::Unassigned) => "No source assigned".into(),
        PropertyReferenceValue::Source(SourceReference::Assigned(id)) => {
            format!("Source reference ({})", id.as_str())
        }
        PropertyReferenceValue::Definition(id) => format!("Pattern definition (ID {})", id.0),
        PropertyReferenceValue::Mechanism(id) => format!("Mechanism (ID {})", id.0),
        PropertyReferenceValue::GuideDimension(id) => format!("Guide dimension (ID {})", id.0),
    }
}

fn current_display(value: &PropertyCurrentValueKind) -> String {
    match value {
        PropertyCurrentValueKind::FiniteF64(value) => format!("{value:.6}"),
        PropertyCurrentValueKind::U32(value) => value.to_string(),
        PropertyCurrentValueKind::Boolean(value) => value.to_string(),
        PropertyCurrentValueKind::EnumChoice(value) => enum_choice_label(*value).into(),
        PropertyCurrentValueKind::Reference(value) => reference_label(value),
        PropertyCurrentValueKind::ReferenceCollection(values) => values
            .iter()
            .map(reference_label)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn is_advanced_descriptor(field: PropertyFieldId) -> bool {
    matches!(
        inspector_group(field),
        "Active family" | "Active mechanism and output"
    )
}

fn is_transition_selector(field: PropertyFieldId) -> bool {
    matches!(
        field,
        PropertyFieldId::RandomCharacter
            | PropertyFieldId::RandomDensityModulation
            | PropertyFieldId::RandomExclusion
            | PropertyFieldId::OutputOrientation
    )
}

fn focus_matches(
    requested: &Option<InspectorFocusIdentity>,
    candidate: &InspectorFocusIdentity,
) -> bool {
    requested.as_ref() == Some(candidate)
}

fn focus_after_command_attempt(
    current: Option<InspectorFocusIdentity>,
    requested: Option<InspectorFocusIdentity>,
    accepted: bool,
) -> Option<InspectorFocusIdentity> {
    if accepted { requested } else { current }
}

fn schedule_inspector_focus(
    state: &Rc<RefCell<AppState>>,
    identity: InspectorFocusIdentity,
    control: &impl IsA<gtk::Widget>,
) {
    if !focus_matches(&state.borrow().inspector_runtime.focus, &identity) {
        return;
    }
    let widget: gtk::Widget = control.clone().upcast();
    let state = Rc::clone(state);
    glib::idle_add_local_once(move || {
        if focus_matches(&state.borrow().inspector_runtime.focus, &identity) {
            widget.grab_focus();
        }
    });
}

fn rebuild_inspector(state: &Rc<RefCell<AppState>>) {
    let (
        inspector,
        status,
        selector,
        labels,
        selected,
        values,
        scope,
        transition,
        status_message,
        expanded_groups,
        focus,
    ) = {
        let mut state = state.borrow_mut();
        let (labels, values, selected) = if let Some(document) = state
            .workspace
            .as_ref()
            .map(|workspace| workspace.document().clone())
        {
            let ids = authoritative_channel_ids(&document);
            let previous_selected = state.inspector_runtime.selected_channel;
            state.inspector_runtime.selected_channel =
                selected_channel_after_transition(previous_selected, &ids);
            let selected = state.inspector_runtime.selected_channel;
            if previous_selected.is_some() && previous_selected != selected {
                state.inspector_runtime.focus = None;
            } else {
                state.inspector_runtime.focus = resolve_focus_after_document_change(
                    &document,
                    selected,
                    state.inspector_runtime.focus.clone(),
                );
            }
            let values = selected
                .map(|channel_id| selected_property_values(&document, channel_id))
                .unwrap_or_default();
            (
                ids.iter()
                    .map(|id| channel_display_name(&document, *id))
                    .collect(),
                values,
                selected,
            )
        } else {
            (Vec::new(), Vec::new(), None)
        };
        state.syncing_inspector = true;
        let selector = state.channel_selector.clone();
        (
            state.inspector.clone(),
            state.inspector_status.clone(),
            selector,
            labels,
            selected,
            values,
            state.inspector_runtime.scope,
            state.inspector_runtime.transition.clone(),
            state.inspector_runtime.status.clone(),
            state.inspector_runtime.expanded_groups.clone(),
            state.inspector_runtime.focus.clone(),
        )
    };
    selector.set_model(Some(&gtk::StringList::new(
        labels
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice(),
    )));
    selector.set_sensitive(!labels.is_empty());
    selector.set_selected(
        selected
            .and_then(|id| {
                authoritative_channel_ids(
                    state
                        .borrow()
                        .workspace
                        .as_ref()
                        .expect("selected has workspace")
                        .document(),
                )
                .iter()
                .position(|candidate| *candidate == id)
            })
            .unwrap_or(gtk::INVALID_LIST_POSITION as usize) as u32,
    );
    state.borrow_mut().syncing_inspector = false;

    while let Some(child) = inspector.first_child() {
        inspector.remove(&child);
    }
    if selected.is_none() {
        status.set_label("No surviving channel is selected.");
        inspector.append(&status);
        return;
    }
    status.set_label(
        status_message
            .as_deref()
            .unwrap_or("Edits are validated by the document and committed atomically."),
    );
    inspector.append(&status);
    append_sharing_controls(state, &inspector, scope);
    if let Some(transition) = transition {
        append_transition_draft(state, &inspector, transition);
    }
    let descriptor_focuses = values
        .iter()
        .map(|value| {
            (
                inspector_key(&value.descriptor),
                focus_for_descriptor_in_values(&values, &value.descriptor, None),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut last_group = "";
    let mut advanced_groups: Vec<(&str, Vec<PropertyCurrentValue>)> = Vec::new();
    for value in values {
        let group = inspector_group(value.descriptor.field);
        if is_advanced_descriptor(value.descriptor.field) {
            if let Some((last, fields)) = advanced_groups.last_mut()
                && *last == group
            {
                fields.push(value);
            } else {
                advanced_groups.push((group, vec![value]));
            }
            continue;
        }
        if group != last_group {
            let heading = gtk::Label::new(Some(group));
            heading.set_xalign(0.0);
            heading.add_css_class("heading");
            heading.set_margin_top(12);
            inspector.append(&heading);
            last_group = group;
        }
        let focus = descriptor_focuses[&inspector_key(&value.descriptor)].clone();
        append_descriptor_control(state, &inspector, value, focus);
    }
    for (group, fields) in advanced_groups {
        let focus_expands_group = fields.iter().any(|value| {
            matches!(
                &focus,
                Some(InspectorFocusIdentity::Descriptor { key, .. })
                    if *key == inspector_key(&value.descriptor)
            )
        });
        if focus_expands_group {
            state
                .borrow_mut()
                .inspector_runtime
                .expanded_groups
                .insert(group.to_owned());
        }
        let advanced = gtk::Box::new(gtk::Orientation::Vertical, 8);
        for value in fields {
            let focus = descriptor_focuses[&inspector_key(&value.descriptor)].clone();
            append_descriptor_control(state, &advanced, value, focus);
        }
        let expander = gtk::Expander::new(Some(group));
        expander.set_child(Some(&advanced));
        expander.set_expanded(expanded_groups.contains(group) || focus_expands_group);
        expander.set_tooltip_text(Some("Show advanced active pattern controls"));
        let state = Rc::clone(state);
        let group = group.to_owned();
        expander.connect_expanded_notify(move |expander| {
            let mut state = state.borrow_mut();
            if expander.is_expanded() {
                state
                    .inspector_runtime
                    .expanded_groups
                    .insert(group.clone());
            } else {
                state.inspector_runtime.expanded_groups.remove(&group);
            }
        });
        inspector.append(&expander);
    }
}

fn append_sharing_controls(
    state: &Rc<RefCell<AppState>>,
    inspector: &gtk::Box,
    scope: DefinitionEditScope,
) {
    let sharing = {
        let binding = state.borrow();
        binding.workspace.as_ref().and_then(|workspace| {
            let document = workspace.document();
            binding
                .inspector_runtime
                .selected_channel
                .and_then(|channel_id| {
                    let definition_id = document
                        .channel(channel_id)
                        .map(|channel| channel.pattern_definition_id)
                        .or_else(|| {
                            document
                                .modeled_channel(channel_id)
                                .map(|channel| channel.pattern_definition_id)
                        })?;
                    let links = authoritative_channel_ids(document)
                        .into_iter()
                        .filter(|id| {
                            document
                                .channel(*id)
                                .map(|channel| channel.pattern_definition_id == definition_id)
                                .or_else(|| {
                                    document.modeled_channel(*id).map(|channel| {
                                        channel.pattern_definition_id == definition_id
                                    })
                                })
                                .unwrap_or(false)
                        })
                        .count();
                    Some(format!(
                        "Definition {} — linked by {links} channel(s)",
                        definition_id.0
                    ))
                })
        })
    };
    if let Some(sharing) = sharing {
        let summary = gtk::Label::new(Some(&sharing));
        summary.set_xalign(0.0);
        summary.set_wrap(true);
        summary.add_css_class("dim-label");
        inspector.append(&summary);
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label = gtk::Label::new(Some("_Structural edit scope"));
    label.set_use_underline(true);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    let selector =
        gtk::DropDown::from_strings(&["Copy selected channel", "Edit shared definition"]);
    selector.set_selected(match scope {
        DefinitionEditScope::SelectedCopy => 0,
        DefinitionEditScope::Shared => 1,
    });
    label.set_mnemonic_widget(Some(&selector));
    let state = Rc::clone(state);
    selector.connect_selected_notify(move |selector| {
        let scope = if selector.selected() == 1 {
            DefinitionEditScope::Shared
        } else {
            DefinitionEditScope::SelectedCopy
        };
        state.borrow_mut().inspector_runtime.scope = scope;
    });
    row.append(&label);
    row.append(&selector);
    inspector.append(&row);
}

fn transition_field_key(field: &VariantTransitionField) -> String {
    format!("transition:{:?}:{:?}", field.target, field.field)
}

fn append_transition_draft(
    state: &Rc<RefCell<AppState>>,
    inspector: &gtk::Box,
    transition: VariantTransitionDraft,
) {
    let heading = gtk::Label::new(Some("Pending structural transition"));
    heading.set_xalign(0.0);
    heading.add_css_class("heading");
    heading.set_margin_top(12);
    inspector.append(&heading);
    let instruction = gtk::Label::new(Some(&format!(
        "{} is selected but not yet applied. Complete every required value, then confirm one atomic edit.",
        enum_choice_label(transition.choice())
    )));
    instruction.set_xalign(0.0);
    instruction.set_wrap(true);
    inspector.append(&instruction);
    let selector_focus = focus_for_descriptor(transition.selector(), None);
    for field in transition.fields().iter().cloned() {
        append_transition_field(state, inspector, field);
    }
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let cancel = gtk::Button::with_mnemonic("_Cancel transition");
    cancel.set_tooltip_text(Some("Discard only the pending structural transition"));
    let state_for_cancel = Rc::clone(state);
    cancel.connect_clicked(move |_| {
        let mut state = state_for_cancel.borrow_mut();
        state.inspector_runtime.transition = None;
        state.inspector_runtime.focus = Some(selector_focus.clone());
        state.inspector_runtime.status = Some("Pending transition discarded.".into());
        drop(state);
        rebuild_inspector(&state_for_cancel);
    });
    let confirm = gtk::Button::with_mnemonic("_Confirm transition");
    confirm.set_tooltip_text(Some("Validate and apply one typed structural command"));
    let state_for_confirm = Rc::clone(state);
    confirm.connect_clicked(move |_| commit_transition_draft(&state_for_confirm));
    buttons.append(&cancel);
    buttons.append(&confirm);
    inspector.append(&buttons);
}

fn append_transition_field(
    state: &Rc<RefCell<AppState>>,
    inspector: &gtk::Box,
    field: VariantTransitionField,
) {
    debug_assert!(matches!(
        transition_control_route(&field.value),
        InspectorControlRoute::FiniteNumber
            | InspectorControlRoute::WholeNumber
            | InspectorControlRoute::Toggle
            | InspectorControlRoute::Choice
            | InspectorControlRoute::Reference
    ));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_valign(gtk::Align::Center);
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    let label = gtk::Label::new(Some(&format!("_{}", inspector_field_label(field.field))));
    label.set_use_underline(true);
    label.set_xalign(0.0);
    label.set_wrap(true);
    let detail = gtk::Label::new(Some(&field_detail(
        field.contract.bounds,
        field.contract.unit,
    )));
    detail.set_xalign(0.0);
    detail.add_css_class("dim-label");
    labels.append(&label);
    labels.append(&detail);
    let key = transition_field_key(&field);
    let focus = InspectorFocusIdentity::TransitionField {
        field: field.field,
        target: field.target,
    };
    match field.value.clone() {
        VariantTransitionValue::FiniteF64(value) => {
            let control = gtk::Entry::new();
            control.set_text(&draft_text(state, &key, &format!("{value:.6}")));
            control.set_input_purpose(gtk::InputPurpose::Number);
            control.set_width_chars(12);
            label.set_mnemonic_widget(Some(&control));
            let state_for_callback = Rc::clone(state);
            control.connect_activate(move |control| {
                let text = control.text().to_string();
                match text.parse::<f64>() {
                    Ok(value) if value.is_finite() => {
                        remember_transition_draft(&state_for_callback, &field, text);
                        update_transition_field(
                            &state_for_callback,
                            field.clone(),
                            VariantTransitionValue::FiniteF64(value),
                        );
                    }
                    _ => record_transition_draft(
                        &state_for_callback,
                        &field,
                        text,
                        "Enter a finite number.",
                    ),
                }
            });
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
        }
        VariantTransitionValue::U32(value) => {
            let control = gtk::Entry::new();
            control.set_text(&draft_text(state, &key, &value.to_string()));
            control.set_input_purpose(gtk::InputPurpose::Digits);
            control.set_width_chars(10);
            label.set_mnemonic_widget(Some(&control));
            let state_for_callback = Rc::clone(state);
            control.connect_activate(move |control| {
                let text = control.text().to_string();
                match text.parse::<u32>() {
                    Ok(value) => {
                        remember_transition_draft(&state_for_callback, &field, text);
                        update_transition_field(
                            &state_for_callback,
                            field.clone(),
                            VariantTransitionValue::U32(value),
                        );
                    }
                    Err(_) => record_transition_draft(
                        &state_for_callback,
                        &field,
                        text,
                        "Enter a whole number.",
                    ),
                }
            });
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
        }
        VariantTransitionValue::Boolean(value) => {
            let control = gtk::Switch::new();
            control.set_active(value);
            label.set_mnemonic_widget(Some(&control));
            let state_for_callback = Rc::clone(state);
            control.connect_state_set(move |_, value| {
                update_transition_field(
                    &state_for_callback,
                    field.clone(),
                    VariantTransitionValue::Boolean(value),
                );
                glib::Propagation::Proceed
            });
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
        }
        VariantTransitionValue::EnumChoice(value) => {
            let choices = field.contract.choices.to_vec();
            let labels_for_choices = choices
                .iter()
                .map(|choice| enum_choice_label(*choice))
                .collect::<Vec<_>>();
            let control = gtk::DropDown::from_strings(&labels_for_choices);
            control.set_selected(
                choices
                    .iter()
                    .position(|choice| *choice == value)
                    .unwrap_or(0) as u32,
            );
            label.set_mnemonic_widget(Some(&control));
            let state_for_callback = Rc::clone(state);
            control.connect_selected_notify(move |control| {
                if let Some(choice) = choices.get(control.selected() as usize).copied() {
                    update_transition_field(
                        &state_for_callback,
                        field.clone(),
                        VariantTransitionValue::EnumChoice(choice),
                    );
                }
            });
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
        }
        VariantTransitionValue::StableReference(selected) => {
            let choices = field.reference_choices.clone();
            let mut labels_for_choices = vec!["Select a stable reference…".to_owned()];
            labels_for_choices.extend(choices.iter().map(reference_label));
            let control = gtk::DropDown::from_strings(
                &labels_for_choices
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            );
            control.set_selected(
                selected
                    .as_ref()
                    .and_then(|reference| {
                        choices
                            .iter()
                            .position(|choice| choice == reference)
                            .map(|index| index + 1)
                    })
                    .unwrap_or(0) as u32,
            );
            label.set_mnemonic_widget(Some(&control));
            let state_for_callback = Rc::clone(state);
            control.connect_selected_notify(move |control| {
                let selected = control.selected();
                if selected == 0 {
                    return;
                }
                if let Some(reference) = choices.get(selected as usize - 1).cloned() {
                    update_transition_field(
                        &state_for_callback,
                        field.clone(),
                        VariantTransitionValue::StableReference(Some(reference)),
                    );
                }
            });
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
        }
    }
    inspector.append(&row);
}

fn append_descriptor_control(
    state: &Rc<RefCell<AppState>>,
    inspector: &gtk::Box,
    current: PropertyCurrentValue,
    focus: InspectorFocusIdentity,
) {
    debug_assert!(control_route(&current).is_some());
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_valign(gtk::Align::Center);
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    let label = gtk::Label::new(Some(&format!(
        "_{}",
        inspector_field_label(current.descriptor.field)
    )));
    label.set_use_underline(true);
    label.set_xalign(0.0);
    label.set_wrap(true);
    let detail = gtk::Label::new(Some(&field_detail(
        current.descriptor.bounds,
        current.descriptor.unit,
    )));
    detail.set_xalign(0.0);
    detail.add_css_class("dim-label");
    labels.append(&label);
    labels.append(&detail);
    let descriptor = current.descriptor.clone();
    match (&descriptor.value_kind, &current.value) {
        (PropertyValueKind::Boolean, PropertyCurrentValueKind::Boolean(active)) => {
            let control = gtk::Switch::new();
            control.set_active(*active);
            control.set_tooltip_text(Some("Commits a typed Boolean command"));
            label.set_mnemonic_widget(Some(&control));
            let state_for_callback = Rc::clone(state);
            let descriptor_for_callback = descriptor.clone();
            let focus_for_callback = focus.clone();
            control.connect_state_set(move |_, active| {
                commit_inspector_input_with_focus(
                    &state_for_callback,
                    descriptor_for_callback.clone(),
                    InspectorInput::Boolean(active),
                    focus_for_callback.clone(),
                );
                glib::Propagation::Proceed
            });
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
        }
        (PropertyValueKind::EnumChoice, PropertyCurrentValueKind::EnumChoice(active)) => {
            let choice_labels = descriptor
                .choices
                .iter()
                .map(|choice| enum_choice_label(*choice))
                .collect::<Vec<_>>();
            let control = gtk::DropDown::from_strings(&choice_labels);
            control.set_selected(
                descriptor
                    .choices
                    .iter()
                    .position(|choice| choice == active)
                    .unwrap_or(0) as u32,
            );
            label.set_mnemonic_widget(Some(&control));
            let choices = descriptor.choices.to_vec();
            let state_for_callback = Rc::clone(state);
            let descriptor_for_callback = descriptor.clone();
            let focus_for_callback = focus.clone();
            control.connect_selected_notify(move |control| {
                if let Some(choice) = choices.get(control.selected() as usize).copied() {
                    if is_transition_selector(descriptor_for_callback.field) {
                        begin_variant_transition(
                            &state_for_callback,
                            descriptor_for_callback.clone(),
                            choice,
                        );
                    } else {
                        commit_inspector_input_with_focus(
                            &state_for_callback,
                            descriptor_for_callback.clone(),
                            InspectorInput::EnumChoice(choice),
                            focus_for_callback.clone(),
                        );
                    }
                }
            });
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
        }
        (PropertyValueKind::FiniteF64, PropertyCurrentValueKind::FiniteF64(value)) => {
            let control = gtk::Entry::new();
            control.set_text(&draft_text(
                state,
                &inspector_key(&descriptor),
                &format!("{value:.6}"),
            ));
            control.set_input_purpose(gtk::InputPurpose::Number);
            control.set_width_chars(12);
            control.set_tooltip_text(Some(&field_detail(descriptor.bounds, descriptor.unit)));
            label.set_mnemonic_widget(Some(&control));
            let state_for_callback = Rc::clone(state);
            let descriptor_for_callback = descriptor.clone();
            let focus_for_callback = focus.clone();
            control.connect_activate(move |control| {
                let text = control.text().to_string();
                match text.parse::<f64>() {
                    Ok(value) if value.is_finite() => {
                        remember_inspector_draft(
                            &state_for_callback,
                            &descriptor_for_callback,
                            text,
                        );
                        commit_inspector_input_with_focus(
                            &state_for_callback,
                            descriptor_for_callback.clone(),
                            InspectorInput::FiniteF64(value),
                            focus_for_callback.clone(),
                        );
                    }
                    _ => record_inspector_draft(
                        &state_for_callback,
                        &descriptor_for_callback,
                        text,
                        "Enter a finite number.",
                    ),
                }
            });
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
        }
        (PropertyValueKind::U32, PropertyCurrentValueKind::U32(value)) => {
            let control = gtk::Entry::new();
            control.set_text(&draft_text(
                state,
                &inspector_key(&descriptor),
                &value.to_string(),
            ));
            control.set_input_purpose(gtk::InputPurpose::Digits);
            control.set_width_chars(10);
            control.set_tooltip_text(Some(&field_detail(descriptor.bounds, descriptor.unit)));
            label.set_mnemonic_widget(Some(&control));
            let state_for_callback = Rc::clone(state);
            let descriptor_for_callback = descriptor.clone();
            let focus_for_callback = focus.clone();
            control.connect_activate(move |control| {
                let text = control.text().to_string();
                match text.parse::<u32>() {
                    Ok(value) => {
                        remember_inspector_draft(
                            &state_for_callback,
                            &descriptor_for_callback,
                            text,
                        );
                        commit_inspector_input_with_focus(
                            &state_for_callback,
                            descriptor_for_callback.clone(),
                            InspectorInput::U32(value),
                            focus_for_callback.clone(),
                        );
                    }
                    Err(_) => record_inspector_draft(
                        &state_for_callback,
                        &descriptor_for_callback,
                        text,
                        "Enter a whole number.",
                    ),
                }
            });
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
        }
        (_, PropertyCurrentValueKind::Reference(reference)) => {
            let choices = reference_choices(state, &descriptor);
            let reference_labels = choices.iter().map(reference_label).collect::<Vec<_>>();
            let control = gtk::DropDown::from_strings(
                &reference_labels
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            );
            control.set_selected(
                choices
                    .iter()
                    .position(|choice| choice == reference)
                    .unwrap_or(0) as u32,
            );
            control.set_sensitive(!choices.is_empty());
            label.set_mnemonic_widget(Some(&control));
            let state_for_callback = Rc::clone(state);
            let descriptor_for_callback = descriptor.clone();
            let focus_for_callback = focus.clone();
            control.connect_selected_notify(move |control| {
                if let Some(value) = choices.get(control.selected() as usize).cloned() {
                    commit_inspector_input_with_focus(
                        &state_for_callback,
                        descriptor_for_callback.clone(),
                        InspectorInput::Reference(value),
                        focus_for_callback.clone(),
                    );
                }
            });
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
        }
        (_, PropertyCurrentValueKind::ReferenceCollection(references)) => {
            let control = gtk::Box::new(gtk::Orientation::Vertical, 4);
            let choices = reference_collection_choices(state, &descriptor);
            for (index, reference) in references.iter().cloned().enumerate() {
                let reference_labels = choices.iter().map(reference_label).collect::<Vec<_>>();
                let select = gtk::DropDown::from_strings(
                    &reference_labels
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                );
                select.set_selected(
                    choices
                        .iter()
                        .position(|choice| choice == &reference)
                        .unwrap_or(0) as u32,
                );
                let state_for_callback = Rc::clone(state);
                let descriptor = descriptor.clone();
                let choices = choices.clone();
                let original = references.clone();
                let focus = focus_with_collection_index(&focus, index);
                let focus_for_callback = focus.clone();
                select.connect_selected_notify(move |select| {
                    if let Some(reference) = choices.get(select.selected() as usize).cloned() {
                        let mut rewritten = original.clone();
                        rewritten[index] = reference;
                        commit_inspector_input_with_focus(
                            &state_for_callback,
                            descriptor.clone(),
                            InspectorInput::ReferenceCollection(rewritten.clone()),
                            focus_for_callback.clone(),
                        );
                    }
                });
                control.append(&select);
                schedule_inspector_focus(state, focus, &select);
            }
            control.set_tooltip_text(Some("Ordered stable guide-dimension references; the domain validates order, cardinality, and uniqueness."));
            label.set_mnemonic_widget(Some(&control));
            row.append(&labels);
            row.append(&control);
        }
        _ => {
            let control = gtk::Label::new(Some(&current_display(&current.value)));
            control.set_xalign(1.0);
            row.append(&labels);
            row.append(&control);
        }
    }
    inspector.append(&row);
}

fn unit_message(unit: toniator_domain::PropertyUnit) -> &'static str {
    match unit {
        toniator_domain::PropertyUnit::None => "No unit",
        toniator_domain::PropertyUnit::Density => "Density",
        toniator_domain::PropertyUnit::Degrees => "Degrees",
        toniator_domain::PropertyUnit::Phase => "Phase distance",
        toniator_domain::PropertyUnit::DocumentDistance => "Document distance",
        toniator_domain::PropertyUnit::NormalizedComponent => "Normalized component",
        toniator_domain::PropertyUnit::Count => "Count",
    }
}

fn bounds_message(bounds: toniator_domain::PropertyBounds) -> String {
    let minimum = bounds
        .minimum
        .map(|value| {
            format!(
                "{}{}",
                if bounds.minimum_inclusive { "≥" } else { ">" },
                value
            )
        })
        .unwrap_or_default();
    let maximum = bounds
        .maximum
        .map(|value| {
            format!(
                "{}{}",
                if bounds.maximum_inclusive { "≤" } else { "<" },
                value
            )
        })
        .unwrap_or_default();
    [minimum, maximum]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn field_detail(
    bounds: Option<toniator_domain::PropertyBounds>,
    unit: toniator_domain::PropertyUnit,
) -> String {
    let bounds = bounds.map(bounds_message).filter(|value| !value.is_empty());
    match (bounds, unit_message(unit)) {
        (Some(bounds), "No unit") => bounds,
        (Some(bounds), unit) => format!("{bounds}; {unit}"),
        (None, unit) => unit.to_owned(),
    }
}

fn draft_text(state: &Rc<RefCell<AppState>>, key: &str, authoritative: &str) -> String {
    state
        .borrow()
        .inspector_runtime
        .drafts
        .get(key)
        .cloned()
        .unwrap_or_else(|| authoritative.to_owned())
}

fn set_inspector_status(state: &mut AppState, message: impl Into<String>) {
    let message = message.into();
    state.inspector_runtime.status = Some(message.clone());
    state.inspector_status.set_label(&message);
}

fn record_transition_draft(
    state: &Rc<RefCell<AppState>>,
    field: &VariantTransitionField,
    text: String,
    message: &str,
) {
    let mut state = state.borrow_mut();
    state
        .inspector_runtime
        .drafts
        .insert(transition_field_key(field), text);
    set_inspector_status(&mut state, message);
}

fn remember_transition_draft(
    state: &Rc<RefCell<AppState>>,
    field: &VariantTransitionField,
    text: String,
) {
    state
        .borrow_mut()
        .inspector_runtime
        .drafts
        .insert(transition_field_key(field), text);
}

fn update_transition_field(
    state: &Rc<RefCell<AppState>>,
    field: VariantTransitionField,
    value: VariantTransitionValue,
) {
    let rejected_text = match &value {
        VariantTransitionValue::FiniteF64(value) => Some(value.to_string()),
        VariantTransitionValue::U32(value) => Some(value.to_string()),
        _ => None,
    };
    let update = VariantTransitionFieldUpdate {
        field: field.field,
        target: field.target,
        value,
    };
    let changed = {
        let mut state = state.borrow_mut();
        let Some(transition) = state.inspector_runtime.transition.as_ref() else {
            return;
        };
        match transition.with_updates(&[update]) {
            Ok(next) => {
                state
                    .inspector_runtime
                    .drafts
                    .remove(&transition_field_key(&field));
                state.inspector_runtime.transition = Some(next);
                state.inspector_runtime.focus = Some(InspectorFocusIdentity::TransitionField {
                    field: field.field,
                    target: field.target,
                });
                set_inspector_status(
                    &mut state,
                    "Transition value updated. Review and confirm the structural edit.",
                );
                true
            }
            Err(error) => {
                if let Some(text) = rejected_text {
                    state
                        .inspector_runtime
                        .drafts
                        .entry(transition_field_key(&field))
                        .or_insert(text);
                }
                set_inspector_status(&mut state, error.to_string());
                false
            }
        }
    };
    if changed {
        rebuild_inspector(state);
    }
}

fn reference_choices(
    state: &Rc<RefCell<AppState>>,
    descriptor: &PropertyDescriptor,
) -> Vec<PropertyReferenceValue> {
    let binding = state.borrow();
    let Some(document) = binding.workspace.as_ref().map(Workspace::document) else {
        return Vec::new();
    };
    match descriptor.field {
        PropertyFieldId::SourceReference => {
            vec![PropertyReferenceValue::Source(document.source().clone())]
        }
        PropertyFieldId::DefinitionSelection => document
            .pattern_definitions()
            .iter()
            .map(|definition| PropertyReferenceValue::Definition(definition.id))
            .collect(),
        PropertyFieldId::OutputSiteProduct => definition_for_target(document, descriptor.target)
            .map(|definition| {
                definition
                    .mechanisms
                    .iter()
                    .map(|mechanism| PropertyReferenceValue::Mechanism(mechanism.id()))
                    .collect()
            })
            .unwrap_or_default(),
        PropertyFieldId::OutputOrientationDimension => {
            guide_dimensions_for_target(document, descriptor.target)
                .into_iter()
                .map(PropertyReferenceValue::GuideDimension)
                .collect()
        }
        _ => Vec::new(),
    }
}

fn reference_collection_choices(
    state: &Rc<RefCell<AppState>>,
    descriptor: &PropertyDescriptor,
) -> Vec<PropertyReferenceValue> {
    let binding = state.borrow();
    let Some(document) = binding.workspace.as_ref().map(Workspace::document) else {
        return Vec::new();
    };
    match descriptor.field {
        PropertyFieldId::IntersectionDimensions | PropertyFieldId::AlongGuideDimensions => {
            guide_dimensions_for_target(document, descriptor.target)
                .into_iter()
                .map(PropertyReferenceValue::GuideDimension)
                .collect()
        }
        _ => Vec::new(),
    }
}

fn record_inspector_draft(
    state: &Rc<RefCell<AppState>>,
    descriptor: &PropertyDescriptor,
    text: String,
    message: &str,
) {
    let mut state = state.borrow_mut();
    state
        .inspector_runtime
        .drafts
        .insert(inspector_key(descriptor), text);
    set_inspector_status(&mut state, message);
}

fn remember_inspector_draft(
    state: &Rc<RefCell<AppState>>,
    descriptor: &PropertyDescriptor,
    text: String,
) {
    state
        .borrow_mut()
        .inspector_runtime
        .drafts
        .insert(inspector_key(descriptor), text);
}

fn clear_transition_drafts(runtime: &mut InspectorRuntime) {
    runtime
        .drafts
        .retain(|key, _| !key.starts_with("transition:"));
    runtime.transition = None;
}

fn begin_variant_transition(
    state: &Rc<RefCell<AppState>>,
    descriptor: PropertyDescriptor,
    choice: PropertyEnumChoice,
) {
    let result = {
        let state = state.borrow();
        state
            .workspace
            .as_ref()
            .ok_or_else(|| "No document is open.".to_owned())
            .and_then(|workspace| {
                workspace
                    .document()
                    .variant_transition_draft(&descriptor, choice)
                    .map_err(|error| error.to_string())
            })
    };
    match result {
        Ok(transition) => {
            let mut app_state = state.borrow_mut();
            clear_transition_drafts(&mut app_state.inspector_runtime);
            app_state.inspector_runtime.transition = Some(transition);
            app_state.inspector_runtime.focus = Some(focus_for_descriptor(&descriptor, None));
            set_inspector_status(
                &mut app_state,
                "Complete the visible transition values, then confirm one atomic structural edit.",
            );
            drop(app_state);
            rebuild_inspector(state);
        }
        Err(error) => {
            let mut state = state.borrow_mut();
            set_inspector_status(&mut state, error);
        }
    }
}

fn command_for_transition_draft(
    document: &Document,
    selected_channel: Option<ChannelId>,
    scope: DefinitionEditScope,
    transition: &VariantTransitionDraft,
) -> Result<DocumentCommand, String> {
    let edit = transition
        .finalize(document)
        .map_err(|error| error.to_string())?;
    let definition_id = target_definition_id(transition.selector().target)
        .ok_or_else(|| "A transition selector must target a pattern definition.".to_owned())?;
    let base_definition = document
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == definition_id)
        .cloned()
        .ok_or_else(|| "The active definition is missing.".to_owned())?;
    match scope {
        DefinitionEditScope::SelectedCopy => {
            Ok(DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: selected_channel
                    .ok_or_else(|| "Select a channel before editing its definition.".to_owned())?,
                base_definition,
                edit,
            })
        }
        DefinitionEditScope::Shared => Ok(DocumentCommand::EditSharedPatternDefinition {
            definition_id,
            base_definition,
            edit,
        }),
    }
}

fn commit_transition_draft(state: &Rc<RefCell<AppState>>) {
    let result = {
        let state = state.borrow();
        let Some(workspace) = state.workspace.as_ref() else {
            return;
        };
        let Some(transition) = state.inspector_runtime.transition.as_ref() else {
            return;
        };
        command_for_transition_draft(
            workspace.document(),
            state.inspector_runtime.selected_channel,
            state.inspector_runtime.scope,
            transition,
        )
    };
    match result {
        Ok(command) => {
            let selector_focus = {
                let state = state.borrow();
                state
                    .inspector_runtime
                    .transition
                    .as_ref()
                    .map(|draft| focus_for_descriptor(draft.selector(), None))
            };
            if apply_inspector_command(state, &command, None, true, selector_focus) {
                rebuild_inspector(state);
            }
        }
        Err(error) => {
            let mut state = state.borrow_mut();
            set_inspector_status(&mut state, error);
        }
    }
}

fn set_preview_pending(state: &mut AppState) {
    state.preview_target = None;
    state.preview_submission = None;
    set_page(state, page_while_preview_pending(state.preview.is_some()));
}

const fn page_while_preview_pending(has_accepted_preview: bool) -> Page {
    if has_accepted_preview {
        Page::Success
    } else {
        Page::Loading
    }
}

const fn page_after_preview_error(has_accepted_preview: bool) -> Page {
    if has_accepted_preview {
        Page::Success
    } else {
        Page::Error
    }
}

fn apply_inspector_command(
    state: &Rc<RefCell<AppState>>,
    command: &DocumentCommand,
    accepted_descriptor: Option<&PropertyDescriptor>,
    accepted_transition: bool,
    focus: Option<InspectorFocusIdentity>,
) -> bool {
    let mut state = state.borrow_mut();
    let Some(workspace) = state.workspace.as_mut() else {
        return false;
    };
    match workspace.history.apply(command) {
        Ok(_) => {
            if let Some(descriptor) = accepted_descriptor {
                state
                    .inspector_runtime
                    .drafts
                    .remove(&inspector_key(descriptor));
            }
            if accepted_transition {
                clear_transition_drafts(&mut state.inspector_runtime);
            }
            state.inspector_runtime.focus =
                focus_after_command_attempt(state.inspector_runtime.focus.clone(), focus, true);
            set_preview_pending(&mut state);
            set_inspector_status(&mut state, "Applied. Preview update is pending.");
            sync_ui(&mut state);
            true
        }
        Err(error) => {
            state.inspector_runtime.focus =
                focus_after_command_attempt(state.inspector_runtime.focus.clone(), focus, false);
            set_inspector_status(&mut state, error.to_string());
            false
        }
    }
}

fn definition_for_target(
    document: &Document,
    target: PropertyTarget,
) -> Option<&toniator_domain::PatternDefinition> {
    let definition_id = match target {
        PropertyTarget::Definition(id)
        | PropertyTarget::Mechanism(id, _)
        | PropertyTarget::OutputLayer(id, _)
        | PropertyTarget::GuideDimension(id, _, _) => id,
        PropertyTarget::Document | PropertyTarget::Channel(_) => return None,
    };
    document
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == definition_id)
}

fn guide_dimensions_for_target(
    document: &Document,
    target: PropertyTarget,
) -> Vec<toniator_domain::GuideDimensionId> {
    definition_for_target(document, target)
        .into_iter()
        .flat_map(|definition| definition.mechanisms.iter())
        .flat_map(|mechanism| match mechanism {
            toniator_domain::PatternMechanism::StraightGuideDimensions { dimensions, .. } => {
                dimensions
                    .iter()
                    .map(|dimension| dimension.id)
                    .collect::<Vec<_>>()
            }
            _ => Vec::new(),
        })
        .collect()
}

fn target_definition_id(target: PropertyTarget) -> Option<PatternDefinitionId> {
    match target {
        PropertyTarget::Definition(id)
        | PropertyTarget::Mechanism(id, _)
        | PropertyTarget::OutputLayer(id, _)
        | PropertyTarget::GuideDimension(id, _, _) => Some(id),
        PropertyTarget::Document | PropertyTarget::Channel(_) => None,
    }
}

fn target_mechanism_id(target: PropertyTarget) -> Option<PatternMechanismId> {
    match target {
        PropertyTarget::Mechanism(_, id) | PropertyTarget::GuideDimension(_, id, _) => Some(id),
        _ => None,
    }
}

fn target_output_layer_id(target: PropertyTarget) -> Option<PatternOutputLayerId> {
    match target {
        PropertyTarget::OutputLayer(_, id) => Some(id),
        _ => None,
    }
}

fn commit_inspector_input_with_focus(
    state: &Rc<RefCell<AppState>>,
    descriptor: PropertyDescriptor,
    input: InspectorInput,
    focus: InspectorFocusIdentity,
) {
    let command = {
        let state = state.borrow();
        let Some(workspace) = state.workspace.as_ref() else {
            return;
        };
        command_for_inspector_input(
            workspace.document(),
            state.inspector_runtime.selected_channel,
            state.inspector_runtime.scope,
            &descriptor,
            input,
        )
    };
    match command {
        Ok(command) => {
            if apply_inspector_command(state, &command, Some(&descriptor), false, Some(focus)) {
                rebuild_inspector(state);
            }
        }
        Err(error) => {
            let mut state = state.borrow_mut();
            set_inspector_status(&mut state, error);
        }
    }
}

fn command_for_inspector_input(
    document: &Document,
    selected_channel: Option<ChannelId>,
    scope: DefinitionEditScope,
    descriptor: &PropertyDescriptor,
    input: InspectorInput,
) -> Result<DocumentCommand, String> {
    let channel_id = match descriptor.target {
        PropertyTarget::Channel(id) => Some(id),
        _ => None,
    };
    let f64_value = |input: InspectorInput| match input {
        InspectorInput::FiniteF64(value) if value.is_finite() => Ok(value),
        _ => Err("Expected a finite numeric value.".to_owned()),
    };
    let boolean = |input: InspectorInput| match input {
        InspectorInput::Boolean(value) => Ok(value),
        _ => Err("Expected a Boolean value.".to_owned()),
    };
    let choice = |input: InspectorInput| match input {
        InspectorInput::EnumChoice(value) => Ok(value),
        _ => Err("Expected a typed choice.".to_owned()),
    };
    let reference = |input: InspectorInput| match input {
        InspectorInput::Reference(value) => Ok(value),
        _ => Err("Expected a stable reference.".to_owned()),
    };
    let channel_id =
        || channel_id.ok_or_else(|| "This field requires a selected channel.".to_owned());
    match descriptor.field {
        PropertyFieldId::DensityAcrossX => Ok(DocumentCommand::SetDensityAxis {
            channel_id: channel_id()?,
            edited_axis: DensityEditedAxis::AcrossX,
            value: f64_value(input)?,
        }),
        PropertyFieldId::DensityAcrossY => Ok(DocumentCommand::SetDensityAxis {
            channel_id: channel_id()?,
            edited_axis: DensityEditedAxis::AcrossY,
            value: f64_value(input)?,
        }),
        PropertyFieldId::DensityAspectLocked => Ok(DocumentCommand::SetDensityAspectLock {
            channel_id: channel_id()?,
            aspect_locked: boolean(input)?,
        }),
        PropertyFieldId::RotationDegrees => Ok(DocumentCommand::SetRotation {
            channel_id: channel_id()?,
            rotation_degrees: f64_value(input)?,
        }),
        PropertyFieldId::TranslationX => Ok(DocumentCommand::SetTranslationAxis {
            channel_id: channel_id()?,
            edited_axis: TranslationEditedAxis::X,
            value: f64_value(input)?,
        }),
        PropertyFieldId::TranslationY => Ok(DocumentCommand::SetTranslationAxis {
            channel_id: channel_id()?,
            edited_axis: TranslationEditedAxis::Y,
            value: f64_value(input)?,
        }),
        PropertyFieldId::MarkMinimumSize => Ok(DocumentCommand::SetMarkGeometryField {
            channel_id: channel_id()?,
            edit: MarkGeometryFieldEdit::MinimumSize(f64_value(input)?),
        }),
        PropertyFieldId::MarkMaximumSize => Ok(DocumentCommand::SetMarkGeometryField {
            channel_id: channel_id()?,
            edit: MarkGeometryFieldEdit::MaximumSize(f64_value(input)?),
        }),
        PropertyFieldId::ColorRed => Ok(DocumentCommand::SetColorComponent {
            channel_id: channel_id()?,
            component: ColorComponent::Red,
            value: f64_value(input)?,
        }),
        PropertyFieldId::ColorGreen => Ok(DocumentCommand::SetColorComponent {
            channel_id: channel_id()?,
            component: ColorComponent::Green,
            value: f64_value(input)?,
        }),
        PropertyFieldId::ColorBlue => Ok(DocumentCommand::SetColorComponent {
            channel_id: channel_id()?,
            component: ColorComponent::Blue,
            value: f64_value(input)?,
        }),
        PropertyFieldId::ColorAlpha => Ok(DocumentCommand::SetColorComponent {
            channel_id: channel_id()?,
            component: ColorComponent::Alpha,
            value: f64_value(input)?,
        }),
        PropertyFieldId::Opacity => Ok(DocumentCommand::SetOpacity {
            channel_id: channel_id()?,
            opacity: f64_value(input)?,
        }),
        PropertyFieldId::Visibility => Ok(DocumentCommand::SetVisibility {
            channel_id: channel_id()?,
            visible: boolean(input)?,
        }),
        PropertyFieldId::SourceReference => match reference(input)? {
            PropertyReferenceValue::Source(source) => {
                Ok(DocumentCommand::SetSourceReference { source })
            }
            _ => Err("Expected a source reference.".to_owned()),
        },
        PropertyFieldId::DefinitionSelection => match reference(input)? {
            PropertyReferenceValue::Definition(definition_id) => {
                Ok(DocumentCommand::RetargetChannelPatternDefinition {
                    channel_id: channel_id()?,
                    definition_id,
                })
            }
            _ => Err("Expected a pattern definition reference.".to_owned()),
        },
        PropertyFieldId::LegacyMappingComponent => match choice(input)? {
            PropertyEnumChoice::SourceMappingComponent(component) => {
                Ok(DocumentCommand::SetLegacyMappingField {
                    channel_id: channel_id()?,
                    edit: LegacyMappingFieldEdit::Component(match component {
                        SourceMappingComponent::Luminance => SourceComponent::Luminance,
                        SourceMappingComponent::Alpha => SourceComponent::Alpha,
                        _ => return Err("Legacy mapping supports Luminance or Alpha.".to_owned()),
                    }),
                })
            }
            _ => Err("Expected a mapping component.".to_owned()),
        },
        PropertyFieldId::LegacyMappingPlacement => match choice(input)? {
            PropertyEnumChoice::SourcePlacement(placement) => {
                Ok(DocumentCommand::SetLegacyMappingField {
                    channel_id: channel_id()?,
                    edit: LegacyMappingFieldEdit::Placement(placement),
                })
            }
            _ => Err("Expected a mapping placement.".to_owned()),
        },
        PropertyFieldId::ModeledMappingComponent => match choice(input)? {
            PropertyEnumChoice::SourceMappingComponent(component) => {
                Ok(DocumentCommand::SetModeledMappingField {
                    channel_id: channel_id()?,
                    edit: ModeledMappingFieldEdit::Component(component),
                })
            }
            _ => Err("Expected a mapping component.".to_owned()),
        },
        PropertyFieldId::ModeledMappingPlacement => match choice(input)? {
            PropertyEnumChoice::SourcePlacement(placement) => {
                Ok(DocumentCommand::SetModeledMappingField {
                    channel_id: channel_id()?,
                    edit: ModeledMappingFieldEdit::Placement(placement),
                })
            }
            _ => Err("Expected a mapping placement.".to_owned()),
        },
        PropertyFieldId::ModeledMappingInverted => Ok(DocumentCommand::SetModeledMappingField {
            channel_id: channel_id()?,
            edit: ModeledMappingFieldEdit::Inverted(boolean(input)?),
        }),
        PropertyFieldId::ModeledMappingGain => Ok(DocumentCommand::SetModeledMappingField {
            channel_id: channel_id()?,
            edit: ModeledMappingFieldEdit::Gain(f64_value(input)?),
        }),
        PropertyFieldId::ModeledMappingBias => Ok(DocumentCommand::SetModeledMappingField {
            channel_id: channel_id()?,
            edit: ModeledMappingFieldEdit::Bias(f64_value(input)?),
        }),
        PropertyFieldId::Paint => match choice(input)? {
            PropertyEnumChoice::Paint(PaintKind::SampledSource) => {
                Ok(DocumentCommand::SetChannelPaint {
                    channel_id: channel_id()?,
                    paint: ChannelPaint::SampledSource,
                })
            }
            PropertyEnumChoice::Paint(PaintKind::Solid) => {
                let color = document
                    .modeled_channel(channel_id()?)
                    .and_then(|channel| match &channel.paint {
                        ChannelPaint::Solid(color) => Some(color.clone()),
                        ChannelPaint::SampledSource => None,
                    })
                    .unwrap_or(toniator_domain::ColorValue {
                        red: 0.0,
                        green: 0.0,
                        blue: 0.0,
                        alpha: 1.0,
                    });
                Ok(DocumentCommand::SetChannelPaint {
                    channel_id: channel_id()?,
                    paint: ChannelPaint::Solid(color),
                })
            }
            _ => Err("Expected a paint choice.".to_owned()),
        },
        _ => structural_command_for_input(document, selected_channel, scope, descriptor, input),
    }
}

fn structural_command_for_input(
    document: &Document,
    selected_channel: Option<ChannelId>,
    scope: DefinitionEditScope,
    descriptor: &PropertyDescriptor,
    input: InspectorInput,
) -> Result<DocumentCommand, String> {
    let definition_id = target_definition_id(descriptor.target)
        .ok_or_else(|| "This field is not structural.".to_owned())?;
    let mechanism_id = target_mechanism_id(descriptor.target);
    let output_layer_id = target_output_layer_id(descriptor.target);
    let number = |input: InspectorInput| match input {
        InspectorInput::FiniteF64(value) if value.is_finite() => Ok(value),
        _ => Err("Expected a finite numeric value.".to_owned()),
    };
    let count = |input: InspectorInput| match input {
        InspectorInput::U32(value) => Ok(value),
        _ => Err("Expected a whole number.".to_owned()),
    };
    let boolean = |input: InspectorInput| match input {
        InspectorInput::Boolean(value) => Ok(value),
        _ => Err("Expected a Boolean value.".to_owned()),
    };
    let choice = |input: InspectorInput| match input {
        InspectorInput::EnumChoice(value) => Ok(value),
        _ => Err("Expected a typed choice.".to_owned()),
    };
    let reference = |input: InspectorInput| match input {
        InspectorInput::Reference(value) => Ok(value),
        _ => Err("Expected a stable reference.".to_owned()),
    };
    let mechanism_id =
        || mechanism_id.ok_or_else(|| "This field requires an active mechanism.".to_owned());
    let output_layer_id =
        || output_layer_id.ok_or_else(|| "This field requires an active output layer.".to_owned());
    let edit = match descriptor.field {
        PropertyFieldId::CoverageGuardSteps => PatternDefinitionEdit::SetCoverageGuardSteps {
            guard_steps: count(input)?,
        },
        PropertyFieldId::CoverageMaximumSupportRadius => {
            PatternDefinitionEdit::SetCoverageMaximumSupportRadius {
                maximum_support_radius: number(input)?,
            }
        }
        PropertyFieldId::GuideBaselineAngle => PatternDefinitionEdit::SetGuideBaselineAngle {
            mechanism_id: mechanism_id()?,
            dimension_id: match descriptor.target {
                PropertyTarget::GuideDimension(_, _, id) => id,
                _ => return Err("Guide dimension target is required.".to_owned()),
            },
            baseline_angle_degrees: number(input)?,
        },
        PropertyFieldId::GuidePhase => PatternDefinitionEdit::SetGuidePhase {
            mechanism_id: mechanism_id()?,
            dimension_id: match descriptor.target {
                PropertyTarget::GuideDimension(_, _, id) => id,
                _ => return Err("Guide dimension target is required.".to_owned()),
            },
            phase: number(input)?,
        },
        PropertyFieldId::GuideSpacingMultiplier => {
            PatternDefinitionEdit::SetGuideSpacingMultiplier {
                mechanism_id: mechanism_id()?,
                dimension_id: match descriptor.target {
                    PropertyTarget::GuideDimension(_, _, id) => id,
                    _ => return Err("Guide dimension target is required.".to_owned()),
                },
                spacing_multiplier: number(input)?,
            }
        }
        PropertyFieldId::IntersectionMergeEpsilon => {
            PatternDefinitionEdit::SetIntersectionMergeEpsilon {
                mechanism_id: mechanism_id()?,
                merge_epsilon: number(input)?,
            }
        }
        PropertyFieldId::AlongGuideIntervalMultiplier => {
            PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
                mechanism_id: mechanism_id()?,
                interval_multiplier: number(input)?,
            }
        }
        PropertyFieldId::AlongGuidePhase => PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id: mechanism_id()?,
            phase: number(input)?,
        },
        PropertyFieldId::RandomSeed => PatternDefinitionEdit::SetRandomSeed {
            mechanism_id: mechanism_id()?,
            seed: count(input)?,
        },
        PropertyFieldId::RandomEvenMinimumCenterDistance => {
            PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
                mechanism_id: mechanism_id()?,
                minimum_center_distance: number(input)?,
            }
        }
        PropertyFieldId::RandomClusterDensity => PatternDefinitionEdit::SetRandomClusterDensity {
            mechanism_id: mechanism_id()?,
            cluster_density: number(input)?,
        },
        PropertyFieldId::RandomClusterSpread => PatternDefinitionEdit::SetRandomClusterSpread {
            mechanism_id: mechanism_id()?,
            cluster_spread: number(input)?,
        },
        PropertyFieldId::RandomClusterStrength => PatternDefinitionEdit::SetRandomClusterStrength {
            mechanism_id: mechanism_id()?,
            cluster_strength: number(input)?,
        },
        PropertyFieldId::ArtworkWeightMappingInverted => {
            PatternDefinitionEdit::SetArtworkWeightMappingInverted {
                mechanism_id: mechanism_id()?,
                inverted: boolean(input)?,
            }
        }
        PropertyFieldId::ArtworkWeightMappingGain => {
            PatternDefinitionEdit::SetArtworkWeightMappingGain {
                mechanism_id: mechanism_id()?,
                gain: number(input)?,
            }
        }
        PropertyFieldId::ArtworkWeightMappingBias => {
            PatternDefinitionEdit::SetArtworkWeightMappingBias {
                mechanism_id: mechanism_id()?,
                bias: number(input)?,
            }
        }
        PropertyFieldId::ArtworkWeightStrength => PatternDefinitionEdit::SetArtworkWeightStrength {
            mechanism_id: mechanism_id()?,
            strength: number(input)?,
        },
        PropertyFieldId::ExclusionMinimumCenterDistance => {
            PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
                mechanism_id: mechanism_id()?,
                minimum_center_distance: number(input)?,
            }
        }
        PropertyFieldId::VisibleMarkMargin => PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id: mechanism_id()?,
            margin: number(input)?,
        },
        PropertyFieldId::RandomMaximumAttempts => PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id: mechanism_id()?,
            maximum_attempts: count(input)?,
        },
        PropertyFieldId::RandomMaximumNeighborChecks => {
            PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
                mechanism_id: mechanism_id()?,
                maximum_neighbor_checks: count(input)?,
            }
        }
        PropertyFieldId::OutputSiteProduct => match reference(input)? {
            PropertyReferenceValue::Mechanism(site_mechanism_id) => {
                PatternDefinitionEdit::SetOutputSiteProduct {
                    output_layer_id: output_layer_id()?,
                    site_mechanism_id,
                }
            }
            _ => return Err("Expected a site-product mechanism reference.".to_owned()),
        },
        PropertyFieldId::OutputOrientationDimension => match reference(input)? {
            PropertyReferenceValue::GuideDimension(dimension_id) => {
                PatternDefinitionEdit::SetOutputOrientationDimension {
                    output_layer_id: output_layer_id()?,
                    dimension_id,
                }
            }
            _ => return Err("Expected a guide dimension reference.".to_owned()),
        },
        PropertyFieldId::RandomCharacter
        | PropertyFieldId::RandomDensityModulation
        | PropertyFieldId::RandomExclusion
        | PropertyFieldId::OutputOrientation => {
            return Err(
                "This compound selector requires the visible transition draft and confirmation."
                    .to_owned(),
            );
        }
        PropertyFieldId::ArtworkWeightMappingComponent => match choice(input)? {
            PropertyEnumChoice::SourceMappingComponent(component) => {
                PatternDefinitionEdit::SetArtworkWeightMappingComponent {
                    mechanism_id: mechanism_id()?,
                    component,
                }
            }
            _ => return Err("Expected a mapping component.".to_owned()),
        },
        PropertyFieldId::ArtworkWeightMappingPlacement => match choice(input)? {
            PropertyEnumChoice::SourcePlacement(placement) => {
                PatternDefinitionEdit::SetArtworkWeightMappingPlacement {
                    mechanism_id: mechanism_id()?,
                    placement,
                }
            }
            _ => return Err("Expected a mapping placement.".to_owned()),
        },
        PropertyFieldId::ArtworkWeightResponse => match choice(input)? {
            PropertyEnumChoice::ArtworkWeightResponse(response) => {
                PatternDefinitionEdit::SetArtworkWeightResponse {
                    mechanism_id: mechanism_id()?,
                    response,
                }
            }
            _ => return Err("Expected an artwork response choice.".to_owned()),
        },
        PropertyFieldId::VisibleMarkSizingPolicy => match choice(input)? {
            PropertyEnumChoice::VisibleMarkSizingPolicy(sizing) => {
                PatternDefinitionEdit::SetVisibleMarkSizingPolicy {
                    mechanism_id: mechanism_id()?,
                    sizing,
                }
            }
            _ => return Err("Expected a visible-mark sizing choice.".to_owned()),
        },
        PropertyFieldId::OutputPrototype => match choice(input)? {
            PropertyEnumChoice::MarkPrototype(_) => PatternDefinitionEdit::SetOutputMarkPrototype {
                output_layer_id: output_layer_id()?,
                prototype: MarkPrototype::Circle,
            },
            _ => return Err("Expected an output prototype choice.".to_owned()),
        },
        PropertyFieldId::IntersectionDimensions | PropertyFieldId::AlongGuideDimensions => {
            let InspectorInput::ReferenceCollection(references) = input else {
                return Err("Expected an ordered stable-reference collection.".to_owned());
            };
            let dimensions = references
                .into_iter()
                .map(|reference| match reference {
                    PropertyReferenceValue::GuideDimension(id) => Ok(id),
                    _ => Err("Expected guide dimension references.".to_owned()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if descriptor.field == PropertyFieldId::IntersectionDimensions {
                PatternDefinitionEdit::SetIntersectionDimensions {
                    mechanism_id: mechanism_id()?,
                    dimensions,
                }
            } else {
                PatternDefinitionEdit::SetAlongGuideDimensions {
                    mechanism_id: mechanism_id()?,
                    dimensions,
                }
            }
        }
        _ => return Err("This active descriptor has no compatible Stage 18 route.".to_owned()),
    };
    let base_definition = document
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == definition_id)
        .cloned()
        .ok_or_else(|| "The active definition is missing.".to_owned())?;
    match scope {
        DefinitionEditScope::SelectedCopy => {
            Ok(DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: selected_channel
                    .ok_or_else(|| "Select a channel before editing its definition.".to_owned())?,
                base_definition,
                edit,
            })
        }
        DefinitionEditScope::Shared => Ok(DocumentCommand::EditSharedPatternDefinition {
            definition_id,
            base_definition,
            edit,
        }),
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
        LifecycleAction::Close => {
            clear_workspace(&mut state.borrow_mut());
            rebuild_inspector(state);
        }
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
        state.inspector_runtime.reset_for_workspace();
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

    {
        let mut app_state = state.borrow_mut();
        update_backdrop(&mut app_state);
        clear_preview(&mut app_state);
        app_state.preview_target = None;
        show_source_diagnostic(&mut app_state);
        let page = if app_state
            .workspace
            .as_ref()
            .is_some_and(|workspace| workspace.source_presentation.is_some())
        {
            Page::Loading
        } else {
            Page::Empty
        };
        set_page(&mut app_state, page);
        sync_ui(&mut app_state);
    }
    rebuild_inspector(state);
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
    state.inspector_runtime.reset_for_workspace();
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
    let mut app_state = state.borrow_mut();
    if !should_apply_model_change(
        app_state.syncing_model,
        app_state.model,
        model,
        app_state.pending_load.is_some(),
        app_state
            .workspace
            .as_ref()
            .is_some_and(|workspace| workspace.source_presentation.is_some()),
    ) {
        return;
    }
    let workspace = app_state
        .workspace
        .as_mut()
        .expect("model change requires a workspace");
    if let Err(error) = replace_model_topology(&mut workspace.history, model) {
        show_error(&mut app_state, error);
        return;
    }
    app_state.model = model;
    update_backdrop(&mut app_state);
    set_preview_pending(&mut app_state);
    set_inspector_status(
        &mut app_state,
        "Channel model changed. Preview update is pending.",
    );
    sync_ui(&mut app_state);
    drop(app_state);
    rebuild_inspector(state);
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
    if document.channel_model() == Some(model.domain()) {
        // A command semantic no-op must not manufacture a history revision.
        // The selector already avoids this path for interaction; this also
        // protects programmatic synchronization and lifecycle tests.
        return Ok(());
    }
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
    set_page(state, page_after_preview_error(state.preview.is_some()));
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
    state.actions.undo.set_enabled(policy.undo_enabled);
    state.actions.redo.set_enabled(policy.redo_enabled);
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
    fn semantic_noop_leaves_workspace_lifecycle_state_unchanged() {
        let mut workspace = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        let id = workspace.document().channel_topology().unwrap().channels()[0].id;
        let before = probe(&workspace);
        let can_undo = workspace.history.can_undo();
        let can_redo = workspace.history.can_redo();
        assert!(
            workspace
                .history
                .apply(&DocumentCommand::SetVisibility {
                    channel_id: id,
                    visible: true,
                })
                .is_err()
        );
        assert_eq!(probe(&workspace), before);
        assert_eq!(workspace.history.can_undo(), can_undo);
        assert_eq!(workspace.history.can_redo(), can_redo);
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
                undo_enabled: false,
                redo_enabled: false,
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

    #[test]
    fn inspector_selection_is_stable_id_based_and_has_deterministic_removal_fallback() {
        let ids = [ChannelId(30), ChannelId(10), ChannelId(20)];
        assert_eq!(
            selected_channel_after_transition(Some(ChannelId(10)), &ids),
            Some(ChannelId(10))
        );
        assert_eq!(
            selected_channel_after_transition(Some(ChannelId(99)), &ids),
            Some(ChannelId(30))
        );
        assert_eq!(
            selected_channel_after_transition(None, &ids),
            Some(ChannelId(30))
        );
        assert_eq!(
            selected_channel_after_transition(Some(ChannelId(10)), &[]),
            None
        );
    }

    #[test]
    fn inspector_uses_descriptor_value_pairs_and_typed_history_commands() {
        let mut workspace = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        let channel_id = authoritative_channel_ids(workspace.document())[0];
        let descriptor = workspace
            .document()
            .property_values()
            .into_iter()
            .find(|value| {
                value.descriptor.target == PropertyTarget::Channel(channel_id)
                    && value.descriptor.field == PropertyFieldId::Opacity
            })
            .unwrap()
            .descriptor;
        let before = workspace.history.revision();
        let command = command_for_inspector_input(
            workspace.document(),
            Some(channel_id),
            DefinitionEditScope::SelectedCopy,
            &descriptor,
            InspectorInput::FiniteF64(0.75),
        )
        .unwrap();
        workspace.history.apply(&command).unwrap();
        assert!(workspace.history.revision() > before);
        workspace.history.undo().unwrap();
        assert!(workspace.history.can_redo());
    }

    #[test]
    fn inspector_runtime_resets_document_scoped_transients_but_not_ordinary_selection_logic() {
        let mut runtime = InspectorRuntime {
            selected_channel: Some(ChannelId(44)),
            scope: DefinitionEditScope::Shared,
            drafts: BTreeMap::from([("opacity".into(), "1.5".into())]),
            expanded_groups: BTreeSet::from(["Active family".into()]),
            transition: None,
            status: Some("domain rejection".into()),
            focus: Some(InspectorFocusIdentity::Descriptor {
                key: "old:focus".into(),
                field: PropertyFieldId::Opacity,
                collection_index: Some(2),
                guide_ordinal: None,
            }),
        };
        runtime.reset_for_workspace();
        assert_eq!(runtime, InspectorRuntime::default());
        assert_eq!(
            selected_channel_after_transition(Some(ChannelId(44)), &[ChannelId(44)]),
            Some(ChannelId(44)),
            "ordinary history edits keep their stable selected ChannelId"
        );
    }

    #[test]
    fn inspector_focus_policy_preserves_rejected_drafts_and_restores_active_controls() {
        let mut workspace = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        let channel_id = authoritative_channel_ids(workspace.document())[0];
        let opacity = workspace
            .document()
            .property_values()
            .into_iter()
            .find(|value| {
                value.descriptor.target == PropertyTarget::Channel(channel_id)
                    && value.descriptor.field == PropertyFieldId::Opacity
            })
            .unwrap()
            .descriptor;
        let opacity_focus = focus_for_descriptor(&opacity, None);
        let collection_focus = focus_for_descriptor(&opacity, Some(3));

        assert_eq!(
            focus_after_command_attempt(
                Some(collection_focus.clone()),
                Some(opacity_focus.clone()),
                false,
            ),
            Some(collection_focus.clone()),
            "a rejected/no-op command leaves the focused invalid draft in place"
        );
        assert_eq!(
            focus_after_command_attempt(Some(collection_focus), Some(opacity_focus.clone()), true,),
            Some(opacity_focus.clone()),
            "an accepted command requests focus for its initiating control"
        );
        assert_eq!(
            resolve_focus_after_document_change(
                workspace.document(),
                Some(channel_id),
                Some(opacity_focus.clone()),
            ),
            Some(opacity_focus.clone()),
            "ordinary apply/undo/redo rebuilds retain an active descriptor target"
        );
        let edit = command_for_inspector_input(
            workspace.document(),
            Some(channel_id),
            DefinitionEditScope::SelectedCopy,
            &opacity,
            InspectorInput::FiniteF64(0.75),
        )
        .unwrap();
        workspace.history.apply(&edit).unwrap();
        assert_eq!(
            resolve_focus_after_document_change(
                workspace.document(),
                Some(channel_id),
                Some(opacity_focus.clone()),
            ),
            Some(opacity_focus.clone()),
            "accepted numeric edits restore the initiating descriptor focus"
        );
        workspace.history.undo().unwrap();
        assert_eq!(
            resolve_focus_after_document_change(
                workspace.document(),
                Some(channel_id),
                Some(opacity_focus.clone()),
            ),
            Some(opacity_focus.clone()),
            "undo restores the same frontend focus identity deterministically"
        );
        workspace.history.redo().unwrap();
        assert_eq!(
            resolve_focus_after_document_change(
                workspace.document(),
                Some(channel_id),
                Some(opacity_focus.clone()),
            ),
            Some(opacity_focus.clone()),
            "redo restores the same frontend focus identity deterministically"
        );

        let stale_definition_key = InspectorFocusIdentity::Descriptor {
            key: "retargeted-definition:random-seed".into(),
            field: PropertyFieldId::Opacity,
            collection_index: Some(3),
            guide_ordinal: None,
        };
        assert_eq!(
            resolve_focus_after_document_change(
                workspace.document(),
                Some(channel_id),
                Some(stale_definition_key),
            ),
            Some(focus_for_descriptor(&opacity, Some(3))),
            "a selected-copy target may remap only to the same active field and collection item"
        );
        assert_eq!(
            selected_channel_after_transition(Some(channel_id), &[]),
            None,
            "topology removal has no surviving channel to focus"
        );
    }

    #[test]
    fn transition_focus_identity_is_transient_and_never_fabricates_a_descriptor_target() {
        let transition_focus = InspectorFocusIdentity::TransitionField {
            field: PropertyFieldId::RandomCharacter,
            target: PropertyTarget::Document,
        };
        assert_eq!(
            focus_after_command_attempt(None, Some(transition_focus.clone()), true),
            Some(transition_focus.clone())
        );
        assert!(focus_matches(
            &Some(transition_focus.clone()),
            &transition_focus
        ));
        let mut runtime = InspectorRuntime {
            focus: Some(transition_focus),
            ..InspectorRuntime::default()
        };
        runtime.reset_for_workspace();
        assert!(
            runtime.focus.is_none(),
            "workspace replacement clears focus identity"
        );
    }

    #[test]
    fn selected_copy_remaps_repeated_guide_scalars_by_authoritative_ordinal_only() {
        let workspace = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        let guide_scalar = workspace
            .document()
            .property_values()
            .into_iter()
            .next()
            .expect("frozen fixture has an active property to clone for pure focus policy");
        let values_for = |definition, mechanism, dimensions: [u64; 3]| {
            dimensions.map(|dimension| {
                let mut value = guide_scalar.clone();
                value.descriptor.field = PropertyFieldId::GuidePhase;
                value.descriptor.target = PropertyTarget::GuideDimension(
                    PatternDefinitionId(definition),
                    PatternMechanismId(mechanism),
                    toniator_domain::GuideDimensionId(dimension),
                );
                value
            })
        };
        let before = values_for(10, 20, [31, 32, 33]);
        let third_focus = focus_for_descriptor_in_values(&before, &before[2].descriptor, None);
        let after = values_for(40, 50, [61, 62, 63]);

        assert_eq!(
            resolve_descriptor_focus(&after, Some(third_focus)),
            Some(focus_for_descriptor_in_values(
                &after,
                &after[2].descriptor,
                None,
            )),
            "fresh selected-copy IDs remap the third repeated guide scalar to the third target"
        );
        assert_eq!(
            resolve_descriptor_focus(
                &after[..2],
                Some(focus_for_descriptor_in_values(
                    &before,
                    &before[2].descriptor,
                    None,
                ))
            ),
            None,
            "if the corresponding guide target is gone, focus clears instead of choosing the first"
        );
    }

    #[test]
    fn compound_selectors_use_the_domain_transition_draft_before_one_typed_history_command() {
        let mut workspace = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        let channel_id = authoritative_channel_ids(workspace.document())[0];
        workspace
            .history
            .apply(&DocumentCommand::AddTypedPatternDefinition {
                definition: toniator_domain::PatternDefinition::random_sites(
                    PatternDefinitionId(50),
                    "app transition test",
                    PatternMechanismId(60),
                    PatternMechanismId(61),
                    PatternMechanismId(62),
                    PatternMechanismId(63),
                    PatternOutputLayerId(70),
                    toniator_domain::RandomSiteCharacter::RawUniform,
                    17,
                    toniator_domain::SiteDensityModulation::Uniform,
                    toniator_domain::SiteExclusionPolicy::None,
                    1_000,
                    2_000,
                    toniator_domain::CoveragePolicy {
                        guard_steps: 3,
                        maximum_support_radius: 8.0,
                    },
                ),
            })
            .unwrap();
        workspace
            .history
            .apply(&DocumentCommand::RetargetChannelPatternDefinition {
                channel_id,
                definition_id: PatternDefinitionId(50),
            })
            .unwrap();
        for (field, choices) in [
            (
                PropertyFieldId::RandomCharacter,
                vec![
                    PropertyEnumChoice::RandomCharacter(RandomCharacterKind::RawUniform),
                    PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
                    PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Clustered),
                ],
            ),
            (
                PropertyFieldId::RandomDensityModulation,
                vec![
                    PropertyEnumChoice::DensityModulation(
                        toniator_domain::DensityModulationKind::Uniform,
                    ),
                    PropertyEnumChoice::DensityModulation(
                        toniator_domain::DensityModulationKind::ArtworkWeighted,
                    ),
                ],
            ),
            (
                PropertyFieldId::RandomExclusion,
                vec![
                    PropertyEnumChoice::Exclusion(toniator_domain::ExclusionKind::None),
                    PropertyEnumChoice::Exclusion(
                        toniator_domain::ExclusionKind::MinimumCenterDistance,
                    ),
                    PropertyEnumChoice::Exclusion(
                        toniator_domain::ExclusionKind::VisibleMarkMargin,
                    ),
                ],
            ),
        ] {
            let selector = workspace
                .document()
                .property_values()
                .into_iter()
                .find(|value| value.descriptor.field == field)
                .unwrap()
                .descriptor;
            for choice in choices {
                let draft = workspace
                    .document()
                    .variant_transition_draft(&selector, choice)
                    .unwrap();
                assert!(draft.fields().iter().all(|field| matches!(
                    transition_control_route(&field.value),
                    InspectorControlRoute::FiniteNumber
                        | InspectorControlRoute::WholeNumber
                        | InspectorControlRoute::Toggle
                        | InspectorControlRoute::Choice
                        | InspectorControlRoute::Reference
                )));
            }
        }
        let descriptor = workspace
            .document()
            .property_values()
            .into_iter()
            .find(|value| value.descriptor.field == PropertyFieldId::RandomCharacter)
            .unwrap()
            .descriptor;
        assert!(is_transition_selector(descriptor.field));
        assert!(
            command_for_inspector_input(
                workspace.document(),
                Some(channel_id),
                DefinitionEditScope::SelectedCopy,
                &descriptor,
                InspectorInput::EnumChoice(PropertyEnumChoice::RandomCharacter(
                    RandomCharacterKind::Even,
                )),
            )
            .is_err()
        );
        let transition = workspace
            .document()
            .variant_transition_draft(
                &descriptor,
                PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
            )
            .unwrap();
        let field = transition.fields().first().unwrap().clone();
        let transition = transition
            .with_updates(&[VariantTransitionFieldUpdate {
                field: field.field,
                target: field.target,
                value: VariantTransitionValue::FiniteF64(3.25),
            }])
            .unwrap();
        let command = command_for_transition_draft(
            workspace.document(),
            Some(channel_id),
            DefinitionEditScope::SelectedCopy,
            &transition,
        )
        .unwrap();
        let before = workspace.history.revision();
        workspace.history.apply(&command).unwrap();
        assert!(workspace.history.revision() > before);
        workspace.history.undo().unwrap();
        assert!(workspace.history.can_redo());
    }

    #[test]
    fn inspector_disclosure_metadata_and_preview_pages_preserve_accepted_content() {
        assert!(is_advanced_descriptor(PropertyFieldId::RandomSeed));
        assert!(!is_advanced_descriptor(PropertyFieldId::Opacity));
        assert_eq!(
            field_detail(
                Some(toniator_domain::PropertyBounds {
                    minimum: Some(0.0),
                    minimum_inclusive: true,
                    maximum: None,
                    maximum_inclusive: true,
                }),
                toniator_domain::PropertyUnit::Count,
            ),
            "≥0; Count"
        );
        assert_eq!(page_while_preview_pending(true), Page::Success);
        assert_eq!(page_while_preview_pending(false), Page::Loading);
        assert_eq!(page_after_preview_error(true), Page::Success);
        assert_eq!(page_after_preview_error(false), Page::Error);
    }

    #[test]
    fn every_active_descriptor_value_has_one_generic_control_route() {
        for input in ["raster-sample-v1.toniator", "vector-sample-v1.toniator"] {
            let workspace = load_workspace(&asset(input)).unwrap();
            assert!(
                workspace
                    .document()
                    .property_values()
                    .iter()
                    .all(|value| control_route(value).is_some())
            );
        }
        for model in [
            PreviewModel::Rgb,
            PreviewModel::Cmyk,
            PreviewModel::SourceColorAlpha,
        ] {
            let mut workspace = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
            replace_model_topology(&mut workspace.history, model).unwrap();
            assert!(
                workspace
                    .document()
                    .property_values()
                    .iter()
                    .all(|value| control_route(value).is_some()),
                "every active descriptor/value pair routes for {model:?}"
            );
        }
        let implementation = include_str!("main.rs");
        let inspector_source = implementation
            .split("fn authoritative_channel_ids")
            .nth(1)
            .and_then(|source| source.split("fn centered").next())
            .unwrap();
        for prohibited in [
            "write_svg(",
            "rasterize_output(",
            "DocumentDtoV1",
            "serde_json",
        ] {
            assert!(
                !inspector_source.contains(prohibited),
                "GTK inspector must not own {prohibited}"
            );
        }
        for prohibited in [
            "random_character_for(",
            "modulation_for(",
            "exclusion_for(",
            "orientation_for(",
            ".first()\n            .copied(),",
        ] {
            assert!(
                !inspector_source.contains(prohibited),
                "compound selectors must use the domain transition draft, not {prohibited}"
            );
        }
    }
}
