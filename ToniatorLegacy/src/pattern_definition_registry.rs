//! In-memory resolution for validated declarative pattern definitions.
//!
//! This registry deliberately has no filesystem, project-persistence, UI, or
//! execution concerns. It resolves already-supplied definitions into one
//! immutable, deterministic lookup surface for later consumers.

use crate::pattern::PatternId;
use crate::pattern_definition::{PatternDefinition, PatternDefinitionError, serialize_tnpattern};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Origin of one definition supplied to the resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PatternDefinitionSource {
    Bundled,
    UserLibrary,
    ProjectEmbedded,
}

impl fmt::Display for PatternDefinitionSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bundled => "bundled",
            Self::UserLibrary => "user library",
            Self::ProjectEmbedded => "project embedded",
        })
    }
}

/// Content identity derived from canonical `.tnpattern` bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatternDefinitionFingerprint([u8; 32]);

impl PatternDefinitionFingerprint {
    pub fn from_canonical_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for PatternDefinitionFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("sha256:")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// An inspectable project-over-user resolution decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternDefinitionResolutionDiagnostic {
    pub id: PatternId,
    pub shadowed_source: PatternDefinitionSource,
    pub shadowed_fingerprint: PatternDefinitionFingerprint,
    pub authoritative_source: PatternDefinitionSource,
    pub authoritative_fingerprint: PatternDefinitionFingerprint,
}

/// Fully resolved content with every matching provenance retained.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPatternDefinition {
    pub definition: PatternDefinition,
    pub fingerprint: PatternDefinitionFingerprint,
    pub sources: BTreeSet<PatternDefinitionSource>,
    pub authoritative_source: PatternDefinitionSource,
    /// Deterministically ordered diagnostics for non-fatal project overrides.
    pub diagnostics: Vec<PatternDefinitionResolutionDiagnostic>,
}

/// Typed errors for definition loading and resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternDefinitionRegistryError {
    InvalidDefinition {
        source: PatternDefinitionSource,
        id: PatternId,
        message: String,
    },
    MissingDefinition(PatternId),
    Conflict {
        id: PatternId,
        existing_fingerprint: PatternDefinitionFingerprint,
        incoming_fingerprint: PatternDefinitionFingerprint,
        existing_sources: BTreeSet<PatternDefinitionSource>,
        incoming_source: PatternDefinitionSource,
    },
    BundledDefinitionImmutable {
        id: PatternId,
        bundled_fingerprint: PatternDefinitionFingerprint,
        incoming_fingerprint: PatternDefinitionFingerprint,
        incoming_source: PatternDefinitionSource,
    },
}

impl fmt::Display for PatternDefinitionRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDefinition {
                source,
                id,
                message,
            } => {
                write!(formatter, "{source} definition {id} is invalid: {message}")
            }
            Self::MissingDefinition(id) => write!(formatter, "pattern definition {id} is missing"),
            Self::Conflict {
                id,
                existing_fingerprint,
                incoming_fingerprint,
                existing_sources,
                incoming_source,
            } => write!(
                formatter,
                "pattern definition {id} conflicts: {incoming_source} content {incoming_fingerprint} differs from {existing_sources:?} content {existing_fingerprint}"
            ),
            Self::BundledDefinitionImmutable {
                id,
                bundled_fingerprint,
                incoming_fingerprint,
                incoming_source,
            } => write!(
                formatter,
                "bundled pattern definition {id} is immutable: {incoming_source} content {incoming_fingerprint} differs from bundled content {bundled_fingerprint}"
            ),
        }
    }
}

impl std::error::Error for PatternDefinitionRegistryError {}

/// Immutable deterministic lookup for validated declarative definitions.
#[derive(Debug, Default)]
pub struct PatternDefinitionRegistry {
    definitions: BTreeMap<PatternId, ResolvedPatternDefinition>,
}

impl PatternDefinitionRegistry {
    /// Resolves sources in deterministic provenance order. Project-embedded
    /// custom content is authoritative over differing user-library content,
    /// with a retained diagnostic. Bundled content and duplicate differences
    /// within one layer remain errors.
    pub fn build(
        bundled: impl IntoIterator<Item = PatternDefinition>,
        user_library: impl IntoIterator<Item = PatternDefinition>,
        project_embedded: impl IntoIterator<Item = PatternDefinition>,
    ) -> Result<Self, PatternDefinitionRegistryError> {
        let mut registry = Self::default();
        registry.insert_all(PatternDefinitionSource::Bundled, bundled)?;
        registry.insert_all(PatternDefinitionSource::UserLibrary, user_library)?;
        registry.insert_all(PatternDefinitionSource::ProjectEmbedded, project_embedded)?;
        Ok(registry)
    }

    pub fn get(
        &self,
        id: &PatternId,
    ) -> Result<&ResolvedPatternDefinition, PatternDefinitionRegistryError> {
        self.definitions
            .get(id)
            .ok_or_else(|| PatternDefinitionRegistryError::MissingDefinition(id.clone()))
    }

    /// Lists resolved definitions by stable ID, which is deterministic because
    /// the registry owns a `BTreeMap` keyed by `PatternId`.
    pub fn definitions(&self) -> impl ExactSizeIterator<Item = &ResolvedPatternDefinition> {
        self.definitions.values()
    }

    /// Lists project-over-user resolution diagnostics by stable pattern ID.
    pub fn diagnostics(&self) -> impl Iterator<Item = &PatternDefinitionResolutionDiagnostic> {
        self.definitions
            .values()
            .flat_map(|definition| definition.diagnostics.iter())
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    fn insert_all(
        &mut self,
        source: PatternDefinitionSource,
        definitions: impl IntoIterator<Item = PatternDefinition>,
    ) -> Result<(), PatternDefinitionRegistryError> {
        for definition in definitions {
            self.insert(source, definition)?;
        }
        Ok(())
    }

    fn insert(
        &mut self,
        source: PatternDefinitionSource,
        definition: PatternDefinition,
    ) -> Result<(), PatternDefinitionRegistryError> {
        let id = definition.id.clone();
        let canonical = serialize_tnpattern(&definition)
            .map_err(|error| invalid_definition_error(source, id.clone(), error))?;
        let fingerprint = PatternDefinitionFingerprint::from_canonical_bytes(&canonical);
        let Some(existing) = self.definitions.get_mut(&id) else {
            let mut sources = BTreeSet::new();
            sources.insert(source);
            self.definitions.insert(
                id,
                ResolvedPatternDefinition {
                    definition,
                    fingerprint,
                    sources,
                    authoritative_source: source,
                    diagnostics: Vec::new(),
                },
            );
            return Ok(());
        };

        if existing.fingerprint == fingerprint {
            existing.sources.insert(source);
            existing.authoritative_source = preferred_source(&existing.sources);
            return Ok(());
        }
        if existing.sources.contains(&PatternDefinitionSource::Bundled)
            || source == PatternDefinitionSource::Bundled
        {
            return Err(PatternDefinitionRegistryError::BundledDefinitionImmutable {
                id,
                bundled_fingerprint: existing.fingerprint,
                incoming_fingerprint: fingerprint,
                incoming_source: source,
            });
        }
        if source == PatternDefinitionSource::ProjectEmbedded
            && existing
                .sources
                .contains(&PatternDefinitionSource::UserLibrary)
            && !existing
                .sources
                .contains(&PatternDefinitionSource::ProjectEmbedded)
        {
            let diagnostic = PatternDefinitionResolutionDiagnostic {
                id: id.clone(),
                shadowed_source: PatternDefinitionSource::UserLibrary,
                shadowed_fingerprint: existing.fingerprint,
                authoritative_source: PatternDefinitionSource::ProjectEmbedded,
                authoritative_fingerprint: fingerprint,
            };
            existing.definition = definition;
            existing.fingerprint = fingerprint;
            existing.sources.clear();
            existing
                .sources
                .insert(PatternDefinitionSource::ProjectEmbedded);
            existing.authoritative_source = PatternDefinitionSource::ProjectEmbedded;
            existing.diagnostics.push(diagnostic);
            return Ok(());
        }
        Err(PatternDefinitionRegistryError::Conflict {
            id,
            existing_fingerprint: existing.fingerprint,
            incoming_fingerprint: fingerprint,
            existing_sources: existing.sources.clone(),
            incoming_source: source,
        })
    }
}

fn invalid_definition_error(
    source: PatternDefinitionSource,
    id: PatternId,
    error: PatternDefinitionError,
) -> PatternDefinitionRegistryError {
    PatternDefinitionRegistryError::InvalidDefinition {
        source,
        id,
        message: error.to_string(),
    }
}

fn preferred_source(sources: &BTreeSet<PatternDefinitionSource>) -> PatternDefinitionSource {
    if sources.contains(&PatternDefinitionSource::ProjectEmbedded) {
        PatternDefinitionSource::ProjectEmbedded
    } else if sources.contains(&PatternDefinitionSource::UserLibrary) {
        PatternDefinitionSource::UserLibrary
    } else {
        PatternDefinitionSource::Bundled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_bundled_weighted_voronoi_definition;

    fn definition(id: &str, name: &str) -> PatternDefinition {
        let mut definition = load_bundled_weighted_voronoi_definition().unwrap();
        definition.id = PatternId::new(id).unwrap();
        definition.display.name = name.into();
        definition
    }

    #[test]
    fn project_embedded_same_content_is_authoritative_and_deduplicated() {
        let bundled = definition("custom.one.v1", "One");
        let registry = PatternDefinitionRegistry::build(
            vec![bundled.clone()],
            vec![bundled.clone()],
            vec![bundled.clone()],
        )
        .unwrap();
        let resolved = registry.get(&bundled.id).unwrap();
        assert_eq!(registry.len(), 1);
        assert_eq!(
            resolved.authoritative_source,
            PatternDefinitionSource::ProjectEmbedded
        );
        assert_eq!(resolved.sources.len(), 3);
    }

    #[test]
    fn bundled_content_is_immutable() {
        let bundled = definition("custom.one.v1", "One");
        let changed = definition("custom.one.v1", "Changed");
        assert!(matches!(
            PatternDefinitionRegistry::build(vec![bundled.clone()], vec![changed], Vec::new()),
            Err(PatternDefinitionRegistryError::BundledDefinitionImmutable { .. })
        ));
    }

    #[test]
    fn project_content_is_authoritative_over_user_content_with_a_diagnostic() {
        let user = definition("custom.one.v1", "One");
        let project = definition("custom.one.v1", "Changed");
        let registry =
            PatternDefinitionRegistry::build(Vec::new(), vec![user.clone()], vec![project.clone()])
                .unwrap();
        let resolved = registry.get(&project.id).unwrap();
        assert_eq!(resolved.definition.display.name, "Changed");
        assert_eq!(
            resolved.authoritative_source,
            PatternDefinitionSource::ProjectEmbedded
        );
        assert_eq!(
            resolved.sources,
            BTreeSet::from([PatternDefinitionSource::ProjectEmbedded])
        );
        assert_eq!(resolved.diagnostics.len(), 1);
        let diagnostic = &resolved.diagnostics[0];
        assert_eq!(diagnostic.id, project.id);
        assert_eq!(
            diagnostic.shadowed_source,
            PatternDefinitionSource::UserLibrary
        );
        assert_eq!(
            diagnostic.authoritative_source,
            PatternDefinitionSource::ProjectEmbedded
        );
        assert_eq!(
            diagnostic.shadowed_fingerprint,
            PatternDefinitionFingerprint::from_canonical_bytes(
                &serialize_tnpattern(&user).unwrap()
            )
        );
        assert_eq!(diagnostic.authoritative_fingerprint, resolved.fingerprint);
        assert_eq!(registry.diagnostics().collect::<Vec<_>>(), vec![diagnostic]);
    }

    #[test]
    fn differing_definitions_within_one_layer_are_fatal() {
        assert!(matches!(
            PatternDefinitionRegistry::build(
                Vec::new(),
                vec![
                    definition("custom.one.v1", "One"),
                    definition("custom.one.v1", "Changed")
                ],
                Vec::new(),
            ),
            Err(PatternDefinitionRegistryError::Conflict { .. })
        ));
    }

    #[test]
    fn missing_invalid_fingerprints_and_order_are_deterministic() {
        let second = definition("custom.second.v1", "Second");
        let first = definition("custom.first.v1", "First");
        let registry = PatternDefinitionRegistry::build(
            Vec::new(),
            vec![second.clone(), first.clone()],
            Vec::new(),
        )
        .unwrap();
        let ids: Vec<_> = registry
            .definitions()
            .map(|resolved| resolved.definition.id.to_string())
            .collect();
        assert_eq!(ids, vec!["custom.first.v1", "custom.second.v1"]);
        let repeated =
            PatternDefinitionRegistry::build(Vec::new(), vec![first.clone()], Vec::new()).unwrap();
        assert_eq!(
            registry.get(&first.id).unwrap().fingerprint,
            repeated.get(&first.id).unwrap().fingerprint
        );
        let changed = PatternDefinitionRegistry::build(
            Vec::new(),
            vec![definition("custom.first.v1", "Changed")],
            Vec::new(),
        )
        .unwrap();
        assert_ne!(
            registry.get(&first.id).unwrap().fingerprint,
            changed.get(&first.id).unwrap().fingerprint
        );
        assert!(matches!(
            registry.get(&PatternId::new("custom.missing.v1").unwrap()),
            Err(PatternDefinitionRegistryError::MissingDefinition(_))
        ));
        let mut invalid = definition("custom.invalid.v1", "Invalid");
        invalid.recipe_version = 3;
        assert!(matches!(
            PatternDefinitionRegistry::build(Vec::new(), vec![invalid], Vec::new()),
            Err(PatternDefinitionRegistryError::InvalidDefinition { .. })
        ));
    }
}
