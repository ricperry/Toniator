//! Source-independent full-tone mark sizing and expected random-thinning response.

use std::collections::BTreeMap;

use toniator_domain::{CanvasSpec, UniformCalibration};
use toniator_patterns::{CoverageCurve, CoverageKnot};
use toniator_render::{
    GeometryOutput, OutputRasterTarget, RasterizationLimits, rasterize_mark_multiplicity,
};

use crate::EvaluationError;

const FULL_TONE_TARGET: f64 = 0.999;
const MAX_RADIUS_PROBES: usize = 22;
const INVERSE_INTERVALS: usize = 256;
const HELDOUT_TONES: [f64; 9] = [0.001, 0.005, 0.01, 0.05, 0.25, 0.5, 0.75, 0.95, 0.999];
#[cfg(test)]
pub(crate) static FIT_ENTRY_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Source-independent baseline and precomputed, validated survival inverse.
#[derive(Clone, Debug)]
pub(crate) struct UniformDensityFit {
    pub radius: f64,
    pub full_coverage: f64,
    pub radius_probes: usize,
    pub heldout_max_absolute_error: f64,
    pub heldout_max_relative_error: f64,
    inverse: CoverageCurve,
}

impl UniformDensityFit {
    /// Returns the measured probability-to-coverage knots for an immutable saved artifact.
    pub fn inverse_knots(&self) -> &[CoverageKnot] {
        self.inverse.knots()
    }
    /// Restores a measured survival inverse at the receiving family's canonical spacing.
    ///
    /// # Errors
    /// Rejects invalid receiving spacing or an inverse that violates the current geometry curve.
    pub fn from_saved(
        saved: &UniformCalibration,
        nominal_spacing: f64,
    ) -> Result<Self, EvaluationError> {
        if !nominal_spacing.is_finite() || nominal_spacing <= 0.0 {
            return Err(EvaluationError::new(
                "density.uniform.spacing",
                "receiving nominal spacing must be finite and positive",
            ));
        }
        let inverse = CoverageCurve::new(
            saved
                .inverse()
                .iter()
                .map(|knot| CoverageKnot {
                    response: knot.input,
                    coverage: knot.coverage,
                })
                .collect(),
        )
        .map_err(EvaluationError::from_pipeline)?;
        let fit = Self {
            radius: saved.radius_per_nominal_spacing() * nominal_spacing,
            full_coverage: saved
                .inverse()
                .last()
                .expect("validated full-tone endpoint")
                .coverage,
            radius_probes: 0,
            heldout_max_absolute_error: 0.0,
            heldout_max_relative_error: 0.0,
            inverse,
        };
        fit.validate_for_reuse()?;
        Ok(fit)
    }
    /// Rechecks that a cached baseline retains the bounded measured calibration contract.
    ///
    /// # Errors
    /// Rejects a stale or internally inconsistent cached fit before artistic scale is applied.
    pub fn validate_for_reuse(&self) -> Result<(), EvaluationError> {
        if self.full_coverage < FULL_TONE_TARGET
            || self.radius_probes > MAX_RADIUS_PROBES
            || !self.heldout_max_absolute_error.is_finite()
            || !self.heldout_max_relative_error.is_finite()
        {
            return Err(EvaluationError::new(
                "density.uniform.cache",
                "cached uniform fit violates its measured endpoint or probe bound",
            ));
        }
        Ok(())
    }

    /// Inverts expected occupied fraction in one bounded lookup without a per-site root solve.
    ///
    /// # Errors
    /// Rejects nonfinite or out-of-range completed weighting before mark selection.
    pub fn probability(&self, weight: f64) -> Result<f64, EvaluationError> {
        self.inverse
            .inverse(weight)
            .map_err(EvaluationError::from_pipeline)
    }
}

/// Counts complete-canvas overlap multiplicities without retaining per-pixel masks.
///
/// # Errors
/// Propagates checked target, renderer, allocation, or cancellation diagnostics.
fn histogram(
    geometry: &GeometryOutput,
    canvas: &CanvasSpec,
    long_side: u32,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(BTreeMap<u32, usize>, usize), EvaluationError> {
    let long = canvas.width.max(canvas.height);
    let width = ((canvas.width / long) * f64::from(long_side))
        .round()
        .max(1.0) as u32;
    let height = ((canvas.height / long) * f64::from(long_side))
        .round()
        .max(1.0) as u32;
    let target = OutputRasterTarget::new(width, height).map_err(EvaluationError::from_render)?;
    let mask = rasterize_mark_multiplicity(
        geometry,
        canvas,
        target,
        RasterizationLimits::default(),
        is_cancelled,
    )
    .map_err(EvaluationError::from_render)?;
    let mut bins = BTreeMap::new();
    for &count in mask.counts() {
        *bins.entry(count).or_insert(0) += 1;
    }
    Ok((bins, mask.counts().len()))
}

/// Pools equal-canvas multiplicity frequencies from one or two frozen site populations.
///
/// Pooling counts and pixel totals forms their arithmetic expected-coverage mean at every
/// survival probability. It never merges overlapping geometry from distinct realizations.
///
/// # Errors
/// Rejects an empty or excessive population set and propagates raster diagnostics.
fn histogram_populations(
    geometries: &[GeometryOutput],
    canvas: &CanvasSpec,
    long_side: u32,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(BTreeMap<u32, usize>, usize), EvaluationError> {
    if !(1..=2).contains(&geometries.len()) {
        return Err(EvaluationError::new(
            "density.uniform.population",
            "uniform reference requires one or two frozen populations",
        ));
    }
    let mut pooled = BTreeMap::new();
    let mut pixels = 0_usize;
    for geometry in geometries {
        let (bins, count) = histogram(geometry, canvas, long_side, is_cancelled)?;
        pixels = pixels.checked_add(count).ok_or(EvaluationError::new(
            "density.uniform.population",
            "uniform histogram pixel count exceeds its bound",
        ))?;
        for (multiplicity, amount) in bins {
            *pooled.entry(multiplicity).or_insert(0) += amount;
        }
    }
    Ok((pooled, pixels))
}

/// Evaluates the stable expected union of independent per-mark Bernoulli survival.
fn expected(bins: &BTreeMap<u32, usize>, pixels: usize, probability: f64) -> f64 {
    if probability == 0.0 {
        return 0.0;
    }
    if probability == 1.0 {
        return 1.0 - bins.get(&0).copied().unwrap_or(0) as f64 / pixels as f64;
    }
    let log_miss = (-probability).ln_1p();
    let occupied = bins
        .iter()
        .filter(|(count, _)| **count > 0)
        .map(|(count, amount)| -((f64::from(*count) * log_miss).exp_m1()) * *amount as f64)
        .sum::<f64>();
    occupied / pixels as f64
}

/// Checks one independent population against a saved paired-population probability inverse.
///
/// The result is empirical transfer evidence at three raster resolutions and fixed tone witnesses.
/// No single deterministic thinning realization or future artwork is promised to match it.
///
/// # Errors
/// Rejects missing 99.9% full tone, a transfer above one percentage point or ten percent of
/// requested low tone, and all raster, inverse, or cancellation failures.
pub(crate) fn check_population_transfer(
    canvas: &CanvasSpec,
    geometry: &GeometryOutput,
    fit: &UniformDensityFit,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(f64, f64), EvaluationError> {
    let histograms = [192, 271, 383]
        .into_iter()
        .map(|side| histogram(geometry, canvas, side, is_cancelled))
        .collect::<Result<Vec<_>, _>>()?;
    let mut max_absolute = 0.0_f64;
    let mut max_relative = 0.0_f64;
    if std::env::var_os("TONIATOR_CALIBRATION_DIAGNOSTICS").is_some() {
        for (bins, pixels) in &histograms {
            let full = expected(bins, *pixels, 1.0);
            eprintln!(
                "Uniform independent full tone: pixels={pixels} coverage={full:.9} target={FULL_TONE_TARGET:.9} radius_ratio={:.9}",
                fit.radius,
            );
        }
    }
    for (bins, pixels) in &histograms {
        let full = expected(bins, *pixels, 1.0);
        if full < FULL_TONE_TARGET {
            return Err(EvaluationError::new(
                "density.uniform.population_full_tone",
                "independent Uniform population misses 99.9% full tone",
            ));
        }
        for tone in HELDOUT_TONES {
            if is_cancelled() {
                return Err(EvaluationError::new(
                    "evaluation.cancelled",
                    "uniform population transfer was cancelled",
                ));
            }
            let p = fit.probability(tone)?;
            let actual = expected(bins, *pixels, p) / full;
            let delta = (actual - tone).abs();
            let relative = delta / tone.max(0.0005);
            max_absolute = max_absolute.max(delta);
            max_relative = max_relative.max(relative);
            if delta > 0.01 || relative > 0.1 {
                return Err(EvaluationError::new(
                    "density.uniform.population_transfer",
                    "independent Uniform population exceeds bounded expected-tone variation",
                ));
            }
        }
    }
    Ok((max_absolute, max_relative))
}

/// Fits one validated 99.9% full-tone footprint with a fixed complete site population.
///
/// A passing upper endpoint is always an actual canonical measurement; local bisection may
/// retain it when an authored nonconvex shape has a nonmonotone size curve. Both primary and
/// independent resolution must pass. The supplied maximum radius is the family support divided
/// by the maximum artistic scale, so all later 0..=2 scale edits retain the same population.
///
/// # Errors
/// Returns an explicit unreachable, inconsistent heldout, bound, producer, or cancellation error.
pub(crate) fn fit(
    canvas: &CanvasSpec,
    maximum_baseline_radius: f64,
    produce: &dyn Fn(f64) -> Result<GeometryOutput, EvaluationError>,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<UniformDensityFit, EvaluationError> {
    fit_populations(
        canvas,
        maximum_baseline_radius,
        &|radius| Ok(vec![produce(radius)?]),
        None,
        is_cancelled,
    )
}

/// Checks the full-tone footprint at all three independent transfer resolutions.
///
/// # Errors
/// Propagates rasterization and cancellation failures from each resolution.
fn meets_full_tone_at_resolutions(
    geometry: &GeometryOutput,
    canvas: &CanvasSpec,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<bool, EvaluationError> {
    let mut reaches_full_tone = true;
    for side in [192, 271, 383] {
        let (bins, pixels) = histogram(geometry, canvas, side, is_cancelled)?;
        reaches_full_tone &= expected(&bins, pixels, 1.0) >= FULL_TONE_TARGET;
    }
    Ok(reaches_full_tone)
}

/// Fits one or two complete canonical populations under one bounded radius and inverse contract.
///
/// A mean fit may use an independent white population to select a common full-tone footprint.
/// Its lower-tone response remains an independent transfer check after the paired inverse is fit.
///
/// # Errors
/// Preserves the existing full-tone, held-out, producer, support, and cancellation gates.
pub(crate) fn fit_populations(
    canvas: &CanvasSpec,
    maximum_baseline_radius: f64,
    produce: &dyn Fn(f64) -> Result<Vec<GeometryOutput>, EvaluationError>,
    full_tone_witness: Option<&dyn Fn(f64) -> Result<GeometryOutput, EvaluationError>>,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<UniformDensityFit, EvaluationError> {
    #[cfg(test)]
    FIT_ENTRY_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if !maximum_baseline_radius.is_finite() || maximum_baseline_radius <= 0.0 {
        return Err(EvaluationError::new(
            "density.uniform.support",
            "uniform support must reserve a positive finite baseline and maximum artistic scale",
        ));
    }
    let mut probes = 0;
    let mut lower = 0.0;
    let mut candidate = maximum_baseline_radius / 8.0;
    let mut passing = None;
    while probes < MAX_RADIUS_PROBES {
        if is_cancelled() {
            return Err(EvaluationError::new(
                "evaluation.cancelled",
                "uniform density fit was cancelled",
            ));
        }
        let geometry = produce(candidate)?;
        let primary = histogram_populations(&geometry, canvas, 192, is_cancelled)?;
        let check = histogram_populations(&geometry, canvas, 271, is_cancelled)?;
        probes += 1;
        if expected(&primary.0, primary.1, 1.0) >= FULL_TONE_TARGET
            && expected(&check.0, check.1, 1.0) >= FULL_TONE_TARGET
        {
            passing = Some((candidate, primary, check, geometry));
            break;
        }
        lower = candidate;
        if candidate == maximum_baseline_radius {
            break;
        }
        candidate = (candidate * 2.0).min(maximum_baseline_radius);
    }
    let Some((mut upper, mut primary, mut check, mut geometry)) = passing else {
        return Err(EvaluationError::new(
            "density.uniform.unreachable",
            "99.9% full-tone coverage is unattainable within the planned family support and maximum artistic scale",
        ));
    };
    for _ in 0..10 {
        if probes >= MAX_RADIUS_PROBES {
            break;
        }
        let middle = (lower + upper) / 2.0;
        if middle <= lower || middle >= upper {
            break;
        }
        let candidate_geometry = produce(middle)?;
        let candidate_primary =
            histogram_populations(&candidate_geometry, canvas, 192, is_cancelled)?;
        let candidate_check =
            histogram_populations(&candidate_geometry, canvas, 271, is_cancelled)?;
        probes += 1;
        if expected(&candidate_primary.0, candidate_primary.1, 1.0) >= FULL_TONE_TARGET
            && expected(&candidate_check.0, candidate_check.1, 1.0) >= FULL_TONE_TARGET
        {
            upper = middle;
            primary = candidate_primary;
            check = candidate_check;
            geometry = candidate_geometry;
        } else {
            lower = middle;
        }
    }
    let mut heldout = histogram_populations(&geometry, canvas, 383, is_cancelled)?;
    let mut independent_full = match full_tone_witness {
        Some(produce_witness) => {
            let witness = produce_witness(upper)?;
            meets_full_tone_at_resolutions(&witness, canvas, is_cancelled)?
        }
        None => true,
    };
    let mut trial_radius = upper;
    for _ in 0..4 {
        if expected(&heldout.0, heldout.1, 1.0) >= FULL_TONE_TARGET && independent_full {
            break;
        }
        let next = (trial_radius * 1.02).min(maximum_baseline_radius);
        if next <= trial_radius || probes >= MAX_RADIUS_PROBES {
            break;
        }
        // A nonconvex footprint can lose full tone at one candidate and regain it at a later
        // bounded radius. Keep the last accepted inverse geometry while advancing the trial.
        trial_radius = next;
        let candidate_geometry = produce(next)?;
        let candidate_primary =
            histogram_populations(&candidate_geometry, canvas, 192, is_cancelled)?;
        let candidate_check =
            histogram_populations(&candidate_geometry, canvas, 271, is_cancelled)?;
        let candidate_heldout =
            histogram_populations(&candidate_geometry, canvas, 383, is_cancelled)?;
        probes += 1;
        if expected(&candidate_primary.0, candidate_primary.1, 1.0) >= FULL_TONE_TARGET
            && expected(&candidate_check.0, candidate_check.1, 1.0) >= FULL_TONE_TARGET
        {
            independent_full = match full_tone_witness {
                Some(produce_witness) => {
                    let witness = produce_witness(next)?;
                    meets_full_tone_at_resolutions(&witness, canvas, is_cancelled)?
                }
                None => true,
            };
            upper = next;
            primary = candidate_primary;
            check = candidate_check;
            heldout = candidate_heldout;
        }
    }
    let full = expected(&primary.0, primary.1, 1.0);
    if expected(&heldout.0, heldout.1, 1.0) < FULL_TONE_TARGET {
        return Err(EvaluationError::new(
            "density.uniform.heldout",
            "calibrated full-tone footprint misses 99.9% at independent resolution",
        ));
    }
    if !independent_full {
        return Err(EvaluationError::new(
            "density.uniform.population_full_tone",
            "independent Uniform population misses 99.9% full tone",
        ));
    }
    let mut tones = vec![0.0, 0.00001, 0.0001, 0.001, 0.005, 0.01];
    tones.extend((1..=INVERSE_INTERVALS).map(|index| index as f64 / INVERSE_INTERVALS as f64));
    tones.sort_by(f64::total_cmp);
    tones.dedup();
    let knots = tones
        .into_iter()
        .map(|tone| -> Result<CoverageKnot, EvaluationError> {
            if is_cancelled() {
                return Err(EvaluationError::new(
                    "evaluation.cancelled",
                    "uniform density inverse was cancelled",
                ));
            }
            let p = if tone == 0.0 {
                0.0
            } else if tone == 1.0 {
                1.0
            } else {
                let mut low = 0.0;
                let mut high = 1.0;
                for _ in 0..48 {
                    let mid = (low + high) / 2.0;
                    if expected(&primary.0, primary.1, mid) < tone * full {
                        low = mid;
                    } else {
                        high = mid;
                    }
                }
                high
            };
            Ok(CoverageKnot {
                response: p,
                coverage: expected(&primary.0, primary.1, p),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let inverse = CoverageCurve::new(knots).map_err(EvaluationError::from_pipeline)?;
    let mut heldout_max_absolute_error = 0.0_f64;
    let mut heldout_max_relative_error = 0.0_f64;
    for tone in HELDOUT_TONES {
        if is_cancelled() {
            return Err(EvaluationError::new(
                "evaluation.cancelled",
                "uniform density validation was cancelled",
            ));
        }
        let p = inverse
            .inverse(tone)
            .map_err(EvaluationError::from_pipeline)?;
        let actual = expected(&check.0, check.1, p);
        let high = expected(&heldout.0, heldout.1, p);
        let check_target = tone * expected(&check.0, check.1, 1.0);
        let high_target = tone * expected(&heldout.0, heldout.1, 1.0);
        let check_delta = (actual - check_target).abs();
        let high_delta = (high - high_target).abs();
        heldout_max_absolute_error = heldout_max_absolute_error.max(check_delta).max(high_delta);
        heldout_max_relative_error = heldout_max_relative_error
            .max(check_delta / check_target.max(0.0005))
            .max(high_delta / high_target.max(0.0005));
        if check_delta > 0.0025
            || check_delta > 0.1 * check_target.max(0.0005)
            || high_delta > 0.0025
            || high_delta > 0.1 * high_target.max(0.0005)
        {
            return Err(EvaluationError::new(
                "density.uniform.probability_heldout",
                "survival inverse disagrees with independent footprint resolution",
            ));
        }
    }
    Ok(UniformDensityFit {
        radius: upper,
        full_coverage: full,
        radius_probes: probes,
        heldout_max_absolute_error,
        heldout_max_relative_error,
        inverse,
    })
}

#[cfg(test)]
mod mean_tests {
    use super::*;
    use toniator_domain::GuideDimensionId;
    use toniator_patterns::{
        CanonicalCircleMark, GuideInstanceId, GuideIntersectionProvenance, Point2, SiteId,
        SiteScope,
    };

    /// Proves pooled multiplicity frequencies equal the arithmetic expected-coverage mean.
    ///
    /// # Panics
    /// Panics if probabilities at zero, intermediate, or full survival lose this identity.
    #[test]
    fn pooled_histogram_is_population_mean_without_geometry_union() {
        let first = BTreeMap::from([(0, 25), (1, 75)]);
        let second = BTreeMap::from([(0, 50), (2, 50)]);
        let pooled = BTreeMap::from([(0, 75), (1, 75), (2, 50)]);
        for probability in [0.0, 0.001, 0.25, 0.5, 1.0] {
            let observed = expected(&pooled, 200, probability);
            let mean =
                (expected(&first, 100, probability) + expected(&second, 100, probability)) / 2.0;
            assert!((observed - mean).abs() <= 1.0e-12);
        }
        assert_eq!(expected(&pooled, 200, 0.0), 0.0);
    }

    /// Exercises bounded upward trials after a nonmonotone full-tone candidate fails.
    ///
    /// This synthetic geometry callback isolates retry control flow; it does not claim that a
    /// physical Pattern has this radius response. Both paired and independent gates still run.
    ///
    /// # Panics
    /// Panics if a failed 2% trial prevents a later passing 4.04% trial, or if a permanently
    /// failing independent population is accepted after the four bounded attempts.
    #[test]
    fn full_tone_retry_advances_past_failed_nonmonotone_trial() {
        let canvas = CanvasSpec {
            width: 10.0,
            height: 10.0,
        };
        let full = GeometryOutput::CircularMarks(vec![
            CanonicalCircleMark::new(
                SiteId {
                    first_dimension_id: 1,
                    first_index: 0,
                    second_dimension_id: 2,
                    second_index: 0,
                },
                Point2::new(5.0, 5.0),
                20.0,
                SiteScope::Canvas,
                GuideIntersectionProvenance {
                    contributors: vec![
                        GuideInstanceId::new(GuideDimensionId(1), 0),
                        GuideInstanceId::new(GuideDimensionId(2), 0),
                    ],
                },
            )
            .expect("finite circle covers the synthetic canvas"),
        ]);
        let empty = GeometryOutput::CircularMarks(Vec::new());
        let paired = |radius: f64| {
            Ok(vec![if radius >= 1.0 && !(1.02..1.04).contains(&radius) {
                full.clone()
            } else {
                empty.clone()
            }])
        };
        let independent = |radius: f64| {
            Ok(if radius >= 1.04 {
                full.clone()
            } else {
                empty.clone()
            })
        };
        let fit = fit_populations(&canvas, 8.0, &paired, Some(&independent), &|| false)
            .expect("later bounded radius passes both populations");
        assert!((fit.radius - 1.0404).abs() < 1.0e-12);
        assert_eq!(fit.radius_probes, 13);
        assert!(fit.full_coverage >= FULL_TONE_TARGET);

        let never_full = |_radius: f64| Ok(empty.clone());
        let error = fit_populations(&canvas, 8.0, &paired, Some(&never_full), &|| false)
            .expect_err("the independent full-tone gate remains authoritative");
        assert_eq!(error.path(), "density.uniform.population_full_tone");
    }
}
