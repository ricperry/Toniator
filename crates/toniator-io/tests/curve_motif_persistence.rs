use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft, AuthoredStructureKind,
    CanvasSpec, CoveragePolicy, Document, DocumentCommand, DocumentHistory, DocumentSession,
    GeneralizedSiteProductDraft, GuideDimensionDraft, MarkOrientationDraft, PathStrokeStyle,
    PatternDefinitionRecipe, PatternStructureRecipe, PresetMetadata, PresetRecord, SourceReference,
    SourceReferenceId,
};
use toniator_io::{
    DOCUMENT_SCHEMA_VERSION, EmbeddedSource, EmbeddedSourceFormat, LoadError,
    PRESET_FORMAT_VERSION, SourceBundle, load, load_preset, save, save_preset,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

static PERSISTENCE_FIXTURE_LOCK: Mutex<()> = Mutex::new(());

/// Returns one derived-only path for a Curve Motif persistence witness.
fn validation_path() -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/stage21b-prerequisite-curve-motif/persistence");
    fs::create_dir_all(&directory).expect("derived persistence directory creates");
    directory.join("curve-motif.preset.json")
}

/// Returns the derived-only current-document path for a Curve Motif schema witness.
fn document_validation_path() -> PathBuf {
    validation_path().with_file_name("curve-motif.toniator")
}

/// Rewrites only `document.json` in a derived archive while preserving every other current member.
fn rewrite_document_json(path: &Path, mutate: impl FnOnce(&mut serde_json::Value)) {
    let input = fs::File::open(path).expect("current archive opens");
    let mut archive = ZipArchive::new(input).expect("current archive is ZIP");
    let mut entries = Vec::new();
    let mut mutate = Some(mutate);
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .expect("current archive member opens");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("current member reads");
        if entry.name() == "document.json" {
            let mut json: serde_json::Value =
                serde_json::from_slice(&bytes).expect("document JSON");
            mutate
                .take()
                .expect("one document member receives the requested mutation")(
                &mut json
            );
            bytes = serde_json::to_vec_pretty(&json).expect("rewritten document JSON");
        }
        entries.push((entry.name().to_owned(), entry.is_dir(), bytes));
    }
    let output = fs::File::create(path).expect("derived archive recreates");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o100644);
    for (name, is_directory, bytes) in entries {
        if is_directory {
            writer
                .add_directory(name, options)
                .expect("directory rewrites");
        } else {
            writer.start_file(name, options).expect("file rewrites");
            writer.write_all(&bytes).expect("file bytes rewrite");
        }
    }
    writer.finish().expect("rewritten archive finishes");
}

/// Rewrites only the archive envelope's schema discriminator while preserving all current members.
fn rewrite_document_schema(path: &Path, schema: u32) {
    rewrite_document_json(path, |json| {
        json["document_schema_version"] = serde_json::Value::from(schema);
    });
}

/// Builds the embedded asymmetric open-path record required by preset-v4 Curve Motifs.
fn record() -> PresetRecord {
    let motif = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                end: AuthoredPoint2 { x: 0.4, y: 0.25 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.4, y: 0.25 },
                end: AuthoredPoint2 { x: 1.0, y: 0.0 },
            },
        ],
    )
    .expect("asymmetric open motif validates");
    PresetRecord {
        metadata: PresetMetadata {
            id: "curve-motif-persistence".into(),
            name: "Curve Motif Persistence".into(),
            category: "Test".into(),
            description: "Embedded open-path motif persistence witness.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::connected(PatternStructureRecipe::CurveMotifPaths {
            definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
                name: "Curve Motif persistence family".into(),
                coverage: CoveragePolicy {
                    guard_steps: 1,
                    additional_margin: 0.0,
                },
                dimensions: vec![GuideDimensionDraft {
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    spacing_multiplier: 1.0,
                }],
                product: GeneralizedSiteProductDraft::AlongGuides {
                    dimension_indices: vec![0],
                    interval_multiplier: 1.0,
                    phase: 0.0,
                },
                orientation: MarkOrientationDraft::GuideTangent { dimension_index: 0 },
            }),
            motif,
            style: PathStrokeStyle::default(),
            mirror_alternate_rows: true,
            alternate_row_phase: Some(0.25),
        }),
    }
}

/// Saves one current document with an owned Curve Motif resource and its required embedded source.
fn save_current_curve_motif_document(path: &Path) -> Document {
    let source_id = SourceReferenceId::new("curve-motif-document-source")
        .expect("fixed source identifier validates");
    let document = Document::new_default_document(
        CanvasSpec {
            width: 96.0,
            height: 64.0,
        },
        SourceReference::Assigned(source_id.clone()),
    )
    .expect("source-backed current document validates");
    let mut history =
        DocumentHistory::new(DocumentSession::new(document).expect("session validates"));
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .map(|bundle| bundle.definition.clone())
        .expect("default definition exists");
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: record().recipe,
        })
        .expect("Curve Motif document recipe materializes");
    let source = EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec(),
        Some("curve-motif.svg".into()),
    )
    .expect("embedded source validates");
    let sources = SourceBundle::new([source]).expect("source bundle validates");
    save(path, history.document(), &sources).expect("current document-v7 saves");
    history.document().clone()
}

/// Round-trips one embedded Curve Motif under v4 and rejects the obsolete v3 discriminator.
#[test]
fn curve_motif_preset_v4_round_trips_embedded_geometry_and_rejects_v3() {
    let _lock = PERSISTENCE_FIXTURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = validation_path();
    let expected = record();
    save_preset(&path, &expected).expect("current Curve Motif preset saves");
    let text = fs::read_to_string(&path).expect("current preset reads");
    assert!(text.contains(&format!(
        "\"preset_format_version\": {PRESET_FORMAT_VERSION}"
    )));
    assert!(text.contains("\"kind\": \"curve_motif_paths\""));
    assert!(text.contains("\"segments\""));
    assert_eq!(load_preset(&path).expect("current preset loads"), expected);
    fs::write(
        &path,
        text.replacen(
            &format!("\"preset_format_version\": {PRESET_FORMAT_VERSION}"),
            "\"preset_format_version\": 3",
            1,
        ),
    )
    .expect("derived obsolete preset writes");
    assert!(load_preset(&path).is_err(), "obsolete preset-v3 rejects");
}

/// Round-trips a document-v7 Curve Motif resource/reference and rejects the obsolete v6 discriminator.
#[test]
fn curve_motif_document_v7_round_trips_owned_open_path_and_rejects_v6() {
    let _lock = PERSISTENCE_FIXTURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = document_validation_path();
    let expected = save_current_curve_motif_document(&path);
    let loaded = load(&path).expect("current document-v7 loads");
    assert_eq!(loaded.versions().document(), DOCUMENT_SCHEMA_VERSION);
    assert_eq!(loaded.document(), &expected);
    let definition_id = loaded.document().pattern_settings().definition_id;
    let realization = &loaded
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == definition_id)
        .expect("selected Curve Motif definition remains")
        .definition
        .output_layers[0]
        .realization;
    let toniator_domain::PatternOutputRealization::CurveMotifPaths { structure_id, .. } =
        realization
    else {
        panic!("Curve Motif output remains typed after current round-trip");
    };
    assert!(
        loaded
            .document()
            .authored_structure(*structure_id)
            .is_some()
    );
    rewrite_document_schema(&path, 6);
    assert!(matches!(load(&path), Err(LoadError::Version { .. })));
}

/// Rejects malformed current-v7 Curve Motif resource references and wrong owned-path kinds.
#[test]
fn curve_motif_document_v7_rejects_missing_reference_and_closed_resource() {
    let _lock = PERSISTENCE_FIXTURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = document_validation_path();
    save_current_curve_motif_document(&path);
    rewrite_document_json(&path, |json| {
        json["document"]["pattern_definition_bundles"][1]["definition"]["output_layers"][0]["realization"]
            ["structure_id"] = serde_json::Value::from(999_u64);
    });
    let missing = load(&path).expect_err("missing owned motif reference rejects");
    assert_eq!(missing.path(), "document.validation");

    save_current_curve_motif_document(&path);
    rewrite_document_json(&path, |json| {
        json["document"]["authored_structures"][0]["kind"] = serde_json::Value::from("closed_path");
    });
    let wrong_kind = load(&path).expect_err("closed motif resource rejects");
    assert_eq!(wrong_kind.path(), "document.json");
}

/// Rejects malformed current-v4 Curve Motif phase, family, and embedded open-path geometry.
#[test]
fn curve_motif_preset_v4_rejects_malformed_phase_family_and_geometry() {
    let _lock = PERSISTENCE_FIXTURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = validation_path();
    let reject = |mutate: &dyn Fn(&mut serde_json::Value)| {
        save_preset(&path, &record()).expect("current preset saves before mutation");
        let mut json: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("current preset bytes read"))
                .expect("current preset JSON parses");
        mutate(&mut json);
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json).expect("mutated preset serializes"),
        )
        .expect("derived malformed preset writes");
        assert!(
            load_preset(&path).is_err(),
            "malformed current preset rejects"
        );
    };
    reject(&|json| {
        json["recipe"]["structure"]["alternate_row_phase"] = serde_json::Value::from(0.0);
    });
    reject(&|json| {
        json["recipe"]["structure"]["alternate_row_phase"] = serde_json::Value::from(1.0);
    });
    reject(&|json| {
        json["recipe"]["structure"]["alternate_row_phase"] = serde_json::Value::from("NaN");
    });
    reject(&|json| {
        json["recipe"]["structure"]["definition"]["dimensions"] = serde_json::json!([
            { "baseline_angle_degrees": 0.0, "phase": 0.0, "spacing_multiplier": 1.0 },
            { "baseline_angle_degrees": 90.0, "phase": 0.0, "spacing_multiplier": 1.0 }
        ]);
        json["recipe"]["structure"]["definition"]["product"]["dimension_indices"] =
            serde_json::json!([0, 1]);
    });
    reject(&|json| {
        json["recipe"]["structure"]["segments"][1]["start"]["x"] = serde_json::Value::from(0.41);
    });
}
