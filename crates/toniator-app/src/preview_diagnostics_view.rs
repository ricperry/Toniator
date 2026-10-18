//! Shared GTK presentation of immutable local preview failure evidence.
use crate::preview_diagnostics::{Diagnostic, Snapshot};
use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Owns one surface's Details route and terminal pre-ticket intent deduplication.
#[derive(Clone)]
pub(crate) struct Controls {
    pub(crate) button: gtk::Button,
    snapshot: Rc<RefCell<Option<Arc<Snapshot>>>>,
    failed_intent: Rc<RefCell<Option<String>>>,
    source: Rc<RefCell<Option<serde_json::Value>>>,
    current_intent: Rc<RefCell<Option<String>>>,
}
impl Controls {
    /// Adds a native Details button next to the existing failure status without new document authority.
    /// # Panics
    /// Panics if a compiled surface status no longer belongs to its required vertical GtkBox.
    pub(crate) fn new(parent: &impl IsA<gtk::Window>, status: &gtk::Label) -> Self {
        let button = gtk::Button::with_label("Details…");
        button.set_halign(gtk::Align::Start);
        button.set_visible(false);
        button.set_tooltip_text(Some(
            "View and copy this preview failure's local technical details.",
        ));
        let container = status
            .parent()
            .and_then(|w| w.downcast::<gtk::Box>().ok())
            .expect("compiled preview status belongs to a GtkBox");
        container.insert_child_after(&button, Some(status));
        Self::for_button(parent, &button)
    }
    /// Connects an existing native Details button to immutable snapshots and focus restoration.
    pub(crate) fn for_button(parent: &impl IsA<gtk::Window>, button: &gtk::Button) -> Self {
        let snapshot = Rc::new(RefCell::new(None::<Arc<Snapshot>>));
        let captured = snapshot.clone();
        let parent = parent.as_ref().downgrade();
        let invoker = button.downgrade();
        button.connect_clicked(move |_| {
            if let (Some(parent), Some(invoker), Some(snapshot)) = (
                parent.upgrade(),
                invoker.upgrade(),
                captured.borrow().clone(),
            ) {
                present(&parent, &invoker, snapshot);
            }
        });
        Self {
            button: button.clone(),
            snapshot,
            failed_intent: Rc::new(RefCell::new(None)),
            source: Rc::new(RefCell::new(None)),
            current_intent: Rc::new(RefCell::new(None)),
        }
    }
    /// Rejects automatic same-intent repeats before source/scheduler work and clears obsolete details.
    pub(crate) fn admit(&self, intent: &str) -> bool {
        if self.failed_intent.borrow().as_deref() == Some(intent) {
            return false;
        }
        if self.current_intent.borrow().as_deref() != Some(intent) {
            self.clear();
            *self.current_intent.borrow_mut() = Some(intent.to_owned());
        }
        true
    }
    /// Records one admitted terminal failure and exposes its short cause and immutable Details route.
    pub(crate) fn fail(
        &self,
        intent: &str,
        diagnostic: Diagnostic,
        mut context: serde_json::Value,
        status: &gtk::Label,
    ) {
        if diagnostic.is_cancelled() || self.failed_intent.borrow().as_deref() == Some(intent) {
            return;
        }
        if let Some(source) = self.source.borrow().as_ref() {
            context["source"] = source.clone();
        }
        let snapshot = Snapshot::record(diagnostic, context);
        status.set_label(snapshot.diagnostic.summary());
        status.set_tooltip_text(Some(&snapshot.diagnostic.to_string()));
        *self.snapshot.borrow_mut() = Some(snapshot);
        *self.failed_intent.borrow_mut() = Some(intent.to_owned());
        self.button.set_visible(true);
        self.button.set_sensitive(true);
    }
    /// Installs an already recorded main-preview snapshot without writing or changing failure intent.
    pub(crate) fn install(&self, snapshot: Arc<Snapshot>) {
        *self.snapshot.borrow_mut() = Some(snapshot);
        self.button.set_sensitive(true);
    }
    /// Removes obsolete details on accepted success or a concrete changed private intent.
    pub(crate) fn clear(&self) {
        self.snapshot.borrow_mut().take();
        self.failed_intent.borrow_mut().take();
        self.button.set_visible(false);
    }
    /// Retains only actual private input format/dimensions captured during source preparation.
    pub(crate) fn set_source(&self, source: serde_json::Value) {
        *self.source.borrow_mut() = Some(source);
    }
    /// Returns the captured snapshot for read-only cause projection and explicit worker recovery.
    pub(crate) fn snapshot(&self) -> Option<Arc<Snapshot>> {
        self.snapshot.borrow().clone()
    }
}

/// Presents selectable bounded evidence with pinned Copy/Close, Escape and invoking-button focus.
/// Local-only wording discloses that raw technical messages may contain local paths.
fn present(parent: &gtk::Window, invoker: &gtk::Button, snapshot: Arc<Snapshot>) {
    let window = gtk::Window::builder()
        .title("Preview failure details")
        .transient_for(parent)
        .modal(true)
        .default_width(480)
        .default_height(360)
        .build();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
    root.set_margin_start(12);
    root.set_margin_end(12);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let cause = gtk::Label::new(Some(snapshot.diagnostic.summary()));
    cause.set_wrap(true);
    cause.set_xalign(0.0);
    cause.add_css_class("heading");
    body.append(&cause);
    for suggestion in snapshot.suggestions() {
        let label = gtk::Label::new(Some(suggestion));
        label.set_wrap(true);
        label.set_xalign(0.0);
        body.append(&label);
    }
    let technical_heading = gtk::Label::new(Some("Technical details"));
    technical_heading.set_xalign(0.0);
    body.append(&technical_heading);
    let technical = gtk::TextView::new();
    technical.set_editable(false);
    technical.set_cursor_visible(true);
    technical.set_wrap_mode(gtk::WrapMode::WordChar);
    technical.buffer().set_text(&snapshot.details());
    technical.update_property(&[gtk::accessible::Property::Label("Technical details")]);
    technical.update_relation(&[gtk::accessible::Relation::LabelledBy(&[
        technical_heading.upcast_ref()
    ])]);
    body.append(&technical);
    scroll.set_child(Some(&body));
    root.append(&scroll);
    let status = gtk::Label::new(None);
    status.set_wrap(true);
    status.set_xalign(0.0);
    root.append(&status);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let copy = gtk::Button::with_label("Copy details");
    let close = gtk::Button::with_label("Close");
    actions.append(&copy);
    actions.append(&close);
    root.append(&actions);
    window.set_child(Some(&root));
    let details = snapshot.details();
    let clipboard = window.clipboard();
    copy.connect_clicked(move |_| {
        clipboard.set_text(&details);
        status.set_label("Details copied.");
        status.update_property(&[gtk::accessible::Property::Description(
            "Details copied to the clipboard.",
        )]);
    });
    let weak = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(window) = weak.upgrade() {
            window.close();
        }
    });
    let invoker = invoker.downgrade();
    window.connect_close_request(move |_| {
        let invoker = invoker.clone();
        glib::idle_add_local_once(move || {
            if let Some(invoker) = invoker.upgrade() {
                invoker.grab_focus();
            }
        });
        glib::Propagation::Proceed
    });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = window.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(window) = weak.upgrade() {
                window.close();
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);
    window.present();
    close.grab_focus();
}
