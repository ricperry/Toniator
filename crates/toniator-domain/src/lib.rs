#![forbid(unsafe_code)]

//! Authoritative, headless document concepts for Toniator.

use std::{collections::HashSet, error::Error, fmt};

use serde::Serialize;

/// A stable identifier for an authoritative document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(pub u64);

/// A stable identifier for a structural pattern definition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatternDefinitionId(pub u64);

/// A stable identifier for one structural guide dimension.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuideDimensionId(pub u64);

/// A stable identifier for a channel owned by a document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelId(pub u64);

/// The discrete revision of an authoritative document session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

/// A stable logical identifier for source artwork. It deliberately does not
/// encode a filesystem location, source bytes, or decoded pixels.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct SourceReferenceId(String);

impl SourceReferenceId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ValidationError::new(
                "source.reference_id",
                "source reference ID must not be empty",
            ));
        }
        if value.contains('/') || value.contains('\\') {
            return Err(ValidationError::new(
                "source.reference_id",
                "source reference ID must not be a filesystem path",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Document-space canvas dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSpec {
    pub width: f64,
    pub height: f64,
}

/// The continuous density metric supplied to a channel's pattern layout.
#[derive(Clone, Debug, PartialEq)]
pub struct DensityMetric2D {
    pub across_x: f64,
    pub across_y: f64,
    pub aspect_locked: bool,
}

/// Per-channel placement controls.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelPatternLayout {
    pub density: DensityMetric2D,
    pub rotation_degrees: f64,
    pub translation_x: f64,
    pub translation_y: f64,
}

/// Canonical linear RGBA color components.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorValue {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

/// Presentation settings that do not alter canonical geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelAppearance {
    pub visible: bool,
    pub color: ColorValue,
    pub opacity: f64,
}

/// The current minimal mark-size response contract.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkGeometryResponse {
    pub minimum_size: f64,
    pub maximum_size: f64,
}

/// The source component selected by a channel's authoritative source mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceComponent {
    Luminance,
    Alpha,
}

/// The sole supported document-to-source placement contract.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourcePlacement {
    #[default]
    StretchToCanvas,
}

/// Authoritative source interpretation for one channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelSourceMapping {
    pub component: SourceComponent,
    pub placement: SourcePlacement,
}

/// The bounded structural family supported by the Stage 3–6 pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternStructure {
    StraightGrid,
    /// Reserved malformed/external data case; no evaluator supports it.
    Unsupported,
}

/// The bounded canonical output declaration supported by this pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternOutput {
    CircularMarks,
    /// Reserved malformed/external data case; no evaluator supports it.
    Unsupported,
}

/// Structural pattern metadata owned by the authoritative document.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternDefinition {
    pub id: PatternDefinitionId,
    pub name: String,
    pub structure: PatternStructure,
    pub output: PatternOutput,
    pub guard_steps: u32,
    /// The largest canonical mark radius this structural definition can cover.
    /// It is a capability, not a transient response default.
    pub maximum_support_radius: f64,
}

/// Source state owned by the document, never a filesystem path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SourceReference {
    #[default]
    Unassigned,
    Assigned(SourceReferenceId),
}

/// Per-channel authoritative state.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelState {
    pub id: ChannelId,
    pub pattern_definition_id: PatternDefinitionId,
    pub layout: ChannelPatternLayout,
    pub appearance: ChannelAppearance,
    pub mark_geometry_response: MarkGeometryResponse,
    pub source_mapping: ChannelSourceMapping,
}

/// The sole document model. Its collections are read-only outside this crate.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    id: DocumentId,
    canvas: CanvasSpec,
    source: SourceReference,
    pattern_definitions: Vec<PatternDefinition>,
    channels: Vec<ChannelState>,
}

impl Document {
    /// Constructs and validates a document with an unassigned source reference.
    pub fn new(
        id: DocumentId,
        canvas: CanvasSpec,
        pattern_definitions: Vec<PatternDefinition>,
        channels: Vec<ChannelState>,
    ) -> Result<Self, ValidationError> {
        Self::with_source(
            id,
            canvas,
            SourceReference::Unassigned,
            pattern_definitions,
            channels,
        )
    }

    /// Constructs and validates a document with explicitly supplied source state.
    pub fn with_source(
        id: DocumentId,
        canvas: CanvasSpec,
        source: SourceReference,
        pattern_definitions: Vec<PatternDefinition>,
        channels: Vec<ChannelState>,
    ) -> Result<Self, ValidationError> {
        let document = Self {
            id,
            canvas,
            source,
            pattern_definitions,
            channels,
        };
        document.validate()?;
        Ok(document)
    }

    pub fn id(&self) -> DocumentId {
        self.id
    }

    pub fn canvas(&self) -> &CanvasSpec {
        &self.canvas
    }

    pub fn source(&self) -> &SourceReference {
        &self.source
    }

    pub fn pattern_definitions(&self) -> &[PatternDefinition] {
        &self.pattern_definitions
    }

    pub fn channels(&self) -> &[ChannelState] {
        &self.channels
    }

    pub fn channel(&self, channel_id: ChannelId) -> Option<&ChannelState> {
        self.channels
            .iter()
            .find(|channel| channel.id == channel_id)
    }

    /// Validates the complete persisted document contract.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_positive_finite(self.canvas.width, "canvas.width")?;
        validate_positive_finite(self.canvas.height, "canvas.height")?;

        let mut definition_ids = HashSet::new();
        for definition in &self.pattern_definitions {
            if !definition_ids.insert(definition.id) {
                return Err(ValidationError::new(
                    "pattern_definitions",
                    "document-owned pattern definition IDs must be unique",
                ));
            }
            if definition.name.trim().is_empty() {
                return Err(ValidationError::new(
                    "pattern_definitions.name",
                    "pattern definition name must not be empty",
                ));
            }
            if definition.structure == PatternStructure::Unsupported {
                return Err(ValidationError::new(
                    "pattern_definitions.structure",
                    "unsupported pattern structure",
                ));
            }
            if definition.output == PatternOutput::Unsupported {
                return Err(ValidationError::new(
                    "pattern_definitions.output",
                    "unsupported pattern output",
                ));
            }
            validate_nonnegative_finite(
                definition.maximum_support_radius,
                "pattern_definitions.maximum_support_radius",
            )?;
        }

        let mut channel_ids = HashSet::new();
        for channel in &self.channels {
            if !channel_ids.insert(channel.id) {
                return Err(ValidationError::new(
                    "channels",
                    "document-owned channel IDs must be unique",
                ));
            }
            if !definition_ids.contains(&channel.pattern_definition_id) {
                return Err(ValidationError::new(
                    "channel.pattern.definition_id",
                    "channel references a missing pattern definition",
                ));
            }
            let definition = self
                .pattern_definitions
                .iter()
                .find(|definition| definition.id == channel.pattern_definition_id)
                .expect("definition existence was checked above");
            validate_channel(channel, definition)?;
        }
        Ok(())
    }

    /// Produces one fully validated document transition without mutating `self`.
    ///
    /// `toniator-engine` is responsible for atomically installing the returned
    /// candidate alongside the corresponding revision advance.
    pub fn apply_command(
        &self,
        command: &DocumentCommand,
    ) -> Result<(Self, CommandResult), ValidationError> {
        command.validate(self)?;
        let mut candidate = self.clone();
        command.apply_to_valid_document(&mut candidate);
        candidate.validate()?;
        Ok((candidate, command.result()))
    }
}

fn validate_channel(
    channel: &ChannelState,
    definition: &PatternDefinition,
) -> Result<(), ValidationError> {
    validate_positive_finite(
        channel.layout.density.across_x,
        "channel.pattern.layout.density.across_x",
    )?;
    validate_positive_finite(
        channel.layout.density.across_y,
        "channel.pattern.layout.density.across_y",
    )?;
    validate_finite(
        channel.layout.rotation_degrees,
        "channel.pattern.layout.rotation_degrees",
    )?;
    validate_finite(
        channel.layout.translation_x,
        "channel.pattern.layout.translation_x",
    )?;
    validate_finite(
        channel.layout.translation_y,
        "channel.pattern.layout.translation_y",
    )?;
    validate_unit_component(channel.appearance.color.red, "channel.appearance.color.red")?;
    validate_unit_component(
        channel.appearance.color.green,
        "channel.appearance.color.green",
    )?;
    validate_unit_component(
        channel.appearance.color.blue,
        "channel.appearance.color.blue",
    )?;
    validate_unit_component(
        channel.appearance.color.alpha,
        "channel.appearance.color.alpha",
    )?;
    validate_unit_component(channel.appearance.opacity, "channel.appearance.opacity")?;
    validate_nonnegative_finite(
        channel.mark_geometry_response.minimum_size,
        "channel.pattern.mark_geometry_response.minimum_size",
    )?;
    validate_nonnegative_finite(
        channel.mark_geometry_response.maximum_size,
        "channel.pattern.mark_geometry_response.maximum_size",
    )?;
    if channel.mark_geometry_response.minimum_size > channel.mark_geometry_response.maximum_size {
        return Err(ValidationError::new(
            "channel.pattern.mark_geometry_response",
            "minimum_size must not exceed maximum_size",
        ));
    }
    if channel.mark_geometry_response.maximum_size / 2.0 > definition.maximum_support_radius {
        return Err(ValidationError::new(
            "channel.pattern.mark_geometry_response.maximum_size",
            "maximum_size exceeds the pattern definition support capability",
        ));
    }
    Ok(())
}

fn validate_finite(value: f64, path: &'static str) -> Result<(), ValidationError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ValidationError::new(path, "value must be finite"))
    }
}

fn validate_positive_finite(value: f64, path: &'static str) -> Result<(), ValidationError> {
    validate_finite(value, path)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(ValidationError::new(
            path,
            "value must be greater than zero",
        ))
    }
}

fn validate_nonnegative_finite(value: f64, path: &'static str) -> Result<(), ValidationError> {
    validate_finite(value, path)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(ValidationError::new(path, "value must not be negative"))
    }
}

fn validate_unit_component(value: f64, path: &'static str) -> Result<(), ValidationError> {
    validate_finite(value, path)?;
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::new(path, "value must be within 0.0..=1.0"))
    }
}

/// A schema-scoped validation failure suitable for a frontend error display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError {
    path: &'static str,
    message: &'static str,
}

impl ValidationError {
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }

    pub const fn path(&self) -> &'static str {
        self.path
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for ValidationError {}

/// The pipeline layer invalidated by a committed document command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidationLevel {
    Presentation,
    Realization,
    Family,
    Source,
}

/// The committed effect of a document command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandResult {
    pub affected_channels: Vec<ChannelId>,
    pub invalidation: InvalidationLevel,
}

/// Supported channel edits in the Stage 2 authoritative command boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum DocumentCommand {
    SetDensity {
        channel_id: ChannelId,
        density: DensityMetric2D,
    },
    SetRotation {
        channel_id: ChannelId,
        rotation_degrees: f64,
    },
    SetTranslation {
        channel_id: ChannelId,
        translation_x: f64,
        translation_y: f64,
    },
    SetMarkGeometryResponse {
        channel_id: ChannelId,
        response: MarkGeometryResponse,
    },
    SetColor {
        channel_id: ChannelId,
        color: ColorValue,
    },
    SetOpacity {
        channel_id: ChannelId,
        opacity: f64,
    },
    SetVisibility {
        channel_id: ChannelId,
        visible: bool,
    },
    SetSourceReference {
        source: SourceReference,
    },
    SetSourceMapping {
        channel_id: ChannelId,
        mapping: ChannelSourceMapping,
    },
}

impl DocumentCommand {
    fn channel_id(&self) -> ChannelId {
        match self {
            Self::SetDensity { channel_id, .. }
            | Self::SetRotation { channel_id, .. }
            | Self::SetTranslation { channel_id, .. }
            | Self::SetMarkGeometryResponse { channel_id, .. }
            | Self::SetColor { channel_id, .. }
            | Self::SetOpacity { channel_id, .. }
            | Self::SetVisibility { channel_id, .. }
            | Self::SetSourceMapping { channel_id, .. } => *channel_id,
            Self::SetSourceReference { .. } => ChannelId(0),
        }
    }

    fn validate(&self, document: &Document) -> Result<(), ValidationError> {
        if !matches!(self, Self::SetSourceReference { .. })
            && document.channel(self.channel_id()).is_none()
        {
            return Err(ValidationError::new(
                "command.channel_id",
                "command targets a missing channel",
            ));
        }

        match self {
            Self::SetDensity { density, .. } => {
                validate_positive_finite(
                    density.across_x,
                    "channel.pattern.layout.density.across_x",
                )?;
                validate_positive_finite(
                    density.across_y,
                    "channel.pattern.layout.density.across_y",
                )
            }
            Self::SetRotation {
                rotation_degrees, ..
            } => validate_finite(*rotation_degrees, "channel.pattern.layout.rotation_degrees"),
            Self::SetTranslation {
                translation_x,
                translation_y,
                ..
            } => {
                validate_finite(*translation_x, "channel.pattern.layout.translation_x")?;
                validate_finite(*translation_y, "channel.pattern.layout.translation_y")
            }
            Self::SetMarkGeometryResponse {
                channel_id,
                response,
            } => {
                validate_nonnegative_finite(
                    response.minimum_size,
                    "channel.pattern.mark_geometry_response.minimum_size",
                )?;
                validate_nonnegative_finite(
                    response.maximum_size,
                    "channel.pattern.mark_geometry_response.maximum_size",
                )?;
                if response.minimum_size > response.maximum_size {
                    return Err(ValidationError::new(
                        "channel.pattern.mark_geometry_response",
                        "minimum_size must not exceed maximum_size",
                    ));
                }
                let channel = document
                    .channel(*channel_id)
                    .expect("command channel existence was checked above");
                let definition = document
                    .pattern_definitions
                    .iter()
                    .find(|definition| definition.id == channel.pattern_definition_id)
                    .expect("document validation keeps channel definitions valid");
                if response.maximum_size / 2.0 > definition.maximum_support_radius {
                    return Err(ValidationError::new(
                        "channel.pattern.mark_geometry_response.maximum_size",
                        "maximum_size exceeds the pattern definition support capability",
                    ));
                }
                Ok(())
            }
            Self::SetColor { color, .. } => {
                validate_unit_component(color.red, "channel.appearance.color.red")?;
                validate_unit_component(color.green, "channel.appearance.color.green")?;
                validate_unit_component(color.blue, "channel.appearance.color.blue")?;
                validate_unit_component(color.alpha, "channel.appearance.color.alpha")
            }
            Self::SetOpacity { opacity, .. } => {
                validate_unit_component(*opacity, "channel.appearance.opacity")
            }
            Self::SetVisibility { .. } => Ok(()),
            Self::SetSourceReference { source } => match source {
                SourceReference::Unassigned => Ok(()),
                SourceReference::Assigned(id) if id.as_str().trim().is_empty() => {
                    Err(ValidationError::new(
                        "source.reference_id",
                        "source reference ID must not be empty",
                    ))
                }
                SourceReference::Assigned(_) => Ok(()),
            },
            Self::SetSourceMapping { .. } => Ok(()),
        }
    }

    fn apply_to_valid_document(&self, document: &mut Document) {
        if let Self::SetSourceReference { source } = self {
            document.source = source.clone();
            return;
        }
        let channel = document
            .channels
            .iter_mut()
            .find(|channel| channel.id == self.channel_id())
            .expect("validated command must target an existing channel");

        match self {
            Self::SetDensity { density, .. } => channel.layout.density = density.clone(),
            Self::SetRotation {
                rotation_degrees, ..
            } => channel.layout.rotation_degrees = *rotation_degrees,
            Self::SetTranslation {
                translation_x,
                translation_y,
                ..
            } => {
                channel.layout.translation_x = *translation_x;
                channel.layout.translation_y = *translation_y;
            }
            Self::SetMarkGeometryResponse { response, .. } => {
                channel.mark_geometry_response = response.clone();
            }
            Self::SetColor { color, .. } => channel.appearance.color = color.clone(),
            Self::SetOpacity { opacity, .. } => channel.appearance.opacity = *opacity,
            Self::SetVisibility { visible, .. } => channel.appearance.visible = *visible,
            Self::SetSourceMapping { mapping, .. } => channel.source_mapping = *mapping,
            Self::SetSourceReference { .. } => unreachable!("handled before channel lookup"),
        }
    }

    fn result(&self) -> CommandResult {
        let invalidation = match self {
            Self::SetDensity { .. } | Self::SetRotation { .. } | Self::SetTranslation { .. } => {
                InvalidationLevel::Family
            }
            Self::SetMarkGeometryResponse { .. } => InvalidationLevel::Realization,
            Self::SetSourceMapping { .. } => InvalidationLevel::Realization,
            Self::SetSourceReference { .. } => InvalidationLevel::Source,
            Self::SetColor { .. } | Self::SetOpacity { .. } | Self::SetVisibility { .. } => {
                InvalidationLevel::Presentation
            }
        };
        CommandResult {
            affected_channels: match self {
                Self::SetSourceReference { .. } => self.channels_for_source_change(),
                _ => vec![self.channel_id()],
            },
            invalidation,
        }
    }
}

impl DocumentCommand {
    fn channels_for_source_change(&self) -> Vec<ChannelId> {
        // Source invalidation applies to the complete document; callers retain
        // the immutable snapshot to discover the current channel list.
        Vec::new()
    }
}

/// An immutable revision/channel token. Its fields are private so callers can
/// observe, but not manufacture, a different revision/channel pairing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationToken {
    revision: Revision,
    channel_id: ChannelId,
}

impl EvaluationToken {
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn channel_id(&self) -> ChannelId {
        self.channel_id
    }
}

/// One atomic read of the authority for a requested channel. A snapshot owns
/// both the document clone and its exact token and has no public constructor.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationSnapshot {
    document: Document,
    token: EvaluationToken,
}

impl EvaluationSnapshot {
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn token(&self) -> EvaluationToken {
        self.token
    }
}

/// The exclusive owner of mutable authoritative document state.
#[derive(Clone, Debug)]
pub struct DocumentSession {
    document: Document,
    revision: Revision,
}

impl DocumentSession {
    pub fn new(document: Document) -> Result<Self, ValidationError> {
        document.validate()?;
        Ok(Self {
            document,
            revision: Revision(0),
        })
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn snapshot(&self) -> Document {
        self.document.clone()
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
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
        let (candidate, mut result) = self.document.apply_command(command)?;
        if matches!(command, DocumentCommand::SetSourceReference { .. }) {
            result.affected_channels = candidate
                .channels
                .iter()
                .map(|channel| channel.id)
                .collect();
        }
        self.document = candidate;
        self.revision = next_revision;
        Ok(result)
    }
    pub fn evaluation_snapshot(
        &self,
        channel_id: ChannelId,
    ) -> Result<EvaluationSnapshot, ValidationError> {
        if self.document.channel(channel_id).is_none() {
            return Err(ValidationError::new(
                "evaluation.channel_id",
                "evaluation targets a missing channel",
            ));
        }
        Ok(EvaluationSnapshot {
            document: self.document.clone(),
            token: EvaluationToken {
                revision: self.revision,
                channel_id,
            },
        })
    }
    pub fn evaluation_token(
        &self,
        channel_id: ChannelId,
    ) -> Result<EvaluationToken, ValidationError> {
        Ok(self.evaluation_snapshot(channel_id)?.token())
    }
    pub fn accepts_evaluation(&self, token: EvaluationToken) -> bool {
        token.revision == self.revision
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentSessionError {
    Validation(ValidationError),
    RevisionExhausted,
}
impl From<ValidationError> for DocumentSessionError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}
impl fmt::Display for DocumentSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(f),
            Self::RevisionExhausted => f.write_str("document revision is exhausted"),
        }
    }
}
impl Error for DocumentSessionError {}
