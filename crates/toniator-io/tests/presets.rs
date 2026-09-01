use std::{fs, path::PathBuf};

use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft, AuthoredStructureKind,
    ConnectedGeometryResponse, CoveragePolicy, CurveRepetition, CurveWinding,
    GeneralizedSiteProductDraft, GenericGuideDimensionDraft, GenericGuidePrototypeDraft,
    GuideDimensionDraft, GuideRepetition, MarkGeometryResponse, MarkOrientationDraft,
    ParametricCurve, ParametricCurveSiteDraft, PathStrokeStyle, PatternDefinitionDraft,
    PatternDefinitionRecipe, PatternGeometryResponse, PatternOutputRealizationRecipe,
    PatternOutputSettingsRecipe, PatternStructureRecipe, PresetMetadata, PresetRecord,
    SiteUseFilterRecipe, SpiralCurve, SpiralShape,
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

/// Round-trips embedded authored ordered outputs through the unchanged v4 DTO language.
#[test]
fn ordered_authored_outputs_round_trip_without_document_resource_ids() {
    let shape = AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: -0.5, y: 0.0 },
                end: AuthoredPoint2 { x: 0.5, y: 0.0 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.5, y: 0.0 },
                end: AuthoredPoint2 { x: -0.5, y: 0.0 },
            },
        ],
    )
    .expect("ordered authored shape validates");
    let motif = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 0.0 },
            end: AuthoredPoint2 { x: 1.0, y: 0.0 },
        }],
    )
    .expect("ordered Curve Motif validates");
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "ordered-authored-v4".into(),
            name: "Ordered Authored v4".into(),
            category: "Test".into(),
            description: "ID-free authored ordered outputs fixture.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe {
            structure: PatternStructureRecipe::AuthoredResources {
                resources: vec![shape, motif],
                definition: Box::new(PatternStructureRecipe::OrderedOutputs {
                    definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
                        name: "one guide".into(),
                        coverage: CoveragePolicy {
                            guard_steps: 1,
                            additional_margin: 0.0,
                        },
                        dimensions: vec![GuideDimensionDraft {
                            baseline_angle_degrees: 0.0,
                            phase: 0.0,
                            spacing_multiplier: 1.0,
                        }],
                        product: GeneralizedSiteProductDraft::AlongGuides {
                            dimension_indices: vec![0],
                            interval_multiplier: 1.0,
                            phase: 0.0,
                        },
                        orientation: MarkOrientationDraft::Fixed,
                    }),
                    outputs: vec![
                        PatternOutputRealizationRecipe::AuthoredClosedShapeMarks {
                            resource_index: 0,
                            orientation: MarkOrientationDraft::Fixed,
                        },
                        PatternOutputRealizationRecipe::CurveMotifPaths {
                            resource_index: 1,
                            style: PathStrokeStyle::default(),
                            mirror_alternate_rows: true,
                            alternate_row_phase: Some(0.25),
                        },
                    ],
                }),
            },
            output_settings: vec![
                PatternOutputSettingsRecipe {
                    source_filter: SiteUseFilterRecipe::All,
                    response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                        minimum_fill: 0.0,
                        maximum_fill: 1.0,
                    }),
                },
                PatternOutputSettingsRecipe {
                    source_filter: SiteUseFilterRecipe::SitesUsedBy { output_index: 0 },
                    response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                        minimum_thickness: 0.0,
                        maximum_thickness: 1.0,
                        bias: 0.0,
                    }),
                },
            ],
        },
    };
    let path = validation_directory().join("ordered-authored-v4.preset.json");
    save_preset(&path, &preset).expect("ordered authored v4 preset saves");
    assert_eq!(
        load_preset(&path).expect("ordered authored v4 preset loads"),
        preset
    );
    assert!(
        !fs::read_to_string(path)
            .expect("serialized preset reads")
            .contains("structure_id")
    );
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
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::AuthoredResources {
            resources: vec![shape],
            definition: Box::new(PatternStructureRecipe::AuthoredClosedShapeMarks {
                definition: Box::new(PatternStructureRecipe::StraightGrid(
                    PatternDefinitionDraft {
                        name: "Bow-tie grid".into(),
                        coverage: CoveragePolicy {
                            guard_steps: 2,
                            additional_margin: 4.5,
                        },
                    },
                )),
                resource_index: 0,
            }),
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

/// Round-trips an embedded generic authored guide through unchanged preset-v4 DTO authority.
#[test]
fn generic_guide_preset_round_trips_without_document_resource_ids() {
    let path = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 0.0 },
            end: AuthoredPoint2 { x: 1.0, y: 0.0 },
        }],
    )
    .expect("open generic guide payload validates");
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "generic-guide-v4".into(),
            name: "Generic Guide v4".into(),
            category: "Test".into(),
            description: "ID-free generic guide recipe fixture.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::AuthoredResources {
            resources: vec![path],
            definition: Box::new(PatternStructureRecipe::GenericGuides {
                name: "generic guide".into(),
                coverage: CoveragePolicy {
                    guard_steps: 1,
                    additional_margin: 0.0,
                },
                dimensions: vec![GenericGuideDimensionDraft {
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    prototype: GenericGuidePrototypeDraft::AuthoredOpenPathReference {
                        resource_index: 0,
                    },
                    repetition: GuideRepetition::Single,
                }],
                product: GeneralizedSiteProductDraft::AlongGuides {
                    dimension_indices: vec![0],
                    interval_multiplier: 1.0,
                    phase: 0.0,
                },
                orientation: MarkOrientationDraft::Fixed,
            }),
        }),
    };
    let path = validation_directory().join("generic-guide-v4.preset.json");
    save_preset(&path, &preset).expect("current v4 generic guide saves");
    assert_eq!(
        load_preset(&path).expect("current v4 generic guide loads"),
        preset
    );
    let text = fs::read_to_string(path).expect("derived v4 preset remains readable");
    assert!(text.contains("\"kind\": \"generic_guides\""));
    assert!(!text.contains("structure_id"));
}

/// Rejects invalid current-v4 root-table resource references without accepting retired IDs.
///
/// # Panics
///
/// Panics when a valid aliased root-table recipe stops loading or a missing table, invalid index,
/// wrong resource kind, or retired stable reference reaches materialization.
#[test]
fn preset_v4_root_authored_resources_validate_aliases_and_references() {
    let directory = validation_directory();
    let valid = serde_json::json!({
        "preset_format_version": PRESET_FORMAT_VERSION,
        "metadata": {
            "id": "root-authored-resources-v4",
            "name": "Root Authored Resources v4",
            "category": "Test",
            "description": "Strict root table fixture.",
            "thumbnail": null
        },
        "recipe": {
            "structure": {
                "kind": "authored_resources",
                "resources": [{
                    "kind": "open_path",
                    "segments": [{
                        "kind": "line",
                        "start": {"x": 0.0, "y": 0.0},
                        "end": {"x": 1.0, "y": 0.0}
                    }]
                }],
                "definition": {
                    "kind": "generic_guides",
                    "name": "Aliased guides",
                    "coverage": {"guard_steps": 1, "additional_margin": 0.0},
                    "dimensions": [{
                        "baseline_angle_degrees": 0.0,
                        "phase": 0.0,
                        "prototype": {"kind": "authored_open_path_reference", "resource_index": 0},
                        "repetition": {"kind": "single"}
                    }, {
                        "baseline_angle_degrees": 90.0,
                        "phase": 0.0,
                        "prototype": {"kind": "authored_open_path_reference", "resource_index": 0},
                        "repetition": {"kind": "single"}
                    }],
                    "product": {
                        "kind": "intersections",
                        "dimension_indices": [0, 1],
                        "merge_epsilon": 0.000001
                    },
                    "orientation": {"kind": "fixed"}
                }
            },
            "output_settings": [{
                "source_filter": {"kind": "all"},
                "response": {"kind": "marks", "response": {"minimum_fill": 0.0, "maximum_fill": 1.0}}
            }]
        }
    });
    let valid_path = directory.join("root-authored-resources-valid.preset.json");
    fs::write(
        &valid_path,
        serde_json::to_vec_pretty(&valid).expect("valid root table fixture serializes"),
    )
    .expect("valid root table fixture writes");
    assert!(load_preset(&valid_path).is_ok());

    let mut missing_table = valid.clone();
    missing_table["recipe"]["structure"] =
        missing_table["recipe"]["structure"]["definition"].clone();
    let mut out_of_range = valid.clone();
    out_of_range["recipe"]["structure"]["definition"]["dimensions"][0]["prototype"]["resource_index"] =
        serde_json::json!(1);
    let mut wrong_kind = valid.clone();
    wrong_kind["recipe"]["structure"]["resources"][0] = serde_json::json!({
        "kind": "closed_shape",
        "segments": [{
            "kind": "line",
            "start": {"x": 0.0, "y": 0.0},
            "end": {"x": 1.0, "y": 0.0}
        }, {
            "kind": "line",
            "start": {"x": 1.0, "y": 0.0},
            "end": {"x": 0.0, "y": 0.0}
        }]
    });
    let mut retired_stable_reference = valid;
    retired_stable_reference["recipe"]["structure"]["definition"]["dimensions"][0]["prototype"]["structure_id"] =
        serde_json::json!(99);
    for (name, malformed) in [
        ("missing-root-table", missing_table),
        ("out-of-range-root-index", out_of_range),
        ("wrong-root-resource-kind", wrong_kind),
        ("retired-stable-reference", retired_stable_reference),
    ] {
        let path = directory.join(format!("{name}.preset.json"));
        fs::write(
            &path,
            serde_json::to_vec_pretty(&malformed).expect("invalid root table fixture serializes"),
        )
        .expect("invalid root table fixture writes");
        assert!(
            load_preset(&path).is_err(),
            "current-v4 rejects {name} rather than allocating or retaining a stable resource ID"
        );
    }
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
