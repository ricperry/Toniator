//! Cancellable source-frame preparation; document timing and interpolation remain headless authority.

use crate::preview_diagnostics::{Diagnostic, Origin};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use toniator_domain::{Document, DocumentSession};
use toniator_engine::{
    EvaluationRequest, EvaluationScheduler, FrameSource, MediaTools, PreviewRasterTarget,
    SourceFrame,
};
use toniator_io::SourceBundle;

/// The only interactive temporal preview states; no arbitrary-position transport is exposed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Endpoint {
    #[default]
    Start,
    End,
}
impl Endpoint {
    /// Selects Start for a one-frame project, whose End controls are hidden and render uses Start.
    /// Authored End values remain in the document for a later multi-frame interval.
    pub(crate) fn for_document(self, document: &Document) -> Self {
        if document.project_timing().frame_range().frame_count() == 1 {
            Self::Start
        } else {
            self
        }
    }

    /// Resolves the selected endpoint from the current authoritative project range.
    pub(crate) fn frame(self, document: &Document) -> u64 {
        let range = document.project_timing().frame_range();
        match self {
            Self::Start => range.start(),
            Self::End => range.end_exclusive() - 1,
        }
    }
}

/// Rejects decoder results after an endpoint switch, edit, replacement, or superseding request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RequestKey {
    pub(crate) workspace: u64,
    pub(crate) revision: u64,
    pub(crate) epoch: u64,
    pub(crate) frame: u64,
}
impl RequestKey {
    /// Identifies source pixels independently of viewport-only request epochs.
    /// Endpoint changes with an unchanged document revision still produce distinct identities.
    pub(crate) const fn frame_identity(self) -> (u64, u64, u64) {
        (self.workspace, self.revision, self.frame)
    }
    /// Checks every acceptance dimension against current application and document authority.
    pub(crate) fn is_current(
        self,
        pending: Option<Self>,
        workspace: u64,
        session: &DocumentSession,
        endpoint: Endpoint,
    ) -> bool {
        pending == Some(self)
            && self.workspace == workspace
            && self.revision == session.revision().0
            && self.frame == endpoint.frame(session.document())
    }
}

/// Proves same-revision endpoints cannot share fallback pixels while viewport epochs can.
#[cfg(test)]
#[test]
fn preview_frame_identity_distinguishes_endpoint_from_viewport_intent() {
    let start = RequestKey {
        workspace: 4,
        revision: 9,
        epoch: 1,
        frame: 0,
    };
    assert_eq!(
        start.frame_identity(),
        RequestKey { epoch: 2, ..start }.frame_identity()
    );
    assert_ne!(
        start.frame_identity(),
        RequestKey { frame: 10, ..start }.frame_identity()
    );
    assert_ne!(
        start.frame_identity(),
        RequestKey {
            workspace: 5,
            ..start
        }
        .frame_identity()
    );
}

/// A bounded straight-RGBA source comparison image prepared entirely off the GTK thread.
pub(crate) struct SourceDisplay {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
    pub(crate) metadata: serde_json::Value,
}

/// Supplies immutable evaluator input and the matching source comparison image together.
pub(crate) struct PreparedFrame {
    pub(crate) request: EvaluationRequest,
    pub(crate) source: SourceDisplay,
}

/// Carries a generation-keyed decode result back to GTK for acceptance before scheduler submission.
pub(crate) struct Completion {
    pub(crate) key: RequestKey,
    pub(crate) result: Result<PreparedFrame, Diagnostic>,
}

struct Job {
    key: RequestKey,
    session: DocumentSession,
    sources: SourceBundle,
    target: PreviewRasterTarget,
    cancelled: Arc<AtomicBool>,
}

/// Chooses the private GTK destination for one decoded source completion.
///
/// The destination is captured when a worker is created, so a completion cannot
/// be redirected to a newer editor or the main workspace by a later edit.
#[derive(Clone, Copy)]
enum Destination {
    /// Delivers a completion to the main workspace preview route.
    Main,
    /// Delivers a completion to one captured Pattern Editor epoch.
    Draft {
        epoch: u64,
        structural_preview: bool,
    },
}

/// Owns one worker and one replaceable pending request; native decoder iterators never cross threads.
pub(crate) struct Worker {
    pending: Arc<(Mutex<Option<Job>>, Condvar)>,
    shutdown: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    latest: Option<Arc<AtomicBool>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    /// Starts one source worker with bounded latest-request queuing and shared source providers.
    ///
    /// # Errors
    /// Returns the operating-system thread-spawn diagnostic without creating a live worker.
    pub(crate) fn new(sender: async_channel::Sender<crate::AppEvent>) -> Result<Self, Diagnostic> {
        Self::new_with_destination(sender, Destination::Main)
    }

    /// Starts one source worker whose completions are scoped to a private Pattern Editor epoch.
    ///
    /// The worker owns native media-provider state and resolves the requested endpoint through the
    /// shared engine frame boundary. Its source bundle and decoder are independent of the main
    /// preview worker and are reaped when the editor surface is dropped.
    ///
    /// # Errors
    /// Returns the operating-system thread-spawn diagnostic without creating a live worker.
    #[cfg(test)]
    pub(crate) fn new_for_pattern_editor(
        sender: async_channel::Sender<crate::AppEvent>,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        Self::new_with_destination(
            sender,
            Destination::Draft {
                epoch,
                structural_preview: false,
            },
        )
    }

    /// Starts a Pattern Editor worker for a disposable structural preview projection.
    ///
    /// Workspace editors retain actual artwork and response intent while bypassing fit for their
    /// preview. Wizard editors use this purpose after projecting their child into a neutral response
    /// document.
    ///
    /// # Errors
    /// Returns the operating-system thread-spawn diagnostic without creating a live worker.
    pub(crate) fn new_for_structural_pattern_editor(
        sender: async_channel::Sender<crate::AppEvent>,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        Self::new_with_destination(
            sender,
            Destination::Draft {
                epoch,
                structural_preview: true,
            },
        )
    }

    /// Starts one source worker for an explicit completion destination.
    ///
    /// The worker keeps at most one pending job and reuses a provider only while the exact source
    /// bundle remains unchanged. Every evaluator request carries the captured decoded frame rather
    /// than reinterpreting moving-media bytes as a still image.
    ///
    /// # Errors
    /// Returns the operating-system thread-spawn diagnostic without creating a live worker.
    fn new_with_destination(
        sender: async_channel::Sender<crate::AppEvent>,
        destination: Destination,
    ) -> Result<Self, Diagnostic> {
        let pending = Arc::new((Mutex::new(None::<Job>), Condvar::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let jobs = Arc::clone(&pending);
        let stop = Arc::clone(&shutdown);
        let alive = Arc::new(AtomicBool::new(true));
        let worker_alive = alive.clone();
        let thread = std::thread::Builder::new()
            .name("toniator-preview-media".into())
            .spawn(move || {
                let mut cached_sources: Option<SourceBundle> = None;
                let mut provider = None;
                loop {
                    let job = {
                        let mut slot = jobs.0.lock().unwrap_or_else(|error| error.into_inner());
                        while slot.is_none() && !stop.load(Ordering::Acquire) {
                            slot = jobs.1.wait(slot).unwrap_or_else(|error| error.into_inner());
                        }
                        if stop.load(Ordering::Acquire) {
                            break;
                        }
                        slot.take().expect("pending job")
                    };
                    let mut guard = SourceJobGuard {
                        sender: sender.clone(),
                        destination,
                        key: job.key,
                        stop: stop.clone(),
                        cancelled: job.cancelled.clone(),
                        completed: false,
                        pending: jobs.clone(),
                        alive: worker_alive.clone(),
                    };
                    let cancelled =
                        || stop.load(Ordering::Acquire) || job.cancelled.load(Ordering::Acquire);
                    if cancelled() {
                        continue;
                    }
                    let result = (|| {
                        if cached_sources.as_ref() != Some(&job.sources) {
                            provider = None;
                            cached_sources = None;
                            provider = Some(
                                toniator_engine::open_source_media(
                                    &job.sources,
                                    MediaTools::default(),
                                    &cancelled,
                                )
                                .map_err(|error| {
                                    Diagnostic::new(Origin::Source, error.path(), error.message())
                                })?,
                            );
                            cached_sources = Some(job.sources.clone());
                        }
                        let media = provider.as_mut().ok_or_else(|| {
                            Diagnostic::new(
                                Origin::Source,
                                "preview.source_worker.unavailable",
                                "Source provider is unavailable",
                            )
                        })?;
                        let time = job
                            .session
                            .document()
                            .project_timing()
                            .source_time_for_frame(job.key.frame)
                            .map_err(|error| {
                                Diagnostic::new(Origin::Source, error.path(), error.message())
                            })?;
                        let frame = media.frame_at(time, &cancelled).map_err(|error| {
                            Diagnostic::new(Origin::Source, error.path(), error.message())
                        })?;
                        let source = source_display(&frame, &cancelled)?;
                        let request = toniator_engine::frame_evaluation_request(
                            &job.session,
                            media,
                            job.key.frame,
                            &cancelled,
                        )
                        .map_err(|error| {
                            Diagnostic::new(Origin::Source, error.path(), error.message())
                        })?
                        .for_preview(job.target);
                        let request = match destination {
                            Destination::Draft {
                                structural_preview: true,
                                ..
                            } => request.for_structural_preview(),
                            Destination::Main
                            | Destination::Draft {
                                structural_preview: false,
                                ..
                            } => request,
                        };
                        Ok(PreparedFrame { request, source })
                    })();
                    guard.completed = true;
                    if !cancelled() {
                        let completion = Completion {
                            key: job.key,
                            result,
                        };
                        let event = match destination {
                            Destination::Main => crate::AppEvent::MediaPreview(completion),
                            Destination::Draft { epoch, .. } => {
                                crate::AppEvent::DraftMediaPreview { epoch, completion }
                            }
                        };
                        let _ = sender.send_blocking(event);
                    }
                }
            })
            .map_err(|error| {
                Diagnostic::new(
                    Origin::Source,
                    "preview.source_worker.spawn",
                    &error.to_string(),
                )
            })?;
        Ok(Self {
            pending,
            shutdown,
            alive,
            latest: None,
            thread: Some(thread),
            // The completion destination is captured by the worker closure. This field is kept
            // out of the mutable worker state so submit/cancel cannot retarget in-flight work.
        })
    }

    /// Replaces pending decode work and cooperatively stops its predecessor without blocking GTK.
    /// The queue lock serializes worker-death publication and admission of the latest source key.
    /// # Errors
    /// Returns a typed unavailable-worker diagnostic instead of stranding a job on a finished thread.
    pub(crate) fn submit(
        &mut self,
        key: RequestKey,
        session: DocumentSession,
        sources: SourceBundle,
        target: PreviewRasterTarget,
    ) -> Result<(), Diagnostic> {
        if self
            .thread
            .as_ref()
            .is_none_or(|thread| thread.is_finished())
        {
            return Err(Diagnostic::new(
                Origin::Source,
                "preview.source_worker.unavailable",
                "Source preview worker stopped",
            ));
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut slot = self
            .pending
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !self.alive.load(Ordering::Acquire) {
            return Err(Diagnostic::new(
                Origin::Source,
                "preview.source_worker.unavailable",
                "Source preview worker stopped",
            ));
        }
        if let Some(previous) = self.latest.take() {
            previous.store(true, Ordering::Release);
        }
        self.latest = Some(cancelled.clone());
        *slot = Some(Job {
            key,
            session,
            sources,
            target,
            cancelled,
        });
        drop(slot);
        self.pending.1.notify_one();
        Ok(())
    }

    /// Cancels running/pending source work; the worker keeps only its current reusable provider.
    pub(crate) fn cancel(&mut self) {
        if let Some(cancelled) = self.latest.take() {
            cancelled.store(true, Ordering::Release);
        }
        self.pending
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }
}

/// Notifies only unexpected in-flight source-worker unwinding, retaining exact destination/key freshness.
struct SourceJobGuard {
    sender: async_channel::Sender<crate::AppEvent>,
    destination: Destination,
    key: RequestKey,
    stop: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    completed: bool,
    pending: Arc<(Mutex<Option<Job>>, Condvar)>,
    alive: Arc<AtomicBool>,
}
impl SourceJobGuard {
    /// Publishes worker death under the queue lock and reports the latest uncancelled captured request.
    /// Queue-before-death is notified; queue-after-death fails submit, so no pending request can strand.
    fn report_unexpected_death(&mut self) {
        let mut slot = self
            .pending
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        self.alive.store(false, Ordering::Release);
        if self.stop.load(Ordering::Acquire) {
            return;
        }
        let key = slot
            .take()
            .filter(|job| !job.cancelled.load(Ordering::Acquire))
            .map(|job| job.key)
            .or_else(|| (!self.cancelled.load(Ordering::Acquire)).then_some(self.key));
        drop(slot);
        let Some(key) = key else {
            return;
        };
        let completion = Completion {
            key,
            result: Err(Diagnostic::new(
                Origin::Source,
                "preview.source_worker.panicked",
                "Source preview worker stopped unexpectedly",
            )),
        };
        let event = match self.destination {
            Destination::Main => crate::AppEvent::MediaPreview(completion),
            Destination::Draft { epoch, .. } => {
                crate::AppEvent::DraftMediaPreview { epoch, completion }
            }
        };
        let _ = self.sender.send_blocking(event);
    }
}
impl Drop for SourceJobGuard {
    /// Reports unexpected unwind only; normal source cancellation/shutdown never becomes a failure.
    fn drop(&mut self) {
        if !self.completed && std::thread::panicking() {
            self.report_unexpected_death();
        }
    }
}

/// Bridges one Pattern Editor scheduler onto GTK until its captured editor epoch closes.
///
/// Progress is drained and discarded because the existing Pattern Editor status remains a single
/// terminal preview message. The bridge is created once per editor and never once per edit; its
/// stop flag and join handle are owned by the editor surface.
///
/// # Errors
/// Returns the operating-system thread-spawn diagnostic without starting a completion bridge.
pub(crate) fn start_pattern_editor_preview_bridge(
    scheduler: Arc<EvaluationScheduler>,
    sender: async_channel::Sender<crate::AppEvent>,
    stop: Arc<AtomicBool>,
    epoch: u64,
) -> Result<std::thread::JoinHandle<()>, String> {
    std::thread::Builder::new()
        .name("toniator-pattern-editor-preview-bridge".into())
        .spawn(move || {
            'bridge: while !stop.load(Ordering::Acquire) {
                loop {
                    match scheduler.try_receive_latest_progress() {
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(error) => {
                            if !stop.load(Ordering::Acquire) {
                                let _ = sender.send_blocking(
                                    crate::AppEvent::PrivatePreviewWorkerFailed {
                                        surface: crate::app_events::PrivateSurface::Editor,
                                        epoch,
                                        scheduler: scheduler.clone(),
                                        diagnostic: Diagnostic::scheduler(error),
                                    },
                                );
                            }
                            break 'bridge;
                        }
                    }
                }
                match scheduler.try_receive_latest() {
                    Ok(Some(completion)) => {
                        if sender
                            .send_blocking(crate::AppEvent::DraftPreview { epoch, completion })
                            .is_err()
                        {
                            break 'bridge;
                        }
                    }
                    Ok(None) => std::thread::park_timeout(std::time::Duration::from_millis(4)),
                    Err(error) => {
                        if !stop.load(Ordering::Acquire) {
                            let _ =
                                sender.send_blocking(crate::AppEvent::PrivatePreviewWorkerFailed {
                                    surface: crate::app_events::PrivateSurface::Editor,
                                    epoch,
                                    scheduler: scheduler.clone(),
                                    diagnostic: Diagnostic::scheduler(error),
                                });
                        }
                        break;
                    }
                }
            }
        })
        .map_err(|error| error.to_string())
}
impl Drop for Worker {
    /// Cancels and wakes the worker without blocking GTK; provider Drop reaps decoder children.
    fn drop(&mut self) {
        self.cancel();
        self.shutdown.store(true, Ordering::Release);
        self.pending.1.notify_one();
        if let Some(thread) = self.thread.take().filter(|thread| thread.is_finished()) {
            let _ = thread.join();
        }
    }
}

/// Copies a uniformly reduced nearest-pixel source comparison without PNG encoding or flattening.
///
/// This display-only image retains the existing 4096-pixel source-view bound. Native sampling and
/// canonical evaluation retain the original decoded field, including hidden RGB and alpha.
///
/// # Errors
/// Returns typed cancellation, checked source-display addressability or actual allocation refusal.
/// # Panics
/// Panics only if decoder-owned dimensions and SourceField storage disagree, violating source authority.
fn source_display(
    frame: &SourceFrame,
    cancelled: &dyn Fn() -> bool,
) -> Result<SourceDisplay, Diagnostic> {
    let identity = frame.field.identity();
    let scale = (f64::from(crate::SOURCE_VIEW_MAX_EDGE)
        / f64::from(identity.width.max(identity.height)))
    .min(1.0);
    let width = (f64::from(identity.width) * scale).round().max(1.0) as u32;
    let height = (f64::from(identity.height) * scale).round().max(1.0) as u32;
    let count = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| {
            Diagnostic::new(
                Origin::Source,
                "preview.source_display.addressability",
                "Source display count exceeds machine representation",
            )
        })?;
    std::alloc::Layout::array::<u8>(count).map_err(|_| {
        Diagnostic::new(
            Origin::Source,
            "preview.source_display.addressability",
            "Source display exceeds machine addressability",
        )
    })?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(count).map_err(|_| {
        Diagnostic::new(
            Origin::Source,
            "preview.source_display.allocation",
            "Source display allocation failed",
        )
    })?;
    for y in 0..height {
        if cancelled() {
            return Err(Diagnostic::new(
                Origin::Source,
                "evaluation.cancelled",
                "Source preview was cancelled",
            ));
        }
        for x in 0..width {
            let source_x = ((u64::from(x) * u64::from(identity.width)) / u64::from(width)) as u32;
            let source_y = ((u64::from(y) * u64::from(identity.height)) / u64::from(height)) as u32;
            let pixel = frame
                .field
                .pixel(source_x, source_y)
                .expect("bounded source coordinates");
            rgba.extend(
                [pixel.red, pixel.green, pixel.blue, pixel.alpha]
                    .map(|value| (value * 255.0).round() as u8),
            );
        }
    }
    Ok(SourceDisplay {
        width,
        height,
        rgba,
        metadata: serde_json::json!({"format":format!("{:?}",identity.format),"width":identity.width,"height":identity.height,"purpose":"actual decoded private source field"}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Proves desktop single-file exports use the selected End values and preserve Start authority.
    ///
    /// # Panics
    /// Panics if a current media fixture, domain endpoint edit, export or decoded-alpha check fails.
    #[test]
    fn desktop_single_exports_use_selected_media_endpoint() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/video-sample0001-0010.mp4");
        let workspace = crate::load_workspace(&path).unwrap();
        let document = Document::new_default_document(
            toniator_domain::CanvasSpec {
                width: 32.0,
                height: 32.0,
            },
            workspace.document().source().clone(),
        )
        .unwrap();
        let edits = (1..=3)
            .map(|id| toniator_domain::TemporalEndpointEdit {
                target: toniator_domain::PropertyTarget::Channel(toniator_domain::ChannelId(id)),
                field: toniator_domain::PropertyFieldId::Opacity,
                effective_end: 0.0,
                easing: toniator_domain::Easing::Linear,
            })
            .collect::<Vec<_>>();
        let end = document.edit_effective_end_command(&edits).unwrap();
        let document = document
            .with_temporal_authority(
                workspace.document().project_timing().clone(),
                end.replacement().end_overrides.clone(),
            )
            .unwrap();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        for frame in [0, 9] {
            let path = std::env::temp_dir().join(format!(
                "toniator-desktop-endpoint-{}-{stamp}-{frame}.png",
                std::process::id()
            ));
            crate::export_snapshot_frame(
                crate::SavedContent {
                    document: document.clone(),
                    sources: workspace.sources.clone(),
                },
                path.clone(),
                crate::ExportSettings {
                    format: crate::ExportFormat::Png,
                    prepare_for_print: false,
                    pixels_per_metre: None,
                    cleanup: crate::print_cleanup::Settings::default(),
                    background: toniator_engine::RasterBackground::Transparent,
                    output_target: None,
                    antialiasing: toniator_engine::RasterAntialiasing::On,
                },
                frame,
            )
            .unwrap();
            let imported = toniator_engine::import_source_media(
                &[path.clone()],
                None,
                MediaTools::default(),
                &|| false,
            )
            .unwrap();
            let mut media = toniator_engine::open_source_media(
                &imported.sources,
                MediaTools::default(),
                &|| false,
            )
            .unwrap();
            let decoded = media
                .frame_at(toniator_domain::RationalTime::default(), &|| false)
                .unwrap();
            let covered =
                (0..32).any(|y| (0..32).any(|x| decoded.field.pixel(x, y).unwrap().alpha > 0.0));
            assert_eq!(covered, frame == 0);
            std::fs::remove_file(path).unwrap();
        }
        assert_eq!(document.temporal_authority().end_overrides.len(), 3);
        assert!(
            document
                .materialize_frame(0)
                .unwrap().document()
                .modeled_channel(toniator_domain::ChannelId(1))
                .is_some()
        );
    }

    /// Receives a matching real worker result under a finite test deadline.
    ///
    /// # Panics
    /// Panics if the worker terminates or fails to deliver the selected endpoint within ten seconds.
    fn receive(
        receiver: &async_channel::Receiver<crate::AppEvent>,
        key: RequestKey,
    ) -> PreparedFrame {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(Instant::now() < deadline, "source worker deadline");
            match receiver.try_recv() {
                Ok(crate::AppEvent::MediaPreview(completion)) if completion.key == key => {
                    return completion.result.unwrap();
                }
                Ok(_) => {}
                Err(async_channel::TryRecvError::Empty) => {
                    std::thread::sleep(Duration::from_millis(2))
                }
                Err(error) => panic!("source worker disconnected: {error}"),
            }
        }
    }

    /// Receives one Pattern Editor source result while preserving its editor epoch destination.
    ///
    /// # Panics
    /// Panics if the source worker terminates or fails to deliver the requested frame within ten
    /// seconds.
    fn receive_draft(
        receiver: &async_channel::Receiver<crate::AppEvent>,
        epoch: u64,
        key: RequestKey,
    ) -> PreparedFrame {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(
                Instant::now() < deadline,
                "Pattern Editor source worker deadline"
            );
            match receiver.try_recv() {
                Ok(crate::AppEvent::DraftMediaPreview {
                    epoch: event_epoch,
                    completion,
                }) if event_epoch == epoch && completion.key == key => {
                    return completion.result.unwrap();
                }
                Ok(_) => {}
                Err(async_channel::TryRecvError::Empty) => {
                    std::thread::sleep(Duration::from_millis(2))
                }
                Err(error) => panic!("Pattern Editor source worker disconnected: {error}"),
            }
        }
    }

    /// Proves endpoint decoding, source comparison, superseding requests and stale acceptance gates.
    ///
    /// # Panics
    /// Panics when the immutable video, configured software decoder, or worker identity rules fail.
    #[test]
    fn media_worker_selects_endpoints_without_history_or_stale_publication() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/video-sample0001-0010.mp4");
        let workspace = crate::load_workspace(&path).unwrap();
        let session = workspace.history.session();
        let original = session.document().clone();
        assert_eq!(original.project_timing().frame_range().frame_count(), 10);
        let (sender, receiver) = async_channel::unbounded();
        let mut worker = Worker::new(sender).unwrap();
        let target = PreviewRasterTarget::new(64, 64).unwrap();
        let start = RequestKey {
            workspace: 7,
            revision: session.revision().0,
            epoch: 1,
            frame: Endpoint::Start.frame(&original),
        };
        worker
            .submit(start, session.clone(), workspace.sources.clone(), target)
            .unwrap();
        let first = receive(&receiver, start);
        assert_eq!((first.source.width, first.source.height), (1080, 1920));
        let end = RequestKey {
            epoch: 2,
            frame: Endpoint::End.frame(&original),
            ..start
        };
        worker
            .submit(end, session.clone(), workspace.sources.clone(), target)
            .unwrap();
        let last = receive(&receiver, end);
        assert_ne!(first.source.rgba, last.source.rgba);
        assert!(end.is_current(Some(end), 7, session, Endpoint::End));
        assert!(!start.is_current(Some(end), 7, session, Endpoint::Start));
        assert!(!end.is_current(Some(end), 8, session, Endpoint::End));
        assert!(!end.is_current(Some(end), 7, session, Endpoint::Start));
        assert!(!end.is_current(None, 7, session, Endpoint::End));
        let latest = RequestKey { epoch: 4, ..start };
        worker
            .submit(
                RequestKey { epoch: 3, ..end },
                session.clone(),
                workspace.sources.clone(),
                target,
            )
            .unwrap();
        worker
            .submit(latest, session.clone(), workspace.sources.clone(), target)
            .unwrap();
        assert_eq!(receive(&receiver, latest).source.rgba, first.source.rgba);
        assert_eq!(session.document(), &original);
        worker.cancel();
        let stopped = Instant::now();
        drop(worker);
        assert!(stopped.elapsed() < Duration::from_secs(2));
    }

    /// Proves the Pattern Editor worker keeps native raster, SVG, and moving-media frames intact.
    ///
    /// Still sources retain their natural dimensions, while the video route selects distinct Start
    /// and End decoded frames. The editor destination, revision gate, superseding request, and
    /// worker drop lifetime remain independent from the main preview worker.
    ///
    /// # Panics
    /// Panics if one current baseline source cannot be loaded, decoded, or delivered to its private
    /// completion destination.
    #[test]
    fn pattern_editor_worker_preserves_current_media_frames_and_gates_stale_edits() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        for (filename, dimensions) in [
            ("raster-sample.png", (1024, 1024)),
            ("vector-sample.svg", (900, 620)),
            ("video-sample0001-0010.mp4", (1080, 1920)),
        ] {
            let workspace = crate::load_workspace(&root.join(filename)).unwrap();
            let session = workspace.history.session();
            let document = session.document().clone();
            let (sender, receiver) = async_channel::unbounded();
            let mut worker = Worker::new_for_pattern_editor(sender, 73).unwrap();
            let target = PreviewRasterTarget::new(64, 64).unwrap();
            let start = RequestKey {
                workspace: 19,
                revision: session.revision().0,
                epoch: 73,
                frame: Endpoint::Start.frame(&document),
            };
            worker
                .submit(start, session.clone(), workspace.sources.clone(), target)
                .unwrap();
            let first = receive_draft(&receiver, 73, start);
            assert_eq!((first.source.width, first.source.height), dimensions);
            assert_eq!(
                first.source.rgba.len(),
                dimensions.0 as usize * dimensions.1 as usize * 4
            );
            assert!(start.is_current(Some(start), 19, session, Endpoint::Start));

            let end = RequestKey {
                epoch: 74,
                frame: Endpoint::End.frame(&document),
                ..start
            };
            worker
                .submit(end, session.clone(), workspace.sources.clone(), target)
                .unwrap();
            let last = receive_draft(&receiver, 73, end);
            if filename.ends_with(".mp4") {
                assert_ne!(first.source.rgba, last.source.rgba);
            } else {
                assert_eq!(first.source.rgba, last.source.rgba);
            }
            assert!(end.is_current(Some(end), 19, session, Endpoint::End));
            assert!(!start.is_current(Some(end), 19, session, Endpoint::Start));

            if filename.ends_with(".mp4") {
                let stale = RequestKey {
                    epoch: 75,
                    frame: start.frame,
                    ..start
                };
                let latest = RequestKey {
                    epoch: 76,
                    frame: end.frame,
                    ..end
                };
                worker
                    .submit(stale, session.clone(), workspace.sources.clone(), target)
                    .unwrap();
                worker
                    .submit(latest, session.clone(), workspace.sources.clone(), target)
                    .unwrap();
                let _ = receive_draft(&receiver, 73, latest);
            }

            let selected = document
                .channel_topology()
                .and_then(|topology| topology.channels().first())
                .map(|channel| channel.id)
                .unwrap_or(toniator_domain::ChannelId(1));
            let mut edited = crate::fresh_history(document.clone()).unwrap();
            edited
                .apply(&toniator_domain::DocumentCommand::SetVisibility {
                    channel_id: selected,
                    visible: false,
                })
                .unwrap();
            assert!(!end.is_current(Some(end), 19, edited.session(), Endpoint::End));
            worker.cancel();
            drop(worker);
        }
    }

    /// Proves one Pattern Editor completion bridge can be stopped and reaped independently.
    ///
    /// The bridge owns no document or GTK state, so closing its captured editor epoch only needs
    /// its stop flag and join handle; no per-edit polling thread remains after the join.
    ///
    /// # Panics
    /// Panics if the private scheduler or bridge thread cannot be created or joined.
    #[test]
    fn pattern_editor_preview_bridge_stops_without_a_per_edit_thread() {
        let scheduler = Arc::new(EvaluationScheduler::new().unwrap());
        let (sender, _receiver) = async_channel::unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let bridge = start_pattern_editor_preview_bridge(
            Arc::clone(&scheduler),
            sender,
            Arc::clone(&stop),
            91,
        )
        .unwrap();
        stop.store(true, Ordering::Release);
        bridge.join().unwrap();
        scheduler.cancel_and_clear();
    }
}

#[cfg(test)]
mod reliability_worker_tests {
    use super::*;
    /// Exercises real queue/death synchronization without provoking a host or decoder panic.
    /// # Panics
    /// Panics if queue-before-death strands a newer request, queue-after-death succeeds, or shutdown logs failure.
    #[test]
    fn unexpected_death_owns_latest_queue_and_excludes_shutdown() {
        let session = DocumentSession::new(
            Document::new_default_document(
                toniator_domain::CanvasSpec {
                    width: 8.0,
                    height: 8.0,
                },
                toniator_domain::SourceReference::Unassigned,
            )
            .unwrap(),
        )
        .unwrap();
        let sources = SourceBundle::new([]).unwrap();
        let target = PreviewRasterTarget::new(8, 8).unwrap();
        for (queued, shutdown, current_cancelled) in [
            (false, false, false),
            (true, false, true),
            (true, true, true),
            (false, false, true),
        ] {
            let (sender, receiver) = async_channel::unbounded();
            let stop = Arc::new(AtomicBool::new(false));
            let alive = Arc::new(AtomicBool::new(true));
            let pending = Arc::new((Mutex::new(None), Condvar::new()));
            let flag = stop.clone();
            let thread = std::thread::spawn(move || {
                while !flag.load(Ordering::Acquire) {
                    std::thread::park_timeout(std::time::Duration::from_millis(1));
                }
            });
            let mut worker = Worker {
                pending: pending.clone(),
                shutdown: stop.clone(),
                alive: alive.clone(),
                latest: None,
                thread: Some(thread),
            };
            let old = RequestKey {
                workspace: 1,
                revision: 0,
                epoch: 1,
                frame: 0,
            };
            let new = RequestKey { epoch: 2, ..old };
            let cancelled = Arc::new(AtomicBool::new(current_cancelled));
            let mut guard = SourceJobGuard {
                sender,
                destination: Destination::Main,
                key: old,
                stop: stop.clone(),
                cancelled,
                completed: false,
                pending,
                alive,
            };
            if queued {
                worker
                    .submit(new, session.clone(), sources.clone(), target)
                    .unwrap();
            }
            if shutdown {
                stop.store(true, Ordering::Release);
            }
            guard.report_unexpected_death();
            if shutdown || (!queued && current_cancelled) {
                assert!(receiver.try_recv().is_err());
            } else {
                let crate::AppEvent::MediaPreview(completion) = receiver.try_recv().unwrap() else {
                    panic!("main destination")
                };
                assert_eq!(completion.key, if queued { new } else { old });
                assert_eq!(
                    completion.result.err().unwrap().code,
                    "preview.source_worker.panicked"
                );
                assert_eq!(
                    worker
                        .submit(
                            RequestKey { epoch: 3, ..new },
                            session.clone(),
                            sources.clone(),
                            target
                        )
                        .unwrap_err()
                        .code,
                    "preview.source_worker.unavailable"
                );
            }
            stop.store(true, Ordering::Release);
            worker.thread.take().unwrap().join().unwrap();
        }
    }
}
