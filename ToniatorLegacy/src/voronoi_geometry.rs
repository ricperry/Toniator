//! Pure clipped Voronoi geometry over neutral ordered points.
//!
//! The only input is a finite domain and ordered point set. This module does
//! not know why points were placed or how its polygons will eventually render.

use crate::{CancellationToken, DomainBounds, OrderedPoint};
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

const POLYGON_EPSILON: f64 = 1.0e-7;
const SITE_SEPARATION_EPSILON: f64 = 1.0e-4;
const BOUNDARY_QUANTUM: f64 = 1.0e-4;

/// Centralized construction limits for pure geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometryLimits {
    pub max_sites: usize,
}

impl Default for GeometryLimits {
    fn default() -> Self {
        Self { max_sites: 8_192 }
    }
}

/// The clipped polygon owned by one ordered input site.
#[derive(Debug, Clone, PartialEq)]
pub struct ClippedVoronoiCell {
    pub site_index: usize,
    pub vertices: Vec<OrderedPoint>,
}

/// A deduplicated segment in the generated diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct VoronoiBoundary {
    pub start: OrderedPoint,
    pub end: OrderedPoint,
    pub kind: VoronoiBoundaryKind,
}

/// Artboard segments are clipping limits; only interior segments are shared by
/// cells and eligible for later visual seam treatment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoronoiBoundaryKind {
    Artboard,
    Interior {
        first_cell: usize,
        second_cell: usize,
    },
}

/// Complete pure geometry, preserving both types of boundaries explicitly.
#[derive(Debug, Clone, PartialEq)]
pub struct VoronoiDiagram {
    pub domain: DomainBounds,
    pub cells: Vec<ClippedVoronoiCell>,
    pub boundaries: Vec<VoronoiBoundary>,
}

pub fn build_voronoi_diagram(
    domain: DomainBounds,
    points: &[OrderedPoint],
) -> Result<VoronoiDiagram> {
    build_voronoi_diagram_cancellable(
        domain,
        points,
        GeometryLimits::default(),
        &CancellationToken::new(),
    )
}

/// Builds ordinary Voronoi cells by exact half-plane clipping.
///
/// A bounded spatial index avoids applying every site to every cell while the
/// expanding radius proves when all potentially constraining sites were seen.
pub fn build_voronoi_diagram_cancellable(
    domain: DomainBounds,
    points: &[OrderedPoint],
    limits: GeometryLimits,
    token: &CancellationToken,
) -> Result<VoronoiDiagram> {
    token.checkpoint()?;
    validate_points(domain, points, limits)?;
    let cells = build_cells(domain, points, token)?;
    let boundaries = collect_boundaries(domain, &cells, token)?;
    token.checkpoint()?;
    Ok(VoronoiDiagram {
        domain,
        cells,
        boundaries,
    })
}

/// Insets a cell only from its actual interior supporting edges. Artboard
/// edges are deliberately retained, so canvas clipping never becomes a seam.
pub fn inset_clipped_cell(
    domain: DomainBounds,
    cell: &ClippedVoronoiCell,
    inset: f64,
) -> Result<Vec<OrderedPoint>> {
    domain.validate()?;
    ensure!(
        inset >= 0.0 && inset.is_finite(),
        "Voronoi inset must be finite and non-negative"
    );
    ensure!(cell.vertices.len() >= 3, "Voronoi cell has no polygon");
    let orientation = signed_area(&cell.vertices).signum();
    ensure!(
        orientation.abs() > POLYGON_EPSILON,
        "Voronoi cell is degenerate"
    );
    let mut polygon = artboard_polygon(domain);
    for (start, end) in edges(&cell.vertices) {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length = (dx * dx + dy * dy).sqrt();
        if length <= POLYGON_EPSILON || lies_on_artboard(start, end, domain) {
            continue;
        }
        let (normal_x, normal_y) = if orientation >= 0.0 {
            (-dy / length, dx / length)
        } else {
            (dy / length, -dx / length)
        };
        let threshold = start.x * normal_x + start.y * normal_y + inset;
        polygon = clip_to_supporting_half_plane(polygon, normal_x, normal_y, threshold);
        ensure!(polygon.len() >= 3, "Voronoi inset collapsed a cell");
    }
    ensure!(
        signed_area(&polygon).abs() > POLYGON_EPSILON,
        "Voronoi inset collapsed a cell"
    );
    Ok(polygon)
}

/// Applies a response-derived inset without exposing clipping internals to a
/// caller. The supplied site only bounds how far an interior support may move;
/// artboard supports remain unchanged.
pub fn inset_clipped_cell_for_response(
    domain: DomainBounds,
    cell: &ClippedVoronoiCell,
    site: OrderedPoint,
    rendered_fraction: f64,
    boundary_gap: f64,
) -> Result<Vec<OrderedPoint>> {
    domain.validate()?;
    ensure!(
        rendered_fraction.is_finite() && (0.0..=1.0).contains(&rendered_fraction),
        "Voronoi response fraction must be normalized"
    );
    ensure!(
        boundary_gap.is_finite() && boundary_gap >= 0.0,
        "Voronoi boundary gap must be finite and non-negative"
    );
    ensure!(cell.vertices.len() >= 3, "Voronoi cell has no polygon");
    let orientation = signed_area(&cell.vertices).signum();
    ensure!(
        orientation.abs() > POLYGON_EPSILON,
        "Voronoi cell is degenerate"
    );
    let mut polygon = artboard_polygon(domain);
    for (start, end) in edges(&cell.vertices) {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length = (dx * dx + dy * dy).sqrt();
        if length <= POLYGON_EPSILON {
            continue;
        }
        let (normal_x, normal_y) = if orientation >= 0.0 {
            (-dy / length, dx / length)
        } else {
            (dy / length, -dx / length)
        };
        let raw_threshold = start.x * normal_x + start.y * normal_y;
        let inset = if lies_on_artboard(start, end, domain) {
            0.0
        } else {
            let site_distance = (site.x * normal_x + site.y * normal_y - raw_threshold).abs();
            (boundary_gap * 0.5 + (1.0 - rendered_fraction) * site_distance * 0.5)
                .min(site_distance * 0.49)
        };
        polygon = clip_to_supporting_half_plane(polygon, normal_x, normal_y, raw_threshold + inset);
        ensure!(
            polygon.len() >= 3,
            "Voronoi response inset collapsed a cell"
        );
    }
    ensure!(
        signed_area(&polygon).abs() > POLYGON_EPSILON,
        "Voronoi response inset collapsed a cell"
    );
    Ok(polygon)
}

fn validate_points(
    domain: DomainBounds,
    points: &[OrderedPoint],
    limits: GeometryLimits,
) -> Result<()> {
    domain.validate()?;
    ensure!(
        points.len() >= 2,
        "Voronoi geometry requires at least two sites"
    );
    ensure!(
        points.len() <= limits.max_sites,
        "Voronoi geometry exceeds the {} site limit",
        limits.max_sites
    );
    for (index, point) in points.iter().copied().enumerate() {
        ensure!(
            point.x.is_finite()
                && point.y.is_finite()
                && (0.0..=f64::from(domain.width)).contains(&point.x)
                && (0.0..=f64::from(domain.height)).contains(&point.y),
            "Voronoi site {index} must be finite and inside the domain"
        );
        for previous in &points[..index] {
            ensure!(
                squared_distance(point, *previous)
                    > SITE_SEPARATION_EPSILON * SITE_SEPARATION_EPSILON,
                "Voronoi sites must not coincide or be near-coincident"
            );
        }
    }
    Ok(())
}

fn build_cells(
    domain: DomainBounds,
    points: &[OrderedPoint],
    token: &CancellationToken,
) -> Result<Vec<ClippedVoronoiCell>> {
    let count = points.len();
    let grid_side = (count as f64).sqrt().ceil().max(1.0) as usize;
    let mut buckets = vec![Vec::<usize>::new(); grid_side * grid_side];
    let width = f64::from(domain.width);
    let height = f64::from(domain.height);
    let bucket_for = |point: OrderedPoint| -> (usize, usize) {
        (
            ((point.x / width) * grid_side as f64)
                .floor()
                .clamp(0.0, (grid_side - 1) as f64) as usize,
            ((point.y / height) * grid_side as f64)
                .floor()
                .clamp(0.0, (grid_side - 1) as f64) as usize,
        )
    };
    for (index, point) in points.iter().copied().enumerate() {
        if index % 256 == 0 {
            token.checkpoint()?;
        }
        let (x, y) = bucket_for(point);
        buckets[y * grid_side + x].push(index);
    }

    let diagonal = (width * width + height * height).sqrt();
    let mut cells = Vec::with_capacity(count);
    let mut seen = vec![0u32; count];
    for (site_index, site) in points.iter().copied().enumerate() {
        if site_index % 8 == 0 {
            token.checkpoint()?;
        }
        let generation = site_index as u32 + 1;
        let mut polygon = artboard_polygon(domain);
        let mut radius = (width.max(height) / grid_side as f64).max(1.0);
        loop {
            let radius_squared = radius * radius;
            let (bucket_x, bucket_y) = bucket_for(site);
            let span_x = (radius / width * grid_side as f64).ceil() as isize;
            let span_y = (radius / height * grid_side as f64).ceil() as isize;
            let min_x = (bucket_x as isize - span_x).max(0) as usize;
            let max_x = (bucket_x as isize + span_x).min(grid_side as isize - 1) as usize;
            let min_y = (bucket_y as isize - span_y).max(0) as usize;
            let max_y = (bucket_y as isize + span_y).min(grid_side as isize - 1) as usize;
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    for &other_index in &buckets[y * grid_side + x] {
                        if other_index == site_index || seen[other_index] == generation {
                            continue;
                        }
                        let other = points[other_index];
                        if squared_distance(site, other) > radius_squared {
                            continue;
                        }
                        seen[other_index] = generation;
                        polygon = clip_to_nearest_site(polygon, site, other);
                        if polygon.len() < 3 {
                            break;
                        }
                    }
                    if polygon.len() < 3 {
                        break;
                    }
                }
                if polygon.len() < 3 {
                    break;
                }
            }
            ensure!(
                polygon.len() >= 3,
                "Voronoi site {site_index} produced a degenerate clipped cell"
            );
            let circumradius = polygon
                .iter()
                .map(|point| squared_distance(*point, site).sqrt())
                .fold(0.0, f64::max);
            if radius >= 2.0 * circumradius + POLYGON_EPSILON || radius >= 2.0 * diagonal {
                break;
            }
            radius = (radius * 2.0).min(2.0 * diagonal);
        }
        ensure!(
            signed_area(&polygon).abs() > POLYGON_EPSILON,
            "Voronoi site {site_index} produced a degenerate clipped cell"
        );
        cells.push(ClippedVoronoiCell {
            site_index,
            vertices: polygon,
        });
    }
    Ok(cells)
}

fn collect_boundaries(
    domain: DomainBounds,
    cells: &[ClippedVoronoiCell],
    token: &CancellationToken,
) -> Result<Vec<VoronoiBoundary>> {
    let mut segments: BTreeMap<SegmentKey, Vec<usize>> = BTreeMap::new();
    for cell in cells {
        if cell.site_index % 16 == 0 {
            token.checkpoint()?;
        }
        for (start, end) in edges(&cell.vertices) {
            let key = segment_key(start, end);
            segments.entry(key).or_default().push(cell.site_index);
        }
    }
    let mut boundaries = Vec::with_capacity(segments.len());
    for (key, owners) in segments {
        token.checkpoint()?;
        let start = point_from_key(key.start, domain);
        let end = point_from_key(key.end, domain);
        let kind = if lies_on_artboard(start, end, domain) {
            VoronoiBoundaryKind::Artboard
        } else {
            ensure!(
                owners.len() == 2,
                "Voronoi geometry has an unmatched interior boundary"
            );
            VoronoiBoundaryKind::Interior {
                first_cell: owners[0].min(owners[1]),
                second_cell: owners[0].max(owners[1]),
            }
        };
        boundaries.push(VoronoiBoundary { start, end, kind });
    }
    Ok(boundaries)
}

fn artboard_polygon(domain: DomainBounds) -> Vec<OrderedPoint> {
    vec![
        OrderedPoint { x: 0.0, y: 0.0 },
        OrderedPoint {
            x: f64::from(domain.width),
            y: 0.0,
        },
        OrderedPoint {
            x: f64::from(domain.width),
            y: f64::from(domain.height),
        },
        OrderedPoint {
            x: 0.0,
            y: f64::from(domain.height),
        },
    ]
}

fn edges(points: &[OrderedPoint]) -> impl Iterator<Item = (OrderedPoint, OrderedPoint)> + '_ {
    points
        .iter()
        .copied()
        .zip(points.iter().copied().cycle().skip(1))
        .take(points.len())
}

fn clip_to_nearest_site(
    polygon: Vec<OrderedPoint>,
    site: OrderedPoint,
    other: OrderedPoint,
) -> Vec<OrderedPoint> {
    let normal_x = other.x - site.x;
    let normal_y = other.y - site.y;
    let threshold =
        (other.x * other.x + other.y * other.y - site.x * site.x - site.y * site.y) / 2.0;
    clip_polygon(
        polygon,
        |point| point.x * normal_x + point.y * normal_y - threshold,
        true,
    )
}

fn clip_to_supporting_half_plane(
    polygon: Vec<OrderedPoint>,
    normal_x: f64,
    normal_y: f64,
    threshold: f64,
) -> Vec<OrderedPoint> {
    clip_polygon(
        polygon,
        |point| point.x * normal_x + point.y * normal_y - threshold,
        false,
    )
}

fn clip_polygon(
    polygon: Vec<OrderedPoint>,
    distance: impl Fn(OrderedPoint) -> f64,
    keep_negative: bool,
) -> Vec<OrderedPoint> {
    if polygon.is_empty() {
        return polygon;
    }
    let mut clipped = Vec::with_capacity(polygon.len() + 1);
    let mut previous = *polygon.last().expect("non-empty polygon");
    let mut previous_distance = distance(previous);
    for current in polygon {
        let current_distance = distance(current);
        let previous_inside = if keep_negative {
            previous_distance <= POLYGON_EPSILON
        } else {
            previous_distance >= -POLYGON_EPSILON
        };
        let current_inside = if keep_negative {
            current_distance <= POLYGON_EPSILON
        } else {
            current_distance >= -POLYGON_EPSILON
        };
        if previous_inside != current_inside {
            let denominator = previous_distance - current_distance;
            if denominator.abs() > f64::EPSILON {
                let amount = (previous_distance / denominator).clamp(0.0, 1.0);
                clipped.push(OrderedPoint {
                    x: previous.x + (current.x - previous.x) * amount,
                    y: previous.y + (current.y - previous.y) * amount,
                });
            }
        }
        if current_inside {
            clipped.push(current);
        }
        previous = current;
        previous_distance = current_distance;
    }
    deduplicate_polygon(clipped)
}

fn deduplicate_polygon(mut polygon: Vec<OrderedPoint>) -> Vec<OrderedPoint> {
    polygon.dedup_by(|left, right| {
        squared_distance(*left, *right) <= POLYGON_EPSILON * POLYGON_EPSILON
    });
    if polygon.len() > 1
        && squared_distance(
            *polygon.first().expect("non-empty polygon"),
            *polygon.last().expect("non-empty polygon"),
        ) <= POLYGON_EPSILON * POLYGON_EPSILON
    {
        polygon.pop();
    }
    polygon
}

fn lies_on_artboard(start: OrderedPoint, end: OrderedPoint, domain: DomainBounds) -> bool {
    let on = |value: f64, boundary: f64| (value - boundary).abs() <= BOUNDARY_QUANTUM;
    (on(start.x, 0.0) && on(end.x, 0.0))
        || (on(start.x, f64::from(domain.width)) && on(end.x, f64::from(domain.width)))
        || (on(start.y, 0.0) && on(end.y, 0.0))
        || (on(start.y, f64::from(domain.height)) && on(end.y, f64::from(domain.height)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PointKey {
    x: i64,
    y: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SegmentKey {
    start: PointKey,
    end: PointKey,
}

fn segment_key(start: OrderedPoint, end: OrderedPoint) -> SegmentKey {
    let start = point_key(start);
    let end = point_key(end);
    if start <= end {
        SegmentKey { start, end }
    } else {
        SegmentKey {
            start: end,
            end: start,
        }
    }
}

fn point_key(point: OrderedPoint) -> PointKey {
    PointKey {
        x: (point.x / BOUNDARY_QUANTUM).round() as i64,
        y: (point.y / BOUNDARY_QUANTUM).round() as i64,
    }
}

fn point_from_key(key: PointKey, domain: DomainBounds) -> OrderedPoint {
    OrderedPoint {
        x: (key.x as f64 * BOUNDARY_QUANTUM).clamp(0.0, f64::from(domain.width)),
        y: (key.y as f64 * BOUNDARY_QUANTUM).clamp(0.0, f64::from(domain.height)),
    }
}

fn squared_distance(left: OrderedPoint, right: OrderedPoint) -> f64 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    dx * dx + dy * dy
}

fn signed_area(polygon: &[OrderedPoint]) -> f64 {
    edges(polygon)
        .map(|(current, next)| current.x * next.y - next.x * current.y)
        .sum::<f64>()
        / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOMAIN: DomainBounds = DomainBounds {
        width: 100,
        height: 80,
    };

    #[test]
    fn cells_are_bounded_and_interior_edges_are_shared() {
        let diagram = build_voronoi_diagram(
            DOMAIN,
            &[
                OrderedPoint { x: 25.0, y: 40.0 },
                OrderedPoint { x: 75.0, y: 40.0 },
            ],
        )
        .unwrap();
        assert_eq!(diagram.cells.len(), 2);
        assert!(
            diagram
                .cells
                .iter()
                .flat_map(|cell| &cell.vertices)
                .all(|point| {
                    (0.0..=100.0).contains(&point.x) && (0.0..=80.0).contains(&point.y)
                })
        );
        assert!(diagram.boundaries.iter().any(|boundary| matches!(
            boundary.kind,
            VoronoiBoundaryKind::Interior {
                first_cell: 0,
                second_cell: 1
            }
        )));
        assert!(
            diagram
                .boundaries
                .iter()
                .any(|boundary| matches!(boundary.kind, VoronoiBoundaryKind::Artboard))
        );
    }

    #[test]
    fn inset_excludes_only_interior_perimeter_and_retains_artboard_support() {
        let diagram = build_voronoi_diagram(
            DOMAIN,
            &[
                OrderedPoint { x: 25.0, y: 40.0 },
                OrderedPoint { x: 75.0, y: 40.0 },
            ],
        )
        .unwrap();
        let inset = inset_clipped_cell(DOMAIN, &diagram.cells[0], 4.0).unwrap();
        assert!(inset.iter().any(|point| point.x.abs() < 1.0e-6));
        assert!(inset.iter().any(|point| point.y.abs() < 1.0e-6));
        assert!(inset.iter().all(|point| point.x <= 46.0 + 1.0e-6));
    }

    #[test]
    fn degenerate_and_cancelled_requests_are_controlled_errors() {
        assert!(build_voronoi_diagram(DOMAIN, &[OrderedPoint { x: 1.0, y: 1.0 }]).is_err());
        assert!(
            build_voronoi_diagram(
                DOMAIN,
                &[
                    OrderedPoint { x: 1.0, y: 1.0 },
                    OrderedPoint { x: 1.0, y: 1.0 },
                ]
            )
            .is_err()
        );
        let token = CancellationToken::new();
        assert!(token.cancel());
        assert!(
            build_voronoi_diagram_cancellable(
                DOMAIN,
                &[
                    OrderedPoint { x: 1.0, y: 1.0 },
                    OrderedPoint { x: 99.0, y: 79.0 }
                ],
                GeometryLimits::default(),
                &token
            )
            .is_err()
        );
    }

    #[test]
    fn clustered_points_remain_bounded_and_limits_are_centralized() {
        let points: Vec<_> = (0..64)
            .map(|index| OrderedPoint {
                x: 40.0 + (index % 8) as f64 * 0.01,
                y: 30.0 + (index / 8) as f64 * 0.01,
            })
            .collect();
        let diagram = build_voronoi_diagram(DOMAIN, &points).unwrap();
        assert_eq!(diagram.cells.len(), points.len());
        assert!(diagram.cells.iter().all(|cell| cell.vertices.len() >= 3));
        assert!(
            build_voronoi_diagram_cancellable(
                DOMAIN,
                &points,
                GeometryLimits { max_sites: 63 },
                &CancellationToken::new()
            )
            .is_err()
        );
    }
}
