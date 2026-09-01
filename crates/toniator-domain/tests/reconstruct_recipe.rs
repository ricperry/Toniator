use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft, AuthoredStructureKind,
    CanvasSpec, ChannelId, ConnectedGeometryResponse, CoveragePolicy, Document, DocumentCommand,
    DocumentHistory, DocumentSession, GeneralizedSiteProduct, GeneralizedSiteProductDraft,
    GenericGuidePrototypeDraft, GuideDimension, GuideDimensionDraft, GuideDimensionId,
    GuidePrototype, GuideRepetition, MarkGeometryResponse, MarkOrientation, MarkOrientationDraft,
    PathStrokeStyle, PatternDefinition, PatternDefinitionId, PatternDefinitionRecipe,
    PatternGeometryResponse, PatternMechanism, PatternMechanismId, PatternOutputLayerId,
    PatternOutputRealizationRecipe, PatternOutputSettingsRecipe, PatternRecipeConnectionMethodKind,
    PatternRecipeConstructionKind, PatternRecipeFamilyKind, PatternRecipeOutputKind,
    PatternRecipeRegionMethodKind, PatternRecipeSiteGenerationKind, PatternStructureRecipe,
    PresetMetadata, PresetRecord, SiteUseFilterRecipe, validate_preset_record,
};

/// Builds a current document history whose generated IDs remain independent from recipe IDs.
fn history() -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 120.0,
            height: 80.0,
        },
        Default::default(),
    )
    .expect("default document validates");
    DocumentHistory::new(DocumentSession::new(document).expect("default session validates"))
}

/// Materializes every domain-owned new-family starter without catalog metadata or allocated IDs.
///
/// # Panics
///
/// Panics when a starter stops validating, reconstructs to a different family, or requires a
/// persisted family discriminator instead of its recipe structure.
#[test]
fn id_free_family_starters_materialize_and_reconstruct_their_derived_kind() {
    for kind in [
        PatternRecipeFamilyKind::Guides,
        PatternRecipeFamilyKind::Dispersion,
        PatternRecipeFamilyKind::Parametric,
    ] {
        let recipe = PatternDefinitionRecipe::starter_for_family(kind);
        assert_eq!(recipe.family_kind(), kind);
        let mut draft = history();
        let base = draft.document().pattern_settings().clone();
        let base_definition = draft
            .document()
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == base.definition_id)
            .expect("starter target retains its document base definition")
            .definition
            .clone();
        draft
            .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base,
                base_definition,
                recipe: recipe.clone(),
            })
            .unwrap_or_else(|error| panic!("{kind:?} starter materializes: {error}"));
        let definition_id = draft.document().pattern_settings().definition_id;
        assert_eq!(
            draft
                .document()
                .reconstruct_pattern_definition_recipe(definition_id)
                .expect("materialized starter reconstructs")
                .family_kind(),
            kind
        );
    }
}

/// Proves the wizard’s consolidated classes retain typed recipe methods and foundation identity.
///
/// Marks include circle and custom-shape realizations, Connections include direct paths, motifs,
/// link networks, and mazes, and Regions include Voronoi and guide cells. Layering another output
/// must never turn the underlying Guides foundation into a separate family.
#[test]
fn construction_classes_consolidate_methods_without_creating_a_hybrid_family() {
    let guides = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides);
    assert_eq!(
        guides.construction_kinds().expect("starter decomposes"),
        vec![PatternRecipeConstructionKind::Marks]
    );
    let custom = guides
        .with_output_kind(0, PatternRecipeOutputKind::CustomShapeMarks)
        .expect("custom shape remains a valid mark method");
    assert_eq!(
        custom.construction_kinds().expect("custom mark decomposes"),
        vec![PatternRecipeConstructionKind::Marks]
    );

    let lines = guides
        .with_construction_kind(0, PatternRecipeConstructionKind::Connections)
        .expect("one guide supports direct lines");
    assert_eq!(
        lines.connection_method_kind(0).expect("method projects"),
        Some(PatternRecipeConnectionMethodKind::GuideLines)
    );
    let motif = lines
        .with_connection_method_kind(0, PatternRecipeConnectionMethodKind::CurveMotif)
        .expect("one-guide along-sites topology supports Curve Motif");
    assert_eq!(
        motif.connection_method_kind(0).expect("motif projects"),
        Some(PatternRecipeConnectionMethodKind::CurveMotif)
    );

    let grid = guides
        .with_guide_dimension_count(2)
        .expect("guide topology grows")
        .with_site_generation_kind(PatternRecipeSiteGenerationKind::GuideIntersections)
        .expect("two guides expose intersections")
        .with_construction_kind(0, PatternRecipeConstructionKind::Connections)
        .expect("intersections support connections")
        .with_connection_method_kind(0, PatternRecipeConnectionMethodKind::Maze)
        .expect("two-guide intersections support a maze");
    assert_eq!(grid.family_kind(), PatternRecipeFamilyKind::Guides);
    let cells = grid
        .with_construction_kind(0, PatternRecipeConstructionKind::Regions)
        .expect("intersections support regions")
        .with_region_method_kind(0, PatternRecipeRegionMethodKind::GuideCells)
        .expect("two-guide topology supports guide cells");
    assert_eq!(
        cells.region_method_kind(0).expect("region method projects"),
        Some(PatternRecipeRegionMethodKind::GuideCells)
    );
    let layered = cells
        .with_appended_construction_kind(PatternRecipeConstructionKind::Marks)
        .expect("foundation supports another visible output");
    assert_eq!(layered.family_kind(), PatternRecipeFamilyKind::Guides);
    assert_eq!(
        layered
            .construction_kinds()
            .expect("layered output projects"),
        vec![
            PatternRecipeConstructionKind::Regions,
            PatternRecipeConstructionKind::Marks,
        ]
    );
}

/// Proves one- and two-guide foundations become centered editable paths without losing intent.
///
/// The conversion retains site/output semantics and round-trips after fresh ID allocation. A
/// locked three-guide intersection lattice rejects curve editing before any document mutation.
#[test]
fn editable_guide_conversion_centers_paths_and_rejects_locked_three_guide_topology() {
    let frame = CanvasSpec {
        width: 120.0,
        height: 80.0,
    };
    for count in [1, 2] {
        let recipe = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides)
            .with_guide_dimension_count(count)
            .expect("guide starter accepts editable count");
        let converted = recipe
            .with_editable_guide_paths(frame.clone())
            .expect("one or two guides convert to authored paths");
        let PatternStructureRecipe::AuthoredResources {
            resources,
            definition,
        } = &converted.structure
        else {
            panic!("converted guide recipe owns a root resource table")
        };
        assert_eq!(resources.len(), usize::from(count));
        for resource in resources {
            assert_eq!(resource.kind(), AuthoredStructureKind::OpenPath);
            assert_eq!(
                resource.segments(),
                &[AuthoredCurveSegment::Line {
                    start: AuthoredPoint2 { x: -60.0, y: 0.0 },
                    end: AuthoredPoint2 { x: 60.0, y: 0.0 },
                }]
            );
        }
        let PatternStructureRecipe::OrderedOutputs {
            definition,
            outputs,
        } = definition.as_ref()
        else {
            panic!("converted recipe keeps explicit painter order")
        };
        let PatternStructureRecipe::GenericGuides {
            dimensions,
            product,
            orientation,
            ..
        } = definition.as_ref()
        else {
            panic!("converted foundation uses generic authored guides")
        };
        assert_eq!(dimensions.len(), usize::from(count));
        assert_eq!(
            product,
            match &recipe.structure {
                PatternStructureRecipe::GeneralizedStraightGuides { product, .. } => product,
                _ => panic!("starter remains generalized before conversion"),
            }
        );
        assert_eq!(
            orientation,
            match &recipe.structure {
                PatternStructureRecipe::GeneralizedStraightGuides { orientation, .. } =>
                    orientation,
                _ => panic!("starter remains generalized before conversion"),
            }
        );
        assert_eq!(outputs.len(), 1);

        let mut materialized = history();
        let base = materialized.document().pattern_settings().clone();
        let base_definition = materialized
            .document()
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == base.definition_id)
            .expect("base definition exists")
            .definition
            .clone();
        materialized
            .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base,
                base_definition,
                recipe: converted.clone(),
            })
            .expect("converted guide recipe materializes");
        assert_eq!(
            materialized
                .document()
                .reconstruct_pattern_definition_recipe(
                    materialized.document().pattern_settings().definition_id,
                )
                .expect("converted guide recipe reconstructs"),
            converted
        );
    }

    let compact = PatternDefinitionRecipe::marks(PatternStructureRecipe::StraightGrid(
        toniator_domain::PatternDefinitionDraft {
            name: "compact grid".into(),
            coverage: CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        },
    ));
    let compact_editable = compact
        .with_editable_guide_paths(frame.clone())
        .expect("compact two-guide grid converts to editable paths");
    let PatternStructureRecipe::AuthoredResources { resources, .. } = compact_editable.structure
    else {
        panic!("compact conversion owns its guide resources")
    };
    assert_eq!(resources.len(), 2);

    let locked = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides)
        .with_guide_dimension_count(3)
        .expect("three-guide starter canonicalizes")
        .with_site_generation_kind(PatternRecipeSiteGenerationKind::GuideIntersections)
        .expect("three guides expose locked intersections");
    let error = locked
        .with_editable_guide_paths(frame)
        .expect_err("locked three-guide topology cannot be curved independently");
    assert_eq!(error.path(), "preset.recipe.guide_editor.count");
}

/// Builds one one-dimensional generic family whose path reference is recipe-local.
///
/// # Panics
///
/// Does not panic; the returned draft is validated only by its caller.
fn generic_reference_family(resource_index: usize) -> PatternStructureRecipe {
    PatternStructureRecipe::GenericGuides {
        name: "indexed generic guide".into(),
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
        dimensions: vec![toniator_domain::GenericGuideDimensionDraft {
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GenericGuidePrototypeDraft::AuthoredOpenPathReference { resource_index },
            repetition: GuideRepetition::Single,
        }],
        product: GeneralizedSiteProductDraft::AlongGuides {
            dimension_indices: vec![0],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
        orientation: MarkOrientationDraft::Fixed,
    }
}

/// Wraps one structure in the smallest complete preset record for validation assertions.
///
/// # Panics
///
/// Does not panic; callers choose whether to expect record validation to succeed.
fn record(structure: PatternStructureRecipe) -> PresetRecord {
    PresetRecord {
        metadata: PresetMetadata {
            id: "indexed-generic".into(),
            name: "Indexed Generic".into(),
            category: "test".into(),
            description: "indexed generic resource validation fixture".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe {
            structure,
            output_settings: vec![PatternOutputSettingsRecipe {
                source_filter: SiteUseFilterRecipe::All,
                response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                    minimum_fill: 0.0,
                    maximum_fill: 1.0,
                }),
            }],
        },
    }
}

/// Builds one valid authored open path for root-table resource-reference fixtures.
///
/// # Panics
///
/// Panics when the fixed fixture no longer satisfies authored open-path validation.
fn open_path() -> AuthoredStructureDraft {
    AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 0.0 },
            end: AuthoredPoint2 { x: 1.0, y: 0.0 },
        }],
    )
    .expect("open path fixture validates")
}

/// Rejects indexed generic guides that omit, exceed, or mismatch their root resource table.
///
/// # Panics
///
/// Panics when validation stops reporting the exact missing-table, bounds, or kind diagnostics.
#[test]
fn indexed_generic_guides_require_an_in_bounds_open_path_root_resource() {
    let missing_table = validate_preset_record(&record(generic_reference_family(0)))
        .expect_err("generic resource reference cannot omit its root table");
    assert_eq!(
        missing_table.path(),
        "preset.recipe.resources.reference.missing_table"
    );
    let out_of_bounds =
        validate_preset_record(&record(PatternStructureRecipe::AuthoredResources {
            resources: vec![open_path()],
            definition: Box::new(generic_reference_family(1)),
        }))
        .expect_err("generic resource reference cannot exceed its root table");
    assert_eq!(
        out_of_bounds.path(),
        "preset.recipe.resources.reference.out_of_bounds"
    );
    let closed_shape = AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: -0.5, y: 0.0 },
                end: AuthoredPoint2 { x: 0.5, y: 0.0 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.5, y: 0.0 },
                end: AuthoredPoint2 { x: -0.5, y: 0.0 },
            },
        ],
    )
    .expect("closed shape fixture validates");
    let wrong_kind = validate_preset_record(&record(PatternStructureRecipe::AuthoredResources {
        resources: vec![closed_shape],
        definition: Box::new(generic_reference_family(0)),
    }))
    .expect_err("generic resource reference cannot target a closed shape");
    assert_eq!(
        wrong_kind.path(),
        "preset.recipe.resources.reference.wrong_kind"
    );
}

/// Rejects missing, out-of-range, and wrong-kind table references from authored output consumers.
///
/// # Panics
///
/// Panics when closed-shape or Curve Motif consumers stop using the same root-table validation
/// diagnostics as generic guides.
#[test]
fn authored_output_consumers_require_compatible_root_table_references() {
    let missing_table =
        validate_preset_record(&record(PatternStructureRecipe::AuthoredClosedShapeMarks {
            definition: Box::new(PatternStructureRecipe::StraightGrid(
                toniator_domain::PatternDefinitionDraft {
                    name: "missing shape table".into(),
                    coverage: CoveragePolicy {
                        guard_steps: 1,
                        additional_margin: 0.0,
                    },
                },
            )),
            resource_index: 0,
        }))
        .expect_err("authored shapes cannot omit the root resource table");
    assert_eq!(
        missing_table.path(),
        "preset.recipe.resources.reference.missing_table"
    );
    let out_of_bounds = validate_preset_record(&PresetRecord {
        metadata: record(PatternStructureRecipe::StraightGrid(
            toniator_domain::PatternDefinitionDraft {
                name: "metadata source".into(),
                coverage: CoveragePolicy {
                    guard_steps: 1,
                    additional_margin: 0.0,
                },
            },
        ))
        .metadata,
        recipe: PatternDefinitionRecipe::connected(PatternStructureRecipe::AuthoredResources {
            resources: vec![open_path()],
            definition: Box::new(PatternStructureRecipe::CurveMotifPaths {
                definition: Box::new(generic_reference_family(0)),
                resource_index: 1,
                style: PathStrokeStyle::default(),
                mirror_alternate_rows: false,
                alternate_row_phase: None,
            }),
        }),
    })
    .expect_err("Curve Motifs cannot exceed the root resource table");
    assert_eq!(
        out_of_bounds.path(),
        "preset.recipe.resources.reference.out_of_bounds"
    );
    let wrong_kind = validate_preset_record(&record(PatternStructureRecipe::AuthoredResources {
        resources: vec![open_path()],
        definition: Box::new(PatternStructureRecipe::OrderedOutputs {
            definition: Box::new(PatternStructureRecipe::StraightGrid(
                toniator_domain::PatternDefinitionDraft {
                    name: "wrong output kind".into(),
                    coverage: CoveragePolicy {
                        guard_steps: 1,
                        additional_margin: 0.0,
                    },
                },
            )),
            outputs: vec![PatternOutputRealizationRecipe::AuthoredClosedShapeMarks {
                resource_index: 0,
                orientation: MarkOrientationDraft::Fixed,
            }],
        }),
    }))
    .expect_err("closed-shape marks cannot consume an open path");
    assert_eq!(
        wrong_kind.path(),
        "preset.recipe.resources.reference.wrong_kind"
    );
}

/// Accepts one root authored-resource table and rejects noncanonical empty or nested tables.
#[test]
fn authored_resource_table_is_root_only_and_nonempty() {
    let resource = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 0.0 },
            end: AuthoredPoint2 { x: 1.0, y: 0.0 },
        }],
    )
    .expect("table resource validates");
    let record = |structure| PresetRecord {
        metadata: PresetMetadata {
            id: "table".into(),
            name: "table".into(),
            category: "test".into(),
            description: "table validation fixture".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::marks(structure),
    };
    let root = PatternStructureRecipe::AuthoredResources {
        resources: vec![resource.clone()],
        definition: Box::new(PatternStructureRecipe::StraightGrid(
            toniator_domain::PatternDefinitionDraft {
                name: "root".into(),
                coverage: CoveragePolicy {
                    guard_steps: 1,
                    additional_margin: 0.0,
                },
            },
        )),
    };
    validate_preset_record(&record(root.clone())).expect("one root table validates");
    let mut draft = history();
    let before = draft.document().clone();
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
            recipe: PatternDefinitionRecipe::marks(root),
        })
        .expect("root table publishes atomically");
    assert_eq!(
        draft.document().authored_structures().len(),
        before.authored_structures().len() + 1
    );
    draft
        .undo()
        .expect("root table undo succeeds")
        .expect("one undo exists");
    assert_eq!(draft.document(), &before);
    let empty = PatternStructureRecipe::AuthoredResources {
        resources: Vec::new(),
        definition: Box::new(PatternStructureRecipe::StraightGrid(
            toniator_domain::PatternDefinitionDraft {
                name: "empty".into(),
                coverage: CoveragePolicy {
                    guard_steps: 1,
                    additional_margin: 0.0,
                },
            },
        )),
    };
    assert!(validate_preset_record(&record(empty)).is_err());
    let nested = PatternStructureRecipe::AuthoredResources {
        resources: vec![resource.clone()],
        definition: Box::new(PatternStructureRecipe::AuthoredResources {
            resources: vec![resource],
            definition: Box::new(PatternStructureRecipe::StraightGrid(
                toniator_domain::PatternDefinitionDraft {
                    name: "nested".into(),
                    coverage: CoveragePolicy {
                        guard_steps: 1,
                        additional_margin: 0.0,
                    },
                },
            )),
        }),
    };
    assert!(validate_preset_record(&record(nested)).is_err());
}

/// Retains authored ordered outputs and their local site-use dependency after fresh ID allocation.
///
/// # Panics
///
/// Panics when ordered authored outputs fail to preserve their root-table resources, painter
/// order, output responses, or recipe-local site-use filter reference.
#[test]
fn ordered_authored_outputs_reconstruct_with_local_resources_and_filters() {
    let closed_shape = AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: -0.5, y: 0.0 },
                end: AuthoredPoint2 { x: 0.5, y: 0.0 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.5, y: 0.0 },
                end: AuthoredPoint2 { x: -0.5, y: 0.0 },
            },
        ],
    )
    .expect("closed authored shape validates");
    let motif = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 0.0 },
            end: AuthoredPoint2 { x: 1.0, y: 0.0 },
        }],
    )
    .expect("open motif validates");
    let recipe = PatternDefinitionRecipe {
        structure: PatternStructureRecipe::AuthoredResources {
            resources: vec![closed_shape, motif],
            definition: Box::new(PatternStructureRecipe::OrderedOutputs {
                definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
                    name: "ordered authored outputs".into(),
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
                    orientation: MarkOrientationDraft::GuideNormal { dimension_index: 0 },
                }),
                outputs: vec![
                    PatternOutputRealizationRecipe::AuthoredClosedShapeMarks {
                        resource_index: 0,
                        orientation: MarkOrientationDraft::GuideNormal { dimension_index: 0 },
                    },
                    PatternOutputRealizationRecipe::CurveMotifPaths {
                        resource_index: 1,
                        style: PathStrokeStyle::default(),
                        mirror_alternate_rows: true,
                        alternate_row_phase: Some(0.25),
                    },
                ],
            }),
        },
        output_settings: vec![
            PatternOutputSettingsRecipe {
                source_filter: SiteUseFilterRecipe::All,
                response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                    minimum_fill: 0.0,
                    maximum_fill: 1.0,
                }),
            },
            PatternOutputSettingsRecipe {
                source_filter: SiteUseFilterRecipe::SitesUnusedBy { output_index: 0 },
                response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                    minimum_thickness: 0.0,
                    maximum_thickness: 1.0,
                    bias: 0.0,
                }),
            },
        ],
    };
    let mut source = history();
    let base = source.document().pattern_settings().clone();
    let base_definition = source
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("base definition exists")
        .definition
        .clone();
    source
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: recipe.clone(),
        })
        .expect("ordered authored recipe materializes");
    let reconstructed = source
        .document()
        .reconstruct_pattern_definition_recipe(source.document().pattern_settings().definition_id)
        .expect("materialized ordered outputs reconstruct");
    assert_eq!(reconstructed, recipe);
}

/// Preserves one open-path table alias across a generic guide and its Curve Motif output.
///
/// # Panics
///
/// Panics when materialization splits the alias, fails to restore grouped history exactly, or
/// reconstruction retains allocated document IDs instead of the one local resource index.
#[test]
fn generic_guide_and_motif_output_share_one_open_path_resource() {
    let recipe = PatternDefinitionRecipe {
        structure: PatternStructureRecipe::AuthoredResources {
            resources: vec![open_path()],
            definition: Box::new(PatternStructureRecipe::CurveMotifPaths {
                definition: Box::new(generic_reference_family(0)),
                resource_index: 0,
                style: PathStrokeStyle::default(),
                mirror_alternate_rows: true,
                alternate_row_phase: Some(0.25),
            }),
        },
        output_settings: vec![PatternOutputSettingsRecipe {
            source_filter: SiteUseFilterRecipe::All,
            response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.0,
                maximum_thickness: 1.0,
                bias: 0.0,
            }),
        }],
    };
    let mut history = history();
    let before = history.document().clone();
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("base definition exists")
        .definition
        .clone();
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: recipe.clone(),
        })
        .expect("shared generic and motif recipe materializes");
    let materialized = history.document().clone();
    let definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == history.document().pattern_settings().definition_id)
        .expect("materialized definition exists");
    let PatternMechanism::GuideDimensions { dimensions, .. } = &definition.mechanisms[0] else {
        panic!("generic family retains its guide dimensions")
    };
    let GuidePrototype::AuthoredOpenPath {
        structure_id: guide_resource,
    } = dimensions[0].prototype
    else {
        panic!("generic guide retains its authored resource")
    };
    let [layer] = definition.output_layers.as_slice() else {
        panic!("Curve Motif wrapper retains one output")
    };
    let toniator_domain::PatternOutputRealization::CurveMotifPaths {
        structure_id: motif_resource,
        ..
    } = layer.realization
    else {
        panic!("output remains Curve Motif")
    };
    assert_eq!(motif_resource, guide_resource);
    assert_eq!(history.document().authored_structures().len(), 1);
    assert_eq!(
        history
            .document()
            .reconstruct_pattern_definition_recipe(definition.id)
            .expect("shared alias graph reconstructs"),
        recipe
    );
    history
        .undo()
        .expect("grouped replacement undoes")
        .expect("one replacement inverse exists");
    assert_eq!(history.document(), &before);
    history
        .redo()
        .expect("grouped replacement redoes")
        .expect("one replacement redo exists");
    assert_eq!(history.document(), &materialized);
}

/// Preserves one open-path table alias across repeated ordered Curve Motif outputs.
///
/// # Panics
///
/// Panics when painter-ordered Curve Motif outputs allocate duplicate paths or lose their shared
/// index, responses, or site-use dependency during reconstruction.
#[test]
fn repeated_motif_outputs_share_one_root_table_resource() {
    let recipe = PatternDefinitionRecipe {
        structure: PatternStructureRecipe::AuthoredResources {
            resources: vec![open_path()],
            definition: Box::new(PatternStructureRecipe::OrderedOutputs {
                definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
                    name: "repeated motifs".into(),
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
                    orientation: MarkOrientationDraft::Fixed,
                }),
                outputs: vec![
                    PatternOutputRealizationRecipe::CurveMotifPaths {
                        resource_index: 0,
                        style: PathStrokeStyle::default(),
                        mirror_alternate_rows: false,
                        alternate_row_phase: None,
                    },
                    PatternOutputRealizationRecipe::CurveMotifPaths {
                        resource_index: 0,
                        style: PathStrokeStyle::default(),
                        mirror_alternate_rows: true,
                        alternate_row_phase: Some(0.25),
                    },
                ],
            }),
        },
        output_settings: vec![
            PatternOutputSettingsRecipe {
                source_filter: SiteUseFilterRecipe::All,
                response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                    minimum_thickness: 0.0,
                    maximum_thickness: 1.0,
                    bias: 0.0,
                }),
            },
            PatternOutputSettingsRecipe {
                source_filter: SiteUseFilterRecipe::SitesUnusedBy { output_index: 0 },
                response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                    minimum_thickness: 0.0,
                    maximum_thickness: 1.0,
                    bias: 0.0,
                }),
            },
        ],
    };
    let mut history = history();
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("base definition exists")
        .definition
        .clone();
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: recipe.clone(),
        })
        .expect("repeated Curve Motif recipe materializes");
    let definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == history.document().pattern_settings().definition_id)
        .expect("materialized definition exists");
    let motif_resources = definition
        .output_layers
        .iter()
        .map(|layer| match layer.realization {
            toniator_domain::PatternOutputRealization::CurveMotifPaths { structure_id, .. } => {
                structure_id
            }
            _ => panic!("ordered output remains Curve Motif"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        motif_resources,
        vec![motif_resources[0], motif_resources[0]]
    );
    assert_eq!(history.document().authored_structures().len(), 1);
    assert_eq!(
        history
            .document()
            .reconstruct_pattern_definition_recipe(definition.id)
            .expect("shared motifs reconstruct"),
        recipe
    );
}

/// Preserves one closed-shape table alias across repeated ordered mark outputs.
///
/// # Panics
///
/// Panics when two output layers allocate duplicate shapes or reconstruction loses the shared
/// root-table index and painter-order response records.
#[test]
fn repeated_closed_shape_outputs_share_one_root_table_resource() {
    let closed_shape = AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: -0.5, y: 0.0 },
                end: AuthoredPoint2 { x: 0.5, y: 0.0 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.5, y: 0.0 },
                end: AuthoredPoint2 { x: -0.5, y: 0.0 },
            },
        ],
    )
    .expect("closed shape fixture validates");
    let recipe = PatternDefinitionRecipe {
        structure: PatternStructureRecipe::AuthoredResources {
            resources: vec![closed_shape],
            definition: Box::new(PatternStructureRecipe::OrderedOutputs {
                definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
                    name: "repeated authored shapes".into(),
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
                    orientation: MarkOrientationDraft::GuideNormal { dimension_index: 0 },
                }),
                outputs: vec![
                    PatternOutputRealizationRecipe::AuthoredClosedShapeMarks {
                        resource_index: 0,
                        orientation: MarkOrientationDraft::GuideNormal { dimension_index: 0 },
                    },
                    PatternOutputRealizationRecipe::AuthoredClosedShapeMarks {
                        resource_index: 0,
                        orientation: MarkOrientationDraft::GuideNormal { dimension_index: 0 },
                    },
                ],
            }),
        },
        output_settings: vec![
            PatternOutputSettingsRecipe {
                source_filter: SiteUseFilterRecipe::All,
                response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                    minimum_fill: 0.0,
                    maximum_fill: 1.0,
                }),
            },
            PatternOutputSettingsRecipe {
                source_filter: SiteUseFilterRecipe::SitesUnusedBy { output_index: 0 },
                response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                    minimum_fill: 0.0,
                    maximum_fill: 1.0,
                }),
            },
        ],
    };
    let mut history = history();
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("base definition exists")
        .definition
        .clone();
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: recipe.clone(),
        })
        .expect("repeated closed-shape recipe materializes");
    let definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == history.document().pattern_settings().definition_id)
        .expect("materialized definition exists");
    let shape_resources = definition
        .output_layers
        .iter()
        .map(|layer| match &layer.realization {
            toniator_domain::PatternOutputRealization::MarkPrototype {
                prototype: toniator_domain::MarkPrototype::AuthoredClosedShape { structure_id },
                ..
            } => *structure_id,
            _ => panic!("ordered output remains authored closed-shape marks"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        shape_resources,
        vec![shape_resources[0], shape_resources[0]]
    );
    assert_eq!(history.document().authored_structures().len(), 1);
    assert_eq!(
        history
            .document()
            .reconstruct_pattern_definition_recipe(definition.id)
            .expect("shared closed shapes reconstruct"),
        recipe
    );
}

/// Reconstructs generic guide aliases through one root table and rematerializes them with fresh IDs.
///
/// # Panics
///
/// Panics when the valid generic topology cannot be materialized or its resource alias graph
/// fails to round trip independently of allocated document IDs.
#[test]
fn generic_guide_recipe_reconstructs_and_rematerializes_with_one_shared_resource() {
    let mut source = history();
    let path = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 0.0 },
            end: AuthoredPoint2 { x: 1.0, y: 0.0 },
        }],
    )
    .expect("open path validates");
    let added = source
        .apply(&DocumentCommand::AddAuthoredStructure { draft: path })
        .expect("resource adds");
    let path_id = added
        .created_authored_structure_id
        .expect("resource has an allocated ID");
    let channel_id = ChannelId(1);
    let base_definition = source
        .document()
        .pattern_definition_for(channel_id)
        .expect("default definition exists")
        .clone();
    let definition = PatternDefinition::generalized_guides(
        PatternDefinitionId(100),
        "generic reconstruction",
        PatternMechanismId(100),
        PatternMechanismId(101),
        PatternOutputLayerId(100),
        vec![
            GuideDimension {
                id: GuideDimensionId(100),
                baseline_angle_degrees: 0.0,
                phase: 0.25,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: path_id,
                },
                repetition: GuideRepetition::TransformStack {
                    direction_degrees: 90.0,
                    spacing_multiplier: 1.5,
                },
            },
            GuideDimension {
                id: GuideDimensionId(101),
                baseline_angle_degrees: 45.0,
                phase: 0.0,
                prototype: GuidePrototype::CircularArc {
                    center: AuthoredPoint2 { x: 0.0, y: 0.0 },
                    radius: 20.0,
                    start_angle_degrees: 0.0,
                    sweep_angle_degrees: 180.0,
                },
                repetition: GuideRepetition::NormalOffset {
                    spacing: 4.0,
                    cleanup: toniator_domain::OffsetCleanup::DissolveCrossings,
                },
            },
            GuideDimension {
                id: GuideDimensionId(102),
                baseline_angle_degrees: 90.0,
                phase: 0.75,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: path_id,
                },
                repetition: GuideRepetition::Single,
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(100), GuideDimensionId(101)],
            merge_epsilon: 0.01,
        },
        MarkOrientation::GuideTangent {
            dimension_id: GuideDimensionId(100),
        },
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 1.0,
        },
    );
    source
        .apply(&DocumentCommand::ReplaceSelectedChannelDefinitionTopology {
            channel_id,
            base_definition,
            definition,
        })
        .expect("generic topology replaces selected channel");
    let definition_id = source
        .document()
        .pattern_definition_for(channel_id)
        .expect("generic definition remains active")
        .id;
    let recipe = source
        .document()
        .reconstruct_pattern_definition_recipe(definition_id)
        .expect("generic guide reconstructs");
    let PatternStructureRecipe::AuthoredResources {
        resources,
        definition,
    } = &recipe.structure
    else {
        panic!("generic definition retains one canonical resource table")
    };
    assert_eq!(resources.len(), 1, "repeated document uses intern once");
    let PatternStructureRecipe::OrderedOutputs { definition, .. } = definition.as_ref() else {
        panic!("generic definition retains explicit painter-order recipe")
    };
    assert!(matches!(
        definition.as_ref(),
        PatternStructureRecipe::GenericGuides {
            dimensions,
            ..
        } if matches!(
            (&dimensions[0].prototype, &dimensions[2].prototype),
            (
                GenericGuidePrototypeDraft::AuthoredOpenPathReference { resource_index: 0 },
                GenericGuidePrototypeDraft::AuthoredOpenPathReference { resource_index: 0 },
            )
        )
    ));
    let mut destination = history();
    let destination_dummy = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: -1.0, y: 0.0 },
            end: AuthoredPoint2 { x: 0.0, y: 0.0 },
        }],
    )
    .expect("destination dummy validates");
    destination
        .apply(&DocumentCommand::AddAuthoredStructure {
            draft: destination_dummy,
        })
        .expect("destination gets an unrelated earlier resource");
    let base = destination.document().pattern_settings().clone();
    let base_definition = destination
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("destination base exists")
        .definition
        .clone();
    destination
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: recipe.clone(),
        })
        .expect("generic recipe materializes with fresh resources");
    let destination_definition = destination
        .document()
        .pattern_definition_for(ChannelId(1))
        .expect("destination generic definition is active");
    let PatternMechanism::GuideDimensions { dimensions, .. } =
        &destination_definition.mechanisms[0]
    else {
        panic!("destination retains generalized guide dimensions")
    };
    let (
        GuidePrototype::AuthoredOpenPath {
            structure_id: first,
        },
        GuidePrototype::AuthoredOpenPath {
            structure_id: third,
        },
    ) = (&dimensions[0].prototype, &dimensions[2].prototype)
    else {
        panic!("destination retains both authored guide references")
    };
    assert_eq!(first, third, "two dimensions retain the table alias");
    assert_ne!(
        *first, path_id,
        "destination allocates an independent stable ID"
    );
    assert_eq!(
        destination
            .document()
            .reconstruct_pattern_definition_recipe(destination_definition.id)
            .expect("fresh generic definition reconstructs"),
        recipe,
        "reconstruction ignores allocated document resource IDs"
    );
    assert!(
        destination
            .document()
            .authored_structure_uses()
            .iter()
            .any(|use_value| matches!(
                use_value,
                toniator_domain::AuthoredStructureUse::Guide { .. }
            ))
    );
}

/// Proves guide-count growth preserves authored settings and round-trips without allocated IDs.
///
/// # Panics
///
/// Panics when the domain transition fails, does not extend an all-dimensions site selection, or
/// reconstruction after materialization differs from the resized ID-free recipe.
#[test]
fn guide_count_growth_round_trips_with_deterministic_dimensions() {
    let starter = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides);
    let recipe = PatternDefinitionRecipe {
        structure: PatternStructureRecipe::OrderedOutputs {
            definition: Box::new(starter.structure),
            outputs: vec![PatternOutputRealizationRecipe::Marks],
        },
        output_settings: starter.output_settings,
    };
    let resized = recipe
        .with_guide_dimension_count(3)
        .expect("one Along Guides dimension can grow to three");
    let PatternStructureRecipe::OrderedOutputs { definition, .. } = &resized.structure else {
        panic!("canonical mark recipe retains explicit output order")
    };
    let PatternStructureRecipe::GeneralizedStraightGuides {
        dimensions,
        product:
            GeneralizedSiteProductDraft::AlongGuides {
                dimension_indices, ..
            },
        ..
    } = definition.as_ref()
    else {
        panic!("guide starter remains a generalized Along Guides recipe")
    };
    assert_eq!(dimension_indices, &[0, 1, 2]);
    assert_eq!(dimensions.len(), 3);
    assert_eq!(dimensions[0].baseline_angle_degrees, 0.0);
    assert_eq!(dimensions[1].baseline_angle_degrees, 60.0);
    assert_eq!(dimensions[2].baseline_angle_degrees, 120.0);
    let mut draft = history();
    let base = draft.document().pattern_settings().clone();
    let base_definition = draft
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("document base exists")
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
            .expect("resized definition reconstructs"),
        resized
    );
}

/// Proves reductions prune selections but reject removal of a still-authored orientation target.
///
/// # Panics
///
/// Panics when a stale recipe-local orientation is accepted or a compatible fixed-orientation
/// reduction fails to retain the ordered two-dimension intersection product.
#[test]
fn guide_count_reduction_rejects_stale_orientation_and_prunes_compatible_selections() {
    let base = PatternStructureRecipe::GeneralizedStraightGuides {
        name: "three guide reduction".into(),
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
        dimensions: vec![
            GuideDimensionDraft {
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
            GuideDimensionDraft {
                baseline_angle_degrees: 60.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
            GuideDimensionDraft {
                baseline_angle_degrees: 120.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
        ],
        product: GeneralizedSiteProductDraft::Intersections {
            dimension_indices: vec![0, 1, 2],
            merge_epsilon: 0.0,
        },
        orientation: MarkOrientationDraft::GuideNormal { dimension_index: 2 },
    };
    let stale = PatternDefinitionRecipe::marks(base.clone())
        .with_guide_dimension_count(2)
        .expect_err("removing the oriented guide must be atomic");
    assert_eq!(stale.path(), "preset.recipe.orientation.dimension_index");
    let mut compatible_base = base;
    let PatternStructureRecipe::GeneralizedStraightGuides { orientation, .. } =
        &mut compatible_base
    else {
        unreachable!("fixture is a generalized guide recipe")
    };
    *orientation = MarkOrientationDraft::Fixed;
    let compatible = PatternDefinitionRecipe::marks(compatible_base)
        .with_guide_dimension_count(2)
        .expect("fixed-orientation intersections can shrink to two");
    let PatternStructureRecipe::GeneralizedStraightGuides {
        dimensions,
        product:
            GeneralizedSiteProductDraft::Intersections {
                dimension_indices, ..
            },
        ..
    } = compatible.structure
    else {
        panic!("compatible reduction retains generalized intersections")
    };
    assert_eq!(dimensions.len(), 2);
    assert_eq!(dimension_indices, vec![0, 1]);
}

/// Proves family authoring can return an intersection design to one guide atomically.
///
/// The transition changes sites to Along Guides, retains the Connections construction class as
/// guide paths, and leaves the strict count transition available for callers that require a
/// minimum-cardinality error.
///
/// # Panics
///
/// Panics when a two-guide maze does not reject strict reduction, the family-authoring transition
/// cannot reduce it, or the reduced recipe retains an incompatible site or maze method.
#[test]
fn guide_family_count_returns_from_two_guide_intersections_to_one_guide() {
    let maze = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides)
        .with_guide_dimension_count(2)
        .expect("guide family grows to two directions")
        .with_site_generation_kind(PatternRecipeSiteGenerationKind::GuideIntersections)
        .expect("two directions create intersections")
        .with_output_kind(0, PatternRecipeOutputKind::Maze)
        .expect("two-guide intersections create maze walls");
    assert!(maze.with_guide_dimension_count(1).is_err());

    let one = maze
        .with_guide_family_dimension_count(1)
        .expect("family authoring prunes only the incompatible intersection intent");
    assert_eq!(
        one.site_generation_kind(),
        PatternRecipeSiteGenerationKind::AlongGuides
    );
    assert_eq!(
        one.output_kinds().expect("reduced output projects"),
        vec![PatternRecipeOutputKind::StructuralPaths]
    );
    let PatternStructureRecipe::OrderedOutputs { definition, .. } = one.structure else {
        panic!("family count transition retains canonical painter order")
    };
    let PatternStructureRecipe::GeneralizedStraightGuides {
        dimensions,
        product:
            GeneralizedSiteProductDraft::AlongGuides {
                dimension_indices, ..
            },
        ..
    } = *definition
    else {
        panic!("reduced recipe retains one straight-guide foundation")
    };
    assert_eq!(dimensions.len(), 1);
    assert_eq!(dimension_indices, vec![0]);
}

/// Proves Guide Faces grows only within its three-dimension structural maximum.
///
/// # Panics
///
/// Panics when the domain does not preserve wrapper selection order or allows a fourth guide to
/// create an invalid four-dimension Guide Faces reference.
#[test]
fn guide_count_growth_preserves_guide_face_bounds() {
    let definition = PatternStructureRecipe::GeneralizedStraightGuides {
        name: "guide faces".into(),
        coverage: CoveragePolicy {
            guard_steps: 1,
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
            merge_epsilon: 0.0,
        },
        orientation: MarkOrientationDraft::Fixed,
    };
    let three = PatternDefinitionRecipe::guide_faces(definition, vec![0, 1])
        .with_guide_dimension_count(3)
        .expect("Guide Faces can grow to three selected dimensions");
    let four = three
        .with_guide_dimension_count(4)
        .expect("a fourth guide stays outside the bounded Guide Faces selection");
    let PatternStructureRecipe::GuideFaceRegions {
        definition,
        dimension_indices,
    } = four.structure
    else {
        panic!("Guide Faces wrapper remains present")
    };
    assert_eq!(dimension_indices, vec![0, 1, 2]);
    assert!(matches!(
        definition.as_ref(),
        PatternStructureRecipe::GeneralizedStraightGuides {
            dimensions,
            product: GeneralizedSiteProductDraft::Intersections {
                dimension_indices,
                ..
            },
            ..
        } if dimensions.len() == 4 && dimension_indices == &vec![0, 1, 2, 3]
    ));
}

/// Proves New guide construction can reach every applicable built-in output family.
///
/// # Panics
///
/// Panics when a three-guide intersection starter cannot create marks, paths, connections, maze,
/// Voronoi, Guide Faces, or a nested custom shape through ID-free domain transitions.
#[test]
fn new_guide_recipe_reaches_all_applicable_output_constructions() {
    let guide = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides)
        .with_guide_dimension_count(3)
        .expect("guide starter grows")
        .with_site_generation_kind(PatternRecipeSiteGenerationKind::GuideIntersections)
        .expect("three guides create intersections");
    for kind in [
        PatternRecipeOutputKind::Marks,
        PatternRecipeOutputKind::CustomShapeMarks,
        PatternRecipeOutputKind::StructuralPaths,
        PatternRecipeOutputKind::Connections,
        PatternRecipeOutputKind::Maze,
        PatternRecipeOutputKind::VoronoiRegions,
        PatternRecipeOutputKind::GuideFaceRegions,
    ] {
        let output = guide
            .with_output_kind(0, kind)
            .unwrap_or_else(|error| panic!("{kind:?} must be reachable: {error}"));
        assert_eq!(
            output.output_kinds().expect("output kinds project"),
            vec![kind]
        );
    }
    assert!(
        guide
            .with_output_kind(0, PatternRecipeOutputKind::CurveMotif)
            .is_err(),
        "Curve Motif stays unavailable until Site Generation is one-guide Along Guides",
    );
}

/// Proves every New count path converges on one usable three-guide intersection lattice.
///
/// # Panics
///
/// Panics when selecting intersections or resizing an intersection recipe fails to restore the
/// canonical angles, common phase, equal spacing, and complete direction selection.
#[test]
fn new_three_guide_intersections_lock_one_canonical_triangular_layout() {
    let family_card = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides)
        .with_guide_dimension_count(3)
        .expect("guide starter grows directly");
    assert!(
        family_card.has_locked_triangular_intersection_layout(),
        "the canonical three-guide family stays locked before its site method is selected"
    );
    let stepped_family_card =
        PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides)
            .with_guide_dimension_count(2)
            .expect("guide starter first grows to two directions")
            .with_guide_dimension_count(3)
            .expect("two-guide family then grows to three directions");
    assert!(
        stepped_family_card.has_locked_triangular_intersection_layout(),
        "the three-guide family locks even when reached through two directions"
    );
    let direct = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides)
        .with_guide_dimension_count(3)
        .expect("guide starter grows directly")
        .with_site_generation_kind(PatternRecipeSiteGenerationKind::GuideIntersections)
        .expect("direct three-guide recipe creates intersections");
    let stepped = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides)
        .with_guide_dimension_count(2)
        .expect("guide starter grows through two")
        .with_guide_dimension_count(3)
        .expect("two-guide recipe grows through three")
        .with_site_generation_kind(PatternRecipeSiteGenerationKind::GuideIntersections)
        .expect("stepped three-guide recipe creates intersections");
    let resized = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides)
        .with_guide_dimension_count(4)
        .expect("guide starter grows through four")
        .with_site_generation_kind(PatternRecipeSiteGenerationKind::GuideIntersections)
        .expect("four-guide recipe creates intersections")
        .with_guide_dimension_count(3)
        .expect("four-guide intersections shrink to three");

    for recipe in [direct, stepped, resized] {
        assert!(recipe.has_locked_triangular_intersection_layout());
        let PatternStructureRecipe::GeneralizedStraightGuides {
            dimensions,
            product:
                GeneralizedSiteProductDraft::Intersections {
                    dimension_indices, ..
                },
            ..
        } = recipe.structure
        else {
            panic!("New remains one unwrapped generalized straight-guide family")
        };
        assert_eq!(
            dimensions
                .iter()
                .map(|dimension| dimension.baseline_angle_degrees)
                .collect::<Vec<_>>(),
            vec![0.0, 60.0, 120.0]
        );
        assert!(dimensions.iter().all(|dimension| dimension.phase == 0.0));
        assert!(
            dimensions
                .iter()
                .all(|dimension| dimension.spacing_multiplier == 1.0)
        );
        assert_eq!(dimension_indices, vec![0, 1, 2]);
    }
}

/// Proves New can construct Curve Motif and parametric marks without an existing preset.
///
/// # Panics
///
/// Panics when the guide or parametric starters cannot cross their domain-owned site and output
/// transitions or when a resource-bearing output omits its canonical authored table.
#[test]
fn new_recipe_reaches_curve_motif_and_parametric_mark_constructions() {
    let motif = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides)
        .with_output_kind(0, PatternRecipeOutputKind::CurveMotif)
        .expect("one-guide Along Guides starter can create a motif");
    assert_eq!(
        motif.output_kinds().expect("motif output projects"),
        vec![PatternRecipeOutputKind::CurveMotif]
    );
    assert!(matches!(
        motif.structure,
        PatternStructureRecipe::AuthoredResources { ref resources, .. }
            if resources.len() == 1
                && resources[0].kind() == AuthoredStructureKind::OpenPath
    ));

    let parametric =
        PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Parametric);
    assert_eq!(
        parametric.site_generation_kind(),
        PatternRecipeSiteGenerationKind::ParametricCurve
    );
    let marks = parametric
        .with_site_generation_kind(PatternRecipeSiteGenerationKind::AlongParametricCurve)
        .expect("parametric starter enables equal-arc sites")
        .with_output_kind(0, PatternRecipeOutputKind::Marks)
        .expect("sites along the curve can render marks");
    assert_eq!(
        marks.output_kinds().expect("parametric marks project"),
        vec![PatternRecipeOutputKind::Marks]
    );
}

/// Proves output insertion and removal preserve recipe-local site-use dependencies atomically.
///
/// # Panics
///
/// Panics when an appended output cannot be referenced, removing its target leaves a stale local
/// index, or removing the final output is accepted.
#[test]
fn new_recipe_output_collection_edits_preserve_local_dependencies() {
    let mut recipe = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides)
        .with_appended_output_kind(PatternRecipeOutputKind::Connections)
        .expect("a connected output appends");
    recipe.output_settings[0].source_filter =
        SiteUseFilterRecipe::SitesUnusedBy { output_index: 1 };
    validate_preset_record(&PresetRecord {
        metadata: PresetMetadata {
            id: "new-output-collection".into(),
            name: "New Output Collection".into(),
            category: "test".into(),
            description: "output collection transition witness".into(),
            thumbnail: None,
        },
        recipe: recipe.clone(),
    })
    .expect("dependent two-output recipe validates");
    let reduced = recipe
        .without_output(1)
        .expect("referenced output removes atomically");
    assert_eq!(
        reduced.output_settings[0].source_filter,
        SiteUseFilterRecipe::All
    );
    assert!(reduced.without_output(0).is_err());
}
