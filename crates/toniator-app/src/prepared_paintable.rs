//! Exact prepared-raster presentation with nearest-neighbor inspection zoom.

use std::cell::{Cell, OnceCell};

use gtk::{gdk, glib, graphene, gsk, prelude::*, subclass::prelude::ObjectSubclassIsExt};

mod imp {
    use super::*;
    use gtk::gdk::subclass::prelude::*;

    /// Immutable texture and per-projection sampling choice for one GTK picture.
    #[derive(Default)]
    pub struct PreparedPaintable {
        pub(super) texture: OnceCell<gdk::Texture>,
        pub(super) nearest: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreparedPaintable {
        const NAME: &'static str = "ToniatorPreparedPaintable";
        type Type = super::PreparedPaintable;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for PreparedPaintable {}

    impl PaintableImpl for PreparedPaintable {
        /// Returns the same immutable final raster for GTK image consumers.
        fn current_image(&self) -> gdk::Paintable {
            self.texture
                .get()
                .expect("constructed texture")
                .clone()
                .upcast()
        }

        /// Reports exact source dimensions independent of requested display zoom.
        fn intrinsic_width(&self) -> i32 {
            self.texture.get().expect("constructed texture").width()
        }

        /// Reports exact source dimensions independent of requested display zoom.
        fn intrinsic_height(&self) -> i32 {
            self.texture.get().expect("constructed texture").height()
        }

        /// Lets GTK cache this immutable texture projection safely.
        fn flags(&self) -> gdk::PaintableFlags {
            gdk::PaintableFlags::STATIC_SIZE | gdk::PaintableFlags::STATIC_CONTENTS
        }

        /// Filters enlarged output pixels as discrete blocks at 100% and above.
        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            let snapshot = snapshot
                .downcast_ref::<gtk::Snapshot>()
                .expect("GTK picture snapshot");
            let filter = if self.nearest.get() {
                gsk::ScalingFilter::Nearest
            } else {
                gsk::ScalingFilter::Linear
            };
            snapshot.append_scaled_texture(
                self.texture.get().expect("constructed texture"),
                filter,
                &graphene::Rect::new(0.0, 0.0, width as f32, height as f32),
            );
        }
    }
}

glib::wrapper! {
    /// Paintable wrapper for the exact prepared PNG texture and display-only filter.
    pub struct PreparedPaintable(ObjectSubclass<imp::PreparedPaintable>) @implements gdk::Paintable;
}

impl PreparedPaintable {
    /// Constructs an immutable texture view; no output pixels are modified.
    pub(crate) fn new(texture: gdk::Texture, nearest: bool) -> Self {
        let paintable: Self = glib::Object::new();
        paintable
            .imp()
            .texture
            .set(texture)
            .expect("new paintable texture set once");
        paintable.imp().nearest.set(nearest);
        paintable
    }
}
