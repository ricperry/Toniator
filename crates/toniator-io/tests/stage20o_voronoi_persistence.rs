use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

use serde_json::Value;
use toniator_domain::{
    CanvasSpec, ChannelId, CoveragePolicy, Document, DocumentCommand, DocumentHistory,
    DocumentSession, PatternDefinitionDraft, PatternDefinitionRecipe, PatternGeometryResponse,
    PatternStructureRecipe, PresetMetadata, PresetRecord, RegionGeometryFieldEdit,
    RegionGeometryResponse, RegionSamplingStrategy, SourceReference, SourceReferenceId,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, load_preset, save, save_preset,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

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

/// Builds one current ordinary-region document with an explicit Stage 20Q base response.
fn region_document_with_response(
    source_id: SourceReferenceId,
    response: RegionGeometryResponse,
) -> Document {
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
    let base = document.pattern_settings().clone();
    let base_definition = document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("base definition")
        .definition
        .clone();
    document
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe,
        })
        .expect("typed region recipe installs")
        .0
}

/// Builds the compact source bundle used by every current-format Stage 20Q archive witness.
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

/// Reads only the persisted JSON authority entry from one derived archive.
fn document_json(path: &Path) -> String {
    let file = fs::File::open(path).expect("archive opens");
    let mut archive = ZipArchive::new(file).expect("archive is ZIP");
    let mut json = String::new();
    archive
        .by_name("document.json")
        .expect("document entry")
        .read_to_string(&mut json)
        .expect("document JSON reads");
    json
}

/// Rewrites a test archive's JSON only, preserving every source entry and deterministic ZIP metadata.
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
            mutate
                .take()
                .expect("document JSON is present exactly once")(&mut root);
            bytes = serde_json::to_vec(&root).expect("document JSON serializes");
        }
        entries.push((name, bytes));
    }
    drop(archive);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o100644);
    let temporary = path.with_extension("toniator.stage20q-rewrite");
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

/// Proves current v5/v3 persistence round-trips every region treatment and sampling tag while
/// retaining only keyed base/delta intent and deterministic JSON authority bytes.
#[test]
fn stage20q_region_response_records_round_trip_without_derived_state() {
    let source_id = SourceReferenceId::new("stage20q-region-persistence").expect("source ID");
    let document = region_document_with_response(
        source_id.clone(),
        RegionGeometryResponse::Scale {
            sampling: RegionSamplingStrategy::AreaAverage,
            minimum_scale: 0.2,
            maximum_scale: 1.8,
        },
    );
    let selected = document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == document.pattern_settings().definition_id)
        .expect("selected region bundle");
    let output_layer_id = selected.output_settings[0].output_layer_id;
    let delta = document
        .set_channel_region_response_field_for_effective(
            ChannelId(1),
            output_layer_id,
            RegionGeometryFieldEdit::MinimumScale(0.7),
        )
        .expect("Scale delta command");
    let (document, _) = document.apply_command(&delta).expect("Scale delta applies");
    let first = std::env::temp_dir().join(format!(
        "toniator-stage20q-response-a-{}.toniator",
        std::process::id()
    ));
    let second = std::env::temp_dir().join(format!(
        "toniator-stage20q-response-b-{}.toniator",
        std::process::id()
    ));
    save(&first, &document, &region_sources(source_id)).expect("v5 saves Scale response");
    let loaded = load(&first).expect("v5 loads Scale response");
    save(&second, loaded.document(), loaded.sources()).expect("reopened v5 saves");
    assert_eq!(loaded.document(), &document);
    assert_eq!(document_json(&first), document_json(&second));
    let json = document_json(&first);
    for required in [
        "\"kind\":\"regions\"",
        "\"treatment\":\"scale\"",
        "\"sampling\":\"area_average\"",
        "\"minimum_scale\":0.2",
        "\"minimum_scale_delta\":0.49999999999999994",
    ] {
        assert!(json.contains(required), "v5 records {required}");
    }
    for forbidden in [
        "effective_response",
        "region_references",
        "samples",
        "treated_regions",
        "diagnostics",
        "limits",
        "cache",
        "scheduler",
    ] {
        assert!(!json.contains(forbidden), "v5 excludes derived {forbidden}");
    }
    fs::remove_file(first).expect("first temporary document removes");
    fs::remove_file(second).expect("second temporary document removes");

    for response in [
        RegionGeometryResponse::Full {
            sampling: RegionSamplingStrategy::ReferencePoint,
        },
        RegionGeometryResponse::ConstantGap {
            sampling: RegionSamplingStrategy::AreaAverage,
            minimum_gap: -4.0,
            maximum_gap: 3.0,
        },
    ] {
        let source_id = SourceReferenceId::new("stage20q-region-variant").expect("source ID");
        let document = region_document_with_response(source_id.clone(), response.clone());
        let path = std::env::temp_dir().join(format!(
            "toniator-stage20q-response-{:?}-{}.toniator",
            response,
            std::process::id()
        ));
        save(&path, &document, &region_sources(source_id)).expect("variant v5 saves");
        assert_eq!(load(&path).expect("variant v5 loads").document(), &document);
        fs::remove_file(path).expect("variant temporary document removes");
    }

    let gap_source = SourceReferenceId::new("stage20q-gap-delta").expect("source ID");
    let gap_document = region_document_with_response(
        gap_source.clone(),
        RegionGeometryResponse::ConstantGap {
            sampling: RegionSamplingStrategy::ReferencePoint,
            minimum_gap: -2.0,
            maximum_gap: 3.0,
        },
    );
    let gap_output = gap_document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == gap_document.pattern_settings().definition_id)
        .expect("gap bundle")
        .output_settings[0]
        .output_layer_id;
    let gap_delta = gap_document
        .set_channel_region_response_field_for_effective(
            ChannelId(1),
            gap_output,
            RegionGeometryFieldEdit::MaximumGap(4.0),
        )
        .expect("gap delta command");
    let (gap_document, _) = gap_document
        .apply_command(&gap_delta)
        .expect("gap delta applies");
    let gap_path = std::env::temp_dir().join(format!(
        "toniator-stage20q-gap-delta-{}.toniator",
        std::process::id()
    ));
    save(&gap_path, &gap_document, &region_sources(gap_source)).expect("gap v5 saves");
    assert_eq!(
        load(&gap_path).expect("gap v5 loads").document(),
        &gap_document
    );
    assert!(document_json(&gap_path).contains("\"maximum_gap_delta\":1.0"));
    fs::remove_file(gap_path).expect("gap temporary document removes");

    let mut recipe = PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
        PatternDefinitionDraft {
            name: "typed preset regions".into(),
            coverage: CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        },
    ));
    recipe.output_settings[0].response =
        PatternGeometryResponse::Regions(RegionGeometryResponse::ConstantGap {
            sampling: RegionSamplingStrategy::AreaAverage,
            minimum_gap: -1.0,
            maximum_gap: 2.0,
        });
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "stage20q-region-preset".into(),
            name: "Stage 20Q regions".into(),
            category: "test".into(),
            description: "typed response round trip".into(),
            thumbnail: None,
        },
        recipe,
    };
    let preset_path = std::env::temp_dir().join(format!(
        "toniator-stage20q-response-{}.preset.json",
        std::process::id()
    ));
    save_preset(&preset_path, &preset).expect("v3 saves typed response");
    let first_preset = fs::read(&preset_path).expect("v3 bytes read");
    assert_eq!(
        load_preset(&preset_path).expect("v3 loads typed response"),
        preset
    );
    save_preset(
        &preset_path,
        &load_preset(&preset_path).expect("v3 reopen before repeat save"),
    )
    .expect("v3 repeats deterministically");
    assert_eq!(
        first_preset,
        fs::read(&preset_path).expect("repeated v3 bytes read")
    );
    fs::remove_file(preset_path).expect("temporary preset removes");
}

/// Proves malformed current-format treatment records and incompatible keyed deltas reject rather
/// than silently defaulting, adapting, or retaining partial region authority.
#[test]
fn stage20q_region_response_records_reject_malformed_and_incompatible_authority() {
    let source_id = SourceReferenceId::new("stage20q-region-invalid").expect("source ID");
    let document = region_document_with_response(
        source_id.clone(),
        RegionGeometryResponse::Scale {
            sampling: RegionSamplingStrategy::ReferencePoint,
            minimum_scale: 0.2,
            maximum_scale: 1.2,
        },
    );
    let output_layer_id = document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == document.pattern_settings().definition_id)
        .expect("selected bundle")
        .output_settings[0]
        .output_layer_id;
    let delta = document
        .set_channel_region_response_field_for_effective(
            ChannelId(1),
            output_layer_id,
            RegionGeometryFieldEdit::MaximumScale(1.5),
        )
        .expect("Scale delta command");
    let (document, _) = document.apply_command(&delta).expect("Scale delta applies");
    let path = std::env::temp_dir().join(format!(
        "toniator-stage20q-invalid-{}.toniator",
        std::process::id()
    ));

    let save_valid = || save(&path, &document, &region_sources(source_id.clone()));
    save_valid().expect("valid archive saves");
    rewrite_document_json(&path, |root| {
        let bundles = root["document"]["pattern_definition_bundles"]
            .as_array_mut()
            .expect("bundles");
        let response = bundles
            .iter_mut()
            .find(|bundle| bundle["definition"]["output_layers"][0]["kind"] == "regions")
            .expect("region bundle")["output_settings"][0]["response"]["response"]
            .as_object_mut()
            .expect("region response");
        response.remove("sampling");
    });
    assert!(
        load(&path)
            .expect_err("missing sampling rejects")
            .to_string()
            .contains("missing field `sampling`")
    );

    save_valid().expect("valid archive saves again");
    rewrite_document_json(&path, |root| {
        let bundles = root["document"]["pattern_definition_bundles"]
            .as_array_mut()
            .expect("bundles");
        let response = &mut bundles
            .iter_mut()
            .find(|bundle| bundle["definition"]["output_layers"][0]["kind"] == "regions")
            .expect("region bundle")["output_settings"][0]["response"]["response"];
        response["effective_response"] = Value::from(1.0);
    });
    assert!(
        load(&path)
            .expect_err("derived response field rejects")
            .to_string()
            .contains("unknown field `effective_response`")
    );

    save_valid().expect("valid archive saves again");
    rewrite_document_json(&path, |root| {
        let bundles = root["document"]["pattern_definition_bundles"]
            .as_array_mut()
            .expect("bundles");
        let response = &mut bundles
            .iter_mut()
            .find(|bundle| bundle["definition"]["output_layers"][0]["kind"] == "regions")
            .expect("region bundle")["output_settings"][0]["response"]["response"];
        response["minimum_scale"] = Value::from(2.0);
        response["maximum_scale"] = Value::from(1.0);
    });
    assert!(
        load(&path)
            .expect_err("inverted Scale rejects")
            .to_string()
            .contains("pattern.region.scale.range")
    );

    save_valid().expect("valid archive saves again");
    rewrite_document_json(&path, |root| {
        let delta = &mut root["document"]["channel_configuration"]["channels"][0]["pattern_instance"]
            ["output_response_deltas"][0]["delta"];
        *delta = serde_json::json!({
            "kind": "regions",
            "delta": {
                "treatment": "constant_gap",
                "minimum_gap_delta": -1.0,
                "maximum_gap_delta": 1.0
            }
        });
    });
    assert!(
        load(&path)
            .expect_err("cross-treatment delta rejects")
            .to_string()
            .contains("channel.pattern.output_deltas.kind")
    );

    save_valid().expect("valid archive saves again");
    rewrite_document_json(&path, |root| {
        root["document"]["channel_configuration"]["channels"][0]["pattern_instance"]["output_response_deltas"]
            [0]["output_layer_id"] = Value::from(99_999_u64);
    });
    assert!(
        load(&path)
            .expect_err("foreign output delta rejects")
            .to_string()
            .contains("channel.pattern.output_deltas.foreign")
    );

    save_valid().expect("valid archive saves again");
    rewrite_document_json(&path, |root| {
        let bundles = root["document"]["pattern_definition_bundles"]
            .as_array_mut()
            .expect("bundles");
        let response = &mut bundles
            .iter_mut()
            .find(|bundle| bundle["definition"]["output_layers"][0]["kind"] == "regions")
            .expect("region bundle")["output_settings"][0]["response"]["response"];
        response["treatment"] = Value::from("unknown_treatment");
    });
    assert!(
        load(&path)
            .expect_err("unknown treatment rejects")
            .to_string()
            .contains("unknown variant `unknown_treatment`")
    );
    fs::remove_file(path).expect("invalid temporary document removes");

    let mut recipe = PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
        PatternDefinitionDraft {
            name: "invalid preset regions".into(),
            coverage: CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        },
    ));
    recipe.output_settings[0].response =
        PatternGeometryResponse::Regions(RegionGeometryResponse::Scale {
            sampling: RegionSamplingStrategy::ReferencePoint,
            minimum_scale: 0.1,
            maximum_scale: 1.0,
        });
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "stage20q-invalid-preset".into(),
            name: "Invalid Stage 20Q preset".into(),
            category: "test".into(),
            description: "malformed response witness".into(),
            thumbnail: None,
        },
        recipe,
    };
    let preset_path = std::env::temp_dir().join(format!(
        "toniator-stage20q-invalid-{}.preset.json",
        std::process::id()
    ));
    save_preset(&preset_path, &preset).expect("valid preset saves");
    let mut root: Value = serde_json::from_slice(&fs::read(&preset_path).expect("preset reads"))
        .expect("preset JSON parses");
    root["recipe"]["output_settings"][0]["response"]["response"]
        .as_object_mut()
        .expect("preset region response")
        .remove("sampling");
    fs::write(
        &preset_path,
        serde_json::to_vec(&root).expect("preset JSON serializes"),
    )
    .expect("malformed preset writes");
    assert!(
        load_preset(&preset_path)
            .expect_err("missing preset sampling rejects")
            .to_string()
            .contains("missing field `sampling`")
    );
    fs::remove_file(preset_path).expect("invalid temporary preset removes");
}
