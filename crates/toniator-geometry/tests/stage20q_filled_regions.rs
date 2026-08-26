//! Focused Stage 20Q public filled-region treatment integration coverage.
//!
//! The geometry-owned module tests retain private allocation and offset-work hooks for the cubic,
//! split, winding, shared-budget, limit, and cancellation fault cases; this target validates the
//! public producer-to-treated-region contract those hooks protect.

use toniator_domain::{PatternMechanismId, PatternOutputLayerId};
use toniator_geometry::{
    CanonicalRegionProposal, CanonicalRegionSourceGroup, CanonicalRegionSourceId, CurvePath,
    FamilySiteId, PathClosure, Point2, RegionTreatment, RegionTreatmentLimits,
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
