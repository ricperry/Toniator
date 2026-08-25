use std::{fs, io::Write};

use toniator_domain::{
    CanvasSpec, CoveragePolicy, Document, DocumentCommand, DocumentHistory, DocumentSession,
    PatternDefinitionDraft, PatternDefinitionRecipe, PatternGeometryResponse,
    PatternStructureRecipe, PresetMetadata, PresetRecord, SourceReference, SourceReferenceId,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, load_preset, save, save_preset,
};

/// Builds one current document with the selected base definition atomically replaced by Regions intent.
fn region_document(source_id: SourceReferenceId) -> Document {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 160.0,
            height: 120.0,
        },
        SourceReference::Assigned(source_id),
    )
    .expect("default document");
    let mut history = DocumentHistory::new(DocumentSession::new(document).expect("session"));
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("base definition")
        .definition
        .clone();
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
                PatternDefinitionDraft {
                    name: "persisted regions".into(),
                    coverage: CoveragePolicy {
                        guard_steps: 2,
                        additional_margin: 0.0,
                    },
                },
            )),
        })
        .expect("region recipe installs");
    history.document().clone()
}

/// Proves preset-v3 persists only fixed ordinary-region intent and reconstructs its recipe exactly.
#[test]
fn preset_v3_round_trips_ordinary_region_intent() {
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "stage20o".into(),
            name: "Stage 20O".into(),
            category: "test".into(),
            description: "ordinary regions".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
            PatternDefinitionDraft {
                name: "grid".into(),
                coverage: CoveragePolicy {
                    guard_steps: 1,
                    additional_margin: 0.0,
                },
            },
        )),
    };
    let path = std::env::temp_dir().join(format!("toniator-stage20o-{}.json", std::process::id()));
    save_preset(&path, &preset).expect("current v3 preset saves");
    let loaded = load_preset(&path).expect("current v3 preset loads");
    fs::remove_file(path).expect("temporary preset removes");
    assert_eq!(loaded, preset);
}

/// Proves v5 documents persist only ordinary-region intent and deterministically reconstruct bundles.
#[test]
fn document_v5_round_trips_ordinary_region_intent_without_derived_state() {
    let source_id = SourceReferenceId::new("stage20o-persistence").expect("source ID");
    let document = region_document(source_id.clone());
    let sources = SourceBundle::new([EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Png,
        vec![137, 80, 78, 71],
        Some("minimal-payload.png".into()),
    )
    .expect("embedded source")])
    .expect("source bundle");
    let path = std::env::temp_dir().join(format!(
        "toniator-stage20o-doc-{}.toniator",
        std::process::id()
    ));
    save(&path, &document, &sources).expect("v5 document saves");
    let loaded = load(&path).expect("v5 document loads");
    fs::remove_file(path).expect("temporary document removes");
    assert_eq!(loaded.versions().document(), 5);
    assert_eq!(loaded.document(), &document);
    let selected = loaded
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == loaded.document().pattern_settings().definition_id)
        .expect("selected bundle");
    assert!(matches!(
        selected.output_settings[0].response,
        PatternGeometryResponse::Regions(_)
    ));
}

/// Proves current-only persistence rejects obsolete document and preset format envelopes without adapters.
#[test]
fn obsolete_document_and_preset_formats_reject() {
    let root = std::env::temp_dir().join(format!("toniator-stage20o-old-{}", std::process::id()));
    let document_path = root.with_extension("toniator");
    let file = fs::File::create(&document_path).expect("obsolete archive");
    let mut archive = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    archive
        .start_file("document.json", options)
        .expect("document entry");
    archive
        .write_all(b"{\"container_version\":1,\"document_schema_version\":4}")
        .expect("old envelope writes");
    archive
        .start_file("sources/x.png", options)
        .expect("source entry");
    archive.write_all(&[0]).expect("source writes");
    archive.finish().expect("obsolete archive finishes");
    assert!(
        load(&document_path)
            .expect_err("obsolete document rejects")
            .to_string()
            .contains("unsupported document schema version 4")
    );
    fs::remove_file(document_path).expect("obsolete document removes");
    let preset_path = root.with_extension("json");
    let current_preset = PresetRecord {
        metadata: PresetMetadata {
            id: "old-format".into(),
            name: "Old format".into(),
            category: "test".into(),
            description: "format witness".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
            PatternDefinitionDraft {
                name: "regions".into(),
                coverage: CoveragePolicy {
                    guard_steps: 1,
                    additional_margin: 0.0,
                },
            },
        )),
    };
    save_preset(&preset_path, &current_preset).expect("current preset saves");
    let current_bytes = fs::read_to_string(&preset_path).expect("current preset reads");
    let obsolete_bytes = current_bytes.replace(
        "\"preset_format_version\": 3",
        "\"preset_format_version\": 2",
    );
    assert_ne!(
        obsolete_bytes, current_bytes,
        "current preset contains its version"
    );
    fs::write(&preset_path, obsolete_bytes).expect("old preset writes");
    assert!(
        load_preset(&preset_path)
            .expect_err("obsolete preset rejects")
            .to_string()
            .contains("unsupported preset format version 2")
    );
    fs::remove_file(preset_path).expect("obsolete preset removes");
}
