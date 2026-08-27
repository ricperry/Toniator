//! Geometry-owned positive region resizing with no negative-space construction.

use std::{collections::BTreeMap, error::Error, fmt};

use rayon::prelude::*;
use toniator_domain::{PatternOutputLayerId, RegionResizeAlgorithm};

use crate::{
    CanonicalRegionLimits, CanonicalRegionProposal, CanonicalRegionSet, CanonicalRegionSourceGroup,
    CubicBezierSegment, CurvePath, CurveSegment, LineSegment, PathClosure, PathOffsetCleanup,
    PathOffsetEndpointPolicy, PathOffsetRequest, PathOffsetResult, PathOffsetWork, Point2, Vector2,
    build_canonical_regions_cancellable, build_tagged_canonical_regions_cancellable,
    offset_path_with_work_region_round_cancellable,
};

/// Versioned private normalized-region-resize contract used by pattern and engine cache identities.
pub const REGION_TREATMENT_CONTRACT_ID: &str = "toniator.region-resize.v2";

/// Reference point owned by a region producer and excluded from source canonical geometry identity.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionReference {
    /// Identifies exactly one untreated canonical region, including its component ordinal.
    pub region_id: crate::CanonicalRegionId,
    /// Supplies the finite affine and reference-point sampling origin for that component.
    pub point: Point2,
}

/// One normalized positive-region resize resolved after source sampling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionTreatment {
    /// Selects affine scaling or signed normal offset on the region's own boundary.
    pub algorithm: RegionResizeAlgorithm,
    /// Stores the finite normalized fill multiplier in the inclusive `0.0..=2.0` range.
    pub fill: f64,
}

/// One resolved resize value for exactly one untreated canonical base region.
///
/// `treatment: None` suppresses a transparent sampled base before geometry construction. A
/// present zero fill has the same positive-geometry outcome while retaining its resolved intent
/// for diagnostics and fingerprint construction.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionTreatmentRequest {
    /// Identifies the untreated canonical region whose components this request may derive.
    pub base_region_id: crate::CanonicalRegionId,
    /// Supplies the producer-owned affine origin when Scale has nonzero, nonidentity fill.
    pub reference: Option<Point2>,
    /// Supplies a typed normalized resize, or omits the base entirely.
    pub treatment: Option<RegionTreatment>,
}

/// Deterministic resized-to-untreated ownership retained outside canonical geometry fingerprints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionTreatmentProvenance {
    /// Identifies one canonical resized component in canonical output order.
    pub treated_region_id: crate::CanonicalRegionId,
    /// Identifies the untreated base region that supplied every component construction input.
    pub base_region_id: crate::CanonicalRegionId,
}

/// Complete atomic result of positive-region resizing and sampled-paint ownership.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionTreatmentResult {
    /// Stores canonical positive fill rings, which may be empty after zero fill or suppression.
    pub regions: CanonicalRegionSet,
    /// Stores one ordered provenance item for every retained canonical region.
    pub provenance: Vec<RegionTreatmentProvenance>,
}

/// Request-wide bounds for resize construction and canonical post-processing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionTreatmentLimits {
    /// Bounds all source and retained region, segment, and inspection work across the request.
    pub canonical: CanonicalRegionLimits,
    /// Bounds every reusable signed normal-offset primitive used by UniformOffset resizing.
    pub path_offset: crate::PathOffsetLimits,
}

impl Default for RegionTreatmentLimits {
    /// Supplies the accepted canonical and reusable path-offset defaults.
    fn default() -> Self {
        Self {
            canonical: CanonicalRegionLimits::default(),
            path_offset: crate::PathOffsetLimits::default(),
        }
    }
}

/// Stable resize failure that never exposes partial canonical geometry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionTreatmentError {
    path: &'static str,
    message: &'static str,
}

impl RegionTreatmentError {
    /// Returns the stable region-resize failure path.
    pub const fn path(&self) -> &'static str {
        self.path
    }

    /// Returns the stable region-resize failure message.
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for RegionTreatmentError {
    /// Formats the stable region-resize failure without exposing a partial candidate.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for RegionTreatmentError {}

/// Tracks canonical-region work across every intermediate and final resize build.
struct CanonicalTreatmentWork {
    limits: CanonicalRegionLimits,
    source_groups: usize,
    regions: usize,
    segments: usize,
    inspections: usize,
}

impl CanonicalTreatmentWork {
    /// Starts one request-wide canonical budget without allocating geometry.
    const fn new(limits: CanonicalRegionLimits) -> Self {
        Self {
            limits,
            source_groups: 0,
            regions: 0,
            segments: 0,
            inspections: 0,
        }
    }

    /// Builds one untagged intermediate against the remaining aggregate budget.
    ///
    /// # Errors
    ///
    /// Returns the stable resize canonical-limit, allocation, geometry, or cancellation error.
    fn build(
        &mut self,
        proposal: CanonicalRegionProposal,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(CanonicalRegionSet, crate::CanonicalRegionDiagnostics), RegionTreatmentError> {
        let result =
            build_canonical_regions_cancellable(proposal, self.remaining_limits()?, cancelled)
                .map_err(map_canonical_treatment_error)?;
        self.charge(result.1)?;
        Ok(result)
    }

    /// Builds final tagged geometry against the same aggregate intermediate-work budget.
    ///
    /// # Errors
    ///
    /// Returns the stable resize canonical-limit, allocation, geometry, or cancellation error.
    fn build_tagged(
        &mut self,
        output_layer_id: PatternOutputLayerId,
        source_groups: Vec<crate::TaggedCanonicalRegionSourceGroup>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<
        (
            CanonicalRegionSet,
            crate::CanonicalRegionDiagnostics,
            Vec<u64>,
        ),
        RegionTreatmentError,
    > {
        let result = build_tagged_canonical_regions_cancellable(
            output_layer_id,
            source_groups,
            self.remaining_limits()?,
            cancelled,
        )
        .map_err(map_canonical_treatment_error)?;
        self.charge(result.1)?;
        Ok(result)
    }

    /// Derives one nonzero canonical builder limit set from remaining aggregate capacity.
    ///
    /// # Errors
    ///
    /// Returns the stable canonical-limit diagnostic when any required budget is exhausted.
    fn remaining_limits(&self) -> Result<CanonicalRegionLimits, RegionTreatmentError> {
        let remaining = [
            self.limits
                .max_source_groups()
                .checked_sub(self.source_groups),
            self.limits.max_regions().checked_sub(self.regions),
            self.limits.max_segments().checked_sub(self.segments),
            self.limits.max_inspections().checked_sub(self.inspections),
        ];
        if remaining
            .iter()
            .any(|value| value.is_none_or(|value| value == 0))
        {
            return Err(region_error(
                "region.resize.limits.canonical",
                "request-wide canonical region treatment limit exceeded",
            ));
        }
        CanonicalRegionLimits::new(
            remaining[0].expect("checked nonzero source-group remainder"),
            remaining[1].expect("checked nonzero region remainder"),
            remaining[2].expect("checked nonzero segment remainder"),
            remaining[3].expect("checked nonzero inspection remainder"),
        )
        .map_err(map_canonical_treatment_error)
    }

    /// Charges one completed canonical build before any later candidate can start.
    ///
    /// # Errors
    ///
    /// Returns the aggregate canonical-limit diagnostic on arithmetic overflow or exhaustion.
    fn charge(
        &mut self,
        diagnostics: crate::CanonicalRegionDiagnostics,
    ) -> Result<(), RegionTreatmentError> {
        self.source_groups = self
            .source_groups
            .checked_add(diagnostics.source_groups)
            .ok_or(region_error(
                "region.resize.limits.canonical",
                "request-wide canonical source-group work overflowed",
            ))?;
        self.regions = self
            .regions
            .checked_add(diagnostics.regions)
            .ok_or(region_error(
                "region.resize.limits.canonical",
                "request-wide canonical region work overflowed",
            ))?;
        self.segments = self
            .segments
            .checked_add(diagnostics.segments)
            .ok_or(region_error(
                "region.resize.limits.canonical",
                "request-wide canonical segment work overflowed",
            ))?;
        self.inspections = self
            .inspections
            .checked_add(diagnostics.inspections)
            .ok_or(region_error(
                "region.resize.limits.canonical",
                "request-wide canonical inspection work overflowed",
            ))?;
        Ok(())
    }
}

/// Resizes independently resolved base regions and returns canonical positive geometry plus ownership.
///
/// Every request addresses exactly one accepted untreated region. Fill zero and transparent bases
/// are omitted before canonical construction. Fill one replays the source ring without an affine
/// transform or offset, preserving the natural producer boundary exactly. UniformOffset solves an
/// area target on each individual positive region; it never measures inter-region space.
///
/// # Errors
///
/// Returns stable identity, geometry, allocation, canonical-limit, offset, or cancellation
/// diagnostics without exposing a partial set or a partially aligned provenance table.
pub fn treat_region_requests_cancellable(
    output_layer_id: PatternOutputLayerId,
    source: &CanonicalRegionSet,
    requests: &[RegionTreatmentRequest],
    limits: RegionTreatmentLimits,
    cancelled: impl Fn() -> bool + Sync,
) -> Result<RegionTreatmentResult, RegionTreatmentError> {
    let cancelled_ref = &cancelled;
    poll_treatment(cancelled_ref)?;
    if requests.len() != source.regions().len() {
        return Err(region_error(
            "region.resize.identity.requests",
            "every untreated region requires exactly one resize request",
        ));
    }
    let mut source_by_id = BTreeMap::new();
    for region in source.regions() {
        if region.id.output_layer_id != output_layer_id {
            return Err(region_error(
                "region.resize.identity.output",
                "resize output identity must match every untreated region",
            ));
        }
        source_by_id.insert(region.id.clone(), region);
    }
    let mut requests_by_id = BTreeMap::new();
    for request in requests {
        poll_treatment(cancelled_ref)?;
        if !source_by_id.contains_key(&request.base_region_id) {
            return Err(region_error(
                "region.resize.identity.base",
                "resize request must address an accepted untreated region",
            ));
        }
        if requests_by_id
            .insert(request.base_region_id.clone(), request)
            .is_some()
        {
            return Err(region_error(
                "region.resize.identity.duplicate",
                "resize requests must not repeat an untreated region",
            ));
        }
        validate_treatment_request(request)?;
    }
    if requests_by_id.len() != source_by_id.len() {
        return Err(region_error(
            "region.resize.identity.requests",
            "resize requests must cover every untreated region",
        ));
    }
    if requests.iter().all(RegionTreatmentRequest::is_identity) {
        return Ok(identity_result(source));
    }

    let mut offset_work = PathOffsetWork::new(limits.path_offset)
        .map_err(|error| region_error("region.resize.limits.offset", error.message()))?;
    let mut canonical_work = CanonicalTreatmentWork::new(limits.canonical);
    let ordered_sources = source_by_id.into_iter().collect::<Vec<_>>();
    let has_nonidentity_offset = ordered_sources.iter().any(|(base_id, _)| {
        requests_by_id[base_id].treatment.is_some_and(|treatment| {
            treatment.algorithm == RegionResizeAlgorithm::UniformOffset
                && treatment.fill != 0.0
                && treatment.fill != 1.0
        })
    });
    let prepared = if has_nonidentity_offset {
        let mut prepared = Vec::with_capacity(ordered_sources.len());
        for (base_id, region) in &ordered_sources {
            poll_treatment(cancelled_ref)?;
            prepared.push(prepare_region_components_serial(
                output_layer_id,
                base_id,
                region,
                requests_by_id[base_id],
                &mut canonical_work,
                &mut offset_work,
                cancelled_ref,
            )?);
        }
        prepared
    } else {
        let results = ordered_sources
            .par_iter()
            .map(|(base_id, region)| {
                poll_treatment(cancelled_ref)?;
                prepare_scale_region_components(
                    base_id,
                    region,
                    requests_by_id[base_id],
                    cancelled_ref,
                )
            })
            .collect::<Vec<_>>();
        results.into_iter().collect::<Result<Vec<_>, _>>()?
    };
    let mut groups: BTreeMap<crate::CanonicalRegionSourceId, Vec<(CurvePath, u64)>> =
        BTreeMap::new();
    let mut owners = Vec::new();
    for prepared in prepared.into_iter().flatten() {
        let owner_tag = u64::try_from(owners.len()).map_err(|_| {
            region_error(
                "region.resize.allocation.provenance",
                "resized base-owner ordinal exceeds u64",
            )
        })?;
        owners.push(prepared.base_id);
        groups.entry(prepared.source_id).or_default().extend(
            prepared
                .components
                .into_iter()
                .map(|component| (component, owner_tag)),
        );
    }
    if groups.is_empty() {
        return Ok(RegionTreatmentResult {
            regions: CanonicalRegionSet::empty(),
            provenance: Vec::new(),
        });
    }
    let source_groups = groups
        .into_iter()
        .map(
            |(source_id, components)| crate::TaggedCanonicalRegionSourceGroup {
                source_id,
                components,
            },
        )
        .collect();
    let (regions, _, tags) =
        canonical_work.build_tagged(output_layer_id, source_groups, cancelled_ref)?;
    let mut provenance = Vec::new();
    provenance
        .try_reserve(regions.regions().len())
        .map_err(|_| {
            region_error(
                "region.resize.allocation.provenance",
                "resized provenance allocation failed",
            )
        })?;
    for (treated, owner_tag) in regions.regions().iter().zip(tags) {
        poll_treatment(cancelled_ref)?;
        provenance.push(RegionTreatmentProvenance {
            treated_region_id: treated.id.clone(),
            base_region_id: owners
                .get(usize::try_from(owner_tag).map_err(|_| {
                    region_error(
                        "region.resize.identity.provenance",
                        "resized component owner tag exceeds usize",
                    )
                })?)
                .ok_or(region_error(
                    "region.resize.identity.provenance",
                    "resized component must retain one base owner",
                ))?
                .clone(),
        });
    }
    Ok(RegionTreatmentResult {
        regions,
        provenance,
    })
}

/// Applies one resolved resize to every supplied canonical region for focused geometry callers.
///
/// # Errors
///
/// Returns the same atomic resize diagnostics as `treat_region_requests_cancellable`.
pub fn treat_regions_cancellable(
    output_layer_id: PatternOutputLayerId,
    source: &CanonicalRegionSet,
    references: &[RegionReference],
    treatment: RegionTreatment,
    cancelled: impl Fn() -> bool + Sync,
) -> Result<CanonicalRegionSet, RegionTreatmentError> {
    let references_by_id: BTreeMap<_, _> = references
        .iter()
        .map(|reference| (reference.region_id.clone(), reference.point))
        .collect();
    let requests = source
        .regions()
        .iter()
        .map(|region| RegionTreatmentRequest {
            base_region_id: region.id.clone(),
            reference: references_by_id.get(&region.id).copied(),
            treatment: Some(treatment),
        })
        .collect::<Vec<_>>();
    treat_region_requests_cancellable(
        output_layer_id,
        source,
        &requests,
        RegionTreatmentLimits::default(),
        cancelled,
    )
    .map(|result| result.regions)
}

/// Validates one resolved normalized resize and its required affine reference.
///
/// # Errors
///
/// Returns stable identity or fill-range diagnostics before geometry work starts.
fn validate_treatment_request(
    request: &RegionTreatmentRequest,
) -> Result<(), RegionTreatmentError> {
    let Some(treatment) = request.treatment else {
        return Ok(());
    };
    if !treatment.fill.is_finite() || !(0.0..=2.0).contains(&treatment.fill) {
        return Err(region_error(
            "region.resize.geometry.fill",
            "region fill must be finite and within 0.0 through 2.0",
        ));
    }
    if treatment.algorithm == RegionResizeAlgorithm::Scale
        && treatment.fill != 0.0
        && treatment.fill != 1.0
        && !request.reference.is_some_and(Point2::is_finite)
    {
        return Err(region_error(
            "region.resize.identity.reference",
            "nonidentity Scale requires a finite producer reference",
        ));
    }
    Ok(())
}

impl RegionTreatmentRequest {
    /// Reports whether this request preserves a base with exact accepted geometry and identity.
    fn is_identity(&self) -> bool {
        self.treatment
            .is_some_and(|treatment| treatment.fill == 1.0)
    }
}

/// Replays all source geometry and one-to-one ownership without numerical construction work.
fn identity_result(source: &CanonicalRegionSet) -> RegionTreatmentResult {
    RegionTreatmentResult {
        regions: source.clone(),
        provenance: source
            .regions()
            .iter()
            .map(|region| RegionTreatmentProvenance {
                treated_region_id: region.id.clone(),
                base_region_id: region.id.clone(),
            })
            .collect(),
    }
}

/// One base region's independently prepared positive components in stable source order.
struct PreparedRegionComponents {
    base_id: crate::CanonicalRegionId,
    source_id: crate::CanonicalRegionSourceId,
    components: Vec<CurvePath>,
}

/// Prepares a Scale, identity, or suppressed region without shared mutable geometry work.
///
/// This is the indexed parallel seam for region treatment. UniformOffset candidates are routed to
/// the serial shared-work helper so canonical and path-offset request budgets retain exact order.
///
/// # Errors
///
/// Returns cancellation or Scale geometry failures, or an internal routing diagnostic if a
/// nonidentity UniformOffset bypasses the shared-work path.
fn prepare_scale_region_components(
    base_id: &crate::CanonicalRegionId,
    region: &crate::CanonicalRegion,
    request: &RegionTreatmentRequest,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<PreparedRegionComponents>, RegionTreatmentError> {
    let Some(treatment) = request.treatment else {
        return Ok(None);
    };
    if treatment.fill == 0.0 {
        return Ok(None);
    }
    let components = match treatment.algorithm {
        RegionResizeAlgorithm::Scale if treatment.fill == 1.0 => vec![region.ring.clone()],
        RegionResizeAlgorithm::Scale => vec![scale_path(
            &region.ring,
            request.reference.expect("validated Scale reference"),
            treatment.fill,
            cancelled,
        )?],
        RegionResizeAlgorithm::UniformOffset if treatment.fill == 1.0 => {
            vec![region.ring.clone()]
        }
        RegionResizeAlgorithm::UniformOffset => {
            return Err(region_error(
                "region.resize.geometry.routing",
                "nonidentity UniformOffset requires shared request work",
            ));
        }
    };
    Ok(Some(PreparedRegionComponents {
        base_id: base_id.clone(),
        source_id: region.id.source_id.clone(),
        components,
    }))
}

/// Prepares one region through the serial shared-work path when UniformOffset is present.
///
/// # Errors
///
/// Returns stable Scale, offset, canonical-budget, path-budget, or cancellation diagnostics.
#[allow(clippy::too_many_arguments)]
fn prepare_region_components_serial(
    output_layer_id: PatternOutputLayerId,
    base_id: &crate::CanonicalRegionId,
    region: &crate::CanonicalRegion,
    request: &RegionTreatmentRequest,
    canonical_work: &mut CanonicalTreatmentWork,
    offset_work: &mut PathOffsetWork,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<PreparedRegionComponents>, RegionTreatmentError> {
    let Some(treatment) = request.treatment else {
        return Ok(None);
    };
    if treatment.fill == 0.0 {
        return Ok(None);
    }
    if treatment.algorithm != RegionResizeAlgorithm::UniformOffset || treatment.fill == 1.0 {
        return prepare_scale_region_components(base_id, region, request, cancelled);
    }
    let components = uniform_offset_components_for_fill(
        output_layer_id,
        region,
        treatment.fill,
        canonical_work,
        offset_work,
        cancelled,
    )?;
    Ok((!components.is_empty()).then(|| PreparedRegionComponents {
        base_id: base_id.clone(),
        source_id: region.id.source_id.clone(),
        components,
    }))
}

/// Solves the signed normal offset whose retained positive area matches `base_area * fill²`.
///
/// The solver measures only canonical positive candidate components from this one region. It does
/// not inspect adjacent regions, canvas complements, walls, or any absolute gap. Convex linear
/// rings use one exact construction; other current producer rings use a fixed shared-work fallback.
///
/// # Errors
///
/// Returns stable offset, canonicalization, finite-arithmetic, or cancellation diagnostics
/// without publishing an intermediate candidate.
fn uniform_offset_components_for_fill(
    output_layer_id: PatternOutputLayerId,
    region: &crate::CanonicalRegion,
    fill: f64,
    canonical_work: &mut CanonicalTreatmentWork,
    work: &mut PathOffsetWork,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CurvePath>, RegionTreatmentError> {
    let target_area = region.area * fill * fill;
    if !target_area.is_finite() || target_area <= 0.0 {
        return Err(region_error(
            "region.resize.geometry.area",
            "uniform offset target area must remain finite and positive",
        ));
    }
    let analytic_distance = convex_line_ring_offset_distance(region, target_area).ok();
    if let Some(signed_distance) = analytic_distance {
        return canonical_offset_components(
            output_layer_id,
            region,
            signed_distance,
            canonical_work,
            work,
            cancelled,
        )
        .map(|candidate| candidate.components);
    }
    uniform_offset_fallback_components(
        output_layer_id,
        region,
        target_area,
        canonical_work,
        work,
        cancelled,
    )
}

/// Canonical positive offset components plus their already-measured total area.
struct CanonicalOffsetCandidate {
    components: Vec<CurvePath>,
    area: f64,
}

/// Builds one positive canonical result from one shared-work offset construction.
fn canonical_offset_components(
    output_layer_id: PatternOutputLayerId,
    region: &crate::CanonicalRegion,
    signed_distance: f64,
    canonical_work: &mut CanonicalTreatmentWork,
    work: &mut PathOffsetWork,
    cancelled: &dyn Fn() -> bool,
) -> Result<CanonicalOffsetCandidate, RegionTreatmentError> {
    poll_treatment(cancelled)?;
    let components = offset_region_path_with_work(&region.ring, signed_distance, work, cancelled)?;
    if components.is_empty() {
        return Ok(CanonicalOffsetCandidate {
            components,
            area: 0.0,
        });
    }
    let (canonical, _) = canonical_work.build(
        CanonicalRegionProposal {
            output_layer_id,
            source_groups: vec![CanonicalRegionSourceGroup {
                source_id: region.id.source_id.clone(),
                components,
            }],
        },
        cancelled,
    )?;
    let area = canonical
        .regions()
        .iter()
        .map(|candidate| candidate.area)
        .sum();
    Ok(CanonicalOffsetCandidate {
        components: canonical
            .regions()
            .iter()
            .map(|candidate| candidate.ring.clone())
            .collect(),
        area,
    })
}

/// Solves nonlinear, nonconvex, or topology-changing current producer rings with shared work.
///
/// The fixed thirty-two-step bisection is private, deterministic, and deliberately
/// non-configurable. Every candidate charges the one request-wide PathOffsetWork. It accepts
/// only canonical numeric tolerance, otherwise failing atomically without a second protocol.
fn uniform_offset_fallback_components(
    output_layer_id: PatternOutputLayerId,
    region: &crate::CanonicalRegion,
    target_area: f64,
    canonical_work: &mut CanonicalTreatmentWork,
    work: &mut PathOffsetWork,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CurvePath>, RegionTreatmentError> {
    const STEPS: usize = 32;
    let grows = target_area > region.area;
    let mut low = 0.0;
    let mut high = region.area.sqrt();
    let mut best: Option<(f64, Vec<CurvePath>)> = None;
    for _ in 0..STEPS {
        poll_treatment(cancelled)?;
        let distance = (low + high) * 0.5;
        let candidate = canonical_offset_components(
            output_layer_id,
            region,
            if grows { -distance } else { distance },
            canonical_work,
            work,
            cancelled,
        )?;
        let area = candidate.area;
        let error = (area - target_area).abs();
        if best
            .as_ref()
            .is_none_or(|(best_error, _)| error < *best_error)
        {
            best = Some((error, candidate.components));
        }
        if (grows && area < target_area) || (!grows && area > target_area) {
            low = distance;
        } else {
            high = distance;
        }
    }
    let (error, components) = best.ok_or(region_error(
        "region.resize.geometry.offset",
        "UniformOffset fallback produced no positive candidate",
    ))?;
    (error <= target_area * 1.0e-6 + 1.0e-9)
        .then_some(components)
        .ok_or(region_error(
            "region.resize.geometry.offset",
            "UniformOffset fallback did not reach its bounded positive-area tolerance",
        ))
}

/// Derives one exact signed normal displacement for a convex linear canonical region.
///
/// The outer quadratic uses the actual RegionRound cubic construction: each convex corner's unit
/// cubic Green-area correction replaces the ideal circular-sector term. The inner quadratic uses
/// the same tangent-intersection corners that RegionRound retains for positive signed distances.
///
/// # Errors
///
/// Rejects nonlinear, nonconvex, disconnected, or numerically unrepresentable future producer
/// rings instead of bypassing the shared request-wide offset accounting with iterative attempts.
fn convex_line_ring_offset_distance(
    region: &crate::CanonicalRegion,
    target_area: f64,
) -> Result<f64, RegionTreatmentError> {
    let segments = region.ring.segments();
    if segments.len() < 3
        || segments
            .iter()
            .any(|segment| !matches!(segment, CurveSegment::Line(_)))
    {
        return Err(region_error(
            "region.resize.geometry.unsupported",
            "UniformOffset currently requires a convex linear producer region",
        ));
    }
    let vertices: Vec<_> = segments.iter().map(CurveSegment::start).collect();
    let mut perimeter: f64 = 0.0;
    let mut inner_quadratic: f64 = 0.0;
    let mut outer_quadratic: f64 = std::f64::consts::PI;
    for index in 0..vertices.len() {
        let previous = vertices[(index + vertices.len() - 1) % vertices.len()];
        let current = vertices[index];
        let next = vertices[(index + 1) % vertices.len()];
        let incoming = Vector2::new(current.x - previous.x, current.y - previous.y);
        let outgoing = Vector2::new(next.x - current.x, next.y - current.y);
        let incoming_length = incoming.x.hypot(incoming.y);
        let outgoing_length = outgoing.x.hypot(outgoing.y);
        let turn = incoming.x * outgoing.y - incoming.y * outgoing.x;
        if incoming_length == 0.0 || outgoing_length == 0.0 || turn <= 1.0e-12 {
            return Err(region_error(
                "region.resize.geometry.unsupported",
                "UniformOffset currently requires a strictly convex linear producer region",
            ));
        }
        perimeter += incoming_length;
        let cosine =
            (incoming.dot(outgoing) / (incoming_length * outgoing_length)).clamp(-1.0, 1.0);
        let sweep = turn.atan2(incoming.dot(outgoing));
        if !sweep.is_finite() || sweep <= 0.0 || cosine.is_nan() {
            return Err(region_error(
                "region.resize.geometry.unsupported",
                "UniformOffset producer turns must remain finite and convex",
            ));
        }
        inner_quadratic += (sweep * 0.5).tan();
        outer_quadratic += unit_region_round_arc_area(sweep) - sweep * 0.5;
    }
    if !perimeter.is_finite()
        || perimeter <= 0.0
        || !inner_quadratic.is_finite()
        || inner_quadratic <= 0.0
        || !outer_quadratic.is_finite()
        || outer_quadratic <= 0.0
    {
        return Err(region_error(
            "region.resize.geometry.unsupported",
            "UniformOffset producer metrics must remain positive and finite",
        ));
    }
    let delta = target_area - region.area;
    let distance = if delta > 0.0 {
        let discriminant = perimeter.mul_add(perimeter, 4.0 * outer_quadratic * delta);
        (discriminant.sqrt() - perimeter) / (2.0 * outer_quadratic)
    } else {
        let discriminant = perimeter.mul_add(perimeter, 4.0 * inner_quadratic * delta);
        if discriminant < 0.0 {
            return Err(region_error(
                "region.resize.geometry.unsupported",
                "UniformOffset target cannot remain a positive convex linear region",
            ));
        }
        (perimeter - discriminant.sqrt()) / (2.0 * inner_quadratic)
    };
    if !distance.is_finite() || distance < 0.0 {
        return Err(region_error(
            "region.resize.geometry.unsupported",
            "UniformOffset displacement must remain finite and nonnegative",
        ));
    }
    Ok(if delta > 0.0 { -distance } else { distance })
}

/// Integrates the exact unit-radius Green-area contribution of one RegionRound cubic arc.
fn unit_region_round_arc_area(sweep: f64) -> f64 {
    let count = (sweep / std::f64::consts::FRAC_PI_2).ceil() as usize;
    let step = sweep / count.max(1) as f64;
    (0..count.max(1))
        .map(|_| unit_region_round_arc_step_area(step))
        .sum()
}

/// Integrates one RegionRound subdivision step using its production cubic handle construction.
fn unit_region_round_arc_step_area(sweep: f64) -> f64 {
    let handle = 4.0 / 3.0 * (sweep * 0.25).tan();
    let points = [
        Point2::new(1.0, 0.0),
        Point2::new(1.0, handle),
        Point2::new(
            sweep.cos() + sweep.sin() * handle,
            sweep.sin() - sweep.cos() * handle,
        ),
        Point2::new(sweep.cos(), sweep.sin()),
    ];
    let x = [
        points[0].x,
        3.0 * (points[1].x - points[0].x),
        3.0 * (points[0].x - 2.0 * points[1].x + points[2].x),
        -points[0].x + 3.0 * points[1].x - 3.0 * points[2].x + points[3].x,
    ];
    let y = [
        points[0].y,
        3.0 * (points[1].y - points[0].y),
        3.0 * (points[0].y - 2.0 * points[1].y + points[2].y),
        -points[0].y + 3.0 * points[1].y - 3.0 * points[2].y + points[3].y,
    ];
    let mut integral = 0.0;
    for first in 0..4 {
        for second in 1..4 {
            integral += (x[first] * y[second] - y[first] * x[second]) * second as f64
                / (first + second) as f64;
        }
    }
    0.5 * integral
}

/// Builds one signed normal-offset result while charging the request-wide reusable work budget.
///
/// # Errors
///
/// Maps reusable offset failures into the positive-region resize boundary atomically.
fn offset_region_path_with_work(
    path: &CurvePath,
    signed_distance: f64,
    work: &mut PathOffsetWork,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CurvePath>, RegionTreatmentError> {
    if !signed_distance.is_finite() {
        return Err(region_error(
            "region.resize.geometry.offset",
            "uniform offset distance must remain finite",
        ));
    }
    if signed_distance == 0.0 {
        return Ok(vec![path.clone()]);
    }
    match offset_path_with_work_region_round_cancellable(
        PathOffsetRequest {
            path,
            signed_distance,
            endpoint_policy: PathOffsetEndpointPolicy::Preserve,
            cleanup: PathOffsetCleanup::DissolveCrossings,
            crossing_barriers: &[],
            limits: work.limits(),
        },
        work,
        cancelled,
    ) {
        Ok(PathOffsetResult::Paths(components)) => Ok(components
            .into_iter()
            .filter_map(|component| {
                (component.path.closure() == PathClosure::Closed).then_some(component.path)
            })
            .collect()),
        Ok(PathOffsetResult::Collapsed) => Ok(Vec::new()),
        Err(error) => Err(region_error(
            match error.path() {
                "evaluation.cancelled" => "evaluation.cancelled",
                path if path.contains("limit") => "region.resize.limits.offset",
                path if path.contains("allocation") => "region.resize.allocation.offset",
                _ => "region.resize.geometry.offset",
            },
            error.message(),
        )),
    }
}

/// Polls cancellation at a resize-owned boundary.
///
/// # Errors
///
/// Returns only the canonical evaluation cancellation diagnostic.
fn poll_treatment(cancelled: &dyn Fn() -> bool) -> Result<(), RegionTreatmentError> {
    (!cancelled())
        .then_some(())
        .ok_or(region_error("evaluation.cancelled", "evaluation cancelled"))
}

/// Maps canonical post-processing failures into the positive-region resize diagnostic namespace.
fn map_canonical_treatment_error(error: crate::CanonicalRegionError) -> RegionTreatmentError {
    region_error(
        match error.path() {
            "evaluation.cancelled" => "evaluation.cancelled",
            path if path.starts_with("region.limits") => "region.resize.limits.canonical",
            path if path.starts_with("region.allocation") => "region.resize.allocation.canonical",
            _ => "region.resize.geometry.canonical",
        },
        error.message(),
    )
}

/// Constructs one stable resize failure without exposing partial geometry.
fn region_error(path: &'static str, message: &'static str) -> RegionTreatmentError {
    RegionTreatmentError { path, message }
}

/// Affinely transforms every line or cubic construction point around a finite producer reference.
///
/// # Errors
///
/// Returns cancellation or geometry validation failures without changing closure or segment kind.
fn scale_path(
    path: &CurvePath,
    reference: Point2,
    factor: f64,
    cancelled: &dyn Fn() -> bool,
) -> Result<CurvePath, RegionTreatmentError> {
    let point = |value: Point2| {
        Point2::new(
            reference.x + (value.x - reference.x) * factor,
            reference.y + (value.y - reference.y) * factor,
        )
    };
    let mut segments = Vec::with_capacity(path.segments().len());
    for segment in path.segments() {
        poll_treatment(cancelled)?;
        let transformed = match segment {
            CurveSegment::Line(line) => CurveSegment::Line(
                LineSegment::new(point(line.start()), point(line.end())).map_err(|_| {
                    region_error(
                        "region.resize.geometry.scale",
                        "scaled line coordinates must remain finite",
                    )
                })?,
            ),
            CurveSegment::CubicBezier(cubic) => CurveSegment::CubicBezier(
                CubicBezierSegment::new(
                    point(cubic.start()),
                    point(cubic.control_1()),
                    point(cubic.control_2()),
                    point(cubic.end()),
                )
                .map_err(|_| {
                    region_error(
                        "region.resize.geometry.scale",
                        "scaled cubic coordinates must remain finite",
                    )
                })?,
            ),
        };
        segments.push(transformed);
    }
    CurvePath::new(segments, PathClosure::Closed).map_err(|_| {
        region_error(
            "region.resize.geometry.scale",
            "scaled region must remain closed and connected",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_domain::PatternMechanismId;

    /// Builds one canonical positive square with its producer identity.
    fn source() -> CanonicalRegionSet {
        build_canonical_regions_cancellable(
            CanonicalRegionProposal {
                output_layer_id: PatternOutputLayerId(71),
                source_groups: vec![CanonicalRegionSourceGroup {
                    source_id: crate::CanonicalRegionSourceId::SiteOwners(vec![
                        crate::FamilySiteId {
                            mechanism_id: PatternMechanismId(9),
                            ordinal: 0,
                        },
                    ]),
                    components: vec![
                        CurvePath::polyline(
                            vec![
                                Point2::new(0.0, 0.0),
                                Point2::new(2.0, 0.0),
                                Point2::new(2.0, 2.0),
                                Point2::new(0.0, 2.0),
                            ],
                            PathClosure::Closed,
                        )
                        .unwrap(),
                    ],
                }],
            },
            CanonicalRegionLimits::default(),
            || false,
        )
        .unwrap()
        .0
    }

    /// Builds two positive squares whose intermediate and final canonical builds share one budget.
    fn two_region_source() -> CanonicalRegionSet {
        build_canonical_regions_cancellable(
            CanonicalRegionProposal {
                output_layer_id: PatternOutputLayerId(71),
                source_groups: (0_usize..2)
                    .map(|ordinal| CanonicalRegionSourceGroup {
                        source_id: crate::CanonicalRegionSourceId::SiteOwners(vec![
                            crate::FamilySiteId {
                                mechanism_id: PatternMechanismId(9),
                                ordinal,
                            },
                        ]),
                        components: vec![
                            CurvePath::polyline(
                                vec![
                                    Point2::new(ordinal as f64 * 4.0, 0.0),
                                    Point2::new(ordinal as f64 * 4.0 + 2.0, 0.0),
                                    Point2::new(ordinal as f64 * 4.0 + 2.0, 2.0),
                                    Point2::new(ordinal as f64 * 4.0, 2.0),
                                ],
                                PathClosure::Closed,
                            )
                            .expect("square closes"),
                        ],
                    })
                    .collect(),
            },
            CanonicalRegionLimits::default(),
            || false,
        )
        .expect("two-region source canonicalizes")
        .0
    }

    /// Builds one request for the supplied normalized algorithm and fill.
    fn request(
        region: &crate::CanonicalRegion,
        algorithm: RegionResizeAlgorithm,
        fill: f64,
    ) -> RegionTreatmentRequest {
        RegionTreatmentRequest {
            base_region_id: region.id.clone(),
            reference: Some(Point2::new(1.0, 1.0)),
            treatment: Some(RegionTreatment { algorithm, fill }),
        }
    }

    /// Verifies fill zero omits positive geometry before canonical output construction.
    #[test]
    fn zero_fill_omits_the_region() {
        let source = source();
        let result = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &[request(
                &source.regions()[0],
                RegionResizeAlgorithm::Scale,
                0.0,
            )],
            RegionTreatmentLimits::default(),
            || false,
        )
        .unwrap();
        assert!(result.regions.regions().is_empty());
        assert!(result.provenance.is_empty());
    }

    /// Enforces canonical source-group work across intermediate offsets and final aggregation.
    #[test]
    fn uniform_offset_charges_one_request_wide_canonical_budget() {
        let source = two_region_source();
        let requests = source
            .regions()
            .iter()
            .map(|region| request(region, RegionResizeAlgorithm::UniformOffset, 1.2))
            .collect::<Vec<_>>();
        let error = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &requests,
            RegionTreatmentLimits {
                canonical: CanonicalRegionLimits::new(3, 100, 1_000, 100_000)
                    .expect("nonzero limits"),
                path_offset: crate::PathOffsetLimits::default(),
            },
            || false,
        )
        .expect_err("two intermediate groups plus two final groups exceed three");
        assert_eq!(error.path(), "region.resize.limits.canonical");
    }

    /// Proves indexed Scale treatment preserves canonical order, identity, and provenance.
    #[test]
    fn parallel_scale_treatment_matches_single_worker_reference() {
        let source = two_region_source();
        let requests = source
            .regions()
            .iter()
            .map(|region| request(region, RegionResizeAlgorithm::Scale, 1.25))
            .collect::<Vec<_>>();
        let run = || {
            treat_region_requests_cancellable(
                PatternOutputLayerId(71),
                &source,
                &requests,
                RegionTreatmentLimits::default(),
                || false,
            )
            .expect("Scale treatment completes")
        };
        let one = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("one-worker pool builds")
            .install(run);
        let many = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .expect("four-worker pool builds")
            .install(run);
        assert_eq!(one, many);
    }

    /// Proves indexed Scale work observes cancellation before any treated region set publishes.
    #[test]
    fn parallel_scale_treatment_cancellation_is_atomic() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let source = two_region_source();
        let requests = source
            .regions()
            .iter()
            .map(|region| request(region, RegionResizeAlgorithm::Scale, 1.25))
            .collect::<Vec<_>>();
        let polls = AtomicUsize::new(0);
        let error = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &requests,
            RegionTreatmentLimits::default(),
            || polls.fetch_add(1, Ordering::Relaxed) >= 6,
        )
        .expect_err("cancelled Scale treatment publishes no partial region set");
        assert_eq!(error.path(), "evaluation.cancelled");
    }

    /// Verifies both algorithms replay the exact natural producer boundary at fill one.
    #[test]
    fn unit_fill_replays_the_exact_natural_boundary() {
        let source = source();
        for algorithm in [
            RegionResizeAlgorithm::Scale,
            RegionResizeAlgorithm::UniformOffset,
        ] {
            let result = treat_region_requests_cancellable(
                PatternOutputLayerId(71),
                &source,
                &[request(&source.regions()[0], algorithm, 1.0)],
                RegionTreatmentLimits::default(),
                || false,
            )
            .unwrap();
            assert_eq!(result.regions, source);
        }
    }

    /// Verifies Scale fill two quadruples positive area around the producer reference.
    #[test]
    fn scale_fill_two_doubles_the_geometric_radius() {
        let source = source();
        let result = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &[request(
                &source.regions()[0],
                RegionResizeAlgorithm::Scale,
                2.0,
            )],
            RegionTreatmentLimits::default(),
            || false,
        )
        .unwrap();
        assert_eq!(
            result.regions.regions()[0].area,
            source.regions()[0].area * 4.0
        );
    }

    /// Verifies UniformOffset fill two deterministically targets four times the positive area.
    #[test]
    fn uniform_offset_fill_two_doubles_the_geometric_radius() {
        let source = source();
        let first = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &[request(
                &source.regions()[0],
                RegionResizeAlgorithm::UniformOffset,
                2.0,
            )],
            RegionTreatmentLimits::default(),
            || false,
        )
        .unwrap();
        let second = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &[request(
                &source.regions()[0],
                RegionResizeAlgorithm::UniformOffset,
                2.0,
            )],
            RegionTreatmentLimits::default(),
            || false,
        )
        .unwrap();
        assert!((first.regions.regions()[0].area - source.regions()[0].area * 4.0).abs() < 1e-6);
        assert_eq!(first, second);
    }

    /// Verifies cancellation prevents the UniformOffset solver from publishing a partial result.
    #[test]
    fn uniform_offset_cancellation_is_atomic() {
        let source = source();
        let error = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &[request(
                &source.regions()[0],
                RegionResizeAlgorithm::UniformOffset,
                0.5,
            )],
            RegionTreatmentLimits::default(),
            || true,
        )
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
    }
}
