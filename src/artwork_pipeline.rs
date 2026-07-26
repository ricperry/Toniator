//! Authoritative artwork-source, alpha, output, and assignment vocabulary.
//!
//! `Document::artwork_pipeline` stores this state directly. The combined GTK
//! mapping control and existing renderers consume temporary projections until
//! their later TON-012 stages replace those runtime adapters.

use crate::model::{Ink, OutputMode, ValueMode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::error::Error;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownStableIdError {
    pub kind: &'static str,
    pub id: String,
}

impl fmt::Display for UnknownStableIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown {} identifier: {}", self.kind, self.id)
    }
}
impl Error for UnknownStableIdError {}

macro_rules! stable_id_enum {
    ($type:ident, $kind:literal, { $($variant:ident => ($id:literal, $label:literal)),+ $(,)? }) => {
        impl $type {
            pub const fn stable_id(self) -> &'static str { match self { $(Self::$variant => $id,)+ } }
            pub const fn label(self) -> &'static str { match self { $(Self::$variant => $label,)+ } }
        }
        impl FromStr for $type {
            type Err = UnknownStableIdError;
            fn from_str(id: &str) -> Result<Self, Self::Err> {
                match id { $($id => Ok(Self::$variant),)+ _ => Err(UnknownStableIdError { kind: $kind, id: id.to_owned() }) }
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyBrightnessKind {
    EncodedRec709InvertedV1,
}
stable_id_enum!(LegacyBrightnessKind, "legacy brightness", {
    EncodedRec709InvertedV1 => ("source.legacy_brightness.encoded_rec709_inverted_v1", "Legacy Brightness (inverted Rec.709)")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkSource {
    FullColor,
    Red,
    Green,
    Blue,
    Value,
    PerceptualLightness,
    Alpha,
    LegacyBrightness(LegacyBrightnessKind),
}
impl ArtworkSource {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::FullColor => "source.full_color",
            Self::Red => "source.red",
            Self::Green => "source.green",
            Self::Blue => "source.blue",
            Self::Value => "source.value",
            Self::PerceptualLightness => "source.perceptual_lightness",
            Self::Alpha => "source.alpha",
            Self::LegacyBrightness(kind) => kind.stable_id(),
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::FullColor => "Full Color",
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Value => "Value",
            Self::PerceptualLightness => "Perceptual Lightness",
            Self::Alpha => "Alpha",
            Self::LegacyBrightness(kind) => kind.label(),
        }
    }
    pub const fn is_scalar(self) -> bool {
        !matches!(self, Self::FullColor)
    }
}
impl FromStr for ArtworkSource {
    type Err = UnknownStableIdError;
    fn from_str(id: &str) -> Result<Self, Self::Err> {
        Ok(match id {
            "source.full_color" => Self::FullColor,
            "source.red" => Self::Red,
            "source.green" => Self::Green,
            "source.blue" => Self::Blue,
            "source.value" => Self::Value,
            "source.perceptual_lightness" => Self::PerceptualLightness,
            "source.alpha" => Self::Alpha,
            "source.legacy_brightness.encoded_rec709_inverted_v1" => {
                Self::LegacyBrightness(LegacyBrightnessKind::EncodedRec709InvertedV1)
            }
            _ => {
                return Err(UnknownStableIdError {
                    kind: "artwork source",
                    id: id.to_owned(),
                });
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAlphaPolicy {
    LegacyCurrentV1,
    Preserve,
    Ignore,
}
stable_id_enum!(SourceAlphaPolicy, "source alpha policy", {
    LegacyCurrentV1 => ("source_alpha.legacy_current_v1", "Legacy source alpha"),
    Preserve => ("source_alpha.preserve", "Preserve source alpha"),
    Ignore => ("source_alpha.ignore", "Ignore source alpha")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputModel {
    CmykPrint,
    RgbScreen,
}
stable_id_enum!(OutputModel, "output model", {
    CmykPrint => ("output.cmyk_print", "CMYK Print"), RgbScreen => ("output.rgb_screen", "RGB Screen")
});
impl OutputModel {
    pub const fn channels(self) -> &'static [OutputChannelId] {
        match self {
            Self::CmykPrint => &OutputChannelId::CMYK,
            Self::RgbScreen => &OutputChannelId::RGB,
        }
    }
    pub const fn default_channel(self) -> OutputChannelId {
        match self {
            Self::CmykPrint => OutputChannelId::CmykCyan,
            Self::RgbScreen => OutputChannelId::RgbRed,
        }
    }
    pub const fn from_legacy(mode: OutputMode) -> Self {
        match mode {
            OutputMode::CmykInks => Self::CmykPrint,
            OutputMode::RgbScreen => Self::RgbScreen,
        }
    }
    pub const fn to_legacy(self) -> OutputMode {
        match self {
            Self::CmykPrint => OutputMode::CmykInks,
            Self::RgbScreen => OutputMode::RgbScreen,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputChannelId {
    CmykCyan,
    CmykMagenta,
    CmykYellow,
    CmykBlack,
    RgbRed,
    RgbGreen,
    RgbBlue,
}
impl OutputChannelId {
    pub const CMYK: [Self; 4] = [
        Self::CmykCyan,
        Self::CmykMagenta,
        Self::CmykYellow,
        Self::CmykBlack,
    ];
    pub const RGB: [Self; 3] = [Self::RgbRed, Self::RgbGreen, Self::RgbBlue];
    pub const fn output_model(self) -> OutputModel {
        match self {
            Self::CmykCyan | Self::CmykMagenta | Self::CmykYellow | Self::CmykBlack => {
                OutputModel::CmykPrint
            }
            Self::RgbRed | Self::RgbGreen | Self::RgbBlue => OutputModel::RgbScreen,
        }
    }
    pub fn belongs_to(self, output: OutputModel) -> bool {
        self.output_model() == output
    }
    pub fn from_legacy_slot(slot: u32, output: OutputModel) -> Result<Self, LegacySlotError> {
        match (slot, output) {
            (0, OutputModel::CmykPrint) => Ok(Self::CmykCyan),
            (1, OutputModel::CmykPrint) => Ok(Self::CmykMagenta),
            (2, OutputModel::CmykPrint) => Ok(Self::CmykYellow),
            (3, OutputModel::CmykPrint) => Ok(Self::CmykBlack),
            (0, OutputModel::RgbScreen) => Ok(Self::RgbRed),
            (1, OutputModel::RgbScreen) => Ok(Self::RgbGreen),
            (2, OutputModel::RgbScreen) => Ok(Self::RgbBlue),
            _ => Err(LegacySlotError { slot, output }),
        }
    }
    pub const fn legacy_slot(self) -> u32 {
        match self {
            Self::CmykCyan | Self::RgbRed => 0,
            Self::CmykMagenta | Self::RgbGreen => 1,
            Self::CmykYellow | Self::RgbBlue => 2,
            Self::CmykBlack => 3,
        }
    }
    pub const fn from_legacy_ink(ink: Ink) -> Self {
        match ink {
            Ink::Cyan => Self::CmykCyan,
            Ink::Magenta => Self::CmykMagenta,
            Ink::Yellow => Self::CmykYellow,
            Ink::Black => Self::CmykBlack,
            Ink::Red => Self::RgbRed,
            Ink::Green => Self::RgbGreen,
            Ink::Blue => Self::RgbBlue,
        }
    }
    pub const fn to_legacy_ink(self) -> Ink {
        match self {
            Self::CmykCyan => Ink::Cyan,
            Self::CmykMagenta => Ink::Magenta,
            Self::CmykYellow => Ink::Yellow,
            Self::CmykBlack => Ink::Black,
            Self::RgbRed => Ink::Red,
            Self::RgbGreen => Ink::Green,
            Self::RgbBlue => Ink::Blue,
        }
    }
}
stable_id_enum!(OutputChannelId, "output channel", {
    CmykCyan => ("channel.cmyk.cyan", "Cyan"), CmykMagenta => ("channel.cmyk.magenta", "Magenta"),
    CmykYellow => ("channel.cmyk.yellow", "Yellow"), CmykBlack => ("channel.cmyk.black", "Black"),
    RgbRed => ("channel.rgb.red", "Red"), RgbGreen => ("channel.rgb.green", "Green"), RgbBlue => ("channel.rgb.blue", "Blue")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomaticSeparationStrategy {
    CmykEncodedRgbMaxBlackV1,
    RgbDirectEncodedComponentsV1,
}
stable_id_enum!(AutomaticSeparationStrategy, "automatic separation strategy", {
    CmykEncodedRgbMaxBlackV1 => ("separation.cmyk.encoded_rgb_max_black_v1", "CMYK encoded RGB max black"),
    RgbDirectEncodedComponentsV1 => ("separation.rgb.direct_encoded_components_v1", "RGB direct encoded components")
});
impl AutomaticSeparationStrategy {
    pub const fn output_model(self) -> OutputModel {
        match self {
            Self::CmykEncodedRgbMaxBlackV1 => OutputModel::CmykPrint,
            Self::RgbDirectEncodedComponentsV1 => OutputModel::RgbScreen,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyCompatibilityAssignment {
    CrosshatchProgressiveKcmyV1,
}
stable_id_enum!(LegacyCompatibilityAssignment, "legacy compatibility assignment", {
    CrosshatchProgressiveKcmyV1 => ("compat.crosshatch.progressive_kcmy_v1", "Legacy Crosshatch")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelAssignment {
    Automatic {
        strategy: AutomaticSeparationStrategy,
    },
    ActiveChannel,
    AllChannels,
    LegacyCompatibility(LegacyCompatibilityAssignment),
}
impl ChannelAssignment {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::Automatic { .. } => "assignment.automatic",
            Self::ActiveChannel => "assignment.active_channel",
            Self::AllChannels => "assignment.all_channels",
            Self::LegacyCompatibility(kind) => kind.stable_id(),
        }
    }
    pub const fn payload_id(self) -> Option<&'static str> {
        match self {
            Self::Automatic { strategy } => Some(strategy.stable_id()),
            Self::LegacyCompatibility(_) => None,
            _ => None,
        }
    }
}
impl FromStr for ChannelAssignment {
    type Err = UnknownStableIdError;
    fn from_str(id: &str) -> Result<Self, Self::Err> {
        match id {
            "assignment.active_channel" => Ok(Self::ActiveChannel),
            "assignment.all_channels" => Ok(Self::AllChannels),
            "assignment.automatic" => Err(UnknownStableIdError {
                kind: "assignment payload",
                id: id.to_owned(),
            }),
            "compat.crosshatch.progressive_kcmy_v1" => Ok(Self::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1,
            )),
            _ => Err(UnknownStableIdError {
                kind: "channel assignment",
                id: id.to_owned(),
            }),
        }
    }
}
impl ChannelAssignment {
    pub fn automatic(strategy: AutomaticSeparationStrategy) -> Self {
        Self::Automatic { strategy }
    }
    pub fn parse(id: &str, payload: Option<&str>) -> Result<Self, UnknownStableIdError> {
        match id {
            "assignment.automatic" => Ok(Self::Automatic {
                strategy: payload
                    .ok_or_else(|| UnknownStableIdError {
                        kind: "automatic assignment payload",
                        id: id.to_owned(),
                    })?
                    .parse()?,
            }),
            "assignment.active_channel" if payload.is_none() => Ok(Self::ActiveChannel),
            "assignment.all_channels" if payload.is_none() => Ok(Self::AllChannels),
            "compat.crosshatch.progressive_kcmy_v1" if payload.is_none() => {
                Ok(Self::LegacyCompatibility(
                    LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1,
                ))
            }
            _ => Err(UnknownStableIdError {
                kind: "channel assignment",
                id: id.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkPipelineSettings {
    pub source: ArtworkSource,
    pub alpha_policy: SourceAlphaPolicy,
    pub output_model: OutputModel,
    pub assignment: ChannelAssignment,
    pub active_channel: Option<OutputChannelId>,
}
impl Default for ArtworkPipelineSettings {
    fn default() -> Self {
        Self {
            source: ArtworkSource::FullColor,
            // New documents deliberately begin at the audited legacy alpha
            // boundary until Stage 2 makes alpha choices visible.
            alpha_policy: SourceAlphaPolicy::LegacyCurrentV1,
            output_model: OutputModel::CmykPrint,
            assignment: ChannelAssignment::automatic(
                AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1,
            ),
            active_channel: Some(OutputChannelId::CmykCyan),
        }
    }
}

/// Stable, dotted-ID document representation.  Keeping this conversion here
/// prevents serde enum spellings or GTK indexes from becoming a file format.
#[derive(Serialize, Deserialize)]
struct PersistedSettings {
    source: String,
    alpha_policy: String,
    output_model: String,
    assignment: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    assignment_payload: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_channel: Option<String>,
}

impl Serialize for ArtworkPipelineSettings {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        PersistedSettings {
            source: self.source.stable_id().into(),
            alpha_policy: self.alpha_policy.stable_id().into(),
            output_model: self.output_model.stable_id().into(),
            assignment: self.assignment.stable_id().into(),
            assignment_payload: self.assignment.payload_id().map(str::to_owned),
            active_channel: self
                .active_channel
                .map(|channel| channel.stable_id().to_owned()),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ArtworkPipelineSettings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = PersistedSettings::deserialize(deserializer)?;
        let settings = Self {
            source: value.source.parse().map_err(serde::de::Error::custom)?,
            alpha_policy: value
                .alpha_policy
                .parse()
                .map_err(serde::de::Error::custom)?,
            output_model: value
                .output_model
                .parse()
                .map_err(serde::de::Error::custom)?,
            assignment: ChannelAssignment::parse(
                &value.assignment,
                value.assignment_payload.as_deref(),
            )
            .map_err(serde::de::Error::custom)?,
            active_channel: value
                .active_channel
                .as_deref()
                .map(str::parse)
                .transpose()
                .map_err(serde::de::Error::custom)?,
        };
        settings.validate().map_err(serde::de::Error::custom)?;
        Ok(settings)
    }
}
impl ArtworkPipelineSettings {
    pub fn validate(&self) -> Result<(), PipelineStateError> {
        if let Some(channel) = self.active_channel
            && !channel.belongs_to(self.output_model)
        {
            return Err(PipelineStateError::InvalidActiveChannel {
                channel,
                output: self.output_model,
            });
        }
        match self.assignment {
            ChannelAssignment::Automatic { strategy } => {
                if self.source != ArtworkSource::FullColor {
                    return Err(PipelineStateError::InvalidSourceAssignment {
                        source: self.source,
                        assignment: "automatic",
                    });
                }
                if strategy.output_model() != self.output_model {
                    return Err(PipelineStateError::IncompatibleSeparationStrategy {
                        strategy,
                        output: self.output_model,
                    });
                }
            }
            ChannelAssignment::ActiveChannel => {
                if !self.source.is_scalar() {
                    return Err(PipelineStateError::InvalidSourceAssignment {
                        source: self.source,
                        assignment: "active channel",
                    });
                }
                if self.active_channel.is_none() {
                    return Err(PipelineStateError::MissingActiveChannel);
                }
            }
            ChannelAssignment::AllChannels => {
                if !self.source.is_scalar() {
                    return Err(PipelineStateError::InvalidSourceAssignment {
                        source: self.source,
                        assignment: "all channels",
                    });
                }
            }
            ChannelAssignment::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1,
            ) => {
                if self.source
                    != ArtworkSource::LegacyBrightness(
                        LegacyBrightnessKind::EncodedRec709InvertedV1,
                    )
                {
                    return Err(PipelineStateError::UnsupportedCrosshatchCombination);
                }
                if self.alpha_policy != SourceAlphaPolicy::LegacyCurrentV1
                    || self.active_channel.is_some()
                {
                    return Err(PipelineStateError::UnsupportedCrosshatchCombination);
                }
            }
        };
        Ok(())
    }
    /// A user-requested model change; it is intentionally distinct from validation.
    pub fn transition_output_model(
        mut self,
        output_model: OutputModel,
        restored_active_channel: Option<OutputChannelId>,
    ) -> Result<Self, PipelineStateError> {
        let prior_slot = self.active_channel.map(OutputChannelId::legacy_slot);
        self.output_model = output_model;
        match self.assignment {
            ChannelAssignment::Automatic { .. } if self.source == ArtworkSource::FullColor => {
                self.assignment = ChannelAssignment::automatic(match output_model {
                    OutputModel::CmykPrint => AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1,
                    OutputModel::RgbScreen => {
                        AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1
                    }
                });
                self.active_channel = restored_active_channel
                    .filter(|channel| channel.belongs_to(output_model))
                    .or_else(|| {
                        prior_slot.and_then(|slot| {
                            OutputChannelId::from_legacy_slot(slot, output_model).ok()
                        })
                    });
            }
            ChannelAssignment::LegacyCompatibility(_) => self.active_channel = None,
            _ => {
                self.active_channel = restored_active_channel
                    .filter(|channel| channel.belongs_to(output_model))
                    .or_else(|| {
                        prior_slot.and_then(|slot| {
                            OutputChannelId::from_legacy_slot(slot, output_model).ok()
                        })
                    })
                    .or_else(|| {
                        matches!(self.assignment, ChannelAssignment::ActiveChannel)
                            .then(|| output_model.default_channel())
                    });
            }
        }
        self.validate()?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacySlotError {
    pub slot: u32,
    pub output: OutputModel,
}
impl fmt::Display for LegacySlotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "legacy scalar slot {} is invalid for {}",
            self.slot,
            self.output.stable_id()
        )
    }
}
impl Error for LegacySlotError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineStateError {
    IncompatibleSeparationStrategy {
        strategy: AutomaticSeparationStrategy,
        output: OutputModel,
    },
    InvalidActiveChannel {
        channel: OutputChannelId,
        output: OutputModel,
    },
    MissingActiveChannel,
    InvalidSourceAssignment {
        source: ArtworkSource,
        assignment: &'static str,
    },
    UnsupportedCrosshatchCombination,
}
impl fmt::Display for PipelineStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid artwork pipeline state: {self:?}")
    }
}
impl Error for PipelineStateError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyValueModeProjection {
    pub value_mode: ValueMode,
    pub output_mode: OutputMode,
    pub scalar_slot: Option<u32>,
    pub scalar_destination: Option<Ink>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyProjectionError {
    InvalidPipeline(PipelineStateError),
    UnsupportedReverseProjection,
}
impl fmt::Display for LegacyProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "legacy projection failed: {self:?}")
    }
}
impl Error for LegacyProjectionError {}
pub fn project_legacy_value_mode(
    settings: &ArtworkPipelineSettings,
) -> Result<LegacyValueModeProjection, LegacyProjectionError> {
    settings
        .validate()
        .map_err(LegacyProjectionError::InvalidPipeline)?;
    let output_mode = settings.output_model.to_legacy();
    match (settings.source, settings.alpha_policy, settings.assignment) {
        (
            ArtworkSource::FullColor,
            SourceAlphaPolicy::LegacyCurrentV1,
            ChannelAssignment::Automatic {
                strategy: AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1,
            },
        ) if settings.output_model == OutputModel::CmykPrint => Ok(LegacyValueModeProjection {
            value_mode: ValueMode::Cmyk,
            output_mode,
            scalar_slot: None,
            scalar_destination: None,
        }),
        (
            ArtworkSource::FullColor,
            SourceAlphaPolicy::LegacyCurrentV1,
            ChannelAssignment::Automatic {
                strategy: AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1,
            },
        ) if settings.output_model == OutputModel::RgbScreen => Ok(LegacyValueModeProjection {
            value_mode: ValueMode::Rgb,
            output_mode,
            scalar_slot: None,
            scalar_destination: None,
        }),
        (
            ArtworkSource::LegacyBrightness(LegacyBrightnessKind::EncodedRec709InvertedV1),
            SourceAlphaPolicy::LegacyCurrentV1,
            ChannelAssignment::ActiveChannel,
        ) => {
            let Some(channel) = settings.active_channel else {
                return Err(LegacyProjectionError::InvalidPipeline(
                    PipelineStateError::MissingActiveChannel,
                ));
            };
            Ok(LegacyValueModeProjection {
                value_mode: ValueMode::SingleChannel,
                output_mode,
                scalar_slot: Some(channel.legacy_slot()),
                scalar_destination: Some(channel.to_legacy_ink()),
            })
        }
        (
            ArtworkSource::LegacyBrightness(LegacyBrightnessKind::EncodedRec709InvertedV1),
            SourceAlphaPolicy::LegacyCurrentV1,
            ChannelAssignment::AllChannels,
        ) => Ok(LegacyValueModeProjection {
            value_mode: ValueMode::Luminance,
            output_mode,
            scalar_slot: None,
            scalar_destination: None,
        }),
        (
            ArtworkSource::LegacyBrightness(LegacyBrightnessKind::EncodedRec709InvertedV1),
            SourceAlphaPolicy::LegacyCurrentV1,
            ChannelAssignment::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1,
            ),
        ) => Ok(LegacyValueModeProjection {
            value_mode: ValueMode::CrosshatchLuminance,
            output_mode,
            scalar_slot: None,
            scalar_destination: None,
        }),
        _ => Err(LegacyProjectionError::UnsupportedReverseProjection),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stable_id_is_explicit_and_round_trips() {
        let sources = [
            (ArtworkSource::FullColor, "source.full_color"),
            (ArtworkSource::Red, "source.red"),
            (ArtworkSource::Green, "source.green"),
            (ArtworkSource::Blue, "source.blue"),
            (ArtworkSource::Value, "source.value"),
            (
                ArtworkSource::PerceptualLightness,
                "source.perceptual_lightness",
            ),
            (ArtworkSource::Alpha, "source.alpha"),
            (
                ArtworkSource::LegacyBrightness(LegacyBrightnessKind::EncodedRec709InvertedV1),
                "source.legacy_brightness.encoded_rec709_inverted_v1",
            ),
        ];
        for (value, id) in sources {
            assert_eq!(value.stable_id(), id);
            assert_eq!(id.parse::<ArtworkSource>().unwrap(), value);
        }
        assert_eq!(
            LegacyBrightnessKind::EncodedRec709InvertedV1.stable_id(),
            "source.legacy_brightness.encoded_rec709_inverted_v1"
        );
        assert_eq!(
            "source.legacy_brightness.encoded_rec709_inverted_v1"
                .parse::<LegacyBrightnessKind>()
                .unwrap(),
            LegacyBrightnessKind::EncodedRec709InvertedV1
        );
        for (value, id) in [
            (
                SourceAlphaPolicy::LegacyCurrentV1,
                "source_alpha.legacy_current_v1",
            ),
            (SourceAlphaPolicy::Preserve, "source_alpha.preserve"),
            (SourceAlphaPolicy::Ignore, "source_alpha.ignore"),
        ] {
            assert_eq!(value.stable_id(), id);
            assert_eq!(id.parse::<SourceAlphaPolicy>().unwrap(), value);
        }
        for (value, id) in [
            (OutputModel::CmykPrint, "output.cmyk_print"),
            (OutputModel::RgbScreen, "output.rgb_screen"),
        ] {
            assert_eq!(value.stable_id(), id);
            assert_eq!(id.parse::<OutputModel>().unwrap(), value);
        }
        for (value, id) in [
            (OutputChannelId::CmykCyan, "channel.cmyk.cyan"),
            (OutputChannelId::CmykMagenta, "channel.cmyk.magenta"),
            (OutputChannelId::CmykYellow, "channel.cmyk.yellow"),
            (OutputChannelId::CmykBlack, "channel.cmyk.black"),
            (OutputChannelId::RgbRed, "channel.rgb.red"),
            (OutputChannelId::RgbGreen, "channel.rgb.green"),
            (OutputChannelId::RgbBlue, "channel.rgb.blue"),
        ] {
            assert_eq!(value.stable_id(), id);
            assert_eq!(id.parse::<OutputChannelId>().unwrap(), value);
        }
        for (value, id) in [
            (
                AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1,
                "separation.cmyk.encoded_rgb_max_black_v1",
            ),
            (
                AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1,
                "separation.rgb.direct_encoded_components_v1",
            ),
        ] {
            assert_eq!(value.stable_id(), id);
            assert_eq!(id.parse::<AutomaticSeparationStrategy>().unwrap(), value);
        }
        let hatch = LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1;
        assert_eq!(hatch.stable_id(), "compat.crosshatch.progressive_kcmy_v1");
        assert_eq!(
            hatch
                .stable_id()
                .parse::<LegacyCompatibilityAssignment>()
                .unwrap(),
            hatch
        );
        for (assignment, id, payload) in [
            (
                ChannelAssignment::automatic(AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1),
                "assignment.automatic",
                Some("separation.cmyk.encoded_rgb_max_black_v1"),
            ),
            (
                ChannelAssignment::ActiveChannel,
                "assignment.active_channel",
                None,
            ),
            (
                ChannelAssignment::AllChannels,
                "assignment.all_channels",
                None,
            ),
        ] {
            assert_eq!(assignment.stable_id(), id);
            assert_eq!(assignment.payload_id(), payload);
            assert_eq!(ChannelAssignment::parse(id, payload).unwrap(), assignment);
        }
        let assignment = ChannelAssignment::LegacyCompatibility(hatch);
        assert_eq!(
            assignment.stable_id(),
            "compat.crosshatch.progressive_kcmy_v1"
        );
        assert_eq!(assignment.payload_id(), None);
        assert_eq!(
            ChannelAssignment::parse(assignment.stable_id(), None).unwrap(),
            assignment
        );
    }
    #[test]
    fn every_identifier_category_rejects_unknown_and_automatic_needs_payload() {
        assert!("unknown".parse::<ArtworkSource>().is_err());
        assert!("unknown".parse::<LegacyBrightnessKind>().is_err());
        assert!("unknown".parse::<SourceAlphaPolicy>().is_err());
        assert!("unknown".parse::<OutputModel>().is_err());
        assert!("unknown".parse::<OutputChannelId>().is_err());
        assert!("unknown".parse::<AutomaticSeparationStrategy>().is_err());
        assert!("unknown".parse::<LegacyCompatibilityAssignment>().is_err());
        assert!("unknown".parse::<ChannelAssignment>().is_err());
        assert!(
            "source.legacy_brightness.encoded_rec709_luma_darkness_v1"
                .parse::<ArtworkSource>()
                .is_err()
        );
        assert!(ChannelAssignment::parse("assignment.automatic", None).is_err());
        assert!(ChannelAssignment::parse("assignment.active_channel", Some("x")).is_err());
    }
    #[test]
    fn channel_order_membership_and_all_slots_are_explicit() {
        assert_eq!(
            OutputModel::CmykPrint.channels(),
            &[
                OutputChannelId::CmykCyan,
                OutputChannelId::CmykMagenta,
                OutputChannelId::CmykYellow,
                OutputChannelId::CmykBlack
            ]
        );
        assert_eq!(
            OutputModel::RgbScreen.channels(),
            &[
                OutputChannelId::RgbRed,
                OutputChannelId::RgbGreen,
                OutputChannelId::RgbBlue
            ]
        );
        for (slot, cmyk, rgb) in [
            (0, OutputChannelId::CmykCyan, OutputChannelId::RgbRed),
            (1, OutputChannelId::CmykMagenta, OutputChannelId::RgbGreen),
            (2, OutputChannelId::CmykYellow, OutputChannelId::RgbBlue),
        ] {
            assert_eq!(
                OutputChannelId::from_legacy_slot(slot, OutputModel::CmykPrint).unwrap(),
                cmyk
            );
            assert_eq!(
                OutputChannelId::from_legacy_slot(slot, OutputModel::RgbScreen).unwrap(),
                rgb
            );
            assert!(cmyk.belongs_to(OutputModel::CmykPrint));
            assert!(rgb.belongs_to(OutputModel::RgbScreen));
        }
        assert_eq!(
            OutputChannelId::from_legacy_slot(3, OutputModel::CmykPrint).unwrap(),
            OutputChannelId::CmykBlack
        );
        assert!(OutputChannelId::from_legacy_slot(3, OutputModel::RgbScreen).is_err());
        assert!(OutputChannelId::from_legacy_slot(4, OutputModel::RgbScreen).is_err());
        assert!(OutputChannelId::from_legacy_slot(u32::MAX, OutputModel::CmykPrint).is_err());
    }
    #[test]
    fn validation_matrix() {
        assert!(ArtworkPipelineSettings::default().validate().is_ok());
        let rgb_default = ArtworkPipelineSettings {
            output_model: OutputModel::RgbScreen,
            assignment: ChannelAssignment::automatic(
                AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1,
            ),
            active_channel: Some(OutputChannelId::RgbRed),
            ..ArtworkPipelineSettings::default()
        };
        assert!(rgb_default.validate().is_ok());
        let invalid_strategy = ArtworkPipelineSettings {
            output_model: OutputModel::RgbScreen,
            active_channel: Some(OutputChannelId::RgbRed),
            ..ArtworkPipelineSettings::default()
        };
        assert!(matches!(
            invalid_strategy.validate(),
            Err(PipelineStateError::IncompatibleSeparationStrategy { .. })
        ));
        let mut state = ArtworkPipelineSettings {
            assignment: ChannelAssignment::ActiveChannel,
            ..ArtworkPipelineSettings::default()
        };
        assert!(matches!(
            state.validate(),
            Err(PipelineStateError::InvalidSourceAssignment { .. })
        ));
        state.source = ArtworkSource::Red;
        assert!(state.validate().is_ok());
        state.active_channel = None;
        assert!(matches!(
            state.validate(),
            Err(PipelineStateError::MissingActiveChannel)
        ));
        state.active_channel = Some(OutputChannelId::RgbRed);
        assert!(matches!(
            state.validate(),
            Err(PipelineStateError::InvalidActiveChannel { .. })
        ));
        state.active_channel = Some(OutputChannelId::CmykCyan);
        assert!(state.validate().is_ok());
        state.assignment = ChannelAssignment::AllChannels;
        state.active_channel = None;
        assert!(state.validate().is_ok());
        state.source = ArtworkSource::FullColor;
        assert!(state.validate().is_err());
    }
    #[test]
    fn crosshatch_is_exclusive_compatibility() {
        let state = ArtworkPipelineSettings {
            source: ArtworkSource::LegacyBrightness(LegacyBrightnessKind::EncodedRec709InvertedV1),
            alpha_policy: SourceAlphaPolicy::LegacyCurrentV1,
            output_model: OutputModel::RgbScreen,
            assignment: ChannelAssignment::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1,
            ),
            active_channel: None,
        };
        assert!(state.validate().is_ok());
        assert!(
            ArtworkPipelineSettings {
                source: ArtworkSource::Alpha,
                ..state
            }
            .validate()
            .is_err()
        );
        assert!(
            ArtworkPipelineSettings {
                alpha_policy: SourceAlphaPolicy::Preserve,
                ..state
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn output_transition_preserves_independent_concepts() {
        let invalid = ArtworkPipelineSettings {
            source: ArtworkSource::LegacyBrightness(LegacyBrightnessKind::EncodedRec709InvertedV1),
            alpha_policy: SourceAlphaPolicy::Preserve,
            output_model: OutputModel::RgbScreen,
            assignment: ChannelAssignment::ActiveChannel,
            active_channel: Some(OutputChannelId::CmykCyan),
        };
        assert!(invalid.validate().is_err());
        assert_eq!(
            invalid
                .transition_output_model(OutputModel::CmykPrint, None)
                .unwrap()
                .active_channel,
            Some(OutputChannelId::CmykCyan)
        );
        let all = ArtworkPipelineSettings {
            source: ArtworkSource::Value,
            alpha_policy: SourceAlphaPolicy::Ignore,
            output_model: OutputModel::CmykPrint,
            assignment: ChannelAssignment::AllChannels,
            active_channel: Some(OutputChannelId::CmykYellow),
        };
        let transitioned = all
            .transition_output_model(OutputModel::RgbScreen, None)
            .unwrap();
        assert_eq!(transitioned.source, ArtworkSource::Value);
        assert_eq!(transitioned.alpha_policy, SourceAlphaPolicy::Ignore);
        assert_eq!(transitioned.assignment, ChannelAssignment::AllChannels);
        assert_eq!(transitioned.active_channel, Some(OutputChannelId::RgbBlue));
        let automatic = ArtworkPipelineSettings::default()
            .transition_output_model(OutputModel::RgbScreen, None)
            .unwrap();
        assert_eq!(automatic.source, ArtworkSource::FullColor);
        assert_eq!(automatic.alpha_policy, SourceAlphaPolicy::LegacyCurrentV1);
        assert_eq!(
            automatic.assignment.payload_id(),
            Some("separation.rgb.direct_encoded_components_v1")
        );
        assert_eq!(automatic.active_channel, Some(OutputChannelId::RgbRed));
    }
    #[test]
    fn reverse_projection_covers_current_valid_compatibility_states() {
        let automatic = ArtworkPipelineSettings::default();
        assert_eq!(
            project_legacy_value_mode(&automatic).unwrap().value_mode,
            ValueMode::Cmyk
        );

        let active = ArtworkPipelineSettings {
            source: ArtworkSource::LegacyBrightness(LegacyBrightnessKind::EncodedRec709InvertedV1),
            alpha_policy: SourceAlphaPolicy::LegacyCurrentV1,
            output_model: OutputModel::RgbScreen,
            assignment: ChannelAssignment::ActiveChannel,
            active_channel: Some(OutputChannelId::RgbGreen),
        };
        let projection = project_legacy_value_mode(&active).unwrap();
        assert_eq!(projection.value_mode, ValueMode::SingleChannel);
        assert_eq!(projection.scalar_destination, Some(Ink::Green));

        let crosshatch = ArtworkPipelineSettings {
            source: ArtworkSource::LegacyBrightness(LegacyBrightnessKind::EncodedRec709InvertedV1),
            alpha_policy: SourceAlphaPolicy::LegacyCurrentV1,
            output_model: OutputModel::CmykPrint,
            assignment: ChannelAssignment::LegacyCompatibility(
                LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1,
            ),
            active_channel: None,
        };
        assert_eq!(
            project_legacy_value_mode(&crosshatch).unwrap().value_mode,
            ValueMode::CrosshatchLuminance
        );

        let unsupported = ArtworkPipelineSettings {
            source: ArtworkSource::Value,
            alpha_policy: SourceAlphaPolicy::Preserve,
            output_model: OutputModel::RgbScreen,
            assignment: ChannelAssignment::AllChannels,
            active_channel: None,
        };
        assert_eq!(
            project_legacy_value_mode(&unsupported),
            Err(LegacyProjectionError::UnsupportedReverseProjection)
        );
    }
}
