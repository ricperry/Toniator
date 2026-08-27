use toniator_domain::{
    CanvasSpec, ChannelId, CoveragePolicy, Document, DocumentCommand, DocumentHistory,
    DocumentSession, PatternCapabilityFlag, PatternCapabilityScope, PatternDefinitionDraft,
    PatternDefinitionRecipe, PatternGeometryResponse, PatternOutputCapabilityProjection,
    PatternStructureRecipe, PropertyEnumChoice, PropertyFieldId, PropertyTarget,
    RegionGeometryResponse, RegionResizeAlgorithm, RegionSamplingStrategy,
    RegionSourceCapabilityKind, SourceReference, SourceReferenceId, validate_preset_record,
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

/// Replaces the document base through the recipe command and returns its history authority.
fn install_region_recipe() -> DocumentHistory {
    let mut history = DocumentHistory::new(DocumentSession::new(document()).expect("session"));
    let base = history.document().pattern_settings().clone();
    let base_definition = history.document().pattern_definition_bundles()[0]
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

/// Returns the selected output ID and its ordered output-scoped descriptor fields.
fn region_descriptor_fields(
    document: &Document,
) -> (toniator_domain::PatternOutputLayerId, Vec<PropertyFieldId>) {
    let bundle = document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == document.pattern_settings().definition_id)
        .expect("selected bundle");
    let output_layer_id = bundle.output_settings[0].output_layer_id;
    let fields = document
        .property_descriptors()
        .into_iter()
        .filter(|descriptor| {
            descriptor.target == PropertyTarget::ChannelOutput(ChannelId(1), output_layer_id)
        })
        .map(|descriptor| descriptor.field)
        .collect();
    (output_layer_id, fields)
}

/// Proves an ordinary region recipe defaults to linear-radius Scale fill 0.0..=1.0.
#[test]
fn region_recipe_materializes_the_current_default_response() {
    let history = install_region_recipe();
    let bundle = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == history.document().pattern_settings().definition_id)
        .expect("selected region bundle");
    assert!(matches!(
        bundle.output_settings[0].response,
        PatternGeometryResponse::Regions(RegionGeometryResponse {
            algorithm: RegionResizeAlgorithm::Scale,
            sampling: RegionSamplingStrategy::ReferencePoint,
            minimum_fill: 0.0,
            maximum_fill: 1.0,
        })
    ));
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
    validate_preset_record(&record).expect("current ordinary-region recipe validates");
}

/// Proves region capabilities advertise exactly two positive resize algorithms and fill response.
#[test]
fn region_capability_projection_excludes_negative_geometry_terms() {
    let history = install_region_recipe();
    let projection = history
        .document()
        .pattern_capabilities(PatternCapabilityScope::DocumentBase)
        .expect("capability projection");
    assert!(projection.supports_all(&[
        PatternCapabilityFlag::Voronoi,
        PatternCapabilityFlag::ScaleRegions,
        PatternCapabilityFlag::UniformOffsetRegions,
        PatternCapabilityFlag::FillRangeResponse,
    ]));
    assert!(matches!(
        projection.outputs.as_slice(),
        [toniator_domain::PatternOutputCapabilityRecord {
            structural: PatternOutputCapabilityProjection::Regions(region),
            ..
        }] if matches!(&region.source, RegionSourceCapabilityKind::OrdinaryVoronoi { .. })
            && region.supported_algorithms
                == [RegionResizeAlgorithm::Scale, RegionResizeAlgorithm::UniformOffset]
    ));
}

/// Proves both algorithms share the same output-scoped algorithm, sampling, and fill descriptors.
#[test]
fn region_descriptors_use_shared_fill_endpoints_for_both_algorithms() {
    let mut history = install_region_recipe();
    let (output_layer_id, fields) = region_descriptor_fields(history.document());
    assert_eq!(
        fields,
        vec![
            PropertyFieldId::RegionResizeAlgorithm,
            PropertyFieldId::RegionSampling,
            PropertyFieldId::RegionMinimumFill,
            PropertyFieldId::RegionMaximumFill,
        ]
    );
    let algorithm = history
        .document()
        .property_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.field == PropertyFieldId::RegionResizeAlgorithm)
        .expect("algorithm descriptor");
    assert_eq!(
        algorithm.choices,
        &[
            PropertyEnumChoice::RegionResizeAlgorithm(RegionResizeAlgorithm::Scale),
            PropertyEnumChoice::RegionResizeAlgorithm(RegionResizeAlgorithm::UniformOffset),
        ]
    );
    for field in [
        PropertyFieldId::RegionMinimumFill,
        PropertyFieldId::RegionMaximumFill,
    ] {
        let descriptor = history
            .document()
            .property_descriptors()
            .into_iter()
            .find(|descriptor| descriptor.field == field)
            .expect("fill descriptor");
        assert_eq!(descriptor.bounds.expect("fill bounds").minimum, Some(0.0));
        assert_eq!(descriptor.bounds.expect("fill bounds").maximum, Some(2.0));
    }
    let delta = history
        .document()
        .set_channel_region_response_field_for_effective(
            ChannelId(1),
            output_layer_id,
            toniator_domain::RegionGeometryFieldEdit::MinimumFill(0.5),
        )
        .expect("fill delta command");
    history.apply(&delta).expect("fill delta applies");
    let switched = history
        .document()
        .set_selected_channel_region_response_for_effective(
            ChannelId(1),
            output_layer_id,
            RegionGeometryResponse {
                algorithm: RegionResizeAlgorithm::UniformOffset,
                sampling: RegionSamplingStrategy::AreaAverage,
                minimum_fill: 0.0,
                maximum_fill: 1.5,
            },
        )
        .expect("algorithm switch command");
    history.apply(&switched).expect("algorithm switch applies");
    assert!(
        history
            .document()
            .channel_pattern_instance(ChannelId(1))
            .expect("channel instance")
            .output_response_deltas
            .iter()
            .any(|entry| matches!(
                entry.delta,
                toniator_domain::ChannelGeometryResponseDelta::Regions(_)
            ))
    );
    history
        .document()
        .validate_property_descriptors()
        .expect("descriptor projection validates");
}
