//! Current-only XDG storage for personal preset-v4 and reusable authored geometry records.
//!
//! This module intentionally has no frontend dependency and never changes document, preset, or
//! container versions. Presets retain embedded geometry; resource files are optional editor input.

use std::{
    collections::BTreeSet,
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft, PersonalAuthoredResource,
    PersonalAuthoredResourceKind, PersonalResourceId, PresetRecord,
};

/// The current-only reusable authored-resource JSON envelope version.
pub const PERSONAL_RESOURCE_FORMAT_VERSION: u32 = 1;
/// The current-only active-library configuration JSON version.
pub const PERSONAL_LIBRARY_CONFIG_VERSION: u32 = 1;
/// The maximum accepted personal-library JSON file size.
pub const MAX_PERSONAL_LIBRARY_JSON_BYTES: u64 = 4 * 1024 * 1024;

const PRESET_EXTENSION: &str = ".preset.json";
const SHAPE_EXTENSION: &str = ".shape.json";
const MOTIF_EXTENSION: &str = ".motif.json";

/// Explicit XDG inputs used to resolve a personal-library default without touching process state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryEnvironment {
    pub data_home: Option<PathBuf>,
    pub config_home: Option<PathBuf>,
    pub home: Option<PathBuf>,
}

/// Canonical locations for one active personal library and its small configuration file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonalLibraryPaths {
    pub root: PathBuf,
    pub config_file: PathBuf,
}

/// Stable personal-library failure with a display-safe context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonalLibraryError {
    context: String,
}

impl PersonalLibraryError {
    /// Returns the stable filesystem, validation, or conflict context.
    pub fn context(&self) -> &str {
        &self.context
    }
}

impl fmt::Display for PersonalLibraryError {
    /// Formats the stable filesystem, validation, or conflict context.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.context)
    }
}

impl std::error::Error for PersonalLibraryError {}

/// An opaque fingerprint proving the exact current bytes observed before an update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonalLibraryFingerprint(String);

impl PersonalLibraryFingerprint {
    /// Returns the stable lowercase SHA-256 hex digest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One nonfatal discovered library problem that never prevents other entries from loading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonalLibraryWarning {
    pub path: PathBuf,
    pub message: String,
}

/// A valid personal preset and the bytes used to detect stale overwrite attempts.
#[derive(Clone, Debug, PartialEq)]
pub struct PersonalPresetSnapshot {
    pub preset: PresetRecord,
    pub fingerprint: PersonalLibraryFingerprint,
}

/// A valid personal resource and the bytes used to detect stale overwrite attempts.
#[derive(Clone, Debug, PartialEq)]
pub struct PersonalResourceSnapshot {
    pub resource: PersonalAuthoredResource,
    pub fingerprint: PersonalLibraryFingerprint,
}

/// The complete nonfatal active-library scan result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PersonalLibrarySnapshot {
    pub presets: Vec<PersonalPresetSnapshot>,
    pub shapes: Vec<PersonalResourceSnapshot>,
    pub motifs: Vec<PersonalResourceSnapshot>,
    pub warnings: Vec<PersonalLibraryWarning>,
}

/// A recoverable trash move that can be undone only while its destinations remain collision-free.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonalLibraryTrashToken {
    original: PathBuf,
    trashed: PathBuf,
    original_thumbnail: Option<PathBuf>,
    trashed_thumbnail: Option<PathBuf>,
}

/// Headless personal-library filesystem authority for exactly one canonical active root.
#[derive(Clone, Debug)]
pub struct PersonalLibrary {
    root: PathBuf,
    config_file: PathBuf,
}

impl PersonalLibraryPaths {
    /// Resolves default XDG locations from explicit environment inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when neither the relevant XDG path nor a usable home directory is supplied.
    pub fn default_for(environment: &LibraryEnvironment) -> Result<Self, PersonalLibraryError> {
        let data_base = environment.data_home.clone().or_else(|| {
            environment
                .home
                .as_ref()
                .map(|home| home.join(".local/share"))
        });
        let config_base = environment
            .config_home
            .clone()
            .or_else(|| environment.home.as_ref().map(|home| home.join(".config")));
        let Some(data_base) = data_base else {
            return Err(error("personal library requires XDG_DATA_HOME or HOME"));
        };
        let Some(config_base) = config_base else {
            return Err(error("personal library requires XDG_CONFIG_HOME or HOME"));
        };
        Ok(Self {
            root: data_base.join("Toniator"),
            config_file: config_base.join("Toniator").join("library.json"),
        })
    }
}

impl PersonalLibrary {
    /// Creates the required active-root layout and returns canonical paths for later safe access.
    ///
    /// # Errors
    ///
    /// Returns a stable path or filesystem error when the configured root cannot become a
    /// canonical absolute directory with the required private child directories.
    pub fn initialize(paths: PersonalLibraryPaths) -> Result<Self, PersonalLibraryError> {
        if !paths.root.is_absolute() || !paths.config_file.is_absolute() {
            return Err(error(
                "personal library root and config path must be absolute",
            ));
        }
        fs::create_dir_all(&paths.root).map_err(|source| io_error(&paths.root, source))?;
        let root = fs::canonicalize(&paths.root).map_err(|source| io_error(&paths.root, source))?;
        let library = Self {
            root,
            config_file: paths.config_file,
        };
        library.ensure_layout()?;
        Ok(library)
    }

    /// Opens the configured active root or initializes the supplied default root on first use.
    ///
    /// # Errors
    ///
    /// Returns a strict configuration or filesystem error without silently rewriting malformed
    /// configuration files.
    pub fn open_or_initialize(
        default_paths: PersonalLibraryPaths,
    ) -> Result<Self, PersonalLibraryError> {
        if path_exists(&default_paths.config_file)? {
            let bytes = read_bounded_regular_file(&default_paths.config_file)?;
            let config: ActiveLibraryConfig = serde_json::from_slice(&bytes)
                .map_err(|source| error(format!("invalid personal library config: {source}")))?;
            if config.version != PERSONAL_LIBRARY_CONFIG_VERSION {
                return Err(error(format!(
                    "unsupported personal library config version {}",
                    config.version
                )));
            }
            let root = PathBuf::from(config.active_root);
            if !root.is_absolute() {
                return Err(error("personal library config root must be absolute"));
            }
            let canonical = fs::canonicalize(&root).map_err(|source| io_error(&root, source))?;
            return Self::initialize(PersonalLibraryPaths {
                root: canonical,
                config_file: default_paths.config_file,
            });
        }
        Self::initialize(default_paths)
    }

    /// Returns the canonical active library root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Persists an explicitly chosen existing root without moving or deleting any library content.
    ///
    /// # Errors
    ///
    /// Rejects relative/nonexistent roots and invalid fixed-layout children before changing either
    /// configuration or the active in-memory root.
    pub fn switch_root(&mut self, root: &Path) -> Result<(), PersonalLibraryError> {
        if !root.is_absolute() {
            return Err(error("personal library root must be absolute"));
        }
        let canonical = fs::canonicalize(root).map_err(|source| io_error(root, source))?;
        if !canonical.is_dir() {
            return Err(error("personal library root must be a directory"));
        }
        let candidate = Self {
            root: canonical.clone(),
            config_file: self.config_file.clone(),
        };
        candidate.ensure_layout()?;
        let canonical_text = canonical
            .to_str()
            .ok_or_else(|| error("personal library root must be UTF-8 for configuration"))?
            .to_owned();
        let bytes = serde_json::to_vec_pretty(&ActiveLibraryConfig {
            version: PERSONAL_LIBRARY_CONFIG_VERSION,
            active_root: canonical_text,
        })
        .map_err(|source| error(source.to_string()))?;
        atomic_write(&self.config_file, &bytes, PublishGuard::Unchecked)?;
        self.root = canonical;
        Ok(())
    }

    /// Scans valid direct children and isolates malformed, obsolete, and conflicting files as warnings.
    pub fn scan(&self) -> Result<PersonalLibrarySnapshot, PersonalLibraryError> {
        self.ensure_layout()?;
        let mut snapshot = PersonalLibrarySnapshot::default();
        snapshot.presets = self.scan_presets(&mut snapshot.warnings)?;
        snapshot.shapes =
            self.scan_resources(PersonalAuthoredResourceKind::Shape, &mut snapshot.warnings)?;
        snapshot.motifs =
            self.scan_resources(PersonalAuthoredResourceKind::Motif, &mut snapshot.warnings)?;
        reject_duplicate_names(&mut snapshot.presets, &mut snapshot.warnings, "preset");
        reject_duplicate_resource_names(&mut snapshot.shapes, &mut snapshot.warnings, "shape");
        reject_duplicate_resource_names(&mut snapshot.motifs, &mut snapshot.warnings, "motif");
        Ok(snapshot)
    }

    /// Creates or replaces one validated personal preset after checking its observed fingerprint.
    ///
    /// # Errors
    ///
    /// Rejects noncanonical IDs, path escapes, stale replacements, and filesystem failures before
    /// publication. A `None` fingerprint is create-only and refuses an existing target.
    pub fn write_preset(
        &self,
        preset: &PresetRecord,
        expected: Option<&PersonalLibraryFingerprint>,
    ) -> Result<PersonalLibraryFingerprint, PersonalLibraryError> {
        validate_personal_preset(preset)?;
        self.ensure_unique_preset_name(preset)?;
        let target = self.preset_path(&preset.metadata.id)?;
        let bytes = serde_json::to_vec_pretty(&crate::PresetEnvelopeDto::from_domain(preset))
            .map_err(|source| error(source.to_string()))?;
        self.write_checked(&target, &bytes, expected)
    }

    /// Creates or replaces one validated personal resource after checking its observed fingerprint.
    ///
    /// # Errors
    ///
    /// Rejects noncanonical IDs, kind/path mismatch, stale replacements, and filesystem failures
    /// before publication. A `None` fingerprint is create-only and refuses an existing target.
    pub fn write_resource(
        &self,
        resource: &PersonalAuthoredResource,
        expected: Option<&PersonalLibraryFingerprint>,
    ) -> Result<PersonalLibraryFingerprint, PersonalLibraryError> {
        self.ensure_unique_resource_name(resource)?;
        let target = self.resource_path(resource.kind(), resource.id().as_str())?;
        let envelope = ResourceEnvelope::from_domain(resource);
        let bytes =
            serde_json::to_vec_pretty(&envelope).map_err(|source| error(source.to_string()))?;
        self.write_checked(&target, &bytes, expected)
    }

    /// Renames a personal preset or resource while preserving its stable ID and filename.
    ///
    /// # Errors
    ///
    /// Returns stale-write, duplicate-name, validation, or filesystem errors before publication.
    pub fn rename_resource(
        &self,
        resource: &PersonalAuthoredResource,
        expected: &PersonalLibraryFingerprint,
    ) -> Result<PersonalLibraryFingerprint, PersonalLibraryError> {
        self.ensure_unique_resource_name(resource)?;
        self.write_resource(resource, Some(expected))
    }

    /// Creates a caller-supplied duplicate personal preset with a fresh validated ID.
    ///
    /// # Errors
    ///
    /// Returns a duplicate-name, existing-ID, validation, or filesystem error without replacing
    /// any source record. The caller owns allocating the fresh canonical ID.
    pub fn duplicate_preset(
        &self,
        preset: &PresetRecord,
    ) -> Result<PersonalLibraryFingerprint, PersonalLibraryError> {
        self.write_preset(preset, None)
    }

    /// Creates a caller-supplied duplicate personal resource with a fresh validated ID.
    ///
    /// # Errors
    ///
    /// Returns a duplicate-name, existing-ID, validation, or filesystem error without replacing
    /// any source record. The caller owns allocating the fresh canonical ID.
    pub fn duplicate_resource(
        &self,
        resource: &PersonalAuthoredResource,
    ) -> Result<PersonalLibraryFingerprint, PersonalLibraryError> {
        self.write_resource(resource, None)
    }

    /// Renames a personal preset while preserving its stable ID and filename.
    ///
    /// # Errors
    ///
    /// Returns stale-write, duplicate-name, validation, or filesystem errors before publication.
    pub fn rename_preset(
        &self,
        preset: &PresetRecord,
        expected: &PersonalLibraryFingerprint,
    ) -> Result<PersonalLibraryFingerprint, PersonalLibraryError> {
        self.ensure_unique_preset_name(preset)?;
        self.write_preset(preset, Some(expected))
    }

    /// Moves one direct-child entry and its optional opaque thumbnail to private trash.
    ///
    /// # Errors
    ///
    /// Returns an error when the entry is unsafe, missing, or a trash collision would overwrite
    /// existing content. It never purges trash.
    pub fn trash(
        &self,
        kind: PersonalEntryKind,
        id: &str,
    ) -> Result<PersonalLibraryTrashToken, PersonalLibraryError> {
        let original = self.entry_path(kind, id)?;
        require_regular_direct_child(&original)?;
        let trash_name = original
            .file_name()
            .ok_or_else(|| error("personal library filename missing"))?;
        let trashed = self.trash_dir().join(trash_name);
        if path_exists(&trashed)? {
            return Err(error("personal library trash destination already exists"));
        }
        let thumbnail = self.thumbnail_path(kind, id)?;
        let (original_thumbnail, trashed_thumbnail) = if optional_regular_file(&thumbnail)? {
            let destination = self.trash_dir().join(
                thumbnail
                    .file_name()
                    .ok_or_else(|| error("personal library thumbnail filename missing"))?,
            );
            if path_exists(&destination)? {
                return Err(error(
                    "personal library thumbnail trash destination already exists",
                ));
            }
            (Some(thumbnail), Some(destination))
        } else {
            (None, None)
        };
        move_entry_and_thumbnail(
            &original,
            &trashed,
            original_thumbnail
                .as_deref()
                .zip(trashed_thumbnail.as_deref()),
        )?;
        Ok(PersonalLibraryTrashToken {
            original,
            trashed,
            original_thumbnail,
            trashed_thumbnail,
        })
    }

    /// Restores a prior trash move only when every original path is still free.
    ///
    /// # Errors
    ///
    /// Refuses collisions and leaves trash unchanged; never overwrites a newly created entry.
    pub fn undo_trash(&self, token: PersonalLibraryTrashToken) -> Result<(), PersonalLibraryError> {
        move_entry_and_thumbnail(
            &token.trashed,
            &token.original,
            token
                .trashed_thumbnail
                .as_deref()
                .zip(token.original_thumbnail.as_deref()),
        )
    }

    /// Ensures the active root has only the fixed current-library layout directories.
    fn ensure_layout(&self) -> Result<(), PersonalLibraryError> {
        for directory in [
            self.presets_dir(),
            self.shapes_dir(),
            self.motifs_dir(),
            self.thumbnails_dir(),
            self.trash_dir(),
        ] {
            ensure_private_directory(&directory)?;
        }
        Ok(())
    }

    /// Writes bytes only after verifying create-only or exact observed-fingerprint semantics.
    fn write_checked(
        &self,
        target: &Path,
        bytes: &[u8],
        expected: Option<&PersonalLibraryFingerprint>,
    ) -> Result<PersonalLibraryFingerprint, PersonalLibraryError> {
        match (path_exists(target)?, expected) {
            (true, Some(expected)) => {
                let actual = fingerprint_file(target)?;
                if &actual != expected {
                    return Err(error("personal library entry changed externally"));
                }
            }
            (true, None) => return Err(error("personal library entry already exists")),
            (false, Some(_)) => return Err(error("personal library entry no longer exists")),
            (false, None) => {}
        }
        atomic_write(
            target,
            bytes,
            expected
                .map(PublishGuard::MatchingFingerprint)
                .unwrap_or(PublishGuard::CreateOnly),
        )?;
        fingerprint_file(target)
    }

    /// Scans one preset directory without turning malformed siblings into a global failure.
    fn scan_presets(
        &self,
        warnings: &mut Vec<PersonalLibraryWarning>,
    ) -> Result<Vec<PersonalPresetSnapshot>, PersonalLibraryError> {
        let mut entries = Vec::new();
        for path in direct_children(&self.presets_dir(), warnings)? {
            if !matches_expected_name(&path, PRESET_EXTENSION) {
                warning(warnings, path, "unrecognized personal preset filename");
                continue;
            }
            match load_checked_preset(&path) {
                Ok((preset, fingerprint))
                    if toniator_domain::is_personal_library_id(&preset.metadata.id)
                        && path == self.preset_path(&preset.metadata.id)? =>
                {
                    entries.push(PersonalPresetSnapshot {
                        fingerprint,
                        preset,
                    })
                }
                Ok(_) => warning(
                    warnings,
                    path,
                    "personal preset ID does not match its direct-child filename",
                ),
                Err(message) => warning(warnings, path, message),
            }
        }
        entries.sort_by(|left, right| {
            left.preset
                .metadata
                .name
                .to_lowercase()
                .cmp(&right.preset.metadata.name.to_lowercase())
                .then_with(|| left.preset.metadata.id.cmp(&right.preset.metadata.id))
        });
        Ok(entries)
    }

    /// Scans one typed resource directory without turning malformed siblings into a global failure.
    fn scan_resources(
        &self,
        kind: PersonalAuthoredResourceKind,
        warnings: &mut Vec<PersonalLibraryWarning>,
    ) -> Result<Vec<PersonalResourceSnapshot>, PersonalLibraryError> {
        let directory = self.resource_dir(kind);
        let extension = extension_for(kind);
        let mut entries = Vec::new();
        for path in direct_children(&directory, warnings)? {
            if !matches_expected_name(&path, extension) {
                warning(warnings, path, "unrecognized personal resource filename");
                continue;
            }
            match load_checked_resource(&path, kind) {
                Ok((resource, fingerprint))
                    if path == self.resource_path(kind, resource.id().as_str())? =>
                {
                    entries.push(PersonalResourceSnapshot {
                        fingerprint,
                        resource,
                    })
                }
                Ok(_) => warning(
                    warnings,
                    path,
                    "personal resource ID does not match its direct-child filename",
                ),
                Err(message) => warning(warnings, path, message),
            }
        }
        entries.sort_by(|left, right| {
            left.resource
                .name()
                .to_lowercase()
                .cmp(&right.resource.name().to_lowercase())
                .then_with(|| {
                    left.resource
                        .id()
                        .as_str()
                        .cmp(right.resource.id().as_str())
                })
        });
        Ok(entries)
    }

    /// Rejects a case-insensitive duplicate personal name before a resource write.
    fn ensure_unique_resource_name(
        &self,
        resource: &PersonalAuthoredResource,
    ) -> Result<(), PersonalLibraryError> {
        let snapshot = self.scan()?;
        let group = match resource.kind() {
            PersonalAuthoredResourceKind::Shape => snapshot.shapes,
            PersonalAuthoredResourceKind::Motif => snapshot.motifs,
        };
        if group.iter().any(|entry| {
            entry.resource.id() != resource.id()
                && entry.resource.name().to_lowercase() == resource.name().to_lowercase()
        }) {
            return Err(error(
                "personal resource names must be case-insensitively unique within their kind",
            ));
        }
        Ok(())
    }

    /// Rejects a case-insensitive duplicate combined preset name before a preset write.
    fn ensure_unique_preset_name(&self, preset: &PresetRecord) -> Result<(), PersonalLibraryError> {
        let snapshot = self.scan()?;
        if snapshot.presets.iter().any(|entry| {
            entry.preset.metadata.id != preset.metadata.id
                && entry.preset.metadata.name.to_lowercase() == preset.metadata.name.to_lowercase()
        }) {
            return Err(error(
                "personal preset names must be case-insensitively unique",
            ));
        }
        Ok(())
    }

    /// Resolves one validated preset direct-child path.
    fn preset_path(&self, id: &str) -> Result<PathBuf, PersonalLibraryError> {
        self.direct_path(&self.presets_dir(), id, PRESET_EXTENSION)
    }
    /// Resolves one validated typed-resource direct-child path.
    fn resource_path(
        &self,
        kind: PersonalAuthoredResourceKind,
        id: &str,
    ) -> Result<PathBuf, PersonalLibraryError> {
        self.direct_path(&self.resource_dir(kind), id, extension_for(kind))
    }
    /// Resolves one non-authoritative opaque SVG thumbnail direct-child path.
    fn thumbnail_path(
        &self,
        kind: PersonalEntryKind,
        id: &str,
    ) -> Result<PathBuf, PersonalLibraryError> {
        self.direct_path(
            &self.thumbnails_dir(),
            id,
            match kind {
                PersonalEntryKind::Preset => ".preset.svg",
                PersonalEntryKind::Shape => ".shape.svg",
                PersonalEntryKind::Motif => ".motif.svg",
            },
        )
    }
    /// Resolves one typed entry direct-child path.
    fn entry_path(
        &self,
        kind: PersonalEntryKind,
        id: &str,
    ) -> Result<PathBuf, PersonalLibraryError> {
        match kind {
            PersonalEntryKind::Preset => self.preset_path(id),
            PersonalEntryKind::Shape => self.resource_path(PersonalAuthoredResourceKind::Shape, id),
            PersonalEntryKind::Motif => self.resource_path(PersonalAuthoredResourceKind::Motif, id),
        }
    }
    /// Resolves one path only after validating the opaque ID and fixed suffix.
    fn direct_path(
        &self,
        directory: &Path,
        id: &str,
        suffix: &str,
    ) -> Result<PathBuf, PersonalLibraryError> {
        if !toniator_domain::is_personal_library_id(id) {
            return Err(error(
                "personal library IDs must use the canonical user-<lowercase UUID> form",
            ));
        }
        Ok(directory.join(format!("{id}{suffix}")))
    }
    /// Returns the fixed preset directory.
    fn presets_dir(&self) -> PathBuf {
        self.root.join("presets")
    }
    /// Returns the fixed shapes directory.
    fn shapes_dir(&self) -> PathBuf {
        self.root.join("shapes")
    }
    /// Returns the fixed motifs directory.
    fn motifs_dir(&self) -> PathBuf {
        self.root.join("motifs")
    }
    /// Returns the fixed opaque-thumbnail directory.
    fn thumbnails_dir(&self) -> PathBuf {
        self.root.join("thumbnails")
    }
    /// Returns the fixed recoverable-trash directory.
    fn trash_dir(&self) -> PathBuf {
        self.root.join(".trash")
    }
    /// Returns the typed resource directory.
    fn resource_dir(&self, kind: PersonalAuthoredResourceKind) -> PathBuf {
        match kind {
            PersonalAuthoredResourceKind::Shape => self.shapes_dir(),
            PersonalAuthoredResourceKind::Motif => self.motifs_dir(),
        }
    }
}

/// One typed personal-library entry family accepted by trash and undo operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonalEntryKind {
    Preset,
    Shape,
    Motif,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveLibraryConfig {
    version: u32,
    active_root: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceEnvelope {
    resource_format_version: u32,
    id: String,
    name: String,
    kind: ResourceKindDto,
    segments: Vec<CurveSegmentDto>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ResourceKindDto {
    Shape,
    Motif,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CurveSegmentDto {
    Line {
        start: PointDto,
        end: PointDto,
    },
    CubicBezier {
        start: PointDto,
        control_1: PointDto,
        control_2: PointDto,
        end: PointDto,
    },
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PointDto {
    x: f64,
    y: f64,
}

impl ResourceEnvelope {
    /// Converts one validated reusable geometry record to the strict current file envelope.
    fn from_domain(resource: &PersonalAuthoredResource) -> Self {
        Self {
            resource_format_version: PERSONAL_RESOURCE_FORMAT_VERSION,
            id: resource.id().as_str().into(),
            name: resource.name().into(),
            kind: match resource.kind() {
                PersonalAuthoredResourceKind::Shape => ResourceKindDto::Shape,
                PersonalAuthoredResourceKind::Motif => ResourceKindDto::Motif,
            },
            segments: resource
                .draft()
                .segments()
                .iter()
                .map(CurveSegmentDto::from_domain)
                .collect(),
        }
    }
    /// Converts one strict current envelope to validated reusable geometry.
    fn into_domain(
        self,
        expected: PersonalAuthoredResourceKind,
    ) -> Result<PersonalAuthoredResource, String> {
        if self.resource_format_version != PERSONAL_RESOURCE_FORMAT_VERSION {
            return Err(format!(
                "unsupported personal resource format version {}",
                self.resource_format_version
            ));
        }
        let kind = match self.kind {
            ResourceKindDto::Shape => PersonalAuthoredResourceKind::Shape,
            ResourceKindDto::Motif => PersonalAuthoredResourceKind::Motif,
        };
        if kind != expected {
            return Err("personal resource kind does not match its directory".into());
        }
        let id = PersonalResourceId::new(self.id).map_err(|error| error.to_string())?;
        let draft = AuthoredStructureDraft::new(
            kind.structure_kind(),
            self.segments
                .into_iter()
                .map(CurveSegmentDto::into_domain)
                .collect(),
        )
        .map_err(|error| error.to_string())?;
        PersonalAuthoredResource::new(id, self.name, kind, draft).map_err(|error| error.to_string())
    }
}

impl CurveSegmentDto {
    /// Converts an exact authored segment into strict JSON data.
    fn from_domain(value: &AuthoredCurveSegment) -> Self {
        match value {
            AuthoredCurveSegment::Line { start, end } => Self::Line {
                start: PointDto::from_domain(*start),
                end: PointDto::from_domain(*end),
            },
            AuthoredCurveSegment::CubicBezier {
                start,
                control_1,
                control_2,
                end,
            } => Self::CubicBezier {
                start: PointDto::from_domain(*start),
                control_1: PointDto::from_domain(*control_1),
                control_2: PointDto::from_domain(*control_2),
                end: PointDto::from_domain(*end),
            },
        }
    }
    /// Converts strict JSON data into one exact authored segment.
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

impl PointDto {
    /// Converts one exact authored point into strict JSON data.
    fn from_domain(value: AuthoredPoint2) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }

    /// Converts strict JSON data into one exact authored point.
    fn into_domain(self) -> AuthoredPoint2 {
        AuthoredPoint2 {
            x: self.x,
            y: self.y,
        }
    }
}

/// Returns one fixed suffix for a typed personal geometry resource.
fn extension_for(kind: PersonalAuthoredResourceKind) -> &'static str {
    match kind {
        PersonalAuthoredResourceKind::Shape => SHAPE_EXTENSION,
        PersonalAuthoredResourceKind::Motif => MOTIF_EXTENSION,
    }
}
/// Builds one stable library error.
fn error(context: impl Into<String>) -> PersonalLibraryError {
    PersonalLibraryError {
        context: context.into(),
    }
}
/// Converts one filesystem error to the stable path-qualified boundary error.
fn io_error(path: &Path, source: std::io::Error) -> PersonalLibraryError {
    error(format!("{}: {source}", path.display()))
}

/// Creates one fixed library directory or rejects an existing symlink/non-directory child.
fn ensure_private_directory(path: &Path) -> Result<(), PersonalLibraryError> {
    if !path.exists() {
        fs::create_dir(path).map_err(|source| io_error(path, source))?;
    }
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(error(
            "personal library layout entries must be non-symlink directories",
        ));
    }
    Ok(())
}
/// Appends one nonfatal scan warning.
fn warning(warnings: &mut Vec<PersonalLibraryWarning>, path: PathBuf, message: impl Into<String>) {
    warnings.push(PersonalLibraryWarning {
        path,
        message: message.into(),
    });
}
/// Validates one personal preset record before it can enter the active library.
fn validate_personal_preset(preset: &PresetRecord) -> Result<(), PersonalLibraryError> {
    if !toniator_domain::is_personal_library_id(&preset.metadata.id) {
        return Err(error(
            "personal preset IDs must use the canonical user-<lowercase UUID> form",
        ));
    }
    toniator_domain::validate_preset_record(preset).map_err(|source| error(source.to_string()))
}
/// Reads one bounded regular JSON file through exactly one no-follow file handle.
///
/// # Errors
///
/// Rejects symlinks, nonregular files, read failures, and files whose actual byte stream exceeds
/// the configured limit. The returned bytes are the exact bytes later parsed by callers.
fn read_bounded_regular_file(path: &Path) -> Result<Vec<u8>, PersonalLibraryError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options
        .open(path)
        .map_err(|source| io_error(path, source))?;
    let metadata = file.metadata().map_err(|source| io_error(path, source))?;
    if !metadata.file_type().is_file() {
        return Err(error(
            "personal library entries must be regular non-symlink files",
        ));
    }
    let maximum_read = MAX_PERSONAL_LIBRARY_JSON_BYTES
        .checked_add(1)
        .ok_or_else(|| error("personal library JSON byte limit overflow"))?;
    let mut bytes =
        Vec::with_capacity(metadata.len().min(MAX_PERSONAL_LIBRARY_JSON_BYTES) as usize);
    Read::by_ref(&mut file)
        .take(maximum_read)
        .read_to_end(&mut bytes)
        .map_err(|source| io_error(path, source))?;
    if bytes.len() as u64 > MAX_PERSONAL_LIBRARY_JSON_BYTES {
        return Err(error("personal library JSON exceeds the 4 MiB limit"));
    }
    Ok(bytes)
}
/// Requires a regular non-symlink file whose parent is one existing directory.
fn require_regular_direct_child(path: &Path) -> Result<(), PersonalLibraryError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(error(
            "personal library entries must be regular non-symlink files",
        ));
    }
    Ok(())
}

/// Returns whether an optional thumbnail exists as a safe regular direct child.
///
/// # Errors
///
/// Rejects hostile symlinks and other nonregular thumbnail entries instead of treating them as
/// absent. A genuinely missing path returns `Ok(false)`.
fn optional_regular_file(path: &Path) -> Result<bool, PersonalLibraryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() && !metadata.file_type().is_symlink() => {
            Ok(true)
        }
        Ok(_) => Err(error(
            "personal library entries must be regular non-symlink files",
        )),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(io_error(path, source)),
    }
}
/// Lists regular direct children while isolating unsafe siblings as scan warnings.
fn direct_children(
    directory: &Path,
    warnings: &mut Vec<PersonalLibraryWarning>,
) -> Result<Vec<PathBuf>, PersonalLibraryError> {
    let mut children = Vec::new();
    for entry in fs::read_dir(directory).map_err(|source| io_error(directory, source))? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(source) => {
                warning(warnings, directory.to_path_buf(), source.to_string());
                continue;
            }
        };
        let path = entry.path();
        match fs::symlink_metadata(&path) {
            Ok(metadata)
                if metadata.file_type().is_file() && !metadata.file_type().is_symlink() =>
            {
                children.push(path)
            }
            Ok(_) => warning(
                warnings,
                path,
                "personal library entries must be regular non-symlink files",
            ),
            Err(source) => warning(warnings, path, source.to_string()),
        }
    }
    children.sort();
    Ok(children)
}
/// Checks that a direct child uses one exact stable ID plus fixed suffix.
fn matches_expected_name(path: &Path, suffix: &str) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| {
            name.strip_suffix(suffix)
                .is_some_and(toniator_domain::is_personal_library_id)
        })
}
/// Loads one safe current preset-v4 file and fingerprints the exact parsed bytes.
fn load_checked_preset(path: &Path) -> Result<(PresetRecord, PersonalLibraryFingerprint), String> {
    let bytes = read_bounded_regular_file(path).map_err(|error| error.to_string())?;
    let envelope: crate::PresetEnvelopeDto =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if envelope.preset_format_version != crate::PRESET_FORMAT_VERSION {
        return Err(format!(
            "unsupported preset format version {}",
            envelope.preset_format_version
        ));
    }
    let preset = envelope.into_domain().map_err(|error| error.to_string())?;
    toniator_domain::validate_preset_record(&preset).map_err(|error| error.to_string())?;
    Ok((preset, fingerprint_bytes(&bytes)))
}
/// Loads one safe current resource-v1 file and fingerprints the exact parsed bytes.
fn load_checked_resource(
    path: &Path,
    kind: PersonalAuthoredResourceKind,
) -> Result<(PersonalAuthoredResource, PersonalLibraryFingerprint), String> {
    let bytes = read_bounded_regular_file(path).map_err(|error| error.to_string())?;
    let resource = serde_json::from_slice::<ResourceEnvelope>(&bytes)
        .map_err(|error| error.to_string())?
        .into_domain(kind)?;
    Ok((resource, fingerprint_bytes(&bytes)))
}
/// Computes the exact current bytes fingerprint after enforcing safe regular-file rules.
fn fingerprint_file(path: &Path) -> Result<PersonalLibraryFingerprint, PersonalLibraryError> {
    let bytes = read_bounded_regular_file(path)?;
    Ok(fingerprint_bytes(&bytes))
}
/// Computes one opaque digest for bytes already read through a validated handle.
fn fingerprint_bytes(bytes: &[u8]) -> PersonalLibraryFingerprint {
    PersonalLibraryFingerprint(format!("{:x}", Sha256::digest(bytes)))
}
/// The target-state condition rechecked immediately before one atomic publication.
#[derive(Clone, Copy)]
enum PublishGuard<'a> {
    CreateOnly,
    MatchingFingerprint(&'a PersonalLibraryFingerprint),
    Unchecked,
}

/// Atomically writes one target through an adjacent restrictive temporary file.
///
/// The stale check is repeated immediately before rename. This remains a single-process writer
/// boundary: ordinary external changes are detected at the two checks, while simultaneous hostile
/// writers cannot receive a cross-process compare-and-swap guarantee from portable rename alone.
fn atomic_write(
    target: &Path,
    bytes: &[u8],
    guard: PublishGuard<'_>,
) -> Result<(), PersonalLibraryError> {
    let parent = target
        .parent()
        .ok_or_else(|| error("personal library target has no parent"))?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    for attempt in 0..1024_u32 {
        let temp = parent.join(format!(
            ".{}.tmp.{attempt}",
            target
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| error("personal library target must be UTF-8"))?
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            options.mode(0o600);
        }
        match options.open(&temp) {
            Ok(mut file) => {
                let result = (|| {
                    file.write_all(bytes)
                        .map_err(|source| io_error(&temp, source))?;
                    file.sync_all().map_err(|source| io_error(&temp, source))?;
                    verify_publish_target(target, guard)?;
                    fs::rename(&temp, target).map_err(|source| io_error(target, source))?;
                    sync_dir(parent)
                })();
                if result.is_err() {
                    let _ = fs::remove_file(&temp);
                }
                return result;
            }
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(io_error(&temp, source)),
        }
    }
    Err(error("personal library temporary filename space exhausted"))
}

/// Rechecks the intended target state immediately before atomic rename publication.
fn verify_publish_target(
    target: &Path,
    guard: PublishGuard<'_>,
) -> Result<(), PersonalLibraryError> {
    match guard {
        PublishGuard::CreateOnly if path_exists(target)? => {
            Err(error("personal library entry already exists"))
        }
        PublishGuard::CreateOnly => Ok(()),
        PublishGuard::Unchecked => {
            if path_exists(target)? {
                require_regular_direct_child(target)?;
            }
            Ok(())
        }
        PublishGuard::MatchingFingerprint(expected) => {
            let actual = fingerprint_file(target)
                .map_err(|_| error("personal library entry changed externally"))?;
            if &actual == expected {
                Ok(())
            } else {
                Err(error("personal library entry changed externally"))
            }
        }
    }
}

/// Moves one entry and optional qualified thumbnail as one recoverable two-file transaction.
///
/// # Errors
///
/// Preflights every source and destination before the primary move. If the thumbnail move fails,
/// it rolls the primary move back and synchronizes all affected directories before returning.
fn move_entry_and_thumbnail(
    source: &Path,
    destination: &Path,
    thumbnail: Option<(&Path, &Path)>,
) -> Result<(), PersonalLibraryError> {
    preflight_move(source, destination)?;
    if let Some((thumbnail_source, thumbnail_destination)) = thumbnail {
        preflight_move(thumbnail_source, thumbnail_destination)?;
    }
    fs::rename(source, destination).map_err(|source_error| io_error(source, source_error))?;
    if let Some((thumbnail_source, thumbnail_destination)) = thumbnail
        && let Err(move_error) = fs::rename(thumbnail_source, thumbnail_destination)
    {
        let rollback = fs::rename(destination, source)
            .map_err(|source_error| io_error(destination, source_error));
        let sync = sync_directories(&[
            source.parent(),
            destination.parent(),
            thumbnail_source.parent(),
            thumbnail_destination.parent(),
        ]);
        return match (rollback, sync) {
            (Ok(()), Ok(())) => Err(io_error(thumbnail_source, move_error)),
            (Err(rollback_error), _) => Err(error(format!(
                "{}; primary entry rollback failed: {rollback_error}",
                io_error(thumbnail_source, move_error)
            ))),
            (_, Err(sync_error)) => Err(error(format!(
                "{}; rollback directory sync failed: {sync_error}",
                io_error(thumbnail_source, move_error)
            ))),
        };
    }
    let directories = match thumbnail {
        Some((thumbnail_source, thumbnail_destination)) => vec![
            source.parent(),
            destination.parent(),
            thumbnail_source.parent(),
            thumbnail_destination.parent(),
        ],
        None => vec![source.parent(), destination.parent()],
    };
    sync_directories(&directories)
}

/// Validates one move pair before either source is changed.
fn preflight_move(source: &Path, destination: &Path) -> Result<(), PersonalLibraryError> {
    require_regular_direct_child(source)?;
    if path_exists(destination)? {
        return Err(error("personal library move destination already exists"));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| error("personal library move destination has no parent"))?;
    let metadata =
        fs::symlink_metadata(parent).map_err(|source_error| io_error(parent, source_error))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(error(
            "personal library move destination parent must be a non-symlink directory",
        ));
    }
    Ok(())
}

/// Synchronizes every unique affected source or destination directory after a paired move.
fn sync_directories(paths: &[Option<&Path>]) -> Result<(), PersonalLibraryError> {
    let mut directories = BTreeSet::new();
    for path in paths.iter().flatten() {
        directories.insert((*path).to_path_buf());
    }
    for directory in directories {
        sync_dir(&directory)?;
    }
    Ok(())
}

/// Returns whether a path entry exists without following a dangling symlink.
fn path_exists(path: &Path) -> Result<bool, PersonalLibraryError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(io_error(path, source)),
    }
}
/// Synchronizes one metadata directory after a rename publication.
fn sync_dir(path: &Path) -> Result<(), PersonalLibraryError> {
    File::open(path)
        .map_err(|source| io_error(path, source))?
        .sync_all()
        .map_err(|source| io_error(path, source))
}
/// Removes duplicate case-insensitive presets from an otherwise valid snapshot and records warnings.
fn reject_duplicate_names(
    entries: &mut Vec<PersonalPresetSnapshot>,
    warnings: &mut Vec<PersonalLibraryWarning>,
    label: &str,
) {
    let mut names = BTreeSet::new();
    entries.retain(|entry| {
        if names.insert(entry.preset.metadata.name.to_lowercase()) {
            true
        } else {
            warning(
                warnings,
                PathBuf::from(&entry.preset.metadata.id),
                format!("duplicate personal {label} name"),
            );
            false
        }
    });
}
/// Removes duplicate case-insensitive typed resources from an otherwise valid snapshot and records warnings.
fn reject_duplicate_resource_names(
    entries: &mut Vec<PersonalResourceSnapshot>,
    warnings: &mut Vec<PersonalLibraryWarning>,
    label: &str,
) {
    let mut names = BTreeSet::new();
    entries.retain(|entry| {
        if names.insert(entry.resource.name().to_lowercase()) {
            true
        } else {
            warning(
                warnings,
                PathBuf::from(entry.resource.id().as_str()),
                format!("duplicate personal {label} name"),
            );
            false
        }
    });
}
