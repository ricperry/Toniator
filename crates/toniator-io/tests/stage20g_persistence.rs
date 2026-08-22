use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};
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
                    "\"document_schema_version\":4",
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

/// Opens every tracked v4 fixture, resolves each effective channel, and
/// verifies that the persisted archive carries no runtime-only projection.
#[test]
fn v4_fixtures_reopen_with_base_and_delta_authority_only() {
    for fixture in [
        "HolidayMugs_2024_2025.toniator",
        "raster-sample.toniator",
        "vector-sample.toniator",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(fixture);
        let loaded = toniator_io::load(&path).expect("current fixture opens");
        for channel_id in [ChannelId(1), ChannelId(2), ChannelId(3)] {
            loaded
                .document()
                .effective_channel_pattern(channel_id)
                .expect("persisted channel resolves through document authority");
        }
    }
}

/// Locks the diverging Holiday channel recipes and rotations after the one-time v4 authority port.
#[test]
fn holiday_fixture_preserves_diverging_effective_channel_settings() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/HolidayMugs_2024_2025.toniator");
    let loaded = load(&path).expect("Holiday v4 fixture opens");
    let effective = [ChannelId(1), ChannelId(2), ChannelId(3)].map(|channel_id| {
        loaded
            .document()
            .effective_channel_pattern(channel_id)
            .expect("Holiday channel resolves")
    });
    assert_eq!(
        effective
            .iter()
            .map(|pattern| pattern.definition_id.0)
            .collect::<Vec<_>>(),
        vec![4, 3, 2]
    );
    assert_eq!(
        effective
            .iter()
            .map(|pattern| pattern.pattern_rotation_degrees)
            .collect::<Vec<_>>(),
        vec![0.0, 30.0, 60.0]
    );
    assert!(effective.iter().all(|pattern| {
        pattern.density.across_x == 400.0
            && pattern.density.across_y == 400.0
            && pattern.shape_rotation_degrees == 0.0
    }));
    assert_eq!(
        loaded
            .document()
            .channel_pattern_instance(ChannelId(1))
            .expect("first channel intent")
            .definition_override,
        None
    );
}

/// Saves deterministically, preserves explicit zero deltas, and omits every effective projection.
#[test]
fn v4_save_is_deterministic_and_serializes_only_base_plus_authored_deltas() {
    let (document, sources) = source_backed_document();
    let document = document
        .apply_command(&DocumentCommand::SetChannelDensityDelta {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(1),
            density: DensityMetricDelta2D {
                across_x_delta: 0.0,
                across_y_delta: 0.0,
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
        .apply_command(&DocumentCommand::SetChannelGeometryResponseDelta {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(1),
            geometry_response: ChannelGeometryResponseDelta::Marks(MarkGeometryResponseDelta {
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
    assert!(text.contains("\"document_schema_version\":4"));
    assert!(text.contains("\"pattern_settings\""));
    assert!(text.contains("\"pattern_instance\""));
    assert!(!text.contains("EffectiveChannelPatternInstance"));
    assert!(!text.contains("effective_channel_pattern"));
    let loaded = load(&first).expect("v4 reloads");
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
            .across_x_delta,
        0.0
    );
    assert_eq!(instance.layout_delta.rotation_degrees, Some(0.0));
    let ChannelGeometryResponseDelta::Marks(delta) = instance
        .geometry_response_delta
        .as_ref()
        .expect("response delta")
    else {
        panic!("mark fixture reloads mark response delta");
    };
    assert_eq!(delta.minimum_fill_delta, Some(0.0));
    assert_eq!(delta.maximum_fill_delta, None);
    fs::remove_file(first).expect("first temporary removes");
    fs::remove_file(second).expect("second temporary removes");
}

/// Rejects document schemas one through three without a migration or fallback decoder.
#[test]
fn document_versions_one_through_three_are_rejected() {
    let (document, sources) = source_backed_document();
    let current = temporary("current.toniator");
    save(&current, &document, &sources).expect("current save succeeds");
    for version in 1..=3 {
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

/// Keeps preset v2 deterministic and materializes the same ID-free recipe at either authority scope.
#[test]
fn preset_v2_bytes_reconstruct_document_base_or_channel_override() {
    assert_eq!(PRESET_FORMAT_VERSION, 2);
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "stage20g-grid".into(),
            name: "Stage 20G Grid".into(),
            category: "Test".into(),
            description: "One ID-free recipe for both pattern scopes.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::StraightGrid(PatternDefinitionDraft {
            name: "Materialized grid".into(),
            coverage: toniator_domain::CoveragePolicy {
                guard_steps: 3,
                additional_margin: 1.25,
            },
        }),
    };
    let first = temporary("preset-a.json");
    let second = temporary("preset-b.json");
    save_preset(&first, &preset).expect("first preset saves");
    save_preset(&second, &preset).expect("second preset saves");
    let bytes = fs::read(&first).expect("preset reads");
    assert_eq!(bytes, fs::read(&second).expect("second preset reads"));
    assert_eq!(load_preset(&first).expect("preset reloads"), preset);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "c0dd63d599e6a111c158448726fc5b3a93d922f66204cd769a44876dd43886de"
    );

    let (document, _) = source_backed_document();
    let base_definition = document.pattern_definitions()[0].clone();
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
