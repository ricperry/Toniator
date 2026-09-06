//! Source-free document Preset archives and guarded atomic publication.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

#[cfg(all(test, unix))]
use std::os::unix::fs::PermissionsExt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use toniator_domain::{Document, DocumentConfiguration};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::{
    DOCUMENT_PRESET_FORMAT_VERSION, DOCUMENT_SCHEMA_VERSION, DocumentConfigurationDtoV8, LoadError,
    MAX_ARCHIVE_BYTES, MAX_DOCUMENT_BYTES, SaveError, declared_zip_entry_count,
    ensure_supported_file_compression, load_opened, read_limited, safe_archive_name,
};

const PRESET_ENTRY_NAME: &str = "preset.json";

/// Failure while reading, validating, or atomically publishing a document Preset.
#[derive(Debug)]
pub enum DocumentPresetError {
    Filesystem { path: PathBuf, context: String },
    Archive { context: String },
    Json { context: String },
    Kind { context: String },
    Version { context: String },
    EntryTopology { context: String },
    Limits { context: String },
    DomainValidation { context: String },
    StaleDestination { context: String },
    Project { source: LoadError },
}

impl DocumentPresetError {
    /// Returns stable frontend routing context for this Preset failure.
    pub const fn path(&self) -> &'static str {
        match self {
            Self::Filesystem { .. } => "filesystem",
            Self::Archive { .. } => "archive",
            Self::Json { .. } => "preset.json",
            Self::Kind { .. } => "preset.kind",
            Self::Version { .. } => "preset.version",
            Self::EntryTopology { .. } => "archive.entries",
            Self::Limits { .. } => "archive.limits",
            Self::DomainValidation { .. } => "document.validation",
            Self::StaleDestination { .. } => "preset.destination",
            Self::Project { source } => source.path(),
        }
    }

    /// Returns actionable detail without exposing internal error representation.
    pub fn context(&self) -> &str {
        match self {
            Self::Filesystem { context, .. }
            | Self::Archive { context }
            | Self::Json { context }
            | Self::Kind { context }
            | Self::Version { context }
            | Self::EntryTopology { context }
            | Self::Limits { context }
            | Self::DomainValidation { context }
            | Self::StaleDestination { context } => context,
            Self::Project { source } => source.context(),
        }
    }
}

impl std::fmt::Display for DocumentPresetError {
    /// Formats stable path and actionable context for frontend display.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path(), self.context())
    }
}

impl std::error::Error for DocumentPresetError {
    /// Retains the ordinary project-load failure as the sole nested source.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Project { source } => Some(source),
            _ => None,
        }
    }
}

/// An opaque observation of one exact destination before a Preset save.
///
/// The guard is path-bound. Existing targets retain a digest of bytes read
/// through a no-follow regular-file handle; missing targets require atomic
/// create-only publication.
#[derive(Clone, Debug)]
pub struct DocumentPresetWriteGuard {
    path: PathBuf,
    state: DestinationState,
}

/// Reports successful publication separately from a subsequent durability warning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentPresetSaveOutcome {
    /// A directory-sync failure does not undo an already published file.
    pub durability_warning: Option<String>,
}

#[derive(Clone, Debug)]
enum DestinationState {
    Missing,
    Existing([u8; 32]),
}

impl DocumentPresetWriteGuard {
    /// Reports whether overwrite confirmation is required for the observed destination.
    pub const fn is_existing(&self) -> bool {
        matches!(self.state, DestinationState::Existing(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArchiveKind {
    DocumentPreset,
    Project,
}

#[derive(Serialize, Deserialize)]
struct DocumentPresetEnvelopeDto {
    kind: String,
    document_preset_format_version: u32,
    document_schema_version: u32,
    configuration: DocumentConfigurationDtoV8,
}

/// Minimal dispatch metadata parsed before the configuration schema is decoded.
#[derive(Deserialize)]
struct DocumentPresetMetadataDto {
    kind: String,
    document_preset_format_version: u32,
    document_schema_version: u32,
}

/// Loads either a source-free document Preset or a complete current project as reusable configuration.
///
/// Archive shape selects the reader, so renaming a project cannot bypass the
/// ordinary project's source manifest, byte-length, digest, reference, schema,
/// and container validation. Both routes bind against `destination` before a
/// configuration is returned.
///
/// # Errors
///
/// Returns bounded no-follow filesystem/archive errors, strict Preset schema
/// errors, ordinary project load errors, or complete destination validation
/// errors without mutating either document or selected file.
pub fn load_document_preset(
    path: &Path,
    destination: &Document,
) -> Result<DocumentConfiguration, DocumentPresetError> {
    let (kind, file) = classify_archive(path)?;
    match kind {
        ArchiveKind::Project => {
            let loaded = load_opened(path, file)
                .map_err(|source| DocumentPresetError::Project { source })?;
            let configuration = DocumentConfiguration::capture(loaded.document());
            configuration.bind(destination).map_err(|error| {
                DocumentPresetError::DomainValidation {
                    context: error.to_string(),
                }
            })?;
            Ok(configuration)
        }
        ArchiveKind::DocumentPreset => load_source_free_preset(path, file, destination),
    }
}

/// Captures the exact regular-file state used by create-only or guarded-overwrite publication.
///
/// # Errors
///
/// Rejects symlinks, nonregular files, files above the Preset archive limit,
/// and read failures. A missing final path produces a create-only guard.
pub fn capture_document_preset_destination(
    path: &Path,
) -> Result<DocumentPresetWriteGuard, DocumentPresetError> {
    let state = match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                return Err(filesystem_error(
                    path,
                    "document Preset destination must be a regular non-symlink file",
                ));
            }
            DestinationState::Existing(fingerprint_regular_file(path, MAX_DOCUMENT_BYTES)?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DestinationState::Missing,
        Err(error) => return Err(filesystem_error(path, error.to_string())),
    };
    Ok(DocumentPresetWriteGuard {
        path: path.to_owned(),
        state,
    })
}

/// Serializes and atomically publishes one immutable source-free configuration snapshot.
///
/// Missing destinations use atomic no-replace publication. Existing targets
/// are fingerprinted again immediately before an atomic rename. That overwrite
/// check is an explicit single-writer boundary: an unrelated cross-process
/// writer can still race between the final digest check and rename.
///
/// # Errors
///
/// Returns serialization, size, stale/mismatched guard, temporary-file, sync,
/// or atomic publication failures while preserving the prior target bytes.
pub fn save_document_preset(
    path: &Path,
    configuration: &DocumentConfiguration,
    guard: &DocumentPresetWriteGuard,
) -> Result<DocumentPresetSaveOutcome, DocumentPresetError> {
    if guard.path != path {
        return Err(DocumentPresetError::StaleDestination {
            context: "document Preset write guard belongs to a different path".into(),
        });
    }
    let envelope = DocumentPresetEnvelopeDto {
        kind: "document_preset".into(),
        document_preset_format_version: DOCUMENT_PRESET_FORMAT_VERSION,
        document_schema_version: DOCUMENT_SCHEMA_VERSION,
        configuration: DocumentConfigurationDtoV8::from_configuration(configuration)
            .map_err(map_save_error)?,
    };
    let mut json = serde_json::to_vec(&envelope).map_err(|error| DocumentPresetError::Json {
        context: error.to_string(),
    })?;
    json.push(b'\n');
    if json.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(DocumentPresetError::Limits {
            context: "preset.json exceeds the 4 MiB document Preset limit".into(),
        });
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let (temporary, file) = create_temporary_file(parent, path)?;
    let result = write_preset_archive(file, &json)
        .and_then(|()| publish_temporary(&temporary, path, &guard.state));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Selects the Preset or ordinary-project reader from validated ZIP entry names.
///
/// # Errors
///
/// Rejects oversized, symlinked, nonregular, malformed, duplicate, mixed, or
/// unrecognized archive topology before either format parser is invoked.
fn classify_archive(path: &Path) -> Result<(ArchiveKind, File), DocumentPresetError> {
    let mut file = open_regular_file(path, MAX_ARCHIVE_BYTES)?;
    let length = file
        .metadata()
        .map_err(|error| filesystem_error(path, error.to_string()))?
        .len();
    let declared = declared_zip_entry_count(&mut file, length).map_err(map_load_error)?;
    let mut archive = ZipArchive::new(file).map_err(|error| DocumentPresetError::Archive {
        context: error.to_string(),
    })?;
    if archive.len() != declared {
        return Err(DocumentPresetError::EntryTopology {
            context: "duplicate or conflicting central-directory entry names".into(),
        });
    }
    let mut has_preset = false;
    let mut has_document = false;
    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|error| DocumentPresetError::Archive {
                context: error.to_string(),
            })?;
        let name = entry.name();
        if !safe_archive_name(name) {
            return Err(DocumentPresetError::EntryTopology {
                context: format!("unsafe archive entry name: {name:?}"),
            });
        }
        has_preset |= name == PRESET_ENTRY_NAME;
        has_document |= name == "document.json";
    }
    let kind = match (has_preset, has_document) {
        (true, false) => ArchiveKind::DocumentPreset,
        (false, true) => ArchiveKind::Project,
        (true, true) => Err(DocumentPresetError::EntryTopology {
            context: "archive mixes document Preset and project entries".into(),
        })?,
        (false, false) => Err(DocumentPresetError::Kind {
            context: "archive is neither a document Preset nor a current project".into(),
        })?,
    };
    Ok((kind, archive.into_inner()))
}

/// Reads, strictly decodes, version-checks, and destination-validates one source-free Preset.
///
/// # Errors
///
/// Rejects every entry other than one bounded `preset.json`, unsupported ZIP
/// features, ignored fields at any nested path, wrong kind/version, trailing
/// JSON, and invalid destination-bound domain state.
fn load_source_free_preset(
    path: &Path,
    mut file: File,
    destination: &Document,
) -> Result<DocumentConfiguration, DocumentPresetError> {
    let length = file
        .metadata()
        .map_err(|error| filesystem_error(path, error.to_string()))?
        .len();
    if length > MAX_DOCUMENT_BYTES {
        return Err(DocumentPresetError::Limits {
            context: "document Preset archive exceeds the 4 MiB limit".into(),
        });
    }
    let declared = declared_zip_entry_count(&mut file, length).map_err(map_load_error)?;
    let mut archive = ZipArchive::new(file).map_err(|error| DocumentPresetError::Archive {
        context: error.to_string(),
    })?;
    if declared != 1 || archive.len() != 1 {
        return Err(DocumentPresetError::EntryTopology {
            context: "document Preset archive must contain exactly preset.json".into(),
        });
    }
    {
        let entry = archive
            .by_index_raw(0)
            .map_err(|error| DocumentPresetError::Archive {
                context: error.to_string(),
            })?;
        if entry.name() != PRESET_ENTRY_NAME || !entry.is_file() {
            return Err(DocumentPresetError::EntryTopology {
                context: "document Preset archive must contain exactly preset.json".into(),
            });
        }
        if entry.encrypted() {
            return Err(DocumentPresetError::Archive {
                context: "encrypted preset.json is unsupported".into(),
            });
        }
        ensure_supported_file_compression(PRESET_ENTRY_NAME, entry.compression())
            .map_err(map_load_error)?;
        if entry.size() > MAX_DOCUMENT_BYTES {
            return Err(DocumentPresetError::Limits {
                context: "preset.json exceeds the 4 MiB document Preset limit".into(),
            });
        }
    }
    let bytes = read_limited(&mut archive, 0, MAX_DOCUMENT_BYTES, PRESET_ENTRY_NAME)
        .map_err(map_load_error)?;
    let metadata: DocumentPresetMetadataDto =
        serde_json::from_slice(&bytes).map_err(|error| DocumentPresetError::Json {
            context: error.to_string(),
        })?;
    if metadata.kind != "document_preset" {
        return Err(DocumentPresetError::Kind {
            context: format!("unsupported resource kind {:?}", metadata.kind),
        });
    }
    if metadata.document_preset_format_version != DOCUMENT_PRESET_FORMAT_VERSION {
        return Err(DocumentPresetError::Version {
            context: format!(
                "unsupported document Preset format version {}",
                metadata.document_preset_format_version
            ),
        });
    }
    if metadata.document_schema_version != DOCUMENT_SCHEMA_VERSION {
        return Err(DocumentPresetError::Version {
            context: format!(
                "unsupported document configuration schema version {}",
                metadata.document_schema_version
            ),
        });
    }
    let mut ignored = Vec::new();
    let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
    let envelope: DocumentPresetEnvelopeDto =
        serde_ignored::deserialize(&mut deserializer, |path| ignored.push(path.to_string()))
            .map_err(|error| DocumentPresetError::Json {
                context: error.to_string(),
            })?;
    deserializer
        .end()
        .map_err(|error| DocumentPresetError::Json {
            context: error.to_string(),
        })?;
    if !ignored.is_empty() {
        return Err(DocumentPresetError::Json {
            context: format!("unknown field at {}", ignored.join(", ")),
        });
    }
    debug_assert_eq!(envelope.kind, metadata.kind);
    debug_assert_eq!(
        envelope.document_preset_format_version,
        metadata.document_preset_format_version
    );
    debug_assert_eq!(
        envelope.document_schema_version,
        metadata.document_schema_version
    );
    envelope
        .configuration
        .into_domain_configuration(destination)
        .map_err(|error| DocumentPresetError::DomainValidation {
            context: error.to_string(),
        })
}

/// Opens one path once without following a final symlink and enforces a real byte limit.
///
/// # Errors
///
/// Rejects symlinks, nonregular files, metadata/read-open failures, and files
/// whose metadata length exceeds `limit`.
fn open_regular_file(path: &Path, limit: u64) -> Result<File, DocumentPresetError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let file = options
        .open(path)
        .map_err(|error| filesystem_error(path, error.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|error| filesystem_error(path, error.to_string()))?;
    if !metadata.file_type().is_file() {
        return Err(filesystem_error(
            path,
            "document Preset input must be a regular non-symlink file",
        ));
    }
    if metadata.len() > limit {
        return Err(DocumentPresetError::Limits {
            context: format!("archive exceeds the {} byte limit", limit),
        });
    }
    Ok(file)
}

/// Hashes one exact bounded regular-file byte stream for stale-write detection.
///
/// # Errors
///
/// Returns no-follow regular-file, read, or byte-limit diagnostics without
/// retaining file contents.
fn fingerprint_regular_file(path: &Path, limit: u64) -> Result<[u8; 32], DocumentPresetError> {
    let mut file = open_regular_file(path, limit)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| filesystem_error(path, error.to_string()))?;
    if bytes.len() as u64 > limit {
        return Err(DocumentPresetError::Limits {
            context: format!("destination exceeds the {} byte limit", limit),
        });
    }
    Ok(Sha256::digest(&bytes).into())
}

/// Allocates one adjacent restrictive temporary file without replacing another writer's file.
///
/// # Errors
///
/// Returns parent/path encoding, permission, or exhausted-name diagnostics
/// before any destination mutation.
fn create_temporary_file(
    parent: &Path,
    destination: &Path,
) -> Result<(PathBuf, File), DocumentPresetError> {
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| filesystem_error(destination, "destination filename must be UTF-8"))?;
    for attempt in 0..1024_u32 {
        let temporary = parent.join(format!(".{name}.tmp.{}.{attempt}", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&temporary) {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(filesystem_error(&temporary, error.to_string())),
        }
    }
    Err(filesystem_error(
        destination,
        "document Preset temporary filename space exhausted",
    ))
}

/// Writes deterministic one-entry ZIP bytes and synchronizes the temporary file.
///
/// # Errors
///
/// Returns ZIP, write, finish, archive-size, or file-sync failures before publication.
fn write_preset_archive(file: File, json: &[u8]) -> Result<(), DocumentPresetError> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o100600);
    let mut writer = ZipWriter::new(file);
    writer
        .start_file(PRESET_ENTRY_NAME, options)
        .map_err(|error| DocumentPresetError::Archive {
            context: error.to_string(),
        })?;
    writer
        .write_all(json)
        .map_err(|error| DocumentPresetError::Archive {
            context: error.to_string(),
        })?;
    let file = writer
        .finish()
        .map_err(|error| DocumentPresetError::Archive {
            context: error.to_string(),
        })?;
    if file
        .metadata()
        .map_err(|error| filesystem_error(Path::new(""), error.to_string()))?
        .len()
        > MAX_DOCUMENT_BYTES
    {
        return Err(DocumentPresetError::Limits {
            context: "document Preset archive exceeds the 4 MiB limit".into(),
        });
    }
    file.sync_all()
        .map_err(|error| filesystem_error(Path::new(""), error.to_string()))
}

/// Publishes a complete temporary archive under the observed destination condition.
///
/// # Errors
///
/// Create-only publication fails atomically if the destination now exists.
/// Overwrite publication fails if the exact no-follow fingerprint changed, or
/// if rename fails. Successful publication returns any later directory-sync
/// failure as a warning because the target is already visible.
fn publish_temporary(
    temporary: &Path,
    destination: &Path,
    state: &DestinationState,
) -> Result<DocumentPresetSaveOutcome, DocumentPresetError> {
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match state {
        DestinationState::Missing => {
            fs::hard_link(temporary, destination).map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    DocumentPresetError::StaleDestination {
                        context: "document Preset destination now exists".into(),
                    }
                } else {
                    filesystem_error(destination, error.to_string())
                }
            })?;
            // Publication has committed. A leftover temporary hard link must
            // not turn successful create-only publication into a false failure.
            let _ = fs::remove_file(temporary);
        }
        DestinationState::Existing(expected) => {
            let actual =
                fingerprint_regular_file(destination, MAX_DOCUMENT_BYTES).map_err(|_| {
                    DocumentPresetError::StaleDestination {
                        context: "document Preset destination changed externally".into(),
                    }
                })?;
            if &actual != expected {
                return Err(DocumentPresetError::StaleDestination {
                    context: "document Preset destination changed externally".into(),
                });
            }
            fs::rename(temporary, destination)
                .map_err(|error| filesystem_error(destination, error.to_string()))?;
        }
    }
    Ok(DocumentPresetSaveOutcome {
        durability_warning: sync_directory(parent).err().map(|error| error.to_string()),
    })
}

/// Synchronizes one publication directory after a successful link/rename transition.
///
/// # Errors
///
/// Returns the directory open or sync failure after the target itself has been published.
fn sync_directory(path: &Path) -> Result<(), DocumentPresetError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| filesystem_error(path, error.to_string()))
}

/// Converts an existing project-load helper failure without erasing its category.
fn map_load_error(error: LoadError) -> DocumentPresetError {
    match error {
        LoadError::Filesystem { path, context } => {
            DocumentPresetError::Filesystem { path, context }
        }
        LoadError::Archive { context } => DocumentPresetError::Archive { context },
        LoadError::Json { context } => DocumentPresetError::Json { context },
        LoadError::Version { context } => DocumentPresetError::Version { context },
        LoadError::EntryTopology { context } => DocumentPresetError::EntryTopology { context },
        LoadError::Limits { context } => DocumentPresetError::Limits { context },
        LoadError::Integrity { context }
        | LoadError::SourceDocumentMismatch { context }
        | LoadError::DomainValidation { context } => {
            DocumentPresetError::DomainValidation { context }
        }
    }
}

/// Converts shared current-v7 DTO projection failures into Preset categories.
fn map_save_error(error: SaveError) -> DocumentPresetError {
    match error {
        SaveError::Filesystem { path, context } => {
            DocumentPresetError::Filesystem { path, context }
        }
        SaveError::Archive { context } => DocumentPresetError::Archive { context },
        SaveError::EntryTopology { context } => DocumentPresetError::EntryTopology { context },
        SaveError::Limits { context } => DocumentPresetError::Limits { context },
        SaveError::SourceDocumentMismatch { context } | SaveError::DomainValidation { context } => {
            DocumentPresetError::DomainValidation { context }
        }
    }
}

/// Builds one path-bearing filesystem error from any displayable context.
fn filesystem_error(path: &Path, context: impl Into<String>) -> DocumentPresetError {
    DocumentPresetError::Filesystem {
        path: path.to_owned(),
        context: context.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Confirms temporary archives use restrictive owner-only permissions on Unix.
    ///
    /// # Panics
    ///
    /// Panics when temporary allocation or metadata inspection fails, or when
    /// the filesystem does not retain the required permission bits.
    #[cfg(unix)]
    #[test]
    fn temporary_archives_are_private() {
        let directory = std::env::temp_dir().join(format!(
            "toniator-document-preset-permissions-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("test directory");
        let destination = directory.join("permission-test.toniator-preset");
        let (temporary, file) =
            create_temporary_file(&directory, &destination).expect("temporary file");
        drop(file);
        let mode = fs::metadata(&temporary)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        fs::remove_file(temporary).expect("remove temporary");
        fs::remove_dir(directory).expect("remove test directory");
    }
}
