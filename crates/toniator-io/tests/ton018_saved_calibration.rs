use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};
use toniator_domain::{
    AttachedCalibration, CalibrationBinding, CalibrationKnot, CalibrationVariation, CanvasSpec, ChannelId, Document,
    DocumentCommand, DocumentConfiguration, PresetMetadata, PresetRecord, PropertyFieldId,
    PropertyTarget, SavedCalibration, SourceReference, SourceReferenceId,
    CALIBRATION_PROFILE_GUIDE_FACES, CALIBRATION_PROFILE_ROUND_INTERIOR,
    CALIBRATION_PROFILE_SEEDED, Easing, FrameRange, FrameRate, ProjectTiming,
    TemporalEndpointEdit,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, capture_document_preset_destination, load,
    load_document_preset, load_document_preset_staged, load_preset, save, save_document_preset,
    save_preset, StagedDocumentPreset,
};
use toniator_patterns::PresetRegistry;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Locates validation-only current-format artifacts without touching personal assets.
///
/// # Panics
/// Panics if the permitted target validation directory cannot be created.
fn output_directory() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018-saved-calibration-20261011/io");
    fs::create_dir_all(&path).unwrap();
    path
}

/// Drops only a superseded Guide Faces numeric companion while retaining current desired
/// Start/End response intent, untouched Preset bytes, and strict ordinary project loading.
///
/// # Panics
/// Panics if current GF32 profile fails to round-trip, old generic v4 survives a source-free
/// Preset, desired endpoints are remapped, or an ordinary project accepts the obsolete profile.
#[test]
fn guide_faces_v2_profile_selectively_converts_optional_companions() {
    let directory = output_directory();
    let record = PresetRegistry::bundled().entries().iter()
        .find(|entry| entry.metadata.id == "three-guide-cells-scale").unwrap().clone();
    let artifact = record.saved_calibration.clone().unwrap();
    assert_eq!(artifact.profile(), CALIBRATION_PROFILE_GUIDE_FACES);
    let source_id = SourceReferenceId::new("guide-faces-profile-v2").unwrap();
    let document = Document::new_default_document(
        CanvasSpec { width: 1024.0, height: 1024.0 },
        SourceReference::Assigned(source_id.clone()),
    ).unwrap();
    let mut document = document.apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: document.pattern_settings().clone(),
        base_definition: document.pattern_definition_bundles()[0].definition.clone(),
        recipe: record.recipe.clone(),
    }).unwrap().0;
    let base_response = document.response_settings().clone();
    let mut response = base_response.clone();
    response.regions_minimum_fill = 0.19;
    response.regions_maximum_fill = 0.71;
    document = document.apply_command(&DocumentCommand::SetAllResponseSettings {
        base: base_response, settings: response,
    }).unwrap().0;
    let initialized = document.initialize_end_command().unwrap();
    document = document.with_temporal_authority(
        ProjectTiming::new(FrameRate::new(24, 1).unwrap(), FrameRange::new(0, 2).unwrap()),
        initialized.replacement().end_overrides.clone(),
    ).unwrap();
    let output_id = document.effective_channel_pattern(ChannelId(1)).unwrap()
        .output_settings[0].output_layer_id;
    let end = document.edit_effective_end_command(&[TemporalEndpointEdit {
        target: PropertyTarget::ChannelOutput(ChannelId(1), output_id),
        field: PropertyFieldId::RegionMaximumFill,
        effective_end: 0.83,
        easing: Easing::SmoothStep,
    }]).unwrap();
    document = document.with_temporal_authority(
        end.replacement().project_timing.clone(), end.replacement().end_overrides.clone(),
    ).unwrap();
    let attached = [1, 2, 3].into_iter().map(|id| AttachedCalibration {
        channel_id: ChannelId(id), artifact: artifact.clone(),
    }).collect();
    document = document.with_saved_calibration_authority(attached, Some(artifact.clone())).unwrap();
    document.require_saved_calibration_authority().unwrap();
    let source = EmbeddedSource::new(source_id, EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1024\" height=\"1024\"/>".to_vec(),
        Some("gf-v2.svg".into())).unwrap();
    let project = directory.join("gf-v2-current.toniator");
    save(&project, &document, &SourceBundle::new([source]).unwrap()).unwrap();
    assert_eq!(load(&project).unwrap().document(), &document);

    let current = directory.join("gf-v2-current.toniator-preset");
    save_document_preset(&current, &DocumentConfiguration::capture(&document),
        &capture_document_preset_destination(&current).unwrap()).unwrap();
    let mut bytes = Vec::new();
    ZipArchive::new(fs::File::open(&current).unwrap()).unwrap()
        .by_name("preset.json").unwrap().read_to_end(&mut bytes).unwrap();
    let mut envelope: Value = serde_json::from_slice(&bytes).unwrap();
    for companion in envelope["configuration"]["saved_calibrations"].as_array_mut().unwrap() {
        companion["profile"] = Value::String("ton018-reference-fixed-canvas1024-spacings32-v4".into());
    }
    envelope["configuration"]["baseline_saved_calibration"]["profile"] =
        Value::String("ton018-reference-fixed-canvas1024-spacings32-v4".into());
    let former = directory.join("gf-v1-companion-config16.toniator-preset");
    let mut archive = ZipWriter::new(fs::File::create(&former).unwrap());
    archive.start_file("preset.json", SimpleFileOptions::default()).unwrap();
    archive.write_all(&serde_json::to_vec(&envelope).unwrap()).unwrap();
    archive.finish().unwrap();
    let before = fs::read(&former).unwrap();
    let StagedDocumentPreset::Current(configuration) =
        load_document_preset_staged(&former, &document).unwrap() else {
            panic!("configuration16 is already absolute desired coverage");
        };
    let rebound = configuration.bind(&document).unwrap();
    assert_eq!(rebound.response_settings(), document.response_settings());
    assert_eq!(rebound.temporal_end_overrides(), document.temporal_end_overrides());
    assert!(rebound.saved_calibrations().is_empty());
    assert!(rebound.baseline_saved_calibration().unwrap().is_none());
    assert!(rebound.require_saved_calibration_authority().is_err());
    assert_eq!(fs::read(&former).unwrap(), before);

    let pattern = directory.join("gf-v2-current.toniator-pattern");
    save_preset(&pattern, &record).unwrap();
    let mut value: Value = serde_json::from_slice(&fs::read(&pattern).unwrap()).unwrap();
    value["saved_calibration"]["profile"] =
        Value::String("ton018-reference-fixed-canvas1024-spacings32-v4".into());
    let former_pattern = directory.join("gf-v1-companion-pattern8.toniator-pattern");
    fs::write(&former_pattern, serde_json::to_vec(&value).unwrap()).unwrap();
    let before = fs::read(&former_pattern).unwrap();
    let restored = load_preset(&former_pattern).unwrap();
    assert_eq!(restored.recipe, record.recipe);
    assert!(restored.saved_calibration.is_none());
    assert_eq!(fs::read(&former_pattern).unwrap(), before);

    let mut project_archive = ZipArchive::new(fs::File::open(&project).unwrap()).unwrap();
    let mut project_json = Vec::new();
    project_archive.by_name("document.json").unwrap()
        .read_to_end(&mut project_json).unwrap();
    let mut project_value: Value = serde_json::from_slice(&project_json).unwrap();
    for companion in project_value["document"]["saved_calibrations"].as_array_mut().unwrap() {
        companion["profile"] = Value::String("ton018-reference-fixed-canvas1024-spacings32-v4".into());
    }
    project_value["document"]["baseline_saved_calibration"]["profile"] =
        Value::String("ton018-reference-fixed-canvas1024-spacings32-v4".into());
    let source_bytes = { let mut bytes = Vec::new(); project_archive.by_name("sources/000000.svg").unwrap().read_to_end(&mut bytes).unwrap(); bytes };
    let former_project = directory.join("gf-v1-companion-project17.toniator");
    let mut archive = ZipWriter::new(fs::File::create(&former_project).unwrap());
    archive.start_file("document.json", SimpleFileOptions::default()).unwrap();
    archive.write_all(&serde_json::to_vec(&project_value).unwrap()).unwrap();
    archive.start_file("sources/000000.svg", SimpleFileOptions::default()).unwrap();
    archive.write_all(&source_bytes).unwrap();
    archive.finish().unwrap();
    assert!(load(&former_project).is_err());
}

/// Round-trips one current covered-interior companion through all three persistence containers
/// using the exact finite receiver binding, without invoking any calibration service on load.
///
/// # Panics
/// Panics if the profile, typed binding, raw numbers, or complete document authority changes.
#[test]
fn finite_round_interior_companion_round_trips_current_containers() {
    let directory = output_directory();
    let registry = PresetRegistry::bundled();
    let record = registry.entries().iter()
        .find(|entry| entry.metadata.id == "round-spiral-line").unwrap();
    let source_id = SourceReferenceId::new("round-interior-io").unwrap();
    let document = Document::new_default_document(
        CanvasSpec { width: 900.0, height: 620.0 },
        SourceReference::Assigned(source_id.clone()),
    ).unwrap();
    let document = document.apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: document.pattern_settings().clone(),
        base_definition: document.pattern_definition_bundles()[0].definition.clone(),
        recipe: record.recipe.clone(),
    }).unwrap().0;
    let binding = CalibrationBinding::for_channel(&document, ChannelId(1)).unwrap();
    assert!(binding.supports_round_interior());
    let artifact = SavedCalibration::with_profile(
        CALIBRATION_PROFILE_ROUND_INTERIOR.to_owned(), binding.clone(),
        vec![
            CalibrationKnot { input: 0.0, coverage: 0.0 },
            CalibrationKnot { input: 0.5, coverage: 0.2 },
            CalibrationKnot { input: 2.0, coverage: 1.0 },
        ], None,
    ).unwrap();
    let attached = [1, 2, 3].into_iter().map(|id| AttachedCalibration {
        channel_id: ChannelId(id), artifact: artifact.clone(),
    }).collect();
    let document = document.with_saved_calibration_authority(attached, Some(artifact.clone())).unwrap();
    document.require_saved_calibration_authority().unwrap();
    let source = EmbeddedSource::new(source_id, EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"900\" height=\"620\"/>".to_vec(),
        Some("round-interior.svg".into())).unwrap();
    let project = directory.join("round-interior-current.toniator");
    save(&project, &document, &SourceBundle::new([source]).unwrap()).unwrap();
    let reopened = load(&project).unwrap();
    assert_eq!(reopened.document().baseline_saved_calibration().unwrap(), Some(&artifact));
    reopened.document().require_saved_calibration_authority().unwrap();

    let preset = directory.join("round-interior-current.toniator-preset");
    save_document_preset(&preset, &DocumentConfiguration::capture(&document),
        &capture_document_preset_destination(&preset).unwrap()).unwrap();
    let rebound = load_document_preset(&preset, &document).unwrap().bind(&document).unwrap();
    assert_eq!(rebound.baseline_saved_calibration().unwrap(), Some(&artifact));
    rebound.require_saved_calibration_authority().unwrap();

    let pattern = PresetRecord {
        metadata: PresetMetadata {
            id: "round-interior-io".into(), name: "Round interior IO".into(),
            category: "Parametric".into(), description: "Finite round reference".into(),
            thumbnail: None,
        },
        recipe: document.reconstruct_pattern_definition_recipe(document.pattern_settings().definition_id).unwrap(),
        saved_calibration: Some(artifact.clone()),
    };
    let path = directory.join("round-interior-current.toniator-pattern");
    save_preset(&path, &pattern).unwrap();
    assert_eq!(load_preset(&path).unwrap(), pattern);
}

/// Preserves current-format authored seed intent while dropping only a former v4 numeric cache.
///
/// # Panics
/// Panics if current Pattern, project, or Document Preset persistence changes exact v5 identity,
/// overwrites the original Pattern bytes, or accepts a former seed-specific companion.
#[test]
fn seeded_companion_roundtrips_and_former_v4_cache_is_discarded() {
    let directory = output_directory();
    let recipe = PresetRegistry::bundled().entries().iter()
        .find(|record| record.metadata.id == "clustered-dispersion-random-links")
        .unwrap().recipe.clone();
    let binding = CalibrationBinding::new(recipe.clone(), 1.0, 0.0).unwrap();
    assert!(binding.has_variation_seed());
    let artifact = SavedCalibration::new(binding, vec![
        CalibrationKnot { input: 0.0, coverage: 0.0 },
        CalibrationKnot { input: 2.0, coverage: 0.9 },
    ], None).unwrap();
    assert_eq!(artifact.profile(), CALIBRATION_PROFILE_SEEDED);
    assert!(SavedCalibration::with_profile(
        "ton018-reference-fixed-canvas1024-spacings32-v4".into(),
        artifact.binding().clone(), artifact.raw().to_vec(), None,
    ).is_err());
    let record = PresetRecord {
        metadata: PresetMetadata {
            id: "clustered-seed-cache".into(), name: "Clustered seed cache".into(),
            category: "Connections".into(), description: "Seed-neutral companion".into(),
            thumbnail: None,
        },
        recipe: recipe.clone(), saved_calibration: Some(artifact.clone()),
    };
    let current = directory.join("seed-neutral-current.toniator-pattern");
    save_preset(&current, &record).unwrap();
    assert_eq!(load_preset(&current).unwrap(), record);
    let mut former: Value = serde_json::from_slice(&fs::read(&current).unwrap()).unwrap();
    former["saved_calibration"]["profile"] = Value::String(
        "ton018-reference-fixed-canvas1024-spacings32-v4".into());
    let former_path = directory.join("seeded-v4-current-format.toniator-pattern");
    fs::write(&former_path, serde_json::to_vec(&former).unwrap()).unwrap();
    let original_bytes = fs::read(&former_path).unwrap();
    let converted = load_preset(&former_path).unwrap();
    assert_eq!(converted.recipe, recipe);
    assert!(converted.saved_calibration.is_none());
    assert_eq!(fs::read(&former_path).unwrap(), original_bytes);
    former["saved_calibration"]["profile"] = Value::String("future-profile".into());
    let unknown = directory.join("seeded-unknown-profile.toniator-pattern");
    fs::write(&unknown, serde_json::to_vec(&former).unwrap()).unwrap();
    assert!(load_preset(&unknown).is_err());

    let source_id = SourceReferenceId::new("seed-neutral-source").unwrap();
    let document = Document::new_default_document(
        CanvasSpec { width: 1024.0, height: 1024.0 },
        SourceReference::Assigned(source_id.clone()),
    ).unwrap();
    let document = document.apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: document.pattern_settings().clone(),
        base_definition: document.pattern_definition_bundles()[0].definition.clone(),
        recipe,
    }).unwrap().0.with_saved_calibration_authority(Vec::new(), Some(artifact.clone())).unwrap();
    let source = EmbeddedSource::new(source_id, EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1024\" height=\"1024\"/>".to_vec(),
        Some("seed-neutral.svg".into())).unwrap();
    let sources = SourceBundle::new([source]).unwrap();
    let project = directory.join("seed-neutral-current.toniator");
    save(&project, &document, &sources).unwrap();
    assert_eq!(load(&project).unwrap().document().baseline_saved_calibration().unwrap(), Some(&artifact));
    let preset = directory.join("seed-neutral-current.toniator-preset");
    save_document_preset(&preset, &DocumentConfiguration::capture(&document),
        &capture_document_preset_destination(&preset).unwrap()).unwrap();
    let restored = load_document_preset(&preset, &document).unwrap().bind(&document).unwrap();
    assert_eq!(restored.baseline_saved_calibration().unwrap(), Some(&artifact));
    let mut bytes = Vec::new();
    ZipArchive::new(fs::File::open(&preset).unwrap()).unwrap()
        .by_name("preset.json").unwrap().read_to_end(&mut bytes).unwrap();
    let mut envelope: Value = serde_json::from_slice(&bytes).unwrap();
    envelope["configuration"]["baseline_saved_calibration"]["profile"] =
        Value::String("ton018-reference-fixed-canvas1024-spacings32-v4".into());
    let former_preset = directory.join("seeded-v4-current-format.toniator-preset");
    let mut archive = ZipWriter::new(fs::File::create(&former_preset).unwrap());
    archive.start_file("preset.json", SimpleFileOptions::default()).unwrap();
    archive.write_all(&serde_json::to_vec(&envelope).unwrap()).unwrap();
    archive.finish().unwrap();
    let converted = load_document_preset(&former_preset, &document).unwrap()
        .bind(&document).unwrap();
    assert!(converted.baseline_saved_calibration().unwrap().is_none());
    assert_eq!(converted.pattern_settings(), document.pattern_settings());
}

/// Keeps the user's copied current-format Poisson/Lloyd Pattern available after v4 cache removal.
///
/// # Panics
/// Panics if a supplied read-only validation copy changes bytes or loses authored seed intent.
#[test]
fn actual_poisson_lloyd_pattern_keeps_recipe_after_cache_discard() {
    let Some(path) = std::env::var_os("TONIATOR_POISSON_LLOYD_PRESET_COPY") else { return; };
    let path = PathBuf::from(path);
    let bytes = fs::read(&path).unwrap();
    let authored: Value = serde_json::from_slice(&bytes).unwrap();
    let loaded = load_preset(&path).unwrap();
    assert_eq!(loaded.metadata.name, "PoissonDiscLloydConnected");
    assert!(loaded.saved_calibration.is_none());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let new_record = PresetRecord { saved_calibration: None, ..loaded };
    let dest = output_directory().join("poisson-lloyd-current-reissued.preset.json");
    save_preset(&dest, &new_record).unwrap();
    let rebuilt: Value = serde_json::from_slice(&fs::read(&dest).unwrap()).unwrap();
    assert_eq!(rebuilt["recipe"], authored["recipe"]);
    if let Some(path) = std::env::var_os("TONIATOR_POISSON_LLOYD_V5_COPY") {
        let validated = load_preset(&PathBuf::from(path)).unwrap();
        assert_eq!(validated.recipe, new_record.recipe);
        assert_eq!(validated.saved_calibration.as_ref().unwrap().profile(),
            toniator_domain::CALIBRATION_PROFILE_MEAN);
    }
}

/// Verifies all three current persistence containers keep the exact numeric profile and binding.
///
/// # Panics
/// Panics if project, document Preset, or Pattern Preset restoration changes the measurement.
#[test]
fn saved_artifact_round_trips_project_and_both_presets() {
    let directory = output_directory();
    let source_id = SourceReferenceId::new("ton018-artifact-source").unwrap();
    let document = Document::new_default_document(
        CanvasSpec {
            width: 64.0,
            height: 48.0,
        },
        SourceReference::Assigned(source_id.clone()),
    )
    .unwrap();
    let original = document
        .saved_calibration_for(ChannelId(1))
        .unwrap()
        .unwrap()
        .clone();
    let source = EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"64\" height=\"48\"/>".to_vec(),
        Some("calibration.svg".into()),
    )
    .unwrap();
    let project = directory.join("current-saved-calibration.toniator");
    let sources = SourceBundle::new([source]).unwrap();
    save(&project, &document, &sources).unwrap();
    let loaded = load(&project).unwrap();
    assert_eq!(
        loaded
            .document()
            .saved_calibration_for(ChannelId(1))
            .unwrap(),
        Some(&original)
    );

    let preset = directory.join("current-saved-calibration.toniator-preset");
    save_document_preset(
        &preset,
        &DocumentConfiguration::capture(&document),
        &capture_document_preset_destination(&preset).unwrap(),
    )
    .unwrap();
    let restored = load_document_preset(&preset, &document)
        .unwrap()
        .bind(&document)
        .unwrap();
    assert_eq!(
        restored.saved_calibration_for(ChannelId(1)).unwrap(),
        Some(&original)
    );

    let recipe = document
        .reconstruct_pattern_definition_recipe(document.pattern_settings().definition_id)
        .unwrap();
    let record = PresetRecord {
        metadata: PresetMetadata {
            id: "saved-straight".into(),
            name: "Saved straight".into(),
            category: "Marks".into(),
            description: "Current artifact".into(),
            thumbnail: None,
        },
        saved_calibration: Some(SavedCalibration::built_in_straight_recipe(&recipe).unwrap()),
        recipe,
    };
    let standalone = directory.join("current-saved-calibration.preset.json");
    save_preset(&standalone, &record).unwrap();
    assert_eq!(load_preset(&standalone).unwrap(), record);

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
    assert!(changed.saved_calibrations().is_empty());
    let edited_project = directory.join("current-structural-edit.toniator");
    save(&edited_project, &changed, &sources).unwrap();
    let reopened = load(&edited_project).unwrap();
    assert!(reopened.document().saved_calibrations().is_empty());

    let mut distinct = document.clone();
    for (channel, aspect) in [
        (ChannelId(1), 1.1),
        (ChannelId(2), 1.3),
        (ChannelId(3), 1.6),
    ] {
        let configuration = distinct
            .edit_channel_start_configuration(&[(
                PropertyTarget::Channel(channel),
                PropertyFieldId::DensityAspect,
                aspect,
            )])
            .unwrap();
        distinct = configuration.bind(&distinct).unwrap();
        let binding = CalibrationBinding::for_channel(&distinct, channel).unwrap();
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
        distinct = distinct.with_saved_calibration(channel, artifact).unwrap();
    }
    assert_eq!(
        distinct.baseline_saved_calibration().unwrap(),
        Some(&original)
    );
    let distinct_project = directory.join("hidden-all-distinct-channels.toniator");
    save(&distinct_project, &distinct, &sources).unwrap();
    let reopened = load(&distinct_project).unwrap();
    assert_eq!(
        reopened.document().baseline_saved_calibration().unwrap(),
        Some(&original)
    );
    assert_eq!(
        reopened.document().saved_calibrations(),
        distinct.saved_calibrations()
    );
    let distinct_preset = directory.join("hidden-all-distinct-channels.toniator-preset");
    save_document_preset(
        &distinct_preset,
        &DocumentConfiguration::capture(&distinct),
        &capture_document_preset_destination(&distinct_preset).unwrap(),
    )
    .unwrap();
    let rebound = load_document_preset(&distinct_preset, &document)
        .unwrap()
        .bind(&document)
        .unwrap();
    assert_eq!(
        rebound.baseline_saved_calibration().unwrap(),
        Some(&original)
    );
    assert_eq!(rebound.saved_calibrations(), distinct.saved_calibrations());
}

/// Converts recognized v2 and v3 source-free Preset profiles while preserving authored settings.
///
/// # Panics
/// Panics if an old numeric artifact blocks conversion, survives as reusable data, or alters intent.
#[test]
fn former_current_document_preset_profile_becomes_explicitly_uncalibrated() {
    let directory = output_directory();
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let current = directory.join("former-profile-source.toniator-preset");
    save_document_preset(
        &current,
        &DocumentConfiguration::capture(&document),
        &capture_document_preset_destination(&current).unwrap(),
    )
    .unwrap();
    let mut bytes = Vec::new();
    ZipArchive::new(fs::File::open(&current).unwrap())
        .unwrap()
        .by_name("preset.json")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let mut envelope: Value = serde_json::from_slice(&bytes).unwrap();
    for (suffix, profile) in [
        ("v2", "ton018-reference-fixed-canvas1024-v2"),
        ("v3-32", "ton018-reference-fixed-canvas1024-spacings32-v3"),
        ("v3-64", "ton018-reference-fixed-canvas1024-spacings64-v3"),
    ] {
        let artifacts = envelope["configuration"]["saved_calibrations"]
            .as_array_mut()
            .unwrap();
        assert!(!artifacts.is_empty());
        for artifact in artifacts {
            artifact["profile"] = Value::String(profile.into());
        }
        envelope["configuration"]["baseline_saved_calibration"]["profile"] =
            Value::String(profile.into());
        let former = directory.join(format!("former-profile-{suffix}-converted.toniator-preset"));
        let mut archive = ZipWriter::new(fs::File::create(&former).unwrap());
        archive
            .start_file("preset.json", SimpleFileOptions::default())
            .unwrap();
        archive
            .write_all(&serde_json::to_vec(&envelope).unwrap())
            .unwrap();
        archive.finish().unwrap();
        let converted = load_document_preset(&former, &document)
            .unwrap()
            .bind(&document)
            .unwrap();
        assert_eq!(converted.pattern_settings(), document.pattern_settings());
        assert_eq!(converted.response_settings(), document.response_settings());
        assert_eq!(
            converted.channel_response_overrides(),
            document.channel_response_overrides()
        );
        assert!(converted.saved_calibrations().is_empty());
        assert!(converted.baseline_saved_calibration().unwrap().is_none());
    }

    for artifact in envelope["configuration"]["saved_calibrations"]
        .as_array_mut()
        .unwrap()
    {
        artifact["profile"] = Value::String("future-reference-profile".into());
    }
    let future = directory.join("future-profile-rejected.toniator-preset");
    let mut archive = ZipWriter::new(fs::File::create(&future).unwrap());
    archive
        .start_file("preset.json", SimpleFileOptions::default())
        .unwrap();
    archive
        .write_all(&serde_json::to_vec(&envelope).unwrap())
        .unwrap();
    archive.finish().unwrap();
    assert!(load_document_preset(&future, &document).is_err());
}

/// Round-trips a typed mean profile and rejects incomplete empirical evidence in a Pattern file.
///
/// # Panics
/// Panics if current preset serialization changes exact variation values or accepts missing data.
#[test]
fn stochastic_mean_variation_round_trips_and_is_required() {
    let directory = output_directory();
    let preset = PresetRegistry::bundled()
        .entries()
        .iter()
        .find(|entry| entry.metadata.id == "clustered-dispersion-random-links")
        .unwrap()
        .clone();
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let base = document.pattern_settings().clone();
    let base_definition = document.pattern_definition_bundles()[0].definition.clone();
    let document = document
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: preset.recipe.clone(),
        })
        .unwrap()
        .0;
    let binding = CalibrationBinding::for_channel(&document, ChannelId(1)).unwrap();
    let variation = CalibrationVariation::new(0.01, 0.01, 0.005, 0.02, None, None).unwrap();
    let artifact = SavedCalibration::with_mean_profile(
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
        variation,
    )
    .unwrap();
    let mut record = preset;
    record.saved_calibration = Some(artifact.clone());
    let path = directory.join("synthetic-clustered-mean.toniator-pattern");
    save_preset(&path, &record).unwrap();
    assert_eq!(load_preset(&path).unwrap(), record);

    let mut json: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    json["saved_calibration"]["variation"] = Value::Null;
    let missing = directory.join("synthetic-clustered-mean-missing-variation.toniator-pattern");
    fs::write(&missing, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(load_preset(&missing).is_err());
    if let Some(path) = std::env::var_os("TONIATOR_CLUSTERED_COMPANION_CHECK") {
        let measured = load_preset(&PathBuf::from(path)).unwrap();
        assert_eq!(measured.metadata.id, "clustered-dispersion-random-links");
        assert_eq!(
            measured.saved_calibration.as_ref().unwrap().profile(),
            toniator_domain::CALIBRATION_PROFILE_MEAN
        );
        assert!(
            measured
                .saved_calibration
                .as_ref()
                .unwrap()
                .variation()
                .is_some()
        );
    }
}
