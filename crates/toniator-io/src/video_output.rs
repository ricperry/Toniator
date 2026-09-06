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
        let mut root = RenderWorkspace::create(Path::new("/tmp")).unwrap();
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
        fs::write(root.frames_path().join("user-notes.txt"), b"keep").unwrap();
        assert!(root.discard().is_err());
        assert!(root.frames_path().join("frame-000000.png").exists());
        fs::remove_file(root.frames_path().join("user-notes.txt")).unwrap();
        fs::create_dir(root.frames_path().join("frame-000001.png")).unwrap();
        assert!(root.discard().is_err());
        assert!(root.frames_path().join("frame-000000.png").exists());
        fs::remove_dir(root.frames_path().join("frame-000001.png")).unwrap();
        let root_path = root.path().to_owned();
        root.discard().unwrap();
        assert!(!root_path.exists());
    }
}

use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use std::{
    ffi::OsString,
    fs::{self, File},
    io::{Read, Seek},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, MetadataExt},
    },
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
/// only the original staging inode. Final publication atomically refuses an existing destination.
#[derive(Debug)]
pub struct VideoOutput {
    parent: File,
    parent_path: PathBuf,
    target: OsString,
    temporary: OsString,
    file: File,
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
        match rustix::fs::statat(&parent, &target, AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => {}
            Ok(_) => return Err(VideoStorageError::new(path, "destination already exists")),
            Err(error) => return Err(VideoStorageError::new(path, error)),
        }
        require_space(&parent, &parent_path, estimated_bytes)?;
        let temporary = OsString::from(format!(".toniator-video-{}.part", random_suffix()?));
        let file = File::from(
            rustix::fs::openat(
                &parent,
                &temporary,
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|error| VideoStorageError::new(path, error))?,
        );
        Ok(Self {
            parent,
            parent_path,
            target,
            temporary,
            file,
        })
    }

    /// Duplicates the owned file for an encoder or validator's standard stream.
    ///
    /// # Errors
    /// Returns descriptor duplication or seek failures without reopening a path.
    pub fn file_handle(&self) -> Result<File, VideoStorageError> {
        let mut file = self
            .file
            .try_clone()
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
        Ok(self
            .parent
            .metadata()
            .map_err(|error| VideoStorageError::new(&self.parent_path, error))?
            .dev()
            == workspace
                .directory
                .metadata()
                .map_err(|error| VideoStorageError::new(&workspace.path, error))?
                .dev())
    }

    /// Truncates only this owned staging file after a successful capability probe.
    ///
    /// # Errors
    /// Returns truncation/seek failures; the destination remains unpublished.
    pub fn clear(&self) -> Result<(), VideoStorageError> {
        self.file
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
        ensure_directory_identity(&self.parent, &self.parent_path)?;
        if !self.owns_temporary()
            || self
                .file
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
        self.file
            .sync_all()
            .map_err(|error| VideoStorageError::new(&self.path(), error))?;
        rustix::fs::renameat_with(
            &self.parent,
            &self.temporary,
            &self.parent,
            &self.target,
            RenameFlags::NOREPLACE,
        )
        .map_err(|error| VideoStorageError::new(&self.path(), error))?;
        self.parent
            .sync_all()
            .map_err(|error| VideoStorageError::new(&self.path(), error))?;
        Ok(self.path())
    }

    /// Checks the staging name still references the exclusively created regular file.
    fn owns_temporary(&self) -> bool {
        let Ok(actual) =
            rustix::fs::statat(&self.parent, &self.temporary, AtFlags::SYMLINK_NOFOLLOW)
        else {
            return false;
        };
        let Ok(owned) = self.file.metadata() else {
            return false;
        };
        actual.st_dev == owned.dev() && actual.st_ino == owned.ino()
    }
}
impl Drop for VideoOutput {
    /// Removes only the unpublished original temporary inode; never follows a replaced name.
    fn drop(&mut self) {
        if self.owns_temporary() {
            let _ = rustix::fs::unlinkat(&self.parent, &self.temporary, AtFlags::empty());
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
    directory: File,
    discarded: bool,
}
impl RenderWorkspace {
    /// Creates a random private render directory under `/tmp` or an explicit temporary parent.
    ///
    /// # Errors
    /// Returns parent, random-source, directory-creation or descriptor-opening failures.
    pub fn create(parent: &Path) -> Result<Self, VideoStorageError> {
        let parent =
            fs::canonicalize(parent).map_err(|error| VideoStorageError::new(parent, error))?;
        let path = parent.join(format!("toniator-render-{}", random_suffix()?));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|error| VideoStorageError::new(&path, error))?;
        let directory = open_directory(&path)?;
        Ok(Self {
            path,
            directory,
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
        require_space(&self.directory, &self.path, bytes)
    }

    /// Reads one retained numbered PNG through an owned directory handle and a caller byte bound.
    ///
    /// # Errors
    /// Rejects discarded workspaces, missing/nonregular files, path replacement or excess bytes.
    pub fn read_frame(&self, index: u64, maximum_bytes: u64) -> Result<Vec<u8>, VideoStorageError> {
        ensure_directory_identity(&self.directory, &self.path)?;
        let frames = self.open_frames()?;
        let name = format!("frame-{index:06}.png");
        let fd = rustix::fs::openat(
            &frames,
            &name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| VideoStorageError::new(&self.frames_path(), error))?;
        let file = File::from(fd);
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
        ensure_directory_identity(&self.directory, &self.path)?;
        let entries = directory_names(&self.directory, &self.path)?;
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
            for name in &names {
                let stat = rustix::fs::statat(&frames, name, AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(|error| VideoStorageError::new(&self.frames_path(), error))?;
                if rustix::fs::FileType::from_raw_mode(stat.st_mode)
                    != rustix::fs::FileType::RegularFile
                {
                    return Err(VideoStorageError::new(
                        &self.frames_path(),
                        "unexpected nonregular frame entry; cleanup refused",
                    ));
                }
            }
            for name in names {
                rustix::fs::unlinkat(&frames, &name, AtFlags::empty())
                    .map_err(|error| VideoStorageError::new(&self.frames_path(), error))?;
            }
            rustix::fs::unlinkat(&self.directory, "frames", AtFlags::REMOVEDIR)
                .map_err(|error| VideoStorageError::new(&self.path, error))?;
        }
        fs::remove_dir(&self.path).map_err(|error| VideoStorageError::new(&self.path, error))?;
        self.discarded = true;
        Ok(())
    }

    /// Opens the fixed frame directory without following a replacement symlink.
    ///
    /// # Errors
    /// Returns missing/discarded/invalid-directory diagnostics.
    fn open_frames(&self) -> Result<File, VideoStorageError> {
        if self.discarded {
            return Err(VideoStorageError::new(
                &self.path,
                "workspace was already discarded",
            ));
        }
        rustix::fs::openat(
            &self.directory,
            "frames",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(|error| VideoStorageError::new(&self.frames_path(), error))
    }
}

/// Opens one directory under the Linux-native no-follow descriptor boundary.
///
/// # Errors
/// Returns the exact directory-open diagnostic.
fn open_directory(path: &Path) -> Result<File, VideoStorageError> {
    rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| VideoStorageError::new(path, error))
}

/// Obtains a random bounded filename component from the Linux random device.
///
/// # Errors
/// Returns random-device open/read failures instead of falling back to predictable names.
fn random_suffix() -> Result<String, VideoStorageError> {
    let mut bytes = [0_u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| VideoStorageError::new(Path::new("/dev/urandom"), error))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Rejects visible directory paths that no longer identify their retained owner descriptor.
///
/// # Errors
/// Returns moved/replaced-directory or metadata diagnostics.
fn ensure_directory_identity(directory: &File, path: &Path) -> Result<(), VideoStorageError> {
    let actual = fs::symlink_metadata(path).map_err(|error| VideoStorageError::new(path, error))?;
    let owned = directory
        .metadata()
        .map_err(|error| VideoStorageError::new(path, error))?;
    if !actual.is_dir() || (actual.dev(), actual.ino()) != (owned.dev(), owned.ino()) {
        return Err(VideoStorageError::new(
            path,
            "owned directory was moved or replaced",
        ));
    }
    Ok(())
}

/// Checks caller-available space using the directory descriptor's filesystem.
///
/// # Errors
/// Returns filesystem-query, arithmetic-overflow or insufficient-space diagnostics.
fn require_space(directory: &File, path: &Path, bytes: u64) -> Result<(), VideoStorageError> {
    let stats =
        rustix::fs::fstatvfs(directory).map_err(|error| VideoStorageError::new(path, error))?;
    let available = stats
        .f_bavail
        .checked_mul(stats.f_frsize)
        .ok_or_else(|| VideoStorageError::new(path, "available space overflowed"))?;
    if available < bytes {
        return Err(VideoStorageError::new(
            path,
            "insufficient available storage",
        ));
    }
    Ok(())
}

/// Enumerates at most the known maximum frame count through a stable Linux descriptor path.
///
/// # Errors
/// Returns directory-read failures, non-UTF-8 names or excessive entries without removing files.
fn directory_names(directory: &File, path: &Path) -> Result<Vec<String>, VideoStorageError> {
    let entries = fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
        .map_err(|error| VideoStorageError::new(path, error))?;
    let mut names = Vec::new();
    for entry in entries {
        if names.len() >= 1_000_003 {
            return Err(VideoStorageError::new(path, "too many workspace entries"));
        }
        names.push(
            entry
                .map_err(|error| VideoStorageError::new(path, error))?
                .file_name()
                .into_string()
                .map_err(|_| VideoStorageError::new(path, "unexpected non-UTF-8 workspace name"))?,
        );
    }
    Ok(names)
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
