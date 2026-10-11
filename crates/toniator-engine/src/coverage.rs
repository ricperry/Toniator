//! Transient, geometry-derived response fitting for one frozen output producer.

use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use rayon::prelude::*;
use toniator_domain::CanvasSpec;
use toniator_patterns::{CoverageCurve, CoverageKnot};
use toniator_domain::SavedCalibration;
#[cfg(test)]
pub(crate) static FIT_ENTRY_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
use toniator_render::{
    GeometryOutput, OutputRasterTarget, RasterizationLimits, VectorOccupancyPrecision,
    measure_geometry_vector_occupancy, rasterize_geometry_occupancy,
};

use crate::EvaluationError;

const MAX_PROBES: usize = 64;
const SEEDS: [f64; 10] = [0.0, 0.01, 0.03, 0.1, 0.25, 0.5, 0.75, 0.9, 0.97, 1.0];
// A failed interval retains one knot and schedules two checks. The initial intervals plus the
// children of at most MAX_PROBES-SEEDS.len() retained refinements bound cached q measurements.
const MAX_CACHED_SAMPLES: usize = SEEDS.len() + (SEEDS.len() - 1) + 2 * (MAX_PROBES - SEEDS.len());
// All producer calls, including endpoint geometry: at most 127 cached seed/midpoint q values,
// 62 interior vector phase rechecks, 18 independent heldouts, and 60 local vector repairs.
const MAX_GEOMETRY_EVALUATIONS: usize =
    MAX_CACHED_SAMPLES + (MAX_PROBES - 2) + 3 * WITNESSES.len() + 10 * WITNESSES.len();
// Small high-coverage residuals can be concentrated in a narrow response interval for authored
// shapes; the probe cap still bounds total work when deeper local subdivision is needed.
const MAX_REFINEMENT_DEPTH: u8 = 12;
const WITNESSES: [f64; 6] = [0.001, 0.005, 0.01, 0.25, 0.5, 0.75];
const MAX_HELDOUT_SAMPLES: usize = 3 * WITNESSES.len();
type VectorEndpointPairs = ((f64, f64), (f64, f64));

/// Numerical consumer retained for each measured response, never inferred from a pattern name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Backend {
    Raster,
    Vector,
}

/// One deterministic pair of independent opaque-union measurements.
#[derive(Clone, Copy, Debug)]
struct Sample {
    q: f64,
    /// Endpoint-normalized occupancy, independent of raster target or vector precision.
    primary: f64,
    check: f64,
    backend: Backend,
    resolution: u32,
}

/// Validated inverse and the selected backend/resolution of every retained measurement knot.
#[derive(Clone, Debug)]
pub(crate) struct CoverageFit {
    pub curve: CoverageCurve,
    pub provenance: Vec<(f64, Backend, u32, f64, f64)>,
    /// Desired absolute coverage endpoints only for a persisted raw-curve transfer.
    pub target_range: Option<(f64, f64)>,
}

/// Projects the complete saved raw zero-to-two measurement into absolute coverage authority.
///
/// Equal desired endpoints remain a constant coverage request, still requiring the raw inverse.
///
/// # Errors
/// Rejects invalid coverage bounds or a stored curve without a monotone raw response.
pub(crate) fn from_saved(
    saved: &SavedCalibration,
    minimum: f64,
    maximum: f64,
) -> Result<CoverageFit, EvaluationError> {
    if !minimum.is_finite() || !maximum.is_finite() || minimum < 0.0 || maximum > 1.0 || minimum > maximum {
        return Err(EvaluationError::new("calibration.interval", "desired coverage bounds must be ordered within zero through one"));
    }
    let knots = saved.raw().iter().map(|point| CoverageKnot {
        response: point.input / 2.0,
        coverage: point.coverage,
    }).collect();
    Ok(CoverageFit {
        curve: CoverageCurve::new(knots).map_err(EvaluationError::from_pipeline)?,
        provenance: Vec::new(),
        target_range: Some((minimum, maximum)),
    })
}

/// Retains sorted fit-knot provenance without changing any measured coverage.
fn fitted(curve: CoverageCurve, table: &[Sample]) -> CoverageFit {
    CoverageFit {
        curve,
        provenance: table
            .iter()
            .map(|sample| {
                (
                    sample.q,
                    sample.backend,
                    sample.resolution,
                    sample.primary,
                    sample.check,
                )
            })
            .collect(),
        target_range: None,
    }
}

/// Returns one native-aspect target with a requested long side and renderer-validated bounds.
///
/// # Errors
/// Returns a target diagnostic when the aspect is not representable within renderer limits.
fn target(canvas: &CanvasSpec, long_side: u32) -> Result<OutputRasterTarget, EvaluationError> {
    let long = canvas.width.max(canvas.height);
    let width = ((canvas.width / long) * f64::from(long_side))
        .round()
        .max(1.0) as u32;
    let height = ((canvas.height / long) * f64::from(long_side))
        .round()
        .max(1.0) as u32;
    OutputRasterTarget::new(width, height).map_err(EvaluationError::from_render)
}

/// Measures one frozen canonical output with independent native-aspect mask targets.
///
/// # Errors
/// Returns the renderer's validation, cancellation, allocation, or edge diagnostic.
fn raster_pair(
    geometry: &GeometryOutput,
    canvas: &CanvasSpec,
    long_sides: [u32; 2],
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(f64, f64), EvaluationError> {
    let first = rasterize_geometry_occupancy(
        geometry,
        canvas,
        target(canvas, long_sides[0])?,
        RasterizationLimits::default(),
        is_cancelled,
    )
    .map_err(EvaluationError::from_render)?
    .fraction();
    let second = rasterize_geometry_occupancy(
        geometry,
        canvas,
        target(canvas, long_sides[1])?,
        RasterizationLimits::default(),
        is_cancelled,
    )
    .map_err(EvaluationError::from_render)?
    .fraction();
    Ok((first, second))
}

/// Measures one native-aspect binary mask without changing canonical geometry or appearance.
///
/// # Errors
/// Returns target, validation, cancellation, allocation, or edge diagnostics.
fn raster_at(
    geometry: &GeometryOutput,
    canvas: &CanvasSpec,
    long_side: u32,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<f64, EvaluationError> {
    rasterize_geometry_occupancy(
        geometry,
        canvas,
        target(canvas, long_side)?,
        RasterizationLimits::default(),
        is_cancelled,
    )
    .map_err(EvaluationError::from_render)
    .map(|mask| mask.fraction())
}

/// Measures one frozen output with two supported vector flatten/quantization precisions.
///
/// # Errors
/// Returns the vector consumer's geometry, cancellation, work-limit, or precision diagnostic.
fn vector_pair(
    geometry: &GeometryOutput,
    canvas: &CanvasSpec,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(f64, f64), EvaluationError> {
    let first = measure_geometry_vector_occupancy(
        geometry,
        canvas,
        VectorOccupancyPrecision::Baseline,
        RasterizationLimits::default(),
        is_cancelled,
    )
    .map_err(EvaluationError::from_render)?
    .fraction;
    let second = measure_geometry_vector_occupancy(
        geometry,
        canvas,
        VectorOccupancyPrecision::Tight,
        RasterizationLimits::default(),
        is_cancelled,
    )
    .map_err(EvaluationError::from_render)?
    .fraction;
    Ok((first, second))
}

/// Tests absolute and endpoint-relative disagreement, with a finite low-end noise floor.
fn disagreement(primary: f64, check: f64) -> bool {
    let delta = (primary - check).abs();
    let edge = primary
        .min(1.0 - primary)
        .min(check.min(1.0 - check))
        .max(0.0);
    delta > 0.0025 || delta > 0.1 * edge.max(0.0005)
}

/// Treats two zero raster hits at a positive response as unresolved spatial sampling.
fn raster_uncertain(primary: f64, check: f64, q: f64) -> bool {
    (q > 0.0 && q < 1.0 && primary == 0.0 && check == 0.0) || disagreement(primary, check)
}

/// Converts measured occupancy to endpoint-normalized coverage without altering observations.
///
/// # Errors
/// Rejects a flat/nonfinite range; valid fixed-size outputs are handled before fitting.
fn normalized(sample: Sample, zero: Sample, one: Sample) -> Result<f64, EvaluationError> {
    normalized_value(sample.primary, zero.primary, one.primary)
}

/// Normalizes one raw consumer fraction using that consumer's own measured endpoints.
///
/// # Errors
/// Rejects an absent or nonfinite attainable range.
fn normalized_value(value: f64, zero: f64, one: f64) -> Result<f64, EvaluationError> {
    let range = one - zero;
    if !range.is_finite() || range <= 0.0 {
        return Err(EvaluationError::new(
            "coverage.response.range",
            "geometry has no positive attainable coverage range",
        ));
    }
    Ok((value - zero) / range)
}

/// Builds a checked monotone curve from retained measurements in deterministic q order.
///
/// # Errors
/// Rejects a decreasing or incomplete curve without sorting or flattening coverage.
fn checked_curve(table: &[Sample]) -> Result<CoverageCurve, EvaluationError> {
    let first = *table.first().ok_or(EvaluationError::new(
        "coverage.response.knots",
        "coverage table is empty",
    ))?;
    let last = *table.last().ok_or(EvaluationError::new(
        "coverage.response.knots",
        "coverage table is empty",
    ))?;
    let mut knots = Vec::with_capacity(table.len());
    for sample in table {
        let coverage = normalized(*sample, first, last)?;
        if !coverage.is_finite() || !(-0.005..=1.005).contains(&coverage) {
            return Err(EvaluationError::new(
                "coverage.response.range",
                "coverage measurement exceeds its endpoint range",
            ));
        }
        knots.push(CoverageKnot {
            response: sample.q,
            coverage: sample.primary,
        });
    }
    CoverageCurve::new(knots).map_err(EvaluationError::from_pipeline)
}

/// Rechecks adjacent phase reversals with the independent vector consumer before curve fitting.
///
/// Each implicated interior knot is replaced by a converged vector measurement. A reversal that
/// remains after two bounded passes is a real unresolved response, never silently flattened.
///
/// # Errors
/// Returns vector-consumer diagnostics or an unresolved monotonicity diagnostic.
fn repair_phase_reversals(
    table: &mut [Sample],
    measure_vector: &impl Fn(f64) -> Result<Sample, EvaluationError>,
) -> Result<(), EvaluationError> {
    for _ in 0..2 {
        let mut indices = Vec::new();
        for (index, pair) in table.windows(2).enumerate() {
            if pair[1].primary < pair[0].primary || pair[1].check < pair[0].check {
                if index > 0 && pair[0].backend != Backend::Vector {
                    indices.push(index);
                }
                if index + 1 < table.len() - 1 && pair[1].backend != Backend::Vector {
                    indices.push(index + 1);
                }
            }
        }
        if indices.is_empty() {
            break;
        }
        indices.sort_unstable();
        indices.dedup();
        for index in indices {
            table[index] = measure_vector(table[index].q)?;
        }
    }
    if table
        .windows(2)
        .any(|pair| pair[1].primary < pair[0].primary || pair[1].check < pair[0].check)
    {
        return Err(EvaluationError::new(
            "coverage.response.monotone",
            "independent coverage measurements remain nonmonotone",
        ));
    }
    Ok(())
}

/// Fits and validates an inverse table on one already-frozen producer, using one caller-owned pool.
///
/// Each q rebuilds only canonical output geometry from the frozen family. The two mask consumers
/// are deterministic; uncertain points use vector union locally. Raster-only curves receive
/// independent noncommensurate raster heldouts; mixed curves receive vector heldouts. Failed
/// intervals receive at most two local vector repairs. Heldouts and repairs use the caller's
/// bounded pool, and identical independent heldouts can be reused across repair rounds. No
/// source tone, opacity, or paint enters this measurement, and no partial curve escapes failure
/// or cancellation.
///
/// # Errors
/// Returns producer, consumer, cancellation, nonmonotone, cap, or unresolved accuracy diagnostics.
pub(crate) fn fit_curve<F>(
    canvas: &CanvasSpec,
    produce: &F,
    pool: &rayon::ThreadPool,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<CoverageFit>, EvaluationError>
where
    F: Fn(f64) -> Result<GeometryOutput, EvaluationError> + Sync,
{
    #[cfg(test)]
    FIT_ENTRY_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if is_cancelled() {
        return Err(EvaluationError::new(
            "evaluation.cancelled",
            "coverage fit was cancelled",
        ));
    }
    let geometry_evaluations = AtomicUsize::new(0);
    let produce_bounded = |q: f64| -> Result<GeometryOutput, EvaluationError> {
        if is_cancelled() {
            return Err(EvaluationError::new(
                "evaluation.cancelled",
                "coverage probe was cancelled",
            ));
        }
        geometry_evaluations
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_GEOMETRY_EVALUATIONS).then_some(count + 1)
            })
            .map_err(|_| {
                EvaluationError::new(
                    "coverage.response.work_limit",
                    "coverage response exceeds its bounded geometry measurement work",
                )
            })?;
        produce(q)
    };
    let cached = Mutex::new(BTreeMap::<u64, Sample>::new());
    let high_resolution_gate = Mutex::new(());
    let zero_geometry = produce_bounded(0.0)?;
    let one_geometry = produce_bounded(1.0)?;
    let raster_endpoints = Mutex::new(BTreeMap::<u32, (f64, f64)>::new());
    let raster_reference = |side: u32| -> Result<(f64, f64), EvaluationError> {
        let mut slot = raster_endpoints.lock().map_err(|_| {
            EvaluationError::new(
                "coverage.response.cache",
                "raster endpoint lock was poisoned",
            )
        })?;
        if let Some(value) = slot.get(&side) {
            return Ok(*value);
        }
        let pair = (
            raster_at(&zero_geometry, canvas, side, is_cancelled)?,
            raster_at(&one_geometry, canvas, side, is_cancelled)?,
        );
        slot.insert(side, pair);
        Ok(pair)
    };
    let raster_normalized =
        |geometry: &GeometryOutput, side: u32| -> Result<f64, EvaluationError> {
            let (zero, one) = raster_reference(side)?;
            normalized_value(raster_at(geometry, canvas, side, is_cancelled)?, zero, one)
        };
    let primary_range = raster_reference(2048)?.1 - raster_reference(2048)?.0;
    let check_range = raster_reference(2713)?.1 - raster_reference(2713)?.0;
    let forced_vector = primary_range <= 0.0 || check_range <= 0.0;
    let vector_endpoints = Mutex::new(None::<VectorEndpointPairs>);
    let vector_references = || -> Result<VectorEndpointPairs, EvaluationError> {
        let mut slot = vector_endpoints.lock().map_err(|_| {
            EvaluationError::new(
                "coverage.response.cache",
                "vector endpoint lock was poisoned",
            )
        })?;
        if let Some(value) = *slot {
            return Ok(value);
        }
        let value = (
            vector_pair(&zero_geometry, canvas, is_cancelled)?,
            vector_pair(&one_geometry, canvas, is_cancelled)?,
        );
        *slot = Some(value);
        Ok(value)
    };
    if forced_vector {
        let (zero, one) = vector_references()?;
        if one.0 == zero.0 && one.1 == zero.1 {
            for q in [0.25, 0.5, 0.75] {
                let geometry = produce_bounded(q)?;
                let (primary, check) = vector_pair(&geometry, canvas, is_cancelled)?;
                if primary != zero.0 || check != zero.1 {
                    return Err(EvaluationError::new(
                        "coverage.response.range",
                        "variable geometry has no monotone endpoint coverage range",
                    ));
                }
            }
            return Ok(None);
        }
    }
    {
        let mut slot = cached.lock().map_err(|_| {
            EvaluationError::new("coverage.response.cache", "probe cache lock was poisoned")
        })?;
        let backend = if forced_vector {
            Backend::Vector
        } else {
            Backend::Raster
        };
        slot.insert(
            0.0_f64.to_bits(),
            Sample {
                q: 0.0,
                primary: 0.0,
                check: 0.0,
                backend,
                resolution: 0,
            },
        );
        slot.insert(
            1.0_f64.to_bits(),
            Sample {
                q: 1.0,
                primary: 1.0,
                check: 1.0,
                backend,
                resolution: 0,
            },
        );
    }
    let vector_from_geometry =
        |q: f64, geometry: &GeometryOutput| -> Result<Sample, EvaluationError> {
            let (primary, check) = vector_pair(geometry, canvas, is_cancelled)?;
            let (zero, one) = vector_references()?;
            let first = normalized_value(primary, zero.0, one.0)?;
            let second = normalized_value(check, zero.1, one.1)?;
            if disagreement(first, second) {
                return Err(EvaluationError::new(
                    "coverage.response.vector_precision",
                    "vector measurements do not converge at the retained probe",
                ));
            }
            Ok(Sample {
                q,
                primary: first,
                check: second,
                backend: Backend::Vector,
                resolution: 0,
            })
        };
    let measure_vector = |q: f64| -> Result<Sample, EvaluationError> {
        if is_cancelled() {
            return Err(EvaluationError::new(
                "evaluation.cancelled",
                "coverage probe was cancelled",
            ));
        }
        vector_from_geometry(q, &produce_bounded(q)?)
    };
    let measure = |q: f64| -> Result<Sample, EvaluationError> {
        if is_cancelled() {
            return Err(EvaluationError::new(
                "evaluation.cancelled",
                "coverage probe was cancelled",
            ));
        }
        if let Some(sample) = cached
            .lock()
            .map_err(|_| {
                EvaluationError::new("coverage.response.cache", "probe cache lock was poisoned")
            })?
            .get(&q.to_bits())
            .copied()
        {
            return Ok(sample);
        }
        let geometry = produce_bounded(q)?;
        let sample = if forced_vector {
            vector_from_geometry(q, &geometry)?
        } else {
            let coarse = raster_normalized(&geometry, 2048)?;
            let fine = raster_normalized(&geometry, 2713)?;
            if !raster_uncertain(coarse, fine, q) {
                Sample {
                    q,
                    primary: coarse,
                    check: fine,
                    backend: Backend::Raster,
                    resolution: 2713,
                }
            } else {
                // High-resolution retries serialize, bounding simultaneous 64-Mpixel masks
                // while retaining the caller's four-worker budget for ordinary probes.
                let _guard = high_resolution_gate.lock().map_err(|_| {
                    EvaluationError::new(
                        "coverage.response.workers",
                        "high-resolution gate was poisoned",
                    )
                })?;
                let next = raster_normalized(&geometry, 5426)?;
                let first_delta = (fine - coarse).abs();
                let next_delta = (next - fine).abs();
                if !raster_uncertain(fine, next, q) && next_delta <= first_delta * 0.5 {
                    Sample {
                        q,
                        primary: next,
                        check: fine,
                        backend: Backend::Raster,
                        resolution: 5426,
                    }
                } else if next_delta < first_delta && target(canvas, 10852).is_ok() {
                    let last = raster_normalized(&geometry, 10852)?;
                    let last_delta = (last - next).abs();
                    if !raster_uncertain(next, last, q) && last_delta <= next_delta * 0.5 {
                        Sample {
                            q,
                            primary: last,
                            check: next,
                            backend: Backend::Raster,
                            resolution: 10852,
                        }
                    } else {
                        vector_from_geometry(q, &geometry)?
                    }
                } else {
                    vector_from_geometry(q, &geometry)?
                }
            }
        };
        let mut slot = cached.lock().map_err(|_| {
            EvaluationError::new("coverage.response.cache", "probe cache lock was poisoned")
        })?;
        if slot.len() >= MAX_CACHED_SAMPLES {
            return Err(EvaluationError::new(
                "coverage.response.work_limit",
                "coverage response exceeds its bounded cached measurement count",
            ));
        }
        slot.insert(q.to_bits(), sample);
        Ok(sample)
    };
    let batch = |qs: &[f64]| -> Result<Vec<Sample>, EvaluationError> {
        let ordered = pool.install(|| qs.par_iter().map(|&q| measure(q)).collect::<Vec<_>>());
        ordered.into_iter().collect()
    };
    let mut table = batch(&SEEDS)?;
    repair_phase_reversals(&mut table, &measure_vector)?;
    let zero = table[0];
    let one = table[table.len() - 1];
    if one.primary == zero.primary && one.check == zero.check {
        return Ok(None);
    }
    checked_curve(&table)?;
    let mut pending = table
        .windows(2)
        .map(|pair| (pair[0], pair[1], 0_u8))
        .collect::<Vec<_>>();
    while !pending.is_empty() {
        // Passing midpoint checks do not consume retained-knot capacity. The separate cached-q
        // and geometry-evaluation limits bound their cost across all rounds and repair phases.
        let qs = pending
            .iter()
            .map(|(left, right, _)| (left.q + right.q) / 2.0)
            .collect::<Vec<_>>();
        let midpoints = batch(&qs)?;
        let mut next = Vec::new();
        for ((left, right, depth), middle) in pending.into_iter().zip(midpoints) {
            let low = normalized(left, zero, one)?;
            let high = normalized(right, zero, one)?;
            let actual = normalized(middle, zero, one)?;
            if actual + 0.005 < low || high + 0.005 < actual {
                return Err(EvaluationError::new(
                    "coverage.response.monotone",
                    "measured occupancy is materially nonmonotone",
                ));
            }
            let miss = (actual - (low + high) / 2.0).abs();
            let tolerance = 0.0025_f64.min(0.1 * actual.min(1.0 - actual).max(0.0005));
            if miss > tolerance {
                if depth >= MAX_REFINEMENT_DEPTH {
                    return Err(EvaluationError::new(
                        "coverage.response.resolution",
                        "bounded curve retains a material interpolation error",
                    ));
                }
                if table.len() >= MAX_PROBES {
                    return Err(EvaluationError::new(
                        "coverage.response.probe_limit",
                        "coverage response needs more than the bounded probe count",
                    ));
                }
                table.push(middle);
                next.push((left, middle, depth + 1));
                next.push((middle, right, depth + 1));
            }
        }
        pending = next;
    }
    table.sort_by(|a, b| a.q.total_cmp(&b.q));
    repair_phase_reversals(&mut table, &measure_vector)?;
    let mut heldout_cache = BTreeMap::<(bool, u64), Sample>::new();
    for round in 0..=2 {
        repair_phase_reversals(&mut table, &measure_vector)?;
        let curve = checked_curve(&table)?;
        let mixed = table.iter().any(|sample| sample.backend == Backend::Vector);
        let mut failing = Vec::new();
        let (reference_zero, reference_one) = if mixed {
            (
                Sample {
                    q: 0.0,
                    primary: 0.0,
                    check: 0.0,
                    backend: Backend::Vector,
                    resolution: 0,
                },
                Sample {
                    q: 1.0,
                    primary: 1.0,
                    check: 1.0,
                    backend: Backend::Vector,
                    resolution: 0,
                },
            )
        } else {
            let (z0, z1) = raster_pair(&zero_geometry, canvas, [2591, 3371], is_cancelled)?;
            let (o0, o1) = raster_pair(&one_geometry, canvas, [2591, 3371], is_cancelled)?;
            (
                Sample {
                    q: 0.0,
                    primary: z0,
                    check: z1,
                    backend: Backend::Raster,
                    resolution: 3371,
                },
                Sample {
                    q: 1.0,
                    primary: o0,
                    check: o1,
                    backend: Backend::Raster,
                    resolution: 3371,
                },
            )
        };
        let inverse_qs = WITNESSES
            .iter()
            .map(|&target| {
                curve
                    .inverse(target)
                    .map_err(EvaluationError::from_pipeline)
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Independent heldouts use their own consumer targets, never the fit-knot cache. The
        // existing four-worker pool measures them concurrently and collects them in target order.
        let heldouts = pool.install(|| {
            inverse_qs
                .par_iter()
                .map(|&q| {
                    if let Some(sample) = heldout_cache.get(&(mixed, q.to_bits())) {
                        return Ok(*sample);
                    }
                    if mixed {
                        measure_vector(q)
                    } else {
                        let geometry = produce_bounded(q)?;
                        let (primary, check) =
                            raster_pair(&geometry, canvas, [2591, 3371], is_cancelled)?;
                        Ok(Sample {
                            q,
                            primary,
                            check,
                            backend: Backend::Raster,
                            resolution: 3371,
                        })
                    }
                })
                .collect::<Vec<_>>()
        });
        for ((target, q), sample) in WITNESSES.into_iter().zip(inverse_qs).zip(heldouts) {
            // An independently sampled target remains a valid witness even for exact-linear
            // curves whose inverse lands on a retained q knot.
            let sample = sample?;
            // A repeated inverse q on the same independent consumer is deterministic across
            // local repair rounds; keep its heldout measurement separate from fit-knot samples.
            if heldout_cache.len() >= MAX_HELDOUT_SAMPLES
                && !heldout_cache.contains_key(&(mixed, q.to_bits()))
            {
                return Err(EvaluationError::new(
                    "coverage.response.work_limit",
                    "coverage response exceeds its bounded heldout measurement count",
                ));
            }
            heldout_cache.insert((mixed, q.to_bits()), sample);
            let actual = normalized(sample, reference_zero, reference_one)?;
            let check_range = reference_one.check - reference_zero.check;
            let checked = if check_range > 0.0 {
                (sample.check - reference_zero.check) / check_range
            } else {
                f64::NAN
            };
            let tolerance = 0.0025_f64.min(0.1 * target.min(1.0 - target).max(0.0005));
            if !checked.is_finite()
                || raster_uncertain(actual, checked, q)
                || (actual - target).abs() > tolerance
            {
                failing.push(q);
            }
        }
        if failing.is_empty() {
            if is_cancelled() {
                return Err(EvaluationError::new(
                    "evaluation.cancelled",
                    "coverage fit was cancelled",
                ));
            }
            return Ok(Some(fitted(curve, &table)));
        }
        if round == 2 {
            return Err(EvaluationError::new(
                "coverage.response.heldout",
                "independent heldout remains unresolved after local vector repair",
            ));
        }
        let mut repairs = Vec::new();
        for q in failing {
            if let Some(index) = table.iter().position(|sample| sample.q == q) {
                repairs.push(q);
                if index > 0 {
                    repairs.extend([table[index - 1].q, (table[index - 1].q + q) / 2.0]);
                }
                if index + 1 < table.len() {
                    repairs.extend([(q + table[index + 1].q) / 2.0, table[index + 1].q]);
                }
                continue;
            }
            let pair = table
                .windows(2)
                .find(|pair| pair[0].q < q && q < pair[1].q)
                .ok_or(EvaluationError::new(
                    "coverage.response.heldout",
                    "failed heldout falls outside its fit interval",
                ))?;
            repairs.extend([pair[0].q, (pair[0].q + pair[1].q) / 2.0, pair[1].q]);
        }
        repairs.sort_by(f64::total_cmp);
        repairs.dedup();
        let added = repairs
            .iter()
            .filter(|&&q| !table.iter().any(|sample| sample.q == q))
            .count();
        if table.len() + added > MAX_PROBES {
            return Err(EvaluationError::new(
                "coverage.response.probe_limit",
                "coverage response needs more than the bounded probe count",
            ));
        }
        let samples = pool.install(|| {
            repairs
                .par_iter()
                .map(|&q| measure_vector(q))
                .collect::<Vec<_>>()
        });
        for sample in samples {
            let sample = sample?;
            let q = sample.q;
            if let Some(existing) = table.iter_mut().find(|existing| existing.q == q) {
                *existing = sample;
            } else {
                table.push(sample);
            }
        }
        table.sort_by(|a, b| a.q.total_cmp(&b.q));
    }
    Err(EvaluationError::new(
        "coverage.response.heldout",
        "coverage heldout validation did not complete",
    ))
}

#[cfg(test)]
mod tests {
    use super::{Backend, Sample, checked_curve, repair_phase_reversals};

    /// Accepts independent heldout measurements whose inverse coincides with a retained knot.
    ///
    /// # Panics
    /// Panics if an exactly linear measured response loses its seed-coincident inverse.
    #[test]
    fn exact_linear_heldout_targets_can_equal_fit_knots() {
        let samples = [0.0, 0.01, 0.25, 0.5, 0.75, 1.0].map(|q| Sample {
            q,
            primary: q,
            check: q,
            backend: Backend::Raster,
            resolution: 2713,
        });
        let curve = checked_curve(&samples).unwrap();
        for target in [0.01, 0.25, 0.5, 0.75] {
            let q = curve.inverse(target).unwrap();
            assert!(samples.iter().any(|sample| sample.q == q));
        }
    }

    /// Replaces a small phase reversal with a converged independent measurement.
    #[test]
    fn local_phase_reversal_rechecks_without_flattening() {
        let mut samples = [
            Sample {
                q: 0.0,
                primary: 0.0,
                check: 0.0,
                backend: Backend::Raster,
                resolution: 0,
            },
            Sample {
                q: 0.25,
                primary: 0.251,
                check: 0.251,
                backend: Backend::Raster,
                resolution: 2713,
            },
            Sample {
                q: 0.5,
                primary: 0.250,
                check: 0.252,
                backend: Backend::Raster,
                resolution: 2713,
            },
            Sample {
                q: 1.0,
                primary: 1.0,
                check: 1.0,
                backend: Backend::Raster,
                resolution: 0,
            },
        ];
        repair_phase_reversals(&mut samples, &|q| {
            Ok(Sample {
                q,
                primary: q,
                check: q,
                backend: Backend::Vector,
                resolution: 0,
            })
        })
        .unwrap();
        assert_eq!(samples[1].backend, Backend::Vector);
        assert_eq!(samples[2].backend, Backend::Vector);
        assert_eq!(checked_curve(&samples).unwrap().inverse(0.5).unwrap(), 0.5);
    }

    /// Rejects a real vector-measured reversal instead of changing retained occupancies.
    #[test]
    fn local_phase_reversal_rejects_persistent_nonmonotonicity() {
        let mut samples = [
            Sample {
                q: 0.0,
                primary: 0.0,
                check: 0.0,
                backend: Backend::Raster,
                resolution: 0,
            },
            Sample {
                q: 0.25,
                primary: 0.251,
                check: 0.251,
                backend: Backend::Raster,
                resolution: 2713,
            },
            Sample {
                q: 0.5,
                primary: 0.250,
                check: 0.252,
                backend: Backend::Raster,
                resolution: 2713,
            },
            Sample {
                q: 1.0,
                primary: 1.0,
                check: 1.0,
                backend: Backend::Raster,
                resolution: 0,
            },
        ];
        let error = repair_phase_reversals(&mut samples, &|q| {
            Ok(Sample {
                q,
                primary: if q == 0.25 { 0.3 } else { 0.2 },
                check: if q == 0.25 { 0.3 } else { 0.2 },
                backend: Backend::Vector,
                resolution: 0,
            })
        })
        .unwrap_err();
        assert_eq!(error.path(), "coverage.response.monotone");
    }
}
