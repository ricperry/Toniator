#![forbid(unsafe_code)]

//! The shared mutable-document boundary for headless Toniator frontends.

use std::{
    error::Error,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

mod scheduler;

pub use scheduler::{EvaluationCompletion, EvaluationScheduler, EvaluationTicket, SchedulerError};

use toniator_domain::{
    CanvasSpec, ChannelId, EvaluationSnapshot, EvaluationToken, PatternOutput, PatternStructure,
    SourceComponent, SourcePlacement, SourceReference, SourceReferenceId,
};
pub use toniator_patterns::{
    CanonicalCircleMark, CircularMarkRealization, MarkResponse, Point2, RealizationError, SiteId,
    SiteScope,
};
use toniator_patterns::{GridFamilyOutput, evaluate_straight_grid, realize_circular_marks};
pub use toniator_render::{
    GeometryOutput, RasterBackground, RasterSurface, RenderError, RenderLayer, RenderScene,
    SceneIdentity, encode_png, linear_to_srgb, rasterize, srgb_to_linear, write_svg,
};
use toniator_sampling::decode_source;
pub use toniator_sampling::{
    SourceField, SourceFormat, SourceFormatHint, SourceIdentity, SvgTextDiagnostic,
};

pub use toniator_patterns::{GridError, GridInspectRequest};

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
pub struct EvaluationRequest {
    snapshot: EvaluationSnapshot,
    source: ResolvedSource,
}

impl EvaluationRequest {
    pub fn new(snapshot: EvaluationSnapshot, source: ResolvedSource) -> Self {
        Self { snapshot, source }
    }

    pub(crate) fn token(&self) -> EvaluationToken {
        self.snapshot.token()
    }
}

/// Immutable result from evaluating one authoritative snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationResult {
    token: EvaluationToken,
    source_identity: SourceIdentity,
    scene: RenderScene,
    raster: RasterSurface,
}

impl EvaluationResult {
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

/// Evaluates exactly the Stage 3 -> Stage 4 -> Stage 5 chain represented by
/// the immutable snapshot. It performs no document mutation or source lookup.
pub fn evaluate(request: EvaluationRequest) -> Result<EvaluationResult, EvaluationError> {
    match evaluate_with_cancellation(request, &NeverCancelled) {
        Ok(result) => Ok(result),
        Err(EvaluationRunError::Evaluation(error)) => Err(error),
        Err(EvaluationRunError::Cancelled) => unreachable!("synchronous evaluation never cancels"),
    }
}

pub(crate) fn evaluate_cancellable(
    request: EvaluationRequest,
    cancelled: &AtomicBool,
) -> Result<EvaluationResult, EvaluationRunError> {
    evaluate_with_cancellation(request, &AtomicCancellation(cancelled))
}

#[cfg(test)]
pub(crate) fn evaluate_cancellable_with_gate(
    request: EvaluationRequest,
    cancelled: &AtomicBool,
    gate: &EvaluationStageGate,
) -> Result<EvaluationResult, EvaluationRunError> {
    evaluate_with_cancellation(request, &ObservedCancellation { cancelled, gate })
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
/// [`evaluate`], checking cancellation only between existing pipeline stages.
fn evaluate_with_cancellation(
    request: EvaluationRequest,
    cancellation: &dyn CancellationProbe,
) -> Result<EvaluationResult, EvaluationRunError> {
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
    let grid = GridInspectRequest {
        canvas: document.canvas().clone(),
        density: channel.layout.density.clone(),
        rotation_degrees: channel.layout.rotation_degrees,
        translation_x: channel.layout.translation_x,
        translation_y: channel.layout.translation_y,
        guard_steps: definition.guard_steps,
        support_radius: response.maximum_size / 2.0,
    };
    let family = evaluate_stage(EvaluationStage::Family, cancellation, || {
        inspect_straight_grid(&grid).map_err(EvaluationError::from_grid)
    })?;
    let source = evaluate_stage(EvaluationStage::Decode, cancellation, || {
        decode_source(request.source.bytes(), request.source.format())
            .map_err(EvaluationError::from_sampling)
    })?;
    let realization = evaluate_stage(EvaluationStage::Realization, cancellation, || {
        realize_from_existing_family(
            &family,
            &source,
            document.canvas(),
            channel.source_mapping.placement,
            channel.source_mapping.component,
            response,
        )
        .map_err(EvaluationError::from_realization)
    })?;
    let scene = evaluate_stage(EvaluationStage::Scene, cancellation, || {
        build_scene(SceneBuild {
            canvas: document.canvas().clone(),
            channel_id,
            visible: channel.appearance.visible,
            color: channel.appearance.color.clone(),
            opacity: channel.appearance.opacity,
            family_fingerprint: realization.family_fingerprint,
            realization_fingerprint: realization.realization_fingerprint,
            marks: realization.marks,
        })
    })?;
    let raster = evaluate_stage(EvaluationStage::Raster, cancellation, || {
        rasterize(&scene, RasterBackground::Transparent).map_err(EvaluationError::from_render)
    })?;
    Ok(EvaluationResult {
        token: request.snapshot.token(),
        source_identity: source.identity().clone(),
        scene,
        raster,
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

#[cfg(test)]
pub(crate) mod test_support {
    use std::{
        sync::{Arc, atomic::AtomicBool, mpsc},
        thread,
        time::Duration,
    };

    use toniator_domain::{
        CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelSourceMapping,
        ChannelState, ColorValue, DensityMetric2D, Document, DocumentId, DocumentSession,
        MarkGeometryResponse, PatternDefinition, PatternDefinitionId, PatternOutput,
        PatternStructure, SourceComponent, SourcePlacement, SourceReference, SourceReferenceId,
    };

    use super::*;

    const GUARD: Duration = Duration::from_secs(15);
    const CHANNEL_ID: ChannelId = ChannelId(1);

    pub(crate) fn request() -> EvaluationRequest {
        request_with_bytes(Arc::<[u8]>::from(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/raster-sample.png"
            ))
            .unwrap(),
        ))
    }

    fn request_with_bytes(bytes: Arc<[u8]>) -> EvaluationRequest {
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
        EvaluationRequest::new(
            session.evaluation_snapshot(CHANNEL_ID).unwrap(),
            ResolvedSource::new(source_id, bytes, SourceFormatHint::Png).unwrap(),
        )
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
                        .send(evaluate_cancellable_with_gate(
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
                .send(evaluate_cancellable_with_gate(
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
}
