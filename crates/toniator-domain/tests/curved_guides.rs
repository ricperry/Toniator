use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureDraft,
    AuthoredStructureId, AuthoredStructureKind, CanvasSpec, CoveragePolicy, Document,
    DocumentCommand, DocumentHistory, DocumentId, DocumentSession, GeneralizedSiteProduct,
    GuideDimension, GuideDimensionId, GuidePrototype, GuideRepetition, InvalidationLevel,
    MarkOrientation, PatternDefinition, PatternDefinitionEdit, PatternDefinitionId,
    PatternMechanismId, PatternOutputLayerId, SourceReference,
};

/// Builds one valid document-owned open path used by generic guide validation tests.
fn open_path() -> AuthoredStructure {
    AuthoredStructure::new(
        AuthoredStructureId(7),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 0.0 },
            end: AuthoredPoint2 { x: 10.0, y: 0.0 },
        }],
    )
    .unwrap()
}

/// Builds a minimal generic definition whose references must resolve through the document store.
fn definition(prototype: GuidePrototype) -> PatternDefinition {
    PatternDefinition::generalized_guides(
        PatternDefinitionId(1),
        "curves",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        vec![
            GuideDimension {
                id: GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: prototype.clone(),
                repetition: GuideRepetition::Single,
            },
            GuideDimension {
                id: GuideDimensionId(2),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                prototype,
                repetition: GuideRepetition::TransformStack {
                    direction_degrees: 0.0,
                    spacing_multiplier: 1.0,
                },
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
    )
}

/// Builds a complete modeled document whose existing channels all reference the generic test root.
fn document_with_generic_guides(prototype: GuidePrototype) -> Document {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default modeled document is valid");
    Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![definition(prototype)],
        base.channel_model().expect("modeled document").to_owned(),
        base.channel_topology().expect("modeled document").clone(),
        vec![open_path()],
    )
    .expect("generic guide root preserves existing channel references")
}

/// Proves prototypes, bounded repetitions, resource references, and existing products validate together.
#[test]
fn generic_guide_definitions_validate_prototypes_repetition_references_and_products() {
    let guide_definition = definition(GuidePrototype::AuthoredOpenPath {
        structure_id: AuthoredStructureId(7),
    });
    let result = Document::with_source_topology_and_authored_structures(
        DocumentId(1),
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
        vec![guide_definition],
        toniator_domain::HalftoneChannelModel::Rgb,
        Document::new_default_document(
            CanvasSpec {
                width: 100.0,
                height: 100.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap()
        .channel_topology()
        .unwrap()
        .clone(),
        vec![open_path()],
    );
    assert!(
        result.is_ok(),
        "open-path guide references resolve through the owning document: {result:?}"
    );
    let arc = GuidePrototype::CircularArc {
        center: AuthoredPoint2 { x: 0.0, y: 0.0 },
        radius: 2.0,
        start_angle_degrees: 0.0,
        sweep_angle_degrees: 90.0,
    };
    toniator_domain::validate_pattern_definition(&definition(arc)).unwrap();
}

/// Proves generic definitions reject malformed arc payloads before any command/history transition.
#[test]
fn generic_guide_edits_descriptors_history_and_affected_channels_are_atomic() {
    let invalid = definition(GuidePrototype::CircularArc {
        center: AuthoredPoint2 {
            x: f64::NAN,
            y: 0.0,
        },
        radius: 1.0,
        start_angle_degrees: 0.0,
        sweep_angle_degrees: 90.0,
    });
    assert_eq!(
        toniator_domain::validate_pattern_definition(&invalid)
            .unwrap_err()
            .path(),
        "pattern_definitions.mechanisms.guide_prototype.arc.center"
    );
    let document = document_with_generic_guides(GuidePrototype::AuthoredOpenPath {
        structure_id: AuthoredStructureId(7),
    });
    let descriptors = document.property_descriptors();
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.field == toniator_domain::PropertyFieldId::GuidePrototype)
    );
    assert!(
        descriptors.iter().any(|descriptor| descriptor.field
            == toniator_domain::PropertyFieldId::GuideAuthoredStructure)
    );
    let original = document.pattern_definitions()[0].clone();
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    let edit = DocumentCommand::EditSharedPatternDefinition {
        definition_id: PatternDefinitionId(1),
        base_definition: original.clone(),
        edit: PatternDefinitionEdit::SetGuideBaselineAngle {
            mechanism_id: PatternMechanismId(1),
            dimension_id: GuideDimensionId(1),
            baseline_angle_degrees: 17.0,
        },
    };
    let result = history
        .apply(&edit)
        .expect("shared generic edit validates atomically");
    assert_eq!(result.invalidation, InvalidationLevel::Family);
    let linked = history
        .document()
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    assert_eq!(result.affected_channels, linked);
    assert!(
        history.apply(&edit).is_err(),
        "stale base rejects before history publication"
    );
    history.undo().expect("generic edit is undoable");
    history.redo().expect("generic edit is redoable");
    let base_structure = history
        .document()
        .authored_structure(AuthoredStructureId(7))
        .unwrap()
        .clone();
    let replacement = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 0.0 },
            end: AuthoredPoint2 { x: 20.0, y: 0.0 },
        }],
    )
    .unwrap();
    let affected = history
        .apply(&DocumentCommand::ReplaceAuthoredStructure {
            base_structure,
            replacement,
        })
        .expect("shared open resource replacement is valid");
    assert_eq!(affected.invalidation, InvalidationLevel::Family);
    let linked = history
        .document()
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    assert_eq!(affected.affected_channels, linked);
    let apply_generic = |history: &mut DocumentHistory, edit: PatternDefinitionEdit| {
        let base = history
            .document()
            .pattern_definitions()
            .iter()
            .find(|definition| definition.id == PatternDefinitionId(1))
            .unwrap()
            .clone();
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(1),
                base_definition: base,
                edit,
            })
            .expect("each active Stage 20D payload mutates one validated candidate")
    };
    apply_generic(
        &mut history,
        PatternDefinitionEdit::SetGuidePrototype {
            mechanism_id: PatternMechanismId(1),
            dimension_id: GuideDimensionId(1),
            prototype: GuidePrototype::CircularArc {
                center: AuthoredPoint2 { x: 1.0, y: 2.0 },
                radius: 3.0,
                start_angle_degrees: 4.0,
                sweep_angle_degrees: 90.0,
            },
        },
    );
    for edit in [
        PatternDefinitionEdit::SetGuideArcCenterX {
            mechanism_id: PatternMechanismId(1),
            dimension_id: GuideDimensionId(1),
            value: 5.0,
        },
        PatternDefinitionEdit::SetGuideArcCenterY {
            mechanism_id: PatternMechanismId(1),
            dimension_id: GuideDimensionId(1),
            value: 6.0,
        },
        PatternDefinitionEdit::SetGuideArcRadius {
            mechanism_id: PatternMechanismId(1),
            dimension_id: GuideDimensionId(1),
            value: 7.0,
        },
        PatternDefinitionEdit::SetGuideArcStartAngle {
            mechanism_id: PatternMechanismId(1),
            dimension_id: GuideDimensionId(1),
            value: 8.0,
        },
        PatternDefinitionEdit::SetGuideArcSweepAngle {
            mechanism_id: PatternMechanismId(1),
            dimension_id: GuideDimensionId(1),
            value: 120.0,
        },
    ] {
        apply_generic(&mut history, edit);
    }
    apply_generic(
        &mut history,
        PatternDefinitionEdit::SetGuideRepetition {
            mechanism_id: PatternMechanismId(1),
            dimension_id: GuideDimensionId(1),
            repetition: GuideRepetition::TransformStack {
                direction_degrees: 0.0,
                spacing_multiplier: 1.0,
            },
        },
    );
    apply_generic(
        &mut history,
        PatternDefinitionEdit::SetGuideStackDirection {
            mechanism_id: PatternMechanismId(1),
            dimension_id: GuideDimensionId(1),
            value: 15.0,
        },
    );
    apply_generic(
        &mut history,
        PatternDefinitionEdit::SetGuideStackSpacingMultiplier {
            mechanism_id: PatternMechanismId(1),
            dimension_id: GuideDimensionId(1),
            value: 2.0,
        },
    );
    apply_generic(
        &mut history,
        PatternDefinitionEdit::SetGuideRepetition {
            mechanism_id: PatternMechanismId(1),
            dimension_id: GuideDimensionId(1),
            repetition: GuideRepetition::Single,
        },
    );
    apply_generic(
        &mut history,
        PatternDefinitionEdit::SetGuidePrototype {
            mechanism_id: PatternMechanismId(1),
            dimension_id: GuideDimensionId(1),
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(7),
            },
        },
    );
    let authored_noop_base = history
        .document()
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == PatternDefinitionId(1))
        .unwrap()
        .clone();
    assert!(
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(1),
                base_definition: authored_noop_base,
                edit: PatternDefinitionEdit::SetGuideAuthoredStructure {
                    mechanism_id: PatternMechanismId(1),
                    dimension_id: GuideDimensionId(1),
                    structure_id: AuthoredStructureId(7),
                },
            })
            .is_err(),
        "identical authored-reference payload is a stable semantic no-op"
    );
    let before_invalid = history.document().clone();
    let before_revision = history.revision();
    let base = before_invalid
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == PatternDefinitionId(1))
        .unwrap()
        .clone();
    assert!(
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(1),
                base_definition: base,
                edit: PatternDefinitionEdit::SetGuideSpacingMultiplier {
                    mechanism_id: PatternMechanismId(1),
                    dimension_id: GuideDimensionId(1),
                    spacing_multiplier: 2.0,
                },
            })
            .is_err(),
        "the straight-only payload cannot silently no-op on a generic guide"
    );
    assert_eq!(history.document(), &before_invalid);
    assert_eq!(history.revision(), before_revision);
}

/// Proves raw authored guide references are reusable definition intent rather than copied path state.
#[test]
fn duplicated_definitions_share_authored_guides_and_live_references_block_removal() {
    let value = definition(GuidePrototype::AuthoredOpenPath {
        structure_id: AuthoredStructureId(7),
    });
    assert!(
        matches!(&value.mechanisms[0], toniator_domain::PatternMechanism::GuideDimensions { dimensions, .. } if matches!(dimensions[0].prototype, GuidePrototype::AuthoredOpenPath { structure_id: AuthoredStructureId(7) }))
    );
    let document = document_with_generic_guides(GuidePrototype::AuthoredOpenPath {
        structure_id: AuthoredStructureId(7),
    });
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    let duplicate = history
        .apply(&DocumentCommand::DuplicatePatternDefinition {
            definition_id: PatternDefinitionId(1),
        })
        .expect("definition duplication preserves raw structure sharing");
    assert_eq!(duplicate.invalidation, InvalidationLevel::Family);
    assert!(
        matches!(history.apply(&DocumentCommand::RemoveUnreferencedAuthoredStructure { structure_id: AuthoredStructureId(7) }), Err(error) if error.to_string().contains("authored_structures.remove.referenced"))
    );
}
