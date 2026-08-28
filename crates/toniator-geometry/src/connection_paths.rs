//! Deterministic positive open paths selected from a Stage 20L adjacency graph.

use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use toniator_domain::{ConnectionProgram, PatternOutputLayerId};

use super::{
    CurvePath, CurveSegment, FamilySiteId, LineSegment, PathClosure, SiteAdjacencyEdge,
    SiteAdjacencyGraph, SiteAdjacencyPolicy,
};

/// Stable identity for the connection-program selection and trail contract.
pub const CONNECTION_PATH_CONTRACT_ID: &str = "toniator-stage-20m-connection-paths-v1";
/// Stable identity for deterministic mutual-nearest selection without a seeded algorithm.
pub const CONNECTION_NEAREST_SELECTION_CONTRACT_ID: &str = "toniator-stage-20m-nearest-links-v1";
/// Stable identity for the seeded FNV random-links selection algorithm.
pub const CONNECTION_RANDOM_SELECTION_CONTRACT_ID: &str =
    "toniator-stage-20m-random-links-fnv1a-v1";
/// Stable identity for the seeded randomized-Prim spanning-tree selection algorithm.
pub const CONNECTION_PRIM_SELECTION_CONTRACT_ID: &str = "toniator-stage-20m-randomized-prim-v1";
/// Stable identity for selected-edge component decomposition into open trails.
pub const CONNECTION_TRAIL_CONTRACT_ID: &str = "toniator-stage-20m-open-trails-v1";

/// Returns the fixed geometry-owned selection contract for one authored program.
pub const fn connection_program_contract_id(program: &ConnectionProgram) -> &'static str {
    match program {
        ConnectionProgram::NearestLinks { .. } => CONNECTION_NEAREST_SELECTION_CONTRACT_ID,
        ConnectionProgram::RandomLinks { .. } => CONNECTION_RANDOM_SELECTION_CONTRACT_ID,
        ConnectionProgram::GridSpanningTree { .. } => CONNECTION_PRIM_SELECTION_CONTRACT_ID,
    }
}

/// Stable identity for one positive emitted connection path.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConnectionPathId {
    pub output_layer_id: PatternOutputLayerId,
    /// Smallest source node in the selected graph component; this remains stable when component ordering shifts.
    pub component_minimum: FamilySiteId,
    pub component_ordinal: u32,
    pub first_endpoint: FamilySiteId,
    pub last_endpoint: FamilySiteId,
    pub ordinal: u32,
}

/// One positive, open, line-only canonical connection path.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionPath {
    pub id: ConnectionPathId,
    pub vertices: Vec<FamilySiteId>,
    pub path: CurvePath,
    pub nominal_basis: f64,
}

/// Best-effort facts that do not change connection identity or produce geometry by themselves.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConnectionPathDiagnostics {
    pub under_connected_nodes: Vec<FamilySiteId>,
    pub isolated_nodes: Vec<FamilySiteId>,
}

/// Complete derived result; operational limits and diagnostics are intentionally outside its fingerprint.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionPathSet {
    pub selected_edges: Vec<SiteAdjacencyEdge>,
    pub paths: Vec<ConnectionPath>,
    pub diagnostics: ConnectionPathDiagnostics,
    fingerprint: String,
}

impl ConnectionPathSet {
    /// Returns the stable derived identity without exposing mutable work policy.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// Configurable bounded work policy for graph selection and open-trail construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionPathLimits {
    pub maximum_selected_edges: usize,
    pub maximum_trails: usize,
    pub maximum_retained_path_points: usize,
    pub maximum_inspections: usize,
}

impl ConnectionPathLimits {
    /// Creates a complete enabled work policy.
    ///
    /// # Errors
    ///
    /// Returns `connection.limits` when any required work category is disabled.
    pub fn new(
        maximum_selected_edges: usize,
        maximum_trails: usize,
        maximum_retained_path_points: usize,
        maximum_inspections: usize,
    ) -> Result<Self, ConnectionPathError> {
        let value = Self {
            maximum_selected_edges,
            maximum_trails,
            maximum_retained_path_points,
            maximum_inspections,
        };
        if [
            value.maximum_selected_edges,
            value.maximum_trails,
            value.maximum_retained_path_points,
            value.maximum_inspections,
        ]
        .contains(&0)
        {
            return Err(ConnectionPathError::new(
                "connection.limits",
                "all connection limits must be nonzero",
            ));
        }
        Ok(value)
    }
}

impl Default for ConnectionPathLimits {
    /// Supplies the exact nonzero Stage 20M connection-work defaults.
    fn default() -> Self {
        Self {
            maximum_selected_edges: 1_048_576,
            maximum_trails: 1_048_576,
            maximum_retained_path_points: 2_097_152,
            maximum_inspections: 33_554_432,
        }
    }
}

/// Stable atomic failure for connection selection or path construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionPathError {
    path: &'static str,
    message: &'static str,
}

impl ConnectionPathError {
    /// Creates one stable connection diagnostic without retaining partial geometry.
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }

    /// Returns the source-owned stable diagnostic path.
    pub const fn path(&self) -> &'static str {
        self.path
    }

    /// Returns the stable human-readable diagnostic message.
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl std::fmt::Display for ConnectionPathError {
    /// Formats the stable error without exposing partial selected edges or trails.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for ConnectionPathError {}

/// Selects program edges and decomposes them into deterministic positive open paths.
///
/// # Errors
///
/// Returns only stable `connection.*` or `evaluation.cancelled` diagnostics and never returns a
/// partial graph selection, path collection, or fingerprint.
pub fn build_connection_paths_cancellable(
    output_layer_id: PatternOutputLayerId,
    graph: &SiteAdjacencyGraph,
    program: &ConnectionProgram,
    limits: ConnectionPathLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<ConnectionPathSet, ConnectionPathError> {
    program
        .validate()
        .map_err(|error| ConnectionPathError::new(error.path(), error.message()))?;
    ConnectionPathLimits::new(
        limits.maximum_selected_edges,
        limits.maximum_trails,
        limits.maximum_retained_path_points,
        limits.maximum_inspections,
    )?;
    cancelled(is_cancelled)?;
    let intent = program.adjacency();
    let expected = SiteAdjacencyPolicy::MutualNearest {
        maximum_degree: intent.maximum_degree as usize,
        maximum_distance: intent.maximum_distance,
    };
    if graph.policy() != expected {
        return Err(ConnectionPathError::new(
            "connection.graph.policy",
            "connection program adjacency does not match the supplied graph policy",
        ));
    }
    let mut work = Work::new(limits.maximum_inspections, is_cancelled);
    let (selected_edges, diagnostics) = select_edges(graph, program, limits, &mut work)?;
    let paths = decompose(graph, output_layer_id, &selected_edges, limits, &mut work)?;
    let fingerprint = fingerprint(
        output_layer_id,
        graph,
        program,
        &selected_edges,
        &paths,
        &mut work,
    )?;
    Ok(ConnectionPathSet {
        selected_edges,
        paths,
        diagnostics,
        fingerprint,
    })
}

/// Reserves one fallible connection-work vector capacity before material allocation.
///
/// # Errors
///
/// Returns `connection.allocation` when the requested capacity cannot be represented or reserved.
fn reserve_connection<T>(
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), ConnectionPathError> {
    values.try_reserve(additional).map_err(|_| {
        ConnectionPathError::new("connection.allocation", "connection work allocation failed")
    })
}

struct Work<'a> {
    remaining: usize,
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a> Work<'a> {
    /// Creates an inspection counter that polls cancellation before every bounded observation.
    fn new(remaining: usize, cancelled: &'a dyn Fn() -> bool) -> Self {
        Self {
            remaining,
            cancelled,
        }
    }

    /// Accounts for one selection, traversal, or identity observation.
    fn tick(&mut self) -> Result<(), ConnectionPathError> {
        cancelled(self.cancelled)?;
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(ConnectionPathError::new(
                "connection.limits.inspections",
                "connection selection or traversal exceeds the inspection limit",
            ))?;
        Ok(())
    }
}

/// Returns program-selected edges in canonical endpoint order and non-fatal degree diagnostics.
fn select_edges(
    graph: &SiteAdjacencyGraph,
    program: &ConnectionProgram,
    limits: ConnectionPathLimits,
    work: &mut Work<'_>,
) -> Result<(Vec<SiteAdjacencyEdge>, ConnectionPathDiagnostics), ConnectionPathError> {
    let mut diagnostics = ConnectionPathDiagnostics::default();
    let mut incident = BTreeMap::<FamilySiteId, Vec<SiteAdjacencyEdge>>::new();
    for node in graph.nodes() {
        work.tick()?;
        incident.entry(node.id).or_default();
    }
    for edge in graph.edges() {
        work.tick()?;
        incident.entry(edge.first).or_default().push(*edge);
        incident.entry(edge.second).or_default().push(*edge);
    }
    for edges in incident.values_mut() {
        edges.sort();
    }
    let mut selected = BTreeSet::new();
    match program {
        ConnectionProgram::NearestLinks { .. } => {
            for edge in graph.edges() {
                work.tick()?;
                selected.insert(*edge);
            }
        }
        ConnectionProgram::RandomLinks {
            adjacency,
            minimum_degree,
            seed,
        } => select_random(
            &incident,
            *minimum_degree as usize,
            adjacency.maximum_degree as usize,
            *seed,
            &mut selected,
            work,
        )?,
        ConnectionProgram::GridSpanningTree { seed, .. } => {
            select_prim(graph, &incident, *seed, &mut selected, work)?
        }
    }
    if selected.len() > limits.maximum_selected_edges {
        return Err(ConnectionPathError::new(
            "connection.limits.selected_edges",
            "selected connection edges exceed the configured limit",
        ));
    }
    let mut selected_degrees = BTreeMap::<FamilySiteId, usize>::new();
    for edge in &selected {
        work.tick()?;
        *selected_degrees.entry(edge.first).or_default() += 1;
        *selected_degrees.entry(edge.second).or_default() += 1;
    }
    for (node, edges) in &incident {
        work.tick()?;
        if edges.is_empty() {
            diagnostics.isolated_nodes.push(*node);
        }
        if let ConnectionProgram::RandomLinks { minimum_degree, .. } = program {
            let degree = selected_degrees.get(node).copied().unwrap_or(0);
            if degree < *minimum_degree as usize {
                diagnostics.under_connected_nodes.push(*node);
            }
        }
    }
    let mut selected_edges = Vec::new();
    reserve_connection(&mut selected_edges, selected.len())?;
    for edge in selected {
        work.tick()?;
        selected_edges.push(edge);
    }
    Ok((selected_edges, diagnostics))
}

/// Selects a bounded best-effort random degree program without exceeding authored maxima.
fn select_random(
    incident: &BTreeMap<FamilySiteId, Vec<SiteAdjacencyEdge>>,
    minimum: usize,
    maximum: usize,
    seed: u32,
    selected: &mut BTreeSet<SiteAdjacencyEdge>,
    work: &mut Work<'_>,
) -> Result<(), ConnectionPathError> {
    let mut targets = BTreeMap::new();
    for node in incident.keys() {
        work.tick()?;
        let range = maximum - minimum + 1;
        targets.insert(
            *node,
            minimum + (priority(seed, *node, *node) as usize % range),
        );
    }
    let mut unique = BTreeSet::new();
    for edges in incident.values() {
        for edge in edges {
            work.tick()?;
            unique.insert(*edge);
        }
    }
    let mut ranked = Vec::new();
    reserve_connection(&mut ranked, unique.len())?;
    for edge in unique {
        work.tick()?;
        ranked.push((priority(seed, edge.first, edge.second), edge));
    }
    ranked.sort_unstable();
    let mut degree = BTreeMap::<FamilySiteId, usize>::new();
    for (_, edge) in &ranked {
        work.tick()?;
        let first = *degree.get(&edge.first).unwrap_or(&0);
        let second = *degree.get(&edge.second).unwrap_or(&0);
        if !selected.contains(edge)
            && (first < minimum || second < minimum)
            && first < maximum
            && second < maximum
        {
            selected.insert(*edge);
            *degree.entry(edge.first).or_default() += 1;
            *degree.entry(edge.second).or_default() += 1;
        }
    }
    for (_, edge) in &ranked {
        work.tick()?;
        let first = *degree.get(&edge.first).unwrap_or(&0);
        let second = *degree.get(&edge.second).unwrap_or(&0);
        if !selected.contains(edge)
            && (first < *targets.get(&edge.first).unwrap_or(&0)
                || second < *targets.get(&edge.second).unwrap_or(&0))
            && first < maximum
            && second < maximum
        {
            selected.insert(*edge);
            *degree.entry(edge.first).or_default() += 1;
            *degree.entry(edge.second).or_default() += 1;
        }
    }
    Ok(())
}

/// Selects independent deterministic-priority Prim trees for every nonisolated graph component.
fn select_prim(
    graph: &SiteAdjacencyGraph,
    incident: &BTreeMap<FamilySiteId, Vec<SiteAdjacencyEdge>>,
    seed: u32,
    selected: &mut BTreeSet<SiteAdjacencyEdge>,
    work: &mut Work<'_>,
) -> Result<(), ConnectionPathError> {
    for component in graph.components() {
        let Some(&root) = component.members.first() else {
            continue;
        };
        if incident.get(&root).is_none_or(Vec::is_empty) {
            continue;
        }
        let mut visited = BTreeSet::from([root]);
        let mut queue = BinaryHeap::<std::cmp::Reverse<(u64, SiteAdjacencyEdge)>>::new();
        queue.try_reserve(incident[&root].len()).map_err(|_| {
            ConnectionPathError::new(
                "connection.allocation",
                "connection priority allocation failed",
            )
        })?;
        for edge in &incident[&root] {
            work.tick()?;
            queue.push(std::cmp::Reverse((
                priority(seed, edge.first, edge.second),
                *edge,
            )));
        }
        while let Some(std::cmp::Reverse((_, edge))) = queue.pop() {
            work.tick()?;
            let first_seen = visited.contains(&edge.first);
            let second_seen = visited.contains(&edge.second);
            if first_seen == second_seen {
                continue;
            }
            let next = if first_seen { edge.second } else { edge.first };
            visited.insert(next);
            selected.insert(edge);
            queue.try_reserve(incident[&next].len()).map_err(|_| {
                ConnectionPathError::new(
                    "connection.allocation",
                    "connection priority allocation failed",
                )
            })?;
            for candidate in &incident[&next] {
                work.tick()?;
                queue.push(std::cmp::Reverse((
                    priority(seed, candidate.first, candidate.second),
                    *candidate,
                )));
            }
        }
    }
    Ok(())
}

/// Decomposes every selected edge exactly once into deterministic open line paths.
fn decompose(
    graph: &SiteAdjacencyGraph,
    output_layer_id: PatternOutputLayerId,
    selected: &[SiteAdjacencyEdge],
    limits: ConnectionPathLimits,
    work: &mut Work<'_>,
) -> Result<Vec<ConnectionPath>, ConnectionPathError> {
    let mut positions = BTreeMap::new();
    for node in graph.nodes() {
        work.tick()?;
        positions.insert(node.id, (node.position, node.nominal_cell_basis));
    }
    let by_component = selected_edge_components(selected, work)?;
    let mut all = Vec::new();
    reserve_connection(&mut all, selected.len())?;
    let mut points = 0usize;
    for (component_ordinal, (component, edges)) in by_component.into_iter().enumerate() {
        let component_ordinal = u32::try_from(component_ordinal).map_err(|_| {
            ConnectionPathError::new(
                "connection.identity",
                "connection component ordinal exceeds u32 identity capacity",
            )
        })?;
        let sequences = trails_for_component(&edges, work)?;
        let mut open_sequences = Vec::new();
        reserve_connection(&mut open_sequences, sequences.len().saturating_add(1))?;
        for sequence in sequences {
            append_curve_path_fragments(&mut open_sequences, sequence)?;
        }
        for vertices in open_sequences {
            work.tick()?;
            if all.len() >= limits.maximum_trails {
                return Err(ConnectionPathError::new(
                    "connection.limits.trails",
                    "connection trail count exceeds the configured limit",
                ));
            }
            points = points
                .checked_add(vertices.len())
                .ok_or(ConnectionPathError::new(
                    "connection.limits.path_points",
                    "connection path points exceed the configured limit",
                ))?;
            if points > limits.maximum_retained_path_points {
                return Err(ConnectionPathError::new(
                    "connection.limits.path_points",
                    "connection path points exceed the configured limit",
                ));
            }
            let mut path_edges = Vec::new();
            reserve_connection(&mut path_edges, vertices.len().saturating_sub(1))?;
            for pair in vertices.windows(2) {
                work.tick()?;
                let start = positions
                    .get(&pair[0])
                    .ok_or(ConnectionPathError::new(
                        "connection.geometry",
                        "connection vertex is missing a finite source position",
                    ))?
                    .0;
                let end = positions
                    .get(&pair[1])
                    .ok_or(ConnectionPathError::new(
                        "connection.geometry",
                        "connection vertex is missing a finite source position",
                    ))?
                    .0;
                let length = (end.x - start.x).hypot(end.y - start.y);
                // The curve authority classifies derivatives no longer than this fixed absolute
                // tolerance as stationary. Reject such selected adjacency here so no partial
                // connection result can escape with a generic curve diagnostic.
                if !length.is_finite() || length <= 1.0e-9 {
                    return Err(ConnectionPathError::new(
                        "connection.geometry",
                        "connection edge must have a positive nonstationary centerline",
                    ));
                }
                path_edges.push(CurveSegment::Line(LineSegment::new(start, end).map_err(
                    |_| {
                        ConnectionPathError::new(
                            "connection.geometry",
                            "connection segment must remain finite",
                        )
                    },
                )?));
            }
            let path = CurvePath::new(path_edges, PathClosure::Open).map_err(|_| {
                ConnectionPathError::new(
                    "connection.geometry",
                    "connection path must be finite and open",
                )
            })?;
            let mut nominal_basis = f64::INFINITY;
            for vertex in &vertices {
                work.tick()?;
                nominal_basis = nominal_basis.min(
                    positions
                        .get(vertex)
                        .ok_or(ConnectionPathError::new(
                            "connection.geometry",
                            "connection vertex is missing a nominal cell basis",
                        ))?
                        .1
                        .diameter(),
                );
            }
            if !nominal_basis.is_finite() || nominal_basis <= 0.0 {
                return Err(ConnectionPathError::new(
                    "connection.geometry",
                    "connection nominal basis must remain finite and positive",
                ));
            }
            let first = vertices[0];
            let last = *vertices.last().expect("nonempty path vertices");
            all.push(ConnectionPath {
                id: ConnectionPathId {
                    output_layer_id,
                    component_minimum: component,
                    component_ordinal,
                    first_endpoint: first.min(last),
                    last_endpoint: first.max(last),
                    ordinal: 0,
                },
                vertices,
                path,
                nominal_basis,
            });
        }
        let _ = component;
    }
    all.sort_by(|left, right| {
        (
            left.id.component_minimum,
            left.id.component_ordinal,
            left.id.first_endpoint,
            left.id.last_endpoint,
            &left.vertices,
        )
            .cmp(&(
                right.id.component_minimum,
                right.id.component_ordinal,
                right.id.first_endpoint,
                right.id.last_endpoint,
                &right.vertices,
            ))
    });
    for (ordinal, path) in all.iter_mut().enumerate() {
        work.tick()?;
        path.id.ordinal = u32::try_from(ordinal).map_err(|_| {
            ConnectionPathError::new(
                "connection.identity",
                "connection path ordinal exceeds u32 identity capacity",
            )
        })?;
    }
    Ok(all)
}

/// Groups selected edges by their own connected components in minimum-node order.
///
/// Source adjacency components deliberately do not participate: seeded random selection may
/// retain multiple disconnected edge components inside one source component.
///
/// # Errors
///
/// Returns cancellation, inspection, or allocation diagnostics before any path is emitted.
fn selected_edge_components(
    selected: &[SiteAdjacencyEdge],
    work: &mut Work<'_>,
) -> Result<Vec<(FamilySiteId, Vec<SiteAdjacencyEdge>)>, ConnectionPathError> {
    let mut incident = BTreeMap::<FamilySiteId, Vec<SiteAdjacencyEdge>>::new();
    for edge in selected {
        work.tick()?;
        incident.entry(edge.first).or_default().push(*edge);
        incident.entry(edge.second).or_default().push(*edge);
    }
    let mut seen = BTreeSet::new();
    let mut components = Vec::new();
    reserve_connection(&mut components, selected.len())?;
    for edge in selected {
        work.tick()?;
        if seen.contains(edge) {
            continue;
        }
        let mut stack = Vec::new();
        let node_capacity = selected
            .len()
            .checked_mul(2)
            .ok_or(ConnectionPathError::new(
                "connection.allocation",
                "connection selected-component capacity overflows",
            ))?;
        reserve_connection(&mut stack, node_capacity)?;
        stack.push(edge.first);
        stack.push(edge.second);
        let mut members = BTreeSet::new();
        let mut edges = BTreeSet::new();
        while let Some(node) = stack.pop() {
            work.tick()?;
            if !members.insert(node) {
                continue;
            }
            for candidate in incident.get(&node).ok_or(ConnectionPathError::new(
                "connection.graph.component",
                "selected edge has no selected-component incidence",
            ))? {
                work.tick()?;
                if edges.insert(*candidate) {
                    stack.push(other(*candidate, node));
                }
            }
        }
        for edge in &edges {
            work.tick()?;
            seen.insert(*edge);
        }
        let minimum = *members.first().ok_or(ConnectionPathError::new(
            "connection.graph.component",
            "selected edge component has no member",
        ))?;
        let mut ordered = Vec::new();
        reserve_connection(&mut ordered, edges.len())?;
        ordered.extend(edges);
        components.push((minimum, ordered));
    }
    components.sort_by_key(|(minimum, _)| *minimum);
    Ok(components)
}

/// Returns the minimum-count open trail decomposition by deterministic unused-edge walks.
fn trails_for_component(
    edges: &[SiteAdjacencyEdge],
    work: &mut Work<'_>,
) -> Result<Vec<Vec<FamilySiteId>>, ConnectionPathError> {
    let mut degrees = BTreeMap::<FamilySiteId, usize>::new();
    for edge in edges {
        work.tick()?;
        *degrees.entry(edge.first).or_default() += 1;
        *degrees.entry(edge.second).or_default() += 1;
    }
    if edges.len() > 1 && degrees.values().all(|degree| degree % 2 == 0) {
        let split = *edges.iter().min().expect("nonempty even component");
        let mut remaining = Vec::new();
        reserve_connection(&mut remaining, edges.len().saturating_sub(1))?;
        for edge in edges {
            work.tick()?;
            if *edge != split {
                remaining.push(*edge);
            }
        }
        let mut paths = Vec::new();
        reserve_connection(&mut paths, 2)?;
        paths.push(vec![split.first, split.second]);
        paths.extend(trails_for_component(&remaining, work)?);
        return Ok(paths);
    }
    #[derive(Clone, Copy)]
    struct Edge {
        first: FamilySiteId,
        second: FamilySiteId,
        virtual_edge: bool,
    }
    let mut augmented = Vec::new();
    reserve_connection(&mut augmented, edges.len())?;
    for edge in edges {
        work.tick()?;
        augmented.push(Edge {
            first: edge.first,
            second: edge.second,
            virtual_edge: false,
        });
    }
    let mut odds = Vec::new();
    reserve_connection(&mut odds, degrees.len())?;
    for (node, degree) in &degrees {
        work.tick()?;
        if degree % 2 == 1 {
            odds.push(*node);
        }
    }
    reserve_connection(&mut augmented, odds.len() / 2)?;
    for pair in odds.chunks_exact(2) {
        work.tick()?;
        augmented.push(Edge {
            first: pair[0],
            second: pair[1],
            virtual_edge: true,
        });
    }
    let mut adjacency = BTreeMap::<FamilySiteId, Vec<usize>>::new();
    for (index, edge) in augmented.iter().enumerate() {
        work.tick()?;
        adjacency.entry(edge.first).or_default().push(index);
        adjacency.entry(edge.second).or_default().push(index);
    }
    for indices in adjacency.values_mut() {
        indices.sort_by_key(|index| {
            let edge = augmented[*index];
            (edge.virtual_edge, edge.first, edge.second)
        });
    }
    let start = *adjacency.keys().next().expect("nonempty component");
    let mut used = Vec::new();
    reserve_connection(&mut used, augmented.len())?;
    used.resize(augmented.len(), false);
    let mut stack = Vec::new();
    reserve_connection(&mut stack, augmented.len().saturating_add(1))?;
    stack.push((start, None::<usize>));
    let mut reversed = Vec::<(FamilySiteId, Option<usize>)>::new();
    reserve_connection(&mut reversed, augmented.len().saturating_add(1))?;
    while let Some((node, _)) = stack.last().copied() {
        work.tick()?;
        let mut candidate = None;
        for index in &adjacency[&node] {
            work.tick()?;
            if !used[*index] {
                candidate = Some(*index);
                break;
            }
        }
        if let Some(index) = candidate {
            used[index] = true;
            let edge = augmented[index];
            stack.push((
                if edge.first == node {
                    edge.second
                } else {
                    edge.first
                },
                Some(index),
            ));
        } else {
            reversed.push(stack.pop().expect("nonempty traversal stack"));
        }
    }
    reversed.reverse();
    let mut output = Vec::new();
    reserve_connection(&mut output, odds.len() / 2 + 1)?;
    let mut current = vec![reversed[0].0];
    for (vertex, incoming) in reversed.into_iter().skip(1) {
        work.tick()?;
        if augmented[incoming.expect("every nonstart vertex has an edge")].virtual_edge {
            append_curve_path_fragments(&mut output, current)?;
            current = vec![vertex];
        } else {
            current.push(vertex);
        }
    }
    append_curve_path_fragments(&mut output, current)?;
    Ok(output)
}

/// Appends one or more edge-continuous fragments that satisfy the fixed `CurvePath` segment
/// bound without changing any trail that already fits that representation.
///
/// Consecutive fragments overlap by one endpoint, so every selected edge appears exactly once
/// across the emitted sequences. Allocation failure is reported before `decompose` publishes a
/// `ConnectionPath`; empty and one-vertex inputs remain absent because they own no edge.
fn append_curve_path_fragments(
    output: &mut Vec<Vec<FamilySiteId>>,
    mut trail: Vec<FamilySiteId>,
) -> Result<(), ConnectionPathError> {
    if trail.len() <= 1 {
        return Ok(());
    }
    if trail[0] > *trail.last().expect("nonempty connection trail") {
        trail.reverse();
    }
    const MAXIMUM_SEGMENTS: usize = 4_096;
    let mut start = 0usize;
    while start + 1 < trail.len() {
        let end = trail.len().min(start.saturating_add(MAXIMUM_SEGMENTS + 1));
        let mut fragment = Vec::new();
        reserve_connection(&mut fragment, end - start)?;
        fragment.extend_from_slice(&trail[start..end]);
        output.push(fragment);
        start = end - 1;
    }
    Ok(())
}

/// Returns the opposite canonical endpoint of one non-loop edge.
fn other(edge: SiteAdjacencyEdge, node: FamilySiteId) -> FamilySiteId {
    if edge.first == node {
        edge.second
    } else {
        edge.first
    }
}

/// Computes the fixed FNV-1a priority contract from a seed and canonical endpoint IDs.
fn priority(seed: u32, first: FamilySiteId, second: FamilySiteId) -> u64 {
    let (first, second) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };
    let mut value = 0xcbf2_9ce4_8422_2325_u64;
    for byte in seed
        .to_le_bytes()
        .into_iter()
        .chain(first.mechanism_id.0.to_le_bytes())
        .chain((first.ordinal as u64).to_le_bytes())
        .chain(second.mechanism_id.0.to_le_bytes())
        .chain((second.ordinal as u64).to_le_bytes())
    {
        value ^= u64::from(byte);
        value = value.wrapping_mul(0x100_0000_01b3);
    }
    value
}

/// Builds the stable content identity while explicitly excluding diagnostics and operational limits.
fn fingerprint(
    output: PatternOutputLayerId,
    graph: &SiteAdjacencyGraph,
    program: &ConnectionProgram,
    edges: &[SiteAdjacencyEdge],
    paths: &[ConnectionPath],
    work: &mut Work<'_>,
) -> Result<String, ConnectionPathError> {
    let mut hasher = ConnectionHasher::new();
    hasher.text(CONNECTION_PATH_CONTRACT_ID);
    hasher.text(CONNECTION_TRAIL_CONTRACT_ID);
    hasher.bytes(&output.0.to_le_bytes());
    hasher.text(graph.fingerprint());
    append_program(&mut hasher, program);
    for edge in edges {
        work.tick()?;
        hasher.site_id(edge.first);
        hasher.site_id(edge.second);
    }
    for path in paths {
        work.tick()?;
        hasher.bytes(&path.id.output_layer_id.0.to_le_bytes());
        hasher.site_id(path.id.component_minimum);
        hasher.bytes(&path.id.component_ordinal.to_le_bytes());
        hasher.site_id(path.id.first_endpoint);
        hasher.site_id(path.id.last_endpoint);
        hasher.bytes(&path.id.ordinal.to_le_bytes());
        for id in &path.vertices {
            work.tick()?;
            hasher.site_id(*id);
        }
    }
    hasher.finish()
}

/// Appends complete authored program values and fixed algorithm contracts.
fn append_program(hasher: &mut ConnectionHasher, program: &ConnectionProgram) {
    let adjacency = program.adjacency();
    hasher.bytes(&adjacency.maximum_degree.to_le_bytes());
    hasher.bytes(&adjacency.maximum_distance.to_bits().to_le_bytes());
    match program {
        ConnectionProgram::NearestLinks { .. } => hasher.byte(1),
        ConnectionProgram::RandomLinks {
            minimum_degree,
            seed,
            ..
        } => {
            hasher.byte(2);
            hasher.text(CONNECTION_RANDOM_SELECTION_CONTRACT_ID);
            hasher.bytes(&minimum_degree.to_le_bytes());
            hasher.bytes(&seed.to_le_bytes());
        }
        ConnectionProgram::GridSpanningTree { seed, .. } => {
            hasher.byte(3);
            hasher.text(CONNECTION_PRIM_SELECTION_CONTRACT_ID);
            hasher.bytes(&seed.to_le_bytes());
        }
    }
}

/// Polls cancellation before an allocation or traversal boundary.
fn cancelled(is_cancelled: &dyn Fn() -> bool) -> Result<(), ConnectionPathError> {
    if is_cancelled() {
        Err(ConnectionPathError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ))
    } else {
        Ok(())
    }
}

/// Streams the fixed connection FNV-1a encoding without an unbounded byte buffer.
struct ConnectionHasher(u64);

impl ConnectionHasher {
    /// Starts the fixed connection identity hash.
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325_u64)
    }

    /// Incorporates one byte of stable identity data.
    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(0x100_0000_01b3);
    }

    /// Incorporates a fixed byte sequence.
    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.byte(*byte);
        }
    }

    /// Incorporates one length-delimited textual identity field.
    fn text(&mut self, value: &str) {
        self.bytes(&(value.len() as u64).to_le_bytes());
        self.bytes(value.as_bytes());
    }

    /// Incorporates one stable family-site identity without presentation formatting.
    fn site_id(&mut self, id: FamilySiteId) {
        self.bytes(&id.mechanism_id.0.to_le_bytes());
        self.bytes(&(id.ordinal as u64).to_le_bytes());
    }

    /// Finishes the fixed hexadecimal FNV-1a identity with fallible result allocation.
    ///
    /// # Errors
    ///
    /// Returns `connection.allocation` when the fixed sixteen-byte fingerprint cannot reserve.
    fn finish(self) -> Result<String, ConnectionPathError> {
        use std::fmt::Write;

        let mut value = String::new();
        value.try_reserve_exact(16).map_err(|_| {
            ConnectionPathError::new(
                "connection.allocation",
                "connection fingerprint allocation failed",
            )
        })?;
        write!(&mut value, "{:016x}", self.0).expect("reserved string formatting is infallible");
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_domain::PatternMechanismId;

    /// Maps a representational vector-reservation failure onto the stable connection allocation path.
    #[test]
    fn fallible_connection_reservation_reports_allocation_failure() {
        let mut values = Vec::<u8>::new();
        let error = reserve_connection(&mut values, usize::MAX).expect_err("overflow rejects");
        assert_eq!(error.path(), "connection.allocation");
    }

    /// Splits an oversized Euler trail only at shared endpoints so the fixed curve-path bound
    /// cannot reject a dense valid connection component or duplicate a selected edge.
    #[test]
    fn oversized_trail_fragments_preserve_every_edge_once_and_openly() {
        let trail = (0..=4_097)
            .map(|ordinal| FamilySiteId {
                mechanism_id: PatternMechanismId(91),
                ordinal,
            })
            .collect::<Vec<_>>();
        let mut fragments = Vec::new();
        append_curve_path_fragments(&mut fragments, trail.clone()).expect("fragmentation succeeds");
        assert_eq!(fragments.len(), 2);
        assert!(fragments.iter().all(|fragment| fragment.len() <= 4_097));
        assert_eq!(fragments[0].last(), fragments[1].first());
        let emitted = fragments
            .iter()
            .flat_map(|fragment| fragment.windows(2))
            .map(|edge| (edge[0], edge[1]))
            .collect::<Vec<_>>();
        let expected = trail
            .windows(2)
            .map(|edge| (edge[0], edge[1]))
            .collect::<Vec<_>>();
        assert_eq!(emitted, expected);
    }
}
