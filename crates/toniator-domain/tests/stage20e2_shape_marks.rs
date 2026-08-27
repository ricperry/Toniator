use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureDraft,
    AuthoredStructureId, AuthoredStructureKind, CanvasSpec, ChannelId, CoveragePolicy, Document,
    DocumentCommand, DocumentHistory, DocumentSession, GeneralizedSiteProduct, GuideDimension,
    GuideDimensionId, GuidePrototype, GuideRepetition, InvalidationLevel, MarkOrientation,
    MarkPrototype, MarkPrototypeKind, PatternDefinition, PatternDefinitionEdit,
    PatternDefinitionId, PatternMechanismId, PatternOutputLayerId, PatternOutputRealization,
    PropertyCurrentValueKind, PropertyEnumChoice, PropertyFieldId, PropertyReferenceConstraint,
    PropertyReferenceValue, PropertyTarget, SourceReference, StraightGuideDimension,
    StraightGuideRepetition, VariantTransitionFieldUpdate, VariantTransitionValue,
};

/// Builds one finite line segment without adding implicit closure or repair semantics.
fn line(start: AuthoredPoint2, end: AuthoredPoint2) -> AuthoredCurveSegment {
    AuthoredCurveSegment::Line { start, end }
}

/// Builds a reusable closed triangle or an open line with a caller-owned stable identity.
fn structure(id: u64, kind: AuthoredStructureKind, offset: f64) -> AuthoredStructure {
    let segments = match kind {
        AuthoredStructureKind::OpenPath => vec![line(
            AuthoredPoint2 { x: offset, y: 0.0 },
            AuthoredPoint2 {
                x: offset + 1.0,
                y: 0.0,
            },
        )],
        AuthoredStructureKind::ClosedShape => {
            let first = AuthoredPoint2 { x: offset, y: 0.0 };
            let second = AuthoredPoint2 {
                x: offset + 2.0,
                y: 0.0,
            };
            let third = AuthoredPoint2 {
                x: offset + 1.0,
                y: 2.0,
            };
            vec![line(first, second), line(second, third), line(third, first)]
        }
    };
    AuthoredStructure::new(AuthoredStructureId(id), kind, segments).unwrap()
}

/// Builds one shared typed circle definition plus closed/open resources and the current topology.
fn shape_reference_document() -> Document {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default modeled document");
    let definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(1),
        "shape references",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(2),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
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
    );
    let mut bundle = base.pattern_definition_bundles()[0].clone();
    bundle.definition = definition;
    Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![bundle],
        base.pattern_settings().clone(),
        base.channel_model().expect("modeled document").to_owned(),
        base.channel_topology().expect("modeled document").clone(),
        vec![
            structure(7, AuthoredStructureKind::ClosedShape, 0.0),
            structure(8, AuthoredStructureKind::ClosedShape, 10.0),
            structure(9, AuthoredStructureKind::OpenPath, 20.0),
        ],
    )
    .unwrap()
}

/// Builds a generic-guide document whose default channels share one open authored path.
fn shared_guide_document() -> Document {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default modeled document");
    let prototype = GuidePrototype::AuthoredOpenPath {
        structure_id: AuthoredStructureId(7),
    };
    let definition = PatternDefinition::generalized_guides(
        PatternDefinitionId(1),
        "shared guide",
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
    Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![{
            let mut bundle = base.pattern_definition_bundles()[0].clone();
            bundle.definition = definition;
            bundle
        }],
        base.pattern_settings().clone(),
        base.channel_model().expect("modeled document").to_owned(),
        base.channel_topology().expect("modeled document").clone(),
        vec![structure(7, AuthoredStructureKind::OpenPath, 0.0)],
    )
    .expect("generic guide document")
}

/// Returns the current shared output prototype without accepting legacy adapter layers.
fn output_prototype(document: &Document, definition_id: PatternDefinitionId) -> &MarkPrototype {
    match &current_definition(document, definition_id).output_layers[0].realization {
        PatternOutputRealization::MarkPrototype { prototype, .. } => prototype,
        PatternOutputRealization::CircularMarks { .. } => {
            panic!("the fixture owns one typed mark layer")
        }
        _ => panic!("the fixture owns one mark output"),
    }
}

/// Resolves one fixture definition through the public ordered document collection.
fn current_definition(
    document: &Document,
    definition_id: PatternDefinitionId,
) -> &PatternDefinition {
    document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == definition_id)
        .map(|bundle| &bundle.definition)
        .unwrap()
}

/// Applies one deliberate shared edit against the exact current immutable definition base.
fn apply_shared(
    history: &mut DocumentHistory,
    edit: PatternDefinitionEdit,
) -> toniator_domain::CommandResult {
    let definition_id = PatternDefinitionId(1);
    let base_definition = current_definition(history.document(), definition_id).clone();
    history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id,
            base_definition,
            edit,
        })
        .unwrap()
}

/// Proves authored-shape selection requires an explicit compatible stable ID and publishes the
/// singular active descriptor plus shared-channel realization invalidation atomically.
#[test]
fn authored_shape_variant_requires_explicit_typed_reference_and_retargets_atomically() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(shape_reference_document()).unwrap());
    let selector = history
        .document()
        .property_descriptors()
        .into_iter()
        .find(|descriptor| {
            descriptor.field == PropertyFieldId::OutputPrototype
                && descriptor.target
                    == PropertyTarget::OutputLayer(PatternDefinitionId(1), PatternOutputLayerId(1))
        })
        .unwrap();
    let draft = history
        .document()
        .variant_transition_draft(
            &selector,
            PropertyEnumChoice::MarkPrototype(MarkPrototypeKind::AuthoredClosedShape),
        )
        .unwrap();
    assert_eq!(draft.fields().len(), 1);
    let reference = &draft.fields()[0];
    assert_eq!(reference.field, PropertyFieldId::OutputAuthoredClosedShape);
    assert_eq!(
        reference.value,
        VariantTransitionValue::StableReference(None)
    );
    assert_eq!(
        reference.reference_choices,
        vec![
            PropertyReferenceValue::AuthoredStructure(AuthoredStructureId(7)),
            PropertyReferenceValue::AuthoredStructure(AuthoredStructureId(8)),
        ]
    );
    assert!(draft.finalize(history.document()).is_err());
    for invalid_id in [AuthoredStructureId(9), AuthoredStructureId(999)] {
        assert!(
            draft
                .with_updates(&[VariantTransitionFieldUpdate {
                    field: PropertyFieldId::OutputAuthoredClosedShape,
                    target: PropertyTarget::OutputLayer(
                        PatternDefinitionId(1),
                        PatternOutputLayerId(1),
                    ),
                    value: VariantTransitionValue::StableReference(Some(
                        PropertyReferenceValue::AuthoredStructure(invalid_id),
                    )),
                }])
                .is_err()
        );
    }
    let draft = draft
        .with_updates(&[VariantTransitionFieldUpdate {
            field: PropertyFieldId::OutputAuthoredClosedShape,
            target: PropertyTarget::OutputLayer(PatternDefinitionId(1), PatternOutputLayerId(1)),
            value: VariantTransitionValue::StableReference(Some(
                PropertyReferenceValue::AuthoredStructure(AuthoredStructureId(7)),
            )),
        }])
        .unwrap();
    let edit = draft.finalize(history.document()).unwrap();
    let result = apply_shared(&mut history, edit);
    assert_eq!(result.invalidation, Some(InvalidationLevel::Realization));
    assert_eq!(
        result.affected_channels,
        vec![ChannelId(1), ChannelId(2), ChannelId(3)]
    );
    assert_eq!(
        output_prototype(history.document(), PatternDefinitionId(1)),
        &MarkPrototype::AuthoredClosedShape {
            structure_id: AuthoredStructureId(7)
        }
    );
    let value = history
        .document()
        .property_values()
        .into_iter()
        .find(|value| value.descriptor.field == PropertyFieldId::OutputAuthoredClosedShape)
        .unwrap();
    assert_eq!(
        value.descriptor.reference_constraint,
        PropertyReferenceConstraint::Singular
    );
    assert_eq!(
        value.descriptor.invalidation,
        InvalidationLevel::Realization
    );
    assert_eq!(
        value.value,
        PropertyCurrentValueKind::Reference(PropertyReferenceValue::AuthoredStructure(
            AuthoredStructureId(7)
        ))
    );
    let retarget = apply_shared(
        &mut history,
        PatternDefinitionEdit::SetOutputAuthoredClosedShape {
            output_layer_id: PatternOutputLayerId(1),
            structure_id: AuthoredStructureId(8),
        },
    );
    assert_eq!(retarget.invalidation, Some(InvalidationLevel::Realization));
    assert_eq!(
        retarget.affected_channels,
        vec![ChannelId(1), ChannelId(2), ChannelId(3)]
    );
    let before_noop = history.document().clone();
    let revision = history.revision();
    let base_definition = current_definition(history.document(), PatternDefinitionId(1)).clone();
    let no_op = history.apply(&DocumentCommand::EditSharedPatternDefinition {
        definition_id: PatternDefinitionId(1),
        base_definition,
        edit: PatternDefinitionEdit::SetOutputAuthoredClosedShape {
            output_layer_id: PatternOutputLayerId(1),
            structure_id: AuthoredStructureId(8),
        },
    });
    assert!(no_op.is_err());
    assert_eq!(history.document(), &before_noop);
    assert_eq!(history.revision(), revision);
}

/// Proves definition/resource duplication, referenced removal, replacement effects, stale bases,
/// and undo/redo preserve stable authored-shape references without implicit retargeting.
#[test]
fn authored_shape_reference_lifecycle_is_shared_failure_atomic_and_history_backed() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(shape_reference_document()).unwrap());
    let stale_base = current_definition(history.document(), PatternDefinitionId(1)).clone();
    apply_shared(
        &mut history,
        PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id: PatternOutputLayerId(1),
            prototype: MarkPrototype::AuthoredClosedShape {
                structure_id: AuthoredStructureId(8),
            },
        },
    );
    let duplicate_resource = history
        .apply(&DocumentCommand::DuplicateAuthoredStructure {
            structure_id: AuthoredStructureId(8),
        })
        .unwrap();
    assert_eq!(
        duplicate_resource.created_authored_structure_id,
        Some(AuthoredStructureId(10))
    );
    assert_eq!(
        output_prototype(history.document(), PatternDefinitionId(1)),
        &MarkPrototype::AuthoredClosedShape {
            structure_id: AuthoredStructureId(8)
        }
    );
    history
        .apply(&DocumentCommand::DuplicatePatternDefinition {
            definition_id: PatternDefinitionId(1),
        })
        .unwrap();
    let duplicate_definition = &history
        .document()
        .pattern_definition_bundles()
        .last()
        .unwrap()
        .definition;
    assert_ne!(duplicate_definition.id, PatternDefinitionId(1));
    assert_eq!(
        output_prototype(history.document(), duplicate_definition.id),
        &MarkPrototype::AuthoredClosedShape {
            structure_id: AuthoredStructureId(8)
        }
    );
    let before_remove = history.document().clone();
    let revision = history.revision();
    assert!(
        history
            .apply(&DocumentCommand::RemoveUnreferencedAuthoredStructure {
                structure_id: AuthoredStructureId(8),
            })
            .is_err()
    );
    assert_eq!(history.document(), &before_remove);
    assert_eq!(history.revision(), revision);
    let old_shape = history
        .document()
        .authored_structure(AuthoredStructureId(8))
        .unwrap()
        .clone();
    let replacement = structure(88, AuthoredStructureKind::ClosedShape, 30.0);
    let replace = history
        .apply(&DocumentCommand::ReplaceAuthoredStructure {
            base_structure: old_shape.clone(),
            replacement: toniator_domain::AuthoredStructureDraft::new(
                replacement.kind(),
                replacement.segments().to_vec(),
            )
            .unwrap(),
        })
        .unwrap();
    assert_eq!(replace.invalidation, Some(InvalidationLevel::Realization));
    assert_eq!(
        replace.affected_channels,
        vec![ChannelId(1), ChannelId(2), ChannelId(3)]
    );
    let replaced = history
        .document()
        .authored_structure(AuthoredStructureId(8))
        .unwrap()
        .clone();
    assert_ne!(replaced, old_shape);
    history.undo().unwrap();
    assert_eq!(
        history
            .document()
            .authored_structure(AuthoredStructureId(8))
            .unwrap(),
        &old_shape
    );
    history.redo().unwrap();
    assert_eq!(
        history
            .document()
            .authored_structure(AuthoredStructureId(8))
            .unwrap(),
        &replaced
    );
    let before_stale = history.document().clone();
    let revision = history.revision();
    assert!(
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(1),
                base_definition: stale_base,
                edit: PatternDefinitionEdit::SetOutputAuthoredClosedShape {
                    output_layer_id: PatternOutputLayerId(1),
                    structure_id: AuthoredStructureId(7),
                },
            })
            .is_err()
    );
    assert_eq!(history.document(), &before_stale);
    assert_eq!(history.revision(), revision);
}

/// Proves a selected mark use projects deterministically and duplicate-retarget stays one undo step.
#[test]
fn authored_structure_use_copy_retargets_one_selected_mark_use_atomically() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(shape_reference_document()).unwrap());
    apply_shared(
        &mut history,
        PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id: PatternOutputLayerId(1),
            prototype: MarkPrototype::AuthoredClosedShape {
                structure_id: AuthoredStructureId(7),
            },
        },
    );
    let selected = history
        .document()
        .authored_structure_uses()
        .into_iter()
        .next()
        .expect("one channel-owned mark use");
    let before_revision = history.revision();
    let result = history
        .duplicate_and_retarget_authored_structure(selected)
        .expect("grouped copy and retarget");
    assert_eq!(
        result.created_authored_structure_id,
        Some(AuthoredStructureId(10))
    );
    assert_eq!(history.revision().0, before_revision.0 + 1);
    assert!(history.can_undo());
    history.undo().expect("one grouped undo");
    assert_eq!(history.document().authored_structures().len(), 3);
}

/// Proves a shared guide copy retargets the exact selected guide use and replaces it in one undo step.
#[test]
fn shared_guide_copy_retarget_and_replacement_stay_one_undo_step() {
    let mut history = DocumentHistory::new(DocumentSession::new(shared_guide_document()).unwrap());
    let selected = history
        .document()
        .authored_structure_uses()
        .into_iter()
        .find(|use_value| {
            matches!(
                use_value,
                toniator_domain::AuthoredStructureUse::Guide { .. }
            )
        })
        .expect("one selected guide use");
    let replacement = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![line(
            AuthoredPoint2 { x: 1.0, y: 0.0 },
            AuthoredPoint2 { x: 3.0, y: 0.0 },
        )],
    )
    .expect("edited guide remains open");
    let result = history
        .duplicate_retarget_and_replace_authored_structure(selected, replacement)
        .expect("grouped guide copy and replacement");
    assert_eq!(
        result.created_authored_structure_id,
        Some(AuthoredStructureId(8))
    );
    assert_eq!(history.document().authored_structures().len(), 2);
    history.undo().expect("one grouped guide undo");
    assert_eq!(history.document().authored_structures().len(), 1);
}

/// Proves a shared mark copy, retarget, and replacement are one exact-ID draft history entry.
#[test]
fn shared_mark_copy_retarget_and_replacement_stay_one_undo_step() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(shape_reference_document()).unwrap());
    apply_shared(
        &mut history,
        PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id: PatternOutputLayerId(1),
            prototype: MarkPrototype::AuthoredClosedShape {
                structure_id: AuthoredStructureId(7),
            },
        },
    );
    let selected = history
        .document()
        .authored_structure_uses()
        .into_iter()
        .next()
        .expect("one selected mark use");
    let original = history
        .document()
        .authored_structure(AuthoredStructureId(7))
        .expect("selected resource exists");
    let translate = |point: AuthoredPoint2| AuthoredPoint2 {
        x: point.x + 1.0,
        y: point.y,
    };
    let replacement = AuthoredStructureDraft::new(
        original.kind(),
        original
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
    .expect("replacement remains a closed shape");
    let result = history
        .duplicate_retarget_and_replace_authored_structure(selected, replacement)
        .expect("grouped shared copy and replacement");
    assert_eq!(
        result.created_authored_structure_id,
        Some(AuthoredStructureId(10))
    );
    assert_eq!(history.document().authored_structures().len(), 4);
    assert!(
        history
            .document()
            .authored_structure_uses()
            .iter()
            .any(|use_value| use_value.structure_id() == AuthoredStructureId(10))
    );
    history.undo().expect("one grouped undo");
    assert_eq!(history.document().authored_structures().len(), 3);
    assert!(
        history
            .document()
            .authored_structure_uses()
            .iter()
            .all(|use_value| use_value.structure_id() != AuthoredStructureId(10))
    );
}

/// Proves an invalid grouped shared copy leaves the private-history document and revision unchanged.
#[test]
fn invalid_shared_mark_copy_replacement_is_atomic() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(shape_reference_document()).unwrap());
    apply_shared(
        &mut history,
        PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id: PatternOutputLayerId(1),
            prototype: MarkPrototype::AuthoredClosedShape {
                structure_id: AuthoredStructureId(7),
            },
        },
    );
    let selected = history
        .document()
        .authored_structure_uses()
        .into_iter()
        .next()
        .expect("one selected mark use");
    let before = history.document().clone();
    let revision = history.revision();
    let invalid = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![line(
            AuthoredPoint2 { x: 0.0, y: 0.0 },
            AuthoredPoint2 { x: 1.0, y: 0.0 },
        )],
    )
    .expect("draft itself is a valid open path");
    assert!(
        history
            .duplicate_retarget_and_replace_authored_structure(selected, invalid)
            .is_err()
    );
    assert_eq!(history.document(), &before);
    assert_eq!(history.revision(), revision);
}
