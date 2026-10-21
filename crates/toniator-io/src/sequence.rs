//! Exclusive numbered-frame publication and truthful incomplete/completed manifests.

use crate::filesystem::{self, Directory, Entry, PublishMode};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::fs;
use std::{
    ffi::OsStr,
    io::Write,
    path::{Path, PathBuf},
};
use toniator_domain::ProjectTiming;

/// Selects the canonical serialized consumer for every frame in one sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SequenceFormat {
    Png,
    Svg,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use toniator_domain::{FrameRange, FrameRate};

    /// Creates unique owned scratch under an explicit current evidence root or the native temporary root.
    ///
    /// # Panics
    /// Panics if test time or temporary storage is unavailable.
    fn root() -> PathBuf {
        let path = std::env::var_os("TONIATOR_B4_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!(
                "toniator-sequence-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    /// Builds a fresh two-frame manifest with nonzero absolute input indices.
    fn manifest() -> SequenceManifest {
        SequenceManifest::new(
            SequenceFormat::Png,
            8,
            8,
            &ProjectTiming::new(
                FrameRate::new(30_000, 1_001).unwrap(),
                FrameRange::new(5, 7).unwrap(),
            ),
        )
    }

    /// Verifies numbered output, incomplete retention, and refusal to replace existing artifacts.
    ///
    /// # Panics
    /// Panics if exclusive output ownership or truthful manifest completion fails.
    #[test]
    fn sequence_publication_preserves_incomplete_and_existing_files() {
        let root = root();
        let path = root.join("frames");
        let mut writer = SequenceWriter::create(&path, manifest(), 4096).unwrap();
        assert!(SequenceWriter::create(&path, manifest(), 4096).is_err());
        writer.write_frame(b"first encoded frame").unwrap();
        let incomplete: SequenceManifest =
            serde_json::from_slice(&fs::read(path.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(incomplete.completed_frames, 1);
        assert!(!incomplete.complete);
        fs::write(path.join("frame-000001.png"), b"existing user file").unwrap();
        assert!(writer.write_frame(b"second encoded frame").is_err());
        assert_eq!(
            fs::read(path.join("frame-000001.png")).unwrap(),
            b"existing user file"
        );
        drop(writer);
        let saved: SequenceManifest =
            serde_json::from_slice(&fs::read(path.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(saved, incomplete);
        assert!(path.join("frame-000000.png").is_file());
        let denied = root.join("no-space");
        assert!(SequenceWriter::create(&denied, manifest(), u64::MAX).is_err());
        assert!(!denied.exists());
        fs::remove_dir_all(root).unwrap();
    }

    /// Uses the retained directory handle even if its original path is replaced during rendering.
    ///
    /// # Panics
    /// Panics if a renamed output directory redirects any write into a replacement directory.
    #[test]
    fn sequence_writes_remain_bound_to_owned_directory() {
        let root = root();
        let path = root.join("frames");
        let retained = root.join("renamed-frames");
        let mut writer = SequenceWriter::create(&path, manifest(), 4096).unwrap();
        fs::rename(&path, &retained).unwrap();
        fs::create_dir(&path).unwrap();
        writer.write_frame(b"first").unwrap();
        writer.write_frame(b"last").unwrap();
        assert!(writer.finish().is_err());
        assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
        let saved: SequenceManifest =
            serde_json::from_slice(&fs::read(retained.join("manifest.json")).unwrap()).unwrap();
        assert!(!saved.complete);
        assert_eq!(saved.completed_frames, 2);
        assert_eq!((saved.start_frame, saved.end_frame_exclusive), (5, 7));
        assert_eq!(
            (saved.fps_numerator, saved.fps_denominator),
            (30_000, 1_001)
        );
        fs::remove_dir_all(root).unwrap();
    }

    /// Publishes frame and truthful progress before reporting a failed final metadata barrier.
    /// # Panics
    /// Panics if directory sync failure leaves manifest progress behind already renamed frame bytes.
    #[test]
    fn sequence_progress_precedes_directory_sync_failure() {
        let root = root();
        let path = root.join("frames");
        let mut writer = SequenceWriter::create(&path, manifest(), 4096).unwrap();
        let error = writer
            .write_frame_with_sync(b"first", |_| {
                Err(std::io::Error::other("injected sync failure"))
            })
            .unwrap_err();
        assert!(!error.completion_published());
        assert_eq!(error.path(), path);
        assert_eq!(fs::read(path.join("frame-000000.png")).unwrap(), b"first");
        let progress: SequenceManifest =
            serde_json::from_slice(&fs::read(path.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(progress.completed_frames, 1);
        assert!(!progress.complete);
        writer.write_frame(b"second").unwrap();
        writer.finish().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    /// Preserves actual complete bytes and directory truth after the final rename's failed sync.
    /// # Panics
    /// Panics if late durability failure hides published completion or damages frame/manifest bytes.
    #[test]
    fn sequence_completion_survives_directory_sync_failure() {
        let root = root();
        let path = root.join("frames");
        let mut writer = SequenceWriter::create(&path, manifest(), 4096).unwrap();
        writer.write_frame(b"first").unwrap();
        writer.write_frame(b"last").unwrap();
        let mut expected = writer.manifest.clone();
        expected.complete = true;
        let mut expected_bytes = serde_json::to_vec_pretty(&expected).unwrap();
        expected_bytes.push(b'\n');
        let error = writer
            .finish_with_sync(|_| Err(std::io::Error::other("injected final sync")))
            .unwrap_err();
        assert!(error.completion_published());
        assert_eq!(error.path(), path);
        assert_eq!(
            fs::read(path.join("manifest.json")).unwrap(),
            expected_bytes
        );
        assert_eq!(fs::read(path.join("frame-000000.png")).unwrap(), b"first");
        assert_eq!(fs::read(path.join("frame-000001.png")).unwrap(), b"last");
        assert!(!path.join(".manifest.partial").exists());
        fs::remove_dir_all(root).unwrap();
    }

    /// Keeps prepublication collision false and leaves existing metadata/unknown bytes untouched.
    /// # Panics
    /// Panics if completion is claimed before rename or the occupied temporary name is replaced.
    #[test]
    fn sequence_completion_collision_is_not_published() {
        let root = root();
        let path = root.join("frames");
        let mut writer = SequenceWriter::create(&path, manifest(), 4096).unwrap();
        writer.write_frame(b"first").unwrap();
        writer.write_frame(b"last").unwrap();
        let previous = fs::read(path.join("manifest.json")).unwrap();
        fs::write(path.join(".manifest.partial"), b"keep occupied name").unwrap();
        let sync_called = std::cell::Cell::new(false);
        let error = writer
            .finish_with_sync(|_| {
                sync_called.set(true);
                Ok(())
            })
            .unwrap_err();
        assert!(!error.completion_published());
        assert!(!sync_called.get());
        assert_eq!(fs::read(path.join("manifest.json")).unwrap(), previous);
        assert_eq!(
            fs::read(path.join(".manifest.partial")).unwrap(),
            b"keep occupied name"
        );
        fs::remove_dir_all(root).unwrap();
    }

}

impl SequenceFormat {
    /// Returns the fixed frame suffix; user labels never become per-frame paths.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
        }
    }
}

/// Records exact timing and completion independently of the immutable input project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceManifest {
    pub version: u32,
    pub format: SequenceFormat,
    pub width: u32,
    pub height: u32,
    pub fps_numerator: u32,
    pub fps_denominator: u32,
    pub start_frame: u64,
    pub end_frame_exclusive: u64,
    pub source_time_range: Option<SequenceTimeRange>,
    pub completed_frames: u64,
    pub complete: bool,
}

/// Stores an exact half-open source interval in seconds without rounding JSON floats.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceTimeRange {
    pub start_numerator: u64,
    pub start_denominator: u64,
    pub end_numerator: u64,
    pub end_denominator: u64,
}

impl SequenceManifest {
    /// Captures a fresh incomplete sequence from domain-owned timing and validated output size.
    pub fn new(format: SequenceFormat, width: u32, height: u32, timing: &ProjectTiming) -> Self {
        Self {
            version: 1,
            format,
            width,
            height,
            fps_numerator: timing.frame_rate().numerator(),
            fps_denominator: timing.frame_rate().denominator(),
            start_frame: timing.frame_range().start(),
            end_frame_exclusive: timing.frame_range().end_exclusive(),
            source_time_range: timing.source_time_range().map(|range| SequenceTimeRange {
                start_numerator: range.start().numerator(),
                start_denominator: range.start().denominator(),
                end_numerator: range.end().numerator(),
                end_denominator: range.end().denominator(),
            }),
            completed_frames: 0,
            complete: false,
        }
    }
}

/// Reports sequence storage errors with explicit truth about an already published complete manifest.
#[derive(Debug)]
pub struct SequenceError {
    path: PathBuf,
    context: String,
    completion_published: bool,
}

impl SequenceError {
    /// Associates one concrete output path with its diagnostic, initially without completion publication.
    fn new(path: &Path, context: impl ToString) -> Self {
        Self {
            path: path.into(),
            context: context.to_string(),
            completion_published: false,
        }
    }
    /// Reports whether the complete manifest rename succeeded before a later directory-sync error.
    /// Incomplete progress and every failure before completion publication remain false.
    pub const fn completion_published(&self) -> bool {
        self.completion_published
    }
    /// Returns the concrete failed output path; a published completion identifies its directory.
    pub fn path(&self) -> &Path {
        &self.path
    }
}
impl std::fmt::Display for SequenceError {
    /// Formats output ownership and storage failures without hiding the relevant path.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.context)
    }
}
impl std::error::Error for SequenceError {}

/// Queries bytes available to the current user without a shell or unsafe application code.
///
/// # Errors
/// Returns the filesystem diagnostic or checked-size overflow for the requested existing path.
pub fn available_space(path: &Path) -> Result<u64, SequenceError> {
    filesystem::available_space(path).map_err(|error| SequenceError::new(path, error))
}

/// Owns an exclusively created directory and writes only fixed names relative to its open handle.
///
/// Cancellation/drop retains successfully completed frames with `complete: false`. Publication
/// never traverses a replaced directory path and never removes a user's preexisting directory.
pub struct SequenceWriter {
    path: PathBuf,
    directory: Directory,
    manifest: SequenceManifest,
}

impl SequenceWriter {
    /// Creates a private child directory after validating ownership, metadata and storage.
    ///
    /// # Errors
    /// Rejects existing destinations, insufficient space, invalid fresh manifests or I/O failures.
    pub fn create(
        path: &Path,
        manifest: SequenceManifest,
        estimated_bytes: u64,
    ) -> Result<Self, SequenceError> {
        if manifest.version != 1
            || manifest.width == 0
            || manifest.height == 0
            || manifest.fps_numerator == 0
            || manifest.fps_denominator == 0
            || manifest.end_frame_exclusive <= manifest.start_frame
            || manifest.completed_frames != 0
            || manifest.complete
        {
            return Err(SequenceError::new(path, "invalid fresh sequence manifest"));
        }
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent_directory =
            Directory::open(parent).map_err(|error| SequenceError::new(path, error))?;
        if parent_directory
            .available_bytes()
            .map_err(|error| SequenceError::new(parent, error))?
            < estimated_bytes
        {
            return Err(SequenceError::new(
                path,
                "insufficient space for the estimated full sequence",
            ));
        }
        let name = path
            .file_name()
            .ok_or_else(|| SequenceError::new(path, "destination must name a directory"))?;
        let directory = parent_directory
            .create_directory(name)
            .map_err(|error| SequenceError::new(path, error))?;
        let writer = Self {
            path: path.into(),
            directory,
            manifest,
        };
        writer.write_manifest(true)?;
        Ok(writer)
    }

    /// Returns the requested sequence destination without transferring directory ownership.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes the next numbered frame, synchronizes it, and atomically updates incomplete progress.
    ///
    /// # Errors
    /// Rejects excessive/empty frames, space exhaustion, name collisions, and write/sync failures.
    /// An interrupted frame remains a `.partial` file, never a completed numbered frame.
    pub fn write_frame(&mut self, bytes: &[u8]) -> Result<(), SequenceError> {
        self.write_frame_with_sync(bytes, Directory::sync)
    }

    /// Preserves file-sync, frame-rename, progress, manifest-rename, then directory-sync ordering.
    /// # Errors
    /// Returns prepublication or final sync failures while retaining truthful renamed-frame progress.
    fn write_frame_with_sync(
        &mut self,
        bytes: &[u8],
        sync: impl FnOnce(&Directory) -> std::io::Result<()>,
    ) -> Result<(), SequenceError> {
        if bytes.is_empty()
            || self.manifest.complete
            || self.manifest.completed_frames
                >= self.manifest.end_frame_exclusive - self.manifest.start_frame
        {
            return Err(SequenceError::new(
                &self.path,
                "invalid frame publication order or empty output",
            ));
        }
        self.require_space((bytes.len() as u64).saturating_add(64 * 1024))?;
        let name = format!(
            "frame-{:06}.{}",
            self.manifest.completed_frames,
            self.manifest.format.extension()
        );
        let temporary = format!(".{name}.partial");
        let mut staging = self.create_file(&temporary, bytes)?;
        staging
            .rename(&self.directory, OsStr::new(&name), PublishMode::NoReplace)
            .map_err(|error| SequenceError::new(&self.path.join(&name), error))?;
        self.manifest.completed_frames += 1;
        self.write_manifest_with_sync(false, sync)
    }

    /// Publishes a complete manifest only after all declared numbered frames are synchronized.
    /// # Errors
    /// Rejects incomplete or displaced sequences and storage failures. A late directory-sync error
    /// has completion_published=true: the complete manifest is visible and is never rolled back.
    pub fn finish(self) -> Result<PathBuf, SequenceError> {
        self.finish_with_sync(Directory::sync)
    }

    /// Retains the actual completion boundary while selecting only its final directory-sync barrier.
    /// # Errors
    /// Returns prepublication failures or a typed post-rename durability error without rollback.
    fn finish_with_sync(
        mut self,
        sync: impl FnOnce(&Directory) -> std::io::Result<()>,
    ) -> Result<PathBuf, SequenceError> {
        self.directory
            .ensure_path(&self.path)
            .map_err(|error| SequenceError::new(&self.path, error))?;
        if self.manifest.completed_frames
            != self.manifest.end_frame_exclusive - self.manifest.start_frame
        {
            return Err(SequenceError::new(
                &self.path,
                "cannot complete a partially rendered sequence",
            ));
        }
        self.manifest.complete = true;
        self.write_manifest_with_sync(false, sync)?;
        Ok(self.path)
    }

    /// Checks current free space on the owned directory's filesystem during a running job.
    ///
    /// # Errors
    /// Returns storage-query, overflow or insufficient-space diagnostics before writing bytes.
    fn require_space(&self, required: u64) -> Result<(), SequenceError> {
        let available = self
            .directory
            .available_bytes()
            .map_err(|error| SequenceError::new(&self.path, error))?;
        if available < required {
            return Err(SequenceError::new(
                &self.path,
                "insufficient space while writing sequence",
            ));
        }
        Ok(())
    }

    /// Creates a fixed relative file exclusively and synchronizes its exact bytes.
    ///
    /// # Errors
    /// Returns create/write/sync failures without replacing any existing file or following links.
    fn create_file(&self, name: &str, bytes: &[u8]) -> Result<Entry, SequenceError> {
        let entry = self
            .directory
            .create_file(OsStr::new(name))
            .map_err(|error| SequenceError::new(&self.path.join(name), error))?;
        let mut file = entry.file();
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| SequenceError::new(&self.path.join(name), error))?;
        Ok(entry)
    }

    /// Atomically replaces only the job-owned manifest after all declared frames are synchronized.
    ///
    /// # Errors
    /// Returns JSON, exclusive temporary file, rename or directory synchronization failures.
    fn write_manifest(&self, initial: bool) -> Result<(), SequenceError> {
        self.write_manifest_with_sync(initial, Directory::sync)
    }

    /// Replaces synchronized manifest bytes before the caller-selected final metadata barrier.
    /// # Errors
    /// Returns serialization/create/file-sync/rename failures with completion_published=false.
    /// Only a directory-sync failure after a complete-manifest rename sets that fact true; manifest
    /// bytes/schema and incomplete progress publication ordering remain unchanged.
    fn write_manifest_with_sync(
        &self,
        initial: bool,
        sync: impl FnOnce(&Directory) -> std::io::Result<()>,
    ) -> Result<(), SequenceError> {
        let mut bytes = serde_json::to_vec_pretty(&self.manifest)
            .map_err(|error| SequenceError::new(&self.path, error))?;
        bytes.push(b'\n');
        let mut staging = self.create_file(".manifest.partial", &bytes)?;
        staging
            .rename(
                &self.directory,
                OsStr::new("manifest.json"),
                if initial {
                    PublishMode::NoReplace
                } else {
                    PublishMode::Replace
                },
            )
            .map_err(|error| SequenceError::new(&self.path, error))?;
        sync(&self.directory).map_err(|error| {
            let mut error = SequenceError::new(&self.path, error);
            error.completion_published = self.manifest.complete;
            error
        })
    }
}
