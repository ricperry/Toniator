//! Application information and support links; no document or library state is edited here.

use gtk::prelude::*;

/// Presents application identity, artist guidance, support links, and the distributed license.
///
/// Links open only on activation through GTK's URI handler. Closing this modal window leaves
/// the parent document untouched. A missing icon decode simply omits the decorative image.
pub fn present(parent: &gtk::Window) {
    let window = gtk::Window::builder()
        .title("About Toniator")
        .transient_for(parent)
        .modal(true)
        .default_width(560)
        .default_height(660)
        .build();
    window.set_titlebar(Some(&gtk::HeaderBar::new()));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    if let Ok(texture) = gtk::gdk::Texture::from_bytes(&gtk::glib::Bytes::from_static(
        include_bytes!("../../../assets/appicon.png"),
    )) {
        let image = gtk::Picture::for_paintable(&texture);
        image.set_size_request(72, 72);
        image.set_content_fit(gtk::ContentFit::Contain);
        image.set_halign(gtk::Align::Center);
        content.append(&image);
    }
    let title = text("Toniator", false);
    title.add_css_class("title-1");
    content.append(&title);
    content.append(&text(
        &format!("Version {} · Linux · Prerelease", env!("CARGO_PKG_VERSION")),
        false,
    ));
    content.append(&text("Turn your artwork into halftones made from dots, lines, curves, and shapes. Export PNG images or editable SVG artwork.", false));
    content.append(&text(
        "Created by Richard Perry · GPL-3.0-only · No warranty",
        false,
    ));
    content.append(&text("Get started", true));
    content.append(&text("Open artwork, choose a Pattern, then adjust its size and appearance. A Pattern describes the shapes and their arrangement. A Preset saves your design settings; a project also keeps your source artwork.", false));
    for (label, uri) in [
        (
            "User guide and examples",
            "https://github.com/ricperry/Toniator#make-your-first-artwork",
        ),
        (
            "Report a problem or request a feature",
            "https://github.com/ricperry/Toniator/issues",
        ),
        (
            "Source code and downloads",
            "https://github.com/ricperry/Toniator",
        ),
    ] {
        let link = gtk::LinkButton::with_label(uri, label);
        link.set_halign(gtk::Align::Start);
        content.append(&link);
    }
    content.append(&text("When reporting a problem, include the version above, your Linux distribution, how you installed Toniator, and the steps that caused it. Attach sample artwork only if you are comfortable sharing it publicly.", false));
    content.append(&text("Credits and license", true));
    content.append(&text("Created by Richard Perry. Free software under the GNU General Public License, version 3 only (GPL-3.0-only). Distributed without warranty.", false));
    let license = gtk::Expander::builder()
        .label("Read the GNU GPL version 3")
        .build();
    license.update_property(&[
        gtk::accessible::Property::Label("Read the GNU GPL version 3"),
        gtk::accessible::Property::Description("Expand to read the complete distributed license."),
    ]);
    let license_text = text(include_str!("../../../LICENSE"), false);
    license_text.set_selectable(true);
    let license_scroll = gtk::ScrolledWindow::builder()
        .min_content_height(180)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&license_text)
        .build();
    license.set_child(Some(&license_scroll));
    content.append(&license);
    let notices = gtk::LinkButton::with_label(
        "https://github.com/ricperry/Toniator/blob/main/packaging/MEDIA-NOTICES.md",
        "Bundled media tools and license notices",
    );
    notices.set_halign(gtk::Align::Start);
    content.append(&notices);
    let close = gtk::Button::with_label("Close");
    close.set_halign(gtk::Align::End);
    let weak = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(window) = weak.upgrade() {
            window.close();
        }
    });
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&content)
        .build();
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout.append(&scroll);
    close.set_margin_top(12);
    close.set_margin_bottom(12);
    close.set_margin_end(24);
    layout.append(&close);
    window.set_child(Some(&layout));
    let keys = gtk::EventControllerKey::new();
    let weak = window.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(window) = weak.upgrade() {
                window.close();
            }
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);
    window.present();
}

/// Creates a wrapping information label; headings are visual presentation only.
fn text(value: &str, heading: bool) -> gtk::Label {
    let label = gtk::Label::new(Some(value));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    if heading {
        label.add_css_class("heading");
    }
    label
}
