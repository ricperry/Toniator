#![cfg(feature = "test-evidence")]
use toniator_patterns::*;
use toniator_patterns::artistic_tone_evidence::{compose_source_location, offset_group_tone_domains};

/// Builds the existing diagnostic cusp cubic whose middle source span disappears after offset.
/// # Panics
/// Panics if the fixed validated cubic cannot be represented.
fn source() -> CurvePath {
    CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(20.0, 160.0),
                Point2::new(96.0, 64.0),
                Point2::new(224.0, 64.0),
                Point2::new(300.0, 160.0),
            )
            .unwrap(),
        )],
        PathClosure::Open,
    )
    .unwrap()
}

/// Proves actual cleanup preserves compact source maps and removed zero collapses to a relink boundary.
/// # Panics
/// Panics if cleanup source correspondence disappears or group origins restart at derived pieces.
#[test]
fn cleanup_removed_zero_uses_retained_source_traversal_boundary() {
    let source = source();
    let PathOffsetResult::Paths(components) = offset_path_cancellable(
        PathOffsetRequest {
            path: &source,
            signed_distance: 216.0,
            endpoint_policy: PathOffsetEndpointPolicy::Preserve,
            cleanup: PathOffsetCleanup::DissolveCrossings,
            crossing_barriers: &[],
            limits: PathOffsetLimits::default(),
        },
        &|| false,
    )
    .unwrap() else {
        panic!("diagnostic branches survive")
    };
    assert_eq!(components.len(), 2);
    for component in &components {
        assert_eq!(
            component.path.segments().len(),
            component.source_intervals.len()
        );
    }
    let domains = offset_group_tone_domains(
        &components,
        PathLocation::new(0, 0.5).unwrap(),
        true,
        &|| false,
    )
    .unwrap();
    let first_length = components[0]
        .path
        .measure_arc_length()
        .unwrap()
        .total_length();
    assert!((domains[0].start_distance + first_length - domains[1].start_distance).abs() < 1e-9);
    assert!(domains[0].start_distance <= 0.0);
    assert!(domains[1].start_distance >= -1e-9);
    // The missing middle owns no invented point: zero collapses exactly to the following boundary.
    assert!(domains[1].start_distance.abs() < 1e-9);
}

/// Proves successive offset ranks compose to original source spans rather than previous-rank indices.
/// # Panics
/// Panics if a valid second rank loses source correspondence or fails transported-origin resolution.
#[test]
fn successive_ranks_compose_original_segment_correspondence() {
    let source = source();
    let PathOffsetResult::Paths(first) = offset_path_cancellable(
        PathOffsetRequest {
            path: &source,
            signed_distance: 40.0,
            endpoint_policy: PathOffsetEndpointPolicy::Preserve,
            cleanup: PathOffsetCleanup::PlanarConstantGap,
            crossing_barriers: &[],
            limits: PathOffsetLimits::default(),
        },
        &|| false,
    )
    .unwrap() else {
        panic!("first rank survives")
    };
    let mut second = Vec::new();
    for parent in &first {
        let PathOffsetResult::Paths(children) = advance_planar_constant_gap_frontier_cancellable(
            &parent.path,
            &parent.planar_switch_nodes,
            40.0,
            PathOffsetLimits::default(),
            &|| false,
        )
        .unwrap() else {
            panic!("second rank survives")
        };
        for mut child in children {
            child.source_start =
                compose_source_location(child.source_start, &parent.source_intervals).unwrap();
            child.source_end =
                compose_source_location(child.source_end, &parent.source_intervals).unwrap();
            for (a, b) in &mut child.source_intervals {
                *a = compose_source_location(*a, &parent.source_intervals).unwrap();
                *b = compose_source_location(*b, &parent.source_intervals).unwrap();
                assert_eq!(a.segment_index(), 0);
                assert_eq!(b.segment_index(), 0);
            }
            second.push(child);
        }
    }
    let domains =
        offset_group_tone_domains(&second, PathLocation::new(0, 0.5).unwrap(), true, &|| false)
            .unwrap();
    assert_eq!(domains.len(), second.len());
    assert!(domains[0].start_distance < 0.0);
}

/// Keeps genuine source-boundary aliases equivalent while retaining integer segment identity.
/// # Panics
/// Panics if a compact source map refuses a boundary alias or rounds a large source ordinal.
#[test]
fn source_boundary_aliases_and_integer_ordinals_remain_exact() {
    let a = PathLocation::new(0, 1.0).unwrap();
    let b = PathLocation::new(1, 0.0).unwrap();
    assert_eq!(
        compose_source_location(PathLocation::new(0, 0.5).unwrap(), &[(a, b)]).unwrap(),
        a
    );
    let ordinal = 1_usize << 53;
    let path = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)).unwrap();
    let component = toniator_geometry::OffsetPathComponent {
        component_ordinal: 0,
        source_start: PathLocation::new(ordinal, 0.25).unwrap(),
        source_end: PathLocation::new(ordinal, 0.75).unwrap(),
        source_intervals: vec![(
            PathLocation::new(ordinal, 0.25).unwrap(),
            PathLocation::new(ordinal, 0.75).unwrap(),
        )],
        path,
        planar_switch_nodes: vec![],
    };
    let domains = offset_group_tone_domains(
        &[component],
        PathLocation::new(ordinal, 0.5).unwrap(),
        true,
        &|| false,
    )
    .unwrap();
    assert_eq!(domains[0].start_distance, -5.0);
}
