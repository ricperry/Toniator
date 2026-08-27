//! Focused Stage 20S public positive-region resize integration coverage.

use toniator_domain::{PatternMechanismId, PatternOutputLayerId, RegionResizeAlgorithm};
use toniator_geometry::{
    CanonicalRegionProposal, CanonicalRegionSourceGroup, CanonicalRegionSourceId, CurvePath,
    FamilySiteId, PathClosure, Point2, RegionTreatment, RegionTreatmentLimits,
    RegionTreatmentRequest, build_canonical_regions, treat_region_requests_cancellable,
};

/// Builds one producer-owned positive square before normalized resizing.
fn stage20s_source() -> toniator_geometry::CanonicalRegionSet {
    build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(20),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(20),
                ordinal: 0,
            }]),
            components: vec![
                CurvePath::polyline(
                    vec![
                        Point2::new(0.0, 0.0),
                        Point2::new(2.0, 0.0),
                        Point2::new(2.0, 2.0),
                        Point2::new(0.0, 2.0),
                    ],
                    PathClosure::Closed,
                )
                .expect("focused square is closed"),
            ],
        }],
    })
    .expect("focused region source canonicalizes")
}

/// Builds one public resize request with a finite producer reference.
fn stage20s_request(
    source: &toniator_geometry::CanonicalRegionSet,
    algorithm: RegionResizeAlgorithm,
    fill: f64,
) -> RegionTreatmentRequest {
    RegionTreatmentRequest {
        base_region_id: source.regions()[0].id.clone(),
        reference: Some(Point2::new(1.0, 1.0)),
        treatment: Some(RegionTreatment { algorithm, fill }),
    }
}

/// Proves exact zero omission and exact unit-boundary replay happen before canonical output.
#[test]
fn stage20s_zero_and_unit_fill_have_exact_positive_geometry_semantics() {
    let source = stage20s_source();
    for algorithm in [
        RegionResizeAlgorithm::Scale,
        RegionResizeAlgorithm::UniformOffset,
    ] {
        let omitted = treat_region_requests_cancellable(
            PatternOutputLayerId(20),
            &source,
            &[stage20s_request(&source, algorithm, 0.0)],
            RegionTreatmentLimits::default(),
            || false,
        )
        .expect("zero fill resolves");
        assert!(omitted.regions.regions().is_empty());
        assert!(omitted.provenance.is_empty());

        let natural = treat_region_requests_cancellable(
            PatternOutputLayerId(20),
            &source,
            &[stage20s_request(&source, algorithm, 1.0)],
            RegionTreatmentLimits::default(),
            || false,
        )
        .expect("unit fill resolves");
        assert_eq!(natural.regions, source);
    }
}

/// Proves both algorithms deterministically make fill two four times the positive source area.
#[test]
fn stage20s_fill_two_doubles_geometric_radius_for_both_algorithms() {
    let source = stage20s_source();
    for algorithm in [
        RegionResizeAlgorithm::Scale,
        RegionResizeAlgorithm::UniformOffset,
    ] {
        let first = treat_region_requests_cancellable(
            PatternOutputLayerId(20),
            &source,
            &[stage20s_request(&source, algorithm, 2.0)],
            RegionTreatmentLimits::default(),
            || false,
        )
        .expect("fill two resolves");
        let replay = treat_region_requests_cancellable(
            PatternOutputLayerId(20),
            &source,
            &[stage20s_request(&source, algorithm, 2.0)],
            RegionTreatmentLimits::default(),
            || false,
        )
        .expect("fill two replay resolves");
        assert!((first.regions.regions()[0].area - source.regions()[0].area * 4.0).abs() < 1e-6);
        assert_eq!(first, replay);
    }
}

/// Proves the RegionRound cubic-arc quadratic makes a triangular fill two four times its base area.
#[test]
fn stage20s_uniform_offset_triangle_uses_its_actual_round_join_area() {
    let source = build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(20),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(20),
                ordinal: 1,
            }]),
            components: vec![
                CurvePath::polyline(
                    vec![
                        Point2::new(0.0, 0.0),
                        Point2::new(2.0, 0.0),
                        Point2::new(0.0, 2.0),
                    ],
                    PathClosure::Closed,
                )
                .expect("focused triangle is closed"),
            ],
        }],
    })
    .expect("focused triangle canonicalizes");
    let result = treat_region_requests_cancellable(
        PatternOutputLayerId(20),
        &source,
        &[stage20s_request(
            &source,
            RegionResizeAlgorithm::UniformOffset,
            2.0,
        )],
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("triangular uniform offset resolves");
    assert!(
        (result.regions.regions()[0].area - source.regions()[0].area * 4.0).abs() < 1e-6,
        "actual={} target={}",
        result.regions.regions()[0].area,
        source.regions()[0].area * 4.0
    );
}

/// Proves nonconvex producer geometry uses the bounded shared-work fallback without negative space.
#[test]
fn stage20s_uniform_offset_supports_nonconvex_region_geometry_with_bounded_tolerance() {
    let source = build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(20),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(20),
                ordinal: 2,
            }]),
            components: vec![
                CurvePath::polyline(
                    vec![
                        Point2::new(0.0, 0.0),
                        Point2::new(3.0, 0.0),
                        Point2::new(3.0, 1.0),
                        Point2::new(1.0, 1.0),
                        Point2::new(1.0, 3.0),
                        Point2::new(0.0, 3.0),
                    ],
                    PathClosure::Closed,
                )
                .expect("focused nonconvex ring is closed"),
            ],
        }],
    })
    .expect("focused nonconvex source canonicalizes");
    let result = treat_region_requests_cancellable(
        PatternOutputLayerId(20),
        &source,
        &[stage20s_request(
            &source,
            RegionResizeAlgorithm::UniformOffset,
            2.0,
        )],
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("nonconvex fallback resolves atomically");
    assert!(
        (result.regions.regions()[0].area - source.regions()[0].area * 4.0).abs()
            <= source.regions()[0].area * 4.0 * 1.0e-6 + 1.0e-9
    );
}
