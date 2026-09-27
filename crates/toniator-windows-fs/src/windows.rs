use std::ffi::{OsStr, OsString, c_void};
use std::fs::File;
use std::io;
use std::mem::{offset_of, size_of};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::{addr_of, null, null_mut};
use std::sync::{Arc, Mutex};
use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_CREATE, FILE_DIRECTORY_FILE, FILE_NAMES_INFORMATION, FILE_NON_DIRECTORY_FILE, FILE_OPEN,
    FILE_OPEN_REPARSE_POINT, FILE_RENAME_INFORMATION, FILE_RENAME_POSIX_SEMANTICS,
    FILE_RENAME_REPLACE_IF_EXISTS, FILE_SYNCHRONOUS_IO_NONALERT, FileFsFullSizeInformation,
    FileNamesInformation, FileRenameInformationEx, NtCreateFile, NtFlushBuffersFileEx,
    NtQueryDirectoryFile, NtQueryVolumeInformationFile, NtSetInformationFile,
};
use windows_sys::Wdk::System::SystemServices::FILE_FS_FULL_SIZE_INFORMATION;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Identifies one filesystem object without trusting its current visible path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Identity {
    pub volume: u64,
    pub file: [u8; 16],
}

/// Selects atomic collision handling; callers own whether replacement is authorized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishMode {
    NoReplace,
    Replace,
}

/// Retains one non-reparse directory; relative operations survive visible path replacement.
///
/// Clones share the retained handle. Drop closes handles and never removes recovery artifacts.
#[derive(Clone, Debug)]
pub struct Directory {
    inner: Arc<DirectoryInner>,
}

#[derive(Debug)]
struct DirectoryInner {
    file: File,
    identity: Identity,
    origin: Option<Origin>,
    enumeration: Mutex<()>,
}

#[derive(Clone, Debug)]
struct Origin {
    parent: Directory,
    name: OsString,
}

/// Retains a regular file and remembers the caller-selected staging component.
///
/// Mutation retains that component's current link after verifying the same file object,
/// including same-object hard links. Drop preserves files; publication revokes cleanup before sync.
#[derive(Debug)]
pub struct Entry {
    file: File,
    identity: Identity,
    origin: Origin,
    published: bool,
}

impl Directory {
    /// Opens the chosen existing directory and rejects a relevant reparse leaf.
    ///
    /// Ancestor selection remains caller authority, matching an ordinary no-follow leaf open.
    /// # Errors
    /// Rejects NUL-containing paths and returns open/type/query failures; no privileges are enabled.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = open_path(
            path,
            FILE_GENERIC_READ | FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY,
        )?;
        Self::from_file(file, None)
    }

    /// Validates a retained directory and associates only an explicitly created/opened child origin.
    /// # Errors
    /// Rejects reparse points, non-directories, or identity-query failures.
    fn from_file(file: File, origin: Option<Origin>) -> io::Result<Self> {
        check_type(&file, true)?;
        let identity = identity(&file)?;
        Ok(Self {
            inner: Arc::new(DirectoryInner {
                file,
                identity,
                origin,
                enumeration: Mutex::new(()),
            }),
        })
    }

    /// Returns the retained object identity; path changes do not alter it.
    pub fn identity(&self) -> Identity {
        self.inner.identity
    }

    /// Tests a visible path against this retained owner without following its reparse leaf.
    /// # Errors
    /// Returns path-open errors or reports moved/replaced directory identity.
    pub fn ensure_path(&self, path: &Path) -> io::Result<()> {
        if Self::open(path)?.identity() != self.identity() {
            return Err(io::Error::other(
                "owned directory path was moved or replaced",
            ));
        }
        Ok(())
    }

    /// Creates one private directory exclusively relative to this retained parent.
    /// # Errors
    /// Rejects invalid components/collisions and returns ACL/native/type failures.
    pub fn create_directory(&self, name: &OsStr) -> io::Result<Self> {
        let security = private_security()?;
        let file = self.open_relative(
            name,
            FILE_CREATE,
            FILE_DIRECTORY_FILE,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE,
            security.0.cast(),
        )?;
        Self::from_file(
            file,
            Some(Origin {
                parent: self.clone(),
                name: name.to_owned(),
            }),
        )
    }

    /// Opens one child directory through the retained parent and rejects reparse leaves.
    /// # Errors
    /// Returns component/open/type/identity failures without reopening the parent path.
    pub fn open_directory(&self, name: &OsStr) -> io::Result<Self> {
        let file = self.open_relative(
            name,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE,
            null(),
        )?;
        Self::from_file(
            file,
            Some(Origin {
                parent: self.clone(),
                name: name.to_owned(),
            }),
        )
    }

    /// Creates one private regular file exclusively relative to this retained parent.
    /// # Errors
    /// Rejects invalid components/collisions and returns ACL/native/type failures.
    pub fn create_file(&self, name: &OsStr) -> io::Result<Entry> {
        let security = private_security()?;
        let file = self.open_relative(
            name,
            FILE_CREATE,
            FILE_NON_DIRECTORY_FILE,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE,
            security.0.cast(),
        )?;
        self.file_entry(file, name)
    }

    /// Opens a regular child for reading and explicit handle-bound deletion.
    /// # Errors
    /// Rejects invalid components, reparse leaves, nonregular objects, or native failures.
    pub fn open_file(&self, name: &OsStr) -> io::Result<Entry> {
        let file = self.open_relative(
            name,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
            FILE_GENERIC_READ | DELETE,
            null(),
        )?;
        self.file_entry(file, name)
    }

    /// Tests whether any relative leaf exists without following it or requiring deletion rights.
    /// # Errors
    /// Rejects invalid components and returns access/query failures other than a missing leaf.
    pub fn entry_exists(&self, name: &OsStr) -> io::Result<bool> {
        match self.open_relative(
            name,
            FILE_OPEN,
            0,
            FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            null(),
        ) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Validates a regular file and records the caller-selected original child name.
    /// # Errors
    /// Rejects reparse/nonregular objects or identity-query failures.
    fn file_entry(&self, file: File, name: &OsStr) -> io::Result<Entry> {
        check_type(&file, false)?;
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

    /// Opens exactly one validated component with official NT structures and a retained root.
    /// # Errors
    /// Returns validation/native errors; reparse resolution is disabled and handles are non-inheritable.
    fn open_relative(
        &self,
        name: &OsStr,
        disposition: u32,
        kind: u32,
        access: u32,
        security: *const windows_sys::Win32::Security::SECURITY_DESCRIPTOR,
    ) -> io::Result<File> {
        let mut wide = component(name)?;
        let length = u16::try_from(wide.len() * 2)
            .map_err(|_| invalid("relative component exceeds the NT string bound"))?;
        let string = UNICODE_STRING {
            Length: length,
            MaximumLength: length,
            Buffer: wide.as_mut_ptr(),
        };
        let attributes = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: self.inner.file.as_raw_handle(),
            ObjectName: &string,
            Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
            SecurityDescriptor: security,
            SecurityQualityOfService: null(),
        };
        let mut handle = null_mut();
        let mut iosb = IO_STATUS_BLOCK::default();
        let status = unsafe {
            NtCreateFile(
                &mut handle,
                access,
                &attributes,
                &mut iosb,
                null(),
                FILE_ATTRIBUTE_NORMAL,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                disposition,
                kind | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                null(),
                0,
            )
        };
        nt_result(status)?;
        Ok(File::from(unsafe { OwnedHandle::from_raw_handle(handle) }))
    }

    /// Returns caller-available bytes from the retained directory's volume, respecting quotas.
    /// # Errors
    /// Returns native query failures, negative allocation counts, or checked multiplication overflow.
    pub fn available_bytes(&self) -> io::Result<u64> {
        capacity(&self.inner.file)
    }

    /// Flushes directory metadata and synchronizes storage using native full-flush flags zero.
    /// # Errors
    /// Returns the actual native flush failure rather than substituting volume-wide or weaker flushing.
    pub fn sync(&self) -> io::Result<()> {
        let mut iosb = IO_STATUS_BLOCK::default();
        nt_result(unsafe {
            NtFlushBuffersFileEx(self.inner.file.as_raw_handle(), 0, null(), 0, &mut iosb)
        })
    }

    /// Enumerates relative names with explicit caller entry and aggregate UTF-16 byte bounds.
    ///
    /// Enumeration never deletes entries or interprets caller cleanup policy; mutations are not a snapshot.
    /// # Errors
    /// Rejects exceeded bounds, malformed native responses, poisoned serialization, or native failures.
    pub fn names(
        &self,
        maximum_entries: usize,
        maximum_name_bytes: usize,
    ) -> io::Result<Vec<OsString>> {
        let _guard = self
            .inner
            .enumeration
            .lock()
            .map_err(|_| io::Error::other("directory enumeration lock poisoned"))?;
        let mut buffer = vec![0usize; 64 * 1024 / size_of::<usize>()];
        let buffer_bytes = buffer.len() * size_of::<usize>();
        let mut names = Vec::new();
        let mut bytes = 0usize;
        let mut restart = true;
        loop {
            let mut iosb = IO_STATUS_BLOCK::default();
            let status = unsafe {
                NtQueryDirectoryFile(
                    self.inner.file.as_raw_handle(),
                    null_mut(),
                    None,
                    null(),
                    &mut iosb,
                    buffer.as_mut_ptr().cast(),
                    buffer_bytes as u32,
                    FileNamesInformation,
                    false,
                    null(),
                    restart,
                )
            };
            restart = false;
            if status == STATUS_NO_MORE_FILES {
                break;
            }
            nt_result(status)?;
            let used = iosb.Information;
            if used == 0 || used > buffer_bytes {
                return Err(io::Error::other("invalid native enumeration size"));
            }
            let mut offset = 0usize;
            loop {
                let header = offset_of!(FILE_NAMES_INFORMATION, FileName);
                if offset.checked_add(header).is_none_or(|end| end > used) {
                    return Err(io::Error::other("truncated native directory entry"));
                }
                // The initialized aligned buffer contains the checked fixed header; fields may be unaligned.
                let record = unsafe {
                    buffer
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset)
                        .cast::<FILE_NAMES_INFORMATION>()
                };
                let next = unsafe { addr_of!((*record).NextEntryOffset).read_unaligned() } as usize;
                let length =
                    unsafe { addr_of!((*record).FileNameLength).read_unaligned() } as usize;
                let end = offset
                    .checked_add(header)
                    .and_then(|start| start.checked_add(length))
                    .ok_or_else(|| io::Error::other("directory entry size overflowed"))?;
                if !length.is_multiple_of(2) || end > used {
                    return Err(io::Error::other("invalid native directory name size"));
                }
                let wide = unsafe {
                    std::slice::from_raw_parts(
                        buffer
                            .as_ptr()
                            .cast::<u8>()
                            .add(offset + header)
                            .cast::<u16>(),
                        length / 2,
                    )
                };
                let name = OsString::from_wide(wide);
                if name != "." && name != ".." {
                    bytes = bytes
                        .checked_add(length)
                        .ok_or_else(|| io::Error::other("directory name byte count overflowed"))?;
                    if names.len() >= maximum_entries || bytes > maximum_name_bytes {
                        return Err(io::Error::other(
                            "directory enumeration exceeds caller bounds",
                        ));
                    }
                    names.push(name);
                }
                if next == 0 {
                    break;
                }
                if next < header + length
                    || !next.is_multiple_of(2)
                    || offset.checked_add(next).is_none_or(|start| start >= used)
                {
                    return Err(io::Error::other("invalid native directory continuation"));
                }
                offset += next;
            }
        }
        Ok(names)
    }

    /// Explicitly marks an empty, still-owned child directory for deletion on final handle close.
    ///
    /// The chosen root has no child-deletion authority; Drop never invokes this operation.
    /// # Errors
    /// Rejects root deletion, moved/replaced membership, nonempty directories, or native failures.
    pub fn remove_empty(self) -> io::Result<()> {
        let origin = self
            .inner
            .origin
            .as_ref()
            .ok_or_else(|| invalid("chosen root has no owned child origin"))?;
        let owned = origin.open_owned(self.identity())?;
        mark_delete(&owned)
    }
}

impl Origin {
    /// Retains the selected component's current link after checking the retained object identity.
    /// # Errors
    /// Returns missing/different-object/reparse membership or native query failures.
    fn open_owned(&self, expected: Identity) -> io::Result<File> {
        let file = self.parent.open_relative(
            &self.name,
            FILE_OPEN,
            0,
            FILE_GENERIC_READ | DELETE,
            null(),
        )?;
        reject_reparse(&file)?;
        if identity(&file)? != expected {
            return Err(io::Error::other("owned child name was moved or replaced"));
        }
        Ok(file)
    }
}

impl Entry {
    /// Returns the retained regular file for ordinary safe IO without reopening any path.
    pub fn file(&self) -> &File {
        &self.file
    }

    /// Returns this retained file's filesystem identity.
    pub fn identity(&self) -> Identity {
        self.identity
    }

    /// Duplicates the regular file handle for safe encoder/validator IO and future process inheritance.
    /// # Errors
    /// Returns the actual handle-duplication failure; no path is reopened.
    pub fn try_clone_file(&self) -> io::Result<File> {
        self.file.try_clone()
    }

    /// Reopens the retained regular object for reading with a new, independent cursor at zero.
    ///
    /// Visible staging paths are never consulted; concurrent readers cannot change each other's offsets.
    /// # Errors
    /// Returns native reopen/type/identity failures rather than reopening a possibly replaced pathname.
    pub fn independent_reader(&self) -> io::Result<File> {
        let handle = unsafe {
            ReOpenFile(
                self.file.as_raw_handle(),
                FILE_GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                FILE_FLAG_OPEN_REPARSE_POINT,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let file = File::from(unsafe { OwnedHandle::from_raw_handle(handle) });
        check_type(&file, false)?;
        if identity(&file)? != self.identity {
            return Err(io::Error::other("reopened reader identity changed"));
        }
        Ok(file)
    }

    /// Reports that rename already published this file, including when subsequent directory sync failed.
    pub fn is_published(&self) -> bool {
        self.published
    }

    /// Synchronizes file contents, atomically publishes through retained directory handles, then syncs the parent.
    /// # Errors
    /// Returns ownership, file-sync, collision, rename or parent-sync failures; a published file is never staging again.
    pub fn publish(
        &mut self,
        parent: &Directory,
        name: &OsStr,
        mode: PublishMode,
    ) -> io::Result<()> {
        self.publish_with_sync(parent, name, mode, Directory::sync)
    }

    /// Publishes through the production rename boundary and permits focused synchronization-failure testing.
    /// # Errors
    /// Returns validation/ownership/sync/rename failures; rename success irrevocably revokes cleanup authority.
    fn publish_with_sync(
        &mut self,
        parent: &Directory,
        name: &OsStr,
        mode: PublishMode,
        sync: impl FnOnce(&Directory) -> io::Result<()>,
    ) -> io::Result<()> {
        if self.published {
            return Err(invalid("file was already published"));
        }
        self.file.sync_all()?;
        self.rename(parent, name, mode)?;
        sync(parent)
    }

    /// Atomically renames the verified staging link and immediately revokes staging cleanup.
    ///
    /// File and directory synchronization remain separate caller-owned barriers. Rename success
    /// makes this entry published even if a later caller operation fails. Replacement keeps readers'
    /// old handles valid while subsequent opens observe the new file, using native POSIX rename semantics.
    /// # Errors
    /// Rejects repeated publication, invalid components, moved/replaced staging, collisions, or native failures.
    pub fn rename(
        &mut self,
        parent: &Directory,
        name: &OsStr,
        mode: PublishMode,
    ) -> io::Result<()> {
        if self.published {
            return Err(invalid("file was already published"));
        }
        let source = self.origin.open_owned(self.identity)?;
        let wide = component(name)?;
        let bytes = wide.len() * 2;
        let total = size_of::<FILE_RENAME_INFORMATION>() + bytes;
        let mut buffer = vec![0usize; total.div_ceil(size_of::<usize>())];
        let information = buffer.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
        // The usize buffer provides native alignment, complete header space and checked UTF-16 payload space.
        unsafe {
            (*information).Anonymous.Flags = match mode {
                PublishMode::NoReplace => 0,
                PublishMode::Replace => FILE_RENAME_REPLACE_IF_EXISTS | FILE_RENAME_POSIX_SEMANTICS,
            };
            (*information).RootDirectory = parent.inner.file.as_raw_handle();
            (*information).FileNameLength = bytes as u32;
            std::ptr::copy_nonoverlapping(
                wide.as_ptr(),
                addr_of!((*information).FileName).cast_mut().cast::<u16>(),
                wide.len(),
            );
        }
        let mut iosb = IO_STATUS_BLOCK::default();
        nt_result(unsafe {
            NtSetInformationFile(
                source.as_raw_handle(),
                &mut iosb,
                information.cast(),
                total as u32,
                FileRenameInformationEx,
            )
        })?;
        self.published = true;
        Ok(())
    }

    /// Explicitly marks the unpublished selected staging link for deletion on final close.
    /// # Errors
    /// Refuses published files or missing/different-object staging, and returns native deletion failures.
    pub fn discard(self) -> io::Result<()> {
        if self.published {
            return Err(invalid("published output cannot be discarded as staging"));
        }
        let owned = self.origin.open_owned(self.identity)?;
        mark_delete(&owned)
    }
}

/// Queries caller-available volume bytes through an existing file or directory with attribute-only access.
///
/// The retained no-follow leaf does not require creation, write-content, or deletion rights.
/// # Errors
/// Rejects NUL paths or reparse leaves and returns native open/query or checked arithmetic failures.
pub fn available_space(path: &Path) -> io::Result<u64> {
    let file = open_path(path, FILE_READ_ATTRIBUTES)?;
    reject_reparse(&file)?;
    capacity(&file)
}

/// Opens one existing regular file for read-only IO without following its reparse leaf.
///
/// Reading never requires parent creation or file deletion authority.
/// # Errors
/// Rejects NUL paths, reparse points, nonregular objects, and native open/type failures.
pub fn open_regular_file(path: &Path) -> io::Result<File> {
    let file = open_path(path, FILE_GENERIC_READ)?;
    check_type(&file, false)?;
    Ok(file)
}

/// Opens the caller-selected existing leaf with explicit access and no reparse traversal.
/// # Errors
/// Rejects embedded NUL or native open failures; ownership of every successful handle transfers to File.
fn open_path(path: &Path, access: u32) -> io::Result<File> {
    let mut name: Vec<u16> = path.as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(invalid("chosen filesystem path contains NUL"));
    }
    name.push(0);
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(File::from(unsafe { OwnedHandle::from_raw_handle(handle) }))
}

/// Queries quota-aware caller capacity from a retained filesystem handle using checked multiplication.
/// # Errors
/// Returns native query failures, negative allocation counts, or byte-count overflow.
fn capacity(file: &File) -> io::Result<u64> {
    let mut information = FILE_FS_FULL_SIZE_INFORMATION::default();
    let mut iosb = IO_STATUS_BLOCK::default();
    nt_result(unsafe {
        NtQueryVolumeInformationFile(
            file.as_raw_handle(),
            &mut iosb,
            (&mut information as *mut FILE_FS_FULL_SIZE_INFORMATION).cast(),
            size_of::<FILE_FS_FULL_SIZE_INFORMATION>() as u32,
            FileFsFullSizeInformation,
        )
    })?;
    let units = u64::try_from(information.CallerAvailableAllocationUnits)
        .map_err(|_| io::Error::other("negative caller-available allocation count"))?;
    units
        .checked_mul(u64::from(information.SectorsPerAllocationUnit))
        .and_then(|bytes| bytes.checked_mul(u64::from(information.BytesPerSector)))
        .ok_or_else(|| io::Error::other("caller-available byte count overflowed"))
}

/// Generates 128 unpredictable bits for a caller-owned filename component without a predictable fallback.
/// # Errors
/// Returns the system random-source failure.
pub fn random_suffix() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Validates one exact relative component without assigning product filename or cleanup policy.
/// # Errors
/// Rejects empty/dot components, separators, NUL, and alternate-stream syntax.
fn component(name: &OsStr) -> io::Result<Vec<u16>> {
    let wide: Vec<u16> = name.encode_wide().collect();
    if wide.is_empty()
        || name == "."
        || name == ".."
        || wide.iter().any(|unit| matches!(*unit, 0 | 47 | 92 | 58))
    {
        return Err(invalid(
            "expected one relative component without an alternate data stream",
        ));
    }
    if wide.len() > u16::MAX as usize / 2 {
        return Err(invalid("relative component exceeds the NT string bound"));
    }
    Ok(wide)
}

/// Produces a stable invalid-input diagnostic for boundary validation.
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// Converts native failure status while retaining its raw hexadecimal value in the diagnostic.
/// # Errors
/// Rejects every status other than completed success, including unexpected pending completion.
fn nt_result(status: NTSTATUS) -> io::Result<()> {
    if status == STATUS_SUCCESS {
        return Ok(());
    }
    let code = unsafe { RtlNtStatusToDosError(status) };
    let error = io::Error::from_raw_os_error(code as i32);
    Err(io::Error::new(
        error.kind(),
        format!("NTSTATUS 0x{:08X}: {error}", status as u32),
    ))
}

/// Queries exact identity from an owned handle rather than a name.
/// # Errors
/// Returns the native file-identity query error.
fn identity(file: &File) -> io::Result<Identity> {
    let mut information = FILE_ID_INFO::default();
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&mut information as *mut FILE_ID_INFO).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(Identity {
        volume: information.VolumeSerialNumber,
        file: information.FileId.Identifier,
    })
}

/// Queries native attributes and rejects the relevant leaf reparse point.
/// # Errors
/// Returns attribute-query errors or a reparse-point rejection.
fn reject_reparse(file: &File) -> io::Result<u32> {
    let mut information = FILE_ATTRIBUTE_TAG_INFO::default();
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileAttributeTagInfo,
            (&mut information as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
            size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if information.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::other(
            "reparse leaf is not an owned filesystem object",
        ));
    }
    Ok(information.FileAttributes)
}

/// Confirms that an owned handle names the requested regular-file or directory kind.
/// # Errors
/// Returns native attribute errors or rejects reparse/device/wrong-kind objects.
fn check_type(file: &File, directory: bool) -> io::Result<()> {
    let attributes = reject_reparse(file)?;
    if (attributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
        || unsafe { GetFileType(file.as_raw_handle()) } != FILE_TYPE_DISK
    {
        return Err(io::Error::other(
            "owned object has the wrong filesystem type",
        ));
    }
    Ok(())
}

/// Marks the handle's original link for deletion without resolving a path or recursing.
/// # Errors
/// Returns native deletion errors, including nonempty-directory refusal.
fn mark_delete(file: &File) -> io::Result<()> {
    let information = FILE_DISPOSITION_INFO { DeleteFile: true };
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&information as *const FILE_DISPOSITION_INFO).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

struct LocalAllocation(*mut c_void);

impl Drop for LocalAllocation {
    /// Releases only the owned allocation returned by the Windows local-allocation APIs.
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

/// Reads the current process user SID without enabling or impersonating privileges.
/// # Errors
/// Returns native token/query/SID conversion failures.
fn current_user_sid() -> io::Result<String> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    let mut bytes = 0;
    unsafe {
        GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut bytes);
    }
    if bytes < size_of::<TOKEN_USER>() as u32 {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            bytes,
            &mut bytes,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let mut string = null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut string) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let allocation = LocalAllocation(string.cast());
    let mut length = 0;
    while unsafe { *string.add(length) } != 0 {
        length += 1;
    }
    let result = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(string, length) });
    drop(allocation);
    Ok(result)
}

/// Builds an explicit protected DACL granting full control only to the current process user at creation.
/// # Errors
/// Returns SID/security-descriptor conversion failures; no inherited broad DACL is used.
fn private_security() -> io::Result<LocalAllocation> {
    let sid = current_user_sid()?;
    let descriptor: Vec<u16> = format!("O:{sid}D:P(A;;FA;;;{sid})")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut security = null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor.as_ptr(),
            SDDL_REVISION_1,
            &mut security,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(LocalAllocation(security))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
