//! Stage 20P typed pipeline projection witness.

use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureId,
    AuthoredStructureKind, CanvasSpec, CoveragePolicy, CurveRepetition, Document,
    GeneralizedSiteProduct, GuideDimension, GuideDimensionId, GuidePrototype, MarkOrientation,
    PatternDefinition, PatternDefinitionId, PatternMechanism, PatternMechanismId,
    PatternOutputLayer, PatternOutputLayerId, RegionSourceIntent, SourceReference,
    StraightGuideDimension, StraightGuideRepetition,
};
use toniator_patterns::{resolve_document_pattern_pipeline, resolve_pattern_pipeline};

/// Proves the pipeline projects Guide Faces as an ordered output capability rather than a site response.
#[test]
fn guide_face_output_resolves_with_its_selected_dimensions() {
    let mut definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(20),
        "guide faces",
        PatternMechanismId(21),
        PatternMechanismId(22),
        PatternOutputLayerId(23),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(24),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(25),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(24), GuideDimensionId(25)],
            merge_epsilon: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = vec![PatternOutputLayer::Regions {
        id: PatternOutputLayerId(23),
        source: RegionSourceIntent::GuideFaces {
            guide_mechanism_id: PatternMechanismId(21),
            dimensions: vec![GuideDimensionId(24), GuideDimensionId(25)],
        },
    }];
    let plan = resolve_pattern_pipeline(&definition).expect("Guide Faces plan");
    assert_eq!(plan.ordered_outputs.len(), 1);
    assert_eq!(plan.ordered_outputs[0].layer_id, PatternOutputLayerId(23));
    assert_eq!(
        plan.ordered_outputs[0].regions(),
        Some(&RegionSourceIntent::GuideFaces {
            guide_mechanism_id: PatternMechanismId(21),
            dimensions: vec![GuideDimensionId(24), GuideDimensionId(25)],
        }),
    );
    let PatternMechanism::StraightGuideDimensions { dimensions, .. } =
        &mut definition.mechanisms[0]
    else {
        panic!("straight fixture retains a guide mechanism");
    };
    dimensions.push(StraightGuideDimension {
        id: GuideDimensionId(26),
        baseline_angle_degrees: 120.0,
        phase: 0.0,
        repetition: StraightGuideRepetition {
            spacing_multiplier: 1.0,
        },
    });
    let PatternOutputLayer::Regions { source, .. } = &mut definition.output_layers[0] else {
        panic!("straight fixture retains a Guide Faces output");
    };
    *source = RegionSourceIntent::GuideFaces {
        guide_mechanism_id: PatternMechanismId(21),
        dimensions: vec![
            GuideDimensionId(24),
            GuideDimensionId(25),
            GuideDimensionId(26),
        ],
    };
    let three = resolve_pattern_pipeline(&definition).expect("three selected guide faces plan");
    assert_eq!(
        three.ordered_outputs[0].regions(),
        Some(&RegionSourceIntent::GuideFaces {
            guide_mechanism_id: PatternMechanismId(21),
            dimensions: vec![
                GuideDimensionId(24),
                GuideDimensionId(25),
                GuideDimensionId(26),
            ],
        }),
    );
}

/// Builds one document that owns the authored open resources used by the generic guide resolver.
fn authored_guide_document() -> Document {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 64.0,
            height: 48.0,
        },
        SourceReference::Unassigned,
    )
    .expect("base document");
    let cubic = AuthoredStructure::new(
        AuthoredStructureId(31),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::CubicBezier {
            start: AuthoredPoint2 { x: 0.0, y: 16.0 },
            control_1: AuthoredPoint2 { x: 16.0, y: 4.0 },
            control_2: AuthoredPoint2 { x: 48.0, y: 4.0 },
            end: AuthoredPoint2 { x: 64.0, y: 16.0 },
        }],
    )
    .expect("cubic resource");
    let line = AuthoredStructure::new(
        AuthoredStructureId(32),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 20.0, y: 0.0 },
            end: AuthoredPoint2 { x: 20.0, y: 48.0 },
        }],
    )
    .expect("line resource");
    Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        base.pattern_definition_bundles().to_vec(),
        base.pattern_settings().clone(),
        base.channel_model().expect("model").to_owned(),
        base.channel_topology().expect("topology").clone(),
        vec![cubic, line],
    )
    .expect("resource document")
}

/// Builds one generic authored-guide definition with a fixed Guide Faces output binding.
fn authored_guide_face_definition(second_prototype: GuidePrototype) -> PatternDefinition {
    let guide_id = PatternMechanismId(41);
    let first = GuideDimensionId(42);
    let second = GuideDimensionId(43);
    let mut definition = PatternDefinition::generalized_guides(
        PatternDefinitionId(40),
        "authored guide faces",
        guide_id,
        PatternMechanismId(44),
        PatternOutputLayerId(45),
        vec![
            GuideDimension {
                id: first,
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(31),
                },
                repetition: CurveRepetition::Single,
            },
            GuideDimension {
                id: second,
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                prototype: second_prototype,
                repetition: CurveRepetition::Single,
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![first, second],
            merge_epsilon: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = vec![PatternOutputLayer::Regions {
        id: PatternOutputLayerId(45),
        source: RegionSourceIntent::GuideFaces {
            guide_mechanism_id: guide_id,
            dimensions: vec![first, second],
        },
    }];
    definition
}

/// Proves document-aware generic resolution accepts authored paths and rejects wrong or ineligible Guide Faces sources.
#[test]
fn generic_authored_guide_faces_have_a_strict_capability_boundary() {
    let document = authored_guide_document();
    let definition = authored_guide_face_definition(GuidePrototype::AuthoredOpenPath {
        structure_id: AuthoredStructureId(32),
    });
    let plan = resolve_document_pattern_pipeline(&document, &definition)
        .expect("authored open guide faces resolve with document resources");
    assert_eq!(plan.ordered_outputs.len(), 1);
    assert!(plan.family.generic_guides.is_some());
    let circular = authored_guide_face_definition(GuidePrototype::CircularArc {
        center: AuthoredPoint2 { x: 32.0, y: 24.0 },
        radius: 12.0,
        start_angle_degrees: 0.0,
        sweep_angle_degrees: 90.0,
    });
    assert_eq!(
        resolve_document_pattern_pipeline(&document, &circular)
            .expect_err("circular Guide Faces source rejects")
            .path(),
        "pattern.output_layers.guide_faces",
    );
    let mut foreign = definition.clone();
    let PatternOutputLayer::Regions { source, .. } = &mut foreign.output_layers[0] else {
        panic!("fixture retains regions output");
    };
    *source = RegionSourceIntent::GuideFaces {
        guide_mechanism_id: PatternMechanismId(99),
        dimensions: vec![GuideDimensionId(42), GuideDimensionId(43)],
    };
    assert_eq!(
        resolve_document_pattern_pipeline(&document, &foreign)
            .expect_err("foreign guide source rejects")
            .path(),
        "pattern.output_layers.guide_faces",
    );
    let mut multiple = definition;
    multiple.output_layers.push(PatternOutputLayer::Regions {
        id: PatternOutputLayerId(46),
        source: RegionSourceIntent::GuideFaces {
            guide_mechanism_id: PatternMechanismId(41),
            dimensions: vec![GuideDimensionId(42), GuideDimensionId(43)],
        },
    });
    assert_eq!(
        resolve_document_pattern_pipeline(&document, &multiple)
            .expect_err("current one-output gate remains")
            .path(),
        "pattern.output_layers.capability",
    );
}
