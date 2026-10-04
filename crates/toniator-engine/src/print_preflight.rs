//! Headless, bounded advisory measurements of the selected final transparent composition.
//!
//! All finding locations are output pixel coordinates with exclusive high bounds. An
//! advisory is never a certification that artwork will print successfully.

use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use sha2::{Digest, Sha256};
use toniator_domain::{DocumentEvaluationToken, DocumentSession, PrintPreparationSettings};
use toniator_sampling::{SourceIdentity, media::FrameSource};

use crate::{
    EvaluationLimits, EvaluationRunError, FrameIdentity, OutputRasterTarget, RasterAntialiasing,
    RasterBackground, RasterSurface, evaluate_cancellable_with_limits, frame_evaluation_request,
    raster_output_identity,
};

/// Versioned final-alpha interpretation and sampled physical-stencil policy.
pub const PREFLIGHT_ALGORITHM_ID: &str = "toniator-garment-preflight-g1b-v1";
/// The exact final-alpha support threshold, including quantized alpha-1 pixels.
pub const SUPPORT_ALPHA: u8 = 1;
/// A stronger-coverage reference, not an opacity or print-safety threshold.
pub const CORE_ALPHA: u8 = 128;
/// Provisional proximity to a same-component core in output pixels.
pub const CORE_BOUNDARY_BAND_PIXELS: i32 = 2;
/// Dimensionless floating-point roundoff allowance on normalized disk inclusion.
pub const DISK_ROUNDOFF_FACTOR: f64 = 32.0;
const CANCEL_POLL_INTERVAL: u64 = 4096;
// Covers simultaneously live component/candidate representations, per-record
// Vec headers, allocator growth and duplicated highlight runs conservatively.
const SCRATCH_BYTES_PER_RECORD: u64 = 512;

/// The chosen output policy and resolved project-owned print settings for one still/current frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PreflightSelection {
    pub frame: u64,
    pub target: OutputRasterTarget,
    pub antialiasing: RasterAntialiasing,
    pub selected_background: RasterBackground,
    pub settings: PrintPreparationSettings,
}

/// Explicit first-version limits on raster, scratch, stencil, findings, and analysis work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreflightLimits {
    pub max_pixels: u64,
    pub max_scratch_bytes: u64,
    pub max_stencil_offsets: u64,
    pub max_records: u64,
    pub max_operations: u64,
}

impl Default for PreflightLimits {
    /// Supplies bounded headless defaults; a limit is not a print-quality recommendation.
    fn default() -> Self {
        Self {
            max_pixels: 8_388_608,
            max_scratch_bytes: 256 * 1024 * 1024,
            max_stencil_offsets: 65_536,
            max_records: 100_000,
            max_operations: 300_000_000,
        }
    }
}

/// One exact horizontal output-pixel run, suitable for future highlight projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRun {
    pub y: u32,
    pub x0: u32,
    pub x1_exclusive: u32,
}

/// One output-pixel rectangle with exclusive maximum coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelBounds {
    pub x0: u32,
    pub y0: u32,
    pub x1_exclusive: u32,
    pub y1_exclusive: u32,
}

/// One eight-connected alpha-support component with exact highlight runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupportComponent {
    pub id: u32,
    pub pixels: u64,
    pub bounds: PixelBounds,
    pub runs: Vec<PixelRun>,
    pub max_alpha: u8,
    pub core_pixels: u64,
    pub low_coverage_pixels: u64,
    pub fractional_alpha_pixels: u64,
    pub boundary_associated_low_coverage_pixels: u64,
    pub unresolved_low_coverage_pixels: u64,
    pub unresolved_low_coverage_runs: Vec<PixelRun>,
    pub touches_canvas: bool,
}

/// Distinguishes material support from the stronger-coverage reference mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskBasis {
    Support,
    Core,
}

/// Candidate nature; none of these values asserts an exact continuous stroke width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeCandidateKind {
    NoErodedCenter,
    ProbeSensitiveResidual,
    PossibleConstriction,
    CanvasBoundary,
}

/// Advisory positive feature or negative gap with exact output-pixel highlight runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeCandidate {
    pub basis: MaskBasis,
    pub negative_gap: bool,
    pub kind: ProbeCandidateKind,
    pub original_component_id: u32,
    pub pixels: u64,
    pub bounds: PixelBounds,
    pub runs: Vec<PixelRun>,
    pub exterior_connected: bool,
}

/// Width check availability, independent for positive and negative settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidthCheckStatus {
    Disabled,
    UnavailablePlacement,
    SamplingLimited,
    AdvisoryCandidates,
    NoWidthCandidatesWithUnresolved,
}

/// Overall completed state; per-check status carries disabled and unavailable detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreflightStatus {
    CompletedAdvisory,
}

/// Exact provenance of the analyzed final transparent output and runtime selection.
#[derive(Clone, Debug, PartialEq)]
pub struct PreflightIdentity {
    input_stamp: crate::SourceCacheKey,
    token: DocumentEvaluationToken,
    source_reference_id: String,
    source: SourceIdentity,
    source_frame: Option<FrameIdentity>,
    frame: u64,
    scene_fingerprint: String,
    transparent_raster_identity: String,
    alpha_sha256: String,
    target: OutputRasterTarget,
    antialiasing: RasterAntialiasing,
    selected_background: RasterBackground,
    settings: PrintPreparationSettings,
    alpha_policy: &'static str,
    algorithm: &'static str,
}

impl PreflightIdentity {
    /// Returns the captured document authority token.
    pub const fn token(&self) -> DocumentEvaluationToken {
        self.token
    }
    /// Returns the selected source reference at measurement time.
    pub fn source_reference_id(&self) -> &str {
        &self.source_reference_id
    }
    /// Returns the resolved source identity at measurement time.
    pub const fn source(&self) -> &SourceIdentity {
        &self.source
    }
    /// Returns the resolved frame identity when the source is timed media.
    pub const fn source_frame(&self) -> Option<&FrameIdentity> {
        self.source_frame.as_ref()
    }
    /// Returns the selected source frame index.
    pub const fn frame(&self) -> u64 {
        self.frame
    }
    /// Returns the evaluated canonical scene fingerprint.
    pub fn scene_fingerprint(&self) -> &str {
        &self.scene_fingerprint
    }
    /// Returns the evaluated transparent output raster identity.
    pub fn transparent_raster_identity(&self) -> &str {
        &self.transparent_raster_identity
    }
    /// Returns a SHA-256 digest of the final output alpha bytes.
    pub fn alpha_sha256(&self) -> &str {
        &self.alpha_sha256
    }
    /// Returns the actual final output raster target.
    pub const fn target(&self) -> OutputRasterTarget {
        self.target
    }
    /// Returns the actual raster antialiasing selection.
    pub const fn antialiasing(&self) -> RasterAntialiasing {
        self.antialiasing
    }
    /// Returns the user-selected export backing, separate from analyzed transparency.
    pub const fn selected_background(&self) -> RasterBackground {
        self.selected_background
    }
    /// Returns the captured project-owned print settings.
    pub const fn settings(&self) -> &PrintPreparationSettings {
        &self.settings
    }
    /// Returns the versioned final-alpha policy label.
    pub const fn alpha_policy(&self) -> &'static str {
        self.alpha_policy
    }
    /// Returns the versioned preflight algorithm label.
    pub const fn algorithm(&self) -> &'static str {
        self.algorithm
    }

    /// Resolves the current provider frame through engine authority and rejects
    /// changed source bytes/timing even under the same document ID and revision.
    ///
    /// This repeats only frame resolution, never geometry, rasterization, or
    /// morphology. The stored input stamp is private and cannot be caller-forged.
    ///
    /// # Errors
    /// Returns the current source/frame diagnostic or cancellation before a
    /// stale report can be published as current.
    pub fn is_current(
        &self,
        session: &DocumentSession,
        media: &mut dyn FrameSource,
        selection: &PreflightSelection,
        cancelled: &AtomicBool,
    ) -> Result<bool, PreflightError> {
        if !self.matches_session_selection_only(session, selection) {
            return Ok(false);
        }
        if cancelled.load(Ordering::Acquire) {
            return Err(PreflightError::new(
                "preflight.cancelled",
                "currentness check was cancelled",
            ));
        }
        let request = match frame_evaluation_request(session, media, selection.frame, &|| {
            cancelled.load(Ordering::Acquire)
        }) {
            Ok(request) => request,
            Err(error)
                if cancelled.load(Ordering::Acquire) || error.path().ends_with("cancelled") =>
            {
                return Err(PreflightError::new(
                    "preflight.cancelled",
                    "currentness check was cancelled",
                ));
            }
            Err(error) => return Err(PreflightError::new("preflight.source", error.to_string())),
        };
        if cancelled.load(Ordering::Acquire) {
            return Err(PreflightError::new(
                "preflight.cancelled",
                "currentness check was cancelled",
            ));
        }
        Ok(
            request.snapshot.token() == self.token
                && request.source.cache_key() == self.input_stamp,
        )
    }

    /// Checks session and runtime selection, without claiming external source freshness.
    fn matches_session_selection_only(
        &self,
        session: &DocumentSession,
        selection: &PreflightSelection,
    ) -> bool {
        session.accepts_document_evaluation(self.token)
            && session.document().print_preparation() == &self.settings
            && matches!(session.document().source(), toniator_domain::SourceReference::Assigned(id)
                if id.as_str() == self.source_reference_id)
            && self.frame == selection.frame
            && self.target == selection.target
            && self.antialiasing == selection.antialiasing
            && self.selected_background == selection.selected_background
            && self.settings == selection.settings
            && self.alpha_policy == "support>=1;core>=128;8fg/4bg;same-component-band2"
            && self.algorithm == PREFLIGHT_ALGORITHM_ID
    }
}

/// Complete final-alpha inventory and advisory morphology findings.
#[derive(Clone, Debug, PartialEq)]
pub struct PreflightReport {
    pub identity: PreflightIdentity,
    pub status: PreflightStatus,
    pub positive_status: WidthCheckStatus,
    pub negative_status: WidthCheckStatus,
    pub ppi_axes: Option<(f64, f64)>,
    pub occupied_bounds: Option<PixelBounds>,
    pub alpha_histogram: Vec<u64>,
    pub support_components: Vec<SupportComponent>,
    pub positive_candidates: Vec<ProbeCandidate>,
    pub negative_candidates: Vec<ProbeCandidate>,
    pub sampling_guard_mm_axes: Option<(f64, f64)>,
    pub sampling_note: Option<String>,
    pub operations: u64,
}

/// A complete result, an explicit resource-unavailable outcome, or cancellation.
#[derive(Clone, Debug, PartialEq)]
pub enum PreflightOutcome {
    Complete(Box<PreflightReport>),
    Unavailable { reason: String },
    Cancelled,
}

/// One inseparable final transparent raster and advisory from the same evaluation.
///
/// Private fields prevent callers from attaching a report to different pixels. The
/// retained raster shares the evaluator's immutable allocation without cloning RGBA.
#[derive(Clone, Debug, PartialEq)]
pub struct PreflightRasterReport {
    report: Box<PreflightReport>,
    raster: Arc<RasterSurface>,
}

impl PreflightRasterReport {
    /// Borrows the report measured from this exact raster.
    pub fn report(&self) -> &PreflightReport {
        &self.report
    }

    /// Borrows the exact transparent final output measured by the report.
    pub fn raster(&self) -> &RasterSurface {
        &self.raster
    }

    /// Consumes the pair for the existing report-only CLI contract.
    fn into_report(self) -> Box<PreflightReport> {
        self.report
    }
}

/// Complete inseparable pair, explicit resource unavailability, or cancellation.
#[derive(Clone, Debug, PartialEq)]
pub enum PreflightRasterOutcome {
    Complete(Box<PreflightRasterReport>),
    Unavailable { reason: String },
    Cancelled,
}

/// Invalid authority or failed upstream source/evaluation; no report is published.
#[derive(Debug)]
pub struct PreflightError {
    path: &'static str,
    message: String,
}

impl PreflightError {
    /// Returns the stable diagnostic location.
    pub const fn path(&self) -> &'static str {
        self.path
    }
    /// Returns the explanatory diagnostic without exposing partial findings.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// Builds one stable headless diagnostic.
    fn new(path: &'static str, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
        }
    }
}

impl fmt::Display for PreflightError {
    /// Formats one diagnostic for the CLI boundary.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for PreflightError {}

/// Runs the existing frame evaluator with a trusted transparent final-output request.
///
/// The selected backing is recorded only; the analyzed RGBA is never flattened.
/// Cancellation and budget failure never publish a partial clean report.
///
/// # Errors
/// Rejects invalid settings or mismatched session authority, source/frame failures,
/// and ordinary evaluator failures before publishing a report.
pub fn preflight_current_frame(
    session: &DocumentSession,
    media: &mut dyn FrameSource,
    selection: &PreflightSelection,
    evaluation_limits: EvaluationLimits,
    limits: PreflightLimits,
    cancelled: &AtomicBool,
) -> Result<PreflightOutcome, PreflightError> {
    Ok(
        match preflight_current_frame_with_raster(
            session,
            media,
            selection,
            evaluation_limits,
            limits,
            cancelled,
        )? {
            PreflightRasterOutcome::Complete(pair) => {
                PreflightOutcome::Complete(pair.into_report())
            }
            PreflightRasterOutcome::Unavailable { reason } => {
                PreflightOutcome::Unavailable { reason }
            }
            PreflightRasterOutcome::Cancelled => PreflightOutcome::Cancelled,
        },
    )
}

/// Evaluates one final transparent output and retains its exact RGBA with its advisory.
///
/// The selected backing is identity metadata only. All analysis consumes the
/// evaluator's transparent raster; neither the report nor this adapter runs a
/// second rasterization. The caller must still check `identity.is_current` and
/// live app selection before admitting a completed pair.
///
/// # Errors
/// Rejects invalid settings or mismatched session authority, source/frame
/// failures, and evaluator failures before publishing either member of a pair.
pub fn preflight_current_frame_with_raster(
    session: &DocumentSession,
    media: &mut dyn FrameSource,
    selection: &PreflightSelection,
    evaluation_limits: EvaluationLimits,
    limits: PreflightLimits,
    cancelled: &AtomicBool,
) -> Result<PreflightRasterOutcome, PreflightError> {
    selection
        .settings
        .validate()
        .map_err(|error| PreflightError::new("preflight.settings", error.to_string()))?;
    if session.document().print_preparation() != &selection.settings {
        return Err(PreflightError::new(
            "preflight.settings",
            "selection does not match the session's print authority",
        ));
    }
    if cancelled.load(Ordering::Acquire) {
        return Ok(PreflightRasterOutcome::Cancelled);
    }
    let pixels = u64::from(selection.target.width())
        .checked_mul(u64::from(selection.target.height()))
        .ok_or_else(|| PreflightError::new("preflight.target", "pixel count overflow"))?;
    if let Some(reason) = limits.preflight_reason(pixels) {
        return Ok(PreflightRasterOutcome::Unavailable { reason });
    }
    let request = match frame_evaluation_request(session, media, selection.frame, &|| {
        cancelled.load(Ordering::Acquire)
    }) {
        Ok(request) => request,
        Err(error) if cancelled.load(Ordering::Acquire) || error.path().ends_with("cancelled") => {
            return Ok(PreflightRasterOutcome::Cancelled);
        }
        Err(error) => return Err(PreflightError::new("preflight.source", error.to_string())),
    };
    let input_stamp = request.source.cache_key();
    let source_frame = match &request.source.data {
        crate::ResolvedSourceData::Frame(frame) => Some(frame.identity.clone()),
        crate::ResolvedSourceData::Encoded { .. } => None,
    };
    let evaluation = match evaluate_cancellable_with_limits(
        request.for_output(
            RasterBackground::Transparent,
            Some(selection.target),
            selection.antialiasing,
        ),
        evaluation_limits,
        cancelled,
    ) {
        Ok(result) => result,
        Err(EvaluationRunError::Cancelled) => return Ok(PreflightRasterOutcome::Cancelled),
        Err(EvaluationRunError::Evaluation(error)) => {
            return Err(PreflightError::new(
                "preflight.evaluation",
                error.to_string(),
            ));
        }
    };
    if cancelled.load(Ordering::Acquire) {
        return Ok(PreflightRasterOutcome::Cancelled);
    }
    let mut alpha_hash = Sha256::new();
    let mut budget = Budget::new(limits, cancelled);
    for pixel in evaluation.raster().pixels().chunks_exact(4) {
        match budget.step() {
            Ok(()) => alpha_hash.update([pixel[3]]),
            Err(Halt::Cancelled) => return Ok(PreflightRasterOutcome::Cancelled),
            Err(Halt::Unavailable(reason)) => {
                return Ok(PreflightRasterOutcome::Unavailable { reason });
            }
        }
    }
    let source_reference_id = match session.document().source() {
        toniator_domain::SourceReference::Assigned(id) => id.as_str().to_owned(),
        toniator_domain::SourceReference::Unassigned => {
            return Err(PreflightError::new(
                "preflight.source",
                "source must be assigned",
            ));
        }
    };
    let identity = PreflightIdentity {
        input_stamp,
        token: evaluation.token(),
        source_reference_id,
        source: evaluation.source_identity().clone(),
        source_frame,
        frame: selection.frame,
        scene_fingerprint: evaluation.scene().identity().scene_fingerprint().to_owned(),
        transparent_raster_identity: raster_output_identity(
            evaluation.scene(),
            RasterBackground::Transparent,
            Some(selection.target),
            selection.antialiasing,
        ),
        alpha_sha256: format!("{:x}", alpha_hash.finalize()),
        target: selection.target,
        antialiasing: selection.antialiasing,
        selected_background: selection.selected_background,
        settings: selection.settings.clone(),
        alpha_policy: "support>=1;core>=128;8fg/4bg;same-component-band2",
        algorithm: PREFLIGHT_ALGORITHM_ID,
    };
    let raster = Arc::clone(&evaluation.raster);
    match analyze_alpha(
        evaluation.raster().pixels(),
        selection,
        identity,
        &mut budget,
    ) {
        Ok(report) => Ok(PreflightRasterOutcome::Complete(Box::new(
            PreflightRasterReport {
                report: Box::new(report),
                raster,
            },
        ))),
        Err(Halt::Cancelled) => Ok(PreflightRasterOutcome::Cancelled),
        Err(Halt::Unavailable(reason)) => Ok(PreflightRasterOutcome::Unavailable { reason }),
    }
}

impl PreflightLimits {
    /// Checks essential nonzero ceilings and a conservative initial scratch estimate.
    fn preflight_reason(self, pixels: u64) -> Option<String> {
        if self.max_pixels == 0
            || self.max_scratch_bytes == 0
            || self.max_stencil_offsets == 0
            || self.max_records == 0
            || self.max_operations == 0
        {
            return Some("preflight limits must all be positive".into());
        }
        if pixels > self.max_pixels {
            return Some("final output exceeds preflight pixel limit".into());
        }
        // Peak includes RGBA plus overlapping masks, labels, queue and candidate records;
        // upstream source/provider/evaluator allocations are outside this detector scratch cap.
        let Some(bytes) = pixels
            .checked_mul(30)
            .and_then(|used| {
                self.max_records
                    .checked_mul(SCRATCH_BYTES_PER_RECORD)
                    .and_then(|records| used.checked_add(records))
            })
            .and_then(|used| {
                self.max_stencil_offsets
                    .checked_mul(8)
                    .and_then(|stencil| used.checked_add(stencil))
            })
        else {
            return Some("preflight scratch estimate overflowed".into());
        };
        (bytes > self.max_scratch_bytes).then(|| "preflight scratch estimate exceeds limit".into())
    }
}

#[derive(Debug)]
enum Halt {
    Cancelled,
    Unavailable(String),
}

struct Budget<'a> {
    limits: PreflightLimits,
    cancelled: &'a AtomicBool,
    operations: u64,
    records: u64,
}

impl<'a> Budget<'a> {
    /// Starts a shared request-wide operations and result-record account.
    fn new(limits: PreflightLimits, cancelled: &'a AtomicBool) -> Self {
        Self {
            limits,
            cancelled,
            operations: 0,
            records: 0,
        }
    }

    /// Counts one bounded operation and polls cancellation at fixed intervals.
    fn step(&mut self) -> Result<(), Halt> {
        self.charge(1)
    }

    /// Charges at most one polling interval of bulk initialization or scanning.
    fn charge(&mut self, count: u64) -> Result<(), Halt> {
        self.operations = self
            .operations
            .checked_add(count)
            .ok_or_else(|| Halt::Unavailable("preflight operation count overflowed".into()))?;
        if self.operations > self.limits.max_operations {
            return Err(Halt::Unavailable("preflight work limit exceeded".into()));
        }
        if (count > 1 || self.operations.is_multiple_of(CANCEL_POLL_INTERVAL))
            && self.cancelled.load(Ordering::Acquire)
        {
            return Err(Halt::Cancelled);
        }
        Ok(())
    }

    /// Polls immediately after one bounded bulk write whose work was precharged.
    fn poll_after_bulk(&self) -> Result<(), Halt> {
        if self.cancelled.load(Ordering::Acquire) {
            Err(Halt::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Bounds every stored component, candidate, and highlight run.
    fn record(&mut self) -> Result<(), Halt> {
        self.records = self
            .records
            .checked_add(1)
            .ok_or_else(|| Halt::Unavailable("preflight record count overflowed".into()))?;
        if self.records > self.limits.max_records {
            Err(Halt::Unavailable(
                "preflight result record limit exceeded".into(),
            ))
        } else {
            Ok(())
        }
    }
}

/// Allocates an explicitly bounded zero-initialized pixel scratch buffer.
fn zeroed_u8(length: usize, budget: &mut Budget<'_>) -> Result<Vec<u8>, Halt> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| Halt::Unavailable("preflight mask allocation failed".into()))?;
    while result.len() < length {
        let next = result
            .len()
            .saturating_add(CANCEL_POLL_INTERVAL as usize)
            .min(length);
        budget.charge((next - result.len()) as u64)?;
        result.resize(next, 0);
        budget.poll_after_bulk()?;
    }
    Ok(result)
}

/// Allocates checked component labels without an implicit unbounded reserve.
fn zeroed_u32(length: usize, budget: &mut Budget<'_>) -> Result<Vec<u32>, Halt> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| Halt::Unavailable("preflight label allocation failed".into()))?;
    while result.len() < length {
        let next = result
            .len()
            .saturating_add(CANCEL_POLL_INTERVAL as usize)
            .min(length);
        budget.charge((next - result.len()) as u64)?;
        result.resize(next, 0);
        budget.poll_after_bulk()?;
    }
    Ok(result)
}

/// Internal connected region in a positive or negative binary mask.
struct Component {
    id: u32,
    pixels: u64,
    bounds: PixelBounds,
    runs: Vec<PixelRun>,
    touches_frame: bool,
}

/// Complete labels and deterministic row-major components for one binary mask.
struct Labeled {
    labels: Vec<u32>,
    components: Vec<Component>,
}

/// Complete alpha scan before optional physical morphology runs.
struct AlphaInventory {
    histogram: Vec<u64>,
    support: Vec<u8>,
    core: Vec<u8>,
    occupied: Option<PixelBounds>,
    components: Vec<SupportComponent>,
}

/// Labels all foreground pixels with 8-connectivity or background with 4-connectivity.
///
/// The queue and labels are allocated under the request's checked scratch plan;
/// all graph visits and emitted exact pixel runs consume the shared budgets.
fn label_mask(
    mask: &[u8],
    width: u32,
    height: u32,
    eight_connected: bool,
    with_runs: bool,
    budget: &mut Budget<'_>,
) -> Result<Labeled, Halt> {
    let w = usize::try_from(width)
        .map_err(|_| Halt::Unavailable("width does not fit memory".into()))?;
    let h = usize::try_from(height)
        .map_err(|_| Halt::Unavailable("height does not fit memory".into()))?;
    let length = w
        .checked_mul(h)
        .ok_or_else(|| Halt::Unavailable("label extent overflowed".into()))?;
    if mask.len() != length {
        return Err(Halt::Unavailable(
            "label mask extent disagrees with target".into(),
        ));
    }
    let mut labels = zeroed_u32(length, budget)?;
    let mut queue = Vec::new();
    queue
        .try_reserve_exact(length)
        .map_err(|_| Halt::Unavailable("preflight component queue allocation failed".into()))?;
    let mut components = Vec::new();
    let neighbours: &[(i32, i32)] = if eight_connected {
        &[
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ]
    } else {
        &[(0, -1), (-1, 0), (1, 0), (0, 1)]
    };
    for index in 0..length {
        budget.step()?;
        if mask[index] == 0 || labels[index] != 0 {
            continue;
        }
        budget.record()?;
        let id = u32::try_from(components.len() + 1)
            .map_err(|_| Halt::Unavailable("component ID capacity exceeded".into()))?;
        queue.clear();
        queue.push(index);
        labels[index] = id;
        let mut head = 0;
        let mut pixels = 0_u64;
        let mut bounds = PixelBounds {
            x0: width,
            y0: height,
            x1_exclusive: 0,
            y1_exclusive: 0,
        };
        let mut touches_frame = false;
        while head < queue.len() {
            budget.step()?;
            let current = queue[head];
            head += 1;
            let x = current % w;
            let y = current / w;
            pixels += 1;
            let x32 = x as u32;
            let y32 = y as u32;
            bounds.x0 = bounds.x0.min(x32);
            bounds.y0 = bounds.y0.min(y32);
            bounds.x1_exclusive = bounds.x1_exclusive.max(x32 + 1);
            bounds.y1_exclusive = bounds.y1_exclusive.max(y32 + 1);
            touches_frame |= x == 0 || y == 0 || x + 1 == w || y + 1 == h;
            for &(dx, dy) in neighbours {
                budget.step()?;
                let nx = x as i64 + i64::from(dx);
                let ny = y as i64 + i64::from(dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let next = ny as usize * w + nx as usize;
                if mask[next] != 0 && labels[next] == 0 {
                    labels[next] = id;
                    queue.push(next);
                }
            }
        }
        components
            .try_reserve(1)
            .map_err(|_| Halt::Unavailable("component record allocation failed".into()))?;
        components.push(Component {
            id,
            pixels,
            bounds,
            runs: Vec::new(),
            touches_frame,
        });
    }
    if with_runs {
        for y in 0..h {
            let mut x = 0;
            while x < w {
                budget.step()?;
                let id = labels[y * w + x];
                if id == 0 {
                    x += 1;
                    continue;
                }
                let start = x;
                x += 1;
                while x < w && labels[y * w + x] == id {
                    budget.step()?;
                    x += 1;
                }
                budget.record()?;
                components[id as usize - 1]
                    .runs
                    .try_reserve(1)
                    .map_err(|_| Halt::Unavailable("component run allocation failed".into()))?;
                components[id as usize - 1].runs.push(PixelRun {
                    y: y as u32,
                    x0: start as u32,
                    x1_exclusive: x as u32,
                });
            }
        }
    }
    Ok(Labeled { labels, components })
}

/// Scans actual straight-alpha output into full histogram, support/core masks, and highlightable support inventory.
fn inventory_alpha(
    rgba: &[u8],
    width: u32,
    height: u32,
    budget: &mut Budget<'_>,
) -> Result<AlphaInventory, Halt> {
    let length = usize::try_from(u64::from(width) * u64::from(height))
        .map_err(|_| Halt::Unavailable("raster length does not fit memory".into()))?;
    if rgba.len()
        != length
            .checked_mul(4)
            .ok_or_else(|| Halt::Unavailable("RGBA byte length overflowed".into()))?
    {
        return Err(Halt::Unavailable("raster byte extent is invalid".into()));
    }
    let mut histogram = Vec::new();
    histogram
        .try_reserve_exact(256)
        .map_err(|_| Halt::Unavailable("alpha histogram allocation failed".into()))?;
    histogram.resize(256, 0_u64);
    let mut support = zeroed_u8(length, budget)?;
    let mut core = zeroed_u8(length, budget)?;
    let mut occupied = None::<PixelBounds>;
    let w = width as usize;
    for (index, pixel) in rgba.chunks_exact(4).enumerate() {
        budget.step()?;
        let alpha = pixel[3];
        histogram[alpha as usize] += 1;
        if alpha >= SUPPORT_ALPHA {
            support[index] = 1;
            let x = (index % w) as u32;
            let y = (index / w) as u32;
            occupied = Some(match occupied {
                None => PixelBounds {
                    x0: x,
                    y0: y,
                    x1_exclusive: x + 1,
                    y1_exclusive: y + 1,
                },
                Some(mut bounds) => {
                    bounds.x0 = bounds.x0.min(x);
                    bounds.y0 = bounds.y0.min(y);
                    bounds.x1_exclusive = bounds.x1_exclusive.max(x + 1);
                    bounds.y1_exclusive = bounds.y1_exclusive.max(y + 1);
                    bounds
                }
            });
        }
        if alpha >= CORE_ALPHA {
            core[index] = 1;
        }
    }
    let labeled = label_mask(&support, width, height, true, true, budget)?;
    let mut components: Vec<SupportComponent> = Vec::new();
    components
        .try_reserve_exact(labeled.components.len())
        .map_err(|_| Halt::Unavailable("support inventory allocation failed".into()))?;
    for component in labeled.components {
        budget.step()?;
        components.push(SupportComponent {
            id: component.id,
            pixels: component.pixels,
            bounds: component.bounds,
            runs: component.runs,
            max_alpha: 0,
            core_pixels: 0,
            low_coverage_pixels: 0,
            fractional_alpha_pixels: 0,
            boundary_associated_low_coverage_pixels: 0,
            unresolved_low_coverage_pixels: 0,
            unresolved_low_coverage_runs: Vec::new(),
            touches_canvas: component.touches_frame,
        });
    }
    let mut unresolved = zeroed_u8(length, budget)?;
    let h = height as usize;
    for index in 0..length {
        budget.step()?;
        let id = labeled.labels[index];
        if id == 0 {
            continue;
        }
        let alpha = rgba[index * 4 + 3];
        let component = &mut components[id as usize - 1];
        component.max_alpha = component.max_alpha.max(alpha);
        if alpha >= CORE_ALPHA {
            component.core_pixels += 1;
        }
        if alpha < 255 {
            component.fractional_alpha_pixels += 1;
        }
        if alpha < CORE_ALPHA {
            component.low_coverage_pixels += 1;
        }
    }
    for index in 0..length {
        budget.step()?;
        let id = labeled.labels[index];
        if id == 0 || rgba[index * 4 + 3] >= CORE_ALPHA {
            continue;
        }
        let component = &mut components[id as usize - 1];
        let x = index % w;
        let y = index / w;
        let mut near_same_core = false;
        if component.core_pixels > 0 {
            for dy in -CORE_BOUNDARY_BAND_PIXELS..=CORE_BOUNDARY_BAND_PIXELS {
                for dx in -CORE_BOUNDARY_BAND_PIXELS..=CORE_BOUNDARY_BAND_PIXELS {
                    budget.step()?;
                    let nx = x as i64 + i64::from(dx);
                    let ny = y as i64 + i64::from(dy);
                    if nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64 {
                        let near = ny as usize * w + nx as usize;
                        if labeled.labels[near] == id && core[near] != 0 {
                            near_same_core = true;
                            break;
                        }
                    }
                }
                if near_same_core {
                    break;
                }
            }
        }
        if near_same_core {
            component.boundary_associated_low_coverage_pixels += 1;
        } else {
            component.unresolved_low_coverage_pixels += 1;
            unresolved[index] = 1;
        }
    }
    for y in 0..h {
        let mut x = 0;
        while x < w {
            budget.step()?;
            let index = y * w + x;
            if unresolved[index] == 0 {
                x += 1;
                continue;
            }
            let id = labeled.labels[index];
            let start = x;
            x += 1;
            while x < w && unresolved[y * w + x] != 0 && labeled.labels[y * w + x] == id {
                budget.step()?;
                x += 1;
            }
            budget.record()?;
            components[id as usize - 1]
                .unresolved_low_coverage_runs
                .try_reserve(1)
                .map_err(|_| Halt::Unavailable("unresolved run allocation failed".into()))?;
            components[id as usize - 1]
                .unresolved_low_coverage_runs
                .push(PixelRun {
                    y: y as u32,
                    x0: start as u32,
                    x1_exclusive: x as u32,
                });
        }
    }
    Ok(AlphaInventory {
        histogram,
        support,
        core,
        occupied,
        components,
    })
}

/// Integer-pixel offsets sampled from a physical disk at the selected axis pitches.
struct Stencil {
    offsets: Vec<(i32, i32)>,
    extent_x: u32,
    extent_y: u32,
}

/// Samples the versioned normalized-disk test with checked extents and loop work.
///
/// The dimensionless 32-epsilon roundoff allowance replaces the G0 script's
/// experimental absolute `1e-12 mm²`; neither value calibrates geometry.
fn physical_stencil(
    threshold_mm: f64,
    dx: f64,
    dy: f64,
    budget: &mut Budget<'_>,
) -> Result<Stencil, Halt> {
    let rx = (threshold_mm / 2.0) / dx;
    let ry = (threshold_mm / 2.0) / dy;
    if !rx.is_finite() || !ry.is_finite() || rx <= 0.0 || ry <= 0.0 {
        return Err(Halt::Unavailable(
            "physical stencil ratios are not representable".into(),
        ));
    }
    let tolerance = DISK_ROUNDOFF_FACTOR * f64::EPSILON;
    let multiplier = (1.0 + tolerance).sqrt();
    let ex = (rx * multiplier).ceil();
    let ey = (ry * multiplier).ceil();
    if !ex.is_finite()
        || !ey.is_finite()
        || ex > f64::from(i32::MAX / 2)
        || ey > f64::from(i32::MAX / 2)
    {
        return Err(Halt::Unavailable(
            "physical stencil extent is unsupported".into(),
        ));
    }
    let ex = ex as i32;
    let ey = ey as i32;
    let rectangular = (i64::from(ex) * 2 + 1)
        .checked_mul(i64::from(ey) * 2 + 1)
        .ok_or_else(|| Halt::Unavailable("physical stencil work overflowed".into()))?
        as u64;
    if rectangular
        > budget
            .limits
            .max_operations
            .saturating_sub(budget.operations)
    {
        return Err(Halt::Unavailable(
            "physical stencil exceeds work limit".into(),
        ));
    }
    let mut offsets = Vec::new();
    let mut has_origin = false;
    offsets
        .try_reserve_exact(
            usize::try_from(budget.limits.max_stencil_offsets)
                .map_err(|_| Halt::Unavailable("stencil limit does not fit memory".into()))?,
        )
        .map_err(|_| Halt::Unavailable("stencil allocation failed".into()))?;
    for j in -ey..=ey {
        for i in -ex..=ex {
            budget.step()?;
            let nx = f64::from(i) / rx;
            let ny = f64::from(j) / ry;
            if nx * nx + ny * ny <= 1.0 + tolerance {
                if offsets.len() as u64 >= budget.limits.max_stencil_offsets {
                    return Err(Halt::Unavailable(
                        "physical stencil offset limit exceeded".into(),
                    ));
                }
                if i == 0 && j == 0 {
                    has_origin = true;
                }
                offsets.push((i, j));
            }
        }
    }
    if !has_origin {
        return Err(Halt::Unavailable("physical stencil lost its origin".into()));
    }
    Ok(Stencil {
        offsets,
        extent_x: ex as u32,
        extent_y: ey as u32,
    })
}

/// Computes exact sampled erosion and opening residual on one bounded binary grid.
fn morphology(
    mask: &[u8],
    width: u32,
    height: u32,
    stencil: &Stencil,
    budget: &mut Budget<'_>,
) -> Result<(Vec<u8>, Vec<u8>), Halt> {
    let w = width as usize;
    let h = height as usize;
    let length = w
        .checked_mul(h)
        .ok_or_else(|| Halt::Unavailable("morphology extent overflowed".into()))?;
    let mut eroded = zeroed_u8(length, budget)?;
    let mut opened = zeroed_u8(length, budget)?;
    for index in 0..length {
        budget.step()?;
        if mask[index] == 0 {
            continue;
        }
        let x = index % w;
        let y = index / w;
        let mut survives = true;
        for &(ox, oy) in &stencil.offsets {
            budget.step()?;
            let nx = x as i64 + i64::from(ox);
            let ny = y as i64 + i64::from(oy);
            if nx < 0
                || ny < 0
                || nx >= w as i64
                || ny >= h as i64
                || mask[ny as usize * w + nx as usize] == 0
            {
                survives = false;
                break;
            }
        }
        if survives {
            eroded[index] = 1;
        }
    }
    for (index, &alive) in eroded.iter().enumerate() {
        budget.step()?;
        if alive == 0 {
            continue;
        }
        let x = index % w;
        let y = index / w;
        for &(ox, oy) in &stencil.offsets {
            budget.step()?;
            let nx = x as i64 + i64::from(ox);
            let ny = y as i64 + i64::from(oy);
            if nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64 {
                opened[ny as usize * w + nx as usize] = 1;
            }
        }
    }
    let mut residual = zeroed_u8(length, budget)?;
    for index in 0..length {
        budget.step()?;
        if mask[index] != 0 && opened[index] == 0 {
            residual[index] = 1;
        }
    }
    Ok((eroded, residual))
}

/// Counts 8-connected eroded islands per original positive component, or 4-connected negative islands.
fn eroded_island_counts(
    original: &Labeled,
    eroded: &Labeled,
    budget: &mut Budget<'_>,
) -> Result<Vec<u32>, Halt> {
    let mut original_of_eroded = zeroed_u32(
        eroded
            .components
            .len()
            .checked_add(1)
            .ok_or_else(|| Halt::Unavailable("eroded component span overflowed".into()))?,
        budget,
    )?;
    for index in 0..eroded.labels.len() {
        budget.step()?;
        let id = eroded.labels[index] as usize;
        if id != 0 && original_of_eroded[id] == 0 {
            original_of_eroded[id] = original.labels[index];
        }
    }
    let mut counts = zeroed_u32(
        original
            .components
            .len()
            .checked_add(1)
            .ok_or_else(|| Halt::Unavailable("original component span overflowed".into()))?,
        budget,
    )?;
    for original_id in original_of_eroded.into_iter().skip(1) {
        budget.step()?;
        if original_id == 0 {
            return Err(Halt::Unavailable(
                "eroded island lost its original component".into(),
            ));
        }
        counts[original_id as usize] += 1;
    }
    Ok(counts)
}

/// Charges and copies exact runs when a whole original component needs a warning.
fn copy_runs(runs: &[PixelRun], budget: &mut Budget<'_>) -> Result<Vec<PixelRun>, Halt> {
    let mut copied = Vec::new();
    copied
        .try_reserve_exact(runs.len())
        .map_err(|_| Halt::Unavailable("candidate run allocation failed".into()))?;
    for &run in runs {
        budget.step()?;
        budget.record()?;
        copied.push(run);
    }
    Ok(copied)
}

/// Emits one sampled positive-feature advisory family on S or C.
fn positive_probe(
    mask: &[u8],
    width: u32,
    height: u32,
    basis: MaskBasis,
    stencil: &Stencil,
    budget: &mut Budget<'_>,
) -> Result<Vec<ProbeCandidate>, Halt> {
    let original = label_mask(mask, width, height, true, true, budget)?;
    let (eroded, residual) = morphology(mask, width, height, stencil, budget)?;
    let eroded = label_mask(&eroded, width, height, true, false, budget)?;
    let counts = eroded_island_counts(&original, &eroded, budget)?;
    let mut candidates = Vec::new();
    for component in &original.components {
        budget.step()?;
        let count = counts[component.id as usize];
        let kind = if count == 0 {
            Some(ProbeCandidateKind::NoErodedCenter)
        } else if count >= 2 {
            Some(ProbeCandidateKind::PossibleConstriction)
        } else if component.touches_frame {
            Some(ProbeCandidateKind::CanvasBoundary)
        } else {
            None
        };
        if let Some(kind) = kind {
            budget.record()?;
            candidates
                .try_reserve(1)
                .map_err(|_| Halt::Unavailable("positive candidate allocation failed".into()))?;
            candidates.push(ProbeCandidate {
                basis,
                negative_gap: false,
                kind,
                original_component_id: component.id,
                pixels: component.pixels,
                bounds: component.bounds,
                runs: copy_runs(&component.runs, budget)?,
                exterior_connected: false,
            });
        }
    }
    let residual = label_mask(&residual, width, height, true, true, budget)?;
    for component in residual.components {
        budget.step()?;
        let first = component
            .runs
            .first()
            .ok_or_else(|| Halt::Unavailable("residual component has no run".into()))?;
        let original_id = original.labels[first.y as usize * width as usize + first.x0 as usize];
        budget.record()?;
        candidates
            .try_reserve(1)
            .map_err(|_| Halt::Unavailable("positive residual allocation failed".into()))?;
        candidates.push(ProbeCandidate {
            basis,
            negative_gap: false,
            kind: ProbeCandidateKind::ProbeSensitiveResidual,
            original_component_id: original_id,
            pixels: component.pixels,
            bounds: component.bounds,
            runs: component.runs,
            exterior_connected: false,
        });
    }
    Ok(candidates)
}

/// Probes negative background after virtual-exterior padding, then crops only report pixels.
///
/// The near-artwork mask is a reporting filter applied after morphology. It is
/// never eroded as though it were the complete negative background.
fn negative_probe(
    foreground: &[u8],
    width: u32,
    height: u32,
    basis: MaskBasis,
    stencil: &Stencil,
    budget: &mut Budget<'_>,
) -> Result<Vec<ProbeCandidate>, Halt> {
    let px = stencil
        .extent_x
        .checked_mul(2)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(|| Halt::Unavailable("negative padding x overflowed".into()))?;
    let py = stencil
        .extent_y
        .checked_mul(2)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(|| Halt::Unavailable("negative padding y overflowed".into()))?;
    let padded_width = width
        .checked_add(
            px.checked_mul(2)
                .ok_or_else(|| Halt::Unavailable("padded width overflowed".into()))?,
        )
        .ok_or_else(|| Halt::Unavailable("padded width overflowed".into()))?;
    let padded_height = height
        .checked_add(
            py.checked_mul(2)
                .ok_or_else(|| Halt::Unavailable("padded height overflowed".into()))?,
        )
        .ok_or_else(|| Halt::Unavailable("padded height overflowed".into()))?;
    let n = u64::from(width) * u64::from(height);
    let m = u64::from(padded_width)
        .checked_mul(u64::from(padded_height))
        .ok_or_else(|| Halt::Unavailable("padded pixel extent overflowed".into()))?;
    // At the eroded-label pass, 19 bytes per padded pixel may be live:
    // background, original labels, eroded/residual masks, new labels and queue.
    // Retained full-canvas RGBA, S/C and near mask plus later cropped labels/queue
    // are conservatively charged at 13 bytes per original pixel.
    let estimate = n
        .checked_mul(13)
        .and_then(|a| m.checked_mul(19).and_then(|b| a.checked_add(b)))
        .and_then(|a| {
            budget
                .limits
                .max_records
                .checked_mul(SCRATCH_BYTES_PER_RECORD)
                .and_then(|b| a.checked_add(b))
        })
        .and_then(|a| {
            budget
                .limits
                .max_stencil_offsets
                .checked_mul(8)
                .and_then(|b| a.checked_add(b))
        })
        .ok_or_else(|| Halt::Unavailable("negative scratch estimate overflowed".into()))?;
    if estimate > budget.limits.max_scratch_bytes {
        return Err(Halt::Unavailable(
            "padded negative scratch exceeds memory limit".into(),
        ));
    }
    let w = width as usize;
    let h = height as usize;
    let pw = padded_width as usize;
    let length = usize::try_from(m)
        .map_err(|_| Halt::Unavailable("padded extent does not fit memory".into()))?;
    let mut background = zeroed_u8(length, budget)?;
    for pixel in &mut background {
        budget.step()?;
        *pixel = 1;
    }
    for y in 0..h {
        for x in 0..w {
            budget.step()?;
            if foreground[y * w + x] != 0 {
                background[(y + py as usize) * pw + (x + px as usize)] = 0;
            }
        }
    }
    let mut near = zeroed_u8(w * h, budget)?;
    for y in 0..h {
        for x in 0..w {
            budget.step()?;
            if foreground[y * w + x] == 0 {
                continue;
            }
            for &(ox, oy) in &stencil.offsets {
                budget.step()?;
                let nx = x as i64 + i64::from(ox);
                let ny = y as i64 + i64::from(oy);
                if nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64 {
                    let index = ny as usize * w + nx as usize;
                    if foreground[index] == 0 {
                        near[index] = 1;
                    }
                }
            }
        }
    }
    let original = label_mask(
        &background,
        padded_width,
        padded_height,
        false,
        false,
        budget,
    )?;
    let (eroded, residual) = morphology(&background, padded_width, padded_height, stencil, budget)?;
    let eroded = label_mask(&eroded, padded_width, padded_height, false, false, budget)?;
    let counts = eroded_island_counts(&original, &eroded, budget)?;
    let mut cropped = zeroed_u8(w * h, budget)?;
    for y in 0..h {
        for x in 0..w {
            budget.step()?;
            let local = y * w + x;
            let padded = (y + py as usize) * pw + (x + px as usize);
            if near[local] != 0 && residual[padded] != 0 {
                cropped[local] = 1;
            }
        }
    }
    let residual = label_mask(&cropped, width, height, false, true, budget)?;
    let mut candidates = Vec::new();
    for component in residual.components {
        budget.step()?;
        let first = component
            .runs
            .first()
            .ok_or_else(|| Halt::Unavailable("negative residual has no run".into()))?;
        let original_id = original.labels
            [(first.y as usize + py as usize) * pw + (first.x0 as usize + px as usize)];
        let kind = if component.touches_frame {
            ProbeCandidateKind::CanvasBoundary
        } else if counts[original_id as usize] == 0 {
            ProbeCandidateKind::NoErodedCenter
        } else {
            ProbeCandidateKind::ProbeSensitiveResidual
        };
        budget.record()?;
        candidates
            .try_reserve(1)
            .map_err(|_| Halt::Unavailable("negative candidate allocation failed".into()))?;
        candidates.push(ProbeCandidate {
            basis,
            negative_gap: true,
            kind,
            original_component_id: original_id,
            pixels: component.pixels,
            bounds: component.bounds,
            runs: component.runs,
            exterior_connected: original.components[original_id as usize - 1].touches_frame,
        });
    }
    // A short constriction can split eroded background yet leave no opening residual.
    let mut split_runs: Vec<Vec<PixelRun>> = Vec::new();
    split_runs
        .try_reserve_exact(
            original
                .components
                .len()
                .checked_add(1)
                .ok_or_else(|| Halt::Unavailable("negative split labels overflowed".into()))?,
        )
        .map_err(|_| Halt::Unavailable("negative constriction records failed".into()))?;
    let split_count = original
        .components
        .len()
        .checked_add(1)
        .ok_or_else(|| Halt::Unavailable("negative split labels overflowed".into()))?;
    while split_runs.len() < split_count {
        let next = split_runs
            .len()
            .saturating_add(CANCEL_POLL_INTERVAL as usize)
            .min(split_count);
        budget.charge((next - split_runs.len()) as u64)?;
        split_runs.resize_with(next, Vec::new);
        budget.poll_after_bulk()?;
    }
    for y in 0..h {
        let mut x = 0;
        while x < w {
            budget.step()?;
            let local = y * w + x;
            if near[local] == 0 {
                x += 1;
                continue;
            }
            let id = original.labels[(y + py as usize) * pw + (x + px as usize)];
            if counts[id as usize] < 2 {
                x += 1;
                continue;
            }
            let start = x;
            x += 1;
            while x < w
                && near[y * w + x] != 0
                && original.labels[(y + py as usize) * pw + (x + px as usize)] == id
            {
                budget.step()?;
                x += 1;
            }
            budget.record()?;
            split_runs[id as usize]
                .try_reserve(1)
                .map_err(|_| Halt::Unavailable("negative split run allocation failed".into()))?;
            split_runs[id as usize].push(PixelRun {
                y: y as u32,
                x0: start as u32,
                x1_exclusive: x as u32,
            });
        }
    }
    for (index, runs) in split_runs.into_iter().enumerate().skip(1) {
        budget.step()?;
        if runs.is_empty() {
            continue;
        }
        let id = index as u32;
        let bounds = bounds_of_runs(&runs, budget)?
            .ok_or_else(|| Halt::Unavailable("negative constriction lost bounds".into()))?;
        let mut pixels = 0_u64;
        for run in &runs {
            budget.step()?;
            pixels = pixels
                .checked_add(u64::from(run.x1_exclusive - run.x0))
                .ok_or_else(|| {
                    Halt::Unavailable("negative candidate pixel count overflowed".into())
                })?;
        }
        budget.record()?;
        candidates
            .try_reserve(1)
            .map_err(|_| Halt::Unavailable("negative constriction allocation failed".into()))?;
        candidates.push(ProbeCandidate {
            basis,
            negative_gap: true,
            kind: ProbeCandidateKind::PossibleConstriction,
            original_component_id: id,
            pixels,
            bounds,
            runs,
            exterior_connected: original.components[index - 1].touches_frame,
        });
    }
    Ok(candidates)
}

/// Computes one exact pixel rectangle around nonempty row-major highlight runs.
fn bounds_of_runs(runs: &[PixelRun], budget: &mut Budget<'_>) -> Result<Option<PixelBounds>, Halt> {
    let Some(first) = runs.first() else {
        return Ok(None);
    };
    let mut bounds = PixelBounds {
        x0: first.x0,
        y0: first.y,
        x1_exclusive: first.x1_exclusive,
        y1_exclusive: first.y + 1,
    };
    for run in runs.iter().skip(1) {
        budget.step()?;
        bounds.x0 = bounds.x0.min(run.x0);
        bounds.y0 = bounds.y0.min(run.y);
        bounds.x1_exclusive = bounds.x1_exclusive.max(run.x1_exclusive);
        bounds.y1_exclusive = bounds.y1_exclusive.max(run.y + 1);
    }
    Ok(Some(bounds))
}

/// Appends already-budgeted candidates with fallible storage growth and cancellation polling.
fn append_candidates(
    destination: &mut Vec<ProbeCandidate>,
    source: Vec<ProbeCandidate>,
    budget: &mut Budget<'_>,
) -> Result<(), Halt> {
    for candidate in source {
        budget.step()?;
        destination
            .try_reserve(1)
            .map_err(|_| Halt::Unavailable("candidate merge allocation failed".into()))?;
        destination.push(candidate);
    }
    Ok(())
}

/// Resolves physical availability and analyzes both mask bases without treating survival as a pass.
fn analyze_alpha(
    rgba: &[u8],
    selection: &PreflightSelection,
    identity: PreflightIdentity,
    budget: &mut Budget<'_>,
) -> Result<PreflightReport, Halt> {
    let width = selection.target.width();
    let height = selection.target.height();
    let AlphaInventory {
        histogram,
        support,
        core,
        occupied,
        components: support_components,
    } = inventory_alpha(rgba, width, height, budget)?;
    let size = selection.settings.size_mm();
    let physical = size.map(|size| {
        let (width_mm, height_mm) = size.millimetres();
        (width_mm / f64::from(width), height_mm / f64::from(height))
    });
    let ppi_axes = size.and_then(|size| {
        size.ppi_for_pixels(width, height)
            .ok()
            .map(|ppi| ppi.axes())
    });
    let guard = physical.and_then(|(dx, dy)| {
        let gx = 2.0 * dx;
        let gy = 2.0 * dy;
        (gx.is_finite() && gy.is_finite() && gx > 0.0 && gy > 0.0).then_some((gx, gy))
    });
    let sampling_note = match (size, physical, guard) {
        (Some(_), Some((dx, dy)), None)
            if !dx.is_finite() || !dy.is_finite() || dx <= 0.0 || dy <= 0.0 =>
        {
            Some("physical pixel pitch is not representable".into())
        }
        (Some(_), _, None) => Some("two-output-pixel physical guard is not representable".into()),
        (Some(_), _, Some(_)) => {
            Some("two-output-pixel guard is provisional; probe survival is not a pass".into())
        }
        (None, _, _) => None,
    };
    let mut positive_candidates = Vec::new();
    let mut negative_candidates = Vec::new();
    let positive_status = match (
        selection.settings.minimum_positive_feature_width_mm(),
        physical,
    ) {
        (0.0, _) => WidthCheckStatus::Disabled,
        (_, None) => WidthCheckStatus::UnavailablePlacement,
        (threshold, Some((dx, dy)))
            if !dx.is_finite()
                || !dy.is_finite()
                || dx <= 0.0
                || dy <= 0.0
                || threshold <= 2.0 * dx
                || threshold <= 2.0 * dy =>
        {
            WidthCheckStatus::SamplingLimited
        }
        (threshold, Some((dx, dy))) => {
            let stencil = physical_stencil(threshold, dx, dy, budget)?;
            if stencil.extent_x == 0 || stencil.extent_y == 0 {
                WidthCheckStatus::SamplingLimited
            } else {
                append_candidates(
                    &mut positive_candidates,
                    positive_probe(
                        &support,
                        width,
                        height,
                        MaskBasis::Support,
                        &stencil,
                        budget,
                    )?,
                    budget,
                )?;
                append_candidates(
                    &mut positive_candidates,
                    positive_probe(&core, width, height, MaskBasis::Core, &stencil, budget)?,
                    budget,
                )?;
                if positive_candidates.is_empty() {
                    WidthCheckStatus::NoWidthCandidatesWithUnresolved
                } else {
                    WidthCheckStatus::AdvisoryCandidates
                }
            }
        }
    };
    let negative_status = match (selection.settings.minimum_negative_gap_width_mm(), physical) {
        (0.0, _) => WidthCheckStatus::Disabled,
        (_, None) => WidthCheckStatus::UnavailablePlacement,
        (threshold, Some((dx, dy)))
            if !dx.is_finite()
                || !dy.is_finite()
                || dx <= 0.0
                || dy <= 0.0
                || threshold <= 2.0 * dx
                || threshold <= 2.0 * dy =>
        {
            WidthCheckStatus::SamplingLimited
        }
        (threshold, Some((dx, dy))) => {
            let stencil = physical_stencil(threshold, dx, dy, budget)?;
            if stencil.extent_x == 0 || stencil.extent_y == 0 {
                WidthCheckStatus::SamplingLimited
            } else {
                append_candidates(
                    &mut negative_candidates,
                    negative_probe(
                        &support,
                        width,
                        height,
                        MaskBasis::Support,
                        &stencil,
                        budget,
                    )?,
                    budget,
                )?;
                append_candidates(
                    &mut negative_candidates,
                    negative_probe(&core, width, height, MaskBasis::Core, &stencil, budget)?,
                    budget,
                )?;
                if negative_candidates.is_empty() {
                    WidthCheckStatus::NoWidthCandidatesWithUnresolved
                } else {
                    WidthCheckStatus::AdvisoryCandidates
                }
            }
        }
    };
    if budget.cancelled.load(Ordering::Acquire) {
        return Err(Halt::Cancelled);
    }
    Ok(PreflightReport {
        identity,
        status: PreflightStatus::CompletedAdvisory,
        positive_status,
        negative_status,
        ppi_axes,
        occupied_bounds: occupied,
        alpha_histogram: histogram,
        support_components,
        positive_candidates,
        negative_candidates,
        sampling_guard_mm_axes: guard,
        sampling_note,
        operations: budget.operations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sets one synthetic final-output alpha byte without inventing source-to-raster behavior.
    fn set_alpha(rgba: &mut [u8], width: usize, x: usize, y: usize, alpha: u8) {
        rgba[(y * width + x) * 4 + 3] = alpha;
    }

    /// Retains exact alpha-1 support and separates same-component near-core coverage from a faint tail.
    #[test]
    fn final_alpha_inventory_keeps_core_free_mark_and_attached_tail() {
        let cancelled = AtomicBool::new(false);
        let mut budget = Budget::new(PreflightLimits::default(), &cancelled);
        let (width, height) = (12, 3);
        let mut rgba = vec![0; width * height * 4];
        for (x, y, alpha) in [
            (0, 0, 1),
            (4, 1, 128),
            (5, 1, 255),
            (5, 0, 254),
            (5, 2, 127),
            (6, 1, 64),
            (7, 1, 1),
            (8, 1, 1),
            (9, 1, 64),
        ] {
            set_alpha(&mut rgba, width, x, y, alpha);
        }
        let inventory = inventory_alpha(&rgba, width as u32, height as u32, &mut budget).unwrap();
        assert_eq!(inventory.histogram[0], 27);
        for alpha in [1, 64, 127, 128, 254, 255] {
            assert!(inventory.histogram[alpha] > 0);
        }
        assert_eq!(inventory.components.len(), 2);
        let isolated = &inventory.components[0];
        assert_eq!(
            (isolated.pixels, isolated.max_alpha, isolated.core_pixels),
            (1, 1, 0)
        );
        assert_eq!(
            isolated.unresolved_low_coverage_runs,
            vec![PixelRun {
                y: 0,
                x0: 0,
                x1_exclusive: 1
            }]
        );
        let attached = &inventory.components[1];
        assert_eq!(
            (
                attached.pixels,
                attached.core_pixels,
                attached.low_coverage_pixels
            ),
            (8, 3, 5)
        );
        assert_eq!(
            (
                attached.boundary_associated_low_coverage_pixels,
                attached.unresolved_low_coverage_pixels
            ),
            (3, 2)
        );
        assert_eq!(attached.fractional_alpha_pixels, 7);
        assert_eq!(
            attached.unresolved_low_coverage_runs,
            vec![PixelRun {
                y: 1,
                x0: 8,
                x1_exclusive: 10
            }]
        );
    }

    /// Uses eight-connected material and four-connected complement labels on diagonal witnesses.
    #[test]
    fn foreground_and_negative_connectivity_follow_dual_convention() {
        let cancelled = AtomicBool::new(false);
        let mut budget = Budget::new(PreflightLimits::default(), &cancelled);
        let diagonal = [1, 0, 0, 0, 1, 0, 0, 0, 1];
        assert_eq!(
            label_mask(&diagonal, 3, 3, true, true, &mut budget)
                .unwrap()
                .components
                .len(),
            1
        );
        assert_eq!(
            label_mask(&diagonal, 3, 3, false, false, &mut budget)
                .unwrap()
                .components
                .len(),
            3
        );
        let negative = [0, 1, 0, 1, 0, 1, 0, 1, 0];
        assert_eq!(
            label_mask(&negative, 3, 3, false, false, &mut budget)
                .unwrap()
                .components
                .len(),
            4
        );
        assert_eq!(
            label_mask(&negative, 3, 3, true, false, &mut budget)
                .unwrap()
                .components
                .len(),
            1
        );
    }

    /// Demonstrates sampled phase ambiguity for a 0.8 mm strip under a 1 mm disk probe.
    #[test]
    fn physical_strip_phase_prevents_a_width_pass_claim() {
        let cancelled = AtomicBool::new(false);
        let mut budget = Budget::new(PreflightLimits::default(), &cancelled);
        let stencil = physical_stencil(1.0, 0.2, 0.4, &mut budget).unwrap();
        assert!(stencil.offsets.contains(&(0, 0)));
        let mut surviving = Vec::new();
        for phase in [0.0_f64, 0.1] {
            let mut strip = vec![0_u8; 31 * 21];
            for y in 0..21 {
                for x in 0..31 {
                    let position = (x as f64 - 15.0) * 0.2 - phase;
                    if position.abs() <= 0.4 {
                        strip[y * 31 + x] = 1;
                    }
                }
            }
            let (eroded, _) = morphology(&strip, 31, 21, &stencil, &mut budget).unwrap();
            surviving.push(eroded.iter().any(|&pixel| pixel != 0));
        }
        assert_ne!(surviving[0], surviving[1]);
    }

    /// Finds an eroded-center split across a thin bridge inside one original support component.
    #[test]
    fn short_thin_neck_is_a_possible_constriction() {
        let cancelled = AtomicBool::new(false);
        let mut budget = Budget::new(PreflightLimits::default(), &cancelled);
        let stencil = physical_stencil(1.0, 0.2, 0.2, &mut budget).unwrap();
        let mut mask = vec![0_u8; 25 * 15];
        for y in 4..=10 {
            for x in (2..=8).chain(14..=20) {
                mask[y * 25 + x] = 1;
            }
        }
        for x in 9..=13 {
            mask[7 * 25 + x] = 1;
        }
        let candidates =
            positive_probe(&mask, 25, 15, MaskBasis::Support, &stencil, &mut budget).unwrap();
        assert!(candidates.iter().any(|candidate| candidate.kind
            == ProbeCandidateKind::PossibleConstriction
            && candidate.original_component_id == 1
            && candidate.bounds.x0 <= 2
            && candidate.bounds.x1_exclusive >= 21));
    }

    /// Preserves a narrow exterior-connected stronger-coverage gap as an advisory location.
    #[test]
    fn negative_probe_keeps_exterior_connected_core_gap() {
        let cancelled = AtomicBool::new(false);
        let mut budget = Budget::new(PreflightLimits::default(), &cancelled);
        let stencil = physical_stencil(1.0, 0.2, 0.2, &mut budget).unwrap();
        let mut core = vec![0_u8; 23 * 17];
        for y in 2..15 {
            for x in (8..=10).chain(12..=14) {
                core[y * 23 + x] = 1;
            }
        }
        let candidates =
            negative_probe(&core, 23, 17, MaskBasis::Core, &stencil, &mut budget).unwrap();
        assert!(candidates.iter().any(|candidate| candidate.negative_gap
            && candidate.basis == MaskBasis::Core
            && candidate.exterior_connected
            && candidate.bounds.x0 <= 11
            && candidate.bounds.x1_exclusive > 11));
    }
}
