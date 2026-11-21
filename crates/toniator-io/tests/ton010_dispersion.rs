use std::{fs, path::PathBuf};
use toniator_domain::{
    CalibrationBinding, CalibrationKnot, DispersionCalibration, DispersionLayout,
    PatternStructureRecipe, PresetRecord, RandomSiteCharacter, SavedCalibration,
};
use toniator_io::{load_preset, save_preset};
use toniator_patterns::PresetRegistry;

/// Mutates only the structural method of a real bundled Wizard recipe, retaining output wrappers.
/// # Panics
/// Panics if the selected bundled fixture ceases to be a Scatter recipe.
fn set_layout(structure: &mut PatternStructureRecipe, layout: DispersionLayout) {
    match structure {
        PatternStructureRecipe::RandomSites {
            character,
            nominal_spacing,
            ..
        } => {
            *character = RandomSiteCharacter::Dispersion { layout };
            *nominal_spacing = Some(8.0);
        }
        PatternStructureRecipe::AuthoredResources { definition, .. }
        | PatternStructureRecipe::AuthoredClosedShapeMarks { definition, .. }
        | PatternStructureRecipe::OrderedOutputs { definition, .. } => {
            set_layout(definition, layout)
        }
        _ => panic!("expected bundled Scatter recipe"),
    }
}

/// Creates synthetic numeric serialization data; this fixture does not claim measured product calibration.
/// # Panics
/// Panics if the bundled record or explicit synthetic numeric fixture is invalid.
fn fixture(layout: DispersionLayout, complete: bool) -> PresetRecord {
    let mut record = PresetRegistry::bundled()
        .entries()
        .iter()
        .find(|record| record.metadata.id == "even-random-circles")
        .unwrap()
        .clone();
    set_layout(&mut record.recipe.structure, layout);
    let binding = CalibrationBinding::new(record.recipe.clone(), 1.0, 0.0).unwrap();
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
    let artifact = SavedCalibration::new(binding, raw, None).unwrap();
    assert!(
        artifact.requires_dispersion(),
        "real OrderedOutputs wrapper must retain intrinsic requirement"
    );
    record.saved_calibration = Some(if complete {
        artifact
            .with_dispersion(
                DispersionCalibration::new(vec![
                    CalibrationKnot {
                        input: 0.0,
                        coverage: 0.0,
                    },
                    CalibrationKnot {
                        input: 0.5,
                        coverage: 0.7,
                    },
                    CalibrationKnot {
                        input: 1.0,
                        coverage: 1.0,
                    },
                ])
                .unwrap(),
            )
            .unwrap()
    } else {
        artifact
    });
    record
}

/// Saves/reloads every layout and rejects absent, malformed and stale new-layout numeric authority.
/// # Panics
/// Panics if wrapper authority, exact roundtrip, required transfer or profile validation changes.
#[test]
fn dispersion_current_pattern_roundtrip_requires_measured_transfer() {
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton010-dispersion-20261118/focused-io");
    fs::create_dir_all(&output).unwrap();
    for layout in [
        DispersionLayout::Bayer,
        DispersionLayout::FloydSteinberg,
        DispersionLayout::InkQuadtree,
        DispersionLayout::TangentSpacing,
        DispersionLayout::TangentSize,
        DispersionLayout::TangentBlend,
        DispersionLayout::ClusterScreen,
    ] {
        let path = output.join(format!("{layout:?}.toniator-pattern"));
        assert!(save_preset(&path, &fixture(layout, false)).is_err());
        let record = fixture(layout, true);
        save_preset(&path, &record).unwrap();
        assert_eq!(load_preset(&path).unwrap(), record);
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for mutation in [0, 1, 2] {
            let mut bad = value.clone();
            let artifact = &mut bad["saved_calibration"];
            match mutation {
                0 => {
                    artifact.as_object_mut().unwrap().remove("dispersion");
                }
                1 => artifact["dispersion"]["profile"] = serde_json::json!("obsolete-v0"),
                _ => artifact["dispersion"]["observations"][0]["coverage"] = serde_json::json!(0.1),
            };
            fs::write(&path, serde_json::to_vec_pretty(&bad).unwrap()).unwrap();
            assert!(
                load_preset(&path).is_err(),
                "{layout:?} mutation={mutation}"
            );
        }
        save_preset(&path, &record).unwrap();
    }
}

/// Preserves absent-transfer serialization and all existing numeric authority for ordinary Patterns.
/// # Panics
/// Panics if the additive field changes old record identity or appears when absent.
#[test]
fn dispersion_absence_preserves_existing_pattern_payload() {
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton010-dispersion-20261118/focused-io");
    fs::create_dir_all(&output).unwrap();
    let record = PresetRegistry::bundled()
        .entries()
        .iter()
        .find(|record| record.metadata.id == "even-random-circles")
        .unwrap()
        .clone();
    let path = output.join("unchanged-scatter.toniator-pattern");
    save_preset(&path, &record).unwrap();
    let bytes = fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("\"dispersion\""));
    assert_eq!(load_preset(&path).unwrap(), record);
}

/// Retains real partial-span and nonmonotone observations without inventing white/full-ink endpoints.
/// # Panics
/// Panics if a valid partial shape span rejects or stored observations are silently normalized.
#[test]
fn dispersion_transfer_preserves_partial_span_and_raw_observations() {
    let observations = vec![
        CalibrationKnot {
            input: 0.0,
            coverage: 0.0,
        },
        CalibrationKnot {
            input: 0.25,
            coverage: 0.2,
        },
        CalibrationKnot {
            input: 0.5,
            coverage: 0.19,
        },
        CalibrationKnot {
            input: 1.0,
            coverage: 0.6,
        },
    ];
    let transfer = DispersionCalibration::new(observations.clone()).unwrap();
    assert_eq!(transfer.observations(), &observations);
    assert_eq!(transfer.inverse(0.0), 0.0);
    assert_eq!(transfer.inverse(1.0), 1.0);
    assert!(transfer.inverse(0.01) > 0.0);
    assert!(transfer.inverse(0.4) > 0.5);
}

/// Interpolates inside the actual rising segment instead of crossing skipped measured valleys.
/// # Panics
/// Panics if the selected crossing interpolates from the older cumulative peak.
#[test]
fn dispersion_inverse_uses_adjacent_observation_after_a_valley() {
    let transfer = DispersionCalibration::new(vec![
        CalibrationKnot {
            input: 0.0,
            coverage: 0.0,
        },
        CalibrationKnot {
            input: 0.25,
            coverage: 0.854,
        },
        CalibrationKnot {
            input: 0.275,
            coverage: 0.718,
        },
        CalibrationKnot {
            input: 0.4,
            coverage: 0.895,
        },
        CalibrationKnot {
            input: 1.0,
            coverage: 1.0,
        },
    ])
    .unwrap();
    let expected = 0.275 + (0.4 - 0.275) * (0.878 - 0.718) / (0.895 - 0.718);
    assert!((transfer.inverse(0.878) - expected).abs() < 1e-14);
}
