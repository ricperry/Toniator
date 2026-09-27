use toniator_domain::{
    CanvasSpec, CoveragePolicy, Document, DocumentCommand, DocumentHistory, DocumentSession,
    InvalidationLevel, MarkGeometryResponse, PatternDefinitionEdit, PatternDefinitionRecipe,
    PatternGeometryResponse, PatternMechanism, PatternOutputSettingsRecipe, PatternStructureRecipe,
    PropertyFieldId, RandomSiteCharacter, RandomSiteRefinement, SiteDensityModulation,
    SiteExclusionPolicy, SiteUseFilterRecipe, SourceReference,
};

/// Builds one neutral current scatter recipe for descriptor and command checks.
fn recipe() -> PatternDefinitionRecipe {
    PatternDefinitionRecipe {
        structure: PatternStructureRecipe::RandomSites {
            name: "Scatter refinement".into(),
            coverage: CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
            character: RandomSiteCharacter::RawUniform,
            seed: 1,
            density_modulation: SiteDensityModulation::Uniform,
            exclusion: SiteExclusionPolicy::None,
            maximum_attempts: 16_000_000,
            maximum_neighbor_checks: 16_000_000,
            refinement: RandomSiteRefinement::default(),
        },
        output_settings: vec![PatternOutputSettingsRecipe {
            source_filter: SiteUseFilterRecipe::All,
            response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.0,
                maximum_fill: 1.0,
            }),
        }],
    }
}

/// Returns the active descriptor fields for the current document state.
fn fields(history: &DocumentHistory) -> Vec<PropertyFieldId> {
    history
        .document()
        .property_descriptors()
        .into_iter()
        .map(|descriptor| descriptor.field)
        .collect()
}

/// Proves Lloyd subordinate controls appear only while enabled and retain inactive intent.
///
/// # Panics
/// Panics if recipe replacement or family edits fail, invalidate at the wrong level, expose
/// misleading inactive controls, or discard weighted/iteration values across an off/on toggle.
#[test]
fn lloyd_controls_use_progressive_disclosure_and_preserve_values() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 96.0,
            height: 64.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .unwrap()
        .definition
        .clone();
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: recipe(),
        })
        .unwrap();
    let definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == history.document().pattern_settings().definition_id)
        .unwrap()
        .definition
        .clone();
    let product_id = definition
        .mechanisms
        .iter()
        .find_map(|mechanism| match mechanism {
            PatternMechanism::RandomSiteProduct { id, .. } => Some(*id),
            _ => None,
        })
        .unwrap();
    assert!(fields(&history).contains(&PropertyFieldId::RandomLloydEnabled));
    assert!(!fields(&history).contains(&PropertyFieldId::RandomLloydDensityWeighted));
    assert!(!fields(&history).contains(&PropertyFieldId::RandomLloydIterations));

    let apply = |history: &mut DocumentHistory, edit| {
        let current = history
            .document()
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == definition.id)
            .unwrap()
            .definition
            .clone();
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: definition.id,
                base_definition: current,
                edit,
            })
            .unwrap()
    };
    assert_eq!(
        apply(
            &mut history,
            PatternDefinitionEdit::SetRandomLloydEnabled {
                mechanism_id: product_id,
                enabled: true,
            },
        )
        .invalidation,
        Some(InvalidationLevel::Family)
    );
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomLloydDensityWeighted {
            mechanism_id: product_id,
            density_weighted: true,
        },
    );
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomLloydIterations {
            mechanism_id: product_id,
            iterations: 7,
        },
    );
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomLloydEnabled {
            mechanism_id: product_id,
            enabled: false,
        },
    );
    assert!(!fields(&history).contains(&PropertyFieldId::RandomLloydDensityWeighted));
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomLloydEnabled {
            mechanism_id: product_id,
            enabled: true,
        },
    );
    let restored = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == definition.id)
        .unwrap()
        .definition
        .mechanisms
        .iter()
        .find_map(|mechanism| match mechanism {
            PatternMechanism::RandomSiteProduct { refinement, .. } => Some(refinement),
            _ => None,
        })
        .unwrap();
    assert!(restored.density_weighted);
    assert_eq!(restored.iterations, 7);
}
