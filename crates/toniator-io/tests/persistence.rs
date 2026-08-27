//! Current-schema container integration witnesses.
//!
//! Toniator is pre-release, so obsolete schema migration belongs nowhere in this suite. These
//! tests exercise the public current-v5 boundary, archive hardening, deterministic bytes,
//! transactional publication, source correspondence, and Stage 20 visible-mark exclusion intent.

use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use toniator_domain::{
    CanvasSpec, CoveragePolicy, Document, PatternDefinition, PatternMechanismId,
    PatternOutputLayerId, RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy,
    SourceReference, SourceReferenceId,
};
use toniator_io::{
    DOCUMENT_SCHEMA_VERSION, EmbeddedSource, EmbeddedSourceFormat, LoadError, MAX_ARCHIVE_BYTES,
    MAX_DOCUMENT_BYTES, MAX_SOURCE_BYTES, MAX_UNCOMPRESSED_BYTES, SourceBundle, load, save,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Allocates one collision-resistant path for a disposable container witness.
fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "toniator-current-persistence-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time follows epoch")
            .as_nanos()
    ))
}

/// Builds a current source-backed document and its exactly corresponding source bundle.
fn source_backed_document() -> (Document, SourceBundle) {
    let source_id = SourceReferenceId::new("current-source").expect("source ID validates");
    let document = Document::new_default_document(
        CanvasSpec {
            width: 160.0,
            height: 90.0,
        },
        SourceReference::Assigned(source_id.clone()),
    )
    .expect("default document validates");
    let source = EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec(),
        Some("source.svg".into()),
    )
    .expect("embedded source validates");
    (
        document,
        SourceBundle::new([source]).expect("one source bundle validates"),
    )
}

/// Rebuilds the source-backed document with active visible-mark-margin exclusion intent.
fn visible_mark_margin_document() -> (Document, SourceBundle) {
    let (base, sources) = source_backed_document();
    let definition = PatternDefinition::random_sites(
        base.pattern_settings().definition_id,
        "visible support exclusion",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternMechanismId(4),
        PatternOutputLayerId(1),
        RandomSiteCharacter::RawUniform,
        42,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::VisibleMarkMargin { margin: 2.75 },
        20_000,
        20_000,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    let mut bundle = base.pattern_definition_bundles()[0].clone();
    bundle.definition = definition;
    let document = Document::with_source_and_topology(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![bundle],
        base.pattern_settings().clone(),
        base.channel_model().expect("modeled document").to_owned(),
        base.channel_topology().expect("modeled document").clone(),
    )
    .expect("visible-mark exclusion document validates");
    (document, sources)
}

/// Reads one named member from a derived test container without extracting it.
fn archive_entry(path: &Path, name: &str) -> Vec<u8> {
    let mut archive =
        ZipArchive::new(File::open(path).expect("container opens")).expect("container is ZIP");
    let mut bytes = Vec::new();
    archive
        .by_name(name)
        .expect("named member exists")
        .read_to_end(&mut bytes)
        .expect("member reads");
    bytes
}

/// Copies a derived container and appends one forbidden archive member for topology rejection.
fn append_extra_member(source: &Path, destination: &Path) {
    let mut input = ZipArchive::new(File::open(source).expect("source container opens"))
        .expect("source is ZIP");
    let mut entries = Vec::new();
    for index in 0..input.len() {
        let mut entry = input.by_index(index).expect("entry opens");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("entry reads");
        entries.push((entry.name().to_owned(), bytes));
    }
    entries.push(("unexpected.bin".into(), vec![1, 2, 3]));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o100644);
    let mut writer = ZipWriter::new(File::create(destination).expect("destination creates"));
    for (name, bytes) in entries {
        writer.start_file(name, options).expect("entry starts");
        writer.write_all(&bytes).expect("entry writes");
    }
    writer.finish().expect("container finishes");
}

/// Returns current document JSON, its canonical source entry name, and exact source bytes.
fn saved_parts(document: &Document, sources: &SourceBundle) -> (Vec<u8>, String, Vec<u8>) {
    let path = temporary("parts.toniator");
    save(&path, document, sources).expect("current parts save");
    let bytes = fs::read(&path).expect("current container reads");
    fs::remove_file(path).expect("temporary parts container removes");
    let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).expect("saved container is ZIP");
    let mut json = Vec::new();
    archive
        .by_name("document.json")
        .expect("document member exists")
        .read_to_end(&mut json)
        .expect("document member reads");
    let source_name = archive
        .by_index(1)
        .expect("source member exists")
        .name()
        .to_owned();
    let mut source = Vec::new();
    archive
        .by_name(&source_name)
        .expect("named source member exists")
        .read_to_end(&mut source)
        .expect("source member reads");
    (json, source_name, source)
}

/// Builds one in-memory ZIP from explicitly named members and compression methods.
fn archive_from_entries(entries: &[(&str, &[u8], CompressionMethod)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes, method) in entries {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(*method),
            )
            .expect("hostile archive member starts");
        writer
            .write_all(bytes)
            .expect("hostile archive member writes");
    }
    writer
        .finish()
        .expect("hostile archive finishes")
        .into_inner()
}

/// Adds a duplicate source central-directory record without changing the declared entry counts.
///
/// This witnesses the hostile shape where an ordinary ZIP reader honors the two-entry EOCD count
/// and ignores a third record that is nevertheless included in the central-directory byte span.
fn duplicate_source_member(mut bytes: Vec<u8>) -> Vec<u8> {
    let end = bytes
        .windows(4)
        .rposition(|value| value == b"PK\x05\x06")
        .expect("ZIP end record exists");
    let central = bytes[..end]
        .windows(4)
        .enumerate()
        .filter_map(|(index, value)| (value == b"PK\x01\x02").then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(central.len(), 2);
    let source = central[1];
    let name_length = u16::from_le_bytes([bytes[source + 28], bytes[source + 29]]) as usize;
    let extra_length = u16::from_le_bytes([bytes[source + 30], bytes[source + 31]]) as usize;
    let comment_length = u16::from_le_bytes([bytes[source + 32], bytes[source + 33]]) as usize;
    let record_length = 46 + name_length + extra_length + comment_length;
    let duplicate = bytes[source..source + record_length].to_vec();
    bytes.splice(end..end, duplicate);
    let shifted_end = end + record_length;
    let central_size = u32::from_le_bytes([
        bytes[shifted_end + 12],
        bytes[shifted_end + 13],
        bytes[shifted_end + 14],
        bytes[shifted_end + 15],
    ]);
    bytes[shifted_end + 12..shifted_end + 16]
        .copy_from_slice(&(central_size + record_length as u32).to_le_bytes());
    bytes
}

/// Builds an archive containing one explicit directory marker plus canonical files.
fn archive_with_directory_marker(
    directory: &str,
    document: &[u8],
    source_name: &str,
    source: &[u8],
) -> Vec<u8> {
    let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .start_file("document.json", SimpleFileOptions::default())
        .expect("document member starts");
    writer.write_all(document).expect("document member writes");
    writer
        .add_directory(directory, SimpleFileOptions::default())
        .expect("directory marker writes");
    writer
        .start_file(source_name, SimpleFileOptions::default())
        .expect("source member starts");
    writer.write_all(source).expect("source member writes");
    writer
        .finish()
        .expect("marked archive finishes")
        .into_inner()
}

/// Overwrites central-directory compressed and uncompressed sizes for one indexed member.
fn set_central_entry_sizes(bytes: &mut [u8], entry_index: usize, size: u64) {
    let positions = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(index, value)| (value == b"PK\x01\x02").then_some(index))
        .collect::<Vec<_>>();
    let position = positions[entry_index];
    let size = u32::try_from(size)
        .expect("test limit fits ZIP32")
        .to_le_bytes();
    bytes[position + 20..position + 24].copy_from_slice(&size);
    bytes[position + 24..position + 28].copy_from_slice(&size);
}

/// Understates one central-directory uncompressed size without changing its deflated payload.
fn set_central_uncompressed_size(bytes: &mut [u8], entry_index: usize, size: u64) {
    let positions = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(index, value)| (value == b"PK\x01\x02").then_some(index))
        .collect::<Vec<_>>();
    let position = positions[entry_index];
    let size = u32::try_from(size)
        .expect("test size fits ZIP32")
        .to_le_bytes();
    bytes[position + 24..position + 28].copy_from_slice(&size);
}

/// Writes hostile container bytes to one disposable path.
fn write_bytes(bytes: &[u8]) -> PathBuf {
    let path = temporary("hostile.toniator");
    fs::write(&path, bytes).expect("hostile bytes write");
    path
}

/// Loads hostile bytes without allowing a panic and checks their public error category.
fn assert_load_error(bytes: &[u8], predicate: impl FnOnce(&LoadError) -> bool) {
    let byte_length = bytes.len();
    let path = write_bytes(bytes);
    let result = std::panic::catch_unwind(|| load(&path));
    assert!(result.is_ok(), "hostile input must not panic");
    let error = match result.expect("load did not panic") {
        Ok(_) => panic!("hostile input ({byte_length} bytes) rejects"),
        Err(error) => error,
    };
    assert!(predicate(&error), "unexpected error: {error}");
    fs::remove_file(path).expect("hostile input removes");
}

/// Proves current-v5 saves are byte deterministic and reload exact authoritative state.
#[test]
fn current_v5_round_trip_is_byte_deterministic() {
    assert_eq!(DOCUMENT_SCHEMA_VERSION, 5);
    let (document, sources) = source_backed_document();
    let first = temporary("first.toniator");
    let second = temporary("second.toniator");
    save(&first, &document, &sources).expect("first save succeeds");
    save(&second, &document, &sources).expect("second save succeeds");
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let loaded = load(&first).expect("current container reloads");
    assert_eq!(loaded.versions().document(), DOCUMENT_SCHEMA_VERSION);
    assert_eq!(loaded.document(), &document);
    assert_eq!(loaded.sources(), &sources);
    fs::remove_file(first).unwrap();
    fs::remove_file(second).unwrap();
}

/// Proves Stage 20 visible-mark exclusion persists only margin intent and no derived support value.
#[test]
fn visible_mark_margin_round_trip_excludes_derived_support() {
    let (document, sources) = visible_mark_margin_document();
    let path = temporary("visible-margin.toniator");
    save(&path, &document, &sources).expect("visible-margin document saves");
    let json = String::from_utf8(archive_entry(&path, "document.json")).unwrap();
    assert!(json.contains("\"visible_mark_margin\""));
    assert!(json.contains("\"margin\":2.75"));
    assert!(!json.contains("support_radius"));
    assert!(!json.contains("sizing"));
    assert_eq!(load(&path).unwrap().document(), &document);
    fs::remove_file(path).unwrap();
}

/// Proves a mismatched or missing source fails before replacing an existing destination.
#[test]
fn failed_save_preserves_existing_destination_transactionally() {
    let (document, _) = source_backed_document();
    let path = temporary("transaction.toniator");
    fs::write(&path, b"existing bytes").expect("sentinel writes");
    let empty = SourceBundle::new([]).expect("empty bundle itself validates");
    let error = save(&path, &document, &empty).expect_err("source mismatch rejects");
    assert_eq!(error.path(), "source.document");
    assert_eq!(fs::read(&path).unwrap(), b"existing bytes");
    fs::remove_file(path).unwrap();
}

/// Proves an extra archive member is rejected without extracting or partially publishing state.
#[test]
fn extra_archive_member_is_rejected_at_topology_boundary() {
    let (document, sources) = source_backed_document();
    let current = temporary("topology-current.toniator");
    let invalid = temporary("topology-invalid.toniator");
    save(&current, &document, &sources).expect("current container saves");
    append_extra_member(&current, &invalid);
    let error = load(&invalid).expect_err("extra member rejects");
    assert!(matches!(error, LoadError::EntryTopology { .. }));
    fs::remove_file(current).unwrap();
    fs::remove_file(invalid).unwrap();
}

/// Proves duplicate source IDs and document/source correspondence remain strict public contracts.
#[test]
fn source_bundle_identity_is_strict() {
    let id = SourceReferenceId::new("duplicate").unwrap();
    let source = EmbeddedSource::new(id, EmbeddedSourceFormat::Png, vec![1, 2, 3], None).unwrap();
    assert!(SourceBundle::new([source.clone(), source]).is_err());
}

/// Proves current archives retain deterministic metadata and exclude transient evaluator state.
#[test]
fn current_archive_metadata_and_json_are_deterministic_and_intent_only() {
    let (document, sources) = source_backed_document();
    let path = temporary("metadata.toniator");
    save(&path, &document, &sources).expect("metadata witness saves");
    let bytes = fs::read(&path).expect("metadata witness reads");
    let mut archive =
        ZipArchive::new(std::io::Cursor::new(bytes)).expect("metadata witness is ZIP");
    assert_eq!(archive.len(), 2);
    assert_eq!(archive.by_index(0).unwrap().name(), "document.json");
    for index in 0..archive.len() {
        let entry = archive.by_index(index).expect("metadata entry opens");
        assert_eq!(entry.compression(), CompressionMethod::Stored);
        assert_eq!(entry.last_modified().expect("fixed timestamp").year(), 1980);
        assert_eq!(entry.unix_mode(), Some(0o100644));
    }
    let mut json = String::new();
    archive
        .by_name("document.json")
        .expect("document member opens")
        .read_to_string(&mut json)
        .expect("document JSON reads");
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
        "performance",
        "worker",
    ] {
        assert!(
            !json.contains(forbidden),
            "unexpected transient key: {forbidden}"
        );
    }
    fs::remove_file(path).expect("metadata witness removes");
}

/// Proves source length and digest correspondence reject changed payloads without extraction.
#[test]
fn source_length_and_hash_integrity_are_checked_before_publication() {
    let (document, sources) = source_backed_document();
    let (json, source_name, source) = saved_parts(&document, &sources);
    let mut same_length = source.clone();
    same_length[0] ^= 1;
    for changed in [same_length, b"different length".to_vec()] {
        assert_load_error(
            &archive_from_entries(&[
                ("document.json", &json, CompressionMethod::Stored),
                (&source_name, &changed, CompressionMethod::Stored),
            ]),
            |error| matches!(error, LoadError::Integrity { .. }),
        );
    }
}

/// Proves hostile names, duplicate members, malformed payloads, and unsupported compression reject.
#[test]
fn archive_topology_and_compression_reject_without_panic() {
    let (document, sources) = source_backed_document();
    let (json, source_name, source) = saved_parts(&document, &sources);
    assert_load_error(
        &archive_from_entries(&[("document.json", &json, CompressionMethod::Stored)]),
        |error| matches!(error, LoadError::EntryTopology { .. }),
    );
    for unsafe_name in [
        "/sources/current-source.svg",
        "sources/../current-source.svg",
        "sources\\current-source.svg",
        "sources/\u{0001}urrent-source.svg",
    ] {
        assert_load_error(
            &archive_from_entries(&[
                ("document.json", &json, CompressionMethod::Stored),
                (unsafe_name, &source, CompressionMethod::Stored),
            ]),
            |error| matches!(error, LoadError::EntryTopology { .. }),
        );
    }
    let alternate_name = source_name.replace("source.svg", "sourcf.svg");
    let multiple_sources = archive_from_entries(&[
        ("document.json", &json, CompressionMethod::Stored),
        (&source_name, &source, CompressionMethod::Stored),
        (&alternate_name, &source, CompressionMethod::Stored),
    ]);
    assert_load_error(&multiple_sources, |error| {
        matches!(error, LoadError::EntryTopology { .. })
    });
    let duplicate = duplicate_source_member(archive_from_entries(&[
        ("document.json", &json, CompressionMethod::Stored),
        (&source_name, &source, CompressionMethod::Stored),
    ]));
    let mut duplicate_archive = ZipArchive::new(Cursor::new(&duplicate)).unwrap();
    let duplicate_names = (0..duplicate_archive.len())
        .map(|index| {
            duplicate_archive
                .by_index_raw(index)
                .unwrap()
                .name()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        duplicate_names,
        vec!["document.json".to_owned(), source_name.clone()],
        "the ZIP reader collapses the repeated raw central-directory name"
    );
    assert_load_error(
        &duplicate,
        |error| matches!(error, LoadError::EntryTopology { context } if context.contains("duplicate or conflicting")),
    );
    assert_load_error(b"not a zip", |error| {
        matches!(error, LoadError::Archive { .. })
    });
    assert_load_error(
        &archive_from_entries(&[
            ("document.json", b"not json", CompressionMethod::Stored),
            (&source_name, &source, CompressionMethod::Stored),
        ]),
        |error| matches!(error, LoadError::Json { .. }),
    );
    let mut unsupported = archive_from_entries(&[
        ("document.json", &json, CompressionMethod::Stored),
        (&source_name, &source, CompressionMethod::Stored),
    ]);
    let local = unsupported
        .windows(4)
        .enumerate()
        .find_map(|(index, value)| (value == b"PK\x03\x04").then_some(index))
        .expect("local ZIP header exists");
    unsupported[local + 8..local + 10].copy_from_slice(&12_u16.to_le_bytes());
    let central = unsupported
        .windows(4)
        .enumerate()
        .find_map(|(index, value)| (value == b"PK\x01\x02").then_some(index))
        .expect("central ZIP header exists");
    unsupported[central + 10..central + 12].copy_from_slice(&12_u16.to_le_bytes());
    assert_load_error(
        &unsupported,
        |error| matches!(error, LoadError::Archive { context } if context.contains("Unsupported(12)")),
    );
}

/// Proves accepted deflate input works while noncanonical directory topology rejects.
#[test]
fn deflated_current_members_load_and_noncanonical_directories_reject() {
    let (document, sources) = source_backed_document();
    let (json, source_name, source) = saved_parts(&document, &sources);
    let deflated = archive_from_entries(&[
        ("document.json", &json, CompressionMethod::Deflated),
        (&source_name, &source, CompressionMethod::Deflated),
    ]);
    let path = write_bytes(&deflated);
    let loaded = load(&path).expect("deflated current archive loads");
    assert_eq!(loaded.document(), &document);
    assert_eq!(loaded.sources(), &sources);
    fs::remove_file(path).expect("deflated witness removes");
    for directory in ["source/", "metadata/"] {
        assert_load_error(
            &archive_with_directory_marker(directory, &json, &source_name, &source),
            |error| matches!(error, LoadError::EntryTopology { .. }),
        );
    }
}

/// Proves archive and member limits reject from metadata and during understated deflate reads.
#[test]
fn archive_and_entry_limits_are_enforced_before_publication() {
    let oversized_path = temporary("oversized.toniator");
    let file = File::create(&oversized_path).expect("oversized sparse file creates");
    file.set_len(MAX_ARCHIVE_BYTES + 1)
        .expect("oversized sparse file extends");
    assert!(matches!(
        load(&oversized_path),
        Err(LoadError::Limits { .. })
    ));
    fs::remove_file(oversized_path).expect("oversized sparse file removes");

    let (document, sources) = source_backed_document();
    let path = temporary("limit-base.toniator");
    save(&path, &document, &sources).expect("limit base saves");
    let base = fs::read(&path).expect("limit base reads");
    fs::remove_file(path).expect("limit base removes");
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
    assert_eq!(
        MAX_UNCOMPRESSED_BYTES,
        MAX_DOCUMENT_BYTES + MAX_SOURCE_BYTES
    );

    let (_, source_name, source) = saved_parts(&document, &sources);
    let oversized_json = vec![b' '; usize::try_from(MAX_DOCUMENT_BYTES + 1).unwrap()];
    let mut understated = archive_from_entries(&[
        (
            "document.json",
            &oversized_json,
            CompressionMethod::Deflated,
        ),
        (&source_name, &source, CompressionMethod::Stored),
    ]);
    set_central_uncompressed_size(&mut understated, 0, 1);
    assert_load_error(
        &understated,
        |error| matches!(error, LoadError::Limits { context } if context.contains("document.json")),
    );
}

/// Proves encrypted flags reject before member reads without requiring encrypted writer support.
#[test]
fn encrypted_entries_are_rejected_before_reading() {
    let (document, sources) = source_backed_document();
    let path = temporary("encrypted-base.toniator");
    save(&path, &document, &sources).expect("encrypted base saves");
    let mut bytes = fs::read(&path).expect("encrypted base reads");
    fs::remove_file(path).expect("encrypted base removes");
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

/// Proves failed final rename removes only its create-new temporary container.
#[test]
fn post_write_rename_failure_cleans_only_the_attempted_temp() {
    let (document, sources) = source_backed_document();
    let root = temporary("rename-failure-root");
    fs::create_dir(&root).expect("rename witness root creates");
    let directory_target = root.join("destination.toniator");
    fs::create_dir(&directory_target).expect("directory target creates");
    let before: HashSet<_> = fs::read_dir(&root)
        .expect("rename root reads")
        .map(|entry| entry.expect("rename root entry").file_name())
        .collect();
    assert!(save(&directory_target, &document, &sources).is_err());
    let after: HashSet<_> = fs::read_dir(&root)
        .expect("rename root rereads")
        .map(|entry| entry.expect("rename root entry").file_name())
        .collect();
    assert_eq!(before, after, "failed rename must clean its temporary file");
    fs::remove_dir(directory_target).expect("directory target removes");
    fs::remove_dir(root).expect("rename witness root removes");
}
