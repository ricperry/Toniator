use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};
use toniator_domain::{
    CanvasSpec, Document, DocumentCommand, DocumentHistory, DocumentSession, PresetMetadata,
    PresetRecord, SourceReference, SourceReferenceId,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, StagedDocumentPreset,
    builtin_document_preset, builtin_document_presets, capture_document_preset_destination, load,
    load_document_preset_staged, load_preset, save, save_document_preset, save_preset,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Creates one current destination without source decoding or user-owned output.
/// # Panics
/// Panics only if the bounded validation fixture cannot be constructed or accessed.
fn destination() -> Document {
    Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("artistic-persistence").unwrap()),
    )
    .unwrap()
}

/// Returns a phase-scoped validation path, with an explicit output override for later bounded reruns.
/// The override prevents current checks from regenerating an earlier stage evidence directory.
/// # Panics
/// Panics only if the bounded validation fixture cannot be constructed or accessed.
fn output(name: &str) -> PathBuf {
    let root = std::env::var_os("TONIATOR_ARTISTIC_VALIDATION_OUTPUT").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/artistic-geometry-20261019/tests"));
    fs::create_dir_all(&root).unwrap();
    root.join(name)
}

/// Decodes one embedded archive's original envelope without rewriting its source bytes.
/// # Panics
/// Panics only if the bounded validation fixture cannot be constructed or accessed.
fn envelope(id: &str) -> Value {
    let preset = builtin_document_preset(id).unwrap();
    let mut archive = ZipArchive::new(std::io::Cursor::new(preset.archive_bytes())).unwrap();
    let mut bytes = Vec::new();
    archive
        .by_name("preset.json")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Writes one derived archive to exercise explicit former-origin dispatch.
/// # Panics
/// Panics only if the bounded validation fixture cannot be constructed or accessed.
fn write_envelope(name: &str, value: &Value) -> PathBuf {
    let path = output(name);
    let mut writer = ZipWriter::new(fs::File::create(&path).unwrap());
    writer
        .start_file("preset.json", SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(&serde_json::to_vec(value).unwrap())
        .unwrap();
    writer.finish().unwrap();
    path
}

/// Serializes a converted configuration so coverage fields can be compared through the real DTO.
/// # Panics
/// Panics only when a valid bounded test configuration cannot serialize or decode.
fn serialized_configuration(
    name: &str,
    configuration: &toniator_domain::DocumentConfiguration,
) -> Value {
    let path = output(name);
    save_document_preset(
        &path,
        configuration,
        &capture_document_preset_destination(&path).unwrap(),
    )
    .unwrap();
    let mut archive = ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut bytes = Vec::new();
    archive
        .by_name("preset.json")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()["configuration"].clone()
}

/// Stages every immutable schema-16 catalog archive without misclassifying desired coverage as raw size.
/// # Panics
/// Panics if conversion loses any ALL/named desired coverage or an archive cannot stage.
#[test]
fn schema16_catalog_stages_coverage_and_preserves_original_bytes() {
    let destination = destination();
    for preset in builtin_document_presets() {
        let before = preset.archive_bytes().to_vec();
        let original = envelope(preset.id);
        let StagedDocumentPreset::PendingCalibration(configuration) =
            preset.load_staged(&destination).unwrap()
        else {
            panic!("schema16 coverage must require only calibration preparation");
        };
        let candidate = configuration.bind(&destination).unwrap();
        let converted = serialized_configuration(
            &format!("catalog-{}-converted.toniator-preset", preset.id),
            &configuration,
        );
        assert_eq!(
            converted["response_settings"],
            original["configuration"]["response_settings"]
        );
        assert_eq!(
            converted["channel_response_overrides"],
            original["configuration"]["channel_response_overrides"]
        );
        assert_eq!(
            converted.get("end_overrides").cloned().unwrap_or(json!([])),
            original["configuration"]
                .get("end_overrides")
                .cloned()
                .unwrap_or(json!([]))
        );

        assert_eq!(
            candidate.response_settings().connected_minimum_thickness,
            original["configuration"]["response_settings"]["connected_minimum_thickness"]
                .as_f64()
                .unwrap()
        );
        assert_eq!(preset.archive_bytes(), before);
    }
}

/// Separates raw schema-15 endpoints from absolute schema-16 coverage without age-based heuristics.
/// # Panics
/// Panics if either recognized origin reaches the wrong explicit preparation route.
#[test]
fn schema15_remains_raw_and_schema16_remains_desired_coverage() {
    let destination = destination();
    let mut former = envelope("magazine-rgb");
    former["document_schema_version"] = json!(15);
    former["configuration"]["response_settings"]["marks_maximum_fill"] = json!(1.5);
    former["configuration"]["saved_calibrations"] = json!([]);
    former["configuration"]["baseline_saved_calibration"] = Value::Null;
    let path = write_envelope("schema15-raw.toniator-preset", &former);
    assert!(matches!(
        load_document_preset_staged(&path, &destination).unwrap(),
        StagedDocumentPreset::PendingRaw(_)
    ));
    former["document_schema_version"] = json!(16);
    former["configuration"]["response_settings"]["marks_maximum_fill"] = json!(0.75);
    let path = write_envelope("schema16-coverage.toniator-preset", &former);
    let StagedDocumentPreset::PendingCalibration(configuration) =
        load_document_preset_staged(&path, &destination).unwrap()
    else {
        panic!("schema16 is coverage");
    };
    assert_eq!(
        configuration
            .bind(&destination)
            .unwrap()
            .response_settings()
            .marks
            .maximum_fill,
        0.75
    );
}

/// Preserves distinct ALL, named and End coverage targets through affected calibration removal.
/// # Panics
/// Panics if source-free conversion reinterprets current coverage as raw thickness or loses animation intent.
#[test]
fn schema16_distinct_all_named_and_end_targets_survive_conversion() {
    let destination = destination();
    let mut former = envelope("controlled-splatter-cmyk");
    let configuration = &mut former["configuration"];
    configuration["response_settings"]["connected_minimum_thickness"] = json!(0.13);
    configuration["response_settings"]["connected_maximum_thickness"] = json!(0.77);
    let channel = configuration["channel_configuration"]["channels"][0]["id"]
        .as_u64()
        .unwrap();
    let base = configuration["pattern_settings"]["definition_id"]
        .as_u64()
        .unwrap();
    let output_id = configuration["pattern_definition_bundles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|bundle| bundle["definition"]["id"].as_u64() == Some(base))
        .unwrap()["definition"]["output_layers"][0]["id"]
        .as_u64()
        .unwrap();
    configuration["channel_response_overrides"] = json!([{"channel_id":channel,"output_layer_id":output_id,"response":{"kind":"connected","minimum_thickness":0.21,"maximum_thickness":0.68}}]);
    // Persisted response End is additive: desired0.44 is named0.68 plus-0.24.
    configuration["end_overrides"] = json!([{"kind":"scalar","target":{"scope":"channel_output","channel_id":channel,"output_id":output_id},"field":"connected_maximum_thickness","end":-0.24,"easing":"smooth_step"}]);
    let path = write_envelope("schema16-distinct-coverage.toniator-preset", &former);
    let StagedDocumentPreset::PendingCalibration(converted) =
        load_document_preset_staged(&path, &destination).unwrap()
    else {
        panic!("desired coverage requires calibration only");
    };
    let serialized =
        serialized_configuration("schema16-distinct-converted.toniator-preset", &converted);
    for field in [
        "response_settings",
        "channel_response_overrides",
        "end_overrides",
    ] {
        assert_eq!(serialized[field], former["configuration"][field]);
    }
    assert!(
        serialized["saved_calibrations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(serialized["baseline_saved_calibration"].is_null());
}

/// Refuses superseded ordinary project schema before decoding document or source data.
/// # Panics
/// Panics if a schema17 ordinary project enters the current-only project decoder.
#[test]
fn ordinary_project_schema17_rejects_without_conversion() {
    let path = output("obsolete-project17.toniator");
    let mut writer = ZipWriter::new(fs::File::create(&path).unwrap());
    writer
        .start_file("document.json", SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(br#"{"container_version":2,"document_schema_version":17,"document":{}}"#)
        .unwrap();
    writer
        .add_directory("sources/", SimpleFileOptions::default())
        .unwrap();
    writer.finish().unwrap();
    let error = load(&path).unwrap_err();
    assert!(
        matches!(&error, toniator_io::LoadError::Version { context } if context.contains("17")),
        "{error:?}"
    );
}

/// Proves nondefault nested output fields survive current Pattern serialization and strict parsing.
/// # Panics
/// Panics if current recipe conversion loses fields or accepts obsolete/missing-field data.
#[test]
fn nondefault_nested_fields_roundtrip_and_current_fields_are_required() {
    let destination = destination();
    for (id, tone) in [
        ("skinny-wave-rgb", true),
        ("controlled-splatter-cmyk", false),
    ] {
        let StagedDocumentPreset::PendingCalibration(configuration) = builtin_document_preset(id)
            .unwrap()
            .load_staged(&destination)
            .unwrap()
        else {
            panic!("former catalog stages");
        };
        let candidate = configuration.bind(&destination).unwrap();
        let recipe = candidate
            .reconstruct_pattern_definition_recipe(candidate.pattern_settings().definition_id)
            .unwrap();
        let record = PresetRecord {
            metadata: PresetMetadata {
                id: id.into(),
                name: id.into(),
                category: "Validation".into(),
                description: "Current artistic intent".into(),
                thumbnail: None,
            },
            recipe,
            saved_calibration: None,
        };
        let path = output(&format!("{id}.pattern.json"));
        save_preset(&path, &record).unwrap();
        let mut data: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let text = serde_json::to_string(&data).unwrap();
        assert!(text.contains(if tone {
            "tone_sample_spacing"
        } else {
            "selection"
        }));
        // Edit the complete typed JSON, then roundtrip the decoded domain recipe.
        /// Changes only derived test intent to witness nondefault DTO preservation.
        fn alter(value: &mut Value, tone: bool) {
            match value {
                Value::Object(object) => {
                    if tone && object.contains_key("tone_sample_spacing") {
                        object.insert("tone_sample_spacing".into(), json!({"multiplier":2.75}));
                    }
                    if !tone && object.contains_key("selection") {
                        object.insert("selection".into(), json!({"maximum_final_degree":2,"cycles":"forbid","minimum_component_points":4}));
                    }
                    for child in object.values_mut() {
                        alter(child, tone);
                    }
                }
                Value::Array(array) => {
                    for child in array {
                        alter(child, tone);
                    }
                }
                _ => {}
            }
        }
        alter(&mut data, tone);
        fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
        let changed = load_preset(&path).unwrap();
        assert_ne!(changed.recipe, record.recipe);
        save_preset(&path, &changed).unwrap();
        assert_eq!(load_preset(&path).unwrap().recipe, changed.recipe);
        let mut history = DocumentHistory::new(DocumentSession::new(candidate.clone()).unwrap());
        let base = candidate.pattern_settings().clone();
        let base_definition = candidate
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == base.definition_id)
            .unwrap()
            .definition
            .clone();
        history
            .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base,
                base_definition,
                recipe: changed.recipe.clone(),
            })
            .unwrap();
        let rebound = history
            .document()
            .reconstruct_pattern_definition_recipe(
                history.document().pattern_settings().definition_id,
            )
            .unwrap();
        assert_eq!(rebound, changed.recipe);
        let project = output(&format!("{id}-current.toniator"));
        save(
            &project,
            history.document(),
            &SourceBundle::new([EmbeddedSource::new(
                SourceReferenceId::new("artistic-persistence").unwrap(),
                EmbeddedSourceFormat::Svg,
                b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\"/>".to_vec(),
                Some("fixture.svg".into()),
            )
            .unwrap()])
            .unwrap(),
        )
        .unwrap();
        assert_eq!(load(&project).unwrap().document(), history.document());
        // A current-format field is required; only the explicit former Document Preset converter supplies defaults.
        let original_text = fs::read_to_string(&path).unwrap();
        let stripped = original_text.replace(
            if tone {
                "\"tone_sample_spacing\""
            } else {
                "\"selection\""
            },
            "\"unrecognized_artistic_field\"",
        );
        fs::write(&path, stripped).unwrap();
        assert!(load_preset(&path).is_err());

        let mut old = data;
        old["preset_format_version"] = json!(8);
        fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        assert!(load_preset(&path).is_err());
    }
}

/// Clears only recognized prior producer fits in current source-free Presets before strict parsing.
/// Original bytes and desired coverage remain authoritative; unknown future profile strings reject.
/// # Panics
/// Panics if guide-span-suffixed numeric caches survive or current/unknown profiles are misclassified.
#[test]
fn current_source_free_presets_clear_exact_prior_suffixed_producer_profiles() {
    use toniator_domain::{
        CalibrationBinding, CalibrationKnot, SavedCalibration, DocumentConfiguration,
    };
    let target = destination();
    let StagedDocumentPreset::PendingCalibration(configuration) =
        builtin_document_preset("skinny-wave-rgb")
            .unwrap()
            .load_staged(&target)
            .unwrap()
    else {
        panic!("old archive stages")
    };
    let mut document = configuration.bind(&target).unwrap();
    let ids = document
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|c| c.id)
        .collect::<Vec<_>>();
    let raw = vec![
        CalibrationKnot {
            input: 0.,
            coverage: 0.,
        },
        CalibrationKnot {
            input: 2.,
            coverage: 1.,
        },
    ];
    for id in ids {
        let binding = CalibrationBinding::for_channel(&document, id).unwrap();
        let fit = SavedCalibration::new(binding, raw.clone(), None).unwrap();
        document = document.with_saved_calibration(id, fit).unwrap();
    }
    let fit =
        SavedCalibration::new(document.baseline_calibration_binding().unwrap(), raw, None).unwrap();
    document = document.with_saved_baseline_calibration(fit).unwrap();
    let path = output("current-producer-profile.toniator-preset");
    save_document_preset(
        &path,
        &DocumentConfiguration::capture(&document),
        &capture_document_preset_destination(&path).unwrap(),
    )
    .unwrap();
    let mut archive = ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
    let mut bytes = Vec::new();
    archive
        .by_name("preset.json")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let mut value: Value = serde_json::from_slice(&bytes).unwrap();
    let text = serde_json::to_string(&value).unwrap();
    assert!(text.contains("+guide-span-v2+anchored-tone-v1"));
    /// Replaces only the exact new producer marker in calibration metadata, preserving recipe values.
    fn prior(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if let Some(Value::String(profile)) = object.get_mut("profile") {
                    *profile = profile.replace("+anchored-tone-v1", "");
                }
                for child in object.values_mut() {
                    prior(child);
                }
            }
            Value::Array(values) => {
                for child in values {
                    prior(child);
                }
            }
            _ => {}
        }
    }
    prior(&mut value);
    let path = write_envelope("current-former-guide-profile.toniator-preset", &value);
    let original = fs::read(&path).unwrap();
    let staged = load_document_preset_staged(&path, &target).unwrap();
    let converted = match staged {
        StagedDocumentPreset::Current(c) | StagedDocumentPreset::PendingCalibration(c) => c,
        StagedDocumentPreset::PendingRaw(_) => panic!("desired coverage must not become raw"),
    };
    let rebound = converted.bind(&target).unwrap();
    assert!(rebound.saved_calibrations().is_empty());
    assert!(rebound.baseline_saved_calibration().unwrap().is_none());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(rebound.response_settings(), document.response_settings());
    let profiles = &mut value["configuration"]["saved_calibrations"];
    profiles[0]["profile"] =
        json!("ton018-reference-fixed-canvas1024-spacings32-v5+unknown-future");
    let unknown = write_envelope("unknown-producer-profile.toniator-preset", &value);
    assert!(load_document_preset_staged(&unknown, &target).is_err());
}
