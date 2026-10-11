//! Transient, geometry-derived response curves for canonical output realization.

use super::PatternPipelineError;

/// One measured opaque-union coverage at a normalized geometry response.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverageKnot {
    /// Normalized geometry response in the inclusive unit interval.
    pub response: f64,
    /// Measured occupied fraction in the inclusive unit interval.
    pub coverage: f64,
}

/// A monotone measured response curve with exact normalized geometry endpoints.
#[derive(Clone, Debug, PartialEq)]
pub struct CoverageCurve {
    knots: Vec<CoverageKnot>,
}

impl CoverageCurve {
    /// Validates an ordered, nondecreasing opaque-union curve without modifying measurements.
    ///
    /// # Errors
    ///
    /// Rejects nonfinite or out-of-range values, unordered responses, decreasing or flat total
    /// coverage, and missing exact response endpoints.
    pub fn new(knots: Vec<CoverageKnot>) -> Result<Self, PatternPipelineError> {
        if knots.len() < 2
            || knots.first().is_none_or(|knot| knot.response != 0.0)
            || knots.last().is_none_or(|knot| knot.response != 1.0)
        {
            return Err(PatternPipelineError::new(
                "coverage.response.knots",
                "coverage response requires exact zero and one geometry endpoints",
            ));
        }
        for (index, knot) in knots.iter().enumerate() {
            if !knot.response.is_finite()
                || !knot.coverage.is_finite()
                || !(0.0..=1.0).contains(&knot.response)
                || !(0.0..=1.0).contains(&knot.coverage)
                || (index > 0
                    && (knot.response <= knots[index - 1].response
                        || knot.coverage < knots[index - 1].coverage))
            {
                return Err(PatternPipelineError::new(
                    "coverage.response.knots",
                    "coverage measurements must be finite and monotone",
                ));
            }
        }
        if knots[0].coverage >= knots[knots.len() - 1].coverage {
            return Err(PatternPipelineError::new(
                "coverage.response.range",
                "coverage response has no attainable occupancy range",
            ));
        }
        Ok(Self { knots })
    }

    /// Returns the measured knots without exposing mutation of the validated curve.
    pub fn knots(&self) -> &[CoverageKnot] {
        &self.knots
    }

    /// Inverts endpoint-normalized coverage with the earliest attainable response at plateaus.
    ///
    /// Exact zero and one retain the authored size bounds. A positive interior target never
    /// snaps to zero, including when the first measured interval has flat coverage.
    ///
    /// # Errors
    ///
    /// Rejects nonfinite or out-of-range target response and unresolvable measurement intervals.
    pub fn inverse(&self, target: f64) -> Result<f64, PatternPipelineError> {
        if !target.is_finite() || !(0.0..=1.0).contains(&target) {
            return Err(PatternPipelineError::new(
                "coverage.response.target",
                "coverage target must be finite and normalized",
            ));
        }
        if target == 0.0 {
            return Ok(0.0);
        }
        if target == 1.0 {
            return Ok(1.0);
        }
        let first = self.knots[0].coverage;
        let last = self.knots[self.knots.len() - 1].coverage;
        let range = last - first;
        let upper = self
            .knots
            .partition_point(|knot| (knot.coverage - first) / range < target);
        if upper == 0 || upper >= self.knots.len() {
            return Err(PatternPipelineError::new(
                "coverage.response.inverse",
                "coverage target is outside measured intervals",
            ));
        }
        let left = self.knots[upper - 1];
        let right = self.knots[upper];
        let left_normalized = (left.coverage - first) / range;
        let right_normalized = (right.coverage - first) / range;
        let fraction = (target - left_normalized) / (right_normalized - left_normalized);
        Ok(left.response + fraction * (right.response - left.response))
    }

    /// Inverts an absolute occupied-fraction target, clamping only what the raw curve cannot reach.
    ///
    /// A plateau chooses its earliest measured raw response. This does not normalize the target
    /// against the measured span; each requested source tone retains its absolute meaning.
    ///
    /// # Errors
    /// Rejects a nonfinite or out-of-range coverage request.
    pub fn inverse_absolute(&self, target: f64) -> Result<f64, PatternPipelineError> {
        if !target.is_finite() || !(0.0..=1.0).contains(&target) {
            return Err(PatternPipelineError::new(
                "coverage.response.target",
                "desired coverage must be finite and normalized",
            ));
        }
        let first = self.knots[0];
        if target <= first.coverage {
            return Ok(first.response);
        }
        let last = self.knots[self.knots.len() - 1];
        let reachable = target.min(last.coverage);
        let upper = self.knots.partition_point(|knot| knot.coverage < reachable);
        let left = self.knots[upper - 1];
        let right = self.knots[upper];
        let fraction = (reachable - left.coverage) / (right.coverage - left.coverage);
        Ok(left.response + fraction * (right.response - left.response))
    }
}

/// One transient post-sampling geometry transfer, never an authored or persisted setting.
#[derive(Clone, Copy, Debug)]
pub enum GeometryResponseInput<'a> {
    /// Preserve current source response and realization semantics.
    Identity,
    /// Use a constant geometry-only probe, bypassing source tone, alpha, and cutoff.
    ConstantProbe(f64),
    /// Use a constant final raw zero-to-two size without changing effective coverage settings.
    RawProbe(f64),
    /// Use one engine-calibrated document-space radius without authored fill validation.
    UniformRadius(f64),
    /// Measure the same radius without ordinary source tone, cutoff, or paint sampling.
    UniformRadiusProbe(f64),
    /// Apply an inverse measured response after completed ordinary source sampling.
    Calibrated(&'a CoverageCurve),
    /// Invert one absolute desired-coverage interval on the complete raw zero-to-two curve.
    AbsoluteCoverage {
        curve: &'a CoverageCurve,
        minimum: f64,
        maximum: f64,
    },
}

impl GeometryResponseInput<'_> {
    /// Validates a transient input once, including when its family emits no geometry.
    ///
    /// # Errors
    /// Rejects nonfinite or out-of-range constant probes before any site-local realization.
    pub fn validate(self) -> Result<(), PatternPipelineError> {
        if let Self::ConstantProbe(value) = self {
            self.transfer(value)?;
        }
        if let Self::RawProbe(size) = self
            && (!size.is_finite() || !(0.0..=2.0).contains(&size))
        {
            return Err(PatternPipelineError::new(
                "coverage.response.raw_probe",
                "raw geometry probe must be finite and within zero through two",
            ));
        }
        if let Self::AbsoluteCoverage { minimum, maximum, .. } = self
            && (!minimum.is_finite()
                || !maximum.is_finite()
                || !(0.0..=1.0).contains(&minimum)
                || !(minimum..=1.0).contains(&maximum))
        {
            return Err(PatternPipelineError::new(
                "coverage.response.interval",
                "desired coverage bounds must be ordered within zero through one",
            ));
        }
        if let Self::UniformRadius(radius) | Self::UniformRadiusProbe(radius) = self
            && (!radius.is_finite() || radius < 0.0)
        {
            return Err(PatternPipelineError::new(
                "density.uniform.radius",
                "uniform radius must be finite and nonnegative",
            ));
        }
        Ok(())
    }

    /// Converts one completed response to geometry response without revisiting source pixels.
    ///
    /// # Errors
    ///
    /// Rejects invalid constant probes or inverse curve targets.
    pub fn transfer(self, response: f64) -> Result<f64, PatternPipelineError> {
        match self {
            Self::Identity => Ok(response),
            Self::ConstantProbe(value) => {
                if value.is_finite() && (0.0..=1.0).contains(&value) {
                    Ok(value)
                } else {
                    Err(PatternPipelineError::new(
                        "coverage.response.probe",
                        "constant geometry probe must be finite and normalized",
                    ))
                }
            }
            Self::Calibrated(curve) => curve.inverse(response),
            Self::RawProbe(_) | Self::AbsoluteCoverage { .. } => Err(PatternPipelineError::new(
                "coverage.response.raw_consumer",
                "raw geometry requests require final-size realization",
            )),
            Self::UniformRadius(_) => Err(PatternPipelineError::new(
                "density.uniform.consumer",
                "uniform radius requires canonical marks",
            )),
            Self::UniformRadiusProbe(_) => Err(PatternPipelineError::new(
                "density.uniform.consumer",
                "uniform radius probe requires canonical marks",
            )),
        }
    }

    /// Resolves a final physical raw size without reapplying authored coverage bounds afterward.
    ///
    /// Ordinary identity and older normalized transient probes retain their own physical
    /// response range. Raw probes and absolute coverage return final zero-to-two sizes directly.
    ///
    /// # Errors
    /// Propagates invalid probes, unreachable inverse targets, or nonfinite output sizes.
    pub fn resolve_raw_size(
        self,
        response: f64,
        physical_minimum: f64,
        physical_maximum: f64,
    ) -> Result<f64, PatternPipelineError> {
        let size = match self {
            Self::RawProbe(size) => size,
            Self::AbsoluteCoverage { curve, minimum, maximum } => {
                let desired = minimum + response * (maximum - minimum);
                2.0 * curve.inverse_absolute(desired)?
            }
            _ => physical_minimum + self.transfer(response)? * (physical_maximum - physical_minimum),
        };
        if !size.is_finite() || !(0.0..=2.0).contains(&size) {
            return Err(PatternPipelineError::new(
                "coverage.response.raw_size",
                "resolved raw geometry size must be finite and within zero through two",
            ));
        }
        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::{CoverageCurve, CoverageKnot, GeometryResponseInput};

    /// Proves absolute targets, saturation, equal bounds, floor, plateau, and raw probes retain units.
    ///
    /// # Panics
    /// Panics if an absolute source tone is renormalized to the reachable span or applied twice.
    #[test]
    fn absolute_coverage_inverts_each_requested_tone() {
        let curve = CoverageCurve::new(vec![
            CoverageKnot { response: 0.0, coverage: 0.0 },
            CoverageKnot { response: 1.0, coverage: 0.8 },
        ]).unwrap();
        let input = GeometryResponseInput::AbsoluteCoverage {
            curve: &curve,
            minimum: 0.0,
            maximum: 1.0,
        };
        for (tone, raw) in [(0.25, 0.625), (0.5, 1.25), (0.75, 1.875), (1.0, 2.0)] {
            assert!((input.resolve_raw_size(tone, 0.0, 1.0).unwrap() - raw).abs() < 1e-12);
        }
        let flat = GeometryResponseInput::AbsoluteCoverage { curve: &curve, minimum: 0.5, maximum: 0.5 };
        assert_eq!(flat.resolve_raw_size(0.0, 0.0, 1.0).unwrap(), 1.25);
        assert_eq!(flat.resolve_raw_size(1.0, 0.0, 1.0).unwrap(), 1.25);
        assert_eq!(GeometryResponseInput::RawProbe(1.75).resolve_raw_size(0.1, 0.0, 1.0).unwrap(), 1.75);

        let plateau = CoverageCurve::new(vec![
            CoverageKnot { response: 0.0, coverage: 0.1 },
            CoverageKnot { response: 0.25, coverage: 0.1 },
            CoverageKnot { response: 0.5, coverage: 0.5 },
            CoverageKnot { response: 0.75, coverage: 0.5 },
            CoverageKnot { response: 1.0, coverage: 0.8 },
        ]).unwrap();
        assert_eq!(plateau.inverse_absolute(0.0).unwrap(), 0.0);
        assert_eq!(plateau.inverse_absolute(0.5).unwrap(), 0.5);
        assert!((plateau.inverse_absolute(0.100001).unwrap() - 0.250000625).abs() < 1e-12);
        assert_eq!(plateau.inverse_absolute(1.0).unwrap(), 1.0);
    }

    /// Verifies exact endpoints, positive low values, and earliest plateau inversion.
    #[test]
    fn inverse_preserves_endpoints_and_low_values() {
        let curve = CoverageCurve::new(vec![
            CoverageKnot {
                response: 0.0,
                coverage: 0.0,
            },
            CoverageKnot {
                response: 0.1,
                coverage: 0.0,
            },
            CoverageKnot {
                response: 0.5,
                coverage: 0.25,
            },
            CoverageKnot {
                response: 1.0,
                coverage: 1.0,
            },
        ])
        .unwrap();
        assert_eq!(curve.inverse(0.0).unwrap(), 0.0);
        assert_eq!(curve.inverse(1.0).unwrap(), 1.0);
        assert!(curve.inverse(0.001).unwrap() > 0.1);
        assert_eq!(curve.inverse(0.25).unwrap(), 0.5);
    }

    /// Rejects measured regressions instead of silently sorting or flattening a curve.
    #[test]
    fn rejects_nonmonotone_coverage() {
        assert!(
            CoverageCurve::new(vec![
                CoverageKnot {
                    response: 0.0,
                    coverage: 0.0
                },
                CoverageKnot {
                    response: 0.5,
                    coverage: 0.4
                },
                CoverageKnot {
                    response: 1.0,
                    coverage: 0.3
                },
            ])
            .is_err()
        );
    }

    /// Preserves a positive interior inverse even when adding it to a nonzero baseline rounds away.
    #[test]
    fn inverse_nonzero_baseline_retains_tiny_positive_target() {
        let curve = CoverageCurve::new(vec![
            CoverageKnot {
                response: 0.0,
                coverage: 0.25,
            },
            CoverageKnot {
                response: 1.0,
                coverage: 0.75,
            },
        ])
        .unwrap();
        assert!(curve.inverse(1e-20).unwrap() > 0.0);
        assert_eq!(curve.inverse(0.0).unwrap(), 0.0);
        assert_eq!(curve.inverse(1.0).unwrap(), 1.0);
    }

    /// Rejects an invalid constant even if a separate empty family would call no site sampler.
    #[test]
    fn constant_probe_validates_before_site_iteration() {
        assert!(
            GeometryResponseInput::ConstantProbe(f64::NAN)
                .validate()
                .is_err()
        );
        assert!(
            GeometryResponseInput::ConstantProbe(1.01)
                .validate()
                .is_err()
        );
        assert!(GeometryResponseInput::ConstantProbe(0.0).validate().is_ok());
    }
}
