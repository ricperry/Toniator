//! Adjacent still-output staging; callbacks own encoding and callers authorize replacement.

use crate::filesystem::{Directory, Entry, PublishMode, random_suffix};
use std::{
    ffi::OsString,
    fmt,
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Failure at a precise staging or publication boundary; postpublication sync errors stay explicit.
#[derive(Debug)]
pub struct AtomicOutputError {
    path: PathBuf,
    phase: &'static str,
    message: String,
    published: bool,
}

impl AtomicOutputError {
    /// Captures the destination and real IO diagnostic without changing filesystem state.
    fn new(path: &Path, phase: &'static str, error: impl fmt::Display, published: bool) -> Self {
        Self {
            path: path.into(),
            phase,
            message: error.to_string(),
            published,
        }
    }

    /// Returns the failed operation's stable diagnostic phase.
    pub fn phase(&self) -> &'static str {
        self.phase
    }

    /// Reports whether publication already occurred before a later directory-sync failure.
    pub fn published(&self) -> bool {
        self.published
    }
}

impl fmt::Display for AtomicOutputError {
    /// Formats the precise output boundary without discarding the underlying filesystem error.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}: {}",
            self.phase,
            self.path.display(),
            self.message
        )
    }
}

impl std::error::Error for AtomicOutputError {}

/// Encodes a still output into exclusive adjacent staging and atomically replaces its destination.
///
/// The callback writes only the supplied retained handle. Success requires callback completion,
/// flush, file synchronization, a final cancellation check, then atomic rename and directory sync.
/// Any prepublication failure preserves the prior destination and discards only verified owned
/// staging. A directory-sync failure after rename reports `published()` truthfully; no rollback
/// claims are made. Replacement is caller-authorized policy, never inferred from file contents.
///
/// # Errors
/// Returns invalid destination, staging/write/sync/cancellation/publication errors. Cleanup failure
/// leaves an owned temporary file and is appended to the original diagnostic.
pub fn write_atomic_output<T>(
    path: &Path,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    encode: impl FnOnce(&mut File) -> io::Result<T>,
) -> Result<T, AtomicOutputError> {
    write_atomic_output_with_sync(path, is_cancelled, encode, File::sync_all, Directory::sync)
}

/// Preserves prepublication file synchronization and truthful postpublication directory failures.
///
/// Injected synchronizers exercise actual retained-file publication ordering in focused tests.
/// # Errors
/// Returns the same encoding, synchronization, cancellation, and atomic-publication diagnostics.
fn write_atomic_output_with_sync<T>(
    path: &Path,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    encode: impl FnOnce(&mut File) -> io::Result<T>,
    sync_file: impl FnOnce(&File) -> io::Result<()>,
    sync_directory: impl FnOnce(&Directory) -> io::Result<()>,
) -> Result<T, AtomicOutputError> {
    check_cancelled(path, is_cancelled)?;
    let target = path.file_name().ok_or_else(|| {
        AtomicOutputError::new(
            path,
            "output.destination",
            "destination must name a file",
            false,
        )
    })?;
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = Directory::open(parent_path)
        .map_err(|error| AtomicOutputError::new(path, "output.staging", error, false))?;
    let temporary = OsString::from(format!(
        ".toniator-output-{}.tmp",
        random_suffix().map_err(|error| AtomicOutputError::new(
            path,
            "output.staging",
            error,
            false
        ))?
    ));
    let mut entry = parent
        .create_file(&temporary)
        .map_err(|error| AtomicOutputError::new(path, "output.staging", error, false))?;
    let result = (|| {
        let mut file = entry
            .try_clone_file()
            .map_err(|error| AtomicOutputError::new(path, "output.staging", error, false))?;
        let value = encode(&mut file)
            .map_err(|error| AtomicOutputError::new(path, "output.encode", error, false))?;
        check_cancelled(path, is_cancelled)?;
        file.flush()
            .map_err(|error| AtomicOutputError::new(path, "output.flush", error, false))?;
        sync_file(&file)
            .map_err(|error| AtomicOutputError::new(path, "output.sync", error, false))?;
        drop(file);
        parent
            .ensure_path(parent_path)
            .map_err(|error| AtomicOutputError::new(path, "output.publish", error, false))?;
        check_cancelled(path, is_cancelled)?;
        entry
            .rename(&parent, target, PublishMode::Replace)
            .map_err(|error| AtomicOutputError::new(path, "output.publish", error, false))?;
        sync_directory(&parent)
            .map_err(|error| AtomicOutputError::new(path, "output.directory_sync", error, true))?;
        Ok(value)
    })();
    finish_staging(entry, result)
}

/// Discards only verified unpublished staging, retaining the first error and any cleanup failure.
///
/// # Errors
/// Returns the original publication result; cleanup failures append context without hiding it.
fn finish_staging<T>(
    entry: Entry,
    result: Result<T, AtomicOutputError>,
) -> Result<T, AtomicOutputError> {
    if !entry.is_published()
        && let Err(mut error) = result
    {
        if let Err(cleanup) = entry.discard() {
            error
                .message
                .push_str(&format!("; owned staging cleanup failed: {cleanup}"));
        }
        return Err(error);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicBool, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    /// Allocates a fresh stage-owned directory and sentinel destination for publication checks.
    /// # Panics
    /// Panics on clock or stage-owned filesystem failures.
    fn destination(label: &str) -> PathBuf {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/validation/ton018-png-streaming-20261013/atomic")
            .join(format!(
                "{label}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("approved.png");
        fs::write(&path, b"approved prior output").unwrap();
        path
    }

    /// Verifies the prior target and unrelated neighbor survive with no leftover owned staging.
    /// # Panics
    /// Panics if destination bytes or verified cleanup differs from the expected transaction.
    fn assert_output(path: &Path, expected: &[u8]) {
        assert_eq!(fs::read(path).unwrap(), expected);
        assert!(!fs::read_dir(path.parent().unwrap()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".toniator-output-")
        }));
    }

    /// Proves success publishes once while callback failure preserves prior and neighboring files.
    /// # Panics
    /// Panics on publication, cleanup, error-phase, or unrelated-file preservation regressions.
    #[test]
    fn atomic_output_preserves_prior_file_on_encode_failure_and_replaces_on_success() {
        let path = destination("encode");
        let neighbor = path.with_file_name("unrelated.png");
        fs::write(&neighbor, b"unrelated").unwrap();
        let error = write_atomic_output(&path, &|| false, |file| {
            file.write_all(b"partial new output")?;
            Err::<(), _>(io::Error::other("injected write failure"))
        })
        .unwrap_err();
        assert_eq!(error.phase(), "output.encode");
        assert!(!error.published());
        assert_output(&path, b"approved prior output");
        let value = write_atomic_output(&path, &|| false, |file| {
            file.write_all(b"complete new output")?;
            Ok(17)
        })
        .unwrap();
        assert_eq!(value, 17);
        assert_output(&path, b"complete new output");
        assert_eq!(fs::read(neighbor).unwrap(), b"unrelated");
    }

    /// Proves initial, callback-time, and final prepublication cancellation preserve prior bytes.
    /// # Panics
    /// Panics if cancellation publishes or leaves owned staging behind.
    #[test]
    fn atomic_output_cancellation_preserves_prior_file_before_publication() {
        let path = destination("cancel");
        let error = write_atomic_output(&path, &|| true, |_| Ok(())).unwrap_err();
        assert_eq!(error.phase(), "output.cancelled");
        assert_output(&path, b"approved prior output");
        let cancelled = AtomicBool::new(false);
        let error = write_atomic_output(&path, &|| cancelled.load(Ordering::Acquire), |file| {
            file.write_all(b"complete but cancelled output")?;
            cancelled.store(true, Ordering::Release);
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.phase(), "output.cancelled");
        assert_output(&path, b"approved prior output");
        cancelled.store(false, Ordering::Release);
        let error = write_atomic_output_with_sync(
            &path,
            &|| cancelled.load(Ordering::Acquire),
            |file| file.write_all(b"fully synced but cancelled output"),
            |file| {
                file.sync_all()?;
                cancelled.store(true, Ordering::Release);
                Ok(())
            },
            Directory::sync,
        )
        .unwrap_err();
        assert_eq!(error.phase(), "output.cancelled");
        assert_output(&path, b"approved prior output");
    }

    /// Proves file-sync failure remains prepublication and directory-sync failure reports publication.
    /// # Panics
    /// Panics on hidden postpublication state or a damaged prior file before rename.
    #[test]
    fn atomic_output_sync_failures_report_truthful_publication_state() {
        let path = destination("sync");
        let error = write_atomic_output_with_sync(
            &path,
            &|| false,
            |file| file.write_all(b"new bytes"),
            |_| Err(io::Error::other("injected file sync failure")),
            Directory::sync,
        )
        .unwrap_err();
        assert_eq!(error.phase(), "output.sync");
        assert!(!error.published());
        assert_output(&path, b"approved prior output");
        let error = write_atomic_output_with_sync(
            &path,
            &|| false,
            |file| file.write_all(b"new bytes"),
            File::sync_all,
            |_| Err(io::Error::other("injected directory sync failure")),
        )
        .unwrap_err();
        assert_eq!(error.phase(), "output.directory_sync");
        assert!(error.published());
        assert_output(&path, b"new bytes");
    }
}

/// Rejects cancellation before another staging or publication boundary.
///
/// # Errors
/// Returns a stable prepublication cancellation diagnostic without touching the destination.
fn check_cancelled(
    path: &Path,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), AtomicOutputError> {
    if is_cancelled() {
        Err(AtomicOutputError::new(
            path,
            "output.cancelled",
            "export was cancelled",
            false,
        ))
    } else {
        Ok(())
    }
}
