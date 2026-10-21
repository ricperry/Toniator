//! GTK upload/projection only. Engine owns accepted-scene/root/coverage/in-flight/LRU authority.
use gtk::{gdk, glib, prelude::*};
use std::{
    collections::HashMap,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
};
use toniator_engine::preview_cache::{Frame, Region};
use toniator_engine::{
    EvaluationLimits, PreviewRasterTarget, RasterizationLimits, RenderScene,
    preview_cache::{self as cache, Artwork, Plan, PreviewCache, RootKey, View},
};

/// Enables owned diagnostic snapshot observations explicitly; normal presentation performs no trace I/O.
fn trace_enabled() -> bool {
    static TRACE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *TRACE
        .get_or_init(|| std::env::var_os("TONIATOR_PREVIEW_CACHE_TRACE").is_some_and(|v| v == "1"))
}

/// Exact GTK4.22.5 Picture contain/ceil/centering and its verified pure-translation viewport frame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Layout {
    pub origin: [f64; 2],
    pub snapshot: [f64; 2],
    pub preferred: [f64; 2],
    pub page: [f64; 2],
    pub device: i32,
    pub canvas: [f64; 2],
    pub scale: f64,
    pub anchor: [f64; 2],
    pub anchor_screen: [f32; 2],
    surface_origin: [f32; 2],
}
/// Outward float32 arithmetic for presentation/sampler support, never artwork geometry.
#[derive(Clone, Copy)]
struct I(f64, f64);
impl I {
    /// Captures one already-rounded finite float32 operand exactly.
    fn point(v: f32) -> Option<Self> {
        v.is_finite().then_some(Self(f64::from(v), f64::from(v)))
    }
    /// Encloses one actual float32 binary result, including ties/subnormal rounding.
    fn rounded(lo: f64, hi: f64) -> Option<Self> {
        let a = lo as f32;
        let b = hi as f32;
        (a.is_finite() && b.is_finite())
            .then(|| Self(f64::from(a.next_down()), f64::from(b.next_up())))
    }
    /// Encloses subtraction without algebraic reassociation.
    fn sub(self, b: Self) -> Option<Self> {
        Self::rounded((self.0 - b.1).next_down(), (self.1 - b.0).next_up())
    }
    /// Encloses addition without algebraic reassociation.
    fn add(self, b: Self) -> Option<Self> {
        Self::rounded((self.0 + b.0).next_down(), (self.1 + b.1).next_up())
    }
    /// Encloses a positive-denominator division; unsupported/nonfinite frames decline caching.
    fn div(self, b: Self) -> Option<Self> {
        if b.0 <= 0. {
            return None;
        }
        let v = [self.0 / b.0, self.0 / b.1, self.1 / b.0, self.1 / b.1];
        Self::rounded(
            v.into_iter().fold(f64::INFINITY, f64::min).next_down(),
            v.into_iter().fold(f64::NEG_INFINITY, f64::max).next_up(),
        )
    }
    /// Encloses multiplication of concrete sampler operands without assuming exact GPU FMA choices.
    fn mul(self, b: Self) -> Option<Self> {
        let v = [self.0 * b.0, self.0 * b.1, self.1 * b.0, self.1 * b.1];
        Self::rounded(
            v.into_iter().fold(f64::INFINITY, f64::min).next_down(),
            v.into_iter().fold(f64::NEG_INFINITY, f64::max).next_up(),
        )
    }
}
impl Layout {
    /// Resolves actual installed GTK contain rounding while retaining the existing uniform root scale.
    /// Only pure-translation Picture frames and physical scale1/2 are admitted; others use ordinary rendering.
    pub fn new(
        canvas: [f64; 2],
        preferred: [f64; 2],
        picture: [f64; 2],
        origin: [f64; 2],
        page: [f64; 2],
        device: i32,
        scale: f64,
    ) -> Option<Self> {
        if ![1, 2].contains(&device)
            || canvas
                .into_iter()
                .chain(preferred)
                .chain(picture)
                .chain(page)
                .any(|v| !v.is_finite() || v <= 0.)
            || origin.iter().any(|v| !v.is_finite())
            || !scale.is_finite()
            || scale <= 0.
        {
            return None;
        }
        let ratio = preferred[0] / preferred[1];
        let (w, h) = if ratio > picture[0] / picture[1] {
            (picture[0], picture[0] / ratio)
        } else {
            (picture[1] * ratio, picture[1])
        };
        let snapshot = [w.ceil(), h.ceil()];
        let t = [
            ((picture[0] - snapshot[0]) / 2.).trunc(),
            ((picture[1] - snapshot[1]).floor() / 2.).trunc(),
        ];
        let origin = [
            f64::from((origin[0] as f32) + (t[0] as f32)),
            f64::from((origin[1] as f32) + (t[1] as f32)),
        ];
        let factor = [
            snapshot[0] / (canvas[0] * scale),
            snapshot[1] / (canvas[1] * scale),
        ];
        let anchor = [
            ((page[0] * 0.5 - origin[0]) / factor[0]).floor(),
            ((page[1] * 0.5 - origin[1]) / factor[1]).floor(),
        ];
        let anchor_screen = [
            (origin[0] + anchor[0] * factor[0]) as f32,
            (origin[1] + anchor[1] * factor[1]) as f32,
        ];
        if origin
            .into_iter()
            .chain(snapshot)
            .chain(anchor)
            .chain(factor)
            .any(|v| !v.is_finite())
            || anchor_screen.iter().any(|v| !v.is_finite())
            || page.iter().any(|v| *v * f64::from(device) > 16_777_216.)
        {
            return None;
        }
        Some(Self {
            origin,
            snapshot,
            preferred,
            page,
            device,
            canvas,
            scale,
            anchor,
            anchor_screen,
            surface_origin: [0., 0.],
        })
    }
    /// Admits only an actual viewport ancestor translation on the physical device-pixel grid.
    /// The existing uniform artistic transform is unchanged; unsupported GTK ancestors use fallback.
    pub fn with_viewport(mut self, scroll: &gtk::ScrolledWindow) -> Option<Self> {
        let window = scroll.root()?.downcast::<gtk::Window>().ok()?;
        let m = scroll.compute_transform(&window)?.to_2d()?;
        if [m.0, m.1, m.2, m.3] != [1., 0., 0., 1.] {
            return None;
        }
        let p = scroll.compute_point(&window, &gtk::graphene::Point::new(0., 0.))?;
        let physical = [p.x() * self.device as f32, p.y() * self.device as f32];
        if physical
            .iter()
            .any(|v| !v.is_finite() || v.fract() != 0. || v.abs() > 16_777_216.)
        {
            return None;
        }
        self.surface_origin = physical;
        Some(self)
    }
    /// Reprojects the same GTK frame using an existing source root's uniform raster scale.
    /// Only presentation anchors change; source pixels and the artistic/evaluation transform do not.
    fn at_scale(&self, scale: f64) -> Option<Self> {
        if !scale.is_finite() || scale <= 0. {
            return None;
        }
        let mut layout = self.clone();
        layout.scale = scale;
        for axis in 0..2 {
            let factor = self.snapshot[axis] / (self.canvas[axis] * scale);
            layout.anchor[axis] = ((self.page[axis] * 0.5 - self.origin[axis]) / factor).floor();
            layout.anchor_screen[axis] = (self.origin[axis] + layout.anchor[axis] * factor) as f32;
            if !layout.anchor[axis].is_finite() || !layout.anchor_screen[axis].is_finite() {
                return None;
            }
        }
        Some(layout)
    }
    /// Maps a root boundary once into viewport-local float32 placement; common boundaries agree.
    fn edge(&self, axis: usize, root: f64) -> f64 {
        f64::from(self.anchor_screen[axis])
            + f64::from(
                ((root - self.anchor[axis]) * self.snapshot[axis]
                    / (self.canvas[axis] * self.scale)) as f32,
            )
    }
    /// Partitions adjacent interiors on shared physical-device-pixel edges, avoiding clip-AA overlap.
    pub fn clip(&self, region: Region) -> Option<Region> {
        let d = f64::from(self.device);
        let left = if region.x == 0 {
            (self.edge(0, 0.) * d).floor()
        } else {
            (self.edge(0, f64::from(region.x)) * d).round()
        }
        .max(0.)
        .min((self.page[0] * d).ceil());
        let top = if region.y == 0 {
            (self.edge(1, 0.) * d).floor()
        } else {
            (self.edge(1, f64::from(region.y)) * d).round()
        }
        .max(0.)
        .min((self.page[1] * d).ceil());
        let edge = self.edge(0, f64::from(region.x) + f64::from(region.width)) * d;
        let right = if region.x + region.width >= (self.canvas[0] * self.scale).ceil() as u32 {
            edge.ceil()
        } else {
            edge.round()
        }
        .max(0.)
        .min((self.page[0] * d).ceil());
        let edge = self.edge(1, f64::from(region.y) + f64::from(region.height)) * d;
        let bottom = if region.y + region.height >= (self.canvas[1] * self.scale).ceil() as u32 {
            edge.ceil()
        } else {
            edge.round()
        }
        .max(0.)
        .min((self.page[1] * d).ceil());
        (right > left && bottom > top).then_some(Region {
            x: left as u32,
            y: top as u32,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        })
    }
    /// Returns the concrete local padded texture rectangle consumed by the GTK snapshot.
    pub fn texture_rect(&self, region: Region) -> [f32; 4] {
        let ax = self.snapshot[0] / (self.canvas[0] * self.scale);
        let ay = self.snapshot[1] / (self.canvas[1] * self.scale);
        [
            ((f64::from(region.x) - self.anchor[0]) * ax) as f32,
            ((f64::from(region.y) - self.anchor[1]) * ay) as f32,
            (f64::from(region.width) * ax) as f32,
            (f64::from(region.height) * ay) as f32,
        ]
    }
    /// Converts placed logical coordinates into exact physical units for device1/2 GSK nodes.
    /// Binary doubling is exact for finite representable results, including subnormals; the inverse
    /// restores the original operand. Overflow or an unsupported device declines presentation.
    fn physical_rect(&self, rect: [f32; 4]) -> Option<gtk::graphene::Rect> {
        if ![1, 2].contains(&self.device) {
            return None;
        }
        let d = self.device as f32;
        let scaled = rect.map(|v| v * d);
        if rect
            .iter()
            .zip(scaled)
            .any(|(original, physical)| !physical.is_finite() || physical / d != *original)
            || scaled[2] <= 0.
            || scaled[3] <= 0.
        {
            return None;
        }
        Some(gtk::graphene::Rect::new(
            scaled[0], scaled[1], scaled[2], scaled[3],
        ))
    }
    /// Encloses the actual four-neighbor fetch from concrete physical TextureScaleNode operands.
    /// Offscreen interiors return None. Canvas edges clamp to the original fixed root frame.
    fn tile_support(&self, frame: Frame, interior: Region, padded: Region) -> Option<Region> {
        let clip = self.clip(interior)?;
        let rect = self.texture_rect(padded);
        let physical = self.physical_rect([
            self.anchor_screen[0] + rect[0],
            self.anchor_screen[1] + rect[1],
            rect[2],
            rect[3],
        ])?;
        let mut bounds = [0.; 4];
        for a in 0..2 {
            let (start, len, pixel_start, pixel_len, origin, extent) = if a == 0 {
                (
                    clip.x,
                    clip.width,
                    padded.x,
                    padded.width,
                    physical.x(),
                    physical.width(),
                )
            } else {
                (
                    clip.y,
                    clip.height,
                    padded.y,
                    padded.height,
                    physical.y(),
                    physical.height(),
                )
            };
            let screen = I(
                f64::from(start) + 0.5,
                f64::from(start) + f64::from(len) - 0.5,
            );
            let surface = I::point(self.surface_origin[a])?;
            let texel = screen
                .add(surface)?
                .sub(I::point(origin)?.add(surface)?)?
                .div(I::point(extent)?)?
                .mul(I::point(pixel_len as f32)?)?;
            bounds[a] = (texel.0 - 0.5).floor() + f64::from(pixel_start);
            bounds[a + 2] = (texel.1 - 0.5).floor() + 2. + f64::from(pixel_start);
        }
        let x = bounds[0].max(0.).min(f64::from(frame.width)) as u32;
        let y = bounds[1].max(0.).min(f64::from(frame.height)) as u32;
        let right = bounds[2].max(0.).min(f64::from(frame.width)) as u32;
        let bottom = bounds[3].max(0.).min(f64::from(frame.height)) as u32;
        (right > x && bottom > y).then_some(Region {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }
    /// Pulls actual physical-node fetches into one outward query plus copied-gutter preparation space.
    /// # Errors
    /// Unsupported/nonfinite operands decline caching instead of guessing an artwork halo.
    pub fn support(&self, frame: Frame, expected: &[Region]) -> Option<Region> {
        if frame.width > 16_777_216 || frame.height > 16_777_216 {
            return None;
        }
        let region = if expected.is_empty() {
            let mut b = [0.; 4];
            for a in 0..2 {
                let factor = self.snapshot[a] / (self.canvas[a] * self.scale);
                b[a] = ((-self.origin[a]) / factor).floor() - 1.;
                b[a + 2] = ((self.page[a] - self.origin[a]) / factor).ceil() + 1.;
            }
            if !b.iter().all(|v| v.is_finite()) {
                return None;
            }
            let x = b[0].max(0.).min(f64::from(frame.width)) as u32;
            let y = b[1].max(0.).min(f64::from(frame.height)) as u32;
            let right = b[2].max(0.).min(f64::from(frame.width)) as u32;
            let bottom = b[3].max(0.).min(f64::from(frame.height)) as u32;
            if right <= x || bottom <= y {
                return None;
            }
            Region {
                x,
                y,
                width: right - x,
                height: bottom - y,
            }
        } else {
            let mut combined = None;
            for interior in expected {
                if self.clip(*interior).is_none() {
                    continue;
                }
                let fetched =
                    self.tile_support(frame, *interior, cache::gutter(*interior, frame))?;
                combined = Some(combined.map_or(fetched, |old| union(old, fetched)));
            }
            combined?
        };
        Some(cache::gutter(region, frame))
    }
}
/// One upload with padded neighboring texels and a disjoint logical root interior.
#[derive(Clone)]
pub(crate) struct Tile {
    pub texture: gdk::Texture,
    pub interior: Region,
    pub padded: Region,
}
/// Immutable GTK projection permission, rechecked lock-free at snapshot time.
#[derive(Clone)]
pub(crate) struct Presentation {
    pub view: Arc<View>,
    pub layout: Layout,
    pub tiles: Vec<Tile>,
    pub alternatives: Vec<Presentation>,
    pub rejected: Arc<AtomicBool>,
    pub geometry: Option<(
        glib::WeakRef<gtk::Picture>,
        glib::WeakRef<gtk::ScrolledWindow>,
    )>,
    pub observed: Arc<AtomicBool>,
}
struct Upload {
    tile: Tile,
    cost: usize,
}
/// Owns only GTK uploads/projection and one serialized checked-out engine-cache worker.
pub(crate) struct Controller {
    shared: Arc<(Mutex<PreviewCache>, Condvar)>,
    shutdown: Arc<AtomicBool>,
    changed: Arc<AtomicU64>,
    seen: u64,
    thread: Option<thread::JoinHandle<()>>,
    uploads: HashMap<u64, Upload>,
    pub presentation: Option<Presentation>,
    pub pending: bool,
    pub exact_complete: bool,
    pub ordinary_required: bool,
    rejected: Arc<AtomicBool>,
    last_request: Option<(Artwork, PreviewRasterTarget, Layout)>,
    fallbacks: HashMap<u64, gdk::Texture>,
}
impl Controller {
    /// Starts one bounded worker; spawn failure leaves the ordinary renderer available.
    pub fn new() -> Self {
        let shared = Arc::new((Mutex::new(PreviewCache::default()), Condvar::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let changed = Arc::new(AtomicU64::new(0));
        let (s, stop, version) = (shared.clone(), shutdown.clone(), changed.clone());
        let thread = thread::Builder::new()
            .name("preview-cache".into())
            .spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let job = {
                        let (m, c) = &*s;
                        let mut cache = m.lock().unwrap_or_else(|p| p.into_inner());
                        while !cache.queued() && !stop.load(Ordering::Relaxed) {
                            cache = c.wait(cache).unwrap_or_else(|p| p.into_inner());
                        }
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        cache.take_job()
                    };
                    // A lazy cursor can drain only failed/already stored cells without producing
                    // a completion. Wake once before the next condvar wait so terminal padding gaps
                    // leave refinement and reach the ordinary renderer instead of lingering forever.
                    if job.is_none() {
                        version.fetch_add(1, Ordering::Release);
                    }
                    if let Some(job) = job {
                        let result = job.run(&stop);
                        let mut cache = s.0.lock().unwrap_or_else(|p| p.into_inner());
                        let visible = result.changes_view(&cache);
                        cache.complete(result);
                        drop(cache);
                        if visible {
                            version.fetch_add(1, Ordering::Release);
                        }
                    }
                }
            })
            .ok();
        Self {
            shared,
            shutdown,
            changed,
            seen: 0,
            thread,
            uploads: HashMap::new(),
            presentation: None,
            pending: false,
            exact_complete: false,
            ordinary_required: false,
            rejected: Arc::new(AtomicBool::new(false)),
            last_request: None,
            fallbacks: HashMap::new(),
        }
    }
    /// Drops derived frame data and revokes running work while keeping the worker itself reusable.
    pub fn clear(&mut self) {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        self.uploads.clear();
        self.presentation = None;
        self.pending = false;
        self.last_request = None;
        self.fallbacks.clear();
    }
    /// Hands only a scheduler-accepted scene Arc to engine ownership, accounting canonical storage.
    pub fn accept(&mut self, artwork: Artwork, scene: Arc<RenderScene>) {
        let estimate = cache::estimate_scene(&scene).unwrap_or(usize::MAX);
        let mut cache = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        if !cache.has_scene(artwork, &scene) {
            self.uploads.clear();
            self.fallbacks.clear();
            self.presentation = None;
        }
        cache.accept_scene(artwork, scene, estimate);
    }
    /// Reports current certified drawable cache pixels without treating alpha as coverage.
    /// Background refinement and ordinary exact-target replacement keep these useful pixels visible;
    /// stale root/scene generations never suppress initial evaluation progress.
    pub fn has_current_pixels(&self) -> bool {
        self.presentation.as_ref().is_some_and(|p| {
            std::iter::once(p)
                .chain(p.alternatives.iter())
                .any(|layer| {
                    layer.view.is_current()
                        && layer
                            .tiles
                            .iter()
                            .any(|tile| layer.layout.clip(tile.interior).is_some())
                })
        })
    }

    /// Reads live stored/full visited-zoom pixels under the short metadata lock, without root scans.
    pub fn background_progress(&self) -> (u64, u64, bool) {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .background_progress()
    }

    /// Returns actual worker/coverage counters for private logs, never user progress arithmetic.
    pub fn statistics(&self) -> cache::Statistics {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .statistics()
    }
    /// Reconciles GTK residency with authoritative engine IDs after every eviction-capable lifecycle.
    /// Refreshing conservative texture charges may evict further unused entries; each iteration removes
    /// ownership or reaches stable IDs. Current visible/inflight engine regions remain pinned.
    fn sync_residency(&mut self) {
        if self
            .presentation
            .as_ref()
            .is_some_and(|p| !p.view.is_current())
        {
            self.presentation = None;
        }
        let mut cache = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            let resident = cache.resident_ids();
            let exact = cache.exact_ids();
            let before = (self.uploads.len(), self.fallbacks.len());
            self.uploads.retain(|id, _| resident.contains(id));
            self.fallbacks.retain(|id, _| exact.contains(id));
            cache.textures(
                self.uploads
                    .values()
                    .fold(0usize, |total, u| total.saturating_add(u.cost)),
                0,
            );
            if before == (self.uploads.len(), self.fallbacks.len())
                && resident == cache.resident_ids()
                && exact == cache.exact_ids()
            {
                break;
            }
        }
    }
    /// Captures weak GTK layout observers solely to reject stale viewport transforms at draw time.
    pub fn bind_geometry(&mut self, picture: &gtk::Picture, scroll: &gtk::ScrolledWindow) {
        if let Some(p) = &mut self.presentation {
            p.geometry = Some((picture.downgrade(), scroll.downgrade()));
        }
    }
    /// Polls serialized completion without blocking GTK on preparation or restoration.
    pub fn poll(&mut self) -> bool {
        if self.thread.as_ref().is_some_and(|t| t.is_finished()) {
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
            self.clear();
            return true;
        }
        let version = self.changed.load(Ordering::Acquire);
        let rejected = self.rejected.swap(false, Ordering::AcqRel);
        if rejected {
            // Allocation/zoom/device/page snapshots are transient. Replan the current frame rather
            // than poisoning a previously valid exact-scale target with old layout assumptions.
            self.presentation = None;
            self.pending = false;
        }
        if version == self.seen && !rejected {
            return false;
        }
        self.seen = version;
        self.sync_residency();
        true
    }
    /// Retains secondary exact-target ordinary pixels without relabeling them as fixed-grid coverage.
    pub fn remember_fallback(
        &mut self,
        artwork: Artwork,
        target: PreviewRasterTarget,
        texture: gdk::Texture,
    ) {
        let mut cache = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        let bytes = (texture.width() as usize)
            .saturating_mul(texture.height() as usize)
            .saturating_mul(12);
        if let Some(id) = cache.remember_exact(artwork, target, bytes) {
            self.fallbacks.insert(id, texture);
        }
        let ids = cache.exact_ids();
        self.fallbacks.retain(|id, _| ids.contains(id));
        drop(cache);
        self.sync_residency();
    }
    /// Reuses a resident ordinary exact-target upload, governed by the engine-owned LRU.
    pub fn fallback(
        &mut self,
        artwork: Artwork,
        target: PreviewRasterTarget,
    ) -> Option<gdk::Texture> {
        self.sync_residency();
        let mut cache = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        let id = cache.exact(artwork, target)?;
        self.fallbacks.get(&id).cloned()
    }
    /// Rebinds derived ownership only after a successful domain None-invalidation transition.
    pub fn rebind_unchanged(&mut self, old: Artwork, new: Artwork) -> bool {
        let rebound = self
            .shared
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .rebind_unchanged(old, new);
        if rebound
            && let Some((artwork, _, _)) = self.last_request.as_mut()
            && *artwork == old
        {
            *artwork = new;
        }
        rebound
    }
    /// Invalidates unobserved authority transitions; no unchanged-byte inference can rebind a revision.
    pub fn require_artwork(&mut self, artwork: Artwork) {
        let matches = self
            .shared
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .has_artwork(artwork);
        if !matches {
            self.clear();
        }
    }
    /// Suspends foreground projection in Source mode while visited zooms continue background filling.
    pub fn suspend(&mut self) {
        self.pending = false;
        self.presentation = None;
        self.shared
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pin_views(&[]);
    }
    /// Retires only current spatial projection/pending state before ordinary-renderer fallback.
    /// Valid roots, CPU pixels, uploads and exact-target memos remain reusable; background demand continues.
    pub fn decline(&mut self) {
        self.ordinary_required = false;
        self.suspend();
        self.presentation = None;
        self.last_request = None;
        self.sync_residency();
    }
    /// Keeps a visited zoom filling autonomously when an ordinary exact-target memo already displays it.
    /// This schedules no evaluation/source work and performs no certificate scan on GTK.
    pub fn visit_zoom(&mut self, artwork: Artwork, target: PreviewRasterTarget) {
        self.presentation = None;
        let mut cache = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        cache.pin_views(&[]);
        if let Some((_, _, scale)) = target.viewport() {
            let limits =
                RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())
                    .expect("Main limits");
            let _ = cache.visit_zoom(artwork, RootKey::new(scale, target.background(), limits));
            self.shared.1.notify_one();
        }
    }
    /// Plans resident reuse or only genuinely missing texels; root/retry work remains off GTK.
    /// Returns false only when the existing viewport renderer must handle this exact query.
    pub fn request(
        &mut self,
        artwork: Artwork,
        target: PreviewRasterTarget,
        layout: Layout,
    ) -> bool {
        self.ordinary_required = false;
        let Some((_, _, scale)) = target.viewport() else {
            self.decline();
            return false;
        };
        let limits =
            RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())
                .expect("Main nonzero limits");
        let key = RootKey::new(scale, target.background(), limits);
        let candidates = self
            .shared
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .candidate_scales(key);
        self.presentation = None;
        let handled = self.request_inner(artwork, target, layout.clone(), true);
        let mut presentations = self.presentation.take().into_iter().collect::<Vec<_>>();
        self.exact_complete = presentations.first().is_some_and(|p| p.visible_complete());
        if !self.exact_complete {
            for scale in candidates {
                let Some(candidate) = layout.at_scale(scale) else {
                    continue;
                };
                self.presentation = None;
                self.request_inner(artwork, target, candidate, false);
                if let Some(presentation) = self.presentation.take() {
                    presentations.push(presentation);
                }
                if covered_viewport(&presentations, &layout) {
                    break;
                }
            }
        }
        retain_drawable(&mut presentations);
        if let Some(mut first) = presentations.into_iter().reduce(|mut first, next| {
            first.alternatives.push(next);
            first
        }) {
            first.geometry = None;
            self.presentation = Some(first);
        }
        self.last_request = Some((artwork, target, layout));
        let pinned = self
            .presentation
            .as_ref()
            .map(|p| {
                std::iter::once(p)
                    .chain(p.alternatives.iter())
                    .map(|layer| layer.view.key)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.shared
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pin_views(&pinned);
        // Accepted-artwork background refinement never raises the foreground evaluation overlay.
        self.pending = false;
        if !handled && self.presentation.is_none() {
            self.decline();
            return false;
        }
        self.shared.1.notify_one();
        self.sync_residency();
        self.ordinary_required = !handled && self.presentation.is_some();
        if trace_enabled() {
            let sources=self.presentation.as_ref().map(|p|std::iter::once(p).chain(p.alternatives.iter())
                .map(|layer|serde_json::json!({"scale":layer.view.frame.scale,"generation":layer.view.generation,
                    "tiles":layer.tiles.len(),"certificate":[layer.view.certificate.region().x,layer.view.certificate.region().y,
                        layer.view.certificate.region().width,layer.view.certificate.region().height]})).collect::<Vec<_>>()).unwrap_or_default();
            eprintln!(
                "preview-cache-projection {}",
                serde_json::json!({"requested_scale":scale,"sources":sources,
                "exact_visible_complete":self.exact_complete,"ordinary_required":self.ordinary_required,"foreground_pending":self.pending})
            );
        }
        handled
    }
    /// Attempts the complete current query; the outer request retires stale state on every decline.
    fn request_inner(
        &mut self,
        artwork: Artwork,
        target: PreviewRasterTarget,
        layout: Layout,
        foreground: bool,
    ) -> bool {
        if self.thread.is_none() {
            self.presentation = None;
            self.pending = false;
            return false;
        }
        let scale = layout.scale;
        let background = target.background();
        let frame = Frame {
            width: (layout.canvas[0] * scale).ceil() as u32,
            height: (layout.canvas[1] * scale).ceil() as u32,
            scale,
            background,
        };
        let Some(mut region) = layout.support(frame, &[]) else {
            return false;
        };
        let limits =
            RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())
                .expect("Main nonzero limits");
        let key = RootKey::new(scale, background, limits);
        let mut cache = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        let mut accepted = None;
        for _ in 0..4 {
            match if foreground {
                cache.plan(artwork, key, region)
            } else {
                cache.plan_cached(artwork, key, region)
            } {
                Ok(Plan::Ready(view)) => {
                    let Some(required) = layout.support(frame, &view.expected) else {
                        break;
                    };
                    let covered = cache::missing(required, [region]).is_empty();
                    if covered {
                        accepted = Some(view);
                        break;
                    }
                    region = union(region, required);
                }
                Ok(Plan::Waiting) => {
                    self.pending = true;
                    self.presentation = None;
                    self.last_request = Some((artwork, target, layout));
                    self.shared.1.notify_one();
                    return true;
                }
                _ => break,
            }
        }
        let Some(mut view) = accepted else {
            cache.suspend();
            self.pending = false;
            self.presentation = None;
            return false;
        };
        cache.pin_view(key);
        let exact = cache.exact_ids();
        self.fallbacks.retain(|id, _| exact.contains(id));
        let resident = cache.resident_ids();
        self.uploads.retain(|id, _| resident.contains(id));
        let mut tiles = Vec::new();
        let mut waiting_upload = false;
        let mut copied = 0u64;
        for pixel in &view.pixels {
            let mut interior = pixel.region;
            let mut padded = cache::gutter(interior, frame);
            if !cache::missing(padded, view.pixels.iter().map(|p| p.region)).is_empty() {
                let available =
                    |r| cache::missing(r, view.pixels.iter().map(|p| p.region)).is_empty();
                let left = u32::from(
                    interior.x > 0
                        && !available(Region {
                            x: interior.x - 1,
                            y: interior.y,
                            width: 1,
                            height: interior.height,
                        }),
                );
                let top = u32::from(
                    interior.y > 0
                        && !available(Region {
                            x: interior.x,
                            y: interior.y - 1,
                            width: interior.width,
                            height: 1,
                        }),
                );
                let right = u32::from(
                    interior.x + interior.width < frame.width
                        && !available(Region {
                            x: interior.x + interior.width,
                            y: interior.y,
                            width: 1,
                            height: interior.height,
                        }),
                );
                let bottom = u32::from(
                    interior.y + interior.height < frame.height
                        && !available(Region {
                            x: interior.x,
                            y: interior.y + interior.height,
                            width: interior.width,
                            height: 1,
                        }),
                );
                if interior.width <= left + right || interior.height <= top + bottom {
                    continue;
                }
                interior = Region {
                    x: interior.x + left,
                    y: interior.y + top,
                    width: interior.width - left - right,
                    height: interior.height - top - bottom,
                };
                padded = cache::gutter(interior, frame);
                if !available(padded) {
                    continue;
                }
            }
            // An existing wider valid upload may keep its copied neighbor pixels after a pan.
            if let Some(upload) = self.uploads.get(&pixel.id)
                && cache::missing(interior, [upload.tile.interior]).is_empty()
            {
                interior = upload.tile.interior;
                padded = upload.tile.padded;
            }
            if cache::intersection(interior, region).is_none() {
                continue;
            }
            let fresh = self
                .uploads
                .get(&pixel.id)
                .is_none_or(|u| u.tile.interior != interior);
            if fresh {
                let bytes = match cache.upload(key, view.generation, pixel.id, padded) {
                    Ok(Some(bytes)) => bytes,
                    Ok(None) => {
                        waiting_upload = true;
                        continue;
                    }
                    Err(_) => return false,
                };
                copied += bytes.len() as u64;
                let cost = bytes.len().saturating_mul(3);
                let texture = gdk::MemoryTexture::new(
                    padded.width as i32,
                    padded.height as i32,
                    gdk::MemoryFormat::R8g8b8a8,
                    &glib::Bytes::from_owned(bytes),
                    padded.width as usize * 4,
                )
                .upcast();
                self.uploads.insert(
                    pixel.id,
                    Upload {
                        tile: Tile {
                            texture,
                            interior,
                            padded,
                        },
                        cost,
                    },
                );
            }
            if let Some(u) = self.uploads.get(&pixel.id) {
                tiles.push(u.tile.clone());
            }
        }
        cache.textures(
            self.uploads
                .values()
                .fold(0usize, |total, u| total.saturating_add(u.cost)),
            copied,
        );
        let mut actual_support = None;
        for tile in &tiles {
            if layout.clip(tile.interior).is_none() {
                continue;
            }
            let Some(fetched) = layout.tile_support(frame, tile.interior, tile.padded) else {
                return false;
            };
            if !cache::missing(fetched, [tile.padded]).is_empty() {
                return false;
            }
            actual_support = Some(actual_support.map_or(fetched, |old| union(old, fetched)));
        }
        if let Some(required) = actual_support
            && !view.certificate.covers(required)
        {
            match if foreground {
                cache.plan(artwork, key, union(region, required))
            } else {
                cache.plan_cached(artwork, key, union(region, required))
            } {
                Ok(Plan::Ready(updated))
                    if updated.generation == view.generation
                        && updated.certificate.covers(required) =>
                {
                    view = updated;
                }
                Ok(Plan::Waiting) => {
                    self.pending = true;
                    self.presentation = None;
                    self.last_request = Some((artwork, target, layout));
                    self.shared.1.notify_one();
                    return true;
                }
                _ => return false,
            }
        }
        let visible = layout.clip(Region {
            x: 0,
            y: 0,
            width: frame.width,
            height: frame.height,
        });
        let visible_complete = visible.is_none_or(|visible| {
            cache::missing(
                visible,
                tiles.iter().filter_map(|t| layout.clip(t.interior)),
            )
            .is_empty()
        });
        if view.complete && !visible_complete && !waiting_upload && !view.background_pending {
            return false;
        }
        self.pending = !visible_complete;
        self.presentation = Some(Presentation {
            view: Arc::new(view),
            layout: layout.clone(),
            tiles,
            alternatives: Vec::new(),
            rejected: self.rejected.clone(),
            geometry: None,
            observed: Arc::new(AtomicBool::new(false)),
        });
        self.last_request = Some((artwork, target, layout));
        self.shared.1.notify_one();
        true
    }
}
impl Drop for Controller {
    /// Cancels owned work and joins only this controller's worker at application teardown.
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        self.shared
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        self.shared.1.notify_one();
        if let Some(worker) = self.thread.take() {
            let _ = worker.join();
        }
    }
}
/// Unions whole-view demand, never certificates from independently admitted tiles.
fn union(a: Region, b: Region) -> Region {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    Region {
        x,
        y,
        width: (a.x + a.width).max(b.x + b.width) - x,
        height: (a.y + a.height).max(b.y + b.height) - y,
    }
}
/// Retains only current sources contributing physical pixels after higher-priority coverage.
/// Empty/occluded sources cannot pin roots or prevent valid later sources from drawing.
fn retain_drawable(layers: &mut Vec<Presentation>) {
    let mut covered = Vec::new();
    layers.retain(|layer| {
        if !layer.view.is_current() {
            return false;
        }
        let clips = layer
            .tiles
            .iter()
            .filter_map(|t| layer.layout.clip(t.interior))
            .collect::<Vec<_>>();
        if !clips
            .iter()
            .any(|clip| !cache::missing(*clip, covered.iter().copied()).is_empty())
        {
            return false;
        }
        covered.extend(clips);
        true
    });
}
/// Tests alpha-independent physical coverage across nearest-first cached source scales.
fn covered_viewport(layers: &[Presentation], layout: &Layout) -> bool {
    let frame = Frame {
        width: (layout.canvas[0] * layout.scale).ceil() as u32,
        height: (layout.canvas[1] * layout.scale).ceil() as u32,
        scale: layout.scale,
        background: toniator_engine::RasterBackground::Transparent,
    };
    layout
        .clip(Region {
            x: 0,
            y: 0,
            width: frame.width,
            height: frame.height,
        })
        .is_none_or(|visible| {
            cache::missing(
                visible,
                layers
                    .iter()
                    .flat_map(|p| p.tiles.iter().filter_map(|t| p.layout.clip(t.interior))),
            )
            .is_empty()
        })
}
impl Presentation {
    /// Reports visible exact-source coverage independently of whole-zoom background completion.
    fn visible_complete(&self) -> bool {
        covered_viewport(std::slice::from_ref(self), &self.layout)
    }

    /// Rejects unsupported live draw authority and requests one controller replan without rendering.
    /// Owned diagnostics distinguish draw rejection from a worker/planning result; ordinary fallback
    /// remains available and no numerical permission is manufactured here.
    fn reject_draw(&self, reason: &str) -> bool {
        let first = !self.rejected.swap(true, Ordering::AcqRel);
        if first && trace_enabled() {
            eprintln!("preview-cache-draw-rejected {reason}");
        }
        false
    }

    /// Projects cached textures at the actual PAINT-phase translation using one containing source
    /// certificate per layer. GTK UPDATE may precede scroll allocation, so captured origin alone is
    /// never current draw authority. This performs bounded placement/containment checks, no root lock,
    /// certificate scan, geometry work or pixel copies; unsupported frames retain ordinary fallback.
    pub fn snapshot(
        &self,
        snapshot: &gtk::Snapshot,
        width: f64,
        height: f64,
        fallback: &gdk::Texture,
        fallback_bounds: (f32, f32, f32, f32),
    ) -> bool {
        if !self.view.is_current() {
            return self.reject_draw("root-generation");
        }
        let mut projected = self.clone();
        if let Some((picture, scroll)) = &self.geometry {
            let (Some(picture), Some(scroll)) = (picture.upgrade(), scroll.upgrade()) else {
                return self.reject_draw("widget-lifetime");
            };
            if scroll.scale_factor() != self.layout.device
                || [
                    scroll.hadjustment().page_size(),
                    scroll.vadjustment().page_size(),
                ] != self.layout.page
            {
                return self.reject_draw("device-or-page");
            }
            let Some(origin) = picture.compute_point(&scroll, &gtk::graphene::Point::new(0., 0.))
            else {
                return self.reject_draw("origin-unavailable");
            };
            let Some(matrix) = picture.compute_transform(&scroll).and_then(|m| m.to_2d()) else {
                return self.reject_draw("matrix-unavailable");
            };
            if [matrix.0, matrix.1, matrix.2, matrix.3] != [1., 0., 0., 1.] {
                return self.reject_draw("nontranslation");
            }
            let Some(layout) = Layout::new(
                self.layout.canvas,
                self.layout.preferred,
                [f64::from(picture.width()), f64::from(picture.height())],
                [f64::from(origin.x()), f64::from(origin.y())],
                self.layout.page,
                self.layout.device,
                self.layout.scale,
            )
            .and_then(|l| l.with_viewport(&scroll)) else {
                return self.reject_draw("unsupported-frame");
            };
            if layout.surface_origin != self.layout.surface_origin {
                return self.reject_draw("surface-origin");
            }
            projected.layout = layout;
        }
        if [width, height] != projected.layout.snapshot {
            return self.reject_draw("snapshot-size");
        }
        let current = projected.layout.clone();
        let mut alternatives = std::mem::take(&mut projected.alternatives);
        for layer in std::iter::once(&mut projected).chain(alternatives.iter_mut()) {
            // Every source needs permission for the WHOLE current sampled view, including its holes.
            let Some(layout) = current.at_scale(layer.view.frame.scale) else {
                return self.reject_draw("source-frame");
            };
            layer.layout = layout;
            let Some(mut required) = layer.layout.support(layer.view.frame, &[]) else {
                layer.tiles.clear();
                continue;
            };
            layer.tiles.retain(|tile| {
                if layer.layout.clip(tile.interior).is_none() {
                    return true;
                }
                let Some(fetch) =
                    layer
                        .layout
                        .tile_support(layer.view.frame, tile.interior, tile.padded)
                else {
                    self.reject_draw("sampler-frame");
                    return false;
                };
                if !cache::missing(fetch, [tile.padded]).is_empty() {
                    self.reject_draw("upload-support");
                    return false;
                }
                required = union(required, fetch);
                true
            });
            if !layer.view.is_current() || !layer.view.certificate.covers(required) {
                layer.tiles.clear();
                self.reject_draw("source-certificate");
            }
        }
        projected.alternatives = alternatives;
        if trace_enabled() {
            eprintln!(
                "preview-cache-draw {}",
                serde_json::json!({
                    "origin":projected.layout.origin,"snapshot":[width,height],
                    "sources":std::iter::once(&projected).chain(projected.alternatives.iter())
                        .filter(|layer|layer.view.is_current()).map(|layer|serde_json::json!({"scale":layer.view.frame.scale,
                        "visible_tiles":layer.tiles.iter().filter(|tile|layer.layout.clip(tile.interior).is_some()).count()})).collect::<Vec<_>>()
                })
            );
        }
        projected.snapshot_projected(snapshot, width, height, fallback, fallback_bounds)
    }

    /// Appends disjoint certified viewport nodes plus ordinary fallback throughout the full canvas.
    /// Physical viewport edges are shared integer boundaries; transparent cached pixels own coverage.
    fn snapshot_projected(
        &self,
        snapshot: &gtk::Snapshot,
        width: f64,
        height: f64,
        fallback: &gdk::Texture,
        fallback_bounds: (f32, f32, f32, f32),
    ) -> bool {
        if trace_enabled() && !self.observed.swap(true, Ordering::Relaxed) {
            eprintln!(
                "preview-cache-snapshot {}",
                serde_json::json!({"actual_snapshot":[width,height],"expected_snapshot":self.layout.snapshot,"paintable_origin":self.layout.origin,"device":self.layout.device,"root_generation":self.view.generation,"tiles":self.tiles.len(),"pure_translation_verified":self.geometry.is_some()})
            );
        }
        let d = self.layout.device as f32;
        let layers = std::iter::once(self)
            .chain(self.alternatives.iter())
            .filter(|p| p.view.is_current())
            .collect::<Vec<_>>();
        let mut clips = Vec::new();
        let mut drawn = Vec::new();
        for layer in layers {
            for tile in &layer.tiles {
                if let Some(clip) = layer.layout.clip(tile.interior) {
                    for uncovered in cache::missing(clip, clips.iter().copied()) {
                        clips.push(uncovered);
                        drawn.push((layer, tile, uncovered));
                    }
                }
            }
        }
        let whole = Region {
            x: 0,
            y: 0,
            width: (self.layout.page[0] * f64::from(self.layout.device)).ceil() as u32,
            height: (self.layout.page[1] * f64::from(self.layout.device)).ceil() as u32,
        };
        snapshot.push_clip(&gtk::graphene::Rect::new(
            0.,
            0.,
            width as f32,
            height as f32,
        ));
        snapshot.save();
        snapshot.translate(&gtk::graphene::Point::new(
            -(self.layout.origin[0] as f32),
            -(self.layout.origin[1] as f32),
        ));
        let sx = width / self.layout.preferred[0];
        let sy = height / self.layout.preferred[1];
        let fallback_rect = gtk::graphene::Rect::new(
            (self.layout.origin[0] + f64::from(fallback_bounds.0) * sx) as f32,
            (self.layout.origin[1] + f64::from(fallback_bounds.1) * sy) as f32,
            (f64::from(fallback_bounds.2) * sx) as f32,
            (f64::from(fallback_bounds.3) * sy) as f32,
        );
        let holes = cache::missing(whole, clips.iter().copied());
        let mut nodes = Vec::<gtk::gsk::RenderNode>::new();
        if nodes
            .try_reserve(holes.len().saturating_add(drawn.len()).saturating_add(4))
            .is_err()
        {
            snapshot.restore();
            snapshot.pop();
            self.rejected.store(true, Ordering::Release);
            return false;
        }
        let Some(canvas) = self.layout.physical_rect([
            self.layout.origin[0] as f32,
            self.layout.origin[1] as f32,
            width as f32,
            height as f32,
        ]) else {
            snapshot.restore();
            snapshot.pop();
            return self.reject_draw("canvas-fallback-frame");
        };
        let Some(fallback_physical) = self.layout.physical_rect([
            fallback_rect.x(),
            fallback_rect.y(),
            fallback_rect.width(),
            fallback_rect.height(),
        ]) else {
            snapshot.restore();
            snapshot.pop();
            return self.reject_draw("fallback-frame");
        };
        let left = canvas.x();
        let right = canvas.x() + canvas.width();
        let top = canvas.y();
        let bottom = canvas.y() + canvas.height();
        let x0 = 0f32.max(left).min(right);
        let x1 = (whole.width as f32).max(x0).min(right);
        let y0 = 0f32.max(top).min(bottom);
        let y1 = (whole.height as f32).max(y0).min(bottom);
        // These four strips are OUTSIDE the captured viewport, never under transparent tile nodes.
        // A retained node therefore still covers newly exposed canvas until explicit invalidation.
        for (x, y, w, h) in [
            (left, top, x0 - left, canvas.height()),
            (x1, top, right - x1, canvas.height()),
            (x0, top, x1 - x0, y0 - top),
            (x0, y1, x1 - x0, bottom - y1),
        ] {
            if w > 0. && h > 0. {
                let texture = gtk::gsk::TextureScaleNode::new(
                    fallback,
                    &fallback_physical,
                    gtk::gsk::ScalingFilter::Linear,
                );
                nodes.push(
                    gtk::gsk::ClipNode::new(texture, &gtk::graphene::Rect::new(x, y, w, h))
                        .upcast(),
                );
            }
        }
        for hole in holes {
            let Some(bounds) = self.layout.physical_rect([
                fallback_rect.x(),
                fallback_rect.y(),
                fallback_rect.width(),
                fallback_rect.height(),
            ]) else {
                snapshot.restore();
                snapshot.pop();
                self.rejected.store(true, Ordering::Release);
                return false;
            };
            let texture =
                gtk::gsk::TextureScaleNode::new(fallback, &bounds, gtk::gsk::ScalingFilter::Linear);
            nodes.push(
                gtk::gsk::ClipNode::new(
                    texture,
                    &gtk::graphene::Rect::new(
                        hole.x as f32,
                        hole.y as f32,
                        hole.width as f32,
                        hole.height as f32,
                    ),
                )
                .upcast(),
            );
        }
        for (layer, tile, clip) in drawn {
            let rect = layer.layout.texture_rect(tile.padded);
            let Some(bounds) = layer.layout.physical_rect([
                layer.layout.anchor_screen[0] + rect[0],
                layer.layout.anchor_screen[1] + rect[1],
                rect[2],
                rect[3],
            ]) else {
                snapshot.restore();
                snapshot.pop();
                self.rejected.store(true, Ordering::Release);
                return false;
            };
            let texture = gtk::gsk::TextureScaleNode::new(
                &tile.texture,
                &bounds,
                gtk::gsk::ScalingFilter::Linear,
            );
            nodes.push(
                gtk::gsk::ClipNode::new(
                    texture,
                    &gtk::graphene::Rect::new(
                        clip.x as f32,
                        clip.y as f32,
                        clip.width as f32,
                        clip.height as f32,
                    ),
                )
                .upcast(),
            );
        }
        // Keep explicit-linear TextureScaleNode at effective device scale1: GTK4.22.5 otherwise
        // resamples each node through a logical-scale1 offscreen before the device-scale pass.
        // Device1/2 and its inverse are exact powers of two; this changes units, not placement.
        if !nodes.is_empty() {
            let transform = gtk::gsk::Transform::new().scale(1. / d, 1. / d);
            snapshot.append_node(gtk::gsk::TransformNode::new(
                gtk::gsk::ContainerNode::new(&nodes),
                Some(&transform),
            ));
        }
        snapshot.restore();
        snapshot.pop();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Verifies installed Picture contain rounding and per-axis presentation without changing root scale.
    /// # Panics
    /// Panics if ideal contain is incorrectly substituted for actual GTK ceil/centering.
    #[test]
    fn picture_ceil_and_float_fetch_support_include_hidpi_and_edges() {
        for device in [1, 2] {
            let layout = Layout::new(
                [900., 620.],
                [297., 204.6],
                [297., 205.],
                [-40.25, -12.5],
                [240., 170.],
                device,
                0.33 * f64::from(device),
            )
            .unwrap();
            assert_eq!(layout.snapshot, [297., 205.]);
            assert_eq!(layout.scale, 0.33 * f64::from(device));
            let frame = Frame {
                width: (900. * layout.scale).ceil() as u32,
                height: (620. * layout.scale).ceil() as u32,
                scale: layout.scale,
                background: toniator_engine::RasterBackground::Transparent,
            };
            let initial = layout.support(frame, &[]).unwrap();
            let support = layout.support(frame, &[initial]).unwrap();
            assert!(support.width > 0 && support.height > 0);
            assert!(support.x + support.width <= frame.width);
            assert!(support.y + support.height <= frame.height);
            let a = Region {
                x: 0,
                y: 0,
                width: 128,
                height: 128,
            };
            let b = Region { x: 128, ..a };
            if let (Some(a), Some(b)) = (layout.clip(a), layout.clip(b)) {
                assert_eq!(a.x + a.width, b.x);
            }
        }
    }
    /// Exercises exact/subnormal float operands and unsupported representation fallback.
    /// # Panics
    /// Panics if finite operations leave their enclosures or invalid layouts silently gain permission.
    #[test]
    fn presentation_intervals_and_unsupported_frames_fail_closed() {
        for (a, b) in [
            (1f32, 0.1f32),
            (-10000.25, 1e-30),
            (f32::from_bits(1), f32::from_bits(2)),
        ] {
            let actual = a - b;
            let bound = I::point(a).unwrap().sub(I::point(b).unwrap()).unwrap();
            assert!(f64::from(actual) >= bound.0 && f64::from(actual) <= bound.1);
        }
        assert!(Layout::new([1., 1.], [1., 1.], [1., 1.], [0., 0.], [1., 1.], 3, 1.).is_none());
        assert!(I::point(f32::INFINITY).is_none());
        assert!(I::point(1.).unwrap().div(I(-1., 1.)).is_none());
    }
}

#[cfg(test)]
mod gtk_pixel_tests {
    use super::*;
    use toniator_domain::{CanvasSpec, ChannelId, ColorValue};
    use toniator_engine::{GeometryOutput, RenderLayer};
    /// Creates a constant straight-alpha native texture; zero-alpha hidden RGB is retained at upload.
    fn texture(width: u32, height: u32, rgba: [u8; 4]) -> gdk::Texture {
        let pixels = rgba.repeat((width * height) as usize);
        gdk::MemoryTexture::new(
            width as i32,
            height as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(pixels),
            width as usize * 4,
        )
        .upcast()
    }
    /// Downloads the renderer's native straight-RGBA bytes including alpha, preserving row stride.
    fn download(texture: &gdk::Texture) -> Vec<u8> {
        let mut downloader = gdk::TextureDownloader::new(texture);
        downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
        let (bytes, stride) = downloader.download_bytes();
        (0..texture.height() as usize)
            .flat_map(|y| {
                bytes[y * stride..y * stride + texture.width() as usize * 4]
                    .iter()
                    .copied()
            })
            .collect()
    }
    /// Copies one padded rectangle from a shared native RGBA image, including neighbor gutters.
    /// # Panics
    /// Panics if this bounded test rectangle exceeds its admitted complete image.
    fn copied_texture(region: Region, width: u32, rgba: &[u8]) -> gdk::Texture {
        let mut bytes = Vec::new();
        for y in region.y..region.y + region.height {
            let start = (y as usize * width as usize + region.x as usize) * 4;
            bytes.extend_from_slice(&rgba[start..start + region.width as usize * 4]);
        }
        gdk::MemoryTexture::new(
            region.width as i32,
            region.height as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(bytes),
            region.width as usize * 4,
        )
        .upcast()
    }
    /// Initializes only an explicitly owned GTK test window and services its identity-gate main context.
    /// # Panics
    /// Panics on missing owned-session authority, GTK registration failure or a timed-out gate.
    fn owned_window() -> (gtk::Application, gtk::ApplicationWindow) {
        assert_eq!(
            std::env::var("TONIATOR_CACHE_C2_GTK_TEST").as_deref(),
            Ok("owned-private-session")
        );
        glib::set_prgname(Some("Toniator"));
        glib::set_application_name("Toniator");
        gtk::init().unwrap();
        let application = gtk::Application::builder()
            .application_id("org.toniator.CachePixelProof")
            .build();
        application.register(None::<&gio::Cancellable>).unwrap();
        let window = gtk::ApplicationWindow::builder()
            .application(&application)
            .default_width(100)
            .default_height(80)
            .build();
        window.set_title(Some("Toniator"));
        window.present();
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        if let Some(gate) = std::env::var_os("TONIATOR_CACHE_C2_PIXEL_GATE") {
            let gate = std::path::PathBuf::from(gate);
            let started = std::time::Instant::now();
            while !gate.exists() {
                assert!(
                    started.elapsed().as_secs() < 60,
                    "owned identity gate timed out"
                );
                while glib::MainContext::default().pending() {
                    glib::MainContext::default().iteration(false);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        (application, window)
    }
    /// Exercises GDK invalidation and actual retained Picture/ScrolledWindow nodes across scrolling.
    /// Cached transparent pixels own coverage; missing uploaded support uses the supplied real full
    /// green fallback, including canvas outside the old viewport patch. No controller job runs in draw.
    /// # Panics
    /// Panics on static-content invalidation, mutable current_image, missing fallback or alpha leakage.
    fn scrolling_picture_preserves_retained_fallback(
        controller: &mut Controller,
        window: &gtk::ApplicationWindow,
        renderer: &gtk::gsk::Renderer,
        out: &std::path::Path,
    ) {
        let green = gdk::MemoryTexture::new(
            1,
            1,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(vec![0u8, 255, 0, 255]),
            4,
        )
        .upcast::<gdk::Texture>();
        let picture = gtk::Picture::new();
        picture.set_size_request(3096, 3096);
        picture.set_can_shrink(false);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&picture));
        window.set_child(Some(&scroll));
        let ordinary = crate::viewport_paintable::ViewportPaintable::new(
            green.clone(),
            3096.,
            3096.,
            (0., 0., 3096., 3096.),
        );
        let ordinary_signals = std::rc::Rc::new(std::cell::Cell::new(0));
        let n = ordinary_signals.clone();
        ordinary.connect_invalidate_contents(move |_| n.set(n.get() + 1));
        picture.set_paintable(Some(&ordinary));
        crate::invalidate_main_view_content(&picture);
        assert_eq!(
            ordinary_signals.get(),
            0,
            "ordinary/source immutable images never invalidate"
        );
        let settle = || {
            for _ in 0..30 {
                while glib::MainContext::default().pending() {
                    glib::MainContext::default().iteration(false);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        settle();
        let point = picture
            .compute_point(&scroll, &gtk::graphene::Point::new(0., 0.))
            .unwrap();
        let page = [
            scroll.hadjustment().page_size(),
            scroll.vadjustment().page_size(),
        ];
        let device = scroll.scale_factor();
        let layout = Layout::new(
            [3096., 3096.],
            [3096., 3096.],
            [f64::from(picture.width()), f64::from(picture.height())],
            [f64::from(point.x()), f64::from(point.y())],
            page,
            device,
            1.,
        )
        .unwrap()
        .with_viewport(&scroll)
        .unwrap();
        let target = PreviewRasterTarget::for_viewport(
            (page[0] * f64::from(device)) as u32,
            (page[1] * f64::from(device)) as u32,
            0.,
            0.,
            1.,
        )
        .unwrap();
        assert!(controller.request((1, 2, 3), target, layout));
        controller.bind_geometry(&picture, &scroll);
        let presentation = controller.presentation.clone().unwrap();
        let image = crate::viewport_paintable::ViewportPaintable::with_tiles(
            green.clone(),
            3096.,
            3096.,
            (0., 0., 3096., 3096.),
            None,
            presentation,
        );
        assert!(!image.flags().contains(gdk::PaintableFlags::STATIC_CONTENTS));
        let signals = std::rc::Rc::new(std::cell::Cell::new(0));
        let n = signals.clone();
        image.connect_invalidate_contents(move |_| n.set(n.get() + 1));
        picture.set_paintable(Some(&image));
        settle();
        let frozen = image.current_image();
        assert!(
            frozen
                .flags()
                .contains(gdk::PaintableFlags::STATIC_CONTENTS)
        );
        let immutable = gtk::Snapshot::new();
        frozen.snapshot(&immutable, 3096., 3096.);
        let retained = immutable.to_node().unwrap();
        assert_eq!(
            (retained.bounds().width(), retained.bounds().height()),
            (3096., 3096.),
            "retained node includes fallback beyond old viewport"
        );
        for (name, x, y) in [("before", 0., 0.), ("after", 600.25, 400.5)] {
            scroll.hadjustment().set_value(x);
            scroll.vadjustment().set_value(y);
            crate::invalidate_main_view_content(&picture);
            settle();
            let snapshot = gtk::Snapshot::new();
            gtk::WidgetPaintable::new(Some(&scroll)).snapshot(
                &snapshot,
                f64::from(scroll.width()),
                f64::from(scroll.height()),
            );
            let texture = renderer.render_texture(
                snapshot.to_node().unwrap(),
                Some(&gtk::graphene::Rect::new(
                    0.,
                    0.,
                    scroll.width() as f32,
                    scroll.height() as f32,
                )),
            );
            texture
                .save_to_png(out.join(format!("scrolling-picture-{name}.png")))
                .unwrap();
            let rgba = download(&texture);
            std::fs::write(out.join(format!("scrolling-picture-{name}.rgba")), &rgba).unwrap();
            let pixel =
                |px: usize, py: usize| &rgba[(py * texture.width() as usize + px) * 4..][..4];
            assert_eq!(
                pixel(10, 10),
                [0, 0, 0, 0],
                "covered alpha0 must not reveal fallback"
            );
            if name == "after" {
                assert_eq!(
                    pixel(page[0] as usize - 20, 20),
                    [0, 255, 0, 255],
                    "newly exposed canvas retains full ordinary fallback"
                );
            }
        }
        assert!(signals.get() >= 2);
        let frozen_again = gtk::Snapshot::new();
        frozen.snapshot(&frozen_again, 3096., 3096.);
        assert_eq!(
            retained.serialize().as_ref(),
            frozen_again.to_node().unwrap().serialize().as_ref(),
            "frozen current_image remains identical after scroll"
        );
    }

    /// Keeps a real complete-ROI/missing-diagonal upload case on the healthy background route.
    /// The sleeping owner worker resumes after the actual request; no synthetic View flags or
    /// certificate permissions are supplied to the production controller.
    /// # Panics
    /// Panics if temporary full-cell padding loss submits ordinary work before background can fill it.
    fn transient_diagonal_gutter_stays_background() {
        let scene = Arc::new(
            RenderScene::new(
                CanvasSpec {
                    width: 6192.,
                    height: 6192.,
                },
                "gutter".into(),
                "gutter".into(),
                vec![
                    RenderLayer::new(
                        ChannelId(1),
                        true,
                        ColorValue {
                            red: 1.,
                            green: 0.,
                            blue: 0.,
                            alpha: 1.,
                        },
                        1.,
                        GeometryOutput::CanonicalMarks(Vec::new()),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
        let mut controller = Controller::new();
        controller.accept((1, 2, 3), scene);
        let target = PreviewRasterTarget::for_viewport(100, 100, 3200., 1700., 1.).unwrap();
        let key = RootKey::new(
            1.,
            target.background(),
            RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())
                .unwrap(),
        );
        let region = Region {
            x: 3198,
            y: 1698,
            width: 104,
            height: 104,
        };
        {
            let mut cache = controller.shared.0.lock().unwrap();
            for i in 0..40 {
                if let Plan::Ready(view) = cache.plan((1, 2, 3), key, region).unwrap() {
                    let has = |x, y| {
                        view.pixels
                            .iter()
                            .any(|p| p.region.x == x && p.region.y == y)
                    };
                    if has(3072, 3072) && !has(4608, 3072) {
                        assert!(view.complete && view.background_pending);
                        break;
                    }
                }
                assert!(
                    i < 39,
                    "specific missing diagonal must precede background completion"
                );
                let job = cache.take_job().unwrap();
                cache.complete(job.run(&AtomicBool::new(false)));
            }
        }
        let layout = Layout::new(
            [6192., 6192.],
            [6192., 6192.],
            [6192., 6192.],
            [-3200., -1700.],
            [100., 100.],
            1,
            1.,
        )
        .unwrap();
        assert!(controller.request((1, 2, 3), target, layout));
        assert!(
            !controller.ordinary_required,
            "pending neighbor support is refinement, not rejection"
        );
    }

    /// Exercises real fixed-grid controller assembly, alpha0 coverage and closest-scale streaming.
    /// # Panics
    /// Panics on a Fit stripe, stale pending/projection, unsupported actual fetch or leaked upload IDs.
    #[test]
    #[ignore = "requires verified agent-owned private Wayland session"]
    fn controller_fixed_grid_multiscale_streaming_preserves_alpha_zero() {
        let (_application, window) = owned_window();
        transient_diagonal_gutter_stays_background();
        let out =
            std::path::PathBuf::from(std::env::var_os("TONIATOR_CACHE_C2_PIXEL_OUTPUT").unwrap());
        std::fs::create_dir_all(&out).unwrap();
        let scene = Arc::new(
            RenderScene::new(
                CanvasSpec {
                    width: 3096.,
                    height: 3096.,
                },
                "assembly".into(),
                "assembly".into(),
                vec![
                    RenderLayer::new(
                        ChannelId(1),
                        true,
                        ColorValue {
                            red: 1.,
                            green: 0.,
                            blue: 0.,
                            alpha: 1.,
                        },
                        1.,
                        GeometryOutput::CanonicalMarks(Vec::new()),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
        let mut controller = Controller::new();
        *controller.shared.0.lock().unwrap() = PreviewCache::default();
        controller.accept((1, 2, 3), scene);
        let layout = Layout::new(
            [3096., 3096.],
            [3096., 3096.],
            [3096., 3096.],
            [-12., -12.],
            [3068., 3068.],
            1,
            1.,
        )
        .unwrap();
        let target = PreviewRasterTarget::for_viewport(3068, 3068, 12., 12., 1.).unwrap();
        let started = std::time::Instant::now();
        loop {
            assert!(
                controller.request((1, 2, 3), target, layout.clone()),
                "healthy off-root query declined"
            );
            if controller.exact_complete {
                break;
            }
            assert!(
                started.elapsed().as_secs() < 15,
                "controller did not complete visible coverage"
            );
            controller.poll();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let presentation = controller.presentation.clone().unwrap();
        assert_eq!(presentation.tiles.len(), 9);
        let fallback = texture(1, 1, [0, 255, 0, 255]);
        let snapshot = gtk::Snapshot::new();
        snapshot.translate(&gtk::graphene::Point::new(-12., -12.));
        assert!(presentation.snapshot(&snapshot, 3096., 3096., &fallback, (0., 0., 3096., 3096.)));
        let renderer = gtk::gsk::Renderer::for_surface(&window.surface().unwrap()).unwrap();
        let result = renderer.render_texture(
            snapshot.to_node().unwrap(),
            Some(&gtk::graphene::Rect::new(0., 0., 3068., 3068.)),
        );
        let rgba = download(&result);
        assert!(
            rgba.chunks_exact(4).all(|p| p[3] == 0),
            "complete transparent coverage leaked Fit"
        );
        result
            .save_to_png(out.join("controller-alpha0.png"))
            .unwrap();
        std::fs::write(out.join("controller-alpha0.rgba"), &rgba).unwrap();
        scrolling_picture_preserves_retained_fallback(&mut controller, &window, &renderer, &out);
        let scroll = gtk::ScrolledWindow::new();
        let picture = gtk::Picture::new();
        scroll.set_child(Some(&picture));
        window.set_child(Some(&scroll));
        let mut guard = presentation.clone();
        guard.geometry = Some((picture.downgrade(), scroll.downgrade()));
        guard.layout.device = if scroll.scale_factor() == 1 { 2 } else { 1 };
        assert!(!guard.snapshot(
            &gtk::Snapshot::new(),
            3096.,
            3096.,
            &fallback,
            (0., 0., 3096., 3096.)
        ));
        assert!(guard.rejected.load(Ordering::Acquire));
        guard.layout.device = scroll.scale_factor();
        guard.layout.page = [
            scroll.hadjustment().page_size() + 1.,
            scroll.vadjustment().page_size(),
        ];
        guard.rejected.store(false, Ordering::Release);
        assert!(!guard.snapshot(
            &gtk::Snapshot::new(),
            3096.,
            3096.,
            &fallback,
            (0., 0., 3096., 3096.)
        ));
        assert!(guard.rejected.load(Ordering::Acquire));
        drop(guard);
        drop(presentation);
        let before = controller.statistics();
        let high_target = PreviewRasterTarget::for_viewport(3068, 3068, 24., 24., 2.).unwrap();
        let high_layout = Layout::new(
            [3096., 3096.],
            [6192., 6192.],
            [6192., 6192.],
            [-24., -24.],
            [3068., 3068.],
            1,
            2.,
        )
        .unwrap();
        assert!(controller.request((1, 2, 3), high_target, high_layout.clone()));
        let immediate = controller
            .presentation
            .as_ref()
            .expect("closest cached scale must remain visible");
        assert_eq!(immediate.view.frame.scale, 1.);
        assert!(!controller.exact_complete && !controller.pending);
        let started = std::time::Instant::now();
        let mut saw_partial = false;
        loop {
            assert!(controller.request((1, 2, 3), high_target, high_layout.clone()));
            if let Some(p) = &controller.presentation {
                if p.view.frame.scale == 2. && !p.tiles.is_empty() && !controller.exact_complete {
                    if !saw_partial {
                        mixed_scale_pixels(p, &renderer, &out);
                    }
                    saw_partial = true;
                    assert!(
                        !p.alternatives.is_empty(),
                        "current fine pixels must retain coarse coverage in holes"
                    );
                }
            }
            if controller.exact_complete {
                break;
            }
            assert!(
                started.elapsed().as_secs() < 30,
                "streaming viewport did not complete"
            );
            controller.poll();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(
            saw_partial,
            "fine resolution must stream before whole visible completion"
        );
        assert!(!controller.pending);
        let after = controller.statistics();
        let unsupported = PreviewRasterTarget::for_viewport(3068, 3068, 24., 24., 1e12).unwrap();
        assert!(
            !controller.request((1, 2, 3), unsupported, high_layout.at_scale(1e12).unwrap()),
            "provisional coarse coverage must not suppress ordinary requested-target fallback"
        );
        assert!(
            controller.presentation.is_some(),
            "ordinary fallback routing must retain coarse projection"
        );
        assert!(
            controller.has_current_pixels(),
            "certified coarse refinement must remain unobtrusive"
        );
        let first_key = RootKey::new(
            1.,
            target.background(),
            RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())
                .unwrap(),
        );
        let second_key = RootKey::new(
            2.,
            target.background(),
            RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())
                .unwrap(),
        );
        let old_ids = {
            let mut cache = controller.shared.0.lock().unwrap();
            cache.pin_views(&[first_key, second_key]);
            cache.textures(usize::MAX, 0); // Synthetic retention pressure, no pixel/GPU allocation.
            cache.resident_ids()
        };
        controller.visit_zoom((1, 2, 3), high_target);
        assert!(controller.presentation.is_none());
        {
            let mut cache = controller.shared.0.lock().unwrap();
            cache.textures(usize::MAX, 0);
            assert!(
                cache.resident_ids().len() < old_ids.len(),
                "memo activation must release older projected roots"
            );
        }
        controller.clear();
        assert!(controller.uploads.is_empty() && controller.presentation.is_none());
        std::fs::write(out.join("results.json"),serde_json::to_vec_pretty(&serde_json::json!({
            "cells":9,"alpha0_visible_complete":true,"device_and_page_guards":true,
            "immediate_fallback_source_scale":1.,"requested_scale":2.,"observed_partial_fine_streaming":saw_partial,
            "foreground_overlay_pending":false,"before":format!("{before:?}"),"after":format!("{after:?}"),
            "renderer":renderer.type_().name()})).unwrap()).unwrap();
        renderer.unrealize();
        window.close();
    }
    /// Renders a real Controller mixed-scale graph with diagnostic colors and transparent fine coverage.
    /// The test replaces texture bytes solely to expose nearest-order/disjoint clipping, not to claim
    /// these colors are canonical artwork. Both device scales reuse certified source-root footprints.
    /// # Panics
    /// Panics on missing coarse holes, invalid fetched support or coarse color beneath alpha0 fine pixels.
    fn mixed_scale_pixels(
        original: &Presentation,
        renderer: &gtk::gsk::Renderer,
        out: &std::path::Path,
    ) {
        let mut empty = original.clone();
        empty.tiles.clear();
        empty.alternatives.clear();
        let coarse = original
            .alternatives
            .iter()
            .find(|p| p.visible_complete())
            .expect("complete cached coarse source")
            .clone();
        let mut candidates = vec![empty, coarse.clone(), original.clone()];
        retain_drawable(&mut candidates);
        assert_eq!(
            candidates.len(),
            1,
            "empty and fully occluded roots must not remain pinned"
        );
        assert_eq!(candidates[0].view.key, coarse.view.key);
        for device in [1, 2] {
            let mut mixed = original.clone();
            let base = Layout::new(
                original.layout.canvas,
                original.layout.preferred,
                original.layout.snapshot,
                original.layout.origin,
                original.layout.page,
                device,
                original.view.frame.scale,
            )
            .unwrap();
            let mut alternatives = std::mem::take(&mut mixed.alternatives);
            for (ordinal, layer) in std::iter::once(&mut mixed)
                .chain(alternatives.iter_mut())
                .enumerate()
            {
                layer.layout = base.at_scale(layer.view.frame.scale).unwrap();
                layer.geometry = None;
                assert!(
                    layer.view.certificate.covers(
                        layer
                            .layout
                            .support(
                                layer.view.frame,
                                &layer.tiles.iter().map(|t| t.interior).collect::<Vec<_>>()
                            )
                            .unwrap()
                    )
                );
                for (i, tile) in layer.tiles.iter_mut().enumerate() {
                    let mut bytes =
                        vec![0u8; tile.padded.width as usize * tile.padded.height as usize * 4];
                    for (index, pixel) in bytes.chunks_exact_mut(4).enumerate() {
                        let rgba = if ordinal == 0 {
                            if i == 0
                                && index % (tile.padded.width as usize)
                                    < tile.padded.width as usize / 2
                            {
                                [0, 0, 0, 0]
                            } else {
                                [255, 0, 0, 128]
                            }
                        } else {
                            [0, 255, 0, 255]
                        };
                        pixel.copy_from_slice(&rgba);
                    }
                    tile.texture = gdk::MemoryTexture::new(
                        tile.padded.width as i32,
                        tile.padded.height as i32,
                        gdk::MemoryFormat::R8g8b8a8,
                        &glib::Bytes::from_owned(bytes),
                        tile.padded.width as usize * 4,
                    )
                    .upcast();
                }
            }
            mixed.alternatives = alternatives;
            let tile = &mixed.tiles[0];
            let root_y = f64::from(tile.interior.y) + f64::from(tile.interior.height) * 0.5;
            let row = (mixed.layout.edge(1, root_y) * f64::from(device)).floor() as u32;
            let whole = Region {
                x: 0,
                y: 0,
                width: (mixed.layout.page[0] * f64::from(device)) as u32,
                height: (mixed.layout.page[1] * f64::from(device)) as u32,
            };
            let holes = cache::missing(
                whole,
                mixed
                    .tiles
                    .iter()
                    .filter_map(|t| mixed.layout.clip(t.interior)),
            );
            let hole = holes
                .iter()
                .find(|r| r.y <= row && row < r.y + r.height && r.width > 4)
                .expect("mixed coarse hole");
            let sample_x = |fraction: f64| {
                (mixed.layout.edge(
                    0,
                    f64::from(tile.padded.x) + f64::from(tile.padded.width) * fraction,
                ) * f64::from(device))
                .floor() as usize
            };
            let clear = sample_x(0.25);
            let red = sample_x(0.75);
            let green = (hole.x + hole.width / 2) as usize;
            let snapshot = gtk::Snapshot::new();
            snapshot.scale(device as f32, device as f32);
            snapshot.translate(&gtk::graphene::Point::new(
                mixed.layout.origin[0] as f32,
                mixed.layout.origin[1] as f32,
            ));
            assert!(mixed.snapshot(
                &snapshot,
                mixed.layout.snapshot[0],
                mixed.layout.snapshot[1],
                &texture(1, 1, [0, 0, 255, 255]),
                (
                    0.,
                    0.,
                    mixed.layout.preferred[0] as f32,
                    mixed.layout.preferred[1] as f32
                )
            ));
            let result = renderer.render_texture(
                snapshot.to_node().unwrap(),
                Some(&gtk::graphene::Rect::new(
                    0.,
                    row as f32,
                    whole.width as f32,
                    4.,
                )),
            );
            let rgba = download(&result);
            assert_eq!(
                &rgba[clear * 4..clear * 4 + 4],
                &[0, 0, 0, 0],
                "fine alpha0 must hide coarse and Fit"
            );
            assert_eq!(
                &rgba[red * 4..red * 4 + 4],
                &[255, 0, 0, 128],
                "fine translucent coverage must not blend coarse"
            );
            assert_eq!(
                &rgba[green * 4..green * 4 + 4],
                &[0, 255, 0, 255],
                "coarse must fill only fine holes"
            );
            result
                .save_to_png(out.join(format!("mixed-scale-device{device}.png")))
                .unwrap();
            std::fs::write(out.join(format!("mixed-scale-device{device}.rgba")), rgba).unwrap();
        }
    }
    /// Compares exact tiled vs single-texture pixels at fractional translated physical clip seams.
    /// Requires an explicitly owned private Wayland session; normal unit runs ignore this test.
    /// # Panics
    /// Panics on changed alpha/coverage, Fit leakage under alpha0, unsupported layout or native I/O.
    #[test]
    #[ignore = "requires verified agent-owned private Wayland session"]
    fn fractional_linear_gutters_and_alpha_zero_match_single_native_texture() {
        pixel_comparison(false);
    }
    /// Records native high-entropy filter quantization without asserting universal consumer byte parity.
    /// # Panics
    /// Panics on invalid owned-session setup, constant-color seams or native artifact failures.
    #[test]
    #[ignore = "requires verified agent-owned private Wayland session"]
    fn high_entropy_fractional_filtering_observation() {
        pixel_comparison(true);
    }
    /// Runs actual GSK comparisons with copied neighboring RGBA texels; observation mode is qualified.
    /// # Panics
    /// Panics on session/setup errors, alpha0 leakage or strict-fixture mismatch; observation records differences.
    fn pixel_comparison(observational: bool) {
        let out = std::env::var_os("TONIATOR_CACHE_C2_PIXEL_OUTPUT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(format!(
                    "target/validation/test-scratch/cache-pixels-{}",
                    std::process::id()
                ))
            });
        std::fs::create_dir_all(&out).unwrap();
        let (_application, window) = owned_window();
        let renderer = gtk::gsk::Renderer::for_surface(&window.surface().unwrap()).unwrap();
        let mut observations = Vec::new();
        for device in [1, 2] {
            for origin in [[-0.375, -0.625], [3.25, 2.5]] {
                for alpha in [0u8, 128, 255, 17] {
                    let scale = f64::from(device);
                    let frame = Frame {
                        width: 96 * device as u32,
                        height: 48 * device as u32,
                        scale,
                        background: toniator_engine::RasterBackground::Transparent,
                    };
                    let scene = Arc::new(
                        RenderScene::new(
                            CanvasSpec {
                                width: 96.,
                                height: 48.,
                            },
                            "pixel-probe".into(),
                            "pixel-probe".into(),
                            vec![
                                RenderLayer::new(
                                    ChannelId(1),
                                    true,
                                    ColorValue {
                                        red: 1.,
                                        green: 0.,
                                        blue: 0.,
                                        alpha: 1.,
                                    },
                                    1.,
                                    GeometryOutput::CanonicalMarks(Vec::new()),
                                )
                                .unwrap(),
                            ],
                        )
                        .unwrap(),
                    );
                    let mut cache = PreviewCache::default();
                    cache.accept_scene((1, 2, 3), scene.clone(), 1000);
                    let key = RootKey::new(scale, frame.background, RasterizationLimits::default());
                    let full = Region {
                        x: 0,
                        y: 0,
                        width: frame.width,
                        height: frame.height,
                    };
                    let view = loop {
                        if let Plan::Ready(view) = cache.plan((1, 2, 3), key, full).unwrap()
                            && view.complete
                        {
                            break Arc::new(view);
                        }
                        while let Some(job) = cache.take_job() {
                            cache.complete(job.run(&AtomicBool::new(false)));
                        }
                    };
                    let layout = Layout::new(
                        [96., 48.],
                        [96., 48.],
                        [96., 48.],
                        origin,
                        [80., 40.],
                        device,
                        scale,
                    )
                    .unwrap();
                    if device == 1 && origin[0] < 0. && alpha == 0 {
                        let mut controller = Controller::new();
                        controller.accept((1, 2, 3), scene.clone());
                        controller.pending = true;
                        controller.presentation = Some(Presentation {
                            view: view.clone(),
                            layout: layout.clone(),
                            tiles: Vec::new(),
                            alternatives: Vec::new(),
                            rejected: Arc::new(AtomicBool::new(false)),
                            geometry: None,
                            observed: Arc::new(AtomicBool::new(false)),
                        });
                        let ordinary = PreviewRasterTarget::new(80, 40).unwrap();
                        assert!(!controller.request((1, 2, 3), ordinary, layout.clone()));
                        assert!(!controller.pending && controller.presentation.is_none());
                        assert!(
                            controller
                                .shared
                                .0
                                .lock()
                                .unwrap()
                                .has_scene((1, 2, 3), &scene)
                        );
                        controller.remember_fallback(
                            (1, 2, 3),
                            ordinary,
                            texture(1, 1, [30, 40, 50, 255]),
                        );
                        assert!(controller.fallback((1, 2, 3), ordinary).is_some());
                    }
                    let color = [210, 30, 110, alpha];
                    let rgba = (0..frame.height)
                        .flat_map(|y| {
                            (0..frame.width).flat_map(move |x| {
                                if alpha == 17 {
                                    if observational {
                                        [
                                            if (x / 7 + y / 5) % 2 == 0 { 240 } else { 20 },
                                            ((x * 13 + y * 23) % 256) as u8,
                                            ((x * 3 + y * 11) % 256) as u8,
                                            [0, 64, 128, 255][((x / 3 + y / 2) % 4) as usize],
                                        ]
                                    } else {
                                        [
                                            if (x / 7 + y / 5) % 2 == 0 { 255 } else { 0 },
                                            if (x / 3) % 2 == 0 { 255 } else { 0 },
                                            if (y / 2) % 2 == 0 { 255 } else { 0 },
                                            [0, 64, 128, 192][((x / 3 + y / 2) % 4) as usize],
                                        ]
                                    }
                                } else {
                                    color
                                }
                            })
                        })
                        .collect::<Vec<_>>();
                    let fallback = texture(1, 1, [0, 255, 0, 255]);
                    let mut pieces = Vec::new();
                    let step = frame.width / 3;
                    for x in [0, step, 2 * step] {
                        let interior = Region {
                            x,
                            y: 0,
                            width: step,
                            height: frame.height,
                        };
                        let padded = cache::gutter(interior, frame);
                        pieces.push(Tile {
                            texture: copied_texture(padded, frame.width, &rgba),
                            interior,
                            padded,
                        });
                    }
                    let mut raw = Vec::new();
                    for (name, tiles) in [
                        ("tiled", pieces),
                        (
                            "single",
                            vec![Tile {
                                texture: copied_texture(full, frame.width, &rgba),
                                interior: full,
                                padded: full,
                            }],
                        ),
                    ] {
                        let presentation = Presentation {
                            view: view.clone(),
                            layout: layout.clone(),
                            tiles,
                            alternatives: Vec::new(),
                            rejected: Arc::new(AtomicBool::new(false)),
                            geometry: None,
                            observed: Arc::new(AtomicBool::new(false)),
                        };
                        let snapshot = gtk::Snapshot::new();
                        snapshot.scale(device as f32, device as f32);
                        snapshot.translate(&gtk::graphene::Point::new(
                            layout.origin[0] as f32,
                            layout.origin[1] as f32,
                        ));
                        assert!(presentation.snapshot(
                            &snapshot,
                            96.,
                            48.,
                            &fallback,
                            (0., 0., 96., 48.)
                        ));
                        let node = snapshot.to_node().unwrap();
                        if device == 2 && alpha == 128 {
                            node.write_to_file(out.join(format!("node-{name}.node")))
                                .unwrap();
                        }
                        let result = renderer.render_texture(
                            &node,
                            Some(&gtk::graphene::Rect::new(
                                0.,
                                0.,
                                80. * device as f32,
                                40. * device as f32,
                            )),
                        );
                        let label = format!(
                            "{device}-{}-{alpha}-{name}",
                            if origin[0] < 0. { "pan" } else { "letterbox" }
                        );
                        result
                            .save_to_png(out.join(format!("{label}.png")))
                            .unwrap();
                        let pixels = download(&result);
                        std::fs::write(out.join(format!("{label}.rgba")), &pixels).unwrap();
                        raw.push(pixels);
                    }
                    let differing = raw[0]
                        .chunks_exact(4)
                        .zip(raw[1].chunks_exact(4))
                        .filter(|(a, b)| a != b)
                        .count();
                    if !observational || alpha != 17 {
                        assert!(
                            differing == 0,
                            "seam/alpha mismatch device{device} origin{origin:?} alpha{alpha}: {differing} pixels"
                        );
                    }
                    if alpha == 0 {
                        assert!(
                            raw[0].chunks_exact(4).all(|p| p[3] == 0),
                            "covered alpha0 leaked Fit"
                        );
                    }
                    let mut max_alpha = 0u8;
                    let mut max_straight = 0u8;
                    let mut max_premultiplied = 0u8;
                    let mut positions = Vec::new();
                    for (index, (a, b)) in raw[0]
                        .chunks_exact(4)
                        .zip(raw[1].chunks_exact(4))
                        .enumerate()
                    {
                        max_alpha = max_alpha.max(a[3].abs_diff(b[3]));
                        for k in 0..3 {
                            max_straight = max_straight.max(a[k].abs_diff(b[k]));
                            let pa = (f64::from(a[k]) * f64::from(a[3]) / 255.).round() as u8;
                            let pb = (f64::from(b[k]) * f64::from(b[3]) / 255.).round() as u8;
                            max_premultiplied = max_premultiplied.max(pa.abs_diff(pb));
                        }
                        if a != b && positions.len() < 16 {
                            positions.push([
                                index % (80 * device as usize),
                                index / (80 * device as usize),
                            ]);
                        }
                    }
                    let boundaries = [
                        Region {
                            x: 0,
                            y: 0,
                            width: step,
                            height: frame.height,
                        },
                        Region {
                            x: step,
                            y: 0,
                            width: step,
                            height: frame.height,
                        },
                    ]
                    .map(|r| layout.clip(r).map(|r| r.x + r.width));
                    observations.push(serde_json::json!({"device":device,"origin":origin,"alpha_case":alpha,"observational":observational,"rgba_exact":differing==0,"differing_pixels":differing,"max_alpha_delta":max_alpha,"max_straight_rgb_delta":max_straight,"max_quantized_premultiplied_rgb_delta":max_premultiplied,"first_differing_xy":positions,"tile_boundaries_x":boundaries,"renderer":renderer.type_().name()}));
                }
            }
        }
        std::fs::write(
            out.join("results.json"),
            serde_json::to_vec_pretty(&observations).unwrap(),
        )
        .unwrap();
        renderer.unrealize();
        window.close();
    }
}
