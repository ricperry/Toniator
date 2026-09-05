use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use toniator_domain::{
    PatternDefinitionDraft, PatternDefinitionRecipe, PatternStructureRecipe, PresetMetadata,
    PresetRecord,
};
use toniator_io::personal_library::{PersonalEntryKind, PersonalLibrary, PersonalLibraryPaths};

/// Creates an isolated final-gate library without addressing the user's configured folders.
fn library(label: &str) -> (PathBuf, PersonalLibrary) {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "toniator-21b4-{label}-{}-{suffix}",
        std::process::id()
    ));
    let library = PersonalLibrary::initialize(PersonalLibraryPaths {
        root: root.join("library"),
        config_file: root.join("config/library.json"),
    })
    .unwrap();
    (root, library)
}

/// Produces a current validated recipe; only metadata changes in filesystem publication tests.
fn record(name: &str) -> PresetRecord {
    PresetRecord {
        metadata: PresetMetadata {
            id: "user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
            name: name.into(),
            category: "Guides".into(),
            description: "Final-gate publication fixture".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::StraightGrid(
            PatternDefinitionDraft {
                name: "Final-gate grid".into(),
                coverage: toniator_domain::CoveragePolicy {
                    guard_steps: 2,
                    additional_margin: 0.0,
                },
            },
        )),
    }
}

/// Keeps complete JSON visible to concurrent readers while one writer publishes, then rejects a stale observer.
/// This tests the documented single-writer contract, not a cross-process compare-and-swap promise.
#[test]
fn single_writer_publication_is_atomic_to_readers_and_stale_observers() {
    let (root, library) = library("readers");
    let mut value = record("Version 0");
    let first = library.write_preset(&value, None).unwrap();
    let path = library
        .root()
        .join("presets")
        .join(format!("{}.preset.json", value.metadata.id));
    let running = Arc::new(AtomicBool::new(true));
    let reader_running = Arc::clone(&running);
    let reader_path = path.clone();
    let reader = std::thread::spawn(move || {
        let mut reads = 0;
        while reader_running.load(Ordering::Acquire) || reads == 0 {
            let bytes = fs::read(&reader_path).unwrap();
            let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(json["preset_format_version"], 4);
            assert!(
                json["metadata"]["name"]
                    .as_str()
                    .unwrap()
                    .starts_with("Version ")
            );
            reads += 1;
        }
        reads
    });
    let mut expected = first.clone();
    for revision in 1..=24 {
        value.metadata.name = format!("Version {revision}");
        expected = library.write_preset(&value, Some(&expected)).unwrap();
    }
    running.store(false, Ordering::Release);
    assert!(reader.join().unwrap() > 0);
    let final_bytes = fs::read(&path).unwrap();
    value.metadata.name = "Stale overwrite".into();
    assert!(library.write_preset(&value, Some(&first)).is_err());
    assert_eq!(fs::read(&path).unwrap(), final_bytes);
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

/// Rejects traversal, symlink destinations, and unwritable publication while preserving external bytes.
#[cfg(unix)]
#[test]
fn unsafe_paths_and_permission_failures_preserve_existing_content() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let (root, library) = library("paths");
    let mut value = record("Safe Pattern");
    let id = value.metadata.id.clone();
    value.metadata.id = "../outside".into();
    assert!(library.write_preset(&value, None).is_err());
    value.metadata.id = id.clone();
    let outside = root.join("outside");
    fs::write(&outside, "protected bytes").unwrap();
    let path = library
        .root()
        .join("presets")
        .join(format!("{id}.preset.json"));
    symlink(&outside, &path).unwrap();
    assert!(library.write_preset(&value, None).is_err());
    assert!(library.trash(PersonalEntryKind::Preset, &id).is_err());
    assert_eq!(fs::read_to_string(&outside).unwrap(), "protected bytes");
    fs::remove_file(&path).unwrap();
    let fingerprint = library.write_preset(&value, None).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let directory = path.parent().unwrap();
    let original_permissions = fs::metadata(directory).unwrap().permissions();
    fs::set_permissions(directory, fs::Permissions::from_mode(0o500)).unwrap();
    value.metadata.name = "Blocked rename".into();
    let result = library.write_preset(&value, Some(&fingerprint));
    fs::set_permissions(directory, original_permissions).unwrap();
    if unsafe { libc::geteuid() } != 0 {
        assert!(
            result.is_err(),
            "unprivileged publication respects directory permissions"
        );
        assert_eq!(
            library.scan().unwrap().presets[0].preset.metadata.name,
            "Safe Pattern"
        );
    }
    fs::remove_dir_all(root).unwrap();
}

/// Refuses both trash and Undo destination collisions without overwriting a newer entry.
#[test]
fn trash_and_undo_collisions_preserve_both_versions() {
    let (root, library) = library("trash");
    let value = record("Original Pattern");
    library.write_preset(&value, None).unwrap();
    let filename = format!("{}.preset.json", value.metadata.id);
    let active = library.root().join("presets").join(&filename);
    let trashed = library.root().join(".trash").join(&filename);
    fs::write(&trashed, "old trash content").unwrap();
    assert!(
        library
            .trash(PersonalEntryKind::Preset, &value.metadata.id)
            .is_err()
    );
    assert!(active.exists());
    assert_eq!(fs::read_to_string(&trashed).unwrap(), "old trash content");
    fs::remove_file(&trashed).unwrap();
    let token = library
        .trash(PersonalEntryKind::Preset, &value.metadata.id)
        .unwrap();
    let mut replacement = value.clone();
    replacement.metadata.name = "Newer replacement".into();
    library.write_preset(&replacement, None).unwrap();
    assert!(library.undo_trash(token).is_err());
    assert!(trashed.exists());
    assert_eq!(library.scan().unwrap().presets[0].preset, replacement);
    fs::remove_dir_all(root).unwrap();
}
