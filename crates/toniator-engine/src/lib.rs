#![forbid(unsafe_code)]

//! The shared mutable-document boundary for headless Toniator frontends.

use std::{error::Error, fmt};

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
    bytes: Box<[u8]>,
    format: SourceFormatHint,
}

impl ResolvedSource {
    pub fn new(
        reference_id: SourceReferenceId,
        bytes: impl Into<Box<[u8]>>,
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
            ));
        }
    };
    if source_id != request.source.reference_id() {
        return Err(EvaluationError::new(
            "evaluation.source_reference",
            "resolved source reference does not match the document snapshot",
        ));
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
        ));
    }
    if definition.output != PatternOutput::CircularMarks {
        return Err(EvaluationError::new(
            "evaluation.pattern_definition.output",
            "unsupported pattern output",
        ));
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
    let family = inspect_straight_grid(&grid).map_err(EvaluationError::from_grid)?;
    let source = decode_source(request.source.bytes(), request.source.format())
        .map_err(EvaluationError::from_sampling)?;
    let realization = realize_from_existing_family(
        &family,
        &source,
        document.canvas(),
        channel.source_mapping.placement,
        channel.source_mapping.component,
        response,
    )
    .map_err(EvaluationError::from_realization)?;
    let scene = build_scene(SceneBuild {
        canvas: document.canvas().clone(),
        channel_id,
        visible: channel.appearance.visible,
        color: channel.appearance.color.clone(),
        opacity: channel.appearance.opacity,
        family_fingerprint: realization.family_fingerprint,
        realization_fingerprint: realization.realization_fingerprint,
        marks: realization.marks,
    })?;
    let raster =
        rasterize(&scene, RasterBackground::Transparent).map_err(EvaluationError::from_render)?;
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
