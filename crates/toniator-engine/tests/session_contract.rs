use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelSourceMapping,
    ChannelState, ColorValue, CoveragePolicy, DensityMetric2D, Document, DocumentCommand,
    DocumentId, DocumentSession, InvalidationLevel, MarkGeometryResponse, PatternDefinition,
    PatternDefinitionId, PatternMechanismId, PatternOutputLayerId, Revision, SourceComponent,
    SourcePlacement,
};

const CHANNEL_ID: ChannelId = ChannelId(1);

fn session() -> DocumentSession {
    let document = Document::new(
        DocumentId(1),
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        vec![PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "minimal",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        )],
        vec![ChannelState {
            id: CHANNEL_ID,
            pattern_definition_id: PatternDefinitionId(1),
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
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                opacity: 0.75,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_fill: 2.0,
                maximum_fill: 9.0,
                rotation_offset_degrees: 0.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )
    .expect("valid fixture");
    DocumentSession::new(document).expect("valid session")
}

#[test]
fn successful_commands_mutate_once_and_advance_revision_once() {
    let mut session = session();
    let commands = [
        (
            DocumentCommand::SetDensityAxis {
                channel_id: CHANNEL_ID,
                edited_axis: toniator_domain::DensityEditedAxis::AcrossX,
                value: 70.0,
            },
            InvalidationLevel::Family,
        ),
        (
            DocumentCommand::SetRotation {
                channel_id: CHANNEL_ID,
                rotation_degrees: 20.0,
            },
            InvalidationLevel::Family,
        ),
        (
            DocumentCommand::SetTranslationAxis {
                channel_id: CHANNEL_ID,
                edited_axis: toniator_domain::TranslationEditedAxis::X,
                value: 2.0,
            },
            InvalidationLevel::Family,
        ),
        (
            DocumentCommand::SetMarkGeometryField {
                channel_id: CHANNEL_ID,
                edit: toniator_domain::MarkGeometryFieldEdit::MaximumFill(8.5),
            },
            InvalidationLevel::Realization,
        ),
        (
            DocumentCommand::SetColorComponent {
                channel_id: CHANNEL_ID,
                component: toniator_domain::ColorComponent::Blue,
                value: 0.6,
            },
            InvalidationLevel::Presentation,
        ),
        (
            DocumentCommand::SetOpacity {
                channel_id: CHANNEL_ID,
                opacity: 0.4,
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

    for (index, (command, invalidation)) in commands.iter().enumerate() {
        let result = session.apply(command).expect("valid command");
        assert_eq!(result.invalidation, *invalidation);
        assert_eq!(result.affected_channels, vec![CHANNEL_ID]);
        assert_eq!(session.revision(), Revision((index + 1) as u64));
    }

    let channel = session
        .document()
        .channel(CHANNEL_ID)
        .expect("channel exists");
    assert_eq!(channel.layout.density.across_x, 70.0);
    assert_eq!(channel.layout.rotation_degrees, 20.0);
    assert_eq!(channel.layout.translation_x, 2.0);
    assert_eq!(channel.layout.translation_y, 0.0);
    assert_eq!(channel.mark_geometry_response.maximum_fill, 8.5);
    assert_eq!(channel.appearance.color.blue, 0.6);
    assert_eq!(channel.appearance.opacity, 0.4);
    assert!(!channel.appearance.visible);
}

#[test]
fn failed_commands_preserve_exact_document_and_revision() {
    let mut session = session();
    let original_document = session.snapshot();
    let original_revision = session.revision();

    for command in [
        DocumentCommand::SetDensityAxis {
            channel_id: CHANNEL_ID,
            edited_axis: toniator_domain::DensityEditedAxis::AcrossX,
            value: 0.0,
        },
        DocumentCommand::SetRotation {
            channel_id: CHANNEL_ID,
            rotation_degrees: f64::NAN,
        },
        DocumentCommand::SetTranslationAxis {
            channel_id: CHANNEL_ID,
            edited_axis: toniator_domain::TranslationEditedAxis::X,
            value: f64::INFINITY,
        },
        DocumentCommand::SetMarkGeometryField {
            channel_id: CHANNEL_ID,
            edit: toniator_domain::MarkGeometryFieldEdit::MinimumFill(-1.0),
        },
        DocumentCommand::SetColorComponent {
            channel_id: CHANNEL_ID,
            component: toniator_domain::ColorComponent::Red,
            value: 1.1,
        },
        DocumentCommand::SetOpacity {
            channel_id: CHANNEL_ID,
            opacity: f64::NAN,
        },
        DocumentCommand::SetVisibility {
            channel_id: ChannelId(99),
            visible: false,
        },
    ] {
        assert!(
            session.apply(&command).is_err(),
            "invalid command must fail"
        );
        assert_eq!(session.snapshot(), original_document);
        assert_eq!(session.revision(), original_revision);
    }
}

#[test]
fn current_results_are_accepted_and_stale_results_are_rejected() {
    let mut session = session();
    let current = session
        .evaluation_token(CHANNEL_ID)
        .expect("channel exists");
    assert!(session.accepts_evaluation(current));

    session
        .apply(&DocumentCommand::SetRotation {
            channel_id: CHANNEL_ID,
            rotation_degrees: 15.0,
        })
        .expect("valid edit");
    assert!(!session.accepts_evaluation(current));

    let replacement = session
        .evaluation_token(CHANNEL_ID)
        .expect("channel exists");
    assert!(session.accepts_evaluation(replacement));
}
