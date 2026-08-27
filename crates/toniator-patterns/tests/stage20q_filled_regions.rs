//! Focused Stage 20S typed region-realizer integration coverage.

use toniator_domain::{
    CanvasSpec, ChannelPaint, EffectivePatternOutputSettings, PatternGeometryResponse,
    PatternMechanismId, PatternOutputLayerId, RegionGeometryResponse, RegionResizeAlgorithm,
    RegionSamplingStrategy, RegionSourceIntent, SiteUseFilter, SourceMapping,
    SourceMappingComponent,
};
use toniator_geometry::{
    CanonicalRegionProposal, CanonicalRegionSet, CanonicalRegionSourceGroup,
    CanonicalRegionSourceId, CurvePath, FamilySiteId, PathClosure, Point2, RegionReference,
    RegionTreatmentLimits, build_canonical_regions,
};
use toniator_patterns::{
    OutputCapability, OutputCapabilityPayload, StructuralProductCapability,
    realize_region_output_cancellable,
};
use toniator_sampling::{RegionSamplingLimits, SourceField, SourceFormatHint, decode_source};

/// Builds one untreated square, its region capability, and its producer reference.
fn stage20s_region_input() -> (CanonicalRegionSet, OutputCapability, Vec<RegionReference>) {
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
                        Point2::new(2.0, 2.0),
                        Point2::new(0.0, 2.0),
                    ],
                    PathClosure::Closed,
                )
                .expect("focused square is closed"),
            ],
        }],
    })
    .expect("untreated canonical region builds");
    let capability = OutputCapability {
        layer_id: output,
        source_filter: SiteUseFilter::All,
        consumes: StructuralProductCapability::RandomSites,
        payload: OutputCapabilityPayload::Regions {
            source: RegionSourceIntent::VoronoiSites {
                site_mechanism_id: PatternMechanismId(20),
            },
        },
    };
    let references = vec![RegionReference {
        region_id: untreated.regions()[0].id.clone(),
        point: Point2::new(1.0, 1.0),
    }];
    (untreated, capability, references)
}

/// Decodes one immutable project-wide source fixture for normalized-fill sampling.
fn stage20s_source(name: &str, hint: SourceFormatHint) -> SourceField {
    let bytes = std::fs::read(format!(
        "{}/../../assets/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("project-wide source fixture reads");
    decode_source(&bytes, hint).expect("project-wide source fixture decodes")
}

/// Builds one explicit normalized region response for the direct realizer boundary.
fn stage20s_response(
    algorithm: RegionResizeAlgorithm,
    sampling: RegionSamplingStrategy,
    minimum_fill: f64,
    maximum_fill: f64,
) -> EffectivePatternOutputSettings {
    EffectivePatternOutputSettings {
        output_layer_id: PatternOutputLayerId(20),
        response: PatternGeometryResponse::Regions(RegionGeometryResponse {
            algorithm,
            sampling,
            minimum_fill,
            maximum_fill,
        }),
    }
}

/// Realizes one normalized region response against a decoded immutable project source.
fn stage20s_realize(
    source: &SourceField,
    setting: &EffectivePatternOutputSettings,
) -> toniator_patterns::TypedRegionOutputRealization {
    let (untreated, capability, references) = stage20s_region_input();
    realize_region_output_cancellable(
        &capability,
        setting,
        &untreated,
        &references,
        Some(source),
        &CanvasSpec {
            width: 2.0,
            height: 2.0,
        },
        SourceMapping::canonical(SourceMappingComponent::Luminance),
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
    .expect("normalized region realization succeeds")
}

/// Proves both immutable project-wide source fixtures preserve the exact natural boundary at fill one.
#[test]
fn stage20s_reference_point_unit_fill_replays_natural_boundary_for_png_and_svg() {
    let setting = stage20s_response(
        RegionResizeAlgorithm::Scale,
        RegionSamplingStrategy::ReferencePoint,
        1.0,
        1.0,
    );
    for (name, hint) in [
        ("raster-sample.png", SourceFormatHint::Png),
        ("vector-sample.svg", SourceFormatHint::Svg),
    ] {
        let result = stage20s_realize(&stage20s_source(name, hint), &setting);
        let (untreated, _, _) = stage20s_region_input();
        assert_eq!(result.regions, untreated);
        assert_eq!(result.diagnostics.sampled_bases, 1);
    }
}

/// Proves AreaAverage maps a zero fill range to omission before canonical scene construction.
#[test]
fn stage20s_area_average_zero_fill_omits_the_region() {
    let result = stage20s_realize(
        &stage20s_source("raster-sample.png", SourceFormatHint::Png),
        &stage20s_response(
            RegionResizeAlgorithm::UniformOffset,
            RegionSamplingStrategy::AreaAverage,
            0.0,
            0.0,
        ),
    );
    assert!(result.regions.regions().is_empty());
    assert_eq!(result.diagnostics.sampled_bases, 1);
}

/// Proves UniformOffset maps fill two to a deterministic doubled geometric radius after sampling.
#[test]
fn stage20s_uniform_offset_fill_two_replays_deterministically() {
    let source = stage20s_source("raster-sample.png", SourceFormatHint::Png);
    let setting = stage20s_response(
        RegionResizeAlgorithm::UniformOffset,
        RegionSamplingStrategy::ReferencePoint,
        2.0,
        2.0,
    );
    let first = stage20s_realize(&source, &setting);
    let second = stage20s_realize(&source, &setting);
    assert_eq!(first, second);
    assert!((first.regions.regions()[0].area - 16.0).abs() < 1e-6);
}
