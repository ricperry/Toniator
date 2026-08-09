use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPaint, ChannelPatternLayout,
    ChannelSourceMapping, ChannelState, ChannelTopology, ChannelTopologyTemplate, ColorValue,
    CoveragePolicy, DensityMetric2D, Document, DocumentCommand, DocumentHistory, DocumentId,
    DocumentSession, DocumentSessionError, HalftoneChannelModel, HalftoneChannelRole,
    MarkGeometryResponse, ModeledChannelState, PatternDefinition, PatternDefinitionDraft,
    PatternDefinitionEdit, PatternDefinitionId, PatternMechanismId, PatternOutputLayerId, Revision,
    SourceComponent, SourceMapping, SourceMappingComponent, SourcePlacement, SourceReference,
    SourceReferenceId,
};

const CHANNEL_ID: ChannelId = ChannelId(7);
const PATTERN_ID: PatternDefinitionId = PatternDefinitionId(3);

fn document() -> Document {
    Document::with_source(
        DocumentId(1),
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("before-source").unwrap()),
        vec![PatternDefinition::supported_straight_grid(
            PATTERN_ID,
            "grid",
            PatternMechanismId(5),
            PatternMechanismId(6),
            PatternOutputLayerId(3),
            CoveragePolicy {
                guard_steps: 2,
                maximum_support_radius: 5.0,
            },
        )],
        vec![legacy_channel()],
    )
    .unwrap()
}

fn shared_history() -> DocumentHistory {
    let mut second = legacy_channel();
    second.id = ChannelId(8);
    let document = Document::with_source(
        DocumentId(2),
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("shared-source").unwrap()),
        vec![PatternDefinition::supported_straight_grid(
            PATTERN_ID,
            "shared grid",
            PatternMechanismId(5),
            PatternMechanismId(6),
            PatternOutputLayerId(3),
            CoveragePolicy {
                guard_steps: 2,
                maximum_support_radius: 5.0,
            },
        )],
        vec![legacy_channel(), second],
    )
    .unwrap();
    DocumentHistory::new(DocumentSession::new(document).unwrap())
}

#[test]
fn definition_commands_require_history_and_history_records_their_exact_inverse() {
    let source = shared_history().document().clone();
    let base = source.pattern_definitions()[0].clone();
    let commands = vec![
        DocumentCommand::AddPatternDefinition {
            definition: PatternDefinitionDraft {
                name: "history only".into(),
                coverage: CoveragePolicy {
                    guard_steps: 2,
                    maximum_support_radius: 5.0,
                },
            },
        },
        DocumentCommand::DuplicatePatternDefinition {
            definition_id: PATTERN_ID,
        },
        DocumentCommand::RetargetChannelPatternDefinition {
            channel_id: CHANNEL_ID,
            definition_id: PATTERN_ID,
        },
        DocumentCommand::RemoveUnreferencedPatternDefinition {
            definition_id: PATTERN_ID,
        },
        DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: CHANNEL_ID,
            base_definition: base.clone(),
            edit: PatternDefinitionEdit::SetCoverage {
                coverage: CoveragePolicy {
                    guard_steps: 3,
                    maximum_support_radius: 5.0,
                },
            },
        },
        DocumentCommand::EditSharedPatternDefinition {
            definition_id: PATTERN_ID,
            base_definition: base,
            edit: PatternDefinitionEdit::SetCoverage {
                coverage: CoveragePolicy {
                    guard_steps: 3,
                    maximum_support_radius: 5.0,
                },
            },
        },
    ];
    let mut session = DocumentSession::new(source.clone()).unwrap();
    for command in &commands {
        assert_eq!(
            session.apply(command),
            Err(DocumentSessionError::HistoryRequired)
        );
        assert_eq!(session.document(), &source);
        assert_eq!(session.revision(), Revision(0));
    }

    let mut history = DocumentHistory::new(DocumentSession::new(source.clone()).unwrap());
    let result = history.apply(&commands[0]).unwrap();
    let after = history.document().clone();
    assert_eq!(history.revision(), Revision(1));
    assert!(history.can_undo());
    assert!(!history.can_redo());
    assert_eq!(history.undo().unwrap(), Some(result.clone()));
    assert_eq!(history.document(), &source);
    assert_eq!(history.revision(), Revision(2));
    assert_eq!(history.redo().unwrap(), Some(result));
    assert_eq!(history.document(), &after);
    assert_eq!(history.revision(), Revision(3));
}

#[test]
fn typed_definition_commands_allocate_copy_share_and_history_atomically() {
    let mut history = shared_history();
    let original = history.document().clone();
    let add = DocumentCommand::AddPatternDefinition {
        definition: PatternDefinitionDraft {
            name: "independent".into(),
            coverage: CoveragePolicy {
                guard_steps: 4,
                maximum_support_radius: 6.0,
            },
        },
    };
    let add_result = history.apply(&add).unwrap();
    assert_eq!(
        add_result.invalidation,
        toniator_domain::InvalidationLevel::Family
    );
    assert!(add_result.affected_channels.is_empty());
    let added = history.document().pattern_definitions().last().unwrap();
    assert_eq!(added.id, PatternDefinitionId(4));
    assert_eq!(added.mechanisms[0].id(), PatternMechanismId(7));
    assert_eq!(added.mechanisms[1].id(), PatternMechanismId(8));
    assert_eq!(added.output_layers[0].id(), PatternOutputLayerId(4));

    let duplicate_result = history
        .apply(&DocumentCommand::DuplicatePatternDefinition {
            definition_id: PATTERN_ID,
        })
        .unwrap();
    assert!(duplicate_result.affected_channels.is_empty());
    let duplicate = history.document().pattern_definitions().last().unwrap();
    assert_eq!(duplicate.id, PatternDefinitionId(5));
    assert_ne!(
        duplicate.mechanisms,
        original.pattern_definitions()[0].mechanisms
    );
    assert_eq!(duplicate.mechanisms[0].id(), PatternMechanismId(9));
    assert_eq!(duplicate.mechanisms[1].id(), PatternMechanismId(10));
    assert_eq!(duplicate.name, original.pattern_definitions()[0].name);
    assert_eq!(
        duplicate.coverage,
        original.pattern_definitions()[0].coverage
    );
    assert_eq!(duplicate.output_layers.len(), 1);
    assert_eq!(duplicate.output_layers[0].id(), PatternOutputLayerId(5));
    assert!(duplicate.supported_straight_grid_compatibility().is_some());

    let before_failure = history.document().clone();
    let before_revision = history.revision();
    assert!(
        history
            .apply(&DocumentCommand::RemoveUnreferencedPatternDefinition {
                definition_id: PATTERN_ID
            })
            .is_err()
    );
    assert_eq!(history.document(), &before_failure);
    assert_eq!(history.revision(), before_revision);

    let copy_result = history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: CHANNEL_ID,
            base_definition: history.document().pattern_definitions()[0].clone(),
            edit: PatternDefinitionEdit::SetCoverage {
                coverage: CoveragePolicy {
                    guard_steps: 3,
                    maximum_support_radius: 5.0,
                },
            },
        })
        .unwrap();
    assert_eq!(copy_result.affected_channels, vec![CHANNEL_ID]);
    let selected = history.document().channel(CHANNEL_ID).unwrap();
    assert_eq!(selected.pattern_definition_id, PatternDefinitionId(6));
    assert_eq!(
        history
            .document()
            .channel(ChannelId(8))
            .unwrap()
            .pattern_definition_id,
        PATTERN_ID
    );
    assert_eq!(
        history.document().pattern_definitions()[0],
        original.pattern_definitions()[0]
    );

    let shared_result = history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PATTERN_ID,
            base_definition: history.document().pattern_definitions()[0].clone(),
            edit: PatternDefinitionEdit::SetCoverage {
                coverage: CoveragePolicy {
                    guard_steps: 3,
                    maximum_support_radius: 5.0,
                },
            },
        })
        .unwrap();
    assert_eq!(shared_result.affected_channels, vec![ChannelId(8)]);
    let after = history.document().clone();
    assert_eq!(history.undo().unwrap(), Some(shared_result.clone()));
    assert_eq!(history.redo().unwrap(), Some(shared_result));
    assert_eq!(history.document(), &after);

    let stale_before = history.document().clone();
    assert!(
        history
            .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: CHANNEL_ID,
                base_definition: original.pattern_definitions()[0].clone(),
                edit: PatternDefinitionEdit::SetCoverage {
                    coverage: CoveragePolicy {
                        guard_steps: 4,
                        maximum_support_radius: 5.0,
                    },
                },
            })
            .is_err()
    );
    assert_eq!(history.document(), &stale_before);
}

#[test]
fn definition_commands_cover_retarget_remove_unshared_stale_and_semantic_noops() {
    let mut history = shared_history();
    history
        .apply(&DocumentCommand::AddPatternDefinition {
            definition: PatternDefinitionDraft {
                name: "other".into(),
                coverage: CoveragePolicy {
                    guard_steps: 2,
                    maximum_support_radius: 5.0,
                },
            },
        })
        .unwrap();
    let other = PatternDefinitionId(4);
    history
        .apply(&DocumentCommand::AddPatternDefinition {
            definition: PatternDefinitionDraft {
                name: "too-small-support".into(),
                coverage: CoveragePolicy {
                    guard_steps: 2,
                    maximum_support_radius: 4.0,
                },
            },
        })
        .unwrap();
    let incompatible_before = history.document().clone();
    assert!(
        history
            .apply(&DocumentCommand::RetargetChannelPatternDefinition {
                channel_id: ChannelId(8),
                definition_id: PatternDefinitionId(5),
            })
            .is_err()
    );
    assert_eq!(history.document(), &incompatible_before);
    let retarget = history
        .apply(&DocumentCommand::RetargetChannelPatternDefinition {
            channel_id: ChannelId(8),
            definition_id: other,
        })
        .unwrap();
    assert_eq!(retarget.affected_channels, vec![ChannelId(8)]);
    let before_fail = history.document().clone();
    let revision = history.revision();
    for command in [
        DocumentCommand::RetargetChannelPatternDefinition {
            channel_id: ChannelId(8),
            definition_id: other,
        },
        DocumentCommand::RetargetChannelPatternDefinition {
            channel_id: ChannelId(8),
            definition_id: PatternDefinitionId(99),
        },
        DocumentCommand::RemoveUnreferencedPatternDefinition {
            definition_id: PATTERN_ID,
        },
        DocumentCommand::RemoveUnreferencedPatternDefinition {
            definition_id: PatternDefinitionId(99),
        },
    ] {
        assert!(history.apply(&command).is_err());
        assert_eq!(history.document(), &before_fail);
        assert_eq!(history.revision(), revision);
    }
    history
        .apply(&DocumentCommand::RetargetChannelPatternDefinition {
            channel_id: CHANNEL_ID,
            definition_id: other,
        })
        .unwrap();
    let removed = history
        .apply(&DocumentCommand::RemoveUnreferencedPatternDefinition {
            definition_id: PATTERN_ID,
        })
        .unwrap();
    assert!(removed.affected_channels.is_empty());
    assert_eq!(history.undo().unwrap(), Some(removed.clone()));
    history
        .apply(&DocumentCommand::AddPatternDefinition {
            definition: PatternDefinitionDraft {
                name: "branch".into(),
                coverage: CoveragePolicy {
                    guard_steps: 2,
                    maximum_support_radius: 5.0,
                },
            },
        })
        .unwrap();
    assert!(!history.can_redo());

    let mut unshared = DocumentHistory::new(DocumentSession::new(document()).unwrap());
    let base = unshared.document().pattern_definitions()[0].clone();
    unshared
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: CHANNEL_ID,
            base_definition: base.clone(),
            edit: PatternDefinitionEdit::SetCoverage {
                coverage: CoveragePolicy {
                    guard_steps: 3,
                    maximum_support_radius: 5.0,
                },
            },
        })
        .unwrap();
    assert_eq!(unshared.document().pattern_definitions().len(), 1);
    assert_eq!(unshared.document().pattern_definitions()[0].id, PATTERN_ID);
    let unchanged = unshared.document().clone();
    let unchanged_revision = unshared.revision();
    assert!(
        unshared
            .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: CHANNEL_ID,
                base_definition: unshared.document().pattern_definitions()[0].clone(),
                edit: PatternDefinitionEdit::SetCoverage {
                    coverage: unshared.document().pattern_definitions()[0]
                        .coverage
                        .clone(),
                },
            })
            .is_err()
    );
    assert_eq!(unshared.document(), &unchanged);
    assert_eq!(unshared.revision(), unchanged_revision);

    let mut shared = shared_history();
    let stale_base = shared.document().pattern_definitions()[0].clone();
    let shared_result = shared
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PATTERN_ID,
            base_definition: stale_base.clone(),
            edit: PatternDefinitionEdit::SetCoverage {
                coverage: CoveragePolicy {
                    guard_steps: 3,
                    maximum_support_radius: 5.0,
                },
            },
        })
        .unwrap();
    assert_eq!(
        shared_result.affected_channels,
        vec![CHANNEL_ID, ChannelId(8)]
    );
    let stale_document = shared.document().clone();
    let stale_revision = shared.revision();
    assert!(
        shared
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PATTERN_ID,
                base_definition: stale_base,
                edit: PatternDefinitionEdit::SetCoverage {
                    coverage: CoveragePolicy {
                        guard_steps: 4,
                        maximum_support_radius: 5.0,
                    },
                },
            })
            .is_err()
    );
    assert_eq!(shared.document(), &stale_document);
    assert_eq!(shared.revision(), stale_revision);
}

fn legacy_channel() -> ChannelState {
    ChannelState {
        id: CHANNEL_ID,
        pattern_definition_id: PATTERN_ID,
        layout: layout(),
        appearance: ChannelAppearance {
            visible: true,
            color: color(0.1, 0.2, 0.3),
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

fn layout() -> ChannelPatternLayout {
    ChannelPatternLayout {
        density: DensityMetric2D {
            across_x: 90.0,
            across_y: 60.0,
            aspect_locked: true,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
    }
}

fn color(red: f64, green: f64, blue: f64) -> ColorValue {
    ColorValue {
        red,
        green,
        blue,
        alpha: 1.0,
    }
}

fn template() -> ChannelTopologyTemplate {
    ChannelTopologyTemplate {
        pattern_definition_id: PATTERN_ID,
        layout: layout(),
        mark_geometry_response: MarkGeometryResponse {
            minimum_size: 2.0,
            maximum_size: 9.0,
        },
    }
}

fn modeled_channel(
    role: HalftoneChannelRole,
    id: ChannelId,
    component: SourceMappingComponent,
    paint: ChannelPaint,
) -> ModeledChannelState {
    ModeledChannelState {
        role,
        id,
        pattern_definition_id: PATTERN_ID,
        layout: layout(),
        mark_geometry_response: MarkGeometryResponse {
            minimum_size: 2.0,
            maximum_size: 9.0,
        },
        mapping: SourceMapping::canonical(component),
        paint,
        visible: true,
        opacity: 1.0,
    }
}

fn rgb_topology() -> ChannelTopology {
    ChannelTopology::new(vec![
        modeled_channel(
            HalftoneChannelRole::Red,
            ChannelId(20),
            SourceMappingComponent::Red,
            ChannelPaint::Solid(color(1.0, 0.0, 0.0)),
        ),
        modeled_channel(
            HalftoneChannelRole::Green,
            ChannelId(10),
            SourceMappingComponent::Green,
            ChannelPaint::Solid(color(0.0, 1.0, 0.0)),
        ),
        modeled_channel(
            HalftoneChannelRole::Blue,
            ChannelId(30),
            SourceMappingComponent::Blue,
            ChannelPaint::Solid(color(0.0, 0.0, 1.0)),
        ),
    ])
}

fn history() -> DocumentHistory {
    DocumentHistory::new(DocumentSession::new(document()).unwrap())
}

fn round_trip(history: &mut DocumentHistory, command: DocumentCommand) {
    let before_document = history.document().clone();
    let before_revision = history.revision();
    let result = history.apply(&command).unwrap();
    let after_document = history.document().clone();
    assert_eq!(history.revision(), Revision(before_revision.0 + 1));
    assert!(history.can_undo());
    assert!(!history.can_redo());

    assert_eq!(history.undo().unwrap(), Some(result.clone()));
    assert_eq!(history.document(), &before_document);
    assert_eq!(history.revision(), Revision(before_revision.0 + 2));
    assert!(history.can_redo());

    assert_eq!(history.redo().unwrap(), Some(result));
    assert_eq!(history.document(), &after_document);
    assert_eq!(history.revision(), Revision(before_revision.0 + 3));
    assert!(history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn history_round_trips_every_command_with_exact_documents_and_results() {
    let mut history = history();
    round_trip(
        &mut history,
        DocumentCommand::SetDensity {
            channel_id: CHANNEL_ID,
            density: DensityMetric2D {
                across_x: 80.0,
                across_y: 50.0,
                aspect_locked: false,
            },
        },
    );
    round_trip(
        &mut history,
        DocumentCommand::SetRotation {
            channel_id: CHANNEL_ID,
            rotation_degrees: 17.0,
        },
    );
    round_trip(
        &mut history,
        DocumentCommand::SetTranslation {
            channel_id: CHANNEL_ID,
            translation_x: 3.0,
            translation_y: -2.0,
        },
    );
    round_trip(
        &mut history,
        DocumentCommand::SetMarkGeometryResponse {
            channel_id: CHANNEL_ID,
            response: MarkGeometryResponse {
                minimum_size: 1.0,
                maximum_size: 8.0,
            },
        },
    );
    round_trip(
        &mut history,
        DocumentCommand::SetColor {
            channel_id: CHANNEL_ID,
            color: color(0.6, 0.5, 0.4),
        },
    );
    round_trip(
        &mut history,
        DocumentCommand::SetOpacity {
            channel_id: CHANNEL_ID,
            opacity: 0.4,
        },
    );
    round_trip(
        &mut history,
        DocumentCommand::SetVisibility {
            channel_id: CHANNEL_ID,
            visible: false,
        },
    );
    round_trip(
        &mut history,
        DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(SourceReferenceId::new("after-source").unwrap()),
        },
    );
    round_trip(
        &mut history,
        DocumentCommand::SetSourceMapping {
            channel_id: CHANNEL_ID,
            mapping: ChannelSourceMapping {
                component: SourceComponent::Alpha,
                placement: SourcePlacement::StretchToCanvas,
            },
        },
    );

    let topology = history
        .document()
        .canonical_channel_topology(HalftoneChannelModel::Rgb, template())
        .unwrap();
    round_trip(
        &mut history,
        DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Rgb,
            topology: topology.clone(),
        },
    );
    assert_eq!(history.document().channel_topology(), Some(&topology));

    round_trip(
        &mut history,
        DocumentCommand::SetTopologySourceMapping {
            channel_id: ChannelId(1),
            mapping: SourceMapping {
                component: SourceMappingComponent::Red,
                placement: SourcePlacement::StretchToCanvas,
                inverted: true,
                gain: 0.8,
                bias: 0.1,
            },
        },
    );
    round_trip(
        &mut history,
        DocumentCommand::SetChannelPaint {
            channel_id: ChannelId(1),
            paint: ChannelPaint::Solid(color(0.2, 0.3, 0.4)),
        },
    );
}

#[test]
fn history_preserves_topology_and_source_result_ordering() {
    let mut history = history();
    let topology = rgb_topology();
    let topology_result = history
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Rgb,
            topology: topology.clone(),
        })
        .unwrap();
    assert_eq!(
        topology_result.affected_channels,
        vec![CHANNEL_ID, ChannelId(20), ChannelId(10), ChannelId(30)]
    );
    assert_eq!(history.undo().unwrap(), Some(topology_result.clone()));
    assert_eq!(history.redo().unwrap(), Some(topology_result));
    assert_eq!(history.document().channel_topology(), Some(&topology));

    let source_result = history
        .apply(&DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(
                SourceReferenceId::new("replacement-source").unwrap(),
            ),
        })
        .unwrap();
    assert_eq!(
        source_result.affected_channels,
        vec![ChannelId(20), ChannelId(10), ChannelId(30)]
    );
    assert_eq!(history.undo().unwrap(), Some(source_result.clone()));
    assert_eq!(history.redo().unwrap(), Some(source_result));
}

#[test]
fn history_stack_order_empty_branching_failures_and_noops_are_atomic() {
    let mut history = history();
    let initial = history.document().clone();
    assert_eq!(history.undo().unwrap(), None);
    assert_eq!(history.redo().unwrap(), None);
    assert_eq!(history.revision(), Revision(0));

    let first = history
        .apply(&DocumentCommand::SetOpacity {
            channel_id: CHANNEL_ID,
            opacity: 0.5,
        })
        .unwrap();
    let after_first = history.document().clone();
    let second = history
        .apply(&DocumentCommand::SetVisibility {
            channel_id: CHANNEL_ID,
            visible: false,
        })
        .unwrap();
    let after_second = history.document().clone();
    assert_eq!(history.undo().unwrap(), Some(second.clone()));
    assert_eq!(history.document(), &after_first);
    assert_eq!(history.undo().unwrap(), Some(first.clone()));
    assert_eq!(history.document(), &initial);
    assert_eq!(history.redo().unwrap(), Some(first));
    assert_eq!(history.document(), &after_first);
    assert_eq!(history.redo().unwrap(), Some(second.clone()));
    assert_eq!(history.document(), &after_second);
    assert_eq!(history.undo().unwrap(), Some(second));

    let before_failed_branch = history.document().clone();
    let before_failed_revision = history.revision();
    assert!(
        history
            .apply(&DocumentCommand::SetOpacity {
                channel_id: CHANNEL_ID,
                opacity: 2.0,
            })
            .is_err()
    );
    assert_eq!(history.document(), &before_failed_branch);
    assert_eq!(history.revision(), before_failed_revision);
    assert!(history.can_redo());

    let branch = history
        .apply(&DocumentCommand::SetRotation {
            channel_id: CHANNEL_ID,
            rotation_degrees: 12.0,
        })
        .unwrap();
    assert!(history.can_undo());
    assert!(!history.can_redo());
    assert_eq!(history.redo().unwrap(), None);
    assert_eq!(history.undo().unwrap(), Some(branch));

    let no_op_before = history.document().clone();
    let no_op_revision = history.revision();
    let no_op = history
        .apply(&DocumentCommand::SetOpacity {
            channel_id: CHANNEL_ID,
            opacity: 0.5,
        })
        .unwrap();
    assert_eq!(history.document(), &no_op_before);
    assert_eq!(history.revision(), Revision(no_op_revision.0 + 1));
    assert_eq!(history.undo().unwrap(), Some(no_op));
    assert_eq!(history.document(), &no_op_before);
}

#[test]
fn history_invalidates_legacy_and_complete_document_tokens_after_every_transition() {
    let mut history = history();
    let legacy = history.session().evaluation_snapshot(CHANNEL_ID).unwrap();
    let complete = history.session().document_evaluation_snapshot();
    history
        .apply(&DocumentCommand::SetVisibility {
            channel_id: CHANNEL_ID,
            visible: false,
        })
        .unwrap();
    assert!(!history.session().accepts_evaluation(legacy.token()));
    assert!(
        !history
            .session()
            .accepts_document_evaluation(complete.token())
    );
    let current_legacy = history.session().evaluation_snapshot(CHANNEL_ID).unwrap();
    let current_complete = history.session().document_evaluation_snapshot();
    assert!(history.session().accepts_evaluation(current_legacy.token()));
    assert!(
        history
            .session()
            .accepts_document_evaluation(current_complete.token())
    );

    history.undo().unwrap();
    assert!(!history.session().accepts_evaluation(current_legacy.token()));
    assert!(
        !history
            .session()
            .accepts_document_evaluation(current_complete.token())
    );
    let undo_legacy = history.session().evaluation_snapshot(CHANNEL_ID).unwrap();
    let undo_complete = history.session().document_evaluation_snapshot();
    assert!(history.session().accepts_evaluation(undo_legacy.token()));
    assert!(
        history
            .session()
            .accepts_document_evaluation(undo_complete.token())
    );

    history.redo().unwrap();
    assert!(!history.session().accepts_evaluation(undo_legacy.token()));
    assert!(
        !history
            .session()
            .accepts_document_evaluation(undo_complete.token())
    );
    let redo_legacy = history.session().evaluation_snapshot(CHANNEL_ID).unwrap();
    let redo_complete = history.session().document_evaluation_snapshot();
    assert!(history.session().accepts_evaluation(redo_legacy.token()));
    assert!(
        history
            .session()
            .accepts_document_evaluation(redo_complete.token())
    );
}
