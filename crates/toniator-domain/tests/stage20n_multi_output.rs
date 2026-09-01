//! Focused Stage 20N keyed-output response authority coverage.

use std::panic::{AssertUnwindSafe, catch_unwind};

use toniator_domain::{
    CanvasSpec, ChannelId, ConnectedGeometryResponse, CoveragePolicy, Document, DocumentCommand,
    DocumentHistory, DocumentSession, MarkGeometryResponse, PatternDefinition,
    PatternDefinitionBundle, PatternDefinitionDraft, PatternDefinitionEdit, PatternDefinitionId,
    PatternDefinitionRecipe, PatternGeometryResponse, PatternMechanismId, PatternOutputLayerId,
    PatternOutputSettings, PatternOutputSettingsRecipe, PatternStructureRecipe, PropertyFieldId,
    PropertyTarget, SiteUseFilterRecipe, SourceReference, effective_pattern_output_settings,
};

/// Proves one bundle exposes the sole aligned typed response in structural output order.
#[test]
fn aligned_output_settings_resolve_in_structural_order() {
    let definition = PatternDefinition::supported_straight_grid(
        PatternDefinitionId(1),
        "grid",
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternOutputLayerId(4),
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    let bundle = PatternDefinitionBundle {
        definition,
        output_settings: vec![PatternOutputSettings {
            output_layer_id: PatternOutputLayerId(4),
            response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.1,
                maximum_fill: 0.9,
            }),
        }],
    };
    let effective = effective_pattern_output_settings(&bundle, &[]).expect("aligned bundle");
    assert_eq!(effective.len(), 1);
    assert_eq!(effective[0].output_layer_id, PatternOutputLayerId(4));
}

/// Proves a foreign keyed setting cannot be accepted as an implicit replacement response.
#[test]
fn foreign_output_setting_is_rejected_atomically() {
    let definition = PatternDefinition::supported_straight_grid(
        PatternDefinitionId(1),
        "grid",
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternOutputLayerId(4),
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    let bundle = PatternDefinitionBundle {
        definition,
        output_settings: vec![PatternOutputSettings {
            output_layer_id: PatternOutputLayerId(5),
            response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.1,
                maximum_fill: 0.9,
            }),
        }],
    };
    assert_eq!(
        bundle
            .validate()
            .expect_err("foreign output setting")
            .path(),
        "pattern.bundle.output_settings.order"
    );
}

/// Proves a missing ordered output setting is rejected before any bundle becomes authoritative.
#[test]
fn missing_output_setting_is_rejected_atomically() {
    let definition = PatternDefinition::supported_straight_grid(
        PatternDefinitionId(1),
        "grid",
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternOutputLayerId(4),
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    let bundle = PatternDefinitionBundle {
        definition,
        output_settings: Vec::new(),
    };
    assert_eq!(
        bundle.validate().expect_err("missing setting").path(),
        "pattern.bundle.output_settings.cardinality"
    );
}

/// Proves a typed setting cannot supply the connected branch to a mark output.
#[test]
fn output_setting_kind_mismatch_is_rejected_atomically() {
    let definition = PatternDefinition::supported_straight_grid(
        PatternDefinitionId(1),
        "grid",
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternOutputLayerId(4),
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    let bundle = PatternDefinitionBundle {
        definition,
        output_settings: vec![PatternOutputSettings {
            output_layer_id: PatternOutputLayerId(4),
            response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.0,
                maximum_thickness: 1.0,
                bias: 0.0,
            }),
        }],
    };
    assert_eq!(
        bundle.validate().expect_err("kind mismatch").path(),
        "pattern.bundle.output_settings.kind"
    );
}

/// Proves recipe materialization allocates an output ID and atomically binds its authored response.
#[test]
fn recipe_materialization_binds_ordered_id_free_output_settings() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document");
    let recipe = PatternDefinitionRecipe {
        structure: PatternStructureRecipe::StraightGrid(PatternDefinitionDraft {
            name: "bound response".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
        }),
        output_settings: vec![PatternOutputSettingsRecipe {
            source_filter: SiteUseFilterRecipe::All,
            response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.7,
            }),
        }],
    };
    let command = DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: document.pattern_settings().clone(),
        base_definition: document.pattern_definition_bundles()[0].definition.clone(),
        recipe,
    };
    let (replaced, _) = document.apply_command(&command).expect("recipe binds");
    let bundle = replaced
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == replaced.pattern_settings().definition_id)
        .expect("selected bundle");
    assert_eq!(
        bundle.output_settings[0].output_layer_id,
        bundle.definition.output_layers[0].id()
    );
    assert_eq!(
        bundle.output_settings[0].response,
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 0.2,
            maximum_fill: 0.7,
        })
    );
}

/// Proves response descriptors identify the effective channel output rather than a singular channel response.
#[test]
fn channel_response_descriptors_target_the_structural_output() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document");
    assert!(document.property_descriptors().iter().any(|descriptor| {
        descriptor.field == PropertyFieldId::MarkMinimumFill
            && descriptor.target
                == PropertyTarget::ChannelOutput(
                    toniator_domain::ChannelId(1),
                    PatternOutputLayerId(1),
                )
    }));
}

/// Proves copy-on-edit remaps a retained typed delta to the duplicate output
/// ID and history restores the exact prior and resulting bundle collections.
#[test]
fn copy_on_edit_remaps_output_deltas_and_history_restores_exact_state() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document");
    let delta = document
        .set_channel_output_response_for_effective(
            ChannelId(1),
            PatternOutputLayerId(1),
            PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.25,
                maximum_fill: 1.0,
            }),
        )
        .expect("typed delta command");
    let (document, _) = document.apply_command(&delta).expect("delta applies");
    let before = document.clone();
    let command = DocumentCommand::EditSelectedChannelPatternDefinition {
        channel_id: ChannelId(1),
        base_definition: document.pattern_definition_bundles()[0].definition.clone(),
        edit: PatternDefinitionEdit::SetCoverageGuardSteps { guard_steps: 3 },
    };
    let mut history = DocumentHistory::new(DocumentSession::new(document).expect("session"));
    history.apply(&command).expect("copy-on-edit applies");
    let after = history.document().clone();
    let output_layer_id = after
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id != PatternDefinitionId(1))
        .expect("duplicate bundle")
        .output_settings[0]
        .output_layer_id;
    assert_ne!(output_layer_id, PatternOutputLayerId(1));
    assert_eq!(
        after
            .channel_pattern_instance(ChannelId(1))
            .expect("channel")
            .output_response_deltas[0]
            .output_layer_id,
        output_layer_id
    );
    history.undo().expect("undo").expect("entry");
    assert_eq!(history.document(), &before);
    history.redo().expect("redo").expect("entry");
    assert_eq!(history.document(), &after);
}

/// Proves a recipe replacement prunes foreign deltas, while the history entry
/// restores both the prior bundle and exact prior keyed delta collection.
#[test]
fn recipe_replacement_prunes_foreign_deltas_and_history_is_exact() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document");
    let delta = document
        .set_channel_output_response_for_effective(
            ChannelId(1),
            PatternOutputLayerId(1),
            PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.25,
                maximum_fill: 1.0,
            }),
        )
        .expect("typed delta command");
    let (document, _) = document.apply_command(&delta).expect("delta applies");
    let before = document.clone();
    let command = DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: document.pattern_settings().clone(),
        base_definition: document.pattern_definition_bundles()[0].definition.clone(),
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::StraightGrid(
            PatternDefinitionDraft {
                name: "replacement".into(),
                coverage: CoveragePolicy {
                    guard_steps: 3,
                    additional_margin: 0.0,
                },
            },
        )),
    };
    let mut history = DocumentHistory::new(DocumentSession::new(document).expect("session"));
    history.apply(&command).expect("recipe applies");
    let after = history.document().clone();
    assert!(
        after
            .channel_pattern_instance(ChannelId(1))
            .expect("channel")
            .output_response_deltas
            .is_empty()
    );
    history.undo().expect("undo").expect("entry");
    assert_eq!(history.document(), &before);
    history.redo().expect("redo").expect("entry");
    assert_eq!(history.document(), &after);
}

/// Proves a malformed public reset command rejects atomically instead of reaching internal command assertions.
#[test]
fn reset_missing_output_delta_channel_is_nonpanicking_and_atomic() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document");
    let before = document.clone();
    let command = DocumentCommand::ResetChannelOutputResponseDelta {
        base: document.pattern_settings().clone(),
        channel_id: ChannelId(999),
        output_layer_id: PatternOutputLayerId(1),
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| document.apply_command(&command)));
    assert!(outcome.is_ok(), "malformed public command must not panic");
    assert_eq!(
        outcome
            .expect("nonpanicking validation")
            .expect_err("missing channel rejects")
            .path(),
        "command.channel_id"
    );
    assert_eq!(document, before);
}

/// Proves shared recipe replacement preserves the shared definition ID, prunes every linked delta,
/// reports document-order affected channels, and restores exact history snapshots.
#[test]
fn shared_recipe_replacement_prunes_linked_deltas_in_order_and_is_reversible() {
    let mut document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document");
    for (channel_id, minimum_fill) in [(ChannelId(1), 0.2), (ChannelId(2), 0.3)] {
        let command = document
            .set_channel_output_response_for_effective(
                channel_id,
                PatternOutputLayerId(1),
                PatternGeometryResponse::Marks(MarkGeometryResponse {
                    minimum_fill,
                    maximum_fill: 1.0,
                }),
            )
            .expect("delta command");
        document = document.apply_command(&command).expect("delta applies").0;
    }
    let before = document.clone();
    let command = DocumentCommand::ReplaceSharedPatternDefinitionRecipe {
        definition_id: PatternDefinitionId(1),
        base_definition: document.pattern_definition_bundles()[0].definition.clone(),
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::StraightGrid(
            PatternDefinitionDraft {
                name: "shared replacement".into(),
                coverage: CoveragePolicy {
                    guard_steps: 3,
                    additional_margin: 0.0,
                },
            },
        )),
    };
    let mut history = DocumentHistory::new(DocumentSession::new(document).expect("session"));
    let result = history.apply(&command).expect("shared replacement applies");
    assert_eq!(
        result.affected_channels,
        vec![ChannelId(1), ChannelId(2), ChannelId(3)]
    );
    let after = history.document().clone();
    assert_eq!(
        after.pattern_definition_bundles()[0].definition.id,
        PatternDefinitionId(1)
    );
    for channel_id in [ChannelId(1), ChannelId(2), ChannelId(3)] {
        assert!(
            after
                .channel_pattern_instance(channel_id)
                .expect("linked channel")
                .output_response_deltas
                .is_empty(),
            "replacement output IDs prune only now-foreign keyed intent"
        );
    }
    history.undo().expect("undo").expect("entry");
    assert_eq!(history.document(), &before);
    history.redo().expect("redo").expect("entry");
    assert_eq!(history.document(), &after);
    assert!(
        history.apply(&command).is_err(),
        "old structural base is stale"
    );
    assert_eq!(history.document(), &after);
}
