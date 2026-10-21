//! Persistent extent intent remains distinct from derived canvas coverage and strict fits.
use toniator_domain::*;

/// Creates identical finite authored spiral inputs with an explicit extent policy.
fn recipe(policy: SpiralCoveragePolicy) -> PatternDefinitionRecipe {
    PatternDefinitionRecipe::connected(PatternStructureRecipe::ParametricCurve {
        name: "Coverage authority fixture".into(),
        coverage: CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
        curve: ParametricCurve::Spiral(SpiralCurve {
            shape: SpiralShape::Round,
            turns: 5.0,
            radial_spacing: 16.0,
            phase_degrees: 17.0,
            winding: CurveWinding::Clockwise,
        }),
        spiral_coverage: policy,
        repetition: CurveRepetition::Single,
        sites: None,
    })
}

/// Applies the ordinary recipe command without generating geometry or preparing a numeric fit.
/// # Panics
/// Panics if the bounded valid fixture cannot be installed through current domain authority.
fn document(policy: SpiralCoveragePolicy, canvas: CanvasSpec) -> Document {
    let base = Document::new_default_document(canvas, SourceReference::Unassigned).unwrap();
    base.apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: base.pattern_settings().clone(),
        base_definition: base.pattern_definition_bundles()[0].definition.clone(),
        recipe: recipe(policy),
    })
    .unwrap()
    .0
}

/// Reads the authored source without interpreting names, recipe provenance or current placement.
/// # Panics
/// Panics if the selected finite fixture loses its parametric source mechanism.
fn source(document: &Document) -> (SpiralCoveragePolicy, SpiralCurve) {
    let definition = &document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.id == document.pattern_settings().definition_id)
        .unwrap()
        .definition;
    match &definition.mechanisms[0] {
        PatternMechanism::ParametricCurveSource {
            curve: ParametricCurve::Spiral(curve),
            spiral_coverage,
            ..
        } => (*spiral_coverage, curve.clone()),
        _ => panic!("parametric source required"),
    }
}

/// Preserves policy and authored turns across canvas-dependent application and recipe reconstruction.
/// # Panics
/// Panics if materialization loses intent or replaces the authored minimum with a derived extent.
#[test]
fn cover_canvas_materialization_retains_authored_minimum_and_policy() {
    for policy in [
        SpiralCoveragePolicy::Fixed,
        SpiralCoveragePolicy::CoverCanvas,
    ] {
        for canvas in [
            CanvasSpec {
                width: 1024.0,
                height: 1024.0,
            },
            CanvasSpec {
                width: 1792.0,
                height: 320.0,
            },
        ] {
            let document = document(policy, canvas);
            let (actual, curve) = source(&document);
            assert_eq!(actual, policy);
            assert_eq!(curve.turns, 5.0);
            assert_eq!(curve.radial_spacing, 16.0);
            let expected = CalibrationBinding::new(recipe(policy), 1.0, 0.0).unwrap();
            assert_eq!(
                CalibrationBinding::for_channel(&document, ChannelId(1)).unwrap(),
                expected
            );
        }
    }
}

/// Keeps explicit coverage structural while whole-pattern translation remains outside measured identity.
/// # Panics
/// Panics if Fixed measurements can stand in for CoverCanvas or unsupported profiles gain validity.
#[test]
fn coverage_policy_is_strict_binding_and_producer_authority() {
    let fixed = CalibrationBinding::new(recipe(SpiralCoveragePolicy::Fixed), 1.0, 0.0).unwrap();
    let cover =
        CalibrationBinding::new(recipe(SpiralCoveragePolicy::CoverCanvas), 1.0, 0.0).unwrap();
    assert!(!fixed.is_compatible_with(&cover));
    assert!(!cover.is_compatible_with(&fixed));
    assert!(!fixed.requires_canvas_coverage());
    assert!(cover.requires_canvas_coverage());
    let previous = fixed.profile_for_reference(CALIBRATION_PROFILE);
    assert_eq!(previous, format!("{CALIBRATION_PROFILE}+anchored-tone-v1"));
    assert_eq!(
        cover.profile_for_reference(CALIBRATION_PROFILE),
        format!("{previous}+cover-envelope-v1")
    );
    assert!(cover.is_obsolete_geometry_profile(&previous));
    assert!(!fixed.is_obsolete_geometry_profile(&previous));
    assert!(!cover.is_obsolete_geometry_profile(&format!("{previous}+unknown-future")));
    let mut original = document(
        SpiralCoveragePolicy::CoverCanvas,
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
    );
    // These metadata-only knots exercise immutable artifact attachment, not measurement accuracy.
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
    for channel_id in [ChannelId(1), ChannelId(2), ChannelId(3)] {
        let binding = CalibrationBinding::for_channel(&original, channel_id).unwrap();
        original = original
            .with_saved_calibration(
                channel_id,
                SavedCalibration::new(binding, raw.clone(), None).unwrap(),
            )
            .unwrap();
    }
    let baseline =
        SavedCalibration::new(original.baseline_calibration_binding().unwrap(), raw, None).unwrap();
    original = original.with_saved_baseline_calibration(baseline).unwrap();
    original.require_saved_calibration_authority().unwrap();
    let moved = original
        .apply_command(&DocumentCommand::SetTranslationAxis {
            channel_id: ChannelId(1),
            edited_axis: TranslationEditedAxis::X,
            value: -4096.0,
        })
        .unwrap()
        .0;
    let moved = moved
        .apply_command(&DocumentCommand::SetTranslationAxis {
            channel_id: ChannelId(1),
            edited_axis: TranslationEditedAxis::Y,
            value: 3072.0,
        })
        .unwrap()
        .0;
    let moved = moved
        .apply_command(&DocumentCommand::SetChannelPatternRotationDelta {
            base: moved.pattern_settings().clone(),
            channel_id: ChannelId(1),
            rotation_degrees: 37.0,
        })
        .unwrap()
        .0;
    assert_eq!(
        CalibrationBinding::for_channel(&moved, ChannelId(1)).unwrap(),
        cover
    );
    assert_eq!(source(&moved), source(&original));
    moved.require_saved_calibration_authority().unwrap();
    assert_eq!(moved.saved_calibrations(), original.saved_calibrations());
    assert_eq!(
        moved.baseline_saved_calibration().unwrap(),
        original.baseline_saved_calibration().unwrap()
    );
    assert_eq!(moved.response_settings(), original.response_settings());
}

/// Retains coverage through named copy-on-edit, stable-ID remapping and exact history replay.
/// # Panics
/// Panics if editing authored extent drops coverage or mutates an unselected sibling.
#[test]
fn named_extent_edit_and_history_preserve_coverage_intent() {
    let original = document(
        SpiralCoveragePolicy::CoverCanvas,
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
    );
    let definition = original
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.id == original.pattern_settings().definition_id)
        .unwrap()
        .definition
        .clone();
    let mechanism_id = definition.mechanisms[0].id();
    let mut history = DocumentHistory::new(DocumentSession::new(original.clone()).unwrap());
    history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: ChannelId(1),
            base_definition: definition,
            edit: PatternDefinitionEdit::SetParametricTurns {
                mechanism_id,
                turns: 9.0,
            },
        })
        .unwrap();
    let selected = history
        .document()
        .effective_channel_pattern(ChannelId(1))
        .unwrap()
        .definition_id;
    let binding = CalibrationBinding::for_channel(history.document(), ChannelId(1)).unwrap();
    assert!(binding.requires_canvas_coverage());
    assert!(
        CalibrationBinding::for_channel(history.document(), ChannelId(2))
            .unwrap()
            .requires_canvas_coverage()
    );
    assert_ne!(
        selected,
        history
            .document()
            .effective_channel_pattern(ChannelId(2))
            .unwrap()
            .definition_id
    );
    let after = history.document().clone();
    history.undo().unwrap();
    assert_eq!(history.document(), &original);
    history.redo().unwrap();
    assert_eq!(history.document(), &after);
}
