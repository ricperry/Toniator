//! Neutral, deterministic site placement for geometry generators.
//!
//! This module deliberately has no knowledge of documents, artwork channels,
//! renderers, or patterns. Callers provide a bounded domain and, when desired,
//! a normalized sampled field.

use crate::CancellationToken;
use anyhow::{Result, ensure};

/// Public safety limits shared by all neutral distribution requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DistributionLimits {
    pub max_sites: usize,
    pub max_candidates: usize,
}

impl Default for DistributionLimits {
    fn default() -> Self {
        Self {
            max_sites: 8_192,
            max_candidates: 65_536,
        }
    }
}

/// A finite rectangular generation domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DomainBounds {
    pub width: u32,
    pub height: u32,
}

impl DomainBounds {
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.width > 0 && self.height > 0,
            "site distribution requires a non-empty domain"
        );
        Ok(())
    }
}

/// A point whose vector position is its stable order in a distribution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrderedPoint {
    pub x: f64,
    pub y: f64,
}

/// Identifies one independent consumer of an otherwise shared arrangement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DistributionIdentity(pub u64);

/// Whether consumers coordinate their candidate arrangement or derive one
/// from their own identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArrangementPolicy {
    Shared,
    Independent,
}

/// How candidate locations are selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DistributionMode {
    Uniform,
    SourceWeighted,
}

/// Which end of a normalized source field attracts sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DistributionPolarity {
    HigherValuesMoreDense,
    LowerValuesMoreDense,
}

/// A normalized scalar field used only by source-weighted placement.
#[derive(Debug, Clone, PartialEq)]
pub struct DistributionField {
    width: u32,
    height: u32,
    values: Vec<f64>,
}

impl DistributionField {
    pub fn new(width: u32, height: u32, values: Vec<f64>) -> Result<Self> {
        ensure!(
            width > 0 && height > 0,
            "distribution field must be non-empty"
        );
        ensure!(
            values.len() == (width as usize).saturating_mul(height as usize),
            "distribution field dimensions do not match its values"
        );
        ensure!(
            values
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value)),
            "distribution field values must be finite normalized values"
        );
        Ok(Self {
            width,
            height,
            values,
        })
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn values(&self) -> &[f64] {
        &self.values
    }

    fn sample_at(&self, point: OrderedPoint, domain: DomainBounds) -> f64 {
        let x = ((point.x / f64::from(domain.width)) * f64::from(self.width))
            .floor()
            .clamp(0.0, f64::from(self.width - 1)) as usize;
        let y = ((point.y / f64::from(domain.height)) * f64::from(self.height))
            .floor()
            .clamp(0.0, f64::from(self.height - 1)) as usize;
        self.values[y * self.width as usize + x]
    }
}

/// Stable input metadata suitable for persistence or cache keys in a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DistributionRequestMetadata {
    pub seed: u64,
    pub identity: DistributionIdentity,
    pub arrangement: ArrangementPolicy,
    pub mode: DistributionMode,
    pub polarity: DistributionPolarity,
    /// Exponent applied to normalized source values for weighted placement.
    pub strength_milli: u32,
}

impl DistributionRequestMetadata {
    pub fn strength(self) -> f64 {
        f64::from(self.strength_milli) / 1_000.0
    }
}

/// One complete finite site-distribution request.
#[derive(Debug, Clone, Copy)]
pub struct DistributionRequest<'a> {
    pub domain: DomainBounds,
    pub count: usize,
    pub metadata: DistributionRequestMetadata,
    pub field: Option<&'a DistributionField>,
    pub limits: DistributionLimits,
}

/// A reproducible fingerprint of the effective request and ordered points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DistributionFingerprint(pub u64);

/// Exact ordered placement output with cacheable provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct SiteDistribution {
    pub domain: DomainBounds,
    pub metadata: DistributionRequestMetadata,
    pub points: Vec<OrderedPoint>,
    pub fingerprint: DistributionFingerprint,
}

pub fn generate_site_distribution(request: DistributionRequest<'_>) -> Result<SiteDistribution> {
    generate_site_distribution_cancellable(request, &CancellationToken::new())
}

/// Generates an exact number of distinct points from a finite candidate grid.
///
/// Uniform mode never reads `field`. Weighted mode uses exponential-race
/// selection without replacement, so it has no rejection loop or attempt cap.
pub fn generate_site_distribution_cancellable(
    request: DistributionRequest<'_>,
    token: &CancellationToken,
) -> Result<SiteDistribution> {
    token.checkpoint()?;
    request.domain.validate()?;
    ensure!(
        request.count > 0,
        "site distribution requires at least one site"
    );
    ensure!(
        request.count <= request.limits.max_sites,
        "site distribution exceeds the {} site limit",
        request.limits.max_sites
    );
    if matches!(request.metadata.mode, DistributionMode::SourceWeighted) {
        ensure!(
            request.field.is_some(),
            "source-weighted site distribution requires a field"
        );
        ensure!(
            request.metadata.strength() > 0.0,
            "source-weighted placement strength must be positive"
        );
    }

    let candidate_count = candidate_count(request.count, request.limits)?;
    let arrangement_seed = arrangement_seed(request.metadata);
    let mut arrangement_rng = SplitMix64::new(arrangement_seed);
    let candidates =
        stratified_candidates(request.domain, candidate_count, &mut arrangement_rng, token)?;
    let points = match request.metadata.mode {
        DistributionMode::Uniform => candidates.into_iter().take(request.count).collect(),
        DistributionMode::SourceWeighted => select_weighted_candidates(
            candidates,
            request.count,
            request.domain,
            request.field.expect("validated weighted field"),
            request.metadata,
            token,
        )?,
    };
    token.checkpoint()?;
    let fingerprint = fingerprint(request.domain, request.metadata, &points);
    Ok(SiteDistribution {
        domain: request.domain,
        metadata: request.metadata,
        points,
        fingerprint,
    })
}

fn candidate_count(count: usize, limits: DistributionLimits) -> Result<usize> {
    let desired = count.saturating_mul(8).max(count);
    ensure!(
        desired <= limits.max_candidates,
        "site distribution needs {desired} candidates but the centralized limit is {}",
        limits.max_candidates
    );
    Ok(desired)
}

fn arrangement_seed(metadata: DistributionRequestMetadata) -> u64 {
    match metadata.arrangement {
        ArrangementPolicy::Shared => metadata.seed,
        ArrangementPolicy::Independent => mix64(metadata.seed ^ metadata.identity.0),
    }
}

fn stratified_candidates(
    domain: DomainBounds,
    count: usize,
    rng: &mut SplitMix64,
    token: &CancellationToken,
) -> Result<Vec<OrderedPoint>> {
    let aspect = f64::from(domain.width) / f64::from(domain.height);
    let columns = ((count as f64 * aspect).sqrt().ceil() as usize).max(1);
    let rows = count.div_ceil(columns);
    let mut cells: Vec<usize> = (0..count).collect();
    for index in (1..cells.len()).rev() {
        if index % 256 == 0 {
            token.checkpoint()?;
        }
        let swap = (rng.next_u64() as usize) % (index + 1);
        cells.swap(index, swap);
    }
    let cell_width = f64::from(domain.width) / columns as f64;
    let cell_height = f64::from(domain.height) / rows as f64;
    let mut points = Vec::with_capacity(count);
    for (order, cell) in cells.into_iter().enumerate() {
        if order % 256 == 0 {
            token.checkpoint()?;
        }
        let column = cell % columns;
        let row = cell / columns;
        points.push(OrderedPoint {
            x: (column as f64 + rng.unit_f64()) * cell_width,
            y: (row as f64 + rng.unit_f64()) * cell_height,
        });
    }
    Ok(points)
}

fn select_weighted_candidates(
    candidates: Vec<OrderedPoint>,
    count: usize,
    domain: DomainBounds,
    field: &DistributionField,
    metadata: DistributionRequestMetadata,
    token: &CancellationToken,
) -> Result<Vec<OrderedPoint>> {
    let mut rng = SplitMix64::new(mix64(arrangement_seed(metadata) ^ 0x4f1b_bcdd_5e47_9a01));
    let strength = metadata.strength();
    let mut ranked = Vec::with_capacity(candidates.len());
    for (index, point) in candidates.into_iter().enumerate() {
        if index % 256 == 0 {
            token.checkpoint()?;
        }
        let source = field.sample_at(point, domain);
        let directed = match metadata.polarity {
            DistributionPolarity::HigherValuesMoreDense => source,
            DistributionPolarity::LowerValuesMoreDense => 1.0 - source,
        };
        // A tiny explicit floor keeps every finite candidate selectable on a
        // blank field without changing the exact-count/no-rejection contract.
        let weight = directed.powf(strength).max(1.0e-9);
        let exponential = -(1.0 - rng.unit_f64()).ln() / weight;
        ranked.push((exponential, index, point));
    }
    ranked.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
    });
    Ok(ranked
        .into_iter()
        .take(count)
        .map(|(_, _, point)| point)
        .collect())
}

fn fingerprint(
    domain: DomainBounds,
    metadata: DistributionRequestMetadata,
    points: &[OrderedPoint],
) -> DistributionFingerprint {
    let mut value = 0xcbf2_9ce4_8422_2325u64;
    for part in [
        u64::from(domain.width),
        u64::from(domain.height),
        metadata.seed,
        metadata.identity.0,
        metadata.arrangement as u64,
        metadata.mode as u64,
        metadata.polarity as u64,
        u64::from(metadata.strength_milli),
        points.len() as u64,
    ] {
        value = hash_word(value, part);
    }
    for point in points {
        value = hash_word(value, point.x.to_bits());
        value = hash_word(value, point.y.to_bits());
    }
    DistributionFingerprint(value)
}

fn hash_word(mut hash: u64, word: u64) -> u64 {
    for byte in word.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix64(self.0)
    }

    fn unit_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(seed: u64, mode: DistributionMode) -> DistributionRequestMetadata {
        DistributionRequestMetadata {
            seed,
            identity: DistributionIdentity(17),
            arrangement: ArrangementPolicy::Shared,
            mode,
            polarity: DistributionPolarity::HigherValuesMoreDense,
            strength_milli: 1_000,
        }
    }

    fn request<'a>(
        mode: DistributionMode,
        field: Option<&'a DistributionField>,
    ) -> DistributionRequest<'a> {
        DistributionRequest {
            domain: DomainBounds {
                width: 100,
                height: 60,
            },
            count: 80,
            metadata: metadata(42, mode),
            field,
            limits: DistributionLimits::default(),
        }
    }

    #[test]
    fn stable_ordered_uniform_output_changes_with_seed_and_is_spatially_spread() {
        let first = generate_site_distribution(request(DistributionMode::Uniform, None)).unwrap();
        let second = generate_site_distribution(request(DistributionMode::Uniform, None)).unwrap();
        assert_eq!(first, second);
        let mut changed = request(DistributionMode::Uniform, None);
        changed.metadata.seed += 1;
        let different = generate_site_distribution(changed).unwrap();
        assert_ne!(first.points, different.points);
        assert_ne!(first.fingerprint, different.fingerprint);
        assert!(first.points.iter().all(|point| {
            point.x >= 0.0 && point.x < 100.0 && point.y >= 0.0 && point.y < 60.0
        }));
        let occupied: std::collections::BTreeSet<_> = first
            .points
            .iter()
            .map(|point| ((point.x / 20.0) as u32, (point.y / 20.0) as u32))
            .collect();
        assert!(occupied.len() >= 14);
    }

    #[test]
    fn uniform_ignores_source_values_and_returns_exact_distinct_count() {
        let dark = DistributionField::new(2, 1, vec![1.0, 0.0]).unwrap();
        let light = DistributionField::new(2, 1, vec![0.0, 1.0]).unwrap();
        let first =
            generate_site_distribution(request(DistributionMode::Uniform, Some(&dark))).unwrap();
        let second =
            generate_site_distribution(request(DistributionMode::Uniform, Some(&light))).unwrap();
        assert_eq!(first.points, second.points);
        assert_eq!(first.points.len(), 80);
        let distinct: std::collections::BTreeSet<_> = first
            .points
            .iter()
            .map(|point| (point.x.to_bits(), point.y.to_bits()))
            .collect();
        assert_eq!(distinct.len(), first.points.len());
    }

    #[test]
    fn source_weighting_follows_polarity_and_strength() {
        let field = DistributionField::new(2, 1, vec![0.8, 0.2]).unwrap();
        let higher =
            generate_site_distribution(request(DistributionMode::SourceWeighted, Some(&field)))
                .unwrap();
        let higher_left = higher.points.iter().filter(|point| point.x < 50.0).count();
        assert!(higher_left > 55);
        let mut lower_request = request(DistributionMode::SourceWeighted, Some(&field));
        lower_request.metadata.polarity = DistributionPolarity::LowerValuesMoreDense;
        let lower = generate_site_distribution(lower_request).unwrap();
        assert!(lower.points.iter().filter(|point| point.x >= 50.0).count() > 55);
        let mut weak_request = request(DistributionMode::SourceWeighted, Some(&field));
        weak_request.metadata.strength_milli = 100;
        let weak = generate_site_distribution(weak_request).unwrap();
        assert!(higher_left > weak.points.iter().filter(|point| point.x < 50.0).count());
    }

    #[test]
    fn shared_and_independent_arrangements_are_explicit() {
        let shared = request(DistributionMode::Uniform, None);
        let mut other_shared = shared;
        other_shared.metadata.identity = DistributionIdentity(99);
        assert_eq!(
            generate_site_distribution(shared).unwrap().points,
            generate_site_distribution(other_shared).unwrap().points
        );
        let mut independent = other_shared;
        independent.metadata.arrangement = ArrangementPolicy::Independent;
        assert_ne!(
            generate_site_distribution(shared).unwrap().points,
            generate_site_distribution(independent).unwrap().points
        );
    }

    #[test]
    fn cancellation_and_central_limits_are_enforced() {
        let token = CancellationToken::new();
        assert!(token.cancel());
        assert!(
            generate_site_distribution_cancellable(
                request(DistributionMode::Uniform, None),
                &token
            )
            .is_err()
        );
        let mut over_limit = request(DistributionMode::Uniform, None);
        over_limit.count = DistributionLimits::default().max_sites + 1;
        assert!(generate_site_distribution(over_limit).is_err());
        let mut candidate_limited = request(DistributionMode::Uniform, None);
        candidate_limited.limits.max_candidates = 100;
        assert!(generate_site_distribution(candidate_limited).is_err());
    }
}
