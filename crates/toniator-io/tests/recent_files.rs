//! Current startup history tests; all artifacts are private temporary metadata and dummy files.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use toniator_io::recent::*;

/// Creates an isolated fixture folder; no existing library or document is touched.
///
/// # Panics
/// Panics if the host clock or temporary filesystem cannot provide a unique fixture folder.
fn fixture(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "toniator-recent-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    path
}

/// Verifies successful opens/saves deduplicate canonical paths, order by use, cap history, and clear
/// only metadata; equal filenames from distinct folders remain independently reachable.
///
/// # Panics
/// Panics when persistence loses ordering/identity or clearing touches any source file.
#[test]
fn recent_files_round_trip_bound_deduplicate_and_clear_without_deleting_targets() {
    let root = fixture("round-trip");
    let history = root.join("state/recent-files.json");
    assert!(load_recent_files(&history).unwrap().is_empty());
    for index in 0..14 {
        let folder = root.join(index.to_string());
        fs::create_dir(&folder).unwrap();
        let path = folder.join("artwork.png");
        fs::write(&path, b"original").unwrap();
        remember_recent_file(&history, &path, index).unwrap();
    }
    let entries = load_recent_files(&history).unwrap();
    assert_eq!(entries.len(), MAX_RECENT_FILES);
    assert_eq!(entries[0].used_at, 13);
    let first = root.join("2/artwork.png");
    let entries = remember_recent_file(&history, &first, 99).unwrap();
    assert_eq!(entries[0].path, fs::canonicalize(&first).unwrap());
    assert_eq!(entries[0].used_at, 99);
    assert_eq!(entries.len(), MAX_RECENT_FILES);
    let before = fs::read(&history).unwrap();
    assert!(remember_recent_file(&history, &root.join("missing.png"), 100).is_err());
    assert_eq!(fs::read(&history).unwrap(), before);
    clear_recent_files(&history).unwrap();
    assert!(load_recent_files(&history).unwrap().is_empty());
    for index in 0..14 {
        assert_eq!(
            fs::read(root.join(index.to_string()).join("artwork.png")).unwrap(),
            b"original"
        );
    }
    fs::remove_dir_all(root).unwrap();
}

/// Rejects malformed/obsolete/oversized history while allowing an explicit Clear List recovery.
///
/// # Panics
/// Panics when a rejected history is silently replaced by Remember or accepted as current data.
#[test]
fn recent_files_reject_bad_metadata_and_allow_explicit_clear() {
    let root = fixture("invalid");
    let history = root.join("recent-files.json");
    let target = root.join("art.png");
    fs::write(&target, b"art").unwrap();
    for bytes in [
        b"invalid".to_vec(),
        br#"{"version":0,"entries":[]}"#.to_vec(),
        vec![b' '; 65537],
    ] {
        fs::write(&history, &bytes).unwrap();
        assert!(load_recent_files(&history).is_err());
        assert!(remember_recent_file(&history, &target, 1).is_err());
        assert_eq!(fs::read(&history).unwrap(), bytes);
    }
    clear_recent_files(&history).unwrap();
    assert!(load_recent_files(&history).unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
}

/// Keeps recent metadata in XDG state storage and ignores invalid relative environment values.
///
/// # Panics
/// Panics if path selection escapes the supplied absolute XDG/home authority.
#[test]
fn recent_files_use_xdg_state_with_home_fallback() {
    let home = std::path::Path::new("/home/artist");
    assert_eq!(
        recent_file_path(Some(std::path::Path::new("/state")), Some(home)),
        Some(PathBuf::from("/state/Toniator/recent-files.json"))
    );
    assert_eq!(
        recent_file_path(Some(std::path::Path::new("relative")), Some(home)),
        Some(home.join(".local/state/Toniator/recent-files.json"))
    );
    assert_eq!(recent_file_path(None, None), None);
}
