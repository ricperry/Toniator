use toniator_domain::*;

/// Builds one authored path output shared by the document's three named channels.
/// # Panics
/// Panics if the current finite structural recipe or modeled document fails validation.
fn document() -> Document {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 120.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let mut definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(900),
        "Tone authority",
        PatternMechanismId(901),
        PatternMechanismId(902),
        PatternOutputLayerId(903),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(905),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(904),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(905), GuideDimensionId(904)],
            merge_epsilon: 1e-9,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = (903..=903)
        .map(|id| {
            PatternOutputLayer::all(
                PatternOutputLayerId(id),
                PatternOutputRealization::ConnectionPaths {
                    site_mechanism_id: PatternMechanismId(902),
                    style: PathStrokeStyle::default(),
                    selection: ConnectionSelectionIntent::default(),
                    program: ConnectionProgram::RandomLinks {
                        adjacency: ConnectionAdjacencyIntent {
                            maximum_degree: 4,
                            maximum_distance: 20.0,
                        },
                        minimum_degree: 0,
                        seed: 17,
                    },
                },
            )
        })
        .collect();
    let output_settings = definition
        .output_layers
        .iter()
        .map(|output| PatternOutputSettings {
            output_layer_id: output.id,
            response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.0,
                maximum_thickness: 1.0,
                bias: 0.0,
            }),
        })
        .collect();
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition.id;
    Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![PatternDefinitionBundle {
            definition,
            output_settings,
        }],
        settings,
        base.channel_model().unwrap().to_owned(),
        base.channel_topology().unwrap().clone(),
        vec![],
    )
    .unwrap()
}

/// Reads exact per-output selection after history or local-ID remapping.
/// # Panics
/// Panics if the requested output is absent or not a connected-points output.
fn policy(
    document: &Document,
    definition: PatternDefinitionId,
    ordinal: usize,
) -> ConnectionSelectionIntent {
    match &document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == definition)
        .unwrap()
        .definition
        .output_layers[ordinal]
        .realization
    {
        PatternOutputRealization::ConnectionPaths { selection, .. } => *selection,
        _ => panic!("connection output"),
    }
}
/// Exercises authoritative bounds, disabled number rejection, output isolation and exact history.
/// # Panics
/// Panics if current descriptor/command outcomes disagree or sibling policy changes.
#[test]
fn final_controls_have_full_u32_bounds_and_targeted_family_history() {
    let original = document();
    let base = original.pattern_definition_bundles()[0].definition.clone();
    let descriptor = original
        .property_descriptors()
        .into_iter()
        .find(|v| v.field == PropertyFieldId::ConnectionMaximumFinalDegree)
        .unwrap();
    assert!(!descriptor.enabled);
    assert_eq!(descriptor.bounds.unwrap().maximum, Some(u32::MAX as f64));
    assert_eq!(descriptor.invalidation, InvalidationLevel::Family);
    assert!(descriptor.copy_on_edit_escalates_to_family);
    let command = |base: PatternDefinition, edit| DocumentCommand::EditSharedPatternDefinition {
        definition_id: base.id,
        base_definition: base,
        edit,
    };
    assert!(
        original
            .apply_command(&command(
                base.clone(),
                PatternDefinitionEdit::SetConnectionMaximumFinalDegree {
                    output_layer_id: PatternOutputLayerId(903),
                    value: 3
                }
            ))
            .is_err()
    );
    let mut history = DocumentHistory::new(DocumentSession::new(original.clone()).unwrap());
    let result = history
        .apply(&command(
            base.clone(),
            PatternDefinitionEdit::SetConnectionFinalDegreeLimit {
                output_layer_id: PatternOutputLayerId(903),
                enabled: true,
            },
        ))
        .unwrap();
    assert_eq!(result.invalidation, Some(InvalidationLevel::Family));
    assert_eq!(
        policy(history.document(), base.id, 0).maximum_final_degree,
        Some(2)
    );
    let enabled = history.document().clone();
    history.undo().unwrap();
    assert_eq!(history.document(), &original);
    history.redo().unwrap();
    assert_eq!(history.document(), &enabled);
    let base = history.document().pattern_definition_bundles()[0]
        .definition
        .clone();
    history
        .apply(&command(
            base.clone(),
            PatternDefinitionEdit::SetConnectionMaximumFinalDegree {
                output_layer_id: PatternOutputLayerId(903),
                value: u32::MAX,
            },
        ))
        .unwrap();
    assert_eq!(
        policy(history.document(), base.id, 0).maximum_final_degree,
        Some(u32::MAX)
    );
    let before = history.document().clone();
    let base = before.pattern_definition_bundles()[0].definition.clone();
    assert!(
        history
            .apply(&command(
                base,
                PatternDefinitionEdit::SetConnectionMinimumComponentPoints {
                    output_layer_id: PatternOutputLayerId(903),
                    value: 0
                }
            ))
            .is_err()
    );
    assert_eq!(history.document(), &before);
}
/// Proves named copy remaps the selected output without modifying the sibling or shared selection.
/// # Panics
/// Panics if copy-on-edit, full history restoration or field readback loses authored intent.
#[test]
fn named_selection_edit_copies_only_recipient_and_remaps_output() {
    let original = document();
    let base = original.pattern_definition_bundles()[0].definition.clone();
    let channel = original.channel_topology().unwrap().channels()[0].id;
    let mut history = DocumentHistory::new(DocumentSession::new(original.clone()).unwrap());
    let outcome = history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: channel,
            base_definition: base.clone(),
            edit: PatternDefinitionEdit::SetConnectionMinimumComponentPoints {
                output_layer_id: PatternOutputLayerId(903),
                value: 3,
            },
        })
        .unwrap();
    assert_eq!(outcome.invalidation, Some(InvalidationLevel::Family));
    let selected = history
        .document()
        .effective_channel_pattern(channel)
        .unwrap()
        .definition_id;
    assert_ne!(selected, base.id);
    assert_eq!(
        policy(history.document(), selected, 0).minimum_component_points,
        3
    );
    assert_eq!(
        policy(history.document(), base.id, 0).minimum_component_points,
        1
    );
    history.undo().unwrap();
    assert_eq!(history.document(), &original);
}
/// Keeps dormant loop intent and independent degree policy across intrinsic forest transitions.
/// # Panics
/// Panics if tree applicability/readback lies or a program transition erases selection.
#[test]
fn tree_loop_control_is_intrinsic_and_program_switch_preserves_policy() {
    let original = document();
    let base = original.pattern_definition_bundles()[0].definition.clone();
    let mut history = DocumentHistory::new(DocumentSession::new(original).unwrap());
    let tree = ConnectionProgram::GridSpanningTree {
        adjacency: ConnectionAdjacencyIntent {
            maximum_degree: 4,
            maximum_distance: 20.,
        },
        algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
        seed: 17,
    };
    history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: base.id,
            base_definition: base,
            edit: PatternDefinitionEdit::SetConnectionProgram {
                output_layer_id: PatternOutputLayerId(903),
                program: tree,
            },
        })
        .unwrap();
    let value = history
        .document()
        .property_values()
        .into_iter()
        .find(|v| v.descriptor.field == PropertyFieldId::ConnectionAllowLoops)
        .unwrap();
    assert!(!value.descriptor.enabled);
    assert_eq!(value.value, PropertyCurrentValueKind::Boolean(false));
    let base = history.document().pattern_definition_bundles()[0]
        .definition
        .clone();
    assert!(
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: base.id,
                base_definition: base.clone(),
                edit: PatternDefinitionEdit::SetConnectionAllowLoops {
                    output_layer_id: PatternOutputLayerId(903),
                    allow: false
                }
            })
            .is_err()
    );
    assert_eq!(
        policy(history.document(), base.id, 0).cycles,
        ConnectionCyclePolicy::Allow
    );
    history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: base.id,
            base_definition: base,
            edit: PatternDefinitionEdit::SetConnectionProgram {
                output_layer_id: PatternOutputLayerId(903),
                program: ConnectionProgram::NearestLinks {
                    adjacency: ConnectionAdjacencyIntent {
                        maximum_degree: 4,
                        maximum_distance: 20.,
                    },
                },
            },
        })
        .unwrap();
    let value = history
        .document()
        .property_values()
        .into_iter()
        .find(|v| v.descriptor.field == PropertyFieldId::ConnectionAllowLoops)
        .unwrap();
    assert!(value.descriptor.enabled);
    assert_eq!(value.value, PropertyCurrentValueKind::Boolean(true));
}
