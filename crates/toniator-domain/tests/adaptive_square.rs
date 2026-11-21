use toniator_domain::*;

/// Creates a fresh current document with ordinary native square Guides and modeled channels.
/// # Panics
/// Panics if fixed valid source-free document construction fails.
fn document() -> Document {
    Document::new_default_document(
        CanvasSpec {
            width: 101.0,
            height: 77.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap()
}

/// Publishes named and ALL strategy edits through history and preserves unrelated assignments.
/// # Panics
/// Panics if scope, undo, square metrics or descriptor authority regress.
#[test]
fn adaptive_square_all_named_history_and_descriptor_authority() {
    let original = document();
    let channels = original
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    assert!(channels.len() > 1);
    let mut history = DocumentHistory::new(DocumentSession::new(original.clone()).unwrap());
    let channel = channels[0];
    let base = original.pattern_definition_for(channel).unwrap();
    assert!(base.supports_adaptive_square_marks());
    let named = original
        .edit_pattern_adaptive_square_configuration(base, base.output_layers[0].id, Some(channel))
        .unwrap();
    history
        .apply_document_configuration(&original, history.revision(), &named)
        .unwrap();
    assert!(
        history
            .document()
            .pattern_definition_for(channel)
            .unwrap()
            .uses_adaptive_square_marks()
    );
    assert!(
        !history
            .document()
            .pattern_definition_for(channels[1])
            .unwrap()
            .uses_adaptive_square_marks()
    );
    let effective = history
        .document()
        .effective_channel_pattern(channel)
        .unwrap();
    assert_eq!(effective.density.aspect, 1.0);
    let controls = history
        .document()
        .pattern_capabilities(PatternCapabilityScope::Channel(channel))
        .unwrap()
        .active_controls;
    for field in [
        PropertyFieldId::DensityAspect,
        PropertyFieldId::ShapeRotationDegrees,
        PropertyFieldId::MarkSizeScale,
        PropertyFieldId::OutputOrientation,
    ] {
        assert!(!controls.iter().any(|control| control.field == field));
    }
    history.undo().unwrap();
    assert_eq!(history.document(), &original);
    let all = original
        .edit_pattern_adaptive_square_configuration(base, base.output_layers[0].id, None)
        .unwrap();
    history
        .apply_document_configuration(&original, history.revision(), &all)
        .unwrap();
    for channel in channels {
        assert!(
            history
                .document()
                .pattern_definition_for(channel)
                .unwrap()
                .uses_adaptive_square_marks()
        );
    }
}

/// Rejects non-square guide directions at the same domain boundary used by Shapes eligibility.
/// # Panics
/// Panics if an oblique guide layout silently accepts the square-cell strategy.
#[test]
fn adaptive_square_rejects_oblique_grid() {
    let original = document();
    let recipe =
        PatternDefinitionRecipe::marks(PatternStructureRecipe::GeneralizedStraightGuides {
            name: "Grid".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
            dimensions: [0.0, 90.0]
                .into_iter()
                .map(|angle| GuideDimensionDraft {
                    baseline_angle_degrees: angle,
                    phase: 0.0,
                    spacing_multiplier: 1.0,
                })
                .collect(),
            product: GeneralizedSiteProductDraft::Intersections {
                dimension_indices: vec![0, 1],
                merge_epsilon: 1e-8,
            },
            orientation: MarkOrientationDraft::Fixed,
        });
    let original = original
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: original.pattern_settings().clone(),
            base_definition: original.pattern_definition_bundles()[0].definition.clone(),
            recipe,
        })
        .unwrap()
        .0;
    let mut definition = original
        .pattern_definition_for(ChannelId(1))
        .unwrap()
        .clone();
    let PatternMechanism::StraightGuideDimensions { dimensions, .. } =
        &mut definition.mechanisms[0]
    else {
        panic!("starter straight Guides")
    };
    dimensions[1].baseline_angle_degrees = 60.0;
    assert!(!definition.supports_adaptive_square_marks());
    definition.output_layers[0].realization = PatternOutputRealization::MarkPrototype {
        site_mechanism_id: definition.output_layers[0].site_mechanism_id().unwrap(),
        prototype: MarkPrototype::AdaptiveSquare,
        orientation: MarkOrientation::Fixed,
    };
    assert_eq!(
        validate_pattern_definition(&definition).unwrap_err().path(),
        "pattern.adaptive_square.grid"
    );
}
