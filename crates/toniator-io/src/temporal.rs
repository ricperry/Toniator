//! Current-schema persistence of project timing and reusable End-only overrides.

use super::*;
use toniator_domain::{
    ColorEndMode, ColorEndOverride, Easing, FrameRange, FrameRate, ProjectTiming, PropertyFieldId,
    PropertyTarget, RationalTime, ScalarEndOverride, TemporalEndOverride, TimeRange,
};

/// Exact project timing; reusable Presets deliberately omit this object.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProjectTimingDto {
    fps_numerator: u32,
    fps_denominator: u32,
    start_frame: u64,
    end_frame_exclusive: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_time_range: Option<TimeRangeDto>,
}

/// Exact source interval in seconds, with no floating-point timestamp round trip.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimeRangeDto {
    start_numerator: u64,
    start_denominator: u64,
    end_numerator: u64,
    end_denominator: u64,
}

impl ProjectTimingDto {
    /// Projects validated project timing without copying it into reusable settings.
    pub(super) fn from_domain(value: &ProjectTiming) -> Self {
        Self {
            fps_numerator: value.frame_rate().numerator(),
            fps_denominator: value.frame_rate().denominator(),
            start_frame: value.frame_range().start(),
            end_frame_exclusive: value.frame_range().end_exclusive(),
            source_time_range: value.source_time_range().map(|range| TimeRangeDto {
                start_numerator: range.start().numerator(),
                start_denominator: range.start().denominator(),
                end_numerator: range.end().numerator(),
                end_denominator: range.end().denominator(),
            }),
        }
    }

    /// Reconstructs exact timing through domain constructors.
    ///
    /// # Errors
    /// Rejects zero rates/denominators, empty intervals, and invalid frame ranges.
    pub(super) fn into_domain(self) -> Result<ProjectTiming, ValidationError> {
        let mut timing = ProjectTiming::new(
            FrameRate::new(self.fps_numerator, self.fps_denominator)?,
            FrameRange::new(self.start_frame, self.end_frame_exclusive)?,
        );
        if let Some(range) = self.source_time_range {
            timing = timing.with_source_time_range(TimeRange::new(
                RationalTime::new(range.start_numerator, range.start_denominator)?,
                RationalTime::new(range.end_numerator, range.end_denominator)?,
            )?);
        }
        Ok(timing)
    }
}

/// Closed serialization vocabulary for temporal easing.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum EasingDto {
    Hold,
    Linear,
    QuadraticIn,
    QuadraticOut,
    SmoothStep,
    SmoothInOut,
}

impl From<Easing> for EasingDto {
    /// Encodes the domain's selected easing without frontend-specific names.
    fn from(value: Easing) -> Self {
        match value {
            Easing::Hold => Self::Hold,
            Easing::Linear => Self::Linear,
            Easing::QuadraticIn => Self::QuadraticIn,
            Easing::QuadraticOut => Self::QuadraticOut,
            Easing::SmoothStep => Self::SmoothStep,
            Easing::SmoothInOut => Self::SmoothInOut,
        }
    }
}

impl From<EasingDto> for Easing {
    /// Reconstructs the single domain easing authority from a known name.
    fn from(value: EasingDto) -> Self {
        match value {
            EasingDto::Hold => Self::Hold,
            EasingDto::Linear => Self::Linear,
            EasingDto::QuadraticIn => Self::QuadraticIn,
            EasingDto::QuadraticOut => Self::QuadraticOut,
            EasingDto::SmoothStep => Self::SmoothStep,
            EasingDto::SmoothInOut => Self::SmoothInOut,
        }
    }
}

/// Stable temporal target identities; definitions cannot acquire End overrides.
#[derive(Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum TemporalTargetDto {
    Document,
    Channel { channel_id: u64 },
    ChannelOutput { channel_id: u64, output_id: u64 },
}

impl TemporalTargetDto {
    /// Encodes only domain-authorized temporal target scopes.
    ///
    /// # Errors
    /// Rejects definition/mechanism targets instead of silently converting authority.
    fn from_domain(value: PropertyTarget) -> Result<Self, SaveError> {
        match value {
            PropertyTarget::Document => Ok(Self::Document),
            PropertyTarget::Channel(channel) => Ok(Self::Channel {
                channel_id: channel.0,
            }),
            PropertyTarget::ChannelOutput(channel, output) => Ok(Self::ChannelOutput {
                channel_id: channel.0,
                output_id: output.0,
            }),
            _ => Err(SaveError::DomainValidation {
                context: "temporal target must be document, channel, or channel output".into(),
            }),
        }
    }

    /// Reconstructs target identity; complete domain validation verifies references.
    fn into_domain(self) -> PropertyTarget {
        match self {
            Self::Document => PropertyTarget::Document,
            Self::Channel { channel_id } => PropertyTarget::Channel(ChannelId(channel_id)),
            Self::ChannelOutput {
                channel_id,
                output_id,
            } => PropertyTarget::ChannelOutput(
                ChannelId(channel_id),
                PatternOutputLayerId(output_id),
            ),
        }
    }
}

/// Stable field names are a serialization projection, not a second capability list.
const FIELD_NAMES: &[(PropertyFieldId, &str)] = &[
    (PropertyFieldId::Density, "density"),
    (PropertyFieldId::DensityAspect, "density_aspect"),
    (PropertyFieldId::RotationDegrees, "rotation_degrees"),
    (PropertyFieldId::TranslationX, "translation_x"),
    (PropertyFieldId::TranslationY, "translation_y"),
    (PropertyFieldId::MarkMinimumFill, "mark_minimum_fill"),
    (PropertyFieldId::MarkMaximumFill, "mark_maximum_fill"),
    (
        PropertyFieldId::ConnectedMinimumThickness,
        "connected_minimum_thickness",
    ),
    (
        PropertyFieldId::ConnectedMaximumThickness,
        "connected_maximum_thickness",
    ),
    (PropertyFieldId::CurveResponseBias, "curve_response_bias"),
    (
        PropertyFieldId::ShapeRotationDegrees,
        "shape_rotation_degrees",
    ),
    (PropertyFieldId::ModeledMappingGain, "modeled_mapping_gain"),
    (PropertyFieldId::ModeledMappingBias, "modeled_mapping_bias"),
    (
        PropertyFieldId::ModeledMappingBlackPoint,
        "modeled_mapping_black_point",
    ),
    (
        PropertyFieldId::ModeledMappingWhitePoint,
        "modeled_mapping_white_point",
    ),
    (
        PropertyFieldId::ModeledMappingGamma,
        "modeled_mapping_gamma",
    ),
    (
        PropertyFieldId::ModeledMappingContrast,
        "modeled_mapping_contrast",
    ),
    (
        PropertyFieldId::ModeledMappingCutoff,
        "modeled_mapping_cutoff",
    ),
    (
        PropertyFieldId::ArtworkWeightMappingGain,
        "artwork_weight_mapping_gain",
    ),
    (
        PropertyFieldId::ArtworkWeightMappingBias,
        "artwork_weight_mapping_bias",
    ),
    (
        PropertyFieldId::ArtworkWeightMappingBlackPoint,
        "artwork_weight_mapping_black_point",
    ),
    (
        PropertyFieldId::ArtworkWeightMappingWhitePoint,
        "artwork_weight_mapping_white_point",
    ),
    (
        PropertyFieldId::ArtworkWeightMappingGamma,
        "artwork_weight_mapping_gamma",
    ),
    (
        PropertyFieldId::ArtworkWeightMappingContrast,
        "artwork_weight_mapping_contrast",
    ),
    (
        PropertyFieldId::ArtworkWeightMappingCutoff,
        "artwork_weight_mapping_cutoff",
    ),
    (
        PropertyFieldId::ArtworkWeightStrength,
        "artwork_weight_strength",
    ),
    (PropertyFieldId::ColorRed, "color_red"),
    (PropertyFieldId::ColorGreen, "color_green"),
    (PropertyFieldId::ColorBlue, "color_blue"),
    (PropertyFieldId::ColorAlpha, "color_alpha"),
    (PropertyFieldId::Opacity, "opacity"),
    (PropertyFieldId::RegionMinimumFill, "region_minimum_fill"),
    (PropertyFieldId::RegionMaximumFill, "region_maximum_fill"),
];

/// End-only configuration extension; ordinary document fields supply every Start value.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum EndOverrideDto {
    Scalar {
        target: TemporalTargetDto,
        field: String,
        end: f64,
        easing: EasingDto,
    },
    Color {
        channel_id: u64,
        end: ColorDto,
        easing: EasingDto,
    },
    HueRotation {
        channel_id: u64,
        end_degrees: f64,
        easing: EasingDto,
    },
}

impl EndOverrideDto {
    /// Encodes reusable End intent without sampled intermediate frames or Start copies.
    ///
    /// # Errors
    /// Rejects unsupported target/field identities rather than losing a track on save.
    pub(super) fn from_domain(value: &TemporalEndOverride) -> Result<Self, SaveError> {
        match value {
            TemporalEndOverride::Scalar(value) => {
                let field = FIELD_NAMES
                    .iter()
                    .find(|(field, _)| *field == value.field)
                    .ok_or_else(|| SaveError::DomainValidation {
                        context: "unsupported temporal field at persistence boundary".into(),
                    })?
                    .1;
                Ok(Self::Scalar {
                    target: TemporalTargetDto::from_domain(value.target)?,
                    field: field.into(),
                    end: value.end,
                    easing: value.easing.into(),
                })
            }
            TemporalEndOverride::Color(value) => match &value.mode {
                ColorEndMode::LinearColor { end } => Ok(Self::Color {
                    channel_id: value.channel_id.0,
                    end: ColorDto::from_domain(end),
                    easing: value.easing.into(),
                }),
                ColorEndMode::HueRotation { end_degrees } => Ok(Self::HueRotation {
                    channel_id: value.channel_id.0,
                    end_degrees: *end_degrees,
                    easing: value.easing.into(),
                }),
            },
        }
    }

    /// Reconstructs End intent for full descriptor/reference validation by the document.
    ///
    /// # Errors
    /// Rejects unknown field names; invalid bounds and conflicting tracks fail domain validation.
    pub(super) fn into_domain(self) -> Result<TemporalEndOverride, ValidationError> {
        Ok(match self {
            Self::Scalar {
                target,
                field,
                end,
                easing,
            } => {
                let field = FIELD_NAMES
                    .iter()
                    .find(|(_, name)| *name == field)
                    .ok_or_else(|| {
                        ValidationError::new("temporal.field", "unknown persisted temporal field")
                    })?
                    .0;
                TemporalEndOverride::Scalar(ScalarEndOverride {
                    target: target.into_domain(),
                    field,
                    end,
                    easing: easing.into(),
                })
            }
            Self::Color {
                channel_id,
                end,
                easing,
            } => TemporalEndOverride::Color(ColorEndOverride {
                channel_id: ChannelId(channel_id),
                mode: ColorEndMode::LinearColor {
                    end: end.into_domain(),
                },
                easing: easing.into(),
            }),
            Self::HueRotation {
                channel_id,
                end_degrees,
                easing,
            } => TemporalEndOverride::Color(ColorEndOverride {
                channel_id: ChannelId(channel_id),
                mode: ColorEndMode::HueRotation { end_degrees },
                easing: easing.into(),
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proves serialized names cover the executable domain allowlist exactly once.
    #[test]
    fn stage22_field_names_match_domain_authority() {
        let fields: HashSet<_> = FIELD_NAMES.iter().map(|(field, _)| *field).collect();
        let names: HashSet<_> = FIELD_NAMES.iter().map(|(_, name)| *name).collect();
        assert_eq!(fields.len(), FIELD_NAMES.len());
        assert_eq!(names.len(), FIELD_NAMES.len());
        assert_eq!(
            fields,
            toniator_domain::ANIMATABLE_SCALAR_FIELD_IDS
                .iter()
                .copied()
                .collect()
        );
    }
}
