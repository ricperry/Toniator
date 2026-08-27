use toniator_domain::{
    ArtworkWeightResponse, AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure,
    AuthoredStructureId, AuthoredStructureKind, CanvasSpec, ChannelId, CoveragePolicy,
    DensityEditedAxis, DensityMetric2D, Document, DocumentCommand, DocumentId,
    GeneralizedSiteProduct, GuideCapabilities, GuideDimension, GuideDimensionId, GuidePrototype,
    GuidePrototypeKind, GuideRepetition, GuideSiteProductCapability, MarkGeometryResponse,
    MarkOrientation, MarkOrientationKind, MarkOutputCapabilityProjection, MarkPrototype,
    MarkPrototypeKind, PatternCapabilityScope, PatternDefinition, PatternDefinitionBundle,
    PatternDefinitionId, PatternFamilyCapabilityProjection, PatternGeometryResponse,
    PatternMechanismId, PatternOutputCapabilityProjection, PatternOutputLayerId,
    PatternOutputRealization, PatternOutputSettings, PropertyTarget, RandomCharacterKind,
    RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy, SourceMapping,
    SourceMappingComponent, SourcePlacement, SourceReference,
};

/// Builds the current modeled document used to verify base and effective authority scopes.
fn default_document() -> Document {
    Document::new_default_document(
        CanvasSpec {
            width: 120.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document validates")
}

/// Rebuilds the retained modeled topology against one supplied active definition and structures.
fn document_for(definition: PatternDefinition, structures: Vec<AuthoredStructure>) -> Document {
    let base = default_document();
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition.id;
    Document::with_source_topology_and_authored_structures(
        DocumentId(81),
        base.canvas().clone(),
        SourceReference::Unassigned,
        vec![PatternDefinitionBundle {
            output_settings: definition
                .output_layers
                .iter()
                .map(|output| PatternOutputSettings {
                    output_layer_id: output.id(),
                    response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                        minimum_fill: 0.0,
                        maximum_fill: 1.0,
                    }),
                })
                .collect(),
            definition,
        }],
        settings,
        base.channel_model().expect("modeled fixture").to_owned(),
        base.channel_topology().expect("modeled fixture").clone(),
        structures,
    )
    .expect("recipe fixture validates")
}

/// Builds one valid typed random recipe with explicit active mechanism discriminants.
fn random_definition(
    character: RandomSiteCharacter,
    modulation: SiteDensityModulation,
    exclusion: SiteExclusionPolicy,
) -> PatternDefinition {
    PatternDefinition::random_sites(
        PatternDefinitionId(44),
        "ignored display name",
        PatternMechanismId(41),
        PatternMechanismId(42),
        PatternMechanismId(43),
        PatternMechanismId(44),
        PatternOutputLayerId(45),
        character,
        1234,
        modulation,
        exclusion,
        10_000,
        10_000,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    )
}

/// Builds an authored line used only as a validated generic-guide resource reference.
fn open_line(id: u64, start: AuthoredPoint2, end: AuthoredPoint2) -> AuthoredStructure {
    AuthoredStructure::new(
        AuthoredStructureId(id),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line { start, end }],
    )
    .expect("open line validates")
}

/// Builds an authored closed shape used only as a validated mark-prototype resource reference.
fn closed_shape(id: u64) -> AuthoredStructure {
    AuthoredStructure::new(
        AuthoredStructureId(id),
        AuthoredStructureKind::ClosedShape,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                end: AuthoredPoint2 { x: 1.0, y: 0.0 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 1.0, y: 0.0 },
                end: AuthoredPoint2 { x: 0.0, y: 0.0 },
            },
        ],
    )
    .expect("closed shape validates")
}

/// Proves document-base and inherited channel projection preserve one resolved
/// structural recipe while exposing their intentionally different edit scopes.
#[test]
fn base_and_inherited_channel_project_the_same_legacy_structure() {
    let document = default_document();
    let base = document
        .pattern_capabilities(PatternCapabilityScope::DocumentBase)
        .expect("base projection resolves");
    let inherited = document
        .pattern_capabilities(PatternCapabilityScope::Channel(ChannelId(1)))
        .expect("inherited projection resolves");
    assert_eq!(base.definition_id, inherited.definition_id);
    assert_eq!(base.features, inherited.features);
    assert_eq!(base.family, inherited.family);
    assert_eq!(base.outputs, inherited.outputs);
    assert!(base.active_controls.iter().all(|descriptor| {
        !matches!(
            descriptor.target,
            PropertyTarget::Channel(_) | PropertyTarget::ChannelOutput(_, _)
        )
    }));
    assert!(inherited.active_controls.iter().any(|descriptor| {
        matches!(
            descriptor.target,
            PropertyTarget::Channel(_) | PropertyTarget::ChannelOutput(_, _)
        )
    }));
    assert_eq!(
        base.family,
        PatternFamilyCapabilityProjection::Grid(toniator_domain::GridCapabilityProjection {
            generator: toniator_domain::GeneratorCapabilities {
                density: true,
                seed: false,
            },
            guides: GuideCapabilities {
                count: 2,
                spacing: false,
                phase: false,
                editable_curve: false,
                prototype_kinds: Vec::new(),
            },
            site_product: GuideSiteProductCapability::Intersections,
        })
    );
    assert!(matches!(
        base.outputs.as_slice(),
        [toniator_domain::PatternOutputCapabilityRecord {
            structural: PatternOutputCapabilityProjection::Marks(MarkOutputCapabilityProjection {
                prototype: MarkPrototypeKind::Circle,
                orientation: MarkOrientationKind::Fixed,
                fill_range: true
            }),
            response: toniator_domain::PatternGeometryResponse::Marks(_),
            ..
        }]
    ));
}

/// Proves channel replacement changes only that channel's active projection and scalar deltas do not.
#[test]
fn override_and_scalar_deltas_preserve_scope_and_structure_authority() {
    let document = default_document();
    let random = random_definition(
        RandomSiteCharacter::Even {
            minimum_center_distance: 3.0,
        },
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
    );
    let (document, _) = document
        .apply_command(&DocumentCommand::AddTypedPatternDefinition { definition: random })
        .expect("random definition installs");
    let (document, _) = document
        .apply_command(&DocumentCommand::SetChannelPatternDefinitionOverride {
            base: document.pattern_settings().clone(),
            channel_id: ChannelId(2),
            definition_id: PatternDefinitionId(44),
        })
        .expect("override installs");
    let before = document
        .pattern_capabilities(PatternCapabilityScope::Channel(ChannelId(2)))
        .expect("override projection resolves");
    assert_eq!(
        before.family,
        PatternFamilyCapabilityProjection::Dispersion(
            toniator_domain::DispersionCapabilityProjection {
                generator: toniator_domain::GeneratorCapabilities {
                    density: true,
                    seed: true,
                },
                character: RandomCharacterKind::Even,
                density_modulation: toniator_domain::DensityModulationKind::Uniform,
                exclusion: toniator_domain::ExclusionKind::None,
            }
        )
    );
    assert_ne!(
        document
            .pattern_capabilities(PatternCapabilityScope::DocumentBase)
            .expect("base projection resolves"),
        before
    );
    let density = document
        .set_channel_density_for_effective(
            ChannelId(2),
            DensityEditedAxis::AcrossX,
            DensityMetric2D {
                across_x: 20.0,
                across_y: 12.0,
                aspect_locked: false,
            },
        )
        .expect("density command builds");
    let (document, _) = document.apply_command(&density).expect("density applies");
    let rotation = document
        .set_channel_pattern_rotation_for_effective(ChannelId(2), 27.0)
        .expect("rotation command builds");
    let (document, _) = document.apply_command(&rotation).expect("rotation applies");
    let shape_rotation = document
        .set_channel_shape_rotation_for_effective(ChannelId(2), -12.0)
        .expect("shape-rotation command builds");
    let (document, _) = document
        .apply_command(&shape_rotation)
        .expect("shape-rotation applies");
    let response = document
        .set_channel_output_response_for_effective(
            ChannelId(2),
            PatternOutputLayerId(45),
            PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.25,
                maximum_fill: 1.5,
            }),
        )
        .expect("mark-response command builds");
    let (document, _) = document
        .apply_command(&response)
        .expect("mark-response applies");
    let after = document
        .pattern_capabilities(PatternCapabilityScope::Channel(ChannelId(2)))
        .expect("delta projection resolves");
    assert_eq!(after.definition_id, before.definition_id);
    assert_eq!(after.family, before.family);
    assert_eq!(
        after.outputs[0].output_layer_id,
        before.outputs[0].output_layer_id
    );
    assert_eq!(after.outputs[0].structural, before.outputs[0].structural);
    assert!(
        matches!(after.outputs[0].response, toniator_domain::PatternGeometryResponse::Marks(MarkGeometryResponse { minimum_fill, maximum_fill }) if minimum_fill == 0.25 && maximum_fill == 1.5)
    );
}

/// Proves typed and generic guides retain only active structural facts in stored order.
#[test]
fn typed_and_generic_guides_project_counts_products_and_active_resources() {
    for count in 1_u64..=4 {
        let typed = PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(50 + count),
            "typed guides",
            PatternMechanismId(51),
            PatternMechanismId(52),
            PatternOutputLayerId(53),
            (1..=count)
                .map(|id| toniator_domain::StraightGuideDimension {
                    id: GuideDimensionId(id),
                    baseline_angle_degrees: id as f64 * 30.0,
                    phase: 0.0,
                    repetition: toniator_domain::StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                })
                .collect(),
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
        );
        let typed_projection = document_for(typed, Vec::new())
            .pattern_capabilities(PatternCapabilityScope::DocumentBase)
            .expect("typed projection resolves");
        let PatternFamilyCapabilityProjection::Grid(typed_grid) = typed_projection.family else {
            panic!("typed definition projects a grid")
        };
        assert_eq!(typed_grid.guides.count, count as u8);
        assert!(typed_grid.guides.spacing && typed_grid.guides.phase);
        assert!(!typed_grid.guides.editable_curve);
        assert_eq!(
            typed_grid.site_product,
            GuideSiteProductCapability::AlongGuides
        );
    }

    let mut generic = PatternDefinition::generalized_guides(
        PatternDefinitionId(61),
        "generic guides",
        PatternMechanismId(61),
        PatternMechanismId(62),
        PatternOutputLayerId(63),
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
                    radius: 30.0,
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
        MarkOrientation::GuideNormal {
            dimension_id: GuideDimensionId(1),
        },
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    let PatternOutputRealization::MarkPrototype { prototype, .. } =
        &mut generic.output_layers[0].realization
    else {
        panic!("generic definition owns marks")
    };
    *prototype = MarkPrototype::AuthoredClosedShape {
        structure_id: AuthoredStructureId(3),
    };
    let generic_projection = document_for(
        generic,
        vec![
            open_line(
                1,
                AuthoredPoint2 { x: 0.0, y: 40.0 },
                AuthoredPoint2 { x: 120.0, y: 40.0 },
            ),
            closed_shape(3),
        ],
    )
    .pattern_capabilities(PatternCapabilityScope::DocumentBase)
    .expect("generic projection resolves");
    let PatternFamilyCapabilityProjection::Grid(generic_grid) = generic_projection.family else {
        panic!("generic definition projects a grid")
    };
    assert_eq!(generic_grid.guides.count, 2);
    assert!(generic_grid.guides.editable_curve);
    assert_eq!(
        generic_grid.guides.prototype_kinds,
        vec![
            GuidePrototypeKind::AuthoredOpenPath,
            GuidePrototypeKind::CircularArc,
        ]
    );
    assert!(matches!(
        generic_projection.outputs.as_slice(),
        [toniator_domain::PatternOutputCapabilityRecord {
            structural: PatternOutputCapabilityProjection::Marks(MarkOutputCapabilityProjection {
                prototype: MarkPrototypeKind::AuthoredClosedShape,
                orientation: MarkOrientationKind::GuideNormal,
                fill_range: true
            }),
            response: toniator_domain::PatternGeometryResponse::Marks(_),
            ..
        }]
    ));
}

/// Proves every accepted random discriminant projects without name dispatch or future branches.
#[test]
fn dispersion_variants_are_deterministic_and_missing_scope_is_an_error() {
    let variants = [
        (
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            RandomCharacterKind::RawUniform,
            toniator_domain::DensityModulationKind::Uniform,
            toniator_domain::ExclusionKind::None,
        ),
        (
            RandomSiteCharacter::Even {
                minimum_center_distance: 2.0,
            },
            SiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping {
                    component: SourceMappingComponent::Luminance,
                    placement: SourcePlacement::StretchToCanvas,
                    inverted: false,
                    gain: 1.0,
                    bias: 0.0,
                },
                strength: 0.5,
                response: ArtworkWeightResponse::Linear,
            },
            SiteExclusionPolicy::MinimumCenterDistance { minimum: 1.0 },
            RandomCharacterKind::Even,
            toniator_domain::DensityModulationKind::ArtworkWeighted,
            toniator_domain::ExclusionKind::MinimumCenterDistance,
        ),
    ];
    for (
        character,
        modulation,
        exclusion,
        expected_character,
        expected_modulation,
        expected_exclusion,
    ) in variants
    {
        let document = document_for(
            random_definition(character, modulation, exclusion),
            Vec::new(),
        );
        let first = document
            .pattern_capabilities(PatternCapabilityScope::DocumentBase)
            .expect("random projection resolves");
        let second = document
            .pattern_capabilities(PatternCapabilityScope::DocumentBase)
            .expect("random projection repeats");
        assert_eq!(first, second);
        assert_eq!(
            first.family,
            PatternFamilyCapabilityProjection::Dispersion(
                toniator_domain::DispersionCapabilityProjection {
                    generator: toniator_domain::GeneratorCapabilities {
                        density: true,
                        seed: true,
                    },
                    character: expected_character,
                    density_modulation: expected_modulation,
                    exclusion: expected_exclusion,
                }
            )
        );
    }
    assert_eq!(
        default_document()
            .pattern_capabilities(PatternCapabilityScope::Channel(ChannelId(999)))
            .expect_err("missing channel rejects")
            .path(),
        "channel.id"
    );
}
