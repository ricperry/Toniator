use toniator_domain::{
    CanvasSpec, ConnectedGeometryResponse, CoveragePolicy, Document, DocumentId,
    GeneralizedSiteProduct, GuideDimensionId, MarkOrientation, PathStrokeStyle, PatternDefinition,
    PatternDefinitionId, PatternGeometryResponse, PatternMechanismId, PatternOutputLayer,
    PatternOutputLayerId, SourceMapping, SourceMappingComponent, SourceReference,
    StraightGuideDimension, StraightGuideRepetition,
};
use toniator_patterns::{
    GridInspectRequest, RealizationStructuralInput, StrokeResponse,
    evaluate_document_typed_family_cancellable, realize_typed_canonical_strokes_cancellable,
    resolve_document_pattern_pipeline,
};
use toniator_sampling::{SourceFormatHint, decode_source};

/// Builds one one-layer guide-path document whose family emits raw ordered straight paths.
fn path_document() -> Document {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 64.0,
            height: 48.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document validates");
    let guide = PatternMechanismId(901);
    let mut definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(900),
        "path witness",
        guide,
        PatternMechanismId(902),
        PatternOutputLayerId(903),
        vec![StraightGuideDimension {
            id: GuideDimensionId(904),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(904)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = vec![PatternOutputLayer::GuidePaths {
        id: PatternOutputLayerId(903),
        guide_mechanism_id: guide,
        style: PathStrokeStyle::default(),
    }];
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition.id;
    settings.density.across_x = 3.0;
    settings.density.across_y = 3.0;
    settings.geometry_response = PatternGeometryResponse::Connected(ConnectedGeometryResponse {
        minimum_thickness: 0.2,
        maximum_thickness: 1.0,
    });
    Document::with_source_topology_and_authored_structures(
        DocumentId(900),
        base.canvas().clone(),
        base.source().clone(),
        vec![definition],
        settings,
        base.channel_model().expect("model").to_owned(),
        base.channel_topology().expect("topology").clone(),
        Vec::new(),
    )
    .expect("path document validates")
}

/// Builds the exact structural family and decoded immutable source used by the public path realizer.
fn path_family() -> (
    Document,
    toniator_patterns::TypedFamilyOutput,
    toniator_patterns::PatternPipelinePlan,
    toniator_sampling::SourceField,
) {
    let document = path_document();
    let definition = &document.pattern_definitions()[0];
    let plan =
        resolve_document_pattern_pipeline(&document, definition).expect("guide plan resolves");
    let family = evaluate_document_typed_family_cancellable(
        &document,
        definition,
        &GridInspectRequest {
            canvas: document.canvas().clone(),
            density: document.pattern_settings().density.clone(),
            rotation_degrees: 0.0,
            translation_x: 0.0,
            translation_y: 0.0,
            guard_steps: 1,
            support_radius: 32.0,
            max_family_candidates: 1_024,
        },
        &|| false,
    )
    .expect("guide family evaluates");
    let source = decode_source(
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="#808080"/></svg>"##,
        SourceFormatHint::Svg,
    ).expect("bounded test source decodes");
    (document, family, plan, source)
}

/// Proves a public guide-path realization consumes only ordered raw paths/bases and adaptively subdivides source footprint intervals.
#[test]
fn guide_path_provenance_is_not_site_authority_and_profile_is_adaptive() {
    let (document, family, plan, source) = path_family();
    let realized = realize_typed_canonical_strokes_cancellable(
        &family,
        &plan,
        &source,
        document.canvas(),
        SourceMapping::canonical(SourceMappingComponent::Red),
        StrokeResponse {
            minimum_thickness: 0.2,
            maximum_thickness: 1.0,
        },
        1.0,
        262_144,
        100_000,
        &|| false,
    )
    .expect("path realization succeeds");
    let RealizationStructuralInput::StructuralPaths {
        paths,
        nominal_bases,
    } = &realized.provenance.structural_input
    else {
        panic!("guide output never claims site input");
    };
    assert_eq!(paths.paths().len(), realized.output.strokes.len());
    assert!(
        paths
            .paths()
            .windows(2)
            .all(|pair| pair[0].id <= pair[1].id)
    );
    assert!(paths.paths().iter().all(|guide| {
        nominal_bases
            .get(&guide.id)
            .is_some_and(|basis| *basis > 0.0)
    }));
    assert!(
        realized
            .output
            .strokes
            .iter()
            .any(|stroke| stroke.profile.len() > stroke.path.segments().len() + 1)
    );
}

/// Proves configurable request-wide bounds and cancellation fail before any partial public realization is returned.
#[test]
fn guide_path_realization_observes_limits_and_cancellation() {
    let (document, family, plan, source) = path_family();
    let limited = realize_typed_canonical_strokes_cancellable(
        &family,
        &plan,
        &source,
        document.canvas(),
        SourceMapping::canonical(SourceMappingComponent::Red),
        StrokeResponse {
            minimum_thickness: 0.2,
            maximum_thickness: 1.0,
        },
        1.0,
        1,
        100_000,
        &|| false,
    )
    .expect_err("one profile sample cannot realize all paths");
    assert_eq!(limited.path(), "realization.stroke.profile_limit");
    let cancelled = realize_typed_canonical_strokes_cancellable(
        &family,
        &plan,
        &source,
        document.canvas(),
        SourceMapping::canonical(SourceMappingComponent::Red),
        StrokeResponse {
            minimum_thickness: 0.2,
            maximum_thickness: 1.0,
        },
        1.0,
        262_144,
        100_000,
        &|| true,
    )
    .expect_err("caller cancellation prevents realization");
    assert_eq!(cancelled.path(), "evaluation.cancelled");
}

/// Proves a heterogeneous mark/path recipe remains invalid rather than acquiring ambiguous provenance.
#[test]
fn mixed_mark_and_guide_outputs_reject_before_realization() {
    let document = path_document();
    let mut definition = document.pattern_definitions()[0].clone();
    definition
        .output_layers
        .push(PatternOutputLayer::CircularMarks {
            id: PatternOutputLayerId(999),
            site_mechanism_id: PatternMechanismId(902),
        });
    assert!(resolve_document_pattern_pipeline(&document, &definition).is_err());
}
