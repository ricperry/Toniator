//! Source-free document Preset archives and guarded atomic publication.

use std::{
    fs::{self, File},
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

#[cfg(all(test, unix))]
use std::os::unix::fs::PermissionsExt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use toniator_domain::{Document, DocumentConfiguration, PendingRawPresetImport};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::{
    ChannelOutputResponseDto, ChannelResponseOverrideDto,
    DOCUMENT_PRESET_CONFIGURATION_SCHEMA_VERSION, DOCUMENT_PRESET_FORMAT_VERSION,
    DocumentAppearanceBaselineDto, DocumentConfigurationDtoV12, DocumentResponseSettingsDto,
    LoadError, MAX_ARCHIVE_BYTES, MAX_DOCUMENT_BYTES, RegionResizeAlgorithmDtoV6,
    RegionSamplingStrategyDtoV6, SaveError, SourceToneDto, declared_zip_entry_count,
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

/// A current Preset or a former raw-response Preset awaiting explicit engine preparation.
///
/// The pending variant does not expose its neutral calibration shell as a current configuration.
#[derive(Debug)]
pub enum StagedDocumentPreset {
    Current(DocumentConfiguration),
    PendingRaw(PendingRawPresetImport),
    /// Desired coverage is already current; only changed geometry companions need preparation.
    PendingCalibration(DocumentConfiguration),
}

#[derive(Serialize, Deserialize)]
struct DocumentPresetEnvelopeDto {
    kind: String,
    document_preset_format_version: u32,
    document_schema_version: u32,
    configuration: DocumentConfigurationDtoV12,
}

/// Minimal dispatch metadata parsed before the configuration schema is decoded.
#[derive(Deserialize)]
struct DocumentPresetMetadataDto {
    kind: String,
    document_preset_format_version: u32,
    document_schema_version: u32,
}

/// Exact pre-response-ownership endpoint payload, decoded only for recognized legacy Presets.
#[derive(Clone, Deserialize)]
#[serde(
    tag = "kind",
    content = "response",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum LegacyResponse {
    Marks {
        minimum_fill: f64,
        maximum_fill: f64,
    },
    Connected {
        minimum_thickness: f64,
        maximum_thickness: f64,
        bias: f64,
    },
    Regions {
        algorithm: RegionResizeAlgorithmDtoV6,
        sampling: RegionSamplingStrategyDtoV6,
        minimum_fill: f64,
        maximum_fill: f64,
    },
}

impl LegacyResponse {
    /// Moves old additive endpoint deltas into an effective named response and zeros their
    /// obsolete structural slots; connected bias remains Pattern-owned.
    ///
    /// # Errors
    /// Rejects malformed or mismatched legacy delta payloads rather than losing channel intent.
    fn absorb_delta(&self, delta: &mut Value) -> Result<Self, DocumentPresetError> {
        let kind = delta.get("kind").and_then(Value::as_str).map(str::to_owned);
        let fields = delta.get_mut("delta").and_then(Value::as_object_mut);
        let fields = fields.ok_or_else(|| DocumentPresetError::Json {
            context: "legacy response delta payload is missing".into(),
        })?;
        let mut take = |key: &str| -> Result<f64, DocumentPresetError> {
            let value = fields.get(key).filter(|value| !value.is_null()).cloned();
            let amount = match &value {
                Some(value) => value.as_f64().ok_or_else(|| DocumentPresetError::Json {
                    context: format!("legacy {key} must be numeric"),
                })?,
                None => 0.0,
            };
            fields.remove(key);
            Ok(amount)
        };
        match (self, kind.as_deref()) {
            (
                Self::Marks {
                    minimum_fill,
                    maximum_fill,
                },
                Some("marks"),
            ) => Ok(Self::Marks {
                minimum_fill: minimum_fill + take("minimum_fill_delta")?,
                maximum_fill: maximum_fill + take("maximum_fill_delta")?,
            }),
            (
                Self::Connected {
                    minimum_thickness,
                    maximum_thickness,
                    bias,
                },
                Some("connected"),
            ) => Ok(Self::Connected {
                minimum_thickness: minimum_thickness + take("minimum_thickness_delta")?,
                maximum_thickness: maximum_thickness + take("maximum_thickness_delta")?,
                bias: *bias,
            }),
            (
                Self::Regions {
                    algorithm,
                    sampling,
                    minimum_fill,
                    maximum_fill,
                },
                Some("regions"),
            ) => Ok(Self::Regions {
                algorithm: *algorithm,
                sampling: *sampling,
                minimum_fill: minimum_fill + take("minimum_fill_delta")?,
                maximum_fill: maximum_fill + take("maximum_fill_delta")?,
            }),
            _ => Err(DocumentPresetError::Json {
                context: "legacy response delta kind does not match output".into(),
            }),
        }
    }
    /// Moves one old effective endpoint pair into current ALL response authority.
    fn set_all(&self, all: &mut DocumentResponseSettingsDto) {
        match self {
            Self::Marks {
                minimum_fill,
                maximum_fill,
            } => {
                all.marks_minimum_fill = *minimum_fill;
                all.marks_maximum_fill = *maximum_fill;
            }
            Self::Connected {
                minimum_thickness,
                maximum_thickness,
                ..
            } => {
                all.connected_minimum_thickness = *minimum_thickness;
                all.connected_maximum_thickness = *maximum_thickness;
            }
            Self::Regions {
                minimum_fill,
                maximum_fill,
                sampling,
                ..
            } => {
                all.regions_minimum_fill = *minimum_fill;
                all.regions_maximum_fill = *maximum_fill;
                all.region_sampling = *sampling;
            }
        }
    }

    /// Returns only a named difference from ALL, preserving inherited channels as inherited.
    fn named_difference(
        &self,
        all: &DocumentResponseSettingsDto,
    ) -> Option<ChannelOutputResponseDto> {
        match self {
            Self::Marks {
                minimum_fill,
                maximum_fill,
            } if *minimum_fill != all.marks_minimum_fill
                || *maximum_fill != all.marks_maximum_fill =>
            {
                Some(ChannelOutputResponseDto::Marks {
                    minimum_fill: Some(*minimum_fill),
                    maximum_fill: Some(*maximum_fill),
                    density_mode: None,
                    size_scale: None,
                })
            }
            Self::Connected {
                minimum_thickness,
                maximum_thickness,
                ..
            } if *minimum_thickness != all.connected_minimum_thickness
                || *maximum_thickness != all.connected_maximum_thickness =>
            {
                Some(ChannelOutputResponseDto::Connected {
                    minimum_thickness: Some(*minimum_thickness),
                    maximum_thickness: Some(*maximum_thickness),
                })
            }
            Self::Regions {
                minimum_fill,
                maximum_fill,
                sampling,
                ..
            } if *minimum_fill != all.regions_minimum_fill
                || *maximum_fill != all.regions_maximum_fill
                || *sampling != all.region_sampling =>
            {
                Some(ChannelOutputResponseDto::Regions {
                    minimum_fill: Some(*minimum_fill),
                    maximum_fill: Some(*maximum_fill),
                    sampling: Some(*sampling),
                })
            }
            _ => None,
        }
    }

    /// Keeps only Pattern-owned connected bias or region resize in a current structural shell.
    fn structural_shell(&self) -> Value {
        match self {
            Self::Marks { .. } => json!({"kind":"marks"}),
            Self::Connected { bias, .. } => json!({"kind":"connected","bias":bias}),
            Self::Regions { algorithm, .. } => json!({"kind":"regions","algorithm":algorithm}),
        }
    }
}

/// Converts only explicitly recognized source-free Preset revisions into the current DTO shape.
///
/// # Errors
/// Rejects malformed legacy fields and unknown version pairs without guessing a repair.
fn migrate_legacy_preset(
    bytes: &[u8],
    metadata: &DocumentPresetMetadataDto,
) -> Result<Vec<u8>, DocumentPresetError> {
    match (
        metadata.document_preset_format_version,
        metadata.document_schema_version,
    ) {
        (1, 7 | 8) | (3, 10..=14) => {}
        _ => {
            return Err(DocumentPresetError::Version {
                context: format!(
                    "unsupported document Preset format/schema pair {}/{}",
                    metadata.document_preset_format_version, metadata.document_schema_version,
                ),
            });
        }
    }
    let mut root: Value =
        serde_json::from_slice(bytes).map_err(|error| DocumentPresetError::Json {
            context: error.to_string(),
        })?;
    let config = root
        .get_mut("configuration")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| DocumentPresetError::Json {
            context: "legacy configuration must be an object".into(),
        })?;
    let base_id = config
        .get("pattern_settings")
        .and_then(|value| value.get("definition_id"))
        .and_then(Value::as_u64)
        .ok_or_else(|| DocumentPresetError::Json {
            context: "legacy base Pattern ID is missing".into(),
        })?;
    let mut old_responses = BTreeMap::<(u64, u64), LegacyResponse>::new();
    let bundles = config
        .get_mut("pattern_definition_bundles")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| DocumentPresetError::Json {
            context: "legacy Pattern bundles are missing".into(),
        })?;
    for bundle in bundles {
        let id = bundle
            .get("definition")
            .and_then(|value| value.get("id"))
            .and_then(Value::as_u64)
            .ok_or_else(|| DocumentPresetError::Json {
                context: "legacy Pattern definition ID is missing".into(),
            })?;
        let settings = bundle
            .get_mut("output_settings")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| DocumentPresetError::Json {
                context: "legacy output settings are missing".into(),
            })?;
        for setting in settings {
            let output_id = setting
                .get("output_layer_id")
                .and_then(Value::as_u64)
                .ok_or_else(|| DocumentPresetError::Json {
                    context: "legacy output layer ID is missing".into(),
                })?;
            let response =
                setting
                    .get_mut("response")
                    .ok_or_else(|| DocumentPresetError::Json {
                        context: "legacy output response is missing".into(),
                    })?;
            if response.get("response").is_some() {
                let old: LegacyResponse =
                    serde_json::from_value(response.clone()).map_err(|error| {
                        DocumentPresetError::Json {
                            context: format!("legacy output response: {error}"),
                        }
                    })?;
                *response = old.structural_shell();
                old_responses.insert((id, output_id), old);
            }
        }
        if let Some(mechanisms) = bundle
            .get_mut("definition")
            .and_then(|definition| definition.get_mut("mechanisms"))
            .and_then(Value::as_array_mut)
        {
            for mechanism in mechanisms {
                let is_process =
                    mechanism.get("kind").and_then(Value::as_str) == Some("random_site_process");
                let is_product =
                    mechanism.get("kind").and_then(Value::as_str) == Some("random_site_product");
                if is_process && mechanism.get("nominal_spacing").is_none() {
                    let even = mechanism
                        .get("character")
                        .and_then(|value| value.get("kind"))
                        .and_then(Value::as_str)
                        == Some("even");
                    mechanism["nominal_spacing"] = if even {
                        Value::Null
                    } else {
                        json!(toniator_domain::DEFAULT_RANDOM_NOMINAL_SPACING)
                    };
                }
                if is_product && mechanism.get("refinement").is_none() {
                    mechanism["refinement"] =
                        json!({"enabled":false,"density_weighted":false,"iterations":4});
                }
            }
        }
    }
    if config.get("response_settings").is_none() {
        if old_responses.is_empty() {
            return Err(DocumentPresetError::Version {
                context: "legacy response ownership has no convertible endpoints".into(),
            });
        }
        let mut all = DocumentResponseSettingsDto::from_domain(
            &toniator_domain::DocumentResponseSettings::default(),
        );
        for ((id, _), response) in &old_responses {
            if *id == base_id {
                response.set_all(&mut all);
            }
        }
        let mut overrides = Vec::<ChannelResponseOverrideDto>::new();
        let channels = config
            .get_mut("channel_configuration")
            .and_then(|value| value.get_mut("channels"))
            .and_then(Value::as_array_mut)
            .ok_or_else(|| DocumentPresetError::Json {
                context: "legacy channels are missing".into(),
            })?;
        for channel in channels {
            let channel_id = channel.get("id").and_then(Value::as_u64).ok_or_else(|| {
                DocumentPresetError::Json {
                    context: "legacy channel ID is missing".into(),
                }
            })?;
            let definition_id = channel
                .get("pattern_instance")
                .and_then(|value| value.get("definition_override"))
                .and_then(Value::as_u64)
                .unwrap_or(base_id);
            let mut effective = old_responses
                .iter()
                .filter(|((id, _), _)| *id == definition_id)
                .map(|((_, output_id), response)| (*output_id, response.clone()))
                .collect::<BTreeMap<_, _>>();
            if let Some(deltas) = channel
                .get_mut("pattern_instance")
                .and_then(|instance| instance.get_mut("output_response_deltas"))
                .and_then(Value::as_array_mut)
            {
                for entry in deltas.iter_mut() {
                    let output_id = entry
                        .get("output_layer_id")
                        .and_then(Value::as_u64)
                        .ok_or_else(|| DocumentPresetError::Json {
                            context: "legacy response delta output ID is missing".into(),
                        })?;
                    let base =
                        effective
                            .get(&output_id)
                            .ok_or_else(|| DocumentPresetError::Json {
                                context: "legacy response delta has no matching output".into(),
                            })?;
                    let delta =
                        entry
                            .get_mut("delta")
                            .ok_or_else(|| DocumentPresetError::Json {
                                context: "legacy response delta is missing".into(),
                            })?;
                    effective.insert(output_id, base.absorb_delta(delta)?);
                }
                deltas.retain(|entry| {
                    entry
                        .get("delta")
                        .and_then(|delta| delta.get("delta"))
                        .and_then(Value::as_object)
                        .is_some_and(|fields| !fields.is_empty())
                });
            }
            for (output_id, response) in effective {
                if let Some(named) = response.named_difference(&all) {
                    overrides.push(ChannelResponseOverrideDto {
                        channel_id,
                        output_layer_id: output_id,
                        response: named,
                    });
                }
            }
            let needs_weighting = channel.get("weighting").is_none();
            if let Some(mapping) = channel.get_mut("mapping")
                && mapping.get("tone").is_none()
            {
                mapping["tone"] = serde_json::to_value(SourceToneDto::from_domain(
                    toniator_domain::SourceTone::identity(),
                ))
                .expect("identity source tone serializes");
            }
            if needs_weighting {
                let mapping =
                    channel
                        .get("mapping")
                        .cloned()
                        .ok_or_else(|| DocumentPresetError::Json {
                            context: "legacy modeled mapping is missing".into(),
                        })?;
                channel["weighting"] =
                    json!({"mapping":mapping,"strength":1.0,"response":{"kind":"linear"}});
            }
        }
        config.insert(
            "response_settings".into(),
            serde_json::to_value(all).expect("current response DTO serializes"),
        );
        config.insert(
            "channel_response_overrides".into(),
            serde_json::to_value(overrides).expect("current named response DTO serializes"),
        );
    }
    if config.get("appearance_baseline").is_none() {
        config.insert(
            "appearance_baseline".into(),
            serde_json::to_value(DocumentAppearanceBaselineDto::from_domain(
                &Default::default(),
            ))
            .expect("default baseline serializes"),
        );
    }
    config.entry("end_overrides").or_insert_with(|| json!([]));
    config
        .entry("saved_calibrations")
        .or_insert_with(|| json!([]));
    root["document_preset_format_version"] = json!(DOCUMENT_PRESET_FORMAT_VERSION);
    root["document_schema_version"] = json!(DOCUMENT_PRESET_CONFIGURATION_SCHEMA_VERSION);
    serde_json::to_vec(&root).map_err(|error| DocumentPresetError::Json {
        context: error.to_string(),
    })
}

/// Removes recognized obsolete reference, seed-specific, Guide Faces, and geometry measurements.
///
/// Pattern structure and every authored response remain unchanged; config16 desired endpoints
/// are never remapped. The receiver must prepare each missing binding before publication.
/// Current projects do not use this
/// converter, and unknown future profiles still fail strict artifact validation.
///
/// # Errors
/// Returns a JSON diagnostic if a recognized Preset cannot be reserialized within its limit.
fn migrate_former_calibration_profile(
    bytes: &[u8],
) -> Result<Option<Vec<u8>>, DocumentPresetError> {
    const FORMER_PROFILES: [&str; 3] = [
        "ton018-reference-fixed-canvas1024-v2",
        "ton018-reference-fixed-canvas1024-spacings32-v3",
        "ton018-reference-fixed-canvas1024-spacings64-v3",
    ];
    let mut root: Value =
        serde_json::from_slice(bytes).map_err(|error| DocumentPresetError::Json {
            context: error.to_string(),
        })?;
    let Some(configuration) = root.get_mut("configuration").and_then(Value::as_object_mut) else {
        return Ok(None);
    };
    let mut changed = false;
    if let Some(artifacts) = configuration
        .get_mut("saved_calibrations")
        .and_then(Value::as_array_mut)
    {
        let original_count = artifacts.len();
        artifacts.retain(|entry| {
            !entry
                .get("profile")
                .and_then(Value::as_str)
                .is_some_and(|profile| FORMER_PROFILES.contains(&profile))
                && !super::obsolete_seeded_v4_companion(entry)
                && !super::obsolete_guide_faces_v4_companion(entry)
                && !super::obsolete_geometry_companion(entry)
        });
        changed |= artifacts.len() != original_count;
    }
    if configuration
        .get("baseline_saved_calibration")
        .and_then(|entry| entry.get("profile"))
        .and_then(Value::as_str)
        .is_some_and(|profile| FORMER_PROFILES.contains(&profile))
        || configuration
            .get("baseline_saved_calibration")
            .is_some_and(super::obsolete_seeded_v4_companion)
        || configuration
            .get("baseline_saved_calibration")
            .is_some_and(super::obsolete_guide_faces_v4_companion)
        || configuration
            .get("baseline_saved_calibration")
            .is_some_and(super::obsolete_geometry_companion)
    {
        configuration.insert("baseline_saved_calibration".into(), Value::Null);
        changed = true;
    }
    if !changed {
        return Ok(None);
    }
    let migrated = serde_json::to_vec(&root).map_err(|error| DocumentPresetError::Json {
        context: error.to_string(),
    })?;
    if migrated.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(DocumentPresetError::Limits {
            context: "converted preset.json exceeds the 4 MiB document Preset limit".into(),
        });
    }
    Ok(Some(migrated))
}

/// Loads a source-free Preset with current coverage semantics or a current project as reusable
/// configuration, preserving destination print intent at binding.
///
/// Archive shape selects the reader, so renaming a project cannot bypass the
/// ordinary project's source manifest, byte-length, digest, reference, schema,
/// and container validation. Former raw-response Presets require staged
/// preparation; this current-only reader rejects them. Both routes bind
/// against `destination` before a configuration is returned.
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
    match load_document_preset_staged(path, destination)? {
        StagedDocumentPreset::Current(configuration) => Ok(configuration),
        StagedDocumentPreset::PendingRaw(_) | StagedDocumentPreset::PendingCalibration(_) => {
            Err(DocumentPresetError::Version {
                context: "Preset requires explicit calibration preparation".into(),
            })
        }
    }
}

/// Loads current configuration or typed former raw intent without publishing a neutral shell.
///
/// # Errors
/// Retains strict archive, schema, source, and destination validation diagnostics.
pub fn load_document_preset_staged(
    path: &Path,
    destination: &Document,
) -> Result<StagedDocumentPreset, DocumentPresetError> {
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
            Ok(StagedDocumentPreset::Current(configuration))
        }
        ArchiveKind::DocumentPreset => {
            let length = file
                .metadata()
                .map_err(|error| filesystem_error(path, error.to_string()))?
                .len();
            load_source_free_preset(file, length, destination)
        }
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

/// Serializes and atomically publishes a source-free v3 Preset snapshot using the current
/// configuration schema without project-owned print intent.
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
        document_schema_version: DOCUMENT_PRESET_CONFIGURATION_SCHEMA_VERSION,
        configuration: DocumentConfigurationDtoV12::from_configuration(configuration)
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
    #[cfg(windows)]
    {
        let mut staging = super::filesystem::Staging::create(path)
            .map_err(|error| filesystem_error(path, error.to_string()))?;
        write_preset_archive(
            staging
                .file()
                .map_err(|error| filesystem_error(path, error.to_string()))?,
            &json,
        )?;
        publish_windows_staging(
            &mut staging,
            path,
            &guard.state,
            super::filesystem::Staging::sync,
        )
    }
    #[cfg(unix)]
    {
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
}

/// Publishes retained Windows staging under the observed destination guard and reports later sync as a warning.
///
/// The caller supplies a fully synchronized archive. Fingerprints detect ordinary external changes;
/// they do not promise a hostile-writer compare-and-swap transaction.
/// # Errors
/// Returns stale-destination or native rename errors before publication; later sync never revokes publication.
#[cfg(windows)]
fn publish_windows_staging(
    staging: &mut super::filesystem::Staging,
    path: &Path,
    state: &DestinationState,
    sync: impl FnOnce(&super::filesystem::Staging) -> std::io::Result<()>,
) -> Result<DocumentPresetSaveOutcome, DocumentPresetError> {
    if let DestinationState::Existing(expected) = state {
        let actual = fingerprint_regular_file(path, MAX_DOCUMENT_BYTES).map_err(|_| {
            DocumentPresetError::StaleDestination {
                context: "document Preset destination changed externally".into(),
            }
        })?;
        if &actual != expected {
            return Err(DocumentPresetError::StaleDestination {
                context: "document Preset destination changed externally".into(),
            });
        }
    }
    staging
        .rename(match state {
            DestinationState::Missing => super::filesystem::PublishMode::NoReplace,
            DestinationState::Existing(_) => super::filesystem::PublishMode::Replace,
        })
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                DocumentPresetError::StaleDestination {
                    context: "document Preset destination now exists".into(),
                }
            } else {
                filesystem_error(path, error.to_string())
            }
        })?;
    Ok(DocumentPresetSaveOutcome {
        durability_warning: sync(staging).err().map(|error| error.to_string()),
    })
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
pub(super) fn load_source_free_preset<R: Read + Seek>(
    mut file: R,
    length: u64,
    destination: &Document,
) -> Result<StagedDocumentPreset, DocumentPresetError> {
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
    // Schema 16 already owns desired coverage; schema 15 retains the prior explicit raw route.
    let pending_raw = matches!(
        (
            metadata.document_preset_format_version,
            metadata.document_schema_version
        ),
        (1, 7 | 8) | (3, 10..=15)
    );
    let pending_calibration =
        metadata.document_preset_format_version == 3 && metadata.document_schema_version == 16;
    let migrated = if metadata.document_preset_format_version == DOCUMENT_PRESET_FORMAT_VERSION
        && metadata.document_schema_version == DOCUMENT_PRESET_CONFIGURATION_SCHEMA_VERSION
    {
        migrate_former_calibration_profile(&bytes)?
    } else if metadata.document_preset_format_version == DOCUMENT_PRESET_FORMAT_VERSION
        && matches!(metadata.document_schema_version, 15..=17)
    {
        let normalized = migrate_former_calibration_profile(&bytes)?.unwrap_or(bytes.clone());
        let mut root: Value =
            serde_json::from_slice(&normalized).map_err(|error| DocumentPresetError::Json {
                context: error.to_string(),
            })?;
        root["document_schema_version"] = json!(DOCUMENT_PRESET_CONFIGURATION_SCHEMA_VERSION);
        let normalized = serde_json::to_vec(&root).map_err(|error| DocumentPresetError::Json {
            context: error.to_string(),
        })?;
        if normalized.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(DocumentPresetError::Limits {
                context: "converted preset.json exceeds the 4 MiB document Preset limit".into(),
            });
        }
        Some(normalized)
    } else {
        Some(migrate_legacy_preset(&bytes, &metadata)?)
    };
    let artistic_migration = if metadata.document_schema_version < 17 {
        Some(migrate_artistic_geometry_defaults(
            migrated.as_deref().unwrap_or(&bytes),
        )?)
    } else {
        None
    };
    let previous = artistic_migration
        .as_deref()
        .or(migrated.as_deref())
        .unwrap_or(&bytes);
    let coverage_migration = if metadata.document_schema_version < 18 {
        Some(migrate_persistent_coverage_defaults(previous)?)
    } else {
        None
    };
    let decoded = coverage_migration.as_deref().unwrap_or(previous);
    let mut ignored = Vec::new();
    let mut deserializer = serde_json::Deserializer::from_slice(decoded);
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
        DOCUMENT_PRESET_FORMAT_VERSION
    );
    debug_assert_eq!(
        envelope.document_schema_version,
        DOCUMENT_PRESET_CONFIGURATION_SCHEMA_VERSION
    );
    if pending_raw {
        envelope
            .configuration
            .into_pending_raw_import(destination)
            .map(StagedDocumentPreset::PendingRaw)
            .map_err(|error| DocumentPresetError::DomainValidation {
                context: error.to_string(),
            })
    } else if pending_calibration {
        envelope
            .configuration
            .into_domain_configuration(destination)
            .map(StagedDocumentPreset::PendingCalibration)
            .map_err(|error| DocumentPresetError::DomainValidation {
                context: error.to_string(),
            })
    } else {
        envelope
            .configuration
            .into_domain_configuration(destination)
            .map(StagedDocumentPreset::Current)
            .map_err(|error| DocumentPresetError::DomainValidation {
                context: error.to_string(),
            })
    }
}

/// Makes lost finite-extent intent explicit only for recognized former source-free configurations.
/// Stored generic recipes retain their policy; missing mechanism policy becomes Fixed without inference.
/// Original bytes, artwork-independent settings, tone controls and valid Fixed measurements remain intact.
/// # Errors
/// Returns bounded JSON diagnostics without modifying the selected archive.
fn migrate_persistent_coverage_defaults(bytes: &[u8]) -> Result<Vec<u8>, DocumentPresetError> {
    /// Adds only the required mechanism field and never rewrites existing authored recipe policy.
    fn insert_fixed(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if object.get("kind").and_then(Value::as_str) == Some("parametric_curve_source") {
                    object
                        .entry("spiral_coverage")
                        .or_insert_with(|| json!("fixed"));
                }
                for child in object.values_mut() {
                    insert_fixed(child);
                }
            }
            Value::Array(values) => {
                for child in values {
                    insert_fixed(child);
                }
            }
            _ => {}
        }
    }
    let mut root: Value =
        serde_json::from_slice(bytes).map_err(|error| DocumentPresetError::Json {
            context: error.to_string(),
        })?;
    let configuration = root
        .get_mut("configuration")
        .ok_or_else(|| DocumentPresetError::Json {
            context: "Preset configuration is missing".into(),
        })?;
    insert_fixed(configuration);
    let result = serde_json::to_vec(&root).map_err(|error| DocumentPresetError::Json {
        context: error.to_string(),
    })?;
    if result.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(DocumentPresetError::Limits {
            context: "converted coverage-intent Preset exceeds size limit".into(),
        });
    }
    Ok(result)
}

/// Adds approved new defaults only to recognized former source-free Preset configurations.
/// Affected numeric artifacts are removed without changing desired Start/End coverage or donors
/// for unrelated mark/region geometry. Ordinary project and Pattern decoders never call this.
/// # Errors
/// Returns JSON/size diagnostics without changing the original archive bytes.
fn migrate_artistic_geometry_defaults(bytes: &[u8]) -> Result<Vec<u8>, DocumentPresetError> {
    /// Identifies artifacts whose typed output construction changes at this schema transition.
    fn affected(value: &Value) -> bool {
        match value {
            Value::Object(object) => {
                matches!(
                    object.get("kind").and_then(Value::as_str),
                    Some(
                        "guide_paths"
                            | "parametric_paths"
                            | "structural_paths"
                            | "curve_motif_paths"
                            | "connection_paths"
                    )
                ) || object.values().any(affected)
            }
            Value::Array(values) => values.iter().any(affected),
            _ => false,
        }
    }
    /// Inserts only required current intent fields, leaving every authored former field intact.
    fn insert_defaults(value: &mut Value) {
        match value {
            Value::Object(object) => {
                match object.get("kind").and_then(Value::as_str) {
                    Some(
                        "guide_paths" | "parametric_paths" | "structural_paths"
                        | "curve_motif_paths",
                    ) => {
                        object.insert("tone_sample_spacing".into(), json!({"multiplier":1.0}));
                    }
                    Some("connection_paths") => {
                        object.insert("selection".into(), json!({"maximum_final_degree":null,"cycles":"allow","minimum_component_points":1}));
                    }
                    _ => {}
                }
                for child in object.values_mut() {
                    insert_defaults(child);
                }
            }
            Value::Array(values) => {
                for child in values {
                    insert_defaults(child);
                }
            }
            _ => {}
        }
    }
    let mut root: Value =
        serde_json::from_slice(bytes).map_err(|error| DocumentPresetError::Json {
            context: error.to_string(),
        })?;
    let configuration = root
        .get_mut("configuration")
        .ok_or_else(|| DocumentPresetError::Json {
            context: "Preset configuration is missing".into(),
        })?;
    if let Some(entries) = configuration
        .get_mut("saved_calibrations")
        .and_then(Value::as_array_mut)
    {
        entries.retain(|entry| !affected(entry));
    }
    if configuration
        .get("baseline_saved_calibration")
        .is_some_and(affected)
    {
        configuration["baseline_saved_calibration"] = Value::Null;
    }
    insert_defaults(configuration);
    let result = serde_json::to_vec(&root).map_err(|error| DocumentPresetError::Json {
        context: error.to_string(),
    })?;
    if result.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(DocumentPresetError::Limits {
            context: "converted artistic geometry Preset exceeds size limit".into(),
        });
    }
    Ok(result)
}

/// Opens one path once without following a final symlink and enforces a real byte limit.
///
/// # Errors
///
/// Rejects symlinks, nonregular files, metadata/read-open failures, and files
/// whose metadata length exceeds `limit`.
fn open_regular_file(path: &Path, limit: u64) -> Result<File, DocumentPresetError> {
    #[cfg(unix)]
    let file = {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        options
            .open(path)
            .map_err(|error| filesystem_error(path, error.to_string()))?
    };
    #[cfg(windows)]
    let file = super::filesystem::open_regular_file(path)
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
#[cfg(unix)]
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
#[cfg(unix)]
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
#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), DocumentPresetError> {
    super::filesystem::sync_directory(path)
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

    /// Encodes a disposable in-memory ZIP through the same writer used for ordinary source-free files.
    ///
    /// # Panics
    /// Panics if a bounded test archive cannot be encoded.
    fn memory_archive(json: &[u8], extra: bool) -> Vec<u8> {
        let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file(PRESET_ENTRY_NAME, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(json).unwrap();
        if extra {
            writer
                .start_file("source.png", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"forbidden embedded source").unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    /// Checks embedded Cursor decoding shares topology, nested-field, trailing-JSON and size rejection.
    ///
    /// # Panics
    /// Panics if the current embedded archive cannot decode or a malformed archive is accepted.
    #[test]
    fn embedded_reader_retains_strict_file_validation() {
        let destination = Document::new_default_document(
            toniator_domain::CanvasSpec {
                width: 900.0,
                height: 620.0,
            },
            toniator_domain::SourceReference::Unassigned,
        )
        .unwrap();
        let builtin = crate::builtin_document_presets().first().unwrap();
        let mut archive = ZipArchive::new(std::io::Cursor::new(builtin.archive_bytes())).unwrap();
        let json = read_limited(&mut archive, 0, MAX_DOCUMENT_BYTES, PRESET_ENTRY_NAME).unwrap();
        let extra = memory_archive(&json, true);
        assert!(matches!(
            load_source_free_preset(
                std::io::Cursor::new(&extra),
                extra.len() as u64,
                &destination
            ),
            Err(DocumentPresetError::EntryTopology { .. })
        ));
        let mut unknown: Value = serde_json::from_slice(&json).unwrap();
        unknown["configuration"]["unexpected"] = json!(true);
        let unknown = memory_archive(&serde_json::to_vec(&unknown).unwrap(), false);
        assert!(matches!(
            load_source_free_preset(
                std::io::Cursor::new(&unknown),
                unknown.len() as u64,
                &destination
            ),
            Err(DocumentPresetError::Json { .. })
        ));
        let mut trailing = json;
        trailing.extend_from_slice(b" {}");
        let trailing = memory_archive(&trailing, false);
        assert!(matches!(
            load_source_free_preset(
                std::io::Cursor::new(&trailing),
                trailing.len() as u64,
                &destination
            ),
            Err(DocumentPresetError::Json { .. })
        ));
        assert!(matches!(
            load_source_free_preset(
                std::io::Cursor::new(builtin.archive_bytes()),
                MAX_DOCUMENT_BYTES + 1,
                &destination
            ),
            Err(DocumentPresetError::Limits { .. })
        ));
    }

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

    /// Keeps published Preset bytes and returns a warning when native directory durability fails after rename.
    /// # Panics
    /// Panics if postpublication sync is fatal, cleanup removes the target, or a staging sibling remains.
    #[cfg(windows)]
    #[test]
    fn published_preset_retains_bytes_on_durability_warning() {
        let root = std::env::temp_dir().join(format!(
            "toniator-preset-{}",
            super::super::filesystem::random_suffix().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let path = root.join("warning.toniator-preset");
        let mut staging = super::super::filesystem::Staging::create(&path).unwrap();
        let mut file = staging.file().unwrap();
        file.write_all(b"published preset").unwrap();
        file.sync_all().unwrap();
        drop(file);
        let result =
            publish_windows_staging(&mut staging, &path, &DestinationState::Missing, |_| {
                Err(std::io::Error::other("injected directory sync failure"))
            })
            .unwrap();
        assert_eq!(
            result.durability_warning.as_deref(),
            Some("injected directory sync failure")
        );
        drop(staging);
        assert_eq!(fs::read(&path).unwrap(), b"published preset");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
