#![forbid(unsafe_code)]

//! The shared mutable-document boundary for headless Toniator frontends.

use std::{error::Error, fmt};

use toniator_domain::{
    ChannelId, CommandResult, Document, DocumentCommand, Revision, ValidationError,
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
    SourceComponent, SourceField, SourceFormat, SourceFormatHint, SourcePlacement,
    SvgTextDiagnostic,
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

/// Presentation attached after Stage 4 realization, before renderer selection.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenePresentation {
    pub channel_id: toniator_domain::ChannelId,
    pub visible: bool,
    pub color: toniator_domain::ColorValue,
    pub opacity: f64,
}

/// The Stage 5 shared source-to-scene request. The engine evaluates the family
/// and realization once, then constructs one renderer-independent scene.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSceneRequest<'a> {
    pub marks: MarksInspectRequest<'a>,
    pub presentation: ScenePresentation,
}

pub fn render_scene(request: &RenderSceneRequest<'_>) -> Result<RenderScene, RenderSceneError> {
    let realization = inspect_circular_marks(&request.marks)?;
    RenderScene::new(
        request.marks.grid.canvas.clone(),
        realization.family_fingerprint,
        realization.realization_fingerprint,
        vec![RenderLayer::new(
            request.presentation.channel_id,
            request.presentation.visible,
            request.presentation.color.clone(),
            request.presentation.opacity,
            GeometryOutput::CircularMarks(realization.marks),
        )?],
    )
    .map_err(RenderSceneError::Render)
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderSceneError {
    Marks(MarksInspectError),
    Render(toniator_render::RenderError),
}

impl From<MarksInspectError> for RenderSceneError {
    fn from(error: MarksInspectError) -> Self {
        Self::Marks(error)
    }
}

impl From<toniator_render::RenderError> for RenderSceneError {
    fn from(error: toniator_render::RenderError) -> Self {
        Self::Render(error)
    }
}

impl fmt::Display for RenderSceneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Marks(error) => error.fmt(formatter),
            Self::Render(error) => error.fmt(formatter),
        }
    }
}

impl Error for RenderSceneError {}

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

/// An immutable evaluation identity bound to one document revision and channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationToken {
    pub revision: Revision,
    pub channel_id: ChannelId,
}

/// The exclusive owner of mutable authoritative document state.
#[derive(Clone, Debug)]
pub struct DocumentSession {
    document: Document,
    revision: Revision,
}

impl DocumentSession {
    /// Validates a document before it becomes the session authority.
    pub fn new(document: Document) -> Result<Self, ValidationError> {
        document.validate()?;
        Ok(Self {
            document,
            revision: Revision(0),
        })
    }

    /// Exposes the current document immutably.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Returns an immutable snapshot suitable for external evaluation.
    pub fn snapshot(&self) -> Document {
        self.document.clone()
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Applies a command atomically, advancing the revision exactly once.
    pub fn apply(
        &mut self,
        command: &DocumentCommand,
    ) -> Result<CommandResult, DocumentSessionError> {
        let next_revision = self
            .revision
            .0
            .checked_add(1)
            .map(Revision)
            .ok_or(DocumentSessionError::RevisionExhausted)?;
        let (candidate, result) = self.document.apply_command(command)?;
        self.document = candidate;
        self.revision = next_revision;
        Ok(result)
    }

    /// Creates an evaluation token for a currently owned channel.
    pub fn evaluation_token(
        &self,
        channel_id: ChannelId,
    ) -> Result<EvaluationToken, ValidationError> {
        if self.document.channel(channel_id).is_none() {
            return Err(ValidationError::new(
                "evaluation.channel_id",
                "evaluation targets a missing channel",
            ));
        }
        Ok(EvaluationToken {
            revision: self.revision,
            channel_id,
        })
    }

    /// Returns true only for a result produced against the current revision.
    pub fn accepts_evaluation(&self, token: EvaluationToken) -> bool {
        token.revision == self.revision
    }
}

/// Errors at the session boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentSessionError {
    Validation(ValidationError),
    RevisionExhausted,
}

impl From<ValidationError> for DocumentSessionError {
    fn from(error: ValidationError) -> Self {
        Self::Validation(error)
    }
}

impl fmt::Display for DocumentSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(formatter),
            Self::RevisionExhausted => formatter.write_str("document revision is exhausted"),
        }
    }
}

impl Error for DocumentSessionError {}
