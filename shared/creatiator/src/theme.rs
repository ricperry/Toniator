// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only bridge from GNOME and portal color-scheme preferences to GTK.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gio::prelude::*;
use glib::variant::ToVariant;
use gtk::{gio, glib};

/// Retains live system-preference subscriptions for the application lifetime.
pub struct SystemThemeBridge {
    _desktop_settings: Option<gio::Settings>,
    _portal: Rc<RefCell<Option<gio::DBusProxy>>>,
}

/// Follows GNOME and sandbox portal preferences without writing host settings.
///
/// Call on GTK's initialized main thread and retain the returned bridge for the
/// application lifetime. Missing schemas or portal services preserve GTK's fallback;
/// portal reads are asynchronous and have a 1,500 ms reply timeout. This function
/// never changes a document or writes GNOME/portal settings.
///
/// # Panics
/// Panics if GTK is not initialized on the calling thread.
pub fn inherit_system_color_scheme() -> SystemThemeBridge {
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

/// Reads the available GNOME schema and retains its color-scheme subscription.
/// Missing schemas or keys preserve the current GTK preference without writing settings.
fn inherit_gnome_color_scheme() -> Option<gio::Settings> {
    let schema_source = gio::SettingsSchemaSource::default()?;
    let schema = schema_source.lookup("org.gnome.desktop.interface", true)?;
    if !schema.has_key("color-scheme") {
        return None;
    }
    let settings = gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None::<&str>);
    let fallback =
        gtk::Settings::default().is_some_and(|s| s.is_gtk_application_prefer_dark_theme());
    apply_gnome(&settings, fallback);
    settings.connect_changed(Some("color-scheme"), move |settings, _| {
        apply_gnome(settings, fallback)
    });
    Some(settings)
}

/// Starts a bounded asynchronous portal read and retains live preference signals.
/// A signal received during the read takes precedence over the initial reply;
/// the weak storage reference prevents the task from retaining the bridge itself.
fn subscribe_portal_color_scheme(storage: &Rc<RefCell<Option<gio::DBusProxy>>>) {
    let weak = Rc::downgrade(storage);
    let fallback =
        gtk::Settings::default().is_some_and(|s| s.is_gtk_application_prefer_dark_theme());
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
        let signal_changed = Arc::clone(&changed);
        proxy.connect_g_signal(move |_, _, signal, parameters| {
            let Some((namespace, key, value)) = parameters.get::<(String, String, glib::Variant)>()
            else {
                return;
            };
            if signal == "SettingChanged"
                && namespace == "org.freedesktop.appearance"
                && key == "color-scheme"
                && let Some(dark) = portal_prefer_dark(&value, fallback)
            {
                signal_changed.store(true, Ordering::Relaxed);
                if let Some(settings) = gtk::Settings::default() {
                    settings.set_gtk_application_prefer_dark_theme(dark);
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
        let Some(storage) = weak.upgrade() else {
            return;
        };
        if !changed.load(Ordering::Relaxed)
            && let Ok(reply) = reply
            && let Some((value,)) = reply.get::<(glib::Variant,)>()
            && let Some(dark) = portal_prefer_dark(&value, fallback)
            && let Some(settings) = gtk::Settings::default()
        {
            settings.set_gtk_application_prefer_dark_theme(dark);
        }
        *storage.borrow_mut() = Some(proxy);
    });
}

/// Projects portal preferences 1/2 to dark/light and preserves fallback otherwise.
/// Malformed values return `None` without changing GTK's preference.
fn portal_prefer_dark(value: &glib::Variant, fallback: bool) -> Option<bool> {
    value.get::<u32>().map(|value| match value {
        1 => true,
        2 => false,
        _ => fallback,
    })
}

/// Projects explicit GNOME preferences into GTK, preserving fallback for default.
/// GTK settings are presentation-only; the desktop settings object is never written.
fn apply_gnome(settings: &gio::Settings, fallback: bool) {
    let Some(gtk_settings) = gtk::Settings::default() else {
        return;
    };
    let dark = match settings.string("color-scheme").as_str() {
        "prefer-dark" => true,
        "prefer-light" => false,
        _ => fallback,
    };
    gtk_settings.set_gtk_application_prefer_dark_theme(dark);
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Preserves explicit portal preferences, fallback values, and malformed-type rejection.
    ///
    /// # Panics
    /// Panics if the shared theme bridge changes the established portal projection.
    #[test]
    fn portal_values_keep_native_fallback_for_unknown_and_reject_wrong_types() {
        assert_eq!(portal_prefer_dark(&1_u32.to_variant(), false), Some(true));
        assert_eq!(portal_prefer_dark(&2_u32.to_variant(), true), Some(false));
        assert_eq!(portal_prefer_dark(&0_u32.to_variant(), true), Some(true));
        assert_eq!(portal_prefer_dark(&99_u32.to_variant(), false), Some(false));
        assert_eq!(portal_prefer_dark(&"dark".to_variant(), false), None);
    }

    /// Decodes a Settings portal ReadOne tuple before projecting its preference value.
    ///
    /// # Panics
    /// Panics if a correctly typed portal tuple cannot select the dark preference.
    #[test]
    fn portal_read_reply_preserves_nested_variant_type() {
        let portal_reply = (1_u32.to_variant(),).to_variant();
        let preference = portal_reply
            .get::<(glib::Variant,)>()
            .and_then(|(value,)| portal_prefer_dark(&value, false));
        assert_eq!(preference, Some(true));
    }
}
