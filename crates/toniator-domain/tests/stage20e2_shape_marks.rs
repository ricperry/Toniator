use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureId,
    AuthoredStructureKind, CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout,
    ChannelSourceMapping, ChannelState, ColorValue, CoveragePolicy, DensityMetric2D, Document,
    DocumentCommand, DocumentHistory, DocumentId, DocumentSession, GeneralizedSiteProduct,
    GuideDimensionId, InvalidationLevel, MarkGeometryResponse, MarkOrientation, MarkPrototype,
    MarkPrototypeKind, PatternDefinition, PatternDefinitionEdit, PatternDefinitionId,
    PatternMechanismId, PatternOutputLayer, PatternOutputLayerId, PropertyCurrentValueKind,
    PropertyEnumChoice, PropertyFieldId, PropertyReferenceConstraint, PropertyReferenceValue,
    PropertyTarget, SourceComponent, SourcePlacement, SourceReference, StraightGuideDimension,
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

/// Builds one shared typed circle definition plus closed/open resources and two linked channels.
fn shape_reference_document() -> Document {
    let definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(10),
        "shape references",
        PatternMechanismId(20),
        PatternMechanismId(21),
        PatternOutputLayerId(30),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(40),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(41),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(40), GuideDimensionId(41)],
            merge_epsilon: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 4.5,
        },
    );
    let channel = |id| ChannelState {
        id: ChannelId(id),
        pattern_definition_id: PatternDefinitionId(10),
        layout: ChannelPatternLayout {
            density: DensityMetric2D {
                across_x: 20.0,
                across_y: 20.0,
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
            opacity: 1.0,
        },
        mark_geometry_response: MarkGeometryResponse {
            minimum_fill: 0.0,
            maximum_fill: 2.0,
            rotation_offset_degrees: 0.0,
        },
        source_mapping: ChannelSourceMapping {
            component: SourceComponent::Luminance,
            placement: SourcePlacement::StretchToCanvas,
        },
    };
    Document::with_source_and_authored_structures(
        DocumentId(1),
        CanvasSpec {
            width: 100.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
        vec![definition],
        vec![channel(1), channel(2)],
        vec![
            structure(7, AuthoredStructureKind::ClosedShape, 0.0),
            structure(8, AuthoredStructureKind::ClosedShape, 10.0),
            structure(9, AuthoredStructureKind::OpenPath, 20.0),
        ],
    )
    .unwrap()
}

/// Returns the current shared output prototype without accepting legacy adapter layers.
fn output_prototype(document: &Document, definition_id: PatternDefinitionId) -> &MarkPrototype {
    match &current_definition(document, definition_id).output_layers[0] {
        PatternOutputLayer::MarkPrototype { prototype, .. } => prototype,
        PatternOutputLayer::CircularMarks { .. } => {
            panic!("the fixture owns one typed mark layer")
        }
    }
}

/// Resolves one fixture definition through the public ordered document collection.
fn current_definition(
    document: &Document,
    definition_id: PatternDefinitionId,
) -> &PatternDefinition {
    document
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == definition_id)
        .unwrap()
}

/// Applies one deliberate shared edit against the exact current immutable definition base.
fn apply_shared(
    history: &mut DocumentHistory,
    edit: PatternDefinitionEdit,
) -> toniator_domain::CommandResult {
    let definition_id = PatternDefinitionId(10);
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
                    == PropertyTarget::OutputLayer(
                        PatternDefinitionId(10),
                        PatternOutputLayerId(30),
                    )
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
                        PatternDefinitionId(10),
                        PatternOutputLayerId(30),
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
            target: PropertyTarget::OutputLayer(PatternDefinitionId(10), PatternOutputLayerId(30)),
            value: VariantTransitionValue::StableReference(Some(
                PropertyReferenceValue::AuthoredStructure(AuthoredStructureId(7)),
            )),
        }])
        .unwrap();
    let edit = draft.finalize(history.document()).unwrap();
    let result = apply_shared(&mut history, edit);
    assert_eq!(result.invalidation, InvalidationLevel::Realization);
    assert_eq!(result.affected_channels, vec![ChannelId(1), ChannelId(2)]);
    assert_eq!(
        output_prototype(history.document(), PatternDefinitionId(10)),
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
            output_layer_id: PatternOutputLayerId(30),
            structure_id: AuthoredStructureId(8),
        },
    );
    assert_eq!(retarget.invalidation, InvalidationLevel::Realization);
    assert_eq!(retarget.affected_channels, vec![ChannelId(1), ChannelId(2)]);
    let before_noop = history.document().clone();
    let revision = history.revision();
    let base_definition = current_definition(history.document(), PatternDefinitionId(10)).clone();
    let no_op = history.apply(&DocumentCommand::EditSharedPatternDefinition {
        definition_id: PatternDefinitionId(10),
        base_definition,
        edit: PatternDefinitionEdit::SetOutputAuthoredClosedShape {
            output_layer_id: PatternOutputLayerId(30),
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
    let stale_base = current_definition(history.document(), PatternDefinitionId(10)).clone();
    apply_shared(
        &mut history,
        PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id: PatternOutputLayerId(30),
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
        output_prototype(history.document(), PatternDefinitionId(10)),
        &MarkPrototype::AuthoredClosedShape {
            structure_id: AuthoredStructureId(8)
        }
    );
    history
        .apply(&DocumentCommand::DuplicatePatternDefinition {
            definition_id: PatternDefinitionId(10),
        })
        .unwrap();
    let duplicate_definition = history.document().pattern_definitions().last().unwrap();
    assert_ne!(duplicate_definition.id, PatternDefinitionId(10));
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
    assert_eq!(replace.invalidation, InvalidationLevel::Realization);
    assert_eq!(replace.affected_channels, vec![ChannelId(1), ChannelId(2)]);
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
                definition_id: PatternDefinitionId(10),
                base_definition: stale_base,
                edit: PatternDefinitionEdit::SetOutputAuthoredClosedShape {
                    output_layer_id: PatternOutputLayerId(30),
                    structure_id: AuthoredStructureId(7),
                },
            })
            .is_err()
    );
    assert_eq!(history.document(), &before_stale);
    assert_eq!(history.revision(), revision);
}
