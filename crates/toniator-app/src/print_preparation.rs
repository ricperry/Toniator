//! Widget-free ownership for an optional final-output print-preparation check.
//!
//! The document session and source bundle are captured together. GTK controls
//! only request or invalidate work; this module never edits pixels or gates export.

use std::{
    mem::size_of,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::print_cleanup;
use toniator_domain::{
    DocumentCommand, DocumentEvaluationToken, DocumentHistory, DocumentSession,
    HalftoneChannelModel, PrintPreparationSettings,
};
use toniator_engine::{
    EvaluationLimits, MediaTools, OutputRasterTarget, RasterAntialiasing, RasterBackground,
    RasterSurface, open_source_media,
    print_preflight::{
        PreflightLimits, PreflightRasterOutcome, PreflightRasterReport, PreflightSelection,
        preflight_current_frame_with_raster,
    },
};
use toniator_io::SourceBundle;

/// The user's export-backing intent; Automatic follows the captured channel model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BackgroundChoice {
    Automatic,
    Explicit(RasterBackground),
}

impl BackgroundChoice {
    /// Resolves export backing without consulting viewer backdrop state.
    pub(crate) fn resolve(self, model: Option<HalftoneChannelModel>) -> RasterBackground {
        match self {
            Self::Automatic => RasterBackground::default_for_model(model),
            Self::Explicit(background) => background,
        }
    }
}

/// Runtime PNG intent shared by the real exporter and optional preflight check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OutputSelection {
    pub(crate) target: OutputRasterTarget,
    pub(crate) antialiasing: RasterAntialiasing,
    pub(crate) background: BackgroundChoice,
    /// Explicit PNG-only garment preparation; a collapsed section never changes this value.
    pub(crate) prepare_for_print: bool,
    /// Exact integer PNG pHYs density, present only for a validated print box.
    pub(crate) pixels_per_metre: Option<u32>,
    pub(crate) cleanup: print_cleanup::Settings,
}

/// Converts a transparent final composition to a binary mask, then applies export backing.
///
/// Alpha below 128 becomes transparent black; alpha 128 or above becomes fully
/// opaque with its original RGB. Opaque backing fills only the discarded pixels;
/// applying backing after thresholding prevents prematted RGB from changing survivors.
///
/// # Errors
/// Returns an allocation or surface-validation error without publishing partial pixels.
pub(crate) fn prepare_binary_alpha(
    surface: &RasterSurface,
    backing: RasterBackground,
    cancelled: &AtomicBool,
) -> Result<RasterSurface, String> {
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(surface.pixels().len())
        .map_err(|error| format!("print preparation: could not allocate final pixels: {error}"))?;
    pixels.extend_from_slice(surface.pixels());
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        if index % 4096 == 0 && cancelled.load(Ordering::Acquire) {
            return Err("Print preparation cancelled.".into());
        }
        if pixel[3] < 128 {
            match backing {
                RasterBackground::Transparent => pixel.copy_from_slice(&[0, 0, 0, 0]),
                RasterBackground::OpaqueBlack => pixel.copy_from_slice(&[0, 0, 0, 255]),
                RasterBackground::OpaqueWhite => pixel.copy_from_slice(&[255, 255, 255, 255]),
            }
        } else {
            pixel[3] = 255;
        }
    }
    RasterSurface::new(surface.width(), surface.height(), pixels).map_err(|error| error.to_string())
}

impl OutputSelection {
    /// Resolves one effective backing from the captured document model.
    pub(crate) fn background_for(self, session: &DocumentSession) -> RasterBackground {
        self.background.resolve(session.document().channel_model())
    }

    /// Projects applied document print intent and runtime output intent into engine selection.
    fn preflight(self, capture: &ExportCapture) -> PreflightSelection {
        PreflightSelection {
            frame: capture.frame,
            target: self.target,
            antialiasing: self.antialiasing,
            selected_background: self.background_for(&capture.session),
            settings: capture.session.document().print_preparation().clone(),
        }
    }
}

/// One coherent live document/source/frame read before export's save chooser.
///
/// This keeps the real session revision. Rebuilding `DocumentSession` from a
/// saved document would reset its token and could falsely admit stale work.
#[derive(Clone, Debug)]
pub(crate) struct ExportCapture {
    pub(crate) session: DocumentSession,
    pub(crate) sources: SourceBundle,
    pub(crate) frame: u64,
    pub(crate) lifecycle_generation: u64,
    pub(crate) workspace_generation: u64,
}

impl ExportCapture {
    /// Checks only workspace/lifecycle replacement; later edits keep this initiated export exact.
    pub(crate) fn workspace_is_current(&self, lifecycle: u64, workspace: u64) -> bool {
        self.lifecycle_generation == lifecycle && self.workspace_generation == workspace
    }
}

/// A completed check status without any implicit safe/pass judgment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CheckStatus {
    NotChecked,
    Running,
    CurrentAdvisory,
    OutOfDate,
    Cancelled,
    Unavailable,
}

#[derive(Clone)]
struct Request {
    capture: ExportCapture,
    selection: OutputSelection,
}

struct Active {
    id: u64,
    request: Request,
    cancelled: Arc<AtomicBool>,
}

/// One independently cancellable worker input; only the controller creates IDs.
pub(crate) struct CheckJob {
    id: u64,
    request: Request,
    cancelled: Arc<AtomicBool>,
}

/// Worker result delivered to the main thread without GTK or mutable document authority.
pub(crate) struct CheckCompletion {
    id: u64,
    outcome: Result<PreflightRasterOutcome, String>,
}

impl CheckJob {
    /// Decodes, renders, analyzes, and rechecks the captured source off GTK's thread.
    ///
    /// Cancellation and budget outcomes cannot publish a partial pair. The
    /// main-thread controller separately checks live workspace/session/selection.
    pub(crate) fn run(self) -> CheckCompletion {
        let result = (|| {
            let capture = &self.request.capture;
            let selection = self.request.selection.preflight(capture);
            let mut media = open_source_media(&capture.sources, MediaTools::default(), &|| {
                self.cancelled.load(Ordering::Acquire)
            })
            .map_err(|error| error.to_string())?;
            let outcome = preflight_current_frame_with_raster(
                &capture.session,
                &mut media,
                &selection,
                EvaluationLimits::default(),
                PreflightLimits::default(),
                &self.cancelled,
            )
            .map_err(|error| error.to_string())?;
            if let PreflightRasterOutcome::Complete(pair) = &outcome
                && !pair
                    .report()
                    .identity
                    .is_current(&capture.session, &mut media, &selection, &self.cancelled)
                    .map_err(|error| error.to_string())?
            {
                return Err("Preflight source became out of date.".to_owned());
            }
            Ok(outcome)
        })();
        CheckCompletion {
            id: self.id,
            outcome: result,
        }
    }
}

/// App-owned single-worker state and only retained report/raster pair.
///
/// Superseding a request cancels the active job and queues the newest capture;
/// its successor starts only after the previous completion frees the busy slot.
#[derive(Default)]
pub(crate) struct Controller {
    next_id: u64,
    active: Option<Active>,
    queued: Option<Request>,
    accepted: Option<Box<PreflightRasterReport>>,
    accepted_request: Option<Request>,
    selection: Option<OutputSelection>,
    status: Option<CheckStatus>,
    unavailable_reason: Option<String>,
}

impl Controller {
    /// Returns the truthful current state for future GTK projection.
    #[allow(dead_code)] // G2b projects this internal result into the review surface.
    pub(crate) fn status(&self) -> CheckStatus {
        self.status.unwrap_or(CheckStatus::NotChecked)
    }

    /// Borrows the pair only while it remains current and retained within the UI budget.
    #[allow(dead_code)] // G2b projects this internal result into the review surface.
    pub(crate) fn accepted(&self) -> Option<&PreflightRasterReport> {
        self.accepted.as_deref()
    }

    /// Returns an explicit resource or worker diagnostic when unavailable.
    #[allow(dead_code)] // G2b projects this internal diagnostic into the review surface.
    pub(crate) fn unavailable_reason(&self) -> Option<&str> {
        self.unavailable_reason.as_deref()
    }

    /// Cancels work and hides old geometry as soon as document or output intent changes.
    pub(crate) fn invalidate(&mut self) {
        let had_result = self.accepted.take().is_some();
        self.accepted_request = None;
        let had_work = self.active.is_some() || self.queued.is_some();
        if let Some(active) = &self.active {
            active.cancelled.store(true, Ordering::Release);
        }
        self.queued = None;
        self.unavailable_reason = None;
        self.status = Some(if had_result {
            CheckStatus::OutOfDate
        } else if had_work {
            CheckStatus::Cancelled
        } else {
            CheckStatus::NotChecked
        });
    }

    /// Cancels explicit review work without retaining a superseded report.
    #[allow(dead_code)] // G2b's Cancel control dispatches this internal action.
    pub(crate) fn cancel(&mut self) {
        if let Some(active) = &self.active {
            active.cancelled.store(true, Ordering::Release);
        }
        self.queued = None;
        self.accepted = None;
        self.accepted_request = None;
        self.unavailable_reason = None;
        self.status = Some(CheckStatus::Cancelled);
    }

    /// Records a new runtime output selection and immediately retires a changed report.
    pub(crate) fn select(&mut self, selection: OutputSelection) {
        if self.selection != Some(selection) {
            self.invalidate();
            self.selection = Some(selection);
        }
    }

    /// Retires a pair or pending job immediately when live document/frame authority changes.
    ///
    /// Presentation-only updates call this safely because an unchanged token
    /// and endpoint leave accepted pixels and running work intact.
    pub(crate) fn reconcile(&mut self, live: Option<LiveAuthority>) {
        let stale = self.active.as_ref().is_some_and(|active| {
            !active.cancelled.load(Ordering::Acquire)
                && live.is_none_or(|live| !live.matches(&active.request))
        }) || self
            .queued
            .as_ref()
            .is_some_and(|request| live.is_none_or(|live| !live.matches(request)))
            || self
                .accepted_request
                .as_ref()
                .is_some_and(|request| live.is_none_or(|live| !live.matches(request)));
        if stale {
            self.invalidate();
            self.status = Some(CheckStatus::OutOfDate);
        }
    }

    /// Starts explicit optional analysis, or keeps only its newest queued successor.
    ///
    /// A target beyond the retained-payload budget becomes Unavailable without
    /// downsampling or changing the caller's selected export target.
    pub(crate) fn request(
        &mut self,
        capture: ExportCapture,
        selection: OutputSelection,
    ) -> Option<CheckJob> {
        self.select(selection);
        let request = Request { capture, selection };
        let pixels = u64::from(selection.target.width())
            .checked_mul(u64::from(selection.target.height()))
            .and_then(|pixels| pixels.checked_mul(4));
        if pixels.is_none_or(|bytes| bytes > MAX_RETAINED_BYTES) {
            self.invalidate();
            self.status = Some(CheckStatus::Unavailable);
            self.unavailable_reason =
                Some("Selected final target exceeds the retained review raster budget.".into());
            return None;
        }
        self.accepted = None;
        self.accepted_request = None;
        self.unavailable_reason = None;
        self.status = Some(CheckStatus::Running);
        if let Some(active) = &self.active {
            active.cancelled.store(true, Ordering::Release);
            self.queued = Some(request);
            None
        } else {
            self.launch(request)
        }
    }

    /// Admits only the matching latest completion and starts a queued successor afterward.
    ///
    /// An obsolete completion cannot clear a newer active job, report, or status.
    pub(crate) fn complete(
        &mut self,
        completion: CheckCompletion,
        live: Option<LiveAuthority>,
    ) -> Option<CheckJob> {
        if self
            .active
            .as_ref()
            .is_none_or(|active| active.id != completion.id)
        {
            return None;
        }
        let active = self.active.take().expect("matching active check exists");
        if let Some(next) = self.queued.take() {
            if live.is_some_and(|live| live.matches(&next)) {
                return self.launch(next);
            }
            self.status = Some(CheckStatus::OutOfDate);
            return None;
        }
        if active.cancelled.load(Ordering::Acquire) {
            return None;
        }
        if live.is_none_or(|live| !live.matches(&active.request))
            || self.selection != Some(active.request.selection)
        {
            self.accepted = None;
            self.status = Some(CheckStatus::OutOfDate);
            return None;
        }
        match completion.outcome {
            Ok(PreflightRasterOutcome::Complete(pair)) => {
                if retained_bytes(&pair).is_none_or(|bytes| bytes > MAX_RETAINED_BYTES) {
                    self.status = Some(CheckStatus::Unavailable);
                    self.unavailable_reason =
                        Some("Review result exceeds the retained UI memory budget.".into());
                } else {
                    self.status = Some(CheckStatus::CurrentAdvisory);
                    self.accepted = Some(pair);
                    self.accepted_request = Some(active.request);
                }
            }
            Ok(PreflightRasterOutcome::Unavailable { reason }) | Err(reason) => {
                self.status = Some(CheckStatus::Unavailable);
                self.unavailable_reason = Some(reason);
            }
            Ok(PreflightRasterOutcome::Cancelled) => {
                self.status = Some(CheckStatus::Cancelled);
            }
        }
        None
    }

    /// Frees all retained review state when its owning surface or workspace closes.
    pub(crate) fn close(&mut self) {
        self.invalidate();
        self.accepted = None;
        self.accepted_request = None;
        self.selection = None;
        self.status = Some(CheckStatus::NotChecked);
    }

    /// Allocates the next controller-local request identity without concurrent work.
    fn launch(&mut self, request: Request) -> Option<CheckJob> {
        let Some(id) = self.next_id.checked_add(1) else {
            self.status = Some(CheckStatus::Unavailable);
            self.unavailable_reason = Some("Review request identity is exhausted.".into());
            return None;
        };
        self.next_id = id;
        let cancelled = Arc::new(AtomicBool::new(false));
        self.active = Some(Active {
            id,
            request: request.clone(),
            cancelled: Arc::clone(&cancelled),
        });
        self.status = Some(CheckStatus::Running);
        Some(CheckJob {
            id,
            request,
            cancelled,
        })
    }
}

/// Borrowed live authority checked after off-thread source freshness succeeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LiveAuthority {
    pub(crate) token: DocumentEvaluationToken,
    pub(crate) frame: u64,
    pub(crate) lifecycle_generation: u64,
    pub(crate) workspace_generation: u64,
}

impl LiveAuthority {
    /// Rejects a replaced workspace, edited session, or changed endpoint.
    fn matches(self, request: &Request) -> bool {
        request
            .capture
            .workspace_is_current(self.lifecycle_generation, self.workspace_generation)
            && self.token == request.capture.session.document_evaluation_token()
            && self.frame == request.capture.frame
    }
}

pub(crate) const MAX_RETAINED_BYTES: u64 = 64 * 1024 * 1024;

/// Accounts for retained RGBA and result-vector payload capacity.
///
/// This is independent from engine detector scratch limits, but not a process
/// RSS or hard object cap: identity strings, allocator overhead, source/provider
/// and evaluator allocations are outside it. Future texture/highlight ownership
/// must charge this same budget before retention.
pub(crate) fn retained_bytes(pair: &PreflightRasterReport) -> Option<u64> {
    let report = pair.report();
    let mut bytes = u64::try_from(pair.raster().pixels().len()).ok()?;
    for (capacity, item_size) in [
        (report.alpha_histogram.capacity(), size_of::<u64>()),
        (
            report.support_components.capacity(),
            size_of::<toniator_engine::print_preflight::SupportComponent>(),
        ),
        (
            report.positive_candidates.capacity(),
            size_of::<toniator_engine::print_preflight::ProbeCandidate>(),
        ),
        (
            report.negative_candidates.capacity(),
            size_of::<toniator_engine::print_preflight::ProbeCandidate>(),
        ),
    ] {
        bytes = bytes.checked_add(u64::try_from(capacity.checked_mul(item_size)?).ok()?)?;
    }
    for component in &report.support_components {
        bytes = bytes.checked_add(
            u64::try_from(
                component
                    .runs
                    .capacity()
                    .checked_mul(size_of::<toniator_engine::print_preflight::PixelRun>())?,
            )
            .ok()?,
        )?;
        bytes = bytes.checked_add(
            u64::try_from(
                component
                    .unresolved_low_coverage_runs
                    .capacity()
                    .checked_mul(size_of::<toniator_engine::print_preflight::PixelRun>())?,
            )
            .ok()?,
        )?;
    }
    for candidate in report
        .positive_candidates
        .iter()
        .chain(&report.negative_candidates)
    {
        bytes = bytes.checked_add(
            u64::try_from(
                candidate
                    .runs
                    .capacity()
                    .checked_mul(size_of::<toniator_engine::print_preflight::PixelRun>())?,
            )
            .ok()?,
        )?;
    }
    Some(bytes)
}

/// Applies complete validated physical intent through the existing reversible history command.
///
/// Returns false for semantic no-op input. The caller retires preflight and
/// preview tokens only after an actual edit; this function never changes art.
///
/// # Errors
/// Rejects invalid print settings or a stale authoritative history transition.
#[allow(dead_code)] // G2b's Apply control will dispatch this internal command entry.
pub(crate) fn apply_settings(
    history: &mut DocumentHistory,
    settings: PrintPreparationSettings,
) -> Result<bool, String> {
    settings.validate().map_err(|error| error.to_string())?;
    let base = history.document().print_preparation().clone();
    if base == settings {
        return Ok(false);
    }
    history
        .apply(&DocumentCommand::SetPrintPreparation { base, settings })
        .map_err(|error| error.to_string())?;
    Ok(true)
}

/// Exposes the captured live revision for focused authority tests.
#[cfg(test)]
fn capture_token(capture: &ExportCapture) -> DocumentEvaluationToken {
    capture.session.document_evaluation_token()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview_coordinator::PreviewCoordinator;
    use std::path::Path;
    use toniator_domain::{CanvasSpec, Document, PhysicalPrintSizeMm, SourceReference};

    /// Checks every eight-bit alpha value at the fixed cutoff and both backing outcomes.
    #[test]
    fn garment_binary_alpha_preserves_survivor_rgb_and_eliminates_fractional_alpha() {
        let mut source = Vec::new();
        for alpha in 0..=255_u8 {
            source.extend_from_slice(&[37, 89, 201, alpha]);
        }
        let surface = RasterSurface::new(256, 1, source).unwrap();
        let transparent = prepare_binary_alpha(
            &surface,
            RasterBackground::Transparent,
            &AtomicBool::new(false),
        )
        .unwrap();
        let white = prepare_binary_alpha(
            &surface,
            RasterBackground::OpaqueWhite,
            &AtomicBool::new(false),
        )
        .unwrap();
        for alpha in 0..=255_usize {
            let offset = alpha * 4;
            assert_eq!(
                &transparent.pixels()[offset..offset + 4],
                if alpha < 128 {
                    &[0, 0, 0, 0]
                } else {
                    &[37, 89, 201, 255]
                }
            );
            assert_eq!(
                &white.pixels()[offset..offset + 4],
                if alpha < 128 {
                    &[255, 255, 255, 255]
                } else {
                    &[37, 89, 201, 255]
                }
            );
        }
        assert_eq!(surface.pixels()[127 * 4 + 3], 127);
        assert_eq!(surface.pixels()[128 * 4 + 3], 128);
    }

    /// Builds an authoritative headless history without starting GTK or source work.
    fn history() -> DocumentHistory {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 64.0,
                height: 48.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap();
        DocumentHistory::new(DocumentSession::new(document).unwrap())
    }

    /// Captures the actual live session revision for controller race tests.
    fn capture(history: &DocumentHistory, workspace_generation: u64) -> ExportCapture {
        ExportCapture {
            session: history.session().clone(),
            sources: SourceBundle::new([]).unwrap(),
            frame: 0,
            lifecycle_generation: 3,
            workspace_generation,
        }
    }

    /// Resolves ordinary PNG defaults with no physical intent or viewer backdrop dependency.
    fn selection() -> OutputSelection {
        OutputSelection {
            target: OutputRasterTarget::new(64, 48).unwrap(),
            antialiasing: RasterAntialiasing::On,
            background: BackgroundChoice::Automatic,
            prepare_for_print: false,
            pixels_per_metre: None,
            cleanup: print_cleanup::Settings::default(),
        }
    }

    /// Returns the main-thread authority stamp for a captured session.
    fn live(capture: &ExportCapture) -> LiveAuthority {
        LiveAuthority {
            token: capture_token(capture),
            frame: capture.frame,
            lifecycle_generation: capture.lifecycle_generation,
            workspace_generation: capture.workspace_generation,
        }
    }

    /// Proves source/session/frame capture remains coherent across later history edits.
    ///
    /// # Panics
    /// Panics if a reconstructed revision or later document silently replaces export content.
    #[test]
    fn capture_retains_live_token_and_document_across_edit() {
        let mut history = history();
        let first = capture(&history, 7);
        let first_token = capture_token(&first);
        let settings = PrintPreparationSettings::new(
            Some(PhysicalPrintSizeMm::new(80.0, 60.0).unwrap()),
            0.5,
            0.2,
        )
        .unwrap();
        assert!(apply_settings(&mut history, settings).unwrap());
        assert_ne!(history.session().document_evaluation_token(), first_token);
        assert_eq!(capture_token(&first), first_token);
        assert!(first.workspace_is_current(3, 7));
        assert!(!first.workspace_is_current(3, 8));
    }

    /// Proves print intent uses one typed reversible history entry and no semantic no-op.
    ///
    /// # Panics
    /// Panics if Apply or Undo/Redo loses physical settings authority.
    #[test]
    fn applied_physical_settings_are_undoable_without_duplicate_noop() {
        let mut history = history();
        let original = history.document().print_preparation().clone();
        let settings = PrintPreparationSettings::new(
            Some(PhysicalPrintSizeMm::new(100.0, 75.0).unwrap()),
            0.4,
            0.3,
        )
        .unwrap();
        assert!(apply_settings(&mut history, settings.clone()).unwrap());
        assert!(!apply_settings(&mut history, settings.clone()).unwrap());
        assert_eq!(history.document().print_preparation(), &settings);
        history.undo().unwrap();
        assert_eq!(history.document().print_preparation(), &original);
        history.redo().unwrap();
        assert_eq!(history.document().print_preparation(), &settings);
    }

    /// Proves Automatic follows channel model while explicit backing stays independent.
    ///
    /// # Panics
    /// Panics if export defaults drift or viewer color leaks into output selection.
    #[test]
    fn automatic_and_explicit_backing_provenance_are_distinct() {
        assert_eq!(
            BackgroundChoice::Automatic.resolve(Some(HalftoneChannelModel::Rgb)),
            RasterBackground::OpaqueBlack
        );
        assert_eq!(
            BackgroundChoice::Automatic.resolve(Some(HalftoneChannelModel::Cmyk)),
            RasterBackground::OpaqueWhite
        );
        assert_eq!(
            BackgroundChoice::Automatic.resolve(Some(HalftoneChannelModel::SourceColorAlpha)),
            RasterBackground::Transparent
        );
        assert_eq!(
            BackgroundChoice::Explicit(RasterBackground::Transparent)
                .resolve(Some(HalftoneChannelModel::Rgb)),
            RasterBackground::Transparent
        );
    }

    /// Proves a None-invalidation intent edit and Undo/Redo retire old preview tickets.
    ///
    /// The existing `set_preview_pending` route calls `queue_refresh`; this
    /// widget-free test checks its token decision while G1b's render-parity test
    /// verifies the existing cache remains reusable for the new token.
    ///
    /// # Panics
    /// Panics if old-token work can be admitted after a print-intent history move.
    #[test]
    fn physical_intent_history_retires_pending_preview_token_without_art_invalidation() {
        let mut history = history();
        let mut preview = PreviewCoordinator::default();
        preview.submit(7, 11);
        let settings = PrintPreparationSettings::new(None, 0.5, 0.0).unwrap();
        assert!(apply_settings(&mut history, settings).unwrap());
        preview.queue_refresh();
        assert!(!preview.accept(7, 11));
        let undo = history.undo().unwrap().unwrap();
        assert_eq!(undo.invalidation, None);
        preview.submit(7, 12);
        preview.queue_refresh();
        assert!(!preview.accept(7, 12));
        let redo = history.redo().unwrap().unwrap();
        assert_eq!(redo.invalidation, None);
        preview.submit(7, 13);
        preview.queue_refresh();
        assert!(!preview.accept(7, 13));
    }

    /// Proves a cancelled old worker frees occupancy without dropping its current queued successor.
    ///
    /// # Panics
    /// Panics if reconcile judges retired active authority current or old completion clears new work.
    #[test]
    fn cancelled_old_job_keeps_new_queued_request_through_reconcile() {
        let mut history = history();
        let mut controller = Controller::default();
        let old_capture = capture(&history, 7);
        let old = controller
            .request(old_capture.clone(), selection())
            .unwrap();
        let settings = PrintPreparationSettings::new(None, 0.5, 0.0).unwrap();
        assert!(apply_settings(&mut history, settings).unwrap());
        let fresh_capture = capture(&history, 7);
        controller.reconcile(Some(live(&fresh_capture)));
        assert_eq!(controller.status(), CheckStatus::OutOfDate);
        assert!(
            controller
                .request(fresh_capture.clone(), selection())
                .is_none()
        );
        controller.reconcile(Some(live(&fresh_capture)));
        assert_eq!(controller.status(), CheckStatus::Running);
        let successor = controller
            .complete(
                CheckCompletion {
                    id: old.id,
                    outcome: Ok(PreflightRasterOutcome::Cancelled),
                },
                Some(live(&fresh_capture)),
            )
            .expect("queued fresh request starts only after old completion");
        assert_ne!(successor.id, old.id);
        assert_eq!(controller.status(), CheckStatus::Running);
        assert!(
            controller
                .complete(
                    CheckCompletion {
                        id: old.id,
                        outcome: Ok(PreflightRasterOutcome::Cancelled)
                    },
                    Some(live(&fresh_capture)),
                )
                .is_none()
        );
        assert_eq!(controller.status(), CheckStatus::Running);
    }

    /// Proves cancellation, close, and stale edit statuses survive retired worker completion.
    ///
    /// # Panics
    /// Panics if a late terminal event republishes state after its owner moved on.
    #[test]
    fn retired_completions_preserve_cancel_close_and_stale_states() {
        for (action, expected) in [
            (0, CheckStatus::Cancelled),
            (1, CheckStatus::NotChecked),
            (2, CheckStatus::OutOfDate),
        ] {
            let history = history();
            let capture = capture(&history, 7);
            let mut controller = Controller::default();
            let job = controller.request(capture.clone(), selection()).unwrap();
            match action {
                0 => controller.cancel(),
                1 => controller.close(),
                _ => controller.reconcile(Some(LiveAuthority {
                    frame: 1,
                    ..live(&capture)
                })),
            }
            assert!(
                controller
                    .complete(
                        CheckCompletion {
                            id: job.id,
                            outcome: Ok(PreflightRasterOutcome::Cancelled)
                        },
                        Some(live(&capture)),
                    )
                    .is_none()
            );
            assert_eq!(controller.status(), expected);
        }
    }

    /// Proves the selected final target is preserved when the app's independent budget rejects it.
    ///
    /// # Panics
    /// Panics if analysis silently reduces the output size or calls a worker.
    #[test]
    fn oversized_review_is_unavailable_without_downsampling() {
        let history = history();
        let mut selected = selection();
        selected.target = OutputRasterTarget::new(5000, 5000).unwrap();
        let mut controller = Controller::default();
        assert!(controller.request(capture(&history, 7), selected).is_none());
        assert_eq!(controller.status(), CheckStatus::Unavailable);
        assert_eq!(controller.selection.unwrap().target, selected.target);
        assert!(controller.unavailable_reason().is_some());
    }

    /// Proves request IDs never saturate into reuse and the busy state stays empty.
    ///
    /// # Panics
    /// Panics if exhausted controller identity aliases a previous request.
    #[test]
    fn request_identity_exhaustion_is_unavailable() {
        let history = history();
        let mut controller = Controller {
            next_id: u64::MAX,
            ..Controller::default()
        };
        assert!(
            controller
                .request(capture(&history, 7), selection())
                .is_none()
        );
        assert_eq!(controller.status(), CheckStatus::Unavailable);
        assert!(controller.active.is_none());
        assert_eq!(controller.next_id, u64::MAX);
    }

    /// Admits a real completed pair, then hides it on every tested live-authority change.
    ///
    /// This uses the immutable current schema-10 fixture only as an input. The
    /// produced exact pair stays private to the controller; no GTK paintable or
    /// highlight is retained after a stale selection.
    ///
    /// # Panics
    /// Panics if stale document, physical Undo/Redo, frame, workspace, or PNG
    /// selection can retain or revive the accepted raster/report pair.
    #[test]
    fn accepted_pair_is_cleared_on_live_authority_changes() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../toniator-io/tests/fixtures/garment-schema10-clean-fade.toniator");
        let loaded = toniator_io::load(&fixture).unwrap();
        for change in [
            "document",
            "undo",
            "redo",
            "frame",
            "workspace",
            "target",
            "aa",
            "backing",
            "backing_provenance",
        ] {
            let mut history =
                DocumentHistory::new(DocumentSession::new(loaded.document().clone()).unwrap());
            let settings = PrintPreparationSettings::new(None, 0.5, 0.0).unwrap();
            if matches!(change, "undo" | "redo") {
                assert!(apply_settings(&mut history, settings.clone()).unwrap());
            }
            if change == "redo" {
                history.undo().unwrap();
            }
            let capture = ExportCapture {
                session: history.session().clone(),
                sources: loaded.sources().clone(),
                frame: 0,
                lifecycle_generation: 3,
                workspace_generation: 7,
            };
            let mut selected = selection();
            selected.target = OutputRasterTarget::new(64, 24).unwrap();
            let mut controller = Controller::default();
            let job = controller.request(capture.clone(), selected).unwrap();
            let old_id = job.id;
            let completion = job.run();
            assert!(
                controller
                    .complete(completion, Some(live(&capture)))
                    .is_none(),
                "{change}"
            );
            assert_eq!(
                controller.status(),
                CheckStatus::CurrentAdvisory,
                "{change}"
            );
            assert!(controller.accepted().is_some(), "{change}");

            match change {
                "document" => {
                    assert!(apply_settings(&mut history, settings).unwrap());
                    controller.reconcile(Some(LiveAuthority {
                        token: history.session().document_evaluation_token(),
                        ..live(&capture)
                    }));
                }
                "undo" => {
                    history.undo().unwrap();
                    controller.reconcile(Some(LiveAuthority {
                        token: history.session().document_evaluation_token(),
                        ..live(&capture)
                    }));
                }
                "redo" => {
                    history.redo().unwrap();
                    controller.reconcile(Some(LiveAuthority {
                        token: history.session().document_evaluation_token(),
                        ..live(&capture)
                    }));
                }
                "frame" => controller.reconcile(Some(LiveAuthority {
                    frame: 1,
                    ..live(&capture)
                })),
                "workspace" => controller.reconcile(Some(LiveAuthority {
                    workspace_generation: 8,
                    ..live(&capture)
                })),
                "target" => {
                    selected.target = OutputRasterTarget::new(65, 24).unwrap();
                    controller.select(selected);
                }
                "aa" => {
                    selected.antialiasing = RasterAntialiasing::Off;
                    controller.select(selected);
                }
                "backing" => {
                    selected.background = BackgroundChoice::Explicit(RasterBackground::OpaqueWhite);
                    controller.select(selected);
                }
                "backing_provenance" => {
                    selected.background =
                        BackgroundChoice::Explicit(selected.background_for(&capture.session));
                    controller.select(selected);
                }
                _ => unreachable!(),
            }
            assert_eq!(controller.status(), CheckStatus::OutOfDate, "{change}");
            assert!(controller.accepted().is_none(), "{change}");
            assert!(
                controller
                    .complete(
                        CheckCompletion {
                            id: old_id,
                            outcome: Ok(PreflightRasterOutcome::Cancelled)
                        },
                        Some(live(&capture)),
                    )
                    .is_none()
            );
            assert!(controller.accepted().is_none(), "{change}");
        }
    }
}
