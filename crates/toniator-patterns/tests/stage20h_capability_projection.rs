use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureId,
    AuthoredStructureKind, CanvasSpec, CoveragePolicy, Document, DocumentId,
    GeneralizedSiteProduct, GuideDimension, GuideDimensionId, GuidePrototype, GuidePrototypeKind,
    GuideRepetition, GuideSiteProductCapability, MarkOrientation, MarkOrientationKind,
    MarkPrototype, MarkPrototypeKind, PatternCapabilityScope, PatternDefinition,
    PatternDefinitionBundle, PatternDefinitionId, PatternFamilyCapabilityProjection,
    PatternGeometryResponse, PatternMechanismId, PatternOutputCapabilityProjection,
    PatternOutputLayerId, PatternOutputSettings, RandomCharacterKind, RandomSiteCharacter,
    RegionGeometryResponse, SiteDensityModulation, SiteExclusionPolicy, SourceReference,
    StraightGuideDimension, StraightGuideRepetition,
};
use toniator_patterns::{
    StructuralProductCapability, resolve_document_pattern_pipeline, resolve_pattern_pipeline,
};

/// Builds a valid modeled document whose base selects the supplied tested definition.
fn document_for(definition: PatternDefinition, structures: Vec<AuthoredStructure>) -> Document {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 120.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .expect("base document validates");
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition.id;
    Document::with_source_topology_and_authored_structures(
        DocumentId(91),
        base.canvas().clone(),
        SourceReference::Unassigned,
        vec![PatternDefinitionBundle {
            output_settings: definition
                .output_layers
                .iter()
                .map(|output| PatternOutputSettings {
                    output_layer_id: output.id(),
                    response: match output {
                        toniator_domain::PatternOutputLayer::CircularMarks { .. }
                        | toniator_domain::PatternOutputLayer::MarkPrototype { .. } => {
                            PatternGeometryResponse::Marks(toniator_domain::MarkGeometryResponse {
                                minimum_fill: 0.0,
                                maximum_fill: 1.0,
                            })
                        }
                        toniator_domain::PatternOutputLayer::Regions { .. } => {
                            PatternGeometryResponse::Regions(RegionGeometryResponse::default())
                        }
                        _ => panic!("Stage 20H fixture owns only mark outputs"),
                    },
                })
                .collect(),
            definition,
        }],
        settings,
        base.channel_model().expect("modeled fixture").to_owned(),
        base.channel_topology().expect("modeled fixture").clone(),
        structures,
    )
    .expect("tested document validates")
}

/// Builds one authored open guide resource for document-aware generic pipeline resolution.
fn open_line() -> AuthoredStructure {
    AuthoredStructure::new(
        AuthoredStructureId(1),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 40.0 },
            end: AuthoredPoint2 { x: 120.0, y: 40.0 },
        }],
    )
    .expect("open guide validates")
}

/// Proves definition-only accepted pipeline products agree with the domain workflow projection.
#[test]
fn definition_only_pipeline_products_match_domain_projection() {
    let definitions = [
        PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(101),
            "not a capability selector",
            PatternMechanismId(101),
            PatternMechanismId(102),
            PatternOutputLayerId(103),
            vec![
                StraightGuideDimension {
                    id: GuideDimensionId(1),
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: GuideDimensionId(2),
                    baseline_angle_degrees: 90.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: GuideDimensionId(3),
                    baseline_angle_degrees: 135.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
            ],
            GeneralizedSiteProduct::AlongGuides {
                dimensions: vec![GuideDimensionId(1)],
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
        ),
        PatternDefinition::random_sites(
            PatternDefinitionId(111),
            "also ignored",
            PatternMechanismId(111),
            PatternMechanismId(112),
            PatternMechanismId(113),
            PatternMechanismId(114),
            PatternOutputLayerId(115),
            RandomSiteCharacter::Clustered {
                cluster_density: 0.5,
                cluster_spread: 2.0,
                cluster_strength: 0.5,
            },
            19,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::VisibleMarkMargin {
                margin: 0.0,
                sizing: toniator_domain::VisibleMarkSizingPolicy::MaximumSupportRadius,
            },
            10_000,
            10_000,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
        ),
    ];
    for definition in definitions {
        let plan = resolve_pattern_pipeline(&definition).expect("definition-only plan resolves");
        let document = document_for(definition, Vec::new());
        let projection = document
            .pattern_capabilities(PatternCapabilityScope::DocumentBase)
            .expect("domain projection resolves");
        assert_eq!(projection.outputs.len(), plan.ordered_outputs.len());
        let projected_record = &projection.outputs[0];
        let PatternOutputCapabilityProjection::Marks(projected_output) =
            &projected_record.structural
        else {
            panic!("mark-only fixture must project a mark output");
        };
        let plan_output = &plan.ordered_outputs[0];
        let (prototype, orientation) = plan_output.marks().expect("mark output authority");
        assert_eq!(
            projected_output.prototype,
            match prototype {
                MarkPrototype::Circle => MarkPrototypeKind::Circle,
                MarkPrototype::AuthoredClosedShape { .. } => MarkPrototypeKind::AuthoredClosedShape,
            }
        );
        assert_eq!(
            projected_output.orientation,
            match orientation {
                MarkOrientation::Fixed => MarkOrientationKind::Fixed,
                MarkOrientation::GuideTangent { .. } => MarkOrientationKind::GuideTangent,
                MarkOrientation::GuideNormal { .. } => MarkOrientationKind::GuideNormal,
            }
        );
        match (&projection.family, plan.family.product) {
            (
                PatternFamilyCapabilityProjection::Grid(grid),
                StructuralProductCapability::AlongGuideSites,
            ) => {
                assert_eq!(grid.guides.count, plan.family.dimensions.len() as u8);
                assert!(grid.generator.density && !grid.generator.seed);
                assert!(grid.guides.spacing && grid.guides.phase);
                assert!(!grid.guides.editable_curve);
                assert!(grid.guides.prototype_kinds.is_empty());
                assert_eq!(grid.site_product, GuideSiteProductCapability::AlongGuides);
                assert!(plan.family.random.is_none());
            }
            (
                PatternFamilyCapabilityProjection::Dispersion(dispersion),
                StructuralProductCapability::RandomSites,
            ) => {
                assert!(dispersion.generator.density && dispersion.generator.seed);
                let random = plan
                    .family
                    .random
                    .as_ref()
                    .expect("random plan retains its chain");
                assert_eq!(
                    dispersion.character,
                    match &random.character {
                        RandomSiteCharacter::RawUniform => RandomCharacterKind::RawUniform,
                        RandomSiteCharacter::Even { .. } => RandomCharacterKind::Even,
                        RandomSiteCharacter::Clustered { .. } => RandomCharacterKind::Clustered,
                    }
                );
                assert_eq!(
                    dispersion.density_modulation,
                    match &random.density_modulation {
                        SiteDensityModulation::Uniform =>
                            toniator_domain::DensityModulationKind::Uniform,
                        SiteDensityModulation::ArtworkWeighted { .. } => {
                            toniator_domain::DensityModulationKind::ArtworkWeighted
                        }
                    }
                );
                assert_eq!(
                    dispersion.exclusion,
                    match &random.exclusion {
                        SiteExclusionPolicy::None => toniator_domain::ExclusionKind::None,
                        SiteExclusionPolicy::MinimumCenterDistance { .. } => {
                            toniator_domain::ExclusionKind::MinimumCenterDistance
                        }
                        SiteExclusionPolicy::VisibleMarkMargin { .. } => {
                            toniator_domain::ExclusionKind::VisibleMarkMargin
                        }
                    }
                );
            }
            other => panic!("accepted family product diverged from domain capability: {other:?}"),
        }
    }
}

/// Proves document-aware generic guide resolution agrees with the domain's active prototype projection.
#[test]
fn document_aware_generic_pipeline_matches_active_resource_projection() {
    let definition = PatternDefinition::generalized_guides(
        PatternDefinitionId(121),
        "generic resource",
        PatternMechanismId(121),
        PatternMechanismId(122),
        PatternOutputLayerId(123),
        vec![
            GuideDimension {
                id: GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(1),
                },
                repetition: GuideRepetition::Single,
            },
            GuideDimension {
                id: GuideDimensionId(2),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                prototype: GuidePrototype::CircularArc {
                    center: AuthoredPoint2 { x: 60.0, y: 40.0 },
                    radius: 20.0,
                    start_angle_degrees: 0.0,
                    sweep_angle_degrees: 180.0,
                },
                repetition: GuideRepetition::Single,
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
            merge_epsilon: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    let document = document_for(definition.clone(), vec![open_line()]);
    let plan = resolve_document_pattern_pipeline(&document, &definition)
        .expect("document-aware generic plan resolves");
    let projection = document
        .pattern_capabilities(PatternCapabilityScope::DocumentBase)
        .expect("generic projection resolves");
    let PatternFamilyCapabilityProjection::Grid(grid) = projection.family else {
        panic!("generic definition projects a grid")
    };
    let generic = plan
        .family
        .generic_guides
        .expect("plan retains generic resources");
    assert_eq!(
        plan.family.product,
        StructuralProductCapability::GuideIntersections
    );
    assert_eq!(grid.guides.count, generic.dimensions.len() as u8);
    assert!(grid.generator.density && !grid.generator.seed);
    assert!(grid.guides.spacing && grid.guides.phase);
    assert!(grid.guides.editable_curve);
    assert_eq!(
        grid.guides.prototype_kinds,
        generic
            .dimensions
            .iter()
            .map(|dimension| match &dimension.prototype {
                GuidePrototype::AuthoredOpenPath { .. } => GuidePrototypeKind::AuthoredOpenPath,
                GuidePrototype::CircularArc { .. } => GuidePrototypeKind::CircularArc,
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(grid.site_product, GuideSiteProductCapability::Intersections);
}
