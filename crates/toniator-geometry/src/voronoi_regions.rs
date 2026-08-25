//! Geometry-owned ordinary Voronoi realization with no canvas-created topology.

use std::{error::Error, fmt};

use spade::{DelaunayTriangulation, HasPosition, Point2 as SpadePoint, Triangulation};
use toniator_domain::PatternOutputLayerId;

use crate::{
    Bounds, CanonicalRegionLimits, CanonicalRegionProposal, CanonicalRegionSet,
    CanonicalRegionSourceGroup, CanonicalRegionSourceId, CurvePath, FamilySiteId, FamilySiteSet,
    PathClosure, Point2, build_canonical_regions_cancellable,
};

/// Fixed private-topology contract included in Stage 20O identities and cache keys.
pub const VORONOI_REGION_CONTRACT_ID: &str = "toniator.voronoi.spade-2.15.1.v1";
/// Default maximum exact-coordinate source groups.
pub const DEFAULT_MAX_VORONOI_SITE_GROUPS: usize = 1_048_576;
/// Default maximum undirected topology edges.
pub const DEFAULT_MAX_VORONOI_TOPOLOGY_EDGES: usize = 4_194_304;
/// Default maximum retained finite regions.
pub const DEFAULT_MAX_VORONOI_REGIONS: usize = 1_048_576;
/// Default maximum retained finite boundary points.
pub const DEFAULT_MAX_VORONOI_BOUNDARY_POINTS: usize = 8_388_608;
/// Default maximum topology and canonicalization inspections.
pub const DEFAULT_MAX_VORONOI_INSPECTIONS: usize = 67_108_864;

/// Request owned by the ordinary-region producer, never by Spade or a renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoronoiRegionRequest {
    /// Output identity propagated into every canonical region.
    pub output_layer_id: PatternOutputLayerId,
    /// Final canvas relevance bounds; these do not create topology.
    pub canvas: Bounds,
}

/// Bounded deterministic work inputs for one complete ordinary-region build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoronoiRegionLimits {
    max_site_groups: usize,
    max_topology_edges: usize,
    max_regions: usize,
    max_boundary_points: usize,
    max_inspections: usize,
}

/// Derived facts deliberately excluded from canonical region fingerprints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VoronoiRegionDiagnostics {
    /// Exact normalized coordinate groups accepted for insertion.
    pub site_groups: usize,
    /// Groups containing more than one co-owner.
    pub duplicate_groups: usize,
    /// Insertions avoided by exact duplicate grouping.
    pub avoided_insertions: usize,
    /// Unique Delaunay topology edges inspected.
    pub topology_edges: usize,
    /// Retained finite canonical regions.
    pub regions: usize,
    /// Retained finite boundary points.
    pub boundary_points: usize,
    /// All deterministic bounded-work inspections.
    pub inspections: usize,
}

/// Stable ordinary-region failure that never exposes partial topology or regions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoronoiRegionError {
    path: &'static str,
    message: &'static str,
}

#[derive(Clone, Debug)]
struct GroupedSite {
    point: Point2,
    owners: Vec<FamilySiteId>,
}

#[derive(Clone, Debug)]
struct SpadeSite {
    point: SpadePoint<f64>,
    group_index: usize,
}

impl HasPosition for SpadeSite {
    type Scalar = f64;

    /// Returns the sole private topology coordinate without exposing a Spade value outside geometry.
    fn position(&self) -> SpadePoint<Self::Scalar> {
        self.point
    }
}

impl VoronoiRegionLimits {
    /// Constructs nonzero complete-build limits.
    ///
    /// # Errors
    ///
    /// Returns `region.voronoi.limits.zero` when any configured work bound is zero.
    pub fn new(
        max_site_groups: usize,
        max_topology_edges: usize,
        max_regions: usize,
        max_boundary_points: usize,
        max_inspections: usize,
    ) -> Result<Self, VoronoiRegionError> {
        if [
            max_site_groups,
            max_topology_edges,
            max_regions,
            max_boundary_points,
            max_inspections,
        ]
        .into_iter()
        .any(|value| value == 0)
        {
            return Err(VoronoiRegionError::new(
                "region.voronoi.limits.zero",
                "ordinary Voronoi limits must be nonzero",
            ));
        }
        Ok(Self {
            max_site_groups,
            max_topology_edges,
            max_regions,
            max_boundary_points,
            max_inspections,
        })
    }

    /// Returns the exact-coordinate group bound.
    pub const fn max_site_groups(self) -> usize {
        self.max_site_groups
    }
    /// Returns the unique topology-edge bound.
    pub const fn max_topology_edges(self) -> usize {
        self.max_topology_edges
    }
    /// Returns the retained region bound.
    pub const fn max_regions(self) -> usize {
        self.max_regions
    }
    /// Returns the retained boundary-point bound.
    pub const fn max_boundary_points(self) -> usize {
        self.max_boundary_points
    }
    /// Returns the deterministic inspection bound.
    pub const fn max_inspections(self) -> usize {
        self.max_inspections
    }
}

impl Default for VoronoiRegionLimits {
    /// Supplies the accepted Stage 20O nonzero limits.
    fn default() -> Self {
        Self {
            max_site_groups: DEFAULT_MAX_VORONOI_SITE_GROUPS,
            max_topology_edges: DEFAULT_MAX_VORONOI_TOPOLOGY_EDGES,
            max_regions: DEFAULT_MAX_VORONOI_REGIONS,
            max_boundary_points: DEFAULT_MAX_VORONOI_BOUNDARY_POINTS,
            max_inspections: DEFAULT_MAX_VORONOI_INSPECTIONS,
        }
    }
}

impl VoronoiRegionError {
    /// Creates one stable ordinary-region diagnostic without partial producer state.
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }
    /// Returns the stable producer-owned diagnostic path.
    pub const fn path(&self) -> &'static str {
        self.path
    }
    /// Returns the stable human-readable diagnostic message.
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for VoronoiRegionError {
    /// Formats the stable ordinary-region diagnostic.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}
impl Error for VoronoiRegionError {}

/// Builds complete ordinary Voronoi regions from a complete reusable family site set.
///
/// Exact duplicate coordinates co-own one cell after signed-zero normalization. The canvas is
/// consulted only to retain finite cells or reject a relevant unbounded cell; it never closes a
/// topology edge. No partially built regions are returned on cancellation or any failure.
///
/// # Errors
///
/// Returns `evaluation.cancelled` for cancellation and stable `region.voronoi.*` paths otherwise.
pub fn build_voronoi_regions_cancellable(
    family: &FamilySiteSet,
    request: VoronoiRegionRequest,
    limits: VoronoiRegionLimits,
    cancelled: impl Fn() -> bool,
) -> Result<(CanonicalRegionSet, VoronoiRegionDiagnostics), VoronoiRegionError> {
    let mut diagnostics = VoronoiRegionDiagnostics::default();
    let groups = group_sites(family, limits, &cancelled, &mut diagnostics)?;
    if groups.len() < 3 {
        return Err(VoronoiRegionError::new(
            "region.voronoi.coverage.unbounded",
            "ordinary Voronoi requires three non-collinear site groups covering the canvas",
        ));
    }
    let mut triangulation: DelaunayTriangulation<SpadeSite> = DelaunayTriangulation::new();
    for (group_index, group) in groups.iter().enumerate() {
        poll(&cancelled)?;
        inspect(&mut diagnostics, limits)?;
        triangulation
            .insert(SpadeSite {
                point: SpadePoint::new(group.point.x, group.point.y),
                group_index,
            })
            .map_err(|_| {
                VoronoiRegionError::new(
                    "region.voronoi.insertion.coordinates",
                    "Voronoi insertion rejected a finite site coordinate",
                )
            })?;
        poll(&cancelled)?;
    }
    if triangulation.num_inner_faces() == 0 {
        return Err(VoronoiRegionError::new(
            "region.voronoi.coverage.unbounded",
            "ordinary Voronoi has no bounded cells for the requested canvas",
        ));
    }
    let edge_count = triangulation.num_undirected_edges();
    if edge_count > limits.max_topology_edges {
        return Err(VoronoiRegionError::new(
            "region.voronoi.limits.topology_edges",
            "ordinary Voronoi topology edge limit exceeded",
        ));
    }
    diagnostics.topology_edges = edge_count;
    let mut sources = Vec::new();
    for vertex in triangulation.vertices() {
        poll(&cancelled)?;
        inspect(&mut diagnostics, limits)?;
        let group = &groups[vertex.data().group_index];
        let mut vertices = Vec::new();
        let mut unbounded = false;
        let mut boundary_intersects_canvas = false;
        for edge in vertex.as_voronoi_face().adjacent_edges() {
            poll(&cancelled)?;
            inspect(&mut diagnostics, limits)?;
            let from = edge.from().position();
            let to = edge.to().position();
            match (from, to) {
                (Some(start), Some(end))
                    if start.x.is_finite()
                        && start.y.is_finite()
                        && end.x.is_finite()
                        && end.y.is_finite() =>
                {
                    let start = Point2::new(start.x, start.y);
                    let end = Point2::new(end.x, end.y);
                    vertices.push(start);
                    boundary_intersects_canvas |=
                        segment_intersects_bounds(start, end, request.canvas);
                }
                (Some(anchor), None) if anchor.x.is_finite() && anchor.y.is_finite() => {
                    unbounded = true;
                    let direction = edge.rev().direction_vector();
                    boundary_intersects_canvas |= ray_intersects_bounds(
                        Point2::new(anchor.x, anchor.y),
                        Point2::new(direction.x, direction.y),
                        request.canvas,
                    );
                }
                (None, Some(anchor)) if anchor.x.is_finite() && anchor.y.is_finite() => {
                    unbounded = true;
                    let direction = edge.direction_vector();
                    boundary_intersects_canvas |= ray_intersects_bounds(
                        Point2::new(anchor.x, anchor.y),
                        Point2::new(direction.x, direction.y),
                        request.canvas,
                    );
                }
                _ => unbounded = true,
            }
        }
        if unbounded {
            let owns_canvas_corner = request.canvas.corners().into_iter().any(|corner| {
                triangulation
                    .nearest_neighbor(SpadePoint::new(corner.x, corner.y))
                    .is_some_and(|nearest| nearest.data().group_index == vertex.data().group_index)
            });
            if request.canvas.contains(group.point)
                || owns_canvas_corner
                || boundary_intersects_canvas
            {
                return Err(VoronoiRegionError::new(
                    "region.voronoi.coverage.unbounded",
                    "a canvas-relevant ordinary Voronoi cell is unbounded",
                ));
            }
            continue;
        }
        vertices.dedup();
        if vertices.len() > 1 && vertices.first() == vertices.last() {
            vertices.pop();
        }
        if vertices.len() < 3 {
            continue;
        }
        let bounds =
            Bounds::from_points(vertices.iter().copied()).ok_or(VoronoiRegionError::new(
                "region.voronoi.geometry.bounds",
                "finite Voronoi vertices must form finite bounds",
            ))?;
        if !finite_cell_relevant(&vertices, bounds, request.canvas) {
            continue;
        }
        let path = CurvePath::polyline(vertices, PathClosure::Closed).map_err(|_| {
            VoronoiRegionError::new(
                "region.voronoi.geometry.ring",
                "finite Voronoi cell could not form a closed line ring",
            )
        })?;
        diagnostics.regions = diagnostics
            .regions
            .checked_add(1)
            .ok_or(VoronoiRegionError::new(
                "region.voronoi.allocation.regions",
                "ordinary Voronoi region count overflowed",
            ))?;
        diagnostics.boundary_points = diagnostics
            .boundary_points
            .checked_add(path.segments().len())
            .ok_or(VoronoiRegionError::new(
                "region.voronoi.allocation.boundary_points",
                "ordinary Voronoi boundary count overflowed",
            ))?;
        if diagnostics.regions > limits.max_regions {
            return Err(VoronoiRegionError::new(
                "region.voronoi.limits.regions",
                "ordinary Voronoi retained region limit exceeded",
            ));
        }
        if diagnostics.boundary_points > limits.max_boundary_points {
            return Err(VoronoiRegionError::new(
                "region.voronoi.limits.boundary_points",
                "ordinary Voronoi retained boundary point limit exceeded",
            ));
        }
        sources.push(CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(group.owners.clone()),
            components: vec![path],
        });
    }
    if sources.is_empty() {
        return Err(VoronoiRegionError::new(
            "region.voronoi.coverage.empty",
            "ordinary Voronoi retained no finite canvas-relevant cells",
        ));
    }
    let remaining_inspections = limits
        .max_inspections
        .checked_sub(diagnostics.inspections)
        .ok_or(VoronoiRegionError::new(
            "region.voronoi.limits.inspections",
            "ordinary Voronoi inspection limit exceeded",
        ))?;
    if remaining_inspections == 0 {
        return Err(VoronoiRegionError::new(
            "region.voronoi.limits.inspections",
            "ordinary Voronoi inspection limit exhausted before canonicalization",
        ));
    }
    let canonical_limits = CanonicalRegionLimits::new(
        limits.max_site_groups,
        limits.max_regions,
        limits.max_boundary_points,
        remaining_inspections,
    )
    .map_err(|_| {
        VoronoiRegionError::new(
            "region.voronoi.limits.canonical",
            "ordinary Voronoi limits cannot configure canonical regions",
        )
    })?;
    let (regions, canonical_diagnostics) = build_canonical_regions_cancellable(
        CanonicalRegionProposal {
            output_layer_id: request.output_layer_id,
            source_groups: sources,
        },
        canonical_limits,
        cancelled,
    )
    .map_err(|error| VoronoiRegionError::new(error.path(), error.message()))?;
    diagnostics.inspections = diagnostics
        .inspections
        .checked_add(canonical_diagnostics.inspections)
        .ok_or(VoronoiRegionError::new(
            "region.voronoi.allocation.inspections",
            "ordinary Voronoi inspection count overflowed",
        ))?;
    Ok((regions, diagnostics))
}

/// Groups exact normalized coordinates without changing source-site identities.
fn group_sites(
    family: &FamilySiteSet,
    limits: VoronoiRegionLimits,
    cancelled: &dyn Fn() -> bool,
    diagnostics: &mut VoronoiRegionDiagnostics,
) -> Result<Vec<GroupedSite>, VoronoiRegionError> {
    let mut sites = family
        .sites()
        .iter()
        .map(|site| {
            (
                Point2::new(
                    normalize_zero(site.position.x),
                    normalize_zero(site.position.y),
                ),
                site.id,
            )
        })
        .collect::<Vec<_>>();
    sites.sort_by(|(left, left_id), (right, right_id)| {
        left.x
            .total_cmp(&right.x)
            .then_with(|| left.y.total_cmp(&right.y))
            .then_with(|| left_id.cmp(right_id))
    });
    let mut groups: Vec<GroupedSite> = Vec::new();
    for (point, id) in sites {
        poll(cancelled)?;
        inspect(diagnostics, limits)?;
        match groups.last_mut() {
            Some(group) if group.point == point => group.owners.push(id),
            _ => {
                if groups.len() == limits.max_site_groups {
                    return Err(VoronoiRegionError::new(
                        "region.voronoi.limits.site_groups",
                        "ordinary Voronoi source-site group limit exceeded",
                    ));
                }
                groups.push(GroupedSite {
                    point,
                    owners: vec![id],
                });
            }
        }
    }
    diagnostics.site_groups = groups.len();
    diagnostics.duplicate_groups = groups.iter().filter(|group| group.owners.len() > 1).count();
    diagnostics.avoided_insertions = family.sites().len().saturating_sub(groups.len());
    Ok(groups)
}

/// Normalizes signed zero before exact-coordinate grouping and identity encoding.
fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}
/// Counts one bounded deterministic inspection and checks cancellation-owned work limits.
fn inspect(
    diagnostics: &mut VoronoiRegionDiagnostics,
    limits: VoronoiRegionLimits,
) -> Result<(), VoronoiRegionError> {
    diagnostics.inspections =
        diagnostics
            .inspections
            .checked_add(1)
            .ok_or(VoronoiRegionError::new(
                "region.voronoi.allocation.inspections",
                "ordinary Voronoi inspection count overflowed",
            ))?;
    if diagnostics.inspections > limits.max_inspections {
        Err(VoronoiRegionError::new(
            "region.voronoi.limits.inspections",
            "ordinary Voronoi inspection limit exceeded",
        ))
    } else {
        Ok(())
    }
}
/// Returns cancellation before any further mutable topology or output allocation occurs.
fn poll(cancelled: &dyn Fn() -> bool) -> Result<(), VoronoiRegionError> {
    if cancelled() {
        Err(VoronoiRegionError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ))
    } else {
        Ok(())
    }
}
/// Tests exact finite polygon/rectangle relevance without manufacturing a topology edge.
fn finite_cell_relevant(vertices: &[Point2], bounds: Bounds, canvas: Bounds) -> bool {
    bounds_overlap(bounds, canvas)
        && (vertices.iter().any(|point| canvas.contains(*point))
            || polygon_contains_any(vertices, canvas.corners())
            || vertices
                .iter()
                .zip(vertices.iter().cycle().skip(1))
                .take(vertices.len())
                .any(|(left, right)| segment_intersects_bounds(*left, *right, canvas)))
}
/// Tests conservative finite rectangle overlap before exact edge or containment testing.
fn bounds_overlap(left: Bounds, right: Bounds) -> bool {
    left.min.x <= right.max.x
        && right.min.x <= left.max.x
        && left.min.y <= right.max.y
        && right.min.y <= left.max.y
}
/// Tests one finite segment against the final canvas rectangle without deriving a new segment.
fn segment_intersects_bounds(start: Point2, end: Point2, bounds: Bounds) -> bool {
    if bounds.contains(start) || bounds.contains(end) {
        return true;
    }
    let corners = bounds.corners();
    corners
        .iter()
        .zip(corners.iter().cycle().skip(1))
        .take(4)
        .any(|(left, right)| segments_intersect(start, end, *left, *right))
}
/// Tests closed finite line-segment intersection including boundary contact.
fn segments_intersect(a: Point2, b: Point2, c: Point2, d: Point2) -> bool {
    fn cross(a: Point2, b: Point2, c: Point2) -> f64 {
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    }
    let ab_c = cross(a, b, c);
    let ab_d = cross(a, b, d);
    let cd_a = cross(c, d, a);
    let cd_b = cross(c, d, b);
    (ab_c == 0.0 && point_on_segment(c, a, b))
        || (ab_d == 0.0 && point_on_segment(d, a, b))
        || (cd_a == 0.0 && point_on_segment(a, c, d))
        || (cd_b == 0.0 && point_on_segment(b, c, d))
        || ((ab_c > 0.0) != (ab_d > 0.0) && (cd_a > 0.0) != (cd_b > 0.0))
}
/// Tests finite collinear containment used by the exact segment intersection predicate.
fn point_on_segment(point: Point2, start: Point2, end: Point2) -> bool {
    point.x >= start.x.min(end.x)
        && point.x <= start.x.max(end.x)
        && point.y >= start.y.min(end.y)
        && point.y <= start.y.max(end.y)
}
/// Tests a half-open outer Voronoi ray against the canvas without clipping or changing topology.
fn ray_intersects_bounds(origin: Point2, direction: Point2, bounds: Bounds) -> bool {
    if bounds.contains(origin) {
        return true;
    }
    if !direction.x.is_finite()
        || !direction.y.is_finite()
        || (direction.x == 0.0 && direction.y == 0.0)
    {
        return false;
    }
    let corners = bounds.corners();
    corners
        .iter()
        .zip(corners.iter().cycle().skip(1))
        .take(4)
        .any(|(start, end)| ray_intersects_segment(origin, direction, *start, *end))
}
/// Tests an infinite-direction ray and a finite segment including endpoint contact.
fn ray_intersects_segment(origin: Point2, direction: Point2, start: Point2, end: Point2) -> bool {
    let segment = Point2::new(end.x - start.x, end.y - start.y);
    let denominator = direction.x * -segment.y - direction.y * -segment.x;
    let offset = Point2::new(start.x - origin.x, start.y - origin.y);
    if denominator == 0.0 {
        return (offset.x * direction.y - offset.y * direction.x) == 0.0
            && [start, end].into_iter().any(|point| {
                let projection =
                    (point.x - origin.x) * direction.x + (point.y - origin.y) * direction.y;
                projection >= 0.0
            });
    }
    let ray_t = (offset.x * -segment.y - offset.y * -segment.x) / denominator;
    let segment_t = (direction.x * offset.y - direction.y * offset.x) / denominator;
    ray_t >= 0.0 && (0.0..=1.0).contains(&segment_t)
}
/// Tests whether one canvas corner lies inside a finite proposal ring for relevance classification.
fn polygon_contains_any(vertices: &[Point2], candidates: [Point2; 4]) -> bool {
    candidates.into_iter().any(|point| {
        let mut inside = false;
        for (left, right) in vertices
            .iter()
            .zip(vertices.iter().cycle().skip(1))
            .take(vertices.len())
        {
            if (left.y > point.y) != (right.y > point.y)
                && point.x < (right.x - left.x) * (point.y - left.y) / (right.y - left.y) + left.x
            {
                inside = !inside;
            }
        }
        inside
    })
}
