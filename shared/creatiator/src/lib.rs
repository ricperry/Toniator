// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared color transfers and optional native presentation helpers.
//! Document, history, and geometry authority remain with each consumer.

pub mod color;

#[cfg(feature = "gtk")]
mod theme;

#[cfg(feature = "gtk")]
pub use theme::{SystemThemeBridge, inherit_system_color_scheme};

/// Installs the embedded suite baseline before an optional application resource stylesheet.
///
/// The suite baseline uses application priority minus one, leaving application rules at
/// application priority. Native theme colors, fonts, and focus rules remain authoritative.
/// Call once for each display after registering the application's resources, on GTK's main
/// thread. A missing or invalid resource is reported through GTK's ordinary CSS diagnostics.
///
/// # Panics
/// Panics if GTK is not initialized on the calling thread.
#[cfg(feature = "gtk")]
pub fn install_styles(display: &gtk::gdk::Display, app_css_resource: Option<&str>) {
    let shared = gtk::CssProvider::new();
    shared.load_from_string(include_str!("../resources/creative-suite.css"));
    gtk::style_context_add_provider_for_display(
        display,
        &shared,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION - 1,
    );
    if let Some(resource) = app_css_resource {
        let application = gtk::CssProvider::new();
        application.load_from_resource(resource);
        gtk::style_context_add_provider_for_display(
            display,
            &application,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
