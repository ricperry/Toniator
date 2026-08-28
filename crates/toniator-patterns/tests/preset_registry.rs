use toniator_domain::{
    ArtworkWeightResponse, AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft,
    AuthoredStructureKind, CanvasSpec, ChannelId, Document, DocumentHistory, DocumentSession,
    GeneralizedSiteProductDraft, GuideDimensionDraft, MarkOrientation, MarkOrientationDraft,
    MarkPrototype, PatternDefinitionDraft, PatternDefinitionRecipe, PatternMechanism,
    PatternOutputLayer, PatternOutputRealization, PatternStructureRecipe, PresetMetadata,
    PresetRecord, RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy, SourceMapping,
    SourceMappingComponent, SourceReference,
};
use toniator_patterns::{
    BUNDLED_PRESET_REGISTRY_VERSION, LayeredPresetCatalog, PresetOrigin, PresetRegistry,
};

/// Builds a modeled document history whose initial RGB definition is shared by
/// all three channels, exercising default copy and explicit shared semantics.
fn history() -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    DocumentHistory::new(DocumentSession::new(document).unwrap())
}

/// Reconstructs every bundled entry deterministically and proves metadata IDs
/// remain stable, sorted, and independent from document-owned allocation.
#[test]
fn bundled_registry_is_stable_and_reconstructs_every_entry() {
    let registry = PresetRegistry::bundled();
    assert_eq!(registry.version(), BUNDLED_PRESET_REGISTRY_VERSION);
    let ids = registry
        .entries()
        .iter()
        .map(|entry| entry.metadata.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids.len(),
        17,
        "Curve Motif is the seventeenth ordinary built-in"
    );
    assert_eq!(
        ids,
        vec![
            "clustered-dispersion-random-links",
            "curve-motif-rows",
            "even-random-circles",
            "grid-voronoi-scale",
            "one-guide-lines",
            "residual-sites-along-guide",
            "round-spiral-line",
            "round-spiral-marks",
            "source-weighted-dispersion-voronoi",
            "square-spiral-marks",
            "straight-grid-circles",
            "three-guide-cells-scale",
            "three-guide-maze",
            "triagrid-custom-shape-marks",
            "triagrid-spanning-tree",
            "two-guide-cells-uniform-offset",
            "two-guide-maze",
        ]
    );
    for id in ids {
        assert!(registry.reconstruct(id).is_some());
        registry
            .apply_to_selected(&mut history(), ChannelId(1), id)
            .expect("every bundled recipe materializes through the ordinary history boundary");
    }
}

/// Combines personal records deterministically without allowing a display-name dispatch surface.
#[test]
fn layered_catalog_orders_personal_entries_and_applies_by_id() {
    let registry = PresetRegistry::bundled();
    let mut zulu = registry.find("even-random-circles").unwrap().clone();
    zulu.metadata.id = "user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into();
    zulu.metadata.name = "Zulu personal".into();
    let mut alpha = zulu.clone();
    alpha.metadata.id = "user-bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".into();
    alpha.metadata.name = "alpha personal".into();
    let catalog = LayeredPresetCatalog::new(&registry, vec![zulu, alpha])
        .expect("valid personal records combine with immutable built-ins");
    assert!(
        catalog.entries()[..registry.entries().len()]
            .iter()
            .all(|entry| entry.origin == PresetOrigin::BuiltIn)
    );
    assert_eq!(
        catalog.entries()[registry.entries().len()..]
            .iter()
            .map(|entry| entry.preset.metadata.name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha personal", "Zulu personal"]
    );
    catalog
        .apply_to_document_base(&mut history(), "user-bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb")
        .expect("stable personal ID applies through the ordinary document command");
}

/// Rejects invalid personal IDs and case-insensitive catalog name collisions before lookup is exposed.
#[test]
fn layered_catalog_rejects_ambiguous_personal_records() {
    let registry = PresetRegistry::bundled();
    let mut invalid_id = registry.find("even-random-circles").unwrap().clone();
    invalid_id.metadata.id = "not-a-user-id".into();
    invalid_id.metadata.name = "Personal marks".into();
    assert!(LayeredPresetCatalog::new(&registry, vec![invalid_id]).is_err());

    let mut first = registry.find("even-random-circles").unwrap().clone();
    first.metadata.id = "user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into();
    first.metadata.name = "Personal marks".into();
    let mut duplicate_name = first.clone();
    duplicate_name.metadata.id = "user-bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".into();
    duplicate_name.metadata.name = "personal MARKS".into();
    assert!(LayeredPresetCatalog::new(&registry, vec![first, duplicate_name]).is_err());
}

/// Isolates a personal name collision with immutable built-in authority without hiding its warning.
#[test]
fn layered_catalog_warns_and_omits_personal_built_in_name_collisions() {
    let registry = PresetRegistry::bundled();
    let mut personal = registry.find("even-random-circles").unwrap().clone();
    personal.metadata.id = "user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into();
    personal.metadata.name = "EVEN DISPERSION MARKS".into();
    let catalog = LayeredPresetCatalog::new(&registry, vec![personal])
        .expect("personal built-in name collision remains nonfatal");
    assert_eq!(catalog.entries().len(), registry.entries().len());
    assert_eq!(catalog.warnings().len(), 1);
    assert_eq!(
        catalog.warnings()[0].id,
        "user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
    );
    assert!(
        catalog
            .find("user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
            .is_none()
    );
}

/// Pins the seventeenth card to the accepted asymmetric Along Guides Curve Motif authority.
#[test]
fn curve_motif_card_uses_the_accepted_row_cadence_and_phase_recipe() {
    let registry = PresetRegistry::bundled();
    let record = registry
        .find("curve-motif-rows")
        .expect("seventeenth Curve Motif card exists");
    let PatternStructureRecipe::CurveMotifPaths {
        definition,
        motif,
        mirror_alternate_rows,
        alternate_row_phase,
        ..
    } = &record.recipe.structure
    else {
        panic!("Curve Motif card retains its ordinary Curve Motif recipe")
    };
    let PatternStructureRecipe::GeneralizedStraightGuides {
        coverage,
        dimensions,
        product,
        orientation,
        ..
    } = definition.as_ref()
    else {
        panic!("Curve Motif card retains one generalized guide family")
    };
    assert_eq!(coverage.guard_steps, 2);
    assert_eq!(dimensions.len(), 1);
    assert_eq!(dimensions[0].phase, 0.125);
    assert_eq!(dimensions[0].spacing_multiplier, 1.0);
    assert!(matches!(
        product,
        GeneralizedSiteProductDraft::AlongGuides {
            dimension_indices,
            interval_multiplier: 1.0,
            phase: 0.25,
        } if dimension_indices == &vec![0]
    ));
    assert!(matches!(
        orientation,
        MarkOrientationDraft::GuideTangent { dimension_index: 0 }
    ));
    assert_eq!(motif.segments().len(), 3);
    assert!(*mirror_alternate_rows);
    assert_eq!(*alternate_row_phase, Some(0.25));
}

/// Pins the Source-Weighted Voronoi catalog recipe to its full-range Scale and AreaAverage response.
///
/// The assertion reads only recipe data. It confirms luminance remains the structural site-weight
/// mapping while channel source mapping and paint remain document-instance authority rather than a
/// preset-specific evaluator behavior.
#[test]
fn source_weighted_voronoi_recipe_uses_luminance_placement_scale_average_and_full_fill() {
    let registry = PresetRegistry::bundled();
    let recipe = &registry
        .entries()
        .iter()
        .find(|entry| entry.metadata.id == "source-weighted-dispersion-voronoi")
        .expect("bundled source-weighted Voronoi recipe exists")
        .recipe;
    let PatternStructureRecipe::VoronoiRegions { definition } = &recipe.structure else {
        panic!("source-weighted Voronoi remains a region recipe")
    };
    let PatternStructureRecipe::RandomSites {
        density_modulation, ..
    } = definition.as_ref()
    else {
        panic!("source-weighted Voronoi retains random site placement")
    };
    assert!(matches!(
        density_modulation,
        SiteDensityModulation::ArtworkWeighted {
            mapping: SourceMapping {
                component: SourceMappingComponent::Luminance,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        recipe.output_settings.as_slice(),
        [toniator_domain::PatternOutputSettingsRecipe {
            response: toniator_domain::PatternGeometryResponse::Regions(response),
            ..
        }] if response.algorithm == toniator_domain::RegionResizeAlgorithm::Scale
            && response.sampling == toniator_domain::RegionSamplingStrategy::AreaAverage
            && response.minimum_fill == 0.0
            && response.maximum_fill == 1.0
    ));
}

/// Rejects duplicate stable metadata IDs before registry consumers can resolve
/// an ambiguous shortcut or construct a document command.
#[test]
fn registry_validation_rejects_duplicate_ids() {
    let entry = PresetRecord {
        metadata: PresetMetadata {
            id: "duplicate".into(),
            name: "Duplicate".into(),
            category: "Test".into(),
            description: "Validation fixture.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::StraightGrid(
            toniator_domain::PatternDefinitionDraft {
                name: "Grid".into(),
                coverage: toniator_domain::CoveragePolicy {
                    guard_steps: 2,
                    additional_margin: 4.5,
                },
            },
        )),
    };
    assert!(PresetRegistry::new(1, vec![entry.clone(), entry]).is_err());
}

/// Rejects an out-of-range exposed-control reference atomically at the document
/// command boundary, leaving the selected channel and history unchanged.
#[test]
fn invalid_recipe_reference_is_rejected_without_publishing_history() {
    let record = PresetRecord {
        metadata: PresetMetadata {
            id: "invalid-guide-index".into(),
            name: "Invalid Guide Index".into(),
            category: "Test".into(),
            description: "Validation fixture.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::GeneralizedStraightGuides {
            name: "Invalid".into(),
            coverage: toniator_domain::CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
            dimensions: vec![GuideDimensionDraft {
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            }],
            product: GeneralizedSiteProductDraft::Intersections {
                dimension_indices: vec![0, 1],
                merge_epsilon: 0.0,
            },
            orientation: MarkOrientationDraft::Fixed,
        }),
    };
    let history = history();
    let before = history.document().clone();
    assert!(PresetRegistry::new(1, vec![record]).is_err());
    assert_eq!(history.document(), &before);
    assert!(!history.can_undo());
}

/// Proves every compound recipe alternative is finalized through the Stage 17A
/// draft authority rather than directly assembling a payload-bearing definition.
#[test]
fn recipe_compound_variants_use_transition_drafts_and_preserve_payloads() {
    let random = PresetRecord {
        metadata: PresetMetadata {
            id: "compound-random".into(),
            name: "Compound Random".into(),
            category: "Test".into(),
            description: "Transition draft fixture.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::RandomSites {
            name: "Compound random".into(),
            coverage: toniator_domain::CoveragePolicy {
                guard_steps: 3,
                additional_margin: 5.0,
            },
            character: RandomSiteCharacter::Clustered {
                cluster_density: 0.5,
                cluster_spread: 3.0,
                cluster_strength: 0.75,
            },
            seed: 42,
            density_modulation: SiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping {
                    component: SourceMappingComponent::Luminance,
                    placement: toniator_domain::SourcePlacement::StretchToCanvas,
                    inverted: true,
                    gain: 0.5,
                    bias: -0.25,
                },
                strength: 0.75,
                response: ArtworkWeightResponse::Smoothstep,
            },
            exclusion: SiteExclusionPolicy::MinimumCenterDistance { minimum: 1.5 },
            maximum_attempts: 100,
            maximum_neighbor_checks: 100,
        }),
    };
    let guided = PresetRecord {
        metadata: PresetMetadata {
            id: "guided-grid".into(),
            name: "Guided Grid".into(),
            category: "Test".into(),
            description: "Transition draft fixture.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::GeneralizedStraightGuides {
            name: "Guided grid".into(),
            coverage: toniator_domain::CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
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
            orientation: MarkOrientationDraft::GuideTangent { dimension_index: 1 },
        }),
    };
    let registry = PresetRegistry::new(1, vec![random, guided]).unwrap();
    let mut history = history();
    registry
        .apply_to_selected(&mut history, ChannelId(1), "compound-random")
        .unwrap();
    let definition = history
        .document()
        .pattern_definition_for(ChannelId(1))
        .unwrap();
    assert!(matches!(
        definition.mechanisms[0],
        PatternMechanism::RandomSiteProcess {
            character: RandomSiteCharacter::Clustered { .. },
            ..
        }
    ));
    assert!(matches!(
        definition.mechanisms[1],
        PatternMechanism::SiteDensityModulation {
            modulation: SiteDensityModulation::ArtworkWeighted { .. },
            ..
        }
    ));
    assert!(matches!(
        definition.mechanisms[2],
        PatternMechanism::SiteExclusion {
            policy: SiteExclusionPolicy::MinimumCenterDistance { minimum: 1.5 },
            ..
        }
    ));
    registry
        .apply_to_selected(&mut history, ChannelId(1), "guided-grid")
        .unwrap();
    let definition = history
        .document()
        .pattern_definition_for(ChannelId(1))
        .unwrap();
    let dimensions = match &definition.mechanisms[0] {
        PatternMechanism::StraightGuideDimensions { dimensions, .. } => dimensions,
        _ => unreachable!(),
    };
    assert!(matches!(
        definition.output_layers[0].realization,
        toniator_domain::PatternOutputRealization::MarkPrototype {
            orientation: MarkOrientation::GuideTangent { dimension_id },
            ..
        } if dimension_id == dimensions[1].id
    ));
    assert!(include_str!("../../toniator-domain/src/lib.rs").contains("variant_transition_draft"));
}

/// Applies one preset as an independent selected-channel replacement, then
/// replaces the original linked definition only through explicit shared history.
#[test]
fn default_application_is_independent_and_shared_replacement_discloses_links() {
    let registry = PresetRegistry::bundled();
    let mut history = history();
    let initial_definition = history
        .document()
        .pattern_definition_for(ChannelId(1))
        .unwrap()
        .id;

    let selected = registry
        .apply_to_selected(&mut history, ChannelId(1), "straight-grid-circles")
        .unwrap();
    assert_eq!(selected.affected_channels, vec![ChannelId(1)]);
    assert_ne!(
        history
            .document()
            .pattern_definition_for(ChannelId(1))
            .unwrap()
            .id,
        initial_definition
    );
    assert_eq!(
        history
            .document()
            .pattern_definition_for(ChannelId(2))
            .unwrap()
            .id,
        initial_definition
    );

    let prepared = registry
        .prepare_shared_replacement(&history, initial_definition, "even-random-circles")
        .unwrap();
    assert_eq!(prepared.affected_channels(), &[ChannelId(2), ChannelId(3)]);
    assert_eq!(
        history
            .document()
            .pattern_definition_for(ChannelId(2))
            .unwrap()
            .id,
        initial_definition
    );
    let shared = prepared.confirm(&mut history).unwrap();
    assert_eq!(shared.affected_channels, vec![ChannelId(2), ChannelId(3)]);
    assert!(history.can_undo());
    history.undo().unwrap();
    assert_eq!(
        history
            .document()
            .pattern_definition_for(ChannelId(2))
            .unwrap()
            .id,
        initial_definition
    );
}

/// Rejects a prepared shared replacement when another history transition
/// changes its disclosed linked-channel scope before confirmation.
#[test]
fn stale_shared_disclosure_rejects_before_dispatch_and_fresh_prepare_confirms() {
    let registry = PresetRegistry::bundled();
    let mut history = history();
    let initial_definition = history
        .document()
        .pattern_definition_for(ChannelId(1))
        .unwrap()
        .id;
    let prepared = registry
        .prepare_shared_replacement(&history, initial_definition, "even-random-circles")
        .unwrap();
    assert_eq!(
        prepared.affected_channels(),
        &[ChannelId(1), ChannelId(2), ChannelId(3)]
    );
    registry
        .apply_to_selected(&mut history, ChannelId(1), "straight-grid-circles")
        .unwrap();
    let after_retarget = history.document().clone();
    let revision_after_retarget = history.revision();
    assert!(prepared.confirm(&mut history).is_err());
    assert_eq!(history.document(), &after_retarget);
    assert_eq!(history.revision(), revision_after_retarget);
    let fresh = registry
        .prepare_shared_replacement(&history, initial_definition, "even-random-circles")
        .unwrap();
    assert_eq!(fresh.affected_channels(), &[ChannelId(2), ChannelId(3)]);
    let confirmed = fresh.confirm(&mut history).unwrap();
    assert_eq!(
        confirmed.affected_channels,
        vec![ChannelId(2), ChannelId(3)]
    );
}

/// Materializes an ID-free shape preset as one document resource plus one typed definition,
/// then proves undo and redo publish or remove both pieces as a single history transition.
#[test]
fn shape_preset_materialization_is_atomic_and_uses_an_ordinary_typed_reference() {
    let points = [
        AuthoredPoint2 { x: -2.0, y: -1.0 },
        AuthoredPoint2 { x: 2.0, y: -1.0 },
        AuthoredPoint2 { x: 0.0, y: 3.0 },
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
    .expect("the triangle fixture is a finite closed shape");
    let registry = PresetRegistry::new(
        1,
        vec![PresetRecord {
            metadata: PresetMetadata {
                id: "shape-grid".into(),
                name: "Shape Grid".into(),
                category: "Test".into(),
                description: "Atomic shape materialization fixture.".into(),
                thumbnail: None,
            },
            recipe: PatternDefinitionRecipe::marks(
                PatternStructureRecipe::AuthoredClosedShapeMarks {
                    definition: Box::new(PatternStructureRecipe::StraightGrid(
                        PatternDefinitionDraft {
                            name: "Triangle grid".into(),
                            coverage: toniator_domain::CoveragePolicy {
                                guard_steps: 2,
                                additional_margin: 4.5,
                            },
                        },
                    )),
                    shape,
                },
            ),
        }],
    )
    .expect("the shape registry entry is valid");
    let mut history = history();
    let before = history.document().clone();
    let result = registry
        .apply_to_selected(&mut history, ChannelId(1), "shape-grid")
        .expect("shape preset materialization succeeds");
    assert_eq!(result.affected_channels, vec![ChannelId(1)]);
    assert_eq!(history.document().authored_structures().len(), 1);
    let structure = &history.document().authored_structures()[0];
    let definition = history
        .document()
        .pattern_definition_for(ChannelId(1))
        .expect("the selected channel targets the materialized definition");
    assert!(matches!(
        definition.output_layers.as_slice(),
        [PatternOutputLayer {
            realization: PatternOutputRealization::MarkPrototype {
                prototype: MarkPrototype::AuthoredClosedShape { structure_id },
                ..
            },
            ..
        }] if *structure_id == structure.id()
    ));
    let after = history.document().clone();
    history.undo().expect("the atomic preset transition undoes");
    assert_eq!(history.document(), &before);
    history.redo().expect("the atomic preset transition redoes");
    assert_eq!(history.document(), &after);
}
