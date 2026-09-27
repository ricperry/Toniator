//! Owned video staging, no-replace publication, and bounded private PNG recovery storage.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Verifies staging cleanup, non-overwrite publication, and refusal to remove unknown files.
    ///
    /// # Panics
    /// Panics if any operation affects an existing destination or an unexpected workspace entry.
    #[test]
    fn video_storage_preserves_existing_files_and_bounds_discard_ownership() {
        let mut root = RenderWorkspace::create(&std::env::temp_dir()).unwrap();
        let path = root.path().join("video.mkv");
        let output = VideoOutput::reserve(&path, 4096).unwrap();
        output
            .file_handle()
            .unwrap()
            .write_all(b"encoded file")
            .unwrap();
        fs::write(&path, b"existing file").unwrap();
        assert!(output.publish().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"existing file");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        assert!(root.discard().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"existing file");
        fs::remove_file(&path).unwrap();
        let output = VideoOutput::reserve(&path, 4096).unwrap();
        output
            .file_handle()
            .unwrap()
            .write_all(b"complete video")
            .unwrap();
        assert_eq!(output.publish().unwrap(), path);
        assert_eq!(fs::read(&path).unwrap(), b"complete video");
        fs::remove_file(&path).unwrap();
        fs::create_dir(root.frames_path()).unwrap();
        fs::write(root.frames_path().join("frame-000000.png"), b"owned").unwrap();
        assert_eq!(root.read_frame(0, 5).unwrap(), b"owned");
        assert!(root.read_frame(0, 4).is_err());
        fs::write(root.frames_path().join("user-notes.txt"), b"keep").unwrap();
        assert!(root.discard().is_err());
        assert!(root.frames_path().join("frame-000000.png").exists());
        fs::remove_file(root.frames_path().join("user-notes.txt")).unwrap();
        fs::create_dir(root.frames_path().join("frame-000001.png")).unwrap();
        assert!(root.read_frame(1, 1024).is_err());
        assert!(root.discard().is_err());
        assert!(root.frames_path().join("frame-000000.png").exists());
        fs::remove_dir(root.frames_path().join("frame-000001.png")).unwrap();
        let root_path = root.path().to_owned();
        root.discard().unwrap();
        assert!(!root_path.exists());
    }

    /// Retains a successfully renamed video when the later metadata durability barrier fails.
    /// # Panics
    /// Panics if failure cleanup removes published bytes or leaves an unpublished staging sibling.
    #[test]
    fn video_publication_survives_parent_sync_failure() {
        let mut root = RenderWorkspace::create(&std::env::temp_dir()).unwrap();
        let path = root.path().join("video.mkv");
        let output = VideoOutput::reserve(&path, 4096).unwrap();
        output
            .file_handle()
            .unwrap()
            .write_all(b"published video")
            .unwrap();
        assert!(
            output
                .publish_with_sync(|_| Err(std::io::Error::other("injected sync failure")))
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), b"published video");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        fs::remove_file(path).unwrap();
        root.discard().unwrap();
        assert!(!root.path().exists());
        root.discard().unwrap();
        assert!(root.require_space(0).is_err());
    }
}

use crate::filesystem::{self, Directory, Entry, PublishMode};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{Read, Seek},
    path::{Path, PathBuf},
};

/// Reports staging ownership, storage, cleanup or publication failures without hiding the path.
#[derive(Debug)]
pub struct VideoStorageError {
    path: PathBuf,
    detail: String,
}
impl VideoStorageError {
    /// Associates a concrete owned location with its filesystem diagnostic.
    fn new(path: &Path, detail: impl ToString) -> Self {
        Self {
            path: path.into(),
            detail: detail.to_string(),
        }
    }
}
impl std::fmt::Display for VideoStorageError {
    /// Formats the concrete path and bounded filesystem diagnostic.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.detail)
    }
}
impl std::error::Error for VideoStorageError {}

/// Holds one open temporary sibling until an encoder validates and publishes the file.
///
/// The encoder writes a duplicated file handle; it never opens the destination path. Drop removes
/// only the verified unpublished staging link. Final publication atomically refuses an existing destination.
#[derive(Debug)]
pub struct VideoOutput {
    parent: Directory,
    parent_path: PathBuf,
    target: OsString,
    temporary: OsString,
    staging: Option<Entry>,
}
impl VideoOutput {
    /// Reserves a private temporary sibling after checking destination absence and space.
    ///
    /// # Errors
    /// Rejects invalid/existing destinations, unavailable space, or exclusive staging failures.
    pub fn reserve(path: &Path, estimated_bytes: u64) -> Result<Self, VideoStorageError> {
        let target = path
            .file_name()
            .ok_or_else(|| VideoStorageError::new(path, "destination must name a file"))?
            .to_owned();
        let parent_path = fs::canonicalize(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        )
        .map_err(|error| VideoStorageError::new(path, error))?;
        let parent = open_directory(&parent_path)?;
        if parent
            .entry_exists(&target)
            .map_err(|error| VideoStorageError::new(path, error))?
        {
            return Err(VideoStorageError::new(path, "destination already exists"));
        }
        require_space(&parent, &parent_path, estimated_bytes)?;
        let temporary = OsString::from(format!(".toniator-video-{}.part", random_suffix()?));
        let staging = parent
            .create_file(&temporary)
            .map_err(|error| VideoStorageError::new(path, error))?;
        Ok(Self {
            parent,
            parent_path,
            target,
            temporary,
            staging: Some(staging),
        })
    }

    /// Duplicates the owned file for an encoder or validator's standard stream.
    ///
    /// # Errors
    /// Returns descriptor duplication or seek failures without reopening a path.
    pub fn file_handle(&self) -> Result<File, VideoStorageError> {
        let mut file = self
            .entry()?
            .try_clone_file()
            .map_err(|error| VideoStorageError::new(&self.path(), error))?;
        file.rewind()
            .map_err(|error| VideoStorageError::new(&self.path(), error))?;
        Ok(file)
    }

    /// Returns the final canonical destination path for completion reporting.
    pub fn path(&self) -> PathBuf {
        self.parent_path.join(&self.target)
    }

    /// Checks actual free space while an encoder is running.
    ///
    /// # Errors
    /// Returns unavailable/insufficient-space diagnostics before further output is requested.
    pub fn require_space(&self, bytes: u64) -> Result<(), VideoStorageError> {
        require_space(&self.parent, &self.parent_path, bytes)
    }

    /// Distinguishes separate filesystems when reserving simultaneous PNG and video storage.
    ///
    /// # Errors
    /// Returns a metadata failure for either owned directory.
    pub fn shares_filesystem(
        &self,
        workspace: &RenderWorkspace,
    ) -> Result<bool, VideoStorageError> {
        Ok(self.parent.identity().volume == workspace.directory()?.identity().volume)
    }

    /// Truncates only this owned staging file after a successful capability probe.
    ///
    /// # Errors
    /// Returns truncation/seek failures; the destination remains unpublished.
    pub fn clear(&self) -> Result<(), VideoStorageError> {
        self.entry()?
            .file()
            .set_len(0)
            .map_err(|error| VideoStorageError::new(&self.path(), error))?;
        self.file_handle()?;
        Ok(())
    }

    /// Atomically publishes the validated file without replacing an existing user artifact.
    ///
    /// # Errors
    /// Rejects moved/replaced staging or parent paths, empty files, collisions, or sync failures.
    pub fn publish(self) -> Result<PathBuf, VideoStorageError> {
        self.publish_with_sync(Directory::sync)
    }

    /// Publishes through the production rename boundary while exposing its later sync barrier for tests.
    /// # Errors
    /// Returns prepublication or sync failures; rename success permanently revokes staging cleanup.
    fn publish_with_sync(
        mut self,
        sync: impl FnOnce(&Directory) -> std::io::Result<()>,
    ) -> Result<PathBuf, VideoStorageError> {
        ensure_directory_identity(&self.parent, &self.parent_path)?;
        if !self.owns_temporary()
            || self
                .entry()?
                .file()
                .metadata()
                .map_err(|error| VideoStorageError::new(&self.path(), error))?
                .len()
                == 0
        {
            return Err(VideoStorageError::new(
                &self.path(),
                "staging file changed or is empty",
            ));
        }
        self.entry()?
            .file()
            .sync_all()
            .map_err(|error| VideoStorageError::new(&self.path(), error))?;
        let path = self.path();
        self.staging
            .as_mut()
            .ok_or_else(|| VideoStorageError::new(&path, "staging handle released"))?
            .rename(&self.parent, &self.target, PublishMode::NoReplace)
            .map_err(|error| VideoStorageError::new(&path, error))?;
        sync(&self.parent).map_err(|error| VideoStorageError::new(&self.path(), error))?;
        Ok(self.path())
    }

    /// Checks the staging name still references the exclusively created regular file.
    fn owns_temporary(&self) -> bool {
        self.staging.as_ref().is_some_and(|owned| {
            self.parent
                .open_file(&self.temporary)
                .is_ok_and(|actual| actual.identity() == owned.identity())
        })
    }

    /// Returns the live staging capability without reopening its path.
    /// # Errors
    /// Rejects a released staging handle instead of substituting another file.
    fn entry(&self) -> Result<&Entry, VideoStorageError> {
        self.staging
            .as_ref()
            .ok_or_else(|| VideoStorageError::new(&self.path(), "staging handle released"))
    }
}
impl Drop for VideoOutput {
    /// Removes only the verified unpublished staging link; published outputs survive later sync failures.
    fn drop(&mut self) {
        if let Some(entry) = self.staging.take()
            && !entry.is_published()
        {
            let _ = entry.discard();
        }
    }
}

/// Owns a private temporary root retained across encoding recovery decisions.
///
/// Drop deliberately retains files. The job explicitly discards on cancellation/success, while
/// encoding failures may hand this value to recovery UI or leave the path available to CLI users.
#[derive(Debug)]
pub struct RenderWorkspace {
    path: PathBuf,
    directory: Option<Directory>,
    discarded: bool,
}
impl RenderWorkspace {
    /// Creates a random private render directory through the caller-selected temporary parent.
    ///
    /// # Errors
    /// Returns parent, random-source, directory-creation or descriptor-opening failures.
    pub fn create(parent: &Path) -> Result<Self, VideoStorageError> {
        let parent =
            fs::canonicalize(parent).map_err(|error| VideoStorageError::new(parent, error))?;
        let parent_directory = open_directory(&parent)?;
        let name = OsString::from(format!("toniator-render-{}", random_suffix()?));
        let path = parent.join(&name);
        let directory = parent_directory
            .create_directory(&name)
            .map_err(|error| VideoStorageError::new(&path, error))?;
        Ok(Self {
            path,
            directory: Some(directory),
            discarded: false,
        })
    }

    /// Returns the exclusively owned workspace location for recovery messages.
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Returns the fixed child directory created by the shared PNG sequence runner.
    pub fn frames_path(&self) -> PathBuf {
        self.path.join("frames")
    }

    /// Checks free space on the temporary filesystem before rendering.
    ///
    /// # Errors
    /// Returns an unavailable/insufficient-space diagnostic for the owned workspace.
    pub fn require_space(&self, bytes: u64) -> Result<(), VideoStorageError> {
        require_space(self.directory()?, &self.path, bytes)
    }

    /// Reads one retained numbered PNG through an owned directory handle and a caller byte bound.
    ///
    /// # Errors
    /// Rejects discarded workspaces, missing/nonregular files, path replacement or excess bytes.
    pub fn read_frame(&self, index: u64, maximum_bytes: u64) -> Result<Vec<u8>, VideoStorageError> {
        ensure_directory_identity(self.directory()?, &self.path)?;
        let frames = self.open_frames()?;
        let name = format!("frame-{index:06}.png");
        let entry = frames
            .open_file(OsStr::new(&name))
            .map_err(|error| VideoStorageError::new(&self.frames_path(), error))?;
        let file = entry.file();
        let metadata = file
            .metadata()
            .map_err(|error| VideoStorageError::new(&self.frames_path(), error))?;
        if !metadata.is_file() || metadata.len() > maximum_bytes {
            return Err(VideoStorageError::new(
                &self.frames_path(),
                "retained frame exceeds its file/size bound",
            ));
        }
        let mut bytes = Vec::new();
        file.take(maximum_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| VideoStorageError::new(&self.frames_path(), error))?;
        if bytes.len() as u64 > maximum_bytes {
            return Err(VideoStorageError::new(
                &self.frames_path(),
                "retained frame expanded beyond its bound",
            ));
        }
        Ok(bytes)
    }

    /// Discards only known job-created PNG, manifest and partial-file names without recursion.
    ///
    /// Unknown entries stop cleanup before deletion. Directory descriptors prevent a replaced
    /// visible path from redirecting removal into another directory.
    ///
    /// # Errors
    /// Rejects moved roots, unexpected files or failed unlink/rmdir operations; retained files stay
    /// available when cleanup cannot prove ownership.
    pub fn discard(&mut self) -> Result<(), VideoStorageError> {
        if self.discarded {
            return Ok(());
        }
        ensure_directory_identity(self.directory()?, &self.path)?;
        let entries = directory_names(self.directory()?, &self.path)?;
        if entries.iter().any(|name| name != "frames") {
            return Err(VideoStorageError::new(
                &self.path,
                "unexpected workspace entries; cleanup refused",
            ));
        }
        if !entries.is_empty() {
            let frames = self.open_frames()?;
            let names = directory_names(&frames, &self.frames_path())?;
            if names.iter().any(|name| !owned_frame_name(name)) {
                return Err(VideoStorageError::new(
                    &self.path,
                    "unexpected frame entries; cleanup refused",
                ));
            }
            let mut validated = Vec::with_capacity(names.len());
            for name in names {
                let entry = frames
                    .open_file(OsStr::new(&name))
                    .map_err(|error| VideoStorageError::new(&self.frames_path(), error))?;
                validated.push(entry);
            }
            for entry in validated {
                entry
                    .discard()
                    .map_err(|error| VideoStorageError::new(&self.frames_path(), error))?;
            }
            frames
                .remove_empty()
                .map_err(|error| VideoStorageError::new(&self.path, error))?;
        }
        self.directory()?
            .clone()
            .remove_empty()
            .map_err(|error| VideoStorageError::new(&self.path, error))?;
        self.directory.take();
        self.discarded = true;
        Ok(())
    }

    /// Opens the fixed frame directory without following a replacement symlink.
    ///
    /// # Errors
    /// Returns missing/discarded/invalid-directory diagnostics.
    fn open_frames(&self) -> Result<Directory, VideoStorageError> {
        if self.discarded {
            return Err(VideoStorageError::new(
                &self.path,
                "workspace was already discarded",
            ));
        }
        self.directory()?
            .open_directory(OsStr::new("frames"))
            .map_err(|error| VideoStorageError::new(&self.frames_path(), error))
    }

    /// Returns the retained workspace until successful explicit removal releases it.
    /// # Errors
    /// Rejects a discarded workspace instead of reopening its visible path.
    fn directory(&self) -> Result<&Directory, VideoStorageError> {
        self.directory
            .as_ref()
            .ok_or_else(|| VideoStorageError::new(&self.path, "workspace was already discarded"))
    }
}

/// Opens one directory under the native no-follow retained-handle boundary.
///
/// # Errors
/// Returns the exact directory-open diagnostic.
fn open_directory(path: &Path) -> Result<Directory, VideoStorageError> {
    Directory::open(path).map_err(|error| VideoStorageError::new(path, error))
}

/// Obtains a random bounded filename component from the native system random source.
///
/// # Errors
/// Returns random-device open/read failures instead of falling back to predictable names.
fn random_suffix() -> Result<String, VideoStorageError> {
    filesystem::random_suffix()
        .map_err(|error| VideoStorageError::new(Path::new("system random source"), error))
}

/// Rejects visible directory paths that no longer identify their retained owner descriptor.
///
/// # Errors
/// Returns moved/replaced-directory or metadata diagnostics.
fn ensure_directory_identity(directory: &Directory, path: &Path) -> Result<(), VideoStorageError> {
    directory
        .ensure_path(path)
        .map_err(|error| VideoStorageError::new(path, error))
}

/// Checks caller-available space using the directory descriptor's filesystem.
///
/// # Errors
/// Returns filesystem-query, arithmetic-overflow or insufficient-space diagnostics.
fn require_space(directory: &Directory, path: &Path, bytes: u64) -> Result<(), VideoStorageError> {
    let available = directory
        .available_bytes()
        .map_err(|error| VideoStorageError::new(path, error))?;
    if available < bytes {
        return Err(VideoStorageError::new(
            path,
            "insufficient available storage",
        ));
    }
    Ok(())
}

/// Enumerates at most the known maximum frame count through the retained native directory.
///
/// # Errors
/// Returns directory-read failures, non-UTF-8 names or excessive entries without removing files.
fn directory_names(directory: &Directory, path: &Path) -> Result<Vec<String>, VideoStorageError> {
    directory
        .names(1_000_003, 1_000_003 * 50)
        .map_err(|error| VideoStorageError::new(path, error))?
        .into_iter()
        .map(|name| {
            name.into_string()
                .map_err(|_| VideoStorageError::new(path, "unexpected non-Unicode workspace name"))
        })
        .collect()
}

/// Recognizes only filenames produced by the current bounded PNG sequence writer.
fn owned_frame_name(name: &str) -> bool {
    if matches!(name, "manifest.json" | ".manifest.partial") {
        return true;
    }
    let frame = name
        .strip_prefix('.')
        .and_then(|name| name.strip_suffix(".partial"))
        .unwrap_or(name);
    frame
        .strip_prefix("frame-")
        .and_then(|name| name.strip_suffix(".png"))
        .is_some_and(|number| number.len() == 6 && number.bytes().all(|byte| byte.is_ascii_digit()))
}
