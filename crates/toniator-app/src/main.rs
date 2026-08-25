#![forbid(unsafe_code)]

//! GTK document lifecycle around the headless document, history, engine, and
//! portable-container boundaries.  The workspace below is controller state;
//! `DocumentHistory` remains the only mutable document authority.

mod app_events;
mod application_model;
mod automation;
mod components;
mod controller;
mod preview_coordinator;
mod stage20f_editor;
mod view_models;

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use adw::prelude::*;
use app_events::AppEvent;
use automation::AutomationSink;
use controller::{UiIntent, history_redo};
#[cfg(test)]
use preview_coordinator::PreviewSubmission;
use preview_coordinator::accepts_submission;
use toniator_domain::{
    AuthoredStructureAttachment, CanvasSpec, ChannelId, ChannelPaint, ChannelTopologyTemplate,
    ColorComponent, DensityEditedAxis, Document, DocumentCommand, DocumentHistory, DocumentSession,
    HalftoneChannelModel, LegacyMappingFieldEdit, MarkGeometryFieldEdit, MarkOrientationKind,
    MarkPrototype, ModeledMappingFieldEdit, PaintKind, PatternDefinitionEdit, PatternDefinitionId,
    PatternGeometryResponse, PatternMechanism, PatternMechanismId, PatternOutputLayerId,
    PropertyCurrentValue, PropertyCurrentValueKind, PropertyDescriptor, PropertyEnumChoice,
    PropertyFieldId, PropertyReferenceValue, PropertyTarget, PropertyValueKind,
    RandomCharacterKind, SourceComponent, SourceMappingComponent, SourceReference,
    SourceReferenceId, TranslationEditedAxis, VariantTransitionFieldUpdate, VariantTransitionValue,
};
#[cfg(test)]
use toniator_domain::{DensityModulationKind, ExclusionKind};
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
use toniator_patterns::PresetRegistry;
use view_models::{LifecycleViewModel, PatternCatalogViewModel, project_document};

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
pub(crate) enum PreviewModel {
    Rgb,
    Cmyk,
    SourceColorAlpha,
}

/// Identifies the private Pattern Editor's selected-copy command boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
enum DefinitionEditScope {
    SelectedCopy,
}

impl Default for DefinitionEditScope {
    /// Starts ordinary structural edits as selected-channel copies.
    fn default() -> Self {
        Self::SelectedCopy
    }
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

/// Preserves the selected stable channel ID when it survives a document
/// transition and otherwise chooses the first authoritative channel. This
/// policy never invents a channel ID and returns `None` for an empty topology.
fn selected_channel_after_transition(
    selected: Option<ChannelId>,
    authoritative_order: &[ChannelId],
) -> Option<ChannelId> {
    selected
        .filter(|id| authoritative_order.contains(id))
        .or_else(|| authoritative_order.first().copied())
}

/// Resolves a GTK selector position through the current authoritative channel
/// order. Invalid GTK positions and positions outside that order are rejected
/// so a notification can never select a stale or fabricated channel.
fn channel_id_at_selector_position(
    position: u32,
    authoritative_order: &[ChannelId],
) -> Option<ChannelId> {
    (position != gtk::INVALID_LIST_POSITION)
        .then_some(position as usize)
        .and_then(|index| authoritative_order.get(index).copied())
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
pub(crate) struct SavedContent {
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
pub(crate) struct Workspace {
    pub(crate) history: DocumentHistory,
    sources: SourceBundle,
    location: Option<PathBuf>,
    pub(crate) display_name: String,
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
            migration_notice: false,
            savepoint: Some(SavedContent { document, sources }),
        })
    }

    fn document(&self) -> &Document {
        self.history.document()
    }

    /// Reports whether mutable document or embedded-source state differs from its savepoint.
    ///
    /// The comparison is read-only and uses the immutable snapshot boundary;
    /// it never writes files or advances history.
    pub(crate) fn is_dirty(&self) -> bool {
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
pub(crate) enum LifecycleAction {
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
pub(crate) enum ExportFormat {
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

/// App-owned lifecycle identity paired with an engine scheduler ticket.  The
/// scheduler remains authoritative for ticket/revision/cache acceptance; this
/// merely prevents two distinct revision-zero workspaces from looking equal to
/// GTK presentation.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletionInstall<T> {
    Install(T),
    Preserve,
}

#[cfg(test)]
fn lifecycle_completion_policy<T>(
    current_generation: u64,
    candidate_generation: u64,
    candidate: T,
) -> CompletionInstall<T> {
    if current_generation == candidate_generation {
        CompletionInstall::Install(candidate)
    } else {
        CompletionInstall::Preserve
    }
}

#[cfg(test)]
fn accepts_preview_submission(
    current_workspace_generation: u64,
    submission: Option<PreviewSubmission>,
    completion_ticket: u64,
) -> bool {
    accepts_submission(current_workspace_generation, submission, completion_ticket)
}

/// Reports whether one private preview completion is the newest terminal result for its draft epoch.
fn accepts_draft_preview_terminal(
    current_epoch: u64,
    pending_ticket: Option<u64>,
    completion_epoch: u64,
    completion_ticket: u64,
) -> bool {
    current_epoch == completion_epoch && pending_ticket == Some(completion_ticket)
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

/// Holds the transient GTK widgets around one private Pattern Editor history.
///
/// The surface presents only a cloned draft document/history and draft preview;
/// it has no route to the main workspace, savepoint, location, or filesystem.
/// Dropping the surface on close discards that private authority.
struct PatternEditorSurface {
    purpose: PatternEditorPurpose,
    window: adw::Window,
    status: gtk::Label,
    picture: gtk::Picture,
    preview_spinner: gtk::Spinner,
    draft: Rc<RefCell<PatternEditorDraft>>,
    introduction: gtk::Label,
    history: gtk::Label,
    current_pattern: gtk::Label,
    primary: gtk::Box,
    advanced_rows: gtk::Box,
    descriptor_components: BTreeMap<String, DraftDescriptorComponent>,
    apply: gtk::Button,
    resource_list: gtk::Box,
    construction_canvas: gtk::DrawingArea,
    coordinate_x: gtk::Entry,
    coordinate_y: gtk::Entry,
    numeric_commit_active: Rc<Cell<bool>>,
    make_curve: gtk::Button,
    make_line: gtk::Button,
    insert_node: gtk::Button,
    delete_node: gtk::Button,
}

/// Identifies the one authored resource and descriptor purpose exposed by a private editor.
///
/// The purpose is GTK presentation policy only. `DocumentHistory` remains the sole authority
/// for every draft mutation, resource attachment, and final publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PatternEditorPurpose {
    Grid,
    Mark,
}

impl PatternEditorPurpose {
    /// Returns the only authored topology this modal may construct or edit.
    const fn structure_kind(self) -> toniator_domain::AuthoredStructureKind {
        match self {
            Self::Grid => toniator_domain::AuthoredStructureKind::OpenPath,
            Self::Mark => toniator_domain::AuthoredStructureKind::ClosedShape,
        }
    }

    /// Returns the artist-facing modal title without exposing implementation IDs.
    const fn title(self) -> &'static str {
        match self {
            Self::Grid => "Grid Pattern Editor",
            Self::Mark => "Mark Editor",
        }
    }

    /// Returns the explicit construction action available in this purpose-specific modal.
    const fn construction_action_label(self) -> &'static str {
        match self {
            Self::Grid => "New guide path",
            Self::Mark => "New mark shape",
        }
    }
}

/// Defines the modal-width breakpoint where construction switches to its vertical fallback.
///
/// The exact pixel value is passed to Adwaita's `MaxWidth` condition; it is not inferred from a
/// child widget's natural allocation.
const NARROW_EDITOR_MAX_WIDTH_PX: f64 = 700.0;

/// Keeps the wide construction resource list legible without preventing the modal breakpoint.
const CONSTRUCTION_SIDEBAR_WIDTH_PX: i32 = 320;

/// Requests only the compact minimum canvas width; wide mode receives surplus width through `hexpand`.
const CONSTRUCTION_CANVAS_MIN_WIDTH_PX: i32 = 220;

/// Names the construction canvas's direct-manipulation gestures without consuming editor height.
const CONSTRUCTION_CANVAS_GESTURE_HINT: &str = "Click to select or add points after choosing New. Scroll to zoom; middle-button drag pans. Enter completes; Escape cancels.";

/// Reports the same inclusive width policy used by the native modal breakpoint.
///
/// This test-only helper keeps the boundary witness aligned with `MaxWidth 700px`; runtime mode
/// changes are emitted by libadwaita rather than by a child allocation notification.
#[cfg(test)]
const fn uses_narrow_editor_layout(width: i32) -> bool {
    width <= NARROW_EDITOR_MAX_WIDTH_PX as i32
}

/// Reports whether the construction widgets' explicit horizontal requests leave room for the breakpoint.
///
/// The shell contributes 24px of horizontal margins and the layout uses a 12px gap. Child natural
/// widths remain GTK authority, but this guards the explicit requests that previously locked the
/// modal above its native `MaxWidth` breakpoint.
#[cfg(test)]
const fn requested_construction_width_allows_narrow_breakpoint() -> bool {
    CONSTRUCTION_SIDEBAR_WIDTH_PX + CONSTRUCTION_CANVAS_MIN_WIDTH_PX + 12 + 24
        < NARROW_EDITOR_MAX_WIDTH_PX as i32
}

/// Retains one private-editor descriptor row and its immutable source value.
///
/// Rows are keyed by descriptor identity so accepted draft edits preserve
/// unaffected widget/focus instances. The row never owns document, scheduler,
/// or savepoint authority.
struct DraftDescriptorComponent {
    row: gtk::Box,
    control: gtk::Widget,
    value: PropertyCurrentValue,
}

/// Owns an intentionally private structural-editing session for one channel.
///
/// Its history starts from a cloned main document and never escapes into the
/// main workspace, savepoint, source bundle, scheduler, or filesystem.
struct PatternEditorDraft {
    history: DocumentHistory,
    selected_channel: ChannelId,
    initial_document: Document,
    discard_confirmed: bool,
    sources: SourceBundle,
    presentation: SourcePresentation,
    scheduler: Arc<EvaluationScheduler>,
    preview_submission: Option<u64>,
    epoch: u64,
    geometry_editor: stage20f_editor::Stage20fEditorState,
    construction_attachment: Option<AuthoredStructureAttachment>,
    pending_shared_edit: Option<PendingSharedPathEdit>,
}

/// Retains a gated private replacement until the artist chooses its shared-resource policy.
///
/// The value stays within the private Pattern Editor and contains no main workspace or preview
/// authority. It is consumed only by an explicit Edit all uses or Make a copy for this use choice.
#[derive(Clone)]
struct PendingSharedPathEdit {
    selected_use: toniator_domain::AuthoredStructureUse,
    base_structure: toniator_domain::AuthoredStructure,
    replacement: toniator_domain::AuthoredStructureDraft,
}

/// Retains one persistent sidebar row and its last immutable descriptor value.
///
/// The component owns GTK presentation only. Its descriptor/value snapshot is
/// replaced from document view data while the row identity and connected
/// callbacks survive ordinary model updates.
struct DescriptorComponent {
    row: gtk::Box,
    control: gtk::Widget,
    value: PropertyCurrentValue,
}

impl PatternEditorDraft {
    /// Reports whether this private editor has changes that a close must disclose.
    fn is_dirty(&self) -> bool {
        self.history.document() != &self.initial_document
    }
}

struct AppState {
    application_model: application_model::ApplicationModel,
    syncing_model: bool,
    window_close: WindowCloseController,
    actions: Actions,
    window: adw::ApplicationWindow,
    window_title: adw::WindowTitle,
    stack: gtk::Stack,
    picture: gtk::Picture,
    viewer: gtk::Overlay,
    preview_spinner: gtk::Spinner,
    error: gtk::Label,
    banner: adw::Banner,
    selector: gtk::DropDown,
    channel_selector: gtk::DropDown,
    channel_selector_model: gtk::StringList,
    inspector_catalog: gtk::Box,
    active_pattern: gtk::Label,
    inspector_descriptors: gtk::Box,
    inspector_status: gtk::Label,
    descriptor_components: BTreeMap<String, DescriptorComponent>,
    inspector_runtime: InspectorRuntime,
    syncing_inspector: bool,
    syncing_draft_editor: bool,
    inspector_rebuild_scheduled: bool,
    pattern_editor: Option<PatternEditorSurface>,
    draft_epoch: u64,
    preview: Option<gtk::gdk::Texture>,
    preview_target: Option<toniator_engine::PreviewRasterTarget>,
    presets: PresetRegistry,
    automation: Option<AutomationSink>,
    event_sender: async_channel::Sender<AppEvent>,
    preview_bridge_stop: Arc<AtomicBool>,
}

impl std::ops::Deref for AppState {
    type Target = application_model::ApplicationModel;

    /// Exposes widget-free application authority to existing controller paths.
    fn deref(&self) -> &Self::Target {
        &self.application_model
    }
}

impl std::ops::DerefMut for AppState {
    /// Exposes mutable application authority without extending GTK widget borrows.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.application_model
    }
}

fn main() {
    register_resources();
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

/// Registers the checked-in GResource bundle before any composite widget is built.
///
/// The registration contains presentation assets only and does not establish
/// document, history, evaluator, or persistence authority. Failure is fatal
/// because a GTK application without its required templates cannot present a
/// coherent window.
fn register_resources() {
    gio::resources_register_include!("toniator.gresource")
        .expect("failed to register compiled Toniator GResource");
}

fn parse_args(arguments: Vec<std::ffi::OsString>) -> Result<Option<PathBuf>, String> {
    match arguments.as_slice() {
        [] => Ok(None),
        [path] if !path.to_string_lossy().starts_with('-') => Ok(Some(PathBuf::from(path))),
        _ => Err("usage: toniator-app [PATH]".to_owned()),
    }
}

/// Constructs the GTK-only application window and connects its controls to the
/// authoritative headless workspace/history boundary. This function owns
/// widget lifetime and notification wiring, but never directly mutates a
/// document outside the existing command paths.
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
    let file_menu = gio::Menu::new();
    for (label, action, tooltip) in LIFECYCLE_BUTTONS {
        file_menu.append(Some(label.trim_start_matches('_')), Some(action));
        let _ = tooltip;
    }
    let file_button = gtk::MenuButton::builder()
        .label("File")
        .menu_model(&file_menu)
        .tooltip_text("Document and export actions")
        .build();
    header.pack_start(&file_button);
    for (label, action, tooltip) in [
        ("_Undo", "app.undo", "Undo the last change (Ctrl+Z)"),
        ("_Redo", "app.redo", "Redo the last change (Ctrl+Shift+Z)"),
    ] {
        let button = gtk::Button::with_mnemonic(label);
        button.set_action_name(Some(action));
        button.set_tooltip_text(Some(tooltip));
        header.pack_start(&button);
    }
    let selector = gtk::DropDown::from_strings(&PreviewModel::ALL.map(PreviewModel::label));
    selector.set_tooltip_text(Some("Choose the channel color model"));
    let channel_selector_model = gtk::StringList::new(&[]);
    let channel_selector = gtk::DropDown::new(
        Some(channel_selector_model.clone()),
        None::<gtk::Expression>,
    );
    channel_selector.set_sensitive(false);
    channel_selector.set_tooltip_text(Some("Choose the channel to adjust"));
    // Model and channel selectors remain in state for the ordinary inspector
    // workflow; the responsive header reserves narrow-space for Undo/Redo and
    // the clearly named settings drawer instead of cropping actionable controls.
    let window_title = adw::WindowTitle::new("Toniator", "Document lifecycle");
    header.set_title_widget(Some(&window_title));

    let shell = components::ToniatorMainShell::new();
    let banner = shell.banner();
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
    let viewer = gtk::Overlay::new();
    viewer.add_css_class("toniator-viewer");
    viewer.add_css_class(PreviewModel::Rgb.css_class());
    viewer.set_child(Some(&picture));
    let preview_spinner = gtk::Spinner::new();
    preview_spinner.set_visible(false);
    preview_spinner.set_halign(gtk::Align::Center);
    preview_spinner.set_valign(gtk::Align::Center);
    preview_spinner.set_tooltip_text(Some("Preview updating"));
    preview_spinner.update_property(&[gtk::accessible::Property::Label("Preview updating")]);
    viewer.add_overlay(&preview_spinner);
    stack.add_named(&viewer, Some(Page::Success.name()));
    stack.set_visible_child_name(Page::Empty.name());
    let channel_editor = components::ToniatorChannelEditor::new();
    channel_editor.add_css_class("toniator-inspector");
    let inspector = channel_editor.content();
    let inspector_status = channel_editor.status();
    inspector_status.set_label("Open a source-backed document to inspect channels.");
    let inspector_catalog = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let inspector_descriptors = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let selection_row = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let model_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let model_label = gtk::Label::new(Some("Color model"));
    model_label.set_xalign(0.0);
    model_label.set_hexpand(true);
    model_label.set_mnemonic_widget(Some(&selector));
    selector.update_property(&[gtk::accessible::Property::Label("Color model")]);
    model_row.append(&model_label);
    model_row.append(&selector);
    let channel_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let channel_label = gtk::Label::new(Some("Channel"));
    channel_label.set_xalign(0.0);
    channel_label.set_hexpand(true);
    channel_label.set_mnemonic_widget(Some(&channel_selector));
    channel_selector.update_property(&[gtk::accessible::Property::Label("Channel")]);
    channel_row.append(&channel_label);
    channel_row.append(&channel_selector);
    selection_row.append(&model_row);
    selection_row.append(&channel_row);
    inspector.append(&selection_row);
    inspector.append(&inspector_catalog);
    inspector.append(&inspector_descriptors);
    let inspector_scroll = gtk::ScrolledWindow::new();
    inspector_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    inspector_scroll.set_min_content_width(300);
    inspector_scroll.set_max_content_width(420);
    inspector_scroll.set_child(Some(&channel_editor));
    let split = shell.split();
    split.set_content(Some(&stack));
    split.set_sidebar(Some(&inspector_scroll));
    split.set_min_sidebar_width(320.0);
    split.set_max_sidebar_width(360.0);
    let drawer = gtk::ToggleButton::with_mnemonic("_Channel settings");
    drawer.set_tooltip_text(Some("Show or hide channel settings"));
    drawer.set_active(true);
    let split_for_drawer = split.clone();
    drawer.connect_toggled(move |button| split_for_drawer.set_show_sidebar(button.is_active()));
    header.pack_end(&drawer);
    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&shell));
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
    let (event_sender, event_receiver) = async_channel::unbounded();
    let state = Rc::new(RefCell::new(AppState {
        application_model: application_model::ApplicationModel::new(),
        syncing_model: false,
        window_close: WindowCloseController::default(),
        actions,
        window: window.clone(),
        window_title,
        stack,
        picture,
        viewer,
        preview_spinner,
        error,
        banner,
        selector,
        channel_selector,
        channel_selector_model,
        inspector_catalog,
        active_pattern: gtk::Label::new(None),
        inspector_descriptors,
        inspector_status,
        descriptor_components: BTreeMap::new(),
        inspector_runtime: InspectorRuntime::default(),
        syncing_inspector: false,
        syncing_draft_editor: false,
        inspector_rebuild_scheduled: false,
        pattern_editor: None,
        draft_epoch: 0,
        preview: None,
        preview_target: None,
        presets: PresetRegistry::bundled(),
        automation: AutomationSink::from_environment(),
        event_sender,
        preview_bridge_stop: Arc::new(AtomicBool::new(false)),
    }));
    let event_state = Rc::clone(&state);
    glib::MainContext::default().spawn_local(async move {
        while let Ok(event) = event_receiver.recv().await {
            handle_app_event(&event_state, event);
        }
    });
    start_preview_event_bridge(
        Arc::clone(&state.borrow().scheduler),
        state.borrow().event_sender.clone(),
        Arc::clone(&state.borrow().preview_bridge_stop),
    );
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
            if let Some(channel_id) = channel_id_at_selector_position(selector.selected(), &ids) {
                {
                    let mut app_state = state.borrow_mut();
                    let previous_channel = app_state.inspector_runtime.selected_channel;
                    app_state.inspector_runtime.selected_channel = Some(channel_id);
                    // A user-selected channel is a new inspector context.  Never
                    // carry a control identity from the previously selected
                    // channel into it.
                    app_state.inspector_runtime.focus = None;
                    if previous_channel != Some(channel_id) {
                        clear_structural_edit_context_for_selection(
                            &mut app_state.inspector_runtime,
                        );
                    }
                }
                dispatch_ui_intent(&state, UiIntent::SelectChannel(channel_id));
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
        action.connect_activate(move |_, _| {
            dispatch_ui_intent(&state, UiIntent::Lifecycle(lifecycle))
        });
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
        action.connect_activate(move |_, _| {
            dispatch_ui_intent(&state, if redo { UiIntent::Redo } else { UiIntent::Undo })
        });
    }
}

/// Dispatches a typed header intent through the authoritative history boundary.
fn dispatch_ui_intent(state: &Rc<RefCell<AppState>>, intent: UiIntent) {
    if let Some(redo) = history_redo(&intent) {
        apply_history_navigation(state, redo);
        return;
    }
    match intent {
        UiIntent::Lifecycle(action) => request_lifecycle(state, action),
        UiIntent::SelectChannel(channel) => {
            state.borrow_mut().selected_channel = Some(channel);
            schedule_inspector_rebuild(state);
        }
        UiIntent::ApplyPreset(id) => apply_selected_preset(state, &id, &id),
        UiIntent::OpenGridPatternEditor => open_pattern_editor(state, PatternEditorPurpose::Grid),
        UiIntent::OpenMarkEditor => open_pattern_editor(state, PatternEditorPurpose::Mark),
        UiIntent::DiscardPatternEditor => request_draft_discard(state),
        UiIntent::Undo | UiIntent::Redo => unreachable!("history intents returned above"),
    }
}

/// Bridges the unchanged scheduler receiver into typed main-context events.
///
/// The bridge never touches GTK, document history, or cache acceptance. It
/// stops when the app coordinator requests shutdown; token/ticket validation
/// remains in `handle_preview_completion` on the main context.
fn start_preview_event_bridge(
    scheduler: Arc<EvaluationScheduler>,
    sender: async_channel::Sender<AppEvent>,
    stop: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        while !stop.load(Ordering::Acquire) {
            match scheduler.try_receive_latest() {
                Ok(Some(completion)) => {
                    if sender.send_blocking(AppEvent::Preview(completion)).is_err() {
                        break;
                    }
                }
                Ok(None) => thread::park_timeout(Duration::from_millis(4)),
                Err(_) => break,
            }
        }
    });
}

/// Moves the authoritative main history cursor and schedules one fresh preview.
///
/// A missing/no-op cursor leaves the document and last successful preview
/// untouched. Accepted navigation mutates only `Workspace::history`; GTK work
/// and scheduler submission occur after the callback's mutable borrow ends.
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
            set_inspector_status(&mut app_state, "Rendering preview…");
            sync_ui(&mut app_state);
            drop(app_state);
            rebuild_inspector(state);
            schedule_main_preview_submission(state);
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
    if document.channel(channel_id).is_some() {
        return "Channel".to_owned();
    }
    document
        .modeled_channel(channel_id)
        .map(|channel| channel_role_label(channel.role).to_owned())
        .unwrap_or_else(|| "Unavailable channel".to_owned())
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

#[cfg(test)]
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
    }
}

/// Selects document and active-definition descriptor values for one channel.
///
/// Effective definition identity comes only from the domain resolver; this
/// presentation filter never reconstructs inheritance or persisted deltas.
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
            PropertyTarget::ChannelOutput(id, _) => id == channel_id,
            PropertyTarget::Definition(id)
            | PropertyTarget::Mechanism(id, _)
            | PropertyTarget::OutputLayer(id, _)
            | PropertyTarget::GuideDimension(id, _, _) => document
                .effective_channel_pattern(channel_id)
                .map(|pattern| pattern.definition_id == id)
                .unwrap_or(false),
        })
        .collect()
}

/// Classifies descriptor targets that edit a document-owned pattern definition.
///
/// Channel and document targets remain in the channel inspector; every other
/// supported target belongs to the transient Pattern Editor. This classification
/// is presentation-only and never changes descriptor, command, or history
/// authority.
fn is_structural_descriptor(descriptor: &PropertyDescriptor) -> bool {
    !matches!(
        descriptor.target,
        PropertyTarget::Document | PropertyTarget::Channel(_)
    )
}

/// Returns only selected-channel instance and presentation values for the
/// sidebar inspector. Structural values are deliberately excluded so the
/// sidebar cannot become an alternate Pattern Editor. Stage 20G also keeps
/// undisclosed document-base inheritance controls out of this existing
/// channel workflow; source selection and the document-owned aspect lock are
/// retained because they are the established controls at those locations.
fn channel_inspector_values(values: &[PropertyCurrentValue]) -> Vec<PropertyCurrentValue> {
    values
        .iter()
        .filter(|value| {
            !is_structural_descriptor(&value.descriptor)
                && match value.descriptor.target {
                    PropertyTarget::Document => matches!(
                        value.descriptor.field,
                        PropertyFieldId::SourceReference | PropertyFieldId::DensityAspectLocked
                    ),
                    _ => true,
                }
        })
        .cloned()
        .collect()
}

/// Returns only the selected purpose's descriptor values for one private editor.
///
/// The immutable reader remains the source of current values. This filter neither materializes a
/// definition copy nor exposes a Grid field in Mark Editor (or vice versa).
fn pattern_editor_values(
    values: &[PropertyCurrentValue],
    purpose: PatternEditorPurpose,
) -> Vec<PropertyCurrentValue> {
    values
        .iter()
        .filter(|value| descriptor_belongs_to_editor(&value.descriptor.field, purpose))
        .cloned()
        .collect()
}

/// Reports whether one structural descriptor belongs to the specified purpose-specific editor.
///
/// The filter is presentation policy only: descriptors and their typed commands remain owned by
/// the domain, and fields omitted here cannot be mutated through this GTK modal.
fn descriptor_belongs_to_editor(field: &PropertyFieldId, purpose: PatternEditorPurpose) -> bool {
    matches!(
        (purpose, field),
        (
            PatternEditorPurpose::Grid,
            PropertyFieldId::DensityAcrossX
                | PropertyFieldId::DensityAcrossY
                | PropertyFieldId::DensityAspectLocked
                | PropertyFieldId::RotationDegrees
                | PropertyFieldId::TranslationX
                | PropertyFieldId::TranslationY
                | PropertyFieldId::GuideBaselineAngle
                | PropertyFieldId::GuidePhase
                | PropertyFieldId::GuideSpacingMultiplier
                | PropertyFieldId::GuidePrototype
                | PropertyFieldId::GuideAuthoredStructure
                | PropertyFieldId::GuideArcCenterX
                | PropertyFieldId::GuideArcCenterY
                | PropertyFieldId::GuideArcRadius
                | PropertyFieldId::GuideArcStartAngle
                | PropertyFieldId::GuideArcSweepAngle
                | PropertyFieldId::GuideRepetition
                | PropertyFieldId::GuideOffsetSpacing
                | PropertyFieldId::GuideOffsetSides
                | PropertyFieldId::GuideOffsetCleanup
                | PropertyFieldId::GuideStackDirection
                | PropertyFieldId::GuideStackSpacingMultiplier
                | PropertyFieldId::IntersectionDimensions
                | PropertyFieldId::IntersectionMergeEpsilon
                | PropertyFieldId::AlongGuideDimensions
                | PropertyFieldId::AlongGuideIntervalMultiplier
                | PropertyFieldId::AlongGuidePhase
                | PropertyFieldId::CoverageGuardSteps
                | PropertyFieldId::CoverageAdditionalMargin
        ) | (
            PatternEditorPurpose::Mark,
            PropertyFieldId::MarkMinimumFill
                | PropertyFieldId::MarkMaximumFill
                | PropertyFieldId::ShapeRotationDegrees
                | PropertyFieldId::OutputSiteProduct
                | PropertyFieldId::OutputPrototype
                | PropertyFieldId::OutputAuthoredClosedShape
                | PropertyFieldId::OutputOrientation
                | PropertyFieldId::OutputOrientationDimension
        )
    )
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
        | PropertyFieldId::ArtworkWeightResponse => "Source",
        PropertyFieldId::Paint
        | PropertyFieldId::ColorRed
        | PropertyFieldId::ColorGreen
        | PropertyFieldId::ColorBlue
        | PropertyFieldId::ColorAlpha
        | PropertyFieldId::Opacity
        | PropertyFieldId::Visibility => "Appearance",
        PropertyFieldId::DensityAcrossX
        | PropertyFieldId::DensityAcrossY
        | PropertyFieldId::DensityAspectLocked
        | PropertyFieldId::RotationDegrees
        | PropertyFieldId::TranslationX
        | PropertyFieldId::TranslationY => "Transform",
        PropertyFieldId::MarkMinimumFill
        | PropertyFieldId::MarkMaximumFill
        | PropertyFieldId::ShapeRotationDegrees => "Marks",
        PropertyFieldId::ConnectedMinimumThickness | PropertyFieldId::ConnectedMaximumThickness => {
            "Paths"
        }
        PropertyFieldId::DefinitionSelection => "Pattern",
        PropertyFieldId::CoverageGuardSteps
        | PropertyFieldId::CoverageAdditionalMargin
        | PropertyFieldId::GuideBaselineAngle
        | PropertyFieldId::GuidePhase
        | PropertyFieldId::GuideSpacingMultiplier
        | PropertyFieldId::GuideOffsetSpacing
        | PropertyFieldId::IntersectionDimensions
        | PropertyFieldId::IntersectionMergeEpsilon
        | PropertyFieldId::AlongGuideDimensions
        | PropertyFieldId::AlongGuideIntervalMultiplier
        | PropertyFieldId::AlongGuidePhase => "Active family",
        _ => "Active mechanism and output",
    }
}

/// Returns the artist-facing inspector label for one typed domain field.
///
/// This presentation mapping does not infer applicability or mutate document
/// state; the domain descriptor remains authoritative for those boundaries.
fn inspector_field_label(field: PropertyFieldId) -> String {
    match field {
        PropertyFieldId::SourceReference => "Source artwork".into(),
        PropertyFieldId::DensityAcrossX => "Density across X".into(),
        PropertyFieldId::DensityAcrossY => "Density across Y".into(),
        PropertyFieldId::DensityAspectLocked => "Lock density aspect".into(),
        PropertyFieldId::RotationDegrees => "Rotation".into(),
        PropertyFieldId::TranslationX => "X offset".into(),
        PropertyFieldId::TranslationY => "Y offset".into(),
        PropertyFieldId::MarkMinimumFill => "Minimum fill".into(),
        PropertyFieldId::MarkMaximumFill => "Maximum fill".into(),
        // Connected values use the same domain-built inspector command boundary as mark fills.
        PropertyFieldId::ConnectedMinimumThickness => "Minimum thickness".into(),
        PropertyFieldId::ConnectedMaximumThickness => "Maximum thickness".into(),
        PropertyFieldId::ShapeRotationDegrees => "Shape rotation".into(),
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
        PropertyFieldId::DefinitionSelection => "Pattern".into(),
        PropertyFieldId::CoverageGuardSteps => "Coverage guard steps".into(),
        PropertyFieldId::CoverageAdditionalMargin => "Additional margin".into(),
        PropertyFieldId::GuideBaselineAngle => "Direction angle".into(),
        PropertyFieldId::GuidePhase => "Direction offset".into(),
        PropertyFieldId::GuideSpacingMultiplier => "Direction spacing".into(),
        PropertyFieldId::GuidePrototype => "Guide prototype".into(),
        PropertyFieldId::GuideAuthoredStructure => "Authored open path".into(),
        PropertyFieldId::GuideArcCenterX => "Arc center X".into(),
        PropertyFieldId::GuideArcCenterY => "Arc center Y".into(),
        PropertyFieldId::GuideArcRadius => "Arc radius".into(),
        PropertyFieldId::GuideArcStartAngle => "Arc start angle".into(),
        PropertyFieldId::GuideArcSweepAngle => "Arc sweep angle".into(),
        PropertyFieldId::GuideRepetition => "Guide repetition".into(),
        PropertyFieldId::GuideOffsetSpacing => "Offset gap".into(),
        PropertyFieldId::GuideOffsetSides => "Offset sides".into(),
        PropertyFieldId::GuideOffsetCleanup => "Offset cleanup".into(),
        PropertyFieldId::GuideStackDirection => "Stack direction".into(),
        PropertyFieldId::GuideStackSpacingMultiplier => "Stack spacing multiplier".into(),
        PropertyFieldId::IntersectionDimensions => "Directions at intersections".into(),
        PropertyFieldId::IntersectionMergeEpsilon => "Intersection merge tolerance".into(),
        PropertyFieldId::AlongGuideDimensions => "Directions along guides".into(),
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
        PropertyFieldId::OutputSiteProduct => "Mark placement".into(),
        PropertyFieldId::OutputPrototype => "Mark prototype".into(),
        PropertyFieldId::OutputAuthoredClosedShape => "Closed shape reference".into(),
        PropertyFieldId::OutputOrientation => "Mark orientation".into(),
        PropertyFieldId::OutputOrientationDimension => "Orientation guide dimension".into(),
        _ => "Advanced pattern setting".into(),
    }
}

/// Returns the static artist-facing label for one typed selector choice.
///
/// Choice validity and variant-transition payloads remain domain-owned; this
/// function supplies presentation text only and has no side effects.
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
        PropertyEnumChoice::RandomCharacter(RandomCharacterKind::RawUniform) => "Uniform",
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
        PropertyEnumChoice::MarkPrototype(
            toniator_domain::MarkPrototypeKind::AuthoredClosedShape,
        ) => "Authored closed shape",
        PropertyEnumChoice::MarkOrientation(MarkOrientationKind::Fixed) => "Fixed",
        PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideTangent) => "Guide tangent",
        PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideNormal) => "Guide normal",
        PropertyEnumChoice::GuidePrototype(
            toniator_domain::GuidePrototypeKind::AuthoredOpenPath,
        ) => "Authored open path",
        PropertyEnumChoice::GuidePrototype(toniator_domain::GuidePrototypeKind::CircularArc) => {
            "Circular arc"
        }
        PropertyEnumChoice::GuideRepetition(toniator_domain::GuideRepetitionKind::Single) => {
            "Single"
        }
        PropertyEnumChoice::GuideRepetition(
            toniator_domain::GuideRepetitionKind::TransformStack,
        ) => "Transform stack",
        PropertyEnumChoice::GuideRepetition(toniator_domain::GuideRepetitionKind::NormalOffset) => {
            "Normal offset"
        }
        PropertyEnumChoice::OffsetSides(toniator_domain::OffsetSides::Left) => "Left",
        PropertyEnumChoice::OffsetSides(toniator_domain::OffsetSides::Right) => "Right",
        PropertyEnumChoice::OffsetSides(toniator_domain::OffsetSides::Both) => "Both",
        PropertyEnumChoice::OffsetCleanup(toniator_domain::OffsetCleanup::DissolveCrossings) => {
            "Dissolve crossings"
        }
        _ => "Advanced pattern choice",
    }
}

/// Returns the artist-facing display text for one typed stable reference.
///
/// The label intentionally does not replace the stable ID used by commands;
/// reference resolution and compatibility remain authoritative in the domain.
fn reference_label(reference: &PropertyReferenceValue) -> String {
    match reference {
        PropertyReferenceValue::Source(SourceReference::Unassigned) => "No source assigned".into(),
        PropertyReferenceValue::Source(SourceReference::Assigned(_)) => {
            "Current source artwork".into()
        }
        PropertyReferenceValue::Definition(_) => "Current pattern".into(),
        PropertyReferenceValue::Mechanism(_) => "Current placement".into(),
        PropertyReferenceValue::GuideDimension(_) => "Direction".into(),
        PropertyReferenceValue::AuthoredStructure(_) => "Authored open path".into(),
    }
}

fn current_display(value: &PropertyCurrentValueKind) -> String {
    match value {
        PropertyCurrentValueKind::FiniteF64(value) => format!("{value:.4}"),
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

/// Projects the selected channel's active pattern in artist-facing vocabulary.
///
/// This reads immutable descriptor values only and never exposes a preset ID,
/// recipe, mechanism, or evaluator detail to GTK. The default guide pattern is
/// named explicitly so new and raw documents remain understandable.
fn artist_pattern_name(values: &[PropertyCurrentValue]) -> String {
    if values
        .iter()
        .any(|value| value.descriptor.field == PropertyFieldId::RandomCharacter)
    {
        "Random".to_owned()
    } else if values
        .iter()
        .any(|value| value.descriptor.field == PropertyFieldId::GuideBaselineAngle)
    {
        "Grid".to_owned()
    } else {
        "Simple XY Grid".to_owned()
    }
}

/// Identifies structural work-limit and tolerance controls that remain
/// progressively disclosed in the Pattern Editor.
///
/// All ordinary supported family/mechanism/modulation/output controls remain
/// visible on open in descriptor order. This presentation policy is
/// frontend-only and does not affect descriptor applicability or authority.
fn is_pattern_editor_advanced_safety_descriptor(field: PropertyFieldId) -> bool {
    matches!(
        field,
        PropertyFieldId::CoverageGuardSteps
            | PropertyFieldId::CoverageAdditionalMargin
            | PropertyFieldId::IntersectionMergeEpsilon
            | PropertyFieldId::RandomDensityModulation
            | PropertyFieldId::ArtworkWeightMappingComponent
            | PropertyFieldId::ArtworkWeightMappingPlacement
            | PropertyFieldId::ArtworkWeightMappingInverted
            | PropertyFieldId::ArtworkWeightMappingGain
            | PropertyFieldId::ArtworkWeightMappingBias
            | PropertyFieldId::ArtworkWeightStrength
            | PropertyFieldId::ArtworkWeightResponse
            | PropertyFieldId::RandomExclusion
            | PropertyFieldId::ExclusionMinimumCenterDistance
            | PropertyFieldId::VisibleMarkMargin
            | PropertyFieldId::VisibleMarkSizingPolicy
            | PropertyFieldId::RandomMaximumAttempts
            | PropertyFieldId::RandomMaximumNeighborChecks
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

/// Coalesces a channel-selector rebuild onto the GTK idle queue. The queue
/// boundary lets `selected-notify` unwind before the selector model or its
/// selected position changes, preventing GTK re-entrancy while retaining the
/// latest stable-ID runtime state.
fn schedule_inspector_rebuild(state: &Rc<RefCell<AppState>>) {
    let should_schedule = {
        let mut app_state = state.borrow_mut();
        if app_state.inspector_rebuild_scheduled {
            false
        } else {
            app_state.inspector_rebuild_scheduled = true;
            true
        }
    };
    if !should_schedule {
        return;
    }
    let state = Rc::clone(state);
    glib::idle_add_local_once(move || {
        state.borrow_mut().inspector_rebuild_scheduled = false;
        rebuild_inspector(&state);
    });
}

/// Rebuilds the inspector from authoritative document descriptors while
/// preserving runtime-only stable selection, focus, expansion, and draft
/// policy. Selector synchronization mutates only its persistent model while
/// programmatic notifications are guarded.
fn rebuild_inspector(state: &Rc<RefCell<AppState>>) {
    let (
        catalog,
        active_pattern,
        descriptors,
        status,
        selector,
        selector_model,
        channel_ids,
        labels,
        selected,
        values,
        active_pattern_text,
        status_message,
    ) = {
        let mut state = state.borrow_mut();
        let (labels, values, selected, active_pattern_text) = if let Some(document) = state
            .workspace
            .as_ref()
            .map(|workspace| workspace.document().clone())
        {
            let ids = authoritative_channel_ids(&document);
            let previous_selected = state.inspector_runtime.selected_channel;
            state.inspector_runtime.selected_channel =
                selected_channel_after_transition(previous_selected, &ids);
            state.selected_channel = state.inspector_runtime.selected_channel;
            let selected = state.inspector_runtime.selected_channel;
            if previous_selected.is_some() && previous_selected != selected {
                state.inspector_runtime.focus = None;
                clear_structural_edit_context_for_selection(&mut state.inspector_runtime);
            } else {
                state.inspector_runtime.focus = resolve_focus_after_document_change(
                    &document,
                    selected,
                    state.inspector_runtime.focus.clone(),
                );
            }
            if disarm_shared_edit_if_stale(&mut state.inspector_runtime, &document) {
                state.inspector_runtime.status = Some(
                    "Shared edit audience changed; disclose the current linked channels again."
                        .to_owned(),
                );
            }
            let all_values = selected
                .map(|channel_id| selected_property_values(&document, channel_id))
                .unwrap_or_default();
            (
                ids.iter()
                    .map(|id| channel_display_name(&document, *id))
                    .collect(),
                channel_inspector_values(&all_values),
                selected,
                selected
                    .map(|channel_id| {
                        artist_pattern_name(&selected_property_values(&document, channel_id))
                    })
                    .unwrap_or_else(|| "Simple XY Grid".to_owned()),
            )
        } else {
            (Vec::new(), Vec::new(), None, String::new())
        };
        state.syncing_inspector = true;
        let selector = state.channel_selector.clone();
        (
            state.inspector_catalog.clone(),
            state.active_pattern.clone(),
            state.inspector_descriptors.clone(),
            state.inspector_status.clone(),
            selector,
            state.channel_selector_model.clone(),
            state
                .workspace
                .as_ref()
                .map(|workspace| authoritative_channel_ids(workspace.document()))
                .unwrap_or_default(),
            labels,
            selected,
            values,
            active_pattern_text,
            state.inspector_runtime.status.clone(),
        )
    };
    selector_model.splice(
        0,
        selector_model.n_items(),
        &labels.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    selector.set_sensitive(!labels.is_empty());
    selector.set_selected(
        selected
            .and_then(|id| channel_ids.iter().position(|candidate| *candidate == id))
            .map(|index| index as u32)
            .unwrap_or(gtk::INVALID_LIST_POSITION),
    );
    state.borrow_mut().syncing_inspector = false;

    if selected.is_none() {
        status.set_label("No surviving channel is selected.");
        catalog.set_visible(false);
        reconcile_descriptor_components(state, &descriptors, Vec::new());
        rebuild_pattern_editor(state);
        return;
    }
    catalog.set_visible(true);
    active_pattern.set_label(&format!("Current pattern: {active_pattern_text}"));
    status.set_label(
        status_message
            .as_deref()
            .unwrap_or("Choose a pattern or adjust this channel."),
    );
    if catalog.first_child().is_none() {
        catalog.append(&active_pattern);
        append_preset_catalog(state, &catalog);
        append_pattern_editor_launch(state, &catalog);
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
    let components = values
        .into_iter()
        .map(|value| {
            let focus = descriptor_focuses[&inspector_key(&value.descriptor)].clone();
            (value, focus)
        })
        .collect();
    reconcile_descriptor_components(state, &descriptors, components);
    rebuild_pattern_editor(state);
    submit_draft_preview(state);
}

/// Reconciles persistent sidebar rows against immutable channel descriptor VMs.
///
/// Unchanged descriptor identities retain their GTK row and signal connections.
/// Simple scalar updates are applied while `syncing_inspector` rejects their
/// programmatic notifications; topology/reference changes replace only their
/// affected row and stale identities are removed. This function never mutates
/// the document, history, or preview authority.
fn reconcile_descriptor_components(
    state: &Rc<RefCell<AppState>>,
    container: &gtk::Box,
    values: Vec<(PropertyCurrentValue, InspectorFocusIdentity)>,
) {
    let active_keys = values
        .iter()
        .map(|(value, _)| inspector_key(&value.descriptor))
        .collect::<BTreeSet<_>>();
    let stale = state
        .borrow()
        .descriptor_components
        .keys()
        .filter(|key| !active_keys.contains(*key))
        .cloned()
        .collect::<Vec<_>>();
    {
        let mut app_state = state.borrow_mut();
        for key in stale {
            if let Some(component) = app_state.descriptor_components.remove(&key) {
                container.remove(&component.row);
            }
        }
    }

    let mut previous_group = None;
    for (value, focus) in values {
        let key = inspector_key(&value.descriptor);
        let group = inspector_group(value.descriptor.field);
        let replace = {
            let mut app_state = state.borrow_mut();
            app_state.syncing_inspector = true;
            let updated = app_state
                .descriptor_components
                .get_mut(&key)
                .map(|component| update_descriptor_component(component, &value))
                .unwrap_or(false);
            app_state.syncing_inspector = false;
            !updated
        };
        if replace {
            let old = state.borrow_mut().descriptor_components.remove(&key);
            if let Some(old) = old {
                container.remove(&old.row);
            }
            let component = append_descriptor_control(
                state,
                value,
                focus,
                (previous_group != Some(group)).then_some(group),
            );
            state
                .borrow_mut()
                .descriptor_components
                .insert(key.clone(), component);
        }
        let row = state
            .borrow()
            .descriptor_components
            .get(&key)
            .expect("active descriptor component is inserted")
            .row
            .clone();
        if row.parent().is_none() {
            container.append(&row);
        }
        previous_group = Some(group);
    }
}

/// Updates a retained scalar descriptor row from its new immutable view value.
///
/// Reference collections intentionally return `false`: their item topology can
/// change, so reconciliation replaces only that row instead of retaining stale
/// selection callbacks. The caller owns synchronization guarding and document
/// authority remains outside this presentation helper.
fn update_descriptor_component(
    component: &mut DescriptorComponent,
    value: &PropertyCurrentValue,
) -> bool {
    if component.value.descriptor != value.descriptor {
        return false;
    }
    match (&component.value.value, &value.value) {
        (PropertyCurrentValueKind::FiniteF64(_), PropertyCurrentValueKind::FiniteF64(next)) => {
            if let Some(entry) = component.control.downcast_ref::<gtk::Entry>() {
                if !entry.has_focus() {
                    entry.set_text(&format!("{next:.4}"));
                }
                component.value = value.clone();
                true
            } else {
                false
            }
        }
        (PropertyCurrentValueKind::U32(_), PropertyCurrentValueKind::U32(next)) => {
            if let Some(entry) = component.control.downcast_ref::<gtk::Entry>() {
                if !entry.has_focus() {
                    entry.set_text(&next.to_string());
                }
                component.value = value.clone();
                true
            } else {
                false
            }
        }
        (PropertyCurrentValueKind::Boolean(_), PropertyCurrentValueKind::Boolean(next)) => {
            if let Some(control) = component.control.downcast_ref::<gtk::Switch>() {
                control.set_active(*next);
                component.value = value.clone();
                true
            } else {
                false
            }
        }
        (PropertyCurrentValueKind::EnumChoice(_), PropertyCurrentValueKind::EnumChoice(next)) => {
            if let Some(control) = component.control.downcast_ref::<gtk::DropDown>() {
                let selected = value
                    .descriptor
                    .choices
                    .iter()
                    .position(|choice| choice == next)
                    .unwrap_or(0);
                control.set_selected(selected as u32);
                component.value = value.clone();
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Adds the stable bundled-pattern catalog without exposing preset IDs as UI authority.
///
/// Each row immediately asks the headless registry for one selected-channel,
/// copy-on-edit history transition. The registry remains the only source of
/// recipe semantics and the catalog never stores a recipe in a GTK widget.
fn append_preset_catalog(state: &Rc<RefCell<AppState>>, inspector: &gtk::Box) {
    let heading = gtk::Label::new(Some("Pattern"));
    heading.set_xalign(0.0);
    heading.add_css_class("heading");
    inspector.append(&heading);
    let entries = state
        .borrow()
        .presets
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.metadata.id.clone(),
                PatternCatalogViewModel {
                    name: entry.metadata.name.clone(),
                    description: entry.metadata.description.clone(),
                },
            )
        })
        .collect::<Vec<_>>();
    for (id, view_model) in entries {
        let row = components::ToniatorPresetRow::new();
        row.add_css_class("toniator-preset-row");
        row.set_preset_name(&view_model.name);
        row.set_preset_description(&view_model.description);
        let button = gtk::Button::with_label(&format!("Use {}", view_model.name));
        let state_for_apply = Rc::clone(state);
        let name = view_model.name.clone();
        button.connect_clicked(move |_| {
            let intent = UiIntent::ApplyPreset(id.clone());
            if let UiIntent::ApplyPreset(id) = intent {
                apply_selected_preset(&state_for_apply, &id, &name);
            }
        });
        row.append(&button);
        inspector.append(&row);
    }
    let shared = gtk::Button::with_label("Replace shared pattern…");
    shared.set_tooltip_text(Some(
        "Replace the linked channels after reviewing their names",
    ));
    let state_for_shared = Rc::clone(state);
    shared.connect_clicked(move |_| choose_shared_preset_choice(&state_for_shared));
    inspector.append(&shared);
}

/// Applies one bundled pattern to only the currently selected channel.
///
/// Registry failure leaves the document, history, selection, and last preview
/// untouched; accepted history transitions schedule exactly one fresh preview.
fn apply_selected_preset(state: &Rc<RefCell<AppState>>, id: &str, name: &str) {
    let result = {
        let mut app_state = state.borrow_mut();
        let selected = app_state.inspector_runtime.selected_channel;
        let registry = app_state.presets.clone();
        let Some(workspace) = app_state.workspace.as_mut() else {
            return;
        };
        let Some(channel) = selected else {
            return;
        };
        registry.apply_to_selected(&mut workspace.history, channel, id)
    };
    match result {
        Ok(_) => {
            let mut app_state = state.borrow_mut();
            set_preview_pending(&mut app_state);
            set_inspector_status(
                &mut app_state,
                format!("{name} applied to the selected channel."),
            );
            emit_automation_state(&mut app_state, "preset_applied", None);
            sync_ui(&mut app_state);
            drop(app_state);
            rebuild_inspector(state);
            schedule_main_preview_submission(state);
        }
        Err(error) => {
            let mut app_state = state.borrow_mut();
            set_inspector_status(&mut app_state, format!("Couldn’t apply {name}: {error}"));
        }
    }
}

/// Opens a deliberate shared-replacement disclosure that never combines with copy-on-edit.
/// Opens an explicit shared-pattern choice before any linked channel is prepared.
///
/// Choosing neither option leaves document/history/preview untouched. Each
/// choice then opens its own affected-channel disclosure and revalidates on
/// confirmation through the preset registry.
fn choose_shared_preset_choice(state: &Rc<RefCell<AppState>>) {
    let dialog = adw::Window::builder()
        .title("Choose a shared pattern")
        .transient_for(&state.borrow().window)
        .modal(true)
        .build();
    let content = components::ToniatorConfirmationContent::new();
    content.set_detail("Choose the pattern to use for every linked channel after confirmation.");
    let actions = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let random = gtk::Button::with_label("Even Random Circles");
    let grid = gtk::Button::with_label("Straight Grid Circles");
    let cancel = gtk::Button::with_label("Cancel");
    actions.append(&random);
    actions.append(&grid);
    actions.append(&cancel);
    content.append(&actions);
    dialog.set_content(Some(&content));
    let state_for_random = Rc::clone(state);
    let dialog_for_random = dialog.clone();
    random.connect_clicked(move |_| {
        dialog_for_random.close();
        choose_shared_preset_replacement(
            &state_for_random,
            "even-random-circles",
            "Even Random Circles",
        );
    });
    let state_for_grid = Rc::clone(state);
    let dialog_for_grid = dialog.clone();
    grid.connect_clicked(move |_| {
        dialog_for_grid.close();
        choose_shared_preset_replacement(
            &state_for_grid,
            "straight-grid-circles",
            "Straight Grid Circles",
        );
    });
    let dialog_for_cancel = dialog.clone();
    cancel.connect_clicked(move |_| dialog_for_cancel.close());
    dialog.present();
}

/// Discloses every affected channel before one explicitly chosen shared replacement.
fn choose_shared_preset_replacement(
    state: &Rc<RefCell<AppState>>,
    preset_id: &str,
    preset_name: &str,
) {
    let prepared = {
        let app_state = state.borrow();
        let Some(workspace) = app_state.workspace.as_ref() else {
            return;
        };
        let Some(selected) = app_state.inspector_runtime.selected_channel else {
            return;
        };
        let Some(definition) = workspace.document().pattern_definition_for(selected) else {
            return;
        };
        app_state
            .presets
            .prepare_shared_replacement(&workspace.history, definition.id, preset_id)
    };
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            set_inspector_status(
                &mut state.borrow_mut(),
                format!("Couldn’t prepare shared replacement: {error}"),
            );
            return;
        }
    };
    let names = {
        let app_state = state.borrow();
        let Some(document) = app_state.workspace.as_ref().map(Workspace::document) else {
            return;
        };
        prepared
            .affected_channels()
            .iter()
            .map(|channel| channel_display_name(document, *channel))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let dialog = adw::Window::builder()
        .title("Replace the shared pattern?")
        .transient_for(&state.borrow().window)
        .modal(true)
        .build();
    let content = components::ToniatorConfirmationContent::new();
    content.set_detail(&format!("Use {preset_name} for: {names}."));
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let replace = gtk::Button::with_label("Replace pattern");
    actions.append(&cancel);
    actions.append(&replace);
    content.append(&actions);
    dialog.set_content(Some(&content));
    let dialog_for_cancel = dialog.clone();
    cancel.connect_clicked(move |_| dialog_for_cancel.close());
    let state_for_confirm = Rc::clone(state);
    let dialog_for_confirm = dialog.clone();
    replace.connect_clicked(move |_| {
        let result = {
            let mut app_state = state_for_confirm.borrow_mut();
            let Some(workspace) = app_state.workspace.as_mut() else {
                return;
            };
            prepared.clone().confirm(&mut workspace.history)
        };
        match result {
            Ok(_) => {
                let mut app_state = state_for_confirm.borrow_mut();
                set_preview_pending(&mut app_state);
                set_inspector_status(&mut app_state, "Shared pattern replaced.");
                emit_automation_state(&mut app_state, "shared_pattern_replaced", None);
                sync_ui(&mut app_state);
                drop(app_state);
                rebuild_inspector(&state_for_confirm);
                schedule_main_preview_submission(&state_for_confirm);
            }
            Err(error) => set_inspector_status(
                &mut state_for_confirm.borrow_mut(),
                format!("Couldn’t replace the shared pattern: {error}"),
            ),
        }
        dialog_for_confirm.close();
    });
    dialog.present();
}

/// Adds separate selected-channel affordances for the purpose-specific private editors.
///
/// The sidebar retains channel-instance controls only. Neither action creates a definition,
/// chooses shared scope, or mutates document/history state before the artist confirms a draft edit.
fn append_pattern_editor_launch(state: &Rc<RefCell<AppState>>, inspector: &gtk::Box) {
    let grid = gtk::Button::with_mnemonic("Edit _guide paths…");
    let grid_description =
        "Edit authored guide-path resources used by the selected channel in a private draft.";
    grid.set_tooltip_text(Some(grid_description));
    grid.update_property(&[gtk::accessible::Property::Description(grid_description)]);
    let state_for_grid = Rc::clone(state);
    grid.connect_clicked(move |_| {
        dispatch_ui_intent(&state_for_grid, UiIntent::OpenGridPatternEditor)
    });
    let mark = gtk::Button::with_mnemonic("Edit _mark shapes…");
    let mark_description =
        "Edit authored mark-shape resources used by the selected channel in a private draft.";
    mark.set_tooltip_text(Some(mark_description));
    mark.update_property(&[gtk::accessible::Property::Description(mark_description)]);
    let state_for_mark = Rc::clone(state);
    mark.connect_clicked(move |_| dispatch_ui_intent(&state_for_mark, UiIntent::OpenMarkEditor));
    inspector.append(&grid);
    inspector.append(&mark);
}

/// Opens or raises one purpose-specific transient editor for the current stable channel.
///
/// The editor owns a cloned private document/history. It can inspect and edit its matching
/// resource purpose, but it has no route back to the main workspace, savepoint, or filesystem;
/// closing always discards the private session.
fn open_pattern_editor(state: &Rc<RefCell<AppState>>, purpose: PatternEditorPurpose) {
    let existing_editor = {
        let app_state = state.borrow();
        app_state.pattern_editor.as_ref().map(|surface| {
            (
                surface.window.clone(),
                surface.purpose,
                surface.status.clone(),
            )
        })
    };
    if let Some((window, existing_purpose, status)) = existing_editor {
        if existing_purpose != purpose {
            status.set_label(&format!(
                "Finish or cancel the open {} before opening {}.",
                existing_purpose.title(),
                purpose.title()
            ));
            window.present();
            return;
        }
        rebuild_pattern_editor(state);
        window.present();
        return;
    }
    let (parent, draft) = {
        let mut app_state = state.borrow_mut();
        let Some(selected_channel) = app_state.inspector_runtime.selected_channel else {
            return;
        };
        let (document, presentation, sources, history) = {
            let Some(workspace) = app_state.workspace.as_ref() else {
                return;
            };
            let Some(presentation) = workspace.source_presentation.clone() else {
                return;
            };
            (
                workspace.document().clone(),
                presentation,
                workspace.sources.clone(),
                DocumentHistory::new_draft(&workspace.history),
            )
        };
        app_state.draft_epoch = app_state.draft_epoch.saturating_add(1);
        let epoch = app_state.draft_epoch;
        (
            app_state.window.clone(),
            Rc::new(RefCell::new(PatternEditorDraft {
                history,
                selected_channel,
                initial_document: document,
                discard_confirmed: false,
                sources,
                presentation,
                scheduler: Arc::new(
                    EvaluationScheduler::new().expect("private draft scheduler starts"),
                ),
                preview_submission: None,
                epoch,
                geometry_editor: stage20f_editor::Stage20fEditorState::default(),
                construction_attachment: None,
                pending_shared_edit: None,
            })),
        )
    };
    let window = adw::Window::builder()
        .title(purpose.title())
        .default_width(980)
        .default_height(760)
        .transient_for(&parent)
        .modal(true)
        .build();
    let editor = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let shell = components::ToniatorPatternEditorShell::new();
    let status = shell.status();
    let picture = shell.picture();
    let preview_spinner = shell.spinner();
    preview_spinner.update_property(&[gtk::accessible::Property::Label("Preview updating")]);
    picture.set_can_shrink(true);
    picture.set_size_request(-1, 160);
    shell.set_editor(&editor);
    let introduction = gtk::Label::new(Some(
        "Changes stay in this private draft. Cancel discards them and leaves your document unchanged.",
    ));
    introduction.set_xalign(0.0);
    introduction.set_wrap(true);
    introduction.add_css_class("dim-label");
    let history = gtk::Label::new(None);
    history.set_xalign(0.0);
    history.add_css_class("dim-label");
    let current_pattern = gtk::Label::new(None);
    current_pattern.set_xalign(0.0);
    current_pattern.add_css_class("heading");
    let resource_list = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let construction_actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let new_structure = gtk::Button::with_label(purpose.construction_action_label());
    construction_actions.append(&new_structure);
    let construction_canvas = gtk::DrawingArea::new();
    construction_canvas.set_content_width(CONSTRUCTION_CANVAS_MIN_WIDTH_PX);
    construction_canvas.set_content_height(220);
    construction_canvas.set_hexpand(true);
    construction_canvas.set_tooltip_text(Some(CONSTRUCTION_CANVAS_GESTURE_HINT));
    construction_canvas.update_property(&[gtk::accessible::Property::Description(
        CONSTRUCTION_CANVAS_GESTURE_HINT,
    )]);
    construction_canvas.add_css_class("toniator-draft-preview");
    let coordinate_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let coordinate_x = gtk::Entry::new();
    coordinate_x.set_sensitive(false);
    let coordinate_y = gtk::Entry::new();
    coordinate_y.set_sensitive(false);
    coordinate_row.append(&gtk::Label::new(Some("X")));
    coordinate_row.append(&coordinate_x);
    coordinate_row.append(&gtk::Label::new(Some("Y")));
    coordinate_row.append(&coordinate_y);
    let segment_actions = gtk::Grid::new();
    segment_actions.set_column_spacing(6);
    segment_actions.set_row_spacing(6);
    let make_curve = gtk::Button::with_label("Make curve");
    make_curve.set_tooltip_text(Some("Convert the selected line segment to a cubic curve."));
    let make_line = gtk::Button::with_label("Make line");
    make_line.set_tooltip_text(Some(
        "Convert the selected cubic segment to a straight line.",
    ));
    let insert_node = gtk::Button::with_label("Insert node");
    insert_node.set_tooltip_text(Some(
        "Split the selected segment at its selected hit position.",
    ));
    let delete_node = gtk::Button::with_label("Delete node");
    delete_node.set_tooltip_text(Some(
        "Delete the selected node when the path retains two nodes.",
    ));
    for (index, action) in [&make_curve, &make_line, &insert_node, &delete_node]
        .into_iter()
        .enumerate()
    {
        action.set_sensitive(false);
        segment_actions.attach(action, (index % 2) as i32, (index / 2) as i32, 1, 1);
    }
    let state_for_canvas = Rc::clone(state);
    construction_canvas.set_draw_func(move |_, context, width, height| {
        context.set_source_rgb(0.16, 0.16, 0.18);
        context.rectangle(0.0, 0.0, f64::from(width), f64::from(height));
        let _ = context.fill();
        context.set_source_rgb(0.55, 0.68, 0.95);
        context.rectangle(0.5, 0.5, f64::from(width - 1), f64::from(height - 1));
        let _ = context.stroke();
        let app_state = state_for_canvas.borrow();
        let Some(surface) = app_state.pattern_editor.as_ref() else {
            return;
        };
        let draft = surface.draft.borrow();
        if let Some(points) = draft.geometry_editor.construction_points() {
            let point = |point: toniator_geometry::Point2| draft.geometry_editor.to_screen(point);
            if let Some(first) = points.first().copied() {
                let first = point(first);
                context.set_source_rgb(0.9, 0.78, 0.32);
                context.move_to(first.x, first.y);
                for next in points.iter().skip(1).copied() {
                    let next = point(next);
                    context.line_to(next.x, next.y);
                }
                context.set_line_width(1.5);
                let _ = context.stroke();
            }
            for construction_point in points {
                let construction_point = point(*construction_point);
                context.set_source_rgb(1.0, 0.85, 0.35);
                context.arc(
                    construction_point.x,
                    construction_point.y,
                    4.0,
                    0.0,
                    std::f64::consts::TAU,
                );
                let _ = context.fill();
            }
            return;
        }
        let Some(structure) = draft
            .geometry_editor
            .selected_structure
            .and_then(|id| draft.history.document().authored_structure(id))
        else {
            return;
        };
        let Ok(source_path) = toniator_geometry::CurvePath::from_authored_structure(structure)
        else {
            return;
        };
        let path = draft
            .geometry_editor
            .local_preview_path()
            .unwrap_or(&source_path);
        let point = |point: toniator_geometry::Point2| draft.geometry_editor.to_screen(point);
        for (index, segment) in path.segments().iter().enumerate() {
            let start = point(segment.start());
            context.move_to(start.x, start.y);
            match segment {
                toniator_geometry::CurveSegment::Line(line) => {
                    let end = point(line.end());
                    context.line_to(end.x, end.y);
                }
                toniator_geometry::CurveSegment::CubicBezier(cubic) => {
                    let c1 = point(cubic.control_1());
                    let c2 = point(cubic.control_2());
                    let end = point(cubic.end());
                    context.curve_to(c1.x, c1.y, c2.x, c2.y, end.x, end.y);
                    context.set_source_rgb(0.55, 0.55, 0.6);
                    context.move_to(start.x, start.y);
                    context.line_to(c1.x, c1.y);
                    context.move_to(end.x, end.y);
                    context.line_to(c2.x, c2.y);
                    let _ = context.stroke();
                }
            }
            if draft.geometry_editor.selected_segment == Some(index) {
                context.set_source_rgb(1.0, 0.65, 0.2);
                context.set_line_width(3.0);
            } else {
                context.set_source_rgb(0.8, 0.9, 1.0);
                context.set_line_width(1.5);
            }
            let _ = context.stroke();
        }
        for segment in path.segments() {
            let anchor = point(segment.start());
            context.set_source_rgb(1.0, 1.0, 1.0);
            context.arc(anchor.x, anchor.y, 4.0, 0.0, std::f64::consts::TAU);
            let _ = context.fill();
            if let toniator_geometry::CurveSegment::CubicBezier(cubic) = segment {
                for control in [cubic.control_1(), cubic.control_2()] {
                    let control = point(control);
                    context.set_source_rgb(0.65, 0.78, 1.0);
                    context.arc(control.x, control.y, 3.0, 0.0, std::f64::consts::TAU);
                    let _ = context.fill();
                }
            }
        }
    });
    let construction_gesture = gtk::GestureClick::new();
    construction_gesture.set_button(1);
    construction_gesture.set_exclusive(false);
    let state_for_construction = Rc::clone(state);
    let canvas_for_construction = construction_canvas.clone();
    construction_gesture.connect_pressed(move |_, _, x, y| {
        canvas_for_construction.grab_focus();
        let (created, selected) = {
            let app_state = state_for_construction.borrow();
            let Some(surface) = app_state.pattern_editor.as_ref() else {
                return;
            };
            let mut draft = surface.draft.borrow_mut();
            let screen = toniator_geometry::Point2::new(x, y);
            if draft.geometry_editor.incomplete() {
                (draft.geometry_editor.add_node_screen(screen, 8.0), false)
            } else {
                let existing = draft
                    .geometry_editor
                    .selected_structure
                    .and_then(|id| draft.history.document().authored_structure(id))
                    .and_then(|structure| {
                        toniator_geometry::CurvePath::from_authored_structure(structure).ok()
                    });
                if let Some(path) = existing {
                    draft.geometry_editor.select_at(&path, screen, 8.0);
                    if draft.geometry_editor.selected_node.is_some()
                        || draft.geometry_editor.selected_segment.is_some()
                    {
                        (None, true)
                    } else {
                        surface.status.set_label(&format!(
                            "Choose {} before adding construction nodes.",
                            surface.purpose.construction_action_label()
                        ));
                        (None, false)
                    }
                } else {
                    surface.status.set_label(&format!(
                        "Choose {} before adding construction nodes.",
                        surface.purpose.construction_action_label()
                    ));
                    (None, false)
                }
            }
        };
        if selected {
            rebuild_pattern_editor(&state_for_construction);
            return;
        }
        if created.is_none() {
            if let Some(surface) = state_for_construction.borrow().pattern_editor.as_ref() {
                surface.construction_canvas.queue_draw();
            }
            return;
        }
        if let Some(draft) = created
            && attach_completed_authored_structure(&state_for_construction, draft)
        {
            rebuild_pattern_editor(&state_for_construction);
            submit_draft_preview(&state_for_construction);
        }
    });
    construction_canvas.add_controller(construction_gesture);
    let drag_gesture = gtk::GestureDrag::new();
    drag_gesture.set_button(1);
    drag_gesture.set_exclusive(false);
    let state_for_drag = Rc::clone(state);
    drag_gesture.connect_drag_begin(move |gesture, x, y| {
        let claim = {
            let app_state = state_for_drag.borrow();
            if let Some(surface) = app_state.pattern_editor.as_ref() {
                let mut draft = surface.draft.borrow_mut();
                if draft.geometry_editor.incomplete() {
                    false
                } else if let Some(structure) = draft
                    .geometry_editor
                    .selected_structure
                    .and_then(|id| draft.history.document().authored_structure(id))
                    && let Ok(path) =
                        toniator_geometry::CurvePath::from_authored_structure(structure)
                {
                    draft.geometry_editor.select_at(
                        &path,
                        toniator_geometry::Point2::new(x, y),
                        8.0,
                    );
                    let target = draft.geometry_editor.selected_target;
                    if draft.geometry_editor.may_claim_target_drag(target) {
                        draft.geometry_editor.begin_target_drag_at(
                            path,
                            target.expect("claimed drag has a selected target"),
                            toniator_geometry::Point2::new(x, y),
                        );
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };
        if !claim {
            gesture.set_state(gtk::EventSequenceState::Denied);
        }
    });
    let state_for_drag_update = Rc::clone(state);
    drag_gesture.connect_drag_update(move |_, offset_x, offset_y| {
        let canvas = {
            let app = state_for_drag_update.borrow();
            let Some(surface) = app.pattern_editor.as_ref() else {
                return;
            };
            let changed = surface
                .draft
                .borrow_mut()
                .geometry_editor
                .update_drag_offset(offset_x, offset_y);
            changed.then(|| surface.construction_canvas.clone())
        };
        if let Some(canvas) = canvas {
            canvas.queue_draw();
        }
    });
    let state_for_drag_end = Rc::clone(state);
    drag_gesture.connect_drag_end(move |_, x, y| {
        let mutation = {
            let app_state = state_for_drag_end.borrow();
            let Some(surface) = app_state.pattern_editor.as_ref() else {
                return;
            };
            let mut draft = surface.draft.borrow_mut();
            let id = draft.geometry_editor.selected_structure;
            let Some(id) = id else {
                return;
            };
            let Some(payload) = draft.geometry_editor.end_drag_offset(x, y) else {
                return;
            };
            let Some(base_structure) = draft.history.document().authored_structure(id).cloned()
            else {
                return;
            };
            (base_structure, payload)
        };
        if request_path_replacement(&state_for_drag_end, mutation.0, mutation.1) {
            rebuild_pattern_editor(&state_for_drag_end);
            submit_draft_preview(&state_for_drag_end);
        }
    });
    construction_canvas.add_controller(drag_gesture);
    let zoom_scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    let state_for_zoom = Rc::clone(state);
    zoom_scroll.connect_scroll(move |_, _, dy| {
        let canvas = {
            let app = state_for_zoom.borrow();
            let Some(surface) = app.pattern_editor.as_ref() else {
                return glib::Propagation::Stop;
            };
            let mut draft = surface.draft.borrow_mut();
            let next =
                (draft.geometry_editor.zoom * if dy < 0.0 { 1.1 } else { 0.9 }).clamp(0.1, 16.0);
            let pan = draft.geometry_editor.pan;
            draft.geometry_editor.set_viewport(pan, next);
            surface.construction_canvas.clone()
        };
        canvas.queue_draw();
        glib::Propagation::Stop
    });
    construction_canvas.add_controller(zoom_scroll);
    let pan_gesture = gtk::GestureDrag::new();
    pan_gesture.set_button(2);
    let state_for_pan_begin = Rc::clone(state);
    pan_gesture.connect_drag_begin(move |_, x, y| {
        if let Some(surface) = state_for_pan_begin.borrow().pattern_editor.as_ref() {
            surface
                .draft
                .borrow_mut()
                .geometry_editor
                .begin_pan(toniator_geometry::Point2::new(x, y));
        }
    });
    let state_for_pan_update = Rc::clone(state);
    pan_gesture.connect_drag_update(move |_, offset_x, offset_y| {
        let canvas = {
            let app = state_for_pan_update.borrow();
            let Some(surface) = app.pattern_editor.as_ref() else {
                return;
            };
            let changed = surface
                .draft
                .borrow_mut()
                .geometry_editor
                .update_pan_offset(offset_x, offset_y);
            changed.then(|| surface.construction_canvas.clone())
        };
        if let Some(canvas) = canvas {
            canvas.queue_draw();
        }
    });
    let state_for_pan_end = Rc::clone(state);
    pan_gesture.connect_drag_end(move |_, _, _| {
        if let Some(surface) = state_for_pan_end.borrow().pattern_editor.as_ref() {
            surface.draft.borrow_mut().geometry_editor.end_pan();
        }
    });
    construction_canvas.add_controller(pan_gesture);
    construction_canvas.set_focusable(true);
    let keys = gtk::EventControllerKey::new();
    let state_for_keys = Rc::clone(state);
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if key == gtk::gdk::Key::Escape {
            if let Some(surface) = state_for_keys.borrow().pattern_editor.as_ref() {
                let mut draft = surface.draft.borrow_mut();
                draft.geometry_editor.cancel();
                draft.construction_attachment = None;
                surface.status.set_label("Construction cancelled.");
                surface.construction_canvas.queue_draw();
            }
            return glib::Propagation::Stop;
        }
        if key == gtk::gdk::Key::Return {
            let payload = {
                let app = state_for_keys.borrow();
                let Some(surface) = app.pattern_editor.as_ref() else {
                    return glib::Propagation::Stop;
                };
                let mut draft = surface.draft.borrow_mut();
                draft.geometry_editor.complete()
            };
            if let Some(payload) = payload
                && attach_completed_authored_structure(&state_for_keys, payload)
            {
                rebuild_pattern_editor(&state_for_keys);
                submit_draft_preview(&state_for_keys);
            }
            return glib::Propagation::Stop;
        }
        if key == gtk::gdk::Key::Insert {
            apply_edited_path(
                &state_for_keys,
                stage20f_editor::PathEdit::InsertNode { parameter: 0.5 },
            );
            return glib::Propagation::Stop;
        }
        let (dx, dy) = match key {
            gtk::gdk::Key::Left => (-1.0, 0.0),
            gtk::gdk::Key::Right => (1.0, 0.0),
            gtk::gdk::Key::Up => (0.0, -1.0),
            gtk::gdk::Key::Down => (0.0, 1.0),
            _ => return glib::Propagation::Proceed,
        };
        let step = stage20f_editor::Stage20fEditorState::nudge_step(
            modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK),
            modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK),
        );
        let mutation = {
            let app = state_for_keys.borrow();
            let Some(surface) = app.pattern_editor.as_ref() else {
                return glib::Propagation::Stop;
            };
            let draft = surface.draft.borrow_mut();
            let Some(structure) = draft
                .geometry_editor
                .selected_structure
                .and_then(|id| draft.history.document().authored_structure(id))
                .cloned()
            else {
                return glib::Propagation::Stop;
            };
            let Ok(path) = toniator_geometry::CurvePath::from_authored_structure(&structure) else {
                return glib::Propagation::Stop;
            };
            let Some(target) = draft.geometry_editor.selected_target else {
                return glib::Propagation::Stop;
            };
            let Some(payload) =
                draft
                    .geometry_editor
                    .nudge_selected(&path, target, dx * step, dy * step)
            else {
                return glib::Propagation::Stop;
            };
            (structure, payload)
        };
        if request_path_replacement(&state_for_keys, mutation.0, mutation.1) {
            rebuild_pattern_editor(&state_for_keys);
            submit_draft_preview(&state_for_keys);
        }
        glib::Propagation::Stop
    });
    construction_canvas.add_controller(keys);
    let state_for_new_structure = Rc::clone(state);
    new_structure.connect_clicked(move |_| {
        begin_authored_construction(&state_for_new_structure, purpose.structure_kind())
    });
    let primary = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let advanced = gtk::Expander::new(Some("Advanced"));
    advanced.set_tooltip_text(Some(
        "Optional modulation, exclusion, coverage, and safety controls for this private draft.",
    ));
    let advanced_rows = gtk::Box::new(gtk::Orientation::Vertical, 12);
    advanced.set_child(Some(&advanced_rows));
    let construction_sidebar = gtk::Box::new(gtk::Orientation::Vertical, 8);
    construction_sidebar.set_size_request(CONSTRUCTION_SIDEBAR_WIDTH_PX, -1);
    construction_sidebar.append(&resource_list);
    construction_sidebar.append(&construction_actions);
    let construction_surface = gtk::Box::new(gtk::Orientation::Vertical, 8);
    construction_surface.set_hexpand(true);
    let canvas_heading = gtk::Label::new(Some("Construction canvas"));
    canvas_heading.set_xalign(0.0);
    canvas_heading.add_css_class("heading");
    construction_surface.append(&canvas_heading);
    construction_surface.append(&construction_canvas);
    construction_surface.append(&coordinate_row);
    construction_surface.append(&segment_actions);
    // A regular box keeps both construction regions in the draft scroll area's natural flow.
    // `GtkPaned` could collapse both children after the native narrow breakpoint changed its
    // orientation, leaving the editor inaccessible at a modal width of 620px.
    let construction_layout = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    construction_layout.set_hexpand(true);
    construction_layout.append(&construction_sidebar);
    construction_layout.append(&construction_surface);
    let narrow_breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        NARROW_EDITOR_MAX_WIDTH_PX,
        adw::LengthUnit::Px,
    ));
    let layout_for_narrow = construction_layout.clone();
    let sidebar_for_narrow = construction_sidebar.clone();
    let canvas_for_narrow = construction_canvas.clone();
    narrow_breakpoint.connect_apply(move |_| {
        layout_for_narrow.set_orientation(gtk::Orientation::Vertical);
        sidebar_for_narrow.set_size_request(-1, -1);
        sidebar_for_narrow.set_hexpand(true);
        canvas_for_narrow.set_content_height(260);
    });
    let layout_for_wide = construction_layout.clone();
    let sidebar_for_wide = construction_sidebar.clone();
    let canvas_for_wide = construction_canvas.clone();
    narrow_breakpoint.connect_unapply(move |_| {
        layout_for_wide.set_orientation(gtk::Orientation::Horizontal);
        sidebar_for_wide.set_size_request(CONSTRUCTION_SIDEBAR_WIDTH_PX, -1);
        sidebar_for_wide.set_hexpand(false);
        canvas_for_wide.set_content_height(220);
    });
    window.add_breakpoint(narrow_breakpoint);
    editor.append(&introduction);
    editor.append(&history);
    editor.append(&current_pattern);
    editor.append(&construction_layout);
    editor.append(&primary);
    editor.append(&advanced);
    let undo = gtk::Button::with_mnemonic("_Undo");
    let state_for_undo = Rc::clone(state);
    undo.connect_clicked(move |_| apply_draft_history_navigation(&state_for_undo, false));
    let redo = gtk::Button::with_mnemonic("_Redo");
    let state_for_redo = Rc::clone(state);
    redo.connect_clicked(move |_| apply_draft_history_navigation(&state_for_redo, true));
    let preset = gtk::Button::with_label("Save as Preset…");
    preset.set_sensitive(false);
    preset.set_tooltip_text(Some("Preset authoring is planned for a later release."));
    let cancel = gtk::Button::with_mnemonic("_Cancel");
    let state_for_cancel = Rc::clone(state);
    cancel.connect_clicked(move |_| {
        dispatch_ui_intent(&state_for_cancel, UiIntent::DiscardPatternEditor)
    });
    let apply = gtk::Button::with_mnemonic("_Apply");
    apply.set_tooltip_text(Some(
        "Publish this valid private draft as one main undo step.",
    ));
    let state_for_apply = Rc::clone(state);
    apply.connect_clicked(move |_| apply_pattern_editor_draft(&state_for_apply));
    shell.append_action(&undo);
    shell.append_action(&redo);
    shell.append_action(&preset);
    shell.append_action(&cancel);
    shell.append_action(&apply);
    let header = adw::HeaderBar::new();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&shell));
    window.set_content(Some(&toolbar));
    let state_for_close = Rc::clone(state);
    window.connect_close_request(move |_| {
        if state_for_close.borrow().pattern_editor.is_none() {
            return glib::Propagation::Proceed;
        }
        let should_close = state_for_close
            .borrow()
            .pattern_editor
            .as_ref()
            .is_some_and(|surface| surface.draft.borrow().discard_confirmed);
        if should_close {
            state_for_close.borrow_mut().pattern_editor = None;
            glib::Propagation::Proceed
        } else {
            request_draft_discard(&state_for_close);
            glib::Propagation::Stop
        }
    });
    state.borrow_mut().pattern_editor = Some(PatternEditorSurface {
        purpose,
        window: window.clone(),
        status,
        picture,
        preview_spinner,
        draft,
        introduction,
        history,
        current_pattern,
        primary,
        advanced_rows,
        descriptor_components: BTreeMap::new(),
        apply,
        resource_list,
        construction_canvas,
        coordinate_x,
        coordinate_y,
        numeric_commit_active: Rc::new(Cell::new(false)),
        make_curve,
        make_line,
        insert_node,
        delete_node,
    });
    let state_for_x = Rc::clone(state);
    let x = state
        .borrow()
        .pattern_editor
        .as_ref()
        .unwrap()
        .coordinate_x
        .clone();
    x.connect_activate(move |_| commit_selected_numeric_once(&state_for_x));
    let focus_x = gtk::EventControllerFocus::new();
    let state_for_x_leave = Rc::clone(state);
    focus_x.connect_leave(move |_| commit_selected_numeric_once(&state_for_x_leave));
    x.add_controller(focus_x);
    let state_for_y = Rc::clone(state);
    let y = state
        .borrow()
        .pattern_editor
        .as_ref()
        .unwrap()
        .coordinate_y
        .clone();
    y.connect_activate(move |_| commit_selected_numeric_once(&state_for_y));
    let focus_y = gtk::EventControllerFocus::new();
    let state_for_y_leave = Rc::clone(state);
    focus_y.connect_leave(move |_| commit_selected_numeric_once(&state_for_y_leave));
    y.add_controller(focus_y);
    let make_curve = state
        .borrow()
        .pattern_editor
        .as_ref()
        .expect("live pattern editor owns its action controls")
        .make_curve
        .clone();
    let state_for_make_curve = Rc::clone(state);
    make_curve.connect_clicked(move |_| {
        apply_edited_path(&state_for_make_curve, stage20f_editor::PathEdit::MakeCurve);
    });
    let make_line = state
        .borrow()
        .pattern_editor
        .as_ref()
        .expect("live pattern editor owns its action controls")
        .make_line
        .clone();
    let state_for_make_line = Rc::clone(state);
    make_line.connect_clicked(move |_| {
        apply_edited_path(&state_for_make_line, stage20f_editor::PathEdit::MakeLine);
    });
    let insert_node = state
        .borrow()
        .pattern_editor
        .as_ref()
        .expect("live pattern editor owns its action controls")
        .insert_node
        .clone();
    let state_for_insert = Rc::clone(state);
    insert_node.connect_clicked(move |_| {
        let parameter = state_for_insert
            .borrow()
            .pattern_editor
            .as_ref()
            .and_then(|surface| {
                surface
                    .draft
                    .borrow()
                    .geometry_editor
                    .selected_segment_parameter
            })
            .unwrap_or(0.5)
            .clamp(0.05, 0.95);
        apply_edited_path(
            &state_for_insert,
            stage20f_editor::PathEdit::InsertNode { parameter },
        );
    });
    let delete_node = state
        .borrow()
        .pattern_editor
        .as_ref()
        .expect("live pattern editor owns its action controls")
        .delete_node
        .clone();
    let state_for_delete = Rc::clone(state);
    delete_node.connect_clicked(move |_| {
        let node = state_for_delete
            .borrow()
            .pattern_editor
            .as_ref()
            .and_then(|surface| surface.draft.borrow().geometry_editor.selected_node);
        if let Some(node) = node {
            apply_edited_path(
                &state_for_delete,
                stage20f_editor::PathEdit::DeleteNode { node },
            );
        }
    });
    emit_draft_automation_state(&mut state.borrow_mut(), "draft_opened", None);
    window.present();
    schedule_pattern_editor_initial_rebuild(state);
}

/// Defers initial private-editor synchronization until the launching GTK callback releases AppState.
///
/// Opening the modal creates its surface under `AppState` authority. GTK may synchronously emit
/// widget notifications while that callback unwinds, so this idle boundary reacquires the state
/// only after those borrows end. It rebuilds and schedules only the private draft preview; it never
/// publishes the draft or mutates the main workspace.
fn schedule_pattern_editor_initial_rebuild(state: &Rc<RefCell<AppState>>) {
    let state = Rc::clone(state);
    glib::idle_add_local_once(move || {
        if state.borrow().pattern_editor.is_none() {
            return;
        }
        rebuild_pattern_editor(&state);
        submit_draft_preview(&state);
    });
}

/// Commits one settled coordinate-entry change synchronously while rejecting reentrant duplicates.
///
/// GTK may emit activation and focus-leave for one Enter or AT-SPI commit. The surface-owned cell
/// protects that one callback cycle without delaying the typed history command past the current
/// input action, so a following Undo always targets the numeric replacement before older history.
fn commit_selected_numeric_once(state: &Rc<RefCell<AppState>>) {
    let active = {
        let app = state.borrow();
        let Some(surface) = app.pattern_editor.as_ref() else {
            return;
        };
        Rc::clone(&surface.numeric_commit_active)
    };
    if !accepts_numeric_commit_callback(&active) {
        return;
    }
    commit_selected_numeric(state);
    active.set(false);
}

/// Claims the one synchronous coordinate callback allowed until its caller releases the cell.
///
/// The cell is surface-local GTK presentation state, never document or history authority. A
/// `false` result means an activation/focus-leave callback is already committing the same entry.
fn accepts_numeric_commit_callback(active: &Cell<bool>) -> bool {
    !active.replace(true)
}

/// Commits the current selected anchor or cubic-control X/Y entries through one private typed replacement.
fn commit_selected_numeric(state: &Rc<RefCell<AppState>>) {
    let mutation = {
        let app = state.borrow();
        let Some(surface) = app.pattern_editor.as_ref() else {
            return;
        };
        let mut draft = surface.draft.borrow_mut();
        let Some(structure) = draft
            .geometry_editor
            .selected_structure
            .and_then(|id| draft.history.document().authored_structure(id))
            .cloned()
        else {
            return;
        };
        let Ok(path) = toniator_geometry::CurvePath::from_authored_structure(&structure) else {
            return;
        };
        let Some(target) = draft.geometry_editor.selected_target else {
            return;
        };
        let x = surface.coordinate_x.text();
        let y = surface.coordinate_y.text();
        if !matches!(
            (x.parse::<f64>(), y.parse::<f64>()),
            (Ok(x), Ok(y)) if x.is_finite() && y.is_finite()
        ) {
            draft.geometry_editor.set_local_invalid(true);
            surface
                .status
                .set_label("Enter finite numeric X and Y coordinates before applying.");
            surface.apply.set_sensitive(false);
            return;
        }
        draft.geometry_editor.set_local_invalid(false);
        let Some(payload) =
            draft
                .geometry_editor
                .commit_numeric(&path, target, x.as_str(), y.as_str())
        else {
            surface
                .apply
                .set_sensitive(draft.geometry_editor.apply_ready(draft.is_dirty()));
            return;
        };
        (structure, payload)
    };
    if request_path_replacement(state, mutation.0, mutation.1) {
        rebuild_pattern_editor(state);
        submit_draft_preview(state);
    }
}

/// Returns the channel that owns one deterministic authored-resource use projection.
fn authored_use_channel(use_value: &toniator_domain::AuthoredStructureUse) -> ChannelId {
    match use_value {
        toniator_domain::AuthoredStructureUse::Guide { channel_id, .. }
        | toniator_domain::AuthoredStructureUse::Mark { channel_id, .. } => *channel_id,
    }
}

/// Produces one stable artist-facing summary without exposing an authored resource's raw ID.
fn authored_use_summary(
    document: &Document,
    use_value: &toniator_domain::AuthoredStructureUse,
) -> String {
    let structure_id = use_value.structure_id();
    let Some(structure) = document.authored_structure(structure_id) else {
        return "Unavailable authored-resource use".to_owned();
    };
    let ordinal = document
        .authored_structures()
        .iter()
        .filter(|candidate| candidate.kind() == structure.kind())
        .position(|candidate| candidate.id() == structure_id)
        .map_or(0, |value| value + 1);
    let name = channel_display_name(document, authored_use_channel(use_value));
    match use_value {
        toniator_domain::AuthoredStructureUse::Guide { .. } => {
            format!("Guide path {ordinal} used by {name}")
        }
        toniator_domain::AuthoredStructureUse::Mark { .. } => {
            format!("Mark shape {ordinal} used by {name}")
        }
    }
}

/// Requests a private typed replacement, gating its first shared-resource mutation for an artist choice.
///
/// A single-use or already-armed resource applies immediately. A newly encountered shared resource
/// retains its exact base and replacement in the private draft and opens a choice dialog; it does
/// not create history, alter preview state, or silently select a sharing policy.
fn request_path_replacement(
    state: &Rc<RefCell<AppState>>,
    base_structure: toniator_domain::AuthoredStructure,
    replacement: toniator_domain::AuthoredStructureDraft,
) -> bool {
    let disclosure = {
        let app = state.borrow();
        let Some(surface) = app.pattern_editor.as_ref() else {
            return false;
        };
        let mut draft = surface.draft.borrow_mut();
        if draft.pending_shared_edit.is_some() {
            surface
                .status
                .set_label("Choose how to edit this shared resource before making another change.");
            return false;
        }
        let uses: Vec<_> = draft
            .history
            .document()
            .authored_structure_uses()
            .into_iter()
            .filter(|use_value| use_value.structure_id() == base_structure.id())
            .collect();
        if uses.len() > 1 && !draft.geometry_editor.is_shared_armed(base_structure.id()) {
            let active_use = uses
                .iter()
                .find(|use_value| authored_use_channel(use_value) == draft.selected_channel)
                .copied()
                .unwrap_or(uses[0]);
            let summaries: Vec<String> = uses
                .iter()
                .map(|use_value| authored_use_summary(draft.history.document(), use_value))
                .collect();
            let active_summary = authored_use_summary(draft.history.document(), &active_use);
            draft.pending_shared_edit = Some(PendingSharedPathEdit {
                selected_use: active_use,
                base_structure,
                replacement,
            });
            Some((active_summary, summaries))
        } else {
            match draft
                .history
                .apply(&DocumentCommand::ReplaceAuthoredStructure {
                    base_structure,
                    replacement,
                }) {
                Ok(_) => return true,
                Err(error) => {
                    surface
                        .status
                        .set_label(&format!("Path edit was not applied: {error}"));
                    return false;
                }
            }
        }
    };
    if let Some((active_summary, summaries)) = disclosure {
        show_shared_resource_choice(state, &active_summary, &summaries);
    }
    false
}

/// Presents the first-mutation shared-resource choice with every deterministic typed-use summary.
fn show_shared_resource_choice(
    state: &Rc<RefCell<AppState>>,
    active_summary: &str,
    summaries: &[String],
) {
    let parent = state
        .borrow()
        .pattern_editor
        .as_ref()
        .map(|surface| surface.window.clone())
        .expect("shared resource choice belongs to a live Pattern Editor");
    let dialog = adw::Window::builder()
        .title("Shared resource")
        .transient_for(&parent)
        .modal(true)
        .build();
    let content = components::ToniatorConfirmationContent::new();
    content.set_detail(&format!(
        "This authored resource has multiple uses:\n{}\n\nThis editor is changing: {active_summary}",
        summaries.join("\n")
    ));
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let edit_all = gtk::Button::with_label("Edit all uses");
    edit_all.set_tooltip_text(Some(
        "Keep the shared resource and change every listed use.",
    ));
    let copy = gtk::Button::with_label("Make a copy for this use");
    copy.set_tooltip_text(Some(
        "Retarget only the listed current use to a newly copied resource.",
    ));
    actions.append(&edit_all);
    actions.append(&copy);
    content.append(&actions);
    dialog.set_content(Some(&content));
    let edit_all_dialog = dialog.clone();
    let edit_all_state = Rc::clone(state);
    edit_all.connect_clicked(move |_| {
        if resolve_shared_path_choice(&edit_all_state, false) {
            edit_all_dialog.close();
        }
    });
    let copy_dialog = dialog.clone();
    let copy_state = Rc::clone(state);
    copy.connect_clicked(move |_| {
        if resolve_shared_path_choice(&copy_state, true) {
            copy_dialog.close();
        }
    });
    dialog.present();
}

/// Resolves one disclosed shared-resource policy atomically and keeps the pending replacement on failure.
fn resolve_shared_path_choice(state: &Rc<RefCell<AppState>>, make_copy: bool) -> bool {
    let changed = {
        let app = state.borrow();
        let Some(surface) = app.pattern_editor.as_ref() else {
            return false;
        };
        let mut draft = surface.draft.borrow_mut();
        let Some(pending) = draft.pending_shared_edit.take() else {
            return false;
        };
        let original_id = pending.base_structure.id();
        let result = if make_copy {
            draft
                .history
                .duplicate_retarget_and_replace_authored_structure(
                    pending.selected_use,
                    pending.replacement.clone(),
                )
        } else {
            draft.geometry_editor.arm_shared_resource(original_id);
            draft
                .history
                .apply(&DocumentCommand::ReplaceAuthoredStructure {
                    base_structure: pending.base_structure.clone(),
                    replacement: pending.replacement.clone(),
                })
        };
        match result {
            Ok(result) => {
                if make_copy {
                    let structure_id = result
                        .created_authored_structure_id
                        .expect("grouped copy reports its exact allocated structure ID");
                    draft.geometry_editor.selected_structure = Some(structure_id);
                }
                draft.geometry_editor.arm_shared_resource(original_id);
                true
            }
            Err(error) => {
                draft.pending_shared_edit = Some(pending);
                surface
                    .status
                    .set_label(&format!("Shared path edit was not applied: {error}"));
                false
            }
        }
    };
    if changed {
        rebuild_pattern_editor(state);
        submit_draft_preview(state);
    }
    changed
}

/// Applies one selected-path edit through the private typed history and refreshes preview only on change.
///
/// Selection and geometry remain local to the private Pattern Editor. This helper is the sole GTK
/// action route to `ReplaceAuthoredStructure`, so no-op segment conversions and failed deletions
/// never create history entries or resubmit a preview.
fn apply_edited_path(state: &Rc<RefCell<AppState>>, edit: stage20f_editor::PathEdit) {
    let mutation = {
        let app = state.borrow();
        let Some(surface) = app.pattern_editor.as_ref() else {
            return;
        };
        let draft = surface.draft.borrow_mut();
        let Some(structure) = draft
            .geometry_editor
            .selected_structure
            .and_then(|id| draft.history.document().authored_structure(id))
            .cloned()
        else {
            surface.status.set_label("Select a path before editing it.");
            return;
        };
        let Ok(path) = toniator_geometry::CurvePath::from_authored_structure(&structure) else {
            surface
                .status
                .set_label("The selected authored structure cannot be edited as a path.");
            return;
        };
        let payload = match draft.geometry_editor.edit_path(&path, edit) {
            Ok(Some(payload)) => payload,
            Ok(None) => {
                surface
                    .status
                    .set_label("That segment already has the requested kind.");
                return;
            }
            Err(stage20f_editor::PathEditError::NoSegmentSelection) => {
                surface
                    .status
                    .set_label("Select a segment before using that action.");
                return;
            }
            Err(stage20f_editor::PathEditError::Geometry(error)) => {
                surface.status.set_label(error.message());
                return;
            }
        };
        (structure, payload)
    };
    let changed = request_path_replacement(state, mutation.0, mutation.1);
    if changed {
        rebuild_pattern_editor(state);
        submit_draft_preview(state);
    }
}

/// Detaches and closes the private Pattern Editor only after discard is chosen.
///
/// The window leaves `AppState` under a short borrow before GTK destruction can
/// synchronously signal `close-request`, avoiding a nested `RefCell` borrow.
fn close_pattern_editor(state: &Rc<RefCell<AppState>>) {
    let window = {
        let mut state = state.borrow_mut();
        state.pattern_editor.take().map(|surface| {
            surface.draft.borrow_mut().preview_submission = None;
            surface.preview_spinner.stop();
            surface.preview_spinner.set_visible(false);
            surface.window
        })
    };
    if let Some(window) = window {
        window.close();
    }
}

/// Rebuilds the private Pattern Editor from its cloned document/history only.
///
/// GTK sees immutable draft value models and sends typed inputs back to that
/// private history. Main workspace state is intentionally not read or mutated
/// after the editor opens, which makes Cancel and titlebar close true discard.
fn rebuild_pattern_editor(state: &Rc<RefCell<AppState>>) {
    let Some((window, status, draft, purpose)) = (|| {
        let state = state.borrow();
        let surface = state.pattern_editor.as_ref()?;
        Some((
            surface.window.clone(),
            surface.status.clone(),
            Rc::clone(&surface.draft),
            surface.purpose,
        ))
    })() else {
        // Take the surface while borrowed, then close it after the RefCell
        // borrow ends because close-request synchronously re-enters AppState.
        let window = {
            let mut state = state.borrow_mut();
            state.pattern_editor.take().map(|surface| surface.window)
        };
        if let Some(window) = window {
            window.close();
        }
        return;
    };
    let (selected, name, values, dirty, can_undo, can_redo) = {
        let mut draft = draft.borrow_mut();
        let document = draft.history.document().clone();
        if draft
            .geometry_editor
            .selected_structure
            .and_then(|id| document.authored_structure(id))
            .is_some_and(|structure| structure.kind() != purpose.structure_kind())
        {
            draft.geometry_editor.clear_structure_selection();
        }
        let ids = authoritative_channel_ids(&document);
        let Some(selected) = selected_channel_after_transition(Some(draft.selected_channel), &ids)
        else {
            return;
        };
        draft.selected_channel = selected;
        (
            selected,
            channel_display_name(&document, selected),
            pattern_editor_values(&selected_property_values(&document, selected), purpose),
            draft.is_dirty(),
            draft.history.can_undo(),
            draft.history.can_redo(),
        )
    };
    window.set_title(Some(&format!("{} — {name}", purpose.title())));
    status.set_label(if dirty {
        "Unsaved changes. Previewing draft…"
    } else {
        "Draft preview updated."
    });
    {
        let app_state = state.borrow();
        let surface = app_state
            .pattern_editor
            .as_ref()
            .expect("live draft surface survives rebuild");
        surface.introduction.set_visible(true);
        surface.history.set_label(&format!(
            "Draft history: {}undo, {}redo",
            if can_undo { "" } else { "no " },
            if can_redo { "" } else { "no " }
        ));
        surface.current_pattern.set_label(&format!(
            "Current pattern: {}",
            draft_pattern_summary(&values, purpose)
        ));
        surface
            .apply
            .set_sensitive(draft.borrow().geometry_editor.apply_ready(dirty));
        let coordinate = {
            let draft = draft.borrow();
            let structure = draft
                .geometry_editor
                .selected_structure
                .and_then(|id| draft.history.document().authored_structure(id));
            structure
                .and_then(|structure| {
                    toniator_geometry::CurvePath::from_authored_structure(structure).ok()
                })
                .and_then(|path| {
                    draft.geometry_editor.selected_target.and_then(|target| {
                        draft
                            .geometry_editor
                            .coordinate(&path, target)
                            .map(|point| (target, point))
                    })
                })
        };
        let (can_make_curve, can_make_line, can_insert_node, can_delete_node) = {
            let draft = draft.borrow();
            let structure = draft
                .geometry_editor
                .selected_structure
                .and_then(|id| draft.history.document().authored_structure(id));
            let path = structure.and_then(|structure| {
                toniator_geometry::CurvePath::from_authored_structure(structure).ok()
            });
            let segment = draft
                .geometry_editor
                .selected_segment
                .and_then(|index| path.as_ref().and_then(|path| path.segments().get(index)));
            (
                matches!(segment, Some(toniator_geometry::CurveSegment::Line(_))),
                matches!(
                    segment,
                    Some(toniator_geometry::CurveSegment::CubicBezier(_))
                ),
                segment.is_some(),
                draft.geometry_editor.selected_node.is_some(),
            )
        };
        surface.make_curve.set_sensitive(can_make_curve);
        surface.make_line.set_sensitive(can_make_line);
        surface.insert_node.set_sensitive(can_insert_node);
        surface.delete_node.set_sensitive(can_delete_node);
        if let Some((target, point)) = coordinate {
            let target_name = match target {
                stage20f_editor::NumericTarget::Anchor(index) => format!("Anchor {}", index + 1),
                stage20f_editor::NumericTarget::Control1(index) => {
                    format!("Segment {} Control 1", index + 1)
                }
                stage20f_editor::NumericTarget::Control2(index) => {
                    format!("Segment {} Control 2", index + 1)
                }
            };
            surface
                .coordinate_x
                .update_property(&[gtk::accessible::Property::Label(&format!(
                    "{target_name} X coordinate"
                ))]);
            surface
                .coordinate_y
                .update_property(&[gtk::accessible::Property::Label(&format!(
                    "{target_name} Y coordinate"
                ))]);
            surface.coordinate_x.set_sensitive(true);
            surface.coordinate_y.set_sensitive(true);
            if !surface.coordinate_x.has_focus() {
                surface.coordinate_x.set_text(&point.x.to_string());
            }
            if !surface.coordinate_y.has_focus() {
                surface.coordinate_y.set_text(&point.y.to_string());
            }
        } else {
            surface
                .coordinate_x
                .update_property(&[gtk::accessible::Property::Label(
                    "No selected point X coordinate",
                )]);
            surface
                .coordinate_y
                .update_property(&[gtk::accessible::Property::Label(
                    "No selected point Y coordinate",
                )]);
            surface.coordinate_x.set_sensitive(false);
            surface.coordinate_y.set_sensitive(false);
            if !surface.coordinate_x.has_focus() {
                surface.coordinate_x.set_text("");
            }
            if !surface.coordinate_y.has_focus() {
                surface.coordinate_y.set_text("");
            }
        }
        rebuild_authored_resource_list(
            state,
            &surface.resource_list,
            draft.borrow().history.document(),
            draft.borrow().geometry_editor.selected_structure,
            purpose,
        );
        surface.construction_canvas.queue_draw();
        sync_draft_preview_pending(surface);
    }
    let (primary_values, advanced_values): (Vec<_>, Vec<_>) = values
        .into_iter()
        .partition(|value| !is_pattern_editor_advanced_safety_descriptor(value.descriptor.field));
    reconcile_draft_descriptor_components(state, selected, primary_values, advanced_values);
}

/// Projects one purpose's authored resources into a stable artist-facing group without raw IDs.
///
/// The domain remains authoritative for structure order and typed uses; this helper assigns only
/// presentation ordinals and concise summaries inside the private editor.
fn rebuild_authored_resource_list(
    state: &Rc<RefCell<AppState>>,
    list: &gtk::Box,
    document: &Document,
    selected_structure: Option<toniator_domain::AuthoredStructureId>,
    purpose: PatternEditorPurpose,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let uses = document.authored_structure_uses();
    let kind = purpose.structure_kind();
    let heading = gtk::Label::new(Some(match purpose {
        PatternEditorPurpose::Grid => "Guide paths",
        PatternEditorPurpose::Mark => "Mark shapes",
    }));
    heading.set_xalign(0.0);
    heading.add_css_class("heading");
    list.append(&heading);
    for (ordinal, structure) in document
        .authored_structures()
        .iter()
        .filter(|value| value.kind() == kind)
        .enumerate()
    {
        let count = uses
            .iter()
            .filter(|usage| usage.structure_id() == structure.id())
            .count();
        let label = match purpose {
            PatternEditorPurpose::Grid => format!(
                "Guide path {} — {} node segments; {} use{}",
                ordinal + 1,
                structure.segments().len(),
                count,
                if count == 1 { "" } else { "s" }
            ),
            PatternEditorPurpose::Mark => format!(
                "Mark shape {} — {} node segments; {} use{}",
                ordinal + 1,
                structure.segments().len(),
                count,
                if count == 1 { "" } else { "s" }
            ),
        };
        let row = gtk::Button::with_label(&label);
        row.set_halign(gtk::Align::Fill);
        row.set_hexpand(true);
        row.set_has_frame(true);
        let selected = selected_structure == Some(structure.id());
        if selected {
            row.add_css_class("suggested-action");
        }
        let state_for_row = Rc::clone(state);
        let structure_id = structure.id();
        row.connect_clicked(move |_| select_authored_resource(&state_for_row, structure_id));
        list.append(&row);
    }
}

/// Selects one explicit authored resource row without mutating its private history or preview.
fn select_authored_resource(
    state: &Rc<RefCell<AppState>>,
    structure_id: toniator_domain::AuthoredStructureId,
) {
    if let Some(surface) = state.borrow().pattern_editor.as_ref() {
        let mut draft = surface.draft.borrow_mut();
        draft.geometry_editor.select_structure(structure_id);
    }
    rebuild_pattern_editor(state);
}

/// Describes the preflight result for one explicit authored construction request.
///
/// This remains app-local presentation state. The typed attachment itself remains in the private
/// draft and does not mutate `DocumentHistory` until successful completion.
enum ConstructionPreflight {
    Ready(AuthoredStructureAttachment),
    ConfirmCustomAlongGuideLayout,
}

/// Begins local open-guide or closed-mark construction without mutating draft history.
fn begin_authored_construction(
    state: &Rc<RefCell<AppState>>,
    kind: toniator_domain::AuthoredStructureKind,
) {
    let preflight = {
        let app = state.borrow();
        let Some(surface) = app.pattern_editor.as_ref() else {
            return;
        };
        if surface.purpose.structure_kind() != kind {
            surface
                .status
                .set_label("This editor only creates its own resource kind.");
            return;
        }
        let mut draft = surface.draft.borrow_mut();
        match authored_attachment_target(draft.history.document(), draft.selected_channel, kind) {
            Ok(preflight) => preflight,
            Err(message) => {
                draft.construction_attachment = None;
                surface.status.set_label(&message);
                return;
            }
        }
    };
    match preflight {
        ConstructionPreflight::Ready(attachment) => {
            activate_authored_construction(state, kind, attachment)
        }
        ConstructionPreflight::ConfirmCustomAlongGuideLayout => {
            show_custom_along_guide_confirmation(state)
        }
    }
}

/// Stores one already-confirmed attachment intent and starts canvas-only construction.
///
/// This operation deliberately changes no private history or preview. Escape clears the local
/// intent, while successful completion alone consumes it through the atomic domain transition.
fn activate_authored_construction(
    state: &Rc<RefCell<AppState>>,
    kind: toniator_domain::AuthoredStructureKind,
    attachment: AuthoredStructureAttachment,
) {
    if let Some(surface) = state.borrow().pattern_editor.as_ref() {
        let mut draft = surface.draft.borrow_mut();
        draft.geometry_editor.begin(kind);
        draft.construction_attachment = Some(attachment);
        draft.geometry_editor.selected_node = None;
        surface
            .status
            .set_label("Click the canvas to add nodes; Enter completes and Escape cancels.");
        surface.construction_canvas.queue_draw();
    }
}

/// Presents the explicit selected-channel transition needed before an ordinary Grid can gain a guide path.
///
/// Confirming stores only a private construction intent. It does not convert, retarget, allocate,
/// or advance history until the artist finishes a valid open path.
fn show_custom_along_guide_confirmation(state: &Rc<RefCell<AppState>>) {
    let parent = state
        .borrow()
        .pattern_editor
        .as_ref()
        .map(|surface| surface.window.clone())
        .expect("guide confirmation belongs to a live Grid Pattern Editor");
    let dialog = adw::Window::builder()
        .title("Create a custom guide layout?")
        .transient_for(&parent)
        .modal(true)
        .build();
    let content = components::ToniatorConfirmationContent::new();
    content.set_detail(
        "This channel will switch to a private custom along-guide layout when the new guide is completed. Linked channels remain unchanged.",
    );
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_mnemonic("_Cancel");
    let confirm = gtk::Button::with_mnemonic("Create custom _guide");
    actions.append(&cancel);
    actions.append(&confirm);
    content.append(&actions);
    dialog.set_content(Some(&content));
    let cancel_dialog = dialog.clone();
    cancel.connect_clicked(move |_| cancel_dialog.close());
    let confirm_dialog = dialog.clone();
    let state_for_confirm = Rc::clone(state);
    confirm.connect_clicked(move |_| {
        activate_authored_construction(
            &state_for_confirm,
            toniator_domain::AuthoredStructureKind::OpenPath,
            AuthoredStructureAttachment::GuideCustomAlongLayout,
        );
        confirm_dialog.close();
    });
    dialog.present();
}

/// Resolves the one active selected-channel typed target or explicit confirmation required before drawing.
///
/// It never chooses among multiple guide dimensions or mark outputs. An ordinary straight Grid
/// returns the explicit custom-along-guide confirmation rather than silently changing topology.
fn authored_attachment_target(
    document: &Document,
    channel_id: ChannelId,
    kind: toniator_domain::AuthoredStructureKind,
) -> Result<ConstructionPreflight, String> {
    let Some(definition) = document.pattern_definition_for(channel_id) else {
        return Err("The selected channel has no active pattern target.".to_owned());
    };
    let targets = match kind {
        toniator_domain::AuthoredStructureKind::OpenPath => definition
            .mechanisms
            .iter()
            .filter_map(|mechanism| match mechanism {
                PatternMechanism::GuideDimensions { id, dimensions } => Some(
                    dimensions
                        .iter()
                        .map(|dimension| AuthoredStructureAttachment::Guide {
                            mechanism_id: *id,
                            dimension_id: dimension.id,
                        }),
                ),
                _ => None,
            })
            .flatten()
            .collect::<Vec<_>>(),
        toniator_domain::AuthoredStructureKind::ClosedShape => definition
            .output_layers
            .iter()
            .map(|layer| AuthoredStructureAttachment::Mark {
                output_layer_id: layer.id(),
            })
            .collect::<Vec<_>>(),
    };
    match targets.as_slice() {
        [target] => Ok(ConstructionPreflight::Ready(*target)),
        [] if kind == toniator_domain::AuthoredStructureKind::OpenPath
            && matches!(
                definition.mechanisms.as_slice(),
                [PatternMechanism::StraightGuides { .. }, PatternMechanism::GuideIntersections { .. }]
            )
            && definition.output_layers.len() == 1 => {
                Ok(ConstructionPreflight::ConfirmCustomAlongGuideLayout)
            }
        [] => Err("This selected pattern has no compatible active target for the new path.".to_owned()),
        _ => Err("This selected pattern has multiple compatible guide targets; choose one before creating a path.".to_owned()),
    }
}

/// Attaches a completed local construction through one atomic private-history resource transition.
///
/// The new resource is never published unattached. A missing or ambiguous descriptor keeps the
/// completed local draft out of history and reports the authority boundary in the editor status.
fn attach_completed_authored_structure(
    state: &Rc<RefCell<AppState>>,
    authored_draft: toniator_domain::AuthoredStructureDraft,
) -> bool {
    let result = {
        let app_state = state.borrow();
        let Some(surface) = app_state.pattern_editor.as_ref() else {
            return false;
        };
        let mut draft = surface.draft.borrow_mut();
        let selected_channel = draft.selected_channel;
        let Some(attachment) = draft.construction_attachment else {
            surface
                .status
                .set_label("Choose a valid construction target before completing this path.");
            return false;
        };
        draft.history.add_and_attach_authored_structure(
            selected_channel,
            attachment,
            authored_draft,
        )
    };
    match result {
        Ok(result) => {
            if let Some(surface) = state.borrow().pattern_editor.as_ref() {
                let mut draft = surface.draft.borrow_mut();
                draft.geometry_editor.selected_structure = result.created_authored_structure_id;
                draft.geometry_editor.cancel();
                draft.construction_attachment = None;
                surface
                    .status
                    .set_label("New path attached to the selected pattern.");
            }
            true
        }
        Err(error) => {
            if let Some(surface) = state.borrow().pattern_editor.as_ref() {
                surface
                    .status
                    .set_label(&format!("New path was not attached: {error}"));
            }
            false
        }
    }
}

/// Squashes the current private editor draft into the main history as one authoritative undo step.
///
/// A stale or invalid draft remains open and leaves both histories untouched. Successful publication
/// closes the private surface, refreshes the main UI, and schedules the existing main preview path.
fn apply_pattern_editor_draft(state: &Rc<RefCell<AppState>>) {
    let result = {
        let mut app_state = state.borrow_mut();
        let Some(draft) = app_state
            .pattern_editor
            .as_ref()
            .map(|surface| Rc::clone(&surface.draft))
        else {
            return;
        };
        let Some(workspace) = app_state.workspace.as_mut() else {
            return;
        };
        workspace.history.squash_draft(&draft.borrow().history)
    };
    match result {
        Ok(summary) if summary.unchanged => {
            if let Some(surface) = state.borrow().pattern_editor.as_ref() {
                surface.status.set_label("No draft changes to apply.");
            }
        }
        Ok(_) => {
            close_pattern_editor(state);
            let mut app_state = state.borrow_mut();
            set_preview_pending(&mut app_state);
            set_inspector_status(&mut app_state, "Pattern draft applied.");
            emit_automation_state(&mut app_state, "draft_applied", None);
            sync_ui(&mut app_state);
            drop(app_state);
            rebuild_inspector(state);
            schedule_main_preview_submission(state);
        }
        Err(error) => {
            if let Some(surface) = state.borrow().pattern_editor.as_ref() {
                surface
                    .status
                    .set_label(&format!("Couldn’t apply this draft: {error}"));
            }
        }
    }
}

/// Projects a concise purpose-specific readout without exposing recipe internals.
///
/// The values are immutable domain projections; the result is presentation text only.
fn draft_pattern_summary(
    values: &[PropertyCurrentValue],
    purpose: PatternEditorPurpose,
) -> &'static str {
    match purpose {
        PatternEditorPurpose::Grid if draft_grid_topology(values).is_some() => {
            "Custom guide layout"
        }
        PatternEditorPurpose::Grid => "Simple XY Grid",
        PatternEditorPurpose::Mark => "Mark appearance",
    }
}

/// Projects the active private grid's artist-visible direction count and site mode.
///
/// The projection reads immutable descriptor values only. `None` deliberately
/// hides grid-only controls for a random draft; recipe rebuilding remains the
/// typed history authority when the artist changes either value.
fn draft_grid_topology(values: &[PropertyCurrentValue]) -> Option<(usize, bool)> {
    let count = values
        .iter()
        .filter(|value| value.descriptor.field == PropertyFieldId::GuideBaselineAngle)
        .count();
    if !(1..=4).contains(&count) {
        return None;
    }
    let intersections = values
        .iter()
        .any(|value| value.descriptor.field == PropertyFieldId::IntersectionDimensions);
    Some((count, intersections))
}

/// Projects a unique artist-facing label for one private draft control.
///
/// Guide identity remains in the typed descriptor target, while its ordinal is
/// derived from the immutable selected-draft value order. GTK receives only
/// the resulting artist vocabulary and never an identifier or geometry rule.
fn draft_descriptor_label(
    state: &Rc<RefCell<AppState>>,
    selected: ChannelId,
    current: &PropertyCurrentValue,
) -> String {
    if !matches!(
        current.descriptor.target,
        PropertyTarget::GuideDimension(..)
    ) {
        return inspector_field_label(current.descriptor.field);
    }
    let ordinal = state
        .borrow()
        .pattern_editor
        .as_ref()
        .and_then(|surface| {
            pattern_editor_values(
                &selected_property_values(surface.draft.borrow().history.document(), selected),
                surface.purpose,
            )
            .iter()
            .filter(|value| {
                value.descriptor.field == current.descriptor.field
                    && matches!(value.descriptor.target, PropertyTarget::GuideDimension(..))
            })
            .position(|value| value.descriptor.target == current.descriptor.target)
            .map(|index| index + 1)
        })
        .unwrap_or(1);
    format!(
        "Direction {ordinal} {}",
        inspector_field_label(current.descriptor.field)
    )
}

/// Renders one private-draft structural control with no main-workspace route.
///
/// Supported scalar/choice controls dispatch the existing typed command builder
/// against the cloned document. Stable-reference fields stay read-only here
/// rather than exposing structural IDs as artist-facing product vocabulary.
fn append_draft_descriptor_control(
    state: &Rc<RefCell<AppState>>,
    selected: ChannelId,
    current: PropertyCurrentValue,
) -> DraftDescriptorComponent {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let artist_label = draft_descriptor_label(state, selected, &current);
    let label = gtk::Label::new(Some(&artist_label));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    let descriptor = current.descriptor.clone();
    let saved_current = current.clone();
    let control: gtk::Widget = match current.value {
        PropertyCurrentValueKind::FiniteF64(value) => {
            let control = gtk::Entry::new();
            control.set_text(&format!("{value:.4}"));
            control.set_input_purpose(gtk::InputPurpose::Number);
            control.set_tooltip_text(Some(&format!(
                "{artist_label}. Press Enter or leave this field to update the draft."
            )));
            let state_for_activate = Rc::clone(state);
            let descriptor_for_activate = descriptor.clone();
            control.connect_activate(move |control| {
                commit_draft_numeric_entry(
                    &state_for_activate,
                    selected,
                    descriptor_for_activate.clone(),
                    control,
                    false,
                );
            });
            let focus = gtk::EventControllerFocus::new();
            let state_for_leave = Rc::clone(state);
            let descriptor_for_leave = descriptor.clone();
            let control_for_leave = control.clone();
            focus.connect_leave(move |_| {
                commit_draft_numeric_entry(
                    &state_for_leave,
                    selected,
                    descriptor_for_leave.clone(),
                    &control_for_leave,
                    false,
                );
            });
            control.add_controller(focus);
            label.set_mnemonic_widget(Some(&control));
            row.append(&label);
            row.append(&control);
            control.upcast()
        }
        PropertyCurrentValueKind::U32(value) => {
            let control = gtk::Entry::new();
            control.set_text(&value.to_string());
            control.set_input_purpose(gtk::InputPurpose::Digits);
            control.set_tooltip_text(Some(&format!(
                "{artist_label}. Press Enter or leave this field to update the draft."
            )));
            let state_for_activate = Rc::clone(state);
            let descriptor_for_activate = descriptor.clone();
            control.connect_activate(move |control| {
                commit_draft_numeric_entry(
                    &state_for_activate,
                    selected,
                    descriptor_for_activate.clone(),
                    control,
                    true,
                );
            });
            let focus = gtk::EventControllerFocus::new();
            let state_for_leave = Rc::clone(state);
            let descriptor_for_leave = descriptor.clone();
            let control_for_leave = control.clone();
            focus.connect_leave(move |_| {
                commit_draft_numeric_entry(
                    &state_for_leave,
                    selected,
                    descriptor_for_leave.clone(),
                    &control_for_leave,
                    true,
                );
            });
            control.add_controller(focus);
            label.set_mnemonic_widget(Some(&control));
            row.append(&label);
            row.append(&control);
            control.upcast()
        }
        PropertyCurrentValueKind::Boolean(value) => {
            let control = gtk::Switch::new();
            control.set_active(value);
            let state = Rc::clone(state);
            control.connect_state_set(move |_, active| {
                if state.borrow().syncing_draft_editor {
                    return glib::Propagation::Proceed;
                }
                apply_draft_input(
                    &state,
                    selected,
                    descriptor.clone(),
                    InspectorInput::Boolean(active),
                );
                glib::Propagation::Proceed
            });
            label.set_mnemonic_widget(Some(&control));
            row.append(&label);
            row.append(&control);
            control.upcast()
        }
        PropertyCurrentValueKind::EnumChoice(active) => {
            let choices = descriptor.choices.to_vec();
            let labels = choices
                .iter()
                .map(|choice| enum_choice_label(*choice))
                .collect::<Vec<_>>();
            let control = gtk::DropDown::from_strings(&labels);
            control.set_selected(
                choices
                    .iter()
                    .position(|choice| *choice == active)
                    .unwrap_or(0) as u32,
            );
            let state = Rc::clone(state);
            control.connect_selected_notify(move |control| {
                if state.borrow().syncing_draft_editor {
                    return;
                }
                if let Some(choice) = choices.get(control.selected() as usize).copied() {
                    apply_draft_input(
                        &state,
                        selected,
                        descriptor.clone(),
                        InspectorInput::EnumChoice(choice),
                    );
                }
            });
            label.set_mnemonic_widget(Some(&control));
            row.append(&label);
            row.append(&control);
            control.upcast()
        }
        PropertyCurrentValueKind::Reference(reference) => {
            let choices = draft_reference_choices(state, &descriptor);
            let labels = choices.iter().map(reference_label).collect::<Vec<_>>();
            let control =
                gtk::DropDown::from_strings(&labels.iter().map(String::as_str).collect::<Vec<_>>());
            control.set_selected(
                choices
                    .iter()
                    .position(|candidate| candidate == &reference)
                    .unwrap_or(0) as u32,
            );
            control.set_sensitive(!choices.is_empty());
            let state_for_change = Rc::clone(state);
            control.connect_selected_notify(move |control| {
                if state_for_change.borrow().syncing_draft_editor {
                    return;
                }
                if let Some(choice) = choices.get(control.selected() as usize).cloned() {
                    apply_draft_input(
                        &state_for_change,
                        selected,
                        descriptor.clone(),
                        InspectorInput::Reference(choice),
                    );
                }
            });
            label.set_mnemonic_widget(Some(&control));
            row.append(&label);
            row.append(&control);
            control.upcast()
        }
        PropertyCurrentValueKind::ReferenceCollection(references) => {
            let choices = draft_reference_choices(state, &descriptor);
            let controls = gtk::Box::new(gtk::Orientation::Vertical, 4);
            for (index, reference) in references.iter().cloned().enumerate() {
                let direction = gtk::Label::new(Some(&format!("Direction {}", index + 1)));
                direction.set_xalign(0.0);
                let labels = choices.iter().map(reference_label).collect::<Vec<_>>();
                let select = gtk::DropDown::from_strings(
                    &labels.iter().map(String::as_str).collect::<Vec<_>>(),
                );
                select.set_selected(
                    choices
                        .iter()
                        .position(|candidate| candidate == &reference)
                        .unwrap_or(0) as u32,
                );
                let accessible_name = format!("Direction {} selector", index + 1);
                select.update_property(&[gtk::accessible::Property::Label(&accessible_name)]);
                direction.set_mnemonic_widget(Some(&select));
                let state_for_change = Rc::clone(state);
                let descriptor_for_change = descriptor.clone();
                let choices_for_change = choices.clone();
                let original = references.clone();
                select.connect_selected_notify(move |select| {
                    if state_for_change.borrow().syncing_draft_editor {
                        return;
                    }
                    if let Some(choice) =
                        choices_for_change.get(select.selected() as usize).cloned()
                    {
                        let mut rewritten = original.clone();
                        rewritten[index] = choice;
                        apply_draft_input(
                            &state_for_change,
                            selected,
                            descriptor_for_change.clone(),
                            InspectorInput::ReferenceCollection(rewritten),
                        );
                    }
                });
                controls.append(&direction);
                controls.append(&select);
            }
            row.append(&label);
            row.append(&controls);
            controls.upcast()
        }
    };
    DraftDescriptorComponent {
        row,
        control,
        value: saved_current,
    }
}

/// Reconciles private-editor descriptor rows without rebuilding its shell.
///
/// Immutable private-history values select the active keys. Unchanged scalar
/// controls keep their widget and callback identity; incompatible/topology
/// controls replace only their own row. This function never reads or changes
/// main workspace state, savepoints, files, or main preview coordination.
fn reconcile_draft_descriptor_components(
    state: &Rc<RefCell<AppState>>,
    selected: ChannelId,
    primary_values: Vec<PropertyCurrentValue>,
    advanced_values: Vec<PropertyCurrentValue>,
) {
    let entries = primary_values
        .into_iter()
        .map(|value| (false, value))
        .chain(advanced_values.into_iter().map(|value| (true, value)))
        .collect::<Vec<_>>();
    let active = entries
        .iter()
        .map(|(_, value)| inspector_key(&value.descriptor))
        .collect::<BTreeSet<_>>();
    let stale = state
        .borrow()
        .pattern_editor
        .as_ref()
        .map(|surface| {
            surface
                .descriptor_components
                .keys()
                .filter(|key| !active.contains(*key))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for key in stale {
        let row = state
            .borrow_mut()
            .pattern_editor
            .as_mut()
            .and_then(|surface| surface.descriptor_components.remove(&key))
            .map(|component| component.row);
        if let Some(row) = row
            && let Some(parent) = row.parent().and_downcast::<gtk::Box>()
        {
            parent.remove(&row);
        }
    }
    for (advanced, value) in entries {
        let key = inspector_key(&value.descriptor);
        let retained = {
            let mut app_state = state.borrow_mut();
            app_state.syncing_draft_editor = true;
            let retained = app_state
                .pattern_editor
                .as_mut()
                .and_then(|surface| surface.descriptor_components.get_mut(&key))
                .map(|component| update_draft_descriptor_component(component, &value))
                .unwrap_or(false);
            app_state.syncing_draft_editor = false;
            retained
        };
        if retained {
            continue;
        }
        let old = state
            .borrow_mut()
            .pattern_editor
            .as_mut()
            .and_then(|surface| surface.descriptor_components.remove(&key));
        if let Some(old) = old
            && let Some(parent) = old.row.parent().and_downcast::<gtk::Box>()
        {
            parent.remove(&old.row);
        }
        let component = append_draft_descriptor_control(state, selected, value);
        let row = component.row.clone();
        let parent = state.borrow().pattern_editor.as_ref().map(|surface| {
            if advanced {
                surface.advanced_rows.clone()
            } else {
                surface.primary.clone()
            }
        });
        if let Some(parent) = parent {
            parent.append(&row);
        }
        if let Some(surface) = state.borrow_mut().pattern_editor.as_mut() {
            surface.descriptor_components.insert(key, component);
        }
    }
}

/// Synchronizes a retained private scalar control from a new immutable value.
///
/// Entry text updates preserve an active artist edit. Other controls replace
/// only their own row when an update could emit an interaction signal, keeping
/// private-history command dispatch free of programmatic duplicate callbacks.
fn update_draft_descriptor_component(
    component: &mut DraftDescriptorComponent,
    value: &PropertyCurrentValue,
) -> bool {
    if component.value.descriptor != value.descriptor {
        return false;
    }
    match (&component.value.value, &value.value) {
        (PropertyCurrentValueKind::FiniteF64(_), PropertyCurrentValueKind::FiniteF64(next)) => {
            if let Some(entry) = component.control.downcast_ref::<gtk::Entry>() {
                if !entry.has_focus() {
                    entry.set_text(&format!("{next:.4}"));
                }
                component.value = value.clone();
                true
            } else {
                false
            }
        }
        (PropertyCurrentValueKind::U32(_), PropertyCurrentValueKind::U32(next)) => {
            if let Some(entry) = component.control.downcast_ref::<gtk::Entry>() {
                if !entry.has_focus() {
                    entry.set_text(&next.to_string());
                }
                component.value = value.clone();
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Projects supported private-draft references without reading main workspace state.
///
/// The private history remains the sole source for its choices. Returned values
/// are typed references that the existing command boundary validates, never
/// display IDs or GTK-owned structural data.
fn draft_reference_choices(
    state: &Rc<RefCell<AppState>>,
    descriptor: &PropertyDescriptor,
) -> Vec<PropertyReferenceValue> {
    let state = state.borrow();
    let Some(surface) = state.pattern_editor.as_ref() else {
        return Vec::new();
    };
    let document = surface.draft.borrow();
    let document = document.history.document();
    match descriptor.field {
        PropertyFieldId::OutputSiteProduct => definition_for_target(document, descriptor.target)
            .map(|definition| {
                definition
                    .mechanisms
                    .iter()
                    .map(|mechanism| PropertyReferenceValue::Mechanism(mechanism.id()))
                    .collect()
            })
            .unwrap_or_default(),
        PropertyFieldId::OutputOrientationDimension
        | PropertyFieldId::IntersectionDimensions
        | PropertyFieldId::AlongGuideDimensions => {
            guide_dimensions_for_target(document, descriptor.target)
                .into_iter()
                .map(PropertyReferenceValue::GuideDimension)
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Commits one completed private numeric edit without reacting to each keystroke.
///
/// The entry keeps artist input local until Enter or focus leave. Invalid text
/// does not touch draft history or its preview and is reported on the private
/// editor surface; finite decimal and whole-number boundaries remain enforced
/// before the existing typed command path receives the value.
fn commit_draft_numeric_entry(
    state: &Rc<RefCell<AppState>>,
    selected: ChannelId,
    descriptor: PropertyDescriptor,
    control: &gtk::Entry,
    whole_number: bool,
) {
    let text = control.text();
    let input = if whole_number {
        text.trim()
            .parse::<u32>()
            .map(InspectorInput::U32)
            .map_err(|_| "Enter a whole number for this setting.".to_owned())
    } else {
        text.trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(InspectorInput::FiniteF64)
            .ok_or_else(|| "Enter a finite number for this setting.".to_owned())
    };
    match input {
        Ok(input) => {
            if draft_numeric_input_matches_current(state, &descriptor, &input) {
                return;
            }
            apply_draft_input(state, selected, descriptor, input)
        }
        Err(message) => {
            if let Some(surface) = state.borrow().pattern_editor.as_ref() {
                surface
                    .status
                    .set_label(&format!("Fix the highlighted setting… {message}"));
            }
        }
    }
}

/// Reports whether a completed private numeric edit already equals draft authority.
///
/// Enter and focus leave both call the commit helper; this guard makes the
/// second callback a no-op without changing private history or submitting an
/// extra preview.
fn draft_numeric_input_matches_current(
    state: &Rc<RefCell<AppState>>,
    descriptor: &PropertyDescriptor,
    input: &InspectorInput,
) -> bool {
    let state = state.borrow();
    let Some(surface) = state.pattern_editor.as_ref() else {
        return false;
    };
    let draft = surface.draft.borrow();
    selected_property_values(draft.history.document(), draft.selected_channel)
        .into_iter()
        .find(|value| value.descriptor == *descriptor)
        .is_some_and(|value| match (&value.value, input) {
            (PropertyCurrentValueKind::FiniteF64(current), InspectorInput::FiniteF64(next)) => {
                current == next
            }
            (PropertyCurrentValueKind::U32(current), InspectorInput::U32(next)) => current == next,
            _ => false,
        })
}

/// Applies one typed structural input to the private draft history only.
///
/// Failure preserves the draft's last successful state and leaves every main
/// document/lifecycle/preview boundary untouched.
fn apply_draft_input(
    state: &Rc<RefCell<AppState>>,
    _selected: ChannelId,
    descriptor: PropertyDescriptor,
    input: InspectorInput,
) {
    let result = {
        let app_state = state.borrow();
        let Some(surface) = app_state.pattern_editor.as_ref() else {
            return;
        };
        let mut draft = surface.draft.borrow_mut();
        let selected = draft.selected_channel;
        let command =
            private_draft_command_for_input(draft.history.document(), selected, &descriptor, input);
        command.and_then(|command| {
            draft
                .history
                .apply(&command)
                .map_err(|error| error.to_string())
        })
    };
    if let Err(error) = result {
        if let Some(surface) = state.borrow().pattern_editor.as_ref() {
            surface
                .status
                .set_label(&format!("Fix the highlighted setting to continue. {error}"));
        }
        return;
    }
    rebuild_pattern_editor(state);
    submit_draft_preview(state);
}

/// Converts one private Pattern Editor input into a selected-copy command.
///
/// Compound enum selectors are finalized through the domain's transient
/// transition draft so their payload defaults and stable references remain
/// domain-owned. This helper is private-editor-only: callers must pass the
/// cloned draft document and it never exposes a main-workspace route.
///
/// # Errors
///
/// Returns an artist-actionable error when the descriptor is stale, the choice
/// is unsupported, a transition payload cannot be completed, or the selected
/// draft channel has no current pattern definition.
fn private_draft_command_for_input(
    document: &Document,
    selected_channel: ChannelId,
    descriptor: &PropertyDescriptor,
    input: InspectorInput,
) -> Result<DocumentCommand, String> {
    match input {
        InspectorInput::EnumChoice(choice)
            if matches!(
                descriptor.field,
                PropertyFieldId::RandomCharacter
                    | PropertyFieldId::RandomDensityModulation
                    | PropertyFieldId::RandomExclusion
                    | PropertyFieldId::OutputOrientation
            ) =>
        {
            private_compound_transition_command(document, selected_channel, descriptor, choice)
        }
        input => command_for_inspector_input(
            document,
            Some(selected_channel),
            DefinitionEditScope::SelectedCopy,
            descriptor,
            input,
        ),
    }
}

/// Finalizes one private compound selector transition as exactly one history command.
///
/// The document creates the transition's scalar payload defaults. A guided
/// orientation is the one payload that cannot be inferred by the document;
/// this editor chooses the first domain-advertised compatible direction so the
/// visible orientation chooser remains immediately operable, after which its
/// stable Direction selector remains available for deliberate adjustment.
/// No main document, main history, lifecycle state, or main preview identity
/// is read or mutated by this private-draft command construction.
///
/// # Errors
///
/// Returns an error if the domain rejects the transition or payload, the
/// selector has no compatible direction, or the selected channel has no base
/// definition for stale-command protection.
fn private_compound_transition_command(
    document: &Document,
    selected_channel: ChannelId,
    descriptor: &PropertyDescriptor,
    choice: PropertyEnumChoice,
) -> Result<DocumentCommand, String> {
    let transition = document
        .variant_transition_draft(descriptor, choice)
        .map_err(|error| error.to_string())?;
    let required_references = transition
        .fields()
        .iter()
        .filter_map(|field| match &field.value {
            VariantTransitionValue::StableReference(None) => {
                field.reference_choices.first().cloned().map(|reference| {
                    VariantTransitionFieldUpdate {
                        field: field.field,
                        target: field.target,
                        value: VariantTransitionValue::StableReference(Some(reference)),
                    }
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let transition = transition
        .with_updates(&required_references)
        .map_err(|error| error.to_string())?;
    let edit = transition
        .finalize(document)
        .map_err(|error| error.to_string())?;
    let base_definition = document
        .pattern_definition_for(selected_channel)
        .cloned()
        .ok_or_else(|| "The selected draft pattern is no longer available.".to_owned())?;
    Ok(DocumentCommand::EditSelectedChannelPatternDefinition {
        channel_id: selected_channel,
        base_definition,
        edit,
    })
}

/// Moves only the private draft history cursor and then redraws draft controls.
fn apply_draft_history_navigation(state: &Rc<RefCell<AppState>>, redo: bool) {
    let result = {
        let app_state = state.borrow();
        let Some(surface) = app_state.pattern_editor.as_ref() else {
            return;
        };
        let mut draft = surface.draft.borrow_mut();
        if redo {
            draft.history.redo()
        } else {
            draft.history.undo()
        }
    };
    if let Err(error) = result {
        if let Some(surface) = state.borrow().pattern_editor.as_ref() {
            surface
                .status
                .set_label(&format!("Couldn’t update this draft: {error}"));
        }
        return;
    }
    rebuild_pattern_editor(state);
    submit_draft_preview(state);
}

/// Submits the private document clone through its own scheduler and source bundle.
///
/// This request never shares a ticket, cache acceptance, texture, or document
/// token with the main workspace preview coordinator.
fn submit_draft_preview(state: &Rc<RefCell<AppState>>) {
    let (scheduler, request, sender, epoch, ticket_value) = {
        let app_state = state.borrow();
        let Some(surface) = app_state.pattern_editor.as_ref() else {
            return;
        };
        let mut draft = surface.draft.borrow_mut();
        let Some(source) = draft.sources.get(&draft.presentation.id) else {
            return;
        };
        let resolved = match ResolvedSource::new(
            draft.presentation.id.clone(),
            Arc::<[u8]>::from(source.bytes()),
            draft.presentation.format,
        ) {
            Ok(source) => source,
            Err(error) => {
                surface.status.set_label(&format!(
                    "Couldn’t render this pattern. Your last preview is still shown. {error}"
                ));
                return;
            }
        };
        let target = toniator_engine::PreviewRasterTarget::new(512, 512)
            .expect("fixed preview target is valid");
        let request = EvaluationRequest::with_preview_target(
            draft.history.session().document_evaluation_snapshot(),
            resolved,
            target,
        );
        let scheduler = Arc::clone(&draft.scheduler);
        let ticket = match scheduler.submit(request.clone()) {
            Ok(ticket) => ticket,
            Err(error) => {
                surface.status.set_label(&format!(
                    "Couldn’t render this pattern. Your last preview is still shown. {error}"
                ));
                return;
            }
        };
        draft.preview_submission = Some(ticket.value());
        surface.status.set_label("Previewing draft…");
        surface.preview_spinner.set_visible(true);
        surface.preview_spinner.start();
        (
            scheduler,
            request,
            app_state.event_sender.clone(),
            draft.epoch,
            ticket.value(),
        )
    };
    emit_draft_automation_state(
        &mut state.borrow_mut(),
        "draft_preview_submitted",
        Some(ticket_value),
    );
    let _ = request;
    thread::spawn(move || {
        loop {
            match scheduler.try_receive_latest() {
                Ok(Some(completion)) => {
                    let _ = sender.send_blocking(AppEvent::DraftPreview { epoch, completion });
                    break;
                }
                Ok(None) => thread::park_timeout(Duration::from_millis(4)),
                Err(_) => break,
            }
        }
    });
}

/// Requests a private-draft discard, confirming only when local edits exist.
fn request_draft_discard(state: &Rc<RefCell<AppState>>) {
    let (dirty, parent) = {
        let app_state = state.borrow();
        let Some(surface) = app_state.pattern_editor.as_ref() else {
            return;
        };
        (surface.draft.borrow().is_dirty(), surface.window.clone())
    };
    if !dirty {
        close_pattern_editor(state);
        return;
    }
    let dialog = adw::Window::builder()
        .title("Discard pattern changes?")
        .transient_for(&parent)
        .modal(true)
        .build();
    let content = components::ToniatorConfirmationContent::new();
    content
        .set_detail("Discarding closes this private draft. Your current document stays unchanged.");
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let keep_editing = gtk::Button::with_label("Keep editing");
    let discard = gtk::Button::with_label("Discard changes");
    discard.add_css_class("destructive-action");
    actions.append(&keep_editing);
    actions.append(&discard);
    content.append(&actions);
    dialog.set_content(Some(&content));
    let dialog_for_keep = dialog.clone();
    keep_editing.connect_clicked(move |_| dialog_for_keep.close());
    let state_for_discard = Rc::clone(state);
    let dialog_for_discard = dialog.clone();
    discard.connect_clicked(move |_| {
        if let Some(surface) = state_for_discard.borrow().pattern_editor.as_ref() {
            surface.draft.borrow_mut().discard_confirmed = true;
        }
        close_pattern_editor(&state_for_discard);
        dialog_for_discard.close();
    });
    dialog.present();
}

/// Clears the frontend-only scalar drafts after a channel context changes.
///
/// Draft text never enters document/history authority; the next immutable view
/// model repopulates matching controls from the selected channel only.
fn clear_structural_edit_context_for_selection(runtime: &mut InspectorRuntime) {
    runtime.drafts.clear();
    runtime.scope = DefinitionEditScope::SelectedCopy;
}

/// Leaves the removed shared-definition editor path disarmed in production.
fn disarm_shared_edit_if_stale(_runtime: &mut InspectorRuntime, _document: &Document) -> bool {
    false
}

/// Retains the normal selected-copy command boundary after removing old shared UI.
fn reject_stale_shared_edit_before_command(_state: &Rc<RefCell<AppState>>) -> bool {
    false
}

/// Parses and commits one ordinary numeric descriptor entry through the shared
/// typed command route. Enter and focus-leave both invoke this helper, preserving identical finite/whole-number parsing, draft retention,
/// focus, invalidation, and history behavior. Invalid text never mutates the
/// document; domain rejection remains status-only until a valid command applies.
fn commit_numeric_descriptor_entry(
    state: &Rc<RefCell<AppState>>,
    descriptor: PropertyDescriptor,
    focus: InspectorFocusIdentity,
    control: &gtk::Entry,
) {
    let text = control.text().to_string();
    match descriptor.value_kind {
        PropertyValueKind::FiniteF64 => match text.parse::<f64>() {
            Ok(value) if value.is_finite() => {
                if main_numeric_input_matches_current(
                    state,
                    &descriptor,
                    &InspectorInput::FiniteF64(value),
                ) {
                    return;
                }
                remember_inspector_draft(state, &descriptor, text);
                commit_inspector_input_with_focus(
                    state,
                    descriptor,
                    InspectorInput::FiniteF64(value),
                    focus,
                );
            }
            _ => record_inspector_draft(state, &descriptor, text, "Enter a finite number."),
        },
        PropertyValueKind::U32 => match text.parse::<u32>() {
            Ok(value) => {
                if main_numeric_input_matches_current(
                    state,
                    &descriptor,
                    &InspectorInput::U32(value),
                ) {
                    return;
                }
                remember_inspector_draft(state, &descriptor, text);
                commit_inspector_input_with_focus(
                    state,
                    descriptor,
                    InspectorInput::U32(value),
                    focus,
                );
            }
            Err(_) => record_inspector_draft(state, &descriptor, text, "Enter a whole number."),
        },
        _ => unreachable!("numeric descriptor helper receives only numeric descriptors"),
    }
}

/// Reports whether an ordinary numeric edit already equals main document authority.
///
/// This suppresses the focus-leave duplicate immediately following an Enter
/// commit, so one completed gesture creates at most one history transition.
fn main_numeric_input_matches_current(
    state: &Rc<RefCell<AppState>>,
    descriptor: &PropertyDescriptor,
    input: &InspectorInput,
) -> bool {
    let state = state.borrow();
    let Some(workspace) = state.workspace.as_ref() else {
        return false;
    };
    let Some(selected) = state.inspector_runtime.selected_channel else {
        return false;
    };
    selected_property_values(workspace.document(), selected)
        .into_iter()
        .find(|value| value.descriptor == *descriptor)
        .is_some_and(|value| match (&value.value, input) {
            (PropertyCurrentValueKind::FiniteF64(current), InspectorInput::FiniteF64(next)) => {
                current == next
            }
            (PropertyCurrentValueKind::U32(current), InspectorInput::U32(next)) => current == next,
            _ => false,
        })
}

/// Renders one descriptor/value control for the channel inspector or Pattern
/// Editor while preserving the immutable descriptor/value authority boundary.
///
/// Numeric entries use the shared completed-edit commit helper; immediate widgets
/// route existing typed commands. Invalid drafts and domain errors remain
/// frontend-only, while successful commands flow through history and scheduler
/// handling. No widget value is independently persisted or evaluated.
fn append_descriptor_control(
    state: &Rc<RefCell<AppState>>,
    current: PropertyCurrentValue,
    focus: InspectorFocusIdentity,
    group_heading: Option<&str>,
) -> DescriptorComponent {
    debug_assert!(control_route(&current).is_some());
    let component = gtk::Box::new(gtk::Orientation::Vertical, 4);
    if let Some(group) = group_heading {
        let heading = gtk::Label::new(Some(group));
        heading.set_xalign(0.0);
        heading.add_css_class("heading");
        heading.set_margin_top(12);
        component.append(&heading);
    }
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
    let control: gtk::Widget = match (&descriptor.value_kind, &current.value) {
        (PropertyValueKind::Boolean, PropertyCurrentValueKind::Boolean(active)) => {
            let control = gtk::Switch::new();
            control.set_active(*active);
            control.set_tooltip_text(Some("Turn this setting on or off"));
            label.set_mnemonic_widget(Some(&control));
            let state_for_callback = Rc::clone(state);
            let descriptor_for_callback = descriptor.clone();
            let focus_for_callback = focus.clone();
            control.connect_state_set(move |_, active| {
                if state_for_callback.borrow().syncing_inspector {
                    return glib::Propagation::Proceed;
                }
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
            control.upcast()
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
                if state_for_callback.borrow().syncing_inspector {
                    return;
                }
                if let Some(choice) = choices.get(control.selected() as usize).copied() {
                    commit_inspector_input_with_focus(
                        &state_for_callback,
                        descriptor_for_callback.clone(),
                        InspectorInput::EnumChoice(choice),
                        focus_for_callback.clone(),
                    );
                }
            });
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
            control.upcast()
        }
        (PropertyValueKind::FiniteF64, PropertyCurrentValueKind::FiniteF64(value)) => {
            let control = gtk::Entry::new();
            control.set_text(&draft_text(
                state,
                &inspector_key(&descriptor),
                &format!("{value:.4}"),
            ));
            control.set_input_purpose(gtk::InputPurpose::Number);
            control.set_width_chars(12);
            control.set_tooltip_text(Some(&field_detail(descriptor.bounds, descriptor.unit)));
            label.set_mnemonic_widget(Some(&control));
            let state_for_callback = Rc::clone(state);
            let descriptor_for_callback = descriptor.clone();
            let focus_for_callback = focus.clone();
            control.connect_activate(move |control| {
                commit_numeric_descriptor_entry(
                    &state_for_callback,
                    descriptor_for_callback.clone(),
                    focus_for_callback.clone(),
                    control,
                );
            });
            let focus_controller = gtk::EventControllerFocus::new();
            let state_for_leave = Rc::clone(state);
            let descriptor_for_leave = descriptor.clone();
            let focus_for_leave = focus.clone();
            let control_for_leave = control.clone();
            focus_controller.connect_leave(move |_| {
                commit_numeric_descriptor_entry(
                    &state_for_leave,
                    descriptor_for_leave.clone(),
                    focus_for_leave.clone(),
                    &control_for_leave,
                );
            });
            control.add_controller(focus_controller);
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
            control.upcast()
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
                commit_numeric_descriptor_entry(
                    &state_for_callback,
                    descriptor_for_callback.clone(),
                    focus_for_callback.clone(),
                    control,
                );
            });
            let focus_controller = gtk::EventControllerFocus::new();
            let state_for_leave = Rc::clone(state);
            let descriptor_for_leave = descriptor.clone();
            let focus_for_leave = focus.clone();
            let control_for_leave = control.clone();
            focus_controller.connect_leave(move |_| {
                commit_numeric_descriptor_entry(
                    &state_for_leave,
                    descriptor_for_leave.clone(),
                    focus_for_leave.clone(),
                    &control_for_leave,
                );
            });
            control.add_controller(focus_controller);
            row.append(&labels);
            row.append(&control);
            schedule_inspector_focus(state, focus, &control);
            control.upcast()
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
                if state_for_callback.borrow().syncing_inspector {
                    return;
                }
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
            control.upcast()
        }
        (_, PropertyCurrentValueKind::ReferenceCollection(references)) => {
            let control = gtk::Box::new(gtk::Orientation::Vertical, 4);
            let choices = reference_collection_choices(state, &descriptor);
            for (index, reference) in references.iter().cloned().enumerate() {
                let direction = gtk::Label::new(Some(&format!("Direction {}", index + 1)));
                direction.set_xalign(0.0);
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
                let accessible_name = format!("Direction {} selector", index + 1);
                select.update_property(&[gtk::accessible::Property::Label(&accessible_name)]);
                direction.set_mnemonic_widget(Some(&select));
                let state_for_callback = Rc::clone(state);
                let descriptor = descriptor.clone();
                let choices = choices.clone();
                let original = references.clone();
                let focus = focus_with_collection_index(&focus, index);
                let focus_for_callback = focus.clone();
                select.connect_selected_notify(move |select| {
                    if state_for_callback.borrow().syncing_inspector {
                        return;
                    }
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
                control.append(&direction);
                control.append(&select);
                schedule_inspector_focus(state, focus, &select);
            }
            control.set_tooltip_text(Some("Choose the numbered directions used by this pattern."));
            row.append(&labels);
            row.append(&control);
            control.upcast()
        }
        _ => {
            let control = gtk::Label::new(Some(&current_display(&current.value)));
            control.set_xalign(1.0);
            row.append(&labels);
            row.append(&control);
            control.upcast()
        }
    };
    component.append(&row);
    DescriptorComponent {
        row: component,
        control,
        value: current,
    }
}

/// Maps a supported semantic unit to concise artist-facing helper text.
///
/// Raw descriptor bounds and unit tokens never reach product vocabulary.
/// Empty text intentionally means that the setting needs no extra helper.
fn unit_message(unit: toniator_domain::PropertyUnit) -> &'static str {
    match unit {
        toniator_domain::PropertyUnit::None => "",
        toniator_domain::PropertyUnit::Density => "Density",
        toniator_domain::PropertyUnit::Degrees => "Degrees",
        toniator_domain::PropertyUnit::Phase => "Offset",
        toniator_domain::PropertyUnit::DocumentDistance => "Canvas distance",
        toniator_domain::PropertyUnit::NormalizedComponent => "Channel amount",
        toniator_domain::PropertyUnit::Count => "Count",
    }
}

/// Produces non-authoritative helper text without exposing domain bounds.
///
/// The ignored bounds remain domain validation authority; UI text never
/// advertises raw comparator strings that would read as backend vocabulary.
fn field_detail(
    _bounds: Option<toniator_domain::PropertyBounds>,
    unit: toniator_domain::PropertyUnit,
) -> String {
    unit_message(unit).to_owned()
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

/// Publishes a runtime-only edit status to every live editor surface.
///
/// Status never enters the document or history; a destroyed Pattern Editor has
/// no surface and therefore cannot receive a stale GTK update.
fn set_inspector_status(state: &mut AppState, message: impl Into<String>) {
    let message = message.into();
    state.inspector_runtime.status = Some(message.clone());
    state.inspector_status.set_label(&message);
    if let Some(surface) = state.pattern_editor.as_ref() {
        surface.status.set_label(&message);
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
            .pattern_definition_bundles()
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

fn set_preview_pending(state: &mut AppState) {
    state.preview_target = None;
    state.preview_coordinator.clear_submission();
    sync_main_preview_pending(state);
    set_page(state, page_while_preview_pending(state.preview.is_some()));
}

/// Synchronizes the visible main-preview spinner with the coordinator's newest pending ticket only.
fn sync_main_preview_pending(state: &AppState) {
    let pending = state.preview_coordinator.submission().is_some();
    state.preview_spinner.set_visible(pending);
    if pending {
        state.preview_spinner.start();
    } else {
        state.preview_spinner.stop();
    }
}

/// Synchronizes the private editor spinner with its independent newest pending ticket only.
fn sync_draft_preview_pending(surface: &PatternEditorSurface) {
    let pending = surface.draft.borrow().preview_submission.is_some();
    surface.preview_spinner.set_visible(pending);
    if pending {
        surface.preview_spinner.start();
    } else {
        surface.preview_spinner.stop();
    }
}

/// Schedules one main preview submission after the current GTK callback unwinds.
///
/// The idle boundary coalesces synchronous widget notifications. The scheduler
/// and preview coordinator remain the sole request/ticket authority; no GTK
/// widget value is sampled by this helper.
fn schedule_main_preview_submission(state: &Rc<RefCell<AppState>>) {
    let state = Rc::clone(state);
    glib::idle_add_local_once(move || {
        let mut app_state = state.borrow_mut();
        submit_if_viewport_ready(&mut app_state);
    });
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

/// Applies one already-typed inspector or Pattern Editor command through the
/// sole mutable document authority. Accepted edits schedule the existing
/// preview path; rejected or no-op edits preserve history, document, and the
/// last successful preview. A deliberate shared edit automatically returns to
/// selected-channel copy-on-edit after its one dispatch.
fn apply_inspector_command(
    state: &Rc<RefCell<AppState>>,
    command: &DocumentCommand,
    accepted_descriptor: Option<&PropertyDescriptor>,
    accepted_transition: bool,
    focus: Option<InspectorFocusIdentity>,
) -> bool {
    let state_handle = Rc::clone(state);
    let mut app_state = state.borrow_mut();
    let Some(workspace) = app_state.workspace.as_mut() else {
        return false;
    };
    match workspace.history.apply(command) {
        Ok(_) => {
            if let Some(descriptor) = accepted_descriptor {
                app_state
                    .inspector_runtime
                    .drafts
                    .remove(&inspector_key(descriptor));
            }
            let _ = accepted_transition;
            app_state.inspector_runtime.focus =
                focus_after_command_attempt(app_state.inspector_runtime.focus.clone(), focus, true);
            set_preview_pending(&mut app_state);
            set_inspector_status(&mut app_state, "Rendering preview…");
            sync_ui(&mut app_state);
            drop(app_state);
            schedule_main_preview_submission(&state_handle);
            true
        }
        Err(error) => {
            app_state.inspector_runtime.focus = focus_after_command_attempt(
                app_state.inspector_runtime.focus.clone(),
                focus,
                false,
            );
            set_inspector_status(&mut app_state, error.to_string());
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
        PropertyTarget::Document
        | PropertyTarget::Channel(_)
        | PropertyTarget::ChannelOutput(_, _) => return None,
    };
    document
        .pattern_definition_bundles()
        .iter()
        .find(|definition| definition.id == definition_id)
        .map(|bundle| &bundle.definition)
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
        PropertyTarget::Document
        | PropertyTarget::Channel(_)
        | PropertyTarget::ChannelOutput(_, _) => None,
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

/// Commits one typed descriptor value from either editor surface through
/// `DocumentHistory`, retaining the requested frontend focus on success.
///
/// It first rejects/disarms a stale shared audience, then delegates all value,
/// target, stale-base, and no-op validation to the existing command/domain
/// boundary. Invalid input affects only runtime status/drafts; accepted input
/// follows the established preview scheduling path and never directly mutates
/// a document from GTK.
fn commit_inspector_input_with_focus(
    state: &Rc<RefCell<AppState>>,
    descriptor: PropertyDescriptor,
    input: InspectorInput,
    focus: InspectorFocusIdentity,
) {
    if reject_stale_shared_edit_before_command(state) {
        rebuild_pattern_editor(state);
        return;
    }
    let command = {
        let state = state.borrow();
        let Some(workspace) = state.workspace.as_ref() else {
            return;
        };
        command_for_inspector_input(
            workspace.document(),
            state.inspector_runtime.selected_channel,
            state.inspector_runtime.scope.clone(),
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

/// Builds one target-aware domain command from a validated inspector value without resolving deltas in GTK.
///
/// # Errors
///
/// Returns a displayable validation message when the value kind, descriptor target, reference,
/// or domain-authored effective-value builder rejects the requested edit.
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
        PropertyFieldId::DensityAcrossX | PropertyFieldId::DensityAcrossY => {
            let axis = if descriptor.field == PropertyFieldId::DensityAcrossX {
                DensityEditedAxis::AcrossX
            } else {
                DensityEditedAxis::AcrossY
            };
            let value = f64_value(input)?;
            match descriptor.target {
                PropertyTarget::Document => document
                    .set_document_density_axis(axis, value)
                    .map_err(|error| error.to_string()),
                PropertyTarget::Channel(channel_id) => {
                    let mut density = document
                        .effective_channel_pattern(channel_id)
                        .map_err(|error| error.to_string())?
                        .density;
                    match axis {
                        DensityEditedAxis::AcrossX => density.across_x = value,
                        DensityEditedAxis::AcrossY => density.across_y = value,
                    }
                    document
                        .set_channel_density_for_effective(channel_id, axis, density)
                        .map_err(|error| error.to_string())
                }
                _ => Err("Density requires document or channel authority.".to_owned()),
            }
        }
        PropertyFieldId::DensityAspectLocked => document
            .set_document_density_aspect_lock(boolean(input)?)
            .map_err(|error| error.to_string()),
        PropertyFieldId::RotationDegrees => {
            let value = f64_value(input)?;
            match descriptor.target {
                PropertyTarget::Document => {
                    let mut settings = document.pattern_settings().clone();
                    settings.pattern_rotation_degrees = value;
                    Ok(DocumentCommand::SetDocumentPatternSettings {
                        base: document.pattern_settings().clone(),
                        settings,
                    })
                }
                PropertyTarget::Channel(channel_id) => document
                    .set_channel_pattern_rotation_for_effective(channel_id, value)
                    .map_err(|error| error.to_string()),
                _ => Err("Pattern rotation requires document or channel authority.".to_owned()),
            }
        }
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
        PropertyFieldId::MarkMinimumFill | PropertyFieldId::MarkMaximumFill => {
            let value = f64_value(input)?;
            let edit = if descriptor.field == PropertyFieldId::MarkMinimumFill {
                MarkGeometryFieldEdit::MinimumFill(value)
            } else {
                MarkGeometryFieldEdit::MaximumFill(value)
            };
            match descriptor.target {
                PropertyTarget::ChannelOutput(channel_id, output_layer_id) => document
                    .set_channel_mark_response_field_for_effective(
                        channel_id,
                        output_layer_id,
                        edit,
                    )
                    .map_err(|error| error.to_string()),
                _ => Err("Mark response requires document or channel authority.".to_owned()),
            }
        }
        PropertyFieldId::ConnectedMinimumThickness | PropertyFieldId::ConnectedMaximumThickness => {
            let value = f64_value(input)?;
            match descriptor.target {
                PropertyTarget::ChannelOutput(channel_id, output_layer_id) => {
                    let effective = document
                        .effective_channel_pattern(channel_id)
                        .map_err(|error| error.to_string())?;
                    let PatternGeometryResponse::Connected(mut response) = effective
                        .output_settings
                        .iter()
                        .find(|setting| setting.output_layer_id == output_layer_id)
                        .ok_or_else(|| "The active output is missing.".to_owned())?
                        .response
                        .clone()
                    else {
                        return Err(
                            "Path controls are inapplicable to the mark response branch."
                                .to_owned(),
                        );
                    };
                    match descriptor.field {
                        PropertyFieldId::ConnectedMinimumThickness => {
                            response.minimum_thickness = value;
                        }
                        PropertyFieldId::ConnectedMaximumThickness => {
                            response.maximum_thickness = value;
                        }
                        _ => unreachable!("connected response arm owns only connected fields"),
                    }
                    document
                        .set_channel_output_response_for_effective(
                            channel_id,
                            output_layer_id,
                            PatternGeometryResponse::Connected(response),
                        )
                        .map_err(|error| error.to_string())
                }
                _ => Err("Connected response requires document or channel authority.".to_owned()),
            }
        }
        PropertyFieldId::ShapeRotationDegrees => {
            let value = f64_value(input)?;
            match descriptor.target {
                PropertyTarget::Document => {
                    let mut settings = document.pattern_settings().clone();
                    settings.shape_rotation_degrees = value;
                    Ok(DocumentCommand::SetDocumentPatternSettings {
                        base: document.pattern_settings().clone(),
                        settings,
                    })
                }
                PropertyTarget::Channel(channel_id) => document
                    .set_channel_shape_rotation_for_effective(channel_id, value)
                    .map_err(|error| error.to_string()),
                _ => Err("Shape rotation requires document or channel authority.".to_owned()),
            }
        }
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
            PropertyReferenceValue::Definition(definition_id) => match descriptor.target {
                PropertyTarget::Document => {
                    let mut settings = document.pattern_settings().clone();
                    settings.definition_id = definition_id;
                    Ok(DocumentCommand::SetDocumentPatternSettings {
                        base: document.pattern_settings().clone(),
                        settings,
                    })
                }
                PropertyTarget::Channel(channel_id) => {
                    Ok(DocumentCommand::SetChannelPatternDefinitionOverride {
                        base: document.pattern_settings().clone(),
                        channel_id,
                        definition_id,
                    })
                }
                _ => Err("Definition selection requires document or channel authority.".to_owned()),
            },
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

/// Converts one active structural descriptor input into its existing typed
/// definition-edit command without applying it.
///
/// The descriptor target must resolve to the current definition/mechanism or
/// output layer, and the domain remains authoritative for semantic validation.
/// Selected-copy commands require a selected channel; shared commands require
/// the exact immutable disclosure arm. Returns an error for mismatched input,
/// unsupported active fields, missing targets, or stale disclosure and has no
/// document/history/preview side effects.
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
        PropertyFieldId::CoverageAdditionalMargin => {
            PatternDefinitionEdit::SetCoverageAdditionalMargin {
                additional_margin: number(input)?,
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
        PropertyFieldId::GuideOffsetSpacing => PatternDefinitionEdit::SetGuideOffsetSpacing {
            mechanism_id: mechanism_id()?,
            dimension_id: match descriptor.target {
                PropertyTarget::GuideDimension(_, _, id) => id,
                _ => return Err("Guide dimension target is required.".to_owned()),
            },
            spacing: number(input)?,
        },
        PropertyFieldId::GuideOffsetSides => match choice(input)? {
            PropertyEnumChoice::OffsetSides(sides) => PatternDefinitionEdit::SetGuideOffsetSides {
                mechanism_id: mechanism_id()?,
                dimension_id: match descriptor.target {
                    PropertyTarget::GuideDimension(_, _, id) => id,
                    _ => return Err("Guide dimension target is required.".to_owned()),
                },
                sides,
            },
            _ => return Err("Expected an offset-side choice.".to_owned()),
        },
        PropertyFieldId::GuideOffsetCleanup => match choice(input)? {
            PropertyEnumChoice::OffsetCleanup(cleanup) => {
                PatternDefinitionEdit::SetGuideOffsetCleanup {
                    mechanism_id: mechanism_id()?,
                    dimension_id: match descriptor.target {
                        PropertyTarget::GuideDimension(_, _, id) => id,
                        _ => return Err("Guide dimension target is required.".to_owned()),
                    },
                    cleanup,
                }
            }
            _ => return Err("Expected an offset-cleanup choice.".to_owned()),
        },
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
        .pattern_definition_bundles()
        .iter()
        .find(|definition| definition.id == definition_id)
        .map(|bundle| bundle.definition.clone())
        .ok_or_else(|| "The active definition is missing.".to_owned())?;
    let _ = (scope, definition_id);
    Ok(DocumentCommand::EditSelectedChannelPatternDefinition {
        channel_id: selected_channel
            .ok_or_else(|| "Select a channel before editing its definition.".to_owned())?,
        base_definition,
        edit,
    })
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

/// Executes an already-resolved lifecycle action at the GTK workspace boundary.
///
/// New/open delegate workspace creation/loading; Close clears the authoritative
/// workspace and then rebuilds presentation. The close path must not retain an
/// `AppState` borrow across Pattern Editor destruction because its GTK callback
/// synchronously re-enters the same `RefCell`. Errors are reported through the
/// existing UI boundary and no action directly edits document/history content.
fn execute_lifecycle(state: &Rc<RefCell<AppState>>, action: LifecycleAction) {
    match action {
        LifecycleAction::New => match Workspace::from_new() {
            Ok(workspace) => install_workspace(state, workspace),
            Err(error) => show_error(&mut state.borrow_mut(), error),
        },
        LifecycleAction::Open => choose_open(state),
        LifecycleAction::Close => {
            clear_workspace(state);
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
    state.pending_load || state.pending_save
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
    let (generation, event_sender) = {
        let mut state = state.borrow_mut();
        state.generation = state.generation.saturating_add(1);
        let generation = state.generation;
        state.pending_load = true;
        if state.workspace.is_none() {
            set_page(&mut state, Page::Loading);
        }
        sync_ui(&mut state);
        (generation, state.event_sender.clone())
    };
    thread::spawn(move || {
        let candidate = load_workspace(&path);
        let _ = event_sender.send_blocking(AppEvent::Load {
            generation,
            result: Box::new(candidate),
        });
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
        let event_snapshot = snapshot.clone();
        let generation = state.generation;
        let event_sender = state.event_sender.clone();
        let document = snapshot.document.clone();
        let sources = snapshot.sources.clone();
        let save_path = path.clone();
        thread::spawn(move || {
            let result =
                save_container(&save_path, &document, &sources).map_err(|error| error.to_string());
            let _ = event_sender.send_blocking(AppEvent::Save {
                generation,
                path: save_path,
                snapshot: event_snapshot,
                after,
                result,
            });
        });
        state.pending_save = true;
        sync_ui(&mut state);
    }
}

fn start_export(state: &Rc<RefCell<AppState>>, path: PathBuf, settings: ExportSettings) {
    let (snapshot, presentation, generation, workspace_generation) = {
        let mut state = state.borrow_mut();
        if state.pending_export {
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
    let event_sender = state.borrow().event_sender.clone();
    let format = settings.format;
    thread::spawn(move || {
        let result = export_snapshot(snapshot, presentation, path, settings);
        let _ = event_sender.send_blocking(AppEvent::Export {
            generation,
            workspace_generation,
            format,
            result,
        });
    });
    let mut state = state.borrow_mut();
    state.pending_export = true;
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

/// Applies one worker completion on GTK's main context without timer polling.
///
/// Workers carry immutable results only. This function retains the existing
/// generation and scheduler token gates before any view or savepoint changes.
fn handle_app_event(state: &Rc<RefCell<AppState>>, event: AppEvent) {
    match event {
        AppEvent::Load { generation, result } => {
            if state.borrow().generation != generation {
                return;
            }
            state.borrow_mut().pending_load = false;
            match *result {
                Ok(workspace) => install_workspace(state, workspace),
                Err(error) => show_error(&mut state.borrow_mut(), error),
            }
        }
        AppEvent::Save {
            generation,
            path,
            snapshot,
            after,
            result,
        } => {
            if state.borrow().generation != generation {
                return;
            }
            state.borrow_mut().pending_save = false;
            let mut app_state = state.borrow_mut();
            let saved = result.is_ok();
            match result {
                Ok(()) => {
                    if let Some(workspace) = app_state.workspace.as_mut() {
                        workspace.accept_saved_snapshot(path, snapshot);
                    }
                    emit_automation_state(&mut app_state, "save_completed", None);
                }
                Err(error) => show_error(
                    &mut app_state,
                    format!("Couldn’t save this document: {error}"),
                ),
            }
            sync_ui(&mut app_state);
            drop(app_state);
            if saved && let Some(after) = after {
                request_lifecycle(state, after);
            }
        }
        AppEvent::Export {
            generation,
            workspace_generation,
            format,
            result,
        } => {
            if state.borrow().generation != generation
                || state.borrow().workspace_generation != workspace_generation
            {
                return;
            }
            state.borrow_mut().pending_export = false;
            let mut app_state = state.borrow_mut();
            match result {
                Ok(()) => {
                    emit_automation_state(&mut app_state, "export_completed", None);
                    set_inspector_status(
                        &mut app_state,
                        format!(
                            "{} export complete.",
                            match format {
                                ExportFormat::Png => "PNG",
                                ExportFormat::Svg => "SVG",
                            }
                        ),
                    );
                }
                Err(error) => show_error(
                    &mut app_state,
                    format!("Couldn’t export this artwork: {error}"),
                ),
            }
            sync_ui(&mut app_state);
        }
        AppEvent::Preview(completion) => handle_preview_completion(state, completion),
        AppEvent::DraftPreview { epoch, completion } => {
            handle_draft_preview_completion(state, epoch, completion)
        }
    }
}

/// Accepts a scheduler completion only for its still-current workspace token.
fn handle_preview_completion(
    state: &Rc<RefCell<AppState>>,
    completion: toniator_engine::EvaluationCompletion,
) {
    let mut app_state = state.borrow_mut();
    let workspace_generation = app_state.workspace_generation;
    let ticket = completion.ticket().value();
    if !accepts_submission(
        workspace_generation,
        app_state.preview_coordinator.submission(),
        ticket,
    ) {
        return;
    }
    let accepted = app_state.workspace.as_ref().map_or(Ok(false), |workspace| {
        app_state
            .scheduler
            .accept_completion(&completion, workspace.history.session())
    });
    match accepted {
        Ok(true) => match completion.result() {
            Some(result) => match texture_from_surface(result.raster()) {
                Ok(texture) => {
                    app_state
                        .preview_coordinator
                        .accept(workspace_generation, ticket);
                    sync_main_preview_pending(&app_state);
                    app_state.picture.set_paintable(Some(&texture));
                    app_state.preview = Some(texture);
                    set_page(&mut app_state, Page::Success);
                    set_inspector_status(&mut app_state, "Preview updated.");
                    emit_automation_state(&mut app_state, "preview_accepted", Some(ticket));
                }
                Err(error) => {
                    app_state
                        .preview_coordinator
                        .fail(workspace_generation, ticket);
                    sync_main_preview_pending(&app_state);
                    show_error(&mut app_state, error);
                }
            },
            None => {
                app_state
                    .preview_coordinator
                    .fail(workspace_generation, ticket);
                sync_main_preview_pending(&app_state);
                show_error(
                    &mut app_state,
                    "Couldn’t render this pattern. Your last preview is still shown.".to_owned(),
                );
            }
        },
        Ok(false) => {
            app_state
                .preview_coordinator
                .fail(workspace_generation, ticket);
            sync_main_preview_pending(&app_state);
        }
        Err(error) => {
            app_state
                .preview_coordinator
                .fail(workspace_generation, ticket);
            sync_main_preview_pending(&app_state);
            show_error(&mut app_state, error.to_string());
        }
    }
}

/// Installs a private draft result only after its private ticket and token pass.
fn handle_draft_preview_completion(
    state: &Rc<RefCell<AppState>>,
    epoch: u64,
    completion: toniator_engine::EvaluationCompletion,
) {
    let app_state = state.borrow();
    let Some(surface) = app_state.pattern_editor.as_ref() else {
        return;
    };
    let mut draft = surface.draft.borrow_mut();
    if !accepts_draft_preview_terminal(
        draft.epoch,
        draft.preview_submission,
        epoch,
        completion.ticket().value(),
    ) {
        return;
    }
    draft.preview_submission = None;
    surface.preview_spinner.stop();
    surface.preview_spinner.set_visible(false);
    match draft
        .scheduler
        .accept_completion(&completion, draft.history.session())
    {
        Ok(true) => match completion.result() {
            Some(result) => match texture_from_surface(result.raster()) {
                Ok(texture) => {
                    surface.picture.set_paintable(Some(&texture));
                    surface.status.set_label("Draft preview updated.");
                    drop(draft);
                    drop(app_state);
                    emit_draft_automation_state(
                        &mut state.borrow_mut(),
                        "draft_preview_accepted",
                        Some(completion.ticket().value()),
                    );
                }
                Err(_) => surface
                    .status
                    .set_label("Couldn’t render this pattern. Your last preview is still shown."),
            },
            None => surface
                .status
                .set_label("Couldn’t render this pattern. Your last preview is still shown."),
        },
        Ok(false) => {}
        Err(_) => surface
            .status
            .set_label("Couldn’t render this pattern. Your last preview is still shown."),
    }
}

/// Installs a newly created or loaded workspace and closes any editor surface
/// bound to the replaced document. The next editor launch starts with fresh
/// runtime-only selection/draft state and cannot target destroyed widgets.
fn install_workspace(state: &Rc<RefCell<AppState>>, workspace: Workspace) {
    let (model, pattern_editor_window) = {
        let mut state = state.borrow_mut();
        state.workspace_generation = state.workspace_generation.saturating_add(1);
        state.preview_coordinator.clear_submission();
        sync_main_preview_pending(&state);
        state.inspector_runtime.reset_for_workspace();
        let pattern_editor_window = state.pattern_editor.take().map(|surface| {
            surface.draft.borrow_mut().preview_submission = None;
            surface.preview_spinner.stop();
            surface.preview_spinner.set_visible(false);
            surface.window
        });
        state.workspace = Some(workspace);
        state.model = state
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.document().channel_model())
            .map(PreviewModel::from_domain)
            .unwrap_or(PreviewModel::Rgb);
        (state.model, pattern_editor_window)
    };
    // The close-request callback borrows AppState, so the detached editor may
    // close only after the workspace-install borrow above has ended.
    if let Some(window) = pattern_editor_window {
        window.close();
    }
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
    let state_for_preview = Rc::clone(state);
    glib::idle_add_local_once(move || {
        let mut app_state = state_for_preview.borrow_mut();
        submit_if_viewport_ready(&mut app_state);
    });
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

/// Clears the workspace and detaches any Pattern Editor before closing it.
///
/// GTK close requests synchronously invoke the editor callback, so this
/// function takes the window while holding `AppState` and calls `close()` only
/// after that mutable `RefCell` borrow ends. Cleanup remains presentation-only
/// and never creates document/history/persistence side effects.
fn clear_workspace(state: &Rc<RefCell<AppState>>) {
    let pattern_editor_window = {
        let mut state = state.borrow_mut();
        state.generation = state.generation.saturating_add(1);
        state.workspace_generation = state.workspace_generation.saturating_add(1);
        state.preview_coordinator.clear_submission();
        sync_main_preview_pending(&state);
        state.inspector_runtime.reset_for_workspace();
        let pattern_editor_window = state.pattern_editor.take().map(|surface| {
            surface.draft.borrow_mut().preview_submission = None;
            surface.preview_spinner.stop();
            surface.preview_spinner.set_visible(false);
            surface.window
        });
        state.workspace = None;
        state.pending_load = false;
        state.pending_save = false;
        state.pending_export = false;
        clear_preview(&mut state);
        state.preview_target = None;
        state.banner.set_revealed(false);
        set_page(&mut state, Page::Empty);
        sync_ui(&mut state);
        pattern_editor_window
    };
    if let Some(window) = pattern_editor_window {
        window.close();
    }
}

/// Replaces the main channel model through its typed history command and refreshes preview.
///
/// This function rejects synchronization/loading/source-less calls before any
/// mutation. A successful change updates only the main workspace history and
/// schedules its canonical preview after GTK notifications unwind.
fn change_model(state: &Rc<RefCell<AppState>>, model: PreviewModel) {
    let mut app_state = state.borrow_mut();
    if !should_apply_model_change(
        app_state.syncing_model,
        app_state.model,
        model,
        app_state.pending_load,
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
    set_inspector_status(&mut app_state, "Rendering preview…");
    sync_ui(&mut app_state);
    drop(app_state);
    rebuild_inspector(state);
    schedule_main_preview_submission(state);
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
        pattern_instance: channel.pattern_instance.clone(),
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
    let workspace_generation = state.workspace_generation;
    state
        .preview_coordinator
        .submit(workspace_generation, ticket.value());
    sync_main_preview_pending(state);
    set_inspector_status(state, "Preview updating…");
    emit_automation_state(state, "preview_submitted", Some(ticket.value()));
    Ok(())
}

/// Emits a bounded immutable state snapshot after an authoritative UI event.
///
/// This is test synchronization evidence only: JSON fields describe the
/// existing workspace/history/preview state and are never parsed by the app.
fn emit_automation_state(state: &mut AppState, event: &str, ticket: Option<u64>) {
    let (revision, dirty, savepoint, channel, family) =
        state
            .workspace
            .as_ref()
            .map_or((None, false, false, None, None), |workspace| {
                let selected = state.inspector_runtime.selected_channel.map(|id| id.0);
                let family = state
                    .inspector_runtime
                    .selected_channel
                    .and_then(|channel| {
                        workspace
                            .document()
                            .pattern_definition_for(channel)
                            .map(|definition| format!("{:?}", definition.family))
                    });
                (
                    Some(workspace.history.session().revision().0),
                    workspace.is_dirty(),
                    workspace.savepoint.is_some(),
                    selected,
                    family,
                )
            });
    let workspace_generation = state.workspace_generation;
    let lifecycle = if state.workspace.is_some() {
        "open"
    } else {
        "empty"
    };
    let record = serde_json::json!({
        "event": event,
        "workspace_generation": workspace_generation,
        "document_revision": revision,
        "selected_channel": channel,
        "active_pattern_family": family,
        "dirty": dirty,
        "has_savepoint": savepoint,
        "lifecycle": lifecycle,
        "preview_identity": ticket.map(|ticket| serde_json::json!({"ticket": ticket, "workspace_generation": workspace_generation, "document_revision": revision})),
    });
    if let Some(sink) = state.automation.as_mut() {
        sink.emit(&record);
    }
}

/// Emits immutable private-editor evidence while retaining the main snapshot.
///
/// The draft fields describe only the cloned history and never establish a
/// second production authority. They let a harness prove that draft changes
/// leave the main revision, accepted preview, savepoint, and filesystem route
/// untouched.
fn emit_draft_automation_state(state: &mut AppState, event: &str, ticket: Option<u64>) {
    let main_revision = state
        .workspace
        .as_ref()
        .map(|workspace| workspace.history.session().revision().0);
    let main_accepted_ticket = state.preview_coordinator.last_accepted_ticket();
    let Some(surface) = state.pattern_editor.as_ref() else {
        return;
    };
    let draft = surface.draft.borrow();
    let draft_revision = draft.history.session().revision().0;
    let draft_family = draft
        .history
        .document()
        .pattern_definition_for(draft.selected_channel)
        .map(|definition| format!("{:?}", definition.family));
    let record = serde_json::json!({
        "event": event,
        "workspace_generation": state.workspace_generation,
        "main_document_revision": main_revision,
        "main_preview_accepted_ticket": main_accepted_ticket,
        "draft_epoch": draft.epoch,
        "draft_document_revision": draft_revision,
        "draft_pattern_family": draft_family,
        "draft_preview_ticket": ticket,
        "selected_channel": draft.selected_channel.0,
    });
    if let Some(sink) = state.automation.as_mut() {
        sink.emit(&record);
    }
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

#[cfg(test)]
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

/// Projects immutable application state into persistent GTK widgets.
///
/// The projection updates enabled state, labels, and presentation only; it
/// never changes workspace/history/scheduler authority or starts I/O.
fn sync_ui(state: &mut AppState) {
    let policy = ui_policy(
        state.workspace.as_ref(),
        state.pending_load,
        state.pending_save,
        state.pending_export,
    );
    let lifecycle_vm = LifecycleViewModel {
        title: policy.title.clone(),
        has_workspace: state.workspace.is_some(),
        dirty: state.workspace.as_ref().is_some_and(Workspace::is_dirty),
    };
    let selected_name = state.workspace.as_ref().and_then(|workspace| {
        state
            .selected_channel
            .map(|channel| channel_display_name(workspace.document(), channel))
    });
    let active_pattern = state.workspace.as_ref().and_then(|workspace| {
        state.selected_channel.map(|channel| {
            artist_pattern_name(&selected_property_values(workspace.document(), channel))
        })
    });
    let (document_vm, channel_vm, preview_vm) =
        project_document(state, selected_name, active_pattern);
    state.actions.new.set_enabled(policy.new_enabled);
    state.actions.open.set_enabled(policy.open_enabled);
    state.actions.close.set_enabled(policy.close_enabled);
    state.actions.save.set_enabled(policy.save_enabled);
    state.actions.save_as.set_enabled(policy.save_as_enabled);
    state.actions.export.set_enabled(policy.export_enabled);
    state.selector.set_sensitive(policy.selector_enabled);
    state.actions.undo.set_enabled(policy.undo_enabled);
    state.actions.redo.set_enabled(policy.redo_enabled);
    state.window.set_title(Some(&lifecycle_vm.title));
    state
        .window_title
        .set_title(lifecycle_vm.title.trim_end_matches(" — Toniator"));
    state
        .window
        .set_tooltip_text(Some(if lifecycle_vm.has_workspace && lifecycle_vm.dirty {
            "Unsaved changes"
        } else {
            &document_vm.title
        }));
    state.channel_selector.set_tooltip_text(
        channel_vm
            .active_pattern
            .as_deref()
            .or(channel_vm.name.as_deref()),
    );
    state.picture.set_tooltip_text(Some(if preview_vm.pending {
        "Rendering preview…"
    } else if preview_vm.accepted_ticket.is_some() {
        "Preview updated."
    } else {
        "No preview yet"
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    /// Builds an app-owned main history whose private drafts use the Pattern Editor publication boundary.
    fn private_draft_main_history() -> DocumentHistory {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 100.0,
                height: 80.0,
            },
            SourceReference::Unassigned,
        )
        .expect("default app document");
        DocumentHistory::new(DocumentSession::new(document).expect("valid app session"))
    }

    /// Builds one valid modeled GuidePaths document for inspector-command authority tests.
    fn connected_path_document() -> Document {
        let base = Document::new_default_document(
            CanvasSpec {
                width: 120.0,
                height: 80.0,
            },
            SourceReference::Unassigned,
        )
        .expect("default connected-path document");
        let guide_id = PatternMechanismId(31);
        let mut definition = toniator_domain::PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(30),
            "app inspector guide paths",
            guide_id,
            PatternMechanismId(32),
            PatternOutputLayerId(33),
            vec![toniator_domain::StraightGuideDimension {
                id: toniator_domain::GuideDimensionId(34),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: toniator_domain::StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            }],
            toniator_domain::GeneralizedSiteProduct::AlongGuides {
                dimensions: vec![toniator_domain::GuideDimensionId(34)],
                interval_multiplier: 1.0,
                phase: 0.0,
            },
            toniator_domain::MarkOrientation::Fixed,
            toniator_domain::CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
        );
        definition.output_layers = vec![toniator_domain::PatternOutputLayer::GuidePaths {
            id: PatternOutputLayerId(33),
            guide_mechanism_id: guide_id,
            style: toniator_domain::PathStrokeStyle::default(),
        }];
        let mut settings = base.pattern_settings().clone();
        settings.definition_id = definition.id;
        settings.geometry_response =
            PatternGeometryResponse::Connected(toniator_domain::ConnectedGeometryResponse {
                minimum_thickness: 0.25,
                maximum_thickness: 1.0,
            });
        Document::with_source_topology_and_authored_structures(
            toniator_domain::DocumentId(201),
            base.canvas().clone(),
            SourceReference::Unassigned,
            vec![definition],
            settings,
            base.channel_model().expect("modeled topology").to_owned(),
            base.channel_topology().expect("modeled topology").clone(),
            Vec::new(),
        )
        .expect("connected guide-path document validates")
    }

    /// Builds stale-aware document and selected-channel Connected commands through the inspector boundary.
    #[test]
    fn inspector_dispatches_connected_document_and_channel_edits() {
        let document = connected_path_document();
        let document_descriptor = document
            .property_values()
            .into_iter()
            .find(|value| {
                value.descriptor.target == PropertyTarget::Document
                    && value.descriptor.field == PropertyFieldId::ConnectedMinimumThickness
            })
            .expect("connected document thickness descriptor")
            .descriptor;
        let document_command = command_for_inspector_input(
            &document,
            Some(ChannelId(1)),
            DefinitionEditScope::SelectedCopy,
            &document_descriptor,
            InspectorInput::FiniteF64(0.4),
        )
        .expect("document connected command");
        let (document_after, _) = document
            .apply_command(&document_command)
            .expect("document connected command applies");
        let PatternGeometryResponse::Connected(response) =
            &document_after.pattern_settings().geometry_response
        else {
            panic!("document remains connected");
        };
        assert_eq!(response.minimum_thickness, 0.4);

        let channel_descriptor = document
            .property_values()
            .into_iter()
            .find(|value| {
                value.descriptor.target == PropertyTarget::Channel(ChannelId(1))
                    && value.descriptor.field == PropertyFieldId::ConnectedMaximumThickness
            })
            .expect("connected channel thickness descriptor")
            .descriptor;
        let channel_command = command_for_inspector_input(
            &document,
            Some(ChannelId(1)),
            DefinitionEditScope::SelectedCopy,
            &channel_descriptor,
            InspectorInput::FiniteF64(1.4),
        )
        .expect("channel connected command");
        let (channel_after, _) = document
            .apply_command(&channel_command)
            .expect("channel connected command applies");
        let PatternGeometryResponse::Connected(response) = channel_after
            .effective_channel_pattern(ChannelId(1))
            .expect("effective channel")
            .geometry_response
        else {
            panic!("channel remains connected");
        };
        assert_eq!(response.maximum_thickness, 1.4);
    }

    /// Proves purpose filtering and construction preflight keep ordinary Grid and Mark workflows separate.
    ///
    /// This witness uses only immutable domain projections and a private history. It creates no GTK
    /// widgets, preview submissions, or external files.
    #[test]
    fn purpose_filters_and_preflights_keep_default_mark_and_guide_workflows_explicit() {
        let mut history = private_draft_main_history();
        let channel = ChannelId(1);
        assert!(matches!(
            authored_attachment_target(
                history.document(),
                channel,
                toniator_domain::AuthoredStructureKind::ClosedShape,
            ),
            Ok(ConstructionPreflight::Ready(
                AuthoredStructureAttachment::Mark { .. }
            ))
        ));
        assert!(matches!(
            authored_attachment_target(
                history.document(),
                channel,
                toniator_domain::AuthoredStructureKind::OpenPath,
            ),
            Ok(ConstructionPreflight::ConfirmCustomAlongGuideLayout)
        ));
        let base_definition = history
            .document()
            .pattern_definition_for(channel)
            .expect("default definition")
            .clone();
        let generic = toniator_domain::PatternDefinition::generalized_guides(
            PatternDefinitionId(2),
            "private generic guide",
            PatternMechanismId(3),
            PatternMechanismId(4),
            PatternOutputLayerId(2),
            vec![toniator_domain::GuideDimension {
                id: toniator_domain::GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: toniator_domain::GuidePrototype::CircularArc {
                    center: toniator_domain::AuthoredPoint2 { x: 0.0, y: 0.0 },
                    radius: 10.0,
                    start_angle_degrees: 0.0,
                    sweep_angle_degrees: 90.0,
                },
                repetition: toniator_domain::GuideRepetition::Single,
            }],
            toniator_domain::GeneralizedSiteProduct::AlongGuides {
                dimensions: vec![toniator_domain::GuideDimensionId(1)],
                interval_multiplier: 1.0,
                phase: 0.0,
            },
            toniator_domain::MarkOrientation::Fixed,
            base_definition.coverage.clone(),
        );
        history
            .apply(&DocumentCommand::ReplaceSelectedChannelDefinitionTopology {
                channel_id: channel,
                base_definition,
                definition: generic,
            })
            .expect("private generic guide setup");
        assert!(matches!(
            authored_attachment_target(
                history.document(),
                channel,
                toniator_domain::AuthoredStructureKind::OpenPath,
            ),
            Ok(ConstructionPreflight::Ready(
                AuthoredStructureAttachment::Guide { .. }
            ))
        ));
        assert!(descriptor_belongs_to_editor(
            &PropertyFieldId::GuidePrototype,
            PatternEditorPurpose::Grid
        ));
        assert!(!descriptor_belongs_to_editor(
            &PropertyFieldId::GuidePrototype,
            PatternEditorPurpose::Mark
        ));
        assert!(descriptor_belongs_to_editor(
            &PropertyFieldId::OutputPrototype,
            PatternEditorPurpose::Mark
        ));
        assert!(!descriptor_belongs_to_editor(
            &PropertyFieldId::OutputPrototype,
            PatternEditorPurpose::Grid
        ));
    }

    /// Proves the editor uses a vertical construction layout before a narrow modal can collapse its canvas.
    ///
    /// This pure policy witness creates no GTK allocation, history, preview, or external state.
    #[test]
    fn narrow_editor_layout_stacks_before_the_canvas_becomes_a_sliver() {
        assert!(uses_narrow_editor_layout(620));
        assert!(uses_narrow_editor_layout(700));
        assert!(!uses_narrow_editor_layout(701));
        assert!(!uses_narrow_editor_layout(980));
        assert!(requested_construction_width_allows_narrow_breakpoint());
        assert!(CONSTRUCTION_CANVAS_GESTURE_HINT.contains("Scroll to zoom"));
        assert!(CONSTRUCTION_CANVAS_GESTURE_HINT.contains("middle-button drag pans"));
        assert!(CONSTRUCTION_CANVAS_GESTURE_HINT.contains("Enter completes"));
        assert!(CONSTRUCTION_CANVAS_GESTURE_HINT.contains("Escape cancels"));
    }

    fn asset(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(name)
    }

    /// Finds one active selected-channel descriptor by its typed field identity.
    ///
    /// The helper projects only the immutable private draft document. It never
    /// creates GTK controls or falls back to a stale descriptor after a
    /// selected-copy transition.
    fn private_descriptor(
        document: &Document,
        channel: ChannelId,
        field: PropertyFieldId,
    ) -> PropertyDescriptor {
        selected_property_values(document, channel)
            .into_iter()
            .find(|value| value.descriptor.field == field)
            .map(|value| value.descriptor)
            .unwrap_or_else(|| panic!("private draft is missing active {field:?}"))
    }

    /// Covers every Stage 20D presentation-only guide variant added to the inspector boundary.
    ///
    /// The witness exercises static label projection without constructing GTK
    /// controls, mutating document state, or claiming frontend edit support.
    #[test]
    fn stage20d_guide_variants_have_inspector_labels() {
        let fields = [
            (PropertyFieldId::GuidePrototype, "Guide prototype"),
            (
                PropertyFieldId::GuideAuthoredStructure,
                "Authored open path",
            ),
            (PropertyFieldId::GuideArcCenterX, "Arc center X"),
            (PropertyFieldId::GuideArcCenterY, "Arc center Y"),
            (PropertyFieldId::GuideArcRadius, "Arc radius"),
            (PropertyFieldId::GuideArcStartAngle, "Arc start angle"),
            (PropertyFieldId::GuideArcSweepAngle, "Arc sweep angle"),
            (PropertyFieldId::GuideRepetition, "Guide repetition"),
            (PropertyFieldId::GuideStackDirection, "Stack direction"),
            (
                PropertyFieldId::GuideStackSpacingMultiplier,
                "Stack spacing multiplier",
            ),
        ];
        for (field, expected) in fields {
            assert_eq!(inspector_field_label(field), expected);
        }

        let choices = [
            (
                PropertyEnumChoice::GuidePrototype(
                    toniator_domain::GuidePrototypeKind::AuthoredOpenPath,
                ),
                "Authored open path",
            ),
            (
                PropertyEnumChoice::GuidePrototype(
                    toniator_domain::GuidePrototypeKind::CircularArc,
                ),
                "Circular arc",
            ),
            (
                PropertyEnumChoice::GuideRepetition(toniator_domain::GuideRepetitionKind::Single),
                "Single",
            ),
            (
                PropertyEnumChoice::GuideRepetition(
                    toniator_domain::GuideRepetitionKind::TransformStack,
                ),
                "Transform stack",
            ),
            (
                PropertyEnumChoice::GuideRepetition(
                    toniator_domain::GuideRepetitionKind::NormalOffset,
                ),
                "Normal offset",
            ),
        ];
        for (choice, expected) in choices {
            assert_eq!(enum_choice_label(choice), expected);
        }

        assert_eq!(
            reference_label(&PropertyReferenceValue::AuthoredStructure(
                toniator_domain::AuthoredStructureId(7),
            )),
            "Authored open path"
        );
    }

    /// Submits and accepts one canonical private preview without a fixed sleep.
    ///
    /// The bounded poll mirrors the scheduler's event bridge: only the private
    /// `DocumentHistory` session may accept this completion. It reads the
    /// cloned source bundle and never touches a main workspace scheduler or
    /// preview coordinator.
    fn accept_private_preview_for_test(
        history: &DocumentHistory,
        sources: &SourceBundle,
        presentation: &SourcePresentation,
    ) {
        let source = sources
            .get(&presentation.id)
            .expect("private source bundle retains the active source");
        let resolved = ResolvedSource::new(
            presentation.id.clone(),
            Arc::<[u8]>::from(source.bytes()),
            presentation.format,
        )
        .expect("private source remains resolvable");
        let target = toniator_engine::PreviewRasterTarget::new(96, 96)
            .expect("bounded private preview target is valid");
        let scheduler = EvaluationScheduler::new().expect("private test scheduler starts");
        let ticket = scheduler
            .submit(EvaluationRequest::with_preview_target(
                history.session().document_evaluation_snapshot(),
                resolved,
                target,
            ))
            .expect("private snapshot submits through canonical scheduler");
        let deadline = Instant::now() + Duration::from_secs(15);
        let completion = loop {
            match scheduler
                .try_receive_latest()
                .expect("private scheduler result channel remains healthy")
            {
                Some(completion) => break completion,
                None if Instant::now() < deadline => thread::yield_now(),
                None => panic!("private preview did not complete before the test guard"),
            }
        };
        assert_eq!(completion.ticket(), ticket);
        assert!(
            scheduler
                .accept_completion(&completion, history.session())
                .expect("private completion validates against its private token"),
            "canonical private scheduler accepts the matching private revision"
        );
        assert!(completion.result().is_some());
        scheduler
            .shutdown()
            .expect("private test scheduler shuts down");
    }

    /// Applies Grid, Random, then Grid through one isolated draft history.
    ///
    /// The test proves family switches advance only the cloned history and
    /// retain a usable selected channel. It creates no GTK widgets, schedulers,
    /// files, or mutations of an external workspace.
    #[test]
    fn private_draft_history_switches_grid_random_grid_without_main_authority() {
        let workspace = load_workspace(&asset("raster-sample-v1.toniator"))
            .expect("frozen raster fixture opens for main-state comparison");
        let main_revision = workspace.history.revision();
        let selected = authoritative_channel_ids(workspace.document())[0];
        let mut draft = fresh_history(workspace.document().clone())
            .expect("private draft gets its own document history");
        for preset in [
            "straight-grid-circles",
            "even-random-circles",
            "straight-grid-circles",
        ] {
            PresetRegistry::bundled()
                .apply_to_selected(&mut draft, selected, preset)
                .expect("bundled family switch applies to the private channel");
        }
        assert_eq!(workspace.history.revision(), main_revision);
        assert_eq!(draft.revision().0, main_revision.0 + 3);
        assert!(draft.document().pattern_definition_for(selected).is_some());
    }

    /// Proves applying a private draft squashes once, clears main redo, and undoes exactly to its base.
    #[test]
    fn pattern_editor_apply_boundary_squashes_as_one_main_undo_step() {
        let mut main = private_draft_main_history();
        main.apply(&DocumentCommand::SetVisibility {
            channel_id: ChannelId(1),
            visible: false,
        })
        .expect("main edit creates redo witness");
        main.undo().expect("main undo creates redo branch");
        let base = main.document().clone();
        let mut draft = DocumentHistory::new_draft(&main);
        draft
            .apply(&DocumentCommand::SetVisibility {
                channel_id: ChannelId(1),
                visible: false,
            })
            .expect("private mutation");
        let final_document = draft.document().clone();
        assert!(!main.squash_draft(&draft).expect("apply squash").unchanged);
        assert_eq!(main.document(), &final_document);
        assert!(main.can_undo());
        assert!(!main.can_redo());
        main.undo().expect("one main undo");
        assert_eq!(main.document(), &base);
        assert!(!main.can_undo());
    }

    /// Proves stale Apply rejects without changing either the app-owned main history or private draft.
    #[test]
    fn pattern_editor_apply_boundary_preserves_stale_main_and_draft_histories() {
        let mut main = private_draft_main_history();
        let mut draft = DocumentHistory::new_draft(&main);
        draft
            .apply(&DocumentCommand::SetVisibility {
                channel_id: ChannelId(1),
                visible: false,
            })
            .expect("private mutation");
        let draft_document = draft.document().clone();
        let draft_revision = draft.revision();
        main.apply(&DocumentCommand::SetOpacity {
            channel_id: ChannelId(1),
            opacity: 0.5,
        })
        .expect("main change stales draft root");
        let main_document = main.document().clone();
        let main_revision = main.revision();
        assert!(main.squash_draft(&draft).is_err());
        assert_eq!(main.document(), &main_document);
        assert_eq!(main.revision(), main_revision);
        assert_eq!(draft.document(), &draft_document);
        assert_eq!(draft.revision(), draft_revision);
    }

    /// Proves Cancel/drop discards a dirty private draft without touching app-owned main authority.
    #[test]
    fn pattern_editor_cancel_boundary_discards_dirty_draft_without_main_mutation() {
        let main = private_draft_main_history();
        let main_document = main.document().clone();
        let main_revision = main.revision();
        let main_can_undo = main.can_undo();
        {
            let mut draft = DocumentHistory::new_draft(&main);
            draft
                .apply(&DocumentCommand::SetVisibility {
                    channel_id: ChannelId(1),
                    visible: false,
                })
                .expect("private mutation");
            assert_ne!(draft.document(), &main_document);
        }
        assert_eq!(main.document(), &main_document);
        assert_eq!(main.revision(), main_revision);
        assert_eq!(main.can_undo(), main_can_undo);
    }

    /// Proves a changed numeric anchor reaches the private replacement command as one history entry.
    ///
    /// This app boundary witnesses the same `commit_selected_numeric` payload and
    /// `ReplaceAuthoredStructure` authority used by GTK without creating widgets, previews, or
    /// external files. Equal values remain excluded by the widget-independent numeric helper.
    #[test]
    fn pattern_editor_numeric_anchor_commit_replaces_the_private_authored_resource_once() {
        let path = toniator_geometry::CurvePath::new(
            vec![
                toniator_geometry::CurveSegment::CubicBezier(
                    toniator_geometry::CubicBezierSegment::new(
                        toniator_geometry::Point2::new(120.0, 110.0),
                        toniator_geometry::Point2::new(180.0, 100.0),
                        toniator_geometry::Point2::new(250.0, 140.0),
                        toniator_geometry::Point2::new(306.0, 154.0),
                    )
                    .expect("finite first cubic"),
                ),
                toniator_geometry::CurveSegment::Line(
                    toniator_geometry::LineSegment::new(
                        toniator_geometry::Point2::new(306.0, 154.0),
                        toniator_geometry::Point2::new(486.0, 150.0),
                    )
                    .expect("finite second line"),
                ),
            ],
            toniator_geometry::PathClosure::Open,
        )
        .expect("connected numeric test path");
        let mut draft = private_draft_main_history();
        let created = draft
            .apply(&DocumentCommand::AddAuthoredStructure {
                draft: path
                    .to_authored_structure_draft()
                    .expect("path converts to an authored resource"),
            })
            .expect("private resource add")
            .created_authored_structure_id
            .expect("private add allocates the exact resource");
        let base = draft
            .document()
            .authored_structure(created)
            .expect("added private resource")
            .clone();
        let current = toniator_geometry::CurvePath::from_authored_structure(&base)
            .expect("private resource remains editable");
        let editor = stage20f_editor::Stage20fEditorState::default();
        assert!(
            editor
                .commit_numeric(
                    &current,
                    stage20f_editor::NumericTarget::Anchor(1),
                    "306",
                    "154",
                )
                .is_none()
        );
        let replacement = editor
            .commit_numeric(
                &current,
                stage20f_editor::NumericTarget::Anchor(1),
                "320",
                "154",
            )
            .expect("changed anchor produces a replacement payload");
        draft
            .apply(&DocumentCommand::ReplaceAuthoredStructure {
                base_structure: base,
                replacement,
            })
            .expect("changed numeric payload replaces the authored resource");
        assert!(draft.can_undo());
        assert_eq!(
            toniator_geometry::CurvePath::from_authored_structure(
                draft
                    .document()
                    .authored_structure(created)
                    .expect("replacement retains the exact resource ID"),
            )
            .expect("replacement remains editable")
            .segments()[0]
                .end(),
            toniator_geometry::Point2::new(320.0, 154.0)
        );
    }

    /// Proves duplicate activation/focus callbacks cannot defer or repeat one numeric history command.
    #[test]
    fn pattern_editor_numeric_callback_gate_is_synchronous_and_reentrancy_safe() {
        let active = Cell::new(false);
        assert!(accepts_numeric_commit_callback(&active));
        assert!(!accepts_numeric_commit_callback(&active));
        active.set(false);
        assert!(accepts_numeric_commit_callback(&active));
    }

    /// Proves a zero-motion target drag cannot reach private replacement history while a moved drag does.
    ///
    /// This app boundary mirrors the canvas release route without GTK input: the first press/release
    /// is a pure selection click, then the moved release applies one `ReplaceAuthoredStructure` and
    /// one undo restores the exact pre-drag resource document.
    #[test]
    fn pattern_editor_drag_release_skips_zero_motion_and_commits_one_moved_replacement() {
        let path = toniator_geometry::CurvePath::line(
            toniator_geometry::Point2::new(10.0, 20.0),
            toniator_geometry::Point2::new(40.0, 20.0),
        )
        .expect("finite drag test path");
        let mut draft = private_draft_main_history();
        let created = draft
            .apply(&DocumentCommand::AddAuthoredStructure {
                draft: path
                    .to_authored_structure_draft()
                    .expect("path converts to an authored resource"),
            })
            .expect("private resource add")
            .created_authored_structure_id
            .expect("private add allocates the exact resource");
        let base_document = draft.document().clone();
        let base = draft
            .document()
            .authored_structure(created)
            .expect("added private resource")
            .clone();
        let current = toniator_geometry::CurvePath::from_authored_structure(&base)
            .expect("private resource remains editable");
        let mut editor = stage20f_editor::Stage20fEditorState::default();
        editor.begin_target_drag(current.clone(), stage20f_editor::NumericTarget::Anchor(0));
        assert!(
            editor
                .end_drag(toniator_geometry::Point2::new(10.0, 20.0))
                .is_none()
        );
        assert_eq!(draft.document(), &base_document);
        editor.begin_target_drag(current, stage20f_editor::NumericTarget::Anchor(0));
        let replacement = editor
            .end_drag(toniator_geometry::Point2::new(12.0, 24.0))
            .expect("moved target yields one replacement payload");
        draft
            .apply(&DocumentCommand::ReplaceAuthoredStructure {
                base_structure: base,
                replacement,
            })
            .expect("moved target replaces the private resource");
        draft.undo().expect("one moved-drag undo");
        assert_eq!(draft.document(), &base_document);
    }

    /// Finalizes every private compound selector through draft history and previews.
    ///
    /// The sequence exercises Uniform, Even spacing, and Clustered random
    /// distributions plus seed, modulation, and exclusion edits, then Grid
    /// angle, placement, spacing, and guided orientation. Each local transition is one selected-copy command
    /// and canonical private scheduler acceptance; the captured main document,
    /// revision, and preview identity remain unchanged throughout.
    #[test]
    fn private_compound_transitions_preview_without_mutating_main_authority() {
        let workspace = load_workspace(&asset("raster-sample.png"))
            .expect("immutable raster source opens into the main workspace");
        let main_document = workspace.document().clone();
        let main_revision = workspace.history.revision();
        let selected = authoritative_channel_ids(&main_document)[0];
        let mut main_preview = preview_coordinator::PreviewCoordinator::default();
        main_preview.submit(41, 700);
        assert!(main_preview.accept(41, 700));
        let main_preview_submission = main_preview.submission();
        let main_preview_accepted = main_preview.last_accepted_ticket();

        let mut draft = fresh_history(main_document.clone())
            .expect("private editor clone starts its own history");
        let sources = workspace.sources.clone();
        let presentation = workspace
            .source_presentation
            .clone()
            .expect("direct raster workspace has private preview presentation");
        PresetRegistry::bundled()
            .apply_to_selected(&mut draft, selected, "even-random-circles")
            .expect("private history can select the random pattern");

        for choice in [
            RandomCharacterKind::RawUniform,
            RandomCharacterKind::Even,
            RandomCharacterKind::Clustered,
        ] {
            let descriptor =
                private_descriptor(draft.document(), selected, PropertyFieldId::RandomCharacter);
            let command = private_draft_command_for_input(
                draft.document(),
                selected,
                &descriptor,
                InspectorInput::EnumChoice(PropertyEnumChoice::RandomCharacter(choice)),
            )
            .expect("private random selector finalizes a typed transition");
            draft
                .apply(&command)
                .expect("private random transition applies exactly once");
        }
        let seed = private_descriptor(draft.document(), selected, PropertyFieldId::RandomSeed);
        let command = private_draft_command_for_input(
            draft.document(),
            selected,
            &seed,
            InspectorInput::U32(20260813),
        )
        .expect("private random seed has an ordinary typed command");
        draft
            .apply(&command)
            .expect("private random seed applies to private history");
        for (field, choice) in [
            (
                PropertyFieldId::RandomDensityModulation,
                PropertyEnumChoice::DensityModulation(DensityModulationKind::ArtworkWeighted),
            ),
            (
                PropertyFieldId::RandomExclusion,
                PropertyEnumChoice::Exclusion(ExclusionKind::MinimumCenterDistance),
            ),
            (
                PropertyFieldId::RandomExclusion,
                PropertyEnumChoice::Exclusion(ExclusionKind::VisibleMarkMargin),
            ),
        ] {
            let descriptor = private_descriptor(draft.document(), selected, field);
            let command = private_draft_command_for_input(
                draft.document(),
                selected,
                &descriptor,
                InspectorInput::EnumChoice(choice),
            )
            .expect("private advanced selector finalizes its domain payload");
            draft
                .apply(&command)
                .expect("private advanced selector applies to private history");
        }
        accept_private_preview_for_test(&draft, &sources, &presentation);

        PresetRegistry::bundled()
            .apply_to_selected(&mut draft, selected, "straight-grid-circles")
            .expect("private history can select the grid pattern");
        for (field, input) in [
            (
                PropertyFieldId::GuideBaselineAngle,
                InspectorInput::FiniteF64(32.5),
            ),
            (PropertyFieldId::GuidePhase, InspectorInput::FiniteF64(0.25)),
            (
                PropertyFieldId::GuideSpacingMultiplier,
                InspectorInput::FiniteF64(1.35),
            ),
        ] {
            let descriptor = private_descriptor(draft.document(), selected, field);
            let command =
                private_draft_command_for_input(draft.document(), selected, &descriptor, input)
                    .expect("private grid scalar has a typed command");
            draft
                .apply(&command)
                .expect("private grid scalar applies to private history");
        }
        let orientation = private_descriptor(
            draft.document(),
            selected,
            PropertyFieldId::OutputOrientation,
        );
        let command = private_draft_command_for_input(
            draft.document(),
            selected,
            &orientation,
            InspectorInput::EnumChoice(PropertyEnumChoice::MarkOrientation(
                MarkOrientationKind::GuideTangent,
            )),
        )
        .expect("private orientation selects a compatible domain direction");
        draft
            .apply(&command)
            .expect("private orientation transition applies once");
        accept_private_preview_for_test(&draft, &sources, &presentation);

        assert_eq!(workspace.document(), &main_document);
        assert_eq!(workspace.history.revision(), main_revision);
        assert_eq!(main_preview.submission(), main_preview_submission);
        assert_eq!(main_preview.last_accepted_ticket(), main_preview_accepted);
        assert!(draft.revision() > main_revision);
    }

    /// Realizes the compiled presentation bundle through GIO lookup before any
    /// GTK workflow uses its composite types. The test keeps resource contents
    /// distinct from document authority and fails when a tracked template or
    /// stylesheet is absent from the actual registered bundle.
    #[test]
    fn compiled_gresource_contains_every_live_composite_template() {
        register_resources();
        for path in [
            "/com/silentbutdigital/Toniator/window.ui",
            "/com/silentbutdigital/Toniator/channel-editor.ui",
            "/com/silentbutdigital/Toniator/pattern-editor.ui",
            "/com/silentbutdigital/Toniator/preset-row.ui",
            "/com/silentbutdigital/Toniator/confirmation-dialog.ui",
            "/com/silentbutdigital/Toniator/toniator.css",
        ] {
            assert!(
                gio::resources_lookup_data(path, gio::ResourceLookupFlags::NONE).is_ok(),
                "compiled GResource is missing {path}"
            );
        }
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

    /// Persists and exports both immutable stage inputs through app-owned boundaries.
    ///
    /// This portal-independent witness writes only the active Stage 19B
    /// validation directory. It proves snapshots reopen with the same document
    /// and source bundle, and that canonical raster and vector outputs retain
    /// their requested dimensions or structural SVG content. It never changes
    /// either immutable input. # Panics
    ///
    /// Panics when a documented input cannot load, persist, reopen, or export.
    #[test]
    fn stage_19b_direct_persistence_and_canonical_exports_cover_both_inputs() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/validation/stage-19b-gui-remediation/direct-boundary");
        fs::create_dir_all(&directory).expect("stage validation directory exists");
        for (input, dimensions) in [
            ("raster-sample.png", (1024, 1024)),
            ("vector-sample.svg", (900, 620)),
        ] {
            let workspace = load_workspace(&asset(input)).expect("immutable input opens");
            let snapshot = workspace.snapshot();
            let base = input.trim_end_matches(".png").trim_end_matches(".svg");
            let document = directory.join(format!("{base}.toniator"));
            save_container(&document, &snapshot.document, &snapshot.sources)
                .expect("snapshot persists without portal state");
            let reopened = load_workspace(&document).expect("saved snapshot reopens");
            assert_eq!(reopened.document(), &snapshot.document);
            assert_eq!(reopened.sources, snapshot.sources);
            let presentation = workspace
                .source_presentation
                .clone()
                .expect("direct input keeps source presentation");
            let png = directory.join(format!("{base}-canonical.png"));
            export_snapshot(
                snapshot.clone(),
                presentation.clone(),
                png.clone(),
                ExportSettings {
                    format: ExportFormat::Png,
                    background: RasterBackground::Transparent,
                    antialiasing: RasterAntialiasing::On,
                    output_target: Some(
                        OutputRasterTarget::new(dimensions.0, dimensions.1)
                            .expect("fixed export target is valid"),
                    ),
                },
            )
            .expect("canonical PNG export succeeds");
            assert_eq!(
                png_dimensions(&fs::read(&png).expect("PNG bytes exist")),
                dimensions
            );
            let svg = directory.join(format!("{base}-canonical.svg"));
            export_snapshot(
                snapshot,
                presentation,
                svg.clone(),
                ExportSettings {
                    format: ExportFormat::Svg,
                    background: RasterBackground::Transparent,
                    antialiasing: RasterAntialiasing::On,
                    output_target: None,
                },
            )
            .expect("canonical SVG export succeeds");
            let svg_text = fs::read_to_string(svg).expect("SVG text exists");
            assert!(svg_text.contains("<svg"));
            assert!(svg_text.contains("<circle "));
        }
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

    /// Proves private pending state clears only for the current epoch and newest ticket.
    #[test]
    fn draft_preview_terminal_gate_rejects_stale_epoch_and_ticket() {
        assert!(accepts_draft_preview_terminal(9, Some(14), 9, 14));
        assert!(!accepts_draft_preview_terminal(9, Some(14), 8, 14));
        assert!(!accepts_draft_preview_terminal(9, Some(14), 9, 13));
        assert!(!accepts_draft_preview_terminal(9, None, 9, 14));
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

    /// Verifies stable-ID selection retention, deterministic removal fallback,
    /// and rejection of invalid GTK selector positions without constructing GTK
    /// widgets or changing document authority.
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
        assert_eq!(
            channel_id_at_selector_position(1, &ids),
            Some(ChannelId(10)),
            "a valid GTK position resolves through authoritative document order"
        );
        assert_eq!(
            channel_id_at_selector_position(gtk::INVALID_LIST_POSITION, &ids),
            None,
            "GTK's invalid position cannot alter selected channel state"
        );
        assert_eq!(
            channel_id_at_selector_position(3, &ids),
            None,
            "out-of-range positions cannot select a stale channel"
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

    /// Proves selected-channel preset application and prepared shared
    /// replacement remain separate one-transition registry operations.
    ///
    /// The test keeps GTK out of the authority path: the UI may disclose the
    /// affected artist-facing names, but only a fresh prepared replacement can
    /// publish the shared history transition.
    #[test]
    fn prepared_shared_preset_replacement_stays_separate_from_selected_copy() {
        let mut workspace = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        replace_model_topology(&mut workspace.history, PreviewModel::Rgb)
            .expect("RGB topology creates the shared pattern audience");
        let selected = authoritative_channel_ids(workspace.document())[0];
        let definition = workspace
            .document()
            .pattern_definition_for(selected)
            .expect("selected modeled channel retains a pattern")
            .id;
        let registry = PresetRegistry::bundled();
        let revision = workspace.history.revision();
        registry
            .apply_to_selected(&mut workspace.history, selected, "even-random-circles")
            .expect("selected-copy application remains available");
        assert_eq!(workspace.history.revision().0, revision.0 + 1);
        let prepared = registry
            .prepare_shared_replacement(
                &workspace.history,
                workspace
                    .document()
                    .pattern_definition_for(selected)
                    .expect("selected copy still has a pattern")
                    .id,
                "straight-grid-circles",
            )
            .expect("shared action prepares after its own disclosure step");
        assert!(!prepared.affected_channels().is_empty());
        let revision = workspace.history.revision();
        prepared
            .confirm(&mut workspace.history)
            .expect("fresh disclosure confirms one shared transition");
        assert_eq!(workspace.history.revision().0, revision.0 + 1);
        assert_ne!(
            definition,
            workspace
                .document()
                .pattern_definition_for(selected)
                .expect("shared replacement preserves a selected pattern")
                .id
        );
    }

    /// Confirms workspace replacement clears every frontend-only inspector
    /// transient without changing the stable-ID selection fallback policy used
    /// by ordinary document history.
    /// This pure test has no GTK, document, history, preview, or panic side
    /// effects beyond the asserted runtime reset.
    #[test]
    fn inspector_runtime_resets_document_scoped_transients_but_not_ordinary_selection_logic() {
        let mut runtime = InspectorRuntime {
            selected_channel: Some(ChannelId(44)),
            scope: DefinitionEditScope::SelectedCopy,
            drafts: BTreeMap::from([("opacity".into(), "1.5".into())]),
            expanded_groups: BTreeSet::from(["Active family".into()]),
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

    /// Preserves the existing preview-page policy while asserting the bounded
    /// Pattern Editor disclosure contract: ordinary structural controls are
    /// primary and only explicit work-limit/tolerance fields are collapsed.
    /// This pure metadata test does not create GTK widgets, document history,
    /// preview state, errors, or panics beyond its assertions.
    #[test]
    fn inspector_disclosure_metadata_and_preview_pages_preserve_accepted_content() {
        assert!(is_pattern_editor_advanced_safety_descriptor(
            PropertyFieldId::CoverageGuardSteps
        ));
        assert!(is_pattern_editor_advanced_safety_descriptor(
            PropertyFieldId::RandomMaximumNeighborChecks
        ));
        assert!(!is_pattern_editor_advanced_safety_descriptor(
            PropertyFieldId::RandomSeed
        ));
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
            "Count"
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
    }

    #[test]
    /// Keeps a private Pattern Editor history independent from main workspace authority.
    ///
    /// The test applies the accepted bundled shortcut only to the clone and
    /// proves the main document remains byte-for-byte equivalent in memory.
    fn private_pattern_editor_history_never_mutates_the_main_workspace() {
        let workspace = load_workspace(&asset("raster-sample-v1.toniator")).unwrap();
        let main_document = workspace.document().clone();
        let selected = authoritative_channel_ids(&main_document)[0];
        let mut draft = fresh_history(main_document.clone()).unwrap();

        PresetRegistry::bundled()
            .apply_to_selected(&mut draft, selected, "even-random-circles")
            .unwrap();

        assert_ne!(draft.document(), &main_document);
        assert_eq!(workspace.document(), &main_document);
        assert!(!workspace.history.can_undo());
    }
}
