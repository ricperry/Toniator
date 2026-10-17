//! Focused opt-in current index contracts without compiling obsolete historical lib tests.
use super::*;

/// Builds deterministic accepted records for the exact existing family-site authority.
///
/// # Panics
/// Panics if the fixed valid nominal basis violates geometry invariants.
fn sites(points: &[Point2]) -> Vec<FamilySite> {
    points
        .iter()
        .enumerate()
        .map(|(ordinal, &position)| FamilySite {
            id: FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal,
            },
            position,
            nominal_cell_basis: NominalCellBasis::new(Vector2::new(1., 0.), Vector2::new(0., 1.))
                .unwrap(),
            scope: SiteScope::Canvas,
            provenance: FamilySiteProvenance::Random {
                candidate_ordinal: ordinal,
                accepted_ordinal: ordinal,
                exclusion_neighbor_ordinal: None,
            },
        })
        .collect()
}

/// Replays dense/sparse lookup against the previous bucket-vector oracle and exact work charge.
/// Signed cells, boundary cells, dense-outside inserts and traversal priority remain identical.
///
/// # Panics
/// Panics if conflict order, whole-bucket accounting, minimum distance or storage fallback changes.
pub fn traversal_and_work_match_oracle() {
    let bounds = Bounds::new(Point2::new(-3., -3.), Point2::new(3., 3.)).unwrap();
    let points = [
        Point2::new(-1.9, -1.9),
        Point2::new(-1.1, -1.1),
        Point2::new(0.1, 0.1),
        Point2::new(0.9, 0.9),
        Point2::new(3., 3.),
        Point2::new(20., 20.),
    ];
    let records = sites(&points);
    let mut dense = SpatialIndex::with_bounds(1., usize::MAX, Some(bounds), &|| false).unwrap();
    let mut sparse = SpatialIndex::new(1., usize::MAX).unwrap();
    let mut oracle: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    assert!(dense.dense.is_some());
    for (ordinal, point) in points.iter().copied().enumerate() {
        dense.insert(point, ordinal).unwrap();
        sparse.insert(point, ordinal).unwrap();
        oracle
            .entry(spatial_cell(point, 1.).unwrap())
            .or_default()
            .push(ordinal);
    }
    assert_eq!(
        dense.cells.len(),
        1,
        "outside-envelope insert remains indexed"
    );
    for point in [
        Point2::new(-1.4, -1.4),
        Point2::new(0.4, 0.4),
        Point2::new(3., 3.),
        Point2::new(20., 20.),
        Point2::new(-4., 0.),
        Point2::new(1., 1.),
    ] {
        let (x, y) = spatial_cell(point, 1.).unwrap();
        let mut expected = None;
        let mut work = 0;
        'search: for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(bucket) = oracle.get(&(x + dx, y + dy)) {
                    work += bucket.len();
                    for &ordinal in bucket {
                        if point_distance(point, points[ordinal]) < 1. {
                            expected = Some(ordinal);
                            break 'search;
                        }
                    }
                }
            }
        }
        for index in [&mut dense, &mut sparse] {
            let before = index.neighbor_work;
            assert_eq!(
                index.find_conflict(point, &records, 1., &|| false).unwrap(),
                expected
            );
            assert_eq!(index.neighbor_work - before, work);
            let before = index.neighbor_work;
            assert_eq!(
                index
                    .find_point_conflict(point, &points, 1., &|| false)
                    .unwrap(),
                expected.is_some()
            );
            assert_eq!(index.neighbor_work - before, work);
        }
    }
}

/// Exercises dense reservation failure/ineligibility, cancellation and exact coordinate diagnostics.
/// Failure is injected at the reservation seam; this does not claim actual host OOM evidence.
///
/// # Panics
/// Panics if fallback drops valid points, creates a creative ceiling, or changes checked error paths.
pub fn fallback_limits_and_cancellation() {
    let small = Bounds::new(Point2::new(-2., -2.), Point2::new(2., 2.)).unwrap();
    let mut failed =
        SpatialIndex::with_reservation(1., usize::MAX, Some(small), &|| false, |_, _| false)
            .unwrap();
    assert!(failed.dense.is_none());
    failed.insert(Point2::new(0., 0.), 0).unwrap();
    assert!(
        failed
            .find_point_conflict(Point2::new(0., 0.), &[Point2::new(0., 0.)], 1., &|| false)
            .unwrap()
    );
    let huge = Bounds::new(Point2::new(-1e12, -1e12), Point2::new(1e12, 1e12)).unwrap();
    assert!(
        SpatialIndex::with_bounds(1., usize::MAX, Some(huge), &|| false)
            .unwrap()
            .dense
            .is_none()
    );
    let polls = std::cell::Cell::new(0);
    let cancelled = SpatialIndex::with_bounds(0.01, usize::MAX, Some(small), &|| {
        polls.set(polls.get() + 1);
        polls.get() >= 3
    });
    assert_eq!(cancelled.err().unwrap().path(), "evaluation.cancelled");
    for bounds in [None, Some(small)] {
        let points = [Point2::new(0.1, 0.1), Point2::new(0.2, 0.2)];
        let mut index = SpatialIndex::with_bounds(1., 1, bounds, &|| false).unwrap();
        for (n, p) in points.iter().copied().enumerate() {
            index.insert(p, n).unwrap();
        }
        let error = index
            .find_point_conflict(Point2::new(0.1, 0.1), &points, 1., &|| false)
            .unwrap_err();
        assert_eq!(error.path(), "coverage.random_sites.neighbor_limit");
        assert_eq!(index.neighbor_work, 2);
        assert_eq!(
            index
                .cell(Point2::new(f64::INFINITY, 0.))
                .unwrap_err()
                .path(),
            "coverage.random_sites.spatial_index"
        );
        let extreme = Point2::new(i64::MIN as f64, 0.);
        assert_eq!(
            index
                .find_point_conflict(extreme, &points, 1., &|| false)
                .unwrap_err()
                .message(),
            "Poisson neighbor coordinate overflowed"
        );
        assert_eq!(
            index
                .find_conflict(extreme, &sites(&points), 1., &|| false)
                .unwrap_err()
                .message(),
            "exclusion neighbor coordinate overflowed"
        );
        assert_eq!(
            index
                .find_point_conflict(Point2::new(0., 0.), &points, 1., &|| true)
                .unwrap_err()
                .path(),
            "evaluation.cancelled"
        );
    }
}

/// Compares streamed fingerprint bytes and candidate median order to independent previous algorithms.
///
/// # Panics
/// Panics if base bytes, float identity, duplicate ordinal ties, complete order or cancellation changes.
pub fn fingerprint_and_order_match_previous_contract() {
    let bounds = Bounds::new(Point2::new(0., 0.), Point2::new(100., 70.)).unwrap();
    let points: Vec<_> = (0..10000)
        .map(|n| Point2::new((n * 19 % 100) as f64, (n * 31 % 70) as f64))
        .collect();
    let records = sites(&points);
    let mut bytes = b"baseline|nominal-cell-basis-v1".to_vec();
    for site in &records {
        bytes.extend(site.id.mechanism_id.0.to_le_bytes());
        bytes.extend((site.id.ordinal as u64).to_le_bytes());
        for v in [
            site.position.x,
            site.position.y,
            site.nominal_cell_basis.axis_a.x,
            site.nominal_cell_basis.axis_a.y,
            site.nominal_cell_basis.axis_b.x,
            site.nominal_cell_basis.axis_b.y,
            site.nominal_cell_basis.diameter(),
        ] {
            bytes.extend(v.to_bits().to_le_bytes());
        }
    }
    assert_eq!(
        family_site_fingerprint("baseline", &records),
        format!("baseline:nominal-cell-basis:{}", fnv1a64(bytes))
    );
    let mut sorted = points.clone();
    sorted.sort_by_key(|p| spatial_order_key(*p, bounds));
    let mut queue = std::collections::VecDeque::from([(0, sorted.len())]);
    let mut expected = Vec::new();
    while let Some((start, end)) = queue.pop_front() {
        let mid = start + (end - start) / 2;
        expected.push(sorted[mid]);
        if start < mid {
            queue.push_back((start, mid));
        }
        if mid + 1 < end {
            queue.push_back((mid + 1, end));
        }
    }
    let mut actual = points.clone();
    spread_candidate_order(&mut actual, points.len(), bounds, &|| false).unwrap();
    assert_eq!(actual, expected);
    for stop in [1, 11, 19, 25, 35] {
        let count = std::cell::Cell::new(0);
        let mut actual = points.clone();
        let error = spread_candidate_order(&mut actual, points.len(), bounds, &|| {
            count.set(count.get() + 1);
            count.get() >= stop
        })
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
        assert_eq!(actual, points);
    }
}
