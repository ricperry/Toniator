//! Authored artistic detail and final connection-selection intent, independent of derived geometry.

use crate::{CanvasSpec, DensityMetric2D, ValidationError};
use serde::{Deserialize, Serialize};

/// Positive relative tone-sample spacing for guide, parametric and motif outputs only.
/// This stores intent, never source knots, geometry tolerances or renderer resolution.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToneSampleSpacing {
    pub multiplier: f64,
}

impl PartialEq for ToneSampleSpacing {
    /// Compares authored bits so invalid constructible NaNs remain reflexive under Eq.
    fn eq(&self, other: &Self) -> bool {
        self.multiplier.to_bits() == other.multiplier.to_bits()
    }
}
impl Eq for ToneSampleSpacing {}
impl Default for ToneSampleSpacing {
    /// Supplies the sole current relative cadence at one Feature-size reference interval.
    fn default() -> Self {
        Self { multiplier: 1.0 }
    }
}
impl ToneSampleSpacing {
    /// Validates finite positive artistic intent without imposing a creative count ceiling.
    /// # Errors
    /// Returns a stable diagnostic for zero, negative, NaN or infinite multipliers.
    pub fn validate(self) -> Result<(), ValidationError> {
        if !self.multiplier.is_finite() || self.multiplier <= 0.0 {
            return Err(ValidationError::new(
                "pattern.tone_sample_spacing",
                "tone sample spacing must be finite and positive",
            ));
        }
        Ok(())
    }

    /// Resolves document distance proportionally to Feature size without a source-pixel floor.
    /// Canvas and effective Density remain domain authority; derived path chords are not inputs.
    /// # Errors
    /// Rejects invalid authored spacing, canvas, Density or an unrepresentable resolved distance.
    pub fn resolve(
        self,
        canvas: &CanvasSpec,
        effective_density: &DensityMetric2D,
    ) -> Result<f64, ValidationError> {
        self.validate()?;
        effective_density.resolve(canvas)?;
        let reference_density = DensityMetric2D::default_for_canvas(canvas)?.density;
        let distance = self.multiplier
            * (canvas.width.max(canvas.height) / 100.0)
            * (reference_density / effective_density.density);
        if !distance.is_finite() || distance <= 0.0 {
            return Err(ValidationError::new(
                "pattern.tone_sample_spacing.resolved",
                "resolved tone spacing must remain finite and positive",
            ));
        }
        Ok(distance)
    }
}

/// Final graph-cycle policy; screen crossings do not themselves form graph cycles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionCyclePolicy {
    #[default]
    Allow,
    Forbid,
}

/// Output-owned final selection independent of the program's candidate adjacency.
/// Degree/cycle bounds include automatic rescue joins; component size counts distinct points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionSelectionIntent {
    pub maximum_final_degree: Option<u32>,
    pub cycles: ConnectionCyclePolicy,
    pub minimum_component_points: u32,
}
impl Default for ConnectionSelectionIntent {
    /// Keeps unrestricted final degree/cycles and retains singleton components by default.
    fn default() -> Self {
        Self {
            maximum_final_degree: None,
            cycles: ConnectionCyclePolicy::Allow,
            minimum_component_points: 1,
        }
    }
}
impl ConnectionSelectionIntent {
    /// Validates authored final-graph limits without constructing or filtering a graph.
    /// # Errors
    /// Rejects a zero optional final degree or zero minimum component size.
    pub fn validate(self) -> Result<(), ValidationError> {
        if self.maximum_final_degree == Some(0) {
            return Err(ValidationError::new(
                "connection.selection.maximum_final_degree",
                "maximum final degree must be positive when present",
            ));
        }
        if self.minimum_component_points == 0 {
            return Err(ValidationError::new(
                "connection.selection.minimum_component_points",
                "minimum component points must be at least one",
            ));
        }
        Ok(())
    }
}
