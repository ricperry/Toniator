//! Document Preset dialogs project one domain configuration/history authority.
//! Workers own immutable snapshots and file results; only the GTK main context
//! may publish a still-current configuration into the existing workspace.

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    thread,
};

use gtk::prelude::*;
use toniator_domain::{Document, DocumentConfiguration, Revision};
use toniator_io::{
    DocumentPresetError, DocumentPresetWriteGuard, capture_document_preset_destination,
    load_document_preset, save_document_preset,
};

use crate::{AppState, InspectorTarget, PreviewModel, Workspace, app_events::AppEvent};

/// Names native menu items from their existing visible labels when GTK maps the supplied menu.
/// Menu actions retain their native roles and sensitivity; no parallel vocabulary is stored.
pub(crate) fn label_menu(button: &gtk::MenuButton) {
    if let Some(popover) = button.popover() {
        popover.connect_map(|popover| {
            let popover = popover.clone();
            glib::idle_add_local_once(move || label_menu_items(popover.upcast_ref()));
        });
    }
}

/// Walks only the supplied popover and supplies each native menu item's visible label.
fn label_menu_items(widget: &gtk::Widget) {
    if widget.accessible_role() == gtk::AccessibleRole::MenuItem {
        if let Some(label) = first_menu_label(widget) {
            widget.reset_relation(gtk::AccessibleRelation::LabelledBy);
            widget.update_property(&[gtk::accessible::Property::Label(&label.text())]);
        }
        return;
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        label_menu_items(&current);
        child = current.next_sibling();
    }
}

/// Finds the first nonempty visible GTK label within one generated native menu item.
fn first_menu_label(widget: &gtk::Widget) -> Option<gtk::Label> {
    if let Some(label) = widget.downcast_ref::<gtk::Label>()
        && !label.text().is_empty()
    {
        return Some(label.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(label) = first_menu_label(&current) {
            return Some(label);
        }
        child = current.next_sibling();
    }
    None
}

/// Names the native confirmation host from its actual heading and transient parent.
/// GTK's generated AlertDialog has no title API; matching its visible heading avoids
/// naming unrelated dialogs or depending on private widget positions.
fn name_confirmation(parent: &gtk::ApplicationWindow, heading: &str) {
    for widget in gtk::Window::list_toplevels() {
        if let Some(window) = widget.downcast_ref::<gtk::Window>()
            && window.transient_for().as_ref() == Some(parent.upcast_ref())
            && first_menu_label(&widget).is_some_and(|label| label.text() == heading)
        {
            window.reset_relation(gtk::AccessibleRelation::LabelledBy);
            window.update_property(&[gtk::accessible::Property::Label(heading)]);
        }
    }
}

/// Keeps technical file diagnostics in stderr and supplies concise recovery guidance to the UI.
fn file_error_message(error: DocumentPresetError) -> String {
    eprintln!("document Preset I/O: {error}");
    match error {
        DocumentPresetError::Filesystem { .. } => "The file could not be accessed. Check its folder and permissions.",
        DocumentPresetError::Version { .. } => "This Preset version is unsupported. Save it again with this version of Toniator.",
        DocumentPresetError::Kind { .. } => "This file is not a document Preset. Choose a .toniator-preset or intact .toniator file.",
        DocumentPresetError::StaleDestination { .. } => "The destination changed. Save again or choose another file.",
        DocumentPresetError::Limits { .. } => "This file exceeds the supported size limits. Choose a smaller file.",
        DocumentPresetError::Project { .. } => "The project is incomplete, damaged, or unsupported. Choose an intact .toniator file.",
        DocumentPresetError::DomainValidation { .. } => "These settings cannot be applied to this artwork. Choose another Preset.",
        _ => "The Preset is invalid. Choose a valid .toniator-preset or intact .toniator file.",
    }.to_owned()
}

/// Identifies an operation independently from document revisions and other workspaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RequestId {
    serial: u64,
    workspace: u64,
}

/// Describes the actual operation phase used for sensitivity and stale-result checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    LoadChooser,
    LoadRead,
    LoadConfirm,
    SaveChooser,
    SaveInspect,
    SaveConfirm,
    SaveWrite,
}

/// Holds file-operation identity and the session-only last directory, without GTK or document state.
#[derive(Default)]
pub(crate) struct OperationState {
    serial: u64,
    pending: Option<(RequestId, Phase)>,
    last_directory: Option<PathBuf>,
}

impl OperationState {
    /// Starts one exclusive operation; exhaustion or an existing operation leaves state unchanged.
    fn begin(&mut self, workspace: u64, phase: Phase) -> Option<RequestId> {
        if self.pending.is_some() {
            return None;
        }
        self.serial = self.serial.checked_add(1)?;
        let id = RequestId {
            serial: self.serial,
            workspace,
        };
        self.pending = Some((id, phase));
        Some(id)
    }

    /// Reports whether file lifecycle and private-editor entry must remain unavailable.
    pub(crate) fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Blocks document edits during load and modal save decisions, but permits edits to a saved snapshot.
    pub(crate) fn blocks_edits(&self) -> bool {
        self.pending
            .is_some_and(|(_, phase)| phase != Phase::SaveWrite)
    }

    /// Rejects callbacks from a replaced workspace, cancelled operation, or different phase.
    fn accepts(&self, id: RequestId, phase: Phase, workspace: u64) -> bool {
        id.workspace == workspace && self.pending == Some((id, phase))
    }

    /// Advances only the captured operation; old callbacks cannot retarget a later dialog.
    fn advance(&mut self, id: RequestId, phase: Phase) {
        if self.pending.is_some_and(|(current, _)| current == id) {
            self.pending = Some((id, phase));
        }
    }

    /// Finishes only the captured operation without disturbing a newer one.
    fn finish(&mut self, id: RequestId) -> bool {
        if self.pending.is_some_and(|(current, _)| current == id) {
            self.pending = None;
            true
        } else {
            false
        }
    }
}

/// Captures every authored binding input and revision before opening a load chooser.
#[derive(Clone)]
pub(crate) struct LoadCapture {
    base: Document,
    revision: Revision,
}

/// Carries immutable file results to the main context without granting workers GTK authority.
pub(crate) enum Completion {
    Read {
        id: RequestId,
        path: PathBuf,
        capture: LoadCapture,
        result: Box<Result<DocumentConfiguration, String>>,
    },
    Destination {
        id: RequestId,
        path: PathBuf,
        configuration: DocumentConfiguration,
        result: Result<DocumentPresetWriteGuard, String>,
    },
    Saved {
        id: RequestId,
        path: PathBuf,
        result: Result<Option<String>, String>,
    },
}

/// Reports whether a private authoring surface currently owns the interaction.
fn private_editor_open(state: &AppState) -> bool {
    state.pattern_wizard.is_some()
        || state.pattern_editor.is_some()
        || state.advanced_settings.is_some()
        || state.pattern_library_surface.is_some()
}

/// Projects file-operation applicability through existing GTK widgets and native actions.
pub(crate) fn sync_controls(state: &mut AppState) {
    let busy = state.document_presets.busy();
    let block_edits = crate::main_document_edits_blocked(state);
    let available = !crate::lifecycle_is_busy(state) && !private_editor_open(state);
    state
        .actions
        .load_document_preset
        .set_enabled(available && state.workspace.as_ref().is_some_and(Workspace::can_save));
    state
        .actions
        .save_document_preset
        .set_enabled(available && state.workspace.is_some());
    if busy {
        for action in [
            &state.actions.new,
            &state.actions.open,
            &state.actions.close,
            &state.actions.save,
            &state.actions.save_as,
            &state.actions.export,
        ] {
            action.set_enabled(false);
        }
    }
    state.shell.inspector_scroll().set_sensitive(!block_edits);
    state
        .channel_segments
        .set_sensitive(!block_edits && state.workspace.is_some());
    // These two groups contain the private Pattern/Advanced launch buttons.
    state.inspector_catalog.set_sensitive(!busy);
    state.advanced_controls.set_sensitive(!busy);
    state.shell.new_button().set_tooltip_text(Some(if state.workspace.as_ref().is_some_and(Workspace::can_save) {
        "New and Open projects; load reusable settings from a Preset or project, or save a source-free Preset."
    } else {
        "Open artwork before loading a Preset. Save preset captures settings without artwork."
    }));
}

/// Opens the native Preset/project chooser after capturing the current source-backed destination.
pub(crate) fn choose_load(state: &Rc<RefCell<AppState>>) {
    let (id, capture, parent) = {
        let mut app = state.borrow_mut();
        if crate::lifecycle_is_busy(&app) || private_editor_open(&app) {
            return;
        }
        let Some(workspace) = app
            .workspace
            .as_ref()
            .filter(|workspace| workspace.can_save())
        else {
            return;
        };
        let capture = LoadCapture {
            base: workspace.document().clone(),
            revision: workspace.history.revision(),
        };
        let generation = app.workspace_generation;
        let Some(id) = app.document_presets.begin(generation, Phase::LoadChooser) else {
            return;
        };
        crate::sync_ui(&mut app);
        (id, capture, app.window.clone())
    };
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Load preset");
    dialog.set_accept_label(Some("Load preset"));
    dialog.set_modal(true);
    dialog.set_filters(Some(&load_filters()));
    set_initial_folder(&dialog, &state.borrow());
    let state = Rc::clone(state);
    dialog.open(Some(&parent), None::<&gio::Cancellable>, move |result| {
        if !accepts(&state, id, Phase::LoadChooser) {
            return;
        }
        match result {
            Ok(file) => match file.path() {
                Some(path) => start_read(&state, id, path, capture),
                None => fail(
                    &state,
                    id,
                    "Choose a local Preset or Toniator project file.".into(),
                ),
            },
            Err(error) if dismissed(&error) => {
                finish(&state, id);
            }
            Err(error) => fail(
                &state,
                id,
                format!("Couldn’t open the Preset chooser: {error}"),
            ),
        }
    });
}

/// Supplies explicit filters for the two supported resource kinds without modifying project Open.
fn load_filters() -> gio::ListStore {
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    for (label, patterns) in [
        (
            "Presets and Toniator projects",
            vec!["*.toniator-preset", "*.toniator"],
        ),
        (
            "Document Presets (.toniator-preset)",
            vec!["*.toniator-preset"],
        ),
        ("Toniator projects (.toniator)", vec!["*.toniator"]),
    ] {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(label));
        for pattern in patterns {
            filter.add_pattern(pattern);
        }
        filters.append(&filter);
    }
    filters
}

/// Reads and validates both input formats off-thread while Cancel retains the existing document.
fn start_read(state: &Rc<RefCell<AppState>>, id: RequestId, path: PathBuf, capture: LoadCapture) {
    let sender = {
        let mut app = state.borrow_mut();
        app.document_presets.advance(id, Phase::LoadRead);
        app.document_presets.last_directory = path.parent().map(Path::to_path_buf);
        app.event_sender.clone()
    };
    show_progress(state, id, "Loading preset…");
    thread::spawn(move || {
        let result = load_document_preset(&path, &capture.base).map_err(file_error_message);
        let _ = sender.send_blocking(AppEvent::DocumentPreset(Completion::Read {
            id,
            path,
            capture,
            result: Box::new(result),
        }));
    });
}

/// Opens a native Save chooser for an immutable committed configuration, including source-less New.
pub(crate) fn choose_save(state: &Rc<RefCell<AppState>>) {
    let (id, configuration, parent) = {
        let mut app = state.borrow_mut();
        if crate::lifecycle_is_busy(&app) || private_editor_open(&app) {
            return;
        }
        let Some(workspace) = app.workspace.as_ref() else {
            return;
        };
        let configuration = DocumentConfiguration::capture(workspace.document());
        let generation = app.workspace_generation;
        let Some(id) = app.document_presets.begin(generation, Phase::SaveChooser) else {
            return;
        };
        crate::sync_ui(&mut app);
        (id, configuration, app.window.clone())
    };
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Save preset");
    dialog.set_accept_label(Some("Save preset"));
    dialog.set_modal(true);
    dialog.set_initial_name(Some("Untitled.toniator-preset"));
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Document Presets (.toniator-preset)"));
    filter.add_pattern("*.toniator-preset");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));
    if let Some(directory) = default_directory()
        && let Err(error) = std::fs::create_dir_all(&directory)
    {
        crate::show_error(
            &mut state.borrow_mut(),
            format!("Couldn’t prepare the Preset folder: {error}. Choose another folder."),
        );
    }
    set_initial_folder(&dialog, &state.borrow());
    let state = Rc::clone(state);
    dialog.save(Some(&parent), None::<&gio::Cancellable>, move |result| {
        if !accepts(&state, id, Phase::SaveChooser) {
            return;
        }
        match result {
            Ok(file) => match file.path() {
                Some(path) => inspect_destination(&state, id, preset_path(path), configuration),
                None => fail(&state, id, "Choose a local folder for the Preset.".into()),
            },
            Err(error) if dismissed(&error) => {
                finish(&state, id);
            }
            Err(error) => fail(
                &state,
                id,
                format!("Couldn’t open the Preset save chooser: {error}"),
            ),
        }
    });
}

/// Normalizes the actual destination before an overwrite check without replacing another extension.
fn preset_path(mut path: PathBuf) -> PathBuf {
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toniator-preset"))
    {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".toniator-preset");
        path.set_file_name(name);
    }
    path
}

/// Resolves the new resource directory independently from the structural Pattern Library root.
fn default_directory() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .map(|root| root.join("Toniator/document-presets"))
}

/// Sets an existing session/default directory without creating folders during Load.
fn set_initial_folder(dialog: &gtk::FileDialog, state: &AppState) {
    if let Some(path) = state
        .document_presets
        .last_directory
        .clone()
        .or_else(default_directory)
        .filter(|path| path.is_dir())
    {
        dialog.set_initial_folder(Some(&gio::File::for_path(path)));
    }
}

/// Captures the exact normalized destination fingerprint off-thread before explicit replacement.
fn inspect_destination(
    state: &Rc<RefCell<AppState>>,
    id: RequestId,
    path: PathBuf,
    configuration: DocumentConfiguration,
) {
    let sender = {
        let mut app = state.borrow_mut();
        app.document_presets.advance(id, Phase::SaveInspect);
        app.document_presets.last_directory = path.parent().map(Path::to_path_buf);
        app.event_sender.clone()
    };
    show_progress(state, id, "Preparing preset…");
    thread::spawn(move || {
        let result = capture_document_preset_destination(&path).map_err(file_error_message);
        let _ = sender.send_blocking(AppEvent::DocumentPreset(Completion::Destination {
            id,
            path,
            configuration,
            result,
        }));
    });
}

/// Publishes the captured snapshot while allowing ordinary edits; completion never changes savepoints.
fn start_write(
    state: &Rc<RefCell<AppState>>,
    id: RequestId,
    path: PathBuf,
    configuration: DocumentConfiguration,
    guard: DocumentPresetWriteGuard,
) {
    close_progress(state);
    let sender = {
        let mut app = state.borrow_mut();
        app.document_presets.advance(id, Phase::SaveWrite);
        crate::sync_ui(&mut app);
        app.event_sender.clone()
    };
    thread::spawn(move || {
        let result = save_document_preset(&path, &configuration, &guard)
            .map(|outcome| outcome.durability_warning)
            .map_err(file_error_message);
        let _ = sender.send_blocking(AppEvent::DocumentPreset(Completion::Saved {
            id,
            path,
            result,
        }));
    });
}

/// Applies only an expected-phase completion to the current operation and workspace.
pub(crate) fn complete(state: &Rc<RefCell<AppState>>, completion: Completion) {
    match completion {
        Completion::Read {
            id,
            path,
            capture,
            result,
        } => {
            if !accepts(state, id, Phase::LoadRead) {
                return;
            }
            close_progress(state);
            match *result {
                Ok(configuration) => confirm_load(state, id, &path, capture, configuration),
                Err(error) => fail(state, id, format!("Couldn’t load preset. {error}")),
            }
        }
        Completion::Destination {
            id,
            path,
            configuration,
            result,
        } => {
            if !accepts(state, id, Phase::SaveInspect) {
                return;
            }
            close_progress(state);
            match result {
                Ok(guard) if guard.is_existing() => {
                    confirm_overwrite(state, id, path, configuration, guard)
                }
                Ok(guard) => start_write(state, id, path, configuration, guard),
                Err(error) => fail(
                    state,
                    id,
                    format!("Couldn’t prepare the Preset destination. {error}"),
                ),
            }
        }
        Completion::Saved { id, path, result } => {
            if !accepts(state, id, Phase::SaveWrite) {
                return;
            }
            match result {
                Ok(warning) => {
                    finish(state, id);
                    let mut app = state.borrow_mut();
                    let message = match warning {
                        Some(warning) => format!(
                            "Preset saved to {}. Directory durability could not be confirmed: {warning}",
                            path.display()
                        ),
                        None => format!(
                            "Preset saved as {}. The project’s saved state is unchanged.",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ),
                    };
                    crate::set_inspector_status(&mut app, message);
                    crate::emit_automation_state(&mut app, "document_preset_saved", None);
                }
                Err(error) => fail(state, id, format!("Couldn’t save the Preset. {error}")),
            }
        }
    }
}

/// Discloses whole-document replacement and retains a safe Cancel default before the atomic command.
fn confirm_load(
    state: &Rc<RefCell<AppState>>,
    id: RequestId,
    path: &Path,
    capture: LoadCapture,
    configuration: DocumentConfiguration,
) {
    state
        .borrow_mut()
        .document_presets
        .advance(id, Phase::LoadConfirm);
    let model = configuration
        .channel_model()
        .map(PreviewModel::from_domain)
        .map(PreviewModel::label)
        .unwrap_or("stored channel configuration");
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let detail = format!(
        "Load {name} using {model}? This replaces all channel Patterns and settings. Your current artwork and canvas stay in place. Undo restores your previous design."
    );
    let dialog = gtk::AlertDialog::builder()
        .message("Load preset into this project?")
        .detail(&detail)
        .modal(true)
        .build();
    dialog.set_buttons(&["Cancel", "Load preset"]);
    dialog.set_cancel_button(0);
    dialog.set_default_button(0);
    let parent = state.borrow().window.clone();
    let state = Rc::clone(state);
    dialog.choose(Some(&parent), None::<&gio::Cancellable>, move |response| {
        if !accepts(&state, id, Phase::LoadConfirm) { return; }
        if response == Ok(1) {
            let result = {
                let mut app = state.borrow_mut();
                app.workspace.as_mut().ok_or_else(|| "The destination project is no longer open.".to_owned())
                    .and_then(|workspace| apply_configuration(workspace, &capture, &configuration))
            };
            match result {
                Ok(changed) => {
                    finish(&state, id);
                    if changed {
                        state.borrow_mut().inspector_runtime.reset_for_workspace();
                        state.borrow_mut().inspector_runtime.target = InspectorTarget::DocumentAll;
                        refresh_configuration(&state);
                    }
                    let mut app = state.borrow_mut();
                    crate::set_inspector_status(&mut app, if changed { "Preset loaded for all channels. Undo restores the previous design." } else { "This Preset already matches the project settings." });
                    crate::emit_automation_state(&mut app, "document_preset_loaded", None);
                }
                Err(error) => fail(&state, id, format!("Couldn’t apply the Preset: {error}. Load it again against the current project.")),
            }
        } else { finish(&state, id); }
    });
    name_confirmation(&parent, "Load preset into this project?");
}

/// Applies configuration through domain history while leaving source, location and savepoint untouched.
///
/// # Errors
/// Returns stale-base/revision or domain-validation errors without modifying the workspace.
fn apply_configuration(
    workspace: &mut Workspace,
    capture: &LoadCapture,
    configuration: &DocumentConfiguration,
) -> Result<bool, String> {
    workspace
        .history
        .apply_document_configuration(&capture.base, capture.revision, configuration)
        .map(|result| !result.unchanged)
        .map_err(|error| error.to_string())
}

/// Refreshes model, inspector and canonical preview after whole-document load or history navigation.
pub(crate) fn refresh_configuration(state: &Rc<RefCell<AppState>>) {
    let model = {
        let mut app = state.borrow_mut();
        app.model = app
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.document().channel_model())
            .map(PreviewModel::from_domain)
            .unwrap_or(PreviewModel::Rgb);
        app.model
    };
    crate::sync_model_selector(state, model);
    {
        let mut app = state.borrow_mut();
        crate::update_backdrop(&mut app);
        crate::set_preview_pending(&mut app);
        crate::sync_ui(&mut app);
    }
    crate::rebuild_inspector(state);
    crate::schedule_main_preview_submission(state);
}

/// Confirms the exact normalized file and retains its observed fingerprint until publication.
fn confirm_overwrite(
    state: &Rc<RefCell<AppState>>,
    id: RequestId,
    path: PathBuf,
    configuration: DocumentConfiguration,
    guard: DocumentPresetWriteGuard,
) {
    state
        .borrow_mut()
        .document_presets
        .advance(id, Phase::SaveConfirm);
    let dialog = gtk::AlertDialog::builder()
        .message("Replace this Preset file?")
        .detail(format!(
            "{} already exists. Replace it with the captured project settings?",
            path.display()
        ))
        .modal(true)
        .build();
    dialog.set_buttons(&["Cancel", "Replace preset"]);
    dialog.set_cancel_button(0);
    dialog.set_default_button(0);
    let parent = state.borrow().window.clone();
    let state = Rc::clone(state);
    dialog.choose(Some(&parent), None::<&gio::Cancellable>, move |response| {
        if !accepts(&state, id, Phase::SaveConfirm) {
            return;
        }
        if response == Ok(1) {
            start_write(&state, id, path, configuration, guard);
        } else {
            finish(&state, id);
        }
    });
    name_confirmation(&parent, "Replace this Preset file?");
}

/// Shows cancellable read/preparation progress in a transient native GTK window.
fn show_progress(state: &Rc<RefCell<AppState>>, id: RequestId, message: &str) {
    let window = gtk::Window::builder()
        .title(message)
        .transient_for(&state.borrow().window)
        .modal(true)
        .default_width(380)
        .resizable(false)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.append(&gtk::Label::new(Some(message)));
    let cancel = gtk::Button::with_mnemonic("_Cancel");
    content.append(&cancel);
    window.set_child(Some(&content));
    let state_for_cancel = Rc::clone(state);
    cancel.connect_clicked(move |_| {
        finish(&state_for_cancel, id);
    });
    let state_for_close = Rc::clone(state);
    window.connect_close_request(move |_| {
        finish(&state_for_close, id);
        glib::Propagation::Proceed
    });
    crate::configure_private_dialog_keyboard(&window, &cancel);
    state.borrow_mut().document_preset_progress = Some(window.clone());
    window.present();
    cancel.grab_focus();
}

/// Detaches before destroying the progress surface so callbacks cannot reenter an AppState borrow.
fn close_progress(state: &Rc<RefCell<AppState>>) {
    let window = state.borrow_mut().document_preset_progress.take();
    if let Some(window) = window {
        window.destroy();
    }
}

/// Tests both the operation phase and the actual current workspace generation.
fn accepts(state: &Rc<RefCell<AppState>>, id: RequestId, phase: Phase) -> bool {
    let app = state.borrow();
    app.document_presets
        .accepts(id, phase, app.workspace_generation)
}

/// Releases the captured operation and restores the invoking New menu's keyboard focus.
fn finish(state: &Rc<RefCell<AppState>>, id: RequestId) -> bool {
    if !state.borrow_mut().document_presets.finish(id) {
        return false;
    }
    close_progress(state);
    crate::sync_ui(&mut state.borrow_mut());
    let weak = Rc::downgrade(state);
    glib::idle_add_local_once(move || {
        if let Some(state) = weak.upgrade() {
            let app = state.borrow();
            if !app.document_presets.busy() {
                app.shell.new_button().grab_focus();
            }
        }
    });
    true
}

/// Reports an actionable failure after releasing busy state; the existing document remains authoritative.
fn fail(state: &Rc<RefCell<AppState>>, id: RequestId, message: String) {
    if finish(state, id) {
        let mut app = state.borrow_mut();
        crate::set_inspector_status(&mut app, &message);
        crate::show_error(&mut app, message);
        crate::emit_automation_state(&mut app, "document_preset_failed", None);
    }
}

/// Distinguishes native dialog cancellation from failures that need an actionable message.
fn dismissed(error: &glib::Error) -> bool {
    error.matches(gtk::DialogError::Dismissed)
        || error.matches(gtk::DialogError::Cancelled)
        || error.matches(gio::IOErrorEnum::Cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_domain::DocumentCommand;

    /// Exercises exclusivity, phase sensitivity and stale callbacks without GTK or file writes.
    #[test]
    fn document_preset_operations_reject_stale_callbacks_and_gate_edits() {
        let mut state = OperationState::default();
        let old = state.begin(4, Phase::LoadChooser).unwrap();
        assert!(state.blocks_edits());
        assert!(state.begin(4, Phase::SaveChooser).is_none());
        assert!(!state.accepts(old, Phase::LoadChooser, 5));
        state.advance(old, Phase::LoadRead);
        assert!(!state.accepts(old, Phase::LoadChooser, 4));
        assert!(state.finish(old));
        let new = state.begin(4, Phase::SaveChooser).unwrap();
        assert!(!state.finish(old));
        state.advance(new, Phase::SaveWrite);
        assert!(state.busy());
        assert!(!state.blocks_edits());
    }

    /// Verifies exact workspace retention, savepoint history and no-op redo across both immutable sources.
    #[test]
    fn document_preset_load_preserves_project_source_canvas_savepoint_and_history() {
        for (bytes, hint) in [
            (
                include_bytes!("../../../assets/raster-sample.png").as_slice(),
                crate::SourceFormatHint::Png,
            ),
            (
                include_bytes!("../../../assets/vector-sample.svg").as_slice(),
                crate::SourceFormatHint::Svg,
            ),
        ] {
            let mut workspace =
                Workspace::from_direct(bytes.into(), hint, "Original artwork".into()).unwrap();
            workspace
                .accept_saved_snapshot(PathBuf::from("original.toniator"), workspace.snapshot());
            let before = workspace.snapshot();
            let capture = LoadCapture {
                base: workspace.document().clone(),
                revision: workspace.history.revision(),
            };
            let mut donor = workspace.document().clone();
            let base = donor.pattern_settings().clone();
            let mut settings = base.clone();
            settings.density.density *= 0.5;
            donor = donor
                .apply_command(&DocumentCommand::SetDocumentPatternSettings { base, settings })
                .unwrap()
                .0;
            let config = DocumentConfiguration::capture(&donor);
            assert!(apply_configuration(&mut workspace, &capture, &config).unwrap());
            assert_eq!(workspace.sources, before.sources);
            assert_eq!(workspace.document().source(), before.document.source());
            assert_eq!(workspace.document().canvas(), before.document.canvas());
            assert_eq!(workspace.location, Some(PathBuf::from("original.toniator")));
            assert!(workspace.is_dirty());
            assert!(apply_configuration(&mut workspace, &capture, &config).is_err());
            workspace.history.undo().unwrap();
            assert!(!workspace.is_dirty());
            let same = LoadCapture {
                base: workspace.document().clone(),
                revision: workspace.history.revision(),
            };
            assert!(
                !apply_configuration(
                    &mut workspace,
                    &same,
                    &DocumentConfiguration::capture(&same.base)
                )
                .unwrap()
            );
            assert!(workspace.history.can_redo());
            workspace.history.redo().unwrap();
            assert_eq!(DocumentConfiguration::capture(workspace.document()), config);
        }
    }
}
