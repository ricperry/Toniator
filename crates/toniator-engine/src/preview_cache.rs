//! One accepted frame's persistent full-zoom preview pixels and retained roots. GTK is never owned here.
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};
pub use toniator_render::preview_tiles::{Frame, Region};
use toniator_render::{
    RasterBackground, RasterSurface, RasterizationLimits, RenderError, RenderScene,
    preview_tiles::{PreviewRoot, RegionCertificate, RootSnapshot},
};

pub const DEFAULT_SOFT_BYTES: usize = 8 * 1024 * 1024 * 1024;
const TILE: u32 = 1536;
/// Accepted upstream workspace/revision/frame identity; viewport changes do not create artwork.
pub type Artwork = (u64, u64, u64);
/// Exact effective uniform physical scale, background and numerical work policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootKey {
    scale: u64,
    background: u8,
    edges: usize,
    boolean: usize,
}
impl RootKey {
    /// Captures the entire current retained-root raster contract, excluding view/pan dimensions.
    pub fn new(scale: f64, background: RasterBackground, limits: RasterizationLimits) -> Self {
        Self {
            scale: scale.to_bits(),
            background: match background {
                RasterBackground::Transparent => 0,
                RasterBackground::OpaqueBlack => 1,
                RasterBackground::OpaqueWhite => 2,
            },
            edges: limits.max_flattened_edges(),
            boolean: limits.max_boolean_operations(),
        }
    }
    /// Restores only the admitted immutable backing selected by this key.
    fn backing(self) -> RasterBackground {
        match self.background {
            1 => RasterBackground::OpaqueBlack,
            2 => RasterBackground::OpaqueWhite,
            _ => RasterBackground::Transparent,
        }
    }
    /// Restores the exact original numerical work allowance without per-tile replenishment.
    fn limits(self) -> RasterizationLimits {
        RasterizationLimits::new(self.edges)
            .expect("admitted positive edge limit")
            .with_max_boolean_operations(self.boolean)
    }
}
/// One immutable CPU rectangle; its bytes are upload storage, not independent display permission.
#[derive(Clone)]
pub struct Pixels {
    pub id: u64,
    pub region: Region,
    pub surface: Arc<RasterSurface>,
    touch: u64,
}
/// Complete current sampled-region permission plus available sparse coverage; alpha is not coverage.
pub struct View {
    pub key: RootKey,
    pub generation: u64,
    pub frame: Frame,
    pub certificate: Arc<RegionCertificate>,
    pub pixels: Vec<Pixels>,
    pub complete: bool,
    /// Actual remaining preparation/paint demand; a temporary upload-gutter gap is not refusal.
    pub background_pending: bool,
    pub expected: Vec<Region>,
    validity: Arc<AtomicU64>,
    authority: Arc<AtomicBool>,
}
impl View {
    /// Rechecks artwork and root publication validity at upload/GTK draw time, without locking.
    pub fn is_current(&self) -> bool {
        let expected = self.generation.wrapping_add(1);
        expected != 0
            && !self.authority.load(Ordering::Acquire)
            && self.validity.load(Ordering::Acquire) == expected
    }
    /// Copies already-rendered neighbor texels into private upload storage without rasterization.
    /// Upload bytes alone grant no display permission; the current whole-view token remains required.
    /// # Errors
    /// Rejects invalidated authority, missing coverage, checked size overflow or allocation refusal.
    pub fn copy_upload(&self, region: Region) -> Result<Vec<u8>, RenderError> {
        if !self.is_current() || !missing(region, self.pixels.iter().map(|p| p.region)).is_empty() {
            return Err(error("upload coverage is unavailable"));
        }
        copy_pixels(region, &self.pixels)
    }
}
/// Copies immutable private storage on the worker; no copied bytes grant presentation permission.
/// # Errors
/// Returns checked size/allocation/missing coverage errors without partially publishing a buffer.
fn copy_pixels(region: Region, pixels: &[Pixels]) -> Result<Vec<u8>, RenderError> {
    if !missing(region, pixels.iter().map(|p| p.region)).is_empty() {
        return Err(error("missing upload texels"));
    }
    let len = (region.width as usize)
        .checked_mul(region.height as usize)
        .and_then(|v| v.checked_mul(4))
        .ok_or(error("upload size overflows"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(len)
        .map_err(|_| error("upload allocation failed"))?;
    bytes.resize(len, 0);
    for p in pixels {
        if let Some(part) = intersection(region, p.region) {
            for row in 0..part.height {
                let src = ((part.y - p.region.y + row) as usize * p.region.width as usize
                    + (part.x - p.region.x) as usize)
                    * 4;
                let dst = ((part.y - region.y + row) as usize * region.width as usize
                    + (part.x - region.x) as usize)
                    * 4;
                let n = part.width as usize * 4;
                bytes[dst..dst + n].copy_from_slice(&p.surface.pixels()[src..src + n]);
            }
        }
    }
    Ok(bytes)
}
/// Distinguishes retained work from ordinary-renderer fallback without manufacturing a render error.
pub enum Plan {
    Unavailable,
    Waiting,
    Ready(View),
}
/// Observed calls and pixels, separate from canonical primitive progress or source evaluation.
#[derive(Default, Clone, Copy, Debug)]
pub struct Statistics {
    pub root_preparations: u64,
    pub inflight_estimated_bytes: usize,
    pub paint_jobs: u64,
    pub painted_pixels: u64,
    pub certificate_queries: u64,
    pub full_hits: u64,
    pub partial_hits: u64,
    pub evictions: u64,
    pub unsupported_roots: u64,
    pub retained_cpu_bytes: usize,
    pub charged_texture_bytes: usize,
    pub retained_prepared_bytes: usize,
    pub retained_materialized_scene_bytes: usize,
    pub estimated_scene_bytes: usize,
    pub copied_upload_bytes: u64,
    pub stored_tiles: usize,
    pub completed_zooms: usize,
    pub visited_zooms: usize,
    pub retired_root_payload_bytes: usize,
}
struct Entry {
    key: RootKey,
    root: Option<Arc<Mutex<PreviewRoot>>>,
    generation: u64,
    frame: Option<Frame>,
    certificates: Vec<Arc<RegionCertificate>>,
    pixels: Vec<Pixels>,
    inflight: Vec<Region>,
    preparing: bool,
    querying: bool,
    unsupported: bool,
    failed_views: Vec<Region>,
    failed_paints: Vec<Region>,
    validity: Arc<AtomicU64>,
    touch: u64,
    visible: Option<Region>,
    projected: bool,
    prepared_bytes: usize,
    materialized_bytes: usize,
    snapshot: Option<RootSnapshot>,
    next_tile: u64,
    uploading: Vec<(u64, Region)>,
    uploads: Vec<(u64, Region, Vec<u8>)>,
    failed_uploads: Vec<(u64, Region)>,
}
/// Bounded queued work owns snapshots, never document transactions or GTK objects.
pub struct Job {
    artwork: Artwork,
    key: RootKey,
    cancelled: Arc<AtomicBool>,
    kind: JobKind,
    inflight: Arc<AtomicUsize>,
}
enum JobKind {
    Prepare {
        scene: Arc<RenderScene>,
        budget: usize,
        scene_bytes: usize,
    },
    Certify {
        root: Arc<Mutex<PreviewRoot>>,
        region: Region,
        validity: Arc<AtomicU64>,
    },
    Retire {
        entries: Vec<Entry>,
    },
    Upload {
        id: u64,
        region: Region,
        generation: u64,
        pixels: Vec<Pixels>,
    },
    Paint {
        root: Arc<Mutex<PreviewRoot>>,
        snapshot: RootSnapshot,
        region: Region,
    },
}
impl Completion {
    /// Tests whether immutable completion can change current visible coverage or view permission.
    /// Only visible cells, their full upload-neighbor support, and background completion/termination
    /// request a GTK replan; unrelated offscreen tiles do not rebuild the presentation.
    pub fn changes_view(&self, cache: &PreviewCache) -> bool {
        if !cache
            .artwork
            .is_some_and(|current| current.0 == self.artwork.0 && current.2 == self.artwork.2)
            || !Arc::ptr_eq(&cache.cancelled, &self.authority)
        {
            return false;
        }
        let Some(e) = cache.entries.iter().find(|e| e.key == self.key) else {
            return false;
        };
        match &self.kind {
            Completed::Painted { region, .. } | Completed::Uploaded { region, .. } => {
                e.frame.is_some_and(|frame| {
                    e.visible.is_some_and(|visible| tiles_for_region(visible, frame).into_iter()
                        .any(|cell| intersection(gutter(cell,frame), *region).is_some()))
                        || matches!(&self.kind, Completed::Painted { generation, .. }
                            if *generation == e.generation && e.inflight.len() == 1 && e.next_tile >= tile_count(frame))
                        || matches!(&self.kind, Completed::Painted { result: Ok(_), generation, .. }
                            if *generation == e.generation && e.pixels.len() as u64 + 1 == tile_count(frame))
                })
            }
            Completed::Retired => false,
            _ => true,
        }
    }
}
/// Private worker outcome; only matching accepted artwork/root may enter resident coverage.
pub struct Completion {
    artwork: Artwork,
    key: RootKey,
    kind: Completed,
    authority: Arc<AtomicBool>,
}
enum Completed {
    Prepared(Result<Prepared, RenderError>),
    Certified {
        region: Region,
        result: Result<RegionCertificate, RenderError>,
        generation: u64,
        prepared_bytes: usize,
        snapshot: Result<RootSnapshot, RenderError>,
    },
    Retired,
    Uploaded {
        id: u64,
        region: Region,
        generation: u64,
        result: Result<Vec<u8>, RenderError>,
    },
    Painted {
        region: Region,
        generation: u64,
        result: Result<RasterSurface, RenderError>,
    },
}

/// Worker-produced root facts keep preparation/materialization scans outside the metadata lock.
struct Prepared {
    root: PreviewRoot,
    snapshot: RootSnapshot,
    prepared_bytes: usize,
    materialized_bytes: usize,
}
/// Owns one active frame and all its exact-scale roots, CPU regions, certificates, queue and LRU.
/// Soft accounting includes known payload plus conservative texture/preparation estimates; it never
/// preallocates8GiB or rejects artwork solely for exceeding the coherent retention target.
struct Exact {
    id: u64,
    target: toniator_render::PreviewRasterTarget,
    touch: u64,
    bytes: usize,
}

pub struct PreviewCache {
    artwork: Option<Artwork>,
    scene: Option<Arc<RenderScene>>,
    cancelled: Arc<AtomicBool>,
    entries: Vec<Entry>,
    queue: VecDeque<Job>,
    clock: u64,
    next_id: u64,
    budget: usize,
    stats: Statistics,
    texture_charge: usize,
    scene_estimate: usize,
    exact: Vec<Exact>,
    inflight: Arc<AtomicUsize>,
    active: Option<RootKey>,
    background_cursor: usize,
    priority_streak: u8,
    visible_streak: u8,
    retired: Vec<Entry>,
}
impl Default for PreviewCache {
    /// Creates the generous full-zoom production retention policy without allocating payload.
    fn default() -> Self {
        Self::new(DEFAULT_SOFT_BYTES)
    }
}
impl Drop for PreviewCache {
    /// Revokes detached projection/work handles when their sole cache owner is released.
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        for entry in &self.entries {
            entry.validity.store(0, Ordering::Release);
        }
    }
}
/// Emits explicitly enabled owned diagnostic records; ordinary production never writes tile traces.
fn trace(event: &str, key: RootKey, region: Option<Region>, stats: Option<Statistics>) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED
        .get_or_init(|| std::env::var_os("TONIATOR_PREVIEW_CACHE_TRACE").is_some_and(|v| v == "1"))
    {
        eprintln!(
            "preview-cache-worker event={event} scale={} region={region:?} stats={stats:?}",
            f64::from_bits(key.scale)
        );
    }
}
/// Returns a stable optimization diagnostic; the frontend retains its ordinary renderer fallback.
fn error(message: &'static str) -> RenderError {
    RenderError::new("preview.cache", message)
}
/// Intersects half-open integer rectangles with widened endpoint arithmetic.
pub fn intersection(a: Region, b: Region) -> Option<Region> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (u64::from(a.x) + u64::from(a.width)).min(u64::from(b.x) + u64::from(b.width));
    let bottom = (u64::from(a.y) + u64::from(a.height)).min(u64::from(b.y) + u64::from(b.height));
    (right > u64::from(x) && bottom > u64::from(y)).then(|| Region {
        x,
        y,
        width: (right - u64::from(x)) as u32,
        height: (bottom - u64::from(y)) as u32,
    })
}
/// Subtracts one coverage rectangle into disjoint residuals; alpha bytes never participate.
fn subtract(a: Region, b: Region) -> Vec<Region> {
    let Some(i) = intersection(a, b) else {
        return vec![a];
    };
    let ar = a.x + a.width;
    let ab = a.y + a.height;
    let ir = i.x + i.width;
    let ib = i.y + i.height;
    [
        (a.x, a.y, a.width, i.y - a.y),
        (a.x, ib, a.width, ab - ib),
        (a.x, i.y, i.x - a.x, i.height),
        (ir, i.y, ar - ir, i.height),
    ]
    .into_iter()
    .filter(|v| v.2 > 0 && v.3 > 0)
    .map(|(x, y, width, height)| Region {
        x,
        y,
        width,
        height,
    })
    .collect()
}
/// Computes missing coverage without repainting any accepted or in-flight root texel.
pub fn missing(region: Region, coverage: impl IntoIterator<Item = Region>) -> Vec<Region> {
    let mut pieces = vec![region];
    for rect in coverage {
        pieces = pieces.into_iter().flat_map(|p| subtract(p, rect)).collect();
    }
    pieces
}
/// Expands by one real root texel for bilinear neighboring samples, clipped only to the root edge.
pub fn gutter(region: Region, frame: Frame) -> Region {
    let x = region.x.saturating_sub(1);
    let y = region.y.saturating_sub(1);
    let right =
        (u64::from(region.x) + u64::from(region.width) + 1).min(u64::from(frame.width)) as u32;
    let bottom =
        (u64::from(region.y) + u64::from(region.height) + 1).min(u64::from(frame.height)) as u32;
    Region {
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}
/// Checks one coherent zoom's known payload without allocating its virtual pixel array.
fn fits_zoom(
    width: u32,
    height: u32,
    scene: usize,
    prepared: usize,
    materialized: usize,
    budget: usize,
) -> bool {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| n.checked_add(scene))
        .and_then(|n| n.checked_add(prepared))
        .and_then(|n| n.checked_add(materialized))
        .is_some_and(|n| n <= budget)
}
/// Counts a fixed canvas-origin grid using widened checked-admitted frame dimensions.
fn tile_count(frame: Frame) -> u64 {
    u64::from(frame.width.div_ceil(TILE)) * u64::from(frame.height.div_ceil(TILE))
}
/// Resolves a stable row-major grid ordinal; callers provide an ordinal below tile_count.
fn tile_at(frame: Frame, index: u64) -> Region {
    let columns = u64::from(frame.width.div_ceil(TILE));
    let x = (index % columns) as u32 * TILE;
    let y = (index / columns) as u32 * TILE;
    Region {
        x,
        y,
        width: TILE.min(frame.width - x),
        height: TILE.min(frame.height - y),
    }
}
/// Enumerates only grid cells intersecting a bounded visible/filter-support rectangle.
fn tiles_for_region(region: Region, frame: Frame) -> Vec<Region> {
    let columns = u64::from(frame.width.div_ceil(TILE));
    let mut result = Vec::new();
    for row in region.y / TILE..=(region.y + region.height - 1) / TILE {
        for col in region.x / TILE..=(region.x + region.width - 1) / TILE {
            result.push(tile_at(frame, u64::from(row) * columns + u64::from(col)));
        }
    }
    result
}
impl PreviewCache {
    /// Creates an empty full-zoom cache; no pixel or prepared-root storage allocates upfront.
    pub fn new(budget: usize) -> Self {
        Self {
            artwork: None,
            scene: None,
            cancelled: Arc::new(AtomicBool::new(false)),
            entries: Vec::new(),
            queue: VecDeque::new(),
            clock: 0,
            next_id: 0,
            budget,
            stats: Statistics::default(),
            texture_charge: 0,
            scene_estimate: 0,
            exact: Vec::new(),
            inflight: Arc::new(AtomicUsize::new(0)),
            active: None,
            background_cursor: 0,
            priority_streak: 0,
            visible_streak: 0,
            retired: Vec::new(),
        }
    }
    /// Revokes all derived work after workspace/frame/artwork replacement, preserving no old scene.
    pub fn clear(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.artwork = None;
        self.active = None;
        self.scene = None;
        self.retired.append(&mut self.entries);
        self.queue.clear();
        self.cancelled = Arc::new(AtomicBool::new(false));
        self.texture_charge = 0;
        self.scene_estimate = 0;
        self.exact.clear();
    }
    /// Receives only a scheduler-accepted immutable scene; equal explicit authority preserves reuse.
    /// `scene_bytes` is the caller's conservative canonical-storage estimate, charged exactly once.
    pub fn accept_scene(&mut self, artwork: Artwork, scene: Arc<RenderScene>, scene_bytes: usize) {
        if self.artwork == Some(artwork)
            && self
                .scene
                .as_ref()
                .is_some_and(|old| old.identity() == scene.identity())
        {
            return;
        }
        self.clear();
        self.artwork = Some(artwork);
        self.scene = Some(scene);
        self.scene_estimate = scene_bytes;
    }
    /// Rebinds only an explicit domain None-invalidation transition with unchanged workspace/frame.
    /// Queued/running derived work keeps its accepted-scene epoch; upstream document tokens stay stale.
    pub fn rebind_unchanged(&mut self, old: Artwork, new: Artwork) -> bool {
        if !self.has_artwork(old) || old.0 != new.0 || old.2 != new.2 {
            return false;
        }
        self.artwork = Some(new);
        true
    }
    /// Tests explicit accepted frame plus immutable scene identity before preserving GTK-derived data.
    pub fn has_scene(&self, artwork: Artwork, scene: &RenderScene) -> bool {
        self.has_artwork(artwork)
            && self
                .scene
                .as_ref()
                .is_some_and(|s| s.identity() == scene.identity())
    }
    /// Retains ordinary exact-target upload identity without claiming spatial fixed-grid coverage.
    /// Activating the memo advances current demand so previous root pins can be reclaimed by LRU.
    pub fn remember_exact(
        &mut self,
        artwork: Artwork,
        target: toniator_render::PreviewRasterTarget,
        bytes: usize,
    ) -> Option<u64> {
        if !self.has_artwork(artwork) {
            return None;
        }
        self.clock = self.clock.wrapping_add(1);
        self.active = None;
        self.exact.retain(|e| e.target != target);
        self.next_id = self.next_id.wrapping_add(1);
        let id = self.next_id;
        self.exact.push(Exact {
            id,
            target,
            touch: self.clock,
            bytes,
        });
        self.evict();
        Some(id)
    }
    /// Resolves a resident ordinary target before decoder/scheduler side effects, updating its LRU.
    pub fn exact(
        &mut self,
        artwork: Artwork,
        target: toniator_render::PreviewRasterTarget,
    ) -> Option<u64> {
        if !self.has_artwork(artwork) {
            return None;
        }
        self.clock = self.clock.wrapping_add(1);
        let entry = self.exact.iter_mut().find(|e| e.target == target)?;
        entry.touch = self.clock;
        Some(entry.id)
    }
    /// Returns resident ordinary upload identities for synchronized GTK release after engine eviction.
    pub fn exact_ids(&self) -> Vec<u64> {
        self.exact.iter().map(|e| e.id).collect()
    }
    /// Suspends foreground demand only; visited zooms continue their accepted-scene background work.
    pub fn suspend(&mut self) {}
    /// Tests accepted frame authority before any source cloning, decoder or scheduler submission.
    pub fn has_artwork(&self, artwork: Artwork) -> bool {
        self.artwork == Some(artwork)
    }
    /// Returns unique stored pixels and full pixel demand for live admitted visited zooms.
    /// Preparing roots contribute demand; unsupported or evicted roots do not. Disjoint fixed cells
    /// contribute once, independently of historical paint counters, upload copies, or alpha.
    pub fn background_progress(&self) -> (u64, u64, bool) {
        let Some(scene) = &self.scene else {
            return (0, 0, false);
        };
        let (stored, total) = self.entries.iter().filter(|entry| !entry.unsupported).fold(
            (0u64, 0u64),
            |(stored, total), entry| {
                let scale = f64::from_bits(entry.key.scale);
                // Each retained entry already passed checked finite-u32 virtual extent admission.
                let width = (scene.canvas().width * scale).ceil() as u64;
                let height = (scene.canvas().height * scale).ceil() as u64;
                let pixels = entry.pixels.iter().fold(0u64, |sum, p| {
                    sum.saturating_add(u64::from(p.region.width) * u64::from(p.region.height))
                });
                (
                    stored.saturating_add(pixels),
                    total.saturating_add(width.saturating_mul(height)),
                )
            },
        );
        let pending = self.entries.iter().any(|e| {
            !e.unsupported
                && (e.preparing
                    || !e.inflight.is_empty()
                    || (e.snapshot.is_some()
                        && e.frame.is_some_and(|f| e.next_tile < tile_count(f))))
        });
        (stored, total, pending)
    }

    /// Returns actual cache counters and measured/estimated retained charges, never primitive counts.
    pub fn statistics(&self) -> Statistics {
        let mut s = self.stats;
        s.stored_tiles = self.entries.iter().map(|e| e.pixels.len()).sum();
        s.visited_zooms = self.entries.len();
        s.completed_zooms = self
            .entries
            .iter()
            .filter(|e| {
                e.frame
                    .is_some_and(|f| e.pixels.len() as u64 == tile_count(f))
            })
            .count();
        s.retired_root_payload_bytes = self.retired.iter().fold(0usize, |bytes, e| {
            bytes
                .saturating_add(e.prepared_bytes)
                .saturating_add(e.materialized_bytes)
                .saturating_add(
                    e.pixels
                        .iter()
                        .map(|p| p.surface.pixels().len())
                        .sum::<usize>(),
                )
        });
        s.inflight_estimated_bytes = self.inflight.load(Ordering::Acquire);
        s.retained_cpu_bytes = self
            .entries
            .iter()
            .flat_map(|e| &e.pixels)
            .map(|p| p.surface.pixels().len())
            .sum::<usize>()
            .saturating_add(
                self.entries
                    .iter()
                    .flat_map(|e| &e.uploads)
                    .map(|u| u.2.capacity())
                    .sum(),
            );
        s.retained_prepared_bytes = self.entries.iter().map(|e| e.prepared_bytes).sum();
        s.retained_materialized_scene_bytes =
            self.entries.iter().map(|e| e.materialized_bytes).sum();
        s.charged_texture_bytes = self
            .texture_charge
            .saturating_add(self.exact.iter().map(|e| e.bytes).sum::<usize>());
        s.estimated_scene_bytes = self.scene_estimate;
        s
    }
    /// Charges app-owned uploads/GPU estimates and copy observations without taking GTK ownership.
    pub fn textures(&mut self, bytes: usize, copied: u64) {
        self.texture_charge = bytes;
        self.stats.copied_upload_bytes = self.stats.copied_upload_bytes.saturating_add(copied);
        self.evict();
    }
    /// Pins every source root currently used by a disjoint multi-resolution GTK presentation.
    /// This protects visible coarse fallback independently of the requested active zoom.
    pub fn pin_views(&mut self, keys: &[RootKey]) {
        for entry in &mut self.entries {
            entry.projected = keys.contains(&entry.key);
        }
    }
    /// Pins a newly assembled source before any texture charge can trigger coherent LRU eviction.
    pub fn pin_view(&mut self, key: RootKey) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.key == key) {
            entry.projected = true;
        }
    }
    /// Lists resident pixel identities so GTK drops uploads after engine eviction/generation changes.
    pub fn resident_ids(&self) -> Vec<u64> {
        self.entries
            .iter()
            .flat_map(|e| e.pixels.iter().map(|p| p.id))
            .collect()
    }
    /// Plans complete-region certification and missing-only work without source/evaluation side effects.
    /// Rare mutable retry runs only on the worker. A busy root never implies valid permission.
    /// # Errors
    /// Returns allocation/geometry admission errors; callers use the ordinary preview path instead.
    pub fn plan(
        &mut self,
        artwork: Artwork,
        key: RootKey,
        region: Region,
    ) -> Result<Plan, RenderError> {
        if !self.has_artwork(artwork) {
            return Ok(Plan::Unavailable);
        }
        let scene = self.scene.as_ref().ok_or(error("scene unavailable"))?;
        let scale = f64::from_bits(key.scale);
        let extent = [scene.canvas().width * scale, scene.canvas().height * scale];
        if !scale.is_finite()
            || scale <= 0.
            || extent
                .iter()
                .any(|v| !v.is_finite() || *v <= 0. || v.ceil() > f64::from(u32::MAX))
        {
            return Ok(Plan::Unavailable);
        }
        if region.width == 0
            || region.height == 0
            || u64::from(region.x) + u64::from(region.width) > extent[0].ceil() as u64
            || u64::from(region.y) + u64::from(region.height) > extent[1].ceil() as u64
        {
            return Err(error("query region exceeds virtual frame"));
        }
        self.clock = self.clock.wrapping_add(1);
        self.active = Some(key);
        let index = if let Some(i) = self.entries.iter().position(|e| e.key == key) {
            i
        } else {
            let scene = self
                .scene
                .as_ref()
                .ok_or(error("scene unavailable"))?
                .clone();
            self.entries
                .try_reserve(1)
                .map_err(|_| error("root metadata allocation failed"))?;
            let cacheable = fits_zoom(
                extent[0].ceil() as u32,
                extent[1].ceil() as u32,
                self.scene_estimate,
                0,
                0,
                self.budget,
            );
            self.entries.push(Entry {
                key,
                root: None,
                generation: 0,
                frame: None,
                certificates: Vec::new(),
                pixels: Vec::new(),
                inflight: Vec::new(),
                preparing: cacheable,
                querying: false,
                unsupported: !cacheable,
                failed_views: Vec::new(),
                failed_paints: Vec::new(),
                validity: Arc::new(AtomicU64::new(0)),
                touch: self.clock,
                visible: Some(region),
                projected: false,
                prepared_bytes: 0,
                materialized_bytes: 0,
                snapshot: None,
                next_tile: 0,
                uploading: Vec::new(),
                uploads: Vec::new(),
                failed_uploads: Vec::new(),
            });
            if !cacheable {
                self.stats.unsupported_roots += 1;
                return Ok(Plan::Unavailable);
            }
            self.queue.push_back(Job {
                artwork,
                key,
                cancelled: self.cancelled.clone(),
                inflight: self.inflight.clone(),
                kind: JobKind::Prepare {
                    scene,
                    budget: self.budget,
                    scene_bytes: self.scene_estimate,
                },
            });

            return Ok(Plan::Waiting);
        };
        let entry = &mut self.entries[index];
        entry.touch = self.clock;
        entry.visible = Some(region);
        if entry.unsupported
            || entry.failed_views.contains(&region)
            || entry
                .failed_paints
                .iter()
                .any(|r| intersection(*r, region).is_some())
        {
            return Ok(Plan::Unavailable);
        }
        let Some(root) = entry.root.clone() else {
            if !entry.preparing {
                entry.preparing = true;
                self.queue.push_back(Job {
                    artwork,
                    key,
                    cancelled: self.cancelled.clone(),
                    inflight: self.inflight.clone(),
                    kind: JobKind::Prepare {
                        scene: self
                            .scene
                            .as_ref()
                            .ok_or(error("scene unavailable"))?
                            .clone(),
                        budget: self.budget,
                        scene_bytes: self.scene_estimate,
                    },
                });
            }
            return Ok(Plan::Waiting);
        };
        if entry.validity.load(Ordering::Acquire) != entry.generation.wrapping_add(1) {
            return Ok(Plan::Waiting);
        }
        let certificate = entry
            .certificates
            .iter()
            .find(|c| c.generation() == entry.generation && c.covers(region))
            .cloned();
        let certificate = match certificate {
            Some(c) => c,
            None => {
                if entry.querying {
                    // Replace only an unstarted view query, never any fixed-tile background demand.
                    for job in &mut self.queue {
                        if job.key == key
                            && let JobKind::Certify {
                                region: pending, ..
                            } = &mut job.kind
                        {
                            *pending = region;
                        }
                    }
                    return Ok(Plan::Waiting);
                }
                entry.querying = true;
                self.queue.push_front(Job {
                    artwork,
                    key,
                    cancelled: self.cancelled.clone(),
                    inflight: self.inflight.clone(),
                    kind: JobKind::Certify {
                        root,
                        region,
                        validity: entry.validity.clone(),
                    },
                });
                return Ok(Plan::Waiting);
            }
        };
        for pixel in &mut entry.pixels {
            if intersection(pixel.region, region).is_some() {
                pixel.touch = self.clock;
            }
        }
        let absent = missing(region, entry.pixels.iter().map(|p| p.region));
        let complete = absent.is_empty();
        if complete {
            self.stats.full_hits += 1
        } else {
            self.stats.partial_hits += 1;
        }
        Ok(Plan::Ready(View {
            key,
            generation: entry.generation,
            frame: entry.frame.ok_or(error("root frame missing"))?,
            certificate,
            pixels: entry.pixels.clone(),
            complete,
            background_pending: entry.preparing
                || !entry.inflight.is_empty()
                || entry.frame.is_some_and(|f| entry.next_tile < tile_count(f)),
            expected: tiles_for_region(
                gutter(region, entry.frame.expect("root frame")),
                entry.frame.expect("root frame"),
            ),
            validity: entry.validity.clone(),
            authority: self.cancelled.clone(),
        }))
    }
    /// Queues worker-only neighboring-texel assembly, or transfers its completed owned buffer to GTK.
    /// Storage assembly never authorizes display; the caller keeps its current whole-view permission.
    /// # Errors
    /// Rejects replaced/missing namespaces and stable copy errors instead of an infinite requeue.
    pub fn upload(
        &mut self,
        key: RootKey,
        generation: u64,
        id: u64,
        region: Region,
    ) -> Result<Option<Vec<u8>>, RenderError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.key == key && e.generation == generation)
            .ok_or(error("upload root replaced"))?;
        // Obsolete checked-out copies may finish, but only current padding may publish a buffer.
        entry.uploads.retain(|u| u.0 != id || u.1 == region);
        entry.uploading.retain(|u| u.0 != id || u.1 == region);
        self.queue.retain(|job| {
            !matches!(&job.kind, JobKind::Upload { id: old, region: old_region, .. }
            if job.key == key && *old == id && *old_region != region)
        });
        if let Some(i) = entry
            .uploads
            .iter()
            .position(|u| u.0 == id && u.1 == region)
        {
            return Ok(Some(entry.uploads.swap_remove(i).2));
        }
        if entry.failed_uploads.contains(&(id, region)) {
            return Err(error("upload copy failed"));
        }
        if !entry.uploading.contains(&(id, region)) {
            let pixels = entry
                .pixels
                .iter()
                .filter(|p| intersection(p.region, region).is_some())
                .cloned()
                .collect::<Vec<_>>();
            if !missing(region, pixels.iter().map(|p| p.region)).is_empty() {
                return Ok(None);
            }
            entry.uploading.push((id, region));
            self.queue.push_back(Job {
                artwork: self.artwork.ok_or(error("artwork unavailable"))?,
                key,
                cancelled: self.cancelled.clone(),
                inflight: self.inflight.clone(),
                kind: JobKind::Upload {
                    id,
                    region,
                    generation,
                    pixels,
                },
            });
        }
        Ok(None)
    }
    /// Registers full-zoom background demand even when an ordinary exact-target memo serves the view.
    /// Existing roots gain no new certificate request; memo display retains its own ordinary authority.
    /// # Errors
    /// Returns the planner's virtual-frame validation/allocation error for a newly visited root.
    pub fn visit_zoom(&mut self, artwork: Artwork, key: RootKey) -> Result<(), RenderError> {
        if !self.has_artwork(artwork) {
            return Ok(());
        }
        if let Some(entry) = self.entries.iter_mut().find(|e| e.key == key) {
            self.clock = self.clock.wrapping_add(1);
            self.active = Some(key);
            entry.touch = self.clock;
            return Ok(());
        }
        self.plan(
            artwork,
            key,
            Region {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
        )?;
        if let Some(entry) = self.entries.iter_mut().find(|e| e.key == key) {
            entry.visible = None;
        }
        Ok(())
    }
    /// Lists existing matching roots by closeness without preparing new scales or changing demand.
    pub fn candidate_scales(&self, key: RootKey) -> Vec<f64> {
        let desired = f64::from_bits(key.scale);
        let mut scales = self
            .entries
            .iter()
            .filter(|e| {
                !e.unsupported
                    && e.root.is_some()
                    && e.key.background == key.background
                    && e.key.edges == key.edges
                    && e.key.boolean == key.boolean
                    && e.key != key
            })
            .map(|e| f64::from_bits(e.key.scale))
            .collect::<Vec<_>>();
        scales.sort_by(|a, b| {
            (a / desired)
                .ln()
                .abs()
                .total_cmp(&(b / desired).ln().abs())
        });
        scales
    }
    /// Queries an already visited fallback scale without changing which zoom receives foreground priority.
    /// # Errors
    /// Returns the same checked region errors as the exact-scale planner.
    pub fn plan_cached(
        &mut self,
        artwork: Artwork,
        key: RootKey,
        region: Region,
    ) -> Result<Plan, RenderError> {
        if !self.entries.iter().any(|e| e.key == key) {
            return Ok(Plan::Unavailable);
        }
        let active = self.active;
        let result = self.plan(artwork, key, region);
        self.active = active;
        result
    }
    /// Checks out priority preparation/certification, then visible fixed tiles, then lazy background.
    /// No pan removes queued demand. At most one paint snapshot is checked out at a time by the owner.
    pub fn take_job(&mut self) -> Option<Job> {
        if !self.retired.is_empty() {
            return Some(Job {
                artwork: self.artwork.unwrap_or((0, 0, 0)),
                key: RootKey::new(
                    1.,
                    RasterBackground::Transparent,
                    RasterizationLimits::default(),
                ),
                cancelled: self.cancelled.clone(),
                inflight: self.inflight.clone(),
                kind: JobKind::Retire {
                    entries: std::mem::take(&mut self.retired),
                },
            });
        }
        if (self.priority_streak < 2
            || !self.entries.iter().any(|e| {
                e.snapshot.is_some()
                    && !e.unsupported
                    && e.frame.is_some_and(|f| e.next_tile < tile_count(f))
            }))
            && let Some(job) = self.queue.pop_front()
        {
            self.priority_streak = self.priority_streak.saturating_add(1);
            match &job.kind {
                JobKind::Prepare { .. } => self.stats.root_preparations += 1,
                JobKind::Certify { .. } => self.stats.certificate_queries += 1,
                _ => {}
            }
            return Some(job);
        }
        if self.visible_streak < 2
            && let Some(i) = self.entries.iter().position(|e| Some(e.key) == self.active)
        {
            let entry = &self.entries[i];
            if let (Some(frame), Some(visible), Some(_)) =
                (entry.frame, entry.visible, &entry.snapshot)
                && let Some(region) = tiles_for_region(gutter(visible, frame), frame)
                    .into_iter()
                    .find(|r| {
                        !entry.pixels.iter().any(|p| p.region == *r)
                            && !entry.inflight.contains(r)
                            && !entry.failed_paints.contains(r)
                    })
            {
                self.visible_streak += 1;
                return self.paint_job(i, region);
            }
        }
        let n = self.entries.len();
        for offset in 0..n {
            let i = (self.background_cursor + offset) % n;
            let entry = &mut self.entries[i];
            if entry.snapshot.is_none() || entry.unsupported {
                continue;
            }
            let frame = entry.frame.expect("prepared snapshot has frame");
            while entry.next_tile < tile_count(frame) {
                let region = tile_at(frame, entry.next_tile);
                entry.next_tile += 1;
                if !entry.pixels.iter().any(|p| p.region == region)
                    && !entry.inflight.contains(&region)
                    && !entry.failed_paints.contains(&region)
                {
                    self.visible_streak = 0;
                    self.background_cursor = (i + 1) % n;
                    return self.paint_job(i, region);
                }
            }
        }
        self.priority_streak = 0;
        if !self.queue.is_empty() {
            return self.take_job();
        }
        None
    }
    /// Reserves one stable grid tile without certification, allocation or painting under metadata lock.
    fn paint_job(&mut self, index: usize, region: Region) -> Option<Job> {
        let entry = &mut self.entries[index];
        if entry.unsupported {
            return None;
        }
        self.priority_streak = 0;
        let snapshot = entry.snapshot.as_ref()?.clone();
        entry.inflight.push(region);
        self.stats.paint_jobs += 1;
        trace("paint", entry.key, Some(region), None);
        Some(Job {
            artwork: self.artwork?,
            key: entry.key,
            cancelled: self.cancelled.clone(),
            inflight: self.inflight.clone(),
            kind: JobKind::Paint {
                root: entry.root.as_ref()?.clone(),
                snapshot,
                region,
            },
        })
    }
    /// Reports lazy autonomous background demand as well as queued priority jobs.
    pub fn queued(&self) -> bool {
        !self.retired.is_empty()
            || !self.queue.is_empty()
            || self.entries.iter().any(|e| {
                e.snapshot.is_some()
                    && !e.unsupported
                    && e.frame.is_some_and(|f| e.next_tile < tile_count(f))
            })
    }
    /// Accepts same-scene-epoch work after panning or explicit None rebinding, never a stale scene epoch.
    /// Matching workspace/frame and the unrevoked epoch authorize derived work, independently of UI tickets.
    pub fn complete(&mut self, result: Completion) {
        if !self
            .artwork
            .is_some_and(|current| current.0 == result.artwork.0 && current.2 == result.artwork.2)
            || !Arc::ptr_eq(&self.cancelled, &result.authority)
        {
            return;
        }
        let Some(entry) = self.entries.iter_mut().find(|e| e.key == result.key) else {
            return;
        };
        match result.kind {
            Completed::Retired => {}
            Completed::Prepared(result) => {
                entry.preparing = false;
                match result {
                    Ok(prepared) => {
                        let root = prepared.root;
                        entry.materialized_bytes = prepared.materialized_bytes;
                        entry.prepared_bytes = prepared.prepared_bytes;
                        entry.snapshot = Some(prepared.snapshot);
                        entry.generation = root.generation();
                        entry.frame = Some(root.frame());
                        entry
                            .validity
                            .store(entry.generation.wrapping_add(1), Ordering::Release);
                        entry.root = Some(Arc::new(Mutex::new(root)));
                    }
                    Err(e) => {
                        if e.path() != "evaluation.cancelled" {
                            entry.unsupported = true;
                            self.stats.unsupported_roots += 1;
                        } else {
                            entry.unsupported = false;
                        }
                    }
                }
            }
            Completed::Certified {
                region,
                result,
                generation,
                prepared_bytes,
                snapshot,
            } => {
                entry.querying = false;
                if generation != entry.generation {
                    entry.pixels.clear();
                    entry.certificates.clear();
                    entry.inflight.clear();
                    entry.failed_views.clear();
                    entry.failed_paints.clear();
                    entry.generation = generation;
                    entry.next_tile = 0;
                    entry.uploads.clear();
                    entry.uploading.clear();
                    entry.failed_uploads.clear();
                    self.queue.retain(|j| j.key != entry.key);
                    entry.prepared_bytes = prepared_bytes;
                }
                match snapshot {
                    Ok(snapshot) => entry.snapshot = Some(snapshot),
                    Err(_) => {
                        entry.snapshot = None;
                        entry.unsupported = true;
                        entry.validity.store(0, Ordering::Release);
                        self.stats.unsupported_roots += 1;
                        return;
                    }
                }
                entry
                    .validity
                    .store(generation.wrapping_add(1), Ordering::Release);
                match result {
                    Ok(c) => {
                        if entry.certificates.len() >= 64 {
                            entry.certificates.remove(0);
                        }
                        entry.certificates.push(Arc::new(c));
                    }
                    Err(e) => {
                        if e.path() != "evaluation.cancelled" {
                            entry.failed_views.push(region);
                        }
                    }
                }
            }
            Completed::Uploaded {
                id,
                region,
                generation,
                result,
            } => {
                let requested = entry.uploading.contains(&(id, region));
                entry.uploading.retain(|u| *u != (id, region));
                if requested && generation == entry.generation {
                    match result {
                        Ok(bytes) => entry.uploads.push((id, region, bytes)),
                        Err(e) if e.path() != "evaluation.cancelled" => {
                            entry.failed_uploads.push((id, region))
                        }
                        _ => {}
                    }
                }
            }
            Completed::Painted {
                region,
                generation,
                result,
            } => {
                entry.inflight.retain(|r| *r != region);
                if generation == entry.generation {
                    match result {
                        Ok(surface) => {
                            self.next_id = self.next_id.wrapping_add(1);

                            self.stats.painted_pixels +=
                                u64::from(region.width) * u64::from(region.height);
                            entry.pixels.push(Pixels {
                                id: self.next_id,
                                region,
                                surface: Arc::new(surface),
                                touch: self.clock,
                            });
                        }
                        Err(e) => {
                            if e.path() != "evaluation.cancelled" {
                                entry.failed_paints.push(region);
                            } else if let Some(frame) = entry.frame {
                                let ordinal = u64::from(region.y / TILE)
                                    * u64::from(frame.width.div_ceil(TILE))
                                    + u64::from(region.x / TILE);
                                entry.next_tile = entry.next_tile.min(ordinal);
                            }
                        }
                    }
                }
            }
        }
        self.evict();
        trace("completion", result.key, None, Some(self.statistics()));
    }
    /// Evicts least-recently-used unpinned roots only when retained estimates exceed the soft target.
    fn evict(&mut self) {
        loop {
            let s = self.statistics();
            let total = s
                .retained_cpu_bytes
                .saturating_add(s.retained_prepared_bytes)
                .saturating_add(s.retained_materialized_scene_bytes)
                .saturating_add(self.scene_estimate)
                .saturating_add(s.charged_texture_bytes)
                .saturating_add(s.inflight_estimated_bytes);
            if total <= self.budget {
                break;
            }
            if let Some(i) = self
                .exact
                .iter()
                .enumerate()
                .filter(|(_, e)| e.touch != self.clock)
                .min_by_key(|(_, e)| e.touch)
                .map(|(i, _)| i)
            {
                self.exact.remove(i);
                self.stats.evictions += 1;
                continue;
            }
            let victim = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    !e.preparing
                        && !e.querying
                        && e.inflight.is_empty()
                        && Some(e.key) != self.active
                        && !e.projected
                })
                .min_by_key(|(_, e)| e.touch)
                .map(|(i, _)| i);
            let Some(i) = victim else {
                break;
            };
            self.entries[i].validity.store(0, Ordering::Release);
            let key = self.entries[i].key;
            self.queue.retain(|job| job.key != key);
            self.retired.push(self.entries.remove(i));
            self.stats.evictions += 1;
        }
    }
}
/// Executes one checked-out job with cancellation shared only by its accepted artwork lifetime.
impl Job {
    /// Performs preparation/certification/painting outside metadata and GTK locks.
    pub fn run(self, shutdown: &AtomicBool) -> Completion {
        let cancelled =
            || shutdown.load(Ordering::Relaxed) || self.cancelled.load(Ordering::Relaxed);
        let kind = match self.kind {
            JobKind::Retire { entries } => {
                drop(entries);
                Completed::Retired
            }
            JobKind::Prepare {
                scene,
                budget,
                scene_bytes,
            } => {
                let result = PreviewRoot::new(
                    scene.clone(),
                    f64::from_bits(self.key.scale),
                    self.key.backing(),
                    self.key.limits(),
                    &cancelled,
                )
                .and_then(|root| {
                    let snapshot = root.storage_snapshot()?;
                    let actual = root.shared_scene();
                    let materialized_bytes = if Arc::ptr_eq(&scene, &actual) {
                        0
                    } else {
                        estimate_scene(&actual)?
                    };
                    let f = root.facts()?;
                    let frame = root.frame();
                    if !fits_zoom(
                        frame.width,
                        frame.height,
                        scene_bytes,
                        f.prepared_capacity_bytes
                            .saturating_add(f.debt_capacity_bytes),
                        materialized_bytes,
                        budget,
                    ) {
                        return Err(error("full-zoom cache payload exceeds retention budget"));
                    }
                    Ok(Prepared {
                        root,
                        snapshot,
                        materialized_bytes,
                        prepared_bytes: f
                            .prepared_capacity_bytes
                            .saturating_add(f.debt_capacity_bytes),
                    })
                });
                Completed::Prepared(result)
            }
            JobKind::Certify {
                root,
                region,
                validity,
            } => {
                let mut root = root.lock().unwrap_or_else(|p| p.into_inner());
                // Speculative containment is read-only: wider refusal never consumes the
                // root-lifetime restoration or changes numerical permission for the actual view.
                let envelope = certificate_envelope(region, root.frame());
                let result = root
                    .certify_without_retry(envelope, &cancelled)
                    .or_else(|_| root.certify_without_retry(region, &cancelled))
                    .or_else(|_| {
                        validity.store(0, Ordering::Release);
                        root.certify(region, &cancelled)
                    });
                let generation = root.generation();
                let prepared_bytes = root
                    .facts()
                    .map(|f| {
                        f.prepared_capacity_bytes
                            .saturating_add(f.debt_capacity_bytes)
                    })
                    .unwrap_or(0);
                Completed::Certified {
                    region,
                    result,
                    generation,
                    prepared_bytes,
                    snapshot: root.storage_snapshot(),
                }
            }
            JobKind::Upload {
                id,
                region,
                generation,
                pixels,
            } => {
                let bytes = (region.width as usize)
                    .saturating_mul(region.height as usize)
                    .saturating_mul(4);
                self.inflight.store(bytes, Ordering::Release);
                let result = if cancelled() {
                    Err(RenderError::new(
                        "evaluation.cancelled",
                        "preview cancelled",
                    ))
                } else {
                    copy_pixels(region, &pixels)
                };
                Completed::Uploaded {
                    id,
                    region,
                    generation,
                    result,
                }
            }
            JobKind::Paint {
                root,
                snapshot,
                region,
            } => {
                let result = snapshot
                    .estimated_paint_payload(region, &cancelled)
                    .and_then(|bytes| {
                        self.inflight.store(bytes, Ordering::Release);
                        snapshot.paint_for_storage(region, &cancelled)
                    })
                    .and_then(|tile| {
                        tile.into_upload(&root.lock().unwrap_or_else(|p| p.into_inner()))
                    });
                Completed::Painted {
                    region,
                    generation: snapshot.generation(),
                    result,
                }
            }
        };
        self.inflight.store(0, Ordering::Release);
        Completion {
            artwork: self.artwork,
            key: self.key,
            kind,
            authority: self.cancelled.clone(),
        }
    }
}

/// Expands a required view by half its extent on each side, bounded by the unchanged root grid.
/// This is speculative certificate reuse only: refusal falls back to the required region and never
/// authorizes pixels, consumes a restoration attempt, allocates pixels, or changes the error budget.
fn certificate_envelope(region: Region, frame: Frame) -> Region {
    let x = region.x.saturating_sub(region.width / 2);
    let y = region.y.saturating_sub(region.height / 2);
    let right = region
        .x
        .saturating_add(region.width)
        .saturating_add(region.width / 2)
        .min(frame.width);
    let bottom = region
        .y
        .saturating_add(region.height)
        .saturating_add(region.height / 2)
        .min(frame.height);
    Region {
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}

/// Estimates shared canonical payload once using public vector capacities and conservative private
/// path/preparation storage factors. This is retention accounting, not a portable allocator/RSS cap.
/// # Errors
/// Returns overflow rather than silently wrapping a memory estimate.
pub fn estimate_scene(scene: &RenderScene) -> Result<usize, RenderError> {
    use std::{collections::HashSet, mem::size_of};
    use toniator_patterns::{CanonicalMark, CurveSegment};
    use toniator_render::GeometryOutputPart;
    let mut total = size_of::<RenderScene>();
    let mut geometry = HashSet::new();
    let mut prototypes = HashSet::new();
    let mut prepared = HashSet::new();
    let mut add = |bytes: usize| -> Result<(), RenderError> {
        total = total
            .checked_add(bytes)
            .ok_or(error("scene storage estimate overflows"))?;
        Ok(())
    };
    for layer in scene.layers() {
        add(size_of::<toniator_render::RenderLayer>())?;
        for output in layer.outputs() {
            add(size_of::<toniator_render::RenderOutputLayer>())?;
            if let Some(paints) = output.primitive_paints() {
                add(paints
                    .len()
                    .checked_mul(size_of::<toniator_domain::ColorValue>() * 2)
                    .ok_or(error("paint estimate overflows"))?)?;
            }
            if !geometry.insert(Arc::as_ptr(&output.geometry) as usize) {
                continue;
            }
            for part in output.geometry().parts() {
                match part {
                    GeometryOutputPart::CircularMarks(v) => add(std::mem::size_of_val(v) * 2)?,
                    GeometryOutputPart::CanonicalMarks(v) => {
                        add(std::mem::size_of_val(v) * 2)?;
                        for mark in v {
                            match mark {
                                CanonicalMark::Circle { .. } => {}
                                CanonicalMark::ClosedPath(p) => {
                                    add(std::mem::size_of_val(p.path.segments()) * 2)?
                                }
                                CanonicalMark::DeferredPath(p) => {
                                    if prototypes.insert(Arc::as_ptr(&p.prototype) as usize) {
                                        add(std::mem::size_of_val(p.prototype.path().segments())
                                            * 2
                                            + std::mem::size_of_val(p.as_ref()))?;
                                    }
                                }
                            }
                        }
                    }
                    GeometryOutputPart::CanonicalStrokes(v) => {
                        add(std::mem::size_of_val(v) * 2)?;
                        for stroke in v {
                            add(std::mem::size_of_val(stroke.path.segments()) * 2)?;
                            add(stroke.profile.capacity()
                                * size_of::<toniator_patterns::StrokeProfileSample>())?;
                            add(std::mem::size_of_val(stroke.outline.contours.as_slice()) * 2)?;
                            for c in &stroke.outline.contours {
                                add(c.segments.capacity() * size_of::<CurveSegment>())?;
                            }
                            if let Some(p) = &stroke.outline.prepared
                                && prepared.insert(Arc::as_ptr(p) as usize)
                            {
                                add(stroke
                                    .outline
                                    .full_segment_count
                                    .unwrap_or(0)
                                    .checked_mul(256)
                                    .ok_or(error("prepared estimate overflows"))?)?;
                            }
                        }
                    }
                    GeometryOutputPart::CanonicalRegions(v) => {
                        for region in v.regions() {
                            add(std::mem::size_of_val(region)
                                + std::mem::size_of_val(region.ring.segments()) * 2)?;
                        }
                    }
                }
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_domain::{CanvasSpec, ChannelId, ColorValue, PatternMechanismId};
    use toniator_patterns::{
        CanonicalFillRule, CanonicalMark, FamilySiteId, FamilySiteProvenance, Point2, SiteScope,
    };
    use toniator_render::{GeometryOutput, RenderLayer};
    /// Builds deterministic finite canonical geometry, bypassing no accepted-scene cache authority.
    /// # Panics
    /// Panics if the current ordinary scene constructor rejects this finite circle witness.
    fn scene(name: &str, size: f64) -> Arc<RenderScene> {
        Arc::new(
            RenderScene::new(
                CanvasSpec {
                    width: size,
                    height: size,
                },
                name.into(),
                name.into(),
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
                        GeometryOutput::CanonicalMarks(vec![CanonicalMark::Circle {
                            source_site_id: FamilySiteId {
                                mechanism_id: PatternMechanismId(1),
                                ordinal: 0,
                            },
                            center: Point2::new(4., 4.),
                            radius: 2.,
                            scope: SiteScope::Canvas,
                            provenance: FamilySiteProvenance::Random {
                                candidate_ordinal: 0,
                                accepted_ordinal: 0,
                                exclusion_neighbor_ordinal: None,
                            },
                            fill_rule: CanonicalFillRule::NonZero,
                        }]),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        )
    }
    /// Drives only checked-out cache work; never creates an evaluation/source or scheduler request.
    /// # Panics
    /// Panics if a healthy bounded region does not settle after its finite job list.
    fn settle(cache: &mut PreviewCache, key: RootKey, region: Region) -> View {
        for _ in 0..100 {
            let plan = cache.plan((1, 2, 3), key, region).unwrap();
            if let Plan::Ready(view) = plan {
                if view.complete {
                    return view;
                }
            }
            let mut ran = false;
            while let Some(job) = cache.take_job() {
                cache.complete(job.run(&AtomicBool::new(false)));
                ran = true;
            }
            assert!(ran);
        }
        panic!("cache failed to settle")
    }
    /// Completes every fixed tile autonomously after one plan and never repaints a completed zoom.
    /// # Panics
    /// Panics on incomplete root storage, duplicate cells, unstable grid placement or warm work.
    #[test]
    fn full_zoom_autonomous_completion_and_warm_return() {
        let mut cache = PreviewCache::default();
        cache.accept_scene((1, 2, 3), scene("whole", f64::from(TILE + 1)), 0);
        let key = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        let first = Region {
            x: TILE - 4,
            y: 0,
            width: 5,
            height: 8,
        };
        cache.plan((1, 2, 3), key, first).unwrap();
        let mut painted = std::collections::HashSet::new();
        while let Some(job) = cache.take_job() {
            if let JobKind::Paint { region, .. } = &job.kind {
                assert!(painted.insert((region.x, region.y)));
            }
            cache.complete(job.run(&AtomicBool::new(false)));
        }
        assert_eq!(painted.len(), 4);
        assert_eq!(
            cache.statistics().painted_pixels,
            u64::from(TILE + 1).pow(2)
        );
        assert_eq!(cache.statistics().completed_zooms, 1);
        let before = cache.statistics();
        settle(&mut cache, key, first);
        let other = Region {
            x: 0,
            y: TILE,
            width: 1,
            height: 1,
        };
        settle(&mut cache, key, other);
        settle(&mut cache, key, first);
        assert_eq!(cache.statistics().paint_jobs, before.paint_jobs);
        assert_eq!(cache.statistics().root_preparations, 1);
        println!(
            "autonomous_fixed_tiles={} painted_pixels={} warm_paint_delta=0",
            painted.len(),
            before.painted_pixels
        );
    }
    /// Retains running and remaining fixed-grid demand while repeated pans coalesce view queries.
    /// Priority bursts are bounded so continuously changing view demand cannot starve pixels.
    /// # Panics
    /// Panics on discarded demand, duplicate paint or certification-only starvation.
    #[test]
    fn sustained_pan_preserves_full_zoom_and_visible_priority() {
        let mut cache = PreviewCache::default();
        cache.accept_scene((1, 2, 3), scene("drag", f64::from(TILE * 2 + 4)), 0);
        let key = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        let mut view = Region {
            x: TILE + 2,
            y: TILE + 2,
            width: 1,
            height: 1,
        };
        cache.plan((1, 2, 3), key, view).unwrap();
        let job = cache.take_job().unwrap();
        cache.complete(job.run(&AtomicBool::new(false)));
        cache.plan((1, 2, 3), key, view).unwrap();
        let job = cache.take_job().unwrap();
        cache.complete(job.run(&AtomicBool::new(false)));
        let running = cache.take_job().unwrap();
        assert!(
            matches!(&running.kind,JobKind::Paint { region,.. } if region.x==TILE && region.y==TILE)
        );
        for i in 0..64 {
            view = Region {
                x: i,
                y: i,
                width: 1,
                height: 1,
            };
            cache.plan((1, 2, 3), key, view).unwrap();
        }
        assert_eq!(cache.entries[0].inflight.len(), 1);
        cache.complete(running.run(&AtomicBool::new(false)));
        for i in 0..24 {
            view = Region {
                x: i,
                y: i,
                width: 1,
                height: 1,
            };
            cache.plan((1, 2, 3), key, view).unwrap();
            if let Some(job) = cache.take_job() {
                cache.complete(job.run(&AtomicBool::new(false)));
            }
        }
        while let Some(job) = cache.take_job() {
            cache.complete(job.run(&AtomicBool::new(false)));
        }
        assert_eq!(cache.statistics().completed_zooms, 1);
        assert_eq!(cache.statistics().paint_jobs, 9);
        assert!(cache.statistics().certificate_queries < 30);
    }
    /// Keeps both visited zooms rendering through a switch and preserves explicit None epoch aliases.
    /// # Panics
    /// Panics if revision rebinding or a new zoom drops a checked-out preparation or queued root.
    #[test]
    fn zoom_switch_and_none_rebind_preserve_background() {
        let mut c = PreviewCache::default();
        c.accept_scene((1, 2, 3), scene("zoom", 32.), 0);
        let key = |s| {
            RootKey::new(
                s,
                RasterBackground::Transparent,
                RasterizationLimits::default(),
            )
        };
        let r = Region {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
        };
        c.plan((1, 2, 3), key(1.), r).unwrap();
        let running = c.take_job().unwrap();
        c.plan((1, 2, 3), key(2.), r).unwrap();
        assert_eq!(c.entries.len(), 2);
        assert!(c.rebind_unchanged((1, 2, 3), (1, 5, 3)));
        c.complete(running.run(&AtomicBool::new(false)));
        while let Some(j) = c.take_job() {
            c.complete(j.run(&AtomicBool::new(false)));
        }
        assert_eq!(c.statistics().completed_zooms, 2);
        assert_eq!(c.statistics().root_preparations, 2);
        assert_eq!(c.candidate_scales(key(1.)), vec![2.]);
    }
    /// Rejects stale same-tuple scene epochs and revokes previously granted draw authority on clear.
    /// # Panics
    /// Panics if a stale worker can install a replaced scene or pixels survive revoked authority.
    #[test]
    fn scene_epoch_and_generation_guard_old_pixels() {
        let mut c = PreviewCache::default();
        let key = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        let r = Region {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
        };
        c.accept_scene((1, 2, 3), scene("first", 32.), 0);
        c.plan((1, 2, 3), key, r).unwrap();
        let old = c.take_job().unwrap().run(&AtomicBool::new(false));
        c.accept_scene((1, 2, 3), scene("second", 32.), 0);
        c.plan((1, 2, 3), key, r).unwrap();
        c.complete(old);
        assert!(c.entries[0].root.is_none());
        let v = settle(&mut c, key, r);
        assert!(v.is_current());
        c.entries[0].validity.store(0, Ordering::Release);
        assert!(!v.is_current());
        c.entries[0].validity.store(1, Ordering::Release);
        assert!(v.is_current());
        c.clear();
        assert!(!v.is_current());
    }
    /// Retains a coherent active zoom beyond a tiny soft target and evicts whole older completed zooms.
    /// # Panics
    /// Panics on partial-root eviction/repaint churn or ordinary memo activation retaining orphan roots.
    #[test]
    fn memory_pressure_keeps_current_zoom_coherent() {
        let mut c = PreviewCache::default();
        c.accept_scene((1, 2, 3), scene("budget", 32.), 0);
        let key = |s| {
            RootKey::new(
                s,
                RasterBackground::Transparent,
                RasterizationLimits::default(),
            )
        };
        let r = Region {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
        };
        let old = settle(&mut c, key(1.), r);
        assert!(old.complete);
        let before = c.statistics();
        settle(&mut c, key(1.), r);
        assert_eq!(c.statistics().paint_jobs, before.paint_jobs);
        settle(&mut c, key(2.), r);
        c.budget = 1;
        c.evict();
        assert_eq!(c.entries.len(), 1);
        assert_eq!(c.entries[0].key, key(2.));
        assert!(!old.is_current());
        let target = toniator_render::PreviewRasterTarget::new(1, 1).unwrap();
        c.remember_exact((1, 2, 3), target, 12);
        assert!(c.entries.is_empty());
        assert_eq!(c.statistics().charged_texture_bytes, 12);
    }
    /// Assembles neighbor upload bytes on a checked-out worker and preserves hidden RGB exactly.
    /// # Panics
    /// Panics if an upload is synchronous, repeated or different from the certified reference bytes.
    #[test]
    fn upload_copy_is_worker_owned_and_generation_scoped() {
        let mut c = PreviewCache::default();
        c.accept_scene((1, 2, 3), scene("upload", 32.), 0);
        let k = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        let r = Region {
            x: 0,
            y: 0,
            width: 16,
            height: 16,
        };
        let v = settle(&mut c, k, r);
        let id = v.pixels[0].id;
        let expected = v.copy_upload(r).unwrap();
        assert!(c.upload(k, v.generation, id, r).unwrap().is_none());
        let job = c.take_job().unwrap();
        assert!(matches!(job.kind, JobKind::Upload { .. }));
        c.complete(job.run(&AtomicBool::new(false)));
        assert_eq!(c.upload(k, v.generation, id, r).unwrap().unwrap(), expected);
    }
    /// Separates private full-canvas storage from whole-view numerical permission without new budgets.
    /// # Panics
    /// Panics if stored pixels can manufacture a certificate or a failed view prevents background work.
    #[test]
    fn stored_background_is_not_a_display_certificate() {
        let mut c = PreviewCache::default();
        c.accept_scene((1, 2, 3), scene("permission", 32.), 0);
        let k = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        let r = Region {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
        };
        c.plan((1, 2, 3), k, r).unwrap();
        while let Some(j) = c.take_job() {
            c.complete(j.run(&AtomicBool::new(false)));
        }
        assert_eq!(c.statistics().completed_zooms, 1);
        assert!(c.entries[0].certificates.is_empty());
        assert!(matches!(c.plan((1, 2, 3), k, r).unwrap(), Plan::Waiting));
        let j = c.take_job().unwrap();
        assert!(matches!(j.kind, JobKind::Certify { .. }));
        c.complete(j.run(&AtomicBool::new(false)));
        assert!(matches!(c.plan((1,2,3),k,r).unwrap(),Plan::Ready(v) if v.complete));
    }
    /// Reports live completion rather than cumulative work across preparation, clear and refusal.
    /// # Panics
    /// Panics if preparing roots vanish from demand or completed/cleared pixels are miscounted.
    #[test]
    fn background_progress_counts_live_unique_root_pixels() {
        let mut c = PreviewCache::default();
        c.accept_scene((1, 2, 3), scene("progress", 32.), 0);
        let k = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        c.visit_zoom((1, 2, 3), k).unwrap();
        assert_eq!(c.background_progress(), (0, 1024, true));
        while let Some(j) = c.take_job() {
            c.complete(j.run(&AtomicBool::new(false)));
        }
        assert_eq!(c.background_progress(), (1024, 1024, false));
        c.clear();
        assert_eq!(c.background_progress(), (0, 0, false));
    }

    /// Hides terminal background activity without claiming a failed cell has become stored pixels.
    /// # Panics
    /// Panics if a drained noncancelled paint failure leaves progress active or fakes completion.
    #[test]
    fn terminal_paint_failure_retires_background_progress() {
        let mut c = PreviewCache::default();
        c.accept_scene((1, 2, 3), scene("failed-progress", 32.), 0);
        let k = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        c.visit_zoom((1, 2, 3), k).unwrap();
        let prepare = c.take_job().unwrap();
        c.complete(prepare.run(&AtomicBool::new(false)));
        let paint = c.take_job().unwrap();
        let mut result = paint.run(&AtomicBool::new(false));
        if let Completed::Painted { result, .. } = &mut result.kind {
            *result = Err(error("injected paint failure"));
        } else {
            panic!("paint expected")
        }
        assert!(
            result.changes_view(&c),
            "last failed paint wakes termination even without a view"
        );
        c.complete(result);
        while let Some(job) = c.take_job() {
            c.complete(job.run(&AtomicBool::new(false)));
        }
        assert!(!c.queued());
        assert_eq!(c.background_progress(), (0, 1024, false));
    }

    /// Reuses worker-certified containment through moving views without granting full-root authority.
    /// # Panics
    /// Panics if a neighboring required view queues a second scan inside the accepted envelope.
    #[test]
    fn moving_view_reuses_read_only_certificate_envelope() {
        let mut c = PreviewCache::default();
        c.accept_scene((1, 2, 3), scene("envelope", 64.), 0);
        let k = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        let r = Region {
            x: 16,
            y: 16,
            width: 16,
            height: 16,
        };
        let first = settle(&mut c, k, r);
        let next = Region { x: 20, y: 20, ..r };
        assert!(first.certificate.covers(next));
        let facts = c.entries[0]
            .root
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .facts()
            .unwrap();
        let queries = c.statistics().certificate_queries;
        assert!(matches!(
            c.plan((1, 2, 3), k, next).unwrap(),
            Plan::Ready(_)
        ));
        assert_eq!(c.statistics().certificate_queries, queries);
        assert_eq!(c.statistics().root_preparations, 1);
        let after = c.entries[0]
            .root
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .facts()
            .unwrap();
        assert_eq!(
            (
                facts.consumed_edges,
                facts.consumed_boolean_operations,
                facts.generation
            ),
            (
                after.consumed_edges,
                after.consumed_boolean_operations,
                after.generation
            )
        );
        let envelope = certificate_envelope(
            Region {
                x: u32::MAX - 8,
                y: u32::MAX - 8,
                width: 8,
                height: 8,
            },
            Frame {
                width: u32::MAX,
                height: u32::MAX,
                scale: 1.,
                background: RasterBackground::Transparent,
            },
        );
        assert_eq!(
            u64::from(envelope.x) + u64::from(envelope.width),
            u64::from(u32::MAX)
        );
    }

    /// Guarantees older-zoom background progress while current zoom pans across fresh fixed cells.
    /// # Panics
    /// Panics if foreground paint selection starves a visited older root beyond its bounded burst.
    #[test]
    fn moving_foreground_cells_cannot_starve_other_zoom_background() {
        let mut c = PreviewCache::default();
        c.accept_scene((1, 2, 3), scene("fair", f64::from(TILE * 3 + 1)), 0);
        let k = |scale| {
            RootKey::new(
                scale,
                RasterBackground::Transparent,
                RasterizationLimits::default(),
            )
        };
        let r = Region {
            x: 10,
            y: 10,
            width: 1,
            height: 1,
        };
        c.plan((1, 2, 3), k(1.), r).unwrap();
        let j = c.take_job().unwrap();
        c.complete(j.run(&AtomicBool::new(false)));
        c.plan((1, 2, 3), k(2.), r).unwrap();
        let j = c.take_job().unwrap();
        c.complete(j.run(&AtomicBool::new(false)));
        let mut foreground = 0;
        let mut older = 0;
        for i in 0..48 {
            let cell = i % 24;
            let view = Region {
                x: (cell % 4) * TILE + 10,
                y: (cell / 4) * TILE + 10,
                width: 1,
                height: 1,
            };
            c.plan((1, 2, 3), k(2.), view).unwrap();
            let j = c.take_job().unwrap();
            if matches!(&j.kind, JobKind::Paint { .. }) {
                if j.key == k(1.) {
                    older += 1;
                    foreground = 0;
                } else {
                    foreground += 1;
                }
                assert!(
                    foreground <= 5,
                    "older root starved behind new foreground cells"
                );
            }
            c.complete(j.run(&AtomicBool::new(false)));
        }
        assert!(older >= 2, "no fair older-zoom progress");
    }
    /// Services more than256 sequential warm queries using the real queued-only worker wake policy.
    /// # Panics
    /// Panics if bounded priority accounting overflows while no background remains to reset it.
    #[test]
    fn warm_query_priority_counter_remains_bounded_across_idle_wakes() {
        let mut c = PreviewCache::default();
        c.accept_scene((1, 2, 3), scene("idle", 64.), 0);
        let k = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        settle(
            &mut c,
            k,
            Region {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
        );
        for i in 0..300 {
            c.plan(
                (1, 2, 3),
                k,
                Region {
                    x: i % 31,
                    y: i / 31,
                    width: 1,
                    height: 1,
                },
            )
            .unwrap();
            while c.queued() {
                let j = c.take_job().unwrap();
                c.complete(j.run(&AtomicBool::new(false)));
            }
        }
        assert!(c.statistics().certificate_queries > 256);
        assert_eq!(c.statistics().paint_jobs, 1);
    }
    /// Rejects old painted notifications before consulting a replacement entry's unfinished frame.
    /// # Panics
    /// Panics if a stale same-key completion can crash or refresh the new scene's GTK projection.
    #[test]
    fn stale_paint_notification_cannot_touch_unprepared_replacement() {
        let mut c = PreviewCache::default();
        let k = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        let r = Region {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        c.accept_scene((1, 2, 3), scene("old", 32.), 0);
        c.plan((1, 2, 3), k, r).unwrap();
        let j = c.take_job().unwrap();
        c.complete(j.run(&AtomicBool::new(false)));
        let old = c.take_job().unwrap().run(&AtomicBool::new(false));
        c.accept_scene((1, 2, 3), scene("new", 32.), 0);
        c.plan((1, 2, 3), k, r).unwrap();
        assert!(c.entries[0].frame.is_none());
        assert!(!old.changes_view(&c));
        c.complete(old);
        assert!(c.entries[0].frame.is_none());
    }
    /// Fails closed when worker snapshot allocation fails instead of returning permanently partial Ready.
    /// # Panics
    /// Panics if a missing immutable painter can leave a supposedly supported root stalled forever.
    #[test]
    fn snapshot_allocation_failure_uses_ordinary_fallback() {
        let mut c = PreviewCache::default();
        let k = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        let r = Region {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        c.accept_scene((1, 2, 3), scene("snapshot", 32.), 0);
        c.plan((1, 2, 3), k, r).unwrap();
        let j = c.take_job().unwrap();
        c.complete(j.run(&AtomicBool::new(false)));
        c.plan((1, 2, 3), k, r).unwrap();
        let j = c.take_job().unwrap();
        let mut result = j.run(&AtomicBool::new(false));
        if let Completed::Certified { snapshot, .. } = &mut result.kind {
            *snapshot = Err(error("injected snapshot allocation failure"));
        } else {
            panic!("expected view certification");
        }
        c.complete(result);
        assert!(matches!(
            c.plan((1, 2, 3), k, r).unwrap(),
            Plan::Unavailable
        ));
        assert!(!c.queued());
    }
    /// Coalesces obsolete queued/running padding copies without changing canonical rendered pixels.
    /// # Panics
    /// Panics if obsolete buffers publish or changing upload padding causes rasterization.
    #[test]
    fn obsolete_padding_copies_do_not_accumulate_or_repaint() {
        let mut c = PreviewCache::default();
        c.accept_scene((1, 2, 3), scene("padding", 32.), 0);
        let k = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        let r = Region {
            x: 0,
            y: 0,
            width: 16,
            height: 16,
        };
        let v = settle(&mut c, k, r);
        let id = v.pixels[0].id;
        c.upload(k, v.generation, id, r).unwrap();
        let old = c.take_job().unwrap();
        let newer = Region { width: 20, ..r };
        c.upload(k, v.generation, id, newer).unwrap();
        c.complete(old.run(&AtomicBool::new(false)));
        assert!(c.entries[0].uploads.is_empty());
        let j = c.take_job().unwrap();
        c.complete(j.run(&AtomicBool::new(false)));
        assert!(c.upload(k, v.generation, id, newer).unwrap().is_some());
        assert_eq!(c.statistics().paint_jobs, 1);
    }
    /// Latches only an oversized full-zoom cache strategy without attempting giant allocation/preparation.
    /// # Panics
    /// Panics if repeated pans retry refused cache preparation or overflow its footprint arithmetic.
    #[test]
    fn oversized_full_zoom_declines_cache_without_allocating_or_repeating() {
        let mut c = PreviewCache::default();
        c.accept_scene((1, 2, 3), scene("large", 1e7), 1000);
        let k = RootKey::new(
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        );
        for x in 0..100 {
            assert!(matches!(
                c.plan(
                    (1, 2, 3),
                    k,
                    Region {
                        x,
                        y: 0,
                        width: 8,
                        height: 8
                    }
                )
                .unwrap(),
                Plan::Unavailable
            ));
        }
        assert_eq!(c.statistics().root_preparations, 0);
        assert_eq!(c.statistics().retained_cpu_bytes, 0);
        assert_eq!(c.statistics().unsupported_roots, 1);
        assert!(!c.queued());
        assert!(!fits_zoom(u32::MAX, u32::MAX, 0, 0, 0, usize::MAX));
    }
}
