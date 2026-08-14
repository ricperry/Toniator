use std::{fs, path::PathBuf};

use toniator_domain::{
    CoveragePolicy, GuideDimensionDraft, MarkOrientationDraft, PatternDefinitionDraft,
    PatternDefinitionRecipe, PresetMetadata, PresetRecord,
};
use toniator_io::{PRESET_FORMAT_VERSION, load_preset, save_preset};

/// Returns an isolated derived-artifact directory while leaving immutable
/// project inputs untouched.
fn validation_directory() -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/stage-19a/preset-io");
    fs::create_dir_all(&directory).unwrap();
    directory
}

/// Serializes and reloads an ordinary pure-schema record through the standalone
/// versioned IO boundary without changing document/container schema.
#[test]
fn preset_round_trips_through_versioned_standalone_io() {
    let directory = validation_directory();
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "io-round-trip".into(),
            name: "IO Round Trip".into(),
            category: "Test".into(),
            description: "Standalone preset serialization fixture.".into(),
            thumbnail: Some("builtin:io-round-trip".into()),
        },
        recipe: PatternDefinitionRecipe::StraightGrid(PatternDefinitionDraft {
            name: "Round-trip grid".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        }),
    };
    let path = directory.join("io-round-trip.preset.json");
    save_preset(&path, &preset).unwrap();
    assert_eq!(load_preset(&path).unwrap(), preset);
    let text = fs::read_to_string(path).unwrap();
    assert!(text.contains(&format!(
        "\"preset_format_version\": {PRESET_FORMAT_VERSION}"
    )));
}

/// Rejects malformed, unknown-version, invalid-metadata, and invalid-recipe
/// inputs without writing a new standalone preset file.
#[test]
fn preset_io_rejects_invalid_serialized_and_save_inputs() {
    let directory = validation_directory();
    let malformed = directory.join("malformed.preset.json");
    fs::write(&malformed, b"{").unwrap();
    assert!(load_preset(&malformed).is_err());
    let unknown = directory.join("unknown-version.preset.json");
    fs::write(
        &unknown,
        r#"{"preset_format_version":99,"metadata":{"id":"x","name":"X","category":"Test","description":"Test","thumbnail":null},"recipe":{"kind":"straight_grid","name":"Grid","coverage":{"guard_steps":2,"additional_margin":4.5}}}"#,
    )
    .unwrap();
    assert!(load_preset(&unknown).is_err());
    let invalid_path = directory.join("invalid-save.preset.json");
    let invalid_metadata = PresetRecord {
        metadata: PresetMetadata {
            id: " ".into(),
            name: "Invalid".into(),
            category: "Test".into(),
            description: "Validation fixture.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::StraightGrid(PatternDefinitionDraft {
            name: "Grid".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        }),
    };
    assert!(save_preset(&invalid_path, &invalid_metadata).is_err());
    assert!(!invalid_path.exists());
    let invalid_recipe = PresetRecord {
        metadata: PresetMetadata {
            id: "invalid-recipe".into(),
            name: "Invalid Recipe".into(),
            category: "Test".into(),
            description: "Validation fixture.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::GeneralizedStraightGuides {
            name: "Bad index".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
            dimensions: vec![GuideDimensionDraft {
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            }],
            product: toniator_domain::GeneralizedSiteProductDraft::AlongGuides {
                dimension_indices: vec![0],
                interval_multiplier: 1.0,
                phase: 0.0,
            },
            orientation: MarkOrientationDraft::GuideNormal { dimension_index: 1 },
        },
    };
    assert!(save_preset(&invalid_path, &invalid_recipe).is_err());
    assert!(!invalid_path.exists());
}
