#![forbid(unsafe_code)]

//! The shared mutable-document boundary for headless Toniator frontends.

#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
};

mod scheduler;

pub use scheduler::{
    ChannelDiagnosticCompletion, ChannelDiagnosticScheduler, ChannelDiagnosticTicket,
    SchedulerError,
};

use toniator_domain::{
    CanvasSpec, ChannelId, EvaluationSnapshot, EvaluationToken, PatternOutput, PatternStructure,
    SourceComponent, SourcePlacement, SourceReference, SourceReferenceId,
};
use toniator_domain::{
    ChannelPaint, DocumentEvaluationSnapshot, DocumentEvaluationToken, DocumentSession,
    HalftoneChannelModel, HalftoneChannelRole, ModeledChannelState,
};
pub use toniator_patterns::{
    CanonicalCircleMark, CircularMarkRealization, MarkResponse, Point2, RealizationError, SiteId,
    SiteScope,
};
use toniator_patterns::{
    GridFamilyOutput, MappedCircularMarkRealization, SourceColorCircularMarkRealization,
    evaluate_straight_grid, realize_circular_marks, realize_mapped_circular_marks,
    realize_source_color_circular_marks,
};
pub use toniator_render::{
    GeometryOutput, PreviewRasterTarget, RasterBackground, RasterSurface, RenderError, RenderLayer,
    RenderScene, SceneIdentity, encode_png, linear_to_srgb, rasterize, rasterize_preview,
    srgb_to_linear, write_svg,
};
use toniator_sampling::decode_source;
pub use toniator_sampling::{
    DECODER_CONTRACT_ID, SourceField, SourceFormat, SourceFormatHint, SourceIdentity,
    SvgTextDiagnostic,
};

pub use toniator_patterns::{GridError, GridInspectRequest};

/// Resolves source identity through the accepted sampling decoder.
///
/// This deliberately performs no format parsing, dimension extraction, or
/// sizing policy of its own. Frontends may use it for preflight while the
/// authoritative evaluator performs its normal decode when scheduled.
pub fn resolve_source_identity(
    bytes: &[u8],
    format_hint: SourceFormatHint,
) -> Result<SourceIdentity, EvaluationError> {
    Ok(decode_source(bytes, format_hint)
        .map_err(EvaluationError::from_sampling)?
        .identity()
        .clone())
}

/// Runs the bounded Stage 3 family evaluation through the shared headless boundary.
pub fn inspect_straight_grid(request: &GridInspectRequest) -> Result<GridFamilyOutput, GridError> {
    evaluate_straight_grid(request)
}

/// One immutable Stage 4 request: the engine creates family output once, then
/// decodes supplied bytes and realizes canonical circles from that output.
#[derive(Clone, Debug, PartialEq)]
pub struct MarksInspectRequest<'a> {
    pub grid: GridInspectRequest,
    pub source_bytes: &'a [u8],
    pub source_format: SourceFormatHint,
    pub source_component: SourceComponent,
    pub placement: SourcePlacement,
    pub response: MarkResponse,
}

/// The shared headless source-to-family-to-realization boundary.
pub fn inspect_circular_marks(
    request: &MarksInspectRequest<'_>,
) -> Result<CircularMarkRealization, MarksInspectError> {
    let family = inspect_straight_grid(&request.grid)?;
    let source = decode_source(request.source_bytes, request.source_format)?;
    Ok(realize_from_existing_family(
        &family,
        &source,
        &request.grid.canvas,
        request.placement,
        request.source_component,
        request.response,
    )?)
}

/// Immutable source bytes resolved outside the domain. The ID and decoding
/// hint travel with the bytes so the engine can reject authority mismatches
/// before it decodes or evaluates geometry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSource {
    reference_id: SourceReferenceId,
    bytes: Arc<[u8]>,
    format: SourceFormatHint,
}

impl ResolvedSource {
    pub fn new(
        reference_id: SourceReferenceId,
        bytes: impl Into<Arc<[u8]>>,
        format: SourceFormatHint,
    ) -> Result<Self, EvaluationError> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(EvaluationError::new(
                "source.bytes",
                "source must not be empty",
            ));
        }
        if matches!(format, SourceFormatHint::Unsupported) {
            return Err(EvaluationError::new(
                "source.format",
                "only PNG and SVG source formats are supported",
            ));
        }
        Ok(Self {
            reference_id,
            bytes,
            format,
        })
    }

    pub fn reference_id(&self) -> &SourceReferenceId {
        &self.reference_id
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn format(&self) -> SourceFormatHint {
        self.format
    }
}

/// The sole public Stage 6 evaluation entry. Its fields are private so a
/// snapshot cannot be mixed with another token or resolved source after it is
/// validated.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelDiagnosticRequest {
    snapshot: EvaluationSnapshot,
    source: ResolvedSource,
}

/// Immutable resource policy for one evaluation or scheduler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationLimits {
    max_family_candidates: usize,
}

impl EvaluationLimits {
    pub const DEFAULT_MAX_FAMILY_CANDIDATES: usize = 1_048_576;

    pub fn new(max_family_candidates: usize) -> Result<Self, EvaluationError> {
        if max_family_candidates == 0 {
            return Err(EvaluationError::new(
                "coverage.candidate_limit",
                "configured candidate limit must be nonzero",
            ));
        }
        Ok(Self {
            max_family_candidates,
        })
    }

    pub const fn max_family_candidates(self) -> usize {
        self.max_family_candidates
    }
}

impl Default for EvaluationLimits {
    fn default() -> Self {
        Self {
            max_family_candidates: Self::DEFAULT_MAX_FAMILY_CANDIDATES,
        }
    }
}

/// Whether a successful evaluation reused an accepted derived value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheDisposition {
    Hit,
    Miss,
}

/// Immutable per-layer diagnostics for a successful evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheDiagnostics {
    pub decoded_source: CacheDisposition,
    pub family: CacheDisposition,
    pub realization: CacheDisposition,
    pub scene: CacheDisposition,
    pub raster: CacheDisposition,
}

impl ChannelDiagnosticRequest {
    pub fn new(snapshot: EvaluationSnapshot, source: ResolvedSource) -> Self {
        Self { snapshot, source }
    }

    pub(crate) fn token(&self) -> EvaluationToken {
        self.snapshot.token()
    }
}

/// Immutable result from evaluating one authoritative snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelDiagnosticResult {
    token: EvaluationToken,
    source_identity: SourceIdentity,
    scene: RenderScene,
    raster: RasterSurface,
}

impl ChannelDiagnosticResult {
    pub fn token(&self) -> EvaluationToken {
        self.token
    }
    pub fn source_identity(&self) -> &SourceIdentity {
        &self.source_identity
    }
    pub fn scene(&self) -> &RenderScene {
        &self.scene
    }
    pub fn raster(&self) -> &RasterSurface {
        &self.raster
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceCacheKey {
    reference_id: String,
    bytes: Arc<[u8]>,
    format: SourceFormatHint,
    decoder_contract: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FamilyCacheKey {
    source: SourceCacheKey,
    decoded_pixel_identity: String,
    canvas: (u64, u64),
    density: (u64, u64),
    aspect_locked: bool,
    rotation: u64,
    translation: (u64, u64),
    guard_steps: u32,
    structure: u8,
    output: u8,
    maximum_support_radius: u64,
    max_family_candidates: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RealizationCacheKey {
    family: FamilyCacheKey,
    decoded_pixel_identity: String,
    canvas: (u64, u64),
    source_component: u8,
    placement: u8,
    response: (u64, u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SceneCacheKey {
    realization: RealizationCacheKey,
    canvas: (u64, u64),
    channel_id: u64,
    visible: bool,
    color: (u64, u64, u64, u64),
    opacity: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RasterCacheKey {
    scene: SceneCacheKey,
    transparent_raster_contract: &'static str,
}

const TRANSPARENT_RASTER_CONTRACT_ID: &str = "toniator-render-transparent-raster-v1";

#[derive(Clone)]
struct DerivedCacheSnapshot {
    source: Option<(SourceCacheKey, Arc<SourceField>)>,
    family: Option<(FamilyCacheKey, Arc<GridFamilyOutput>)>,
    realization: Option<(RealizationCacheKey, Arc<CircularMarkRealization>)>,
    scene: Option<(SceneCacheKey, Arc<RenderScene>)>,
    raster: Option<(RasterCacheKey, Arc<RasterSurface>)>,
}

impl DerivedCacheSnapshot {
    fn empty() -> Self {
        Self {
            source: None,
            family: None,
            realization: None,
            scene: None,
            raster: None,
        }
    }
}

/// Private, disposable, last-successful cache. Values are immutable `Arc`s and
/// are installed only by scheduler acceptance.
#[derive(Default)]
pub(crate) struct DerivedCache {
    source: Option<(SourceCacheKey, Arc<SourceField>)>,
    family: Option<(FamilyCacheKey, Arc<GridFamilyOutput>)>,
    realization: Option<(RealizationCacheKey, Arc<CircularMarkRealization>)>,
    scene: Option<(SceneCacheKey, Arc<RenderScene>)>,
    raster: Option<(RasterCacheKey, Arc<RasterSurface>)>,
}

impl DerivedCache {
    pub(crate) fn snapshot(&self) -> DerivedCacheSnapshot {
        DerivedCacheSnapshot {
            source: self.source.clone(),
            family: self.family.clone(),
            realization: self.realization.clone(),
            scene: self.scene.clone(),
            raster: self.raster.clone(),
        }
    }

    pub(crate) fn commit(&mut self, transaction: CacheTransaction) {
        if let Some(value) = transaction.source {
            self.source = Some(value);
        }
        if let Some(value) = transaction.family {
            self.family = Some(value);
        }
        if let Some(value) = transaction.realization {
            self.realization = Some(value);
        }
        if let Some(value) = transaction.scene {
            self.scene = Some(value);
        }
        if let Some(value) = transaction.raster {
            self.raster = Some(value);
        }
    }
}

#[derive(Default)]
pub(crate) struct CacheTransaction {
    source: Option<(SourceCacheKey, Arc<SourceField>)>,
    family: Option<(FamilyCacheKey, Arc<GridFamilyOutput>)>,
    realization: Option<(RealizationCacheKey, Arc<CircularMarkRealization>)>,
    scene: Option<(SceneCacheKey, Arc<RenderScene>)>,
    raster: Option<(RasterCacheKey, Arc<RasterSurface>)>,
}

pub(crate) struct CachedEvaluation {
    pub(crate) result: ChannelDiagnosticResult,
    pub(crate) diagnostics: CacheDiagnostics,
    pub(crate) transaction: CacheTransaction,
}

/// Evaluates exactly the Stage 3 -> Stage 4 -> Stage 5 chain represented by
/// the immutable snapshot. It performs no document mutation or source lookup.
pub fn evaluate_channel_diagnostic(
    request: ChannelDiagnosticRequest,
) -> Result<ChannelDiagnosticResult, EvaluationError> {
    evaluate_channel_diagnostic_with_limits(request, EvaluationLimits::default())
}

/// Evaluates one immutable request under a caller-selected family candidate
/// policy. This synchronous entry is intentionally uncached.
pub fn evaluate_channel_diagnostic_with_limits(
    request: ChannelDiagnosticRequest,
    limits: EvaluationLimits,
) -> Result<ChannelDiagnosticResult, EvaluationError> {
    match evaluate_channel_diagnostic_cached_with_cancellation(
        request,
        &NeverCancelled,
        DerivedCacheSnapshot::empty(),
        limits,
    ) {
        Ok(result) => Ok(result.result),
        Err(EvaluationRunError::Evaluation(error)) => Err(error),
        Err(EvaluationRunError::Cancelled) => unreachable!("synchronous evaluation never cancels"),
    }
}

pub(crate) fn evaluate_channel_diagnostic_cancellable_cached(
    request: ChannelDiagnosticRequest,
    cancelled: &AtomicBool,
    cache: DerivedCacheSnapshot,
    limits: EvaluationLimits,
) -> Result<CachedEvaluation, EvaluationRunError> {
    evaluate_channel_diagnostic_cached_with_cancellation(
        request,
        &AtomicCancellation(cancelled),
        cache,
        limits,
    )
}

#[cfg(test)]
pub(crate) fn evaluate_channel_diagnostic_cancellable_with_gate(
    request: ChannelDiagnosticRequest,
    cancelled: &AtomicBool,
    gate: &EvaluationStageGate,
) -> Result<ChannelDiagnosticResult, EvaluationRunError> {
    evaluate_channel_diagnostic_cached_with_cancellation(
        request,
        &ObservedCancellation { cancelled, gate },
        DerivedCacheSnapshot::empty(),
        EvaluationLimits::default(),
    )
    .map(|result| result.result)
}

trait CancellationProbe {
    fn is_cancelled(&self) -> bool;

    #[cfg(test)]
    fn observe_stage(&self, _stage: EvaluationStage, _checkpoint: EvaluationCheckpoint) {}
}

struct NeverCancelled;

impl CancellationProbe for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

struct AtomicCancellation<'a>(&'a AtomicBool);

impl CancellationProbe for AtomicCancellation<'_> {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[cfg(test)]
struct ObservedCancellation<'a> {
    cancelled: &'a AtomicBool,
    gate: &'a EvaluationStageGate,
}

#[cfg(test)]
impl CancellationProbe for ObservedCancellation<'_> {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn observe_stage(&self, stage: EvaluationStage, checkpoint: EvaluationCheckpoint) {
        self.gate.wait(stage, checkpoint);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EvaluationStage {
    #[cfg(test)]
    Channel,
    Family,
    Decode,
    Realization,
    Scene,
    Raster,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EvaluationCheckpoint {
    Before,
    After,
}

#[cfg(test)]
struct EvaluationStageGateState {
    entered: bool,
    released: bool,
}

#[cfg(test)]
pub(crate) struct EvaluationStageGate {
    stage: EvaluationStage,
    checkpoint: EvaluationCheckpoint,
    entered: std::sync::mpsc::Sender<()>,
    state: std::sync::Mutex<EvaluationStageGateState>,
    wake: std::sync::Condvar,
}

#[cfg(test)]
impl EvaluationStageGate {
    pub(crate) fn new(
        stage: EvaluationStage,
        checkpoint: EvaluationCheckpoint,
    ) -> (std::sync::Arc<Self>, std::sync::mpsc::Receiver<()>) {
        let (entered, receiver) = std::sync::mpsc::channel();
        (
            std::sync::Arc::new(Self {
                stage,
                checkpoint,
                entered,
                state: std::sync::Mutex::new(EvaluationStageGateState {
                    entered: false,
                    released: false,
                }),
                wake: std::sync::Condvar::new(),
            }),
            receiver,
        )
    }

    pub(crate) fn release(&self) {
        let mut state = self
            .state
            .lock()
            .expect("evaluation stage gate lock poisoned");
        state.released = true;
        self.wake.notify_all();
    }

    fn wait(&self, stage: EvaluationStage, checkpoint: EvaluationCheckpoint) {
        if stage != self.stage || checkpoint != self.checkpoint {
            return;
        }
        let mut state = self
            .state
            .lock()
            .expect("evaluation stage gate lock poisoned");
        if state.entered {
            return;
        }
        state.entered = true;
        let _ = self.entered.send(());
        while !state.released {
            state = self
                .wake
                .wait(state)
                .expect("evaluation stage gate lock poisoned");
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EvaluationRunError {
    Cancelled,
    Evaluation(EvaluationError),
}

impl From<EvaluationError> for EvaluationRunError {
    fn from(error: EvaluationError) -> Self {
        Self::Evaluation(error)
    }
}

fn evaluate_stage<T>(
    stage: EvaluationStage,
    cancellation: &dyn CancellationProbe,
    evaluate: impl FnOnce() -> Result<T, EvaluationError>,
) -> Result<T, EvaluationRunError> {
    #[cfg(not(test))]
    let _ = stage;
    if cancellation.is_cancelled() {
        return Err(EvaluationRunError::Cancelled);
    }
    #[cfg(test)]
    cancellation.observe_stage(stage, EvaluationCheckpoint::Before);
    if cancellation.is_cancelled() {
        return Err(EvaluationRunError::Cancelled);
    }
    let result = evaluate();
    #[cfg(test)]
    cancellation.observe_stage(stage, EvaluationCheckpoint::After);
    if cancellation.is_cancelled() {
        return Err(EvaluationRunError::Cancelled);
    }
    result.map_err(EvaluationRunError::Evaluation)
}

/// Evaluates the same ordered Stage 3 -> Stage 4 -> Stage 5 path as
/// [`evaluate_channel_diagnostic`], checking cancellation only between
/// existing pipeline stages.
fn evaluate_channel_diagnostic_cached_with_cancellation(
    request: ChannelDiagnosticRequest,
    cancellation: &dyn CancellationProbe,
    cache: DerivedCacheSnapshot,
    limits: EvaluationLimits,
) -> Result<CachedEvaluation, EvaluationRunError> {
    let document = request.snapshot.document();
    let channel_id = request.snapshot.token().channel_id();
    let channel = document.channel(channel_id).ok_or(EvaluationError::new(
        "evaluation.channel_id",
        "evaluation targets a missing channel",
    ))?;
    let source_id = match document.source() {
        SourceReference::Assigned(id) => id,
        SourceReference::Unassigned => {
            return Err(EvaluationError::new(
                "evaluation.source_reference",
                "evaluation requires an assigned source reference",
            )
            .into());
        }
    };
    if source_id != request.source.reference_id() {
        return Err(EvaluationError::new(
            "evaluation.source_reference",
            "resolved source reference does not match the document snapshot",
        )
        .into());
    }
    let definition = document
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == channel.pattern_definition_id)
        .ok_or(EvaluationError::new(
            "evaluation.pattern_definition",
            "channel references a missing pattern definition",
        ))?;
    if definition.structure != PatternStructure::StraightGrid {
        return Err(EvaluationError::new(
            "evaluation.pattern_definition.structure",
            "unsupported pattern structure",
        )
        .into());
    }
    if definition.output != PatternOutput::CircularMarks {
        return Err(EvaluationError::new(
            "evaluation.pattern_definition.output",
            "unsupported pattern output",
        )
        .into());
    }
    let response = MarkResponse {
        minimum_size: channel.mark_geometry_response.minimum_size,
        maximum_size: channel.mark_geometry_response.maximum_size,
    };
    let source_key = SourceCacheKey {
        reference_id: request.source.reference_id().as_str().to_owned(),
        bytes: Arc::clone(&request.source.bytes),
        format: request.source.format(),
        decoder_contract: DECODER_CONTRACT_ID,
    };
    // Preflight above remains authoritative. Decode deliberately occurs before
    // the family lookup so decoded-pixel identity participates downstream.
    let (source, source_disposition) =
        evaluate_stage(EvaluationStage::Decode, cancellation, || {
            match &cache.source {
                Some((key, source)) if *key == source_key => {
                    Ok((Arc::clone(source), CacheDisposition::Hit))
                }
                _ => decode_source(request.source.bytes(), request.source.format())
                    .map(|source| (Arc::new(source), CacheDisposition::Miss))
                    .map_err(EvaluationError::from_sampling),
            }
        })?;
    let family_key = FamilyCacheKey {
        source: source_key.clone(),
        decoded_pixel_identity: source.identity().decoded_pixel_hash.clone(),
        canvas: canvas_key(document.canvas()),
        density: (
            channel.layout.density.across_x.to_bits(),
            channel.layout.density.across_y.to_bits(),
        ),
        aspect_locked: channel.layout.density.aspect_locked,
        rotation: channel.layout.rotation_degrees.to_bits(),
        translation: (
            channel.layout.translation_x.to_bits(),
            channel.layout.translation_y.to_bits(),
        ),
        guard_steps: definition.guard_steps,
        structure: structure_key(definition.structure),
        output: output_key(definition.output),
        maximum_support_radius: definition.maximum_support_radius.to_bits(),
        max_family_candidates: limits.max_family_candidates(),
    };
    let grid = GridInspectRequest {
        canvas: document.canvas().clone(),
        density: channel.layout.density.clone(),
        rotation_degrees: channel.layout.rotation_degrees,
        translation_x: channel.layout.translation_x,
        translation_y: channel.layout.translation_y,
        guard_steps: definition.guard_steps,
        support_radius: definition.maximum_support_radius,
        max_family_candidates: limits.max_family_candidates(),
    };
    let (family, family_disposition) =
        evaluate_stage(EvaluationStage::Family, cancellation, || {
            match &cache.family {
                Some((key, family)) if *key == family_key => {
                    Ok((Arc::clone(family), CacheDisposition::Hit))
                }
                _ => inspect_straight_grid(&grid)
                    .map(|family| (Arc::new(family), CacheDisposition::Miss))
                    .map_err(EvaluationError::from_grid),
            }
        })?;
    let realization_key = RealizationCacheKey {
        family: family_key.clone(),
        decoded_pixel_identity: source.identity().decoded_pixel_hash.clone(),
        canvas: canvas_key(document.canvas()),
        source_component: source_component_key(channel.source_mapping.component),
        placement: placement_key(channel.source_mapping.placement),
        response: (
            response.minimum_size.to_bits(),
            response.maximum_size.to_bits(),
        ),
    };
    let (realization, realization_disposition) = evaluate_stage(
        EvaluationStage::Realization,
        cancellation,
        || match &cache.realization {
            Some((key, realization)) if *key == realization_key => {
                Ok((Arc::clone(realization), CacheDisposition::Hit))
            }
            _ => realize_from_existing_family(
                &family,
                &source,
                document.canvas(),
                channel.source_mapping.placement,
                channel.source_mapping.component,
                response,
            )
            .map(|realization| (Arc::new(realization), CacheDisposition::Miss))
            .map_err(EvaluationError::from_realization),
        },
    )?;
    let scene_key = SceneCacheKey {
        realization: realization_key.clone(),
        canvas: canvas_key(document.canvas()),
        channel_id: channel_id.0,
        visible: channel.appearance.visible,
        color: color_key(&channel.appearance.color),
        opacity: channel.appearance.opacity.to_bits(),
    };
    let (scene, scene_disposition) = evaluate_stage(EvaluationStage::Scene, cancellation, || {
        match &cache.scene {
            Some((key, scene)) if *key == scene_key => {
                Ok((Arc::clone(scene), CacheDisposition::Hit))
            }
            _ => build_scene(SceneBuild {
                canvas: document.canvas().clone(),
                channel_id,
                visible: channel.appearance.visible,
                color: channel.appearance.color.clone(),
                opacity: channel.appearance.opacity,
                family_fingerprint: realization.family_fingerprint.clone(),
                realization_fingerprint: realization.realization_fingerprint.clone(),
                marks: realization.marks.clone(),
            })
            .map(|scene| (Arc::new(scene), CacheDisposition::Miss)),
        }
    })?;
    let raster_key = RasterCacheKey {
        scene: scene_key.clone(),
        transparent_raster_contract: TRANSPARENT_RASTER_CONTRACT_ID,
    };
    let (raster, raster_disposition) =
        evaluate_stage(EvaluationStage::Raster, cancellation, || {
            match &cache.raster {
                Some((key, raster)) if *key == raster_key => {
                    Ok((Arc::clone(raster), CacheDisposition::Hit))
                }
                _ => rasterize(&scene, RasterBackground::Transparent)
                    .map(|raster| (Arc::new(raster), CacheDisposition::Miss))
                    .map_err(EvaluationError::from_render),
            }
        })?;
    let diagnostics = CacheDiagnostics {
        decoded_source: source_disposition,
        family: family_disposition,
        realization: realization_disposition,
        scene: scene_disposition,
        raster: raster_disposition,
    };
    let transaction = CacheTransaction {
        source: matches!(source_disposition, CacheDisposition::Miss)
            .then(|| (source_key, source.clone())),
        family: matches!(family_disposition, CacheDisposition::Miss)
            .then(|| (family_key, family.clone())),
        realization: matches!(realization_disposition, CacheDisposition::Miss)
            .then(|| (realization_key, realization.clone())),
        scene: matches!(scene_disposition, CacheDisposition::Miss)
            .then(|| (scene_key, scene.clone())),
        raster: matches!(raster_disposition, CacheDisposition::Miss)
            .then(|| (raster_key, raster.clone())),
    };
    Ok(CachedEvaluation {
        result: ChannelDiagnosticResult {
            token: request.snapshot.token(),
            source_identity: source.identity().clone(),
            scene: (*scene).clone(),
            raster: (*raster).clone(),
        },
        diagnostics,
        transaction,
    })
}

struct SceneBuild {
    canvas: CanvasSpec,
    channel_id: ChannelId,
    visible: bool,
    color: toniator_domain::ColorValue,
    opacity: f64,
    family_fingerprint: String,
    realization_fingerprint: String,
    marks: Vec<CanonicalCircleMark>,
}

fn build_scene(input: SceneBuild) -> Result<RenderScene, EvaluationError> {
    RenderScene::new(
        input.canvas,
        input.family_fingerprint,
        input.realization_fingerprint,
        vec![
            RenderLayer::new(
                input.channel_id,
                input.visible,
                input.color,
                input.opacity,
                GeometryOutput::CircularMarks(input.marks),
            )
            .map_err(EvaluationError::from_render)?,
        ],
    )
    .map_err(EvaluationError::from_render)
}

fn canvas_key(canvas: &CanvasSpec) -> (u64, u64) {
    (canvas.width.to_bits(), canvas.height.to_bits())
}

fn structure_key(value: PatternStructure) -> u8 {
    match value {
        PatternStructure::StraightGrid => 1,
        PatternStructure::Unsupported => 255,
    }
}

fn output_key(value: PatternOutput) -> u8 {
    match value {
        PatternOutput::CircularMarks => 1,
        PatternOutput::Unsupported => 255,
    }
}

fn source_component_key(value: SourceComponent) -> u8 {
    match value {
        SourceComponent::Luminance => 1,
        SourceComponent::Alpha => 2,
    }
}

fn placement_key(value: SourcePlacement) -> u8 {
    match value {
        SourcePlacement::StretchToCanvas => 1,
    }
}

fn color_key(color: &toniator_domain::ColorValue) -> (u64, u64, u64, u64) {
    (
        color.red.to_bits(),
        color.green.to_bits(),
        color.blue.to_bits(),
        color.alpha.to_bits(),
    )
}

/// Exposes realization from an already evaluated family so callers can prove
/// exact Stage 3 reuse while varying only the realization response.
pub fn realize_from_existing_family(
    family: &GridFamilyOutput,
    source: &SourceField,
    canvas: &toniator_domain::CanvasSpec,
    placement: SourcePlacement,
    component: SourceComponent,
    response: MarkResponse,
) -> Result<CircularMarkRealization, RealizationError> {
    realize_circular_marks(family, source, canvas, placement, component, response)
}

/// Errors crossing the source, family, and realization boundaries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarksInspectError {
    Grid(GridError),
    Sampling(toniator_sampling::SamplingError),
    Realization(RealizationError),
}

/// Stable failures at the authoritative document-evaluation boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationError {
    path: &'static str,
    message: &'static str,
}

impl EvaluationError {
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }
    pub const fn path(&self) -> &'static str {
        self.path
    }
    pub const fn message(&self) -> &'static str {
        self.message
    }
    fn from_grid(error: GridError) -> Self {
        Self::new(error.path(), error.message())
    }
    fn from_sampling(error: toniator_sampling::SamplingError) -> Self {
        Self::new(error.path(), error.message())
    }
    fn from_realization(error: RealizationError) -> Self {
        Self::new(error.path(), error.message())
    }
    fn from_render(error: toniator_render::RenderError) -> Self {
        Self::new(error.path(), error.message())
    }
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for EvaluationError {}

impl From<GridError> for MarksInspectError {
    fn from(error: GridError) -> Self {
        Self::Grid(error)
    }
}

impl From<toniator_sampling::SamplingError> for MarksInspectError {
    fn from(error: toniator_sampling::SamplingError) -> Self {
        Self::Sampling(error)
    }
}

impl From<RealizationError> for MarksInspectError {
    fn from(error: RealizationError) -> Self {
        Self::Realization(error)
    }
}

impl fmt::Display for MarksInspectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Grid(error) => error.fmt(formatter),
            Self::Sampling(error) => error.fmt(formatter),
            Self::Realization(error) => error.fmt(formatter),
        }
    }
}

impl Error for MarksInspectError {}

/// Complete-document evaluation request. The retained single-channel request
/// is explicitly diagnostic and cannot enter this authority.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationRequest {
    snapshot: DocumentEvaluationSnapshot,
    source: ResolvedSource,
    preview_target: Option<PreviewRasterTarget>,
}

impl EvaluationRequest {
    pub fn new(snapshot: DocumentEvaluationSnapshot, source: ResolvedSource) -> Self {
        Self {
            snapshot,
            source,
            preview_target: None,
        }
    }

    /// Keeps the authoritative document/scene native while deriving only the
    /// transparent preview raster at this checked output target.
    pub fn with_preview_target(
        snapshot: DocumentEvaluationSnapshot,
        source: ResolvedSource,
        preview_target: PreviewRasterTarget,
    ) -> Self {
        Self {
            snapshot,
            source,
            preview_target: Some(preview_target),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelEvaluationSummary {
    role: HalftoneChannelRole,
    channel_id: ChannelId,
    family_identity: String,
    realization_identity: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelCacheDiagnostics {
    pub channel_id: ChannelId,
    pub family: CacheDisposition,
    pub realization: CacheDisposition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentCacheDiagnostics {
    pub aggregate: CacheDiagnostics,
    pub channels: Vec<ChannelCacheDiagnostics>,
}
impl ChannelEvaluationSummary {
    pub const fn role(&self) -> HalftoneChannelRole {
        self.role
    }
    pub const fn channel_id(&self) -> ChannelId {
        self.channel_id
    }
    pub fn family_identity(&self) -> &str {
        &self.family_identity
    }
    pub fn realization_identity(&self) -> &str {
        &self.realization_identity
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationResult {
    token: DocumentEvaluationToken,
    source_identity: SourceIdentity,
    channels: Vec<ChannelEvaluationSummary>,
    scene: RenderScene,
    raster: RasterSurface,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationTicket(u64);
impl EvaluationTicket {
    pub const fn value(self) -> u64 {
        self.0
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum EvaluationCompletion {
    Completed {
        ticket: EvaluationTicket,
        result: Box<EvaluationResult>,
        diagnostics: DocumentCacheDiagnostics,
    },
    Failed {
        ticket: EvaluationTicket,
        token: DocumentEvaluationToken,
        error: EvaluationError,
    },
}
impl EvaluationCompletion {
    pub const fn ticket(&self) -> EvaluationTicket {
        match self {
            Self::Completed { ticket, .. } | Self::Failed { ticket, .. } => *ticket,
        }
    }
    pub fn token(&self) -> DocumentEvaluationToken {
        match self {
            Self::Completed { result, .. } => result.token(),
            Self::Failed { token, .. } => *token,
        }
    }
    pub fn result(&self) -> Option<&EvaluationResult> {
        match self {
            Self::Completed { result, .. } => Some(result),
            Self::Failed { .. } => None,
        }
    }
    pub fn error(&self) -> Option<&EvaluationError> {
        match self {
            Self::Completed { .. } => None,
            Self::Failed { error, .. } => Some(error),
        }
    }
    pub fn cache_diagnostics(&self) -> Option<&DocumentCacheDiagnostics> {
        match self {
            Self::Completed { diagnostics, .. } => Some(diagnostics),
            Self::Failed { .. } => None,
        }
    }
}

/// Complete-document scheduler state. The diagnostic scheduler remains a
/// separate retained API; this cache is never writable authority.
struct DocumentJob {
    ticket: EvaluationTicket,
    request: EvaluationRequest,
    cancelled: Arc<AtomicBool>,
    cache: DocumentDerivedCache,
    #[cfg(test)]
    decode_observer: Option<Arc<AtomicUsize>>,
}
struct DocumentWorkerCompletion {
    completion: EvaluationCompletion,
    transaction: Option<DocumentCacheTransaction>,
}
struct DocumentSchedulerState {
    next_ticket: Option<u64>,
    sender: Option<Sender<DocumentJob>>,
    worker: Option<JoinHandle<()>>,
    latest_cancellation: Option<Arc<AtomicBool>>,
    latest_ticket: Option<EvaluationTicket>,
    cache: DocumentDerivedCache,
    pending: Option<(EvaluationTicket, DocumentCacheTransaction)>,
    accepted: Option<EvaluationTicket>,
    #[cfg(test)]
    decode_observer: Option<Arc<AtomicUsize>>,
}
pub struct EvaluationScheduler {
    state: Mutex<DocumentSchedulerState>,
    latest_ticket: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
    completions: Mutex<Receiver<DocumentWorkerCompletion>>,
}
impl EvaluationScheduler {
    pub fn new() -> Result<Self, SchedulerError> {
        Self::new_with_limits(EvaluationLimits::default())
    }

    pub fn new_with_limits(limits: EvaluationLimits) -> Result<Self, SchedulerError> {
        #[cfg(test)]
        return Self::new_with_limits_and_gate(limits, None, None);
        #[cfg(not(test))]
        Self::new_with_limits_and_gate(limits)
    }

    #[cfg(test)]
    pub(crate) fn new_with_test_gate(
        limits: EvaluationLimits,
        gate: Arc<EvaluationStageGate>,
    ) -> Result<Self, SchedulerError> {
        Self::new_with_limits_and_gate(limits, Some(gate), None)
    }

    #[cfg(test)]
    fn new_with_test_decode_observer(
        limits: EvaluationLimits,
        decode_observer: Arc<AtomicUsize>,
    ) -> Result<Self, SchedulerError> {
        Self::new_with_limits_and_gate(limits, None, Some(decode_observer))
    }

    fn new_with_limits_and_gate(
        limits: EvaluationLimits,
        #[cfg(test)] gate: Option<Arc<EvaluationStageGate>>,
        #[cfg(test)] decode_observer: Option<Arc<AtomicUsize>>,
    ) -> Result<Self, SchedulerError> {
        let (sender, receiver) = mpsc::channel::<DocumentJob>();
        let (completion_sender, completions) = mpsc::channel::<DocumentWorkerCompletion>();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = Arc::clone(&shutdown);
        #[cfg(test)]
        let worker_gate = gate;
        let worker = thread::Builder::new()
            .name("toniator-document-evaluation".into())
            .spawn(move || {
                while !worker_shutdown.load(Ordering::Acquire) {
                    let mut job = match receiver.recv() {
                        Ok(job) => job,
                        Err(_) => break,
                    };
                    while let Ok(next) = receiver.try_recv() {
                        job.cancelled.store(true, Ordering::Release);
                        job = next;
                    }
                    if job.cancelled.load(Ordering::Acquire) {
                        continue;
                    }
                    let token = job.request.snapshot.token();
                    #[cfg(test)]
                    let outcome = match worker_gate.as_ref() {
                        Some(gate) => evaluate_cached_document_with_test_observer(
                            job.request,
                            limits,
                            &job.cache,
                            &ObservedCancellation {
                                cancelled: &job.cancelled,
                                gate,
                            },
                            job.decode_observer.as_deref(),
                        ),
                        None => evaluate_cached_document_with_test_observer(
                            job.request,
                            limits,
                            &job.cache,
                            &AtomicCancellation(&job.cancelled),
                            job.decode_observer.as_deref(),
                        ),
                    };
                    #[cfg(not(test))]
                    let outcome = evaluate_cached_document(
                        job.request,
                        limits,
                        &job.cache,
                        &AtomicCancellation(&job.cancelled),
                    );
                    if job.cancelled.load(Ordering::Acquire) {
                        continue;
                    }
                    let completion = match outcome {
                        Ok(value) => DocumentWorkerCompletion {
                            completion: EvaluationCompletion::Completed {
                                ticket: job.ticket,
                                result: Box::new(value.result),
                                diagnostics: value.diagnostics,
                            },
                            transaction: Some(value.transaction),
                        },
                        Err(EvaluationRunError::Evaluation(error)) => DocumentWorkerCompletion {
                            completion: EvaluationCompletion::Failed {
                                ticket: job.ticket,
                                token,
                                error,
                            },
                            transaction: None,
                        },
                        Err(EvaluationRunError::Cancelled) => continue,
                    };
                    if !job.cancelled.load(Ordering::Acquire) {
                        let _ = completion_sender.send(completion);
                    }
                }
            })
            .map_err(|_| SchedulerError::WorkerSpawn)?;
        Ok(Self {
            state: Mutex::new(DocumentSchedulerState {
                next_ticket: Some(1),
                sender: Some(sender),
                worker: Some(worker),
                latest_cancellation: None,
                latest_ticket: None,
                cache: DocumentDerivedCache::default(),
                pending: None,
                accepted: None,
                #[cfg(test)]
                decode_observer,
            }),
            latest_ticket: Arc::new(AtomicU64::new(0)),
            shutdown,
            completions: Mutex::new(completions),
        })
    }

    pub fn submit(&self, request: EvaluationRequest) -> Result<EvaluationTicket, SchedulerError> {
        let mut state = self
            .state
            .lock()
            .expect("document scheduler state lock poisoned");
        if self.shutdown.load(Ordering::Acquire) {
            return Err(SchedulerError::WorkerUnavailable);
        }
        let value = state
            .next_ticket
            .take()
            .ok_or(SchedulerError::TicketExhausted)?;
        state.next_ticket = value.checked_add(1);
        if let Some(cancelled) = &state.latest_cancellation {
            cancelled.store(true, Ordering::Release);
        }
        state.pending = None;
        state.accepted = None;
        let ticket = EvaluationTicket(value);
        let cancelled = Arc::new(AtomicBool::new(false));
        let cache = state.cache.snapshot();
        #[cfg(test)]
        let decode_observer = state.decode_observer.clone();
        state.latest_cancellation = Some(Arc::clone(&cancelled));
        state.latest_ticket = Some(ticket);
        self.latest_ticket.store(ticket.value(), Ordering::Release);
        state
            .sender
            .as_ref()
            .ok_or(SchedulerError::WorkerUnavailable)?
            .send(DocumentJob {
                ticket,
                request,
                cancelled,
                cache,
                #[cfg(test)]
                decode_observer,
            })
            .map_err(|_| SchedulerError::WorkerUnavailable)?;
        Ok(ticket)
    }

    pub fn is_latest(&self, ticket: EvaluationTicket) -> bool {
        !self.shutdown.load(Ordering::Acquire)
            && self
                .state
                .lock()
                .expect("document scheduler state lock poisoned")
                .latest_ticket
                == Some(ticket)
    }

    #[cfg(test)]
    fn set_next_ticket_for_test(&self, next_ticket: Option<u64>) {
        self.state
            .lock()
            .expect("document scheduler state lock poisoned")
            .next_ticket = next_ticket;
    }

    pub fn try_receive_latest(&self) -> Result<Option<EvaluationCompletion>, SchedulerError> {
        let receiver = self
            .completions
            .lock()
            .expect("document scheduler completion lock poisoned");
        let mut current = None;
        loop {
            match receiver.try_recv() {
                Ok(worker) => {
                    let mut state = self
                        .state
                        .lock()
                        .expect("document scheduler state lock poisoned");
                    if !self.shutdown.load(Ordering::Acquire)
                        && state.latest_ticket == Some(worker.completion.ticket())
                    {
                        if let Some(transaction) = worker.transaction {
                            state.pending = Some((worker.completion.ticket(), transaction));
                        }
                        current = Some(worker.completion);
                    }
                }
                Err(TryRecvError::Empty) => return Ok(current),
                Err(TryRecvError::Disconnected) => {
                    return if self.shutdown.load(Ordering::Acquire) {
                        Ok(current)
                    } else {
                        Err(SchedulerError::WorkerUnavailable)
                    };
                }
            }
        }
    }

    pub fn accept_completion(
        &self,
        completion: &EvaluationCompletion,
        session: &DocumentSession,
    ) -> Result<bool, SchedulerError> {
        let mut state = self
            .state
            .lock()
            .expect("document scheduler state lock poisoned");
        if self.shutdown.load(Ordering::Acquire)
            || state.latest_ticket != Some(completion.ticket())
            || !session.accepts_document_evaluation(completion.token())
        {
            return Ok(false);
        }
        if state.accepted == Some(completion.ticket()) {
            return Ok(true);
        }
        if completion.result().is_some()
            && let Some((ticket, transaction)) = state.pending.take()
            && ticket == completion.ticket()
        {
            state.cache.commit(transaction);
        }
        state.accepted = Some(completion.ticket());
        Ok(true)
    }

    pub fn shutdown(mut self) -> Result<(), SchedulerError> {
        self.stop_worker()
    }

    fn stop_worker(&mut self) -> Result<(), SchedulerError> {
        let worker = {
            let mut state = self
                .state
                .lock()
                .expect("document scheduler state lock poisoned");
            self.shutdown.store(true, Ordering::Release);
            if let Some(cancelled) = &state.latest_cancellation {
                cancelled.store(true, Ordering::Release);
            }
            state.sender.take();
            state.worker.take()
        };
        if let Some(worker) = worker {
            worker.join().map_err(|_| SchedulerError::WorkerPanicked)?;
        }
        Ok(())
    }
}
impl Drop for EvaluationScheduler {
    fn drop(&mut self) {
        let _ = self.stop_worker();
    }
}
impl EvaluationResult {
    pub const fn token(&self) -> DocumentEvaluationToken {
        self.token
    }
    pub fn source_identity(&self) -> &SourceIdentity {
        &self.source_identity
    }
    pub fn channels(&self) -> &[ChannelEvaluationSummary] {
        &self.channels
    }
    pub fn scene(&self) -> &RenderScene {
        &self.scene
    }
    pub fn raster(&self) -> &RasterSurface {
        &self.raster
    }
}

/// Evaluates all authoritative topology channels in stable order. A failing
/// channel aborts before any scene/raster is returned.
pub fn evaluate(request: EvaluationRequest) -> Result<EvaluationResult, EvaluationError> {
    evaluate_with_limits(request, EvaluationLimits::default())
}

pub fn evaluate_with_limits(
    request: EvaluationRequest,
    limits: EvaluationLimits,
) -> Result<EvaluationResult, EvaluationError> {
    evaluate_cached_document(
        request,
        limits,
        &DocumentDerivedCache::default(),
        &NeverCancelled,
    )
    .map(|value| value.result)
    .map_err(|error| match error {
        EvaluationRunError::Evaluation(error) => error,
        EvaluationRunError::Cancelled => unreachable!("synchronous evaluation never cancels"),
    })
}

fn evaluate_cached_document(
    request: EvaluationRequest,
    limits: EvaluationLimits,
    accepted: &DocumentDerivedCache,
    cancellation: &dyn CancellationProbe,
) -> Result<CachedDocumentEvaluation, EvaluationRunError> {
    #[cfg(test)]
    {
        evaluate_cached_document_with_test_observer(request, limits, accepted, cancellation, None)
    }
    #[cfg(not(test))]
    evaluate_cached_document_impl(request, limits, accepted, cancellation)
}

#[cfg(test)]
fn evaluate_cached_document_with_test_observer(
    request: EvaluationRequest,
    limits: EvaluationLimits,
    accepted: &DocumentDerivedCache,
    cancellation: &dyn CancellationProbe,
    decode_observer: Option<&AtomicUsize>,
) -> Result<CachedDocumentEvaluation, EvaluationRunError> {
    evaluate_cached_document_impl(request, limits, accepted, cancellation, decode_observer)
}

fn evaluate_cached_document_impl(
    request: EvaluationRequest,
    limits: EvaluationLimits,
    accepted: &DocumentDerivedCache,
    cancellation: &dyn CancellationProbe,
    #[cfg(test)] decode_observer: Option<&AtomicUsize>,
) -> Result<CachedDocumentEvaluation, EvaluationRunError> {
    if cancellation.is_cancelled() {
        return Err(EvaluationRunError::Cancelled);
    }
    let document = request.snapshot.document();
    document
        .validate()
        .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
    let model = document.channel_model().ok_or(EvaluationError::new(
        "evaluation.channel_topology",
        "complete evaluation requires a modeled topology",
    ))?;
    let topology = document.channel_topology().ok_or(EvaluationError::new(
        "evaluation.channel_topology",
        "complete evaluation requires an installed topology",
    ))?;
    let source_id = match document.source() {
        SourceReference::Assigned(value) => value,
        SourceReference::Unassigned => {
            return Err(EvaluationError::new(
                "evaluation.source_reference",
                "evaluation requires an assigned source reference",
            )
            .into());
        }
    };
    if source_id != request.source.reference_id() {
        return Err(EvaluationError::new(
            "evaluation.source_reference",
            "resolved source reference does not match the document snapshot",
        )
        .into());
    }
    // Complete document preflight intentionally precedes decode: a later
    // invalid topology channel cannot produce an acceptable partial result.
    for channel in topology.channels() {
        if cancellation.is_cancelled() {
            return Err(EvaluationRunError::Cancelled);
        }
        #[cfg(test)]
        cancellation.observe_stage(EvaluationStage::Channel, EvaluationCheckpoint::Before);
        if cancellation.is_cancelled() {
            return Err(EvaluationRunError::Cancelled);
        }
        let definition = document
            .pattern_definitions()
            .iter()
            .find(|value| value.id == channel.pattern_definition_id)
            .ok_or(EvaluationError::new(
                "evaluation.pattern_definition",
                "channel references a missing pattern definition",
            ))?;
        if definition.structure != PatternStructure::StraightGrid
            || definition.output != PatternOutput::CircularMarks
        {
            return Err(EvaluationError::new(
                "evaluation.pattern_definition",
                "unsupported pattern structure or output",
            )
            .into());
        }
    }
    let source_key = SourceCacheKey {
        reference_id: request.source.reference_id().as_str().to_owned(),
        bytes: Arc::clone(&request.source.bytes),
        format: request.source.format(),
        decoder_contract: DECODER_CONTRACT_ID,
    };
    let (source, source_hit) = match &accepted.decoded_source {
        Some((key, value)) if *key == source_key => (Arc::clone(value), CacheDisposition::Hit),
        _ => (
            Arc::new(evaluate_stage(
                EvaluationStage::Decode,
                cancellation,
                || {
                    #[cfg(test)]
                    if let Some(observer) = decode_observer {
                        observer.fetch_add(1, Ordering::Relaxed);
                    }
                    decode_source(request.source.bytes(), request.source.format())
                        .map_err(EvaluationError::from_sampling)
                },
            )?),
            CacheDisposition::Miss,
        ),
    };
    let mut summaries = Vec::with_capacity(topology.channels().len());
    let mut layers = Vec::with_capacity(topology.channels().len());
    let mut families = Vec::with_capacity(topology.channels().len());
    let mut realizations = Vec::with_capacity(topology.channels().len());
    let mut family_dispositions = Vec::with_capacity(topology.channels().len());
    let mut realization_dispositions = Vec::with_capacity(topology.channels().len());
    for channel in topology.channels() {
        if cancellation.is_cancelled() {
            return Err(EvaluationRunError::Cancelled);
        }
        #[cfg(test)]
        cancellation.observe_stage(EvaluationStage::Channel, EvaluationCheckpoint::After);
        if cancellation.is_cancelled() {
            return Err(EvaluationRunError::Cancelled);
        }
        let definition = document
            .pattern_definitions()
            .iter()
            .find(|value| value.id == channel.pattern_definition_id)
            .ok_or(EvaluationError::new(
                "evaluation.pattern_definition",
                "channel references a missing pattern definition",
            ))?;
        if definition.structure != PatternStructure::StraightGrid
            || definition.output != PatternOutput::CircularMarks
        {
            return Err(EvaluationError::new(
                "evaluation.pattern_definition",
                "unsupported pattern structure or output",
            )
            .into());
        }
        let key = document_family_cache_key(document.canvas(), definition, channel, limits);
        let (family, disposition) = match accepted
            .families
            .iter()
            .find(|(candidate, _)| *candidate == key)
        {
            Some((_, family)) => (Arc::clone(family), CacheDisposition::Hit),
            None => (
                Arc::new(evaluate_stage(
                    EvaluationStage::Family,
                    cancellation,
                    || {
                        inspect_straight_grid(&GridInspectRequest {
                            canvas: document.canvas().clone(),
                            density: channel.layout.density.clone(),
                            rotation_degrees: channel.layout.rotation_degrees,
                            translation_x: channel.layout.translation_x,
                            translation_y: channel.layout.translation_y,
                            guard_steps: definition.guard_steps,
                            support_radius: definition.maximum_support_radius,
                            max_family_candidates: limits.max_family_candidates(),
                        })
                        .map_err(EvaluationError::from_grid)
                    },
                )?),
                CacheDisposition::Miss,
            ),
        };
        let realization_key = DocumentRealizationCacheKey {
            family_content: key.content.clone(),
            decoded_identity: source.identity().decoded_pixel_hash.clone(),
            mapping: format!(
                "{:?}:{:?}:{}:{}:{}",
                channel.mapping.component,
                channel.mapping.placement,
                channel.mapping.inverted,
                channel.mapping.gain.to_bits(),
                channel.mapping.bias.to_bits()
            ),
            response: (
                channel.mark_geometry_response.minimum_size.to_bits(),
                channel.mark_geometry_response.maximum_size.to_bits(),
            ),
            sampled_paint: matches!(channel.paint, ChannelPaint::SampledSource),
        };
        let (realization, realization_disposition) = match accepted
            .realizations
            .iter()
            .find(|(candidate, _)| *candidate == realization_key)
        {
            Some((_, realization)) => (Arc::clone(realization), CacheDisposition::Hit),
            None => (
                Arc::new(evaluate_stage(
                    EvaluationStage::Realization,
                    cancellation,
                    || evaluate_document_channel(document, channel, &source, &family),
                )?),
                CacheDisposition::Miss,
            ),
        };
        summaries.push(ChannelEvaluationSummary {
            role: channel.role,
            channel_id: channel.id,
            family_identity: family.family_fingerprint.clone(),
            realization_identity: document_realization_identity(&realization).to_owned(),
        });
        layers.push(document_render_layer(channel, (*realization).clone())?);
        families.push((key, family));
        realizations.push((realization_key, realization));
        family_dispositions.push(disposition);
        realization_dispositions.push(realization_disposition);
        if cancellation.is_cancelled() {
            return Err(EvaluationRunError::Cancelled);
        }
    }
    let family_identity = aggregate_document_identity(
        "family",
        model,
        summaries
            .iter()
            .map(|value| (value.role, value.channel_id, value.family_identity.as_str())),
    );
    let realization_identity = aggregate_document_identity(
        "realization",
        model,
        summaries.iter().map(|value| {
            (
                value.role,
                value.channel_id,
                value.realization_identity.as_str(),
            )
        }),
    );
    let built_scene = evaluate_stage(EvaluationStage::Scene, cancellation, || {
        RenderScene::new_modeled(
            document.canvas().clone(),
            family_identity,
            realization_identity,
            model,
            layers,
        )
        .map_err(EvaluationError::from_render)
    })?;
    let scene_key = built_scene.identity().scene_fingerprint().to_owned();
    let (scene, scene_disposition) = match &accepted.scene {
        Some((key, value)) if *key == scene_key => (Arc::clone(value), CacheDisposition::Hit),
        _ => (Arc::new(built_scene), CacheDisposition::Miss),
    };
    let raster_key = match request.preview_target {
        Some(target) => format!(
            "{}:{TRANSPARENT_RASTER_CONTRACT_ID}:preview-v1:{model:?}:{}x{}",
            scene.identity().scene_fingerprint(),
            target.width(),
            target.height()
        ),
        None => format!(
            "{}:{TRANSPARENT_RASTER_CONTRACT_ID}:{model:?}",
            scene.identity().scene_fingerprint()
        ),
    };
    let (raster, raster_disposition) = match &accepted.raster {
        Some((key, value)) if *key == raster_key => (Arc::clone(value), CacheDisposition::Hit),
        _ => (
            Arc::new(evaluate_stage(
                EvaluationStage::Raster,
                cancellation,
                || {
                    match request.preview_target {
                        Some(target) => rasterize_preview(&scene, target),
                        None => rasterize(&scene, RasterBackground::Transparent),
                    }
                    .map_err(EvaluationError::from_render)
                },
            )?),
            CacheDisposition::Miss,
        ),
    };
    let result = EvaluationResult {
        token: request.snapshot.token(),
        source_identity: source.identity().clone(),
        channels: summaries,
        scene: (*scene).clone(),
        raster: (*raster).clone(),
    };
    let channels = result
        .channels
        .iter()
        .enumerate()
        .map(|(index, value)| ChannelCacheDiagnostics {
            channel_id: value.channel_id,
            family: family_dispositions[index],
            realization: realization_dispositions[index],
        })
        .collect();
    Ok(CachedDocumentEvaluation {
        result,
        diagnostics: DocumentCacheDiagnostics {
            aggregate: CacheDiagnostics {
                decoded_source: source_hit,
                family: if family_dispositions
                    .iter()
                    .all(|value| *value == CacheDisposition::Hit)
                {
                    CacheDisposition::Hit
                } else {
                    CacheDisposition::Miss
                },
                realization: if realization_dispositions
                    .iter()
                    .all(|value| *value == CacheDisposition::Hit)
                {
                    CacheDisposition::Hit
                } else {
                    CacheDisposition::Miss
                },
                scene: scene_disposition,
                raster: raster_disposition,
            },
            channels,
        },
        transaction: DocumentCacheTransaction {
            decoded_source: (source_hit == CacheDisposition::Miss).then_some((source_key, source)),
            families,
            realizations,
            scene: (scene_disposition == CacheDisposition::Miss).then_some((scene_key, scene)),
            raster: (raster_disposition == CacheDisposition::Miss).then_some((raster_key, raster)),
        },
    })
}

#[derive(Clone)]
enum DocumentRealization {
    Mapped(MappedCircularMarkRealization),
    SourceColor(SourceColorCircularMarkRealization),
}
fn document_realization_identity(value: &DocumentRealization) -> &str {
    match value {
        DocumentRealization::Mapped(value) => &value.realization_fingerprint,
        DocumentRealization::SourceColor(value) => &value.realization_fingerprint,
    }
}
fn evaluate_document_channel(
    document: &toniator_domain::Document,
    channel: &ModeledChannelState,
    source: &SourceField,
    family: &GridFamilyOutput,
) -> Result<DocumentRealization, EvaluationError> {
    let response = MarkResponse {
        minimum_size: channel.mark_geometry_response.minimum_size,
        maximum_size: channel.mark_geometry_response.maximum_size,
    };
    let realization = match channel.paint {
        ChannelPaint::Solid(_) => DocumentRealization::Mapped(
            realize_mapped_circular_marks(
                family,
                source,
                document.canvas(),
                channel.mapping,
                response,
            )
            .map_err(EvaluationError::from_realization)?,
        ),
        ChannelPaint::SampledSource => DocumentRealization::SourceColor(
            realize_source_color_circular_marks(
                family,
                source,
                document.canvas(),
                channel.mapping,
                response,
            )
            .map_err(EvaluationError::from_realization)?,
        ),
    };
    Ok(realization)
}
fn document_render_layer(
    channel: &ModeledChannelState,
    realization: DocumentRealization,
) -> Result<RenderLayer, EvaluationError> {
    match realization {
        DocumentRealization::Mapped(value) => match channel.paint {
            ChannelPaint::Solid(ref color) => RenderLayer::new(
                channel.id,
                channel.visible,
                color.clone(),
                channel.opacity,
                GeometryOutput::CircularMarks(value.marks),
            )
            .map_err(EvaluationError::from_render),
            ChannelPaint::SampledSource => unreachable!(),
        },
        DocumentRealization::SourceColor(value) => RenderLayer::new_source_color(
            channel.id,
            channel.visible,
            channel.opacity,
            value
                .marks
                .into_iter()
                .map(|entry| toniator_render::SourceColorCircle {
                    mark: entry.mark,
                    paint: toniator_domain::ColorValue {
                        red: entry.paint.red,
                        green: entry.paint.green,
                        blue: entry.paint.blue,
                        alpha: entry.paint.alpha,
                    },
                })
                .collect(),
        )
        .map_err(EvaluationError::from_render),
    }
}
fn aggregate_document_identity<'a>(
    prefix: &str,
    model: HalftoneChannelModel,
    values: impl IntoIterator<Item = (HalftoneChannelRole, ChannelId, &'a str)>,
) -> String {
    let mut output = format!("{prefix}:{model:?}");
    for (role, id, value) in values {
        output.push_str(&format!(":{role:?}:{}:{value}", id.0));
    }
    output
}

// Stage 9D keeps exactly five last-successful aggregate cache slots. The two
// collections deliberately keep entries per channel key so a document edit can
// reuse unaffected immutable artifacts without making a cache authoritative.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DocumentFamilyContentKey {
    canvas: (u64, u64),
    density: (u64, u64),
    rotation: u64,
    translation: (u64, u64),
    guard_steps: u32,
    support_radius: u64,
    structure: PatternStructure,
    output: PatternOutput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DocumentFamilyCacheKey {
    content: DocumentFamilyContentKey,
    candidate_limit: usize,
}

fn document_family_cache_key(
    canvas: &CanvasSpec,
    definition: &toniator_domain::PatternDefinition,
    channel: &ModeledChannelState,
    limits: EvaluationLimits,
) -> DocumentFamilyCacheKey {
    DocumentFamilyCacheKey {
        content: DocumentFamilyContentKey {
            canvas: (canvas.width.to_bits(), canvas.height.to_bits()),
            density: (
                channel.layout.density.across_x.to_bits(),
                channel.layout.density.across_y.to_bits(),
            ),
            rotation: channel.layout.rotation_degrees.to_bits(),
            translation: (
                channel.layout.translation_x.to_bits(),
                channel.layout.translation_y.to_bits(),
            ),
            guard_steps: definition.guard_steps,
            support_radius: definition.maximum_support_radius.to_bits(),
            structure: definition.structure,
            output: definition.output,
        },
        candidate_limit: limits.max_family_candidates(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DocumentRealizationCacheKey {
    family_content: DocumentFamilyContentKey,
    decoded_identity: String,
    mapping: String,
    response: (u64, u64),
    sampled_paint: bool,
}

#[derive(Clone, Default)]
struct DocumentDerivedCache {
    decoded_source: Option<(SourceCacheKey, Arc<SourceField>)>,
    families: Vec<(DocumentFamilyCacheKey, Arc<GridFamilyOutput>)>,
    realizations: Vec<(DocumentRealizationCacheKey, Arc<DocumentRealization>)>,
    scene: Option<(String, Arc<RenderScene>)>,
    raster: Option<(String, Arc<RasterSurface>)>,
}

#[derive(Clone)]
struct DocumentCacheTransaction {
    decoded_source: Option<(SourceCacheKey, Arc<SourceField>)>,
    families: Vec<(DocumentFamilyCacheKey, Arc<GridFamilyOutput>)>,
    realizations: Vec<(DocumentRealizationCacheKey, Arc<DocumentRealization>)>,
    scene: Option<(String, Arc<RenderScene>)>,
    raster: Option<(String, Arc<RasterSurface>)>,
}

struct CachedDocumentEvaluation {
    result: EvaluationResult,
    diagnostics: DocumentCacheDiagnostics,
    transaction: DocumentCacheTransaction,
}

impl DocumentDerivedCache {
    fn snapshot(&self) -> Self {
        self.clone()
    }

    fn commit(&mut self, transaction: DocumentCacheTransaction) {
        if let Some(source) = transaction.decoded_source {
            self.decoded_source = Some(source);
        }
        self.families = transaction.families;
        self.realizations = transaction.realizations;
        if let Some(scene) = transaction.scene {
            self.scene = Some(scene);
        }
        if let Some(raster) = transaction.raster {
            self.raster = Some(raster);
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::{
        sync::{Arc, atomic::AtomicBool, mpsc},
        thread,
        time::{Duration, Instant},
    };

    use toniator_domain::{
        CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelSourceMapping,
        ChannelState, ChannelTopologyTemplate, ColorValue, DensityMetric2D, Document,
        DocumentCommand, DocumentId, DocumentSession, HalftoneChannelModel, MarkGeometryResponse,
        PatternDefinition, PatternDefinitionId, PatternOutput, PatternStructure, SourceComponent,
        SourcePlacement, SourceReference, SourceReferenceId,
    };

    use super::*;

    const GUARD: Duration = Duration::from_secs(15);
    const CHANNEL_ID: ChannelId = ChannelId(1);

    pub(crate) fn request() -> ChannelDiagnosticRequest {
        request_with_bytes(Arc::<[u8]>::from(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/raster-sample.png"
            ))
            .unwrap(),
        ))
    }

    fn request_with_bytes(bytes: Arc<[u8]>) -> ChannelDiagnosticRequest {
        let source_id = SourceReferenceId::new("cancellation-test-source").unwrap();
        let document = Document::with_source(
            DocumentId(1),
            CanvasSpec {
                width: 900.0,
                height: 600.0,
            },
            SourceReference::Assigned(source_id.clone()),
            vec![PatternDefinition {
                id: PatternDefinitionId(1),
                name: "straight-grid".to_owned(),
                structure: PatternStructure::StraightGrid,
                output: PatternOutput::CircularMarks,
                guard_steps: 2,
                maximum_support_radius: 4.5,
            }],
            vec![ChannelState {
                id: CHANNEL_ID,
                pattern_definition_id: PatternDefinitionId(1),
                layout: ChannelPatternLayout {
                    density: DensityMetric2D {
                        across_x: 90.0,
                        across_y: 60.0,
                        aspect_locked: true,
                    },
                    rotation_degrees: 17.0,
                    translation_x: 3.25,
                    translation_y: -4.5,
                },
                appearance: ChannelAppearance {
                    visible: true,
                    color: ColorValue {
                        red: 0.0,
                        green: srgb_to_linear(183.0 / 255.0),
                        blue: 1.0,
                        alpha: 1.0,
                    },
                    opacity: 0.72,
                },
                mark_geometry_response: MarkGeometryResponse {
                    minimum_size: 2.0,
                    maximum_size: 9.0,
                },
                source_mapping: ChannelSourceMapping {
                    component: SourceComponent::Luminance,
                    placement: SourcePlacement::StretchToCanvas,
                },
            }],
        )
        .unwrap();
        let session = DocumentSession::new(document).unwrap();
        ChannelDiagnosticRequest::new(
            session.evaluation_snapshot(CHANNEL_ID).unwrap(),
            ResolvedSource::new(source_id, bytes, SourceFormatHint::Png).unwrap(),
        )
    }

    fn modeled_document_session() -> DocumentSession {
        modeled_document_session_for(HalftoneChannelModel::Rgb)
    }

    fn modeled_document_session_for(model: HalftoneChannelModel) -> DocumentSession {
        let diagnostic = request();
        let mut session = DocumentSession::new(diagnostic.snapshot.document().clone()).unwrap();
        let channel = session.document().channel(CHANNEL_ID).unwrap();
        let template = ChannelTopologyTemplate {
            pattern_definition_id: channel.pattern_definition_id,
            layout: channel.layout.clone(),
            mark_geometry_response: channel.mark_geometry_response.clone(),
        };
        let topology = session
            .document()
            .canonical_channel_topology(model, template)
            .unwrap();
        session
            .apply(&DocumentCommand::ReplaceChannelTopology { model, topology })
            .unwrap();
        session
    }

    fn document_request(session: &DocumentSession, bytes: Arc<[u8]>) -> EvaluationRequest {
        document_request_for_source(session, "cancellation-test-source", bytes)
    }

    fn document_request_for_source(
        session: &DocumentSession,
        source_id: &str,
        bytes: Arc<[u8]>,
    ) -> EvaluationRequest {
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                SourceReferenceId::new(source_id).unwrap(),
                bytes,
                SourceFormatHint::Png,
            )
            .unwrap(),
        )
    }

    fn valid_document_bytes() -> Arc<[u8]> {
        Arc::<[u8]>::from(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/raster-sample.png"
            ))
            .unwrap(),
        )
    }

    fn wait_for_document_completion(scheduler: &EvaluationScheduler) -> EvaluationCompletion {
        let deadline = Instant::now() + GUARD;
        loop {
            if let Some(completion) = scheduler.try_receive_latest().unwrap() {
                return completion;
            }
            assert!(Instant::now() < deadline, "document scheduler timed out");
            thread::yield_now();
        }
    }

    #[test]
    fn actual_pipeline_cancels_at_each_named_before_and_after_checkpoint() {
        for checkpoint in [EvaluationCheckpoint::Before, EvaluationCheckpoint::After] {
            for stage in [
                EvaluationStage::Family,
                EvaluationStage::Decode,
                EvaluationStage::Realization,
                EvaluationStage::Scene,
                EvaluationStage::Raster,
            ] {
                let (gate, entered) = EvaluationStageGate::new(stage, checkpoint);
                let cancelled = Arc::new(AtomicBool::new(false));
                let worker_gate = Arc::clone(&gate);
                let worker_cancelled = Arc::clone(&cancelled);
                let (result_sender, result_receiver) = mpsc::channel();
                let worker = thread::spawn(move || {
                    result_sender
                        .send(evaluate_channel_diagnostic_cancellable_with_gate(
                            request(),
                            &worker_cancelled,
                            &worker_gate,
                        ))
                        .unwrap();
                });
                entered.recv_timeout(GUARD).unwrap();
                cancelled.store(true, Ordering::Release);
                gate.release();
                assert_eq!(
                    result_receiver.recv_timeout(GUARD).unwrap(),
                    Err(EvaluationRunError::Cancelled),
                    "{stage:?} {checkpoint:?}"
                );
                worker.join().unwrap();
            }
        }
    }

    #[test]
    fn cancellation_after_a_real_decode_error_suppresses_the_failure() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Decode, EvaluationCheckpoint::After);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_gate = Arc::clone(&gate);
        let worker_cancelled = Arc::clone(&cancelled);
        let (result_sender, result_receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            result_sender
                .send(evaluate_channel_diagnostic_cancellable_with_gate(
                    request_with_bytes(Arc::<[u8]>::from(vec![1_u8, 2, 3])),
                    &worker_cancelled,
                    &worker_gate,
                ))
                .unwrap();
        });
        entered.recv_timeout(GUARD).unwrap();
        cancelled.store(true, Ordering::Release);
        gate.release();
        assert_eq!(
            result_receiver.recv_timeout(GUARD).unwrap(),
            Err(EvaluationRunError::Cancelled)
        );
        worker.join().unwrap();
    }

    #[test]
    fn complete_scheduler_cancellation_after_decode_error_is_silent() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Decode, EvaluationCheckpoint::After);
        let scheduler =
            EvaluationScheduler::new_with_test_gate(EvaluationLimits::default(), Arc::clone(&gate))
                .unwrap();
        let session = modeled_document_session();
        let stale_ticket = scheduler
            .submit(document_request(
                &session,
                Arc::<[u8]>::from(vec![1_u8, 2, 3]),
            ))
            .unwrap();
        entered.recv_timeout(GUARD).unwrap();
        let newest_ticket = scheduler
            .submit(document_request(&session, valid_document_bytes()))
            .unwrap();
        gate.release();

        let completion = wait_for_document_completion(&scheduler);
        assert_ne!(stale_ticket, newest_ticket);
        assert_eq!(completion.ticket(), newest_ticket);
        assert!(completion.result().is_some());
        assert!(completion.error().is_none());
        assert!(scheduler.accept_completion(&completion, &session).unwrap());
        scheduler.shutdown().unwrap();
    }

    #[test]
    fn complete_scheduler_decodes_once_per_source_miss() {
        let mut session = modeled_document_session();
        let decode_observer = Arc::new(AtomicUsize::new(0));
        let scheduler = EvaluationScheduler::new_with_test_decode_observer(
            EvaluationLimits::default(),
            Arc::clone(&decode_observer),
        )
        .unwrap();

        let first_ticket = scheduler
            .submit(document_request(&session, valid_document_bytes()))
            .unwrap();
        let first = wait_for_document_completion(&scheduler);
        assert_eq!(first.ticket(), first_ticket);
        assert_eq!(first.result().unwrap().channels().len(), 3);
        assert!(scheduler.accept_completion(&first, &session).unwrap());
        assert_eq!(decode_observer.load(Ordering::Relaxed), 1);

        let repeated_ticket = scheduler
            .submit(document_request(&session, valid_document_bytes()))
            .unwrap();
        let repeated = wait_for_document_completion(&scheduler);
        assert_eq!(repeated.ticket(), repeated_ticket);
        assert!(scheduler.accept_completion(&repeated, &session).unwrap());
        assert_eq!(decode_observer.load(Ordering::Relaxed), 1);

        session
            .apply(&DocumentCommand::SetSourceReference {
                source: SourceReference::Assigned(
                    SourceReferenceId::new("changed-cancellation-test-source").unwrap(),
                ),
            })
            .unwrap();
        let changed_ticket = scheduler
            .submit(document_request_for_source(
                &session,
                "changed-cancellation-test-source",
                valid_document_bytes(),
            ))
            .unwrap();
        let changed = wait_for_document_completion(&scheduler);
        assert_eq!(changed.ticket(), changed_ticket);
        assert!(scheduler.accept_completion(&changed, &session).unwrap());
        assert_eq!(decode_observer.load(Ordering::Relaxed), 2);
        scheduler.shutdown().unwrap();
    }

    #[test]
    fn complete_scheduler_cancels_at_boundary_and_between_channel_checkpoints() {
        for (stage, checkpoint) in [
            (EvaluationStage::Decode, EvaluationCheckpoint::Before),
            (EvaluationStage::Decode, EvaluationCheckpoint::After),
            (EvaluationStage::Family, EvaluationCheckpoint::Before),
            (EvaluationStage::Family, EvaluationCheckpoint::After),
            (EvaluationStage::Realization, EvaluationCheckpoint::Before),
            (EvaluationStage::Realization, EvaluationCheckpoint::After),
            (EvaluationStage::Channel, EvaluationCheckpoint::Before),
            (EvaluationStage::Channel, EvaluationCheckpoint::After),
        ] {
            let (gate, entered) = EvaluationStageGate::new(stage, checkpoint);
            let scheduler = EvaluationScheduler::new_with_test_gate(
                EvaluationLimits::default(),
                Arc::clone(&gate),
            )
            .unwrap();
            let session = modeled_document_session();
            let stale_ticket = scheduler
                .submit(document_request(&session, valid_document_bytes()))
                .unwrap();
            entered.recv_timeout(GUARD).unwrap();
            let newest_ticket = scheduler
                .submit(document_request(&session, valid_document_bytes()))
                .unwrap();
            gate.release();

            let completion = wait_for_document_completion(&scheduler);
            assert_ne!(stale_ticket, newest_ticket, "{stage:?} {checkpoint:?}");
            assert_eq!(
                completion.ticket(),
                newest_ticket,
                "{stage:?} {checkpoint:?}"
            );
            assert!(completion.result().is_some(), "{stage:?} {checkpoint:?}");
            assert!(scheduler.accept_completion(&completion, &session).unwrap());
            scheduler.shutdown().unwrap();
        }
    }

    #[test]
    fn complete_scheduler_reports_checked_ticket_exhaustion_without_panicking() {
        let scheduler = EvaluationScheduler::new().unwrap();
        let session = modeled_document_session();
        scheduler.set_next_ticket_for_test(Some(u64::MAX));
        assert_eq!(
            scheduler
                .submit(document_request(&session, valid_document_bytes()))
                .unwrap()
                .value(),
            u64::MAX
        );
        assert_eq!(
            scheduler.submit(document_request(&session, valid_document_bytes())),
            Err(SchedulerError::TicketExhausted)
        );
        scheduler.shutdown().unwrap();
    }

    #[test]
    fn complete_scheduler_polling_is_nonblocking_and_shutdown_rejects_completion() {
        let mut scheduler = EvaluationScheduler::new().unwrap();
        let session = modeled_document_session();
        assert_eq!(scheduler.try_receive_latest().unwrap(), None);
        let ticket = scheduler
            .submit(document_request(&session, valid_document_bytes()))
            .unwrap();
        let completion = wait_for_document_completion(&scheduler);
        assert_eq!(completion.ticket(), ticket);
        scheduler.stop_worker().unwrap();
        assert!(!scheduler.accept_completion(&completion, &session).unwrap());
        assert_eq!(scheduler.try_receive_latest().unwrap(), None);
        assert_eq!(
            scheduler.submit(document_request(&session, valid_document_bytes())),
            Err(SchedulerError::WorkerUnavailable)
        );
    }

    #[test]
    fn complete_scheduler_shutdown_and_drop_join_gated_work_without_hanging() {
        for explicit_shutdown in [true, false] {
            let (gate, entered) =
                EvaluationStageGate::new(EvaluationStage::Decode, EvaluationCheckpoint::After);
            let scheduler = EvaluationScheduler::new_with_test_gate(
                EvaluationLimits::default(),
                Arc::clone(&gate),
            )
            .unwrap();
            let session = modeled_document_session();
            scheduler
                .submit(document_request(&session, valid_document_bytes()))
                .unwrap();
            entered.recv_timeout(GUARD).unwrap();
            let (done_sender, done_receiver) = mpsc::channel();
            thread::spawn(move || {
                if explicit_shutdown {
                    done_sender.send(scheduler.shutdown()).unwrap();
                } else {
                    drop(scheduler);
                    done_sender.send(Ok(())).unwrap();
                }
            });
            gate.release();
            assert_eq!(done_receiver.recv_timeout(GUARD).unwrap(), Ok(()));
        }
    }
}

#[cfg(test)]
mod cache_key_tests {
    use super::*;

    #[test]
    fn svg_decoded_pixel_identity_participates_in_every_downstream_key() {
        let source = SourceCacheKey {
            reference_id: "svg".to_owned(),
            bytes: Arc::<[u8]>::from(vec![1_u8, 2, 3]),
            format: SourceFormatHint::Svg,
            decoder_contract: DECODER_CONTRACT_ID,
        };
        let family = |decoded: &str, aspect_locked: bool| FamilyCacheKey {
            source: source.clone(),
            decoded_pixel_identity: decoded.to_owned(),
            canvas: (900.0_f64.to_bits(), 600.0_f64.to_bits()),
            density: (90.0_f64.to_bits(), 60.0_f64.to_bits()),
            aspect_locked,
            rotation: 17.0_f64.to_bits(),
            translation: (3.25_f64.to_bits(), (-4.5_f64).to_bits()),
            guard_steps: 2,
            structure: 1,
            output: 1,
            maximum_support_radius: 4.5_f64.to_bits(),
            max_family_candidates: EvaluationLimits::DEFAULT_MAX_FAMILY_CANDIDATES,
        };
        let first_family = family("pixels-a", true);
        let second_family = family("pixels-b", true);
        assert_ne!(first_family, second_family);
        assert_ne!(family("pixels-a", true), family("pixels-a", false));
        let realization = |family: FamilyCacheKey| RealizationCacheKey {
            family,
            decoded_pixel_identity: "pixels-a".to_owned(),
            canvas: (900.0_f64.to_bits(), 600.0_f64.to_bits()),
            source_component: 1,
            placement: 1,
            response: (2.0_f64.to_bits(), 9.0_f64.to_bits()),
        };
        let first_realization = realization(first_family);
        let second_realization = realization(second_family);
        assert_ne!(first_realization, second_realization);
        let scene = |realization: RealizationCacheKey| SceneCacheKey {
            realization,
            canvas: (900.0_f64.to_bits(), 600.0_f64.to_bits()),
            channel_id: 1,
            visible: true,
            color: (0, 0, 0, 1.0_f64.to_bits()),
            opacity: 0.72_f64.to_bits(),
        };
        let first_scene = scene(first_realization);
        let second_scene = scene(second_realization);
        assert_ne!(first_scene, second_scene);
        assert_ne!(
            RasterCacheKey {
                scene: first_scene,
                transparent_raster_contract: TRANSPARENT_RASTER_CONTRACT_ID
            },
            RasterCacheKey {
                scene: second_scene,
                transparent_raster_contract: TRANSPARENT_RASTER_CONTRACT_ID
            },
        );
    }

    #[test]
    fn source_key_invalidation_includes_reference_bytes_format_and_decoder_contract() {
        let baseline = SourceCacheKey {
            reference_id: "source-a".to_owned(),
            bytes: Arc::<[u8]>::from(vec![1_u8, 2, 3]),
            format: SourceFormatHint::Png,
            decoder_contract: "decoder-a",
        };
        let variants = [
            SourceCacheKey {
                reference_id: "source-b".to_owned(),
                ..baseline.clone()
            },
            SourceCacheKey {
                bytes: Arc::<[u8]>::from(vec![1_u8, 2, 4]),
                ..baseline.clone()
            },
            SourceCacheKey {
                format: SourceFormatHint::Svg,
                ..baseline.clone()
            },
            SourceCacheKey {
                decoder_contract: "decoder-b",
                ..baseline.clone()
            },
        ];
        for changed in variants {
            assert_ne!(baseline, changed);
            assert_ne!(
                FamilyCacheKey {
                    source: baseline.clone(),
                    decoded_pixel_identity: "pixels".to_owned(),
                    canvas: (1, 1),
                    density: (1, 1),
                    aspect_locked: true,
                    rotation: 1,
                    translation: (1, 1),
                    guard_steps: 2,
                    structure: 1,
                    output: 1,
                    maximum_support_radius: 1,
                    max_family_candidates: 1,
                },
                FamilyCacheKey {
                    source: changed,
                    decoded_pixel_identity: "pixels".to_owned(),
                    canvas: (1, 1),
                    density: (1, 1),
                    aspect_locked: true,
                    rotation: 1,
                    translation: (1, 1),
                    guard_steps: 2,
                    structure: 1,
                    output: 1,
                    maximum_support_radius: 1,
                    max_family_candidates: 1,
                },
            );
        }
    }
}
