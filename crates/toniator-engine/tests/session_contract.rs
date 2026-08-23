use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternInstance, ChannelPatternLayoutDelta,
    ChannelSourceMapping, ChannelState, ColorValue, CoveragePolicy, DensityEditedAxis,
    DensityMetric2D, Document, DocumentCommand, DocumentId, DocumentSession, InvalidationLevel,
    MarkGeometryFieldEdit, MarkGeometryResponse, PatternDefinition, PatternDefinitionId,
    PatternGeometryResponse, PatternMechanismId, PatternOutputLayerId, Revision, SourceComponent,
    SourcePlacement,
};

const CHANNEL_ID: ChannelId = ChannelId(1);

/// Builds one legacy-channel session whose family and realization inputs
/// inherit from document-owned pattern settings for command-contract tests.
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
        toniator_domain::DocumentPatternSettings {
            definition_id: PatternDefinitionId(1),
            density: DensityMetric2D {
                across_x: 90.0,
                across_y: 60.0,
                aspect_locked: true,
            },
            pattern_rotation_degrees: 0.0,
            shape_rotation_degrees: 0.0,
            geometry_response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.9,
            }),
        },
        vec![ChannelState {
            id: CHANNEL_ID,
            pattern_instance: ChannelPatternInstance {
                definition_override: None,
                layout_delta: ChannelPatternLayoutDelta {
                    density: None,
                    rotation_degrees: None,
                    translation_x: 0.0,
                    translation_y: 0.0,
                },
                shape_rotation_delta_degrees: None,
                geometry_response_delta: None,
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
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )
    .expect("valid fixture");
    DocumentSession::new(document).expect("valid session")
}

/// Proves successful domain-built mutations advance legacy-session revision
/// exactly once while exposing the correct current effective pattern values.
#[test]
fn successful_commands_mutate_once_and_advance_revision_once() {
    let mut session = session();
    let commands = [
        (
            session
                .document()
                .set_channel_density_for_effective(
                    CHANNEL_ID,
                    DensityEditedAxis::AcrossX,
                    DensityMetric2D {
                        across_x: 70.0,
                        across_y: 60.0,
                        aspect_locked: true,
                    },
                )
                .expect("density command builds"),
            InvalidationLevel::Family,
        ),
        (
            session
                .document()
                .set_channel_pattern_rotation_for_effective(CHANNEL_ID, 20.0)
                .expect("rotation command builds"),
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
            session
                .document()
                .set_channel_mark_response_field_for_effective(
                    CHANNEL_ID,
                    MarkGeometryFieldEdit::MaximumFill(0.85),
                )
                .expect("mark response command builds"),
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
        assert_eq!(result.invalidation, Some(*invalidation));
        assert_eq!(result.affected_channels, vec![CHANNEL_ID]);
        assert_eq!(session.revision(), Revision((index + 1) as u64));
    }

    let channel = session
        .document()
        .effective_channel_pattern(CHANNEL_ID)
        .expect("channel resolves");
    assert_eq!(channel.density.across_x, 70.0);
    assert_eq!(channel.pattern_rotation_degrees, 20.0);
    assert_eq!(channel.translation_x, 2.0);
    assert_eq!(channel.translation_y, 0.0);
    let PatternGeometryResponse::Marks(response) = channel.geometry_response else {
        panic!("fixture remains marks")
    };
    assert_eq!(response.maximum_fill, 0.85);
    let channel = session
        .document()
        .channel(CHANNEL_ID)
        .expect("channel exists");
    assert_eq!(channel.appearance.color.blue, 0.6);
    assert_eq!(channel.appearance.opacity, 0.4);
    assert!(!channel.appearance.visible);
}

/// Proves invalid domain-built command attempts preserve exact legacy
/// authority and revision without manufacturing obsolete raw commands.
#[test]
fn failed_commands_preserve_exact_document_and_revision() {
    let mut session = session();
    let original_document = session.snapshot();
    let original_revision = session.revision();

    let invalid_commands = [
        session.document().set_channel_density_for_effective(
            CHANNEL_ID,
            DensityEditedAxis::AcrossX,
            DensityMetric2D {
                across_x: 0.0,
                across_y: 60.0,
                aspect_locked: true,
            },
        ),
        session
            .document()
            .set_channel_pattern_rotation_for_effective(CHANNEL_ID, f64::NAN),
        session
            .document()
            .set_channel_mark_response_field_for_effective(
                CHANNEL_ID,
                MarkGeometryFieldEdit::MinimumFill(-1.0),
            ),
    ];
    for command in invalid_commands {
        assert!(command.is_err(), "invalid effective edit must not build");
        assert_eq!(session.snapshot(), original_document);
        assert_eq!(session.revision(), original_revision);
    }
    for command in [
        DocumentCommand::SetTranslationAxis {
            channel_id: CHANNEL_ID,
            edited_axis: toniator_domain::TranslationEditedAxis::X,
            value: f64::INFINITY,
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

/// Proves evaluation tokens remain current only until a stale-aware effective
/// pattern command advances the authoritative document revision.
#[test]
fn current_results_are_accepted_and_stale_results_are_rejected() {
    let mut session = session();
    let current = session
        .evaluation_token(CHANNEL_ID)
        .expect("channel exists");
    assert!(session.accepts_evaluation(current));

    session
        .apply(
            &session
                .document()
                .set_channel_pattern_rotation_for_effective(CHANNEL_ID, 15.0)
                .expect("rotation command builds"),
        )
        .expect("valid edit");
    assert!(!session.accepts_evaluation(current));

    let replacement = session
        .evaluation_token(CHANNEL_ID)
        .expect("channel exists");
    assert!(session.accepts_evaluation(replacement));
}
