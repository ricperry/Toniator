use toniator_domain::*;

/// Projects public modeled topology and bundle authority for current integration checks.
trait Inspect {
    fn test_channels(&self) -> Vec<ChannelId>;
    fn test_definition(&self, id: PatternDefinitionId) -> Option<&PatternDefinition>;
}
impl Inspect for Document {
    /// Reads current modeled channel order without depending on private domain helpers.
    /// # Panics
    /// Panics when a fixture lacks the required modeled channel topology.
    fn test_channels(&self) -> Vec<ChannelId> {
        self.channel_topology()
            .unwrap()
            .channels()
            .iter()
            .map(|channel| channel.id)
            .collect()
    }
    /// Resolves one definition from the public immutable bundle collection.
    fn test_definition(&self, id: PatternDefinitionId) -> Option<&PatternDefinition> {
        self.pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == id)
            .map(|bundle| &bundle.definition)
    }
}

/// Publishes one complete configuration against its captured revision as one history entry.
/// # Panics
/// Panics if a validated fixture configuration cannot publish.
fn publish(history: &mut DocumentHistory, configuration: DocumentConfiguration) {
    let before = history.document().clone();
    history
        .apply_document_configuration(&before, history.revision(), &configuration)
        .unwrap();
}

/// Builds independent guide Patterns with distinct phases for ALL assignment preservation.
/// # Panics
/// Panics if the current starter or scoped typed edits reject canonical fixture inputs.
fn independent_guides() -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    let recipe = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides);
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: history.document().pattern_settings().clone(),
            base_definition: history
                .document()
                .test_definition(history.document().pattern_settings().definition_id)
                .unwrap()
                .clone(),
            recipe,
        })
        .unwrap();
    for (index, channel) in history.document().test_channels().into_iter().enumerate() {
        let definition = history
            .document()
            .pattern_definition_for(channel)
            .unwrap()
            .clone();
        let (mechanism_id, dimension_id) = definition
            .mechanisms
            .iter()
            .find_map(|mechanism| match mechanism {
                PatternMechanism::StraightGuideDimensions { id, dimensions } => {
                    Some((*id, dimensions[0].id))
                }
                _ => None,
            })
            .unwrap();
        history
            .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: channel,
                base_definition: definition,
                edit: PatternDefinitionEdit::SetGuidePhase {
                    mechanism_id,
                    dimension_id,
                    phase: (index + 1) as f64 * 0.1,
                },
            })
            .unwrap();
    }
    history
}

/// Reads exact embedded shape geometry using public document identities.
fn mark_shape_matches(
    document: &Document,
    output: &PatternOutputLayer,
    shape: Option<&AuthoredStructureDraft>,
) -> bool {
    match (&output.realization, shape) {
        (
            PatternOutputRealization::CircularMarks { .. }
            | PatternOutputRealization::MarkPrototype {
                prototype: MarkPrototype::Circle,
                ..
            },
            None,
        ) => true,
        (
            PatternOutputRealization::MarkPrototype {
                prototype: MarkPrototype::AuthoredClosedShape { structure_id },
                ..
            },
            Some(shape),
        ) => document
            .authored_structure(*structure_id)
            .is_some_and(|resource| {
                resource.kind() == shape.kind() && resource.segments() == shape.segments()
            }),
        _ => false,
    }
}

/// Supplies two distinct closed silhouettes for replacement and bounded-resource checks.
///
/// # Panics
/// Panics if fixed test polygon data violates authored geometry validation.
fn mark_shape_fixture(tip: f64) -> AuthoredStructureDraft {
    let points = [
        AuthoredPoint2 { x: -1.0, y: 1.0 },
        AuthoredPoint2 { x: 0.0, y: tip },
        AuthoredPoint2 { x: 1.0, y: 1.0 },
    ];
    AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        (0..3)
            .map(|index| AuthoredCurveSegment::Line {
                start: points[index],
                end: points[(index + 1) % 3],
            })
            .collect(),
    )
    .unwrap()
}

/// Exercises default CircularMarks, bounded cycling, exact no-op, analytic restoration and Undo.
///
/// # Panics
/// Panics on lost placement/settings, leaked obsolete shapes or broken reversible publication.
#[test]
fn shape_library_default_grid_cycles_without_resource_growth_and_preserves_settings() {
    for named in [false, true] {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 120.0,
                height: 80.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap();
        let channel = document.test_channels()[0];
        let original = document.clone();
        let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
        let first = mark_shape_fixture(-1.0);
        let second = mark_shape_fixture(-0.5);
        for shape in [&first, &second, &first] {
            let before = history.document().clone();
            let definition = before.pattern_definition_for(channel).unwrap().clone();
            let output = definition.output_layers[0].id;
            let configuration = before
                .edit_pattern_mark_shape_configuration(
                    &definition,
                    output,
                    named.then_some(channel),
                    Some(shape),
                )
                .unwrap();
            publish(&mut history, configuration);
            let after = history.document().pattern_definition_for(channel).unwrap();
            assert_eq!(
                after
                    .mechanisms
                    .iter()
                    .map(std::mem::discriminant)
                    .collect::<Vec<_>>(),
                definition
                    .mechanisms
                    .iter()
                    .map(std::mem::discriminant)
                    .collect::<Vec<_>>()
            );
            let after_effective = history
                .document()
                .effective_channel_pattern(channel)
                .unwrap();
            let before_effective = before.effective_channel_pattern(channel).unwrap();
            assert_eq!(after_effective.density, before_effective.density);
            assert_eq!(
                after_effective.pattern_rotation_degrees,
                before_effective.pattern_rotation_degrees
            );
            assert_eq!(
                after_effective.shape_rotation_degrees,
                before_effective.shape_rotation_degrees
            );
            assert_eq!(
                history
                    .document()
                    .effective_channel_pattern(channel)
                    .unwrap()
                    .output_settings
                    .iter()
                    .map(|setting| &setting.response)
                    .collect::<Vec<_>>(),
                before
                    .effective_channel_pattern(channel)
                    .unwrap()
                    .output_settings
                    .iter()
                    .map(|setting| &setting.response)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                history.document().authored_structures().len(),
                1,
                "cycled shapes remain bounded"
            );
            let noop = history
                .document()
                .edit_pattern_mark_shape_configuration(
                    after,
                    after.output_layers[0].id,
                    named.then_some(channel),
                    Some(shape),
                )
                .unwrap()
                .bind(history.document())
                .unwrap();
            assert_eq!(&noop, history.document());
        }
        let before_circle = history.document().clone();
        let definition = before_circle.pattern_definition_for(channel).unwrap();
        let configuration = before_circle
            .edit_pattern_mark_shape_configuration(
                definition,
                definition.output_layers[0].id,
                named.then_some(channel),
                None,
            )
            .unwrap();
        publish(&mut history, configuration);
        assert!(matches!(
            history
                .document()
                .pattern_definition_for(channel)
                .unwrap()
                .output_layers[0]
                .realization,
            PatternOutputRealization::MarkPrototype {
                prototype: MarkPrototype::Circle,
                ..
            } | PatternOutputRealization::CircularMarks { .. }
        ));
        assert!(history.document().authored_structures().is_empty());
        history.undo().unwrap();
        assert_eq!(history.document(), &before_circle);
        for _ in 0..3 {
            history.undo().unwrap();
        }
        assert_eq!(history.document(), &original);
    }
}

/// Preserves independent effective definitions and their response/filter/orientation during ALL.
///
/// # Panics
/// Panics on partial fanout, geometry aliases changing unrelated slots or response loss.
#[test]
fn shape_library_all_preserves_independent_intent_and_reselect_updates_mixed_shapes() {
    let mut history = independent_guides();
    let before = history.document().clone();
    let base_id = before.pattern_settings().definition_id;
    let base = before.test_definition(base_id).unwrap();
    let shape = mark_shape_fixture(-1.0);
    let configuration = before
        .edit_pattern_mark_shape_configuration(base, base.output_layers[0].id, None, Some(&shape))
        .unwrap();
    publish(&mut history, configuration);
    for channel in before.test_channels() {
        let prior = before.pattern_definition_for(channel).unwrap();
        let after = history.document().pattern_definition_for(channel).unwrap();
        assert_eq!(prior.mechanisms, after.mechanisms);
        assert_eq!(
            before
                .effective_channel_pattern(channel)
                .unwrap()
                .output_settings,
            history
                .document()
                .effective_channel_pattern(channel)
                .unwrap()
                .output_settings
        );
        assert!(mark_shape_matches(
            history.document(),
            &after.output_layers[0],
            Some(&shape)
        ));
    }
    let channel = before.test_channels()[0];
    let base = history.document().pattern_definition_for(channel).unwrap();
    let other = mark_shape_fixture(-0.4);
    let configuration = history
        .document()
        .edit_pattern_mark_shape_configuration(
            base,
            base.output_layers[0].id,
            Some(channel),
            Some(&other),
        )
        .unwrap();
    publish(&mut history, configuration);
    let base = history.document().test_definition(base_id).unwrap();
    let configuration = history
        .document()
        .edit_pattern_mark_shape_configuration(base, base.output_layers[0].id, None, Some(&shape))
        .unwrap();
    publish(&mut history, configuration);
    assert!(history.document().test_channels().iter().all(|channel| {
        mark_shape_matches(
            history.document(),
            &history
                .document()
                .pattern_definition_for(*channel)
                .unwrap()
                .output_layers[0],
            Some(&shape),
        )
    }));
    history.undo().unwrap();
    history.undo().unwrap();
    history.undo().unwrap();
    assert_eq!(history.document(), &before);
}

/// Retains guide-relative orientation and nondefault response across authored and analytic shapes.
///
/// # Panics
/// Panics if shape replacement resets compatible intent or changes an unrelated painter output.
#[test]
fn shape_library_preserves_oriented_mark_response() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    let mut recipe = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides);
    if let PatternStructureRecipe::GeneralizedStraightGuides { orientation, .. } =
        &mut recipe.structure
    {
        *orientation = MarkOrientationDraft::GuideTangent { dimension_index: 0 };
    }
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: history.document().pattern_settings().clone(),
            base_definition: history
                .document()
                .test_definition(history.document().pattern_settings().definition_id)
                .unwrap()
                .clone(),
            recipe,
        })
        .unwrap();
    history
        .apply(&DocumentCommand::SetAllResponseField {
            base: history.document().response_settings().clone(),
            edit: DocumentResponseFieldEdit::MarkMinimumFill(0.12),
        })
        .unwrap();
    history
        .apply(&DocumentCommand::SetAllResponseField {
            base: history.document().response_settings().clone(),
            edit: DocumentResponseFieldEdit::MarkMaximumFill(0.78),
        })
        .unwrap();
    let before = history.document().clone();
    let base = before
        .test_definition(before.pattern_settings().definition_id)
        .unwrap();
    let shape = mark_shape_fixture(-1.0);
    let configuration = before
        .edit_pattern_mark_shape_configuration(base, base.output_layers[0].id, None, Some(&shape))
        .unwrap();
    publish(&mut history, configuration);
    let after = history.document().test_definition(base.id).unwrap();
    assert_eq!(
        history.document().response_settings(),
        before.response_settings()
    );
    assert_eq!(after.mechanisms, base.mechanisms);
    assert_eq!(
        after.output_layers[0].source_filter,
        base.output_layers[0].source_filter
    );
    match (
        &base.output_layers[0].realization,
        &after.output_layers[0].realization,
    ) {
        (
            PatternOutputRealization::MarkPrototype {
                orientation: prior, ..
            },
            PatternOutputRealization::MarkPrototype {
                orientation: current,
                ..
            },
        ) => assert_eq!(prior, current),
        value => panic!("unexpected oriented mark pair: {value:?}"),
    }
    assert_eq!(
        history
            .document()
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == base.id)
            .unwrap()
            .output_settings,
        before
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == base.id)
            .unwrap()
            .output_settings
    );
    let oriented = history.document().clone();
    let definition = oriented.test_definition(base.id).unwrap();
    let configuration = oriented
        .edit_pattern_mark_shape_configuration(
            definition,
            definition.output_layers[0].id,
            None,
            None,
        )
        .unwrap();
    publish(&mut history, configuration);
    let restored = history.document().test_definition(base.id).unwrap();
    assert_eq!(
        restored.output_layers[0].realization,
        base.output_layers[0].realization
    );
}
