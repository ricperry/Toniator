//! Explicit fixed-reference measurements for immutable saved calibration artifacts.
//!
//! This module does not participate in ordinary preview, render, or cache publication.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use toniator_domain::{
    CALIBRATION_PROFILE, CALIBRATION_PROFILE_64, CALIBRATION_PROFILE_GUIDE_FACES,
    CALIBRATION_PROFILE_GUIDE_FACES_64, CALIBRATION_PROFILE_SEEDED,
    CALIBRATION_PROFILE_SEEDED_64, CALIBRATION_PROFILE_ROUND_INTERIOR,
    CALIBRATION_REFERENCE_CANVAS_SIDE, CalibrationBinding,
    CalibrationKnot, CalibrationVariation, ChannelId, DensityMetric2D, Document, DocumentCommand,
    MarkDensityMode, PatternDefinitionRecipe, PatternGeometryResponse, PatternStructureRecipe,
    MAX_SAVED_CALIBRATION_KNOTS, SavedCalibration, SourceReference, SourceReferenceId, SourceWeightingFieldEdit,
    UniformCalibration,
};
use toniator_patterns::{GeometryResponseInput, PatternPipelinePlan, TypedFamilyOutput};
use toniator_render::{
    GeometryOutput, OutputRasterTarget, RasterizationLimits, VectorOccupancyPrecision,
    measure_geometry_vector_occupancy, rasterize_geometry_occupancy,
};

use crate::{
    CanvasSpec, EvaluationError, EvaluationLimits, GridInspectRequest, SourceField,
    coverage_output, ordered_output_bindings, required_support_radius_for_outputs, uniform_density,
};

/// Versioned 32-spacing reference profile on a fixed white field.
pub const REFERENCE_PROFILE: &str = CALIBRATION_PROFILE;
/// Fixed covered-interior canvas side for eligible finite Round Spiral measurements.
const ROUND_INTERIOR_CANVAS_SIDE: f64 = 768.0;
/// Largest raw geometry response admitted by current authored response bounds.
pub const RAW_SIZE_MAXIMUM: f64 = 2.0;
/// Fixed measurement positions, retaining low response and both sides of common authored bounds.
pub const RAW_SIZE_PROBES: [f64; 12] = [
    0.0, 0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0,
];
/// Maximum retained raw knots; midpoint checks have their own derived work ceiling.
const MAX_RAW_KNOTS: usize = MAX_SAVED_CALIBRATION_KNOTS;
/// Bounds raw logical-size probes, including passing midpoint checks; a mean probe visits both frozen populations.
const MAX_RAW_MEASUREMENTS: usize = 2 * MAX_RAW_KNOTS - 1;

/// Exact scalar vector observations reusable after strict transfer, without caching geometry.
#[derive(Default)]
struct ReferenceMeasurementCache {
    points: HashMap<(u32, u8, u64), RawCoveragePoint>,
}

impl ReferenceMeasurementCache {
    /// Reuses one exact side, size, and precision observation while honoring cancellation.
    ///
    /// # Errors
    /// Returns canonical producer or vector-consumer diagnostics without caching failures.
    fn measure(
        &mut self,
        probe: &FrozenCoverageProbe,
        side: u32,
        size: f64,
        precision: VectorOccupancyPrecision,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<RawCoveragePoint, EvaluationError> {
        if is_cancelled() {
            return Err(cancelled());
        }
        let kind = match precision {
            VectorOccupancyPrecision::Baseline => 0,
            VectorOccupancyPrecision::Tight => 1,
        };
        let key = (side, kind, size.to_bits());
        if let Some(point) = self.points.get(&key) {
            return Ok(*point);
        }
        let point = probe.measure_at(size, precision, is_cancelled)?;
        self.points.insert(key, point);
        Ok(point)
    }
}
/// One unnormalized opaque-union measurement and its bounded vector work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawCoveragePoint {
    pub size: f64,
    pub coverage: f64,
    pub vertices: usize,
    pub elapsed: Duration,
}

/// Validated raw coverage measurements, independent of a receiving response interval.
#[derive(Clone, Debug)]
pub struct RawCoverageCurve {
    points: Vec<RawCoveragePoint>,
}

impl RawCoverageCurve {
    /// Validates ordered raw measurements without sorting or smoothing physical response.
    /// An exact one-ULP reversal after coverage reaches one is normalized to saturation.
    ///
    /// # Errors
    /// Rejects missing endpoints, nonfinite values, out-of-range coverage, and any measured
    /// reversal; a failed calibration cannot masquerade as a plateau.
    pub fn new(mut points: Vec<RawCoveragePoint>) -> Result<Self, EvaluationError> {
        if points.len() < 2
            || points.first().is_none_or(|point| point.size != 0.0)
            || points
                .last()
                .is_none_or(|point| point.size != RAW_SIZE_MAXIMUM)
        {
            return Err(EvaluationError::new(
                "reference.curve.endpoints",
                "raw coverage requires exact size-zero and size-two endpoints",
            ));
        }
        for index in 0..points.len() {
            if index > 0
                && points[index - 1].coverage == 1.0
                && points[index].coverage == f64::from_bits(1.0_f64.to_bits() - 1)
            {
                points[index].coverage = 1.0;
            }
            let point = &points[index];
            if !point.size.is_finite()
                || !point.coverage.is_finite()
                || !(0.0..=RAW_SIZE_MAXIMUM).contains(&point.size)
                || !(0.0..=1.0).contains(&point.coverage)
                || (index > 0
                    && (point.size <= points[index - 1].size
                        || point.coverage < points[index - 1].coverage))
            {
                return Err(EvaluationError::new(
                    "reference.curve.monotone",
                    "raw coverage measurements are invalid or nonmonotone",
                ));
            }
        }
        Ok(Self { points })
    }

    /// Returns immutable measured knots without exposing mutation of the validated sequence.
    pub fn points(&self) -> &[RawCoveragePoint] {
        &self.points
    }

    /// Interpolates raw coverage only inside the measured size domain.
    ///
    /// # Errors
    /// Rejects a nonfinite or out-of-domain size.
    pub fn coverage_at(&self, size: f64) -> Result<f64, EvaluationError> {
        if !size.is_finite() || !(0.0..=RAW_SIZE_MAXIMUM).contains(&size) {
            return Err(EvaluationError::new(
                "reference.curve.size",
                "raw size must be finite and within zero through two",
            ));
        }
        let upper = self.points.partition_point(|point| point.size < size);
        if let Some(point) = self.points.get(upper)
            && point.size == size
        {
            return Ok(point.coverage);
        }
        let left = self.points[upper - 1];
        let right = self.points[upper];
        let fraction = (size - left.size) / (right.size - left.size);
        Ok(left.coverage + fraction * (right.coverage - left.coverage))
    }

    /// Inverts one receiving interval's endpoint-normalized raw coverage.
    ///
    /// Exact tones zero and one return authored bounds. A proven flat interval uses linear size
    /// interpolation because every response in it has identical measured coverage. Positive
    /// interior tones on a nonflat interval choose the first attainable size.
    ///
    /// # Errors
    /// Rejects invalid bounds or tone and an inconsistent measured crossing.
    pub fn inverse_interval(
        &self,
        minimum: f64,
        maximum: f64,
        tone: f64,
    ) -> Result<f64, EvaluationError> {
        if !minimum.is_finite()
            || !maximum.is_finite()
            || !tone.is_finite()
            || !(0.0..=RAW_SIZE_MAXIMUM).contains(&minimum)
            || !(minimum..=RAW_SIZE_MAXIMUM).contains(&maximum)
            || !(0.0..=1.0).contains(&tone)
        {
            return Err(EvaluationError::new(
                "reference.curve.interval",
                "response interval and tone must be finite and ordered within their bounds",
            ));
        }
        if tone == 0.0 {
            return Ok(minimum);
        }
        if tone == 1.0 {
            return Ok(maximum);
        }
        let low = self.coverage_at(minimum)?;
        let high = self.coverage_at(maximum)?;
        if high == low {
            return Ok(minimum + tone * (maximum - minimum));
        }
        let target = low + tone * (high - low);
        let mut left_size = minimum;
        let mut left_coverage = low;
        for point in self
            .points
            .iter()
            .filter(|point| point.size > minimum && point.size < maximum)
        {
            if point.coverage >= target {
                return Ok(left_size
                    + (target - left_coverage) / (point.coverage - left_coverage)
                        * (point.size - left_size));
            }
            left_size = point.size;
            left_coverage = point.coverage;
        }
        if high < target || high == left_coverage {
            return Err(EvaluationError::new(
                "reference.curve.inverse",
                "target coverage has no measured interval crossing",
            ));
        }
        Ok(left_size + (target - left_coverage) / (high - left_coverage) * (maximum - left_size))
    }
}

/// One output producer whose complete family and dependent topology are frozen before probes.
pub struct FrozenCoverageProbe {
    document: Document,
    channel_id: ChannelId,
    source: SourceField,
    family: TypedFamilyOutput,
    plan: PatternPipelinePlan,
    capability: toniator_patterns::OutputCapability,
    setting: toniator_domain::EffectivePatternOutputSettings,
    effective: toniator_domain::EffectiveChannelPatternInstance,
    definition: toniator_domain::PatternDefinition,
    mapping: toniator_domain::SourceMapping,
    prepared: Option<coverage_output::PreparedCoverageOutput>,
    limits: EvaluationLimits,
}

/// Non-persisted Uniform mark baseline and its validated survival inverse.
pub struct UniformReferenceFit {
    fit: uniform_density::UniformDensityFit,
    pub radius_per_nominal_spacing: f64,
    pub full_coverage: f64,
    pub radius_probes: usize,
    pub heldout_max_absolute_error: f64,
    pub heldout_max_relative_error: f64,
}

impl FrozenCoverageProbe {
    /// Generates one complete max-size-support family and its dependent topology, without fitting.
    ///
    /// The caller supplies a validated document with neutral coverage bounds 0..1 and one
    /// decoded source field. Typed raw probes still retain full size-two support on this family.
    ///
    /// # Errors
    /// Rejects a missing/compound output, nonneutral bounds, invalid family, resource limit,
    /// producer failure, or cancellation before exposing any probe.
    pub fn new(
        document: &Document,
        source: SourceField,
        channel_id: ChannelId,
        limits: EvaluationLimits,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Self, EvaluationError> {
        if is_cancelled() {
            return Err(cancelled());
        }
        let channel = document
            .channel_topology()
            .and_then(|topology| {
                topology
                    .channels()
                    .iter()
                    .find(|channel| channel.id == channel_id)
            })
            .ok_or(EvaluationError::new(
                "reference.channel",
                "selected modeled channel is missing",
            ))?;
        let effective = document
            .effective_channel_pattern(channel_id)
            .map_err(EvaluationError::from_domain)?;
        let definition = document
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.id == effective.definition_id)
            .map(|bundle| bundle.definition.clone())
            .ok_or(EvaluationError::new(
                "reference.definition",
                "selected Pattern definition is missing",
            ))?;
        let mut plan = toniator_patterns::resolve_document_pattern_pipeline_for_channel(
            document,
            &definition,
            channel.weighting,
        )
        .map_err(EvaluationError::from_pipeline)?;
        let bindings = ordered_output_bindings(&effective, &plan)?;
        let [(capability, setting)] = bindings.as_slice() else {
            return Err(EvaluationError::new(
                "reference.output",
                "reference calibration requires one drawing output",
            ));
        };
        if !coverage_bounds(&setting.response) {
            return Err(EvaluationError::new(
                "reference.bounds",
                "reference output must have neutral desired coverage bounds zero through one",
            ));
        }
        let capability = (*capability).clone();
        let setting = (*setting).clone();
        let uniform = capability.marks().is_some()
            && document
                .effective_mark_density(channel_id, capability.layer_id)
                .map_err(EvaluationError::from_domain)?
                .mode
                == MarkDensityMode::Uniform;
        if uniform && let Some(random) = &mut plan.family.random {
            random.full_population_for_uniform_marks = true;
            random.density_modulation =
                toniator_patterns::ResolvedSiteDensityModulation::ArtworkWeightedUnbound;
        }
        let request = GridInspectRequest {
            canvas: document.canvas().clone(),
            density: effective.resolved_density,
            rotation_degrees: effective.pattern_rotation_degrees,
            translation_x: effective.translation_x,
            translation_y: effective.translation_y,
            guard_steps: crate::curve_motif_phase_guard_steps(
                definition.coverage.guard_steps,
                &plan.ordered_outputs,
            )?,
            support_radius: required_support_radius_for_outputs(
                document.canvas(),
                &effective,
                &definition,
                &plan.family,
                &plan.ordered_outputs,
            )?,
            max_family_candidates: limits.max_family_candidates(),
        };
        let family =
            toniator_patterns::evaluate_typed_family_product_with_source_progress_cancellable(
                &plan.family,
                &request,
                toniator_patterns::family_requires_decoded_source(&plan.family).then_some(&source),
                is_cancelled,
                &|_, _| {},
            )
            .map_err(EvaluationError::from_pipeline)?;
        let prepared = coverage_output::prepare(
            document,
            &definition,
            &effective,
            &family,
            &plan,
            &capability,
            limits,
            is_cancelled,
        )?;
        Ok(Self {
            document: document.clone(),
            channel_id,
            source,
            family,
            plan,
            capability,
            setting,
            effective,
            definition,
            mapping: channel.mapping,
            prepared,
            limits,
        })
    }

    /// Returns the exact frozen family fingerprint shared by every later size probe.
    pub fn family_fingerprint(&self) -> &str {
        self.family.family_fingerprint()
    }

    /// Counts the frozen canonical family sites without rerunning or altering placement.
    pub fn family_site_count(&self) -> usize {
        self.family.site_set().sites().len()
    }

    /// Fits the existing 99.9% Uniform footprint on this source-independent frozen family.
    ///
    /// The planned support reserves scale two; the returned dimensionless radius transfers at
    /// scale one using the receiving family's resolved nominal spacing. No artwork weighting
    /// enters this baseline or its probability inverse.
    ///
    /// # Errors
    /// Rejects non-Uniform/non-mark output or an unreachable/unstable bounded footprint fit.
    pub fn fit_uniform_reference(
        &self,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<UniformReferenceFit, EvaluationError> {
        let mode = self
            .document
            .effective_mark_density(self.channel_id, self.capability.layer_id)
            .map_err(EvaluationError::from_domain)?
            .mode;
        if self.capability.marks().is_none() || mode != MarkDensityMode::Uniform {
            return Err(EvaluationError::new(
                "reference.uniform.mode",
                "Uniform reference requires an eligible Uniform mark output",
            ));
        }
        let fit = uniform_density::fit(
            self.document.canvas(),
            self.family.planned_support_radius() / 2.0,
            &|radius| self.produce_uniform(&self.family, radius, true, is_cancelled),
            is_cancelled,
        )?;
        let spacing = self.nominal_spacing()?;
        let result = UniformReferenceFit {
            radius_per_nominal_spacing: fit.radius / spacing,
            full_coverage: fit.full_coverage,
            radius_probes: fit.radius_probes,
            heldout_max_absolute_error: fit.heldout_max_absolute_error,
            heldout_max_relative_error: fit.heldout_max_relative_error,
            fit,
        };
        Ok(result)
    }

    /// Measures stable Uniform site thinning at one constant tone on a receiving frozen family.
    ///
    /// This isolates baseline and survival transfer from the source's spatial tone variation.
    /// The exact current mark-selection hash and canonical mark producer remain authoritative.
    ///
    /// # Errors
    /// Returns inverse, thinning, support, producer, raster, or cancellation diagnostics.
    pub fn measure_uniform_constant_tone(
        &self,
        fit: &UniformReferenceFit,
        tone: f64,
        long_side: u32,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<RawCoveragePoint, EvaluationError> {
        if !tone.is_finite() || !(0.0..=1.0).contains(&tone) {
            return Err(EvaluationError::new(
                "reference.uniform.tone",
                "Uniform tone must be normalized",
            ));
        }
        let started = Instant::now();
        let probability = fit.fit.probability(tone)?;
        let selected = self
            .family
            .thinned_for_uniform_marks(
                self.channel_id.0 ^ self.capability.layer_id.0,
                &|_| Ok(probability),
                is_cancelled,
            )
            .map_err(EvaluationError::from_pipeline)?;
        let radius = fit.radius_per_nominal_spacing * self.nominal_spacing()?;
        if radius > self.family.planned_support_radius() {
            return Err(EvaluationError::new(
                "reference.uniform.support",
                "transferred radius exceeds frozen family support",
            ));
        }
        let geometry = self.produce_uniform(&selected, radius, false, is_cancelled)?;
        let canvas = self.document.canvas();
        let axis = canvas.width.max(canvas.height);
        let width = ((canvas.width / axis) * f64::from(long_side))
            .round()
            .max(1.0) as u32;
        let height = ((canvas.height / axis) * f64::from(long_side))
            .round()
            .max(1.0) as u32;
        let target =
            OutputRasterTarget::new(width, height).map_err(EvaluationError::from_render)?;
        let coverage = rasterize_geometry_occupancy(
            &geometry,
            canvas,
            target,
            RasterizationLimits::default(),
            is_cancelled,
        )
        .map_err(EvaluationError::from_render)?
        .fraction();
        Ok(RawCoveragePoint {
            size: tone,
            coverage,
            vertices: selected.site_set().sites().len(),
            elapsed: started.elapsed(),
        })
    }

    /// Reads Uniform Scatter's physical spacing from the canonical producer's cell basis.
    ///
    /// This experimental normalization must use the same Feature-scaled authored scale as
    /// population and mark realization; canvas-derived guide frequency is not Scatter spacing.
    ///
    /// # Errors
    /// Rejects an ineligible non-Scatter family or an invalid producer cell basis.
    fn nominal_spacing(&self) -> Result<f64, EvaluationError> {
        if self.plan.family.random.is_none() {
            return Err(EvaluationError::new(
                "reference.uniform.family",
                "Uniform reference spacing requires a Scatter family",
            ));
        }
        let diameter = toniator_patterns::maximum_nominal_cell_diameter(
            &self.plan.family,
            self.document.canvas(),
            &self.effective.resolved_density,
        )
        .map_err(EvaluationError::from_pipeline)?;
        Ok(diameter / 2.0_f64.sqrt())
    }

    /// Realizes one Uniform footprint through the existing canonical mark producer.
    ///
    /// # Errors
    /// Returns shape, support, transformed-segment, or cancellation diagnostics.
    fn produce_uniform(
        &self,
        family: &TypedFamilyOutput,
        radius: f64,
        probe: bool,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<GeometryOutput, EvaluationError> {
        let mut one_output = self.plan.clone();
        one_output.ordered_outputs = vec![self.capability.clone()];
        one_output.evaluation_order = vec![self.capability.layer_id];
        let input = if probe {
            GeometryResponseInput::UniformRadiusProbe(radius)
        } else {
            GeometryResponseInput::UniformRadius(radius)
        };
        let output =
            toniator_patterns::realize_typed_canonical_mark_output_with_response_cancellable(
                &self.document,
                family,
                &one_output,
                &self.capability,
                &self.setting,
                &self.source,
                self.document.canvas(),
                self.mapping,
                false,
                self.effective.shape_rotation_degrees,
                self.limits.max_transformed_curve_segment_instances(),
                input,
                is_cancelled,
            )
            .map_err(EvaluationError::from_pipeline)?;
        Ok(GeometryOutput::CanonicalMarks(
            output.realization.output.marks,
        ))
    }

    /// Retains only midpoint knots needed to explain the raw 0..2 coverage curve.
    ///
    /// Passing checks still consume a separate bounded measurement count. Error and reversal
    /// tolerances scale with reachable coverage, so sparse recipes cannot pass on a permissive
    /// full-canvas absolute error. Only exact saturation roundoff is normalized after fitting.
    ///
    /// # Errors
    /// Returns a producer/consumer error, nonmonotone response, or explicit knot, work, or
    /// floating-point midpoint diagnostic when interpolation cannot fit the bounded envelope.
    pub fn fit_raw_curve(
        &self,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<RawCoverageCurve, EvaluationError> {
        let mut measured = 0_usize;
        let mut measure = |size| {
            if measured >= MAX_RAW_MEASUREMENTS {
                return Err(EvaluationError::new(
                    "reference.curve.work_limit",
                    "raw coverage needs more than the bounded measurement count",
                ));
            }
            measured += 1;
            self.measure_at(size, VectorOccupancyPrecision::Baseline, is_cancelled)
        };
        fit_raw_curve_from_measure(&mut measure)
    }

    /// Retains a raw curve while a caller reuses exact scalar observations from prior attempts.
    ///
    /// # Errors
    /// Preserves the common knot, logical measurement, monotonicity, and interpolation gates.
    fn fit_raw_curve_with_measure(
        &self,
        measure: &mut impl FnMut(f64) -> Result<RawCoveragePoint, EvaluationError>,
    ) -> Result<RawCoverageCurve, EvaluationError> {
        fit_raw_curve_from_measure(measure)
    }
}

/// Fits actual same-size observations from one or more frozen populations under the shared cap.
///
/// # Errors
/// Rejects nonmonotone, unresolved, or over-budget measured curves without smoothing samples.
fn fit_raw_curve_from_measure(
    measure: &mut impl FnMut(f64) -> Result<RawCoveragePoint, EvaluationError>,
) -> Result<RawCoverageCurve, EvaluationError> {
    let diagnostics = std::env::var_os("TONIATOR_CALIBRATION_DIAGNOSTICS").is_some();
    let mut retained = RAW_SIZE_PROBES
        .iter()
        .map(|&size| measure(size))
        .collect::<Result<Vec<_>, _>>()?;
    let mut measured = retained.len();
    let minimum_coverage = retained[0].coverage;
    let maximum_coverage = retained.last().unwrap().coverage;
    let span = maximum_coverage - minimum_coverage;
    let mut pending = retained
        .windows(2)
        .map(|pair| (pair[0], pair[1], 0_usize))
        .collect::<Vec<_>>();
    while !pending.is_empty() {
        let mut next = Vec::new();
        let pending_count = pending.len();
        for (left, right, depth) in pending {
            let midpoint = (left.size + right.size) / 2.0;
            if midpoint <= left.size || midpoint >= right.size {
                return Err(EvaluationError::new(
                    "reference.curve.precision_limit",
                    "raw coverage interval has no representable interior midpoint",
                ));
            }
            let middle = measure(midpoint)?;
            measured += 1;
            if middle.coverage + 0.005 * span < left.coverage
                || right.coverage + 0.005 * span < middle.coverage
            {
                if std::env::var_os("TONIATOR_CALIBRATION_DIAGNOSTICS").is_some() {
                    eprintln!(
                        "reference reversal: interval=[{:.6},{:.6}] coverage=[{:.9},{:.9}] middle={:.6} coverage={:.9} span={span:.9} allowed={:.9}",
                        left.size,
                        right.size,
                        left.coverage,
                        right.coverage,
                        middle.size,
                        middle.coverage,
                        0.005 * span,
                    );
                }
                return Err(EvaluationError::new(
                    "reference.curve.monotone",
                    "raw coverage is materially nonmonotone",
                ));
            }
            let predicted = (left.coverage + right.coverage) / 2.0;
            let edge = (middle.coverage - minimum_coverage)
                .min(maximum_coverage - middle.coverage)
                .max(0.0001 * span);
            let tolerance = (0.0001 * span).min(0.02 * edge);
            if diagnostics {
                eprintln!(
                    "reference interval: left={:.17} middle={:.17} right={:.17} coverage=[{:.17},{:.17},{:.17}] residual={:.17} tolerance={:.17} depth={depth} split={} retained={} measured={measured} pending={pending_count}",
                    left.size,
                    middle.size,
                    right.size,
                    left.coverage,
                    middle.coverage,
                    right.coverage,
                    (middle.coverage - predicted).abs(),
                    tolerance,
                    (middle.coverage - predicted).abs() > tolerance,
                    retained.len(),
                );
            }
            if (middle.coverage - predicted).abs() > tolerance {
                if retained.len() >= MAX_RAW_KNOTS {
                    return Err(EvaluationError::new(
                        "reference.curve.knot_limit",
                        "raw coverage needs more than the bounded retained knot count",
                    ));
                }
                retained.push(middle);
                next.push((left, middle, depth + 1));
                next.push((middle, right, depth + 1));
            }
        }
        pending = next;
    }
    retained.sort_by(|left, right| left.size.total_cmp(&right.size));
    RawCoverageCurve::new(retained)
}

impl FrozenCoverageProbe {
    /// Realizes one raw response on the frozen family and measures its opaque vector union.
    ///
    /// Size enters an explicit transient `RawProbe(size)` beside valid coverage settings;
    /// canonical geometry and its complete off-canvas topology remain unchanged.
    ///
    /// # Errors
    /// Returns producer, cancellation, flattened-edge, Boolean, or precision diagnostics.
    pub fn measure_at(
        &self,
        size: f64,
        precision: VectorOccupancyPrecision,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<RawCoveragePoint, EvaluationError> {
        if !size.is_finite() || !(0.0..=RAW_SIZE_MAXIMUM).contains(&size) {
            return Err(EvaluationError::new(
                "reference.size",
                "raw size must be within zero through two",
            ));
        }
        if is_cancelled() {
            return Err(cancelled());
        }
        let started = Instant::now();
        let geometry = self.produce(size / RAW_SIZE_MAXIMUM, is_cancelled)?;
        let measured = measure_geometry_vector_occupancy(
            &geometry,
            self.document.canvas(),
            precision,
            RasterizationLimits::default(),
            is_cancelled,
        )
        .map_err(EvaluationError::from_render)?;
        if std::env::var_os("TONIATOR_CALIBRATION_DIAGNOSTICS").is_some() {
            eprintln!(
                "reference measurement: size={size:.17} precision={precision:?} coverage={:.17} vertices={} seconds={:.6}",
                measured.fraction,
                measured.vertices,
                started.elapsed().as_secs_f64(),
            );
        }
        Ok(RawCoveragePoint {
            size,
            coverage: measured.fraction,
            vertices: measured.vertices,
            elapsed: started.elapsed(),
        })
    }

    /// Measures one frozen raw response through the existing bounded binary raster consumer.
    ///
    /// This cheaper transfer witness is explicitly resolution dependent; it cannot replace the
    /// independent vector reference measurements or certify optical antialiasing.
    ///
    /// # Errors
    /// Returns invalid size, target, canonical producer, allocation, edge, or cancellation errors.
    pub fn measure_raster_at(
        &self,
        size: f64,
        long_side: u32,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<RawCoveragePoint, EvaluationError> {
        if !size.is_finite() || !(0.0..=RAW_SIZE_MAXIMUM).contains(&size) {
            return Err(EvaluationError::new(
                "reference.size",
                "raw size must be within zero through two",
            ));
        }
        if is_cancelled() {
            return Err(cancelled());
        }
        let canvas = self.document.canvas();
        let axis = canvas.width.max(canvas.height);
        let width = ((canvas.width / axis) * f64::from(long_side))
            .round()
            .max(1.0) as u32;
        let height = ((canvas.height / axis) * f64::from(long_side))
            .round()
            .max(1.0) as u32;
        let target =
            OutputRasterTarget::new(width, height).map_err(EvaluationError::from_render)?;
        let started = Instant::now();
        let geometry = self.produce(size / RAW_SIZE_MAXIMUM, is_cancelled)?;
        let fraction = rasterize_geometry_occupancy(
            &geometry,
            canvas,
            target,
            RasterizationLimits::default(),
            is_cancelled,
        )
        .map_err(EvaluationError::from_render)?
        .fraction();
        Ok(RawCoveragePoint {
            size,
            coverage: fraction,
            vertices: 0,
            elapsed: started.elapsed(),
        })
    }

    /// Realizes one canonical output through the same typed producers as current calibration.
    ///
    /// # Errors
    /// Returns current binding, shape, outline, treatment, or cancellation diagnostics.
    fn produce(
        &self,
        normalized: f64,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<GeometryOutput, EvaluationError> {
        if let Some(prepared) = &self.prepared {
            return prepared.probe(
                &self.capability,
                &self.setting,
                &self.source,
                self.document.canvas(),
                self.mapping,
                self.limits,
                normalized,
                is_cancelled,
            );
        }
        let mut one_output = self.plan.clone();
        one_output.ordered_outputs = vec![self.capability.clone()];
        one_output.evaluation_order = vec![self.capability.layer_id];
        let input = GeometryResponseInput::RawProbe(normalized * RAW_SIZE_MAXIMUM);
        let authored = self
            .definition
            .output_layers
            .iter()
            .find(|output| output.id == self.capability.layer_id)
            .ok_or(EvaluationError::new(
                "reference.output",
                "authored output is missing",
            ))?;
        match &authored.realization {
            toniator_domain::PatternOutputRealization::MarkPrototype { .. } => {
                let output = toniator_patterns::realize_typed_canonical_mark_output_with_response_cancellable(
                    &self.document, &self.family, &one_output, &self.capability, &self.setting,
                    &self.source, self.document.canvas(), self.mapping, false,
                    self.effective.shape_rotation_degrees,
                    self.limits.max_transformed_curve_segment_instances(), input, is_cancelled,
                ).map_err(EvaluationError::from_pipeline)?;
                Ok(GeometryOutput::CanonicalMarks(
                    output.realization.output.marks,
                ))
            }
            toniator_domain::PatternOutputRealization::CircularMarks { .. } => {
                let output =
                    toniator_patterns::realize_typed_mapped_output_with_response_cancellable(
                        &self.family,
                        &one_output,
                        &self.capability,
                        &self.setting,
                        &self.source,
                        self.document.canvas(),
                        self.mapping,
                        self.effective.shape_rotation_degrees,
                        input,
                        is_cancelled,
                    )
                    .map_err(EvaluationError::from_pipeline)?;
                Ok(GeometryOutput::CircularMarks(
                    output.realization.output.marks,
                ))
            }
            _ => {
                let output = toniator_patterns::realize_typed_canonical_stroke_output_with_response_cancellable(
                    &self.family, &one_output, &self.capability, &self.setting, &self.source,
                    self.document.canvas(), self.mapping,
                    self.limits.max_stroke_profile_samples(),
                    self.limits.max_stroke_outline_segments(), input, is_cancelled,
                ).map_err(EvaluationError::from_pipeline)?;
                Ok(GeometryOutput::CanonicalStrokes(
                    output.realization.output.strokes,
                ))
            }
        }
    }
}

/// Builds a source-independent fixed reference with neutral desired coverage bounds.
///
/// This stage-local constructor does not persist the document or make it a new product authority.
/// A fixed 1024-square canvas holds authored absolute-length ratios constant while density 32
/// or 64 supplies the nominal-spacing observation count. It retains structural seeds/resources
/// and uses an exact white, opaque 64-square sampling field with neutral channel weighting.
/// All output topology is generated off-canvas.
///
/// # Errors
/// Returns current recipe, density, response, source, or document validation diagnostics.
pub fn reference_document(
    recipe: &PatternDefinitionRecipe,
    side: u32,
) -> Result<(Document, SourceField, ChannelId), EvaluationError> {
    reference_document_with_geometry(recipe, side, 1.0, 0.0)
}

/// Builds the existing fixed 1024-square reference for one aspect and shape orientation.
///
/// Scalar Density is still the reference-window frequency; receiving Feature changes never
/// alter the saved binding. Whole-Pattern rotation and translation remain source-independent.
///
/// # Errors
/// Propagates current recipe, aspect, shape, response, source, or document validation errors.
fn reference_document_with_geometry(
    recipe: &PatternDefinitionRecipe,
    side: u32,
    density_aspect: f64,
    shape_rotation_degrees: f64,
) -> Result<(Document, SourceField, ChannelId), EvaluationError> {
    reference_document_with_canvas(
        recipe,
        CALIBRATION_REFERENCE_CANVAS_SIDE,
        side,
        density_aspect,
        shape_rotation_degrees,
    )
}

/// Builds one fixed reference canvas without changing the recipe or density interpretation.
///
/// # Errors
/// Propagates current recipe, aspect, shape, response, source, or document validation errors.
fn reference_document_with_canvas(
    recipe: &PatternDefinitionRecipe,
    canvas_side: f64,
    side: u32,
    density_aspect: f64,
    shape_rotation_degrees: f64,
) -> Result<(Document, SourceField, ChannelId), EvaluationError> {
    if !matches!(side, 32 | 64 | 128) {
        return Err(EvaluationError::new(
            "reference.window",
            "reference window must be 32, 64, or 128 nominal spacings",
        ));
    }
    let canvas = CanvasSpec {
        width: canvas_side,
        height: canvas_side,
    };
    let source_id = SourceReferenceId::new("ton018-reference-white-v1")
        .map_err(EvaluationError::from_domain)?;
    let mut document = Document::new_default_document(canvas, SourceReference::Assigned(source_id))
        .map_err(EvaluationError::from_domain)?;
    let base = document.pattern_settings().clone();
    let base_definition = document.pattern_definition_bundles()[0].definition.clone();
    document = document
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: recipe.clone(),
        })
        .map_err(EvaluationError::from_domain)?
        .0;
    let base = document.pattern_settings().clone();
    let mut settings = base.clone();
    settings.density = DensityMetric2D {
        density: f64::from(side),
        aspect: density_aspect,
    };
    settings.shape_rotation_degrees = shape_rotation_degrees;
    document = document
        .apply_command(&DocumentCommand::SetDocumentPatternSettings { base, settings })
        .map_err(EvaluationError::from_domain)?
        .0;
    let base = document.response_settings().clone();
    let mut settings = base.clone();
    settings.marks.minimum_fill = 0.0;
    settings.marks.maximum_fill = 1.0;
    settings.connected_minimum_thickness = 0.0;
    settings.connected_maximum_thickness = 1.0;
    settings.regions_minimum_fill = 0.0;
    settings.regions_maximum_fill = 1.0;
    settings.mark_density_mode = MarkDensityMode::Variable;
    document = document
        .apply_command(&DocumentCommand::SetAllResponseSettings { base, settings })
        .map_err(EvaluationError::from_domain)?
        .0;
    let channel_id = document
        .channel_topology()
        .and_then(|topology| topology.channels().first())
        .map(|channel| channel.id)
        .ok_or(EvaluationError::new(
            "reference.channel",
            "reference RGB channel is missing",
        ))?;
    document = document
        .apply_command(&DocumentCommand::SetSourceWeightingField {
            channel_id,
            edit: SourceWeightingFieldEdit::Strength(0.0),
        })
        .map_err(EvaluationError::from_domain)?
        .0;
    let source = SourceField::from_straight_rgba8(64, 64, vec![255; 64 * 64 * 4])
        .map_err(EvaluationError::from_sampling)?;
    Ok((document, source, channel_id))
}

/// Fits raw 0..2 coverage on a frozen reference and checks an independent doubled population.
///
/// This explicit experiment returns measured data only; no ordinary evaluation or preset uses it.
///
/// # Errors
/// Rejects any family, producer, consumer, cancellation, nonmonotone curve, or independent
/// reference-window discrepancy exceeding 0.25% of reachable occupied fraction, or a
/// low-tone inverse witness exceeding the bounded absolute and relative tone tolerances.
pub fn calibrate_reference_pattern(
    recipe: &PatternDefinitionRecipe,
    limits: EvaluationLimits,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(RawCoverageCurve, f64, Duration, Duration), EvaluationError> {
    let mut cache = ReferenceMeasurementCache::default();
    let (curve, error, prepare, elapsed, _) =
        calibrate_reference_geometry(recipe, 1.0, 0.0, limits, &mut cache, false, is_cancelled)?;
    Ok((curve, error, prepare, elapsed))
}

/// Explicitly measures one concrete channel Pattern and returns a reusable immutable artifact.
///
/// The reference uses a fixed 1024-square white source. It begins at 32 nominal spacings with
/// independent 64-spacing witnesses, retrying at 64/128 only for population convergence. It
/// retains channel-effective aspect and shape orientation while leaving
/// scalar Feature size, artwork, whole-Pattern placement, and Advanced response out of the fit.
/// Eligible marks include Uniform footprint and survival data even when Variable is selected.
/// Ordinary evaluation never calls this function.
///
/// # Errors
/// Returns typed reconstruction, bounded measurement, Uniform, cancellation, or validation errors
/// without modifying the supplied document.
pub fn calibrate_saved_pattern(
    document: &Document,
    channel_id: ChannelId,
    limits: EvaluationLimits,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<SavedCalibration, EvaluationError> {
    let binding = CalibrationBinding::for_channel(document, channel_id)
        .map_err(EvaluationError::from_domain)?;
    calibrate_saved_binding(&binding, limits, is_cancelled)
}

/// Measures a typed shared-baseline or active-channel binding without synthesizing a channel.
///
/// Strict 32/64 and 64/128 transfers retain precedence. After both convergence failures, only
/// an eligible finite Round Spiral uses a 768-square 64/128 covered-interior reference; typed
/// stochastic recipes retain their separate bounded mean. Guide Faces use a distinct strict
/// producer profile at either tier; all profiles preserve the binding.
///
/// # Errors
/// Preserves strict fit, inverse, producer, cancellation, interior, and mean diagnostics.
pub fn calibrate_saved_binding(
    binding: &CalibrationBinding,
    limits: EvaluationLimits,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<SavedCalibration, EvaluationError> {
    let recipe = reference_recipe_from_binding(binding);
    let mut cache = ReferenceMeasurementCache::default();
    let strict = calibrate_reference_geometry(
        &recipe,
        binding.density_aspect(),
        binding.shape_rotation_degrees(),
        limits,
        &mut cache,
        binding.has_variation_seed(),
        is_cancelled,
    );
    let (raw, reference_side, reference_canvas_side, interior, mean) = match strict {
        Ok((raw, _, _, _, side)) => (raw, side, CALIBRATION_REFERENCE_CANVAS_SIDE, false, None),
        Err(error)
            if error.path() == "reference.window.convergence"
                && binding.supports_round_interior() =>
        {
            // ReferenceMeasurementCache keys spacing tier, size, and precision; a separate
            // canvas cannot share observations with the preceding 1024-square strict attempts.
            let mut interior_cache = ReferenceMeasurementCache::default();
            let (raw, _, _, _) = calibrate_reference_geometry_at_side(
                &recipe,
                (ROUND_INTERIOR_CANVAS_SIDE, binding.density_aspect(), binding.shape_rotation_degrees()),
                64,
                limits,
                &mut interior_cache,
                false,
                is_cancelled,
            )?;
            (raw, 64, ROUND_INTERIOR_CANVAS_SIDE, true, None)
        }
        Err(error)
            if error.path() == "reference.window.convergence"
                && binding.supports_stochastic_mean() =>
        {
            let evidence = calibrate_mean_reference_geometry(
                &recipe,
                binding.density_aspect(),
                binding.shape_rotation_degrees(),
                limits,
                &mut cache,
                is_cancelled,
            )?;
            (evidence.curve.clone(), 0, CALIBRATION_REFERENCE_CANVAS_SIDE, false, Some(evidence))
        }
        Err(error) => return Err(error),
    };
    let (uniform, uniform_transfer) = if mean.is_some() {
        fit_uniform_mean(&recipe, binding, limits, is_cancelled)?
    } else {
        (
            fit_uniform_at_side(
                &recipe, binding, reference_canvas_side, reference_side, limits, is_cancelled,
            )?,
            None,
        )
    };
    let knots = raw
        .points()
        .iter()
        .map(|point| CalibrationKnot {
            input: point.size,
            coverage: point.coverage,
        })
        .collect();
    if let Some(mean) = mean {
        let variation = CalibrationVariation::new(
            mean.raw_absolute,
            mean.raw_fraction_of_span,
            mean.tone_absolute,
            mean.low_tone_relative,
            uniform_transfer.map(|value| value.0),
            uniform_transfer.map(|value| value.1),
        )
        .map_err(EvaluationError::from_domain)?;
        SavedCalibration::with_mean_profile(binding.clone(), knots, uniform, variation)
            .map_err(EvaluationError::from_domain)
    } else {
        let profile = if interior {
            CALIBRATION_PROFILE_ROUND_INTERIOR
        } else if binding.uses_guide_faces() {
            if reference_side == 64 {
                CALIBRATION_PROFILE_GUIDE_FACES_64
            } else {
                CALIBRATION_PROFILE_GUIDE_FACES
            }
        } else if binding.has_variation_seed() {
            if reference_side == 64 {
                CALIBRATION_PROFILE_SEEDED_64
            } else {
                CALIBRATION_PROFILE_SEEDED
            }
        } else if reference_side == 64 {
            CALIBRATION_PROFILE_64
        } else {
            CALIBRATION_PROFILE
        };
        SavedCalibration::with_profile(binding.profile_for_reference(profile), binding.clone(), knots, uniform)
            .map_err(EvaluationError::from_domain)
    }
}

/// Supplies a presentation-only name required by document materialization after binding normalization.
///
/// Saved bindings deliberately erase family labels; this clone restores no structural field and
/// never changes the persisted binding or measured geometry.
fn reference_recipe_from_binding(binding: &CalibrationBinding) -> PatternDefinitionRecipe {
    /// Restores only an empty family label through current typed wrapper structures.
    fn label(structure: &mut PatternStructureRecipe) {
        match structure {
            PatternStructureRecipe::StraightGrid(draft) => {
                if draft.name.is_empty() {
                    draft.name = "Reference geometry".into();
                }
            }
            PatternStructureRecipe::GeneralizedStraightGuides { name, .. }
            | PatternStructureRecipe::GenericGuides { name, .. }
            | PatternStructureRecipe::RandomSites { name, .. }
            | PatternStructureRecipe::ParametricCurve { name, .. } => {
                if name.is_empty() {
                    *name = "Reference geometry".into();
                }
            }
            PatternStructureRecipe::AuthoredResources { definition, .. }
            | PatternStructureRecipe::ConnectionPaths { definition, .. }
            | PatternStructureRecipe::MazeWalls { definition, .. }
            | PatternStructureRecipe::AuthoredClosedShapeMarks { definition, .. }
            | PatternStructureRecipe::CurveMotifPaths { definition, .. }
            | PatternStructureRecipe::VoronoiRegions { definition }
            | PatternStructureRecipe::GuideFaceRegions { definition, .. }
            | PatternStructureRecipe::OrderedOutputs { definition, .. } => label(definition),
        }
    }
    let mut recipe = binding.recipe().clone();
    label(&mut recipe.structure);
    recipe
}

/// Prepares one optional Uniform mark population without changing the raw reference response.
///
/// # Errors
/// Propagates current document, density eligibility, producer, and cancellation diagnostics.
fn uniform_reference_probe(
    recipe: &PatternDefinitionRecipe,
    binding: &CalibrationBinding,
    canvas_side: f64,
    side: u32,
    limits: EvaluationLimits,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<FrozenCoverageProbe>, EvaluationError> {
    let (reference, source, reference_channel) = reference_document_with_canvas(
        recipe,
        canvas_side,
        side,
        binding.density_aspect(),
        binding.shape_rotation_degrees(),
    )?;
    let base = reference.response_settings().clone();
    let mut settings = base.clone();
    settings.mark_density_mode = MarkDensityMode::Uniform;
    let reference = reference
        .apply_command(&DocumentCommand::SetAllResponseSettings { base, settings })
        .map_err(EvaluationError::from_domain)?
        .0;
    let eligible = reference
        .effective_channel_pattern(reference_channel)
        .map_err(EvaluationError::from_domain)?
        .output_settings
        .first()
        .filter(|output| matches!(output.response, PatternGeometryResponse::Marks(_)))
        .map(|output| reference.effective_mark_density(reference_channel, output.output_layer_id))
        .transpose()
        .map_err(EvaluationError::from_domain)?
        .is_some_and(|density| density.mode == MarkDensityMode::Uniform);
    if !eligible {
        return Ok(None);
    }
    FrozenCoverageProbe::new(&reference, source, reference_channel, limits, is_cancelled).map(Some)
}

/// Fits the ordinary single-tier Uniform payload for a strict deterministic reference.
///
/// # Errors
/// Preserves the existing radius, 99.9% full-tone, and held-out probability gates.
fn fit_uniform_at_side(
    recipe: &PatternDefinitionRecipe,
    binding: &CalibrationBinding,
    canvas_side: f64,
    side: u32,
    limits: EvaluationLimits,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<UniformCalibration>, EvaluationError> {
    let Some(probe) = uniform_reference_probe(recipe, binding, canvas_side, side, limits, is_cancelled)? else {
        return Ok(None);
    };
    let fit = probe.fit_uniform_reference(is_cancelled)?;
    UniformCalibration::new(
        fit.radius_per_nominal_spacing,
        fit.fit
            .inverse_knots()
            .iter()
            .map(|knot| CalibrationKnot {
                input: knot.response,
                coverage: knot.coverage,
            })
            .collect(),
    )
    .map(Some)
    .map_err(EvaluationError::from_domain)
}

/// Retains measured mean coverage and the independent raw and inverse transfer maxima.
struct MeanReferenceEvidence {
    curve: RawCoverageCurve,
    raw_absolute: f64,
    raw_fraction_of_span: f64,
    tone_absolute: f64,
    low_tone_relative: f64,
}

/// Fits actual 32+64 same-size occupied fractions after both strict transfers fail.
///
/// The 255-probe ceiling counts paired logical sizes; each member is measured from its own frozen
/// canonical family. Tight shadows retain strict interpolation/inverse quality, while an
/// independent 128 population supplies the approved empirical transfer bound.
///
/// # Errors
/// Rejects non-stochastic, over-budget, nonmonotone, inaccurate, or cancelled measurements.
fn calibrate_mean_reference_geometry(
    recipe: &PatternDefinitionRecipe,
    density_aspect: f64,
    shape_rotation_degrees: f64,
    limits: EvaluationLimits,
    cache: &mut ReferenceMeasurementCache,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<MeanReferenceEvidence, EvaluationError> {
    if !CalibrationBinding::new(recipe.clone(), density_aspect, shape_rotation_degrees)
        .map_err(EvaluationError::from_domain)?
        .supports_stochastic_mean()
    {
        return Err(EvaluationError::new(
            "reference.mean.eligibility",
            "average reference requires a typed stochastic population",
        ));
    }
    let (d32, s32, c32) =
        reference_document_with_geometry(recipe, 32, density_aspect, shape_rotation_degrees)?;
    let p32 = FrozenCoverageProbe::new(&d32, s32, c32, limits, is_cancelled)?;
    let (d64, s64, c64) =
        reference_document_with_geometry(recipe, 64, density_aspect, shape_rotation_degrees)?;
    let p64 = FrozenCoverageProbe::new(&d64, s64, c64, limits, is_cancelled)?;
    let mut measured = 0_usize;
    let curve = fit_raw_curve_from_measure(&mut |size| {
        if measured >= MAX_RAW_MEASUREMENTS {
            return Err(EvaluationError::new(
                "reference.curve.work_limit",
                "mean raw coverage needs more than the bounded logical measurement count",
            ));
        }
        measured += 1;
        let first = cache.measure(
            &p32,
            32,
            size,
            VectorOccupancyPrecision::Baseline,
            is_cancelled,
        )?;
        let second = cache.measure(
            &p64,
            64,
            size,
            VectorOccupancyPrecision::Baseline,
            is_cancelled,
        )?;
        Ok(RawCoveragePoint {
            size,
            coverage: (first.coverage + second.coverage) / 2.0,
            vertices: first.vertices.saturating_add(second.vertices),
            elapsed: first.elapsed.saturating_add(second.elapsed),
        })
    })?;
    let span = curve.points().last().unwrap().coverage - curve.points()[0].coverage;
    let mut tight_mean = |size| -> Result<f64, EvaluationError> {
        let first = cache.measure(
            &p32,
            32,
            size,
            VectorOccupancyPrecision::Tight,
            is_cancelled,
        )?;
        let second = cache.measure(
            &p64,
            64,
            size,
            VectorOccupancyPrecision::Tight,
            is_cancelled,
        )?;
        Ok((first.coverage + second.coverage) / 2.0)
    };
    for size in [
        0.0007, 0.003, 0.007, 0.075, 0.175, 0.375, 0.625, 0.875, 1.25, 1.75,
    ] {
        let predicted = curve.coverage_at(size)?;
        let actual = tight_mean(size)?;
        let tone = (predicted - curve.points()[0].coverage) / span;
        let delta = (actual - predicted).abs();
        if delta > 0.0025 * span || delta > 0.1 * tone.min(1.0 - tone).max(0.0005) * span {
            return Err(EvaluationError::new(
                "reference.mean.interpolation",
                "paired-population Tight shadow exceeds bounded raw interpolation",
            ));
        }
    }
    for (minimum, maximum) in [(0.0, 2.0), (0.0, 1.5), (0.0, 1.0), (0.1, 1.2)] {
        let low = tight_mean(minimum)?;
        let high = tight_mean(maximum)?;
        if high <= low {
            return Err(EvaluationError::new(
                "reference.mean.range",
                "paired Tight response has no reachable interval",
            ));
        }
        for tone in [0.001_f64, 0.005, 0.01, 0.25, 0.5, 0.75] {
            let size = curve.inverse_interval(minimum, maximum, tone)?;
            let actual = (tight_mean(size)? - low) / (high - low);
            let delta = (actual - tone).abs();
            if delta > 0.0025 || delta > 0.1 * tone.max(0.0005) {
                return Err(EvaluationError::new(
                    "reference.mean.inverse",
                    "paired-population Tight inverse misses the bounded tone target",
                ));
            }
        }
    }
    drop(p32);
    drop(p64);
    let (d128, s128, c128) =
        reference_document_with_geometry(recipe, 128, density_aspect, shape_rotation_degrees)?;
    let p128 = FrozenCoverageProbe::new(&d128, s128, c128, limits, is_cancelled)?;
    let mut raw_absolute = 0.0_f64;
    for size in [0.0, 0.01, 0.1, 0.5, 1.0, 1.5, 2.0] {
        let check = cache.measure(
            &p128,
            128,
            size,
            VectorOccupancyPrecision::Tight,
            is_cancelled,
        )?;
        raw_absolute = raw_absolute.max((check.coverage - curve.coverage_at(size)?).abs());
    }
    if raw_absolute > 0.02 * span {
        return Err(EvaluationError::new(
            "reference.mean.population",
            "independent population exceeds two percent of reachable raw coverage",
        ));
    }
    let mut tone_absolute = 0.0_f64;
    let mut low_tone_relative = 0.0_f64;
    for (minimum, maximum) in [(0.0, 2.0), (0.0, 1.5), (0.0, 1.0), (0.1, 1.2)] {
        let low = cache
            .measure(
                &p128,
                128,
                minimum,
                VectorOccupancyPrecision::Tight,
                is_cancelled,
            )?
            .coverage;
        let high = cache
            .measure(
                &p128,
                128,
                maximum,
                VectorOccupancyPrecision::Tight,
                is_cancelled,
            )?
            .coverage;
        if high <= low {
            return Err(EvaluationError::new(
                "reference.mean.population_range",
                "independent population has no reachable interval",
            ));
        }
        for tone in [0.001_f64, 0.005, 0.01, 0.25, 0.5, 0.75] {
            let size = curve.inverse_interval(minimum, maximum, tone)?;
            let point = cache.measure(
                &p128,
                128,
                size,
                VectorOccupancyPrecision::Tight,
                is_cancelled,
            )?;
            let delta = ((point.coverage - low) / (high - low) - tone).abs();
            let relative = delta / tone.max(0.0005);
            tone_absolute = tone_absolute.max(delta);
            low_tone_relative = low_tone_relative.max(relative);
            if delta > 0.01 || relative > 0.1 {
                return Err(EvaluationError::new(
                    "reference.mean.population_inverse",
                    "independent population exceeds bounded normalized-tone variation",
                ));
            }
        }
    }
    Ok(MeanReferenceEvidence {
        curve,
        raw_absolute,
        raw_fraction_of_span: raw_absolute / span,
        tone_absolute,
        low_tone_relative,
    })
}

/// Optional paired Uniform calibration and independent empirical tone maxima.
type MeanUniformTransfer = (Option<UniformCalibration>, Option<(f64, f64)>);

/// Fits eligible Uniform marks from the same two frozen populations as the raw mean.
///
/// # Errors
/// Rejects insufficient common support, missing 99.9% full tone, failed independent 128
/// transfer, and all existing Uniform producer, held-out, or cancellation diagnostics.
fn fit_uniform_mean(
    recipe: &PatternDefinitionRecipe,
    binding: &CalibrationBinding,
    limits: EvaluationLimits,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<MeanUniformTransfer, EvaluationError> {
    let Some(p32) = uniform_reference_probe(recipe, binding, CALIBRATION_REFERENCE_CANVAS_SIDE, 32, limits, is_cancelled)? else {
        return Ok((None, None));
    };
    let p64 = uniform_reference_probe(recipe, binding, CALIBRATION_REFERENCE_CANVAS_SIDE, 64, limits, is_cancelled)?.ok_or(
        EvaluationError::new(
            "reference.mean.uniform",
            "Uniform eligibility differs between paired populations",
        ),
    )?;
    let p128 = uniform_reference_probe(recipe, binding, CALIBRATION_REFERENCE_CANVAS_SIDE, 128, limits, is_cancelled)?.ok_or(
        EvaluationError::new(
            "reference.mean.uniform",
            "Uniform eligibility differs at independent population",
        ),
    )?;
    let spacing32 = p32.nominal_spacing()?;
    let spacing64 = p64.nominal_spacing()?;
    let spacing128 = p128.nominal_spacing()?;
    let maximum_ratio = (p32.family.planned_support_radius() / (2.0 * spacing32))
        .min(p64.family.planned_support_radius() / (2.0 * spacing64))
        .min(p128.family.planned_support_radius() / (2.0 * spacing128));
    let fit = uniform_density::fit_populations(
        p32.document.canvas(),
        maximum_ratio,
        &|ratio| {
            Ok(vec![
                p32.produce_uniform(&p32.family, ratio * spacing32, true, is_cancelled)?,
                p64.produce_uniform(&p64.family, ratio * spacing64, true, is_cancelled)?,
            ])
        },
        Some(&|ratio| p128.produce_uniform(&p128.family, ratio * spacing128, true, is_cancelled)),
        is_cancelled,
    )?;
    drop(p32);
    drop(p64);
    let radius128 = fit.radius * spacing128;
    if std::env::var_os("TONIATOR_CALIBRATION_DIAGNOSTICS").is_some() {
        eprintln!(
            "mean Uniform transfer: radius_ratio={:.9} independent_radius={:.9} independent_support={:.9} paired_full={:.9} radius_probes={}",
            fit.radius,
            radius128,
            p128.family.planned_support_radius() / 2.0,
            fit.full_coverage,
            fit.radius_probes,
        );
    }
    if radius128 > p128.family.planned_support_radius() / 2.0 {
        return Err(EvaluationError::new(
            "reference.mean.uniform.support",
            "mean Uniform radius exceeds independent population support",
        ));
    }
    let geometry = p128.produce_uniform(&p128.family, radius128, true, is_cancelled)?;
    let transfer = uniform_density::check_population_transfer(
        p128.document.canvas(),
        &geometry,
        &fit,
        is_cancelled,
    )?;
    let uniform = UniformCalibration::new(
        fit.radius,
        fit.inverse_knots()
            .iter()
            .map(|knot| CalibrationKnot {
                input: knot.response,
                coverage: knot.coverage,
            })
            .collect(),
    )
    .map_err(EvaluationError::from_domain)?;
    Ok((Some(uniform), Some(transfer)))
}

/// Measures one exact structural reference with independent larger-window witnesses.
///
/// Seed-bearing saved bindings may reject an exact failed endpoint before completing a strict
/// curve; all inconclusive cases still use the original bounded full-fit path.
///
/// # Errors
/// Rejects a non-converged or nonmonotone raw fit, in addition to producer and cancellation errors.
fn calibrate_reference_geometry(
    recipe: &PatternDefinitionRecipe,
    density_aspect: f64,
    shape_rotation_degrees: f64,
    limits: EvaluationLimits,
    cache: &mut ReferenceMeasurementCache,
    early_endpoint: bool,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(RawCoverageCurve, f64, Duration, Duration, u32), EvaluationError> {
    match calibrate_reference_geometry_at_side(
        recipe,
        (CALIBRATION_REFERENCE_CANVAS_SIDE, density_aspect, shape_rotation_degrees),
        32,
        limits,
        cache,
        early_endpoint,
        is_cancelled,
    ) {
        Ok((curve, error, prepare, elapsed)) => Ok((curve, error, prepare, elapsed, 32)),
        Err(error) if error.path() == "reference.window.convergence" => {
            let (curve, error, prepare, elapsed) = calibrate_reference_geometry_at_side(
                recipe,
                (CALIBRATION_REFERENCE_CANVAS_SIDE, density_aspect, shape_rotation_degrees),
                64,
                limits,
                cache,
                early_endpoint,
                is_cancelled,
            )?;
            Ok((curve, error, prepare, elapsed, 64))
        }
        Err(error) => Err(error),
    }
}

/// Fits one fixed population and checks the doubled independent witness without changing its gate.
///
/// Stochastic saved calibration first checks the exact raw-size-two endpoint. A violation rejects
/// this strict candidate before adaptive fitting; inconclusive endpoints take the full path.
/// The reference tuple contains fixed canvas side, density aspect, and shape rotation in degrees;
/// none comes from receiving artwork.
///
/// # Errors
/// Preserves producer, bounded fitter, cancellation, convergence, and low-tone inverse diagnostics.
fn calibrate_reference_geometry_at_side(
    recipe: &PatternDefinitionRecipe,
    reference: (f64, f64, f64),
    side: u32,
    limits: EvaluationLimits,
    cache: &mut ReferenceMeasurementCache,
    early_endpoint: bool,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(RawCoverageCurve, f64, Duration, Duration), EvaluationError> {
    let (canvas_side, density_aspect, shape_rotation_degrees) = reference;
    let started = Instant::now();
    let (document, source, channel) =
        reference_document_with_canvas(recipe, canvas_side, side, density_aspect, shape_rotation_degrees)?;
    let probe = FrozenCoverageProbe::new(&document, source, channel, limits, is_cancelled)?;
    let prepare_time = started.elapsed();
    let early_check_probe = if early_endpoint {
        let (check_document, check_source, check_channel) = reference_document_with_canvas(
            recipe,
            canvas_side,
            side * 2,
            density_aspect,
            shape_rotation_degrees,
        )?;
        Some(FrozenCoverageProbe::new(
            &check_document,
            check_source,
            check_channel,
            limits,
            is_cancelled,
        )?)
    } else {
        None
    };
    if early_endpoint {
        let check_probe = early_check_probe
            .as_ref()
            .expect("stochastic early witness prepared");
        // The raw-size-two knot is exact. A failed doubled-population endpoint alone proves
        // this strict candidate cannot pass its unchanged transfer gate. Other raw checks are
        // not claimed to have passed; the bounded mean retains its own full fit and witnesses.
        let zero = cache.measure(
            &probe,
            side,
            0.0,
            VectorOccupancyPrecision::Baseline,
            is_cancelled,
        )?;
        let two = cache.measure(
            &probe,
            side,
            2.0,
            VectorOccupancyPrecision::Baseline,
            is_cancelled,
        )?;
        let check = cache.measure(
            check_probe,
            side * 2,
            2.0,
            VectorOccupancyPrecision::Tight,
            is_cancelled,
        )?;
        let span = two.coverage - zero.coverage;
        // Leave several ULPs for the existing exact-saturation endpoint normalization.
        if span > 0.0 && (check.coverage - two.coverage).abs() > 0.0025 * span + 8.0 * f64::EPSILON
        {
            if std::env::var_os("TONIATOR_CALIBRATION_DIAGNOSTICS").is_some() {
                eprintln!(
                    "reference convergence: side={side} check={} size=2.000000 error={:.9} span={span:.9} limit={:.9}",
                    side * 2,
                    (check.coverage - two.coverage).abs(),
                    0.0025 * span,
                );
            }
            return Err(EvaluationError::new(
                "reference.window.convergence",
                "independent doubled-population reference disagrees with the raw curve",
            ));
        }
    }
    let mut measured = 0_usize;
    let curve = probe.fit_raw_curve_with_measure(&mut |size| {
        if measured >= MAX_RAW_MEASUREMENTS {
            return Err(EvaluationError::new(
                "reference.curve.work_limit",
                "raw coverage needs more than the bounded measurement count",
            ));
        }
        measured += 1;
        cache.measure(
            &probe,
            side,
            size,
            VectorOccupancyPrecision::Baseline,
            is_cancelled,
        )
    })?;
    let check_probe = if let Some(probe) = early_check_probe {
        probe
    } else {
        let (check_document, check_source, check_channel) = reference_document_with_canvas(
            recipe,
            canvas_side,
            side * 2,
            density_aspect,
            shape_rotation_degrees,
        )?;
        FrozenCoverageProbe::new(
            &check_document,
            check_source,
            check_channel,
            limits,
            is_cancelled,
        )?
    };
    let mut maximum_error = 0.0_f64;
    let mut maximum_error_size = 0.0_f64;
    for size in [0.0, 0.01, 0.1, 0.5, 1.0, 1.5, 2.0] {
        let check = cache.measure(
            &check_probe,
            side * 2,
            size,
            VectorOccupancyPrecision::Tight,
            is_cancelled,
        )?;
        let error = (check.coverage - curve.coverage_at(size)?).abs();
        if error > maximum_error {
            maximum_error = error;
            maximum_error_size = size;
        }
    }
    let span = curve.points().last().unwrap().coverage - curve.points()[0].coverage;
    if maximum_error > 0.0025 * span {
        if std::env::var_os("TONIATOR_CALIBRATION_DIAGNOSTICS").is_some() {
            eprintln!(
                "reference convergence: side={side} check={} size={maximum_error_size:.6} error={maximum_error:.9} span={span:.9} limit={:.9}",
                side * 2,
                0.0025 * span
            );
        }
        return Err(EvaluationError::new(
            "reference.window.convergence",
            "independent doubled-population reference disagrees with the raw curve",
        ));
    }
    for (minimum, maximum) in [(0.0, 1.5), (0.1, 1.2)] {
        let low = cache.measure(
            &check_probe,
            side * 2,
            minimum,
            VectorOccupancyPrecision::Tight,
            is_cancelled,
        )?;
        let high = cache.measure(
            &check_probe,
            side * 2,
            maximum,
            VectorOccupancyPrecision::Tight,
            is_cancelled,
        )?;
        if high.coverage == low.coverage {
            continue;
        }
        for tone in [0.001_f64, 0.005, 0.01, 0.25, 0.5, 0.75] {
            let size = curve.inverse_interval(minimum, maximum, tone)?;
            let point = cache.measure(
                &check_probe,
                side * 2,
                size,
                VectorOccupancyPrecision::Tight,
                is_cancelled,
            )?;
            let actual = (point.coverage - low.coverage) / (high.coverage - low.coverage);
            let error = (actual - tone).abs();
            if error > 0.0025 || error > 0.1 * tone.max(0.0005) {
                #[cfg(test)]
                eprintln!(
                    "reference inverse: interval=[{minimum:.3},{maximum:.3}] tone={tone:.4} actual={actual:.6} size={size:.6} error={error:.6} relative_limit={:.6}",
                    0.1 * tone.max(0.0005)
                );
                return Err(EvaluationError::new(
                    "reference.window.inverse",
                    "independent low-tone response misses the bounded inverse target",
                ));
            }
        }
    }
    Ok((curve, maximum_error, prepare_time, started.elapsed()))
}

/// Returns the stable cancellation diagnostic before a partial reference result is exposed.
fn cancelled() -> EvaluationError {
    EvaluationError::new(
        "evaluation.cancelled",
        "reference calibration was cancelled",
    )
}

/// Recognizes neutral coverage intent before transient raw producer settings are derived.
fn coverage_bounds(response: &PatternGeometryResponse) -> bool {
    match response {
        PatternGeometryResponse::Marks(value) => {
            value.minimum_fill == 0.0 && value.maximum_fill == 1.0
        }
        PatternGeometryResponse::Connected(value) => {
            value.minimum_thickness == 0.0 && value.maximum_thickness == 1.0
        }
        PatternGeometryResponse::Regions(value) => {
            value.minimum_fill == 0.0 && value.maximum_fill == 1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FrozenCoverageProbe, RAW_SIZE_MAXIMUM, RawCoverageCurve, RawCoveragePoint,
        calibrate_saved_pattern, reference_document,
    };
    use std::time::Duration;
    use toniator_domain::{
        DocumentCommand, MarkDensityMode, PatternStructureRecipe, SiteDensityModulation,
        SiteExclusionPolicy,
    };
    use toniator_patterns::PresetRegistry;
    use toniator_render::VectorOccupancyPrecision;

    /// Materializes one bundled finite spiral on a concrete receiver before taking its exact
    /// source-free binding; the registry's CoverCanvas shortcut itself is not a saved binding.
    ///
    /// # Panics
    /// Panics if current bundled construction or channel identity is unavailable.
    fn concrete_spiral_binding(id: &str, canvas_side: f64) -> toniator_domain::CalibrationBinding {
        let recipe = PresetRegistry::bundled().entries().iter()
            .find(|entry| entry.metadata.id == id).unwrap().recipe.clone();
        let document = toniator_domain::Document::new_default_document(
            toniator_domain::CanvasSpec { width: canvas_side, height: canvas_side },
            toniator_domain::SourceReference::Unassigned,
        ).unwrap();
        let document = document.apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: document.pattern_settings().clone(),
            base_definition: document.pattern_definition_bundles()[0].definition.clone(),
            recipe,
        }).unwrap().0;
        toniator_domain::CalibrationBinding::for_channel(&document, toniator_domain::ChannelId(1)).unwrap()
    }

    /// Keeps covered-interior eligibility tied to typed finite geometry, full outer-revolution
    /// support, one ordinary output, and isotropic density rather than a Pattern label or ID.
    ///
    /// # Panics
    /// Panics if a square, repeated, anisotropic, or insufficient-support recipe gains eligibility.
    #[test]
    fn round_interior_eligibility_is_structural_and_bounded() {
        let line = concrete_spiral_binding("round-spiral-line", 900.0);
        let marks = concrete_spiral_binding("round-spiral-marks", 900.0);
        assert!(line.supports_round_interior());
        assert!(marks.supports_round_interior());
        assert!(!concrete_spiral_binding("square-spiral-marks", 900.0).supports_round_interior());
        assert!(concrete_spiral_binding("round-spiral-line", 1024.0).supports_round_interior());
        let mut recipe = line.recipe().clone();
        {
            let PatternStructureRecipe::OrderedOutputs { definition, .. } = &mut recipe.structure else { panic!("ordered spiral output"); };
            let PatternStructureRecipe::ParametricCurve { curve: toniator_domain::ParametricCurve::Spiral(spiral), .. } = definition.as_mut() else { panic!("finite spiral"); };
            spiral.turns = 34.0;
        }
        let insufficient = toniator_domain::CalibrationBinding::new(recipe.clone(), 1.0, 0.0).unwrap();
        assert!(!insufficient.supports_round_interior());
        let raw = vec![
            toniator_domain::CalibrationKnot { input: 0.0, coverage: 0.0 },
            toniator_domain::CalibrationKnot { input: 2.0, coverage: 1.0 },
        ];
        assert!(toniator_domain::SavedCalibration::with_profile(
            toniator_domain::CALIBRATION_PROFILE_ROUND_INTERIOR.to_owned(),
            insufficient, raw.clone(), None,
        ).is_err());
        {
            let PatternStructureRecipe::OrderedOutputs { definition, .. } = &mut recipe.structure else { unreachable!() };
            let PatternStructureRecipe::ParametricCurve { curve: toniator_domain::ParametricCurve::Spiral(spiral), .. } = definition.as_mut() else { unreachable!() };
            spiral.turns = 37.0;
        }
        let eligible = toniator_domain::CalibrationBinding::new(recipe.clone(), 1.0, 0.0).unwrap();
        assert!(eligible.supports_round_interior());
        assert!(toniator_domain::SavedCalibration::with_profile(
            toniator_domain::CALIBRATION_PROFILE_ROUND_INTERIOR.to_owned(),
            eligible, raw.clone(), None,
        ).is_ok());
        assert!(!toniator_domain::CalibrationBinding::new(recipe.clone(), 1.25, 0.0).unwrap().supports_round_interior());
        let PatternStructureRecipe::OrderedOutputs { definition, .. } = &mut recipe.structure else { unreachable!() };
        let PatternStructureRecipe::ParametricCurve { repetition, .. } = definition.as_mut() else { unreachable!() };
        *repetition = toniator_domain::CurveRepetition::TransformStack { direction_degrees: 90.0, spacing_multiplier: 1.0 };
        let repeated = toniator_domain::CalibrationBinding::new(recipe, 1.0, 0.0).unwrap();
        assert!(!repeated.supports_round_interior());
        assert!(toniator_domain::SavedCalibration::with_profile(
            toniator_domain::CALIBRATION_PROFILE_ROUND_INTERIOR.to_owned(),
            repeated, raw, None,
        ).is_err());
        let mut wrapped_recipe = line.recipe().clone();
        wrapped_recipe.structure = PatternStructureRecipe::AuthoredResources {
            resources: vec![toniator_domain::AuthoredStructureDraft::new(
                toniator_domain::AuthoredStructureKind::OpenPath,
                vec![toniator_domain::AuthoredCurveSegment::Line {
                    start: toniator_domain::AuthoredPoint2 { x: 0.0, y: 0.0 },
                    end: toniator_domain::AuthoredPoint2 { x: 1.0, y: 0.0 },
                }],
            ).unwrap()],
            definition: Box::new(wrapped_recipe.structure),
        };
        let wrapped = toniator_domain::CalibrationBinding::new(wrapped_recipe, 1.0, 0.0).unwrap();
        assert!(!wrapped.supports_round_interior());
        assert_eq!(super::calibrate_saved_binding(
            &line, crate::EvaluationLimits::default(), &|| true,
        ).unwrap_err().path(), "evaluation.cancelled");
    }

    /// Re-fits both active bundled Guide Faces recipes through every strict interpolation,
    /// inverse, population-transfer, and cancellation gate before trusting reissued profiles.
    ///
    /// # Panics
    /// Panics if a producer revision changes either complete 77-knot numeric payload, binding,
    /// profile, or optional Uniform authority without an explicit companion update.
    #[test]
    fn guide_faces_v2_builtin_profiles_match_complete_fits() {
        let registry = toniator_patterns::PresetRegistry::bundled();
        for id in ["three-guide-cells-scale", "two-guide-cells-uniform-offset"] {
            let record = registry.entries().iter().find(|record| record.metadata.id == id).unwrap();
            let retained = record.saved_calibration.as_ref().unwrap();
            assert_eq!(retained.profile(), toniator_domain::CALIBRATION_PROFILE_GUIDE_FACES);
            assert!(retained.binding().uses_guide_faces());
            assert_eq!(retained.raw().len(), 77);
            let fitted = super::calibrate_saved_binding(
                retained.binding(), crate::EvaluationLimits::default(), &|| false,
            ).unwrap();
            assert_eq!(fitted, *retained, "complete revised fit differs for {id}");
        }
    }

    /// Preserves successful 1024-square routes for the existing complete 47-turn Round Spiral
    /// and the 36-turn Square Spiral instead of retagging either saved numeric artifact.
    ///
    /// # Panics
    /// Panics if a previously converged strict companion changes profile or exact binding.
    #[test]
    fn existing_spiral_strict_profiles_remain_unchanged() {
        for (id, canvas_side, expected_profile) in [
            ("round-spiral-line", 1024.0, toniator_domain::CALIBRATION_PROFILE),
            ("square-spiral-marks", 900.0, toniator_domain::CALIBRATION_PROFILE_64),
        ] {
            let binding = concrete_spiral_binding(id, canvas_side);
            let saved = super::calibrate_saved_binding(&binding, crate::EvaluationLimits::default(), &|| false).unwrap();
            assert_eq!(saved.profile(), expected_profile);
            assert_eq!(saved.binding(), &binding);
        }
    }

    /// Measures another supported finite Round turn count without a number-specific branch.
    ///
    /// # Panics
    /// Panics if the complete 37-turn line cannot pass the same covered-interior gates.
    #[test]
    fn another_finite_round_turn_count_uses_covered_interior() {
        let binding = concrete_spiral_binding("round-spiral-line", 900.0);
        let mut recipe = binding.recipe().clone();
        let PatternStructureRecipe::OrderedOutputs { definition, .. } = &mut recipe.structure else { panic!("ordered spiral output"); };
        let PatternStructureRecipe::ParametricCurve { curve: toniator_domain::ParametricCurve::Spiral(spiral), .. } = definition.as_mut() else { panic!("finite spiral"); };
        spiral.turns = 37.0;
        let binding = toniator_domain::CalibrationBinding::new(recipe, 1.0, 0.0).unwrap();
        let saved = super::calibrate_saved_binding(&binding, crate::EvaluationLimits::default(), &|| false).unwrap();
        assert_eq!(saved.profile(), toniator_domain::CALIBRATION_PROFILE_ROUND_INTERIOR);
        assert_eq!(saved.binding(), &binding);
    }

    /// Proves the strict 1024 tiers precede the independent 768/64/128 fallback for both concrete
    /// Fixed36 outputs; a shared strict/interior cache would fail these exact measurements.
    ///
    /// # Panics
    /// Panics if either output loses its source-free profile, binding, complete range, or low tones.
    #[test]
    fn fixed36_round_outputs_save_covered_interior_curves() {
        for id in ["round-spiral-line", "round-spiral-marks"] {
            let binding = concrete_spiral_binding(id, 900.0);
            let saved = super::calibrate_saved_binding(&binding, crate::EvaluationLimits::default(), &|| false).unwrap();
            assert_eq!(saved.profile(), toniator_domain::CALIBRATION_PROFILE_ROUND_INTERIOR);
            assert_eq!(saved.binding(), &binding);
            assert_eq!(saved.raw().first().unwrap().input, 0.0);
            assert_eq!(saved.raw().last().unwrap().input, 2.0);
            assert!(saved.raw().iter().any(|knot| knot.input == 0.001 && knot.coverage > 0.0));
            assert!(saved.uniform().is_none());
        }
    }

    /// Measures a concrete built-in Pattern once and attaches its validated raw result.
    ///
    /// # Panics
    /// Panics if the fixed reference fails convergence or changes the stored profile/knots.
    /// Coverage stays numerically equivalent within absolute 1e-12: the same geometric profile
    /// can round differently when Boolean union and clipping share one operation. This test
    /// bound does not change fitter, inverse, transfer, or area acceptance gates or stored bytes.
    /// Input comparison retains its original 1e-15 serialization bound; zero/full membership
    /// stays exact at every knot so absolute inverse plateaus retain their authority.
    #[test]
    fn saved_default_calibration_is_attachable() {
        let document = toniator_domain::Document::new_default_document(
            toniator_domain::CanvasSpec {
                width: 128.0,
                height: 128.0,
            },
            toniator_domain::SourceReference::Unassigned,
        )
        .unwrap();
        let saved = calibrate_saved_pattern(
            &document,
            toniator_domain::ChannelId(1),
            crate::EvaluationLimits::default(),
            &|| false,
        )
        .unwrap();
        if let Ok(directory) = std::env::var("TONIATOR_CALIBRATION_GENERATE_DIR") {
            let path = std::path::Path::new(&directory).join("default-straight-current.csv");
            let csv = saved
                .raw()
                .iter()
                .map(|knot| format!("{:.17},{:.17}\n", knot.input, knot.coverage))
                .collect::<String>();
            std::fs::write(path, csv).unwrap();
        }
        assert!(saved.raw().len() > 12);
        assert!(saved.uniform().is_none());
        let checked_in = document
            .saved_calibration_for(toniator_domain::ChannelId(1))
            .unwrap()
            .unwrap();
        assert_eq!(saved.profile(), checked_in.profile());
        assert_eq!(saved.binding(), checked_in.binding());
        assert_eq!(saved.raw().len(), checked_in.raw().len());
        assert_eq!(saved.raw().first(), checked_in.raw().first());
        assert_eq!(saved.raw().last(), checked_in.raw().last());
        for (measured, stored) in saved.raw().iter().zip(checked_in.raw()) {
            assert!((measured.input - stored.input).abs() <= 1e-15);
            assert_eq!(measured.coverage == 0.0, stored.coverage == 0.0);
            assert_eq!(measured.coverage == 1.0, stored.coverage == 1.0);
            assert!(
                (measured.coverage - stored.coverage).abs() <= 1e-12,
                "{}: measured={} stored={}",
                measured.input,
                measured.coverage,
                stored.coverage
            );
        }
        document
            .with_saved_calibration(toniator_domain::ChannelId(1), saved)
            .unwrap();
    }
    /// Measures both raw and Uniform payloads for one eligible authored Even marks recipe.
    ///
    /// # Panics
    /// Panics if an eligible Pattern requires a second mode-dependent fit after publication.
    #[test]
    fn saved_even_marks_include_uniform_payload() {
        let document = toniator_domain::Document::new_default_document(
            toniator_domain::CanvasSpec {
                width: 128.0,
                height: 128.0,
            },
            toniator_domain::SourceReference::Unassigned,
        )
        .unwrap();
        let mut preset = PresetRegistry::bundled()
            .entries()
            .iter()
            .find(|entry| entry.metadata.id == "even-random-circles")
            .unwrap()
            .clone();
        let mut recipe = preset.recipe.clone();
        if let PatternStructureRecipe::RandomSites {
            density_modulation, ..
        } = &mut recipe.structure
        {
            *density_modulation = SiteDensityModulation::ArtworkWeighted;
        }
        let base = document.pattern_settings().clone();
        let base_definition = document.pattern_definition_bundles()[0].definition.clone();
        let document = document
            .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base,
                base_definition,
                recipe: recipe.clone(),
            })
            .unwrap()
            .0;
        let saved = calibrate_saved_pattern(
            &document,
            toniator_domain::ChannelId(1),
            crate::EvaluationLimits::default(),
            &|| false,
        )
        .unwrap();
        let uniform = saved
            .uniform()
            .expect("eligible marks retain Uniform payload");
        assert!(uniform.inverse().last().unwrap().coverage >= 0.999);
        if let Some(path) = std::env::var_os("TONIATOR_WEIGHTED_EVEN_COMPANION_OUT") {
            preset.recipe = recipe;
            preset.saved_calibration = Some(saved.clone());
            let path = std::path::PathBuf::from(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            toniator_io::save_preset(&path, &preset).unwrap();
            eprintln!(
                "weighted Even current companion saved to {}",
                path.display()
            );
        }
        document
            .with_saved_calibration(toniator_domain::ChannelId(1), saved)
            .unwrap();
    }

    /// Exercises the real bundled Clustered Connections recipe through both rejected strict tiers
    /// and the bounded mean, recording independent low-tone transfer for acceptance.
    ///
    /// # Panics
    /// Panics if the current typed recipe cannot publish a validated mean artifact.
    #[test]
    #[ignore = "explicit long-running stochastic reference acceptance"]
    fn clustered_connections_mean_reference_acceptance() {
        let document = toniator_domain::Document::new_default_document(
            toniator_domain::CanvasSpec {
                width: 1024.0,
                height: 1024.0,
            },
            toniator_domain::SourceReference::Unassigned,
        )
        .unwrap();
        let mut preset = PresetRegistry::bundled()
            .entries()
            .iter()
            .find(|entry| entry.metadata.id == "clustered-dispersion-random-links")
            .unwrap()
            .clone();
        let recipe = preset.recipe.clone();
        let base = document.pattern_settings().clone();
        let base_definition = document.pattern_definition_bundles()[0].definition.clone();
        let document = document
            .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base,
                base_definition,
                recipe,
            })
            .unwrap()
            .0;
        let start = std::time::Instant::now();
        let saved = calibrate_saved_pattern(
            &document,
            toniator_domain::ChannelId(1),
            crate::EvaluationLimits::default(),
            &|| false,
        )
        .unwrap();
        eprintln!(
            "clustered mean elapsed={:?} knots={} variation={:?}",
            start.elapsed(),
            saved.raw().len(),
            saved.variation()
        );
        assert_eq!(saved.profile(), toniator_domain::CALIBRATION_PROFILE_MEAN);
        assert!(saved.uniform().is_none());
        if let Some(path) = std::env::var_os("TONIATOR_CLUSTERED_COMPANION_OUT") {
            preset.saved_calibration = Some(saved);
            let path = std::path::PathBuf::from(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            toniator_io::save_preset(&path, &preset).unwrap();
            eprintln!("clustered current companion saved to {}", path.display());
        }
    }

    /// Samples alternate authored site and line seeds against one already-measured v5 response.
    ///
    /// This target-only acceptance fixture records observed differences at existing independent
    /// raw and inverse witnesses. It is not a fit-time loop or an all-seed render guarantee.
    ///
    /// # Panics
    /// Panics when the supplied current companion is absent or a representative variation
    /// exceeds the existing bounded empirical raw, normalized-tone, or relative-tone gate.
    #[test]
    #[ignore = "explicit bounded alternate-seed acceptance"]
    fn clustered_seed_variation_heldout_acceptance() {
        let path = std::env::var_os("TONIATOR_SEED_NEUTRAL_COMPANION")
            .expect("current Clustered v5 companion path is required");
        let record = toniator_io::load_preset(&std::path::PathBuf::from(path)).unwrap();
        let saved = record.saved_calibration.unwrap();
        let curve = RawCoverageCurve::new(
            saved
                .raw()
                .iter()
                .map(|knot| RawCoveragePoint {
                    size: knot.input,
                    coverage: knot.coverage,
                    vertices: 0,
                    elapsed: std::time::Duration::ZERO,
                })
                .collect(),
        )
        .unwrap();
        let span = curve.points().last().unwrap().coverage - curve.points()[0].coverage;
        let mut failed = Vec::new();
        for (label, site_seed, line_seed) in [
            ("site-zero", 0, 43),
            ("line-zero", 43, 0),
            ("both-max", u32::MAX, u32::MAX),
        ] {
            let mut recipe = record.recipe.clone();
            let PatternStructureRecipe::ConnectionPaths {
                definition,
                program,
                ..
            } = &mut recipe.structure
            else {
                panic!("Clustered preset keeps a connection wrapper");
            };
            let PatternStructureRecipe::RandomSites { seed, .. } = definition.as_mut() else {
                panic!("Clustered preset keeps random sites");
            };
            *seed = site_seed;
            let toniator_domain::ConnectionProgram::RandomLinks { seed, .. } = program else {
                panic!("Clustered preset keeps random links");
            };
            *seed = line_seed;
            let (document, source, channel) = reference_document(&recipe, 128).unwrap();
            let probe = FrozenCoverageProbe::new(
                &document,
                source,
                channel,
                crate::EvaluationLimits::default(),
                &|| false,
            )
            .unwrap();
            let mut measured = std::collections::HashMap::<u64, f64>::new();
            let mut at = |size: f64| -> f64 {
                *measured.entry(size.to_bits()).or_insert_with(|| {
                    probe
                        .measure_at(size, VectorOccupancyPrecision::Tight, &|| false)
                        .unwrap()
                        .coverage
                })
            };
            let mut raw_max = 0.0_f64;
            for size in [0.0, 0.01, 0.1, 0.5, 1.0, 1.5, 2.0] {
                raw_max = raw_max.max((at(size) - curve.coverage_at(size).unwrap()).abs());
            }
            let mut tone_max = 0.0_f64;
            let mut relative_max = 0.0_f64;
            for (minimum, maximum) in [(0.0, 2.0), (0.0, 1.5), (0.0, 1.0), (0.1, 1.2)] {
                let low = at(minimum);
                let high = at(maximum);
                assert!(high > low, "{label} retains a reachable response interval");
                for tone in [0.001_f64, 0.005, 0.01, 0.25, 0.5, 0.75] {
                    let size = curve.inverse_interval(minimum, maximum, tone).unwrap();
                    let delta = ((at(size) - low) / (high - low) - tone).abs();
                    tone_max = tone_max.max(delta);
                    relative_max = relative_max.max(delta / tone.max(0.0005));
                }
            }
            eprintln!(
                "seed variation {label}: raw={raw_max:.9} raw/span={:.6} tone={tone_max:.9} relative={relative_max:.6} probes={}",
                raw_max / span,
                measured.len()
            );
            if raw_max > 0.02 * span || tone_max > 0.01 || relative_max > 0.1 {
                failed.push(label);
            }
        }
        assert!(
            failed.is_empty(),
            "alternate seeds exceeded existing empirical gates: {failed:?}"
        );
    }

    /// Checks representative maze and weighted-dot seed changes without fitting each new seed.
    ///
    /// Full fitted reference artifacts remain fixed at seed 43. Alternate seed zero is observed
    /// on the doubled witness population using the existing raw/tone bounds; eligible Uniform
    /// marks additionally retain full-tone and survival transfer at their saved radius.
    ///
    /// # Panics
    /// Panics if either current catalog reference or its alternate seed exceeds the approved
    /// empirical bounds. This target-only check is not a per-render guarantee.
    #[test]
    #[ignore = "explicit bounded alternate-seed maze and weighted Uniform acceptance"]
    fn maze_and_weighted_uniform_seed_variation_heldout_acceptance() {
        for (id, uniform) in [("two-guide-maze", false), ("even-random-circles", true)] {
            let document = toniator_domain::Document::new_default_document(
                toniator_domain::CanvasSpec {
                    width: 1024.0,
                    height: 1024.0,
                },
                toniator_domain::SourceReference::Unassigned,
            )
            .unwrap();
            let mut preset = PresetRegistry::bundled()
                .entries()
                .iter()
                .find(|entry| entry.metadata.id == id)
                .unwrap()
                .clone();
            let mut recipe = preset.recipe.clone();
            if uniform {
                let PatternStructureRecipe::RandomSites {
                    density_modulation, ..
                } = &mut recipe.structure
                else {
                    panic!("Even marks retain random sites");
                };
                *density_modulation = SiteDensityModulation::ArtworkWeighted;
            }
            let candidate = document
                .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                    base: document.pattern_settings().clone(),
                    base_definition: document.pattern_definition_bundles()[0].definition.clone(),
                    recipe: recipe.clone(),
                })
                .unwrap()
                .0;
            let channel = toniator_domain::ChannelId(1);
            let binding =
                toniator_domain::CalibrationBinding::for_channel(&candidate, channel).unwrap();
            let started = std::time::Instant::now();
            let saved = if uniform {
                if let Some(path) = std::env::var_os("TONIATOR_WEIGHTED_EVEN_COMPANION_IN") {
                    let existing =
                        toniator_io::load_preset(&std::path::PathBuf::from(path)).unwrap();
                    assert_eq!(existing.recipe, recipe);
                    existing
                        .saved_calibration
                        .expect("weighted Even input has v5 companion")
                } else {
                    calibrate_saved_pattern(
                        &candidate,
                        channel,
                        crate::EvaluationLimits::default(),
                        &|| false,
                    )
                    .unwrap()
                }
            } else {
                calibrate_saved_pattern(
                    &candidate,
                    channel,
                    crate::EvaluationLimits::default(),
                    &|| false,
                )
                .unwrap()
            };
            eprintln!(
                "seed witness {id}: canonical fit {:?} profile={}",
                started.elapsed(),
                saved.profile()
            );
            let witness_side = if saved.profile() == toniator_domain::CALIBRATION_PROFILE_SEEDED {
                64
            } else {
                128
            };
            let mut alternate = recipe.clone();
            if uniform {
                let PatternStructureRecipe::RandomSites { seed, .. } = &mut alternate.structure
                else {
                    unreachable!()
                };
                *seed = 0;
            } else {
                let PatternStructureRecipe::MazeWalls { program, .. } = &mut alternate.structure
                else {
                    panic!("two-guide maze retains maze wrapper");
                };
                program.seed = 0;
            }
            let curve = RawCoverageCurve::new(
                saved
                    .raw()
                    .iter()
                    .map(|knot| RawCoveragePoint {
                        size: knot.input,
                        coverage: knot.coverage,
                        vertices: 0,
                        elapsed: std::time::Duration::ZERO,
                    })
                    .collect(),
            )
            .unwrap();
            let span = curve.points().last().unwrap().coverage - curve.points()[0].coverage;
            let (reference, source, reference_channel) =
                reference_document(&alternate, witness_side).unwrap();
            let probe = FrozenCoverageProbe::new(
                &reference,
                source,
                reference_channel,
                crate::EvaluationLimits::default(),
                &|| false,
            )
            .unwrap();
            let mut measurements = std::collections::HashMap::<u64, f64>::new();
            let mut at = |size: f64| -> f64 {
                *measurements.entry(size.to_bits()).or_insert_with(|| {
                    probe
                        .measure_at(size, VectorOccupancyPrecision::Tight, &|| false)
                        .unwrap()
                        .coverage
                })
            };
            let mut raw_max = 0.0_f64;
            for size in [0.0, 0.01, 0.1, 0.5, 1.0, 1.5, 2.0] {
                raw_max = raw_max.max((at(size) - curve.coverage_at(size).unwrap()).abs());
            }
            let mut tone_max = 0.0_f64;
            let mut relative_max = 0.0_f64;
            for (minimum, maximum) in [(0.0, 2.0), (0.0, 1.0)] {
                let low = at(minimum);
                let high = at(maximum);
                assert!(high > low, "{id} remains reachable");
                for tone in [0.001_f64, 0.005, 0.01, 0.25, 0.5, 0.75] {
                    let size = curve.inverse_interval(minimum, maximum, tone).unwrap();
                    let delta = ((at(size) - low) / (high - low) - tone).abs();
                    tone_max = tone_max.max(delta);
                    relative_max = relative_max.max(delta / tone.max(0.0005));
                }
            }
            eprintln!(
                "seed witness {id}: raw/span={:.6} tone={tone_max:.9} relative={relative_max:.6} probes={}",
                raw_max / span,
                measurements.len()
            );
            assert!(
                raw_max <= 0.02 * span && tone_max <= 0.01 && relative_max <= 0.1,
                "{id} alternate seed exceeds bounded response variation"
            );
            if uniform {
                let uniform = saved
                    .uniform()
                    .expect("weighted Even marks include Uniform");
                let uniform_probe = super::uniform_reference_probe(
                    &alternate,
                    &binding,
                    toniator_domain::CALIBRATION_REFERENCE_CANVAS_SIDE,
                    witness_side,
                    crate::EvaluationLimits::default(),
                    &|| false,
                )
                .unwrap()
                .expect("weighted marks admit Uniform");
                let fit = crate::uniform_density::UniformDensityFit::from_saved(
                    uniform,
                    uniform_probe.nominal_spacing().unwrap(),
                )
                .unwrap();
                let geometry = uniform_probe
                    .produce_uniform(&uniform_probe.family, fit.radius, true, &|| false)
                    .unwrap();
                let (absolute, relative) = crate::uniform_density::check_population_transfer(
                    uniform_probe.document.canvas(),
                    &geometry,
                    &fit,
                    &|| false,
                )
                .unwrap();
                eprintln!("seed witness {id} Uniform: tone={absolute:.9} relative={relative:.6}");
            } else if let Some(path) = std::env::var_os("TONIATOR_MAZE_COMPANION_OUT") {
                preset.saved_calibration = Some(saved);
                let path = std::path::PathBuf::from(path);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                toniator_io::save_preset(&path, &preset).unwrap();
                eprintln!("maze current companion saved to {}", path.display());
            }
        }
    }

    /// Keeps ALL and channel bindings equal when Scatter ignores a changed guide aspect.
    ///
    /// # Panics
    /// Panics if the typed bundled recipe or a current aspect edit cannot validate.
    #[test]
    fn clustered_baseline_ignores_nonstructural_guide_aspect() {
        let document = toniator_domain::Document::new_default_document(
            toniator_domain::CanvasSpec {
                width: 1024.0,
                height: 1024.0,
            },
            toniator_domain::SourceReference::Unassigned,
        )
        .unwrap();
        let recipe = PresetRegistry::bundled()
            .entries()
            .iter()
            .find(|entry| entry.metadata.id == "clustered-dispersion-random-links")
            .unwrap()
            .recipe
            .clone();
        let base = document.pattern_settings().clone();
        let definition = document.pattern_definition_bundles()[0].definition.clone();
        let document = document
            .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base,
                base_definition: definition,
                recipe,
            })
            .unwrap()
            .0;
        let base = document.pattern_settings().clone();
        let mut settings = base.clone();
        settings.density.aspect = 1.4;
        let document = document
            .apply_command(&DocumentCommand::SetDocumentPatternSettings { base, settings })
            .unwrap()
            .0;
        assert_eq!(
            document.baseline_calibration_binding().unwrap(),
            toniator_domain::CalibrationBinding::for_channel(
                &document,
                toniator_domain::ChannelId(1)
            )
            .unwrap()
        );
    }

    /// Refines one paired Uniform radius until the independent population reaches full tone.
    ///
    /// # Panics
    /// Panics if an artwork-weighted real Pattern cannot retain bounded 128 transfer.
    #[test]
    fn even_random_marks_paired_uniform_refines_independent_full_tone() {
        let mut recipe = PresetRegistry::bundled()
            .entries()
            .iter()
            .find(|entry| entry.metadata.id == "even-random-circles")
            .unwrap()
            .recipe
            .clone();
        if let PatternStructureRecipe::RandomSites {
            density_modulation, ..
        } = &mut recipe.structure
        {
            *density_modulation = SiteDensityModulation::ArtworkWeighted;
        }
        let (document, _, channel) = reference_document(&recipe, 32).unwrap();
        let binding = toniator_domain::CalibrationBinding::for_channel(&document, channel).unwrap();
        assert!(binding.supports_stochastic_mean());
        let (uniform, transfer) = super::fit_uniform_mean(
            &recipe,
            &binding,
            crate::EvaluationLimits::default(),
            &|| false,
        )
        .unwrap();
        let uniform = uniform.expect("eligible weighted marks retain a Uniform calibration");
        let (tone_absolute, tone_relative) = transfer.expect("128 transfer is measured");
        assert!(uniform.radius_per_nominal_spacing() >= 1.020609202 * 1.02 - 1.0e-6);
        assert!(uniform.inverse().last().unwrap().coverage >= 0.999);
        assert!(tone_absolute <= 0.01);
        assert!(tone_relative <= 0.1);
    }

    /// Separates low-tone inverse interpolation error from cross-population transfer.
    ///
    /// # Panics
    /// Panics if the frozen 32-spacing Even family cannot measure independent held-out sizes.
    #[test]
    #[ignore = "explicit bounded low-tone interpolation diagnostic"]
    fn even_same_window_low_tone_diagnostic() {
        let registry = PresetRegistry::bundled();
        let recipe = &registry
            .entries()
            .iter()
            .find(|entry| entry.metadata.id == "even-random-circles")
            .unwrap()
            .recipe;
        let (document, source, channel) = reference_document(recipe, 32).unwrap();
        let probe = FrozenCoverageProbe::new(
            &document,
            source,
            channel,
            crate::EvaluationLimits::default(),
            &|| false,
        )
        .unwrap();
        let curve = probe.fit_raw_curve(&|| false).unwrap();
        let low = probe
            .measure_at(0.1, VectorOccupancyPrecision::Tight, &|| false)
            .unwrap()
            .coverage;
        let high = probe
            .measure_at(1.2, VectorOccupancyPrecision::Tight, &|| false)
            .unwrap()
            .coverage;
        eprintln!(
            "even frozen32 low={low:.9} high={high:.9} curve_low={:.9} curve_high={:.9}",
            curve.coverage_at(0.1).unwrap(),
            curve.coverage_at(1.2).unwrap()
        );
        for tone in [0.001, 0.005, 0.01] {
            let size = curve.inverse_interval(0.1, 1.2, tone).unwrap();
            let direct = probe
                .measure_at(size, VectorOccupancyPrecision::Tight, &|| false)
                .unwrap()
                .coverage;
            let actual = (direct - low) / (high - low);
            eprintln!(
                "even frozen32 tone={tone:.4} target_c={:.9} size={size:.9} curve_c={:.9} direct_c={direct:.9} actual={actual:.9} error={:.9}",
                low + tone * (high - low),
                curve.coverage_at(size).unwrap(),
                (actual - tone).abs()
            );
        }
    }

    /// Constructs a finite raw knot for interval-inverse contract tests.
    fn point(size: f64, coverage: f64) -> RawCoveragePoint {
        RawCoveragePoint {
            size,
            coverage,
            vertices: 0,
            elapsed: Duration::ZERO,
        }
    }

    /// Preserves exact authored endpoints and positive low-tone response in a subinterval.
    ///
    /// # Panics
    /// Panics only if finite monotone test knots or their inverse violate the stated contract.
    #[test]
    fn interval_inverse_preserves_endpoints_and_low_tones() {
        let curve = RawCoverageCurve::new(vec![
            point(0.0, 0.0),
            point(0.25, 0.0),
            point(1.0, 0.4),
            point(RAW_SIZE_MAXIMUM, 0.9),
        ])
        .unwrap();
        assert_eq!(curve.inverse_interval(0.25, 1.5, 0.0).unwrap(), 0.25);
        assert_eq!(curve.inverse_interval(0.25, 1.5, 1.0).unwrap(), 1.5);
        assert!(curve.inverse_interval(0.0, 2.0, 0.001).unwrap() > 0.25);
        assert_eq!(curve.inverse_interval(0.6, 0.6, 0.4).unwrap(), 0.6);
    }

    /// Distinguishes measured saturation from a reversed or invalid response table.
    ///
    /// # Panics
    /// Panics only if flat-interval interpolation or monotone rejection regresses.
    #[test]
    fn plateau_is_explicit_and_reversal_rejects() {
        let curve =
            RawCoverageCurve::new(vec![point(0.0, 0.0), point(1.0, 0.8), point(2.0, 0.8)]).unwrap();
        assert_eq!(curve.inverse_interval(1.2, 1.8, 0.25).unwrap(), 1.35);
        assert_eq!(
            RawCoverageCurve::new(vec![point(0.0, 0.0), point(1.0, 0.8), point(2.0, 0.7)])
                .unwrap_err()
                .path(),
            "reference.curve.monotone"
        );
    }

    /// Normalizes only one representable saturation step while retaining strict larger reversal rejection.
    ///
    /// # Panics
    /// Panics if full-coverage floating roundoff changes response shape or a two-ULP reversal
    /// passes the validated raw-curve boundary.
    #[test]
    fn exact_saturation_roundoff_is_the_only_normalized_reversal() {
        let one_below = f64::from_bits(1.0_f64.to_bits() - 1);
        let two_below = f64::from_bits(1.0_f64.to_bits() - 2);
        let curve = RawCoverageCurve::new(vec![
            point(0.0, 0.0),
            point(1.0, 1.0),
            point(1.5, one_below),
            point(2.0, 1.0),
        ])
        .unwrap();
        assert_eq!(curve.points()[2].coverage, 1.0);
        assert_eq!(
            RawCoverageCurve::new(vec![
                point(0.0, 0.0),
                point(1.0, 1.0),
                point(1.5, two_below),
                point(2.0, 1.0),
            ])
            .unwrap_err()
            .path(),
            "reference.curve.monotone",
        );
    }

    /// Normalizes an inverse target to a sparse curve's own reachable coverage interval.
    ///
    /// # Panics
    /// Panics if a low-fill but monotone curve loses a low-tone crossing or authored endpoints.
    #[test]
    fn sparse_curve_inverse_uses_reachable_span() {
        let curve = RawCoverageCurve::new(vec![
            point(0.0, 0.0),
            point(0.25, 0.0001),
            point(0.5, 0.002),
            point(1.0, 0.011),
            point(2.0, 0.011),
        ])
        .unwrap();
        let minimum = 0.25;
        let maximum = 1.0;
        let size = curve.inverse_interval(minimum, maximum, 0.005).unwrap();
        let target = curve.coverage_at(minimum).unwrap()
            + 0.005 * (curve.coverage_at(maximum).unwrap() - curve.coverage_at(minimum).unwrap());
        assert!((curve.coverage_at(size).unwrap() - target).abs() < 1e-12);
        assert!(size > minimum && size < maximum);
    }

    /// Keeps one complete Straight circle family through raw zero, mid, and maximum probes.
    ///
    /// # Panics
    /// Panics if the current bundled recipe cannot form its white reference, family identity
    /// changes between probes, or canonical union coverage reverses at these witness sizes.
    #[test]
    fn straight_reference_reuses_one_frozen_family() {
        let registry = PresetRegistry::bundled();
        let recipe = &registry
            .entries()
            .iter()
            .find(|entry| entry.metadata.id == "straight-grid-circles")
            .unwrap()
            .recipe;
        let (document, source, channel) = reference_document(recipe, 32).unwrap();
        let probe = FrozenCoverageProbe::new(
            &document,
            source,
            channel,
            super::EvaluationLimits::default(),
            &|| false,
        )
        .unwrap();
        let fingerprint = probe.family_fingerprint().to_owned();
        let values = [0.0, 0.5, 1.0, 2.0].map(|size| {
            let measurement = probe
                .measure_at(size, VectorOccupancyPrecision::Baseline, &|| false)
                .unwrap();
            assert_eq!(probe.family_fingerprint(), fingerprint);
            measurement.coverage
        });
        assert!(
            values.windows(2).all(|pair| pair[0] <= pair[1]),
            "{values:?}"
        );
        assert!(values[3] > values[0]);
    }

    /// Keeps the original zero/two coverage envelope while adaptive midpoints append.
    ///
    /// The fixed Straight fixture needs fewer than 64 knots with immutable endpoints; reading
    /// `retained.last()` after an append expands it past that bound and fails this regression.
    ///
    /// # Panics
    /// Panics if the frozen-family fit exceeds its expected work or misses an interior witness.
    #[test]
    fn adaptive_knots_keep_frozen_endpoint_envelope() {
        let registry = PresetRegistry::bundled();
        let recipe = &registry
            .entries()
            .iter()
            .find(|entry| entry.metadata.id == "straight-grid-circles")
            .unwrap()
            .recipe;
        let (document, source, channel) = reference_document(recipe, 32).unwrap();
        let probe = FrozenCoverageProbe::new(
            &document,
            source,
            channel,
            super::EvaluationLimits::default(),
            &|| false,
        )
        .unwrap();
        let curve = probe.fit_raw_curve(&|| false).unwrap();
        assert!(curve.points().len() <= 64, "{} knots", curve.points().len());
        assert_eq!(curve.points()[0].size, 0.0);
        assert_eq!(curve.points().last().unwrap().size, RAW_SIZE_MAXIMUM);
        let observed = probe
            .measure_at(0.125, VectorOccupancyPrecision::Baseline, &|| false)
            .unwrap()
            .coverage;
        assert!((curve.coverage_at(0.125).unwrap() - observed).abs() < 0.0005);
    }

    /// Keeps Uniform's stored radius unit tied to Even's authored distance, not guide density.
    ///
    /// # Panics
    /// Panics if the reference family cannot form or its physical spacing changes authority.
    #[test]
    fn uniform_scatter_radius_uses_authored_spatial_basis() {
        let registry = PresetRegistry::bundled();
        let recipe = &registry
            .entries()
            .iter()
            .find(|entry| entry.metadata.id == "even-random-circles")
            .unwrap()
            .recipe;
        for (side, expected) in [(32, 25.0), (64, 12.5)] {
            let (document, source, channel) = reference_document(recipe, side).unwrap();
            let probe = FrozenCoverageProbe::new(
                &document,
                source,
                channel,
                super::EvaluationLimits::default(),
                &|| false,
            )
            .unwrap();
            assert!((probe.nominal_spacing().unwrap() - expected).abs() < 1.0e-12);
        }
    }

    /// Keeps strict Visible Mark Margin in Variable mode despite dormant Uniform intent.
    ///
    /// # Panics
    /// Panics if current recipe validation, applicability projection, or the experimental
    /// Uniform fit guard accepts a strict-separation mark output.
    #[test]
    fn strict_margin_cannot_enter_uniform_reference_fit() {
        let registry = PresetRegistry::bundled();
        let mut recipe = registry
            .entries()
            .iter()
            .find(|entry| entry.metadata.id == "even-random-circles")
            .unwrap()
            .recipe
            .clone();
        let PatternStructureRecipe::RandomSites {
            density_modulation,
            exclusion,
            ..
        } = &mut recipe.structure
        else {
            panic!("fixture is a random site recipe")
        };
        *density_modulation = SiteDensityModulation::ArtworkWeighted;
        *exclusion = SiteExclusionPolicy::VisibleMarkMargin { margin: 0.1 };
        let (document, source, channel) = reference_document(&recipe, 32).unwrap();
        let base = document.response_settings().clone();
        let mut settings = base.clone();
        settings.mark_density_mode = MarkDensityMode::Uniform;
        let document = document
            .apply_command(&DocumentCommand::SetAllResponseSettings { base, settings })
            .unwrap()
            .0;
        let output_id = document
            .effective_channel_pattern(channel)
            .unwrap()
            .output_settings[0]
            .output_layer_id;
        assert_eq!(
            document
                .effective_mark_density(channel, output_id)
                .unwrap()
                .mode,
            MarkDensityMode::Variable
        );
        let probe = FrozenCoverageProbe::new(
            &document,
            source,
            channel,
            super::EvaluationLimits::default(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            probe.fit_uniform_reference(&|| false).err().unwrap().path(),
            "reference.uniform.mode"
        );
    }
}

#[cfg(test)]
mod artistic_saturation_diagnostic {
    use super::*;

    /// Remeasures the exact tree saturation pairs on frozen references at both existing precisions.
    /// This diagnostic changes no acceptance criteria, canonical producer, or stored artifact.
    /// # Panics
    /// Panics if the current registry fixture, geometry, or independent occupancy consumer fails.
    #[test]
    #[ignore = "bounded Phase5 numerical diagnosis"]
    fn tree_saturation_pairs() {
        let registry = toniator_patterns::PresetRegistry::bundled();
        let recipe = &registry.find("triagrid-spanning-tree").unwrap().recipe;
        let (base, _, channel) = reference_document(recipe, 32).unwrap();
        let binding = CalibrationBinding::for_channel(&base, channel).unwrap();
        let recipe = reference_recipe_from_binding(&binding);
        for side in [32, 64] {
            let (document, source, channel) = reference_document(&recipe, side).unwrap();
            let probe = FrozenCoverageProbe::new(
                &document,
                source,
                channel,
                EvaluationLimits::default(),
                &|| false,
            )
            .unwrap();
            for size in [0.8125, 0.84375, 0.9375, 1.0, 1.5, 1.75] {
                let geometry = probe.produce(size / RAW_SIZE_MAXIMUM, &|| false).unwrap();
                for precision in [
                    VectorOccupancyPrecision::Baseline,
                    VectorOccupancyPrecision::Tight,
                ] {
                    let result = measure_geometry_vector_occupancy(
                        &geometry,
                        document.canvas(),
                        precision,
                        RasterizationLimits::default(),
                        &|| false,
                    )
                    .unwrap();
                    eprintln!(
                        "TREE_PAIR side={side} size={size:.17} precision={precision:?} fraction={:.17} normalized_canvas_area=1 vertices={}",
                        result.fraction, result.vertices
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod artistic_strict_curve_tests {
    use super::*;

    /// Keeps genuine raw-coverage reversals invalid after the vector adapter correction.
    /// # Panics
    /// Panics if strict calibration smooths, sorts coverage, or accepts a decreasing measured curve.
    #[test]
    fn real_coverage_reversal_still_rejects() {
        let points = vec![(0.0, 0.0), (1.0, 0.7), (2.0, 0.69)]
            .into_iter()
            .map(|(size, coverage)| RawCoveragePoint {
                size,
                coverage,
                vertices: 0,
                elapsed: Duration::ZERO,
            })
            .collect();
        assert_eq!(
            RawCoverageCurve::new(points).unwrap_err().path(),
            "reference.curve.monotone"
        );
    }
}
