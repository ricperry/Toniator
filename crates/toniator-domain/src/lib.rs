#![forbid(unsafe_code)]

//! Authoritative, headless document concepts for Toniator.

use std::{collections::HashSet, error::Error, fmt};

/// A stable identifier for an authoritative document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(pub u64);

/// A stable identifier for a structural pattern definition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatternDefinitionId(pub u64);

/// A stable identifier for a channel owned by a document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelId(pub u64);

/// The discrete revision of an authoritative document session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

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

/// Minimal structural pattern metadata used to validate stable references.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternDefinition {
    pub id: PatternDefinitionId,
    pub name: String,
}

/// Source state deliberately limited to an unresolved, unassigned reference.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SourceReference {
    #[default]
    Unassigned,
}

/// Per-channel authoritative state.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelState {
    pub id: ChannelId,
    pub pattern_definition_id: PatternDefinitionId,
    pub layout: ChannelPatternLayout,
    pub appearance: ChannelAppearance,
    pub mark_geometry_response: MarkGeometryResponse,
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
            validate_channel(channel)?;
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

fn validate_channel(channel: &ChannelState) -> Result<(), ValidationError> {
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
            | Self::SetVisibility { channel_id, .. } => *channel_id,
        }
    }

    fn validate(&self, document: &Document) -> Result<(), ValidationError> {
        if document.channel(self.channel_id()).is_none() {
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
            Self::SetMarkGeometryResponse { response, .. } => {
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
        }
    }

    fn apply_to_valid_document(&self, document: &mut Document) {
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
        }
    }

    fn result(&self) -> CommandResult {
        let invalidation = match self {
            Self::SetDensity { .. } | Self::SetRotation { .. } | Self::SetTranslation { .. } => {
                InvalidationLevel::Family
            }
            Self::SetMarkGeometryResponse { .. } => InvalidationLevel::Realization,
            Self::SetColor { .. } | Self::SetOpacity { .. } | Self::SetVisibility { .. } => {
                InvalidationLevel::Presentation
            }
        };
        CommandResult {
            affected_channels: vec![self.channel_id()],
            invalidation,
        }
    }
}
