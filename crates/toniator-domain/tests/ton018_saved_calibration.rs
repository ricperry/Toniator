use toniator_domain::{
    AttachedCalibration, CALIBRATION_PROFILE, CALIBRATION_PROFILE_64,
    CALIBRATION_PROFILE_GUIDE_FACES, CALIBRATION_PROFILE_GUIDE_FACES_64,
    CalibrationBinding, CalibrationKnot,
    CalibrationVariation, CanvasSpec, ChannelId, ChannelTopologyTemplate, CoveragePolicy,
    CurveRepetition, CurveWinding, Document, DocumentCommand, DocumentHistory, DocumentSession,
    GeneralizedSiteProductDraft, GuideDimensionDraft, HalftoneChannelModel, InvalidationLevel,
    MarkOrientationDraft, ParametricCurve, PatternDefinitionRecipe, PatternOutputRealizationRecipe,
    PatternStructureRecipe, PresetMetadata, PresetRecord, PropertyFieldId, PropertyTarget,
    SavedCalibration, SourceReference, SpiralCoveragePolicy, SpiralCurve, SpiralShape,
    UniformCalibration, validate_preset_record,
};

/// Guide Faces profile identity follows the typed active producer at both strict tiers.
///
/// # Panics
/// Panics if generic v4 accepts revised Guide Faces geometry, a Guide Faces profile leaks to an
/// unrelated producer, or the normalized one-output binding loses its Guide Faces predicate.
#[test]
fn guide_faces_profiles_are_producer_scoped_at_both_strict_tiers() {
    let guide_faces = PatternDefinitionRecipe::guide_faces(
        PatternStructureRecipe::GeneralizedStraightGuides {
            name: "Two guides".into(),
            coverage: CoveragePolicy { guard_steps: 2, additional_margin: 0.0 },
            dimensions: [0.0, 90.0].map(|angle| GuideDimensionDraft {
                baseline_angle_degrees: angle,
                phase: 0.0,
                spacing_multiplier: 1.0,
            }).to_vec(),
            product: GeneralizedSiteProductDraft::Intersections {
                dimension_indices: vec![0, 1], merge_epsilon: 1e-9,
            },
            orientation: MarkOrientationDraft::Fixed,
        },
        vec![0, 1],
    );
    let binding = CalibrationBinding::new(guide_faces, 1.0, 0.0).unwrap();
    assert!(binding.uses_guide_faces());
    assert!(!binding.has_variation_seed());
    let raw = vec![
        CalibrationKnot { input: 0.0, coverage: 0.0 },
        CalibrationKnot { input: 2.0, coverage: 1.0 },
    ];
    assert_eq!(SavedCalibration::new(binding.clone(), raw.clone(), None).unwrap().profile(),
        CALIBRATION_PROFILE_GUIDE_FACES);
    assert!(SavedCalibration::with_profile(CALIBRATION_PROFILE_GUIDE_FACES_64.into(),
        binding.clone(), raw.clone(), None).is_ok());
    for former in [CALIBRATION_PROFILE, CALIBRATION_PROFILE_64] {
        assert!(SavedCalibration::with_profile(former.into(), binding.clone(), raw.clone(), None).is_err());
    }
    let straight = CalibrationBinding::for_channel(&default_document(), ChannelId(1)).unwrap();
    assert!(!straight.uses_guide_faces());
    assert!(SavedCalibration::with_profile(CALIBRATION_PROFILE.into(), straight.clone(), raw.clone(), None).is_ok());
    assert!(SavedCalibration::with_profile(CALIBRATION_PROFILE_GUIDE_FACES.into(), straight, raw, None).is_err());
}

/// Creates the current built-in Pattern with its checked-in saved numeric response.
///
/// # Panics
/// Panics if the built-in document or its numeric artifact no longer validates.
fn default_document() -> Document {
    Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap()
}

/// Builds the generic catalog spiral without using any persisted or source-backed artifact.
fn cover_canvas_spiral_recipe() -> PatternDefinitionRecipe {
    PatternDefinitionRecipe::connected(PatternStructureRecipe::ParametricCurve {
        name: "Reference spiral".into(),
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
        curve: ParametricCurve::Spiral(SpiralCurve {
            shape: SpiralShape::Round,
            turns: 5.0,
            radial_spacing: 16.0,
            phase_degrees: 37.0,
            winding: CurveWinding::Clockwise,
        }),
        spiral_coverage: SpiralCoveragePolicy::CoverCanvas,
        repetition: CurveRepetition::Single,
        sites: None,
    })
}

/// Accepts only the canonical 1024 concrete companion and keeps receiver matching exact.
///
/// # Panics
/// Panics if CoverCanvas intent is erased, forged geometry validates, or a mismatched canvas
/// receives the reference measurement as part of a failed atomic Pattern replacement.
#[test]
fn cover_canvas_preset_companion_matches_only_concrete_reference() {
    let recipe = cover_canvas_spiral_recipe();
    let reference = default_document();
    let (concrete, _) = reference
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: reference.pattern_settings().clone(),
            base_definition: reference.pattern_definition_bundles()[0].definition.clone(),
            recipe: recipe.clone(),
        })
        .unwrap();
    let binding = CalibrationBinding::for_channel(&concrete, ChannelId(1)).unwrap();
    let raw = vec![
        CalibrationKnot {
            input: 0.0,
            coverage: 0.0,
        },
        CalibrationKnot {
            input: 2.0,
            coverage: 1.0,
        },
    ];
    let artifact = SavedCalibration::new(binding.clone(), raw.clone(), None).unwrap();
    let record = PresetRecord {
        metadata: PresetMetadata {
            id: "reference-spiral".into(),
            name: "Reference spiral".into(),
            category: "Parametric".into(),
            description: "Canonical reference companion".into(),
            thumbnail: None,
        },
        recipe: recipe.clone(),
        saved_calibration: Some(artifact.clone()),
    };
    validate_preset_record(&record).unwrap();
    assert_eq!(record.recipe, recipe);
    let PatternStructureRecipe::OrderedOutputs { definition, .. } =
        binding.recipe().structure.clone()
    else {
        panic!("concrete reference has one ordered connected output");
    };
    let PatternStructureRecipe::ParametricCurve {
        curve: ParametricCurve::Spiral(spiral),
        spiral_coverage,
        ..
    } = *definition
    else {
        panic!("reference materializes a finite spiral");
    };
    assert_eq!(spiral_coverage, SpiralCoveragePolicy::Fixed);
    assert_eq!(spiral.turns, 47.0);

    let mut wrong = record.clone();
    let mut wrong_recipe = binding.recipe().clone();
    let PatternStructureRecipe::OrderedOutputs { definition, .. } = &mut wrong_recipe.structure
    else {
        panic!("concrete reference has one ordered connected output");
    };
    let PatternStructureRecipe::ParametricCurve {
        curve: ParametricCurve::Spiral(spiral),
        ..
    } = definition.as_mut()
    else {
        panic!("reference materializes a finite spiral");
    };
    spiral.turns -= 1.0;
    wrong.saved_calibration = Some(
        SavedCalibration::new(
            CalibrationBinding::new(wrong_recipe, 1.0, 0.0).unwrap(),
            raw,
            None,
        )
        .unwrap(),
    );
    assert_eq!(
        validate_preset_record(&wrong).unwrap_err().path(),
        "preset.calibration.binding"
    );

    let command = |document: &Document| DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: document.pattern_settings().clone(),
        base_definition: document.pattern_definition_bundles()[0].definition.clone(),
        recipe: record.recipe.clone(),
    };
    let mut matched = DocumentHistory::new(DocumentSession::new(default_document()).unwrap());
    let matched_command = command(matched.document());
    matched
        .apply_pattern_with_saved_calibration(&matched_command, Some(&artifact))
        .unwrap();
    assert_eq!(
        matched
            .document()
            .saved_calibration_for(ChannelId(1))
            .unwrap(),
        Some(&artifact)
    );

    let other = Document::new_default_document(
        CanvasSpec {
            width: 900.0,
            height: 620.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let mut unmatched = DocumentHistory::new(DocumentSession::new(other.clone()).unwrap());
    let unmatched_command = command(&other);
    assert!(
        unmatched
            .apply_pattern_with_saved_calibration(&unmatched_command, Some(&artifact))
            .is_err()
    );
    assert_eq!(unmatched.document(), &other);
}

/// Treats a typed one-output preset shorthand as the same concrete ordered output geometry.
///
/// # Panics
/// Panics if canonicalization loses an actual coverage-policy difference or the current built-in
/// document no longer reconstructs a single mark output.
#[test]
fn preset_shorthand_and_concrete_ordered_output_share_binding() {
    let concrete = CalibrationBinding::for_channel(&default_document(), ChannelId(1)).unwrap();
    let PatternStructureRecipe::OrderedOutputs {
        definition,
        outputs,
    } = concrete.recipe().structure.clone()
    else {
        panic!("default mark definition must reconstruct ordered outputs");
    };
    assert_eq!(outputs, vec![PatternOutputRealizationRecipe::Marks]);
    let shorthand = PatternDefinitionRecipe::marks(*definition.clone());
    assert_eq!(
        CalibrationBinding::new(shorthand.clone(), 1.0, 0.0).unwrap(),
        concrete
    );
    let mut changed = shorthand;
    let PatternStructureRecipe::StraightGrid(draft) = &mut changed.structure else {
        panic!("default mark shorthand must retain straight guide structure");
    };
    draft.coverage.guard_steps += 1;
    assert_ne!(
        CalibrationBinding::new(changed, 1.0, 0.0).unwrap(),
        concrete
    );
}

/// Ignores dormant family mark orientation for non-mark outputs but binds active mark direction.
///
/// # Panics
/// Panics if concrete non-mark materialization retains a phantom orientation or active marks
/// lose a coverage-relevant orientation distinction.
#[test]
fn binding_ignores_only_inactive_family_mark_orientation() {
    let structure = PatternStructureRecipe::GeneralizedStraightGuides {
        name: "Guide regions".into(),
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
        dimensions: vec![
            GuideDimensionDraft {
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
            GuideDimensionDraft {
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
        ],
        product: GeneralizedSiteProductDraft::Intersections {
            dimension_indices: vec![0, 1],
            merge_epsilon: 0.0,
        },
        orientation: MarkOrientationDraft::Fixed,
    };
    let mut tangent = structure.clone();
    let PatternStructureRecipe::GeneralizedStraightGuides { orientation, .. } = &mut tangent else {
        unreachable!("constructed generalized guide family")
    };
    *orientation = MarkOrientationDraft::GuideTangent { dimension_index: 0 };
    let regions_fixed = PatternDefinitionRecipe::regions(structure.clone());
    let regions_tangent = PatternDefinitionRecipe::regions(tangent.clone());
    assert_eq!(
        CalibrationBinding::new(regions_fixed, 1.0, 0.0).unwrap(),
        CalibrationBinding::new(regions_tangent, 1.0, 0.0).unwrap()
    );
    assert_ne!(
        CalibrationBinding::new(PatternDefinitionRecipe::marks(structure), 1.0, 0.0).unwrap(),
        CalibrationBinding::new(PatternDefinitionRecipe::marks(tangent), 1.0, 0.0).unwrap()
    );
}

/// Proves ordinary Feature and Advanced response edits retain the same structural artifact.
///
/// # Panics
/// Panics if a validated command fails or drops the checked-in artifact.
#[test]
fn feature_and_response_edits_reuse_saved_reference() {
    let document = default_document();
    let original = document
        .saved_calibration_for(ChannelId(1))
        .unwrap()
        .unwrap()
        .clone();
    let base = document.pattern_settings().clone();
    let mut settings = base.clone();
    settings.density.density = 400.0;
    let document = document
        .apply_command(&DocumentCommand::SetDocumentPatternSettings { base, settings })
        .unwrap()
        .0;
    assert_eq!(
        document.saved_calibration_for(ChannelId(1)).unwrap(),
        Some(&original)
    );
    let base = document.response_settings().clone();
    let mut settings = base.clone();
    settings.marks.maximum_fill = 0.75;
    let document = document
        .apply_command(&DocumentCommand::SetAllResponseSettings { base, settings })
        .unwrap()
        .0;
    assert_eq!(
        document.saved_calibration_for(ChannelId(1)).unwrap(),
        Some(&original)
    );
    assert_eq!(
        CalibrationBinding::for_channel(&document, ChannelId(1)).unwrap(),
        *original.binding()
    );
}

/// Rejects a stale structural measurement while retaining the previous bytes for Undo.
///
/// # Panics
/// Panics if the structural command fails or stale data is accepted as current.
#[test]
fn structural_edit_requires_recalibration() {
    let document = default_document();
    let old = document
        .saved_calibration_for(ChannelId(1))
        .unwrap()
        .unwrap()
        .clone();
    let base = document.pattern_settings().clone();
    let base_definition = document.pattern_definition_bundles()[0].definition.clone();
    let mut recipe = document
        .reconstruct_pattern_definition_recipe(base.definition_id)
        .unwrap();
    if let toniator_domain::PatternStructureRecipe::OrderedOutputs { definition, .. } =
        &mut recipe.structure
        && let toniator_domain::PatternStructureRecipe::StraightGrid(draft) = definition.as_mut()
    {
        draft.coverage.guard_steps = 3;
    }
    let changed = document
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe,
        })
        .unwrap()
        .0;
    assert!(
        changed
            .saved_calibration_for(ChannelId(1))
            .unwrap()
            .is_none()
    );
    assert!(changed.saved_calibrations().is_empty());
    assert_eq!(
        changed
            .require_saved_calibration_authority()
            .unwrap_err()
            .path(),
        "calibration.baseline.missing"
    );
    assert!(changed.with_saved_calibration(ChannelId(1), old).is_err());
}

/// Replaces all equal structural bindings and reports every affected channel in one history step.
///
/// # Panics
/// Panics if linked channels retain older curves or the history result omits them.
#[test]
fn replacement_updates_every_compatible_channel() {
    let document = default_document();
    let binding = CalibrationBinding::for_channel(&document, ChannelId(1)).unwrap();
    let replacement = SavedCalibration::new(
        binding,
        vec![
            CalibrationKnot {
                input: 0.0,
                coverage: 0.0,
            },
            CalibrationKnot {
                input: 2.0,
                coverage: 1.0,
            },
        ],
        None,
    )
    .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(document.clone()).unwrap());
    let result = history
        .attach_saved_calibration(
            &document,
            history.revision(),
            ChannelId(1),
            replacement.clone(),
        )
        .unwrap();
    assert_eq!(result.affected_channels.len(), 3);
    for channel_id in [ChannelId(1), ChannelId(2), ChannelId(3)] {
        assert_eq!(
            history
                .document()
                .saved_calibration_for(channel_id)
                .unwrap(),
            Some(&replacement)
        );
    }
    history.undo().unwrap();
    assert_ne!(
        history
            .document()
            .saved_calibration_for(ChannelId(2))
            .unwrap(),
        Some(&replacement)
    );
}

/// Rejects unmeasurable whole-artifact spans while allowing local plateaus.
///
/// # Panics
/// Panics if domain acceptance disagrees with runtime curve requirements.
#[test]
fn whole_flat_payloads_are_rejected() {
    let document = default_document();
    let binding = CalibrationBinding::for_channel(&document, ChannelId(1)).unwrap();
    assert!(
        SavedCalibration::new(
            binding,
            vec![
                CalibrationKnot {
                    input: 0.0,
                    coverage: 0.5
                },
                CalibrationKnot {
                    input: 2.0,
                    coverage: 0.5
                },
            ],
            None
        )
        .is_err()
    );
    assert!(
        UniformCalibration::new(
            0.4,
            vec![
                CalibrationKnot {
                    input: 0.0,
                    coverage: 0.999
                },
                CalibrationKnot {
                    input: 1.0,
                    coverage: 0.999
                },
            ]
        )
        .is_err()
    );
}

/// Rejects forged stochastic metadata on a deterministic structural binding.
///
/// # Panics
/// Panics if current validation allows a mean profile without typed stochastic geometry.
#[test]
fn deterministic_binding_cannot_claim_stochastic_mean() {
    let binding = CalibrationBinding::for_channel(&default_document(), ChannelId(1)).unwrap();
    let variation = CalibrationVariation::new(0.0, 0.0, 0.0, 0.0, None, None).unwrap();
    assert!(
        SavedCalibration::with_mean_profile(
            binding,
            vec![
                CalibrationKnot {
                    input: 0.0,
                    coverage: 0.0
                },
                CalibrationKnot {
                    input: 2.0,
                    coverage: 1.0
                },
            ],
            None,
            variation
        )
        .is_err()
    );
}

/// Carries one compatible structural measurement through a real RGB-to-CMYK topology change.
///
/// # Panics
/// Panics if new modeled channel IDs lose reusable saved measurements or stale IDs remain.
#[test]
fn model_switch_rebinds_compatible_calibration() {
    let document = default_document();
    let artifact = document
        .saved_calibration_for(ChannelId(1))
        .unwrap()
        .unwrap()
        .clone();
    let template = ChannelTopologyTemplate {
        pattern_instance: document.channel_topology().unwrap().channels()[0]
            .pattern_instance
            .clone(),
    };
    let topology = document
        .canonical_channel_topology_with_appearance_baseline(HalftoneChannelModel::Cmyk, template)
        .unwrap();
    let changed = document
        .apply_command(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Cmyk,
            topology,
        })
        .unwrap()
        .0;
    for channel in changed.channel_topology().unwrap().channels() {
        assert_eq!(
            changed.saved_calibration_for(channel.id).unwrap(),
            Some(&artifact)
        );
    }
    assert_eq!(changed.saved_calibrations().len(), 4);
    changed.require_saved_calibration_authority().unwrap();
}

/// Retains the last ALL artifact while every RGB channel uses a different structural aspect.
///
/// A model change restores the ALL aspect onto fresh CMYK channels using the hidden companion;
/// changing that hidden aspect clears only its stale artifact until a guarded fit attaches one.
///
/// # Panics
/// Panics if named assignment, baseline retention, model rebinding, or Undo/Redo loses authority.
#[test]
fn hidden_all_calibration_survives_distinct_channels_and_model_rebind() {
    let mut history = DocumentHistory::new(DocumentSession::new(default_document()).unwrap());
    let original = history
        .document()
        .baseline_saved_calibration()
        .unwrap()
        .unwrap()
        .clone();
    let channels: Vec<_> = history
        .document()
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect();
    for (channel, aspect) in channels.iter().copied().zip([1.1, 1.3, 1.6]) {
        let before = history.document().clone();
        let config = before
            .edit_channel_start_configuration(&[(
                PropertyTarget::Channel(channel),
                PropertyFieldId::DensityAspect,
                aspect,
            )])
            .unwrap();
        history
            .apply_document_configuration(&before, history.revision(), &config)
            .unwrap();
        let binding = CalibrationBinding::for_channel(history.document(), channel).unwrap();
        let artifact = SavedCalibration::new(
            binding,
            vec![
                CalibrationKnot {
                    input: 0.0,
                    coverage: 0.0,
                },
                CalibrationKnot {
                    input: 2.0,
                    coverage: 1.0,
                },
            ],
            None,
        )
        .unwrap();
        let base = history.document().clone();
        history
            .attach_saved_calibration(&base, history.revision(), channel, artifact)
            .unwrap();
    }
    assert_eq!(
        history.document().baseline_saved_calibration().unwrap(),
        Some(&original)
    );
    assert!(channels.iter().all(|channel| {
        history
            .document()
            .saved_calibration_for(*channel)
            .unwrap()
            .unwrap()
            .binding()
            != original.binding()
    }));
    let base = history.document().pattern_settings().clone();
    let mut settings = base.clone();
    settings.density.aspect = 1.4;
    history
        .apply(&DocumentCommand::SetDocumentPatternSettings { base, settings })
        .unwrap();
    assert!(
        history
            .document()
            .baseline_saved_calibration()
            .unwrap()
            .is_none()
    );
    assert!(channels.iter().all(|channel| {
        history
            .document()
            .saved_calibration_for(*channel)
            .unwrap()
            .is_none()
    }));
    for channel in channels.iter().copied() {
        let binding = CalibrationBinding::for_channel(history.document(), channel).unwrap();
        let artifact = SavedCalibration::new(
            binding,
            vec![
                CalibrationKnot {
                    input: 0.0,
                    coverage: 0.0,
                },
                CalibrationKnot {
                    input: 2.0,
                    coverage: 1.0,
                },
            ],
            None,
        )
        .unwrap();
        let base = history.document().clone();
        history
            .attach_saved_calibration(&base, history.revision(), channel, artifact)
            .unwrap();
    }
    assert!(
        history
            .document()
            .baseline_saved_calibration()
            .unwrap()
            .is_none()
    );
    let binding = history.document().baseline_calibration_binding().unwrap();
    let replacement = SavedCalibration::new(
        binding,
        vec![
            CalibrationKnot {
                input: 0.0,
                coverage: 0.0,
            },
            CalibrationKnot {
                input: 2.0,
                coverage: 1.0,
            },
        ],
        None,
    )
    .unwrap();
    let base = history.document().clone();
    history
        .attach_saved_baseline_calibration(&base, history.revision(), replacement.clone())
        .unwrap();
    let refreshed = history.document().clone();
    history.undo().unwrap().unwrap();
    assert!(
        history
            .document()
            .baseline_saved_calibration()
            .unwrap()
            .is_none()
    );
    history.redo().unwrap().unwrap();
    assert_eq!(history.document(), &refreshed);

    let topology = history
        .document()
        .canonical_channel_topology_with_appearance_baseline(
            HalftoneChannelModel::Cmyk,
            ChannelTopologyTemplate::document_base(),
        )
        .unwrap();
    history
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Cmyk,
            topology,
        })
        .unwrap();
    assert_eq!(
        history.document().baseline_saved_calibration().unwrap(),
        Some(&replacement)
    );
    for channel in history.document().channel_topology().unwrap().channels() {
        assert_eq!(
            history
                .document()
                .saved_calibration_for(channel.id)
                .unwrap(),
            Some(&replacement)
        );
    }
}

/// Verifies one artifact-only publication invalidates realization and rejects a stale root.
///
/// # Panics
/// Panics if current history, artifact validation, or revision handling regresses.
#[test]
fn attachment_is_one_history_step_with_realization_invalidation() {
    let document = default_document();
    let artifact = SavedCalibration::built_in_straight(&document, ChannelId(1)).unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(document.clone()).unwrap());
    let revision = history.revision();
    let result = history
        .attach_saved_calibration(&document, revision, ChannelId(1), artifact.clone())
        .unwrap();
    assert_eq!(result.invalidation, Some(InvalidationLevel::Realization));
    assert!(
        history
            .attach_saved_calibration(&document, revision, ChannelId(1), artifact)
            .is_err()
    );
}

/// Rejects duplicate channel artifacts and incompatible persisted geometry profiles.
///
/// # Panics
/// Panics if complete restoration silently overwrites one entry or accepts an obsolete profile.
#[test]
fn restore_rejects_duplicate_channels_and_obsolete_profile() {
    let document = default_document();
    let artifact = document
        .saved_calibration_for(ChannelId(1))
        .unwrap()
        .unwrap()
        .clone();
    let entry = AttachedCalibration {
        channel_id: ChannelId(1),
        artifact: artifact.clone(),
    };
    assert!(
        document
            .with_saved_calibrations(vec![entry.clone(), entry])
            .is_err()
    );
    assert!(
        SavedCalibration::with_profile(
            "unknown-reference-contract".to_owned(),
            artifact.binding().clone(),
            artifact.raw().to_vec(),
            None,
        )
        .is_err()
    );
    assert!(
        SavedCalibration::with_profile(
            "ton018-reference-fixed-canvas1024-v2".to_owned(),
            artifact.binding().clone(),
            artifact.raw().to_vec(),
            None,
        )
        .is_err()
    );
    assert!(
        SavedCalibration::with_profile(
            CALIBRATION_PROFILE_64.to_owned(),
            artifact.binding().clone(),
            artifact.raw().to_vec(),
            None,
        )
        .is_ok()
    );
}

/// Reuses measurements across compatible modeled channels despite different channel IDs.
///
/// # Panics
/// Panics if a channel ID becomes part of structural binding or valid data disappears.
#[test]
fn compatible_channel_ids_share_artifact() {
    let document = default_document();
    let first = document
        .saved_calibration_for(ChannelId(1))
        .unwrap()
        .unwrap()
        .clone();
    let only_first = document
        .with_saved_calibrations(vec![AttachedCalibration {
            channel_id: ChannelId(1),
            artifact: first.clone(),
        }])
        .unwrap();
    assert_eq!(
        only_first.saved_calibration_for(ChannelId(2)).unwrap(),
        Some(&first)
    );
}
