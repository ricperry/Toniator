//! Canonical, positive-area closed-region authority shared by future region producers and renderers.

use std::{cmp::Ordering, error::Error, fmt};

use toniator_domain::PatternOutputLayerId;

use crate::{
    Bounds, CubicBezierSegment, CurvePath, CurveSegment, FamilySiteId, LineSegment, PathClosure,
    Point2, StructuralPathLocationProvenance,
};

/// Fixed identity for the Stage 20N canonical-region construction contract.
pub const CANONICAL_REGION_CONTRACT_ID: &str = "toniator.canonical-region.v1";
/// Default maximum source groups accepted by one atomic canonical-region build.
pub const DEFAULT_MAX_REGION_SOURCE_GROUPS: usize = 1_048_576;
/// Default maximum retained canonical region components accepted by one build.
pub const DEFAULT_MAX_REGIONS: usize = 1_048_576;
/// Default maximum retained curve segments accepted by one build.
pub const DEFAULT_MAX_REGION_SEGMENTS: usize = 8_388_608;
/// Default maximum validation, ordering, and intersection inspections accepted by one build.
pub const DEFAULT_MAX_REGION_INSPECTIONS: usize = 67_108_864;

/// Authored/derived identity supplied by a future region producer; this module never invents it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CanonicalRegionSourceId {
    /// A canonical region is jointly owned by sorted family-site identities.
    SiteOwners(Vec<FamilySiteId>),
    /// A canonical region is identified by sorted structural-boundary locations.
    GuideBoundary(Vec<StructuralPathLocationProvenance>),
}

/// Stable identity of one positive connected region component.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalRegionId {
    /// The structural output whose contract owns this region.
    pub output_layer_id: PatternOutputLayerId,
    /// The producer-supplied stable source identity.
    pub source_id: CanonicalRegionSourceId,
    /// Ordered component index within the sorted source group.
    pub component_ordinal: u32,
}

/// One validated, counter-clockwise, closed, hole-free canonical region.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalRegion {
    /// Stable output/source/component identity.
    pub id: CanonicalRegionId,
    /// Closed finite canonical ring in deterministic segment order.
    pub ring: CurvePath,
    /// Exact signed Cartesian area, guaranteed finite and strictly positive.
    pub area: f64,
    /// Exact finite bounds of the canonical ring.
    pub bounds: Bounds,
}

/// Complete atomic ordered output of a canonical-region build.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalRegionSet {
    regions: Vec<CanonicalRegion>,
    fingerprint: String,
}

/// One producer-supplied source identity and one or more closed candidate components.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalRegionSourceGroup {
    /// Future producer-owned identity; it must be nonempty, sorted, and unique for its kind.
    pub source_id: CanonicalRegionSourceId,
    /// One or more independent complete candidate rings for the source identity.
    pub components: Vec<CurvePath>,
}

/// Input for one atomic canonical-region build.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalRegionProposal {
    /// Output identity propagated into every retained region ID.
    pub output_layer_id: PatternOutputLayerId,
    /// Producer-supplied groups; the builder canonicalizes their order.
    pub source_groups: Vec<CanonicalRegionSourceGroup>,
}

/// Configurable bounded-work limits for one canonical-region build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanonicalRegionLimits {
    max_source_groups: usize,
    max_regions: usize,
    max_segments: usize,
    max_inspections: usize,
}

/// Stable cancellation, identity, geometry, allocation, and bounded-work error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalRegionError {
    path: &'static str,
    message: &'static str,
}

/// Non-authoritative bounded-work facts deliberately excluded from fingerprints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CanonicalRegionDiagnostics {
    /// Source groups accepted before deterministic canonical ordering.
    pub source_groups: usize,
    /// Retained positive canonical components.
    pub regions: usize,
    /// Retained canonical ring segments.
    pub segments: usize,
    /// Validation/order/intersection inspections consumed by the complete build.
    pub inspections: usize,
}

impl CanonicalRegionLimits {
    /// Constructs explicit nonzero bounded-work limits.
    ///
    /// # Errors
    ///
    /// Returns `region.limits.zero` when any caller-supplied limit is zero.
    pub fn new(
        max_source_groups: usize,
        max_regions: usize,
        max_segments: usize,
        max_inspections: usize,
    ) -> Result<Self, CanonicalRegionError> {
        if [
            max_source_groups,
            max_regions,
            max_segments,
            max_inspections,
        ]
        .into_iter()
        .any(|value| value == 0)
        {
            return Err(CanonicalRegionError::new(
                "region.limits.zero",
                "canonical-region limits must be nonzero",
            ));
        }
        Ok(Self {
            max_source_groups,
            max_regions,
            max_segments,
            max_inspections,
        })
    }

    /// Returns the configured maximum producer source groups.
    pub const fn max_source_groups(self) -> usize {
        self.max_source_groups
    }
    /// Returns the configured maximum retained components.
    pub const fn max_regions(self) -> usize {
        self.max_regions
    }
    /// Returns the configured maximum retained ring segments.
    pub const fn max_segments(self) -> usize {
        self.max_segments
    }
    /// Returns the configured maximum inspection count.
    pub const fn max_inspections(self) -> usize {
        self.max_inspections
    }
}

impl Default for CanonicalRegionLimits {
    /// Supplies the accepted Stage 20N finite default limits.
    fn default() -> Self {
        Self {
            max_source_groups: DEFAULT_MAX_REGION_SOURCE_GROUPS,
            max_regions: DEFAULT_MAX_REGIONS,
            max_segments: DEFAULT_MAX_REGION_SEGMENTS,
            max_inspections: DEFAULT_MAX_REGION_INSPECTIONS,
        }
    }
}

impl CanonicalRegionError {
    /// Creates a stable canonical-region diagnostic without exposing partial output.
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }
    /// Returns the stable diagnostic path.
    pub const fn path(&self) -> &'static str {
        self.path
    }
    /// Returns the stable diagnostic message.
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for CanonicalRegionError {
    /// Formats the stable canonical-region diagnostic.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for CanonicalRegionError {}

impl CanonicalRegionSet {
    /// Returns the complete ordered canonical regions; no partial result is ever exposed.
    pub fn regions(&self) -> &[CanonicalRegion] {
        &self.regions
    }
    /// Returns the deterministic geometry identity excluding diagnostics and limits.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// Builds complete positive canonical regions or returns one stable error without partial output.
///
/// The cancellation callback is polled during validation, intersection work, sorting, and hashing.
///
/// # Errors
///
/// Returns `evaluation.cancelled` for cancellation and `region.identity.*`, `region.geometry.*`,
/// `region.limits.*`, or `region.allocation.*` for all other rejected candidate state.
pub fn build_canonical_regions_cancellable(
    proposal: CanonicalRegionProposal,
    limits: CanonicalRegionLimits,
    cancelled: impl Fn() -> bool,
) -> Result<(CanonicalRegionSet, CanonicalRegionDiagnostics), CanonicalRegionError> {
    let mut work = RegionWork::new(limits, &cancelled);
    if proposal.output_layer_id.0 == 0 {
        return Err(CanonicalRegionError::new(
            "region.identity.output",
            "canonical regions require a nonzero output-layer ID",
        ));
    }
    if proposal.source_groups.is_empty() {
        return Err(CanonicalRegionError::new(
            "region.identity.groups",
            "canonical regions require at least one source group",
        ));
    }
    if proposal.source_groups.len() > limits.max_source_groups {
        return Err(CanonicalRegionError::new(
            "region.limits.source_groups",
            "canonical-region source-group limit exceeded",
        ));
    }
    let source_group_count = proposal.source_groups.len();
    let mut groups = proposal.source_groups;
    for group in &groups {
        validate_source_id(&group.source_id)?;
        work.poll()?;
    }
    groups.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    if groups
        .windows(2)
        .any(|pair| pair[0].source_id == pair[1].source_id)
    {
        return Err(CanonicalRegionError::new(
            "region.identity.duplicate",
            "canonical-region source identities must be unique",
        ));
    }
    let mut regions = Vec::new();
    for group in groups {
        if group.components.is_empty() {
            return Err(CanonicalRegionError::new(
                "region.geometry.components",
                "canonical-region source groups require at least one component",
            ));
        }
        let mut canonical_components = Vec::new();
        for component in group.components {
            canonical_components.push(canonicalize_ring(component, &mut work)?);
        }
        canonical_components.sort_by(canonical_component_order);
        for (ordinal, (ring, area, bounds)) in canonical_components.into_iter().enumerate() {
            if ordinal > u32::MAX as usize {
                return Err(CanonicalRegionError::new(
                    "region.allocation.ordinal",
                    "canonical-region component ordinal exceeds u32",
                ));
            }
            if regions.len() >= limits.max_regions {
                return Err(CanonicalRegionError::new(
                    "region.limits.regions",
                    "canonical-region retained-region limit exceeded",
                ));
            }
            regions.push(CanonicalRegion {
                id: CanonicalRegionId {
                    output_layer_id: proposal.output_layer_id,
                    source_id: group.source_id.clone(),
                    component_ordinal: ordinal as u32,
                },
                ring,
                area,
                bounds,
            });
        }
    }
    regions.sort_by(|left, right| left.id.cmp(&right.id));
    work.poll()?;
    let fingerprint = region_fingerprint(&regions, &mut work)?;
    let diagnostics = CanonicalRegionDiagnostics {
        source_groups: source_group_count,
        regions: regions.len(),
        segments: work.segments,
        inspections: work.inspections,
    };
    Ok((
        CanonicalRegionSet {
            regions,
            fingerprint,
        },
        diagnostics,
    ))
}

/// Builds complete canonical regions with the accepted default limits and no cancellation request.
///
/// # Errors
///
/// Propagates every stable canonical-region validation, identity, allocation, and limit failure.
pub fn build_canonical_regions(
    proposal: CanonicalRegionProposal,
) -> Result<CanonicalRegionSet, CanonicalRegionError> {
    build_canonical_regions_cancellable(proposal, CanonicalRegionLimits::default(), || false)
        .map(|(set, _)| set)
}

/// Tracks bounded work and polls cancellation without publishing a candidate result.
struct RegionWork<'a, F: Fn() -> bool> {
    limits: CanonicalRegionLimits,
    cancelled: &'a F,
    inspections: usize,
    segments: usize,
}

impl<'a, F: Fn() -> bool> RegionWork<'a, F> {
    /// Creates an empty bounded-work tracker for one atomic request.
    fn new(limits: CanonicalRegionLimits, cancelled: &'a F) -> Self {
        Self {
            limits,
            cancelled,
            inspections: 0,
            segments: 0,
        }
    }
    /// Polls cancellation and consumes one bounded inspection.
    fn poll(&mut self) -> Result<(), CanonicalRegionError> {
        if (self.cancelled)() {
            return Err(CanonicalRegionError::new(
                "evaluation.cancelled",
                "canonical-region evaluation was cancelled",
            ));
        }
        self.inspections = self
            .inspections
            .checked_add(1)
            .ok_or(CanonicalRegionError::new(
                "region.allocation.inspections",
                "canonical-region inspection counter overflowed",
            ))?;
        if self.inspections > self.limits.max_inspections {
            return Err(CanonicalRegionError::new(
                "region.limits.inspections",
                "canonical-region inspection limit exceeded",
            ));
        }
        Ok(())
    }
}

/// Validates the complete supplied identity before geometry is considered.
fn validate_source_id(source: &CanonicalRegionSourceId) -> Result<(), CanonicalRegionError> {
    match source {
        CanonicalRegionSourceId::SiteOwners(ids) => {
            if ids.is_empty()
                || ids.iter().any(|id| id.mechanism_id.0 == 0)
                || ids.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(CanonicalRegionError::new(
                    "region.identity.site_owners",
                    "site-owner identities must be nonempty, nonzero, sorted, and unique",
                ));
            }
        }
        CanonicalRegionSourceId::GuideBoundary(locations) => {
            if locations.is_empty()
                || locations
                    .iter()
                    .any(|location| !location.path.source.is_valid())
                || locations.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(CanonicalRegionError::new(
                    "region.identity.guide_boundary",
                    "guide-boundary identities must be nonempty, valid, sorted, and unique",
                ));
            }
        }
    }
    Ok(())
}

/// Validates, normalizes, orients, and rotates one complete supplied closed ring.
fn canonicalize_ring<F: Fn() -> bool>(
    path: CurvePath,
    work: &mut RegionWork<'_, F>,
) -> Result<(CurvePath, f64, Bounds), CanonicalRegionError> {
    work.poll()?;
    if path.closure() != PathClosure::Closed || path.start() != path.end() {
        return Err(CanonicalRegionError::new(
            "region.geometry.closure",
            "canonical regions require exactly closed paths",
        ));
    }
    let mut segments = path
        .segments()
        .iter()
        .copied()
        .map(normalize_segment)
        .collect::<Result<Vec<_>, _>>()?;
    if segments.iter().any(is_zero_segment) {
        return Err(CanonicalRegionError::new(
            "region.geometry.zero_length",
            "canonical-region rings cannot contain zero-length segments",
        ));
    }
    work.segments = work
        .segments
        .checked_add(segments.len())
        .ok_or(CanonicalRegionError::new(
            "region.allocation.segments",
            "canonical-region segment counter overflowed",
        ))?;
    if work.segments > work.limits.max_segments {
        return Err(CanonicalRegionError::new(
            "region.limits.segments",
            "canonical-region retained-segment limit exceeded",
        ));
    }
    for first in 0..segments.len() {
        for second in first + 1..segments.len() {
            work.poll()?;
            if adjacent(first, second, segments.len()) {
                continue;
            }
            if !segments[first]
                .intersections(&segments[second])
                .map_err(|_| {
                    CanonicalRegionError::new(
                        "region.geometry.intersection",
                        "canonical-region intersection evaluation failed",
                    )
                })?
                .is_empty()
            {
                return Err(CanonicalRegionError::new(
                    "region.geometry.self_crossing",
                    "canonical-region rings cannot self-cross, overlap, or nonadjacently touch",
                ));
            }
        }
    }
    let mut area = ring_area(&segments)?;
    if area == 0.0 {
        return Err(CanonicalRegionError::new(
            "region.geometry.zero_area",
            "canonical-region rings require strictly positive area",
        ));
    }
    if area < 0.0 {
        segments = reverse_segments(&segments)?;
        area = -area;
    }
    rotate_canonical_start(&mut segments, work)?;
    let ring = CurvePath::new(segments, PathClosure::Closed).map_err(|_| {
        CanonicalRegionError::new(
            "region.geometry.connected",
            "canonical-region rings must remain finite and connected",
        )
    })?;
    let bounds = exact_ring_bounds(&ring)?;
    Ok((ring, area, bounds))
}

/// Computes unexpanded analytical line/cubic extrema bounds for one canonical ring.
///
/// Region identity requires exact construction-derived bounds, rather than the conservative
/// tolerance-expanded bounds used by generic curve operations for intersection safety.
///
/// # Errors
///
/// Returns the stable finite-bounds diagnostic when derivative arithmetic cannot remain finite.
fn exact_ring_bounds(ring: &CurvePath) -> Result<Bounds, CanonicalRegionError> {
    let mut points = Vec::new();
    for segment in ring.segments() {
        match segment {
            CurveSegment::Line(line) => points.extend([line.start(), line.end()]),
            CurveSegment::CubicBezier(cubic) => {
                points.extend([cubic.start(), cubic.end()]);
                for parameter in cubic_extrema(
                    cubic.start().x,
                    cubic.control_1().x,
                    cubic.control_2().x,
                    cubic.end().x,
                )?
                .into_iter()
                .chain(cubic_extrema(
                    cubic.start().y,
                    cubic.control_1().y,
                    cubic.control_2().y,
                    cubic.end().y,
                )?) {
                    points.push(segment.point_at(parameter).map_err(|_| {
                        CanonicalRegionError::new(
                            "region.geometry.bounds",
                            "canonical-region bounds must remain finite",
                        )
                    })?);
                }
            }
        }
    }
    Bounds::from_points(points).ok_or(CanonicalRegionError::new(
        "region.geometry.bounds",
        "canonical-region bounds must remain finite",
    ))
}

/// Returns strict unit-interval derivative roots for one cubic coordinate without tolerance expansion.
///
/// # Errors
///
/// Returns the stable finite-bounds diagnostic when the coordinate polynomial overflows.
fn cubic_extrema(
    first: f64,
    second: f64,
    third: f64,
    fourth: f64,
) -> Result<Vec<f64>, CanonicalRegionError> {
    let a = -first + 3.0 * second - 3.0 * third + fourth;
    let b = 2.0 * (first - 2.0 * second + third);
    let c = second - first;
    if !(a.is_finite() && b.is_finite() && c.is_finite()) {
        return Err(CanonicalRegionError::new(
            "region.geometry.bounds",
            "canonical-region bounds must remain finite",
        ));
    }
    let mut roots = Vec::new();
    if a == 0.0 {
        if b != 0.0 {
            roots.push(-c / b);
        }
    } else {
        let discriminant = b.mul_add(b, -4.0 * a * c);
        if !discriminant.is_finite() {
            return Err(CanonicalRegionError::new(
                "region.geometry.bounds",
                "canonical-region bounds must remain finite",
            ));
        }
        if discriminant >= 0.0 {
            let root = discriminant.sqrt();
            roots.extend([(-b + root) / (2.0 * a), (-b - root) / (2.0 * a)]);
        }
    }
    Ok(roots
        .into_iter()
        .filter(|value| value.is_finite() && *value > 0.0 && *value < 1.0)
        .collect())
}

/// Tests whether two segment indexes meet through one authored ring neighbor relation.
fn adjacent(first: usize, second: usize, count: usize) -> bool {
    second == first + 1 || (first == 0 && second + 1 == count)
}

/// Converts negative zero to positive zero while retaining finite line/cubic construction kind.
fn normalize_segment(segment: CurveSegment) -> Result<CurveSegment, CanonicalRegionError> {
    let point = |point: Point2| {
        Point2::new(
            if point.x == 0.0 { 0.0 } else { point.x },
            if point.y == 0.0 { 0.0 } else { point.y },
        )
    };
    match segment {
        CurveSegment::Line(line) => LineSegment::new(point(line.start()), point(line.end()))
            .map(CurveSegment::Line)
            .map_err(|_| {
                CanonicalRegionError::new(
                    "region.geometry.finite",
                    "canonical-region coordinates must be finite",
                )
            }),
        CurveSegment::CubicBezier(cubic) => CubicBezierSegment::new(
            point(cubic.start()),
            point(cubic.control_1()),
            point(cubic.control_2()),
            point(cubic.end()),
        )
        .map(CurveSegment::CubicBezier)
        .map_err(|_| {
            CanonicalRegionError::new(
                "region.geometry.finite",
                "canonical-region coordinates must be finite",
            )
        }),
    }
}

/// Rejects exact degenerate line segments and cubics with no construction extent.
fn is_zero_segment(segment: &CurveSegment) -> bool {
    match segment {
        CurveSegment::Line(line) => line.start() == line.end(),
        CurveSegment::CubicBezier(cubic) => {
            cubic.start() == cubic.control_1()
                && cubic.start() == cubic.control_2()
                && cubic.start() == cubic.end()
        }
    }
}

/// Integrates the exact polynomial line/cubic Green-area contribution with finite arithmetic.
fn ring_area(segments: &[CurveSegment]) -> Result<f64, CanonicalRegionError> {
    let mut area = 0.0;
    for segment in segments {
        let value = segment_area(*segment)?;
        area += value;
        if !area.is_finite() {
            return Err(CanonicalRegionError::new(
                "region.geometry.area",
                "canonical-region area must remain finite",
            ));
        }
    }
    Ok(area)
}

/// Integrates one Bernstein line/cubic contribution to signed Cartesian area exactly in f64 arithmetic.
fn segment_area(segment: CurveSegment) -> Result<f64, CanonicalRegionError> {
    let points = match segment {
        CurveSegment::Line(line) => [line.start(), line.end(), line.end(), line.end()],
        CurveSegment::CubicBezier(cubic) => [
            cubic.start(),
            cubic.control_1(),
            cubic.control_2(),
            cubic.end(),
        ],
    };
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
    for i in 0..4 {
        for j in 1..4 {
            integral += (x[i] * y[j] - y[i] * x[j]) * j as f64 / (i + j) as f64;
        }
    }
    let result = 0.5 * integral;
    result
        .is_finite()
        .then_some(result)
        .ok_or(CanonicalRegionError::new(
            "region.geometry.area",
            "canonical-region area must remain finite",
        ))
}

/// Reverses a ring while preserving each curve's construction geometry exactly.
fn reverse_segments(segments: &[CurveSegment]) -> Result<Vec<CurveSegment>, CanonicalRegionError> {
    segments
        .iter()
        .rev()
        .map(|segment| {
            match segment {
                CurveSegment::Line(line) => {
                    LineSegment::new(line.end(), line.start()).map(CurveSegment::Line)
                }
                CurveSegment::CubicBezier(cubic) => CubicBezierSegment::new(
                    cubic.end(),
                    cubic.control_2(),
                    cubic.control_1(),
                    cubic.start(),
                )
                .map(CurveSegment::CubicBezier),
            }
            .map_err(|_| {
                CanonicalRegionError::new(
                    "region.geometry.finite",
                    "canonical-region reversal must remain finite",
                )
            })
        })
        .collect()
}

/// Rotates a CCW ring to its lexicographically smallest anchor and cyclic segment tie break.
fn rotate_canonical_start<F: Fn() -> bool>(
    segments: &mut [CurveSegment],
    work: &mut RegionWork<'_, F>,
) -> Result<(), CanonicalRegionError> {
    let mut best = 0;
    for candidate in 1..segments.len() {
        work.poll()?;
        if compare_rotation(segments, candidate, best) == Ordering::Less {
            best = candidate;
        }
    }
    segments.rotate_left(best);
    Ok(())
}

/// Compares two cyclic encodings by anchor then exact segment construction bits.
fn compare_rotation(segments: &[CurveSegment], left: usize, right: usize) -> Ordering {
    for offset in 0..segments.len() {
        let ordering = segment_key(segments[(left + offset) % segments.len()])
            .cmp(&segment_key(segments[(right + offset) % segments.len()]));
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

/// Produces a total-orderable exact construction encoding for one segment.
fn segment_key(segment: CurveSegment) -> Vec<u64> {
    match segment {
        CurveSegment::Line(line) => vec![
            0,
            line.start().x.to_bits(),
            line.start().y.to_bits(),
            line.end().x.to_bits(),
            line.end().y.to_bits(),
        ],
        CurveSegment::CubicBezier(cubic) => vec![
            1,
            cubic.start().x.to_bits(),
            cubic.start().y.to_bits(),
            cubic.control_1().x.to_bits(),
            cubic.control_1().y.to_bits(),
            cubic.control_2().x.to_bits(),
            cubic.control_2().y.to_bits(),
            cubic.end().x.to_bits(),
            cubic.end().y.to_bits(),
        ],
    }
}

/// Orders canonical components by exact cyclic segments, then area and bounds for total determinism.
fn canonical_component_order(
    left: &(CurvePath, f64, Bounds),
    right: &(CurvePath, f64, Bounds),
) -> Ordering {
    let left_key: Vec<_> = left
        .0
        .segments()
        .iter()
        .copied()
        .flat_map(segment_key)
        .collect();
    let right_key: Vec<_> = right
        .0
        .segments()
        .iter()
        .copied()
        .flat_map(segment_key)
        .collect();
    left_key
        .cmp(&right_key)
        .then_with(|| left.1.total_cmp(&right.1))
}

/// Hashes complete canonical identity and exact geometry while excluding limits and diagnostics.
fn region_fingerprint<F: Fn() -> bool>(
    regions: &[CanonicalRegion],
    work: &mut RegionWork<'_, F>,
) -> Result<String, CanonicalRegionError> {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    add_bytes(&mut hash, CANONICAL_REGION_CONTRACT_ID.bytes());
    for region in regions {
        work.poll()?;
        add_bytes(&mut hash, region.id.output_layer_id.0.to_le_bytes());
        hash_source(&mut hash, &region.id.source_id);
        add_bytes(&mut hash, region.id.component_ordinal.to_le_bytes());
        add_bytes(&mut hash, region.area.to_bits().to_le_bytes());
        for value in [
            region.bounds.min.x,
            region.bounds.min.y,
            region.bounds.max.x,
            region.bounds.max.y,
        ] {
            add_bytes(&mut hash, value.to_bits().to_le_bytes());
        }
        for segment in region.ring.segments() {
            for value in segment_key(*segment) {
                add_bytes(&mut hash, value.to_le_bytes());
            }
        }
    }
    Ok(format!("{hash:016x}"))
}

/// Adds bytes to the fixed FNV-1a identity stream.
fn add_bytes(hash: &mut u64, bytes: impl IntoIterator<Item = u8>) {
    for byte in bytes {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x100_0000_01b3);
    }
}

/// Adds stable typed source identity to the canonical-region fingerprint.
fn hash_source(hash: &mut u64, source: &CanonicalRegionSourceId) {
    match source {
        CanonicalRegionSourceId::SiteOwners(ids) => {
            add_bytes(hash, [1]);
            for id in ids {
                add_bytes(hash, id.mechanism_id.0.to_le_bytes());
                add_bytes(hash, (id.ordinal as u64).to_le_bytes());
            }
        }
        CanonicalRegionSourceId::GuideBoundary(locations) => {
            add_bytes(hash, [2]);
            for location in locations {
                match location.path.source {
                    crate::StructuralPathSourceId::GuideDimension(id) => {
                        add_bytes(hash, [1]);
                        add_bytes(hash, id.0.to_le_bytes());
                    }
                    crate::StructuralPathSourceId::ParametricCurve(id) => {
                        add_bytes(hash, [2]);
                        add_bytes(hash, id.0.to_le_bytes());
                    }
                }
                add_bytes(hash, location.path.repetition_index.to_le_bytes());
                add_bytes(hash, location.path.component_ordinal.to_le_bytes());
                add_bytes(hash, (location.segment_index as u64).to_le_bytes());
                add_bytes(hash, location.parameter_bits.to_le_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_domain::PatternMechanismId;

    /// Builds one valid triangle proposal with an explicit output and source identity.
    fn triangle_proposal(path: CurvePath) -> CanonicalRegionProposal {
        CanonicalRegionProposal {
            output_layer_id: PatternOutputLayerId(4),
            source_groups: vec![CanonicalRegionSourceGroup {
                source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                    mechanism_id: PatternMechanismId(2),
                    ordinal: 1,
                }]),
                components: vec![path],
            }],
        }
    }

    /// Builds one connected nondegenerate triangle from supplied vertices.
    fn triangle(points: [(f64, f64); 3]) -> CurvePath {
        CurvePath::polyline(
            points.into_iter().map(|(x, y)| Point2::new(x, y)).collect(),
            PathClosure::Closed,
        )
        .expect("finite connected triangle")
    }

    /// Proves clockwise input becomes a deterministic positive counter-clockwise canonical ring.
    #[test]
    fn canonicalizes_clockwise_triangle_and_replays_identity() {
        let path = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(0.0, 2.0),
                Point2::new(2.0, 0.0),
            ],
            PathClosure::Closed,
        )
        .expect("finite triangle");
        let proposal = CanonicalRegionProposal {
            output_layer_id: PatternOutputLayerId(4),
            source_groups: vec![CanonicalRegionSourceGroup {
                source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                    mechanism_id: PatternMechanismId(2),
                    ordinal: 1,
                }]),
                components: vec![path],
            }],
        };
        let first = build_canonical_regions(proposal.clone()).expect("canonical triangle");
        let second = build_canonical_regions(proposal).expect("replayed triangle");
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_eq!(first.regions()[0].area, 2.0);
        assert_eq!(first.regions()[0].ring.start(), Point2::new(0.0, 0.0));
    }

    /// Proves cancellation atomically rejects a candidate before a region set can publish.
    #[test]
    fn cancellation_returns_the_shared_evaluation_diagnostic() {
        let path = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(0.0, 1.0),
            ],
            PathClosure::Closed,
        )
        .expect("finite triangle");
        let proposal = CanonicalRegionProposal {
            output_layer_id: PatternOutputLayerId(4),
            source_groups: vec![CanonicalRegionSourceGroup {
                source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                    mechanism_id: PatternMechanismId(2),
                    ordinal: 1,
                }]),
                components: vec![path],
            }],
        };
        let error =
            build_canonical_regions_cancellable(proposal, CanonicalRegionLimits::default(), || {
                true
            })
            .expect_err("cancelled");
        assert_eq!(error.path(), "evaluation.cancelled");
    }

    /// Proves signed zero disappears from canonical construction bytes and fingerprints.
    #[test]
    fn canonicalizes_signed_zero_before_geometry_identity() {
        let negative = triangle([(-0.0, -0.0), (1.0, -0.0), (-0.0, 1.0)]);
        let positive = triangle([(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]);
        let negative = build_canonical_regions(triangle_proposal(negative)).expect("valid ring");
        let positive = build_canonical_regions(triangle_proposal(positive)).expect("valid ring");
        assert_eq!(negative.fingerprint(), positive.fingerprint());
        assert_eq!(
            negative.regions()[0].ring.start().x.to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(
            negative.regions()[0].ring.start().y.to_bits(),
            0.0_f64.to_bits()
        );
    }

    /// Proves exact polynomial cubic area and extrema-derived bounds survive canonical orientation.
    #[test]
    fn cubic_ring_has_analytic_area_and_finite_bounds() {
        let curve = CubicBezierSegment::new(
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 1.0),
            Point2::new(1.0, 1.0),
            Point2::new(1.0, 0.0),
        )
        .expect("finite cubic");
        let base =
            LineSegment::new(Point2::new(1.0, 0.0), Point2::new(0.0, 0.0)).expect("finite base");
        let path = CurvePath::new(
            vec![CurveSegment::CubicBezier(curve), CurveSegment::Line(base)],
            PathClosure::Closed,
        )
        .expect("closed cubic ring");
        let set = build_canonical_regions(triangle_proposal(path)).expect("valid cubic ring");
        let region = &set.regions()[0];
        assert!((region.area - 0.6).abs() < 1e-12);
        assert_eq!(region.bounds.min, Point2::new(0.0, 0.0));
        assert_eq!(region.bounds.max, Point2::new(1.0, 0.75));
    }

    /// Proves source and component sorting assign replayable contiguous component ordinals.
    #[test]
    fn sorts_sources_and_components_before_assigning_ordinals() {
        let mut proposal = triangle_proposal(triangle([(3.0, 0.0), (4.0, 0.0), (3.0, 1.0)]));
        proposal.source_groups[0]
            .components
            .push(triangle([(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]));
        proposal.source_groups.push(CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal: 0,
            }]),
            components: vec![triangle([(6.0, 0.0), (7.0, 0.0), (6.0, 1.0)])],
        });
        let first = build_canonical_regions(proposal.clone()).expect("valid ordered groups");
        proposal.source_groups.reverse();
        proposal.source_groups[1].components.reverse();
        let replayed = build_canonical_regions(proposal).expect("valid replay");
        assert_eq!(first, replayed);
        assert_eq!(
            first
                .regions()
                .iter()
                .map(|value| value.id.component_ordinal)
                .collect::<Vec<_>>(),
            vec![0, 0, 1]
        );
        assert_eq!(first.regions()[1].ring.start(), Point2::new(0.0, 0.0));
    }

    /// Proves malformed, degenerate, and self-intersecting rings receive stable geometry diagnostics.
    #[test]
    fn rejects_malformed_and_self_crossing_rings() {
        let open = CurvePath::polyline(
            vec![Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)],
            PathClosure::Open,
        )
        .expect("open path");
        assert_eq!(
            build_canonical_regions(triangle_proposal(open))
                .unwrap_err()
                .path(),
            "region.geometry.closure"
        );
        let zero_area = triangle([(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]);
        assert_eq!(
            build_canonical_regions(triangle_proposal(zero_area))
                .unwrap_err()
                .path(),
            "region.geometry.zero_area"
        );
        let crossing = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 1.0),
                Point2::new(0.0, 1.0),
                Point2::new(1.0, 0.0),
            ],
            PathClosure::Closed,
        )
        .expect("connected bow tie");
        assert_eq!(
            build_canonical_regions(triangle_proposal(crossing))
                .unwrap_err()
                .path(),
            "region.geometry.self_crossing"
        );
    }

    /// Proves every bounded-work category rejects atomically at its configured threshold.
    #[test]
    fn enforces_each_configured_region_limit() {
        let path = triangle([(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]);
        let proposal = triangle_proposal(path.clone());
        assert_eq!(
            build_canonical_regions_cancellable(
                proposal.clone(),
                CanonicalRegionLimits::new(1, 1, 2, 100).unwrap(),
                || false
            )
            .unwrap_err()
            .path(),
            "region.limits.segments"
        );
        assert_eq!(
            build_canonical_regions_cancellable(
                proposal.clone(),
                CanonicalRegionLimits::new(1, 1, 3, 1).unwrap(),
                || false
            )
            .unwrap_err()
            .path(),
            "region.limits.inspections"
        );
        let mut two_groups = proposal.clone();
        two_groups.source_groups.push(CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(3),
                ordinal: 0,
            }]),
            components: vec![path.clone()],
        });
        assert_eq!(
            build_canonical_regions_cancellable(
                two_groups,
                CanonicalRegionLimits::new(1, 2, 6, 100).unwrap(),
                || false
            )
            .unwrap_err()
            .path(),
            "region.limits.source_groups"
        );
        let mut two_components = proposal;
        two_components.source_groups[0].components.push(path);
        assert_eq!(
            build_canonical_regions_cancellable(
                two_components,
                CanonicalRegionLimits::new(1, 1, 6, 100).unwrap(),
                || false
            )
            .unwrap_err()
            .path(),
            "region.limits.regions"
        );
    }
}
