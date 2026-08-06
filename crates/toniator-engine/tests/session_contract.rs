use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelState, ColorValue,
    DensityMetric2D, Document, DocumentCommand, DocumentId, InvalidationLevel,
    MarkGeometryResponse, PatternDefinition, PatternDefinitionId, Revision,
};
use toniator_engine::DocumentSession;

const CHANNEL_ID: ChannelId = ChannelId(1);

fn session() -> DocumentSession {
    let document = Document::new(
        DocumentId(1),
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        vec![PatternDefinition {
            id: PatternDefinitionId(1),
            name: "minimal".to_owned(),
        }],
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
                minimum_size: 0.0,
                maximum_size: 1.0,
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
            DocumentCommand::SetDensity {
                channel_id: CHANNEL_ID,
                density: DensityMetric2D {
                    across_x: 70.0,
                    across_y: 40.0,
                    aspect_locked: false,
                },
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
            DocumentCommand::SetTranslation {
                channel_id: CHANNEL_ID,
                translation_x: 2.0,
                translation_y: -4.0,
            },
            InvalidationLevel::Family,
        ),
        (
            DocumentCommand::SetMarkGeometryResponse {
                channel_id: CHANNEL_ID,
                response: MarkGeometryResponse {
                    minimum_size: 0.2,
                    maximum_size: 1.2,
                },
            },
            InvalidationLevel::Realization,
        ),
        (
            DocumentCommand::SetColor {
                channel_id: CHANNEL_ID,
                color: ColorValue {
                    red: 0.2,
                    green: 0.4,
                    blue: 0.6,
                    alpha: 0.8,
                },
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
    assert_eq!(channel.layout.translation_y, -4.0);
    assert_eq!(channel.mark_geometry_response.maximum_size, 1.2);
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
        DocumentCommand::SetDensity {
            channel_id: CHANNEL_ID,
            density: DensityMetric2D {
                across_x: 0.0,
                across_y: 60.0,
                aspect_locked: false,
            },
        },
        DocumentCommand::SetRotation {
            channel_id: CHANNEL_ID,
            rotation_degrees: f64::NAN,
        },
        DocumentCommand::SetTranslation {
            channel_id: CHANNEL_ID,
            translation_x: f64::INFINITY,
            translation_y: 0.0,
        },
        DocumentCommand::SetMarkGeometryResponse {
            channel_id: CHANNEL_ID,
            response: MarkGeometryResponse {
                minimum_size: -1.0,
                maximum_size: 1.0,
            },
        },
        DocumentCommand::SetColor {
            channel_id: CHANNEL_ID,
            color: ColorValue {
                red: 1.1,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
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
