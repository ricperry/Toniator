//! One approximate native preparation on the existing serialized worker, with metadata-only cache state.
use super::*;
impl PreviewCache {
    /// Copies only already published shared native IDs and bounded requested-cell facts; no native object is accessed.
    pub(super) fn diagnostic_fast_cells(
        &self,
        key: RootKey,
        region: Region,
    ) -> Option<DiagnosticCells> {
        let entry = self.fast.entry.as_ref().filter(|e| e.key == key)?;
        let frame = entry.frame?;
        Some(DiagnosticCells {
            key,
            frame,
            generation: entry.generation,
            ready: entry.phase == Phase::Ready,
            cells: diagnostic_regions(region, frame, false)?
                .into_iter()
                .map(|r| {
                    (
                        r,
                        entry.cells.iter().any(|p| p.region == gutter(r, frame)),
                        entry.painting.contains(&gutter(r, frame)),
                    )
                })
                .collect(),
            native_device_root: (entry.phase == Phase::Ready).then_some((entry.device, entry.root)),
            assembly_region: entry.assembled.as_ref().map(|a| a.region),
            upload_ready: entry.assembled.as_ref().is_some_and(|a| a.upload.is_some()),
            exact_memo: None,
        })
    }
}
#[cfg(feature = "fast-preview")]
use toniator_render::fast_preview::{
    FastPreviewDevice, FastPreviewError, FastPreviewLimits, FastPreviewPlan, FastPreviewRoot,
};

/// Approximate completeness for one whole region; it contains no CPU area/error certificate.
#[derive(Clone)]
pub struct FastPermission {
    region: Region,
    pub device: u64,
    pub root: u64,
    pub scene: String,
}
impl FastPermission {
    /// Tests containment within the complete painted/uploaded region only.
    pub fn covers(&self, r: Region) -> bool {
        self.region.x <= r.x
            && self.region.y <= r.y
            && u64::from(self.region.x) + u64::from(self.region.width)
                >= u64::from(r.x) + u64::from(r.width)
            && u64::from(self.region.y) + u64::from(self.region.height)
                >= u64::from(r.y) + u64::from(r.height)
    }
    /// Returns complete region provenance without making a numerical error claim.
    pub fn region(&self) -> Region {
        self.region
    }
}
/// Atomic Fast planning result; partial GPU pixels never become a Ready view.
#[allow(clippy::large_enum_variant)] // A ready projection is returned by value on frequent cache polls.
pub enum FastPlan {
    Unavailable,
    Waiting,
    Ready(View),
    CpuFallback {
        provenance: Provenance,
        reason: FallbackReason,
    },
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Admitting,
    RetiringCpu,
    Preparing,
    Ready,
}
struct Assembly {
    region: Region,
    surface: Arc<RasterSurface>,
    upload: Option<Vec<u8>>,
    id: u64,
}
struct FastEntry {
    key: RootKey,
    phase: Phase,
    frame: Option<Frame>,
    required: usize,
    root: u64,
    device: u64,
    wanted: Region,
    cells: Vec<Pixels>,
    painting: Vec<Region>,
    assembling: Option<Region>,
    assembled: Option<Assembly>,
    validity: Arc<AtomicU64>,
    generation: u64,
}
#[derive(Clone)]
struct Latch {
    intent: FailureIntent,
    reason: FallbackReason,
}
/// Derived presentation intent, queued work and reservation facts; no native GPU object is shared here.
pub(super) struct FastMetadata {
    requested: Quality,
    stamp: Provenance,
    authority: Arc<AtomicBool>,
    entry: Option<FastEntry>,
    queue: VecDeque<(RootKey, FastJob)>,
    latches: Vec<Latch>,
    last_failure: Option<Latch>,
    worker_charge: usize,
    worker_sequence: u64,
    next_generation: u64,
    retire_pending: bool,
}
impl Default for FastMetadata {
    /// Starts Accurate and resource-free; requesting Fast is the only path to adapter work.
    fn default() -> Self {
        Self {
            requested: Quality::Accurate,
            stamp: Provenance::default(),
            authority: Arc::new(AtomicBool::new(false)),
            entry: None,
            queue: VecDeque::new(),
            latches: Vec::new(),
            last_failure: None,
            worker_charge: 0,
            worker_sequence: 0,
            next_generation: 0,
            retire_pending: false,
        }
    }
}
impl FastMetadata {
    /// Returns the current derived pixel namespace.
    pub(super) fn provenance(&self) -> Provenance {
        self.stamp
    }
    /// Shares current presentation cancellation independently of accepted canonical-scene lifetime.
    pub(super) fn authority(&self) -> Arc<AtomicBool> {
        self.authority.clone()
    }
    /// Returns artist-requested presentation precision without changing document authority.
    pub(super) fn requested(&self) -> Quality {
        self.requested
    }
    /// Returns retained worker charge until an actual retirement acknowledgement releases it.
    pub(super) fn charge(&self) -> usize {
        self.worker_charge.max(
            self.entry
                .as_ref()
                .filter(|e| matches!(e.phase, Phase::Preparing | Phase::Ready))
                .map_or(0, |e| e.required),
        )
    }
    /// Reports only actual queued/lazy demand, excluding failed or incomplete inactive work.
    pub(super) fn queued(&self) -> bool {
        !self.queue.is_empty()
    }
    /// Suppresses CPU background filling while an approximate request owns foreground demand.
    pub(super) fn is_fast_active(&self) -> bool {
        self.requested == Quality::Fast && self.stamp.effective == Quality::Fast
    }
    /// Revokes old upload/draw leases without changing document revision or accepted-scene ownership.
    fn advance(&mut self, effective: Quality) {
        self.authority.store(true, Ordering::Release);
        self.authority = Arc::new(AtomicBool::new(false));
        let epoch = self.stamp.epoch.wrapping_add(1);
        self.stamp = if effective == Quality::Fast {
            Provenance::fast(epoch, self.stamp.device_epoch)
        } else {
            Provenance::accurate(epoch, self.stamp.device_epoch, self.requested)
        };
    }
    /// Revokes old presentation leases when accepted artwork is cleared or replaced.
    pub(super) fn revoke(&mut self) {
        self.authority.store(true, Ordering::Release);
        self.authority = Arc::new(AtomicBool::new(false));
    }
    /// Computes retained Fast pixel/copy payload separately from the overlapping device/root charge.
    pub(super) fn pixels(&self) -> usize {
        self.entry.as_ref().map_or(0, |e| {
            e.cells
                .iter()
                .map(|p| p.surface.pixels().len())
                .sum::<usize>()
                .saturating_add(e.assembled.as_ref().map_or(0, |a| {
                    a.surface
                        .pixels()
                        .len()
                        .saturating_add(a.upload.as_ref().map_or(0, Vec::capacity))
                }))
        })
    }
}
/// Pure worker commands carry immutable scene/IDs/budgets, never Device or native Scene objects.
#[cfg_attr(not(feature = "fast-preview"), allow(dead_code))]
pub(super) enum FastJob {
    Admit {
        scene: Arc<RenderScene>,
        available: usize,
    },
    Prepare,
    Paint {
        region: Region,
    },
    Assemble {
        region: Region,
        pixels: Vec<Pixels>,
    },
    Retire {
        keep_device: bool,
    },
}
struct Facts {
    frame: Frame,
    required: usize,
    device: u64,
    root: u64,
    initialized: bool,
}
#[cfg_attr(not(feature = "fast-preview"), allow(dead_code))]
enum Outcome {
    Admitted {
        frame: Frame,
        required: usize,
    },
    Prepared(Facts),
    Painted {
        region: Region,
        surface: RasterSurface,
        device: u64,
        root: u64,
    },
    Assembled {
        region: Region,
        surface: RasterSurface,
        upload: Vec<u8>,
    },
    Retired,
    Failed(FallbackReason),
}
/// Owned facts/pixels/errors; its charge is reconciled even when its presentation has gone stale.
pub(super) struct FastCompleted {
    sequence: u64,
    charge: usize,
    outcome: Outcome,
}
impl FastCompleted {
    /// Refuses accidental Fast execution through the legacy stateless CPU worker API.
    pub(super) fn missing_worker() -> Self {
        Self {
            sequence: 0,
            charge: 0,
            outcome: Outcome::Failed(FallbackReason::Unavailable(
                "persistent preview worker is unavailable".into(),
            )),
        }
    }
}
/// Worker-only final retirement acknowledgement; no pixel publication permission is carried.
pub struct WorkerRetirement {
    sequence: u64,
}
/// Owns exactly one worker-local GPU context/root and one admitted pending plan across idle wakes.
#[derive(Default)]
pub struct WorkerState {
    sequence: u64,
    #[cfg(feature = "fast-preview")]
    device: Option<FastPreviewDevice>,
    #[cfg(feature = "fast-preview")]
    root: Option<(RootKey, FastPreviewRoot)>,
    #[cfg(feature = "fast-preview")]
    plan: Option<(RootKey, FastPreviewPlan)>,
    #[cfg(feature = "fast-preview")]
    attempted_epoch: Option<u64>,
    #[cfg(all(test, feature = "fast-preview"))]
    unavailable_probe: Option<Arc<AtomicUsize>>,
    #[cfg(all(test, feature = "fast-preview"))]
    initialization_gate: Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>,
}
impl WorkerState {
    /// Executes one job outside cache/GTK locks while retaining native state on this worker only.
    /// Cancellation after submit retires reuse; shutdown drops all native objects on this thread.
    pub fn run(&mut self, job: Job, shutdown: &AtomicBool) -> Completion {
        if !matches!(job.kind, JobKind::Fast(_)) {
            return job.run(shutdown);
        }
        let Job {
            artwork,
            key,
            cancelled,
            presentation,
            inflight,
            kind,
        } = job;
        let JobKind::Fast(work) = kind else {
            unreachable!("matched Fast job")
        };
        let cancel = || {
            shutdown.load(Ordering::Acquire)
                || cancelled.load(Ordering::Acquire)
                || presentation.load(Ordering::Acquire)
        };
        let outcome = self.execute(key, work, &cancel, &inflight);
        // Completion-owned output remains charged until cache reconciliation under its mutex.
        self.sequence = self.sequence.wrapping_add(1);
        Completion {
            artwork,
            key,
            authority: cancelled,
            presentation,
            kind: Completed::Fast(FastCompleted {
                sequence: self.sequence,
                charge: self.charge(),
                outcome,
            }),
        }
    }
    /// Drops all native objects/plans on the worker outside metadata/GTK locks at shutdown.
    pub fn finish(&mut self) -> WorkerRetirement {
        #[cfg(feature = "fast-preview")]
        {
            self.root = None;
            self.plan = None;
            self.device = None;
        }
        self.sequence = self.sequence.wrapping_add(1);
        WorkerRetirement {
            sequence: self.sequence,
        }
    }
    /// Reports one whole native consumer charge; root snapshots overlap and are never added.
    fn charge(&self) -> usize {
        #[cfg(feature = "fast-preview")]
        {
            self.device.as_ref().map_or(0, |d| {
                usize::try_from(d.reservation().total_bytes()).unwrap_or(usize::MAX)
            })
        }
        #[cfg(not(feature = "fast-preview"))]
        {
            0
        }
    }
    /// Performs explicit retirement and owned assembly even when a queued presentation is obsolete.
    /// All other native work checks the independent scene/presentation cancellation boundary.
    fn execute(
        &mut self,
        key: RootKey,
        work: FastJob,
        cancelled: &(dyn Fn() -> bool + Sync),
        inflight: &AtomicUsize,
    ) -> Outcome {
        if let FastJob::Retire { keep_device } = work {
            #[cfg(feature = "fast-preview")]
            {
                self.root = None;
                self.plan = None;
                if !keep_device {
                    self.device = None;
                }
            }
            #[cfg(not(feature = "fast-preview"))]
            let _ = keep_device;
            return Outcome::Retired;
        }
        if cancelled() {
            return Outcome::Failed(FallbackReason::Cancelled);
        }
        if let FastJob::Assemble { region, pixels } = work {
            inflight.store(
                region.width as usize * region.height as usize * 8,
                Ordering::Release,
            );
            return match copy_pixels(region, &pixels).and_then(|bytes| {
                let mut upload = Vec::new();
                upload
                    .try_reserve_exact(bytes.len())
                    .map_err(|_| error("Fast upload allocation failed"))?;
                upload.extend_from_slice(&bytes);
                Ok((
                    RasterSurface::new(region.width, region.height, bytes)?,
                    upload,
                ))
            }) {
                Ok((surface, upload)) if !cancelled() => Outcome::Assembled {
                    region,
                    surface,
                    upload,
                },
                Ok(_) => Outcome::Failed(FallbackReason::Cancelled),
                Err(e) => Outcome::Failed(FallbackReason::Resource(e.to_string())),
            };
        }
        #[cfg(not(feature = "fast-preview"))]
        {
            let _ = (key, work);
            Outcome::Failed(FallbackReason::Unavailable(
                "Fast preview support is absent from this build".into(),
            ))
        }
        #[cfg(feature = "fast-preview")]
        {
            let result = (|| -> Result<Outcome, FastPreviewError> {
                match work {
                    FastJob::Admit { scene, available } => {
                        let limits = FastPreviewLimits {
                            available_payload_bytes: available as u64,
                            ..Default::default()
                        };
                        let plan = match key.projection {
                            Projection::Natural => FastPreviewPlan::new(
                                scene,
                                key.scale(),
                                key.backing(),
                                limits,
                                cancelled,
                            )?,
                            Projection::Target(target) => {
                                FastPreviewPlan::for_target(scene, target, limits, cancelled)?
                            }
                        };
                        let reservation = match &self.device {
                            Some(d) => d.reservation_for_plan(&plan)?,
                            None => plan.reservation(),
                        };
                        let frame = plan.frame();
                        self.plan = Some((key, plan));
                        Ok(Outcome::Admitted {
                            frame,
                            required: reservation.total_bytes() as usize,
                        })
                    }
                    FastJob::Prepare => {
                        let (planned, plan) =
                            self.plan.take().ok_or(FastPreviewError::Cancelled)?;
                        if planned != key {
                            return Err(FastPreviewError::Cancelled);
                        }
                        self.root = None; // one native root; release BEFORE replacement allocation.
                        let mut initialized = false;
                        if self.device.is_none() {
                            if self.attempted_epoch == Some(key.provenance.device_epoch) {
                                return Err(FastPreviewError::Unavailable(
                                    "adapter attempt is latched for this retry epoch".into(),
                                ));
                            }
                            self.attempted_epoch = Some(key.provenance.device_epoch);
                            #[cfg(test)]
                            if let Some(probe) = &self.unavailable_probe {
                                probe.fetch_add(1, Ordering::Relaxed);
                                if let Some((entered, resume)) = self.initialization_gate.take() {
                                    let _ = entered.send(());
                                    let _ = resume.recv();
                                }
                                if cancelled() {
                                    self.attempted_epoch = None;
                                    return Err(FastPreviewError::Cancelled);
                                }
                                return Err(FastPreviewError::Unavailable(
                                    "deterministic adapter unavailable".into(),
                                ));
                            }
                            self.device = Some(FastPreviewDevice::new(&plan, cancelled).map_err(
                                |error| match error {
                                    FastPreviewError::Cancelled => {
                                        self.attempted_epoch = None;
                                        FastPreviewError::Cancelled
                                    }
                                    other => FastPreviewError::Unavailable(other.to_string()),
                                },
                            )?);
                            initialized = true;
                        }
                        let device = self.device.as_mut().expect("initialized worker device");
                        let root = device.prepare(plan, cancelled)?;
                        let frame = root.frame();
                        // Native IDs come from actual complete pixels; obtain them on first paint.
                        let required = device.reservation().total_bytes() as usize;
                        self.root = Some((key, root));
                        Ok(Outcome::Prepared(Facts {
                            frame,
                            required,
                            device: 0,
                            root: 0,
                            initialized,
                        }))
                    }
                    FastJob::Paint { region } => {
                        let (prepared, root) =
                            self.root.as_ref().ok_or(FastPreviewError::Cancelled)?;
                        if *prepared != key {
                            return Err(FastPreviewError::Cancelled);
                        }
                        let output = self
                            .device
                            .as_mut()
                            .ok_or(FastPreviewError::Cancelled)?
                            .render_region(root, region, cancelled)?;
                        Ok(Outcome::Painted {
                            region,
                            surface: output.surface,
                            device: output.provenance.device_id,
                            root: output.provenance.root_id,
                        })
                    }
                    FastJob::Retire { .. } | FastJob::Assemble { .. } => {
                        unreachable!("handled worker-neutral commands")
                    }
                }
            })();
            match result {
                Ok(outcome) => outcome,
                Err(e) => Outcome::Failed(reason(e)),
            }
        }
    }
}
#[cfg(feature = "fast-preview")]
/// Classifies failure retry authority rather than treating a partial GPU error as terminal preview failure.
fn reason(error: FastPreviewError) -> FallbackReason {
    match error {
        FastPreviewError::UnsupportedScene(s) => FallbackReason::Unsupported(s.into()),
        FastPreviewError::Limits(s) => FallbackReason::Resource(s.into()),
        FastPreviewError::Allocation => FallbackReason::Resource("allocation refused".into()),
        FastPreviewError::Unavailable(s) => FallbackReason::Unavailable(s),
        FastPreviewError::Cancelled => FallbackReason::Cancelled,
        other => FallbackReason::Device(other.to_string()),
    }
}

impl PreviewCache {
    /// Applies actual worker shutdown only after all GPU resources have dropped outside the mutex.
    pub fn worker_retired(&mut self, ack: WorkerRetirement) {
        if ack.sequence >= self.fast.worker_sequence {
            self.fast.worker_sequence = ack.sequence;
            self.fast.worker_charge = 0;
            self.fast.retire_pending = false;
        }
    }
    /// Returns current requested/effective backend identity for intent and exact texture keys.
    pub fn provenance(&self) -> Provenance {
        self.fast.provenance()
    }
    /// Returns the session-only requested mode without initializing a backend.
    pub fn requested_quality(&self) -> Quality {
        self.fast.requested()
    }
    /// Changes derived presentation epoch, preserving accepted canonical scene and reusable CPU roots.
    /// OFF→ON grants one new device attempt; repeated equal requests are inert.
    pub fn set_quality(&mut self, quality: Quality) -> bool {
        if self.fast.requested == quality {
            return false;
        }
        self.fast.requested = quality;
        if quality == Quality::Fast {
            self.fast.stamp.device_epoch = self.fast.stamp.device_epoch.wrapping_add(1);
        }
        self.retire_fast();
        self.fast.advance(quality);
        // Only queued work is abandoned. Checked-out CPU mutation must reconcile before
        // a replacement can reuse its root; retaining its pending flag prevents duplicates.
        for job in self.queue.drain(..) {
            if let Some(e) = self.entries.iter_mut().find(|e| e.key == job.key) {
                match job.kind {
                    JobKind::Prepare { .. } => e.preparing = false,
                    JobKind::Certify { .. } => e.querying = false,
                    JobKind::Paint { region, .. } => e.inflight.retain(|r| *r != region),
                    JobKind::Upload { id, region, .. } => {
                        e.uploading.retain(|u| *u != (id, region))
                    }
                    _ => {}
                }
            }
        }
        self.active = None;
        if quality == Quality::Accurate {
            self.rekey_cpu();
        }
        true
    }
    /// Schedules actual worker retirement even when no scene or paint has ever been accepted.
    pub(super) fn retire_fast(&mut self) {
        let needed = self.fast.requested == Quality::Fast
            || self.fast.worker_charge != 0
            || self.fast.entry.is_some()
            || !self.fast.queue.is_empty();
        if let Some(entry) = self.fast.entry.take() {
            entry.validity.store(0, Ordering::Release);
        }
        self.fast.queue.clear();
        if !needed {
            return;
        }
        self.fast.retire_pending = true;
        let key = RootKey::new(
            1.0,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
        )
        .with_provenance(self.fast.stamp);
        self.fast
            .queue
            .push_back((key, FastJob::Retire { keep_device: false }));
    }
    /// Retires document-owned native roots/plans on Close while retaining a successful window device.
    /// No closed canonical Arc survives acknowledgement; the living device stays charged until OFF/drop.
    pub(super) fn retire_fast_root(&mut self) {
        let keep_device = self.fast.is_fast_active();
        self.retire_fast();
        if keep_device
            && let Some((_, FastJob::Retire { keep_device })) = self.fast.queue.back_mut()
        {
            *keep_device = true;
        }
    }
    /// Rewraps unchanged numerical CPU entries/memos into a fresh presentation namespace explicitly.
    fn rekey_cpu(&mut self) {
        let provenance = self.fast.provenance();
        for e in &mut self.entries {
            e.diagnostic_support = None;
            e.key.provenance = provenance;
            for p in &mut e.pixels {
                p.provenance = provenance;
            }
        }
        for e in &mut self.exact {
            e.provenance = provenance;
        }
    }
    /// Returns the retained typed fallback diagnostic until matching complete Fast pixels publish.
    pub fn fallback_reason(&self) -> Option<(&FailureIntent, &FallbackReason)> {
        self.fast
            .last_failure
            .as_ref()
            .map(|l| (&l.intent, &l.reason))
    }
    /// Records only current complete uploaded Fast acceptance; stale upload callbacks cannot clear errors.
    pub fn publish_fast(&mut self, key: RootKey, generation: u64) -> bool {
        let accepted = self.fast.entry.as_ref().is_some_and(|e| {
            e.key == key
                && e.generation == generation
                && e.assembled.is_some()
                && e.phase == Phase::Ready
        }) && key.provenance == self.fast.stamp
            && !self.fast.authority.load(Ordering::Acquire);
        if accepted {
            self.fast.last_failure = None;
        }
        accepted
    }
    /// Returns all current retained/in-flight charges before admitting an additional owned copy.
    fn fast_payload(&self) -> usize {
        let s = self.statistics();
        s.retained_cpu_bytes
            .saturating_add(s.retained_prepared_bytes)
            .saturating_add(s.retained_materialized_scene_bytes)
            .saturating_add(s.charged_texture_bytes)
            .saturating_add(s.estimated_scene_bytes)
            .saturating_add(s.fast_reserved_bytes)
            .saturating_add(s.retired_root_payload_bytes)
            .saturating_add(s.inflight_estimated_bytes)
    }
    /// Lists qualified resident upload identities so the app cannot collide across backend/epoch changes.
    pub fn resident_identities(&self) -> Vec<PixelIdentity> {
        self.entries
            .iter()
            .flat_map(|e| e.pixels.iter().map(Pixels::identity))
            .chain(self.fast.entry.iter().filter_map(|e| {
                e.assembled.as_ref().map(|a| PixelIdentity {
                    provenance: e.key.provenance,
                    id: a.id,
                })
            }))
            .collect()
    }
    /// Lists qualified ordinary exact-target memo identities for GTK ownership reconciliation.
    pub fn exact_identities(&self) -> Vec<PixelIdentity> {
        self.exact
            .iter()
            .map(|e| PixelIdentity {
                provenance: e.provenance,
                id: e.id,
            })
            .collect()
    }
    /// Reports the current exact-target ordinary texture identity, including effective fallback provenance.
    pub fn exact_identity(
        &mut self,
        artwork: Artwork,
        target: toniator_render::PreviewRasterTarget,
    ) -> Option<PixelIdentity> {
        let id = self.exact(artwork, target)?;
        Some(PixelIdentity {
            provenance: self.provenance(),
            id,
        })
    }
    /// Plans complete Fast demand; unlike Accurate, sparse partial coverage is never display permission.
    /// Admission runs off-thread before CPU eviction, preserving efficient unsupported-scene fallback.
    /// # Errors
    /// Rejects malformed region/metadata allocation; ordinary typed refusal returns CpuFallback.
    pub fn plan_fast(
        &mut self,
        artwork: Artwork,
        mut key: RootKey,
        region: Region,
    ) -> Result<FastPlan, RenderError> {
        if region.width == 0
            || region.height == 0
            || region.x.checked_add(region.width).is_none()
            || region.y.checked_add(region.height).is_none()
        {
            return Err(error("invalid Fast sampled region"));
        }
        if !self.has_artwork(artwork) || self.fast.requested != Quality::Fast {
            return Ok(FastPlan::Unavailable);
        }
        let scene = self
            .scene
            .as_ref()
            .ok_or(error("accepted scene unavailable"))?;
        let identity = scene.identity().scene_fingerprint();
        key.provenance = Provenance::fast(self.fast.stamp.epoch, self.fast.stamp.device_epoch);
        if let Some(reason) = self
            .fast
            .latches
            .iter()
            .find(|l| matches_latch(l, identity, key, self.fast.stamp.device_epoch))
            .map(|l| l.reason.clone())
        {
            self.select_fallback(reason.clone());
            return Ok(FastPlan::CpuFallback {
                provenance: self.fast.stamp,
                reason,
            });
        }
        if self.fast.stamp.effective != Quality::Fast {
            self.fast.advance(Quality::Fast);
        }
        key.provenance = self.fast.stamp;
        if self.fast.entry.as_ref().is_none_or(|e| e.key != key) {
            if let Some(old) = self.fast.entry.take() {
                old.validity.store(0, Ordering::Release);
            }
            self.fast
                .queue
                .retain(|(_, j)| matches!(j, FastJob::Retire { .. }));
            self.fast.next_generation = self.fast.next_generation.wrapping_add(1);
            self.fast.entry = Some(FastEntry {
                key,
                phase: Phase::Admitting,
                frame: None,
                required: 0,
                root: 0,
                device: 0,
                wanted: region,
                cells: Vec::new(),
                painting: Vec::new(),
                assembling: None,
                assembled: None,
                validity: Arc::new(AtomicU64::new(0)),
                generation: self.fast.next_generation,
            });
            let available = self
                .budget
                .saturating_sub(self.scene_estimate)
                .saturating_sub(self.last_good_charge);
            self.fast.queue.push_back((
                key,
                FastJob::Admit {
                    scene: self.scene.as_ref().expect("checked scene").clone(),
                    available,
                },
            ));
            return Ok(FastPlan::Waiting);
        }
        let entry = self.fast.entry.as_mut().expect("matching Fast entry");
        entry.wanted = region;
        if let Some(frame) = entry.frame
            && (u64::from(region.x) + u64::from(region.width) > u64::from(frame.width)
                || u64::from(region.y) + u64::from(region.height) > u64::from(frame.height))
        {
            return Err(error("Fast sampled region exceeds frame"));
        }
        let Some(assembled) = entry
            .assembled
            .as_ref()
            .filter(|a| contains(a.region, region))
        else {
            self.queue_fast_demand();
            return Ok(FastPlan::Waiting);
        };
        let permission = FastPermission {
            region: assembled.region,
            device: entry.device,
            root: entry.root,
            scene: self
                .scene
                .as_ref()
                .expect("accepted scene")
                .identity()
                .scene_fingerprint()
                .into(),
        };
        Ok(FastPlan::Ready(View {
            captured_key: key,
            key,
            generation: entry.generation,
            frame: entry.frame.expect("assembled frame"),
            permission: ViewPermission::Fast(permission),
            artwork,
            presentation_authority: self.fast.authority.clone(),
            pixels: vec![Pixels {
                provenance: key.provenance,
                id: assembled.id,
                region: assembled.region,
                surface: assembled.surface.clone(),
                touch: self.clock,
            }],
            complete: true,
            background_pending: false,
            expected: vec![assembled.region],
            validity: entry.validity.clone(),
            authority: self.cancelled.clone(),
        }))
    }
    /// Transfers the worker-owned complete upload once; no GTK-thread pixel copy is needed.
    pub fn take_fast_upload(
        &mut self,
        key: RootKey,
        id: PixelIdentity,
        region: Region,
    ) -> Option<Vec<u8>> {
        if self
            .fast_payload()
            .saturating_add(region.width as usize * region.height as usize * 8)
            > self.budget
        {
            self.fail_fast(
                key,
                FallbackReason::Resource("Fast upload ownership exceeds shared budget".into()),
            );
            return None;
        }
        let entry = self
            .fast
            .entry
            .as_mut()
            .filter(|e| e.key == key && e.key.provenance == id.provenance)?;
        let assembly = entry
            .assembled
            .as_mut()
            .filter(|a| a.id == id.id && contains(a.region, region))?;
        if let Some(upload) = assembly.upload.take() {
            return Some(upload);
        }
        if entry.assembling.is_none() {
            entry.assembling = Some(assembly.region);
            self.fast.queue.push_back((
                key,
                FastJob::Assemble {
                    region: assembly.region,
                    pixels: entry.cells.clone(),
                },
            ));
        }
        None
    }
    /// Queues only missing fixed cells with full gutters, then one coherent whole-view assembly.
    fn queue_fast_demand(&mut self) {
        if let Some(entry) = self.fast.entry.as_ref().filter(|e| e.phase == Phase::Ready) {
            let frame = entry.frame.expect("ready frame");
            let future = tiles_for_region(entry.wanted, frame)
                .into_iter()
                .map(|r| gutter(r, frame))
                .filter(|r| !entry.cells.iter().any(|p| p.region == *r))
                .fold(0usize, |n, r| {
                    n.saturating_add(r.width as usize * r.height as usize * 4)
                });
            let assembly = entry.wanted.width as usize * entry.wanted.height as usize * 8;
            if self
                .fast_payload()
                .saturating_add(future)
                .saturating_add(assembly)
                > self.budget
            {
                let key = entry.key;
                self.fail_fast(
                    key,
                    FallbackReason::Resource(
                        "complete Fast view/copy exceeds shared payload allowance".into(),
                    ),
                );
                return;
            }
        }
        let Some(entry) = self.fast.entry.as_mut().filter(|e| e.phase == Phase::Ready) else {
            return;
        };
        let frame = entry.frame.expect("prepared frame");
        for interior in tiles_for_region(entry.wanted, frame) {
            let storage = gutter(interior, frame);
            if !entry.cells.iter().any(|p| p.region == storage)
                && !entry.painting.contains(&storage)
            {
                entry.painting.push(storage);
                self.fast
                    .queue
                    .push_back((entry.key, FastJob::Paint { region: storage }));
            }
        }
        if entry.assembling.is_none()
            && entry
                .assembled
                .as_ref()
                .is_none_or(|a| !contains(a.region, entry.wanted))
            && missing(entry.wanted, entry.cells.iter().map(|p| p.region)).is_empty()
        {
            let region = entry.wanted;
            entry.assembling = Some(region);
            self.fast.queue.push_back((
                entry.key,
                FastJob::Assemble {
                    region,
                    pixels: entry
                        .cells
                        .iter()
                        .filter(|p| intersection(p.region, region).is_some())
                        .cloned()
                        .collect(),
                },
            ));
        }
    }
    /// Retires native references before dropping the corresponding retired canonical/CPU payload.
    pub(super) fn take_fast_retirement(&mut self) -> Option<Job> {
        if !self
            .fast
            .queue
            .front()
            .is_some_and(|(_, j)| matches!(j, FastJob::Retire { .. }))
        {
            return None;
        }
        let (key, work) = self.fast.queue.pop_front()?;
        Some(Job {
            artwork: self.artwork.unwrap_or((0, 0, 0)),
            key,
            cancelled: self.cancelled.clone(),
            presentation: self.fast.authority(),
            inflight: self.inflight.clone(),
            kind: JobKind::Fast(work),
        })
    }
    /// Waits for both worker-owned CPU retirement and app-owned texture release acknowledgements.
    pub(super) fn fast_prepare_ready(&self) -> bool {
        self.fast
            .entry
            .as_ref()
            .is_some_and(|e| e.phase == Phase::RetiringCpu)
            && self.retired_inflight == 0
            && self.retired.is_empty()
            && self.texture_charge <= self.last_good_charge
            && self.exact.is_empty()
    }
    /// Checks out Fast work only after CPU payload retirement has actually acknowledged completion.
    pub(super) fn take_fast_job(&mut self) -> Option<Job> {
        if self.fast_prepare_ready() {
            let e = self.fast.entry.as_mut().expect("waiting Fast entry");
            e.phase = Phase::Preparing;
            // Reserve before checkout; removing the entry during cancellation cannot release
            // native allocations that the worker has not yet acknowledged retiring.
            self.fast.worker_charge = self.fast.worker_charge.max(e.required);
            self.fast.queue.push_back((e.key, FastJob::Prepare));
        }
        let (key, work) = self.fast.queue.pop_front()?;
        Some(Job {
            artwork: self.artwork.unwrap_or((0, 0, 0)),
            key,
            cancelled: self.cancelled.clone(),
            presentation: self.fast.authority(),
            inflight: self.inflight.clone(),
            kind: JobKind::Fast(work),
        })
    }
    /// Reconciles resource facts BEFORE stale presentation checks, retaining charges until worker ACK.
    pub(super) fn complete_fast(&mut self, result: Completion) {
        self.inflight.store(0, Ordering::Release);
        let Completed::Fast(completion) = result.kind else {
            return;
        };
        if completion.sequence >= self.fast.worker_sequence {
            self.fast.worker_sequence = completion.sequence;
            self.fast.worker_charge = completion.charge;
        }
        if matches!(&completion.outcome, Outcome::Retired) {
            self.fast.retire_pending = false;
            return;
        }
        if !Arc::ptr_eq(&result.authority, &self.cancelled)
            || !Arc::ptr_eq(&result.presentation, &self.fast.authority)
            || !self
                .artwork
                .is_some_and(|a| a.0 == result.artwork.0 && a.2 == result.artwork.2)
        {
            return;
        }
        let Some(entry) = self.fast.entry.as_mut().filter(|e| e.key == result.key) else {
            return;
        };
        match completion.outcome {
            Outcome::Admitted { frame, required } => {
                if u64::from(entry.wanted.x) + u64::from(entry.wanted.width)
                    > u64::from(frame.width)
                    || u64::from(entry.wanted.y) + u64::from(entry.wanted.height)
                        > u64::from(frame.height)
                {
                    self.fail_fast(
                        result.key,
                        FallbackReason::Resource("sampled Fast demand exceeds native frame".into()),
                    );
                    return;
                }
                entry.frame = Some(frame);
                entry.required = required;
                let other = self.scene_estimate.saturating_add(self.last_good_charge);
                if required.saturating_add(other) > self.budget {
                    self.fail_fast(
                        result.key,
                        FallbackReason::Resource("shared preview payload budget exhausted".into()),
                    );
                    return;
                }
                // Capability/target admission succeeded. Now retire inactive CPU derived roots;
                // their charges persist through the checked-out worker retirement acknowledgement.
                self.queue.clear();
                for e in &mut self.entries {
                    e.diagnostic_support = None;
                    e.validity.store(0, Ordering::Release);
                }
                self.retired.append(&mut self.entries);
                self.exact.clear();
                self.active = None;
                entry.phase = Phase::RetiringCpu;
            }
            Outcome::Prepared(facts) => {
                entry.phase = Phase::Ready;
                entry.frame = Some(facts.frame);
                entry.required = facts.required;
                entry.device = facts.device;
                entry.root = facts.root;
                entry
                    .validity
                    .store(entry.generation.wrapping_add(1), Ordering::Release);
                self.stats.fast_preparations += 1;
                self.stats.fast_device_initializations += u64::from(facts.initialized);
            }
            Outcome::Painted {
                region,
                surface,
                device,
                root,
            } => {
                entry.device = device;
                entry.root = root;
                entry.painting.retain(|r| *r != region);
                self.next_id = self.next_id.wrapping_add(1);
                entry.cells.push(Pixels {
                    provenance: entry.key.provenance,
                    id: self.next_id,
                    region,
                    surface: Arc::new(surface),
                    touch: self.clock,
                });
            }
            Outcome::Assembled {
                region,
                surface,
                upload,
            } => {
                entry.assembling = None;
                self.next_id = self.next_id.wrapping_add(1);
                entry.assembled = Some(Assembly {
                    region,
                    surface: Arc::new(surface),
                    upload: Some(upload),
                    id: self.next_id,
                });
            }
            Outcome::Failed(reason) => {
                self.fail_fast(result.key, reason);
                return;
            }
            Outcome::Retired => {}
        }
        self.queue_fast_demand();
    }
    /// Records failure under its proper retry scope and chooses an atomic Accurate namespace.
    fn fail_fast(&mut self, key: RootKey, reason: FallbackReason) {
        if reason == FallbackReason::Cancelled {
            self.retire_fast();
            return;
        }
        if let Some(scene) = &self.scene {
            let latch = Latch {
                intent: FailureIntent {
                    scene: scene.identity().scene_fingerprint().into(),
                    key,
                },
                reason: reason.clone(),
            };
            self.fast.last_failure = Some(latch.clone());
            self.fast.latches.push(latch);
        }
        self.select_fallback(reason);
    }
    /// Advances fallback epoch once, retains canonical authority and reuses valid CPU roots/memos.
    fn select_fallback(&mut self, _reason: FallbackReason) {
        if self.fast.stamp.effective == Quality::Fast {
            self.fast.advance(Quality::Accurate);
            self.retire_fast();
            self.rekey_cpu();
        }
    }
}
/// Matches content, target or device retry identity without making pans create new attempts.
fn matches_latch(l: &Latch, scene: &str, key: RootKey, device_epoch: u64) -> bool {
    match l.reason {
        FallbackReason::Unsupported(_) => {
            l.intent.scene == scene && l.intent.key.provenance.algorithm == key.provenance.algorithm
        }
        FallbackReason::Resource(_) => {
            l.intent.scene == scene
                && l.intent.key.projection == key.projection
                && l.intent.key.scale == key.scale
                && l.intent.key.background == key.background
                && l.intent.key.edges == key.edges
                && l.intent.key.boolean == key.boolean
                && l.intent.key.provenance.backend == key.provenance.backend
                && l.intent.key.provenance.algorithm == key.provenance.algorithm
        }
        FallbackReason::Unavailable(_) | FallbackReason::Device(_) => {
            l.intent.key.provenance.device_epoch == device_epoch
        }
        FallbackReason::Cancelled => false,
    }
}
/// Bounds half-open whole-region containment with widened endpoints.
fn contains(a: Region, b: Region) -> bool {
    a.x <= b.x
        && a.y <= b.y
        && u64::from(a.x) + u64::from(a.width) >= u64::from(b.x) + u64::from(b.width)
        && u64::from(a.y) + u64::from(a.height) >= u64::from(b.y) + u64::from(b.height)
}
/// Computes retired CPU payload before moving ownership out of the metadata cache.
pub(super) fn retired_payload(entries: &[Entry]) -> usize {
    entries.iter().fold(0usize, |n, e| {
        n.saturating_add(e.prepared_bytes)
            .saturating_add(e.materialized_bytes)
            .saturating_add(
                e.pixels
                    .iter()
                    .map(|p| p.surface.pixels().len())
                    .sum::<usize>(),
            )
            .saturating_add(e.uploads.iter().map(|u| u.2.capacity()).sum::<usize>())
    })
}

#[cfg(test)]
mod tests;
