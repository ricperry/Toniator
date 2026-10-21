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
        vec![StraightGuideDimension {
            id: GuideDimensionId(904),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(904)],
            interval_multiplier: 1.0,
            phase: 0.0,
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
                PatternOutputRealization::GuidePaths {
                    guide_mechanism_id: PatternMechanismId(901),
                    style: PathStrokeStyle::default(),
                    tone_sample_spacing: ToneSampleSpacing::default(),
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

/// Reads immutable descriptor-owned tone values in output order after remapping/copy/history.
fn tones(document: &Document, definition: PatternDefinitionId) -> Vec<f64> {
    document
        .property_values()
        .into_iter()
        .filter_map(
            |value| match (value.descriptor.target, value.descriptor.field, value.value) {
                (
                    PropertyTarget::OutputLayer(id, _),
                    PropertyFieldId::ToneSampleSpacing,
                    PropertyCurrentValueKind::FiniteF64(value),
                ) if id == definition => Some(value),
                _ => None,
            },
        )
        .collect()
}

/// Proves descriptor/command invalidation, per-channel output isolation, named copy remapping and exact history.
/// # Panics
/// Panics if valid shared/named edits fail or descriptor/readback/history disagree with authority.
#[test]
fn tone_edit_targets_one_output_and_history_preserves_shared_intent() {
    let original = document();
    let base = original.pattern_definition_bundles()[0].definition.clone();
    let descriptor = original
        .property_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.field == PropertyFieldId::ToneSampleSpacing)
        .unwrap();
    assert_eq!(descriptor.invalidation, InvalidationLevel::Realization);
    assert!(descriptor.copy_on_edit_escalates_to_family);
    assert_eq!(descriptor.unit, PropertyUnit::FeatureSizeMultiplier);
    let edit = PatternDefinitionEdit::SetToneSampleSpacing {
        output_layer_id: PatternOutputLayerId(903),
        value: ToneSampleSpacing { multiplier: 2.0 },
    };
    let mut all = DocumentHistory::new(DocumentSession::new(original.clone()).unwrap());
    let outcome = all
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: base.id,
            base_definition: base.clone(),
            edit: edit.clone(),
        })
        .unwrap();
    assert_eq!(outcome.invalidation, Some(InvalidationLevel::Realization));
    assert_eq!(tones(all.document(), base.id), vec![2.0]);
    let all_after = all.document().clone();
    all.undo().unwrap();
    assert_eq!(all.document(), &original);
    all.redo().unwrap();
    assert_eq!(all.document(), &all_after);
    let mut named = DocumentHistory::new(DocumentSession::new(original.clone()).unwrap());
    let channel = original.channel_topology().unwrap().channels()[0].id;
    let sibling = original.channel_topology().unwrap().channels()[1].id;
    let outcome = named
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: channel,
            base_definition: base,
            edit,
        })
        .unwrap();
    assert_eq!(outcome.invalidation, Some(InvalidationLevel::Family));
    let selected = named
        .document()
        .effective_channel_pattern(channel)
        .unwrap()
        .definition_id;
    assert_ne!(
        selected,
        named
            .document()
            .effective_channel_pattern(sibling)
            .unwrap()
            .definition_id
    );
    assert_eq!(tones(named.document(), selected), vec![2.0]);
    assert_eq!(
        tones(named.document(), original.pattern_settings().definition_id),
        vec![1.0]
    );
    let after = named.document().clone();
    named.undo().unwrap();
    assert_eq!(named.document(), &original);
    named.redo().unwrap();
    assert_eq!(named.document(), &after);
}

/// Rejects current invalid tone edits atomically without changing the shared sibling channel.
/// # Panics
/// Panics if invalid finite-positive bounds or an unsupported mark output accepts a tone edit.
#[test]
fn invalid_tone_edits_do_not_change_current_authority() {
    let document = document();
    let base = document.pattern_definition_bundles()[0].definition.clone();
    for multiplier in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            document
                .apply_command(&DocumentCommand::EditSharedPatternDefinition {
                    definition_id: base.id,
                    base_definition: base.clone(),
                    edit: PatternDefinitionEdit::SetToneSampleSpacing {
                        output_layer_id: PatternOutputLayerId(903),
                        value: ToneSampleSpacing { multiplier }
                    }
                })
                .is_err()
        );
    }
    let marks = Document::new_default_document(
        CanvasSpec {
            width: 120.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let base = marks.pattern_definition_bundles()[0].definition.clone();
    assert!(
        marks
            .apply_command(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: base.id,
                base_definition: base.clone(),
                edit: PatternDefinitionEdit::SetToneSampleSpacing {
                    output_layer_id: base.output_layers[0].id,
                    value: ToneSampleSpacing::default()
                }
            })
            .is_err()
    );
    assert!(
        !marks
            .property_descriptors()
            .iter()
            .any(|descriptor| descriptor.field == PropertyFieldId::ToneSampleSpacing)
    );
}

/// Retains nondefault tone intent across applicable output-kind transitions and canonical recipe reconstruction.
/// # Panics
/// Panics if replacing a guide path with a motif resets output-owned spacing.
#[test]
fn applicable_output_transitions_retain_tone_intent() {
    let document = document();
    let base = document.pattern_definition_bundles()[0].definition.clone();
    let (changed, _) = document
        .apply_command(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: base.id,
            base_definition: base,
            edit: PatternDefinitionEdit::SetToneSampleSpacing {
                output_layer_id: PatternOutputLayerId(903),
                value: ToneSampleSpacing { multiplier: 2.5 },
            },
        })
        .unwrap();
    let recipe = changed
        .reconstruct_pattern_definition_recipe(changed.pattern_settings().definition_id)
        .unwrap();
    let motif = recipe
        .with_output_kind(0, PatternRecipeOutputKind::CurveMotif)
        .unwrap();
    let paths = motif
        .with_output_kind(0, PatternRecipeOutputKind::StructuralPaths)
        .unwrap();
    let structure = match paths.structure {
        PatternStructureRecipe::AuthoredResources { definition, .. } => *definition,
        structure => structure,
    };
    let PatternStructureRecipe::OrderedOutputs { outputs, .. } = structure else {
        panic!("canonical ordered output")
    };
    let PatternOutputRealizationRecipe::StructuralPaths {
        tone_sample_spacing,
        ..
    } = outputs[0]
    else {
        panic!("structural path remains applicable")
    };
    assert_eq!(tone_sample_spacing.multiplier, 2.5);
}

/// Keeps graph connections and wall mazes outside tone-field applicability and authoritative editing.
/// # Panics
/// Panics if a valid current connection/maze document gains the curve-only field or accepts its edit.
#[test]
fn connections_and_mazes_reject_curve_tone_intent() {
    for maze in [false, true] {
        let base = document();
        let mut definition = PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(900),
            "Excluded tone output",
            PatternMechanismId(901),
            PatternMechanismId(902),
            PatternOutputLayerId(903),
            [0.0, 90.0]
                .into_iter()
                .enumerate()
                .map(|(i, angle)| StraightGuideDimension {
                    id: GuideDimensionId(904 + i as u64),
                    baseline_angle_degrees: angle,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                })
                .collect(),
            GeneralizedSiteProduct::Intersections {
                dimensions: vec![GuideDimensionId(904), GuideDimensionId(905)],
                merge_epsilon: 1e-9,
            },
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        definition.output_layers[0].realization = if maze {
            PatternOutputRealization::MazeWalls {
                site_mechanism_id: PatternMechanismId(902),
                program: MazeProgram {
                    algorithm: GridMazeAlgorithm::RecursiveBacktracker,
                    seed: 7,
                },
                style: PathStrokeStyle::default(),
            }
        } else {
            PatternOutputRealization::ConnectionPaths {
                site_mechanism_id: PatternMechanismId(902),
                program: ConnectionProgram::NearestLinks {
                    adjacency: ConnectionAdjacencyIntent {
                        maximum_degree: 4,
                        maximum_distance: 40.0,
                    },
                },
                style: PathStrokeStyle::default(),
                selection: ConnectionSelectionIntent::default(),
            }
        };
        let bundle = PatternDefinitionBundle {
            definition: definition.clone(),
            output_settings: base.pattern_definition_bundles()[0].output_settings.clone(),
        };
        let current = Document::with_source_topology_and_authored_structures(
            base.id(),
            base.canvas().clone(),
            base.source().clone(),
            vec![bundle],
            base.pattern_settings().clone(),
            base.channel_model().unwrap().to_owned(),
            base.channel_topology().unwrap().clone(),
            vec![],
        )
        .unwrap();
        assert!(
            !current
                .property_descriptors()
                .iter()
                .any(|descriptor| descriptor.field == PropertyFieldId::ToneSampleSpacing)
        );
        assert!(
            current
                .apply_command(&DocumentCommand::EditSharedPatternDefinition {
                    definition_id: definition.id,
                    base_definition: definition,
                    edit: PatternDefinitionEdit::SetToneSampleSpacing {
                        output_layer_id: PatternOutputLayerId(903),
                        value: ToneSampleSpacing { multiplier: 2.0 }
                    }
                })
                .is_err()
        );
    }
}
