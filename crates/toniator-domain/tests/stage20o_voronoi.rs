use toniator_domain::{
    CanvasSpec, ChannelGeometryResponseDelta, ChannelId, CoveragePolicy, Document, DocumentCommand,
    DocumentHistory, DocumentSession, InvalidationLevel, MarkGeometryResponse,
    MarkGeometryResponseDelta, PatternCapabilityScope, PatternDefinitionBundle,
    PatternDefinitionDraft, PatternDefinitionRecipe, PatternGeometryResponse,
    PatternOutputCapabilityProjection, PatternOutputResponseDelta, PatternOutputSettingsEdit,
    PatternStructureRecipe, PropertyEnumChoice, PropertyFieldId, PropertyTarget, PropertyUnit,
    RegionGeometryResponse, RegionSamplingStrategy, RegionSourceCapabilityKind,
    RegionTreatmentCapability, SourceReference, SourceReferenceId, validate_pattern_output_deltas,
    validate_preset_record,
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

/// Materializes one explicit Stage 20Q response through recipe authority so descriptor coverage
/// never mutates an accepted bundle directly.
fn document_with_region_response(response: RegionGeometryResponse) -> Document {
    let document = document();
    let mut recipe = PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
        PatternDefinitionDraft {
            name: "descriptor regions".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
        },
    ));
    recipe.output_settings[0].response = PatternGeometryResponse::Regions(response);
    let command = DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: document.pattern_settings().clone(),
        base_definition: document.pattern_definition_bundles()[0].definition.clone(),
        recipe,
    };
    document
        .apply_command(&command)
        .expect("explicit region recipe materializes")
        .0
}

/// Returns the ordered descriptor fields for channel one and its sole region output.
fn region_descriptor_fields(
    document: &Document,
) -> (toniator_domain::PatternOutputLayerId, Vec<PropertyFieldId>) {
    let definition_id = document.pattern_settings().definition_id;
    let bundle = document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == definition_id)
        .expect("selected bundle");
    let output_layer_id = bundle.output_settings[0].output_layer_id;
    let fields = document
        .property_descriptors()
        .iter()
        .filter(|descriptor| {
            descriptor.target
                == PropertyTarget::ChannelOutput(toniator_domain::ChannelId(1), output_layer_id)
        })
        .map(|descriptor| descriptor.field)
        .collect();
    (output_layer_id, fields)
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
        PatternGeometryResponse::Regions(RegionGeometryResponse::Full { .. })
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
            response: PatternGeometryResponse::Regions(RegionGeometryResponse::Full { .. }),
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
            response: PatternGeometryResponse::Regions(RegionGeometryResponse::Full { .. }),
            ..
        }] if matches!(
            &region.source,
            RegionSourceCapabilityKind::OrdinaryVoronoi { .. }
        ) && region.supported_treatments.len() == 3 && region.sampled_paint
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

/// Proves every region treatment exposes only its exact output-scoped Stage 20Q descriptor
/// vocabulary, including its treatment-specific numeric endpoint contract.
#[test]
fn region_response_descriptors_are_output_scoped_and_treatment_specific() {
    let full = document_with_region_response(RegionGeometryResponse::Full {
        sampling: RegionSamplingStrategy::ReferencePoint,
    });
    let (full_output, full_fields) = region_descriptor_fields(&full);
    assert_eq!(
        full_fields,
        vec![
            PropertyFieldId::RegionTreatment,
            PropertyFieldId::RegionSampling
        ]
    );
    let treatment = full
        .property_descriptors()
        .into_iter()
        .find(|descriptor| {
            descriptor.field == PropertyFieldId::RegionTreatment
                && descriptor.target
                    == PropertyTarget::ChannelOutput(toniator_domain::ChannelId(1), full_output)
        })
        .expect("Full treatment descriptor");
    assert_eq!(
        treatment.choices,
        &[
            PropertyEnumChoice::RegionTreatment(RegionTreatmentCapability::Full),
            PropertyEnumChoice::RegionTreatment(RegionTreatmentCapability::Scale),
            PropertyEnumChoice::RegionTreatment(RegionTreatmentCapability::ConstantGap),
        ]
    );
    let sampling = full
        .property_descriptors()
        .into_iter()
        .find(|descriptor| {
            descriptor.field == PropertyFieldId::RegionSampling
                && descriptor.target
                    == PropertyTarget::ChannelOutput(toniator_domain::ChannelId(1), full_output)
        })
        .expect("Full sampling descriptor");
    assert_eq!(
        sampling.choices,
        &[
            PropertyEnumChoice::RegionSampling(RegionSamplingStrategy::ReferencePoint),
            PropertyEnumChoice::RegionSampling(RegionSamplingStrategy::AreaAverage),
        ]
    );

    let scale = document_with_region_response(RegionGeometryResponse::Scale {
        sampling: RegionSamplingStrategy::AreaAverage,
        minimum_scale: 0.0,
        maximum_scale: 2.5,
    });
    let (scale_output, scale_fields) = region_descriptor_fields(&scale);
    assert_eq!(
        scale_fields,
        vec![
            PropertyFieldId::RegionTreatment,
            PropertyFieldId::RegionSampling,
            PropertyFieldId::RegionMinimumScale,
            PropertyFieldId::RegionMaximumScale,
        ]
    );
    for field in [
        PropertyFieldId::RegionMinimumScale,
        PropertyFieldId::RegionMaximumScale,
    ] {
        let descriptor = scale
            .property_descriptors()
            .into_iter()
            .find(|descriptor| {
                descriptor.field == field
                    && descriptor.target
                        == PropertyTarget::ChannelOutput(
                            toniator_domain::ChannelId(1),
                            scale_output,
                        )
            })
            .expect("Scale endpoint descriptor");
        assert_eq!(descriptor.bounds.expect("Scale bounds").minimum, Some(0.0));
        assert_eq!(descriptor.bounds.expect("Scale bounds").maximum, None);
        assert_eq!(descriptor.unit, PropertyUnit::None);
    }

    let gap = document_with_region_response(RegionGeometryResponse::ConstantGap {
        sampling: RegionSamplingStrategy::AreaAverage,
        minimum_gap: -4.0,
        maximum_gap: 3.0,
    });
    let (gap_output, gap_fields) = region_descriptor_fields(&gap);
    assert_eq!(
        gap_fields,
        vec![
            PropertyFieldId::RegionTreatment,
            PropertyFieldId::RegionSampling,
            PropertyFieldId::RegionMinimumGap,
            PropertyFieldId::RegionMaximumGap,
        ]
    );
    for field in [
        PropertyFieldId::RegionMinimumGap,
        PropertyFieldId::RegionMaximumGap,
    ] {
        let descriptor = gap
            .property_descriptors()
            .into_iter()
            .find(|descriptor| {
                descriptor.field == field
                    && descriptor.target
                        == PropertyTarget::ChannelOutput(toniator_domain::ChannelId(1), gap_output)
            })
            .expect("gap endpoint descriptor");
        assert_eq!(descriptor.bounds, None);
        assert_eq!(descriptor.unit, PropertyUnit::DocumentDistance);
    }
    for document in [&full, &scale, &gap] {
        assert_eq!(
            document.property_values().len(),
            document.property_descriptors().len()
        );
        document
            .validate_property_descriptors()
            .expect("region descriptor projection validates");
    }
}

/// Proves atomic region bundle edits retain compatible deltas, prune incompatible treatments,
/// remap selected shared copies, enumerate shared links in order, and restore exact snapshots.
#[test]
fn region_bundle_edits_are_stale_aware_history_owned_and_delta_safe() {
    let document = document_with_region_response(RegionGeometryResponse::Scale {
        sampling: RegionSamplingStrategy::ReferencePoint,
        minimum_scale: 0.2,
        maximum_scale: 1.2,
    });
    let (output_layer_id, _) = region_descriptor_fields(&document);
    let delta = document
        .set_channel_region_response_field_for_effective(
            ChannelId(1),
            output_layer_id,
            toniator_domain::RegionGeometryFieldEdit::MinimumScale(0.7),
        )
        .expect("Scale endpoint delta");
    let (document, _) = document.apply_command(&delta).expect("delta applies");
    let mut history = DocumentHistory::new(DocumentSession::new(document).expect("session"));
    let selected_base = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == history.document().pattern_settings().definition_id)
        .expect("shared Scale bundle")
        .clone();
    let before_selected = history.document().clone();
    let selected = history
        .document()
        .set_selected_channel_region_response_for_effective(
            ChannelId(1),
            output_layer_id,
            RegionGeometryResponse::Scale {
                sampling: RegionSamplingStrategy::ReferencePoint,
                minimum_scale: 0.3,
                maximum_scale: 1.4,
            },
        )
        .expect("selected numeric response command");
    let selected_result = history.apply(&selected).expect("selected copy applies");
    assert_eq!(selected_result.affected_channels, vec![ChannelId(1)]);
    assert_eq!(
        selected_result.invalidation,
        Some(InvalidationLevel::Family)
    );
    let after_selected = history.document().clone();
    let selected_definition_id = history
        .document()
        .effective_channel_pattern(ChannelId(1))
        .expect("selected effective pattern")
        .definition_id;
    assert_ne!(selected_definition_id, selected_base.definition.id);
    let selected_bundle = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == selected_definition_id)
        .expect("copied selected bundle")
        .clone();
    let remapped_output = selected_bundle.output_settings[0].output_layer_id;
    assert_ne!(remapped_output, output_layer_id);
    assert!(matches!(
        history
            .document()
            .channel_pattern_instance(ChannelId(1))
            .expect("selected instance")
            .output_response_deltas
            .as_slice(),
        [toniator_domain::PatternOutputResponseDelta {
            output_layer_id: id,
            delta: ChannelGeometryResponseDelta::Regions(_),
        }] if *id == remapped_output
    ));
    history.undo().expect("selected undo");
    assert_eq!(history.document(), &before_selected);
    history.redo().expect("selected redo");
    assert_eq!(history.document(), &after_selected);

    let stale = DocumentCommand::EditSelectedChannelPatternDefinitionBundle {
        channel_id: ChannelId(1),
        base_bundle: selected_base.clone(),
        edit: PatternOutputSettingsEdit::SetRegionResponse {
            output_layer_id,
            response: RegionGeometryResponse::Scale {
                sampling: RegionSamplingStrategy::ReferencePoint,
                minimum_scale: 0.4,
                maximum_scale: 1.5,
            },
        },
    };
    assert_eq!(
        history
            .document()
            .apply_command(&stale)
            .expect_err("stale selected base rejects")
            .to_string(),
        "pattern.bundle.base: selected response bundle base is stale"
    );
    let foreign = DocumentCommand::EditSelectedChannelPatternDefinitionBundle {
        channel_id: ChannelId(1),
        base_bundle: selected_bundle.clone(),
        edit: PatternOutputSettingsEdit::SetRegionResponse {
            output_layer_id: toniator_domain::PatternOutputLayerId(99_999),
            response: RegionGeometryResponse::Scale {
                sampling: RegionSamplingStrategy::ReferencePoint,
                minimum_scale: 0.4,
                maximum_scale: 1.5,
            },
        },
    };
    assert_eq!(
        history
            .document()
            .apply_command(&foreign)
            .expect_err("foreign output rejects")
            .to_string(),
        "pattern.bundle.output_settings.output_layer_id: region response edit targets a missing output"
    );

    let selected_sampling = history
        .document()
        .set_selected_channel_region_response_for_effective(
            ChannelId(1),
            remapped_output,
            RegionGeometryResponse::Scale {
                sampling: RegionSamplingStrategy::AreaAverage,
                minimum_scale: 0.3,
                maximum_scale: 1.4,
            },
        )
        .expect("selected sampling command");
    assert_eq!(
        history
            .apply(&selected_sampling)
            .expect("sampling applies")
            .invalidation,
        Some(InvalidationLevel::Family)
    );
    assert!(
        history
            .document()
            .channel_pattern_instance(ChannelId(1))
            .expect("selected instance")
            .output_response_deltas
            .iter()
            .any(|entry| entry.output_layer_id == remapped_output)
    );
    let treatment = history
        .document()
        .set_selected_channel_region_response_for_effective(
            ChannelId(1),
            remapped_output,
            RegionGeometryResponse::ConstantGap {
                sampling: RegionSamplingStrategy::AreaAverage,
                minimum_gap: -2.0,
                maximum_gap: 3.0,
            },
        )
        .expect("selected treatment command");
    history.apply(&treatment).expect("treatment applies");
    assert!(
        history
            .document()
            .channel_pattern_instance(ChannelId(1))
            .expect("selected instance")
            .output_response_deltas
            .is_empty()
    );

    let shared = history
        .document()
        .set_shared_region_response_for_definition(
            selected_base.definition.id,
            output_layer_id,
            RegionGeometryResponse::Scale {
                sampling: RegionSamplingStrategy::ReferencePoint,
                minimum_scale: 0.5,
                maximum_scale: 1.5,
            },
        )
        .expect("shared response command");
    let shared_result = history.apply(&shared).expect("shared edit applies");
    assert_eq!(
        shared_result.affected_channels,
        vec![ChannelId(2), ChannelId(3)]
    );
    assert_eq!(
        shared_result.invalidation,
        Some(InvalidationLevel::Realization)
    );
}
