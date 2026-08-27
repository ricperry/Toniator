use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

use serde_json::Value;
use toniator_domain::{
    CanvasSpec, CoveragePolicy, Document, DocumentCommand, PatternDefinitionDraft,
    PatternDefinitionRecipe, PatternGeometryResponse, PatternStructureRecipe, PresetMetadata,
    PresetRecord, RegionGeometryResponse, RegionResizeAlgorithm, RegionSamplingStrategy,
    SourceReference, SourceReferenceId,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, load_preset, save, save_preset,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Builds one current document with the selected base definition atomically replaced by Region intent.
fn region_document(source_id: SourceReferenceId, response: RegionGeometryResponse) -> Document {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 160.0,
            height: 120.0,
        },
        SourceReference::Assigned(source_id),
    )
    .expect("default document");
    let mut recipe = PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
        PatternDefinitionDraft {
            name: "persisted typed regions".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
        },
    ));
    recipe.output_settings[0].response = PatternGeometryResponse::Regions(response);
    document
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: document.pattern_settings().clone(),
            base_definition: document.pattern_definition_bundles()[0].definition.clone(),
            recipe,
        })
        .expect("region recipe installs")
        .0
}

/// Builds the compact source bundle used by every current-format archive witness.
fn region_sources(source_id: SourceReferenceId) -> SourceBundle {
    SourceBundle::new([EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Png,
        vec![137, 80, 78, 71],
        Some("minimal-payload.png".into()),
    )
    .expect("embedded source")])
    .expect("source bundle")
}

/// Rewrites the document JSON only while preserving every source entry and deterministic ZIP metadata.
fn rewrite_document_json(path: &Path, mutate: impl FnOnce(&mut Value)) {
    let mut archive =
        ZipArchive::new(fs::File::open(path).expect("archive opens")).expect("archive is ZIP");
    let mut entries = Vec::new();
    let mut mutate = Some(mutate);
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("archive entry");
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("archive entry reads");
        if name == "document.json" {
            let mut root: Value = serde_json::from_slice(&bytes).expect("document JSON parses");
            mutate.take().expect("document JSON is present")(&mut root);
            bytes = serde_json::to_vec(&root).expect("document JSON serializes");
        }
        entries.push((name, bytes));
    }
    drop(archive);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o100644);
    let temporary = path.with_extension("toniator.stage20s-rewrite");
    let mut writer = ZipWriter::new(fs::File::create(&temporary).expect("rewrite archive creates"));
    for (name, bytes) in entries {
        writer
            .start_file(name, options)
            .expect("archive entry starts");
        writer.write_all(&bytes).expect("archive entry writes");
    }
    writer.finish().expect("rewrite archive finishes");
    fs::rename(temporary, path).expect("rewrite archive replaces");
}

/// Builds a standalone current-v3 region preset with an explicit positive-geometry response.
fn region_preset() -> PresetRecord {
    let mut recipe = PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
        PatternDefinitionDraft {
            name: "preset regions".into(),
            coverage: CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        },
    ));
    recipe.output_settings[0].response = PatternGeometryResponse::Regions(RegionGeometryResponse {
        algorithm: RegionResizeAlgorithm::UniformOffset,
        sampling: RegionSamplingStrategy::AreaAverage,
        minimum_fill: 0.25,
        maximum_fill: 1.5,
    });
    PresetRecord {
        metadata: PresetMetadata {
            id: "stage20o".into(),
            name: "Stage 20O".into(),
            category: "test".into(),
            description: "ordinary regions".into(),
            thumbnail: None,
        },
        recipe,
    }
}

/// Proves v5 and v3 round trips retain only algorithm, sampling, and shared fill endpoints.
#[test]
fn current_v5_and_v3_round_trip_positive_region_intent() {
    let source_id = SourceReferenceId::new("stage20o-persistence").expect("source ID");
    let response = RegionGeometryResponse {
        algorithm: RegionResizeAlgorithm::UniformOffset,
        sampling: RegionSamplingStrategy::AreaAverage,
        minimum_fill: 0.25,
        maximum_fill: 1.5,
    };
    let document = region_document(source_id.clone(), response.clone());
    let document_path = std::env::temp_dir().join(format!(
        "toniator-stage20o-v5-{}.toniator",
        std::process::id()
    ));
    save(&document_path, &document, &region_sources(source_id)).expect("v5 document saves");
    let loaded = load(&document_path).expect("v5 document loads");
    assert_eq!(loaded.versions().document(), 5);
    assert_eq!(loaded.document(), &document);
    fs::remove_file(&document_path).expect("temporary document removes");

    let preset = region_preset();
    let preset_path = std::env::temp_dir().join(format!(
        "toniator-stage20o-v3-{}.preset.json",
        std::process::id()
    ));
    save_preset(&preset_path, &preset).expect("v3 preset saves");
    let text = fs::read_to_string(&preset_path).expect("v3 preset reads");
    assert!(text.contains("\"algorithm\": \"uniform_offset\""));
    assert!(text.contains("\"minimum_fill\": 0.25"));
    assert_eq!(load_preset(&preset_path).expect("v3 preset loads"), preset);
    fs::remove_file(preset_path).expect("temporary preset removes");
}

/// Proves current v5 rejects each obsolete region tag and endpoint field without migration.
#[test]
fn v5_rejects_obsolete_region_treatments_and_endpoint_fields() {
    let source_id = SourceReferenceId::new("stage20o-obsolete").expect("source ID");
    let document = region_document(source_id.clone(), RegionGeometryResponse::default());
    let path = std::env::temp_dir().join(format!(
        "toniator-stage20o-obsolete-{}.toniator",
        std::process::id()
    ));
    let save_valid = || save(&path, &document, &region_sources(source_id.clone()));
    for (field, value) in [
        ("treatment", Value::from("full")),
        ("minimum_scale", Value::from(0.5)),
        ("maximum_scale", Value::from(1.0)),
        ("minimum_gap", Value::from(1.0)),
        ("maximum_gap", Value::from(2.0)),
    ] {
        save_valid().expect("valid v5 archive saves");
        rewrite_document_json(&path, |root| {
            let bundles = root["document"]["pattern_definition_bundles"]
                .as_array_mut()
                .expect("bundles");
            let response = bundles
                .iter_mut()
                .find(|bundle| {
                    bundle["definition"]["output_layers"][0]["realization"]["kind"] == "regions"
                })
                .expect("region bundle")["output_settings"][0]["response"]["response"]
                .as_object_mut()
                .expect("region response");
            response.insert(field.to_owned(), value);
        });
        assert!(load(&path).is_err(), "obsolete {field} rejects");
    }
    fs::remove_file(path).expect("temporary document removes");
}

/// Proves current v3 rejects obsolete treatment tags and endpoint fields without migration.
#[test]
fn v3_rejects_obsolete_region_treatments_and_endpoint_fields() {
    let path = std::env::temp_dir().join(format!(
        "toniator-stage20o-obsolete-{}.preset.json",
        std::process::id()
    ));
    let preset = region_preset();
    for (field, value) in [
        ("treatment", Value::from("constant_gap")),
        ("minimum_scale", Value::from(0.5)),
        ("maximum_scale", Value::from(1.0)),
        ("minimum_gap", Value::from(1.0)),
        ("maximum_gap", Value::from(2.0)),
    ] {
        save_preset(&path, &preset).expect("valid v3 preset saves");
        let mut root: Value = serde_json::from_slice(&fs::read(&path).expect("preset reads"))
            .expect("preset JSON parses");
        root["recipe"]["output_settings"][0]["response"]["response"]
            .as_object_mut()
            .expect("region response")
            .insert(field.to_owned(), value);
        fs::write(
            &path,
            serde_json::to_vec(&root).expect("preset JSON serializes"),
        )
        .expect("obsolete preset writes");
        assert!(load_preset(&path).is_err(), "obsolete {field} rejects");
    }
    fs::remove_file(path).expect("temporary preset removes");
}
