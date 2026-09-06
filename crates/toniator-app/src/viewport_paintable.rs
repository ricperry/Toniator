//! Immutable texture presentation with a document-sized intrinsic extent.

use gtk::{gdk, glib, prelude::*, subclass::prelude::*};
use std::cell::OnceCell;

/// Stores presentation coordinates only; canonical geometry and pixel bytes remain unchanged.
struct Image {
    texture: gdk::Texture,
    width: f64,
    height: f64,
    bounds: (f32, f32, f32, f32),
}

mod imp {
    use super::*;

    /// Implements a fixed-size, fixed-content texture paintable without retaining a GTK snapshot.
    #[derive(Default)]
    pub struct ViewportPaintable {
        pub(super) image: OnceCell<Image>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ViewportPaintable {
        const NAME: &'static str = "ToniatorViewportPaintable";
        type Type = super::ViewportPaintable;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for ViewportPaintable {}

    impl gdk::subclass::prelude::PaintableImpl for ViewportPaintable {
        /// Returns this immutable image without creating another render-node snapshot.
        fn current_image(&self) -> gdk::Paintable {
            self.obj().clone().upcast()
        }

        /// Declares that construction fixes both dimensions and content for this image.
        fn flags(&self) -> gdk::PaintableFlags {
            gdk::PaintableFlags::STATIC_SIZE | gdk::PaintableFlags::STATIC_CONTENTS
        }

        /// Reports the document-sized preferred width, independently of proxy resolution.
        fn intrinsic_width(&self) -> i32 {
            self.image
                .get()
                .map_or(0, |image| image.width.round() as i32)
        }

        /// Reports the document-sized preferred height, independently of proxy resolution.
        fn intrinsic_height(&self) -> i32 {
            self.image
                .get()
                .map_or(0, |image| image.height.round() as i32)
        }

        /// Preserves the preferred canvas aspect through GTK's contain layout.
        fn intrinsic_aspect_ratio(&self) -> f64 {
            self.image
                .get()
                .map_or(0.0, |image| image.width / image.height)
        }

        /// Appends the texture at paint time, clipping only the requested viewport rectangle.
        ///
        /// GDK supplies positive dimensions. No offscreen snapshot is retained across GTK frame
        /// updates, and scaling removes existing preview padding without altering source pixels.
        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            let Some(image) = self.image.get() else {
                return;
            };
            let Some(snapshot) = snapshot.downcast_ref::<gtk::Snapshot>() else {
                return;
            };
            let scale_x = (width / image.width) as f32;
            let scale_y = (height / image.height) as f32;
            let (x, y, w, h) = image.bounds;
            snapshot.push_clip(&gtk::graphene::Rect::new(
                0.0,
                0.0,
                width as f32,
                height as f32,
            ));
            snapshot.append_texture(
                &image.texture,
                &gtk::graphene::Rect::new(x * scale_x, y * scale_y, w * scale_x, h * scale_y),
            );
            snapshot.pop();
        }
    }
}

glib::wrapper! {
    /// Presents one immutable texture at canvas size through GDK's native paintable interface.
    pub struct ViewportPaintable(ObjectSubclass<imp::ViewportPaintable>) @implements gdk::Paintable;
}

impl ViewportPaintable {
    /// Captures positive canvas dimensions and one clipped texture rectangle for GTK presentation.
    pub(super) fn new(
        texture: gdk::Texture,
        width: f64,
        height: f64,
        bounds: (f32, f32, f32, f32),
    ) -> Self {
        let object: Self = glib::Object::new();
        let _ = object.imp().image.set(Image {
            texture,
            width,
            height,
            bounds,
        });
        object
    }
}
