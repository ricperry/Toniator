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
    ArtworkWeightResponse, AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure,
    AuthoredStructureDraft, AuthoredStructureId, AuthoredStructureKind, CanvasSpec,
    ChannelAppearance, ChannelGeometryResponseDelta, ChannelId, ChannelPaint,
    ChannelPatternInstance, ChannelPatternLayoutDelta, ChannelSourceMapping, ChannelState,
    ChannelTopology, ColorValue, ConnectedGeometryResponse, ConnectedGeometryResponseDelta,
    CoveragePolicy, CurveWinding, DensityMetric2D, DensityMetricDelta2D, Document, DocumentId,
    DocumentPatternSettings, GeneralizedSiteProductDraft, GuideDimension, GuideDimensionDraft,
    GuideDimensionId, GuidePrototype, GuideRepetition, HalftoneChannelModel, HalftoneChannelRole,
    MarkGeometryResponse, MarkGeometryResponseDelta, MarkOrientation, MarkOrientationDraft,
    MarkPrototype, MazeProgram, ModeledChannelState, ParametricCurve, PathStrokeStyle,
    PatternDefinition, PatternDefinitionBundle, PatternDefinitionDraft, PatternDefinitionId,
    PatternDefinitionRecipe, PatternGeometryResponse, PatternMechanismId, PatternOutputLayerId,
    PatternOutputRealizationRecipe, PatternOutputResponseDelta, PatternOutputSettings,
    PatternOutputSettingsRecipe, PatternStructureRecipe, PresetMetadata, PresetRecord,
    RandomSiteCharacter, RegionGeometryResponse, RegionGeometryResponseDelta,
    RegionSamplingStrategy, RegionSourceIntent, SiteDensityModulation, SiteExclusionPolicy,
    SiteUseFilterRecipe, SourceComponent, SourceMapping, SourceMappingComponent, SourcePlacement,
    SourceReference, SourceReferenceId, SpiralCurve, SpiralShape, StraightGuideDimension,
    StraightGuideRepetition, ValidationError, VisibleMarkSizingPolicy,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const CONTAINER_VERSION: u32 = 1;
pub const DOCUMENT_SCHEMA_VERSION: u32 = 5;
/// Standalone pure-schema preset JSON format version. It is deliberately
/// independent from the `.toniator` container and document schema versions.
pub const PRESET_FORMAT_VERSION: u32 = 3;
pub const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_DOCUMENT_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_SOURCE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_UNCOMPRESSED_BYTES: u64 = 132 * 1024 * 1024;

/// Failure at the standalone preset serialization boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresetIoError {
    context: String,
}

impl PresetIoError {
    /// Returns the precise serialization or validation context.
    pub fn context(&self) -> &str {
        &self.context
    }
}

impl std::fmt::Display for PresetIoError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.context)
    }
}

impl std::error::Error for PresetIoError {}

/// Saves a standalone versioned preset without changing the document container.
pub fn save_preset(path: &Path, preset: &PresetRecord) -> Result<(), PresetIoError> {
    toniator_domain::validate_preset_record(preset).map_err(|error| PresetIoError {
        context: error.to_string(),
    })?;
    let bytes =
        serde_json::to_vec_pretty(&PresetEnvelopeDto::from_domain(preset)).map_err(|error| {
            PresetIoError {
                context: error.to_string(),
            }
        })?;
    fs::write(path, bytes).map_err(|error| PresetIoError {
        context: format!("{}: {error}", path.display()),
    })
}

/// Loads and validates one standalone current-version pure-schema preset.
pub fn load_preset(path: &Path) -> Result<PresetRecord, PresetIoError> {
    let bytes = fs::read(path).map_err(|error| PresetIoError {
        context: format!("{}: {error}", path.display()),
    })?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(PresetIoError {
            context: "preset JSON exceeds the 4 MiB limit".into(),
        });
    }
    let envelope: PresetEnvelopeDto =
        serde_json::from_slice(&bytes).map_err(|error| PresetIoError {
            context: error.to_string(),
        })?;
    if envelope.preset_format_version != PRESET_FORMAT_VERSION {
        return Err(PresetIoError {
            context: format!(
                "unsupported preset format version {}",
                envelope.preset_format_version
            ),
        });
    }
    let preset = envelope.into_domain()?;
    toniator_domain::validate_preset_record(&preset).map_err(|error| PresetIoError {
        context: error.to_string(),
    })?;
    Ok(preset)
}

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

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedDocument {
    document: Document,
    sources: SourceBundle,
    versions: StoredVersions,
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
    let (current, manifest) = match envelope.document_schema_version {
        DOCUMENT_SCHEMA_VERSION => {
            let stored: StoredDocumentDtoV4 =
                serde_json::from_slice(&document_bytes).map_err(|error| LoadError::Json {
                    context: error.to_string(),
                })?;
            (
                CurrentDocumentDto {
                    document: stored.document,
                },
                stored.source,
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

/// Saves one fully source-backed current document using deterministic v4 JSON
/// inside the immutable v1 ZIP container layout.
pub fn save(path: &Path, document: &Document, sources: &SourceBundle) -> Result<(), SaveError> {
    document.validate().map_err(save_domain_error)?;
    let source_id = match document.source() {
        SourceReference::Assigned(id) => id,
        SourceReference::Unassigned => {
            return Err(SaveError::SourceDocumentMismatch {
                context: "v4 saving requires an assigned document source".into(),
            });
        }
    };
    if sources.len() != 1 {
        return Err(SaveError::SourceDocumentMismatch {
            context: "v4 saving requires exactly one embedded source".into(),
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
    let dto = StoredDocumentDtoV4::from_domain(document, source, entry_name.clone())?;
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

#[derive(Deserialize)]
struct VersionEnvelope {
    container_version: u32,
    document_schema_version: u32,
}
#[derive(Serialize, Deserialize)]
struct PresetEnvelopeDto {
    preset_format_version: u32,
    metadata: PresetMetadataDto,
    recipe: PresetRecipeDto,
}
#[derive(Serialize, Deserialize)]
struct PresetMetadataDto {
    id: String,
    name: String,
    category: String,
    description: String,
    thumbnail: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct PresetRecipeDto {
    structure: PresetStructureRecipeDto,
    output_settings: Vec<PatternOutputSettingsRecipeDtoV3>,
}
#[derive(Serialize, Deserialize)]
struct PatternOutputSettingsRecipeDtoV3 {
    source_filter: SiteUseFilterRecipeDtoV3,
    response: PatternGeometryResponseDto,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SiteUseFilterRecipeDtoV3 {
    All,
    SitesUsedBy { output_index: usize },
    SitesUnusedBy { output_index: usize },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PresetStructureRecipeDto {
    StraightGrid {
        name: String,
        coverage: CoverageDtoV4,
    },
    GeneralizedStraightGuides {
        name: String,
        coverage: CoverageDtoV4,
        dimensions: Vec<GuideDimensionDraftDto>,
        product: GeneralizedSiteProductDraftDto,
        orientation: MarkOrientationDraftDto,
    },
    RandomSites {
        name: String,
        coverage: CoverageDtoV4,
        character: RandomSiteCharacterDtoV4,
        seed: u32,
        density_modulation: SiteDensityModulationDtoV4,
        exclusion: SiteExclusionPolicyDtoV4,
        maximum_attempts: u32,
        maximum_neighbor_checks: u32,
    },
    ConnectionPaths {
        definition: Box<PresetStructureRecipeDto>,
        program: ConnectionProgramDtoV4,
        style: PathStrokeStyle,
    },
    MazeWalls {
        definition: Box<PresetStructureRecipeDto>,
        program: MazeProgramDtoV4,
        style: PathStrokeStyle,
    },
    AuthoredClosedShapeMarks {
        definition: Box<PresetStructureRecipeDto>,
        segments: Vec<AuthoredCurveSegmentDtoV4>,
    },
    VoronoiRegions {
        definition: Box<PresetStructureRecipeDto>,
    },
    GuideFaceRegions {
        definition: Box<PresetStructureRecipeDto>,
        dimension_indices: Vec<usize>,
    },
    OrderedOutputs {
        definition: Box<PresetStructureRecipeDto>,
        outputs: Vec<PatternOutputRealizationRecipeDtoV3>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PatternOutputRealizationRecipeDtoV3 {
    Marks,
    StructuralPaths {
        style: PathStrokeStyle,
    },
    ConnectionPaths {
        program: ConnectionProgramDtoV4,
        style: PathStrokeStyle,
    },
    MazeWalls {
        program: MazeProgramDtoV4,
        style: PathStrokeStyle,
    },
    VoronoiRegions,
    GuideFaceRegions {
        dimension_indices: Vec<usize>,
    },
}
#[derive(Serialize, Deserialize)]
struct GuideDimensionDraftDto {
    baseline_angle_degrees: f64,
    phase: f64,
    spacing_multiplier: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum GeneralizedSiteProductDraftDto {
    Intersections {
        dimension_indices: Vec<usize>,
        merge_epsilon: f64,
    },
    AlongGuides {
        dimension_indices: Vec<usize>,
        interval_multiplier: f64,
        phase: f64,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MarkOrientationDraftDto {
    Fixed,
    GuideTangent { dimension_index: usize },
    GuideNormal { dimension_index: usize },
}
#[derive(Clone, Serialize, Deserialize)]
struct SourceManifestDto {
    id: String,
    entry_name: String,
    format: EmbeddedSourceFormatDto,
    byte_length: u64,
    sha256: String,
    display_name: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct StoredDocumentDtoV4 {
    container_version: u32,
    document_schema_version: u32,
    document: DocumentDtoV4,
    source: SourceManifestDto,
}
#[derive(Serialize, Deserialize)]
struct CurrentDocumentDto {
    document: DocumentDtoV4,
}

impl StoredDocumentDtoV4 {
    /// Projects a validated document and its matching source into the exact v4 archive envelope.
    ///
    /// # Errors
    ///
    /// Returns a save error when the document cannot be represented by current-v4 persistence.
    fn from_domain(
        document: &Document,
        source: &EmbeddedSource,
        entry_name: String,
    ) -> Result<Self, SaveError> {
        Ok(Self {
            container_version: CONTAINER_VERSION,
            document_schema_version: DOCUMENT_SCHEMA_VERSION,
            document: DocumentDtoV4::from_domain(document)?,
            source: SourceManifestDto {
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
struct DocumentDtoV4 {
    id: u64,
    canvas: CanvasDto,
    source_reference_id: String,
    pattern_definition_bundles: Vec<PatternDefinitionBundleDtoV5>,
    pattern_settings: DocumentPatternSettingsDto,
    channel_configuration: ChannelConfigurationDto,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    authored_structures: Vec<AuthoredStructureDtoV4>,
}

/// Current-v4 persistence representation of one document-owned authored structure.
#[derive(Serialize, Deserialize)]
struct AuthoredStructureDtoV4 {
    id: u64,
    kind: AuthoredStructureKindDtoV4,
    segments: Vec<AuthoredCurveSegmentDtoV4>,
}

/// Current-v4 persistence representation of declared authored-structure topology.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AuthoredStructureKindDtoV4 {
    OpenPath,
    ClosedShape,
}

/// Current-v4 persistence representation of one explicit authored construction segment.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum AuthoredCurveSegmentDtoV4 {
    Line {
        start: AuthoredPointDtoV4,
        end: AuthoredPointDtoV4,
    },
    CubicBezier {
        start: AuthoredPointDtoV4,
        control_1: AuthoredPointDtoV4,
        control_2: AuthoredPointDtoV4,
        end: AuthoredPointDtoV4,
    },
}

/// Current-v4 persistence representation of one authored finite coordinate pair.
#[derive(Serialize, Deserialize)]
struct AuthoredPointDtoV4 {
    x: f64,
    y: f64,
}
#[derive(Serialize, Deserialize)]
struct CanvasDto {
    width: f64,
    height: f64,
}
#[derive(Serialize, Deserialize)]
struct PatternDefinitionDtoV4 {
    id: u64,
    name: String,
    family: PatternFamilyDtoV4,
    mechanisms: Vec<PatternMechanismDtoV4>,
    output_layers: Vec<PatternOutputLayerDtoV4>,
    modulation: PatternModulationDtoV4,
    coverage: CoverageDtoV4,
}

/// Current-v5 atomic structural definition and ordered response authority.
#[derive(Serialize, Deserialize)]
struct PatternDefinitionBundleDtoV5 {
    definition: PatternDefinitionDtoV4,
    output_settings: Vec<PatternOutputSettingsDtoV5>,
}

/// Current-v5 persisted base response keyed to one structural output layer.
#[derive(Serialize, Deserialize)]
struct PatternOutputSettingsDtoV5 {
    output_layer_id: u64,
    response: PatternGeometryResponseDto,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PatternFamilyDtoV4 {
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
    ParametricCurve {
        curve_mechanism_id: u64,
        site_mechanism_id: Option<u64>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PatternMechanismDtoV4 {
    StraightGuides {
        id: u64,
    },
    GuideIntersections {
        id: u64,
        guide_mechanism_id: u64,
    },
    StraightGuideDimensions {
        id: u64,
        dimensions: Vec<StraightGuideDimensionDtoV4>,
    },
    GuideDimensions {
        id: u64,
        dimensions: Vec<GuideDimensionDtoV4>,
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
    ParametricCurveSource {
        id: u64,
        curve: ParametricCurveDtoV4,
        repetition: GuideRepetitionDtoV4,
    },
    AlongParametricCurveSites {
        id: u64,
        curve_mechanism_id: u64,
        interval: f64,
        phase: f64,
    },
    RandomSiteProcess {
        id: u64,
        character: RandomSiteCharacterDtoV4,
        seed: u32,
    },
    SiteDensityModulation {
        id: u64,
        base_site_process_id: u64,
        modulation: SiteDensityModulationDtoV4,
    },
    SiteExclusion {
        id: u64,
        density_modulation_id: u64,
        policy: SiteExclusionPolicyDtoV4,
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
enum RandomSiteCharacterDtoV4 {
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
enum SiteDensityModulationDtoV4 {
    Uniform,
    ArtworkWeighted {
        mapping: SourceMappingDto,
        strength: f64,
        response: ArtworkWeightResponseDtoV4,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ArtworkWeightResponseDtoV4 {
    Linear,
    Smoothstep,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SiteExclusionPolicyDtoV4 {
    None,
    MinimumCenterDistance {
        minimum: f64,
    },
    VisibleMarkMargin {
        margin: f64,
        sizing: VisibleMarkSizingPolicyDtoV4,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VisibleMarkSizingPolicyDtoV4 {
    MaximumSupportRadius,
}
#[derive(Serialize, Deserialize)]
struct StraightGuideDimensionDtoV4 {
    id: u64,
    baseline_angle_degrees: f64,
    phase: f64,
    spacing_multiplier: f64,
}
#[derive(Serialize, Deserialize)]
struct GuideDimensionDtoV4 {
    id: u64,
    baseline_angle_degrees: f64,
    phase: f64,
    prototype: GuidePrototypeDtoV4,
    repetition: GuideRepetitionDtoV4,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum GuidePrototypeDtoV4 {
    AuthoredOpenPath {
        structure_id: u64,
    },
    CircularArc {
        center: AuthoredPointDtoV4,
        radius: f64,
        start_angle_degrees: f64,
        sweep_angle_degrees: f64,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum GuideRepetitionDtoV4 {
    Single,
    TransformStack {
        direction_degrees: f64,
        spacing_multiplier: f64,
    },
    NormalOffset {
        spacing: f64,
        sides: OffsetSidesDtoV4,
        cleanup: OffsetCleanupDtoV4,
    },
}

/// Current-v4 analytic intent for the bounded parametric source vocabulary.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ParametricCurveDtoV4 {
    Spiral {
        shape: SpiralShapeDtoV4,
        turns: f64,
        radial_spacing: f64,
        phase_degrees: f64,
        winding: CurveWindingDtoV4,
    },
}

/// Current-v4 round/square discriminant for one spiral source.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SpiralShapeDtoV4 {
    Round,
    Square,
}

/// Current-v4 winding discriminant for one spiral source.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CurveWindingDtoV4 {
    Clockwise,
    CounterClockwise,
}

/// Persisted current-v4 signed-side intent for normal-offset guide repetition.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OffsetSidesDtoV4 {
    Left,
    Right,
    Both,
}

/// Persisted current-v4 cleanup discriminant for normal-offset guide repetition.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OffsetCleanupDtoV4 {
    DissolveCrossings,
}

impl GuideDimensionDtoV4 {
    /// Projects one generic guide dimension into persisted intent without resolving resources.
    fn from_domain(value: &GuideDimension) -> Self {
        Self {
            id: value.id.0,
            baseline_angle_degrees: value.baseline_angle_degrees,
            phase: value.phase,
            prototype: GuidePrototypeDtoV4::from_domain(&value.prototype),
            repetition: GuideRepetitionDtoV4::from_domain(&value.repetition),
        }
    }

    /// Rebuilds one generic guide dimension for later complete-document validation.
    fn into_domain(self) -> GuideDimension {
        GuideDimension {
            id: GuideDimensionId(self.id),
            baseline_angle_degrees: self.baseline_angle_degrees,
            phase: self.phase,
            prototype: self.prototype.into_domain(),
            repetition: self.repetition.into_domain(),
        }
    }
}

impl GuidePrototypeDtoV4 {
    /// Projects a persisted generic-guide prototype into deterministic current-v4 fields.
    fn from_domain(value: &GuidePrototype) -> Self {
        match value {
            GuidePrototype::AuthoredOpenPath { structure_id } => Self::AuthoredOpenPath {
                structure_id: structure_id.0,
            },
            GuidePrototype::CircularArc {
                center,
                radius,
                start_angle_degrees,
                sweep_angle_degrees,
            } => Self::CircularArc {
                center: AuthoredPointDtoV4::from_domain(*center),
                radius: *radius,
                start_angle_degrees: *start_angle_degrees,
                sweep_angle_degrees: *sweep_angle_degrees,
            },
        }
    }

    /// Rebuilds generic-guide prototype intent for authoritative domain validation.
    fn into_domain(self) -> GuidePrototype {
        match self {
            Self::AuthoredOpenPath { structure_id } => GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(structure_id),
            },
            Self::CircularArc {
                center,
                radius,
                start_angle_degrees,
                sweep_angle_degrees,
            } => GuidePrototype::CircularArc {
                center: center.into_domain(),
                radius,
                start_angle_degrees,
                sweep_angle_degrees,
            },
        }
    }
}

impl GuideRepetitionDtoV4 {
    /// Projects a bounded generic-guide repetition variant into current-v4 intent fields.
    fn from_domain(value: &GuideRepetition) -> Self {
        match value {
            GuideRepetition::Single => Self::Single,
            GuideRepetition::TransformStack {
                direction_degrees,
                spacing_multiplier,
            } => Self::TransformStack {
                direction_degrees: *direction_degrees,
                spacing_multiplier: *spacing_multiplier,
            },
            GuideRepetition::NormalOffset {
                spacing,
                sides,
                cleanup,
            } => Self::NormalOffset {
                spacing: *spacing,
                sides: match sides {
                    toniator_domain::OffsetSides::Left => OffsetSidesDtoV4::Left,
                    toniator_domain::OffsetSides::Right => OffsetSidesDtoV4::Right,
                    toniator_domain::OffsetSides::Both => OffsetSidesDtoV4::Both,
                },
                cleanup: match cleanup {
                    toniator_domain::OffsetCleanup::DissolveCrossings => {
                        OffsetCleanupDtoV4::DissolveCrossings
                    }
                },
            },
        }
    }

    /// Rebuilds one repetition variant for authoritative domain validation.
    fn into_domain(self) -> GuideRepetition {
        match self {
            Self::Single => GuideRepetition::Single,
            Self::TransformStack {
                direction_degrees,
                spacing_multiplier,
            } => GuideRepetition::TransformStack {
                direction_degrees,
                spacing_multiplier,
            },
            Self::NormalOffset {
                spacing,
                sides,
                cleanup,
            } => GuideRepetition::NormalOffset {
                spacing,
                sides: match sides {
                    OffsetSidesDtoV4::Left => toniator_domain::OffsetSides::Left,
                    OffsetSidesDtoV4::Right => toniator_domain::OffsetSides::Right,
                    OffsetSidesDtoV4::Both => toniator_domain::OffsetSides::Both,
                },
                cleanup: match cleanup {
                    OffsetCleanupDtoV4::DissolveCrossings => {
                        toniator_domain::OffsetCleanup::DissolveCrossings
                    }
                },
            },
        }
    }
}
#[derive(Serialize, Deserialize)]
struct PatternOutputLayerDtoV4 {
    id: u64,
    source_filter: SiteUseFilterDtoV5,
    realization: PatternOutputRealizationDtoV5,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SiteUseFilterDtoV5 {
    All,
    SitesUsedBy { output_layer_id: u64 },
    SitesUnusedBy { output_layer_id: u64 },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PatternOutputRealizationDtoV5 {
    CircularMarks {
        site_mechanism_id: u64,
    },
    MarkPrototype {
        site_mechanism_id: u64,
        prototype: MarkPrototypeDtoV4,
        orientation: MarkOrientationDtoV4,
    },
    GuidePaths {
        guide_mechanism_id: u64,
        style: PathStrokeStyle,
    },
    ParametricPaths {
        curve_mechanism_id: u64,
        style: PathStrokeStyle,
    },
    ConnectionPaths {
        site_mechanism_id: u64,
        program: ConnectionProgramDtoV4,
        style: PathStrokeStyle,
    },
    MazeWalls {
        site_mechanism_id: u64,
        program: MazeProgramDtoV4,
        style: PathStrokeStyle,
    },
    Regions {
        source: RegionSourceIntentDtoV5,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RegionSourceIntentDtoV5 {
    VoronoiSites {
        site_mechanism_id: u64,
    },
    GuideFaces {
        guide_mechanism_id: u64,
        dimensions: Vec<u64>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ConnectionProgramDtoV4 {
    NearestLinks {
        adjacency: ConnectionAdjacencyIntentDtoV4,
    },
    RandomLinks {
        adjacency: ConnectionAdjacencyIntentDtoV4,
        minimum_degree: u32,
        seed: u32,
    },
    GridSpanningTree {
        adjacency: ConnectionAdjacencyIntentDtoV4,
        algorithm: GridSpanningTreeAlgorithmDtoV4,
        seed: u32,
    },
}

#[derive(Serialize, Deserialize)]
struct ConnectionAdjacencyIntentDtoV4 {
    maximum_degree: u32,
    maximum_distance: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GridMazeAlgorithmDtoV4 {
    RecursiveBacktracker,
}

#[derive(Serialize, Deserialize)]
struct MazeProgramDtoV4 {
    algorithm: GridMazeAlgorithmDtoV4,
    seed: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GridSpanningTreeAlgorithmDtoV4 {
    RandomizedPrim,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MarkPrototypeDtoV4 {
    Circle,
    AuthoredClosedShape { structure_id: u64 },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MarkOrientationDtoV4 {
    Fixed,
    GuideTangent { dimension_id: u64 },
    GuideNormal { dimension_id: u64 },
}
#[derive(Serialize, Deserialize)]
struct PatternModulationDtoV4 {}
#[derive(Serialize, Deserialize)]
struct CoverageDtoV4 {
    guard_steps: u32,
    additional_margin: f64,
}

impl PresetEnvelopeDto {
    /// Converts a pure-schema preset record into the standalone v2 DTO.
    fn from_domain(value: &PresetRecord) -> Self {
        Self {
            preset_format_version: PRESET_FORMAT_VERSION,
            metadata: PresetMetadataDto {
                id: value.metadata.id.clone(),
                name: value.metadata.name.clone(),
                category: value.metadata.category.clone(),
                description: value.metadata.description.clone(),
                thumbnail: value.metadata.thumbnail.clone(),
            },
            recipe: PresetRecipeDto::from_domain(&value.recipe),
        }
    }

    /// Converts a validated-format DTO into ordinary domain schema data.
    ///
    /// # Errors
    ///
    /// Returns stable metadata or embedded-recipe validation context without
    /// allocating any document-owned identities.
    fn into_domain(self) -> Result<PresetRecord, PresetIoError> {
        let metadata = PresetMetadata {
            id: self.metadata.id,
            name: self.metadata.name,
            category: self.metadata.category,
            description: self.metadata.description,
            thumbnail: self.metadata.thumbnail,
        };
        for value in [
            &metadata.id,
            &metadata.name,
            &metadata.category,
            &metadata.description,
        ] {
            if value.trim().is_empty() || value.chars().any(char::is_control) {
                return Err(PresetIoError {
                    context: "preset metadata must be nonempty printable text".into(),
                });
            }
        }
        if metadata
            .thumbnail
            .as_deref()
            .is_some_and(|value| value.trim().is_empty() || value.chars().any(char::is_control))
        {
            return Err(PresetIoError {
                context: "preset thumbnail must be printable text when present".into(),
            });
        }
        Ok(PresetRecord {
            metadata,
            recipe: self.recipe.into_domain()?,
        })
    }
}

impl PresetRecipeDto {
    /// Converts the complete v3 ID-free recipe, including ordered output responses.
    fn from_domain(value: &PatternDefinitionRecipe) -> Self {
        Self {
            structure: PresetStructureRecipeDto::from_domain(&value.structure),
            output_settings: value
                .output_settings
                .iter()
                .map(|setting| PatternOutputSettingsRecipeDtoV3 {
                    source_filter: match setting.source_filter {
                        SiteUseFilterRecipe::All => SiteUseFilterRecipeDtoV3::All,
                        SiteUseFilterRecipe::SitesUsedBy { output_index } => {
                            SiteUseFilterRecipeDtoV3::SitesUsedBy { output_index }
                        }
                        SiteUseFilterRecipe::SitesUnusedBy { output_index } => {
                            SiteUseFilterRecipeDtoV3::SitesUnusedBy { output_index }
                        }
                    },
                    response: PatternGeometryResponseDto::from_domain(&setting.response),
                })
                .collect(),
        }
    }

    /// Rebuilds the complete v3 recipe without allocating document IDs.
    fn into_domain(self) -> Result<PatternDefinitionRecipe, PresetIoError> {
        Ok(PatternDefinitionRecipe {
            structure: self.structure.into_domain()?,
            output_settings: self
                .output_settings
                .into_iter()
                .map(|setting| PatternOutputSettingsRecipe {
                    source_filter: match setting.source_filter {
                        SiteUseFilterRecipeDtoV3::All => SiteUseFilterRecipe::All,
                        SiteUseFilterRecipeDtoV3::SitesUsedBy { output_index } => {
                            SiteUseFilterRecipe::SitesUsedBy { output_index }
                        }
                        SiteUseFilterRecipeDtoV3::SitesUnusedBy { output_index } => {
                            SiteUseFilterRecipe::SitesUnusedBy { output_index }
                        }
                    },
                    response: setting.response.into_domain(),
                })
                .collect(),
        })
    }
}

impl PresetStructureRecipeDto {
    /// Converts an ID-free recipe into its stable standalone representation.
    fn from_domain(value: &PatternStructureRecipe) -> Self {
        let coverage = |coverage: &CoveragePolicy| CoverageDtoV4 {
            guard_steps: coverage.guard_steps,
            additional_margin: coverage.additional_margin,
        };
        match value {
            PatternStructureRecipe::StraightGrid(draft) => Self::StraightGrid {
                name: draft.name.clone(),
                coverage: coverage(&draft.coverage),
            },
            PatternStructureRecipe::GeneralizedStraightGuides {
                name,
                coverage: definition_coverage,
                dimensions,
                product,
                orientation,
            } => Self::GeneralizedStraightGuides {
                name: name.clone(),
                coverage: coverage(definition_coverage),
                dimensions: dimensions
                    .iter()
                    .map(|dimension| GuideDimensionDraftDto {
                        baseline_angle_degrees: dimension.baseline_angle_degrees,
                        phase: dimension.phase,
                        spacing_multiplier: dimension.spacing_multiplier,
                    })
                    .collect(),
                product: GeneralizedSiteProductDraftDto::from_domain(product),
                orientation: MarkOrientationDraftDto::from_domain(orientation),
            },
            PatternStructureRecipe::RandomSites {
                name,
                coverage: definition_coverage,
                character,
                seed,
                density_modulation,
                exclusion,
                maximum_attempts,
                maximum_neighbor_checks,
            } => Self::RandomSites {
                name: name.clone(),
                coverage: coverage(definition_coverage),
                character: RandomSiteCharacterDtoV4::from_domain(character),
                seed: *seed,
                density_modulation: SiteDensityModulationDtoV4::from_domain(density_modulation),
                exclusion: SiteExclusionPolicyDtoV4::from_domain(exclusion),
                maximum_attempts: *maximum_attempts,
                maximum_neighbor_checks: *maximum_neighbor_checks,
            },
            PatternStructureRecipe::ConnectionPaths {
                definition,
                program,
                style,
            } => Self::ConnectionPaths {
                definition: Box::new(Self::from_domain(definition)),
                program: ConnectionProgramDtoV4::from_domain(program),
                style: *style,
            },
            PatternStructureRecipe::MazeWalls {
                definition,
                program,
                style,
            } => Self::MazeWalls {
                definition: Box::new(Self::from_domain(definition)),
                program: MazeProgramDtoV4::from_domain(program),
                style: *style,
            },
            PatternStructureRecipe::AuthoredClosedShapeMarks { definition, shape } => {
                Self::AuthoredClosedShapeMarks {
                    definition: Box::new(Self::from_domain(definition)),
                    segments: shape
                        .segments()
                        .iter()
                        .map(AuthoredCurveSegmentDtoV4::from_domain)
                        .collect(),
                }
            }
            PatternStructureRecipe::VoronoiRegions { definition } => Self::VoronoiRegions {
                definition: Box::new(Self::from_domain(definition)),
            },
            PatternStructureRecipe::GuideFaceRegions {
                definition,
                dimension_indices,
            } => Self::GuideFaceRegions {
                definition: Box::new(Self::from_domain(definition)),
                dimension_indices: dimension_indices.clone(),
            },
            PatternStructureRecipe::OrderedOutputs {
                definition,
                outputs,
            } => Self::OrderedOutputs {
                definition: Box::new(Self::from_domain(definition)),
                outputs: outputs
                    .iter()
                    .map(PatternOutputRealizationRecipeDtoV3::from_domain)
                    .collect(),
            },
        }
    }

    /// Rebuilds an ID-free recipe without allocating document-owned IDs.
    ///
    /// # Errors
    ///
    /// Returns stable preset validation context when an embedded authored shape is invalid.
    fn into_domain(self) -> Result<PatternStructureRecipe, PresetIoError> {
        let coverage_from = |value: CoverageDtoV4| CoveragePolicy {
            guard_steps: value.guard_steps,
            additional_margin: value.additional_margin,
        };
        match self {
            Self::StraightGrid { name, coverage } => Ok(PatternStructureRecipe::StraightGrid(
                PatternDefinitionDraft {
                    name,
                    coverage: coverage_from(coverage),
                },
            )),
            Self::GeneralizedStraightGuides {
                name,
                coverage: stored_coverage,
                dimensions,
                product,
                orientation,
            } => Ok(PatternStructureRecipe::GeneralizedStraightGuides {
                name,
                coverage: coverage_from(stored_coverage),
                dimensions: dimensions
                    .into_iter()
                    .map(|dimension| GuideDimensionDraft {
                        baseline_angle_degrees: dimension.baseline_angle_degrees,
                        phase: dimension.phase,
                        spacing_multiplier: dimension.spacing_multiplier,
                    })
                    .collect(),
                product: product.into_domain(),
                orientation: orientation.into_domain(),
            }),
            Self::RandomSites {
                name,
                coverage: stored_coverage,
                character,
                seed,
                density_modulation,
                exclusion,
                maximum_attempts,
                maximum_neighbor_checks,
            } => Ok(PatternStructureRecipe::RandomSites {
                name,
                coverage: coverage_from(stored_coverage),
                character: character.into_domain(),
                seed,
                density_modulation: density_modulation.into_domain(),
                exclusion: exclusion.into_domain(),
                maximum_attempts,
                maximum_neighbor_checks,
            }),
            Self::ConnectionPaths {
                definition,
                program,
                style,
            } => Ok(PatternStructureRecipe::ConnectionPaths {
                definition: Box::new(definition.into_domain()?),
                program: program.into_domain(),
                style,
            }),
            Self::MazeWalls {
                definition,
                program,
                style,
            } => Ok(PatternStructureRecipe::MazeWalls {
                definition: Box::new(definition.into_domain()?),
                program: program.into_domain(),
                style,
            }),
            Self::AuthoredClosedShapeMarks {
                definition,
                segments,
            } => {
                let shape = AuthoredStructureDraft::new(
                    AuthoredStructureKind::ClosedShape,
                    segments
                        .into_iter()
                        .map(AuthoredCurveSegmentDtoV4::into_domain)
                        .collect(),
                )
                .map_err(|error| PresetIoError {
                    context: error.to_string(),
                })?;
                Ok(PatternStructureRecipe::AuthoredClosedShapeMarks {
                    definition: Box::new(definition.into_domain()?),
                    shape,
                })
            }
            Self::VoronoiRegions { definition } => Ok(PatternStructureRecipe::VoronoiRegions {
                definition: Box::new(definition.into_domain()?),
            }),
            Self::GuideFaceRegions {
                definition,
                dimension_indices,
            } => Ok(PatternStructureRecipe::GuideFaceRegions {
                definition: Box::new(definition.into_domain()?),
                dimension_indices,
            }),
            Self::OrderedOutputs {
                definition,
                outputs,
            } => Ok(PatternStructureRecipe::OrderedOutputs {
                definition: Box::new(definition.into_domain()?),
                outputs: outputs
                    .into_iter()
                    .map(PatternOutputRealizationRecipeDtoV3::into_domain)
                    .collect(),
            }),
        }
    }
}

impl PatternOutputRealizationRecipeDtoV3 {
    /// Converts one ordered ID-free output recipe without allocating document references.
    fn from_domain(value: &PatternOutputRealizationRecipe) -> Self {
        match value {
            PatternOutputRealizationRecipe::Marks => Self::Marks,
            PatternOutputRealizationRecipe::StructuralPaths { style } => {
                Self::StructuralPaths { style: *style }
            }
            PatternOutputRealizationRecipe::ConnectionPaths { program, style } => {
                Self::ConnectionPaths {
                    program: ConnectionProgramDtoV4::from_domain(program),
                    style: *style,
                }
            }
            PatternOutputRealizationRecipe::MazeWalls { program, style } => Self::MazeWalls {
                program: MazeProgramDtoV4::from_domain(program),
                style: *style,
            },
            PatternOutputRealizationRecipe::VoronoiRegions => Self::VoronoiRegions,
            PatternOutputRealizationRecipe::GuideFaceRegions { dimension_indices } => {
                Self::GuideFaceRegions {
                    dimension_indices: dimension_indices.clone(),
                }
            }
        }
    }

    /// Restores one ID-free output recipe without accepting derived output state.
    fn into_domain(self) -> PatternOutputRealizationRecipe {
        match self {
            Self::Marks => PatternOutputRealizationRecipe::Marks,
            Self::StructuralPaths { style } => {
                PatternOutputRealizationRecipe::StructuralPaths { style }
            }
            Self::ConnectionPaths { program, style } => {
                PatternOutputRealizationRecipe::ConnectionPaths {
                    program: program.into_domain(),
                    style,
                }
            }
            Self::MazeWalls { program, style } => PatternOutputRealizationRecipe::MazeWalls {
                program: program.into_domain(),
                style,
            },
            Self::VoronoiRegions => PatternOutputRealizationRecipe::VoronoiRegions,
            Self::GuideFaceRegions { dimension_indices } => {
                PatternOutputRealizationRecipe::GuideFaceRegions { dimension_indices }
            }
        }
    }
}

impl GeneralizedSiteProductDraftDto {
    /// Converts typed index references into standalone DTO form.
    fn from_domain(value: &GeneralizedSiteProductDraft) -> Self {
        match value {
            GeneralizedSiteProductDraft::Intersections {
                dimension_indices,
                merge_epsilon,
            } => Self::Intersections {
                dimension_indices: dimension_indices.clone(),
                merge_epsilon: *merge_epsilon,
            },
            GeneralizedSiteProductDraft::AlongGuides {
                dimension_indices,
                interval_multiplier,
                phase,
            } => Self::AlongGuides {
                dimension_indices: dimension_indices.clone(),
                interval_multiplier: *interval_multiplier,
                phase: *phase,
            },
        }
    }

    /// Converts standalone index references back to typed recipe data.
    fn into_domain(self) -> GeneralizedSiteProductDraft {
        match self {
            Self::Intersections {
                dimension_indices,
                merge_epsilon,
            } => GeneralizedSiteProductDraft::Intersections {
                dimension_indices,
                merge_epsilon,
            },
            Self::AlongGuides {
                dimension_indices,
                interval_multiplier,
                phase,
            } => GeneralizedSiteProductDraft::AlongGuides {
                dimension_indices,
                interval_multiplier,
                phase,
            },
        }
    }
}

impl MarkOrientationDraftDto {
    /// Converts typed orientation selection into standalone DTO form.
    fn from_domain(value: &MarkOrientationDraft) -> Self {
        match value {
            MarkOrientationDraft::Fixed => Self::Fixed,
            MarkOrientationDraft::GuideTangent { dimension_index } => Self::GuideTangent {
                dimension_index: *dimension_index,
            },
            MarkOrientationDraft::GuideNormal { dimension_index } => Self::GuideNormal {
                dimension_index: *dimension_index,
            },
        }
    }

    /// Converts standalone orientation selection back to typed recipe data.
    fn into_domain(self) -> MarkOrientationDraft {
        match self {
            Self::Fixed => MarkOrientationDraft::Fixed,
            Self::GuideTangent { dimension_index } => {
                MarkOrientationDraft::GuideTangent { dimension_index }
            }
            Self::GuideNormal { dimension_index } => {
                MarkOrientationDraft::GuideNormal { dimension_index }
            }
        }
    }
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
    pattern_instance: ChannelPatternInstanceDto,
    appearance: AppearanceDto,
    source_mapping: LegacySourceMappingDto,
}
#[derive(Serialize, Deserialize)]
struct ModeledChannelDto {
    role: HalftoneChannelRoleDto,
    id: u64,
    pattern_instance: ChannelPatternInstanceDto,
    mapping: SourceMappingDto,
    paint: PaintDto,
    visible: bool,
    opacity: f64,
}
#[derive(Serialize, Deserialize)]
struct DocumentPatternSettingsDto {
    definition_id: u64,
    density: DensityDto,
    pattern_rotation_degrees: f64,
    shape_rotation_degrees: f64,
}
#[derive(Serialize, Deserialize)]
struct ChannelPatternInstanceDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    definition_override: Option<u64>,
    layout_delta: LayoutDeltaDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    shape_rotation_delta_degrees: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    output_response_deltas: Vec<PatternOutputResponseDeltaDtoV5>,
}

/// Current-v5 optional channel response intent keyed to a structural output.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PatternOutputResponseDeltaDtoV5 {
    output_layer_id: u64,
    delta: ChannelGeometryResponseDeltaDto,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum PatternGeometryResponseDto {
    Marks { response: MarkResponseDto },
    Connected { response: ConnectedResponseDto },
    Regions { response: RegionResponseDtoV5 },
}

/// Current v5 tagged authored treatment for a region output.
#[derive(Serialize, Deserialize)]
#[serde(tag = "treatment", rename_all = "snake_case", deny_unknown_fields)]
enum RegionResponseDtoV5 {
    Full {
        sampling: RegionSamplingStrategyDtoV5,
    },
    Scale {
        sampling: RegionSamplingStrategyDtoV5,
        minimum_scale: f64,
        maximum_scale: f64,
    },
    ConstantGap {
        sampling: RegionSamplingStrategyDtoV5,
        minimum_gap: f64,
        maximum_gap: f64,
    },
}

/// Current v5 sampling selector; absence is intentionally rejected by serde.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RegionSamplingStrategyDtoV5 {
    ReferencePoint,
    AreaAverage,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ChannelGeometryResponseDeltaDto {
    Marks { delta: MarkResponseDeltaDto },
    Connected { delta: ConnectedResponseDeltaDto },
    Regions { delta: RegionResponseDeltaDtoV5 },
}

/// Current v5 tagged region endpoint delta record.
#[derive(Serialize, Deserialize)]
#[serde(tag = "treatment", rename_all = "snake_case", deny_unknown_fields)]
enum RegionResponseDeltaDtoV5 {
    Scale {
        minimum_scale_delta: Option<f64>,
        maximum_scale_delta: Option<f64>,
    },
    ConstantGap {
        minimum_gap_delta: Option<f64>,
        maximum_gap_delta: Option<f64>,
    },
}

impl PatternGeometryResponseDto {
    /// Projects one typed base response without resolving channel deltas.
    fn from_domain(value: &PatternGeometryResponse) -> Self {
        match value {
            PatternGeometryResponse::Marks(response) => Self::Marks {
                response: MarkResponseDto::from_domain(response),
            },
            PatternGeometryResponse::Connected(response) => Self::Connected {
                response: ConnectedResponseDto::from_domain(response),
            },
            PatternGeometryResponse::Regions(response) => Self::Regions {
                response: RegionResponseDtoV5::from_domain(response),
            },
        }
    }

    /// Rebuilds one typed base response for domain-owned bundle validation.
    fn into_domain(self) -> PatternGeometryResponse {
        match self {
            Self::Marks { response } => PatternGeometryResponse::Marks(response.into_domain()),
            Self::Connected { response } => {
                PatternGeometryResponse::Connected(response.into_domain())
            }
            Self::Regions { response } => PatternGeometryResponse::Regions(response.into_domain()),
        }
    }
}

impl ChannelGeometryResponseDeltaDto {
    /// Projects one typed channel delta without materializing an effective response.
    fn from_domain(value: &ChannelGeometryResponseDelta) -> Self {
        match value {
            ChannelGeometryResponseDelta::Marks(delta) => Self::Marks {
                delta: MarkResponseDeltaDto::from_domain(delta),
            },
            ChannelGeometryResponseDelta::Connected(delta) => Self::Connected {
                delta: ConnectedResponseDeltaDto::from_domain(delta),
            },
            ChannelGeometryResponseDelta::Regions(delta) => Self::Regions {
                delta: RegionResponseDeltaDtoV5::from_domain(delta),
            },
        }
    }

    /// Rebuilds one typed channel delta for domain-owned alignment validation.
    fn into_domain(self) -> ChannelGeometryResponseDelta {
        match self {
            Self::Marks { delta } => ChannelGeometryResponseDelta::Marks(delta.into_domain()),
            Self::Connected { delta } => {
                ChannelGeometryResponseDelta::Connected(delta.into_domain())
            }
            Self::Regions { delta } => ChannelGeometryResponseDelta::Regions(delta.into_domain()),
        }
    }
}

impl RegionResponseDtoV5 {
    /// Projects current authored region treatment without resolving effective channel values.
    fn from_domain(value: &RegionGeometryResponse) -> Self {
        match value {
            RegionGeometryResponse::Full { sampling } => Self::Full {
                sampling: RegionSamplingStrategyDtoV5::from_domain(*sampling),
            },
            RegionGeometryResponse::Scale {
                sampling,
                minimum_scale,
                maximum_scale,
            } => Self::Scale {
                sampling: RegionSamplingStrategyDtoV5::from_domain(*sampling),
                minimum_scale: *minimum_scale,
                maximum_scale: *maximum_scale,
            },
            RegionGeometryResponse::ConstantGap {
                sampling,
                minimum_gap,
                maximum_gap,
            } => Self::ConstantGap {
                sampling: RegionSamplingStrategyDtoV5::from_domain(*sampling),
                minimum_gap: *minimum_gap,
                maximum_gap: *maximum_gap,
            },
        }
    }

    /// Rebuilds one current authored region treatment for domain validation.
    fn into_domain(self) -> RegionGeometryResponse {
        match self {
            Self::Full { sampling } => RegionGeometryResponse::Full {
                sampling: sampling.into_domain(),
            },
            Self::Scale {
                sampling,
                minimum_scale,
                maximum_scale,
            } => RegionGeometryResponse::Scale {
                sampling: sampling.into_domain(),
                minimum_scale,
                maximum_scale,
            },
            Self::ConstantGap {
                sampling,
                minimum_gap,
                maximum_gap,
            } => RegionGeometryResponse::ConstantGap {
                sampling: sampling.into_domain(),
                minimum_gap,
                maximum_gap,
            },
        }
    }
}

impl RegionSamplingStrategyDtoV5 {
    /// Projects the stable sampling tag without deriving a sample.
    fn from_domain(value: RegionSamplingStrategy) -> Self {
        match value {
            RegionSamplingStrategy::ReferencePoint => Self::ReferencePoint,
            RegionSamplingStrategy::AreaAverage => Self::AreaAverage,
        }
    }
    /// Rebuilds the stable sampling tag without deriving a sample.
    fn into_domain(self) -> RegionSamplingStrategy {
        match self {
            Self::ReferencePoint => RegionSamplingStrategy::ReferencePoint,
            Self::AreaAverage => RegionSamplingStrategy::AreaAverage,
        }
    }
}

impl RegionResponseDeltaDtoV5 {
    /// Projects treatment-compatible endpoint delta intent without materialization.
    fn from_domain(value: &RegionGeometryResponseDelta) -> Self {
        match value {
            RegionGeometryResponseDelta::Scale {
                minimum_scale_delta,
                maximum_scale_delta,
            } => Self::Scale {
                minimum_scale_delta: *minimum_scale_delta,
                maximum_scale_delta: *maximum_scale_delta,
            },
            RegionGeometryResponseDelta::ConstantGap {
                minimum_gap_delta,
                maximum_gap_delta,
            } => Self::ConstantGap {
                minimum_gap_delta: *minimum_gap_delta,
                maximum_gap_delta: *maximum_gap_delta,
            },
        }
    }
    /// Rebuilds treatment-compatible endpoint delta intent without materialization.
    fn into_domain(self) -> RegionGeometryResponseDelta {
        match self {
            Self::Scale {
                minimum_scale_delta,
                maximum_scale_delta,
            } => RegionGeometryResponseDelta::Scale {
                minimum_scale_delta,
                maximum_scale_delta,
            },
            Self::ConstantGap {
                minimum_gap_delta,
                maximum_gap_delta,
            } => RegionGeometryResponseDelta::ConstantGap {
                minimum_gap_delta,
                maximum_gap_delta,
            },
        }
    }
}
#[derive(Serialize, Deserialize)]
struct LayoutDeltaDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    density: Option<DensityDeltaDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rotation_degrees: Option<f64>,
    translation_x: f64,
    translation_y: f64,
}
#[derive(Serialize, Deserialize)]
struct DensityDeltaDto {
    across_x_delta: f64,
    across_y_delta: f64,
}
#[derive(Serialize, Deserialize)]
struct MarkResponseDeltaDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    minimum_fill_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maximum_fill_delta: Option<f64>,
}
#[derive(Serialize, Deserialize)]
struct ConnectedResponseDto {
    minimum_thickness: f64,
    maximum_thickness: f64,
}
#[derive(Serialize, Deserialize)]
struct ConnectedResponseDeltaDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    minimum_thickness_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maximum_thickness_delta: Option<f64>,
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
    minimum_fill: f64,
    maximum_fill: f64,
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

impl DocumentDtoV4 {
    /// Projects an authoritative document into deterministic current-v4 persistence without runtime state.
    ///
    /// # Errors
    ///
    /// Returns a save error for an unassigned source or incoherent channel configuration before an
    /// archive is written; an empty authored store is omitted to preserve accepted old-v4 bytes.
    fn from_domain(document: &Document) -> Result<Self, SaveError> {
        let source_reference_id = match document.source() {
            SourceReference::Assigned(id) => id.as_str().to_owned(),
            SourceReference::Unassigned => {
                return Err(SaveError::SourceDocumentMismatch {
                    context: "v4 saving requires an assigned document source".into(),
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
            pattern_definition_bundles: document
                .pattern_definition_bundles()
                .iter()
                .map(PatternDefinitionBundleDtoV5::from_domain)
                .collect(),
            pattern_settings: DocumentPatternSettingsDto::from_domain(document.pattern_settings()),
            channel_configuration,
            authored_structures: document
                .authored_structures()
                .iter()
                .map(AuthoredStructureDtoV4::from_domain)
                .collect(),
        })
    }
    /// Rebuilds and validates the complete authoritative document before a loaded archive commits.
    ///
    /// # Errors
    ///
    /// Returns stable domain validation diagnostics for authored IDs, coordinates, topology, bounds,
    /// or existing document state; no partially rebuilt document escapes this boundary.
    fn into_domain(self) -> Result<Document, ValidationError> {
        let source = SourceReference::Assigned(dto_source_id(&self.source_reference_id)?);
        let definitions = self
            .pattern_definition_bundles
            .into_iter()
            .map(PatternDefinitionBundleDtoV5::into_domain)
            .collect::<Result<Vec<_>, _>>()?;
        let authored_structures = self
            .authored_structures
            .into_iter()
            .map(AuthoredStructureDtoV4::into_domain)
            .collect::<Result<Vec<_>, _>>()?;
        match self.channel_configuration {
            ChannelConfigurationDto::Legacy { channels } => {
                Document::with_source_and_authored_structures(
                    DocumentId(self.id),
                    CanvasSpec {
                        width: self.canvas.width,
                        height: self.canvas.height,
                    },
                    source,
                    definitions,
                    self.pattern_settings.into_domain(),
                    channels
                        .into_iter()
                        .map(LegacyChannelDto::into_domain)
                        .collect(),
                    authored_structures,
                )
            }
            ChannelConfigurationDto::Topology { model, channels } => {
                Document::with_source_topology_and_authored_structures(
                    DocumentId(self.id),
                    CanvasSpec {
                        width: self.canvas.width,
                        height: self.canvas.height,
                    },
                    source,
                    definitions,
                    self.pattern_settings.into_domain(),
                    model.into(),
                    ChannelTopology::new(
                        channels
                            .into_iter()
                            .map(ModeledChannelDto::into_domain)
                            .collect(),
                    ),
                    authored_structures,
                )
            }
        }
    }
}

impl AuthoredStructureDtoV4 {
    /// Projects one validated domain-owned structure into deterministic current-v4 persistence fields.
    fn from_domain(value: &AuthoredStructure) -> Self {
        Self {
            id: value.id().0,
            kind: AuthoredStructureKindDtoV4::from_domain(value.kind()),
            segments: value
                .segments()
                .iter()
                .map(AuthoredCurveSegmentDtoV4::from_domain)
                .collect(),
        }
    }

    /// Rebuilds one validated domain-owned structure before the document can commit loaded data.
    ///
    /// # Errors
    ///
    /// Returns the stable domain validation diagnostic for invalid IDs, coordinates, topology, or bounds.
    fn into_domain(self) -> Result<AuthoredStructure, ValidationError> {
        AuthoredStructure::new(
            AuthoredStructureId(self.id),
            self.kind.into_domain(),
            self.segments
                .into_iter()
                .map(AuthoredCurveSegmentDtoV4::into_domain)
                .collect(),
        )
    }
}

impl AuthoredStructureKindDtoV4 {
    /// Converts declared domain topology into its stable current-v4 string representation.
    fn from_domain(value: AuthoredStructureKind) -> Self {
        match value {
            AuthoredStructureKind::OpenPath => Self::OpenPath,
            AuthoredStructureKind::ClosedShape => Self::ClosedShape,
        }
    }

    /// Converts stable current-v4 topology into its domain-owned enum.
    fn into_domain(self) -> AuthoredStructureKind {
        match self {
            Self::OpenPath => AuthoredStructureKind::OpenPath,
            Self::ClosedShape => AuthoredStructureKind::ClosedShape,
        }
    }
}

impl AuthoredCurveSegmentDtoV4 {
    /// Projects one explicit domain segment into the matching tagged current-v4 representation.
    fn from_domain(value: &AuthoredCurveSegment) -> Self {
        match value {
            AuthoredCurveSegment::Line { start, end } => Self::Line {
                start: AuthoredPointDtoV4::from_domain(*start),
                end: AuthoredPointDtoV4::from_domain(*end),
            },
            AuthoredCurveSegment::CubicBezier {
                start,
                control_1,
                control_2,
                end,
            } => Self::CubicBezier {
                start: AuthoredPointDtoV4::from_domain(*start),
                control_1: AuthoredPointDtoV4::from_domain(*control_1),
                control_2: AuthoredPointDtoV4::from_domain(*control_2),
                end: AuthoredPointDtoV4::from_domain(*end),
            },
        }
    }

    /// Converts one tagged current-v4 segment without inferring closure, winding, or render semantics.
    fn into_domain(self) -> AuthoredCurveSegment {
        match self {
            Self::Line { start, end } => AuthoredCurveSegment::Line {
                start: start.into_domain(),
                end: end.into_domain(),
            },
            Self::CubicBezier {
                start,
                control_1,
                control_2,
                end,
            } => AuthoredCurveSegment::CubicBezier {
                start: start.into_domain(),
                control_1: control_1.into_domain(),
                control_2: control_2.into_domain(),
                end: end.into_domain(),
            },
        }
    }
}

impl AuthoredPointDtoV4 {
    /// Projects one authored coordinate pair into deterministic current-v4 numeric fields.
    fn from_domain(value: AuthoredPoint2) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }

    /// Converts one stored coordinate pair for authoritative domain validation.
    fn into_domain(self) -> AuthoredPoint2 {
        AuthoredPoint2 {
            x: self.x,
            y: self.y,
        }
    }
}

impl PatternDefinitionDtoV4 {
    fn from_domain(value: &PatternDefinition) -> Self {
        Self {
            id: value.id.0,
            name: value.name.clone(),
            family: PatternFamilyDtoV4::from_domain(&value.family),
            mechanisms: value
                .mechanisms
                .iter()
                .map(PatternMechanismDtoV4::from_domain)
                .collect(),
            output_layers: value
                .output_layers
                .iter()
                .map(PatternOutputLayerDtoV4::from_domain)
                .collect(),
            modulation: PatternModulationDtoV4 {},
            coverage: CoverageDtoV4 {
                guard_steps: value.coverage.guard_steps,
                additional_margin: value.coverage.additional_margin,
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
                .map(PatternMechanismDtoV4::into_domain)
                .collect(),
            output_layers: self
                .output_layers
                .into_iter()
                .map(PatternOutputLayerDtoV4::into_domain)
                .collect(),
            modulation: toniator_domain::PatternModulation,
            coverage: CoveragePolicy {
                guard_steps: self.coverage.guard_steps,
                additional_margin: self.coverage.additional_margin,
            },
        }
    }
}

impl PatternDefinitionBundleDtoV5 {
    /// Projects one complete v5 structural-and-response bundle without derived state.
    fn from_domain(value: &PatternDefinitionBundle) -> Self {
        Self {
            definition: PatternDefinitionDtoV4::from_domain(&value.definition),
            output_settings: value
                .output_settings
                .iter()
                .map(|setting| PatternOutputSettingsDtoV5 {
                    output_layer_id: setting.output_layer_id.0,
                    response: PatternGeometryResponseDto::from_domain(&setting.response),
                })
                .collect(),
        }
    }

    /// Rebuilds one complete v5 bundle for domain-owned alignment validation.
    fn into_domain(self) -> Result<PatternDefinitionBundle, ValidationError> {
        Ok(PatternDefinitionBundle {
            definition: self.definition.into_domain(),
            output_settings: self
                .output_settings
                .into_iter()
                .map(|setting| PatternOutputSettings {
                    output_layer_id: PatternOutputLayerId(setting.output_layer_id),
                    response: setting.response.into_domain(),
                })
                .collect(),
        })
    }
}
impl PatternFamilyDtoV4 {
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
            toniator_domain::PatternFamily::ParametricCurve {
                curve_mechanism_id,
                site_mechanism_id,
            } => Self::ParametricCurve {
                curve_mechanism_id: curve_mechanism_id.0,
                site_mechanism_id: site_mechanism_id.map(|id| id.0),
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
            Self::ParametricCurve {
                curve_mechanism_id,
                site_mechanism_id,
            } => toniator_domain::PatternFamily::ParametricCurve {
                curve_mechanism_id: PatternMechanismId(curve_mechanism_id),
                site_mechanism_id: site_mechanism_id.map(PatternMechanismId),
            },
        }
    }
}
impl PatternMechanismDtoV4 {
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
                        .map(StraightGuideDimensionDtoV4::from_domain)
                        .collect(),
                }
            }
            toniator_domain::PatternMechanism::GuideDimensions { id, dimensions } => {
                Self::GuideDimensions {
                    id: id.0,
                    dimensions: dimensions
                        .iter()
                        .map(GuideDimensionDtoV4::from_domain)
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
            toniator_domain::PatternMechanism::ParametricCurveSource {
                id,
                curve,
                repetition,
            } => Self::ParametricCurveSource {
                id: id.0,
                curve: ParametricCurveDtoV4::from_domain(curve),
                repetition: GuideRepetitionDtoV4::from_domain(repetition),
            },
            toniator_domain::PatternMechanism::AlongParametricCurveSites {
                id,
                curve_mechanism_id,
                interval,
                phase,
            } => Self::AlongParametricCurveSites {
                id: id.0,
                curve_mechanism_id: curve_mechanism_id.0,
                interval: *interval,
                phase: *phase,
            },
            toniator_domain::PatternMechanism::RandomSiteProcess {
                id,
                character,
                seed,
            } => Self::RandomSiteProcess {
                id: id.0,
                character: RandomSiteCharacterDtoV4::from_domain(character),
                seed: *seed,
            },
            toniator_domain::PatternMechanism::SiteDensityModulation {
                id,
                base_site_process_id,
                modulation,
            } => Self::SiteDensityModulation {
                id: id.0,
                base_site_process_id: base_site_process_id.0,
                modulation: SiteDensityModulationDtoV4::from_domain(modulation),
            },
            toniator_domain::PatternMechanism::SiteExclusion {
                id,
                density_modulation_id,
                policy,
            } => Self::SiteExclusion {
                id: id.0,
                density_modulation_id: density_modulation_id.0,
                policy: SiteExclusionPolicyDtoV4::from_domain(policy),
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
                        .map(StraightGuideDimensionDtoV4::into_domain)
                        .collect(),
                }
            }
            Self::GuideDimensions { id, dimensions } => {
                toniator_domain::PatternMechanism::GuideDimensions {
                    id: PatternMechanismId(id),
                    dimensions: dimensions
                        .into_iter()
                        .map(GuideDimensionDtoV4::into_domain)
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
            Self::ParametricCurveSource {
                id,
                curve,
                repetition,
            } => toniator_domain::PatternMechanism::ParametricCurveSource {
                id: PatternMechanismId(id),
                curve: curve.into_domain(),
                repetition: repetition.into_domain(),
            },
            Self::AlongParametricCurveSites {
                id,
                curve_mechanism_id,
                interval,
                phase,
            } => toniator_domain::PatternMechanism::AlongParametricCurveSites {
                id: PatternMechanismId(id),
                curve_mechanism_id: PatternMechanismId(curve_mechanism_id),
                interval,
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

impl RandomSiteCharacterDtoV4 {
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

impl SiteDensityModulationDtoV4 {
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
                response: ArtworkWeightResponseDtoV4::from_domain(response),
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

impl ArtworkWeightResponseDtoV4 {
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

impl SiteExclusionPolicyDtoV4 {
    fn from_domain(value: &SiteExclusionPolicy) -> Self {
        match value {
            SiteExclusionPolicy::None => Self::None,
            SiteExclusionPolicy::MinimumCenterDistance { minimum } => {
                Self::MinimumCenterDistance { minimum: *minimum }
            }
            SiteExclusionPolicy::VisibleMarkMargin { margin, sizing } => Self::VisibleMarkMargin {
                margin: *margin,
                sizing: VisibleMarkSizingPolicyDtoV4::from_domain(*sizing),
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

impl VisibleMarkSizingPolicyDtoV4 {
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
impl PatternOutputLayerDtoV4 {
    /// Serializes one authored output layer, including its explicit site-use filter.
    fn from_domain(value: &toniator_domain::PatternOutputLayer) -> Self {
        let source_filter = match value.source_filter {
            toniator_domain::SiteUseFilter::All => SiteUseFilterDtoV5::All,
            toniator_domain::SiteUseFilter::SitesUsedBy { output_layer_id } => {
                SiteUseFilterDtoV5::SitesUsedBy {
                    output_layer_id: output_layer_id.0,
                }
            }
            toniator_domain::SiteUseFilter::SitesUnusedBy { output_layer_id } => {
                SiteUseFilterDtoV5::SitesUnusedBy {
                    output_layer_id: output_layer_id.0,
                }
            }
        };
        let realization = match &value.realization {
            toniator_domain::PatternOutputRealization::CircularMarks { site_mechanism_id } => {
                PatternOutputRealizationDtoV5::CircularMarks {
                    site_mechanism_id: site_mechanism_id.0,
                }
            }
            toniator_domain::PatternOutputRealization::MarkPrototype {
                site_mechanism_id,
                prototype,
                orientation,
            } => PatternOutputRealizationDtoV5::MarkPrototype {
                site_mechanism_id: site_mechanism_id.0,
                prototype: MarkPrototypeDtoV4::from_domain(prototype),
                orientation: MarkOrientationDtoV4::from_domain(orientation),
            },
            toniator_domain::PatternOutputRealization::GuidePaths {
                guide_mechanism_id,
                style,
            } => PatternOutputRealizationDtoV5::GuidePaths {
                guide_mechanism_id: guide_mechanism_id.0,
                style: *style,
            },
            toniator_domain::PatternOutputRealization::ParametricPaths {
                curve_mechanism_id,
                style,
            } => PatternOutputRealizationDtoV5::ParametricPaths {
                curve_mechanism_id: curve_mechanism_id.0,
                style: *style,
            },
            toniator_domain::PatternOutputRealization::ConnectionPaths {
                site_mechanism_id,
                program,
                style,
            } => PatternOutputRealizationDtoV5::ConnectionPaths {
                site_mechanism_id: site_mechanism_id.0,
                program: ConnectionProgramDtoV4::from_domain(program),
                style: *style,
            },
            toniator_domain::PatternOutputRealization::MazeWalls {
                site_mechanism_id,
                program,
                style,
            } => PatternOutputRealizationDtoV5::MazeWalls {
                site_mechanism_id: site_mechanism_id.0,
                program: MazeProgramDtoV4::from_domain(program),
                style: *style,
            },
            toniator_domain::PatternOutputRealization::Regions { source } => {
                PatternOutputRealizationDtoV5::Regions {
                    source: RegionSourceIntentDtoV5::from_domain(source),
                }
            }
        };
        Self {
            id: value.id.0,
            source_filter,
            realization,
        }
    }

    /// Restores one authored output layer and rejects no derived realization state.
    fn into_domain(self) -> toniator_domain::PatternOutputLayer {
        let source_filter = match self.source_filter {
            SiteUseFilterDtoV5::All => toniator_domain::SiteUseFilter::All,
            SiteUseFilterDtoV5::SitesUsedBy { output_layer_id } => {
                toniator_domain::SiteUseFilter::SitesUsedBy {
                    output_layer_id: PatternOutputLayerId(output_layer_id),
                }
            }
            SiteUseFilterDtoV5::SitesUnusedBy { output_layer_id } => {
                toniator_domain::SiteUseFilter::SitesUnusedBy {
                    output_layer_id: PatternOutputLayerId(output_layer_id),
                }
            }
        };
        let realization = match self.realization {
            PatternOutputRealizationDtoV5::CircularMarks { site_mechanism_id } => {
                toniator_domain::PatternOutputRealization::CircularMarks {
                    site_mechanism_id: PatternMechanismId(site_mechanism_id),
                }
            }
            PatternOutputRealizationDtoV5::MarkPrototype {
                site_mechanism_id,
                prototype,
                orientation,
            } => toniator_domain::PatternOutputRealization::MarkPrototype {
                site_mechanism_id: PatternMechanismId(site_mechanism_id),
                prototype: prototype.into_domain(),
                orientation: orientation.into_domain(),
            },
            PatternOutputRealizationDtoV5::GuidePaths {
                guide_mechanism_id,
                style,
            } => toniator_domain::PatternOutputRealization::GuidePaths {
                guide_mechanism_id: PatternMechanismId(guide_mechanism_id),
                style,
            },
            PatternOutputRealizationDtoV5::ParametricPaths {
                curve_mechanism_id,
                style,
            } => toniator_domain::PatternOutputRealization::ParametricPaths {
                curve_mechanism_id: PatternMechanismId(curve_mechanism_id),
                style,
            },
            PatternOutputRealizationDtoV5::ConnectionPaths {
                site_mechanism_id,
                program,
                style,
            } => toniator_domain::PatternOutputRealization::ConnectionPaths {
                site_mechanism_id: PatternMechanismId(site_mechanism_id),
                program: program.into_domain(),
                style,
            },
            PatternOutputRealizationDtoV5::MazeWalls {
                site_mechanism_id,
                program,
                style,
            } => toniator_domain::PatternOutputRealization::MazeWalls {
                site_mechanism_id: PatternMechanismId(site_mechanism_id),
                program: program.into_domain(),
                style,
            },
            PatternOutputRealizationDtoV5::Regions { source } => {
                toniator_domain::PatternOutputRealization::Regions {
                    source: source.into_domain(),
                }
            }
        };
        toniator_domain::PatternOutputLayer {
            id: PatternOutputLayerId(self.id),
            source_filter,
            realization,
        }
    }
}

impl RegionSourceIntentDtoV5 {
    /// Serializes authored region intent without derived topology.
    fn from_domain(value: &RegionSourceIntent) -> Self {
        match value {
            RegionSourceIntent::VoronoiSites { site_mechanism_id } => Self::VoronoiSites {
                site_mechanism_id: site_mechanism_id.0,
            },
            RegionSourceIntent::GuideFaces {
                guide_mechanism_id,
                dimensions,
            } => Self::GuideFaces {
                guide_mechanism_id: guide_mechanism_id.0,
                dimensions: dimensions.iter().map(|dimension| dimension.0).collect(),
            },
        }
    }

    /// Restores authored region intent without accepting derived cells.
    fn into_domain(self) -> RegionSourceIntent {
        match self {
            Self::VoronoiSites { site_mechanism_id } => RegionSourceIntent::VoronoiSites {
                site_mechanism_id: PatternMechanismId(site_mechanism_id),
            },
            Self::GuideFaces {
                guide_mechanism_id,
                dimensions,
            } => RegionSourceIntent::GuideFaces {
                guide_mechanism_id: PatternMechanismId(guide_mechanism_id),
                dimensions: dimensions.into_iter().map(GuideDimensionId).collect(),
            },
        }
    }
}

impl ConnectionProgramDtoV4 {
    /// Serializes authored program intent without materializing a graph or path result.
    fn from_domain(value: &toniator_domain::ConnectionProgram) -> Self {
        use toniator_domain::ConnectionProgram;
        match value {
            ConnectionProgram::NearestLinks { adjacency } => Self::NearestLinks {
                adjacency: ConnectionAdjacencyIntentDtoV4::from_domain(*adjacency),
            },
            ConnectionProgram::RandomLinks {
                adjacency,
                minimum_degree,
                seed,
            } => Self::RandomLinks {
                adjacency: ConnectionAdjacencyIntentDtoV4::from_domain(*adjacency),
                minimum_degree: *minimum_degree,
                seed: *seed,
            },
            ConnectionProgram::GridSpanningTree {
                adjacency,
                algorithm,
                seed,
            } => Self::GridSpanningTree {
                adjacency: ConnectionAdjacencyIntentDtoV4::from_domain(*adjacency),
                algorithm: GridSpanningTreeAlgorithmDtoV4::from_domain(*algorithm),
                seed: *seed,
            },
        }
    }

    /// Rebuilds authored program intent without accepting derived state from persistent bytes.
    fn into_domain(self) -> toniator_domain::ConnectionProgram {
        use toniator_domain::ConnectionProgram;
        match self {
            Self::NearestLinks { adjacency } => ConnectionProgram::NearestLinks {
                adjacency: adjacency.into_domain(),
            },
            Self::RandomLinks {
                adjacency,
                minimum_degree,
                seed,
            } => ConnectionProgram::RandomLinks {
                adjacency: adjacency.into_domain(),
                minimum_degree,
                seed,
            },
            Self::GridSpanningTree {
                adjacency,
                algorithm,
                seed,
            } => ConnectionProgram::GridSpanningTree {
                adjacency: adjacency.into_domain(),
                algorithm: algorithm.into_domain(),
                seed,
            },
        }
    }
}

impl MazeProgramDtoV4 {
    /// Serializes only recursive-backtracker maze intent and its deterministic seed.
    fn from_domain(value: &MazeProgram) -> Self {
        Self {
            algorithm: GridMazeAlgorithmDtoV4::from_domain(value.algorithm),
            seed: value.seed,
        }
    }

    /// Rebuilds maze intent without accepting any derived arrangement or path state.
    fn into_domain(self) -> MazeProgram {
        MazeProgram {
            algorithm: self.algorithm.into_domain(),
            seed: self.seed,
        }
    }
}

impl ConnectionAdjacencyIntentDtoV4 {
    /// Serializes only finite authored adjacency controls.
    fn from_domain(value: toniator_domain::ConnectionAdjacencyIntent) -> Self {
        Self {
            maximum_degree: value.maximum_degree,
            maximum_distance: value.maximum_distance,
        }
    }
    /// Rebuilds unvalidated authored adjacency controls for the document validation boundary.
    fn into_domain(self) -> toniator_domain::ConnectionAdjacencyIntent {
        toniator_domain::ConnectionAdjacencyIntent {
            maximum_degree: self.maximum_degree,
            maximum_distance: self.maximum_distance,
        }
    }
}

impl GridMazeAlgorithmDtoV4 {
    /// Serializes the fixed maze algorithm contract.
    fn from_domain(value: toniator_domain::GridMazeAlgorithm) -> Self {
        match value {
            toniator_domain::GridMazeAlgorithm::RecursiveBacktracker => Self::RecursiveBacktracker,
        }
    }
    /// Rebuilds the fixed maze algorithm contract.
    fn into_domain(self) -> toniator_domain::GridMazeAlgorithm {
        match self {
            Self::RecursiveBacktracker => toniator_domain::GridMazeAlgorithm::RecursiveBacktracker,
        }
    }
}

impl GridSpanningTreeAlgorithmDtoV4 {
    /// Serializes the fixed spanning-tree algorithm contract.
    fn from_domain(value: toniator_domain::GridSpanningTreeAlgorithm) -> Self {
        match value {
            toniator_domain::GridSpanningTreeAlgorithm::RandomizedPrim => Self::RandomizedPrim,
        }
    }
    /// Rebuilds the fixed spanning-tree algorithm contract.
    fn into_domain(self) -> toniator_domain::GridSpanningTreeAlgorithm {
        match self {
            Self::RandomizedPrim => toniator_domain::GridSpanningTreeAlgorithm::RandomizedPrim,
        }
    }
}

impl ParametricCurveDtoV4 {
    /// Projects analytic parametric intent without serializing derived CurvePath geometry.
    fn from_domain(value: &ParametricCurve) -> Self {
        match value {
            ParametricCurve::Spiral(SpiralCurve {
                shape,
                turns,
                radial_spacing,
                phase_degrees,
                winding,
            }) => Self::Spiral {
                shape: match shape {
                    SpiralShape::Round => SpiralShapeDtoV4::Round,
                    SpiralShape::Square => SpiralShapeDtoV4::Square,
                },
                turns: *turns,
                radial_spacing: *radial_spacing,
                phase_degrees: *phase_degrees,
                winding: match winding {
                    CurveWinding::Clockwise => CurveWindingDtoV4::Clockwise,
                    CurveWinding::CounterClockwise => CurveWindingDtoV4::CounterClockwise,
                },
            },
        }
    }

    /// Rebuilds analytic parametric intent for later authoritative domain validation.
    fn into_domain(self) -> ParametricCurve {
        match self {
            Self::Spiral {
                shape,
                turns,
                radial_spacing,
                phase_degrees,
                winding,
            } => ParametricCurve::Spiral(SpiralCurve {
                shape: match shape {
                    SpiralShapeDtoV4::Round => SpiralShape::Round,
                    SpiralShapeDtoV4::Square => SpiralShape::Square,
                },
                turns,
                radial_spacing,
                phase_degrees,
                winding: match winding {
                    CurveWindingDtoV4::Clockwise => CurveWinding::Clockwise,
                    CurveWindingDtoV4::CounterClockwise => CurveWinding::CounterClockwise,
                },
            }),
        }
    }
}

impl StraightGuideDimensionDtoV4 {
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
impl MarkPrototypeDtoV4 {
    /// Serializes the exact persisted mark variant and its explicit resource reference.
    fn from_domain(value: &MarkPrototype) -> Self {
        match value {
            MarkPrototype::Circle => Self::Circle,
            MarkPrototype::AuthoredClosedShape { structure_id } => Self::AuthoredClosedShape {
                structure_id: structure_id.0,
            },
        }
    }

    /// Restores the exact persisted mark variant without resolving document resources.
    fn into_domain(self) -> MarkPrototype {
        match self {
            Self::Circle => MarkPrototype::Circle,
            Self::AuthoredClosedShape { structure_id } => MarkPrototype::AuthoredClosedShape {
                structure_id: toniator_domain::AuthoredStructureId(structure_id),
            },
        }
    }
}
impl MarkOrientationDtoV4 {
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
    /// Projects one retained legacy channel without serializing its derived effective pattern.
    fn from_domain(value: &ChannelState) -> Self {
        Self {
            id: value.id.0,
            pattern_instance: ChannelPatternInstanceDto::from_domain(&value.pattern_instance),
            appearance: AppearanceDto::from_domain(&value.appearance),
            source_mapping: LegacySourceMappingDto::from_domain(value.source_mapping),
        }
    }
    /// Rebuilds one retained legacy channel's authored instance and presentation state.
    fn into_domain(self) -> ChannelState {
        ChannelState {
            id: ChannelId(self.id),
            pattern_instance: self.pattern_instance.into_domain(),
            appearance: self.appearance.into_domain(),
            source_mapping: self.source_mapping.into_domain(),
        }
    }
}
impl ModeledChannelDto {
    /// Projects one modeled channel without serializing its derived effective pattern.
    fn from_domain(value: &ModeledChannelState) -> Self {
        Self {
            role: value.role.into(),
            id: value.id.0,
            pattern_instance: ChannelPatternInstanceDto::from_domain(&value.pattern_instance),
            mapping: SourceMappingDto::from_domain(value.mapping),
            paint: PaintDto::from_domain(&value.paint),
            visible: value.visible,
            opacity: value.opacity,
        }
    }
    /// Rebuilds one modeled channel's authored instance, mapping, paint, and presentation.
    fn into_domain(self) -> ModeledChannelState {
        ModeledChannelState {
            role: self.role.into(),
            id: ChannelId(self.id),
            pattern_instance: self.pattern_instance.into_domain(),
            mapping: self.mapping.into_domain(),
            paint: self.paint.into_domain(),
            visible: self.visible,
            opacity: self.opacity,
        }
    }
}
impl AppearanceDto {
    /// Projects retained legacy appearance values without pattern authority.
    fn from_domain(value: &ChannelAppearance) -> Self {
        Self {
            visible: value.visible,
            color: ColorDto::from_domain(&value.color),
            opacity: value.opacity,
        }
    }
    /// Rebuilds retained legacy appearance values for domain validation.
    fn into_domain(self) -> ChannelAppearance {
        ChannelAppearance {
            visible: self.visible,
            color: self.color.into_domain(),
            opacity: self.opacity,
        }
    }
}
impl MarkResponseDto {
    /// Projects the accepted mark-response branch without shape rotation.
    fn from_domain(value: &MarkGeometryResponse) -> Self {
        Self {
            minimum_fill: value.minimum_fill,
            maximum_fill: value.maximum_fill,
        }
    }
    /// Rebuilds the accepted mark-response branch for domain validation.
    fn into_domain(self) -> MarkGeometryResponse {
        MarkGeometryResponse {
            minimum_fill: self.minimum_fill,
            maximum_fill: self.maximum_fill,
        }
    }
}

impl ConnectedResponseDto {
    /// Projects the persisted normalized stroke response without resolving channel inheritance.
    fn from_domain(value: &ConnectedGeometryResponse) -> Self {
        Self {
            minimum_thickness: value.minimum_thickness,
            maximum_thickness: value.maximum_thickness,
        }
    }
    /// Rebuilds the connected response for domain-owned validation.
    fn into_domain(self) -> ConnectedGeometryResponse {
        ConnectedGeometryResponse {
            minimum_thickness: self.minimum_thickness,
            maximum_thickness: self.maximum_thickness,
        }
    }
}

impl ConnectedResponseDeltaDto {
    /// Projects optional additive stroke response intent without materializing effective values.
    fn from_domain(value: &ConnectedGeometryResponseDelta) -> Self {
        Self {
            minimum_thickness_delta: value.minimum_thickness_delta,
            maximum_thickness_delta: value.maximum_thickness_delta,
        }
    }
    /// Rebuilds optional additive stroke response intent.
    fn into_domain(self) -> ConnectedGeometryResponseDelta {
        ConnectedGeometryResponseDelta {
            minimum_thickness_delta: self.minimum_thickness_delta,
            maximum_thickness_delta: self.maximum_thickness_delta,
        }
    }
}

impl DocumentPatternSettingsDto {
    /// Projects document-wide layout authority without output response state.
    fn from_domain(value: &DocumentPatternSettings) -> Self {
        Self {
            definition_id: value.definition_id.0,
            density: DensityDto {
                across_x: value.density.across_x,
                across_y: value.density.across_y,
                aspect_locked: value.density.aspect_locked,
            },
            pattern_rotation_degrees: value.pattern_rotation_degrees,
            shape_rotation_degrees: value.shape_rotation_degrees,
        }
    }
    /// Rebuilds document-wide layout authority for complete validation.
    fn into_domain(self) -> DocumentPatternSettings {
        DocumentPatternSettings {
            definition_id: PatternDefinitionId(self.definition_id),
            density: DensityMetric2D {
                across_x: self.density.across_x,
                across_y: self.density.across_y,
                aspect_locked: self.density.aspect_locked,
            },
            pattern_rotation_degrees: self.pattern_rotation_degrees,
            shape_rotation_degrees: self.shape_rotation_degrees,
        }
    }
}
impl ChannelPatternInstanceDto {
    /// Projects optional replacement and additive intent without resolving inherited values.
    fn from_domain(value: &ChannelPatternInstance) -> Self {
        Self {
            definition_override: value.definition_override.map(|id| id.0),
            layout_delta: LayoutDeltaDto::from_domain(&value.layout_delta),
            shape_rotation_delta_degrees: value.shape_rotation_delta_degrees,
            output_response_deltas: value
                .output_response_deltas
                .iter()
                .map(|entry| PatternOutputResponseDeltaDtoV5 {
                    output_layer_id: entry.output_layer_id.0,
                    delta: ChannelGeometryResponseDeltaDto::from_domain(&entry.delta),
                })
                .collect(),
        }
    }
    /// Rebuilds optional replacement and additive intent without materializing inheritance.
    fn into_domain(self) -> ChannelPatternInstance {
        ChannelPatternInstance {
            definition_override: self.definition_override.map(PatternDefinitionId),
            layout_delta: self.layout_delta.into_domain(),
            shape_rotation_delta_degrees: self.shape_rotation_delta_degrees,
            output_response_deltas: self
                .output_response_deltas
                .into_iter()
                .map(|entry| PatternOutputResponseDelta {
                    output_layer_id: PatternOutputLayerId(entry.output_layer_id),
                    delta: entry.delta.into_domain(),
                })
                .collect(),
        }
    }
}
impl LayoutDeltaDto {
    /// Projects typed density and rotation deltas plus retained absolute translation.
    fn from_domain(value: &ChannelPatternLayoutDelta) -> Self {
        Self {
            density: value.density.as_ref().map(DensityDeltaDto::from_domain),
            rotation_degrees: value.rotation_degrees,
            translation_x: value.translation_x,
            translation_y: value.translation_y,
        }
    }
    /// Rebuilds typed density and rotation deltas plus retained absolute translation.
    fn into_domain(self) -> ChannelPatternLayoutDelta {
        ChannelPatternLayoutDelta {
            density: self.density.map(DensityDeltaDto::into_domain),
            rotation_degrees: self.rotation_degrees,
            translation_x: self.translation_x,
            translation_y: self.translation_y,
        }
    }
}
impl DensityDeltaDto {
    /// Projects both optional-authority density-axis deltas exactly.
    fn from_domain(value: &DensityMetricDelta2D) -> Self {
        Self {
            across_x_delta: value.across_x_delta,
            across_y_delta: value.across_y_delta,
        }
    }
    /// Rebuilds both density-axis deltas for later effective validation.
    fn into_domain(self) -> DensityMetricDelta2D {
        DensityMetricDelta2D {
            across_x_delta: self.across_x_delta,
            across_y_delta: self.across_y_delta,
        }
    }
}
impl MarkResponseDeltaDto {
    /// Projects independently optional mark-response field deltas exactly.
    fn from_domain(value: &MarkGeometryResponseDelta) -> Self {
        Self {
            minimum_fill_delta: value.minimum_fill_delta,
            maximum_fill_delta: value.maximum_fill_delta,
        }
    }
    /// Rebuilds independently optional mark-response field deltas for later effective validation.
    fn into_domain(self) -> MarkGeometryResponseDelta {
        MarkGeometryResponseDelta {
            minimum_fill_delta: self.minimum_fill_delta,
            maximum_fill_delta: self.maximum_fill_delta,
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

#[cfg(test)]
mod stage20p_tests {
    use super::*;

    /// Proves v5 persists only keyed guide-face authoring intent and no derived arrangement state.
    #[test]
    fn guide_face_source_round_trips_without_derived_regions() {
        let source = RegionSourceIntent::GuideFaces {
            guide_mechanism_id: PatternMechanismId(41),
            dimensions: vec![GuideDimensionId(5), GuideDimensionId(9)],
        };
        let dto = RegionSourceIntentDtoV5::from_domain(&source);
        assert!(matches!(&dto, RegionSourceIntentDtoV5::GuideFaces { .. }));
        assert_eq!(dto.into_domain(), source);
    }
}

#[cfg(test)]
mod stage20r_tests {
    use super::*;
    use toniator_domain::{ConnectionAdjacencyIntent, ConnectionProgram};

    /// Proves schema-v5 output records persist both reference-filter variants and stable IDs.
    #[test]
    fn output_filters_round_trip_as_authored_state() {
        for source_filter in [
            toniator_domain::SiteUseFilter::SitesUsedBy {
                output_layer_id: PatternOutputLayerId(7),
            },
            toniator_domain::SiteUseFilter::SitesUnusedBy {
                output_layer_id: PatternOutputLayerId(7),
            },
        ] {
            let layer = toniator_domain::PatternOutputLayer::new(
                PatternOutputLayerId(9),
                source_filter,
                toniator_domain::PatternOutputRealization::CircularMarks {
                    site_mechanism_id: PatternMechanismId(4),
                },
            );
            let dto = PatternOutputLayerDtoV4::from_domain(&layer);
            let json = serde_json::to_string(&dto).expect("output DTO serializes");
            assert!(json.contains("source_filter"));
            let decoded: PatternOutputLayerDtoV4 =
                serde_json::from_str(&json).expect("output DTO decodes");
            assert_eq!(decoded.into_domain(), layer);
        }
    }

    /// Proves current schema records reject an omitted filter instead of silently adapting it to All.
    #[test]
    fn output_record_rejects_missing_filter() {
        let malformed = r#"{
            "id": 9,
            "realization": {"kind":"circular_marks","site_mechanism_id":4}
        }"#;
        assert!(serde_json::from_str::<PatternOutputLayerDtoV4>(malformed).is_err());
    }

    /// Proves preset-v3 filter references remain ID-free recipe-local indices.
    #[test]
    fn preset_filter_reference_round_trips_without_document_ids() {
        let setting = PatternOutputSettingsRecipe {
            source_filter: SiteUseFilterRecipe::SitesUnusedBy { output_index: 0 },
            response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.0,
                maximum_fill: 1.0,
            }),
        };
        let dto = PatternOutputSettingsRecipeDtoV3 {
            source_filter: SiteUseFilterRecipeDtoV3::SitesUnusedBy { output_index: 0 },
            response: PatternGeometryResponseDto::from_domain(&setting.response),
        };
        let json = serde_json::to_string(&dto).expect("preset output setting serializes");
        assert!(json.contains("output_index"));
        assert!(!json.contains("output_layer_id"));
    }

    /// Proves preset-v3 persists one family plus ordered heterogeneous realizations and local filters.
    #[test]
    fn ordered_composite_preset_round_trips_deterministically() {
        let record = PresetRecord {
            metadata: PresetMetadata {
                id: "stage-20r-ordered".into(),
                name: "Stage 20R ordered".into(),
                category: "tests".into(),
                description: "ID-free ordered composite".into(),
                thumbnail: None,
            },
            recipe: PatternDefinitionRecipe {
                structure: PatternStructureRecipe::OrderedOutputs {
                    definition: Box::new(PatternStructureRecipe::StraightGrid(
                        PatternDefinitionDraft {
                            name: "ordered composite".into(),
                            coverage: CoveragePolicy {
                                guard_steps: 1,
                                additional_margin: 0.0,
                            },
                        },
                    )),
                    outputs: vec![
                        PatternOutputRealizationRecipe::Marks,
                        PatternOutputRealizationRecipe::ConnectionPaths {
                            program: ConnectionProgram::NearestLinks {
                                adjacency: ConnectionAdjacencyIntent {
                                    maximum_degree: 2,
                                    maximum_distance: 12.0,
                                },
                            },
                            style: PathStrokeStyle::default(),
                        },
                    ],
                },
                output_settings: vec![
                    PatternOutputSettingsRecipe {
                        source_filter: SiteUseFilterRecipe::SitesUnusedBy { output_index: 1 },
                        response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                            minimum_fill: 0.1,
                            maximum_fill: 0.8,
                        }),
                    },
                    PatternOutputSettingsRecipe {
                        source_filter: SiteUseFilterRecipe::All,
                        response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                            minimum_thickness: 0.05,
                            maximum_thickness: 0.2,
                        }),
                    },
                ],
            },
        };
        toniator_domain::validate_preset_record(&record).expect("ordered preset validates");
        let first = serde_json::to_vec_pretty(&PresetEnvelopeDto::from_domain(&record))
            .expect("ordered preset serializes");
        let envelope: PresetEnvelopeDto =
            serde_json::from_slice(&first).expect("ordered preset parses");
        let decoded = envelope.into_domain().expect("ordered preset decodes");
        toniator_domain::validate_preset_record(&decoded).expect("decoded preset validates");
        assert_eq!(decoded, record);
        assert_eq!(
            serde_json::to_vec_pretty(&PresetEnvelopeDto::from_domain(&decoded))
                .expect("decoded preset reserializes"),
            first
        );
        let text = String::from_utf8(first).expect("preset JSON is UTF-8");
        assert!(text.contains("ordered_outputs"));
        assert!(text.contains("output_index"));
        assert!(!text.contains("output_layer_id"));
    }
}
