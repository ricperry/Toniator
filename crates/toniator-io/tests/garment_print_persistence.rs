use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};
use toniator_domain::{DocumentConfiguration, PhysicalPrintSizeMm, PrintPreparationSettings};
use toniator_io::{
    DOCUMENT_SCHEMA_VERSION, capture_document_preset_destination, load, load_document_preset, save,
    save_document_preset,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

const SCHEMA10_BYTES: &[u8] = include_bytes!("fixtures/garment-schema10-clean-fade.toniator");

/// Makes one unique temporary directory for isolated archive mutations.
fn directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "toniator-garment-io-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    path
}

/// Reads just the document JSON while retaining the original source members.
fn json(path: &Path) -> Value {
    let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
    let mut bytes = Vec::new();
    archive
        .by_name("document.json")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Repackages a project with one changed JSON member and unchanged encoded source members.
fn mutate_document(input: &Path, output: &Path, replacement: &Value) {
    mutate_document_bytes(input, output, &serde_json::to_vec(replacement).unwrap());
}

/// Repackages one exact JSON byte stream to exercise malformed-number rejection.
fn mutate_document_bytes(input: &Path, output: &Path, replacement: &[u8]) {
    let mut reader = ZipArchive::new(File::open(input).unwrap()).unwrap();
    let mut writer = ZipWriter::new(File::create(output).unwrap());
    for index in 0..reader.len() {
        let mut member = reader.by_index(index).unwrap();
        let name = member.name().to_owned();
        let options = SimpleFileOptions::default().compression_method(member.compression());
        if member.is_dir() {
            writer.add_directory(name, options).unwrap();
        } else {
            writer.start_file(name.clone(), options).unwrap();
            if name == "document.json" {
                writer.write_all(replacement).unwrap();
            } else {
                std::io::copy(&mut member, &mut writer).unwrap();
            }
        }
    }
    writer.finish().unwrap();
}

/// Proves pre-change schema 10 loads with default intent and writes required schema 11 intent.
#[test]
fn schema10_fixture_loads_and_nondefault_schema11_roundtrips() {
    let dir = directory();
    let old_path = dir.join("old.toniator");
    fs::write(&old_path, SCHEMA10_BYTES).unwrap();
    let old = load(&old_path).unwrap();
    assert_eq!(old.versions().document(), 10);
    assert_eq!(
        old.document().print_preparation(),
        &PrintPreparationSettings::default()
    );
    let original_source: Vec<_> = old
        .sources()
        .entries()
        .map(|source| source.bytes().to_vec())
        .collect();
    let settings = PrintPreparationSettings::new(
        Some(PhysicalPrintSizeMm::new(101.6, 50.8).unwrap()),
        0.8,
        0.2,
    )
    .unwrap();
    let configured = old
        .document()
        .clone()
        .with_print_preparation(settings.clone())
        .unwrap();
    let new_path = dir.join("new.toniator");
    save(&new_path, &configured, old.sources()).unwrap();
    let root = json(&new_path);
    assert_eq!(root["document_schema_version"], DOCUMENT_SCHEMA_VERSION);
    assert_eq!(root["container_version"], 2);
    assert_eq!(
        root["document"]["print_preparation"]["physical_size_mm"]["width_mm"],
        101.6
    );
    let reloaded = load(&new_path).unwrap();
    assert_eq!(reloaded.versions().document(), 11);
    assert_eq!(reloaded.document(), &configured);
    assert_eq!(
        reloaded
            .sources()
            .entries()
            .map(|source| source.bytes().to_vec())
            .collect::<Vec<_>>(),
        original_source
    );
    fs::remove_dir_all(dir).unwrap();
}

/// Rejects incomplete, hybrid, unknown, and malformed print fields transactionally.
#[test]
fn schema11_requires_complete_strict_validated_intent() {
    let dir = directory();
    let old_path = dir.join("old.toniator");
    fs::write(&old_path, SCHEMA10_BYTES).unwrap();
    let old = load(&old_path).unwrap();
    let current_path = dir.join("current.toniator");
    save(&current_path, old.document(), old.sources()).unwrap();
    let baseline = json(&current_path);
    let mut cases: Vec<(&str, Value)> = Vec::new();
    let mut missing = baseline.clone();
    missing["document"]
        .as_object_mut()
        .unwrap()
        .remove("print_preparation");
    cases.push(("missing-settings", missing));
    let mut missing_size = baseline.clone();
    missing_size["document"]["print_preparation"]
        .as_object_mut()
        .unwrap()
        .remove("physical_size_mm");
    cases.push(("missing-size", missing_size));
    let mut missing_threshold = baseline.clone();
    missing_threshold["document"]["print_preparation"]
        .as_object_mut()
        .unwrap()
        .remove("minimum_negative_gap_width_mm");
    cases.push(("missing-threshold", missing_threshold));
    let mut half = baseline.clone();
    half["document"]["print_preparation"]["physical_size_mm"] = json!({"width_mm": 100.0});
    cases.push(("half-placement", half));
    let mut negative = baseline.clone();
    negative["document"]["print_preparation"]["minimum_positive_feature_width_mm"] = json!(-0.1);
    cases.push(("negative", negative));
    let mut wrong_type = baseline.clone();
    wrong_type["document"]["print_preparation"]["minimum_negative_gap_width_mm"] =
        json!("Infinity");
    cases.push(("wrong-type-infinity", wrong_type));
    let mut wrong_type_nan = baseline.clone();
    wrong_type_nan["document"]["print_preparation"]["minimum_positive_feature_width_mm"] =
        json!("NaN");
    cases.push(("wrong-type-nan", wrong_type_nan));
    let mut zero_size = baseline.clone();
    zero_size["document"]["print_preparation"]["physical_size_mm"] =
        json!({"width_mm": 0.0, "height_mm": 40.0});
    cases.push(("zero-size", zero_size));
    let mut extra = baseline.clone();
    extra["document"]["print_preparation"]["unrecognized"] = json!(1);
    cases.push(("unknown", extra));
    let mut future = baseline.clone();
    future["document_schema_version"] = json!(12);
    cases.push(("future", future));
    let mut obsolete = baseline.clone();
    obsolete["document_schema_version"] = json!(9);
    cases.push(("obsolete", obsolete));
    for (name, candidate) in cases {
        let path = dir.join(format!("{name}.toniator"));
        mutate_document(&current_path, &path, &candidate);
        assert!(load(&path).is_err(), "{name} should fail");
    }
    let serialized = serde_json::to_string(&baseline).unwrap();
    let needle = "\"minimum_negative_gap_width_mm\":0.0";
    assert!(serialized.contains(needle));
    for (name, token) in [("numeric-overflow", "1e309"), ("invalid-nan-token", "NaN")] {
        let bytes = serialized.replace(
            needle,
            &format!("\"minimum_negative_gap_width_mm\":{token}"),
        );
        let path = dir.join(format!("{name}.toniator"));
        mutate_document_bytes(&current_path, &path, bytes.as_bytes());
        assert!(load(&path).is_err(), "{name} should fail");
    }
    let mut hybrid = json(&old_path);
    hybrid["document"]["print_preparation"] = baseline["document"]["print_preparation"].clone();
    let path = dir.join("hybrid.toniator");
    mutate_document(&old_path, &path, &hybrid);
    assert!(load(&path).is_err());
    assert_eq!(load(&current_path).unwrap().document(), old.document());
    fs::remove_dir_all(dir).unwrap();
}

/// Keeps nondefault destination intent through source-free and project-as-preset binding.
#[test]
fn both_document_preset_routes_preserve_destination_print_intent() {
    let dir = directory();
    let project = dir.join("source.toniator");
    fs::write(&project, SCHEMA10_BYTES).unwrap();
    let loaded = load(&project).unwrap();
    let intent = PrintPreparationSettings::new(
        Some(PhysicalPrintSizeMm::new(100.0, 40.0).unwrap()),
        0.6,
        0.3,
    )
    .unwrap();
    let destination = loaded
        .document()
        .clone()
        .with_print_preparation(intent.clone())
        .unwrap();
    let configuration = DocumentConfiguration::capture(loaded.document());
    let preset = dir.join("source.toniator-preset");
    let guard = capture_document_preset_destination(&preset).unwrap();
    save_document_preset(&preset, &configuration, &guard).unwrap();
    for path in [&preset, &project] {
        let bound = load_document_preset(path, &destination)
            .unwrap()
            .bind(&destination)
            .unwrap();
        assert_eq!(bound.print_preparation(), &intent);
    }
    fs::remove_dir_all(dir).unwrap();
}
