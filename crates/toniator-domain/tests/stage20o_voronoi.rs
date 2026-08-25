use toniator_domain::{
    CanvasSpec, ChannelGeometryResponseDelta, CoveragePolicy, Document, DocumentCommand,
    DocumentHistory, DocumentSession, MarkGeometryResponse, MarkGeometryResponseDelta,
    PatternCapabilityScope, PatternDefinitionBundle, PatternDefinitionDraft,
    PatternDefinitionRecipe, PatternGeometryResponse, PatternOutputCapabilityProjection,
    PatternOutputResponseDelta, PatternStructureRecipe, RegionGeometryResponse, SourceReference,
    SourceReferenceId, validate_pattern_output_deltas, validate_preset_record,
};

/// Builds a current document whose selected definition can atomically materialize a region recipe.
fn document() -> Document {
    Document::new_default_document(
        CanvasSpec {
            width: 160.0,
            height: 120.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("stage20o-domain").expect("source")),
    )
    .expect("default document")
}

/// Replaces the document base through the stale-aware recipe command and returns its history.
fn install_region_recipe() -> DocumentHistory {
    let mut history = DocumentHistory::new(DocumentSession::new(document()).expect("session"));
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("base definition")
        .definition
        .clone();
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
                PatternDefinitionDraft {
                    name: "ordinary regions".into(),
                    coverage: CoveragePolicy {
                        guard_steps: 2,
                        additional_margin: 0.0,
                    },
                },
            )),
        })
        .expect("region recipe installs");
    history
}

/// Proves the complete ID-free region recipe wraps site structure and binds only the fixed response.
#[test]
fn region_recipe_is_complete_and_fixed() {
    let recipe = PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
        PatternDefinitionDraft {
            name: "ordinary regions".into(),
            coverage: CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        },
    ));
    assert!(matches!(
        recipe.structure,
        PatternStructureRecipe::VoronoiRegions { .. }
    ));
    assert!(matches!(
        recipe.output_settings[0].response,
        PatternGeometryResponse::Regions(RegionGeometryResponse::Full)
    ));
}

/// Proves preset validation accepts the new fixed region response without adding a mutable delta branch.
#[test]
fn region_recipe_validates_as_current_schema_authority() {
    let record = toniator_domain::PresetRecord {
        metadata: toniator_domain::PresetMetadata {
            id: "ordinary-voronoi".into(),
            name: "Ordinary Voronoi".into(),
            category: "regions".into(),
            description: "complete ordinary cells".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
            PatternDefinitionDraft {
                name: "ordinary".into(),
                coverage: CoveragePolicy {
                    guard_steps: 1,
                    additional_margin: 0.0,
                },
            },
        )),
    };
    validate_preset_record(&record).expect("fixed ordinary-region recipe validates");
}

/// Proves a region recipe materializes one keyed Region response and a read-only capability projection.
#[test]
fn recipe_materialization_binds_regions_and_projects_fixed_capability() {
    let history = install_region_recipe();
    let definition_id = history.document().pattern_settings().definition_id;
    let bundle = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == definition_id)
        .expect("materialized bundle");
    assert!(matches!(
        bundle.definition.output_layers.as_slice(),
        [toniator_domain::PatternOutputLayer::Regions { .. }]
    ));
    assert!(matches!(
        bundle.output_settings.as_slice(),
        [toniator_domain::PatternOutputSettings {
            response: PatternGeometryResponse::Regions(RegionGeometryResponse::Full),
            ..
        }]
    ));
    let projection = history
        .document()
        .pattern_capabilities(PatternCapabilityScope::DocumentBase)
        .expect("capability projection");
    assert!(matches!(
        projection.outputs.as_slice(),
        [toniator_domain::PatternOutputCapabilityRecord {
            structural: PatternOutputCapabilityProjection::Regions(region),
            response: PatternGeometryResponse::Regions(RegionGeometryResponse::Full),
            ..
        }] if region.ordinary_voronoi && region.full_treatment_only && !region.sampled_paint
    ));
}

/// Proves bundle IDs, order, and response kinds reject malformed region authority before evaluation.
#[test]
fn bundle_and_region_delta_validation_rejects_foreign_and_incompatible_intent() {
    let history = install_region_recipe();
    let bundle = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == history.document().pattern_settings().definition_id)
        .expect("region bundle")
        .clone();
    let mut wrong_kind = bundle.clone();
    wrong_kind.output_settings[0].response = PatternGeometryResponse::Marks(MarkGeometryResponse {
        minimum_fill: 0.0,
        maximum_fill: 1.0,
    });
    assert_eq!(
        wrong_kind
            .validate()
            .expect_err("kind mismatch rejects")
            .path(),
        "pattern.bundle.output_settings.kind"
    );
    let mut foreign = bundle.clone();
    foreign.output_settings[0].output_layer_id = toniator_domain::PatternOutputLayerId(99_999);
    assert_eq!(
        foreign.validate().expect_err("foreign ID rejects").path(),
        "pattern.bundle.output_settings.order"
    );
    let truncated = PatternDefinitionBundle {
        definition: bundle.definition.clone(),
        output_settings: Vec::new(),
    };
    assert_eq!(
        truncated
            .validate()
            .expect_err("missing setting rejects")
            .path(),
        "pattern.bundle.output_settings.cardinality"
    );
    let region_id = bundle.output_settings[0].output_layer_id;
    assert_eq!(
        validate_pattern_output_deltas(
            &bundle,
            &[PatternOutputResponseDelta {
                output_layer_id: region_id,
                delta: ChannelGeometryResponseDelta::Marks(MarkGeometryResponseDelta {
                    minimum_fill_delta: Some(0.1),
                    maximum_fill_delta: None,
                }),
            }],
        )
        .expect_err("Regions never accept additive deltas")
        .path(),
        "channel.pattern.output_deltas.kind"
    );
}
