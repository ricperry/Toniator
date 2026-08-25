//! Conventional wall-maze arrangement and dual traversal derived from guide-intersection sites.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use toniator_domain::{GridMazeAlgorithm, MazeProgram, PatternOutputLayerId};

use super::{
    Bounds, CurvePath, FamilySite, FamilySiteProvenance, FamilySiteSet, GuideInstanceId, Point2,
    Vector2,
};

/// Stable identity for the bounded guide-arrangement and dual wall-maze contract.
pub const MAZE_WALL_CONTRACT_ID: &str = "toniator-stage-20m-wall-maze-v1";

/// One normalized tangent supplied by the evaluated straight-guide family for maze wall ordering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MazeGuideAxis {
    pub id: GuideInstanceId,
    tangent: Vector2,
}

impl MazeGuideAxis {
    /// Validates and normalizes the authoritative tangent for one actual guide instance.
    ///
    /// # Errors
    ///
    /// Returns `maze.family.guide_axis` when the finite guide authority has no usable direction.
    pub fn new(id: GuideInstanceId, tangent: Vector2) -> Result<Self, MazeError> {
        let length = tangent.x.hypot(tangent.y);
        if !length.is_finite() || length <= 1e-12 {
            return Err(MazeError::new(
                "maze.family.guide_axis",
                "maze guide tangents must be finite and nonzero",
            ));
        }
        Ok(Self {
            id,
            tangent: Vector2::new(tangent.x / length, tangent.y / length),
        })
    }
}

/// One canonical undirected primal wall connecting consecutive sites on one actual guide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MazeWallId {
    pub first: MazeVertexId,
    pub second: MazeVertexId,
}

impl MazeWallId {
    /// Canonicalizes one non-loop source-wall identity from two site IDs.
    ///
    /// # Errors
    ///
    /// Returns a stable arrangement diagnostic when both endpoints are identical.
    pub fn new(first: MazeVertexId, second: MazeVertexId) -> Result<Self, MazeError> {
        if first == second {
            return Err(MazeError::new(
                "maze.source_walls.loop",
                "source walls require two distinct site endpoints",
            ));
        }
        Ok(if first < second {
            Self { first, second }
        } else {
            Self {
                first: second,
                second: first,
            }
        })
    }
}

/// Stable maze-local vertex identity assigned from exact quantized geometry and contributor order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MazeVertexId(pub u32);

/// One exact family site paired with its envelope-independent maze-local vertex identity.
#[derive(Clone, Debug, PartialEq)]
pub struct MazeSourceSite {
    pub id: MazeVertexId,
    pub source: FamilySite,
}

/// One immutable primal arrangement wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MazeWall {
    pub id: MazeWallId,
}

/// Stable bounded-face identity assigned in canonical boundary order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MazeCellId(pub u32);

/// One bounded arrangement face; its vertices remain site IDs in cyclic order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MazeCell {
    pub id: MazeCellId,
    pub vertices: Vec<MazeVertexId>,
    pub walls: Vec<MazeWallId>,
}

/// Stable identity for one dual adjacency across exactly one source wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MazeDualEdgeId(pub u32);

/// One dual adjacency between two bounded cells across one primal source wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MazeDualEdge {
    pub id: MazeDualEdgeId,
    pub first: MazeCellId,
    pub second: MazeCellId,
    pub shared_wall: MazeWallId,
}

/// Stable identity for one positive retained wall path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MazeWallPathId {
    pub output_layer_id: PatternOutputLayerId,
    pub wall: MazeWallId,
}

/// One positive open line path retained as a conventional maze wall.
#[derive(Clone, Debug, PartialEq)]
pub struct MazeWallPath {
    pub id: MazeWallPathId,
    pub vertices: [MazeVertexId; 2],
    pub path: CurvePath,
    pub nominal_basis: f64,
}

/// Maps one canonical source wall to every bounded cell sharing it.
type WallFaces = BTreeMap<MazeWallId, Vec<MazeCellId>>;

/// One predecessor map retained for deterministic dual-tree route reconstruction.
type TreePredecessors = BTreeMap<MazeCellId, (MazeCellId, MazeWallId)>;

/// One bidirectional selected-passage tree indexed by each reachable maze cell.
///
/// Standard `BTreeMap`/`BTreeSet` insertion is deterministic but cannot report allocation failure
/// through the Rust standard library; all adjacent material `Vec`/`VecDeque` products reserve
/// fallibly.
type MazeTree = BTreeMap<MazeCellId, BTreeSet<(MazeCellId, MazeWallId)>>;

/// One deterministic cell-to-cell solution through removed passage walls only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MazeSolution {
    pub entrance: MazeCellId,
    pub exit: MazeCellId,
    pub cells: Vec<MazeCellId>,
    pub passage_walls: Vec<MazeWallId>,
}

/// One derived perimeter opening removed from the positive wall complement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MazeOpening {
    pub wall: MazeWallId,
    pub point: Point2,
    pub cell: MazeCellId,
    pub side: MazeOpeningSide,
}

/// Nearest document-canvas side used only to choose deterministic wall openings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MazeOpeningSide {
    Left,
    Right,
    Top,
    Bottom,
}

/// Non-fatal arrangement facts excluded from the result fingerprint.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MazeDiagnostics {
    /// Counts reachable cells not used by the one derived entrance-to-exit solution.
    pub off_solution_cells: usize,
    /// Counts selected-tree cells with one incident passage, excluding the one-cell maze.
    pub dead_end_cells: usize,
    /// Counts selected-tree cells with three or more incident passages.
    pub branch_cells: usize,
}

/// Complete immutable conventional maze result before paint/stroke realization.
#[derive(Clone, Debug, PartialEq)]
pub struct MazeProgramResult {
    /// Exact inclusive candidate-arrangement sites, including sites outside emitted components.
    ///
    /// These retain the actual evaluated guide-family identity and provenance used before
    /// largest-component selection. Only `cells`, `source_walls`, and positive paths describe
    /// the emitted connected bounded-face component.
    pub source_sites: Vec<MazeSourceSite>,
    /// Exact input-site positions retained for self-contained wall and cell geometry inspection.
    pub source_site_positions: BTreeMap<MazeVertexId, Point2>,
    pub source_walls: Vec<MazeWall>,
    pub cells: Vec<MazeCell>,
    pub dual_edges: Vec<MazeDualEdge>,
    pub perimeter_walls: Vec<MazeWallId>,
    pub removed_passage_walls: Vec<MazeWallId>,
    pub entrance: MazeOpening,
    pub exit: MazeOpening,
    pub retained_walls: Vec<MazeWall>,
    pub wall_paths: Vec<MazeWallPath>,
    pub solution: MazeSolution,
    pub diagnostics: MazeDiagnostics,
    fingerprint: String,
}

impl MazeProgramResult {
    /// Returns the stable derived maze identity without exposing work-policy details.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// Bounded arrangement, dual-selection, and wall-path work policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MazeLimits {
    pub maximum_source_walls: usize,
    pub maximum_faces: usize,
    pub maximum_dual_adjacencies: usize,
    pub maximum_passages: usize,
    pub maximum_wall_trails: usize,
    pub maximum_retained_points: usize,
    pub maximum_inspections: usize,
}

impl MazeLimits {
    /// Creates one fully enabled wall-maze work policy.
    ///
    /// # Errors
    ///
    /// Returns `maze.limits` when any independently bounded work category is disabled.
    pub fn new(
        maximum_source_walls: usize,
        maximum_faces: usize,
        maximum_dual_adjacencies: usize,
        maximum_passages: usize,
        maximum_wall_trails: usize,
        maximum_retained_points: usize,
        maximum_inspections: usize,
    ) -> Result<Self, MazeError> {
        let limits = Self {
            maximum_source_walls,
            maximum_faces,
            maximum_dual_adjacencies,
            maximum_passages,
            maximum_wall_trails,
            maximum_retained_points,
            maximum_inspections,
        };
        if [
            limits.maximum_source_walls,
            limits.maximum_faces,
            limits.maximum_dual_adjacencies,
            limits.maximum_passages,
            limits.maximum_wall_trails,
            limits.maximum_retained_points,
            limits.maximum_inspections,
        ]
        .contains(&0)
        {
            return Err(MazeError::new(
                "maze.limits",
                "all maze limits must be nonzero",
            ));
        }
        Ok(limits)
    }
}

impl Default for MazeLimits {
    /// Supplies finite defaults for the current bounded straight-guide maze implementation.
    fn default() -> Self {
        Self {
            maximum_source_walls: 1_048_576,
            maximum_faces: 1_048_576,
            maximum_dual_adjacencies: 1_048_576,
            maximum_passages: 1_048_576,
            maximum_wall_trails: 1_048_576,
            maximum_retained_points: 2_097_152,
            maximum_inspections: 33_554_432,
        }
    }
}

/// Stable atomic arrangement, selection, or path-construction failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MazeError {
    path: &'static str,
    message: &'static str,
}

impl MazeError {
    /// Creates one stable wall-maze diagnostic without retaining partial geometry.
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }

    /// Returns the geometry-owned stable diagnostic path.
    pub const fn path(&self) -> &'static str {
        self.path
    }

    /// Returns the human-readable stable diagnostic message.
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl std::fmt::Display for MazeError {
    /// Formats the stable failure without exposing a partial arrangement.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for MazeError {}

/// Derives a conventional wall maze from on-or-inside guide-intersection sites and a dual DFS.
///
/// The canvas admits every evaluated site on or inside its bounds, then derives every positively
/// oriented bounded arrangement face solely from those sites. If that finite arrangement has
/// multiple disconnected face components, only its stable largest component emits maze walls. The
/// canvas never adds arrangement walls, cells, passages, or endpoints.
///
/// # Errors
///
/// Returns a stable `maze.*` or `evaluation.cancelled` error without exposing partial output.
pub fn build_maze_walls_cancellable(
    output_layer_id: PatternOutputLayerId,
    sites: &FamilySiteSet,
    canvas: Bounds,
    guide_axes: &[MazeGuideAxis],
    program: &MazeProgram,
    limits: MazeLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<MazeProgramResult, MazeError> {
    build_maze_walls_from_sites_cancellable(
        output_layer_id,
        sites.family_fingerprint(),
        canvas,
        guide_axes,
        sites.sites(),
        program,
        limits,
        is_cancelled,
    )
}

/// Derives a conventional maze from an ID/provenance/basis-preserving selected site slice.
///
/// # Errors
///
/// Returns a stable arrangement or cancellation error without publishing partial wall geometry.
#[allow(clippy::too_many_arguments)] // The public seam keeps all independent geometry authorities explicit.
pub fn build_maze_walls_from_sites_cancellable(
    output_layer_id: PatternOutputLayerId,
    family_fingerprint: &str,
    canvas: Bounds,
    guide_axes: &[MazeGuideAxis],
    sites: &[FamilySite],
    program: &MazeProgram,
    limits: MazeLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<MazeProgramResult, MazeError> {
    program
        .validate()
        .map_err(|error| MazeError::new(error.path(), error.message()))?;
    MazeLimits::new(
        limits.maximum_source_walls,
        limits.maximum_faces,
        limits.maximum_dual_adjacencies,
        limits.maximum_passages,
        limits.maximum_wall_trails,
        limits.maximum_retained_points,
        limits.maximum_inspections,
    )?;
    let mut work = Work::new(limits.maximum_inspections, is_cancelled);
    let in_canvas = in_canvas_sites(sites, canvas, &mut work)?;
    let source_sites = canonical_source_sites(&in_canvas, &mut work)?;
    let positions = positions(&source_sites, &mut work)?;
    let bases = nominal_bases(&source_sites, &mut work)?;
    let arrangement_walls = source_walls(&source_sites, guide_axes, &positions, limits, &mut work)?;
    let (arrangement_cells, arrangement_wall_faces) =
        bounded_faces(&arrangement_walls, &positions, limits, &mut work)?;
    let (cells, wall_faces) = select_largest_bounded_face_component(
        &arrangement_cells,
        &arrangement_wall_faces,
        &mut work,
    )?;
    let source_walls = bounded_source_walls(&arrangement_walls, &wall_faces, &mut work)?;
    let dual_edges = dual_edges(&wall_faces, limits, &mut work)?;
    ensure_dual_connected(&cells, &dual_edges, &mut work)?;
    let (removed_passage_walls, tree) =
        select_passages(&cells, &dual_edges, &positions, program, limits, &mut work)?;
    let (perimeter_walls, openings) = select_openings(&wall_faces, &positions, canvas, &mut work)?;
    let solution = solution_between(openings.0.cell, openings.1.cell, &tree, &mut work)?;
    let diagnostics = maze_diagnostics(&cells, &tree, &solution, &mut work)?;
    let mut removed = BTreeSet::new();
    for wall in &removed_passage_walls {
        work.tick()?;
        removed.insert(*wall);
    }
    removed.insert(openings.0.wall);
    removed.insert(openings.1.wall);
    let mut retained_walls = Vec::new();
    reserve(
        &mut retained_walls,
        source_walls.len(),
        "retained-wall allocation failed",
    )?;
    for wall in &source_walls {
        work.tick()?;
        if !removed.contains(wall) {
            retained_walls.push(MazeWall { id: *wall });
        }
    }
    if retained_walls.len() > limits.maximum_wall_trails {
        return Err(MazeError::new(
            "maze.limits.wall_trails",
            "retained maze walls exceed the trail limit",
        ));
    }
    let points = retained_walls.len().checked_mul(2).ok_or(MazeError::new(
        "maze.limits.points",
        "retained wall points overflow",
    ))?;
    if points > limits.maximum_retained_points {
        return Err(MazeError::new(
            "maze.limits.points",
            "retained maze wall points exceed the limit",
        ));
    }
    let mut wall_paths = Vec::new();
    reserve(
        &mut wall_paths,
        retained_walls.len(),
        "wall-path allocation failed",
    )?;
    for wall in &retained_walls {
        work.tick()?;
        wall_paths.push(wall_path(output_layer_id, wall.id, &positions, &bases)?);
    }
    let fingerprint = fingerprint(
        family_fingerprint,
        program,
        &source_sites,
        &source_walls,
        &cells,
        &dual_edges,
        &removed_passage_walls,
        &retained_walls,
        &wall_paths,
        &solution,
        openings,
        &mut work,
    )?;
    let mut source_wall_output = Vec::new();
    reserve(
        &mut source_wall_output,
        source_walls.len(),
        "source-wall result allocation failed",
    )?;
    for id in source_walls {
        work.tick()?;
        source_wall_output.push(MazeWall { id });
    }
    Ok(MazeProgramResult {
        source_sites,
        source_site_positions: positions,
        source_walls: source_wall_output,
        cells,
        dual_edges,
        perimeter_walls,
        removed_passage_walls,
        entrance: openings.0,
        exit: openings.1,
        retained_walls,
        wall_paths,
        solution,
        diagnostics,
        fingerprint,
    })
}

/// Tracks every bounded maze inspection and cancellation poll.
struct Work<'a> {
    remaining: usize,
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a> Work<'a> {
    /// Creates one bounded inspection authority for the active maze request.
    fn new(remaining: usize, cancelled: &'a dyn Fn() -> bool) -> Self {
        Self {
            remaining,
            cancelled,
        }
    }

    /// Polls cancellation and consumes one inspected arrangement or traversal element.
    fn tick(&mut self) -> Result<(), MazeError> {
        if (self.cancelled)() {
            return Err(MazeError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        self.remaining = self.remaining.checked_sub(1).ok_or(MazeError::new(
            "maze.limits.inspections",
            "maze arrangement or traversal exceeds the inspection limit",
        ))?;
        Ok(())
    }
}

/// Selects every exact family site inside or on the document canvas before maze topology.
///
/// Inclusive site authority preserves actual guide intersections on canvas edges. Later face
/// extraction uses only bounded faces from this candidate arrangement and never creates a canvas
/// edge, site, wall, or face.
fn in_canvas_sites(
    sites: &[FamilySite],
    canvas: Bounds,
    work: &mut Work<'_>,
) -> Result<Vec<FamilySite>, MazeError> {
    let mut count = 0_usize;
    for site in sites {
        work.tick()?;
        if canvas.contains(site.position) {
            count = count.checked_add(1).ok_or(MazeError::new(
                "maze.allocation",
                "in-canvas site count overflows",
            ))?;
        }
    }
    let mut values = Vec::new();
    reserve(&mut values, count, "in-canvas site allocation failed")?;
    for site in sites {
        work.tick()?;
        if canvas.contains(site.position) {
            values.push(site.clone());
        }
    }
    Ok(values)
}

/// Reserves one material maze vector through a stable allocation-error mapping.
///
/// `Vec` exposes fallible reservation while standard `BTreeMap`/`BTreeSet` insertion does not;
/// callers use this seam for material result vectors so allocation failure cannot leak a partial
/// result.
///
/// # Errors
///
/// Returns `maze.allocation` when the requested capacity cannot be reserved.
fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    context: &'static str,
) -> Result<(), MazeError> {
    values
        .try_reserve(additional)
        .map_err(|_| MazeError::new("maze.allocation", context))
}

/// Assigns local maze vertices from envelope-independent exact geometry and contributor identity.
///
/// # Errors
///
/// Returns provenance, allocation, inspection, or cancellation errors before arrangement construction.
fn canonical_source_sites(
    sites: &[FamilySite],
    work: &mut Work<'_>,
) -> Result<Vec<MazeSourceSite>, MazeError> {
    let mut ordered = Vec::new();
    ordered
        .try_reserve(sites.len())
        .map_err(|_| MazeError::new("maze.allocation", "maze source-site allocation failed"))?;
    for site in sites {
        work.tick()?;
        let FamilySiteProvenance::GuideIntersection { contributors } = &site.provenance else {
            return Err(MazeError::new(
                "maze.family.provenance",
                "maze walls require guide-intersection site contributors",
            ));
        };
        ordered.push((site.clone(), contributors.clone()));
    }
    ordered.sort_unstable_by(|left, right| {
        quantized_point(left.0.position)
            .cmp(&quantized_point(right.0.position))
            .then(left.1.cmp(&right.1))
            .then(
                left.0
                    .nominal_cell_basis
                    .diameter()
                    .total_cmp(&right.0.nominal_cell_basis.diameter()),
            )
    });
    for pair in ordered.windows(2) {
        if quantized_point(pair[0].0.position) == quantized_point(pair[1].0.position)
            && pair[0].1 == pair[1].1
        {
            return Err(MazeError::new(
                "maze.family.sites",
                "maze input repeats one geometric guide-intersection site",
            ));
        }
    }
    let mut values = Vec::new();
    values
        .try_reserve(ordered.len())
        .map_err(|_| MazeError::new("maze.allocation", "maze vertex allocation failed"))?;
    for (ordinal, (source, _)) in ordered.into_iter().enumerate() {
        work.tick()?;
        values.push(MazeSourceSite {
            id: MazeVertexId(
                u32::try_from(ordinal).map_err(|_| {
                    MazeError::new("maze.vertices", "maze vertex ordinal overflows")
                })?,
            ),
            source,
        });
    }
    Ok(values)
}

/// Copies exact site positions into a maze-local vertex lookup.
fn positions(
    sites: &[MazeSourceSite],
    work: &mut Work<'_>,
) -> Result<BTreeMap<MazeVertexId, Point2>, MazeError> {
    let mut values = BTreeMap::new();
    for site in sites {
        work.tick()?;
        values.insert(site.id, site.source.position);
    }
    Ok(values)
}

/// Copies exact site nominal-cell diameters into the retained-wall width authority.
fn nominal_bases(
    sites: &[MazeSourceSite],
    work: &mut Work<'_>,
) -> Result<BTreeMap<MazeVertexId, f64>, MazeError> {
    let mut values = BTreeMap::new();
    for site in sites {
        work.tick()?;
        values.insert(site.id, site.source.nominal_cell_basis.diameter());
    }
    Ok(values)
}

/// Connects consecutive exact sites along every actual guide contributor into canonical source walls.
fn source_walls(
    sites: &[MazeSourceSite],
    guide_axes: &[MazeGuideAxis],
    positions: &BTreeMap<MazeVertexId, Point2>,
    limits: MazeLimits,
    work: &mut Work<'_>,
) -> Result<Vec<MazeWallId>, MazeError> {
    let axes = guide_axes
        .iter()
        .copied()
        .map(|axis| (axis.id, axis.tangent))
        .collect::<BTreeMap<_, _>>();
    let mut grouped = BTreeMap::<GuideInstanceId, Vec<MazeVertexId>>::new();
    for site in sites {
        work.tick()?;
        let FamilySiteProvenance::GuideIntersection { contributors } = &site.source.provenance
        else {
            return Err(MazeError::new(
                "maze.family.provenance",
                "maze walls require guide-intersection site contributors",
            ));
        };
        for contributor in contributors {
            work.tick()?;
            grouped.entry(*contributor).or_default().push(site.id);
        }
    }
    let mut walls = BTreeSet::new();
    for (guide, ids) in &mut grouped {
        let tangent = axes.get(guide).copied().ok_or(MazeError::new(
            "maze.family.guide_axis",
            "maze site contributor lacks its evaluated straight-guide tangent",
        ))?;
        sort_guide_sites(ids, tangent, positions, work)?;
        ids.dedup();
        for pair in ids.windows(2) {
            work.tick()?;
            let first = positions[&pair[0]];
            let second = positions[&pair[1]];
            if distance(first, second) <= 1e-12 {
                return Err(MazeError::new(
                    "maze.source_walls.geometry",
                    "consecutive guide sites must have a positive separation",
                ));
            }
            walls.insert(MazeWallId::new(pair[0], pair[1])?);
            if walls.len() > limits.maximum_source_walls {
                return Err(MazeError::new(
                    "maze.limits.source_walls",
                    "source maze walls exceed the limit",
                ));
            }
        }
    }
    if walls.is_empty() {
        return Err(MazeError::new(
            "maze.source_walls",
            "guide sites produce no source walls",
        ));
    }
    let mut values = Vec::new();
    values
        .try_reserve(walls.len())
        .map_err(|_| MazeError::new("maze.allocation", "source-wall allocation failed"))?;
    values.extend(walls);
    Ok(values)
}

/// Orders one actual-guide site group along its evaluated normalized tangent.
///
/// The supplied family guide remains the authority for direction; quantized positions only break
/// equal projected positions so finite line-intersection roundoff cannot reorder retained sites.
///
/// # Errors
///
/// Returns an inspection or cancellation failure before source-wall construction continues.
fn sort_guide_sites(
    ids: &mut Vec<MazeVertexId>,
    tangent: Vector2,
    positions: &BTreeMap<MazeVertexId, Point2>,
    work: &mut Work<'_>,
) -> Result<(), MazeError> {
    let mut ordered = Vec::new();
    ordered
        .try_reserve(ids.len())
        .map_err(|_| MazeError::new("maze.allocation", "guide ordering allocation failed"))?;
    for id in ids.iter().copied() {
        work.tick()?;
        let point = positions[&id];
        let (x, y) = quantized_point(point);
        ordered.push((point.x.mul_add(tangent.x, point.y * tangent.y), x, y, id));
    }
    ordered.sort_unstable_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
            .then(left.3.cmp(&right.3))
    });
    ids.clear();
    ids.extend(ordered.into_iter().map(|(_, _, _, id)| id));
    Ok(())
}

/// Extracts bounded arrangement faces from ordered half-edges without introducing canvas edges.
fn bounded_faces(
    walls: &[MazeWallId],
    positions: &BTreeMap<MazeVertexId, Point2>,
    limits: MazeLimits,
    work: &mut Work<'_>,
) -> Result<(Vec<MazeCell>, WallFaces), MazeError> {
    let mut outgoing = BTreeMap::<MazeVertexId, Vec<MazeVertexId>>::new();
    for wall in walls {
        work.tick()?;
        outgoing.entry(wall.first).or_default().push(wall.second);
        outgoing.entry(wall.second).or_default().push(wall.first);
    }
    for (vertex, neighbors) in &mut outgoing {
        neighbors.sort_by(|left, right| {
            angle(positions[vertex], positions[left])
                .total_cmp(&angle(positions[vertex], positions[right]))
        });
    }
    let mut visited = BTreeSet::<(MazeVertexId, MazeVertexId)>::new();
    let mut cycles = Vec::<Vec<MazeVertexId>>::new();
    cycles
        .try_reserve(walls.len())
        .map_err(|_| MazeError::new("maze.allocation", "bounded-face allocation failed"))?;
    for wall in walls {
        for start in [(wall.first, wall.second), (wall.second, wall.first)] {
            if visited.contains(&start) {
                continue;
            }
            let mut cycle = Vec::new();
            let mut current = start;
            loop {
                work.tick()?;
                if !visited.insert(current) {
                    if current == start {
                        break;
                    }
                    return Err(MazeError::new(
                        "maze.faces",
                        "half-edge traversal revisited an incomplete face",
                    ));
                }
                cycle.push(current.0);
                let neighbors = &outgoing[&current.1];
                let reverse = neighbors
                    .iter()
                    .position(|value| *value == current.0)
                    .ok_or(MazeError::new("maze.faces", "half-edge reverse is missing"))?;
                let next = neighbors[crate::planar_arrangement::predecessor_of_reverse_index(
                    reverse,
                    neighbors.len(),
                )];
                current = (current.1, next);
            }
            if cycle.len() >= 3 && polygon_area(&cycle, positions) > 1e-10 {
                cycles.push(canonical_cycle(cycle, positions));
            }
        }
    }
    let mut bounded = cycles;
    if bounded.is_empty() {
        return Err(MazeError::new(
            "maze.faces",
            "arrangement contains no bounded face",
        ));
    }
    bounded.sort_by(|left, right| compare_cycles_by_position(left, right, positions));
    if bounded.len() > limits.maximum_faces {
        return Err(MazeError::new(
            "maze.limits.faces",
            "bounded maze faces exceed the limit",
        ));
    }
    let mut wall_faces = BTreeMap::<MazeWallId, Vec<MazeCellId>>::new();
    let mut cells = Vec::new();
    cells
        .try_reserve(bounded.len())
        .map_err(|_| MazeError::new("maze.allocation", "maze cell allocation failed"))?;
    for (index, vertices) in bounded.into_iter().enumerate() {
        let id = MazeCellId(
            u32::try_from(index)
                .map_err(|_| MazeError::new("maze.faces", "face ordinal overflows"))?,
        );
        let mut walls = Vec::new();
        walls
            .try_reserve(vertices.len())
            .map_err(|_| MazeError::new("maze.allocation", "cell-wall allocation failed"))?;
        for (first, second) in vertices
            .iter()
            .copied()
            .zip(vertices.iter().copied().cycle().skip(1))
            .take(vertices.len())
        {
            walls.push(MazeWallId::new(first, second)?);
        }
        for wall in &walls {
            wall_faces.entry(*wall).or_default().push(id);
        }
        cells.push(MazeCell {
            id,
            vertices,
            walls,
        });
    }
    Ok((cells, wall_faces))
}

/// Derives dual cell adjacencies only where exactly two bounded faces share one source wall.
fn dual_edges(
    wall_faces: &BTreeMap<MazeWallId, Vec<MazeCellId>>,
    limits: MazeLimits,
    work: &mut Work<'_>,
) -> Result<Vec<MazeDualEdge>, MazeError> {
    let mut edges = Vec::new();
    edges
        .try_reserve(wall_faces.len())
        .map_err(|_| MazeError::new("maze.allocation", "dual-edge allocation failed"))?;
    for (wall, faces) in wall_faces {
        work.tick()?;
        if faces.len() == 2 {
            let id = MazeDualEdgeId(
                u32::try_from(edges.len())
                    .map_err(|_| MazeError::new("maze.dual", "dual-edge ordinal overflows"))?,
            );
            edges.push(MazeDualEdge {
                id,
                first: faces[0],
                second: faces[1],
                shared_wall: *wall,
            });
            if edges.len() > limits.maximum_dual_adjacencies {
                return Err(MazeError::new(
                    "maze.limits.dual_adjacencies",
                    "dual maze adjacencies exceed the limit",
                ));
            }
        }
    }
    Ok(edges)
}

/// Retains only arrangement edges that bound at least one completed maze cell.
///
/// This removes dangling in-canvas guide fragments before they can affect perimeter ranking,
/// retained-wall output, or maze identity.
///
/// # Errors
///
/// Returns allocation or cancellation failures before derived source walls are published.
fn bounded_source_walls(
    walls: &[MazeWallId],
    wall_faces: &WallFaces,
    work: &mut Work<'_>,
) -> Result<Vec<MazeWallId>, MazeError> {
    let mut count = 0_usize;
    for wall in walls {
        work.tick()?;
        if wall_faces.contains_key(wall) {
            count = count.checked_add(1).ok_or(MazeError::new(
                "maze.allocation",
                "bounded source-wall count overflows",
            ))?;
        }
    }
    let mut values = Vec::new();
    reserve(&mut values, count, "bounded source-wall allocation failed")?;
    for wall in walls {
        work.tick()?;
        if wall_faces.contains_key(wall) {
            values.push(*wall);
        }
    }
    Ok(values)
}

/// Retains the largest deterministic connected component of bounded arrangement faces.
///
/// Source-site eligibility is exclusively `Bounds::contains`; this selection deliberately has no
/// stroke-width, site-inset, canvas-proximity, or unused-outward-segment policy. Every positively
/// oriented bounded face formed solely from inclusive evaluated sites is eligible. The largest
/// stable dual component becomes the emitted maze; disconnected components emit no fragment.
/// Every inclusive source site remains candidate and fingerprint authority, and compact maze-local
/// cell IDs avoid family-envelope ordinal dependence.
///
/// # Errors
///
/// Returns `maze.cells` when no bounded face exists. A one-cell arrangement remains
/// valid, preserving two perimeter openings. Stable allocation or cancellation errors occur
/// before publishing topology.
fn select_largest_bounded_face_component(
    arrangement_cells: &[MazeCell],
    arrangement_wall_faces: &WallFaces,
    work: &mut Work<'_>,
) -> Result<(Vec<MazeCell>, WallFaces), MazeError> {
    if arrangement_cells.is_empty() {
        return Err(MazeError::new(
            "maze.cells",
            "maze requires one complete bounded arrangement cell",
        ));
    }
    let mut active = BTreeSet::new();
    for cell in arrangement_cells {
        work.tick()?;
        active.insert(cell.id);
    }
    let mut neighbors = BTreeMap::<MazeCellId, BTreeSet<MazeCellId>>::new();
    for cell in arrangement_cells {
        work.tick()?;
        neighbors.entry(cell.id).or_default();
    }
    for faces in arrangement_wall_faces.values() {
        work.tick()?;
        if let [first, second] = faces.as_slice() {
            neighbors.entry(*first).or_default().insert(*second);
            neighbors.entry(*second).or_default().insert(*first);
        }
    }
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    queue
        .try_reserve(active.len())
        .map_err(|_| MazeError::new("maze.allocation", "maze core traversal allocation failed"))?;
    let mut best = Vec::new();
    reserve(
        &mut best,
        active.len(),
        "maze core component allocation failed",
    )?;
    for start in &active {
        work.tick()?;
        if !visited.insert(*start) {
            continue;
        }
        let mut component = Vec::new();
        reserve(
            &mut component,
            active.len(),
            "maze core component allocation failed",
        )?;
        queue.push_back(*start);
        while let Some(cell) = queue.pop_front() {
            work.tick()?;
            component.push(cell);
            for neighbor in &neighbors[&cell] {
                work.tick()?;
                if active.contains(neighbor) && visited.insert(*neighbor) {
                    queue.push_back(*neighbor);
                }
            }
        }
        if component.len() > best.len()
            || (component.len() == best.len() && component.first() < best.first())
        {
            best = component;
        }
    }
    let selected = best.into_iter().collect::<BTreeSet<_>>();
    let mut cells = Vec::new();
    reserve(
        &mut cells,
        selected.len(),
        "maze core cell allocation failed",
    )?;
    for cell in arrangement_cells {
        work.tick()?;
        if !selected.contains(&cell.id) {
            continue;
        }
        let id =
            MazeCellId(u32::try_from(cells.len()).map_err(|_| {
                MazeError::new("maze.core.cells", "maze core cell ordinal overflows")
            })?);
        let mut vertices = Vec::new();
        reserve(
            &mut vertices,
            cell.vertices.len(),
            "maze core vertex allocation failed",
        )?;
        for vertex in &cell.vertices {
            work.tick()?;
            vertices.push(*vertex);
        }
        let mut walls = Vec::new();
        reserve(
            &mut walls,
            cell.walls.len(),
            "maze core wall allocation failed",
        )?;
        for wall in &cell.walls {
            work.tick()?;
            walls.push(*wall);
        }
        cells.push(MazeCell {
            id,
            vertices,
            walls,
        });
    }
    let mut wall_faces = BTreeMap::<MazeWallId, Vec<MazeCellId>>::new();
    for cell in &cells {
        work.tick()?;
        for wall in &cell.walls {
            work.tick()?;
            wall_faces.entry(*wall).or_default().push(cell.id);
        }
    }
    Ok((cells, wall_faces))
}

/// Selects two arrangement-perimeter walls, preferring opposite canvas sides then separation.
///
/// Perimeter ownership comes solely from completed arrangement cells. Canvas-side classification
/// is a deterministic ranking hint, not a proximity requirement and never introduces a wall.
fn select_openings(
    wall_faces: &WallFaces,
    positions: &BTreeMap<MazeVertexId, Point2>,
    canvas: Bounds,
    work: &mut Work<'_>,
) -> Result<(Vec<MazeWallId>, (MazeOpening, MazeOpening)), MazeError> {
    let mut candidates = Vec::new();
    reserve(
        &mut candidates,
        wall_faces.len(),
        "maze opening allocation failed",
    )?;
    for (wall, cells) in wall_faces {
        work.tick()?;
        if cells.len() == 1 {
            let first = positions[&wall.first];
            let second = positions[&wall.second];
            let point = Point2::new((first.x + second.x) / 2.0, (first.y + second.y) / 2.0);
            candidates.push(MazeOpening {
                wall: *wall,
                point,
                cell: cells[0],
                side: opening_side(point, canvas),
            });
        }
    }
    if candidates.len() < 2 {
        return Err(MazeError::new(
            "maze.openings",
            "maze requires two distinct perimeter wall openings",
        ));
    }
    candidates.sort_by_key(|opening| opening.wall);
    let width = canvas.max.x - canvas.min.x;
    let height = canvas.max.y - canvas.min.y;
    let mut best: Option<(bool, f64, MazeOpening, MazeOpening)> = None;
    for (index, first) in candidates.iter().copied().enumerate() {
        for second in candidates.iter().copied().skip(index + 1) {
            work.tick()?;
            let dx = ((first.point.x - second.point.x) / width).abs();
            let dy = ((first.point.y - second.point.y) / height).abs();
            let opposite = matches!(
                (first.side, second.side),
                (MazeOpeningSide::Left, MazeOpeningSide::Right)
                    | (MazeOpeningSide::Right, MazeOpeningSide::Left)
                    | (MazeOpeningSide::Top, MazeOpeningSide::Bottom)
                    | (MazeOpeningSide::Bottom, MazeOpeningSide::Top)
            );
            let score = dx.hypot(dy);
            let candidate = (opposite, score, first, second);
            if best.as_ref().is_none_or(|current| {
                (candidate.0 && !current.0)
                    || (candidate.0 == current.0
                        && (candidate.1 > current.1 + 1e-12
                            || ((candidate.1 - current.1).abs() <= 1e-12
                                && (candidate.2.wall, candidate.3.wall)
                                    < (current.2.wall, current.3.wall))))
            }) {
                best = Some(candidate);
            }
        }
    }
    let (_, _, first, second) = best.expect("at least one distinct opening pair");
    let mut perimeter_walls = Vec::new();
    reserve(
        &mut perimeter_walls,
        candidates.len(),
        "maze perimeter-wall allocation failed",
    )?;
    for opening in candidates {
        work.tick()?;
        perimeter_walls.push(opening.wall);
    }
    Ok((perimeter_walls, (first, second)))
}

/// Classifies a perimeter-wall midpoint by nearest normalized canvas side with stable ties.
fn opening_side(point: Point2, canvas: Bounds) -> MazeOpeningSide {
    let width = canvas.max.x - canvas.min.x;
    let height = canvas.max.y - canvas.min.y;
    let values = [
        ((point.x - canvas.min.x) / width, MazeOpeningSide::Left),
        ((canvas.max.x - point.x) / width, MazeOpeningSide::Right),
        ((point.y - canvas.min.y) / height, MazeOpeningSide::Top),
        ((canvas.max.y - point.y) / height, MazeOpeningSide::Bottom),
    ];
    values
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .expect("four canvas sides")
        .1
}

/// Rejects an emitted bounded-face component unless all retained cells share one dual component.
///
/// # Errors
///
/// Returns `maze.dual.connected` before passages or openings are selected when the emitted
/// component would produce a forest. Inclusive source-arrangement fringe components are
/// intentionally handled by largest-component selection before this invariant, and bounded
/// cancellation/allocation failures remain atomic.
fn ensure_dual_connected(
    cells: &[MazeCell],
    dual_edges: &[MazeDualEdge],
    work: &mut Work<'_>,
) -> Result<(), MazeError> {
    let start = cells
        .first()
        .ok_or(MazeError::new(
            "maze.cells",
            "maze requires one bounded cell",
        ))?
        .id;
    let mut neighbors = BTreeMap::<MazeCellId, BTreeSet<MazeCellId>>::new();
    for edge in dual_edges {
        work.tick()?;
        neighbors.entry(edge.first).or_default().insert(edge.second);
        neighbors.entry(edge.second).or_default().insert(edge.first);
    }
    let mut queue = VecDeque::new();
    queue
        .try_reserve(cells.len())
        .map_err(|_| MazeError::new("maze.allocation", "dual-connectivity allocation failed"))?;
    let mut visited = BTreeSet::new();
    visited.insert(start);
    queue.push_back(start);
    while let Some(current) = queue.pop_front() {
        work.tick()?;
        for next in neighbors.get(&current).into_iter().flatten() {
            work.tick()?;
            if visited.insert(*next) {
                queue.push_back(*next);
            }
        }
    }
    if visited.len() != cells.len() {
        return Err(MazeError::new(
            "maze.dual.connected",
            "bounded maze cells must form one connected dual component",
        ));
    }
    Ok(())
}

/// Selects one seeded recursive-backtracker spanning tree over a prevalidated connected dual.
fn select_passages(
    cells: &[MazeCell],
    dual_edges: &[MazeDualEdge],
    positions: &BTreeMap<MazeVertexId, Point2>,
    program: &MazeProgram,
    limits: MazeLimits,
    work: &mut Work<'_>,
) -> Result<(Vec<MazeWallId>, MazeTree), MazeError> {
    let GridMazeAlgorithm::RecursiveBacktracker = program.algorithm;
    let mut neighbors = BTreeMap::<MazeCellId, BTreeSet<(MazeCellId, MazeWallId)>>::new();
    for edge in dual_edges {
        work.tick()?;
        neighbors
            .entry(edge.first)
            .or_default()
            .insert((edge.second, edge.shared_wall));
        neighbors
            .entry(edge.second)
            .or_default()
            .insert((edge.first, edge.shared_wall));
    }
    let mut visited = BTreeSet::new();
    let mut passages = Vec::new();
    passages
        .try_reserve(cells.len().saturating_sub(1))
        .map_err(|_| MazeError::new("maze.allocation", "maze passage allocation failed"))?;
    let start = cells
        .first()
        .ok_or(MazeError::new(
            "maze.cells",
            "maze requires one bounded cell",
        ))?
        .id;
    visited.insert(start);
    let mut stack = Vec::new();
    reserve(&mut stack, cells.len(), "maze DFS stack allocation failed")?;
    stack.push(start);
    let mut tree = MazeTree::new();
    while let Some(&current) = stack.last() {
        work.tick()?;
        let mut selected = None;
        for (next, wall) in neighbors.get(&current).into_iter().flatten() {
            work.tick()?;
            if !visited.contains(next) {
                let candidate = (
                    priority(program.seed, current, *next, *wall, positions),
                    *next,
                    *wall,
                );
                if selected
                    .as_ref()
                    .is_none_or(|current: &(u64, MazeCellId, MazeWallId)| candidate < *current)
                {
                    selected = Some(candidate);
                }
            }
        }
        if let Some((_, next, wall)) = selected {
            if passages.len() >= limits.maximum_passages {
                return Err(MazeError::new(
                    "maze.limits.passages",
                    "selected maze passages exceed the limit",
                ));
            }
            visited.insert(next);
            passages.push(wall);
            tree.entry(current).or_default().insert((next, wall));
            tree.entry(next).or_default().insert((current, wall));
            stack.push(next);
        } else {
            stack.pop();
        }
    }
    let expected_passages = cells.len().checked_sub(1).ok_or(MazeError::new(
        "maze.cells",
        "maze requires one bounded cell",
    ))?;
    if visited.len() != cells.len() || passages.len() != expected_passages {
        return Err(MazeError::new(
            "maze.selection.tree",
            "recursive backtracker must select exactly one spanning tree",
        ));
    }
    passages.sort();
    Ok((passages, tree))
}

/// Reconstructs the unique selected-tree route between the two opening-adjacent cells.
fn solution_between(
    entrance: MazeCellId,
    exit: MazeCellId,
    tree: &MazeTree,
    work: &mut Work<'_>,
) -> Result<MazeSolution, MazeError> {
    work.tick()?;
    let mut queue = VecDeque::new();
    let queue_capacity = tree.len().checked_add(1).ok_or(MazeError::new(
        "maze.allocation",
        "solution queue allocation size overflows",
    ))?;
    queue
        .try_reserve(queue_capacity)
        .map_err(|_| MazeError::new("maze.allocation", "solution queue allocation failed"))?;
    queue.push_back(entrance);
    let mut previous = TreePredecessors::new();
    let mut visited = BTreeSet::from([entrance]);
    while let Some(current) = queue.pop_front() {
        work.tick()?;
        if current == exit {
            break;
        }
        for (next, wall) in tree.get(&current).into_iter().flatten() {
            work.tick()?;
            if visited.insert(*next) {
                previous.insert(*next, (current, *wall));
                queue.push_back(*next);
            }
        }
    }
    let (cells, walls) = route(entrance, exit, &previous, work)?;
    Ok(MazeSolution {
        entrance,
        exit,
        cells,
        passage_walls: walls,
    })
}

/// Reconstructs one unique tree route using predecessors retained by a breadth-first traversal.
fn route(
    start: MazeCellId,
    end: MazeCellId,
    previous: &TreePredecessors,
    work: &mut Work<'_>,
) -> Result<(Vec<MazeCellId>, Vec<MazeWallId>), MazeError> {
    let capacity = previous.len().checked_add(1).ok_or(MazeError::new(
        "maze.allocation",
        "solution route allocation size overflows",
    ))?;
    let mut cells = Vec::new();
    reserve(
        &mut cells,
        capacity,
        "solution cell-route allocation failed",
    )?;
    cells.push(end);
    let mut walls = Vec::new();
    reserve(
        &mut walls,
        previous.len(),
        "solution wall-route allocation failed",
    )?;
    while cells.last().copied() != Some(start) {
        work.tick()?;
        let (parent, wall) = previous
            .get(cells.last().expect("route remains nonempty"))
            .copied()
            .ok_or(MazeError::new(
                "maze.solution",
                "tree route is disconnected",
            ))?;
        cells.push(parent);
        walls.push(wall);
    }
    cells.reverse();
    walls.reverse();
    Ok((cells, walls))
}

/// Counts non-fatal tree shape facts after one connected maze and solution are fully derived.
///
/// # Errors
///
/// Returns cancellation or inspection-limit failures before the complete maze result is returned.
fn maze_diagnostics(
    cells: &[MazeCell],
    tree: &MazeTree,
    solution: &MazeSolution,
    work: &mut Work<'_>,
) -> Result<MazeDiagnostics, MazeError> {
    let mut solution_cells = BTreeSet::new();
    for cell in &solution.cells {
        work.tick()?;
        solution_cells.insert(*cell);
    }
    let mut diagnostics = MazeDiagnostics::default();
    for cell in cells {
        work.tick()?;
        let degree = tree.get(&cell.id).map_or(0, BTreeSet::len);
        if !solution_cells.contains(&cell.id) {
            diagnostics.off_solution_cells =
                diagnostics
                    .off_solution_cells
                    .checked_add(1)
                    .ok_or(MazeError::new(
                        "maze.allocation",
                        "maze diagnostic count overflows",
                    ))?;
        }
        if cells.len() > 1 && degree == 1 {
            diagnostics.dead_end_cells =
                diagnostics
                    .dead_end_cells
                    .checked_add(1)
                    .ok_or(MazeError::new(
                        "maze.allocation",
                        "maze diagnostic count overflows",
                    ))?;
        }
        if degree >= 3 {
            diagnostics.branch_cells =
                diagnostics
                    .branch_cells
                    .checked_add(1)
                    .ok_or(MazeError::new(
                        "maze.allocation",
                        "maze diagnostic count overflows",
                    ))?;
        }
    }
    Ok(diagnostics)
}

/// Converts one retained primal wall into a positive open line path using exact family positions.
fn wall_path(
    output_layer_id: PatternOutputLayerId,
    wall: MazeWallId,
    positions: &BTreeMap<MazeVertexId, Point2>,
    bases: &BTreeMap<MazeVertexId, f64>,
) -> Result<MazeWallPath, MazeError> {
    let first = positions[&wall.first];
    let second = positions[&wall.second];
    let path = CurvePath::line(first, second).map_err(|_| {
        MazeError::new(
            "maze.wall_paths",
            "retained wall must have a positive finite line",
        )
    })?;
    Ok(MazeWallPath {
        id: MazeWallPathId {
            output_layer_id,
            wall,
        },
        vertices: [wall.first, wall.second],
        path,
        nominal_basis: bases[&wall.first].min(bases[&wall.second]),
    })
}

/// Returns a canonical cycle rotation independent of the half-edge traversal's start point.
fn canonical_cycle(
    mut vertices: Vec<MazeVertexId>,
    positions: &BTreeMap<MazeVertexId, Point2>,
) -> Vec<MazeVertexId> {
    let minimum = vertices
        .iter()
        .enumerate()
        .min_by_key(|(_, value)| quantized_point(positions[value]))
        .expect("face has vertices")
        .0;
    vertices.rotate_left(minimum);
    vertices
}

/// Orders bounded cells by their quantized geometry so broader cached site IDs cannot alter maze choices.
fn compare_cycles_by_position(
    left: &[MazeVertexId],
    right: &[MazeVertexId],
    positions: &BTreeMap<MazeVertexId, Point2>,
) -> std::cmp::Ordering {
    left.iter()
        .map(|id| quantized_point(positions[id]))
        .cmp(right.iter().map(|id| quantized_point(positions[id])))
}

/// Computes signed polygon area without making a canvas boundary part of the arrangement.
fn polygon_area(vertices: &[MazeVertexId], positions: &BTreeMap<MazeVertexId, Point2>) -> f64 {
    vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
        .map(|(first, second)| {
            let a = positions[&first];
            let b = positions[&second];
            a.x.mul_add(b.y, -a.y * b.x)
        })
        .sum::<f64>()
        / 2.0
}

/// Returns one directed segment angle for deterministic half-edge ordering.
fn angle(first: Point2, second: Point2) -> f64 {
    (second.y - first.y).atan2(second.x - first.x)
}

/// Returns Euclidean distance without changing topology or ordering authority.
fn distance(first: Point2, second: Point2) -> f64 {
    (second.x - first.x).hypot(second.y - first.y)
}

/// Returns a stable seeded ordering key for one dual DFS choice.
fn priority(
    seed: u32,
    first: MazeCellId,
    second: MazeCellId,
    wall: MazeWallId,
    positions: &BTreeMap<MazeVertexId, Point2>,
) -> u64 {
    let mut value =
        u64::from(seed) ^ u64::from(first.0).rotate_left(13) ^ u64::from(second.0).rotate_left(29);
    let first_point = quantized_point(positions[&wall.first]);
    let second_point = quantized_point(positions[&wall.second]);
    value ^= (first_point.0 as u64).rotate_left(7) ^ (first_point.1 as u64).rotate_left(19);
    value ^= (second_point.0 as u64).rotate_left(31) ^ (second_point.1 as u64).rotate_left(43);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value.wrapping_mul(0x94d0_49bb_1331_11eb) ^ (value >> 31)
}

/// Converts one finite geometry point into the cache-envelope-stable priority lattice.
fn quantized_point(point: Point2) -> (i64, i64) {
    (
        (point.x * 1e9).round() as i64,
        (point.y * 1e9).round() as i64,
    )
}

/// Converts one finite scalar into the envelope-stable maze identity lattice.
fn quantized_scalar(value: f64) -> i64 {
    (value * 1e9).round() as i64
}

/// Streams all structural maze facts into one stable limit-independent fingerprint.
#[allow(clippy::too_many_arguments)] // Every structural collection is deliberately explicit in the stable maze identity.
fn fingerprint(
    family: &str,
    program: &MazeProgram,
    source_sites: &[MazeSourceSite],
    source_walls: &[MazeWallId],
    cells: &[MazeCell],
    dual_edges: &[MazeDualEdge],
    passages: &[MazeWallId],
    retained: &[MazeWall],
    paths: &[MazeWallPath],
    solution: &MazeSolution,
    openings: (MazeOpening, MazeOpening),
    work: &mut Work<'_>,
) -> Result<String, MazeError> {
    let mut hasher = MazeHasher::new();
    hasher.bytes(MAZE_WALL_CONTRACT_ID.as_bytes());
    hasher.bytes(family.as_bytes());
    hasher.byte(1);
    hasher.bytes(&program.seed.to_le_bytes());
    for site in source_sites {
        work.tick()?;
        append_site(&mut hasher, site.id);
        let (x, y) = quantized_point(site.source.position);
        hasher.bytes(&x.to_le_bytes());
        hasher.bytes(&y.to_le_bytes());
        hasher.bytes(&quantized_scalar(site.source.nominal_cell_basis.diameter()).to_le_bytes());
        let FamilySiteProvenance::GuideIntersection { contributors } = &site.source.provenance
        else {
            unreachable!("maze source-site validation preserves guide intersections");
        };
        hasher.bytes(&(contributors.len() as u64).to_le_bytes());
        for contributor in contributors {
            work.tick()?;
            hasher.bytes(&contributor.dimension_id.to_le_bytes());
            hasher.bytes(&contributor.index.to_le_bytes());
            hasher.bytes(&contributor.component_ordinal.to_le_bytes());
        }
    }
    for wall in source_walls
        .iter()
        .chain(passages)
        .chain(retained.iter().map(|wall| &wall.id))
    {
        work.tick()?;
        append_wall(&mut hasher, *wall);
    }
    for cell in cells {
        work.tick()?;
        hasher.bytes(&cell.id.0.to_le_bytes());
        for vertex in &cell.vertices {
            work.tick()?;
            append_site(&mut hasher, *vertex);
        }
    }
    for edge in dual_edges {
        work.tick()?;
        hasher.bytes(&edge.id.0.to_le_bytes());
        hasher.bytes(&edge.first.0.to_le_bytes());
        hasher.bytes(&edge.second.0.to_le_bytes());
        append_wall(&mut hasher, edge.shared_wall);
    }
    for path in paths {
        work.tick()?;
        append_wall(&mut hasher, path.id.wall);
        hasher.bytes(&quantized_scalar(path.nominal_basis).to_le_bytes());
    }
    for opening in [openings.0, openings.1] {
        work.tick()?;
        append_wall(&mut hasher, opening.wall);
        hasher.bytes(&opening.cell.0.to_le_bytes());
        let (x, y) = quantized_point(opening.point);
        hasher.bytes(&x.to_le_bytes());
        hasher.bytes(&y.to_le_bytes());
        hasher.byte(opening.side as u8);
    }
    work.tick()?;
    hasher.bytes(&solution.entrance.0.to_le_bytes());
    hasher.bytes(&solution.exit.0.to_le_bytes());
    for cell in &solution.cells {
        work.tick()?;
        hasher.bytes(&cell.0.to_le_bytes());
    }
    for wall in &solution.passage_walls {
        work.tick()?;
        append_wall(&mut hasher, *wall);
    }
    hasher.finish()
}

/// Appends one stable site identity to a streaming maze fingerprint.
fn append_site(hasher: &mut MazeHasher, site: MazeVertexId) {
    hasher.bytes(&site.0.to_le_bytes());
}

/// Appends one canonical wall identity to a streaming maze fingerprint.
fn append_wall(hasher: &mut MazeHasher, wall: MazeWallId) {
    append_site(hasher, wall.first);
    append_site(hasher, wall.second);
}

/// Streams the fixed maze FNV-1a encoding without a material byte buffer.
struct MazeHasher(u64);

impl MazeHasher {
    /// Starts the fixed maze identity hash.
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325_u64)
    }

    /// Incorporates one stable identity byte.
    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(0x100_0000_01b3);
    }

    /// Incorporates one fixed identity byte sequence.
    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.byte(*byte);
        }
    }

    /// Finishes the fixed legacy-compatible hexadecimal maze identity with a fallible string allocation.
    ///
    /// # Errors
    ///
    /// Returns `maze.allocation` when the fixed result string cannot reserve its exact capacity.
    fn finish(self) -> Result<String, MazeError> {
        use std::fmt::Write;

        let mut value = String::new();
        value
            .try_reserve_exact(24)
            .map_err(|_| MazeError::new("maze.allocation", "maze fingerprint allocation failed"))?;
        write!(&mut value, "fnv1a64:{:016x}", self.0)
            .expect("reserved maze fingerprint formatting is infallible");
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises the fallible maze vector reservation seam without replacing the process allocator.
    #[test]
    fn reservation_capacity_overflow_reports_maze_allocation() {
        let mut values = Vec::<MazeWall>::new();
        let error = reserve(&mut values, usize::MAX, "test allocation failure")
            .expect_err("an impossible Vec reservation fails deterministically");
        assert_eq!(error.path(), "maze.allocation");
    }
}
