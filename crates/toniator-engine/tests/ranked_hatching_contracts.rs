use std::{fs, path::PathBuf};
use toniator_domain::*;
use toniator_engine::{
    EvaluationLimits,
    reference_calibration::{calibrate_saved_pattern, reference_document},
};
use toniator_patterns::{PresetRegistry, interleaved_hatch_recipe};

/// Publishes an ordinary recipe on the current reference document without fixture-only geometry.
/// # Panics
/// Panics if current recipe materialization rejects a valid fixed-width hatch configuration.
fn hatch_document(rings: bool) -> Document {
    reference_document(&interleaved_hatch_recipe(rings), 32)
        .unwrap()
        .0
}

/// Proves discrete calibration measures real attainable levels and survives current Pattern storage.
/// # Panics
/// Panics if calibration smooths a staircase, invents endpoints, loses width, or accepts a wrong profile.
#[test]
fn hatch_measured_populations_profile_and_preset_roundtrip() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ranked-hatch-20261118/focused-contracts");
    fs::create_dir_all(&directory).unwrap();
    for rings in [false, true] {
        let document = hatch_document(rings);
        let measured = calibrate_saved_pattern(
            &document,
            ChannelId(1),
            EvaluationLimits::new(2_000_000).unwrap(),
            &|| false,
        )
        .unwrap();
        assert!(measured.profile().contains(CALIBRATION_PROFILE_HATCH));
        assert_eq!(measured.raw().len(), 9);
        assert_eq!(measured.raw()[0].coverage, 0.0);
        assert!(
            measured.raw()[8].coverage > 0.98,
            "default full population should reach nearly full union: {:?}",
            measured.raw()
        );
        assert!(
            measured
                .raw()
                .windows(2)
                .all(|pair| pair[0].coverage <= pair[1].coverage)
        );
        assert!(measured.uniform().is_none());
        assert!(
            SavedCalibration::with_profile(
                measured
                    .binding()
                    .profile_for_reference(CALIBRATION_PROFILE),
                measured.binding().clone(),
                measured.raw().to_vec(),
                None
            )
            .is_err()
        );
        let mut record = PresetRegistry::bundled()
            .find(if rings {
                "concentric-hatching"
            } else {
                "parallel-hatching"
            })
            .unwrap()
            .clone();
        record.saved_calibration = Some(measured);
        let path = directory.join(if rings {
            "rings.toniator-pattern"
        } else {
            "parallel.toniator-pattern"
        });
        toniator_io::save_preset(&path, &record).unwrap();
        assert_eq!(toniator_io::load_preset(&path).unwrap(), record);
    }
}

/// Keeps width editing descriptor-owned, finite, reversible and realization-invalidating.
/// # Panics
/// Panics if local history loses fixed-width intent or invalid input reaches the document.
#[test]
fn hatch_width_descriptor_history_and_unsupported_policy_rejection() {
    let document = hatch_document(false);
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    let before = history.document().clone();
    let definition = before.pattern_definition_for(ChannelId(1)).unwrap().clone();
    let output = definition.output_layers[0].id;
    assert!(
        before
            .property_descriptors()
            .iter()
            .any(|d| d.field == PropertyFieldId::HatchStrokeWidth)
    );
    let command = DocumentCommand::EditSharedPatternDefinition {
        definition_id: definition.id,
        base_definition: definition.clone(),
        edit: PatternDefinitionEdit::SetHatchStrokeWidth {
            output_layer_id: output,
            value: 0.6,
        },
    };
    let result = history.apply(&command).unwrap();
    assert_eq!(result.invalidation, Some(InvalidationLevel::Realization));
    assert_eq!(
        history
            .document()
            .pattern_definition_for(ChannelId(1))
            .unwrap()
            .output_layers[0]
            .realization
            .hatch_style()
            .unwrap()
            .width,
        0.6
    );
    history.undo().unwrap();
    assert_eq!(history.document(), &before);
    history.redo().unwrap();
    for value in [0.0, f64::NAN, f64::INFINITY, 2.01] {
        let definition = history
            .document()
            .pattern_definition_for(ChannelId(1))
            .unwrap()
            .clone();
        assert!(
            history
                .apply(&DocumentCommand::EditSharedPatternDefinition {
                    definition_id: definition.id,
                    base_definition: definition,
                    edit: PatternDefinitionEdit::SetHatchStrokeWidth {
                        output_layer_id: output,
                        value
                    }
                })
                .is_err()
        );
    }
    let mut bad = PresetRegistry::bundled()
        .find("clustered-dispersion-random-links")
        .unwrap()
        .recipe
        .clone();
    match &mut bad.structure {
        PatternStructureRecipe::ConnectionPaths { style, .. } => {
            style.hatch = Some(HatchStyle { width: 1.0 })
        }
        PatternStructureRecipe::OrderedOutputs { outputs, .. } => {
            let PatternOutputRealizationRecipe::ConnectionPaths { style, .. } = &mut outputs[0]
            else {
                panic!("connection output")
            };
            style.hatch = Some(HatchStyle { width: 1.0 });
        }
        _ => panic!("connection recipe"),
    }
    assert!(
        reference_document(&bad, 32).is_err(),
        "unsupported hatch policy must never be silently ignored"
    );
    let mut mixed = interleaved_hatch_recipe(false);
    let PatternStructureRecipe::OrderedOutputs { outputs, .. } = &mut mixed.structure else {
        panic!("ordinary ordered hatch")
    };
    outputs.push(PatternOutputRealizationRecipe::StructuralPaths {
        tone_sample_spacing: Default::default(),
        style: Default::default(),
    });
    mixed.output_settings.push(mixed.output_settings[0].clone());
    assert!(
        reference_document(&mixed, 32).is_err(),
        "current single drawing output gate rejects mixed continuous and hatch calibration"
    );
}

/// Keeps support-changing toggles family-invalidating and finite carriers truthfully inapplicable.
/// # Panics
/// Panics if a cached analytic ring support can survive an incompatible mode or source change.
#[test]
fn hatch_family_toggle_and_carrier_applicability() {
    let mut document = hatch_document(true);
    for enabled in [false, true] {
        let definition = document
            .pattern_definition_for(ChannelId(1))
            .unwrap()
            .clone();
        let output = definition.output_layers[0].id;
        let (next, result) = document
            .apply_command(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: definition.id,
                base_definition: definition,
                edit: PatternDefinitionEdit::SetHatchEnabled {
                    output_layer_id: output,
                    enabled,
                },
            })
            .unwrap();
        assert_eq!(result.invalidation, Some(InvalidationLevel::Family));
        document = next;
        assert_eq!(document.property_descriptors().iter().any(|descriptor| descriptor.field == PropertyFieldId::AlongGuideIntervalMultiplier), !enabled);
        let definition = document.pattern_definition_for(ChannelId(1)).unwrap();
        let plan = toniator_patterns::resolve_document_pattern_pipeline_for_channel(
            &document,
            definition,
            SourceWeighting::canonical(SourceMappingComponent::Cyan),
        )
        .unwrap();
        assert_eq!(
            plan.family.generic_guides.unwrap().analytic_hatch_circles,
            enabled
        );
    }
    let mut single = interleaved_hatch_recipe(true);
    let PatternStructureRecipe::OrderedOutputs { definition, .. } = &mut single.structure else {
        panic!("ordered hatching")
    };
    let PatternStructureRecipe::GenericGuides { dimensions, .. } = definition.as_mut() else {
        panic!("generic ring foundation")
    };
    dimensions[0].repetition = GuideRepetition::Single;
    assert!(reference_document(&single, 32).is_err());
    let original = PresetRegistry::bundled()
        .find("round-spiral-line")
        .unwrap()
        .recipe
        .clone();
    let canonical = reference_document(&original, 32).unwrap().0;
    let mut spiral = canonical
        .reconstruct_pattern_definition_recipe(canonical.pattern_settings().definition_id)
        .unwrap();
    let PatternStructureRecipe::OrderedOutputs { outputs, .. } = &mut spiral.structure else {
        panic!("ordered spiral")
    };
    let PatternOutputRealizationRecipe::StructuralPaths { style, .. } = &mut outputs[0] else {
        panic!("structural spiral")
    };
    style.hatch = Some(HatchStyle { width: 1.0 });
    assert!(reference_document(&spiral, 32).is_err());
}
