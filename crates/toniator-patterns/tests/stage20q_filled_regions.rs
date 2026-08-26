//! Focused Stage 20Q typed region-realizer integration coverage.
//!
//! The patterns module's `stage20q_region_realizer_tests` retains source-field and cancellation
//! fixtures for ReferencePoint/AreaAverage, interpolation, zero-alpha, provenance, and limits.

use toniator_domain::{
    CanvasSpec, ChannelPaint, EffectivePatternOutputSettings, PatternGeometryResponse,
    PatternMechanismId, PatternOutputLayerId, RegionGeometryResponse, RegionSamplingStrategy,
    RegionSourceIntent, SourceMapping, SourceMappingComponent,
};
use toniator_geometry::{
    CanonicalRegionProposal, CanonicalRegionSourceGroup, CanonicalRegionSourceId, CurvePath,
    FamilySiteId, PathClosure, Point2, RegionReference, RegionTreatmentLimits,
    build_canonical_regions,
};
use toniator_patterns::{
    OutputCapability, OutputCapabilityPayload, StructuralProductCapability,
    realize_region_output_cancellable,
};
use toniator_sampling::RegionSamplingLimits;

/// Proves the sole public realizer preserves Full-plus-solid canonical identity without source work.
#[test]
fn stage20q_full_solid_realizer_replays_untreated_regions() {
    let output = PatternOutputLayerId(20);
    let untreated = build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: output,
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
    .expect("untreated canonical region builds");
    let capability = OutputCapability {
        layer_id: output,
        consumes: StructuralProductCapability::RandomSites,
        payload: OutputCapabilityPayload::Regions {
            source: RegionSourceIntent::VoronoiSites {
                site_mechanism_id: PatternMechanismId(20),
            },
        },
    };
    let setting = EffectivePatternOutputSettings {
        output_layer_id: output,
        response: PatternGeometryResponse::Regions(RegionGeometryResponse::Full {
            sampling: RegionSamplingStrategy::ReferencePoint,
        }),
    };
    let result = realize_region_output_cancellable(
        &capability,
        &setting,
        &untreated,
        &[RegionReference {
            region_id: untreated.regions()[0].id.clone(),
            point: Point2::new(0.0, 0.0),
        }],
        None,
        &CanvasSpec {
            width: 2.0,
            height: 2.0,
        },
        SourceMapping::canonical(SourceMappingComponent::Alpha),
        &ChannelPaint::Solid(toniator_domain::ColorValue {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        }),
        RegionSamplingLimits::default(),
        RegionTreatmentLimits::default(),
        &|| false,
    )
    .expect("Full solid realization needs neither source nor sampling");
    assert_eq!(result.regions, untreated);
    assert!(result.paints.is_none());
    assert_eq!(result.diagnostics.sampled_bases, 0);
}
