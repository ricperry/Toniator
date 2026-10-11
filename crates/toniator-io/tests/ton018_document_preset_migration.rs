use serde_json::Value;
use serde_json::json;
use std::{
    env,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};
use toniator_domain::{
    CanvasSpec, ChannelId, DEFAULT_RANDOM_NOMINAL_SPACING, Document, DocumentHistory,
    DocumentSession, PatternGeometryResponse, PatternStructureRecipe, RandomSiteCharacter,
    SourceReference,
};
use toniator_io::{
    capture_document_preset_destination, load_document_preset, save_document_preset,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Exercises copies of all observed personal legacy Presets against a current destination.
///
/// # Panics
/// Panics if a recognized version cannot migrate and bind, or an active response value changes.
#[test]
fn observed_legacy_document_presets_migrate() {
    let Ok(directory) = env::var("TONIATOR_PRESET_MIGRATION_DIR") else {
        return;
    };
    let destination = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    for name in [
        "ControlledSplatterCMYK",
        "HexGridDotsAlignedRGB",
        "SkinnyCurve",
        "RGB_magazine",
        "CMYK_magazine",
        "CMYK_skinnyWave",
        "RGB_skinnyWave",
    ] {
        let path = Path::new(&directory).join(format!("{name}.toniator-preset"));
        let mut archive = ZipArchive::new(File::open(&path).unwrap()).unwrap();
        let mut old_bytes = Vec::new();
        archive
            .by_name("preset.json")
            .unwrap()
            .read_to_end(&mut old_bytes)
            .unwrap();
        let old: Value = serde_json::from_slice(&old_bytes).unwrap();
        let old_config = &old["configuration"];
        let configuration = load_document_preset(&path, &destination)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let bound = configuration.bind(&destination).unwrap();
        assert_eq!(
            bound.pattern_definition_bundles().len(),
            old_config["pattern_definition_bundles"]
                .as_array()
                .unwrap()
                .len()
        );
        assert_eq!(
            bound.pattern_settings().density.density,
            old_config["pattern_settings"]["density"]["density"]
                .as_f64()
                .unwrap()
        );
        for old_channel in old_config["channel_configuration"]["channels"]
            .as_array()
            .unwrap()
        {
            let id = old_channel["id"].as_u64().unwrap();
            let old_definition_id = old_channel["pattern_instance"]["definition_override"]
                .as_u64()
                .unwrap_or(
                    old_config["pattern_settings"]["definition_id"]
                        .as_u64()
                        .unwrap(),
                );
            let old_bundle = old_config["pattern_definition_bundles"]
                .as_array()
                .unwrap()
                .iter()
                .find(|bundle| bundle["definition"]["id"].as_u64() == Some(old_definition_id))
                .unwrap();
            let effective = bound.effective_channel_pattern(ChannelId(id)).unwrap();
            for old_setting in old_bundle["output_settings"].as_array().unwrap() {
                let output_id = old_setting["output_layer_id"].as_u64().unwrap();
                let old_response = &old_setting["response"];
                let old_values = &old_response["response"];
                let new_response = &effective
                    .output_settings
                    .iter()
                    .find(|setting| setting.output_layer_id.0 == output_id)
                    .unwrap()
                    .response;
                let old_delta = old_channel["pattern_instance"]["output_response_deltas"]
                    .as_array()
                    .and_then(|deltas| {
                        deltas
                            .iter()
                            .find(|delta| delta["output_layer_id"].as_u64() == Some(output_id))
                    })
                    .map(|delta| &delta["delta"]["delta"]);
                let value = |field: &str, delta_field: &str| {
                    old_values[field].as_f64().unwrap()
                        + old_delta
                            .and_then(|delta| delta[delta_field].as_f64())
                            .unwrap_or(0.0)
                };
                match new_response {
                    PatternGeometryResponse::Marks(current) => {
                        assert_eq!(
                            current.minimum_fill,
                            value("minimum_fill", "minimum_fill_delta"),
                            "{name} channel {id}"
                        );
                        assert_eq!(
                            current.maximum_fill,
                            value("maximum_fill", "maximum_fill_delta"),
                            "{name} channel {id}"
                        );
                    }
                    PatternGeometryResponse::Connected(current) => {
                        assert_eq!(
                            current.minimum_thickness,
                            value("minimum_thickness", "minimum_thickness_delta"),
                            "{name} channel {id}"
                        );
                        assert_eq!(
                            current.maximum_thickness,
                            value("maximum_thickness", "maximum_thickness_delta"),
                            "{name} channel {id}"
                        );
                        assert_eq!(
                            current.bias,
                            value("bias", "bias_delta"),
                            "{name} channel {id}"
                        );
                    }
                    PatternGeometryResponse::Regions(current) => {
                        assert_eq!(
                            current.minimum_fill,
                            value("minimum_fill", "minimum_fill_delta"),
                            "{name} channel {id}"
                        );
                        assert_eq!(
                            current.maximum_fill,
                            value("maximum_fill", "maximum_fill_delta"),
                            "{name} channel {id}"
                        );
                    }
                }
            }
        }
        if name == "ControlledSplatterCMYK" {
            assert_eq!(bound.response_settings().connected_maximum_thickness, 1.2);
        }
        if name == "HexGridDotsAlignedRGB" {
            assert_eq!(bound.response_settings().regions_maximum_fill, 1.0);
        }
        if let Ok(output_dir) = env::var("TONIATOR_PRESET_MIGRATION_OUTPUT_DIR") {
            let saved_path = Path::new(&output_dir).join(format!("{name}.toniator-preset"));
            let guard = capture_document_preset_destination(&saved_path).unwrap();
            save_document_preset(&saved_path, &configuration, &guard).unwrap();
            let reloaded = load_document_preset(&saved_path, &destination)
                .unwrap()
                .bind(&destination)
                .unwrap();
            assert_eq!(reloaded.pattern_settings(), bound.pattern_settings());
            assert_eq!(reloaded.response_settings(), bound.response_settings());
            assert_eq!(
                reloaded.channel_response_overrides(),
                bound.channel_response_overrides()
            );
        }
    }
}

/// Builds a small version-one Preset from three current concrete output kinds and tests the
/// explicit endpoint/delta migration without relying on personal files or environment variables.
///
/// # Panics
/// Panics if old ALL or named response intent, missing random defaults, or future rejection drifts.
#[test]
fn synthetic_legacy_response_and_spacing_conversion() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018-saved-calibration-20261011/io");
    fs::create_dir_all(&directory).unwrap();
    let destination = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(destination.clone()).unwrap());
    let registry = toniator_patterns::PresetRegistry::bundled();
    let mut raw = registry.reconstruct("even-random-circles").unwrap();
    if let PatternStructureRecipe::RandomSites {
        character,
        nominal_spacing,
        ..
    } = &mut raw.structure
    {
        *character = RandomSiteCharacter::RawUniform;
        *nominal_spacing = Some(DEFAULT_RANDOM_NOMINAL_SPACING);
    } else {
        panic!("random fixture recipe changed");
    }
    let base_definition = history
        .document()
        .pattern_definition_for(ChannelId(1))
        .unwrap()
        .clone();
    history
        .apply(
            &toniator_domain::DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base: history.document().pattern_settings().clone(),
                channel_id: ChannelId(1),
                base_definition,
                recipe: raw,
            },
        )
        .unwrap();
    registry
        .apply_to_selected(&mut history, ChannelId(2), "one-guide-lines")
        .unwrap();
    registry
        .apply_to_selected(&mut history, ChannelId(3), "grid-voronoi-scale")
        .unwrap();
    let current = directory.join("synthetic-source.toniator-preset");
    save_document_preset(
        &current,
        &toniator_domain::DocumentConfiguration::capture(history.document()),
        &capture_document_preset_destination(&current).unwrap(),
    )
    .unwrap();
    let mut bytes = Vec::new();
    ZipArchive::new(File::open(&current).unwrap())
        .unwrap()
        .by_name("preset.json")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let mut old: Value = serde_json::from_slice(&bytes).unwrap();
    old["document_preset_format_version"] = json!(1);
    old["document_schema_version"] = json!(7);
    let config = old["configuration"].as_object_mut().unwrap();
    for field in [
        "response_settings",
        "channel_response_overrides",
        "appearance_baseline",
        "saved_calibrations",
    ] {
        config.remove(field);
    }
    let base_id = config["pattern_settings"]["definition_id"]
        .as_u64()
        .unwrap();
    let channels = config["channel_configuration"]["channels"]
        .as_array()
        .unwrap();
    let definitions = channels
        .iter()
        .map(|channel| {
            (
                channel["id"].as_u64().unwrap(),
                channel["pattern_instance"]["definition_override"]
                    .as_u64()
                    .unwrap(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    for bundle in config["pattern_definition_bundles"].as_array_mut().unwrap() {
        let id = bundle["definition"]["id"].as_u64().unwrap();
        for setting in bundle["output_settings"].as_array_mut().unwrap() {
            setting["response"] = if id == base_id {
                json!({"kind":"marks","response":{"minimum_fill":0.2,"maximum_fill":0.8}})
            } else if id == definitions[&1] {
                json!({"kind":"marks","response":{"minimum_fill":0.3,"maximum_fill":0.9}})
            } else if id == definitions[&2] {
                json!({"kind":"connected","response":{"minimum_thickness":0.1,"maximum_thickness":0.9,"bias":0.15}})
            } else {
                json!({"kind":"regions","response":{"algorithm":"scale","sampling":"reference_point","minimum_fill":0.3,"maximum_fill":1.1}})
            };
        }
        for mechanism in bundle["definition"]["mechanisms"].as_array_mut().unwrap() {
            if mechanism["kind"] == "random_site_process" {
                mechanism.as_object_mut().unwrap().remove("nominal_spacing");
            }
            if mechanism["kind"] == "random_site_product" {
                mechanism.as_object_mut().unwrap().remove("refinement");
            }
        }
    }
    let output_ids = config["pattern_definition_bundles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|bundle| {
            (
                bundle["definition"]["id"].as_u64().unwrap(),
                bundle["output_settings"][0]["output_layer_id"]
                    .as_u64()
                    .unwrap(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    for channel in config["channel_configuration"]["channels"]
        .as_array_mut()
        .unwrap()
    {
        let id = channel["id"].as_u64().unwrap();
        channel.as_object_mut().unwrap().remove("weighting");
        channel["mapping"].as_object_mut().unwrap().remove("tone");
        let output_id = output_ids[&definitions[&id]];
        let delta = match id {
            1 => json!({"kind":"marks","delta":{"maximum_fill_delta":0.05}}),
            2 => {
                json!({"kind":"connected","delta":{"minimum_thickness_delta":0.02,"bias_delta":0.03}})
            }
            _ => json!({"kind":"regions","delta":{"maximum_fill_delta":0.04}}),
        };
        channel["pattern_instance"]["output_response_deltas"] =
            json!([{"output_layer_id":output_id,"delta":delta}]);
    }
    let legacy = directory.join("synthetic-v1-schema7.toniator-preset");
    let mut writer = ZipWriter::new(File::create(&legacy).unwrap());
    writer
        .start_file("preset.json", SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(&serde_json::to_vec(&old).unwrap())
        .unwrap();
    writer.finish().unwrap();
    let migrated = load_document_preset(&legacy, &destination)
        .unwrap()
        .bind(&destination)
        .unwrap();
    assert_eq!(migrated.response_settings().marks.minimum_fill, 0.2);
    assert_eq!(migrated.response_settings().marks.maximum_fill, 0.8);
    let expected = [(1, 0.3, 0.95), (2, 0.12, 0.9), (3, 0.3, 1.14)];
    for (id, minimum, maximum) in expected {
        let response = &migrated
            .effective_channel_pattern(ChannelId(id))
            .unwrap()
            .output_settings[0]
            .response;
        let actual = match response {
            PatternGeometryResponse::Marks(value) => (value.minimum_fill, value.maximum_fill),
            PatternGeometryResponse::Connected(value) => {
                (value.minimum_thickness, value.maximum_thickness)
            }
            PatternGeometryResponse::Regions(value) => (value.minimum_fill, value.maximum_fill),
        };
        assert!((actual.0 - minimum).abs() < 1e-12);
        assert!((actual.1 - maximum).abs() < 1e-12);
    }
    let connected = &migrated
        .effective_channel_pattern(ChannelId(2))
        .unwrap()
        .output_settings[0]
        .response;
    assert!(
        matches!(connected, PatternGeometryResponse::Connected(value)
        if (value.bias - 0.18).abs() < 1e-12)
    );
    let regions = &migrated
        .effective_channel_pattern(ChannelId(3))
        .unwrap()
        .output_settings[0]
        .response;
    assert!(matches!(regions, PatternGeometryResponse::Regions(value)
        if value.sampling == toniator_domain::RegionSamplingStrategy::ReferencePoint));
    let random = migrated.pattern_definition_for(ChannelId(1)).unwrap();
    assert!(random.mechanisms.iter().any(|mechanism| matches!(mechanism,
        toniator_domain::PatternMechanism::RandomSiteProcess { nominal_spacing: Some(value), .. }
        if *value == DEFAULT_RANDOM_NOMINAL_SPACING)));
    assert!(random.mechanisms.iter().any(|mechanism| matches!(mechanism,
        toniator_domain::PatternMechanism::RandomSiteProduct { refinement, .. }
        if !refinement.enabled && !refinement.density_weighted && refinement.iterations == 4)));
    let future = directory.join("synthetic-future.toniator-preset");
    old["document_schema_version"] = json!(99);
    let mut writer = ZipWriter::new(File::create(&future).unwrap());
    writer
        .start_file("preset.json", SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(&serde_json::to_vec(&old).unwrap())
        .unwrap();
    writer.finish().unwrap();
    assert!(load_document_preset(&future, &destination).is_err());
}
