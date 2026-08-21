use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureAttachment,
    AuthoredStructureDraft, AuthoredStructureId, AuthoredStructureKind, CanvasSpec, ChannelId,
    CoveragePolicy, Document, DocumentCommand, DocumentHistory, DocumentSession,
    GeneralizedSiteProduct, GuideDimension, GuideDimensionId, GuidePrototype, GuideRepetition,
    InvalidationLevel, MarkOrientation, MarkPrototype, PatternDefinition, PatternDefinitionEdit,
    PatternDefinitionId, PatternMechanismId, PatternOutputLayerId, SourceReference,
};

/// Builds the supported default document used by draft-root history witnesses.
fn document() -> Document {
    Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document")
}

/// Builds a document whose shared generic definition refers to one guide and one mark resource.
fn document_with_typed_guide_and_mark_uses() -> Document {
    let base = document();
    let definition = PatternDefinition::generalized_guides(
        PatternDefinitionId(1),
        "typed guide and mark uses",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        vec![
            GuideDimension {
                id: GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(7),
                },
                repetition: GuideRepetition::Single,
            },
            GuideDimension {
                id: GuideDimensionId(2),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(7),
                },
                repetition: GuideRepetition::Single,
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
            merge_epsilon: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 4.5,
        },
    );
    let guide = AuthoredStructure::new(
        AuthoredStructureId(7),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 0.0 },
            end: AuthoredPoint2 { x: 6.0, y: 0.0 },
        }],
    )
    .expect("guide");
    let first = AuthoredPoint2 { x: 0.0, y: 0.0 };
    let second = AuthoredPoint2 { x: 2.0, y: 0.0 };
    let third = AuthoredPoint2 { x: 1.0, y: 2.0 };
    let mark = AuthoredStructure::new(
        AuthoredStructureId(8),
        AuthoredStructureKind::ClosedShape,
        vec![
            AuthoredCurveSegment::Line {
                start: first,
                end: second,
            },
            AuthoredCurveSegment::Line {
                start: second,
                end: third,
            },
            AuthoredCurveSegment::Line {
                start: third,
                end: first,
            },
        ],
    )
    .expect("mark");
    Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![definition],
        base.pattern_settings().clone(),
        base.channel_model().expect("modeled document").to_owned(),
        base.channel_topology().expect("modeled document").clone(),
        vec![guide, mark],
    )
    .expect("typed resource document")
}

/// Translates one authored resource into a valid replacement draft without changing its kind.
fn translated_draft(structure: &AuthoredStructure, x_offset: f64) -> AuthoredStructureDraft {
    let translate = |point: AuthoredPoint2| AuthoredPoint2 {
        x: point.x + x_offset,
        y: point.y,
    };
    AuthoredStructureDraft::new(
        structure.kind(),
        structure
            .segments()
            .iter()
            .map(|segment| match segment {
                AuthoredCurveSegment::Line { start, end } => AuthoredCurveSegment::Line {
                    start: translate(*start),
                    end: translate(*end),
                },
                AuthoredCurveSegment::CubicBezier {
                    start,
                    control_1,
                    control_2,
                    end,
                } => AuthoredCurveSegment::CubicBezier {
                    start: translate(*start),
                    control_1: translate(*control_1),
                    control_2: translate(*control_2),
                    end: translate(*end),
                },
            })
            .collect(),
    )
    .expect("translated draft")
}

/// Proves private draft squash publishes one main undo entry and preserves ordinary redo branching.
#[test]
fn private_draft_squash_is_one_main_undo_step_and_clears_redo() {
    let mut main = DocumentHistory::new(DocumentSession::new(document()).expect("session"));
    let channel = ChannelId(1);
    main.apply(&DocumentCommand::SetVisibility {
        channel_id: channel,
        visible: false,
    })
    .expect("main edit");
    main.undo().expect("main undo");
    assert!(main.can_redo());
    let mut draft = DocumentHistory::new_draft(&main);
    draft
        .apply(&DocumentCommand::SetVisibility {
            channel_id: channel,
            visible: false,
        })
        .expect("draft edit");
    let summary = main.squash_draft(&draft).expect("squash");
    assert!(!summary.unchanged);
    assert_eq!(summary.affected_channels, vec![channel]);
    assert_eq!(summary.invalidation, Some(InvalidationLevel::Presentation));
    assert!(main.can_undo());
    assert!(!main.can_redo());
    main.undo().expect("single squash undo");
    assert!(
        main.document()
            .modeled_channel(channel)
            .expect("modeled channel")
            .visible
    );
}

/// Proves unchanged and stale drafts leave the corresponding main and draft histories intact.
#[test]
fn private_draft_squash_rejects_stale_roots_and_keeps_unchanged_drafts_noop() {
    let mut main = DocumentHistory::new(DocumentSession::new(document()).expect("session"));
    let unchanged = DocumentHistory::new_draft(&main);
    assert!(
        main.squash_draft(&unchanged)
            .expect("unchanged squash")
            .unchanged
    );
    assert!(!main.can_undo());
    let stale = DocumentHistory::new_draft(&main);
    main.apply(&DocumentCommand::SetVisibility {
        channel_id: ChannelId(1),
        visible: false,
    })
    .expect("main edit");
    let before = main.document().clone();
    assert_eq!(
        main.squash_draft(&stale)
            .expect_err("stale draft")
            .to_string(),
        "document.draft: private draft root is stale against the main history"
    );
    assert_eq!(main.document(), &before);
}

/// Proves undone private entries do not publish and retained draft redo stays local.
#[test]
fn private_draft_squash_ignores_undone_entries() {
    let mut main = DocumentHistory::new(DocumentSession::new(document()).expect("session"));
    let mut draft = DocumentHistory::new_draft(&main);
    draft
        .apply(&DocumentCommand::SetVisibility {
            channel_id: ChannelId(1),
            visible: false,
        })
        .expect("draft edit");
    draft.undo().expect("draft undo");
    assert!(draft.can_redo());
    let result = main.squash_draft(&draft).expect("unchanged publication");
    assert!(result.unchanged);
    assert!(!main.can_undo());
    assert!(draft.can_redo());
}

/// Proves one added authored resource retains its exact allocated ID through a draft squash.
#[test]
fn private_draft_squash_preserves_created_authored_structure_id() {
    let mut main = DocumentHistory::new(DocumentSession::new(document()).expect("session"));
    let mut draft = DocumentHistory::new_draft(&main);
    let result = draft
        .apply(&DocumentCommand::AddAuthoredStructure {
            draft: AuthoredStructureDraft::new(
                AuthoredStructureKind::OpenPath,
                vec![AuthoredCurveSegment::Line {
                    start: AuthoredPoint2 { x: 1.0, y: 2.0 },
                    end: AuthoredPoint2 { x: 4.0, y: 2.0 },
                }],
            )
            .expect("draft path"),
        })
        .expect("add resource");
    let id = result.created_authored_structure_id.expect("allocated ID");
    assert!(!main.squash_draft(&draft).expect("squash").unchanged);
    assert_eq!(
        main.document()
            .authored_structure(id)
            .map(AuthoredStructure::id),
        Some(id)
    );
}

/// Proves the resource-use projection is deterministic and presentation-free for a document without uses.
#[test]
fn authored_structure_use_projection_is_empty_and_stable_without_references() {
    let document = document();
    assert!(document.authored_structure_uses().is_empty());
    assert_eq!(
        document.authored_structure_uses(),
        document.authored_structure_uses()
    );
}

/// Proves guide and mark uses project in stable channel-local typed order with their exact IDs.
#[test]
fn authored_structure_uses_project_real_guides_and_marks_in_order() {
    let mut history = DocumentHistory::new(
        DocumentSession::new(document_with_typed_guide_and_mark_uses()).expect("session"),
    );
    let base = history.document().pattern_definitions()[0].clone();
    history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(1),
            base_definition: base,
            edit: PatternDefinitionEdit::SetOutputMarkPrototype {
                output_layer_id: PatternOutputLayerId(1),
                prototype: MarkPrototype::AuthoredClosedShape {
                    structure_id: AuthoredStructureId(8),
                },
            },
        })
        .expect("mark reference");
    let uses = history.document().authored_structure_uses();
    assert!(uses.len() >= 3);
    assert!(matches!(
        uses[0],
        toniator_domain::AuthoredStructureUse::Guide {
            structure_id: AuthoredStructureId(7),
            ..
        }
    ));
    assert!(matches!(
        uses[1],
        toniator_domain::AuthoredStructureUse::Guide {
            structure_id: AuthoredStructureId(7),
            ..
        }
    ));
    assert!(matches!(
        uses[2],
        toniator_domain::AuthoredStructureUse::Mark {
            structure_id: AuthoredStructureId(8),
            ..
        }
    ));
    assert_eq!(uses, history.document().authored_structure_uses());
}

/// Proves structural draft aggregation follows document channel order and reports its strongest invalidation.
#[test]
fn structural_draft_squash_orders_affected_channels_and_aggregates_invalidation() {
    let document = document_with_typed_guide_and_mark_uses();
    let expected_channels = document
        .channel_topology()
        .expect("modeled topology")
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    let mut main = DocumentHistory::new(DocumentSession::new(document).expect("session"));
    let mut draft = DocumentHistory::new_draft(&main);
    for id in [AuthoredStructureId(7), AuthoredStructureId(8)] {
        let base = draft
            .document()
            .authored_structure(id)
            .expect("resource")
            .clone();
        draft
            .apply(&DocumentCommand::ReplaceAuthoredStructure {
                replacement: translated_draft(&base, 1.0),
                base_structure: base,
            })
            .expect("replacement");
    }
    let summary = main.squash_draft(&draft).expect("structural squash");
    assert_eq!(summary.affected_channels, expected_channels);
    assert_eq!(summary.invalidation, Some(InvalidationLevel::Family));
}

/// Proves new guide and mark resources attach to exact selected-channel targets as one undo entry.
#[test]
fn new_authored_resources_attach_atomically_for_guide_and_mark_targets() {
    for (attachment, source_id) in [
        (
            AuthoredStructureAttachment::Guide {
                mechanism_id: PatternMechanismId(1),
                dimension_id: GuideDimensionId(1),
            },
            AuthoredStructureId(7),
        ),
        (
            AuthoredStructureAttachment::Mark {
                output_layer_id: PatternOutputLayerId(1),
            },
            AuthoredStructureId(8),
        ),
    ] {
        let mut history = DocumentHistory::new(
            DocumentSession::new(document_with_typed_guide_and_mark_uses()).expect("session"),
        );
        let draft = translated_draft(
            history
                .document()
                .authored_structure(source_id)
                .expect("source structure"),
            1.0,
        );
        let result = history
            .add_and_attach_authored_structure(ChannelId(1), attachment, draft)
            .expect("atomic attachment");
        let created = result
            .created_authored_structure_id
            .expect("exact created ID");
        assert_eq!(
            history
                .document()
                .authored_structures()
                .last()
                .map(AuthoredStructure::id),
            Some(created)
        );
        assert!(
            history
                .document()
                .authored_structure_uses()
                .iter()
                .any(|usage| {
                    usage.structure_id() == created
                        && matches!(
                            usage,
                            toniator_domain::AuthoredStructureUse::Guide {
                                channel_id: ChannelId(1),
                                ..
                            } | toniator_domain::AuthoredStructureUse::Mark {
                                channel_id: ChannelId(1),
                                ..
                            }
                        )
                })
        );
        history.undo().expect("one grouped undo");
        assert!(history.document().authored_structure(created).is_none());
    }
}

/// Proves ordinary circular Grid output promotes to one authored mark attachment atomically.
#[test]
fn ordinary_grid_mark_attachment_promotes_the_circular_output() {
    let mut history = DocumentHistory::new(DocumentSession::new(document()).expect("session"));
    let draft = AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                end: AuthoredPoint2 { x: 3.0, y: 0.0 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 3.0, y: 0.0 },
                end: AuthoredPoint2 { x: 1.5, y: 2.0 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 1.5, y: 2.0 },
                end: AuthoredPoint2 { x: 0.0, y: 0.0 },
            },
        ],
    )
    .expect("closed mark");
    let result = history
        .add_and_attach_authored_structure(
            ChannelId(1),
            AuthoredStructureAttachment::Mark {
                output_layer_id: PatternOutputLayerId(1),
            },
            draft,
        )
        .expect("ordinary output promotion");
    let created = result
        .created_authored_structure_id
        .expect("allocated mark");
    assert!(history.document().authored_structure_uses().iter().any(|use_value| {
        matches!(use_value, toniator_domain::AuthoredStructureUse::Mark { channel_id: ChannelId(1), structure_id, .. } if *structure_id == created)
    }));
    history.undo().expect("one atomic undo");
    assert!(history.document().authored_structure(created).is_none());
}

/// Proves a confirmed ordinary Grid guide transition creates one selected-channel
/// authored along-guide layout while preserving linked channels and one undo boundary.
#[test]
fn ordinary_grid_guide_attachment_creates_selected_custom_along_guide_layout() {
    let mut history = DocumentHistory::new(DocumentSession::new(document()).expect("session"));
    let base_definition = history
        .document()
        .pattern_definition_for(ChannelId(1))
        .expect("selected default definition")
        .clone();
    let linked_before = history.document().linked_channels(base_definition.id);
    let draft = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 1.0, y: 1.0 },
            end: AuthoredPoint2 { x: 6.0, y: 1.0 },
        }],
    )
    .expect("open guide");
    let result = history
        .add_and_attach_authored_structure(
            ChannelId(1),
            AuthoredStructureAttachment::GuideCustomAlongLayout,
            draft,
        )
        .expect("selected custom guide attachment");
    let created = result
        .created_authored_structure_id
        .expect("allocated guide");
    let selected_definition = history
        .document()
        .pattern_definition_for(ChannelId(1))
        .expect("fresh selected definition");
    assert_ne!(selected_definition.id, base_definition.id);
    assert!(
        selected_definition
            .mechanisms
            .iter()
            .any(|mechanism| matches!(
                mechanism,
                toniator_domain::PatternMechanism::AlongGuideSites { .. }
            ))
    );
    assert!(history.document().authored_structure_uses().iter().any(|use_value| {
        matches!(use_value, toniator_domain::AuthoredStructureUse::Guide { channel_id: ChannelId(1), structure_id, .. } if *structure_id == created)
    }));
    for channel in linked_before
        .into_iter()
        .filter(|channel| *channel != ChannelId(1))
    {
        assert_eq!(
            history
                .document()
                .pattern_definition_for(channel)
                .map(|definition| definition.id),
            Some(base_definition.id)
        );
    }
    history.undo().expect("one atomic undo");
    assert!(history.document().authored_structure(created).is_none());
    assert_eq!(
        history
            .document()
            .pattern_definition_for(ChannelId(1))
            .map(|definition| definition.id),
        Some(base_definition.id)
    );
}
