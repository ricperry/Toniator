use toniator_domain::{
    CanvasSpec, CoveragePolicy, GeneralizedSiteProduct, GuideDimensionId, MarkOrientation,
    PatternDefinition, PatternDefinitionId, PatternMechanismId, PatternOutputLayerId,
    ResolvedDensityMetric2D, SourceComponent, SourcePlacement, StraightGuideDimension,
    StraightGuideRepetition,
};
use toniator_patterns::{
    GridInspectRequest, MarkResponse, evaluate_typed_family, maximum_nominal_cell_diameter,
    radius_from_ink_with_diameter, realize_typed_diagnostic_outputs, resolve_pattern_pipeline,
};
use toniator_sampling::{SourceFormatHint, decode_source};

/// Proves the normalized response linearly scales the exact nominal-cell diameter at both overlap bounds.
#[test]
fn normalized_fill_maps_signal_to_nominal_diameter() {
    let response = MarkResponse {
        minimum_fill: 0.0,
        maximum_fill: 2.0,
        rotation_offset_degrees: 37.5,
    };

    assert_eq!(
        radius_from_ink_with_diameter(0.0, response, 5.0).expect("valid response"),
        0.0
    );
    assert_eq!(
        radius_from_ink_with_diameter(0.5, response, 5.0).expect("valid response"),
        2.5
    );
    assert_eq!(
        radius_from_ink_with_diameter(1.0, response, 5.0).expect("valid response"),
        5.0
    );
}

/// Proves normalized fill rejects an obsolete absolute-diameter value above the Stage 20E1 bound.
#[test]
fn normalized_fill_rejects_obsolete_absolute_size_ranges() {
    let response = MarkResponse {
        minimum_fill: 0.0,
        maximum_fill: 4.5,
        rotation_offset_degrees: 0.0,
    };

    let error =
        radius_from_ink_with_diameter(1.0, response, 5.0).expect_err("fill above two rejects");
    assert_eq!(error.path(), "realization.response");
}

/// Proves a repeated along-guide family preflights enough support for every realized nominal cell.
#[test]
fn repeated_along_guides_preflight_their_largest_nominal_cell() {
    let definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(101),
        "multiplied along guides",
        PatternMechanismId(102),
        PatternMechanismId(103),
        PatternOutputLayerId(104),
        vec![StraightGuideDimension {
            id: GuideDimensionId(105),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 10.0,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(105)],
            interval_multiplier: 10.0,
            phase: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    let mut request = GridInspectRequest {
        canvas: CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        density: ResolvedDensityMetric2D {
            across_x: 10.0,
            across_y: 10.0,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 1,
        support_radius: 0.0,
        max_family_candidates: 100_000,
    };
    let plan = resolve_pattern_pipeline(&definition).expect("valid generalized plan");
    let maximum_diameter =
        maximum_nominal_cell_diameter(&plan.family, &request.canvas, &request.density)
            .expect("finite maximum nominal diameter");
    request.support_radius = maximum_diameter;
    let family = evaluate_typed_family(&definition, &request)
        .expect("family evaluates with preflight envelope");

    assert!(
        family
            .site_set()
            .sites()
            .iter()
            .all(|site| site.nominal_cell_basis.diameter() <= maximum_diameter)
    );
    assert_eq!(family.planned_support_radius(), maximum_diameter);
    let source = decode_source(
        &std::fs::read("../../assets/raster-sample.png").expect("fixture reads"),
        SourceFormatHint::Png,
    )
    .expect("fixture decodes");
    let realization = realize_typed_diagnostic_outputs(
        &family,
        &plan,
        &source,
        &request.canvas,
        SourcePlacement::StretchToCanvas,
        SourceComponent::Luminance,
        MarkResponse {
            minimum_fill: 0.0,
            maximum_fill: 2.0,
            rotation_offset_degrees: 0.0,
        },
    )
    .expect("preflight envelope covers every realized maximum-fill mark");
    assert!(!realization.output.marks.is_empty());
}
