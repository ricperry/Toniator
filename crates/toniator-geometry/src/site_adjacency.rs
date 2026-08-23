//! Deterministic derived topology for already evaluated family sites.

use std::{cmp::Ordering, collections::BinaryHeap};

use super::{
    FamilySite, FamilySiteId, FamilySiteProvenance, FamilySiteSet, GuideInstanceId,
    NominalCellBasis, Point2, SiteScope, StructuralPathInstanceId,
    StructuralPathLocationProvenance, StructuralPathSourceId,
};

/// Stable identity for the mathematical adjacency contract implemented here.
pub const SITE_ADJACENCY_CONTRACT_ID: &str = "toniator-stage-20l-mutual-nearest-v1";

/// Caller-supplied rule for deriving topology from one immutable family product.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SiteAdjacencyPolicy {
    /// Retains an edge only when both endpoints select each other among their nearest neighbours.
    MutualNearest {
        maximum_degree: usize,
        maximum_distance: f64,
    },
}

impl SiteAdjacencyPolicy {
    /// Returns the bounded neighbour count required by this policy.
    pub const fn maximum_degree(self) -> usize {
        match self {
            Self::MutualNearest { maximum_degree, .. } => maximum_degree,
        }
    }

    /// Returns the finite absolute distance required by this policy.
    pub const fn maximum_distance(self) -> f64 {
        match self {
            Self::MutualNearest {
                maximum_distance, ..
            } => maximum_distance,
        }
    }

    /// Validates the fixed Stage 20L policy bounds before any topology work begins.
    ///
    /// # Errors
    ///
    /// Returns a stable adjacency diagnostic when degree is outside `1..=32` or distance is not
    /// finite and strictly positive.
    pub fn validate(self) -> Result<(), SiteAdjacencyError> {
        if !(1..=32).contains(&self.maximum_degree()) {
            return Err(SiteAdjacencyError::new(
                "adjacency.policy.maximum_degree",
                "maximum degree must be in 1..=32",
            ));
        }
        if !self.maximum_distance().is_finite() || self.maximum_distance() <= 0.0 {
            return Err(SiteAdjacencyError::new(
                "adjacency.policy.maximum_distance",
                "maximum distance must be finite and positive",
            ));
        }
        Ok(())
    }
}

/// Configurable resource bounds for one derived adjacency result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SiteAdjacencyLimits {
    pub maximum_nodes: usize,
    pub maximum_neighbor_memberships: usize,
    pub maximum_edges: usize,
    pub maximum_distance_checks: usize,
}

impl SiteAdjacencyLimits {
    /// Builds a complete nonzero resource policy.
    ///
    /// # Errors
    ///
    /// Returns a stable adjacency diagnostic when any enabled work category is disabled.
    pub fn new(
        maximum_nodes: usize,
        maximum_neighbor_memberships: usize,
        maximum_edges: usize,
        maximum_distance_checks: usize,
    ) -> Result<Self, SiteAdjacencyError> {
        let limits = Self {
            maximum_nodes,
            maximum_neighbor_memberships,
            maximum_edges,
            maximum_distance_checks,
        };
        if [
            limits.maximum_nodes,
            limits.maximum_neighbor_memberships,
            limits.maximum_edges,
            limits.maximum_distance_checks,
        ]
        .contains(&0)
        {
            return Err(SiteAdjacencyError::new(
                "adjacency.limits",
                "all adjacency limits must be nonzero",
            ));
        }
        Ok(limits)
    }
}

impl Default for SiteAdjacencyLimits {
    /// Supplies the fixed Stage 20L bounded-work defaults.
    fn default() -> Self {
        Self {
            maximum_nodes: 262_144,
            maximum_neighbor_memberships: 2_097_152,
            maximum_edges: 1_048_576,
            maximum_distance_checks: 33_554_432,
        }
    }
}

/// One evaluator-ordered graph node retaining all site facts without reinterpretation.
#[derive(Clone, Debug, PartialEq)]
pub struct SiteAdjacencyNode {
    pub id: FamilySiteId,
    pub position: Point2,
    pub scope: SiteScope,
    pub nominal_cell_basis: NominalCellBasis,
    pub provenance: FamilySiteProvenance,
}

impl SiteAdjacencyNode {
    /// Copies immutable family-site facts into one topology-only node with fallible provenance allocation.
    ///
    /// # Errors
    ///
    /// Returns `adjacency.allocation` when truthful contributor provenance cannot be retained.
    fn try_from_site(site: &FamilySite) -> Result<Self, SiteAdjacencyError> {
        Ok(Self {
            id: site.id,
            position: site.position,
            scope: site.scope,
            nominal_cell_basis: site.nominal_cell_basis,
            provenance: clone_provenance(&site.provenance)?,
        })
    }
}

/// One canonical undirected edge ordered by stable endpoint ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SiteAdjacencyEdge {
    pub first: FamilySiteId,
    pub second: FamilySiteId,
}

/// One connected component, including components containing a single isolated node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteAdjacencyComponent {
    pub members: Vec<FamilySiteId>,
}

/// Complete immutable derived topology; resource policy is intentionally absent from its identity.
#[derive(Clone, Debug, PartialEq)]
pub struct SiteAdjacencyGraph {
    family_fingerprint: String,
    policy: SiteAdjacencyPolicy,
    fingerprint: String,
    nodes: Vec<SiteAdjacencyNode>,
    edges: Vec<SiteAdjacencyEdge>,
    components: Vec<SiteAdjacencyComponent>,
}

impl SiteAdjacencyGraph {
    /// Returns the source family identity from which this graph was derived.
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    /// Returns the caller-supplied topology rule without turning it into document intent.
    pub const fn policy(&self) -> SiteAdjacencyPolicy {
        self.policy
    }

    /// Returns the stable Stage 20L graph identity, excluding operational resource limits.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Returns nodes in the source evaluator's original order.
    pub fn nodes(&self) -> &[SiteAdjacencyNode] {
        &self.nodes
    }

    /// Returns deduplicated canonical edges sorted by endpoint IDs.
    pub fn edges(&self) -> &[SiteAdjacencyEdge] {
        &self.edges
    }

    /// Returns ID-sorted components, ordered by their smallest member ID.
    pub fn components(&self) -> &[SiteAdjacencyComponent] {
        &self.components
    }
}

/// Stable derived-topology failure that never exposes a partial graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteAdjacencyError {
    path: &'static str,
    message: &'static str,
}

impl SiteAdjacencyError {
    /// Creates one stable bounded-adjacency diagnostic.
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }

    /// Returns the stable source-owned diagnostic path.
    pub const fn path(&self) -> &'static str {
        self.path
    }

    /// Returns the stable human-readable diagnostic message.
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl std::fmt::Display for SiteAdjacencyError {
    /// Formats the stable topology failure without exposing partial work.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for SiteAdjacencyError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Cell {
    x: i64,
    y: i64,
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    id: FamilySiteId,
    distance: f64,
}

impl PartialEq for Candidate {
    /// Compares candidates by exact retained ID and finite distance bits.
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.distance.to_bits() == other.distance.to_bits()
    }
}

impl Eq for Candidate {}

impl Ord for Candidate {
    /// Orders candidates from nearest/lowest ID to farthest/highest ID for the bounded max-heap.
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance
            .total_cmp(&other.distance)
            .then_with(|| self.id.cmp(&other.id))
    }
}

impl PartialOrd for Candidate {
    /// Reports the total candidate order required by the bounded nearest-neighbour heap.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Builds an atomic mutual-nearest graph using a deterministic uniform spatial index.
///
/// The index cell width equals the policy distance and visits each node's fixed `3x3` cell
/// neighbourhood. Nodes retain source order; all returned topology is canonical and independent
/// of operational limits.
///
/// # Errors
///
/// Returns stable policy, limit, coordinate, distance, allocation, or cancellation diagnostics
/// before any graph is returned. Cancellation is always `evaluation.cancelled`.
pub fn build_site_adjacency_cancellable(
    sites: &FamilySiteSet,
    policy: SiteAdjacencyPolicy,
    limits: SiteAdjacencyLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<SiteAdjacencyGraph, SiteAdjacencyError> {
    policy.validate()?;
    SiteAdjacencyLimits::new(
        limits.maximum_nodes,
        limits.maximum_neighbor_memberships,
        limits.maximum_edges,
        limits.maximum_distance_checks,
    )?;
    cancelled(is_cancelled)?;
    if sites.len() > limits.maximum_nodes {
        return Err(SiteAdjacencyError::new(
            "adjacency.limits.nodes",
            "site count exceeds configured adjacency node limit",
        ));
    }

    let mut nodes = Vec::new();
    reserve(&mut nodes, sites.len())?;
    let mut indexed = Vec::new();
    reserve(&mut indexed, sites.len())?;
    for (index, site) in sites.iter().enumerate() {
        cancelled(is_cancelled)?;
        nodes.push(SiteAdjacencyNode::try_from_site(site)?);
        indexed.push((cell_for(site.position, policy.maximum_distance())?, index));
    }
    indexed.sort_unstable();

    let degree = policy.maximum_degree();
    let mut selections = Vec::<Vec<FamilySiteId>>::new();
    reserve(&mut selections, nodes.len())?;
    let mut retained_memberships = 0_usize;
    let mut distance_checks = 0_usize;
    for index in 0..nodes.len() {
        cancelled(is_cancelled)?;
        let origin = cell_for(nodes[index].position, policy.maximum_distance())?;
        let mut nearest = BinaryHeap::<Candidate>::new();
        nearest.try_reserve(degree).map_err(|_| {
            SiteAdjacencyError::new("adjacency.allocation", "adjacency allocation failed")
        })?;
        for dy in -1_i64..=1 {
            for dx in -1_i64..=1 {
                let cell = Cell {
                    x: origin.x.checked_add(dx).ok_or(SiteAdjacencyError::new(
                        "adjacency.cell_coordinate",
                        "spatial cell coordinate is not representable",
                    ))?,
                    y: origin.y.checked_add(dy).ok_or(SiteAdjacencyError::new(
                        "adjacency.cell_coordinate",
                        "spatial cell coordinate is not representable",
                    ))?,
                };
                let start = indexed.partition_point(|(candidate, _)| *candidate < cell);
                let end = indexed.partition_point(|(candidate, _)| *candidate <= cell);
                for &(_, other) in &indexed[start..end] {
                    cancelled(is_cancelled)?;
                    if index == other {
                        continue;
                    }
                    distance_checks =
                        distance_checks
                            .checked_add(1)
                            .ok_or(SiteAdjacencyError::new(
                                "adjacency.limits.distance_checks",
                                "distance-check count exceeds configured adjacency limit",
                            ))?;
                    if distance_checks > limits.maximum_distance_checks {
                        return Err(SiteAdjacencyError::new(
                            "adjacency.limits.distance_checks",
                            "distance-check count exceeds configured adjacency limit",
                        ));
                    }
                    let distance = distance_between(nodes[index].position, nodes[other].position)?;
                    if distance == 0.0 || distance > policy.maximum_distance() {
                        continue;
                    }
                    let candidate = Candidate {
                        id: nodes[other].id,
                        distance,
                    };
                    if nearest.len() < degree {
                        nearest.push(candidate);
                    } else if nearest.peek().is_some_and(|farthest| candidate < *farthest) {
                        nearest.pop();
                        nearest.push(candidate);
                    }
                }
            }
        }
        let mut chosen = nearest.into_sorted_vec();
        retained_memberships =
            retained_memberships
                .checked_add(chosen.len())
                .ok_or(SiteAdjacencyError::new(
                    "adjacency.limits.neighbor_memberships",
                    "retained neighbour memberships exceed configured adjacency limit",
                ))?;
        if retained_memberships > limits.maximum_neighbor_memberships {
            return Err(SiteAdjacencyError::new(
                "adjacency.limits.neighbor_memberships",
                "retained neighbour memberships exceed configured adjacency limit",
            ));
        }
        let mut ids = Vec::new();
        reserve(&mut ids, chosen.len())?;
        ids.extend(chosen.drain(..).map(|candidate| candidate.id));
        selections.push(ids);
    }

    let mut edges = Vec::new();
    for index in 0..nodes.len() {
        cancelled(is_cancelled)?;
        for &other_id in &selections[index] {
            let other = other_id.ordinal;
            if other >= nodes.len() || !selections[other].contains(&nodes[index].id) {
                continue;
            }
            let edge = if nodes[index].id < other_id {
                SiteAdjacencyEdge {
                    first: nodes[index].id,
                    second: other_id,
                }
            } else {
                SiteAdjacencyEdge {
                    first: other_id,
                    second: nodes[index].id,
                }
            };
            if edge.first != nodes[index].id {
                continue;
            }
            let next_edge_count = edges.len().checked_add(1).ok_or(SiteAdjacencyError::new(
                "adjacency.limits.edges",
                "edge count exceeds configured adjacency limit",
            ))?;
            if next_edge_count > limits.maximum_edges {
                return Err(SiteAdjacencyError::new(
                    "adjacency.limits.edges",
                    "edge count exceeds configured adjacency limit",
                ));
            }
            if edges.len() == edges.capacity() {
                reserve(&mut edges, 1)?;
            }
            edges.push(edge);
        }
    }
    edges.sort_unstable();
    edges.dedup();

    let mut neighbors = Vec::<Vec<usize>>::new();
    reserve(&mut neighbors, nodes.len())?;
    neighbors.resize_with(nodes.len(), Vec::new);
    for edge in &edges {
        cancelled(is_cancelled)?;
        let first = edge.first.ordinal;
        let second = edge.second.ordinal;
        if neighbors[first].len() == neighbors[first].capacity() {
            reserve(&mut neighbors[first], 1)?;
        }
        neighbors[first].push(second);
        if neighbors[second].len() == neighbors[second].capacity() {
            reserve(&mut neighbors[second], 1)?;
        }
        neighbors[second].push(first);
    }
    let mut visited = Vec::new();
    reserve(&mut visited, nodes.len())?;
    visited.resize(nodes.len(), false);
    let mut components = Vec::new();
    reserve(&mut components, nodes.len())?;
    for start in 0..nodes.len() {
        cancelled(is_cancelled)?;
        if visited[start] {
            continue;
        }
        let mut pending = Vec::new();
        reserve(&mut pending, 1)?;
        pending.push(start);
        visited[start] = true;
        let mut members = Vec::new();
        reserve(&mut members, 1)?;
        while let Some(current) = pending.pop() {
            cancelled(is_cancelled)?;
            if members.len() == members.capacity() {
                reserve(&mut members, 1)?;
            }
            members.push(nodes[current].id);
            for &next in &neighbors[current] {
                if !visited[next] {
                    visited[next] = true;
                    if pending.len() == pending.capacity() {
                        reserve(&mut pending, 1)?;
                    }
                    pending.push(next);
                }
            }
        }
        members.sort_unstable();
        components.push(SiteAdjacencyComponent { members });
    }
    components.sort_unstable_by(|left, right| left.members[0].cmp(&right.members[0]));
    let fingerprint = graph_fingerprint(sites.family_fingerprint(), policy, &nodes, &edges)?;
    Ok(SiteAdjacencyGraph {
        family_fingerprint: clone_string(sites.family_fingerprint())?,
        policy,
        fingerprint,
        nodes,
        edges,
        components,
    })
}

/// Returns cancellation before a caller can observe a partial topology result.
fn cancelled(is_cancelled: &dyn Fn() -> bool) -> Result<(), SiteAdjacencyError> {
    is_cancelled().then_some(()).map_or(Ok(()), |_| {
        Err(SiteAdjacencyError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ))
    })
}

/// Reserves an exact additional vector capacity and maps allocation failure to the stable contract.
fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<(), SiteAdjacencyError> {
    values
        .try_reserve(additional)
        .map_err(|_| SiteAdjacencyError::new("adjacency.allocation", "adjacency allocation failed"))
}

/// Maps one finite point into the fixed-width spatial index coordinate system.
fn cell_for(point: Point2, width: f64) -> Result<Cell, SiteAdjacencyError> {
    let x = (point.x / width).floor();
    let y = (point.y / width).floor();
    const I64_UPPER_EXCLUSIVE: f64 = 9_223_372_036_854_775_808.0;
    if !x.is_finite()
        || !y.is_finite()
        || x < i64::MIN as f64
        || x >= I64_UPPER_EXCLUSIVE
        || y < i64::MIN as f64
        || y >= I64_UPPER_EXCLUSIVE
    {
        return Err(SiteAdjacencyError::new(
            "adjacency.cell_coordinate",
            "spatial cell coordinate is not representable",
        ));
    }
    Ok(Cell {
        x: x as i64,
        y: y as i64,
    })
}

/// Calculates one Euclidean pair distance while rejecting nonfinite intermediate geometry.
fn distance_between(left: Point2, right: Point2) -> Result<f64, SiteAdjacencyError> {
    let distance = (left.x - right.x).hypot(left.y - right.y);
    distance
        .is_finite()
        .then_some(distance)
        .ok_or(SiteAdjacencyError::new(
            "adjacency.distance",
            "computed site distance must be finite",
        ))
}

/// Hashes the complete semantic graph payload without operational resource limits.
fn graph_fingerprint(
    family_fingerprint: &str,
    policy: SiteAdjacencyPolicy,
    nodes: &[SiteAdjacencyNode],
    edges: &[SiteAdjacencyEdge],
) -> Result<String, SiteAdjacencyError> {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_bytes(&mut hash, SITE_ADJACENCY_CONTRACT_ID.as_bytes());
    hash_length_delimited(&mut hash, family_fingerprint.as_bytes());
    hash_bytes(&mut hash, &policy.maximum_degree().to_le_bytes());
    hash_bytes(
        &mut hash,
        &policy.maximum_distance().to_bits().to_le_bytes(),
    );
    hash_length(&mut hash, nodes.len());
    for node in nodes {
        hash_site_id(&mut hash, node.id);
        hash_bytes(&mut hash, &node.position.x.to_bits().to_le_bytes());
        hash_bytes(&mut hash, &node.position.y.to_bits().to_le_bytes());
        hash_bytes(&mut hash, &[matches!(node.scope, SiteScope::Guard) as u8]);
        hash_bytes(
            &mut hash,
            &node.nominal_cell_basis.axis_a.x.to_bits().to_le_bytes(),
        );
        hash_bytes(
            &mut hash,
            &node.nominal_cell_basis.axis_a.y.to_bits().to_le_bytes(),
        );
        hash_bytes(
            &mut hash,
            &node.nominal_cell_basis.axis_b.x.to_bits().to_le_bytes(),
        );
        hash_bytes(
            &mut hash,
            &node.nominal_cell_basis.axis_b.y.to_bits().to_le_bytes(),
        );
        hash_provenance(&mut hash, &node.provenance);
    }
    hash_length(&mut hash, edges.len());
    for edge in edges {
        hash_site_id(&mut hash, edge.first);
        hash_site_id(&mut hash, edge.second);
    }
    let mut fingerprint = String::new();
    fingerprint
        .try_reserve(SITE_ADJACENCY_CONTRACT_ID.len() + 17)
        .map_err(|_| {
            SiteAdjacencyError::new("adjacency.allocation", "adjacency allocation failed")
        })?;
    use std::fmt::Write as _;
    write!(fingerprint, "{SITE_ADJACENCY_CONTRACT_ID}:{hash:016x}").map_err(|_| {
        SiteAdjacencyError::new("adjacency.allocation", "adjacency allocation failed")
    })?;
    Ok(fingerprint)
}

/// Copies one truthful provenance payload while checking all caller-sized contributor allocation.
fn clone_provenance(
    provenance: &FamilySiteProvenance,
) -> Result<FamilySiteProvenance, SiteAdjacencyError> {
    let allocation =
        || SiteAdjacencyError::new("adjacency.allocation", "adjacency allocation failed");
    Ok(match provenance {
        FamilySiteProvenance::GuideIntersection { contributors } => {
            let mut copied = Vec::new();
            copied
                .try_reserve(contributors.len())
                .map_err(|_| allocation())?;
            copied.extend_from_slice(contributors);
            FamilySiteProvenance::GuideIntersection {
                contributors: copied,
            }
        }
        FamilySiteProvenance::AlongGuide {
            guide_id,
            guide_order,
            sequence,
            absolute_arc_position_bits,
            local_arc_position_bits,
        } => FamilySiteProvenance::AlongGuide {
            guide_id: *guide_id,
            guide_order: *guide_order,
            sequence: *sequence,
            absolute_arc_position_bits: *absolute_arc_position_bits,
            local_arc_position_bits: *local_arc_position_bits,
        },
        FamilySiteProvenance::Random {
            candidate_ordinal,
            accepted_ordinal,
            exclusion_neighbor_ordinal,
        } => FamilySiteProvenance::Random {
            candidate_ordinal: *candidate_ordinal,
            accepted_ordinal: *accepted_ordinal,
            exclusion_neighbor_ordinal: *exclusion_neighbor_ordinal,
        },
        FamilySiteProvenance::CurveGuideIntersection { contributors } => {
            let mut copied = Vec::new();
            copied
                .try_reserve(contributors.len())
                .map_err(|_| allocation())?;
            copied.extend_from_slice(contributors);
            FamilySiteProvenance::CurveGuideIntersection {
                contributors: copied,
            }
        }
        FamilySiteProvenance::CurveAlongGuide {
            location,
            guide_order,
            sequence,
            absolute_arc_position_bits,
            local_arc_position_bits,
        } => FamilySiteProvenance::CurveAlongGuide {
            location: *location,
            guide_order: *guide_order,
            sequence: *sequence,
            absolute_arc_position_bits: *absolute_arc_position_bits,
            local_arc_position_bits: *local_arc_position_bits,
        },
        FamilySiteProvenance::AlongParametricCurve {
            location,
            path_order,
            sequence,
            absolute_arc_position_bits,
            local_arc_position_bits,
        } => FamilySiteProvenance::AlongParametricCurve {
            location: *location,
            path_order: *path_order,
            sequence: *sequence,
            absolute_arc_position_bits: *absolute_arc_position_bits,
            local_arc_position_bits: *local_arc_position_bits,
        },
    })
}

/// Copies source family identity through a checked adjacency-owned allocation boundary.
fn clone_string(value: &str) -> Result<String, SiteAdjacencyError> {
    let mut copied = String::new();
    copied.try_reserve(value.len()).map_err(|_| {
        SiteAdjacencyError::new("adjacency.allocation", "adjacency allocation failed")
    })?;
    copied.push_str(value);
    Ok(copied)
}

/// Incorporates one stable family-site identity into the graph hash.
fn hash_site_id(hash: &mut u64, id: FamilySiteId) {
    hash_bytes(hash, &id.mechanism_id.0.to_le_bytes());
    hash_bytes(hash, &id.ordinal.to_le_bytes());
}

/// Hashes each truthful provenance variant with typed tags and length-delimited contributor lists.
fn hash_provenance(hash: &mut u64, provenance: &FamilySiteProvenance) {
    match provenance {
        FamilySiteProvenance::GuideIntersection { contributors } => {
            hash_bytes(hash, &[0]);
            hash_length(hash, contributors.len());
            for contributor in contributors {
                hash_guide_instance(hash, *contributor);
            }
        }
        FamilySiteProvenance::AlongGuide {
            guide_id,
            guide_order,
            sequence,
            absolute_arc_position_bits,
            local_arc_position_bits,
        } => {
            hash_bytes(hash, &[1]);
            hash_guide_instance(hash, *guide_id);
            hash_length(hash, *guide_order);
            hash_bytes(hash, &sequence.to_le_bytes());
            hash_bytes(hash, &absolute_arc_position_bits.to_le_bytes());
            hash_bytes(hash, &local_arc_position_bits.to_le_bytes());
        }
        FamilySiteProvenance::Random {
            candidate_ordinal,
            accepted_ordinal,
            exclusion_neighbor_ordinal,
        } => {
            hash_bytes(hash, &[2]);
            hash_length(hash, *candidate_ordinal);
            hash_length(hash, *accepted_ordinal);
            match exclusion_neighbor_ordinal {
                Some(value) => {
                    hash_bytes(hash, &[1]);
                    hash_length(hash, *value);
                }
                None => hash_bytes(hash, &[0]),
            }
        }
        FamilySiteProvenance::CurveGuideIntersection { contributors } => {
            hash_bytes(hash, &[3]);
            hash_length(hash, contributors.len());
            for contributor in contributors {
                hash_location(hash, *contributor);
            }
        }
        FamilySiteProvenance::CurveAlongGuide {
            location,
            guide_order,
            sequence,
            absolute_arc_position_bits,
            local_arc_position_bits,
        } => {
            hash_bytes(hash, &[4]);
            hash_location(hash, *location);
            hash_length(hash, *guide_order);
            hash_bytes(hash, &sequence.to_le_bytes());
            hash_bytes(hash, &absolute_arc_position_bits.to_le_bytes());
            hash_bytes(hash, &local_arc_position_bits.to_le_bytes());
        }
        FamilySiteProvenance::AlongParametricCurve {
            location,
            path_order,
            sequence,
            absolute_arc_position_bits,
            local_arc_position_bits,
        } => {
            hash_bytes(hash, &[5]);
            hash_location(hash, *location);
            hash_length(hash, *path_order);
            hash_bytes(hash, &sequence.to_le_bytes());
            hash_bytes(hash, &absolute_arc_position_bits.to_le_bytes());
            hash_bytes(hash, &local_arc_position_bits.to_le_bytes());
        }
    }
}

/// Hashes one guide-source contributor in its complete stable identity order.
fn hash_guide_instance(hash: &mut u64, guide: GuideInstanceId) {
    hash_bytes(hash, &guide.dimension_id.to_le_bytes());
    hash_bytes(hash, &guide.index.to_le_bytes());
    hash_bytes(hash, &guide.component_ordinal.to_le_bytes());
}

/// Hashes one exact structural path source identity.
fn hash_path_instance(hash: &mut u64, path: StructuralPathInstanceId) {
    match path.source {
        StructuralPathSourceId::GuideDimension(id) => {
            hash_bytes(hash, &[0]);
            hash_bytes(hash, &id.0.to_le_bytes());
        }
        StructuralPathSourceId::ParametricCurve(id) => {
            hash_bytes(hash, &[1]);
            hash_bytes(hash, &id.0.to_le_bytes());
        }
    }
    hash_bytes(hash, &path.repetition_index.to_le_bytes());
    hash_bytes(hash, &path.component_ordinal.to_le_bytes());
}

/// Hashes one segment-local structural location without inferring source semantics.
fn hash_location(hash: &mut u64, location: StructuralPathLocationProvenance) {
    hash_path_instance(hash, location.path);
    hash_length(hash, location.segment_index);
    hash_bytes(hash, &location.parameter_bits.to_le_bytes());
}

/// Hashes platform-width counts through one stable `u64` representation.
fn hash_length(hash: &mut u64, value: usize) {
    hash_bytes(hash, &(value as u64).to_le_bytes());
}

/// Updates the deterministic FNV-1a identity accumulator.
fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// Hashes one variable-length payload with an unambiguous little-endian length prefix.
fn hash_length_delimited(hash: &mut u64, bytes: &[u8]) {
    hash_bytes(hash, &(bytes.len() as u64).to_le_bytes());
    hash_bytes(hash, bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proves fallible vector reservation reports the stable allocation diagnostic without panic.
    #[test]
    fn reserve_maps_capacity_overflow_to_allocation_error() {
        let error = reserve(&mut Vec::<u8>::new(), usize::MAX)
            .expect_err("impossible vector capacity must fail atomically");
        assert_eq!(error.path(), "adjacency.allocation");
    }
}
