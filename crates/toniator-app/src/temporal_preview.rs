//! Cancellable source-frame preparation; document timing and interpolation remain headless authority.

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

/// A bounded straight-RGBA source comparison image prepared entirely off the GTK thread.
pub(crate) struct SourceDisplay {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

/// Supplies immutable evaluator input and the matching source comparison image together.
pub(crate) struct PreparedFrame {
    pub(crate) request: EvaluationRequest,
    pub(crate) source: SourceDisplay,
}

/// Carries a generation-keyed decode result back to GTK for acceptance before scheduler submission.
pub(crate) struct Completion {
    pub(crate) key: RequestKey,
    pub(crate) result: Result<PreparedFrame, String>,
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
    Draft { epoch: u64 },
}

/// Owns one worker and one replaceable pending request; native decoder iterators never cross threads.
pub(crate) struct Worker {
    pending: Arc<(Mutex<Option<Job>>, Condvar)>,
    shutdown: Arc<AtomicBool>,
    latest: Option<Arc<AtomicBool>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    /// Starts one source worker with bounded latest-request queuing and shared source providers.
    ///
    /// # Errors
    /// Returns the operating-system thread-spawn diagnostic without creating a live worker.
    pub(crate) fn new(sender: async_channel::Sender<crate::AppEvent>) -> Result<Self, String> {
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
    pub(crate) fn new_for_pattern_editor(
        sender: async_channel::Sender<crate::AppEvent>,
        epoch: u64,
    ) -> Result<Self, String> {
        Self::new_with_destination(sender, Destination::Draft { epoch })
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
    ) -> Result<Self, String> {
        let pending = Arc::new((Mutex::new(None::<Job>), Condvar::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let jobs = Arc::clone(&pending);
        let stop = Arc::clone(&shutdown);
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
                                .map_err(|error| error.to_string())?,
                            );
                            cached_sources = Some(job.sources.clone());
                        }
                        let media = provider.as_mut().ok_or("Source provider is unavailable")?;
                        let time = job
                            .session
                            .document()
                            .project_timing()
                            .source_time_for_frame(job.key.frame)
                            .map_err(|error| error.to_string())?;
                        let frame = media
                            .frame_at(time, &cancelled)
                            .map_err(|error| error.to_string())?;
                        let source = source_display(&frame, &cancelled)?;
                        let request = toniator_engine::frame_evaluation_request(
                            &job.session,
                            media,
                            job.key.frame,
                            &cancelled,
                        )
                        .map_err(|error| error.to_string())?
                        .for_preview(job.target);
                        Ok(PreparedFrame { request, source })
                    })();
                    if !cancelled() {
                        let completion = Completion {
                            key: job.key,
                            result,
                        };
                        let event = match destination {
                            Destination::Main => crate::AppEvent::MediaPreview(completion),
                            Destination::Draft { epoch } => {
                                crate::AppEvent::DraftMediaPreview { epoch, completion }
                            }
                        };
                        let _ = sender.send_blocking(event);
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            pending,
            shutdown,
            latest: None,
            thread: Some(thread),
            // The completion destination is captured by the worker closure. This field is kept
            // out of the mutable worker state so submit/cancel cannot retarget in-flight work.
        })
    }

    /// Replaces pending decode work and cooperatively stops its predecessor without blocking GTK.
    pub(crate) fn submit(
        &mut self,
        key: RequestKey,
        session: DocumentSession,
        sources: SourceBundle,
        target: PreviewRasterTarget,
    ) {
        self.cancel();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.latest = Some(Arc::clone(&cancelled));
        *self
            .pending
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(Job {
            key,
            session,
            sources,
            target,
            cancelled,
        });
        self.pending.1.notify_one();
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
                while matches!(scheduler.try_receive_latest_progress(), Ok(Some(_))) {}
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
                    Err(_) => break,
                }
            }
        })
        .map_err(|error| error.to_string())
}
impl Drop for Worker {
    /// Stops the worker, reaps decoder children through provider Drop, and joins before shutdown.
    fn drop(&mut self) {
        self.cancel();
        self.shutdown.store(true, Ordering::Release);
        self.pending.1.notify_one();
        if let Some(thread) = self.thread.take() {
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
/// Returns cancellation before publishing a partially prepared image.
fn source_display(
    frame: &SourceFrame,
    cancelled: &dyn Fn() -> bool,
) -> Result<SourceDisplay, String> {
    let identity = frame.field.identity();
    let scale = (f64::from(crate::SOURCE_VIEW_MAX_EDGE)
        / f64::from(identity.width.max(identity.height)))
    .min(1.0);
    let width = (f64::from(identity.width) * scale).round().max(1.0) as u32;
    let height = (f64::from(identity.height) * scale).round().max(1.0) as u32;
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        if cancelled() {
            return Err("Source preview was cancelled".into());
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
                .unwrap()
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
        worker.submit(start, session.clone(), workspace.sources.clone(), target);
        let first = receive(&receiver, start);
        assert_eq!((first.source.width, first.source.height), (1080, 1920));
        let end = RequestKey {
            epoch: 2,
            frame: Endpoint::End.frame(&original),
            ..start
        };
        worker.submit(end, session.clone(), workspace.sources.clone(), target);
        let last = receive(&receiver, end);
        assert_ne!(first.source.rgba, last.source.rgba);
        assert!(end.is_current(Some(end), 7, session, Endpoint::End));
        assert!(!start.is_current(Some(end), 7, session, Endpoint::Start));
        assert!(!end.is_current(Some(end), 8, session, Endpoint::End));
        assert!(!end.is_current(Some(end), 7, session, Endpoint::Start));
        assert!(!end.is_current(None, 7, session, Endpoint::End));
        let latest = RequestKey { epoch: 4, ..start };
        worker.submit(
            RequestKey { epoch: 3, ..end },
            session.clone(),
            workspace.sources.clone(),
            target,
        );
        worker.submit(latest, session.clone(), workspace.sources.clone(), target);
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
            worker.submit(start, session.clone(), workspace.sources.clone(), target);
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
            worker.submit(end, session.clone(), workspace.sources.clone(), target);
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
                worker.submit(stale, session.clone(), workspace.sources.clone(), target);
                worker.submit(latest, session.clone(), workspace.sources.clone(), target);
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
