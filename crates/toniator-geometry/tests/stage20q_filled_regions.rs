//! Focused Stage 20Q public filled-region treatment integration coverage.
//!
//! The geometry-owned module tests retain private allocation and offset-work hooks for the cubic,
//! split, winding, shared-budget, limit, and cancellation fault cases; this target validates the
//! public producer-to-treated-region contract those hooks protect.

use toniator_domain::{PatternMechanismId, PatternOutputLayerId};
use toniator_geometry::{
    CanonicalRegionProposal, CanonicalRegionSourceGroup, CanonicalRegionSourceId, CurvePath,
    CurveSegment, FamilySiteId, PathClosure, Point2, RegionTreatment, RegionTreatmentLimits,
    RegionTreatmentRequest, build_canonical_regions, treat_region_requests_cancellable,
};

/// Builds one producer-owned positive triangle before treatment and never asks rendering to repair it.
fn stage20q_source() -> toniator_geometry::CanonicalRegionSet {
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
                        Point2::new(0.0, 2.0),
                    ],
                    PathClosure::Closed,
                )
                .expect("focused triangle is closed"),
            ],
        }],
    })
    .expect("focused region source canonicalizes")
}

/// Proves Full identity, producer-reference Scale, signed Gap, empty collapse, and provenance are
/// all carried by geometry before a renderer sees the result.
#[test]
fn stage20q_treatment_preserves_identity_and_base_provenance() {
    let source = stage20q_source();
    let base = &source.regions()[0];
    let full = treat_region_requests_cancellable(
        PatternOutputLayerId(20),
        &source,
        &[RegionTreatmentRequest {
            base_region_id: base.id.clone(),
            reference: None,
            treatment: Some(RegionTreatment::Full),
        }],
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("Full replays canonical source");
    assert_eq!(full.regions, source);
    assert_eq!(full.provenance[0].base_region_id, base.id);

    let scale = treat_region_requests_cancellable(
        PatternOutputLayerId(20),
        &source,
        &[RegionTreatmentRequest {
            base_region_id: base.id.clone(),
            reference: Some(Point2::new(-1.0, -1.0)),
            treatment: Some(RegionTreatment::Scale(2.0)),
        }],
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("Scale accepts producer reference outside the region");
    assert!(scale.regions.regions()[0].area > base.area);

    let suppressed = treat_region_requests_cancellable(
        PatternOutputLayerId(20),
        &source,
        &[RegionTreatmentRequest {
            base_region_id: base.id.clone(),
            reference: Some(Point2::new(0.0, 0.0)),
            treatment: None,
        }],
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("zero-alpha suppression omits a base before treatment");
    assert!(suppressed.regions.regions().is_empty());
    assert!(suppressed.provenance.is_empty());
}

/// Proves distinct untreated ordinals sharing one producer source remain independently treated
/// and retain their own provenance after aggregate source canonicalization.
#[test]
fn stage20q_shared_source_bases_keep_treatment_and_provenance_separate() {
    let source = build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(20),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(20),
                ordinal: 7,
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
                .expect("first shared-source triangle is closed"),
                CurvePath::polyline(
                    vec![
                        Point2::new(8.0, 0.0),
                        Point2::new(10.0, 0.0),
                        Point2::new(8.0, 2.0),
                    ],
                    PathClosure::Closed,
                )
                .expect("second shared-source triangle is closed"),
            ],
        }],
    })
    .expect("shared-source bases canonicalize as two ordinals");
    assert_eq!(source.regions().len(), 2);
    assert_eq!(
        source.regions()[0].id.source_id,
        source.regions()[1].id.source_id
    );

    let treated = treat_region_requests_cancellable(
        PatternOutputLayerId(20),
        &source,
        &[
            RegionTreatmentRequest {
                base_region_id: source.regions()[0].id.clone(),
                reference: Some(Point2::new(0.0, 0.0)),
                treatment: Some(RegionTreatment::Scale(0.5)),
            },
            RegionTreatmentRequest {
                base_region_id: source.regions()[1].id.clone(),
                reference: None,
                treatment: Some(RegionTreatment::ConstantGap(0.0)),
            },
        ],
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("shared-source requests remain valid");
    assert_eq!(treated.regions.regions().len(), 2);
    assert_eq!(treated.provenance.len(), 2);
    assert!(
        treated
            .provenance
            .iter()
            .any(|entry| entry.base_region_id == source.regions()[0].id)
    );
    assert!(
        treated
            .provenance
            .iter()
            .any(|entry| entry.base_region_id == source.regions()[1].id)
    );
    assert_eq!(
        treated
            .regions
            .regions()
            .iter()
            .map(|region| region.id.component_ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

/// Proves closed Region ConstantGap keeps an inset equilateral face triangular while splitting
/// each outward 120-degree corner into smooth circular cubic joins instead of a straight bevel.
#[test]
fn stage20q_constant_gap_keeps_equilateral_triangle_corners_round_outward() {
    let height = 3.0_f64.sqrt();
    let source = build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(20),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(20),
                ordinal: 42,
            }]),
            components: vec![
                CurvePath::polyline(
                    vec![
                        Point2::new(-1.0, 0.0),
                        Point2::new(1.0, 0.0),
                        Point2::new(0.0, height),
                    ],
                    PathClosure::Closed,
                )
                .expect("equilateral triangle is closed"),
            ],
        }],
    })
    .expect("equilateral source canonicalizes");
    let request = |gap| RegionTreatmentRequest {
        base_region_id: source.regions()[0].id.clone(),
        reference: None,
        treatment: Some(RegionTreatment::ConstantGap(gap)),
    };
    let outset = treat_region_requests_cancellable(
        PatternOutputLayerId(20),
        &source,
        &[request(-0.5)],
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("outward gap remains a triangular region");
    let inset = treat_region_requests_cancellable(
        PatternOutputLayerId(20),
        &source,
        &[request(0.5)],
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("inward gap remains a triangular region");
    assert_eq!(outset.regions.regions().len(), 1);
    assert_eq!(inset.regions.regions().len(), 1);
    let outward_segments = outset.regions.regions()[0].ring.segments();
    assert_eq!(
        outward_segments
            .iter()
            .filter(|segment| matches!(segment, CurveSegment::Line(_)))
            .count(),
        3
    );
    assert!(
        outward_segments
            .iter()
            .filter(|segment| matches!(segment, CurveSegment::CubicBezier(_)))
            .count()
            >= 3,
        "each convex outset corner has one or more smooth cubic arcs"
    );
    for (index, segment) in outward_segments.iter().enumerate() {
        if let CurveSegment::CubicBezier(_) = segment {
            let previous =
                outward_segments[(index + outward_segments.len() - 1) % outward_segments.len()];
            let next = outward_segments[(index + 1) % outward_segments.len()];
            assert_eq!(previous.end(), segment.start());
            assert_eq!(segment.end(), next.start());
            let before = previous
                .unit_tangent_at(1.0)
                .expect("previous tangent is finite");
            let start = segment
                .unit_tangent_at(0.0)
                .expect("cubic start tangent is finite");
            let end = segment
                .unit_tangent_at(1.0)
                .expect("cubic end tangent is finite");
            let after = next.unit_tangent_at(0.0).expect("next tangent is finite");
            assert!(before.x * start.x + before.y * start.y > 0.999_999);
            assert!(end.x * after.x + end.y * after.y > 0.999_999);
        }
    }
    assert!(outset.regions.regions()[0].area > 0.0);
    assert_eq!(inset.regions.regions()[0].ring.segments().len(), 3);
    assert!(
        inset.regions.regions()[0]
            .ring
            .segments()
            .iter()
            .all(|segment| matches!(segment, CurveSegment::Line(_)))
    );
    assert!(inset.regions.regions()[0].area > 0.0);
    assert!(outset.regions.regions()[0].area > source.regions()[0].area);
    assert!(inset.regions.regions()[0].area < source.regions()[0].area);
}

/// Proves a concave Region inset never inserts a circular join and instead leaves all corner
/// handling to tangent intersection plus the existing positive-ring dissolution boundary.
#[test]
fn stage20q_concave_inward_gap_uses_intersection_without_round_join() {
    let source = build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(20),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(20),
                ordinal: 43,
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
                .expect("concave L ring is closed"),
            ],
        }],
    })
    .expect("concave source canonicalizes");
    let treated = treat_region_requests_cancellable(
        PatternOutputLayerId(20),
        &source,
        &[RegionTreatmentRequest {
            base_region_id: source.regions()[0].id.clone(),
            reference: None,
            treatment: Some(RegionTreatment::ConstantGap(0.4)),
        }],
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("concave inset resolves through canonical cleanup");
    assert!(treated.regions.regions().iter().all(|region| {
        region.area > 0.0
            && region
                .ring
                .segments()
                .iter()
                .all(|segment| matches!(segment, CurveSegment::Line(_)))
    }));
}

/// Proves the production narrow-neck 10-unit bridge shrunk by seven units dissolves its
/// coincident collapse into deterministic positive split components without inward round joins.
#[test]
fn stage20q_narrow_neck_inward_gap_dissolves_coincident_overlap_into_split() {
    let source = build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(20),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(20),
                ordinal: 44,
            }]),
            components: vec![
                CurvePath::polyline(
                    vec![
                        Point2::new(60.0, 60.0),
                        Point2::new(120.0, 60.0),
                        Point2::new(120.0, 95.0),
                        Point2::new(180.0, 95.0),
                        Point2::new(180.0, 60.0),
                        Point2::new(240.0, 60.0),
                        Point2::new(240.0, 140.0),
                        Point2::new(180.0, 140.0),
                        Point2::new(180.0, 105.0),
                        Point2::new(120.0, 105.0),
                        Point2::new(120.0, 140.0),
                        Point2::new(60.0, 140.0),
                    ],
                    PathClosure::Closed,
                )
                .expect("narrow-neck source ring is closed"),
            ],
        }],
    })
    .expect("narrow-neck source canonicalizes");
    let request = RegionTreatmentRequest {
        base_region_id: source.regions()[0].id.clone(),
        reference: None,
        treatment: Some(RegionTreatment::ConstantGap(14.0)),
    };
    let treated = treat_region_requests_cancellable(
        PatternOutputLayerId(20),
        &source,
        std::slice::from_ref(&request),
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("coincident inward neck overlap dissolves rather than escaping as an error");
    let replay = treat_region_requests_cancellable(
        PatternOutputLayerId(20),
        &source,
        std::slice::from_ref(&request),
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("narrow-neck dissolution replays deterministically");
    assert_eq!(treated, replay);
    assert!(treated.regions.regions().len() > 1);
    assert!(treated.regions.regions().iter().all(|region| {
        region.area > 0.0
            && region
                .ring
                .segments()
                .iter()
                .all(|segment| matches!(segment, CurveSegment::Line(_)))
    }));
}
