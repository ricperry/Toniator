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
    ArtworkWeightResponse, CanvasSpec, ChannelAppearance, ChannelId, ChannelPaint,
    ChannelPatternLayout, ChannelSourceMapping, ChannelState, ChannelTopology, ColorValue,
    CoveragePolicy, DensityMetric2D, Document, DocumentId, GuideDimensionId, HalftoneChannelModel,
    HalftoneChannelRole, MarkGeometryResponse, MarkOrientation, MarkPrototype, ModeledChannelState,
    PatternDefinition, PatternDefinitionId, PatternMechanismId, PatternOutputLayerId,
    RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy, SourceComponent,
    SourceMapping, SourceMappingComponent, SourcePlacement, SourceReference, SourceReferenceId,
    StraightGuideDimension, StraightGuideRepetition, ValidationError, VisibleMarkSizingPolicy,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const CONTAINER_VERSION: u32 = 1;
pub const DOCUMENT_SCHEMA_VERSION: u32 = 2;
const DOCUMENT_SCHEMA_VERSION_V1: u32 = 1;
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
    migrated_v1: bool,
    generated_definitions: Vec<MigrationDefinitionReport>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationDefinitionReport {
    pub definition_id: PatternDefinitionId,
    pub mechanism_ids: Vec<PatternMechanismId>,
    pub output_layer_ids: Vec<PatternOutputLayerId>,
}
impl MigrationReport {
    pub const fn is_empty(&self) -> bool {
        !self.migrated_v1
    }
    pub fn generated_definition_ids(&self) -> Vec<PatternDefinitionId> {
        // Kept as a compact convenience projection for lifecycle diagnostics.
        // Detailed generated addresses are available below.
        // The report remains lifecycle data and is never serialized.
        self.generated_definitions
            .iter()
            .map(|definition| definition.definition_id)
            .collect()
    }
    pub fn generated_definitions(&self) -> &[MigrationDefinitionReport] {
        &self.generated_definitions
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
    let (current, manifest, report) = match envelope.document_schema_version {
        DOCUMENT_SCHEMA_VERSION_V1 => {
            // This private parser is intentionally immutable Stage 12 input.
            let stored: StoredDocumentDtoV1 =
                serde_json::from_slice(&document_bytes).map_err(|error| LoadError::Json {
                    context: error.to_string(),
                })?;
            let manifest = stored.source.clone();
            let current = migrate_v1(stored)?;
            let generated_definitions = current
                .document
                .pattern_definitions
                .iter()
                .map(|definition| MigrationDefinitionReport {
                    definition_id: PatternDefinitionId(definition.id),
                    mechanism_ids: definition
                        .mechanisms
                        .iter()
                        .map(|mechanism| match mechanism {
                            PatternMechanismDtoV2::StraightGuides { id }
                            | PatternMechanismDtoV2::GuideIntersections { id, .. }
                            | PatternMechanismDtoV2::StraightGuideDimensions { id, .. }
                            | PatternMechanismDtoV2::SelectedGuideIntersections { id, .. }
                            | PatternMechanismDtoV2::AlongGuideSites { id, .. }
                            | PatternMechanismDtoV2::RandomSiteProcess { id, .. }
                            | PatternMechanismDtoV2::SiteDensityModulation { id, .. }
                            | PatternMechanismDtoV2::SiteExclusion { id, .. }
                            | PatternMechanismDtoV2::RandomSiteProduct { id, .. } => {
                                PatternMechanismId(*id)
                            }
                        })
                        .collect(),
                    output_layer_ids: definition
                        .output_layers
                        .iter()
                        .map(|layer| match layer {
                            PatternOutputLayerDtoV2::CircularMarks { id, .. }
                            | PatternOutputLayerDtoV2::MarkPrototype { id, .. } => {
                                PatternOutputLayerId(*id)
                            }
                        })
                        .collect(),
                })
                .collect();
            (
                current,
                manifest,
                MigrationReport {
                    migrated_v1: true,
                    generated_definitions,
                },
            )
        }
        DOCUMENT_SCHEMA_VERSION => {
            let stored: StoredDocumentDtoV2 =
                serde_json::from_slice(&document_bytes).map_err(|error| LoadError::Json {
                    context: error.to_string(),
                })?;
            (
                CurrentDocumentDto {
                    document: stored.document,
                },
                stored.source,
                MigrationReport::default(),
            )
        }
        value => {
            return Err(LoadError::Version {
                context: format!("unsupported document schema version {value}"),
            });
        }
    };
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
        versions: StoredVersions::new(CONTAINER_VERSION, envelope.document_schema_version),
        report,
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

/// Saves one fully source-backed current document using deterministic v2 JSON
/// inside the immutable v1 ZIP container layout.
pub fn save(path: &Path, document: &Document, sources: &SourceBundle) -> Result<(), SaveError> {
    document.validate().map_err(save_domain_error)?;
    let source_id = match document.source() {
        SourceReference::Assigned(id) => id,
        SourceReference::Unassigned => {
            return Err(SaveError::SourceDocumentMismatch {
                context: "v2 saving requires an assigned document source".into(),
            });
        }
    };
    if sources.len() != 1 {
        return Err(SaveError::SourceDocumentMismatch {
            context: "v2 saving requires exactly one embedded source".into(),
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
    let dto = StoredDocumentDtoV2::from_domain(document, source, entry_name.clone())?;
    let mut document_json = serde_json::to_vec(&dto).map_err(|error| SaveError::Archive {
        context: error.to_string(),
    })?;
    document_json.push(b'\n');
    if document_json.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(SaveError::Limits {
            context: "document.json exceeds the 4 MiB container limit".into(),
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
    // The envelope was dispatched before this immutable parser; retaining these
    // reads keeps its complete v1 DTO interpretation explicit.
    let _container_version = stored.container_version;
    let _document_schema_version = stored.document_schema_version;
    Ok(CurrentDocumentDto {
        document: stored.document.migrate()?,
    })
}

#[derive(Deserialize)]
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
#[derive(Clone, Serialize, Deserialize)]
struct SourceManifestDtoV1 {
    id: String,
    entry_name: String,
    format: EmbeddedSourceFormatDto,
    byte_length: u64,
    sha256: String,
    display_name: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct StoredDocumentDtoV2 {
    container_version: u32,
    document_schema_version: u32,
    document: DocumentDtoV2,
    source: SourceManifestDtoV1,
}
#[derive(Serialize, Deserialize)]
struct CurrentDocumentDto {
    document: DocumentDtoV2,
}

impl StoredDocumentDtoV2 {
    fn from_domain(
        document: &Document,
        source: &EmbeddedSource,
        entry_name: String,
    ) -> Result<Self, SaveError> {
        Ok(Self {
            container_version: CONTAINER_VERSION,
            document_schema_version: DOCUMENT_SCHEMA_VERSION,
            document: DocumentDtoV2::from_domain(document)?,
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
struct DocumentDtoV2 {
    id: u64,
    canvas: CanvasDto,
    source_reference_id: String,
    pattern_definitions: Vec<PatternDefinitionDtoV2>,
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
struct PatternDefinitionDtoV2 {
    id: u64,
    name: String,
    family: PatternFamilyDtoV2,
    mechanisms: Vec<PatternMechanismDtoV2>,
    output_layers: Vec<PatternOutputLayerDtoV2>,
    modulation: PatternModulationDtoV2,
    coverage: CoverageDtoV2,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PatternFamilyDtoV2 {
    GuideIntersections {
        guide_mechanism_id: u64,
        site_mechanism_id: u64,
    },
    RandomSites {
        base_site_process_id: u64,
        density_modulation_id: u64,
        exclusion_id: u64,
        site_product_id: u64,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PatternMechanismDtoV2 {
    StraightGuides {
        id: u64,
    },
    GuideIntersections {
        id: u64,
        guide_mechanism_id: u64,
    },
    StraightGuideDimensions {
        id: u64,
        dimensions: Vec<StraightGuideDimensionDtoV2>,
    },
    SelectedGuideIntersections {
        id: u64,
        guide_mechanism_id: u64,
        dimensions: Vec<u64>,
        merge_epsilon: f64,
    },
    AlongGuideSites {
        id: u64,
        guide_mechanism_id: u64,
        dimensions: Vec<u64>,
        interval_multiplier: f64,
        phase: f64,
    },
    RandomSiteProcess {
        id: u64,
        character: RandomSiteCharacterDtoV2,
        seed: u32,
    },
    SiteDensityModulation {
        id: u64,
        base_site_process_id: u64,
        modulation: SiteDensityModulationDtoV2,
    },
    SiteExclusion {
        id: u64,
        density_modulation_id: u64,
        policy: SiteExclusionPolicyDtoV2,
    },
    RandomSiteProduct {
        id: u64,
        exclusion_id: u64,
        maximum_attempts: u32,
        #[serde(default = "default_random_site_neighbor_checks")]
        maximum_neighbor_checks: u32,
    },
}

const fn default_random_site_neighbor_checks() -> u32 {
    16_000_000
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RandomSiteCharacterDtoV2 {
    RawUniform,
    Even {
        minimum_center_distance: f64,
    },
    Clustered {
        cluster_density: f64,
        cluster_spread: f64,
        cluster_strength: f64,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SiteDensityModulationDtoV2 {
    Uniform,
    ArtworkWeighted {
        mapping: SourceMappingDto,
        strength: f64,
        response: ArtworkWeightResponseDtoV2,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ArtworkWeightResponseDtoV2 {
    Linear,
    Smoothstep,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SiteExclusionPolicyDtoV2 {
    None,
    MinimumCenterDistance {
        minimum: f64,
    },
    VisibleMarkMargin {
        margin: f64,
        sizing: VisibleMarkSizingPolicyDtoV2,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VisibleMarkSizingPolicyDtoV2 {
    MaximumSupportRadius,
}
#[derive(Serialize, Deserialize)]
struct StraightGuideDimensionDtoV2 {
    id: u64,
    baseline_angle_degrees: f64,
    phase: f64,
    spacing_multiplier: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PatternOutputLayerDtoV2 {
    CircularMarks {
        id: u64,
        site_mechanism_id: u64,
    },
    MarkPrototype {
        id: u64,
        site_mechanism_id: u64,
        prototype: MarkPrototypeDtoV2,
        orientation: MarkOrientationDtoV2,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MarkPrototypeDtoV2 {
    Circle,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MarkOrientationDtoV2 {
    Fixed,
    GuideTangent { dimension_id: u64 },
    GuideNormal { dimension_id: u64 },
}
#[derive(Serialize, Deserialize)]
struct PatternModulationDtoV2 {}
#[derive(Serialize, Deserialize)]
struct CoverageDtoV2 {
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
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PatternStructureDto {
    StraightGrid,
    Unsupported,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PatternOutputDto {
    CircularMarks,
    Unsupported,
}
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
    fn migrate(self) -> Result<DocumentDtoV2, LoadError> {
        let mut definitions = Vec::with_capacity(self.pattern_definitions.len());
        for definition in self.pattern_definitions {
            if !matches!(definition.structure, PatternStructureDto::StraightGrid)
                || !matches!(definition.output, PatternOutputDto::CircularMarks)
            {
                return Err(LoadError::DomainValidation {
                    context: "v1 definition does not map to the supported typed v2 mechanisms"
                        .into(),
                });
            }
            let guide_mechanism_id = definition
                .id
                .checked_mul(2)
                .and_then(|value| value.checked_sub(1))
                .ok_or_else(|| LoadError::DomainValidation {
                    context: "v1 definition ID cannot allocate deterministic typed mechanism IDs"
                        .into(),
                })?;
            let site_mechanism_id =
                definition
                    .id
                    .checked_mul(2)
                    .ok_or_else(|| LoadError::DomainValidation {
                        context:
                            "v1 definition ID cannot allocate deterministic typed mechanism IDs"
                                .into(),
                    })?;
            definitions.push(PatternDefinitionDtoV2 {
                id: definition.id,
                name: definition.name,
                family: PatternFamilyDtoV2::GuideIntersections {
                    guide_mechanism_id,
                    site_mechanism_id,
                },
                mechanisms: vec![
                    PatternMechanismDtoV2::StraightGuides {
                        id: guide_mechanism_id,
                    },
                    PatternMechanismDtoV2::GuideIntersections {
                        id: site_mechanism_id,
                        guide_mechanism_id,
                    },
                ],
                output_layers: vec![PatternOutputLayerDtoV2::CircularMarks {
                    id: definition.id,
                    site_mechanism_id,
                }],
                modulation: PatternModulationDtoV2 {},
                coverage: CoverageDtoV2 {
                    guard_steps: definition.guard_steps,
                    maximum_support_radius: definition.maximum_support_radius,
                },
            });
        }
        Ok(DocumentDtoV2 {
            id: self.id,
            canvas: self.canvas,
            source_reference_id: self.source_reference_id,
            pattern_definitions: definitions,
            channel_configuration: self.channel_configuration,
        })
    }
}

impl DocumentDtoV2 {
    fn from_domain(document: &Document) -> Result<Self, SaveError> {
        let source_reference_id = match document.source() {
            SourceReference::Assigned(id) => id.as_str().to_owned(),
            SourceReference::Unassigned => {
                return Err(SaveError::SourceDocumentMismatch {
                    context: "v2 saving requires an assigned document source".into(),
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
                .map(PatternDefinitionDtoV2::from_domain)
                .collect(),
            channel_configuration,
        })
    }
    fn into_domain(self) -> Result<Document, ValidationError> {
        let source = SourceReference::Assigned(dto_source_id(&self.source_reference_id)?);
        let definitions = self
            .pattern_definitions
            .into_iter()
            .map(PatternDefinitionDtoV2::into_domain)
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
impl PatternDefinitionDtoV2 {
    fn from_domain(value: &PatternDefinition) -> Self {
        Self {
            id: value.id.0,
            name: value.name.clone(),
            family: PatternFamilyDtoV2::from_domain(&value.family),
            mechanisms: value
                .mechanisms
                .iter()
                .map(PatternMechanismDtoV2::from_domain)
                .collect(),
            output_layers: value
                .output_layers
                .iter()
                .map(PatternOutputLayerDtoV2::from_domain)
                .collect(),
            modulation: PatternModulationDtoV2 {},
            coverage: CoverageDtoV2 {
                guard_steps: value.coverage.guard_steps,
                maximum_support_radius: value.coverage.maximum_support_radius,
            },
        }
    }
    fn into_domain(self) -> PatternDefinition {
        PatternDefinition {
            id: PatternDefinitionId(self.id),
            name: self.name,
            family: self.family.into_domain(),
            mechanisms: self
                .mechanisms
                .into_iter()
                .map(PatternMechanismDtoV2::into_domain)
                .collect(),
            output_layers: self
                .output_layers
                .into_iter()
                .map(PatternOutputLayerDtoV2::into_domain)
                .collect(),
            modulation: toniator_domain::PatternModulation,
            coverage: CoveragePolicy {
                guard_steps: self.coverage.guard_steps,
                maximum_support_radius: self.coverage.maximum_support_radius,
            },
        }
    }
}
impl PatternFamilyDtoV2 {
    fn from_domain(value: &toniator_domain::PatternFamily) -> Self {
        match value {
            toniator_domain::PatternFamily::GuideIntersections {
                guide_mechanism_id,
                site_mechanism_id,
            } => Self::GuideIntersections {
                guide_mechanism_id: guide_mechanism_id.0,
                site_mechanism_id: site_mechanism_id.0,
            },
            toniator_domain::PatternFamily::RandomSites {
                base_site_process_id,
                density_modulation_id,
                exclusion_id,
                site_product_id,
            } => Self::RandomSites {
                base_site_process_id: base_site_process_id.0,
                density_modulation_id: density_modulation_id.0,
                exclusion_id: exclusion_id.0,
                site_product_id: site_product_id.0,
            },
        }
    }
    fn into_domain(self) -> toniator_domain::PatternFamily {
        match self {
            Self::GuideIntersections {
                guide_mechanism_id,
                site_mechanism_id,
            } => toniator_domain::PatternFamily::GuideIntersections {
                guide_mechanism_id: PatternMechanismId(guide_mechanism_id),
                site_mechanism_id: PatternMechanismId(site_mechanism_id),
            },
            Self::RandomSites {
                base_site_process_id,
                density_modulation_id,
                exclusion_id,
                site_product_id,
            } => toniator_domain::PatternFamily::RandomSites {
                base_site_process_id: PatternMechanismId(base_site_process_id),
                density_modulation_id: PatternMechanismId(density_modulation_id),
                exclusion_id: PatternMechanismId(exclusion_id),
                site_product_id: PatternMechanismId(site_product_id),
            },
        }
    }
}
impl PatternMechanismDtoV2 {
    fn from_domain(value: &toniator_domain::PatternMechanism) -> Self {
        match value {
            toniator_domain::PatternMechanism::StraightGuides { id } => {
                Self::StraightGuides { id: id.0 }
            }
            toniator_domain::PatternMechanism::GuideIntersections {
                id,
                guide_mechanism_id,
            } => Self::GuideIntersections {
                id: id.0,
                guide_mechanism_id: guide_mechanism_id.0,
            },
            toniator_domain::PatternMechanism::StraightGuideDimensions { id, dimensions } => {
                Self::StraightGuideDimensions {
                    id: id.0,
                    dimensions: dimensions
                        .iter()
                        .map(StraightGuideDimensionDtoV2::from_domain)
                        .collect(),
                }
            }
            toniator_domain::PatternMechanism::SelectedGuideIntersections {
                id,
                guide_mechanism_id,
                dimensions,
                merge_epsilon,
            } => Self::SelectedGuideIntersections {
                id: id.0,
                guide_mechanism_id: guide_mechanism_id.0,
                dimensions: dimensions.iter().map(|id| id.0).collect(),
                merge_epsilon: *merge_epsilon,
            },
            toniator_domain::PatternMechanism::AlongGuideSites {
                id,
                guide_mechanism_id,
                dimensions,
                interval_multiplier,
                phase,
            } => Self::AlongGuideSites {
                id: id.0,
                guide_mechanism_id: guide_mechanism_id.0,
                dimensions: dimensions.iter().map(|id| id.0).collect(),
                interval_multiplier: *interval_multiplier,
                phase: *phase,
            },
            toniator_domain::PatternMechanism::RandomSiteProcess {
                id,
                character,
                seed,
            } => Self::RandomSiteProcess {
                id: id.0,
                character: RandomSiteCharacterDtoV2::from_domain(character),
                seed: *seed,
            },
            toniator_domain::PatternMechanism::SiteDensityModulation {
                id,
                base_site_process_id,
                modulation,
            } => Self::SiteDensityModulation {
                id: id.0,
                base_site_process_id: base_site_process_id.0,
                modulation: SiteDensityModulationDtoV2::from_domain(modulation),
            },
            toniator_domain::PatternMechanism::SiteExclusion {
                id,
                density_modulation_id,
                policy,
            } => Self::SiteExclusion {
                id: id.0,
                density_modulation_id: density_modulation_id.0,
                policy: SiteExclusionPolicyDtoV2::from_domain(policy),
            },
            toniator_domain::PatternMechanism::RandomSiteProduct {
                id,
                exclusion_id,
                maximum_attempts,
                maximum_neighbor_checks,
            } => Self::RandomSiteProduct {
                id: id.0,
                exclusion_id: exclusion_id.0,
                maximum_attempts: *maximum_attempts,
                maximum_neighbor_checks: *maximum_neighbor_checks,
            },
        }
    }
    fn into_domain(self) -> toniator_domain::PatternMechanism {
        match self {
            Self::StraightGuides { id } => toniator_domain::PatternMechanism::StraightGuides {
                id: PatternMechanismId(id),
            },
            Self::GuideIntersections {
                id,
                guide_mechanism_id,
            } => toniator_domain::PatternMechanism::GuideIntersections {
                id: PatternMechanismId(id),
                guide_mechanism_id: PatternMechanismId(guide_mechanism_id),
            },
            Self::StraightGuideDimensions { id, dimensions } => {
                toniator_domain::PatternMechanism::StraightGuideDimensions {
                    id: PatternMechanismId(id),
                    dimensions: dimensions
                        .into_iter()
                        .map(StraightGuideDimensionDtoV2::into_domain)
                        .collect(),
                }
            }
            Self::SelectedGuideIntersections {
                id,
                guide_mechanism_id,
                dimensions,
                merge_epsilon,
            } => toniator_domain::PatternMechanism::SelectedGuideIntersections {
                id: PatternMechanismId(id),
                guide_mechanism_id: PatternMechanismId(guide_mechanism_id),
                dimensions: dimensions.into_iter().map(GuideDimensionId).collect(),
                merge_epsilon,
            },
            Self::AlongGuideSites {
                id,
                guide_mechanism_id,
                dimensions,
                interval_multiplier,
                phase,
            } => toniator_domain::PatternMechanism::AlongGuideSites {
                id: PatternMechanismId(id),
                guide_mechanism_id: PatternMechanismId(guide_mechanism_id),
                dimensions: dimensions.into_iter().map(GuideDimensionId).collect(),
                interval_multiplier,
                phase,
            },
            Self::RandomSiteProcess {
                id,
                character,
                seed,
            } => toniator_domain::PatternMechanism::RandomSiteProcess {
                id: PatternMechanismId(id),
                character: character.into_domain(),
                seed,
            },
            Self::SiteDensityModulation {
                id,
                base_site_process_id,
                modulation,
            } => toniator_domain::PatternMechanism::SiteDensityModulation {
                id: PatternMechanismId(id),
                base_site_process_id: PatternMechanismId(base_site_process_id),
                modulation: modulation.into_domain(),
            },
            Self::SiteExclusion {
                id,
                density_modulation_id,
                policy,
            } => toniator_domain::PatternMechanism::SiteExclusion {
                id: PatternMechanismId(id),
                density_modulation_id: PatternMechanismId(density_modulation_id),
                policy: policy.into_domain(),
            },
            Self::RandomSiteProduct {
                id,
                exclusion_id,
                maximum_attempts,
                maximum_neighbor_checks,
            } => toniator_domain::PatternMechanism::RandomSiteProduct {
                id: PatternMechanismId(id),
                exclusion_id: PatternMechanismId(exclusion_id),
                maximum_attempts,
                maximum_neighbor_checks,
            },
        }
    }
}

impl RandomSiteCharacterDtoV2 {
    fn from_domain(value: &RandomSiteCharacter) -> Self {
        match value {
            RandomSiteCharacter::RawUniform => Self::RawUniform,
            RandomSiteCharacter::Even {
                minimum_center_distance,
            } => Self::Even {
                minimum_center_distance: *minimum_center_distance,
            },
            RandomSiteCharacter::Clustered {
                cluster_density,
                cluster_spread,
                cluster_strength,
            } => Self::Clustered {
                cluster_density: *cluster_density,
                cluster_spread: *cluster_spread,
                cluster_strength: *cluster_strength,
            },
        }
    }
    fn into_domain(self) -> RandomSiteCharacter {
        match self {
            Self::RawUniform => RandomSiteCharacter::RawUniform,
            Self::Even {
                minimum_center_distance,
            } => RandomSiteCharacter::Even {
                minimum_center_distance,
            },
            Self::Clustered {
                cluster_density,
                cluster_spread,
                cluster_strength,
            } => RandomSiteCharacter::Clustered {
                cluster_density,
                cluster_spread,
                cluster_strength,
            },
        }
    }
}

impl SiteDensityModulationDtoV2 {
    fn from_domain(value: &SiteDensityModulation) -> Self {
        match value {
            SiteDensityModulation::Uniform => Self::Uniform,
            SiteDensityModulation::ArtworkWeighted {
                mapping,
                strength,
                response,
            } => Self::ArtworkWeighted {
                mapping: SourceMappingDto::from_domain(*mapping),
                strength: *strength,
                response: ArtworkWeightResponseDtoV2::from_domain(response),
            },
        }
    }
    fn into_domain(self) -> SiteDensityModulation {
        match self {
            Self::Uniform => SiteDensityModulation::Uniform,
            Self::ArtworkWeighted {
                mapping,
                strength,
                response,
            } => SiteDensityModulation::ArtworkWeighted {
                mapping: mapping.into_domain(),
                strength,
                response: response.into_domain(),
            },
        }
    }
}

impl ArtworkWeightResponseDtoV2 {
    fn from_domain(value: &ArtworkWeightResponse) -> Self {
        match value {
            ArtworkWeightResponse::Linear => Self::Linear,
            ArtworkWeightResponse::Smoothstep => Self::Smoothstep,
        }
    }
    fn into_domain(self) -> ArtworkWeightResponse {
        match self {
            Self::Linear => ArtworkWeightResponse::Linear,
            Self::Smoothstep => ArtworkWeightResponse::Smoothstep,
        }
    }
}

impl SiteExclusionPolicyDtoV2 {
    fn from_domain(value: &SiteExclusionPolicy) -> Self {
        match value {
            SiteExclusionPolicy::None => Self::None,
            SiteExclusionPolicy::MinimumCenterDistance { minimum } => {
                Self::MinimumCenterDistance { minimum: *minimum }
            }
            SiteExclusionPolicy::VisibleMarkMargin { margin, sizing } => Self::VisibleMarkMargin {
                margin: *margin,
                sizing: VisibleMarkSizingPolicyDtoV2::from_domain(*sizing),
            },
        }
    }
    fn into_domain(self) -> SiteExclusionPolicy {
        match self {
            Self::None => SiteExclusionPolicy::None,
            Self::MinimumCenterDistance { minimum } => {
                SiteExclusionPolicy::MinimumCenterDistance { minimum }
            }
            Self::VisibleMarkMargin { margin, sizing } => SiteExclusionPolicy::VisibleMarkMargin {
                margin,
                sizing: sizing.into_domain(),
            },
        }
    }
}

impl VisibleMarkSizingPolicyDtoV2 {
    fn from_domain(value: VisibleMarkSizingPolicy) -> Self {
        match value {
            VisibleMarkSizingPolicy::MaximumSupportRadius => Self::MaximumSupportRadius,
        }
    }
    fn into_domain(self) -> VisibleMarkSizingPolicy {
        match self {
            Self::MaximumSupportRadius => VisibleMarkSizingPolicy::MaximumSupportRadius,
        }
    }
}
impl PatternOutputLayerDtoV2 {
    fn from_domain(value: &toniator_domain::PatternOutputLayer) -> Self {
        match value {
            toniator_domain::PatternOutputLayer::CircularMarks {
                id,
                site_mechanism_id,
            } => Self::CircularMarks {
                id: id.0,
                site_mechanism_id: site_mechanism_id.0,
            },
            toniator_domain::PatternOutputLayer::MarkPrototype {
                id,
                site_mechanism_id,
                prototype,
                orientation,
            } => Self::MarkPrototype {
                id: id.0,
                site_mechanism_id: site_mechanism_id.0,
                prototype: MarkPrototypeDtoV2::from_domain(prototype),
                orientation: MarkOrientationDtoV2::from_domain(orientation),
            },
        }
    }
    fn into_domain(self) -> toniator_domain::PatternOutputLayer {
        match self {
            Self::CircularMarks {
                id,
                site_mechanism_id,
            } => toniator_domain::PatternOutputLayer::CircularMarks {
                id: PatternOutputLayerId(id),
                site_mechanism_id: PatternMechanismId(site_mechanism_id),
            },
            Self::MarkPrototype {
                id,
                site_mechanism_id,
                prototype,
                orientation,
            } => toniator_domain::PatternOutputLayer::MarkPrototype {
                id: PatternOutputLayerId(id),
                site_mechanism_id: PatternMechanismId(site_mechanism_id),
                prototype: prototype.into_domain(),
                orientation: orientation.into_domain(),
            },
        }
    }
}

impl StraightGuideDimensionDtoV2 {
    fn from_domain(value: &StraightGuideDimension) -> Self {
        Self {
            id: value.id.0,
            baseline_angle_degrees: value.baseline_angle_degrees,
            phase: value.phase,
            spacing_multiplier: value.repetition.spacing_multiplier,
        }
    }
    fn into_domain(self) -> StraightGuideDimension {
        StraightGuideDimension {
            id: GuideDimensionId(self.id),
            baseline_angle_degrees: self.baseline_angle_degrees,
            phase: self.phase,
            repetition: StraightGuideRepetition {
                spacing_multiplier: self.spacing_multiplier,
            },
        }
    }
}
impl MarkPrototypeDtoV2 {
    fn from_domain(value: &MarkPrototype) -> Self {
        match value {
            MarkPrototype::Circle => Self::Circle,
        }
    }
    fn into_domain(self) -> MarkPrototype {
        match self {
            Self::Circle => MarkPrototype::Circle,
        }
    }
}
impl MarkOrientationDtoV2 {
    fn from_domain(value: &MarkOrientation) -> Self {
        match value {
            MarkOrientation::Fixed => Self::Fixed,
            MarkOrientation::GuideTangent { dimension_id } => Self::GuideTangent {
                dimension_id: dimension_id.0,
            },
            MarkOrientation::GuideNormal { dimension_id } => Self::GuideNormal {
                dimension_id: dimension_id.0,
            },
        }
    }
    fn into_domain(self) -> MarkOrientation {
        match self {
            Self::Fixed => MarkOrientation::Fixed,
            Self::GuideTangent { dimension_id } => MarkOrientation::GuideTangent {
                dimension_id: GuideDimensionId(dimension_id),
            },
            Self::GuideNormal { dimension_id } => MarkOrientation::GuideNormal {
                dimension_id: GuideDimensionId(dimension_id),
            },
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
