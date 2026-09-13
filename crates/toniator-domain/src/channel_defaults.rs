//! Canonical channel-property defaults, independent of frontend presentation and history.

use super::*;

impl Document {
    /// Returns a canonical default for a source, paint, opacity or region interpretation field.
    /// Modeled channels use their fixed role's palette and matching source component; legacy
    /// channels use black paint and luminance. Pattern geometry, layout and authored fill bounds
    /// have no default through this API. The caller chooses scope and applies normal commands.
    /// Returns `None` for a missing channel, an unsupported field or sampled paint components.
    pub fn channel_property_default(
        &self,
        channel_id: ChannelId,
        field: PropertyFieldId,
    ) -> Option<PropertyCurrentValueKind> {
        let (mapping, weighting, paint) = if let Some(channel) = self.modeled_channel(channel_id) {
            let defaults = ModeledChannelState::canonical(
                channel.role,
                &ChannelTopologyTemplate {
                    pattern_instance: channel.pattern_instance.clone(),
                },
            );
            (defaults.mapping, defaults.weighting, defaults.paint)
        } else {
            self.channel(channel_id)?;
            (
                SourceMapping::canonical(SourceMappingComponent::Luminance),
                SourceWeighting::canonical(SourceMappingComponent::Luminance),
                ChannelPaint::Solid(ColorValue::black()),
            )
        };
        let mapping = match field {
            PropertyFieldId::ArtworkWeightMappingComponent
            | PropertyFieldId::ArtworkWeightMappingPlacement
            | PropertyFieldId::ArtworkWeightMappingInverted
            | PropertyFieldId::ArtworkWeightMappingGain
            | PropertyFieldId::ArtworkWeightMappingBias
            | PropertyFieldId::ArtworkWeightMappingBlackPoint
            | PropertyFieldId::ArtworkWeightMappingWhitePoint
            | PropertyFieldId::ArtworkWeightMappingGamma
            | PropertyFieldId::ArtworkWeightMappingContrast
            | PropertyFieldId::ArtworkWeightMappingCutoff => weighting.mapping,
            _ => mapping,
        };
        use PropertyCurrentValueKind::{Boolean, EnumChoice, FiniteF64};
        let value =
            match field {
                PropertyFieldId::LegacyMappingComponent
                | PropertyFieldId::ModeledMappingComponent => EnumChoice(
                    PropertyEnumChoice::SourceMappingComponent(mapping.component),
                ),
                PropertyFieldId::ArtworkWeightMappingComponent => EnumChoice(
                    PropertyEnumChoice::SourceMappingComponent(weighting.mapping.component),
                ),
                PropertyFieldId::LegacyMappingPlacement
                | PropertyFieldId::ModeledMappingPlacement
                | PropertyFieldId::ArtworkWeightMappingPlacement => {
                    EnumChoice(PropertyEnumChoice::SourcePlacement(mapping.placement))
                }
                PropertyFieldId::ModeledMappingInverted
                | PropertyFieldId::ArtworkWeightMappingInverted => Boolean(mapping.inverted),
                PropertyFieldId::ModeledMappingGain | PropertyFieldId::ArtworkWeightMappingGain => {
                    FiniteF64(mapping.gain)
                }
                PropertyFieldId::ModeledMappingBias | PropertyFieldId::ArtworkWeightMappingBias => {
                    FiniteF64(mapping.bias)
                }
                PropertyFieldId::ModeledMappingBlackPoint
                | PropertyFieldId::ArtworkWeightMappingBlackPoint => {
                    FiniteF64(mapping.tone.black_point)
                }
                PropertyFieldId::ModeledMappingWhitePoint
                | PropertyFieldId::ArtworkWeightMappingWhitePoint => {
                    FiniteF64(mapping.tone.white_point)
                }
                PropertyFieldId::ModeledMappingGamma
                | PropertyFieldId::ArtworkWeightMappingGamma => FiniteF64(mapping.tone.gamma),
                PropertyFieldId::ModeledMappingContrast
                | PropertyFieldId::ArtworkWeightMappingContrast => FiniteF64(mapping.tone.contrast),
                PropertyFieldId::ModeledMappingCutoff
                | PropertyFieldId::ArtworkWeightMappingCutoff => FiniteF64(mapping.tone.cutoff),
                PropertyFieldId::ArtworkWeightStrength => FiniteF64(weighting.strength),
                PropertyFieldId::ArtworkWeightResponse => EnumChoice(
                    PropertyEnumChoice::ArtworkWeightResponse(weighting.response),
                ),
                PropertyFieldId::Opacity => FiniteF64(1.0),
                PropertyFieldId::Paint => EnumChoice(PropertyEnumChoice::Paint(match paint {
                    ChannelPaint::Solid(_) => PaintKind::Solid,
                    ChannelPaint::SampledSource => PaintKind::SampledSource,
                })),
                PropertyFieldId::ColorRed
                | PropertyFieldId::ColorGreen
                | PropertyFieldId::ColorBlue
                | PropertyFieldId::ColorAlpha => {
                    let ChannelPaint::Solid(color) = paint else {
                        return None;
                    };
                    FiniteF64(match field {
                        PropertyFieldId::ColorRed => color.red,
                        PropertyFieldId::ColorGreen => color.green,
                        PropertyFieldId::ColorBlue => color.blue,
                        _ => color.alpha,
                    })
                }
                PropertyFieldId::RegionResizeAlgorithm => {
                    EnumChoice(PropertyEnumChoice::RegionResizeAlgorithm(
                        RegionGeometryResponse::default().algorithm,
                    ))
                }
                PropertyFieldId::RegionSampling => EnumChoice(PropertyEnumChoice::RegionSampling(
                    RegionGeometryResponse::default().sampling,
                )),
                _ => return None,
            };
        Some(value)
    }
}
