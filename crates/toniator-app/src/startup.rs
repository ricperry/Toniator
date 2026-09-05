//! Native startup presentation; document loading and recent-file persistence stay with controllers.

use gtk::prelude::*;
use std::path::Path;
use toniator_io::recent::RecentFile;

/// Owns startup widgets without retaining document, file-write, or evaluator authority.
pub(crate) struct StartupScreen {
    pub root: gtk::ScrolledWindow,
    pub columns: gtk::Box,
    pub start: gtk::Button,
    pub clear: gtk::Button,
    pub status: gtk::Label,
    pub recent: gtk::Box,
}

impl StartupScreen {
    /// Builds a responsive native projection of SplashMockup.png, with an unchanged banner source.
    /// The banner clips only the artwork region; all text and controls are real theme-aware widgets.
    ///
    /// # Panics
    /// Panics if the build-time registered splash reference resource cannot be decoded.
    pub fn new() -> Self {
        let root = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.add_css_class("toniator-startup");
        let texture =
            gtk::gdk::Texture::from_resource("/com/silentbutdigital/Toniator/splash-reference.png");
        let snapshot = gtk::Snapshot::new();
        let banner_height = 366.0;
        snapshot.push_clip(&gtk::graphene::Rect::new(
            0.0,
            0.0,
            texture.width() as f32,
            banner_height,
        ));
        snapshot.append_texture(
            &texture,
            &gtk::graphene::Rect::new(0.0, 0.0, texture.width() as f32, texture.height() as f32),
        );
        snapshot.pop();
        let paintable = snapshot.to_paintable(Some(&gtk::graphene::Size::new(
            texture.width() as f32,
            banner_height,
        )));
        let picture = gtk::Picture::new();
        picture.set_paintable(paintable.as_ref());
        picture.set_can_shrink(true);
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.update_property(&[gtk::accessible::Property::Label(
            "Toniator halftone artwork",
        )]);
        let banner = gtk::AspectFrame::builder()
            .ratio(texture.width() as f32 / banner_height)
            .obey_child(false)
            .child(&picture)
            .build();
        content.append(&banner);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 14);
        body.set_margin_top(20);
        body.set_margin_bottom(18);
        body.set_margin_start(22);
        body.set_margin_end(22);
        let columns = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        columns.set_homogeneous(true);
        let about = card();
        let title = gtk::Label::new(Some("About Toniator"));
        title.add_css_class("title-2");
        about.append(&title);
        let description = gtk::Label::new(Some(
            "Toniator is a halftone and pattern design tool for creating stylized CMYK and RGB artwork from source images.",
        ));
        description.set_wrap(true);
        description.set_xalign(0.0);
        description.set_max_width_chars(46);
        description.add_css_class("toniator-startup-description");
        about.append(&description);
        let start = gtk::Button::with_label("Start New Project");
        start.set_action_name(Some("app.open"));
        start.add_css_class("suggested-action");
        start.add_css_class("toniator-startup-primary");
        start.set_tooltip_text(Some(
            "Choose a source image or open an existing .toniator project.",
        ));
        about.append(&start);
        let hint = gtk::Label::new(Some(
            "Choose a source image to begin, or use this button to open an existing project file.",
        ));
        hint.set_wrap(true);
        hint.set_max_width_chars(46);
        hint.add_css_class("dim-label");
        about.append(&hint);
        columns.append(&about);

        let recent_card = card();
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let recent_title = gtk::Label::new(Some("Recent Files"));
        recent_title.add_css_class("title-2");
        recent_title.set_xalign(0.0);
        recent_title.set_hexpand(true);
        let clear = gtk::Button::with_label("Clear List");
        clear.set_tooltip_text(Some(
            "Clear this history list. Your files stay where they are.",
        ));
        heading.append(&recent_title);
        heading.append(&clear);
        recent_card.append(&heading);
        let recent = gtk::Box::new(gtk::Orientation::Vertical, 0);
        recent.update_property(&[gtk::accessible::Property::Label("Recent Files")]);
        recent_card.append(&recent);
        columns.append(&recent_card);
        body.append(&columns);
        let status = gtk::Label::new(None);
        status.set_wrap(true);
        status.set_visible(false);
        status.update_property(&[gtk::accessible::Property::Label("Startup status")]);
        body.append(&status);
        let tip = gtk::Label::new(Some(
            "Tip: Use CMYK mode for print workflows and RGB for digital displays.",
        ));
        tip.set_wrap(true);
        tip.add_css_class("dim-label");
        body.append(&tip);
        content.append(&body);
        root.set_child(Some(&content));
        Self {
            root,
            columns,
            start,
            clear,
            status,
            recent,
        }
    }

    /// Rebuilds native recent-file buttons from immutable IO metadata; opening stays callback-owned.
    /// Full paths distinguish equal basenames in the UI, tooltip, and accessible description.
    pub fn populate(&self, entries: &[RecentFile], on_open: impl Fn(&Path) + Clone + 'static) {
        while let Some(child) = self.recent.first_child() {
            self.recent.remove(&child);
        }
        if entries.is_empty() {
            let empty = gtk::Label::new(Some(
                "No recent files yet. Open artwork or a project to add it here.",
            ));
            empty.set_wrap(true);
            empty.set_max_width_chars(42);
            empty.set_margin_top(24);
            empty.set_margin_bottom(24);
            empty.add_css_class("dim-label");
            self.recent.append(&empty);
        }
        for entry in entries {
            let name = entry
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let button = gtk::Button::new();
            button.add_css_class("toniator-recent-file");
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            let icon = gtk::Image::from_icon_name(
                if entry
                    .path
                    .extension()
                    .is_some_and(|value| value.eq_ignore_ascii_case("toniator"))
                {
                    "document-open-symbolic"
                } else {
                    "image-x-generic-symbolic"
                },
            );
            row.append(&icon);
            let labels = gtk::Box::new(gtk::Orientation::Vertical, 3);
            labels.set_hexpand(true);
            let title = gtk::Label::new(Some(&name));
            title.set_xalign(0.0);
            title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            title.set_max_width_chars(36);
            labels.append(&title);
            let folder = gtk::Label::new(Some(
                &entry
                    .path
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .to_string_lossy(),
            ));
            folder.set_xalign(0.0);
            folder.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            folder.set_max_width_chars(36);
            folder.add_css_class("caption");
            folder.add_css_class("dim-label");
            labels.append(&folder);
            row.append(&labels);
            let date = i64::try_from(entry.used_at)
                .ok()
                .and_then(|seconds| glib::DateTime::from_unix_local(seconds).ok())
                .and_then(|date| date.format("%b %e, %H:%M").ok())
                .map(|date| date.to_string())
                .unwrap_or_default();
            let timestamp = gtk::Label::new(Some(&date));
            timestamp.add_css_class("caption");
            timestamp.add_css_class("dim-label");
            row.append(&timestamp);
            button.set_child(Some(&row));
            button.set_tooltip_text(Some(&entry.path.to_string_lossy()));
            button.update_property(&[
                gtk::accessible::Property::Label(&format!("Open {name}")),
                gtk::accessible::Property::Description(&entry.path.to_string_lossy()),
            ]);
            let path = entry.path.clone();
            let open = on_open.clone();
            button.connect_clicked(move |_| open(&path));
            self.recent.append(&button);
        }
    }

    /// Displays a nonfatal startup operation/history message without altering recent entries.
    pub fn set_status(&self, text: &str) {
        self.status.set_label(text);
        self.status.set_visible(!text.is_empty());
    }
}

/// Creates one theme-derived startup card with native spacing and no fixed dark-mode colors.
fn card() -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 18);
    card.set_hexpand(true);
    card.set_valign(gtk::Align::Fill);
    card.add_css_class("toniator-startup-card");
    card
}
