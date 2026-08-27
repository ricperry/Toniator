use toniator_domain::{
    CanvasSpec, ChannelGeometryResponseDelta, ChannelId, ConnectedGeometryResponse,
    ConnectedGeometryResponseDelta, CoveragePolicy, Document, DocumentCommand, DocumentHistory,
    DocumentId, DocumentSession, GeneralizedSiteProduct, MarkOrientation, PathStrokeStyle,
    PatternDefinition, PatternDefinitionBundle, PatternDefinitionId, PatternGeometryResponse,
    PatternMechanismId, PatternOutputLayer, PatternOutputLayerId, PatternOutputRealization,
    PatternOutputSettings, SourceReference, StraightGuideDimension, StraightGuideRepetition,
};

/// Builds a modeled document whose one homogeneous recipe is guide-path output.
fn stroke_document() -> Document {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 120.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document validates");
    let guide_id = PatternMechanismId(31);
    let mut definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(30),
        "round guide paths",
        guide_id,
        PatternMechanismId(32),
        PatternOutputLayerId(33),
        vec![StraightGuideDimension {
            id: toniator_domain::GuideDimensionId(34),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![toniator_domain::GuideDimensionId(34)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = vec![PatternOutputLayer::all(
        PatternOutputLayerId(33),
        PatternOutputRealization::GuidePaths {
            guide_mechanism_id: guide_id,
            style: PathStrokeStyle::default(),
        },
    )];
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition.id;
    let bundle = PatternDefinitionBundle {
        output_settings: vec![PatternOutputSettings {
            output_layer_id: PatternOutputLayerId(33),
            response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.25,
                maximum_thickness: 1.0,
            }),
        }],
        definition,
    };
    Document::with_source_topology_and_authored_structures(
        DocumentId(201),
        base.canvas().clone(),
        SourceReference::Unassigned,
        vec![bundle],
        settings,
        base.channel_model().expect("modeled").to_owned(),
        base.channel_topology().expect("modeled").clone(),
        Vec::new(),
    )
    .expect("stroke document validates")
}

/// Proves the sole effective resolver composes connected deltas and reset restores inheritance.
#[test]
fn connected_response_delta_is_effective_only_and_resettable() {
    let document = stroke_document();
    let command = DocumentCommand::SetChannelOutputResponseDelta {
        base: document.pattern_settings().clone(),
        channel_id: ChannelId(1),
        output_layer_id: PatternOutputLayerId(33),
        delta: ChannelGeometryResponseDelta::Connected(ConnectedGeometryResponseDelta {
            minimum_thickness_delta: Some(0.25),
            maximum_thickness_delta: Some(0.5),
        }),
    };
    let (document, _) = document.apply_command(&command).expect("delta applies");
    let PatternGeometryResponse::Connected(response) = &document
        .effective_channel_pattern(ChannelId(1))
        .expect("effective stroke response")
        .output_settings[0]
        .response
    else {
        panic!("guide-path document resolves the connected branch");
    };
    assert_eq!(
        (response.minimum_thickness, response.maximum_thickness),
        (0.5, 1.5)
    );
    let reset = DocumentCommand::ResetChannelOutputResponseDelta {
        base: document.pattern_settings().clone(),
        channel_id: ChannelId(1),
        output_layer_id: PatternOutputLayerId(33),
    };
    let (document, _) = document.apply_command(&reset).expect("reset applies");
    assert!(
        document
            .channel_pattern_instance(ChannelId(1))
            .expect("channel")
            .output_response_deltas
            .is_empty()
    );
}

/// Proves desired-effective connected builders retain branch authority and reject invalid widths before commands exist.
#[test]
fn connected_desired_effective_builder_validates_branch_and_reset_intent() {
    let document = stroke_document();
    let command = document
        .set_channel_output_response_for_effective(
            ChannelId(1),
            PatternOutputLayerId(33),
            PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.4,
                maximum_thickness: 1.2,
            }),
        )
        .expect("valid desired connected response builds a delta command");
    assert!(matches!(
        command,
        DocumentCommand::SetChannelOutputResponseDelta {
            delta: ChannelGeometryResponseDelta::Connected(_),
            ..
        }
    ));
    let error = document
        .set_channel_output_response_for_effective(
            ChannelId(1),
            PatternOutputLayerId(33),
            PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 1.2,
                maximum_thickness: 0.4,
            }),
        )
        .expect_err("inverted connected response rejects");
    assert_eq!(error.path(), "channel.pattern.geometry_response");
    let error = document
        .set_channel_output_response_for_effective(
            ChannelId(1),
            PatternOutputLayerId(33),
            PatternGeometryResponse::Marks(toniator_domain::MarkGeometryResponse {
                minimum_fill: 0.0,
                maximum_fill: 1.0,
            }),
        )
        .expect_err("cross-branch desired response rejects");
    assert_eq!(error.path(), "channel.pattern.geometry_response");
    assert!(matches!(
        document.reset_channel_output_response_delta(ChannelId(1), PatternOutputLayerId(33)),
        Ok(DocumentCommand::ResetChannelOutputResponseDelta { .. })
    ));
}

/// Proves only connected thickness descriptors are applicable to a connected guide-path document.
#[test]
fn connected_document_exposes_connected_not_mark_response_descriptors() {
    let document = stroke_document();
    let descriptors = document.property_descriptors();
    assert!(descriptors.iter().any(|descriptor| descriptor.field
        == toniator_domain::PropertyFieldId::ConnectedMinimumThickness));
    assert!(!descriptors.iter().any(|descriptor| descriptor.field == toniator_domain::PropertyFieldId::MarkMinimumFill));
    document
        .validate_property_descriptors()
        .expect("connected descriptor surface is complete");
}

/// Keeps raw guide-path output outside the mark-only descriptor and value surface.
#[test]
fn guide_paths_expose_connected_fields_without_mark_output_descriptors() {
    let document = stroke_document();
    let descriptors = document.property_descriptors();
    let minimum = descriptors
        .iter()
        .find(|descriptor| {
            descriptor.field == toniator_domain::PropertyFieldId::ConnectedMinimumThickness
        })
        .expect("connected minimum descriptor exists");
    assert_eq!(
        minimum.target,
        toniator_domain::PropertyTarget::ChannelOutput(ChannelId(1), PatternOutputLayerId(33))
    );
    assert_eq!(
        minimum.authority,
        toniator_domain::PropertyAuthority::ChannelDelta
    );
    assert_eq!(
        minimum.invalidation,
        toniator_domain::InvalidationLevel::Realization
    );
    assert!(minimum.reset_capable);
    assert!(!descriptors.iter().any(|descriptor| {
        descriptor.field == toniator_domain::PropertyFieldId::OutputSiteProduct
    }));
    assert!(!descriptors.iter().any(|descriptor| {
        descriptor.field == toniator_domain::PropertyFieldId::OutputPrototype
    }));
    let values = document.property_values();
    assert_eq!(values.len(), descriptors.len());
    document
        .validate_property_descriptors()
        .expect("guide paths retain a complete non-mark descriptor surface");

    let command = document
        .set_channel_output_response_for_effective(
            ChannelId(1),
            PatternOutputLayerId(33),
            PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.3,
                maximum_thickness: 1.1,
            }),
        )
        .expect("connected delta command builds");
    let (candidate, _) = document
        .apply_command(&command)
        .expect("connected delta command applies");
    let authored = candidate
        .property_values()
        .into_iter()
        .filter(|value| {
            value.descriptor.target
                == toniator_domain::PropertyTarget::ChannelOutput(
                    ChannelId(1),
                    PatternOutputLayerId(33),
                )
                && matches!(
                    value.descriptor.field,
                    toniator_domain::PropertyFieldId::ConnectedMinimumThickness
                        | toniator_domain::PropertyFieldId::ConnectedMaximumThickness
                )
        })
        .map(|value| (value.descriptor.field, value.authored_value))
        .collect::<Vec<_>>();
    assert_eq!(authored.len(), 2);
    for (field, value) in authored {
        let Some(toniator_domain::PropertyCurrentValueKind::FiniteF64(delta)) = value else {
            panic!("connected delta descriptor must expose authored intent")
        };
        let expected = match field {
            toniator_domain::PropertyFieldId::ConnectedMinimumThickness => 0.05,
            toniator_domain::PropertyFieldId::ConnectedMaximumThickness => 0.1,
            _ => unreachable!("filter retains only connected response fields"),
        };
        assert!((delta - expected).abs() < 1.0e-12);
    }
}

/// Proves connected response authority rejects stale bases and round-trips exact stored delta intent through history.
#[test]
fn connected_delta_is_stale_aware_and_history_reversible() {
    let document = stroke_document();
    let command = document
        .set_channel_output_response_for_effective(
            ChannelId(1),
            PatternOutputLayerId(33),
            PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.3,
                maximum_thickness: 1.1,
            }),
        )
        .expect("connected command builds");
    let mut settings = document.pattern_settings().clone();
    settings.pattern_rotation_degrees = 5.0;
    let base_change = DocumentCommand::SetDocumentPatternSettings {
        base: document.pattern_settings().clone(),
        settings,
    };
    let (advanced, _) = document.apply_command(&base_change).expect("base advances");
    assert!(advanced.apply_command(&command).is_err());
    let mut history = DocumentHistory::new(DocumentSession::new(document).expect("session valid"));
    history.apply(&command).expect("connected delta applies");
    assert!(
        history
            .document()
            .channel_pattern_instance(ChannelId(1))
            .expect("channel")
            .output_response_deltas
            .iter()
            .any(|entry| entry.output_layer_id == PatternOutputLayerId(33))
    );
    history
        .undo()
        .expect("undo works")
        .expect("history entry exists");
    assert!(
        history
            .document()
            .channel_pattern_instance(ChannelId(1))
            .expect("channel")
            .output_response_deltas
            .is_empty()
    );
    history
        .redo()
        .expect("redo works")
        .expect("history entry exists");
    assert!(
        history
            .document()
            .channel_pattern_instance(ChannelId(1))
            .expect("channel")
            .output_response_deltas
            .iter()
            .any(|entry| entry.output_layer_id == PatternOutputLayerId(33))
    );
}
