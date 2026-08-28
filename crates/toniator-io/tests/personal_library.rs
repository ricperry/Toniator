use std::{
    fs::{self, File},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft, PatternDefinitionDraft,
    PatternDefinitionRecipe, PatternStructureRecipe, PersonalAuthoredResource,
    PersonalAuthoredResourceKind, PersonalResourceId, PresetMetadata, PresetRecord,
};
use toniator_io::personal_library::{
    LibraryEnvironment, MAX_PERSONAL_LIBRARY_JSON_BYTES, PersonalEntryKind, PersonalLibrary,
    PersonalLibraryPaths,
};

/// Creates one isolated absolute test directory without addressing a user XDG path.
fn temporary_directory(label: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("toniator-{label}-{}-{suffix}", std::process::id()));
    fs::create_dir(&path).expect("isolated temporary directory creates");
    path
}

/// Builds one valid personal preset-v4 record without using a bundled registry.
fn preset(id: &str, name: &str) -> PresetRecord {
    PresetRecord {
        metadata: PresetMetadata {
            id: id.into(),
            name: name.into(),
            category: "Guides".into(),
            description: "A private test preset.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::StraightGrid(
            PatternDefinitionDraft {
                name: "private test grid".into(),
                coverage: toniator_domain::CoveragePolicy {
                    guard_steps: 2,
                    additional_margin: 0.0,
                },
            },
        )),
    }
}

/// Builds one valid exact open motif resource for strict v1 resource persistence.
fn motif(id: &str, name: &str) -> PersonalAuthoredResource {
    PersonalAuthoredResource::new(
        PersonalResourceId::new(id.into()).expect("test UUID is canonical"),
        name.into(),
        PersonalAuthoredResourceKind::Motif,
        AuthoredStructureDraft::new(
            toniator_domain::AuthoredStructureKind::OpenPath,
            vec![AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                end: AuthoredPoint2 { x: 1.0, y: 0.0 },
            }],
        )
        .expect("fixed motif validates"),
    )
    .expect("motif record validates")
}

/// Persists strict resource-v1 geometry and detects a stale preset replacement before publication.
#[test]
fn personal_library_round_trips_resources_and_rejects_stale_preset_overwrite() {
    let root = temporary_directory("library-round-trip");
    let config = root.join("config/library.json");
    let library = PersonalLibrary::initialize(PersonalLibraryPaths {
        root: root.join("data/Toniator"),
        config_file: config,
    })
    .expect("isolated library initializes");
    let motif_id = "user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let resource = motif(motif_id, "Asymmetric motif");
    let resource_fingerprint = library
        .write_resource(&resource, None)
        .expect("resource writes atomically");
    let preset_id = "user-bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    let first = library
        .write_preset(&preset(preset_id, "Private guides"), None)
        .expect("preset writes atomically");
    let changed = library
        .write_preset(&preset(preset_id, "Private guides renamed"), Some(&first))
        .expect("fresh fingerprint updates");
    assert_ne!(first, changed);
    assert!(
        library
            .write_preset(&preset(preset_id, "Stale write"), Some(&first))
            .is_err()
    );
    let snapshot = library.scan().expect("valid entries scan");
    assert_eq!(snapshot.motifs[0].resource, resource);
    assert_eq!(snapshot.motifs[0].fingerprint, resource_fingerprint);
    assert_eq!(
        snapshot.presets[0].preset.metadata.name,
        "Private guides renamed"
    );
    fs::remove_dir_all(root).expect("isolated temporary directory removes");
}

/// Resolves explicit XDG defaults, switches configuration without moving old content, and isolates malformed siblings.
#[test]
fn personal_library_switch_is_config_only_and_malformed_children_warn() {
    let root = temporary_directory("library-switch");
    let paths = PersonalLibraryPaths::default_for(&LibraryEnvironment {
        data_home: Some(root.join("xdg-data")),
        config_home: Some(root.join("xdg-config")),
        home: None,
    })
    .expect("explicit XDG inputs resolve");
    let mut library = PersonalLibrary::initialize(paths).expect("default library initializes");
    let old_root = library.root().to_path_buf();
    let alternative = root.join("alternate");
    fs::create_dir(&alternative).expect("existing alternative root creates");
    library
        .switch_root(&alternative)
        .expect("existing root switches");
    assert!(
        old_root.exists(),
        "switch never moves or deletes the old library"
    );
    let invalid = library
        .root()
        .join("motifs/user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.motif.json");
    fs::write(
        invalid,
        "{\"resource_format_version\": 1, \"unknown\": true}",
    )
    .expect("malformed sibling writes");
    let snapshot = library.scan().expect("malformed sibling remains nonfatal");
    assert_eq!(snapshot.motifs.len(), 0);
    assert_eq!(snapshot.warnings.len(), 1);
    fs::remove_dir_all(root).expect("isolated temporary directory removes");
}

/// Rejects a symlinked fixed library directory before scanning can follow outside content.
#[cfg(unix)]
#[test]
fn personal_library_rejects_symlinked_layout_directory() {
    use std::os::unix::fs::symlink;

    let root = temporary_directory("library-symlink");
    let library = PersonalLibrary::initialize(PersonalLibraryPaths {
        root: root.join("data/Toniator"),
        config_file: root.join("config/library.json"),
    })
    .expect("isolated library initializes");
    let motifs = library.root().join("motifs");
    fs::remove_dir(&motifs).expect("empty fixed directory removes for hostile replacement");
    let outside = root.join("outside");
    fs::create_dir(&outside).expect("isolated outside directory creates");
    symlink(&outside, &motifs).expect("hostile symlink creates");
    assert!(library.scan().is_err(), "symlinked library layout rejects");
    fs::remove_dir_all(root).expect("isolated temporary directory removes");
}

/// Rejects duplicate personal names before a second typed resource can be published.
#[test]
fn personal_library_rejects_case_insensitive_duplicate_resource_names() {
    let root = temporary_directory("library-duplicate-name");
    let library = PersonalLibrary::initialize(PersonalLibraryPaths {
        root: root.join("data/Toniator"),
        config_file: root.join("config/library.json"),
    })
    .expect("isolated library initializes");
    library
        .write_resource(
            &motif("user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", "Repeated"),
            None,
        )
        .expect("first resource writes");
    assert!(
        library
            .write_resource(
                &motif("user-bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", "repeated"),
                None,
            )
            .is_err()
    );
    fs::remove_dir_all(root).expect("isolated temporary directory removes");
}

/// Isolates strict v1 resource failures and actual stream-size overflow as nonfatal scan warnings.
#[test]
fn personal_library_warns_for_wrong_resource_version_kind_unknown_field_and_stream_overflow() {
    let root = temporary_directory("library-strict-resource");
    let library = PersonalLibrary::initialize(PersonalLibraryPaths {
        root: root.join("data/Toniator"),
        config_file: root.join("config/library.json"),
    })
    .expect("isolated library initializes");
    let motifs = library.root().join("motifs");
    fs::write(
        motifs.join("user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.motif.json"),
        r#"{"resource_format_version":2,"id":"user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","name":"Old","kind":"motif","segments":[]}"#,
    )
    .expect("obsolete resource fixture writes");
    fs::write(
        motifs.join("user-bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb.motif.json"),
        r#"{"resource_format_version":1,"id":"user-bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","name":"Wrong kind","kind":"shape","segments":[]}"#,
    )
    .expect("wrong-kind resource fixture writes");
    fs::write(
        motifs.join("user-cccccccc-cccc-4ccc-8ccc-cccccccccccc.motif.json"),
        r#"{"resource_format_version":1,"id":"user-cccccccc-cccc-4ccc-8ccc-cccccccccccc","name":"Unknown","kind":"motif","segments":[],"unknown":true}"#,
    )
    .expect("unknown-field resource fixture writes");
    let oversized = motifs.join("user-dddddddd-dddd-4ddd-8ddd-dddddddddddd.motif.json");
    File::create(&oversized)
        .expect("oversized sparse fixture creates")
        .set_len(MAX_PERSONAL_LIBRARY_JSON_BYTES + 1)
        .expect("oversized sparse fixture grows");
    let snapshot = library.scan().expect("malformed siblings remain nonfatal");
    assert!(snapshot.motifs.is_empty());
    assert_eq!(snapshot.warnings.len(), 4);
    fs::remove_dir_all(root).expect("isolated temporary directory removes");
}

/// Validates a candidate root before switch configuration or active-root state can change.
#[cfg(unix)]
#[test]
fn personal_library_rejects_invalid_switch_candidate_without_changing_active_state() {
    use std::os::unix::fs::symlink;

    let root = temporary_directory("library-switch-rollback");
    let config = root.join("config/library.json");
    let mut library = PersonalLibrary::initialize(PersonalLibraryPaths {
        root: root.join("data/Toniator"),
        config_file: config.clone(),
    })
    .expect("isolated library initializes");
    let valid = root.join("valid");
    fs::create_dir(&valid).expect("valid root creates");
    library.switch_root(&valid).expect("valid root switches");
    let expected_root = library.root().to_path_buf();
    let expected_config = fs::read(&config).expect("switch config reads");
    let hostile = root.join("hostile");
    fs::create_dir(&hostile).expect("hostile root creates");
    let outside = root.join("outside");
    fs::create_dir(&outside).expect("outside directory creates");
    symlink(&outside, hostile.join("motifs")).expect("hostile layout symlink creates");
    assert!(library.switch_root(&hostile).is_err());
    assert_eq!(library.root(), expected_root);
    assert_eq!(
        fs::read(&config).expect("switch config rereads"),
        expected_config
    );
    fs::remove_dir_all(root).expect("isolated temporary directory removes");
}

/// Moves only the entry-kind-qualified thumbnail and treats missing or hostile thumbnails safely.
#[cfg(unix)]
#[test]
fn personal_library_qualifies_thumbnails_and_preflights_hostile_thumbnail_moves() {
    use std::os::unix::fs::symlink;

    let root = temporary_directory("library-qualified-thumbnails");
    let library = PersonalLibrary::initialize(PersonalLibraryPaths {
        root: root.join("data/Toniator"),
        config_file: root.join("config/library.json"),
    })
    .expect("isolated library initializes");
    let shared_id = "user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    library
        .write_preset(&preset(shared_id, "Preset"), None)
        .expect("preset writes");
    library
        .write_resource(&motif(shared_id, "Motif"), None)
        .expect("motif writes");
    let thumbnails = library.root().join("thumbnails");
    let preset_thumbnail = thumbnails.join(format!("{shared_id}.preset.svg"));
    let motif_thumbnail = thumbnails.join(format!("{shared_id}.motif.svg"));
    fs::write(&preset_thumbnail, "preset thumbnail").expect("preset thumbnail writes");
    fs::write(&motif_thumbnail, "motif thumbnail").expect("motif thumbnail writes");
    let token = library
        .trash(PersonalEntryKind::Preset, shared_id)
        .expect("preset and its qualified thumbnail trash");
    assert!(!preset_thumbnail.exists());
    assert!(motif_thumbnail.exists(), "motif thumbnail remains distinct");
    library
        .undo_trash(token)
        .expect("paired preset move undoes");
    assert!(preset_thumbnail.exists());

    let hostile_id = "user-bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    library
        .write_resource(&motif(hostile_id, "Hostile thumbnail"), None)
        .expect("second motif writes");
    let outside = root.join("outside-thumbnail");
    fs::write(&outside, "outside").expect("outside file writes");
    symlink(&outside, thumbnails.join(format!("{hostile_id}.motif.svg")))
        .expect("hostile thumbnail symlink creates");
    assert!(library.trash(PersonalEntryKind::Motif, hostile_id).is_err());
    assert!(
        library
            .root()
            .join("motifs")
            .join(format!("{hostile_id}.motif.json"))
            .exists(),
        "primary entry remains after hostile thumbnail preflight failure"
    );

    let missing_id = "user-cccccccc-cccc-4ccc-8ccc-cccccccccccc";
    library
        .write_resource(&motif(missing_id, "Missing thumbnail"), None)
        .expect("third motif writes");
    let missing_token = library
        .trash(PersonalEntryKind::Motif, missing_id)
        .expect("missing thumbnail does not prevent primary trash");
    library
        .undo_trash(missing_token)
        .expect("missing thumbnail primary undo succeeds");
    fs::remove_dir_all(root).expect("isolated temporary directory removes");
}
