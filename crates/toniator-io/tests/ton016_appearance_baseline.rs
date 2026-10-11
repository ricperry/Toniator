use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use toniator_domain::{
    CanvasSpec, Document, DocumentAppearanceBaseline, DocumentConfiguration, SourceReference,
    SourceReferenceId,
};
use toniator_io::{
    DOCUMENT_PRESET_CONFIGURATION_SCHEMA_VERSION, DOCUMENT_PRESET_FORMAT_VERSION,
    DOCUMENT_SCHEMA_VERSION, EmbeddedSource, EmbeddedSourceFormat, SourceBundle,
    capture_document_preset_destination, load, load_document_preset, save, save_document_preset,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Creates a unique directory for bounded project and Preset persistence witnesses.
///
/// # Panics
/// Panics if the system clock predates the Unix epoch or the directory already exists.
fn temporary_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock follows the Unix epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "toniator-ton016-appearance-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&directory).expect("isolated persistence directory creates");
    directory
}

/// Builds one source-backed document with explicit last-ALL Start values and exact source bytes.
///
/// # Panics
/// Panics if any current domain or source identity invariant rejects the fixture.
fn source_backed_document() -> (Document, SourceBundle) {
    let source_id =
        SourceReferenceId::new("ton016-persistence-source").expect("source reference ID validates");
    let document = Document::new_default_document(
        CanvasSpec {
            width: 90.0,
            height: 62.0,
        },
        SourceReference::Assigned(source_id.clone()),
    )
    .expect("source-backed default document validates")
    .with_appearance_baseline(DocumentAppearanceBaseline {
        density: Some(37.0),
        density_aspect: Some(1.6),
        pattern_rotation_degrees: Some(23.0),
        translation_x: Some(0.17),
        translation_y: Some(-0.21),
    })
    .expect("explicit baseline validates");
    let source = EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec(),
        Some("ton016-source.svg".into()),
    )
    .expect("embedded source validates");
    (
        document,
        SourceBundle::new([source]).expect("source bundle validates"),
    )
}

/// Rewrites one JSON archive member while preserving every other member's exact payload.
///
/// # Panics
/// Panics if the fixture archive, target member, JSON, or rewritten ZIP cannot be read and written.
fn rewrite_json_member(
    source: &Path,
    destination: &Path,
    member_name: &str,
    edit: impl FnOnce(&mut Value),
) {
    let mut reader = ZipArchive::new(File::open(source).expect("source archive opens"))
        .expect("source archive is ZIP");
    let mut writer = ZipWriter::new(File::create(destination).expect("destination creates"));
    let mut edit = Some(edit);
    for index in 0..reader.len() {
        let mut member = reader.by_index(index).expect("archive member opens");
        let name = member.name().to_owned();
        let options = SimpleFileOptions::default().compression_method(member.compression());
        if member.is_dir() {
            writer
                .add_directory(name, options)
                .expect("directory member copies");
            continue;
        }
        let mut bytes = Vec::new();
        member
            .read_to_end(&mut bytes)
            .expect("archive member reads");
        if name == member_name {
            let mut json: Value = serde_json::from_slice(&bytes).expect("target JSON parses");
            edit.take().expect("target member appears once")(&mut json);
            bytes = serde_json::to_vec(&json).expect("edited JSON serializes");
        }
        writer
            .start_file(name, options)
            .expect("archive member starts");
        writer
            .write_all(&bytes)
            .expect("archive member payload writes");
    }
    assert!(edit.is_none(), "target member exists in source archive");
    writer.finish().expect("rewritten archive closes");
}

/// Persists last-ALL Start intent through current project and source-free Preset round trips.
///
/// # Panics
/// Panics if project schema 12 or Preset configuration schema 11 loses exact baseline values.
#[test]
fn ton016_project_and_preset_round_trip_explicit_baseline() {
    let directory = temporary_directory();
    let (document, sources) = source_backed_document();
    let project = directory.join("appearance.toniator");
    save(&project, &document, &sources).expect("current project saves");
    let loaded_project = load(&project).expect("current project loads");
    assert_eq!(
        loaded_project.versions().document(),
        DOCUMENT_SCHEMA_VERSION
    );
    assert_eq!(loaded_project.document(), &document);
    assert_eq!(
        loaded_project.document().appearance_baseline(),
        document.appearance_baseline()
    );

    let configuration = DocumentConfiguration::capture(&document);
    let preset = directory.join("appearance.toniator-preset");
    let guard = capture_document_preset_destination(&preset).expect("new Preset path is guarded");
    save_document_preset(&preset, &configuration, &guard).expect("source-free Preset saves");
    let loaded_configuration = load_document_preset(&preset, &document)
        .expect("source-free Preset loads against destination");
    assert_eq!(loaded_configuration, configuration);
    assert_eq!(
        loaded_configuration.appearance_baseline(),
        configuration.appearance_baseline()
    );

    let mut archive =
        ZipArchive::new(File::open(&preset).expect("Preset opens")).expect("Preset archive is ZIP");
    let mut bytes = Vec::new();
    archive
        .by_name("preset.json")
        .expect("source-free Preset has one JSON member")
        .read_to_end(&mut bytes)
        .expect("Preset JSON reads");
    let json: Value = serde_json::from_slice(&bytes).expect("Preset JSON parses");
    assert_eq!(
        json["document_preset_format_version"],
        DOCUMENT_PRESET_FORMAT_VERSION
    );
    assert_eq!(
        json["document_schema_version"],
        DOCUMENT_PRESET_CONFIGURATION_SCHEMA_VERSION
    );
    assert_eq!(
        json["configuration"]["appearance_baseline"]["density"],
        37.0
    );
    drop(archive);
    fs::remove_dir_all(directory).expect("persistence witness directory removes");
}

/// Rejects obsolete schemas and malformed baseline values before project or Preset authority loads.
///
/// # Panics
/// Panics if a superseded schema or invalid stored effective density is accepted.
#[test]
fn ton016_obsolete_and_invalid_baselines_are_rejected() {
    let directory = temporary_directory();
    let (document, sources) = source_backed_document();
    let project = directory.join("current.toniator");
    save(&project, &document, &sources).expect("current project saves");
    let preset = directory.join("current.toniator-preset");
    let configuration = DocumentConfiguration::capture(&document);
    let guard = capture_document_preset_destination(&preset).expect("new Preset path is guarded");
    save_document_preset(&preset, &configuration, &guard).expect("current Preset saves");

    let obsolete_project = directory.join("obsolete.toniator");
    rewrite_json_member(&project, &obsolete_project, "document.json", |json| {
        json["document_schema_version"] = (DOCUMENT_SCHEMA_VERSION - 1).into();
    });
    let obsolete_project_error = load(&obsolete_project).expect_err("prior project schema rejects");
    assert_eq!(obsolete_project_error.path(), "version");

    let obsolete_preset = directory.join("obsolete.toniator-preset");
    rewrite_json_member(&preset, &obsolete_preset, "preset.json", |json| {
        json["document_schema_version"] = (DOCUMENT_PRESET_CONFIGURATION_SCHEMA_VERSION - 1).into();
    });
    let obsolete_preset_error = load_document_preset(&obsolete_preset, &document)
        .expect_err("prior Preset configuration schema rejects");
    assert_eq!(obsolete_preset_error.path(), "preset.version");

    let invalid_project = directory.join("invalid.toniator");
    rewrite_json_member(&project, &invalid_project, "document.json", |json| {
        json["document"]["appearance_baseline"]["density"] = serde_json::json!(-1.0);
    });
    let invalid_project_error = load(&invalid_project).expect_err("invalid density rejects");
    assert_eq!(invalid_project_error.path(), "document.validation");
    assert!(
        invalid_project_error
            .context()
            .contains("document.appearance_baseline.density")
    );

    let invalid_preset = directory.join("invalid.toniator-preset");
    rewrite_json_member(&preset, &invalid_preset, "preset.json", |json| {
        json["configuration"]["appearance_baseline"]["density"] = serde_json::json!(-1.0);
    });
    let invalid_preset_error = load_document_preset(&invalid_preset, &document)
        .expect_err("invalid Preset density rejects");
    assert_eq!(invalid_preset_error.path(), "document.validation");
    assert!(
        invalid_preset_error
            .context()
            .contains("document.appearance_baseline.density")
    );
    fs::remove_dir_all(directory).expect("persistence witness directory removes");
}
