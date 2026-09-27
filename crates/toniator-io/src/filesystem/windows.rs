//! Adjacent Windows staging mechanics; callers own validation and durability policy.

use super::{Directory, Entry, PublishMode, random_suffix};
use std::{
    ffi::OsString,
    fs::File,
    io,
    path::{Path, PathBuf},
};

/// Retains adjacent private staging and its publication parent until explicit rename or cleanup.
pub(crate) struct Staging {
    parent: Directory,
    parent_path: PathBuf,
    target: OsString,
    entry: Option<Entry>,
}

impl Staging {
    /// Creates an exclusive private sibling through the retained no-follow publication directory.
    /// # Errors
    /// Rejects invalid targets and returns parent, random-source, ACL, or exclusive-create failures.
    pub(crate) fn create(path: &Path) -> io::Result<Self> {
        let target = path
            .file_name()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "destination must name a file")
            })?
            .to_owned();
        let parent_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_owned();
        let parent = Directory::open(&parent_path)?;
        let temporary = OsString::from(format!(".toniator-{}.tmp", random_suffix()?));
        let entry = parent.create_file(&temporary)?;
        Ok(Self {
            parent,
            parent_path,
            target,
            entry: Some(entry),
        })
    }

    /// Duplicates the retained staging file for ordinary writers without reopening its path.
    /// # Errors
    /// Returns native handle duplication failure.
    pub(crate) fn file(&self) -> io::Result<File> {
        self.entry
            .as_ref()
            .ok_or_else(|| io::Error::other("staging handle released"))?
            .try_clone_file()
    }

    /// Atomically renames staging and irrevocably revokes cleanup after publication.
    ///
    /// The caller must synchronize contents before rename and choose its later directory-sync policy.
    /// # Errors
    /// Returns ownership, collision, or native rename failures without weakening no-follow guarantees.
    pub(crate) fn rename(&mut self, mode: PublishMode) -> io::Result<()> {
        self.parent.ensure_path(&self.parent_path)?;
        self.entry
            .as_mut()
            .ok_or_else(|| io::Error::other("staging handle released"))?
            .rename(&self.parent, &self.target, mode)
    }

    /// Fully synchronizes publication-directory metadata after successful rename.
    /// # Errors
    /// Returns the real native full-flush failure; a published target remains visible.
    pub(crate) fn sync(&self) -> io::Result<()> {
        self.parent.sync()
    }
}

impl Drop for Staging {
    /// Discards only this verified unpublished staging link and preserves every published target.
    fn drop(&mut self) {
        if let Some(entry) = self.entry.take()
            && !entry.is_published()
        {
            let _ = entry.discard();
        }
    }
}
