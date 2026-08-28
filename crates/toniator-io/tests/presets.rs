use std::{fs, path::PathBuf};

use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft, AuthoredStructureKind,
    CoveragePolicy, CurveRepetition, CurveWinding, GuideDimensionDraft, MarkOrientationDraft,
    ParametricCurve, ParametricCurveSiteDraft, PatternDefinitionDraft, PatternDefinitionRecipe,
    PatternStructureRecipe, PresetMetadata, PresetRecord, SpiralCurve, SpiralShape,
};
use toniator_io::{PRESET_FORMAT_VERSION, load_preset, save_preset};

/// Returns an isolated derived-artifact directory while leaving immutable
/// project inputs untouched.
fn validation_directory() -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/stage-20e2/preset-io");
    fs::create_dir_all(&directory).unwrap();
    directory
}

/// Round-trips a preset-v4 ID-free self-intersecting shape recipe without allocating IDs.
#[test]
fn authored_closed_shape_preset_round_trips_as_embedded_recipe_geometry() {
    let points = [
        AuthoredPoint2 { x: -1.0, y: -1.0 },
        AuthoredPoint2 { x: 1.0, y: 1.0 },
        AuthoredPoint2 { x: -1.0, y: 1.0 },
        AuthoredPoint2 { x: 1.0, y: -1.0 },
    ];
    let shape = AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        (0..4)
            .map(|index| AuthoredCurveSegment::Line {
                start: points[index],
                end: points[(index + 1) % 4],
            })
            .collect(),
    )
    .expect("the bow-tie fixture is a finite closed shape");
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "shape-round-trip".into(),
            name: "Shape Round Trip".into(),
            category: "Test".into(),
            description: "ID-free authored-shape preset fixture.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::AuthoredClosedShapeMarks {
            definition: Box::new(PatternStructureRecipe::StraightGrid(
                PatternDefinitionDraft {
                    name: "Bow-tie grid".into(),
                    coverage: CoveragePolicy {
                        guard_steps: 2,
                        additional_margin: 4.5,
                    },
                },
            )),
            shape,
        }),
    };
    let path = validation_directory().join("shape-round-trip.preset.json");
    save_preset(&path, &preset).expect("the valid shape preset saves");
    assert_eq!(
        load_preset(&path).expect("the current shape preset reloads"),
        preset
    );
    let text = fs::read_to_string(path).expect("the derived preset remains readable");
    assert!(text.contains("\"kind\": \"authored_closed_shape_marks\""));
    assert!(!text.contains("structure_id"));
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
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::StraightGrid(
            PatternDefinitionDraft {
                name: "Round-trip grid".into(),
                coverage: CoveragePolicy {
                    guard_steps: 2,
                    additional_margin: 4.5,
                },
            },
        )),
    };
    let path = directory.join("io-round-trip.preset.json");
    save_preset(&path, &preset).unwrap();
    assert_eq!(load_preset(&path).unwrap(), preset);
    let text = fs::read_to_string(path).unwrap();
    assert!(text.contains(&format!(
        "\"preset_format_version\": {PRESET_FORMAT_VERSION}"
    )));
}

/// Round-trips the v4 tagged parametric recipe without allocating document-owned IDs.
#[test]
fn parametric_recipe_round_trips_without_a_preset_format_bump() {
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "parametric-v4".into(),
            name: "Parametric v4".into(),
            category: "Test".into(),
            description: "ID-free parametric recipe serialization fixture.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::ParametricCurve {
            name: "Square spiral".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
            curve: ParametricCurve::Spiral(SpiralCurve {
                shape: SpiralShape::Square,
                turns: 5.0,
                radial_spacing: 16.0,
                phase_degrees: 0.0,
                winding: CurveWinding::Clockwise,
            }),
            spiral_coverage: toniator_domain::SpiralCoveragePolicy::Fixed,
            repetition: CurveRepetition::Single,
            sites: Some(ParametricCurveSiteDraft {
                interval: 16.0,
                phase: 0.0,
            }),
        }),
    };
    let path = validation_directory().join("parametric-v4.preset.json");
    save_preset(&path, &preset).expect("current parametric recipe saves");
    assert_eq!(
        load_preset(&path).expect("current parametric recipe reloads"),
        preset
    );
    let text = fs::read_to_string(&path).expect("derived preset remains readable");
    assert_eq!(
        text,
        r#"{
  "preset_format_version": 4,
  "metadata": {
    "id": "parametric-v4",
    "name": "Parametric v4",
    "category": "Test",
    "description": "ID-free parametric recipe serialization fixture.",
    "thumbnail": null
  },
  "recipe": {
    "structure": {
      "kind": "parametric_curve",
      "name": "Square spiral",
      "coverage": {
        "guard_steps": 2,
        "additional_margin": 0.0
      },
      "curve": {
        "kind": "spiral",
        "shape": "square",
        "turns": 5.0,
        "radial_spacing": 16.0,
        "phase_degrees": 0.0,
        "winding": "clockwise"
      },
      "spiral_coverage": "fixed",
      "repetition": {
        "kind": "single"
      },
      "sites": {
        "interval": 16.0,
        "phase": 0.0
      }
    },
    "output_settings": [
      {
        "source_filter": {
          "kind": "all"
        },
        "response": {
          "kind": "marks",
          "response": {
            "minimum_fill": 0.0,
            "maximum_fill": 1.0
          }
        }
      }
    ]
  }
}"#
    );
    fs::write(&path, text.replace("\"fixed\"", "\"unknown\""))
        .expect("malformed coverage policy writes");
    assert!(
        load_preset(&path).is_err(),
        "unknown coverage policy is rejected"
    );
}

/// Rejects unknown and obsolete current preset-v4 fields before any recipe reaches domain validation.
#[test]
fn preset_v4_rejects_unknown_envelope_and_obsolete_recipe_fields() {
    let directory = validation_directory();
    let unknown = directory.join("unknown-envelope-field.preset.json");
    fs::write(
        &unknown,
        r#"{"preset_format_version":4,"metadata":{"id":"x","name":"X","category":"Test","description":"Test","thumbnail":null},"recipe":{"structure":{"kind":"straight_grid","name":"Grid","coverage":{"guard_steps":2,"additional_margin":0}},"output_settings":[{"source_filter":{"kind":"all"},"response":{"kind":"marks","response":{"minimum_fill":0.25,"maximum_fill":0.85}}}]},"obsolete":true}"#,
    )
    .expect("derived malformed preset writes");
    assert!(load_preset(&unknown).is_err());

    let obsolete = directory.join("obsolete-structure-field.preset.json");
    fs::write(
        &obsolete,
        r#"{"preset_format_version":4,"metadata":{"id":"x","name":"X","category":"Test","description":"Test","thumbnail":null},"recipe":{"structure":{"kind":"straight_grid","name":"Grid","coverage":{"guard_steps":2,"additional_margin":0},"legacy_diameter":2},"output_settings":[{"source_filter":{"kind":"all"},"response":{"kind":"marks","response":{"minimum_fill":0.25,"maximum_fill":0.85}}}]}}"#,
    )
    .expect("derived obsolete preset writes");
    assert!(load_preset(&obsolete).is_err());

    let retired_visible_margin = directory.join("retired-visible-mark-margin.preset.json");
    fs::write(
        &retired_visible_margin,
        r#"{"preset_format_version":4,"metadata":{"id":"x","name":"X","category":"Test","description":"Test","thumbnail":null},"recipe":{"structure":{"kind":"straight_grid","name":"Grid","coverage":{"guard_steps":2,"additional_margin":0},"visible_mark_margin":1},"output_settings":[{"source_filter":{"kind":"all"},"response":{"kind":"marks","response":{"minimum_fill":0.25,"maximum_fill":0.85}}}]}}"#,
    )
    .expect("derived retired-field preset writes");
    assert!(
        load_preset(&retired_visible_margin).is_err(),
        "current preset-v4 strictly rejects the retired visible-mark margin field"
    );
}

/// Rejects unknown or retired fields inside current preset-v4 parametric and
/// generalized-recipe nested DTOs before domain recipe validation can discard them.
#[test]
fn preset_v4_rejects_unknown_nested_recipe_fields() {
    let directory = validation_directory();
    let parametric = serde_json::json!({
        "preset_format_version": 4,
        "metadata": {
            "id": "nested-parametric",
            "name": "Nested Parametric",
            "category": "Test",
            "description": "Nested DTO strictness fixture.",
            "thumbnail": null
        },
        "recipe": {
            "structure": {
                "kind": "parametric_curve",
                "name": "Round spiral",
                "coverage": { "guard_steps": 2, "additional_margin": 0.0 },
                "curve": {
                    "kind": "spiral",
                    "shape": "round",
                    "turns": 2.0,
                    "radial_spacing": 8.0,
                    "phase_degrees": 0.0,
                    "winding": "clockwise"
                },
                "spiral_coverage": "fixed",
                "repetition": { "kind": "single" },
                "sites": { "interval": 8.0, "phase": 0.0 }
            },
            "output_settings": [{
                "source_filter": { "kind": "all" },
                "response": {
                    "kind": "marks",
                    "response": { "minimum_fill": 0.25, "maximum_fill": 0.85 }
                }
            }]
        }
    });
    let generalized = serde_json::json!({
        "preset_format_version": 4,
        "metadata": {
            "id": "nested-generalized",
            "name": "Nested Generalized",
            "category": "Test",
            "description": "Nested DTO strictness fixture.",
            "thumbnail": null
        },
        "recipe": {
            "structure": {
                "kind": "generalized_straight_guides",
                "name": "One guide",
                "coverage": { "guard_steps": 2, "additional_margin": 0.0 },
                "dimensions": [{
                    "baseline_angle_degrees": 0.0,
                    "phase": 0.0,
                    "spacing_multiplier": 1.0
                }],
                "product": {
                    "kind": "along_guides",
                    "dimension_indices": [0],
                    "interval_multiplier": 1.0,
                    "phase": 0.0
                },
                "orientation": { "kind": "guide_tangent", "dimension_index": 0 }
            },
            "output_settings": [{
                "source_filter": { "kind": "all" },
                "response": {
                    "kind": "marks",
                    "response": { "minimum_fill": 0.25, "maximum_fill": 0.85 }
                }
            }]
        }
    });

    let valid_parametric = directory.join("nested-parametric-valid.preset.json");
    fs::write(
        &valid_parametric,
        serde_json::to_vec_pretty(&parametric).expect("valid parametric fixture serializes"),
    )
    .expect("valid parametric fixture writes");
    assert!(
        load_preset(&valid_parametric).is_ok(),
        "the unmodified current-v4 parametric fixture remains accepted"
    );

    let valid_generalized = directory.join("nested-generalized-valid.preset.json");
    fs::write(
        &valid_generalized,
        serde_json::to_vec_pretty(&generalized).expect("valid generalized fixture serializes"),
    )
    .expect("valid generalized fixture writes");
    assert!(
        load_preset(&valid_generalized).is_ok(),
        "the unmodified current-v4 generalized fixture remains accepted"
    );

    let mut unknown_sites = parametric.clone();
    unknown_sites["recipe"]["structure"]["sites"]["obsolete_interval"] = serde_json::json!(4.0);
    let mut unknown_dimension = generalized.clone();
    unknown_dimension["recipe"]["structure"]["dimensions"][0]["obsolete_baseline"] =
        serde_json::json!(45.0);
    let mut unknown_product = generalized.clone();
    unknown_product["recipe"]["structure"]["product"]["obsolete_product"] = serde_json::json!(true);
    let mut unknown_orientation = generalized;
    unknown_orientation["recipe"]["structure"]["orientation"]["obsolete_orientation"] =
        serde_json::json!(true);

    for (name, malformed) in [
        ("unknown-parametric-sites", unknown_sites),
        ("unknown-guide-dimension", unknown_dimension),
        ("unknown-generalized-product", unknown_product),
        ("unknown-mark-orientation", unknown_orientation),
    ] {
        let path = directory.join(format!("{name}.preset.json"));
        fs::write(
            &path,
            serde_json::to_vec_pretty(&malformed).expect("malformed fixture serializes"),
        )
        .expect("malformed fixture writes");
        assert!(
            load_preset(&path).is_err(),
            "current-v4 rejects the nested {name} field instead of silently discarding it"
        );
    }
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
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::StraightGrid(
            PatternDefinitionDraft {
                name: "Grid".into(),
                coverage: CoveragePolicy {
                    guard_steps: 2,
                    additional_margin: 4.5,
                },
            },
        )),
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
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::GeneralizedStraightGuides {
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
        }),
    };
    assert!(save_preset(&invalid_path, &invalid_recipe).is_err());
    assert!(!invalid_path.exists());
}
