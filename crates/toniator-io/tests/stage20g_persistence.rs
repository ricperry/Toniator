use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use toniator_domain::{
    CanvasSpec, ChannelGeometryResponseDelta, ChannelId, DensityMetricDelta2D, Document,
    DocumentCommand, DocumentHistory, DocumentSession, MarkGeometryResponseDelta,
    PatternDefinitionDraft, PatternDefinitionRecipe, PresetMetadata, PresetRecord, SourceReference,
    SourceReferenceId,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, LoadError, PRESET_FORMAT_VERSION, SourceBundle, load,
    load_preset, save, save_preset,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Returns a collision-resistant temporary path for one bounded persistence witness.
fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "toniator-stage20g-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time follows epoch")
            .as_nanos()
    ))
}

/// Builds one source-backed current document and its exact one-source bundle.
fn source_backed_document() -> (Document, SourceBundle) {
    let id = SourceReferenceId::new("source-1").expect("valid source ID");
    let document = Document::new_default_document(
        CanvasSpec {
            width: 90.0,
            height: 60.0,
        },
        SourceReference::Assigned(id.clone()),
    )
    .expect("default document is valid");
    let sources = SourceBundle::new([EmbeddedSource::new(
        id,
        EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec(),
        Some("source.svg".into()),
    )
    .expect("embedded source is valid")])
    .expect("source bundle is valid");
    (document, sources)
}

/// Reads the canonical document JSON entry without interpreting derived domain state.
fn document_json(path: &Path) -> String {
    let file = File::open(path).expect("container opens");
    let mut archive = ZipArchive::new(file).expect("container is a ZIP archive");
    let mut text = String::new();
    archive
        .by_name("document.json")
        .expect("document entry exists")
        .read_to_string(&mut text)
        .expect("document entry is UTF-8");
    text
}

/// Rewrites only the schema discriminator of a derived test container.
fn rewrite_schema_version(source: &Path, destination: &Path, version: u32) {
    let mut archive = ZipArchive::new(File::open(source).expect("source container opens"))
        .expect("source container is ZIP");
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("archive entry opens");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("archive entry reads");
        if entry.name() == "document.json" {
            let text = String::from_utf8(bytes).expect("document JSON is UTF-8");
            bytes = text
                .replacen(
                    "\"document_schema_version\":7",
                    &format!("\"document_schema_version\":{version}"),
                    1,
                )
                .into_bytes();
        }
        entries.push((entry.name().to_owned(), bytes));
    }
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o100644);
    let mut writer = ZipWriter::new(File::create(destination).expect("derived container creates"));
    for (name, bytes) in entries {
        writer.start_file(name, options).expect("entry starts");
        writer.write_all(&bytes).expect("entry writes");
    }
    writer.finish().expect("derived container finishes");
}

/// Ports one tracked current-format fixture from the intentional v4 authority shape to v5.
///
/// This test-only one-shot port preserves embedded source bytes and structural definition order.
/// Production IO deliberately has no v4 adapter; after the port, normal v5 loading validates the
/// converted fixture through the current authoritative DTO boundary.
#[test]
#[ignore = "one-shot current-fixture port; run only before replacing tracked v4 fixtures"]
fn port_tracked_v4_fixtures_to_v5() {
    for fixture in [
        "HolidayMugs_2024_2025.toniator",
        "raster-sample.toniator",
        "vector-sample.toniator",
    ] {
        port_v4_fixture_to_v5(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets")
                .join(fixture),
        );
    }
}

/// Ports tracked current-v5 fixtures to normalized Stage 20R records with explicit `All`.
#[test]
#[ignore = "one-shot current-v5 normalized-output port"]
fn port_tracked_v5_fixtures_to_explicit_all_filters() {
    for fixture in [
        "HolidayMugs_2024_2025.toniator",
        "raster-sample.toniator",
        "vector-sample.toniator",
    ] {
        port_v5_fixture_to_explicit_all_filter(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets")
                .join(fixture),
        );
    }
}

/// Rewrites only current output records that predate the normalized realization/filter fields.
fn port_v5_fixture_to_explicit_all_filter(path: &Path) {
    let mut archive = ZipArchive::new(File::open(path).expect("v5 fixture archive opens"))
        .expect("v5 fixture is a ZIP archive");
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("fixture entry opens");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("fixture entry reads");
        if entry.name() == "document.json" {
            let mut root: Value = serde_json::from_slice(&bytes).expect("fixture document is JSON");
            assert_eq!(root["document_schema_version"], Value::from(5_u64));
            let bundles = root["document"]["pattern_definition_bundles"]
                .as_array_mut()
                .expect("v5 fixture carries definition bundles");
            for bundle in bundles {
                let outputs = bundle["definition"]["output_layers"]
                    .as_array_mut()
                    .expect("v5 fixture carries output layers");
                for output in outputs {
                    let object = output.as_object_mut().expect("output is an object");
                    object
                        .entry("source_filter")
                        .or_insert_with(|| serde_json::json!({ "kind": "all" }));
                    if !object.contains_key("realization") {
                        let id = object.remove("id").expect("legacy output has an ID");
                        let source_filter = object
                            .remove("source_filter")
                            .expect("port installed an explicit filter");
                        let realization = Value::Object(std::mem::take(object));
                        *output = serde_json::json!({
                            "id": id,
                            "source_filter": source_filter,
                            "realization": realization,
                        });
                    }
                }
            }
            bytes = serde_json::to_vec(&root).expect("explicit-filter fixture JSON serializes");
        }
        entries.push((entry.name().to_owned(), bytes));
    }
    drop(archive);
    let temporary = path.with_extension("toniator.stage20r-filter-port");
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o100644);
    let mut writer = ZipWriter::new(File::create(&temporary).expect("port archive creates"));
    for (name, bytes) in entries {
        writer.start_file(name, options).expect("entry starts");
        writer.write_all(&bytes).expect("entry writes");
    }
    writer.finish().expect("port archive finishes");
    fs::rename(temporary, path).expect("port atomically replaces fixture");
}

/// Rewrites a known v4 fixture archive as v5 keyed bundle authority without interpreting it as a document.
fn port_v4_fixture_to_v5(path: &Path) {
    let mut archive = ZipArchive::new(File::open(path).expect("v4 fixture archive opens"))
        .expect("v4 fixture is a ZIP archive");
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("fixture entry opens");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("fixture entry reads");
        if entry.name() == "document.json" {
            let mut root: Value = serde_json::from_slice(&bytes).expect("fixture document is JSON");
            root["document_schema_version"] = Value::from(5_u64);
            let document = root["document"]
                .as_object_mut()
                .expect("fixture document object");
            let response = document
                .get("pattern_settings")
                .and_then(Value::as_object)
                .and_then(|settings| settings.get("geometry_response"))
                .cloned()
                .expect("v4 fixture carries base response");
            let definitions = document
                .remove("pattern_definitions")
                .and_then(|value| value.as_array().cloned())
                .expect("v4 fixture carries definitions");
            let bundles = definitions
                .into_iter()
                .map(|definition| {
                    let settings = definition["output_layers"]
                        .as_array()
                        .expect("fixture output layers")
                        .iter()
                        .map(|output| {
                            serde_json::json!({
                                "output_layer_id": output["id"].clone(),
                                "response": response.clone(),
                            })
                        })
                        .collect::<Vec<_>>();
                    serde_json::json!({
                        "definition": definition,
                        "output_settings": settings,
                    })
                })
                .collect::<Vec<_>>();
            document.insert("pattern_definition_bundles".into(), Value::Array(bundles));
            document
                .get_mut("pattern_settings")
                .and_then(Value::as_object_mut)
                .expect("fixture settings object")
                .remove("geometry_response");
            bytes = serde_json::to_vec(&root).expect("v5 fixture JSON serializes");
        }
        entries.push((entry.name().to_owned(), bytes));
    }
    drop(archive);
    let temporary = path.with_extension("toniator.stage20n-port");
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o100644);
    let mut writer = ZipWriter::new(File::create(&temporary).expect("port archive creates"));
    for (name, bytes) in entries {
        writer.start_file(name, options).expect("port entry starts");
        writer.write_all(&bytes).expect("port entry writes");
    }
    writer.finish().expect("port archive finishes");
    fs::rename(temporary, path).expect("port atomically replaces fixture");
}

/// Loads the tracked sample containers as strict current-v7 fixtures with exact source bytes.
#[test]
fn tracked_sample_fixtures_are_current_v7_with_exact_sources() {
    for (fixture, source, format) in [
        (
            "raster-sample.toniator",
            "raster-sample.png",
            EmbeddedSourceFormat::Png,
        ),
        (
            "vector-sample.toniator",
            "vector-sample.svg",
            EmbeddedSourceFormat::Svg,
        ),
    ] {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let loaded = load(&assets.join(fixture)).expect("tracked v7 fixture loads");
        let embedded = loaded
            .sources()
            .entries()
            .next()
            .expect("tracked fixture embeds one source");
        assert_eq!(loaded.sources().len(), 1);
        assert_eq!(embedded.format(), format);
        assert_eq!(embedded.bytes(), fs::read(assets.join(source)).unwrap());
    }
}

/// Reports the obsolete Holiday fixture through the same strict version boundary.
#[test]
fn holiday_v5_fixture_is_not_migrated_implicitly() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/HolidayMugs_2024_2025.toniator");
    let error = load(&path).expect_err("Holiday v5 fixture rejects");
    assert!(matches!(error, LoadError::Version { .. }));
}

/// Saves deterministic v7 density/aspect deltas and omits every effective projection.
#[test]
fn v7_save_is_deterministic_and_serializes_only_base_plus_authored_deltas() {
    let (document, sources) = source_backed_document();
    let document = document
        .apply_command(&DocumentCommand::SetChannelDensityDelta {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(1),
            density: DensityMetricDelta2D {
                density_delta: 0.0,
                aspect_delta: 0.0,
            },
        })
        .expect("explicit zero density applies")
        .0;
    let document = document
        .apply_command(&DocumentCommand::SetChannelPatternRotationDelta {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(1),
            rotation_degrees: 0.0,
        })
        .expect("explicit zero rotation applies")
        .0;
    let document = document
        .apply_command(&DocumentCommand::SetChannelOutputResponseDelta {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(1),
            output_layer_id: document.pattern_definition_bundles()[0]
                .definition
                .output_layers[0]
                .id(),
            delta: ChannelGeometryResponseDelta::Marks(MarkGeometryResponseDelta {
                minimum_fill_delta: Some(0.0),
                maximum_fill_delta: None,
            }),
        })
        .expect("partial explicit response applies")
        .0;
    let first = temporary("deterministic-a.toniator");
    let second = temporary("deterministic-b.toniator");
    save(&first, &document, &sources).expect("first save succeeds");
    save(&second, &document, &sources).expect("second save succeeds");
    assert_eq!(
        fs::read(&first).expect("first reads"),
        fs::read(&second).expect("second reads")
    );
    let text = document_json(&first);
    assert!(text.contains("\"document_schema_version\":7"));
    assert!(text.contains("\"pattern_settings\""));
    assert!(text.contains("\"pattern_instance\""));
    assert!(!text.contains("EffectiveChannelPatternInstance"));
    assert!(!text.contains("effective_channel_pattern"));
    let loaded = load(&first).expect("v7 reloads");
    assert_eq!(loaded.document(), &document);
    let instance = loaded
        .document()
        .channel_pattern_instance(ChannelId(1))
        .expect("explicit intent reloads");
    assert_eq!(
        instance
            .layout_delta
            .density
            .as_ref()
            .expect("density delta")
            .density_delta,
        0.0
    );
    assert_eq!(instance.layout_delta.rotation_degrees, Some(0.0));
    let ChannelGeometryResponseDelta::Marks(delta) = &instance
        .output_response_deltas
        .first()
        .expect("response delta")
        .delta
    else {
        panic!("mark fixture reloads mark response delta");
    };
    assert_eq!(delta.minimum_fill_delta, Some(0.0));
    assert_eq!(delta.maximum_fill_delta, None);
    fs::remove_file(first).expect("first temporary removes");
    fs::remove_file(second).expect("second temporary removes");
}

/// Rejects document schemas one through six without a migration or fallback decoder.
#[test]
fn document_versions_one_through_six_are_rejected() {
    let (document, sources) = source_backed_document();
    let current = temporary("current.toniator");
    save(&current, &document, &sources).expect("current save succeeds");
    for version in 1..=6 {
        let stale = temporary(&format!("schema-{version}.toniator"));
        rewrite_schema_version(&current, &stale, version);
        let error = load(&stale).expect_err("obsolete schema rejects");
        assert!(matches!(error, LoadError::Version { .. }));
        assert!(
            error
                .to_string()
                .contains(&format!("schema version {version}"))
        );
        fs::remove_file(stale).expect("stale temporary removes");
    }
    fs::remove_file(current).expect("current temporary removes");
}

/// Keeps preset v4 deterministic and materializes the same ID-free recipe at either authority scope.
#[test]
fn preset_v4_bytes_reconstruct_document_base_or_channel_override() {
    assert_eq!(PRESET_FORMAT_VERSION, 4);
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "stage20g-grid".into(),
            name: "Stage 20G Grid".into(),
            category: "Test".into(),
            description: "One ID-free recipe for both pattern scopes.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::marks(
            toniator_domain::PatternStructureRecipe::StraightGrid(PatternDefinitionDraft {
                name: "Materialized grid".into(),
                coverage: toniator_domain::CoveragePolicy {
                    guard_steps: 3,
                    additional_margin: 1.25,
                },
            }),
        ),
    };
    let first = temporary("preset-a.json");
    let second = temporary("preset-b.json");
    save_preset(&first, &preset).expect("first preset saves");
    save_preset(&second, &preset).expect("second preset saves");
    let bytes = fs::read(&first).expect("preset reads");
    assert_eq!(bytes, fs::read(&second).expect("second preset reads"));
    assert_eq!(load_preset(&first).expect("preset reloads"), preset);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).expect("current preset JSON")["preset_format_version"],
        serde_json::json!(4)
    );
    let obsolete = temporary("preset-v3.json");
    let mut obsolete_json: serde_json::Value = serde_json::from_slice(&bytes).expect("preset JSON");
    obsolete_json["preset_format_version"] = serde_json::json!(3);
    fs::write(
        &obsolete,
        serde_json::to_vec(&obsolete_json).expect("obsolete preset JSON"),
    )
    .expect("obsolete preset writes");
    assert!(
        load_preset(&obsolete)
            .expect_err("preset v3 rejects")
            .context()
            .contains("unsupported preset format version 3")
    );
    fs::remove_file(obsolete).expect("obsolete preset removes");

    let (document, _) = source_backed_document();
    let base_definition = document.pattern_definition_bundles()[0].definition.clone();
    let mut base_history =
        DocumentHistory::new(DocumentSession::new(document.clone()).expect("valid base session"));
    base_history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: document.pattern_settings().clone(),
            base_definition: base_definition.clone(),
            recipe: preset.recipe.clone(),
        })
        .expect("base recipe materializes");
    assert_eq!(
        base_history.document().pattern_settings().definition_id.0,
        2
    );
    assert_eq!(
        base_history
            .document()
            .effective_channel_pattern(ChannelId(2))
            .expect("base applies to channel")
            .definition_id
            .0,
        2
    );

    let mut override_history =
        DocumentHistory::new(DocumentSession::new(document).expect("valid override session"));
    override_history
        .apply(
            &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base: override_history.document().pattern_settings().clone(),
                channel_id: ChannelId(2),
                base_definition,
                recipe: preset.recipe.clone(),
            },
        )
        .expect("override recipe materializes");
    assert_eq!(
        override_history
            .document()
            .pattern_settings()
            .definition_id
            .0,
        1
    );
    assert_eq!(
        override_history
            .document()
            .effective_channel_pattern(ChannelId(2))
            .expect("override applies to selected channel")
            .definition_id
            .0,
        2
    );
    assert_eq!(
        override_history
            .document()
            .effective_channel_pattern(ChannelId(1))
            .expect("other channel inherits base")
            .definition_id
            .0,
        1
    );
    fs::remove_file(first).expect("first preset removes");
    fs::remove_file(second).expect("second preset removes");
}
