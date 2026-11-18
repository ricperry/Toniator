//! Independent pre-change Lloyd reference for exact serial streaming regression checks.
//! The allocation-heavy reference below is frozen from444bafa and is test-only.

use super::*;

/// Compares complete saturated seeded populations, duplicates, guards and fingerprints to the frozen reference.
///
/// Global topology and all position bits remain identical with ordinary and source-weighted Lloyd;
/// failed refinement publishes no positions, IDs, scopes or provenance.
/// # Panics
/// Panics if streaming storage or prepared scalar reuse changes any authoritative family value.
#[test]
fn streaming_lloyd_matches_prechange_seeded_population_and_cancellation() {
    let canvas = CanvasSpec {
        width: 40.0,
        height: 30.0,
    };
    let bounds = Bounds::new(Point2::new(-3.0, -3.0), Point2::new(43.0, 33.0)).unwrap();
    let canvas_bounds = Bounds::new(Point2::new(0.0, 0.0), Point2::new(40.0, 30.0)).unwrap();
    let mut rng = StablePrng::new(19);
    let mut points =
        bridson_candidates(bounds, 1.5, 100_000, usize::MAX, &mut rng, &|| false).unwrap();
    assert!(points.len() > 300);
    points.push(points[0]);
    points.push(Point2::new(-0.0, 0.0));
    points.push(Point2::new(0.0, -0.0));
    let original = points
        .into_iter()
        .enumerate()
        .map(|(ordinal, position)| FamilySite {
            id: FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal,
            },
            position,
            nominal_cell_basis: NominalCellBasis::new(
                Vector2::new(1.5, 0.0),
                Vector2::new(0.0, 1.5),
            )
            .unwrap(),
            scope: if canvas_bounds.contains(position) {
                SiteScope::Canvas
            } else {
                SiteScope::Guard
            },
            provenance: FamilySiteProvenance::Random {
                candidate_ordinal: ordinal,
                accepted_ordinal: ordinal,
                exclusion_neighbor_ordinal: None,
            },
        })
        .collect::<Vec<_>>();
    let source = SourceField::from_straight_rgba8(
        2,
        3,
        vec![
            255, 10, 30, 0, 43, 174, 89, 255, 34, 192, 201, 31, 88, 70, 222, 192, 0, 0, 0, 255,
            255, 255, 255, 128,
        ],
    )
    .unwrap();
    for density_weighted in [false, true] {
        let random = RandomSiteCapability {
            character: RandomSiteCharacter::Even {
                minimum_center_distance: 1.5,
            },
            nominal_spacing: None,
            seed: 19,
            density_modulation: ResolvedSiteDensityModulation::Uniform,
            full_population_for_uniform_marks: false,
            exclusion: SiteExclusionPolicy::None,
            maximum_attempts: 100_000,
            maximum_neighbor_checks: 100_000,
            refinement: toniator_domain::RandomSiteRefinement {
                enabled: true,
                density_weighted,
                iterations: 4,
            },
            refinement_weighting: Some(SourceWeighting::canonical(
                SourceMappingComponent::Luminance,
            )),
        };
        let mut expected = original.clone();
        relax_random_sites(
            &mut expected,
            bounds,
            canvas_bounds,
            &canvas,
            &random,
            Some(&source),
            1.5,
            &|| false,
        )
        .unwrap();
        let mut actual = original.clone();
        super::relax_random_sites(
            &mut actual,
            bounds,
            canvas_bounds,
            &canvas,
            &random,
            Some(&source),
            1.5,
            &|| false,
        )
        .unwrap();
        assert_eq!(actual, expected);
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_eq!(actual.position.x.to_bits(), expected.position.x.to_bits());
            assert_eq!(actual.position.y.to_bits(), expected.position.y.to_bits());
        }
        assert_eq!(
            family_site_fingerprint("streaming-reference", &actual),
            family_site_fingerprint("streaming-reference", &expected)
        );
        let mut cancelled = original.clone();
        let polls = std::cell::Cell::new(0);
        let error = super::relax_random_sites(
            &mut cancelled,
            bounds,
            canvas_bounds,
            &canvas,
            &random,
            Some(&source),
            1.5,
            &|| {
                polls.set(polls.get() + 1);
                polls.get() >= 100
            },
        )
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
        assert_eq!(cancelled, original);
    }
}

/// Applies bounded clipped-cell Lloyd updates while preserving IDs and exclusion guarantees.
///
/// # Errors
///
/// Returns source-sampling, triangulation, cancellation, or finite-geometry diagnostics without
/// publishing a partially relaxed family.
#[allow(clippy::too_many_arguments)]
fn relax_random_sites(
    sites: &mut [FamilySite],
    bounds: Bounds,
    canvas_bounds: Bounds,
    canvas: &CanvasSpec,
    random: &RandomSiteCapability,
    source: Option<&SourceField>,
    exclusion_distance: f64,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), PatternPipelineError> {
    let weighting = if random.refinement.density_weighted {
        Some(
            random
                .refinement_weighting
                .ok_or(PatternPipelineError::new(
                    "pattern.family.random_sites.lloyd_weighting",
                    "density-weighted Lloyd relaxation requires channel weighting",
                ))?,
        )
    } else {
        None
    };
    let mut positions = sites.iter().map(|site| site.position).collect::<Vec<_>>();
    for _ in 0..random.refinement.iterations {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        let cells = clipped_relaxation_cells(&positions, bounds, is_cancelled)?;
        let mut next = positions.clone();
        for (index, (polygon, nearest_distance)) in cells.into_iter().enumerate() {
            if sites[index].scope == SiteScope::Guard {
                continue;
            }
            let Some(polygon) = polygon
                .map(|polygon| clip_polygon_to_bounds(&polygon, canvas_bounds))
                .filter(|polygon| !polygon.is_empty())
            else {
                continue;
            };
            let target = if let (Some(field), Some(weighting)) = (source, weighting) {
                weighted_polygon_centroid(&polygon, field, canvas, weighting)?
                    .unwrap_or(positions[index])
            } else {
                polygon_centroid(&polygon).unwrap_or(positions[index])
            };
            let dx = target.x - positions[index].x;
            let dy = target.y - positions[index].y;
            let length = (dx * dx + dy * dy).sqrt();
            let maximum_move = if exclusion_distance > 0.0 && nearest_distance.is_finite() {
                ((nearest_distance - exclusion_distance) * 0.5).max(0.0)
            } else {
                f64::INFINITY
            };
            let scale = if length > maximum_move && length > 0.0 {
                maximum_move / length
            } else {
                1.0
            };
            next[index] = Point2::new(
                positions[index].x + dx * scale,
                positions[index].y + dy * scale,
            );
        }
        positions = next;
    }
    for (site, position) in sites.iter_mut().zip(positions) {
        site.position = position;
    }
    Ok(())
}

type RelaxationCell = (Option<Vec<Point2>>, f64);

/// Builds each box-clipped Voronoi cell from Delaunay neighbors and nearest-neighbor slack.
/// Duplicate sites retain their position; collinear populations remain finite through box clipping.
/// A spatial hierarchy locates randomly ordered sites without long walks from the previous site;
/// insertion order and the per-site cancellation boundary remain deterministic and unchanged.
///
/// # Errors
///
/// Returns cancellation or finite insertion diagnostics.
fn clipped_relaxation_cells(
    positions: &[Point2],
    bounds: Bounds,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RelaxationCell>, PatternPipelineError> {
    let mut triangulation: DelaunayTriangulation<
        RelaxVertex,
        (),
        (),
        (),
        HierarchyHintGenerator<f64>,
    > = DelaunayTriangulation::new();
    let mut unique = HashMap::new();
    let mut duplicate = vec![false; positions.len()];
    for (index, point) in positions.iter().copied().enumerate() {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        let key = (
            normalized_coordinate_bits(point.x),
            normalized_coordinate_bits(point.y),
        );
        if unique.insert(key, index).is_some() {
            duplicate[index] = true;
            continue;
        }
        triangulation
            .insert(RelaxVertex {
                point: SpadePoint::new(point.x, point.y),
                site_index: index,
            })
            .map_err(|_| {
                PatternPipelineError::new(
                    "pattern.family.random_sites.lloyd_coordinates",
                    "Lloyd triangulation rejected finite site coordinates",
                )
            })?;
    }
    let mut cells = vec![(None, f64::INFINITY); positions.len()];
    for vertex in triangulation.vertices() {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        let index = vertex.data().site_index;
        let site = positions[index];
        let mut polygon = bounds.corners().to_vec();
        let mut nearest = f64::INFINITY;
        for edge in vertex.out_edges() {
            let neighbor = positions[edge.to().data().site_index];
            nearest = nearest.min(point_distance(site, neighbor));
            polygon = clip_polygon_to_site_half_plane(&polygon, site, neighbor);
            if polygon.is_empty() {
                break;
            }
        }
        cells[index] = ((!polygon.is_empty()).then_some(polygon), nearest);
    }
    for (index, is_duplicate) in duplicate.into_iter().enumerate() {
        if is_duplicate {
            cells[index] = (None, 0.0);
        }
    }
    Ok(cells)
}

/// Clips a convex polygon to points no farther from `site` than from `neighbor`.
fn clip_polygon_to_site_half_plane(
    polygon: &[Point2],
    site: Point2,
    neighbor: Point2,
) -> Vec<Point2> {
    if polygon.is_empty() {
        return Vec::new();
    }
    let nx = neighbor.x - site.x;
    let ny = neighbor.y - site.y;
    let boundary =
        (neighbor.x * neighbor.x + neighbor.y * neighbor.y - site.x * site.x - site.y * site.y)
            * 0.5;
    let side = |point: Point2| point.x * nx + point.y * ny - boundary;
    let mut output = Vec::new();
    let mut previous = *polygon.last().expect("nonempty polygon has a last point");
    let mut previous_side = side(previous);
    for &current in polygon {
        let current_side = side(current);
        let previous_inside = previous_side <= 0.0;
        let current_inside = current_side <= 0.0;
        if previous_inside != current_inside {
            let denominator = previous_side - current_side;
            if denominator != 0.0 {
                let t = previous_side / denominator;
                output.push(Point2::new(
                    previous.x + (current.x - previous.x) * t,
                    previous.y + (current.y - previous.y) * t,
                ));
            }
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_side = current_side;
    }
    output
}

/// Clips one convex cell to the visible canvas so source weighting never samples edge clamps.
fn clip_polygon_to_bounds(polygon: &[Point2], bounds: Bounds) -> Vec<Point2> {
    let mut output = polygon.to_vec();
    for (site, neighbor) in [
        (
            Point2::new(bounds.min.x + 0.5, 0.0),
            Point2::new(bounds.min.x - 0.5, 0.0),
        ),
        (
            Point2::new(bounds.max.x - 0.5, 0.0),
            Point2::new(bounds.max.x + 0.5, 0.0),
        ),
        (
            Point2::new(0.0, bounds.min.y + 0.5),
            Point2::new(0.0, bounds.min.y - 0.5),
        ),
        (
            Point2::new(0.0, bounds.max.y - 0.5),
            Point2::new(0.0, bounds.max.y + 0.5),
        ),
    ] {
        output = clip_polygon_to_site_half_plane(&output, site, neighbor);
    }
    output
}

/// Returns the finite area centroid of a convex clipped cell.
fn polygon_centroid(polygon: &[Point2]) -> Option<Point2> {
    let mut cross_sum = 0.0;
    let mut x_sum = 0.0;
    let mut y_sum = 0.0;
    for index in 0..polygon.len() {
        let first = polygon[index];
        let second = polygon[(index + 1) % polygon.len()];
        let cross = first.x * second.y - second.x * first.y;
        cross_sum += cross;
        x_sum += (first.x + second.x) * cross;
        y_sum += (first.y + second.y) * cross;
    }
    (cross_sum.abs() > f64::EPSILON)
        .then(|| Point2::new(x_sum / (3.0 * cross_sum), y_sum / (3.0 * cross_sum)))
}

/// Integrates artwork mass over a convex cell with a linear-exact triangle quadrature.
/// Zero-mass cells return `None`, retaining the prior site position.
///
/// # Errors
///
/// Returns the sampling layer's stable density diagnostic.
fn weighted_polygon_centroid(
    polygon: &[Point2],
    source: &SourceField,
    canvas: &CanvasSpec,
    weighting: SourceWeighting,
) -> Result<Option<Point2>, PatternPipelineError> {
    weighted_polygon_centroid_with(polygon, |point| {
        let sampled = source
            .sample_density_weight(point, canvas, weighting.mapping)
            .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?;
        Ok(completed_density_weight(
            sampled,
            weighting.mapping,
            weighting.strength,
            &weighting.response,
        ))
    })
}
