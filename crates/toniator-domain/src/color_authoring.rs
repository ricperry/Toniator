//! Solid-paint authoring uses canonical linear RGBA and atomic existing history boundaries.

use super::*;
use temporal::{
    linear_to_srgb, solid_channel_color, solid_channel_color_mut, srgb_to_linear,
    validate_color_value,
};

/// Explicitly replaces one channel's color animation; `None` resets RGB and alpha animation.
///
/// A hue replacement preserves independent alpha intent. A linear endpoint takes ownership
/// of all four components; either mode replaces competing component RGB bindings.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorAnimationEdit {
    pub channel_id: ChannelId,
    pub mode: Option<ColorEndMode>,
    pub easing: Easing,
}

impl ColorValue {
    /// Converts an explicit encoded-sRGB picker assignment to canonical straight linear RGBA.
    /// Alpha is unchanged and hidden RGB remains meaningful at zero alpha.
    ///
    /// # Errors
    /// Rejects nonfinite components and values outside zero through one.
    pub fn from_srgb_rgba(rgba: [f64; 4]) -> Result<Self, ValidationError> {
        let value = Self {
            red: rgba[0],
            green: rgba[1],
            blue: rgba[2],
            alpha: rgba[3],
        };
        validate_color_value(&value)?;
        Ok(Self {
            red: srgb_to_linear(value.red),
            green: srgb_to_linear(value.green),
            blue: srgb_to_linear(value.blue),
            alpha: value.alpha,
        })
    }

    /// Parses `#RRGGBB` or `#RRGGBBAA` encoded sRGB into straight canonical linear RGBA.
    ///
    /// Six digits imply alpha one. Whitespace around the value is ignored; alpha is never
    /// gamma-transformed or multiplied into RGB, including fully transparent colors.
    ///
    /// # Errors
    /// Rejects missing `#`, invalid hexadecimal digits and every other component width.
    pub fn from_srgb_hex(text: &str) -> Result<Self, ValidationError> {
        let text = text.trim();
        let digits = text.strip_prefix('#').ok_or_else(invalid_hex)?;
        if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid_hex());
        }
        let component = |offset| {
            u8::from_str_radix(&digits[offset..offset + 2], 16)
                .map(|value| f64::from(value) / 255.0)
                .map_err(|_| invalid_hex())
        };
        Ok(Self {
            red: srgb_to_linear(component(0)?),
            green: srgb_to_linear(component(2)?),
            blue: srgb_to_linear(component(4)?),
            alpha: if digits.len() == 8 {
                component(6)?
            } else {
                1.0
            },
        })
    }

    /// Projects validated linear paint to encoded sRGB plus unchanged straight alpha for display.
    ///
    /// # Errors
    /// Rejects nonfinite or out-of-range canonical components instead of clamping authored state.
    pub fn srgb_rgba(&self) -> Result<[f64; 4], ValidationError> {
        validate_color_value(self)?;
        Ok([
            linear_to_srgb(self.red),
            linear_to_srgb(self.green),
            linear_to_srgb(self.blue),
            self.alpha,
        ])
    }

    /// Formats an eight-digit sRGB display value without changing stored component precision.
    ///
    /// Frontends must not parse this quantized display back into authority unless the user edits
    /// it. The alpha byte represents the same straight alpha as the separate numeric control.
    ///
    /// # Errors
    /// Rejects nonfinite or out-of-range canonical components.
    pub fn to_srgb_hex(&self) -> Result<String, ValidationError> {
        let rgba = self.srgb_rgba()?.map(|value| (value * 255.0).round() as u8);
        Ok(format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            rgba[0], rgba[1], rgba[2], rgba[3]
        ))
    }
}

/// Returns the shared HEX input diagnostic without exposing an unrelated parser representation.
fn invalid_hex() -> ValidationError {
    ValidationError::new("color.hex", "enter #RRGGBB or #RRGGBBAA")
}

impl Document {
    /// Reads ordinary Start solid paint; sampled-source paint has no editable solid endpoint.
    ///
    /// # Errors
    /// Rejects a missing channel or sampled-source paint.
    pub fn solid_paint(&self, channel: ChannelId) -> Result<&ColorValue, ValidationError> {
        solid_channel_color(self, channel)
    }

    /// Builds one validated Start-paint batch for the existing atomic configuration command.
    ///
    /// Each channel keeps its identity, source mapping, structural settings and End animation.
    /// The caller submits this configuration with this exact document/revision as its stale base.
    ///
    /// # Errors
    /// Rejects an empty/duplicate batch, missing or sampled channels, invalid colors, or an
    /// invalid complete Start/End state before returning any candidate configuration.
    pub fn edit_start_colors_configuration(
        &self,
        edits: &[(ChannelId, ColorValue)],
    ) -> Result<DocumentConfiguration, ValidationError> {
        validate_color_channels(edits.iter().map(|(channel, _)| *channel))?;
        let mut candidate = self.clone();
        for (channel, color) in edits {
            validate_color_value(color)?;
            *solid_channel_color_mut(&mut candidate, *channel)? = color.clone();
        }
        candidate.validate()?;
        Ok(DocumentConfiguration::capture(&candidate))
    }

    /// Builds one explicit color-mode batch that replaces competing RGB bindings atomically.
    ///
    /// Linear endpoints own RGBA. Hue preserves scalar alpha, or converts a previous grouped
    /// alpha endpoint into that same scalar authority with its previous easing. Reset removes
    /// all paint-color End bindings, leaving channel opacity and every other setting untouched.
    ///
    /// # Errors
    /// Rejects an empty/duplicate batch, missing or sampled channels, invalid colors/angles, or
    /// an invalid complete End state. This operation never changes ordinary Start paint.
    pub fn edit_color_animation_command(
        &self,
        edits: &[ColorAnimationEdit],
    ) -> Result<TemporalCommand, ValidationError> {
        validate_color_channels(edits.iter().map(|edit| edit.channel_id))?;
        let mut overrides = self.temporal_end_overrides().to_vec();
        for edit in edits {
            let _ = self.solid_paint(edit.channel_id)?;
            let hue = matches!(edit.mode, Some(ColorEndMode::HueRotation { .. }));
            let grouped_alpha = overrides.iter().find_map(|entry| match entry {
                TemporalEndOverride::Color(ColorEndOverride {
                    channel_id,
                    mode: ColorEndMode::LinearColor { end },
                    easing,
                }) if *channel_id == edit.channel_id => Some((end.alpha, *easing)),
                _ => None,
            });
            overrides.retain(|entry| match entry {
                TemporalEndOverride::Color(value) => value.channel_id != edit.channel_id,
                TemporalEndOverride::Scalar(value) => {
                    value.target != PropertyTarget::Channel(edit.channel_id)
                        || !matches!(
                            value.field,
                            PropertyFieldId::ColorRed
                                | PropertyFieldId::ColorGreen
                                | PropertyFieldId::ColorBlue
                        ) && (hue || value.field != PropertyFieldId::ColorAlpha)
                }
            });
            if hue && let Some((end, easing)) = grouped_alpha {
                overrides.push(TemporalEndOverride::Scalar(ScalarEndOverride {
                    target: PropertyTarget::Channel(edit.channel_id),
                    field: PropertyFieldId::ColorAlpha,
                    end,
                    easing,
                }));
            }
            if let Some(mode) = &edit.mode {
                overrides.push(TemporalEndOverride::Color(ColorEndOverride {
                    channel_id: edit.channel_id,
                    mode: mode.clone(),
                    easing: edit.easing,
                }));
            }
        }
        // Use the same complete-document validation as ordinary temporal commands, while
        // retaining the original document as the stale base of the returned command.
        self.clone()
            .with_temporal_authority(self.project_timing().clone(), overrides.clone())?;
        Ok(self.replace_temporal_authority_command(self.project_timing().clone(), overrides))
    }

    /// Edits one solid-paint component across channels without introducing competing writers.
    ///
    /// Linear-color mode edits its existing grouped endpoint and keeps its easing. Hue rejects
    /// RGB component edits and permits independent scalar alpha. New hue alpha inherits the
    /// hue easing, while existing alpha retains its authored easing. Ungrouped values use the
    /// ordinary effective-End builder. All changes publish as one original-root command.
    ///
    /// # Errors
    /// Rejects empty/duplicate/missing/sampled channels, hue-owned RGB, invalid components or
    /// complete End states; no document or history is mutated on failure.
    pub fn edit_paint_component_end_command(
        &self,
        channels: &[ChannelId],
        component: ColorComponent,
        value: f64,
    ) -> Result<TemporalCommand, ValidationError> {
        validate_color_channels(channels.iter().copied())?;
        validate_unit_component(value, "color.component")?;
        let field = match component {
            ColorComponent::Red => PropertyFieldId::ColorRed,
            ColorComponent::Green => PropertyFieldId::ColorGreen,
            ColorComponent::Blue => PropertyFieldId::ColorBlue,
            ColorComponent::Alpha => PropertyFieldId::ColorAlpha,
        };
        let mut candidate = self.clone();
        let mut scalar_edits = Vec::new();
        for channel in channels {
            let _ = self.solid_paint(*channel)?;
            let group = candidate
                .temporal_end_overrides
                .iter_mut()
                .find_map(|entry| match entry {
                    TemporalEndOverride::Color(group) if group.channel_id == *channel => {
                        Some(group)
                    }
                    _ => None,
                });
            let group_easing = group.as_ref().map(|group| group.easing);
            if let Some(group) = group {
                match &mut group.mode {
                    ColorEndMode::LinearColor { end } => {
                        match component {
                            ColorComponent::Red => end.red = value,
                            ColorComponent::Green => end.green = value,
                            ColorComponent::Blue => end.blue = value,
                            ColorComponent::Alpha => end.alpha = value,
                        }
                        continue;
                    }
                    ColorEndMode::HueRotation { .. } if component != ColorComponent::Alpha => {
                        return Err(ValidationError::new(
                            "color.component",
                            "hue rotation controls RGB; choose Colors to edit components",
                        ));
                    }
                    ColorEndMode::HueRotation { .. } => {}
                }
            }
            let easing = self
                .temporal_end_overrides()
                .iter()
                .find_map(|entry| match entry {
                    TemporalEndOverride::Scalar(end)
                        if end.target == PropertyTarget::Channel(*channel)
                            && end.field == field =>
                    {
                        Some(end.easing)
                    }
                    _ => None,
                })
                .unwrap_or(group_easing.unwrap_or(Easing::Linear));
            scalar_edits.push(TemporalEndpointEdit {
                target: PropertyTarget::Channel(*channel),
                field,
                effective_end: value,
                easing,
            });
        }
        if !scalar_edits.is_empty() {
            let command = candidate.edit_effective_end_command(&scalar_edits)?;
            candidate.temporal_end_overrides = command.replacement().end_overrides.clone();
        }
        candidate.validate()?;
        Ok(self.replace_temporal_authority_command(
            self.project_timing().clone(),
            candidate.temporal_end_overrides,
        ))
    }
}

/// Requires a nonempty batch addressing each real channel at most once before candidate mutation.
///
/// # Errors
/// Rejects an empty sequence or repeated channel ID.
fn validate_color_channels(
    channels: impl Iterator<Item = ChannelId>,
) -> Result<(), ValidationError> {
    let mut seen = HashSet::new();
    for channel in channels {
        if !seen.insert(channel) {
            return Err(ValidationError::new(
                "color.batch",
                "each channel may be edited only once",
            ));
        }
    }
    if seen.is_empty() {
        return Err(ValidationError::new(
            "color.batch",
            "at least one channel is required",
        ));
    }
    Ok(())
}
