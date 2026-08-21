use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelGeometryResponseDelta, ChannelId, ChannelPatternInstance,
    ChannelPatternLayoutDelta, ChannelSourceMapping, ChannelState, ColorValue, CoveragePolicy,
    DensityEditedAxis, DensityMetric2D, DensityMetricDelta2D, Document, DocumentCommand,
    DocumentHistory, DocumentId, DocumentSession, DocumentSessionError, InvalidationLevel,
    MarkGeometryFieldEdit, MarkGeometryResponse, MarkGeometryResponseDelta, PatternDefinition,
    PatternDefinitionDraft, PatternDefinitionId, PatternDefinitionRecipe, PatternGeometryResponse,
    PatternMechanismId, PatternOutputLayerId, PropertyFieldId, PropertyInheritance, PropertyTarget,
    SourceComponent, SourcePlacement, SourceReference,
};

/// Builds a current-format authority fixture with a resolved RGB topology.
fn document() -> Document {
    Document::new_default_document(
        CanvasSpec {
            width: 200.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document is valid")
}

/// Builds a second valid mark-producing definition with disjoint document IDs.
fn second_definition() -> PatternDefinition {
    PatternDefinition::supported_straight_grid(
        PatternDefinitionId(2),
        "second grid",
        PatternMechanismId(3),
        PatternMechanismId(4),
        PatternOutputLayerId(2),
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    )
}

/// Adds the second definition without changing any channel or base reference.
fn with_second_definition(document: Document) -> Document {
    document
        .apply_command(&DocumentCommand::AddTypedPatternDefinition {
            definition: second_definition(),
        })
        .expect("second definition is valid")
        .0
}

/// Applies one command and returns only its validated candidate document.
fn apply(document: Document, command: DocumentCommand) -> Document {
    document.apply_command(&command).expect("command applies").0
}

/// Builds the retained legacy channel configuration against the same base authority.
fn legacy_document() -> Document {
    let modeled = document();
    let definition = modeled.pattern_definitions()[0].clone();
    Document::new(
        DocumentId(9),
        modeled.canvas().clone(),
        vec![definition],
        modeled.pattern_settings().clone(),
        vec![ChannelState {
            id: ChannelId(41),
            pattern_instance: ChannelPatternInstance {
                definition_override: None,
                layout_delta: ChannelPatternLayoutDelta {
                    density: None,
                    rotation_degrees: None,
                    translation_x: 3.0,
                    translation_y: -2.0,
                },
                shape_rotation_delta_degrees: None,
                geometry_response_delta: None,
            },
            appearance: ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.2,
                    green: 0.3,
                    blue: 0.4,
                    alpha: 1.0,
                },
                opacity: 1.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )
    .expect("legacy configuration resolves through the shared base")
}

/// Resolves additive density, rotation, shape rotation, and mark response only
/// through the document boundary without frontend-side subtraction.
#[test]
fn effective_pattern_composes_typed_channel_deltas() {
    let document = document();
    let density = document
        .set_channel_density_for_effective(
            ChannelId(2),
            DensityEditedAxis::AcrossY,
            DensityMetric2D {
                across_x: 20.0,
                across_y: 16.0,
                aspect_locked: false,
            },
        )
        .expect("domain derives a locked pair");
    let (document, _) = document.apply_command(&density).expect("density applies");
    let rotation = document
        .set_channel_pattern_rotation_for_effective(ChannelId(2), 33.0)
        .expect("rotation delta builds");
    let (document, _) = document.apply_command(&rotation).expect("rotation applies");
    let shape = document
        .set_channel_shape_rotation_for_effective(ChannelId(2), -12.0)
        .expect("shape delta builds");
    let (document, _) = document.apply_command(&shape).expect("shape applies");
    let response = document
        .set_channel_geometry_response_for_effective(
            ChannelId(2),
            PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.25,
                maximum_fill: 1.5,
            }),
        )
        .expect("response delta builds");
    let (document, _) = document.apply_command(&response).expect("response applies");
    let effective = document
        .effective_channel_pattern(ChannelId(2))
        .expect("channel resolves");
    assert_eq!(effective.density.across_y, 16.0);
    assert_eq!(effective.pattern_rotation_degrees, 33.0);
    assert_eq!(effective.shape_rotation_degrees, -12.0);
    let PatternGeometryResponse::Marks(response) = effective.geometry_response;
    assert_eq!(response.minimum_fill, 0.25);
    assert_eq!(response.maximum_fill, 1.5);
}

/// Rejects a stale base and preserves the current authoritative document.
#[test]
fn channel_delta_rejects_stale_document_base() {
    let document = document();
    let command = document
        .set_channel_pattern_rotation_for_effective(ChannelId(1), 10.0)
        .expect("command builds");
    let new_base = document
        .set_document_density_aspect_lock(false)
        .expect("base command builds");
    let (document, _) = document.apply_command(&new_base).expect("base applies");
    assert!(document.apply_command(&command).is_err());
}

/// Removes stored response intent instead of copying its effective values and
/// therefore follows later base changes.
#[test]
fn reset_response_removes_intent_and_later_base_change_flows_through() {
    let document = document();
    let command = document
        .set_channel_geometry_response_for_effective(
            ChannelId(1),
            PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.5,
                maximum_fill: 1.0,
            }),
        )
        .expect("response command builds");
    let (document, _) = document.apply_command(&command).expect("response applies");
    let reset = DocumentCommand::ResetChannelGeometryResponseDelta {
        base: document.pattern_settings().clone(),
        channel_id: ChannelId(1),
    };
    let (document, _) = document.apply_command(&reset).expect("reset applies");
    assert!(
        document
            .channel_pattern_instance(ChannelId(1))
            .expect("channel instance")
            .geometry_response_delta
            .is_none()
    );
    let mut settings = document.pattern_settings().clone();
    settings.geometry_response = PatternGeometryResponse::Marks(MarkGeometryResponse {
        minimum_fill: 0.2,
        maximum_fill: 1.2,
    });
    let (document, _) = document
        .apply_command(&DocumentCommand::SetDocumentPatternSettings {
            base: document.pattern_settings().clone(),
            settings,
        })
        .expect("base applies");
    let PatternGeometryResponse::Marks(response) = document
        .effective_channel_pattern(ChannelId(1))
        .expect("effective response")
        .geometry_response;
    assert_eq!(response.minimum_fill, 0.2);
}

/// Retains an untouched mark-response member as inherited and exposes reset
/// only for the channel delta descriptor, never for the document base.
#[test]
fn partial_mark_delta_keeps_companion_inherited_and_base_not_resettable() {
    let document = document();
    let command = document
        .set_channel_mark_response_field_for_effective(
            ChannelId(1),
            MarkGeometryFieldEdit::MinimumFill(0.25),
        )
        .expect("field command builds");
    let (document, _) = document
        .apply_command(&command)
        .expect("field command applies");
    let values = document.property_values();
    let minimum = values
        .iter()
        .find(|value| {
            value.descriptor.target == PropertyTarget::Channel(ChannelId(1))
                && value.descriptor.field == PropertyFieldId::MarkMinimumFill
        })
        .expect("minimum descriptor");
    let maximum = values
        .iter()
        .find(|value| {
            value.descriptor.target == PropertyTarget::Channel(ChannelId(1))
                && value.descriptor.field == PropertyFieldId::MarkMaximumFill
        })
        .expect("maximum descriptor");
    let base = values
        .iter()
        .find(|value| {
            value.descriptor.target == PropertyTarget::Document
                && value.descriptor.field == PropertyFieldId::MarkMinimumFill
        })
        .expect("base descriptor");
    assert_eq!(minimum.inheritance, PropertyInheritance::Explicit);
    assert_eq!(maximum.inheritance, PropertyInheritance::Inherited);
    assert!(minimum.descriptor.reset_capable);
    assert!(!base.descriptor.reset_capable);
    assert!(!values.iter().any(|value| {
        matches!(value.descriptor.target, PropertyTarget::Channel(_))
            && value.descriptor.field == PropertyFieldId::DensityAspectLocked
    }));
}

/// Resolves both retained channel configurations through the same domain-only projection.
#[test]
fn modeled_and_legacy_channels_share_the_effective_resolver() {
    let modeled = document()
        .effective_channel_pattern(ChannelId(1))
        .expect("modeled channel resolves");
    let legacy = legacy_document()
        .effective_channel_pattern(ChannelId(41))
        .expect("legacy channel resolves");
    assert_eq!(modeled.definition_id, legacy.definition_id);
    assert_eq!(modeled.density, legacy.density);
    assert_eq!(legacy.translation_x, 3.0);
    assert_eq!(legacy.translation_y, -2.0);
}

/// Keeps authored degrees verbatim while rejecting non-finite additive composition.
#[test]
fn rotations_are_additive_without_normalization_and_overflow_is_rejected() {
    let document = document();
    let mut settings = document.pattern_settings().clone();
    settings.pattern_rotation_degrees = 720.0;
    let document = apply(
        document.clone(),
        DocumentCommand::SetDocumentPatternSettings {
            base: document.pattern_settings().clone(),
            settings,
        },
    );
    let command = document
        .set_channel_pattern_rotation_for_effective(ChannelId(1), -450.0)
        .expect("finite authored angle builds");
    let document = apply(document, command);
    assert_eq!(
        document
            .effective_channel_pattern(ChannelId(1))
            .expect("rotation resolves")
            .pattern_rotation_degrees,
        -450.0
    );

    let mut settings = document.pattern_settings().clone();
    settings.pattern_rotation_degrees = f64::MAX;
    let document = apply(
        document.clone(),
        DocumentCommand::SetDocumentPatternSettings {
            base: document.pattern_settings().clone(),
            settings,
        },
    );
    let overflow = DocumentCommand::SetChannelPatternRotationDelta {
        base: document.pattern_settings().clone(),
        channel_id: ChannelId(2),
        rotation_degrees: f64::MAX,
    };
    assert!(document.apply_command(&overflow).is_err());
}

/// Derives the locked companion density axis for edits originating on either axis.
#[test]
fn aspect_lock_derives_both_document_and_channel_companion_axes() {
    let document = document();
    let command = document
        .set_document_density_axis(DensityEditedAxis::AcrossY, 25.0)
        .expect("document Y edit builds");
    let document = apply(document, command);
    assert_eq!(document.pattern_settings().density.across_x, 50.0);
    assert_eq!(document.pattern_settings().density.across_y, 25.0);

    let command = document
        .set_channel_density_for_effective(
            ChannelId(1),
            DensityEditedAxis::AcrossX,
            DensityMetric2D {
                across_x: 32.0,
                across_y: 999.0,
                aspect_locked: false,
            },
        )
        .expect("channel X edit builds");
    let document = apply(document, command);
    let effective = document
        .effective_channel_pattern(ChannelId(1))
        .expect("density resolves");
    assert_eq!(effective.density.across_x, 32.0);
    assert_eq!(effective.density.across_y, 16.0);
    assert!(effective.density.aspect_locked);
}

/// Rejects missing definitions, invalid density, and every accepted mark-response bound atomically.
#[test]
fn invalid_effective_values_never_publish_a_candidate() {
    let document = document();
    assert!(
        document
            .apply_command(&DocumentCommand::SetChannelPatternDefinitionOverride {
                base: document.pattern_settings().clone(),
                channel_id: ChannelId(1),
                definition_id: PatternDefinitionId(999),
            })
            .is_err()
    );
    assert!(
        document
            .apply_command(&DocumentCommand::SetChannelDensityDelta {
                base: document.pattern_settings().clone(),
                channel_id: ChannelId(1),
                density: DensityMetricDelta2D {
                    across_x_delta: -document.pattern_settings().density.across_x,
                    across_y_delta: 0.0,
                },
            })
            .is_err()
    );
    for response in [
        MarkGeometryResponse {
            minimum_fill: -0.01,
            maximum_fill: 1.0,
        },
        MarkGeometryResponse {
            minimum_fill: 0.0,
            maximum_fill: 2.01,
        },
        MarkGeometryResponse {
            minimum_fill: 1.5,
            maximum_fill: 1.0,
        },
        MarkGeometryResponse {
            minimum_fill: f64::NAN,
            maximum_fill: 1.0,
        },
    ] {
        assert!(
            document
                .set_channel_geometry_response_for_effective(
                    ChannelId(1),
                    PatternGeometryResponse::Marks(response),
                )
                .is_err()
        );
    }
}

/// Rejects a base edit when an existing additive density would become nonpositive.
#[test]
fn document_base_edit_revalidates_every_existing_channel_atomically() {
    let document = document();
    let command = DocumentCommand::SetChannelDensityDelta {
        base: document.pattern_settings().clone(),
        channel_id: ChannelId(1),
        density: DensityMetricDelta2D {
            across_x_delta: -19.0,
            across_y_delta: -9.0,
        },
    };
    let document = apply(document, command);
    let before = document.clone();
    let mut settings = document.pattern_settings().clone();
    settings.density.across_x = 10.0;
    settings.density.across_y = 5.0;
    assert!(
        document
            .apply_command(&DocumentCommand::SetDocumentPatternSettings {
                base: document.pattern_settings().clone(),
                settings,
            })
            .is_err()
    );
    assert_eq!(document, before);
}

/// Treats explicit equal-output intent as meaningful authority but rejects repeating it.
#[test]
fn authority_only_changes_report_no_evaluation_work_and_true_noops_reject() {
    let document = document();
    let command = document
        .set_channel_pattern_rotation_for_effective(ChannelId(1), 0.0)
        .expect("zero delta builds");
    let (document, result) = document
        .apply_command(&command)
        .expect("absent to explicit zero is meaningful intent");
    assert!(result.affected_channels.is_empty());
    assert_eq!(result.invalidation, None);
    assert_eq!(
        document
            .channel_pattern_instance(ChannelId(1))
            .expect("channel intent")
            .layout_delta
            .rotation_degrees,
        Some(0.0)
    );
    assert!(document.apply_command(&command).is_err());
    assert!(
        document
            .apply_command(&DocumentCommand::ResetChannelDensityDelta {
                base: document.pattern_settings().clone(),
                channel_id: ChannelId(1),
            })
            .is_err()
    );
}

/// Orders only channels whose effective definitions change during a document-base edit.
#[test]
fn document_edits_report_ordered_changed_channels_and_exact_invalidation() {
    let document = with_second_definition(document());
    let document = apply(
        document.clone(),
        DocumentCommand::SetChannelPatternDefinitionOverride {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(2),
            definition_id: PatternDefinitionId(2),
        },
    );
    let mut settings = document.pattern_settings().clone();
    settings.definition_id = PatternDefinitionId(2);
    let (document, result) = document
        .apply_command(&DocumentCommand::SetDocumentPatternSettings {
            base: document.pattern_settings().clone(),
            settings,
        })
        .expect("base definition changes");
    assert_eq!(result.affected_channels, vec![ChannelId(1), ChannelId(3)]);
    assert_eq!(result.invalidation, Some(InvalidationLevel::Family));

    let mut settings = document.pattern_settings().clone();
    settings.shape_rotation_degrees = 15.0;
    let (_, result) = document
        .apply_command(&DocumentCommand::SetDocumentPatternSettings {
            base: document.pattern_settings().clone(),
            settings,
        })
        .expect("shape base changes");
    assert_eq!(
        result.affected_channels,
        vec![ChannelId(1), ChannelId(2), ChannelId(3)]
    );
    assert_eq!(result.invalidation, Some(InvalidationLevel::Realization));
}

/// Removes every stored channel override or delta while preserving unrelated intent and later inheritance.
#[test]
fn resets_remove_intent_and_follow_later_document_base_changes() {
    let document = with_second_definition(document());
    let document = apply(
        document.clone(),
        DocumentCommand::SetChannelPatternDefinitionOverride {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(1),
            definition_id: PatternDefinitionId(2),
        },
    );
    let density = document
        .set_channel_density_for_effective(
            ChannelId(1),
            DensityEditedAxis::AcrossX,
            DensityMetric2D {
                across_x: 30.0,
                across_y: 15.0,
                aspect_locked: true,
            },
        )
        .expect("density builds");
    let document = apply(document, density);
    let rotation = document
        .set_channel_pattern_rotation_for_effective(ChannelId(1), 25.0)
        .expect("rotation builds");
    let document = apply(document, rotation);
    let shape = document
        .set_channel_shape_rotation_for_effective(ChannelId(1), 35.0)
        .expect("shape builds");
    let document = apply(document, shape);
    let response = document
        .set_channel_geometry_response_for_effective(
            ChannelId(1),
            PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.2,
                maximum_fill: 1.4,
            }),
        )
        .expect("response builds");
    let mut document = apply(document, response);
    for command in [
        DocumentCommand::ResetChannelPatternDefinitionOverride {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(1),
        },
        DocumentCommand::ResetChannelDensityDelta {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(1),
        },
        DocumentCommand::ResetChannelPatternRotationDelta {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(1),
        },
        DocumentCommand::ResetChannelShapeRotationDelta {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(1),
        },
        DocumentCommand::ResetChannelGeometryResponseDelta {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(1),
        },
    ] {
        document = apply(document, command);
    }
    let instance = document
        .channel_pattern_instance(ChannelId(1))
        .expect("channel intent");
    assert_eq!(instance.definition_override, None);
    assert_eq!(instance.layout_delta.density, None);
    assert_eq!(instance.layout_delta.rotation_degrees, None);
    assert_eq!(instance.shape_rotation_delta_degrees, None);
    assert_eq!(instance.geometry_response_delta, None);

    let mut settings = document.pattern_settings().clone();
    settings.definition_id = PatternDefinitionId(2);
    settings.density.across_x = 40.0;
    settings.density.across_y = 20.0;
    settings.pattern_rotation_degrees = 10.0;
    settings.shape_rotation_degrees = -10.0;
    settings.geometry_response = PatternGeometryResponse::Marks(MarkGeometryResponse {
        minimum_fill: 0.1,
        maximum_fill: 1.1,
    });
    let document = apply(
        document.clone(),
        DocumentCommand::SetDocumentPatternSettings {
            base: document.pattern_settings().clone(),
            settings,
        },
    );
    let effective = document
        .effective_channel_pattern(ChannelId(1))
        .expect("reset channel inherits later base");
    assert_eq!(effective.definition_id, PatternDefinitionId(2));
    assert_eq!(effective.density.across_x, 40.0);
    assert_eq!(effective.pattern_rotation_degrees, 10.0);
    assert_eq!(effective.shape_rotation_degrees, -10.0);
}

/// Preserves a partial mark delta exactly instead of materializing its inherited companion.
#[test]
fn direct_partial_mark_delta_composes_and_resets_atomically() {
    let document = document();
    let command = DocumentCommand::SetChannelGeometryResponseDelta {
        base: document.pattern_settings().clone(),
        channel_id: ChannelId(3),
        geometry_response: ChannelGeometryResponseDelta::Marks(MarkGeometryResponseDelta {
            minimum_fill_delta: Some(0.25),
            maximum_fill_delta: None,
        }),
    };
    let document = apply(document, command);
    let PatternGeometryResponse::Marks(response) = document
        .effective_channel_pattern(ChannelId(3))
        .expect("partial response resolves")
        .geometry_response;
    assert_eq!(response.minimum_fill, 0.25);
    assert_eq!(response.maximum_fill, 1.0);
    let reset = DocumentCommand::ResetChannelGeometryResponseDelta {
        base: document.pattern_settings().clone(),
        channel_id: ChannelId(3),
    };
    let document = apply(document, reset);
    assert_eq!(
        document
            .channel_pattern_instance(ChannelId(3))
            .expect("channel intent")
            .geometry_response_delta,
        None
    );
}

/// Rejects stale reset commands before checking whether their stored field is present.
#[test]
fn every_channel_reset_is_stale_against_a_later_document_base() {
    let document = document();
    let stale_base = document.pattern_settings().clone();
    let mut settings = stale_base.clone();
    settings.shape_rotation_degrees = 5.0;
    let document = apply(
        document,
        DocumentCommand::SetDocumentPatternSettings {
            base: stale_base.clone(),
            settings,
        },
    );
    for command in [
        DocumentCommand::ResetChannelPatternDefinitionOverride {
            base: stale_base.clone(),
            channel_id: ChannelId(1),
        },
        DocumentCommand::ResetChannelDensityDelta {
            base: stale_base.clone(),
            channel_id: ChannelId(1),
        },
        DocumentCommand::ResetChannelPatternRotationDelta {
            base: stale_base.clone(),
            channel_id: ChannelId(1),
        },
        DocumentCommand::ResetChannelShapeRotationDelta {
            base: stale_base.clone(),
            channel_id: ChannelId(1),
        },
        DocumentCommand::ResetChannelGeometryResponseDelta {
            base: stale_base.clone(),
            channel_id: ChannelId(1),
        },
    ] {
        assert!(document.apply_command(&command).is_err());
    }
}

/// Records additive authority in history and restores its exact absence through undo and redo.
#[test]
fn history_round_trips_channel_delta_intent() {
    let document = document();
    let command = document
        .set_channel_shape_rotation_for_effective(ChannelId(1), 22.0)
        .expect("shape command builds");
    let mut history = DocumentHistory::new(DocumentSession::new(document).expect("valid session"));
    let result = history.apply(&command).expect("history applies");
    assert_eq!(result.invalidation, Some(InvalidationLevel::Realization));
    assert_eq!(history.revision().0, 1);
    assert_eq!(
        history
            .document()
            .channel_pattern_instance(ChannelId(1))
            .expect("channel intent")
            .shape_rotation_delta_degrees,
        Some(22.0)
    );
    history
        .undo()
        .expect("undo succeeds")
        .expect("entry exists");
    assert_eq!(
        history
            .document()
            .channel_pattern_instance(ChannelId(1))
            .expect("channel intent")
            .shape_rotation_delta_degrees,
        None
    );
    history
        .redo()
        .expect("redo succeeds")
        .expect("entry exists");
    assert_eq!(
        history
            .document()
            .effective_channel_pattern(ChannelId(1))
            .expect("shape resolves")
            .shape_rotation_degrees,
        22.0
    );
}

/// Requires history for recipe materialization and round-trips a fresh document-base definition.
#[test]
fn recipe_materialization_is_fresh_stale_aware_and_history_owned() {
    let document = document();
    let command = DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: document.pattern_settings().clone(),
        base_definition: document.pattern_definitions()[0].clone(),
        recipe: PatternDefinitionRecipe::StraightGrid(PatternDefinitionDraft {
            name: "replacement".into(),
            coverage: CoveragePolicy {
                guard_steps: 3,
                additional_margin: 1.0,
            },
        }),
    };
    let mut session = DocumentSession::new(document.clone()).expect("valid session");
    assert_eq!(
        session.apply(&command),
        Err(DocumentSessionError::HistoryRequired)
    );
    let mut history = DocumentHistory::new(DocumentSession::new(document).expect("valid session"));
    history
        .apply(&command)
        .expect("history materializes recipe");
    assert_eq!(history.document().pattern_definitions().len(), 2);
    assert_eq!(
        history.document().pattern_settings().definition_id,
        PatternDefinitionId(2)
    );
    history
        .undo()
        .expect("undo succeeds")
        .expect("entry exists");
    assert_eq!(history.document().pattern_definitions().len(), 1);
    history
        .redo()
        .expect("redo succeeds")
        .expect("entry exists");
    assert_eq!(history.document().pattern_definitions().len(), 2);
}

/// Materializes a document-base recipe without assuming that a valid legacy document has channels.
#[test]
fn document_recipe_materialization_supports_zero_channel_legacy_authority() {
    let fixture = document();
    let document = Document::new(
        DocumentId(10),
        fixture.canvas().clone(),
        fixture.pattern_definitions().to_vec(),
        fixture.pattern_settings().clone(),
        Vec::new(),
    )
    .expect("zero-channel legacy documents remain valid");
    let command = DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: document.pattern_settings().clone(),
        base_definition: document.pattern_definitions()[0].clone(),
        recipe: PatternDefinitionRecipe::StraightGrid(PatternDefinitionDraft {
            name: "zero-channel replacement".into(),
            coverage: CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        }),
    };
    let mut history = DocumentHistory::new(
        DocumentSession::new(document).expect("zero-channel session remains valid"),
    );
    history
        .apply(&command)
        .expect("document-base recipe does not require a channel");
    assert_eq!(history.document().pattern_definitions().len(), 2);
    assert_eq!(
        history.document().pattern_settings().definition_id,
        PatternDefinitionId(2)
    );
    assert!(history.document().channels().is_some_and(<[_]>::is_empty));
}

/// Squashes explicit equal-output draft intent without inventing evaluation work.
#[test]
fn draft_squash_preserves_authority_only_change_with_no_invalidation() {
    let document = document();
    let mut main = DocumentHistory::new(DocumentSession::new(document).expect("valid session"));
    let mut draft = DocumentHistory::new_draft(&main);
    let command = draft
        .document()
        .set_channel_pattern_rotation_for_effective(ChannelId(1), 0.0)
        .expect("zero delta builds");
    draft.apply(&command).expect("draft records explicit zero");
    let result = main.squash_draft(&draft).expect("draft publishes");
    assert!(!result.unchanged);
    assert!(result.affected_channels.is_empty());
    assert_eq!(result.invalidation, None);
    assert_eq!(
        main.document()
            .channel_pattern_instance(ChannelId(1))
            .expect("channel intent")
            .layout_delta
            .rotation_degrees,
        Some(0.0)
    );
}
