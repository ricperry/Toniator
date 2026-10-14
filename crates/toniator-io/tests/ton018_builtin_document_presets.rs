//! Current complete built-ins retain exact archive/configuration intent through the shared reader.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::PathBuf};
use toniator_domain::{
    CanvasSpec, Document, DocumentConfiguration, DocumentHistory, DocumentSession, SourceReference,
    SourceReferenceId,
};
use toniator_io::{
    builtin_document_preset, builtin_document_presets, capture_document_preset_destination,
    load_document_preset, save_document_preset,
};

/// Builds a current destination with deliberately unrelated source identity and intrinsic canvas.
///
/// # Panics
/// Panics if the ordinary current default document cannot be constructed.
fn destination(width: f64, height: f64) -> Document {
    Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(SourceReferenceId::new("builtin-destination-artwork").unwrap()),
    )
    .unwrap()
}

/// Checks the exact fourteen reviewed archives and stable metadata, including both Magazine variants.
///
/// # Panics
/// Panics on changed provenance, source payloads, obsolete schemas, duplicate IDs or lost variants.
#[test]
fn catalog_contains_fourteen_exact_current_distinct_archives() {
    let index: Value =
        serde_json::from_str(include_str!("../../../assets/document-presets/index.json")).unwrap();
    let entries = index["presets"].as_array().unwrap();
    let catalog = builtin_document_presets();
    assert_eq!(catalog.len(), 14);
    assert_eq!(entries.len(), catalog.len());
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut digests = BTreeSet::new();
    for (preset, entry) in catalog.iter().zip(entries) {
        assert!(ids.insert(preset.id));
        assert!(names.insert(preset.name));
        assert_eq!(builtin_document_preset(preset.id).unwrap().id, preset.id);
        assert_eq!(entry["id"], preset.id);
        assert_eq!(entry["name"], preset.name);
        let digest = format!("{:x}", Sha256::digest(preset.archive_bytes()));
        assert_eq!(digest, preset.archive_sha256);
        assert_eq!(entry["sha256"], digest);
        assert!(digests.insert(digest));
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(preset.archive_bytes())).unwrap();
        assert_eq!(archive.len(), 1);
        let json: Value = serde_json::from_reader(archive.by_name("preset.json").unwrap()).unwrap();
        assert_eq!(json["kind"], "document_preset");
        assert_eq!(json["document_preset_format_version"], 3);
        assert_eq!(json["document_schema_version"], 16);
    }
    assert!(builtin_document_preset("unknown").is_none());
    let destination = destination(1024.0, 1024.0);
    for model in ["rgb", "cmyk"] {
        let primary = builtin_document_preset(&format!("magazine-{model}"))
            .unwrap()
            .load(&destination)
            .unwrap();
        let alternate = builtin_document_preset(&format!("magazine-{model}-alternate"))
            .unwrap()
            .load(&destination)
            .unwrap();
        assert_ne!(primary, alternate);
    }
    assert_eq!(index["omitted_duplicates"].as_array().unwrap().len(), 1);
}

/// Loads all complete recipes on both intrinsic canvases and keeps source, calibration and one Undo.
///
/// # Panics
/// Panics if active/hidden ALL calibration, destination authority or atomic history changes regress.
#[test]
fn every_builtin_binds_both_canvases_with_active_and_all_calibration() {
    for (width, height) in [(1024.0, 1024.0), (900.0, 620.0)] {
        let destination = destination(width, height);
        for preset in builtin_document_presets() {
            let configuration = preset
                .load(&destination)
                .unwrap_or_else(|error| panic!("{}: {error}", preset.id));
            let bound = configuration.bind(&destination).unwrap();
            assert_eq!(bound.id(), destination.id());
            assert_eq!(bound.source(), destination.source());
            assert_eq!(bound.canvas(), destination.canvas());
            for channel in bound
                .channel_topology()
                .unwrap()
                .channels()
                .iter()
                .map(|channel| channel.id)
            {
                assert!(
                    bound.saved_calibration_for(channel).unwrap().is_some(),
                    "{} active",
                    preset.id
                );
            }
            assert!(
                bound.baseline_saved_calibration().unwrap().is_some(),
                "{} ALL",
                preset.id
            );
            let mut history =
                DocumentHistory::new(DocumentSession::new(destination.clone()).unwrap());
            let before_revision = history.revision();
            history
                .apply_document_configuration(&destination, before_revision, &configuration)
                .unwrap();
            let loaded = history.document().clone();
            history.undo().unwrap();
            assert_eq!(history.document(), &destination);
            let revision = history.revision();
            let unchanged = history
                .apply_document_configuration(
                    &destination,
                    revision,
                    &DocumentConfiguration::capture(&destination),
                )
                .unwrap();
            assert!(unchanged.unchanged);
            assert_eq!(history.revision(), revision);
            assert!(history.can_redo());
            history.redo().unwrap();
            assert_eq!(history.document(), &loaded);
        }
    }
}

/// Saves independent editable personal copies without assigning file identity to immutable built-ins.
/// Writes a unique disposable directory, or a unique run under the requested evidence root.
///
/// # Panics
/// Panics if strict file and embedded loading disagree, save/reopen changes intent, or assets change.
#[test]
fn builtin_save_copies_reopen_exactly_without_changing_catalog_bytes() {
    let output = std::env::var_os("TONIATOR_BUILTIN_PRESET_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("toniator-builtin-document-presets"))
        .join(format!(
            "copies-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    eprintln!("built-in save-copy artifacts: {}", output.display());
    fs::create_dir_all(&output).unwrap();
    let destination = destination(900.0, 620.0);
    for preset in builtin_document_presets() {
        let configuration = preset.load(&destination).unwrap();
        let path = output.join(format!("{}.toniator-preset", preset.id));
        save_document_preset(
            &path,
            &configuration,
            &capture_document_preset_destination(&path).unwrap(),
        )
        .unwrap();
        assert_eq!(
            load_document_preset(&path, &destination).unwrap(),
            configuration
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(preset.archive_bytes())),
            preset.archive_sha256
        );
    }
}
