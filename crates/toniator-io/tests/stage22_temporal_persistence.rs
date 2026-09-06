use serde_json::Value;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use toniator_domain::{
    CanvasSpec, ChannelId, ColorEndMode, ColorValue, Document, DocumentConfiguration, Easing,
    FrameRange, FrameRate, ProjectTiming, PropertyFieldId, PropertyTarget, SourceReference,
    SourceReferenceId, TemporalEndpointEdit,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, SourceMediaManifest,
    capture_document_preset_destination, load, load_document_preset, save, save_document_preset,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Owns only one unique test directory and removes its generated files at scope exit.
struct TestDirectory(PathBuf);

impl TestDirectory {
    /// Creates an exclusive directory for one persistence test.
    ///
    /// # Panics
    /// Panics if the test clock or temporary filesystem is unavailable.
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "toniator-stage22-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    /// Removes only this test's exclusively created directory.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Constructs current start settings plus scalar, grouped-color and hue End intent.
///
/// # Panics
/// Panics if the known current domain fixture fails validation.
fn fixture() -> (Document, SourceBundle) {
    let id = SourceReferenceId::new("stage22-source").unwrap();
    let mut document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Assigned(id.clone()),
    )
    .unwrap();
    let timing = ProjectTiming::new(
        FrameRate::new(30_000, 1_001).unwrap(),
        FrameRange::new(0, 3).unwrap(),
    );
    let command = document
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::Channel(ChannelId(1)),
            field: PropertyFieldId::Opacity,
            effective_end: 0.25,
            easing: Easing::SmoothStep,
        }])
        .unwrap();
    document = document
        .with_temporal_authority(timing.clone(), command.replacement().end_overrides.clone())
        .unwrap();
    let command = document
        .edit_color_end_command(
            ChannelId(2),
            Some((
                ColorEndMode::HueRotation {
                    end_degrees: -720.0,
                },
                Easing::Linear,
            )),
        )
        .unwrap();
    document = document
        .with_temporal_authority(timing.clone(), command.replacement().end_overrides.clone())
        .unwrap();
    let command = document
        .edit_color_end_command(
            ChannelId(3),
            Some((
                ColorEndMode::LinearColor {
                    end: ColorValue {
                        red: 0.123456789,
                        green: 0.4,
                        blue: 0.7,
                        alpha: 0.35,
                    },
                },
                Easing::QuadraticOut,
            )),
        )
        .unwrap();
    document = document
        .with_temporal_authority(timing, command.replacement().end_overrides.clone())
        .unwrap();
    let sources = SourceBundle::new([EmbeddedSource::new(
        id,
        EmbeddedSourceFormat::Png,
        include_bytes!("../../../assets/raster-sample.png").as_slice(),
        Some("raster-sample.png".into()),
    )
    .unwrap()])
    .unwrap();
    (document, sources)
}

/// Reads exact persisted JSON from a generated project or document Preset.
///
/// # Panics
/// Panics when the generated archive or its document entry is invalid.
fn json(path: &Path) -> Value {
    let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
    let name = if archive.file_names().any(|name| name == "preset.json") {
        "preset.json"
    } else {
        "document.json"
    };
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Rewrites only a generated test archive's JSON while retaining all source entries.
///
/// # Panics
/// Panics if generated archive I/O fails.
fn rewrite_json(path: &Path, mutate: impl FnOnce(&mut Value)) {
    let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        entries.push((name, bytes));
    }
    drop(archive);
    let entry = entries
        .iter_mut()
        .find(|(name, _)| name == "document.json")
        .unwrap();
    let mut value = serde_json::from_slice(&entry.1).unwrap();
    mutate(&mut value);
    entry.1 = serde_json::to_vec(&value).unwrap();
    let mut archive = ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in entries {
        archive
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&bytes).unwrap();
    }
    archive.finish().unwrap();
}

/// Proves projects persist only End intent and retain exact rational timing and evaluated frames.
#[test]
fn project_roundtrip_keeps_start_and_end_authority_separate() {
    let directory = TestDirectory::new();
    let path = directory.0.join("animation.toniator");
    let (document, sources) = fixture();
    save(&path, &document, &sources).unwrap();
    let saved = json(&path);
    assert_eq!(saved["document_schema_version"], 8);
    let overrides = saved["document"]["end_overrides"].as_array().unwrap();
    assert_eq!(overrides.len(), 3);
    for value in overrides {
        assert!(value.get("start").is_none());
        assert!(value.get("start_frame").is_none());
    }
    let loaded = load(&path).unwrap();
    assert_eq!(loaded.document(), &document);
    assert_eq!(loaded.sources(), &sources);
    for frame in 0..3 {
        assert_eq!(
            loaded.document().materialize_frame(frame).unwrap(),
            document.materialize_frame(frame).unwrap()
        );
    }
}

/// Proves initialized equal End settings survive project storage independently of later Start edits.
///
/// # Panics
/// Panics if persistence drops initialization or duplicates Start authority.
#[test]
fn initialized_end_roundtrip_preserves_equal_settings() {
    let directory = TestDirectory::new();
    let path = directory.0.join("initialized.toniator");
    let (document, sources) = fixture();
    let (document, _) = document
        .apply_command(&toniator_domain::DocumentCommand::SetColorComponent {
            channel_id: ChannelId(1),
            component: toniator_domain::ColorComponent::Blue,
            value: ColorValue::from_srgb_hex("#FFE080FF").unwrap().blue,
        })
        .unwrap();
    let command = document.initialize_end_command().unwrap();
    let initialized = document
        .with_temporal_authority(
            command.replacement().project_timing.clone(),
            command.replacement().end_overrides.clone(),
        )
        .unwrap();
    save(&path, &initialized, &sources).unwrap();
    let loaded = load(&path).unwrap();
    assert_eq!(loaded.document(), &initialized);
    assert_eq!(
        loaded
            .document()
            .initialize_end_command()
            .unwrap()
            .replacement(),
        &initialized.temporal_authority()
    );
    let (edited, _) = loaded
        .document()
        .apply_command(&toniator_domain::DocumentCommand::SetColorComponent {
            channel_id: ChannelId(1),
            component: toniator_domain::ColorComponent::Red,
            value: 0.123,
        })
        .unwrap();
    assert_eq!(
        edited
            .materialize_frame(2)
            .unwrap()
            .solid_paint(ChannelId(1))
            .unwrap(),
        initialized
            .materialize_frame(2)
            .unwrap()
            .solid_paint(ChannelId(1))
            .unwrap()
    );
    for value in json(&path)["document"]["end_overrides"].as_array().unwrap() {
        assert!(value.get("start").is_none());
    }
}

/// Proves both Preset input formats transfer End intent while retaining destination timing/source.
#[test]
fn document_presets_reuse_end_intent_but_retain_project_timing() {
    let directory = TestDirectory::new();
    let (document, sources) = fixture();
    let preset = directory.0.join("animation.toniator-preset");
    let configuration = DocumentConfiguration::capture(&document);
    let guard = capture_document_preset_destination(&preset).unwrap();
    save_document_preset(&preset, &configuration, &guard).unwrap();
    let saved = json(&preset);
    assert!(saved["configuration"].get("project_timing").is_none());
    assert_eq!(
        saved["configuration"]["end_overrides"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let timing = ProjectTiming::new(
        FrameRate::new(6, 1).unwrap(),
        FrameRange::new(0, 10).unwrap(),
    );
    let destination = Document::new_default_document(
        CanvasSpec {
            width: 900.0,
            height: 620.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap()
    .with_temporal_authority(timing.clone(), vec![])
    .unwrap();
    let project = directory.0.join("animation.toniator");
    save(&project, &document, &sources).unwrap();
    for path in [&preset, &project] {
        let configuration = load_document_preset(path, &destination).unwrap();
        let applied = configuration.bind(&destination).unwrap();
        assert_eq!(applied.project_timing(), &timing);
        assert_eq!(applied.source(), destination.source());
        assert_eq!(applied.canvas(), destination.canvas());
        assert_eq!(
            applied.temporal_end_overrides(),
            document.temporal_end_overrides()
        );
    }
}

/// Proves obsolete schemas and invalid or redundant Start data never load as current End intent.
#[test]
fn malformed_temporal_data_is_rejected_without_migration() {
    let directory = TestDirectory::new();
    let (document, sources) = fixture();
    let path = directory.0.join("invalid.toniator");
    for case in 0..8 {
        save(&path, &document, &sources).unwrap();
        rewrite_json(&path, |value| match case {
            0 => value["document_schema_version"] = 7.into(),
            1 => value["document"]["project_timing"]["fps_denominator"] = 0.into(),
            2 => value["document"]["end_overrides"][0]["field"] = "random_seed".into(),
            3 => value["document"]["end_overrides"][0]["start"] = 0.1.into(),
            4 => value["start_snapshot"] = serde_json::json!({}),
            5 => value["document"]["start_snapshot"] = serde_json::json!({}),
            6 => value["sources"][0]["foreign_setting"] = true.into(),
            _ => value["document"]["pattern_settings"]["foreign_setting"] = true.into(),
        });
        assert!(load(&path).is_err(), "malformed case {case}");
    }
}

/// Preserves encoded video ownership, temporal intent, and project-as-Preset isolation.
///
/// # Panics
/// Panics if the generated current-format fixture fails persistence or domain validation.
#[test]
fn video_container_roundtrip_preserves_media_and_end_intent() {
    let directory = TestDirectory::new();
    let (document, _) = fixture();
    let SourceReference::Assigned(id) = document.source() else {
        panic!("assigned fixture")
    };
    let sources = SourceBundle::new([EmbeddedSource::new(
        id.clone(),
        EmbeddedSourceFormat::Video,
        include_bytes!("../../../assets/video-sample0001-0010.mp4").as_slice(),
        Some("video-sample0001-0010.mp4".into()),
    )
    .unwrap()])
    .unwrap();
    let path = directory.0.join("video.toniator");
    save(&path, &document, &sources).unwrap();
    let loaded = load(&path).unwrap();
    assert_eq!(loaded.versions().container(), 2);
    assert_eq!(loaded.sources(), &sources);
    assert_eq!(loaded.document(), &document);
    let stored = json(&path);
    assert_eq!(stored["media"]["kind"], "video");
    assert_eq!(stored["media"]["video_stream"], 0);
    assert_eq!(stored["sources"][0]["entry_name"], "sources/000000.video");
    let destination = Document::new_default_document(
        CanvasSpec {
            width: 900.0,
            height: 620.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let applied = load_document_preset(&path, &destination)
        .unwrap()
        .bind(&destination)
        .unwrap();
    assert_eq!(applied.source(), destination.source());
    assert_eq!(applied.canvas(), destination.canvas());
    assert_eq!(applied.project_timing(), destination.project_timing());
    assert_eq!(
        applied.temporal_end_overrides(),
        document.temporal_end_overrides()
    );
}

/// Keeps authored sequence order and repeated frames independent of sorted physical ZIP entries.
///
/// # Panics
/// Panics when a valid generated sequence fails persistence or a malformed manifest is accepted.
#[test]
fn sequence_manifest_roundtrip_and_malformed_ownership() {
    let directory = TestDirectory::new();
    let first = SourceReferenceId::new("z-first").unwrap();
    let second = SourceReferenceId::new("a-second").unwrap();
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Assigned(first.clone()),
    )
    .unwrap();
    let sources = SourceBundle::new_media(
        [first.clone(), second.clone()].into_iter().map(|id| {
            EmbeddedSource::new(
                id,
                EmbeddedSourceFormat::Png,
                include_bytes!("../../../assets/raster-sample.png").as_slice(),
                None,
            )
            .unwrap()
        }),
        SourceMediaManifest::ImageSequence {
            source_ids: vec![first, second, SourceReferenceId::new("z-first").unwrap()],
            frame_rate: FrameRate::new(30_000, 1_001).unwrap(),
        },
    )
    .unwrap();
    let path = directory.0.join("sequence.toniator");
    save(&path, &document, &sources).unwrap();
    let loaded = load(&path).unwrap();
    assert_eq!(loaded.sources(), &sources);
    assert_eq!(loaded.document(), &document);
    let stored = json(&path);
    assert_eq!(stored["sources"][0]["id"], "a-second");
    assert_eq!(stored["sources"][1]["id"], "z-first");
    assert_eq!(
        stored["media"]["source_ids"],
        serde_json::json!(["z-first", "a-second", "z-first"])
    );
    for case in 0..10 {
        save(&path, &document, &sources).unwrap();
        rewrite_json(&path, |value| match case {
            0 => value["container_version"] = 1.into(),
            1 => value["media"]["source_ids"] = serde_json::json!([]),
            2 => value["media"]["source_ids"] = serde_json::json!(["z-first"]),
            3 => {
                value["media"]["source_ids"] = serde_json::json!(["missing", "a-second", "z-first"])
            }
            4 => value["media"]["source_ids"] = serde_json::json!(["a-second", "z-first"]),
            5 => value["media"]["fps_denominator"] = 0.into(),
            6 => value["media"]["foreign_setting"] = true.into(),
            7 => value["sources"][0]["entry_name"] = "../escape.png".into(),
            8 => value["sources"][0]["sha256"] = "incorrect".into(),
            _ => value["sources"][1]["id"] = "a-second".into(),
        });
        assert!(load(&path).is_err(), "malformed media case {case}");
    }
}
