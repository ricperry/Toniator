use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft, AuthoredStructureKind,
    CanvasSpec, ConnectedGeometryResponse, CoveragePolicy, Document, DocumentCommand,
    DocumentHistory, DocumentSession, GeneralizedSiteProduct, GeneralizedSiteProductDraft,
    GuideDimensionDraft, GuideDimensionId, MarkGeometryResponse, MarkOrientation,
    MarkOrientationDraft, PathStrokeStyle, PatternDefinition, PatternDefinitionId,
    PatternDefinitionRecipe, PatternGeometryResponse, PatternMechanismId, PatternOutputLayerId,
    PatternOutputRealization, PatternOutputRealizationRecipe, PatternOutputSettingsRecipe,
    PatternRecipeSiteGenerationKind, PatternStructureRecipe, PresetMetadata, PresetRecord,
    SiteUseFilterRecipe, SourceReference, validate_pattern_definition, validate_preset_record,
};

/// Builds one canonical two-guide intersection recipe with a selected mark orientation.
fn oriented_intersection_recipe(orientation: MarkOrientationDraft) -> PatternDefinitionRecipe {
    PatternDefinitionRecipe {
        structure: PatternStructureRecipe::OrderedOutputs {
            definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
                name: "oriented intersections".into(),
                coverage: CoveragePolicy {
                    guard_steps: 2,
                    additional_margin: 0.0,
                },
                dimensions: vec![
                    GuideDimensionDraft {
                        baseline_angle_degrees: 0.0,
                        phase: 0.0,
                        spacing_multiplier: 1.0,
                    },
                    GuideDimensionDraft {
                        baseline_angle_degrees: 90.0,
                        phase: 0.0,
                        spacing_multiplier: 1.0,
                    },
                ],
                product: GeneralizedSiteProductDraft::Intersections {
                    dimension_indices: vec![0, 1],
                    merge_epsilon: 1e-9,
                },
                orientation,
            }),
            outputs: vec![PatternOutputRealizationRecipe::Marks],
        },
        output_settings: vec![PatternOutputSettingsRecipe {
            source_filter: SiteUseFilterRecipe::All,
            response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.9,
            }),
        }],
    }
}

/// Builds fresh history for materialization and reconstruction assertions.
fn history() -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 120.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document validates");
    DocumentHistory::new(DocumentSession::new(document).expect("default session validates"))
}

/// Proves relative transitions select their contributor while fixed marks retain every dimension.
///
/// Each transitioned recipe materializes and reconstructs exactly, so the selection is canonical
/// command authority rather than an evaluator or frontend repair.
#[test]
fn guide_relative_transition_selects_dimension_zero_and_one_and_round_trips() {
    for (orientation, expected) in [
        (MarkOrientationDraft::Fixed, vec![0, 1]),
        (
            MarkOrientationDraft::GuideTangent { dimension_index: 0 },
            vec![0],
        ),
        (
            MarkOrientationDraft::GuideNormal { dimension_index: 1 },
            vec![1],
        ),
    ] {
        let transitioned = oriented_intersection_recipe(orientation)
            .with_site_generation_kind(PatternRecipeSiteGenerationKind::AlongGuides)
            .expect("guide-relative Along Guides transition is valid");
        let PatternStructureRecipe::OrderedOutputs { definition, .. } = &transitioned.structure
        else {
            panic!("transition retains canonical ordered outputs")
        };
        let PatternStructureRecipe::GeneralizedStraightGuides {
            product:
                GeneralizedSiteProductDraft::AlongGuides {
                    dimension_indices, ..
                },
            ..
        } = definition.as_ref()
        else {
            panic!("transition installs Along Guides")
        };
        assert_eq!(dimension_indices, &expected);

        let mut draft = history();
        let base = draft.document().pattern_settings().clone();
        let base_definition = draft
            .document()
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == base.definition_id)
            .expect("base definition exists")
            .definition
            .clone();
        draft
            .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base,
                base_definition,
                recipe: transitioned.clone(),
            })
            .expect("transitioned recipe materializes");
        let reconstructed = draft
            .document()
            .reconstruct_pattern_definition_recipe(
                draft.document().pattern_settings().definition_id,
            )
            .expect("transitioned recipe reconstructs");
        assert_eq!(reconstructed, transitioned);
    }
}

/// Proves one Along Guides site product cannot admit conflicting active mark orientations.
#[test]
fn along_guide_transition_rejects_conflicting_active_mark_orientations() {
    let mut recipe =
        oriented_intersection_recipe(MarkOrientationDraft::GuideTangent { dimension_index: 0 });
    let PatternStructureRecipe::OrderedOutputs { outputs, .. } = &mut recipe.structure else {
        panic!("fixture retains canonical ordered outputs")
    };
    outputs.push(PatternOutputRealizationRecipe::AuthoredClosedShapeMarks {
        resource_index: 0,
        orientation: MarkOrientationDraft::GuideNormal { dimension_index: 1 },
    });
    recipe
        .output_settings
        .push(recipe.output_settings[0].clone());
    let points = [
        AuthoredPoint2 { x: 0.0, y: 0.0 },
        AuthoredPoint2 { x: 1.0, y: 0.0 },
        AuthoredPoint2 { x: 0.0, y: 1.0 },
    ];
    let shape = AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        (0..3)
            .map(|index| AuthoredCurveSegment::Line {
                start: points[index],
                end: points[(index + 1) % 3],
            })
            .collect(),
    )
    .expect("closed triangle validates");
    recipe.structure = PatternStructureRecipe::AuthoredResources {
        resources: vec![shape],
        definition: Box::new(recipe.structure),
    };
    validate_preset_record(&PresetRecord {
        metadata: PresetMetadata {
            id: "conflicting-oriented-sites".into(),
            name: "Conflicting Oriented Sites".into(),
            category: "test".into(),
            description: "Valid intersections before the rejected placement transition".into(),
            thumbnail: None,
        },
        recipe: recipe.clone(),
    })
    .expect("both contributors and the closed shape are valid before transitioning");

    let error = recipe
        .with_site_generation_kind(PatternRecipeSiteGenerationKind::AlongGuides)
        .expect_err("conflicting active mark orientations cannot share Along Guides sites");
    assert_eq!(error.path(), "preset.recipe.site_generation.orientation");
}

/// Proves one-guide family resizing resets removed orientation before choosing site provenance.
///
/// The resulting fixed mark uses the remaining guide, then materializes and reconstructs through
/// the same document command used by the frontend.
#[test]
fn one_guide_family_resize_resets_removed_orientation_before_site_transition() {
    let resized =
        oriented_intersection_recipe(MarkOrientationDraft::GuideNormal { dimension_index: 1 })
            .with_guide_family_dimension_count(1)
            .expect("removing the oriented dimension produces a valid one-guide family");
    let PatternStructureRecipe::OrderedOutputs { definition, .. } = &resized.structure else {
        panic!("resize retains canonical ordered outputs")
    };
    let PatternStructureRecipe::GeneralizedStraightGuides {
        dimensions,
        product:
            GeneralizedSiteProductDraft::AlongGuides {
                dimension_indices, ..
            },
        orientation,
        ..
    } = definition.as_ref()
    else {
        panic!("resize retains generalized guides with Along Guides sites")
    };
    assert_eq!(dimensions.len(), 1);
    assert_eq!(dimension_indices, &[0]);
    assert_eq!(orientation, &MarkOrientationDraft::Fixed);

    let mut draft = history();
    let base = draft.document().pattern_settings().clone();
    let base_definition = draft
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("base definition exists")
        .definition
        .clone();
    draft
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: resized.clone(),
        })
        .expect("resized recipe materializes");
    assert_eq!(
        draft
            .document()
            .reconstruct_pattern_definition_recipe(
                draft.document().pattern_settings().definition_id,
            )
            .expect("resized recipe reconstructs"),
        resized
    );
}

/// Proves unused family orientation cannot invalidate temporary ordered-output materialization.
///
/// A structural-path-only recipe may retain artist intent for a dormant mark orientation while
/// Along Guides emits both dimensions because no selected output consumes that orientation.
#[test]
fn structural_only_along_guides_materializes_with_unused_family_orientation() {
    let mut recipe =
        oriented_intersection_recipe(MarkOrientationDraft::GuideTangent { dimension_index: 1 });
    let PatternStructureRecipe::OrderedOutputs {
        definition,
        outputs,
    } = &mut recipe.structure
    else {
        panic!("fixture retains canonical ordered outputs")
    };
    let PatternStructureRecipe::GeneralizedStraightGuides { product, .. } = definition.as_mut()
    else {
        panic!("fixture retains generalized guides")
    };
    *product = GeneralizedSiteProductDraft::AlongGuides {
        dimension_indices: vec![0, 1],
        interval_multiplier: 1.0,
        phase: 0.0,
    };
    *outputs = vec![PatternOutputRealizationRecipe::StructuralPaths {
        style: PathStrokeStyle::default(),
    }];
    recipe.output_settings = vec![PatternOutputSettingsRecipe {
        source_filter: SiteUseFilterRecipe::All,
        response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
            minimum_thickness: 0.0,
            maximum_thickness: 1.0,
            bias: 0.0,
        }),
    }];
    validate_preset_record(&PresetRecord {
        metadata: PresetMetadata {
            id: "unused-family-orientation".into(),
            name: "Unused Family Orientation".into(),
            category: "test".into(),
            description: "Structural output does not consume family mark orientation".into(),
            thumbnail: None,
        },
        recipe: recipe.clone(),
    })
    .expect("unused family orientation is valid recipe intent");

    let mut draft = history();
    let base = draft.document().pattern_settings().clone();
    let base_definition = draft
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("base definition exists")
        .definition
        .clone();
    draft
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe,
        })
        .expect("structural-only recipe materializes without a temporary mark failure");
    let definition = draft
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == draft.document().pattern_settings().definition_id)
        .expect("materialized definition exists");
    assert!(matches!(
        definition.definition.output_layers.as_slice(),
        [toniator_domain::PatternOutputLayer {
            realization: PatternOutputRealization::GuidePaths { .. },
            ..
        }]
    ));
}

/// Proves validation rejects mismatched mark provenance and ignores unused family orientation.
#[test]
fn along_guide_orientation_validation_is_active_output_aware() {
    let invalid_recipe = PatternDefinitionRecipe {
        structure: PatternStructureRecipe::OrderedOutputs {
            definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
                name: "invalid oriented sites".into(),
                coverage: CoveragePolicy {
                    guard_steps: 2,
                    additional_margin: 0.0,
                },
                dimensions: vec![
                    GuideDimensionDraft {
                        baseline_angle_degrees: 0.0,
                        phase: 0.0,
                        spacing_multiplier: 1.0,
                    },
                    GuideDimensionDraft {
                        baseline_angle_degrees: 90.0,
                        phase: 0.0,
                        spacing_multiplier: 1.0,
                    },
                ],
                product: GeneralizedSiteProductDraft::AlongGuides {
                    dimension_indices: vec![0, 1],
                    interval_multiplier: 1.0,
                    phase: 0.0,
                },
                orientation: MarkOrientationDraft::GuideTangent { dimension_index: 0 },
            }),
            outputs: vec![PatternOutputRealizationRecipe::Marks],
        },
        output_settings: vec![PatternOutputSettingsRecipe {
            source_filter: SiteUseFilterRecipe::All,
            response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.9,
            }),
        }],
    };
    let error = validate_preset_record(&PresetRecord {
        metadata: PresetMetadata {
            id: "invalid-oriented-sites".into(),
            name: "Invalid Oriented Sites".into(),
            category: "test".into(),
            description: "validation fixture".into(),
            thumbnail: None,
        },
        recipe: invalid_recipe,
    })
    .expect_err("active mark orientation must cover every Along Guides site");
    assert_eq!(error.path(), "preset.recipe.orientation.provenance");

    let mut invalid_document = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(10),
        "invalid document orientation",
        PatternMechanismId(11),
        PatternMechanismId(12),
        PatternOutputLayerId(13),
        vec![
            toniator_domain::StraightGuideDimension {
                id: GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: toniator_domain::StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            toniator_domain::StraightGuideDimension {
                id: GuideDimensionId(2),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                repetition: toniator_domain::StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
        MarkOrientation::GuideTangent {
            dimension_id: GuideDimensionId(1),
        },
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    let error = validate_pattern_definition(&invalid_document)
        .expect_err("materialized mark orientation must cover every Along Guides site");
    assert_eq!(
        error.path(),
        "pattern_definitions.output_layers.orientation.provenance"
    );

    invalid_document.output_layers[0].realization = PatternOutputRealization::GuidePaths {
        guide_mechanism_id: PatternMechanismId(11),
        style: PathStrokeStyle::default(),
    };
    validate_pattern_definition(&invalid_document)
        .expect("unused family orientation does not constrain a structural-path output");
}
