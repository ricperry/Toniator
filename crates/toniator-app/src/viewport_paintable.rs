//! Fixed-canvas texture presentation with explicitly invalidated viewport-dependent tile content.

use gtk::{gdk, glib, prelude::*, subclass::prelude::*};
use std::cell::OnceCell;

/// Stores presentation coordinates only; canonical geometry and pixel bytes remain unchanged.
struct Image {
    texture: gdk::Texture,
    tiles: Option<crate::preview_cache::Presentation>,
    bounds: (f32, f32, f32, f32),
    fallback: Option<(gdk::Texture, (f32, f32, f32, f32))>,
    width: f64,
    height: f64,
}

mod imp {
    use super::*;

    /// Implements fixed intrinsic geometry with explicitly invalidated viewport-dependent tile content.
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
        /// Freezes the current tiled draw into an immutable render-node paintable for GDK consumers.
        /// Ordinary texture images are already immutable and may return themselves. No GTK snapshot
        /// retained here observes later scroll/layout/generation changes.
        fn current_image(&self) -> gdk::Paintable {
            let Some(image) = self.image.get() else {
                return gdk::Paintable::new_empty(0, 0);
            };
            if image.tiles.is_none() {
                return self.obj().clone().upcast();
            }
            let snapshot = gtk::Snapshot::new();
            self.obj().snapshot(&snapshot, image.width, image.height);
            snapshot
                .to_paintable(Some(&gtk::graphene::Size::new(
                    image.width as f32,
                    image.height as f32,
                )))
                .unwrap_or_else(|| {
                    gdk::Paintable::new_empty(self.intrinsic_width(), self.intrinsic_height())
                })
        }

        /// Declares immutable size; viewport-dependent tiled content changes only with invalidation.
        fn flags(&self) -> gdk::PaintableFlags {
            if self.image.get().is_some_and(|image| image.tiles.is_some()) {
                gdk::PaintableFlags::STATIC_SIZE
            } else {
                gdk::PaintableFlags::STATIC_SIZE | gdk::PaintableFlags::STATIC_CONTENTS
            }
        }

        /// Reports the document-sized preferred width, independently of proxy resolution.
        fn intrinsic_width(&self) -> i32 {
            self.image
                .get()
                .map_or(0, |image| image.width.round().max(1.0) as i32)
        }

        /// Reports the document-sized preferred height, independently of proxy resolution.
        fn intrinsic_height(&self) -> i32 {
            self.image
                .get()
                .map_or(0, |image| image.height.round().max(1.0) as i32)
        }

        /// Preserves the preferred canvas aspect through GTK's contain layout.
        fn intrinsic_aspect_ratio(&self) -> f64 {
            self.image
                .get()
                .map_or(0.0, |image| image.width / image.height)
        }

        /// Appends certified current-translation tiles and disjoint full-canvas fallback at paint time.
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
            if let Some(tiles) = &image.tiles {
                let (fallback, bounds) = image
                    .fallback
                    .as_ref()
                    .map(|(t, b)| (t, *b))
                    .unwrap_or((&image.texture, image.bounds));
                if tiles.snapshot(snapshot, width, height, fallback, bounds) {
                    return;
                }
            }
            let scale_x = (width / image.width) as f32;
            let scale_y = (height / image.height) as f32;
            snapshot.push_clip(&gtk::graphene::Rect::new(
                0.0,
                0.0,
                width as f32,
                height as f32,
            ));
            let (x, y, w, h) = image.bounds;
            if let Some((texture, (fx, fy, fw, fh))) = &image.fallback {
                let left = x.clamp(0.0, image.width as f32);
                let right = (x + w).clamp(left, image.width as f32);
                let top = y.clamp(0.0, image.height as f32);
                let bottom = (y + h).clamp(top, image.height as f32);
                for (tx, ty, tw, th) in [
                    (0.0, 0.0, left, image.height as f32),
                    (right, 0.0, image.width as f32 - right, image.height as f32),
                    (left, 0.0, right - left, top),
                    (left, bottom, right - left, image.height as f32 - bottom),
                ] {
                    if tw <= 0.0 || th <= 0.0 {
                        continue;
                    }
                    snapshot.push_clip(&gtk::graphene::Rect::new(
                        tx * scale_x,
                        ty * scale_y,
                        tw * scale_x,
                        th * scale_y,
                    ));
                    snapshot.append_texture(
                        texture,
                        &gtk::graphene::Rect::new(
                            *fx * scale_x,
                            *fy * scale_y,
                            *fw * scale_x,
                            *fh * scale_y,
                        ),
                    );
                    snapshot.pop();
                }
            }
            snapshot.append_texture(
                &image.texture,
                &gtk::graphene::Rect::new(x * scale_x, y * scale_y, w * scale_x, h * scale_y),
            );
            snapshot.pop();
        }
    }
}

glib::wrapper! {
    /// Presents immutable pixel sources at canvas size through GDK's native paintable interface.
    /// Tiled content explicitly invalidates when live viewport placement changes.
    pub struct ViewportPaintable(ObjectSubclass<imp::ViewportPaintable>) @implements gdk::Paintable;
}

impl ViewportPaintable {
    /// Captures engine-certified sparse textures while retaining the ordinary image for holes.
    pub(super) fn with_tiles(
        texture: gdk::Texture,
        width: f64,
        height: f64,
        bounds: (f32, f32, f32, f32),
        fallback: Option<(gdk::Texture, (f32, f32, f32, f32))>,
        tiles: crate::preview_cache::Presentation,
    ) -> Self {
        let object: Self = glib::Object::new();
        let _ = object.imp().image.set(Image {
            texture,
            tiles: Some(tiles),
            bounds,
            fallback,
            width,
            height,
        });
        object
    }
    /// Captures positive canvas dimensions and one clipped texture rectangle for GTK presentation.
    pub(super) fn new(
        texture: gdk::Texture,
        width: f64,
        height: f64,
        bounds: (f32, f32, f32, f32),
    ) -> Self {
        Self::with_fallback(texture, width, height, bounds, None)
    }

    /// Captures disjoint fallback tiles and a fresh crop without blending their pixel coverage.
    pub(super) fn with_fallback(
        texture: gdk::Texture,
        width: f64,
        height: f64,
        bounds: (f32, f32, f32, f32),
        fallback: Option<(gdk::Texture, (f32, f32, f32, f32))>,
    ) -> Self {
        let object: Self = glib::Object::new();
        let _ = object.imp().image.set(Image {
            texture,
            tiles: None,
            bounds,
            fallback,
            width,
            height,
        });
        object
    }
}
