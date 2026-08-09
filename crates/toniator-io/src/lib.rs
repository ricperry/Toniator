#![forbid(unsafe_code)]

//! The portable, versioned `.toniator` filesystem boundary.
//!
//! Archive JSON is deliberately represented by the private version-specific
//! DTOs below. Domain structures neither derive archive serde nor know about
//! ZIP/filesystem details.

use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPaint, ChannelPatternLayout,
    ChannelSourceMapping, ChannelState, ChannelTopology, ColorValue, DensityMetric2D, Document,
    DocumentId, HalftoneChannelModel, HalftoneChannelRole, MarkGeometryResponse,
    ModeledChannelState, PatternDefinition, PatternDefinitionId, PatternOutput, PatternStructure,
    SourceComponent, SourceMapping, SourceMappingComponent, SourcePlacement, SourceReference,
    SourceReferenceId, ValidationError,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const CONTAINER_VERSION: u32 = 1;
pub const DOCUMENT_SCHEMA_VERSION: u32 = 1;
pub const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_DOCUMENT_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_SOURCE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_UNCOMPRESSED_BYTES: u64 = 132 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbeddedSourceFormat {
    Png,
    Svg,
}

impl EmbeddedSourceFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
        }
    }

    pub fn from_extension(value: &str) -> Option<Self> {
        match value {
            "png" => Some(Self::Png),
            "svg" => Some(Self::Svg),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddedSource {
    id: SourceReferenceId,
    format: EmbeddedSourceFormat,
    bytes: Arc<[u8]>,
    display_name: Option<String>,
}

impl EmbeddedSource {
    pub fn new(
        id: SourceReferenceId,
        format: EmbeddedSourceFormat,
        bytes: impl Into<Arc<[u8]>>,
        display_name: Option<String>,
    ) -> Result<Self, SourceBundleError> {
        validate_source_id(&id)?;
        let bytes = bytes.into();
        if bytes.is_empty() || bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(SourceBundleError::new(
                "source.bytes",
                "source bytes exceed the v1 limit",
            ));
        }
        if display_name
            .as_ref()
            .is_some_and(|name| name.chars().any(char::is_control))
        {
            return Err(SourceBundleError::new(
                "source.display_name",
                "display name contains a control character",
            ));
        }
        Ok(Self {
            id,
            format,
            bytes,
            display_name,
        })
    }

    pub fn id(&self) -> &SourceReferenceId {
        &self.id
    }
    pub const fn format(&self) -> EmbeddedSourceFormat {
        self.format
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceBundle {
    entries: BTreeMap<SourceReferenceId, EmbeddedSource>,
}

impl SourceBundle {
    pub fn new(
        entries: impl IntoIterator<Item = EmbeddedSource>,
    ) -> Result<Self, SourceBundleError> {
        let mut mapped = BTreeMap::new();
        for entry in entries {
            if mapped.insert(entry.id.clone(), entry).is_some() {
                return Err(SourceBundleError::new(
                    "source.id",
                    "source IDs must be unique",
                ));
            }
        }
        Ok(Self { entries: mapped })
    }
    pub fn get(&self, id: &SourceReferenceId) -> Option<&EmbeddedSource> {
        self.entries.get(id)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn entries(&self) -> impl ExactSizeIterator<Item = &EmbeddedSource> {
        self.entries.values()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredVersions {
    container: u32,
    document: u32,
}
impl StoredVersions {
    pub const fn new(container: u32, document: u32) -> Self {
        Self {
            container,
            document,
        }
    }
    pub const fn container(&self) -> u32 {
        self.container
    }
    pub const fn document(&self) -> u32 {
        self.document
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrationReport {
    _private: (),
}
impl MigrationReport {
    pub const fn is_empty(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedDocument {
    document: Document,
    sources: SourceBundle,
    versions: StoredVersions,
    report: MigrationReport,
}
impl LoadedDocument {
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn sources(&self) -> &SourceBundle {
        &self.sources
    }
    pub const fn versions(&self) -> StoredVersions {
        self.versions
    }
    pub fn migration_report(&self) -> &MigrationReport {
        &self.report
    }
}

#[derive(Debug)]
pub enum LoadError {
    Filesystem { path: PathBuf, context: String },
    Archive { context: String },
    Json { context: String },
    Version { context: String },
    EntryTopology { context: String },
    Limits { context: String },
    Integrity { context: String },
    SourceDocumentMismatch { context: String },
    DomainValidation { context: String },
}

#[derive(Debug)]
pub enum SaveError {
    Filesystem { path: PathBuf, context: String },
    EntryTopology { context: String },
    Limits { context: String },
    SourceDocumentMismatch { context: String },
    DomainValidation { context: String },
    Archive { context: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceBundleError {
    path: &'static str,
    context: &'static str,
}
impl SourceBundleError {
    const fn new(path: &'static str, context: &'static str) -> Self {
        Self { path, context }
    }
    pub const fn path(&self) -> &'static str {
        self.path
    }
    pub const fn context(&self) -> &'static str {
        self.context
    }
}
impl std::fmt::Display for SourceBundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.context)
    }
}
impl std::error::Error for SourceBundleError {}

macro_rules! error_accessors {
    ($name:ident) => {
        impl $name {
            pub const fn path(&self) -> &'static str {
                match self {
                    Self::Filesystem { .. } => "filesystem",
                    Self::Archive { .. } => "archive",
                    Self::Json { .. } => "document.json",
                    Self::Version { .. } => "version",
                    Self::EntryTopology { .. } => "archive.entries",
                    Self::Limits { .. } => "archive.limits",
                    Self::Integrity { .. } => "source.integrity",
                    Self::SourceDocumentMismatch { .. } => "source.document",
                    Self::DomainValidation { .. } => "document.validation",
                }
            }
            pub fn context(&self) -> &str {
                match self {
                    Self::Filesystem { context, .. }
                    | Self::Archive { context }
                    | Self::Json { context }
                    | Self::Version { context }
                    | Self::EntryTopology { context }
                    | Self::Limits { context }
                    | Self::Integrity { context }
                    | Self::SourceDocumentMismatch { context }
                    | Self::DomainValidation { context } => context,
                }
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}: {}", self.path(), self.context())
            }
        }
        impl std::error::Error for $name {}
    };
}
error_accessors!(LoadError);

impl SaveError {
    pub const fn path(&self) -> &'static str {
        match self {
            Self::Filesystem { .. } => "filesystem",
            Self::EntryTopology { .. } => "archive.entries",
            Self::Limits { .. } => "archive.limits",
            Self::SourceDocumentMismatch { .. } => "source.document",
            Self::DomainValidation { .. } => "document.validation",
            Self::Archive { .. } => "archive",
        }
    }
    pub fn context(&self) -> &str {
        match self {
            Self::Filesystem { context, .. }
            | Self::EntryTopology { context }
            | Self::Limits { context }
            | Self::SourceDocumentMismatch { context }
            | Self::DomainValidation { context }
            | Self::Archive { context } => context,
        }
    }
}
impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path(), self.context())
    }
}
impl std::error::Error for SaveError {}

/// Loads through the immutable v1 dispatch pipeline.
pub fn load(path: &Path) -> Result<LoadedDocument, LoadError> {
    let metadata = fs::metadata(path).map_err(|error| LoadError::Filesystem {
        path: path.to_owned(),
        context: error.to_string(),
    })?;
    if metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(LoadError::Limits {
            context: "archive exceeds the 256 MiB v1 limit".into(),
        });
    }
    let file = File::open(path).map_err(|error| LoadError::Filesystem {
        path: path.to_owned(),
        context: error.to_string(),
    })?;
    let mut archive = ZipArchive::new(file).map_err(|error| LoadError::Archive {
        context: error.to_string(),
    })?;
    if !(2..=3).contains(&archive.len()) {
        return Err(LoadError::EntryTopology {
            context: "v1 archive must contain exactly two file entries; the only optional marker is an empty sources/ directory entry".into(),
        });
    }

    let mut names = HashSet::new();
    let mut uncompressed = 0_u64;
    let mut document_index = None;
    let mut source_index = None;
    let mut sources_marker = false;
    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|error| LoadError::Archive {
                context: error.to_string(),
            })?;
        let name = entry.name().to_owned();
        if !safe_archive_name(&name) {
            return Err(LoadError::EntryTopology {
                context: format!("unsafe archive entry name: {name:?}"),
            });
        }
        if !names.insert(name.clone()) {
            return Err(LoadError::EntryTopology {
                context: format!("duplicate archive entry: {name}"),
            });
        }
        if entry.encrypted() {
            return Err(LoadError::Archive {
                context: format!("encrypted entry is unsupported: {name}"),
            });
        }
        uncompressed = uncompressed
            .checked_add(entry.size())
            .ok_or_else(|| LoadError::Limits {
                context: "uncompressed size overflow".into(),
            })?;
        if uncompressed > MAX_UNCOMPRESSED_BYTES {
            return Err(LoadError::Limits {
                context: "total uncompressed data exceeds the 132 MiB v1 limit".into(),
            });
        }
        match name.as_str() {
            "sources/" => {
                if !entry.is_dir() || entry.size() != 0 {
                    return Err(LoadError::EntryTopology {
                        context: "sources/ must be an empty directory marker".into(),
                    });
                }
                if sources_marker {
                    return Err(LoadError::EntryTopology {
                        context: "duplicate sources/ directory marker".into(),
                    });
                }
                sources_marker = true;
            }
            "document.json" if entry.is_file() => {
                if document_index.replace(index).is_some() {
                    return Err(LoadError::EntryTopology {
                        context: "multiple document.json file entries".into(),
                    });
                }
                ensure_supported_file_compression(&name, entry.compression())?;
            }
            _ if name.starts_with("sources/") && entry.is_file() => {
                if source_index.replace(index).is_some() {
                    return Err(LoadError::EntryTopology {
                        context: "multiple source file entries".into(),
                    });
                }
                ensure_supported_file_compression(&name, entry.compression())?;
            }
            _ => {
                return Err(LoadError::EntryTopology {
                    context: format!("unexpected archive entry: {name}"),
                });
            }
        }
    }
    let document_index = document_index.ok_or_else(|| LoadError::EntryTopology {
        context: "missing document.json".into(),
    })?;
    let source_index = source_index.ok_or_else(|| LoadError::EntryTopology {
        context: "missing source entry".into(),
    })?;
    let document_bytes = read_limited(
        &mut archive,
        document_index,
        MAX_DOCUMENT_BYTES,
        "document.json",
    )?;
    let envelope: VersionEnvelope =
        serde_json::from_slice(&document_bytes).map_err(|error| LoadError::Json {
            context: error.to_string(),
        })?;
    if envelope.container_version != CONTAINER_VERSION {
        return Err(LoadError::Version {
            context: format!(
                "unsupported container version {}",
                envelope.container_version
            ),
        });
    }
    if envelope.document_schema_version != DOCUMENT_SCHEMA_VERSION {
        return Err(LoadError::Version {
            context: format!(
                "unsupported document schema version {}",
                envelope.document_schema_version
            ),
        });
    }
    let stored: StoredDocumentDtoV1 =
        serde_json::from_slice(&document_bytes).map_err(|error| LoadError::Json {
            context: error.to_string(),
        })?;
    let manifest = &stored.source;
    let source_id = dto_source_id(&manifest.id).map_err(domain_error)?;
    validate_source_id(&source_id).map_err(|error| LoadError::EntryTopology {
        context: error.to_string(),
    })?;
    let expected_entry =
        source_entry_name(&source_id, manifest.format.into()).map_err(|error| {
            LoadError::EntryTopology {
                context: error.context().into(),
            }
        })?;
    if manifest.entry_name != expected_entry {
        return Err(LoadError::EntryTopology {
            context: "manifest source entry name is not canonical".into(),
        });
    }
    let actual_name = archive
        .by_index(source_index)
        .map_err(|error| LoadError::Archive {
            context: error.to_string(),
        })?
        .name()
        .to_owned();
    if actual_name != manifest.entry_name {
        return Err(LoadError::EntryTopology {
            context: "manifest source entry does not match archive topology".into(),
        });
    }
    let source_bytes = read_limited(&mut archive, source_index, MAX_SOURCE_BYTES, &actual_name)?;
    if source_bytes.len() as u64 != manifest.byte_length {
        return Err(LoadError::Integrity {
            context: "source byte length does not match manifest".into(),
        });
    }
    if sha256_hex(&source_bytes) != manifest.sha256 {
        return Err(LoadError::Integrity {
            context: "source SHA-256 does not match manifest".into(),
        });
    }
    let source = EmbeddedSource::new(
        source_id.clone(),
        manifest.format.into(),
        source_bytes,
        manifest.display_name.clone(),
    )
    .map_err(|error| LoadError::EntryTopology {
        context: error.to_string(),
    })?;
    let sources = SourceBundle::new([source]).map_err(|error| LoadError::EntryTopology {
        context: error.to_string(),
    })?;
    let current = migrate_v1(stored)?;
    let document = current.document.into_domain().map_err(domain_error)?;
    match document.source() {
        SourceReference::Assigned(id) if id == &source_id => {}
        _ => {
            return Err(LoadError::SourceDocumentMismatch {
                context: "document source reference must match the single embedded source".into(),
            });
        }
    }
    Ok(LoadedDocument {
        document,
        sources,
        versions: StoredVersions::new(CONTAINER_VERSION, DOCUMENT_SCHEMA_VERSION),
        report: MigrationReport::default(),
    })
}

fn ensure_supported_file_compression(
    name: &str,
    method: CompressionMethod,
) -> Result<(), LoadError> {
    if matches!(
        method,
        CompressionMethod::Stored | CompressionMethod::Deflated
    ) {
        return Ok(());
    }
    Err(LoadError::Archive {
        context: format!(
            "unsupported compression method {method:?} for entry {name}; only Stored or Deflated file entries are accepted"
        ),
    })
}

/// Saves one fully source-backed current document using deterministic v1 bytes.
pub fn save(path: &Path, document: &Document, sources: &SourceBundle) -> Result<(), SaveError> {
    document.validate().map_err(save_domain_error)?;
    let source_id = match document.source() {
        SourceReference::Assigned(id) => id,
        SourceReference::Unassigned => {
            return Err(SaveError::SourceDocumentMismatch {
                context: "v1 saving requires an assigned document source".into(),
            });
        }
    };
    if sources.len() != 1 {
        return Err(SaveError::SourceDocumentMismatch {
            context: "v1 saving requires exactly one embedded source".into(),
        });
    }
    let source = sources
        .get(source_id)
        .ok_or_else(|| SaveError::SourceDocumentMismatch {
            context: "source bundle does not contain the document source".into(),
        })?;
    let entry_name = source_entry_name(source.id(), source.format()).map_err(|error| {
        SaveError::EntryTopology {
            context: error.context().into(),
        }
    })?;
    let dto = StoredDocumentDtoV1::from_domain(document, source, entry_name.clone())?;
    let mut document_json = serde_json::to_vec(&dto).map_err(|error| SaveError::Archive {
        context: error.to_string(),
    })?;
    document_json.push(b'\n');
    if document_json.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(SaveError::Limits {
            context: "document.json exceeds the 4 MiB v1 limit".into(),
        });
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let (temporary_path, file) = create_temp(parent, path)?;
    let result = write_archive(file, &document_json, &entry_name, source);
    match result.and_then(|()| {
        fs::rename(&temporary_path, path).map_err(|error| SaveError::Filesystem {
            path: path.to_owned(),
            context: error.to_string(),
        })
    }) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary_path);
            Err(error)
        }
    }
}

fn write_archive(
    file: File,
    document_json: &[u8],
    entry_name: &str,
    source: &EmbeddedSource,
) -> Result<(), SaveError> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o100644);
    let mut writer = ZipWriter::new(file);
    writer
        .start_file("document.json", options)
        .map_err(|error| SaveError::Archive {
            context: error.to_string(),
        })?;
    writer
        .write_all(document_json)
        .map_err(|error| SaveError::Archive {
            context: error.to_string(),
        })?;
    writer
        .start_file(entry_name, options)
        .map_err(|error| SaveError::Archive {
            context: error.to_string(),
        })?;
    writer
        .write_all(source.bytes())
        .map_err(|error| SaveError::Archive {
            context: error.to_string(),
        })?;
    let file = writer.finish().map_err(|error| SaveError::Archive {
        context: error.to_string(),
    })?;
    file.sync_all().map_err(|error| SaveError::Filesystem {
        path: PathBuf::new(),
        context: error.to_string(),
    })
}

fn create_temp(parent: &Path, destination: &Path) -> Result<(PathBuf, File), SaveError> {
    for attempt in 0..128_u32 {
        let candidate = parent.join(format!(
            ".{}.toniator-{}-{}.tmp",
            destination
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("document"),
            std::process::id(),
            attempt
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(SaveError::Filesystem {
                    path: candidate,
                    context: error.to_string(),
                });
            }
        }
    }
    Err(SaveError::Filesystem {
        path: destination.to_owned(),
        context: "could not allocate a unique temporary file".into(),
    })
}

fn read_limited(
    archive: &mut ZipArchive<File>,
    index: usize,
    limit: u64,
    name: &str,
) -> Result<Vec<u8>, LoadError> {
    let mut entry = archive
        .by_index(index)
        .map_err(|error| LoadError::Archive {
            context: error.to_string(),
        })?;
    if entry.size() > limit {
        return Err(LoadError::Limits {
            context: format!("{name} exceeds its v1 size limit"),
        });
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| LoadError::Archive {
            context: error.to_string(),
        })?;
    if bytes.len() as u64 > limit {
        return Err(LoadError::Limits {
            context: format!("{name} exceeds its v1 size limit"),
        });
    }
    Ok(bytes)
}

fn safe_archive_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains(':')
        && !name.contains('\\')
        && !name.contains("..")
        && !name.chars().any(char::is_control)
}
fn validate_source_id(id: &SourceReferenceId) -> Result<(), SourceBundleError> {
    let value = id.as_str();
    if value == "."
        || value == ".."
        || value.contains(':')
        || value.contains("..")
        || value.chars().any(char::is_control)
    {
        return Err(SourceBundleError::new(
            "source.id",
            "source ID is not a safe archive component",
        ));
    }
    Ok(())
}
fn source_entry_name(
    id: &SourceReferenceId,
    format: EmbeddedSourceFormat,
) -> Result<String, SourceBundleError> {
    validate_source_id(id)?;
    Ok(format!("sources/{}.{}", id.as_str(), format.extension()))
}
fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn dto_source_id(value: &str) -> Result<SourceReferenceId, ValidationError> {
    SourceReferenceId::new(value.to_owned())
}
fn domain_error(error: ValidationError) -> LoadError {
    LoadError::DomainValidation {
        context: error.to_string(),
    }
}
fn save_domain_error(error: ValidationError) -> SaveError {
    SaveError::DomainValidation {
        context: error.to_string(),
    }
}

fn migrate_v1(stored: StoredDocumentDtoV1) -> Result<CurrentDocumentDto, LoadError> {
    Ok(CurrentDocumentDto {
        document: stored.document,
    })
}

#[derive(Serialize, Deserialize)]
struct StoredDocumentDtoV1 {
    container_version: u32,
    document_schema_version: u32,
    document: DocumentDtoV1,
    source: SourceManifestDtoV1,
}
#[derive(Deserialize)]
struct VersionEnvelope {
    container_version: u32,
    document_schema_version: u32,
}
#[derive(Serialize, Deserialize)]
struct SourceManifestDtoV1 {
    id: String,
    entry_name: String,
    format: EmbeddedSourceFormatDto,
    byte_length: u64,
    sha256: String,
    display_name: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct CurrentDocumentDto {
    document: DocumentDtoV1,
}

impl StoredDocumentDtoV1 {
    fn from_domain(
        document: &Document,
        source: &EmbeddedSource,
        entry_name: String,
    ) -> Result<Self, SaveError> {
        Ok(Self {
            container_version: CONTAINER_VERSION,
            document_schema_version: DOCUMENT_SCHEMA_VERSION,
            document: DocumentDtoV1::from_domain(document)?,
            source: SourceManifestDtoV1 {
                id: source.id().as_str().into(),
                entry_name,
                format: source.format().into(),
                byte_length: source.bytes().len() as u64,
                sha256: sha256_hex(source.bytes()),
                display_name: source.display_name().map(str::to_owned),
            },
        })
    }
}

#[derive(Serialize, Deserialize)]
struct DocumentDtoV1 {
    id: u64,
    canvas: CanvasDto,
    source_reference_id: String,
    pattern_definitions: Vec<PatternDefinitionDto>,
    channel_configuration: ChannelConfigurationDto,
}
#[derive(Serialize, Deserialize)]
struct CanvasDto {
    width: f64,
    height: f64,
}
#[derive(Serialize, Deserialize)]
struct PatternDefinitionDto {
    id: u64,
    name: String,
    structure: PatternStructureDto,
    output: PatternOutputDto,
    guard_steps: u32,
    maximum_support_radius: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ChannelConfigurationDto {
    Legacy {
        channels: Vec<LegacyChannelDto>,
    },
    Topology {
        model: HalftoneChannelModelDto,
        channels: Vec<ModeledChannelDto>,
    },
}
#[derive(Serialize, Deserialize)]
struct LegacyChannelDto {
    id: u64,
    pattern_definition_id: u64,
    layout: LayoutDto,
    appearance: AppearanceDto,
    mark_geometry_response: MarkResponseDto,
    source_mapping: LegacySourceMappingDto,
}
#[derive(Serialize, Deserialize)]
struct ModeledChannelDto {
    role: HalftoneChannelRoleDto,
    id: u64,
    pattern_definition_id: u64,
    layout: LayoutDto,
    mark_geometry_response: MarkResponseDto,
    mapping: SourceMappingDto,
    paint: PaintDto,
    visible: bool,
    opacity: f64,
}
#[derive(Serialize, Deserialize)]
struct LayoutDto {
    density: DensityDto,
    rotation_degrees: f64,
    translation_x: f64,
    translation_y: f64,
}
#[derive(Serialize, Deserialize)]
struct DensityDto {
    across_x: f64,
    across_y: f64,
    aspect_locked: bool,
}
#[derive(Serialize, Deserialize)]
struct AppearanceDto {
    visible: bool,
    color: ColorDto,
    opacity: f64,
}
#[derive(Serialize, Deserialize)]
struct MarkResponseDto {
    minimum_size: f64,
    maximum_size: f64,
}
#[derive(Serialize, Deserialize)]
struct LegacySourceMappingDto {
    component: SourceComponentDto,
    placement: SourcePlacementDto,
}
#[derive(Serialize, Deserialize)]
struct SourceMappingDto {
    component: SourceMappingComponentDto,
    placement: SourcePlacementDto,
    inverted: bool,
    gain: f64,
    bias: f64,
}
#[derive(Serialize, Deserialize)]
struct ColorDto {
    red: f64,
    green: f64,
    blue: f64,
    alpha: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PaintDto {
    Solid { color: ColorDto },
    SampledSource,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EmbeddedSourceFormatDto {
    Png,
    Svg,
}
impl From<EmbeddedSourceFormat> for EmbeddedSourceFormatDto {
    fn from(value: EmbeddedSourceFormat) -> Self {
        match value {
            EmbeddedSourceFormat::Png => Self::Png,
            EmbeddedSourceFormat::Svg => Self::Svg,
        }
    }
}
impl From<EmbeddedSourceFormatDto> for EmbeddedSourceFormat {
    fn from(value: EmbeddedSourceFormatDto) -> Self {
        match value {
            EmbeddedSourceFormatDto::Png => Self::Png,
            EmbeddedSourceFormatDto::Svg => Self::Svg,
        }
    }
}
macro_rules! dto_enum { ($dto:ident, $domain:ident { $($variant:ident),+ $(,)? }) => {
    #[derive(Clone, Copy, Serialize, Deserialize)] #[serde(rename_all = "snake_case")] enum $dto { $($variant),+ }
    impl From<$domain> for $dto { fn from(value: $domain) -> Self { match value { $($domain::$variant => Self::$variant),+ } } }
    impl From<$dto> for $domain { fn from(value: $dto) -> Self { match value { $($dto::$variant => Self::$variant),+ } } }
}; }
dto_enum!(
    PatternStructureDto,
    PatternStructure {
        StraightGrid,
        Unsupported
    }
);
dto_enum!(
    PatternOutputDto,
    PatternOutput {
        CircularMarks,
        Unsupported
    }
);
dto_enum!(
    HalftoneChannelModelDto,
    HalftoneChannelModel {
        Rgb,
        Cmyk,
        SourceColorAlpha
    }
);
dto_enum!(
    HalftoneChannelRoleDto,
    HalftoneChannelRole {
        Red,
        Green,
        Blue,
        Cyan,
        Magenta,
        Yellow,
        Black,
        SourceColor
    }
);
dto_enum!(SourceComponentDto, SourceComponent { Luminance, Alpha });
dto_enum!(SourcePlacementDto, SourcePlacement { StretchToCanvas });
dto_enum!(
    SourceMappingComponentDto,
    SourceMappingComponent {
        Red,
        Green,
        Blue,
        Cyan,
        Magenta,
        Yellow,
        Black,
        Alpha,
        Luminance
    }
);

impl DocumentDtoV1 {
    fn from_domain(document: &Document) -> Result<Self, SaveError> {
        let source_reference_id = match document.source() {
            SourceReference::Assigned(id) => id.as_str().to_owned(),
            SourceReference::Unassigned => {
                return Err(SaveError::SourceDocumentMismatch {
                    context: "v1 saving requires an assigned document source".into(),
                });
            }
        };
        let channel_configuration = match (
            document.channels(),
            document.channel_model(),
            document.channel_topology(),
        ) {
            (Some(channels), None, None) => ChannelConfigurationDto::Legacy {
                channels: channels.iter().map(LegacyChannelDto::from_domain).collect(),
            },
            (None, Some(model), Some(topology)) => ChannelConfigurationDto::Topology {
                model: model.into(),
                channels: topology
                    .channels()
                    .iter()
                    .map(ModeledChannelDto::from_domain)
                    .collect(),
            },
            _ => {
                return Err(SaveError::DomainValidation {
                    context: "document has an incoherent channel configuration".into(),
                });
            }
        };
        Ok(Self {
            id: document.id().0,
            canvas: CanvasDto {
                width: document.canvas().width,
                height: document.canvas().height,
            },
            source_reference_id,
            pattern_definitions: document
                .pattern_definitions()
                .iter()
                .map(PatternDefinitionDto::from_domain)
                .collect(),
            channel_configuration,
        })
    }
    fn into_domain(self) -> Result<Document, ValidationError> {
        let source = SourceReference::Assigned(dto_source_id(&self.source_reference_id)?);
        let definitions = self
            .pattern_definitions
            .into_iter()
            .map(PatternDefinitionDto::into_domain)
            .collect();
        match self.channel_configuration {
            ChannelConfigurationDto::Legacy { channels } => Document::with_source(
                DocumentId(self.id),
                CanvasSpec {
                    width: self.canvas.width,
                    height: self.canvas.height,
                },
                source,
                definitions,
                channels
                    .into_iter()
                    .map(LegacyChannelDto::into_domain)
                    .collect(),
            ),
            ChannelConfigurationDto::Topology { model, channels } => {
                Document::with_source_and_topology(
                    DocumentId(self.id),
                    CanvasSpec {
                        width: self.canvas.width,
                        height: self.canvas.height,
                    },
                    source,
                    definitions,
                    model.into(),
                    ChannelTopology::new(
                        channels
                            .into_iter()
                            .map(ModeledChannelDto::into_domain)
                            .collect(),
                    ),
                )
            }
        }
    }
}
impl PatternDefinitionDto {
    fn from_domain(value: &PatternDefinition) -> Self {
        Self {
            id: value.id.0,
            name: value.name.clone(),
            structure: value.structure.into(),
            output: value.output.into(),
            guard_steps: value.guard_steps,
            maximum_support_radius: value.maximum_support_radius,
        }
    }
    fn into_domain(self) -> PatternDefinition {
        PatternDefinition {
            id: PatternDefinitionId(self.id),
            name: self.name,
            structure: self.structure.into(),
            output: self.output.into(),
            guard_steps: self.guard_steps,
            maximum_support_radius: self.maximum_support_radius,
        }
    }
}
impl LegacyChannelDto {
    fn from_domain(value: &ChannelState) -> Self {
        Self {
            id: value.id.0,
            pattern_definition_id: value.pattern_definition_id.0,
            layout: LayoutDto::from_domain(&value.layout),
            appearance: AppearanceDto::from_domain(&value.appearance),
            mark_geometry_response: MarkResponseDto::from_domain(&value.mark_geometry_response),
            source_mapping: LegacySourceMappingDto::from_domain(value.source_mapping),
        }
    }
    fn into_domain(self) -> ChannelState {
        ChannelState {
            id: ChannelId(self.id),
            pattern_definition_id: PatternDefinitionId(self.pattern_definition_id),
            layout: self.layout.into_domain(),
            appearance: self.appearance.into_domain(),
            mark_geometry_response: self.mark_geometry_response.into_domain(),
            source_mapping: self.source_mapping.into_domain(),
        }
    }
}
impl ModeledChannelDto {
    fn from_domain(value: &ModeledChannelState) -> Self {
        Self {
            role: value.role.into(),
            id: value.id.0,
            pattern_definition_id: value.pattern_definition_id.0,
            layout: LayoutDto::from_domain(&value.layout),
            mark_geometry_response: MarkResponseDto::from_domain(&value.mark_geometry_response),
            mapping: SourceMappingDto::from_domain(value.mapping),
            paint: PaintDto::from_domain(&value.paint),
            visible: value.visible,
            opacity: value.opacity,
        }
    }
    fn into_domain(self) -> ModeledChannelState {
        ModeledChannelState {
            role: self.role.into(),
            id: ChannelId(self.id),
            pattern_definition_id: PatternDefinitionId(self.pattern_definition_id),
            layout: self.layout.into_domain(),
            mark_geometry_response: self.mark_geometry_response.into_domain(),
            mapping: self.mapping.into_domain(),
            paint: self.paint.into_domain(),
            visible: self.visible,
            opacity: self.opacity,
        }
    }
}
impl LayoutDto {
    fn from_domain(value: &ChannelPatternLayout) -> Self {
        Self {
            density: DensityDto {
                across_x: value.density.across_x,
                across_y: value.density.across_y,
                aspect_locked: value.density.aspect_locked,
            },
            rotation_degrees: value.rotation_degrees,
            translation_x: value.translation_x,
            translation_y: value.translation_y,
        }
    }
    fn into_domain(self) -> ChannelPatternLayout {
        ChannelPatternLayout {
            density: DensityMetric2D {
                across_x: self.density.across_x,
                across_y: self.density.across_y,
                aspect_locked: self.density.aspect_locked,
            },
            rotation_degrees: self.rotation_degrees,
            translation_x: self.translation_x,
            translation_y: self.translation_y,
        }
    }
}
impl AppearanceDto {
    fn from_domain(value: &ChannelAppearance) -> Self {
        Self {
            visible: value.visible,
            color: ColorDto::from_domain(&value.color),
            opacity: value.opacity,
        }
    }
    fn into_domain(self) -> ChannelAppearance {
        ChannelAppearance {
            visible: self.visible,
            color: self.color.into_domain(),
            opacity: self.opacity,
        }
    }
}
impl MarkResponseDto {
    fn from_domain(value: &MarkGeometryResponse) -> Self {
        Self {
            minimum_size: value.minimum_size,
            maximum_size: value.maximum_size,
        }
    }
    fn into_domain(self) -> MarkGeometryResponse {
        MarkGeometryResponse {
            minimum_size: self.minimum_size,
            maximum_size: self.maximum_size,
        }
    }
}
impl LegacySourceMappingDto {
    fn from_domain(value: ChannelSourceMapping) -> Self {
        Self {
            component: value.component.into(),
            placement: value.placement.into(),
        }
    }
    fn into_domain(self) -> ChannelSourceMapping {
        ChannelSourceMapping {
            component: self.component.into(),
            placement: self.placement.into(),
        }
    }
}
impl SourceMappingDto {
    fn from_domain(value: SourceMapping) -> Self {
        Self {
            component: value.component.into(),
            placement: value.placement.into(),
            inverted: value.inverted,
            gain: value.gain,
            bias: value.bias,
        }
    }
    fn into_domain(self) -> SourceMapping {
        SourceMapping {
            component: self.component.into(),
            placement: self.placement.into(),
            inverted: self.inverted,
            gain: self.gain,
            bias: self.bias,
        }
    }
}
impl ColorDto {
    fn from_domain(value: &ColorValue) -> Self {
        Self {
            red: value.red,
            green: value.green,
            blue: value.blue,
            alpha: value.alpha,
        }
    }
    fn into_domain(self) -> ColorValue {
        ColorValue {
            red: self.red,
            green: self.green,
            blue: self.blue,
            alpha: self.alpha,
        }
    }
}
impl PaintDto {
    fn from_domain(value: &ChannelPaint) -> Self {
        match value {
            ChannelPaint::Solid(color) => Self::Solid {
                color: ColorDto::from_domain(color),
            },
            ChannelPaint::SampledSource => Self::SampledSource,
        }
    }
    fn into_domain(self) -> ChannelPaint {
        match self {
            Self::Solid { color } => ChannelPaint::Solid(color.into_domain()),
            Self::SampledSource => ChannelPaint::SampledSource,
        }
    }
}
