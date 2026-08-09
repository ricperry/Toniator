use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPaint, ChannelPatternLayout,
    ChannelSourceMapping, ChannelState, ChannelTopology, ChannelTopologyTemplate, ColorValue,
    CoveragePolicy, DensityMetric2D, Document, DocumentHistory, DocumentId, DocumentSession,
    GeneralizedSiteProduct, GuideDimensionId, HalftoneChannelModel, MarkGeometryResponse,
    MarkOrientation, PatternDefinition, PatternDefinitionId, PatternMechanismId,
    PatternOutputLayerId, SourceComponent, SourceMappingComponent, SourcePlacement,
    SourceReference, SourceReferenceId, StraightGuideDimension, StraightGuideRepetition,
};
use toniator_io::{
    CONTAINER_VERSION, DOCUMENT_SCHEMA_VERSION, EmbeddedSource, EmbeddedSourceFormat, LoadError,
    MAX_ARCHIVE_BYTES, MAX_DOCUMENT_BYTES, MAX_SOURCE_BYTES, SourceBundle, load, save,
};

fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "toniator-stage12-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn legacy_document() -> (Document, SourceBundle) {
    let id = SourceReferenceId::new("source-1").unwrap();
    let document = Document::with_source(
        DocumentId(81),
        CanvasSpec {
            width: 101.25,
            height: 77.5,
        },
        SourceReference::Assigned(id.clone()),
        vec![PatternDefinition::supported_straight_grid(
            PatternDefinitionId(91),
            "grid",
            PatternMechanismId(181),
            PatternMechanismId(182),
            PatternOutputLayerId(91),
            CoveragePolicy {
                guard_steps: 2,
                maximum_support_radius: 4.5,
            },
        )],
        vec![ChannelState {
            id: ChannelId(55),
            pattern_definition_id: PatternDefinitionId(91),
            layout: ChannelPatternLayout {
                density: DensityMetric2D {
                    across_x: 10.125,
                    across_y: 7.75,
                    aspect_locked: false,
                },
                rotation_degrees: -17.25,
                translation_x: 3.5,
                translation_y: -2.75,
            },
            appearance: ChannelAppearance {
                visible: false,
                color: ColorValue {
                    red: 0.1,
                    green: 0.2,
                    blue: 0.3,
                    alpha: 0.4,
                },
                opacity: 0.5,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_size: 1.0,
                maximum_size: 8.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Alpha,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )
    .unwrap();
    let bundle = SourceBundle::new([EmbeddedSource::new(
        id,
        EmbeddedSourceFormat::Svg,
        b"<svg/>".to_vec(),
        Some("source.svg".into()),
    )
    .unwrap()])
    .unwrap();
    (document, bundle)
}

#[test]
fn generalized_v2_definition_round_trips_without_serializing_runtime_state() {
    let (mut document, sources) = legacy_document();
    let definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(91),
        "typed",
        PatternMechanismId(181),
        PatternMechanismId(182),
        PatternOutputLayerId(91),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(2),
                baseline_angle_degrees: 60.0,
                phase: 0.25,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(3),
                baseline_angle_degrees: 120.0,
                phase: 0.5,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 0.75,
                },
            },
        ],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(3)],
            interval_multiplier: 0.5,
            phase: 1.25,
        },
        MarkOrientation::GuideTangent {
            dimension_id: GuideDimensionId(3),
        },
        CoveragePolicy {
            guard_steps: 2,
            maximum_support_radius: 4.5,
        },
    );
    document = Document::with_source(
        DocumentId(81),
        document.canvas().clone(),
        document.source().clone(),
        vec![definition],
        document.channels().unwrap().to_vec(),
    )
    .unwrap();
    let path = temporary("stage16a-generalized.toniator");
    save(&path, &document, &sources).unwrap();
    let loaded = load(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert_eq!(loaded.document(), &document);
}

fn asset(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(name)
}

fn saved_parts(document: &Document, sources: &SourceBundle) -> (Vec<u8>, String, Vec<u8>) {
    let path = temporary("parts.toniator");
    save(&path, document, sources).unwrap();
    let bytes = fs::read(&path).unwrap();
    fs::remove_file(path).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut json = Vec::new();
    archive
        .by_name("document.json")
        .unwrap()
        .read_to_end(&mut json)
        .unwrap();
    let source_name = archive.by_index(1).unwrap().name().to_owned();
    let mut source = Vec::new();
    archive
        .by_name(&source_name)
        .unwrap()
        .read_to_end(&mut source)
        .unwrap();
    (json, source_name, source)
}

fn archive_from_entries(entries: &[(&str, &[u8], zip::CompressionMethod)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes, method) in entries {
        writer
            .start_file(
                *name,
                zip::write::SimpleFileOptions::default().compression_method(*method),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn archive_with_sources_marker(
    document: (&[u8], zip::CompressionMethod),
    source_name: &str,
    source: (&[u8], zip::CompressionMethod),
) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .start_file(
            "document.json",
            zip::write::SimpleFileOptions::default().compression_method(document.1),
        )
        .unwrap();
    writer.write_all(document.0).unwrap();
    writer
        .add_directory("sources/", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer
        .start_file(
            source_name,
            zip::write::SimpleFileOptions::default().compression_method(source.1),
        )
        .unwrap();
    writer.write_all(source.0).unwrap();
    writer.finish().unwrap().into_inner()
}

fn archive_with_directory_and_files(
    directory: &str,
    document: &[u8],
    source_name: &str,
    source: &[u8],
) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .start_file("document.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(document).unwrap();
    writer
        .add_directory(directory, zip::write::SimpleFileOptions::default())
        .unwrap();
    writer
        .start_file(source_name, zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(source).unwrap();
    writer.finish().unwrap().into_inner()
}

fn set_central_entry_sizes(bytes: &mut [u8], entry_index: usize, size: u64) {
    let positions = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(index, value)| (value == b"PK\x01\x02").then_some(index))
        .collect::<Vec<_>>();
    let position = positions[entry_index];
    let size = u32::try_from(size).unwrap().to_le_bytes();
    bytes[position + 20..position + 24].copy_from_slice(&size);
    bytes[position + 24..position + 28].copy_from_slice(&size);
}

fn set_central_uncompressed_size(bytes: &mut [u8], entry_index: usize, size: u64) {
    let positions = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(index, value)| (value == b"PK\x01\x02").then_some(index))
        .collect::<Vec<_>>();
    let position = positions[entry_index];
    let size = u32::try_from(size).unwrap().to_le_bytes();
    bytes[position + 24..position + 28].copy_from_slice(&size);
}

fn set_local_entry_sizes(bytes: &mut [u8], entry_index: usize, size: u64) {
    let position = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(index, value)| (value == b"PK\x03\x04").then_some(index))
        .collect::<Vec<_>>()[entry_index];
    let size = u32::try_from(size).unwrap().to_le_bytes();
    bytes[position + 18..position + 22].copy_from_slice(&size);
    bytes[position + 22..position + 26].copy_from_slice(&size);
}

fn replace_central_name(bytes: &mut [u8], entry_index: usize, replacement: &[u8]) {
    let position = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(index, value)| (value == b"PK\x01\x02").then_some(index))
        .collect::<Vec<_>>()[entry_index];
    let name_length = u16::from_le_bytes([bytes[position + 28], bytes[position + 29]]) as usize;
    assert_eq!(name_length, replacement.len());
    bytes[position + 46..position + 46 + name_length].copy_from_slice(replacement);
}

fn write_bytes(bytes: &[u8]) -> PathBuf {
    let path = temporary("hostile.toniator");
    fs::write(&path, bytes).unwrap();
    path
}

fn assert_load_error(bytes: &[u8], predicate: impl FnOnce(&LoadError) -> bool) {
    let path = write_bytes(bytes);
    let result = std::panic::catch_unwind(|| load(&path));
    assert!(result.is_ok(), "hostile input must not panic");
    let error = result.unwrap().unwrap_err();
    assert!(predicate(&error), "unexpected error: {error}");
    fs::remove_file(path).unwrap();
}

#[test]
fn v1_round_trips_legacy_exactly_and_is_byte_deterministic() {
    let (document, sources) = legacy_document();
    let one = temporary("one.toniator");
    let two = temporary("two.toniator");
    save(&one, &document, &sources).unwrap();
    save(&two, &document, &sources).unwrap();
    assert_eq!(fs::read(&one).unwrap(), fs::read(&two).unwrap());
    let loaded = load(&one).unwrap();
    assert_eq!(loaded.document(), &document);
    assert_eq!(loaded.sources(), &sources);
    assert_eq!(loaded.versions().container(), CONTAINER_VERSION);
    assert_eq!(loaded.versions().document(), DOCUMENT_SCHEMA_VERSION);
    assert!(loaded.migration_report().is_empty());
    let history = DocumentHistory::new(DocumentSession::new(loaded.document().clone()).unwrap());
    assert_eq!(history.revision().0, 0);
    assert!(!history.can_undo());
    assert!(!history.can_redo());
    fs::remove_file(one).unwrap();
    fs::remove_file(two).unwrap();
}

#[test]
fn v1_round_trips_every_canonical_modeled_topology() {
    for model in [
        HalftoneChannelModel::Rgb,
        HalftoneChannelModel::Cmyk,
        HalftoneChannelModel::SourceColorAlpha,
    ] {
        let (legacy, sources) = legacy_document();
        let topology = legacy
            .canonical_channel_topology(
                model,
                ChannelTopologyTemplate {
                    pattern_definition_id: PatternDefinitionId(91),
                    layout: legacy.channels().unwrap()[0].layout.clone(),
                    mark_geometry_response: legacy.channels().unwrap()[0]
                        .mark_geometry_response
                        .clone(),
                },
            )
            .unwrap();
        let document = Document::with_source_and_topology(
            legacy.id(),
            legacy.canvas().clone(),
            legacy.source().clone(),
            legacy.pattern_definitions().to_vec(),
            model,
            topology,
        )
        .unwrap();
        let path = temporary("modeled.toniator");
        save(&path, &document, &sources).unwrap();
        assert_eq!(load(&path).unwrap().document(), &document);
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn rejects_extra_entries_and_changed_source_integrity_without_extracting() {
    let (document, sources) = legacy_document();
    let path = temporary("bad.toniator");
    save(&path, &document, &sources).unwrap();
    let bytes = fs::read(&path).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut document_json = Vec::new();
    archive
        .by_name("document.json")
        .unwrap()
        .read_to_end(&mut document_json)
        .unwrap();
    let mut source = Vec::new();
    archive
        .by_name("sources/source-1.svg")
        .unwrap()
        .read_to_end(&mut source)
        .unwrap();
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("document.json", options).unwrap();
    writer.write_all(&document_json).unwrap();
    writer.start_file("sources/source-1.svg", options).unwrap();
    writer.write_all(b"changed").unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    fs::write(&path, bytes).unwrap();
    assert!(matches!(
        load(&path),
        Err(toniator_io::LoadError::Integrity { .. })
    ));
    fs::remove_file(path).unwrap();
}

#[test]
fn failed_save_does_not_replace_existing_destination() {
    let (document, sources) = legacy_document();
    let path = temporary("destination.toniator");
    fs::write(&path, b"preserve me").unwrap();
    let empty = SourceBundle::new([]).unwrap();
    assert!(save(&path, &document, &empty).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"preserve me");
    let _ = sources;
    fs::remove_file(path).unwrap();
}

#[test]
fn modeled_round_trip_retains_nondefault_fields_and_arbitrary_stable_ids() {
    for model in [
        HalftoneChannelModel::Rgb,
        HalftoneChannelModel::Cmyk,
        HalftoneChannelModel::SourceColorAlpha,
    ] {
        let (legacy, sources) = legacy_document();
        let mut channels = legacy
            .canonical_channel_topology(
                model,
                ChannelTopologyTemplate {
                    pattern_definition_id: PatternDefinitionId(91),
                    layout: legacy.channels().unwrap()[0].layout.clone(),
                    mark_geometry_response: legacy.channels().unwrap()[0]
                        .mark_geometry_response
                        .clone(),
                },
            )
            .unwrap()
            .channels()
            .to_vec();
        for (index, channel) in channels.iter_mut().enumerate() {
            channel.id = ChannelId(1_000 + index as u64 * 17);
            channel.layout.density.aspect_locked = false;
            channel.layout.rotation_degrees = index as f64 + 3.5;
            channel.layout.translation_x = -(index as f64) - 0.25;
            channel.layout.translation_y = index as f64 + 0.75;
            channel.mark_geometry_response = MarkGeometryResponse {
                minimum_size: 1.5,
                maximum_size: 8.5,
            };
            channel.mapping.component = if index % 2 == 0 {
                SourceMappingComponent::Luminance
            } else {
                SourceMappingComponent::Alpha
            };
            channel.mapping.inverted = index % 2 == 1;
            channel.mapping.gain = 0.75;
            channel.mapping.bias = -0.1;
            channel.visible = index % 2 == 0;
            channel.opacity = 0.6;
            if let ChannelPaint::Solid(color) = &mut channel.paint {
                *color = ColorValue {
                    red: 0.2,
                    green: 0.3,
                    blue: 0.4,
                    alpha: 0.5,
                };
            }
        }
        let document = Document::with_source_and_topology(
            legacy.id(),
            legacy.canvas().clone(),
            legacy.source().clone(),
            legacy.pattern_definitions().to_vec(),
            model,
            ChannelTopology::new(channels),
        )
        .unwrap();
        let path = temporary("non-default-modeled.toniator");
        save(&path, &document, &sources).unwrap();
        assert_eq!(load(&path).unwrap().document(), &document);
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn frozen_fixtures_preserve_committed_baseline_bytes_and_v1_metadata() {
    for (fixture, baseline, format) in [
        (
            "raster-sample-v1.toniator",
            "raster-sample.png",
            EmbeddedSourceFormat::Png,
        ),
        (
            "vector-sample-v1.toniator",
            "vector-sample.svg",
            EmbeddedSourceFormat::Svg,
        ),
    ] {
        let loaded = load(&asset(fixture)).unwrap();
        assert_eq!(loaded.versions().container(), 1);
        assert_eq!(loaded.versions().document(), 1);
        assert!(!loaded.migration_report().is_empty());
        assert_eq!(
            loaded.migration_report().generated_definition_ids(),
            vec![PatternDefinitionId(1)]
        );
        assert_eq!(
            loaded.migration_report().generated_definitions()[0].mechanism_ids,
            vec![PatternMechanismId(1), PatternMechanismId(2)]
        );
        assert_eq!(
            loaded.migration_report().generated_definitions()[0].output_layer_ids,
            vec![PatternOutputLayerId(1)]
        );
        let source = loaded.sources().entries().next().unwrap();
        assert_eq!(source.format(), format);
        assert_eq!(source.bytes(), fs::read(asset(baseline)).unwrap());
    }
    assert!(
        std::str::from_utf8(
            load(&asset("vector-sample-v1.toniator"))
                .unwrap()
                .sources()
                .entries()
                .next()
                .unwrap()
                .bytes()
        )
        .unwrap()
        .contains(">T<")
    );
}

#[test]
fn frozen_v1_documents_migrate_to_deterministic_v2_saves_without_transient_state() {
    let validation = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/validation/stage-14");
    fs::create_dir_all(&validation).unwrap();
    for fixture in ["raster-sample-v1.toniator", "vector-sample-v1.toniator"] {
        let loaded = load(&asset(fixture)).unwrap();
        assert_eq!(loaded.versions().document(), 1);
        assert!(!loaded.migration_report().is_empty());
        let v2 = validation.join(fixture.replace("-v1", "-migrated-v2"));
        let duplicate = validation.join(fixture.replace("-v1", "-migrated-v2-repeat"));
        save(&v2, loaded.document(), loaded.sources()).unwrap();
        save(&duplicate, loaded.document(), loaded.sources()).unwrap();
        assert_eq!(fs::read(&v2).unwrap(), fs::read(&duplicate).unwrap());
        let reopened = load(&v2).unwrap();
        assert_eq!(reopened.versions().document(), 2);
        assert!(reopened.migration_report().is_empty());
        assert_eq!(reopened.document(), loaded.document());
        assert_eq!(
            reopened.sources().entries().next().unwrap().bytes(),
            loaded.sources().entries().next().unwrap().bytes()
        );
        let (json, _, _) = saved_parts(reopened.document(), reopened.sources());
        let json = String::from_utf8(json).unwrap();
        assert!(json.contains("\"document_schema_version\":2"));
        assert!(json.contains("\"mechanisms\""));
        assert!(json.contains("\"output_layers\""));
        for forbidden in [
            "history",
            "revision",
            "dirty",
            "savepoint",
            "preview",
            "export",
            "original_path",
            "scheduler",
            "cache",
        ] {
            assert!(
                !json.contains(&format!("\"{forbidden}\"")),
                "serialized transient {forbidden}"
            );
        }
    }
}

#[test]
fn moved_container_survives_external_source_deletion() {
    let root = temporary("move-root");
    fs::create_dir(&root).unwrap();
    let external = root.join("external.png");
    fs::copy(asset("raster-sample.png"), &external).unwrap();
    let (document, _) = legacy_document();
    let sources = SourceBundle::new([EmbeddedSource::new(
        SourceReferenceId::new("source-1").unwrap(),
        EmbeddedSourceFormat::Png,
        fs::read(&external).unwrap(),
        None,
    )
    .unwrap()])
    .unwrap();
    let original = root.join("original.toniator");
    save(&original, &document, &sources).unwrap();
    let moved = root.join("moved.toniator");
    fs::rename(&original, &moved).unwrap();
    fs::remove_file(external).unwrap();
    let loaded = load(&moved).unwrap();
    assert_eq!(
        loaded.sources().entries().next().unwrap().bytes(),
        fs::read(asset("raster-sample.png")).unwrap()
    );
    fs::remove_file(moved).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn deterministic_archive_metadata_and_json_exclude_transient_state() {
    let (document, sources) = legacy_document();
    let path = temporary("metadata.toniator");
    save(&path, &document, &sources).unwrap();
    let bytes = fs::read(&path).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(archive.len(), 2);
    assert_eq!(archive.by_index(0).unwrap().name(), "document.json");
    {
        let document_entry = archive.by_index(0).unwrap();
        assert_eq!(document_entry.compression(), zip::CompressionMethod::Stored);
        assert_eq!(document_entry.last_modified().unwrap().year(), 1980);
        assert_eq!(document_entry.unix_mode(), Some(0o100644));
    }
    {
        let source_entry = archive.by_index(1).unwrap();
        assert_eq!(source_entry.compression(), zip::CompressionMethod::Stored);
        assert_eq!(source_entry.last_modified().unwrap().year(), 1980);
        assert_eq!(source_entry.unix_mode(), Some(0o100644));
    }
    let mut json = String::new();
    archive
        .by_index(0)
        .unwrap()
        .read_to_string(&mut json)
        .unwrap();
    assert!(json.ends_with('\n'));
    for forbidden in [
        "history",
        "revision",
        "dirty",
        "savepoint",
        "window",
        "recovery",
        "scheduler",
        "cache",
        "decoded",
        "path",
    ] {
        assert!(
            !json.contains(forbidden),
            "unexpected transient key: {forbidden}"
        );
    }
    fs::remove_file(path).unwrap();
}

#[test]
fn archive_topology_names_formats_and_integrity_fail_without_panic() {
    let (document, sources) = legacy_document();
    let (json, source_name, source) = saved_parts(&document, &sources);
    assert_load_error(
        &archive_from_entries(&[("document.json", &json, zip::CompressionMethod::Stored)]),
        |e| matches!(e, LoadError::EntryTopology { .. }),
    );
    assert_load_error(
        &archive_from_entries(&[
            ("document.json", &json, zip::CompressionMethod::Stored),
            (
                "sources/source-1.svg",
                &source,
                zip::CompressionMethod::Stored,
            ),
            ("extra", b"x", zip::CompressionMethod::Stored),
        ]),
        |e| matches!(e, LoadError::EntryTopology { .. }),
    );
    let mut duplicate = archive_from_entries(&[
        (
            "sources/source-1.svg",
            &json,
            zip::CompressionMethod::Stored,
        ),
        (
            "sources/source-2.svg",
            &source,
            zip::CompressionMethod::Stored,
        ),
    ]);
    replace_central_name(&mut duplicate, 1, b"sources/source-1.svg");
    assert_load_error(&duplicate, |e| matches!(e, LoadError::EntryTopology { .. }));
    for unsafe_name in [
        "/sources/source-1.svg",
        "sources/../source-1.svg",
        "sources\\source-1.svg",
        "sources/\u{0001}.svg",
    ] {
        assert_load_error(
            &archive_from_entries(&[
                ("document.json", &json, zip::CompressionMethod::Stored),
                (unsafe_name, &source, zip::CompressionMethod::Stored),
            ]),
            |e| matches!(e, LoadError::EntryTopology { .. }),
        );
    }
    assert_load_error(
        &archive_from_entries(&[
            ("document.json", b"not json", zip::CompressionMethod::Stored),
            (&source_name, &source, zip::CompressionMethod::Stored),
        ]),
        |e| matches!(e, LoadError::Json { .. }),
    );
    assert_load_error(b"not a zip", |e| matches!(e, LoadError::Archive { .. }));
    let mut changed = json.clone();
    changed[0] = b'!';
    assert_load_error(
        &archive_from_entries(&[
            ("document.json", &changed, zip::CompressionMethod::Stored),
            (&source_name, &source, zip::CompressionMethod::Stored),
        ]),
        |e| matches!(e, LoadError::Json { .. }),
    );
}

#[test]
fn v1_reader_accepts_deflated_files_and_optional_sources_directory_marker() {
    let (document, sources) = legacy_document();
    let (json, source_name, source) = saved_parts(&document, &sources);
    for document_method in [
        zip::CompressionMethod::Stored,
        zip::CompressionMethod::Deflated,
    ] {
        for source_method in [
            zip::CompressionMethod::Stored,
            zip::CompressionMethod::Deflated,
        ] {
            let bytes = archive_from_entries(&[
                ("document.json", &json, document_method),
                (&source_name, &source, source_method),
            ]);
            let path = write_bytes(&bytes);
            let loaded = load(&path).unwrap();
            assert_eq!(loaded.document(), &document);
            assert_eq!(loaded.sources(), &sources);
            fs::remove_file(path).unwrap();

            let marked = archive_with_sources_marker(
                (&json, document_method),
                &source_name,
                (&source, source_method),
            );
            let path = write_bytes(&marked);
            let loaded = load(&path).unwrap();
            assert_eq!(loaded.document(), &document);
            assert_eq!(loaded.sources(), &sources);
            fs::remove_file(path).unwrap();
        }
    }
}

#[test]
fn v1_reader_rejects_noncanonical_directory_topology_and_compression() {
    let (document, sources) = legacy_document();
    let (json, source_name, source) = saved_parts(&document, &sources);
    for directory in ["source/", "metadata/"] {
        assert_load_error(
            &archive_with_directory_and_files(directory, &json, &source_name, &source),
            |error| matches!(error, LoadError::EntryTopology { .. }),
        );
    }
    let mut nonempty_marker = archive_with_sources_marker(
        (&json, zip::CompressionMethod::Stored),
        &source_name,
        (&source, zip::CompressionMethod::Stored),
    );
    set_central_entry_sizes(&mut nonempty_marker, 1, 1);
    set_local_entry_sizes(&mut nonempty_marker, 1, 1);
    assert_load_error(
        &nonempty_marker,
        |error| matches!(error, LoadError::EntryTopology { context } if context.contains("sources/")),
    );
    let mut unsupported = archive_from_entries(&[
        ("document.json", &json, zip::CompressionMethod::Stored),
        (&source_name, &source, zip::CompressionMethod::Stored),
    ]);
    let local = unsupported
        .windows(4)
        .enumerate()
        .find_map(|(index, value)| (value == b"PK\x03\x04").then_some(index))
        .unwrap();
    unsupported[local + 8..local + 10].copy_from_slice(&12_u16.to_le_bytes());
    let central = unsupported
        .windows(4)
        .enumerate()
        .find_map(|(index, value)| (value == b"PK\x01\x02").then_some(index))
        .unwrap();
    unsupported[central + 10..central + 12].copy_from_slice(&12_u16.to_le_bytes());
    assert_load_error(
        &unsupported,
        |error| matches!(error, LoadError::Archive { context } if context.contains("document.json") && context.contains("Unsupported(12)")),
    );
}

#[test]
fn manifests_versions_and_invalid_domain_are_rejected() {
    let (document, sources) = legacy_document();
    let (json, source_name, source) = saved_parts(&document, &sources);
    for (needle, replacement, category) in [
        (
            "\"container_version\":1",
            "\"container_version\":2",
            "version",
        ),
        (
            "\"document_schema_version\":2",
            "\"document_schema_version\":3",
            "version",
        ),
        (
            "\"entry_name\":\"sources/source-1.svg\"",
            "\"entry_name\":\"sources/source-1.png\"",
            "topology",
        ),
        ("\"byte_length\":6", "\"byte_length\":7", "integrity"),
        ("\"sha256\":\"", "\"sha256\":\"0", "integrity"),
        ("\"width\":101.25", "\"width\":0", "domain"),
    ] {
        let altered = String::from_utf8(json.clone())
            .unwrap()
            .replacen(needle, replacement, 1)
            .into_bytes();
        assert_load_error(
            &archive_from_entries(&[
                ("document.json", &altered, zip::CompressionMethod::Stored),
                (&source_name, &source, zip::CompressionMethod::Stored),
            ]),
            |error| match category {
                "version" => matches!(error, LoadError::Version { .. }),
                "topology" => matches!(error, LoadError::EntryTopology { .. }),
                "integrity" => matches!(error, LoadError::Integrity { .. }),
                _ => matches!(error, LoadError::DomainValidation { .. }),
            },
        );
    }
}

#[test]
fn bundle_constructors_and_save_correspondence_are_strict() {
    let id = SourceReferenceId::new("source-1").unwrap();
    assert!(
        EmbeddedSource::new(
            id.clone(),
            EmbeddedSourceFormat::Png,
            Vec::<u8>::new(),
            None
        )
        .is_err()
    );
    assert!(
        EmbeddedSource::new(
            SourceReferenceId::new("x..y").unwrap(),
            EmbeddedSourceFormat::Png,
            b"x".to_vec(),
            None
        )
        .is_err()
    );
    let a =
        EmbeddedSource::new(id.clone(), EmbeddedSourceFormat::Png, b"x".to_vec(), None).unwrap();
    let b = EmbeddedSource::new(id, EmbeddedSourceFormat::Png, b"y".to_vec(), None).unwrap();
    assert!(SourceBundle::new([a, b]).is_err());
    let (document, sources) = legacy_document();
    let wrong = SourceBundle::new([EmbeddedSource::new(
        SourceReferenceId::new("other").unwrap(),
        EmbeddedSourceFormat::Png,
        b"x".to_vec(),
        None,
    )
    .unwrap()])
    .unwrap();
    assert!(matches!(
        save(&temporary("mismatch.toniator"), &document, &wrong),
        Err(toniator_io::SaveError::SourceDocumentMismatch { .. })
    ));
    let _ = sources;
}

#[test]
fn archive_size_limit_is_checked_before_zip_opening() {
    let path = temporary("oversized.toniator");
    let file = fs::File::create(&path).unwrap();
    file.set_len(MAX_ARCHIVE_BYTES + 1).unwrap();
    assert!(matches!(load(&path), Err(LoadError::Limits { .. })));
    fs::remove_file(path).unwrap();
}

#[test]
fn entry_limits_are_checked_from_zip_metadata_before_reading_payloads() {
    let (document, sources) = legacy_document();
    let path = temporary("limit-base.toniator");
    save(&path, &document, &sources).unwrap();
    let base = fs::read(&path).unwrap();
    fs::remove_file(path).unwrap();
    let mut oversized_document = base.clone();
    set_central_entry_sizes(&mut oversized_document, 0, MAX_DOCUMENT_BYTES + 1);
    assert_load_error(&oversized_document, |error| {
        matches!(error, LoadError::Limits { .. })
    });
    let mut oversized_source = base;
    set_central_entry_sizes(&mut oversized_source, 1, MAX_SOURCE_BYTES + 1);
    assert_load_error(&oversized_source, |error| {
        matches!(error, LoadError::Limits { .. })
    });
    let aggregate_path = temporary("aggregate-base.toniator");
    save(&aggregate_path, &document, &sources).unwrap();
    let mut aggregate = fs::read(&aggregate_path).unwrap();
    fs::remove_file(aggregate_path).unwrap();
    set_central_entry_sizes(&mut aggregate, 0, MAX_DOCUMENT_BYTES);
    set_central_entry_sizes(&mut aggregate, 1, MAX_SOURCE_BYTES + 1);
    assert_load_error(
        &aggregate,
        |error| matches!(error, LoadError::Limits { context } if context.contains("total uncompressed data exceeds the 132 MiB")),
    );
    assert_eq!(
        MAX_DOCUMENT_BYTES + MAX_SOURCE_BYTES,
        132 * 1024 * 1024,
        "the two-entry v1 aggregate limit is exactly its individually allowed boundary"
    );
}

#[test]
fn deflated_document_with_understated_metadata_is_limited_while_reading() {
    let (document, sources) = legacy_document();
    let (_, source_name, source) = saved_parts(&document, &sources);
    let oversized_document = vec![b' '; usize::try_from(MAX_DOCUMENT_BYTES + 1).unwrap()];
    let mut bytes = archive_from_entries(&[
        (
            "document.json",
            &oversized_document,
            zip::CompressionMethod::Deflated,
        ),
        (&source_name, &source, zip::CompressionMethod::Stored),
    ]);
    set_central_uncompressed_size(&mut bytes, 0, 1);
    assert_load_error(
        &bytes,
        |error| matches!(error, LoadError::Limits { context } if context.contains("document.json")),
    );
}

#[test]
fn encrypted_flag_is_rejected_without_needing_an_encrypted_writer() {
    let (document, sources) = legacy_document();
    let path = temporary("encrypted-base.toniator");
    save(&path, &document, &sources).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    fs::remove_file(path).unwrap();
    let positions = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(index, value)| (value == b"PK\x01\x02").then_some(index))
        .collect::<Vec<_>>();
    for position in positions {
        let flags = u16::from_le_bytes([bytes[position + 8], bytes[position + 9]]) | 1;
        bytes[position + 8..position + 10].copy_from_slice(&flags.to_le_bytes());
    }
    assert_load_error(
        &bytes,
        |error| matches!(error, LoadError::Archive { context } if context.contains("encrypted")),
    );
}

#[test]
fn post_write_rename_failure_cleans_only_the_attempted_temp() {
    let (document, sources) = legacy_document();
    let root = temporary("rename-failure-root");
    fs::create_dir(&root).unwrap();
    let directory_target = root.join("destination.toniator");
    fs::create_dir(&directory_target).unwrap();
    let parent = &root;
    let before: HashSet<_> = fs::read_dir(parent)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(matches!(
        save(&directory_target, &document, &sources),
        Err(toniator_io::SaveError::Filesystem { .. })
    ));
    let after: HashSet<_> = fs::read_dir(parent)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(
        before, after,
        "rename failure must remove its create-new temporary file"
    );
    fs::remove_dir(directory_target).unwrap();
    fs::remove_dir(root).unwrap();
}
