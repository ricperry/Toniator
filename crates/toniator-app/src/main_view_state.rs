//! Main-window-only presentation state and system theme inheritance.
//!
//! This module cannot see documents, history, evaluation, or persistence. It
//! owns only viewport choices and a live projection of the desktop color
//! preference into GTK's application theme hint.

use gio::prelude::*;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Selects which existing image is shown in the shared main viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MainViewMode {
    /// Shows the newest accepted canonical pattern preview.
    #[default]
    Preview,
    /// Shows the immutable embedded source artwork.
    Source,
}

/// Retains bounded zoom and view choices without participating in document authority.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MainViewState {
    mode: MainViewMode,
    zoom_index: usize,
    fit: bool,
}

impl Default for MainViewState {
    /// Starts in the canonical Preview view fitted to the available viewport.
    fn default() -> Self {
        Self {
            mode: MainViewMode::Preview,
            zoom_index: Self::DEFAULT_ZOOM_INDEX,
            fit: true,
        }
    }
}

impl MainViewState {
    const ZOOM_LEVELS: [f64; 9] = [0.25, 0.33, 0.5, 0.67, 1.0, 1.5, 2.0, 3.0, 4.0];
    const DEFAULT_ZOOM_INDEX: usize = 4;

    /// Returns the current presentation mode without changing document or preview state.
    pub(crate) const fn mode(self) -> MainViewMode {
        self.mode
    }

    /// Selects an already available viewport image and preserves the current pan/zoom choice.
    pub(crate) fn set_mode(&mut self, mode: MainViewMode) {
        self.mode = mode;
    }

    /// Returns whether the current image is fitted to the shared viewport.
    pub(crate) const fn is_fit(self) -> bool {
        self.fit
    }

    /// Returns the current bounded zoom multiplier.
    pub(crate) fn zoom(self) -> f64 {
        Self::ZOOM_LEVELS[self.zoom_index]
    }

    /// Returns the user-facing whole-percent label for the current zoom multiplier.
    pub(crate) fn zoom_label(self) -> String {
        format!("{}%", (self.zoom() * 100.0).round() as u32)
    }

    /// Selects Fit without changing the retained manual zoom level.
    pub(crate) fn fit(&mut self) {
        self.fit = true;
    }

    /// Advances to the next bounded zoom level and leaves Fit mode.
    pub(crate) fn zoom_in(&mut self) {
        self.zoom_index = (self.zoom_index + 1).min(Self::ZOOM_LEVELS.len() - 1);
        self.fit = false;
    }

    /// Moves to the previous bounded zoom level and leaves Fit mode.
    pub(crate) fn zoom_out(&mut self) {
        self.zoom_index = self.zoom_index.saturating_sub(1);
        self.fit = false;
    }

    /// Reports whether a larger bounded zoom level remains available.
    pub(crate) const fn can_zoom_in(self) -> bool {
        self.zoom_index + 1 < Self::ZOOM_LEVELS.len()
    }

    /// Reports whether a smaller bounded zoom level remains available.
    pub(crate) const fn can_zoom_out(self) -> bool {
        self.zoom_index > 0
    }
}

/// Retains desktop preference subscriptions without document or persistence authority.
pub(crate) struct SystemThemeBridge {
    _desktop_settings: Option<gio::Settings>,
    _portal: Rc<RefCell<Option<gio::DBusProxy>>>,
}

/// Follows native GNOME preferences and the Settings portal in Flatpak/AppImage packages.
///
/// Portal reads are asynchronous and bounded; unavailable services leave GTK's fallback intact.
/// This function only reads desktop preferences and never writes them or document state.
pub(crate) fn inherit_system_color_scheme() -> SystemThemeBridge {
    let portal = Rc::new(RefCell::new(None));
    let desktop_settings = inherit_gnome_color_scheme();
    if std::env::var_os("FLATPAK_ID").is_some() || std::env::var_os("APPIMAGE").is_some() {
        subscribe_portal_color_scheme(&portal);
    }
    SystemThemeBridge {
        _desktop_settings: desktop_settings,
        _portal: portal,
    }
}

/// Retains the native GNOME setting callback when the desktop schema is available.
/// Missing schemas preserve GTK's current theme and require no backend writes.
fn inherit_gnome_color_scheme() -> Option<gio::Settings> {
    let schema_source = gio::SettingsSchemaSource::default()?;
    let schema = schema_source.lookup("org.gnome.desktop.interface", true)?;
    if !schema.has_key("color-scheme") {
        return None;
    }
    let desktop_settings =
        gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None::<&str>);
    let default_prefer_dark = gtk::Settings::default()
        .is_some_and(|settings| settings.is_gtk_application_prefer_dark_theme());
    apply_system_color_scheme(&desktop_settings, default_prefer_dark);
    desktop_settings.connect_changed(Some("color-scheme"), move |settings, _| {
        apply_system_color_scheme(settings, default_prefer_dark)
    });
    Some(desktop_settings)
}

/// Reads the Settings portal and follows its signals on GTK's main context.
/// A live signal takes precedence over an older initial reply; dropping the bridge releases the proxy.
fn subscribe_portal_color_scheme(storage: &Rc<RefCell<Option<gio::DBusProxy>>>) {
    let weak_storage = Rc::downgrade(storage);
    let fallback = gtk::Settings::default()
        .is_some_and(|settings| settings.is_gtk_application_prefer_dark_theme());
    glib::MainContext::default().spawn_local(async move {
        let Ok(proxy) = gio::DBusProxy::for_bus_future(
            gio::BusType::Session,
            gio::DBusProxyFlags::DO_NOT_LOAD_PROPERTIES,
            None,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
        )
        .await
        else {
            return;
        };
        let changed = Arc::new(AtomicBool::new(false));
        let changed_for_signal = Arc::clone(&changed);
        proxy.connect_g_signal(move |_, _, signal, parameters| {
            if signal != "SettingChanged" {
                return;
            }
            let Some((namespace, key, value)) = parameters.get::<(String, String, glib::Variant)>()
            else {
                return;
            };
            if namespace == "org.freedesktop.appearance"
                && key == "color-scheme"
                && let Some(prefer_dark) = portal_prefer_dark(&value, fallback)
            {
                changed_for_signal.store(true, Ordering::Relaxed);
                if let Some(settings) = gtk::Settings::default() {
                    settings.set_gtk_application_prefer_dark_theme(prefer_dark);
                }
            }
        });
        let reply = proxy
            .call_future(
                "ReadOne",
                Some(&("org.freedesktop.appearance", "color-scheme").to_variant()),
                gio::DBusCallFlags::NONE,
                1500,
            )
            .await;
        let Some(storage) = weak_storage.upgrade() else {
            return;
        };
        if !changed.load(Ordering::Relaxed)
            && let Ok(reply) = reply
            && let Some((value,)) = reply.get::<(glib::Variant,)>()
            && let Some(prefer_dark) = portal_prefer_dark(&value, fallback)
            && let Some(settings) = gtk::Settings::default()
        {
            settings.set_gtk_application_prefer_dark_theme(prefer_dark);
        }
        *storage.borrow_mut() = Some(proxy);
    });
}

/// Projects portal values 1/2 to dark/light; zero and unknown values retain GTK's fallback.
/// Malformed non-uint32 values are ignored, preserving the current presentation.
fn portal_prefer_dark(value: &glib::Variant, fallback: bool) -> Option<bool> {
    value.get::<u32>().map(|value| match value {
        1 => true,
        2 => false,
        _ => fallback,
    })
}

/// Projects explicit GNOME preferences and restores GTK's original hint for system default.
fn apply_system_color_scheme(desktop_settings: &gio::Settings, default_prefer_dark: bool) {
    let Some(gtk_settings) = gtk::Settings::default() else {
        return;
    };
    let prefer_dark = match desktop_settings.string("color-scheme").as_str() {
        "prefer-dark" => true,
        "prefer-light" => false,
        _ => default_prefer_dark,
    };
    gtk_settings.set_gtk_application_prefer_dark_theme(prefer_dark);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks standardized portal preferences and ignores malformed replies without inventing a theme.
    #[test]
    fn portal_color_scheme_handles_preferences_and_fallback() {
        assert_eq!(portal_prefer_dark(&1u32.to_variant(), false), Some(true));
        assert_eq!(portal_prefer_dark(&2u32.to_variant(), true), Some(false));
        assert_eq!(portal_prefer_dark(&0u32.to_variant(), true), Some(true));
        assert_eq!(portal_prefer_dark(&99u32.to_variant(), false), Some(false));
        assert_eq!(portal_prefer_dark(&"dark".to_variant(), false), None);
        let reply = (1u32.to_variant(),).to_variant();
        assert_eq!(
            reply
                .get::<(glib::Variant,)>()
                .and_then(|(v,)| portal_prefer_dark(&v, false)),
            Some(true)
        );
    }

    /// Proves viewport choices stay bounded and mode changes preserve zoom state.
    #[test]
    fn main_view_state_bounds_zoom_and_keeps_mode_independent() {
        let mut state = MainViewState::default();
        state.zoom_in();
        let zoom = state.zoom();
        state.set_mode(MainViewMode::Source);
        assert_eq!(state.mode(), MainViewMode::Source);
        assert_eq!(state.zoom(), zoom);
        for _ in 0..20 {
            state.zoom_in();
        }
        assert_eq!(state.zoom(), 4.0);
        assert!(!state.can_zoom_in());
        for _ in 0..20 {
            state.zoom_out();
        }
        assert_eq!(state.zoom(), 0.25);
        assert!(!state.can_zoom_out());
        state.fit();
        assert!(state.is_fit());
    }
}
