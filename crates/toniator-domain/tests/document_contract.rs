use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPaint, ChannelPatternLayout,
    ChannelSourceMapping, ChannelState, ChannelTopology, ChannelTopologyTemplate, ColorValue,
    DensityMetric2D, Document, DocumentCommand, DocumentEvaluationToken, DocumentHistory,
    DocumentId, DocumentSession, HalftoneChannelModel, HalftoneChannelRole, InvalidationLevel,
    MarkGeometryResponse, ModeledChannelState, PatternDefinition, PatternDefinitionId,
    PatternOutput, PatternStructure, SourceComponent, SourceMapping, SourceMappingComponent,
    SourcePlacement, SourceReference, SourceReferenceId,
};

const CHANNEL_ID: ChannelId = ChannelId(7);
const PATTERN_ID: PatternDefinitionId = PatternDefinitionId(3);

fn canvas() -> CanvasSpec {
    CanvasSpec {
        width: 900.0,
        height: 600.0,
    }
}

fn channel() -> ChannelState {
    ChannelState {
        id: CHANNEL_ID,
        pattern_definition_id: PATTERN_ID,
        layout: ChannelPatternLayout {
            density: DensityMetric2D {
                across_x: 90.0,
                across_y: 60.0,
                aspect_locked: true,
            },
            rotation_degrees: 0.0,
            translation_x: 0.0,
            translation_y: 0.0,
        },
        appearance: ChannelAppearance {
            visible: true,
            color: ColorValue {
                red: 0.1,
                green: 0.2,
                blue: 0.3,
                alpha: 1.0,
            },
            opacity: 0.75,
        },
        mark_geometry_response: MarkGeometryResponse {
            minimum_size: 2.0,
            maximum_size: 9.0,
        },
        source_mapping: ChannelSourceMapping {
            component: SourceComponent::Luminance,
            placement: SourcePlacement::StretchToCanvas,
        },
    }
}

fn definition() -> PatternDefinition {
    PatternDefinition {
        id: PATTERN_ID,
        name: "minimal".to_owned(),
        structure: PatternStructure::StraightGrid,
        output: PatternOutput::CircularMarks,
        guard_steps: 2,
        maximum_support_radius: 4.5,
    }
}

fn document_with(
    canvas: CanvasSpec,
    definitions: Vec<PatternDefinition>,
    channels: Vec<ChannelState>,
) -> Result<Document, toniator_domain::ValidationError> {
    Document::new(DocumentId(1), canvas, definitions, channels)
}

fn valid_document() -> Document {
    document_with(canvas(), vec![definition()], vec![channel()]).expect("valid fixture")
}

#[test]
fn default_document_factory_builds_the_accepted_modeled_document_at_revision_zero() {
    let source = SourceReference::Assigned(SourceReferenceId::new("content-derived-id").unwrap());
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 620.0,
        },
        source.clone(),
    )
    .unwrap();
    assert_eq!(document.source(), &source);
    assert_eq!(document.channel_model(), Some(HalftoneChannelModel::Rgb));
    assert_eq!(document.pattern_definitions().len(), 1);
    let definition = &document.pattern_definitions()[0];
    assert_eq!(definition.structure, PatternStructure::StraightGrid);
    assert_eq!(definition.output, PatternOutput::CircularMarks);
    assert_eq!(definition.guard_steps, 2);
    assert_eq!(definition.maximum_support_radius, 4.5);
    for channel in document.channel_topology().unwrap().channels() {
        assert_eq!(channel.layout.density.across_x, 102.4);
        assert_eq!(channel.layout.density.across_y, 62.0);
        assert!(channel.layout.density.aspect_locked);
        assert_eq!(channel.mark_geometry_response.minimum_size, 2.0);
        assert_eq!(channel.mark_geometry_response.maximum_size, 9.0);
    }
    let history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    assert_eq!(history.revision().0, 0);
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

fn topology_template() -> ChannelTopologyTemplate {
    ChannelTopologyTemplate {
        pattern_definition_id: PATTERN_ID,
        layout: channel().layout,
        mark_geometry_response: channel().mark_geometry_response,
    }
}

fn modeled_channel(
    role: HalftoneChannelRole,
    id: ChannelId,
    mapping: SourceMapping,
    paint: ChannelPaint,
) -> ModeledChannelState {
    let template = topology_template();
    ModeledChannelState {
        role,
        id,
        pattern_definition_id: template.pattern_definition_id,
        layout: template.layout,
        mark_geometry_response: template.mark_geometry_response,
        mapping,
        paint,
        visible: true,
        opacity: 1.0,
    }
}

fn solid(red: f64, green: f64, blue: f64) -> ChannelPaint {
    ChannelPaint::Solid(ColorValue {
        red,
        green,
        blue,
        alpha: 1.0,
    })
}

fn explicit_rgb(ids: [ChannelId; 3]) -> ChannelTopology {
    ChannelTopology::new(vec![
        modeled_channel(
            HalftoneChannelRole::Red,
            ids[0],
            SourceMapping::canonical(SourceMappingComponent::Red),
            solid(1.0, 0.0, 0.0),
        ),
        modeled_channel(
            HalftoneChannelRole::Green,
            ids[1],
            SourceMapping::canonical(SourceMappingComponent::Green),
            solid(0.0, 1.0, 0.0),
        ),
        modeled_channel(
            HalftoneChannelRole::Blue,
            ids[2],
            SourceMapping::canonical(SourceMappingComponent::Blue),
            solid(0.0, 0.0, 1.0),
        ),
    ])
}

fn explicit_cmyk(ids: [ChannelId; 4]) -> ChannelTopology {
    ChannelTopology::new(vec![
        modeled_channel(
            HalftoneChannelRole::Cyan,
            ids[0],
            SourceMapping::canonical(SourceMappingComponent::Cyan),
            solid(0.0, 1.0, 1.0),
        ),
        modeled_channel(
            HalftoneChannelRole::Magenta,
            ids[1],
            SourceMapping::canonical(SourceMappingComponent::Magenta),
            solid(1.0, 0.0, 1.0),
        ),
        modeled_channel(
            HalftoneChannelRole::Yellow,
            ids[2],
            SourceMapping::canonical(SourceMappingComponent::Yellow),
            solid(1.0, 1.0, 0.0),
        ),
        modeled_channel(
            HalftoneChannelRole::Black,
            ids[3],
            SourceMapping::canonical(SourceMappingComponent::Black),
            solid(0.0, 0.0, 0.0),
        ),
    ])
}

fn assert_path(result: Result<Document, toniator_domain::ValidationError>, expected_path: &str) {
    assert_eq!(result.expect_err("must be invalid").path(), expected_path);
}

#[test]
fn accepts_the_required_900_by_600_density_document() {
    let document = valid_document();

    assert_eq!(document.canvas().width, 900.0);
    assert_eq!(document.canvas().height, 600.0);
    assert_eq!(
        document.channels().expect("legacy channels")[0]
            .layout
            .density
            .across_x,
        90.0
    );
    assert_eq!(
        document.channels().expect("legacy channels")[0]
            .layout
            .density
            .across_y,
        60.0
    );
}

#[test]
fn rejects_every_stage_two_document_validation_rule() {
    for (candidate, path) in [
        (
            document_with(
                CanvasSpec {
                    width: f64::NAN,
                    height: 600.0,
                },
                vec![definition()],
                vec![channel()],
            ),
            "canvas.width",
        ),
        (
            document_with(
                CanvasSpec {
                    width: 0.0,
                    height: 600.0,
                },
                vec![definition()],
                vec![channel()],
            ),
            "canvas.width",
        ),
        (
            document_with(
                CanvasSpec {
                    width: -1.0,
                    height: 600.0,
                },
                vec![definition()],
                vec![channel()],
            ),
            "canvas.width",
        ),
        (
            document_with(
                CanvasSpec {
                    width: 900.0,
                    height: 0.0,
                },
                vec![definition()],
                vec![channel()],
            ),
            "canvas.height",
        ),
        (
            document_with(
                CanvasSpec {
                    width: 900.0,
                    height: -1.0,
                },
                vec![definition()],
                vec![channel()],
            ),
            "canvas.height",
        ),
    ] {
        assert_path(candidate, path);
    }

    for (invalid_channel, path) in [
        (
            {
                let mut value = channel();
                value.layout.density.across_x = 0.0;
                value
            },
            "channel.pattern.layout.density.across_x",
        ),
        (
            {
                let mut value = channel();
                value.layout.density.across_x = -1.0;
                value
            },
            "channel.pattern.layout.density.across_x",
        ),
        (
            {
                let mut value = channel();
                value.layout.density.across_y = 0.0;
                value
            },
            "channel.pattern.layout.density.across_y",
        ),
        (
            {
                let mut value = channel();
                value.layout.density.across_y = -1.0;
                value
            },
            "channel.pattern.layout.density.across_y",
        ),
        (
            {
                let mut value = channel();
                value.layout.density.across_y = f64::INFINITY;
                value
            },
            "channel.pattern.layout.density.across_y",
        ),
        (
            {
                let mut value = channel();
                value.layout.rotation_degrees = f64::NAN;
                value
            },
            "channel.pattern.layout.rotation_degrees",
        ),
        (
            {
                let mut value = channel();
                value.layout.translation_x = f64::INFINITY;
                value
            },
            "channel.pattern.layout.translation_x",
        ),
        (
            {
                let mut value = channel();
                value.layout.translation_y = f64::NAN;
                value
            },
            "channel.pattern.layout.translation_y",
        ),
        (
            {
                let mut value = channel();
                value.appearance.color.red = 1.1;
                value
            },
            "channel.appearance.color.red",
        ),
        (
            {
                let mut value = channel();
                value.appearance.color.green = -0.1;
                value
            },
            "channel.appearance.color.green",
        ),
        (
            {
                let mut value = channel();
                value.appearance.color.blue = f64::NAN;
                value
            },
            "channel.appearance.color.blue",
        ),
        (
            {
                let mut value = channel();
                value.appearance.color.alpha = f64::INFINITY;
                value
            },
            "channel.appearance.color.alpha",
        ),
        (
            {
                let mut value = channel();
                value.appearance.opacity = -0.1;
                value
            },
            "channel.appearance.opacity",
        ),
        (
            {
                let mut value = channel();
                value.appearance.opacity = f64::NAN;
                value
            },
            "channel.appearance.opacity",
        ),
        (
            {
                let mut value = channel();
                value.mark_geometry_response.minimum_size = -0.1;
                value
            },
            "channel.pattern.mark_geometry_response.minimum_size",
        ),
        (
            {
                let mut value = channel();
                value.mark_geometry_response.maximum_size = f64::NAN;
                value
            },
            "channel.pattern.mark_geometry_response.maximum_size",
        ),
        (
            {
                let mut value = channel();
                value.mark_geometry_response.maximum_size = -0.1;
                value
            },
            "channel.pattern.mark_geometry_response.maximum_size",
        ),
        (
            {
                let mut value = channel();
                value.mark_geometry_response.minimum_size = 9.0;
                value.mark_geometry_response.maximum_size = 2.0;
                value
            },
            "channel.pattern.mark_geometry_response",
        ),
    ] {
        assert_path(
            document_with(canvas(), vec![definition()], vec![invalid_channel]),
            path,
        );
    }

    let mut missing_reference = channel();
    missing_reference.pattern_definition_id = PatternDefinitionId(99);
    assert_path(
        document_with(canvas(), vec![definition()], vec![missing_reference]),
        "channel.pattern.definition_id",
    );
    assert_path(
        document_with(canvas(), vec![definition(), definition()], vec![channel()]),
        "pattern_definitions",
    );
    assert_path(
        document_with(canvas(), vec![definition()], vec![channel(), channel()]),
        "channels",
    );
}

#[test]
fn commands_return_the_required_invalidation_and_affected_channel() {
    let cases = vec![
        (
            DocumentCommand::SetDensity {
                channel_id: CHANNEL_ID,
                density: DensityMetric2D {
                    across_x: 80.0,
                    across_y: 50.0,
                    aspect_locked: false,
                },
            },
            InvalidationLevel::Family,
        ),
        (
            DocumentCommand::SetRotation {
                channel_id: CHANNEL_ID,
                rotation_degrees: 15.0,
            },
            InvalidationLevel::Family,
        ),
        (
            DocumentCommand::SetTranslation {
                channel_id: CHANNEL_ID,
                translation_x: 2.0,
                translation_y: -3.0,
            },
            InvalidationLevel::Family,
        ),
        (
            DocumentCommand::SetMarkGeometryResponse {
                channel_id: CHANNEL_ID,
                response: MarkGeometryResponse {
                    minimum_size: 2.0,
                    maximum_size: 8.5,
                },
            },
            InvalidationLevel::Realization,
        ),
        (
            DocumentCommand::SetSourceMapping {
                channel_id: CHANNEL_ID,
                mapping: ChannelSourceMapping {
                    component: SourceComponent::Alpha,
                    placement: SourcePlacement::StretchToCanvas,
                },
            },
            InvalidationLevel::Realization,
        ),
        (
            DocumentCommand::SetColor {
                channel_id: CHANNEL_ID,
                color: ColorValue {
                    red: 0.9,
                    green: 0.8,
                    blue: 0.7,
                    alpha: 0.6,
                },
            },
            InvalidationLevel::Presentation,
        ),
        (
            DocumentCommand::SetOpacity {
                channel_id: CHANNEL_ID,
                opacity: 0.5,
            },
            InvalidationLevel::Presentation,
        ),
        (
            DocumentCommand::SetVisibility {
                channel_id: CHANNEL_ID,
                visible: false,
            },
            InvalidationLevel::Presentation,
        ),
    ];

    for (command, expected_invalidation) in cases {
        let document = valid_document();
        let original = document.clone();
        let (candidate, result) = document.apply_command(&command).expect("valid command");
        assert_eq!(
            document, original,
            "a domain transition must not mutate its input"
        );
        assert_eq!(result.affected_channels, vec![CHANNEL_ID]);
        assert_eq!(result.invalidation, expected_invalidation);
        assert_ne!(
            candidate, original,
            "each command must change its candidate"
        );
    }
}

#[test]
fn stage_six_source_reference_snapshot_and_diameter_contracts_are_authoritative() {
    assert_eq!(
        SourceReferenceId::new("")
            .expect_err("empty IDs fail")
            .path(),
        "source.reference_id"
    );
    assert_eq!(
        SourceReferenceId::new("/tmp/source.png")
            .expect_err("paths are not IDs")
            .path(),
        "source.reference_id"
    );

    let source_id = SourceReferenceId::new("stage6-input").unwrap();
    let mut session = DocumentSession::new(valid_document()).unwrap();
    let result = session
        .apply(&DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(source_id.clone()),
        })
        .unwrap();
    assert_eq!(result.invalidation, InvalidationLevel::Source);
    assert_eq!(result.affected_channels, vec![CHANNEL_ID]);

    let snapshot = session.evaluation_snapshot(CHANNEL_ID).unwrap();
    assert_eq!(snapshot.token().channel_id(), CHANNEL_ID);
    assert_eq!(snapshot.token().revision(), session.revision());
    assert_eq!(
        snapshot.document().source(),
        &SourceReference::Assigned(source_id)
    );
    assert_eq!(
        session
            .evaluation_snapshot(ChannelId(999))
            .expect_err("missing channel fails at snapshot boundary")
            .path(),
        "evaluation.channel_id"
    );

    let mut below_baseline = channel();
    below_baseline.mark_geometry_response.minimum_size = 0.25;
    below_baseline.mark_geometry_response.maximum_size = 1.0;
    assert!(document_with(canvas(), vec![definition()], vec![below_baseline]).is_ok());
    let mut too_large = channel();
    too_large.mark_geometry_response.maximum_size = 9.01;
    assert_path(
        document_with(canvas(), vec![definition()], vec![too_large]),
        "channel.pattern.mark_geometry_response.maximum_size",
    );
    let mut larger_definition = definition();
    larger_definition.maximum_support_radius = 6.0;
    let mut above_baseline = channel();
    above_baseline.mark_geometry_response.minimum_size = 10.0;
    above_baseline.mark_geometry_response.maximum_size = 12.0;
    assert!(document_with(canvas(), vec![larger_definition], vec![above_baseline]).is_ok());

    let mut invalid_capability = definition();
    invalid_capability.maximum_support_radius = f64::NAN;
    assert_path(
        document_with(canvas(), vec![invalid_capability], vec![channel()]),
        "pattern_definitions.maximum_support_radius",
    );

    let mut unsupported = definition();
    unsupported.structure = PatternStructure::Unsupported;
    assert_path(
        document_with(canvas(), vec![unsupported], vec![channel()]),
        "pattern_definitions.structure",
    );
}

#[test]
fn retained_legacy_channel_snapshot_and_token_stay_current_then_stale_after_mutation() {
    let mut session = DocumentSession::new(valid_document()).expect("valid session");
    let snapshot = session
        .evaluation_snapshot(CHANNEL_ID)
        .expect("legacy channel snapshot");
    let token = snapshot.token();

    assert_eq!(snapshot.document(), session.document());
    assert_eq!(token.channel_id(), CHANNEL_ID);
    assert_eq!(token.revision(), session.revision());
    assert!(session.accepts_evaluation(token));

    session
        .apply(&DocumentCommand::SetVisibility {
            channel_id: CHANNEL_ID,
            visible: false,
        })
        .expect("valid legacy mutation");

    assert!(!session.accepts_evaluation(token));
    let current = session
        .evaluation_snapshot(CHANNEL_ID)
        .expect("current legacy snapshot");
    assert_eq!(current.token().channel_id(), CHANNEL_ID);
    assert_eq!(current.token().revision(), session.revision());
    assert!(session.accepts_evaluation(current.token()));
}

#[test]
fn commands_reject_missing_channels_and_nonfinite_transforms_before_mutation() {
    let document = valid_document();
    let original = document.clone();
    let error = document
        .apply_command(&DocumentCommand::SetRotation {
            channel_id: ChannelId(999),
            rotation_degrees: 1.0,
        })
        .expect_err("missing channel must fail");
    assert_eq!(error.path(), "command.channel_id");
    assert_eq!(document, original);

    for command in [
        DocumentCommand::SetRotation {
            channel_id: CHANNEL_ID,
            rotation_degrees: f64::NAN,
        },
        DocumentCommand::SetTranslation {
            channel_id: CHANNEL_ID,
            translation_x: f64::INFINITY,
            translation_y: 0.0,
        },
    ] {
        let document = valid_document();
        let original = document.clone();
        assert!(document.apply_command(&command).is_err());
        assert_eq!(document, original);
    }
}

#[test]
fn canonical_topologies_have_exact_roles_ids_mappings_paints_and_cloned_templates() {
    let document = valid_document();
    let template = topology_template();
    let cases = [
        (
            HalftoneChannelModel::Rgb,
            vec![
                (
                    HalftoneChannelRole::Red,
                    ChannelId(1),
                    SourceMappingComponent::Red,
                    solid(1.0, 0.0, 0.0),
                ),
                (
                    HalftoneChannelRole::Green,
                    ChannelId(2),
                    SourceMappingComponent::Green,
                    solid(0.0, 1.0, 0.0),
                ),
                (
                    HalftoneChannelRole::Blue,
                    ChannelId(3),
                    SourceMappingComponent::Blue,
                    solid(0.0, 0.0, 1.0),
                ),
            ],
        ),
        (
            HalftoneChannelModel::Cmyk,
            vec![
                (
                    HalftoneChannelRole::Cyan,
                    ChannelId(4),
                    SourceMappingComponent::Cyan,
                    solid(0.0, 1.0, 1.0),
                ),
                (
                    HalftoneChannelRole::Magenta,
                    ChannelId(5),
                    SourceMappingComponent::Magenta,
                    solid(1.0, 0.0, 1.0),
                ),
                (
                    HalftoneChannelRole::Yellow,
                    ChannelId(6),
                    SourceMappingComponent::Yellow,
                    solid(1.0, 1.0, 0.0),
                ),
                (
                    HalftoneChannelRole::Black,
                    ChannelId(7),
                    SourceMappingComponent::Black,
                    solid(0.0, 0.0, 0.0),
                ),
            ],
        ),
        (
            HalftoneChannelModel::SourceColorAlpha,
            vec![(
                HalftoneChannelRole::SourceColor,
                ChannelId(8),
                SourceMappingComponent::Alpha,
                ChannelPaint::SampledSource,
            )],
        ),
    ];

    for (model, expected) in cases {
        let topology = document
            .canonical_channel_topology(model, template.clone())
            .expect("valid canonical topology");
        assert_eq!(topology.channels().len(), expected.len());
        for (channel, (role, id, component, paint)) in topology.channels().iter().zip(expected) {
            assert_eq!(channel.role, role);
            assert_eq!(channel.id, id);
            assert_eq!(channel.mapping, SourceMapping::canonical(component));
            assert_eq!(channel.paint, paint);
            assert_eq!(
                channel.pattern_definition_id,
                template.pattern_definition_id
            );
            assert_eq!(channel.layout, template.layout);
            assert_eq!(
                channel.mark_geometry_response,
                template.mark_geometry_response
            );
            assert!(channel.visible);
            assert_eq!(channel.opacity, 1.0);
        }
    }
}

#[test]
fn explicit_topology_accepts_arbitrary_ids_and_installs_the_actual_document_channels() {
    let mut session = DocumentSession::new(valid_document()).expect("valid session");
    let topology = explicit_rgb([ChannelId(41), CHANNEL_ID, ChannelId(99)]);
    let result = session
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Rgb,
            topology: topology.clone(),
        })
        .expect("valid replacement");

    assert_eq!(result.invalidation, InvalidationLevel::ChannelTopology);
    assert_eq!(
        result.affected_channels,
        vec![CHANNEL_ID, ChannelId(41), ChannelId(99)]
    );
    assert_eq!(session.revision(), toniator_domain::Revision(1));
    assert_eq!(
        session.document().channel_model(),
        Some(HalftoneChannelModel::Rgb)
    );
    assert_eq!(session.document().channel_topology(), Some(&topology));
    assert!(session.document().channels().is_none());
    assert_eq!(
        session
            .document()
            .channel_topology()
            .expect("modeled topology")
            .channels()
            .iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>(),
        vec![ChannelId(41), CHANNEL_ID, ChannelId(99)]
    );
}

#[test]
fn topology_validation_rejects_missing_duplicate_extraneous_out_of_order_and_duplicate_ids() {
    let invalid_topologies = [
        ChannelTopology::new(
            explicit_rgb([ChannelId(1), ChannelId(2), ChannelId(3)]).channels()[..2].to_vec(),
        ),
        ChannelTopology::new(vec![
            modeled_channel(
                HalftoneChannelRole::Red,
                ChannelId(1),
                SourceMapping::canonical(SourceMappingComponent::Red),
                solid(1.0, 0.0, 0.0),
            ),
            modeled_channel(
                HalftoneChannelRole::Red,
                ChannelId(2),
                SourceMapping::canonical(SourceMappingComponent::Red),
                solid(1.0, 0.0, 0.0),
            ),
            modeled_channel(
                HalftoneChannelRole::Blue,
                ChannelId(3),
                SourceMapping::canonical(SourceMappingComponent::Blue),
                solid(0.0, 0.0, 1.0),
            ),
        ]),
        ChannelTopology::new(vec![
            modeled_channel(
                HalftoneChannelRole::Red,
                ChannelId(1),
                SourceMapping::canonical(SourceMappingComponent::Red),
                solid(1.0, 0.0, 0.0),
            ),
            modeled_channel(
                HalftoneChannelRole::Cyan,
                ChannelId(2),
                SourceMapping::canonical(SourceMappingComponent::Cyan),
                solid(0.0, 1.0, 1.0),
            ),
            modeled_channel(
                HalftoneChannelRole::Blue,
                ChannelId(3),
                SourceMapping::canonical(SourceMappingComponent::Blue),
                solid(0.0, 0.0, 1.0),
            ),
        ]),
        ChannelTopology::new(vec![
            modeled_channel(
                HalftoneChannelRole::Green,
                ChannelId(1),
                SourceMapping::canonical(SourceMappingComponent::Green),
                solid(0.0, 1.0, 0.0),
            ),
            modeled_channel(
                HalftoneChannelRole::Red,
                ChannelId(2),
                SourceMapping::canonical(SourceMappingComponent::Red),
                solid(1.0, 0.0, 0.0),
            ),
            modeled_channel(
                HalftoneChannelRole::Blue,
                ChannelId(3),
                SourceMapping::canonical(SourceMappingComponent::Blue),
                solid(0.0, 0.0, 1.0),
            ),
        ]),
        explicit_rgb([ChannelId(1), ChannelId(1), ChannelId(3)]),
    ];

    for topology in invalid_topologies {
        let mut session = DocumentSession::new(valid_document()).expect("valid session");
        let before = session.snapshot();
        assert!(
            session
                .apply(&DocumentCommand::ReplaceChannelTopology {
                    model: HalftoneChannelModel::Rgb,
                    topology,
                })
                .is_err()
        );
        assert_eq!(session.snapshot(), before);
        assert_eq!(session.revision(), toniator_domain::Revision(0));
    }
}

#[test]
fn atomic_replacement_rejects_every_invalid_modeled_state_without_mutation() {
    let mut invalid_cases = Vec::new();

    let mut channels = explicit_rgb([ChannelId(10), ChannelId(11), ChannelId(12)])
        .channels()
        .to_vec();
    channels[0].pattern_definition_id = PatternDefinitionId(999);
    invalid_cases.push(ChannelTopology::new(channels));

    let mut channels = explicit_rgb([ChannelId(10), ChannelId(11), ChannelId(12)])
        .channels()
        .to_vec();
    channels[0].layout.density.across_x = 0.0;
    invalid_cases.push(ChannelTopology::new(channels));

    let mut channels = explicit_rgb([ChannelId(10), ChannelId(11), ChannelId(12)])
        .channels()
        .to_vec();
    channels[0].mark_geometry_response.maximum_size = 9.1;
    invalid_cases.push(ChannelTopology::new(channels));

    let mut channels = explicit_rgb([ChannelId(10), ChannelId(11), ChannelId(12)])
        .channels()
        .to_vec();
    channels[0].opacity = 1.1;
    invalid_cases.push(ChannelTopology::new(channels));

    let mut channels = explicit_rgb([ChannelId(10), ChannelId(11), ChannelId(12)])
        .channels()
        .to_vec();
    channels[0].paint = solid(1.1, 0.0, 0.0);
    invalid_cases.push(ChannelTopology::new(channels));

    let mut channels = explicit_rgb([ChannelId(10), ChannelId(11), ChannelId(12)])
        .channels()
        .to_vec();
    channels[0].mapping.gain = -0.1;
    invalid_cases.push(ChannelTopology::new(channels));

    let mut channels = explicit_rgb([ChannelId(10), ChannelId(11), ChannelId(12)])
        .channels()
        .to_vec();
    channels[0].mapping.bias = f64::NAN;
    invalid_cases.push(ChannelTopology::new(channels));

    let mut channels = explicit_rgb([ChannelId(10), ChannelId(11), ChannelId(12)])
        .channels()
        .to_vec();
    channels[0].paint = ChannelPaint::SampledSource;
    invalid_cases.push(ChannelTopology::new(channels));

    for topology in invalid_cases {
        let mut session = DocumentSession::new(valid_document()).expect("valid session");
        let before = session.snapshot();
        assert!(
            session
                .apply(&DocumentCommand::ReplaceChannelTopology {
                    model: HalftoneChannelModel::Rgb,
                    topology,
                })
                .is_err()
        );
        assert_eq!(session.snapshot(), before);
        assert_eq!(session.revision(), toniator_domain::Revision(0));
    }
}

#[test]
fn modeled_replacement_reports_old_then_new_ids_and_refuses_legacy_evaluation_access() {
    let mut session = DocumentSession::new(valid_document()).expect("valid session");
    session
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Rgb,
            topology: explicit_rgb([ChannelId(30), ChannelId(20), ChannelId(10)]),
        })
        .expect("first modeled topology");
    let result = session
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Cmyk,
            topology: explicit_cmyk([ChannelId(20), ChannelId(40), ChannelId(10), ChannelId(60)]),
        })
        .expect("second modeled topology");
    assert_eq!(
        result.affected_channels,
        vec![
            ChannelId(30),
            ChannelId(20),
            ChannelId(10),
            ChannelId(40),
            ChannelId(60)
        ]
    );
    assert!(session.document().channels().is_none());
    assert!(session.document().channel(ChannelId(20)).is_none());
    assert_eq!(
        session
            .evaluation_snapshot(ChannelId(20))
            .expect_err("Stage 9A must not offer modeled legacy evaluation")
            .path(),
        "evaluation.channel_topology"
    );
}

#[test]
fn complete_document_snapshots_support_every_modeled_topology() {
    for model in [
        HalftoneChannelModel::Rgb,
        HalftoneChannelModel::Cmyk,
        HalftoneChannelModel::SourceColorAlpha,
    ] {
        let mut session = DocumentSession::new(valid_document()).expect("valid session");
        let topology = session
            .document()
            .canonical_channel_topology(model, topology_template())
            .expect("valid canonical topology");
        session
            .apply(&DocumentCommand::ReplaceChannelTopology { model, topology })
            .expect("valid modeled replacement");

        let snapshot = session.document_evaluation_snapshot();
        assert_eq!(snapshot.document(), session.document());
        assert_eq!(snapshot.document().channel_model(), Some(model));
        assert_eq!(snapshot.token().document_id(), session.document().id());
        assert_eq!(snapshot.token().revision(), session.revision());
        assert!(session.accepts_document_evaluation(snapshot.token()));
    }
}

#[test]
fn complete_document_snapshot_atomically_retains_document_and_revision() {
    let mut session = DocumentSession::new(valid_document()).expect("valid session");
    let topology = session
        .document()
        .canonical_channel_topology(HalftoneChannelModel::Rgb, topology_template())
        .expect("valid canonical topology");
    session
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Rgb,
            topology,
        })
        .expect("valid modeled replacement");

    let snapshot = session.document_evaluation_snapshot();
    let token = snapshot.token();
    let captured_document = session.snapshot();
    assert_eq!(snapshot.document(), &captured_document);
    assert_eq!(token.revision(), session.revision());

    session
        .apply(&DocumentCommand::SetVisibility {
            channel_id: ChannelId(1),
            visible: false,
        })
        .expect("valid modeled mutation");

    assert_eq!(snapshot.document(), &captured_document);
    assert!(!session.accepts_document_evaluation(token));
    let current = session.document_evaluation_snapshot();
    assert_eq!(current.document(), session.document());
    assert_eq!(current.token().revision(), session.revision());
    assert!(session.accepts_document_evaluation(current.token()));
}

#[test]
fn complete_document_tokens_are_carryable_but_only_sessions_mint_current_tokens() {
    fn carry(_: DocumentEvaluationToken) {}
    fn assert_copy<T: Copy>() {}

    let session = DocumentSession::new(valid_document()).expect("valid session");
    let token = session.document_evaluation_token();
    assert_copy::<DocumentEvaluationToken>();
    carry(token);
    assert!(session.accepts_document_evaluation(token));

    let other_document =
        Document::new(DocumentId(2), canvas(), vec![definition()], vec![channel()])
            .expect("valid distinct document");
    let other_session = DocumentSession::new(other_document).expect("valid session");
    assert!(
        !other_session.accepts_document_evaluation(token),
        "a same-revision token from another document must not be accepted"
    );

    // `DocumentEvaluationToken` has no public constructor or fields. This
    // integration test can verify its externally carryable API shape; Rust
    // privacy prevents a downstream test crate from forging its contents.
}

#[test]
fn complete_mapping_validates_numeric_fields_and_uses_the_authoritative_transform() {
    let valid = SourceMapping {
        component: SourceMappingComponent::Luminance,
        placement: SourcePlacement::StretchToCanvas,
        inverted: true,
        gain: 1.5,
        bias: -0.25,
    };
    assert_eq!(valid.transform(0.0), 1.0);
    assert_eq!(valid.transform(0.5), 0.5);
    assert_eq!(valid.transform(1.0), 0.0);

    let mut session = DocumentSession::new(valid_document()).expect("valid session");
    session
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Rgb,
            topology: explicit_rgb([ChannelId(10), ChannelId(11), ChannelId(12)]),
        })
        .expect("valid replacement");
    for component in [
        SourceMappingComponent::Red,
        SourceMappingComponent::Green,
        SourceMappingComponent::Blue,
        SourceMappingComponent::Cyan,
        SourceMappingComponent::Magenta,
        SourceMappingComponent::Yellow,
        SourceMappingComponent::Black,
        SourceMappingComponent::Alpha,
        SourceMappingComponent::Luminance,
    ] {
        session
            .apply(&DocumentCommand::SetTopologySourceMapping {
                channel_id: ChannelId(10),
                mapping: SourceMapping::canonical(component),
            })
            .expect("closed component and placement are supported");
        let mapping = session
            .document()
            .modeled_channel(ChannelId(10))
            .expect("modeled channel")
            .mapping;
        assert_eq!(mapping.component, component);
        assert_eq!(mapping.placement, SourcePlacement::StretchToCanvas);
    }

    for mapping in [
        SourceMapping {
            gain: -0.1,
            ..valid
        },
        SourceMapping {
            gain: f64::NAN,
            ..valid
        },
        SourceMapping {
            bias: f64::INFINITY,
            ..valid
        },
    ] {
        let mut session = DocumentSession::new(valid_document()).expect("valid session");
        let topology = explicit_rgb([ChannelId(10), ChannelId(11), ChannelId(12)]);
        session
            .apply(&DocumentCommand::ReplaceChannelTopology {
                model: HalftoneChannelModel::Rgb,
                topology,
            })
            .expect("valid replacement");
        assert!(
            session
                .apply(&DocumentCommand::SetTopologySourceMapping {
                    channel_id: ChannelId(10),
                    mapping,
                })
                .is_err()
        );
    }
}

#[test]
fn paint_model_compatibility_and_per_channel_invalidation_are_enforced() {
    let mut session = DocumentSession::new(valid_document()).expect("valid session");
    session
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Rgb,
            topology: explicit_rgb([ChannelId(10), ChannelId(11), ChannelId(12)]),
        })
        .expect("valid replacement");

    assert!(
        session
            .apply(&DocumentCommand::SetChannelPaint {
                channel_id: ChannelId(10),
                paint: ChannelPaint::SampledSource,
            })
            .is_err()
    );
    assert_eq!(
        session
            .apply(&DocumentCommand::SetDensity {
                channel_id: ChannelId(10),
                density: DensityMetric2D {
                    across_x: 80.0,
                    across_y: 50.0,
                    aspect_locked: false,
                },
            })
            .expect("modeled layout edit")
            .invalidation,
        InvalidationLevel::Family
    );
    assert_eq!(
        session
            .apply(&DocumentCommand::SetTranslation {
                channel_id: ChannelId(10),
                translation_x: 3.0,
                translation_y: -2.0,
            })
            .expect("modeled layout edit")
            .invalidation,
        InvalidationLevel::Family
    );
    let layout = &session
        .document()
        .modeled_channel(ChannelId(10))
        .expect("modeled channel")
        .layout;
    assert_eq!(layout.density.across_x, 80.0);
    assert_eq!((layout.translation_x, layout.translation_y), (3.0, -2.0));
    assert_eq!(
        session
            .apply(&DocumentCommand::SetTopologySourceMapping {
                channel_id: ChannelId(10),
                mapping: SourceMapping::canonical(SourceMappingComponent::Alpha),
            })
            .expect("valid full mapping")
            .invalidation,
        InvalidationLevel::Realization
    );
    let mapped_channel = &session
        .document()
        .channel_topology()
        .expect("topology remains authoritative")
        .channels()[0];
    assert_eq!(
        mapped_channel.mapping.component,
        SourceMappingComponent::Alpha
    );
    assert_eq!(
        session
            .document()
            .modeled_channel(ChannelId(10))
            .expect("actual modeled topology channel")
            .mapping
            .component,
        SourceMappingComponent::Alpha
    );
    assert_eq!(
        session
            .apply(&DocumentCommand::SetMarkGeometryResponse {
                channel_id: ChannelId(10),
                response: MarkGeometryResponse {
                    minimum_size: 2.0,
                    maximum_size: 8.0
                },
            })
            .expect("valid response")
            .invalidation,
        InvalidationLevel::Realization
    );
    assert_eq!(
        session
            .document()
            .channel_topology()
            .expect("topology remains authoritative")
            .channels()[0]
            .mark_geometry_response
            .maximum_size,
        8.0
    );
    for command in [
        DocumentCommand::SetChannelPaint {
            channel_id: ChannelId(10),
            paint: solid(0.2, 0.3, 0.4),
        },
        DocumentCommand::SetOpacity {
            channel_id: ChannelId(10),
            opacity: 0.5,
        },
        DocumentCommand::SetVisibility {
            channel_id: ChannelId(10),
            visible: false,
        },
    ] {
        assert_eq!(
            session
                .apply(&command)
                .expect("valid presentation edit")
                .invalidation,
            InvalidationLevel::Presentation
        );
    }
    assert_eq!(
        session
            .apply(&DocumentCommand::SetColor {
                channel_id: ChannelId(10),
                color: ColorValue {
                    red: 0.6,
                    green: 0.5,
                    blue: 0.4,
                    alpha: 1.0,
                },
            })
            .expect("ordinary solid color")
            .invalidation,
        InvalidationLevel::Presentation
    );

    let source_topology = valid_document()
        .canonical_channel_topology(HalftoneChannelModel::SourceColorAlpha, topology_template())
        .expect("valid sampled-source topology");
    let mut source_session = DocumentSession::new(valid_document()).expect("valid session");
    source_session
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::SourceColorAlpha,
            topology: source_topology,
        })
        .expect("valid replacement");
    assert!(
        source_session
            .apply(&DocumentCommand::SetChannelPaint {
                channel_id: ChannelId(8),
                paint: solid(1.0, 1.0, 1.0),
            })
            .is_err()
    );
}
