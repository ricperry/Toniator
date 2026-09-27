//! Exact-order and cancellation regressions for scatter candidate preparation.
use super::*;

/// Compares the optimized ordering with stable spatial sort and breadth-first median traversal.
///
/// # Panics
/// Panics if any point, duplicate tie, or truncatable prefix changes.
#[test]
fn candidate_order_matches_stable_reference() {
    let bounds = Bounds::new(Point2::new(0.0, 0.0), Point2::new(100.0, 70.0)).unwrap();
    let points: Vec<_> = (0..10_000)
        .map(|i| Point2::new((i * 19 % 100) as f64, (i * 31 % 70) as f64))
        .collect();
    let mut sorted = points.clone();
    sorted.sort_by_key(|point| spatial_order_key(*point, bounds));
    let mut reference = Vec::new();
    let mut intervals = vec![(0, sorted.len())];
    while !intervals.is_empty() {
        let mut next = Vec::new();
        for (start, end) in intervals {
            let middle = start + (end - start) / 2;
            reference.push(sorted[middle]);
            if start < middle {
                next.push((start, middle));
            }
            if middle + 1 < end {
                next.push((middle + 1, end));
            }
        }
        intervals = next;
    }
    let mut actual = points;
    spread_candidate_order(&mut actual, 100, bounds, &|| false).unwrap();
    assert_eq!(actual, reference);
}

/// Cancels during key generation, run sorting, merge and traversal without replacing the input.
///
/// # Panics
/// Panics if cancellation is not observed or a partial population escapes.
#[test]
fn candidate_order_cancels_transactionally() {
    let bounds = Bounds::new(Point2::new(0.0, 0.0), Point2::new(100.0, 70.0)).unwrap();
    let original = vec![Point2::new(1.0, 2.0); 10_000];
    for cancel_at in [1, 12, 20, 30] {
        let calls = std::cell::Cell::new(0);
        let mut points = original.clone();
        let result = spread_candidate_order(&mut points, 100, bounds, &|| {
            calls.set(calls.get() + 1);
            calls.get() >= cancel_at
        });
        assert_eq!(result.unwrap_err().path(), "evaluation.cancelled");
        assert_eq!(points, original);
    }
}
