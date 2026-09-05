//! Bounded recent-file metadata for the startup screen, independent of document contents.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};

/// Limits the startup list without retaining an unbounded file history.
pub const MAX_RECENT_FILES: usize = 12;
const MAX_METADATA_BYTES: u64 = 64 * 1024;
static WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Records a successfully opened source/project or saved project, never embedded artwork.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecentFile {
    /// Canonical local path, used to distinguish equal filenames in different folders.
    pub path: PathBuf,
    /// Last successful use in Unix seconds; display formatting belongs to the frontend.
    pub used_at: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecentEnvelope {
    version: u32,
    entries: Vec<RecentFile>,
}

/// Resolves application history under XDG_STATE_HOME, falling back to HOME/.local/state.
/// Returns no path when neither supplied base is absolute; this never creates directories.
pub fn recent_file_path(state_home: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    state_home
        .filter(|path| path.is_absolute())
        .map(Path::to_path_buf)
        .or_else(|| {
            home.filter(|path| path.is_absolute())
                .map(|path| path.join(".local/state"))
        })
        .map(|path| path.join("Toniator/recent-files.json"))
}

/// Loads current recent metadata, preserving missing targets so the UI can explain failed opens.
/// A missing history file is an empty list. Reads are bounded and do not open listed artwork.
///
/// # Errors
/// Returns I/O or invalid-data errors for unreadable, oversized, obsolete, or malformed metadata.
pub fn load_recent_files(path: &Path) -> io::Result<Vec<RecentFile>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(MAX_METADATA_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Recent Files metadata is too large.",
        ));
    }
    let envelope: RecentEnvelope = serde_json::from_slice(&bytes)?;
    if envelope.version != 1
        || envelope.entries.len() > MAX_RECENT_FILES
        || envelope
            .entries
            .iter()
            .any(|entry| !entry.path.is_absolute())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported Recent Files metadata.",
        ));
    }
    let mut entries = Vec::new();
    for entry in envelope.entries {
        if !entries
            .iter()
            .any(|existing: &RecentFile| existing.path == entry.path)
        {
            entries.push(entry);
        }
    }
    Ok(entries)
}

/// Records a successful file operation at the front of the current on-disk list.
/// Canonical paths deduplicate aliases; older entries are capped at twelve. Source files are read
/// only for canonicalization and metadata, never rewritten by this operation.
///
/// # Errors
/// Returns target-path, history-read, serialization, or atomic-publication errors.
pub fn remember_recent_file(
    history: &Path,
    target: &Path,
    used_at: u64,
) -> io::Result<Vec<RecentFile>> {
    let target = fs::canonicalize(target)?;
    if !target.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Recent target must be a file.",
        ));
    }
    let mut entries = load_recent_files(history)?;
    entries.retain(|entry| entry.path != target);
    entries.insert(
        0,
        RecentFile {
            path: target,
            used_at,
        },
    );
    entries.truncate(MAX_RECENT_FILES);
    publish_recent_files(history, &entries)?;
    Ok(entries)
}

/// Clears only recent-file metadata; none of the listed files are removed or changed.
///
/// # Errors
/// Returns serialization or atomic-publication errors while preserving the previous history.
pub fn clear_recent_files(history: &Path) -> io::Result<()> {
    publish_recent_files(history, &[])
}

/// Atomically replaces a small metadata file using a uniquely created sibling temporary file.
/// On failure, only this call's owned temporary file is cleaned up; targets remain untouched.
///
/// # Errors
/// Returns invalid-parent, serialization, directory, write, sync, or rename errors.
fn publish_recent_files(path: &Path, entries: &[RecentFile]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "History needs a parent folder.",
            )
        })?;
    let bytes = serde_json::to_vec_pretty(&RecentEnvelope {
        version: 1,
        entries: entries.to_vec(),
    })?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Recent Files metadata is too large.",
        ));
    }
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".recent-{}-{}.tmp",
        std::process::id(),
        WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
