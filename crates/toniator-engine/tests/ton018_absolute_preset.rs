use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

use serde_json::{Value, json};
use toniator_domain::{
    CanvasSpec, Document, DocumentConfiguration, DocumentHistory, DocumentSession, MarkDensityMode,
    PropertyFieldId, PropertyTarget, SourceReference, SourceReferenceId, TemporalEndOverride,
};
use toniator_engine::{
    EvaluationLimits, EvaluationRequest, ResolvedSource, SourceFormatHint, evaluate,
    prepare_pending_raw_preset,
};
use toniator_io::{
    StagedDocumentPreset, capture_document_preset_destination, load_document_preset,
    load_document_preset_staged, save_document_preset,
};
use toniator_patterns::PresetRegistry;
use toniator_render::GeometryOutput;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Writes one validation-only Preset envelope without touching the user's original files.
///
/// # Panics
/// Panics if the bounded target fixture cannot be written.
fn write_preset(path: &std::path::Path, envelope: &Value) {
    let mut archive = ZipWriter::new(fs::File::create(path).unwrap());
    archive
        .start_file("preset.json", SimpleFileOptions::default())
        .unwrap();
    archive
        .write_all(&serde_json::to_vec(envelope).unwrap())
        .unwrap();
    archive.finish().unwrap();
}

/// Stages a former raw range above one and converts it through its retained measured curve.
///
/// # Panics
/// Panics if old raw intent is clipped, the neutral shell escapes, or source bytes change.
#[test]
fn former_raw_preset_requires_coverage_conversion() {
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018/absolute-preset");
    fs::create_dir_all(&output).unwrap();
    let initial = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(initial).unwrap());
    PresetRegistry::bundled()
        .apply_to_document_base(&mut history, "straight-grid-circles")
        .unwrap();
    let document = history.document().clone();
    let current = output.join("current.toniator-preset");
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
    envelope["document_schema_version"] = json!(15);
    envelope["configuration"]["response_settings"]["marks_minimum_fill"] = json!(0.25);
    envelope["configuration"]["response_settings"]["marks_maximum_fill"] = json!(1.5);
    let baseline = document.baseline_saved_calibration().unwrap().unwrap();
    assert!(
        PresetRegistry::bundled()
            .entries()
            .iter()
            .filter_map(|record| record.saved_calibration.as_ref())
            .any(|saved| saved.binding() == baseline.binding())
    );
    envelope["configuration"]["saved_calibrations"] = json!([]);
    envelope["configuration"]["baseline_saved_calibration"] = Value::Null;
    let former = output.join("former-raw.toniator-preset");
    write_preset(&former, &envelope);
    let original = fs::read(&former).unwrap();
    assert!(load_document_preset(&former, &document).is_err());
    let StagedDocumentPreset::PendingRaw(cancelled) =
        load_document_preset_staged(&former, &document).unwrap()
    else {
        panic!("former raw Preset must remain pending");
    };
    assert!(
        prepare_pending_raw_preset(cancelled, &document, EvaluationLimits::default(), &|| true)
            .is_err()
    );
    let StagedDocumentPreset::PendingRaw(pending) =
        load_document_preset_staged(&former, &document).unwrap()
    else {
        panic!("former raw Preset must remain pending");
    };
    let prepared =
        prepare_pending_raw_preset(pending, &document, EvaluationLimits::default(), &|| false)
            .unwrap();
    let expected = |raw: f64| {
        let points = baseline.raw();
        let upper = points.partition_point(|point| point.input < raw);
        if points[upper].input == raw {
            return points[upper].coverage;
        }
        let left = &points[upper - 1];
        let right = &points[upper];
        left.coverage
            + (raw - left.input) / (right.input - left.input) * (right.coverage - left.coverage)
    };
    assert!((prepared.response_settings().marks.minimum_fill - expected(0.25)).abs() < 1e-12);
    assert!((prepared.response_settings().marks.maximum_fill - expected(1.5)).abs() < 1e-12);
    let rebound = prepared.bind(&document).unwrap();
    rebound.require_saved_calibration_authority().unwrap();
    assert_eq!(fs::read(&former).unwrap(), original);

    let channel = document.channel_topology().unwrap().channels()[0].id;
    let output_id = document
        .effective_channel_pattern(channel)
        .unwrap()
        .output_settings[0]
        .output_layer_id;
    let response_end = json!({"kind":"scalar", "target":{"scope":"channel_output",
        "channel_id":channel.0, "output_id":output_id.0}, "field":"mark_maximum_fill",
        "end":0.1, "easing":"linear"});
    let mut duplicate = envelope.clone();
    duplicate["configuration"]["end_overrides"] =
        json!([response_end.clone(), response_end.clone()]);
    let duplicate_path = output.join("duplicate-raw-end.toniator-preset");
    write_preset(&duplicate_path, &duplicate);
    let StagedDocumentPreset::PendingRaw(pending) =
        load_document_preset_staged(&duplicate_path, &document).unwrap()
    else {
        panic!("duplicate former End remains pending");
    };
    assert_eq!(
        prepare_pending_raw_preset(pending, &document, EvaluationLimits::default(), &|| false)
            .unwrap_err()
            .path(),
        "preset.raw_end.duplicate"
    );
    let mut foreign = envelope;
    let mut response_end = response_end;
    response_end["target"]["output_id"] = json!(999);
    foreign["configuration"]["end_overrides"] = json!([response_end]);
    let foreign_path = output.join("foreign-raw-end.toniator-preset");
    write_preset(&foreign_path, &foreign);
    let StagedDocumentPreset::PendingRaw(pending) =
        load_document_preset_staged(&foreign_path, &document).unwrap()
    else {
        panic!("foreign former End remains pending");
    };
    assert_eq!(
        prepare_pending_raw_preset(pending, &document, EvaluationLimits::default(), &|| false)
            .unwrap_err()
            .path(),
        "preset.raw_end.target"
    );
}

/// A former raw Guide Faces Preset discards only its v4 numeric cache, then maps raw Start and
/// initialized named End through the newly profiled exact curve before publishing configuration.
///
/// # Panics
/// Panics if config15 conversion reuses the obsolete profile, clips raw values above one,
/// changes easing or file bytes, or exposes a configuration without all companions.
#[test]
fn former_raw_guide_faces_preset_uses_revised_profile_and_curve() {
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018/absolute-preset");
    fs::create_dir_all(&output).unwrap();
    let initial = Document::new_default_document(
        CanvasSpec { width: 1024.0, height: 1024.0 }, SourceReference::Unassigned,
    ).unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(initial).unwrap());
    PresetRegistry::bundled().apply_to_document_base(&mut history, "three-guide-cells-scale").unwrap();
    let document = history.document();
    let channel_id = document.channel_topology().unwrap().channels()[0].id;
    let output_id = document.effective_channel_pattern(channel_id).unwrap()
        .output_settings[0].output_layer_id;
    let current = output.join("guide-faces-profile-v2-current.toniator-preset");
    save_document_preset(&current, &DocumentConfiguration::capture(document),
        &capture_document_preset_destination(&current).unwrap()).unwrap();
    let mut bytes = Vec::new();
    ZipArchive::new(fs::File::open(&current).unwrap()).unwrap()
        .by_name("preset.json").unwrap().read_to_end(&mut bytes).unwrap();
    let mut envelope: Value = serde_json::from_slice(&bytes).unwrap();
    envelope["document_schema_version"] = json!(15);
    envelope["configuration"]["response_settings"]["regions_minimum_fill"] = json!(0.25);
    envelope["configuration"]["response_settings"]["regions_maximum_fill"] = json!(1.5);
    for companion in envelope["configuration"]["saved_calibrations"].as_array_mut().unwrap() {
        companion["profile"] = json!("ton018-reference-fixed-canvas1024-spacings32-v4");
    }
    envelope["configuration"]["baseline_saved_calibration"]["profile"] =
        json!("ton018-reference-fixed-canvas1024-spacings32-v4");
    envelope["configuration"]["end_overrides"] = json!([{
        "kind":"scalar", "target":{"scope":"channel_output", "channel_id":channel_id.0,
            "output_id":output_id.0}, "field":"region_maximum_fill", "end":0.3,
        "easing":"smooth_step"
    }]);
    let former = output.join("guide-faces-profile-v1-raw15.toniator-preset");
    write_preset(&former, &envelope);
    let original = fs::read(&former).unwrap();
    let StagedDocumentPreset::PendingRaw(pending) =
        load_document_preset_staged(&former, document).unwrap() else {
            panic!("former raw Guide Faces intent must remain private");
        };
    let rebound = prepare_pending_raw_preset(pending, document, EvaluationLimits::default(), &|| false)
        .unwrap().bind(document).unwrap();
    rebound.require_saved_calibration_authority().unwrap();
    let saved = rebound.saved_calibration_for(channel_id).unwrap().unwrap();
    assert_eq!(saved.profile(), toniator_domain::CALIBRATION_PROFILE_GUIDE_FACES);
    let expected = |raw: f64| {
        let points = saved.raw();
        let upper = points.partition_point(|point| point.input < raw);
        if points[upper].input == raw { return points[upper].coverage; }
        let left = &points[upper - 1];
        let right = &points[upper];
        left.coverage + (raw - left.input) / (right.input - left.input)
            * (right.coverage - left.coverage)
    };
    assert!((rebound.response_settings().regions_minimum_fill - expected(0.25)).abs() < 1e-12);
    assert!((rebound.response_settings().regions_maximum_fill - expected(1.5)).abs() < 1e-12);
    let end = rebound.temporal_end_overrides().iter().find_map(|entry| match entry {
        TemporalEndOverride::Scalar(value)
            if value.target == PropertyTarget::ChannelOutput(channel_id, output_id)
                && value.field == PropertyFieldId::RegionMaximumFill => Some(value),
        _ => None,
    }).unwrap();
    assert_eq!(end.easing, toniator_domain::Easing::SmoothStep);
    assert!((end.end + rebound.response_settings().regions_maximum_fill - expected(1.8)).abs() < 1e-12);
    assert_eq!(fs::read(&former).unwrap(), original);
}

/// Converts a named connected End through its own curve while hidden ALL remains circular marks.
///
/// # Panics
/// Panics if an inactive ALL kind blocks conversion or its raw End is silently clipped.
#[test]
fn mixed_kind_named_end_uses_its_own_saved_curve() {
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018/absolute-preset");
    fs::create_dir_all(&output).unwrap();
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let channel_id = document.channel_topology().unwrap().channels()[1].id;
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    PresetRegistry::bundled()
        .apply_to_selected(&mut history, channel_id, "one-guide-lines")
        .unwrap();
    let document = history.document();
    let output_id = document
        .effective_channel_pattern(channel_id)
        .unwrap()
        .output_settings[0]
        .output_layer_id;
    let current = output.join("mixed-current.toniator-preset");
    save_document_preset(
        &current,
        &DocumentConfiguration::capture(document),
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
    envelope["document_schema_version"] = json!(15);
    envelope["configuration"]["response_settings"]["connected_minimum_thickness"] = json!(0.2);
    envelope["configuration"]["response_settings"]["connected_maximum_thickness"] = json!(1.3);
    envelope["configuration"]["channel_response_overrides"][0]["response"]["minimum_thickness"] =
        json!(0.2);
    envelope["configuration"]["channel_response_overrides"][0]["response"]["maximum_thickness"] =
        json!(1.3);
    envelope["configuration"]["end_overrides"] = json!([{
        "kind":"scalar", "target":{"scope":"channel_output", "channel_id":channel_id.0, "output_id":output_id.0},
        "field":"connected_maximum_thickness", "end":0.5, "easing":"smooth_step"
    }]);
    let former = output.join("mixed-former-raw.toniator-preset");
    write_preset(&former, &envelope);
    let StagedDocumentPreset::PendingRaw(pending) =
        load_document_preset_staged(&former, document).unwrap()
    else {
        panic!("former raw Preset must remain pending");
    };
    let prepared =
        prepare_pending_raw_preset(pending, document, EvaluationLimits::default(), &|| false)
            .unwrap();
    let rebound = prepared.bind(document).unwrap();
    let saved = rebound.saved_calibration_for(channel_id).unwrap().unwrap();
    let expected = |raw: f64| {
        let points = saved.raw();
        let upper = points.partition_point(|point| point.input < raw);
        if points[upper].input == raw {
            return points[upper].coverage;
        }
        let left = &points[upper - 1];
        let right = &points[upper];
        left.coverage
            + (raw - left.input) / (right.input - left.input) * (right.coverage - left.coverage)
    };
    let effective = rebound.effective_channel_pattern(channel_id).unwrap();
    let toniator_domain::PatternGeometryResponse::Connected(response) =
        &effective.output_settings[0].response
    else {
        panic!("named output remains connected");
    };
    assert!((response.minimum_thickness - expected(0.2)).abs() < 1e-12);
    assert!((response.maximum_thickness - expected(1.3)).abs() < 1e-12);
    assert!(
        rebound
            .temporal_end_overrides()
            .iter()
            .all(|entry| !matches!(entry,
        TemporalEndOverride::Scalar(value) if value.target == PropertyTarget::Document
            && value.field == PropertyFieldId::ConnectedMaximumThickness))
    );
    let named_end = rebound
        .temporal_end_overrides()
        .iter()
        .find_map(|entry| match entry {
            TemporalEndOverride::Scalar(value)
                if value.target == PropertyTarget::ChannelOutput(channel_id, output_id)
                    && value.field == PropertyFieldId::ConnectedMaximumThickness =>
            {
                Some(value)
            }
            _ => None,
        })
        .unwrap();
    assert!(
        (named_end.end + rebound.response_settings().connected_maximum_thickness - expected(1.8))
            .abs()
            < 1e-12
    );
}

/// Keeps an inherited former raw interval numerically intact across unequal same-kind bindings.
///
/// # Panics
/// Panics if conversion reuses hidden ALL measurements for a distinct named Pattern.
#[test]
fn inherited_raw_range_materializes_distinct_named_coverage() {
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018/absolute-preset");
    fs::create_dir_all(&output).unwrap();
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let channel_id = document.channel_topology().unwrap().channels()[1].id;
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    PresetRegistry::bundled()
        .apply_to_selected(&mut history, channel_id, "triagrid-custom-shape-marks")
        .unwrap();
    let document = history.document();
    let output_id = document
        .effective_channel_pattern(channel_id)
        .unwrap()
        .output_settings[0]
        .output_layer_id;
    let current = output.join("distinct-current.toniator-preset");
    save_document_preset(
        &current,
        &DocumentConfiguration::capture(document),
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
    envelope["document_schema_version"] = json!(15);
    envelope["configuration"]["response_settings"]["marks_minimum_fill"] = json!(0.0);
    envelope["configuration"]["response_settings"]["marks_maximum_fill"] = json!(0.5);
    envelope["configuration"]["response_settings"]["mark_density_mode"] = json!("variable");
    envelope["configuration"]["channel_response_overrides"] = json!([]);
    envelope["configuration"]["end_overrides"] = json!([{
        "kind":"scalar", "target":{"scope":"channel_output", "channel_id":channel_id.0, "output_id":output_id.0},
        "field":"mark_maximum_fill", "end":0.3, "easing":"quadratic_out"
    }]);
    let former = output.join("distinct-former-raw.toniator-preset");
    write_preset(&former, &envelope);
    let StagedDocumentPreset::PendingRaw(pending) =
        load_document_preset_staged(&former, document).unwrap()
    else {
        panic!("former raw Preset must remain pending");
    };
    let rebound =
        prepare_pending_raw_preset(pending, document, EvaluationLimits::default(), &|| false)
            .unwrap()
            .bind(document)
            .unwrap();
    let named = rebound.saved_calibration_for(channel_id).unwrap().unwrap();
    let shared = rebound.baseline_saved_calibration().unwrap().unwrap();
    assert_ne!(named.binding(), shared.binding());
    let named_at_half = named
        .raw()
        .iter()
        .find(|point| point.input == 0.5)
        .unwrap()
        .coverage;
    let shared_at_half = shared
        .raw()
        .iter()
        .find(|point| point.input == 0.5)
        .unwrap()
        .coverage;
    assert!(
        (named_at_half - shared_at_half).abs() > 0.001,
        "named={named_at_half}, shared={shared_at_half}"
    );
    let toniator_domain::PatternGeometryResponse::Marks(response) = &rebound
        .effective_channel_pattern(channel_id)
        .unwrap()
        .output_settings[0]
        .response
    else {
        panic!("named output remains marks");
    };
    assert!((response.maximum_fill - named_at_half).abs() < 1e-12);
    assert!((rebound.response_settings().marks.maximum_fill - shared_at_half).abs() < 1e-12);
    let named_end = rebound
        .temporal_end_overrides()
        .iter()
        .find_map(|entry| match entry {
            TemporalEndOverride::Scalar(value)
                if value.target == PropertyTarget::ChannelOutput(channel_id, output_id)
                    && value.field == PropertyFieldId::MarkMaximumFill =>
            {
                Some(value)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(named_end.easing, toniator_domain::Easing::QuadraticOut);
    let current = output.join("distinct-converted-current.toniator-preset");
    save_document_preset(
        &current,
        &DocumentConfiguration::capture(&rebound),
        &capture_document_preset_destination(&current).unwrap(),
    )
    .unwrap();
    let restored = load_document_preset(&current, &rebound)
        .unwrap()
        .bind(&rebound)
        .unwrap();
    assert_eq!(restored.response_settings(), rebound.response_settings());
    assert_eq!(
        restored.channel_response_overrides(),
        rebound.channel_response_overrides()
    );
    assert_eq!(
        restored.temporal_end_overrides(),
        rebound.temporal_end_overrides()
    );
}

/// Uses real source sampling to keep exact black at zero size and a near-black tone positive.
///
/// # Panics
/// Panics if source zero is raised by the inverse or a positive low tone is rounded to zero.
#[test]
fn variable_marks_keep_zero_and_tiny_source_tones_distinct() {
    let source_id = SourceReferenceId::new("absolute-source-witness").unwrap();
    let initial = Document::new_default_document(
        CanvasSpec {
            width: 64.0,
            height: 64.0,
        },
        SourceReference::Assigned(source_id.clone()),
    )
    .unwrap();
    let mut settings = initial.response_settings().clone();
    settings.mark_density_mode = MarkDensityMode::Variable;
    settings.marks.minimum_fill = 0.0;
    settings.marks.maximum_fill = 1.0;
    let document = initial
        .with_response_authority(settings, Vec::new())
        .unwrap();
    let sample = |hex: &str| {
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"64\" height=\"64\"><rect width=\"64\" height=\"64\" fill=\"#{hex}\"/></svg>"
        );
        let session = DocumentSession::new(document.clone()).unwrap();
        let request = EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), svg.into_bytes(), SourceFormatHint::Svg)
                .unwrap(),
        );
        let result = evaluate(request).unwrap();
        result.scene().layers()[0]
            .outputs()
            .iter()
            .flat_map(|output| match output.geometry() {
                GeometryOutput::CircularMarks(marks) => {
                    marks.iter().map(|mark| mark.radius).collect::<Vec<_>>()
                }
                _ => Vec::new(),
            })
            .collect::<Vec<_>>()
    };
    let zero = sample("000000");
    let tiny = sample("010101");
    assert!(!zero.is_empty());
    assert!(zero.iter().all(|radius| *radius == 0.0));
    assert_eq!(zero.len(), tiny.len());
    assert!(tiny.iter().any(|radius| *radius > 0.0));
}

/// Stages optional read-only personal source-free Presets without fitting or rewriting originals.
///
/// # Panics
/// Panics if a recognized archive cannot become a typed pending import against the current shell.
#[test]
fn observed_personal_presets_stage_as_pending_raw() {
    let Some(paths) = std::env::var_os("TONIATOR_PRESET_STAGE_DIRS") else {
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
    let mut count = 0;
    for directory in std::env::split_paths(&paths) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_none_or(|extension| extension != "toniator-preset")
            {
                continue;
            }
            let before = fs::read(&path).unwrap();
            assert!(
                matches!(
                    load_document_preset_staged(&path, &destination).unwrap(),
                    StagedDocumentPreset::PendingRaw(_)
                ),
                "{}",
                path.display()
            );
            assert_eq!(fs::read(&path).unwrap(), before);
            count += 1;
        }
    }
    assert!(count > 0);
}
