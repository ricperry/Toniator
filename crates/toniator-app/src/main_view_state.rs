//! Main-window-only presentation state and system theme inheritance.
//!
//! This module cannot see documents, history, evaluation, or persistence. It
//! owns only viewport choices and a live projection of the desktop color
//! preference into GTK's application theme hint.

use gio::prelude::*;

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

/// Retains the optional GNOME settings object so its change subscription stays live.
pub(crate) struct SystemThemeBridge {
    _desktop_settings: Option<gio::Settings>,
}

/// Applies and follows GNOME's color-scheme preference when its schema is available.
///
/// Missing schemas and settings backends leave GTK's own theme selection untouched.
/// This function reads desktop preferences only and never writes them.
pub(crate) fn inherit_system_color_scheme() -> SystemThemeBridge {
    let Some(schema_source) = gio::SettingsSchemaSource::default() else {
        return SystemThemeBridge {
            _desktop_settings: None,
        };
    };
    let Some(schema) = schema_source.lookup("org.gnome.desktop.interface", true) else {
        return SystemThemeBridge {
            _desktop_settings: None,
        };
    };
    if !schema.has_key("color-scheme") {
        return SystemThemeBridge {
            _desktop_settings: None,
        };
    }
    let desktop_settings =
        gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None::<&str>);
    let default_prefer_dark = gtk::Settings::default()
        .is_some_and(|settings| settings.is_gtk_application_prefer_dark_theme());
    apply_system_color_scheme(&desktop_settings, default_prefer_dark);
    desktop_settings.connect_changed(Some("color-scheme"), move |settings, _| {
        apply_system_color_scheme(settings, default_prefer_dark)
    });
    SystemThemeBridge {
        _desktop_settings: Some(desktop_settings),
    }
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
