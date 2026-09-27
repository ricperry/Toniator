use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{self, Read},
    os::unix::{ffi::OsStrExt, fs::MetadataExt, io::AsRawFd},
    path::Path,
    sync::Arc,
};

/// Identifies an opened filesystem object independently of its visible path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    pub(crate) volume: u64,
    file: u64,
}

/// Selects existing caller-authorized atomic collision handling.
#[derive(Clone, Copy, Debug)]
pub(crate) enum PublishMode {
    NoReplace,
    Replace,
}

/// Retains a no-follow directory descriptor and optional parent-relative child ownership.
#[derive(Clone, Debug)]
pub(crate) struct Directory(Arc<DirectoryInner>);

#[derive(Debug)]
struct DirectoryInner {
    file: File,
    identity: Identity,
    origin: Option<Origin>,
}

#[derive(Clone, Debug)]
struct Origin {
    parent: Directory,
    name: OsString,
}

/// Retains one regular file and selected relative link; Drop never deletes artifacts.
#[derive(Debug)]
pub(crate) struct Entry {
    file: File,
    identity: Identity,
    origin: Origin,
    published: bool,
}

impl Directory {
    /// Opens the chosen directory with the existing Unix no-follow, close-on-exec boundary.
    /// # Errors
    /// Returns open or retained-identity failures; ancestor selection remains caller authority.
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let file = File::from(rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        Self::from_file(file, None)
    }

    /// Records a retained directory and only the explicitly selected child origin.
    /// # Errors
    /// Returns descriptor metadata failure.
    fn from_file(file: File, origin: Option<Origin>) -> io::Result<Self> {
        let identity = identity(&file)?;
        Ok(Self(Arc::new(DirectoryInner {
            file,
            identity,
            origin,
        })))
    }

    /// Returns retained volume/object identity without reopening a path.
    pub(crate) fn identity(&self) -> Identity {
        self.0.identity
    }

    /// Rejects a visible path whose no-follow directory identity differs from its retained owner.
    /// # Errors
    /// Returns missing/replaced paths, symlink leaves, or metadata failures.
    pub(crate) fn ensure_path(&self, path: &Path) -> io::Result<()> {
        let actual = fs::symlink_metadata(path)?;
        if !actual.is_dir()
            || (actual.dev(), actual.ino()) != (self.identity().volume, self.identity().file)
        {
            return Err(io::Error::other("owned directory was moved or replaced"));
        }
        Ok(())
    }

    /// Creates one exclusive private 0700 directory relative to the retained parent.
    /// # Errors
    /// Returns component/create/no-follow child-open failures and never removes a partial directory.
    pub(crate) fn create_directory(&self, name: &OsStr) -> io::Result<Self> {
        component(name)?;
        rustix::fs::mkdirat(&self.0.file, name, Mode::RUSR | Mode::WUSR | Mode::XUSR)?;
        self.open_directory(name)
    }

    /// Opens one no-follow child directory relative to this retained parent.
    /// # Errors
    /// Returns component, open, or identity failures.
    pub(crate) fn open_directory(&self, name: &OsStr) -> io::Result<Self> {
        component(name)?;
        let file = File::from(rustix::fs::openat(
            &self.0.file,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        Self::from_file(
            file,
            Some(Origin {
                parent: self.clone(),
                name: name.to_owned(),
            }),
        )
    }

    /// Creates one exclusive private 0600 regular file through the retained parent.
    /// # Errors
    /// Returns component/collision/open/metadata failures and preserves partial files on failure.
    pub(crate) fn create_file(&self, name: &OsStr) -> io::Result<Entry> {
        component(name)?;
        let file = File::from(rustix::fs::openat(
            &self.0.file,
            name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )?);
        self.entry(file, name)
    }

    /// Opens a regular no-follow child for bounded reads or explicit ownership-checked deletion.
    /// # Errors
    /// Rejects nonregular objects and returns component/open/metadata failures without blocking on FIFOs.
    pub(crate) fn open_file(&self, name: &OsStr) -> io::Result<Entry> {
        component(name)?;
        let file = File::from(rustix::fs::openat(
            &self.0.file,
            name,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        self.entry(file, name)
    }

    /// Binds a regular file to its selected relative link and retained identity.
    /// # Errors
    /// Returns metadata failure or rejects a nonregular entry.
    fn entry(&self, file: File, name: &OsStr) -> io::Result<Entry> {
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("expected a regular file"));
        }
        let identity = identity(&file)?;
        Ok(Entry {
            file,
            identity,
            origin: Origin {
                parent: self.clone(),
                name: name.to_owned(),
            },
            published: false,
        })
    }

    /// Tests any selected leaf's existence without following its symlink.
    /// # Errors
    /// Returns component or stat failures other than an absent leaf.
    pub(crate) fn entry_exists(&self, name: &OsStr) -> io::Result<bool> {
        component(name)?;
        match rustix::fs::statat(&self.0.file, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => Ok(true),
            Err(rustix::io::Errno::NOENT) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// Returns quota-aware available bytes from the retained directory filesystem.
    /// # Errors
    /// Returns descriptor query failure or checked byte-count overflow.
    pub(crate) fn available_bytes(&self) -> io::Result<u64> {
        let stats = rustix::fs::fstatvfs(&self.0.file)?;
        stats
            .f_bavail
            .checked_mul(stats.f_frsize)
            .ok_or_else(|| io::Error::other("available space overflowed"))
    }

    /// Synchronizes retained directory metadata without reopening its visible path.
    /// # Errors
    /// Returns the actual descriptor sync failure.
    pub(crate) fn sync(&self) -> io::Result<()> {
        self.0.file.sync_all()
    }

    /// Enumerates retained-directory names within caller entry and aggregate byte bounds.
    /// # Errors
    /// Returns descriptor directory-read failures or exceeded bounds without deleting entries.
    pub(crate) fn names(
        &self,
        maximum_entries: usize,
        maximum_name_bytes: usize,
    ) -> io::Result<Vec<OsString>> {
        let entries = fs::read_dir(format!("/proc/self/fd/{}", self.0.file.as_raw_fd()))?;
        let mut names = Vec::new();
        let mut bytes = 0usize;
        for entry in entries {
            let name = entry?.file_name();
            bytes = bytes
                .checked_add(name.as_bytes().len())
                .ok_or_else(|| io::Error::other("name bytes overflowed"))?;
            if names.len() >= maximum_entries || bytes > maximum_name_bytes {
                return Err(io::Error::other("workspace enumeration bound exceeded"));
            }
            names.push(name);
        }
        Ok(names)
    }

    /// Removes only an empty child whose selected relative link retains its original directory identity.
    /// # Errors
    /// Rejects root deletion, moved/replaced membership, nonempty directories, or unlink failures.
    pub(crate) fn remove_empty(self) -> io::Result<()> {
        let origin = self
            .0
            .origin
            .as_ref()
            .ok_or_else(|| io::Error::other("chosen root has no owned child origin"))?;
        origin.ensure(self.identity())?;
        rustix::fs::unlinkat(&origin.parent.0.file, &origin.name, AtFlags::REMOVEDIR)?;
        Ok(())
    }
}

impl Origin {
    /// Verifies the selected no-follow link still identifies the retained object.
    /// # Errors
    /// Returns missing/replaced membership or native stat failure.
    fn ensure(&self, expected: Identity) -> io::Result<()> {
        let actual =
            rustix::fs::statat(&self.parent.0.file, &self.name, AtFlags::SYMLINK_NOFOLLOW)?;
        if (actual.st_dev, actual.st_ino) != (expected.volume, expected.file) {
            return Err(io::Error::other("owned child name was moved or replaced"));
        }
        Ok(())
    }
}

impl Entry {
    /// Returns the retained regular file without reopening a path.
    pub(crate) fn file(&self) -> &File {
        &self.file
    }
    /// Returns retained object identity for bounded cleanup preflight.
    pub(crate) fn identity(&self) -> Identity {
        self.identity
    }
    /// Duplicates the retained file for safe ordinary IO or process inheritance.
    /// # Errors
    /// Returns descriptor duplication failure.
    pub(crate) fn try_clone_file(&self) -> io::Result<File> {
        self.file.try_clone()
    }
    /// Reports successful rename even when a subsequent caller sync fails.
    pub(crate) fn is_published(&self) -> bool {
        self.published
    }

    /// Atomically renames the verified staging link and immediately revokes cleanup authority.
    /// # Errors
    /// Rejects repeated publication, invalid components, replaced staging, collisions, or rename failure.
    pub(crate) fn rename(
        &mut self,
        parent: &Directory,
        name: &OsStr,
        mode: PublishMode,
    ) -> io::Result<()> {
        if self.published {
            return Err(io::Error::other("file was already published"));
        }
        component(name)?;
        self.origin.ensure(self.identity)?;
        rustix::fs::renameat_with(
            &self.origin.parent.0.file,
            &self.origin.name,
            &parent.0.file,
            name,
            match mode {
                PublishMode::NoReplace => RenameFlags::NOREPLACE,
                PublishMode::Replace => RenameFlags::empty(),
            },
        )?;
        self.published = true;
        Ok(())
    }

    /// Removes only the verified unpublished selected staging link; Drop never calls this.
    /// # Errors
    /// Rejects published files or replaced staging and returns unlink failure.
    pub(crate) fn discard(self) -> io::Result<()> {
        if self.published {
            return Err(io::Error::other(
                "published output cannot be discarded as staging",
            ));
        }
        self.origin.ensure(self.identity)?;
        rustix::fs::unlinkat(
            &self.origin.parent.0.file,
            &self.origin.name,
            AtFlags::empty(),
        )?;
        Ok(())
    }
}

/// Queries capacity for any existing path using the existing Unix statvfs authority.
/// # Errors
/// Returns path query failure or byte-count overflow.
pub(crate) fn available_space(path: &Path) -> io::Result<u64> {
    let stats = rustix::fs::statvfs(path)?;
    stats
        .f_bavail
        .checked_mul(stats.f_frsize)
        .ok_or_else(|| io::Error::other("available space overflowed"))
}

/// Synchronizes an existing directory while retaining the persistence caller's existing path policy.
/// # Errors
/// Returns the actual File open or sync failure.
pub(crate) fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

/// Obtains 128 unpredictable bits from the existing Unix random device without a fallback.
/// # Errors
/// Returns random-device open or exact-read failure.
pub(crate) fn random_suffix() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Records retained filesystem identity from descriptor metadata.
/// # Errors
/// Returns the actual metadata failure.
fn identity(file: &File) -> io::Result<Identity> {
    let metadata = file.metadata()?;
    Ok(Identity {
        volume: metadata.dev(),
        file: metadata.ino(),
    })
}

/// Restricts relative operations to one exact component without interpreting product filename policy.
/// # Errors
/// Rejects empty/dot components, separators, or NUL.
fn component(name: &OsStr) -> io::Result<()> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || matches!(bytes, b"." | b"..")
        || bytes.contains(&b'/')
        || bytes.contains(&0)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected one relative component",
        ));
    }
    Ok(())
}
