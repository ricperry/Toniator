use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft, AuthoredStructureKind,
    CanvasSpec, CoveragePolicy, Document, DocumentCommand, DocumentHistory, DocumentSession,
    GeneralizedSiteProductDraft, GuideDimensionDraft, MarkOrientationDraft, PathStrokeStyle,
    PatternDefinitionDraft, PatternDefinitionEdit, PatternDefinitionRecipe,
    PatternGeometryResponse, PatternOutputRealization, PatternStructureRecipe,
    PropertyCurrentValueKind, PropertyFieldId, PropertyFieldValue, PropertyTarget,
};

/// Builds the single-guide Along Guides recipe required by the Curve Motif authority.
fn curve_motif_recipe(phase: Option<f64>) -> PatternDefinitionRecipe {
    let motif = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                end: AuthoredPoint2 { x: 0.45, y: 0.2 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.45, y: 0.2 },
                end: AuthoredPoint2 { x: 1.0, y: 0.0 },
            },
        ],
    )
    .expect("asymmetric open motif validates");
    PatternDefinitionRecipe::connected(PatternStructureRecipe::CurveMotifPaths {
        definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
            name: "Curve Motif test".into(),
            coverage: CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
            dimensions: vec![GuideDimensionDraft {
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            }],
            product: GeneralizedSiteProductDraft::AlongGuides {
                dimension_indices: vec![0],
                interval_multiplier: 1.0,
                phase: 0.0,
            },
            orientation: MarkOrientationDraft::GuideTangent { dimension_index: 0 },
        }),
        motif,
        style: PathStrokeStyle::default(),
        mirror_alternate_rows: true,
        alternate_row_phase: phase,
    })
}

/// Builds a current document history that can atomically materialize one motif recipe.
fn history() -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 96.0,
            height: 64.0,
        },
        Default::default(),
    )
    .expect("current default document validates");
    DocumentHistory::new(DocumentSession::new(document).expect("current document starts"))
}

/// Materializes a motif resource, exposes connected controls, and restores exact history state.
#[test]
fn curve_motif_recipe_materializes_resource_descriptors_and_exact_history() {
    let mut history = history();
    let before = history.document().clone();
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .map(|bundle| &bundle.definition)
        .expect("base definition exists")
        .clone();
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: curve_motif_recipe(Some(0.25)),
        })
        .expect("Curve Motif recipe materializes");
    let materialized = history.document().clone();
    let definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == history.document().pattern_settings().definition_id)
        .map(|bundle| &bundle.definition)
        .expect("materialized definition exists");
    let [layer] = definition.output_layers.as_slice() else {
        panic!("one Curve Motif output materializes")
    };
    let PatternOutputRealization::CurveMotifPaths {
        structure_id,
        mirror_alternate_rows,
        alternate_row_phase,
        ..
    } = &layer.realization
    else {
        panic!("materialized output remains Curve Motif")
    };
    assert!(
        history
            .document()
            .authored_structure(*structure_id)
            .is_some()
    );
    assert!(*mirror_alternate_rows);
    assert_eq!(*alternate_row_phase, Some(0.25));
    let motif_structure_id = *structure_id;
    assert!(
        history
            .document()
            .property_descriptors()
            .iter()
            .any(|descriptor| {
                matches!(
                    descriptor.field,
                    toniator_domain::PropertyFieldId::ConnectedMinimumThickness
                )
            })
    );
    assert!(matches!(
        history
            .document()
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == definition.id)
            .expect("materialized response bundle exists")
            .output_settings[0]
            .response,
        PatternGeometryResponse::Connected(_)
    ));
    history
        .undo()
        .expect("replacement undoes")
        .expect("one undo exists");
    assert_eq!(history.document(), &before);
    history
        .redo()
        .expect("replacement redoes")
        .expect("one redo exists");
    assert_eq!(history.document(), &materialized);
    let original = history
        .document()
        .authored_structure(motif_structure_id)
        .expect("materialized motif resource exists")
        .clone();
    let replacement = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                end: AuthoredPoint2 { x: 0.35, y: -0.3 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.35, y: -0.3 },
                end: AuthoredPoint2 { x: 1.0, y: 0.0 },
            },
        ],
    )
    .expect("replacement motif validates");
    let edit = history
        .apply(&DocumentCommand::ReplaceAuthoredStructure {
            base_structure: original,
            replacement,
        })
        .expect("motif resource edit applies");
    assert_eq!(
        edit.invalidation,
        Some(toniator_domain::InvalidationLevel::Realization)
    );
    assert_eq!(edit.affected_channels.len(), 3);
    let edited = history.document().clone();
    history
        .undo()
        .expect("motif edit undoes")
        .expect("one edit undo exists");
    assert_eq!(history.document(), &materialized);
    history
        .redo()
        .expect("motif edit redoes")
        .expect("one edit redo exists");
    assert_eq!(history.document(), &edited);
}

/// Rejects a non-open phase fraction before recipe materialization can publish a resource.
#[test]
fn curve_motif_recipe_rejects_invalid_alternate_phase() {
    let mut history = history();
    let before = history.document().clone();
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .map(|bundle| &bundle.definition)
        .expect("base definition exists")
        .clone();
    let error = history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: curve_motif_recipe(Some(1.0)),
        })
        .expect_err("closed-unit phase rejects");
    assert!(
        error
            .to_string()
            .contains("pattern_definitions.recipe.curve_motif.alternate_row_phase")
    );
    assert_eq!(history.document(), &before);
}

/// Rejects malformed Curve Motif paths, two-guide families, and nonfinite layout before history can publish intent.
#[test]
fn curve_motif_recipe_rejects_invalid_path_and_family_shapes() {
    assert!(AuthoredStructureDraft::new(AuthoredStructureKind::OpenPath, Vec::new()).is_err());
    let coincident = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 0.0 },
            end: AuthoredPoint2 { x: 0.0, y: 0.0 },
        }],
    )
    .expect("coincident authored open path is independently representable");
    let closed = AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                end: AuthoredPoint2 { x: 0.5, y: 0.5 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.5, y: 0.5 },
                end: AuthoredPoint2 { x: 0.0, y: 0.0 },
            },
        ],
    )
    .expect("closed nonempty resource validates independently");
    let reject = |recipe: PatternDefinitionRecipe| {
        let mut history = history();
        let base = history.document().pattern_settings().clone();
        let base_definition = history
            .document()
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == base.definition_id)
            .map(|bundle| bundle.definition.clone())
            .expect("base definition exists");
        history
            .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base,
                base_definition,
                recipe,
            })
            .is_err()
    };
    let mut closed_recipe = curve_motif_recipe(None);
    let PatternStructureRecipe::CurveMotifPaths { motif, .. } = &mut closed_recipe.structure else {
        panic!("fixture remains Curve Motif")
    };
    *motif = closed;
    assert!(reject(closed_recipe));
    let mut coincident_recipe = curve_motif_recipe(None);
    let PatternStructureRecipe::CurveMotifPaths { motif, .. } = &mut coincident_recipe.structure
    else {
        panic!("fixture remains Curve Motif")
    };
    *motif = coincident;
    assert!(reject(coincident_recipe));
    let mut two_guide_recipe = curve_motif_recipe(None);
    let PatternStructureRecipe::CurveMotifPaths { definition, .. } =
        &mut two_guide_recipe.structure
    else {
        panic!("fixture remains Curve Motif")
    };
    let PatternStructureRecipe::GeneralizedStraightGuides {
        dimensions,
        product,
        ..
    } = definition.as_mut()
    else {
        panic!("fixture retains generalized guides")
    };
    dimensions.push(GuideDimensionDraft {
        baseline_angle_degrees: 90.0,
        phase: 0.0,
        spacing_multiplier: 1.0,
    });
    *product = GeneralizedSiteProductDraft::AlongGuides {
        dimension_indices: vec![0, 1],
        interval_multiplier: 1.0,
        phase: 0.0,
    };
    assert!(reject(two_guide_recipe));
    let mut nonfinite_recipe = curve_motif_recipe(None);
    let PatternStructureRecipe::CurveMotifPaths { definition, .. } =
        &mut nonfinite_recipe.structure
    else {
        panic!("fixture remains Curve Motif")
    };
    let PatternStructureRecipe::GeneralizedStraightGuides { dimensions, .. } = definition.as_mut()
    else {
        panic!("fixture retains generalized guides")
    };
    dimensions[0].phase = f64::NAN;
    assert!(reject(nonfinite_recipe));
    let mut wrong_family_recipe = curve_motif_recipe(None);
    let PatternStructureRecipe::CurveMotifPaths { definition, .. } =
        &mut wrong_family_recipe.structure
    else {
        panic!("fixture remains Curve Motif")
    };
    **definition = PatternStructureRecipe::StraightGrid(PatternDefinitionDraft {
        name: "wrong family".into(),
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    });
    assert!(reject(wrong_family_recipe));
}

/// Exposes exact Curve Motif descriptor identities and restores its optional phase edit through history.
#[test]
fn curve_motif_descriptor_commands_are_exact_and_history_backed() {
    let mut history = history();
    let base_settings = history.document().pattern_settings().clone();
    let initial_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base_settings.definition_id)
        .map(|bundle| &bundle.definition)
        .expect("base definition exists")
        .clone();
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: base_settings,
            base_definition: initial_definition,
            recipe: curve_motif_recipe(Some(0.25)),
        })
        .expect("Curve Motif materializes");
    let definition_id = history.document().pattern_settings().definition_id;
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == definition_id)
        .map(|bundle| &bundle.definition)
        .expect("materialized definition exists")
        .clone();
    let output_layer_id = base_definition.output_layers[0].id();
    let descriptors = history.document().property_descriptors();
    let motif_descriptors: Vec<_> = descriptors
        .iter()
        .filter(|descriptor| {
            matches!(
                descriptor.field,
                PropertyFieldId::CurveMotifMirrorAlternateRows
                    | PropertyFieldId::CurveMotifAlternateRowPhase
            )
        })
        .collect();
    assert_eq!(motif_descriptors.len(), 2);
    assert!(motif_descriptors.iter().all(|descriptor| {
        descriptor.target == PropertyTarget::OutputLayer(definition_id, output_layer_id)
            && descriptor.invalidation == toniator_domain::InvalidationLevel::Realization
    }));
    assert!(!descriptors.iter().any(|descriptor| {
        matches!(
            descriptor.field,
            PropertyFieldId::Visibility | PropertyFieldId::AlongGuidePhase
        ) && descriptor.target == PropertyTarget::OutputLayer(definition_id, output_layer_id)
    }));
    let values = history.document().property_values();
    assert!(values.iter().any(|value| {
        value.descriptor.field == PropertyFieldId::CurveMotifMirrorAlternateRows
            && value.value == PropertyCurrentValueKind::Boolean(true)
    }));
    assert!(values.iter().any(|value| {
        value.descriptor.field == PropertyFieldId::CurveMotifAlternateRowPhase
            && value.value == PropertyCurrentValueKind::OptionalFiniteF64(Some(0.25))
    }));

    let mirror_edit = PatternDefinitionEdit::SetCurveMotifMirrorAlternateRows {
        output_layer_id,
        mirror_alternate_rows: false,
    };
    assert_eq!(
        mirror_edit.field_projection(),
        toniator_domain::PropertyCommandFieldProjection {
            field: PropertyFieldId::CurveMotifMirrorAlternateRows,
            value: PropertyFieldValue::Boolean(false),
        }
    );
    let mirror_result = history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id,
            base_definition,
            edit: mirror_edit,
        })
        .expect("mirror command applies");
    assert_eq!(
        mirror_result.invalidation,
        Some(toniator_domain::InvalidationLevel::Realization)
    );
    let after_mirror = history.document().clone();
    let phase_base = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == definition_id)
        .map(|bundle| &bundle.definition)
        .expect("mirror definition remains")
        .clone();
    let phase_edit = PatternDefinitionEdit::SetCurveMotifAlternateRowPhase {
        output_layer_id,
        alternate_row_phase: None,
    };
    assert_eq!(
        phase_edit.field_projection(),
        toniator_domain::PropertyCommandFieldProjection {
            field: PropertyFieldId::CurveMotifAlternateRowPhase,
            value: PropertyFieldValue::OptionalFiniteF64(None),
        }
    );
    history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id,
            base_definition: phase_base,
            edit: phase_edit,
        })
        .expect("phase disable command applies");
    assert!(history.document().property_values().iter().any(|value| {
        value.descriptor.field == PropertyFieldId::CurveMotifAlternateRowPhase
            && value.value == PropertyCurrentValueKind::OptionalFiniteF64(None)
    }));
    let after_phase = history.document().clone();
    history
        .undo()
        .expect("phase command undoes")
        .expect("phase undo exists");
    assert_eq!(history.document(), &after_mirror);
    history
        .redo()
        .expect("phase command redoes")
        .expect("phase redo exists");
    assert_eq!(history.document(), &after_phase);
}
