//! Current-only projects and explicit source-free conversion preserve coverage intent.
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use toniator_domain::*;
use toniator_io::*;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Returns owned Stage A evidence paths without touching prior diagnosis or historical outputs.
/// # Panics
/// Panics if the local validation directory cannot be created.
fn output(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/offset-coverage-20261019/implementation/stage-a/tests");
    fs::create_dir_all(&root).unwrap();
    root.join(name)
}

/// Creates exact authored extent with a nondefault artistic cadence, without claiming a measured fit.
/// # Panics
/// Panics if valid recipe or output controls cannot be applied through ordinary authority.
fn document(policy: SpiralCoveragePolicy) -> Document {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 768.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("coverage-fixture").unwrap()),
    )
    .unwrap();
    let recipe = PatternDefinitionRecipe::connected(PatternStructureRecipe::ParametricCurve {
        name: "Coverage persistence fixture".into(),
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
    });
    let document = base
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: base.pattern_settings().clone(),
            base_definition: base.pattern_definition_bundles()[0].definition.clone(),
            recipe,
        })
        .unwrap()
        .0;
    let definition = document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.id == document.pattern_settings().definition_id)
        .unwrap()
        .definition
        .clone();
    document
        .apply_command(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: definition.id,
            base_definition: definition.clone(),
            edit: PatternDefinitionEdit::SetToneSampleSpacing {
                output_layer_id: definition.output_layers[0].id,
                value: ToneSampleSpacing { multiplier: 2.75 },
            },
        })
        .unwrap()
        .0
}

/// Reads one archive JSON entry without rewriting its original bytes.
/// # Panics
/// Panics if an owned bounded archive or its JSON entry is malformed.
fn envelope(path: &Path, entry: &str) -> Value {
    let mut archive = ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut bytes = Vec::new();
    archive
        .by_name(entry)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Writes a source-free derived envelope to exercise explicit version dispatch without user files.
/// # Panics
/// Panics if the owned fixture cannot be serialized into a bounded ZIP archive.
fn write_envelope(path: &Path, entry: &str, value: &Value) {
    let mut writer = ZipWriter::new(fs::File::create(path).unwrap());
    writer
        .start_file(entry, SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(&serde_json::to_vec(value).unwrap())
        .unwrap();
    if entry == "document.json" {
        writer
            .add_directory("sources/", SimpleFileOptions::default())
            .unwrap();
    }
    writer.finish().unwrap();
}

/// Captures current source-free intent through its public archive writer.
/// # Panics
/// Panics if current authoritative configuration cannot be saved or decoded.
fn preset(document: &Document, name: &str) -> (PathBuf, Value) {
    let path = output(name);
    save_document_preset(
        &path,
        &DocumentConfiguration::capture(document),
        &capture_document_preset_destination(&path).unwrap(),
    )
    .unwrap();
    let value = envelope(&path, "preset.json");
    (path, value)
}

/// Preserves explicit mechanism policy in current projects and rejects ordinary project18 unchanged.
/// # Panics
/// Panics if current fields disappear or an obsolete project enters a compatibility path.
#[test]
fn current_project_roundtrip_requires_explicit_policy_and_rejects_project18() {
    assert_eq!(
        (
            DOCUMENT_SCHEMA_VERSION,
            DOCUMENT_PRESET_CONFIGURATION_SCHEMA_VERSION,
            PRESET_FORMAT_VERSION
        ),
        (19, 18, 10)
    );
    let sources = SourceBundle::new([EmbeddedSource::new(
        SourceReferenceId::new("coverage-fixture").unwrap(),
        EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\"/>".to_vec(),
        None,
    )
    .unwrap()])
    .unwrap();
    for (name, policy, expected) in [
        ("fixed", SpiralCoveragePolicy::Fixed, "fixed"),
        ("cover", SpiralCoveragePolicy::CoverCanvas, "cover_canvas"),
    ] {
        let document = document(policy);
        let path = output(&format!("{name}-current.toniator"));
        save(&path, &document, &sources).unwrap();
        assert_eq!(load(&path).unwrap().document(), &document);
        let value = envelope(&path, "document.json");
        let bundles = value["document"]["pattern_definition_bundles"]
            .as_array()
            .unwrap();
        let selected = bundles
            .iter()
            .find(|bundle| {
                bundle["definition"]["id"] == value["document"]["pattern_settings"]["definition_id"]
            })
            .unwrap();
        assert_eq!(
            selected["definition"]["mechanisms"][0]["spiral_coverage"],
            expected
        );
        assert_eq!(
            selected["definition"]["mechanisms"][0]["curve"]["turns"],
            5.0
        );
    }
    let path = output("ordinary-project18.toniator");
    write_envelope(
        &path,
        "document.json",
        &json!({"container_version":2,"document_schema_version":18,"document":{}}),
    );
    let error = load(&path).unwrap_err();
    assert!(
        matches!(&error, LoadError::Version { context } if context.contains("18")),
        "{error:?}"
    );
}

/// Roundtrips generic CoverCanvas intent without centered finite conversion or old Pattern9 acceptance.
/// # Panics
/// Panics if current recipe policy/turns change or an obsolete standalone format is accepted.
#[test]
fn standalone_pattern10_retains_generic_cover_recipe_and_rejects_pattern9() {
    let document = document(SpiralCoveragePolicy::CoverCanvas);
    let recipe = document
        .reconstruct_pattern_definition_recipe(document.pattern_settings().definition_id)
        .unwrap();
    let record = PresetRecord {
        metadata: PresetMetadata {
            id: "coverage-fixture".into(),
            name: "Coverage fixture".into(),
            category: "Validation".into(),
            description: "Persistent authored extent".into(),
            thumbnail: None,
        },
        recipe,
        saved_calibration: None,
    };
    let path = output("cover.pattern.json");
    save_preset(&path, &record).unwrap();
    assert_eq!(load_preset(&path).unwrap(), record);
    let mut former: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert!(
        serde_json::to_string(&former)
            .unwrap()
            .contains("\"spiral_coverage\":\"cover_canvas\"")
    );
    former["preset_format_version"] = json!(9);
    let old = output("obsolete-pattern9.json");
    fs::write(&old, serde_json::to_vec(&former).unwrap()).unwrap();
    assert!(load_preset(&old).is_err());
}

/// Converts only lost former mechanism intent to Fixed while preserving artistic controls and bytes.
/// # Panics
/// Panics if config17 guesses cover intent, resets nondefault cadence, mutates bytes or converts nondeterministically.
#[test]
fn source_free_config17_converts_finite_policy_without_resetting_artistic_intent() {
    let original = document(SpiralCoveragePolicy::Fixed);
    let (_, mut former) = preset(&original, "fixed-current.toniator-preset");
    former["document_schema_version"] = json!(17);
    for bundle in former["configuration"]["pattern_definition_bundles"]
        .as_array_mut()
        .unwrap()
    {
        for mechanism in bundle["definition"]["mechanisms"].as_array_mut().unwrap() {
            if mechanism["kind"] == "parametric_curve_source" {
                mechanism.as_object_mut().unwrap().remove("spiral_coverage");
            }
        }
    }
    let path = output("former-config17.toniator-preset");
    write_envelope(&path, "preset.json", &former);
    let before = fs::read(&path).unwrap();
    let staged = load_document_preset_staged(&path, &original).unwrap();
    let configuration = match staged {
        StagedDocumentPreset::Current(value) | StagedDocumentPreset::PendingCalibration(value) => {
            value
        }
        StagedDocumentPreset::PendingRaw(_) => panic!("desired coverage stays desired"),
    };
    let repeated = load_document_preset_staged(&path, &original).unwrap();
    assert_eq!(
        format!("{repeated:?}"),
        format!("{:?}", StagedDocumentPreset::Current(configuration.clone()))
    );
    let rebound = configuration.bind(&original).unwrap();
    assert_eq!(
        rebound.pattern_definition_bundles(),
        original.pattern_definition_bundles()
    );
    assert_eq!(rebound.response_settings(), original.response_settings());
    assert!(
        !CalibrationBinding::for_channel(&rebound, ChannelId(1))
            .unwrap()
            .requires_canvas_coverage()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    let (_, current) = preset(&rebound, "converted-config18.toniator-preset");
    assert_eq!(current["document_schema_version"], 18);
}

/// Keeps current source-free coverage policy and authored minimum unchanged through destination binding.
/// # Panics
/// Panics if a current CoverCanvas configuration loses policy or is marked raw during decoding.
#[test]
fn current_source_free_cover_policy_roundtrips_without_inference() {
    let original = document(SpiralCoveragePolicy::CoverCanvas);
    let (path, _) = preset(&original, "cover-current.toniator-preset");
    let StagedDocumentPreset::Current(configuration) =
        load_document_preset_staged(&path, &original).unwrap()
    else {
        panic!("current configuration")
    };
    let rebound = configuration.bind(&original).unwrap();
    assert_eq!(
        rebound.pattern_definition_bundles(),
        original.pattern_definition_bundles()
    );
    assert!(
        CalibrationBinding::for_channel(&rebound, ChannelId(1))
            .unwrap()
            .requires_canvas_coverage()
    );
    let (_, mut malformed) = preset(&original, "required-policy-current.toniator-preset");
    for bundle in malformed["configuration"]["pattern_definition_bundles"]
        .as_array_mut()
        .unwrap()
    {
        for mechanism in bundle["definition"]["mechanisms"].as_array_mut().unwrap() {
            if mechanism["kind"] == "parametric_curve_source" {
                mechanism.as_object_mut().unwrap().remove("spiral_coverage");
            }
        }
    }
    let missing = output("missing-current-policy.toniator-preset");
    write_envelope(&missing, "preset.json", &malformed);
    let error = load_document_preset_staged(&missing, &original).unwrap_err();
    assert!(error.to_string().contains("spiral_coverage"), "{error:?}");
}

/// Preserves serialized named and hidden ALL calibration payloads under combined X/Y/rotation edits.
/// Metadata-only fixture knots test authority and byte reuse; no calibration solver or fit work runs.
/// # Panics
/// Panics if placement alters response endpoints, binding/profile/raw bytes or accepted saved authority.
#[test]
fn combined_placement_reuses_exact_saved_artifacts_and_response_bytes() {
    let mut original = document(SpiralCoveragePolicy::CoverCanvas);
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
        let fit = SavedCalibration::new(
            CalibrationBinding::for_channel(&original, channel_id).unwrap(),
            raw.clone(),
            None,
        )
        .unwrap();
        original = original.with_saved_calibration(channel_id, fit).unwrap();
    }
    let baseline =
        SavedCalibration::new(original.baseline_calibration_binding().unwrap(), raw, None).unwrap();
    original = original.with_saved_baseline_calibration(baseline).unwrap();
    original.require_saved_calibration_authority().unwrap();
    let mut moved = original.clone();
    for (channel_id, x, y, rotation_degrees) in [
        (ChannelId(1), -4096.0, 3072.0, 37.0),
        (ChannelId(2), 128.0, -256.0, -29.0),
        (ChannelId(3), -512.0, 1536.0, 91.0),
    ] {
        for (edited_axis, value) in [(TranslationEditedAxis::X, x), (TranslationEditedAxis::Y, y)] {
            moved = moved
                .apply_command(&DocumentCommand::SetTranslationAxis {
                    channel_id,
                    edited_axis,
                    value,
                })
                .unwrap()
                .0;
        }
        moved = moved
            .apply_command(&DocumentCommand::SetChannelPatternRotationDelta {
                base: moved.pattern_settings().clone(),
                channel_id,
                rotation_degrees,
            })
            .unwrap()
            .0;
    }
    moved.require_saved_calibration_authority().unwrap();
    assert_eq!(moved.saved_calibrations(), original.saved_calibrations());
    assert_eq!(
        moved.baseline_saved_calibration().unwrap(),
        original.baseline_saved_calibration().unwrap()
    );
    let (_, before) = preset(&original, "calibration-before-placement.toniator-preset");
    let (_, after) = preset(&moved, "calibration-after-placement.toniator-preset");
    assert!(!before["configuration"]["baseline_saved_calibration"].is_null());
    for field in [
        "saved_calibrations",
        "baseline_saved_calibration",
        "response_settings",
        "channel_response_overrides",
        "end_overrides",
    ] {
        assert_eq!(
            serde_json::to_vec(&before["configuration"][field]).unwrap(),
            serde_json::to_vec(&after["configuration"][field]).unwrap(),
            "placement changed {field}"
        );
    }
}

/// Returns fresh Stage C conversion evidence paths without touching immutable Stage A outputs.
/// # Panics
/// Panics if the bounded local evidence directory cannot be created.
fn stage_c_output(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/offset-coverage-20261019/implementation/stage-c/io-tests");
    fs::create_dir_all(&root).unwrap();
    root.join(name)
}

/// Converts supported config17 with genuinely measured Fixed artifacts and nonempty named/End responses.
/// Missing former mechanism policy becomes Fixed; numeric binding/profile/raw and response bytes remain exact.
/// # Panics
/// Panics if a recorded real fit is unavailable, conversion guesses Cover, loses authority or mutates its input.
#[test]
fn stage_c_real_fixed_config17_preserves_named_end_and_measured_artifacts() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/offset-coverage-20261019/implementation/stage-c");
    let project = load(&root.join("fixed-movement/zero.toniator")).unwrap();
    let original = project.document();
    original.require_saved_calibration_authority().unwrap();
    assert!(!original.temporal_end_overrides().is_empty());
    assert!(!original.channel_response_overrides().is_empty());
    let current = envelope(
        &root.join("fixed-movement/zero.toniator-preset"),
        "preset.json",
    );
    let mut former = current.clone();
    former["document_schema_version"] = json!(17);
    for bundle in former["configuration"]["pattern_definition_bundles"]
        .as_array_mut()
        .unwrap()
    {
        for mechanism in bundle["definition"]["mechanisms"].as_array_mut().unwrap() {
            if mechanism["kind"] == "parametric_curve_source" {
                mechanism.as_object_mut().unwrap().remove("spiral_coverage");
            }
        }
    }
    let path = stage_c_output("real-fixed-config17.toniator-preset");
    write_envelope(&path, "preset.json", &former);
    let before = fs::read(&path).unwrap();
    let StagedDocumentPreset::Current(configuration) =
        load_document_preset_staged(&path, original).unwrap()
    else {
        panic!("genuine unchanged Fixed artifacts must stay exact current authority");
    };
    let rebound = configuration.bind(original).unwrap();
    rebound.require_saved_calibration_authority().unwrap();
    assert_eq!(rebound.saved_calibrations(), original.saved_calibrations());
    assert_eq!(
        rebound.baseline_saved_calibration().unwrap(),
        original.baseline_saved_calibration().unwrap()
    );
    assert_eq!(
        rebound.temporal_end_overrides(),
        original.temporal_end_overrides()
    );
    let converted = stage_c_output("real-fixed-converted18.toniator-preset");
    save_document_preset(
        &converted,
        &DocumentConfiguration::capture(&rebound),
        &capture_document_preset_destination(&converted).unwrap(),
    )
    .unwrap();
    let after = envelope(&converted, "preset.json");
    for field in [
        "saved_calibrations",
        "baseline_saved_calibration",
        "response_settings",
        "channel_response_overrides",
        "end_overrides",
    ] {
        assert_eq!(
            serde_json::to_vec(&after["configuration"][field]).unwrap(),
            serde_json::to_vec(&current["configuration"][field]).unwrap(),
            "{field}"
        );
    }
    assert_eq!(fs::read(&path).unwrap(), before);
    let reopened = load_document_preset(&converted, original)
        .unwrap()
        .bind(original)
        .unwrap();
    assert_eq!(reopened, rebound);
    save(
        &stage_c_output("real-fixed-converted.toniator"),
        &rebound,
        project.sources(),
    )
    .unwrap();
}

/// Retains explicit old generic Cover intent and nonempty responses while dropping only recognized obsolete numbers.
/// The actual old-profile companion uses measured prior Fixed raw knots strictly as an incompatible donor, never relabelled authority.
/// # Panics
/// Panics if conversion changes authored controls/responses, accepts stale numbers or mutates original Preset bytes.
#[test]
fn stage_c_explicit_generic_cover_drops_obsolete_companion_without_losing_intent() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/offset-coverage-20261019/implementation/stage-c");
    let project = load(&root.join("reported/movement/zero.toniator")).unwrap();
    let original = project.document();
    let current = envelope(
        &root.join("reported/movement/zero.toniator-preset"),
        "preset.json",
    );
    assert!(!original.temporal_end_overrides().is_empty());
    let fixed = envelope(
        &root.join("fixed-movement/zero.toniator-preset"),
        "preset.json",
    );
    let mut former = current.clone();
    former["document_schema_version"] = json!(17);
    for (index, artifact) in former["configuration"]["saved_calibrations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        artifact["profile"] =
            fixed["configuration"]["saved_calibrations"][index]["profile"].clone();
        artifact["raw"] = fixed["configuration"]["saved_calibrations"][index]["raw"].clone();
    }
    former["configuration"]["baseline_saved_calibration"]["profile"] =
        fixed["configuration"]["baseline_saved_calibration"]["profile"].clone();
    former["configuration"]["baseline_saved_calibration"]["raw"] =
        fixed["configuration"]["baseline_saved_calibration"]["raw"].clone();
    let path = stage_c_output("explicit-generic-cover-config17.toniator-preset");
    write_envelope(&path, "preset.json", &former);
    let before = fs::read(&path).unwrap();
    let configuration = match load_document_preset_staged(&path, original).unwrap() {
        StagedDocumentPreset::Current(value) | StagedDocumentPreset::PendingCalibration(value) => {
            value
        }
        StagedDocumentPreset::PendingRaw(_) => {
            panic!("explicit Cover remains typed desired intent")
        }
    };
    let rebound = configuration.bind(original).unwrap();
    assert!(rebound.saved_calibrations().is_empty());
    assert!(rebound.baseline_saved_calibration().unwrap().is_none());
    assert!(
        CalibrationBinding::for_channel(&rebound, ChannelId(1))
            .unwrap()
            .requires_canvas_coverage()
    );
    assert_eq!(
        rebound.pattern_definition_bundles(),
        original.pattern_definition_bundles()
    );
    assert_eq!(rebound.response_settings(), original.response_settings());
    assert_eq!(
        rebound.channel_response_overrides(),
        original.channel_response_overrides()
    );
    assert_eq!(
        rebound.temporal_end_overrides(),
        original.temporal_end_overrides()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    save(
        &stage_c_output("explicit-cover-awaiting-genuine-prepare.toniator"),
        &rebound,
        project.sources(),
    )
    .unwrap();
}

/// Saves and reopens newly measured Cover authority after supported source-free obsolete-number removal.
/// Named and hidden ALL artifacts are genuine solver outputs; nonempty Start/End response intent remains exact.
/// # Panics
/// Panics if the real solver evidence is absent, strict authority fails or current Preset persistence changes bytes.
#[test]
fn stage_c_reprepared_cover_saves_and_reopens_real_named_all_and_end() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/offset-coverage-20261019/implementation/stage-c");
    let project =
        load(&root.join("io-tests/explicit-cover-genuine-prepared/prepared.toniator")).unwrap();
    let document = project.document();
    document.require_saved_calibration_authority().unwrap();
    assert_eq!(document.saved_calibrations().len(), 3);
    assert!(document.baseline_saved_calibration().unwrap().is_some());
    assert!(!document.temporal_end_overrides().is_empty());
    let path = stage_c_output("explicit-cover-newly-measured-current18.toniator-preset");
    save_document_preset(
        &path,
        &DocumentConfiguration::capture(document),
        &capture_document_preset_destination(&path).unwrap(),
    )
    .unwrap();
    let reopened = load_document_preset(&path, document)
        .unwrap()
        .bind(document)
        .unwrap();
    reopened.require_saved_calibration_authority().unwrap();
    assert_eq!(reopened, *document);
    let before = envelope(
        &root.join("reported/movement/zero.toniator-preset"),
        "preset.json",
    );
    let after = envelope(&path, "preset.json");
    for field in [
        "response_settings",
        "channel_response_overrides",
        "end_overrides",
    ] {
        assert_eq!(
            before["configuration"][field], after["configuration"][field],
            "{field}"
        );
    }
}
