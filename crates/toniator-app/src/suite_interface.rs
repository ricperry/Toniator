//! Shared creative-suite navigation projected onto Toniator's existing GTK controls.
use crate::*;

/// Groups existing actions without changing document, Pattern, export, or animation authority.
pub fn main_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let file = gio::Menu::new();
    append(&file, &[("New", "app.new"), ("Open…", "app.open")]);
    append(&file, MAIN_MENU_SECTIONS[0]);
    append(
        &file,
        &[
            ("Select preset…", "app.select-document-preset"),
            ("Load preset…", "app.load-document-preset"),
            ("Save preset", "app.save-document-preset"),
            ("Import image sequence…", "app.import-sequence"),
        ],
    );
    append(&file, MAIN_MENU_SECTIONS[1]);
    append(&file, &[("Close", "app.close"), ("Quit", "app.exit")]);
    menu.append_submenu(Some("File"), &file);
    let edit = gio::Menu::new();
    append(&edit, &[("Undo", "app.undo"), ("Redo", "app.redo")]);
    menu.append_submenu(Some("Edit"), &edit);
    let view = gio::Menu::new();
    append(
        &view,
        &[
            ("Settings", "app.settings"),
            ("Fit", "app.fit"),
            ("Zoom in", "app.zoom-in"),
            ("Zoom out", "app.zoom-out"),
        ],
    );
    menu.append_submenu(Some("View"), &view);
    let tools = gio::Menu::new();
    append(&tools, MAIN_MENU_SECTIONS[2]);
    menu.append_submenu(Some("Tools"), &tools);
    let help = gio::Menu::new();
    append(&help, &[("Keyboard Shortcuts", "app.shortcuts")]);
    append(
        &help,
        &[("Help", "app.help"), ("About Toniator", "app.about")],
    );
    menu.append_submenu(Some("Help"), &help);
    menu
}

/// Appends a coherent action group with native menu separators and accelerator discovery.
fn append(menu: &gio::Menu, entries: &[(&str, &str)]) {
    let section = gio::Menu::new();
    for &(label, action) in entries {
        section.append(Some(label), Some(action));
    }
    menu.append_section(None, &section);
}

/// Connects suite navigation to existing controls; viewport actions remain presentation-only.
/// Weak state captures do not extend the document/window lifetime.
pub fn connect(app: &gtk::Application, state: &Rc<RefCell<AppState>>) {
    observe_dialogs(app);
    connect_canvas_keyboard(state);
    let shell = state.borrow().shell.clone();
    let drawer = shell.drawer();
    let settings =
        gio::SimpleAction::new_stateful("settings", None, &drawer.is_active().to_variant());
    let weak = Rc::downgrade(state);
    settings.connect_activate(move |_, _| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        let (window, drawer) = {
            let app = state.borrow();
            (app.window.clone(), app.shell.drawer())
        };
        if window.is_active() {
            drawer.set_active(!drawer.is_active());
        }
    });
    let weak = settings.downgrade();
    drawer.connect_toggled(move |drawer| {
        if let Some(action) = weak.upgrade() {
            action.set_state(&drawer.is_active().to_variant());
        }
    });
    app.add_action(&settings);
    // GTK's built-in F10 header traversal runs before application accelerators.
    // Capture only this explicit navigation key; native text editing remains untouched.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = app.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::F10 {
            if let Some(app) = weak.upgrade() {
                app.activate_action("main-menu", None);
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    state.borrow().window.add_controller(keys);

    for (name, keys) in [
        ("settings", &["<Primary>comma"][..]),
        ("main-menu", &["F10"][..]),
        ("shortcuts", &["<Primary>question"][..]),
        ("about", &[][..]),
        ("cycle-region", &["F6"][..]),
        ("fit", &["<Primary>0"][..]),
        (
            "zoom-in",
            &["<Primary>plus", "<Primary>equal", "<Primary>KP_Add"][..],
        ),
        ("zoom-out", &["<Primary>minus", "<Primary>KP_Subtract"][..]),
    ] {
        app.set_accels_for_action(&format!("app.{name}"), keys);
        if name == "settings" {
            continue;
        }
        let action = gio::SimpleAction::new(name, None);
        let control: Option<gtk::Widget> = {
            let app = state.borrow();
            match name {
                "zoom-in" => Some(app.zoom_in.clone().upcast()),
                "zoom-out" => Some(app.zoom_out.clone().upcast()),
                "fit" => Some(app.fit.clone().upcast()),
                _ => None,
            }
        };
        if let Some(control) = control {
            control
                .bind_property("sensitive", &action, "enabled")
                .sync_create()
                .build();
        }
        let weak = Rc::downgrade(state);
        action.connect_activate(move |_, _| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let (shell, window) = {
                let app = state.borrow();
                (app.shell.clone(), app.window.clone())
            };
            if !window.is_active() {
                return;
            }
            match name {
                "main-menu" => shell.file_button().popup(),
                "shortcuts" => {
                    let window = state.borrow().window.clone();
                    present_shortcuts(&window);
                }
                "about" => {
                    let window = state.borrow().window.clone();
                    about::present(window.upcast_ref());
                }
                "zoom-in" => {
                    let button = state.borrow().zoom_in.clone();
                    if button.is_sensitive() {
                        button.emit_clicked();
                    }
                }
                "zoom-out" => {
                    let button = state.borrow().zoom_out.clone();
                    if button.is_sensitive() {
                        button.emit_clicked();
                    }
                }
                "fit" => {
                    let button = state.borrow().fit.clone();
                    if button.is_sensitive() {
                        button.set_active(true);
                    }
                }
                "cycle-region" => cycle_region(&state),
                _ => unreachable!("only suite actions are connected"),
            }
        });
        app.add_action(&action);
    }
}

/// Moves native focus between Canvas and visible Settings, exposing a hidden drawer.
/// No document or selection state changes; the existing drawer owns responsive presentation.
fn cycle_region(state: &Rc<RefCell<AppState>>) {
    let (window, inspector, drawer, canvas) = {
        let app = state.borrow();
        (
            app.window.clone(),
            app.shell.inspector_scroll(),
            app.shell.drawer(),
            app.viewer.clone(),
        )
    };
    let region = inspector
        .parent()
        .unwrap_or_else(|| inspector.clone().upcast());
    let in_settings =
        gtk::prelude::GtkWindowExt::focus(&window).is_some_and(|focus| focus.is_ancestor(&region));
    if in_settings {
        if !canvas.is_mapped() {
            drawer.set_active(false);
        }
        canvas.grab_focus();
    } else {
        drawer.set_active(true);
        inspector.child_focus(gtk::DirectionType::TabForward);
    }
}

/// Presents local, wrapping shortcut help with native scrolling and Close/Escape dismissal.
/// The prior focused widget is restored on close when it still exists.
fn present_shortcuts(parent: &gtk::ApplicationWindow) {
    present_information(parent.upcast_ref(), "Keyboard Shortcuts", SHORTCUTS);
}

/// Presents workflow help locally; browser links remain opt-in in About.
pub fn present_help(parent: &gtk::Window) {
    present_information(
        parent,
        "Toniator Help",
        "First steps\nOpen artwork, choose a color Model, then choose Change… in Pattern Recipe. Apply a Pattern or Customize its construction. Adjust Feature size and Appearance; edits are undoable.\n\nPreview and output\nUse Preview / Source to compare, Ctrl+0 to Fit, and Ctrl++ / Ctrl+- to inspect detail. Save a .toniator project to retain source artwork and settings. Export image… writes PNG or SVG.\n\nPatterns and Presets\nA Pattern describes shapes and arrangement. A Preset stores reusable document settings while retaining the destination artwork. New options provides Presets and sequence import.\n\nNavigation and support\nF10 opens Main Menu. Ctrl+, toggles Settings. F6 cycles canvas and Settings. Ctrl+? opens Keyboard Shortcuts. About Toniator provides the user guide, feedback and source links; they open only when activated.",
    );
}

const SHORTCUTS: &str = "Documents\nCtrl+N  New    Ctrl+O  Open\nCtrl+S  Save    Ctrl+Shift+S  Save As\nCtrl+E  Export image    Ctrl+W  Close    Ctrl+Q  Quit\n\nEditing\nCtrl+Z  Undo    Ctrl+Shift+Z or Ctrl+Y  Redo\nText controls keep their native editing commands.\n\nNavigation\nF10  Main Menu    Ctrl+,  Settings\nF6  Cycle canvas / Settings\nCtrl+0  Fit    Ctrl++ / Ctrl+-  Zoom\nArrow keys  Pan when Canvas has focus\nTab / Shift+Tab  Next / previous control\nEscape  Cancel or close the current dialog\n\nHelp\nF1  Local help    Ctrl+?  Keyboard Shortcuts";

/// Builds a wrapping, scrolling information modal without editing authored state.
fn present_information(parent: &gtk::Window, title: &str, body: &str) {
    let window = gtk::Window::builder()
        .title(title)
        .accessible_role(gtk::AccessibleRole::Dialog)
        .destroy_with_parent(true)
        .transient_for(parent)
        .modal(true)
        .default_width(540)
        .default_height(520)
        .build();
    window.add_css_class("creative-suite");
    window.set_titlebar(Some(&gtk::HeaderBar::new()));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.add_css_class("suite-dialog-content");
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("suite-section-title");
    content.append(&heading);
    let text = gtk::Label::new(Some(body));
    text.set_wrap(true);
    text.set_xalign(0.0);
    text.set_selectable(true);
    content.append(&text);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&content)
        .build();
    let close = gtk::Button::with_label("Close");
    let weak = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(window) = weak.upgrade() {
            window.close();
        }
    });
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.add_css_class("suite-dialog-actions");
    actions.set_halign(gtk::Align::End);
    actions.append(&close);
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout.append(&scroll);
    layout.append(&actions);
    window.set_child(Some(&layout));
    window.set_default_widget(Some(&close));
    information_dialog(&window, parent);
    window.present();
    close.grab_focus();
    text.select_region(0, 0);
}

/// Adds theme scope, bubble Escape through existing close guards, and weak focus restoration.
/// Closing never activates Apply; existing close-request handlers retain cancellation authority.
/// A per-window CSS marker prevents duplicate handlers. Native dialogs and choosers are excluded
/// by the observer; explicit informational windows use the same lifecycle hooks.
pub fn information_dialog(window: &gtk::Window, parent: &gtk::Window) {
    if window.has_css_class("suite-dialog") {
        return;
    }
    window.add_css_class("creative-suite");
    window.add_css_class("suite-dialog");
    if let Some(header) = window.titlebar() {
        header.add_css_class("suite-header");
    }
    if window.default_widget().is_none()
        && let Some(button) = window.child().and_then(|child| safe_dismiss_button(&child))
    {
        window.set_default_widget(Some(&button));
        if let Some(actions) = button.parent().and_downcast::<gtk::Box>()
            && actions.orientation() == gtk::Orientation::Horizontal
        {
            actions.add_css_class("suite-dialog-actions");
        }
    }
    let previous = gtk::prelude::GtkWindowExt::focus(parent).map(|widget| widget.downgrade());
    window.connect_destroy(move |_| {
        if let Some(widget) = previous.as_ref().and_then(|weak| weak.upgrade()) {
            widget.grab_focus();
        }
    });
    let keys = gtk::EventControllerKey::new();
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
}

/// Observes only this application's custom transient windows, leaving native dialogs untouched.
/// Weak application captures and a per-window marker avoid retaining documents or duplicating hooks.
/// The native top-level model owns its signal; mapped custom windows have completed construction.
fn observe_dialogs(app: &gtk::Application) {
    let windows = gtk::Window::toplevels();
    let weak = app.downgrade();
    windows.connect_items_changed(move |windows, start, _, added| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        for index in start..start + added {
            let Some(window) = windows.item(index).and_downcast::<gtk::Window>() else {
                continue;
            };
            let weak = app.downgrade();
            window.connect_map(move |window| {
                let Some(app) = weak.upgrade() else {
                    return;
                };
                let Some(parent) = window.transient_for() else {
                    return;
                };
                let mut owner = Some(parent.clone());
                let mut owned = false;
                while let Some(ancestor) = owner {
                    if ancestor.application().as_ref() == Some(&app) {
                        owned = true;
                        break;
                    }
                    owner = ancestor.transient_for();
                }
                if owned
                    && window.type_() == gtk::Window::static_type()
                    && window.title().as_deref() != Some("Welcome to Toniator")
                {
                    if !window.has_css_class("suite-dialog")
                        && let Some(content) = window.child()
                    {
                        content.add_css_class("suite-dialog-content");
                    }
                    information_dialog(window, &parent);
                }
            });
        }
    });
}

/// Finds only explicit non-affirmative native buttons for a custom dialog's safe default.
/// The visible label remains authority; no Apply, Save, Discard or worker action is inferred.
fn safe_dismiss_button(widget: &gtk::Widget) -> Option<gtk::Button> {
    if let Some(button) = widget.downcast_ref::<gtk::Button>() {
        let label = button
            .label()
            .map(|label| label.replace('_', ""))
            .unwrap_or_default();
        if button.is_mapped()
            && button.is_sensitive()
            && matches!(label.as_str(), "Cancel" | "Keep editing" | "Close")
        {
            return Some(button.clone());
        }
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(button) = safe_dismiss_button(&widget) {
            return Some(button);
        }
        child = widget.next_sibling();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recurses native menu links to inspect real exported action routes rather than copied labels.
    fn actions(menu: &gio::MenuModel) -> Vec<String> {
        let mut result = Vec::new();
        for index in 0..menu.n_items() {
            if let Some(action) = menu.item_attribute_value(index, "action", None) {
                result.push(action.str().unwrap().to_owned());
            }
            for link in ["section", "submenu"] {
                if let Some(child) = menu.item_link(index, link) {
                    result.extend(actions(&child));
                }
            }
        }
        result
    }

    /// Verifies common navigation order and retention of Toniator-specific lifecycle routes.
    /// # Panics
    /// Panics when grouping, preset/animation/export routes, or separate Help/About routing regress.
    #[test]
    fn suite_menu_preserves_routes_and_order() {
        let menu = main_menu();
        let labels: Vec<_> = (0..menu.n_items())
            .map(|index| {
                menu.item_attribute_value(index, "label", None)
                    .unwrap()
                    .str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(labels, ["File", "Edit", "View", "Tools", "Help"]);
        let routes = actions(menu.upcast_ref());
        assert_eq!(
            &routes[..4],
            ["app.new", "app.open", "app.save", "app.save-as"]
        );
        for route in [
            "app.select-document-preset",
            "app.load-document-preset",
            "app.save-document-preset",
            "app.import-sequence",
            "app.export-image",
            "app.export-video",
            "app.animation-settings",
            "app.close",
            "app.exit",
            "app.undo",
            "app.redo",
            "app.settings",
            "app.fit",
            "app.zoom-in",
            "app.zoom-out",
            "app.shortcuts",
            "app.help",
            "app.about",
        ] {
            assert_eq!(
                routes
                    .iter()
                    .filter(|action| action.as_str() == route)
                    .count(),
                1,
                "{route}"
            );
        }
    }
}

/// Exposes focused-only spatial navigation through the existing viewport scroll adjustments.
/// Arrow keys never intercept text editing or mutate document/history/rendering authority.
fn connect_canvas_keyboard(state: &Rc<RefCell<AppState>>) {
    let (canvas, viewport) = {
        let app = state.borrow();
        (app.viewer.clone(), app.viewport_scroll.clone())
    };
    canvas.add_css_class("suite-canvas");
    canvas.set_focusable(true);
    canvas.update_property(&[
        gtk::accessible::Property::Label("Canvas"),
        gtk::accessible::Property::Description(
            "Rendered artwork. Arrow keys pan when zoomed; Ctrl+0 fits. F6 moves to Settings.",
        ),
    ]);
    viewport
        .hscrollbar()
        .update_property(&[gtk::accessible::Property::Label("Canvas horizontal scroll")]);
    viewport
        .vscrollbar()
        .update_property(&[gtk::accessible::Property::Label("Canvas vertical scroll")]);
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = canvas.downgrade();
    let scroll = viewport.downgrade();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        let Some(canvas) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if !canvas.has_focus() || !modifiers.is_empty() {
            return glib::Propagation::Proceed;
        }
        let Some(viewport) = scroll.upgrade() else {
            return glib::Propagation::Proceed;
        };
        let (adjustment, direction) = match key {
            gtk::gdk::Key::Left => (viewport.hadjustment(), -1.0),
            gtk::gdk::Key::Right => (viewport.hadjustment(), 1.0),
            gtk::gdk::Key::Up => (viewport.vadjustment(), -1.0),
            gtk::gdk::Key::Down => (viewport.vadjustment(), 1.0),
            _ => return glib::Propagation::Proceed,
        };
        let maximum = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
        let step = adjustment.step_increment().max(24.0);
        adjustment
            .set_value((adjustment.value() + direction * step).clamp(adjustment.lower(), maximum));
        glib::Propagation::Stop
    });
    canvas.add_controller(keys);
}
