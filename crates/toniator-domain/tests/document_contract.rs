use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelState, ColorValue,
    DensityMetric2D, Document, DocumentCommand, DocumentId, InvalidationLevel,
    MarkGeometryResponse, PatternDefinition, PatternDefinitionId,
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
            minimum_size: 0.0,
            maximum_size: 1.0,
        },
    }
}

fn definition() -> PatternDefinition {
    PatternDefinition {
        id: PATTERN_ID,
        name: "minimal".to_owned(),
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

fn assert_path(result: Result<Document, toniator_domain::ValidationError>, expected_path: &str) {
    assert_eq!(result.expect_err("must be invalid").path(), expected_path);
}

#[test]
fn accepts_the_required_900_by_600_density_document() {
    let document = valid_document();

    assert_eq!(document.canvas().width, 900.0);
    assert_eq!(document.canvas().height, 600.0);
    assert_eq!(document.channels()[0].layout.density.across_x, 90.0);
    assert_eq!(document.channels()[0].layout.density.across_y, 60.0);
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
                value.mark_geometry_response.minimum_size = 2.0;
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
                    minimum_size: 0.2,
                    maximum_size: 1.5,
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
