//! Filesystem-backed lifecycle inputs for declarative pattern definitions.
//!
//! This module deliberately stops before UI selection, document mutation, and
//! rendering.  It turns the XDG user library and already-persisted project
//! embeddings into deterministic inputs for the existing layered registry.

use crate::{
    DefinitionParameterScope, OutputChannelId, PatternDefinition, PatternDefinitionRegistry,
    PatternDefinitionRegistryError, PatternDefinitionSource, PatternDocumentState, PatternId,
    PatternInstanceParameters, PatternSelection, ResolvedPatternDefinition,
    load_bundled_pattern_definition_registry, parse_tnpattern, serialize_tnpattern,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const USER_PATTERN_LIBRARY_RELATIVE_PATH: &str = "toniator/patterns";

/// A current-schema external definition that has been fully parsed before any
/// user-library destination is considered.
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalPatternDefinitionImport {
    pub source_path: PathBuf,
    pub definition: PatternDefinition,
}

/// A stable-ID import decision prepared from an already-loaded resolver. The
/// UI must obtain an explicit choice for either conflict form; no plan mutates
/// the library by itself.
#[derive(Debug, Clone, PartialEq)]
pub enum ExternalPatternDefinitionImportPlan {
    Ready {
        import: ExternalPatternDefinitionImport,
        destination: PathBuf,
    },
    Identical {
        import: ExternalPatternDefinitionImport,
        existing_source: PatternDefinitionSource,
    },
    UserLibraryConflict {
        import: ExternalPatternDefinitionImport,
        /// Deterministically ordered direct user-library paths carrying this
        /// stable ID. Replacement is available only when this has one member.
        matching_paths: Vec<PathBuf>,
    },
    ProtectedConflict {
        import: ExternalPatternDefinitionImport,
        source: PatternDefinitionSource,
    },
}

impl ExternalPatternDefinitionImportPlan {
    pub fn import(&self) -> &ExternalPatternDefinitionImport {
        match self {
            Self::Ready { import, .. }
            | Self::Identical { import, .. }
            | Self::UserLibraryConflict { import, .. }
            | Self::ProtectedConflict { import, .. } => import,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPatternDefinitionImportChoice {
    WriteNew,
    ReplaceUserLibrary,
    ImportAsNewCustomId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternalPatternDefinitionImportCommit {
    pub definition: PatternDefinition,
    pub destination: PathBuf,
    pub replaced_user_library_file: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalPatternDefinitionImportError {
    SourceRead {
        path: PathBuf,
        message: String,
    },
    InvalidSource {
        path: PathBuf,
        message: String,
    },
    InvalidChoice {
        id: PatternId,
        choice: ExternalPatternDefinitionImportChoice,
    },
    NewCustomId {
        id: PatternId,
        message: String,
    },
    Write(PatternDefinitionLifecycleError),
}

impl fmt::Display for ExternalPatternDefinitionImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceRead { path, message } => {
                write!(
                    formatter,
                    "could not read external .tnpattern {}: {message}",
                    path.display()
                )
            }
            Self::InvalidSource { path, message } => write!(
                formatter,
                "external .tnpattern {} is not a current valid definition: {message}",
                path.display()
            ),
            Self::InvalidChoice { id, choice } => {
                write!(
                    formatter,
                    "import choice {choice:?} is not available for {id}"
                )
            }
            Self::NewCustomId { id, message } => {
                write!(
                    formatter,
                    "could not allocate a new custom ID for {id}: {message}"
                )
            }
            Self::Write(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ExternalPatternDefinitionImportError {}

/// Resolves the XDG data location used for user-authored `.tnpattern` files.
///
/// The explicit inputs make the policy testable.  Native callers should use
/// [`native_user_pattern_library_dir`].
pub fn user_pattern_library_dir(data_home: Option<&Path>, home: Option<&Path>) -> PathBuf {
    data_home
        .map(Path::to_path_buf)
        .or_else(|| home.map(|path| path.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"))
        .join(USER_PATTERN_LIBRARY_RELATIVE_PATH)
}

/// Resolves the native XDG data location without creating it.
pub fn native_user_pattern_library_dir() -> PathBuf {
    user_pattern_library_dir(
        std::env::var_os("XDG_DATA_HOME").as_deref().map(Path::new),
        std::env::var_os("HOME").as_deref().map(Path::new),
    )
}

/// One valid definition discovered in the user library.
#[derive(Debug, Clone, PartialEq)]
pub struct UserPatternLibraryEntry {
    pub path: PathBuf,
    pub definition: PatternDefinition,
}

/// A non-fatal file-level problem discovered while scanning the user library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserPatternLibraryDiagnostic {
    pub path: PathBuf,
    pub message: String,
}

/// A deterministic snapshot of one user pattern directory.
#[derive(Debug, Clone, PartialEq)]
pub struct UserPatternLibrarySnapshot {
    directory: PathBuf,
    entries: Vec<UserPatternLibraryEntry>,
    diagnostics: Vec<UserPatternLibraryDiagnostic>,
}

impl UserPatternLibrarySnapshot {
    /// Discovers direct `.tnpattern` files in bytewise path order. Missing user
    /// library directories are empty by design; malformed or unreadable files
    /// remain visible as diagnostics while valid neighboring definitions load.
    pub fn load(directory: impl Into<PathBuf>) -> Result<Self, PatternDefinitionLifecycleError> {
        let directory = directory.into();
        let read_dir = match fs::read_dir(&directory) {
            Ok(read_dir) => read_dir,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    directory,
                    entries: Vec::new(),
                    diagnostics: Vec::new(),
                });
            }
            Err(error) => {
                return Err(PatternDefinitionLifecycleError::UserLibraryDirectory {
                    path: directory,
                    message: error.to_string(),
                });
            }
        };

        let mut paths = Vec::new();
        for entry in read_dir {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if is_tnpattern_path(&path) {
                        paths.push(path);
                    }
                }
                Err(error) => {
                    return Err(PatternDefinitionLifecycleError::UserLibraryDirectory {
                        path: directory,
                        message: error.to_string(),
                    });
                }
            }
        }
        paths.sort();

        let mut entries = Vec::new();
        let mut diagnostics = Vec::new();
        for path in paths {
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    diagnostics.push(UserPatternLibraryDiagnostic {
                        path,
                        message: format!("could not read .tnpattern: {error}"),
                    });
                    continue;
                }
            };
            match parse_tnpattern(&bytes) {
                Ok(definition) => entries.push(UserPatternLibraryEntry { path, definition }),
                Err(error) => diagnostics.push(UserPatternLibraryDiagnostic {
                    path,
                    message: error.to_string(),
                }),
            }
        }

        Ok(Self {
            directory,
            entries,
            diagnostics,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Valid entries remain in deterministic filesystem-path order.
    pub fn entries(&self) -> &[UserPatternLibraryEntry] {
        &self.entries
    }

    /// File diagnostics remain in deterministic filesystem-path order.
    pub fn diagnostics(&self) -> &[UserPatternLibraryDiagnostic] {
        &self.diagnostics
    }

    pub fn reload(&mut self) -> Result<(), PatternDefinitionLifecycleError> {
        *self = Self::load(self.directory.clone())?;
        Ok(())
    }
}

/// Inputs required to reconstruct one resolver after a document reopen.
#[derive(Debug, Clone, PartialEq)]
pub struct DefinitionLifecycleInputs {
    pub user_library: UserPatternLibrarySnapshot,
    pub project_embedded: Vec<PatternDefinition>,
}

impl DefinitionLifecycleInputs {
    pub fn from_document(
        user_library: UserPatternLibrarySnapshot,
        pattern_state: &PatternDocumentState,
    ) -> Self {
        Self {
            user_library,
            project_embedded: pattern_state
                .embedded_patterns
                .values()
                .map(|embedded| embedded.definition.clone())
                .collect(),
        }
    }
}

/// Typed recovery context when a selected stable ID cannot be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingPatternDefinitionDiagnostic {
    pub requested_id: PatternId,
    /// Stable-ID sorted candidates; this is presentation-neutral recovery
    /// input and deliberately does not choose a replacement automatically.
    pub available_ids: Vec<PatternId>,
}

/// Application-level lifecycle resolution built on the immutable registry.
#[derive(Debug)]
pub struct PatternDefinitionLifecycleResolver {
    user_library: UserPatternLibrarySnapshot,
    registry: PatternDefinitionRegistry,
}

impl PatternDefinitionLifecycleResolver {
    pub fn load_for_document(
        directory: impl Into<PathBuf>,
        pattern_state: &PatternDocumentState,
    ) -> Result<Self, PatternDefinitionLifecycleError> {
        let user_library = UserPatternLibrarySnapshot::load(directory)?;
        Self::from_inputs(DefinitionLifecycleInputs::from_document(
            user_library,
            pattern_state,
        ))
    }

    pub fn from_inputs(
        inputs: DefinitionLifecycleInputs,
    ) -> Result<Self, PatternDefinitionLifecycleError> {
        let bundled = load_bundled_pattern_definition_registry().map_err(|error| {
            PatternDefinitionLifecycleError::BundledDefinitions {
                message: error.to_string(),
            }
        })?;
        let registry = PatternDefinitionRegistry::build(
            bundled
                .definitions()
                .map(|resolved| resolved.definition.clone()),
            inputs
                .user_library
                .entries()
                .iter()
                .map(|entry| entry.definition.clone()),
            inputs.project_embedded,
        )
        .map_err(PatternDefinitionLifecycleError::Registry)?;
        Ok(Self {
            user_library: inputs.user_library,
            registry,
        })
    }

    pub fn user_library(&self) -> &UserPatternLibrarySnapshot {
        &self.user_library
    }

    pub fn registry(&self) -> &PatternDefinitionRegistry {
        &self.registry
    }

    /// Rebuilds the user layer from disk while retaining project-owned
    /// embeddings from the supplied persisted state.  Replacement happens only
    /// after the complete layered registry is valid, so a failed reload leaves
    /// this resolver's previous snapshot and resolution surface intact.
    pub fn reload_for_document(
        &mut self,
        pattern_state: &PatternDocumentState,
    ) -> Result<(), PatternDefinitionLifecycleError> {
        let refreshed_user_library =
            UserPatternLibrarySnapshot::load(self.user_library.directory())?;
        let replacement = Self::from_inputs(DefinitionLifecycleInputs::from_document(
            refreshed_user_library,
            pattern_state,
        ))?;
        *self = replacement;
        Ok(())
    }

    pub fn resolve(
        &self,
        pattern_id: &PatternId,
    ) -> Result<&ResolvedPatternDefinition, MissingPatternDefinitionDiagnostic> {
        self.registry
            .get(pattern_id)
            .map_err(|_| self.missing_definition_diagnostic(pattern_id))
    }

    /// Resolves the persisted registered selection without falling back to a
    /// label, a renderer adapter, or an arbitrary available definition.
    pub fn resolve_selected(
        &self,
        pattern_state: &PatternDocumentState,
    ) -> Result<Option<&ResolvedPatternDefinition>, MissingPatternDefinitionDiagnostic> {
        let PatternSelection::Registered(pattern_id) = &pattern_state.selected else {
            return Ok(None);
        };
        self.resolve(pattern_id).map(Some)
    }

    pub fn missing_definition_diagnostic(
        &self,
        requested_id: &PatternId,
    ) -> MissingPatternDefinitionDiagnostic {
        MissingPatternDefinitionDiagnostic {
            requested_id: requested_id.clone(),
            available_ids: self
                .registry
                .definitions()
                .map(|resolved| resolved.definition.id.clone())
                .collect(),
        }
    }

    /// Materializes a user-library definition as a complete project-owned
    /// definition plus a valid default instance. Callers install both values
    /// atomically through the document editor; this resolver never mutates a
    /// document or chooses an alternative stable ID.
    pub fn user_library_selection(
        &self,
        pattern_id: &PatternId,
        channels: impl IntoIterator<Item = OutputChannelId>,
    ) -> Result<(PatternDefinition, PatternInstanceParameters), UserLibrarySelectionError> {
        let resolved = self
            .resolve(pattern_id)
            .map_err(UserLibrarySelectionError::MissingDefinition)?;
        if resolved.authoritative_source != PatternDefinitionSource::UserLibrary {
            return Err(UserLibrarySelectionError::NotUserLibrary {
                id: pattern_id.clone(),
                source: resolved.authoritative_source,
            });
        }
        let instance = resolved
            .definition
            .default_instance_parameters(channels)
            .map_err(|error| UserLibrarySelectionError::DefaultInstance {
                id: pattern_id.clone(),
                message: error.to_string(),
            })?;
        Ok((resolved.definition.clone(), instance))
    }
}

/// Reads and strictly validates an external current-schema `.tnpattern` before
/// any XDG user-library mutation.  Stable-ID collisions are classified from
/// the existing resolver; the caller presents the resulting explicit choice.
pub fn inspect_external_pattern_definition_import(
    source_path: &Path,
    resolver: &PatternDefinitionLifecycleResolver,
) -> Result<ExternalPatternDefinitionImportPlan, ExternalPatternDefinitionImportError> {
    if !is_tnpattern_path(source_path) {
        return Err(ExternalPatternDefinitionImportError::InvalidSource {
            path: source_path.to_path_buf(),
            message: "external pattern imports must use the .tnpattern extension".into(),
        });
    }
    let bytes = fs::read(source_path).map_err(|error| {
        ExternalPatternDefinitionImportError::SourceRead {
            path: source_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    let definition = parse_tnpattern(&bytes).map_err(|error| {
        ExternalPatternDefinitionImportError::InvalidSource {
            path: source_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    let import = ExternalPatternDefinitionImport {
        source_path: source_path.to_path_buf(),
        definition,
    };
    let user_entries = resolver
        .user_library()
        .entries()
        .iter()
        .filter(|entry| entry.definition.id == import.definition.id)
        .collect::<Vec<_>>();
    if user_entries
        .iter()
        .any(|entry| entry.definition == import.definition)
    {
        return Ok(ExternalPatternDefinitionImportPlan::Identical {
            import,
            existing_source: PatternDefinitionSource::UserLibrary,
        });
    }
    if let Ok(resolved) = resolver.resolve(&import.definition.id) {
        if resolved.definition == import.definition {
            return Ok(ExternalPatternDefinitionImportPlan::Identical {
                import,
                existing_source: resolved.authoritative_source,
            });
        }
        if resolved.sources.contains(&PatternDefinitionSource::Bundled) {
            return Ok(ExternalPatternDefinitionImportPlan::ProtectedConflict {
                import,
                source: PatternDefinitionSource::Bundled,
            });
        }
        if resolved
            .sources
            .contains(&PatternDefinitionSource::ProjectEmbedded)
        {
            return Ok(ExternalPatternDefinitionImportPlan::ProtectedConflict {
                import,
                source: PatternDefinitionSource::ProjectEmbedded,
            });
        }
    }
    if !user_entries.is_empty() {
        return Ok(ExternalPatternDefinitionImportPlan::UserLibraryConflict {
            import,
            matching_paths: user_entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect(),
        });
    }
    Ok(ExternalPatternDefinitionImportPlan::Ready {
        destination: unused_user_library_destination(
            resolver.user_library().directory(),
            &import.definition.id,
        ),
        import,
    })
}

/// Performs only the explicit import action supplied by the caller. All
/// destination writes reuse the atomic definition writer; a failed write never
/// replaces a user file, and resolver refresh remains a caller-owned action.
pub fn commit_external_pattern_definition_import(
    plan: &ExternalPatternDefinitionImportPlan,
    choice: ExternalPatternDefinitionImportChoice,
    resolver: &PatternDefinitionLifecycleResolver,
) -> Result<ExternalPatternDefinitionImportCommit, ExternalPatternDefinitionImportError> {
    let (definition, destination, replaced_user_library_file) = match (plan, choice) {
        (
            ExternalPatternDefinitionImportPlan::Ready {
                import,
                destination,
            },
            ExternalPatternDefinitionImportChoice::WriteNew,
        ) => (import.definition.clone(), destination.clone(), false),
        (
            ExternalPatternDefinitionImportPlan::UserLibraryConflict {
                import,
                matching_paths,
            },
            ExternalPatternDefinitionImportChoice::ReplaceUserLibrary,
        ) if matching_paths.len() == 1 => {
            (import.definition.clone(), matching_paths[0].clone(), true)
        }
        (
            ExternalPatternDefinitionImportPlan::UserLibraryConflict { import, .. }
            | ExternalPatternDefinitionImportPlan::ProtectedConflict { import, .. },
            ExternalPatternDefinitionImportChoice::ImportAsNewCustomId,
        ) => {
            let original_id = import.definition.id.clone();
            let id = next_imported_custom_id(&original_id, resolver).map_err(|message| {
                ExternalPatternDefinitionImportError::NewCustomId {
                    id: original_id,
                    message,
                }
            })?;
            let destination =
                unused_user_library_destination(resolver.user_library().directory(), &id);
            let mut definition = import.definition.clone();
            definition.id = id;
            definition.validate().map_err(|error| {
                ExternalPatternDefinitionImportError::NewCustomId {
                    id: import.definition.id.clone(),
                    message: error.to_string(),
                }
            })?;
            (definition, destination, false)
        }
        _ => {
            return Err(ExternalPatternDefinitionImportError::InvalidChoice {
                id: plan.import().definition.id.clone(),
                choice,
            });
        }
    };
    definition
        .validate()
        .map_err(|error| ExternalPatternDefinitionImportError::NewCustomId {
            id: definition.id.clone(),
            message: error.to_string(),
        })?;
    save_user_pattern_definition(&destination, &definition)
        .map_err(ExternalPatternDefinitionImportError::Write)?;
    Ok(ExternalPatternDefinitionImportCommit {
        definition,
        destination,
        replaced_user_library_file,
    })
}

fn next_imported_custom_id(
    source: &PatternId,
    resolver: &PatternDefinitionLifecycleResolver,
) -> Result<PatternId, String> {
    let normalized = source.as_str().replace('.', "-");
    let occupied = resolver
        .registry()
        .definitions()
        .map(|resolved| resolved.definition.id.clone())
        .collect::<BTreeSet<_>>();
    for suffix in 1..=10_000_u32 {
        let candidate = PatternId::new(format!("custom.imported-{normalized}.{suffix}"))
            .map_err(|error| error.to_string())?;
        if !occupied.contains(&candidate) {
            return Ok(candidate);
        }
    }
    Err("all 10,000 generated stable IDs are occupied".into())
}

fn unused_user_library_destination(directory: &Path, id: &PatternId) -> PathBuf {
    let base = id.as_str();
    for suffix in 0..=10_000_u32 {
        let file_name = if suffix == 0 {
            format!("{base}.tnpattern")
        } else {
            format!("{base}-{suffix}.tnpattern")
        };
        let candidate = directory.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
    }
    // The bounded loop is intentionally impossible in normal operation. Let
    // the atomic writer surface a concrete filesystem failure rather than
    // widening the destination scope.
    directory.join(format!("{base}-10001.tnpattern"))
}

/// Saves one complete current-schema definition atomically. The caller owns
/// selection and subsequent discovery reload, so write failure cannot mutate a
/// document or replace a live resolver.
pub fn save_user_pattern_definition(
    path: &Path,
    definition: &PatternDefinition,
) -> Result<(), PatternDefinitionLifecycleError> {
    let bytes = serialize_tnpattern(definition).map_err(|error| {
        PatternDefinitionLifecycleError::UserLibraryWrite {
            path: path.to_path_buf(),
            message: format!("could not serialize .tnpattern: {error}"),
        }
    })?;
    crate::persistence::atomic_write(path, &bytes).map_err(|error| {
        PatternDefinitionLifecycleError::UserLibraryWrite {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })
}

/// Projects a complete shared-draft definition into a reusable library
/// definition. Definition-owned current instance values become new parameter
/// defaults, while output-channel instance values deliberately remain outside
/// the `.tnpattern` definition. A later library use creates fresh validated
/// per-channel defaults for the active output model.
pub fn project_definition_for_library_save(
    definition: &PatternDefinition,
    instance: &PatternInstanceParameters,
) -> Result<PatternDefinition, PatternDefinitionLibrarySaveError> {
    definition
        .validate_instance_parameters(instance)
        .map_err(|error| PatternDefinitionLibrarySaveError::InvalidInstance {
            id: definition.id.clone(),
            message: error.to_string(),
        })?;
    let current_values = instance
        .pattern_values
        .iter()
        .map(|value| (value.key.as_str(), &value.value))
        .collect::<BTreeMap<_, _>>();
    let mut projected = definition.clone();
    for parameter in &mut projected.parameters {
        if parameter.scope != DefinitionParameterScope::Pattern {
            continue;
        }
        let value = current_values.get(parameter.key.as_str()).ok_or_else(|| {
            PatternDefinitionLibrarySaveError::MissingPatternValue {
                id: definition.id.clone(),
                parameter: parameter.key.clone(),
            }
        })?;
        parameter.default = (*value).clone();
    }
    projected.validate().map_err(|error| {
        PatternDefinitionLibrarySaveError::InvalidProjectedDefinition {
            id: projected.id.clone(),
            message: error.to_string(),
        }
    })?;
    Ok(projected)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternDefinitionLibrarySaveError {
    InvalidInstance { id: PatternId, message: String },
    MissingPatternValue { id: PatternId, parameter: String },
    InvalidProjectedDefinition { id: PatternId, message: String },
}

impl fmt::Display for PatternDefinitionLibrarySaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInstance { id, message } => {
                write!(
                    formatter,
                    "could not save {id}: its current instance is invalid: {message}"
                )
            }
            Self::MissingPatternValue { id, parameter } => {
                write!(
                    formatter,
                    "could not save {id}: pattern value {parameter} is missing"
                )
            }
            Self::InvalidProjectedDefinition { id, message } => {
                write!(
                    formatter,
                    "could not save {id}: projected defaults are invalid: {message}"
                )
            }
        }
    }
}

impl std::error::Error for PatternDefinitionLibrarySaveError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserLibrarySelectionError {
    MissingDefinition(MissingPatternDefinitionDiagnostic),
    NotUserLibrary {
        id: PatternId,
        source: PatternDefinitionSource,
    },
    DefaultInstance {
        id: PatternId,
        message: String,
    },
}

impl fmt::Display for UserLibrarySelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDefinition(diagnostic) => write!(
                formatter,
                "user-library definition {} is missing; available definitions: {}",
                diagnostic.requested_id,
                diagnostic
                    .available_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::NotUserLibrary { id, source } => {
                write!(
                    formatter,
                    "definition {id} is authoritative from {source}, not the user library"
                )
            }
            Self::DefaultInstance { id, message } => {
                write!(
                    formatter,
                    "could not create a default instance for {id}: {message}"
                )
            }
        }
    }
}

impl std::error::Error for UserLibrarySelectionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternDefinitionLifecycleError {
    BundledDefinitions { message: String },
    UserLibraryDirectory { path: PathBuf, message: String },
    UserLibraryWrite { path: PathBuf, message: String },
    Registry(PatternDefinitionRegistryError),
}

impl fmt::Display for PatternDefinitionLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BundledDefinitions { message } => {
                write!(
                    formatter,
                    "could not load bundled pattern definitions: {message}"
                )
            }
            Self::UserLibraryDirectory { path, message } => {
                write!(
                    formatter,
                    "could not read user pattern library {}: {message}",
                    path.display()
                )
            }
            Self::UserLibraryWrite { path, message } => {
                write!(
                    formatter,
                    "could not write user pattern {}: {message}",
                    path.display()
                )
            }
            Self::Registry(error) => {
                write!(formatter, "could not resolve pattern definitions: {error}")
            }
        }
    }
}

impl std::error::Error for PatternDefinitionLifecycleError {}

fn is_tnpattern_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("tnpattern"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EmbeddedPatternDefinition;
    use crate::{
        Document, DocumentEditor, EmbeddedSvgAsset, GraphPosition, LiteralValue,
        PatternDefinitionSource, QuickControlDefinition, QuickControlKind, SharedRecipeEditorDraft,
        SourceArtwork, load_bundled_quadratic_radial_spiral_definition, load_document,
        save_document_atomic, serialize_tnpattern,
    };
    use sha2::{Digest, Sha256};
    use std::sync::Arc;

    fn definition(id: &str, name: &str) -> PatternDefinition {
        let mut definition = load_bundled_quadratic_radial_spiral_definition().unwrap();
        definition.id = PatternId::new(id).unwrap();
        definition.display.name = name.into();
        definition
    }

    fn pattern_state_with_embedded(definition: PatternDefinition) -> PatternDocumentState {
        let id = definition.id.clone();
        let instance = definition
            .default_instance_parameters(crate::OutputChannelId::CMYK)
            .unwrap();
        PatternDocumentState {
            selected: PatternSelection::Registered(id.clone()),
            instances: BTreeMap::new(),
            bundled_definition_instances: BTreeMap::new(),
            embedded_patterns: BTreeMap::from([(
                id,
                EmbeddedPatternDefinition {
                    definition,
                    instance,
                },
            )]),
        }
    }

    fn editor() -> DocumentEditor {
        DocumentEditor::new(Document::new(SourceArtwork {
            name: "source.svg".into(),
            media_type: "image/svg+xml".into(),
            bytes: Arc::from(
                br#"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"32\" height=\"24\"/>"#
                    .as_slice(),
            ),
        }))
    }

    #[test]
    fn xdg_directory_policy_is_stable() {
        assert_eq!(
            user_pattern_library_dir(Some(Path::new("/data")), Some(Path::new("/home/me"))),
            PathBuf::from("/data/toniator/patterns")
        );
        assert_eq!(
            user_pattern_library_dir(None, Some(Path::new("/home/me"))),
            PathBuf::from("/home/me/.local/share/toniator/patterns")
        );
    }

    #[test]
    fn discovery_is_sorted_and_malformed_files_remain_diagnostics() {
        let directory = tempfile::tempdir().unwrap();
        let alpha = definition("custom.alpha.v1", "Alpha");
        let zulu = definition("custom.zulu.v1", "Zulu");
        fs::write(
            directory.path().join("zulu.tnpattern"),
            serialize_tnpattern(&zulu).unwrap(),
        )
        .unwrap();
        fs::write(directory.path().join("broken.tnpattern"), b"not json").unwrap();
        fs::write(
            directory.path().join("alpha.tnpattern"),
            serialize_tnpattern(&alpha).unwrap(),
        )
        .unwrap();
        fs::write(directory.path().join("ignored.txt"), b"not a pattern").unwrap();

        let snapshot = UserPatternLibrarySnapshot::load(directory.path()).unwrap();
        assert_eq!(
            snapshot
                .entries()
                .iter()
                .map(|entry| entry
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned())
                .collect::<Vec<_>>(),
            vec!["alpha.tnpattern", "zulu.tnpattern"]
        );
        assert_eq!(snapshot.diagnostics().len(), 1);
        assert_eq!(
            snapshot.diagnostics()[0].path.file_name().unwrap(),
            "broken.tnpattern"
        );
        assert!(
            snapshot.diagnostics()[0]
                .message
                .contains("invalid .tnpattern JSON")
        );
    }

    #[test]
    fn reload_observes_changed_user_definition_without_choosing_selection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("spiral.tnpattern");
        fs::write(
            &path,
            serialize_tnpattern(&definition("custom.spiral.v1", "First")).unwrap(),
        )
        .unwrap();
        let mut snapshot = UserPatternLibrarySnapshot::load(directory.path()).unwrap();
        fs::write(
            &path,
            serialize_tnpattern(&definition("custom.spiral.v1", "Second")).unwrap(),
        )
        .unwrap();
        snapshot.reload().unwrap();
        assert_eq!(snapshot.entries()[0].definition.display.name, "Second");
    }

    #[test]
    fn resolver_reload_retains_project_selected_identity() {
        let directory = tempfile::tempdir().unwrap();
        let project = definition("custom.spiral.v1", "Project Spiral");
        let state = pattern_state_with_embedded(project);
        let mut resolver =
            PatternDefinitionLifecycleResolver::load_for_document(directory.path(), &state)
                .unwrap();
        fs::write(
            directory.path().join("spiral.tnpattern"),
            serialize_tnpattern(&definition("custom.spiral.v1", "Changed User Spiral")).unwrap(),
        )
        .unwrap();

        resolver.reload_for_document(&state).unwrap();
        let resolved = resolver.resolve_selected(&state).unwrap().unwrap();
        assert_eq!(resolved.definition.display.name, "Project Spiral");
        assert_eq!(
            resolved.authoritative_source,
            PatternDefinitionSource::ProjectEmbedded
        );
    }

    #[test]
    fn failed_resolver_reload_preserves_the_previous_resolution_surface() {
        let directory = tempfile::tempdir().unwrap();
        let user = definition("custom.user.v1", "User Definition");
        fs::write(
            directory.path().join("user.tnpattern"),
            serialize_tnpattern(&user).unwrap(),
        )
        .unwrap();
        let state = PatternDocumentState {
            selected: PatternSelection::Registered(user.id.clone()),
            instances: BTreeMap::new(),
            bundled_definition_instances: BTreeMap::new(),
            embedded_patterns: BTreeMap::new(),
        };
        let mut resolver =
            PatternDefinitionLifecycleResolver::load_for_document(directory.path(), &state)
                .unwrap();
        let before_snapshot = resolver.user_library().clone();
        let before_selection = state.selected.clone();
        let before_resolved = resolver.resolve_selected(&state).unwrap().unwrap().clone();
        let before_diagnostics = resolver
            .registry()
            .diagnostics()
            .cloned()
            .collect::<Vec<_>>();

        let conflict = definition("custom.user.v1", "Conflicting User Definition");
        fs::write(
            directory.path().join("conflict.tnpattern"),
            serialize_tnpattern(&conflict).unwrap(),
        )
        .unwrap();

        assert!(matches!(
            resolver.reload_for_document(&state),
            Err(PatternDefinitionLifecycleError::Registry(
                PatternDefinitionRegistryError::Conflict { .. }
            ))
        ));
        assert_eq!(resolver.user_library(), &before_snapshot);
        assert_eq!(state.selected, before_selection);
        let after_resolved = resolver.resolve_selected(&state).unwrap().unwrap();
        assert_eq!(after_resolved.definition, before_resolved.definition);
        assert_eq!(after_resolved.fingerprint, before_resolved.fingerprint);
        assert_eq!(
            after_resolved.authoritative_source,
            before_resolved.authoritative_source
        );
        assert_eq!(
            after_resolved.authoritative_source,
            PatternDefinitionSource::UserLibrary
        );
        assert_eq!(
            resolver
                .registry()
                .diagnostics()
                .cloned()
                .collect::<Vec<_>>(),
            before_diagnostics
        );
    }

    #[test]
    fn project_embedding_survives_state_round_trip_and_overrides_user_with_diagnostic() {
        let directory = tempfile::tempdir().unwrap();
        let user = definition("custom.spiral.v1", "User Spiral");
        fs::write(
            directory.path().join("spiral.tnpattern"),
            serialize_tnpattern(&user).unwrap(),
        )
        .unwrap();
        let project = definition("custom.spiral.v1", "Project Spiral");
        let state: PatternDocumentState = serde_json::from_slice(
            &serde_json::to_vec(&pattern_state_with_embedded(project)).unwrap(),
        )
        .unwrap();

        let resolver =
            PatternDefinitionLifecycleResolver::load_for_document(directory.path(), &state)
                .unwrap();
        let resolved = resolver.resolve_selected(&state).unwrap().unwrap();
        assert_eq!(resolved.definition.display.name, "Project Spiral");
        assert_eq!(
            resolved.authoritative_source,
            PatternDefinitionSource::ProjectEmbedded
        );
        assert_eq!(resolver.registry().diagnostics().count(), 1);
    }

    #[test]
    fn changed_user_content_cannot_replace_an_immutable_bundle() {
        let directory = tempfile::tempdir().unwrap();
        let mut changed = load_bundled_quadratic_radial_spiral_definition().unwrap();
        changed.display.name = "Changed bundled Spiral".into();
        fs::write(
            directory.path().join("spiral.tnpattern"),
            serialize_tnpattern(&changed).unwrap(),
        )
        .unwrap();

        let state = PatternDocumentState {
            selected: PatternSelection::NativeBasicV1,
            instances: BTreeMap::new(),
            bundled_definition_instances: BTreeMap::new(),
            embedded_patterns: BTreeMap::new(),
        };
        assert!(matches!(
            PatternDefinitionLifecycleResolver::load_for_document(directory.path(), &state),
            Err(PatternDefinitionLifecycleError::Registry(
                PatternDefinitionRegistryError::BundledDefinitionImmutable { .. }
            ))
        ));
    }

    #[test]
    fn missing_definition_diagnostic_has_sorted_recovery_candidates() {
        let directory = tempfile::tempdir().unwrap();
        let state = PatternDocumentState {
            selected: PatternSelection::Registered(PatternId::new("custom.missing.v1").unwrap()),
            instances: BTreeMap::new(),
            bundled_definition_instances: BTreeMap::new(),
            embedded_patterns: BTreeMap::new(),
        };
        let resolver =
            PatternDefinitionLifecycleResolver::load_for_document(directory.path(), &state)
                .unwrap();
        let missing = resolver.resolve_selected(&state).unwrap_err();
        assert_eq!(
            missing.requested_id,
            PatternId::new("custom.missing.v1").unwrap()
        );
        assert_eq!(
            missing
                .available_ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec![
                "compat.curves.v1",
                "compat.shapes.v1",
                "parametric-paths.quadratic-radial-spiral.v1",
                "structured-fields.wave-line-field.v1",
                "weighted-voronoi.v1",
            ]
        );
    }

    #[test]
    fn empty_missing_directory_is_a_valid_library_input() {
        let directory = tempfile::tempdir().unwrap().path().join("does-not-exist");
        let snapshot = UserPatternLibrarySnapshot::load(&directory).unwrap();
        assert_eq!(snapshot.directory(), directory);
        assert!(snapshot.entries().is_empty());
        assert!(snapshot.diagnostics().is_empty());
    }

    #[test]
    fn user_library_selection_embeds_undoes_redoes_and_reopens_portably() {
        let directory = tempfile::tempdir().unwrap();
        let user = definition("custom.library-spiral.v1", "Library Spiral");
        save_user_pattern_definition(&directory.path().join("spiral.tnpattern"), &user).unwrap();
        let mut editor = editor();
        let resolver = PatternDefinitionLifecycleResolver::load_for_document(
            directory.path(),
            &editor.document().pattern_state,
        )
        .unwrap();
        let (definition, instance) = resolver
            .user_library_selection(
                &user.id,
                editor
                    .document()
                    .artwork_pipeline
                    .output_model
                    .channels()
                    .to_vec(),
            )
            .unwrap();

        assert!(editor.install_and_select_embedded_pattern(definition.clone(), instance));
        assert_eq!(
            editor.document().pattern_state.selected_pattern_id(),
            Some(user.id.clone())
        );
        assert_eq!(
            editor
                .document()
                .pattern_state
                .selected_embedded_pattern()
                .map(|embedded| &embedded.definition),
            Some(&definition)
        );
        assert!(editor.undo());
        assert_ne!(
            editor.document().pattern_state.selected_pattern_id(),
            Some(user.id.clone())
        );
        assert!(editor.redo());

        let path = directory.path().join("portable.tnt");
        save_document_atomic(&path, editor.document()).unwrap();
        let reopened = load_document(&path).unwrap();
        assert_eq!(reopened.pattern_state.selected_pattern_id(), Some(user.id));
        assert_eq!(
            reopened
                .pattern_state
                .selected_embedded_pattern()
                .map(|embedded| &embedded.definition),
            Some(&definition)
        );
    }

    #[test]
    fn save_as_writes_the_full_definition_and_preserves_destination_on_validation_failure() {
        let directory = tempfile::tempdir().unwrap();
        let definition = definition("custom.complete-payload.v1", "Complete Payload");
        let destination = directory.path().join("complete.tnpattern");
        save_user_pattern_definition(&destination, &definition).unwrap();
        let snapshot = UserPatternLibrarySnapshot::load(directory.path()).unwrap();
        assert_eq!(snapshot.entries()[0].definition, definition);

        fs::write(&destination, b"previous contents").unwrap();
        let mut invalid = definition.clone();
        invalid.recipe_version = 3;
        assert!(matches!(
            save_user_pattern_definition(&destination, &invalid),
            Err(PatternDefinitionLifecycleError::UserLibraryWrite { .. })
        ));
        assert_eq!(fs::read(&destination).unwrap(), b"previous contents");
    }

    #[test]
    fn immutable_bundled_definitions_cannot_be_selected_as_user_library_content() {
        let directory = tempfile::tempdir().unwrap();
        let state = PatternDocumentState {
            selected: PatternSelection::NativeBasicV1,
            instances: BTreeMap::new(),
            bundled_definition_instances: BTreeMap::new(),
            embedded_patterns: BTreeMap::new(),
        };
        let resolver =
            PatternDefinitionLifecycleResolver::load_for_document(directory.path(), &state)
                .unwrap();
        assert!(matches!(
            resolver.user_library_selection(
                &PatternId::QUADRATIC_RADIAL_SPIRAL_V1,
                crate::OutputChannelId::CMYK,
            ),
            Err(UserLibrarySelectionError::NotUserLibrary {
                source: PatternDefinitionSource::Bundled,
                ..
            })
        ));
    }

    #[test]
    fn library_save_projects_structural_defaults_without_copying_channel_instance_values() {
        const UNEXPOSED_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"><path d=\"M0 0h1v1H0z\"/></svg>";
        let directory = tempfile::tempdir().unwrap();
        let mut definition = definition("custom.projected-spiral.v1", "Projected Spiral");
        definition.assets.push(EmbeddedSvgAsset {
            digest: format!("sha256:{:x}", Sha256::digest(UNEXPOSED_SVG.as_bytes())),
            svg: UNEXPOSED_SVG.into(),
        });
        definition.quick_controls.push(QuickControlDefinition {
            id: "unexposed-turns-shortcut".into(),
            parameter: "turns".into(),
            scope: DefinitionParameterScope::Pattern,
            kind: QuickControlKind::Slider,
            label: "Unexposed Turns Shortcut".into(),
        });
        definition.layout.node_positions.insert(
            "unexposed-layout-anchor".into(),
            GraphPosition { x: -77.5, y: 19.25 },
        );
        definition.validate().unwrap();
        let original_assets = definition.assets.clone();
        let original_controls = definition.quick_controls.clone();
        let original_layout = definition.layout.clone();
        let original_recipe = definition.recipe.clone();
        let mut instance = definition
            .default_instance_parameters(
                OutputChannelId::CMYK
                    .into_iter()
                    .chain(OutputChannelId::RGB),
            )
            .unwrap();
        for (key, value) in [
            ("turns", LiteralValue::Number(7.5)),
            ("center-x", LiteralValue::Number(0.25)),
            ("center-y", LiteralValue::Number(0.75)),
        ] {
            instance
                .pattern_values
                .iter_mut()
                .find(|entry| entry.key == key)
                .unwrap()
                .value = value;
        }
        let changed_channel = instance.output_channel_values.first_mut().unwrap();
        let changed_channel_id = changed_channel.channel.clone();
        changed_channel
            .values
            .iter_mut()
            .find(|entry| entry.key == "opacity")
            .unwrap()
            .value = LiteralValue::Number(0.17);
        definition.validate_instance_parameters(&instance).unwrap();

        let projected = project_definition_for_library_save(&definition, &instance).unwrap();
        assert_eq!(projected.format_version, definition.format_version);
        assert_eq!(projected.recipe_version, definition.recipe_version);
        assert_eq!(projected.display, definition.display);
        assert_eq!(projected.family, definition.family);
        assert_eq!(projected.outputs, definition.outputs);
        assert_eq!(projected.assets, original_assets);
        assert_eq!(projected.quick_controls, original_controls);
        assert_eq!(projected.layout, original_layout);
        assert_eq!(projected.recipe, original_recipe);
        for (key, expected) in [
            ("turns", LiteralValue::Number(7.5)),
            ("center-x", LiteralValue::Number(0.25)),
            ("center-y", LiteralValue::Number(0.75)),
        ] {
            assert_eq!(
                projected
                    .parameters
                    .iter()
                    .find(|parameter| parameter.key == key)
                    .unwrap()
                    .default,
                expected
            );
        }
        assert_eq!(
            projected
                .parameters
                .iter()
                .find(|parameter| parameter.key == "opacity")
                .unwrap()
                .default,
            LiteralValue::Number(0.92)
        );

        save_user_pattern_definition(&directory.path().join("projected.tnpattern"), &projected)
            .unwrap();
        let state = PatternDocumentState {
            selected: PatternSelection::NativeBasicV1,
            instances: BTreeMap::new(),
            bundled_definition_instances: BTreeMap::new(),
            embedded_patterns: BTreeMap::new(),
        };
        let resolver =
            PatternDefinitionLifecycleResolver::load_for_document(directory.path(), &state)
                .unwrap();
        let (discovered, selected_instance) = resolver
            .user_library_selection(
                &projected.id,
                OutputChannelId::CMYK
                    .into_iter()
                    .chain(OutputChannelId::RGB),
            )
            .unwrap();
        assert_eq!(discovered, projected);
        for (key, expected) in [
            ("turns", LiteralValue::Number(7.5)),
            ("center-x", LiteralValue::Number(0.25)),
            ("center-y", LiteralValue::Number(0.75)),
        ] {
            assert_eq!(
                selected_instance
                    .pattern_values
                    .iter()
                    .find(|entry| entry.key == key)
                    .unwrap()
                    .value,
                expected
            );
        }
        assert_eq!(
            selected_instance
                .output_channel_values
                .iter()
                .find(|channel| channel.channel == changed_channel_id)
                .unwrap()
                .values
                .iter()
                .find(|entry| entry.key == "opacity")
                .unwrap()
                .value,
            LiteralValue::Number(0.92)
        );
    }

    #[test]
    fn external_import_is_strict_explicit_and_preserves_complete_payloads() {
        let library = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let state = PatternDocumentState {
            selected: PatternSelection::NativeBasicV1,
            instances: BTreeMap::new(),
            bundled_definition_instances: BTreeMap::new(),
            embedded_patterns: BTreeMap::new(),
        };
        let imported = definition("custom.external-import.v1", "External Import");
        let source = external.path().join("external.tnpattern");
        fs::write(&source, serialize_tnpattern(&imported).unwrap()).unwrap();
        let resolver =
            PatternDefinitionLifecycleResolver::load_for_document(library.path(), &state).unwrap();

        let ready = inspect_external_pattern_definition_import(&source, &resolver).unwrap();
        let committed = commit_external_pattern_definition_import(
            &ready,
            ExternalPatternDefinitionImportChoice::WriteNew,
            &resolver,
        )
        .unwrap();
        assert!(!committed.replaced_user_library_file);
        assert_eq!(committed.definition, imported);
        assert_eq!(
            UserPatternLibrarySnapshot::load(library.path())
                .unwrap()
                .entries()[0]
                .definition,
            imported
        );

        let resolver =
            PatternDefinitionLifecycleResolver::load_for_document(library.path(), &state).unwrap();
        assert!(matches!(
            inspect_external_pattern_definition_import(&source, &resolver).unwrap(),
            ExternalPatternDefinitionImportPlan::Identical {
                existing_source: PatternDefinitionSource::UserLibrary,
                ..
            }
        ));

        let mut replacement = imported.clone();
        replacement.display.name = "Explicit Replacement".into();
        let replacement_source = external.path().join("replacement.tnpattern");
        fs::write(
            &replacement_source,
            serialize_tnpattern(&replacement).unwrap(),
        )
        .unwrap();
        let conflict =
            inspect_external_pattern_definition_import(&replacement_source, &resolver).unwrap();
        assert!(matches!(
            conflict,
            ExternalPatternDefinitionImportPlan::UserLibraryConflict { .. }
        ));
        assert!(matches!(
            commit_external_pattern_definition_import(
                &conflict,
                ExternalPatternDefinitionImportChoice::WriteNew,
                &resolver,
            ),
            Err(ExternalPatternDefinitionImportError::InvalidChoice { .. })
        ));
        assert_eq!(
            UserPatternLibrarySnapshot::load(library.path())
                .unwrap()
                .entries()[0]
                .definition,
            imported
        );
        let user_conflict_copy = commit_external_pattern_definition_import(
            &conflict,
            ExternalPatternDefinitionImportChoice::ImportAsNewCustomId,
            &resolver,
        )
        .unwrap();
        assert_ne!(user_conflict_copy.definition.id, replacement.id);
        let mut expected_user_conflict_copy = replacement.clone();
        expected_user_conflict_copy.id = user_conflict_copy.definition.id.clone();
        assert_eq!(user_conflict_copy.definition, expected_user_conflict_copy);
        let replaced = commit_external_pattern_definition_import(
            &conflict,
            ExternalPatternDefinitionImportChoice::ReplaceUserLibrary,
            &resolver,
        )
        .unwrap();
        assert!(replaced.replaced_user_library_file);
        assert_eq!(
            UserPatternLibrarySnapshot::load(library.path())
                .unwrap()
                .entries()
                .iter()
                .find(|entry| entry.definition.id == replacement.id)
                .unwrap()
                .definition,
            replacement
        );

        let mut bundled_collision = load_bundled_quadratic_radial_spiral_definition().unwrap();
        bundled_collision.display.name = "External bundled collision".into();
        let bundled_source = external.path().join("bundled-collision.tnpattern");
        fs::write(
            &bundled_source,
            serialize_tnpattern(&bundled_collision).unwrap(),
        )
        .unwrap();
        let resolver =
            PatternDefinitionLifecycleResolver::load_for_document(library.path(), &state).unwrap();
        let protected =
            inspect_external_pattern_definition_import(&bundled_source, &resolver).unwrap();
        assert!(matches!(
            protected,
            ExternalPatternDefinitionImportPlan::ProtectedConflict {
                source: PatternDefinitionSource::Bundled,
                ..
            }
        ));
        assert!(matches!(
            commit_external_pattern_definition_import(
                &protected,
                ExternalPatternDefinitionImportChoice::ReplaceUserLibrary,
                &resolver,
            ),
            Err(ExternalPatternDefinitionImportError::InvalidChoice { .. })
        ));
        let duplicated = commit_external_pattern_definition_import(
            &protected,
            ExternalPatternDefinitionImportChoice::ImportAsNewCustomId,
            &resolver,
        )
        .unwrap();
        assert_ne!(duplicated.definition.id, bundled_collision.id);
        let mut expected_duplicate = bundled_collision.clone();
        expected_duplicate.id = duplicated.definition.id.clone();
        assert_eq!(duplicated.definition, expected_duplicate);
    }

    #[test]
    fn external_import_protects_project_ids_and_never_replaces_a_destination_on_failure() {
        let library = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let project = definition("custom.project-import.v1", "Project Definition");
        let state = pattern_state_with_embedded(project.clone());
        let mut incoming = project.clone();
        incoming.display.name = "External Project Collision".into();
        let source = external.path().join("project-collision.tnpattern");
        fs::write(&source, serialize_tnpattern(&incoming).unwrap()).unwrap();
        let resolver =
            PatternDefinitionLifecycleResolver::load_for_document(library.path(), &state).unwrap();
        let protected = inspect_external_pattern_definition_import(&source, &resolver).unwrap();
        assert!(matches!(
            protected,
            ExternalPatternDefinitionImportPlan::ProtectedConflict {
                source: PatternDefinitionSource::ProjectEmbedded,
                ..
            }
        ));
        let duplicate = commit_external_pattern_definition_import(
            &protected,
            ExternalPatternDefinitionImportChoice::ImportAsNewCustomId,
            &resolver,
        )
        .unwrap();
        assert_ne!(duplicate.definition.id, project.id);
        assert!(
            UserPatternLibrarySnapshot::load(library.path())
                .unwrap()
                .entries()
                .iter()
                .any(|entry| entry.definition == duplicate.definition)
        );

        let blocked_parent = library.path().join("blocked-parent");
        fs::write(&blocked_parent, b"not a directory").unwrap();
        let blocked_destination = blocked_parent.join("destination.tnpattern");
        let failing_plan = ExternalPatternDefinitionImportPlan::Ready {
            import: ExternalPatternDefinitionImport {
                source_path: source,
                definition: incoming,
            },
            destination: blocked_destination.clone(),
        };
        assert!(matches!(
            commit_external_pattern_definition_import(
                &failing_plan,
                ExternalPatternDefinitionImportChoice::WriteNew,
                &resolver,
            ),
            Err(ExternalPatternDefinitionImportError::Write(_))
        ));
        assert!(!blocked_destination.exists());

        let malformed = external.path().join("malformed.tnpattern");
        fs::write(&malformed, b"not JSON").unwrap();
        assert!(matches!(
            inspect_external_pattern_definition_import(&malformed, &resolver),
            Err(ExternalPatternDefinitionImportError::InvalidSource { .. })
        ));
        let wrong_extension = external.path().join("valid-but-wrong-extension.txt");
        fs::write(&wrong_extension, serialize_tnpattern(&project).unwrap()).unwrap();
        assert!(matches!(
            inspect_external_pattern_definition_import(&wrong_extension, &resolver),
            Err(ExternalPatternDefinitionImportError::InvalidSource { .. })
        ));
    }

    #[test]
    fn duplicate_user_library_ids_reject_replace_and_allow_a_new_custom_id() {
        let library = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let existing = definition("custom.duplicate-user-id.v1", "Existing Duplicate");
        let first = library.path().join("first.tnpattern");
        let second = library.path().join("second.tnpattern");
        let existing_bytes = serialize_tnpattern(&existing).unwrap();
        fs::write(&first, &existing_bytes).unwrap();
        fs::write(&second, &existing_bytes).unwrap();
        let state = PatternDocumentState {
            selected: PatternSelection::NativeBasicV1,
            instances: BTreeMap::new(),
            bundled_definition_instances: BTreeMap::new(),
            embedded_patterns: BTreeMap::new(),
        };
        let resolver =
            PatternDefinitionLifecycleResolver::load_for_document(library.path(), &state).unwrap();
        let mut incoming = existing.clone();
        incoming.display.name = "Incoming Different Content".into();
        let source = external.path().join("incoming.tnpattern");
        fs::write(&source, serialize_tnpattern(&incoming).unwrap()).unwrap();
        let plan = inspect_external_pattern_definition_import(&source, &resolver).unwrap();
        assert!(matches!(
            &plan,
            ExternalPatternDefinitionImportPlan::UserLibraryConflict { matching_paths, .. }
                if matching_paths == &vec![first.clone(), second.clone()]
        ));
        assert!(matches!(
            commit_external_pattern_definition_import(
                &plan,
                ExternalPatternDefinitionImportChoice::ReplaceUserLibrary,
                &resolver,
            ),
            Err(ExternalPatternDefinitionImportError::InvalidChoice { .. })
        ));
        assert_eq!(fs::read(&first).unwrap(), existing_bytes);
        assert_eq!(fs::read(&second).unwrap(), existing_bytes);
        let new_id = commit_external_pattern_definition_import(
            &plan,
            ExternalPatternDefinitionImportChoice::ImportAsNewCustomId,
            &resolver,
        )
        .unwrap();
        assert_ne!(new_id.definition.id, incoming.id);
        let mut expected = incoming;
        expected.id = new_id.definition.id.clone();
        assert_eq!(new_id.definition, expected);
        assert_eq!(fs::read(&first).unwrap(), existing_bytes);
        assert_eq!(fs::read(&second).unwrap(), existing_bytes);
    }

    #[test]
    fn project_definition_draft_reopens_edits_undoes_redoes_and_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let definition = definition("custom.project-edit.v1", "Project Edit");
        let mut instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        instance
            .output_channel_values
            .first_mut()
            .unwrap()
            .values
            .iter_mut()
            .find(|value| value.key == "opacity")
            .unwrap()
            .value = LiteralValue::Number(0.37);
        let mut editor = editor();
        assert!(editor.install_and_select_embedded_pattern(definition.clone(), instance.clone()));
        let before = editor
            .document()
            .pattern_state
            .embedded_patterns
            .get(&definition.id)
            .cloned()
            .unwrap();
        let mut draft = SharedRecipeEditorDraft::document_local(
            before.definition.clone(),
            before.instance.clone(),
        )
        .unwrap();
        draft.set_value("turns", LiteralValue::Number(7.5)).unwrap();
        draft
            .set_node_position("spiral", GraphPosition { x: 88.0, y: -33.0 })
            .unwrap();
        assert!(editor.install_and_select_embedded_pattern(
            draft.definition().clone(),
            draft.instance().clone()
        ));
        let after = editor
            .document()
            .pattern_state
            .embedded_patterns
            .get(&definition.id)
            .cloned()
            .unwrap();
        assert_eq!(
            after.instance.output_channel_values,
            before.instance.output_channel_values
        );
        assert_eq!(
            after
                .instance
                .pattern_values
                .iter()
                .find(|value| value.key == "turns")
                .unwrap()
                .value,
            LiteralValue::Number(7.5)
        );
        assert!(editor.undo());
        assert_eq!(
            editor
                .document()
                .pattern_state
                .embedded_patterns
                .get(&definition.id),
            Some(&before)
        );
        assert!(editor.redo());
        assert_eq!(
            editor
                .document()
                .pattern_state
                .embedded_patterns
                .get(&definition.id),
            Some(&after)
        );
        let document_path = directory.path().join("project-edit.tnt");
        save_document_atomic(&document_path, editor.document()).unwrap();
        let reopened = load_document(&document_path).unwrap();
        assert_eq!(
            reopened.pattern_state.embedded_patterns.get(&definition.id),
            Some(&after)
        );
    }
}
