use std::{fs, fs::File, io::Read, path::PathBuf};

use serde_json::Value;
use zip::ZipArchive;

use toniator_domain::{
    CanvasSpec, ChannelId, ChannelOutputResponse, Document, DocumentCommand, DocumentConfiguration,
    Easing, FrameRange, FrameRate, MarkDensityMode, MarkGeometryResponse, PresetMetadata,
    PresetRecord, ProjectTiming, PropertyFieldId, PropertyTarget, SourceReference,
    SourceReferenceId, TemporalEndpointEdit,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, capture_document_preset_destination, load,
    load_document_preset, load_preset, save, save_document_preset, save_preset,
};

/// Builds a source-backed current document whose ALL and named response fields differ.
///
/// # Panics
/// Panics if the current domain or embedded-source invariants reject the fixture.
fn fixture() -> (Document, SourceBundle) {
    let source_id = SourceReferenceId::new("ton018-response-source").expect("source ID validates");
    let mut document = Document::new_default_document(
        CanvasSpec {
            width: 64.0,
            height: 48.0,
        },
        SourceReference::Assigned(source_id.clone()),
    )
    .expect("default document validates");
    let mut settings = document.response_settings().clone();
    settings.marks = MarkGeometryResponse {
        minimum_fill: 0.1,
        maximum_fill: 0.9,
    };
    settings.mark_density_mode = MarkDensityMode::Uniform;
    settings.mark_size_scale = 1.3;
    document = document
        .apply_command(&DocumentCommand::SetAllResponseSettings {
            base: document.response_settings().clone(),
            settings,
        })
        .expect("ALL response validates")
        .0;
    let output_layer_id = document
        .effective_channel_pattern(ChannelId(1))
        .expect("channel resolves")
        .output_settings[0]
        .output_layer_id;
    document = document
        .apply_command(&DocumentCommand::SetChannelResponse {
            channel_id: ChannelId(1),
            output_layer_id,
            response: ChannelOutputResponse::Marks {
                minimum_fill: Some(0.3),
                maximum_fill: None,
                density_mode: Some(MarkDensityMode::Variable),
                size_scale: Some(0.8),
            },
        })
        .expect("named response validates")
        .0;
    let source = EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"64\" height=\"48\"/>".to_vec(),
        Some("ton018-source.svg".into()),
    )
    .expect("embedded SVG validates");
    (
        document,
        SourceBundle::new([source]).expect("source bundle validates"),
    )
}

/// Locates TON-018 derived persistence artifacts below the validation directory.
///
/// # Panics
/// Panics if the validation directory cannot be created.
fn validation_directory() -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018-uniform-density-production-20261009/io");
    fs::create_dir_all(&directory).expect("validation directory creates");
    directory
}

/// Proves project 14 and document Preset configuration 13 retain separate ALL and named intent.
/// The persisted structural Pattern response excludes adjustable fill endpoints.
///
/// # Panics
/// Panics if current archive serialization, versioning, or restoration loses response intent.
#[test]
fn project_and_document_preset_round_trip_response_authority() {
    let directory = validation_directory();
    let (document, sources) = fixture();
    let project = directory.join("response.toniator");
    save(&project, &document, &sources).expect("current project saves");
    let loaded = load(&project).expect("current project loads");
    assert_eq!(loaded.document(), &document);
    assert_eq!(
        loaded.versions().document(),
        toniator_io::DOCUMENT_SCHEMA_VERSION
    );
    let mut archive = ZipArchive::new(File::open(&project).expect("project opens"))
        .expect("project archive validates");
    let mut document_json = Vec::new();
    archive
        .by_name("document.json")
        .expect("document member exists")
        .read_to_end(&mut document_json)
        .expect("document member reads");
    let json: Value = serde_json::from_slice(&document_json).expect("current document JSON parses");
    let structural_response =
        &json["document"]["pattern_definition_bundles"][0]["output_settings"][0]["response"];
    assert_eq!(structural_response["kind"], "marks");
    assert!(structural_response.get("minimum_fill").is_none());
    assert_eq!(
        json["document"]["response_settings"]["marks_minimum_fill"],
        0.1
    );
    assert_eq!(
        json["document"]["channel_response_overrides"][0]["response"]["minimum_fill"],
        0.3
    );
    assert_eq!(
        json["document"]["response_settings"]["mark_density_mode"],
        "uniform"
    );
    assert_eq!(
        json["document"]["response_settings"]["mark_size_scale"],
        1.3
    );
    assert_eq!(
        json["document"]["channel_response_overrides"][0]["response"]["density_mode"],
        "variable"
    );
    assert_eq!(
        json["document"]["channel_response_overrides"][0]["response"]["size_scale"],
        0.8
    );

    let configuration = DocumentConfiguration::capture(&document);
    let preset = directory.join("response.toniator-preset");
    let guard = capture_document_preset_destination(&preset).expect("destination guard captures");
    save_document_preset(&preset, &configuration, &guard).expect("current document Preset saves");
    let loaded_configuration =
        load_document_preset(&preset, &document).expect("current document Preset loads");
    assert_eq!(loaded_configuration, configuration);
}

/// Proves standalone Pattern preset 6 stores only structural response kind and rejects version 5.
///
/// # Panics
/// Panics if a reconstructed current Pattern, standalone persistence, or JSON boundary fails.
#[test]
fn standalone_pattern_preset_omits_response_bounds_and_rejects_obsolete_version() {
    let directory = validation_directory();
    let (document, _) = fixture();
    let recipe = document
        .reconstruct_pattern_definition_recipe(document.pattern_settings().definition_id)
        .expect("structural recipe reconstructs");
    let preset = PresetRecord {
        saved_calibration: None,
        metadata: PresetMetadata {
            id: "ton018-structural-response".into(),
            name: "TON-018 structural response".into(),
            category: "Test".into(),
            description: "Only Pattern construction persists".into(),
            thumbnail: None,
        },
        recipe,
    };
    let current = directory.join("structural.toniator-pattern");
    save_preset(&current, &preset).expect("current standalone Pattern saves");
    assert_eq!(
        load_preset(&current).expect("current standalone Pattern loads"),
        preset
    );
    let mut json: Value = serde_json::from_slice(&fs::read(&current).expect("preset reads"))
        .expect("preset JSON parses");
    assert_eq!(
        json["preset_format_version"],
        toniator_io::PRESET_FORMAT_VERSION
    );
    assert!(json.to_string().contains("\"marks\""));
    assert!(!json.to_string().contains("minimum_fill"));
    json["preset_format_version"] = Value::from(5);
    let obsolete = directory.join("obsolete-v5-copy.toniator-pattern");
    fs::write(
        &obsolete,
        serde_json::to_vec(&json).expect("obsolete copy encodes"),
    )
    .expect("obsolete copy writes");
    assert!(load_preset(&obsolete).is_err());
}

/// Proves animatable mark-size scale End values and easing survive both current configuration formats.
///
/// # Panics
/// Panics if initialized End intent, edited scale, easing, or archive restoration is lost.
#[test]
fn mark_size_scale_end_round_trips_in_project_and_document_preset() {
    let directory = validation_directory();
    let (document, sources) = fixture();
    let initialized_command = document
        .initialize_end_command()
        .expect("typed temporal fields initialize End");
    let initialized = document
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(24, 1).expect("test frame rate validates"),
                FrameRange::new(0, 2).expect("two-frame endpoint interval validates"),
            ),
            initialized_command.replacement().end_overrides.clone(),
        )
        .expect("initialized End configuration validates");
    let output_layer_id = initialized
        .effective_channel_pattern(ChannelId(1))
        .expect("channel resolves")
        .output_settings[0]
        .output_layer_id;
    let scale_end = initialized
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::ChannelOutput(ChannelId(1), output_layer_id),
            field: PropertyFieldId::MarkSizeScale,
            effective_end: 1.75,
            easing: Easing::SmoothStep,
        }])
        .expect("named End scale edit validates");
    let edited = initialized
        .with_temporal_authority(
            scale_end.replacement().project_timing.clone(),
            scale_end.replacement().end_overrides.clone(),
        )
        .expect("edited End scale validates");

    let project = directory.join("mark-size-scale-end.toniator");
    save(&project, &edited, &sources).expect("project with scale End saves");
    let loaded = load(&project).expect("project with scale End loads");
    assert_eq!(loaded.document(), &edited);
    let stored_end = json_project_end_override(&project, "mark_size_scale");
    // Channel-output tracks persist a delta from the ALL scale (1.3), while
    // the displayed effective endpoint remains 1.75.
    assert!((stored_end["end"].as_f64().expect("scale delta is numeric") - 0.45).abs() < 1e-12);
    assert_eq!(stored_end["easing"], "smooth_step");
    let last_frame = loaded
        .document()
        .project_timing()
        .frame_range()
        .end_exclusive()
        - 1;
    let endpoint = loaded
        .document()
        .materialize_frame(last_frame)
        .expect("scale endpoint materializes")
        .channel_scalar_batch(PropertyFieldId::MarkSizeScale)
        .expect("mark scale batch remains available");
    assert_eq!(
        endpoint
            .values
            .iter()
            .find(|value| value.target
                == PropertyTarget::ChannelOutput(ChannelId(1), output_layer_id))
            .expect("named mark output remains in the batch")
            .value,
        1.75
    );

    let configuration = DocumentConfiguration::capture(&edited);
    let preset = directory.join("mark-size-scale-end.toniator-preset");
    let guard = capture_document_preset_destination(&preset).expect("preset guard captures");
    save_document_preset(&preset, &configuration, &guard)
        .expect("document Preset with scale End saves");
    let loaded_configuration =
        load_document_preset(&preset, &edited).expect("document Preset with scale End loads");
    let rebound = loaded_configuration
        .bind(&edited)
        .expect("document Preset configuration binds");
    assert_eq!(
        rebound.temporal_end_overrides(),
        edited.temporal_end_overrides()
    );
}

/// Reads one named End override from a generated current-schema project archive.
///
/// # Panics
/// Panics if the archive omits the requested End field or its JSON is malformed.
fn json_project_end_override(path: &std::path::Path, field: &str) -> Value {
    let mut archive = ZipArchive::new(File::open(path).expect("project archive opens"))
        .expect("project archive validates");
    let mut document_json = Vec::new();
    archive
        .by_name("document.json")
        .expect("document entry exists")
        .read_to_end(&mut document_json)
        .expect("document entry reads");
    let json: Value = serde_json::from_slice(&document_json).expect("document JSON parses");
    json["document"]["end_overrides"]
        .as_array()
        .expect("End overrides are an array")
        .iter()
        .find(|value| value["field"] == field)
        .cloned()
        .expect("requested End field exists")
}
